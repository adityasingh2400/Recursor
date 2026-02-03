//! State file management for Recursor
//!
//! Handles saving and loading the window state between save and restore operations.
//! Implements "intelligent refocus" logic to avoid forcing users back to windows
//! they've manually switched away from.
//!
//! Uses conversation_id to track state per Cursor window, so multiple Cursor
//! windows can each restore to the correct window.

use crate::platform::WindowInfo;
use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

/// State for a single conversation/window
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversationState {
    /// The window that was active when the user submitted a prompt
    pub saved_window: WindowInfo,
    /// The Cursor window that triggered this save (so we can restore to it)
    #[serde(default)]
    pub cursor_window: Option<WindowInfo>,
    /// When the window was saved
    pub saved_at: DateTime<Utc>,
    /// Whether the user has manually switched to a different app
    #[serde(default)]
    pub user_switched: bool,
}

impl ConversationState {
    /// Create a new state with the given windows
    pub fn new(saved_window: WindowInfo, cursor_window: Option<WindowInfo>) -> Self {
        Self {
            saved_window,
            cursor_window,
            saved_at: Utc::now(),
            user_switched: false,
        }
    }

    /// Check if this state is stale (older than 1 hour)
    pub fn is_stale(&self) -> bool {
        let age = Utc::now() - self.saved_at;
        age.num_hours() >= 1
    }
}

/// Full state file format - maps conversation_id to state
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RecursorState {
    /// Map of conversation_id -> state
    #[serde(default)]
    pub conversations: HashMap<String, ConversationState>,
}

impl RecursorState {
    /// Clean up stale entries
    pub fn cleanup_stale(&mut self) {
        self.conversations.retain(|_, state| !state.is_stale());
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

    /// Load the full state
    fn load_full(&self) -> Result<RecursorState> {
        if !self.state_path.exists() {
            return Ok(RecursorState::default());
        }

        let json = fs::read_to_string(&self.state_path).context("Failed to read state file")?;

        let mut state: RecursorState = serde_json::from_str(&json).unwrap_or_default();

        // Clean up stale entries
        state.cleanup_stale();

        Ok(state)
    }

    /// Save the full state
    fn save_full(&self, state: &RecursorState) -> Result<()> {
        let json = serde_json::to_string_pretty(state).context("Failed to serialize state")?;
        fs::write(&self.state_path, json).context("Failed to write state file")?;
        Ok(())
    }

    /// Save state for a specific conversation
    pub fn save_conversation(
        &self,
        conversation_id: &str,
        saved_window: WindowInfo,
        cursor_window: Option<WindowInfo>,
    ) -> Result<()> {
        let mut state = self.load_full()?;

        let conv_state = ConversationState::new(saved_window, cursor_window);
        state
            .conversations
            .insert(conversation_id.to_string(), conv_state);

        self.save_full(&state)?;
        Ok(())
    }

    /// Load state for a specific conversation
    pub fn load_conversation(&self, conversation_id: &str) -> Result<Option<ConversationState>> {
        let state = self.load_full()?;
        Ok(state.conversations.get(conversation_id).cloned())
    }

    /// Clear state for a specific conversation
    pub fn clear_conversation(&self, conversation_id: &str) -> Result<()> {
        let mut state = self.load_full()?;
        state.conversations.remove(conversation_id);
        self.save_full(&state)?;
        Ok(())
    }

    /// Clear all saved state
    pub fn clear(&self) -> Result<()> {
        if self.state_path.exists() {
            fs::remove_file(&self.state_path).context("Failed to remove state file")?;
        }
        Ok(())
    }

    /// Get all conversations (for status display)
    pub fn get_all_conversations(&self) -> Result<HashMap<String, ConversationState>> {
        let state = self.load_full()?;
        Ok(state.conversations)
    }

    /// Check if we should restore focus to Cursor for a conversation
    #[allow(dead_code)]
    pub fn should_restore_cursor(
        &self,
        conversation_id: &str,
        current_window: &WindowInfo,
    ) -> Result<bool> {
        let conv_state = match self.load_conversation(conversation_id)? {
            Some(s) => s,
            None => return Ok(true), // No saved state, always restore
        };

        // If user explicitly switched apps, don't force them back
        if conv_state.user_switched {
            return Ok(false);
        }

        // If user is in Cursor already, no need to restore
        if current_window.is_cursor() {
            return Ok(true); // Still restore (re-focus) to make sure correct window is front
        }

        // If user is still in the app we sent them to, restore to Cursor
        // If user manually switched to a DIFFERENT app, don't interrupt them
        if current_window.app_name != conv_state.saved_window.app_name {
            return Ok(false); // User switched to a different app, don't interrupt
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

        let state = ConversationState::new(window, None);
        assert!(!state.is_stale());
    }
}
