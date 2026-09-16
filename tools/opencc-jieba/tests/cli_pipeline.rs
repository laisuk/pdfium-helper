use std::io::Write;
use std::process::{Command, Stdio};

fn run(args: &[&str], input: &str) -> String {
    let mut child = Command::new(env!("CARGO_BIN_EXE_opencc-jieba"))
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(input.as_bytes())
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout)
        .unwrap()
        .replace("\r\n", "\n")
}

#[test]
fn detofu_is_optional_and_runs_after_conversion() {
    assert_eq!(run(&["convert", "-c", "t2s"], "𬴂"), "𬴂\n");
    assert_eq!(run(&["convert", "-c", "t2s", "--detofu"], "𬴂"), "騑\n");
}

#[test]
fn pdf_extract_needs_no_config_or_conversion_engine() {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let input = root.join("tests/samples/CHUNK_ABC.pdf");
    let output =
        std::env::temp_dir().join(format!("opencc-jieba-extract-{}.txt", std::process::id()));
    run(
        &[
            "pdf",
            "--extract",
            "-i",
            input.to_str().unwrap(),
            "-o",
            output.to_str().unwrap(),
            "--pdfium",
            root.to_str().unwrap(),
            "-D",
            "invalid:invalid:missing.txt",
        ],
        "",
    );
    assert!(!std::fs::read_to_string(&output).unwrap().is_empty());
    std::fs::remove_file(output).unwrap();
}

#[test]
fn segment_normalizes_without_running_conversion() {
    let engine = opencc_jieba_rs::OpenCC::new();
    let input = "Ａ車漢語";
    for (flag, normalized) in [
        ("-n", engine.normalize_compat(input)),
        ("-E", engine.normalize_compat_extended(input)),
    ] {
        let expected = engine.jieba_cut(&normalized, true).join("|");
        assert_eq!(
            run(&["segment", flag, "--delim", "|"], input),
            format!("{expected}\n")
        );
    }
    let output = Command::new(env!("CARGO_BIN_EXE_opencc-jieba"))
        .args(["segment", "--detofu"])
        .output()
        .unwrap();
    assert!(!output.status.success());
}

#[test]
fn conversion_commands_expose_boolean_detofu() {
    for command in ["convert", "office", "pdf"] {
        let help = run(&[command, "--help"], "");
        assert!(help.contains("--detofu"));
        assert!(!help.contains("--detofu ["));
    }
}
