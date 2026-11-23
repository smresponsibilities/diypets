//! Pack catalog & lazy on-demand downloader for eSheep and remote packs.

use crate::deskpet_xml::DeskpetPack;
use crate::settings::{find_pack_xml, thumb_cache_dir, user_packs_dir};
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::Duration;

pub const EMBEDDED_CATALOG_JSON: &str = include_str!("../catalog.json");
pub const DEFAULT_APP_ICON_PNG: &[u8] = include_bytes!("../catalog_icons/highland_cow.png");

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CatalogEntry {
    pub id: String,
    pub name: String,
    pub author: String,
    pub category: String,
    pub size_kb: u64,
    pub xml_url: String,
    pub icon_url: String,
}

impl CatalogEntry {
    pub fn is_made_by_sm(&self) -> bool {
        self.category == "made_by_sm"
    }
}

pub fn load_embedded_catalog() -> Vec<CatalogEntry> {
    serde_json::from_str(EMBEDDED_CATALOG_JSON).unwrap_or_default()
}

pub fn is_pack_installed(id: &str) -> bool {
    find_pack_xml(id).is_some()
}

pub fn is_user_downloaded(id: &str) -> bool {
    user_packs_dir().join(id).join("deskpet.xml").is_file()
}

fn http_agent() -> ureq::Agent {
    ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(10))
        .timeout_read(Duration::from_secs(30))
        .user_agent(concat!("diypets/", env!("CARGO_PKG_VERSION")))
        .build()
}

/// Download a pack's `deskpet.xml` on demand, validate it parses cleanly, and store it
/// atomically in `user_packs_dir()/<id>/deskpet.xml`.
pub fn download_pack(entry: &CatalogEntry) -> Result<PathBuf, String> {
    if entry.id.is_empty()
        || entry.id.contains("..")
        || entry.id.contains('/')
        || entry.id.contains('\\')
    {
        return Err(format!("invalid pack id: {}", entry.id));
    }

    let agent = http_agent();
    let resp = agent
        .get(&entry.xml_url)
        .call()
        .map_err(|e| format!("failed to download {}: {e}", entry.name))?;
    let xml_text = resp
        .into_string()
        .map_err(|e| format!("failed reading {}: {e}", entry.name))?;

    // Validate before touching disk so 404 HTML or corrupt responses never break local packs.
    let pack = DeskpetPack::parse(&xml_text)
        .map_err(|e| format!("downloaded pack {} is invalid XML: {e}", entry.name))?;

    let pack_dir = user_packs_dir().join(&entry.id);
    fs::create_dir_all(&pack_dir)
        .map_err(|e| format!("cannot create {}: {e}", pack_dir.display()))?;

    let xml_path = pack_dir.join("deskpet.xml");
    let tmp_path = pack_dir.join("deskpet.xml.tmp");
    fs::write(&tmp_path, &xml_text)
        .map_err(|e| format!("cannot write {}: {e}", tmp_path.display()))?;
    if fs::rename(&tmp_path, &xml_path).is_err() {
        if let Err(e) = fs::remove_file(&tmp_path) { crate::logging::warn(&format!("catalog: temp cleanup failed {}: {e}", tmp_path.display())); }
        fs::write(&xml_path, &xml_text)
            .map_err(|e| format!("cannot write {}: {e}", xml_path.display()))?;
    }

    // Also save a crisp tile-0 icon.png next to the downloaded pack.
    if let Some(first_frame) = pack.frames.first() {
        let mut rgba = Vec::with_capacity((pack.tile_w * pack.tile_h * 4) as usize);
        for &pixel in first_frame {
            let [a, r, g, b] = pixel.to_be_bytes();
            let straight =
                |ch: u8| (u16::from(ch) * 255 / u16::from(a.max(1))).min(255) as u8;
            rgba.extend_from_slice(&[straight(r), straight(g), straight(b), a]);
        }
        if let Some(img) = image::RgbaImage::from_raw(pack.tile_w, pack.tile_h, rgba)
            && let Err(e) = img.save(pack_dir.join("icon.png"))
        {
            crate::logging::warn(&format!("catalog: failed saving pack icon.png: {e}"));
        }
    }

    Ok(xml_path)
}

/// Lazily download and cache a small preview PNG for a catalog entry when not bundled locally.
pub fn download_and_cache_icon(entry: &CatalogEntry) -> Option<image::RgbaImage> {
    let cache_dir = thumb_cache_dir();
    let cached = cache_dir.join(format!("{}_v2.png", entry.id));
    if cached.is_file()
        && let Ok(img) = image::open(&cached)
    {
        return Some(img.to_rgba8());
    }

    let agent = http_agent();
    let resp = agent.get(&entry.icon_url).call().ok()?;
    let mut bytes = Vec::new();
    resp.into_reader().read_to_end(&mut bytes).ok()?;
    let img = image::load_from_memory(&bytes).ok()?.to_rgba8();
    if let Err(e) = fs::create_dir_all(&cache_dir) { crate::logging::warn(&format!("catalog: failed creating cache dir: {e}")); }
    if let Err(e) = img.save(&cached) { crate::logging::warn(&format!("catalog: failed caching icon {}: {e}", cached.display())); }
    Some(img)
}

