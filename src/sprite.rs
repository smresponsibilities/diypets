use image::{Rgba, load_from_memory};

pub struct DecodedSprite {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u32>,
}

pub fn decode_sprite_png(data: &[u8]) -> Result<DecodedSprite, String> {
    let img = load_from_memory(data).map_err(|e| format!("failed to decode PNG: {e}"))?;
    let rgba = img.to_rgba8();
    let width = rgba.width();
    let height = rgba.height();
    let pixels: Vec<u32> = rgba
        .pixels()
        .map(|&Rgba([r, g, b, a])| (a as u32) << 24 | (r as u32) << 16 | (g as u32) << 8 | b as u32)
        .collect();
    Ok(DecodedSprite {
        width,
        height,
        pixels,
    })
}

/// Split a horizontal strip into `frame_count` frames of `frame_w` x `frame_h`.
///
/// Used once at startup so the render loop never touches the disk.
pub fn slice_strip(
    strip: &DecodedSprite,
    frame_w: u32,
    frame_h: u32,
    frame_count: usize,
) -> Result<Vec<DecodedSprite>, String> {
    if frame_w == 0 || frame_h == 0 || frame_count == 0 {
        return Err("frame width, frame height, and frame count must be greater than zero".into());
    }
    let needed_w = frame_w as u64 * frame_count as u64;
    if needed_w > strip.width as u64 || frame_h > strip.height {
        return Err(format!(
            "strip is {}x{}, too small for {frame_count} frames of {frame_w}x{frame_h}",
            strip.width, strip.height
        ));
    }

    let mut frames = Vec::with_capacity(frame_count);
    for index in 0..frame_count {
        let frame_x = index as u32 * frame_w;
        let mut pixels = Vec::with_capacity((frame_w * frame_h) as usize);
        for y in 0..frame_h {
            let row = (y * strip.width + frame_x) as usize;
            pixels.extend_from_slice(&strip.pixels[row..row + frame_w as usize]);
        }
        frames.push(DecodedSprite {
            width: frame_w,
            height: frame_h,
            pixels,
        });
    }
    Ok(frames)
}

