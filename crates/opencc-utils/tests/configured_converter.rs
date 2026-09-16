use opencc_utils::{convert_office_document, handle_pdf_with_converter, PdfOptions};
use std::cell::RefCell;
use std::path::PathBuf;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

#[test]
fn office_filename_and_content_share_configured_converter() {
    let dir = std::env::temp_dir().join(format!("configured-office-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let input = dir.join("source.docx");
    std::fs::copy(root().join("tools/opencc-rs/OneDay.docx"), &input).unwrap();
    let seen = RefCell::new(Vec::new());
    let target = "configured";
    let output = convert_office_document(input.to_str().unwrap(), None, None, true, true, |text| {
        seen.borrow_mut().push(text.to_owned());
        text.replace("source", target)
    })
    .unwrap();
    assert_eq!(
        PathBuf::from(&output),
        dir.join("configured_converted.docx")
    );
    assert!(PathBuf::from(&output).is_file());
    let seen = seen.into_inner();
    assert_eq!(seen[0], "source");
    assert!(seen[1..].iter().any(|text| text.contains("w:document")));
    std::fs::remove_file(input).unwrap();
    std::fs::remove_file(output).unwrap();
    std::fs::remove_dir(dir).unwrap();
}

#[test]
fn pdf_converts_reflowed_text_with_stateful_closure() {
    let input = root().join("tests/samples/CHUNK_ABC.pdf");
    let input = input.to_str().unwrap();
    let output = std::env::temp_dir().join(format!("configured-pdf-{}.txt", std::process::id()));
    let output = output.to_str().unwrap().to_owned();
    let pdfium_dir = root().to_str().unwrap().to_owned();
    let options = || PdfOptions {
        input_file: input,
        output_file: Some(&output),
        reflow: true,
        compact: false,
        header: false,
        ignore_untrusted_pdf_text: false,
        pdfium_dir: Some(&pdfium_dir),
    };
    let extracted = std::fs::read_to_string(root().join("tests/samples/CHUNK_ABC_reflowed.txt"))
        .unwrap()
        .replace("\r\n", "\n")
        .replace('\r', "\n");
    assert!(!extracted.is_empty());
    let mut calls = 0;
    handle_pdf_with_converter(options(), |text| {
        calls += 1;
        assert_eq!(text.replace("\r\n", "\n").replace('\r', "\n"), extracted);
        format!("configured\n{text}")
    })
    .unwrap();
    assert_eq!(calls, 1);
    assert_eq!(
        std::fs::read_to_string(&output).unwrap(),
        format!("configured\n{extracted}")
    );
    std::fs::remove_file(output).unwrap();
}
