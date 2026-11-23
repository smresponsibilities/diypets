//! Reactive Developer Companion Modes (T-010 Typing Cadence & T-011 CPU Build Sprint).
//!
//! Privacy & AV-safety design:
//! - T-010 NEVER installs a keyboard hook and NEVER reads keycodes or characters.
//!   On Windows it checks `GetLastInputInfo` timestamps combined with cursor stillness
//!   to detect typing/click rhythm safely without triggering antivirus false positives.
//! - T-011 reads native OS CPU counters (`GetSystemTimes` on Windows, `/proc/stat` on Linux,
//!   `getloadavg` on macOS) with zero external dependencies and ~0% overhead.

use std::collections::VecDeque;
use std::time::{Duration, Instant};

const TAP_WINDOW: Duration = Duration::from_secs(2);
const HOP_COOLDOWN: Duration = Duration::from_millis(240);
const CPU_POLL_INTERVAL: Duration = Duration::from_millis(1000);
const INPUT_POLL_INTERVAL: Duration = Duration::from_millis(55);

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ReactiveOutput {
    /// True on beats where active pets should play a rhythmic Bongo hop/bounce.
    pub trigger_hop: bool,
    /// Walk speed multiplier (`0.85..=2.20`) driven by live CPU load when `cpu_sprint` is enabled.
    pub speed_multiplier: f32,
    /// Smoothed CPU usage percentage (`0..=100`).
    pub cpu_percent: u8,
    /// Recent privacy-safe input cadence in taps/sec.
    pub taps_per_sec: f32,
    /// True whenever `speed_multiplier` changed enough that pet walk speeds should be refreshed.
    pub speed_dirty: bool,
}

/// Map normalized CPU utilization (`0.0..=1.0`) to a RunCat-style walk/sprint multiplier.
pub fn cpu_load_to_speed_multiplier(cpu_fraction: f32) -> f32 {
    let c = cpu_fraction.clamp(0.0, 1.0);
    if c < 0.15 {
        0.85
    } else if c < 0.35 {
        1.0
    } else {
        let t = ((c - 0.35) / 0.60).clamp(0.0, 1.0);
        1.0 + t * 1.2
    }
}

#[derive(Debug, Default)]
struct CpuSampler {
    #[cfg(windows)]
    prev_times: Option<(u64, u64, u64)>,
    #[cfg(target_os = "linux")]
    prev_stat: Option<(u64, u64)>,
}

impl CpuSampler {
    fn sample_fraction(&mut self) -> Option<f32> {
        #[cfg(windows)]
        {
            use windows::Win32::Foundation::FILETIME;
            use windows::Win32::System::Threading::GetSystemTimes;

            let ft_u64 = |ft: FILETIME| ((ft.dwHighDateTime as u64) << 32) | (ft.dwLowDateTime as u64);
            let mut idle = FILETIME::default();
            let mut kernel = FILETIME::default();
            let mut user = FILETIME::default();
            let ok = unsafe {
                GetSystemTimes(Some(&mut idle), Some(&mut kernel), Some(&mut user)).is_ok()
            };
            if !ok {
                return None;
            }
            let cur = (ft_u64(idle), ft_u64(kernel), ft_u64(user));
            let prev = self.prev_times.replace(cur)?;
            let idle_d = cur.0.saturating_sub(prev.0);
            let kernel_d = cur.1.saturating_sub(prev.1);
            let user_d = cur.2.saturating_sub(prev.2);
            let total = kernel_d.saturating_add(user_d);
            if total == 0 {
                return Some(0.0);
            }
            let busy = total.saturating_sub(idle_d);
            Some((busy as f32 / total as f32).clamp(0.0, 1.0))
        }

        #[cfg(target_os = "linux")]
        {
            let content = std::fs::read_to_string("/proc/stat").ok()?;
            let first = content.lines().next()?;
            let mut nums = first
                .split_whitespace()
                .skip(1)
                .filter_map(|s| s.parse::<u64>().ok());
            let user = nums.next()?;
            let nice = nums.next()?;
            let system = nums.next()?;
            let idle = nums.next()?;
            let iowait = nums.next().unwrap_or(0);
            let irq = nums.next().unwrap_or(0);
            let softirq = nums.next().unwrap_or(0);
            let steal = nums.next().unwrap_or(0);

            let idle_all = idle.saturating_add(iowait);
            let total = user
                .saturating_add(nice)
                .saturating_add(system)
                .saturating_add(idle_all)
                .saturating_add(irq)
                .saturating_add(softirq)
                .saturating_add(steal);
            let prev = self.prev_stat.replace((idle_all, total))?;
            let total_d = total.saturating_sub(prev.1);
            let idle_d = idle_all.saturating_sub(prev.0);
            if total_d == 0 {
                return Some(0.0);
            }
            let busy = total_d.saturating_sub(idle_d);
            Some((busy as f32 / total_d as f32).clamp(0.0, 1.0))
        }

        #[cfg(target_os = "macos")]
        {
            unsafe extern "C" {
                fn getloadavg(loadavg: *mut f64, nelem: i32) -> i32;
            }
            let mut avg = [0.0f64; 1];
            let n = unsafe { getloadavg(avg.as_mut_ptr(), 1) };
            if n < 1 {
                return None;
            }
            let cores = std::thread::available_parallelism()
                .map(|c| c.get() as f64)
                .unwrap_or(4.0);
            Some(((avg[0] / cores) as f32).clamp(0.0, 1.0))
        }

        #[cfg(not(any(windows, target_os = "linux", target_os = "macos")))]
        {
            None
        }
    }
}

