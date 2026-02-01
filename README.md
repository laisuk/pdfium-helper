# pdfium-helper

**pdfium-helper** is an **internal Rust helper crate** that provides safe, ergonomic access to **PDFium**
for **PDF text extraction** and **CJK-aware paragraph reflow**.

It is designed primarily to support **OpenCC tooling** (such as the `opencc-rs` CLI and related
bindings), and is **not intended to be a general-purpose PDF library**.

This repository is public for **transparency and auditability**.

---

## Purpose and scope

This crate exists to support a very specific workflow:

* Extract readable text from **text-embedded PDFs**
* Reflow fragmented CJK text into readable paragraphs
* Feed the result into **OpenCC-based conversion pipelines**

It intentionally avoids broader PDF concerns such as rendering, OCR, or editing.

---

## Design goals

* Reliable text extraction from **text-based PDFs**
* Page-by-page extraction with callback support
* Deterministic native PDFium loading
* High-quality CJK paragraph reflow (novels / ebooks)
* Small, predictable API surface
* Easy embedding into CLI tools

### Explicit non-goals

* ❌ OCR
* ❌ PDF rendering
* ❌ Annotation or editing
* ❌ Image-based PDF support
* ❌ Stable public API guarantees

---

## Important limitation

Only **text-embedded PDFs** are supported.

Scanned PDFs (image-only, OCR-required, or DRM-protected image pages) will not produce usable output.
If PDFium cannot extract text from a page, the extracted text for that page will be empty.

---

## Crate structure

```
src/
├── pdfium_loader.rs   # Native library discovery & loading
├── pdfium_text.rs     # PDF text extraction (page-based)
├── reflow_helper.rs   # CJK paragraph reflow logic
└── lib.rs
```

Each module has a **single, well-defined responsibility**.

---

## Native PDFium loading

Native PDFium libraries are **not bundled** with this crate.

All native discovery and loading logic is implemented in `pdfium_loader.rs`.

### Supported layouts

Two layouts are supported at each search location:

#### 1) Single-library layout (recommended)

```
<dir>/pdfium.dll        (Windows)
<dir>/libpdfium.so     (Linux)
<dir>/libpdfium.dylib  (macOS)
```

This layout is ideal for portable CLI tools where the executable and the
PDFium library are placed side-by-side.

#### 2) Bundled layout (multi-platform)

```
<dir>/pdfium/<platform>/<library>
```

Example:

```
pdfium/
  win-x64/pdfium.dll
  linux-x64/libpdfium.so
  osx-arm64/libpdfium.dylib
```

This layout allows shipping multiple platforms together.

---

### Load order

The loader attempts the following locations **in order**:

1. `PDFIUM_LIB_DIR` environment variable
2. Directory containing the current executable
3. Current working directory
4. `CARGO_MANIFEST_DIR` (development fallback)

For each location:

* the single-library layout is tried first
* the bundled layout is tried second

The first successfully loaded library is used, and its resolved path
is returned for logging and diagnostics.

---

## PDFium binaries

Prebuilt PDFium native libraries can be obtained from the
**pdfium-binaries** project:

> [https://github.com/bblanchon/pdfium-binaries](https://github.com/bblanchon/pdfium-binaries)

This repository provides ready-to-use PDFium binaries for common platforms
(Windows, Linux, macOS).

You may supply either:

* A **single native library** (`pdfium.dll`, `libpdfium.so`, `libpdfium.dylib`)
  placed next to the executable, or
* A bundled `pdfium/` directory containing multiple platforms

`pdfium-helper` (and OpenCC tooling using it) will automatically detect
and load the correct library at runtime.

---

## PDF text extraction

PDF text extraction is implemented in `pdfium_text.rs`.

### Characteristics

* Page-by-page extraction
* UTF-16 → UTF-8 decoding handled internally
* Callback-based API for progress reporting
* No layout reconstruction beyond line-level extraction

### Typical usage

```
extract_pdf_pages_with_callback_pdfium(
    &pdfium,
    input_path,
    |page, total, text| {
        // page-based callback
    }
)?;
```

This design enables progress reporting, early cancellation,
and memory-efficient streaming.

---

## CJK paragraph reflow

CJK paragraph reflow is implemented in `reflow_helper.rs`.

### Purpose

PDF text extraction often produces fragmented lines, especially for:

* Chinese novels
* Japanese ebooks
* Dialogue-heavy content

The reflow helper merges broken lines, detects paragraph boundaries,
preserves dialogue punctuation, and optionally inserts page headers
or compacts whitespace.

### What reflow does NOT do

* It does not infer semantic structure
* It does not perform language detection
* It does not modify wording or meaning
* It does not reorder content

Reflow is a **best-effort formatting pass**, not a semantic transformation.

---

## Typical integration flow

```
PDFium load
   ↓
Page-by-page text extraction
   ↓
Optional CJK paragraph reflow
   ↓
Consumer tool (OpenCC, search, export, etc.)
```

Downstream tools are responsible for encoding conversion,
dictionary-based transformations, and output formatting.

---

## Development notes

* Built with the official **Rust stable toolchain**
* No runtime code generation or extraction
* No global state or hidden caching

---

## Platform support

* Windows (x64)
* Linux (x64)
* macOS (Intel / Apple Silicon)

Platform support depends on the availability of compatible
PDFium native libraries.

---

## License

This project is licensed under the **MIT License**.

---

## Intended audience

This crate is intended for:

* OpenCC-related tooling
* CLI tool authors embedding PDFium
* Ebook / novel processing pipelines
* Developers studying CJK PDF text extraction and reflow

It is **not** intended as a general-purpose PDF library.
