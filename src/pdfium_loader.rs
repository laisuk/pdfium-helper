use std::path::{Path, PathBuf};
use thiserror::Error;

#[cfg(feature = "pdfium-embed")]
use std::fs;
#[cfg(feature = "pdfium-embed")]
use std::io::{self, Write};

#[cfg(feature = "pdfium-embed")]
fn pdfium_cache_dir(app_name: &str) -> PathBuf {
    #[cfg(target_os = "windows")]
    {
        if let Ok(local) = std::env::var("LOCALAPPDATA") {
            return PathBuf::from(local).join(app_name).join("natives");
        }
    }

    #[cfg(target_os = "linux")]
    {
        if let Ok(xdg) = std::env::var("XDG_CACHE_HOME") {
            return PathBuf::from(xdg).join(app_name).join("natives");
        }
        if let Ok(home) = std::env::var("HOME") {
            return PathBuf::from(home)
                .join(".cache")
                .join(app_name)
                .join("natives");
        }
    }

    #[cfg(target_os = "macos")]
    {
        if let Ok(home) = std::env::var("HOME") {
            return PathBuf::from(home)
                .join("Library")
                .join("Caches")
                .join(app_name)
                .join("natives");
        }
    }

    std::env::temp_dir().join(app_name).join("natives")
}

#[cfg(feature = "pdfium-embed")]
fn write_atomic(path: &Path, bytes: &[u8]) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    // If already exists and size matches, keep it (fast path)
    if let Ok(meta) = fs::metadata(path) {
        if meta.len() == bytes.len() as u64 {
            return Ok(());
        }
    }

    let tmp = path.with_extension("tmp");
    {
        let mut f = fs::File::create(&tmp)?;
        f.write_all(bytes)?;
        f.flush()?;
    }

    // Rename is atomic on most platforms. On Windows, rename fails if the target DLL is loaded.
    // That’s why we version the filename below (so we don't overwrite a loaded DLL).
    fs::rename(&tmp, path).or_else(|e| {
        let _ = fs::remove_file(&tmp);
        Err(e)
    })?;

    Ok(())
}

#[derive(Debug, Error)]
pub enum PdfiumLoadError {
    #[error("unsupported platform: {0}")]
    UnsupportedPlatform(String),

    #[error("pdfium native library missing: {0}")]
    MissingLibrary(PathBuf),

    #[error("failed to load pdfium library: {0}")]
    LoadFailed(String),
}

/// Equivalent to `_detect_platform_folder()` in Python. \:contentReference[oaicite:4]{index=4}
pub fn detect_platform_folder() -> Result<String, PdfiumLoadError> {
    #[cfg(target_os = "windows")]
    {
        let arch = match std::env::consts::ARCH {
            "aarch64" => "arm64",
            _ => {
                if cfg!(target_pointer_width = "64") {
                    "x64"
                } else {
                    "x86"
                }
            }
        };
        return Ok(format!("win-{}", arch));
    }

    #[cfg(target_os = "linux")]
    {
        let arch = match std::env::consts::ARCH {
            "aarch64" => "arm64",
            "x86_64" => "x64",
            "x86" | "i686" => "x86",
            _other => {
                // best effort: treat unknown 64-bit as x64, else x86
                if cfg!(target_pointer_width = "64") {
                    "x64"
                } else {
                    "x86"
                }
            }
        };
        return Ok(format!("linux-{}", arch));
    }

    #[cfg(target_os = "macos")]
    {
        let arch = if std::env::consts::ARCH == "aarch64" {
            "arm64"
        } else {
            "x64"
        };
        return Ok(format!("macos-{}", arch));
    }

    #[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
    {
        Err(PdfiumLoadError::UnsupportedPlatform(
            std::env::consts::OS.to_string(),
        ))
    }
}

pub fn default_library_name() -> &'static str {
    if cfg!(target_os = "windows") {
        "pdfium.dll"
    } else if cfg!(target_os = "linux") {
        "libpdfium.so"
    } else {
        "libpdfium.dylib"
    }
}

