@echo off
call "C:\Program Files\Microsoft Visual Studio\2022\Community\VC\Auxiliary\Build\vcvars64.bat" >nul
cd /d "%~dp0.."
cl /nologo /O2 /W3 /I src\uzlib scripts\test_archive.c src\uzlib\tinflate.c src\uzlib\tinfgzip.c src\uzlib\defl_static.c src\uzlib\genlz77.c src\uzlib\adler32.c src\uzlib\crc32.c /Fe:scripts\test_archive.exe /Foscripts\ 1>scripts\_clout.txt 2>&1
echo CL_EXIT=%errorlevel%
type scripts\_clout.txt
