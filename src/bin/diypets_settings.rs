#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use diypets::agent_api;
use diypets::autostart;
use diypets::catalog::{
    CatalogEntry, default_app_icon, delete_downloaded_pack, download_and_cache_icon, download_pack,
    is_pack_installed, is_user_downloaded, load_embedded_catalog, make_square_app_icon,
    open_folder_in_os, strip_chroma_key_if_opaque,
};
use diypets::deskpet_xml::DeskpetPack;
use diypets::settings::{
    MAX_ACTIVE_PETS, PET_SIZE_MAX, PET_SIZE_MIN, Settings, WALK_SPEED_MAX, WALK_SPEED_MIN,
    catalog_icons_dir, find_pack_icon, find_pack_xml, pack_search_dirs, resolve_project_root,
    sibling_binary, thumb_cache_dir, user_packs_dir,
};
use diypets::studio_model::Archetype;
use diypets::svg_import::import_svg_to_project;
use diypets::ui_toast::ToastQueue;
use eframe::egui;
use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const MAX_THUMB_DIM: u32 = 128;
const REFRESH_INTERVAL: Duration = Duration::from_secs(2);

const BUNDLED_ORIGINALS: &[&str] = &["highland_cow", "dog", "polar_bear", "goat", "stegosaurus"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SettingsTab {
    Gallery,
    Herd,
    Controls,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PackCategory {
    MadeBySM,
    ESheep,
}

fn classify_local_pack(id: &str, pack_dir: &Path, author: &str) -> PackCategory {
    if BUNDLED_ORIGINALS.contains(&id)
        || pack_dir.join("meta.toml").is_file()
        || pack_dir.join("strip.png").is_file()
    {
        return PackCategory::MadeBySM;
    }
    let author_lower = author.trim().to_lowercase();
    if author_lower == "sm"
        || author_lower.contains("made by sm")
        || author_lower.contains("made by me")
        || author_lower.contains("creator")
        || author_lower.contains("custom")
        || author_lower.contains("diypet")
    {
        return PackCategory::MadeBySM;
    }
    PackCategory::MadeBySM
}

fn find_tag_value(xml_prefix: &str, tag_names: &[&str]) -> Option<String> {
    let lower = xml_prefix.to_lowercase();
    for tag in tag_names {
        let open_needle = format!("<{}", tag.to_lowercase());
        let close_needle = format!("</{}>", tag.to_lowercase());
        if let Some(start_tag) = lower.find(&open_needle) {
            let after_open = &lower[start_tag + open_needle.len()..];
            if let Some(gt_rel) = after_open.find('>') {
                let content_start = start_tag + open_needle.len() + gt_rel + 1;
                if let Some(end_rel) = lower[content_start..].find(&close_needle) {
                    let raw = xml_prefix[content_start..content_start + end_rel].trim();
                    let cleaned = raw
                        .strip_prefix("<![CDATA[")
                        .and_then(|s| s.strip_suffix("]]>"))
                        .unwrap_or(raw)
                        .trim();
                    if !cleaned.is_empty() {
                        return Some(cleaned.to_string());
                    }
                }
            }
        }
    }
    None
}

fn read_xml_metadata_fast(xml_path: &Path, fallback_id: &str) -> (String, String) {
    let Ok(mut file) = File::open(xml_path) else {
        return (fallback_id.to_string(), String::new());
    };
    let mut buf = vec![0u8; 16 * 1024];
    let n = file.read(&mut buf).unwrap_or(0);
    let prefix = String::from_utf8_lossy(&buf[..n]);
    let head = match prefix.to_lowercase().find("<png") {
        Some(idx) => &prefix[..idx],
        None => &prefix,
    };

    let name = find_tag_value(head, &["title", "name"]).unwrap_or_else(|| fallback_id.to_string());
    let author = find_tag_value(head, &["author"]).unwrap_or_default();
    (name, author)
}

fn rgba_to_color_image(image: &image::RgbaImage) -> egui::ColorImage {
    let size = [image.width() as usize, image.height() as usize];
    let pixels = image.as_flat_samples();
    egui::ColorImage::from_rgba_unmultiplied(size, pixels.as_slice())
}

fn rgba_to_egui_icon(source: &image::RgbaImage) -> Arc<egui::IconData> {
    let square = make_square_app_icon(source, 64);
    Arc::new(egui::IconData {
        rgba: square.into_raw(),
        width: 64,
        height: 64,
    })
}

fn fit_thumbnail(mut img: image::RgbaImage) -> image::RgbaImage {
    strip_chroma_key_if_opaque(&mut img);
    let (w, h) = img.dimensions();
    if w <= MAX_THUMB_DIM && h <= MAX_THUMB_DIM {
        return img;
    }
    let scale = (MAX_THUMB_DIM as f32 / w as f32).min(MAX_THUMB_DIM as f32 / h as f32);
    let nw = ((w as f32 * scale).round() as u32).max(1);
    let nh = ((h as f32 * scale).round() as u32).max(1);
    image::imageops::resize(&img, nw, nh, image::imageops::FilterType::Lanczos3)
}

fn load_square_thumbnail(path: &Path) -> Option<image::RgbaImage> {
    let image = image::open(path).ok()?.to_rgba8();
    let cropped = if image.width() > image.height() {
        let side = image.height();
        image::imageops::crop_imm(&image, 0, 0, side, side).to_image()
    } else {
        image
    };
    Some(fit_thumbnail(cropped))
}

fn extract_xml_tile0_thumbnail(xml_path: &Path) -> Option<image::RgbaImage> {
    let xml = std::fs::read_to_string(xml_path).ok()?;
    let doc = roxmltree::Document::parse(&xml).ok()?;
    let png_node = doc
        .descendants()
        .find(|n| n.tag_name().name().eq_ignore_ascii_case("png"))?;

    let mut b64 = png_node.text()?.trim();
    if let Some(stripped) = b64
        .strip_prefix("<![CDATA[")
        .and_then(|s| s.strip_suffix("]]>"))
    {
        b64 = stripped.trim();
    }

    let compact: String = b64.chars().filter(|c| !c.is_whitespace()).collect();
    let bytes = base64::Engine::decode(&base64::engine::general_purpose::STANDARD, compact).ok()?;
    let sheet = image::load_from_memory(&bytes).ok()?.to_rgba8();

    let tiles_x: u32 = doc
        .descendants()
        .find(|n| n.tag_name().name().eq_ignore_ascii_case("tilesx"))
        .and_then(|n| n.text())
        .and_then(|t| t.trim().parse().ok())
        .unwrap_or(1);
    let tiles_y: u32 = doc
        .descendants()
        .find(|n| n.tag_name().name().eq_ignore_ascii_case("tilesy"))
        .and_then(|n| n.text())
        .and_then(|t| t.trim().parse().ok())
        .unwrap_or(1);

    let tile_w = sheet.width().checked_div(tiles_x)?;
    let tile_h = sheet.height().checked_div(tiles_y)?;
    if tiles_x == 0 || tiles_y == 0 || tile_w == 0 || tile_h == 0 {
        return None;
    }

    let tile = image::imageops::crop_imm(&sheet, 0, 0, tile_w, tile_h).to_image();
    Some(fit_thumbnail(tile))
}

fn cache_is_fresh(cache_path: &Path, source_path: &Path) -> bool {
    let Ok(cache_mod) = std::fs::metadata(cache_path).and_then(|m| m.modified()) else {
        return false;
    };
    let Ok(src_mod) = std::fs::metadata(source_path).and_then(|m| m.modified()) else {
        return false;
    };
    cache_mod >= src_mod
}

fn load_best_thumbnail(
    id: &str,
    pack_dir: Option<&Path>,
    catalog_entry: Option<&CatalogEntry>,
    cache_dir: &Path,
) -> Option<image::RgbaImage> {
    if let Some(dir) = pack_dir {
        let icon_path = dir.join("icon.png");
        if icon_path.is_file() {
            return load_square_thumbnail(&icon_path);
        }
    }

    if let Some(icons_dir) = catalog_icons_dir() {
        let bundled_icon = icons_dir.join(format!("{id}.png"));
        if bundled_icon.is_file()
            && let Some(img) = load_square_thumbnail(&bundled_icon)
        {
            return Some(img);
        }
    }

    let cached_path = cache_dir.join(format!("{id}_v2.png"));
    if let Some(dir) = pack_dir {
        let xml_path = dir.join("deskpet.xml");
        if cached_path.is_file()
            && cache_is_fresh(&cached_path, &xml_path)
            && let Some(img) = load_square_thumbnail(&cached_path)
        {
            return Some(img);
        }
        if let Some(thumb) = extract_xml_tile0_thumbnail(&xml_path)
            .or_else(|| load_square_thumbnail(&dir.join("strip.png")))
        {
            if let Err(e) = std::fs::create_dir_all(cache_dir) {
                diypets::logging::warn(&format!("settings: failed to create cache dir: {e}"));
            }
            if let Err(e) = thumb.save(&cached_path) {
                diypets::logging::warn(&format!(
                    "settings: failed to save cached icon {}: {e}",
                    cached_path.display()
                ));
            }
            return Some(thumb);
        }
    } else if cached_path.is_file()
        && let Some(img) = load_square_thumbnail(&cached_path)
    {
        return Some(img);
    }

    if let Some(entry) = catalog_entry {
        return download_and_cache_icon(entry);
    }
    None
}

fn load_startup_icon(active_pack: &str) -> Option<Arc<egui::IconData>> {
    if let Some(icon_path) = find_pack_icon(active_pack)
        && let Ok(img) = image::open(&icon_path)
    {
        return Some(rgba_to_egui_icon(&img.to_rgba8()));
    }
    default_app_icon(64).map(|sq| {
        Arc::new(egui::IconData {
            rgba: sq.into_raw(),
            width: 64,
            height: 64,
        })
    })
}

struct PetPreview {
    id: String,
    name: String,
    author: String,
    category: PackCategory,
    installed: bool,
    user_downloaded: bool,
    size_kb: Option<u64>,
    catalog_entry: Option<CatalogEntry>,
    rgba_preview: Option<image::RgbaImage>,
    texture: Option<egui::TextureHandle>,
}

impl PetPreview {
    fn window_icon(&self) -> Option<Arc<egui::IconData>> {
        if let Some(img) = &self.rgba_preview {
            return Some(rgba_to_egui_icon(img));
        }
        load_startup_icon(&self.id)
    }
}

enum DownloadOutcome {
    Success { id: String, name: String },
    Failed { id: String, error: String },
}

fn load_gallery(mut add: impl FnMut(PetPreview) + Send) {
    let cache_dir = thumb_cache_dir();
    let catalog = load_embedded_catalog();
    let catalog_map: HashMap<String, CatalogEntry> =
        catalog.iter().cloned().map(|e| (e.id.clone(), e)).collect();

    let mut local_dirs: HashMap<String, PathBuf> = HashMap::new();
    for root in pack_search_dirs() {
        if let Ok(entries) = std::fs::read_dir(root) {
            for entry in entries.flatten() {
                if let Ok(ft) = entry.file_type()
                    && ft.is_dir()
                {
                    let path = entry.path();
                    if path.join("deskpet.xml").is_file() {
                        let id = entry.file_name().to_string_lossy().into_owned();
                        local_dirs.entry(id).or_insert(path);
                    }
                }
            }
        }
    }

    let mut all_ids: Vec<String> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    for entry in &catalog {
        if seen.insert(entry.id.clone()) {
            all_ids.push(entry.id.clone());
        }
    }
    for id in local_dirs.keys() {
        if seen.insert(id.clone()) {
            all_ids.push(id.clone());
        }
    }

    all_ids.sort_by_key(|id| {
        let is_sm = catalog_map
            .get(id)
            .map(|e| e.is_made_by_sm())
            .unwrap_or(true);
        (!is_sm, id.to_lowercase())
    });

    let queue = Mutex::new(all_ids.into_iter());
    let add_cb = Mutex::new(&mut add);
    let worker_count = std::thread::available_parallelism()
        .map(|n| n.get().clamp(2, 8))
        .unwrap_or(4);

    std::thread::scope(|scope| {
        for _ in 0..worker_count {
            let queue = &queue;
            let add_cb = &add_cb;
            let local_dirs = &local_dirs;
            let catalog_map = &catalog_map;
            let cache_dir = &cache_dir;

            scope.spawn(move || {
                loop {
                    let next = {
                        let mut guard = queue.lock().unwrap();
                        guard.next()
                    };
                    let Some(id) = next else { break };

                    let pack_dir = local_dirs.get(&id).map(PathBuf::as_path);
                    let cat_entry = catalog_map.get(&id).cloned();

                    let (name, author, category, size_kb) = if let Some(entry) = &cat_entry {
                        let cat = if entry.is_made_by_sm() {
                            PackCategory::MadeBySM
                        } else {
                            PackCategory::ESheep
                        };
                        (
                            entry.name.clone(),
                            entry.author.clone(),
                            cat,
                            Some(entry.size_kb),
                        )
                    } else if let Some(dir) = pack_dir {
                        let xml_path = dir.join("deskpet.xml");
                        let (n, a) = read_xml_metadata_fast(&xml_path, &id);
                        let cat = classify_local_pack(&id, dir, &a);
                        let kb = std::fs::metadata(&xml_path)
                            .map(|m| m.len().div_ceil(1024))
                            .ok();
                        (n, a, cat, kb)
                    } else {
                        continue;
                    };

                    let installed = pack_dir.is_some() || is_pack_installed(&id);
                    let user_downloaded = is_user_downloaded(&id);
                    let rgba_preview =
                        load_best_thumbnail(&id, pack_dir, cat_entry.as_ref(), cache_dir);

                    let preview = PetPreview {
                        id,
                        name,
                        author,
                        category,
                        installed,
                        user_downloaded,
                        size_kb,
                        catalog_entry: cat_entry,
                        rgba_preview,
                        texture: None,
                    };
                    if let Ok(mut cb) = add_cb.lock() {
                        cb(preview);
                    }
                }
            });
        }
    });
}

fn card_frame(selected: bool) -> egui::Frame {
    let (fill, stroke_col) = if selected {
        (
            egui::Color32::from_rgb(35, 58, 96),
            egui::Color32::from_rgb(96, 165, 250),
        )
    } else {
        (
            egui::Color32::from_rgb(28, 33, 44),
            egui::Color32::from_rgb(46, 54, 70),
        )
    };
    egui::Frame::default()
        .fill(fill)
        .stroke(egui::Stroke::new(
            if selected { 1.5 } else { 1.0 },
            stroke_col,
        ))
        .inner_margin(10.0)
        .corner_radius(egui::CornerRadius::same(10))
}

fn panel_card() -> egui::Frame {
    egui::Frame::default()
        .fill(egui::Color32::from_rgb(26, 30, 40))
        .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(44, 52, 68)))
        .inner_margin(14.0)
        .corner_radius(egui::CornerRadius::same(10))
}

