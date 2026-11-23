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
/// Desktop window bounds in logical/physical screen coordinates.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WindowRect {
    pub id: isize,
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

/// Select the topmost non-fullscreen window whose rectangle contains `(x, y)`.
pub fn pick_window_under_point(
    windows: &[WindowRect],
    x: i32,
    y: i32,
    monitors: &[(i32, i32, i32, i32)],
) -> Option<WindowRect> {
    for &w in windows {
        if x < w.left || x > w.right || y < w.top || y > w.bottom {
            continue;
        }
        let is_fullscreen = monitors
            .iter()
            .any(|&(mx, my, mr, mb)| w.left <= mx && w.top <= my && w.right >= mr && w.bottom >= mb);
        if !is_fullscreen {
            return Some(w);
        }
    }
    None
}

#[cfg(not(windows))]
static UNIX_WINDOWS: std::sync::OnceLock<std::sync::Mutex<Vec<WindowRect>>> =
    std::sync::OnceLock::new();

#[cfg(not(windows))]
fn unix_windows_store() -> &'static std::sync::Mutex<Vec<WindowRect>> {
    UNIX_WINDOWS.get_or_init(|| std::sync::Mutex::new(Vec::new()))
}

#[cfg(target_os = "macos")]
fn poll_unix_windows_once() -> Vec<WindowRect> {
    let script = r#"ObjC.import('CoreGraphics');
var list = ObjC.deepUnwrap($.CGWindowListCopyWindowInfo($.kCGWindowListOptionOnScreenOnly | $.kCGWindowListExcludeDesktopElements, 0)) || [];
var out = [];
for (var i = 0; i < list.length; i++) {
  var w = list[i];
  if (w.kCGWindowLayer !== 0) continue;
  var owner = String(w.kCGWindowOwnerName || '').toLowerCase();
  if (owner.indexOf('diypets') >= 0) continue;
  var b = w.kCGWindowBounds;
  if (!b || b.Width < 80 || b.Height < 40) continue;
  out.push([w.kCGWindowNumber || i, Math.round(b.X), Math.round(b.Y), Math.round(b.X + b.Width), Math.round(b.Y + b.Height)].join(','));
}
out.join('\n');"#;
    let Ok(out) = std::process::Command::new("osascript")
        .args(["-l", "JavaScript", "-e", script])
        .output()
    else {
        return Vec::new();
    };
    let text = String::from_utf8_lossy(&out.stdout);
    text.lines()
        .filter_map(|line| {
            let mut p = line.trim().split(',');
            let id = p.next()?.parse::<isize>().ok()?;
            let left = p.next()?.parse::<i32>().ok()?;
            let top = p.next()?.parse::<i32>().ok()?;
            let right = p.next()?.parse::<i32>().ok()?;
            let bottom = p.next()?.parse::<i32>().ok()?;
            Some(WindowRect { id, left, top, right, bottom })
        })
        .collect()
}

#[cfg(all(unix, not(target_os = "macos")))]
fn poll_unix_windows_once() -> Vec<WindowRect> {
    let Ok(out) = std::process::Command::new("wmctrl").args(["-lG"]).output() else {
        return Vec::new();
    };
    let text = String::from_utf8_lossy(&out.stdout);
    text.lines()
        .filter_map(|line| {
            if line.to_lowercase().contains("diypets") {
                return None;
            }
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() < 6 {
                return None;
            }
            let id = isize::from_str_radix(parts[0].trim_start_matches("0x"), 16).ok()?;
            let left = parts[2].parse::<i32>().ok()?;
            let top = parts[3].parse::<i32>().ok()?;
            let w = parts[4].parse::<i32>().ok()?;
            let h = parts[5].parse::<i32>().ok()?;
            if w < 80 || h < 40 {
                return None;
            }
            Some(WindowRect { id, left, top, right: left + w, bottom: top + h })
        })
        .collect()
}

/// Start a lightweight background thread (300ms) on macOS/Linux to cache desktop window ledges.
#[cfg(not(windows))]
pub fn start_unix_window_poller() {
    let _ = unix_windows_store();
    std::thread::spawn(|| loop {
        let rects = poll_unix_windows_once();
        if let Ok(mut guard) = unix_windows_store().lock() {
            *guard = rects;
        }
        std::thread::sleep(std::time::Duration::from_millis(300));
    });
}

#[cfg(windows)]
pub fn start_unix_window_poller() {}

#[cfg(not(windows))]
pub fn find_unix_window_under_point(
    x: i32,
    y: i32,
    monitors: &[(i32, i32, i32, i32)],
) -> Option<WindowRect> {
    let guard = unix_windows_store().lock().ok()?;
    pick_window_under_point(&guard, x, y, monitors)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pick_window_under_point_skips_fullscreen_and_selects_topmost() {
        let monitors = [(0, 0, 1920, 1080)];
        let windows = [
            WindowRect { id: 1, left: 0, top: 0, right: 1920, bottom: 1080 },
            WindowRect { id: 2, left: 200, top: 300, right: 900, bottom: 700 },
            WindowRect { id: 3, left: 100, top: 250, right: 950, bottom: 800 },
        ];
        let hit = pick_window_under_point(&windows, 400, 302, &monitors).unwrap();
        assert_eq!(hit.id, 2);
        assert_eq!(pick_window_under_point(&windows, 50, 50, &monitors), None);
    }
}