//! Cross-platform "Start on Login" (Autostart) without heavy external crates.
//!
//! - Windows: `HKCU\Software\Microsoft\Windows\CurrentVersion\Run` via `reg.exe` (hidden window)
//! - macOS:   `~/Library/LaunchAgents/com.smresponsibilities.diypets.plist`
//! - Linux:   `~/.config/autostart/diypets.desktop`

use crate::settings::sibling_binary;
use std::path::PathBuf;

const APP_ID: &str = "DIYPets";

/// Always register the main `diypets` runner binary (even when toggled from `diypets-settings`).
pub fn runner_executable() -> Option<PathBuf> {
    if let Some(runner) = sibling_binary("diypets")
        && runner.is_file()
    {
        return Some(runner);
    }
    std::env::current_exe().ok()
}

#[cfg(windows)]
pub fn is_enabled() -> bool {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    std::process::Command::new("reg")
        .args([
            "query",
            r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run",
            "/v",
            APP_ID,
        ])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

#[cfg(windows)]
pub fn set_enabled(enable: bool) -> Result<(), String> {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let key = r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run";
    if enable {
        let exe = runner_executable().ok_or("cannot locate diypets executable")?;
        let quoted = format!("\"{}\"", exe.display());
        let out = std::process::Command::new("reg")
            .args(["add", key, "/v", APP_ID, "/t", "REG_SZ", "/d", &quoted, "/f"])
            .creation_flags(CREATE_NO_WINDOW)
            .output()
            .map_err(|e| format!("failed to run reg.exe: {e}"))?;
        if !out.status.success() {
            return Err(String::from_utf8_lossy(&out.stderr).trim().to_string());
        }
    } else {
        if let Err(e) = std::process::Command::new("reg")
            .args(["delete", key, "/v", APP_ID, "/f"])
            .creation_flags(CREATE_NO_WINDOW)
            .output() { crate::logging::warn(&format!("autostart: reg command failed: {e}")); }
    }
    Ok(())
}

#[cfg(target_os = "macos")]
fn plist_path() -> Option<PathBuf> {
    let home = std::env::var_os("HOME")?;
    Some(
        PathBuf::from(home)
            .join("Library")
            .join("LaunchAgents")
            .join("com.smresponsibilities.diypets.plist"),
    )
}

#[cfg(target_os = "macos")]
pub fn is_enabled() -> bool {
    plist_path().map(|p| p.is_file()).unwrap_or(false)
}

#[cfg(target_os = "macos")]
pub fn set_enabled(enable: bool) -> Result<(), String> {
    let path = plist_path().ok_or("HOME not set")?;
    if enable {
        let exe = runner_executable().ok_or("cannot locate diypets executable")?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("cannot create LaunchAgents dir: {e}"))?;
        }
        let plist = format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>com.smresponsibilities.diypets</string>
    <key>ProgramArguments</key>
    <array>
        <string>{}</string>
    </array>
    <key>RunAtLoad</key>
    <true/>
</dict>
</plist>
"#,
            exe.display()
        );
        std::fs::write(&path, plist).map_err(|e| format!("cannot write plist: {e}"))?;
    } else if path.is_file() {
        std::fs::remove_file(&path).map_err(|e| format!("cannot remove plist: {e}"))?;
    }
    Ok(())
}

#[cfg(all(unix, not(target_os = "macos")))]
fn desktop_entry_path() -> Option<PathBuf> {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))?;
    Some(base.join("autostart").join("diypets.desktop"))
}

#[cfg(all(unix, not(target_os = "macos")))]
pub fn is_enabled() -> bool {
    desktop_entry_path().map(|p| p.is_file()).unwrap_or(false)
}

#[cfg(all(unix, not(target_os = "macos")))]
pub fn set_enabled(enable: bool) -> Result<(), String> {
    let path = desktop_entry_path().ok_or("HOME/XDG_CONFIG_HOME not set")?;
    if enable {
        let exe = runner_executable().ok_or("cannot locate diypets executable")?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("cannot create autostart dir: {e}"))?;
        }
        let desktop = format!(
            "[Desktop Entry]\nType=Application\nName=DIYPets\nComment=Lightweight cross-platform desktop mascot engine\nExec=\"{}\"\nTerminal=false\nX-GNOME-Autostart-enabled=true\n",
            exe.display()
        );
        std::fs::write(&path, desktop).map_err(|e| format!("cannot write desktop entry: {e}"))?;
    } else if path.is_file() {
        std::fs::remove_file(&path).map_err(|e| format!("cannot remove desktop entry: {e}"))?;
    }
    Ok(())
}