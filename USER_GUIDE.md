# pdfium-helper User Guide

This guide is for developers who want to use `pdfium-helper` directly from Rust.

It focuses on the crate's practical public API:

- loading a Pdfium native library
- extracting text from text-based PDFs
- reflowing CJK-heavy extracted text into readable paragraphs
- handling errors and basic progress reporting

If you are new to the crate, start with the quick start section and the two main functions:

- `PdfiumLibrary::load_with_fallbacks()`
- `extract_pdf_pages_with_callback_pdfium()` or `extract_pdf_text_pdfium()`

---

## What This Crate Is Good At

`pdfium-helper` is designed for a narrow workflow:

1. Load a compatible Pdfium native library.
2. Extract text from a text-embedded PDF.
3. Optionally reflow fragmented CJK text.
4. Pass the result into your own converter, search indexer, exporter, or CLI.

It is not a general-purpose PDF toolkit. It does not do rendering, OCR, annotation, or editing.

---

## Before You Start

### 1. Add the dependency

```toml
[dependencies]
pdfium-helper = { path = "../pdfium-helper" }
```

If you want embedded native loading:

```toml
[dependencies]
pdfium-helper = { path = "../pdfium-helper", features = ["pdfium-embed"] }
```

### 2. Provide a Pdfium native library

In dynamic mode, `pdfium-helper` expects a platform-matching Pdfium library such as:

- Windows: `pdfium.dll`
- Linux: `libpdfium.so`
- macOS: `libpdfium.dylib`

The loader can find it in either of these layouts:

```text
<dir>/pdfium.dll
<dir>/libpdfium.so
<dir>/libpdfium.dylib
```

or:

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

---

## Quick Start

This is the simplest complete flow: load Pdfium, extract all text, then reflow it.

```rust
use pdfium_helper::{
    extract_pdf_text_pdfium,
    reflow_cjk_paragraphs,
    PdfiumLibrary,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (pdfium, lib_path) = PdfiumLibrary::load_with_fallbacks()?;
    println!("Loaded Pdfium: {}", lib_path.display());

    let extracted = extract_pdf_text_pdfium(&pdfium, "document.pdf", false)?;
    let reflowed = reflow_cjk_paragraphs(&extracted, false, false);

    std::fs::write("document.txt", reflowed)?;
    Ok(())
}
```

Use this when:

- you want the full extracted text in memory
- you do not need per-page progress callbacks
- you want a small integration surface

---

## Core API Overview

The main public API is small.

### Loading Pdfium

- `PdfiumLibrary::load_with_fallbacks()`
- `PdfiumLibrary::global_with_fallbacks()`
- `PdfiumLibrary::load_from_bundled_dir()`
- `PdfiumLibrary::load_from_base_dir_flexible()`
- `PdfiumLibrary::load_from_exe_dir()`
- `PdfiumLibrary::load_from_path()`

### Extracting PDF text

- `extract_pdf_pages_with_callback_pdfium()`
- `extract_pdf_text_pdfium()`

### Reflowing extracted text

- `reflow_cjk_paragraphs()`
- `reflow_cjk_paragraphs_with_heading_regex()`

### Error and utility helpers

- `PdfiumLoadError`
- `PdfiumExtractError`
- `PdfiumLastError`
- `format_thousand()`
- `print_progress()`

`print_progress()` is a convenience helper for simple CLIs. Library users do not need to use it.

---

## Loading Pdfium

### `PdfiumLibrary::load_with_fallbacks()`

This is the default loader for most CLI tools and small applications.

Search order:

1. `PDFIUM_LIB_DIR`
2. directory containing the current executable
3. current working directory
4. `CARGO_MANIFEST_DIR`
5. embedded fallback, if `pdfium-embed` is enabled

At each location it tries:

1. single-library layout
2. bundled `pdfium/<platform>/...` layout

Typical usage:

```rust
use pdfium_helper::PdfiumLibrary;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (pdfium, lib_path) = PdfiumLibrary::load_with_fallbacks()?;
    println!("Loaded Pdfium from {}", lib_path.display());
    let _ = pdfium;
    Ok(())
}
```

Use this when:

- you are building a CLI
- you want development and release-friendly behavior
- you do not want to manually resolve native paths

### `PdfiumLibrary::global_with_fallbacks()`

This returns a process-global shared library handle:

