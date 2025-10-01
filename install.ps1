# VedDB Server Installation Script for Windows
# This script installs VedDB server and sets up environment variables

param(
    [string]$InstallPath = "$env:ProgramFiles\VedDB",
    [switch]$AddToPath = $true
)

Write-Host "`n=== VedDB Server Installation ===" -ForegroundColor Cyan
Write-Host ""

# Check if running as administrator
$isAdmin = ([Security.Principal.WindowsPrincipal] [Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
if (-not $isAdmin) {
    Write-Host "Warning: Not running as administrator. Environment variables will be set for current user only." -ForegroundColor Yellow
    $InstallPath = "$env:LOCALAPPDATA\VedDB"
}

# Step 1: Build the server
Write-Host "[1/5] Building VedDB server..." -ForegroundColor Yellow
cargo build --release -p veddb-server
if ($LASTEXITCODE -ne 0) {
    Write-Host "Build failed!" -ForegroundColor Red
    exit 1
}
Write-Host "  Build successful" -ForegroundColor Green

# Step 2: Create installation directory
Write-Host "`n[2/5] Creating installation directory..." -ForegroundColor Yellow
if (-not (Test-Path $InstallPath)) {
    New-Item -ItemType Directory -Path $InstallPath -Force | Out-Null
}
Write-Host "  Installation path: $InstallPath" -ForegroundColor Green

# Step 3: Copy binaries
Write-Host "`n[3/5] Installing binaries..." -ForegroundColor Yellow
Copy-Item "target\release\veddb-server.exe" -Destination "$InstallPath\" -Force
Write-Host "  Copied veddb-server.exe" -ForegroundColor Green

# Step 4: Set environment variables
Write-Host "`n[4/5] Setting environment variables..." -ForegroundColor Yellow

$scope = if ($isAdmin) { "Machine" } else { "User" }

# Set VEDDB_HOME
[Environment]::SetEnvironmentVariable("VEDDB_HOME", $InstallPath, $scope)
Write-Host "  VEDDB_HOME = $InstallPath" -ForegroundColor Green

# Set VEDDB_VERSION
$version = "0.1.0"
[Environment]::SetEnvironmentVariable("VEDDB_VERSION", $version, $scope)
Write-Host "  VEDDB_VERSION = $version" -ForegroundColor Green

# Set VEDDB_BIN
$binPath = $InstallPath
[Environment]::SetEnvironmentVariable("VEDDB_BIN", $binPath, $scope)
Write-Host "  VEDDB_BIN = $binPath" -ForegroundColor Green

# Step 5: Add to PATH
if ($AddToPath) {
    Write-Host "`n[5/5] Adding to PATH..." -ForegroundColor Yellow
    $currentPath = [Environment]::GetEnvironmentVariable("Path", $scope)
    if ($currentPath -notlike "*$InstallPath*") {
        $newPath = "$currentPath;$InstallPath"
        [Environment]::SetEnvironmentVariable("Path", $newPath, $scope)
        Write-Host "  Added to PATH" -ForegroundColor Green
    } else {
        Write-Host "  Already in PATH" -ForegroundColor Yellow
    }
}

# Create uninstall script
$uninstallScript = @"
# VedDB Uninstall Script
Write-Host "Uninstalling VedDB..." -ForegroundColor Yellow

# Remove installation directory
if (Test-Path "$InstallPath") {
    Remove-Item -Path "$InstallPath" -Recurse -Force
    Write-Host "Removed installation directory" -ForegroundColor Green
}

# Remove environment variables
`$scope = "$scope"
[Environment]::SetEnvironmentVariable("VEDDB_HOME", `$null, `$scope)
[Environment]::SetEnvironmentVariable("VEDDB_VERSION", `$null, `$scope)
[Environment]::SetEnvironmentVariable("VEDDB_BIN", `$null, `$scope)

# Remove from PATH
`$currentPath = [Environment]::GetEnvironmentVariable("Path", `$scope)
`$newPath = `$currentPath -replace [regex]::Escape("$InstallPath;?"), ""
[Environment]::SetEnvironmentVariable("Path", `$newPath, `$scope)

Write-Host "VedDB uninstalled successfully" -ForegroundColor Green
"@

Set-Content -Path "$InstallPath\uninstall.ps1" -Value $uninstallScript

# Summary
Write-Host "`n=== Installation Complete ===" -ForegroundColor Green
Write-Host ""
Write-Host "VedDB has been installed to: $InstallPath" -ForegroundColor Cyan
Write-Host ""
Write-Host "Environment variables set:" -ForegroundColor Yellow
Write-Host "  VEDDB_HOME    = $InstallPath"
Write-Host "  VEDDB_VERSION = $version"
Write-Host "  VEDDB_BIN     = $binPath"
Write-Host ""
Write-Host "To use VedDB, open a NEW terminal and run:" -ForegroundColor Yellow
Write-Host "  veddb-server --help" -ForegroundColor White
Write-Host ""
Write-Host "To uninstall, run:" -ForegroundColor Yellow
Write-Host "  $InstallPath\uninstall.ps1" -ForegroundColor White
Write-Host ""
