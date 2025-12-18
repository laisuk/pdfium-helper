use std::io::{self, Write};
use std::path::Path;

use pdfium_helper::{
    extract_pdf_pages_with_callback_pdfium,
    reflow_cjk_paragraphs,
};

fn main() -> anyhow::Result<()> {
    // input_file = "tests/My_Golden_Blood.pdf";
    let input_file = "tests/盗墓笔记.pdf";
    let output_file = "tests/盗墓笔记_extracted.txt";

    println!("Extracting PDF page-by-page with PDFium: {input_file}");

    // Load Pdfium native (dev + release friendly)
    let (pdfium, lib_path) = pdfium_helper::PdfiumLibrary::load_with_fallbacks()?;
    println!("Loaded pdfium: {}", lib_path.display());

    let mut pages: Vec<String> = Vec::new();

        // Page-by-page extraction with progress
    extract_pdf_pages_with_callback_pdfium(&pdfium, input_file, |page, total, text| {
        print_progress(page, total, text);
        pages.push(text.to_owned());
    })?;

    println!(); // move to next line after progress

    let full_text = pages.concat();
    println!(
        "Total extracted characters: {}",
        pdfium_helper::format_thousand(full_text.chars().count())
    );

    println!("Reflowing CJK paragraphs...");
    let reflowed = reflow_cjk_paragraphs(
        &full_text,
        false, // add_pdf_page_header
        false, // compact
    );

    println!("Writing reflowed text to: {output_file}");
    write_text_unix_newlines(output_file, &reflowed)?;

    println!("Done.");
    Ok(())
}

/// Write UTF-8 text using Unix newlines (`\n`) on all platforms
fn write_text_unix_newlines<P: AsRef<Path>>(path: P, s: &str) -> io::Result<()> {
    let normalized = s.replace("\r\n", "\n").replace('\r', "\n");
    std::fs::write(path, normalized.as_bytes())
}

fn print_progress(page: i32, total: i32, text: &str) {
    let percent = page * 100 / total.max(1);

    let msg = format!(
        "[{}/{}] ({:3}%) Extracted {} chars",
        page,
        total,
        percent,
        text.chars().count()
    );

    let mut line = msg;
    if line.len() < 80 {
        line.push_str(&" ".repeat(80 - line.len()));
    }

    print!("\r{}", line);
    let _ = io::stdout().flush();
}

