#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use diypets::catalog::default_app_icon;
use diypets::settings::{Settings, local_project_root, user_packs_dir};
use diypets::studio_model::{
    Archetype, BOARD_SIZE, CustomAnimation, CustomKeyframe, FaceSlot, PartRole,
    PreviewClip, RgbaPixel, RigPart, StudioProject, paint_ellipse, paint_outline,
    rgba_image_fit_to_board, sanitize_pack_id,
};
use diypets::svg_import::import_svg_to_project;
use eframe::egui;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

const MAX_UNDO: usize = 35;

const BUNDLED_PROTECTED: &[&str] = &[
    "highland_cow",
    "dog",
    "polar_bear",
    "goat",
    "stegosaurus",
    "esheep64",
];

const PALETTE: &[(u8, u8, u8)] = &[
    (28, 28, 34),
    (255, 255, 255),
    (242, 224, 200),
    (214, 158, 92),
    (160, 96, 52),
    (96, 62, 44),
    (176, 184, 196),
    (108, 118, 134),
    (240, 118, 134),
    (222, 68, 74),
    (245, 146, 64),
    (246, 206, 86),
    (132, 202, 98),
    (66, 156, 112),
    (98, 196, 212),
    (84, 144, 226),
    (74, 88, 184),
    (152, 108, 214),
    (216, 118, 196),
    (255, 196, 206),
];

