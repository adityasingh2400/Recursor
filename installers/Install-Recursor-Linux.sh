#!/bin/bash
# Recursor One-Click Installer for Linux
# Double-click this file to install Recursor
# (Make sure to right-click > Properties > Permissions > Allow executing as program)

# Check if running in a terminal, if not open one
if [ ! -t 0 ]; then
    if command -v gnome-terminal &> /dev/null; then
        gnome-terminal -- bash -c "$0; exec bash"
        exit 0
    elif command -v konsole &> /dev/null; then
        konsole -e bash -c "$0; exec bash"
        exit 0
    elif command -v xterm &> /dev/null; then
        xterm -e bash -c "$0; exec bash"
        exit 0
    fi
fi

echo ""
echo "  ========================================"
echo "       Installing Recursor..."
echo "  ========================================"
echo ""

# Run the installer
curl -fsSL https://raw.githubusercontent.com/adityasingh2400/Recursor/main/install.sh | sh

echo ""
echo "  ========================================"
echo "    Installation complete!"
echo "    Restart Cursor to activate."
echo "  ========================================"
echo ""
read -p "Press Enter to close..."
