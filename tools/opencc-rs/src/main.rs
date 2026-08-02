use clap::builder::{StringValueParser, TypedValueParser, ValueParser};
use clap::{Arg, ArgMatches, Command};
use opencc_fmmseg::{
    CustomDictFileSpec, CustomDictMode, DetofuLevel, DetofuMap, DictSlot, DictionaryMaxlength,
    OpenCC, OpenccConfig,
};
use opencc_utils::{
    convert_office_document, decode_input, encode_and_write_output, exit_on_error,
    handle_pdf_with_converter, open_input_file, open_output, remove_utf8_bom, should_remove_bom,
    validate_distinct_input_output, validate_encoding, validate_input_file, validate_output_path,
    PdfOptions,
};
use std::io::{self, BufReader, IsTerminal, Read};
use std::path::PathBuf;
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
                    Arg::new("keep-ids")
                        .long("keep-ids")
                        .help("Preserve Unicode IDS expressions during conversion")
                        .action(clap::ArgAction::SetTrue),
                )
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
        Arg::new("norm-compat")
            .short('n')
            .long("norm-compat")
            .action(clap::ArgAction::SetTrue)
            .help("Normalize CJK Compatibility Ideographs before conversion."),
        Arg::new("detofu")
            .long("detofu")
            .value_name("LEVEL")
            .num_args(0..=1)
            .default_missing_value("all")
            .help("Apply tofu-safe fallback after conversion: all, ext-c, ext-d, ext-e, ext-f, ext-g, ext-h, ext-i"),
        Arg::new("detofu-file")
            .long("detofu-file")
            .value_name("FILE")
            .requires("detofu")
            .help(
                "Load additional detofu fallback mappings from a UTF-8 text file. \
         Custom mappings override built-in mappings (requires --detofu)",
            ),
        Arg::new("custom-dict")
            .short('D')
            .long("custom-dict")
            .value_name("SLOT:MODE:FILE")
            .action(clap::ArgAction::Append)
            .help("Custom dictionary file, e.g. hkphrasesrev:append:my_hk_dict.txt"),
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

    let detofu_map = match matches.get_one::<String>("detofu") {
        Some(level) => {
            let level = DetofuLevel::parse(level)?;
            match matches.get_one::<String>("detofu-file") {
                Some(path) => {
                    validate_input_file(path)?;
                    Some(DetofuMap::builtin(level).with_custom_file(path)?)
                }
                None => Some(DetofuMap::builtin(level)),
            }
        }
        None => None,
    };

    let mut cc = build_opencc(matches)?;

    let is_console = input_file.is_none();
    let mut input: Box<dyn Read> = match input_file {
        Some(file_name) => Box::new(open_input_file(file_name)?),
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
    let normalized_input;
    let convert_input: &str = if matches.get_flag("norm-compat") {
        normalized_input = cc.normalize_compat(&input_str);
        &normalized_input
    } else {
        &input_str
    };

    if matches.get_flag("keep-ids") {
        cc.set_preserve_ids(true);
    }

    let output_str = cc.convert(&convert_input, config, punctuation);

    let output_str = if let Some(map) = detofu_map {
        map.detofu(&output_str)
    } else {
        output_str
    };

    // let output_str = if let Some(level) = matches.get_one::<String>("detofu") {
    //     let level = DetofuLevel::parse(level)?;
    //
    //     if let Some(path) = matches.get_one::<String>("detofu-file") {
    //         cc.detofu_with_custom_file(&output_str, level, path)?
    //     } else {
    //         cc.detofu(&output_str, level)
    //     }
    // } else {
    //     output_str
    // };

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
    let helper = build_opencc(matches)?;

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
        converter_name: "Opencc-Fmmseg",
    };

    if extract_only {
        return handle_pdf_with_converter(options, |_, _, _| unreachable!());
    }

    let helper = build_opencc(matches)?;
    handle_pdf_with_converter(options, |text, config, punctuation| {
        helper.convert(text, config, punctuation)
    })
}

