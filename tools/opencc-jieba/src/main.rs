use clap::builder::{StringValueParser, TypedValueParser, ValueParser};
use clap::{Arg, ArgMatches, Command};
use opencc_jieba_rs::{OpenCC as OpenccJieba, OpenccConfig as OpenccJiebaConfig};
use opencc_utils::{
    convert_office_document, decode_input, encode_and_write_output, exit_on_error,
    handle_pdf_with_converter, normalize_line_endings, open_input_file, open_output,
    remove_utf8_bom, should_remove_bom, validate_distinct_input_output, validate_encoding,
    validate_input_file, validate_output_path, PdfOptions,
};
use std::io::{self, BufRead, BufReader, IsTerminal, Read};
use std::sync::OnceLock;

const BLUE: &str = "\x1B[1;34m";
const RESET: &str = "\x1B[0m";

const PROMPT_CONVERT: &str = concat!(
    "\x1B[1;34m",
    "Input text to convert, <ctrl-z> or <ctrl-d> to submit:",
    "\x1B[0m"
);

const PROMPT_SEGMENT: &str = concat!(
    "\x1B[1;34m",
    "Input text to segment, <ctrl-z> or <ctrl-d> to submit:",
    "\x1B[0m"
);

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let matches = Command::new("opencc-jieba")
        .about(format!(
            "{}OpenCC Jieba Rust: Command Line Open Chinese Converter{}",
            BLUE, RESET
        ))
        .subcommand_required(true)
        .arg_required_else_help(true)
        .subcommand(
            Command::new("convert")
                .about(format!(
                    "{}opencc-jieba convert: Convert Chinese Traditional/Simplified text using OpenCC{}",
                    BLUE, RESET
                ))
                .args(common_args())
                .mut_arg("config", |a| a.required(true))
                .args(enc_args()),
        )
        .subcommand(
            Command::new("office")
                .about(format!(
                    "{}opencc-jieba office: Convert Office or EPUB documents using OpenCC{}",
                    BLUE, RESET
                ))
                .args(common_args())
                .mut_arg("config", |a| a.required(true))
                .arg(
                    Arg::new("format")
                        .short('f')
                        .long("format")
                        .value_name("ext")
                        .help(
                            "Force office document format <ext>: docx, xlsx, pptx, odt, ods, odp, epub",
                        ),
                )
                .arg(
                    Arg::new("keep_font")
                        .short('k')
                        .long("keep-font")
                        .action(clap::ArgAction::SetTrue)
                        .help("Preserve original font styles"),
                )
                .arg(
                    Arg::new("convert_filename")
                        .short('F')
                        .long("convert-filename")
                        .action(clap::ArgAction::SetTrue)
                        .help(
                            "Convert the output filename using the selected OpenCC configuration",
                        ),
                ),
        )
        .subcommand(
            Command::new("segment")
                .about(format!(
                    "{}opencc-jieba segment: Segment Chinese input text into words{}",
                    BLUE, RESET
                ))
                .arg(
                    Arg::new("input")
                        .short('i')
                        .long("input")
                        .value_name("file")
                        .help("Input file to segment")
                        .required(false),
                )
                .arg(
                    Arg::new("output")
                        .short('o')
                        .long("output")
                        .value_name("file")
                        .help("Write segmented result to file")
                        .required(false),
                )
                .arg(
                    Arg::new("delimiter")
                        .short('d')
                        .long("delim")
                        .value_name("character")
                        .help("Delimiter character for segmented text (use \" \" for space)")
                        .required(false)
                        .default_value(" "),
                )
                .arg(
                    Arg::new("separator")
                        .short('s')
                        .long("separator")
                        .value_name("character")
                        .help("Separator character for segmented mode=tag (use \" \" for space)")
                        .required(false)
                        .default_value("/"),
                )
                .arg(
                    Arg::new("mode")
                        .short('m')
                        .long("mode")
                        .value_name("mode")
                        .value_parser(["cut", "search", "all", "tag"])
                        .default_value("cut")
                        .help("Segmentation mode: cut | search | all | tag"),
                )
                .arg(
                    Arg::new("no_hmm")
                        .long("no-hmm")
                        .action(clap::ArgAction::SetTrue)
                        .help("Disable HMM for segmentation and tagging"),
                )
                .args(enc_args()),
        )
        .subcommand(
            Command::new("pdf")
                .about("Extract PDF text and convert using Opencc-Jieba")
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
                        .short('C')
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
        Some(("convert", sub_matches)) => handle_convert(sub_matches),
        Some(("office", sub_matches)) => handle_office(sub_matches),
        Some(("segment", sub_matches)) => handle_segment(sub_matches),
        Some(("pdf", sub_matches)) => handle_pdf(sub_matches),
        _ => unreachable!("Clap ensures only valid subcommands are passed"),
    };

    exit_on_error(result);

    Ok(())
}

