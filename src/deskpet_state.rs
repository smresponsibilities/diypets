use crate::deskpet_xml::{DeskpetAnimation, DeskpetPack};
use rand::Rng;

fn eval_arithmetic(expr: &str) -> Result<f64, ()> {
    struct Parser<'a> {
        bytes: &'a [u8],
        pos: usize,
    }
    impl<'a> Parser<'a> {
        fn skip_ws(&mut self) {
            while self.pos < self.bytes.len() && self.bytes[self.pos].is_ascii_whitespace() {
                self.pos += 1;
            }
        }
        fn peek(&mut self) -> Option<u8> {
            self.skip_ws();
            self.bytes.get(self.pos).copied()
        }
        fn parse_expr(&mut self) -> Result<f64, ()> {
            let mut val = self.parse_term()?;
            while let Some(op) = self.peek() {
                if op == b'+' || op == b'-' {
                    self.pos += 1;
                    let rhs = self.parse_term()?;
                    val = if op == b'+' { val + rhs } else { val - rhs };
                } else {
                    break;
                }
            }
            Ok(val)
        }
        fn parse_term(&mut self) -> Result<f64, ()> {
            let mut val = self.parse_unary()?;
            while let Some(op) = self.peek() {
                if op == b'*' || op == b'/' || op == b'%' {
                    self.pos += 1;
                    let rhs = self.parse_unary()?;
                    val = match op {
                        b'*' => val * rhs,
                        b'/' => {
                            if rhs.abs() < f64::EPSILON {
                                return Err(());
                            } else {
                                val / rhs
                            }
                        }
                        _ => {
                            if rhs.abs() < f64::EPSILON {
                                return Err(());
                            } else {
                                val % rhs
                            }
                        }
                    };
                } else {
                    break;
                }
            }
            Ok(val)
        }
        fn parse_unary(&mut self) -> Result<f64, ()> {
            match self.peek() {
                Some(b'+') => {
                    self.pos += 1;
                    self.parse_unary()
                }
                Some(b'-') => {
                    self.pos += 1;
                    Ok(-self.parse_unary()?)
                }
                _ => self.parse_primary(),
            }
        }
        fn parse_primary(&mut self) -> Result<f64, ()> {
            if self.peek() == Some(b'(') {
                self.pos += 1;
                let v = self.parse_expr()?;
                if self.peek() != Some(b')') {
                    return Err(());
                }
                self.pos += 1;
                return Ok(v);
            }
            self.skip_ws();
            let start = self.pos;
            let mut saw_dot = false;
            while self.pos < self.bytes.len() {
                let b = self.bytes[self.pos];
                if b.is_ascii_digit() {
                    self.pos += 1;
                } else if b == b'.' && !saw_dot {
                    saw_dot = true;
                    self.pos += 1;
                } else {
                    break;
                }
            }
            if self.pos == start {
                return Err(());
            }
            std::str::from_utf8(&self.bytes[start..self.pos])
                .map_err(|_| ())?
                .parse::<f64>()
                .map_err(|_| ())
        }
    }

    let mut p = Parser {
        bytes: expr.as_bytes(),
        pos: 0,
    };
    let val = p.parse_expr()?;
    p.skip_ws();
    if p.pos != p.bytes.len() || !val.is_finite() {
        return Err(());
    }
    Ok(val)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HerdEncounter {
    Greet,
    BumpTurn,
}

/// Detect when two pets walking toward each other on the same floor meet.
pub fn detect_herd_encounter(
    ax: f32,
    ay: f32,
    a_moving_left: bool,
    bx: f32,
    by: f32,
    b_moving_left: bool,
    seed: u64,
) -> Option<HerdEncounter> {
    if (ay - by).abs() > 38.0 {
        return None;
    }
    let dx = bx - ax;
    if dx.abs() > 46.0 {
        return None;
    }
    let approaching = (dx >= 0.0 && !a_moving_left && b_moving_left)
        || (dx < 0.0 && a_moving_left && !b_moving_left);
    if !approaching {
        return None;
    }
    if seed.is_multiple_of(2) {
        Some(HerdEncounter::Greet)
    } else {
        Some(HerdEncounter::BumpTurn)
    }
}
#[derive(Clone, Debug, PartialEq)]
pub enum DeskpetStateId {
    Drag,
    Fall,
    FallSoft,
    FallHard,
    Kill,
    Sync,
    Toss,
    Custom(String),
}

