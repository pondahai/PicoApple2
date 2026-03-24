@echo off
setlocal enabledelayedexpansion

:: --- [配置區] ---
set "FQBN=rp2040:rp2040:rpipico"
set "PROJECT_ROOT=%~dp0"
set "ARDUINO_CLI=%PROJECT_ROOT%arduino-cli.exe"
set "CUSTOM_LIB_PATH=C:\Users\Dell\Dropbox\Arduino\libraries"
set "TEST_SKETCH=sd_test.ino"

:: 使用固定編譯路徑來達成「預編譯快取」效果
set "BUILD_CACHE=%PROJECT_ROOT%build_cache_sd"
set "OUTPUT_DIR=%PROJECT_ROOT%dist_sd"

echo ========================================================
echo [PRECOMPILE] Starting Fast Build for SD Test...
echo ========================================================

if not exist "%BUILD_CACHE%" mkdir "%BUILD_CACHE%"
if not exist "%OUTPUT_DIR%" mkdir "%OUTPUT_DIR%"

:: 使用 --build-path 來保留編譯中間檔 (預編譯效果)
"%ARDUINO_CLI%" compile --fqbn %FQBN% ^
    --libraries "%CUSTOM_LIB_PATH%" ^
    --build-path "%BUILD_CACHE%" ^
    --output-dir "%OUTPUT_DIR%" ^
    "%PROJECT_ROOT%%TEST_SKETCH%"

if %errorlevel% neq 0 (
    echo.
    echo [ERROR] Compilation failed.
    pause
    exit /b %errorlevel%
)

echo.
echo ========================================================
echo [OK] Pre-compiled Binary ready in: %OUTPUT_DIR%
echo Binary: %TEST_SKETCH%.uf2 / %TEST_SKETCH%.elf
echo ========================================================

:: 問用戶是否要立即上傳
set /p "CHOICE=Do you want to upload now? (y/n): "
if /i "%CHOICE%"=="y" (
    call test_sd.bat
)

pause
