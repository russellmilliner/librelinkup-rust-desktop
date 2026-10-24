//! Windows platform integration: DPAPI-protected master key, login autostart (HKCU Run),
//! single-instance guard, toast-notification identity (AppUserModelID), console attach
//! for `--version`, and native window show/hide/flash helpers.
//!
//! Non-Windows builds get inert stubs so the crate still type-checks on Linux.

use std::path::PathBuf;

/// Matches the Linux app id and the AppUserModelID set on the installer's Start Menu shortcut.
pub const APP_USER_MODEL_ID: &str = "io.github.russellmilliner.LibreLinkUpDesktop";
pub const APP_DISPLAY_NAME: &str = "LibreLinkUp Rust Desktop";
pub const MAIN_WINDOW_TITLE: &str = "LibreLinkUp Rust Desktop";
const RUN_VALUE_NAME: &str = "LibreLinkUpDesktop";
const SINGLE_INSTANCE_MUTEX: &str = "Local\\io.github.russellmilliner.LibreLinkUpDesktop.SingleInstance";

#[cfg(windows)]
fn wide(s: &str) -> Vec<u16> {
    use std::os::windows::ffi::OsStrExt;
    std::ffi::OsStr::new(s).encode_wide().chain(std::iter::once(0)).collect()
}

/// File holding the DPAPI-protected random master key used to encrypt credentials.
pub fn dpapi_master_key_path() -> PathBuf {
    librelinkup_core::paths::config_dir().join("master.dpapi")
}

/// Returns the credential-encryption master secret, creating and DPAPI-protecting a new
/// random key on first use. DPAPI binds the key to the current Windows user account, so the
/// vault cannot be decrypted by other users or after copying it to another machine.
pub fn load_or_create_master_secret() -> Result<Vec<u8>, String> {
    let path = dpapi_master_key_path();
    if let Ok(blob) = std::fs::read(&path) {
        return dpapi_unprotect(&blob);
    }
    let key = librelinkup_core::crypto::generate_random_key()?;
    let blob = dpapi_protect(&key)?;
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    std::fs::write(&path, blob).map_err(|e| format!("Failed to write master key: {e}"))?;
    Ok(key.to_vec())
}

#[cfg(windows)]
pub fn dpapi_protect(data: &[u8]) -> Result<Vec<u8>, String> {
    use windows_sys::Win32::Security::Cryptography::{
        CryptProtectData, CRYPTPROTECT_UI_FORBIDDEN, CRYPT_INTEGER_BLOB,
    };
    let input = CRYPT_INTEGER_BLOB { cbData: data.len() as u32, pbData: data.as_ptr() as *mut u8 };
    let mut output = CRYPT_INTEGER_BLOB { cbData: 0, pbData: std::ptr::null_mut() };
    let description = wide("LibreLinkUp credential key");
    let ok = unsafe {
        CryptProtectData(
            &input,
            description.as_ptr(),
            std::ptr::null(),
            std::ptr::null(),
            std::ptr::null(),
            CRYPTPROTECT_UI_FORBIDDEN,
            &mut output,
        )
    };
    if ok == 0 {
        return Err("CryptProtectData failed".into());
    }
    Ok(take_blob(output))
}

#[cfg(windows)]
pub fn dpapi_unprotect(data: &[u8]) -> Result<Vec<u8>, String> {
    use windows_sys::Win32::Security::Cryptography::{
        CryptUnprotectData, CRYPTPROTECT_UI_FORBIDDEN, CRYPT_INTEGER_BLOB,
    };
    let input = CRYPT_INTEGER_BLOB { cbData: data.len() as u32, pbData: data.as_ptr() as *mut u8 };
    let mut output = CRYPT_INTEGER_BLOB { cbData: 0, pbData: std::ptr::null_mut() };
    let ok = unsafe {
        CryptUnprotectData(
            &input,
            std::ptr::null_mut(),
            std::ptr::null(),
            std::ptr::null(),
            std::ptr::null(),
            CRYPTPROTECT_UI_FORBIDDEN,
            &mut output,
        )
    };
    if ok == 0 {
        return Err("CryptUnprotectData failed (vault belongs to another user or is corrupt)".into());
    }
    Ok(take_blob(output))
}

#[cfg(windows)]
fn take_blob(blob: windows_sys::Win32::Security::Cryptography::CRYPT_INTEGER_BLOB) -> Vec<u8> {
    use windows_sys::Win32::Foundation::LocalFree;
    let out = unsafe { std::slice::from_raw_parts(blob.pbData, blob.cbData as usize) }.to_vec();
    unsafe { LocalFree(blob.pbData as _) };
    out
}

