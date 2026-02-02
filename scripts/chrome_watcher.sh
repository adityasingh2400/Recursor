#!/bin/bash
# Chrome Window Watcher for Recursor
# Tracks the currently focused Chrome window and saves it for Recursor to use

SAVE_FILE="$HOME/.cursor/recursor_chrome_window.txt"

while true; do
    # Check if Chrome is the frontmost app
    FRONT_APP=$(osascript -e 'tell application "System Events" to return name of first application process whose frontmost is true' 2>/dev/null)
    
    if [ "$FRONT_APP" = "Google Chrome" ]; then
        # Get the front window title
        WINDOW_TITLE=$(osascript -e 'tell application "Google Chrome" to return title of front window' 2>/dev/null)
        TIMESTAMP=$(date +%s)
        
        if [ -n "$WINDOW_TITLE" ]; then
            echo "${WINDOW_TITLE}|${TIMESTAMP}" > "$SAVE_FILE"
        fi
    fi
    
    # Check every 500ms
    sleep 0.5
done
