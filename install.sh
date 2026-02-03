#!/bin/sh
# Recursor Installer
# One-liner: curl -fsSL https://raw.githubusercontent.com/adityasingh2400/Recursor/main/install.sh | sh

set -e

REPO="adityasingh2400/Recursor"
INSTALL_DIR="$HOME/.cursor/bin"
HOOKS_FILE="$HOME/.cursor/hooks.json"
BINARY_NAME="recursor"
MENUBAR_NAME="recursor-menubar"
LAUNCHAGENT_DIR="$HOME/Library/LaunchAgents"
LAUNCHAGENT_PLIST="com.recursor.menubar.plist"

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

# Install Linux dependencies
install_linux_deps() {
    [ "$OS" != "linux" ] && return
    
    if ! command -v xdotool >/dev/null 2>&1; then
        info "Installing xdotool..."
        if command -v apt >/dev/null 2>&1; then
            sudo apt update && sudo apt install -y xdotool
        elif command -v dnf >/dev/null 2>&1; then
            sudo dnf install -y xdotool
        elif command -v pacman >/dev/null 2>&1; then
            sudo pacman -S --noconfirm xdotool
        elif command -v zypper >/dev/null 2>&1; then
            sudo zypper install -y xdotool
        else
            warn "Please install xdotool manually"
        fi
    fi
}

# Get download URL
get_download_url() {
    case "$OS" in
        macos) BINARY_FILE="recursor-macos-universal" ;;
        linux) BINARY_FILE="recursor-linux-${ARCH}" ;;
        windows) BINARY_FILE="recursor-windows-${ARCH}.exe" ;;
    esac
    DOWNLOAD_URL="https://github.com/${REPO}/releases/latest/download/${BINARY_FILE}"
    MENUBAR_URL="https://github.com/${REPO}/releases/latest/download/recursor-menubar"
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
    
    # Build menu bar app on macOS
    if [ "$OS" = "macos" ]; then
        build_menubar_app "$TEMP_DIR/recursor"
    fi
    
    cd "$HOME"
    rm -rf "$TEMP_DIR"
    success "Built and installed to $DEST"
}

# Build menu bar app (macOS only)
build_menubar_app() {
    SRC_DIR="$1"
    info "Building menu bar app..."
    
    MENUBAR_SRC="$SRC_DIR/menubar/RecursorMenuBar.swift"
    MENUBAR_DEST="$INSTALL_DIR/$MENUBAR_NAME"
    
    if [ -f "$MENUBAR_SRC" ]; then
        swiftc -O -o "$MENUBAR_DEST" "$MENUBAR_SRC" 2>/dev/null || {
            warn "Could not compile menu bar app (Swift not available)"
            return
        }
        chmod +x "$MENUBAR_DEST"
        success "Menu bar app built"
    fi
}

# Install menu bar app (macOS only)
install_menubar() {
    [ "$OS" != "macos" ] && return
    
    info "Setting up menu bar status indicator..."
    
    # Try to download pre-built menu bar app
    MENUBAR_DEST="$INSTALL_DIR/$MENUBAR_NAME"
    if [ ! -f "$MENUBAR_DEST" ]; then
        if command -v curl >/dev/null 2>&1; then
            curl -fsSL "$MENUBAR_URL" -o "$MENUBAR_DEST" 2>/dev/null || true
        fi
    fi
    
    # If still no binary, try to compile from source
    if [ ! -f "$MENUBAR_DEST" ]; then
        MENUBAR_SWIFT_URL="https://raw.githubusercontent.com/${REPO}/main/menubar/RecursorMenuBar.swift"
        TEMP_SWIFT=$(mktemp)
        if curl -fsSL "$MENUBAR_SWIFT_URL" -o "$TEMP_SWIFT" 2>/dev/null; then
            swiftc -O -o "$MENUBAR_DEST" "$TEMP_SWIFT" 2>/dev/null || {
                warn "Could not compile menu bar app"
                rm -f "$TEMP_SWIFT"
                return
            }
            rm -f "$TEMP_SWIFT"
        fi
    fi
    
    if [ -f "$MENUBAR_DEST" ]; then
        chmod +x "$MENUBAR_DEST"
        
        # Create LaunchAgent
        mkdir -p "$LAUNCHAGENT_DIR"
        cat > "$LAUNCHAGENT_DIR/$LAUNCHAGENT_PLIST" << EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>com.recursor.menubar</string>
    <key>ProgramArguments</key>
    <array>
        <string>$MENUBAR_DEST</string>
    </array>
    <key>RunAtLoad</key>
    <true/>
    <key>KeepAlive</key>
    <true/>
</dict>
</plist>
EOF
        
        # Load the launch agent
        launchctl unload "$LAUNCHAGENT_DIR/$LAUNCHAGENT_PLIST" 2>/dev/null || true
        launchctl load "$LAUNCHAGENT_DIR/$LAUNCHAGENT_PLIST" 2>/dev/null || true
        
        success "Menu bar app installed and started"
    else
        warn "Menu bar app not available (optional feature)"
    fi
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
    "beforeShellExecution": [
      { "command": "$RECURSOR_CMD before-shell" }
    ],
    "afterShellExecution": [
      { "command": "$RECURSOR_CMD after-shell" }
    ],
    "stop": [
      { "command": "$RECURSOR_CMD restore" }
    ]
  }
}
EOF
    success "Created $HOOKS_FILE"
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
    if [ "$OS" = "macos" ]; then
        echo "  On first run, macOS will ask for Accessibility"
        echo "  permission - just click Allow when prompted."
        echo ""
    fi
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
    
    install_linux_deps
    get_download_url
    download_binary
    configure_hooks
    install_menubar
    print_success
}

main