```rust
use pdfium_helper::PdfiumLibrary;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (pdfium, lib_path) = PdfiumLibrary::global_with_fallbacks()?;
    println!("Using shared Pdfium from {}", lib_path.display());
    let _ = pdfium;
    Ok(())
}
```

Use this when:

- your process may perform multiple extraction jobs over time
- you want to avoid repeated library load/unload cycles
- you are building a GUI app or long-lived service

Important note:

- this improves library lifetime stability
- it does not automatically make extraction safe for overlapping concurrent jobs
- if overlapping extraction is possible, serialize access at the application layer

### `PdfiumLibrary::load_from_bundled_dir(base_dir)`

Use this when your deployment layout is explicitly:

```text
base_dir/pdfium/<platform>/<library>
```

Example:

```rust
use std::path::Path;
use pdfium_helper::PdfiumLibrary;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (pdfium, path) = PdfiumLibrary::load_from_bundled_dir(Path::new("."))?;
    println!("Loaded bundled Pdfium from {}", path.display());
    let _ = pdfium;
    Ok(())
}
```

### `PdfiumLibrary::load_from_base_dir_flexible(base_dir)`

This accepts either:

- `<base_dir>/<library>`
- `<base_dir>/pdfium/<platform>/<library>`

It is a good choice when you want to accept user-supplied paths without forcing one exact layout.

### `PdfiumLibrary::load_from_exe_dir()`

This is a narrower helper for the "library sits next to the executable" case.

### `PdfiumLibrary::load_from_path(lib_path)`

Use this only when you already know the exact native library file path.

---

## Extracting Text

### `extract_pdf_pages_with_callback_pdfium()`

Signature shape:

```text
pub fn extract_pdf_pages_with_callback_pdfium<F>(
    lib: &PdfiumLibrary,
    path: &str,
    add_page_header: bool,
    callback: F,
) -> Result<(), PdfiumExtractError>
where
    F: FnMut(i32, i32, &str)
```

This is the most flexible extraction API.

The callback receives:

- `page`: current 1-based page number
- `total`: total page count
- `text`: normalized text for that page

Typical usage:

```rust
use pdfium_helper::{extract_pdf_pages_with_callback_pdfium, PdfiumLibrary};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (pdfium, _) = PdfiumLibrary::load_with_fallbacks()?;
    let mut pages = Vec::new();

    extract_pdf_pages_with_callback_pdfium(&pdfium, "document.pdf", false, |page, total, text| {
        println!("page {page}/{total}: {} chars", text.chars().count());
        pages.push(text.to_owned());
    })?;

    let full_text = pages.concat();
    println!("Extracted {} chars total", full_text.chars().count());
    Ok(())
}
```

Use this when:

- you want progress reporting
- you want to stream or batch-process page output
- you want to accumulate pages yourself
- you may later add cancellation or custom per-page logic

### `extract_pdf_text_pdfium()`

This is the convenience wrapper around the callback API.

```rust
use pdfium_helper::{extract_pdf_text_pdfium, PdfiumLibrary};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (pdfium, _) = PdfiumLibrary::load_with_fallbacks()?;
    let text = extract_pdf_text_pdfium(&pdfium, "document.pdf", false)?;
    println!("Extracted {} chars", text.chars().count());
    Ok(())
}
```

Use this when:

- you just want one big `String`
- you do not need page progress callbacks
- your PDFs are small enough to comfortably hold in memory

---

## Extraction Behavior Details

These details matter because `pdfium-helper` intentionally follows Pdfium's flat-text behavior rather than reconstructing visual layout.

### Text normalization rules

For each page, extraction currently:

- decodes Pdfium UTF-16 output to UTF-8 Rust `String`
- normalizes `CRLF` and `CR` to `\n`
- compresses runs of 3 or more newlines down to at most 2
- trims trailing whitespace from each page's text block
- appends `\n\n` after non-empty pages
- returns `"\n"` for blank or non-extractable pages

### `add_page_header`

When `add_page_header` is `true`, each emitted page begins with a header like:

```text
=== [Page 3/120] ===
```

Use page headers when:

- you want visible page boundaries in exported text
- you plan to preserve page structure through later reflow or conversion

Keep it `false` when:

- you want the cleanest plain-text output
- page markers would interfere with downstream processing

### Blank-page and failure behavior

If a page cannot be loaded or a text page handle cannot be created, the extractor emits a blank-page marker for that page rather than aborting the entire document.

