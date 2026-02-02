#!/bin/bash
# Recursor One-Click Installer for macOS
# Double-click this file to install Recursor

# Show a notification that installation is starting
osascript -e 'display notification "Installing Recursor..." with title "Recursor"'

# Run the installer
curl -fsSL https://raw.githubusercontent.com/adityasingh2400/Recursor/main/install.sh | sh

# Show completion notification
osascript -e 'display notification "Installation complete! Restart Cursor to activate." with title "Recursor" sound name "Glass"'

# Open a dialog with instructions
osascript -e 'display dialog "Recursor installed successfully!\n\nRestart Cursor to activate.\n\nIf prompted, grant Accessibility permission in System Preferences." with title "Recursor" buttons {"OK"} default button "OK" with icon note'
