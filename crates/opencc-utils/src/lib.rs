use encoding_rs::Encoding;
use encoding_rs_io::DecodeReaderBytesBuilder;
use opencc_office_converter::OfficeConverter;
use pdfium_helper::{
    detect_platform_folder, extract_pdf_pages_with_callback_pdfium, reflow_cjk_paragraphs,
    PdfiumLibrary,
};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::{self, BufReader, BufWriter, Read, Write};
use std::path::{Path, PathBuf};

pub use pdfium_helper::PdfiumExtractError;

pub fn exit_on_error(result: Result<(), Box<dyn std::error::Error>>) {
    if let Err(e) = result {
        if let Some(pe) = e.downcast_ref::<PdfiumExtractError>() {
            eprintln!("{}", pe.pretty());
        } else {
            eprintln!("Error: {e}");
        }
        std::process::exit(1);
    }
}

pub fn decode_input(buffer: &[u8], enc: &str) -> io::Result<String> {
    if enc.eq_ignore_ascii_case("UTF-8") {
        return Ok(String::from_utf8_lossy(buffer).into_owned());
    }

    let encoding = Encoding::for_label(enc.as_bytes()).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("Unsupported encoding: {enc}"),
        )
    })?;

    let mut reader = DecodeReaderBytesBuilder::new()
        .encoding(Some(encoding))
        .build(buffer);

    let mut decoded = String::new();
    reader.read_to_string(&mut decoded)?;
    Ok(decoded)
}

pub fn encode_and_write_output(
    output_str: &str,
    enc: &str,
    output: &mut dyn Write,
) -> io::Result<()> {
    if enc.eq_ignore_ascii_case("UTF-8") {
        output.write_all(output_str.as_bytes())?;
        return Ok(());
    }

    let encoding = Encoding::for_label(enc.as_bytes()).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("Unsupported output encoding: {enc}"),
        )
    })?;

    let (encoded, _, _) = encoding.encode(output_str);
    output.write_all(&encoded)?;
    Ok(())
}

pub fn open_output(output_file: Option<&String>) -> io::Result<(bool, Box<dyn Write>)> {
    let is_console_output = output_file.is_none();

    let output: Box<dyn Write> = match output_file {
        Some(file_name) => Box::new(BufWriter::new(File::create(file_name)?)),
        None => Box::new(BufWriter::new(io::stdout().lock())),
    };

    Ok((is_console_output, output))
}

pub fn should_remove_bom(in_enc: &str, out_enc: &str) -> bool {
    in_enc.eq_ignore_ascii_case("UTF-8") && !out_enc.eq_ignore_ascii_case("UTF-8")
}

pub fn remove_utf8_bom(input: &mut Vec<u8>) {
    if input.starts_with(&[0xEF, 0xBB, 0xBF]) {
        input.drain(..3);
    }
}

pub fn normalize_line_endings(s: &str) -> String {
    if !s.contains('\r') {
        return s.to_string();
    }

    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();

    while let Some(c) = chars.next() {
        if c == '\r' {
            if matches!(chars.peek(), Some('\n')) {
                chars.next();
            }
            out.push('\n');
        } else {
            out.push(c);
        }
    }

    out
}

pub fn write_text_unix_newlines<P: AsRef<Path>>(path: P, s: &str) -> io::Result<()> {
    let normalized = s.replace("\r\n", "\n").replace('\r', "\n");
    std::fs::write(path, normalized.as_bytes())
}

pub fn convert_office_document<F>(
    input_file: &str,
    output_file: Option<&String>,
    format: Option<&str>,
    keep_font: bool,
    convert_filename: bool,
    config: &str,
    punctuation: bool,
    convert_text: F,
) -> Result<String, Box<dyn std::error::Error>>
where
    F: Fn(&str, &str, bool) -> String,
{
    validate_input_file(input_file)?;

    let office_extensions: HashSet<&'static str> =
        ["docx", "xlsx", "pptx", "odt", "ods", "odp", "epub"].into();

    let office_format = if let Some(f) = format {
        f.to_lowercase()
    } else {
        let ext = Path::new(input_file)
            .extension()
            .and_then(|e| e.to_str())
            .ok_or("❌  Cannot infer file extension. Please provide --format.")?
            .to_lowercase();

        if office_extensions.contains(ext.as_str()) {
            ext
        } else {
            return Err(format!(
                "❌  Unsupported Office extension: .{ext}. Please provide --format."
            )
            .into());
        }
    };

    let final_output = match output_file {
        Some(path) => {
            let output_path = Path::new(path);

            if output_path.extension().is_none() {
                format!("{path}.{}", office_format)
            } else {
                path.clone()
            }
        }
        None => {
            let input_path = Path::new(input_file);
            let file_stem = input_path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("converted");

            let parent = input_path.parent().unwrap_or_else(|| ".".as_ref());
            let final_stem = if convert_filename {
                let file_stem_converted = convert_text(file_stem, config, punctuation);
                format!("{file_stem_converted}_converted")
            } else {
                format!("{file_stem}_converted")
            };

            parent
                .join(format!("{final_stem}.{office_format}"))
                .to_string_lossy()
                .to_string()
        }
    };

    match OfficeConverter::convert(
        input_file,
        &final_output,
        &office_format,
        &convert_text,
        config,
        punctuation,
        keep_font,
    ) {
        Ok(result) if result.success => {
            eprintln!("{}\n📁  Output saved to: {}", result.message, final_output);
        }
        Ok(result) => {
            eprintln!("❌  Office document conversion failed: {}", result.message);
        }
        Err(e) => {
            eprintln!("❌  Error: {}", e);
        }
    }

    Ok(final_output)
}

