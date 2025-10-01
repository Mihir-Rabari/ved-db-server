# Build MSI Installer for VedDB Server
# Requires: WiX Toolset v3.11+ (https://wixtoolset.org/)

param(
    [string]$Version = "0.1.1",
    [string]$OutputDir = ".\output"
)

Write-Host "`n=== Building VedDB MSI Installer ===" -ForegroundColor Cyan
Write-Host ""

# Check if WiX is installed
$wixPath = "${env:WIX}bin"
if (-not (Test-Path $wixPath)) {
    Write-Host "ERROR: WiX Toolset not found!" -ForegroundColor Red
    Write-Host "Please install WiX Toolset from: https://wixtoolset.org/" -ForegroundColor Yellow
    Write-Host "Or install via: choco install wixtoolset" -ForegroundColor Yellow
    exit 1
}

Write-Host "WiX Toolset found at: $wixPath" -ForegroundColor Green

# Step 1: Build the server
Write-Host "`n[1/5] Building VedDB server..." -ForegroundColor Yellow
Push-Location ..
cargo build --release -p veddb-server
if ($LASTEXITCODE -ne 0) {
    Write-Host "Build failed!" -ForegroundColor Red
    Pop-Location
    exit 1
}
Pop-Location
Write-Host "  Build successful" -ForegroundColor Green

# Step 2: Create output directory
Write-Host "`n[2/5] Preparing output directory..." -ForegroundColor Yellow
if (-not (Test-Path $OutputDir)) {
    New-Item -ItemType Directory -Path $OutputDir -Force | Out-Null
}
Write-Host "  Output directory: $OutputDir" -ForegroundColor Green

# Step 3: Create default config
Write-Host "`n[3/5] Creating default configuration..." -ForegroundColor Yellow
$configDir = "config"
if (-not (Test-Path $configDir)) {
    New-Item -ItemType Directory -Path $configDir -Force | Out-Null
}

$defaultConfig = @"
# VedDB Server Configuration
[server]
name = "veddb_main"
memory_mb = 256
workers = 4
port = 50051
session_timeout_secs = 300

[logging]
level = "info"
file = "logs/veddb.log"

[persistence]
enabled = false
wal_path = "data/wal"
snapshot_path = "data/snapshots"
"@

Set-Content -Path "$configDir\default.toml" -Value $defaultConfig
Write-Host "  Created default.toml" -ForegroundColor Green

# Step 4: Create icon (placeholder)
Write-Host "`n[4/5] Preparing assets..." -ForegroundColor Yellow
$assetsDir = "assets"
if (-not (Test-Path $assetsDir)) {
    New-Item -ItemType Directory -Path $assetsDir -Force | Out-Null
}

# Create a simple ICO file (you should replace this with a real icon)
if (-not (Test-Path "$assetsDir\veddb.ico")) {
    Write-Host "  Note: Using placeholder icon. Replace assets\veddb.ico with your logo" -ForegroundColor Yellow
    # Copy a system icon as placeholder
    Copy-Item "$env:SystemRoot\System32\imageres.dll" "$assetsDir\veddb.ico" -ErrorAction SilentlyContinue
}

# Step 5: Build MSI
Write-Host "`n[5/5] Building MSI package..." -ForegroundColor Yellow

# Compile WiX source
Write-Host "  Compiling WiX source..." -ForegroundColor Cyan
& "$wixPath\candle.exe" -nologo veddb.wxs -out "$OutputDir\veddb.wixobj"
if ($LASTEXITCODE -ne 0) {
    Write-Host "  Compilation failed!" -ForegroundColor Red
    exit 1
}

# Link to create MSI
Write-Host "  Linking MSI package..." -ForegroundColor Cyan
& "$wixPath\light.exe" -nologo "$OutputDir\veddb.wixobj" -out "$OutputDir\VedDB-$Version.msi" -ext WixUIExtension
if ($LASTEXITCODE -ne 0) {
    Write-Host "  Linking failed!" -ForegroundColor Red
    exit 1
}

# Summary
Write-Host "`n=== Build Complete ===" -ForegroundColor Green
Write-Host ""
Write-Host "MSI Installer created:" -ForegroundColor Cyan
Write-Host "  $OutputDir\VedDB-$Version.msi" -ForegroundColor White
Write-Host ""
Write-Host "File size:" -ForegroundColor Yellow
$msiFile = Get-Item "$OutputDir\VedDB-$Version.msi"
Write-Host "  $([math]::Round($msiFile.Length / 1MB, 2)) MB" -ForegroundColor White
Write-Host ""
Write-Host "To install:" -ForegroundColor Yellow
Write-Host "  Double-click the MSI file" -ForegroundColor White
Write-Host "  Or run: msiexec /i $OutputDir\VedDB-$Version.msi" -ForegroundColor White
Write-Host ""
Write-Host "To install silently:" -ForegroundColor Yellow
Write-Host "  msiexec /i $OutputDir\VedDB-$Version.msi /quiet /qn" -ForegroundColor White
Write-Host ""