impl DeskpetStateId {
    pub fn from_string(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "drag" => Self::Drag,
            "fall" => Self::Fall,
            "fall soft" => Self::FallSoft,
            "fall hard" => Self::FallHard,
            "kill" => Self::Kill,
            "sync" => Self::Sync,
            "toss" => Self::Toss,
            _ => Self::Custom(s.to_string()),
        }
    }

    #[allow(clippy::inherent_to_string)]
    pub fn to_string(&self) -> String {
        match self {
            Self::Drag => "drag".to_string(),
            Self::Fall => "fall".to_string(),
            Self::FallSoft => "fall soft".to_string(),
            Self::FallHard => "fall hard".to_string(),
            Self::Kill => "kill".to_string(),
            Self::Sync => "sync".to_string(),
            Self::Toss => "toss".to_string(),
            Self::Custom(s) => s.clone(),
        }
    }
}

pub enum Trigger {
    Next,
    Border,
    Gravity,
}

pub fn window_floor_candidate(
    window_top: f32,
    pet_height: f32,
    feet_y: f32,
    screen_top: f32,
) -> Option<f32> {
    const CONTACT_TOLERANCE: f32 = 8.0;
    let floor = window_top - pet_height;
    (window_top.is_finite()
        && pet_height.is_finite()
        && feet_y.is_finite()
        && screen_top.is_finite()
        && pet_height > 0.0
        && floor >= screen_top
        && (0.0..=CONTACT_TOLERANCE).contains(&(feet_y - window_top)))
    .then_some(floor)
}

pub fn select_window_floor(desktop_floor: f32, window_floor: Option<f32>) -> f32 {
    match window_floor {
        Some(floor) if floor < desktop_floor - 3.0 => floor,
        _ => desktop_floor,
    }
}

pub struct DeskpetState {
    pub pack: DeskpetPack,
    pub current_anim_id: String,
    pub sequence_index: usize,
    pub elapsed_in_frame: u64,
    pub rand_s: f64,
    pub x: f32,
    pub y: f32,
    pub opacity: f32,
    pub is_moving_left: bool,
    pub is_tossing: bool,
    pub toss_force_x: f32,
    pub toss_vert_vel: f32,

    // Evaluated for the current animation start/end X/Y
    anim_start_x: f32,
    anim_end_x: f32,
    anim_start_y: f32,
    anim_end_y: f32,

    pub screen_w: f32,
    pub screen_h: f32,
    pub area_h: f32, // work area height (screen minus taskbar)
    pub pet_size: u32,
    pub walk_speed: i32,
    pub on_window: bool,
    pub climb_enabled: bool,
    current_total_steps: usize,
}

impl DeskpetState {
    pub fn ground_height(&self) -> f32 {
        self.pack.ground_y as f32 * self.pet_size as f32 / self.pack.tile_w as f32
    }

    pub fn release_drag(&mut self, dx: f32, dy: f32, sample_ms: u64) {
        let factor = 10.0 / sample_ms.max(1) as f32;
        let (vx, vy) = (dx * factor, dy * factor);
        self.is_tossing = vx.hypot(vy) > 5.0;
        self.toss_force_x = if self.is_tossing { vx } else { 0.0 };
        self.toss_vert_vel = if self.is_tossing { vy } else { 0.0 };
        self.set_state(DeskpetStateId::Fall);
    }

