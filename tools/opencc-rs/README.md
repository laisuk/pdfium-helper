# opencc-rs

**opencc-rs** is a fast, cross-platform command-line tool for converting Chinese text between Simplified and Traditional
variants using **OpenCC lexicons**, with advanced support for:

- Plain text conversion
- Office documents (`.docx`, `.odt`, `.epub`, etc.)
- PDF text extraction via **PDFium**
- CJK paragraph reflow optimized for novels and ebooks

It is designed to be **portable**, **dependency-light**, and **easy to use** for both developers and end users.

---

## Features

- 🚀 High-performance OpenCC conversion (`opencc-fmmseg` Rust backend)
- 📄 Convert plain text files
- 📦 Convert Office / EPUB documents
- 📕 Extract and convert text-embedded PDFs (`PDFium` backend)
- 🧠 CJK-aware paragraph reflow (novels / ebooks friendly)
- 📊 Live page-by-page PDF progress display
- 🧳 Portable: no installer, no system dependencies

> ⚠️ **Note**
>
> Only **text-embedded PDFs** are supported.
> Scanned / image-only PDFs are not supported.

---

## Installation

### Option 1: Download prebuilt binaries (recommended)

1. Go to **GitHub Releases**
2. Download:
    - `opencc-rs-<platform>.zip`
    - Pdfium native libraries (see below)
3. Extract into the same directory

You can now run `opencc-rs` directly.

---

## Pdfium native library setup (important)

`opencc-rs` does **not bundle** Pdfium.
You must provide Pdfium native libraries yourself.

### Easiest way (recommended)

If you are unsure which native library to download:

```
opencc-rs.exe
pdfium/
  win-x64/pdfium.dll
  linux-x64/libpdfium.so
  osx-arm64/libpdfium.dylib
```

Just copy the entire `pdfium/` directory next to the executable.
`opencc-rs` will automatically select the correct one.

### Advanced (single library)

You may also place **one native library** next to the executable:

```
opencc-rs.exe
pdfium.dll        (Windows)
libpdfium.so     (Linux)
libpdfium.dylib  (macOS)
```

---

## Usage

```
opencc-rs <command> [options]
```

Available subcommands:

- `convert` – convert plain text
- `office`  – convert Office / EPUB documents
- `pdf`     – extract + convert PDF files

---

## Plain text conversion

```
opencc-rs convert -i input.txt -o output.txt -c s2t
```

Options:

- `-i, --input`        Input file (default: stdin)
- `-o, --output`       Output file (default: stdout)
- `-c, --config`       OpenCC config (e.g. `s2t`, `t2s`, `s2tw`)
- `-p, --punct`        Convert punctuation

---

## Office / EPUB conversion

```
opencc-rs office -i book.docx -o book_converted.docx -c s2t
```

Supported formats include:

- `.docx`
- `.odt`
- `.epub`

Options:

- `--keep-font`        Preserve original fonts
- `--format <ext>`    Force document format
- `--auto-ext`        Infer format from file extension

---

## PDF conversion

```
opencc-rs pdf -i book.pdf -c s2t -p -r
```

### PDF options

- `-r, --reflow`       Reflow CJK paragraphs (recommended for novels)
- `--compact`          Compact reflow output
- `-H, --header`       Add page headers like `=== [Page 3/120] ===`

If no output file is specified:

```
input.pdf → input_converted.txt
```

### Example output

```
[4410/4410] (100%) Extracted 191 chars
Total extracted characters: 1,598,793
Reflowing CJK paragraphs...
Converting with OpenCC (config=s2t, punct=true) ...
Done.
```

---

## Progress display note

When running via `cargo run`, stdout may be buffered and intermediate
progress updates may not be visible.

For best progress display, run the compiled binary directly:

```
target/release/opencc-rs pdf -i book.pdf -c s2t -p -r
```

---

## Development

```
cargo build --release
```

For development, `Pdfium` can be placed in:

- executable directory
- current working directory
- `pdfium/<platform>/` layout
- directory specified by `PDFIUM_LIB_DIR`

---

## Supported platforms

- Windows (x64)
- Linux (x64)
- macOS (Intel / Apple Silicon)

---

## Antivirus false-positive notice

Some antivirus products may report **false positives** for `opencc-rs`, especially on Windows.

This is a known issue affecting many **Rust-based CLI tools**, and is typically caused by a combination of:

- Statically linked or highly optimized Rust binaries
- Low distribution prevalence (new or niche tools)
- Heuristic / ML-based detection engines
- Command-line behavior such as file processing and native library loading

### Important facts

- `opencc-rs` is built using the **official Rust stable toolchain**
- No packers, obfuscates, or self-modifying code are used
- No network access, persistence, or privilege escalation behavior exists
- The source code is fully open and auditable

If your antivirus flags the binary:

- Verify the checksum against the GitHub Release
- Add an exclusion for the executable if necessary
- Or build from source using `cargo build --release`

As the project gains adoption and reputation, these false positives typically disappear automatically.

> ⚠️ This is a detection heuristic issue, not an indication of malicious behavior.

---

## License

This project is licensed under the **MIT License**.

---

## Acknowledgements

- OpenCC project
- PDFium project
- opencc-fmmseg project