fn build_opencc(matches: &ArgMatches) -> Result<OpenCC, Box<dyn std::error::Error>> {
    let Some(values) = matches.get_many::<String>("custom-dict") else {
        return Ok(OpenCC::new());
    };

    let specs = values
        .map(|v| parse_custom_dict_spec(v))
        .collect::<Result<Vec<_>, _>>()?;

    for spec in &specs {
        for file in &spec.files {
            validate_input_file(file)?;
        }
    }

    let dictionary = DictionaryMaxlength::from_zstd()?.with_custom_dict_files(&specs)?;

    Ok(OpenCC::from_dictionary(dictionary))
}

fn parse_custom_dict_spec(
    arg: &str,
) -> Result<CustomDictFileSpec<PathBuf>, Box<dyn std::error::Error>> {
    let mut parts = arg.splitn(3, ':');

    let slot = parts.next().ok_or("Missing custom dict slot")?;
    let mode = parts.next().ok_or("Missing custom dict mode")?;
    let file = parts.next().ok_or("Missing custom dict file")?;
    let file = file.trim();
    if file.is_empty() {
        return Err("Custom dictionary path cannot be empty".into());
    }

    let slot = DictSlot::from_name_ignore_ascii_case(slot.trim())
        .ok_or_else(|| format!("Unknown custom dictionary slot: {slot}"))?;

    let mode = match mode.trim().to_ascii_lowercase().as_str() {
        "append" => CustomDictMode::Append,
        "override" => CustomDictMode::Override,
        other => return Err(format!("Unknown custom dict mode: {other}").into()),
    };

    Ok(CustomDictFileSpec {
        slot,
        files: vec![PathBuf::from(file)],
        mode,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cli_exposes_opencc_fmmseg_0_11_4_hong_kong_phrase_configs() {
        let supported = get_supported_configs();

        assert!(supported.split(" | ").any(|config| config == "t2hkp"));
        assert!(supported.split(" | ").any(|config| config == "hk2tp"));
        assert_eq!(OpenccConfig::try_from("t2hkp"), Ok(OpenccConfig::T2hkp));
        assert_eq!(OpenccConfig::try_from("hk2tp"), Ok(OpenccConfig::Hk2tp));
    }

    #[test]
    fn hong_kong_phrase_configs_convert_in_both_directions() {
        let dictionary = DictionaryMaxlength::from_zstd().unwrap();
        let (traditional, hong_kong) = dictionary
            .hk_phrases
            .map
            .iter()
            .next()
            .map(|(source, target)| (source.iter().collect::<String>(), target.to_string()))
            .expect("the built-in HKPhrases dictionary should not be empty");
        let (hong_kong_reverse, traditional_reverse) = dictionary
            .hk_phrases_rev
            .map
            .iter()
            .next()
            .map(|(source, target)| (source.iter().collect::<String>(), target.to_string()))
            .expect("the built-in HKPhrasesRev dictionary should not be empty");
        let cc = OpenCC::from_dictionary(dictionary);

        assert_eq!(cc.convert(&traditional, "t2hkp", false), hong_kong);
        assert_eq!(
            cc.convert(&hong_kong_reverse, "hk2tp", false),
            traditional_reverse
        );
    }

    #[test]
    fn custom_dict_specs_use_canonical_case_insensitive_slot_parsing() {
        let spec = parse_custom_dict_spec("hkphrasesrev:APPEND:custom.txt").unwrap();

        assert_eq!(spec.slot, DictSlot::HKPhrasesRev);
        assert_eq!(spec.mode, CustomDictMode::Append);
        assert_eq!(spec.files, vec![PathBuf::from("custom.txt")]);
    }

    #[test]
    fn custom_dict_specs_reject_empty_paths() {
        assert!(parse_custom_dict_spec("STPhrases:append:   ").is_err());
    }
}