#[derive(Clone, Copy, PartialEq, Eq)]
enum StudioTab {
    Draw,
    Rig,
    Animate,
    Export,
    ImportSheet,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum EditLayer {
    Body,
    Part(usize),
    Face(FaceSlot),
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tool {
    Brush,
    Eraser,
    Fill,
    Shade,
    Recolor,
    Eyedropper,
    Line,
    Rect,
    Circle,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum RigPin {
    NearSocket(usize),
    FarSocket(usize),
    Pivot(usize),
}

fn export_root_dir() -> PathBuf {
    local_project_root()
        .map(|r| r.join("assets"))
        .unwrap_or_else(user_packs_dir)
}

fn rgba_to_c32(p: RgbaPixel) -> egui::Color32 {
    egui::Color32::from_rgba_unmultiplied(p[0], p[1], p[2], p[3])
}

fn c32_to_rgba(c: egui::Color32) -> RgbaPixel {
    c.to_srgba_unmultiplied()
}

fn shade_pixel(p: RgbaPixel, lighten: bool) -> RgbaPixel {
    if p[3] == 0 {
        return p;
    }
    let d: i16 = if lighten { 16 } else { -16 };
    [
        (p[0] as i16 + d).clamp(0, 255) as u8,
        (p[1] as i16 + d).clamp(0, 255) as u8,
        (p[2] as i16 + d).clamp(0, 255) as u8,
        p[3],
    ]
}

fn buf_to_color_image(buf: &[RgbaPixel]) -> egui::ColorImage {
    let pixels: Vec<egui::Color32> = buf.iter().map(|&p| rgba_to_c32(p)).collect();
    egui::ColorImage {
        size: [BOARD_SIZE, BOARD_SIZE],
        source_size: egui::vec2(BOARD_SIZE as f32, BOARD_SIZE as f32),
        pixels,
    }
}

/// Headless AI Agent CLI mode:
/// - `diypets-creator --from-svg bird.svg --name "Sky Bird" --archetype bird --author "SM" [--out dir] [--activate]`
/// - `diypets-creator --build pet.diypet.json [--out dir] [--activate]`
/// - `diypets-creator --template <cat|buddy|bird|snake|fish> [--name "..."] [--out dir] [--activate]`
/// - `diypets-creator --schema`
fn try_run_cli(args: &[String]) -> Option<Result<(), String>> {
    if args.len() <= 1 {
        return None;
    }
    if args.iter().any(|a| a == "--help" || a == "-h") {
        println!(
            "DIYPets Studio V2 CLI (for AI Agents & Automation)\n\
             Usage:\n\
               diypets-creator --from-svg <file.svg> [--name <Name>] [--archetype quadruped|biped|bird|crawler|swimmer] [--author <Author>] [--out <dir>] [--activate]\n\
               diypets-creator --build <project.diypet.json> [--out <dir>] [--activate]\n\
               diypets-creator --template <cat|buddy|bird|snake|fish> [--name <Name>] [--out <dir>] [--activate]\n\
               diypets-creator --schema"
        );
        return Some(Ok(()));
    }
    if args.iter().any(|a| a == "--schema") {
        let sample = StudioProject::template_bird();
        let dto = diypets::studio_model::SavedProjectV2::from_project(&sample);
        match serde_json::to_string_pretty(&dto) {
            Ok(json) => {
                println!("{json}");
                return Some(Ok(()));
            }
            Err(e) => return Some(Err(format!("schema error: {e}"))),
        }
    }

    let flag_val = |name: &str| -> Option<String> {
        args.iter()
            .position(|a| a == name)
            .and_then(|i| args.get(i + 1))
            .cloned()
    };
    let has_flag = |name: &str| args.iter().any(|a| a == name);

    let mut project = if let Some(svg_path) = flag_val("--from-svg") {
        let svg_text = match std::fs::read_to_string(&svg_path) {
            Ok(s) => s,
            Err(e) => return Some(Err(format!("cannot read {svg_path}: {e}"))),
        };
        let stem = Path::new(&svg_path)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("svg_pet");
        let name = flag_val("--name").unwrap_or_else(|| stem.to_string());
        let arch = flag_val("--archetype")
            .and_then(|s| Archetype::from_str_loose(&s))
            .unwrap_or(Archetype::Quadruped);
        match import_svg_to_project(&svg_text, &name, arch) {
            Ok(p) => p,
            Err(e) => return Some(Err(e)),
        }
    } else if let Some(json_path) = flag_val("--build") {
        match StudioProject::load_json(Path::new(&json_path)) {
            Ok(p) => p,
            Err(e) => return Some(Err(e)),
        }
    } else if let Some(tpl) = flag_val("--template") {
        let arch = Archetype::from_str_loose(&tpl).unwrap_or(Archetype::Quadruped);
        StudioProject::template_by_archetype(arch)
    } else {
        return None;
    };

    if let Some(name) = flag_val("--name") {
        project.display_name = name.clone();
        project.pack_id = sanitize_pack_id(&name);
    }
    if let Some(id) = flag_val("--id") {
        project.pack_id = sanitize_pack_id(&id);
    }
    if let Some(author) = flag_val("--author") {
        project.author = author;
    }

    let out_dir = flag_val("--out")
        .map(PathBuf::from)
        .unwrap_or_else(|| export_root_dir().join(&project.pack_id));

    if let Err(e) = project.export_to_dir(&out_dir) {
        return Some(Err(e));
    }
    println!(
        "Built DIYPets pack '{}' ({}) -> {}",
        project.display_name,
        project.archetype.label(),
        out_dir.display()
    );

    if has_flag("--activate") {
        let sp = Settings::default_path();
        let mut s = Settings::load_sanitized(&sp);
        s.pet_pack = project.pack_id.clone();
        if let Err(e) = s.save(&sp) {
            return Some(Err(format!("exported pack, but failed to activate: {e}")));
        }
        println!("Activated '{}' in {}", project.pack_id, sp.display());
    }
    Some(Ok(()))
}

struct StudioApp {
    project: StudioProject,
    tab: StudioTab,
    edit_layer: EditLayer,
    tool: Tool,
    brush_size: usize,
    current_color: egui::Color32,
    shade_lighten: bool,
    mirror_draw_x: bool,
    show_grid: bool,
    light_checker: bool,
    show_onion_skin: bool,
    zoom: f32,

    undo_stack: Vec<StudioProject>,
    redo_stack: Vec<StudioProject>,

    selected_part_idx: Option<usize>,
    dragging_pin: Option<RigPin>,
    new_part_role: PartRole,
    new_part_name: String,

    selected_custom_anim: usize,
    selected_keyframe: usize,

    last_draw_pos: Option<(usize, usize)>,
    shape_start: Option<(usize, usize)>,
    shape_current: Option<(usize, usize)>,

    preview_clip: PreviewClip,
    preview_playing: bool,
    preview_desktop_scale: bool,
    show_contact_sheet: bool,
    start_time: Instant,

    pack_id_locked: bool,
    confirm_overwrite_path: Option<PathBuf>,
    pending_activate: bool,
    status_msg: Option<(String, bool)>,

    canvas_dirty: bool,
    sheet_dirty: bool,
    tex_active: Option<egui::TextureHandle>,
    tex_body_ghost: Option<egui::TextureHandle>,
    tex_preview: Option<egui::TextureHandle>,
    tex_sheet: Option<egui::TextureHandle>,
    sheet_import_img: Option<image::RgbaImage>,
    sheet_import_tex: Option<egui::TextureHandle>,
    sheet_cols: u32,
    sheet_rows: u32,
    sheet_walk_count: u32,
    sheet_fps_ms: u32,
    sheet_name: String,
}

impl Default for StudioApp {
    fn default() -> Self {
        Self {
            project: StudioProject::template_cat(),
            tab: StudioTab::Draw,
            edit_layer: EditLayer::Body,
            tool: Tool::Brush,
            brush_size: 4,
            current_color: egui::Color32::from_rgb(28, 28, 34),
            shade_lighten: false,
            mirror_draw_x: false,
            show_grid: false,
            light_checker: true,
            show_onion_skin: true,
            zoom: 4.0,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            selected_part_idx: Some(0),
            dragging_pin: None,
            new_part_role: PartRole::Wing,
            new_part_name: "New Wing".to_string(),
            selected_custom_anim: 0,
            selected_keyframe: 0,
            last_draw_pos: None,
            shape_start: None,
            shape_current: None,
            preview_clip: PreviewClip::Walk,
            preview_playing: true,
            preview_desktop_scale: false,
            show_contact_sheet: false,
            start_time: Instant::now(),
            pack_id_locked: false,
            confirm_overwrite_path: None,
            pending_activate: false,
            status_msg: Some((
                "Welcome to DIYPets Studio V2 — pick a template, draw, or click Import 2D SVG!".to_string(),
                false,
            )),
            canvas_dirty: true,
            sheet_dirty: true,
            tex_active: None,
            tex_body_ghost: None,
            tex_preview: None,
            tex_sheet: None,
            sheet_import_img: None,
            sheet_import_tex: None,
            sheet_cols: 4,
            sheet_rows: 1,
            sheet_walk_count: 4,
            sheet_fps_ms: 140,
            sheet_name: "sheet_pet".to_string(),
        }
    }
}

impl StudioApp {
    fn mark_dirty(&mut self) {
        self.canvas_dirty = true;
        self.sheet_dirty = true;
    }

    fn push_undo(&mut self) {
        self.undo_stack.push(self.project.clone());
        if self.undo_stack.len() > MAX_UNDO {
            self.undo_stack.remove(0);
        }
        self.redo_stack.clear();
    }

    fn undo(&mut self) {
        if let Some(prev) = self.undo_stack.pop() {
            self.redo_stack.push(self.project.clone());
            self.project = prev;
            self.clamp_selections();
            self.mark_dirty();
        }
    }

    fn redo(&mut self) {
        if let Some(next) = self.redo_stack.pop() {
            self.undo_stack.push(self.project.clone());
            self.project = next;
            self.clamp_selections();
            self.mark_dirty();
        }
    }

    fn clamp_selections(&mut self) {
        if let EditLayer::Part(i) = self.edit_layer
            && i >= self.project.parts.len()
        {
            self.edit_layer = EditLayer::Body;
        }
        if let Some(i) = self.selected_part_idx
            && i >= self.project.parts.len()
        {
            self.selected_part_idx = if self.project.parts.is_empty() {
                None
            } else {
                Some(0)
            };
        }
        if self.selected_custom_anim >= self.project.custom_animations.len() {
            self.selected_custom_anim = 0;
        }
    }

    fn active_buf(&self) -> &[RgbaPixel] {
        match self.edit_layer {
            EditLayer::Body => &self.project.body,
            EditLayer::Part(i) => self
                .project
                .parts
                .get(i)
                .map(|p| p.pixels.as_slice())
                .unwrap_or(&self.project.body),
            EditLayer::Face(slot) => &self.project.faces[slot.index()],
        }
    }

    fn active_buf_mut(&mut self) -> &mut [RgbaPixel] {
        match self.edit_layer {
            EditLayer::Body => &mut self.project.body,
            EditLayer::Part(i) => {
                if i < self.project.parts.len() {
                    &mut self.project.parts[i].pixels
                } else {
                    &mut self.project.body
                }
            }
            EditLayer::Face(slot) => &mut self.project.faces[slot.index()],
        }
    }

    fn active_layer_title(&self) -> String {
        match self.edit_layer {
            EditLayer::Body => "Body (Core)".to_string(),
            EditLayer::Part(i) => self
                .project
                .parts
                .get(i)
                .map(|p| format!("{} [{}]", p.name, p.role.short_tag()))
                .unwrap_or_else(|| "Body".to_string()),
            EditLayer::Face(slot) => format!("Face: {}", slot.label()),
        }
    }

    fn apply_brush(&mut self, cx: usize, cy: usize, color: RgbaPixel, shade: Option<bool>) {
        let size = self.brush_size as isize;
        let r = size as f32 * 0.5;
        let off = size / 2;
        let mirror = self.mirror_draw_x;
        let buf = self.active_buf_mut();

        let mut stamp = |tx: isize, ty: isize| {
            if tx >= 0 && tx < BOARD_SIZE as isize && ty >= 0 && ty < BOARD_SIZE as isize {
                let idx = ty as usize * BOARD_SIZE + tx as usize;
                if let Some(lighten) = shade {
                    buf[idx] = shade_pixel(buf[idx], lighten);
                } else {
                    buf[idx] = color;
                }
            }
        };

        for dy in 0..size {
            for dx in 0..size {
                if size >= 3 {
                    let fx = dx as f32 + 0.5 - r;
                    let fy = dy as f32 + 0.5 - r;
                    if fx * fx + fy * fy > r * r + 0.25 {
                        continue;
                    }
                }
                let px = cx as isize + dx - off;
                let py = cy as isize + dy - off;
                stamp(px, py);
                if mirror {
                    stamp((BOARD_SIZE as isize - 1) - px, py);
                }
            }
        }
        self.mark_dirty();
    }

    fn paint_line(&mut self, s: (usize, usize), e: (usize, usize), color: RgbaPixel, shade: Option<bool>) {
        let (mut x0, mut y0) = (s.0 as isize, s.1 as isize);
        let (x1, y1) = (e.0 as isize, e.1 as isize);
        let dx = (x1 - x0).abs();
        let dy = -(y1 - y0).abs();
        let sx = if x0 < x1 { 1 } else { -1 };
        let sy = if y0 < y1 { 1 } else { -1 };
        let mut err = dx + dy;
        loop {
            self.apply_brush(x0 as usize, y0 as usize, color, shade);
            if x0 == x1 && y0 == y1 {
                break;
            }
            let e2 = 2 * err;
            if e2 >= dy {
                err += dy;
                x0 += sx;
            }
            if e2 <= dx {
                err += dx;
                y0 += sy;
            }
        }
    }

    fn paint_rect(&mut self, s: (usize, usize), e: (usize, usize), color: RgbaPixel) {
        let x0 = s.0.min(e.0);
        let x1 = s.0.max(e.0).min(BOARD_SIZE - 1);
        let y0 = s.1.min(e.1);
        let y1 = s.1.max(e.1).min(BOARD_SIZE - 1);
        let mirror = self.mirror_draw_x;
        let buf = self.active_buf_mut();
        for y in y0..=y1 {
            for x in x0..=x1 {
                buf[y * BOARD_SIZE + x] = color;
                if mirror {
                    buf[y * BOARD_SIZE + (BOARD_SIZE - 1 - x)] = color;
                }
            }
        }
        self.mark_dirty();
    }

    fn paint_circle(&mut self, s: (usize, usize), e: (usize, usize), color: RgbaPixel) {
        let min_x = s.0.min(e.0) as f32;
        let max_x = s.0.max(e.0) as f32;
        let min_y = s.1.min(e.1) as f32;
        let max_y = s.1.max(e.1) as f32;
        let cx = (min_x + max_x) * 0.5;
        let cy = (min_y + max_y) * 0.5;
        let rx = ((max_x - min_x) * 0.5).max(1.0);
        let ry = ((max_y - min_y) * 0.5).max(1.0);
        let mirror = self.mirror_draw_x;
        let buf = self.active_buf_mut();
        paint_ellipse(buf, cx, cy, rx, ry, color);
        if mirror {
            paint_ellipse(buf, (BOARD_SIZE - 1) as f32 - cx, cy, rx, ry, color);
        }
        self.mark_dirty();
    }

    fn flood_fill(&mut self, sx: usize, sy: usize, color: RgbaPixel) {
        let buf = self.active_buf_mut();
        let idx = sy * BOARD_SIZE + sx;
        let target = buf[idx];
        if target == color {
            return;
        }
        let mut stack = vec![(sx, sy)];
        buf[idx] = color;
        while let Some((x, y)) = stack.pop() {
            for (nx, ny) in [
                (x.wrapping_sub(1), y),
                (x + 1, y),
                (x, y.wrapping_sub(1)),
                (x, y + 1),
            ] {
                if nx < BOARD_SIZE && ny < BOARD_SIZE {
                    let ni = ny * BOARD_SIZE + nx;
                    if buf[ni] == target {
                        buf[ni] = color;
                        stack.push((nx, ny));
                    }
                }
            }
        }
        self.mark_dirty();
    }

    fn recolor_matching(&mut self, sx: usize, sy: usize, new_color: RgbaPixel) {
        let buf = self.active_buf_mut();
        let target = buf[sy * BOARD_SIZE + sx];
        if target[3] == 0 || target == new_color {
            return;
        }
        for px in buf.iter_mut() {
            if *px == target {
                *px = new_color;
            }
        }
        self.mark_dirty();
    }

    fn flip_layer_h(&mut self) {
        self.push_undo();
        let buf = self.active_buf_mut();
        for y in 0..BOARD_SIZE {
            for x in 0..(BOARD_SIZE / 2) {
                buf.swap(y * BOARD_SIZE + x, y * BOARD_SIZE + (BOARD_SIZE - 1 - x));
            }
        }
        self.mark_dirty();
    }

    fn nudge_layer(&mut self, dx: isize, dy: isize) {
        self.push_undo();
        let src = self.active_buf().to_vec();
        let dst = self.active_buf_mut();
        dst.fill([0, 0, 0, 0]);
        for y in 0..BOARD_SIZE {
            for x in 0..BOARD_SIZE {
                let nx = x as isize + dx;
                let ny = y as isize + dy;
                if nx >= 0 && nx < BOARD_SIZE as isize && ny >= 0 && ny < BOARD_SIZE as isize {
                    dst[ny as usize * BOARD_SIZE + nx as usize] = src[y * BOARD_SIZE + x];
                }
            }
        }
        self.mark_dirty();
    }

    fn import_svg_dialog(&mut self) {
        let Some(path) = rfd::FileDialog::new()
            .add_filter("2D Vector SVG", &["svg"])
            .pick_file()
        else {
            return;
        };
        let Ok(svg_text) = std::fs::read_to_string(&path) else {
            self.status_msg = Some(("Could not read SVG file.".to_string(), true));
            return;
        };
        let stem = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("SVG Pet");
        match import_svg_to_project(&svg_text, stem, self.project.archetype) {
            Ok(proj) => {
                let count = proj.parts.len();
                self.push_undo();
                self.project = proj;
                self.edit_layer = EditLayer::Body;
                self.selected_part_idx = if count > 0 { Some(0) } else { None };
                self.pack_id_locked = false;
                self.mark_dirty();
                self.status_msg = Some((
                    format!("Imported SVG '{}' -> auto-split into Body + {count} rigged part(s)!", stem),
                    false,
                ));
            }
            Err(e) => self.status_msg = Some((format!("SVG import error: {e}"), true)),
        }
    }

    fn import_png_layer_dialog(&mut self) {
        let Some(path) = rfd::FileDialog::new()
            .add_filter("PNG Image", &["png"])
            .pick_file()
        else {
            return;
        };
        match image::open(&path) {
            Ok(img) => {
                self.push_undo();
                let fitted = rgba_image_fit_to_board(&img.to_rgba8());
                self.active_buf_mut().copy_from_slice(&fitted);
                self.mark_dirty();
                self.status_msg = Some((format!("Imported PNG into {}", self.active_layer_title()), false));
            }
            Err(e) => self.status_msg = Some((format!("PNG import error: {e}"), true)),
        }
    }

    fn save_project_dialog(&mut self) {
        let Some(path) = rfd::FileDialog::new()
            .set_file_name(format!("{}.diypet.json", self.project.pack_id))
            .add_filter("DIYPets V2 Project", &["json"])
            .save_file()
        else {
            return;
        };
        match self.project.save_json(&path) {
            Ok(()) => self.status_msg = Some((format!("Saved project to {}", path.display()), false)),
            Err(e) => self.status_msg = Some((e, true)),
        }
    }

    fn open_project_dialog(&mut self) {
        let Some(path) = rfd::FileDialog::new()
            .add_filter("DIYPets V2 Project", &["json"])
            .pick_file()
        else {
            return;
        };
        match StudioProject::load_json(&path) {
            Ok(proj) => {
                self.push_undo();
                self.project = proj;
                self.edit_layer = EditLayer::Body;
                self.clamp_selections();
                self.mark_dirty();
                self.status_msg = Some((format!("Opened {}", path.display()), false));
            }
            Err(e) => self.status_msg = Some((e, true)),
        }
    }

    fn export_standalone_xml_dialog(&mut self) {
        let Some(path) = rfd::FileDialog::new()
            .set_file_name(format!("{}.xml", sanitize_pack_id(&self.project.pack_id)))
            .add_filter("DIYPets / eSheep XML", &["xml"])
            .save_file()
        else {
            return;
        };
        match self.project.build_export_artifacts() {
            Ok((_, _, xml)) => match std::fs::write(&path, xml) {
                Ok(()) => self.status_msg = Some((format!("Saved shareable XML to {}", path.display()), false)),
                Err(e) => self.status_msg = Some((format!("Write error: {e}"), true)),
            },
            Err(e) => self.status_msg = Some((e, true)),
        }
    }

    fn request_export(&mut self, activate: bool) {
        let id = sanitize_pack_id(&self.project.pack_id);
        self.project.pack_id = id.clone();
        if BUNDLED_PROTECTED.contains(&id.as_str()) {
            self.status_msg = Some((
                format!("'{id}' is a bundled pet name. Choose a unique Pack ID."),
                true,
            ));
            return;
        }
        let target = export_root_dir().join(&id);
        if target.join("deskpet.xml").is_file() {
            self.confirm_overwrite_path = Some(target);
            self.pending_activate = activate;
            return;
        }
        self.execute_export(&target, activate);
    }

    fn execute_export(&mut self, target: &Path, activate: bool) {
        match self.project.export_to_dir(target) {
            Ok(()) => {
                if activate {
                    let sp = Settings::default_path();
                    let mut s = Settings::load_sanitized(&sp);
                    s.pet_pack = self.project.pack_id.clone();
                    let _ = s.save(&sp);
                    self.status_msg = Some((
                        format!("Exported & activated '{}' on your desktop!", self.project.display_name),
                        false,
                    ));
                } else {
                    self.status_msg = Some((
                        format!("Exported to {} (visible in Settings -> Made by SM)!", target.display()),
                        false,
                    ));
                }
            }
            Err(e) => self.status_msg = Some((e, true)),
        }
        self.confirm_overwrite_path = None;
        self.pending_activate = false;
    }

    fn pick_sprite_sheet_dialog(&mut self, ctx: &egui::Context) {
        let Some(path) = rfd::FileDialog::new()
            .add_filter("PNG Sprite Sheet", &["png"])
            .pick_file()
        else {
            return;
        };
        match image::open(&path) {
            Ok(img) => {
                let mut rgba = img.to_rgba8();
                diypets::catalog::strip_chroma_key_if_opaque(&mut rgba);
                let stem = path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("sheet_pet");
                self.sheet_name = sanitize_pack_id(stem);
                let size = [rgba.width() as usize, rgba.height() as usize];
                let ci = egui::ColorImage::from_rgba_unmultiplied(size, rgba.as_flat_samples().as_slice());
                self.sheet_import_tex = Some(ctx.load_texture("v3_import_sheet", ci, egui::TextureOptions::NEAREST));
                self.sheet_import_img = Some(rgba);
                self.status_msg = Some((format!("Loaded sprite sheet '{}' ({}x{})", self.sheet_name, size[0], size[1]), false));
            }
            Err(e) => self.status_msg = Some((format!("Failed to open PNG: {e}"), true)),
        }
    }

    fn export_imported_sheet(&mut self, activate: bool) {
        let Some(sheet) = &self.sheet_import_img else {
            self.status_msg = Some(("Pick a PNG sprite sheet first.".to_string(), true));
            return;
        };
        let cols = self.sheet_cols.max(1);
        let rows = self.sheet_rows.max(1);
        let tw = sheet.width() / cols;
        let th = sheet.height() / rows;
        if tw == 0 || th == 0 {
            self.status_msg = Some(("Grid Cols/Rows too large for image size.".to_string(), true));
            return;
        }
        let total = cols * rows;
        let walk_n = self.sheet_walk_count.clamp(1, total);

        // Compute GroundY from bottom-most opaque pixel across walk frames
        let mut bottom_y = th.saturating_sub(1);
        let mut found_px = false;
        for idx in 0..walk_n {
            let tx = (idx % cols) * tw;
            let ty = (idx / cols) * th;
            for y in (0..th).rev() {
                if (0..tw).any(|x| sheet.get_pixel(tx + x, ty + y)[3] > 24) {
                    if !found_px {
                        bottom_y = y;
                        found_px = true;
                    } else {
                        bottom_y = bottom_y.max(y);
                    }
                    break;
                }
            }
        }
        let ground_y = (bottom_y + 1).clamp(1, th);

        use base64::{Engine as _, engine::general_purpose::STANDARD};
        let mut png_bytes = Vec::new();
        if let Err(e) = sheet.write_to(&mut std::io::Cursor::new(&mut png_bytes), image::ImageFormat::Png) {
            self.status_msg = Some((format!("PNG encode error: {e}"), true));
            return;
        }
        let b64 = STANDARD.encode(&png_bytes);
        let walk_frames: String = (0..walk_n).map(|f| format!("<Frame>{f}</Frame>")).collect();
        let id = sanitize_pack_id(&self.sheet_name);
        if BUNDLED_PROTECTED.contains(&id.as_str()) {
            self.status_msg = Some((format!("'{id}' is reserved. Pick a unique pet name."), true));
            return;
        }
        let iv = self.sheet_fps_ms.clamp(50, 500);
        let xml = format!(
            "<?xml version=\"1.0\" encoding=\"utf-8\"?>\n<AnimationXML>\n  <Header><Title>{id}</Title><Author>{}</Author></Header>\n  <Image><TilesX>{cols}</TilesX><TilesY>{rows}</TilesY><GroundY>{ground_y}</GroundY><Png>{b64}</Png></Image>\n  <Spawns><Spawn id=\"1\" probability=\"100\"><X>screenW/2</X><Y>0</Y><Next>5</Next></Spawn></Spawns>\n  <Animation id=\"1\"><Name>walk</Name><Start><X>-6</X><Y>0</Y><Interval>{iv}</Interval><Opacity>1</Opacity></Start><End><X>-6</X><Y>0</Y><Interval>{iv}</Interval><Opacity>1</Opacity></End><Sequence repeat=\"4\" repeatfrom=\"0\">{walk_frames}</Sequence><Border><Next probability=\"100\">2</Next></Border><Gravity><Next probability=\"100\">5</Next></Gravity><Next probability=\"75\">1</Next><Next probability=\"25\">12</Next></Animation>\n  <Animation id=\"2\"><Name>turn</Name><Action>flip</Action><Start><X>0</X><Y>0</Y><Interval>160</Interval><Opacity>1</Opacity></Start><End><X>0</X><Y>0</Y><Interval>160</Interval><Opacity>1</Opacity></End><Sequence repeat=\"0\" repeatfrom=\"0\"><Frame>0</Frame></Sequence><Next probability=\"100\">1</Next></Animation>\n  <Animation id=\"4\"><Name>drag</Name><Start><X>0</X><Y>0</Y><Interval>140</Interval><Opacity>1</Opacity></Start><End><X>0</X><Y>0</Y><Interval>140</Interval><Opacity>1</Opacity></End><Sequence repeat=\"2\" repeatfrom=\"0\"><Frame>0</Frame></Sequence><Next probability=\"100\">4</Next></Animation>\n  <Animation id=\"5\"><Name>fall</Name><Start><X>0</X><Y>12</Y><Interval>60</Interval><Opacity>1</Opacity></Start><End><X>0</X><Y>18</Y><Interval>60</Interval><Opacity>1</Opacity></End><Sequence repeat=\"1\" repeatfrom=\"0\"><Frame>0</Frame></Sequence><Border><Next probability=\"100\">9</Next></Border><Next probability=\"100\">5</Next></Animation>\n  <Animation id=\"9\"><Name>fall soft</Name><Start><X>0</X><Y>0</Y><Interval>140</Interval><Opacity>1</Opacity></Start><End><X>0</X><Y>0</Y><Interval>140</Interval><Opacity>1</Opacity></End><Sequence repeat=\"0\" repeatfrom=\"0\"><Frame>0</Frame></Sequence><Next probability=\"100\">1</Next></Animation>\n  <Animation id=\"12\"><Name>idle</Name><Start><X>0</X><Y>0</Y><Interval>350</Interval><Opacity>1</Opacity></Start><End><X>0</X><Y>0</Y><Interval>350</Interval><Opacity>1</Opacity></End><Sequence repeat=\"3\" repeatfrom=\"0\"><Frame>0</Frame></Sequence><Gravity><Next probability=\"100\">5</Next></Gravity><Next probability=\"100\">1</Next></Animation>\n</AnimationXML>\n",
            self.project.author
        );
        if let Err(e) = diypets::deskpet_xml::DeskpetPack::parse(&xml) {
            self.status_msg = Some((format!("Sheet validation failed: {e}"), true));
            return;
        }
        let target = export_root_dir().join(&id);
        let _ = std::fs::create_dir_all(&target);
        if let Err(e) = std::fs::write(target.join("deskpet.xml"), &xml) {
            self.status_msg = Some((format!("Write error: {e}"), true));
            return;
        }
        let icon = image::imageops::crop_imm(sheet, 0, 0, tw, th).to_image();
        let _ = icon.save(target.join("icon.png"));
        if activate {
            let sp = Settings::default_path();
            let mut s = Settings::load_sanitized(&sp);
            s.pet_pack = id.clone();
            let _ = s.save(&sp);
            self.status_msg = Some((format!("Imported & activated sprite sheet '{id}' on your desktop!"), false));
        } else {
            self.status_msg = Some((format!("Saved sprite sheet pet '{id}' to library!"), false));
        }
    }
    fn sync_canvas_textures(&mut self, ctx: &egui::Context) {
        if !self.canvas_dirty {
            return;
        }
        self.canvas_dirty = false;
        let opts = egui::TextureOptions::NEAREST;

        let active_img = buf_to_color_image(self.active_buf());
        if let Some(t) = &mut self.tex_active {
            t.set(active_img, opts);
        } else {
            self.tex_active = Some(ctx.load_texture("v2_active", active_img, opts));
        }

        let ghost_img = buf_to_color_image(&self.project.body);
        if let Some(t) = &mut self.tex_body_ghost {
            t.set(ghost_img, opts);
        } else {
            self.tex_body_ghost = Some(ctx.load_texture("v2_ghost", ghost_img, opts));
        }
    }

    fn draw_checkerboard(painter: &egui::Painter, rect: egui::Rect, tile_px: f32, light_mode: bool) {
        let (light, dark) = if light_mode {
            (egui::Color32::from_rgb(238, 238, 242), egui::Color32::from_rgb(214, 216, 224))
        } else {
            (egui::Color32::from_rgb(54, 58, 68), egui::Color32::from_rgb(42, 46, 54))
        };
        painter.rect_filled(rect, 4.0, light);
        let cols = (rect.width() / tile_px).ceil() as i32;
        let rows = (rect.height() / tile_px).ceil() as i32;
        for row in 0..rows {
            for col in 0..cols {
                if (row + col) % 2 == 1 {
                    let min = egui::pos2(
                        rect.min.x + col as f32 * tile_px,
                        rect.min.y + row as f32 * tile_px,
                    );
                    let max = egui::pos2(
                        (min.x + tile_px).min(rect.max.x),
                        (min.y + tile_px).min(rect.max.y),
                    );
                    painter.rect_filled(egui::Rect::from_min_max(min, max), 0.0, dark);
                }
            }
        }
    }

    fn render_preview_box(&mut self, ui: &mut egui::Ui, box_size: f32, show_stage: bool) {
        let t = if self.preview_playing {
            self.start_time.elapsed().as_secs_f32()
        } else {
            0.0
        };
        let pose = if !self.preview_playing
            && self.tab == StudioTab::Animate
            && matches!(self.preview_clip, PreviewClip::Custom(_))
        {
            if let Some(anim) = self.project.custom_animations.get(self.selected_custom_anim)
                && !anim.keyframes.is_empty()
            {
                let kf_idx = self.selected_keyframe.min(anim.keyframes.len() - 1);
                self.project.keyframe_to_pose(&anim.keyframes[kf_idx])
            } else {
                self.project.live_pose(self.preview_clip, t)
            }
        } else {
            self.project.live_pose(self.preview_clip, t)
        };
        let pm = self.project.render_pose(pose);

        let side = pm.width() as usize;
        let pixels: Vec<egui::Color32> = pm
            .pixels()
            .iter()
            .map(|p| egui::Color32::from_rgba_premultiplied(p.red(), p.green(), p.blue(), p.alpha()))
            .collect();
        let img = egui::ColorImage {
            size: [side, side],
            source_size: egui::vec2(side as f32, side as f32),
            pixels,
        };
        if let Some(tex) = &mut self.tex_preview {
            tex.set(img, egui::TextureOptions::LINEAR);
        } else {
            self.tex_preview = Some(ui.ctx().load_texture("v2_preview", img, egui::TextureOptions::LINEAR));
        }

        let (rect, _) = ui.allocate_exact_size(egui::vec2(box_size, box_size), egui::Sense::hover());
        let painter = ui.painter_at(rect);

        // Theme-matched background per archetype / clip
        if self.project.archetype == Archetype::Swimmer {
            painter.rect_filled(rect, 6.0, egui::Color32::from_rgb(22, 54, 78));
            // Gentle rising water bubbles
            for i in 0..5 {
                let bx = rect.min.x + 24.0 + i as f32 * (box_size - 48.0) / 4.0;
                let by = rect.max.y - ((t * 36.0 + i as f32 * 47.0) % box_size);
                painter.circle_stroke(
                    egui::pos2(bx, by),
                    3.5,
                    egui::Stroke::new(1.0, egui::Color32::from_rgba_unmultiplied(150, 220, 255, 110)),
                );
            }
        } else {
            Self::draw_checkerboard(&painter, rect, 12.0, false);
        }

        if self.preview_clip == PreviewClip::Climb {
            let wall_w = 24.0;
            let wall = egui::Rect::from_min_max(rect.min, egui::pos2(rect.min.x + wall_w, rect.max.y));
            painter.rect_filled(wall, 0.0, egui::Color32::from_rgb(34, 42, 56));
            let scroll = (t * 42.0) % 24.0;
            let mut y = rect.min.y + scroll;
            while y < rect.max.y {
                painter.line_segment(
                    [egui::pos2(rect.min.x + 4.0, y), egui::pos2(rect.min.x + wall_w - 4.0, y)],
                    egui::Stroke::new(1.5, egui::Color32::from_rgb(80, 96, 124)),
                );
                y += 24.0;
            }
        } else if self.preview_clip == PreviewClip::Fall {
            let st = egui::Stroke::new(1.5, egui::Color32::from_rgba_unmultiplied(160, 195, 255, 120));
            for i in 0..6 {
                let x = rect.min.x + 20.0 + i as f32 * (box_size - 40.0) / 5.0;
                let off = ((t * 220.0 + i as f32 * 53.0) % (box_size + 40.0)) - 20.0;
                let y = rect.max.y - off;
                painter.line_segment([egui::pos2(x, y), egui::pos2(x, y - 22.0)], st);
            }
        } else if show_stage {
            let floor = egui::Rect::from_min_max(egui::pos2(rect.min.x, rect.max.y - 22.0), rect.max);
            painter.rect_filled(floor, 0.0, egui::Color32::from_rgb(26, 30, 40));
            painter.text(
                egui::pos2(rect.min.x + 8.0, rect.max.y - 11.0),
                egui::Align2::LEFT_CENTER,
                format!("{} Stage", self.project.archetype.label()),
                egui::FontId::proportional(10.0),
                egui::Color32::from_rgb(165, 178, 202),
            );
        }

        if let Some(tex) = &self.tex_preview {
            let draw_side = if self.preview_desktop_scale {
                128.0_f32.min(box_size)
            } else {
                box_size * 0.90
            };
            let (cx, cy) = match self.preview_clip {
                PreviewClip::Climb => (rect.min.x + 24.0 + draw_side * 0.28, rect.center().y),
                PreviewClip::Walk if show_stage && self.preview_playing => {
                    let u = (t % 4.0) / 4.0;
                    let ping = if u < 0.5 { 1.0 - u * 4.0 } else { (u - 0.5) * 4.0 - 1.0 };
                    (rect.center().x + ping * ((box_size - draw_side) * 0.35).max(0.0), rect.center().y - 6.0)
                }
                _ => (rect.center().x, rect.center().y - if show_stage { 6.0 } else { 0.0 }),
            };
            let flip = show_stage
                && self.preview_clip == PreviewClip::Walk
                && self.preview_playing
                && ((t % 4.0) >= 2.0);
            let uv = if flip {
                egui::Rect::from_min_max(egui::pos2(1.0, 0.0), egui::pos2(0.0, 1.0))
            } else {
                egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0))
            };
            let dst = egui::Rect::from_center_size(egui::pos2(cx, cy), egui::vec2(draw_side, draw_side));
            painter.image(tex.id(), dst, uv, egui::Color32::WHITE);
        }

        painter.rect_stroke(
            rect,
            6.0,
            egui::Stroke::new(1.0, egui::Color32::from_rgb(76, 84, 100)),
            egui::StrokeKind::Inside,
        );
        if self.preview_playing {
            ui.ctx().request_repaint();
        }
    }

    fn render_clip_picker(&mut self, ui: &mut egui::Ui) {
        ui.horizontal_wrapped(|ui| {
            for (clip, label) in [
                (PreviewClip::Walk, "Walk"),
                (PreviewClip::Idle, "Idle"),
                (PreviewClip::Sit, "Sit"),
                (PreviewClip::Sleep, "Sleep"),
                (PreviewClip::Drag, "Drag"),
                (PreviewClip::Fall, "Fall"),
                (PreviewClip::Climb, "Climb"),
            ] {
                ui.selectable_value(&mut self.preview_clip, clip, label);
            }
            for (i, ca) in self.project.custom_animations.iter().enumerate() {
                ui.selectable_value(&mut self.preview_clip, PreviewClip::Custom(i), &ca.name);
            }
        });
    }

    fn render_contact_sheet(&mut self, ui: &mut egui::Ui) {
        if self.sheet_dirty || self.tex_sheet.is_none() {
            if let Ok((sheet, _, _)) = self.project.build_export_artifacts() {
                let size = [sheet.width() as usize, sheet.height() as usize];
                let ci = egui::ColorImage::from_rgba_unmultiplied(size, sheet.as_flat_samples().as_slice());
                self.tex_sheet = Some(ui.ctx().load_texture("v2_sheet", ci, egui::TextureOptions::LINEAR));
                self.sheet_dirty = false;
            }
        }
        if let Some(tex) = &self.tex_sheet {
            ui.label(
                egui::RichText::new("Exported Sprite Sheet (Walk 0-7 | Idle 8-9 | Sit 10-11 | Sleep 12-13 | Drag 14-15 | Fall/Land 16-17 | Climb 18-19 | Custom 20+)")
                    .size(11.0)
                    .color(egui::Color32::GRAY),
            );
            egui::ScrollArea::vertical().show(ui, |ui| {
                let avail_w = ui.available_width().max(240.0);
                let s = tex.size_vec2();
                let sc = (avail_w / s.x).min(1.0);
                ui.image((tex.id(), s * sc));
            });
        }
    }
}

impl eframe::App for StudioApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.sync_canvas_textures(&ctx);

