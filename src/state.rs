#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PetState {
    Idle,
    Walk,
    Chase,
    Sleep,
    Drag,
    Fall,
    Climb,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PetBehavior {
    Neko,
    ESheep,
    Shimeji,
}

/// Which way the frame is turned when it is drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rotation {
    Upright,
    /// A quarter turn clockwise, used while the pet scales a window edge.
    QuarterTurn,
}

/// Screen rectangle of an application window, in physical pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WindowRect {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

impl WindowRect {
    pub fn left(&self) -> i32 {
        self.x
    }

    pub fn right(&self) -> i32 {
        self.x + self.width
    }

    /// How far the window moved since `previous`.
    pub fn delta(&self, previous: &WindowRect) -> (i32, i32) {
        (self.x - previous.x, self.y - previous.y)
    }
}

/// One muddy mark left behind on the desktop.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Footprint {
    pub x: i32,
    pub y: i32,
    pub born_ms: u64,
}

/// Marks the pet leaves while it walks, in the spirit of the goose.
///
/// A mark is stamped once the pet has walked `spacing` pixels from the last
/// one, and every mark is dropped `lifetime_ms` after it was laid.
///
/// The distance test uses the pet's centre, never an individual foot. The
/// tracker alternates the foot itself for the stamp position: letting the
/// caller pass a left then a right foot would make the tracked point jump
/// sideways while the pet stands still, which reads as travel and lays a mark
/// every frame.
#[derive(Debug, Clone)]
pub struct Footprints {
    marks: Vec<Footprint>,
    last_x: i32,
    last_y: i32,
    spacing: i32,
    lifetime_ms: u64,
    primed: bool,
    /// 0 or 1, which foot lays the next mark.
    side: i32,
}

impl Footprints {
    pub fn new(spacing: i32, lifetime_ms: u64) -> Self {
        Self {
            marks: Vec::new(),
            last_x: 0,
            last_y: 0,
            spacing: spacing.max(1),
            lifetime_ms,
            primed: false,
            side: 0,
        }
    }

    /// Note that the pet's body centre reached `(centre_x, foot_y)`.
    ///
    /// `pet_px` is the pet's edge length, used to alternate the mark between a
    /// left and a right foot so the trail reads as a walk.
    pub fn track(&mut self, centre_x: i32, foot_y: i32, pet_px: i32, elapsed_ms: u64) {
        if !self.primed {
            self.last_x = centre_x;
            self.last_y = foot_y;
            self.primed = true;
            return;
        }

        let (dx, dy) = (centre_x - self.last_x, foot_y - self.last_y);
        if dx * dx + dy * dy < self.spacing * self.spacing {
            return;
        }

        self.last_x = centre_x;
        self.last_y = foot_y;
        let offset = pet_px / 4;
        self.marks.push(Footprint {
            x: if self.side == 0 {
                centre_x - offset
            } else {
                centre_x + offset
            },
            y: foot_y,
            born_ms: elapsed_ms,
        });
        self.side = 1 - self.side;
    }

    /// Drop marks that have dried, so the trail fades instead of filling the disk.
    pub fn expire(&mut self, elapsed_ms: u64) {
        self.marks
            .retain(|mark| elapsed_ms.saturating_sub(mark.born_ms) < self.lifetime_ms);
    }

    pub fn marks(&self) -> &[Footprint] {
        &self.marks
    }

    pub fn is_empty(&self) -> bool {
        self.marks.is_empty()
    }
}

/// Axis-aligned area the pet window's top-left corner may occupy, in screen pixels.
///
/// `main.rs` builds this from the monitor rectangle and the pet's physical size.
/// Every state clamps its position into it, so no state can walk the window off
/// the desktop (the old window-climb code parked the pet at a negative `y`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Bounds {
    pub min_x: i32,
    pub min_y: i32,
    pub max_x: i32,
    pub max_y: i32,
}

impl Bounds {
    /// `pet_px` is the pet window's edge length in physical pixels.
    pub fn from_monitor(mon_x: i32, mon_y: i32, mon_w: u32, mon_h: u32, pet_px: i32) -> Self {
        let pet_px = pet_px.max(0);
        Self {
            min_x: mon_x,
            min_y: mon_y,
            max_x: (mon_x + mon_w as i32 - pet_px).max(mon_x),
            max_y: (mon_y + mon_h as i32 - pet_px).max(mon_y),
        }
    }

