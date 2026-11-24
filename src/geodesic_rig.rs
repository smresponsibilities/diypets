//! Native Rust Geodesic-Weighted Rigid Moving Least Squares (Schaefer et al., SIGGRAPH 2006;
//! Jacobson et al., SIGGRAPH 2011) for tear-free, rubber-free pixel-art rigging in Studio.

use image::{Rgba, RgbaImage};
use std::cmp::Ordering;
use std::collections::BinaryHeap;

#[derive(Debug, Clone, Copy)]
pub struct RigPin {
    pub x: f32,
    pub y: f32,
    pub depth: i8,
}

#[derive(Debug, Clone)]
pub struct GeodesicSkinning {
    pub width: u32,
    pub height: u32,
    pub num_pins: usize,
    pub weights: Vec<f32>,
    pub dom_pin: Vec<usize>,
}

#[derive(Copy, Clone)]
struct HeapItem {
    dist: f32,
    idx: usize,
}

impl PartialEq for HeapItem {
    fn eq(&self, other: &Self) -> bool {
        self.dist == other.dist && self.idx == other.idx
    }
}
impl Eq for HeapItem {}
impl PartialOrd for HeapItem {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for HeapItem {
    fn cmp(&self, other: &Self) -> Ordering {
        other
            .dist
            .partial_cmp(&self.dist)
            .unwrap_or(Ordering::Equal)
            .then_with(|| self.idx.cmp(&other.idx))
    }
}

/// Precompute 8-connected interior geodesic skinning weights (12x air penalty isolates legs across gaps).
pub fn compute_geodesic_skinning(img: &RgbaImage, pins: &[RigPin]) -> GeodesicSkinning {
    let (w, h) = img.dimensions();
    let n = (w as usize) * (h as usize);
    let num_pins = pins.len();
    let mut weights = vec![0.0f32; n * num_pins];
    let mut dom_pin = vec![0usize; n];
    if n == 0 || num_pins == 0 {
        return GeodesicSkinning {
            width: w,
            height: h,
            num_pins,
            weights,
            dom_pin,
        };
    }

    let mut all_dists = vec![f32::INFINITY; n * num_pins];
    let dirs: [(i32, i32, f32); 8] = [
        (-1, 0, 1.0),
        (1, 0, 1.0),
        (0, -1, 1.0),
        (0, 1, 1.0),
        (-1, -1, std::f32::consts::SQRT_2),
        (1, -1, std::f32::consts::SQRT_2),
        (-1, 1, std::f32::consts::SQRT_2),
        (1, 1, std::f32::consts::SQRT_2),
    ];

    for (pi, pin) in pins.iter().enumerate() {
        let base = pi * n;
        let sx = (pin.x.round() as i32).clamp(0, w as i32 - 1) as usize;
        let sy = (pin.y.round() as i32).clamp(0, h as i32 - 1) as usize;
        let start = sy * (w as usize) + sx;
        all_dists[base + start] = 0.0;
        let mut heap = BinaryHeap::new();
        heap.push(HeapItem {
            dist: 0.0,
            idx: start,
        });

        while let Some(HeapItem { dist, idx }) = heap.pop() {
            if dist > all_dists[base + idx] {
                continue;
            }
            let ux = (idx % (w as usize)) as i32;
            let uy = (idx / (w as usize)) as i32;
            for (dx, dy, cost) in dirs {
                let nx = ux + dx;
                let ny = uy + dy;
                if nx < 0 || nx >= w as i32 || ny < 0 || ny >= h as i32 {
                    continue;
                }
                let v = (ny as usize) * (w as usize) + (nx as usize);
                let opaque = img.get_pixel(nx as u32, ny as u32)[3] > 0;
                let nd = dist + cost * if opaque { 1.0 } else { 12.0 };
                if nd < all_dists[base + v] {
                    all_dists[base + v] = nd;
                    heap.push(HeapItem { dist: nd, idx: v });
                }
            }
        }
    }

    for i in 0..n {
        let mut w_sum = 0.0f32;
        let mut best_w = -1.0f32;
        let mut best_p = 0usize;
        for pi in 0..num_pins {
            let d = all_dists[pi * n + i];
            let wi = 1.0 / (d + 1.25).powf(3.5);
            weights[i * num_pins + pi] = wi;
            w_sum += wi;
            if wi > best_w {
                best_w = wi;
                best_p = pi;
            }
        }
        let inv = if w_sum > 0.0 {
            1.0 / w_sum
        } else {
            1.0 / (num_pins as f32)
        };
        for pi in 0..num_pins {
            weights[i * num_pins + pi] *= inv;
        }
        dom_pin[i] = best_p;
    }

    GeodesicSkinning {
        width: w,
        height: h,
        num_pins,
        weights,
        dom_pin,
    }
}

/// Evaluate closed-form 2D Rigid Moving Least Squares forward transform for vertex `(vx, vy)`.
pub fn eval_rigid_mls_forward(
    vx: f32,
    vy: f32,
    src_pins: &[RigPin],
    dst_pins: &[RigPin],
    w_row: &[f32],
) -> (f32, f32) {
    let mut px_star = 0.0f32;
    let mut py_star = 0.0f32;
    let mut qx_star = 0.0f32;
    let mut qy_star = 0.0f32;
    for i in 0..src_pins.len() {
        let wi = w_row[i];
        px_star += wi * src_pins[i].x;
        py_star += wi * src_pins[i].y;
        qx_star += wi * dst_pins[i].x;
        qy_star += wi * dst_pins[i].y;
    }
    let dvx = vx - px_star;
    let dvy = vy - py_star;
    let dv_len = dvx.hypot(dvy);
    if dv_len < 1e-6 {
        return (qx_star, qy_star);
    }

    let mut frx = 0.0f32;
    let mut fry = 0.0f32;
    for i in 0..src_pins.len() {
        let wi = w_row[i];
        let phx = src_pins[i].x - px_star;
        let phy = src_pins[i].y - py_star;
        let qhx = dst_pins[i].x - qx_star;
        let qhy = dst_pins[i].y - qy_star;
        let dot = phx * dvx + phy * dvy;
        let cross = phx * dvy - phy * dvx;
        frx += wi * (qhx * dot - qhy * cross);
        fry += wi * (qhy * dot + qhx * cross);
    }
    let fr_len = frx.hypot(fry);
    if fr_len < 1e-6 {
        return (qx_star, qy_star);
    }
    let scale = dv_len / fr_len;
    (qx_star + frx * scale, qy_star + fry * scale)
}

/// Warp an RGBA sprite using precomputed geodesic weights & depth-sorted pixel rasterization.
pub fn warp_geodesic_mls(
    img: &RgbaImage,
    src_pins: &[RigPin],
    dst_pins: &[RigPin],
    skin: &GeodesicSkinning,
) -> RgbaImage {
    let (w, h) = img.dimensions();
    let mut out = RgbaImage::new(w, h);
    let mut zbuf = vec![i8::MIN; (w as usize) * (h as usize)];

    for y in 0..h {
        for x in 0..w {
            let px = *img.get_pixel(x, y);
            if px[3] == 0 {
                continue;
            }
            let idx = (y as usize) * (w as usize) + (x as usize);
            let w_row = &skin.weights[idx * skin.num_pins..(idx + 1) * skin.num_pins];
            let (tx, ty) = eval_rigid_mls_forward(x as f32, y as f32, src_pins, dst_pins, w_row);
            let ix = tx.round() as i32;
            let iy = ty.round() as i32;
            if ix >= 0 && ix < w as i32 && iy >= 0 && iy < h as i32 {
                let d_idx = (iy as usize) * (w as usize) + (ix as usize);
                let depth = src_pins[skin.dom_pin[idx]].depth;
                if depth >= zbuf[d_idx] {
                    out.put_pixel(ix as u32, iy as u32, Rgba([px[0], px[1], px[2], 255]));
                    zbuf[d_idx] = depth;
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn geodesic_skinning_isolates_legs_across_transparent_gap() {
        let mut img = RgbaImage::new(32, 32);
        for y in 12..30 {
            for x in 6..10 {
                img.put_pixel(x, y, Rgba([240, 240, 240, 255]));
            }
            for x in 16..20 {
                img.put_pixel(x, y, Rgba([200, 200, 200, 255]));
            }
        }
        for y in 8..12 {
            for x in 6..20 {
                img.put_pixel(x, y, Rgba([220, 220, 220, 255]));
            }
        }
        let pins = [
            RigPin {
                x: 8.0,
                y: 28.0,
                depth: 5,
            },
            RigPin {
                x: 18.0,
                y: 28.0,
                depth: 1,
            },
        ];
        let skin = compute_geodesic_skinning(&img, &pins);
        let left_paw_idx = 28 * 32 + 8;
        let right_paw_idx = 28 * 32 + 18;
        assert_eq!(skin.dom_pin[left_paw_idx], 0);
        assert_eq!(skin.dom_pin[right_paw_idx], 1);
        assert!(skin.weights[left_paw_idx * 2] > 0.90);

        let dst = [
            RigPin {
                x: 5.0,
                y: 26.0,
                depth: 5,
            },
            RigPin {
                x: 20.0,
                y: 28.0,
                depth: 1,
            },
        ];
        let warped = warp_geodesic_mls(&img, &pins, &dst, &skin);
        assert_eq!(warped.dimensions(), (32, 32));
    }
}
