@echo off
:: Compile-only check (no Rust rebuild, no upload). Uses prebuilt src\libapple2_core.a.
setlocal
set "ROOT=%~dp0.."
call "%ROOT%\build_env.bat"
cd /d "%ROOT%"
"%ARDUINO_CLI_PATH%" compile --fqbn %FQBN% --libraries "%ARDUINO_USER_LIB_PATH%" ^
    --build-property "compiler.c.elf.extra_flags=\"-L%PROJECT_ROOT%src\" -lapple2_core" ^
    --output-dir . "PicoApple2.ino"
echo COMPILE_EXIT=%errorlevel%
