//! Reactive Developer Companion Modes:
//! - T-010: Privacy-Safe Typing Cadence Reflex (Bongo bounce, never logs keys)
//! - T-011: Live CPU Build Sprint (RunCat walk acceleration)
//! - Process-Spike Radar: Native zero-bloat culprit detector that calls out runaway
//!   processes (`CPU 91%: cargo`) when CPU >= 85% with a 25s anti-spam cooldown.

#[cfg(any(windows, target_os = "linux"))]
use std::collections::HashMap;
use std::collections::VecDeque;
use std::time::{Duration, Instant};

const TAP_WINDOW: Duration = Duration::from_secs(2);
const HOP_COOLDOWN: Duration = Duration::from_millis(240);
const CPU_POLL_INTERVAL: Duration = Duration::from_millis(1000);
const INPUT_POLL_INTERVAL: Duration = Duration::from_millis(55);
const SPIKE_WARMUP_THRESHOLD: f32 = 0.70;
const SPIKE_ALERT_THRESHOLD: f32 = 0.82;
const SPIKE_RELEASE_THRESHOLD: f32 = 0.65;
const SPIKE_SUSTAIN_TICKS: u8 = 3;
const RAM_ALERT_THRESHOLD: f32 = 0.92;
const SPIKE_ALERT_COOLDOWN: Duration = Duration::from_secs(25);

#[derive(Debug, Clone, PartialEq)]
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
    /// Optional ambient radar speech bubble when CPU >= 85% (e.g. `"CPU 91%: cargo"`).
    pub spike_alert: Option<String>,
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

/// Clean a raw OS executable/process name for a compact pet speech bubble.
pub fn sanitize_process_label(raw: &str) -> Option<String> {
    let base = raw.trim().rsplit(['\\', '/']).next().unwrap_or("").trim();
    let stripped = base
        .strip_suffix(".exe")
        .or_else(|| base.strip_suffix(".EXE"))
        .unwrap_or(base)
        .trim();
    if stripped.is_empty() {
        return None;
    }
    let low = stripped.to_ascii_lowercase();
    if matches!(
        low.as_str(),
        "system" | "system idle process" | "idle" | "registry" | "memory compression"
    ) {
        return None;
    }
    let clean: String = stripped
        .chars()
        .filter(|c| !c.is_control())
        .take(20)
        .collect();
    if clean.is_empty() { None } else { Some(clean) }
}

/// Select the top non-idle process from `(name, cpu_delta)` pairs.
pub fn pick_top_culprit(deltas: &[(String, u64)]) -> Option<String> {
    deltas
        .iter()
        .filter(|(_, d)| *d > 0)
        .filter_map(|(name, d)| sanitize_process_label(name).map(|clean| (clean, *d)))
        .max_by_key(|(_, d)| *d)
        .map(|(clean, _)| clean)
}

/// Native zero-crate physical RAM utilization sampler (`0.0..=1.0`).
fn sample_ram_usage_fraction() -> Option<f32> {
    #[cfg(windows)]
    {
        use windows::Win32::System::SystemInformation::{GlobalMemoryStatusEx, MEMORYSTATUSEX};
        let mut status = MEMORYSTATUSEX {
            dwLength: std::mem::size_of::<MEMORYSTATUSEX>() as u32,
            ..Default::default()
        };
        if unsafe { GlobalMemoryStatusEx(&mut status) }.is_ok() {
            return Some((status.dwMemoryLoad as f32 / 100.0).clamp(0.0, 1.0));
        }
        None
    }
    #[cfg(target_os = "linux")]
    {
        let txt = std::fs::read_to_string("/proc/meminfo").ok()?;
        let mut total = 0u64;
        let mut avail = 0u64;
        for line in txt.lines() {
            if let Some(rest) = line.strip_prefix("MemTotal:") {
                total = rest.split_whitespace().next()?.parse().ok()?;
            } else if let Some(rest) = line.strip_prefix("MemAvailable:") {
                avail = rest.split_whitespace().next()?.parse().ok()?;
            }
        }
        if total == 0 {
            return None;
        }
        Some((total.saturating_sub(avail) as f32 / total as f32).clamp(0.0, 1.0))
    }
    #[cfg(target_os = "macos")]
    {
        let vm = std::process::Command::new("vm_stat").output().ok()?;
        let mem = std::process::Command::new("sysctl")
            .args(["-n", "hw.memsize"])
            .output()
            .ok()?;
        let total_bytes = String::from_utf8_lossy(&mem.stdout)
            .trim()
            .parse::<u64>()
            .ok()?;
        parse_macos_vm_stat(&String::from_utf8_lossy(&vm.stdout), total_bytes)
    }
    #[cfg(not(any(windows, target_os = "linux", target_os = "macos")))]
    {
        None
    }
}

