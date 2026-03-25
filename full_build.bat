@echo off
setlocal enabledelayedexpansion

:: --- [Dynamic Environment Setup] ---
set "SCRIPT_DIR=%~dp0"
echo Scanning environment...
powershell -ExecutionPolicy Bypass -File "%SCRIPT_DIR%scripts\scan_env.ps1"
if %errorlevel% neq 0 ( echo [ERROR] Environment scan failed. & pause & exit /b )
call "%SCRIPT_DIR%build_env.bat"

:: Use the variables from build_env.bat
set "FQBN=%FQBN%"
set "RUST_PROJECT_DIR=%PROJECT_ROOT%apple2_core"
set "ARDUINO_CLI=%ARDUINO_CLI_PATH%"
set "CUSTOM_LIB_PATH=%ARDUINO_USER_LIB_PATH%"
set "PICOTOOL=%PICOTOOL_PATH%"

echo ========================================================
echo [1/4] Compiling Rust Core...
echo ========================================================
cd /d "%RUST_PROJECT_DIR%"
cargo build --target thumbv6m-none-eabi --release
if %errorlevel% neq 0 ( echo [ERROR] Rust failed. & pause & exit /b )

echo.
echo [2/4] Syncing Library to Project Local...
echo ========================================================
if not exist "%PROJECT_ROOT%src" mkdir "%PROJECT_ROOT%src"
copy /y "%RUST_PROJECT_DIR%\target\thumbv6m-none-eabi\release\libapple2_core.a" "%PROJECT_ROOT%src\libapple2_core.a"
echo [OK] Static library synced to local src/

echo.
echo [3/4] Compiling Arduino Sketch (Direct Link Mode)...
echo ========================================================
cd /d "%PROJECT_ROOT%"
"%ARDUINO_CLI%" compile --fqbn %FQBN% --libraries "%CUSTOM_LIB_PATH%" ^
    --build-property "compiler.c.elf.extra_flags=\"-L%PROJECT_ROOT%src\" -lapple2_core" ^
    --output-dir . "PicoApple2.ino"
if %errorlevel% neq 0 ( echo [ERROR] Arduino build failed. & pause & exit /b )

echo.
echo [4/4] Uploading...
echo ========================================================
:: 釋放串口佔用
powershell -NoProfile -Command "Get-CimInstance Win32_Process | Where-Object { $_.CommandLine -like '*serial_monitor.ps1*' } | ForEach-Object { Stop-Process -Id $_.ProcessId -Force }" >nul 2>&1
taskkill /FI "WINDOWTITLE eq Pico Terminal*" /T /F >nul 2>&1
timeout /t 1 /nobreak >nul

:: 還原最原始的偵測邏輯
"%ARDUINO_CLI%" board list > boards.txt
set "T_COM="
for /f "tokens=1" %%a in ('findstr "rp2040" boards.txt') do (
    set "LINE=%%a"
    if "!LINE:~0,3!"=="COM" set "T_COM=!LINE!"
)
del boards.txt

if not "!T_COM!"=="" (
    echo Found Pico on !T_COM!, sending 1200bps reset signal...
    powershell -NoProfile -Command "try { $p = New-Object System.IO.Ports.SerialPort '!T_COM!', 1200; $p.Open(); $p.Close(); } catch { }"
    timeout /t 3 >nul
)

:: 執行上傳
"%PICOTOOL%" load -x "PicoApple2.ino.elf"
if %errorlevel% equ 0 (
    echo SUCCESS! Starting Terminal...
    start "Pico Terminal" cmd /c "%PROJECT_ROOT%terminal.bat"
)
pause
