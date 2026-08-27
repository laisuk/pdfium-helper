use std::path::PathBuf;
use std::sync::Mutex;

use pdfium_helper::{extract_pdf_text_pdfium, PdfiumLibrary};

fn fixture_path(relative: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(relative)
}

fn load_pdfium() -> &'static PdfiumLibrary {
    let (pdfium, lib_path) = PdfiumLibrary::global_with_fallbacks()
        .unwrap_or_else(|e| panic!("failed to load global pdfium: {e}"));

    assert!(
        lib_path.exists(),
        "resolved pdfium path does not exist: {}",
        lib_path.display()
    );

    pdfium
}

fn extraction_lock() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: Mutex<()> = Mutex::new(());
    LOCK.lock().expect("extraction lock poisoned")
}

#[test]
fn ignore_untrusted_pdf_text_removes_repeated_overlay_watermark() {
    let _guard = extraction_lock();
    let pdfium = load_pdfium();
    let pdf_path = fixture_path("tests/samples/untrusted_sample.pdf");
    let pdf_path_str = pdf_path
        .to_str()
        .unwrap_or_else(|| panic!("non-utf8 test path: {}", pdf_path.display()));

    let normal = extract_pdf_text_pdfium(pdfium, pdf_path_str, false, false)
        .unwrap_or_else(|e| panic!("normal extraction failed for {}: {e}", pdf_path.display()));

    let filtered = extract_pdf_text_pdfium(pdfium, pdf_path_str, false, true)
        .unwrap_or_else(|e| panic!("filtered extraction failed for {}: {e}", pdf_path.display()));

    // The fixture intentionally contains a heavily repeated overlay watermark.
    assert!(
        normal.contains("萌主推剧"),
        "ordinary extraction should expose the repeated overlay watermark"
    );
    assert!(
        !filtered.contains("萌主推剧"),
        "untrusted-text filtering should remove the repeated overlay watermark"
    );

    // Legitimate body text must survive filtering.
    assert!(
        filtered.contains("一晚上烧掉千万韩元"),
        "filtered extraction unexpectedly removed legitimate page text"
    );
    assert!(
        filtered.contains("五千五百韩元"),
        "filtered extraction unexpectedly removed legitimate text from the last page"
    );
}

#[test]
fn ignore_untrusted_pdf_text_preserves_repeated_dot_punctuation() {
    let _guard = extraction_lock();
    let pdfium = load_pdfium();
    let pdf_path = fixture_path("tests/samples/untrusted_sample.pdf");
    let pdf_path_str = pdf_path
        .to_str()
        .unwrap_or_else(|| panic!("non-utf8 test path: {}", pdf_path.display()));

    let filtered = extract_pdf_text_pdfium(pdfium, pdf_path_str, false, true)
        .unwrap_or_else(|e| panic!("filtered extraction failed for {}: {e}", pdf_path.display()));

    // This is real body text, not overlay noise. The punctuation-only guard must
    // prevent repeated dots from being mistaken for untrusted repeated objects.
    assert!(
        filtered.contains("故...意...灌..."),
        "filtered extraction should preserve legitimate repeated-dot punctuation"
    );
}