/// A handle to a dynamically loaded **Pdfium native library**.
///
/// `PdfiumLibrary` owns a `libloading::Library` instance to ensure that
/// all resolved Pdfium symbols remain valid for the lifetime of the handle.
///
/// This type does **not** perform any Pdfium initialization by itself
/// (e.g. `FPDF_InitLibrary`). It is responsible only for:
///
/// - locating a compatible Pdfium native library (`.dll`, `.so`, `.dylib`)
/// - loading it into the current process
/// - keeping the library alive so function pointers remain valid
///
/// ## Runtime layout
///
/// The expected on-disk layout is:
///
/// ```text
/// <base_dir>/
/// └── pdfium/
///     └── <platform>/
///         └── <pdfium library>
/// ```
///
/// Where:
///
/// - `<platform>` is one of:
///   - `win-x64`, `win-x86`
///   - `linux-x64`, `linux-arm64`
///   - `macos-x64`, `macos-arm64`
/// - `<pdfium library>` is:
///   - `pdfium.dll` (Windows)
///   - `libpdfium.so` (Linux)
///   - `libpdfium.dylib` (macOS)
///
/// This design intentionally mirrors common Pdfium-based tools:
/// users can simply place the correct Pdfium binary next to the executable
/// without recompiling.
///
/// ## Safety
///
/// Loading a native library is inherently unsafe. All `unsafe` usage
/// is contained within this type; consumers interact with it through
/// safe abstractions.
pub struct PdfiumLibrary {
    lib: libloading::Library,
}

impl PdfiumLibrary {
    /// Loads Pdfium from a **bundled directory layout**.
    ///
    /// This function looks for Pdfium under:
    ///
    /// ```text
    /// base_dir/pdfium/<platform>/<library>
    /// ```
    ///
    /// where `<platform>` and `<library>` are determined automatically
    /// for the current operating system and CPU architecture.
    ///
    /// ### Typical usage
    ///
    /// This is the recommended loading method for:
    ///
    /// - released CLI binaries
    /// - portable ZIP distributions
    /// - applications that bundle Pdfium alongside the executable
    ///
    /// ### Errors
    ///
    /// Returns [`PdfiumLoadError::MissingLibrary`] if the expected file
    /// does not exist, or [`PdfiumLoadError::LoadFailed`] if the library
    /// exists but cannot be loaded by the OS.
    pub fn load_from_bundled_dir(base_dir: &Path) -> Result<(Self, PathBuf), PdfiumLoadError> {
        let platform_folder = detect_platform_folder()?;
        let lib_path = base_dir
            .join("pdfium")
            .join(platform_folder)
            .join(default_library_name());

        if !lib_path.exists() {
            return Err(PdfiumLoadError::MissingLibrary(lib_path));
        }

        let lib = unsafe {
            libloading::Library::new(&lib_path)
                .map_err(|e| PdfiumLoadError::LoadFailed(format!("{e}")))?
        };

        Ok((Self { lib }, lib_path))
    }

    /// Loads Pdfium from a directory that contains the native library directly:
    ///
    /// ```text
    /// <dir>/<library>
    /// ```
    ///
    /// e.g. `opencc-rs.exe` and `pdfium.dll` in the same folder.
    fn load_from_dir_single_lib(dir: &Path) -> Result<(Self, PathBuf), PdfiumLoadError> {
        let lib_path = dir.join(default_library_name());

        if !lib_path.exists() {
            return Err(PdfiumLoadError::MissingLibrary(lib_path));
        }

        let lib = unsafe {
            libloading::Library::new(&lib_path)
                .map_err(|e| PdfiumLoadError::LoadFailed(format!("{e}")))?
        };

        Ok((Self { lib }, lib_path))
    }

    pub fn load_from_exe_dir() -> Result<(PdfiumLibrary, PathBuf), PdfiumLoadError> {
        let exe =
            std::env::current_exe().map_err(|e| PdfiumLoadError::LoadFailed(e.to_string()))?;

        let dir = exe
            .parent()
            .ok_or_else(|| PdfiumLoadError::LoadFailed("Cannot determine exe directory".into()))?;

        PdfiumLibrary::load_from_dir_single_lib(dir)
    }

    /// Loads Pdfium from an **explicit file path**.
    ///
    /// This is primarily intended for:
    ///
    /// - development and debugging
    /// - unit or integration tests
    /// - custom embedding scenarios
    ///
    /// The caller is responsible for ensuring the provided path points
    /// to a compatible Pdfium native library for the current platform.
    pub fn load_from_path(lib_path: &Path) -> Result<Self, PdfiumLoadError> {
        let lib = unsafe {
            libloading::Library::new(lib_path)
                .map_err(|e| PdfiumLoadError::LoadFailed(format!("{e}")))?
        };
        Ok(Self { lib })
    }

    /// Resolves a raw symbol from the loaded Pdfium library.
    ///
    /// This is an internal helper used by higher-level bindings to obtain
    /// typed function pointers (e.g. `FPDF_LoadDocument`).
    ///
    /// # Safety
    ///
    /// - The caller must request the correct symbol name and type.
    /// - The returned value must not outlive `self`.
    pub(crate) unsafe fn get<T>(&self, name: &[u8]) -> Result<T, PdfiumLoadError>
    where
        T: Copy,
    {
        let sym: libloading::Symbol<T> = self
            .lib
            .get(name)
            .map_err(|e| PdfiumLoadError::LoadFailed(format!("{e}")))?;
        Ok(*sym)
    }

