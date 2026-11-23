#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

#[cfg(windows)]
use std::collections::HashMap;
#[cfg(not(windows))]
use std::num::NonZeroU32;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::{Arc, Mutex, mpsc};
use std::time::{Duration, Instant};

use diypets::agent_api::{self, AgentCommand, SharedHerd};
use diypets::autostart;
use diypets::bubble::{render_pixel_ball_argb, render_speech_bubble_argb};
use diypets::catalog::{default_app_icon, make_square_app_icon};
use diypets::deskpet_state::{
    DeskpetState, DeskpetStateId, HerdEncounter, detect_herd_encounter, select_window_floor,
    window_floor_candidate,
};
use diypets::logging;
use diypets::platform;
use diypets::squash::{SquashSpring, apply_squash_nearest};
use diypets::deskpet_xml::DeskpetPack;
use diypets::settings::{
    MAX_ACTIVE_PETS, Settings, find_pack_icon, find_pack_xml, pack_search_dirs,
    resolve_project_root, sibling_binary,
};
#[cfg(not(windows))]
use softbuffer::{Context, Surface};
use tray_icon::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem, Submenu};
use tray_icon::{Icon as TrayIconImage, TrayIcon, TrayIconBuilder};
use winit::application::ApplicationHandler;
use winit::dpi::PhysicalPosition;
use winit::event::{ElementState, MouseButton, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::{Window, WindowId, WindowLevel};

const DEFAULT_XML_DATA: &str = include_str!("../../assets/highland_cow/deskpet.xml");
const FRAME_INTERVAL: Duration = Duration::from_millis(16);
const SETTINGS_POLL_MS: u64 = 500;
const WINDOW_SCAN_MS: u64 = 80;

fn scaled_height(tile_w: u32, tile_h: u32, pet_size: u32) -> u32 {
    if tile_w == 0 {
        return pet_size.max(1);
    }
    ((tile_h as f32 * (pet_size as f32 / tile_w as f32)).round() as u32).max(1)
}

#[cfg(windows)]
fn scale_nearest_into(
    src: &[u32],
    src_w: u32,
    src_h: u32,
    dst_w: u32,
    dst_h: u32,
    dst: &mut Vec<u32>,
) {
    let needed = (dst_w * dst_h) as usize;
    if dst.len() != needed {
        dst.resize(needed, 0);
    }
    if src_w == 0 || src_h == 0 || dst_w == 0 || dst_h == 0 {
        return;
    }
    for y in 0..dst_h {
        let src_row = ((y * src_h / dst_h) * src_w) as usize;
        let dst_row = (y * dst_w) as usize;
        for x in 0..dst_w {
            let src_idx = src_row + (x * src_w / dst_w) as usize;
            dst[dst_row + x as usize] = src[src_idx];
        }
    }
}

#[cfg(windows)]
#[allow(dead_code)]
fn scale_nearest(src: &[u32], src_w: u32, src_h: u32, dst_w: u32, dst_h: u32) -> Vec<u32> {
    let mut dst = Vec::new();
    scale_nearest_into(src, src_w, src_h, dst_w, dst_h, &mut dst);
    dst
}

#[cfg(all(test, windows))]
mod scaling_tests {
    use super::{scale_nearest, scaled_height};

    #[test]
    fn nearest_scaling_keeps_opaque_and_transparent_edges_crisp() {
        assert_eq!(
            scale_nearest(&[0xffff8000, 0], 2, 1, 4, 2),
            vec![
                0xffff8000, 0xffff8000, 0, 0,
                0xffff8000, 0xffff8000, 0, 0
            ]
        );
    }

    #[test]
    fn scaled_height_preserves_aspect_ratio() {
        assert_eq!(scaled_height(40, 40, 128), 128);
        assert_eq!(scaled_height(64, 32, 128), 64);
    }
}

fn load_pack(pack_id: &str) -> DeskpetPack {
    if let Some(xml_path) = find_pack_xml(pack_id)
        && let Ok(xml_data) = std::fs::read_to_string(&xml_path)
            && let Ok(pack) = DeskpetPack::parse(&xml_data) {
                return pack;
            }
    DeskpetPack::parse(DEFAULT_XML_DATA).unwrap()
}

fn pet_icon(id: &str, pack: &DeskpetPack) -> Option<image::RgbaImage> {
    if let Some(icon_path) = find_pack_icon(id)
        && let Ok(img) = image::open(&icon_path) {
            return Some(make_square_app_icon(&img.to_rgba8(), 64));
        }
    if let Some(first) = pack.frames.first() {
        let rgba: Vec<u8> = first
            .iter()
            .flat_map(|pixel| {
                let [a, r, g, b] = pixel.to_be_bytes();
                let straight =
                    |channel: u8| (u16::from(channel) * 255 / u16::from(a.max(1))).min(255) as u8;
                [straight(r), straight(g), straight(b), a]
            })
            .collect();
        if let Some(raw) = image::RgbaImage::from_raw(pack.tile_w, pack.tile_h, rgba) {
            return Some(make_square_app_icon(&raw, 64));
        }
    }
    default_app_icon(64)
}

#[cfg(windows)]
fn hwnd_of(window: &Window) -> Option<windows::Win32::Foundation::HWND> {
    use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
    let handle = window.window_handle().ok()?;
    match handle.as_raw() {
        RawWindowHandle::Win32(h) => Some(windows::Win32::Foundation::HWND(h.hwnd.get() as _)),
        _ => None,
    }
}

#[cfg(windows)]
fn apply_topmost(window: &Window, topmost: bool) {
    use windows::Win32::UI::WindowsAndMessaging::{
        HWND_NOTOPMOST, HWND_TOPMOST, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SetWindowPos,
    };
    if let Some(hwnd) = hwnd_of(window) {
        unsafe {
            let insert_after = if topmost { HWND_TOPMOST } else { HWND_NOTOPMOST };
            let _ = SetWindowPos(
                hwnd,
                insert_after,
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
            );
        }
    }
}

#[cfg(windows)]
struct LayeredSurface {
    hdc_mem: windows::Win32::Graphics::Gdi::HDC,
    hbmp: windows::Win32::Graphics::Gdi::HBITMAP,
    hbmp_old: windows::Win32::Graphics::Gdi::HGDIOBJ,
    bits: *mut u32,
    width: u32,
    height: u32,
}

#[cfg(windows)]
impl LayeredSurface {
    fn new(width: u32, height: u32) -> Option<Self> {
        use windows::Win32::Graphics::Gdi::{
            BI_RGB, BITMAPINFO, BITMAPINFOHEADER, CreateCompatibleDC, CreateDIBSection,
            DIB_RGB_COLORS, DeleteDC, SelectObject,
        };
        unsafe {
            let hdc_mem = CreateCompatibleDC(None);
            if hdc_mem.is_invalid() {
                return None;
            }
            let bmi = BITMAPINFO {
                bmiHeader: BITMAPINFOHEADER {
                    biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                    biWidth: width as i32,
                    biHeight: -(height as i32),
                    biPlanes: 1,
                    biBitCount: 32,
                    biCompression: BI_RGB.0,
                    ..Default::default()
                },
                ..Default::default()
            };
            let mut bits: *mut std::ffi::c_void = std::ptr::null_mut();
            let Ok(hbmp) = CreateDIBSection(None, &bmi, DIB_RGB_COLORS, &mut bits, None, 0) else {
                let _ = DeleteDC(hdc_mem);
                return None;
            };
            if bits.is_null() {
                let _ = windows::Win32::Graphics::Gdi::DeleteObject(hbmp);
                let _ = DeleteDC(hdc_mem);
                return None;
            }
            let hbmp_old = SelectObject(hdc_mem, hbmp);
            Some(Self {
                hdc_mem,
                hbmp,
                hbmp_old,
                bits: bits as *mut u32,
                width,
                height,
            })
        }
    }

    fn present(&mut self, window: &Window, buffer: &[u32], x: i32, y: i32) {
        use windows::Win32::Foundation::{COLORREF, POINT, SIZE};
        use windows::Win32::Graphics::Gdi::{AC_SRC_ALPHA, AC_SRC_OVER, BLENDFUNCTION};
        use windows::Win32::UI::WindowsAndMessaging::{ULW_ALPHA, UpdateLayeredWindow};

        let Some(hwnd) = hwnd_of(window) else { return };
        let pixel_count = (self.width * self.height) as usize;
        if buffer.len() < pixel_count {
            return;
        }
        unsafe {
            std::ptr::copy_nonoverlapping(buffer.as_ptr(), self.bits, pixel_count);
            let pt_src = POINT { x: 0, y: 0 };
            let pt_dst = POINT { x, y };
            let size = SIZE {
                cx: self.width as i32,
                cy: self.height as i32,
            };
            let blend = BLENDFUNCTION {
                BlendOp: AC_SRC_OVER as u8,
                BlendFlags: 0,
                SourceConstantAlpha: 255,
                AlphaFormat: AC_SRC_ALPHA as u8,
            };
            let res = UpdateLayeredWindow(
                hwnd,
                None,
                Some(&pt_dst as *const _),
                Some(&size as *const _),
                self.hdc_mem,
                Some(&pt_src as *const _),
                COLORREF(0),
                Some(&blend as *const _),
                ULW_ALPHA,
            );
            static ULW_OK_LOGGED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
            static ULW_ERR_COUNT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
            match res {
                Ok(()) => {
                    if !ULW_OK_LOGGED.swap(true, std::sync::atomic::Ordering::Relaxed) {
                        let nz = buffer[..pixel_count].iter().filter(|&&px| (px >> 24) > 0).count();
                        logging::info(&format!(
                            "UpdateLayeredWindow OK: hwnd={:?} pos=({x},{y}) size={}x{} opaque_px={nz}",
                            hwnd.0, self.width, self.height
                        ));
                    }
                }
                Err(e) => {
                    // Self-heal if WS_EX_LAYERED was cleared or conflicted with SetLayeredWindowAttributes
                    use windows::Win32::UI::WindowsAndMessaging::{
                        GWL_EXSTYLE, GetWindowLongW, SetWindowLongW, WS_EX_LAYERED,
                    };
                    let ex = GetWindowLongW(hwnd, GWL_EXSTYLE);
                    SetWindowLongW(hwnd, GWL_EXSTYLE, ex & !(WS_EX_LAYERED.0 as i32));
                    SetWindowLongW(hwnd, GWL_EXSTYLE, ex | (WS_EX_LAYERED.0 as i32));
                    let retry = UpdateLayeredWindow(
                        hwnd,
                        None,
                        Some(&pt_dst as *const _),
                        Some(&size as *const _),
                        self.hdc_mem,
                        Some(&pt_src as *const _),
                        COLORREF(0),
                        Some(&blend as *const _),
                        ULW_ALPHA,
                    );
                    if retry.is_ok() {
                        logging::warn(&format!(
                            "UpdateLayeredWindow recovered after resetting WS_EX_LAYERED on hwnd={:?}",
                            hwnd.0
                        ));
                    } else if ULW_ERR_COUNT.fetch_add(1, std::sync::atomic::Ordering::Relaxed) < 10 {
                        logging::error(&format!(
                            "UpdateLayeredWindow FAILED (after retry): hwnd={:?} pos=({x},{y}) size={}x{} err={e:?}",
                            hwnd.0, self.width, self.height
                        ));
                    }
                }
            }
        }
    }
}

#[cfg(windows)]
impl Drop for LayeredSurface {
    fn drop(&mut self) {
        use windows::Win32::Graphics::Gdi::{DeleteDC, DeleteObject, SelectObject};
        unsafe {
            SelectObject(self.hdc_mem, self.hbmp_old);
            let _ = DeleteObject(self.hbmp);
            let _ = DeleteDC(self.hdc_mem);
        }
    }
}

/// Pure Win32 style sanitizer ensuring borderless WS_POPUP + WS_EX_LAYERED never regress.
fn sanitize_layered_window_styles(style: u32, ex_style: i32, topmost: bool) -> (u32, i32) {
    const WS_POPUP_M: u32 = 0x8000_0000;
    const WS_CAPTION_M: u32 = 0x00C0_0000;
    const WS_THICKFRAME_M: u32 = 0x0004_0000;
    const WS_BORDER_M: u32 = 0x0080_0000;
    const WS_SYSMENU_M: u32 = 0x0008_0000;
    const WS_MINIMIZEBOX_M: u32 = 0x0002_0000;
    const WS_MAXIMIZEBOX_M: u32 = 0x0001_0000;

    const WS_EX_TOPMOST_M: i32 = 0x0000_0008;
    const WS_EX_TRANSPARENT_M: i32 = 0x0000_0020;
    const WS_EX_TOOLWINDOW_M: i32 = 0x0000_0080;
    const WS_EX_LAYERED_M: i32 = 0x0008_0000;
    const WS_EX_NOREDIRECTIONBITMAP_M: i32 = 0x0020_0000;
    const WS_EX_NOACTIVATE_M: i32 = 0x0800_0000;

    let strip_mask = WS_CAPTION_M
        | WS_THICKFRAME_M
        | WS_BORDER_M
        | WS_SYSMENU_M
        | WS_MINIMIZEBOX_M
        | WS_MAXIMIZEBOX_M;
    let clean_style = (style & !strip_mask) | WS_POPUP_M;

    let mut clean_ex = (ex_style | WS_EX_NOACTIVATE_M | WS_EX_TOOLWINDOW_M | WS_EX_LAYERED_M)
        & !WS_EX_NOREDIRECTIONBITMAP_M
        & !WS_EX_TRANSPARENT_M;
    if topmost {
        clean_ex |= WS_EX_TOPMOST_M;
    } else {
        clean_ex &= !WS_EX_TOPMOST_M;
    }
    (clean_style, clean_ex)
}

#[cfg(windows)]
fn enforce_layered_styles(hwnd: windows::Win32::Foundation::HWND, topmost: bool) {
    use windows::Win32::UI::WindowsAndMessaging::{
        GWL_EXSTYLE, GWL_STYLE, GetWindowLongW, SetWindowLongW,
    };

    unsafe {
        let style = GetWindowLongW(hwnd, GWL_STYLE) as u32;
        let ex = GetWindowLongW(hwnd, GWL_EXSTYLE);
        let (clean_style, new_ex) = sanitize_layered_window_styles(style, ex, topmost);
        if clean_style != style {
            SetWindowLongW(hwnd, GWL_STYLE, clean_style as i32);
        }
        if new_ex != ex {
            SetWindowLongW(hwnd, GWL_EXSTYLE, new_ex);
        }
    }
}

#[cfg(windows)]
fn set_window_visible_safe(window: &Window, visible: bool, topmost: bool) {
    use windows::Win32::UI::WindowsAndMessaging::{SW_HIDE, SW_SHOWNOACTIVATE, ShowWindow};
    if let Some(hwnd) = hwnd_of(window) {
        if visible {
            enforce_layered_styles(hwnd, topmost);
            unsafe {
                let _ = ShowWindow(hwnd, SW_SHOWNOACTIVATE);
            }
            apply_topmost(window, topmost);
        } else {
            unsafe {
                let _ = ShowWindow(hwnd, SW_HIDE);
            }
        }
    }
}

#[cfg(not(windows))]
fn set_window_visible_safe(window: &Window, visible: bool, _topmost: bool) {
    window.set_visible(visible);
}

#[cfg(windows)]
fn prevent_focus_stealing(window: &Window, topmost: bool) {
    if let Some(hwnd) = hwnd_of(window) {
        enforce_layered_styles(hwnd, topmost);
        if let Some(mut prime) = LayeredSurface::new(1, 1) {
            prime.present(window, &[0u32], 0, 0);
        }
        apply_topmost(window, topmost);
    }
}

#[cfg(windows)]
fn get_work_area() -> Option<windows::Win32::Foundation::RECT> {
    use windows::Win32::UI::WindowsAndMessaging::{
        SPI_GETWORKAREA, SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS, SystemParametersInfoW,
    };
    let mut rect = windows::Win32::Foundation::RECT::default();
    unsafe {
        if SystemParametersInfoW(
            SPI_GETWORKAREA,
            0,
            Some(&mut rect as *mut _ as _),
            SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
        )
        .is_ok()
        {
            return Some(rect);
        }
    }
    None
}

#[cfg(windows)]
fn show_pet_context_menu(
    window: &Window,
    can_clone: bool,
    can_dismiss: bool,
    follow_cursor: bool,
) -> i32 {
    use windows::Win32::Foundation::POINT;
    use windows::Win32::UI::WindowsAndMessaging::{
        AppendMenuW, CreatePopupMenu, DestroyMenu, GetCursorPos, MF_CHECKED, MF_GRAYED,
        MF_SEPARATOR, MF_STRING, SetForegroundWindow, TPM_RETURNCMD, TPM_RIGHTBUTTON,
        TrackPopupMenu,
    };
    use windows::core::PCWSTR;

    let Some(hwnd) = hwnd_of(window) else { return 0 };
    unsafe {
        let Ok(hmenu) = CreatePopupMenu() else { return 0 };
        let add_item = |id: usize, label: &str, enabled: bool, checked: bool| {
            let wide: Vec<u16> = label.encode_utf16().chain(std::iter::once(0)).collect();
            let mut flags = MF_STRING;
            if !enabled {
                flags |= MF_GRAYED;
            }
            if checked {
                flags |= MF_CHECKED;
            }
            let _ = AppendMenuW(hmenu, flags, id, PCWSTR(wide.as_ptr()));
        };
        add_item(1, "Headpat / Hop", true, false);
        add_item(7, "Drop Bouncy Ball", true, false);
        add_item(8, "Follow Cursor", true, follow_cursor);
        add_item(2, "Sleep", true, false);
        let _ = AppendMenuW(hmenu, MF_SEPARATOR, 0, PCWSTR::null());
        add_item(3, "Clone Pet (+1)", can_clone, false);
        add_item(4, "Dismiss This Pet", can_dismiss, false);
        let _ = AppendMenuW(hmenu, MF_SEPARATOR, 0, PCWSTR::null());
        add_item(5, "Application Settings", true, false);
        add_item(6, "DIYPets Studio", true, false);

        let mut pt = POINT::default();
        let _ = GetCursorPos(&mut pt);
        let _ = SetForegroundWindow(hwnd);
        let cmd = TrackPopupMenu(
            hmenu,
            TPM_RETURNCMD | TPM_RIGHTBUTTON,
            pt.x,
            pt.y,
            0,
            hwnd,
            None,
        )
        .0;
        let _ = DestroyMenu(hmenu);
        cmd
    }
}



fn stamp_hearts_argb(buf: &mut [u32], w: u32, h: u32, progress: f32) {
    const HEART: [&str; 6] = [
        ".##.##.",
        "#######",
        "#######",
        ".#####.",
        "..###..",
        "...#...",
    ];
    let scale: i32 = if w >= 96 { 2 } else { 1 };
    let fill: u32 = 0xFF_F0_3C_62;
    let hi: u32 = 0xFF_FF_C6_D4;
    let out: u32 = 0xFF_38_10_1C;

    for (k, (bx_ratio, phase)) in [(0.32_f32, 0.0_f32), (0.64_f32, 0.25_f32)]
        .into_iter()
        .enumerate()
    {
        let p = ((progress + phase) % 1.0).clamp(0.0, 1.0);
        let sway = ((p * std::f32::consts::TAU + k as f32).sin() * 4.0).round() as i32;
        let ox = (w as f32 * bx_ratio) as i32 + sway - (7 * scale) / 2;
        let oy = ((h as f32 * 0.28) * (1.0 - p)) as i32;

        for (ry, row) in HEART.iter().enumerate() {
            for (rx, ch) in row.bytes().enumerate() {
                if ch != b'#' {
                    continue;
                }
                let col = if ry == 1 && (rx == 1 || rx == 2) {
                    hi
                } else {
                    fill
                };
                for sy in 0..scale {
                    for sx in 0..scale {
                        let px = ox + rx as i32 * scale + sx;
                        let py = oy + ry as i32 * scale + sy;
                        if px >= 1 && px < w as i32 - 1 && py >= 1 && py < h as i32 - 1 {
                            let idx = (py as u32 * w + px as u32) as usize;
                            if idx < buf.len() {
                                buf[idx] = col;
                                if idx + 1 < buf.len() && buf[idx + 1] == 0 {
                                    buf[idx + 1] = out;
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

struct SpeechBubbleOverlay {
    window: Rc<Window>,
    #[cfg(windows)]
    layered: Option<LayeredSurface>,
    #[cfg(not(windows))]
    surface: Option<Surface<Rc<Window>, Rc<Window>>>,
    pixels: Vec<u32>,
    width: u32,
    height: u32,
    until_ms: u64,
    last_pos: Option<(i32, i32)>,
}

impl SpeechBubbleOverlay {
    fn create(event_loop: &ActiveEventLoop, text: &str, until_ms: u64) -> Option<Self> {
        let (w, h, pixels) = render_speech_bubble_argb(text);
        let attr = Window::default_attributes()
            .with_title("diypets-bubble")
            .with_inner_size(winit::dpi::PhysicalSize::new(w, h))
            .with_resizable(false)
            .with_decorations(false)
            .with_transparent(cfg!(not(windows)))
            .with_window_level(WindowLevel::AlwaysOnTop);
        let window = Rc::new(event_loop.create_window(attr).ok()?);
        let _ = window.set_cursor_hittest(false);
        window.set_visible(true);

        #[cfg(windows)]
        let layered = {
            prevent_focus_stealing(&window, true);
            let _ = window.set_cursor_hittest(false);
            LayeredSurface::new(w, h)
        };

        #[cfg(not(windows))]
        let surface = {
            let ctx = Context::new(window.clone()).ok()?;
            let mut surf = Surface::new(&ctx, window.clone()).ok()?;
            let _ = surf.resize(NonZeroU32::new(w)?, NonZeroU32::new(h)?);
            Some(surf)
        };

        Some(Self {
            window,
            #[cfg(windows)]
            layered,
            #[cfg(not(windows))]
            surface,
            pixels,
            width: w,
            height: h,
            until_ms,
            last_pos: None,
        })
    }

    fn update_position(&mut self, pet_x: f32, pet_y: f32, pet_size: u32) {
        let bx = (pet_x + (pet_size as f32 * 0.5) - (self.width as f32 * 0.5)).round() as i32;
        let by = (pet_y - self.height as f32 - 4.0).max(4.0).round() as i32;
        if self.last_pos == Some((bx, by)) {
            return;
        }
        self.last_pos = Some((bx, by));
        self.window.set_outer_position(PhysicalPosition::new(bx, by));

        #[cfg(windows)]
        if let Some(layered) = &mut self.layered {
            layered.present(&self.window, &self.pixels, bx, by);
        }

        #[cfg(not(windows))]
        if let Some(surface) = &mut self.surface {
            if let Ok(mut buf) = surface.buffer_mut() {
                let n = buf.len().min(self.pixels.len());
                buf[..n].copy_from_slice(&self.pixels[..n]);
                let _ = buf.present();
            }
        }
    }
}

struct ToyBallOverlay {
    window: Rc<Window>,
    #[cfg(windows)]
    layered: Option<LayeredSurface>,
    #[cfg(not(windows))]
    surface: Option<Surface<Rc<Window>, Rc<Window>>>,
    pixels: Vec<u32>,
    size: u32,
    x: f32,
    y: f32,
    vx: f32,
    vy: f32,
    until_ms: u64,
    last_kick_ms: u64,
    last_pos: Option<(i32, i32)>,
}

impl ToyBallOverlay {
    fn spawn(event_loop: &ActiveEventLoop, start_x: f32, start_y: f32, now_ms: u64) -> Option<Self> {
        let (w, h, pixels) = render_pixel_ball_argb();
        let attr = Window::default_attributes()
            .with_title("diypets-ball")
            .with_inner_size(winit::dpi::PhysicalSize::new(w, h))
            .with_resizable(false)
            .with_decorations(false)
            .with_transparent(cfg!(not(windows)))
            .with_window_level(WindowLevel::AlwaysOnTop);
        let window = Rc::new(event_loop.create_window(attr).ok()?);
        let _ = window.set_cursor_hittest(false);
        window.set_visible(true);

        #[cfg(windows)]
        let layered = {
            prevent_focus_stealing(&window, true);
            let _ = window.set_cursor_hittest(false);
            LayeredSurface::new(w, h)
        };

        #[cfg(not(windows))]
        let surface = {
            let ctx = Context::new(window.clone()).ok()?;
            let mut surf = Surface::new(&ctx, window.clone()).ok()?;
            let _ = surf.resize(NonZeroU32::new(w)?, NonZeroU32::new(h)?);
            Some(surf)
        };

        let vx = if now_ms.is_multiple_of(2) { 6.0 } else { -6.0 };
        Some(Self {
            window,
            #[cfg(windows)]
            layered,
            #[cfg(not(windows))]
            surface,
            pixels,
            size: w,
            x: start_x,
            y: start_y,
            vx,
            vy: -4.0,
            until_ms: now_ms + 20_000,
            last_kick_ms: 0,
            last_pos: None,
        })
    }

    fn step_and_present(&mut self, min_x: f32, max_x: f32, floor_y: f32) {
        self.vy += 0.65;
        self.x += self.vx;
        self.y += self.vy;

        let max_ball_x = (max_x - self.size as f32).max(min_x);
        if self.x < min_x {
            self.x = min_x;
            self.vx = self.vx.abs() * 0.82;
        } else if self.x > max_ball_x {
            self.x = max_ball_x;
            self.vx = -self.vx.abs() * 0.82;
        }

        let max_ball_y = (floor_y - self.size as f32).max(4.0);
        if self.y >= max_ball_y {
            self.y = max_ball_y;
            if self.vy.abs() > 2.2 {
                self.vy = -self.vy * 0.72;
            } else {
                self.vy = 0.0;
            }
            self.vx *= 0.96;
            if self.vx.abs() < 0.08 {
                self.vx = 0.0;
            }
        } else if self.y < 4.0 {
            self.y = 4.0;
            self.vy = self.vy.abs() * 0.5;
        }

        let px = self.x.round() as i32;
        let py = self.y.round() as i32;
        if self.last_pos == Some((px, py)) {
            return;
        }
        self.last_pos = Some((px, py));
        self.window.set_outer_position(PhysicalPosition::new(px, py));

        #[cfg(windows)]
        if let Some(layered) = &mut self.layered {
            layered.present(&self.window, &self.pixels, px, py);
        }

        #[cfg(not(windows))]
        if let Some(surface) = &mut self.surface {
            if let Ok(mut buf) = surface.buffer_mut() {
                let n = buf.len().min(self.pixels.len());
                buf[..n].copy_from_slice(&self.pixels[..n]);
                let _ = buf.present();
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct RenderKey {
    frame_idx: usize,
    moving_left: bool,
    opacity_u8: u8,
    x: i32,
    y: i32,
    dst_w: u32,
    dst_h: u32,
    heart_tick: u8,
    squash_x_q: u16,
    squash_y_q: u16,
}

struct PetInstance {
    pack_id: String,
    window: Rc<Window>,
    #[cfg(not(windows))]
    surface: Option<Surface<Rc<Window>, Rc<Window>>>,
    #[cfg(windows)]
    layered: Option<LayeredSurface>,
    #[cfg(windows)]
    buffer: Vec<u32>,
    #[cfg(windows)]
    scaled_buf: Vec<u32>,
    #[cfg(windows)]
    squash_buf: Vec<u32>,
    #[cfg(windows)]
    frame_cache: HashMap<(usize, bool, u32, u32), Vec<u32>>,
    state: DeskpetState,
    last_tick: u64,
    last_window_scan: u64,
    cached_floor_y: Option<f32>,
    cached_wall_left: Option<f32>,
    cached_wall_right: Option<f32>,
    standing_window: Option<(isize, i32, i32)>,
    cursor_pos: (f32, f32),
    drag_grab_offset: (f32, f32),
    cursor_velocity: (f32, f32),
    drag_sample_ms: u64,
    is_dragging: bool,
    heart_until_ms: u64,
    rub_accum: f32,
    last_rub_ms: u64,
    last_interact_ms: u64,
    squash: SquashSpring,
    was_airborne: bool,
    last_render: Option<RenderKey>,
}

struct DeskpetApp {
    pets: Vec<PetInstance>,
    desired_herd: Vec<String>,
    desired_sizes: Vec<u32>,
    desired_speeds: Vec<i32>,
    pet_size: u32,
    walk_speed: i32,
    is_topmost: bool,
    climb_windows: bool,
    click_through: bool,
    start_on_login: bool,
    follow_cursor: bool,
    auto_hide_fullscreen: bool,
    quiet_mode: bool,
    fullscreen_hidden: bool,
    fullscreen_streak: u8,
    last_visible: bool,
    last_fullscreen_check: u64,
    settings_path: PathBuf,
    last_settings_mod: Option<std::time::SystemTime>,
    start_time: Instant,
    next_frame: Instant,
    last_settings_poll: u64,
    tray: Option<TrayIcon>,
    settings_process: Option<std::process::Child>,
    creator_process: Option<std::process::Child>,
    #[allow(dead_code)]
    agent_tx: mpsc::Sender<AgentCommand>,
    agent_rx: mpsc::Receiver<AgentCommand>,
    shared_herd: SharedHerd,
    bubble: Option<SpeechBubbleOverlay>,
    ball: Option<ToyBallOverlay>,
}

impl DeskpetApp {
    fn spawn_sibling(slot: &mut Option<std::process::Child>, binary_stem: &str) {
        if let Some(child) = slot.as_mut()
            && matches!(child.try_wait(), Ok(None)) {
                return;
            }
        if let Some(exe) = sibling_binary(binary_stem) {
            let mut cmd = std::process::Command::new(exe);
            cmd.current_dir(resolve_project_root());
            if let Ok(child) = cmd.spawn() {
                *slot = Some(child);
            }
        }
    }

    fn slot_size(&self, slot: usize) -> u32 {
        self.desired_sizes.get(slot).copied().unwrap_or(self.pet_size)
    }

    fn slot_speed(&self, slot: usize) -> i32 {
        self.desired_speeds.get(slot).copied().unwrap_or(self.walk_speed)
    }

    fn persist_current_settings(&mut self) {
        let mut s = Settings::load_sanitized(&self.settings_path);
        s.always_on_top = self.is_topmost;
        s.start_on_login = self.start_on_login;
        s.follow_cursor = self.follow_cursor;
        s.auto_hide_fullscreen = self.auto_hide_fullscreen;
        if let Some(first) = self.desired_herd.first() {
            s.pet_pack = first.clone();
            s.extra_pets = self.desired_herd[1..].to_vec();
        }
        s = s.sanitized();
        if s.save(&self.settings_path).is_ok() {
            self.last_settings_mod = std::fs::metadata(&self.settings_path)
                .and_then(|m| m.modified())
                .ok();
        }
    }

    fn drop_toy_ball(&mut self, event_loop: &ActiveEventLoop, now_ms: u64) {
        let (sx, sy) = self
            .pets
            .first()
            .map(|p| ((p.state.x + 140.0).max(120.0), (p.state.y - 240.0).max(40.0)))
            .unwrap_or((600.0, 120.0));
        self.ball = ToyBallOverlay::spawn(event_loop, sx, sy, now_ms);
    }

    fn spawn_pet_instance(
        &self,
        event_loop: &ActiveEventLoop,
        pack_id: &str,
        slot_index: usize,
        now_ms: u64,
    ) -> PetInstance {
        let pack = load_pack(pack_id);
        let icon_img = pet_icon(pack_id, &pack);

        let mut state = DeskpetState::new(pack);
        state.pet_size = self.slot_size(slot_index);
        state.walk_speed = self.slot_speed(slot_index);
        if slot_index > 0 {
            state.x = 180.0 + ((slot_index as f32 * 190.0) % 980.0);
            state.y = 40.0 + ((slot_index as f32 * 45.0) % 180.0);
            state.is_moving_left = slot_index.is_multiple_of(2);
        }

        let scaled_w = state.pet_size;
        let scaled_h = scaled_height(state.pack.tile_w, state.pack.tile_h, scaled_w);

        let attr = Window::default_attributes()
            .with_title(format!("diypets - {pack_id}"))
            .with_window_icon(icon_img.and_then(|img| {
                winit::window::Icon::from_rgba(img.to_vec(), img.width(), img.height()).ok()
            }))
            .with_inner_size(winit::dpi::PhysicalSize::new(scaled_w, scaled_h))
            .with_resizable(false)
            .with_decorations(false)
            .with_transparent(cfg!(not(windows)))
            .with_window_level(if self.is_topmost {
                WindowLevel::AlwaysOnTop
            } else {
                WindowLevel::Normal
            });

        let window = Rc::new(event_loop.create_window(attr).unwrap());

        #[cfg(not(windows))]
        let surface = {
            let context = Context::new(window.clone()).unwrap();
            let mut surface = Surface::new(&context, window.clone()).unwrap();
            surface
                .resize(
                    NonZeroU32::new(scaled_w).unwrap(),
                    NonZeroU32::new(scaled_h).unwrap(),
                )
                .unwrap();
            Some(surface)
        };

        #[cfg(not(windows))]
        window
            .set_cursor_hittest(!self.click_through)
            .unwrap_or(());
        #[cfg(windows)]
        let _ = window.set_cursor_hittest(true);
        window.set_visible(true);

        #[cfg(windows)]
        let (layered, buffer, scaled_buf) = {
            prevent_focus_stealing(&window, self.is_topmost);
            let buffer = vec![0; (state.pack.tile_w * state.pack.tile_h) as usize];
            let scaled_buf = vec![0; (scaled_w * scaled_h) as usize];
            let mut layered = LayeredSurface::new(scaled_w, scaled_h);
            if let Some(l) = &mut layered {
                l.present(
                    &window,
                    &scaled_buf,
                    state.x.round() as i32,
                    state.y.round() as i32,
                );
            }
            (layered, buffer, scaled_buf)
        };

        window.request_redraw();

        PetInstance {
            pack_id: pack_id.to_string(),
            window,
            #[cfg(not(windows))]
            surface,
            #[cfg(windows)]
            layered,
            #[cfg(windows)]
            buffer,
            #[cfg(windows)]
            scaled_buf,
            #[cfg(windows)]
            squash_buf: Vec::new(),
            #[cfg(windows)]
            frame_cache: HashMap::new(),
            state,
            last_tick: now_ms,
            last_window_scan: 0,
            cached_floor_y: None,
            cached_wall_left: None,
            cached_wall_right: None,
            standing_window: None,
            cursor_pos: (0.0, 0.0),
            drag_grab_offset: (64.0, 24.0),
            cursor_velocity: (0.0, 0.0),
            drag_sample_ms: 1,
            is_dragging: false,
            heart_until_ms: 0,
            rub_accum: 0.0,
            last_rub_ms: 0,
            last_interact_ms: 0,
            squash: SquashSpring::default(),
            was_airborne: false,
            last_render: None,
        }
    }

    fn sync_herd(&mut self, event_loop: &ActiveEventLoop, now_ms: u64) {
        if self.desired_herd.is_empty() {
            self.desired_herd.push("highland_cow".to_string());
        }
        self.desired_herd.truncate(MAX_ACTIVE_PETS);

        if self.pets.len() > self.desired_herd.len() {
            self.pets.truncate(self.desired_herd.len());
        }

        for i in 0..self.pets.len() {
            let wanted = self.desired_herd[i].clone();
            if self.pets[i].pack_id != wanted {
                let pack = load_pack(&wanted);
                if let Some(img) = pet_icon(&wanted, &pack) {
                    if i == 0
                        && let Some(tray) = &self.tray {
                            let icon =
                                TrayIconImage::from_rgba(img.to_vec(), img.width(), img.height())
                                    .ok();
                            if let Err(e) = tray.set_icon(icon) { logging::warn(&format!("runner: tray icon update failed: {e}")); }
                        }
                    self.pets[i].window.set_window_icon(
                        winit::window::Icon::from_rgba(img.to_vec(), img.width(), img.height())
                            .ok(),
                    );
                }
                let prev_x = self.pets[i].state.x;
                let prev_y = self.pets[i].state.y;
                let mut st = DeskpetState::new(pack);
                st.pet_size = self.slot_size(i);
                st.walk_speed = self.slot_speed(i);
                st.x = prev_x;
                st.y = prev_y;

                #[cfg(windows)]
                {
                    self.pets[i].buffer = vec![0; (st.pack.tile_w * st.pack.tile_h) as usize];
                    self.pets[i].frame_cache.clear();
                }
                self.pets[i].pack_id = wanted;
                self.pets[i].state = st;
                self.pets[i].last_render = None;

                let scaled_w = self.slot_size(i);
                let scaled_h = scaled_height(
                    self.pets[i].state.pack.tile_w,
                    self.pets[i].state.pack.tile_h,
                    scaled_w,
                );
                let _ = self.pets[i]
                    .window
                    .request_inner_size(winit::dpi::PhysicalSize::new(scaled_w, scaled_h));
                #[cfg(not(windows))]
                if let Some(surface) = &mut self.pets[i].surface {
                    if let (Some(w), Some(h)) =
                        (NonZeroU32::new(scaled_w), NonZeroU32::new(scaled_h))
                    {
                        let _ = surface.resize(w, h);
                    }
                }
            }
        }

        while self.pets.len() < self.desired_herd.len() {
            let idx = self.pets.len();
            let id = self.desired_herd[idx].clone();
            let inst = self.spawn_pet_instance(event_loop, &id, idx, now_ms);
            self.pets.push(inst);
        }
        if let Ok(mut guard) = self.shared_herd.lock() {
            *guard = self.desired_herd.clone();
        }
    }

    fn reload_settings_if_changed(&mut self, event_loop: &ActiveEventLoop, now_ms: u64) {
        if now_ms.saturating_sub(self.last_settings_poll) < SETTINGS_POLL_MS {
            return;
        }
        self.last_settings_poll = now_ms;

        let Ok(metadata) = std::fs::metadata(&self.settings_path) else {
            return;
        };
        let Ok(modified) = metadata.modified() else {
            return;
        };
        if self.last_settings_mod == Some(modified) {
            return;
        }
        self.last_settings_mod = Some(modified);

        let settings = Settings::load_sanitized(&self.settings_path);
        let topmost_changed = self.is_topmost != settings.always_on_top;
        self.is_topmost = settings.always_on_top;
        self.climb_windows = settings.climb_windows;
        self.click_through = settings.click_through;
        self.follow_cursor = settings.follow_cursor;
        self.auto_hide_fullscreen = settings.auto_hide_fullscreen;
        self.pet_size = settings.pet_size;
        self.walk_speed = settings.walk_speed;
        if self.start_on_login != settings.start_on_login {
            self.start_on_login = settings.start_on_login;
            if let Err(e) = autostart::set_enabled(self.start_on_login) { logging::warn(&format!("runner: autostart toggle failed: {e}")); }
        }

        let herd = settings.active_herd();
        self.desired_sizes = (0..herd.len()).map(|i| settings.slot_pet_size(i)).collect();
        self.desired_speeds = (0..herd.len()).map(|i| settings.slot_walk_speed(i)).collect();
        self.desired_herd = herd;
        self.sync_herd(event_loop, now_ms);

        for i in 0..self.pets.len() {
            let sz = self.slot_size(i);
            let sp = self.slot_speed(i);
            let pet = &mut self.pets[i];
            let size_changed = pet.state.pet_size != sz;
            pet.state.pet_size = sz;
            pet.state.walk_speed = sp;
            pet.cached_floor_y = None;
            pet.cached_wall_left = None;
            pet.cached_wall_right = None;
            pet.last_render = None;
            #[cfg(windows)]
            if size_changed {
                pet.frame_cache.clear();
            }

            if topmost_changed {
                pet.window.set_window_level(if self.is_topmost {
                    WindowLevel::AlwaysOnTop
                } else {
                    WindowLevel::Normal
                });
                #[cfg(windows)]
                apply_topmost(&pet.window, self.is_topmost);
            }
            #[cfg(not(windows))]
            pet.window
                .set_cursor_hittest(!self.click_through)
                .unwrap_or(());
            #[cfg(windows)]
            let _ = pet.window.set_cursor_hittest(true);

            if size_changed {
                let scaled_h = scaled_height(pet.state.pack.tile_w, pet.state.pack.tile_h, sz);
                let _ = pet
                    .window
                    .request_inner_size(winit::dpi::PhysicalSize::new(sz, scaled_h));
                #[cfg(not(windows))]
                if let Some(surface) = &mut pet.surface {
                    if let (Some(w), Some(h)) = (NonZeroU32::new(sz), NonZeroU32::new(scaled_h)) {
                        let _ = surface.resize(w, h);
                    }
                }
            }
        }
    }

    fn process_tray_events(&mut self, event_loop: &ActiveEventLoop, now_ms: u64) {
        while let Ok(cmd) = self.agent_rx.try_recv() {
            if let Some(msg) = cmd.text.as_deref()
                && !msg.trim().is_empty()
            {
                let dur = cmd.duration_ms.unwrap_or(4500).clamp(1000, 15_000);
                self.bubble = SpeechBubbleOverlay::create(event_loop, msg.trim(), now_ms + dur);
            }
            match cmd.mood.to_lowercase().as_str() {
                "happy" | "hop" | "celebrate" => {
                    for pet in &mut self.pets {
                        pet.heart_until_ms = now_ms + 2600;
                        pet.state.is_tossing = true;
                        pet.state.toss_force_x = if pet.state.is_moving_left { -3.0 } else { 3.0 };
                        pet.state.toss_vert_vel = -10.0;
                        pet.squash.trigger_hop();
                    }
                }
                "say" => {}
                "open_settings" => Self::spawn_sibling(&mut self.settings_process, "diypets-settings"),
                "open_creator" => Self::spawn_sibling(&mut self.creator_process, "diypets-creator"),
                "follow" | "follow_cursor" => {
                    self.follow_cursor = cmd.enabled.unwrap_or(!self.follow_cursor);
                    self.persist_current_settings();
                }
                "ball" | "play" | "treat" => {
                    self.drop_toy_ball(event_loop, now_ms);
                }
                "sleep" => {
                    for pet in &mut self.pets {
                        pet.state
                            .set_state(DeskpetStateId::Custom("sleep".to_string()));
                    }
                }
                "walk" => {
                    for pet in &mut self.pets {
                        pet.state.set_state(DeskpetStateId::Custom("1".to_string()));
                    }
                }
                "spawn"
                    if self.desired_herd.len() < MAX_ACTIVE_PETS => {
                        let id = cmd.pack_id.unwrap_or_else(|| {
                            self.desired_herd
                                .first()
                                .cloned()
                                .unwrap_or_else(|| "highland_cow".to_string())
                        });
                        self.desired_herd.push(id);
                        self.desired_sizes.push(self.pet_size);
                        self.desired_speeds.push(self.walk_speed);
                        self.sync_herd(event_loop, now_ms);
                        self.persist_current_settings();
                    }
                _ => {}
            }
        }

        let _ = tray_icon::TrayIconEvent::receiver().try_recv();
        while let Ok(tray_event) = tray_icon::menu::MenuEvent::receiver().try_recv() {
            match tray_event.id.0.as_str() {
                "quit" => event_loop.exit(),
                "drop_ball" => {
                    self.drop_toy_ball(event_loop, now_ms);
                }
                "quiet_mode" => {
                    self.quiet_mode = !self.quiet_mode;
                    self.sync_visibility();
                }
                "follow_cursor" => {
                    self.follow_cursor = !self.follow_cursor;
                    self.persist_current_settings();
                }
                id if id.starts_with("add_pack:") => {
                    if self.desired_herd.len() < MAX_ACTIVE_PETS {
                        let pack_id = id.trim_start_matches("add_pack:").to_string();
                        self.desired_herd.push(pack_id);
                        self.desired_sizes.push(self.pet_size);
                        self.desired_speeds.push(self.walk_speed);
                        self.sync_herd(event_loop, now_ms);
                        self.persist_current_settings();
                    }
                }
                "remove_pet" => {
                    if self.desired_herd.len() > 1 {
                        self.desired_herd.pop();
                        self.desired_sizes.pop();
                        self.desired_speeds.pop();
                        self.sync_herd(event_loop, now_ms);
                        self.persist_current_settings();
                    }
                }
                "sleep" => {
                    for pet in &mut self.pets {
                        pet.state
                            .set_state(DeskpetStateId::Custom("sleep".to_string()));
                    }
                }
                "settings" => {
                    Self::spawn_sibling(&mut self.settings_process, "diypets-settings");
                }
                "creator" => {
                    Self::spawn_sibling(&mut self.creator_process, "diypets-creator");
                }
                "topmost" => {
                    self.is_topmost = !self.is_topmost;
                    for pet in &self.pets {
                        pet.window.set_window_level(if self.is_topmost {
                            WindowLevel::AlwaysOnTop
                        } else {
                            WindowLevel::Normal
                        });
                        #[cfg(windows)]
                        apply_topmost(&pet.window, self.is_topmost);
                    }
                    self.persist_current_settings();
                }
                "autostart" => {
                    self.start_on_login = !self.start_on_login;
                    if autostart::set_enabled(self.start_on_login).is_ok() {
                        self.persist_current_settings();
                    } else {
                        self.start_on_login = !self.start_on_login;
                    }
                }
                _ => {}
            }
        }
    }

    fn sync_visibility(&mut self) {
        let should_show = !self.quiet_mode && !self.fullscreen_hidden;
        if should_show == self.last_visible {
            return;
        }
        logging::info(&format!(
            "sync_visibility: {} -> {} (quiet_mode={}, fullscreen_hidden={})",
            self.last_visible, should_show, self.quiet_mode, self.fullscreen_hidden
        ));
        self.last_visible = should_show;
        let topmost = self.is_topmost;
        for pet in &mut self.pets {
            set_window_visible_safe(&pet.window, should_show, topmost);
            if should_show {
                pet.last_render = None;
                pet.window.request_redraw();
            }
        }
        if let Some(b) = &self.bubble {
            set_window_visible_safe(&b.window, should_show, true);
        }
        if let Some(ball) = &self.ball {
            set_window_visible_safe(&ball.window, should_show, true);
        }
    }

    fn check_fullscreen_auto_hide(&mut self, event_loop: &ActiveEventLoop, now_ms: u64) {
        if !self.auto_hide_fullscreen {
            self.fullscreen_hidden = false;
            self.sync_visibility();
            return;
        }
        if now_ms.saturating_sub(self.last_fullscreen_check) < 350 {
            return;
        }
        self.last_fullscreen_check = now_ms;

        let monitors: Vec<(i32, i32, i32, i32)> = event_loop
            .available_monitors()
            .map(|m| {
                let p = m.position();
                let s = m.size();
                (p.x, p.y, p.x + s.width as i32, p.y + s.height as i32)
            })
            .collect();

        #[cfg(windows)]
        let own_hwnds: Vec<isize> = {
            let mut v: Vec<isize> = self
                .pets
                .iter()
                .filter_map(|p| hwnd_of(&p.window).map(|h| h.0 as isize))
                .collect();
            if let Some(b) = &self.bubble
                && let Some(h) = hwnd_of(&b.window)
            {
                v.push(h.0 as isize);
            }
            if let Some(ball) = &self.ball
                && let Some(h) = hwnd_of(&ball.window)
            {
                v.push(h.0 as isize);
            }
            v
        };
        #[cfg(not(windows))]
        let own_hwnds: Vec<isize> = Vec::new();

        let detected = platform::is_fullscreen_app_active(&own_hwnds, &monitors);
        if detected {
            self.fullscreen_streak = self.fullscreen_streak.saturating_add(1);
            if self.fullscreen_streak >= 2 {
                self.fullscreen_hidden = true;
            }
        } else {
            self.fullscreen_streak = 0;
            self.fullscreen_hidden = false;
        }
        self.sync_visibility();
    }

    fn step_herd_interactions(&mut self, now_ms: u64) {
        let count = self.pets.len();
        if count < 2 {
            return;
        }
        for i in 0..count {
            for j in (i + 1)..count {
                let can_i = !self.pets[i].is_dragging
                    && self.pets[i].state.current_anim_id != "13"
                    && now_ms.saturating_sub(self.pets[i].last_interact_ms) >= 6000;
                let can_j = !self.pets[j].is_dragging
                    && self.pets[j].state.current_anim_id != "13"
                    && now_ms.saturating_sub(self.pets[j].last_interact_ms) >= 6000;
                if !can_i || !can_j {
                    continue;
                }
                let ax = self.pets[i].state.x + self.pets[i].state.pet_size as f32 * 0.5;
                let ay = self.pets[i].state.y + self.pets[i].state.ground_height();
                let bx = self.pets[j].state.x + self.pets[j].state.pet_size as f32 * 0.5;
                let by = self.pets[j].state.y + self.pets[j].state.ground_height();

                let Some(kind) = detect_herd_encounter(
                    ax,
                    ay,
                    self.pets[i].state.is_moving_left,
                    bx,
                    by,
                    self.pets[j].state.is_moving_left,
                    now_ms / 100 + (i + j) as u64,
                ) else {
                    continue;
                };

                self.pets[i].last_interact_ms = now_ms;
                self.pets[j].last_interact_ms = now_ms;
                let a_on_left = ax <= bx;
                self.pets[i].state.is_moving_left = a_on_left;
                self.pets[j].state.is_moving_left = !a_on_left;

                if matches!(kind, HerdEncounter::Greet) {
                    self.pets[i].heart_until_ms = now_ms + 1800;
                    self.pets[j].heart_until_ms = now_ms + 1800;
                    self.pets[i].state.is_tossing = true;
                    self.pets[i].state.toss_force_x = if a_on_left { -2.2 } else { 2.2 };
                    self.pets[i].state.toss_vert_vel = -7.5;
                    self.pets[i].squash.trigger_hop();
                    self.pets[j].state.is_tossing = true;
                    self.pets[j].state.toss_force_x = if a_on_left { 2.2 } else { -2.2 };
                    self.pets[j].state.toss_vert_vel = -7.5;
                    self.pets[j].squash.trigger_hop();
                }
            }
        }
    }

    fn step_play_mode(&mut self, event_loop: &ActiveEventLoop, now_ms: u64) {
        let mut min_x: f32 = 0.0;
        let mut max_x: f32 = 1920.0;
        let mut max_y: f32 = 1080.0;
        let mut any_mon = false;
        for monitor in event_loop.available_monitors() {
            let pos = monitor.position();
            let size = monitor.size();
            if !any_mon {
                min_x = pos.x as f32;
                max_x = (pos.x + size.width as i32) as f32;
                max_y = (pos.y + size.height as i32) as f32;
                any_mon = true;
            } else {
                min_x = min_x.min(pos.x as f32);
                max_x = max_x.max((pos.x + size.width as i32) as f32);
                max_y = max_y.max((pos.y + size.height as i32) as f32);
            }
        }
        let mut taskbar_y = max_y;
        #[cfg(windows)]
        if let Some(rect) = get_work_area() {
            taskbar_y = rect.bottom as f32;
        }

        if let Some(ball) = &mut self.ball {
            if now_ms >= ball.until_ms {
                self.ball = None;
            } else {
                ball.step_and_present(min_x, max_x, taskbar_y);
                let ball_cx = ball.x + ball.size as f32 * 0.5;
                let ball_cy = ball.y + ball.size as f32 * 0.5;

                for pet in &mut self.pets {
                    if pet.is_dragging || pet.state.current_anim_id == "13" {
                        continue;
                    }
                    let pet_cx = pet.state.x + pet.state.pet_size as f32 * 0.5;
                    let pet_cy = pet.state.y + pet.state.ground_height() * 0.65;
                    let dx = ball_cx - pet_cx;
                    let dy = ball_cy - pet_cy;

                    if dx.abs() < 36.0
                        && dy.abs() < 56.0
                        && now_ms.saturating_sub(ball.last_kick_ms) > 550
                    {
                        ball.vy = -12.5;
                        ball.vx = if dx >= 0.0 { 7.5 } else { -7.5 };
                        ball.last_kick_ms = now_ms;
                        ball.until_ms = now_ms + 16_000;
                        pet.heart_until_ms = now_ms + 1800;
                    } else if dx.abs() > 22.0 {
                        pet.state.is_moving_left = dx < 0.0;
                        if matches!(pet.state.current_anim_id.as_str(), "2" | "12") {
                            pet.state.set_state(DeskpetStateId::Custom("1".to_string()));
                        }
                    }
                }
            }
        } else if self.follow_cursor
            && let Some((cx, _cy)) = platform::global_cursor_pos() {
                for pet in &mut self.pets {
                    if pet.is_dragging || pet.state.current_anim_id == "13" {
                        continue;
                    }
                    let pet_cx = pet.state.x + pet.state.pet_size as f32 * 0.5;
                    let dx = cx - pet_cx;
                    if dx.abs() > 48.0 {
                        pet.state.is_moving_left = dx < 0.0;
                        if matches!(pet.state.current_anim_id.as_str(), "2" | "12") {
                            pet.state.set_state(DeskpetStateId::Custom("1".to_string()));
                        }
                    } else if matches!(pet.state.current_anim_id.as_str(), "1" | "8") {
                        pet.state.set_state(DeskpetStateId::Custom("2".to_string()));
                    }
                }
            }
    }
}

impl ApplicationHandler for DeskpetApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let now_ms = self.start_time.elapsed().as_millis() as u64;
        if self.pets.is_empty() {
            logging::info(&format!("resumed(): syncing herd={:?} pet_size={} click_through={} auto_hide_fullscreen={}", self.desired_herd, self.pet_size, self.click_through, self.auto_hide_fullscreen));
            self.sync_herd(event_loop, now_ms);
            logging::info(&format!("resumed(): spawned {} pet window(s)", self.pets.len()));
        }

        if self.tray.is_none() {
            let primary_id = self
                .desired_herd
                .first()
                .map(String::as_str)
                .unwrap_or("highland_cow");
            let image = self
                .pets
                .first()
                .and_then(|p| pet_icon(primary_id, &p.state.pack))
                .or_else(|| default_app_icon(64));

            let menu = Menu::new();
            let add_pet = Submenu::new("Add Pet to Herd (+1)", true);
            let mut installed_ids: Vec<String> = Vec::new();
            for dir in pack_search_dirs() {
                if let Ok(entries) = std::fs::read_dir(dir) {
                    for e in entries.flatten() {
                        if e.path().join("deskpet.xml").is_file() {
                            let id = e.file_name().to_string_lossy().into_owned();
                            if !installed_ids.contains(&id) {
                                installed_ids.push(id);
                            }
                        }
                    }
                }
            }
            installed_ids.sort();
            if installed_ids.is_empty() {
                installed_ids.push("highland_cow".to_string());
            }
            for id in installed_ids.iter().take(24) {
                let item = MenuItem::with_id(format!("add_pack:{id}"), id, true, None);
                let _ = add_pet.append(&item);
            }

            let remove_pet = MenuItem::with_id("remove_pet", "Remove Extra Pet (-1)", true, None);
            let drop_ball = MenuItem::with_id("drop_ball", "Drop Bouncy Ball", true, None);
            let follow = CheckMenuItem::with_id(
                "follow_cursor",
                "Follow Cursor",
                true,
                self.follow_cursor,
                None,
            );
            let quiet = CheckMenuItem::with_id("quiet_mode", "Quiet Mode (Hide Pets)", true, self.quiet_mode, None);
            let sleep = MenuItem::with_id("sleep", "Sleep All", true, None);
            let creator = MenuItem::with_id("creator", "DIYPets Studio", true, None);
            let settings = MenuItem::with_id("settings", "Application Settings", true, None);
            let topmost = CheckMenuItem::with_id(
                "topmost",
                "Always on Top",
                true,
                self.is_topmost,
                None,
            );
            let autostart_item = CheckMenuItem::with_id(
                "autostart",
                "Start on Login",
                true,
                self.start_on_login,
                None,
            );
            let quit = MenuItem::with_id("quit", "Quit", true, None);

            let _ = menu.append(&add_pet);
            let _ = menu.append(&remove_pet);
            let _ = menu.append(&PredefinedMenuItem::separator());
            let _ = menu.append(&drop_ball);
            let _ = menu.append(&follow);
            let _ = menu.append(&sleep);
            let _ = menu.append(&quiet);
            let _ = menu.append(&PredefinedMenuItem::separator());
            let _ = menu.append(&creator);
            let _ = menu.append(&settings);
            let _ = menu.append(&topmost);
            let _ = menu.append(&autostart_item);
            let _ = menu.append(&PredefinedMenuItem::separator());
            let _ = menu.append(&quit);

            let mut builder = TrayIconBuilder::new()
                .with_tooltip("DIYPets")
                .with_menu(Box::new(menu));
            if let Some(img) = &image
                && let Ok(icon) = TrayIconImage::from_rgba(img.to_vec(), img.width(), img.height())
                {
                    builder = builder.with_icon(icon);
                }
            self.tray = builder.build().map_err(|e| eprintln!("Tray icon: {e}")).ok();
        }

        self.next_frame = Instant::now();
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        let now = Instant::now();
        if now >= self.next_frame {
            self.next_frame = now + FRAME_INTERVAL;
            let now_ms = self.start_time.elapsed().as_millis() as u64;
            self.process_tray_events(event_loop, now_ms);
            self.reload_settings_if_changed(event_loop, now_ms);
            self.check_fullscreen_auto_hide(event_loop, now_ms);
            if !self.last_visible {
                event_loop.set_control_flow(ControlFlow::WaitUntil(self.next_frame));
                return;
            }
            self.step_herd_interactions(now_ms);
            self.step_play_mode(event_loop, now_ms);

            if let Some(b) = &mut self.bubble {
                if now_ms >= b.until_ms {
                    self.bubble = None;
                } else if let Some(primary) = self.pets.first() {
                    b.update_position(primary.state.x, primary.state.y, primary.state.pet_size);
                }
            }
            for pet in &self.pets {
                pet.window.request_redraw();
            }
        }
        event_loop.set_control_flow(ControlFlow::WaitUntil(self.next_frame));
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        let Some(pet_idx) = self.pets.iter().position(|p| p.window.id() == window_id) else {
            return;
        };

        match event {
            WindowEvent::CloseRequested => {
                if self.pets.len() > 1 {
                    self.pets.remove(pet_idx);
                    if pet_idx < self.desired_herd.len() {
                        self.desired_herd.remove(pet_idx);
                    }
                    if pet_idx < self.desired_sizes.len() {
                        self.desired_sizes.remove(pet_idx);
                    }
                    if pet_idx < self.desired_speeds.len() {
                        self.desired_speeds.remove(pet_idx);
                    }
                    self.persist_current_settings();
                } else {
                    event_loop.exit();
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                let now_ms = self.start_time.elapsed().as_millis() as u64;
                let pet = &mut self.pets[pet_idx];
                let px = position.x as f32;
                let py = position.y as f32;
                #[cfg(not(windows))]
                if pet.is_dragging {
                    if let Ok(outer) = pet.window.outer_position() {
                        let new_x = outer.x as f32 + (px - pet.drag_grab_offset.0);
                        let new_y = outer.y as f32 + (py - pet.drag_grab_offset.1);
                        pet.cursor_velocity = (new_x - pet.state.x, new_y - pet.state.y);
                        pet.drag_sample_ms = 16;
                        pet.state.x = new_x;
                        pet.state.y = new_y;
                    }
                }
                if !pet.is_dragging {
                    let dx = px - pet.cursor_pos.0;
                    let dy = py - pet.cursor_pos.1;
                    pet.cursor_velocity = (dx, dy);
                    if now_ms.saturating_sub(pet.last_rub_ms) > 900 {
                        pet.rub_accum = 0.0;
                    }
                    pet.last_rub_ms = now_ms;
                    pet.rub_accum += dx.hypot(dy).min(24.0);
                    if pet.rub_accum > 140.0 {
                        pet.rub_accum = 0.0;
                        pet.heart_until_ms = now_ms + 2000;
                    }
                }
                pet.cursor_pos = (px, py);
            }
            WindowEvent::MouseInput {
                state: ElementState::Released,
                button: MouseButton::Right,
                ..
            } => {
                #[cfg(windows)]
                let can_clone = self.desired_herd.len() < MAX_ACTIVE_PETS;
                #[cfg(windows)]
                let can_dismiss = self.pets.len() > 1;
                #[cfg(not(windows))]
                {
                    platform::spawn_unix_pet_menu(self.agent_tx.clone(), self.follow_cursor);
                    return;
                }
                #[cfg(windows)]
                let cmd = show_pet_context_menu(
                    &self.pets[pet_idx].window,
                    can_clone,
                    can_dismiss,
                    self.follow_cursor,
                );
                #[cfg(windows)]
                {
                let now_ms = self.start_time.elapsed().as_millis() as u64;
                match cmd {
                    1 => {
                        let pet = &mut self.pets[pet_idx];
                        pet.heart_until_ms = now_ms + 2200;
                        pet.state.is_tossing = true;
                        pet.state.toss_force_x = if pet.state.is_moving_left { -3.0 } else { 3.0 };
                        pet.state.toss_vert_vel = -10.0;
                        pet.squash.trigger_hop();
                        self.bubble = SpeechBubbleOverlay::create(event_loop, "<3", now_ms + 2200);
                    }
                    2 => {
                        self.pets[pet_idx]
                            .state
                            .set_state(DeskpetStateId::Custom("sleep".to_string()));
                    }
                    3 if can_clone => {
                        let clone_id = self.pets[pet_idx].pack_id.clone();
                        self.desired_herd.push(clone_id);
                        self.desired_sizes.push(self.pet_size);
                        self.desired_speeds.push(self.walk_speed);
                        self.sync_herd(event_loop, now_ms);
                        self.persist_current_settings();
                    }
                    4 if can_dismiss => {
                        self.pets.remove(pet_idx);
                        if pet_idx < self.desired_herd.len() {
                            self.desired_herd.remove(pet_idx);
                        }
                        if pet_idx < self.desired_sizes.len() {
                            self.desired_sizes.remove(pet_idx);
                        }
                        if pet_idx < self.desired_speeds.len() {
                            self.desired_speeds.remove(pet_idx);
                        }
                        self.persist_current_settings();
                    }
                    5 => Self::spawn_sibling(&mut self.settings_process, "diypets-settings"),
                    6 => Self::spawn_sibling(&mut self.creator_process, "diypets-creator"),
                    7 => self.drop_toy_ball(event_loop, now_ms),
                    8 => {
                        self.follow_cursor = !self.follow_cursor;
                        self.persist_current_settings();
                    }
                    _ => {}
                }
                }
            }
            WindowEvent::MouseInput {
                state,
                button: MouseButton::Left,
                ..
            } => {
                let pet = &mut self.pets[pet_idx];
                if state == ElementState::Pressed {
                    pet.is_dragging = true;
                    pet.drag_grab_offset = pet.cursor_pos;
                    pet.cursor_velocity = (0.0, 0.0);
                    pet.state.is_tossing = false;
                    pet.state.set_state(DeskpetStateId::Drag);
                } else {
                    pet.is_dragging = false;
                    pet.state.release_drag(
                        pet.cursor_velocity.0,
                        pet.cursor_velocity.1,
                        pet.drag_sample_ms,
                    );
                }
                pet.window.request_redraw();
            }
            WindowEvent::RedrawRequested => {
                static FIRST_REDRAW: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
                if !FIRST_REDRAW.swap(true, std::sync::atomic::Ordering::Relaxed) {
                    logging::info(&format!("RedrawRequested fired for pet_idx={pet_idx} (total_pets={})", self.pets.len()));
                }
                let now = self.start_time.elapsed().as_millis() as u64;

                let mut min_x: f32 = 0.0;
                let mut max_x: f32 = 0.0;
                let mut min_y: f32 = 0.0;
                let mut max_y: f32 = 0.0;
                for monitor in event_loop.available_monitors() {
                    let pos = monitor.position();
                    let size = monitor.size();
                    min_x = min_x.min(pos.x as f32);
                    min_y = min_y.min(pos.y as f32);
                    max_x = max_x.max((pos.x + size.width as i32) as f32);
                    max_y = max_y.max((pos.y + size.height as i32) as f32);
                }

                #[cfg(windows)]
                let own_hwnds: Vec<windows::Win32::Foundation::HWND> = {
                    let mut v: Vec<_> = self.pets.iter().filter_map(|p| hwnd_of(&p.window)).collect();
                    if let Some(b) = &self.bubble
                        && let Some(h) = hwnd_of(&b.window)
                    {
                        v.push(h);
                    }
                    if let Some(ball) = &self.ball
                        && let Some(h) = hwnd_of(&ball.window)
                    {
                        v.push(h);
                    }
                    v
                };

                let climb_windows = self.climb_windows;
                let pet = &mut self.pets[pet_idx];

                let delta = now.saturating_sub(pet.last_tick);
                pet.last_tick = now;

                let scaled_w = pet.state.pet_size;
                let scaled_h =
                    scaled_height(pet.state.pack.tile_w, pet.state.pack.tile_h, scaled_w);
                let ground_height = pet.state.ground_height();

                let mut base_floor_y = max_y - ground_height;
                #[cfg(windows)]
                if let Some(rect) = get_work_area() {
                    base_floor_y = (rect.bottom as f32) - ground_height;
                }

                let mut floor_y = pet.cached_floor_y.unwrap_or(base_floor_y);
                let mut wall_left = pet.cached_wall_left.unwrap_or(min_x);
                let mut wall_right = pet.cached_wall_right.unwrap_or(max_x);

                if !climb_windows {
                    floor_y = base_floor_y;
                    wall_left = min_x;
                    wall_right = max_x;
                    pet.cached_floor_y = Some(floor_y);
                    pet.cached_wall_left = Some(wall_left);
                    pet.cached_wall_right = Some(wall_right);
                } else if !pet.is_dragging
                    && (pet.cached_floor_y.is_none()
                        || now.saturating_sub(pet.last_window_scan) >= WINDOW_SCAN_MS)
                {
                    pet.last_window_scan = now;
                    floor_y = base_floor_y;
                    wall_left = min_x;
                    wall_right = max_x;

                    #[cfg(windows)]
                    {
                        use windows::Win32::Foundation::POINT;
                        use windows::Win32::Graphics::Dwm::{DWMWA_CLOAKED, DwmGetWindowAttribute};
                        use windows::Win32::UI::WindowsAndMessaging::{
                            GA_ROOT, GWL_EXSTYLE, GetAncestor, GetWindowLongW, GetWindowRect,
                            IsIconic, WS_EX_TOOLWINDOW, WS_EX_TRANSPARENT, WindowFromPoint,
                        };

                        let center_x = pet.state.x as i32 + (scaled_w / 2) as i32;
                        let feet_y = (pet.state.y + ground_height) as i32 + 2;

                        unsafe {
                            let mut hwnd = WindowFromPoint(POINT {
                                x: center_x,
                                y: feet_y,
                            });

                            if !hwnd.is_invalid() {
                                let root = GetAncestor(hwnd, GA_ROOT);
                                if !root.is_invalid() {
                                    hwnd = root;
                                }

                                let style = GetWindowLongW(hwnd, GWL_EXSTYLE) as u32;
                                let mut cloaked = 0u32;
                                let is_cloaked = DwmGetWindowAttribute(
                                    hwnd,
                                    DWMWA_CLOAKED,
                                    &mut cloaked as *mut _ as _,
                                    std::mem::size_of_val(&cloaked) as u32,
                                )
                                .is_ok()
                                    && cloaked != 0;

                                if !own_hwnds.contains(&hwnd)
                                    && !IsIconic(hwnd).as_bool()
                                    && !is_cloaked
                                    && style & (WS_EX_TRANSPARENT.0 | WS_EX_TOOLWINDOW.0) == 0
                                {
                                    let mut rect = windows::Win32::Foundation::RECT::default();
                                    if GetWindowRect(hwnd, &mut rect).is_ok() {
                                        let win_y = rect.top as f32;
                                        let win_x = rect.left as f32;
                                        let win_h = (rect.bottom - rect.top) as f32;

                                        let mut is_fullscreen = false;
                                        for monitor in event_loop.available_monitors() {
                                            let pos = monitor.position();
                                            let size = monitor.size();
                                            if rect.left <= pos.x
                                                && rect.top <= pos.y
                                                && rect.right >= pos.x + size.width as i32
                                                && rect.bottom >= pos.y + size.height as i32
                                            {
                                                is_fullscreen = true;
                                                break;
                                            }
                                        }

                                        if !is_fullscreen {
                                            if let Some(window_floor) = window_floor_candidate(
                                                win_y,
                                                ground_height,
                                                pet.state.y + ground_height,
                                                min_y,
                                            ) {
                                                let prev_f = floor_y;
                                                floor_y = select_window_floor(
                                                    floor_y,
                                                    Some(window_floor),
                                                );
                                                if floor_y < prev_f - 0.5 {
                                                    let hid = hwnd.0 as isize;
                                                    if let Some((old_h, ox, oy)) =
                                                        pet.standing_window
                                                        && old_h == hid {
                                                            let dx = rect.left - ox;
                                                            let dy = rect.top - oy;
                                                            if dx.abs() < 300 && dy.abs() < 300 {
                                                                pet.state.x += dx as f32;
                                                                pet.state.y += dy as f32;
                                                            }
                                                        }
                                                    pet.standing_window =
                                                        Some((hid, rect.left, rect.top));
                                                }
                                            }

                                            if win_y > 10.0
                                                && pet.state.y + (scaled_h as f32) > win_y
                                                && pet.state.y < win_y + win_h
                                                && pet.state.x < win_x
                                            {
                                                wall_right =
                                                    wall_right.min(win_x - (scaled_w as f32));
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }

                    #[cfg(not(windows))]
                    {
                        let center_x = pet.state.x as i32 + (scaled_w / 2) as i32;
                        let feet_y = (pet.state.y + ground_height) as i32 + 2;
                        let mons: Vec<(i32, i32, i32, i32)> = event_loop
                            .available_monitors()
                            .map(|m| {
                                let p = m.position();
                                let s = m.size();
                                (p.x, p.y, p.x + s.width as i32, p.y + s.height as i32)
                            })
                            .collect();
                        if let Some(wrect) = platform::find_unix_window_under_point(center_x, feet_y, &mons) {
                            let win_y = wrect.top as f32;
                            let win_x = wrect.left as f32;
                            let win_h = (wrect.bottom - wrect.top) as f32;
                            if let Some(window_floor) = window_floor_candidate(
                                win_y,
                                ground_height,
                                pet.state.y + ground_height,
                                min_y,
                            ) {
                                let prev_f = floor_y;
                                floor_y = select_window_floor(floor_y, Some(window_floor));
                                if floor_y < prev_f - 0.5 {
                                    if let Some((old_h, ox, oy)) = pet.standing_window {
                                        if old_h == wrect.id {
                                            let dx = wrect.left - ox;
                                            let dy = wrect.top - oy;
                                            if dx.abs() < 300 && dy.abs() < 300 {
                                                pet.state.x += dx as f32;
                                                pet.state.y += dy as f32;
                                            }
                                        }
                                    }
                                    pet.standing_window = Some((wrect.id, wrect.left, wrect.top));
                                }
                            }
                            if win_y > 10.0
                                && pet.state.y + (scaled_h as f32) > win_y
                                && pet.state.y < win_y + win_h
                                && pet.state.x < win_x
                            {
                                wall_right = wall_right.min(win_x - (scaled_w as f32));
                            }
                        }
                    }

                    pet.cached_floor_y = Some(floor_y);
                    pet.cached_wall_left = Some(wall_left);
                    pet.cached_wall_right = Some(wall_right);
                }

                if pet.is_dragging {
                    if let Some((gx, gy)) = platform::global_cursor_pos() {
                        let new_x = gx - (scaled_w as f32 / 2.0);
                        let new_y = gy - 10.0;
                        pet.cursor_velocity = (new_x - pet.state.x, new_y - pet.state.y);
                        pet.drag_sample_ms = delta.max(1);
                        pet.state.x = new_x;
                        pet.state.y = new_y;
                    }
                } else {
                    pet.state.climb_enabled = climb_windows;
                    pet.state.on_window = floor_y < base_floor_y - 3.0;
                    pet.state.screen_w = max_x - min_x;
                    pet.state.screen_h = max_y - min_y;
                    pet.state.area_h = floor_y + ground_height;
                    pet.state.tick_in_bounds(delta, floor_y, wall_left, wall_right);
                    let now_airborne = pet.state.y < floor_y - 3.0;
                    if pet.was_airborne && !now_airborne {
                        pet.squash.trigger_land();
                    }
                    pet.was_airborne = now_airborne;
                }
                let is_idle_or_sleep = matches!(pet.state.current_anim_id.as_str(), "2" | "12" | "13");
                let (sq_x, sq_y) = pet.squash.step(delta as f32 / 1000.0, now as f32 / 1000.0, is_idle_or_sleep);

                let frame_idx = pet.state.current_frame().unwrap_or(0) as usize;
                let pos_x = pet.state.x.round() as i32;
                let pos_y = pet.state.y.round() as i32;
                let render_key = RenderKey {
                    frame_idx,
                    moving_left: pet.state.is_moving_left,
                    opacity_u8: (pet.state.opacity.clamp(0.0, 1.0) * 255.0).round() as u8,
                    x: pos_x,
                    y: pos_y,
                    dst_w: scaled_w,
                    dst_h: scaled_h,
                    heart_tick: if now < pet.heart_until_ms {
                        ((now / 70) % 255) as u8
                    } else {
                        0
                    },
                    squash_x_q: (sq_x * 100.0).round() as u16,
                    squash_y_q: (sq_y * 100.0).round() as u16,
                };

                if pet.last_render == Some(render_key) {
                    return;
                }
                pet.last_render = Some(render_key);

                pet.window
                    .set_outer_position(PhysicalPosition::new(pos_x, pos_y));

                let w = pet.state.pack.tile_w as usize;
                let h = pet.state.pack.tile_h as usize;

                #[cfg(not(windows))]
                if let Some(surface) = &mut pet.surface {
                    let mut buffer = surface.buffer_mut().unwrap();
                    buffer.fill(0);
                    if frame_idx < pet.state.pack.frames.len() {
                        let frame_data = &pet.state.pack.frames[frame_idx];
                        let dst_w = scaled_w as usize;
                        let dst_h = scaled_h as usize;
                        for y in 0..dst_h {
                            let src_y = (y * h) / dst_h;
                            for x in 0..dst_w {
                                let i = y * dst_w + x;
                                if i >= buffer.len() {
                                    break;
                                }
                                let base_src_x = (x * w) / dst_w;
                                let src_x = if pet.state.is_moving_left {
                                    base_src_x
                                } else {
                                    w.saturating_sub(1) - base_src_x
                                };
                                let src_i = src_y * w + src_x;
                                if src_i >= frame_data.len() {
                                    continue;
                                }
                                let mut pixel = frame_data[src_i];
                                if pet.state.opacity < 1.0 {
                                    let a = (pixel >> 24) & 0xFF;
                                    let r = (pixel >> 16) & 0xFF;
                                    let g = (pixel >> 8) & 0xFF;
                                    let b = pixel & 0xFF;
                                    let new_a = (a as f32 * pet.state.opacity) as u32;
                                    let pr = (r as f32 * pet.state.opacity) as u32;
                                    let pg = (g as f32 * pet.state.opacity) as u32;
                                    let pb = (b as f32 * pet.state.opacity) as u32;
                                    pixel = (new_a << 24) | (pr << 16) | (pg << 8) | pb;
                                }
                                buffer[i] = pixel;
                            }
                        }
                    }
                    if now < pet.heart_until_ms {
                        let rem = pet.heart_until_ms.saturating_sub(now) as f32;
                        let prog = (1.0 - (rem / 2200.0)).clamp(0.0, 1.0);
                        stamp_hearts_argb(&mut buffer, scaled_w, scaled_h, prog);
                    }
                    buffer.present().unwrap();
                }

                #[cfg(windows)]
                {
                    let cache_key = (frame_idx, pet.state.is_moving_left, scaled_w, scaled_h);
                    let use_cache = pet.state.opacity >= 0.99;
                    if !use_cache || !pet.frame_cache.contains_key(&cache_key) {
                        pet.buffer.fill(0);
                        if frame_idx < pet.state.pack.frames.len() {
                            let frame_data = &pet.state.pack.frames[frame_idx];
                            for y in 0..h {
                                for x in 0..w {
                                    let i = y * w + x;
                                    if i >= pet.buffer.len() {
                                        break;
                                    }
                                    let src_x = if !pet.state.is_moving_left {
                                        w.saturating_sub(1) - x
                                    } else {
                                        x
                                    };
                                    let src_i = y * w + src_x;
                                    if src_i >= frame_data.len() {
                                        continue;
                                    }
                                    let mut pixel = frame_data[src_i];
                                    if pet.state.opacity < 1.0 {
                                        let a = (pixel >> 24) & 0xFF;
                                        let r = (pixel >> 16) & 0xFF;
                                        let g = (pixel >> 8) & 0xFF;
                                        let b = pixel & 0xFF;
                                        let new_a = (a as f32 * pet.state.opacity) as u32;
                                        let pr = (r as f32 * pet.state.opacity) as u32;
                                        let pg = (g as f32 * pet.state.opacity) as u32;
                                        let pb = (b as f32 * pet.state.opacity) as u32;
                                        pixel = (new_a << 24) | (pr << 16) | (pg << 8) | pb;
                                    }
                                    pet.buffer[i] = pixel;
                                }
                            }
                        }
                        scale_nearest_into(
                            &pet.buffer,
                            pet.state.pack.tile_w,
                            pet.state.pack.tile_h,
                            scaled_w,
                            scaled_h,
                            &mut pet.scaled_buf,
                        );
                        if use_cache {
                            if pet.frame_cache.len() > 128 {
                                pet.frame_cache.clear();
                            }
                            pet.frame_cache.insert(cache_key, pet.scaled_buf.clone());
                        }
                    }

                    let show_hearts = now < pet.heart_until_ms;
                    let present_buf = if show_hearts {
                        if use_cache
                            && let Some(cached) = pet.frame_cache.get(&cache_key) {
                                pet.scaled_buf.clear();
                                pet.scaled_buf.extend_from_slice(cached);
                            }
                        let rem = pet.heart_until_ms.saturating_sub(now) as f32;
                        let prog = (1.0 - (rem / 2200.0)).clamp(0.0, 1.0);
                        stamp_hearts_argb(&mut pet.scaled_buf, scaled_w, scaled_h, prog);
                        &pet.scaled_buf
                    } else if use_cache {
                        pet.frame_cache
                            .get(&cache_key)
                            .map(Vec::as_slice)
                            .unwrap_or(&pet.scaled_buf)
                    } else {
                        &pet.scaled_buf
                    };

                    let needs_recreate = match &pet.layered {
                        Some(l) => l.width != scaled_w || l.height != scaled_h,
                        None => true,
                    };
                    if needs_recreate {
                        pet.layered = LayeredSurface::new(scaled_w, scaled_h);
                    }
                    let final_buf = if (sq_x - 1.0).abs() >= 0.006 || (sq_y - 1.0).abs() >= 0.006 {
                        apply_squash_nearest(present_buf, &mut pet.squash_buf, scaled_w, scaled_h, sq_x, sq_y);
                        pet.squash_buf.as_slice()
                    } else {
                        present_buf
                    };
                    if let Some(layered) = &mut pet.layered {
                        layered.present(&pet.window, final_buf, pos_x, pos_y);
                    }
                }
            }
            _ => {}
        }
    }
}

impl Drop for DeskpetApp {
    fn drop(&mut self) {
        if let Some(mut child) = self.settings_process.take() {
            let _ = child.kill();
        }
        if let Some(mut child) = self.creator_process.take() {
            let _ = child.kill();
        }
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.get(1).map(String::as_str) != Some("mcp")
        && logging::init("diypets").is_none() { eprintln!("diypets: warning: could not initialize file logger"); }
    if matches!(args.get(1).map(String::as_str), Some("--version" | "-V" | "version")) {
        println!(
            "diypets v{} ({}, {}, {})",
            env!("CARGO_PKG_VERSION"),
            env!("DIYPETS_GIT_HASH"),
            env!("DIYPETS_TARGET"),
            env!("DIYPETS_PROFILE")
        );
        return;
    }
    if args.get(1).map(String::as_str) == Some("doctor") {
        let cfg_path = Settings::default_path();
        let settings = Settings::load_sanitized(&cfg_path);
        println!("DIYPets Doctor v{} ({})", env!("CARGO_PKG_VERSION"), env!("DIYPETS_GIT_HASH"));
        println!("  Target / Profile : {} / {}", env!("DIYPETS_TARGET"), env!("DIYPETS_PROFILE"));
        println!("  Config Path      : {} (exists={})", cfg_path.display(), cfg_path.is_file());
        println!("  Log Directory    : {}", logging::log_dir().display());
        println!("  Active Herd      : {:?} (extra={:?})", settings.pet_pack, settings.extra_pets);
        println!("  Size / Speed     : {}px / {}", settings.pet_size, settings.walk_speed);
        println!("  Click-Through    : {} | Auto-Hide Fullscreen: {}", settings.click_through, settings.auto_hide_fullscreen);
        return;
    }
    if args.get(1).map(String::as_str) == Some("hooks") {
        let action = args.get(2).map(String::as_str).unwrap_or("status");
        let is_project = args.iter().any(|a| a == "--project");
        let scope = if is_project {
            diypets::agent_hooks::HookScope::Project
        } else {
            diypets::agent_hooks::HookScope::User
        };
        match action {
            "install" => {
                let target_arg = args.get(3).map(String::as_str).unwrap_or("all");
                match diypets::agent_hooks::AgentTarget::parse_targets(target_arg) {
                    Ok(targets) => {
                        for t in targets {
                            match diypets::agent_hooks::install_hooks(t, scope) {
                                Ok((path, n)) => println!("Installed {n} DIYPets hooks for {} -> {}", t.label(), path.display()),
                                Err(e) => eprintln!("Failed to install {} hooks: {e}", t.label()),
                            }
                        }
                    }
                    Err(e) => eprintln!("{e}"),
                }
            }
            "uninstall" | "remove" => {
                let target_arg = args.get(3).map(String::as_str).unwrap_or("all");
                match diypets::agent_hooks::AgentTarget::parse_targets(target_arg) {
                    Ok(targets) => {
                        for t in targets {
                            match diypets::agent_hooks::uninstall_hooks(t, scope) {
                                Ok((path, n)) => println!("Removed {n} DIYPets hooks for {} from {}", t.label(), path.display()),
                                Err(e) => eprintln!("Failed to uninstall {} hooks: {e}", t.label()),
                            }
                        }
                    }
                    Err(e) => eprintln!("{e}"),
                }
            }
            "status" => {
                let scope_label = if is_project { "project" } else { "user" };
                println!("DIYPets Coding-Agent Hooks ({scope_label} scope):");
                for (target, path, count) in diypets::agent_hooks::status_report(scope) {
                    println!("  {:<12}: {} hook(s) ({})", target.label(), count, path.display());
                }
            }
            _ => {
                println!("Usage:");
                println!("  diypets hooks install <claude-code|cursor|all> [--project]");
                println!("  diypets hooks uninstall <claude-code|cursor|all> [--project]");
                println!("  diypets hooks status [--project]");
            }
        }
        return;
    }
    if args.get(1).map(String::as_str) == Some("hook-event") {
        let agent = args.get(2).map(String::as_str).unwrap_or("claude-code");
        let event = args.get(3).map(String::as_str).unwrap_or("");
        let mut stdin_buf = String::new();
        if !std::io::IsTerminal::is_terminal(&std::io::stdin()) {
            use std::io::Read;
            if let Err(e) = std::io::stdin().take(4096).read_to_string(&mut stdin_buf) { logging::warn(&format!("hook-event: stdin read failed: {e}")); }
        }
        if let Some(reaction) = diypets::agent_hooks::map_hook_event(agent, event, &stdin_buf) {
            diypets::agent_hooks::send_hook_reaction_nonblocking(&reaction);
        }
        // Always exit 0 so hooks never block or fail a coding agent.
        return;
    }
    if args.get(1).map(String::as_str) == Some("export-gif") {
        let xml_arg = args.get(2).map(String::as_str);
        let out_arg = args.get(3).map(String::as_str).unwrap_or("diypets_preview.gif");
        let Some(xml_path) = xml_arg else {
            eprintln!("Usage: diypets export-gif <path/to/deskpet.xml> [output.gif] [--card]");
            return;
        };
        let is_card = args.iter().any(|a| a == "--card");
        let cfg = if is_card {
            diypets::gif_export::GifExportConfig::social_card(2)
        } else {
            diypets::gif_export::GifExportConfig::transparent(2)
        };
        match std::fs::read_to_string(xml_path)
            .map_err(|e| format!("Failed to read {xml_path}: {e}"))
            .and_then(|xml| diypets::gif_export::export_xml_pack_gif(&xml, "1", &cfg))
            .and_then(|bytes| std::fs::write(out_arg, bytes).map_err(|e| format!("Write failed: {e}")))
        {
            Ok(()) => println!("Exported animated GIF -> {out_arg}"),
            Err(e) => eprintln!("GIF export failed: {e}"),
        }
        return;
    }
    if args.get(1).map(String::as_str) == Some("mcp") {
        agent_api::run_stdio_mcp_bridge();
        return;
    }
    if args.get(1).map(String::as_str) == Some("say") {
        let msg = args[2..].join(" ");
        let msg = if msg.trim().is_empty() { "Hello!".to_string() } else { msg };
        match agent_api::send_cli_react("say", Some(&msg)) {
            Ok(_) => println!("DIYPets said: {msg}"),
            Err(e) => {
                eprintln!("{e}");
                std::process::exit(1);
            }
        }
        return;
    }
    if args.get(1).map(String::as_str) == Some("ball") {
        match agent_api::send_cli_react("ball", Some("Play time!")) {
            Ok(_) => println!("Dropped bouncy ball for DIYPets herd!"),
            Err(e) => {
                eprintln!("{e}");
                std::process::exit(1);
            }
        }
        return;
    }
    if args.get(1).map(String::as_str) == Some("react") {
        let mood = args
            .iter()
            .position(|a| a == "--mood")
            .and_then(|i| args.get(i + 1))
            .map(String::as_str)
            .unwrap_or("happy");
        let text = args
            .iter()
            .position(|a| a == "--text" || a == "--bubble")
            .and_then(|i| args.get(i + 1))
            .map(String::as_str);
        match agent_api::send_cli_react(mood, text) {
            Ok(_) => println!("DIYPets reacted: {mood}"),
            Err(e) => {
                eprintln!("{e}");
                std::process::exit(1);
            }
        }
        return;
    }

    if agent_api::send_cli_react("hop", Some("Already running!")).is_ok() {
        return;
    }

    let (agent_tx, agent_rx) = mpsc::channel();
    let shared_herd: SharedHerd = Arc::new(Mutex::new(Vec::new()));
    agent_api::start_local_server(agent_tx.clone(), shared_herd.clone());
    platform::start_unix_window_poller();

    #[cfg(windows)]
    unsafe {
        let _ = windows::Win32::UI::WindowsAndMessaging::SetProcessDPIAware();
    }

    let settings_path = Settings::default_path();
    let settings = Settings::load_sanitized(&settings_path);
    let last_settings_mod = std::fs::metadata(&settings_path)
        .and_then(|m| m.modified())
        .ok();
    let herd = settings.active_herd();
    let desired_sizes = (0..herd.len()).map(|i| settings.slot_pet_size(i)).collect();
    let desired_speeds = (0..herd.len()).map(|i| settings.slot_walk_speed(i)).collect();

    let mut app = DeskpetApp {
        pets: Vec::new(),
        desired_herd: herd,
        desired_sizes,
        desired_speeds,
        pet_size: settings.pet_size,
        walk_speed: settings.walk_speed,
        is_topmost: settings.always_on_top,
        climb_windows: settings.climb_windows,
        click_through: settings.click_through,
        start_on_login: autostart::is_enabled() || settings.start_on_login,
        follow_cursor: settings.follow_cursor,
        auto_hide_fullscreen: settings.auto_hide_fullscreen,
        quiet_mode: false,
        fullscreen_hidden: false,
        fullscreen_streak: 0,
        last_visible: true,
        last_fullscreen_check: 0,
        settings_path,
        last_settings_mod,
        start_time: Instant::now(),
        next_frame: Instant::now(),
        last_settings_poll: 0,
        tray: None,
        settings_process: None,
        creator_process: None,
        agent_tx,
        agent_rx,
        shared_herd,
        bubble: None,
        ball: None,
    };

    let event_loop = EventLoop::new().unwrap();
    if let Err(e) = event_loop.run_app(&mut app) { logging::error(&format!("runner: event loop exited with error: {e}")); }
}

#[cfg(test)]
mod layered_style_tests {
    use super::sanitize_layered_window_styles;

    #[test]
    fn layered_window_styles_strip_chrome_and_preserve_layered_flag() {
        let (s_top, ex_top) = sanitize_layered_window_styles(0x00CF_0000, 0x0020_0020, true);
        assert_eq!(s_top & 0x00CF_0000, 0);
        assert_ne!(s_top & 0x8000_0000, 0);
        assert_ne!(ex_top & 0x0008_0000, 0);
        assert_eq!(ex_top & 0x0020_0000, 0);
        assert_eq!(ex_top & 0x0000_0020, 0);
        assert_ne!(ex_top & 0x0000_0008, 0);

        let (_, ex_no_top) = sanitize_layered_window_styles(0x00CF_0000, 0x0020_0020, false);
        assert_eq!(ex_no_top & 0x0000_0008, 0);
    }
}
