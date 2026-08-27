#![allow(non_camel_case_types)]

use crate::pdfium_loader::{PdfiumLibrary, PdfiumLoadError};
use std::collections::HashMap;
use std::ffi::c_char;
use std::sync::{Mutex, OnceLock};
use unicode_general_category::{get_general_category, GeneralCategory};

type FPDF_DOCUMENT = *mut core::ffi::c_void;
type FPDF_PAGE = *mut core::ffi::c_void;
type FPDF_TEXTPAGE = *mut core::ffi::c_void;
// New: Get Page Object
type FPDF_PAGEOBJECT = *mut core::ffi::c_void;

macro_rules! pdfium_fn {
    (fn($($arg:ty),*) -> $ret:ty) => { extern "C" fn($($arg),*) -> $ret };
    (fn($($arg:ty),*)) => { extern "C" fn($($arg),*) };
}

// usage:
type FPDF_InitLibrary = pdfium_fn!(fn());
type FPDF_DestroyLibrary = pdfium_fn!(fn());

type FPDF_LoadDocument = pdfium_fn!(fn(*const c_char, *const c_char) -> FPDF_DOCUMENT);
type FPDF_CloseDocument = pdfium_fn!(fn(FPDF_DOCUMENT));

type FPDF_GetPageCount = pdfium_fn!(fn(FPDF_DOCUMENT) -> i32);
type FPDF_LoadPage = pdfium_fn!(fn(FPDF_DOCUMENT, i32) -> FPDF_PAGE);
type FPDF_ClosePage = pdfium_fn!(fn(FPDF_PAGE));

type FPDFText_LoadPage = pdfium_fn!(fn(FPDF_PAGE) -> FPDF_TEXTPAGE);
type FPDFText_ClosePage = pdfium_fn!(fn(FPDF_TEXTPAGE));
type FPDFText_CountChars = pdfium_fn!(fn(FPDF_TEXTPAGE) -> i32);
type FPDFText_GetText = pdfium_fn!(fn(FPDF_TEXTPAGE, i32, i32, *mut u16) -> i32);
// ✅ NEW:
type FPDF_GetLastError = pdfium_fn!(fn() -> u32);

// New:
type FPDFPage_CountObjects = pdfium_fn!(fn(FPDF_PAGE) -> i32);
type FPDFPage_GetObject = pdfium_fn!(fn(FPDF_PAGE, i32) -> FPDF_PAGEOBJECT);
type FPDFPageObj_GetType = pdfium_fn!(fn(FPDF_PAGEOBJECT) -> i32);
type FPDFPageObj_GetBounds =
    pdfium_fn!(fn(FPDF_PAGEOBJECT, *mut f32, *mut f32, *mut f32, *mut f32) -> i32);

type FPDFTextObj_GetText = pdfium_fn!(fn(FPDF_PAGEOBJECT, FPDF_TEXTPAGE, *mut u16, usize) -> usize);

#[derive(Debug, thiserror::Error)]
pub enum PdfiumExtractError {
    #[error(transparent)]
    Load(#[from] PdfiumLoadError),

    #[error("failed to open PDF")]
    LoadDocument {
        path: String,
        error: PdfiumLastError,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum PdfiumLastError {
    Success = 0,
    Unknown = 1,
    File = 2,
    Format = 3,
    Password = 4,
    Security = 5,
    Page = 6,
    Other = 0xFFFF_FFFF,
}

impl From<u32> for PdfiumLastError {
    fn from(v: u32) -> Self {
        match v {
            0 => Self::Success,
            1 => Self::Unknown,
            2 => Self::File,
            3 => Self::Format,
            4 => Self::Password,
            5 => Self::Security,
            6 => Self::Page,
            _ => Self::Other,
        }
    }
}

impl std::fmt::Display for PdfiumLastError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // short “name” only
        let name = match *self {
            PdfiumLastError::Success => "Success",
            PdfiumLastError::Unknown => "Unknown",
            PdfiumLastError::File => "File",
            PdfiumLastError::Format => "Format",
            PdfiumLastError::Password => "Password",
            PdfiumLastError::Security => "Security",
            PdfiumLastError::Page => "Page",
            PdfiumLastError::Other => "Other",
        };
        f.write_str(name)
    }
}

impl PdfiumLastError {
    pub fn message(self) -> &'static str {
        match self {
            Self::Success => "no error reported",
            Self::Unknown => "unknown error",
            Self::File => "cannot open file (missing / permission / IO error)",
            Self::Format => "invalid or corrupted PDF format",
            Self::Password => "PDF is password protected",
            Self::Security => "PDF security handler blocked access",
            Self::Page => "page processing error",
            Self::Other => "unrecognized PDFium error code",
        }
    }