        let wants_kb = ctx.egui_wants_keyboard_input();
        ctx.input(|i| {
            if i.modifiers.command && i.key_pressed(egui::Key::Z) {
                if i.modifiers.shift {
                    self.redo();
                } else {
                    self.undo();
                }
            } else if i.modifiers.command && i.key_pressed(egui::Key::Y) {
                self.redo();
            } else if !wants_kb && self.tab == StudioTab::Draw {
                if i.key_pressed(egui::Key::B) {
                    self.tool = Tool::Brush;
                } else if i.key_pressed(egui::Key::E) {
                    self.tool = Tool::Eraser;
                } else if i.key_pressed(egui::Key::G) {
                    self.tool = Tool::Fill;
                } else if i.key_pressed(egui::Key::H) {
                    self.tool = Tool::Shade;
                } else if i.key_pressed(egui::Key::R) {
                    self.tool = Tool::Recolor;
                } else if i.key_pressed(egui::Key::I) {
                    self.tool = Tool::Eyedropper;
                } else if i.key_pressed(egui::Key::L) {
                    self.tool = Tool::Line;
                } else if i.key_pressed(egui::Key::U) {
                    self.tool = Tool::Rect;
                } else if i.key_pressed(egui::Key::O) {
                    self.tool = Tool::Circle;
                } else if i.key_pressed(egui::Key::M) {
                    self.mirror_draw_x = !self.mirror_draw_x;
                }
            }
        });

