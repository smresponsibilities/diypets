//! Lossless pixel-art animated GIF exporter for DIYPets Studio and CLI (T-007).
//!
//! Uses an exact indexed color table whenever a frame has `<= 255` unique opaque colors
//! (avoiding NeuQuant color drift) and always sets `DisposalMethod::Background` so
//! transparent sprites never smear or leave ghost trails across frames.

use std::borrow::Cow;
use std::collections::HashMap;

use base64::{Engine as _, engine::general_purpose::STANDARD};
use image::{Rgba, RgbaImage};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GifExportConfig {
    /// Integer nearest-neighbor scale factor (`1..=4`).
    pub scale: u32,
    /// `None` exports a true transparent GIF; `Some([r, g, b])` renders an opaque social card.
    pub background: Option<[u8; 3]>,
    /// Optional fixed delay in milliseconds; when `None`, uses each frame's authored interval.
    pub frame_delay_ms: Option<u32>,
    /// Uniform padding in unscaled pixels around each frame.
    pub pad_px: u32,
}

impl Default for GifExportConfig {
    fn default() -> Self {
        Self {
            scale: 2,
            background: None,
            frame_delay_ms: None,
            pad_px: 0,
        }
    }
}

impl GifExportConfig {
    pub fn transparent(scale: u32) -> Self {
        Self {
            scale: scale.clamp(1, 4),
            background: None,
            frame_delay_ms: None,
            pad_px: 0,
        }
    }

    pub fn social_card(scale: u32) -> Self {
        Self {
            scale: scale.clamp(1, 4),
            background: Some([23, 44, 50]),
            frame_delay_ms: None,
            pad_px: 8,
        }
    }
}

fn prepare_frame_canvas(src: &RgbaImage, cfg: &GifExportConfig) -> RgbaImage {
    let scale = cfg.scale.clamp(1, 4);
    let pad = cfg.pad_px;
    let sw = src.width();
    let sh = src.height();
    let out_w = (sw + pad * 2) * scale;
    let out_h = (sh + pad * 2) * scale;

    let fill = match cfg.background {
        Some([r, g, b]) => Rgba([r, g, b, 255]),
        None => Rgba([0, 0, 0, 0]),
    };
    let mut canvas = RgbaImage::from_pixel(out_w, out_h, fill);

    for sy in 0..sh {
        for sx in 0..sw {
            let px = *src.get_pixel(sx, sy);
            let blended = if px[3] < 128 {
                fill
            } else {
                Rgba([px[0], px[1], px[2], 255])
            };
            let base_x = (sx + pad) * scale;
            let base_y = (sy + pad) * scale;
            for dy in 0..scale {
                for dx in 0..scale {
                    canvas.put_pixel(base_x + dx, base_y + dy, blended);
                }
            }
        }
    }
    canvas
}

fn build_gif_frame(canvas: &RgbaImage, delay_ms: u32, transparent_bg: bool) -> gif::Frame<'static> {
    let width = canvas.width() as u16;
    let height = canvas.height() as u16;
    let delay_cs = delay_ms.div_ceil(10).clamp(2, 500) as u16;

    // Attempt exact lossless palette first (index 0 reserved for transparency when transparent_bg).
    let mut color_to_idx: HashMap<[u8; 3], u8> = HashMap::new();
    let mut palette: Vec<u8> = Vec::new();
    let mut overflow = false;

    if transparent_bg {
        palette.extend_from_slice(&[0, 0, 0]);
    }

    let mut indices = Vec::with_capacity((canvas.width() * canvas.height()) as usize);
    for y in 0..canvas.height() {
        for x in 0..canvas.width() {
            let px = *canvas.get_pixel(x, y);
            if transparent_bg && px[3] < 128 {
                indices.push(0);
                continue;
            }
            let rgb = [px[0], px[1], px[2]];
            if let Some(&idx) = color_to_idx.get(&rgb) {
                indices.push(idx);
            } else {
                let next_idx = palette.len() / 3;
                if next_idx >= 256 {
                    overflow = true;
                    break;
                }
                let idx_u8 = next_idx as u8;
                color_to_idx.insert(rgb, idx_u8);
                palette.extend_from_slice(&rgb);
                indices.push(idx_u8);
            }
        }
        if overflow {
            break;
        }
    }

    if !overflow {
        // GIF color tables require at least 2 entries (6 bytes).
        while palette.len() < 6 {
            palette.extend_from_slice(&[0, 0, 0]);
        }
        return gif::Frame {
            width,
            height,
            delay: delay_cs,
            dispose: gif::DisposalMethod::Background,
            transparent: if transparent_bg { Some(0) } else { None },
            palette: Some(palette),
            buffer: Cow::Owned(indices),
            ..gif::Frame::default()
        };
    }

    // High-color fallback (> 255 unique colors in a single frame).
    let mut rgba_bytes = canvas.as_raw().clone();
    let mut frame = gif::Frame::from_rgba_speed(width, height, &mut rgba_bytes, 10);
    frame.delay = delay_cs;
    frame.dispose = gif::DisposalMethod::Background;
    frame
}

