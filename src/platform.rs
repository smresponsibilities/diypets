//! Cross-platform desktop helpers (Windows, macOS, Linux) with zero extra crates.

use crate::agent_api::AgentCommand;
use std::sync::mpsc::Sender;

/// Return the global screen cursor position `(x, y)` in desktop coordinates when supported.
#[cfg(windows)]
pub fn global_cursor_pos() -> Option<(f32, f32)> {
    use windows::Win32::Foundation::POINT;
    use windows::Win32::UI::WindowsAndMessaging::GetCursorPos;
    let mut pt = POINT::default();
    unsafe {
        if GetCursorPos(&mut pt).is_ok() {
            return Some((pt.x as f32, pt.y as f32));
        }
    }
    None
}

#[cfg(target_os = "macos")]
pub fn global_cursor_pos() -> Option<(f32, f32)> {
    #[repr(C)]
    struct CGPoint {
        x: f64,
        y: f64,
    }
    #[link(name = "CoreGraphics", kind = "framework")]
    unsafe extern "C" {
        fn CGEventCreate(source: *const std::ffi::c_void) -> *const std::ffi::c_void;
        fn CGEventGetLocation(event: *const std::ffi::c_void) -> CGPoint;
    }
    #[link(name = "CoreFoundation", kind = "framework")]
    unsafe extern "C" {
        fn CFRelease(cf: *const std::ffi::c_void);
    }
    unsafe {
        let ev = CGEventCreate(std::ptr::null());
        if ev.is_null() {
            return None;
        }
        let pt = CGEventGetLocation(ev);
        CFRelease(ev);
        Some((pt.x as f32, pt.y as f32))
    }
}

#[cfg(all(unix, not(target_os = "macos")))]
pub fn global_cursor_pos() -> Option<(f32, f32)> {
    None
}

/// Detect whether another non-desktop foreground window covers an entire monitor (fullscreen game/video).
#[cfg(windows)]
pub fn is_fullscreen_app_active(own_hwnds: &[isize], monitors: &[(i32, i32, i32, i32)]) -> bool {
    use windows::Win32::Foundation::RECT;
    use windows::Win32::Graphics::Dwm::{DWMWA_CLOAKED, DwmGetWindowAttribute};
    use windows::Win32::UI::WindowsAndMessaging::{
        GetClassNameW, GetForegroundWindow, GetWindowRect, IsIconic,
    };

    unsafe {
        let fg = GetForegroundWindow();
        if fg.is_invalid() || own_hwnds.contains(&(fg.0 as isize)) || IsIconic(fg).as_bool() {
            return false;
        }

        let mut cloaked = 0u32;
        if DwmGetWindowAttribute(
            fg,
            DWMWA_CLOAKED,
            &mut cloaked as *mut _ as _,
            std::mem::size_of_val(&cloaked) as u32,
        )
        .is_ok()
            && cloaked != 0
        {
            return false;
        }

        let mut class_buf = [0u16; 128];
        let len = GetClassNameW(fg, &mut class_buf) as usize;
        if len > 0 {
            let class_name = String::from_utf16_lossy(&class_buf[..len]);
            if matches!(
                class_name.as_str(),
                "Progman" | "WorkerW" | "Shell_TrayWnd" | "Shell_SecondaryTrayWnd"
            ) {
                return false;
            }
        }

        let mut rect = RECT::default();
        if GetWindowRect(fg, &mut rect).is_err() {
            return false;
        }

        for &(mx, my, mr, mb) in monitors {
            if rect.left <= mx + 2
                && rect.top <= my + 2
                && rect.right >= mr - 2
                && rect.bottom >= mb - 2
            {
                return true;
            }
        }
    }
    false
}

#[cfg(not(windows))]
pub fn is_fullscreen_app_active(_own_hwnds: &[isize], _monitors: &[(i32, i32, i32, i32)]) -> bool {
    false
}

/// Non-blocking cross-platform right-click pet menu for macOS (`osascript`) and Linux (`zenity`).
#[cfg(not(windows))]
pub fn spawn_unix_pet_menu(tx: Sender<AgentCommand>, follow_cursor: bool) {
    std::thread::spawn(move || {
        let follow_lbl = if follow_cursor {
            "Disable Follow Cursor"
        } else {
            "Enable Follow Cursor"
        };
        let choice: Option<String> = {
            #[cfg(target_os = "macos")]
            {
                let script = format!(
                    "choose from list {{\"Headpat / Hop\", \"Drop Bouncy Ball\", \"{follow_lbl}\", \"Sleep\", \"Clone Pet (+1)\", \"Application Settings\", \"DIYPets Studio\"}} with title \"DIYPets\" with prompt \"Pet Action:\""
                );
                std::process::Command::new("osascript")
                    .args(["-e", &script])
                    .output()
                    .ok()
                    .and_then(|o| {
                        let s = String::from_utf8_lossy(&o.stdout).trim().to_string();
                        if s.is_empty() || s == "false" { None } else { Some(s) }
                    })
            }
            #[cfg(all(unix, not(target_os = "macos")))]
            {
                std::process::Command::new("zenity")
                    .args([
                        "--list",
                        "--title=DIYPets",
                        "--column=Action",
                        "Headpat / Hop",
                        "Drop Bouncy Ball",
                        follow_lbl,
                        "Sleep",
                        "Clone Pet (+1)",
                        "Application Settings",
                        "DIYPets Studio",
                    ])
                    .output()
                    .ok()
                    .and_then(|o| {
                        let s = String::from_utf8_lossy(&o.stdout).trim().to_string();
                        if s.is_empty() { None } else { Some(s) }
                    })
            }
        };

        let mood = match choice.as_deref() {
            Some("Headpat / Hop") => ("happy", Some("<3")),
            Some("Drop Bouncy Ball") => ("ball", Some("Play time!")),
            Some(s) if s.contains("Follow Cursor") => ("follow_cursor", None),
            Some("Sleep") => ("sleep", None),
            Some("Clone Pet (+1)") => ("spawn", None),
            Some("Application Settings") => ("open_settings", None),
            Some("DIYPets Studio") => ("open_creator", None),
            None => ("ball", Some("Play time!")),
            _ => return,
        };

        let _ = tx.send(AgentCommand {
            mood: mood.0.to_string(),
            text: mood.1.map(String::from),
            pack_id: None,
            duration_ms: None,
            enabled: None,
        });
    });
}

#[cfg(windows)]
#[allow(dead_code)]
pub fn spawn_unix_pet_menu(_tx: Sender<AgentCommand>, _follow_cursor: bool) {}