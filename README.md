# pdfium-helper

**pdfium-helper** is an internal Rust helper crate that provides safe, ergonomic access to **PDFium** for **PDF text extraction** and **CJK-aware paragraph reflow**.

It is designed primarily to support **OpenCC tooling** (such as the `opencc-rs` CLI and related bindings), and is not intended to be a general-purpose PDF library.

This repository is public for transparency and auditability.

---

## Purpose and scope

This crate exists to support a very specific workflow:

* Extract readable text from text-embedded PDFs
* Reflow fragmented CJK text into readable paragraphs
* Feed the result into OpenCC-based conversion pipelines

It intentionally avoids broader PDF concerns such as rendering, OCR, or editing.

---

## Design goals

* Reliable text extraction from text-based PDFs
* Page-by-page extraction with callback support
* Deterministic native PDFium loading
* High-quality CJK paragraph reflow (novels / ebooks)
* Small, predictable API surface
* Easy embedding into CLI tools

### Explicit non-goals

* OCR
* PDF rendering
* Annotation or editing
* Image-based PDF support
* Stable public API guarantees

---

## Important limitation

Only text-embedded PDFs are supported.

Scanned PDFs (image-only, OCR-required, or DRM-protected image pages) will not produce usable output. If PDFium cannot extract text from a page, the extracted text for that page is emitted as a single blank-line marker (`"\n"`) so downstream consumers can still preserve page progress and page-boundary structure.

---

## Crate structure

```text
src/
├── pdfium_loader.rs   # Native library discovery & loading
├── pdfium_text.rs     # PDF text extraction (page-based)
├── reflow_helper.rs   # CJK paragraph reflow logic
└── lib.rs
```

Each module has a single, well-defined responsibility.

---

## Native PDFium loading

All native discovery and loading logic is implemented in `pdfium_loader.rs`.

### Supported layouts

Two layouts are supported at each search location.

#### 1) Single-library layout (recommended)

```text
<dir>/pdfium.dll        (Windows)
<dir>/libpdfium.so      (Linux)
<dir>/libpdfium.dylib   (macOS)
```

This layout is ideal for portable CLI tools where the executable and the PDFium library are placed side-by-side.

#### 2) Bundled layout (multi-platform)

```text
<dir>/pdfium/<platform>/<library>
```

Example:

```text
pdfium/
  win-x64/pdfium.dll
  linux-x64/libpdfium.so
  macos-arm64/libpdfium.dylib
```

This layout allows shipping multiple platforms together.

### Load order

The loader attempts the following locations in order:

1. `PDFIUM_LIB_DIR` environment variable
2. Directory containing the current executable
3. Current working directory
4. `CARGO_MANIFEST_DIR` (development fallback)

For each location:

* the single-library layout is tried first
* the bundled layout is tried second

The first successfully loaded library is used, and its resolved path is returned for logging and diagnostics.

### Process-global loading

For long-lived applications, `PdfiumLibrary::global_with_fallbacks()` caches the first successful native load for the lifetime of the process and returns a shared `&'static PdfiumLibrary` plus the resolved path.

This is useful for GUI backends and services because it avoids repeated native load/unload cycles. It stabilizes library lifetime, but it does **not** make Pdfium extraction automatically safe for concurrent use. If your application may run overlapping extraction jobs, serialize extraction at the app layer with a `Mutex` or similar guard.

---

## Embedded PDFium support (optional)

`pdfium-helper` supports an optional embedded-native mode, enabled via the `pdfium-embed` feature.

When this feature is enabled:

* A single platform-specific PDFium native library is embedded into the binary
* The embedded native is compressed with zstd at build time
* On first actual use, the library is decompressed and written to a cache directory
* Subsequent runs try to load the cached extracted library first and only decompress again if the cache is missing or no longer loadable

### Enabling embedded mode

In a downstream crate (for example, `opencc-rs`):

```toml
pdfium-helper = { path = "../pdfium-helper", features = ["pdfium-embed"] }
```

Or, if you want to re-export the feature:

```toml
[features]
pdfium-embed = ["pdfium-helper/pdfium-embed"]
```

Build with:

```bash
cargo build --release --features pdfium-embed
```

### Runtime behavior

* On first use (or after a PDFium version change), the embedded native is extracted to:
  * Windows: `%LOCALAPPDATA%/pdfium-helper/natives/`
  * Linux: `$XDG_CACHE_HOME/pdfium-helper/natives/` or `~/.cache/pdfium-helper/natives/`
  * macOS: `~/Library/Caches/pdfium-helper/natives/`