If the document itself cannot be opened, extraction returns `PdfiumExtractError::LoadDocument`.

---

## Reflowing CJK Text

### `reflow_cjk_paragraphs()`

This is the main reflow function.

```rust
use pdfium_helper::reflow_cjk_paragraphs;

fn main() {
    let text = "第一行\n第二行\n\n第三行\n";
    let out = reflow_cjk_paragraphs(text, false, false);
    println!("{out}");
}
```

Parameters:

- `text`: usually the output of PDF extraction
- `add_pdf_page_header`: whether blank lines should be treated more strictly as structural boundaries
- `compact`: output paragraphs separated by `\n` instead of `\n\n`

What it does:

- normalizes line endings
- merges many artificial line breaks caused by PDF text extraction
- preserves common structure such as headings, metadata lines, visual dividers, page markers, and dialogue blocks
- applies heuristics aimed at CJK-heavy prose such as novels and ebooks

What it does not do:

- reconstruct exact page layout
- infer true semantic document structure
- perform OCR
- guarantee perfect behavior for every PDF genre

### `reflow_cjk_paragraphs_with_heading_regex()`

Use this when you need custom heading detection in addition to the built-in heuristics.

```rust
use regex::Regex;
use pdfium_helper::reflow_cjk_paragraphs_with_heading_regex;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let text = "附录\n第一行\n第二行\n";
    let heading_re = Regex::new(r"^(附录|Appendix|Side Story)")?;
    let out = reflow_cjk_paragraphs_with_heading_regex(text, false, false, Some(&heading_re));
    println!("{out}");
    Ok(())
}
```

Choose this when:

- your corpus uses project-specific heading styles
- the built-in rules are close, but not quite enough
- you are batch-processing a known document family

Performance tip:

- compile the regex once and reuse it

---

## Error Handling

There are two main error layers.

### `PdfiumLoadError`

Returned when loading the native Pdfium library fails.

Variants:

- `UnsupportedPlatform(String)`
- `MissingLibrary(PathBuf)`
- `LoadFailed(String)`

Typical handling:

```rust
use pdfium_helper::PdfiumLibrary;

fn main() {
    match PdfiumLibrary::load_with_fallbacks() {
        Ok((pdfium, path)) => {
            println!("Loaded Pdfium from {}", path.display());
            let _ = pdfium;
        }
        Err(err) => {
            eprintln!("Failed to load Pdfium: {err}");
        }
    }
}
```

### `PdfiumExtractError`

Returned when document-level extraction fails.

Current variants:

- `Load(PdfiumLoadError)`
- `LoadDocument { path, error }`

`LoadDocument` carries a `PdfiumLastError`, which provides:

- a short display label
- `message()` for human-readable explanation
- `hint()` for next-step guidance

Example:

```rust
use pdfium_helper::{extract_pdf_text_pdfium, PdfiumExtractError, PdfiumLibrary};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (pdfium, _) = PdfiumLibrary::load_with_fallbacks()?;

    match extract_pdf_text_pdfium(&pdfium, "document.pdf", false) {
        Ok(text) => println!("{}", text.len()),
        Err(err) => {
            if let PdfiumExtractError::LoadDocument { path, error } = &err {
                eprintln!("failed to open PDF: {path}");
                eprintln!("pdfium error: {}", error.message());
                eprintln!("hint: {}", error.hint());
            } else {
                eprintln!("extraction error: {err}");
            }
        }
    }

    Ok(())
}
```

### Pretty error output

For CLI-oriented rendering, `PdfiumExtractError` also provides:

```rust
use pdfium_helper::{extract_pdf_text_pdfium, PdfiumLibrary};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (pdfium, _) = PdfiumLibrary::load_with_fallbacks()?;

    if let Err(err) = extract_pdf_text_pdfium(&pdfium, "document.pdf", false) {
        eprintln!("{}", err.pretty());
    }

    Ok(())
}
```

That gives you a richer multi-line display without having the library print directly to stderr.

---

## Progress and Utility Helpers

### `print_progress(page, total, text)`

This is a convenience function for simple in-place console progress lines.

```rust
use pdfium_helper::{extract_pdf_pages_with_callback_pdfium, PdfiumLibrary};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let input = "document.pdf";
    let (pdfium, _) = PdfiumLibrary::load_with_fallbacks()?;

    extract_pdf_pages_with_callback_pdfium(&pdfium, input, false, |page, total, text| {
        pdfium_helper::print_progress(page, total, text);
    })?;
    println!();

    Ok(())
}
```

