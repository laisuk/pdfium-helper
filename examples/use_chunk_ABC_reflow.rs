use std::fs;
use std::io;
use std::path::Path;

use pdfium_helper::reflow_cjk_paragraphs;

fn main() -> anyhow::Result<()> {
    let input_file = "tests/chunk_ABC.txt";
    let output_file = "tests/chunk_ABC_reflowed.txt";

    // 1) Read raw text from input file (UTF-8)
    println!("Reading text from: {input_file}");
    let text = match fs::read_to_string(input_file) {
        Ok(s) => s,
        Err(e) if e.kind() == io::ErrorKind::NotFound => {
            println!("Error: input file not found: {input_file}");
            return Ok(());
        }
        Err(e) => return Err(e.into()),
    };

    if text.trim().is_empty() {
        println!("Warning: input text is empty or whitespace only.");
    } else {
        println!("Loaded {} characters from input file.", text.chars().count());
    }

    // 2) Reflow CJK paragraphs
    // - add_pdf_page_header = false → skip fake page gaps without end punctuation
    // - compact = false → use blank line between paragraphs
    println!("Reflowing CJK paragraphs...");
    let reflowed = reflow_cjk_paragraphs(&text, false, false);

    // 3) Save result to disk (UTF-8, Unix newlines)
    println!("Writing reflowed text to: {output_file}");
    write_text_unix_newlines(output_file, &reflowed)?;

    println!("Done.");
    Ok(())
}

/// Write UTF-8 text using Unix newlines (`\n`) regardless of platform.
fn write_text_unix_newlines<P: AsRef<Path>>(path: P, s: &str) -> io::Result<()> {
    // Normalize CRLF/CR to LF, then write bytes.
    // (Your reflow already normalizes, but this guarantees the file format.)
    let normalized = s.replace("\r\n", "\n").replace('\r', "\n");
    fs::write(path, normalized.as_bytes())
}
