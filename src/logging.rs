//! Lightweight zero-dependency rotating file logger with automatic retention cleanup.
//! Logs to `<config_dir>/diypets/logs/<bin>-<session>.log` and prunes stale/excess files on startup.

use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const MAX_LOG_AGE_SECS: u64 = 7 * 24 * 60 * 60; // Auto-delete logs older than 7 days
const MAX_LOG_FILES: usize = 5;                 // Keep at most 5 recent log files
const MAX_LOG_BYTES: u64 = 2 * 1024 * 1024;     // Cap single log file at 2 MiB

struct LogState {
    path: PathBuf,
    file: File,
    bytes_written: u64,
}

static LOGGER: OnceLock<Mutex<LogState>> = OnceLock::new();

/// Directory where DIYPets stores rotating diagnostic logs.
pub fn log_dir() -> PathBuf {
    #[cfg(windows)]
    if let Ok(appdata) = std::env::var("APPDATA") {
        return PathBuf::from(appdata).join("diypets").join("logs");
    }
    #[cfg(target_os = "macos")]
    if let Ok(home) = std::env::var("HOME") {
        return PathBuf::from(home)
            .join("Library")
            .join("Application Support")
            .join("diypets")
            .join("logs");
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    if let Ok(home) = std::env::var("HOME") {
        return PathBuf::from(home)
            .join(".local")
            .join("share")
            .join("diypets")
            .join("logs");
    }
    std::env::temp_dir().join("diypets-logs")
}

/// Remove log files older than `max_age` and enforce `max_files` retention cap.
pub fn cleanup_logs_in(dir: &Path, max_age: Duration, max_files: usize) -> usize {
    let Ok(entries) = fs::read_dir(dir) else {
        return 0;
    };
    let now = SystemTime::now();
    let mut kept: Vec<(SystemTime, PathBuf)> = Vec::new();
    let mut removed = 0usize;

    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("log") {
            continue;
        }
        let modified = entry
            .metadata()
            .and_then(|m| m.modified())
            .unwrap_or(UNIX_EPOCH);
        let age = now.duration_since(modified).unwrap_or(Duration::ZERO);
        if age > max_age {
            if fs::remove_file(&path).is_ok() {
                removed += 1;
            }
        } else {
            kept.push((modified, path));
        }
    }

    // Sort newest-first and prune anything beyond `max_files`
    kept.sort_by_key(|a| std::cmp::Reverse(a.0));
    if kept.len() > max_files {
        for (_, old_path) in kept.into_iter().skip(max_files) {
            if fs::remove_file(&old_path).is_ok() {
                removed += 1;
            }
        }
    }
    removed
}

fn unix_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::ZERO)
        .as_secs()
}

/// Initialize the logger for `bin_name`, run automatic cleanup, and install a panic backtrace hook.
pub fn init(bin_name: &str) -> Option<PathBuf> {
    let dir = log_dir();
    let _ = fs::create_dir_all(&dir);
    let cleaned = cleanup_logs_in(&dir, Duration::from_secs(MAX_LOG_AGE_SECS), MAX_LOG_FILES.saturating_sub(1));

    let ts = unix_secs();
    let pid = std::process::id();
    let path = dir.join(format!("{bin_name}-{ts}-{pid}.log"));
    let file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .ok()?;

    let _ = LOGGER.set(Mutex::new(LogState {
        path: path.clone(),
        file,
        bytes_written: 0,
    }));

    std::panic::set_hook(Box::new(|panic_info| {
        let bt = std::backtrace::Backtrace::force_capture();
        write_line("PANIC", &format!("{panic_info}\n{bt}"));
    }));

    info(&format!(
        "=== {bin_name} v{} started (pid={pid}, cleaned_old_logs={cleaned}, log={}) ===",
        env!("CARGO_PKG_VERSION"),
        path.display()
    ));
    Some(path)
}

pub fn active_log_path() -> Option<PathBuf> {
    LOGGER.get().and_then(|m| m.lock().ok().map(|s| s.path.clone()))
}

fn write_line(level: &str, msg: &str) {
    let line = format!("[{}][{level}] {msg}\n", unix_secs());
    #[cfg(debug_assertions)]
    eprint!("{line}");

    if let Some(lock) = LOGGER.get()
        && let Ok(mut state) = lock.lock()
    {
            if state.bytes_written > MAX_LOG_BYTES {
                // Rotate in place by truncating once size cap is hit
                if let Ok(f) = OpenOptions::new()
                    .create(true)
                    .write(true)
                    .truncate(true)
                    .open(&state.path)
                {
                    state.file = f;
                    state.bytes_written = 0;
                    let note = "[ROTATED] Log exceeded 2 MiB cap and was truncated.\n";
                    let _ = state.file.write_all(note.as_bytes());
                    state.bytes_written += note.len() as u64;
                }
            }
            if state.file.write_all(line.as_bytes()).is_ok() {
                let _ = state.file.flush();
                state.bytes_written += line.len() as u64;
            }
    }
}

pub fn info(msg: &str) {
    write_line("INFO", msg);
}

pub fn warn(msg: &str) {
    write_line("WARN", msg);
}

pub fn error(msg: &str) {
    write_line("ERROR", msg);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cleanup_prunes_excess_log_files_and_keeps_newest() {
        let tmp = std::env::temp_dir().join(format!("diypets_log_test_{}", std::process::id()));
        let _ = fs::remove_dir_all(&tmp);
        fs::create_dir_all(&tmp).unwrap();

        for i in 0..7 {
            let p = tmp.join(format!("diypets-{i}.log"));
            fs::write(&p, format!("entry {i}")).unwrap();
            std::thread::sleep(Duration::from_millis(12));
        }

        let removed = cleanup_logs_in(&tmp, Duration::from_secs(3600), 3);
        assert_eq!(removed, 4);
        let remaining = fs::read_dir(&tmp).unwrap().count();
        assert_eq!(remaining, 3);
        let _ = fs::remove_dir_all(&tmp);
    }
}