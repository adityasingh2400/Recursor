//! Recursor - The "Bounce Back" Utility for Cursor AI Agents
//!
//! Automatically saves and restores window focus when using Cursor AI agents.
//!
//! # Usage
//!
//! ```bash
//! # Save current window (called by beforeSubmitPrompt hook)
//! recursor save
//!
//! # Restore focus to Cursor (called by stop hook)
//! recursor restore
//!
//! # Check current status
//! recursor status
//!
//! # Trigger permission prompts (macOS)
//! recursor permissions
//! ```

mod hooks;
mod platform;
mod state;

use anyhow::Result;
use chrono::Utc;
use clap::{Parser, Subcommand};
use platform::{create_window_manager, WindowManager};
use state::StateManager;
use std::path::PathBuf;
use std::time::Duration;

const SHELL_FAILSAFE_DELAY_SECONDS: u64 = 5;

/// Check if Recursor is enabled by reading the config file
/// Returns true if enabled (default), false if explicitly disabled
fn is_enabled() -> bool {
    let config_path = get_config_path();

    if !config_path.exists() {
        return true; // Default to enabled
    }

    match std::fs::read_to_string(&config_path) {
        Ok(contents) => {
            // Parse JSON and check "enabled" field
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&contents) {
                return json
                    .get("enabled")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(true);
            }
            true // Default to enabled if parse fails
        }
        Err(_) => true, // Default to enabled if read fails
    }
}

/// Get the path to the config file
fn get_config_path() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".cursor")
        .join("recursor_config.json")
}

/// Recursor - The "Bounce Back" Utility for Cursor AI Agents
#[derive(Parser)]
#[command(name = "recursor")]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Save the current active window (excluding Cursor) and optionally return focus to it
    Save {
        /// Don't switch focus back to the saved window
        #[arg(long)]
        no_focus: bool,
    },

    /// Restore focus to Cursor (if appropriate based on intelligent refocus logic)
    Restore,

    /// Called before shell execution (always allow; no focus change)
    BeforeShell,

    /// Called after shell execution - switch back to video and resume
    AfterShell,

    /// Show current saved state
    Status,

    /// Trigger permission prompts (macOS) by attempting window operations
    Permissions,

    /// Clear saved state
    Clear,

    /// Check if a shell command is still pending after timeout (failsafe)
    CheckIdle {
        /// The conversation ID to check
        conversation_id: String,
        /// Delay before checking pending state (used by failsafe timer process)
        #[arg(long, default_value_t = 0)]
        delay_seconds: u64,
    },
}

fn main() {
    if let Err(e) = run() {
        eprintln!("Error: {:#}", e);
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Save { no_focus } => cmd_save(no_focus),
        Commands::Restore => cmd_restore(),
        Commands::BeforeShell => cmd_before_shell(),
        Commands::AfterShell => cmd_after_shell(),
        Commands::Status => cmd_status(),
        Commands::Permissions => cmd_permissions(),
        Commands::Clear => cmd_clear(),
        Commands::CheckIdle {
            conversation_id,
            delay_seconds,
        } => cmd_check_idle(&conversation_id, delay_seconds),
    }
}

/// Save command - called by beforeSubmitPrompt hook
fn cmd_save(no_focus: bool) -> Result<()> {
    // Check if Recursor is enabled
    if !is_enabled() {
        // Just output allow response without any window management
        hooks::write_output(&hooks::BeforeSubmitPromptOutput::allow())?;
        return Ok(());
    }

    let wm = create_window_manager();
    let state_mgr = StateManager::new()?;

    // Read hook input from stdin (if available)
    let input: Option<hooks::BeforeSubmitPromptInput> = hooks::try_read_input();

    // Get conversation_id from hook input, or use a default
    let conversation_id = input
        .as_ref()
        .and_then(|i| i.common.conversation_id.clone())
        .unwrap_or_else(|| "default".to_string());

    // Get the current active window (this is Cursor, since user just submitted prompt)
    let cursor_window = wm.get_active_window().ok();

    // Get the previous app the user was using (before they switched to Cursor)
    let previous_window = wm.get_previous_window().ok();

    // Save state: remember the window to return to after commands are approved.
    // Prefer the previous app, but fall back to the current Cursor window if needed.
    let window_to_save = select_window_to_save(cursor_window.clone(), previous_window.clone());

    if let Some(ref w) = window_to_save {
        state_mgr.save_conversation(&conversation_id, w.clone(), cursor_window.clone())?;
    }

    // Update menu bar status with rich information
    if let Some(ref w) = window_to_save {
        let media_playing = if w.app_name == "Google Chrome" {
            Some(wm.is_youtube_playing())
        } else {
            None
        };
        wm.update_menu_bar_status_full(
            "working",
            Some("Agent working on task..."),
            Some(&w.app_name),
            Some(&w.title),
            media_playing,
        );
    }

    // Switch focus back to the previous app (unless explicitly disabled).
    if !no_focus {
        if let Some(ref prev) = previous_window {
            // Small delay to let the prompt submission complete.
            std::thread::sleep(Duration::from_millis(50));

            // Focus the previous window first.
            let _ = wm.focus_window(prev);

            // If it's Chrome, resume only if a YouTube tab is paused.
            if prev.app_name == "Google Chrome" {
                std::thread::sleep(Duration::from_millis(150));
                let _ = wm.resume_youtube(&prev.title);
            }
        }
    }

    // Output success response for the hook
    hooks::write_output(&hooks::BeforeSubmitPromptOutput::allow())?;

    Ok(())
}