Use it when:

- you are building a straightforward terminal tool
- a carriage-return progress line is good enough

Avoid it when:

- you need structured logging
- you are writing a GUI or web app
- you want total control over formatting

### `format_thousand(n)`

Formats integers with comma thousands separators.

```rust
fn main() {
    let s = pdfium_helper::format_thousand(1_234_567);
    assert_eq!(s, "1,234,567");
}
```

---

## Recommended Usage Patterns

### Pattern 1: Simple CLI extractor

Use:

- `PdfiumLibrary::load_with_fallbacks()`
- `extract_pdf_text_pdfium()`

Good when you only need a text dump.

### Pattern 2: CLI with live progress

Use:

- `PdfiumLibrary::load_with_fallbacks()`
- `extract_pdf_pages_with_callback_pdfium()`
- optionally `print_progress()`

Good when users are waiting on large PDFs.

### Pattern 3: Novel or ebook processing pipeline

Use:

- `extract_pdf_text_pdfium()` or callback extraction
- `reflow_cjk_paragraphs()`
- your own downstream writer/converter

Good for CJK-heavy long-form text.

### Pattern 4: Long-lived app or service

Use:

- `PdfiumLibrary::global_with_fallbacks()`
- app-level serialization for overlapping extraction tasks

Good when Pdfium is loaded once and reused many times.

---

## Common Pitfalls

### Scanned PDFs do not magically work

This crate only extracts text that already exists as embedded PDF text. If a PDF is image-only, you need OCR outside this crate.

### Reflow is heuristic, not semantic

`reflow_cjk_paragraphs()` is designed for practical readability, especially for CJK prose. It will not perfectly reconstruct every layout or genre.

### Page callbacks receive normalized page chunks

The callback text is not raw bytes from Pdfium. It already includes the wrapper's normalization rules and optional page headers.

### Concurrency still needs care

`global_with_fallbacks()` keeps the library loaded, but you should still protect overlapping extraction jobs if your app might run them concurrently.

### Utility helpers are not the main product

`format_thousand()` and `print_progress()` are handy, but most consumers only need the loading, extraction, and reflow APIs.

---

## A Practical End-to-End Example

```rust
use pdfium_helper::{
    extract_pdf_pages_with_callback_pdfium,
    format_thousand,
    reflow_cjk_paragraphs,
    PdfiumExtractError,
    PdfiumLibrary,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let input = "document.pdf";
    let output = "document.txt";

    let (pdfium, lib_path) = PdfiumLibrary::load_with_fallbacks()?;
    println!("Loaded Pdfium: {}", lib_path.display());

    let mut pages = Vec::new();

    let result = extract_pdf_pages_with_callback_pdfium(&pdfium, input, false, |page, total, text| {
        pdfium_helper::print_progress(page, total, text);
        pages.push(text.to_owned());
    });

    println!();

    if let Err(err) = result {
        eprintln!("{}", err.pretty());
        return Err(Box::new(err));
    }

    let extracted = pages.concat();
    println!("Extracted {} chars", format_thousand(extracted.chars().count()));

    let reflowed = reflow_cjk_paragraphs(&extracted, false, false);
    std::fs::write(output, reflowed)?;

    println!("Saved {output}");
    Ok(())
}
```

---

## Public API Reference Summary

Most users only need these items:

- `PdfiumLibrary`
- `PdfiumLoadError`
- `extract_pdf_pages_with_callback_pdfium()`
- `extract_pdf_text_pdfium()`
- `PdfiumExtractError`
- `PdfiumLastError`
- `reflow_cjk_paragraphs()`
- `reflow_cjk_paragraphs_with_heading_regex()`

Additional helpers exported from the crate root are available, but they are lower-level or convenience-oriented and usually not needed for basic integrations.

---

## Where To Look Next

- [`README.md`](README.md) for crate scope and deployment notes
- [`examples/use_pdfium_extract_only_progress.rs`](examples/use_pdfium_extract_only_progress.rs) for a progress-oriented extractor
- [`examples/use_pdfium_extract_and_reflow_progress.rs`](examples/use_pdfium_extract_and_reflow_progress.rs) for extract-then-reflow usage
- [`src/lib.rs`](src/lib.rs) for the crate root exports