    pub fn clamp_x(&self, x: i32) -> i32 {
        x.clamp(self.min_x, self.max_x)
    }

    pub fn clamp_y(&self, y: i32) -> i32 {
        y.clamp(self.min_y, self.max_y)
    }

    pub fn contains(&self, x: i32, y: i32) -> bool {
        x >= self.min_x && x <= self.max_x && y >= self.min_y && y <= self.max_y
    }
}

/// Most animation steps one late `tick` may apply, so a paused process does not
/// teleport the pet across the screen when it wakes up.
const MAX_STEPS_PER_TICK: u32 = 4;

/// Animation frames with no pointer movement before a `neko` pack falls asleep.
const IDLE_FRAMES_BEFORE_SLEEP: u32 = 500;

pub struct StateMachine {
    state: PetState,
    pub behavior: PetBehavior,
    frame_index: u32,
    idle_frames: Vec<u32>,
    walk_frames: Vec<u32>,
    pub x: i32,
    pub y: i32,
    pub dy: i32,
    pub dx: i32,
    /// Unit direction per axis: -1, 0, or 1. Diagonal walking is `dir_x` and `dir_y` both moving.
    pub dir_x: i32,
    pub dir_y: i32,
    pub walk_speed: i32,
    pub climb_speed: i32,
    pub gravity: i32,
    pub max_fall_speed: i32,
    /// When false the pet keeps its walking line and only moves on x.
    pub diagonal_walk: bool,
    /// Hard limit for the window position. No state may leave it.
    pub bounds: Bounds,
    /// Where the pet stands: monitor floor, or the top of a window it climbed on.
    pub surface_y: i32,
    /// Where an active `Climb` stops. Clamped into `bounds`.
    pub climb_target_y: i32,
    /// Pet window edge in physical pixels, needed to find the feet and the
    /// surface the pet stands on.
    pub pet_px: i32,
    /// How the current frame must be turned to be drawn.
    pub rotation: Rotation,
    frame_period_ms: u64,
    last_frame_tick: u64,
    pub cursor_pos: (i32, i32),
    pub idle_timer: u32,
}

impl StateMachine {
    pub fn new(
        idle_frames: Vec<u32>,
        walk_frames: Vec<u32>,
        screen_width: u32,
        frame_period_ms: u64,
    ) -> Self {
        let screen_width = screen_width.max(1);
        let bounds = Bounds {
            min_x: 0,
            min_y: 0,
            max_x: screen_width as i32,
            max_y: screen_width as i32,
        };
        Self {
            state: PetState::Idle,
            behavior: PetBehavior::ESheep,
            frame_index: idle_frames.first().copied().unwrap_or(0),
            idle_frames,
            walk_frames,
            x: 0,
            y: 0,
            dy: 0,
            dx: 0,
            dir_x: 1,
            dir_y: 1,
            walk_speed: 5,
            climb_speed: 2,
            gravity: 5,
            max_fall_speed: 40,
            diagonal_walk: true,
            bounds,
            surface_y: bounds.max_y,
            climb_target_y: bounds.min_y,
            pet_px: 128,
            rotation: Rotation::Upright,
            frame_period_ms,
            last_frame_tick: 0,
            cursor_pos: (0, 0),
            idle_timer: 0,
        }
    }