#[cfg(not(windows))]
pub fn dpapi_protect(data: &[u8]) -> Result<Vec<u8>, String> {
    Ok(data.to_vec())
}

#[cfg(not(windows))]
pub fn dpapi_unprotect(data: &[u8]) -> Result<Vec<u8>, String> {
    Ok(data.to_vec())
}

// ----------------------------------------------------------------------------------------
// Autostart (HKCU\Software\Microsoft\Windows\CurrentVersion\Run)
// ----------------------------------------------------------------------------------------

#[cfg(windows)]
const RUN_KEY: &str = "Software\\Microsoft\\Windows\\CurrentVersion\\Run";

#[cfg(windows)]
pub fn set_autostart(enabled: bool, start_minimized: bool) -> Result<(), String> {
    use windows_sys::Win32::System::Registry::{
        RegDeleteKeyValueW, RegSetKeyValueW, HKEY_CURRENT_USER, REG_SZ,
    };
    let key = wide(RUN_KEY);
    let name = wide(RUN_VALUE_NAME);
    if !enabled {
        let rc = unsafe { RegDeleteKeyValueW(HKEY_CURRENT_USER, key.as_ptr(), name.as_ptr()) };
        // ERROR_FILE_NOT_FOUND (2) just means it was already disabled.
        return if rc == 0 || rc == 2 { Ok(()) } else { Err(format!("RegDeleteKeyValueW failed ({rc})")) };
    }
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let mut command = format!("\"{}\"", exe.display());
    if start_minimized {
        command.push_str(" --minimized");
    }
    let data = wide(&command);
    let rc = unsafe {
        RegSetKeyValueW(
            HKEY_CURRENT_USER,
            key.as_ptr(),
            name.as_ptr(),
            REG_SZ,
            data.as_ptr() as *const _,
            (data.len() * 2) as u32,
        )
    };
    if rc == 0 { Ok(()) } else { Err(format!("RegSetKeyValueW failed ({rc})")) }
}

#[cfg(windows)]
pub fn is_autostart_registered() -> bool {
    use windows_sys::Win32::System::Registry::{RegGetValueW, HKEY_CURRENT_USER, RRF_RT_REG_SZ};
    let key = wide(RUN_KEY);
    let name = wide(RUN_VALUE_NAME);
    let mut size: u32 = 0;
    let rc = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            key.as_ptr(),
            name.as_ptr(),
            RRF_RT_REG_SZ,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            &mut size,
        )
    };
    rc == 0
}

#[cfg(not(windows))]
pub fn set_autostart(_enabled: bool, _start_minimized: bool) -> Result<(), String> {
    Ok(())
}

#[cfg(not(windows))]
pub fn is_autostart_registered() -> bool {
    false
}

// ----------------------------------------------------------------------------------------
// Toast notification identity
// ----------------------------------------------------------------------------------------

/// Registers the AppUserModelID for this process and under
/// `HKCU\Software\Classes\AppUserModelId\<id>` (DisplayName + IconUri) so Windows toast
/// notifications show "LibreLinkUp Rust Desktop" with our icon, even for the portable exe.
#[cfg(windows)]
pub fn register_notification_identity(icon_png: &[u8]) {
    use windows_sys::Win32::System::Registry::{RegSetKeyValueW, HKEY_CURRENT_USER, REG_SZ};
    use windows_sys::Win32::UI::Shell::SetCurrentProcessExplicitAppUserModelID;

    let id = wide(APP_USER_MODEL_ID);
    unsafe { SetCurrentProcessExplicitAppUserModelID(id.as_ptr()) };

    let icon_path = librelinkup_core::paths::data_dir().join("notification-icon.png");
    let _ = std::fs::write(&icon_path, icon_png);

    let key = wide(&format!("Software\\Classes\\AppUserModelId\\{APP_USER_MODEL_ID}"));
    for (name, value) in [
        ("DisplayName", APP_DISPLAY_NAME.to_string()),
        ("IconUri", icon_path.display().to_string()),
    ] {
        let n = wide(name);
        let v = wide(&value);
        unsafe {
            RegSetKeyValueW(
                HKEY_CURRENT_USER,
                key.as_ptr(),
                n.as_ptr(),
                REG_SZ,
                v.as_ptr() as *const _,
                (v.len() * 2) as u32,
            );
        }
    }
}

#[cfg(not(windows))]
pub fn register_notification_identity(_icon_png: &[u8]) {}

// ----------------------------------------------------------------------------------------
// Single instance
// ----------------------------------------------------------------------------------------

/// Holds the named mutex for the lifetime of the process.
pub struct SingleInstance {
    #[cfg(windows)]
    _handle: windows_sys::Win32::Foundation::HANDLE,
}

