# Glass House CLI — One-command setup
# Run: powershell -ExecutionPolicy Bypass -File .\setup.ps1

param(
    [switch]$Global,
    [string]$Version = "1.0.0"
)

$ErrorActionPreference = "Stop"
Write-Host "Glass House CLI Setup v$Version" -ForegroundColor Cyan
Write-Host ""

$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path

# Check if glas.exe exists
$GlasPath = Join-Path $ScriptDir "glas.exe"

if (-not (Test-Path $GlasPath)) {
    Write-Host "Downloading glas.exe..." -ForegroundColor Yellow
    $url = "https://github.com/OniuUI/Glas-CLI/releases/latest/download/glas.exe"
    try {
        Invoke-WebRequest -Uri $url -OutFile $GlasPath -UseBasicParsing
        Write-Host "  Downloaded." -ForegroundColor Green
    } catch {
        Write-Host "  Could not download. Place glas.exe in this directory manually." -ForegroundColor Red
        Write-Host "  Run: rustc --edition 2021 src/main.rs -o glas.exe" -ForegroundColor Yellow
        exit 1
    }
} else {
    Write-Host "glas.exe found." -ForegroundColor Green
}

if ($Global) {
    $InstallDir = "$env:LOCALAPPDATA\GlassHouse"
    New-Item -ItemType Directory -Force -Path $InstallDir | Out-Null
    Copy-Item $GlasPath "$InstallDir\glas.exe" -Force

    $currentPath = [Environment]::GetEnvironmentVariable("Path", "User")
    if ($currentPath -notlike "*$InstallDir*") {
        [Environment]::SetEnvironmentVariable("Path", "$currentPath;$InstallDir", "User")
        Write-Host "Added to PATH. Restart your terminal for 'glas' to work globally." -ForegroundColor Green
    }
    Write-Host ""
    Write-Host "glas is now installed globally. Run:" -ForegroundColor Cyan
    Write-Host "  glas init my-app"
    Write-Host "  cd my-app"
    Write-Host "  glas serve"
} else {
    $TargetDir = $ScriptDir
    Write-Host ""
    Write-Host "To use glas globally, run:" -ForegroundColor Yellow
    Write-Host "  .\setup.ps1 -Global"
    Write-Host ""
    Write-Host "Or use it directly:" -ForegroundColor Cyan
    Write-Host "  $TargetDir\glas init my-app"
    Write-Host "  cd my-app"
    Write-Host "  $TargetDir\glas serve"
}
