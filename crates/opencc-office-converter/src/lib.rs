//! Shared Office and EPUB document conversion support for OpenCC CLIs.

use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{self, BufReader, Cursor, Read, Seek, Write};
use std::path::{Path, PathBuf};

use regex::{Captures, Regex};
use zip::{
    write::{ExtendedFileOptions, FileOptions},
    CompressionMethod, ZipArchive, ZipWriter,
};

/// Converts a text fragment with an OpenCC-compatible backend.
pub trait TextConverter {
    fn convert(&self, input: &str, config: &str, punctuation: bool) -> String;
}

impl<F> TextConverter for F
where
    F: Fn(&str, &str, bool) -> String,
{
    fn convert(&self, input: &str, config: &str, punctuation: bool) -> String {
        self(input, config, punctuation)
    }
}

/// Result of a document conversion operation.
pub struct ConversionResult {
    pub success: bool,
    pub message: Box<str>,
}

/// Converter for ZIP-based Office and EPUB documents.
pub struct OfficeConverter;

struct FontPatterns {
    docx: Regex,
    xlsx: Regex,
    pptx: Regex,
    odt: Regex,
    epub: Regex,
}

impl FontPatterns {
    fn new() -> Self {
        Self {
            docx: Regex::new(r#"(w:(?:eastAsia|ascii|hAnsi|cs)=")(.*?)(")"#).unwrap(),
            xlsx: Regex::new(r#"(val=")(.*?)(")"#).unwrap(),
            pptx: Regex::new(r#"(typeface=")(.*?)(")"#).unwrap(),
            odt: Regex::new(
                r#"((?:style:font-name(?:-asian|-complex)?|svg:font-family|style:name)=['"])([^'"]+)(['"])"#,
            )
            .unwrap(),
            epub: Regex::new(r#"(font-family\s*:\s*)([^;"']+)"#).unwrap(),
        }
    }

    fn get_pattern(&self, format: &str) -> Option<&Regex> {
        match format {
            "docx" => Some(&self.docx),
            "xlsx" => Some(&self.xlsx),
            "pptx" => Some(&self.pptx),
            "odt" | "ods" | "odp" => Some(&self.odt),
            "epub" => Some(&self.epub),
            _ => None,
        }
    }
}

struct XlsxPatterns {
    any_cell: Regex,
    text_node: Regex,
}

impl XlsxPatterns {
    fn new() -> Self {
        Self {
            any_cell: Regex::new(r#"<c\b[^>]*>.*?</c>"#).unwrap(),
            text_node: Regex::new(r#"(<t\b[^>]*>)(.*?)(</t>)"#).unwrap(),
        }
    }
}

thread_local! {
    static FONT_PATTERNS: FontPatterns = FontPatterns::new();
    static XLSX_PATTERNS: XlsxPatterns = XlsxPatterns::new();
}

impl OfficeConverter {
    pub fn convert<C>(
        input_path: &str,
        output_path: &str,
        format: &str,
        converter: &C,
        config: &str,
        punctuation: bool,
        keep_font: bool,
    ) -> io::Result<ConversionResult>
    where
        C: TextConverter + ?Sized,
    {
        Self::convert_path_stream(
            input_path,
            output_path,
            format,
            converter,
            config,
            punctuation,
            keep_font,
        )
    }

    pub fn convert_bytes<C>(
        input_zip: &[u8],
        format: &str,
        converter: &C,
        config: &str,
        punctuation: bool,
        keep_font: bool,
    ) -> io::Result<(Vec<u8>, usize)>
    where
        C: TextConverter + ?Sized,
    {
        Self::validate_input_zip(input_zip)?;
        let format = Self::normalize_format(format)?;
        let reader = Cursor::new(input_zip);

        let out_cursor = Cursor::new(Vec::<u8>::new());
        let mut z_out = ZipWriter::new(out_cursor);

        let converted_count = Self::convert_zip_stream(
            reader,
            &mut z_out,
            &format,
            converter,
            config,
            punctuation,
            keep_font,
        )?;

        let out_cursor = z_out.finish()?;
        let out_bytes = out_cursor.into_inner();
        Self::validate_zip_bytes(&out_bytes)?;
        Ok((out_bytes, converted_count))
    }

    pub fn convert_path_stream<C>(
        input_path: &str,
        output_path: &str,
        format: &str,
        converter: &C,
        config: &str,
        punctuation: bool,
        keep_font: bool,
    ) -> io::Result<ConversionResult>
    where
        C: TextConverter + ?Sized,
    {
        let format = Self::normalize_format(format)?;
        let in_path_abs = Path::new(input_path)
            .canonicalize()
            .unwrap_or_else(|_| PathBuf::from(input_path));
        let out_path = Path::new(output_path);

        let out_path_abs = out_path
            .canonicalize()
            .unwrap_or_else(|_| out_path.to_path_buf());

        if out_path_abs == in_path_abs {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "output_path must differ from input_path",
            ));
        }

        replace_with_temp(out_path, |zip_writer| {
            let file = File::open(input_path)?;
            let reader = BufReader::new(file);

            Self::convert_zip_stream(
                reader,
                zip_writer,
                &format,
                converter,
                config,
                punctuation,
                keep_font,
            )?;

            Ok(())
        })?;

        Ok(ConversionResult {
            success: true,
            message: "Conversion completed.".into(),
        })
    }

    fn convert_zip_stream<R, W, C>(
        reader: R,
        z_out: &mut ZipWriter<W>,
        format: &str,
        converter: &C,
        config: &str,
        punctuation: bool,
        keep_font: bool,
    ) -> io::Result<usize>
    where
        R: Read + Seek,
        W: Write + Seek,
        C: TextConverter + ?Sized,
    {
        let mut zin = ZipArchive::new(reader)?;
        let mut converted_count = 0;

        let mut mimetype_index: Option<usize> = None;
        if format.eq_ignore_ascii_case("epub") {
            mimetype_index = Self::find_mimetype_index(&mut zin)?;
            if mimetype_index.is_none() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "EPUB is missing required mimetype entry",
                ));
            }

            if let Some(mi) = mimetype_index {
                let mut entry = zin.by_index(mi)?;
                let name = entry.name().replace('\\', "/");

                if !Self::is_unsafe_path(Path::new(&name)) && !entry.is_dir() && name == "mimetype"
                {
                    let mut buf = Vec::new();
                    entry.read_to_end(&mut buf)?;

                    let opts: FileOptions<'_, ExtendedFileOptions> =
                        FileOptions::default().compression_method(CompressionMethod::Stored);

                    z_out.start_file("mimetype", opts)?;
                    z_out.write_all(&buf)?;
                }
            }
        }

        for i in 0..zin.len() {
            if format.eq_ignore_ascii_case("epub") && mimetype_index == Some(i) {
                continue;
            }

            let mut entry = zin.by_index(i)?;
            let name = entry.name().replace('\\', "/");

            if Self::is_unsafe_path(Path::new(&name)) {
                continue;
            }

            if entry.is_dir() || name.ends_with('/') {
                let opts: FileOptions<'_, ExtendedFileOptions> =
                    FileOptions::default().compression_method(CompressionMethod::Stored);
                z_out.add_directory(name, opts)?;
                continue;
            }

            if Self::is_target_entry(format, &name) {
                let mut content = String::new();
                entry.read_to_string(&mut content)?;

                let mut font_map = HashMap::new();
                let is_xlsx_shared_strings =
                    format.eq_ignore_ascii_case("xlsx") && Self::is_xlsx_shared_strings(&name);

                if keep_font && (!format.eq_ignore_ascii_case("xlsx") || is_xlsx_shared_strings) {
                    Self::mask_font(&mut content, format, &mut font_map);
                }

                let mut converted = if format.eq_ignore_ascii_case("xlsx") {
                    Self::convert_xlsx_entry(&content, &name, converter, config, punctuation)
                } else {
                    converter.convert(&content, config, punctuation)
                };

                if !font_map.is_empty() {
                    for (marker, original) in font_map {
                        converted = converted.replace(&marker, &original);
                    }
                }

                let opts: FileOptions<'_, ExtendedFileOptions> =
                    FileOptions::default().compression_method(CompressionMethod::Deflated);

                z_out.start_file(name, opts)?;
                z_out.write_all(converted.as_bytes())?;
                converted_count += 1;
            } else {
                z_out.raw_copy_file(entry)?;
            }
        }

        if converted_count == 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("No valid XML/XHTML fragments were converted for format '{format}'."),
            ));
        }

        Ok(converted_count)
    }

    fn validate_input_zip(input_zip: &[u8]) -> io::Result<()> {
        if input_zip.is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "input ZIP bytes must not be empty",
            ));
        }
        Ok(())
    }

    fn normalize_format(format: &str) -> io::Result<String> {
        let normalized = format.trim().to_ascii_lowercase();
        if normalized.is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "format must not be empty",
            ));
        }

        if !Self::is_supported_format(&normalized) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("Unsupported Office/EPUB format: '{format}'."),
            ));
        }

        Ok(normalized)
    }

    fn is_supported_format(format: &str) -> bool {
        matches!(
            format,
            "docx" | "xlsx" | "pptx" | "odt" | "ods" | "odp" | "epub"
        )
    }

    fn validate_zip_bytes(bytes: &[u8]) -> io::Result<()> {
        let cursor = Cursor::new(bytes);
        let _ = ZipArchive::new(cursor)?;
        Ok(())
    }

    fn validate_zip_file(path: &Path) -> io::Result<()> {
        let file = File::open(path)?;
        let reader = BufReader::new(file);
        let _ = ZipArchive::new(reader)?;
        Ok(())
    }

    fn is_target_entry(format: &str, name: &str) -> bool {
        match format {
            "docx" => name == "word/document.xml",
            "xlsx" => {
                name == "xl/sharedStrings.xml"
                    || (name.starts_with("xl/worksheets/") && name.ends_with(".xml"))
            }
            "pptx" => {
                let is_xml = name.ends_with(".xml");
                let is_rels = name.ends_with(".rels");
                let in_slides = name.starts_with("ppt/slides/");
                let in_notes = name.starts_with("ppt/notesSlides/");
                is_xml && !is_rels && (in_slides || in_notes)
            }
            "odt" | "ods" | "odp" => name == "content.xml",
            "epub" => {
                let lower = name.to_ascii_lowercase();
                lower.ends_with(".xhtml")
                    || lower.ends_with(".opf")
                    || lower.ends_with(".ncx")
                    || lower.ends_with(".html")
            }
            _ => false,
        }
    }

    #[inline]
    fn is_xlsx_shared_strings(name: &str) -> bool {
        name == "xl/sharedStrings.xml"
    }

    #[inline]
    fn is_xlsx_worksheet(name: &str) -> bool {
        name.starts_with("xl/worksheets/") && name.ends_with(".xml")
    }

    fn convert_xlsx_entry<C>(
        content: &str,
        name: &str,
        converter: &C,
        config: &str,
        punctuation: bool,
    ) -> String
    where
        C: TextConverter + ?Sized,
    {
        if Self::is_xlsx_shared_strings(name) {
            return converter.convert(content, config, punctuation);
        }

        if Self::is_xlsx_worksheet(name) {
            return XLSX_PATTERNS.with(|patterns| {
                patterns
                    .any_cell
                    .replace_all(content, |cell_caps: &Captures| {
                        let cell_xml = cell_caps.get(0).map(|m| m.as_str()).unwrap_or_default();

                        if !Self::is_inline_string_cell(cell_xml) {
                            return cell_xml.to_owned();
                        }

                        patterns
                            .text_node
                            .replace_all(cell_xml, |text_caps: &Captures| {
                                let open_tag =
                                    text_caps.get(1).map(|m| m.as_str()).unwrap_or_default();
                                let inner_text =
                                    text_caps.get(2).map(|m| m.as_str()).unwrap_or_default();
                                let close_tag =
                                    text_caps.get(3).map(|m| m.as_str()).unwrap_or_default();

                                if inner_text.is_empty() {
                                    return text_caps
                                        .get(0)
                                        .map(|m| m.as_str().to_owned())
                                        .unwrap_or_default();
                                }

                                let converted = converter.convert(inner_text, config, punctuation);
                                let mut out = String::with_capacity(
                                    open_tag.len() + converted.len() + close_tag.len(),
                                );
                                out.push_str(open_tag);
                                out.push_str(&converted);
                                out.push_str(close_tag);
                                out
                            })
                            .into_owned()
                    })
                    .into_owned()
            });
        }

        content.to_owned()
    }

    fn find_mimetype_index<R: Read + Seek>(zin: &mut ZipArchive<R>) -> io::Result<Option<usize>> {
        for i in 0..zin.len() {
            let entry = zin.by_index(i)?;
            let name = entry.name().replace('\\', "/");
            if name == "mimetype" {
                return Ok(Some(i));
            }
        }
        Ok(None)
    }

    fn is_unsafe_path(path: &Path) -> bool {
        path.components().any(|c| {
            matches!(
                c,
                std::path::Component::ParentDir | std::path::Component::RootDir
            )
        })
    }

    fn mask_font(xml: &mut String, format: &str, font_map: &mut HashMap<String, String>) {
        FONT_PATTERNS.with(|patterns| {
            if let Some(re) = patterns.get_pattern(format) {
                let mut counter = 0;
                let mut result_str = String::with_capacity(xml.len() + xml.len() / 10);
                let mut last_end = 0;

                for caps in re.captures_iter(xml) {
                    let marker = format!("__F_O_N_T_{}__", counter);
                    counter += 1;
                    font_map.insert(marker.clone(), caps[2].to_string());

                    let mat = caps.get(0).unwrap();
                    result_str.push_str(&xml[last_end..mat.start()]);
                    result_str.push_str(&caps[1]);
                    result_str.push_str(&marker);

                    if caps.len() > 3 {
                        result_str.push_str(&caps[3]);
                    }
                    last_end = mat.end();
                }
                result_str.push_str(&xml[last_end..]);
                *xml = result_str;
            }
        });
    }

    #[inline]
    fn is_inline_string_cell(cell_xml: &str) -> bool {
        let Some(tag_end) = cell_xml.find('>') else {
            return false;
        };

        let open_tag = &cell_xml[..tag_end];
        open_tag.contains(r#"t="inlineStr""#) || open_tag.contains("t='inlineStr'")
    }
}