/// Restore command - called by stop hook when agent finishes
/// ALWAYS brings user to Cursor so they can see the results
fn cmd_restore() -> Result<()> {
    // Check if Recursor is enabled
    if !is_enabled() {
        // Just output empty response without any window management
        hooks::write_output(&hooks::StopOutput::empty())?;
        return Ok(());
    }

    let wm = create_window_manager();
    let state_mgr = StateManager::new()?;

    // Read hook input from stdin (if available)
    let input: Option<hooks::StopInput> = hooks::try_read_input();

    // Get conversation_id from hook input
    let conversation_id = input
        .as_ref()
        .and_then(|i| i.common.conversation_id.clone())
        .unwrap_or_else(|| "default".to_string());

    // Load saved state BEFORE clearing - we need the specific Cursor window info
    let saved_state = state_mgr.load_conversation(&conversation_id)?;

    // Pause YouTube if the saved previous window was Chrome with YouTube
    // We use the saved state (not get_active_window) because Cursor may already
    // have focus by the time this hook fires
    if let Some(ref state) = saved_state {
        if state.saved_window.app_name == "Google Chrome" && state.saved_window.pid > 0 {
            let script = format!(
                r#"
                tell application "System Events"
                    set chromeProc to first application process whose unix id is {}
                    set allWins to every window of chromeProc
                    set winCount to count of allWins
                    repeat with i from 1 to winCount
                        try
                            set winName to name of item i of allWins
                            if winName contains "YouTube" and winName contains "Audio playing" then
                                set frontmost of chromeProc to true
                                delay 0.15
                                perform action "AXRaise" of (item i of allWins)
                                delay 0.2
                                keystroke "k"
                                return "paused"
                            end if
                        end try
                    end repeat
                    return "not_playing"
                end tell
                "#,
                state.saved_window.pid
            );
            let _ = std::process::Command::new("osascript")
                .args(["-e", &script])
                .output();
        }
    }

    // Small delay then bring user to the CORRECT Cursor window.
    // When multiple Cursor windows are open, we must focus the specific one
    // where the prompt was submitted, not just any Cursor window.
    std::thread::sleep(Duration::from_millis(100));
    if let Some(ref state) = saved_state {
        if let Some(ref cursor_win) = state.cursor_window {
            let _ = wm.focus_cursor_window(cursor_win);
        } else {
            let _ = wm.focus_cursor();
        }
    } else {
        let _ = wm.focus_cursor();
    }

    // Update menu bar status - agent finished, now idle
    wm.update_menu_bar_status_full("idle", Some("Agent finished"), None, None, None);

    // Clear the saved state for this conversation
    state_mgr.clear_conversation(&conversation_id)?;

    // Output response for the hook
    hooks::write_output(&hooks::StopOutput::empty())?;

    Ok(())
}

/// BeforeShell command - called before every shell execution.
/// Instead of immediately bringing user to Cursor, we save state and spawn a 5-second
/// failsafe timer. If the command is still pending after 5 seconds, the failsafe
/// brings the user to Cursor.
fn cmd_before_shell() -> Result<()> {
    // Check if Recursor is enabled
    if !is_enabled() {
        // Just allow the command without any window management
        let output = serde_json::json!({ "permission": "allow" });
        println!("{}", output);
        return Ok(());
    }

    let state_mgr = StateManager::new()?;
    let wm = create_window_manager();

    // Read hook input to get the command
    let input: Option<hooks::BeforeShellInput> = hooks::try_read_input();

    let conversation_id = input
        .as_ref()
        .and_then(|i| i.common.conversation_id.clone())
        .unwrap_or_else(|| "default".to_string());

    // Get current window and determine the secondary window to track
    let current_window = wm.get_active_window().ok();

    // Determine the secondary window (the one to return to after command completes)
    // If user is in Cursor, get the previous window they were in
    // If user is NOT in Cursor, use current window as secondary
    let secondary_window = if let Some(ref current) = current_window {
        if current.is_cursor() {
            // User is in Cursor - get the previous window they came from
            wm.get_previous_window().ok()
        } else {
            // User is in another app - that's our secondary window
            Some(current.clone())
        }
    } else {
        None
    };

    // Always save state and spawn failsafe timer
    // This fixes the back-to-back command issue where command 2 fires while user is still in Cursor
    if let Some(ref secondary) = secondary_window {
        let shell_conv_id = format!("{}_shell", conversation_id);
        state_mgr.save_conversation(&shell_conv_id, secondary.clone(), None)?;

        // Spawn a 5-second failsafe timer
        // If the command is still pending after 5 seconds, check-idle will bring user to Cursor
        spawn_failsafe_timer(&conversation_id);
    }

    // Always allow the command to proceed
    let output = serde_json::json!({ "permission": "allow" });
    println!("{}", output);
    Ok(())
}

