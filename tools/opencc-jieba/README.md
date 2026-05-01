# opencc-jieba

**opencc-jieba** is a fast, cross-platform command-line tool for converting Chinese text between Simplified and
Traditional
variants using **OpenCC lexicons**, with additional **Jieba** word segmentation support.

It supports:

- Plain text conversion
- Chinese word segmentation
- Office documents (`.docx`, `.odt`, `.epub`, etc.)
- PDF text extraction via **PDFium**
- CJK paragraph reflow optimized for novels and ebooks

It is designed to be **portable**, **dependency-light**, and **easy to use** for both developers and end users.

---

## Features

- 🚀 High-performance OpenCC conversion (`opencc-jieba-rs` Rust backend)
- ✂️ Jieba Chinese word segmentation and tagging
- 📄 Convert plain text files
- 📦 Convert Office / EPUB documents
- 📕 Extract and convert text-embedded PDFs (**PDFium backend**)
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
2. Download the appropriate package for your platform
3. Extract the archive

You can now run `opencc-jieba` directly.

> ✅ **Out-of-the-box ready**
>
> Each release package includes:
>
> - `opencc-jieba`
> - the matching platform Pdfium native library
> - `README.md`
> - `VERSION` manifest for verified bundled-version reporting

### Portable runtime layout

Current release packages use a flat portable layout:

```text
opencc-jieba(.exe)
pdfium.dll / libpdfium.so / libpdfium.dylib
VERSION
README.md
```

Keep the native library in the same directory as `opencc-jieba` unless you explicitly load Pdfium from another location
with `--pdfium`.

---

## Pdfium runtime note

`opencc-jieba` tries to load Pdfium in this order:

1. The custom path or base directory passed via `--pdfium`
2. The default `pdfium-helper` fallback search:
    - directory containing the current executable
    - current working directory
    - `CARGO_MANIFEST_DIR` during development
    - embedded Pdfium fallback when built with the `pdfium-embed` feature

For each search location, the loader tries:

- a side-by-side native library in the same directory as the executable
- then a bundled `pdfium/<platform>/...` layout

### Version display behavior

When `opencc-jieba` loads bundled or default-discovered Pdfium, it may print a verified version line such as:

```text
Loaded pdfium: R:/PortableApps/pdfium.dll (version: 148.0.7776.0)
```

This only happens when:

- a `VERSION` manifest is present at the portable root or under `pdfium/VERSION`
- the manifest contains `version=...`
- the manifest contains a matching SHA-256 entry for the loaded native library

If the manifest is missing or does not match the native binary, `opencc-jieba` silently falls back to printing the
loaded
path only.

If you pass `--pdfium`, version display is intentionally suppressed for that successful custom load.

---

## Usage

```text
opencc-jieba <command> [options]
```

Available subcommands:

- `convert` – convert plain text
- `segment` – segment Chinese text into words
- `office`  – convert Office / EPUB documents
- `pdf`     – extract + convert PDF files

---

## Plain text conversion

```text
opencc-jieba convert -i input.txt -o output.txt -c s2t
```

Options:

- `-i, --input`        Input file (default: stdin)
- `-o, --output`       Output file (default: stdout)
- `-c, --config`       OpenCC config (e.g. `s2t`, `t2s`, `s2tw`)
- `-p, --punct`        Convert punctuation
- `--in-enc`           Input encoding (default: `UTF-8`)
- `--out-enc`          Output encoding (default: `UTF-8`)

---

## Jieba segmentation

The `segment` subcommand splits Chinese text into words without OpenCC conversion.

```text
opencc-jieba segment -i input.txt -o segmented.txt
```

### Segmentation modes

- `cut`    – default precise mode
- `search` – search-engine mode with finer tokens
- `all`    – full mode
- `tag`    – part-of-speech tagging

### Examples

Segment stdin using the default `/` delimiter:

```text
echo "南京市长江大桥" | opencc-jieba segment
```

Use search mode and a space delimiter:

```text
opencc-jieba segment -i input.txt -o segmented.txt --mode search --delim " "
```

Use full mode:

```text
opencc-jieba segment -i input.txt --mode all
```

Generate part-of-speech tags with custom word and tag separators:

```text
opencc-jieba segment -i input.txt --mode tag --delim " " --separator ":"
```

Disable HMM:

```text
opencc-jieba segment -i input.txt --no-hmm
```

Encoding options are also supported:

```text
opencc-jieba segment -i input-big5.txt -o output-utf8.txt --in-enc BIG5 --out-enc UTF-8
```

---

## Office / EPUB conversion

```text
opencc-jieba office -i book.docx -o book_converted.docx -c s2t
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

```text
opencc-jieba pdf -i book.pdf -c s2t -p -r
```

### PDF options

- `-r, --reflow`       Reflow CJK paragraphs (recommended for novels)
- `--compact`          Compact reflow output
- `-H, --header`       Add page headers like `=== [Page 3/120] ===`
- `-e, --extract`      Extract PDF text only without OpenCC conversion
- `--pdfium <dir>`     Custom Pdfium file or base directory; falls back to default lookup if loading fails

If no output file is specified:

```text
input.pdf -> input_converted.txt
```

### Example output

```text
Extracting PDF page-by-page with PDFium: book.pdf
Loaded pdfium: /path/to/pdfium.dll (version: 148.0.7776.0)
Loading [4410/4410] (100%) Extracted 191 chars
Total extracted characters: 1,598,793
Reflowing CJK paragraphs...
Converting with Opencc-Jieba (config=s2t, punct=true) ...
✅  PDF converted.
```

---

## Progress display note

When running via `cargo run`, stdout may be buffered and intermediate
progress updates may not be visible.

For best progress display, run the compiled binary directly:

```text
target/release/opencc-jieba pdf -i book.pdf -c s2t -p -r
```

---

## Development

```text
cargo build --release -p opencc-jieba
```

For development, Pdfium can be provided via:

- `--pdfium <dir>` or `--pdfium <file>`
- the executable directory
- the current working directory
- a bundled `pdfium/<platform>/` layout
- the directory specified by `PDFIUM_LIB_DIR`

If you build a portable distribution and want verified version reporting, place a compatible `VERSION` manifest at the
artifact root.

---

## Supported platforms

- Windows (x64)
- Linux (x64)
- macOS (Intel / Apple Silicon)

---

## Antivirus false-positive notice

Some antivirus products may report **false positives** for `opencc-jieba`, especially on Windows.

This is a known issue affecting many **Rust-based CLI tools**, and is typically caused by a combination of:

- Statically linked or highly optimized Rust binaries
- Low distribution prevalence (new or niche tools)
- Heuristic / ML-based detection engines
- Command-line behavior such as file processing and native library loading

### Important facts

- `opencc-jieba` is built using the **official Rust stable toolchain**
- No packers, obfuscates, or self-modifying code are used
- No network access, persistence, or privilege escalation behavior exists
- The source code is fully open and auditable

If your antivirus flags the binary:

- Verify the checksum against the GitHub Release
- Add an exclusion for the executable if necessary
- Or build from source using `cargo build --release -p opencc-jieba`

As the project gains adoption and reputation, these false positives typically disappear automatically.

> ⚠️ This is a detection heuristic issue, not an indication of malicious behavior.

---

## License

This project is licensed under the **MIT License**.

---

## Acknowledgements

- OpenCC project
- Jieba project
- PDFium project
- opencc-jieba-rs project