fn get_supported_configs() -> &'static str {
    static SUPPORTED: OnceLock<String> = OnceLock::new();
    SUPPORTED.get_or_init(|| {
        let mut s = String::with_capacity(128);
        for (i, cfg) in OpenccJiebaConfig::ALL.iter().enumerate() {
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
        OpenccJiebaConfig::try_from(s.as_str())
            .map(OpenccJiebaConfig::as_str)
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
            .help("Input <file> (use stdin if omitted for non-office documents)"),
        Arg::new("output")
            .short('o')
            .long("output")
            .value_name("file")
            .help("Output <file> (use stdout if omitted for non-office documents)"),
        Arg::new("config")
            .short('c')
            .long("config")
            .value_name("config")
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

fn enc_args() -> Vec<Arg> {
    vec![
        Arg::new("in_enc")
            .long("in-enc")
            .value_name("encoding")
            .default_value("UTF-8")
            .global(true)
            .help("Encoding for input: UTF-8|GB2312|GBK|gb18030|BIG5"),
        Arg::new("out_enc")
            .long("out-enc")
            .value_name("encoding")
            .default_value("UTF-8")
            .global(true)
            .help("Encoding for output: UTF-8|GB2312|GBK|gb18030|BIG5"),
    ]
}

fn handle_convert(matches: &ArgMatches) -> Result<(), Box<dyn std::error::Error>> {
    let input_file = matches.get_one::<String>("input");
    let output_file = matches.get_one::<String>("output");
    let config = matches.get_one::<String>("config").unwrap();
    let in_enc = matches.get_one::<String>("in_enc").unwrap();
    let out_enc = matches.get_one::<String>("out_enc").unwrap();
    let punctuation = matches.get_flag("punct");

    validate_encoding(in_enc)?;
    validate_encoding(out_enc)?;
    if let Some(input) = input_file {
        validate_input_file(input)?;
    }
    if let Some(path) = output_file {
        validate_output_path(path)?;
        if let Some(input) = input_file {
            validate_distinct_input_output(input, path)?;
        }
    }

    let is_console = input_file.is_none();
    let mut input: Box<dyn Read> = match input_file {
        Some(file_name) => Box::new(open_input_file(file_name)?),
        None => {
            if io::stdin().is_terminal() {
                eprintln!("{PROMPT_CONVERT}");
            }
            Box::new(BufReader::new(io::stdin().lock()))
        }
    };

    let mut buffer = read_input(&mut *input, is_console)?;
    if should_remove_bom(in_enc, out_enc) {
        remove_utf8_bom(&mut buffer);
    }

    let input_str = decode_input(&buffer, in_enc)?;
    let opencc = OpenccJieba::new();
    let output_str = opencc.convert(&input_str, config, punctuation);

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
    let convert_filename = matches.get_flag("convert_filename");
    let format = matches.get_one::<String>("format").map(String::as_str);

    validate_input_file(input_file)?;
    if let Some(path) = output_file {
        validate_output_path(path)?;
    }
    let helper = OpenccJieba::new();
    convert_office_document(
        input_file,
        output_file,
        format,
        keep_font,
        convert_filename,
        config,
        punctuation,
        |text, config, punctuation| helper.convert(text, config, punctuation),
    )?;

    Ok(())
}

fn handle_segment(matches: &ArgMatches) -> Result<(), Box<dyn std::error::Error>> {
    let input_file = matches.get_one::<String>("input");
    let output_file = matches.get_one::<String>("output");
    let delimiter = matches.get_one::<String>("delimiter").unwrap();
    let separator = matches.get_one::<String>("separator").unwrap();
    let mode = matches.get_one::<String>("mode").unwrap();
    let in_enc = matches.get_one::<String>("in_enc").unwrap();
    let out_enc = matches.get_one::<String>("out_enc").unwrap();
    let hmm = !matches.get_flag("no_hmm");

    validate_encoding(in_enc)?;
    validate_encoding(out_enc)?;
    if let Some(input) = input_file {
        validate_input_file(input)?;
    }
    if let Some(path) = output_file {
        validate_output_path(path)?;
        if let Some(input) = input_file {
            validate_distinct_input_output(input, path)?;
        }
    }

    let is_console = input_file.is_none();
    let mut input: Box<dyn Read> = match input_file {
        Some(file_name) => Box::new(open_input_file(file_name)?),
        None => {
            if io::stdin().is_terminal() {
                eprintln!("{PROMPT_SEGMENT}");
            }
            Box::new(BufReader::new(io::stdin().lock()))
        }
    };

    let mut buffer = read_input(&mut *input, is_console)?;
    if should_remove_bom(in_enc, out_enc) {
        remove_utf8_bom(&mut buffer);
    }

    let mut input_str = decode_input(&buffer, in_enc)?;
    let opencc = OpenccJieba::new();
    if is_console {
        input_str = normalize_line_endings(&input_str);
        // Remove trailing submit newline from interactive console input
        input_str = input_str.trim_end_matches('\n').to_string();
    }

    let output_str = match mode.as_str() {
        "search" => opencc.jieba_cut_for_search(&input_str, hmm).join(delimiter),
        "all" => opencc.jieba_cut_all(&input_str).join(delimiter),
        "tag" => {
            let pairs = opencc.jieba_tag(&input_str, hmm);
            let mut out = String::new();

            for (i, (w, t)) in pairs.into_iter().enumerate() {
                if i > 0 {
                    out.push_str(delimiter);
                }
                out.push_str(&w);
                out.push_str(&separator);
                out.push_str(&t);
            }

            out
        }
        _ => opencc.jieba_cut(&input_str, hmm).join(delimiter),
    };

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

fn read_input(input: &mut dyn Read, is_console: bool) -> io::Result<Vec<u8>> {
    if is_console {
        let mut reader = BufReader::new(input);
        let mut text = String::new();
        let mut line = String::new();

        loop {
            line.clear();
            let n = reader.read_line(&mut line)?;
            if n == 0 {
                break;
            }
            text.push_str(&line);
        }

        Ok(text.into_bytes())
    } else {
        let mut buffer = Vec::new();
        input.read_to_end(&mut buffer)?;
        Ok(buffer)
    }
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

    validate_input_file(input_file)?;
    if let Some(path) = output_file {
        validate_output_path(path)?;
    }
    let options = PdfOptions {
        input_file,
        output_file,
        config,
        punctuation,
        reflow,
        compact,
        header,
        extract_only,
        pdfium_dir,
        converter_name: "Opencc-Jieba",
    };

    if extract_only {
        return handle_pdf_with_converter(options, |_, _, _| unreachable!());
    }

    let helper = OpenccJieba::new();
    handle_pdf_with_converter(options, |text, config, punctuation| {
        helper.convert(text, config, punctuation)
    })
}
