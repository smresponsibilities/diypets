//! Pure-Rust crisp pixel-art floating speech bubble renderer (premultiplied ARGB `u32`).
//!
//! Used by MCP / HTTP / CLI agent reactions (`diypets react --mood happy --text "Tests passed!"`)
//! and pet headpats.

fn glyph_5x7(ch: char) -> [u8; 7] {
    match ch.to_ascii_uppercase() {
        'A' => [0b01110, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001],
        'B' => [0b11110, 0b10001, 0b10001, 0b11110, 0b10001, 0b10001, 0b11110],
        'C' => [0b01110, 0b10001, 0b10000, 0b10000, 0b10000, 0b10001, 0b01110],
        'D' => [0b11100, 0b10010, 0b10001, 0b10001, 0b10001, 0b10010, 0b11100],
        'E' => [0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b11111],
        'F' => [0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b10000],
        'G' => [0b01110, 0b10001, 0b10000, 0b10111, 0b10001, 0b10001, 0b01110],
        'H' => [0b10001, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001],
        'I' => [0b01110, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110],
        'J' => [0b00111, 0b00010, 0b00010, 0b00010, 0b10010, 0b10010, 0b01100],
        'K' => [0b10001, 0b10010, 0b10100, 0b11000, 0b10100, 0b10010, 0b10001],
        'L' => [0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b11111],
        'M' => [0b10001, 0b11011, 0b10101, 0b10101, 0b10001, 0b10001, 0b10001],
        'N' => [0b10001, 0b11001, 0b10101, 0b10011, 0b10001, 0b10001, 0b10001],
        'O' => [0b01110, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110],
        'P' => [0b11110, 0b10001, 0b10001, 0b11110, 0b10000, 0b10000, 0b10000],
        'Q' => [0b01110, 0b10001, 0b10001, 0b10001, 0b10101, 0b10010, 0b01101],
        'R' => [0b11110, 0b10001, 0b10001, 0b11110, 0b10100, 0b10010, 0b10001],
        'S' => [0b01111, 0b10000, 0b10000, 0b01110, 0b00001, 0b00001, 0b11110],
        'T' => [0b11111, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100],
        'U' => [0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110],
        'V' => [0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01010, 0b00100],
        'W' => [0b10001, 0b10001, 0b10001, 0b10101, 0b10101, 0b11011, 0b10001],
        'X' => [0b10001, 0b10001, 0b01010, 0b00100, 0b01010, 0b10001, 0b10001],
        'Y' => [0b10001, 0b10001, 0b01010, 0b00100, 0b00100, 0b00100, 0b00100],
        'Z' => [0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b10000, 0b11111],
        '0' => [0b01110, 0b10001, 0b10011, 0b10101, 0b11001, 0b10001, 0b01110],
        '1' => [0b00100, 0b01100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110],
        '2' => [0b01110, 0b10001, 0b00001, 0b00110, 0b01000, 0b10000, 0b11111],
        '3' => [0b11110, 0b00001, 0b00001, 0b01110, 0b00001, 0b00001, 0b11110],
        '4' => [0b00010, 0b00110, 0b01010, 0b10010, 0b11111, 0b00010, 0b00010],
        '5' => [0b11111, 0b10000, 0b11110, 0b00001, 0b00001, 0b10001, 0b01110],
        '6' => [0b01110, 0b10000, 0b11110, 0b10001, 0b10001, 0b10001, 0b01110],
        '7' => [0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b01000, 0b01000],
        '8' => [0b01110, 0b10001, 0b10001, 0b01110, 0b10001, 0b10001, 0b01110],
        '9' => [0b01110, 0b10001, 0b10001, 0b01111, 0b00001, 0b00001, 0b01110],
        '!' => [0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00000, 0b00100],
        '?' => [0b01110, 0b10001, 0b00001, 0b00110, 0b00100, 0b00000, 0b00100],
        '.' => [0b00000, 0b00000, 0b00000, 0b00000, 0b00000, 0b01100, 0b01100],
        ',' => [0b00000, 0b00000, 0b00000, 0b00000, 0b00100, 0b00100, 0b01000],
        ':' => [0b00000, 0b00100, 0b00100, 0b00000, 0b00100, 0b00100, 0b00000],
        '-' => [0b00000, 0b00000, 0b00000, 0b11111, 0b00000, 0b00000, 0b00000],
        '_' => [0b00000, 0b00000, 0b00000, 0b00000, 0b00000, 0b00000, 0b11111],
        '/' => [0b00001, 0b00010, 0b00100, 0b00100, 0b01000, 0b10000, 0b00000],
        '+' => [0b00000, 0b00100, 0b00100, 0b11111, 0b00100, 0b00100, 0b00000],
        '*' => [0b00000, 0b10101, 0b01110, 0b11111, 0b01110, 0b10101, 0b00000],
        '<' => [0b00010, 0b00100, 0b01000, 0b10000, 0b01000, 0b00100, 0b00010],
        '>' => [0b01000, 0b00100, 0b00010, 0b00001, 0b00010, 0b00100, 0b01000],
        '(' => [0b00010, 0b00100, 0b01000, 0b01000, 0b01000, 0b00100, 0b00010],
        ')' => [0b01000, 0b00100, 0b00010, 0b00010, 0b00010, 0b00100, 0b01000],
        '\'' => [0b00100, 0b00100, 0b01000, 0b00000, 0b00000, 0b00000, 0b00000],
        '%' => [0b11001, 0b11010, 0b00100, 0b00100, 0b01011, 0b10011, 0b00000],
        '#' => [0b01010, 0b11111, 0b01010, 0b01010, 0b11111, 0b01010, 0b00000],
        ' ' => [0; 7],
        _ => [0; 7],
    }
}

