//! Pdfium native loading, PDF text extraction, and CJK-aware paragraph reflow.
//!
//! Start with [`PdfiumLibrary::load_with_fallbacks()`] to load Pdfium, then use
//! [`extract_pdf_text_pdfium()`] or [`extract_pdf_pages_with_callback_pdfium()`]
//! to extract text and [`reflow_cjk_paragraphs()`] to reflow it.
//!
//! # Features
//!
//! Default features are empty. Enable `pdfium-embed` to embed a precompressed,
//! platform-specific Pdfium library and extract it to a versioned cache file
//! when needed. Decompression uses a vendored pure Rust decoder adapted from
//! ruzstd 0.9.0; it requires no native Zstandard library or C compiler for
//! Zstandard decoding. Pdfium itself remains a dynamically loaded native library.
//!
//! The decoder is internal and compiled only with `pdfium-embed`. It decodes
//! one Zstandard frame, supports frames with or without a declared content size,
//! and limits the history window to 100 MiB. Frame checksums are consumed but
//! not validated, and Zstandard decoding dictionaries are unsupported.
//!
//! This crate is intended for internal Git-based use (`publish = false`).
//! Generate API documentation locally with
//! `cargo doc -p pdfium-helper --no-deps --features pdfium-embed`.

mod cjk_text;
mod pdfium_loader;
mod pdfium_text;
mod punct_sets;
mod reflow_helper;
mod utils;
#[cfg(feature = "pdfium-embed")]
mod zstd;

pub use pdfium_loader::{detect_platform_folder, PdfiumLibrary, PdfiumLoadError};
pub use pdfium_text::{
    extract_pdf_pages_with_callback_pdfium, extract_pdf_text_pdfium, PdfiumExtractError,
};
pub use reflow_helper::reflow_cjk_paragraphs;
pub use reflow_helper::reflow_cjk_paragraphs_with_heading_regex;
pub use utils::*;

// #[cfg(test)]
// mod tests {
//     use super::*;
//
//     #[test]
//     fn it_works() {
//         let result = add(2, 2);
//         assert_eq!(result, 4);
//     }
// }