/// Returns `None` if another instance is already running (and brings it to the front).
#[cfg(windows)]
pub fn acquire_single_instance() -> Option<SingleInstance> {
    use windows_sys::Win32::Foundation::{GetLastError, ERROR_ALREADY_EXISTS};
    use windows_sys::Win32::System::Threading::CreateMutexW;
    let name = wide(SINGLE_INSTANCE_MUTEX);
    let handle = unsafe { CreateMutexW(std::ptr::null(), 0, name.as_ptr()) };
    if handle.is_null() {
        return Some(SingleInstance { _handle: handle });
    }
    if unsafe { GetLastError() } == ERROR_ALREADY_EXISTS {
        activate_existing_instance();
        return None;
    }
    Some(SingleInstance { _handle: handle })
}

#[cfg(not(windows))]
pub fn acquire_single_instance() -> Option<SingleInstance> {
    Some(SingleInstance {})
}

#[cfg(windows)]
fn activate_existing_instance() {
    use windows_sys::Win32::UI::WindowsAndMessaging::FindWindowW;
    let title = wide(MAIN_WINDOW_TITLE);
    let hwnd = unsafe { FindWindowW(std::ptr::null(), title.as_ptr()) };
    if !hwnd.is_null() {
        show_window(hwnd as isize, true);
    }
}

// ----------------------------------------------------------------------------------------
// Native window helpers (HWND stored as isize so it can cross threads)
// ----------------------------------------------------------------------------------------

#[cfg(windows)]
pub fn show_window(hwnd: isize, visible: bool) {
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        SetForegroundWindow, ShowWindow, SW_HIDE, SW_RESTORE, SW_SHOW,
    };
    if hwnd == 0 {
        return;
    }
    let h = hwnd as windows_sys::Win32::Foundation::HWND;
    unsafe {
        if visible {
            ShowWindow(h, SW_SHOW);
            ShowWindow(h, SW_RESTORE);
            SetForegroundWindow(h);
        } else {
            ShowWindow(h, SW_HIDE);
        }
    }
}

#[cfg(not(windows))]
pub fn show_window(_hwnd: isize, _visible: bool) {}

/// Flashes the taskbar button / window caption until the window is focused.
#[cfg(windows)]
pub fn flash_window(hwnd: isize) {
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        FlashWindowEx, FLASHWINFO, FLASHW_ALL, FLASHW_TIMERNOFG,
    };
    if hwnd == 0 {
        return;
    }
    let info = FLASHWINFO {
        cbSize: std::mem::size_of::<FLASHWINFO>() as u32,
        hwnd: hwnd as windows_sys::Win32::Foundation::HWND,
        dwFlags: FLASHW_ALL | FLASHW_TIMERNOFG,
        uCount: 0,
        dwTimeout: 0,
    };
    unsafe { FlashWindowEx(&info) };
}

#[cfg(not(windows))]
pub fn flash_window(_hwnd: isize) {}

/// Configures whether the `"LibreLinkUp Overlay"` window appears on all Windows 10/11
/// Virtual Desktops (`WS_EX_TOOLWINDOW` set) or only the current Virtual Desktop.
#[cfg(windows)]
pub fn set_overlay_all_desktops(on_all_desktops: bool) {
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        FindWindowW, GetWindowLongPtrW, SetWindowLongPtrW, SetWindowPos, GWL_EXSTYLE,
        SWP_FRAMECHANGED, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER,
        WS_EX_TOOLWINDOW,
    };
    let title = wide("LibreLinkUp Overlay");
    let hwnd = unsafe { FindWindowW(std::ptr::null(), title.as_ptr()) };
    if hwnd.is_null() {
        return;
    }
    unsafe {
        let cur = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
        let want = if on_all_desktops {
            cur | (WS_EX_TOOLWINDOW as isize)
        } else {
            cur & !(WS_EX_TOOLWINDOW as isize)
        };
        if want != cur {
            SetWindowLongPtrW(hwnd, GWL_EXSTYLE, want);
            SetWindowPos(
                hwnd,
                std::ptr::null_mut(),
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE | SWP_FRAMECHANGED,
            );
        }
    }
}

#[cfg(not(windows))]
pub fn set_overlay_all_desktops(_on_all_desktops: bool) {}

// ----------------------------------------------------------------------------------------
// Console output for --version / --help (GUI-subsystem exe has no console by default)
// ----------------------------------------------------------------------------------------

#[cfg(windows)]
pub fn attach_parent_console() {
    use windows_sys::Win32::System::Console::{AttachConsole, ATTACH_PARENT_PROCESS};
    unsafe { AttachConsole(ATTACH_PARENT_PROCESS) };
}

#[cfg(not(windows))]
pub fn attach_parent_console() {}