    pub fn new(pack: DeskpetPack) -> Self {
        let mut rng = rand::thread_rng();
        let rand_s = rng.gen_range(10.0..90.0);

        let mut state = Self {
            pet_size: pack.tile_w,
            walk_speed: 14,
            pack,
            current_anim_id: String::new(),
            sequence_index: 0,
            elapsed_in_frame: 0,
            rand_s,
            x: 500.0,
            y: 0.0,
            opacity: 1.0,
            is_moving_left: true,
            is_tossing: false,
            toss_force_x: 0.0,
            toss_vert_vel: 0.0,
            anim_start_x: 0.0,
            anim_end_x: 0.0,
            anim_start_y: 0.0,
            anim_end_y: 0.0,
            screen_w: 1920.0,
            screen_h: 1080.0,
            area_h: 1080.0,
            on_window: false,
            climb_enabled: true,
            current_total_steps: 1,
        };

        let mut start_anim = "1".to_string();
        if !state.pack.spawns.is_empty() {
            let total_prob: u32 = state.pack.spawns.iter().map(|s| s.probability).sum();
            let mut pick = rng.gen_range(0..total_prob.max(1));
            for spawn in &state.pack.spawns {
                if pick < spawn.probability {
                    state.x = state
                        .eval_math(&spawn.x)
                        .unwrap_or(state.screen_w as f64 + 10.0)
                        as f32;
                    state.y = state.eval_math(&spawn.y).unwrap_or(0.0) as f32;
                    // C# uses spawn.Next (the animation id to start), not spawn.id
                    if !spawn.next_animation_id.is_empty() {
                        start_anim = spawn.next_animation_id.clone();
                    }
                    break;
                }
                pick -= spawn.probability;
            }
        }

        state.set_state(DeskpetStateId::Custom(start_anim));
        state
    }

    #[allow(clippy::result_unit_err)]
    pub fn eval_math(&self, expr: &str) -> Result<f64, ()> {
        let mut rng = rand::thread_rng();
        let random = rng.gen_range(0..100);
        let e = expr
            .replace("randS", &self.rand_s.to_string())
            .replace("screenW", &self.screen_w.to_string())
            .replace("screenH", &self.screen_h.to_string())
            .replace("areaW", &self.screen_w.to_string())
            .replace("areaH", &self.area_h.to_string())
            .replace("imageW", &self.pack.tile_w.to_string())
            .replace("imageH", &self.pack.tile_h.to_string())
            .replace("random", &random.to_string());
        eval_arithmetic(&e)
    }

    /// Return the frame index to display. Handles repeat-loop wrapping identical to C#.
    pub fn current_frame(&self) -> Option<u32> {
        let anim = self.pack.animations.get(&self.current_anim_id)?;
        let base = anim.sequence.len();
        if base == 0 {
            return None;
        }
        if self.sequence_index < base {
            return Some(anim.sequence[self.sequence_index]);
        }
        // Repeat from the requested frame after the initial sequence.
        let repeat_from = anim.repeat_from.min(base.saturating_sub(1));
        let loop_len = base.saturating_sub(repeat_from);
        if loop_len == 0 {
            return anim.sequence.last().copied();
        }
        let idx = ((self.sequence_index - base) % loop_len) + repeat_from;
        Some(anim.sequence[idx.min(base - 1)])
    }

