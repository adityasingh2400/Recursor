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

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use platform::{create_window_manager, WindowManager};
use state::StateManager;

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

    /// Show current saved state
    Status,

    /// Trigger permission prompts (macOS) by attempting window operations
    Permissions,

    /// Clear saved state
    Clear,
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
        Commands::Status => cmd_status(),
        Commands::Permissions => cmd_permissions(),
        Commands::Clear => cmd_clear(),
    }
}

/// Save command - called by beforeSubmitPrompt hook
fn cmd_save(no_focus: bool) -> Result<()> {
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

    // If the active window is not Cursor, something is off - but save it anyway
    // The user might have a different setup
    
    // We need to get the PREVIOUS window (before Cursor got focus)
    // Unfortunately, we can't easily get this. The workaround is:
    // 1. Save the current Cursor window info
    // 2. After a brief delay, the user will have switched back to their previous app
    //    OR we focus back to it
    
    // For now, if Cursor is active, we'll save Cursor's info and return
    // The actual "previous window" logic happens via the focus_window call
    
    if let Some(ref cw) = cursor_window {
        if wm.is_cursor_window(cw) {
            // Cursor is active - this is expected
            // We need to save state and then switch to previous app
            
            // Small delay to let the prompt be processed
            std::thread::sleep(std::time::Duration::from_millis(50));
            
            // Try to get the window that will be focused after we switch away
            // This is tricky - we'll save Cursor's window and handle restore properly
            
            // For the "previous window", we'll need to switch focus and then capture it
            // OR we can use platform-specific APIs to get the window list
            
            // Simpler approach: just save Cursor window, and on restore, focus THIS specific window
            state_mgr.save_conversation(&conversation_id, cw.clone(), cursor_window.clone())?;
            
            // Output success response for the hook
            hooks::write_output(&hooks::BeforeSubmitPromptOutput::allow())?;
            return Ok(());
        }
    }

    // If we get here, the active window is NOT Cursor (user submitted from elsewhere?)
    // Save it as the window to return to
    let active_window = wm
        .get_active_window()
        .context("Failed to get active window")?;

    // Save the window state with conversation_id
    state_mgr.save_conversation(&conversation_id, active_window.clone(), cursor_window)?;

    // Optionally switch focus back to the saved window
    if !no_focus {
        std::thread::sleep(std::time::Duration::from_millis(100));
        if let Err(e) = wm.focus_window(&active_window) {
            eprintln!("Warning: Could not focus saved window: {}", e);
        }
    }

    // Output success response for the hook
    hooks::write_output(&hooks::BeforeSubmitPromptOutput::allow())?;

    Ok(())
}

/// Restore command - called by stop hook
fn cmd_restore() -> Result<()> {
    let wm = create_window_manager();
    let state_mgr = StateManager::new()?;

    // Read hook input from stdin (if available)
    let input: Option<hooks::StopInput> = hooks::try_read_input();
    
    // Get conversation_id from hook input
    let conversation_id = input
        .as_ref()
        .and_then(|i| i.common.conversation_id.clone())
        .unwrap_or_else(|| "default".to_string());

    // Get current active window to check if user switched apps
    let current_window = wm.get_active_window().ok();

    // Check if we should restore focus to Cursor
    let should_restore = if let Some(ref current) = current_window {
        state_mgr.should_restore_cursor(&conversation_id, current)?
    } else {
        true // If we can't get current window, try to restore anyway
    };

    if should_restore {
        // Try to focus the specific Cursor window that triggered this conversation
        let conv_state = state_mgr.load_conversation(&conversation_id)?;
        
        if let Some(state) = conv_state {
            if let Some(ref cursor_win) = state.cursor_window {
                // Focus the specific Cursor window
                if wm.focus_cursor_window(cursor_win).is_err() {
                    // Fallback to generic Cursor focus
                    let _ = wm.focus_cursor();
                }
            } else {
                // No specific window saved, use generic focus
                let _ = wm.focus_cursor();
            }
        } else {
            // No state found, just focus Cursor
            let _ = wm.focus_cursor();
        }
    }

    // Clear the saved state for this conversation
    state_mgr.clear_conversation(&conversation_id)?;

    // Output response for the hook
    hooks::write_output(&hooks::StopOutput::empty())?;

    Ok(())
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