fn apply_settings_theme(ctx: &egui::Context) {
    let mut style = (*ctx.style_of(egui::Theme::Dark)).clone();
    style.spacing.item_spacing = egui::vec2(8.0, 6.0);
    style.spacing.button_padding = egui::vec2(10.0, 6.0);
    let mut visuals = egui::Visuals::dark();
    visuals.window_fill = egui::Color32::from_rgb(18, 21, 28);
    visuals.panel_fill = egui::Color32::from_rgb(21, 25, 34);
    visuals.faint_bg_color = egui::Color32::from_rgb(28, 33, 44);
    visuals.selection.bg_fill = egui::Color32::from_rgb(56, 120, 224);
    visuals.selection.stroke = egui::Stroke::new(1.0, egui::Color32::WHITE);
    visuals.widgets.inactive.bg_fill = egui::Color32::from_rgb(36, 42, 56);
    visuals.widgets.inactive.corner_radius = egui::CornerRadius::same(7);
    visuals.widgets.hovered.bg_fill = egui::Color32::from_rgb(48, 56, 74);
    visuals.widgets.hovered.corner_radius = egui::CornerRadius::same(7);
    visuals.widgets.active.bg_fill = egui::Color32::from_rgb(56, 120, 224);
    visuals.widgets.active.corner_radius = egui::CornerRadius::same(7);
    style.visuals = visuals;
    ctx.set_style_of(egui::Theme::Dark, style.clone());
    ctx.set_style_of(egui::Theme::Light, style);
}

