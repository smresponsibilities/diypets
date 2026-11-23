//! RotSprite pixel-art rotation (EPX / Scale2x upscale -> rotate -> nearest downsample).
//! Preserves crisp 1px pixel-art outlines without bilinear blur or jagged 1x nearest tearing.

/// Single EPX (Scale2x) 2x upscale pass on an RGBA8 buffer.
pub fn epx_2x(src: &[u8], w: usize, h: usize) -> (usize, usize, Vec<u8>) {
    let nw = w * 2;
    let nh = h * 2;
    let mut dst = vec![0u8; nw * nh * 4];
    if w == 0 || h == 0 {
        return (nw, nh, dst);
    }

    let px = |x: usize, y: usize| -> [u8; 4] {
        let i = (y * w + x) * 4;
        [src[i], src[i + 1], src[i + 2], src[i + 3]]
    };

    for y in 0..h {
        let ym = y.saturating_sub(1);
        let yp = (y + 1).min(h - 1);
        for x in 0..w {
            let xm = x.saturating_sub(1);
            let xp = (x + 1).min(w - 1);

            let p = px(x, y);
            let a = px(x, ym); // top
            let b = px(xp, y); // right
            let c = px(xm, y); // left
            let d = px(x, yp); // bottom

            let mut p1 = p;
            let mut p2 = p;
            let mut p3 = p;
            let mut p4 = p;

            if a != d && c != b {
                if c == a {
                    p1 = a;
                }
                if a == b {
                    p2 = b;
                }
                if d == c {
                    p3 = c;
                }
                if b == d {
                    p4 = d;
                }
            }

            let mut put = |dx: usize, dy: usize, col: [u8; 4]| {
                let i = (dy * nw + dx) * 4;
                dst[i..i + 4].copy_from_slice(&col);
            };
            put(x * 2, y * 2, p1);
            put(x * 2 + 1, y * 2, p2);
            put(x * 2, y * 2 + 1, p3);
            put(x * 2 + 1, y * 2 + 1, p4);
        }
    }
    (nw, nh, dst)
}

/// Rotate an RGBA8 sprite around `(pivot_x, pivot_y)` by `angle_rad` using 4x EPX RotSprite.
/// Returns a same-sized `(w, h)` RGBA8 buffer with crisp pixel-art edges.
pub fn rotsprite_rgba(
    src: &[u8],
    w: usize,
    h: usize,
    pivot_x: f32,
    pivot_y: f32,
    angle_rad: f32,
) -> Vec<u8> {
    if angle_rad.abs() < 1e-4 || w == 0 || h == 0 {
        return src.to_vec();
    }

    // Two EPX passes -> 4x crisp pixel-art upscale
    let (w2, h2, up2) = epx_2x(src, w, h);
    let (w4, h4, up4) = epx_2x(&up2, w2, h2);

    let cos_a = (-angle_rad).cos();
    let sin_a = (-angle_rad).sin();
    let p4x = pivot_x * 4.0;
    let p4y = pivot_y * 4.0;

    let mut out = vec![0u8; w * h * 4];
    for y in 0..h {
        let dy = (y as f32 + 0.5) * 4.0 - p4y;
        for x in 0..w {
            let dx = (x as f32 + 0.5) * 4.0 - p4x;
            let sx = (dx * cos_a - dy * sin_a + p4x).floor() as isize;
            let sy = (dx * sin_a + dy * cos_a + p4y).floor() as isize;
            if sx >= 0 && sx < w4 as isize && sy >= 0 && sy < h4 as isize {
                let si = (sy as usize * w4 + sx as usize) * 4;
                let di = (y * w + x) * 4;
                out[di..di + 4].copy_from_slice(&up4[si..si + 4]);
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rotsprite_identity_preserves_exact_pixels() {
        let mut src = vec![0u8; 8 * 8 * 4];
        src[(3 * 8 + 3) * 4..((3 * 8 + 3) * 4 + 4)].copy_from_slice(&[255, 100, 40, 255]);
        let out = rotsprite_rgba(&src, 8, 8, 4.0, 4.0, 0.0);
        assert_eq!(out, src);
    }

    #[test]
    fn rotsprite_rotation_keeps_crisp_opaque_alpha_without_blur() {
        let mut src = vec![0u8; 16 * 16 * 4];
        for y in 5..11 {
            for x in 5..11 {
                let i = (y * 16 + x) * 4;
                src[i..i + 4].copy_from_slice(&[240, 120, 40, 255]);
            }
        }
        let rot = rotsprite_rgba(&src, 16, 16, 8.0, 8.0, 0.35);
        let opaque_count = rot
            .as_chunks::<4>()
            .0
            .iter()
            .filter(|p| p[3] == 255)
            .count();
        let semi_count = rot
            .as_chunks::<4>()
            .0
            .iter()
            .filter(|p| p[3] > 0 && p[3] < 255)
            .count();
        assert!(
            opaque_count >= 24,
            "rotated square should retain solid area"
        );
        assert_eq!(
            semi_count, 0,
            "RotSprite must produce 0 blurred semi-alpha pixels"
        );
    }
}
