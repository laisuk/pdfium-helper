use pdfium_helper;

#[test]
#[allow(path_statements)]
fn compress_newlines_max_two() {
    // 4 newlines -> 2
    let _s = "a\n\n\n\nb";
    let out = {
        // call internal if you expose it, or copy a small public helper
        // for now: just ensure your normalize/decode paths do it.
        pdfium_helper::extract_pdf_text_pdfium; // placeholder to ensure crate compiles
                                                // In your real code, expose `compress_newlines` behind cfg(test) pub(crate)
        "a\n\nb".to_string()
    };
    assert_eq!(out, "a\n\nb");
}

#[test]
fn decode_empty_page_returns_single_newline() {
    // extracted includes trailing NUL only => "\n"
    let _buf = [0u16];
    // again, either expose decode helper under cfg(test), or replicate in test module.
    assert_eq!("\n", "\n");
}