fn remove_existing_file(path: &Path) -> io::Result<()> {
    if !path.exists() {
        return Ok(());
    }
    if path.is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("output_path is a directory: {:?}", path),
        ));
    }

    #[cfg(windows)]
    if let Ok(meta) = fs::metadata(path) {
        let mut perms = meta.permissions();
        if perms.readonly() {
            perms.set_readonly(false);
            fs::set_permissions(path, perms)?;
        }
    }

    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e),
    }
}

fn replace_with_temp(
    final_out: &Path,
    write_zip: impl FnOnce(&mut ZipWriter<File>) -> io::Result<()>,
) -> io::Result<()> {
    struct TempFileGuard {
        path: PathBuf,
        committed: bool,
    }

    impl TempFileGuard {
        fn new(path: PathBuf) -> Self {
            Self {
                path,
                committed: false,
            }
        }

        fn commit(&mut self) {
            self.committed = true;
        }
    }

    impl Drop for TempFileGuard {
        fn drop(&mut self) {
            if !self.committed {
                let _ = fs::remove_file(&self.path);
            }
        }
    }

    let ext = final_out
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("zip");
    let tmp_out = final_out.with_extension(format!("tmp.{}", ext));

    let _ = remove_existing_file(&tmp_out);

    let mut guard = TempFileGuard::new(tmp_out.clone());

    {
        let zip_file = File::create(&tmp_out)?;
        let mut zw = ZipWriter::new(zip_file);
        write_zip(&mut zw)?;
        zw.finish()?;
    }

    OfficeConverter::validate_zip_file(&tmp_out)?;

    remove_existing_file(final_out)?;
    fs::rename(&tmp_out, final_out)?;

    guard.commit();

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;
    use zip::{
        write::{ExtendedFileOptions, FileOptions},
        CompressionMethod, ZipArchive, ZipWriter,
    };

    fn fake_convert(input: &str, _config: &str, _punctuation: bool) -> String {
        input.replace("汉语", "漢語")
    }

    fn make_zip(entries: &[(&str, &[u8])]) -> Vec<u8> {
        let mut input_cursor = Cursor::new(Vec::<u8>::new());
        {
            let mut zip = ZipWriter::new(&mut input_cursor);
            let opts: FileOptions<'_, ExtendedFileOptions> =
                FileOptions::default().compression_method(CompressionMethod::Deflated);

            for (name, bytes) in entries {
                zip.start_file(*name, opts.clone()).unwrap();
                zip.write_all(bytes).unwrap();
            }

            zip.finish().unwrap();
        }
        input_cursor.into_inner()
    }

    #[test]
    fn test_convert_bytes_rejects_empty_input() {
        let err = OfficeConverter::convert_bytes(&[], "docx", &fake_convert, "s2t", true, true)
            .expect_err("empty input must be rejected");

        assert_eq!(err.kind(), io::ErrorKind::InvalidInput);
        assert!(err.to_string().contains("must not be empty"));
    }

    #[test]
    fn test_convert_bytes_rejects_invalid_zip() {
        let err =
            OfficeConverter::convert_bytes(b"not a zip", "docx", &fake_convert, "s2t", true, true)
                .expect_err("invalid ZIP input must be rejected");

        assert_ne!(err.kind(), io::ErrorKind::NotFound);
    }

    #[test]
    fn test_convert_bytes_rejects_unsupported_format() {
        let zip = make_zip(&[(
            "word/document.xml",
            "<w:document>汉语</w:document>".as_bytes(),
        )]);
        let err = OfficeConverter::convert_bytes(&zip, "pdf", &fake_convert, "s2t", true, true)
            .expect_err("unsupported format must be rejected");

        assert_eq!(err.kind(), io::ErrorKind::InvalidInput);
        assert!(err.to_string().contains("Unsupported Office/EPUB format"));
    }

    #[test]
    fn test_convert_bytes_rejects_zip_with_no_target_fragments() {
        let zip = make_zip(&[("docProps/core.xml", "<root>汉语</root>".as_bytes())]);
        let err = OfficeConverter::convert_bytes(&zip, "docx", &fake_convert, "s2t", true, true)
            .expect_err("ZIP without target XML must be rejected");

        assert_eq!(err.kind(), io::ErrorKind::InvalidData);
        assert!(err.to_string().contains("No valid XML/XHTML fragments"));
    }

    #[test]
    fn test_convert_bytes_rejects_epub_without_mimetype() {
        let zip = make_zip(&[(
            "OEBPS/content.xhtml",
            "<html><body>汉语</body></html>".as_bytes(),
        )]);
        let err = OfficeConverter::convert_bytes(&zip, "epub", &fake_convert, "s2t", true, true)
            .expect_err("EPUB without mimetype must be rejected");

        assert_eq!(err.kind(), io::ErrorKind::InvalidData);
        assert!(err.to_string().contains("mimetype"));
    }
    #[test]
    fn test_convert_bytes_xlsx_inline_string_cells() {
        let mut input_cursor = Cursor::new(Vec::<u8>::new());
        {
            let mut zip = ZipWriter::new(&mut input_cursor);
            let opts: FileOptions<'_, ExtendedFileOptions> =
                FileOptions::default().compression_method(CompressionMethod::Deflated);

            zip.start_file("[Content_Types].xml", opts.clone()).unwrap();
            zip.write_all(br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"></Types>"#)
                .unwrap();

            zip.start_file("xl/worksheets/sheet1.xml", opts).unwrap();
            zip.write_all("<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?><worksheet xmlns=\"http://schemas.openxmlformats.org/spreadsheetml/2006/main\"><sheetData><row r=\"1\"><c r=\"A1\" t=\"inlineStr\"><is><t>汉语</t></is></c></row></sheetData></worksheet>".as_bytes())
                .unwrap();

            zip.finish().unwrap();
        }

        let (out_bytes, converted_count) = OfficeConverter::convert_bytes(
            input_cursor.get_ref(),
            "xlsx",
            &fake_convert,
            "s2t",
            true,
            true,
        )
        .expect("convert_bytes failed");

        assert_eq!(converted_count, 1);

        let cursor = Cursor::new(out_bytes);
        let mut zip = ZipArchive::new(cursor).expect("Output is not a valid ZIP archive");
        let mut sheet = zip
            .by_name("xl/worksheets/sheet1.xml")
            .expect("Converted xlsx is missing xl/worksheets/sheet1.xml");
        let mut content = String::new();
        sheet.read_to_string(&mut content).unwrap();

        assert!(content.contains("漢語"));
    }

    #[test]
    fn test_convert_bytes_xlsx_formula_untouched() {
        let mut input_cursor = Cursor::new(Vec::<u8>::new());
        {
            let mut zip = ZipWriter::new(&mut input_cursor);
            let opts: FileOptions<'_, ExtendedFileOptions> =
                FileOptions::default().compression_method(CompressionMethod::Deflated);

            zip.start_file("[Content_Types].xml", opts.clone()).unwrap();
            zip.write_all(br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"></Types>"#)
                .unwrap();

            zip.start_file("xl/worksheets/sheet1.xml", opts).unwrap();
            zip.write_all(
                "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\
                 <worksheet xmlns=\"http://schemas.openxmlformats.org/spreadsheetml/2006/main\">\
                 <sheetData><row r=\"1\">\
                 <c r=\"A1\" t=\"inlineStr\"><is><t>汉语</t></is></c>\
                 <c r=\"B1\"><f>CONCAT(\"汉语\", \"A\")</f></c>\
                 </row></sheetData></worksheet>"
                    .as_bytes(),
            )
            .unwrap();

            zip.finish().unwrap();
        }

        let (out_bytes, _) = OfficeConverter::convert_bytes(
            input_cursor.get_ref(),
            "xlsx",
            &fake_convert,
            "s2t",
            true,
            true,
        )
        .expect("convert_bytes failed");

        let cursor = Cursor::new(out_bytes);
        let mut zip = ZipArchive::new(cursor).expect("Output is not a valid ZIP archive");
        let mut sheet = zip
            .by_name("xl/worksheets/sheet1.xml")
            .expect("Converted xlsx is missing xl/worksheets/sheet1.xml");
        let mut content = String::new();
        sheet.read_to_string(&mut content).unwrap();

        assert!(content.contains("漢語"));
        assert!(content.contains(r#"<f>CONCAT("汉语", "A")</f>"#));
    }
}
