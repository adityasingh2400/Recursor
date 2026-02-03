//! macOS window management implementation
//!
//! Uses AppleScript via osascript for all window operations.
//! This approach is more reliable and doesn't require complex CoreFoundation bindings.

use super::{WindowInfo, WindowManager};
use anyhow::{anyhow, Context, Result};
use std::process::Command;
use rusqlite::Connection;

/// macOS window manager implementation
pub struct MacOSWindowManager;

impl MacOSWindowManager {
    pub fn new() -> Self {
        Self
    }

    /// Execute an AppleScript and return the output
    fn run_applescript(&self, script: &str) -> Result<String> {
        let output = Command::new("osascript")
            .arg("-e")
            .arg(script)
            .output()
            .context("Failed to execute osascript")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow!("AppleScript failed: {}", stderr));
        }

        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }

    /// Check if a Chrome window is playing YouTube and pause it
    /// Returns true if a video was paused
    pub fn pause_youtube_if_playing(&self, _window_title: &str) -> bool {
        // Try to pause any YouTube video in Chrome, regardless of window title
        let js_script = r#"
            tell application "Google Chrome"
                repeat with win in (every window)
                    set activeTab to active tab of win
                    set tabURL to URL of activeTab
                    if tabURL contains "youtube.com/watch" then
                        try
                            set jsResult to execute activeTab javascript "
                                (function() {
                                    var video = document.querySelector('video');
                                    if (video && !video.paused) {
                                        video.pause();
                                        return 'paused';
                                    }
                                    return 'not_playing';
                                })();
                            "
                            if jsResult is "paused" then
                                return "paused"
                            end if
                        end try
                    end if
                end repeat
                return "not_paused"
            end tell
        "#;

        if let Ok(result) = self.run_applescript(js_script) {
            return result == "paused";
        }
        
        false
    }

    /// Resume YouTube video in Chrome
    /// Returns true if a video was resumed
    pub fn resume_youtube(&self, _window_title: &str) -> bool {
        // Try to resume any paused YouTube video in Chrome
        let js_script = r#"
            tell application "Google Chrome"
                repeat with win in (every window)
                    set activeTab to active tab of win
                    set tabURL to URL of activeTab
                    if tabURL contains "youtube.com/watch" then
                        try
                            set jsResult to execute activeTab javascript "
                                (function() {
                                    var video = document.querySelector('video');
                                    if (video && video.paused) {
                                        video.play();
                                        return 'resumed';
                                    }
                                    return 'already_playing';
                                })();
                            "
                            if jsResult is "resumed" then
                                return "resumed"
                            end if
                        end try
                    end if
                end repeat
                return "not_resumed"
            end tell
        "#;

        if let Ok(result) = self.run_applescript(&js_script) {
            return result == "resumed";
        }
        
        false
    }

    /// Check if YouTube is currently playing (without pausing it)
    /// Returns true if a video is playing
    pub fn is_youtube_playing(&self) -> bool {
        let js_script = r#"
            tell application "Google Chrome"
                repeat with win in (every window)
                    set activeTab to active tab of win
                    set tabURL to URL of activeTab
                    if tabURL contains "youtube.com/watch" then
                        try
                            set jsResult to execute activeTab javascript "
                                (function() {
                                    var video = document.querySelector('video');
                                    if (video && !video.paused) {
                                        return 'playing';
                                    }
                                    return 'not_playing';
                                })();
                            "
                            if jsResult is "playing" then
                                return "playing"
                            end if
                        end try
                    end if
                end repeat
                return "not_playing"
            end tell
        "#;

        if let Ok(result) = self.run_applescript(js_script) {
            return result == "playing";
        }
        
        false
    }

    /// Update the menu bar status with rich information
    pub fn update_menu_bar_status(&self, status: &str, window_title: Option<&str>) {
        self.update_menu_bar_status_full(status, None, None, None, None);
        // Also update with window title for backwards compatibility
        if window_title.is_some() {
            self.update_menu_bar_status_full(status, None, None, window_title, None);
        }
    }
    
    /// Update the menu bar status with full details
    pub fn update_menu_bar_status_full(
        &self,
        status: &str,
        cursor_state: Option<&str>,
        secondary_app: Option<&str>,
        secondary_title: Option<&str>,
        media_playing: Option<bool>,
    ) {
        let home = std::env::var("HOME").unwrap_or_default();
        let status_file = format!("{}/.cursor/recursor_status.json", home);
        
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        
        // Build JSON with all available fields
        let mut json_parts = vec![
            format!(r#""status":"{}""#, status),
            format!(r#""timestamp":{}"#, timestamp),
        ];
        
        if let Some(cs) = cursor_state {
            json_parts.push(format!(r#""cursor_state":"{}""#, cs.replace('"', "\\\"")));
        }
        
        if let Some(app) = secondary_app {
            json_parts.push(format!(r#""secondary_app":"{}""#, app.replace('"', "\\\"")));
        }
        
        if let Some(title) = secondary_title {
            json_parts.push(format!(r#""secondary_title":"{}""#, title.replace('"', "\\\"")));
            // Also include "window" for backwards compatibility
            json_parts.push(format!(r#""window":"{}""#, title.replace('"', "\\\"")));
        }
        
        if let Some(playing) = media_playing {
            json_parts.push(format!(r#""media_playing":{}"#, playing));
        }
        
        let json = format!("{{{}}}", json_parts.join(","));
        let _ = std::fs::write(&status_file, json);
    }

    /// Get the previously active application (the one before the current frontmost app)
    /// This is useful when Cursor is frontmost and we want to know what app the user was in before
    /// 
    /// Uses lsappinfo's bringForwardOrder which tracks the actual app activation order.
    /// For Chrome specifically, we try to get the most recently focused window.
    pub fn get_previous_application(&self) -> Result<WindowInfo> {
        // Use lsappinfo to get the app activation order
        let output = Command::new("lsappinfo")
            .arg("metainfo")
            .output()
            .context("Failed to run lsappinfo")?;

        if !output.status.success() {
            return Err(anyhow!("lsappinfo failed"));
        }

        let output_str = String::from_utf8_lossy(&output.stdout);
        
        // Parse bringForwardOrder to get app order
        // Format: bringForwardOrder = "Cursor" ASN:... "Terminal" ASN:... "Chrome" ASN:...
        if let Some(order_line) = output_str.lines().find(|l| l.contains("bringForwardOrder")) {
            // Extract app names in order (they're in quotes)
            let mut apps: Vec<String> = Vec::new();
            let mut in_quotes = false;
            let mut current_app = String::new();
            
            for ch in order_line.chars() {
                if ch == '"' {
                    if in_quotes {
                        apps.push(current_app.clone());
                        current_app.clear();
                    }
                    in_quotes = !in_quotes;
                } else if in_quotes {
                    current_app.push(ch);
                }
            }
            
            // Find the first non-Cursor app (that's the previous one)
            for app_name in apps.iter().skip(1) {  // Skip first (current frontmost)
                if !app_name.to_lowercase().contains("cursor") {
                    // Get window info for this app using AppleScript
                    return self.get_app_window_info(app_name);
                }
            }
        }

        // Fallback to Finder
        self.get_app_window_info("Finder")
    }

    /// Get window info for a specific app by name
    fn get_app_window_info(&self, app_name: &str) -> Result<WindowInfo> {
        let escaped_name = app_name.replace('"', "\\\"");
        
        // For Chrome, try multiple strategies to get the right window
        if app_name == "Google Chrome" {
            // Strategy 1: Check if there's a saved "last focused window" from our daemon
            if let Ok(saved) = self.get_chrome_saved_window() {
                return Ok(saved);
            }
            
            // Strategy 2: Use Chrome's scripting - get the first VISIBLE window
            // (visible windows are on the current Space)
            let script = r#"
                tell application "Google Chrome"
                    repeat with win in (every window)
                        if visible of win then
                            set winTitle to title of win
                            set winId to id of win
                            tell application "System Events"
                                set chromePID to unix id of application process "Google Chrome"
                            end tell
                            return "Google Chrome|" & chromePID & "|" & winTitle & "|" & winId
                        end if
                    end repeat
                    -- Fallback to front window
                    set activeWin to front window
                    set winTitle to title of activeWin
                    set winId to id of activeWin
                    tell application "System Events"
                        set chromePID to unix id of application process "Google Chrome"
                    end tell
                    return "Google Chrome|" & chromePID & "|" & winTitle & "|" & winId
                end tell
            "#;
            
            if let Ok(result) = self.run_applescript(script) {
                if let Ok(info) = self.parse_window_info(&result) {
                    return Ok(info);
                }
            }
        }
        
        // For Safari, use Safari's own scripting
        if app_name == "Safari" {
            let script = r#"
                tell application "Safari"
                    set activeWin to front window
                    set winTitle to name of activeWin
                end tell
                tell application "System Events"
                    set safariPID to unix id of application process "Safari"
                end tell
                return "Safari|" & safariPID & "|" & winTitle & "|1"
            "#;
            
            if let Ok(result) = self.run_applescript(script) {
                if let Ok(info) = self.parse_window_info(&result) {
                    return Ok(info);
                }
            }
        }
        
        // Default: use System Events (works for most apps)
        let script = format!(
            r#"
            tell application "System Events"
                try
                    set targetProc to application process "{}"
                    set appPID to unix id of targetProc
                    
                    set windowTitle to ""
                    set windowIndex to 1
                    try
                        set frontWin to front window of targetProc
                        set windowTitle to name of frontWin
                        set windowIndex to index of frontWin
                    end try
                    
                    return "{}" & "|" & appPID & "|" & windowTitle & "|" & windowIndex
                on error
                    return "error"
                end try
            end tell
            "#,
            escaped_name, escaped_name
        );

        let result = self.run_applescript(&script)?;
        if result == "error" {
            return Err(anyhow!("Could not get info for app: {}", app_name));
        }
        self.parse_window_info(&result)
    }
    
    /// Try to get saved Chrome window from our tracking file
    fn get_chrome_saved_window(&self) -> Result<WindowInfo> {
        let home = std::env::var("HOME").context("No HOME")?;
        let path = format!("{}/.cursor/recursor_chrome_window.txt", home);
        
        if let Ok(content) = std::fs::read_to_string(&path) {
            // Format: title|timestamp
            let parts: Vec<&str> = content.trim().split('|').collect();
            if parts.len() >= 1 {
                let title = parts[0].to_string();
                // Check if it's recent (within last 30 seconds)
                if parts.len() >= 2 {
                    if let Ok(ts) = parts[1].parse::<u64>() {
                        let now = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .map(|d| d.as_secs())
                            .unwrap_or(0);
                        if now - ts > 30 {
                            return Err(anyhow!("Saved window too old"));
                        }
                    }
                }
                
                // Get Chrome's PID
                let pid_script = r#"
                    tell application "System Events"
                        return unix id of application process "Google Chrome"
                    end tell
                "#;
                let pid: u32 = self.run_applescript(pid_script)?.parse().unwrap_or(0);
                
                return Ok(WindowInfo {
                    pid,
                    window_id: "saved".to_string(),
                    app_name: "Google Chrome".to_string(),
                    title,
                });
            }
        }
        
        Err(anyhow!("No saved Chrome window"))
    }

    /// Parse window info from AppleScript output format: "appName|pid|title|windowIndex"
    fn parse_window_info(&self, result: &str) -> Result<WindowInfo> {
        let parts: Vec<&str> = result.splitn(4, '|').collect();

        if parts.len() < 2 {
            return Err(anyhow!("Unexpected AppleScript output: {}", result));
        }

        let app_name = parts[0].to_string();
        let pid: u32 = parts[1]
            .parse()
            .context("Failed to parse PID from AppleScript")?;
        let title = parts.get(2).unwrap_or(&"").to_string();
        let window_index = parts.get(3).unwrap_or(&"1").to_string();

        // Use PID and window index as window ID
        let window_id = format!("{}:{}", pid, window_index);

        Ok(WindowInfo {
            pid,
            window_id,
            app_name,
            title,
        })
    }
}

impl WindowManager for MacOSWindowManager {
    fn get_active_window(&self) -> Result<WindowInfo> {
        // Get frontmost application info using AppleScript
        let script = r#"
            tell application "System Events"
                set frontApp to first application process whose frontmost is true
                set appName to name of frontApp
                set appPID to unix id of frontApp
                
                -- Try to get window title and index
                set windowTitle to ""
                set windowIndex to 1
                try
                    set frontWin to front window of frontApp
                    set windowTitle to name of frontWin
                    -- Get window index for later focusing
                    set windowIndex to index of frontWin
                end try
                
                return appName & "|" & appPID & "|" & windowTitle & "|" & windowIndex
            end tell
        "#;

        let result = self.run_applescript(script)?;
        self.parse_window_info(&result)
    }
    
    fn get_previous_window(&self) -> Result<WindowInfo> {
        self.get_previous_application()
    }

    fn focus_window(&self, window: &WindowInfo) -> Result<()> {
        // Escape the app name for AppleScript
        let escaped_name = window.app_name.replace('"', "\\\"");
        let escaped_title = window.title.replace('"', "\\\"");
        
        // For Chrome, use Chrome's own scripting to focus the correct window
        if window.app_name == "Google Chrome" && !window.title.is_empty() {
            let script = format!(
                r#"
                tell application "Google Chrome"
                    activate
                    set targetIndex to 1
                    repeat with win in (every window)
                        if title of win contains "{}" then
                            set index of win to 1
                            set active tab index of win to (active tab index of win)
                            exit repeat
                        end if
                        set targetIndex to targetIndex + 1
                    end repeat
                end tell
                "#,
                escaped_title
            );

            if self.run_applescript(&script).is_ok() {
                return Ok(());
            }
        }
        
        // For Safari, use Safari's own scripting
        if window.app_name == "Safari" && !window.title.is_empty() {
            let script = format!(
                r#"
                tell application "Safari"
                    activate
                    repeat with win in (every window)
                        if name of win contains "{}" then
                            set index of win to 1
                            exit repeat
                        end if
                    end repeat
                end tell
                "#,
                escaped_title
            );

            if self.run_applescript(&script).is_ok() {
                return Ok(());
            }
        }
        
        // Try to focus the specific window by title first using System Events
        if !window.title.is_empty() {
            let script = format!(
                r#"
                tell application "System Events"
                    tell process "{}"
                        set frontmost to true
                        try
                            -- Try to find and focus the specific window by title
                            set targetWindow to first window whose name contains "{}"
                            perform action "AXRaise" of targetWindow
                        end try
                    end tell
                end tell
                tell application "{}"
                    activate
                end tell
            "#,
                escaped_name, escaped_title, escaped_name
            );

            if self.run_applescript(&script).is_ok() {
                return Ok(());
            }
        }
        
        // Fallback: just activate the app
        let script = format!(
            r#"
            tell application "{}"
                activate
            end tell
        "#,
            escaped_name
        );

        self.run_applescript(&script)?;
        Ok(())
    }

    fn focus_cursor(&self) -> Result<()> {
        let script = r#"
            tell application "Cursor"
                activate
            end tell
        "#;

        self.run_applescript(script)?;
        Ok(())
    }
    
    /// Focus a specific Cursor window by its title
    fn focus_cursor_window(&self, window: &WindowInfo) -> Result<()> {
        let escaped_title = window.title.replace('"', "\\\"");
        
        // Try to focus the specific Cursor window by title
        if !window.title.is_empty() {
            let script = format!(
                r#"
                tell application "System Events"
                    tell process "Cursor"
                        set frontmost to true
                        try
                            -- Try to find and focus the specific window by title
                            set targetWindow to first window whose name contains "{}"
                            perform action "AXRaise" of targetWindow
                        end try
                    end tell
                end tell
                tell application "Cursor"
                    activate
                end tell
            "#,
                escaped_title
            );

            if self.run_applescript(&script).is_ok() {
                return Ok(());
            }
        }
        
        // Fallback to generic Cursor focus
        self.focus_cursor()
    }
    
    fn pause_youtube_if_playing(&self, window_title: &str) -> bool {
        MacOSWindowManager::pause_youtube_if_playing(self, window_title)
    }
    
    fn resume_youtube(&self, window_title: &str) -> bool {
        MacOSWindowManager::resume_youtube(self, window_title)
    }
    
    fn update_menu_bar_status(&self, status: &str, window_title: Option<&str>) {
        MacOSWindowManager::update_menu_bar_status(self, status, window_title)
    }
    
    fn is_youtube_playing(&self) -> bool {
        MacOSWindowManager::is_youtube_playing(self)
    }
    
    fn update_menu_bar_status_full(
        &self,
        status: &str,
        cursor_state: Option<&str>,
        secondary_app: Option<&str>,
        secondary_title: Option<&str>,
        media_playing: Option<bool>,
    ) {
        MacOSWindowManager::update_menu_bar_status_full(
            self,
            status,
            cursor_state,
            secondary_app,
            secondary_title,
            media_playing,
        )
    }
}

impl Default for MacOSWindowManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Read Cursor's command allowlist from its SQLite database.
/// Returns the list of commands that are auto-approved (won't need user confirmation).
pub fn read_cursor_allowlist() -> Result<Vec<String>> {
    let home = std::env::var("HOME").context("No HOME environment variable")?;
    let db_path = format!(
        "{}/Library/Application Support/Cursor/User/globalStorage/state.vscdb",
        home
    );
    
    // Open the database in read-only mode
    let conn = Connection::open_with_flags(
        &db_path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
    ).context("Failed to open Cursor state database")?;
    
    // Query the persistent storage key
    let key = "src.vs.platform.reactivestorage.browser.reactiveStorageServiceImpl.persistentStorage.applicationUser";
    let value: String = conn
        .query_row(
            "SELECT value FROM ItemTable WHERE key = ?",
            [key],
            |row| row.get(0),
        )
        .context("Failed to read persistent storage from Cursor database")?;
    
    // Parse the JSON and extract yoloCommandAllowlist
    let parsed: serde_json::Value = serde_json::from_str(&value)
        .context("Failed to parse Cursor persistent storage JSON")?;
    
    let allowlist = parsed
        .get("composerState")
        .and_then(|cs| cs.get("yoloCommandAllowlist"))
        .and_then(|al| al.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default();
    
    Ok(allowlist)
}

/// Check if a command matches any entry in the allowlist.
/// Cursor uses prefix matching - if the command starts with an allowlist entry, it's allowed.
pub fn is_command_allowed(command: &str, allowlist: &[String]) -> bool {
    let cmd_trimmed = command.trim();
    
    for allowed in allowlist {
        // Exact match
        if cmd_trimmed == allowed {
            return true;
        }
        // Prefix match: command starts with allowed entry followed by space or end
        if cmd_trimmed.starts_with(allowed) {
            let rest = &cmd_trimmed[allowed.len()..];
            if rest.is_empty() || rest.starts_with(' ') || rest.starts_with('\t') {
                return true;
            }
        }
    }
    
    false
}