        // TOP HEADER BAR
        egui::Panel::top("v2_top").show(ui, |ui| {
            ui.add_space(2.0);
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new("DIYPets Studio V3")
                        .strong()
                        .size(16.0)
                        .color(egui::Color32::from_rgb(130, 195, 255)),
                );
                ui.separator();
                ui.selectable_value(&mut self.tab, StudioTab::Draw, "1. Draw & Layers");
                ui.selectable_value(&mut self.tab, StudioTab::Rig, "2. Rig & Parts");
                ui.selectable_value(&mut self.tab, StudioTab::Animate, "3. Custom Anims");
                ui.selectable_value(&mut self.tab, StudioTab::Export, "4. Export & Share");
                ui.selectable_value(&mut self.tab, StudioTab::ImportSheet, "5. Sprite Sheet");

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.add(egui::Button::new(egui::RichText::new("Run on Desktop").strong().color(egui::Color32::WHITE)).fill(egui::Color32::from_rgb(46, 134, 86))).clicked() { self.request_export(true); }
                    if ui.button("Open Project").clicked() {
                        self.open_project_dialog();
                    }
                    if ui.button("Save Project").clicked() {
                        self.save_project_dialog();
                    }
                    if ui.button("Import 2D SVG").clicked() {
                        self.import_svg_dialog();
                    }
                    ui.separator();
                    if ui.button("Redo").clicked() {
                        self.redo();
                    }
                    if ui.button("Undo").clicked() {
                        self.undo();
                    }
                });
            });

            if let Some((msg, is_err)) = &self.status_msg {
                let col = if *is_err {
                    egui::Color32::from_rgb(245, 110, 110)
                } else {
                    egui::Color32::from_rgb(115, 215, 145)
                };
                ui.label(egui::RichText::new(msg).size(12.0).color(col));
            }
            ui.add_space(2.0);
        });

        // RIGHT PANEL: Live Preview + Layer & Face Stack (always accessible in Draw/Rig/Animate)
        if !matches!(self.tab, StudioTab::Export | StudioTab::ImportSheet) {
            egui::Panel::right("v2_right").show(ui, |ui| {
                ui.set_width(268.0);
                egui::ScrollArea::vertical().show(ui, |ui| {
                    ui.label(egui::RichText::new("Live Preview").strong().size(14.0));
                    self.render_clip_picker(ui);
                    ui.add_space(4.0);
                    self.render_preview_box(ui, 248.0, false);
                    ui.horizontal(|ui| {
                        ui.checkbox(&mut self.preview_playing, "Animate");
                        ui.checkbox(&mut self.preview_desktop_scale, "128px");
                    });

                    ui.separator();
                    ui.label(egui::RichText::new("Body & Extra Parts").strong());
                    if ui
                        .selectable_label(self.edit_layer == EditLayer::Body, "Body (Core) [Face LEFT]")
                        .clicked()
                    {
                        self.edit_layer = EditLayer::Body;
                        self.canvas_dirty = true;
                    }

                    let mut delete_idx: Option<usize> = None;
                    let mut move_up: Option<usize> = None;
                    let mut move_dn: Option<usize> = None;
                    let part_count = self.project.parts.len();
                    for i in 0..part_count {
                        let sel = self.edit_layer == EditLayer::Part(i);
                        let rgb = self.project.parts[i].role.pin_rgb();
                        let tag = self.project.parts[i].role.short_tag();
                        let label = format!("[{tag}] {}", self.project.parts[i].name);
                        ui.horizontal(|ui| {
                            if ui.checkbox(&mut self.project.parts[i].visible, "").changed() {
                                self.sheet_dirty = true;
                            }
                            let (dot, _) = ui.allocate_exact_size(egui::vec2(8.0, 8.0), egui::Sense::hover());
                            ui.painter().circle_filled(
                                dot.center(),
                                4.0,
                                egui::Color32::from_rgb(rgb[0], rgb[1], rgb[2]),
                            );
                            if ui.selectable_label(sel, label).clicked() {
                                self.edit_layer = EditLayer::Part(i);
                                self.selected_part_idx = Some(i);
                                self.canvas_dirty = true;
                            }
                            if i > 0 && ui.small_button("^").on_hover_text("Move part earlier").clicked() {
                                move_up = Some(i);
                            }
                            if i + 1 < part_count && ui.small_button("v").on_hover_text("Move part later").clicked() {
                                move_dn = Some(i);
                            }
                            if ui.small_button("x").clicked() {
                                delete_idx = Some(i);
                            }
                        });
                    }
                    if let Some(u) = move_up {
                        self.push_undo();
                        self.project.parts.swap(u, u - 1);
                        self.selected_part_idx = Some(u - 1);
                        self.edit_layer = EditLayer::Part(u - 1);
                        self.mark_dirty();
                    } else if let Some(d) = move_dn {
                        self.push_undo();
                        self.project.parts.swap(d, d + 1);
                        self.selected_part_idx = Some(d + 1);
                        self.edit_layer = EditLayer::Part(d + 1);
                        self.mark_dirty();
                    } else if let Some(di) = delete_idx {
                        self.push_undo();
                        self.project.parts.remove(di);
                        self.clamp_selections();
                        self.mark_dirty();
                    }

                    ui.add_space(4.0);
                    ui.horizontal(|ui| {
                        ui.add(egui::TextEdit::singleline(&mut self.new_part_name).desired_width(110.0));
                        if ui.small_button("+ Add Part").clicked() {
                            self.push_undo();
                            let nm = if self.new_part_name.trim().is_empty() {
                                self.new_part_role.label().to_string()
                            } else {
                                self.new_part_name.trim().to_string()
                            };
                            self.project.parts.push(RigPart::new(nm, self.new_part_role));
                            let idx = self.project.parts.len() - 1;
                            self.selected_part_idx = Some(idx);
                            self.edit_layer = EditLayer::Part(idx);
                            self.mark_dirty();
                        }
                    });
                    ui.horizontal_wrapped(|ui| {
                        for &role in PartRole::all() {
                            if ui
                                .selectable_label(self.new_part_role == role, role.short_tag())
                                .on_hover_text(role.label())
                                .clicked()
                            {
                                self.new_part_role = role;
                                self.new_part_name = role.label().to_string();
                            }
                        }
                    });

                    ui.separator();
                    ui.label(egui::RichText::new("Face Expressions (Auto-Swapped)").strong());
                    ui.horizontal_wrapped(|ui| {
                        for &slot in FaceSlot::all() {
                            let active = self.edit_layer == EditLayer::Face(slot);
                            if ui.selectable_label(active, slot.label()).clicked() {
                                self.edit_layer = EditLayer::Face(slot);
                                self.canvas_dirty = true;
                            }
                        }
                    });
                });
            });
        }

        // LEFT PANEL: Step-Specific Inspector
        egui::Panel::left("v2_left").show(ui, |ui| {
            ui.set_width(250.0);
            egui::ScrollArea::vertical().show(ui, |ui| match self.tab {
                StudioTab::Draw => {
                    ui.label(egui::RichText::new("Creature Templates").strong());
                    ui.horizontal_wrapped(|ui| {
                        for (arch, lbl) in [
                            (Archetype::Quadruped, "Cat"),
                            (Archetype::Biped, "Buddy"),
                            (Archetype::Bird, "Bird"),
                            (Archetype::Crawler, "Snake"),
                            (Archetype::Swimmer, "Fish"),
                        ] {
                            if ui.button(lbl).clicked() {
                                self.push_undo();
                                self.project = StudioProject::template_by_archetype(arch);
                                self.edit_layer = EditLayer::Body;
                                self.clamp_selections();
                                self.mark_dirty();
                                self.status_msg = Some((format!("Loaded {lbl} ({}) template.", arch.label()), false));
                            }
                        }
                        if ui.button("Blank").clicked() {
                            self.push_undo();
                            self.project = StudioProject::blank(self.project.archetype);
                            self.edit_layer = EditLayer::Body;
                            self.clamp_selections();
                            self.mark_dirty();
                        }
                    });

                    ui.horizontal(|ui| {
                        if ui.button("Import PNG").clicked() {
                            self.import_png_layer_dialog();
                        }
                        if ui.button("Clear Layer").clicked() {
                            self.push_undo();
                            self.active_buf_mut().fill([0, 0, 0, 0]);
                            self.mark_dirty();
                        }
                    });
                    ui.checkbox(&mut self.show_onion_skin, "Body Ghost on Parts & Faces");
                    ui.checkbox(&mut self.mirror_draw_x, "Mirror X Symmetry (M)");
                    ui.checkbox(&mut self.show_grid, "Pixel Grid");
                    ui.checkbox(&mut self.light_checker, "Light Checkerboard");

                    ui.separator();
                    ui.label(egui::RichText::new("Tools").strong());
                    ui.horizontal_wrapped(|ui| {
                        ui.selectable_value(&mut self.tool, Tool::Brush, "Brush (B)");
                        ui.selectable_value(&mut self.tool, Tool::Eraser, "Erase (E)");
                        ui.selectable_value(&mut self.tool, Tool::Fill, "Fill (G)");
                        ui.selectable_value(&mut self.tool, Tool::Shade, "Shade (H)");
                        ui.selectable_value(&mut self.tool, Tool::Recolor, "Recolor (R)");
                        ui.selectable_value(&mut self.tool, Tool::Eyedropper, "Pick (I)");
                        ui.selectable_value(&mut self.tool, Tool::Line, "Line (L)");
                        ui.selectable_value(&mut self.tool, Tool::Rect, "Rect (U)");
                        ui.selectable_value(&mut self.tool, Tool::Circle, "Oval (O)");
                    });
                    if self.tool == Tool::Shade {
                        ui.horizontal(|ui| {
                            ui.selectable_value(&mut self.shade_lighten, false, "Darken");
                            ui.selectable_value(&mut self.shade_lighten, true, "Lighten");
                        });
                    }
                    ui.add(egui::Slider::new(&mut self.brush_size, 1..=20).text("Brush px"));
                    ui.add(egui::Slider::new(&mut self.zoom, 2.0..=6.0).text("Zoom"));

                    ui.separator();
                    ui.label(egui::RichText::new("Palette").strong());
                    ui.horizontal(|ui| {
                        ui.color_edit_button_srgba(&mut self.current_color);
                        ui.label("Active Color");
                    });
                    ui.add_space(4.0);
                    egui::Grid::new("v2_palette").spacing([4.0, 4.0]).show(ui, |ui| {
                        for (idx, &(r, g, b)) in PALETTE.iter().enumerate() {
                            let c = egui::Color32::from_rgb(r, g, b);
                            let (rect, resp) =
                                ui.allocate_exact_size(egui::vec2(20.0, 20.0), egui::Sense::click());
                            ui.painter().rect_filled(rect, 3.0, c);
                            let st = if self.current_color == c {
                                egui::Stroke::new(2.0, egui::Color32::YELLOW)
                            } else {
                                egui::Stroke::new(1.0, egui::Color32::from_gray(90))
                            };
                            ui.painter().rect_stroke(rect, 3.0, st, egui::StrokeKind::Inside);
                            if resp.clicked() {
                                self.current_color = c;
                                if self.tool == Tool::Eraser {
                                    self.tool = Tool::Brush;
                                }
                            }
                            if (idx + 1) % 5 == 0 {
                                ui.end_row();
                            }
                        }
                    });

                    ui.separator();
                    ui.label(egui::RichText::new("Polish & Transform").strong());
                    if ui.button("1-Click Auto-Outline").clicked() {
                        self.push_undo();
                        let oc = c32_to_rgba(self.current_color);
                        paint_outline(self.active_buf_mut(), oc);
                        self.mark_dirty();
                    }
                    ui.horizontal(|ui| {
                        if ui.small_button("Flip H").clicked() {
                            self.flip_layer_h();
                        }
                        if ui.small_button("<-").clicked() {
                            self.nudge_layer(-2, 0);
                        }
                        if ui.small_button("->").clicked() {
                            self.nudge_layer(2, 0);
                        }
                        if ui.small_button("Up").clicked() {
                            self.nudge_layer(0, -2);
                        }
                        if ui.small_button("Dn").clicked() {
                            self.nudge_layer(0, 2);
                        }
                    });
                }

                StudioTab::Rig => {
                    ui.label(egui::RichText::new("Creature Archetype").strong());
                    for &arch in Archetype::all() {
                        if ui
                            .selectable_value(&mut self.project.archetype, arch, arch.label())
                            .changed()
                        {
                            if matches!(arch, Archetype::Crawler | Archetype::Swimmer)
                                && self.project.wave_amplitude < 2.0
                            {
                                self.project.wave_amplitude = 5.0;
                            }
                            self.sheet_dirty = true;
                        }
                    }

                    ui.separator();
                    ui.label(egui::RichText::new("Global Motion").strong());
                    if ui.checkbox(&mut self.project.shade_far_limbs, "2.5D Far-Limb Shading").changed() {
                        self.sheet_dirty = true;
                    }
                    if ui.checkbox(&mut self.project.seamless_outline, "Seamless Silhouette Outline").changed() {
                        self.sheet_dirty = true;
                    }
                    if self.project.seamless_outline {
                        ui.horizontal(|ui| {
                            let mut c = egui::Color32::from_rgb(
                                self.project.outline_color[0],
                                self.project.outline_color[1],
                                self.project.outline_color[2],
                            );
                            if ui.color_edit_button_srgba(&mut c).changed() {
                                self.project.outline_color = [c.r(), c.g(), c.b()];
                                self.sheet_dirty = true;
                            }
                            ui.label("Outline Ink");
                        });
                    }
                    ui.add(egui::Slider::new(&mut self.project.global_limb_scale, 0.5..=1.8).text("Limb Scale"));
                    ui.add(egui::Slider::new(&mut self.project.global_swing_mult, 0.3..=2.0).text("Swing Mult"));
                    ui.add(egui::Slider::new(&mut self.project.body_bob, 0.0..=6.0).text("Walk Bob"));
                    ui.add(egui::Slider::new(&mut self.project.wave_amplitude, 0.0..=10.0).text("Wave Slither"));

                    if ui.button("Auto-Align All Joints").clicked() {
                        self.project.auto_fit_anchors();
                        self.sheet_dirty = true;
                    }

                    ui.separator();
                    if let Some(idx) = self.selected_part_idx
                        && idx < self.project.parts.len()
                    {
                        let p = &mut self.project.parts[idx];
                        ui.label(egui::RichText::new(format!("Selected Part: {}", p.name)).strong());
                        ui.text_edit_singleline(&mut p.name);
                        ui.horizontal_wrapped(|ui| {
                            for &r in PartRole::all() {
                                ui.selectable_value(&mut p.role, r, r.short_tag())
                                    .on_hover_text(r.label());
                            }
                        });
                        ui.checkbox(&mut p.paired, "Paired (Near + Shaded Far Twin)");
                        ui.checkbox(&mut p.mirror_far, "Mirror Far Twin");
                        ui.checkbox(&mut p.behind_body, "Render Behind Body");
                        ui.add(egui::Slider::new(&mut p.swing_deg, 0.0..=60.0).text("Swing deg"));
                        ui.add(egui::Slider::new(&mut p.phase_offset_deg, 0.0..=360.0).text("Phase deg"));
                        ui.add(egui::Slider::new(&mut p.scale, 0.4..=2.0).text("Part Scale"));
                        ui.label(
                            egui::RichText::new(format!(
                                "Near ({},{}) | Far ({},{}) | Pivot ({},{})",
                                p.socket_near.0, p.socket_near.1, p.socket_far.0, p.socket_far.1, p.pivot.0, p.pivot.1
                            ))
                            .size(10.0)
                            .color(egui::Color32::GRAY),
                        );
                    } else {
                        ui.label("Select a part in the right sidebar to tweak its rig.");
                    }
                }

                StudioTab::Animate => {
                    ui.label(egui::RichText::new("Custom Animations").strong());
                    ui.horizontal_wrapped(|ui| {
                        if ui.button("+ Hop").clicked() {
                            self.project.custom_animations.push(CustomAnimation::hop_preset());
                            self.selected_custom_anim = self.project.custom_animations.len() - 1;
                            self.preview_clip = PreviewClip::Custom(self.selected_custom_anim);
                            self.sheet_dirty = true;
                        }
                        if ui.button("+ Dance").clicked() {
                            self.project.custom_animations.push(CustomAnimation::dance_preset());
                            self.selected_custom_anim = self.project.custom_animations.len() - 1;
                            self.preview_clip = PreviewClip::Custom(self.selected_custom_anim);
                            self.sheet_dirty = true;
                        }
                        if ui.button("+ Blank").clicked() {
                            self.project.custom_animations.push(CustomAnimation {
                                name: format!("Action {}", self.project.custom_animations.len() + 1),
                                interval_ms: 140,
                                step_x: 0,
                                step_y: 0,
                                repeat: 2,
                                probability: 10,
                                keyframes: vec![CustomKeyframe::default(), CustomKeyframe::default()],
                            });
                            self.selected_custom_anim = self.project.custom_animations.len() - 1;
                            self.preview_clip = PreviewClip::Custom(self.selected_custom_anim);
                            self.sheet_dirty = true;
                        }
                    });

                    let mut remove_anim: Option<usize> = None;
                    for i in 0..self.project.custom_animations.len() {
                        ui.horizontal(|ui| {
                            if ui
                                .selectable_label(
                                    self.selected_custom_anim == i,
                                    &self.project.custom_animations[i].name,
                                )
                                .clicked()
                            {
                                self.selected_custom_anim = i;
                                self.selected_keyframe = 0;
                                self.preview_clip = PreviewClip::Custom(i);
                            }
                            if ui.small_button("x").clicked() {
                                remove_anim = Some(i);
                            }
                        });
                    }
                    if let Some(ri) = remove_anim {
                        self.project.custom_animations.remove(ri);
                        self.selected_custom_anim = 0;
                        self.preview_clip = PreviewClip::Walk;
                        self.sheet_dirty = true;
                    }

                    ui.separator();
                    if let Some(anim) = self.project.custom_animations.get_mut(self.selected_custom_anim) {
                        ui.label("Animation Name:");
                        ui.text_edit_singleline(&mut anim.name);
                        ui.add(egui::Slider::new(&mut anim.interval_ms, 50..=500).text("Frame ms"));
                        ui.add(egui::Slider::new(&mut anim.step_x, -20..=20).text("Move X"));
                        ui.add(egui::Slider::new(&mut anim.step_y, -20..=20).text("Move Y"));
                        ui.add(egui::Slider::new(&mut anim.repeat, 0..=8).text("Repeats"));
                        ui.add(egui::Slider::new(&mut anim.probability, 0..=50).text("Walk Prob %"));
                        self.sheet_dirty = true;
                    }
                }

                StudioTab::ImportSheet => {
                    ui.label(egui::RichText::new("PNG Sprite Sheet Importer").strong());
                    ui.label(egui::RichText::new("Drop any Aseprite / itch.io / Piskel PNG grid and turn it into a desktop pet in 10 seconds.").size(11.0).color(egui::Color32::GRAY));
                    ui.add_space(6.0);
                    if ui.button("Choose PNG Sprite Sheet...").clicked() {
                        self.pick_sprite_sheet_dialog(ui.ctx());
                    }
                    ui.separator();
                    ui.label("Pet ID:");
                    ui.text_edit_singleline(&mut self.sheet_name);
                    ui.add(egui::Slider::new(&mut self.sheet_cols, 1..=20).text("Grid Cols (X)"));
                    ui.add(egui::Slider::new(&mut self.sheet_rows, 1..=20).text("Grid Rows (Y)"));
                    let max_f = (self.sheet_cols * self.sheet_rows).max(1);
                    self.sheet_walk_count = self.sheet_walk_count.clamp(1, max_f);
                    ui.add(egui::Slider::new(&mut self.sheet_walk_count, 1..=max_f).text("Walk Frames"));
                    ui.add(egui::Slider::new(&mut self.sheet_fps_ms, 60..=400).text("Frame ms"));
                    ui.add_space(8.0);
                    if ui.button("Export Sheet & Run on Desktop").clicked() {
                        self.export_imported_sheet(true);
                    }
                    if ui.button("Export Sheet to Library").clicked() {
                        self.export_imported_sheet(false);
                    }
                }
                StudioTab::ImportSheet => {
                ui.heading("Sprite Sheet Grid Preview");
                if let Some(tex) = &self.sheet_import_tex {
                    egui::ScrollArea::both().show(ui, |ui| {
                        let s = tex.size_vec2();
                        let avail_w = ui.available_width().max(240.0);
                        let sc = (avail_w / s.x).min(2.0);
                        let disp = s * sc;
                        let (rect, _) = ui.allocate_exact_size(disp, egui::Sense::hover());
                        let p = ui.painter_at(rect);
                        Self::draw_checkerboard(&p, rect, 12.0, self.light_checker);
                        p.image(tex.id(), rect, egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)), egui::Color32::WHITE);
                        let cols = self.sheet_cols.max(1);
                        let rows = self.sheet_rows.max(1);
                        let cw = rect.width() / cols as f32;
                        let ch = rect.height() / rows as f32;
                        let st = egui::Stroke::new(1.0, egui::Color32::from_rgba_unmultiplied(80, 190, 255, 190));
                        for i in 1..cols {
                            let x = rect.min.x + i as f32 * cw;
                            p.line_segment([egui::pos2(x, rect.min.y), egui::pos2(x, rect.max.y)], st);
                        }
                        for j in 1..rows {
                            let y = rect.min.y + j as f32 * ch;
                            p.line_segment([egui::pos2(rect.min.x, y), egui::pos2(rect.max.x, y)], st);
                        }
                    });
                } else {
                    ui.label("Click 'Choose PNG Sprite Sheet...' on the left to load any sprite strip or grid.");
                }
            }
            StudioTab::Export => {
                    ui.label(egui::RichText::new("Pet Metadata").strong());
                    ui.label("Display Name:");
                    if ui.text_edit_singleline(&mut self.project.display_name).changed()
                        && !self.pack_id_locked
                    {
                        self.project.pack_id = sanitize_pack_id(&self.project.display_name);
                    }
                    ui.label("Pack Folder ID:");
                    if ui.text_edit_singleline(&mut self.project.pack_id).changed() {
                        self.pack_id_locked = true;
                    }
                    ui.label("Author Credit:");
                    ui.text_edit_singleline(&mut self.project.author);

                    ui.separator();
                    ui.label(egui::RichText::new("Desktop Physics").strong());
                    ui.add(egui::Slider::new(&mut self.project.walk_step_px, 2..=18).text("Walk Step px"));
                    ui.add(egui::Slider::new(&mut self.project.frame_interval_ms, 60..=250).text("Frame ms"));

                    ui.separator();
                    ui.checkbox(&mut self.show_contact_sheet, "Inspect Full Sprite Sheet");
                    ui.add_space(6.0);
                    if ui.button("Export & Use on Desktop").clicked() {
                        self.request_export(true);
                    }
                    ui.add_space(4.0);
                    if ui.button("Export to Local Library").clicked() {
                        self.request_export(false);
                    }
                    ui.add_space(4.0);
                    if ui.button("Save Shareable .xml File...").clicked() {
                        self.export_standalone_xml_dialog();
                    }

                    if let Some(existing) = self.confirm_overwrite_path.clone() {
                        ui.add_space(8.0);
                        egui::Frame::default()
                            .fill(egui::Color32::from_rgb(76, 44, 34))
                            .inner_margin(8.0)
                            .corner_radius(6.0)
                            .show(ui, |ui| {
                                ui.colored_label(
                                    egui::Color32::from_rgb(255, 200, 130),
                                    format!("Pack '{}' already exists!", self.project.pack_id),
                                );
                                ui.horizontal(|ui| {
                                    if ui.button("Overwrite").clicked() {
                                        let act = self.pending_activate;
                                        self.execute_export(&existing, act);
                                    }
                                    if ui.button("Cancel").clicked() {
                                        self.confirm_overwrite_path = None;
                                    }
                                });
                            });
                    }
                }
            });
        });

        // CENTER STAGE
        egui::CentralPanel::default().show(ui, |ui| match self.tab {
            StudioTab::ImportSheet => {
                ui.heading("Sprite Sheet Grid Preview");
                if let Some(tex) = &self.sheet_import_tex {
                    egui::ScrollArea::both().show(ui, |ui| {
                        let s = tex.size_vec2();
                        let avail_w = ui.available_width().max(240.0);
                        let sc = (avail_w / s.x).min(2.0);
                        let disp = s * sc;
                        let (rect, _) = ui.allocate_exact_size(disp, egui::Sense::hover());
                        let p = ui.painter_at(rect);
                        Self::draw_checkerboard(&p, rect, 12.0, self.light_checker);
                        p.image(tex.id(), rect, egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)), egui::Color32::WHITE);
                        let cols = self.sheet_cols.max(1);
                        let rows = self.sheet_rows.max(1);
                        let cw = rect.width() / cols as f32;
                        let ch = rect.height() / rows as f32;
                        let st = egui::Stroke::new(1.0, egui::Color32::from_rgba_unmultiplied(80, 190, 255, 190));
                        for i in 1..cols {
                            let x = rect.min.x + i as f32 * cw;
                            p.line_segment([egui::pos2(x, rect.min.y), egui::pos2(x, rect.max.y)], st);
                        }
                        for j in 1..rows {
                            let y = rect.min.y + j as f32 * ch;
                            p.line_segment([egui::pos2(rect.min.x, y), egui::pos2(rect.max.x, y)], st);
                        }
                    });
                } else {
                    ui.label("Click 'Choose PNG Sprite Sheet...' on the left to load any sprite strip or grid.");
                }
            }
            StudioTab::Export => {
                ui.vertical_centered(|ui| {
                    ui.heading("Animation Tester & Exporter");
                    self.render_clip_picker(ui);
                    ui.horizontal(|ui| {
                        ui.checkbox(&mut self.preview_playing, "Play");
                        ui.checkbox(&mut self.preview_desktop_scale, "128px Desktop Size");
                    });
                    ui.add_space(8.0);
                    if self.show_contact_sheet {
                        self.render_contact_sheet(ui);
                    } else {
                        let side = ui.available_width().min(ui.available_height()).clamp(240.0, 440.0);
                        self.render_preview_box(ui, side, true);
                    }
                });
            }

            StudioTab::Animate => {
                ui.heading("Custom Keyframe Timeline");
                if let Some(anim) = self.project.custom_animations.get_mut(self.selected_custom_anim) {
                    ui.horizontal_wrapped(|ui| {
                        for k in 0..anim.keyframes.len() {
                            if ui
                                .selectable_label(self.selected_keyframe == k, format!("Frame {}", k + 1))
                                .clicked()
                            {
                                self.selected_keyframe = k;
                            }
                        }
                        if ui.button("+ Add Frame").clicked() {
                            let clone_kf = anim
                                .keyframes
                                .get(self.selected_keyframe)
                                .cloned()
                                .unwrap_or_default();
                            anim.keyframes.push(clone_kf);
                            self.selected_keyframe = anim.keyframes.len() - 1;
                            self.sheet_dirty = true;
                        }
                        if anim.keyframes.len() > 1 && ui.button("- Remove Frame").clicked() {
                            let idx = self.selected_keyframe.min(anim.keyframes.len() - 1);
                            anim.keyframes.remove(idx);
                            self.selected_keyframe = idx.saturating_sub(1);
                            self.sheet_dirty = true;
                        }
                    });
                    ui.separator();

                    let kf_idx = self.selected_keyframe.min(anim.keyframes.len().saturating_sub(1));
                    if let Some(kf) = anim.keyframes.get_mut(kf_idx) {
                        ui.label(egui::RichText::new(format!("Editing Keyframe #{}", kf_idx + 1)).strong());
                        ui.add(egui::Slider::new(&mut kf.limb_swing, -1.0..=1.0).text("Limb Swing"));
                        ui.add(egui::Slider::new(&mut kf.wing_flap, -1.5..=1.5).text("Wing Flap"));
                        ui.add(egui::Slider::new(&mut kf.tail_wag, -1.5..=1.5).text("Tail Wag"));
                        ui.add(egui::Slider::new(&mut kf.body_dy, -22.0..=16.0).text("Body Bounce Y"));
                        ui.add(egui::Slider::new(&mut kf.body_tilt_deg, -25.0..=25.0).text("Body Tilt deg"));
                        ui.add(egui::Slider::new(&mut kf.torso_scale_y, 0.75..=1.25).text("Squash / Stretch"));
                        ui.add(egui::Slider::new(&mut kf.wave_phase, 0.0..=6.28).text("Slither / Swim Wave"));
                        ui.horizontal_wrapped(|ui| {
                            ui.label("Face:");
                            for &slot in FaceSlot::all() {
                                ui.selectable_value(&mut kf.face, slot, slot.label());
                            }
                        });
                        self.sheet_dirty = true;
                    }
                } else {
                    ui.label("Click + Hop, + Dance, or + Blank on the left to build a custom animation.");
                }
            }

            StudioTab::Draw | StudioTab::Rig => {
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new(format!("Canvas: {}", self.active_layer_title())).strong());
                    ui.label(
                        egui::RichText::new("  <- Pet faces LEFT")
                            .size(11.0)
                            .color(egui::Color32::GRAY),
                    );
                    if self.tab == StudioTab::Rig {
                        let hint = if self.edit_layer == EditLayer::Body {
                            "Drag any socket pin (Near/Far) on the Body"
                        } else {
                            "Drag the green Pivot pin to set rotation hinge"
                        };
                        ui.separator();
                        ui.colored_label(egui::Color32::from_rgb(130, 200, 255), hint);
                    }
                });

                egui::ScrollArea::both().show(ui, |ui| {
                    let wheel_y = ui.ctx().input(|i| if i.modifiers.ctrl || i.modifiers.command { i.smooth_scroll_delta.y } else { 0.0 });
                    if wheel_y.abs() > 0.1 {
                        self.zoom = (self.zoom + wheel_y * 0.01).clamp(1.5, 8.0);
                    }
                    let board_px = (BOARD_SIZE as f32 * self.zoom).max(256.0);
                    let (rect, response) = ui.allocate_exact_size(
                        egui::vec2(board_px, board_px),
                        egui::Sense::click_and_drag(),
                    );
                    let painter = ui.painter_at(rect);
                    Self::draw_checkerboard(&painter, rect, (self.zoom * 4.0).max(8.0), self.light_checker);

                    let uv = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
                    if self.show_onion_skin && self.edit_layer != EditLayer::Body {
                        if let Some(ghost) = &self.tex_body_ghost {
                            painter.image(
                                ghost.id(),
                                rect,
                                uv,
                                egui::Color32::from_rgba_unmultiplied(255, 255, 255, 90),
                            );
                        }
                    }
                    if let Some(active) = &self.tex_active {
                        painter.image(active.id(), rect, uv, egui::Color32::WHITE);
                    }

                    let scale = board_px / BOARD_SIZE as f32;
                    if self.mirror_draw_x && self.tab == StudioTab::Draw {
                        let mx = rect.center().x;
                        painter.line_segment(
                            [egui::pos2(mx, rect.min.y), egui::pos2(mx, rect.max.y)],
                            egui::Stroke::new(1.0, egui::Color32::from_rgba_unmultiplied(90, 180, 255, 160)),
                        );
                    }
                    if self.show_grid && scale >= 4.0 {
                        let gs = egui::Stroke::new(0.5, egui::Color32::from_rgba_unmultiplied(255, 255, 255, 22));
                        for i in 1..BOARD_SIZE {
                            let p = i as f32 * scale;
                            painter.line_segment([egui::pos2(rect.min.x + p, rect.min.y), egui::pos2(rect.min.x + p, rect.max.y)], gs);
                            painter.line_segment([egui::pos2(rect.min.x, rect.min.y + p), egui::pos2(rect.max.x, rect.min.y + p)], gs);
                        }
                    }

                    // Shape drag preview + brush ring
                    if self.tab == StudioTab::Draw {
                        if let (Some(s), Some(e)) = (self.shape_start, self.shape_current) {
                            let p0 = rect.min + egui::vec2((s.0 as f32 + 0.5) * scale, (s.1 as f32 + 0.5) * scale);
                            let p1 = rect.min + egui::vec2((e.0 as f32 + 0.5) * scale, (e.1 as f32 + 0.5) * scale);
                            if self.tool == Tool::Line {
                                painter.line_segment([p0, p1], egui::Stroke::new(2.0, self.current_color));
                            } else {
                                painter.rect_stroke(egui::Rect::from_two_pos(p0, p1), 0.0, egui::Stroke::new(1.5, self.current_color), egui::StrokeKind::Middle);
                            }
                        }
                        if let Some(hover) = response.hover_pos() {
                            if matches!(self.tool, Tool::Brush | Tool::Eraser | Tool::Shade) {
                                let rad = (self.brush_size as f32 * 0.5 * scale).max(2.0);
                                painter.circle_stroke(hover, rad, egui::Stroke::new(1.0, egui::Color32::WHITE));
                            }
                        }
                    }

                    // Rigging pins overlay
                    let mut visible_pins: Vec<(RigPin, (usize, usize), egui::Color32, String)> = Vec::new();
                    if self.tab == StudioTab::Rig {
                        match self.edit_layer {
                            EditLayer::Body => {
                                for (i, p) in self.project.parts.iter().enumerate() {
                                    let rgb = p.role.pin_rgb();
                                    let col = egui::Color32::from_rgb(rgb[0], rgb[1], rgb[2]);
                                    visible_pins.push((RigPin::NearSocket(i), p.socket_near, col, p.role.short_tag().to_string()));
                                    if p.paired {
                                        let far_col = egui::Color32::from_rgb(rgb[0] / 2 + 60, rgb[1] / 2 + 60, rgb[2] / 2 + 60);
                                        visible_pins.push((RigPin::FarSocket(i), p.socket_far, far_col, format!("{}f", p.role.short_tag())));
                                    }
                                }
                            }
                            EditLayer::Part(i) => {
                                if let Some(p) = self.project.parts.get(i) {
                                    visible_pins.push((
                                        RigPin::Pivot(i),
                                        p.pivot,
                                        egui::Color32::from_rgb(80, 232, 136),
                                        "PIVOT".to_string(),
                                    ));
                                }
                            }
                            EditLayer::Face(_) => {}
                        }

                        for &(_, (ax, ay), col, ref tag) in &visible_pins {
                            let c = rect.min + egui::vec2((ax as f32 + 0.5) * scale, (ay as f32 + 0.5) * scale);
                            painter.circle_filled(c, 7.0, col);
                            painter.circle_stroke(c, 7.0, egui::Stroke::new(2.0, egui::Color32::WHITE));
                            painter.text(
                                c + egui::vec2(10.0, 0.0),
                                egui::Align2::LEFT_CENTER,
                                tag,
                                egui::FontId::proportional(11.0),
                                egui::Color32::WHITE,
                            );
                        }
                    }

                    // Pointer interaction
                    if let Some(pos) = response.interact_pointer_pos() {
                        let local = pos - rect.min;
                        let lx = (local.x / scale).floor().clamp(0.0, (BOARD_SIZE - 1) as f32) as usize;
                        let ly = (local.y / scale).floor().clamp(0.0, (BOARD_SIZE - 1) as f32) as usize;

                        if self.tab == StudioTab::Rig {
                            if response.drag_started() {
                                let mut best: Option<RigPin> = None;
                                let mut best_d = 22.0_f32;
                                for &(pin, (ax, ay), _, _) in &visible_pins {
                                    let c = rect.min + egui::vec2((ax as f32 + 0.5) * scale, (ay as f32 + 0.5) * scale);
                                    let d = c.distance(pos);
                                    if d < best_d {
                                        best_d = d;
                                        best = Some(pin);
                                    }
                                }
                                self.dragging_pin = best;
                            }
                            let active_pin = self.dragging_pin.or_else(|| {
                                if response.clicked() {
                                    if let EditLayer::Part(i) = self.edit_layer {
                                        Some(RigPin::Pivot(i))
                                    } else {
                                        self.selected_part_idx.map(RigPin::NearSocket)
                                    }
                                } else {
                                    None
                                }
                            });
                            if let Some(pin) = active_pin {
                                match pin {
                                    RigPin::NearSocket(i) => {
                                        if let Some(p) = self.project.parts.get_mut(i) {
                                            p.socket_near = (lx, ly);
                                            self.selected_part_idx = Some(i);
                                        }
                                    }
                                    RigPin::FarSocket(i) => {
                                        if let Some(p) = self.project.parts.get_mut(i) {
                                            p.socket_far = (lx, ly);
                                            self.selected_part_idx = Some(i);
                                        }
                                    }
                                    RigPin::Pivot(i) => {
                                        if let Some(p) = self.project.parts.get_mut(i) {
                                            p.pivot = (lx, ly);
                                        }
                                    }
                                }
                                self.sheet_dirty = true;
                            }
                            if response.drag_stopped() {
                                self.dragging_pin = None;
                            }
                        } else if self.tab == StudioTab::Draw {
                            let rgba = c32_to_rgba(self.current_color);
                            match self.tool {
                                Tool::Brush | Tool::Eraser | Tool::Shade => {
                                    let col = if self.tool == Tool::Eraser { [0, 0, 0, 0] } else { rgba };
                                    let shade = if self.tool == Tool::Shade { Some(self.shade_lighten) } else { None };
                                    if response.drag_started() || response.clicked() {
                                        self.push_undo();
                                        self.last_draw_pos = Some((lx, ly));
                                        self.apply_brush(lx, ly, col, shade);
                                    }
                                    if response.dragged() {
                                        if let Some(last) = self.last_draw_pos {
                                            self.paint_line(last, (lx, ly), col, shade);
                                        }
                                        self.last_draw_pos = Some((lx, ly));
                                    }
                                    if response.drag_stopped() {
                                        self.last_draw_pos = None;
                                    }
                                }
                                Tool::Fill => {
                                    if response.clicked() {
                                        self.push_undo();
                                        self.flood_fill(lx, ly, rgba);
                                    }
                                }
                                Tool::Recolor => {
                                    if response.clicked() {
                                        self.push_undo();
                                        self.recolor_matching(lx, ly, rgba);
                                    }
                                }
                                Tool::Eyedropper => {
                                    if response.clicked() || response.dragged() {
                                        let picked = self.active_buf()[ly * BOARD_SIZE + lx];
                                        if picked[3] > 0 {
                                            self.current_color = rgba_to_c32(picked);
                                            self.tool = Tool::Brush;
                                        }
                                    }
                                }
                                Tool::Line | Tool::Rect | Tool::Circle => {
                                    if response.drag_started() {
                                        self.shape_start = Some((lx, ly));
                                        self.shape_current = Some((lx, ly));
                                    }
                                    if response.dragged() {
                                        self.shape_current = Some((lx, ly));
                                    }
                                    if response.drag_stopped() {
                                        if let (Some(s), Some(e)) = (self.shape_start, self.shape_current) {
                                            self.push_undo();
                                            match self.tool {
                                                Tool::Line => self.paint_line(s, e, rgba, None),
                                                Tool::Rect => self.paint_rect(s, e, rgba),
                                                Tool::Circle => self.paint_circle(s, e, rgba),
                                                _ => {}
                                            }
                                        }
                                        self.shape_start = None;
                                        self.shape_current = None;
                                    }
                                }
                            }
                        }
                    }
                });
            }
        });
    }
}