/// Rotate a frame 90 degrees clockwise. Square frames stay square, so the
/// pet window does not need to resize when the pet scales a wall.
pub fn rotate_quarter_turn(sprite: &DecodedSprite) -> DecodedSprite {
    let width = sprite.width;
    let height = sprite.height;
    let mut pixels = vec![0u32; (width * height) as usize];

    // Source (x, y) lands at (height - 1 - y, x) for a clockwise quarter turn.
    for y in 0..height {
        for x in 0..width {
            let target_x = height - 1 - y;
            let target_y = x;
            pixels[(target_y * height + target_x) as usize] =
                sprite.pixels[(y * width + x) as usize];
        }
    }

    DecodedSprite {
        width: height,
        height: width,
        pixels,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{Rgba, RgbaImage};
    use std::io::Cursor;

    #[test]
    fn decode_sprite_png_roundtrip() {
        let mut img = RgbaImage::new(2, 2);
        img.put_pixel(0, 0, Rgba([255u8, 0, 0, 255]));
        img.put_pixel(1, 0, Rgba([0, 255, 0, 255]));
        img.put_pixel(0, 1, Rgba([0, 0, 255, 255]));
        img.put_pixel(1, 1, Rgba([255, 255, 0, 255]));

        let mut png_bytes: Vec<u8> = Vec::new();
        img.write_to(&mut Cursor::new(&mut png_bytes), image::ImageFormat::Png)
            .expect("encode PNG");

        let sprite = decode_sprite_png(&png_bytes).expect("decode should succeed");

        assert_eq!(sprite.width, 2);
        assert_eq!(sprite.height, 2);
        assert_eq!(sprite.pixels.len(), 4);

        let expected: Vec<u32> = vec![
            0xFF_FF_00_00, // red:  A=FF R=FF G=00 B=00
            0xFF_00_FF_00, // green: A=FF R=00 G=FF B=00
            0xFF_00_00_FF, // blue: A=FF R=00 G=00 B=FF
            0xFF_FF_FF_00, // yellow: A=FF R=FF G=FF B=00
        ];
        assert_eq!(sprite.pixels, expected);
    }

    #[test]
    fn slice_strip_splits_frames_left_to_right() {
        let mut img = RgbaImage::new(4, 2);
        for y in 0..2 {
            for x in 0..4 {
                let color = if x < 2 {
                    Rgba([255u8, 0, 0, 255])
                } else {
                    Rgba([0, 0, 255, 255])
                };
                img.put_pixel(x, y, color);
            }
        }
        let mut png_bytes: Vec<u8> = Vec::new();
        img.write_to(&mut Cursor::new(&mut png_bytes), image::ImageFormat::Png)
            .expect("encode PNG");
        let strip = decode_sprite_png(&png_bytes).expect("decode strip");

        let frames = slice_strip(&strip, 2, 2, 2).expect("slice should succeed");
        assert_eq!(frames.len(), 2);
        assert_eq!(frames[0].width, 2);
        assert_eq!(frames[0].height, 2);
        assert_eq!(frames[0].pixels, vec![0xFF_FF_00_00; 4]);
        assert_eq!(frames[1].pixels, vec![0xFF_00_00_FF; 4]);
    }

    #[test]
    fn rotate_quarter_turn_moves_the_top_row_to_the_right_column() {
        let strip = DecodedSprite {
            width: 2,
            height: 2,
            // top row red and green, bottom row blue and white
            pixels: vec![
                0xFF_FF_00_00,
                0xFF_00_FF_00, //
                0xFF_00_00_FF,
                0xFF_FF_FF_FF,
            ],
        };

        let turned = rotate_quarter_turn(&strip);
        assert_eq!((turned.width, turned.height), (2, 2), "square stays square");
        assert_eq!(
            turned.pixels,
            vec![
                0xFF_00_00_FF,
                0xFF_FF_00_00, // green, red
                0xFF_FF_FF_FF,
                0xFF_00_FF_00, // white, blue
            ]
        );
    }

    #[test]
    fn four_quarter_turns_return_the_original_frame() {
        let strip = DecodedSprite {
            width: 3,
            height: 3,
            pixels: (0..9u32).map(|i| 0xFF00_0000 | i).collect(),
        };

        let mut current = rotate_quarter_turn(&strip);
        for _ in 0..3 {
            current = rotate_quarter_turn(&current);
        }
        assert_eq!(current.pixels, strip.pixels, "no pixel is lost or moved");
        assert_eq!((current.width, current.height), (3, 3));
    }

    #[test]
    fn rotate_quarter_turn_transposes_a_non_square_frame() {
        let strip = DecodedSprite {
            width: 3,
            height: 1,
            pixels: vec![1, 2, 3],
        };

        let turned = rotate_quarter_turn(&strip);
        assert_eq!((turned.width, turned.height), (1, 3), "dimensions swap");
        // A clockwise turn stacks the row top to bottom, leftmost pixel first.
        assert_eq!(turned.pixels, vec![1, 2, 3]);
    }

    #[test]
    fn slice_strip_rejects_frame_count_larger_than_strip() {
        let strip = DecodedSprite {
            width: 4,
            height: 2,
            pixels: vec![0xFF_FF_00_00; 8],
        };
        let result = slice_strip(&strip, 2, 2, 3);
        assert!(result.is_err(), "3 frames do not fit in a 4x2 strip");
    }

    #[test]
    fn slice_strip_rejects_zero_sized_frames() {
        let strip = DecodedSprite {
            width: 4,
            height: 2,
            pixels: vec![0xFF_FF_00_00; 8],
        };
        assert!(slice_strip(&strip, 0, 2, 2).is_err());
        assert!(slice_strip(&strip, 2, 0, 2).is_err());
        assert!(slice_strip(&strip, 2, 2, 0).is_err());
    }
}
