:: scripts/build-stable.cmd
@echo off
setlocal

REM ----------------------------------------
REM Stable workspace build
REM ----------------------------------------

REM Optional feature flag
set FEATURES=

if not "%~1"=="" (
    set FEATURES=--features pdfium-embed
)

REM Optional: sanity echo
if not "%FEATURES%"=="" echo FEATURES=%FEATURES%
echo.

cargo +stable build -r --workspace %FEATURES%

endlocal

