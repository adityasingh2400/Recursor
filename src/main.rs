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
use state::{RecursorState, StateManager};

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
    let _input: Option<hooks::BeforeSubmitPromptInput> = hooks::try_read_input();

    // Get the current active window
    let active_window = wm
        .get_active_window()
        .context("Failed to get active window")?;

    // If the active window is Cursor, we don't need to save anything special
    // The user is already in Cursor
    if wm.is_cursor_window(&active_window) {
        // Output success response for the hook
        hooks::write_output(&hooks::BeforeSubmitPromptOutput::allow())?;
        return Ok(());
    }

    // Save the window state
    let state = RecursorState::new(active_window.clone());
    state_mgr.save(&state)?;

    // Optionally switch focus back to the saved window
    // (This allows the user to continue what they were doing while the agent works)
    if !no_focus {
        // Small delay to let Cursor process the prompt
        std::thread::sleep(std::time::Duration::from_millis(100));

        // Focus back to the saved window
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
    let _input: Option<hooks::StopInput> = hooks::try_read_input();

    // Get current active window to check if user switched apps
    let current_window = wm.get_active_window().ok();

    // Check if we should restore focus to Cursor
    let should_restore = if let Some(ref current) = current_window {
        state_mgr.should_restore_cursor(current)?
    } else {
        true // If we can't get current window, try to restore anyway
    };

    if should_restore {
        // Bring Cursor to the foreground
        if let Err(e) = wm.focus_cursor() {
            eprintln!("Warning: Could not focus Cursor: {}", e);
        }
    }

    // Clear the saved state
    state_mgr.clear()?;

    // Output response for the hook
    hooks::write_output(&hooks::StopOutput::empty())?;

    Ok(())
}

/// Status command - show current saved state
fn cmd_status() -> Result<()> {
    let state_mgr = StateManager::new()?;

    match state_mgr.load()? {
        Some(state) => {
            println!("Reflex State:");
            println!("  Saved Window:");
            println!("    App: {}", state.saved_window.app_name);
            println!("    Title: {}", state.saved_window.title);
            println!("    PID: {}", state.saved_window.pid);
            println!("    Window ID: {}", state.saved_window.window_id);
            println!("  Saved At: {}", state.saved_at);
            println!("  User Switched: {}", state.user_switched);
        }
        None => {
            println!("No saved state.");
        }
    }

    Ok(())
}

/// Permissions command - trigger permission prompts on macOS
fn cmd_permissions() -> Result<()> {
    let wm = create_window_manager();

    println!("Recursor Permissions Check");
    println!("========================");
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