/// Encode a sequence of RGBA frames into an infinitely looping GIF89a byte vector.
pub fn encode_animated_gif(
    frames: &[RgbaImage],
    delays_ms: &[u32],
    cfg: &GifExportConfig,
) -> Result<Vec<u8>, String> {
    if frames.is_empty() {
        return Err("Cannot export GIF: no frames provided".to_string());
    }

    let prepared: Vec<RgbaImage> = frames
        .iter()
        .map(|f| prepare_frame_canvas(f, cfg))
        .collect();
    let w = prepared[0].width();
    let h = prepared[0].height();
    if w == 0 || h == 0 || w > u16::MAX as u32 || h > u16::MAX as u32 {
        return Err(format!("Unsupported GIF dimensions: {w}x{h}"));
    }

    let mut bytes = Vec::new();
    {
        let mut encoder = gif::Encoder::new(&mut bytes, w as u16, h as u16, &[])
            .map_err(|e| format!("GIF encoder init failed: {e}"))?;
        encoder
            .set_repeat(gif::Repeat::Infinite)
            .map_err(|e| format!("GIF repeat header failed: {e}"))?;

        let transparent_bg = cfg.background.is_none();
        for (idx, canvas) in prepared.iter().enumerate() {
            let default_ms = delays_ms.get(idx).copied().unwrap_or(140);
            let delay_ms = cfg.frame_delay_ms.unwrap_or(default_ms);
            let frame = build_gif_frame(canvas, delay_ms, transparent_bg);
            encoder
                .write_frame(&frame)
                .map_err(|e| format!("GIF frame {idx} write failed: {e}"))?;
        }
    }
    Ok(bytes)
}

fn parse_xml_anim_frames(xml: &str, anim_id: &str) -> Result<(u32, u32, Vec<u32>, u32), String> {
    let doc = roxmltree::Document::parse(xml).map_err(|e| format!("XML parse error: {e}"))?;
    let mut tiles_x = 1u32;
    let mut tiles_y = 1u32;

    for node in doc.descendants() {
        if node.has_tag_name("TilesX")
            && let Some(t) = node.text()
            && let Ok(v) = t.trim().parse::<u32>()
        {
            tiles_x = v.max(1);
        } else if node.has_tag_name("TilesY")
            && let Some(t) = node.text()
            && let Ok(v) = t.trim().parse::<u32>()
        {
            tiles_y = v.max(1);
        }
    }

    let mut chosen_anim = None;
    let mut first_anim = None;
    for node in doc.descendants() {
        if node.has_tag_name("Animation") {
            if first_anim.is_none() {
                first_anim = Some(node);
            }
            if node.attribute("id") == Some(anim_id) {
                chosen_anim = Some(node);
                break;
            }
        }
    }

    let anim_node = chosen_anim
        .or(first_anim)
        .ok_or_else(|| "No <Animation> found in deskpet.xml".to_string())?;

    let mut frame_ids = Vec::new();
    let mut interval_ms = 140u32;
    for child in anim_node.descendants() {
        if child.has_tag_name("Frame")
            && let Some(t) = child.text()
            && let Ok(fid) = t.trim().parse::<u32>()
        {
            frame_ids.push(fid);
        } else if child.has_tag_name("Interval")
            && let Some(t) = child.text()
            && let Ok(iv) = t.trim().parse::<u32>()
        {
            interval_ms = iv.clamp(40, 1000);
        }
    }

    if frame_ids.is_empty() {
        frame_ids.push(0);
    }
    Ok((tiles_x, tiles_y, frame_ids, interval_ms))
}

