//! Scratch-style Costumes Flipbook helpers & JuiceFX 1-Click Procedural Auto-Animator (T-005).
//!
//! Generates crisp, palette-preserving 8-bit animations from a single static RGBA frame using
//! foot-anchored nearest-neighbor warps so pets keep 0px vertical foot jitter on desktop ledges.

use std::f32::consts::TAU;

use base64::{Engine as _, engine::general_purpose::STANDARD};
use image::{Rgba, RgbaImage};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JuiceEffect {
    WalkBounce,
    IdleBreathe,
    TailEarWave,
    SquashHop,
    MotionSmear,
}

impl JuiceEffect {
    pub fn all() -> &'static [Self] {
        &[
            Self::WalkBounce,
            Self::IdleBreathe,
            Self::TailEarWave,
            Self::SquashHop,
            Self::MotionSmear,
        ]
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::WalkBounce => "Auto-Walk Bounce (6f)",
            Self::IdleBreathe => "Idle Breathing (4f)",
            Self::TailEarWave => "Tail / Ear Wave (6f)",
            Self::SquashHop => "Squash Hop (6f)",
            Self::MotionSmear => "Motion Smear (4f)",
        }
    }

    pub fn default_interval_ms(self) -> u32 {
        match self {
            Self::WalkBounce => 115,
            Self::IdleBreathe => 220,
            Self::TailEarWave => 135,
            Self::SquashHop => 105,
            Self::MotionSmear => 85,
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct PixelWarp {
    scale_x: f32,
    scale_y: f32,
    shift_x: f32,
    lift_y: f32,
    shear_x: f32,
    foot_y: u32,
}

/// Find the opaque bounding box `(min_x, min_y, max_x, max_y)` of an RGBA image.
pub fn opaque_bounds(img: &RgbaImage) -> Option<(u32, u32, u32, u32)> {
    let (w, h) = img.dimensions();
    let mut min_x = w;
    let mut min_y = h;
    let mut max_x = 0u32;
    let mut max_y = 0u32;
    let mut found = false;

    for y in 0..h {
        for x in 0..w {
            if img.get_pixel(x, y)[3] >= 16 {
                min_x = min_x.min(x);
                min_y = min_y.min(y);
                max_x = max_x.max(x);
                max_y = max_y.max(y);
                found = true;
            }
        }
    }
    if found {
        Some((min_x, min_y, max_x, max_y))
    } else {
        None
    }
}

/// Detect the lowest opaque row (`foot_y`) in an image, defaulting near the bottom edge if blank.
pub fn detect_foot_y(img: &RgbaImage) -> u32 {
    opaque_bounds(img)
        .map(|(_, _, _, max_y)| max_y)
        .unwrap_or_else(|| img.height().saturating_sub(6))
}

/// Mirror a costume horizontally without blurring pixels.
pub fn flip_horizontal(img: &RgbaImage) -> RgbaImage {
    let (w, h) = img.dimensions();
    let mut out = RgbaImage::new(w, h);
    for y in 0..h {
        for x in 0..w {
            out.put_pixel(w - 1 - x, y, *img.get_pixel(x, y));
        }
    }
    out
}

/// Vertically shift a costume so its lowest opaque pixels land flush on `target_foot_y`.
pub fn snap_feet_to_baseline(img: &RgbaImage, target_foot_y: u32) -> RgbaImage {
    let Some((_, _, _, cur_foot)) = opaque_bounds(img) else {
        return img.clone();
    };
    let (w, h) = img.dimensions();
    let clamped_target = target_foot_y.min(h.saturating_sub(1));
    let dy = clamped_target as i32 - cur_foot as i32;
    if dy == 0 {
        return img.clone();
    }
    let mut out = RgbaImage::new(w, h);
    for y in 0..h {
        let ny = y as i32 + dy;
        if ny < 0 || ny >= h as i32 {
            continue;
        }
        for x in 0..w {
            out.put_pixel(x, ny as u32, *img.get_pixel(x, y));
        }
    }
    out
}

/// Nearest-neighbor squash/stretch/shear/lift transform anchored cleanly to `(center_x, foot_y)`.
fn warp_foot_anchored(src: &RgbaImage, warp: PixelWarp) -> RgbaImage {
    let (w, h) = src.dimensions();
    let mut out = RgbaImage::new(w, h);
    let Some((min_x, min_y, max_x, max_y)) = opaque_bounds(src) else {
        return out;
    };

    let cx = (min_x + max_x) as f32 * 0.5;
    let body_h = (max_y.saturating_sub(min_y) + 1).max(8) as f32;
    let fy = warp.foot_y.min(h.saturating_sub(1)) as f32;
    let sx = warp.scale_x.clamp(0.4, 2.0);
    let sy = warp.scale_y.clamp(0.4, 2.0);

    for dy in 0..h {
        let y_from_foot = (fy - warp.lift_y) - dy as f32;
        let src_y_f = fy - (y_from_foot / sy);
        let src_y = src_y_f.round() as i32;
        if src_y < 0 || src_y >= h as i32 {
            continue;
        }
        let h_ratio = (y_from_foot / body_h).clamp(0.0, 1.25);
        let shear_at_y = h_ratio * warp.shear_x;

        for dx in 0..w {
            let x_from_cx = dx as f32 - (cx + warp.shift_x) - shear_at_y;
            let src_x_f = cx + (x_from_cx / sx);
            let src_x = src_x_f.round() as i32;
            if src_x < 0 || src_x >= w as i32 {
                continue;
            }
            let px = *src.get_pixel(src_x as u32, src_y as u32);
            if px[3] >= 16 {
                out.put_pixel(dx, dy, px);
            }
        }
    }
    out
}

fn generate_wave_frame(src: &RgbaImage, foot_y: u32, phase: f32, amp_px: f32) -> RgbaImage {
    let (w, h) = src.dimensions();
    let mut out = RgbaImage::new(w, h);
    let Some((_, min_y, _, max_y)) = opaque_bounds(src) else {
        return out;
    };
    let body_h = (max_y.saturating_sub(min_y) + 1).max(8) as f32;
    let fy = foot_y.min(h.saturating_sub(1)) as f32;

    for y in 0..h {
        let h_ratio = ((fy - y as f32) / body_h).clamp(0.0, 1.0);
        // Keep the bottom 28% (feet/ankles) completely anchored to the ledge.
        let dx_off = if h_ratio <= 0.28 {
            0.0
        } else {
            let active = (h_ratio - 0.28) / 0.72;
            (phase + active * 4.2).sin() * (active * active) * amp_px
        };

        for x in 0..w {
            let sx = (x as f32 - dx_off).round() as i32;
            if sx < 0 || sx >= w as i32 {
                continue;
            }
            let px = *src.get_pixel(sx as u32, y);
            if px[3] >= 16 {
                out.put_pixel(x, y, px);
            }
        }
    }
    out
}

fn add_motion_smear(base: &RgbaImage, trail_px: u32, dir_right: bool) -> RgbaImage {
    let (w, h) = base.dimensions();
    let mut out = base.clone();
    if trail_px == 0 {
        return out;
    }

    for y in 0..h {
        // Alternate scanline smear lengths for crisp retro 8-bit dash streaks.
        let row_trail = if y % 3 == 0 {
            trail_px
        } else if y % 3 == 1 {
            (trail_px * 2) / 3
        } else {
            trail_px / 2
        };
        if row_trail == 0 {
            continue;
        }
        for x in 0..w {
            let px = *base.get_pixel(x, y);
            if px[3] < 16 {
                continue;
            }
            for step in 1..=row_trail {
                let tx = if dir_right {
                    x as i32 - step as i32
                } else {
                    x as i32 + step as i32
                };
                if tx < 0 || tx >= w as i32 {
                    continue;
                }
                let ux = tx as u32;
                if out.get_pixel(ux, y)[3] < 16 {
                    // Crisp desaturated/brightened speed-line tint (opaque alpha for clean GIFs)
                    let lift = (step as u16 * 18).min(90) as u8;
                    let smear = Rgba([
                        px[0].saturating_add(lift),
                        px[1].saturating_add(lift),
                        px[2].saturating_add(lift),
                        255,
                    ]);
                    out.put_pixel(ux, y, smear);
                }
            }
        }
    }
    out
}

/// Generate a complete looping sequence of crisp pixel-art frames from a single static costume.
pub fn generate_juice_frames(
    source: &RgbaImage,
    effect: JuiceEffect,
    foot_y: u32,
    strength: f32,
) -> Vec<RgbaImage> {
    let k = strength.clamp(0.4, 2.2);
    match effect {
        JuiceEffect::WalkBounce => {
            let steps = 6usize;
            let mut frames = Vec::with_capacity(steps);
            for i in 0..steps {
                let phase = (i as f32 / steps as f32) * TAU;
                let bounce = phase.sin().abs();
                let stride = phase.cos();
                let warp = PixelWarp {
                    scale_x: 1.0 + (0.05 * (1.0 - bounce) - 0.03 * bounce) * k,
                    scale_y: 1.0 + (0.06 * bounce - 0.04 * (1.0 - bounce)) * k,
                    shift_x: stride * 2.2 * k,
                    lift_y: (bounce * 3.5 * k).round(),
                    shear_x: stride * 2.6 * k,
                    foot_y,
                };
                frames.push(warp_foot_anchored(source, warp));
            }
            frames
        }
        JuiceEffect::IdleBreathe => {
            let factors = [0.0f32, 0.7, 1.0, 0.4];
            factors
                .iter()
                .map(|&f| {
                    let sy = 1.0 + (f * 0.045 * k);
                    let sx = 1.0 - (f * 0.025 * k);
                    warp_foot_anchored(
                        source,
                        PixelWarp {
                            scale_x: sx,
                            scale_y: sy,
                            shift_x: 0.0,
                            lift_y: 0.0,
                            shear_x: 0.0,
                            foot_y,
                        },
                    )
                })
                .collect()
        }
        JuiceEffect::TailEarWave => {
            let steps = 6usize;
            let mut frames = Vec::with_capacity(steps);
            for i in 0..steps {
                let phase = (i as f32 / steps as f32) * TAU;
                frames.push(generate_wave_frame(source, foot_y, phase, 4.2 * k));
            }
            frames
        }
        JuiceEffect::SquashHop => {
            // Anticipation crouch -> launch stretch -> apex -> fall -> landing squash -> settle
            let spec: [(f32, f32, f32, f32); 6] = [
                (1.12, 0.86, 0.0, 0.0),
                (0.88, 1.15, 5.0, 1.5),
                (0.96, 1.05, 11.0, 0.0),
                (0.90, 1.12, 5.0, -1.5),
                (1.15, 0.82, 0.0, 0.0),
                (1.00, 1.00, 0.0, 0.0),
            ];
            spec.iter()
                .map(|&(sx, sy, lift, shear)| {
                    warp_foot_anchored(
                        source,
                        PixelWarp {
                            scale_x: 1.0 + (sx - 1.0) * k,
                            scale_y: 1.0 + (sy - 1.0) * k,
                            shift_x: 0.0,
                            lift_y: (lift * k).round(),
                            shear_x: shear * k,
                            foot_y,
                        },
                    )
                })
                .collect()
        }
        JuiceEffect::MotionSmear => {
            let spec: [(f32, f32, f32, u32); 4] = [
                (1.06, 0.94, 4.0, 5),
                (1.10, 0.92, 6.0, 8),
                (1.06, 0.94, 4.0, 5),
                (1.00, 1.00, 1.0, 2),
            ];
            spec.iter()
                .map(|&(sx, sy, shear, trail)| {
                    let warped = warp_foot_anchored(
                        source,
                        PixelWarp {
                            scale_x: 1.0 + (sx - 1.0) * k,
                            scale_y: 1.0 + (sy - 1.0) * k,
                            shift_x: (shear * 0.4 * k).round(),
                            lift_y: 0.0,
                            shear_x: -shear * k,
                            foot_y,
                        },
                    );
                    let scaled_trail = ((trail as f32 * k).round() as u32).clamp(1, 14);
                    add_motion_smear(&warped, scaled_trail, false)
                })
                .collect()
        }
    }
}

/// Pack a Scratch-style Costumes flipbook into a validated DIYPets `deskpet.xml` + `icon.png`.
pub fn build_costumes_pack_xml(
    pack_id: &str,
    author: &str,
    frames: &[RgbaImage],
    interval_ms: u32,
    foot_y: u32,
) -> Result<(String, RgbaImage), String> {
    if frames.is_empty() {
        return Err("Add at least 1 costume frame before exporting.".to_string());
    }
    let tile = frames[0].width().max(frames[0].height()).clamp(16, 256);
    let count = frames.len() as u32;
    let cols = count.min(8);
    let rows = count.div_ceil(cols);

    let mut atlas = RgbaImage::new(cols * tile, rows * tile);
    for (idx, fr) in frames.iter().enumerate() {
        let fitted = if fr.width() == tile && fr.height() == tile {
            fr.clone()
        } else {
            image::imageops::resize(fr, tile, tile, image::imageops::FilterType::Nearest)
        };
        let cx = (idx as u32 % cols) * tile;
        let cy = (idx as u32 / cols) * tile;
        image::imageops::replace(&mut atlas, &fitted, cx as i64, cy as i64);
    }

    let mut png_bytes = Vec::new();
    atlas
        .write_to(
            &mut std::io::Cursor::new(&mut png_bytes),
            image::ImageFormat::Png,
        )
        .map_err(|e| format!("PNG encode failed: {e}"))?;
    let b64 = STANDARD.encode(&png_bytes);

    let all_frames: String = (0..count).map(|f| format!("<Frame>{f}</Frame>")).collect();
    let idle_frames = if count >= 2 {
        "<Frame>0</Frame><Frame>1</Frame>".to_string()
    } else {
        "<Frame>0</Frame>".to_string()
    };
    let last_frame = format!("<Frame>{}</Frame>", count - 1);
    let ground_y = (foot_y + 1).clamp(1, tile);
    let iv = interval_ms.clamp(50, 500);

    let xml = format!(
        "<?xml version=\"1.0\" encoding=\"utf-8\"?>\n\
         <AnimationXML>\n\
           <Header><Title>{pack_id}</Title><Author>{author}</Author></Header>\n\
           <Image><TilesX>{cols}</TilesX><TilesY>{rows}</TilesY><GroundY>{ground_y}</GroundY><Png>{b64}</Png></Image>\n\
           <Spawns><Spawn id=\"1\" probability=\"100\"><X>screenW/2</X><Y>0</Y><Next>5</Next></Spawn></Spawns>\n\
           <Animation id=\"1\"><Name>walk</Name><Start><X>-3</X><Y>0</Y><Interval>{iv}</Interval><Opacity>1</Opacity></Start><End><X>-3</X><Y>0</Y><Interval>{iv}</Interval><Opacity>1</Opacity></End><Sequence repeat=\"4\" repeatfrom=\"0\">{all_frames}</Sequence><Border><Next probability=\"100\">7</Next></Border><Gravity><Next probability=\"100\">5</Next></Gravity><Next probability=\"75\">1</Next><Next probability=\"25\">2</Next></Animation>\n\
           <Animation id=\"2\"><Name>idle</Name><Start><X>0</X><Y>0</Y><Interval>280</Interval><Opacity>1</Opacity></Start><End><X>0</X><Y>0</Y><Interval>280</Interval><Opacity>1</Opacity></End><Sequence repeat=\"3\" repeatfrom=\"0\">{idle_frames}</Sequence><Gravity><Next probability=\"100\">5</Next></Gravity><Next probability=\"85\">1</Next><Next probability=\"15\">13</Next></Animation>\n\
           <Animation id=\"4\"><Name>drag</Name><Start><X>0</X><Y>0</Y><Interval>130</Interval><Opacity>1</Opacity></Start><End><X>0</X><Y>0</Y><Interval>130</Interval><Opacity>1</Opacity></End><Sequence repeat=\"2\" repeatfrom=\"0\">{last_frame}</Sequence><Next probability=\"100\">4</Next></Animation>\n\
           <Animation id=\"5\"><Name>fall</Name><Start><X>0</X><Y>12</Y><Interval>60</Interval><Opacity>1</Opacity></Start><End><X>0</X><Y>18</Y><Interval>60</Interval><Opacity>1</Opacity></End><Sequence repeat=\"1\" repeatfrom=\"0\">{last_frame}</Sequence><Border><Next probability=\"100\">9</Next></Border><Next probability=\"100\">5</Next></Animation>\n\
           <Animation id=\"7\"><Name>turn</Name><Action>flip</Action><Start><X>0</X><Y>0</Y><Interval>150</Interval><Opacity>1</Opacity></Start><End><X>0</X><Y>0</Y><Interval>150</Interval><Opacity>1</Opacity></End><Sequence repeat=\"0\" repeatfrom=\"0\"><Frame>0</Frame></Sequence><Next probability=\"100\">1</Next></Animation>\n\
           <Animation id=\"9\"><Name>fall soft</Name><Start><X>0</X><Y>0</Y><Interval>140</Interval><Opacity>1</Opacity></Start><End><X>0</X><Y>0</Y><Interval>140</Interval><Opacity>1</Opacity></End><Sequence repeat=\"0\" repeatfrom=\"0\"><Frame>0</Frame></Sequence><Next probability=\"100\">1</Next></Animation>\n\
           <Animation id=\"12\"><Name>sit</Name><Start><X>0</X><Y>0</Y><Interval>350</Interval><Opacity>1</Opacity></Start><End><X>0</X><Y>0</Y><Interval>350</Interval><Opacity>1</Opacity></End><Sequence repeat=\"3\" repeatfrom=\"0\"><Frame>0</Frame></Sequence><Gravity><Next probability=\"100\">5</Next></Gravity><Next probability=\"100\">1</Next></Animation>\n\
           <Animation id=\"13\"><Name>sleep</Name><Start><X>0</X><Y>0</Y><Interval>420</Interval><Opacity>1</Opacity></Start><End><X>0</X><Y>0</Y><Interval>420</Interval><Opacity>1</Opacity></End><Sequence repeat=\"5\" repeatfrom=\"0\"><Frame>0</Frame></Sequence><Gravity><Next probability=\"100\">5</Next></Gravity><Next probability=\"100\">2</Next></Animation>\n\
         </AnimationXML>\n"
    );

    crate::deskpet_xml::DeskpetPack::parse(&xml)
        .map_err(|e| format!("Costume pack validation failed: {e}"))?;

    let icon = image::imageops::resize(&frames[0], 64, 64, image::imageops::FilterType::Nearest);
    Ok((xml, icon))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_pet_sprite() -> RgbaImage {
        let mut img = RgbaImage::new(64, 64);
        let body = Rgba([230, 140, 60, 255]);
        let ear = Rgba([180, 90, 40, 255]);
        // Ears near top
        for y in 12..22 {
            for x in 20..26 {
                img.put_pixel(x, y, ear);
            }
            for x in 38..44 {
                img.put_pixel(x, y, ear);
            }
        }
        // Body + feet flush at y = 55
        for y in 22..56 {
            for x in 20..44 {
                img.put_pixel(x, y, body);
            }
        }
        img
    }

    #[test]
    fn all_five_juice_effects_generate_crisp_non_empty_loops() {
        let base = sample_pet_sprite();
        let foot = detect_foot_y(&base);
        assert_eq!(foot, 55);

        for &effect in JuiceEffect::all() {
            let frames = generate_juice_frames(&base, effect, foot, 1.0);
            assert!(frames.len() >= 4, "{effect:?} should produce >= 4 frames");
            for (idx, fr) in frames.iter().enumerate() {
                assert_eq!(fr.dimensions(), (64, 64));
                assert!(
                    opaque_bounds(fr).is_some(),
                    "{effect:?} frame {idx} must be non-empty"
                );
            }
        }
    }

    #[test]
    fn idle_breathing_and_tail_wave_keep_feet_locked_at_baseline() {
        let base = sample_pet_sprite();
        let foot = detect_foot_y(&base);

        for effect in [JuiceEffect::IdleBreathe, JuiceEffect::TailEarWave] {
            let frames = generate_juice_frames(&base, effect, foot, 1.0);
            for (idx, fr) in frames.iter().enumerate() {
                let f_y = detect_foot_y(fr);
                assert_eq!(
                    f_y, foot,
                    "{effect:?} frame {idx} drifted vertically (got {f_y}, expected {foot})"
                );
            }
        }
    }

    #[test]
    fn snap_feet_and_flip_horizontal_preserve_crisp_pixels() {
        let base = sample_pet_sprite();
        let snapped = snap_feet_to_baseline(&base, 60);
        assert_eq!(detect_foot_y(&snapped), 60);

        let flipped = flip_horizontal(&base);
        assert_eq!(detect_foot_y(&flipped), detect_foot_y(&base));
        assert_eq!(flipped.get_pixel(63 - 20, 30), base.get_pixel(20, 30));
    }

    #[test]
    fn motion_smear_adds_trailing_pixels_beyond_original_width() {
        let base = sample_pet_sprite();
        let foot = detect_foot_y(&base);
        let (_, _, base_max_x, _) = opaque_bounds(&base).unwrap();
        let frames = generate_juice_frames(&base, JuiceEffect::MotionSmear, foot, 1.2);
        let max_trail_x = frames
            .iter()
            .filter_map(opaque_bounds)
            .map(|(_, _, mx, _)| mx)
            .max()
            .unwrap_or(0);
        assert!(max_trail_x > base_max_x);
    }

    #[test]
    fn build_costumes_pack_xml_produces_parseable_deskpet_pack() {
        let base = sample_pet_sprite();
        let foot = detect_foot_y(&base);
        let frames = generate_juice_frames(&base, JuiceEffect::WalkBounce, foot, 1.0);
        let (xml, icon) = build_costumes_pack_xml("juice_test", "SM", &frames, 120, foot)
            .expect("costumes XML export should validate");
        assert_eq!(icon.dimensions(), (64, 64));
        assert!(xml.contains("<Title>juice_test</Title>"));
    }
}