    pub fn hint(self) -> &'static str {
        match self {
            Self::File =>
                "Check the file path and permissions. Network drives may fail; try copying the PDF to a local disk.",
            Self::Password =>
                "Decrypt the PDF first or provide a password (if supported).",
            Self::Format =>
                "Try opening the PDF in a viewer; re-export or re-download if it fails.",
            _ =>
                "Run with --verbose and include PDF path + pdfium version when reporting.",
        }
    }
}

struct PrettyPdfiumExtractError<'a>(&'a PdfiumExtractError);

impl std::fmt::Display for PrettyPdfiumExtractError<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.0 {
            PdfiumExtractError::LoadDocument { path, error } => {
                write!(
                    f,
                    "Error: failed to open PDF\n  Path   : {}\n  PDFium : {} - {}\n  Hint   : {}",
                    path,
                    error,
                    error.message(),
                    error.hint()
                )
            }
            other => write!(f, "Error: {other}"),
        }
    }
}

impl PdfiumExtractError {
    pub fn pretty(&self) -> impl std::fmt::Display + '_ {
        PrettyPdfiumExtractError(self)
    }
}

#[derive(Clone, Copy)]
struct PdfiumFns {
    init: FPDF_InitLibrary,
    #[allow(dead_code)]
    destroy: FPDF_DestroyLibrary,

    load_document: FPDF_LoadDocument,
    close_document: FPDF_CloseDocument,

    get_page_count: FPDF_GetPageCount,
    load_page: FPDF_LoadPage,
    close_page: FPDF_ClosePage,

    page_count_objects: FPDFPage_CountObjects,
    page_get_object: FPDFPage_GetObject,
    page_obj_get_type: FPDFPageObj_GetType,
    page_obj_get_bounds: FPDFPageObj_GetBounds,

    text_load_page: FPDFText_LoadPage,
    text_close_page: FPDFText_ClosePage,
    text_count_chars: FPDFText_CountChars,
    text_get_text: FPDFText_GetText,
    text_obj_get_text: FPDFTextObj_GetText,

    get_last_error: FPDF_GetLastError, // ✅ NEW
}

fn resolved_fns(lib: &PdfiumLibrary) -> Result<PdfiumFns, PdfiumLoadError> {
    static PDFIUM_FNS_CACHE: OnceLock<Mutex<HashMap<usize, PdfiumFns>>> = OnceLock::new();

    let key = lib as *const PdfiumLibrary as usize;
    let cache = PDFIUM_FNS_CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    let mut guard = cache.lock().expect("pdfium function cache poisoned");

    if let Some(fns) = guard.get(&key).copied() {
        return Ok(fns);
    }

    let fns = unsafe {
        PdfiumFns {
            init: lib.get(b"FPDF_InitLibrary\0")?,
            destroy: lib.get(b"FPDF_DestroyLibrary\0")?,
            load_document: lib.get(b"FPDF_LoadDocument\0")?,
            close_document: lib.get(b"FPDF_CloseDocument\0")?,
            get_page_count: lib.get(b"FPDF_GetPageCount\0")?,
            load_page: lib.get(b"FPDF_LoadPage\0")?,
            close_page: lib.get(b"FPDF_ClosePage\0")?,

            page_count_objects: lib.get(b"FPDFPage_CountObjects\0")?,
            page_get_object: lib.get(b"FPDFPage_GetObject\0")?,
            page_obj_get_type: lib.get(b"FPDFPageObj_GetType\0")?,
            page_obj_get_bounds: lib.get(b"FPDFPageObj_GetBounds\0")?,

            text_load_page: lib.get(b"FPDFText_LoadPage\0")?,
            text_close_page: lib.get(b"FPDFText_ClosePage\0")?,
            text_count_chars: lib.get(b"FPDFText_CountChars\0")?,
            text_get_text: lib.get(b"FPDFText_GetText\0")?,
            text_obj_get_text: lib.get(b"FPDFTextObj_GetText\0")?,

            get_last_error: lib.get(b"FPDF_GetLastError\0")?,
        }
    };
    guard.insert(key, fns);
    Ok(fns)
}

