//! DIYPets Studio V2 Core Model, Seamless Compositor & Exporter.
//!
//! Features:
//! - Unified seamless silhouette outlining (zero cut-out shoulder/hip seams!)
//! - 5 creature archetypes (Quadruped, Biped, Bird, Crawler/Snake, Swimmer/Fish)
//! - Unlimited attachable limbs/wings/tails/ears/props with 2.5D far-side shading
//! - 5 switchable face expressions (Neutral, Blink, Happy, Sleep, Surprised)
//! - Custom keyframed animations + validated eSheep/DIYPets XML export

use crate::catalog::strip_chroma_key_if_opaque;
use crate::deskpet_xml::DeskpetPack;
use base64::{Engine as _, engine::general_purpose::STANDARD};
use serde::{Deserialize, Serialize};
use std::io::Cursor;
use std::path::Path;
use tiny_skia::{ColorU8, Pixmap, PixmapPaint, Transform};

pub const BOARD_SIZE: usize = 128;
pub const EXPORT_TILE: u32 = 128;
pub const BOARD_LEN: usize = BOARD_SIZE * BOARD_SIZE;

pub type RgbaPixel = [u8; 4];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Archetype {
    Quadruped,
    Biped,
    Bird,
    Crawler,
    Swimmer,
}

impl Archetype {
    pub fn all() -> &'static [Self] {
        &[
            Self::Quadruped,
            Self::Biped,
            Self::Bird,
            Self::Crawler,
            Self::Swimmer,
        ]
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Quadruped => "Quadruped (4-Leg)",
            Self::Biped => "Biped (2-Leg)",
            Self::Bird => "Bird / Flyer",
            Self::Crawler => "Crawler (Snake)",
            Self::Swimmer => "Swimmer (Fish)",
        }
    }

    pub fn from_str_loose(s: &str) -> Option<Self> {
        match s.trim().to_lowercase().as_str() {
            "quadruped" | "cat" | "dog" | "cow" | "4leg" => Some(Self::Quadruped),
            "biped" | "human" | "buddy" | "chibi" | "2leg" => Some(Self::Biped),
            "bird" | "flyer" | "flying" | "owl" | "duck" => Some(Self::Bird),
            "crawler" | "snake" | "worm" | "caterpillar" | "slither" => Some(Self::Crawler),
            "swimmer" | "fish" | "whale" | "shark" | "jellyfish" | "swim" => Some(Self::Swimmer),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PartRole {
    FrontLimb,
    BackLimb,
    Wing,
    Tail,
    Ear,
    Extra,
}

impl PartRole {
    pub fn all() -> &'static [Self] {
        &[
            Self::FrontLimb,
            Self::BackLimb,
            Self::Wing,
            Self::Tail,
            Self::Ear,
            Self::Extra,
        ]
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::FrontLimb => "Front Limb / Arm",
            Self::BackLimb => "Back Limb / Leg",
            Self::Wing => "Wing",
            Self::Tail => "Tail / Fin",
            Self::Ear => "Ear / Antenna",
            Self::Extra => "Extra Part / Prop",
        }
    }

    pub fn short_tag(self) -> &'static str {
        match self {
            Self::FrontLimb => "FL",
            Self::BackLimb => "BL",
            Self::Wing => "WG",
            Self::Tail => "TL",
            Self::Ear => "ER",
            Self::Extra => "EX",
        }
    }

    pub fn pin_rgb(self) -> [u8; 3] {
        match self {
            Self::FrontLimb => [245, 86, 86],
            Self::BackLimb => [76, 178, 255],
            Self::Wing => [246, 198, 68],
            Self::Tail => [80, 224, 130],
            Self::Ear => [240, 108, 212],
            Self::Extra => [164, 130, 250],
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FaceSlot {
    Neutral,
    Blink,
    Happy,
    Sleep,
    Surprised,
}

impl FaceSlot {
    pub fn all() -> &'static [Self] {
        &[
            Self::Neutral,
            Self::Blink,
            Self::Happy,
            Self::Sleep,
            Self::Surprised,
        ]
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Neutral => "Neutral",
            Self::Blink => "Blink",
            Self::Happy => "Happy",
            Self::Sleep => "Sleep",
            Self::Surprised => "Surprised",
        }
    }

    pub fn index(self) -> usize {
        match self {
            Self::Neutral => 0,
            Self::Blink => 1,
            Self::Happy => 2,
            Self::Sleep => 3,
            Self::Surprised => 4,
        }
    }
}

#[derive(Debug, Clone)]
pub struct RigPart {
    pub name: String,
    pub role: PartRole,
    pub visible: bool,
    pub paired: bool,
    pub mirror_far: bool,
    pub behind_body: bool,
    pub socket_near: (usize, usize),
    pub socket_far: (usize, usize),
    pub pivot: (usize, usize),
    pub swing_deg: f32,
    pub phase_offset_deg: f32,
    pub scale: f32,
    pub pixels: Vec<RgbaPixel>,
}