#[derive(Debug, Default)]
struct InputCadencePoller {
    prev_tick: Option<u32>,
    prev_cursor: Option<(i32, i32)>,
}

impl InputCadencePoller {
    fn feed_sample(&mut self, input_tick: u32, cursor: Option<(i32, i32)>) -> bool {
        let Some(last_tick) = self.prev_tick else {
            self.prev_tick = Some(input_tick);
            self.prev_cursor = cursor;
            return false;
        };

        let moved_mouse = match (self.prev_cursor, cursor) {
            (Some((px, py)), Some((cx, cy))) => (cx - px).abs() > 2 || (cy - py).abs() > 2,
            _ => false,
        };
        self.prev_tick = Some(input_tick);
        self.prev_cursor = cursor;

        // Only count as a typing/click tap when OS input tick advanced without passive mouse gliding.
        input_tick != last_tick && !moved_mouse
    }

    fn poll_os_tap(&mut self) -> bool {
        #[cfg(windows)]
        {
            use windows::Win32::Foundation::POINT;
            use windows::Win32::UI::Input::KeyboardAndMouse::{GetLastInputInfo, LASTINPUTINFO};
            use windows::Win32::UI::WindowsAndMessaging::GetCursorPos;

            let mut info = LASTINPUTINFO {
                cbSize: std::mem::size_of::<LASTINPUTINFO>() as u32,
                dwTime: 0,
            };
            let ok = unsafe { GetLastInputInfo(&mut info).as_bool() };
            if !ok {
                return false;
            }
            let mut pt = POINT::default();
            let cursor = unsafe {
                if GetCursorPos(&mut pt).is_ok() {
                    Some((pt.x, pt.y))
                } else {
                    None
                }
            };
            self.feed_sample(info.dwTime, cursor)
        }

        #[cfg(not(windows))]
        {
            false
        }
    }
}

#[derive(Debug)]
pub struct ReactiveCompanion {
    cpu_sampler: CpuSampler,
    input_poller: InputCadencePoller,
    tap_times: VecDeque<Instant>,
    last_hop: Option<Instant>,
    last_cpu_poll: Option<Instant>,
    last_input_poll: Option<Instant>,
    smoothed_cpu: f32,
    applied_multiplier: f32,
    was_cpu_sprint: bool,
}

impl Default for ReactiveCompanion {
    fn default() -> Self {
        Self::new()
    }
}

impl ReactiveCompanion {
    pub fn new() -> Self {
        Self {
            cpu_sampler: CpuSampler::default(),
            input_poller: InputCadencePoller::default(),
            tap_times: VecDeque::with_capacity(32),
            last_hop: None,
            last_cpu_poll: None,
            last_input_poll: None,
            smoothed_cpu: 0.25,
            applied_multiplier: 1.0,
            was_cpu_sprint: false,
        }
    }

    /// Record a privacy-safe tap pulse (used internally by OS polling and unit tests).
    pub fn record_tap(&mut self, now: Instant) -> bool {
        self.prune_taps(now);
        self.tap_times.push_back(now);
        let ready = self
            .last_hop
            .is_none_or(|prev| now.saturating_duration_since(prev) >= HOP_COOLDOWN);
        if ready {
            self.last_hop = Some(now);
            true
        } else {
            false
        }
    }