    pub fn set_state(&mut self, state: DeskpetStateId) {
        let mut id = state.to_string();
        if !self.pack.animations.contains_key(&id) {
            // eSheep standard XML mappings with graceful fallbacks for incomplete packs
            let candidates: &[&str] = match &state {
                DeskpetStateId::Drag => &["4", "5", "1"],
                DeskpetStateId::Fall => &["5", "3", "1"],
                DeskpetStateId::FallSoft => &["9", "12", "1"],
                DeskpetStateId::FallHard => &["10", "9", "12", "1"],
                DeskpetStateId::Custom(name) => match name.to_lowercase().as_str() {
                    "sleep" | "nap" => &["13", "12", "1"],
                    "sit" | "idle" | "rest" => &["12", "1"],
                    "climb" | "wall" => &["19", "1"],
                    "walk" => &["1"],
                    _ => &["1"],
                },
                _ => &["1"],
            };
            id = candidates
                .iter()
                .find(|cid| self.pack.animations.contains_key(**cid))
                .copied()
                .unwrap_or("1")
                .to_string();
        }
        // Last resort: try animation "1"
        if !self.pack.animations.contains_key(&id) {
            id = "1".to_string();
        }

        if let Some(anim) = self.pack.animations.get(&id) {
            self.current_anim_id = id.clone();
            self.sequence_index = 0;
            self.elapsed_in_frame = 0;
            self.anim_start_x = self.eval_math(&anim.start_x).unwrap_or(0.0) as f32;
            self.anim_end_x = self
                .eval_math(&anim.end_x)
                .unwrap_or(self.anim_start_x as f64) as f32;
            self.anim_start_y = self.eval_math(&anim.start_y).unwrap_or(0.0) as f32;
            self.anim_end_y = self
                .eval_math(&anim.end_y)
                .unwrap_or(self.anim_start_y as f64) as f32;

            let base = anim.sequence.len();
            let repeat_count = self
                .eval_math(&anim.repeat_expr)
                .ok()
                .map(|v| v.round().clamp(0.0, 5000.0) as usize)
                .unwrap_or(0);
            let steps = if anim.repeat_from > 0 {
                base + base.saturating_sub(anim.repeat_from) * repeat_count
            } else {
                base + base * repeat_count
            };
            self.current_total_steps = steps.max(anim.total_steps).max(base).max(1);
        }
    }

    /// One physics tick, called once per frame. Matches C# NextStep() logic exactly:
    /// 1. Interpolate interval/opacity.
    /// 2. Check if interval has elapsed — if not, return early.
    /// 3. Compute per-step dx/dy.
    /// 4. Check borders BEFORE applying movement (C# checks PositionX+x vs bounds before PositionX+=x).
    /// 5. Apply movement if no border was hit.
    /// 6. Advance sequence_index, check animation completion.
    pub fn tick(&mut self, delta_ms: u64, floor_y: f32, screen_width: f32) {
        self.tick_in_bounds(delta_ms, floor_y, 0.0, screen_width);
    }