pub struct PdfOptions<'a> {
    pub input_file: &'a str,
    pub output_file: Option<&'a String>,
    pub config: Option<&'a str>,
    pub punctuation: bool,
    pub reflow: bool,
    pub compact: bool,
    pub header: bool,
    pub extract_only: bool,
    pub pdfium_dir: Option<&'a String>,
    pub converter_name: &'a str,
}

pub fn handle_pdf_with_converter<F>(
    options: PdfOptions<'_>,
    mut convert_text: F,
) -> Result<(), Box<dyn std::error::Error>>
where
    F: FnMut(&str, &str, bool) -> String,
{
    let input_norm = normalize_input_path(options.input_file);
    validate_input_file(&input_norm)?;
    let input_path = Path::new(&input_norm);
    let final_output = default_pdf_output(input_path, options.output_file, options.extract_only);

    if options.extract_only {
        println!("Extracting PDF page-by-page with PDFium (extract-only): {input_norm}");
    } else {
        println!("Extracting PDF page-by-page with PDFium: {input_norm}");
    }

    let pdfium = load_pdfium(options.pdfium_dir)?;
    let mut pages: Vec<String> = Vec::new();

    extract_pdf_pages_with_callback_pdfium(
        &pdfium,
        &input_norm,
        options.header,
        |page, total, text| {
            pdfium_helper::print_progress(page, total, text);
            pages.push(text.to_owned());
        },
    )?;

    pdfium_helper::print_done(pages.len() as i32);

    let mut extracted = pages.concat();

    println!(
        "Total extracted characters: {}",
        pdfium_helper::format_thousand(extracted.chars().count())
    );

    if options.reflow {
        println!("Reflowing CJK paragraphs...");
        extracted = reflow_cjk_paragraphs(&extracted, options.header, options.compact);
    }

    if options.extract_only {
        write_text_unix_newlines(&final_output, &extracted)?;
        eprintln!(
            "✅  PDF extracted.\n📁  Output saved to: {}",
            final_output.display()
        );
        return Ok(());
    }

    let config = options
        .config
        .ok_or("❌  --config is required unless --extract is used")?;

    println!(
        "Converting with {} (config: {}, punct: {}) ...",
        options.converter_name, config, options.punctuation
    );

    let converted = convert_text(&extracted, config, options.punctuation);
    write_text_unix_newlines(&final_output, &converted)?;

    eprintln!(
        "✅  PDF converted.\n📁  Output saved to: {}",
        final_output.display()
    );
    Ok(())
}

fn normalize_input_path(input_file: &str) -> String {
    if cfg!(windows) {
        input_file.replace(['/', '\\'], &std::path::MAIN_SEPARATOR.to_string())
    } else {
        input_file.to_owned()
    }
}

fn default_pdf_output(
    input_path: &Path,
    output_file: Option<&String>,
    extract_only: bool,
) -> PathBuf {
    match output_file {
        Some(p) => PathBuf::from(p),
        None => {
            let parent = input_path.parent().unwrap_or_else(|| Path::new("."));
            let stem = input_path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("input");

            let suffix = if extract_only {
                "_extracted.txt"
            } else {
                "_converted.txt"
            };
            parent.join(format!("{stem}{suffix}"))
        }
    }
}

fn load_pdfium(pdfium_dir: Option<&String>) -> Result<PdfiumLibrary, Box<dyn std::error::Error>> {
    if let Some(dir) = pdfium_dir {
        let base = Path::new(dir);

        match PdfiumLibrary::load_from_base_dir_flexible(base) {
            Ok((pdfium, lib_path)) => {
                print_loaded_pdfium(&lib_path, false);
                Ok(pdfium)
            }
            Err(e) => {
                eprintln!(
                    "Warning: failed to load Pdfium from {}: {}",
                    base.display(),
                    e
                );

                let (pdfium, lib_path) = PdfiumLibrary::load_with_fallbacks()?;
                print_loaded_pdfium(&lib_path, true);
                Ok(pdfium)
            }
        }
    } else {
        let (pdfium, lib_path) = PdfiumLibrary::load_with_fallbacks()?;
        print_loaded_pdfium(&lib_path, true);
        Ok(pdfium)
    }
}

