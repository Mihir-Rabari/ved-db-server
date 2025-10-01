#!/bin/bash
# VedDB Server Installation Script for Unix/Linux/macOS

set -e

# Configuration
INSTALL_PREFIX="${INSTALL_PREFIX:-/usr/local}"
INSTALL_BIN="$INSTALL_PREFIX/bin"
INSTALL_LIB="$INSTALL_PREFIX/lib/veddb"
VERSION="0.1.0"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
NC='\033[0m' # No Color

echo -e "${CYAN}\n=== VedDB Server Installation ===${NC}\n"

# Check if running as root
if [ "$EUID" -ne 0 ] && [ "$INSTALL_PREFIX" = "/usr/local" ]; then
    echo -e "${YELLOW}Warning: Not running as root. Installing to ~/.local${NC}"
    INSTALL_PREFIX="$HOME/.local"
    INSTALL_BIN="$INSTALL_PREFIX/bin"
    INSTALL_LIB="$INSTALL_PREFIX/lib/veddb"
fi

# Step 1: Build the server
echo -e "${YELLOW}[1/5] Building VedDB server...${NC}"
cargo build --release -p veddb-server
if [ $? -ne 0 ]; then
    echo -e "${RED}Build failed!${NC}"
    exit 1
fi
echo -e "${GREEN}  Build successful${NC}"

# Step 2: Create installation directories
echo -e "\n${YELLOW}[2/5] Creating installation directories...${NC}"
mkdir -p "$INSTALL_BIN"
mkdir -p "$INSTALL_LIB"
echo -e "${GREEN}  Installation path: $INSTALL_PREFIX${NC}"

# Step 3: Copy binaries
echo -e "\n${YELLOW}[3/5] Installing binaries...${NC}"
cp target/release/veddb-server "$INSTALL_BIN/"
chmod +x "$INSTALL_BIN/veddb-server"
echo -e "${GREEN}  Copied veddb-server to $INSTALL_BIN${NC}"

# Step 4: Set environment variables
echo -e "\n${YELLOW}[4/5] Setting environment variables...${NC}"

# Determine shell config file
if [ -n "$ZSH_VERSION" ]; then
    SHELL_CONFIG="$HOME/.zshrc"
elif [ -n "$BASH_VERSION" ]; then
    if [ -f "$HOME/.bashrc" ]; then
        SHELL_CONFIG="$HOME/.bashrc"
    else
        SHELL_CONFIG="$HOME/.bash_profile"
    fi
else
    SHELL_CONFIG="$HOME/.profile"
fi

# Add environment variables to shell config
ENV_VARS="
# VedDB Environment Variables
export VEDDB_HOME=\"$INSTALL_PREFIX\"
export VEDDB_VERSION=\"$VERSION\"
export VEDDB_BIN=\"$INSTALL_BIN\"
export PATH=\"\$VEDDB_BIN:\$PATH\"
"

# Check if already added
if ! grep -q "VEDDB_HOME" "$SHELL_CONFIG" 2>/dev/null; then
    echo "$ENV_VARS" >> "$SHELL_CONFIG"
    echo -e "${GREEN}  Added environment variables to $SHELL_CONFIG${NC}"
else
    echo -e "${YELLOW}  Environment variables already in $SHELL_CONFIG${NC}"
fi

echo -e "${GREEN}  VEDDB_HOME = $INSTALL_PREFIX${NC}"
echo -e "${GREEN}  VEDDB_VERSION = $VERSION${NC}"
echo -e "${GREEN}  VEDDB_BIN = $INSTALL_BIN${NC}"

# Step 5: Create uninstall script
echo -e "\n${YELLOW}[5/5] Creating uninstall script...${NC}"
cat > "$INSTALL_BIN/veddb-uninstall" << 'EOF'
#!/bin/bash
# VedDB Uninstall Script

echo "Uninstalling VedDB..."

# Remove binaries
rm -f "$VEDDB_BIN/veddb-server"
rm -f "$VEDDB_BIN/veddb-uninstall"

# Remove lib directory
rm -rf "$VEDDB_HOME/lib/veddb"

# Remove environment variables from shell config
for config in ~/.bashrc ~/.bash_profile ~/.zshrc ~/.profile; do
    if [ -f "$config" ]; then
        sed -i.bak '/# VedDB Environment Variables/,/^$/d' "$config"
    fi
done

echo "VedDB uninstalled successfully"
echo "Please restart your shell or run: source ~/.bashrc (or your shell config)"
EOF

chmod +x "$INSTALL_BIN/veddb-uninstall"
echo -e "${GREEN}  Created uninstall script${NC}"

# Summary
echo -e "\n${GREEN}=== Installation Complete ===${NC}\n"
echo -e "${CYAN}VedDB has been installed to: $INSTALL_PREFIX${NC}\n"
echo -e "${YELLOW}Environment variables set:${NC}"
echo -e "  VEDDB_HOME    = $INSTALL_PREFIX"
echo -e "  VEDDB_VERSION = $VERSION"
echo -e "  VEDDB_BIN     = $INSTALL_BIN"
echo ""
echo -e "${YELLOW}To use VedDB, restart your terminal or run:${NC}"
echo -e "  ${NC}source $SHELL_CONFIG${NC}"
echo ""
echo -e "${YELLOW}Then you can run:${NC}"
echo -e "  ${NC}veddb-server --help${NC}"
echo ""
echo -e "${YELLOW}To uninstall, run:${NC}"
echo -e "  ${NC}veddb-uninstall${NC}"
echo ""