    /// Advance animation and movement from the monotonic `elapsed_ms` clock.
    ///
    /// Movement is one step per animation frame rather than one step per call,
    /// so a 360 Hz redraw does not walk six times faster than a 60 Hz one.
    pub fn tick(&mut self, elapsed_ms: u64) {
        let frames = match self.state {
            PetState::Idle | PetState::Sleep => &self.idle_frames,
            PetState::Walk
            | PetState::Chase
            | PetState::Drag
            | PetState::Fall
            | PetState::Climb => &self.walk_frames,
        };

        if frames.is_empty() {
            return;
        }

        let period = self.frame_period_ms.max(1);
        let due = elapsed_ms.saturating_sub(self.last_frame_tick) / period;

        if due > 0 {
            let offset = due % frames.len() as u64;
            let pos = frames
                .iter()
                .position(|&f| f == self.frame_index)
                .unwrap_or(0);
            let new_pos = (pos + offset as usize) % frames.len();
            self.frame_index = frames[new_pos];
            self.last_frame_tick += due * period;

            // Count animation frames, not redraws: at 300 fps a redraw counter
            // would put a neko to sleep in under two seconds.
            if self.behavior == PetBehavior::Neko {
                self.idle_timer = self.idle_timer.saturating_add(1);
            }
        }

        let steps = due.min(u64::from(MAX_STEPS_PER_TICK)) as i32;

        if steps > 0 {
            match self.state {
                PetState::Walk => self.walk_step(steps),
                PetState::Chase => self.chase_step(steps),
                PetState::Fall => self.fall_step(steps),
                PetState::Climb => self.climb_step(steps),
                PetState::Idle | PetState::Sleep | PetState::Drag => {}
            }

            // Hard limit. Every state ends inside the bounds, so no combination
            // of window sizes, monitor layouts, or transitions can strand the
            // pet off the desktop.
            self.x = self.bounds.clamp_x(self.x);
            self.y = self.bounds.clamp_y(self.y);
        }

        if self.behavior == PetBehavior::Neko
            && self.idle_timer > IDLE_FRAMES_BEFORE_SLEEP
            && self.state != PetState::Sleep
        {
            self.set_state(PetState::Sleep);
        }
    }

    /// Walk along `dir_x`, and along `dir_y` too when diagonal walking is on.
    fn walk_step(&mut self, steps: i32) {
        let step_x = self.dir_x.signum() * self.walk_speed * steps;
        let step_y = if self.diagonal_walk {
            self.dir_y.signum() * self.walk_speed * steps
        } else {
            0
        };

        self.x += step_x;
        self.y += step_y;

        if step_x > 0 && self.x >= self.bounds.max_x {
            self.x = self.bounds.max_x;
            self.dir_x = -1;
        } else if step_x < 0 && self.x <= self.bounds.min_x {
            self.x = self.bounds.min_x;
            self.dir_x = 1;
        }

        if step_y > 0 && self.y >= self.bounds.max_y {
            self.y = self.bounds.max_y;
            self.dir_y = -1;
        } else if step_y < 0 && self.y <= self.bounds.min_y {
            self.y = self.bounds.min_y;
            self.dir_y = 1;
        }

        if !self.diagonal_walk {
            if self.y < self.surface_y {
                // The floor moved out from under the pet.
                self.set_state(PetState::Fall);
            } else {
                self.y = self.surface_y;
            }
        }
    }

    fn chase_step(&mut self, steps: i32) {
        if self.behavior != PetBehavior::Neko {
            return;
        }
        if self.cursor_pos.0 > self.x {
            self.dir_x = 1;
        } else if self.cursor_pos.0 < self.x {
            self.dir_x = -1;
        }
        self.x += self.dir_x * 2 * steps;
    }

    /// Gravity, horizontal throw, and the bounce that settles on the surface.
    fn fall_step(&mut self, steps: i32) {
        for _ in 0..steps {
            self.dy = (self.dy + self.gravity).min(self.max_fall_speed);
            self.x += self.dx;
            self.dx -= self.dx / 8;
            self.y += self.dy;

            if self.y >= self.surface_y {
                self.y = self.surface_y;
                let bounce = -(self.dy / 2);
                if bounce.abs() <= 1 {
                    self.dy = 0;
                    self.dx = 0;
                    self.set_state(PetState::Idle);
                    break;
                }
                self.dy = bounce;
            }
        }
    }

    /// Climb up to the target, then walk on whatever surface is up there.
    fn climb_step(&mut self, steps: i32) {
        if self.behavior == PetBehavior::Neko {
            self.set_state(PetState::Walk);
            return;
        }
        self.y -= self.climb_speed * steps;
        if self.y <= self.climb_target_y {
            self.y = self.climb_target_y;
            self.set_state(PetState::Walk);
        }
    }

    pub fn set_state(&mut self, state: PetState) {
        if self.state != state {
            self.state = state;
            // Climbing turns the pet on its side, the way it looks scaling a
            // window edge. Every other state draws it upright.
            self.rotation = if state == PetState::Climb {
                Rotation::QuarterTurn
            } else {
                Rotation::Upright
            };
            let frames = match state {
                PetState::Idle | PetState::Sleep => &self.idle_frames,
                PetState::Walk
                | PetState::Chase
                | PetState::Drag
                | PetState::Fall
                | PetState::Climb => &self.walk_frames,
            };
            self.frame_index = frames.first().copied().unwrap_or(0);
        }
    }

