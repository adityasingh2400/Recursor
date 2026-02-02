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
                
                -- Try to get window title
                set windowTitle to ""
                try
                    set windowTitle to name of front window of frontApp
                end try
                
                return appName & "|" & appPID & "|" & windowTitle
            end tell
        "#;

        let result = self.run_applescript(script)?;
        let parts: Vec<&str> = result.splitn(3, '|').collect();

        if parts.len() < 2 {
            return Err(anyhow!("Unexpected AppleScript output: {}", result));
        }

        let app_name = parts[0].to_string();
        let pid: u32 = parts[1]
            .parse()
            .context("Failed to parse PID from AppleScript")?;
        let title = parts.get(2).unwrap_or(&"").to_string();

        // Use PID as window ID since AppleScript doesn't give us a reliable window ID
        let window_id = format!("pid:{}", pid);

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
}

impl Default for MacOSWindowManager {
    fn default() -> Self {
        Self::new()
    }
}