/// Export an animation loop (`anim_id`, e.g. `"1"` for walk) from a pre-rendered sprite sheet + XML.
pub fn export_sheet_and_xml_gif(
    sheet: &RgbaImage,
    xml: &str,
    anim_id: &str,
    cfg: &GifExportConfig,
) -> Result<Vec<u8>, String> {
    let (tiles_x, tiles_y, frame_ids, interval_ms) = parse_xml_anim_frames(xml, anim_id)?;
    let tile_w = sheet.width() / tiles_x;
    let tile_h = sheet.height() / tiles_y;
    if tile_w == 0 || tile_h == 0 {
        return Err("Sprite sheet tile dimensions are zero".to_string());
    }

    let total_tiles = tiles_x * tiles_y;
    let mut frames = Vec::with_capacity(frame_ids.len());
    let mut delays = Vec::with_capacity(frame_ids.len());

    for fid in frame_ids {
        let safe_fid = fid % total_tiles;
        let col = safe_fid % tiles_x;
        let row = safe_fid / tiles_x;
        let tile = image::imageops::crop_imm(sheet, col * tile_w, row * tile_h, tile_w, tile_h)
            .to_image();
        frames.push(tile);
        delays.push(interval_ms);
    }

    encode_animated_gif(&frames, &delays, cfg)
}

/// Export an animation loop directly from a standalone `deskpet.xml` string containing `<Png>`.
pub fn export_xml_pack_gif(
    xml: &str,
    anim_id: &str,
    cfg: &GifExportConfig,
) -> Result<Vec<u8>, String> {
    let doc = roxmltree::Document::parse(xml).map_err(|e| format!("XML parse error: {e}"))?;
    let mut png_b64 = None;
    for node in doc.descendants() {
        if node.has_tag_name("Png")
            && let Some(t) = node.text()
        {
            png_b64 = Some(t.trim().to_string());
            break;
        }
    }
    let b64 = png_b64.ok_or_else(|| "Missing <Png> element in deskpet.xml".to_string())?;
    let clean_b64: String = b64.chars().filter(|c| !c.is_whitespace()).collect();
    let png_bytes = STANDARD
        .decode(clean_b64)
        .map_err(|e| format!("Base64 PNG decode failed: {e}"))?;
    let sheet = image::load_from_memory(&png_bytes)
        .map_err(|e| format!("PNG image decode failed: {e}"))?
        .to_rgba8();

    export_sheet_and_xml_gif(&sheet, xml, anim_id, cfg)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_frame(offset: u32, color: Rgba<u8>) -> RgbaImage {
        let mut img = RgbaImage::new(16, 16);
        for y in 4..12 {
            for x in (2 + offset)..(8 + offset) {
                img.put_pixel(x, y, color);
            }
        }
        img
    }

    #[test]
    fn lossless_transparent_gif_preserves_frame_count_and_disposal() {
        let frames = vec![
            make_frame(0, Rgba([240, 120, 40, 255])),
            make_frame(1, Rgba([240, 120, 40, 255])),
            make_frame(2, Rgba([240, 120, 40, 255])),
            make_frame(3, Rgba([240, 120, 40, 255])),
        ];
        let delays = vec![120, 120, 120, 120];
        let bytes = encode_animated_gif(&frames, &delays, &GifExportConfig::transparent(2))
            .expect("transparent GIF should encode");
        assert!(bytes.starts_with(b"GIF89a"));

        let mut opts = gif::DecodeOptions::new();
        opts.set_color_output(gif::ColorOutput::Indexed);
        let mut dec = opts
            .read_info(std::io::Cursor::new(&bytes))
            .expect("GIF should decode cleanly");
        assert_eq!(dec.width(), 32);
        assert_eq!(dec.height(), 32);

        let mut count = 0usize;
        while let Some(fr) = dec.read_next_frame().expect("frame decode ok") {
            assert_eq!(fr.dispose, gif::DisposalMethod::Background);
            assert_eq!(fr.transparent, Some(0));
            assert_eq!(fr.delay, 12);
            count += 1;
        }
        assert_eq!(count, 4);
    }

    #[test]
    fn high_color_frame_falls_back_cleanly_without_error() {
        let mut img = RgbaImage::new(20, 20);
        for y in 0..20 {
            for x in 0..20 {
                img.put_pixel(x, y, Rgba([(x * 12) as u8, (y * 12) as u8, ((x + y) * 5) as u8, 255]));
            }
        }
        let bytes = encode_animated_gif(&[img], &[150], &GifExportConfig::social_card(1))
            .expect("high-color fallback should succeed");
        assert!(bytes.starts_with(b"GIF89a"));
    }

    #[test]
    fn export_bundled_highland_cow_xml_produces_valid_walk_gif() {
        let xml = include_str!("../assets/highland_cow/deskpet.xml");
        let bytes = export_xml_pack_gif(xml, "1", &GifExportConfig::transparent(1))
            .expect("highland_cow walk GIF export should succeed");
        assert!(bytes.starts_with(b"GIF89a"));
        assert!(bytes.len() > 200);
    }
}
