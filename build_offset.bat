@echo off
setlocal enabledelayedexpansion
chcp 65001 >nul
set "PYTHONIOENCODING=utf-8"

:: ===========================================================================
:: build_offset.bat - offset build for rp2040-retro-loader
::
:: NOTE: keep this file pure ASCII. cmd.exe parses .bat with the system
:: codepage (cp950 here); UTF-8 Chinese in comments corrupts the surrounding
:: syntax (%VAR% expansions silently lose characters, and even the
:: "setlocal enabledelayedexpansion" line gets eaten). The Chinese write-up
:: lives in the README chapter on the boot loader, and in loader_offset/*.py.
::
:: Difference from full_build.bat: the image links at 0x10004000 instead of
:: 0x10000000, leaving the first 16KB to the boot loader (when loaded from
:: SD) or to the trampoline (when flashed over USB).
::
:: Why not just move the file: RP2040 runs XIP, so every address is baked
:: into the machine code at compile time. Moving bytes only makes every
:: pointer wrong. The image has to be re-linked.
::
:: Usage:
::     build_offset.bat                  loader repo defaults to ..\rp2040-retro-loader
::     build_offset.bat <loader repo path>
::
:: Outputs (all under build_offset\):
::     PicoApple2.ino.uf2          body only, first 16KB empty - NOT flashable alone
::     PicoApple2_standalone.uf2   trampoline + body - this is the one to use
::
:: This script does not flash. The standalone file works both ways: drop it
:: in the SD card root for the loader, or picotool load -v -x it directly.
:: ===========================================================================

set "SCRIPT_DIR=%~dp0"

echo Scanning environment...
powershell -ExecutionPolicy Bypass -File "%SCRIPT_DIR%scripts\scan_env.ps1"
if %errorlevel% neq 0 ( echo [ERROR] Environment scan failed. & pause & exit /b 1 )
call "%SCRIPT_DIR%build_env.bat"

set "RUST_PROJECT_DIR=%PROJECT_ROOT%apple2_core"
set "ARDUINO_CLI=%ARDUINO_CLI_PATH%"
set "OUT_DIR=%PROJECT_ROOT%build_offset"
set "OFFSET_LD=%PROJECT_ROOT%loader_offset\memmap_app_arduino.ld"
set "GEN_LIB_DIR=%OUT_DIR%\arduino_libs"

if "%~1"=="" (
    set "LOADER_PATH=%PROJECT_ROOT%..\rp2040-retro-loader"
) else (
    set "LOADER_PATH=%~1"
)
if not exist "!LOADER_PATH!\tools\merge_uf2.py" (
    echo [ERROR] rp2040-retro-loader not found at "!LOADER_PATH!"
    echo         Usage: build_offset.bat ^<loader repo path^>
    pause & exit /b 1
)

echo.
echo ========================================================
echo [1/7] Compiling Rust Core...
echo ========================================================
cd /d "%RUST_PROJECT_DIR%"
cargo build --target thumbv6m-none-eabi --release
if %errorlevel% neq 0 ( echo [ERROR] Rust failed. & pause & exit /b 1 )

echo.
echo ========================================================
echo [2/7] Syncing Library to Project Local...
echo ========================================================
if not exist "%PROJECT_ROOT%src" mkdir "%PROJECT_ROOT%src"
copy /y "%RUST_PROJECT_DIR%\target\thumbv6m-none-eabi\release\libapple2_core.a" "%PROJECT_ROOT%src\libapple2_core.a"
echo [OK] Static library synced to local src/

echo.
echo ========================================================
echo [3/7] Generating self-contained Apple2Core library...
echo ========================================================
:: This build does NOT use the Arduino sketchbook's Apple2Core library.
:: That dependency broke three times (sketchbook moved to another drive, src/
:: left behind, stale .a silently linked) and every failure looked like
:: something other than an environment problem. The library is generated here
:: instead, from the repo's own Apple2Core.h and the .a just built above.
:: See loader_offset/make_arduino_lib.py for the full reasoning.
cd /d "%PROJECT_ROOT%"
python "%PROJECT_ROOT%loader_offset\make_arduino_lib.py" "%GEN_LIB_DIR%" "%PROJECT_ROOT%src\libapple2_core.a"
if %errorlevel% neq 0 ( echo [ERROR] make_arduino_lib.py failed. & pause & exit /b 1 )

echo.
echo ========================================================
echo [4/7] Generating offset linker script...
echo ========================================================
python "%PROJECT_ROOT%loader_offset\gen_app_ld.py"
if %errorlevel% neq 0 ( echo [ERROR] gen_app_ld.py failed. & pause & exit /b 1 )

echo.
echo ========================================================
echo [5/7] Compiling Arduino Sketch (OFFSET, link at 0x10004000)...
echo ========================================================
:: --libraries points ONLY at the generated library from step 3. Everything
:: else the sketch includes (SPI, SD, SDFS, SdFat) ships with the rp2040
:: platform, so nothing from the user's sketchbook is needed and nothing from
:: it can break this build.
::
:: The Rust .a is linked via the generated library's "ldflags=-lapple2_core"
:: (precompiled library mechanism), which arduino-cli places inside the link
:: command's --start-group and after the sketch objects. That position is
:: load-bearing: an archive listed before the objects that need its symbols is
:: skipped entirely by ld, giving a wall of "undefined reference to apple2_*".
::
:: One build-property override remains:
::
:: recipe.hooks.linking.prelink.1.pattern
::     arduino-pico generates its linker script at build time: simplesub.py
::     substitutes __FLASH_LENGTH__ etc. into lib/rp2040/memmap_default.ld and
::     writes {build.path}/memmap_default.ld, which the link recipe then reads
::     by that hardcoded name. So the way to swap linker scripts is NOT to add
::     -Wl,--script (it would fight the hardcoded one) but to repoint this
::     hook's --input at our offset template. Every other argument is copied
::     verbatim from platform.txt.
"%ARDUINO_CLI%" compile --fqbn %FQBN% --libraries "%GEN_LIB_DIR%" ^
    --build-property "recipe.hooks.linking.prelink.1.pattern=\"{runtime.tools.pqt-python3.path}/python3\" -I \"{runtime.platform.path}/tools/simplesub.py\" --input \"%OFFSET_LD%\" --out \"{build.path}/memmap_default.ld\" --sub __FLASH_LENGTH__ {build.flash_length} --sub __EEPROM_START__ {build.eeprom_start} --sub __FS_START__ {build.fs_start} --sub __FS_END__ {build.fs_end} --sub __RAM_LENGTH__ {build.ram_length} --sub __PSRAM_LENGTH__ {build.psram_length}" ^
    --output-dir "%OUT_DIR%" "PicoApple2.ino"
if %errorlevel% neq 0 ( echo [ERROR] Arduino build failed. & pause & exit /b 1 )

echo.
echo ========================================================
echo [6/7] Checking flash layout...
echo ========================================================
python "%PROJECT_ROOT%loader_offset\check_flash_layout.py" "%OUT_DIR%\PicoApple2.ino.uf2"
if %errorlevel% neq 0 ( echo [ERROR] Layout check failed - do not flash this file. & pause & exit /b 1 )

echo.
echo ========================================================
echo [7/7] Merging with trampoline...
echo ========================================================
if not exist "!LOADER_PATH!\build\trampoline.uf2" (
    echo [ERROR] "!LOADER_PATH!\build\trampoline.uf2" not found.
    echo         Build the loader once first - see its README section 2.2.
    pause & exit /b 1
)
python "!LOADER_PATH!\tools\merge_uf2.py" "!LOADER_PATH!\build\trampoline.uf2" ^
    "%OUT_DIR%\PicoApple2.ino.uf2" -o "%OUT_DIR%\PicoApple2_standalone.uf2"
if %errorlevel% neq 0 ( echo [ERROR] merge_uf2.py failed. & pause & exit /b 1 )

echo.
echo ========================================================
echo SUCCESS
echo ========================================================
echo   %OUT_DIR%\PicoApple2_standalone.uf2
echo.
echo   Put it in the SD card root for the loader, or flash it directly:
echo     picotool load -v -x "%OUT_DIR%\PicoApple2_standalone.uf2"
echo.
echo   Note: picotool load -x is a soft reset, so the loader passes straight
echo   through to the app without showing its menu. To see the menu, flash
echo   without -x and then power-cycle.
echo ========================================================
pause