/// Spawn a background process that will check if the shell command is still pending after 5 seconds
fn spawn_failsafe_timer(conversation_id: &str) {
    use std::process::{Command, Stdio};

    // Get the path to the recursor binary
    let recursor_path = std::env::current_exe().unwrap_or_else(|_| "recursor".into());

    // Spawn a detached check-idle process that sleeps internally before checking.
    // This avoids shell interpolation issues for conversation IDs and binary paths.
    let _ = Command::new(recursor_path)
        .arg("check-idle")
        .arg("--delay-seconds")
        .arg(SHELL_FAILSAFE_DELAY_SECONDS.to_string())
        .arg("--")
        .arg(conversation_id)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn();
}

/// AfterShell command - called after a shell command has run.
/// Switch user back to where they were (e.g., YouTube) if we brought them to Cursor.
fn cmd_after_shell() -> Result<()> {
    // Check if Recursor is enabled
    if !is_enabled() {
        // No window management when disabled
        return Ok(());
    }

    let wm = create_window_manager();
    let state_mgr = StateManager::new()?;

    // Read raw stdin and parse
    use std::io::{self, BufRead};
    let stdin = io::stdin();
    let mut raw_input = String::new();
    for l in stdin.lock().lines().map_while(Result::ok) {
        raw_input.push_str(&l);
        raw_input.push('\n');
    }

    let input: Option<hooks::AfterShellInput> = serde_json::from_str(&raw_input).ok();

    let conversation_id = input
        .as_ref()
        .and_then(|i| i.common.conversation_id.clone())
        .unwrap_or_else(|| "default".to_string());

    // Check for shell-specific saved state
    let shell_conv_id = format!("{}_shell", conversation_id);

    if let Some(state) = state_mgr.load_conversation(&shell_conv_id)? {
        // We saved state in beforeShellExecution, meaning we brought user to Cursor
        // Now bring them back to where they were
        let prev = &state.saved_window;

        std::thread::sleep(Duration::from_millis(100));
        let _ = wm.focus_window(prev);

        // Resume YouTube if it was Chrome
        let mut media_playing = None;
        if prev.app_name == "Google Chrome" {
            std::thread::sleep(Duration::from_millis(150));
            media_playing = Some(wm.resume_youtube(&prev.title));
        }

        // Clear the shell-specific state
        state_mgr.clear_conversation(&shell_conv_id)?;

        // Update menu bar - command approved, back to working
        wm.update_menu_bar_status_full(
            "working",
            Some("Agent working..."),
            Some(&prev.app_name),
            Some(&prev.title),
            media_playing,
        );
    }

    Ok(())
}

/// CheckIdle command - failsafe that brings user to Cursor if shell command is still pending
/// Called by background timer spawned in beforeShellExecution after 5 seconds
fn cmd_check_idle(conversation_id: &str, delay_seconds: u64) -> Result<()> {
    // Check if Recursor is enabled
    if !is_enabled() {
        // Don't pull user to Cursor when disabled
        return Ok(());
    }

    if delay_seconds > 0 {
        std::thread::sleep(Duration::from_secs(delay_seconds));
    }

    let wm = create_window_manager();
    let state_mgr = StateManager::new()?;

    // Check for shell-specific saved state
    let shell_conv_id = format!("{}_shell", conversation_id);

    if let Some(state) = state_mgr.load_conversation(&shell_conv_id)? {
        // Verify that at least 5 seconds have actually elapsed since state was saved
        // This prevents race conditions where timer fires but command just started
        let elapsed = Utc::now() - state.saved_at;
        if elapsed.num_seconds() < SHELL_FAILSAFE_DELAY_SECONDS as i64 {
            // Not enough time has passed, don't bring user to Cursor yet
            return Ok(());
        }

        // State still exists after 5 seconds - command is likely waiting for approval
        // This is our failsafe: bring user to Cursor

        // Pause YouTube if user was watching.
        let media_playing = if state.saved_window.app_name == "Google Chrome"
            && wm.pause_youtube_if_playing(&state.saved_window.title)
        {
            Some(false)
        } else {
            None
        };

        // Get the Cursor window from the main conversation state
        if let Some(main_state) = state_mgr.load_conversation(conversation_id)? {
            if let Some(ref cursor_win) = main_state.cursor_window {
                let _ = wm.focus_cursor_window(cursor_win);
            } else {
                let _ = wm.focus_cursor();
            }
        } else {
            let _ = wm.focus_cursor();
        }

        // Update menu bar to indicate we're waiting for approval
        wm.update_menu_bar_status_full(
            "approval_needed",
            Some("Waiting for command approval..."),
            Some(&state.saved_window.app_name),
            Some(&state.saved_window.title),
            media_playing,
        );
    }
    // If state doesn't exist, command already finished - do nothing

    Ok(())
}

