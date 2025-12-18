mod office_converter;

use office_converter::OfficeConverter;

use clap::{Arg, ArgMatches, Command};
use encoding_rs::Encoding;
use encoding_rs_io::DecodeReaderBytesBuilder;
use opencc_fmmseg::OpenCC;
use pdfium_helper::{extract_pdf_pages_with_callback_pdfium, reflow_cjk_paragraphs, PdfiumLibrary};
use std::collections::HashSet;
use std::fs::File;
use std::io::{self, BufReader, BufWriter, IsTerminal, Read, Write};

const CONFIG_LIST: [&str; 16] = [
    "s2t", "t2s", "s2tw", "tw2s", "s2twp", "tw2sp", "s2hk", "hk2s", "t2tw", "t2twp", "t2hk",
    "tw2t", "tw2tp", "hk2t", "t2jp", "jp2t",
];

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let matches = Command::new("opencc-rs")
        .about("OpenCC Rust: Command Line Open Chinese Converter")
        .subcommand_required(true)
        .arg_required_else_help(true)
        .subcommand(
            Command::new("convert")
                .about("Convert plain text using OpenCC")
                .args(common_args())
                .arg(
                    Arg::new("in_enc")
                        .long("in-enc")
                        .default_value("UTF-8")
                        .help("Encoding for input"),
                )
                .arg(
                    Arg::new("out_enc")
                        .long("out-enc")
                        .default_value("UTF-8")
                        .help("Encoding for output"),
                ),
        )
        .subcommand(
            Command::new("office")
                .about("Convert Office or EPUB documents using OpenCC")
                .args(common_args())
                .arg(
                    Arg::new("format")
                        .short('f')
                        .long("format")
                        .value_name("ext")
                        .help("Force document format: docx, odt, epub..."),
                )
                .arg(
                    Arg::new("keep_font")
                        .long("keep-font")
                        .action(clap::ArgAction::SetTrue)
                        .help("Preserve original font styles"),
                )
                .arg(
                    Arg::new("auto_ext")
                        .long("auto-ext")
                        .action(clap::ArgAction::SetTrue)
                        .help("Infer format from file extension"),
                ),
        )
        .subcommand(
            Command::new("pdf")
                .about("Extract PDF text and convert using OpenCC")
                // reuse common args: -i/-o/-c/-p
                .args(common_args())
                // PDF input should not use stdin; enforce in handler
                .arg(
                    Arg::new("reflow")
                        .short('r')
                        .long("reflow")
                        .action(clap::ArgAction::SetTrue)
                        .help("Reflow extracted PDF lines into CJK paragraphs"),
                )
                .arg(
                    Arg::new("compact")
                        .long("compact")
                        .action(clap::ArgAction::SetTrue)
                        .help("Compact reflow output (remove extra blank lines/spaces)"),
                )
                .arg(
                    Arg::new("header")
                        .short('H')
                        .long("header")
                        .action(clap::ArgAction::SetTrue)
                        .help("Add PDF page headers like: === [Page 3/120] ==="),
                ),
        )
        .get_matches();

    match matches.subcommand() {
        Some(("convert", sub)) => handle_convert(sub),
        Some(("office", sub)) => handle_office(sub),
        Some(("pdf", sub)) => handle_pdf(sub),
        _ => unreachable!(),
    }
}

fn common_args() -> Vec<Arg> {
    vec![
        Arg::new("input")
            .short('i')
            .long("input")
            .value_name("file")
            .help("Input file (use stdin if omitted for non-office documents)"),
        Arg::new("output")
            .short('o')
            .long("output")
            .value_name("file")
            .help("Output file (use stdout if omitted for non-office documents)"),
        Arg::new("config")
            .short('c')
            .long("config")
            .required(true)
            .value_parser(CONFIG_LIST)
            .help("Conversion configuration"),
        Arg::new("punct")
            .short('p')
            .long("punct")
            .action(clap::ArgAction::SetTrue)
            .help("Enable punctuation conversion"),
    ]
}