    fn prune_taps(&mut self, now: Instant) {
        while let Some(&front) = self.tap_times.front() {
            if now.saturating_duration_since(front) > TAP_WINDOW {
                self.tap_times.pop_front();
            } else {
                break;
            }
        }
    }

    /// Advance reactive state once per runner tick.
    pub fn update(
        &mut self,
        now: Instant,
        typing_reflex: bool,
        cpu_sprint: bool,
    ) -> ReactiveOutput {
        let mut trigger_hop = false;
        let mut speed_dirty = false;

        if typing_reflex {
            let poll_due = self
                .last_input_poll
                .is_none_or(|prev| now.saturating_duration_since(prev) >= INPUT_POLL_INTERVAL);
            if poll_due {
                self.last_input_poll = Some(now);
                if self.input_poller.poll_os_tap() {
                    trigger_hop = self.record_tap(now);
                } else {
                    self.prune_taps(now);
                }
            }
        } else if !self.tap_times.is_empty() {
            self.tap_times.clear();
        }

        if cpu_sprint {
            let cpu_due = self
                .last_cpu_poll
                .is_none_or(|prev| now.saturating_duration_since(prev) >= CPU_POLL_INTERVAL);
            if cpu_due {
                self.last_cpu_poll = Some(now);
                if let Some(sample) = self.cpu_sampler.sample_fraction() {
                    self.smoothed_cpu = self.smoothed_cpu * 0.55 + sample * 0.45;
                    let target_mult = cpu_load_to_speed_multiplier(self.smoothed_cpu);
                    if (target_mult - self.applied_multiplier).abs() >= 0.04 || !self.was_cpu_sprint
                    {
                        self.applied_multiplier = target_mult;
                        speed_dirty = true;
                    }
                }
            }
            if !self.was_cpu_sprint {
                self.was_cpu_sprint = true;
                speed_dirty = true;
            }
        } else if self.was_cpu_sprint {
            self.was_cpu_sprint = false;
            self.applied_multiplier = 1.0;
            speed_dirty = true;
        }

        let taps_per_sec = self.tap_times.len() as f32 / TAP_WINDOW.as_secs_f32();
        ReactiveOutput {
            trigger_hop,
            speed_multiplier: self.applied_multiplier,
            cpu_percent: (self.smoothed_cpu * 100.0).round().clamp(0.0, 100.0) as u8,
            taps_per_sec,
            speed_dirty,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cpu_load_maps_idle_normal_and_build_spike_multipliers() {
        assert!((cpu_load_to_speed_multiplier(0.05) - 0.85).abs() < 0.01);
        assert!((cpu_load_to_speed_multiplier(0.25) - 1.0).abs() < 0.01);
        let build_spike = cpu_load_to_speed_multiplier(0.95);
        assert!((2.15..=2.21).contains(&build_spike));
    }

    #[test]
    fn typing_cadence_triggers_rhythmic_hops_and_respects_cooldown() {
        let mut comp = ReactiveCompanion::new();
        let t0 = Instant::now();
        assert!(comp.record_tap(t0));
        // Too soon (< 240ms cooldown) -> no double jitter
        assert!(!comp.record_tap(t0 + Duration::from_millis(80)));
        // Next beat (>= 240ms) -> crisp rhythmic hop
        assert!(comp.record_tap(t0 + Duration::from_millis(260)));
    }

    #[test]
    fn input_poller_ignores_passive_cursor_movement_without_keystroke() {
        let mut poller = InputCadencePoller::default();
        assert!(!poller.feed_sample(1000, Some((500, 500))));
        // Cursor moved 40px -> passive mouse glide, NOT a typing/click tap
        assert!(!poller.feed_sample(1050, Some((540, 500))));
        // Input tick advanced while cursor stayed still -> keystroke or click tap!
        assert!(poller.feed_sample(1120, Some((540, 500))));
        // Same input tick -> idle
        assert!(!poller.feed_sample(1120, Some((540, 500))));
    }

    #[test]
    fn disabling_cpu_sprint_resets_multiplier_to_unit_speed() {
        let mut comp = ReactiveCompanion::new();
        let t0 = Instant::now();
        comp.was_cpu_sprint = true;
        comp.applied_multiplier = 2.0;
        let out = comp.update(t0, false, false);
        assert!(out.speed_dirty);
        assert!((out.speed_multiplier - 1.0).abs() < 0.001);
    }
}