/// Pure parser for macOS `vm_stat` output + `hw.memsize` bytes -> used RAM fraction (`0.0..=1.0`).
pub fn parse_macos_vm_stat(vm_stat_text: &str, total_bytes: u64) -> Option<f32> {
    if total_bytes == 0 {
        return None;
    }
    let mut page_size: u64 = 4096;
    let mut active: u64 = 0;
    let mut wired: u64 = 0;
    let mut compressed: u64 = 0;
    for line in vm_stat_text.lines() {
        let trimmed = line.trim();
        if let Some(idx) = trimmed.find("page size of ") {
            let after = &trimmed[idx + "page size of ".len()..];
            if let Some(num) = after.split_whitespace().next()
                && let Ok(ps) = num.parse::<u64>()
                && ps > 0
            {
                page_size = ps;
            }
        } else if let Some((k, v)) = trimmed.split_once(':') {
            let val = v.trim().trim_end_matches('.').parse::<u64>().unwrap_or(0);
            match k.trim() {
                "Pages active" => active = val,
                "Pages wired down" => wired = val,
                "Pages occupied by compressor" => compressed = val,
                _ => {}
            }
        }
    }
    let used_bytes = active
        .saturating_add(wired)
        .saturating_add(compressed)
        .saturating_mul(page_size);
    if used_bytes == 0 {
        return None;
    }
    Some((used_bytes as f64 / total_bytes as f64).clamp(0.0, 1.0) as f32)
}

/// Pure parser for Linux `/proc/interrupts` keyboard IRQ totals (works on X11 & Wayland, never logs keys).
pub fn parse_linux_kbd_interrupts(proc_interrupts: &str) -> Option<u64> {
    let mut total: u64 = 0;
    let mut found = false;
    for line in proc_interrupts.lines() {
        let low = line.to_ascii_lowercase();
        if !(low.contains("i8042") || low.contains("atkbd") || low.contains("keyboard")) {
            continue;
        }
        let mut parts = line.split_whitespace();
        let Some(irq) = parts.next() else { continue };
        if !irq.ends_with(':') {
            continue;
        }
        let mut line_sum = 0u64;
        for tok in parts {
            if let Ok(n) = tok.parse::<u64>() {
                line_sum = line_sum.saturating_add(n);
            } else {
                break;
            }
        }
        total = total.saturating_add(line_sum);
        found = true;
    }
    found.then_some(total)
}

#[derive(Debug, Default)]
struct ProcessRadar {
    #[cfg(any(windows, target_os = "linux"))]
    prev_times: HashMap<u32, u64>,
    last_alert: Option<Instant>,
    sustained_high_ticks: u8,
    hysteresis_armed: bool,
}

impl ProcessRadar {
    fn reset(&mut self) {
        #[cfg(any(windows, target_os = "linux"))]
        self.prev_times.clear();
        self.sustained_high_ticks = 0;
        self.hysteresis_armed = true;
    }

