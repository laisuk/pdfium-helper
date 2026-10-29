# opencc-office-converter

Shared ZIP-to-ZIP Office/EPUB converter for the workspace CLI tools. Supports
DOCX, XLSX, PPTX, ODT, ODS, ODP, and EPUB, with a configured `TextConverter`
or `Fn(&str) -> String` closure. Requires Rust 1.75.0.

```rust,no_run
use opencc_office_converter::OfficeConverter;
# fn main() -> std::io::Result<()> {
let convert_text = |text: &str| text.replace("汉语", "漢語");
OfficeConverter::convert_path_stream(
    "input.docx", "output.docx", "docx", &convert_text, true,
)?;
# Ok(())
# }
```

`convert_bytes(input, format, converter, keep_font)` returns rebuilt bytes and
the converted-entry count. `convert_path_stream` writes a sibling temporary file,
validates it, then replaces the destination; failures preserve the destination
and clean up the temporary file. Replacement is atomic on Unix, with no atomicity
guarantee on Windows. Only target XML/XHTML is decoded; other assets are raw-copied.
EPUB `mimetype` is emitted first and Stored. Conversion and font policies are unchanged.

## ZIP implementation and dependency tree

ZIP handling is a private vendored adaptation of zip 3.0.0 from the local
`opencc-fmmseg/tools/opencc-rs` Office converter. There is no external `zip`
package or ZIP dev dependency. The library and integration harness use the same
private implementation. Support crates are direct dependencies of this crate:

```text
opencc-office-converter
├── regex 1.13
└── private src/zip module (adapted zip 3.0.0)
    ├── crc32fast 1.5
    ├── flate2 =1.1.1 [rust_backend, no default features]
    │   ├── crc32fast
    │   └── miniz_oxide → adler2
    ├── indexmap =2.7.1 → equivalent, hashbrown
    └── memchr 2.7
```

The exact flate2/indexmap pins match the supplied source and preserve Rust 1.75
compatibility. Zopfli and zlib-rs are removed from the converter's dependency
tree. Other workspace consumers may still require shared transitive crates.
DEFLATE bytes may differ from earlier builds while decoding to the same content.
Inspect the resolved tree with `cargo tree -p opencc-office-converter --offline`.

Both archive constructors require consistent local and central headers before
conversion or raw-copy. Checks cover raw filenames, UnicodePath identity,
compression methods, selected flags, CRC/sizes, duplicate effective names,
checked offsets, central-directory boundaries, and overlapping indexed entries.
Malformed headers reject the entire conversion. Portable unsafe paths are skipped,
preserving this crate's prior policy; filtering now also covers drive-relative
paths, alternate data streams, and embedded NUL bytes. Rebuilt archives are
reopened through the same validator before return or destination replacement.

See [vendoring notes](src/zip/VENDOR_NOTES.md) for provenance, license, limitations,
and maintenance. This is scoped header hardening, not a general ZIP security audit.
Raw-copy does not decode payloads to verify CRCs, and archive resource quotas are
not imposed. Vendoring requires continued monitoring of upstream ZIP advisories.

## Checks

```text
cargo test -p opencc-office-converter --offline
cargo fmt -p opencc-office-converter -- --check
cargo clippy -p opencc-office-converter --all-targets --offline -- -D warnings
```
