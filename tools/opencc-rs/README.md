# opencc-rs

**opencc-rs** is a fast, cross-platform command-line tool for converting
Chinese text between Simplified and Traditional variants using **OpenCC
lexicons**, with advanced support for:

- Plain text conversion
- Office documents (`.docx`, `.odt`, `.epub`, etc.)
- PDF text extraction via **PDFium**
- CJK paragraph reflow optimized for novels and ebooks

It is designed to be **portable**, **dependency-light**, and **easy to
use** for both developers and end users.

------------------------------------------------------------------------

## Features

- 🚀 High-performance OpenCC conversion (`opencc-fmmseg` Rust backend)
- 📄 Convert plain text files
- 📦 Convert Office / EPUB documents
- 📕 Extract and convert text-embedded PDFs (**bundled PDFium
  backend**)
- 🧠 CJK-aware paragraph reflow (novels / ebooks friendly)
- 📊 Live page-by-page PDF progress display
- 🧳 Portable: no installer, no system dependencies

> ⚠️ **Note**
>
> Only **text-embedded PDFs** are supported. Scanned / image-only PDFs
> are not supported.

------------------------------------------------------------------------

## Installation

### Option 1: Download prebuilt binaries (recommended)

1. Go to **GitHub Releases**
2. Download the appropriate package for your platform
3. Extract the archive

You can now run `opencc-rs` directly.

> ✅ **Out-of-the-box ready**
>
> Each release package includes:
>
> - `opencc-rs`
> - the matching platform Pdfium native library
> - `README.md`
> - `VERSION` manifest for verified bundled-version reporting

### Portable runtime layout

Current release packages use a flat portable layout:

``` text
opencc-rs(.exe)
pdfium.dll / libpdfium.so / libpdfium.dylib
VERSION
README.md
```

Keep the native library in the same directory as `opencc-rs` unless you
explicitly load Pdfium from another location with `--pdfium`.

------------------------------------------------------------------------

## Pdfium runtime note

`opencc-rs` tries to load Pdfium in this order:

1. The custom path or base directory passed via `--pdfium`
2. The default `pdfium-helper` fallback search:

- directory containing the current executable
- current working directory
- `CARGO_MANIFEST_DIR` during development
- embedded Pdfium fallback when built with the `pdfium-embed`
  feature

For each search location, the loader tries:

- a side-by-side native library in the same directory as the
  executable
- then a bundled `pdfium/<platform>/...` layout

### Version display behavior

When `opencc-rs` loads bundled or default-discovered Pdfium, it may
print a verified version line such as:

``` text
Loaded pdfium: R:/PortableApps/pdfium.dll (version: 148.0.7776.0)
```

This only happens when:

- a `VERSION` manifest is present at the portable root or under
  `pdfium/VERSION`
- the manifest contains `version=...`
- the manifest contains a matching SHA-256 entry for the loaded native
  library

If the manifest is missing or does not match the native binary,
`opencc-rs` silently falls back to printing the loaded path only.

If you pass `--pdfium`, version display is intentionally suppressed for
that successful custom load.

------------------------------------------------------------------------

## Usage

``` text
opencc-rs <command> [options]
```

Available subcommands:

- `convert` -- convert plain text
- `office` -- convert Office / EPUB documents
- `pdf` -- extract PDF text, optionally reflow it, and optionally
  convert it

Use `opencc-rs <command> -h` for the authoritative option list for your
build.

------------------------------------------------------------------------

## Supported conversion configurations

All conversion commands use the same configuration set:

- `s2t` -- Simplified to Traditional
- `s2tw` -- Simplified to Traditional Taiwan
- `s2twp` -- Simplified to Traditional Taiwan with idioms
- `s2hk` -- Simplified to Traditional Hong Kong
- `s2hkp` -- Simplified to Traditional Hong Kong with idioms
- `t2s` -- Traditional to Simplified
- `t2tw` -- Traditional to Traditional Taiwan
- `t2twp` -- Traditional to Traditional Taiwan with idioms
- `t2hk` -- Traditional to Traditional Hong Kong
- `t2hkp` -- Traditional to Traditional Hong Kong with idioms
- `tw2s` -- Traditional Taiwan to Simplified
- `tw2sp` -- Traditional Taiwan to Simplified with idioms
- `tw2t` -- Traditional Taiwan to general Traditional
- `tw2tp` -- Traditional Taiwan with idioms to general Traditional
- `hk2s` -- Traditional Hong Kong to Simplified
- `hk2sp` -- Traditional Hong Kong to Simplified with idioms
- `hk2t` -- Traditional Hong Kong to general Traditional
- `hk2tp` -- Traditional Hong Kong with idioms to general Traditional
- `jp2t` -- Japanese Shinjitai to Traditional Chinese
- `t2jp` -- Traditional Chinese to Japanese Shinjitai
- `s2seal` -- Simplified Chinese to Seal script
- `t2seal` -- Traditional Chinese to Seal script
- `seal2s` -- Seal script to Simplified Chinese
- `seal2t` -- Seal script to Traditional Chinese

