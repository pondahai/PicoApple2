@echo off
setlocal enabledelayedexpansion

:: --- [Dynamic Environment Setup] ---
set "SCRIPT_DIR=%~dp0"
echo Scanning environment...
powershell -ExecutionPolicy Bypass -File "%SCRIPT_DIR%scripts\scan_env.ps1"
if %errorlevel% neq 0 ( echo [ERROR] Environment scan failed. & pause & exit /b )
call "%SCRIPT_DIR%build_env.bat"

:: Use variables from build_env.bat
set "FQBN=%FQBN%"
set "ARDUINO_CLI=%ARDUINO_CLI_PATH%"
set "CUSTOM_LIB_PATH=%ARDUINO_USER_LIB_PATH%"
set "PICOTOOL=%PICOTOOL_PATH%"
set "TEST_SKETCH=sd_test\sd_test.ino"
set "OUTPUT_DIR=build_sd_test"

echo ========================================================
echo [1/4] Compiling SD Test Sketch (Pre-linked Core)...
echo ========================================================
if not exist "%OUTPUT_DIR%" mkdir "%OUTPUT_DIR%"

"%ARDUINO_CLI%" compile --fqbn %FQBN% --libraries "%CUSTOM_LIB_PATH%" ^
    --build-property "compiler.c.elf.extra_flags=\"-L%PROJECT_ROOT%src\" -lapple2_core" ^
    --output-dir "%OUTPUT_DIR%" "%PROJECT_ROOT%%TEST_SKETCH%"

if %errorlevel% neq 0 (
    echo [ERROR] Compilation failed.
    pause
    exit /b %errorlevel%
)

echo.
echo [2/4] Terminating Monitors...
echo ========================================================
powershell -NoProfile -Command "Get-CimInstance Win32_Process | Where-Object { $_.CommandLine -like '*serial_monitor.ps1*' } | ForEach-Object { Stop-Process -Id $_.ProcessId -Force }" >nul 2>&1
taskkill /FI "WINDOWTITLE eq Pico Terminal*" /T /F >nul 2>&1
timeout /t 1 /nobreak >nul

echo.
echo [3/4] MANDATORY 1200bps Reset (Safety First)...
echo ========================================================
:: 自動偵測並嘗試重置所有 RP2040 埠
for /f "tokens=1" %%a in ('%ARDUINO_CLI% board list ^| findstr "rp2040"') do (
    set "COM_PORT=%%a"
    if "!COM_PORT:~0,3!"=="COM" (
        echo Found Pico on !COM_PORT!, sending 1200bps reset signal...
        powershell -NoProfile -Command "$p = New-Object System.IO.Ports.SerialPort '!COM_PORT!', 1200; try { $p.Open(); $p.Close(); } catch { }"
    )
)
echo Waiting 5 seconds for Pico to enter BOOTSEL mode...
timeout /t 5 /nobreak >nul

echo.
echo [4/4] Uploading via picotool...
echo ========================================================
"%PICOTOOL%" load -x "%OUTPUT_DIR%\sd_test.ino.elf" -x

if %errorlevel% equ 0 (
    echo SUCCESS! Starting Terminal Monitor...
    start "Pico Terminal" cmd /c "%PROJECT_ROOT%terminal.bat"
) else (
    echo [ERROR] Upload failed. Pico may not have entered BOOTSEL mode correctly.
    pause
)
