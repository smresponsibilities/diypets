//! Lightweight runtime Squash & Stretch + Idle Breathing deformation for premultiplied ARGB buffers.
//! Anchors scaling at the pet's feet (`bottom-center`) so pets stay planted on windows & taskbar.

#[derive(Debug, Clone, Copy)]
pub struct SquashSpring {
    pub scale_x: f32,
    pub scale_y: f32,
    vel_x: f32,
    vel_y: f32,
}

impl Default for SquashSpring {
    fn default() -> Self {
        Self {
            scale_x: 1.0,
            scale_y: 1.0,
            vel_x: 0.0,
            vel_y: 0.0,
        }
    }
}

impl SquashSpring {
    /// Trigger an impact squash (wider & shorter) when landing from a fall.
    pub fn trigger_land(&mut self) {
        self.scale_x = 1.14;
        self.scale_y = 0.86;
        self.vel_x = 0.0;
        self.vel_y = 0.0;
    }

    /// Trigger a launch stretch (narrower & taller) on hop or toss.
    pub fn trigger_hop(&mut self) {
        self.scale_x = 0.88;
        self.scale_y = 1.14;
        self.vel_x = 0.0;
        self.vel_y = 0.0;
    }

    /// Advance spring-damper toward `(1.0, 1.0)` + subtle breathing while idle/sleeping.
    pub fn step(&mut self, dt_sec: f32, time_sec: f32, is_idle: bool) -> (f32, f32) {
        let dt = dt_sec.clamp(0.001, 0.05);
        let k = 180.0_f32;
        let damp = 16.0_f32;

        let ax = (1.0 - self.scale_x) * k - self.vel_x * damp;
        let ay = (1.0 - self.scale_y) * k - self.vel_y * damp;
        self.vel_x += ax * dt;
        self.vel_y += ay * dt;
        self.scale_x = (self.scale_x + self.vel_x * dt).clamp(0.75, 1.30);
        self.scale_y = (self.scale_y + self.vel_y * dt).clamp(0.75, 1.30);

        let breath = if is_idle {
            (time_sec * 2.6).sin() * 0.025
        } else {
            0.0
        };
        (
            (self.scale_x - breath * 0.6).clamp(0.75, 1.30),
            (self.scale_y + breath).clamp(0.75, 1.30),
        )
    }
}

/// Apply bottom-anchored nearest-neighbor squash & stretch into `dst` (same dimensions `w x h`).
pub fn apply_squash_nearest(
    src: &[u32],
    dst: &mut Vec<u32>,
    w: u32,
    h: u32,
    scale_x: f32,
    scale_y: f32,
) {
    let needed = (w * h) as usize;
    if dst.len() != needed {
        dst.resize(needed, 0);
    }
    if (scale_x - 1.0).abs() < 0.005 && (scale_y - 1.0).abs() < 0.005 {
        dst.copy_from_slice(&src[..needed]);
        return;
    }
    dst.fill(0);
    let bottom = (h.saturating_sub(1)) as f32;
    let cx = (w.saturating_sub(1)) as f32 * 0.5;
    let inv_sx = 1.0 / scale_x.max(0.2);
    let inv_sy = 1.0 / scale_y.max(0.2);

    for y in 0..h {
        // Anchor at bottom pixel row (`h - 1`) so feet stay planted on the taskbar/window ledge
        let sy = (bottom - (bottom - y as f32) * inv_sy).round() as i32;
        if sy < 0 || sy >= h as i32 {
            continue;
        }
        let src_row = (sy as u32 * w) as usize;
        let dst_row = (y * w) as usize;
        for x in 0..w {
            let sx = (cx + (x as f32 - cx) * inv_sx).round() as i32;
            if sx >= 0 && sx < w as i32 {
                dst[dst_row + x as usize] = src[src_row + sx as usize];
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spring_settles_back_to_unit_scale_and_preserves_bottom_anchor() {
        let mut sp = SquashSpring::default();
        sp.trigger_land();
        assert!(sp.scale_x > 1.1 && sp.scale_y < 0.9);
        for i in 0..120 {
            sp.step(0.016, i as f32 * 0.016, false);
        }
        assert!((sp.scale_x - 1.0).abs() < 0.02);
        assert!((sp.scale_y - 1.0).abs() < 0.02);

        // Verify bottom-most pixel stays planted on the floor row when squashed
        let mut src = vec![0u32; 16 * 16];
        src[15 * 16 + 8] = 0xFF_FF_88_22;
        let mut dst = Vec::new();
        apply_squash_nearest(&src, &mut dst, 16, 16, 1.15, 0.85);
        assert_eq!(dst[15 * 16 + 8], 0xFF_FF_88_22);
    }
}
