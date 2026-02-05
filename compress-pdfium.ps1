param(
    [switch]$Overwrite
)

# Compress all pdfium native binaries to .zst
# Run from repository root
# Requires: zstd.exe in PATH

$ErrorActionPreference = "Stop"

$root = Join-Path (Get-Location) "pdfium"

if (-not (Test-Path $root)) {
    Write-Error "pdfium directory not found at repo root"
    exit 1
}

# Native file patterns
$patterns = @(
    "*.dll",    # Windows
    "*.so",     # Linux
    "*.dylib"   # macOS
)

foreach ($pattern in $patterns) {
    Get-ChildItem -Path $root -Recurse -File -Filter $pattern | ForEach-Object {
        $src = $_.FullName
        $dst = "$src.zst"

        if ((Test-Path $dst) -and (-not $Overwrite)) {
            Write-Host "[SKIP] $dst already exists (use -Overwrite to force)"
            return
        }

        Write-Host "[ZSTD] $src -> $dst"

        & zstd `
            -19 `
            --long=27 `
            --threads=0 `
            -f `
            $src `
            -o $dst

        if ($LASTEXITCODE -ne 0) {
            Write-Error "zstd failed on $src"
            exit 1
        }
    }
}

Write-Host "Done. All pdfium natives compressed."