/// Compress multiple '\n' to max 2 (matches Python `_compress_newlines`). :contentReference[oaicite:6]{index=6}
fn compress_newlines(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut seen = 0usize;
    for ch in s.chars() {
        if ch == '\n' {
            seen += 1;
            if seen <= 2 {
                out.push('\n');
            }
        } else {
            seen = 0;
            out.push(ch);
        }
    }
    out
}

/// Mirrors `_decode_pdfium_buffer()` behavior. :contentReference[oaicite:7]{index=7}
fn decode_pdfium_u16(buf: &[u16], extracted: i32) -> String {
    if extracted <= 0 {
        return String::new();
    }

    let mut len = extracted as usize;

    // strip trailing NUL if present
    if len > 0 && buf.get(len - 1) == Some(&0u16) {
        len -= 1;
    }

    // empty page => "\n" (blank paragraph marker)
    if len == 0 {
        return "\n".to_string();
    }

    let mut text = String::from_utf16_lossy(&buf[..len]);

    // normalize CRLF/CR to LF
    text = text.replace("\r\n", "\n").replace('\r', "\n");

    // compress 3+ newlines down to 2
    compress_newlines(&text)
}

// Text Object Start

fn get_text_from_text_object(
    fns: &PdfiumFns,
    text_obj: FPDF_PAGEOBJECT,
    text_page: FPDF_TEXTPAGE,
) -> String {
    let required_bytes = (fns.text_obj_get_text)(text_obj, text_page, std::ptr::null_mut(), 0);

    if required_bytes == 0 {
        return String::new();
    }

    let u16_count = (required_bytes + 1) / 2;

    if u16_count <= 1 || u16_count > 10_000_000 {
        return String::new();
    }

    let mut buf = vec![0u16; u16_count];

    let written_bytes =
        (fns.text_obj_get_text)(text_obj, text_page, buf.as_mut_ptr(), required_bytes);

    if written_bytes == 0 {
        return String::new();
    }

    let mut len = written_bytes / 2;

    if len == 0 {
        return String::new();
    }

    if buf[len - 1] == 0 {
        len -= 1;
    }

    if len == 0 {
        return String::new();
    }

    String::from_utf16_lossy(&buf[..len])
}

fn bucket_y(y_mid: f32) -> i32 {
    const Y_BAND_STEP: f32 = 5.0;
    (y_mid / Y_BAND_STEP).floor() as i32
}

fn normalize_whitespace(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut last_was_ws = false;

    for ch in s.chars() {
        if ch.is_whitespace() {
            if !last_was_ws {
                out.push(' ');
                last_was_ws = true;
            }
        } else {
            out.push(ch);
            last_was_ws = false;
        }
    }

    out.trim().to_string()
}

fn is_untrusted_overlay(norm: &str, repeat_count: usize) -> bool {
    if is_punctuation_or_symbol_only(norm) {
        return false;
    }
    repeat_count > 4 && norm.chars().count() >= 3 && norm.chars().count() <= 200
}

struct TextObjItem {
    raw: String,
    norm: String,
    y_bucket: i32,
}

fn extract_page_text_ignore_untrusted(
    fns: &PdfiumFns,
    page: FPDF_PAGE,
    text_page: FPDF_TEXTPAGE,
) -> String {
    const FPDF_PAGEOBJ_TEXT: i32 = 1;

    let obj_count = (fns.page_count_objects)(page);

    if obj_count <= 0 {
        return String::new();
    }

    let mut items = Vec::with_capacity((obj_count as usize).min(2048));

    for i in 0..obj_count {
        let obj = (fns.page_get_object)(page, i);

        if obj.is_null() {
            continue;
        }

        if (fns.page_obj_get_type)(obj) != FPDF_PAGEOBJ_TEXT {
            continue;
        }

        let raw = get_text_from_text_object(fns, obj, text_page);

        if raw.trim().is_empty() {
            continue;
        }

        let mut left = 0.0f32;
        let mut bottom = 0.0f32;
        let mut right = 0.0f32;
        let mut top = 0.0f32;

        let y_bucket =
            if (fns.page_obj_get_bounds)(obj, &mut left, &mut bottom, &mut right, &mut top) != 0 {
                let y_mid = (bottom + top) * 0.5;
                bucket_y(y_mid)
            } else {
                0
            };

        let norm = normalize_whitespace(&raw);

        if norm.is_empty() {
            continue;
        }

        items.push(TextObjItem {
            raw,
            norm,
            y_bucket,
        });
    }

    if items.is_empty() {
        return String::new();
    }

    let mut freq: HashMap<(String, i32), usize> = HashMap::with_capacity(items.len());

    for item in &items {
        *freq.entry((item.norm.clone(), item.y_bucket)).or_insert(0) += 1;
    }

    const LINE_BUCKET_GAP: i32 = 3;

    let mut out = String::new();
    let mut last_bucket: Option<i32> = None;

    for item in items {
        let repeats = freq
            .get(&(item.norm.clone(), item.y_bucket))
            .copied()
            .unwrap_or(0);

        if is_untrusted_overlay(&item.norm, repeats) {
            continue;
        }

        if let Some(last) = last_bucket {
            if (item.y_bucket - last).abs() >= LINE_BUCKET_GAP {
                out.push('\n');
            }
        }

        out.push_str(&item.raw);
        last_bucket = Some(item.y_bucket);
    }

    out
}

