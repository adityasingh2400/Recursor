//! State file management for Recursor
//!
//! Handles saving and loading the window state between save and restore operations.
//! Implements "intelligent refocus" logic to avoid forcing users back to windows
//! they've manually switched away from.

use crate::platform::WindowInfo;
use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

/// State file format
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecursorState {
    /// The window that was active when the user submitted a prompt
    pub saved_window: WindowInfo,
    /// When the window was saved
    pub saved_at: DateTime<Utc>,
    /// Last time we checked if the user switched apps
    #[serde(default)]
    pub last_active_check: Option<DateTime<Utc>>,
    /// Whether the user has manually switched to a different app
    #[serde(default)]
    pub user_switched: bool,
}

impl RecursorState {
    /// Create a new state with the given window
    pub fn new(window: WindowInfo) -> Self {
        Self {
            saved_window: window,
            saved_at: Utc::now(),
            last_active_check: None,
            user_switched: false,
        }
    }

    /// Mark that the user has switched to a different app
    pub fn mark_user_switched(&mut self) {
        self.user_switched = true;
        self.last_active_check = Some(Utc::now());
    }

    /// Check if this state is stale (older than 1 hour)
    pub fn is_stale(&self) -> bool {
        let age = Utc::now() - self.saved_at;
        age.num_hours() >= 1
    }
}

/// Manager for state file operations
pub struct StateManager {
    state_path: PathBuf,
}

impl StateManager {
    /// Create a new state manager
    pub fn new() -> Result<Self> {
        let state_path = Self::get_state_path()?;
        Ok(Self { state_path })
    }

    /// Get the path to the state file
    fn get_state_path() -> Result<PathBuf> {
        let home = dirs::home_dir().context("Could not determine home directory")?;
        let cursor_dir = home.join(".cursor");

        // Ensure .cursor directory exists
        if !cursor_dir.exists() {
            fs::create_dir_all(&cursor_dir).context("Failed to create .cursor directory")?;
        }

        Ok(cursor_dir.join("recursor_state.json"))
    }

    /// Save the current window state
    pub fn save(&self, state: &RecursorState) -> Result<()> {
        let json = serde_json::to_string_pretty(state).context("Failed to serialize state")?;

        fs::write(&self.state_path, json).context("Failed to write state file")?;

        Ok(())
    }

    /// Load the saved window state
    pub fn load(&self) -> Result<Option<RecursorState>> {
        if !self.state_path.exists() {
            return Ok(None);
        }

        let json = fs::read_to_string(&self.state_path).context("Failed to read state file")?;

        let state: RecursorState =
            serde_json::from_str(&json).context("Failed to parse state file")?;

        // Check if state is stale
        if state.is_stale() {
            // Remove stale state file
            let _ = fs::remove_file(&self.state_path);
            return Ok(None);
        }

        Ok(Some(state))
    }

    /// Clear the saved state
    pub fn clear(&self) -> Result<()> {
        if self.state_path.exists() {
            fs::remove_file(&self.state_path).context("Failed to remove state file")?;
        }
        Ok(())
    }

    /// Update the state to mark that the user switched apps
    pub fn mark_user_switched(&self) -> Result<()> {
        if let Some(mut state) = self.load()? {
            state.mark_user_switched();
            self.save(&state)?;
        }
        Ok(())
    }

    /// Check if we should restore focus to Cursor
    ///
    /// Returns false if the user has manually switched to a different app
    /// while the agent was working.
    pub fn should_restore_cursor(&self, current_window: &WindowInfo) -> Result<bool> {
        let state = match self.load()? {
            Some(s) => s,
            None => return Ok(true), // No saved state, always restore
        };

        // If user explicitly switched apps, don't force them back
        if state.user_switched {
            return Ok(false);
        }

        // If current window is different from saved AND not Cursor,
        // the user has switched apps
        if current_window != &state.saved_window && !current_window.is_cursor() {
            return Ok(false);
        }

        Ok(true)
    }
}

impl Default for StateManager {
    fn default() -> Self {
        Self::new().expect("Failed to create state manager")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_state_staleness() {
        let window = WindowInfo {
            pid: 1234,
            window_id: "test".to_string(),
            app_name: "Test".to_string(),
            title: "Test Window".to_string(),
        };

        let state = RecursorState::new(window);
        assert!(!state.is_stale());
    }

    #[test]
    fn test_user_switched() {
        let window = WindowInfo {
            pid: 1234,
            window_id: "test".to_string(),
            app_name: "Test".to_string(),
            title: "Test Window".to_string(),
        };

        let mut state = RecursorState::new(window);
        assert!(!state.user_switched);

        state.mark_user_switched();
        assert!(state.user_switched);
    }
}
