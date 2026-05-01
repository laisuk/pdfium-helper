use clap::builder::{StringValueParser, TypedValueParser, ValueParser};
use clap::{Arg, ArgMatches, Command};
use opencc_fmmseg::{OpenCC, OpenccConfig};
use opencc_utils::{
    convert_office_document, decode_input, encode_and_write_output, exit_on_error,
    handle_pdf_with_converter, open_output, remove_utf8_bom, should_remove_bom, PdfOptions,
};
use std::fs::File;
use std::io::{self, BufReader, IsTerminal, Read};
use std::sync::OnceLock;

fn main() {
    let matches = Command::new("opencc-rs")
        .about("OpenCC Rust: Command Line Open Chinese Converter")
        .subcommand_required(true)
        .arg_required_else_help(true)
        .subcommand(
            Command::new("convert")
                .about("Convert plain text using OpenCC")
                .args(common_args())
                // 👇 require config for this subcommand
                .mut_arg("config", |a| a.required(true))
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
                .mut_arg("config", |a| a.required(true))
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
                )
                .arg(
                    Arg::new("extract")
                        .short('e')
                        .long("extract")
                        .action(clap::ArgAction::SetTrue)
                        .help("Extract text from PDF document only (default: false)"),
                )
                .arg(
                    Arg::new("pdfium")
                        .long("pdfium")
                        .value_name("dir")
                        .help("Custom Pdfium native base dir; falls back to default bundled lookup if invalid"),
                )
                // 👇 KEY LINE
                .arg_required_else_help(false)
                .mut_arg("config", |a| a.required_unless_present("extract")),
        )
        .get_matches();

    let result = match matches.subcommand() {
        Some(("convert", sub)) => handle_convert(sub),
        Some(("office", sub)) => handle_office(sub),
        Some(("pdf", sub)) => handle_pdf(sub),
        _ => unreachable!(),
    };

    exit_on_error(result);
}

fn get_supported_configs() -> &'static str {
    static SUPPORTED: OnceLock<String> = OnceLock::new();
    SUPPORTED.get_or_init(|| {
        let mut s = String::with_capacity(128);
        for (i, cfg) in OpenccConfig::ALL.iter().enumerate() {
            if i > 0 {
                s.push_str(" | ");
            }
            s.push_str(cfg.as_str());
        }
        s
    })
}

fn config_value_parser() -> ValueParser {
    ValueParser::new(StringValueParser::new().try_map(|s| {
        OpenccConfig::try_from(s.as_str())
            .map(OpenccConfig::as_str)
            .map(str::to_owned)
            .map_err(|_| format!("\nSupported configs: {}", get_supported_configs()))
    }))
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
            // .required(true)
            .value_parser(config_value_parser())
            .help(format!(
                "Conversion configuration ({})",
                get_supported_configs()
            )),
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
    if should_remove_bom(in_enc, out_enc) {
        remove_utf8_bom(&mut buffer);
    }

    let input_str = decode_input(&buffer, in_enc)?;
    let output_str = OpenCC::new().convert(&input_str, config, punctuation);

    let (is_console_output, mut output) = open_output(output_file)?;

    let final_output = if is_console_output && !output_str.ends_with('\n') {
        format!("{output_str}\n")
    } else {
        output_str
    };

    encode_and_write_output(&final_output, out_enc, &mut *output)?;
    output.flush()?;

    Ok(())
}

fn handle_office(matches: &ArgMatches) -> Result<(), Box<dyn std::error::Error>> {
    let input_file = matches
        .get_one::<String>("input")
        .ok_or("❌  Input file is required for office mode")?;

    let output_file = matches.get_one::<String>("output");
    let config = matches.get_one::<String>("config").unwrap();
    let punctuation = matches.get_flag("punct");
    let keep_font = matches.get_flag("keep_font");
    let auto_ext = matches.get_flag("auto_ext");
    let format = matches.get_one::<String>("format").map(String::as_str);

    let helper = OpenCC::new();
    convert_office_document(
        input_file,
        output_file,
        format,
        auto_ext,
        keep_font,
        config,
        punctuation,
        |text, config, punctuation| helper.convert(text, config, punctuation),
    )?;

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
    let punctuation = matches.get_flag("punct");
    let reflow = matches.get_flag("reflow");
    let compact = matches.get_flag("compact");
    let header = matches.get_flag("header");
    let extract_only = matches.get_flag("extract");
    let pdfium_dir = matches.get_one::<String>("pdfium");
    let config = if extract_only {
        None
    } else {
        Some(
            matches
                .get_one::<String>("config")
                .ok_or("❌  --config is required unless --extract is used")?
                .as_str(),
        )
    };

    let helper = OpenCC::new();
    handle_pdf_with_converter(
        PdfOptions {
            input_file,
            output_file,
            config,
            punctuation,
            reflow,
            compact,
            header,
            extract_only,
            pdfium_dir,
            converter_name: "Opencc-Fmmseg",
        },
        |text, config, punctuation| helper.convert(text, config, punctuation),
    )
}
