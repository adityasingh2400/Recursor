//! Platform-specific window management
//!
//! This module provides a cross-platform abstraction for window management operations
//! needed by Reflex: getting the active window, focusing windows, and detecting Cursor.

use anyhow::Result;
use serde::{Deserialize, Serialize};

/// Information about a window
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WindowInfo {
    /// Process ID of the window owner
    pub pid: u32,
    /// Platform-specific window identifier
    pub window_id: String,
    /// Name of the application (e.g., "Google Chrome")
    pub app_name: String,
    /// Window title (e.g., "YouTube - Google Chrome")
    #[serde(default)]
    pub title: String,
}

impl WindowInfo {
    /// Check if this window belongs to Cursor
    pub fn is_cursor(&self) -> bool {
        let app_lower = self.app_name.to_lowercase();
        app_lower.contains("cursor")
    }
}

/// Trait for platform-specific window management operations
pub trait WindowManager {
    /// Get information about the currently active/focused window
    fn get_active_window(&self) -> Result<WindowInfo>;

    /// Get the previously active window (before the current one)
    /// This is useful when Cursor is frontmost and we want to know what app the user was in before
    fn get_previous_window(&self) -> Result<WindowInfo> {
        // Default implementation: just return active window (platforms should override)
        self.get_active_window()
    }

    /// Focus/activate a specific window
    fn focus_window(&self, window: &WindowInfo) -> Result<()>;

    /// Focus the Cursor application window (any window)
    fn focus_cursor(&self) -> Result<()>;

    /// Focus a specific Cursor window (by title/id)
    fn focus_cursor_window(&self, window: &WindowInfo) -> Result<()> {
        // Default implementation: try focus_window, fallback to focus_cursor
        self.focus_window(window).or_else(|_| self.focus_cursor())
    }

    /// Check if a window belongs to Cursor
    fn is_cursor_window(&self, window: &WindowInfo) -> bool {
        window.is_cursor()
    }
    
    /// Pause YouTube if playing in the given window (returns true if paused)
    fn pause_youtube_if_playing(&self, _window_title: &str) -> bool {
        false // Default: no-op
    }
    
    /// Resume YouTube in the given window (returns true if resumed)
    fn resume_youtube(&self, _window_title: &str) -> bool {
        false // Default: no-op
    }
    
    /// Update menu bar status indicator
    fn update_menu_bar_status(&self, _status: &str, _window_title: Option<&str>) {
        // Default: no-op
    }
}

// Platform-specific implementations
#[cfg(target_os = "macos")]
pub mod macos;
#[cfg(target_os = "macos")]
pub use macos::MacOSWindowManager as PlatformWindowManager;

#[cfg(target_os = "windows")]
mod windows;
#[cfg(target_os = "windows")]
pub use windows::WindowsWindowManager as PlatformWindowManager;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
pub use linux::LinuxWindowManager as PlatformWindowManager;

/// Create a new platform-specific window manager
pub fn create_window_manager() -> PlatformWindowManager {
    PlatformWindowManager::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_cursor() {
        let cursor_window = WindowInfo {
            pid: 1234,
            window_id: "test".to_string(),
            app_name: "Cursor".to_string(),
            title: "main.rs - Cursor".to_string(),
        };
        assert!(cursor_window.is_cursor());

        let chrome_window = WindowInfo {
            pid: 5678,
            window_id: "test2".to_string(),
            app_name: "Google Chrome".to_string(),
            title: "YouTube".to_string(),
        };
        assert!(!chrome_window.is_cursor());
    }
}