The accepted names are derived from `OpenccConfig::ALL`, keeping CLI
validation and help aligned with the conversion library.

------------------------------------------------------------------------

## Common conversion options

The three commands share the OpenCC conversion options below. `convert`
and `office` require `--config`; `pdf` requires it unless `--extract` is
used.

  --------------------------------------------------------------------------
Option Description
  -------------------------------------- -----------------------------------
`-i, --input <file>`                   Input file. Plain-text `convert`
can read stdin when omitted.

`-o, --output <file>`                  Output file. Plain-text `convert`
can write stdout when omitted.

`-c, --config <config>`                Select one of the supported
conversion configurations above.

`-p, --punct`                          Enable punctuation conversion.

`-n, --norm-compat`                    Normalize CJK Compatibility
Ideographs before conversion.

`-E, --norm-compat-extended`           Normalize extended Unicode
compatibility forms before
conversion.

`--detofu [<LEVEL>]`                   Apply tofu-safe fallback after
conversion. Omitting the level
selects `all`.

`--detofu-file <FILE>`                 Load additional UTF-8 DeTofu
mappings. Custom mappings override
built-ins; requires `--detofu`.

`-D, --custom-dict <SLOT:MODE:FILE>`   Apply a custom dictionary with
`append` or `override`; may be
repeated.
  --------------------------------------------------------------------------

`--norm-compat` and `--norm-compat-extended` are mutually exclusive.

Supported DeTofu levels are `all`, `ext-c`, `ext-d`, `ext-e`, `ext-f`,
`ext-g`, `ext-h`, and `ext-i`.

Custom dictionary example:

``` text
opencc-rs convert -i input.txt -o output.txt -c t2s -D tsphrases:append:my_ts_dict.txt
```

Slot names are parsed case-insensitively. The argument is split only
after the slot and mode, so Windows drive-letter paths remain usable.

------------------------------------------------------------------------

## Plain text conversion

Basic conversion:

``` text
opencc-rs convert -i input.txt -o output.txt -c s2t
```

Seal conversion:

``` text
opencc-rs convert -i input.txt -o output.txt -c s2seal
```

Additional `convert` options:

Option Description
  ------------------------ -----------------------------------------------------
`--keep-ids`             Preserve Unicode IDS expressions during conversion.
`--in-enc <encoding>`    Input encoding; default is `UTF-8`.
`--out-enc <encoding>`   Output encoding; default is `UTF-8`.

When `-i` is omitted, `convert` reads stdin. When `-o` is omitted, it
writes stdout.

------------------------------------------------------------------------

## Office / EPUB conversion

``` text
opencc-rs office -i book.epub -o book_converted.epub -c s2t
```

Supported formats include `.docx`, `.odt`, and `.epub`.

Additional `office` options:

  -----------------------------------------------------------------------
Option Description
  ----------------------------------- -----------------------------------
`-f, --format <ext>`                Force the document format, for
example `docx`, `odt`, or `epub`.

`-k, --keep-font`                   Preserve original font styles.

`-F, --convert-filename`            Convert the generated output
filename using the selected OpenCC
configuration.
  -----------------------------------------------------------------------

Example with punctuation conversion, filename conversion, and a custom
dictionary:

``` text
opencc-rs office -i book.epub -c s2t -p -F -D stphrases:append:my_st_dict.txt
```

Document content and automatically generated filename stems use the same
configured conversion pipeline.

------------------------------------------------------------------------

## PDF extraction and conversion

Convert a text-embedded PDF and reflow CJK paragraphs:

``` text
opencc-rs pdf -i book.pdf -c s2t -p -r
```

Extract text only:

``` text
opencc-rs pdf -i book.pdf -e
```

`--extract` does not require `--config` and does not construct an OpenCC
conversion engine.

Additional `pdf` options:

  -----------------------------------------------------------------------
Option Description
  ----------------------------------- -----------------------------------
`-r, --reflow`                      Reflow extracted PDF lines into CJK
paragraphs.

`-C, --compact`                     Compact reflow output by removing
extra blank lines/spaces.

`-H, --header`                      Add page headers such as
`=== [Page 3/120] ===`.

`-e, --extract`                     Extract PDF text only; skip OpenCC
conversion.

