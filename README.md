# pdfium-helper

**pdfium-helper** is a lightweight Rust helper library that provides safe, ergonomic access to **PDFium**
for **PDF text extraction** and **CJK-aware paragraph reflow**, designed to be embedded in CLI tools
such as `opencc-rs`.

This crate focuses strictly on **text extraction** and intentionally avoids rendering,
OCR, or document editing features.

---

## Scope and design goals

- Extract text from **text-embedded PDFs**
- Page-by-page extraction with callback support
- Deterministic native library loading
- High-quality CJK paragraph reflow (novels / ebooks)
- No OCR
- No PDF rendering
- No image-based PDF support

The goal is to remain small, predictable, and easy to integrate.

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

Each module has a single responsibility.

---

## Native library loading

Native PDFium libraries are **not bundled**.

All native loading logic is implemented in `pdfium_loader.rs`.

### Supported layouts

Two layouts are supported at each search location:

#### 1) Single-library layout (recommended)

```
<dir>/pdfium.dll        (Windows)
<dir>/libpdfium.so     (Linux)
<dir>/libpdfium.dylib  (macOS)
```

This layout is ideal for portable CLI tools where the executable and the Pdfium
library are placed side-by-side.

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

The loader attempts the following locations in order:

1. `PDFIUM_LIB_DIR` environment variable
2. Directory containing the current executable
3. Current working directory
4. `CARGO_MANIFEST_DIR` (development fallback)

For each location:

- the single-library layout is tried first
- the bundled layout is tried second

The first successfully loaded library is used, and its path is returned for logging
and diagnostics.

---

## Pdfium binaries

Prebuilt Pdfium native libraries can be obtained from the
**pdfium-binaries** GitHub repository:

> Pdfium native binaries can be obtained from  
> https://github.com/bblanchon/pdfium-binaries

This repository provides ready-to-use Pdfium binaries for common platforms
(Windows, Linux, macOS).

You may download either:

- A **single native library** (`pdfium.dll`, `libpdfium.so`, `libpdfium.dylib`)
  and place it next to the executable, or
- The entire `pdfium/` directory structure and place it next to the executable

`opencc-rs` / `pdfium-helper` will automatically detect and load the correct
library at runtime.

---

## PDF text extraction

PDF text extraction is implemented in `pdfium_text.rs`.

### Characteristics

- Page-by-page extraction
- UTF-16 to UTF-8 decoding handled internally
- Callback-based API for progress reporting
- No layout reconstruction beyond line-level extraction

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

This design allows progress reporting, early cancellation, and memory-efficient streaming.

---

## CJK paragraph reflow

CJK paragraph reflow is implemented in `reflow_helper.rs`.

### Purpose

PDF text extraction often produces fragmented lines, especially for:

- Chinese novels
- Japanese ebooks
- Dialogue-heavy content

The reflow helper merges broken lines, detects paragraph boundaries,
preserves dialog punctuation, and optionally inserts page headers
or compacts whitespace.

### What reflow does NOT do

- It does not infer semantic structure
- It does not perform language detection
- It does not modify wording or meaning
- It does not reorder content

Reflow is a best-effort formatting pass, not a semantic transformation.

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

Downstream tools are responsible for encoding conversion, dictionary-based
transformations, and output formatting.

---

## Development notes

- Built with the official **Rust stable toolchain**
- No runtime code generation or extraction
- No global state or hidden caching

---

## Platform support

- Windows (x64)
- Linux (x64)
- macOS (Intel / Apple Silicon)

Support depends on the availability of compatible PDFium native libraries.

---

## License

This project is licensed under the **MIT License**.

---

## Intended users

- CLI tool authors
- Ebook / novel processing pipelines
- Developers needing reliable PDF text extraction
- Projects embedding PDFium without full rendering stacks
