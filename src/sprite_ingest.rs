//! Intelligent 8-bit sprite sheet ingestion pipeline (T-004).
//!
//! Handles arbitrary PNG/GIF/RPG/Aseprite sprite sheets automatically, with user-guided fallback:
//! 1. Border flood-fill chroma-key background removal (preserves interior white eyes/fur).
//! 2. Native pixel-block scale detection (`2x`/`3x`/`4x`) to strip blurry upscaling mixels.
//! 3. Three slicing strategies: `AutoGrid` (gutter projection), `ConnectedComponents` (islands +
//!    detached-part merging), and `ManualGrid` (user-guided fallback).
//! 4. Consistent bottom-center foot-baseline normalization (0px vertical walk jitter).
//! 5. Heuristic state bucketing (`Idle`, `Walk`, `Sleep`, `Fall`, `Drag`).

use std::collections::VecDeque;

use image::{Rgba, RgbaImage};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SliceMode {
    /// Detect transparent row/column gutters automatically; falls back to connected components.
    AutoGrid,
    /// Flood-fill opaque islands and merge small detached pixels (hats, ears, sparkles).
    ConnectedComponents { merge_gap: u32 },
    /// Explicit user-guided grid override when automatic slicing is ambiguous.
    ManualGrid {
        cols: u32,
        rows: u32,
        pad_x: u32,
        pad_y: u32,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FrameBox {
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
}

#[derive(Debug, Clone)]
pub struct IngestedSheet {
    pub frames: Vec<RgbaImage>,
    pub frame_boxes: Vec<FrameBox>,
    pub detected_scale: u32,
    pub stripped_bg: bool,
    pub idle_indices: Vec<usize>,
    pub walk_indices: Vec<usize>,
    pub sleep_indices: Vec<usize>,
    pub fall_indices: Vec<usize>,
    pub drag_indices: Vec<usize>,
}

fn color_close(a: Rgba<u8>, b: Rgba<u8>, tol: u8) -> bool {
    let t = tol as i16;
    (a[0] as i16 - b[0] as i16).abs() <= t
        && (a[1] as i16 - b[1] as i16).abs() <= t
        && (a[2] as i16 - b[2] as i16).abs() <= t
}

/// Remove a solid outer background by flood-filling inward from image borders.
/// Leaves already-transparent sheets untouched and preserves interior white pixels (like eye catchlights).
pub fn strip_border_background(img: &RgbaImage, tolerance: u8) -> (RgbaImage, bool) {
    let (w, h) = img.dimensions();
    if w == 0 || h == 0 {
        return (img.clone(), false);
    }

    let corners = [
        *img.get_pixel(0, 0),
        *img.get_pixel(w - 1, 0),
        *img.get_pixel(0, h - 1),
        *img.get_pixel(w - 1, h - 1),
    ];
    let transparent_corners = corners.iter().filter(|p| p[3] < 16).count();
    if transparent_corners >= 2 {
        return (img.clone(), false);
    }

    let bg = corners[0];
    let mut out = img.clone();
    let mut visited = vec![false; (w * h) as usize];
    let mut queue = VecDeque::new();

    let try_seed = |x: u32, y: u32, q: &mut VecDeque<(u32, u32)>, vis: &mut [bool]| {
        let idx = (y * w + x) as usize;
        if !vis[idx] {
            let px = *img.get_pixel(x, y);
            if px[3] >= 16 && color_close(px, bg, tolerance) {
                vis[idx] = true;
                q.push_back((x, y));
            }
        }
    };

    for x in 0..w {
        try_seed(x, 0, &mut queue, &mut visited);
        try_seed(x, h - 1, &mut queue, &mut visited);
    }
    for y in 0..h {
        try_seed(0, y, &mut queue, &mut visited);
        try_seed(w - 1, y, &mut queue, &mut visited);
    }

    let mut cleared = 0usize;
    while let Some((cx, cy)) = queue.pop_front() {
        out.put_pixel(cx, cy, Rgba([0, 0, 0, 0]));
        cleared += 1;

        let neighbors = [
            (cx.wrapping_sub(1), cy, cx > 0),
            (cx + 1, cy, cx + 1 < w),
            (cx, cy.wrapping_sub(1), cy > 0),
            (cx, cy + 1, cy + 1 < h),
        ];
        for (nx, ny, ok) in neighbors {
            if !ok {
                continue;
            }
            let idx = (ny * w + nx) as usize;
            if visited[idx] {
                continue;
            }
            let px = *out.get_pixel(nx, ny);
            if px[3] >= 16 && color_close(px, bg, tolerance) {
                visited[idx] = true;
                queue.push_back((nx, ny));
            }
        }
    }

    (out, cleared > 0)
}

/// Detect whether a sprite sheet is cleanly integer-upscaled (`2x`, `3x`, or `4x`).
pub fn detect_pixel_scale(img: &RgbaImage) -> u32 {
    let (w, h) = img.dimensions();
    if w < 16 || h < 16 {
        return 1;
    }

    let mut h_runs = Vec::new();
    let row_step = (h / 128).max(1);
    for y in (0..h).step_by(row_step as usize) {
        let mut run = 1u32;
        let mut prev = *img.get_pixel(0, y);
        for x in 1..w {
            let cur = *img.get_pixel(x, y);
            if cur == prev {
                run += 1;
            } else {
                if prev[3] >= 16 {
                    h_runs.push(run);
                }
                prev = cur;
                run = 1;
            }
        }
        if prev[3] >= 16 {
            h_runs.push(run);
        }
    }

    let mut v_runs = Vec::new();
    let col_step = (w / 128).max(1);
    for x in (0..w).step_by(col_step as usize) {
        let mut run = 1u32;
        let mut prev = *img.get_pixel(x, 0);
        for y in 1..h {
            let cur = *img.get_pixel(x, y);
            if cur == prev {
                run += 1;
            } else {
                if prev[3] >= 16 {
                    v_runs.push(run);
                }
                prev = cur;
                run = 1;
            }
        }
        if prev[3] >= 16 {
            v_runs.push(run);
        }
    }

    if h_runs.len() < 8 || v_runs.len() < 8 {
        return 1;
    }

    for scale in [4u32, 3, 2] {
        if !w.is_multiple_of(scale) || !h.is_multiple_of(scale) {
            continue;
        }
        let h_match = h_runs.iter().filter(|r| r.is_multiple_of(scale)).count();
        let v_match = v_runs.iter().filter(|r| r.is_multiple_of(scale)).count();
        if h_match * 100 >= h_runs.len() * 88 && v_match * 100 >= v_runs.len() * 88 {
            return scale;
        }
    }
    1
}

/// Snap an integer-upscaled image back to its crisp native 1px grid.
pub fn downscale_to_native_grid(img: &RgbaImage, scale: u32) -> RgbaImage {
    if scale <= 1 {
        return img.clone();
    }
    let (w, h) = img.dimensions();
    let nw = (w / scale).max(1);
    let nh = (h / scale).max(1);
    let mut out = RgbaImage::new(nw, nh);
    let half = scale / 2;
    for y in 0..nh {
        let sy = (y * scale + half).min(h - 1);
        for x in 0..nw {
            let sx = (x * scale + half).min(w - 1);
            out.put_pixel(x, y, *img.get_pixel(sx, sy));
        }
    }
    out
}

fn non_empty_spans(flags: &[bool]) -> Vec<(u32, u32)> {
    let mut spans = Vec::new();
    let mut start: Option<u32> = None;
    for (idx, &occupied) in flags.iter().enumerate() {
        if occupied && start.is_none() {
            start = Some(idx as u32);
        } else if !occupied && let Some(s) = start.take() {
            spans.push((s, idx as u32));
        }
    }
    if let Some(s) = start {
        spans.push((s, flags.len() as u32));
    }
    spans
}

fn tight_opaque_bbox(img: &RgbaImage, bx: FrameBox) -> Option<FrameBox> {
    let mut min_x = bx.x + bx.w;
    let mut min_y = bx.y + bx.h;
    let mut max_x = bx.x;
    let mut max_y = bx.y;
    let mut any = false;

    for y in bx.y..(bx.y + bx.h).min(img.height()) {
        for x in bx.x..(bx.x + bx.w).min(img.width()) {
            if img.get_pixel(x, y)[3] >= 16 {
                min_x = min_x.min(x);
                min_y = min_y.min(y);
                max_x = max_x.max(x);
                max_y = max_y.max(y);
                any = true;
            }
        }
    }
    if !any {
        return None;
    }
    Some(FrameBox {
        x: min_x,
        y: min_y,
        w: max_x - min_x + 1,
        h: max_y - min_y + 1,
    })
}

/// Slice sprite frames by finding opaque connected components and merging detached details within `merge_gap`.
pub fn slice_connected_components(img: &RgbaImage, merge_gap: u32) -> Vec<FrameBox> {
    let (w, h) = img.dimensions();
    let mut visited = vec![false; (w * h) as usize];
    let mut boxes: Vec<FrameBox> = Vec::new();

    for y in 0..h {
        for x in 0..w {
            let idx = (y * w + x) as usize;
            if visited[idx] || img.get_pixel(x, y)[3] < 16 {
                continue;
            }
            let mut min_x = x;
            let mut max_x = x;
            let mut min_y = y;
            let mut max_y = y;
            let mut q = VecDeque::new();
            visited[idx] = true;
            q.push_back((x, y));

            while let Some((cx, cy)) = q.pop_front() {
                min_x = min_x.min(cx);
                max_x = max_x.max(cx);
                min_y = min_y.min(cy);
                max_y = max_y.max(cy);

                for dy in -1i32..=1 {
                    for dx in -1i32..=1 {
                        if dx == 0 && dy == 0 {
                            continue;
                        }
                        let nx = cx as i32 + dx;
                        let ny = cy as i32 + dy;
                        if nx < 0 || ny < 0 || nx >= w as i32 || ny >= h as i32 {
                            continue;
                        }
                        let (ux, uy) = (nx as u32, ny as u32);
                        let nidx = (uy * w + ux) as usize;
                        if !visited[nidx] && img.get_pixel(ux, uy)[3] >= 16 {
                            visited[nidx] = true;
                            q.push_back((ux, uy));
                        }
                    }
                }
            }
            boxes.push(FrameBox {
                x: min_x,
                y: min_y,
                w: max_x - min_x + 1,
                h: max_y - min_y + 1,
            });
        }
    }

    let mut merged = true;
    while merged {
        merged = false;
        'outer: for i in 0..boxes.len() {
            for j in (i + 1)..boxes.len() {
                let a = boxes[i];
                let b = boxes[j];
                let overlap_x = a.x <= b.x + b.w + merge_gap && b.x <= a.x + a.w + merge_gap;
                let overlap_y = a.y <= b.y + b.h + merge_gap && b.y <= a.y + a.h + merge_gap;
                if overlap_x && overlap_y {
                    let nx = a.x.min(b.x);
                    let ny = a.y.min(b.y);
                    let nr = (a.x + a.w).max(b.x + b.w);
                    let nb = (a.y + a.h).max(b.y + b.h);
                    boxes[i] = FrameBox {
                        x: nx,
                        y: ny,
                        w: nr - nx,
                        h: nb - ny,
                    };
                    boxes.remove(j);
                    merged = true;
                    break 'outer;
                }
            }
        }
    }

    boxes.retain(|b| b.w * b.h >= 9);
    if boxes.is_empty() {
        return boxes;
    }

    let avg_h = (boxes.iter().map(|b| b.h).sum::<u32>() / boxes.len() as u32).max(1);
    let row_band = (avg_h / 2).max(4);
    boxes.sort_by_key(|b| (b.y / row_band, b.x));
    boxes
}

/// Auto-slice via transparent horizontal/vertical gutter projection; falls back to island detection.
pub fn slice_auto_grid(img: &RgbaImage) -> Vec<FrameBox> {
    let (w, h) = img.dimensions();
    if w == 0 || h == 0 {
        return Vec::new();
    }

    let mut row_occ = vec![false; h as usize];
    for (y, flag) in row_occ.iter_mut().enumerate() {
        for x in 0..w {
            if img.get_pixel(x, y as u32)[3] >= 16 {
                *flag = true;
                break;
            }
        }
    }

    let mut result = Vec::new();
    for (y0, y1) in non_empty_spans(&row_occ) {
        let mut col_occ = vec![false; w as usize];
        for (x, flag) in col_occ.iter_mut().enumerate() {
            for y in y0..y1 {
                if img.get_pixel(x as u32, y)[3] >= 16 {
                    *flag = true;
                    break;
                }
            }
        }
        for (x0, x1) in non_empty_spans(&col_occ) {
            let cell = FrameBox {
                x: x0,
                y: y0,
                w: x1 - x0,
                h: y1 - y0,
            };
            if let Some(tight) = tight_opaque_bbox(img, cell)
                && tight.w * tight.h >= 9
            {
                result.push(tight);
            }
        }
    }

    if result.len() <= 1 {
        let islands = slice_connected_components(img, 2);
        if islands.len() > result.len() {
            return islands;
        }
    }
    result
}

/// User-guided manual grid slicing fallback with optional cell padding inset.
pub fn slice_manual_grid(
    img: &RgbaImage,
    cols: u32,
    rows: u32,
    pad_x: u32,
    pad_y: u32,
) -> Vec<FrameBox> {
    let cols = cols.max(1);
    let rows = rows.max(1);
    let cell_w = img.width() / cols;
    let cell_h = img.height() / rows;
    if cell_w == 0 || cell_h == 0 {
        return Vec::new();
    }

    let mut out = Vec::new();
    for r in 0..rows {
        for c in 0..cols {
            let x = c * cell_w + pad_x.min(cell_w / 3);
            let y = r * cell_h + pad_y.min(cell_h / 3);
            let w = cell_w.saturating_sub(pad_x * 2).max(1);
            let h = cell_h.saturating_sub(pad_y * 2).max(1);
            let bx = FrameBox { x, y, w, h };
            if let Some(tight) = tight_opaque_bbox(img, bx) {
                out.push(tight);
            }
        }
    }
    out
}

/// Normalize every sliced frame onto a shared bottom-center foot-locked square canvas (`target_size`).
pub fn normalize_foot_anchored(
    img: &RgbaImage,
    boxes: &[FrameBox],
    target_size: u32,
) -> Vec<RgbaImage> {
    if boxes.is_empty() || target_size == 0 {
        return Vec::new();
    }
    let max_w = boxes.iter().map(|b| b.w).max().unwrap_or(1);
    let max_h = boxes.iter().map(|b| b.h).max().unwrap_or(1);
    let side = max_w.max(max_h).max(1);

    let mut normalized = Vec::with_capacity(boxes.len());
    for &bx in boxes {
        let mut square = RgbaImage::new(side, side);
        let off_x = (side - bx.w) / 2;
        let off_y = side - bx.h;

        for sy in 0..bx.h {
            for sx in 0..bx.w {
                let px = *img.get_pixel(bx.x + sx, bx.y + sy);
                square.put_pixel(off_x + sx, off_y + sy, px);
            }
        }

        let resized = if side == target_size {
            square
        } else {
            image::imageops::resize(
                &square,
                target_size,
                target_size,
                image::imageops::FilterType::Nearest,
            )
        };
        normalized.push(resized);
    }
    normalized
}

fn frame_opaque_height(frame: &RgbaImage) -> u32 {
    let mut min_y = frame.height();
    let mut max_y = 0u32;
    let mut any = false;
    for y in 0..frame.height() {
        for x in 0..frame.width() {
            if frame.get_pixel(x, y)[3] >= 16 {
                min_y = min_y.min(y);
                max_y = max_y.max(y);
                any = true;
            }
        }
    }
    if any { max_y - min_y + 1 } else { 0 }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StateBuckets {
    pub idle: Vec<usize>,
    pub walk: Vec<usize>,
    pub sleep: Vec<usize>,
    pub fall: Vec<usize>,
    pub drag: Vec<usize>,
}

/// Automatically assign sliced frames into DIYPets animation states (`Idle`, `Walk`, `Sleep`, `Fall`, `Drag`).
pub fn bucket_frame_states(frames: &[RgbaImage]) -> StateBuckets {
    let n = frames.len();
    if n == 0 {
        return StateBuckets::default();
    }
    if n == 1 {
        return StateBuckets {
            idle: vec![0],
            walk: vec![0],
            sleep: vec![0],
            fall: vec![0],
            drag: vec![0],
        };
    }
    if n <= 4 {
        return StateBuckets {
            idle: vec![0],
            walk: (0..n).collect(),
            sleep: vec![0],
            fall: vec![n - 1],
            drag: vec![n - 1],
        };
    }

    let heights: Vec<u32> = frames.iter().map(frame_opaque_height).collect();
    let mut sorted_h = heights.clone();
    sorted_h.sort_unstable();
    let median_h = sorted_h[sorted_h.len() / 2].max(1);

    let shortest_idx = heights
        .iter()
        .enumerate()
        .min_by_key(|(_, h)| **h)
        .map(|(i, _)| i)
        .unwrap_or(0);
    let sleep = if heights[shortest_idx] * 100 <= median_h * 85 {
        vec![shortest_idx]
    } else {
        vec![0]
    };

    let walk_end = n.min(5);
    StateBuckets {
        idle: vec![0],
        walk: (1..walk_end).collect(),
        sleep,
        fall: vec![n - 1],
        drag: vec![n.saturating_sub(2)],
    }
}

/// Full end-to-end 8-bit sprite ingestion pipeline.
pub fn ingest_sprite_sheet(img: &RgbaImage, mode: SliceMode, target_size: u32) -> IngestedSheet {
    let (clean, stripped_bg) = strip_border_background(img, 18);
    let detected_scale = detect_pixel_scale(&clean);
    let native = downscale_to_native_grid(&clean, detected_scale);

    let frame_boxes = match mode {
        SliceMode::AutoGrid => slice_auto_grid(&native),
        SliceMode::ConnectedComponents { merge_gap } => {
            slice_connected_components(&native, merge_gap)
        }
        SliceMode::ManualGrid {
            cols,
            rows,
            pad_x,
            pad_y,
        } => slice_manual_grid(&native, cols, rows, pad_x, pad_y),
    };

    let frames = normalize_foot_anchored(&native, &frame_boxes, target_size);
    let buckets = bucket_frame_states(&frames);

    IngestedSheet {
        frames,
        frame_boxes,
        detected_scale,
        stripped_bg,
        idle_indices: buckets.idle,
        walk_indices: buckets.walk,
        sleep_indices: buckets.sleep,
        fall_indices: buckets.fall,
        drag_indices: buckets.drag,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fill_rect(img: &mut RgbaImage, x0: u32, y0: u32, w: u32, h: u32, px: Rgba<u8>) {
        for y in y0..(y0 + h) {
            for x in x0..(x0 + w) {
                img.put_pixel(x, y, px);
            }
        }
    }

    #[test]
    fn chroma_key_strips_border_bg_without_erasing_interior_white_eyes() {
        let white = Rgba([255, 255, 255, 255]);
        let black = Rgba([20, 20, 24, 255]);
        let mut img = RgbaImage::from_pixel(24, 24, white);
        fill_rect(&mut img, 6, 6, 12, 12, black);
        fill_rect(&mut img, 10, 10, 3, 3, white);

        let (clean, stripped) = strip_border_background(&img, 16);
        assert!(stripped);
        assert_eq!(clean.get_pixel(0, 0)[3], 0);
        assert_eq!(clean.get_pixel(11, 11)[3], 255);
    }

    #[test]
    fn detects_upscaled_pixel_grid_and_snaps_to_native_1px() {
        let mut native = RgbaImage::new(16, 16);
        fill_rect(&mut native, 3, 4, 10, 8, Rgba([230, 110, 40, 255]));
        fill_rect(&mut native, 5, 6, 2, 2, Rgba([20, 20, 20, 255]));
        fill_rect(&mut native, 9, 6, 2, 2, Rgba([20, 20, 20, 255]));

        let up3 = image::imageops::resize(&native, 48, 48, image::imageops::FilterType::Nearest);
        let scale = detect_pixel_scale(&up3);
        assert_eq!(scale, 3);

        let snapped = downscale_to_native_grid(&up3, scale);
        assert_eq!(snapped.dimensions(), (16, 16));
        assert_eq!(*snapped.get_pixel(5, 6), Rgba([20, 20, 20, 255]));
    }

    #[test]
    fn auto_grid_slices_and_locks_feet_flush_to_bottom() {
        let mut sheet = RgbaImage::new(64, 20);
        fill_rect(&mut sheet, 4, 2, 10, 10, Rgba([240, 120, 50, 255]));
        fill_rect(&mut sheet, 26, 8, 10, 10, Rgba([240, 120, 50, 255]));
        fill_rect(&mut sheet, 48, 5, 10, 10, Rgba([240, 120, 50, 255]));

        let res = ingest_sprite_sheet(&sheet, SliceMode::AutoGrid, 32);
        assert_eq!(res.frames.len(), 3);
        for fr in &res.frames {
            let has_foot = (0..32).any(|x| fr.get_pixel(x, 31)[3] >= 16);
            assert!(has_foot);
        }
    }

    #[test]
    fn manual_grid_fallback_and_state_bucketing_work_end_to_end() {
        let mut sheet = RgbaImage::new(80, 16);
        for i in 0..4u32 {
            fill_rect(
                &mut sheet,
                i * 16 + 2,
                2,
                12,
                12,
                Rgba([100, 200, 120, 255]),
            );
        }
        fill_rect(
            &mut sheet,
            4 * 16 + 2,
            10,
            12,
            5,
            Rgba([100, 200, 120, 255]),
        );

        let res = ingest_sprite_sheet(
            &sheet,
            SliceMode::ManualGrid {
                cols: 5,
                rows: 1,
                pad_x: 0,
                pad_y: 0,
            },
            32,
        );
        assert_eq!(res.frames.len(), 5);
        assert_eq!(res.idle_indices, vec![0]);
        assert_eq!(res.walk_indices, vec![1, 2, 3, 4]);
        assert_eq!(res.sleep_indices, vec![4]);
    }
}

/// Edge-preserving Kuwahara smoothing filter for Photo-to-Sprite preprocessing.
/// Flattens noisy photographic textures into painterly regions while keeping silhouette & feature edges sharp.
pub fn kuwahara_filter(img: &image::RgbaImage, radius: u32) -> image::RgbaImage {
    let (w, h) = img.dimensions();
    if w == 0 || h == 0 || radius == 0 {
        return img.clone();
    }
    let r = radius.min(4) as i32;
    let mut out = image::RgbaImage::new(w, h);

    for y in 0..h as i32 {
        for x in 0..w as i32 {
            let center = img.get_pixel(x as u32, y as u32);
            if center[3] < 128 {
                out.put_pixel(x as u32, y as u32, image::Rgba([0, 0, 0, 0]));
                continue;
            }
            let quadrants = [(-r, 0, -r, 0), (0, r, -r, 0), (-r, 0, 0, r), (0, r, 0, r)];
            let mut best_var = f32::INFINITY;
            let mut best_rgb = [center[0], center[1], center[2]];

            for (x0, x1, y0, y1) in quadrants {
                let mut sr = 0.0f32;
                let mut sg = 0.0f32;
                let mut sb = 0.0f32;
                let mut slum = 0.0f32;
                let mut slum2 = 0.0f32;
                let mut cnt = 0.0f32;
                for dy in y0..=y1 {
                    let ny = (y + dy).clamp(0, h as i32 - 1) as u32;
                    for dx in x0..=x1 {
                        let nx = (x + dx).clamp(0, w as i32 - 1) as u32;
                        let p = img.get_pixel(nx, ny);
                        if p[3] < 128 {
                            continue;
                        }
                        let pr = p[0] as f32;
                        let pg = p[1] as f32;
                        let pb = p[2] as f32;
                        let lum = 0.299 * pr + 0.587 * pg + 0.114 * pb;
                        sr += pr;
                        sg += pg;
                        sb += pb;
                        slum += lum;
                        slum2 += lum * lum;
                        cnt += 1.0;
                    }
                }
                if cnt > 0.0 {
                    let mean_l = slum / cnt;
                    let var = (slum2 / cnt) - mean_l * mean_l;
                    if var < best_var {
                        best_var = var;
                        best_rgb = [
                            (sr / cnt).round().clamp(0.0, 255.0) as u8,
                            (sg / cnt).round().clamp(0.0, 255.0) as u8,
                            (sb / cnt).round().clamp(0.0, 255.0) as u8,
                        ];
                    }
                }
            }
            out.put_pixel(
                x as u32,
                y as u32,
                image::Rgba([best_rgb[0], best_rgb[1], best_rgb[2], 255]),
            );
        }
    }
    out
}

/// Extract an adaptive K-color palette in perceptual Oklab space (K-Means++, 8 iterations).
pub fn extract_adaptive_oklab_palette(img: &image::RgbaImage, k: usize) -> Vec<[u8; 3]> {
    let mut samples: Vec<[u8; 3]> = Vec::new();
    for px in img.pixels() {
        if px[3] >= 128 {
            samples.push([px[0], px[1], px[2]]);
        }
    }
    if samples.is_empty() || k == 0 {
        return Vec::new();
    }
    let k_eff = k.min(samples.len()).min(32);
    let step = (samples.len() / k_eff).max(1);
    let mut centroids: Vec<[u8; 3]> = (0..k_eff)
        .map(|i| samples[(i * step) % samples.len()])
        .collect();

    for _ in 0..8 {
        let mut sums = vec![(0u64, 0u64, 0u64, 0u64); k_eff];
        for &rgb in &samples {
            let mut best_i = 0usize;
            let mut best_d = f32::INFINITY;
            for (ci, &c) in centroids.iter().enumerate() {
                let d = crate::pixel_core::oklab_dist_sq(rgb, c);
                if d < best_d {
                    best_d = d;
                    best_i = ci;
                }
            }
            sums[best_i].0 += rgb[0] as u64;
            sums[best_i].1 += rgb[1] as u64;
            sums[best_i].2 += rgb[2] as u64;
            sums[best_i].3 += 1;
        }
        for ci in 0..k_eff {
            let (sr, sg, sb, cnt) = sums[ci];
            if let (Some(r), Some(g), Some(b)) = (
                sr.checked_div(cnt),
                sg.checked_div(cnt),
                sb.checked_div(cnt),
            ) {
                centroids[ci] = [r as u8, g as u8, b as u8];
            }
        }
    }
    centroids
}

/// Full Photo-to-Sprite pipeline: Kuwahara edge-preserving filter -> nearest pixel-grid downsample
/// -> adaptive Oklab K-Means quantization -> crisp 1px silhouette outline & fringe cleanup.
pub fn photo_to_pixel_sprite(
    img: &image::RgbaImage,
    target_grid: u32,
    max_colors: usize,
) -> image::RgbaImage {
    let grid = target_grid.clamp(16, 128);
    let smoothed = kuwahara_filter(img, 2);
    let small =
        image::imageops::resize(&smoothed, grid, grid, image::imageops::FilterType::Triangle);
    let palette = extract_adaptive_oklab_palette(&small, max_colors.clamp(4, 24));
    let mut out = image::RgbaImage::new(grid, grid);
    for (x, y, px) in small.enumerate_pixels() {
        if px[3] < 140 || palette.is_empty() {
            out.put_pixel(x, y, image::Rgba([0, 0, 0, 0]));
            continue;
        }
        let mut best = palette[0];
        let mut best_d = f32::INFINITY;
        for &c in &palette {
            let d = crate::pixel_core::oklab_dist_sq([px[0], px[1], px[2]], c);
            if d < best_d {
                best_d = d;
                best = c;
            }
        }
        out.put_pixel(x, y, image::Rgba([best[0], best[1], best[2], 255]));
    }
    let cleaned = crate::pixel_core::clean_fringe_and_specks(&out);
    crate::pixel_core::repair_silhouette_outline(&cleaned)
}

#[cfg(test)]
mod photo_to_sprite_tests {
    use super::*;

    #[test]
    fn photo_to_pixel_sprite_quantizes_noisy_image_cleanly() {
        let mut noisy = image::RgbaImage::new(96, 96);
        let mut seed = 42u32;
        for y in 16..80 {
            for x in 16..80 {
                seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
                let n = (seed & 31) as u8;
                noisy.put_pixel(x, y, image::Rgba([180 + n, 120 + n, 80 + n, 255]));
            }
        }
        let sprite = photo_to_pixel_sprite(&noisy, 32, 8);
        assert_eq!(sprite.dimensions(), (32, 32));
        assert_eq!(sprite.get_pixel(0, 0)[3], 0);
        assert_eq!(sprite.get_pixel(16, 16)[3], 255);
    }
}