struct MyApp {
    toasts: ToastQueue,
    tab: SettingsTab,
    settings: Settings,
    path: PathBuf,
    pets: Vec<PetPreview>,
    gallery: std::sync::mpsc::Receiver<PetPreview>,
    dl_tx: std::sync::mpsc::Sender<DownloadOutcome>,
    dl_rx: std::sync::mpsc::Receiver<DownloadOutcome>,
    downloading: HashSet<String>,
    status_banner: Option<(String, bool)>,
    icon_pack: String,
    loading: bool,
    search: String,
    last_scan: Instant,
    creator_process: Option<std::process::Child>,
    runner_process: Option<std::process::Child>,
    update_rx: std::sync::mpsc::Receiver<String>,
    update_available: Option<String>,
    test_bubble_text: String,
    mcp_snippet: String,
}

impl MyApp {
    fn new(ctx: egui::Context) -> Self {
        let path = Settings::default_path();
        let mut settings = Settings::load_sanitized(&path);
        settings.start_on_login = autostart::is_enabled();
        let (sender, gallery) = std::sync::mpsc::channel();
        let ctx_clone = ctx.clone();
        std::thread::spawn(move || {
            load_gallery(|pet| {
                if sender.send(pet).is_ok() {
                    ctx_clone.request_repaint();
                }
            });
            drop(sender);
            ctx_clone.request_repaint();
        });
        let (dl_tx, dl_rx) = std::sync::mpsc::channel();
        let (upd_tx, update_rx) = std::sync::mpsc::channel();
        let ctx_upd = ctx.clone();
        std::thread::spawn(move || {
            let current = env!("CARGO_PKG_VERSION");
            if let Ok(resp) =
                ureq::get("https://api.github.com/repos/smresponsibilities/diypets/releases/latest")
                    .set("User-Agent", "diypets-updater")
                    .timeout(Duration::from_secs(4))
                    .call()
                && let Ok(text) = resp.into_string()
                && let Ok(v) = serde_json::from_str::<serde_json::Value>(&text)
                && let Some(tag) = v.get("tag_name").and_then(|t| t.as_str())
            {
                let latest = tag.trim_start_matches('v');
                if !latest.is_empty() && latest != current {
                    if let Err(e) = upd_tx.send(tag.to_string()) {
                        diypets::logging::warn(&format!("settings: update check send failed: {e}"));
                    }
                    ctx_upd.request_repaint();
                }
            }
        });

        let mcp_snippet = r#"{
  "mcpServers": {
    "diypets": {
      "command": "diypets",
      "args": ["mcp"]
    }
  }
}"#
        .to_string();

