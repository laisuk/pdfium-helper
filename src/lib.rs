mod pdfium_loader;
mod pdfium_text;
mod reflow_helper;
mod punct_sets;
mod utils;
mod cjk_text;

pub use pdfium_loader::{PdfiumLibrary, PdfiumLoadError};
pub use pdfium_text::{
    extract_pdf_pages_with_callback_pdfium,
    extract_pdf_text_pdfium,
    PdfiumExtractError,
    print_error,
};
pub use reflow_helper::reflow_cjk_paragraphs;
pub use reflow_helper::reflow_cjk_paragraphs_with_heading_regex;
// use punct_sets::*;
// use cjk_text::*;
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
