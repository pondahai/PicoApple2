@echo off
setlocal enabledelayedexpansion

:: --- [Dynamic Environment Setup] ---
set "SCRIPT_DIR=%~dp0"
echo Scanning environment...
powershell -ExecutionPolicy Bypass -File "%SCRIPT_DIR%scripts\scan_env.ps1"
if %errorlevel% neq 0 ( echo [ERROR] Environment scan failed. & pause & exit /b )
call "%SCRIPT_DIR%build_env.bat"

:: --- 配置區 ---
set "RUST_PROJECT_DIR=%PROJECT_ROOT%apple2_core"
set "ARDUINO_LIB_SRC=%ARDUINO_USER_LIB_PATH%\Apple2Core\src"
set "TARGET=thumbv6m-none-eabi"
set "LIB_NAME=libapple2_core.a"
set "HEADER_NAME=Apple2Core.h"

echo ========================================================
echo [1/4] Checking Environment...
echo ========================================================

:: 檢查 Rust 是否安裝
where cargo >nul 2>nul
if %errorlevel% neq 0 (
    echo [ERROR] Cargo not found. Please install Rust from https://rustup.rs/
    pause
    exit /b 1
)

:: 確保編譯目標已安裝 (RP2040 Cortex-M0+)
call rustup target add %TARGET%

echo.
echo ========================================================
echo [2/4] Compiling Rust Core (%TARGET%, Release)...
echo ========================================================

cd /d "%RUST_PROJECT_DIR%"
cargo build --target %TARGET% --release

if %errorlevel% neq 0 (
    echo.
    echo [ERROR] Rust compilation failed! Please check the errors above.
    pause
    exit /b %errorlevel%
)

echo.
echo ========================================================
echo [3/4] Preparing Arduino Library Directory...
echo ========================================================

:: 確保 Arduino Library 的 src 目錄存在
if not exist "%ARDUINO_LIB_SRC%" (
    echo Creating directory: %ARDUINO_LIB_SRC%
    mkdir "%ARDUINO_LIB_SRC%"
)

echo.
echo ========================================================
echo [4/4] Syncing Files to Arduino...
echo ========================================================

:: 複製編譯產出的靜態庫 (.a)
set "SOURCE_LIB=%RUST_PROJECT_DIR%\target\%TARGET%\release\%LIB_NAME%"
if exist "%SOURCE_LIB%" (
    copy /y "%SOURCE_LIB%" "%ARDUINO_LIB_SRC%\" >nul
    if !errorlevel! EQU 0 (
        echo [OK] Copied %LIB_NAME% to Library
    ) else (
        echo [ERROR] Failed to copy %LIB_NAME%
    )
) else (
    echo [ERROR] Could not find compiled library at: %SOURCE_LIB%
)

:: 複製 C++ 介面標頭檔 (.h)
set "SOURCE_HEADER=%PROJECT_ROOT%%HEADER_NAME%"
if exist "%SOURCE_HEADER%" (
    copy /y "%SOURCE_HEADER%" "%ARDUINO_LIB_SRC%\" >nul
    if !errorlevel! EQU 0 (
        echo [OK] Copied %HEADER_NAME% to Library
    ) else (
        echo [ERROR] Failed to copy %HEADER_NAME%
    )
) else (
    echo [ERROR] Could not find header file at: %SOURCE_HEADER%
)

echo.
echo ========================================================
echo SUCCESS: Bridge synchronized!
echo Now you can click 'Verify/Upload' in Arduino IDE.
echo ========================================================
cd /d "%PROJECT_ROOT%"
pause