    pub fn tick_in_bounds(&mut self, delta_ms: u64, floor_y: f32, min_x: f32, screen_width: f32) {
        let delta_ms = delta_ms.min(100);
        let max_x = (screen_width - self.pet_size as f32).max(min_x);
        if !self.x.is_finite() {
            self.x = min_x;
        } else {
            self.x = self.x.clamp(min_x, max_x);
        }
        if !self.y.is_finite() || self.y > floor_y {
            self.y = floor_y;
        }

        let anim = match self.pack.animations.get(&self.current_anim_id) {
            Some(a) => a.clone(),
            None => return,
        };

        if !self.is_tossing
            && !anim.next_gravity.is_empty()
            && self.y < floor_y
            && self.y >= floor_y - 8.0
        {
            self.y = floor_y;
        }

        let total_steps = self.current_total_steps.max(1) as f32;
        let step = self.sequence_index as f32;
        let progress_ratio = (step / total_steps).min(1.0);

        let current_interval = anim.start_interval as f32
            + (anim.end_interval as f32 - anim.start_interval as f32) * progress_ratio;
        let moving_h = self.anim_start_x.abs() > 0.01 || self.anim_end_x.abs() > 0.01;
        let current_interval = if moving_h && self.walk_speed != 5 {
            let speed_mult = (self.walk_speed as f32 / 5.0).max(0.2);
            (current_interval / speed_mult.sqrt()).clamp(35.0, 600.0)
        } else {
            current_interval
        };

        self.opacity =
            anim.start_opacity + (anim.end_opacity - anim.start_opacity) * progress_ratio;

        let interval_u = if self.is_tossing {
            40
        } else {
            current_interval.max(1.0) as u64
        };

        let fraction = (delta_ms as f32) / (interval_u as f32).max(1.0);

        if self.is_tossing {
            let mut hit_floor = false;
            let move_y = self.toss_vert_vel * fraction;
            if self.y + move_y >= floor_y {
                hit_floor = true;
            }

            if hit_floor {
                self.y = floor_y;
                if self.toss_vert_vel < 40.0 {
                    self.set_state(DeskpetStateId::FallSoft);
                } else {
                    self.set_state(DeskpetStateId::FallHard);
                }
                self.is_tossing = false;

                if (self.toss_force_x < 0.0 && !self.is_moving_left)
                    || (self.toss_force_x > 0.0 && self.is_moving_left)
                {
                    self.is_moving_left = !self.is_moving_left;
                }
                return;
            } else {
                let move_x = self.toss_force_x * fraction;
                if self.x + move_x < min_x {
                    self.toss_force_x = -self.toss_force_x * 0.3;
                    self.x = min_x;
                } else if self.x + move_x + self.pet_size as f32 > screen_width {
                    self.toss_force_x = -self.toss_force_x * 0.3;
                    self.x = max_x;
                } else {
                    self.x += move_x;
                }

                self.y += move_y;
                if self.y < 0.0 {
                    self.y = 0.0;
                    if self.toss_vert_vel < 0.0 {
                        self.toss_vert_vel = 0.0;
                    }
                }
                self.toss_vert_vel += 1.5 * fraction;
            }
        } else {
            let step_divisor = (total_steps - 1.0).max(1.0);
            let speed_mult = self.walk_speed as f32 / 3.5;
            let x_vel = (self.anim_start_x
                + (self.anim_end_x - self.anim_start_x) * step / step_divisor)
                * speed_mult;
            let y_vel = (self.anim_start_y
                + (self.anim_end_y - self.anim_start_y) * step / step_divisor)
                * speed_mult;

            let dir = if self.is_moving_left { 1.0 } else { -1.0 };
            let move_x = x_vel * dir * fraction;
            let move_y = y_vel * fraction;

            let mut apply = true;
            if move_x < 0.0 && self.x + move_x < min_x {
                let prev = self.current_anim_id.clone();
                self.pick_next_animation(&anim, Trigger::Border, true);
                self.x = min_x;
                apply = false;
                if self.current_anim_id == prev {
                    self.is_moving_left = false;
                }
            } else if move_x > 0.0 && self.x + move_x + self.pet_size as f32 > screen_width {
                let prev = self.current_anim_id.clone();
                self.pick_next_animation(&anim, Trigger::Border, true);
                self.x = max_x;
                apply = false;
                if self.current_anim_id == prev {
                    self.is_moving_left = true;
                }
            } else if move_y > 0.0 && self.y + move_y >= floor_y {
                let prev = self.current_anim_id.clone();
                self.pick_next_animation(&anim, Trigger::Border, false);
                self.y = floor_y;
                if self.current_anim_id != prev {
                    return;
                }
                apply = false;
            } else if move_y < 0.0 && self.y + move_y < 0.0 {
                self.pick_next_animation(&anim, Trigger::Border, false);
                apply = false;
            }

            if apply {
                self.x += move_x;
                self.y += move_y;

                if !anim.next_gravity.is_empty() && self.y < floor_y - 8.0 {
                    self.is_tossing = true;
                    self.toss_force_x = x_vel * dir;
                    self.toss_vert_vel = 0.0;
                    self.pick_next_animation(&anim, Trigger::Gravity, false);
                }
            }
        }

        self.elapsed_in_frame += delta_ms;
        if self.elapsed_in_frame < interval_u {
            return;
        }
        self.elapsed_in_frame -= interval_u;

        if !self.is_tossing
            && anim.action == "flip"
            && self.sequence_index + 1 >= self.current_total_steps
        {
            self.is_moving_left = !self.is_moving_left;
        }

        self.sequence_index += 1;
        if self.sequence_index >= self.current_total_steps {
            if self.is_tossing {
                self.sequence_index = 0;
            } else {
                self.pick_next_animation(&anim, Trigger::Next, false);
            }
        }
    }

