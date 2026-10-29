# Vendored ZIP integration

Synced on 2026-10-30 from
`D:\codes\Rust\opencc-fmmseg\tools\opencc-rs\src\office_converter\zip`,
source commit `87afddd02ecd70c2ebdb4699de4d1ebb87b1182c` (clean source tree).
This is the same trimmed and hardened ZIP 3.0.0 implementation used in the
opencc-fmmseg-wasm port, integrated here directly from the Rust 2021 source
to preserve this crate's Rust 1.75.0 support.

The MIT license is preserved verbatim in [LICENSE](LICENSE). Original port and
security notes are retained in [UPSTREAM_VENDOR_NOTES.md](UPSTREAM_VENDOR_NOTES.md)
and [SECURITY_ADVISORY.md](SECURITY_ADVISORY.md). Their relative source paths and
verification results refer to the original repositories, not this port.

## Local changes

- Rebased internal paths from `crate::office_converter::zip` to `crate::zip`.
- Declared private `mod zip` and used it in library and unit-test imports.
- Removed the external ZIP dependency; added crc32fast 1.5, memchr 2.7,
  indexmap =2.7.1, and flate2 =1.1.1 with only rust_backend.
- The existing public-API integration harness includes the same private module
  by path; it adds no external ZIP dev dependency and exposes no public ZIP API.
- Reused the portable unsafe-path predicate while retaining this crate's skip
  policy. Invalid archive headers fail before reconstruction is published.
- Kept all eight source ZIP regression tests and adapted three converter security
  regressions to the configured closure API and argument order. Existing package
  selection, fonts, raw assets, and output lifecycle behavior remain intact.
- Marked ten upstream crate-only examples as `rust,ignore`: they reference the external ZIP crate and private APIs. The public converter doctest remains active.
- Applied this crate's Rust 1.75 rustfmt layout; no Rust 2024-specific edits from
  the WASM port are needed here.

## Scope and maintenance

Mandatory archive validation checks local/central raw names, UnicodePath fields,
selected flags, compression method, CRC/sizes, checked offsets, duplicate names,
directory boundaries, and indexed-entry overlaps. Both `new` and `with_config`
use it. Rebuilt output is reopened before return or destination replacement.
Decoded reads retain CRC checking; raw-copy does not decode payloads to check
CRCs, and this crate's existing output validation only reopens archive metadata.

Validation does not reconcile all parser interpretations, unindexed local headers,
data-descriptor trailer contents, timestamp/permission parity, concurrent input
mutation, or archive resource quotas. Some ambiguous archives previously accepted
are intentionally rejected. Native Windows destination replacement remains
non-atomic. See the source notes for further limitations.

The private ZIP source is not automatically covered by external zip dependency
alerts. Track upstream/RustSec advisories, assess fixes against retained paths,
keep attribution and notes current, and run Rust 1.75 tests plus workspace checks
on updates. Do not close an advisory just because zip vanished from Cargo.lock.


## Port verification (2026-10-30, Windows x86_64 MSVC, Rust 1.75.0)

- `cargo test -p opencc-office-converter --offline`: PASS, 32 unit tests,
  9 integration tests, and the active public API doctest. Ten upstream private
  ZIP reference examples are explicitly ignored as described above.
- `cargo test --workspace --offline`: PASS, including both CLI tools,
  opencc-utils consumers, PDF regression tests, and workspace doctests.
- `cargo clippy -p opencc-office-converter --all-targets --offline -- -D warnings`: PASS.
- `cargo fmt -p opencc-office-converter -- --check` and `git diff --check`: PASS.
- `cargo doc -p opencc-office-converter --no-deps --offline`: PASS.
- `cargo tree -p opencc-office-converter --offline`: no external zip, Zopfli,
  or zlib-rs; those packages are also absent from the workspace lockfile.
- Original ZIP license SHA-256 matches the supplied source exactly.

No cross-platform runtime or large real-world Office/EPUB corpus testing was run.
