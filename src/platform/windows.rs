//! Windows window management implementation
//!
//! Uses Win32 API for window management operations:
//! - GetForegroundWindow to get the active window
//! - SetForegroundWindow to focus windows
//! - GetWindowThreadProcessId to get process information

use super::{WindowInfo, WindowManager};
use anyhow::{anyhow, Result};
use std::ffi::OsString;
use std::os::windows::ffi::OsStringExt;
use windows_sys::Win32::Foundation::{BOOL, HWND, MAX_PATH};
use windows_sys::Win32::System::ProcessStatus::GetModuleBaseNameW;
use windows_sys::Win32::System::Threading::{
    OpenProcess, PROCESS_QUERY_INFORMATION, PROCESS_VM_READ,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GetForegroundWindow, GetWindowTextLengthW, GetWindowTextW,
    GetWindowThreadProcessId, IsWindowVisible, SetForegroundWindow, ShowWindow, SW_RESTORE,
};

/// Windows window manager implementation
pub struct WindowsWindowManager;

impl WindowsWindowManager {
    pub fn new() -> Self {
        Self
    }

    /// Get the process name for a given PID
    fn get_process_name(&self, pid: u32) -> Option<String> {
        unsafe {
            let handle = OpenProcess(PROCESS_QUERY_INFORMATION | PROCESS_VM_READ, 0, pid);
            if handle == 0 {
                return None;
            }

            let mut name_buf: [u16; MAX_PATH as usize] = [0; MAX_PATH as usize];
            let len = GetModuleBaseNameW(handle, 0, name_buf.as_mut_ptr(), MAX_PATH);

            if len == 0 {
                return None;
            }

            let name = OsString::from_wide(&name_buf[..len as usize]);
            name.to_str().map(|s| {
                // Remove .exe extension if present
                s.strip_suffix(".exe").unwrap_or(s).to_string()
            })
        }
    }

    /// Get window title for a given HWND
    fn get_window_title(&self, hwnd: HWND) -> String {
        unsafe {
            let len = GetWindowTextLengthW(hwnd);
            if len == 0 {
                return String::new();
            }

            let mut title_buf: Vec<u16> = vec![0; (len + 1) as usize];
            let actual_len = GetWindowTextW(hwnd, title_buf.as_mut_ptr(), len + 1);

            if actual_len == 0 {
                return String::new();
            }

            OsString::from_wide(&title_buf[..actual_len as usize])
                .to_string_lossy()
                .to_string()
        }
    }

    /// Find the main window for Cursor application
    fn find_cursor_window(&self) -> Option<HWND> {
        struct EnumData {
            cursor_hwnd: Option<HWND>,
        }

        unsafe extern "system" fn enum_callback(hwnd: HWND, lparam: isize) -> BOOL {
            let data = &mut *(lparam as *mut EnumData);

            if IsWindowVisible(hwnd) == 0 {
                return 1; // Continue enumeration
            }

            let mut pid: u32 = 0;
            GetWindowThreadProcessId(hwnd, &mut pid);

            // Get process name
            let handle = OpenProcess(PROCESS_QUERY_INFORMATION | PROCESS_VM_READ, 0, pid);
            if handle != 0 {
                let mut name_buf: [u16; MAX_PATH as usize] = [0; MAX_PATH as usize];
                let len = GetModuleBaseNameW(handle, 0, name_buf.as_mut_ptr(), MAX_PATH);

                if len > 0 {
                    let name = OsString::from_wide(&name_buf[..len as usize]);
                    if let Some(name_str) = name.to_str() {
                        if name_str.to_lowercase().contains("cursor") {
                            data.cursor_hwnd = Some(hwnd);
                            return 0; // Stop enumeration
                        }
                    }
                }
            }

            1 // Continue enumeration
        }

        let mut data = EnumData { cursor_hwnd: None };

        unsafe {
            EnumWindows(Some(enum_callback), &mut data as *mut _ as isize);
        }

        data.cursor_hwnd
    }
}