    /// Hard limit for the window position. Applied to every state, so no state
    /// can strand the pet off the desktop. Also pulls a stranded pet back.
    pub fn set_bounds(&mut self, bounds: Bounds) {
        self.bounds = bounds;
        self.x = self.bounds.clamp_x(self.x);
        self.y = self.bounds.clamp_y(self.y);
        self.surface_y = self.bounds.clamp_y(self.surface_y);
        self.climb_target_y = self.bounds.clamp_y(self.climb_target_y);
    }

    /// Where the pet stands: the monitor floor, or the top edge of a window.
    pub fn set_surface(&mut self, surface_y: i32) {
        self.surface_y = self.bounds.clamp_y(surface_y);
    }

    /// Where an active `Climb` stops.
    pub fn set_climb_target(&mut self, target_y: i32) {
        self.climb_target_y = self.bounds.clamp_y(target_y);
    }

    /// Pet window edge in physical pixels. The feet sit one pet height below
    /// the window's top-left corner.
    pub fn set_pet_size(&mut self, pet_px: i32) {
        self.pet_px = pet_px.max(1);
    }

    /// Screen y of the pet's feet.
    pub fn feet_y(&self) -> i32 {
        self.y + self.pet_px
    }

    /// True when the pet is standing on the top edge of `window`.
    pub fn stands_on(&self, window: &WindowRect, tolerance: i32) -> bool {
        if self.state == PetState::Drag {
            return false;
        }
        let feet = self.feet_y();
        (feet - window.y).abs() <= tolerance
            && self.x + self.pet_px > window.left()
            && self.x < window.right()
    }

    /// Carry the pet along when the user drags the window it is standing on.
    ///
    /// Without this the pet stays behind in mid air and the next frame drops
    /// it to the floor. Returns true when the pet rode the window.
    pub fn ride_platform(
        &mut self,
        previous: &WindowRect,
        current: &WindowRect,
        tolerance: i32,
    ) -> bool {
        if !self.stands_on(previous, tolerance) {
            return false;
        }

        let (dx, dy) = current.delta(previous);
        if dx != 0 {
            self.x = self.bounds.clamp_x(self.x + dx);
        }
        if dy != 0 {
            self.y = self.bounds.clamp_y(self.y + dy);
        }

        // Keep standing on the new top edge rather than on the old one.
        let ledge_y = self.bounds.clamp_y(current.y - self.pet_px);
        self.surface_y = ledge_y;
        self.climb_target_y = ledge_y;
        if !self.diagonal_walk {
            self.y = ledge_y;
        }
        true
    }

    /// Direction per axis, taken as a sign so callers can pass any velocity.
    pub fn set_direction(&mut self, dir_x: i32, dir_y: i32) {
        self.dir_x = dir_x.signum();
        self.dir_y = dir_y.signum();
    }

    pub fn set_walk_speed(&mut self, speed: i32) {
        self.walk_speed = speed.clamp(1, 20);
    }

    pub fn set_diagonal_walk(&mut self, on: bool) {
        self.diagonal_walk = on;
    }

    pub fn y(&self) -> i32 {
        self.y
    }

    pub fn climb_target_y(&self) -> i32 {
        self.climb_target_y
    }

    pub fn state(&self) -> PetState {
        self.state
    }

    pub fn frame_index(&self) -> u32 {
        self.frame_index
    }

    pub fn x(&self) -> i32 {
        self.x
    }
}

