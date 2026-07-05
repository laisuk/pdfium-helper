# Changelog

All notable changes to this project will be documented in this file.

This project adheres to [Semantic Versioning](https://semver.org/).

---

[0.1.6] - Unreleased

### Changed

- Update dictionary data.
- Update CLI tools deps.
- Optimized Reflow helper for handling unclosed dialog quote in standalone finalizer.
- Reflow: Allow commas in title headings when they appear within the first 20 characters.
- Reflow: Fixed reflow stitching when a dialog closer appears on its own line after sentence-ending punctuation.

---

## [0.1.5] - 2026-06-29

### Added

- Added `opencc-rs --custom-dict`

### Changed

- Update release workflow for `opencc-rs` v0.11.2 and `opencc-jieba` v0.8.0
- Optimized `opencc-rs` and `opencc-jieba` subcommand `office`
- Optimized `Reflow Helper` for completed standalone finalizer

---

## [0.1.4] - 2026-05-25

### Changed

- Update release workflow for `opencc-rs` v0.10.0 and `opencc-jieba` v0.7.6

---

## [0.1.3] - 2026-05-02

### Added

- Added CLI tool `opencc-jieba`

### Changed

- Update release workflow for `opencc-rs` v0.9.2 and `opencc-jieba` v0.7.5

---

## [0.1.2] - 2026-04-25

### Added

- Added `USER_GUIDE.md`, a practical Rust-facing user manual covering Pdfium loading, extraction, reflow, error
  handling,
  and common integration patterns.
- Added `PdfiumExtractError::pretty()` for richer caller-controlled CLI error rendering without requiring the library to
  print directly to stderr.
- Added `compute-pdfium-hash.ps1` to generate a root `VERSION` manifest with `version=...` plus SHA-256 entries for all
  bundled Pdfium native binaries.

### Changed

- Replaced the old public `print_error()` helper with the new `PdfiumExtractError::pretty()` display adapter so the
  library exposes formatted error information without owning console output.
- Updated `README.md` to link to the new user guide and corrected the `Typical usage` Rust example to be a complete,
  valid snippet.
- Revised `USER_GUIDE.md` Rust code samples so editor tooling can treat them as self-contained examples instead of
  incomplete fragments.
- Updated Pdfium natives to version `PDFium 148.0.7776.0`.
- Refactored `opencc-rs` Pdfium version reporting to read an explicit `version=...` manifest entry and only display the
  bundled Pdfium version when the loaded native matches the manifest SHA-256.
- `opencc-rs` now prefers a root `VERSION` manifest for portable distributions, still falls back to `pdfium/VERSION`,
  and suppresses version display for successful custom `--pdfium` loads.
- Normalized Pdfium missing-library diagnostics to use forward-slash display paths for cleaner cross-platform CLI
  messages.
- Aligned `.github/workflows/release-opencc-rs.yml` more closely with the `opencc-fmmseg` release workflow, including
  the expanded runner matrix, manylinux builds, Win7 variants, prerelease handling, and shipping `VERSION` in the
  release artifact root.
- Removed duplicated Pdfium platform-folder logic in `opencc-rs` by reusing the shared loader implementation.

### Fixed

- Fixed the Linux arm64 embedded Pdfium path/configuration type mismatch that caused build issues on that target.

---

## Unreleased - 2026-04-02

### Fixed

- Fixed XLSX conversion to also process worksheet inline strings (`t="inlineStr"`), preventing missed text conversion in
  hybrid workbooks that contain both shared strings and inline strings

### Added

* Added `PdfiumLibrary::global_with_fallbacks()` for process-global native loading with a stable shared handle and
  resolved path.
* Added regression coverage for:
    * `CHUNK_ABC.pdf` extraction output
    * `CHUNK_ABC` reflow output
    * repeated extraction through the shared loader
    * callback `page/total` progress reporting
    * a real repeated-phrase collapse case in CJK reflow
* Added focused unit tests for extraction normalization behavior:
    * preserving leading indentation when Pdfium returns it
    * preserving blank-page markers

### Changed

* Embedded `pdfium-embed` loading now prefers the cached extracted native and only decompresses on demand when needed.
* Standardized lazy initialization on the Rust standard library and removed the `once_cell` dependency.
* PDF extraction now reuses the UTF-16 page buffer across pages and avoids redundant page-text normalization work.
* CJK reflow now skips the expensive repeated-token collapse path for most ordinary tokens via a conservative precheck.
* `collapse_repeated_segments()` now has a cheaper fast path for common single-token lines.

### Notes

* Raw extraction intentionally follows Pdfium flat text behavior. It preserves Pdfium-returned line breaks and
  wrapper-added blank-page markers, but it does not reconstruct visual paragraph gaps that Pdfium itself flattens within
  a page.
* Process-global loading improves native library lifetime stability, but applications should still serialize overlapping
  extraction jobs if concurrent Pdfium use is possible.