        Self {
            toasts: ToastQueue::new(),
            tab: SettingsTab::Gallery,
            icon_pack: settings.pet_pack.clone(),
            settings,
            path,
            pets: Vec::new(),
            gallery,
            dl_tx,
            dl_rx,
            downloading: HashSet::new(),
            status_banner: None,
            loading: true,
            search: String::new(),
            last_scan: Instant::now(),
            creator_process: None,
            runner_process: None,
            update_rx,
            update_available: None,
            test_bubble_text: "Build passed!".to_string(),
            mcp_snippet,
        }
    }

    fn import_xml_path(&mut self, src_path: &Path) -> bool {
        let Ok(xml_text) = std::fs::read_to_string(src_path) else {
            self.status_banner = Some(("Could not read selected XML file.".to_string(), true));
            return false;
        };
        let pack = match DeskpetPack::parse(&xml_text) {
            Ok(p) => p,
            Err(e) => {
                self.status_banner = Some((format!("Invalid pet XML: {e}"), true));
                return false;
            }
        };

        let stem = src_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("imported_pet");
        let id: String = stem
            .to_lowercase()
            .chars()
            .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
            .collect::<String>()
            .trim_matches('_')
            .to_string();
        let id = if id.is_empty() || id == "deskpet" {
            format!("imported_{}", self.pets.len() + 1)
        } else {
            id
        };

        let dest_dir = user_packs_dir().join(&id);
        if let Err(e) = std::fs::create_dir_all(&dest_dir) {
            self.status_banner = Some((format!("Cannot create pack folder: {e}"), true));
            return false;
        }
        if let Err(e) = std::fs::write(dest_dir.join("deskpet.xml"), &xml_text) {
            self.status_banner = Some((format!("Cannot save XML: {e}"), true));
            return false;
        }
        if let Some(first) = pack.frames.first() {
            let rgba: Vec<u8> = first
                .iter()
                .flat_map(|pixel| {
                    let [a, r, g, b] = pixel.to_be_bytes();
                    let s = |ch: u8| (u16::from(ch) * 255 / u16::from(a.max(1))).min(255) as u8;
                    [s(r), s(g), s(b), a]
                })
                .collect();
            if let Some(img) = image::RgbaImage::from_raw(pack.tile_w, pack.tile_h, rgba)
                && let Err(e) = img.save(dest_dir.join("icon.png"))
            {
                diypets::logging::warn(&format!("settings: failed to save imported icon.png: {e}"));
            }
        }
        self.settings.pet_pack = id.clone();
        self.last_scan = Instant::now() - REFRESH_INTERVAL;
        self.scan_for_new_local_pets();
        self.status_banner = Some((format!("Imported & activated '{id}'!"), false));
        true
    }

    fn import_svg_path(&mut self, src_path: &Path) -> bool {
        let Ok(svg_text) = std::fs::read_to_string(src_path) else {
            self.status_banner = Some(("Could not read selected SVG file.".to_string(), true));
            return false;
        };
        let stem = src_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("svg_pet");
        let proj = match import_svg_to_project(&svg_text, stem, Archetype::Quadruped) {
            Ok(p) => p,
            Err(e) => {
                self.status_banner = Some((format!("SVG import error: {e}"), true));
                return false;
            }
        };
        let dest_dir = user_packs_dir().join(&proj.pack_id);
        if let Err(e) = proj.export_to_dir(&dest_dir) {
            self.status_banner = Some((format!("SVG export error: {e}"), true));
            return false;
        }
        self.settings.pet_pack = proj.pack_id.clone();
        self.last_scan = Instant::now() - REFRESH_INTERVAL;
        self.scan_for_new_local_pets();
        self.status_banner = Some((
            format!(
                "Auto-rigged SVG '{}' & activated on desktop!",
                proj.display_name
            ),
            false,
        ));
        true
    }

    fn import_xml_dialog(&mut self) -> bool {
        let Some(src_path) = rfd::FileDialog::new()
            .add_filter("DIYPets / eSheep XML or 2D SVG", &["xml", "svg"])
            .pick_file()
        else {
            return false;
        };
        match src_path
            .extension()
            .and_then(|e| e.to_str())
            .map(|s| s.to_ascii_lowercase())
            .as_deref()
        {
            Some("svg") => self.import_svg_path(&src_path),
            _ => self.import_xml_path(&src_path),
        }
    }

    fn launch_sibling(slot: &mut Option<std::process::Child>, stem: &str) {
        if let Some(child) = slot.as_mut()
            && matches!(child.try_wait(), Ok(None))
        {
            return;
        }
        if let Some(exe) = sibling_binary(stem) {
            let mut cmd = std::process::Command::new(exe);
            cmd.current_dir(resolve_project_root());
            if let Ok(child) = cmd.spawn() {
                *slot = Some(child);
            }
        }
    }

    fn start_download(&mut self, entry: CatalogEntry, ctx: egui::Context) {
        if !self.downloading.insert(entry.id.clone()) {
            return;
        }
        self.status_banner = Some((format!("Downloading {}...", entry.name), false));
        let tx = self.dl_tx.clone();
        std::thread::spawn(move || {
            let outcome = match download_pack(&entry) {
                Ok(_) => DownloadOutcome::Success {
                    id: entry.id.clone(),
                    name: entry.name.clone(),
                },
                Err(error) => DownloadOutcome::Failed {
                    id: entry.id.clone(),
                    error,
                },
            };
            if let Err(e) = tx.send(outcome) {
                diypets::logging::warn(&format!("settings: download result send failed: {e}"));
            }
            ctx.request_repaint();
        });
    }

    fn scan_for_new_local_pets(&mut self) {
        if self.loading || self.last_scan.elapsed() < REFRESH_INTERVAL {
            return;
        }
        self.last_scan = Instant::now();

        let existing: HashSet<String> = self.pets.iter().map(|p| p.id.clone()).collect();
        let cache_dir = thumb_cache_dir();
        let mut added = false;

        for root in pack_search_dirs() {
            if let Ok(entries) = std::fs::read_dir(root) {
                for entry in entries.flatten() {
                    if let Ok(ft) = entry.file_type()
                        && ft.is_dir()
                    {
                        let path = entry.path();
                        let xml_path = path.join("deskpet.xml");
                        if !xml_path.is_file() {
                            continue;
                        }
                        let id = entry.file_name().to_string_lossy().into_owned();
                        if existing.contains(&id) {
                            continue;
                        }
                        let (name, author) = read_xml_metadata_fast(&xml_path, &id);
                        let category = classify_local_pack(&id, &path, &author);
                        let size_kb = std::fs::metadata(&xml_path)
                            .map(|m| m.len().div_ceil(1024))
                            .ok();
                        let rgba_preview = load_best_thumbnail(&id, Some(&path), None, &cache_dir);
                        self.pets.push(PetPreview {
                            id: id.clone(),
                            name,
                            author,
                            category,
                            installed: true,
                            user_downloaded: is_user_downloaded(&id),
                            size_kb,
                            catalog_entry: None,
                            rgba_preview,
                            texture: None,
                        });
                        added = true;
                    }
                }
            }
        }

        if added {
            self.pets.sort_by_cached_key(|pet| pet.name.to_lowercase());
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn render_pet_row(
        ui: &mut egui::Ui,
        row_id: &str,
        indices: &[usize],
        pets: &[PetPreview],
        selected_pack: &mut String,
        can_add_more: bool,
        downloading: &HashSet<String>,
        changed: &mut bool,
        to_add_extra: &mut Option<String>,
        to_download: &mut Option<CatalogEntry>,
        to_delete: &mut Option<String>,
    ) {
        if indices.is_empty() {
            ui.label(
                egui::RichText::new("No matching pets in this row.")
                    .italics()
                    .color(egui::Color32::GRAY),
            );
            return;
        }

        egui::ScrollArea::horizontal()
            .id_salt(row_id)
            .auto_shrink([false, true])
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 12.0;
                    for &idx in indices {
                        let pet = &pets[idx];
                        let is_selected = *selected_pack == pet.id;
                        let is_downloading = downloading.contains(&pet.id);
                        let mut add_clicked = false;

                        let response = card_frame(is_selected)
                            .show(ui, |ui| {
                                ui.set_width(134.0);
                                ui.set_min_height(158.0);
                                ui.vertical_centered(|ui| {
                                    if let Some(texture) = &pet.texture {
                                        let mut size = texture.size_vec2();
                                        let max_box = 78.0;
                                        let scale = (max_box / size.x).min(max_box / size.y);
                                        if size.x > max_box || size.y > max_box {
                                            size *= scale;
                                        } else if size.x < 48.0 && size.y < 48.0 {
                                            size *= scale.floor().max(1.0);
                                        }
                                        ui.add_sized(
                                            [max_box, max_box],
                                            egui::Image::new((texture.id(), size)),
                                        );
                                    } else {
                                        ui.add_space(78.0);
                                    }

                                    ui.add_space(5.0);
                                    ui.label(
                                        egui::RichText::new(&pet.name).strong().size(13.0).color(
                                            if is_selected {
                                                egui::Color32::WHITE
                                            } else {
                                                egui::Color32::from_rgb(232, 236, 244)
                                            },
                                        ),
                                    );
                                    if !pet.author.is_empty() {
                                        ui.label(
                                            egui::RichText::new(format!("By {}", pet.author))
                                                .size(10.5)
                                                .color(egui::Color32::from_rgb(142, 152, 172)),
                                        );
                                    }

                                    ui.add_space(4.0);
                                    if is_downloading {
                                        ui.horizontal(|ui| {
                                            ui.spinner();
                                            ui.label(
                                                egui::RichText::new("Getting...")
                                                    .size(10.5)
                                                    .color(egui::Color32::from_rgb(120, 185, 255)),
                                            );
                                        });
                                    } else if !pet.installed {
                                        let kb_label = pet
                                            .size_kb
                                            .map(|kb| format!("Get ({kb} KB)"))
                                            .unwrap_or_else(|| "Get pack".to_string());
                                        ui.label(
                                            egui::RichText::new(kb_label)
                                                .size(11.0)
                                                .strong()
                                                .color(egui::Color32::from_rgb(112, 182, 255)),
                                        );
                                    } else {
                                        ui.horizontal(|ui| {
                                            if is_selected {
                                                ui.label(
                                                    egui::RichText::new("Active")
                                                        .size(10.5)
                                                        .strong()
                                                        .color(egui::Color32::from_rgb(
                                                            134, 239, 172,
                                                        )),
                                                );
                                            }
                                            if can_add_more
                                                && ui
                                                    .small_button("+ Herd")
                                                    .on_hover_text(
                                                        "Spawn alongside your current pets",
                                                    )
                                                    .clicked()
                                            {
                                                add_clicked = true;
                                            }
                                        });
                                    }
                                });
                            })
                            .response;

                        if add_clicked {
                            *to_add_extra = Some(pet.id.clone());
                        } else {
                            let response = response.interact(egui::Sense::click());
                            if response.clicked() && !is_downloading {
                                if pet.installed {
                                    if *selected_pack != pet.id {
                                        *selected_pack = pet.id.clone();
                                        *changed = true;
                                    }
                                } else if let Some(entry) = &pet.catalog_entry {
                                    *to_download = Some(entry.clone());
                                }
                            }
                            if pet.user_downloaded {
                                response.context_menu(|ui| {
                                    if ui.button("Remove downloaded pack").clicked() {
                                        *to_delete = Some(pet.id.clone());
                                        ui.close();
                                    }
                                });
                            }
                            if response.hovered() {
                                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                            }
                        }
                    }
                });
            });
    }
}