pub fn run_headless(
    idle_frames: Vec<u32>,
    walk_frames: Vec<u32>,
    screen_width: u32,
    ticks: u64,
    frame_period_ms: u64,
) -> Vec<(u32, i32)> {
    let mut sm = StateMachine::new(idle_frames, walk_frames, screen_width, frame_period_ms);
    sm.set_state(PetState::Walk);
    let mut results = Vec::new();
    for tick_ms in 1..=ticks {
        sm.tick(tick_ms * frame_period_ms);
        results.push((sm.frame_index(), sm.x()));
    }
    results
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn state_machine_starts_idle() {
        let sm = StateMachine::new(vec![0, 1], vec![2, 3], 100, 150);
        assert_eq!(sm.state(), PetState::Idle);
        assert_eq!(sm.frame_index(), 0);
    }

    #[test]
    fn idle_cycles_frames_at_150ms() {
        let mut sm = StateMachine::new(vec![0, 1, 2, 3], vec![4, 5, 6, 7], 100, 150);
        sm.tick(150);
        assert_eq!(sm.frame_index(), 1);
        sm.tick(300);
        assert_eq!(sm.frame_index(), 2);
        sm.tick(450);
        assert_eq!(sm.frame_index(), 3);
        sm.tick(600);
        assert_eq!(sm.frame_index(), 0);
    }

    #[test]
    fn walk_advances_frame_and_position() {
        let mut sm = StateMachine::new(vec![0], vec![0, 1, 2, 3], 200, 150);
        sm.set_state(PetState::Walk);
        sm.tick(150);
        assert_eq!(sm.frame_index(), 1);
        assert_eq!(sm.x(), 5);
    }

    #[test]
    fn walk_bounces_off_the_right_wall() {
        let mut sm = StateMachine::new(vec![0], vec![0, 1], 100, 100);
        sm.set_bounds(Bounds {
            min_x: 0,
            min_y: 0,
            max_x: 10,
            max_y: 100,
        });
        sm.set_diagonal_walk(false);
        sm.set_surface(0);
        sm.set_direction(1, 0);
        sm.set_state(PetState::Walk);

        sm.tick(100);
        assert_eq!(sm.x(), 5, "walks right");
        sm.tick(200);
        assert_eq!(sm.x(), 10, "stops at the wall instead of wrapping");
        sm.tick(300);
        assert_eq!(sm.x(), 5, "turns around after reaching the wall");
    }

    #[test]
    fn state_transition_resets_frame() {
        let mut sm = StateMachine::new(vec![0, 1, 2], vec![3, 4], 100, 150);
        sm.tick(300);
        assert_eq!(sm.frame_index(), 2);
        sm.set_state(PetState::Walk);
        assert_eq!(sm.frame_index(), 3);
        sm.set_state(PetState::Idle);
        assert_eq!(sm.frame_index(), 0);
    }

    #[test]
    fn no_frame_advancement_without_enough_time() {
        let mut sm = StateMachine::new(vec![0, 1], vec![2, 3], 100, 150);
        sm.tick(50);
        assert_eq!(sm.frame_index(), 0);
    }

    #[test]
    fn multiple_frames_advance_in_large_tick() {
        let mut sm = StateMachine::new(vec![0, 1, 2, 3], vec![4, 5, 6, 7], 100, 150);
        sm.tick(600);
        assert_eq!(sm.frame_index(), 0);
    }

    #[test]
    fn walk_does_not_move_without_state() {
        let mut sm = StateMachine::new(vec![0], vec![0, 1], 100, 150);
        sm.tick(150);
        assert_eq!(sm.x(), 0);
    }

    #[test]
    fn headless_produces_expected_frame_sequence() {
        let results = run_headless(vec![0, 1], vec![2, 3, 4, 5], 10, 4, 150);
        let expected = vec![(3, 5), (4, 10), (5, 5), (2, 0)];
        assert_eq!(results, expected);
    }

    fn walking_machine(bounds: Bounds) -> StateMachine {
        let mut sm = StateMachine::new(vec![0], vec![0, 1], 100, 100);
        sm.set_bounds(bounds);
        sm.set_state(PetState::Walk);
        sm
    }

    #[test]
    fn diagonal_walk_moves_on_both_axes() {
        let mut sm = walking_machine(Bounds {
            min_x: 0,
            min_y: 0,
            max_x: 1000,
            max_y: 1000,
        });
        sm.set_direction(1, 1);
        sm.tick(100);
        assert_eq!((sm.x(), sm.y()), (5, 5));
    }

    #[test]
    fn non_diagonal_walk_keeps_the_walking_line() {
        let mut sm = walking_machine(Bounds {
            min_x: 0,
            min_y: 0,
            max_x: 1000,
            max_y: 1000,
        });
        sm.set_diagonal_walk(false);
        sm.set_surface(0);
        sm.set_direction(1, 1);
        sm.tick(100);
        assert_eq!(sm.x(), 5);
        assert_eq!(sm.y(), 0, "stays on the surface while walking");
    }

    #[test]
    fn walk_bounces_off_the_floor_and_the_ceiling() {
        let mut sm = walking_machine(Bounds {
            min_x: 0,
            min_y: 0,
            max_x: 1000,
            max_y: 20,
        });
        sm.set_direction(0, 1);
        for step in 1..=4 {
            sm.tick(step * 100);
        }
        assert_eq!(sm.y(), 20, "stops at the floor");
        sm.tick(500);
        assert_eq!(sm.y(), 15, "turns around after the floor");
    }

    #[test]
    fn walk_never_leaves_the_bounds() {
        let bounds = Bounds {
            min_x: -100,
            min_y: -50,
            max_x: 300,
            max_y: 150,
        };
        let mut sm = walking_machine(bounds);
        sm.set_direction(1, 1);
        for step in 1..=5000 {
            sm.tick(step * 100);
            assert!(
                bounds.contains(sm.x(), sm.y()),
                "escaped at step {step}: ({}, {})",
                sm.x(),
                sm.y()
            );
        }
    }

    #[test]
    fn climb_stops_on_the_target_and_never_leaves_the_bounds() {
        let bounds = Bounds {
            min_x: 0,
            min_y: 0,
            max_x: 500,
            max_y: 900,
        };
        let mut sm = StateMachine::new(vec![0], vec![0, 1], 100, 100);
        sm.set_bounds(bounds);
        sm.set_surface(400);
        sm.set_diagonal_walk(false);
        sm.y = 900;
        sm.set_climb_target(400);
        sm.set_state(PetState::Climb);
        for step in 1..=300 {
            sm.tick(step * 100);
            assert!(
                bounds.contains(sm.x(), sm.y()),
                "climb left the screen at step {step}: ({}, {})",
                sm.x(),
                sm.y()
            );
        }
        assert_eq!(sm.y(), 400, "stops on top of the climbed surface");
        assert_eq!(sm.state(), PetState::Walk);
    }

    #[test]
    fn climb_target_above_the_desktop_is_clamped_to_the_ceiling() {
        let bounds = Bounds {
            min_x: 0,
            min_y: 0,
            max_x: 500,
            max_y: 900,
        };
        let mut sm = StateMachine::new(vec![0], vec![0, 1], 100, 100);
        sm.set_bounds(bounds);
        sm.set_climb_target(-500);
        assert_eq!(
            sm.climb_target_y(),
            bounds.min_y,
            "a window hanging off the top of the screen must not park the pet off-screen"
        );
    }

    #[test]
    fn fall_lands_on_the_surface_and_settles_on_idle() {
        let bounds = Bounds {
            min_x: 0,
            min_y: 0,
            max_x: 500,
            max_y: 900,
        };
        let mut sm = StateMachine::new(vec![0], vec![0, 1], 100, 100);
        sm.set_bounds(bounds);
        sm.set_surface(900);
        sm.y = 100;
        sm.behavior = PetBehavior::Neko;
        sm.set_state(PetState::Fall);
        for step in 1..=400 {
            sm.tick(step * 100);
        }
        assert_eq!(sm.y(), 900, "rests exactly on the surface");
        assert_eq!(sm.dy, 0, "the bounce settled");
        assert_eq!(sm.state(), PetState::Idle, "fall works for every behavior");
    }

    #[test]
    fn fall_applies_horizontal_throw_velocity() {
        let mut sm = StateMachine::new(vec![0], vec![0, 1], 100, 100);
        sm.set_bounds(Bounds {
            min_x: 0,
            min_y: 0,
            max_x: 500,
            max_y: 900,
        });
        sm.set_surface(900);
        sm.y = 100;
        sm.dx = 12;
        sm.set_state(PetState::Fall);
        sm.tick(100);
        assert_eq!(sm.x(), 12, "thrown sideways");
        assert!(sm.dx < 12, "sideways speed decays");
    }

    #[test]
    fn fall_cannot_push_the_pet_past_the_bounds() {
        let bounds = Bounds {
            min_x: 0,
            min_y: 0,
            max_x: 500,
            max_y: 900,
        };
        let mut sm = StateMachine::new(vec![0], vec![0, 1], 100, 100);
        sm.set_bounds(bounds);
        sm.set_surface(400);
        sm.y = 0;
        sm.dy = 1000;
        sm.set_state(PetState::Fall);
        for step in 1..=20 {
            sm.tick(step * 100);
            assert!(
                bounds.contains(sm.x(), sm.y()),
                "fall left the screen at step {step}: ({}, {})",
                sm.x(),
                sm.y()
            );
        }
    }

    #[test]
    fn set_bounds_pulls_a_stranded_pet_back_on_screen() {
        let mut sm = StateMachine::new(vec![0], vec![0, 1], 100, 100);
        sm.y = -4000;
        sm.x = 99_999;
        sm.set_bounds(Bounds {
            min_x: 0,
            min_y: 0,
            max_x: 800,
            max_y: 600,
        });
        assert_eq!((sm.x(), sm.y()), (800, 0));
    }

    #[test]
    fn late_tick_does_not_teleport_the_pet() {
        let mut sm = walking_machine(Bounds {
            min_x: 0,
            min_y: 0,
            max_x: 10_000,
            max_y: 10_000,
        });
        sm.set_direction(1, 1);
        sm.tick(100);
        assert_eq!((sm.x(), sm.y()), (5, 5));
        sm.tick(500_000);
        assert_eq!(
            (sm.x(), sm.y()),
            (25, 25),
            "at most 4 steps are applied per tick"
        );
    }

    /// A window standing under the pet: `x`, `y` is its top-left corner.
    fn window(x: i32, y: i32) -> WindowRect {
        WindowRect {
            x,
            y,
            width: 600,
            height: 400,
        }
    }

    /// A pet parked on the top edge of a window whose top is at `ledge_y`.
    fn pet_on_ledge(ledge_y: i32) -> StateMachine {
        let mut sm = StateMachine::new(vec![0], vec![0, 1], 100, 100);
        sm.set_bounds(Bounds {
            min_x: 0,
            min_y: 0,
            max_x: 1400,
            max_y: 800,
        });
        sm.set_pet_size(100);
        sm.diagonal_walk = false;
        sm.x = 300;
        sm.y = ledge_y - 100;
        sm.set_surface(ledge_y - 100);
        sm
    }

    #[test]
    fn climbing_turns_the_pet_on_its_side_and_walking_turns_it_back() {
        let mut sm = StateMachine::new(vec![0], vec![0, 1], 100, 100);
        assert_eq!(sm.rotation, Rotation::Upright, "idle is upright");

        sm.set_state(PetState::Climb);
        assert_eq!(
            sm.rotation,
            Rotation::QuarterTurn,
            "a climbing pet is turned a quarter turn"
        );

        sm.set_state(PetState::Walk);
        assert_eq!(sm.rotation, Rotation::Upright, "back on the ledge");
    }

    #[test]
    fn pet_rides_a_window_the_user_drags_sideways() {
        let before = window(200, 400);
        let after = window(260, 400);
        let mut sm = pet_on_ledge(400);

        assert!(sm.ride_platform(&before, &after, 16), "the pet rode it");
        assert_eq!(sm.x(), 360, "moved by the window's own 60 pixels");
        assert_eq!(sm.y(), 300, "still standing on the window top");
    }

    #[test]
    fn pet_rides_a_window_that_moves_up() {
        let mut sm = pet_on_ledge(500);
        let before = window(100, 500);
        let after = window(100, 420);

        assert!(sm.ride_platform(&before, &after, 16));
        assert_eq!(sm.y(), 320, "lifted 80 pixels with the window");
    }

    #[test]
    fn a_window_moved_above_the_ceiling_cannot_lift_the_pet_off_screen() {
        let mut sm = pet_on_ledge(300);
        let before = window(0, 300);
        let after = window(0, -400);

        assert!(sm.ride_platform(&before, &after, 16));
        assert_eq!(sm.y(), 0, "clamped to the ceiling, not a negative y");
    }

    #[test]
    fn a_pet_standing_elsewhere_does_not_ride_the_window() {
        let mut sm = StateMachine::new(vec![0], vec![0, 1], 100, 100);
        sm.set_bounds(Bounds {
            min_x: 0,
            min_y: 0,
            max_x: 1400,
            max_y: 800,
        });
        sm.set_pet_size(100);
        sm.x = 1000;
        sm.y = 700;

        assert!(
            !sm.ride_platform(&window(200, 400), &window(260, 400), 16),
            "a pet on the floor ignores a window move"
        );
        assert_eq!(sm.x(), 1000);
    }

    #[test]
    fn a_dragged_pet_is_not_carried_by_a_window() {
        let mut sm = pet_on_ledge(300);
        sm.set_state(PetState::Drag);

        assert!(
            !sm.ride_platform(&window(0, 300), &window(50, 300), 16),
            "the user is holding the pet, so it does not ride"
        );
        assert_eq!(sm.x(), 300);
    }

    #[test]
    fn footprints_are_stamped_by_distance_not_by_frame_count() {
        let mut prints = Footprints::new(20, 6_000);
        prints.track(0, 0, 128, 0);
        assert!(
            prints.is_empty(),
            "the first sighting only primes the trail"
        );

        prints.track(10, 0, 128, 100);
        assert!(
            prints.is_empty(),
            "a 10 pixel shuffle is under the 20 pixel spacing"
        );

        prints.track(25, 0, 128, 200);
        assert_eq!(prints.marks().len(), 1, "one mark after 20 pixels");
        assert_eq!(prints.marks()[0].x, 25 - 32, "laid by the left foot");

        prints.track(90, 0, 128, 300);
        assert_eq!(prints.marks().len(), 2, "a second mark 65 pixels later");
        assert_eq!(
            prints.marks()[1].x,
            90 + 32,
            "the second mark comes from the other foot"
        );
    }

    #[test]
    fn footprints_dry_up_after_their_lifetime() {
        let mut prints = Footprints::new(10, 1_000);
        prints.track(0, 0, 128, 0);
        prints.track(50, 0, 128, 100);
        prints.track(100, 0, 128, 200);
        assert_eq!(prints.marks().len(), 2);

        prints.expire(1_200);
        assert!(
            prints.is_empty(),
            "both marks are older than the one second lifetime"
        );

        let mut fresh = Footprints::new(10, 1_000);
        fresh.track(0, 0, 128, 0);
        fresh.track(10, 0, 128, 900);
        fresh.expire(1_100);
        assert_eq!(fresh.marks().len(), 1, "the young mark survives");
    }

    #[test]
    fn a_pet_that_alternates_its_foot_does_not_stamp_a_mark_every_frame() {
        // A pet standing still must leave no trail. The tracker measures travel
        // from the pet's centre, so this holds however the foot alternates.
        let mut prints = Footprints::new(24, 6_000);
        prints.track(100, 100, 128, 0);

        for frame in 1..=60u64 {
            prints.track(100, 100, 128, frame * 16);
        }

        assert!(
            prints.marks().is_empty(),
            "a pet standing still leaves no trail, got {} marks",
            prints.marks().len()
        );
    }

    #[test]
    fn a_walking_pet_leaves_marks_alternating_between_its_feet() {
        let mut prints = Footprints::new(24, 6_000);
        prints.track(100, 100, 128, 0);

        // One spacing of travel per frame, so every frame lays a mark.
        for frame in 1..=6u64 {
            prints.track(100 + (frame as i32) * 24, 100, 128, frame * 16);
        }

        let xs: Vec<i32> = prints.marks().iter().map(|mark| mark.x).collect();
        assert_eq!(xs.len(), 6, "24 pixels of travel every frame, got {xs:?}");

        // Consecutive marks jump forward then backward: the foot offset (64)
        // alternates around the steady 24 pixel step of the body.
        let steps: Vec<i32> = xs.windows(2).map(|pair| pair[1] - pair[0]).collect();
        assert_eq!(
            steps,
            vec![88, -40, 88, -40, 88],
            "the trail alternates between the left and the right foot"
        );
    }

    #[test]
    fn neko_sleep_counts_animation_frames_not_redraws() {
        const PERIOD: u64 = 100;

        let mut sleeping = StateMachine::new(vec![0, 1], vec![0, 1], 100, PERIOD);
        sleeping.behavior = PetBehavior::Neko;
        sleeping.set_state(PetState::Walk);
        for step in 1..=IDLE_FRAMES_BEFORE_SLEEP + 1 {
            sleeping.tick(u64::from(step) * PERIOD);
        }
        assert_eq!(sleeping.state(), PetState::Sleep, "500 idle frames");

        // Redraws inside one frame period must not count toward sleep.
        let mut awake = StateMachine::new(vec![0, 1], vec![0, 1], 100, PERIOD);
        awake.behavior = PetBehavior::Neko;
        awake.set_state(PetState::Walk);
        awake.tick(PERIOD);
        for _ in 0..5000 {
            awake.tick(PERIOD);
        }
        assert!(
            awake.idle_timer <= 2,
            "redraws without a frame advance must not accumulate, got {}",
            awake.idle_timer
        );
        assert_eq!(awake.state(), PetState::Walk);
    }
}
