# 🔨 Building the VedDB MSI Installer

This guide will walk you through building the VedDB Windows MSI installer.

## Prerequisites

### 1. Install WiX Toolset

**Option A: Using Chocolatey (Recommended)**
```powershell
# Install Chocolatey if you don't have it
# See: https://chocolatey.org/install

# Install WiX Toolset
choco install wixtoolset -y

# Verify installation
candle -?
```

**Option B: Manual Installation**
1. Download WiX Toolset v3.11.2 from: https://github.com/wixtoolset/wix3/releases
2. Run the installer: `wix311.exe`
3. Restart your terminal

**Option C: Using .NET CLI (WiX v4)**
```powershell
dotnet tool install --global wix
```

### 2. Verify WiX Installation

```powershell
# Check if WiX is in PATH
$env:WIX
# Should output: C:\Program Files (x86)\WiX Toolset v3.11\

# Check tools are available
candle -?
light -?
```

### 3. Install Rust (if not already installed)

```powershell
# Download and install from: https://rustup.rs/
# Or use chocolatey
choco install rust -y

# Verify
rustc --version
cargo --version
```

---

## Building the MSI

### Quick Build (Automated)

```powershell
# Navigate to installer directory
cd ved-db/installer

# Run the build script
./build-msi.ps1

# The MSI will be created at: output/VedDB-0.1.0.msi
```

That's it! The script automatically:
- ✅ Builds the server binary
- ✅ Creates default configuration
- ✅ Compiles WiX source
- ✅ Generates the MSI package

---

## Manual Build (Step-by-Step)

### Step 1: Build the Server Binary

```powershell
# Go to ved-db root
cd ved-db

# Build in release mode
cargo build --release -p veddb-server

# Verify binary exists
Test-Path target/release/veddb-server.exe
```

### Step 2: Prepare Assets

```powershell
cd installer

# Create config directory
New-Item -ItemType Directory -Path config -Force

# Create default configuration
@"
[server]
name = "veddb_main"
memory_mb = 256
workers = 4
port = 50051
"@ | Out-File -FilePath config/default.toml -Encoding UTF8

# Create assets directory for icon
New-Item -ItemType Directory -Path assets -Force

# Add your icon file (veddb.ico) to assets/
# You can use any .ico file or create one from an image
```

### Step 3: Compile WiX Source

```powershell
# Compile the WiX source file
candle.exe veddb.wxs -out output/veddb.wixobj

# Check for errors
if ($LASTEXITCODE -ne 0) {
    Write-Host "Compilation failed!" -ForegroundColor Red
    exit 1
}
```

### Step 4: Link to Create MSI

```powershell
# Link the compiled object to create MSI
light.exe output/veddb.wixobj -out output/VedDB-0.1.0.msi -ext WixUIExtension

# Check for errors
if ($LASTEXITCODE -ne 0) {
    Write-Host "Linking failed!" -ForegroundColor Red
    exit 1
}
```

### Step 5: Verify the MSI

```powershell
# Check MSI file
Get-Item output/VedDB-0.1.0.msi | Select-Object Name, Length, LastWriteTime

# Test install (silent, no actual installation)
msiexec /i output/VedDB-0.1.0.msi /qn /norestart INSTALLDIR="C:\Temp\VedDBTest"
```

---

## Customizing the MSI

### Change Product Details

Edit `veddb.wxs`:

```xml
<Product Id="*" 
         Name="VedDB Server"           <!-- Change product name -->
         Version="0.1.0"                <!-- Change version -->
         Manufacturer="Your Company">   <!-- Change manufacturer -->
```

### Change Installation Directory

```xml
<Directory Id="ProgramFilesFolder">
  <Directory Id="INSTALLDIR" Name="VedDB">  <!-- Change folder name -->
```

### Add More Files

```xml
<Component Id="MyFile" Guid="*">
  <File Id="myfile.txt" Source="path\to\myfile.txt" KeyPath="yes" />
</Component>
```

### Configure Default Settings

```xml
<Property Id="MEMORY_SIZE" Value="512" />     <!-- Change default memory -->
<Property Id="WORKER_COUNT" Value="8" />       <!-- Change default workers -->
<Property Id="SERVER_PORT" Value="50051" />    <!-- Change default port -->
```

---

## Advanced Options

### Building with Custom Version

```powershell
# Set version in environment
$env:PRODUCT_VERSION = "0.2.0"

# Build with version
./build-msi.ps1 -Version "0.2.0"
```

### Building for Distribution

```powershell
# Build release MSI
./build-msi.ps1 -Version "0.1.0" -OutputDir "releases"

# Sign the MSI (requires code signing certificate)
signtool sign /f "certificate.pfx" /p "password" /t http://timestamp.digicert.com releases/VedDB-0.1.0.msi
```

### Creating MSI with Transforms (MST)

