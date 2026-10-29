// Use the same private ZIP implementation as the library.
#[path = "../src/zip/mod.rs"]
mod zip;

use crate::zip::{write::SimpleFileOptions, ZipArchive, ZipWriter};
use opencc_office_converter::OfficeConverter;
use std::collections::hash_map::RandomState;
use std::fs;
use std::hash::BuildHasher;
use std::io::{Cursor, Read, Write};
use std::path::{Path, PathBuf};

struct TestDirectory(PathBuf);

impl TestDirectory {
    fn new() -> Self {
        loop {
            let path = std::env::temp_dir().join(format!(
                "opencc-cli-test-{:016x}",
                RandomState::new().hash_one(std::process::id())
            ));
            match fs::create_dir(&path) {
                Ok(()) => return Self(path),
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(e) => panic!("create test directory: {e}"),
            }
        }
    }
}

impl Drop for TestDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn convert(input: &Path, output: &Path) -> std::io::Result<()> {
    OfficeConverter::convert_path_stream(
        input.to_str().unwrap(),
        output.to_str().unwrap(),
        "docx",
        &|text: &str| text.replace("汉语", "漢語"),
        false,
    )
    .map(|_| ())
}

#[test]
fn public_path_api_output_lifecycle() {
    let dir = TestDirectory::new();
    let input = dir.0.join("input.docx");
    let output = dir.0.join("output.docx");
    let mut zip = ZipWriter::new(Cursor::new(Vec::new()));
    zip.start_file("word/document.xml", SimpleFileOptions::default())
        .unwrap();
    zip.write_all("<w:t>汉语</w:t>".as_bytes()).unwrap();
    let original = zip.finish().unwrap().into_inner();
    fs::write(&input, &original).unwrap();

    for existing in [false, true] {
        if existing {
            fs::write(&output, b"old destination").unwrap();
        }
        let result = convert(&input, &output);
        assert!(result.is_ok(), "{:?}", result);
        let mut archive = ZipArchive::new(fs::File::open(&output).unwrap()).unwrap();
        let mut text = String::new();
        archive
            .by_name("word/document.xml")
            .unwrap()
            .read_to_string(&mut text)
            .unwrap();
        assert_eq!(text, "<w:t>漢語</w:t>");
        assert_eq!(fs::read_dir(&dir.0).unwrap().count(), 2);
    }

    let result = convert(&input, &input);
    assert!(result.is_err());
    assert_eq!(fs::read(&input).unwrap(), original);

    fs::write(&input, b"invalid input ZIP").unwrap();
    fs::write(&output, b"original destination").unwrap();
    let result = convert(&input, &output);
    assert!(result.is_err());
    assert_eq!(fs::read(&output).unwrap(), b"original destination");
    assert_eq!(fs::read_dir(&dir.0).unwrap().count(), 2);
}
