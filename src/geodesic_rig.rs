//! Native Rust Geodesic-Weighted Rigid Moving Least Squares (Schaefer et al., SIGGRAPH 2006;
//! Jacobson et al., SIGGRAPH 2011) with hole-free barycentric quad rasterization and
//! 5 creature archetypes (Quadruped, Biped, Crawler, Swimmer, Bird) x 5 animation FX.

use image::{Rgba, RgbaImage};
use std::cmp::Ordering;
use std::collections::BinaryHeap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RigArchetype {
    Quadruped,
    Biped,
    Crawler,
    Swimmer,
    Bird,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RigFx {
    Walk,
    Sprint,
    Dance,
    Pet,
    Sleep,
}

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

#[derive(Copy, Clone)]
struct TriVert {
    x: f32,
    y: f32,
    u: f32,
    v: f32,
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

fn rasterize_tri(
    out: &mut RgbaImage,
    zbuf: &mut [i8],
    src: &RgbaImage,
    tri: [TriVert; 3],
    depth: i8,
) {
    let (w, h) = out.dimensions();
    let [a, b, c] = tri;
    let min_x = a.x.min(b.x).min(c.x).floor().max(0.0) as i32;
    let max_x = a.x.max(b.x).max(c.x).ceil().min((w as i32 - 1) as f32) as i32;
    let min_y = a.y.min(b.y).min(c.y).floor().max(0.0) as i32;
    let max_y = a.y.max(b.y).max(c.y).ceil().min((h as i32 - 1) as f32) as i32;
    let denom = (b.y - c.y) * (a.x - c.x) + (c.x - b.x) * (a.y - c.y);
    if denom.abs() < 1e-5 {
        return;
    }
    let inv_d = 1.0 / denom;

    for py in min_y..=max_y {
        let cyp = py as f32 + 0.5;
        for px in min_x..=max_x {
            let cxp = px as f32 + 0.5;
            let l1 = ((b.y - c.y) * (cxp - c.x) + (c.x - b.x) * (cyp - c.y)) * inv_d;
            let l2 = ((c.y - a.y) * (cxp - c.x) + (a.x - c.x) * (cyp - c.y)) * inv_d;
            let l3 = 1.0 - l1 - l2;
            if l1 < -0.02 || l2 < -0.02 || l3 < -0.02 {
                continue;
            }
            let d_idx = (py as usize) * (w as usize) + (px as usize);
            if depth < zbuf[d_idx] {
                continue;
            }
            let u = (l1 * a.u + l2 * b.u + l3 * c.u).floor() as i32;
            let v = (l1 * a.v + l2 * b.v + l3 * c.v).floor() as i32;
            let su = u.clamp(0, w as i32 - 1) as u32;
            let sv = v.clamp(0, h as i32 - 1) as u32;
            let spx = *src.get_pixel(su, sv);
            if spx[3] == 0 {
                continue;
            }
            out.put_pixel(px as u32, py as u32, Rgba([spx[0], spx[1], spx[2], 255]));
            zbuf[d_idx] = depth;
        }
    }
}

/// Warp an RGBA sprite using precomputed geodesic weights & hole-free depth-sorted quad rasterization.
pub fn warp_geodesic_mls(
    img: &RgbaImage,
    src_pins: &[RigPin],
    dst_pins: &[RigPin],
    skin: &GeodesicSkinning,
) -> RgbaImage {
    let (w, h) = img.dimensions();
    let mut out = RgbaImage::new(w, h);
    if w == 0 || h == 0 || src_pins.is_empty() || src_pins.len() != dst_pins.len() {
        return out;
    }

    let step: usize = if w.max(h) > 96 { 2 } else { 1 };
    let vw = (w as usize) + 1;
    let vh = (h as usize) + 1;
    let mut vx_arr = vec![0.0f32; vw * vh];
    let mut vy_arr = vec![0.0f32; vw * vh];
    let mut zbuf = vec![i8::MIN; (w as usize) * (h as usize)];

    let mut y = 0usize;
    while y <= h as usize {
        let sy = y.min(h as usize - 1);
        let mut x = 0usize;
        while x <= w as usize {
            let sx = x.min(w as usize - 1);
            let cell_idx = sy * (w as usize) + sx;
            let w_row = &skin.weights[cell_idx * skin.num_pins..(cell_idx + 1) * skin.num_pins];
            let (tx, ty) = eval_rigid_mls_forward(x as f32, y as f32, src_pins, dst_pins, w_row);
            let vi = y * vw + x;
            vx_arr[vi] = tx;
            vy_arr[vi] = ty;
            if x == w as usize {
                break;
            }
            x = (x + step).min(w as usize);
        }
        if y == h as usize {
            break;
        }
        y = (y + step).min(h as usize);
    }

    let mut cy = 0usize;
    while cy < h as usize {
        let y_next = (cy + step).min(h as usize);
        let mut cx = 0usize;
        while cx < w as usize {
            let x_next = (cx + step).min(w as usize);
            let mut sample_idx = None;
            'scan: for ty in cy..y_next {
                for tx in cx..x_next {
                    if img.get_pixel(tx as u32, ty as u32)[3] > 0 {
                        sample_idx = Some(ty * (w as usize) + tx);
                        break 'scan;
                    }
                }
            }
            if let Some(si) = sample_idx {
                let depth = src_pins[skin.dom_pin[si]].depth;
                let i00 = cy * vw + cx;
                let i10 = cy * vw + x_next;
                let i01 = y_next * vw + cx;
                let i11 = y_next * vw + x_next;
                let v00 = TriVert {
                    x: vx_arr[i00],
                    y: vy_arr[i00],
                    u: cx as f32,
                    v: cy as f32,
                };
                let v10 = TriVert {
                    x: vx_arr[i10],
                    y: vy_arr[i10],
                    u: x_next as f32,
                    v: cy as f32,
                };
                let v01 = TriVert {
                    x: vx_arr[i01],
                    y: vy_arr[i01],
                    u: cx as f32,
                    v: y_next as f32,
                };
                let v11 = TriVert {
                    x: vx_arr[i11],
                    y: vy_arr[i11],
                    u: x_next as f32,
                    v: y_next as f32,
                };
                rasterize_tri(&mut out, &mut zbuf, img, [v00, v10, v11], depth);
                rasterize_tri(&mut out, &mut zbuf, img, [v00, v11, v01], depth);
            }
            cx = x_next;
        }
        cy = y_next;
    }

    out
}

/// Generate a complete multi-frame animation cycle for any of the 5 creature archetypes & 5 FX.
pub fn generate_rigged_cycle(
    img: &RgbaImage,
    archetype: RigArchetype,
    fx: RigFx,
    total_frames: usize,
) -> Vec<RgbaImage> {
    let frames_n = total_frames.clamp(2, 16);
    let (w, h) = img.dimensions();
    let (mut min_x, mut max_x, mut min_y, mut max_y) = (w, 0u32, h, 0u32);
    for (x, y, p) in img.enumerate_pixels() {
        if p[3] > 24 {
            min_x = min_x.min(x);
            max_x = max_x.max(x);
            min_y = min_y.min(y);
            max_y = max_y.max(y);
        }
    }
    if max_x <= min_x + 4 || max_y <= min_y + 4 {
        return vec![img.clone(); frames_n];
    }

    let bw = (max_x - min_x) as f32;
    let bh = (max_y - min_y) as f32;
    let mx = min_x as f32;
    let my = min_y as f32;
    let paw_y = my + bh * 0.94;

    let src_pins: Vec<RigPin> = match archetype {
        RigArchetype::Quadruped => vec![
            RigPin {
                x: mx + bw * 0.20,
                y: my + bh * 0.30,
                depth: 4,
            }, // 0: head
            RigPin {
                x: mx + bw * 0.42,
                y: my + bh * 0.46,
                depth: 3,
            }, // 1: chest
            RigPin {
                x: mx + bw * 0.68,
                y: my + bh * 0.50,
                depth: 3,
            }, // 2: pelvis
            RigPin {
                x: mx + bw * 0.92,
                y: my + bh * 0.44,
                depth: 2,
            }, // 3: tail
            RigPin {
                x: mx + bw * 0.20,
                y: paw_y,
                depth: 5,
            }, // 4: fl
            RigPin {
                x: mx + bw * 0.40,
                y: paw_y,
                depth: 1,
            }, // 5: fr
            RigPin {
                x: mx + bw * 0.66,
                y: paw_y,
                depth: 1,
            }, // 6: bl
            RigPin {
                x: mx + bw * 0.85,
                y: paw_y,
                depth: 5,
            }, // 7: br
        ],
        RigArchetype::Biped => vec![
            RigPin {
                x: mx + bw * 0.50,
                y: my + bh * 0.20,
                depth: 4,
            }, // 0: head
            RigPin {
                x: mx + bw * 0.50,
                y: my + bh * 0.44,
                depth: 3,
            }, // 1: chest
            RigPin {
                x: mx + bw * 0.50,
                y: my + bh * 0.62,
                depth: 3,
            }, // 2: pelvis
            RigPin {
                x: mx + bw * 0.22,
                y: my + bh * 0.54,
                depth: 6,
            }, // 3: l_hand
            RigPin {
                x: mx + bw * 0.78,
                y: my + bh * 0.54,
                depth: 1,
            }, // 4: r_hand
            RigPin {
                x: mx + bw * 0.35,
                y: paw_y,
                depth: 5,
            }, // 5: l_foot
            RigPin {
                x: mx + bw * 0.65,
                y: paw_y,
                depth: 1,
            }, // 6: r_foot
        ],
        RigArchetype::Crawler => (0..5)
            .map(|i| RigPin {
                x: mx + bw * (0.12 + (i as f32 / 4.0) * 0.78),
                y: my + bh * 0.65,
                depth: if i == 0 { 4 } else { 3 },
            })
            .collect(),
        RigArchetype::Swimmer => vec![
            RigPin {
                x: mx + bw * 0.16,
                y: my + bh * 0.50,
                depth: 4,
            }, // 0: head
            RigPin {
                x: mx + bw * 0.38,
                y: my + bh * 0.50,
                depth: 3,
            }, // 1: chest
            RigPin {
                x: mx + bw * 0.64,
                y: my + bh * 0.50,
                depth: 3,
            }, // 2: pelvis
            RigPin {
                x: mx + bw * 0.90,
                y: my + bh * 0.50,
                depth: 2,
            }, // 3: caudal tail
        ],
        RigArchetype::Bird => vec![
            RigPin {
                x: mx + bw * 0.24,
                y: my + bh * 0.32,
                depth: 4,
            }, // 0: head
            RigPin {
                x: mx + bw * 0.48,
                y: my + bh * 0.50,
                depth: 3,
            }, // 1: torso
            RigPin {
                x: mx + bw * 0.86,
                y: my + bh * 0.58,
                depth: 2,
            }, // 2: tail
            RigPin {
                x: mx + bw * 0.52,
                y: my + bh * 0.42,
                depth: 6,
            }, // 3: wing
            RigPin {
                x: mx + bw * 0.50,
                y: paw_y,
                depth: 5,
            }, // 4: feet
        ],
    };

    let skin = compute_geodesic_skinning(img, &src_pins);
    let stride = (bw * 0.08).clamp(2.0, 9.0);
    let lift = (bh * 0.075).clamp(1.5, 7.0);
    let mut frames = Vec::with_capacity(frames_n);

    for fi in 0..frames_n {
        let phase = (fi as f32 / frames_n as f32) * std::f32::consts::TAU;
        let mut dst = src_pins.clone();

        match fx {
            RigFx::Walk | RigFx::Sprint => {
                let mult = if fx == RigFx::Sprint { 1.45 } else { 1.0 };
                let sa = phase.sin();
                let sb = (phase + std::f32::consts::PI).sin();
                let la = phase.cos().max(0.0);
                let lb = (phase + std::f32::consts::PI).cos().max(0.0);
                let bob = -(phase.cos().abs()) * (bh * 0.028).max(0.8) * mult;
                match archetype {
                    RigArchetype::Quadruped => {
                        dst[0].y += bob * 1.1;
                        dst[1].y += bob;
                        dst[2].y += bob * 0.9;
                        dst[3].y += bob + (phase * 2.0).sin() * 1.4 * mult;
                        dst[4].x -= sa * stride * mult;
                        dst[4].y -= la * lift * mult;
                        dst[7].x -= sa * stride * mult;
                        dst[7].y -= la * lift * mult;
                        dst[5].x -= sb * stride * mult;
                        dst[5].y -= lb * lift * mult;
                        dst[6].x -= sb * stride * mult;
                        dst[6].y -= lb * lift * mult;
                    }
                    RigArchetype::Biped => {
                        dst[0].y += bob * 1.1;
                        dst[1].y += bob;
                        dst[2].y += bob * 0.9;
                        dst[3].x -= sb * stride * 0.85 * mult;
                        dst[4].x -= sa * stride * 0.85 * mult;
                        dst[5].x -= sa * stride * mult;
                        dst[5].y -= la * lift * mult;
                        dst[6].x -= sb * stride * mult;
                        dst[6].y -= lb * lift * mult;
                    }
                    RigArchetype::Crawler => {
                        for (i, pin) in dst.iter_mut().enumerate() {
                            let amp = if i == 0 {
                                0.5
                            } else {
                                (bh * 0.14).max(1.2) * (0.45 + 0.55 * (i as f32 / 4.0)) * mult
                            };
                            pin.y -= (phase - i as f32 * 1.05).sin().abs() * amp;
                            pin.x -=
                                (phase - i as f32 * 1.05).cos() * if i == 0 { 0.5 } else { 1.1 };
                        }
                    }
                    RigArchetype::Swimmer => {
                        let buoy = phase.sin() * (bh * 0.04).max(0.8);
                        dst[0].y += buoy * 0.5;
                        dst[1].y += buoy * 0.8 + (phase - 0.5).sin() * 0.5;
                        dst[2].y += buoy + (phase - 1.2).sin() * (bh * 0.07).max(1.4) * mult;
                        dst[3].y += buoy + (phase - 2.1).sin() * (bh * 0.16).max(2.6) * mult;
                    }
                    RigArchetype::Bird => {
                        let flap = phase.sin();
                        let body_lift = -flap * (bh * 0.06).max(1.4) * mult;
                        dst[0].y += body_lift;
                        dst[1].y += body_lift;
                        dst[2].y += body_lift - flap * 1.4;
                        dst[3].y += flap * (bh * 0.22).max(3.0) * mult;
                        dst[4].y += body_lift;
                    }
                }
            }
            RigFx::Dance => {
                let sway = phase.sin() * (bw * 0.045).max(1.6);
                let bounce = -(phase * 2.0).sin().abs() * (bh * 0.05).max(1.4);
                for (i, pin) in dst.iter_mut().enumerate() {
                    pin.x += if i % 2 == 0 { sway } else { -sway * 0.6 };
                    pin.y += bounce;
                }
            }
            RigFx::Pet => {
                let squish = phase.sin().max(0.0);
                dst[0].y += squish * (bh * 0.075).max(2.0);
                if dst.len() > 1 {
                    dst[1].y += squish * (bh * 0.04).max(1.0);
                }
                if let Some(last) = dst.last_mut() {
                    last.y += (phase * 4.0).sin() * 1.6;
                }
            }
            RigFx::Sleep => {
                let br = phase.sin();
                dst[0].y += (bh * 0.04).max(1.2) + br * 0.6;
                if dst.len() > 1 {
                    dst[1].y -= br * (bh * 0.032).max(1.0);
                }
            }
        }

        frames.push(warp_geodesic_mls(img, &src_pins, &dst, &skin));
    }

    if matches!(archetype, RigArchetype::Bird | RigArchetype::Swimmer) || fx == RigFx::Sprint {
        frames
    } else {
        crate::pixel_core::align_foot_baseline(&frames)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn geodesic_skinning_isolates_legs_and_all_archetypes_animate() {
        let mut img = RgbaImage::new(40, 40);
        for y in 12..34 {
            for x in 8..14 {
                img.put_pixel(x, y, Rgba([240, 220, 200, 255]));
            }
            for x in 24..30 {
                img.put_pixel(x, y, Rgba([200, 180, 160, 255]));
            }
        }
        for y in 8..16 {
            for x in 8..30 {
                img.put_pixel(x, y, Rgba([220, 200, 180, 255]));
            }
        }
        let pins = [
            RigPin {
                x: 11.0,
                y: 32.0,
                depth: 5,
            },
            RigPin {
                x: 27.0,
                y: 32.0,
                depth: 1,
            },
        ];
        let skin = compute_geodesic_skinning(&img, &pins);
        let left_idx = 32 * 40 + 11;
        assert_eq!(skin.dom_pin[left_idx], 0);
        assert!(skin.weights[left_idx * 2] > 0.90);

        for arch in [
            RigArchetype::Quadruped,
            RigArchetype::Biped,
            RigArchetype::Crawler,
            RigArchetype::Swimmer,
            RigArchetype::Bird,
        ] {
            let frames = generate_rigged_cycle(&img, arch, RigFx::Walk, 6);
            assert_eq!(frames.len(), 6);
            let diff = frames[0]
                .pixels()
                .zip(frames[2].pixels())
                .filter(|(a, b)| a[3] != b[3])
                .count();
            assert!(diff > 5, "archetype {arch:?} did not animate");
        }
    }
}
