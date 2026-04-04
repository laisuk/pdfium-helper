# Changelog

All notable changes to this project will be documented in this file.

This project adheres to [Semantic Versioning](https://semver.org/).

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
