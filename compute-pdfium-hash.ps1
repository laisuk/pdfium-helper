[CmdletBinding()]
param(
    [string]$RepoRoot = $PSScriptRoot,
    [string]$PdfiumDir = (Join-Path $PSScriptRoot 'pdfium'),
    [string]$VersionFile = (Join-Path $PSScriptRoot 'VERSION'),
    [string]$LegacyVersionFile = (Join-Path $PSScriptRoot 'pdfium\VERSION')
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

function Get-NormalizedRelativePath {
    param(
        [Parameter(Mandatory = $true)]
        [string]$BaseDir,
        [Parameter(Mandatory = $true)]
        [string]$Path
    )

    $relative = [System.IO.Path]::GetRelativePath($BaseDir, $Path)
    return $relative.Replace('\', '/')
}

function Get-VersionValue {
    param(
        [Parameter(Mandatory = $true)]
        [string[]]$Paths
    )

    foreach ($path in $Paths) {
        if (-not (Test-Path -LiteralPath $path)) {
            continue
        }

        foreach ($rawLine in Get-Content -LiteralPath $path) {
            $line = $rawLine.Trim()
            if ([string]::IsNullOrWhiteSpace($line) -or $line.StartsWith('#')) {
                continue
            }

            if ($line.Contains('=')) {
                $parts = $line.Split('=', 2)
                if ($parts[0].Trim() -eq 'version') {
                    $value = $parts[1].Trim()
                    if (-not [string]::IsNullOrWhiteSpace($value)) {
                        return $value
                    }
                }
                continue
            }

            # Legacy format: first non-comment, non-empty line is the version.
            return $line
        }
    }

    throw "Could not find a version value in: $($Paths -join ', ')"
}

if (-not (Test-Path -LiteralPath $RepoRoot -PathType Container)) {
    throw "Repository root not found: $RepoRoot"
}

if (-not (Test-Path -LiteralPath $PdfiumDir -PathType Container)) {
    throw "Pdfium directory not found: $PdfiumDir"
}

$version = Get-VersionValue -Paths @($VersionFile, $LegacyVersionFile)

$nativeFiles = Get-ChildItem -LiteralPath $PdfiumDir -Recurse -File |
    Where-Object { $_.Extension -in '.dll', '.so', '.dylib' } |
    Sort-Object { Get-NormalizedRelativePath -BaseDir $RepoRoot -Path $_.FullName }

if (-not $nativeFiles) {
    throw "No native Pdfium libraries found under $PdfiumDir"
}

$hashLines = foreach ($file in $nativeFiles) {
    $relativePath = Get-NormalizedRelativePath -BaseDir $RepoRoot -Path $file.FullName
    $hash = (Get-FileHash -LiteralPath $file.FullName -Algorithm SHA256).Hash.ToUpperInvariant()
    "{0}=SHA256:{1}" -f $relativePath, $hash
}

$outputLines = @(
    '# Pdfium native manifest'
    ''
    "version=$version"
    ''
) + $hashLines

Set-Content -LiteralPath $VersionFile -Value $outputLines
Set-Content -LiteralPath $LegacyVersionFile -Value $outputLines
Write-Host "Updated $VersionFile and $LegacyVersionFile with version=$version and $($hashLines.Count) hash entries."
