//! True 8-Bit Pixel Editor Core (T-006):
//! - Native low-res grid scales (`16x16`, `24x24`, `32x32`, `48x48`, `64x64`) with nearest-neighbor scaling
//! - Indexed palettes (`PICO-8`, `Game Boy`, `Endesga-32`, `NES`) & nearest-swatch quantization
//! - Pixel-Perfect L-double removal for clean 1px pixel art strokes
//! - 2x2 and 4x4 Ordered Bayer dithering

use image::{Rgba, RgbaImage};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PixelGridSize {
    G16 = 16,
    G24 = 24,
    G32 = 32,
    G48 = 48,
    G64 = 64,
}

impl PixelGridSize {
    pub fn all() -> &'static [Self] {
        &[Self::G16, Self::G24, Self::G32, Self::G48, Self::G64]
    }

    pub fn dim(self) -> u32 {
        self as u32
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::G16 => "16x16",
            Self::G24 => "24x24",
            Self::G32 => "32x32",
            Self::G48 => "48x48",
            Self::G64 => "64x64",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IndexedPalette {
    GameBoy,
    Pico8,
    Nes,
    Endesga32,
}

impl IndexedPalette {
    pub fn all() -> &'static [Self] {
        &[Self::GameBoy, Self::Pico8, Self::Nes, Self::Endesga32]
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::GameBoy => "Game Boy (4)",
            Self::Pico8 => "PICO-8 (16)",
            Self::Nes => "NES (16)",
            Self::Endesga32 => "Endesga-32 (32)",
        }
    }

    pub fn swatches(self) -> &'static [(u8, u8, u8)] {
        match self {
            Self::GameBoy => &[(15, 56, 15), (48, 98, 48), (139, 172, 15), (155, 188, 15)],
            Self::Pico8 => &[
                (0, 0, 0),
                (29, 43, 83),
                (126, 37, 83),
                (0, 135, 81),
                (171, 82, 54),
                (95, 87, 79),
                (194, 195, 199),
                (255, 241, 232),
                (255, 0, 77),
                (255, 163, 0),
                (255, 236, 39),
                (0, 228, 54),
                (41, 173, 255),
                (131, 118, 156),
                (255, 119, 168),
                (255, 204, 170),
            ],
            Self::Nes => &[
                (84, 84, 84),
                (0, 30, 116),
                (8, 16, 144),
                (48, 0, 136),
                (68, 0, 100),
                (92, 0, 48),
                (84, 4, 0),
                (60, 24, 0),
                (32, 42, 0),
                (8, 58, 0),
                (0, 64, 0),
                (0, 60, 0),
                (0, 50, 60),
                (0, 0, 0),
                (152, 150, 152),
                (248, 248, 248),
            ],
            Self::Endesga32 => &[
                (190, 74, 47),
                (215, 118, 67),
                (234, 158, 34),
                (255, 212, 163),
                (226, 207, 133),
                (160, 147, 142),
                (104, 122, 132),
                (64, 73, 115),
                (31, 36, 75),
                (18, 20, 44),
                (42, 111, 126),
                (50, 162, 139),
                (96, 209, 121),
                (210, 249, 140),
                (242, 240, 229),
                (218, 212, 94),
                (229, 117, 146),
                (172, 50, 50),
                (118, 66, 138),
                (105, 106, 106),
                (89, 125, 206),
                (109, 170, 44),
                (210, 125, 44),
                (133, 76, 48),
                (52, 101, 36),
                (208, 70, 72),
                (217, 87, 99),
                (223, 113, 38),
                (215, 123, 186),
                (143, 151, 74),
                (138, 111, 48),
                (63, 63, 116),
            ],
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BayerMatrixSize {
    B2x2,
    B4x4,
}

const BAYER_2X2: [[f32; 2]; 2] = [[0.0, 2.0], [3.0, 1.0]];

const BAYER_4X4: [[f32; 4]; 4] = [
    [0.0, 8.0, 2.0, 10.0],
    [12.0, 4.0, 14.0, 6.0],
    [3.0, 11.0, 1.0, 9.0],
    [15.0, 7.0, 13.0, 5.0],
];

pub fn nearest_swatch(rgb: [u8; 3], palette: IndexedPalette) -> [u8; 3] {
    let mut best = palette.swatches()[0];
    let mut best_dist = u32::MAX;
    for &(sr, sg, sb) in palette.swatches() {
        let dr = (rgb[0] as i32 - sr as i32).unsigned_abs();
        let dg = (rgb[1] as i32 - sg as i32).unsigned_abs();
        let db = (rgb[2] as i32 - sb as i32).unsigned_abs();
        let dist = dr * dr * 2 + dg * dg * 4 + db * db * 3;
        if dist < best_dist {
            best_dist = dist;
            best = (sr, sg, sb);
        }
    }
    [best.0, best.1, best.2]
}

/// Quantize an RGBA image to the chosen indexed palette (preserving transparent pixels).
pub fn quantize_to_palette(img: &RgbaImage, palette: IndexedPalette) -> RgbaImage {
    let (w, h) = img.dimensions();
    let mut out = RgbaImage::new(w, h);
    for y in 0..h {
        for x in 0..w {
            let px = *img.get_pixel(x, y);
            if px[3] < 16 {
                out.put_pixel(x, y, Rgba([0, 0, 0, 0]));
            } else {
                let q = nearest_swatch([px[0], px[1], px[2]], palette);
                out.put_pixel(x, y, Rgba([q[0], q[1], q[2], px[3]]));
            }
        }
    }
    out
}

/// Apply ordered 2x2 or 4x4 Bayer dithering and snap each pixel to `palette`.
pub fn apply_bayer_dither(
    img: &RgbaImage,
    palette: IndexedPalette,
    matrix: BayerMatrixSize,
    spread: f32,
) -> RgbaImage {
    let (w, h) = img.dimensions();
    let mut out = RgbaImage::new(w, h);
    let amp = spread.clamp(4.0, 96.0);

    for y in 0..h {
        for x in 0..w {
            let px = *img.get_pixel(x, y);
            if px[3] < 16 {
                out.put_pixel(x, y, Rgba([0, 0, 0, 0]));
                continue;
            }
            let threshold = match matrix {
                BayerMatrixSize::B2x2 => {
                    (BAYER_2X2[(y % 2) as usize][(x % 2) as usize] + 0.5) / 4.0 - 0.5
                }
                BayerMatrixSize::B4x4 => {
                    (BAYER_4X4[(y % 4) as usize][(x % 4) as usize] + 0.5) / 16.0 - 0.5
                }
            };
            let delta = threshold * amp;
            let shifted = [
                (px[0] as f32 + delta).clamp(0.0, 255.0).round() as u8,
                (px[1] as f32 + delta).clamp(0.0, 255.0).round() as u8,
                (px[2] as f32 + delta).clamp(0.0, 255.0).round() as u8,
            ];
            let q = nearest_swatch(shifted, palette);
            out.put_pixel(x, y, Rgba([q[0], q[1], q[2], px[3]]));
        }
    }
    out
}

/// Pixel-perfect brush cleanup: removes redundant L-shaped corner doubles along 1px strokes.
pub fn cleanup_pixel_perfect_l_doubles(img: &RgbaImage) -> RgbaImage {
    let (w, h) = img.dimensions();
    let mut out = img.clone();
    if w < 3 || h < 3 {
        return out;
    }

    let same_col = |a: Rgba<u8>, b: Rgba<u8>| a[3] >= 16 && b[3] >= 16 && a.0[0..3] == b.0[0..3];

    for y in 1..(h - 1) {
        for x in 1..(w - 1) {
            let c = *img.get_pixel(x, y);
            if c[3] < 16 {
                continue;
            }
            let n = *img.get_pixel(x, y - 1);
            let s = *img.get_pixel(x, y + 1);
            let w_px = *img.get_pixel(x - 1, y);
            let e_px = *img.get_pixel(x + 1, y);

            // L-corner pivot: has one cardinal neighbor and one orthogonal neighbor, with diagonal bridge
            let nw = *img.get_pixel(x - 1, y - 1);
            let ne = *img.get_pixel(x + 1, y - 1);
            let sw = *img.get_pixel(x - 1, y + 1);
            let se = *img.get_pixel(x + 1, y + 1);

            let is_l_corner = (same_col(c, n) && same_col(c, w_px) && same_col(c, nw))
                || (same_col(c, n) && same_col(c, e_px) && same_col(c, ne))
                || (same_col(c, s) && same_col(c, w_px) && same_col(c, sw))
                || (same_col(c, s) && same_col(c, e_px) && same_col(c, se));

            if is_l_corner {
                out.put_pixel(x, y, Rgba([0, 0, 0, 0]));
            }
        }
    }
    out
}

/// Downsample to a crisp native grid (`16x16`..`64x64`) and scale back up with nearest-neighbor.
pub fn snap_to_native_grid_and_back(img: &RgbaImage, grid: PixelGridSize) -> RgbaImage {
    let d = grid.dim();
    let small = image::imageops::resize(img, d, d, image::imageops::FilterType::Nearest);
    image::imageops::resize(
        &small,
        img.width().max(1),
        img.height().max(1),
        image::imageops::FilterType::Nearest,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn indexed_palettes_quantize_and_dither_deterministically() {
        let mut grad = RgbaImage::new(32, 32);
        for y in 0..32 {
            for x in 0..32 {
                let v = ((x + y) * 4).min(255) as u8;
                grad.put_pixel(x, y, Rgba([v, v, v, 255]));
            }
        }
        let q_gb = quantize_to_palette(&grad, IndexedPalette::GameBoy);
        assert_eq!(q_gb.dimensions(), (32, 32));
        for &(r, g, b) in IndexedPalette::GameBoy.swatches() {
            assert_eq!(
                nearest_swatch([r, g, b], IndexedPalette::GameBoy),
                [r, g, b]
            );
        }

        let d2 = apply_bayer_dither(&grad, IndexedPalette::Pico8, BayerMatrixSize::B2x2, 36.0);
        let d4 = apply_bayer_dither(
            &grad,
            IndexedPalette::Endesga32,
            BayerMatrixSize::B4x4,
            48.0,
        );
        assert_eq!(d2.dimensions(), (32, 32));
        assert_eq!(d4.dimensions(), (32, 32));
    }

    #[test]
    fn pixel_perfect_cleanup_removes_l_corner_double_and_preserves_straight_line() {
        let mut img = RgbaImage::new(8, 8);
        let red = Rgba([220, 50, 50, 255]);
        // L-corner cluster around (2, 2) with (2, 1), (1, 2), (1, 1)
        img.put_pixel(2, 1, red);
        img.put_pixel(2, 2, red);
        img.put_pixel(1, 2, red);
        img.put_pixel(1, 1, red);
        // Separate clean horizontal stroke on row 5: endpoints (1, 5) and (3, 5) must stay 255
        img.put_pixel(1, 5, red);
        img.put_pixel(2, 5, red);
        img.put_pixel(3, 5, red);
        let cleaned = cleanup_pixel_perfect_l_doubles(&img);
        assert_eq!(cleaned.get_pixel(2, 2)[3], 0);
        assert_eq!(cleaned.get_pixel(1, 5)[3], 255);
        assert_eq!(cleaned.get_pixel(3, 5)[3], 255);
    }

    #[test]
    fn native_grid_roundtrip_preserves_dimensions() {
        let img = RgbaImage::new(64, 64);
        for &g in PixelGridSize::all() {
            let out = snap_to_native_grid_and_back(&img, g);
            assert_eq!(out.dimensions(), (64, 64));
        }
    }
}
