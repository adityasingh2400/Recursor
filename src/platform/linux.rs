//! Linux window management implementation
//!
//! Uses X11 via x11rb crate for window management, with xdotool/wmctrl fallback.
//! Note: Wayland support is limited due to protocol restrictions.

use super::{WindowInfo, WindowManager};
use anyhow::{anyhow, Context, Result};
use std::process::Command;
use x11rb::connection::Connection;
use x11rb::protocol::xproto::{AtomEnum, ConnectionExt, Window};
use x11rb::rust_connection::RustConnection;

/// Linux window manager implementation
pub struct LinuxWindowManager {
    /// X11 connection (if available)
    conn: Option<RustConnection>,
    /// Root window
    root: Window,
    /// Whether we're using X11 or fallback
    use_x11: bool,
}

impl LinuxWindowManager {
    pub fn new() -> Self {
        // Try to connect to X11
        match RustConnection::connect(None) {
            Ok((conn, screen_num)) => {
                let root = conn.setup().roots[screen_num].root;
                Self {
                    conn: Some(conn),
                    root,
                    use_x11: true,
                }
            }
            Err(_) => {
                // Fall back to command-line tools
                Self {
                    conn: None,
                    root: 0,
                    use_x11: false,
                }
            }
        }
    }

    /// Get active window using xdotool
    fn get_active_window_xdotool(&self) -> Result<WindowInfo> {
        // Get active window ID
        let output = Command::new("xdotool")
            .arg("getactivewindow")
            .output()
            .context("Failed to run xdotool getactivewindow")?;

        if !output.status.success() {
            return Err(anyhow!("xdotool getactivewindow failed"));
        }

        let window_id = String::from_utf8_lossy(&output.stdout).trim().to_string();

        // Get window PID
        let pid_output = Command::new("xdotool")
            .args(["getwindowpid", &window_id])
            .output()
            .context("Failed to run xdotool getwindowpid")?;

        let pid: u32 = if pid_output.status.success() {
            String::from_utf8_lossy(&pid_output.stdout)
                .trim()
                .parse()
                .unwrap_or(0)
        } else {
            0
        };

        // Get window name
        let name_output = Command::new("xdotool")
            .args(["getwindowname", &window_id])
            .output()
            .context("Failed to run xdotool getwindowname")?;

        let title = if name_output.status.success() {
            String::from_utf8_lossy(&name_output.stdout)
                .trim()
                .to_string()
        } else {
            String::new()
        };

        // Try to get app name from /proc
        let app_name = if pid > 0 {
            std::fs::read_to_string(format!("/proc/{}/comm", pid))
                .map(|s| s.trim().to_string())
                .unwrap_or_else(|_| "Unknown".to_string())
        } else {
            "Unknown".to_string()
        };

        Ok(WindowInfo {
            pid,
            window_id,
            app_name,
            title,
        })
    }

    /// Get active window using X11
    fn get_active_window_x11(&self) -> Result<WindowInfo> {
        let conn = self
            .conn
            .as_ref()
            .ok_or_else(|| anyhow!("No X11 connection"))?;

        // Get _NET_ACTIVE_WINDOW atom
        let active_atom = conn
            .intern_atom(false, b"_NET_ACTIVE_WINDOW")
            .context("Failed to intern _NET_ACTIVE_WINDOW")?
            .reply()
            .context("Failed to get _NET_ACTIVE_WINDOW reply")?
            .atom;

        // Get active window property
        let reply = conn
            .get_property(false, self.root, active_atom, AtomEnum::WINDOW, 0, 1)
            .context("Failed to get active window property")?
            .reply()
            .context("Failed to get active window reply")?;

        if reply.value.is_empty() {
            return Err(anyhow!("No active window"));
        }

        let window_id = u32::from_ne_bytes([
            reply.value[0],
            reply.value[1],
            reply.value[2],
            reply.value[3],
        ]);

        // Get _NET_WM_PID
        let pid_atom = conn
            .intern_atom(false, b"_NET_WM_PID")
            .context("Failed to intern _NET_WM_PID")?
            .reply()
            .context("Failed to get _NET_WM_PID reply")?
            .atom;

        let pid_reply = conn
            .get_property(false, window_id, pid_atom, AtomEnum::CARDINAL, 0, 1)
            .context("Failed to get PID property")?
            .reply()
            .context("Failed to get PID reply")?;

        let pid: u32 = if pid_reply.value.len() >= 4 {
            u32::from_ne_bytes([
                pid_reply.value[0],
                pid_reply.value[1],
                pid_reply.value[2],
                pid_reply.value[3],
            ])
        } else {
            0
        };

        // Get _NET_WM_NAME or WM_NAME
        let name_atom = conn
            .intern_atom(false, b"_NET_WM_NAME")
            .context("Failed to intern _NET_WM_NAME")?
            .reply()
            .context("Failed to get _NET_WM_NAME reply")?
            .atom;

        let utf8_atom = conn
            .intern_atom(false, b"UTF8_STRING")
            .context("Failed to intern UTF8_STRING")?
            .reply()
            .context("Failed to get UTF8_STRING reply")?
            .atom;

        let name_reply = conn
            .get_property(false, window_id, name_atom, utf8_atom, 0, 1024)
            .context("Failed to get window name")?
            .reply()
            .context("Failed to get window name reply")?;

        let title = String::from_utf8_lossy(&name_reply.value)
            .trim()
            .to_string();

        // Get app name from /proc
        let app_name = if pid > 0 {
            std::fs::read_to_string(format!("/proc/{}/comm", pid))
                .map(|s| s.trim().to_string())
                .unwrap_or_else(|_| "Unknown".to_string())
        } else {
            "Unknown".to_string()
        };

        Ok(WindowInfo {
            pid,
            window_id: format!("{}", window_id),
            app_name,
            title,
        })
    }