    /// Load the Pdfium native library using a series of fallback strategies.
    ///
    /// This loader supports **two directory layouts** at each search location:
    ///
    /// ## Supported layouts
    ///
    /// ### 1) Single-library layout (no bundling)
    ///
    /// A single Pdfium native library placed directly in the directory:
    ///
    /// ```text
    /// <dir>/pdfium.dll        (Windows)
    /// <dir>/libpdfium.so     (Linux)
    /// <dir>/libpdfium.dylib  (macOS)
    /// ```
    ///
    /// This is intended for **CLI / portable distribution**, where the executable
    /// and Pdfium library are placed side-by-side without subdirectories.
    ///
    /// ---
    ///
    /// ### 2) Bundled layout (platform folder)
    ///
    /// ```text
    /// <dir>/pdfium/<platform>/<library>
    /// ```
    ///
    /// Example:
    ///
    /// ```text
    /// pdfium/win-x64/pdfium.dll
    /// pdfium/linux-x64/libpdfium.so
    /// pdfium/macos-arm64/libpdfium.dylib
    /// ```
    ///
    /// This layout is recommended when shipping **multiple platforms** together
    /// or when embedding Pdfium as part of a larger distribution.
    ///
    /// ---
    ///
    /// ## Search order
    ///
    /// The loader tries the following locations **in order**, and for each location
    /// attempts **single-library layout first**, then bundled layout:
    ///
    /// 1. `PDFIUM_LIB_DIR` environment variable
    /// 2. Directory containing the current executable
    /// 3. Current working directory
    /// 4. `CARGO_MANIFEST_DIR` (development fallback)
    ///
    /// The first successfully loaded library is used.
    ///
    /// ---
    ///
    /// ## Returns
    ///
    /// On success, returns:
    ///
    /// ```text
    /// (PdfiumLibrary, PathBuf)
    /// ```
    ///
    /// where the `PathBuf` is the **actual native library path loaded**.
    ///
    /// ---
    ///
    /// ## Errors
    ///
    /// Returns `PdfiumLoadError::MissingLibrary` if no suitable Pdfium library
    /// can be found in any fallback location, or `LoadFailed` if loading fails.
    ///
    /// ---
    ///
    /// ## Notes
    ///
    /// - This function does **not** bundle or extract native libraries.
    /// - It is suitable for both **development** (`cargo run`) and
    ///   **production CLI binaries**.
    /// - Keeping the Pdfium library next to the executable is the
    ///   recommended approach for end users.
    pub fn load_with_fallbacks() -> Result<(PdfiumLibrary, PathBuf), PdfiumLoadError> {
        // 1) Explicit override: PDFIUM_LIB_DIR
        if let Ok(dir) = std::env::var("PDFIUM_LIB_DIR") {
            let base = PathBuf::from(dir);

            // (A) single library in that dir
            if let Ok(ok) = PdfiumLibrary::load_from_dir_single_lib(&base) {
                return Ok(ok);
            }
            // (B) bundled layout under that dir
            return PdfiumLibrary::load_from_bundled_dir(&base);
        }

        // 2) Next to current executable (release-friendly)
        if let Ok(exe) = std::env::current_exe() {
            if let Some(dir) = exe.parent() {
                // (A) single library next to exe
                if let Ok(ok) = PdfiumLibrary::load_from_dir_single_lib(dir) {
                    return Ok(ok);
                }
                // (B) bundled layout next to exe
                if let Ok(ok) = PdfiumLibrary::load_from_bundled_dir(dir) {
                    return Ok(ok);
                }
            }
        }

        // 3) Current working directory
        if let Ok(cwd) = std::env::current_dir() {
            if let Ok(ok) = PdfiumLibrary::load_from_dir_single_lib(&cwd) {
                return Ok(ok);
            }
            if let Ok(ok) = PdfiumLibrary::load_from_bundled_dir(&cwd) {
                return Ok(ok);
            }
        }

        // 4) Crate root (development only)
        if let Ok(manifest) = std::env::var("CARGO_MANIFEST_DIR") {
            let base = PathBuf::from(manifest);

            if let Ok(ok) = PdfiumLibrary::load_from_dir_single_lib(&base) {
                return Ok(ok);
            }
            if let Ok(ok) = PdfiumLibrary::load_from_bundled_dir(&base) {
                return Ok(ok);
            }
        }

        // 5) Embedded Pdfium fallback (optional)
        #[cfg(feature = "pdfium-embed")]
        if let Ok(ok) = PdfiumLibrary::load_from_embedded("pdfium-helper") {
            return Ok(ok);
        }

        // Prefer returning a “likely” expected path (keep your existing behavior)
        let platform = detect_platform_folder()?;
        let expected = PathBuf::from("pdfium")
            .join(platform)
            .join(default_library_name());

        Err(PdfiumLoadError::MissingLibrary(expected))
    }