fn print_loaded_pdfium(path: &Path, include_version: bool) {
    let display_path = path.display().to_string().replace('\\', "/");
    if include_version {
        match read_pdfium_version(path) {
            Some(version) => println!("Loaded pdfium: {} (version: {})", display_path, version),
            None => println!("Loaded pdfium: {}", display_path),
        }
    } else {
        println!("Loaded pdfium: {}", display_path);
    }
}

fn read_pdfium_version(lib_path: &Path) -> Option<String> {
    let version_path = find_pdfium_version_file(lib_path)?;
    let manifest_dir = version_path.parent()?;
    let relative_path = lib_path
        .strip_prefix(manifest_dir)
        .ok()?
        .display()
        .to_string()
        .replace('\\', "/");

    let (version, hashes) = read_pdfium_manifest(&version_path)?;
    let expected_hash = manifest_hash_candidates(&relative_path, lib_path)
        .into_iter()
        .find_map(|candidate| hashes.get(&candidate).cloned())?;
    let actual_hash = compute_sha256_hex(lib_path)?;
    if !actual_hash.eq_ignore_ascii_case(&expected_hash) {
        return None;
    }

    Some(version)
}

fn read_pdfium_manifest(version_path: &Path) -> Option<(String, HashMap<String, String>)> {
    let contents = std::fs::read_to_string(version_path).ok()?;
    let mut version = None;
    let mut hashes = HashMap::new();

    for line in contents.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        let (key, value) = line.split_once('=')?;
        let key = key.trim();
        let value = value.trim();
        if key.is_empty() || value.is_empty() {
            continue;
        }

        if key == "version" {
            version = Some(value.to_owned());
            continue;
        }

        let hash = value.strip_prefix("SHA256:")?.trim();
        if hash.is_empty() {
            continue;
        }

        hashes.insert(key.replace('\\', "/"), hash.to_owned());
    }

    Some((version?, hashes))
}

fn manifest_hash_candidates(lib_relative_path: &str, lib_path: &Path) -> Vec<String> {
    let mut candidates = Vec::new();
    push_unique_candidate(&mut candidates, lib_relative_path.to_owned());

    let file_name = lib_path
        .file_name()
        .and_then(|name| name.to_str())
        .map(|name| name.replace('\\', "/"));

    if let Some(file_name) = file_name {
        push_unique_candidate(&mut candidates, file_name.clone());

        if let Ok(platform) = detect_platform_folder() {
            push_unique_candidate(&mut candidates, format!("pdfium/{platform}/{file_name}"));
            push_unique_candidate(&mut candidates, format!("{platform}/{file_name}"));
        }
    }

    candidates
}

fn push_unique_candidate(candidates: &mut Vec<String>, candidate: String) {
    if !candidates.iter().any(|existing| existing == &candidate) {
        candidates.push(candidate);
    }
}

fn compute_sha256_hex(path: &Path) -> Option<String> {
    let mut file = File::open(path).ok()?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 8192];

    loop {
        let read = file.read(&mut buffer).ok()?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }

    Some(format!("{:X}", hasher.finalize()))
}

fn find_pdfium_version_file(lib_path: &Path) -> Option<PathBuf> {
    let ancestors: Vec<_> = lib_path.ancestors().collect();

    for ancestor in ancestors.iter().rev() {
        let candidate = ancestor.join("VERSION");
        if candidate.is_file() {
            return Some(candidate);
        }
    }

    for ancestor in ancestors.iter().rev() {
        let candidate = ancestor.join("pdfium").join("VERSION");
        if candidate.is_file() {
            return Some(candidate);
        }
    }

    None
}

pub fn open_input_file<P: AsRef<Path>>(path: P) -> io::Result<BufReader<File>> {
    let path = path.as_ref();
    validate_input_file(path)?;
    Ok(BufReader::new(File::open(path)?))
}

pub fn validate_input_file<P: AsRef<Path>>(path: P) -> io::Result<()> {
    let path = path.as_ref();

    let metadata = std::fs::metadata(path).map_err(|error| {
        if error.kind() == io::ErrorKind::NotFound {
            io::Error::new(
                io::ErrorKind::NotFound,
                format!("Input file not found: {}", path.display()),
            )
        } else {
            io::Error::new(
                error.kind(),
                format!("Cannot access input file {}: {error}", path.display()),
            )
        }
    })?;

    if !metadata.is_file() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("Input path is not a file: {}", path.display()),
        ));
    }

    Ok(())
}