`--ignore-untrusted-text`           Ignore repeated untrusted PDF text
overlays during extraction.

`--pdfium <dir>`                    Use a custom PDFium native base
directory; invalid locations fall
back to normal lookup.
  -----------------------------------------------------------------------

When conversion is requested, normalization, OpenCC conversion, and
DeTofu are applied after extraction and optional reflow.

------------------------------------------------------------------------

## Text conversion pipeline

`convert`, `office`, and converted PDF output use one configured
pipeline:

1. Optional normalization with `-n/--norm-compat` or
   `-E/--norm-compat-extended`.
2. OpenCC conversion using the selected config, punctuation setting,
   and loaded custom dictionaries.
3. Optional DeTofu fallback.

Office/EPUB content and automatically generated filename stems (`-F`)
use this same pipeline. PDF conversion applies it after extraction and
optional reflow. `pdf --extract` skips conversion entirely.

`--detofu [LEVEL]` defaults to `all` when LEVEL is omitted.
`--detofu-file FILE` requires `--detofu` and loads custom mappings that
override built-in mappings. `convert --keep-ids` preserves IDS
expressions in the OpenCC conversion step.

``` text
opencc-rs office -c s2t -i book.epub -F -E --detofu
opencc-rs convert -c t2s -i input.txt -o output.txt --detofu ext-c --detofu-file fallback.txt
```

------------------------------------------------------------------------

## Progress display note

When running via `cargo run`, stdout may be buffered and intermediate
progress updates may not be visible.

For best progress display, run the compiled binary directly:

    target/release/opencc-rs pdf -i book.pdf -c s2t -p -r

------------------------------------------------------------------------

## Text conversion pipeline

`convert`, `office`, and `pdf` use one configured pipeline:

1. Optional normalization with `-n/--norm-compat` or
   `-E/--norm-compat-extended`.
2. OpenCC conversion using the selected config, punctuation setting,
   and loaded custom dictionaries.
3. Optional DeTofu fallback.

Office/EPUB content and automatically generated filename stems (`-F`)
use this same pipeline. PDF conversion applies it after extraction and
optional reflow. `pdf --extract` skips conversion entirely and does not
require `-c` or construct a conversion engine.

`--detofu [LEVEL]` retains its existing level parsing; omitting LEVEL
selects `all` (Extension B onward). Supported levels are `all`, `ext-c`,
`ext-d`, `ext-e`, `ext-f`, `ext-g`, `ext-h`, and `ext-i`.
`--detofu-file FILE` requires `--detofu` and loads custom mappings that
override built-in mappings. These options also apply to filename
conversion with `-F`. `convert --keep-ids` continues to preserve IDS
expressions in the OpenCC conversion step, and `-D` custom dictionaries
remain part of engine setup.

``` bash
opencc-rs office -c s2t -i book.epub -F -E --detofu
opencc-rs convert -c t2s -i input.txt -o output.txt --detofu ext-c --detofu-file fallback.txt
```

------------------------------------------------------------------------

## Development

``` text
cargo build --release
```

For development, Pdfium can be provided via:

- `--pdfium <dir>` or `--pdfium <file>`
- the executable directory
- the current working directory
- a bundled `pdfium/<platform>/` layout

If you build a portable distribution and want verified version
reporting, place a compatible `VERSION` manifest at the artifact root.

------------------------------------------------------------------------

## Supported platforms

- Windows (x64)
- Linux (x64)
- macOS (Intel / Apple Silicon)

------------------------------------------------------------------------

## Antivirus false-positive notice

Some antivirus products may report **false positives** for `opencc-rs`,
especially on Windows.

This is a known issue affecting many **Rust-based CLI tools**, and is
typically caused by a combination of:

- Statically linked or highly optimized Rust binaries
- Low distribution prevalence (new or niche tools)
- Heuristic / ML-based detection engines
- Command-line behavior such as file processing and native library
  loading

### Important facts

- `opencc-rs` is built using the **official Rust stable toolchain**
- No packers, obfuscates, or self-modifying code are used
- No network access, persistence, or privilege escalation behavior
  exists
- The source code is fully open and auditable

If your antivirus flags the binary:

- Verify the checksum against the GitHub Release
- Add an exclusion for the executable if necessary
- Or build from source using `cargo build --release`

As the project gains adoption and reputation, these false positives
typically disappear automatically.

> ⚠️ This is a detection heuristic issue, not an indication of malicious
> behavior.

------------------------------------------------------------------------

## License

This project is licensed under the **MIT License**.

------------------------------------------------------------------------

## Acknowledgements

- OpenCC project
- PDFium project
- opencc-fmmseg project
