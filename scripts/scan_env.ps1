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
$libPath = Join-Path $arduinoUser "libraries"
Write-Host " [OK] Libraries   -> $libPath"

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
$content | Out-File -FilePath $outputBat -Encoding ascii -Force

Write-Host "--------------------------------------------------------"
Write-Host "Scan Complete! Settings saved to build_env.bat"
