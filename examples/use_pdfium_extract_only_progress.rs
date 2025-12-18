
use pdfium_helper::{extract_pdf_pages_with_callback_pdfium, PdfiumLibrary};

fn main() -> anyhow::Result<()> {
    // input_file = "tests/My_Golden_Blood.pdf";
    let input_file = "tests/盗墓笔记.pdf";
    let output_file = "tests/盗墓笔记_extracted.txt";

    println!("Extracting PDF page-by-page with PDFium: {input_file}");

    // Locate bundled pdfium relative to executable
    // let exe_dir: PathBuf = std::env::current_exe()?
    //     .parent()
    //     .unwrap_or_else(|| Path::new("."))
    //     .to_path_buf();

    // let (pdfium, _lib_path) = PdfiumLibrary::load_from_bundled_dir(&exe_dir)?;
    let (pdfium, lib_path) = PdfiumLibrary::load_with_fallbacks()?;
    println!("Loaded pdfium: {}", lib_path.display());


    let mut pages: Vec<String> = Vec::new();

    extract_pdf_pages_with_callback_pdfium(&pdfium, input_file, |page, total, text| {
        // let percent = page * 100 / total.max(1);
        //
        // let msg = format!(
        //     "[{}/{}] ({:3}%) Extracted {} chars",
        //     page,
        //     total,
        //     percent,
        //     text.chars().count()
        // );
        //
        // // Pad so previous content is fully overwritten (Python: ljust(80))
        // let mut line = msg;
        // if line.len() < 80 {
        //     line.push_str(&" ".repeat(80 - line.len()));
        // }
        //
        // print!("\r{}", line);
        // let _ = io::stdout().flush();
        pdfium_helper::print_progress(page, total, text);

        pages.push(text.to_owned());
    })?;

    println!(); // move to next line after progress

    let full_text = pages.concat();
    println!(
        "Total extracted characters: {}",
         pdfium_helper::format_thousand(full_text.chars().count())
    );

    println!("Writing extracted text to: {output_file}");
    std::fs::write(output_file, full_text)?;

    println!("Done.");
    Ok(())
}
