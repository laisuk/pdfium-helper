use std::path::PathBuf;
use std::sync::Mutex;

use pdfium_helper::{
    extract_pdf_pages_with_callback_pdfium, extract_pdf_text_pdfium, reflow_cjk_paragraphs,
    PdfiumLibrary,
};

fn manifest_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn fixture_path(relative: &str) -> PathBuf {
    manifest_dir().join(relative)
}

fn read_fixture(relative: &str) -> String {
    let path = fixture_path(relative);
    std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("failed to read fixture {}: {e}", path.display()))
}

fn normalize_newlines(s: &str) -> String {
    s.replace("\r\n", "\n").replace('\r', "\n")
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
fn global_pdfium_loader_returns_same_instance_and_path() {
    let (first_lib, first_path) = PdfiumLibrary::global_with_fallbacks()
        .unwrap_or_else(|e| panic!("failed to load first global pdfium: {e}"));
    let (second_lib, second_path) = PdfiumLibrary::global_with_fallbacks()
        .unwrap_or_else(|e| panic!("failed to load second global pdfium: {e}"));

    assert!(std::ptr::eq(first_lib, second_lib));
    assert_eq!(first_path, second_path);
}

#[test]
fn extract_chunk_abc_matches_current_golden_output() {
    let _guard = extraction_lock();
    let pdfium = load_pdfium();
    let pdf_path = fixture_path("tests/CHUNK_ABC.pdf");

    let extracted = extract_pdf_text_pdfium(
        pdfium,
        pdf_path
            .to_str()
            .unwrap_or_else(|| panic!("non-utf8 test path: {}", pdf_path.display())),
        false,
    )
    .unwrap_or_else(|e| panic!("failed to extract {}: {e}", pdf_path.display()));

    let actual = normalize_newlines(&extracted);
    let expected = normalize_newlines(&read_fixture("tests/CHUNK_ABC_extracted.txt"));

    assert_eq!(actual, expected);
    assert!(actual.contains("=== [Page 2/10] ==="));
    assert!(actual.contains("=== [Page 15/220] ==="));
    assert!(actual.contains("=== [Page 16/220] ==="));
    assert_eq!(actual.matches("=== [Page ").count(), 3);
}

#[test]
fn reflow_chunk_abc_matches_current_golden_output() {
    let input = read_fixture("tests/CHUNK_ABC_extracted.txt");
    let expected = normalize_newlines(&read_fixture("tests/CHUNK_ABC_reflowed.txt"));

    let actual = reflow_cjk_paragraphs(&input, false, false);

    assert_eq!(normalize_newlines(&actual), expected);
}

#[test]
fn extract_chunk_abc_callback_output_matches_full_text_api() {
    let _guard = extraction_lock();
    let pdfium = load_pdfium();
    let pdf_path = fixture_path("tests/CHUNK_ABC.pdf");
    let pdf_path_str = pdf_path
        .to_str()
        .unwrap_or_else(|| panic!("non-utf8 test path: {}", pdf_path.display()));

    let mut callback_pages = Vec::new();
    let mut callback_progress = Vec::new();
    extract_pdf_pages_with_callback_pdfium(pdfium, pdf_path_str, false, |page, total, text| {
        assert!(page >= 1);
        assert!(total >= page);
        callback_progress.push((page, total));
        callback_pages.push(text.to_owned());
    })
    .unwrap_or_else(|e| panic!("callback extraction failed for {}: {e}", pdf_path.display()));

    assert_eq!(callback_progress, vec![(1, 3), (2, 3), (3, 3)]);

    let via_callback = callback_pages.concat();
    let via_full_text = extract_pdf_text_pdfium(pdfium, pdf_path_str, false)
        .unwrap_or_else(|e| panic!("full extraction failed for {}: {e}", pdf_path.display()));

    assert_eq!(normalize_newlines(&via_callback), normalize_newlines(&via_full_text));
}

#[test]
fn repeated_extraction_via_global_loader_is_stable() {
    let _guard = extraction_lock();
    let pdfium = load_pdfium();
    let pdf_path = fixture_path("tests/CHUNK_ABC.pdf");
    let pdf_path_str = pdf_path
        .to_str()
        .unwrap_or_else(|| panic!("non-utf8 test path: {}", pdf_path.display()));
    let expected = normalize_newlines(&read_fixture("tests/CHUNK_ABC_extracted.txt"));

    for round in 0..3 {
        let extracted = extract_pdf_text_pdfium(pdfium, pdf_path_str, false)
            .unwrap_or_else(|e| panic!("repeated extraction round {round} failed: {e}"));
        assert_eq!(normalize_newlines(&extracted), expected);
    }
}

#[test]
fn reflow_yi_zuo_fei_matches_current_golden_text() {
    let input = read_fixture("tests/yi_zuo_fei.txt");
    let expected = normalize_newlines(&read_fixture("tests/yi_zuo_fei_reflowed.txt"));

    let actual = reflow_cjk_paragraphs(&input, false, false);

    assert_eq!(normalize_newlines(&actual), expected);
}

#[test]
fn extract_jiamianyouxi_smoke_test_produces_non_empty_pages() {
    let _guard = extraction_lock();
    let pdfium = load_pdfium();
    let pdf_path = fixture_path("tests/JiaMianYouXi.pdf");
    let pdf_path_str = pdf_path
        .to_str()
        .unwrap_or_else(|| panic!("non-utf8 test path: {}", pdf_path.display()));

    let mut page_count = 0usize;
    let mut non_empty_pages = 0usize;
    let mut total_chars = 0usize;

    extract_pdf_pages_with_callback_pdfium(pdfium, pdf_path_str, false, |page, total, text| {
        assert!(page >= 1);
        assert!(total >= page);
        page_count += 1;

        let trimmed = text.trim();
        if !trimmed.is_empty() {
            non_empty_pages += 1;
            total_chars += trimmed.chars().count();
        }
    })
    .unwrap_or_else(|e| panic!("failed to extract {}: {e}", pdf_path.display()));

    assert!(page_count > 0, "expected at least one page");
    assert!(non_empty_pages > 0, "expected at least one non-empty page");
    assert!(
        total_chars > 100,
        "expected substantial extracted text, got {total_chars} chars"
    );
}