    #[cfg(feature = "pdfium-embed")]
    fn embedded_version_tag() -> &'static str {
        // Put *your* PDFium build version here (or crate version).
        // Important on Windows: change this when you ship a new DLL so it writes a new filename.
        "145.0.7616.0"
    }

    #[cfg(feature = "pdfium-embed")]
    pub fn load_from_embedded(app_name: &str) -> Result<(Self, PathBuf), PdfiumLoadError> {
        let platform = detect_platform_folder()?;

        // IMPORTANT: version the filename so we never need to overwrite a loaded DLL on Windows
        let file_name = format!(
            "pdfium-{}-{}.{}",
            Self::embedded_version_tag(),
            platform,
            if cfg!(target_os = "windows") {
                "dll"
            } else if cfg!(target_os = "linux") {
                "so"
            } else {
                "dylib"
            }
        );

        let base = pdfium_cache_dir(app_name);
        let out = base.join(file_name);

        let compressed: &'static [u8] = PDFIUM_ZSTD;
        let raw = decompress_native(compressed)?;
        write_atomic(&out, &raw).map_err(|e| {
            PdfiumLoadError::LoadFailed(format!("extract embedded pdfium failed: {e}"))
        })?;

        let lib = unsafe {
            libloading::Library::new(&out)
                .map_err(|e| PdfiumLoadError::LoadFailed(e.to_string()))?
        };

        Ok((Self { lib }, out))
    }
}

#[cfg(all(
    feature = "pdfium-embed",
    target_os = "windows",
    target_arch = "x86_64"
))]
static PDFIUM_ZSTD: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/pdfium/win-x64/pdfium.dll.zst"
));

#[cfg(all(feature = "pdfium-embed", target_os = "windows", target_arch = "x86"))]
static PDFIUM_ZSTD: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/pdfium/win-x86/pdfium.dll.zst"
));

#[cfg(all(
    feature = "pdfium-embed",
    target_os = "windows",
    target_arch = "aarch64"
))]
static PDFIUM_ZSTD: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/pdfium/win-arm64/pdfium.dll.zst"
));

#[cfg(all(feature = "pdfium-embed", target_os = "linux", target_arch = "x86_64"))]
static PDFIUM_ZSTD: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/pdfium/linux-x64/libpdfium.so.zst"
));

#[cfg(all(feature = "pdfium-embed", target_os = "linux", target_arch = "aarch64"))]
static PDFIUM_ZSTD: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/pdfium/linux-arm64/libpdfium.so.zst"
));

#[cfg(all(feature = "pdfium-embed", target_os = "macos", target_arch = "x86_64"))]
static PDFIUM_ZSTD: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/pdfium/macos-x64/libpdfium.dylib.zst"
));

#[cfg(all(feature = "pdfium-embed", target_os = "macos", target_arch = "aarch64"))]
static PDFIUM_ZSTD: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/pdfium/macos-arm64/libpdfium.dylib.zst"
));

#[cfg(all(
    feature = "pdfium-embed",
    not(any(
        all(
            target_os = "windows",
            any(target_arch = "x86_64", target_arch = "x86", target_arch = "aarch64")
        ),
        all(
            target_os = "linux",
            any(target_arch = "x86_64", target_arch = "aarch64")
        ),
        all(
            target_os = "macos",
            any(target_arch = "x86_64", target_arch = "aarch64")
        ),
    ))
))]
compile_error!("pdfium-embed enabled but no embedded pdfium binary for this target");

#[cfg(feature = "pdfium-embed")]
fn decompress_native(zstd_bytes: &[u8]) -> Result<Vec<u8>, PdfiumLoadError> {
    use std::io::Read;

    let mut decoder = zstd::stream::read::Decoder::new(zstd_bytes)
        .map_err(|e| PdfiumLoadError::LoadFailed(format!("zstd decoder: {e}")))?;

    let mut out = Vec::new();
    decoder
        .read_to_end(&mut out)
        .map_err(|e| PdfiumLoadError::LoadFailed(format!("zstd decode: {e}")))?;

    Ok(out)
}
