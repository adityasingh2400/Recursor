//! Cursor hooks protocol handling
//!
//! Handles the JSON stdin/stdout protocol used by Cursor hooks.
//! Scripts receive JSON input via stdin and return JSON output via stdout.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::io::{self, BufRead, Write};

/// Common fields present in all hook inputs
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct HookInput {
    /// Stable ID of the conversation
    #[serde(default)]
    pub conversation_id: Option<String>,
    /// Current generation ID
    #[serde(default)]
    pub generation_id: Option<String>,
    /// Model being used
    #[serde(default)]
    pub model: Option<String>,
    /// Which hook is being run
    #[serde(default)]
    pub hook_event_name: Option<String>,
    /// Cursor version
    #[serde(default)]
    pub cursor_version: Option<String>,
    /// Workspace roots
    #[serde(default)]
    pub workspace_roots: Vec<String>,
    /// User email
    #[serde(default)]
    pub user_email: Option<String>,
}

/// Input for beforeSubmitPrompt hook
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct BeforeSubmitPromptInput {
    #[serde(flatten)]
    pub common: HookInput,
    /// The user's prompt text
    #[serde(default)]
    pub prompt: Option<String>,
}

/// Output for beforeSubmitPrompt hook
#[derive(Debug, Serialize)]
pub struct BeforeSubmitPromptOutput {
    /// Whether to continue with the prompt submission
    #[serde(rename = "continue")]
    pub continue_submission: bool,
    /// Optional message to show to user if blocked
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_message: Option<String>,
}

impl BeforeSubmitPromptOutput {
    /// Create an output that allows the submission to proceed
    pub fn allow() -> Self {
        Self {
            continue_submission: true,
            user_message: None,
        }
    }

    /// Create an output that blocks the submission
    #[allow(dead_code)]
    pub fn block(message: &str) -> Self {
        Self {
            continue_submission: false,
            user_message: Some(message.to_string()),
        }
    }
}

/// Input for afterShellExecution hook
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct AfterShellInput {
    #[serde(flatten)]
    pub common: HookInput,
    #[serde(default)]
    pub command: Option<String>,
    #[serde(default)]
    pub output: Option<String>,
    #[serde(default)]
    pub duration: Option<f64>,
}

/// Input for beforeShellExecution hook
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct BeforeShellInput {
    #[serde(flatten)]
    pub common: HookInput,
    /// The command about to be executed
    #[serde(default)]
    pub command: Option<String>,
}

/// Input for stop hook
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct StopInput {
    #[serde(flatten)]
    pub common: HookInput,
    /// Status of the agent loop
    #[serde(default)]
    pub status: Option<String>,
    /// Number of times stop hook has triggered auto follow-up
    #[serde(default)]
    pub loop_count: u32,
}

/// Output for stop hook
#[derive(Debug, Serialize)]
pub struct StopOutput {
    /// Optional follow-up message to auto-submit
    #[serde(skip_serializing_if = "Option::is_none")]
    pub followup_message: Option<String>,
}

impl StopOutput {
    /// Create an empty output (no follow-up)
    pub fn empty() -> Self {
        Self {
            followup_message: None,
        }
    }

    /// Create an output with a follow-up message
    #[allow(dead_code)]
    pub fn with_followup(message: &str) -> Self {
        Self {
            followup_message: Some(message.to_string()),
        }
    }
}

/// Read JSON input from stdin
pub fn read_input<T: for<'de> Deserialize<'de>>() -> Result<T> {
    let stdin = io::stdin();
    let mut input = String::new();

    // Read all input from stdin
    for line in stdin.lock().lines() {
        let line = line.context("Failed to read from stdin")?;
        input.push_str(&line);
        input.push('\n');
    }

    // Parse JSON
    serde_json::from_str(&input).context("Failed to parse JSON input")
}

/// Write JSON output to stdout
pub fn write_output<T: Serialize>(output: &T) -> Result<()> {
    let json = serde_json::to_string(output).context("Failed to serialize output")?;

    let stdout = io::stdout();
    let mut handle = stdout.lock();

    writeln!(handle, "{}", json).context("Failed to write to stdout")?;
    handle.flush().context("Failed to flush stdout")?;

    Ok(())
}

/// Try to read input, returning None if stdin is empty or not valid JSON
pub fn try_read_input<T: for<'de> Deserialize<'de>>() -> Option<T> {
    read_input().ok()
}