impl eframe::App for MyApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let mut changed = false;

        // Drag-and-drop .xml or .svg files anywhere on Settings!
        let dropped: Vec<PathBuf> = ui.ctx().input(|i| {
            i.raw
                .dropped_files
                .iter()
                .map(|f| f.path().to_path_buf())
                .collect()
        });
        for path in dropped {
            match path
                .extension()
                .and_then(|e| e.to_str())
                .map(|s| s.to_ascii_lowercase())
                .as_deref()
            {
                Some("xml") => {
                    if self.import_xml_path(&path) {
                        changed = true;
                    }
                }
                Some("svg") if self.import_svg_path(&path) => {
                    changed = true;
                }
                _ => {}
            }
        }

        let previous_count = self.pets.len();
        while self.loading {
            match self.gallery.try_recv() {
                Ok(pet) => self.pets.push(pet),
                Err(std::sync::mpsc::TryRecvError::Empty) => break,
                Err(std::sync::mpsc::TryRecvError::Disconnected) => self.loading = false,
            }
        }
        if self.pets.len() != previous_count {
            self.pets.sort_by_cached_key(|pet| pet.name.to_lowercase());
        }

        while let Ok(outcome) = self.dl_rx.try_recv() {
            match outcome {
                DownloadOutcome::Success { id, name } => {
                    self.downloading.remove(&id);
                    if let Some(pet) = self.pets.iter_mut().find(|p| p.id == id) {
                        pet.installed = true;
                        pet.user_downloaded = true;
                    }
                    self.settings.pet_pack = id;
                    changed = true;
                    self.status_banner = Some((format!("Downloaded and activated {name}!"), false));
                }
                DownloadOutcome::Failed { id, error } => {
                    self.downloading.remove(&id);
                    self.status_banner = Some((error, true));
                }
            }
        }

        if let Ok(tag) = self.update_rx.try_recv() {
            self.update_available = Some(tag);
        }
        self.scan_for_new_local_pets();

        for pet in &mut self.pets {
            if pet.texture.is_none()
                && let Some(rgba) = &pet.rgba_preview
            {
                let img = rgba_to_color_image(rgba);
                let tex_opts = if img.size[0].max(img.size[1]) >= 80 {
                    egui::TextureOptions::LINEAR
                } else {
                    egui::TextureOptions::NEAREST
                };
                pet.texture = Some(ui.ctx().load_texture(&pet.id, img, tex_opts));
            }
        }

        let mut open_creator = false;
        let mut launch_runner = false;
        let mut to_add_extra: Option<String> = None;
        let mut to_download: Option<CatalogEntry> = None;
        let mut to_delete: Option<String> = None;

        // TOP NAVIGATION HEADER
        egui::Panel::top("settings_topbar").show(ui, |ui| {
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new("DIYPets")
                        .strong()
                        .size(18.0)
                        .color(egui::Color32::from_rgb(245, 166, 92)),
                );
                ui.label(
                    egui::RichText::new(format!("v{}", env!("CARGO_PKG_VERSION")))
                        .size(11.0)
                        .color(egui::Color32::GRAY),
                );
                ui.separator();

                let herd_len = 1 + self.settings.extra_pets.len();
                ui.selectable_value(&mut self.tab, SettingsTab::Gallery, "Pet Gallery");
                ui.selectable_value(
                    &mut self.tab,
                    SettingsTab::Herd,
                    format!("Active Herd ({herd_len}/{MAX_ACTIVE_PETS})"),
                );
                ui.selectable_value(&mut self.tab, SettingsTab::Controls, "Behavior & AI Agent");

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui
                        .add(
                            egui::Button::new(
                                egui::RichText::new("+ Create Pet")
                                    .strong()
                                    .color(egui::Color32::WHITE),
                            )
                            .fill(egui::Color32::from_rgb(56, 120, 224)),
                        )
                        .clicked()
                    {
                        open_creator = true;
                    }
                    if ui.button("Import .xml / .svg").clicked() {
                        self.toasts
                            .info("Select a .xml pack or .svg file to import");
                        if self.import_xml_dialog() {
                            changed = true;
                        }
                    }
                    if ui
                        .button("Launch Pet")
                        .on_hover_text("Start the desktop pet runner")
                        .clicked()
                    {
                        self.toasts.success("Launching DIYPets on desktop");
                        launch_runner = true;
                    }
                });
            });

            if let Some(tag) = &self.update_available {
                ui.add_space(4.0);
                egui::Frame::default()
                    .fill(egui::Color32::from_rgb(28, 56, 40))
                    .inner_margin(6.0)
                    .corner_radius(egui::CornerRadius::same(6))
                    .show(ui, |ui| {
                        ui.horizontal_wrapped(|ui| {
                            ui.colored_label(
                                egui::Color32::from_rgb(134, 239, 172),
                                format!(
                                    "Update available: {tag} (current v{})",
                                    env!("CARGO_PKG_VERSION")
                                ),
                            );
                            ui.hyperlink_to(
                                "Download Latest",
                                "https://github.com/smresponsibilities/diypets/releases/latest",
                            );
                            ui.label(
                                egui::RichText::new("or run: cargo install diypets --force")
                                    .size(11.0)
                                    .color(egui::Color32::LIGHT_GRAY),
                            );
                        });
                    });
            }

            if let Some((msg, is_err)) = &self.status_banner {
                ui.add_space(2.0);
                let color = if *is_err {
                    egui::Color32::from_rgb(248, 113, 113)
                } else {
                    egui::Color32::from_rgb(134, 239, 172)
                };
                ui.label(egui::RichText::new(msg).size(12.0).color(color));
            }
            ui.add_space(4.0);
        });

        // MAIN TAB CONTENT
        egui::CentralPanel::default().show(ui, |ui| match self.tab {
            SettingsTab::Gallery => {
                ui.horizontal(|ui| {
                    let installed_count = self.pets.iter().filter(|p| p.installed).count();
                    let remote_count = self.pets.len().saturating_sub(installed_count);
                    ui.label(
                        egui::RichText::new(format!(
                            "{installed_count} installed  |  {remote_count} on-demand  |  Tip: drop any .xml or .svg file here!"
                        ))
                        .size(11.5)
                        .color(egui::Color32::from_rgb(150, 160, 180)),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.add(
                            egui::TextEdit::singleline(&mut self.search)
                                .hint_text("Search pets or authors...")
                                .desired_width(210.0),
                        );
                    });
                });
                ui.add_space(8.0);

                if self.loading && self.pets.is_empty() {
                    ui.label("Loading pets...");
                    return;
                }

                let query = self.search.trim().to_lowercase();
                let mut made_by_sm = Vec::new();
                let mut esheep = Vec::new();
                for (idx, pet) in self.pets.iter().enumerate() {
                    if !query.is_empty()
                        && !pet.name.to_lowercase().contains(&query)
                        && !pet.id.to_lowercase().contains(&query)
                        && !pet.author.to_lowercase().contains(&query)
                    {
                        continue;
                    }
                    match pet.category {
                        PackCategory::MadeBySM => made_by_sm.push(idx),
                        PackCategory::ESheep => esheep.push(idx),
                    }
                }
                let can_add_more = 1 + self.settings.extra_pets.len() < MAX_ACTIVE_PETS;

                egui::ScrollArea::vertical().show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new(format!("Made by SM ({})", made_by_sm.len()))
                                .strong()
                                .size(15.0),
                        );
                        ui.label(
                            egui::RichText::new("Original roster & your Studio creations")
                                .size(11.5)
                                .color(egui::Color32::GRAY),
                        );
                    });
                    ui.add_space(6.0);
                    Self::render_pet_row(
                        ui,
                        "row_made_by_sm",
                        &made_by_sm,
                        &self.pets,
                        &mut self.settings.pet_pack,
                        can_add_more,
                        &self.downloading,
                        &mut changed,
                        &mut to_add_extra,
                        &mut to_download,
                        &mut to_delete,
                    );

                    ui.add_space(18.0);
                    ui.separator();
                    ui.add_space(10.0);

                    ui.horizontal_wrapped(|ui| {
                        ui.label(
                            egui::RichText::new(format!("eSheep Community ({})", esheep.len()))
                                .strong()
                                .size(15.0),
                        );
                        ui.label(
                            egui::RichText::new("- Credit: Adriano Petrucci (")
                                .size(11.5)
                                .color(egui::Color32::GRAY),
                        );
                        ui.hyperlink_to(
                            egui::RichText::new("Adrianotiger/desktopPet").size(11.5),
                            "https://github.com/Adrianotiger/desktopPet",
                        );
                        ui.label(
                            egui::RichText::new(") & original pack authors")
                                .size(11.5)
                                .color(egui::Color32::GRAY),
                        );
                    });
                    ui.add_space(6.0);
                    Self::render_pet_row(
                        ui,
                        "row_esheep",
                        &esheep,
                        &self.pets,
                        &mut self.settings.pet_pack,
                        can_add_more,
                        &self.downloading,
                        &mut changed,
                        &mut to_add_extra,
                        &mut to_download,
                        &mut to_delete,
                    );
                });
            }

            SettingsTab::Herd => {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    panel_card().show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.label(
                                egui::RichText::new(format!(
                                    "Active Desktop Herd ({}/{MAX_ACTIVE_PETS})",
                                    1 + self.settings.extra_pets.len()
                                ))
                                .strong()
                                .size(16.0),
                            );
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                if !self.settings.extra_pets.is_empty()
                                    && ui.button("Clear All Extras").clicked()
                                {
                            self.toasts.info("Cleared extra herd pets");
                                    self.settings.extra_pets.clear();
                                    changed = true;
                                }
                            });
                        });
                        ui.label(
                            egui::RichText::new("Mix any installed pets together on your taskbar. Right-click any pet on desktop to headpat, sleep, clone, or dismiss it.")
                                .size(12.0)
                                .color(egui::Color32::GRAY),
                        );
                        ui.add_space(10.0);

                        let herd = self.settings.active_herd();
                        let mut remove_extra_idx: Option<usize> = None;
                        ui.horizontal_wrapped(|ui| {
                            for (slot, pack_id) in herd.iter().enumerate() {
                                card_frame(slot == 0).show(ui, |ui| {
                                    ui.set_width(136.0);
                                    ui.vertical_centered(|ui| {
                                        if let Some(pet) = self.pets.iter().find(|p| &p.id == pack_id)
                                            && let Some(tex) = &pet.texture
                                        {
                                            let mut s = tex.size_vec2();
                                            let sc = (68.0 / s.x).min(68.0 / s.y).min(1.5);
                                            s *= sc;
                                            ui.add_sized([68.0, 68.0], egui::Image::new((tex.id(), s)));
                                        }
                                        ui.label(egui::RichText::new(pack_id).strong());
                                        if slot == 0 {
                                            ui.colored_label(
                                                egui::Color32::from_rgb(134, 239, 172),
                                                "Main Pet",
                                            );
                                        } else if ui.small_button("Dismiss").clicked() {
                                self.toasts.info("Dismissed pet from herd");
                                            remove_extra_idx = Some(slot - 1);
                                        }
                                    });
                                });
                            }
                        });
                        if let Some(ri) = remove_extra_idx {
                            self.settings.remove_extra_pet(ri);
                            changed = true;
                        }
                    });

                    ui.add_space(12.0);
                    panel_card().show(ui, |ui| {
                        ui.label(egui::RichText::new("Quick Add Installed Pet").strong().size(14.0));
                        ui.add_space(6.0);
                        let can_add = 1 + self.settings.extra_pets.len() < MAX_ACTIVE_PETS;
                        ui.horizontal_wrapped(|ui| {
                            for pet in self.pets.iter().filter(|p| p.installed) {
                                if ui
                                    .add_enabled(can_add, egui::Button::new(format!("+ {}", pet.name)))
                                    .clicked()
                                {
                                    to_add_extra = Some(pet.id.clone());
                                }
                            }
                        });
                    });
                });
            }

            SettingsTab::Controls => {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    ui.columns(2, |cols| {
                        // Left card: Desktop Physics & OS Integration
                        panel_card().show(&mut cols[0], |ui| {
                            ui.label(egui::RichText::new("Desktop Physics & System").strong().size(15.0));
                            ui.add_space(8.0);
                            changed |= ui
                                .add(
                                    egui::Slider::new(&mut self.settings.pet_size, PET_SIZE_MIN..=PET_SIZE_MAX)
                                        .text("Pet Size (px)"),
                                )
                                .changed();
                            changed |= ui
                                .add(
                                    egui::Slider::new(
                                        &mut self.settings.walk_speed,
                                        WALK_SPEED_MIN..=WALK_SPEED_MAX,
                                    )
                                    .text("Walk Speed"),
                                )
                                .changed();
                            ui.add_space(8.0);
                            changed |= ui
                                .checkbox(&mut self.settings.always_on_top, "Always on Top")
                                .changed();
                            changed |= ui
                                .checkbox(&mut self.settings.climb_windows, "Climb & Ride Open Windows")
                                .changed();
                            changed |= ui
                                .checkbox(&mut self.settings.click_through, "Click Through Transparent Corners")
                                .changed();
                            changed |= ui
                                .checkbox(&mut self.settings.typing_reflex, "Typing Cadence Reflex (Bongo Bounce)")
                                .changed();
                            changed |= ui
                                .checkbox(&mut self.settings.cpu_sprint, "CPU Build Sprint (RunCat Speed)")
                                .changed();
                            if ui
                                .checkbox(&mut self.settings.start_on_login, "Start on Login (Autostart)")
                                .changed()
                            {
                                match autostart::set_enabled(self.settings.start_on_login) {
                                    Ok(()) => {
                                        self.status_banner = Some((
                                            if self.settings.start_on_login {
                                                "Enabled Start on Login.".to_string()
                                            } else {
                                                "Disabled Start on Login.".to_string()
                                            },
                                            false,
                                        ));
                                        changed = true;
                                    }
                                    Err(e) => {
                                        self.settings.start_on_login = !self.settings.start_on_login;
                                        self.status_banner = Some((format!("Autostart error: {e}"), true));
                                    }
                                }
                            }
                            ui.add_space(10.0);
                            ui.separator();
                            ui.add_space(6.0);
                            ui.horizontal(|ui| {
                                if ui.button("Open Packs Folder").clicked() {
                    self.toasts.info("Opened local packs folder");
                                    open_folder_in_os(&user_packs_dir());
                                }
                                if ui.button("Open Project GitHub").clicked() {
                    self.toasts.info("Opening GitHub repository");
                                    ui.ctx().open_url(egui::OpenUrl::new_tab(
                                        "https://github.com/smresponsibilities/diypets",
                                    ));
                                }
                            });
                        });

                        // Right card: AI Agent / MCP Live Control & Speech Bubble Tester
                        panel_card().show(&mut cols[1], |ui| {
                            ui.label(egui::RichText::new("AI Agent & MCP Speech Bubbles").strong().size(15.0));
                            ui.label(
                                egui::RichText::new("Local endpoint active while DIYPets runs: http://127.0.0.1:3721")
                                    .size(11.5)
                                    .color(egui::Color32::from_rgb(134, 239, 172)),
                            );
                            ui.add_space(8.0);
                            ui.label("Test Speech Bubble Message:");
                            ui.text_edit_singleline(&mut self.test_bubble_text);
                            ui.add_space(4.0);
                            ui.horizontal_wrapped(|ui| {
                                for (mood, label) in [
                                    ("happy", "Happy + Hearts"),
                                    ("hop", "Hop"),
                                    ("sleep", "Sleep"),
                                    ("walk", "Walk"),
                                ] {
                                    if ui.button(label).clicked() {
                                        let txt = self.test_bubble_text.trim();
                                        let opt = if txt.is_empty() { None } else { Some(txt) };
                                        match agent_api::send_cli_react(mood, opt) {
                                            Ok(_) => {
                                                self.status_banner = Some((
                                                    format!("Sent '{mood}' reaction to desktop pet!"),
                                                    false,
                                                ));
                                            }
                                            Err(e) => {
                                                self.status_banner = Some((e, true));
                                            }
                                        }
                                    }
                                }
                            });

                            ui.add_space(10.0);
                            ui.separator();
                            ui.add_space(6.0);
                            ui.label(egui::RichText::new("Claude Code / Cursor / Codex MCP Config:").strong().size(12.0));
                            ui.add(
                                egui::TextEdit::multiline(&mut self.mcp_snippet)
                                    .font(egui::TextStyle::Monospace)
                                    .desired_rows(6)
                                    .desired_width(f32::INFINITY),
                            );
                            ui.label(
                                egui::RichText::new("CLI: diypets react --mood happy --text \"Tests passed!\"")
                                    .size(11.0)
                                    .color(egui::Color32::GRAY),
                            );
                        });
                    });
                });
            }
        });

        if open_creator {
            Self::launch_sibling(&mut self.creator_process, "diypets-creator");
        }
        if launch_runner {
            Self::launch_sibling(&mut self.runner_process, "diypets");
        }
        if let Some(add_id) = to_add_extra
            && self.settings.add_extra_pet(&add_id)
        {
            changed = true;
            self.status_banner = Some((
                format!(
                    "Added '{add_id}' to your desktop herd ({}/{MAX_ACTIVE_PETS})!",
                    1 + self.settings.extra_pets.len()
                ),
                false,
            ));
        }
        if let Some(entry) = to_download {
            self.start_download(entry, ui.ctx().clone());
        }
        if let Some(del_id) = to_delete {
            match delete_downloaded_pack(&del_id) {
                Ok(()) => {
                    if let Some(pet) = self.pets.iter_mut().find(|p| p.id == del_id) {
                        pet.user_downloaded = false;
                        pet.installed = find_pack_xml(&del_id).is_some();
                    }
                    self.settings.extra_pets.retain(|p| p != &del_id);
                    if self.settings.pet_pack == del_id && find_pack_xml(&del_id).is_none() {
                        self.settings.pet_pack = "highland_cow".to_string();
                    }
                    changed = true;
                    self.status_banner =
                        Some((format!("Removed downloaded pack {del_id}."), false));
                }
                Err(e) => {
                    self.status_banner = Some((e, true));
                }
            }
        }

        if self.icon_pack != self.settings.pet_pack
            && let Some(pet) = self.pets.iter().find(|p| p.id == self.settings.pet_pack)
            && let Some(icon) = pet.window_icon()
        {
            ui.ctx()
                .send_viewport_cmd(egui::ViewportCommand::Icon(Some(icon)));
            self.icon_pack = pet.id.clone();
        }
        if changed && let Err(e) = self.settings.save(&self.path) {
            eprintln!("Failed to save settings: {}", e);
        }

        ui.ctx().request_repaint_after(REFRESH_INTERVAL);
    }
}