fn is_punctuation_or_symbol_only(s: &str) -> bool {
    let mut saw_char = false;

    for ch in s.chars() {
        if ch.is_whitespace() {
            continue;
        }

        saw_char = true;

        if !matches!(
            get_general_category(ch),
            GeneralCategory::ConnectorPunctuation
                | GeneralCategory::DashPunctuation
                | GeneralCategory::OpenPunctuation
                | GeneralCategory::ClosePunctuation
                | GeneralCategory::InitialPunctuation
                | GeneralCategory::FinalPunctuation
                | GeneralCategory::OtherPunctuation
                | GeneralCategory::MathSymbol
                | GeneralCategory::CurrencySymbol
                | GeneralCategory::ModifierSymbol
                | GeneralCategory::OtherSymbol
        ) {
            return false;
        }
    }

    saw_char
}

// text Object End

/// Matches `_normalize_page_text()` behavior. :contentReference[oaicite:8]{index=8}
fn normalize_page_text(s: String) -> String {
    if s.trim().is_empty() {
        return "\n".to_string();
    }

    let trimmed = s.trim_end();
    let mut out = String::with_capacity(trimmed.len() + 2);
    out.push_str(trimmed);
    out.push('\n');
    out.push('\n');
    out
}

static PDFIUM_INIT_ONCE: OnceLock<()> = OnceLock::new();

/// Page-by-page extraction with callback, same contract as Python `extract_pdf_pages_with_callback_pdfium`.
pub fn extract_pdf_pages_with_callback_pdfium<F>(
    lib: &PdfiumLibrary,
    path: &str,
    add_page_header: bool,           // ✅ new
    ignore_untrusted_pdf_text: bool, // ✅ new
    mut callback: F,
) -> Result<(), PdfiumExtractError>
where
    F: FnMut(i32, i32, &str),
{
    let fns = resolved_fns(lib)?;

    // init once per process (safer than calling init/destroy per call if you multi-call in CLI).
    PDFIUM_INIT_ONCE.get_or_init(|| (fns.init)());

    // PDFium expects a C string path (NUL-terminated). `CString::new` fails if the path contains an interior '\0'.
    let c_path =
        std::ffi::CString::new(path.as_bytes()).map_err(|_| PdfiumExtractError::LoadDocument {
            path: path.to_string(),
            error: PdfiumLastError::Unknown,
        })?;

    let doc = (fns.load_document)(c_path.as_ptr(), std::ptr::null());
    if doc.is_null() {
        let code = (fns.get_last_error)(); // FPDF_GetLastError
        return Err(PdfiumExtractError::LoadDocument {
            path: path.to_string(),
            error: PdfiumLastError::from(code),
        });
    }

    // Ensure doc closed
    struct DocGuard {
        doc: FPDF_DOCUMENT,
        close: FPDF_CloseDocument,
    }
    impl Drop for DocGuard {
        fn drop(&mut self) {
            (self.close)(self.doc)
        }
    }
    let _doc_guard = DocGuard {
        doc,
        close: fns.close_document,
    };

    let total = (fns.get_page_count)(doc);
    if total <= 0 {
        // keep behavior consistent
        callback(1, 1, "\n");
        return Ok(());
    }

    for i in 0..total {
        let page_no = i + 1;

        let page = (fns.load_page)(doc, i);
        if page.is_null() {
            callback(page_no, total, "\n");
            continue;
        }

        let text_page = (fns.text_load_page)(page);
        if text_page.is_null() {
            (fns.close_page)(page);
            callback(page_no, total, "\n");
            continue;
        }

        let raw = if ignore_untrusted_pdf_text {
            extract_page_text_ignore_untrusted(&fns, page, text_page)
        } else {
            let count = (fns.text_count_chars)(text_page);

            if count > 0 {
                let mut buf = vec![0u16; (count as usize) + 1];
                let extracted = (fns.text_get_text)(text_page, 0, count, buf.as_mut_ptr());

                decode_pdfium_u16(&buf, extracted)
            } else {
                String::new()
            }
        };

        (fns.text_close_page)(text_page);
        (fns.close_page)(page);

        let out = normalize_page_text(raw);

        if add_page_header {
            // ✅ match C# format exactly
            let header = format!("=== [Page {page_no}/{total}] ===\n");
            let with_header = header + &out;
            callback(page_no, total, &with_header);
        } else {
            callback(page_no, total, &out);
        }
    }

    Ok(())
}

