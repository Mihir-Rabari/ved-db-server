# VedDB MSI Installer

This directory contains the Windows Installer (MSI) package configuration for VedDB Server.

## Prerequisites

### Required
- **WiX Toolset v3.11+**: Download from [wixtoolset.org](https://wixtoolset.org/)
- **Rust toolchain**: For building the server binary

### Installation via Chocolatey (Recommended)
```powershell
choco install wixtoolset
```

### Manual Installation
1. Download WiX Toolset from https://wixtoolset.org/releases/
2. Run the installer
3. Add WiX bin directory to PATH (usually done automatically)

## Building the MSI

### Quick Build
```powershell
cd installer
./build-msi.ps1
```

### Custom Version
```powershell
./build-msi.ps1 -Version "0.2.0"
```

### Custom Output Directory
```powershell
./build-msi.ps1 -OutputDir "C:\Builds\VedDB"
```

## MSI Features

### Installation Options
The MSI installer provides:
- **GUI Installation**: User-friendly wizard
- **Silent Installation**: Automated deployment
- **Custom Installation Path**: Choose install location
- **Service Installation**: Optional Windows Service
- **Environment Variables**: Automatic setup
- **Start Menu Shortcuts**: Easy access
- **Uninstaller**: Clean removal

### Configurable Parameters
During installation, you can configure:
- Installation directory
- Database name
- Memory size (MB)
- Worker thread count
- Server port
- Install as Windows Service (Yes/No)

## Installation Methods

### 1. GUI Installation (Recommended for Desktop)
```powershell
# Double-click the MSI file
# Or run:
.\output\VedDB-0.1.0.msi
```

**Installation Wizard Steps:**
1. Welcome screen
2. License agreement
3. Installation directory selection
4. Configuration options:
   - Database name (default: veddb_main)
   - Memory size (default: 256 MB)
   - Worker threads (default: 4)
   - Server port (default: 50051)
   - Install as service (default: Yes)
5. Ready to install
6. Installation progress
7. Completion

### 2. Silent Installation (For Automation/Deployment)
```powershell
# Basic silent install
msiexec /i VedDB-0.1.0.msi /quiet /qn

# Silent install with custom path
msiexec /i VedDB-0.1.0.msi /quiet /qn INSTALLDIR="C:\VedDB"

# Silent install with custom configuration
msiexec /i VedDB-0.1.0.msi /quiet /qn ^
  INSTALLDIR="C:\VedDB" ^
  DB_NAME="production_db" ^
  MEMORY_SIZE="1024" ^
  WORKER_COUNT="8" ^
  SERVER_PORT="50051" ^
  INSTALL_SERVICE="1"

# Silent install with logging
msiexec /i VedDB-0.1.0.msi /quiet /qn /l*v install.log
```

### 3. Administrative Installation
```powershell
# Extract files without installing
msiexec /a VedDB-0.1.0.msi /qb TARGETDIR="C:\ExtractedFiles"
```

## MSI Properties

You can customize the installation using these properties:

| Property | Description | Default | Example |
|----------|-------------|---------|---------|
| `INSTALLDIR` | Installation directory | `C:\Program Files\VedDB` | `C:\VedDB` |
| `DB_NAME` | Database instance name | `veddb_main` | `production_db` |
| `MEMORY_SIZE` | Memory size in MB | `256` | `1024` |
| `WORKER_COUNT` | Number of worker threads | `4` | `8` |
| `SERVER_PORT` | Server port number | `50051` | `50051` |
| `INSTALL_SERVICE` | Install as Windows Service | `1` | `0` (no service) |

## Environment Variables

The installer automatically sets these environment variables:

- `VEDDB_HOME`: Installation directory
- `VEDDB_VERSION`: Installed version
- `VEDDB_BIN`: Binary directory
- `PATH`: Updated to include VedDB binaries

## Windows Service

If you choose to install as a service:

### Service Details
- **Name**: VedDBServer
- **Display Name**: VedDB Server
- **Description**: VedDB High-Performance Database Server
- **Startup Type**: Automatic
- **Account**: Local System

### Service Management
```powershell
# Start service
net start VedDBServer
# Or
sc start VedDBServer

# Stop service
net stop VedDBServer
# Or
sc stop VedDBServer

# Check status
sc query VedDBServer

# View service configuration
sc qc VedDBServer
```

## Uninstallation

### GUI Uninstall
1. Open "Add or Remove Programs"
2. Find "VedDB Server"
3. Click "Uninstall"

### Silent Uninstall
```powershell
# Find product code
wmic product where "name='VedDB Server'" get IdentifyingNumber

# Uninstall using product code
msiexec /x {PRODUCT-CODE-GUID} /quiet /qn

# Or uninstall using MSI file
msiexec /x VedDB-0.1.0.msi /quiet /qn
```

## Upgrade Installation

To upgrade to a newer version:

### GUI Upgrade
1. Run the new MSI installer
2. It will automatically detect and upgrade the existing installation

### Silent Upgrade
```powershell
msiexec /i VedDB-0.2.0.msi /quiet /qn
```

The installer handles:
- Stopping the service (if running)
- Backing up configuration
- Upgrading binaries
- Preserving data
- Restarting the service

## Deployment Scenarios

### 1. Single Server Deployment
```powershell
# Install on a single server
msiexec /i VedDB-0.1.0.msi /quiet /qn ^
  INSTALLDIR="C:\VedDB" ^
  DB_NAME="main_db" ^
  MEMORY_SIZE="2048" ^
  WORKER_COUNT="8"
```

### 2. Multiple Instances on Same Server
```powershell
# Instance 1
msiexec /i VedDB-0.1.0.msi /quiet /qn ^
  INSTALLDIR="C:\VedDB\Instance1" ^
  DB_NAME="db1" ^
  SERVER_PORT="50051" ^
  INSTALL_SERVICE="0"

# Instance 2
msiexec /i VedDB-0.1.0.msi /quiet /qn ^
  INSTALLDIR="C:\VedDB\Instance2" ^
  DB_NAME="db2" ^
  SERVER_PORT="50052" ^
  INSTALL_SERVICE="0"
```

### 3. Enterprise Deployment (GPO)
1. Place MSI on network share
2. Create Group Policy Object
3. Assign/Publish the MSI
4. Configure installation properties

### 4. Docker-like Deployment
```powershell
# Install to temporary location
msiexec /i VedDB-0.1.0.msi /quiet /qn ^
  INSTALLDIR="C:\Temp\VedDB" ^
  INSTALL_SERVICE="0"

# Copy to container/VM
Copy-Item -Recurse "C:\Temp\VedDB" -Destination "\\server\share\"
```

## Troubleshooting

### Installation Fails
```powershell
# Check Windows Installer log
msiexec /i VedDB-0.1.0.msi /l*v install.log
notepad install.log
```

### Service Won't Start
```powershell
# Check service status
sc query VedDBServer

# Check event log
Get-EventLog -LogName Application -Source "VedDBServer" -Newest 10

# Try manual start
C:\Program Files\VedDB\bin\veddb-server.exe --create --name test_db
```

### Environment Variables Not Set
```powershell
# Refresh environment
$env:Path = [System.Environment]::GetEnvironmentVariable("Path","Machine")

# Or restart terminal/computer
```

### Permission Issues
- Run installer as Administrator
- Check folder permissions
- Verify service account has necessary rights

## Advanced Configuration

### Custom Transform (MST)
Create a transform file for custom deployments:
```powershell
# Create transform
msiexec /i VedDB-0.1.0.msi /qb TRANSFORMS=custom.mst

# Apply transform during install
msiexec /i VedDB-0.1.0.msi TRANSFORMS=custom.mst /quiet
```

### Patch Installation (MSP)
Apply patches to existing installation:
```powershell
msiexec /p VedDB-0.1.1-patch.msp /quiet
```

## Building Custom MSI

### Modify Configuration
1. Edit `veddb.wxs` for custom features
2. Update version numbers
3. Add/remove components
4. Customize UI dialogs

### Add Custom Actions
```xml
<CustomAction Id="MyAction" 
              BinaryKey="CustomActionDLL" 
              DllEntry="MyFunction" 
              Execute="deferred" />
```

### Include Additional Files
```xml
<Component Id="MyFile" Guid="*">
  <File Id="myfile.txt" Source="path\to\myfile.txt" />
</Component>
```

## Support

For issues or questions:
- Check the main [README](../README.md)
- Review [Installation Guide](../../INSTALLATION_GUIDE.md)
- Open an issue on GitHub

## License

Same as VedDB Server - MIT License

---

**Happy Installing!** 🚀
