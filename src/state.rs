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
use std::time::{SystemTime, UNIX_EPOCH};

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

    #[cfg(test)]
    fn with_state_path(state_path: PathBuf) -> Self {
        Self { state_path }
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
        let before_cleanup = state.conversations.len();

        // Clean up stale entries
        state.cleanup_stale();
        if state.conversations.len() != before_cleanup {
            // Best-effort persistence of cleanup; stale entries can otherwise
            // linger indefinitely if no new save operation occurs.
            let _ = self.save_full(&state);
        }

        Ok(state)
    }

    /// Save the full state
    fn save_full(&self, state: &RecursorState) -> Result<()> {
        let json = serde_json::to_string_pretty(state).context("Failed to serialize state")?;
        if let Some(parent) = self.state_path.parent() {
            fs::create_dir_all(parent).context("Failed to create state directory")?;
        }

        let temp_path = self.temp_state_path();
        fs::write(&temp_path, json).context("Failed to write temporary state file")?;
        if let Err(err) = fs::rename(&temp_path, &self.state_path) {
            let _ = fs::remove_file(&temp_path);
            return Err(err).context("Failed to replace state file atomically");
        }
        Ok(())
    }

    fn temp_state_path(&self) -> PathBuf {
        let parent = self
            .state_path
            .parent()
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("."));
        let file_name = self
            .state_path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("recursor_state.json");
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);

        parent.join(format!(
            "{}.tmp.{}.{}",
            file_name,
            std::process::id(),
            nonce
        ))
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
    use chrono::Duration as ChronoDuration;
    use std::collections::HashMap;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn test_window() -> WindowInfo {
        WindowInfo {
            pid: 1234,
            window_id: "test:1".to_string(),
            app_name: "Test".to_string(),
            title: "Test Window".to_string(),
        }
    }

    fn unique_test_dir(label: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let dir = std::env::temp_dir().join(format!(
            "recursor_state_{}_{}_{}",
            label,
            std::process::id(),
            nonce
        ));
        fs::create_dir_all(&dir).expect("failed to create temp test dir");
        dir
    }

    #[test]
    fn test_state_staleness() {
        let state = ConversationState::new(test_window(), None);
        assert!(!state.is_stale());
    }

    #[test]
    fn stale_conversations_are_persistently_cleaned_up() {
        let dir = unique_test_dir("stale_cleanup");
        let state_path = dir.join("recursor_state.json");
        let manager = StateManager::with_state_path(state_path.clone());

        let mut conversations = HashMap::new();
        conversations.insert(
            "stale".to_string(),
            ConversationState {
                saved_window: test_window(),
                cursor_window: None,
                saved_at: Utc::now() - ChronoDuration::hours(2),
                user_switched: false,
            },
        );
        conversations.insert(
            "fresh".to_string(),
            ConversationState {
                saved_window: test_window(),
                cursor_window: None,
                saved_at: Utc::now(),
                user_switched: false,
            },
        );
        let state = RecursorState { conversations };
        let json = serde_json::to_string_pretty(&state).expect("serialize state");
        fs::write(&state_path, json).expect("write initial state");

        let loaded = manager.get_all_conversations().expect("load state");
        assert!(loaded.contains_key("fresh"));
        assert!(!loaded.contains_key("stale"));

        let persisted_json = fs::read_to_string(&state_path).expect("read persisted state");
        let persisted: RecursorState =
            serde_json::from_str(&persisted_json).expect("parse persisted state");
        assert!(persisted.conversations.contains_key("fresh"));
        assert!(!persisted.conversations.contains_key("stale"));

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn save_conversation_does_not_leave_tmp_files() {
        let dir = unique_test_dir("tmp_cleanup");
        let state_path = dir.join("recursor_state.json");
        let manager = StateManager::with_state_path(state_path.clone());

        manager
            .save_conversation("conv-1", test_window(), None)
            .expect("save state");

        assert!(state_path.exists());

        let has_tmp = fs::read_dir(&dir)
            .expect("read dir")
            .filter_map(Result::ok)
            .any(|entry| entry.file_name().to_string_lossy().contains(".tmp."));
        assert!(!has_tmp);

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn invalid_json_falls_back_to_default_state() {
        let dir = unique_test_dir("invalid_json");
        let state_path = dir.join("recursor_state.json");
        let manager = StateManager::with_state_path(state_path.clone());

        fs::write(&state_path, "{invalid").expect("write invalid json");
        let loaded = manager
            .get_all_conversations()
            .expect("load should not fail");
        assert!(loaded.is_empty());

        let _ = fs::remove_dir_all(&dir);
    }
}