fn handle_convert(matches: &ArgMatches) -> Result<(), Box<dyn std::error::Error>> {
    let input_file = matches.get_one::<String>("input");
    let output_file = matches.get_one::<String>("output");
    let config = matches.get_one::<String>("config").unwrap();
    let in_enc = matches.get_one::<String>("in_enc").unwrap();
    let out_enc = matches.get_one::<String>("out_enc").unwrap();
    let punctuation = matches.get_flag("punct");

    let is_console = input_file.is_none();
    let mut input: Box<dyn Read> = match input_file {
        Some(file_name) => Box::new(BufReader::new(File::open(file_name)?)),
        None => {
            if io::stdin().is_terminal() {
                println!("Input text to convert, <ctrl-z/d> to submit:");
            }
            Box::new(BufReader::new(io::stdin().lock()))
        }
    };

    let mut buffer = read_input(&mut *input, is_console)?;
    if in_enc == "UTF-8" && out_enc != "UTF-8" {
        remove_utf8_bom(&mut buffer);
    }

    let input_str = decode_input(&buffer, in_enc)?;
    let output_str = OpenCC::new().convert(&input_str, config, punctuation);

    let is_console_output = output_file.is_none();
    let mut output: Box<dyn Write> = match output_file {
        Some(file_name) => Box::new(BufWriter::new(File::create(file_name)?)),
        None => Box::new(BufWriter::new(io::stdout().lock())),
    };

    let final_output = if is_console_output && !output_str.ends_with('\n') {
        format!("{output_str}\n")
    } else {
        output_str
    };

    encode_and_write_output(&final_output, out_enc, &mut output)?;
    output.flush()?;

    Ok(())
}

fn handle_office(matches: &ArgMatches) -> Result<(), Box<dyn std::error::Error>> {
    let office_extensions: HashSet<&'static str> =
        ["docx", "xlsx", "pptx", "odt", "ods", "odp", "epub"].into();

    let input_file = matches
        .get_one::<String>("input")
        .ok_or("❌  Input file is required for office mode")?;

    let output_file = matches.get_one::<String>("output");
    let config = matches.get_one::<String>("config").unwrap();
    let punctuation = matches.get_flag("punct");
    let keep_font = matches.get_flag("keep_font");
    let auto_ext = matches.get_flag("auto_ext");
    let format = matches.get_one::<String>("format").map(String::as_str);

    let office_format = match format {
        Some(f) => f.to_lowercase(),
        None => {
            if auto_ext {
                let ext = std::path::Path::new(input_file)
                    .extension()
                    .and_then(|e| e.to_str())
                    .ok_or("❌  Cannot infer file extension")?;
                if office_extensions.contains(ext) {
                    ext.to_string()
                } else {
                    return Err(format!("❌  Unsupported Office extension: .{ext}").into());
                }
            } else {
                return Err("❌  Please provide --format or use --auto-ext".into());
            }
        }
    };

    let helper = OpenCC::new();

    let final_output = match output_file {
        Some(path) => {
            if auto_ext
                && std::path::Path::new(path).extension().is_none()
                && office_extensions.contains(office_format.as_str())
            {
                format!("{path}.{}", office_format)
            } else {
                path.clone()
            }
        }
        None => {
            let input_path = std::path::Path::new(input_file);
            let file_stem = input_path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("converted");
            let ext = office_format.as_str();
            let parent = input_path.parent().unwrap_or_else(|| ".".as_ref());
            // run conversion on the stem
            let file_stem_converted = helper.convert(file_stem, config, punctuation);
            // pick final stem depending on auto_ext
            let final_stem = if auto_ext {
                format!("{file_stem_converted}_converted")
            } else {
                format!("{file_stem}_converted")
            };

            parent
                .join(format!("{final_stem}.{ext}"))
                .to_string_lossy()
                .to_string()
        }
    };

    match OfficeConverter::convert(
        input_file,
        &final_output,
        &office_format,
        &helper,
        config,
        punctuation,
        keep_font,
    ) {
        Ok(result) if result.success => {
            eprintln!("{}\n📁  Output saved to: {}", result.message, final_output);
        }
        Ok(result) => {
            eprintln!("❌  Office document conversion failed: {}", result.message);
        }
        Err(e) => {
            eprintln!("❌  Error: {}", e);
        }
    }

    Ok(())
}

