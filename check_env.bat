@echo off
setlocal enabledelayedexpansion
title PicoApple2 Environment Checker

echo ========================================================
echo   PicoApple2 Toolchain ^& Library Scanner
echo ========================================================
echo.

:: 執行掃描
set "SCRIPT_DIR=%~dp0"
powershell -ExecutionPolicy Bypass -File "%SCRIPT_DIR%scripts\scan_env.ps1"

if %errorlevel% neq 0 (
    echo.
    echo [ERROR] 掃描過程中發生錯誤！
    pause
    exit /b %errorlevel%
)

:: 載入產出的環境變數
if exist "%SCRIPT_DIR%build_env.bat" (
    call "%SCRIPT_DIR%build_env.bat"
) else (
    echo [ERROR] 找不到 build_env.bat，請確認掃描器是否正常運作。
    pause
    exit /b 1
)

echo.
echo --- Final Environment Variables Check ---
echo Project Root: %PROJECT_ROOT%
echo Arduino CLI:  %ARDUINO_CLI_PATH%
echo Picotool:     %PICOTOOL_PATH%
echo Library Path: %ARDUINO_USER_LIB_PATH%
echo Board FQBN:   %FQBN%
echo.
echo ========================================================
echo Scan Complete! You can now run any build scripts safely.
echo ========================================================
pause