fn apply_studio_theme(ctx: &egui::Context) {
    let mut style = (*ctx.style()).clone();
    style.spacing.item_spacing = egui::vec2(8.0, 6.0);
    style.spacing.button_padding = egui::vec2(9.0, 5.0);
    let mut visuals = egui::Visuals::dark();
    visuals.window_fill = egui::Color32::from_rgb(20, 23, 30);
    visuals.panel_fill = egui::Color32::from_rgb(25, 29, 38);
    visuals.faint_bg_color = egui::Color32::from_rgb(33, 38, 50);
    visuals.selection.bg_fill = egui::Color32::from_rgb(59, 130, 246);
    visuals.selection.stroke = egui::Stroke::new(1.0, egui::Color32::WHITE);
    visuals.widgets.inactive.bg_fill = egui::Color32::from_rgb(38, 44, 58);
    visuals.widgets.inactive.corner_radius = egui::CornerRadius::same(6);
    visuals.widgets.hovered.bg_fill = egui::Color32::from_rgb(50, 58, 76);
    visuals.widgets.hovered.corner_radius = egui::CornerRadius::same(6);
    visuals.widgets.active.bg_fill = egui::Color32::from_rgb(59, 130, 246);
    visuals.widgets.active.corner_radius = egui::CornerRadius::same(6);
    style.visuals = visuals;
    ctx.set_style(style);
}