    /// Focus window using xdotool
    fn focus_window_xdotool(&self, window: &WindowInfo) -> Result<()> {
        let output = Command::new("xdotool")
            .args(["windowactivate", "--sync", &window.window_id])
            .output()
            .context("Failed to run xdotool windowactivate")?;

        if !output.status.success() {
            // Try wmctrl as fallback
            let wmctrl_output = Command::new("wmctrl")
                .args(["-i", "-a", &window.window_id])
                .output();

            if wmctrl_output.is_err() || !wmctrl_output.unwrap().status.success() {
                return Err(anyhow!("Failed to focus window"));
            }
        }

        Ok(())
    }

    /// Focus window using X11
    fn focus_window_x11(&self, window: &WindowInfo) -> Result<()> {
        let conn = self
            .conn
            .as_ref()
            .ok_or_else(|| anyhow!("No X11 connection"))?;

        let window_id: u32 = window.window_id.parse().context("Invalid window ID")?;

        // Get _NET_ACTIVE_WINDOW atom for the request
        let active_atom = conn
            .intern_atom(false, b"_NET_ACTIVE_WINDOW")
            .context("Failed to intern _NET_ACTIVE_WINDOW")?
            .reply()
            .context("Failed to get _NET_ACTIVE_WINDOW reply")?
            .atom;

        // Send _NET_ACTIVE_WINDOW client message
        let event = x11rb::protocol::xproto::ClientMessageEvent {
            response_type: x11rb::protocol::xproto::CLIENT_MESSAGE_EVENT,
            format: 32,
            sequence: 0,
            window: window_id,
            type_: active_atom,
            data: x11rb::protocol::xproto::ClientMessageData::from([
                1u32, // Source indication: normal application
                0,    // Timestamp (0 = current time)
                0,    // Currently active window (0 = none)
                0, 0,
            ]),
        };

        conn.send_event(
            false,
            self.root,
            x11rb::protocol::xproto::EventMask::SUBSTRUCTURE_REDIRECT
                | x11rb::protocol::xproto::EventMask::SUBSTRUCTURE_NOTIFY,
            event,
        )
        .context("Failed to send focus event")?;

        conn.flush().context("Failed to flush X11 connection")?;

        Ok(())
    }

    /// Find Cursor window
    fn find_cursor_window(&self) -> Option<WindowInfo> {
        // Try using wmctrl to list windows
        let output = Command::new("wmctrl").args(["-l", "-p"]).output().ok()?;

        if !output.status.success() {
            return None;
        }

        let output_str = String::from_utf8_lossy(&output.stdout);

        for line in output_str.lines() {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 5 {
                let window_id = parts[0];
                let pid: u32 = parts[2].parse().unwrap_or(0);

                // Check if this is Cursor
                let app_name = if pid > 0 {
                    std::fs::read_to_string(format!("/proc/{}/comm", pid))
                        .map(|s| s.trim().to_string())
                        .unwrap_or_default()
                } else {
                    String::new()
                };

                if app_name.to_lowercase().contains("cursor") {
                    return Some(WindowInfo {
                        pid,
                        window_id: window_id.to_string(),
                        app_name,
                        title: parts[4..].join(" "),
                    });
                }
            }
        }

        None
    }
}

impl WindowManager for LinuxWindowManager {
    fn get_active_window(&self) -> Result<WindowInfo> {
        if self.use_x11 {
            self.get_active_window_x11()
        } else {
            self.get_active_window_xdotool()
        }
    }

    fn focus_window(&self, window: &WindowInfo) -> Result<()> {
        if self.use_x11 {
            // Try X11 first, fall back to xdotool
            self.focus_window_x11(window)
                .or_else(|_| self.focus_window_xdotool(window))
        } else {
            self.focus_window_xdotool(window)
        }
    }

    fn focus_cursor(&self) -> Result<()> {
        if let Some(cursor_window) = self.find_cursor_window() {
            self.focus_window(&cursor_window)
        } else {
            // Try using xdotool to search for Cursor window
            let output = Command::new("xdotool")
                .args(["search", "--name", "Cursor"])
                .output()
                .context("Failed to search for Cursor window")?;

            if output.status.success() {
                let window_ids = String::from_utf8_lossy(&output.stdout);
                if let Some(first_id) = window_ids.lines().next() {
                    let window = WindowInfo {
                        pid: 0,
                        window_id: first_id.trim().to_string(),
                        app_name: "Cursor".to_string(),
                        title: String::new(),
                    };
                    return self.focus_window(&window);
                }
            }

            Err(anyhow!("Cursor window not found"))
        }
    }
}

impl Default for LinuxWindowManager {
    fn default() -> Self {
        Self::new()
    }
}
