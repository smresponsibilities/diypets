//! Pure-Rust 2D SVG Importer & Automatic Part Splitter (uses `roxmltree` + `tiny-skia`).
//!
//! Supports semantic `<g id="...">` tags (`body`, `front_leg`/`arm`, `back_leg`/`leg`,
//! `wing`, `tail`, `ear`, `extra`, `face`, `face_blink`, `face_happy`, `face_sleep`, `face_surprised`)
//! AND falls back to geometric auto-splitting for untagged SVGs so any vector drawing or
//! AI-generated SVG becomes a rigged, animated desktop pet in one step.

use crate::studio_model::{
    Archetype, BOARD_LEN, BOARD_SIZE, FaceSlot, PartRole, RgbaPixel, RigPart, StudioProject,
    buf_bbox, sanitize_pack_id,
};
use tiny_skia::{
    Color, FillRule, LineCap, LineJoin, Paint, Path, PathBuilder, Pixmap, Rect, Stroke, Transform,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SemanticTarget {
    Body,
    Part(PartRole),
    Face(FaceSlot),
}

#[derive(Clone, Copy)]
struct StyleState {
    fill: Option<[u8; 4]>,
    stroke: Option<[u8; 4]>,
    stroke_width: f32,
    opacity: f32,
}

impl Default for StyleState {
    fn default() -> Self {
        Self {
            fill: Some([40, 40, 46, 255]),
            stroke: None,
            stroke_width: 1.0,
            opacity: 1.0,
        }
    }
}

struct ParsedShape {
    path: Path,
    style: StyleState,
    transform: Transform,
    target: Option<SemanticTarget>,
    part_name: Option<String>,
}

fn classify_id(raw: &str) -> Option<SemanticTarget> {
    let s = raw.trim().to_lowercase();
    if s.is_empty() {
        return None;
    }
    if s.contains("blink") {
        return Some(SemanticTarget::Face(FaceSlot::Blink));
    }
    if s.contains("happy") || s.contains("smile") {
        return Some(SemanticTarget::Face(FaceSlot::Happy));
    }
    if s.contains("sleep") {
        return Some(SemanticTarget::Face(FaceSlot::Sleep));
    }
    if s.contains("surpris") || s.contains("shock") {
        return Some(SemanticTarget::Face(FaceSlot::Surprised));
    }
    if s.contains("face") || s.contains("eye") || s.contains("expression") {
        return Some(SemanticTarget::Face(FaceSlot::Neutral));
    }
    if s.contains("wing") || s.contains("pectoral") || s.contains("side_fin") {
        return Some(SemanticTarget::Part(PartRole::Wing));
    }
    if s.contains("tail") {
        return Some(SemanticTarget::Part(PartRole::Tail));
    }
    if s.contains("ear") || s.contains("horn") || s.contains("crest") || s.contains("antenna") {
        return Some(SemanticTarget::Part(PartRole::Ear));
    }
    if s.contains("front_leg")
        || s.contains("front_limb")
        || s.contains("front_paw")
        || s.contains("arm")
    {
        return Some(SemanticTarget::Part(PartRole::FrontLimb));
    }
    if s.contains("back_leg")
        || s.contains("back_limb")
        || s.contains("back_paw")
        || s.contains("hind")
        || s.contains("leg")
        || s.contains("foot")
    {
        return Some(SemanticTarget::Part(PartRole::BackLimb));
    }
    if s.contains("hat") || s.contains("prop") || s.contains("extra") || s.contains("accessory") {
        return Some(SemanticTarget::Part(PartRole::Extra));
    }
    if s.contains("body") || s.contains("torso") || s.contains("head") {
        return Some(SemanticTarget::Body);
    }
    None
}

fn parse_color(val: &str) -> Option<Option<[u8; 4]>> {
    let s = val.trim().to_lowercase();
    if s.is_empty() {
        return None;
    }
    if s == "none" || s == "transparent" {
        return Some(None);
    }
    if let Some(hex) = s.strip_prefix('#') {
        if hex.len() == 3 {
            let r = u8::from_str_radix(&hex[0..1], 16).ok()? * 17;
            let g = u8::from_str_radix(&hex[1..2], 16).ok()? * 17;
            let b = u8::from_str_radix(&hex[2..3], 16).ok()? * 17;
            return Some(Some([r, g, b, 255]));
        } else if hex.len() == 6 {
            let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
            let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
            let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
            return Some(Some([r, g, b, 255]));
        } else if hex.len() == 8 {
            let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
            let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
            let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
            let a = u8::from_str_radix(&hex[6..8], 16).ok()?;
            return Some(Some([r, g, b, a]));
        }
    }
    if let Some(inner) = s.strip_prefix("rgb(").and_then(|x| x.strip_suffix(')')) {
        let nums: Vec<u8> = inner
            .split(',')
            .filter_map(|p| p.trim().parse::<f32>().ok())
            .map(|n| n.clamp(0.0, 255.0) as u8)
            .collect();
        if nums.len() >= 3 {
            return Some(Some([nums[0], nums[1], nums[2], 255]));
        }
    }
    let named = match s.as_str() {
        "black" => [0, 0, 0, 255],
        "white" => [255, 255, 255, 255],
        "red" => [225, 64, 64, 255],
        "green" => [64, 178, 96, 255],
        "blue" => [68, 138, 232, 255],
        "yellow" => [246, 214, 72, 255],
        "orange" => [244, 150, 62, 255],
        "purple" | "violet" => [150, 96, 214, 255],
        "pink" => [246, 146, 178, 255],
        "brown" => [146, 92, 54, 255],
        "gray" | "grey" => [136, 142, 152, 255],
        "cyan" | "aqua" => [78, 200, 218, 255],
        "gold" => [242, 198, 64, 255],
        "teal" => [52, 152, 148, 255],
        "coral" => [246, 124, 88, 255],
        "beige" | "cream" => [245, 232, 204, 255],
        "tan" => [210, 176, 134, 255],
        "lime" => [132, 214, 84, 255],
        "navy" => [36, 52, 112, 255],
        "silver" => [194, 200, 210, 255],
        _ => return None,
    };
    Some(Some(named))
}

fn apply_style(node: roxmltree::Node<'_, '_>, parent: StyleState) -> StyleState {
    let mut s = parent;
    let mut apply_kv = |k: &str, v: &str| match k.trim() {
        "fill" => {
            if let Some(c) = parse_color(v) {
                s.fill = c;
            }
        }
        "stroke" => {
            if let Some(c) = parse_color(v) {
                s.stroke = c;
            }
        }
        "stroke-width" => {
            if let Ok(w) = v.trim().trim_end_matches("px").parse::<f32>() {
                s.stroke_width = w.max(0.0);
            }
        }
        "opacity" | "fill-opacity" => {
            if let Ok(op) = v.trim().parse::<f32>() {
                s.opacity *= op.clamp(0.0, 1.0);
            }
        }
        _ => {}
    };

    for attr in ["fill", "stroke", "stroke-width", "opacity", "fill-opacity"] {
        if let Some(v) = node.attribute(attr) {
            apply_kv(attr, v);
        }
    }
    if let Some(style_str) = node.attribute("style") {
        for decl in style_str.split(';') {
            if let Some((k, v)) = decl.split_once(':') {
                apply_kv(k, v);
            }
        }
    }
    s
}

fn parse_transform(attr: Option<&str>) -> Transform {
    let Some(mut s) = attr else {
        return Transform::identity();
    };
    let mut total = Transform::identity();
    while let Some(open) = s.find('(') {
        let name = s[..open].trim().trim_start_matches(',').trim();
        let rest = &s[open + 1..];
        let Some(close) = rest.find(')') else {
            break;
        };
        let args: Vec<f32> = rest[..close]
            .split(|c: char| c == ',' || c.is_whitespace())
            .filter(|t| !t.is_empty())
            .filter_map(|t| t.parse::<f32>().ok())
            .collect();
        let step = match name {
            "translate" if !args.is_empty() => {
                Transform::from_translate(args[0], *args.get(1).unwrap_or(&0.0))
            }
            "scale" if !args.is_empty() => {
                Transform::from_scale(args[0], *args.get(1).unwrap_or(&args[0]))
            }
            "rotate" if !args.is_empty() => {
                if args.len() >= 3 {
                    Transform::from_rotate_at(args[0], args[1], args[2])
                } else {
                    Transform::from_rotate(args[0])
                }
            }
            _ => Transform::identity(),
        };
        total = total.pre_concat(step);
        s = &rest[close + 1..];
    }
    total
}

fn build_ellipse_path(cx: f32, cy: f32, rx: f32, ry: f32) -> Option<Path> {
    if rx <= 0.0 || ry <= 0.0 {
        return None;
    }
    let k = 0.552_284_8;
    let mut pb = PathBuilder::new();
    pb.move_to(cx + rx, cy);
    pb.cubic_to(cx + rx, cy + ry * k, cx + rx * k, cy + ry, cx, cy + ry);
    pb.cubic_to(cx - rx * k, cy + ry, cx - rx, cy + ry * k, cx - rx, cy);
    pb.cubic_to(cx - rx, cy - ry * k, cx - rx * k, cy - ry, cx, cy - ry);
    pb.cubic_to(cx + rx * k, cy - ry, cx + rx, cy - ry * k, cx + rx, cy);
    pb.close();
    pb.finish()
}

fn parse_points_list(pts: &str) -> Vec<(f32, f32)> {
    let nums: Vec<f32> = pts
        .split(|c: char| c == ',' || c.is_whitespace())
        .filter(|t| !t.is_empty())
        .filter_map(|t| t.parse::<f32>().ok())
        .collect();
    nums.as_chunks::<2>().0.iter().map(|c| (c[0], c[1])).collect()
}

fn parse_svg_path_d(d: &str) -> Option<Path> {
    let bytes = d.as_bytes();
    let mut i = 0usize;
    let mut pb = PathBuilder::new();
    let mut cmd = b'M';
    let mut cx = 0.0_f32;
    let mut cy = 0.0_f32;
    let mut sx = 0.0_f32;
    let mut sy = 0.0_f32;
    let mut last_ctrl: Option<(f32, f32)> = None;

    let skip_sep = |bytes: &[u8], i: &mut usize| {
        while *i < bytes.len() && (bytes[*i].is_ascii_whitespace() || bytes[*i] == b',') {
            *i += 1;
        }
    };

    let read_num = |bytes: &[u8], i: &mut usize| -> Option<f32> {
        skip_sep(bytes, i);
        let start = *i;
        if *i < bytes.len() && (bytes[*i] == b'+' || bytes[*i] == b'-') {
            *i += 1;
        }
        let mut saw_dot = false;
        while *i < bytes.len() {
            let b = bytes[*i];
            if b.is_ascii_digit() {
                *i += 1;
            } else if b == b'.' && !saw_dot {
                saw_dot = true;
                *i += 1;
            } else if (b == b'e' || b == b'E') && *i > start {
                *i += 1;
                if *i < bytes.len() && (bytes[*i] == b'+' || bytes[*i] == b'-') {
                    *i += 1;
                }
            } else {
                break;
            }
        }
        if *i == start {
            return None;
        }
        std::str::from_utf8(&bytes[start..*i]).ok()?.parse::<f32>().ok()
    };

    while i < bytes.len() {
        skip_sep(bytes, &mut i);
        if i >= bytes.len() {
            break;
        }
        if bytes[i].is_ascii_alphabetic() {
            cmd = bytes[i];
            i += 1;
        }
        match cmd {
            b'M' | b'm' => {
                let Some(x) = read_num(bytes, &mut i) else { break };
                let Some(y) = read_num(bytes, &mut i) else { break };
                if cmd == b'm' {
                    cx += x;
                    cy += y;
                } else {
                    cx = x;
                    cy = y;
                }
                sx = cx;
                sy = cy;
                pb.move_to(cx, cy);
                last_ctrl = None;
                cmd = if cmd == b'm' { b'l' } else { b'L' };
            }
            b'L' | b'l' => {
                let Some(x) = read_num(bytes, &mut i) else { break };
                let Some(y) = read_num(bytes, &mut i) else { break };
                if cmd == b'l' {
                    cx += x;
                    cy += y;
                } else {
                    cx = x;
                    cy = y;
                }
                pb.line_to(cx, cy);
                last_ctrl = None;
            }
            b'H' | b'h' => {
                let Some(x) = read_num(bytes, &mut i) else { break };
                cx = if cmd == b'h' { cx + x } else { x };
                pb.line_to(cx, cy);
                last_ctrl = None;
            }
            b'V' | b'v' => {
                let Some(y) = read_num(bytes, &mut i) else { break };
                cy = if cmd == b'v' { cy + y } else { y };
                pb.line_to(cx, cy);
                last_ctrl = None;
            }
            b'C' | b'c' => {
                let (Some(x1), Some(y1), Some(x2), Some(y2), Some(x), Some(y)) = (
                    read_num(bytes, &mut i),
                    read_num(bytes, &mut i),
                    read_num(bytes, &mut i),
                    read_num(bytes, &mut i),
                    read_num(bytes, &mut i),
                    read_num(bytes, &mut i),
                ) else {
                    break;
                };
                let (ax1, ay1, ax2, ay2, ax, ay) = if cmd == b'c' {
                    (cx + x1, cy + y1, cx + x2, cy + y2, cx + x, cy + y)
                } else {
                    (x1, y1, x2, y2, x, y)
                };
                pb.cubic_to(ax1, ay1, ax2, ay2, ax, ay);
                last_ctrl = Some((ax2, ay2));
                cx = ax;
                cy = ay;
            }
            b'S' | b's' => {
                let (Some(x2), Some(y2), Some(x), Some(y)) = (
                    read_num(bytes, &mut i),
                    read_num(bytes, &mut i),
                    read_num(bytes, &mut i),
                    read_num(bytes, &mut i),
                ) else {
                    break;
                };
                let (ax1, ay1) = match last_ctrl {
                    Some((lx, ly)) => (2.0 * cx - lx, 2.0 * cy - ly),
                    None => (cx, cy),
                };
                let (ax2, ay2, ax, ay) = if cmd == b's' {
                    (cx + x2, cy + y2, cx + x, cy + y)
                } else {
                    (x2, y2, x, y)
                };
                pb.cubic_to(ax1, ay1, ax2, ay2, ax, ay);
                last_ctrl = Some((ax2, ay2));
                cx = ax;
                cy = ay;
            }
            b'Q' | b'q' => {
                let (Some(x1), Some(y1), Some(x), Some(y)) = (
                    read_num(bytes, &mut i),
                    read_num(bytes, &mut i),
                    read_num(bytes, &mut i),
                    read_num(bytes, &mut i),
                ) else {
                    break;
                };
                let (ax1, ay1, ax, ay) = if cmd == b'q' {
                    (cx + x1, cy + y1, cx + x, cy + y)
                } else {
                    (x1, y1, x, y)
                };
                pb.quad_to(ax1, ay1, ax, ay);
                last_ctrl = Some((ax1, ay1));
                cx = ax;
                cy = ay;
            }
            b'A' | b'a' => {
                // Consume 7 arc params and connect cleanly to endpoint
                let (_rx, _ry, _rot, _laf, _sf, Some(x), Some(y)) = (
                    read_num(bytes, &mut i),
                    read_num(bytes, &mut i),
                    read_num(bytes, &mut i),
                    read_num(bytes, &mut i),
                    read_num(bytes, &mut i),
                    read_num(bytes, &mut i),
                    read_num(bytes, &mut i),
                ) else {
                    break;
                };
                if cmd == b'a' {
                    cx += x;
                    cy += y;
                } else {
                    cx = x;
                    cy = y;
                }
                pb.line_to(cx, cy);
                last_ctrl = None;
            }
            b'Z' | b'z' => {
                pb.close();
                cx = sx;
                cy = sy;
                last_ctrl = None;
            }
            _ => {
                let _ = read_num(bytes, &mut i);
            }
        }
    }
    pb.finish()
}

fn shape_from_node(node: roxmltree::Node<'_, '_>) -> Option<Path> {
    let f = |attr: &str, default: f32| -> f32 {
        node.attribute(attr)
            .and_then(|v| v.trim().trim_end_matches("px").parse::<f32>().ok())
            .unwrap_or(default)
    };
    match node.tag_name().name() {
        "circle" => PathBuilder::from_circle(f("cx", 0.0), f("cy", 0.0), f("r", 0.0)),
        "ellipse" => build_ellipse_path(f("cx", 0.0), f("cy", 0.0), f("rx", 0.0), f("ry", 0.0)),
        "rect" => {
            let r = Rect::from_xywh(f("x", 0.0), f("y", 0.0), f("width", 0.0), f("height", 0.0))?;
            Some(PathBuilder::from_rect(r))
        }
        "line" => {
            let mut pb = PathBuilder::new();
            pb.move_to(f("x1", 0.0), f("y1", 0.0));
            pb.line_to(f("x2", 0.0), f("y2", 0.0));
            pb.finish()
        }
        "polygon" | "polyline" => {
            let pts = parse_points_list(node.attribute("points")?);
            if pts.len() < 2 {
                return None;
            }
            let mut pb = PathBuilder::new();
            pb.move_to(pts[0].0, pts[0].1);
            for &(x, y) in &pts[1..] {
                pb.line_to(x, y);
            }
            if node.tag_name().name() == "polygon" {
                pb.close();
            }
            pb.finish()
        }
        "path" => parse_svg_path_d(node.attribute("d")?),
        _ => None,
    }
}

fn collect_shapes(
    node: roxmltree::Node<'_, '_>,
    parent_style: StyleState,
    parent_xform: Transform,
    inherited_target: Option<SemanticTarget>,
    inherited_name: Option<String>,
    out: &mut Vec<ParsedShape>,
) {
    if !node.is_element() {
        return;
    }
    let tag = node.tag_name().name();
    if matches!(tag, "defs" | "metadata" | "title" | "desc" | "style") {
        return;
    }

    let style = apply_style(node, parent_style);
    let xform = parent_xform.pre_concat(parse_transform(node.attribute("transform")));

    let own_id = node
        .attribute("id")
        .or_else(|| node.attribute("data-part"))
        .or_else(|| node.attribute("class"));
    let target = own_id.and_then(classify_id).or(inherited_target);
    let part_name = if own_id.and_then(classify_id).is_some() {
        own_id.map(|s| s.to_string())
    } else {
        inherited_name
    };

    if let Some(path) = shape_from_node(node) {
        out.push(ParsedShape {
            path,
            style,
            transform: xform,
            target,
            part_name: part_name.clone(),
        });
    }

    for child in node.children() {
        collect_shapes(child, style, xform, target, part_name.clone(), out);
    }
}

fn rasterize_shapes(shapes: &[&ParsedShape], fit_xform: Transform) -> Vec<RgbaPixel> {
    let mut pm = Pixmap::new(BOARD_SIZE as u32, BOARD_SIZE as u32).unwrap();
    for sh in shapes {
        let xf = fit_xform.pre_concat(sh.transform);
        if let Some([r, g, b, a]) = sh.style.fill {
            let mut paint = Paint::default();
            let alpha = ((a as f32) * sh.style.opacity).round().clamp(0.0, 255.0) as u8;
            paint.set_color(Color::from_rgba8(r, g, b, alpha));
            paint.anti_alias = true;
            pm.fill_path(&sh.path, &paint, FillRule::Winding, xf, None);
        }
        if let Some([r, g, b, a]) = sh.style.stroke
            && sh.style.stroke_width > 0.0
        {
            let mut paint = Paint::default();
            let alpha = ((a as f32) * sh.style.opacity).round().clamp(0.0, 255.0) as u8;
            paint.set_color(Color::from_rgba8(r, g, b, alpha));
            paint.anti_alias = true;
            let stroke = Stroke {
                width: sh.style.stroke_width,
                line_cap: LineCap::Round,
                line_join: LineJoin::Round,
                ..Default::default()
            };
            pm.stroke_path(&sh.path, &paint, &stroke, xf, None);
        }
    }

    let mut buf = vec![[0, 0, 0, 0]; BOARD_LEN];
    for (i, p) in pm.pixels().iter().enumerate() {
        let a = p.alpha();
        if a > 0 {
            buf[i] = [
                ((p.red() as u16 * 255) / a as u16).min(255) as u8,
                ((p.green() as u16 * 255) / a as u16).min(255) as u8,
                ((p.blue() as u16 * 255) / a as u16).min(255) as u8,
                a,
            ];
        }
    }
    buf
}

/// Import a 2D SVG string into a fully rigged `StudioProject`.
pub fn import_svg_to_project(
    svg_text: &str,
    display_name: &str,
    archetype: Archetype,
) -> Result<StudioProject, String> {
    let doc = roxmltree::Document::parse(svg_text).map_err(|e| format!("invalid SVG XML: {e}"))?;
    let root = doc.root_element();

    let mut shapes = Vec::new();
    collect_shapes(
        root,
        StyleState::default(),
        Transform::identity(),
        None,
        None,
        &mut shapes,
    );
    if shapes.is_empty() {
        return Err("No supported SVG shapes found (<path>, <circle>, <ellipse>, <rect>, <polygon>, <line>).".to_string());
    }

    // Determine SVG viewport bounds (prefer viewBox, fallback to combined shape bounds)
    let (vx, vy, vw, vh) = if let Some(vb) = root.attribute("viewBox") {
        let nums: Vec<f32> = vb
            .split(|c: char| c == ',' || c.is_whitespace())
            .filter(|t| !t.is_empty())
            .filter_map(|t| t.parse::<f32>().ok())
            .collect();
        if nums.len() == 4 && nums[2] > 0.0 && nums[3] > 0.0 {
            (nums[0], nums[1], nums[2], nums[3])
        } else {
            (0.0, 0.0, 128.0, 128.0)
        }
    } else {
        let mut min_x = f32::INFINITY;
        let mut min_y = f32::INFINITY;
        let mut max_x = f32::NEG_INFINITY;
        let mut max_y = f32::NEG_INFINITY;
        for sh in &shapes {
            if let Some(tp) = sh.path.clone().transform(sh.transform) {
                let b = tp.bounds();
                min_x = min_x.min(b.left());
                min_y = min_y.min(b.top());
                max_x = max_x.max(b.right());
                max_y = max_y.max(b.bottom());
            }
        }
        if min_x.is_finite() && max_x > min_x && max_y > min_y {
            (min_x, min_y, max_x - min_x, max_y - min_y)
        } else {
            (0.0, 0.0, 128.0, 128.0)
        }
    };

    let pad = 10.0;
    let inner = BOARD_SIZE as f32 - pad * 2.0;
    let scale = (inner / vw.max(1.0)).min(inner / vh.max(1.0));
    let ox = (BOARD_SIZE as f32 - vw * scale) * 0.5 - vx * scale;
    let oy = (BOARD_SIZE as f32 - vh * scale) * 0.5 - vy * scale;
    let fit_xform = Transform::from_translate(ox, oy).pre_scale(scale, scale);

    let has_semantic_parts = shapes
        .iter()
        .any(|s| matches!(s.target, Some(SemanticTarget::Part(_))));

    let mut project = StudioProject::blank(archetype);
    project.display_name = display_name.to_string();
    project.pack_id = sanitize_pack_id(display_name);
    project.parts.clear();

    if has_semantic_parts {
        let mut body_shapes: Vec<&ParsedShape> = Vec::new();
        let mut face_buckets: [Vec<&ParsedShape>; 5] = Default::default();
        let mut part_groups: Vec<(String, PartRole, Vec<&ParsedShape>)> = Vec::new();

        for sh in &shapes {
            match sh.target {
                Some(SemanticTarget::Face(slot)) => face_buckets[slot.index()].push(sh),
                Some(SemanticTarget::Part(role)) => {
                    let pname = sh
                        .part_name
                        .clone()
                        .unwrap_or_else(|| role.label().to_string());
                    if let Some((_, _, list)) = part_groups
                        .iter_mut()
                        .find(|(n, r, _)| *n == pname && *r == role)
                    {
                        list.push(sh);
                    } else {
                        part_groups.push((pname, role, vec![sh]));
                    }
                }
                _ => body_shapes.push(sh),
            }
        }

        project.body = rasterize_shapes(&body_shapes, fit_xform);
        for slot in FaceSlot::all() {
            if !face_buckets[slot.index()].is_empty() {
                project.faces[slot.index()] =
                    rasterize_shapes(&face_buckets[slot.index()], fit_xform);
            }
        }
        for (name, role, list) in part_groups {
            let mut part = RigPart::new(name, role);
            part.pixels = rasterize_shapes(&list, fit_xform);
            if buf_bbox(&part.pixels).is_some() {
                project.parts.push(part);
            }
        }
    } else {
        // Geometric auto-split for untagged SVGs: detect lower limb shapes & far-right tail shapes
        let all_refs: Vec<&ParsedShape> = shapes.iter().collect();
        let full_buf = rasterize_shapes(&all_refs, fit_xform);
        let (gx0, gy0, gx1, gy1) = buf_bbox(&full_buf).unwrap_or((16, 16, 112, 112));
        let gw = (gx1 - gx0).max(1) as f32;
        let gh = (gy1 - gy0).max(1) as f32;

        let mut body_shapes: Vec<&ParsedShape> = Vec::new();
        let mut front_shapes: Vec<&ParsedShape> = Vec::new();
        let mut back_shapes: Vec<&ParsedShape> = Vec::new();
        let mut tail_shapes: Vec<&ParsedShape> = Vec::new();

        for sh in &shapes {
            let single = rasterize_shapes(&[sh], fit_xform);
            let Some((sx0, sy0, sx1, sy1)) = buf_bbox(&single) else {
                continue;
            };
            let sw = (sx1 - sx0 + 1) as f32;
            let sh_h = (sy1 - sy0 + 1) as f32;
            let area_ratio = (sw * sh_h) / (gw * gh);
            let cy_norm = (((sy0 + sy1) as f32 * 0.5) - gy0 as f32) / gh;
            let cx_norm = (((sx0 + sx1) as f32 * 0.5) - gx0 as f32) / gw;

            if area_ratio < 0.22 && cy_norm > 0.68 && archetype != Archetype::Crawler {
                if cx_norm < 0.52 {
                    front_shapes.push(sh);
                } else {
                    back_shapes.push(sh);
                }
            } else if area_ratio < 0.22 && cx_norm > 0.78 && cy_norm > 0.25 && cy_norm < 0.75 {
                tail_shapes.push(sh);
            } else {
                body_shapes.push(sh);
            }
        }

        if body_shapes.is_empty() {
            project.body = full_buf;
        } else {
            project.body = rasterize_shapes(&body_shapes, fit_xform);
            if !front_shapes.is_empty() {
                let mut fl = RigPart::new("Front Limb", PartRole::FrontLimb);
                fl.pixels = rasterize_shapes(&front_shapes, fit_xform);
                project.parts.push(fl);
            }
            if !back_shapes.is_empty() {
                let mut bl = RigPart::new("Back Limb", PartRole::BackLimb);
                bl.pixels = rasterize_shapes(&back_shapes, fit_xform);
                project.parts.push(bl);
            }
            if !tail_shapes.is_empty() {
                let mut tl = RigPart::new("Tail", PartRole::Tail);
                tl.pixels = rasterize_shapes(&tail_shapes, fit_xform);
                project.parts.push(tl);
            }
        }
    }

    project.auto_fit_anchors();
    Ok(project)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn semantic_svg_splits_body_wings_tail_and_faces_automatically() {
        let svg = r##"
        <svg viewBox="0 0 128 128" xmlns="http://www.w3.org/2000/svg">
          <g id="body">
            <ellipse cx="64" cy="68" rx="24" ry="16" fill="#58b8f5" stroke="#1e2a3a" stroke-width="2"/>
          </g>
          <g id="face">
            <circle cx="46" cy="62" r="3" fill="#1e2a3a"/>
          </g>
          <g id="wing">
            <ellipse cx="68" cy="66" rx="14" ry="8" fill="#3a8ad6"/>
          </g>
          <g id="tail">
            <polygon points="86,66 108,58 106,74" fill="#3a8ad6"/>
          </g>
        </svg>
        "##;
        let proj = import_svg_to_project(svg, "Vector Bird", Archetype::Bird).unwrap();
        assert!(buf_bbox(&proj.body).is_some());
        assert!(buf_bbox(&proj.faces[FaceSlot::Neutral.index()]).is_some());
        assert_eq!(proj.parts.len(), 2);
        assert!(proj.parts.iter().any(|p| p.role == PartRole::Wing));
        assert!(proj.parts.iter().any(|p| p.role == PartRole::Tail));
        let (_sheet, _icon, xml) = proj.build_export_artifacts().unwrap();
        assert!(xml.contains("Vector Bird"));
    }

    #[test]
    fn untagged_svg_auto_splits_limbs_geometrically() {
        let svg = r##"
        <svg viewBox="0 0 128 128" xmlns="http://www.w3.org/2000/svg">
          <ellipse cx="64" cy="54" rx="28" ry="20" fill="#e8a258"/>
          <rect x="44" y="74" width="10" height="22" fill="#d08238"/>
          <rect x="74" y="74" width="10" height="22" fill="#d08238"/>
        </svg>
        "##;
        let proj = import_svg_to_project(svg, "Auto Pet", Archetype::Quadruped).unwrap();
        assert!(buf_bbox(&proj.body).is_some());
        assert!(proj.parts.iter().any(|p| p.role == PartRole::FrontLimb));
        assert!(proj.parts.iter().any(|p| p.role == PartRole::BackLimb));
    }
}