```powershell
# Create a transform for custom deployments
msitran -g output/VedDB-0.1.0.msi custom.msi -o custom.mst

# Apply transform during installation
msiexec /i VedDB-0.1.0.msi TRANSFORMS=custom.mst
```

---

## Troubleshooting

### Error: "WiX Toolset not found"

**Solution:**
```powershell
# Check if WIX environment variable is set
$env:WIX

# If not set, add to PATH manually
$env:Path += ";C:\Program Files (x86)\WiX Toolset v3.11\bin"

# Or restart terminal after installation
```

### Error: "candle.exe: error LGHT0103"

**Solution:** Missing WiX extension

```powershell
# Add -ext flag
candle.exe veddb.wxs -ext WixUtilExtension
light.exe veddb.wixobj -ext WixUIExtension -ext WixUtilExtension
```

### Error: "Binary not found"

**Solution:** Build the server first

```powershell
cd ..
cargo build --release -p veddb-server
cd installer
```

### Error: "Access Denied" during build

**Solution:** Run PowerShell as Administrator

```powershell
# Right-click PowerShell -> Run as Administrator
cd ved-db/installer
./build-msi.ps1
```

### Warning: "ICE" warnings during build

These are validation warnings from Windows Installer. Most can be ignored during development.

To suppress:
```powershell
light.exe veddb.wixobj -sice:ICE61 -sice:ICE69 -out VedDB.msi
```

---

## Testing the MSI

### Test Installation

```powershell
# Install to test location
msiexec /i output/VedDB-0.1.0.msi /qb INSTALLDIR="C:\Temp\VedDBTest"

# Verify installation
Test-Path "C:\Temp\VedDBTest\veddb-server.exe"

# Check environment variables
$env:VEDDB_HOME

# Uninstall
msiexec /x output/VedDB-0.1.0.msi /qb
```

### Test in Virtual Machine

1. Create a clean Windows VM
2. Copy the MSI to the VM
3. Install and test
4. Check all features work
5. Uninstall and verify cleanup

---

## Automating Builds

### GitHub Actions

Create `.github/workflows/build-msi.yml`:

```yaml
name: Build MSI

on:
  push:
    tags:
      - 'v*'

jobs:
  build:
    runs-on: windows-latest
    steps:
      - uses: actions/checkout@v3
      
      - name: Install WiX
        run: choco install wixtoolset -y
      
      - name: Install Rust
        uses: actions-rs/toolchain@v1
        with:
          toolchain: stable
      
      - name: Build MSI
        run: |
          cd ved-db/installer
          ./build-msi.ps1
      
      - name: Upload Artifact
        uses: actions/upload-artifact@v3
        with:
          name: VedDB-MSI
          path: ved-db/installer/output/*.msi
```

### Local Automation Script

```powershell
# build-release.ps1
param([string]$Version = "0.1.0")

Write-Host "Building VedDB Release $Version"

# Build server
cd ved-db
cargo build --release

# Build MSI
cd installer
./build-msi.ps1 -Version $Version

# Create checksum
$hash = Get-FileHash "output/VedDB-$Version.msi" -Algorithm SHA256
$hash.Hash | Out-File "output/VedDB-$Version.msi.sha256"

# Create release package
Compress-Archive -Path "output/VedDB-$Version.msi" -DestinationPath "output/VedDB-$Version-Windows.zip"

Write-Host "Release package created!"
```

---

## Distribution Checklist

Before releasing the MSI:

- [ ] Test on clean Windows 10
- [ ] Test on clean Windows 11
- [ ] Test on Windows Server
- [ ] Verify service installation works
- [ ] Verify uninstallation is clean
- [ ] Test upgrade from previous version
- [ ] Sign the MSI with code signing certificate
- [ ] Create SHA256 checksum
- [ ] Test silent installation
- [ ] Verify Start Menu shortcuts work
- [ ] Check environment variables are set
- [ ] Test on both x64 and ARM64 (if supported)

---

## Resources

- **WiX Documentation**: https://wixtoolset.org/documentation/
- **WiX Tutorial**: https://www.firegiant.com/wix/tutorial/
- **MSI Reference**: https://docs.microsoft.com/en-us/windows/win32/msi/
- **Signing Code**: https://docs.microsoft.com/en-us/windows/win32/seccrypto/signtool

---

## Quick Reference

```powershell
# Full build command
cd ved-db/installer
./build-msi.ps1

# Test install
msiexec /i output/VedDB-0.1.0.msi /qb

# Silent install
msiexec /i output/VedDB-0.1.0.msi /quiet /qn

# Uninstall
msiexec /x output/VedDB-0.1.0.msi /qb

# View MSI properties
msiexec /i output/VedDB-0.1.0.msi /qn /l*v install.log
```

---

<div align="center">

**Need Help?** Open an issue on [GitHub](https://github.com/yourusername/veddb/issues)

[⬅️ Back to Installer README](README.md)

</div>