    fn sample_top_process(&mut self) -> Option<String> {
        #[cfg(windows)]
        {
            use windows::Win32::Foundation::{CloseHandle, FILETIME};
            use windows::Win32::System::Diagnostics::ToolHelp::{
                CreateToolhelp32Snapshot, PROCESSENTRY32W, Process32FirstW, Process32NextW,
                TH32CS_SNAPPROCESS,
            };
            use windows::Win32::System::Threading::{
                GetProcessTimes, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION,
            };

            let ft_u64 =
                |ft: FILETIME| ((ft.dwHighDateTime as u64) << 32) | (ft.dwLowDateTime as u64);

            let snap = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) }.ok()?;
            let mut entry = PROCESSENTRY32W {
                dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
                ..Default::default()
            };

            let mut next_times = HashMap::with_capacity(self.prev_times.len().max(64));
            let mut deltas: Vec<(String, u64)> = Vec::new();
            let self_pid = std::process::id();

            if unsafe { Process32FirstW(snap, &mut entry) }.is_ok() {
                loop {
                    let pid = entry.th32ProcessID;
                    if pid > 4
                        && pid != self_pid
                        && let Ok(proc_h) =
                            unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) }
                    {
                        let mut c_t = FILETIME::default();
                        let mut e_t = FILETIME::default();
                        let mut k_t = FILETIME::default();
                        let mut u_t = FILETIME::default();
                        if unsafe {
                            GetProcessTimes(proc_h, &mut c_t, &mut e_t, &mut k_t, &mut u_t)
                        }
                        .is_ok()
                        {
                            let total = ft_u64(k_t).saturating_add(ft_u64(u_t));
                            if let Some(&prev) = self.prev_times.get(&pid) {
                                let d = total.saturating_sub(prev);
                                if d > 0 {
                                    let nul = entry
                                        .szExeFile
                                        .iter()
                                        .position(|&c| c == 0)
                                        .unwrap_or(entry.szExeFile.len());
                                    let name = String::from_utf16_lossy(&entry.szExeFile[..nul]);
                                    deltas.push((name, d));
                                }
                            }
                            next_times.insert(pid, total);
                        }
                        let _ = unsafe { CloseHandle(proc_h) };
                    }
                    if unsafe { Process32NextW(snap, &mut entry) }.is_err() {
                        break;
                    }
                }
            }
            let _ = unsafe { CloseHandle(snap) };
            self.prev_times = next_times;
            pick_top_culprit(&deltas)
        }

        #[cfg(target_os = "linux")]
        {
            let mut next_times = HashMap::with_capacity(self.prev_times.len().max(64));
            let mut deltas: Vec<(String, u64)> = Vec::new();
            let self_pid = std::process::id();
            let Ok(entries) = std::fs::read_dir("/proc") else {
                return None;
            };
            for ent in entries.flatten() {
                let Ok(pid) = ent.file_name().to_string_lossy().parse::<u32>() else {
                    continue;
                };
                if pid == self_pid {
                    continue;
                }
                let Ok(stat) = std::fs::read_to_string(ent.path().join("stat")) else {
                    continue;
                };
                let Some(rparen) = stat.rfind(')') else {
                    continue;
                };
                let fields: Vec<&str> = stat[rparen + 1..].split_whitespace().collect();
                if fields.len() < 13 {
                    continue;
                }
                let utime = fields[11].parse::<u64>().unwrap_or(0);
                let stime = fields[12].parse::<u64>().unwrap_or(0);
                let total = utime.saturating_add(stime);
                if let Some(&prev) = self.prev_times.get(&pid) {
                    let d = total.saturating_sub(prev);
                    if d > 0 {
                        let name =
                            std::fs::read_to_string(ent.path().join("comm")).unwrap_or_default();
                        deltas.push((name, d));
                    }
                }
                next_times.insert(pid, total);
            }
            self.prev_times = next_times;
            pick_top_culprit(&deltas)
        }

        #[cfg(target_os = "macos")]
        {
            let out = std::process::Command::new("ps")
                .args(["-Arcwwwxo", "%cpu,comm"])
                .output()
                .ok()?;
            let text = String::from_utf8_lossy(&out.stdout);
            for line in text.lines().skip(1) {
                let mut parts = line.split_whitespace();
                let _pct = parts.next()?;
                let comm = parts.next()?;
                if let Some(clean) = sanitize_process_label(comm)
                    && !clean.eq_ignore_ascii_case("diypets")
                {
                    return Some(clean);
                }
            }
            None
        }

        #[cfg(not(any(windows, target_os = "linux", target_os = "macos")))]
        {
            None
        }
    }

    fn maybe_alert(&mut self, now: Instant, cpu_fraction: f32) -> Option<String> {
        // Schmitt-trigger hysteresis: re-arm only once CPU drops cleanly below SPIKE_RELEASE_THRESHOLD
        if cpu_fraction < SPIKE_RELEASE_THRESHOLD {
            self.hysteresis_armed = true;
            self.sustained_high_ticks = 0;
        }

        let ready = self
            .last_alert
            .is_none_or(|prev| now.saturating_duration_since(prev) >= SPIKE_ALERT_COOLDOWN);

        if cpu_fraction >= SPIKE_WARMUP_THRESHOLD {
            let top = self.sample_top_process();
            if cpu_fraction >= SPIKE_ALERT_THRESHOLD {
                self.sustained_high_ticks = self.sustained_high_ticks.saturating_add(1);
                if self.sustained_high_ticks >= SPIKE_SUSTAIN_TICKS
                    && self.hysteresis_armed
                    && ready
                    && let Some(culprit) = top
                {
                    self.last_alert = Some(now);
                    self.hysteresis_armed = false;
                    self.sustained_high_ticks = 0;
                    let pct = (cpu_fraction * 100.0).round().clamp(0.0, 100.0) as u8;
                    return Some(format!("CPU {pct}%: {culprit}"));
                }
            } else {
                self.sustained_high_ticks = 0;
            }
        } else {
            self.reset();
        }

        // Secondary ambient check: physical RAM exhaustion (>= 92%)
        if ready
            && let Some(ram) = sample_ram_usage_fraction()
            && ram >= RAM_ALERT_THRESHOLD
        {
            self.last_alert = Some(now);
            let pct = (ram * 100.0).round().clamp(0.0, 100.0) as u8;
            return Some(format!("RAM {pct}% full!"));
        }
        None
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

            let ft_u64 =
                |ft: FILETIME| ((ft.dwHighDateTime as u64) << 32) | (ft.dwLowDateTime as u64);
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
    prev_idle_secs: Option<f64>,
}