fn premul(a: u8, r: u8, g: u8, b: u8) -> u32 {
    let pr = ((r as u16 * a as u16) / 255) as u32;
    let pg = ((g as u16 * a as u16) / 255) as u32;
    let pb = ((b as u16 * a as u16) / 255) as u32;
    ((a as u32) << 24) | (pr << 16) | (pg << 8) | pb
}

fn wrap_lines(input: &str, max_chars: usize, max_lines: usize) -> Vec<String> {
    let cleaned: String = input
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect();
    let mut lines: Vec<String> = Vec::new();
    let mut cur = String::new();

    for word in cleaned.split_whitespace() {
        let mut chunk = word;
        while !chunk.is_empty() {
            if cur.is_empty() {
                if chunk.chars().count() <= max_chars {
                    cur.push_str(chunk);
                    break;
                } else {
                    let part: String = chunk.chars().take(max_chars).collect();
                    let byte_len = part.len();
                    lines.push(part);
                    chunk = &chunk[byte_len..];
                    if lines.len() >= max_lines {
                        return lines;
                    }
                }
            } else if cur.chars().count() + 1 + chunk.chars().count() <= max_chars {
                cur.push(' ');
                cur.push_str(chunk);
                break;
            } else {
                lines.push(std::mem::take(&mut cur));
                if lines.len() >= max_lines {
                    if let Some(last) = lines.last_mut() {
                        let chars: Vec<char> = last.chars().collect();
                        if chars.len() >= 3 {
                            let kept: String = chars[..chars.len() - 3].iter().collect();
                            *last = format!("{kept}...");
                        }
                    }
                    return lines;
                }
            }
        }
    }
    if !cur.is_empty() && lines.len() < max_lines {
        lines.push(cur);
    }
    if lines.is_empty() {
        lines.push("HI!".to_string());
    }
    lines
}

