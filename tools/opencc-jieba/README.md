# opencc-jieba

**opencc-jieba** is a fast, cross-platform command-line tool for
converting Chinese text between Simplified and Traditional variants
using **OpenCC lexicons**, with additional **Jieba** word segmentation
support.

It supports:

- Plain text conversion
- Chinese word segmentation
- Office documents (`.docx`, `.odt`, `.epub`, etc.)
- PDF text extraction via **PDFium**
- CJK paragraph reflow optimized for novels and ebooks

It is designed to be **portable**, **dependency-light**, and **easy to
use** for both developers and end users.

------------------------------------------------------------------------

## Features

- 🚀 High-performance OpenCC conversion (`opencc-jieba-rs` Rust
  backend)
- ✂️ Jieba Chinese word segmentation and tagging
- 📄 Convert plain text files
- 📦 Convert Office / EPUB documents
- 📕 Extract and convert text-embedded PDFs (**PDFium backend**)
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

``` text
opencc-jieba(.exe)
pdfium.dll / libpdfium.so / libpdfium.dylib
VERSION
README.md
```

Keep the native library in the same directory as `opencc-jieba` unless
you explicitly load Pdfium from another location with `--pdfium`.

------------------------------------------------------------------------

## Pdfium runtime note

`opencc-jieba` tries to load Pdfium in this order:

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

When `opencc-jieba` loads bundled or default-discovered Pdfium, it may
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
`opencc-jieba` silently falls back to printing the loaded path only.

If you pass `--pdfium`, version display is intentionally suppressed for
that successful custom load.

------------------------------------------------------------------------

## Usage

``` text
opencc-jieba <command> [options]
```

Available subcommands:

- `convert` -- convert plain text with OpenCC
- `segment` -- segment or tag Chinese text with Jieba
- `office` -- convert Office / EPUB documents
- `pdf` -- extract PDF text, optionally reflow it, and optionally
  convert it

Use `opencc-jieba <command> -h` for the authoritative option list for
your build.

------------------------------------------------------------------------

## Supported conversion configurations

`convert`, `office`, and converted PDF output use the configuration set
exposed by `OpenccConfig::ALL`:

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

The CLI derives the accepted names from the library's
`OpenccConfig::ALL`, keeping command-line validation and help aligned
with `opencc-jieba-rs`.

------------------------------------------------------------------------

## Conversion options

`convert`, `office`, and `pdf` share the OpenCC options below. `convert`
and `office` require `--config`; `pdf` requires it unless `--extract` is
used.

  --------------------------------------------------------------------------
Option Description
  -------------------------------------- -----------------------------------
`-i, --input <file>`                   Input file. `convert` can read
stdin when omitted.

`-o, --output <file>`                  Output file. `convert` can write
stdout when omitted.

`-c, --config <config>`                Select one of the supported
conversion configurations above.

`-p, --punct`                          Enable punctuation conversion.

`--detofu`                             Apply cumulative DeTofu fallback
for CJK Extension B--I characters
after conversion.

`-D, --custom-dict <SLOT:MODE:FILE>`   Apply a custom OpenCC conversion
dictionary; may be repeated.

`-U, --user-dict-file <FILE>`          Load a Jieba user dictionary; may
be repeated.
  --------------------------------------------------------------------------

`convert` and `office` also expose normalization options:

  -----------------------------------------------------------------------
Option Description
  ----------------------------------- -----------------------------------
`-n, --norm-compat`                 Normalize CJK Compatibility
Ideographs before processing.

`-E, --norm-compat-extended`        Normalize extended Unicode
compatibility forms before
processing.
  -----------------------------------------------------------------------

These normalization modes are mutually exclusive. The `segment` command
exposes the same normalization pair.

`-U` and `-D` serve different purposes: Jieba user dictionaries affect
tokenization, while custom OpenCC dictionaries affect conversion
mappings.

------------------------------------------------------------------------

## Plain text conversion

``` text
opencc-jieba convert -i input.txt -o output.txt -c s2t
```

Seal conversion:

``` text
opencc-jieba convert -i input.txt -o output.txt -c s2seal
```

`convert` supports configurable text encodings:

Option Description
  ------------------------ --------------------------------------
`--in-enc <encoding>`    Input encoding; default is `UTF-8`.
`--out-enc <encoding>`   Output encoding; default is `UTF-8`.

Accepted encoding names are validated by the shared encoding layer; the
CLI advertises UTF-8, GB2312, GBK, GB18030, and BIG5.

When `-i` is omitted, `convert` reads stdin. When `-o` is omitted, it
writes stdout.

------------------------------------------------------------------------

## Jieba segmentation

The `segment` subcommand performs Jieba tokenization or part-of-speech
tagging without OpenCC conversion.

``` text
opencc-jieba segment -i input.txt -o segmented.txt
```

### Segmentation options

  -----------------------------------------------------------------------
Option Description
  ----------------------------------- -----------------------------------
`-m, --mode <mode>`                 `cut`, `search`, `all`, or `tag`;
default is `cut`.

`-d, --delim <character>`           Delimiter between output tokens;
default is a space.

`-s, --separator <character>`       Word/tag separator for `tag` mode;
default is `/`.

`--no-hmm`                          Disable HMM for segmentation and
tagging.

`-U, --user-dict-file <FILE>`       Load a Jieba user dictionary; may
be repeated.

`-n, --norm-compat`                 Normalize CJK Compatibility
Ideographs before segmentation.

`-E, --norm-compat-extended`        Normalize extended compatibility
forms before segmentation.

`--in-enc <encoding>`               Input encoding; default is `UTF-8`.

