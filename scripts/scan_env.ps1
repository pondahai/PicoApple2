# scan_env.ps1 - Dynamic toolchain and path discovery for PicoApple2
# Extracts paths from Arduino IDE settings to ensure build scripts are portable.

$arduinoConfigPath = "$HOME\.arduinoIDE\arduino-cli.yaml"
$projectRoot = Get-Item "." | Select-Object -ExpandProperty FullName
$outputBat = Join-Path $projectRoot "build_env.bat"

Write-Host "--- Scanning Arduino Environment ---"

if (-not (Test-Path $arduinoConfigPath)) {
    Write-Warning "Arduino IDE config not found at $arduinoConfigPath. Falling back to default paths."
    $arduinoData = "$env:LOCALAPPDATA\Arduino15"
    $arduinoUser = "$HOME\Documents\Arduino"
} else {
    $configLines = Get-Content $arduinoConfigPath
    $arduinoData = ($configLines | Select-String -Pattern "data: (.*)").Matches.Groups[1].Value.Trim()
    $arduinoUser = ($configLines | Select-String -Pattern "user: (.*)").Matches.Groups[1].Value.Trim()
}

Write-Host " Data Dir: $arduinoData"
Write-Host " User Dir: $arduinoUser"

# 1. Find arduino-cli.exe
$arduinoCli = Join-Path $projectRoot "arduino-cli.exe"
if (-not (Test-Path $arduinoCli)) {
    $arduinoCli = "C:\Program Files\Arduino IDE\resources\app\lib\backend\resources\arduino-cli.exe"
    if (-not (Test-Path $arduinoCli)) {
        $arduinoCli = Get-Command arduino-cli -ErrorAction SilentlyContinue | Select-Object -ExpandProperty Source
    }
}

if (-not (Test-Path $arduinoCli)) {
    Write-Error "arduino-cli.exe not found! Please ensure Arduino IDE 2 is installed."
    exit 1
}
Write-Host " [OK] Arduino CLI -> $arduinoCli"

# 2. Find picotool.exe
$picotool = Get-ChildItem -Path $arduinoData -Filter "picotool.exe" -Recurse -ErrorAction SilentlyContinue | 
            Sort-Object LastWriteTime -Descending | Select-Object -First 1 -ExpandProperty FullName

if (-not (Test-Path $picotool)) {
    Write-Error "picotool.exe not found! Please install the RP2040 board package in Arduino IDE."
    exit 1
}
Write-Host " [OK] Picotool    -> $picotool"

# 3. Resolve Library Path
#    NOTE (2026-08-27): PicoApple2 no longer depends on this path. full_build.bat
#    generates a self-contained Apple2Core library via loader_offset/make_arduino_lib.py.
#    We still emit it for other sketches, but warn loudly when it does not exist:
#    the sketchbook moved off Dropbox and arduino-cli.yaml user: was left stale,
#    which surfaced only as "Apple2Core.h: No such file or directory".
$libPath = Join-Path $arduinoUser "libraries"
if (Test-Path $libPath) {
    Write-Host " [OK] Libraries   -> $libPath"
} else {
    Write-Warning "Sketchbook libraries path does not exist: $libPath"
    Write-Warning "  arduino-cli.yaml user: likely points at a moved/deleted folder."
    Write-Warning "  PicoApple2 is unaffected (it uses a generated library); other sketches will fail."
}

# 4. Create build_env.bat
$rootWithSlash = $projectRoot
if (-not $rootWithSlash.EndsWith("\")) { $rootWithSlash += "\" }

$content = @(
    "@echo off",
    ":: AUTO-GENERATED - DO NOT EDIT",
    "set `"ARDUINO_CLI_PATH=$arduinoCli`"",
    "set `"PICOTOOL_PATH=$picotool`"",
    "set `"ARDUINO_USER_LIB_PATH=$libPath`"",
    "set `"PROJECT_ROOT=$rootWithSlash`"",
    "set `"FQBN=rp2040:rp2040:rpipico`""
)
# Write as OEM (the console codepage cmd.exe uses to parse .bat files), not ascii.
# The sketchbook now lives under a path with non-ASCII characters
# (G:\<CJK>\dropbox_dahai_pon\Arduino); -Encoding ascii turned every one of them
# into '?' and produced a silently broken ARDUINO_USER_LIB_PATH. Google Drive does
# not expose 8.3 short names, so there is no ASCII-only spelling of that path.
# Caveat: this assumes the caller's console is at the system default codepage.
# A console forced to 65001 (as build_offset.bat does) will mis-decode the line,
# which is harmless today because neither build script reads this variable.
$content | Out-File -FilePath $outputBat -Encoding oem -Force

Write-Host "--------------------------------------------------------"
Write-Host "Scan Complete! Settings saved to build_env.bat"