impl InputCadencePoller {
    #[cfg_attr(not(any(windows, target_os = "linux")), allow(dead_code))]
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

        #[cfg(target_os = "macos")]
        {
            #[link(name = "CoreGraphics", kind = "framework")]
            unsafe extern "C" {
                fn CGEventSourceSecondsSinceLastEventType(state_id: i32, event_type: u32) -> f64;
            }
            // kCGEventSourceStateCombinedSessionState = 0, kCGEventKeyDown = 10
            let idle = unsafe { CGEventSourceSecondsSinceLastEventType(0, 10) };
            self.feed_idle_seconds(idle)
        }

        #[cfg(target_os = "linux")]
        {
            let Ok(txt) = std::fs::read_to_string("/proc/interrupts") else {
                return false;
            };
            let Some(irqs) = parse_linux_kbd_interrupts(&txt) else {
                return false;
            };
            self.feed_sample((irqs & 0xFFFF_FFFF) as u32, None)
        }

        #[cfg(not(any(windows, target_os = "macos", target_os = "linux")))]
        {
            false
        }
    }

    #[cfg_attr(not(target_os = "macos"), allow(dead_code))]
    fn feed_idle_seconds(&mut self, idle_secs: f64) -> bool {
        if !idle_secs.is_finite() || idle_secs < 0.0 {
            return false;
        }
        let Some(prev) = self.prev_idle_secs.replace(idle_secs) else {
            return false;
        };
        idle_secs + 0.005 < prev || (idle_secs < 0.05 && prev >= 0.05)
    }
}

