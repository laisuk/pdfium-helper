#![allow(non_camel_case_types)]

use crate::pdfium_loader::{PdfiumLibrary, PdfiumLoadError};
use std::sync::OnceLock;
use thiserror::Error;

type FPDF_DOCUMENT = *mut core::ffi::c_void;
type FPDF_PAGE = *mut core::ffi::c_void;
type FPDF_TEXTPAGE = *mut core::ffi::c_void;

#[cfg(target_os = "windows")]
// macro_rules! pdfium_fn {
//     (fn($($arg:ty),*) -> $ret:ty) => { extern "system" fn($($arg),*) -> $ret };
//     (fn($($arg:ty),*)) => { extern "system" fn($($arg),*) };
// }
macro_rules! pdfium_fn {
    (fn($($arg:ty),*) -> $ret:ty) => { extern "C" fn($($arg),*) -> $ret };
    (fn($($arg:ty),*)) => { extern "C" fn($($arg),*) };
}

#[cfg(not(target_os = "windows"))]
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

#[derive(Debug, Error)]
pub enum PdfiumExtractError {
    #[error(transparent)]
    Load(#[from] PdfiumLoadError),

    #[error("pdfium failed to load document: {0}")]
    LoadDocument(String),
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

    // Pdfium expects UTF-8 file path bytes (like your Python `encode("utf-8")`). :contentReference[oaicite:9]{index=9}
    let c_path = std::ffi::CString::new(path)
        .map_err(|_| PdfiumExtractError::LoadDocument(path.to_string()))?;

    let doc = (fns.load_document)(c_path.as_ptr(), std::ptr::null());
    if doc.is_null() {
        return Err(PdfiumExtractError::LoadDocument(path.to_string()));
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
