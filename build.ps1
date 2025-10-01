# Build script for VedDB Server and Core (Windows)

Write-Host "Building VedDB Server and Core..." -ForegroundColor Green
cargo build --release
if ($LASTEXITCODE -ne 0) {
    Write-Host "Failed to build server and core" -ForegroundColor Red
    exit 1
}

Write-Host ""
Write-Host "Build complete!" -ForegroundColor Green
Write-Host "Server binary: target\release\veddb-server.exe" -ForegroundColor Cyan
Write-Host ""
Write-Host "Note: To build the client, go to the clients\rust-client directory and run 'cargo build --release'" -ForegroundColor Yellow
