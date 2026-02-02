#!/bin/sh
# Recursor Installer
# One-liner: curl -fsSL https://raw.githubusercontent.com/adityasingh2400/Recursor/main/install.sh | sh

set -e

REPO="adityasingh2400/Recursor"
INSTALL_DIR="$HOME/.cursor/bin"
HOOKS_FILE="$HOME/.cursor/hooks.json"
BINARY_NAME="recursor"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

info() { printf "${BLUE}[INFO]${NC} %s\n" "$1"; }
success() { printf "${GREEN}[OK]${NC} %s\n" "$1"; }
warn() { printf "${YELLOW}[WARN]${NC} %s\n" "$1"; }
error() { printf "${RED}[ERROR]${NC} %s\n" "$1"; exit 1; }

# Detect OS
detect_os() {
    case "$(uname -s)" in
        Darwin*) OS="macos" ;;
        Linux*) OS="linux" ;;
        MINGW*|MSYS*|CYGWIN*) OS="windows" ;;
        *) error "Unsupported OS: $(uname -s)" ;;
    esac
}

# Detect architecture
detect_arch() {
    case "$(uname -m)" in
        x86_64|amd64) ARCH="x86_64" ;;
        arm64|aarch64) ARCH="aarch64" ;;
        *) error "Unsupported architecture: $(uname -m)" ;;
    esac
}

# Get download URL
get_download_url() {
    case "$OS" in
        macos) BINARY_FILE="recursor-macos-universal" ;;
        linux) BINARY_FILE="recursor-linux-${ARCH}" ;;
        windows) BINARY_FILE="recursor-windows-${ARCH}.exe" ;;
    esac
    DOWNLOAD_URL="https://github.com/${REPO}/releases/latest/download/${BINARY_FILE}"
}

# Download binary
download_binary() {
    info "Downloading Recursor..."
    mkdir -p "$INSTALL_DIR"
    
    DEST="$INSTALL_DIR/$BINARY_NAME"
    [ "$OS" = "windows" ] && DEST="$INSTALL_DIR/${BINARY_NAME}.exe"
    
    if command -v curl >/dev/null 2>&1; then
        if ! curl -fsSL "$DOWNLOAD_URL" -o "$DEST" 2>/dev/null; then
            warn "No release found. Building from source..."
            build_from_source
            return
        fi
    elif command -v wget >/dev/null 2>&1; then
        if ! wget -q "$DOWNLOAD_URL" -O "$DEST" 2>/dev/null; then
            warn "No release found. Building from source..."
            build_from_source
            return
        fi
    else
        error "curl or wget required"
    fi
    
    chmod +x "$DEST"
    success "Installed to $DEST"
}

# Build from source
build_from_source() {
    info "Building from source..."
    
    if ! command -v cargo >/dev/null 2>&1; then
        info "Installing Rust..."
        curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
        . "$HOME/.cargo/env"
    fi
    
    TEMP_DIR=$(mktemp -d)
    cd "$TEMP_DIR"
    
    git clone --depth 1 "https://github.com/${REPO}.git" recursor
    cd recursor
    cargo build --release
    
    DEST="$INSTALL_DIR/$BINARY_NAME"
    [ "$OS" = "windows" ] && DEST="$INSTALL_DIR/${BINARY_NAME}.exe"
    
    cp "target/release/$BINARY_NAME" "$DEST" 2>/dev/null || cp "target/release/${BINARY_NAME}.exe" "$DEST"
    chmod +x "$DEST"
    
    cd "$HOME"
    rm -rf "$TEMP_DIR"
    success "Built and installed to $DEST"
}

# Configure hooks
configure_hooks() {
    info "Configuring Cursor hooks..."
    mkdir -p "$HOME/.cursor"
    
    RECURSOR_CMD="$INSTALL_DIR/$BINARY_NAME"
    [ "$OS" = "windows" ] && RECURSOR_CMD="$INSTALL_DIR/${BINARY_NAME}.exe"
    
    if [ -f "$HOOKS_FILE" ]; then
        if grep -q "recursor" "$HOOKS_FILE" 2>/dev/null; then
            warn "Recursor already configured"
            return
        fi
        cp "$HOOKS_FILE" "${HOOKS_FILE}.backup"
        warn "Existing hooks.json backed up. Please merge manually:"
        echo ""
        echo "Add to $HOOKS_FILE:"
    fi
    
    cat > "$HOOKS_FILE" << EOF
{
  "version": 1,
  "hooks": {
    "beforeSubmitPrompt": [
      { "command": "$RECURSOR_CMD save" }
    ],
    "stop": [
      { "command": "$RECURSOR_CMD restore" }
    ]
  }
}
EOF
    success "Created $HOOKS_FILE"
}

# macOS permissions
setup_permissions() {
    [ "$OS" != "macos" ] && return
    
    info "Checking macOS permissions..."
    "$INSTALL_DIR/$BINARY_NAME" permissions 2>/dev/null || true
}

# Success message
print_success() {
    echo ""
    echo "════════════════════════════════════════════════"
    printf "${GREEN}  Recursor installed successfully!${NC}\n"
    echo "════════════════════════════════════════════════"
    echo ""
    echo "  Restart Cursor to activate."
    echo ""
    echo "  Then just use Cursor normally - Recursor"
    echo "  handles everything automatically!"
    echo ""
    [ "$OS" = "macos" ] && echo "  Note: Grant Accessibility permission if prompted."
    echo ""
}

# Main
main() {
    echo ""
    echo "  ╭─────────────────────────────────────╮"
    echo "  │         Installing Recursor         │"
    echo "  │   Bounce Back for Cursor Agents     │"
    echo "  ╰─────────────────────────────────────╯"
    echo ""
    
    detect_os
    detect_arch
    info "Detected: $OS ($ARCH)"
    
    get_download_url
    download_binary
    configure_hooks
    setup_permissions
    print_success
}

main