    fn pick_next_animation(
        &mut self,
        anim: &DeskpetAnimation,
        trigger: Trigger,
        border_vertical: bool,
    ) {
        let raw_pool = match trigger {
            Trigger::Next => &anim.next,
            Trigger::Border => &anim.next_border,
            Trigger::Gravity => &anim.next_gravity,
        };

        if raw_pool.is_empty() {
            if matches!(trigger, Trigger::Next) {
                self.sequence_index = 0;
            }
            return;
        }

        let filtered: Vec<&crate::deskpet_xml::DeskpetNext> = raw_pool
            .iter()
            .filter(|n| {
                if !self.climb_enabled && (n.only == "vertical" || n.animation_id == "19") {
                    return false;
                }
                match n.only.as_str() {
                    "" | "none" => true,
                    "taskbar" => !self.on_window,
                    "window" => self.on_window,
                    "vertical" => border_vertical && self.climb_enabled,
                    "horizontal+" => !border_vertical,
                    _ => true,
                }
            })
            .collect();

        let pool: Vec<&crate::deskpet_xml::DeskpetNext> = if filtered.is_empty() {
            raw_pool.iter().collect()
        } else {
            filtered
        };

        let total_prob: u32 = pool.iter().map(|n| n.probability).sum();
        if total_prob == 0 {
            self.set_state(DeskpetStateId::Custom(pool[0].animation_id.clone()));
            return;
        }

        let mut rng = rand::thread_rng();
        let mut pick = rng.gen_range(0..total_prob);

        for next_node in pool {
            if pick < next_node.probability {
                self.set_state(DeskpetStateId::Custom(next_node.animation_id.clone()));
                break;
            } else {
                pick -= next_node.probability;
            }
        }
    }
}

#[cfg(test)]
mod window_floor_tests {
    use super::{
        DeskpetPack, DeskpetState, DeskpetStateId, select_window_floor, window_floor_candidate,
    };

    #[test]
    fn near_taskbar_window_does_not_raise_floor() {
        assert_eq!(select_window_floor(963.0, Some(961.0)), 963.0);
        assert_eq!(select_window_floor(963.0, Some(800.0)), 800.0);
        assert_eq!(select_window_floor(963.0, None), 963.0);
    }

    #[test]
    fn long_pause_does_not_teleport_walking_pet() {
        let pack = DeskpetPack::parse(include_str!("../assets/highland_cow/deskpet.xml")).unwrap();
        let mut pet = DeskpetState::new(pack);
        pet.x = 500.0;
        pet.y = 100.0;
        pet.tick(5000, 100.0, 1920.0);
        assert!(pet.x >= 470.0, "pet moved too far after a pause: {}", pet.x);
    }

    #[test]
    fn repeat_from_restarts_at_requested_frame() {
        let xml = include_str!("../assets/highland_cow/deskpet.xml").replacen(
            "<Sequence repeat=\"2\" repeatfrom=\"0\"><Frame>16</Frame>",
            "<Sequence repeat=\"2\" repeatfrom=\"1\"><Frame>16</Frame>",
            1,
        );
        let mut pet = DeskpetState::new(DeskpetPack::parse(&xml).unwrap());
        pet.set_state(DeskpetStateId::Custom("12".to_string()));
        pet.sequence_index = 4;
        assert_eq!(pet.current_frame(), Some(17));
        pet.sequence_index = 5;
        assert_eq!(pet.current_frame(), Some(18));
    }