fn select_window_to_save(
    cursor_window: Option<platform::WindowInfo>,
    previous_window: Option<platform::WindowInfo>,
) -> Option<platform::WindowInfo> {
    previous_window.or(cursor_window)
}

/// Status command - show current saved state
fn cmd_status() -> Result<()> {
    let state_mgr = StateManager::new()?;

    let conversations = state_mgr.get_all_conversations()?;

    if conversations.is_empty() {
        println!("No saved state.");
        return Ok(());
    }

    println!("Recursor State:");
    println!("===============");

    for (conv_id, state) in conversations {
        println!("\nConversation: {}", conv_id);
        println!("  Saved Window:");
        println!("    App: {}", state.saved_window.app_name);
        println!("    Title: {}", state.saved_window.title);
        println!("    PID: {}", state.saved_window.pid);
        if let Some(ref cursor_win) = state.cursor_window {
            println!("  Cursor Window:");
            println!("    Title: {}", cursor_win.title);
            println!("    PID: {}", cursor_win.pid);
        }
        println!("  Saved At: {}", state.saved_at);
        println!("  User Switched: {}", state.user_switched);
    }

    Ok(())
}

/// Permissions command - trigger permission prompts on macOS
fn cmd_permissions() -> Result<()> {
    let wm = create_window_manager();

    println!("Recursor Permissions Check");
    println!("==========================");
    println!();

    // Try to get the active window
    print!("Checking window access... ");
    match wm.get_active_window() {
        Ok(window) => {
            println!("OK");
            println!("  Current window: {} ({})", window.app_name, window.title);
        }
        Err(e) => {
            println!("FAILED");
            println!("  Error: {}", e);
            println!();
            println!("On macOS, you may need to grant permissions:");
            println!("  1. Open System Preferences > Privacy & Security");
            println!("  2. Go to Accessibility");
            println!("  3. Add and enable 'recursor' or 'Terminal'");
            println!();

            #[cfg(target_os = "macos")]
            {
                println!("Opening System Preferences...");
                let _ = std::process::Command::new("open")
                    .arg("x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility")
                    .spawn();
            }

            return Err(e);
        }
    }

    // Try to focus Cursor
    print!("Checking Cursor focus... ");
    match wm.focus_cursor() {
        Ok(()) => {
            println!("OK");
        }
        Err(e) => {
            println!("FAILED (Cursor may not be running)");
            println!("  Error: {}", e);
        }
    }

    println!();
    println!("Permissions check complete!");

    Ok(())
}

/// Clear command - remove saved state
fn cmd_clear() -> Result<()> {
    let state_mgr = StateManager::new()?;
    state_mgr.clear()?;
    println!("Saved state cleared.");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn window(app_name: &str, title: &str, pid: u32) -> platform::WindowInfo {
        platform::WindowInfo {
            pid,
            window_id: format!("{}:1", pid),
            app_name: app_name.to_string(),
            title: title.to_string(),
        }
    }

    #[test]
    fn select_window_to_save_prefers_previous_window() {
        let cursor = window("Cursor", "main.rs - Recursor - Cursor", 100);
        let previous = window("Google Chrome", "YouTube", 200);

        let selected = select_window_to_save(Some(cursor), Some(previous.clone()));
        assert_eq!(selected, Some(previous));
    }

    #[test]
    fn select_window_to_save_falls_back_to_cursor_window() {
        let cursor = window("Cursor", "main.rs - Recursor - Cursor", 100);

        let selected = select_window_to_save(Some(cursor.clone()), None);
        assert_eq!(selected, Some(cursor));
    }

    #[test]
    fn select_window_to_save_handles_missing_windows() {
        assert_eq!(select_window_to_save(None, None), None);
    }
}
