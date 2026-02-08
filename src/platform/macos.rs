//! macOS window management implementation
//!
//! Uses AppleScript via osascript for all window operations.
//! This approach is more reliable and doesn't require complex CoreFoundation bindings.

use super::{WindowInfo, WindowManager};
use anyhow::{anyhow, Context, Result};
use rusqlite::Connection;
use serde_json::{Map, Value};
use std::path::PathBuf;
use std::process::Command;

/// Helper struct for ASN query results
struct AsnInfo {
    pid: u32,
    is_cursor_child: bool,
}

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

        if let Ok(result) = self.run_applescript(js_script) {
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
    #[allow(dead_code)]
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
        let Some(cursor_dir) = Self::cursor_dir() else {
            return;
        };
        let status_file = cursor_dir.join("recursor_status.json");
        let _ = std::fs::create_dir_all(&cursor_dir);

        let payload = Self::build_status_payload(
            status,
            cursor_state,
            secondary_app,
            secondary_title,
            media_playing,
        );
        if let Ok(json) = serde_json::to_string(&payload) {
            let _ = std::fs::write(&status_file, json);
        }
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

        // Parse bringForwardOrder to get (app_name, ASN) pairs
        if let Some(order_line) = output_str.lines().find(|l| l.contains("bringForwardOrder")) {
            let pairs = Self::parse_bring_forward_order(order_line);

            // Skip first entry (frontmost = Cursor), find the real previous app
            for (app_name, asn) in pairs.iter().skip(1) {
                if app_name.to_lowercase().contains("cursor") {
                    continue;
                }

                // For Chrome, we must verify this isn't Cursor's headless Chrome
                if app_name == "Google Chrome" {
                    if let Some(info) = Self::query_asn_info(asn) {
                        if info.is_cursor_child {
                            continue; // Skip Cursor's Chrome extension host
                        }
                        if info.pid > 0 {
                            // Get window title using PID-based System Events
                            let title = Self::get_window_title_by_pid(info.pid);
                            return Ok(WindowInfo {
                                pid: info.pid,
                                window_id: format!("{}:1", info.pid),
                                app_name: "Google Chrome".to_string(),
                                title,
                            });
                        }
                    }
                }

                // Non-Chrome app — use standard approach
                return self.get_app_window_info(app_name);
            }
        }

        // Fallback to Finder
        self.get_app_window_info("Finder")
    }

    /// Parse bringForwardOrder line into (app_name, ASN) pairs
    fn parse_bring_forward_order(line: &str) -> Vec<(String, String)> {
        let mut pairs = Vec::new();
        let mut i = 0;
        let chars: Vec<char> = line.chars().collect();
        while i < chars.len() {
            if chars[i] == '"' {
                i += 1;
                let mut name = String::new();
                while i < chars.len() && chars[i] != '"' {
                    name.push(chars[i]);
                    i += 1;
                }
                i += 1; // skip closing quote
                        // Now find the ASN (e.g., ASN:0x0-0x123123:)
                while i < chars.len() && chars[i] == ' ' {
                    i += 1;
                }
                let mut asn = String::new();
                while i < chars.len() && chars[i] != ' ' && chars[i] != '"' {
                    asn.push(chars[i]);
                    i += 1;
                }
                if !name.is_empty() {
                    pairs.push((name, asn));
                }
            } else {
                i += 1;
            }
        }
        pairs
    }

    /// Query lsappinfo for detailed info about an ASN
    fn query_asn_info(asn: &str) -> Option<AsnInfo> {
        let output = Command::new("lsappinfo")
            .args(["info", asn])
            .output()
            .ok()?;
        let text = String::from_utf8_lossy(&output.stdout);

        let mut pid: u32 = 0;
        let mut parent_asn = String::new();

        for line in text.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with("pid") && trimmed.contains('=') {
                if let Some(val) = trimmed.split('=').nth(1) {
                    pid = val.trim().parse().unwrap_or(0);
                }
            }
            if trimmed.starts_with("\"parentASN\"") && trimmed.contains('=') {
                if let Some(val) = trimmed.split('=').nth(1) {
                    parent_asn = val.trim().trim_matches('"').to_string();
                }
            }
        }

        // Check if parent is Cursor
        let is_cursor_child = if !parent_asn.is_empty() && parent_asn != "ASN:0x0-0x0:" {
            if let Ok(parent_out) = Command::new("lsappinfo")
                .args(["info", &parent_asn])
                .output()
            {
                let parent_text = String::from_utf8_lossy(&parent_out.stdout);
                parent_text.contains("Cursor")
            } else {
                false
            }
        } else {
            false
        };

        Some(AsnInfo {
            pid,
            is_cursor_child,
        })
    }

    /// Get the title of the frontmost window for a process by PID
    fn get_window_title_by_pid(pid: u32) -> String {
        let script = format!(
            r#"
            tell application "System Events"
                set targetProc to first application process whose unix id is {}
                set allWins to every window of targetProc
                set winCount to count of allWins
                repeat with i from 1 to winCount
                    try
                        set winName to name of item i of allWins
                        if winName is not "" then
                            return winName
                        end if
                    end try
                end repeat
                return ""
            end tell
            "#,
            pid
        );
        Command::new("osascript")
            .args(["-e", &script])
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
            .unwrap_or_default()
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
            if !parts.is_empty() {
                let title = parts[0].to_string();
                // Check if it's recent (within last 30 seconds)
                if parts.len() >= 2 {
                    if let Ok(ts) = parts[1].parse::<u64>() {
                        let now = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .map(|d| d.as_secs())
                            .unwrap_or(0);
                        if now.saturating_sub(ts) > 30 {
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
        let mut first_split = result.splitn(3, '|');
        let app_name = first_split
            .next()
            .ok_or_else(|| anyhow!("Unexpected AppleScript output: {}", result))?;
        let pid_part = first_split
            .next()
            .ok_or_else(|| anyhow!("Unexpected AppleScript output: {}", result))?;
        let title_and_index = first_split.next().unwrap_or("");

        let (title, window_index) =
            if let Some((title, window_index)) = title_and_index.rsplit_once('|') {
                (title, window_index)
            } else {
                (title_and_index, "1")
            };

        if app_name.is_empty() {
            return Err(anyhow!("Unexpected AppleScript output: {}", result));
        }

        let pid: u32 = pid_part
            .parse()
            .context("Failed to parse PID from AppleScript")?;

        // Use PID and window index as window ID
        let window_id = format!("{}:{}", pid, window_index);

        Ok(WindowInfo {
            pid,
            window_id,
            app_name: app_name.to_string(),
            title: title.to_string(),
        })
    }

    fn cursor_dir() -> Option<PathBuf> {
        dirs::home_dir().map(|h| h.join(".cursor"))
    }

    fn build_status_payload(
        status: &str,
        cursor_state: Option<&str>,
        secondary_app: Option<&str>,
        secondary_title: Option<&str>,
        media_playing: Option<bool>,
    ) -> Value {
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        let mut payload = Map::new();
        payload.insert("status".to_string(), Value::String(status.to_string()));
        payload.insert("timestamp".to_string(), Value::Number(timestamp.into()));

        if let Some(cs) = cursor_state {
            payload.insert("cursor_state".to_string(), Value::String(cs.to_string()));
        }
        if let Some(app) = secondary_app {
            payload.insert("secondary_app".to_string(), Value::String(app.to_string()));
        }
        if let Some(title) = secondary_title {
            payload.insert(
                "secondary_title".to_string(),
                Value::String(title.to_string()),
            );
            // Backwards compatibility with older menu bar versions.
            payload.insert("window".to_string(), Value::String(title.to_string()));
        }
        if let Some(playing) = media_playing {
            payload.insert("media_playing".to_string(), Value::Bool(playing));
        }

        Value::Object(payload)
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

        // For Chrome, use PID-based System Events targeting to avoid hitting
        // Cursor's headless Chrome extension process
        if window.app_name == "Google Chrome" && window.pid > 0 {
            let escaped_title_for_chrome = window.title.replace('"', "\\\"");
            let script = format!(
                r#"
                tell application "System Events"
                    set chromeProc to first application process whose unix id is {}
                    set frontmost of chromeProc to true
                    delay 0.1
                    set allWins to every window of chromeProc
                    set winCount to count of allWins
                    if "{}" is not "" then
                        repeat with i from 1 to winCount
                            try
                                set winName to name of item i of allWins
                                if winName contains "{}" then
                                    perform action "AXRaise" of (item i of allWins)
                                    return "ok"
                                end if
                            end try
                        end repeat
                    end if
                    repeat with i from 1 to winCount
                        try
                            perform action "AXRaise" of (item i of allWins)
                            return "ok"
                        end try
                    end repeat
                end tell
                "#,
                window.pid, escaped_title_for_chrome, escaped_title_for_chrome
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
    ///
    /// Uses a multi-strategy approach to reliably find the correct window
    /// even when multiple Cursor windows are open:
    ///
    /// 1. Match by project name (stable - doesn't change when agent opens files)
    /// 2. Match by full title (exact match fallback)
    /// 3. Generic Cursor activation (last resort)
    fn focus_cursor_window(&self, window: &WindowInfo) -> Result<()> {
        // Strategy 1: Match by project/workspace name extracted from the title.
        // Cursor titles follow "filename - ProjectName - Cursor". The filename
        // changes as the agent opens different files, but the project name is
        // constant. This reliably identifies the right window.
        if let Some(project_name) = window.cursor_project_name() {
            let escaped_project = project_name.replace('"', "\\\"");
            let script = format!(
                r#"
                tell application "System Events"
                    tell process "Cursor"
                        set frontmost to true
                        try
                            repeat with win in (every window)
                                if name of win contains "{}" then
                                    perform action "AXRaise" of win
                                    tell application "Cursor" to activate
                                    return "found"
                                end if
                            end repeat
                        end try
                    end tell
                end tell
                return "not_found"
                "#,
                escaped_project
            );

            if let Ok(result) = self.run_applescript(&script) {
                if result == "found" {
                    return Ok(());
                }
            }
        }

        // Strategy 2: Try full title match (works if title hasn't changed)
        if !window.title.is_empty() {
            let escaped_title = window.title.replace('"', "\\\"");
            let script = format!(
                r#"
                tell application "System Events"
                    tell process "Cursor"
                        set frontmost to true
                        try
                            repeat with win in (every window)
                                if name of win contains "{}" then
                                    perform action "AXRaise" of win
                                    tell application "Cursor" to activate
                                    return "found"
                                end if
                            end repeat
                        end try
                    end tell
                end tell
                return "not_found"
                "#,
                escaped_title
            );

            if let Ok(result) = self.run_applescript(&script) {
                if result == "found" {
                    return Ok(());
                }
            }
        }

        // Strategy 3: Generic Cursor focus (last resort)
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
#[allow(dead_code)]
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
    )
    .context("Failed to open Cursor state database")?;

    // Query the persistent storage key
    let key = "src.vs.platform.reactivestorage.browser.reactiveStorageServiceImpl.persistentStorage.applicationUser";
    let value: String = conn
        .query_row("SELECT value FROM ItemTable WHERE key = ?", [key], |row| {
            row.get(0)
        })
        .context("Failed to read persistent storage from Cursor database")?;

    // Parse the JSON and extract yoloCommandAllowlist
    let parsed: serde_json::Value =
        serde_json::from_str(&value).context("Failed to parse Cursor persistent storage JSON")?;

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
#[allow(dead_code)]
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_window_info_supports_pipe_in_title() {
        let wm = MacOSWindowManager::new();
        let info = wm
            .parse_window_info("Google Chrome|123|YouTube | Live | Music|7")
            .expect("parse should succeed");

        assert_eq!(info.app_name, "Google Chrome");
        assert_eq!(info.pid, 123);
        assert_eq!(info.title, "YouTube | Live | Music");
        assert_eq!(info.window_id, "123:7");
    }

    #[test]
    fn build_status_payload_handles_special_characters() {
        let payload = MacOSWindowManager::build_status_payload(
            "working",
            Some("Agent says \"ok\""),
            Some("Google Chrome"),
            Some("Line 1\nLine 2 \\ path"),
            Some(false),
        );

        assert_eq!(payload["status"], "working");
        assert_eq!(payload["cursor_state"], "Agent says \"ok\"");
        assert_eq!(payload["secondary_app"], "Google Chrome");
        assert_eq!(payload["secondary_title"], "Line 1\nLine 2 \\ path");
        assert_eq!(payload["window"], "Line 1\nLine 2 \\ path");
        assert_eq!(payload["media_playing"], false);
        assert!(payload["timestamp"].is_number());
    }

    #[test]
    fn parse_bring_forward_order_extracts_name_and_asn() {
        let line =
            "bringForwardOrder=\"Cursor\" ASN:0x0-0x11111: \"Google Chrome\" ASN:0x0-0x22222:";
        let pairs = MacOSWindowManager::parse_bring_forward_order(line);
        assert_eq!(pairs.len(), 2);
        assert_eq!(
            pairs[0],
            ("Cursor".to_string(), "ASN:0x0-0x11111:".to_string())
        );
        assert_eq!(
            pairs[1],
            ("Google Chrome".to_string(), "ASN:0x0-0x22222:".to_string())
        );
    }

    #[test]
    fn command_allowlist_prefix_is_boundary_aware() {
        let allowlist = vec!["git commit".to_string()];
        assert!(is_command_allowed("git commit -m test", &allowlist));
        assert!(!is_command_allowed("git committed", &allowlist));
    }
}