    #[test]
    fn lowercase_transition_probabilities_match_original_esheep_format() {
        let xml = include_str!("../assets/highland_cow/deskpet.xml")
            .replace("Probability=", "probability=");
        let pack = DeskpetPack::parse(&xml).unwrap();
        let walk = &pack.animations["1"];
        assert_eq!(walk.next[0].probability, 60);
        assert_eq!(walk.next[1].probability, 6);
    }

    #[test]
    fn ordinary_fall_stays_in_fall_state_until_contact() {
        let pack = DeskpetPack::parse(include_str!("../assets/highland_cow/deskpet.xml")).unwrap();
        let mut pet = DeskpetState::new(pack);
        pet.set_state(DeskpetStateId::Fall);
        pet.y = 0.0;
        pet.sequence_index = pet.pack.animations["5"].total_steps - 1;
        pet.elapsed_in_frame = 30;
        pet.tick(100, 1000.0, 1920.0);
        assert_eq!(pet.current_anim_id, "5");
    }

    #[test]
    fn landing_starts_the_new_animation_at_its_first_frame() {
        let pack = DeskpetPack::parse(include_str!("../assets/highland_cow/deskpet.xml")).unwrap();
        let mut pet = DeskpetState::new(pack);
        pet.set_state(DeskpetStateId::Fall);
        pet.sequence_index = 2;
        pet.is_tossing = true;
        pet.toss_vert_vel = 5.0;
        pet.y = 99.0;
        pet.tick(40, 100.0, 1920.0);
        assert_eq!(pet.current_anim_id, "9");
        assert_eq!(pet.sequence_index, 0);
        assert!(!pet.is_tossing);
    }

    #[test]
    fn authored_foot_baseline_scales_with_pet_size() {
        let xml = include_str!("../assets/highland_cow/deskpet.xml").replacen(
            "<TilesY>16</TilesY>",
            "<TilesY>16</TilesY><GroundY>117</GroundY>",
            1,
        );
        let mut pet = DeskpetState::new(DeskpetPack::parse(&xml).unwrap());
        pet.pet_size = 256;
        assert_eq!(pet.ground_height(), 234.0);
    }

    #[test]
    fn small_floor_gap_is_closed_without_starting_a_fall() {
        let pack = DeskpetPack::parse(include_str!("../assets/highland_cow/deskpet.xml")).unwrap();
        let mut pet = DeskpetState::new(pack);
        pet.y = 98.0;
        pet.tick(0, 100.0, 1920.0);
        assert_eq!(pet.y, 100.0);
        assert!(!pet.is_tossing);
    }

    #[test]
    fn ordinary_fall_transitions_to_landing_on_contact() {
        let pack = DeskpetPack::parse(include_str!("../assets/highland_cow/deskpet.xml")).unwrap();
        let mut pet = DeskpetState::new(pack);
        pet.set_state(DeskpetStateId::Fall);
        pet.y = 99.0;
        pet.tick(130, 100.0, 1920.0);
        assert_eq!(pet.current_anim_id, "9");
        assert_eq!(pet.sequence_index, 0);
    }

    #[test]
    fn gentle_release_falls_but_a_fast_throw_uses_toss_physics() {
        let pack = DeskpetPack::parse(include_str!("../assets/highland_cow/deskpet.xml")).unwrap();
        let mut pet = DeskpetState::new(pack);
        pet.release_drag(4.0, 0.0, 20);
        assert!(!pet.is_tossing);
        assert_eq!(pet.current_anim_id, "5");
        pet.release_drag(20.0, 0.0, 20);
        assert!(pet.is_tossing);
        assert_eq!(pet.toss_force_x, 10.0);
    }

    #[test]
    fn a_window_body_over_the_pet_is_not_a_floor() {
        assert_eq!(window_floor_candidate(300.0, 64.0, 500.0, 0.0), None);
    }

    #[test]
    fn a_pet_touching_a_window_top_gets_a_surface() {
        assert_eq!(window_floor_candidate(500.0, 64.0, 502.0, 0.0), Some(436.0));
    }