/// Render a crisp retro speech bubble into premultiplied ARGB `u32` pixels.
/// Returns `(width, height, pixels)`.
pub fn render_speech_bubble_argb(text: &str) -> (u32, u32, Vec<u32>) {
    let lines = wrap_lines(text, 24, 2);
    let scale = 2usize;
    let char_w = 6 * scale; // 5px glyph + 1px spacing
    let char_h = 7 * scale;
    let line_gap = 6usize;
    let pad_x = 14usize;
    let pad_y = 10usize;
    let tail_h = 8usize;

    let longest = lines.iter().map(|l| l.chars().count()).max().unwrap_or(1);
    let text_w = longest * char_w;
    let text_h = lines.len() * char_h + lines.len().saturating_sub(1) * line_gap;

    let box_w = (text_w + pad_x * 2).clamp(64, 340);
    let box_h = (text_h + pad_y * 2).clamp(32, 96);
    let total_w = box_w;
    let total_h = box_h + tail_h;

    let mut buf = vec![0u32; total_w * total_h];
    let bg = premul(244, 22, 26, 36);
    let border = premul(255, 232, 126, 62);
    let text_col = premul(255, 250, 246, 238);
    let shadow_col = premul(180, 10, 12, 18);
    let r = 8i32;

    let inside_rounded = |x: i32, y: i32, bw: i32, bh: i32, rad: i32| -> bool {
        if x < 0 || y < 0 || x >= bw || y >= bh {
            return false;
        }
        let cx = if x < rad {
            rad
        } else if x >= bw - rad {
            bw - rad - 1
        } else {
            x
        };
        let cy = if y < rad {
            rad
        } else if y >= bh - rad {
            bh - rad - 1
        } else {
            y
        };
        let dx = x - cx;
        let dy = y - cy;
        dx * dx + dy * dy <= rad * rad
    };

    // Draw rounded bubble box with 2px warm amber border
    for y in 0..box_h {
        for x in 0..box_w {
            if inside_rounded(x as i32, y as i32, box_w as i32, box_h as i32, r) {
                let inner = inside_rounded(
                    x as i32 - 2,
                    y as i32 - 2,
                    box_w as i32 - 4,
                    box_h as i32 - 4,
                    (r - 2).max(1),
                );
                buf[y * total_w + x] = if inner { bg } else { border };
            }
        }
    }

    // Draw centered downward speech pointer tail
    let mid = (box_w / 2) as i32;
    for dy in 0..tail_h as i32 {
        let half = (tail_h as i32 - 1 - dy).max(0);
        let y = (box_h as i32 - 2 + dy).clamp(0, total_h as i32 - 1) as usize;
        for dx in -half..=half {
            let x = (mid + dx).clamp(0, total_w as i32 - 1) as usize;
            let is_edge = dx.abs() >= half - 1 || dy == tail_h as i32 - 1;
            buf[y * total_w + x] = if is_edge { border } else { bg };
        }
    }
    // Clean interior seam above tail
    for dx in -(tail_h as i32 - 3)..=(tail_h as i32 - 3) {
        let x = (mid + dx).clamp(2, total_w as i32 - 3) as usize;
        if box_h >= 3 {
            buf[(box_h - 2) * total_w + x] = bg;
            buf[(box_h - 1) * total_w + x] = bg;
        }
    }

    // Stamp centered bitmap text lines
    for (li, line) in lines.iter().enumerate() {
        let line_px_w = line.chars().count() * char_w;
        let start_x = (box_w.saturating_sub(line_px_w)) / 2;
        let start_y = pad_y + li * (char_h + line_gap);

        for (ci, ch) in line.chars().enumerate() {
            let rows = glyph_5x7(ch);
            let gx = start_x + ci * char_w;
            for (ry, &bits) in rows.iter().enumerate() {
                for rx in 0..5usize {
                    if (bits & (1 << (4 - rx))) == 0 {
                        continue;
                    }
                    for sy in 0..scale {
                        for sx in 0..scale {
                            let px = gx + rx * scale + sx;
                            let py = start_y + ry * scale + sy;
                            if px + 1 < total_w && py + 1 < box_h {
                                buf[(py + 1) * total_w + (px + 1)] = shadow_col;
                            }
                            if px < total_w && py < box_h {
                                buf[py * total_w + px] = text_col;
                            }
                        }
                    }
                }
            }
        }
    }

    (total_w as u32, total_h as u32, buf)
}


/// Render a crisp 24x24 shaded pixel toy ball (premultiplied ARGB `u32`) for Play Mode.
pub fn render_pixel_ball_argb() -> (u32, u32, Vec<u32>) {
    let size = 24usize;
    let mut buf = vec![0u32; size * size];
    let c = 11.5_f32;
    let r_out = 10.6_f32;
    let r_in = 8.8_f32;

    let outline = premul(255, 42, 24, 24);
    let base = premul(255, 242, 104, 62);
    let shade = premul(255, 198, 66, 38);
    let stripe = premul(255, 255, 224, 128);
    let hi = premul(255, 255, 248, 236);

    for y in 0..size {
        for x in 0..size {
            let dx = x as f32 - c;
            let dy = y as f32 - c;
            let dist = dx.hypot(dy);
            if dist <= r_out {
                let col = if dist > r_in {
                    outline
                } else if (dx + 3.2).hypot(dy + 3.4) < 2.6 {
                    hi
                } else if (dx - dy).abs() < 1.9 {
                    stripe
                } else if dx + dy > 4.5 {
                    shade
                } else {
                    base
                };
                buf[y * size + x] = col;
            }
        }
    }
    (size as u32, size as u32, buf)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn speech_bubble_renders_opaque_box_and_handles_long_or_empty_text() {
        let (w, h, px) = render_speech_bubble_argb("Tests passed! 86/86");
        assert!(w >= 64 && w <= 340);
        assert!(h >= 40 && h <= 104);
        assert_eq!(px.len(), (w * h) as usize);
        assert!(px.iter().any(|&p| (p >> 24) == 255));
        assert_eq!(px[0], 0, "rounded corner should remain transparent");

        let (ew, eh, epx) = render_speech_bubble_argb("   ");
        assert_eq!(epx.len(), (ew * eh) as usize);

        let (lw, lh, lpx) = render_speech_bubble_argb(
            "This is a super long coding agent build status message that must wrap cleanly without overflowing",
        );
        assert!(lw <= 340 && lh <= 104);
        assert_eq!(lpx.len(), (lw * lh) as usize);
        let (bw, bh, bpx) = render_pixel_ball_argb();
        assert_eq!((bw, bh), (24, 24));
        assert_eq!(bpx[0], 0);
        assert!(bpx[12 * 24 + 12] >> 24 == 255);
    }
}