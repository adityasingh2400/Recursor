//! macOS window management implementation
//!
//! Uses AppleScript via osascript for all window operations.
//! This approach is more reliable and doesn't require complex CoreFoundation bindings.

use super::{WindowInfo, WindowManager};
use anyhow::{anyhow, Context, Result};
use std::process::Command;

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

    fn focus_window(&self, window: &WindowInfo) -> Result<()> {
        // Escape the app name for AppleScript
        let escaped_name = window.app_name.replace('"', "\\\"");
        let escaped_title = window.title.replace('"', "\\\"");
        
        // Try to focus the specific window by title first
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
}

impl Default for MacOSWindowManager {
    fn default() -> Self {
        Self::new()
    }
}