    #[test]
    fn a_window_too_close_to_screen_top_cannot_pull_pet_off_screen() {
        assert_eq!(window_floor_candidate(50.0, 64.0, 50.0, 0.0), None);
    }

    #[test]
    fn foot_skating_speed_scales_frame_interval() {
        let xml = include_str!("../assets/highland_cow/deskpet.xml");
        let mut slow = DeskpetState::new(DeskpetPack::parse(xml).unwrap());
        slow.set_state(DeskpetStateId::Custom("1".to_string()));
        slow.walk_speed = 2;
        slow.x = 600.0;
        slow.y = 800.0;

        let mut fast = DeskpetState::new(DeskpetPack::parse(xml).unwrap());
        fast.set_state(DeskpetStateId::Custom("1".to_string()));
        fast.walk_speed = 16;
        fast.x = 600.0;
        fast.y = 800.0;

        for _ in 0..20 {
            slow.tick(30, 800.0, 1920.0);
            fast.tick(30, 800.0, 1920.0);
        }
        assert!(
            fast.sequence_index > slow.sequence_index,
            "fast walk should step frames quicker (fast={}, slow={})",
            fast.sequence_index,
            slow.sequence_index
        );
    }

    #[test]
    fn herd_encounter_detects_approaching_pets_and_ignores_distant_or_diverging() {
        use super::{HerdEncounter, detect_herd_encounter};
        assert_eq!(
            detect_herd_encounter(200.0, 500.0, false, 232.0, 505.0, true, 0),
            Some(HerdEncounter::Greet)
        );
        assert_eq!(
            detect_herd_encounter(200.0, 500.0, false, 232.0, 505.0, true, 1),
            Some(HerdEncounter::BumpTurn)
        );
        assert_eq!(
            detect_herd_encounter(200.0, 500.0, true, 232.0, 505.0, false, 0),
            None,
            "pets walking away from each other should not collide"
        );
        assert_eq!(
            detect_herd_encounter(200.0, 500.0, false, 232.0, 200.0, true, 0),
            None,
            "pets on different window floors should not collide"
        );
    }

    #[test]
    fn multi_monitor_negative_min_x_bounds_work() {
        let pack = DeskpetPack::parse(include_str!("../assets/highland_cow/deskpet.xml")).unwrap();
        let mut pet = DeskpetState::new(pack);
        pet.pet_size = 128;
        pet.x = -500.0;
        pet.y = 500.0;
        pet.tick_in_bounds(16, 800.0, -1920.0, 1920.0);
        assert!(
            pet.x < 0.0 && pet.x >= -1920.0,
            "pet clamped away from left monitor: {}",
            pet.x
        );
    }

    #[test]
    fn shrinking_screen_width_pulls_stranded_pet_back_on_screen() {
        let pack = DeskpetPack::parse(include_str!("../assets/highland_cow/deskpet.xml")).unwrap();
        let mut pet = DeskpetState::new(pack);
        pet.pet_size = 128;
        pet.x = 2400.0;
        pet.y = 500.0;
        pet.tick(16, 800.0, 1920.0);
        assert!(
            pet.x >= 0.0 && pet.x <= 1920.0 - 128.0,
            "pet stayed off-screen after monitor shrank: x={}",
            pet.x
        );
    }

    #[test]
    fn floor_jitter_does_not_interrupt_walk_but_a_ledge_does() {
        let pack = DeskpetPack::parse(include_str!("../assets/deskpet.xml")).unwrap();
        let mut pet = DeskpetState::new(pack);
        pet.set_state(DeskpetStateId::Custom("1".to_string()));
        pet.y = 500.0;
        pet.tick(1000, 504.0, 1920.0);
        assert_eq!(pet.current_anim_id, "1");
        assert!(!pet.is_tossing);
        pet.tick(1000, 520.0, 1920.0);
        assert_eq!(pet.current_anim_id, "5");
        assert!(pet.is_tossing);
    }
}