#[derive(Debug)]
pub struct ReactiveCompanion {
    cpu_sampler: CpuSampler,
    process_radar: ProcessRadar,
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
            process_radar: ProcessRadar {
                hysteresis_armed: true,
                ..Default::default()
            },
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

    pub fn update(
        &mut self,
        now: Instant,
        typing_reflex: bool,
        cpu_sprint: bool,
    ) -> ReactiveOutput {
        let mut trigger_hop = false;
        let mut speed_dirty = false;
        let mut spike_alert = None;

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
                    spike_alert = self.process_radar.maybe_alert(now, self.smoothed_cpu);
                    if spike_alert.is_some() {
                        trigger_hop = true;
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
            self.process_radar.reset();
            speed_dirty = true;
        }

        let taps_per_sec = self.tap_times.len() as f32 / TAP_WINDOW.as_secs_f32();
        ReactiveOutput {
            trigger_hop,
            speed_multiplier: self.applied_multiplier,
            cpu_percent: (self.smoothed_cpu * 100.0).round().clamp(0.0, 100.0) as u8,
            taps_per_sec,
            speed_dirty,
            spike_alert,
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
        assert!(!comp.record_tap(t0 + Duration::from_millis(80)));
        assert!(comp.record_tap(t0 + Duration::from_millis(260)));
    }

    #[test]
    fn input_poller_ignores_passive_cursor_movement_without_keystroke() {
        let mut poller = InputCadencePoller::default();
        assert!(!poller.feed_sample(1000, Some((500, 500))));
        assert!(!poller.feed_sample(1050, Some((540, 500))));
        assert!(poller.feed_sample(1120, Some((540, 500))));
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

    #[test]
    fn process_radar_picks_real_culprit_and_filters_system_idle() {
        let deltas = vec![
            ("System Idle Process".to_string(), 99_000),
            ("C:\\Users\\sm\\.cargo\\bin\\cargo.exe".to_string(), 42_500),
            ("node.exe".to_string(), 11_200),
        ];
        assert_eq!(pick_top_culprit(&deltas), Some("cargo".to_string()));
        assert_eq!(sanitize_process_label("System"), None);
    }

    #[test]
    fn process_radar_requires_sustained_ticks_before_alerting() {
        let mut r = ProcessRadar {
            hysteresis_armed: true,
            ..Default::default()
        };
        let t0 = Instant::now();
        assert_eq!(r.sustained_high_ticks, 0);
        // Below warmup resets counter cleanly
        assert_eq!(r.maybe_alert(t0, 0.20), None);
        assert!(r.hysteresis_armed);
    }

    #[test]
    fn parses_macos_vm_stat_ram_fraction() {
        let sample = "Mach Virtual Memory Statistics: (page size of 16384 bytes)\nPages free: 10000.\nPages active: 200000.\nPages wired down: 100000.\nPages occupied by compressor: 50000.\n";
        let total_bytes = 16u64 * 1024 * 1024 * 1024; // 16 GiB
        let frac = parse_macos_vm_stat(sample, total_bytes).expect("valid vm_stat");
        // (350000 * 16384) / 16 GiB ~= 0.3338
        assert!((0.32..=0.35).contains(&frac), "unexpected frac={frac}");
    }

    #[test]
    fn parses_linux_keyboard_interrupts_and_macos_idle_drops() {
        let irqs = "           CPU0       CPU1\n  1:        120        340   IO-APIC   1-edge      i8042\n  8:          0          1   IO-APIC   8-edge      rtc0\n";
        assert_eq!(parse_linux_kbd_interrupts(irqs), Some(460));

        let mut poller = InputCadencePoller::default();
        assert!(!poller.feed_idle_seconds(1.20));
        assert!(!poller.feed_idle_seconds(1.26));
        assert!(poller.feed_idle_seconds(0.02));
        assert!(!poller.feed_idle_seconds(0.08));
    }
}
