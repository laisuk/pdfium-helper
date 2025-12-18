mod pdfium_loader;
mod pdfium_text;
mod reflow_helper;
mod utils;

pub use pdfium_loader::{PdfiumLibrary, PdfiumLoadError,};
pub use pdfium_text::{
    extract_pdf_pages_with_callback_pdfium,
    extract_pdf_text_pdfium,
    PdfiumExtractError,
};
pub use reflow_helper::reflow_cjk_paragraphs;
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