/// Remove a user-downloaded pack from `user_packs_dir()` (never deletes bundled repo assets).
pub fn delete_downloaded_pack(id: &str) -> Result<(), String> {
    let dir = user_packs_dir().join(id);
    if dir.is_dir() {
        fs::remove_dir_all(&dir).map_err(|e| format!("cannot remove {}: {e}", dir.display()))?;
    }
    Ok(())
}

/// Strip corner chroma-key background when an image has no alpha transparency.
pub fn strip_chroma_key_if_opaque(img: &mut image::RgbaImage) {
    if img.width() == 0 || img.height() == 0 {
        return;
    }
    if img.pixels().any(|p| p[3] < 250) {
        return;
    }
    let key = *img.get_pixel(0, 0);
    for px in img.pixels_mut() {
        if px[0] == key[0] && px[1] == key[1] && px[2] == key[2] {
            *px = image::Rgba([0, 0, 0, 0]);
        }
    }
}

/// Produce a crisp, tightly framed square RGBA app/tray icon (no weird borders or tiny margins).
pub fn make_square_app_icon(source: &image::RgbaImage, side: u32) -> image::RgbaImage {
    let mut img = source.clone();
    strip_chroma_key_if_opaque(&mut img);
    let (w, h) = img.dimensions();
    let mut min_x = w;
    let mut min_y = h;
    let mut max_x = 0u32;
    let mut max_y = 0u32;
    let mut found = false;
    for y in 0..h {
        for x in 0..w {
            if img.get_pixel(x, y)[3] > 16 {
                found = true;
                min_x = min_x.min(x);
                min_y = min_y.min(y);
                max_x = max_x.max(x);
                max_y = max_y.max(y);
            }
        }
    }
    let cropped = if found && max_x >= min_x && max_y >= min_y {
        image::imageops::crop_imm(&img, min_x, min_y, max_x - min_x + 1, max_y - min_y + 1).to_image()
    } else {
        img
    };
    let (cw, ch) = cropped.dimensions();
    let target = side.max(1);
    let scale = (target as f32 / cw.max(1) as f32).min(target as f32 / ch.max(1) as f32);
    let nw = ((cw as f32 * scale).round() as u32).clamp(1, target);
    let nh = ((ch as f32 * scale).round() as u32).clamp(1, target);
    let resized = image::imageops::resize(&cropped, nw, nh, image::imageops::FilterType::Lanczos3);
    let mut canvas = image::RgbaImage::from_pixel(target, target, image::Rgba([0, 0, 0, 0]));
    let ox = (target - nw) / 2;
    let oy = (target - nh) / 2;
    image::imageops::overlay(&mut canvas, &resized, ox as i64, oy as i64);
    canvas
}

pub fn default_app_icon(target_size: u32) -> Option<image::RgbaImage> {
    let img = image::load_from_memory(DEFAULT_APP_ICON_PNG).ok()?.to_rgba8();
    Some(make_square_app_icon(&img, target_size))
}

/// Open a directory in the OS file manager (Explorer / Finder / xdg-open).
pub fn open_folder_in_os(path: &Path) {
    if let Err(e) = fs::create_dir_all(path) { crate::logging::warn(&format!("catalog: failed creating folder {}: {e}", path.display())); }
    #[cfg(windows)]
    {
        if let Err(e) = std::process::Command::new("explorer").arg(path).spawn() { crate::logging::warn(&format!("catalog: failed launching explorer: {e}")); }
    }
    #[cfg(target_os = "macos")]
    {
        if let Err(e) = std::process::Command::new("open").arg(path).spawn() { crate::logging::warn(&format!("catalog: failed launching open: {e}")); }
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        if let Err(e) = std::process::Command::new("xdg-open").arg(path).spawn() { crate::logging::warn(&format!("catalog: failed launching xdg-open: {e}")); }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_catalog_parses_with_both_rows() {
        let entries = load_embedded_catalog();
        assert!(entries.len() >= 5, "expected bundled originals in catalog");
        assert!(entries.iter().any(|e| e.id == "highland_cow" && e.is_made_by_sm()));
        assert!(entries.iter().any(|e| !e.is_made_by_sm()));
    }

    #[test]
    fn default_app_icon_is_crisp_square_and_non_empty() {
        let icon = default_app_icon(64).expect("default app icon");
        assert_eq!(icon.dimensions(), (64, 64));
        assert!(icon.pixels().any(|p| p[3] > 200));
    }
}