fn main() -> Result<(), eframe::Error> {
    let settings = Settings::load_sanitized(&Settings::default_path());
    let mut viewport = egui::ViewportBuilder::default()
        .with_inner_size([960.0, 660.0])
        .with_min_inner_size([640.0, 480.0])
        .with_title("DIYPets Settings");

    if let Some(icon) = load_startup_icon(&settings.pet_pack) {
        viewport = viewport.with_icon(icon);
    }

    let options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };
    eframe::run_native(
        "DIYPets Settings",
        options,
        Box::new(|cc| {
            apply_settings_theme(&cc.egui_ctx);
            Ok(Box::new(MyApp::new(cc.egui_ctx.clone())))
        }),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gallery_has_icons_for_every_bundled_pet() {
        let mut pets = Vec::new();
        load_gallery(|pet| pets.push(pet));
        assert!(pets.iter().any(|pet| pet.id == "highland_cow"));
        for pet in pets {
            let icon = pet.window_icon().expect(&pet.id);
            assert_eq!((icon.width, icon.height), (64, 64));
            assert!(
                icon.rgba.as_chunks::<4>().0.iter().any(|p| p[3] != 0),
                "{}",
                pet.id
            );
        }
    }

    #[test]
    fn classifies_originals_and_esheep_into_two_rows() {
        let mut pets = Vec::new();
        load_gallery(|pet| pets.push(pet));
        let cow = pets
            .iter()
            .find(|p| p.id == "highland_cow")
            .expect("highland_cow");
        assert_eq!(cow.category, PackCategory::MadeBySM);
        if let Some(sheep) = pets.iter().find(|p| p.id == "esheep64") {
            assert_eq!(sheep.category, PackCategory::ESheep);
        }
    }
}