* The extracted filename is versioned, for example `pdfium-145.0.7616.0-win-x64.dll`
* Existing cached files are reused when they are already present and loadable

This design avoids DLL locking issues on Windows and keeps startup overhead minimal.

### Trade-offs

* Embedded mode performs one extraction write on first use per version
* Dynamic (non-embedded) mode performs no writes and loads the native directly

Users may choose the mode that best fits their deployment model.

### Note on cross-compilation

The `pdfium-embed` feature embeds a prebuilt Pdfium native library for the current target OS and architecture using `#[cfg(target_os, target_arch)]`.

This feature is primarily intended for native builds, where the build machine architecture matches the target architecture.

When cross-compiling, it is recommended to disable `pdfium-embed` and provide Pdfium externally instead. In such cases, `pdfium-helper` will load Pdfium dynamically using its fallback mechanisms.

---

## PDFium binaries (dynamic mode)

If not using embedded mode, prebuilt PDFium native libraries can be obtained from the **pdfium-binaries** project:

[https://github.com/bblanchon/pdfium-binaries](https://github.com/bblanchon/pdfium-binaries)

You may supply either:

* A single native library placed next to the executable
* A bundled `pdfium/` directory containing multiple platforms

`pdfium-helper` will automatically detect and load the correct library at runtime.

---

## PDF text extraction

PDF text extraction is implemented in `pdfium_text.rs`.

### Characteristics

* Page-by-page extraction
* UTF-16 → UTF-8 decoding handled internally
* Callback-based API for progress reporting
* Blank pages are emitted as a single blank-line marker (`"\n"`)
* No geometry-based layout reconstruction

### Extraction contract

The current extractor intentionally follows **Pdfium flat text behavior**:

* it preserves the line breaks that `FPDFText_GetText()` actually returns
* it preserves blank-page markers added by the wrapper
* it does **not** reconstruct visual paragraph spacing or indentation that Pdfium itself flattens within a page

In other words, this is a raw Pdfium-text layer, not a layout-faithful page reconstruction layer.

### Typical usage

```rust
extract_pdf_pages_with_callback_pdfium(
    &pdfium,
    input_path,
    false,
    |page, total, text| {
        // page-based callback for progress or streaming output
    },
)?;
```

This design enables progress reporting, early cancellation, and memory-efficient streaming.

---

## CJK paragraph reflow

CJK paragraph reflow is implemented in `reflow_helper.rs`.

### Purpose

PDF text extraction often produces fragmented lines, especially for:

* Chinese novels
* Japanese ebooks
* Dialogue-heavy content

The reflow helper merges broken lines, detects paragraph boundaries, preserves dialogue punctuation, and optionally inserts page headers or compacts whitespace.

### What reflow does NOT do

* It does not infer semantic structure
* It does not perform language detection
* It does not modify wording or meaning
* It does not reorder content

Reflow is a best-effort formatting pass, not a semantic transformation.

### Performance notes

The current implementation includes several low-risk hot-path optimizations:

* repeated-token collapse skips most ordinary tokens via a cheap precheck
* single-token lines avoid the heavier multi-token collapse pipeline
* the extraction path reuses the UTF-16 page buffer across pages

These changes are intended to preserve output behavior while improving throughput on large novels and batch conversions.

---

## Typical integration flow

```text
PDFium load
   ↓
Page-by-page text extraction
   ↓
Optional CJK paragraph reflow
   ↓
Consumer tool (OpenCC, search, export, etc.)
```

Downstream tools are responsible for encoding conversion, dictionary-based transformations, and output formatting.

---

## Development notes

* Built with the official Rust stable toolchain
* No runtime code generation
* Embedded mode performs controlled, one-time extraction only
* Shared/global loading is supported for library lifetime stability
* Applications should still serialize concurrent extraction jobs if overlapping Pdfium use is possible

---

## Platform support

* Windows (x64 / x86 / arm64)
* Linux (x64 / arm64)
* macOS (Intel / Apple Silicon)

Platform support depends on the availability of compatible PDFium native libraries.

---

## License

This project is licensed under the MIT License.

---

## Intended audience

This crate is intended for:

* OpenCC-related tooling
* CLI tool authors embedding PDFium
* Ebook / novel processing pipelines
* Developers studying CJK PDF text extraction and reflow

It is not intended as a general-purpose PDF library.
