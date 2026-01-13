#![allow(non_camel_case_types)]

use crate::pdfium_loader::{PdfiumLibrary, PdfiumLoadError};
use std::sync::OnceLock;

type FPDF_DOCUMENT = *mut core::ffi::c_void;
type FPDF_PAGE = *mut core::ffi::c_void;
type FPDF_TEXTPAGE = *mut core::ffi::c_void;

macro_rules! pdfium_fn {
    (fn($($arg:ty),*) -> $ret:ty) => { extern "C" fn($($arg),*) -> $ret };
    (fn($($arg:ty),*)) => { extern "C" fn($($arg),*) };
}

// usage:
type FPDF_InitLibrary = pdfium_fn!(fn());
type FPDF_DestroyLibrary = pdfium_fn!(fn());

type FPDF_LoadDocument = pdfium_fn!(fn(*const i8, *const i8) -> FPDF_DOCUMENT);
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
                "Check the file path and permissions. Network drives (e.g. R:\\) may fail; try copying the PDF to a local disk.",
            Self::Password =>
                "Decrypt the PDF first or provide a password (if supported).",
            Self::Format =>
                "Try opening the PDF in a viewer; re-export or re-download if it fails.",
            _ =>
                "Run with --verbose and include PDF path + pdfium version when reporting.",
        }
    }
}

// #[allow(dead_code)]
pub fn print_error(e: &PdfiumExtractError) {
    match e {
        PdfiumExtractError::LoadDocument { path, error } => {
            eprintln!("Error: failed to open PDF");
            eprintln!("  Path   : {}", path);
            eprintln!("  PDFium : {} — {}", error, error.message());
            eprintln!("  Hint   : {}", error.hint());
        }
        other => {
            eprintln!("Error: {other}");
        }
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
    text_load_page: FPDFText_LoadPage,
    text_close_page: FPDFText_ClosePage,
    text_count_chars: FPDFText_CountChars,
    text_get_text: FPDFText_GetText,
    get_last_error: FPDF_GetLastError, // ✅ NEW
}

impl PdfiumFns {
    unsafe fn resolve(lib: &PdfiumLibrary) -> Result<Self, PdfiumLoadError> {
        Ok(Self {
            init: lib.get(b"FPDF_InitLibrary\0")?,
            destroy: lib.get(b"FPDF_DestroyLibrary\0")?,
            load_document: lib.get(b"FPDF_LoadDocument\0")?,
            close_document: lib.get(b"FPDF_CloseDocument\0")?,
            get_page_count: lib.get(b"FPDF_GetPageCount\0")?,
            load_page: lib.get(b"FPDF_LoadPage\0")?,
            close_page: lib.get(b"FPDF_ClosePage\0")?,
            text_load_page: lib.get(b"FPDFText_LoadPage\0")?,
            text_close_page: lib.get(b"FPDFText_ClosePage\0")?,
            text_count_chars: lib.get(b"FPDFText_CountChars\0")?,
            text_get_text: lib.get(b"FPDFText_GetText\0")?,
            get_last_error: lib.get(b"FPDF_GetLastError\0")?, // ✅
        })
    }
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

/// Matches `_normalize_page_text()` behavior. :contentReference[oaicite:8]{index=8}
fn normalize_page_text(mut s: String) -> String {
    if !s.is_empty() {
        s = s.replace("\r\n", "\n").replace('\r', "\n");
    }

    if s.trim().is_empty() {
        return "\n".to_string();
    }

    let trimmed = s.trim().to_string();
    format!("{trimmed}\n\n")
}

static PDFIUM_INIT_ONCE: OnceLock<()> = OnceLock::new();

/// Page-by-page extraction with callback, same contract as Python `extract_pdf_pages_with_callback_pdfium`.
pub fn extract_pdf_pages_with_callback_pdfium<F>(
    lib: &PdfiumLibrary,
    path: &str,
    mut callback: F,
) -> Result<(), PdfiumExtractError>
where
    F: FnMut(i32, i32, &str),
{
    let fns = unsafe { PdfiumFns::resolve(lib)? };

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
        callback(1, 1, "\n");
        return Ok(());
    }

    for i in 0..total {
        let page = (fns.load_page)(doc, i);
        if page.is_null() {
            callback(i + 1, total, "\n");
            continue;
        }

        let text_page = (fns.text_load_page)(page);
        if text_page.is_null() {
            (fns.close_page)(page);
            callback(i + 1, total, "\n");
            continue;
        }

        let count = (fns.text_count_chars)(text_page);

        let raw = if count > 0 {
            let mut buf = vec![0u16; (count as usize) + 1];
            let extracted = (fns.text_get_text)(text_page, 0, count, buf.as_mut_ptr());
            decode_pdfium_u16(&buf, extracted)
        } else {
            String::new()
        };

        (fns.text_close_page)(text_page);
        (fns.close_page)(page);

        let out = normalize_page_text(raw);
        callback(i + 1, total, &out);
    }

    Ok(())
}

/// Convenience: extract full text by concatenating callback outputs.
pub fn extract_pdf_text_pdfium(
    lib: &PdfiumLibrary,
    path: &str,
) -> Result<String, PdfiumExtractError> {
    let mut out = String::new();
    extract_pdf_pages_with_callback_pdfium(lib, path, |_, _, s| out.push_str(s))?;
    Ok(out)
}