fn read_input(input: &mut dyn Read, is_console: bool) -> io::Result<Vec<u8>> {
    let mut buffer = Vec::new();
    if is_console {
        let mut chunk = [0; 1024];
        while let Ok(bytes_read) = input.read(&mut chunk) {
            if bytes_read == 0 {
                break;
            }
            buffer.extend_from_slice(&chunk[..bytes_read]);
        }
    } else {
        input.read_to_end(&mut buffer)?;
    }
    Ok(buffer)
}

fn handle_pdf(matches: &ArgMatches) -> Result<(), Box<dyn std::error::Error>> {
    let input_file = matches
        .get_one::<String>("input")
        .ok_or("❌  Input PDF is required for pdf mode (-i/--input)")?;

    let output_file = matches.get_one::<String>("output");
    let config = matches.get_one::<String>("config").unwrap();
    let punctuation = matches.get_flag("punct");

    let reflow = matches.get_flag("reflow");
    let compact = matches.get_flag("compact");
    let header = matches.get_flag("header");

    // ---- default output: <input_stem>_converted.txt (same folder) ----
    let final_output = match output_file {
        Some(path) => path.clone(),
        None => {
            let input_path = std::path::Path::new(input_file);
            let parent = input_path.parent().unwrap_or_else(|| ".".as_ref());
            let stem = input_path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("input");
            parent
                .join(format!("{stem}_converted.txt"))
                .to_string_lossy()
                .to_string()
        }
    };

    println!("Extracting PDF page-by-page with PDFium: {input_file}");

    // Load Pdfium native (dev + release friendly)
    let (pdfium, lib_path) = PdfiumLibrary::load_with_fallbacks()?;
    println!("Loaded pdfium: {}", lib_path.display());

    let mut pages: Vec<String> = Vec::new();

    // Page-by-page extraction with progress
    extract_pdf_pages_with_callback_pdfium(&pdfium, input_file, |page, total, text| {
        // same progress style you like
        pdfium_helper::print_progress(page, total, text);

        // collect page text
        pages.push(text.to_owned());
    })?;

    println!(); // move to next line after progress

    let mut extracted = pages.concat();

    println!(
        "Total extracted characters: {}",
        pdfium_helper::format_thousand(extracted.chars().count())
    );

    // Optional reflow
    if reflow {
        println!("Reflowing CJK paragraphs...");
        extracted = reflow_cjk_paragraphs(
            &extracted, header,  // add_pdf_page_header
            compact, // compact
        );
    }

    // OpenCC conversion
    println!("Converting with OpenCC (config={config}, punct={punctuation}) ...");
    let helper = OpenCC::new();
    let converted = helper.convert(&extracted, config, punctuation);

    // Write as UTF-8 text (Unix newlines are usually nicer for CLI output)
    println!("Writing output to: {final_output}");
    write_text_unix_newlines(&final_output, &converted)?;

    eprintln!("✅  PDF converted.\n📁  Output saved to: {}", final_output);
    Ok(())
}

/// Write UTF-8 text using Unix newlines (`\n`) on all platforms
fn write_text_unix_newlines<P: AsRef<std::path::Path>>(path: P, s: &str) -> io::Result<()> {
    let normalized = s.replace("\r\n", "\n").replace('\r', "\n");
    std::fs::write(path, normalized.as_bytes())
}

fn decode_input(buffer: &[u8], enc: &str) -> io::Result<String> {
    if enc == "UTF-8" {
        return Ok(String::from_utf8_lossy(buffer).into_owned());
    }
    let encoding = Encoding::for_label(enc.as_bytes()).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("Unsupported encoding: {enc}"),
        )
    })?;
    let mut reader = DecodeReaderBytesBuilder::new()
        .encoding(Some(encoding))
        .build(buffer);
    let mut decoded = String::new();
    reader.read_to_string(&mut decoded)?;
    Ok(decoded)
}

fn encode_and_write_output(output_str: &str, enc: &str, output: &mut dyn Write) -> io::Result<()> {
    if enc == "UTF-8" {
        write!(output, "{}", output_str)
    } else {
        let encoding = Encoding::for_label(enc.as_bytes()).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("Unsupported encoding: {enc}"),
            )
        })?;
        let (encoded, _, _) = encoding.encode(output_str);
        output.write_all(&encoded)
    }
}

fn remove_utf8_bom(input: &mut Vec<u8>) {
    if input.starts_with(&[0xEF, 0xBB, 0xBF]) {
        input.drain(..3);
    }
}
