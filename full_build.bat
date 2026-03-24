@echo off
setlocal enabledelayedexpansion

:: --- [配置區] ---
set "FQBN=rp2040:rp2040:rpipico"
set "PROJECT_ROOT=C:\Users\Dell\Documents\pico_apple2_emulator\"
set "RUST_PROJECT_DIR=%PROJECT_ROOT%apple2_core"
set "ARDUINO_CLI=%PROJECT_ROOT%arduino-cli.exe"
set "CUSTOM_LIB_PATH=C:\Users\Dell\Dropbox\Arduino\libraries"

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
:: 直接使用 --build-property 指定連結參數，指向當前目錄的 src
"%ARDUINO_CLI%" compile --fqbn %FQBN% --libraries "%CUSTOM_LIB_PATH%" ^
    --build-property "compiler.c.elf.extra_flags=\"-L%PROJECT_ROOT%src\" -lapple2_core" ^
    --output-dir . .
if %errorlevel% neq 0 ( echo [ERROR] Arduino build failed. & pause & exit /b )

echo.
echo [4/4] Uploading...
echo ========================================================
:: 強制關閉任何可能佔用串口的 PowerShell 或 CMD 監控視窗
powershell -NoProfile -Command "Get-CimInstance Win32_Process | Where-Object { $_.CommandLine -like '*serial_monitor.ps1*' } | ForEach-Object { Stop-Process -Id $_.ProcessId -Force }" >nul 2>&1
taskkill /FI "WINDOWTITLE eq Pico Terminal*" /T /F >nul 2>&1
timeout /t 1 /nobreak >nul

"%ARDUINO_CLI%" board list > boards.txt
set "T_COM="
for /f "tokens=1" %%a in ('findstr "rp2040" boards.txt') do (
    set "LINE=%%a"
    if "!LINE:~0,3!"=="COM" set "T_COM=!LINE!"
)
del boards.txt
if not "!T_COM!"=="" (
    powershell -NoProfile -Command "try { $p = New-Object System.IO.Ports.SerialPort '!T_COM!', 1200; $p.Open(); $p.Close(); } catch { }"
    timeout /t 3 >nul
)

C:\Users\Dell\AppData\Local\Arduino15\packages\rp2040\tools\pqt-picotool\4.1.0-1aec55e\picotool.exe load -x "pico_apple2_emulator.ino.elf"
if %errorlevel% equ 0 (
    echo SUCCESS! Starting Terminal...
    start "Pico Terminal" cmd /c "%PROJECT_ROOT%terminal.bat"
)
pause
