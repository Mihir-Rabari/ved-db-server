# 📥 Download VedDB

Choose your platform and download the installer:

## Windows 🪟

### MSI Installer (Recommended)
**Easy installation with GUI wizard**

[![Download for Windows](https://img.shields.io/badge/Download-Windows%20MSI-blue?style=for-the-badge&logo=windows)](https://github.com/mihir-Rabari/ved-db-server/releases/latest/download/VedDB-Setup.msi)

**Features:**
- ✅ One-click installation
- ✅ Automatic environment setup
- ✅ Optional Windows Service
- ✅ Start menu shortcuts

**Requirements:** Windows 10/11 or Windows Server 2016+

---

### Portable Version
**No installation required**

[![Download Portable](https://img.shields.io/badge/Download-Windows%20Portable-lightblue?style=for-the-badge&logo=windows)](https://github.com/mihir-Rabari/ved-db-server/releases/latest/download/veddb-server-windows.zip)

**Just extract and run!**

---

## Linux 🐧

### x64 (64-bit)

[![Download for Linux](https://img.shields.io/badge/Download-Linux%20x64-orange?style=for-the-badge&logo=linux)](https://github.com/mihir-Rabari/ved-db-server/releases/latest/download/veddb-server-linux-x64.tar.gz)

**Installation:**
```bash
wget https://github.com/mihir-Rabari/ved-db-server/releases/latest/download/veddb-server-linux-x64.tar.gz
tar -xzf veddb-server-linux-x64.tar.gz
cd veddb-server
sudo ./install.sh
```

**Supported Distributions:**
- Ubuntu 20.04+
- Debian 11+
- Fedora 35+
- Arch Linux
- CentOS/RHEL 8+

---

### ARM64 (Raspberry Pi, ARM servers)

[![Download for Linux ARM](https://img.shields.io/badge/Download-Linux%20ARM64-red?style=for-the-badge&logo=linux)](https://github.com/mihir-Rabari/ved-db-server/releases/latest/download/veddb-server-linux-arm64.tar.gz)

**Perfect for:**
- Raspberry Pi 4/5
- ARM-based cloud servers
- Edge devices

---

## macOS 🍎

### Intel (x64)

[![Download for macOS Intel](https://img.shields.io/badge/Download-macOS%20Intel-lightgrey?style=for-the-badge&logo=apple)](https://github.com/mihir-Rabari/ved-db-server/releases/latest/download/veddb-server-macos-x64.tar.gz)

**Installation:**
```bash
curl -LO https://github.com/mihir-Rabari/ved-db-server/releases/latest/download/veddb-server-macos-x64.tar.gz
tar -xzf veddb-server-macos-x64.tar.gz
cd veddb-server
sudo ./install.sh
```

---

### Apple Silicon (M1/M2/M3)

[![Download for macOS ARM](https://img.shields.io/badge/Download-macOS%20Apple%20Silicon-black?style=for-the-badge&logo=apple)](https://github.com/mihir-Rabari/ved-db-server/releases/latest/download/veddb-server-macos-arm64.tar.gz)

**Optimized for Apple Silicon chips**

---

## Docker 🐳

```bash
docker pull veddb/server:latest
```

**Run with:**
```bash
docker run -d -p 50051:50051 veddb/server:latest
```

[Docker Hub](https://hub.docker.com/r/veddb/server) | [Documentation](https://github.com/mihir-Rabari/ved-db-server/blob/main/docs/docker.md)

---

## Package Managers

### Coming Soon

```bash
# Windows (Chocolatey)
choco install veddb

# Windows (Winget)
winget install VedDB.Server

# macOS (Homebrew)
brew install veddb

# Linux (APT)
sudo apt install veddb-server

# Linux (DNF)
sudo dnf install veddb-server

# Arch Linux (AUR)
yay -S veddb-server
```

---

## Verify Your Download

### Windows
```powershell
# Check SHA256 checksum
Get-FileHash VedDB-Setup.msi
```

### Linux/macOS
```bash
# Check SHA256 checksum
sha256sum veddb-server-*.tar.gz

# Compare with checksums.txt
wget https://github.com/mihir-Rabari/ved-db-server/releases/latest/download/checksums.txt
cat checksums.txt
```

---

## Need Help?

- 📚 [Installation Guide](INSTALLATION_GUIDE.md)
- 📖 [Documentation](README.md)
- 💬 [Community Forum](https://github.com/mihir-Rabari/ved-db-server/discussions)
- 🐛 [Report Issues](https://github.com/mihir-Rabari/ved-db-server/issues)

---

## Version History

See all releases: [GitHub Releases](https://github.com/mihir-Rabari/ved-db-server/releases)

**Latest Version:** v0.1.0  
**Release Date:** 2024-01-15  
**Release Notes:** [What's New](https://github.com/mihir-Rabari/ved-db-server/releases/latest)

---

<div align="center">

**Thank you for choosing VedDB!** 🚀

[⬅️ Back to README](README.md)

</div>