impl WindowsWindowManager {
    /// Find a Cursor window whose title contains the given search string
    fn find_cursor_window_by_title(&self, search: &str) -> Option<HWND> {
        struct EnumData {
            target_hwnd: Option<HWND>,
            search_lower: String,
        }

        unsafe extern "system" fn enum_callback(hwnd: HWND, lparam: isize) -> BOOL {
            let data = &mut *(lparam as *mut EnumData);

            if IsWindowVisible(hwnd) == 0 {
                return 1;
            }

            let mut pid: u32 = 0;
            GetWindowThreadProcessId(hwnd, &mut pid);

            let handle = OpenProcess(PROCESS_QUERY_INFORMATION | PROCESS_VM_READ, 0, pid);
            if handle != 0 {
                let mut name_buf: [u16; MAX_PATH as usize] = [0; MAX_PATH as usize];
                let len = GetModuleBaseNameW(handle, 0, name_buf.as_mut_ptr(), MAX_PATH);

                if len > 0 {
                    let name = OsString::from_wide(&name_buf[..len as usize]);
                    if let Some(name_str) = name.to_str() {
                        if name_str.to_lowercase().contains("cursor") {
                            // Check if this Cursor window's title contains our search string
                            let title_len = GetWindowTextLengthW(hwnd);
                            if title_len > 0 {
                                let mut title_buf: Vec<u16> = vec![0; (title_len + 1) as usize];
                                let actual_len =
                                    GetWindowTextW(hwnd, title_buf.as_mut_ptr(), title_len + 1);
                                if actual_len > 0 {
                                    let title = OsString::from_wide(
                                        &title_buf[..actual_len as usize],
                                    )
                                    .to_string_lossy()
                                    .to_lowercase();
                                    if title.contains(&data.search_lower) {
                                        data.target_hwnd = Some(hwnd);
                                        return 0; // Stop enumeration
                                    }
                                }
                            }
                        }
                    }
                }
            }

            1 // Continue enumeration
        }

        let mut data = EnumData {
            target_hwnd: None,
            search_lower: search.to_lowercase(),
        };

        unsafe {
            EnumWindows(Some(enum_callback), &mut data as *mut _ as isize);
        }

        data.target_hwnd
    }
}

impl WindowManager for WindowsWindowManager {
    fn get_active_window(&self) -> Result<WindowInfo> {
        unsafe {
            let hwnd = GetForegroundWindow();
            if hwnd == 0 {
                return Err(anyhow!("No foreground window found"));
            }

            let mut pid: u32 = 0;
            GetWindowThreadProcessId(hwnd, &mut pid);

            let app_name = self
                .get_process_name(pid)
                .unwrap_or_else(|| "Unknown".to_string());

            let title = self.get_window_title(hwnd);

            Ok(WindowInfo {
                pid,
                window_id: format!("{}", hwnd),
                app_name,
                title,
            })
        }
    }

    fn focus_window(&self, window: &WindowInfo) -> Result<()> {
        let hwnd: HWND = window
            .window_id
            .parse()
            .map_err(|_| anyhow!("Invalid window ID"))?;

        unsafe {
            // Restore window if minimized
            ShowWindow(hwnd, SW_RESTORE);

            // Bring to foreground
            if SetForegroundWindow(hwnd) == 0 {
                return Err(anyhow!("Failed to set foreground window"));
            }
        }

        Ok(())
    }

    fn focus_cursor(&self) -> Result<()> {
        let hwnd = self
            .find_cursor_window()
            .ok_or_else(|| anyhow!("Cursor window not found"))?;

        unsafe {
            ShowWindow(hwnd, SW_RESTORE);

            if SetForegroundWindow(hwnd) == 0 {
                return Err(anyhow!("Failed to focus Cursor window"));
            }
        }

        Ok(())
    }

    fn focus_cursor_window(&self, window: &WindowInfo) -> Result<()> {
        // Strategy 1: Try direct HWND focus (works if the window handle is still valid)
        if self.focus_window(window).is_ok() {
            return Ok(());
        }

        // Strategy 2: Search for Cursor window by project name
        if let Some(project_name) = window.cursor_project_name() {
            if let Some(hwnd) = self.find_cursor_window_by_title(&project_name) {
                unsafe {
                    ShowWindow(hwnd, SW_RESTORE);
                    if SetForegroundWindow(hwnd) != 0 {
                        return Ok(());
                    }
                }
            }
        }

        // Strategy 3: Search by full title
        if !window.title.is_empty() {
            if let Some(hwnd) = self.find_cursor_window_by_title(&window.title) {
                unsafe {
                    ShowWindow(hwnd, SW_RESTORE);
                    if SetForegroundWindow(hwnd) != 0 {
                        return Ok(());
                    }
                }
            }
        }

        // Strategy 4: Generic Cursor focus
        self.focus_cursor()
    }
}

impl Default for WindowsWindowManager {
    fn default() -> Self {
        Self::new()
    }
}
