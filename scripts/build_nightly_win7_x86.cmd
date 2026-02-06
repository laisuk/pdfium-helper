:: scripts/build_nightly_win7_x86.cmd
@echo off
setlocal

REM ----------------------------------------
REM Win7 x86 target build (nightly + build-std)
REM ----------------------------------------

REM Force Win7 subsystem baseline (6.01)
set RUSTFLAGS=-C link-arg=/SUBSYSTEM:CONSOLE,6.01

REM Feature flag (optional)
set FEATURES=

if not "%~1"=="" (
    set FEATURES=--features pdfium-embed
)

REM Optional: sanity echo (helps logs / CI)
echo RUSTFLAGS=%RUSTFLAGS%
if not "%FEATURES%"=="" echo FEATURES=%FEATURES%
echo.

cargo +nightly build -r --workspace ^
  -Z build-std=std,panic_abort ^
  --target i686-win7-windows-msvc ^
  %FEATURES%

endlocal