fn main() -> eframe::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    if let Some(res) = try_run_cli(&args) {
        if let Err(e) = res {
            eprintln!("diypets-creator error: {e}");
            std::process::exit(1);
        }
        return Ok(());
    }

    let mut viewport = egui::ViewportBuilder::default()
        .with_inner_size([1240.0, 770.0])
        .with_min_inner_size([920.0, 600.0])
        .with_title("DIYPets Studio V3");

    if let Some(icon) = default_app_icon(64) {
        viewport = viewport.with_icon(Arc::new(egui::IconData {
            rgba: icon.into_raw(),
            width: 64,
            height: 64,
        }));
    }

    let options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };
    eframe::run_native(
        "DIYPets Studio V3",
        options,
        Box::new(|cc| {
            apply_studio_theme(&cc.egui_ctx);
            Ok(Box::new(StudioApp::default()))
        }),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cli_from_svg_builds_playable_bird_pack() {
        let dir = tempfile::tempdir().unwrap();
        let svg_path = dir.path().join("robin.svg");
        let out_dir = dir.path().join("robin_pack");
        std::fs::write(
            &svg_path,
            r##"<svg viewBox="0 0 128 128"><g id="body"><ellipse cx="64" cy="64" rx="22" ry="15" fill="#e25848"/></g><g id="wing"><ellipse cx="70" cy="64" rx="12" ry="7" fill="#a83428"/></g></svg>"##,
        )
        .unwrap();

        let args = vec![
            "diypets-creator".to_string(),
            "--from-svg".to_string(),
            svg_path.to_string_lossy().into_owned(),
            "--name".to_string(),
            "Robin".to_string(),
            "--archetype".to_string(),
            "bird".to_string(),
            "--author".to_string(),
            "SM".to_string(),
            "--out".to_string(),
            out_dir.to_string_lossy().into_owned(),
        ];
        try_run_cli(&args).unwrap().unwrap();
        assert!(out_dir.join("deskpet.xml").is_file());
        assert!(out_dir.join("icon.png").is_file());
    }
}