/// Convenience: extract full text by concatenating callback outputs.
pub fn extract_pdf_text_pdfium(
    lib: &PdfiumLibrary,
    path: &str,
    add_page_header: bool,
    ignore_untrusted_pdf_text: bool,
) -> Result<String, PdfiumExtractError> {
    let mut out = String::new();

    extract_pdf_pages_with_callback_pdfium(
        lib,
        path,
        add_page_header,
        ignore_untrusted_pdf_text,
        |_, _, s| out.push_str(s),
    )?;

    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::{is_punctuation_or_symbol_only, is_untrusted_overlay, normalize_page_text};

    #[test]
    fn normalize_page_text_preserves_leading_indentation() {
        let input = "\u{3000}\u{3000}Indented first line\nSecond line\n".to_string();
        let out = normalize_page_text(input);
        assert_eq!(out, "\u{3000}\u{3000}Indented first line\nSecond line\n\n");
    }

    #[test]
    fn normalize_page_text_keeps_blank_page_marker() {
        let out = normalize_page_text("   \n\n\t".to_string());
        assert_eq!(out, "\n");
    }

    #[test]
    fn repeated_punctuation_is_not_untrusted_overlay() {
        assert!(!is_untrusted_overlay("...", 4));
        assert!(!is_untrusted_overlay("......", 6));
        assert!(!is_untrusted_overlay("……", 10));
        assert!(!is_untrusted_overlay("！！！", 10));
        assert!(!is_untrusted_overlay("？？？", 10));
    }

    #[test]
    fn repeated_punctuation_and_symbols_are_not_untrusted_overlay() {
        // ASCII punctuation.
        assert!(!is_untrusted_overlay("...", 4));
        assert!(!is_untrusted_overlay("......", 6));
        assert!(!is_untrusted_overlay("------", 10));

        // CJK / Unicode punctuation.
        assert!(!is_untrusted_overlay("……", 10));
        assert!(!is_untrusted_overlay("！！！", 10));
        assert!(!is_untrusted_overlay("？？？", 10));
        assert!(!is_untrusted_overlay("。。。。。。", 10));

        // Box drawing.
        assert!(!is_untrusted_overlay("────────", 10));
        assert!(!is_untrusted_overlay("════════", 10));
        assert!(!is_untrusted_overlay("┼┼┼┼┼┼", 10));

        // Other symbols.
        assert!(!is_untrusted_overlay("★★★★★★", 10));
        assert!(!is_untrusted_overlay("■■■■■■", 10));
        assert!(!is_untrusted_overlay("→→→→→→", 10));

        // Actual repeated text must still be rejected.
        assert!(is_untrusted_overlay("萌主推剧", 6));
    }

    #[test]
    fn punctuation_or_symbol_only_requires_no_text() {
        assert!(is_punctuation_or_symbol_only("......"));
        assert!(is_punctuation_or_symbol_only("……！？"));
        assert!(is_punctuation_or_symbol_only("─┼─┼─"));
        assert!(is_punctuation_or_symbol_only("★ → ■"));

        assert!(!is_punctuation_or_symbol_only(""));
        assert!(!is_punctuation_or_symbol_only("   "));
        assert!(!is_punctuation_or_symbol_only("萌主推剧"));
        assert!(!is_punctuation_or_symbol_only("萌主......"));
    }
}