impl RigPart {
    pub fn new(name: impl Into<String>, role: PartRole) -> Self {
        let paired = matches!(role, PartRole::FrontLimb | PartRole::BackLimb | PartRole::Wing);
        let behind_body = matches!(role, PartRole::Tail);
        let (swing_deg, phase_offset_deg) = match role {
            PartRole::FrontLimb => (20.0, 0.0),
            PartRole::BackLimb => (19.0, 180.0),
            PartRole::Wing => (34.0, 0.0),
            PartRole::Tail => (15.0, 90.0),
            PartRole::Ear => (8.0, 45.0),
            PartRole::Extra => (10.0, 0.0),
        };
        Self {
            name: name.into(),
            role,
            visible: true,
            paired,
            mirror_far: false,
            behind_body,
            socket_near: (64, 72),
            socket_far: (58, 70),
            pivot: (64, 68),
            swing_deg,
            phase_offset_deg,
            scale: 1.0,
            pixels: vec![[0, 0, 0, 0]; BOARD_LEN],
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomKeyframe {
    pub limb_swing: f32,
    pub wing_flap: f32,
    pub tail_wag: f32,
    pub body_dy: f32,
    pub body_tilt_deg: f32,
    pub torso_scale_y: f32,
    pub wave_phase: f32,
    pub face: FaceSlot,
}

impl Default for CustomKeyframe {
    fn default() -> Self {
        Self {
            limb_swing: 0.0,
            wing_flap: 0.0,
            tail_wag: 0.0,
            body_dy: 0.0,
            body_tilt_deg: 0.0,
            torso_scale_y: 1.0,
            wave_phase: 0.0,
            face: FaceSlot::Neutral,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomAnimation {
    pub name: String,
    pub interval_ms: u32,
    pub step_x: i32,
    pub step_y: i32,
    pub repeat: u32,
    pub probability: u32,
    pub keyframes: Vec<CustomKeyframe>,
}

impl CustomAnimation {
    pub fn hop_preset() -> Self {
        Self {
            name: "Happy Hop".to_string(),
            interval_ms: 110,
            step_x: -4,
            step_y: 0,
            repeat: 2,
            probability: 12,
            keyframes: vec![
                CustomKeyframe {
                    body_dy: 2.0,
                    torso_scale_y: 0.90,
                    tail_wag: 0.6,
                    face: FaceSlot::Happy,
                    ..Default::default()
                },
                CustomKeyframe {
                    limb_swing: 0.75,
                    wing_flap: 1.0,
                    tail_wag: -0.6,
                    body_dy: -10.0,
                    body_tilt_deg: -5.0,
                    torso_scale_y: 1.06,
                    face: FaceSlot::Happy,
                    ..Default::default()
                },
                CustomKeyframe {
                    limb_swing: -0.5,
                    wing_flap: -0.5,
                    tail_wag: 0.4,
                    body_dy: -4.0,
                    body_tilt_deg: 2.5,
                    face: FaceSlot::Happy,
                    ..Default::default()
                },
                CustomKeyframe {
                    body_dy: 1.5,
                    torso_scale_y: 0.94,
                    face: FaceSlot::Blink,
                    ..Default::default()
                },
            ],
        }
    }

    pub fn dance_preset() -> Self {
        Self {
            name: "Wiggle Dance".to_string(),
            interval_ms: 130,
            step_x: 0,
            step_y: 0,
            repeat: 3,
            probability: 10,
            keyframes: vec![
                CustomKeyframe {
                    limb_swing: 0.7,
                    wing_flap: 0.8,
                    tail_wag: 1.0,
                    body_dy: -2.0,
                    body_tilt_deg: -6.0,
                    wave_phase: 0.8,
                    face: FaceSlot::Happy,
                    ..Default::default()
                },
                CustomKeyframe {
                    limb_swing: -0.7,
                    wing_flap: -0.8,
                    tail_wag: -1.0,
                    body_dy: 1.0,
                    body_tilt_deg: 6.0,
                    wave_phase: 2.4,
                    face: FaceSlot::Happy,
                    ..Default::default()
                },
            ],
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct PoseParams {
    pub phase_rad: f32,
    pub limb_amp: f32,
    pub wing_amp: f32,
    pub tail_amp: f32,
    pub ear_amp: f32,
    /// Positive = clockwise (limb swings LEFT/forward); Negative = CCW (limb swings RIGHT/backward).
    pub front_bias_deg: f32,
    pub back_bias_deg: f32,
    pub whole_dy: f32,
    pub torso_dy: f32,
    pub torso_scale_y: f32,
    pub body_tilt_deg: f32,
    pub rotate_whole_deg: f32,
    pub wave_amp_px: f32,
    pub wave_phase: f32,
    pub face: FaceSlot,
    pub zzz_phase: Option<f32>,
}

impl Default for PoseParams {
    fn default() -> Self {
        Self {
            phase_rad: 0.0,
            limb_amp: 0.0,
            wing_amp: 0.0,
            tail_amp: 0.3,
            ear_amp: 0.2,
            front_bias_deg: 0.0,
            back_bias_deg: 0.0,
            whole_dy: 0.0,
            torso_dy: 0.0,
            torso_scale_y: 1.0,
            body_tilt_deg: 0.0,
            rotate_whole_deg: 0.0,
            wave_amp_px: 0.0,
            wave_phase: 0.0,
            face: FaceSlot::Neutral,
            zzz_phase: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreviewClip {
    Walk,
    Idle,
    Sit,
    Sleep,
    Drag,
    Fall,
    Climb,
    Custom(usize),
}

#[derive(Debug, Clone)]
pub struct StudioProject {
    pub display_name: String,
    pub pack_id: String,
    pub author: String,
    pub archetype: Archetype,
    pub shade_far_limbs: bool,
    /// Composite body + near limbs seamlessly and wrap the outer silhouette with a clean outline!
    pub seamless_outline: bool,
    pub outline_color: [u8; 3],
    pub global_limb_scale: f32,
    pub global_swing_mult: f32,
    pub body_bob: f32,
    pub wave_amplitude: f32,
    pub walk_step_px: i32,
    pub frame_interval_ms: u32,
    pub body: Vec<RgbaPixel>,
    pub parts: Vec<RigPart>,
    pub faces: [Vec<RgbaPixel>; 5],
    pub custom_animations: Vec<CustomAnimation>,
}

impl Default for StudioProject {
    fn default() -> Self {
        Self::template_cat()
    }
}

pub fn sanitize_pack_id(input: &str) -> String {
    let mut out = String::new();
    let mut prev_u = false;
    for ch in input.trim().chars() {
        let low = ch.to_ascii_lowercase();
        if low.is_ascii_alphanumeric() {
            out.push(low);
            prev_u = false;
        } else if (ch == '_' || ch == '-' || ch.is_whitespace()) && !out.is_empty() && !prev_u {
            out.push('_');
            prev_u = true;
        }
    }
    let t = out.trim_matches('_').to_string();
    if t.is_empty() { "my_pet".to_string() } else { t }
}

pub fn paint_ellipse(buf: &mut [RgbaPixel], cx: f32, cy: f32, rx: f32, ry: f32, color: RgbaPixel) {
    let x0 = ((cx - rx).floor() as isize).max(0) as usize;
    let x1 = ((cx + rx).ceil() as isize).min(BOARD_SIZE as isize - 1) as usize;
    let y0 = ((cy - ry).floor() as isize).max(0) as usize;
    let y1 = ((cy + ry).ceil() as isize).min(BOARD_SIZE as isize - 1) as usize;
    for y in y0..=y1 {
        for x in x0..=x1 {
            let dx = (x as f32 - cx) / rx.max(0.5);
            let dy = (y as f32 - cy) / ry.max(0.5);
            if dx * dx + dy * dy <= 1.0 {
                buf[y * BOARD_SIZE + x] = color;
            }
        }
    }
}

/// Paints an ellipse ONLY where the buffer is already opaque (prevents stripes/spots from poking outside the body!).
pub fn paint_ellipse_clipped(
    buf: &mut [RgbaPixel],
    cx: f32,
    cy: f32,
    rx: f32,
    ry: f32,
    color: RgbaPixel,
) {
    let x0 = ((cx - rx).floor() as isize).max(0) as usize;
    let x1 = ((cx + rx).ceil() as isize).min(BOARD_SIZE as isize - 1) as usize;
    let y0 = ((cy - ry).floor() as isize).max(0) as usize;
    let y1 = ((cy + ry).ceil() as isize).min(BOARD_SIZE as isize - 1) as usize;
    for y in y0..=y1 {
        for x in x0..=x1 {
            let idx = y * BOARD_SIZE + x;
            if buf[idx][3] == 0 {
                continue;
            }
            let dx = (x as f32 - cx) / rx.max(0.5);
            let dy = (y as f32 - cy) / ry.max(0.5);
            if dx * dx + dy * dy <= 1.0 {
                buf[idx] = color;
            }
        }
    }
}

pub fn paint_triangle(
    buf: &mut [RgbaPixel],
    a: (f32, f32),
    b: (f32, f32),
    c: (f32, f32),
    color: RgbaPixel,
) {
    let min_x = a.0.min(b.0).min(c.0).floor().max(0.0) as usize;
    let max_x = a.0.max(b.0).max(c.0).ceil().min((BOARD_SIZE - 1) as f32) as usize;
    let min_y = a.1.min(b.1).min(c.1).floor().max(0.0) as usize;
    let max_y = a.1.max(b.1).max(c.1).ceil().min((BOARD_SIZE - 1) as f32) as usize;
    let edge = |p0: (f32, f32), p1: (f32, f32), p: (f32, f32)| {
        (p.0 - p0.0) * (p1.1 - p0.1) - (p.1 - p0.1) * (p1.0 - p0.0)
    };
    for y in min_y..=max_y {
        for x in min_x..=max_x {
            let pt = (x as f32 + 0.5, y as f32 + 0.5);
            let w0 = edge(a, b, pt);
            let w1 = edge(b, c, pt);
            let w2 = edge(c, a, pt);
            if (w0 >= 0.0 && w1 >= 0.0 && w2 >= 0.0) || (w0 <= 0.0 && w1 <= 0.0 && w2 <= 0.0) {
                buf[y * BOARD_SIZE + x] = color;
            }
        }
    }
}

/// Adds subtle underside cel-shading to give flat fills rich 2D sprite volume.
pub fn paint_soft_underside_shade(buf: &mut [RgbaPixel], band_px: usize, factor: f32) {
    let src = buf.to_vec();
    for x in 0..BOARD_SIZE {
        let mut bottom = None;
        for y in (0..BOARD_SIZE).rev() {
            if src[y * BOARD_SIZE + x][3] > 200 {
                bottom = Some(y);
                break;
            }
        }
        if let Some(by) = bottom {
            let start = by.saturating_sub(band_px);
            for y in start..=by {
                let idx = y * BOARD_SIZE + x;
                let p = src[idx];
                if p[3] > 200 {
                    buf[idx] = [
                        ((p[0] as f32) * factor).round().clamp(0.0, 255.0) as u8,
                        ((p[1] as f32) * factor).round().clamp(0.0, 255.0) as u8,
                        ((p[2] as f32) * factor).round().clamp(0.0, 255.0) as u8,
                        p[3],
                    ];
                }
            }
        }
    }
}

pub fn paint_outline(buf: &mut [RgbaPixel], outline: RgbaPixel) {
    let src = buf.to_vec();
    for y in 1..(BOARD_SIZE - 1) {
        for x in 1..(BOARD_SIZE - 1) {
            let idx = y * BOARD_SIZE + x;
            if src[idx][3] > 0 {
                continue;
            }
            if src[idx - 1][3] > 0
                || src[idx + 1][3] > 0
                || src[idx - BOARD_SIZE][3] > 0
                || src[idx + BOARD_SIZE][3] > 0
            {
                buf[idx] = outline;
            }
        }
    }
}

/// Apply a crisp silhouette outline directly around opaque regions of a tiny-skia `Pixmap`.
fn outline_pixmap_silhouette(pm: &mut Pixmap, rgb: [u8; 3], thickness: usize) {
    let w = pm.width() as usize;
    let h = pm.height() as usize;
    let outline_px = ColorU8::from_rgba(rgb[0], rgb[1], rgb[2], 255).premultiply();

    for _ in 0..thickness {
        let alpha_mask: Vec<bool> = pm.pixels().iter().map(|p| p.alpha() >= 110).collect();
        let pixels = pm.pixels_mut();
        for y in 1..(h - 1) {
            let row = y * w;
            for x in 1..(w - 1) {
                let idx = row + x;
                if alpha_mask[idx] {
                    continue;
                }
                if alpha_mask[idx - 1]
                    || alpha_mask[idx + 1]
                    || alpha_mask[idx - w]
                    || alpha_mask[idx + w]
                {
                    pixels[idx] = outline_px;
                }
            }
        }
    }
}

pub fn paint_standard_faces(
    faces: &mut [Vec<RgbaPixel>; 5],
    eye_x: f32,
    eye_y: f32,
    ink: RgbaPixel,
    blush: RgbaPixel,
) {
    for f in faces.iter_mut() {
        f.fill([0, 0, 0, 0]);
    }
    let white = [255, 255, 255, 255];

    // Neutral: big glossy cute eye + twin sparkles + soft blush
    paint_ellipse(&mut faces[0], eye_x, eye_y, 3.2, 3.8, ink);
    paint_ellipse(&mut faces[0], eye_x - 1.0, eye_y - 1.3, 1.35, 1.35, white);
    paint_ellipse(&mut faces[0], eye_x + 1.0, eye_y + 1.4, 0.7, 0.7, white);
    paint_ellipse(&mut faces[0], eye_x + 5.2, eye_y + 4.8, 3.0, 1.7, blush);

    // Blink: clean closed curve
    paint_ellipse(&mut faces[1], eye_x, eye_y + 0.6, 3.6, 1.0, ink);
    paint_ellipse(&mut faces[1], eye_x + 5.2, eye_y + 4.8, 3.0, 1.7, blush);

    // Happy: joyful ^ eye + rosy cheek
    paint_ellipse(&mut faces[2], eye_x - 2.0, eye_y + 0.4, 1.4, 1.3, ink);
    paint_ellipse(&mut faces[2], eye_x, eye_y - 1.2, 1.6, 1.2, ink);
    paint_ellipse(&mut faces[2], eye_x + 2.0, eye_y + 0.4, 1.4, 1.3, ink);
    paint_ellipse(&mut faces[2], eye_x + 5.2, eye_y + 4.4, 3.8, 2.1, blush);

    // Sleep: peaceful lowered U curve
    paint_ellipse(&mut faces[3], eye_x - 1.8, eye_y + 1.0, 1.3, 1.0, ink);
    paint_ellipse(&mut faces[3], eye_x, eye_y + 1.9, 1.8, 1.0, ink);
    paint_ellipse(&mut faces[3], eye_x + 1.8, eye_y + 1.0, 1.3, 1.0, ink);
    paint_ellipse(&mut faces[3], eye_x + 5.2, eye_y + 5.2, 2.8, 1.5, blush);

    // Surprised: cute extra-wide starry eye + tiny "o" mouth (no creepy bullseye!)
    paint_ellipse(&mut faces[4], eye_x, eye_y, 3.9, 4.4, ink);
    paint_ellipse(&mut faces[4], eye_x - 1.2, eye_y - 1.4, 1.6, 1.6, white);
    paint_ellipse(&mut faces[4], eye_x + 1.3, eye_y + 1.4, 0.9, 0.9, white);
    paint_ellipse(&mut faces[4], eye_x - 3.5, eye_y + 6.2, 1.6, 1.8, ink);
    paint_ellipse(&mut faces[4], eye_x + 5.4, eye_y + 4.6, 3.2, 1.8, blush);
}

pub fn rgba_buf_to_png_b64(buf: &[RgbaPixel]) -> String {
    let mut img = image::RgbaImage::new(BOARD_SIZE as u32, BOARD_SIZE as u32);
    for (i, &px) in buf.iter().enumerate() {
        img.put_pixel((i % BOARD_SIZE) as u32, (i / BOARD_SIZE) as u32, image::Rgba(px));
    }
    let mut bytes = Vec::new();
    if let Err(e) = img.write_to(&mut Cursor::new(&mut bytes), image::ImageFormat::Png) { crate::logging::warn(&format!("studio_model: PNG encode failed: {e}")); }
    STANDARD.encode(&bytes)
}

pub fn png_b64_to_rgba_buf(b64: &str) -> Result<Vec<RgbaPixel>, String> {
    let bytes = STANDARD
        .decode(b64.trim())
        .map_err(|e| format!("base64 error: {e}"))?;
    let img = image::load_from_memory(&bytes)
        .map_err(|e| format!("png decode error: {e}"))?
        .to_rgba8();
    Ok(rgba_image_fit_to_board(&img))
}

pub fn rgba_image_fit_to_board(source: &image::RgbaImage) -> Vec<RgbaPixel> {
    let mut img = source.clone();
    strip_chroma_key_if_opaque(&mut img);
    let (w, h) = img.dimensions();
    let resized = if w == BOARD_SIZE as u32 && h == BOARD_SIZE as u32 {
        img
    } else {
        let scale = (BOARD_SIZE as f32 / w.max(1) as f32).min(BOARD_SIZE as f32 / h.max(1) as f32);
        let nw = ((w as f32 * scale).round() as u32).clamp(1, BOARD_SIZE as u32);
        let nh = ((h as f32 * scale).round() as u32).clamp(1, BOARD_SIZE as u32);
        let filter = if w.max(h) < 64 {
            image::imageops::FilterType::Nearest
        } else {
            image::imageops::FilterType::Lanczos3
        };
        let scaled = image::imageops::resize(&img, nw, nh, filter);
        let mut canvas = image::RgbaImage::from_pixel(
            BOARD_SIZE as u32,
            BOARD_SIZE as u32,
            image::Rgba([0, 0, 0, 0]),
        );
        let ox = (BOARD_SIZE as u32 - nw) / 2;
        let oy = (BOARD_SIZE as u32 - nh) / 2;
        image::imageops::overlay(&mut canvas, &scaled, ox as i64, oy as i64);
        canvas
    };

    let mut out = vec![[0, 0, 0, 0]; BOARD_LEN];
    for y in 0..BOARD_SIZE {
        for x in 0..BOARD_SIZE {
            out[y * BOARD_SIZE + x] = resized.get_pixel(x as u32, y as u32).0;
        }
    }
    out
}

pub fn buf_bbox(buf: &[RgbaPixel]) -> Option<(usize, usize, usize, usize)> {
    let mut min_x = BOARD_SIZE;
    let mut min_y = BOARD_SIZE;
    let mut max_x = 0;
    let mut max_y = 0;
    let mut found = false;
    for y in 0..BOARD_SIZE {
        for x in 0..BOARD_SIZE {
            if buf[y * BOARD_SIZE + x][3] > 16 {
                found = true;
                min_x = min_x.min(x);
                min_y = min_y.min(y);
                max_x = max_x.max(x);
                max_y = max_y.max(y);
            }
        }
    }
    found.then_some((min_x, min_y, max_x, max_y))
}

impl StudioProject {
    pub fn blank(archetype: Archetype) -> Self {
        let parts = match archetype {
            Archetype::Quadruped => vec![
                RigPart::new("Front Leg", PartRole::FrontLimb),
                RigPart::new("Back Leg", PartRole::BackLimb),
                RigPart::new("Tail", PartRole::Tail),
            ],
            Archetype::Biped => vec![
                RigPart::new("Arm", PartRole::FrontLimb),
                RigPart::new("Leg", PartRole::BackLimb),
            ],
            Archetype::Bird => vec![
                RigPart::new("Wing", PartRole::Wing),
                RigPart::new("Tail", PartRole::Tail),
                RigPart::new("Foot", PartRole::BackLimb),
            ],
            Archetype::Crawler => vec![RigPart::new("Crest", PartRole::Ear)],
            Archetype::Swimmer => vec![
                RigPart::new("Side Fin", PartRole::Wing),
                RigPart::new("Tail Fin", PartRole::Tail),
            ],
        };
        Self {
            display_name: "My Pet".to_string(),
            pack_id: "my_pet".to_string(),
            author: "SM".to_string(),
            archetype,
            shade_far_limbs: true,
            seamless_outline: true,
            outline_color: [44, 30, 26],
            global_limb_scale: 1.0,
            global_swing_mult: 1.0,
            body_bob: if archetype == Archetype::Crawler { 0.8 } else { 1.8 },
            wave_amplitude: match archetype {
                Archetype::Crawler => 5.5,
                Archetype::Swimmer => 4.2,
                _ => 0.0,
            },
            walk_step_px: 7,
            frame_interval_ms: 115,
            body: vec![[0, 0, 0, 0]; BOARD_LEN],
            parts,
            faces: std::array::from_fn(|_| vec![[0, 0, 0, 0]; BOARD_LEN]),
            custom_animations: vec![CustomAnimation::hop_preset()],
        }
    }

    pub fn template_by_archetype(archetype: Archetype) -> Self {
        match archetype {
            Archetype::Quadruped => Self::template_cat(),
            Archetype::Biped => Self::template_buddy(),
            Archetype::Bird => Self::template_bird(),
            Archetype::Crawler => Self::template_snake(),
            Archetype::Swimmer => Self::template_fish(),
        }
    }

    /// Polished chibi Ginger Cat with clipped fur stripes, soft shading, & seamless limb joints!
    pub fn template_cat() -> Self {
        let mut p = Self::blank(Archetype::Quadruped);
        p.display_name = "Ginger Cat".to_string();
        p.pack_id = "ginger_cat".to_string();
        p.outline_color = [52, 32, 24];

        let ink = [52, 32, 24, 255];
        let fur = [238, 166, 90, 255];
        let fur_light = [248, 194, 124, 255];
        let belly = [254, 240, 216, 255];
        let stripe = [194, 110, 48, 255];
        let blush = [248, 162, 174, 255];
        let nose = [232, 114, 126, 255];

        // Plump torso + chest fluff
        paint_ellipse(&mut p.body, 69.0, 72.0, 25.0, 15.5, fur);
        paint_ellipse(&mut p.body, 48.0, 72.0, 12.0, 13.0, fur);
        // Clipped belly & tabby stripes (100% INSIDE torso — never pokes out!)
        paint_ellipse_clipped(&mut p.body, 67.0, 81.0, 20.0, 9.5, belly);
        paint_ellipse_clipped(&mut p.body, 58.0, 58.0, 3.5, 7.5, stripe);
        paint_ellipse_clipped(&mut p.body, 69.0, 57.0, 3.5, 8.0, stripe);
        paint_ellipse_clipped(&mut p.body, 80.0, 58.0, 3.2, 7.0, stripe);
        paint_ellipse_clipped(&mut p.body, 67.0, 64.0, 16.0, 5.0, fur_light);

        // Chibi head facing LEFT with pointed ears
        paint_triangle(&mut p.body, (29.0, 48.0), (31.0, 31.0), (41.0, 43.0), fur);
        paint_triangle(&mut p.body, (44.0, 43.0), (52.0, 31.0), (56.0, 48.0), fur);
        paint_triangle(&mut p.body, (31.5, 45.0), (32.5, 35.5), (38.5, 43.0), blush);
        paint_triangle(&mut p.body, (46.5, 43.0), (51.5, 35.5), (53.5, 45.0), blush);
        paint_ellipse(&mut p.body, 42.0, 54.0, 18.5, 16.0, fur);
        paint_ellipse_clipped(&mut p.body, 43.0, 40.0, 3.0, 5.5, stripe);
        paint_ellipse_clipped(&mut p.body, 49.0, 41.0, 2.6, 4.5, stripe);
        // Cute cream muzzle + pink nose
        paint_ellipse_clipped(&mut p.body, 31.0, 59.5, 9.0, 6.2, belly);
        paint_ellipse(&mut p.body, 25.5, 57.5, 2.2, 1.6, nose);
        paint_soft_underside_shade(&mut p.body, 5, 0.90);

        paint_standard_faces(&mut p.faces, 35.5, 52.5, ink, blush);

        // Seamless Front Leg (no top outline seam!)
        let mut fl = RigPart::new("Front Leg", PartRole::FrontLimb);
        paint_ellipse(&mut fl.pixels, 51.0, 88.0, 6.2, 11.5, fur);
        paint_ellipse(&mut fl.pixels, 48.5, 98.5, 6.5, 4.2, belly);
        paint_soft_underside_shade(&mut fl.pixels, 3, 0.90);
        fl.socket_near = (51, 80);
        fl.socket_far = (45, 79);
        fl.pivot = (51, 80);
        fl.swing_deg = 19.0;

        // Seamless Back Leg
        let mut bl = RigPart::new("Back Leg", PartRole::BackLimb);
        paint_ellipse(&mut bl.pixels, 82.0, 88.0, 7.2, 11.5, fur);
        paint_ellipse(&mut bl.pixels, 79.5, 98.5, 6.8, 4.2, belly);
        paint_soft_underside_shade(&mut bl.pixels, 3, 0.90);
        bl.socket_near = (82, 80);
        bl.socket_far = (76, 79);
        bl.pivot = (82, 80);
        bl.swing_deg = 18.0;

        // Expressive Curved Tail
        let mut tail = RigPart::new("Tail", PartRole::Tail);
        paint_ellipse(&mut tail.pixels, 95.0, 66.0, 8.5, 5.0, fur);
        paint_ellipse(&mut tail.pixels, 102.0, 56.0, 5.5, 9.5, fur);
        paint_ellipse(&mut tail.pixels, 101.0, 46.0, 5.2, 5.2, belly);
        paint_ellipse_clipped(&mut tail.pixels, 101.0, 56.0, 5.0, 2.5, stripe);
        tail.socket_near = (90, 67);
        tail.pivot = (90, 67);
        tail.swing_deg = 14.0;

        p.parts = vec![fl, bl, tail];
        p
    }

    pub fn template_buddy() -> Self {
        let mut p = Self::blank(Archetype::Biped);
        p.display_name = "Chibi Buddy".to_string();
        p.pack_id = "chibi_buddy".to_string();
        p.outline_color = [32, 36, 54];

        let ink = [32, 36, 54, 255];
        let hoodie = [92, 152, 234, 255];
        let hoodie_hi = [126, 178, 246, 255];
        let skin = [252, 224, 198, 255];
        let hair = [44, 50, 74, 255];
        let shoe = [236, 94, 100, 255];
        let blush = [250, 166, 178, 255];

        paint_ellipse(&mut p.body, 64.0, 74.0, 15.5, 16.0, hoodie);
        paint_ellipse_clipped(&mut p.body, 62.0, 66.0, 12.0, 6.0, hoodie_hi);
        paint_ellipse(&mut p.body, 62.0, 45.0, 18.5, 17.5, skin);
        paint_ellipse(&mut p.body, 65.0, 34.0, 19.0, 10.0, hair);
        paint_ellipse(&mut p.body, 46.0, 39.0, 8.5, 3.8, hoodie);
        paint_soft_underside_shade(&mut p.body, 4, 0.90);

        paint_standard_faces(&mut p.faces, 52.0, 47.0, ink, blush);

        let mut arm = RigPart::new("Arm", PartRole::FrontLimb);
        paint_ellipse(&mut arm.pixels, 64.0, 74.0, 5.2, 10.5, hoodie);
        paint_ellipse(&mut arm.pixels, 63.0, 84.0, 4.6, 4.2, skin);
        arm.socket_near = (66, 66);
        arm.socket_far = (58, 66);
        arm.pivot = (64, 65);
        arm.swing_deg = 24.0;

        let mut leg = RigPart::new("Leg", PartRole::BackLimb);
        paint_ellipse(&mut leg.pixels, 64.0, 94.0, 5.8, 10.5, hair);
        paint_ellipse(&mut leg.pixels, 60.0, 103.5, 7.6, 4.2, shoe);
        leg.socket_near = (66, 86);
        leg.socket_far = (59, 86);
        leg.pivot = (64, 86);
        leg.swing_deg = 22.0;

        p.parts = vec![arm, leg];
        p
    }

    pub fn template_bird() -> Self {
        let mut p = Self::blank(Archetype::Bird);
        p.display_name = "Sky Bird".to_string();
        p.pack_id = "sky_bird".to_string();
        p.outline_color = [28, 46, 72];
        p.body_bob = 3.0;

        let ink = [28, 46, 72, 255];
        let sky = [92, 188, 246, 255];
        let belly = [244, 250, 255, 255];
        let wing_c = [58, 144, 218, 255];
        let beak = [248, 180, 58, 255];
        let blush = [252, 166, 178, 255];

        paint_ellipse(&mut p.body, 65.0, 70.0, 22.5, 17.0, sky);
        paint_ellipse_clipped(&mut p.body, 56.0, 78.0, 17.0, 11.0, belly);
        paint_ellipse(&mut p.body, 48.0, 52.0, 15.5, 14.5, sky);
        paint_ellipse(&mut p.body, 53.0, 38.0, 5.2, 4.2, wing_c);
        paint_triangle(&mut p.body, (35.0, 51.0), (24.0, 55.5), (35.0, 59.0), beak);
        paint_soft_underside_shade(&mut p.body, 4, 0.91);

        paint_standard_faces(&mut p.faces, 43.0, 50.5, ink, blush);

        let mut wing = RigPart::new("Wing", PartRole::Wing);
        paint_ellipse(&mut wing.pixels, 70.0, 68.0, 15.5, 9.0, wing_c);
        paint_ellipse_clipped(&mut wing.pixels, 74.0, 71.0, 10.0, 5.5, sky);
        wing.socket_near = (60, 66);
        wing.socket_far = (56, 62);
        wing.pivot = (58, 66);
        wing.swing_deg = 32.0;

        let mut tail = RigPart::new("Tail", PartRole::Tail);
        paint_triangle(&mut tail.pixels, (84.0, 66.0), (107.0, 59.0), (104.0, 74.0), wing_c);
        tail.socket_near = (85, 68);
        tail.pivot = (85, 68);
        tail.swing_deg = 14.0;

        let mut foot = RigPart::new("Foot", PartRole::BackLimb);
        paint_ellipse(&mut foot.pixels, 63.0, 90.0, 2.5, 7.5, beak);
        paint_ellipse(&mut foot.pixels, 59.5, 96.5, 6.2, 2.6, beak);
        foot.socket_near = (64, 84);
        foot.socket_far = (57, 83);
        foot.pivot = (63, 84);
        foot.swing_deg = 18.0;

        p.parts = vec![wing, tail, foot];
        p
    }

    pub fn template_snake() -> Self {
        let mut p = Self::blank(Archetype::Crawler);
        p.display_name = "Emerald Snake".to_string();
        p.pack_id = "emerald_snake".to_string();
        p.outline_color = [22, 54, 38];
        p.wave_amplitude = 5.5;
        p.body_bob = 0.5;
        p.walk_step_px = 6;

        let ink = [22, 54, 38, 255];
        let green = [96, 204, 114, 255];
        let belly = [224, 248, 184, 255];
        let spot = [54, 154, 78, 255];
        let tongue = [238, 76, 86, 255];
        let blush = [252, 160, 170, 255];

        paint_ellipse(&mut p.body, 28.0, 74.0, 13.0, 10.5, green);
        paint_ellipse(&mut p.body, 14.0, 76.0, 4.5, 1.5, tongue);
        for i in 0..9 {
            let cx = 38.0 + i as f32 * 8.5;
            let ry = (9.5 - i as f32 * 0.72).max(2.8);
            paint_ellipse(&mut p.body, cx, 78.0 + (i as f32 * 0.35), 7.5, ry, green);
        }
        for i in 0..8 {
            let cx = 38.0 + i as f32 * 8.5;
            let ry = (9.5 - i as f32 * 0.72).max(2.8);
            paint_ellipse_clipped(&mut p.body, cx, 82.0 + (i as f32 * 0.3), 5.8, ry * 0.45, belly);
            if i % 2 == 1 {
                paint_ellipse_clipped(&mut p.body, cx, 73.5, 3.2, ry * 0.48, spot);
            }
        }
        paint_soft_underside_shade(&mut p.body, 3, 0.90);

        paint_standard_faces(&mut p.faces, 25.0, 71.0, ink, blush);
        p.parts.clear();
        p
    }

    pub fn template_fish() -> Self {
        let mut p = Self::blank(Archetype::Swimmer);
        p.display_name = "Koi Fish".to_string();
        p.pack_id = "koi_fish".to_string();
        p.outline_color = [78, 34, 24];
        p.wave_amplitude = 4.2;
        p.body_bob = 2.4;

        let ink = [78, 34, 24, 255];
        let coral = [246, 126, 64, 255];
        let cream = [254, 238, 214, 255];
        let fin_c = [252, 182, 96, 255];
        let blush = [252, 160, 170, 255];

        paint_ellipse(&mut p.body, 58.0, 68.0, 27.5, 16.5, coral);
        paint_ellipse(&mut p.body, 62.0, 51.0, 11.0, 6.0, fin_c);
        paint_ellipse_clipped(&mut p.body, 54.0, 76.0, 20.0, 9.0, cream);
        paint_ellipse_clipped(&mut p.body, 67.0, 63.0, 8.0, 6.0, cream);
        paint_soft_underside_shade(&mut p.body, 4, 0.90);

        paint_standard_faces(&mut p.faces, 39.0, 65.0, ink, blush);

        let mut fin = RigPart::new("Side Fin", PartRole::Wing);
        paint_ellipse(&mut fin.pixels, 58.0, 77.0, 8.5, 4.8, fin_c);
        fin.socket_near = (54, 75);
        fin.socket_far = (50, 73);
        fin.pivot = (52, 75);
        fin.swing_deg = 22.0;

        let mut tail = RigPart::new("Tail Fin", PartRole::Tail);
        paint_ellipse(&mut tail.pixels, 95.0, 62.0, 11.0, 6.5, fin_c);
        paint_ellipse(&mut tail.pixels, 95.0, 74.0, 11.0, 6.5, fin_c);
        tail.socket_near = (84, 68);
        tail.pivot = (85, 68);
        tail.swing_deg = 24.0;

        p.parts = vec![fin, tail];
        p
    }

    pub fn auto_fit_anchors(&mut self) {
        let Some((bx0, by0, bx1, by1)) = buf_bbox(&self.body) else {
            return;
        };
        let bw = bx1.saturating_sub(bx0).max(1);
        let bh = by1.saturating_sub(by0).max(1);

        for part in &mut self.parts {
            if let Some((px0, py0, px1, py1)) = buf_bbox(&part.pixels) {
                let pcx = (px0 + px1) / 2;
                let pcy = (py0 + py1) / 2;
                part.pivot = match part.role {
                    PartRole::Tail | PartRole::Wing => (px0 + (px1 - px0) / 6, pcy),
                    PartRole::Ear => (pcx, py1.saturating_sub((py1 - py0) / 6)),
                    _ => (pcx, py0 + (py1 - py0) / 6),
                };
                let sx = pcx.clamp(bx0, bx1);
                let sy = part.pivot.1.clamp(by0, by1);
                part.socket_near = (sx, sy);
                part.socket_far = (sx.saturating_sub(6), sy.saturating_sub(2));
            } else {
                let (sx, sy) = match part.role {
                    PartRole::FrontLimb => (bx0 + (bw * 32) / 100, by0 + (bh * 78) / 100),
                    PartRole::BackLimb => (bx0 + (bw * 74) / 100, by0 + (bh * 78) / 100),
                    PartRole::Wing => (bx0 + (bw * 52) / 100, by0 + (bh * 48) / 100),
                    PartRole::Tail => (bx1.saturating_sub(2), by0 + (bh * 55) / 100),
                    PartRole::Ear => (bx0 + (bw * 28) / 100, by0 + (bh * 12) / 100),
                    PartRole::Extra => ((bx0 + bx1) / 2, (by0 + by1) / 2),
                };
                part.socket_near = (sx.min(BOARD_SIZE - 1), sy.min(BOARD_SIZE - 1));
                part.socket_far = (sx.saturating_sub(6), sy.saturating_sub(2));
                part.pivot = part.socket_near;
            }
        }
    }

    fn buf_to_pixmap(buf: &[RgbaPixel], mirror_x: bool, darken: f32) -> Pixmap {
        let mut pm = Pixmap::new(BOARD_SIZE as u32, BOARD_SIZE as u32).unwrap();
        let pixels = pm.pixels_mut();
        for y in 0..BOARD_SIZE {
            for x in 0..BOARD_SIZE {
                let sx = if mirror_x { BOARD_SIZE - 1 - x } else { x };
                let [r, g, b, a] = buf[y * BOARD_SIZE + sx];
                if a > 0 {
                    let dr = ((r as f32) * darken).round().clamp(0.0, 255.0) as u8;
                    let dg = ((g as f32) * darken).round().clamp(0.0, 255.0) as u8;
                    let db = ((b as f32) * darken).round().clamp(0.0, 255.0) as u8;
                    pixels[y * BOARD_SIZE + x] = ColorU8::from_rgba(dr, dg, db, a).premultiply();
                }
            }
        }
        pm
    }

    fn composite_body_and_face(&self, face: FaceSlot, wave_amp: f32, wave_phase: f32) -> Pixmap {
        let mut combined = self.body.clone();
        let face_buf = &self.faces[face.index()];
        let has_face = face_buf.iter().any(|p| p[3] > 0);
        let active_face = if has_face {
            face_buf
        } else {
            &self.faces[FaceSlot::Neutral.index()]
        };

        for i in 0..BOARD_LEN {
            let fg = active_face[i];
            if fg[3] > 0 {
                if fg[3] == 255 || combined[i][3] == 0 {
                    combined[i] = fg;
                } else {
                    let fa = fg[3] as f32 / 255.0;
                    let ba = combined[i][3] as f32 / 255.0;
                    let out_a = fa + ba * (1.0 - fa);
                    if out_a > 0.001 {
                        let blend = |f: u8, b: u8| {
                            ((f as f32 * fa + b as f32 * ba * (1.0 - fa)) / out_a)
                                .round()
                                .clamp(0.0, 255.0) as u8
                        };
                        combined[i] = [
                            blend(fg[0], combined[i][0]),
                            blend(fg[1], combined[i][1]),
                            blend(fg[2], combined[i][2]),
                            (out_a * 255.0).round().clamp(0.0, 255.0) as u8,
                        ];
                    }
                }
            }
        }

        if wave_amp.abs() < 0.05 {
            return Self::buf_to_pixmap(&combined, false, 1.0);
        }

        let (bx0, _, bx1, _) = buf_bbox(&combined).unwrap_or((16, 16, 112, 112));
        let span = (bx1.saturating_sub(bx0)).max(1) as f32;
        let mut warped = vec![[0, 0, 0, 0]; BOARD_LEN];
        for x in 0..BOARD_SIZE {
            let u = ((x as f32 - bx0 as f32) / span).clamp(0.0, 1.0);
            let envelope = u.powf(0.75);
            let shift = ((u * std::f32::consts::TAU * 1.35 - wave_phase).sin() * wave_amp * envelope)
                .round() as isize;
            for y in 0..BOARD_SIZE {
                let src_y = y as isize - shift;
                if src_y >= 0 && src_y < BOARD_SIZE as isize {
                    warped[y * BOARD_SIZE + x] = combined[src_y as usize * BOARD_SIZE + x];
                }
            }
        }
        Self::buf_to_pixmap(&warped, false, 1.0)
    }

    pub fn render_pose(&self, pose: PoseParams) -> Pixmap {
        let canvas_side = (BOARD_SIZE * 2) as u32;
        let mut far_layer = Pixmap::new(canvas_side, canvas_side).unwrap();
        let mut main_layer = Pixmap::new(canvas_side, canvas_side).unwrap();

        let far_shade = if self.shade_far_limbs { 0.78 } else { 1.0 };
        let body_pm = self.composite_body_and_face(pose.face, pose.wave_amp_px, pose.wave_phase);

        let ox = (BOARD_SIZE / 2) as f32;
        let oy = (BOARD_SIZE / 2) as f32 + pose.whole_dy;

        let (_, by0, _, by1) = buf_bbox(&self.body).unwrap_or((32, 32, 96, 96));
        let hip_y = oy + (by0 as f32 + (by1 - by0) as f32 * 0.78);
        let body_cx = ox + (BOARD_SIZE as f32) * 0.5;

        let limb_frame_xform = Transform::from_translate(body_cx, hip_y)
            .pre_rotate(pose.body_tilt_deg)
            .pre_translate(-body_cx, -hip_y);

        let torso_xform = Transform::from_translate(body_cx, hip_y + pose.torso_dy)
            .pre_rotate(pose.body_tilt_deg)
            .pre_scale(1.0, pose.torso_scale_y)
            .pre_translate(-body_cx, -hip_y);

        let part_angle = |part: &RigPart, is_far: bool| -> f32 {
            let phase = pose.phase_rad
                + part.phase_offset_deg.to_radians()
                + if is_far { std::f32::consts::PI } else { 0.0 };
            let s = phase.sin();
            let base = part.swing_deg * self.global_swing_mult;
            match part.role {
                PartRole::FrontLimb => pose.front_bias_deg + s * base * pose.limb_amp,
                PartRole::BackLimb => pose.back_bias_deg + s * base * pose.limb_amp,
                PartRole::Wing => s * base * pose.wing_amp,
                PartRole::Tail => s * base * pose.tail_amp,
                PartRole::Ear => s * base * pose.ear_amp,
                PartRole::Extra => s * base * pose.limb_amp,
            }
        };

        let draw_part_instance =
            |target: &mut Pixmap, part: &RigPart, is_far: bool, angle_deg: f32| {
                if !part.visible {
                    return;
                }
                let mirrored = is_far && part.mirror_far;
                let shade = if is_far { far_shade } else { 1.0 };
                let pm = Self::buf_to_pixmap(&part.pixels, mirrored, shade);
                let socket = if is_far {
                    part.socket_far
                } else {
                    part.socket_near
                };
                let px = if mirrored {
                    (BOARD_SIZE - 1 - part.pivot.0) as f32
                } else {
                    part.pivot.0 as f32
                };
                let py = part.pivot.1 as f32;
                let sc = part.scale * self.global_limb_scale;
                let lx = Transform::from_translate(ox + socket.0 as f32, oy + socket.1 as f32)
                    .pre_rotate(angle_deg)
                    .pre_scale(sc, sc)
                    .pre_translate(-px, -py);
                let frame = if matches!(part.role, PartRole::Ear | PartRole::Tail | PartRole::Wing)
                {
                    torso_xform
                } else {
                    limb_frame_xform
                };
                target.draw_pixmap(0, 0, pm.as_ref(), &PixmapPaint::default(), frame.pre_concat(lx), None);
            };

        // Pass 1: Far paired limbs on separate far_layer (gets its own clean depth-separated outline!)
        for part in &self.parts {
            if part.paired {
                draw_part_instance(&mut far_layer, part, true, part_angle(part, true));
            }
        }
        if self.seamless_outline {
            outline_pixmap_silhouette(&mut far_layer, self.outline_color, 1);
        }

        // Pass 2: Behind-body parts (tails) + Body + Near limbs composited SEAMLESSLY on main_layer!
        for part in &self.parts {
            if part.behind_body {
                draw_part_instance(&mut main_layer, part, false, part_angle(part, false));
            }
        }
        let body_draw = torso_xform.pre_concat(Transform::from_translate(ox, oy));
        main_layer.draw_pixmap(0, 0, body_pm.as_ref(), &PixmapPaint::default(), body_draw, None);
        for part in &self.parts {
            if !part.behind_body {
                draw_part_instance(&mut main_layer, part, false, part_angle(part, false));
            }
        }
        if self.seamless_outline {
            outline_pixmap_silhouette(&mut main_layer, self.outline_color, 2);
        }

        // Combine far_layer behind main_layer
        let mut canvas = far_layer;
        canvas.draw_pixmap(
            0,
            0,
            main_layer.as_ref(),
            &PixmapPaint::default(),
            Transform::identity(),
            None,
        );

        let mut final_pm = if pose.rotate_whole_deg.abs() > 0.01 {
            let mut rotated = Pixmap::new(canvas_side, canvas_side).unwrap();
            let c = canvas_side as f32 * 0.5;
            let xf = Transform::from_translate(c, c)
                .pre_rotate(pose.rotate_whole_deg)
                .pre_translate(-c, -c);
            rotated.draw_pixmap(0, 0, canvas.as_ref(), &PixmapPaint::default(), xf, None);
            rotated
        } else {
            canvas
        };

        if let Some(phase) = pose.zzz_phase {
            let drift = (phase * 6.0).round() as i32;
            Self::stamp_zzz(
                &mut final_pm,
                (ox + 34.0) as i32,
                (oy + 20.0) as i32 - drift,
            );
        }
        final_pm
    }

    fn stamp_zzz(pm: &mut Pixmap, x0: i32, y0: i32) {
        let glyph: &[(i32, i32)] = &[
            (0, 0), (1, 0), (2, 0), (3, 0), (4, 0),
            (3, 1), (2, 2), (1, 3),
            (0, 4), (1, 4), (2, 4), (3, 4), (4, 4),
        ];
        let w = pm.width() as i32;
        let h = pm.height() as i32;
        let outline = ColorU8::from_rgba(32, 52, 92, 255).premultiply();
        let fill = ColorU8::from_rgba(145, 202, 255, 255).premultiply();
        let pixels = pm.pixels_mut();
        for (gx, gy, scale) in [(x0, y0, 2), (x0 - 11, y0 - 12, 1)] {
            for &(dx, dy) in glyph {
                for sy in 0..scale {
                    for sx in 0..scale {
                        let px = gx + dx * scale + sx;
                        let py = gy + dy * scale + sy;
                        if px >= 1 && px < w - 1 && py >= 1 && py < h - 1 {
                            pixels[(py * w + px + 1) as usize] = outline;
                            pixels[((py + 1) * w + px) as usize] = outline;
                            pixels[(py * w + px) as usize] = fill;
                        }
                    }
                }
            }
        }
    }

    pub fn walk_pose(&self, phase_rad: f32) -> PoseParams {
        let s = phase_rad.sin();
        let bob = (phase_rad * 2.0).sin().abs() * -self.body_bob;
        match self.archetype {
            Archetype::Quadruped | Archetype::Biped => PoseParams {
                phase_rad,
                limb_amp: 1.0,
                wing_amp: 0.45,
                tail_amp: 0.75,
                ear_amp: 0.45,
                whole_dy: bob,
                body_tilt_deg: s * 1.4,
                ..Default::default()
            },
            Archetype::Bird => PoseParams {
                phase_rad,
                limb_amp: 0.85,
                wing_amp: 1.15,
                tail_amp: 0.85,
                whole_dy: phase_rad.sin().abs() * -self.body_bob * 1.35,
                body_tilt_deg: s * 4.0,
                ..Default::default()
            },
            Archetype::Crawler => PoseParams {
                phase_rad,
                limb_amp: 0.4,
                tail_amp: 0.8,
                whole_dy: bob * 0.4,
                wave_amp_px: self.wave_amplitude.max(3.0),
                wave_phase: phase_rad,
                ..Default::default()
            },
            Archetype::Swimmer => PoseParams {
                phase_rad,
                wing_amp: 1.0,
                tail_amp: 1.2,
                whole_dy: (phase_rad * 0.5).sin() * self.body_bob,
                body_tilt_deg: (phase_rad * 0.5).cos() * 3.2,
                wave_amp_px: self.wave_amplitude.max(2.5),
                wave_phase: phase_rad,
                ..Default::default()
            },
        }
    }

    pub fn keyframe_to_pose(&self, kf: &CustomKeyframe) -> PoseParams {
        let phase = kf.limb_swing.clamp(-1.0, 1.0) * std::f32::consts::FRAC_PI_2;
        PoseParams {
            phase_rad: phase,
            limb_amp: kf.limb_swing.abs().max(0.01),
            wing_amp: kf.wing_flap,
            tail_amp: kf.tail_wag,
            ear_amp: kf.tail_wag * 0.5,
            whole_dy: kf.body_dy,
            torso_scale_y: kf.torso_scale_y.clamp(0.7, 1.3),
            body_tilt_deg: kf.body_tilt_deg,
            wave_amp_px: if kf.wave_phase.abs() > 0.01 {
                self.wave_amplitude.max(3.5)
            } else {
                0.0
            },
            wave_phase: kf.wave_phase,
            face: kf.face,
            ..Default::default()
        }
    }

    pub fn live_pose(&self, clip: PreviewClip, t: f32) -> PoseParams {
        let phase = t * 6.0;
        match clip {
            PreviewClip::Walk => self.walk_pose(phase),
            PreviewClip::Idle => {
                let breath = (t * 2.2).sin();
                let blink = (t % 3.6) > 3.4;
                PoseParams {
                    phase_rad: t * 2.4,
                    tail_amp: 0.45,
                    ear_amp: 0.3,
                    wing_amp: if self.archetype == Archetype::Swimmer { 0.4 } else { 0.08 },
                    torso_dy: breath * 0.7,
                    torso_scale_y: 1.0 - breath * 0.018,
                    body_tilt_deg: (t * 1.1).sin() * 1.0,
                    wave_amp_px: if self.archetype == Archetype::Swimmer { 2.0 } else { 0.0 },
                    wave_phase: t * 3.0,
                    face: if blink { FaceSlot::Blink } else { FaceSlot::Neutral },
                    ..Default::default()
                }
            }
            // Anatomically clean Sit: front paws planted down, hind paws tucked neatly forward (no belly stab!)
            PreviewClip::Sit => {
                let breath = (t * 2.0).sin();
                PoseParams {
                    phase_rad: t * 2.0,
                    tail_amp: 0.5,
                    front_bias_deg: 5.0,
                    back_bias_deg: 26.0,
                    whole_dy: if matches!(self.archetype, Archetype::Crawler | Archetype::Swimmer) {
                        1.5
                    } else {
                        3.5
                    },
                    torso_dy: breath * 0.5,
                    torso_scale_y: 1.0 - breath * 0.015,
                    body_tilt_deg: 7.0,
                    face: FaceSlot::Happy,
                    ..Default::default()
                }
            }
            // Cozy Loaf Sleep: legs tucked gently under without crossing like scissors
            PreviewClip::Sleep => {
                let breath = (t * 1.6).sin();
                PoseParams {
                    front_bias_deg: -24.0,
                    back_bias_deg: 24.0,
                    whole_dy: if matches!(self.archetype, Archetype::Crawler | Archetype::Swimmer) {
                        2.0
                    } else {
                        6.0
                    },
                    torso_dy: 0.8 + breath * 0.6,
                    torso_scale_y: 0.95 - breath * 0.022,
                    face: FaceSlot::Sleep,
                    zzz_phase: Some((t * 0.45) % 1.0),
                    ..Default::default()
                }
            }
            // Scruff-held dangle: relaxed limbs swaying gently
            PreviewClip::Drag => PoseParams {
                phase_rad: t * 6.5,
                limb_amp: 0.45,
                wing_amp: 0.85,
                tail_amp: 0.85,
                front_bias_deg: 10.0,
                back_bias_deg: -12.0,
                body_tilt_deg: (t * 6.0).sin() * 4.0,
                face: FaceSlot::Surprised,
                ..Default::default()
            },
            // Mid-air drop: front paws reach slightly forward, back paws stretch slightly back (no X cross!)
            PreviewClip::Fall => PoseParams {
                phase_rad: t * 12.0,
                limb_amp: 0.45,
                wing_amp: 1.25,
                tail_amp: 1.0,
                front_bias_deg: 20.0,
                back_bias_deg: -20.0,
                whole_dy: (t * 10.0).sin() * 1.2,
                body_tilt_deg: -5.0 + (t * 12.0).sin() * 1.2,
                face: FaceSlot::Surprised,
                ..Default::default()
            },
            // Wall Climb: compact paw-over-paw pulls facing UP
            PreviewClip::Climb => PoseParams {
                phase_rad: t * 6.5,
                limb_amp: 0.65,
                wing_amp: 0.5,
                tail_amp: 0.5,
                whole_dy: ((t * 6.5).sin().abs() - 0.5) * 2.2,
                rotate_whole_deg: 90.0,
                wave_amp_px: if self.archetype == Archetype::Crawler { 4.2 } else { 0.0 },
                wave_phase: t * 6.5,
                ..Default::default()
            },
            PreviewClip::Custom(idx) => {
                if let Some(anim) = self.custom_animations.get(idx)
                    && !anim.keyframes.is_empty()
                {
                    let step_s = (anim.interval_ms.max(40) as f32) / 1000.0;
                    let kidx = ((t / step_s) as usize) % anim.keyframes.len();
                    return self.keyframe_to_pose(&anim.keyframes[kidx]);
                }
                PoseParams::default()
            }
        }
    }

    pub fn build_export_artifacts(
        &self,
    ) -> Result<(image::RgbaImage, image::RgbaImage, String), String> {
        let mut poses = Vec::with_capacity(20 + self.custom_animations.len() * 4);
        for i in 0..8 {
            let mut p = self.walk_pose((i as f32 / 8.0) * std::f32::consts::TAU);
            if i == 5 {
                p.face = FaceSlot::Blink;
            }
            poses.push(p);
        }
        poses.push(PoseParams::default());
        poses.push(PoseParams {
            torso_dy: 0.8,
            torso_scale_y: 0.98,
            body_tilt_deg: 1.0,
            face: FaceSlot::Blink,
            ..Default::default()
        });
        poses.push(self.live_pose(PreviewClip::Sit, 0.0));
        poses.push(self.live_pose(PreviewClip::Sit, 0.7));
        poses.push(self.live_pose(PreviewClip::Sleep, 0.2));
        poses.push(self.live_pose(PreviewClip::Sleep, 1.5));
        poses.push(self.live_pose(PreviewClip::Drag, 0.2));
        poses.push(self.live_pose(PreviewClip::Drag, 0.65));
        poses.push(self.live_pose(PreviewClip::Fall, 0.1));
        poses.push(PoseParams {
            front_bias_deg: 8.0,
            back_bias_deg: -8.0,
            whole_dy: 3.5,
            torso_dy: 1.5,
            torso_scale_y: 0.91,
            face: FaceSlot::Blink,
            ..Default::default()
        });
        poses.push(self.live_pose(PreviewClip::Climb, 0.2));
        poses.push(self.live_pose(PreviewClip::Climb, 0.7));

        let mut custom_ranges: Vec<(u32, &CustomAnimation, Vec<usize>)> = Vec::new();
        for (c_idx, anim) in self.custom_animations.iter().enumerate() {
            if anim.keyframes.is_empty() {
                continue;
            }
            let mut fids = Vec::with_capacity(anim.keyframes.len());
            for kf in &anim.keyframes {
                fids.push(poses.len());
                poses.push(self.keyframe_to_pose(kf));
            }
            custom_ranges.push((30 + c_idx as u32, anim, fids));
        }

        let raw_frames: Vec<Pixmap> = poses.iter().map(|&p| self.render_pose(p)).collect();
        let side = raw_frames[0].width() as usize;

        let mut min_x = side;
        let mut min_y = side;
        let mut max_x = 0usize;
        let mut max_y = 0usize;
        let mut any = false;
        for pm in &raw_frames {
            for y in 0..side {
                for x in 0..side {
                    if pm.pixels()[y * side + x].alpha() > 8 {
                        any = true;
                        min_x = min_x.min(x);
                        min_y = min_y.min(y);
                        max_x = max_x.max(x);
                        max_y = max_y.max(y);
                    }
                }
            }
        }
        if !any {
            return Err("Canvas is empty! Paint or import a pet before exporting.".to_string());
        }

        let pad = 4usize;
        let cx0 = min_x.saturating_sub(pad);
        let cy0 = min_y.saturating_sub(pad);
        let cx1 = (max_x + pad).min(side - 1);
        let cy1 = (max_y + pad).min(side - 1);
        let crop_w = (cx1 - cx0 + 1) as u32;
        let crop_h = (cy1 - cy0 + 1) as u32;

        let target_inner = (EXPORT_TILE - 8) as f32;
        let fit_scale =
            (target_inner / crop_w.max(1) as f32).min(target_inner / crop_h.max(1) as f32);
        let dst_w = ((crop_w as f32 * fit_scale).round() as u32).clamp(1, EXPORT_TILE);
        let dst_h = ((crop_h as f32 * fit_scale).round() as u32).clamp(1, EXPORT_TILE);
        let off_x = (EXPORT_TILE - dst_w) / 2;
        let off_y = EXPORT_TILE - dst_h - 4;

        let cols = 5u32;
        let rows = (raw_frames.len() as u32).div_ceil(cols).max(4);
        let mut sheet = image::RgbaImage::new(cols * EXPORT_TILE, rows * EXPORT_TILE);
        let mut walk_bottom_max = 0u32;
        let mut icon_tile = image::RgbaImage::new(EXPORT_TILE, EXPORT_TILE);

        for (idx, pm) in raw_frames.iter().enumerate() {
            let mut cropped = image::RgbaImage::new(crop_w, crop_h);
            for y in 0..crop_h {
                for x in 0..crop_w {
                    let p = pm.pixels()[(cy0 + y as usize) * side + (cx0 + x as usize)];
                    let a = p.alpha();
                    let (r, g, b) = if a == 0 {
                        (0, 0, 0)
                    } else {
                        (
                            ((p.red() as u16 * 255) / a as u16).min(255) as u8,
                            ((p.green() as u16 * 255) / a as u16).min(255) as u8,
                            ((p.blue() as u16 * 255) / a as u16).min(255) as u8,
                        )
                    };
                    cropped.put_pixel(x, y, image::Rgba([r, g, b, a]));
                }
            }

            let mut resized = image::imageops::resize(
                &cropped,
                dst_w,
                dst_h,
                image::imageops::FilterType::Lanczos3,
            );
            // Crisp matte cleanup so exported tiles have zero fuzzy fringe halos
            for px in resized.pixels_mut() {
                if px[3] < 40 {
                    *px = image::Rgba([0, 0, 0, 0]);
                } else if px[3] > 215 {
                    px[3] = 255;
                }
            }

            let mut tile =
                image::RgbaImage::from_pixel(EXPORT_TILE, EXPORT_TILE, image::Rgba([0, 0, 0, 0]));
            image::imageops::overlay(&mut tile, &resized, off_x as i64, off_y as i64);

            if idx < 8 {
                for y in (0..EXPORT_TILE).rev() {
                    if (0..EXPORT_TILE).any(|x| tile.get_pixel(x, y)[3] > 24) {
                        walk_bottom_max = walk_bottom_max.max(y);
                        break;
                    }
                }
            }
            if idx == 8 {
                icon_tile = tile.clone();
            }

            let tx = (idx as u32 % cols) * EXPORT_TILE;
            let ty = (idx as u32 / cols) * EXPORT_TILE;
            image::imageops::overlay(&mut sheet, &tile, tx as i64, ty as i64);
        }

        let ground_y = (walk_bottom_max + 1).clamp(64, EXPORT_TILE);

        let mut png_bytes = Vec::new();
        sheet
            .write_to(&mut Cursor::new(&mut png_bytes), image::ImageFormat::Png)
            .map_err(|e| format!("failed to encode PNG sheet: {e}"))?;
        let b64_png = STANDARD.encode(&png_bytes);

        let esc = |s: &str| {
            s.replace('&', "&amp;")
                .replace('<', "&lt;")
                .replace('>', "&gt;")
        };
        let title = if self.display_name.trim().is_empty() {
            sanitize_pack_id(&self.pack_id)
        } else {
            self.display_name.trim().to_string()
        };
        let author = if self.author.trim().is_empty() {
            "SM".to_string()
        } else {
            self.author.trim().to_string()
        };
        let walk_x = -self.walk_step_px.clamp(2, 20);
        let iv = self.frame_interval_ms.clamp(50, 400);

        let mut custom_walk_next = String::new();
        let mut custom_anim_blocks = String::new();
        for (aid, anim, fids) in &custom_ranges {
            if anim.probability > 0 {
                custom_walk_next.push_str(&format!(
                    "\n    <Next probability=\"{}\">{}</Next>",
                    anim.probability, aid
                ));
            }
            let seq: String = fids.iter().map(|f| format!("<Frame>{f}</Frame>")).collect();
            custom_anim_blocks.push_str(&format!(
                r#"
  <Animation id="{aid}">
    <Name>{name}</Name>
    <Start><X>{sx}</X><Y>{sy}</Y><Interval>{civ}</Interval><Opacity>1</Opacity></Start>
    <End><X>{sx}</X><Y>{sy}</Y><Interval>{civ}</Interval><Opacity>1</Opacity></End>
    <Sequence repeat="{rep}" repeatfrom="0">{seq}</Sequence>
    <Border><Next probability="100">2</Next></Border>
    <Gravity><Next probability="100">5</Next></Gravity>
    <Next probability="100">1</Next>
  </Animation>"#,
                aid = aid,
                name = esc(&anim.name),
                sx = anim.step_x,
                sy = anim.step_y,
                civ = anim.interval_ms.clamp(40, 600),
                rep = anim.repeat,
                seq = seq,
            ));
        }

        let xml = format!(
            r#"<?xml version="1.0" encoding="utf-8"?>
<AnimationXML>
  <Header>
    <Title>{title}</Title>
    <Author>{author}</Author>
    <Version>2.0</Version>
    <Info>Created with DIYPets Studio ({arch})</Info>
  </Header>
  <Image>
    <TilesX>{cols}</TilesX>
    <TilesY>{rows}</TilesY>
    <GroundY>{ground_y}</GroundY>
    <Png>{b64_png}</Png>
  </Image>
  <Spawns>
    <Spawn id="1" probability="100">
      <X>screenW/2</X>
      <Y>0</Y>
      <Next>5</Next>
    </Spawn>
  </Spawns>
  <Animation id="1">
    <Name>walk</Name>
    <Start><X>{walk_x}</X><Y>0</Y><Interval>{iv}</Interval><Opacity>1</Opacity></Start>
    <End><X>{walk_x}</X><Y>0</Y><Interval>{iv}</Interval><Opacity>1</Opacity></End>
    <Sequence repeat="4" repeatfrom="0">
      <Frame>0</Frame><Frame>1</Frame><Frame>2</Frame><Frame>3</Frame>
      <Frame>4</Frame><Frame>5</Frame><Frame>6</Frame><Frame>7</Frame>
    </Sequence>
    <Border>
      <Next probability="80">2</Next>
      <Next probability="20">19</Next>
    </Border>
    <Gravity><Next probability="100">5</Next></Gravity>
    <Next probability="60">1</Next>
    <Next probability="15">12</Next>
    <Next probability="10">13</Next>
    <Next probability="10">2</Next>{custom_walk_next}
  </Animation>
  <Animation id="2">
    <Name>turn</Name>
    <Action>flip</Action>
    <Start><X>0</X><Y>0</Y><Interval>160</Interval><Opacity>1</Opacity></Start>
    <End><X>0</X><Y>0</Y><Interval>160</Interval><Opacity>1</Opacity></End>
    <Sequence repeat="0" repeatfrom="0"><Frame>8</Frame><Frame>9</Frame></Sequence>
    <Gravity><Next probability="100">5</Next></Gravity>
    <Next probability="100">1</Next>
  </Animation>
  <Animation id="4">
    <Name>drag</Name>
    <Start><X>0</X><Y>0</Y><Interval>120</Interval><Opacity>1</Opacity></Start>
    <End><X>0</X><Y>0</Y><Interval>120</Interval><Opacity>1</Opacity></End>
    <Sequence repeat="2" repeatfrom="0"><Frame>14</Frame><Frame>15</Frame></Sequence>
    <Next probability="100">4</Next>
  </Animation>
  <Animation id="5">
    <Name>fall</Name>
    <Start><X>0</X><Y>10</Y><Interval>55</Interval><Opacity>1</Opacity></Start>
    <End><X>0</X><Y>18</Y><Interval>55</Interval><Opacity>1</Opacity></End>
    <Sequence repeat="1" repeatfrom="0"><Frame>16</Frame><Frame>14</Frame></Sequence>
    <Border><Next probability="100">9</Next></Border>
    <Next probability="100">5</Next>
  </Animation>
  <Animation id="9">
    <Name>fall soft</Name>
    <Start><X>0</X><Y>0</Y><Interval>140</Interval><Opacity>1</Opacity></Start>
    <End><X>0</X><Y>0</Y><Interval>140</Interval><Opacity>1</Opacity></End>
    <Sequence repeat="0" repeatfrom="0"><Frame>17</Frame><Frame>8</Frame></Sequence>
    <Next probability="100">1</Next>
  </Animation>
  <Animation id="10">
    <Name>fall hard</Name>
    <Start><X>0</X><Y>0</Y><Interval>160</Interval><Opacity>1</Opacity></Start>
    <End><X>0</X><Y>0</Y><Interval>160</Interval><Opacity>1</Opacity></End>
    <Sequence repeat="0" repeatfrom="0"><Frame>17</Frame><Frame>10</Frame><Frame>8</Frame></Sequence>
    <Next probability="100">1</Next>
  </Animation>
  <Animation id="12">
    <Name>sit</Name>
    <Start><X>0</X><Y>0</Y><Interval>320</Interval><Opacity>1</Opacity></Start>
    <End><X>0</X><Y>0</Y><Interval>320</Interval><Opacity>1</Opacity></End>
    <Sequence repeat="4" repeatfrom="0"><Frame>10</Frame><Frame>11</Frame></Sequence>
    <Gravity><Next probability="100">5</Next></Gravity>
    <Next probability="65">1</Next>
    <Next probability="35">13</Next>
  </Animation>
  <Animation id="13">
    <Name>sleep</Name>
    <Start><X>0</X><Y>0</Y><Interval>450</Interval><Opacity>1</Opacity></Start>
    <End><X>0</X><Y>0</Y><Interval>450</Interval><Opacity>1</Opacity></End>
    <Sequence repeat="5" repeatfrom="0"><Frame>12</Frame><Frame>13</Frame></Sequence>
    <Gravity><Next probability="100">5</Next></Gravity>
    <Next probability="50">13</Next>
    <Next probability="50">1</Next>
  </Animation>
  <Animation id="19">
    <Name>climb</Name>
    <Start><X>0</X><Y>-4</Y><Interval>140</Interval><Opacity>1</Opacity></Start>
    <End><X>0</X><Y>-4</Y><Interval>140</Interval><Opacity>1</Opacity></End>
    <Sequence repeat="4" repeatfrom="0"><Frame>18</Frame><Frame>19</Frame></Sequence>
    <Border><Next probability="100">5</Next></Border>
    <Next probability="60">19</Next>
    <Next probability="40">5</Next>
  </Animation>{custom_anim_blocks}
</AnimationXML>
"#,
            title = esc(&title),
            author = esc(&author),
            arch = self.archetype.label(),
            cols = cols,
            rows = rows,
            ground_y = ground_y,
            b64_png = b64_png,
            walk_x = walk_x,
            iv = iv,
            custom_walk_next = custom_walk_next,
            custom_anim_blocks = custom_anim_blocks,
        );

        DeskpetPack::parse(&xml).map_err(|e| format!("export validation failed: {e}"))?;
        Ok((sheet, icon_tile, xml))
    }

    pub fn export_to_dir(&self, pack_dir: &Path) -> Result<(), String> {
        let (_sheet, icon_tile, xml) = self.build_export_artifacts()?;
        std::fs::create_dir_all(pack_dir)
            .map_err(|e| format!("cannot create {}: {e}", pack_dir.display()))?;
        let tmp = pack_dir.join("deskpet.xml.tmp");
        let dst = pack_dir.join("deskpet.xml");
        std::fs::write(&tmp, &xml).map_err(|e| format!("cannot write xml: {e}"))?;
        if std::fs::rename(&tmp, &dst).is_err() {
            if let Err(e) = std::fs::remove_file(&tmp) { crate::logging::warn(&format!("studio_model: temp file cleanup failed: {e}")); }
            std::fs::write(&dst, &xml).map_err(|e| format!("cannot write xml: {e}"))?;
        }
        icon_tile
            .save(pack_dir.join("icon.png"))
            .map_err(|e| format!("cannot save icon.png: {e}"))?;
        Ok(())
    }

    pub fn save_json(&self, path: &Path) -> Result<(), String> {
        let dto = SavedProjectV2::from_project(self);
        let text = serde_json::to_string_pretty(&dto)
            .map_err(|e| format!("serialize project error: {e}"))?;
        std::fs::write(path, text).map_err(|e| format!("write project error: {e}"))
    }

    pub fn load_json(path: &Path) -> Result<Self, String> {
        let text =
            std::fs::read_to_string(path).map_err(|e| format!("read project error: {e}"))?;
        let dto: SavedProjectV2 =
            serde_json::from_str(&text).map_err(|e| format!("parse project json error: {e}"))?;
        dto.into_project()
    }
}

#[derive(Serialize, Deserialize)]
pub struct SavedPartDto {
    pub name: String,
    pub role: PartRole,
    #[serde(default = "default_true")]
    pub visible: bool,
    #[serde(default)]
    pub paired: bool,
    #[serde(default)]
    pub mirror_far: bool,
    #[serde(default)]
    pub behind_body: bool,
    pub socket_near: (usize, usize),
    pub socket_far: (usize, usize),
    pub pivot: (usize, usize),
    #[serde(default = "default_swing")]
    pub swing_deg: f32,
    #[serde(default)]
    pub phase_offset_deg: f32,
    #[serde(default = "default_one")]
    pub scale: f32,
    pub png_b64: String,
}

fn default_true() -> bool {
    true
}
fn default_swing() -> f32 {
    20.0
}
fn default_one() -> f32 {
    1.0
}
fn default_outline_rgb() -> [u8; 3] {
    [44, 30, 26]
}

#[derive(Serialize, Deserialize)]
pub struct SavedProjectV2 {
    pub version: u32,
    pub display_name: String,
    pub pack_id: String,
    pub author: String,
    pub archetype: Archetype,
    #[serde(default = "default_true")]
    pub shade_far_limbs: bool,
    #[serde(default = "default_true")]
    pub seamless_outline: bool,
    #[serde(default = "default_outline_rgb")]
    pub outline_color: [u8; 3],
    #[serde(default = "default_one")]
    pub global_limb_scale: f32,
    #[serde(default = "default_one")]
    pub global_swing_mult: f32,
    #[serde(default)]
    pub body_bob: f32,
    #[serde(default)]
    pub wave_amplitude: f32,
    pub walk_step_px: i32,
    pub frame_interval_ms: u32,
    pub body_png_b64: String,
    #[serde(default)]
    pub parts: Vec<SavedPartDto>,
    #[serde(default)]
    pub faces_png_b64: Vec<String>,
    #[serde(default)]
    pub custom_animations: Vec<CustomAnimation>,
}

impl SavedProjectV2 {
    pub fn from_project(p: &StudioProject) -> Self {
        Self {
            version: 2,
            display_name: p.display_name.clone(),
            pack_id: p.pack_id.clone(),
            author: p.author.clone(),
            archetype: p.archetype,
            shade_far_limbs: p.shade_far_limbs,
            seamless_outline: p.seamless_outline,
            outline_color: p.outline_color,
            global_limb_scale: p.global_limb_scale,
            global_swing_mult: p.global_swing_mult,
            body_bob: p.body_bob,
            wave_amplitude: p.wave_amplitude,
            walk_step_px: p.walk_step_px,
            frame_interval_ms: p.frame_interval_ms,
            body_png_b64: rgba_buf_to_png_b64(&p.body),
            parts: p
                .parts
                .iter()
                .map(|part| SavedPartDto {
                    name: part.name.clone(),
                    role: part.role,
                    visible: part.visible,
                    paired: part.paired,
                    mirror_far: part.mirror_far,
                    behind_body: part.behind_body,
                    socket_near: part.socket_near,
                    socket_far: part.socket_far,
                    pivot: part.pivot,
                    swing_deg: part.swing_deg,
                    phase_offset_deg: part.phase_offset_deg,
                    scale: part.scale,
                    png_b64: rgba_buf_to_png_b64(&part.pixels),
                })
                .collect(),
            faces_png_b64: p.faces.iter().map(|f| rgba_buf_to_png_b64(f)).collect(),
            custom_animations: p.custom_animations.clone(),
        }
    }

    pub fn into_project(self) -> Result<StudioProject, String> {
        let body = png_b64_to_rgba_buf(&self.body_png_b64)?;
        let mut parts = Vec::with_capacity(self.parts.len());
        for dto in self.parts {
            parts.push(RigPart {
                name: dto.name,
                role: dto.role,
                visible: dto.visible,
                paired: dto.paired,
                mirror_far: dto.mirror_far,
                behind_body: dto.behind_body,
                socket_near: dto.socket_near,
                socket_far: dto.socket_far,
                pivot: dto.pivot,
                swing_deg: dto.swing_deg,
                phase_offset_deg: dto.phase_offset_deg,
                scale: dto.scale,
                pixels: png_b64_to_rgba_buf(&dto.png_b64)?,
            });
        }
        let mut faces: [Vec<RgbaPixel>; 5] = std::array::from_fn(|_| vec![[0, 0, 0, 0]; BOARD_LEN]);
        for (i, b64) in self.faces_png_b64.iter().take(5).enumerate() {
            if !b64.is_empty() {
                faces[i] = png_b64_to_rgba_buf(b64)?;
            }
        }
        Ok(StudioProject {
            display_name: self.display_name,
            pack_id: sanitize_pack_id(&self.pack_id),
            author: self.author,
            archetype: self.archetype,
            shade_far_limbs: self.shade_far_limbs,
            seamless_outline: self.seamless_outline,
            outline_color: self.outline_color,
            global_limb_scale: self.global_limb_scale,
            global_swing_mult: self.global_swing_mult,
            body_bob: self.body_bob,
            wave_amplitude: self.wave_amplitude,
            walk_step_px: self.walk_step_px,
            frame_interval_ms: self.frame_interval_ms,
            body,
            parts,
            faces,
            custom_animations: self.custom_animations,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_five_archetype_templates_export_valid_packs_with_custom_animations() {
        for &arch in Archetype::all() {
            let mut proj = StudioProject::template_by_archetype(arch);
            proj.custom_animations.push(CustomAnimation::dance_preset());
            let (_sheet, _icon, xml) = proj.build_export_artifacts().unwrap_or_else(|e| panic!("{}: {e:?}", arch.label()));
            let pack = DeskpetPack::parse(&xml).unwrap_or_else(|e| panic!("{}: {e:?}", arch.label()));
            assert!(pack.frames.len() >= 24);
            assert!(pack.animations.contains_key("1"));
            assert!(pack.animations.contains_key("19"));
            assert!(pack.animations.contains_key("30"), "missing custom anim 30");
            assert!(pack.animations.contains_key("31"), "missing custom anim 31");
        }
    }

    #[test]
    fn v2_project_json_roundtrip_preserves_parts_faces_and_custom_anims() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("bird.diypet.json");
        let orig = StudioProject::template_bird();
        orig.save_json(&path).unwrap();
        let loaded = StudioProject::load_json(&path).unwrap();
        assert_eq!(loaded.archetype, Archetype::Bird);
        assert_eq!(loaded.parts.len(), orig.parts.len());
        assert_eq!(loaded.custom_animations.len(), orig.custom_animations.len());
        assert_eq!(loaded.body, orig.body);
        assert_eq!(loaded.faces[0], orig.faces[0]);
    }
}