`--out-enc <encoding>`              Output encoding; default is
`UTF-8`.
  -----------------------------------------------------------------------

Examples:

``` text
echo "南京市长江大桥" | opencc-jieba segment
opencc-jieba segment -i input.txt --mode search --delim " "
opencc-jieba segment -i input.txt --mode all
opencc-jieba segment -i input.txt --mode tag --delim " " --separator ":"
opencc-jieba segment -i input.txt --no-hmm
opencc-jieba segment -i input-big5.txt -o output-utf8.txt --in-enc BIG5 --out-enc UTF-8
```

Interactive console input is line-ending normalized before segmentation.

------------------------------------------------------------------------

## Custom dictionaries

There are two independent dictionary mechanisms.

### Jieba user dictionaries

Use repeatable `-U/--user-dict-file` options to extend Jieba
tokenization:

``` text
opencc-jieba segment -U words.txt -U names.txt -i input.txt
```

For conversion commands, Jieba user dictionaries are loaded before
custom OpenCC dictionaries.

### OpenCC conversion dictionaries

Use:

``` text
-D SLOT:MODE:FILE
```

where `MODE` is `append` or `override`. Slot names are ASCII
case-insensitive, and `-D` may be repeated.

Examples:

``` text
-D STPhrases:append:my_st_dict.txt
-D TSPhrases:override:my_ts_dict.txt
-D SealVariantsRev:append:my_seal_reverse.txt
```

The argument is split only after the slot and mode, so Windows
drive-letter paths remain usable.

------------------------------------------------------------------------

## Office / EPUB conversion

``` text
opencc-jieba office -i book.epub -o book_converted.epub -c s2t
```

The format override accepts `docx`, `xlsx`, `pptx`, `odt`, `ods`, `odp`,
and `epub`.

Additional `office` options:

  -----------------------------------------------------------------------
Option Description
  ----------------------------------- -----------------------------------
`-f, --format <ext>`                Force the Office/EPUB document
format.

`-k, --keep-font`                   Preserve original font styles.

`-F, --convert-filename`            Convert the generated output
filename using the selected OpenCC
configuration.
  -----------------------------------------------------------------------

Example:

``` text
opencc-jieba office -i book.epub -c s2t -p -F -E -D STPhrases:append:my_st_dict.txt
```

Document content and automatically generated filename stems use the same
configured conversion pipeline.

------------------------------------------------------------------------

## PDF extraction and conversion

Convert a text-embedded PDF and reflow CJK paragraphs:

``` text
opencc-jieba pdf -i book.pdf -c s2t -p -r
```

Extract text only:

``` text
opencc-jieba pdf -i book.pdf -e
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

When conversion is requested, the configured OpenCC pipeline is applied
after extraction and optional reflow.

------------------------------------------------------------------------

## Text conversion pipeline

`convert`, `office`, and converted PDF output use one configured
pipeline:

1. Optional CJK compatibility normalization.
2. OpenCC conversion using the selected configuration, punctuation
   setting, and custom conversion dictionaries.
3. Optional DeTofu fallback.

Office/EPUB content and automatically generated filename stems (`-F`)
use this same pipeline. PDF conversion applies it after extraction and
optional reflow. `pdf --extract` skips conversion entirely.

`--detofu` is a boolean option in `opencc-jieba`: when enabled, it
applies cumulative fallback for CJK Extension B--I characters (`DetofuLevel::ExtB`). It does not take a level argument.

``` text
opencc-jieba convert -c t2s -i input.txt -o output.txt --detofu
opencc-jieba office -c s2t -i book.epub -F -E --detofu
opencc-jieba pdf -c s2t -i book.pdf -r --detofu
```

`segment` does not expose `--detofu`; its normalization is performed
independently before Jieba segmentation.

------------------------------------------------------------------------

## Progress display note

When running via `cargo run`, stdout may be buffered and intermediate
progress updates may not be visible.

For best progress display, run the compiled binary directly:

``` text
target/release/opencc-jieba pdf -i book.pdf -c s2t -p -r
```

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

### DeTofu

Add boolean `--detofu` to `convert`, `office`, or `pdf` to apply
cumulative fallback for CJK Extension B-I characters after conversion (`DetofuLevel::ExtB`). It is disabled by default
and takes no level
argument.

``` bash
opencc-jieba convert -c t2s -i input.txt -o output.txt --detofu
opencc-jieba office -c s2t -i book.epub -F -E --detofu
opencc-jieba pdf -c s2t -i book.pdf -r --detofu
```

`segment` does not expose `--detofu`: it retains independent
normalization before Jieba segmentation. Repeatable `-U` user
dictionaries still configure Jieba tokenization, while `-D` dictionaries
configure conversion mappings.

------------------------------------------------------------------------

## Development

``` text
cargo build --release -p opencc-jieba
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

Some antivirus products may report **false positives** for
`opencc-jieba`, especially on Windows.

This is a known issue affecting many **Rust-based CLI tools**, and is
typically caused by a combination of:

- Statically linked or highly optimized Rust binaries
- Low distribution prevalence (new or niche tools)
- Heuristic / ML-based detection engines
- Command-line behavior such as file processing and native library
  loading

### Important facts

- `opencc-jieba` is built using the **official Rust stable toolchain**
- No packers, obfuscates, or self-modifying code are used
- No network access, persistence, or privilege escalation behavior
  exists
- The source code is fully open and auditable

If your antivirus flags the binary:

- Verify the checksum against the GitHub Release
- Add an exclusion for the executable if necessary
- Or build from source using `cargo build --release -p opencc-jieba`

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
- Jieba project
- PDFium project
- opencc-jieba-rs project
