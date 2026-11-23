//! User settings & canonical directory resolution (dev/portable + global `cargo install`).

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

pub const PET_SIZE_MIN: u32 = 32;
pub const PET_SIZE_MAX: u32 = 256;
pub const WALK_SPEED_MIN: i32 = 1;
pub const WALK_SPEED_MAX: i32 = 30;
pub const MAX_ACTIVE_PETS: usize = 8;

fn is_safe_pack_id(id: &str) -> bool {
    !id.is_empty() && !id.contains("..") && !id.contains('/') && !id.contains('\\')
}

/// Optional per-slot size and speed overrides in Multi-Pet Herd Mode.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PetOverride {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub walk_speed: Option<i32>,
}

/// Tunables the tray menu and settings UI write. Field names are the `settings.toml` keys.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// Default pet window edge length in logical pixels.
    pub pet_size: u32,
    /// Default pixels per animation step while walking.
    pub walk_speed: i32,
    /// Keep pet windows above other windows.
    pub always_on_top: bool,
    /// Treat other desktop windows as ledges/walls to stand on and climb.
    pub climb_windows: bool,
    /// Let clicks over transparent sprite corners reach the desktop.
    pub click_through: bool,
    /// Launch DIYPets automatically when you log in.
    pub start_on_login: bool,
    /// Play Mode: steer roaming pets toward the mouse cursor.
    pub follow_cursor: bool,
    /// Auto-hide pets and pause physics while a fullscreen game or video is active.
    pub auto_hide_fullscreen: bool,
    /// Primary active pet pack ID.
    pub pet_pack: String,
    /// Additional simultaneous desktop pets (Herd Mode, up to `MAX_ACTIVE_PETS - 1`).
    pub extra_pets: Vec<String>,
    /// Per-slot size & speed overrides (slot 0 = primary pet, slots 1.. = extra pets).
    pub herd_overrides: Vec<PetOverride>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            pet_pack: "highland_cow".to_string(),
            extra_pets: Vec::new(),
            herd_overrides: Vec::new(),
            pet_size: 96,
            walk_speed: 14,
            always_on_top: true,
            climb_windows: true,
            click_through: true,
            start_on_login: false,
            follow_cursor: false,
            auto_hide_fullscreen: true,
        }
    }
}

pub fn user_data_dir() -> PathBuf {
    #[cfg(windows)]
    {
        if let Some(appdata) = std::env::var_os("APPDATA") {
            return PathBuf::from(appdata).join("diypets");
        }
        if let Some(profile) = std::env::var_os("USERPROFILE") {
            return PathBuf::from(profile)
                .join("AppData")
                .join("Roaming")
                .join("diypets");
        }
    }
    #[cfg(target_os = "macos")]
    {
        if let Some(home) = std::env::var_os("HOME") {
            return PathBuf::from(home)
                .join("Library")
                .join("Application Support")
                .join("diypets");
        }
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        if let Some(xdg) = std::env::var_os("XDG_DATA_HOME") {
            return PathBuf::from(xdg).join("diypets");
        }
        if let Some(home) = std::env::var_os("HOME") {
            return PathBuf::from(home)
                .join(".local")
                .join("share")
                .join("diypets");
        }
    }
    std::env::current_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join(".diypets")
}

pub fn user_packs_dir() -> PathBuf {
    user_data_dir().join("packs")
}

pub fn local_project_root() -> Option<PathBuf> {
    if let Ok(cwd) = std::env::current_dir()
        && cwd.join("assets").is_dir()
    {
        return Some(cwd);
    }
    if let Ok(exe) = std::env::current_exe() {
        let mut dir = exe.parent();
        for _ in 0..5 {
            let Some(candidate) = dir else { break };
            if candidate.join("assets").is_dir() {
                return Some(candidate.to_path_buf());
            }
            dir = candidate.parent();
        }
    }
    None
}

pub fn resolve_project_root() -> PathBuf {
    local_project_root().unwrap_or_else(user_data_dir)
}

pub fn assets_dir() -> PathBuf {
    local_project_root()
        .map(|root| root.join("assets"))
        .unwrap_or_else(user_packs_dir)
}

pub fn catalog_icons_dir() -> Option<PathBuf> {
    let root = local_project_root()?;
    let dir = root.join("catalog_icons");
    if dir.is_dir() { Some(dir) } else { None }
}

pub fn thumb_cache_dir() -> PathBuf {
    if let Some(root) = local_project_root() {
        root.join(".thumb_cache")
    } else {
        user_data_dir().join("thumb_cache")
    }
}

pub fn pack_search_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    let user_dir = user_packs_dir();
    dirs.push(user_dir.clone());
    if let Some(root) = local_project_root() {
        let local_assets = root.join("assets");
        if local_assets != user_dir {
            dirs.push(local_assets);
        }
    }
    dirs
}

pub fn find_pack_xml(pack_id: &str) -> Option<PathBuf> {
    if !is_safe_pack_id(pack_id) {
        return None;
    }
    for dir in pack_search_dirs() {
        let candidate = dir.join(pack_id).join("deskpet.xml");
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}

pub fn find_pack_icon(pack_id: &str) -> Option<PathBuf> {
    if !is_safe_pack_id(pack_id) {
        return None;
    }
    for dir in pack_search_dirs() {
        let candidate = dir.join(pack_id).join("icon.png");
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    if let Some(icons) = catalog_icons_dir() {
        let candidate = icons.join(format!("{pack_id}.png"));
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    let cached = thumb_cache_dir().join(format!("{pack_id}_v2.png"));
    if cached.is_file() {
        return Some(cached);
    }
    None
}

pub fn sibling_binary(stem: &str) -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let file_name = format!("{stem}{}", std::env::consts::EXE_SUFFIX);
    Some(exe.with_file_name(file_name))
}

impl Settings {
    pub fn default_path() -> PathBuf {
        resolve_project_root().join("settings.toml")
    }

    pub fn active_herd(&self) -> Vec<String> {
        let mut herd = Vec::with_capacity(1 + self.extra_pets.len());
        let primary = if is_safe_pack_id(&self.pet_pack) {
            self.pet_pack.clone()
        } else {
            "highland_cow".to_string()
        };
        herd.push(primary);
        for id in &self.extra_pets {
            if herd.len() >= MAX_ACTIVE_PETS {
                break;
            }
            if is_safe_pack_id(id) {
                herd.push(id.clone());
            }
        }
        herd
    }

    pub fn slot_pet_size(&self, slot: usize) -> u32 {
        self.herd_overrides
            .get(slot)
            .and_then(|o| o.size)
            .unwrap_or(self.pet_size)
            .clamp(PET_SIZE_MIN, PET_SIZE_MAX)
    }

    pub fn slot_walk_speed(&self, slot: usize) -> i32 {
        self.herd_overrides
            .get(slot)
            .and_then(|o| o.walk_speed)
            .unwrap_or(self.walk_speed)
            .clamp(WALK_SPEED_MIN, WALK_SPEED_MAX)
    }

    pub fn set_slot_size(&mut self, slot: usize, size: Option<u32>) {
        if slot >= MAX_ACTIVE_PETS {
            return;
        }
        if self.herd_overrides.len() <= slot {
            self.herd_overrides.resize(slot + 1, PetOverride::default());
        }
        self.herd_overrides[slot].size = size.map(|v| v.clamp(PET_SIZE_MIN, PET_SIZE_MAX));
    }

    pub fn set_slot_walk_speed(&mut self, slot: usize, speed: Option<i32>) {
        if slot >= MAX_ACTIVE_PETS {
            return;
        }
        if self.herd_overrides.len() <= slot {
            self.herd_overrides.resize(slot + 1, PetOverride::default());
        }
        self.herd_overrides[slot].walk_speed = speed.map(|v| v.clamp(WALK_SPEED_MIN, WALK_SPEED_MAX));
    }

    pub fn reset_slot_override(&mut self, slot: usize) {
        if let Some(o) = self.herd_overrides.get_mut(slot) {
            *o = PetOverride::default();
        }
    }

    pub fn add_extra_pet(&mut self, pack_id: &str) -> bool {
        if !is_safe_pack_id(pack_id) || 1 + self.extra_pets.len() >= MAX_ACTIVE_PETS {
            return false;
        }
        self.extra_pets.push(pack_id.to_string());
        true
    }

    pub fn remove_extra_pet(&mut self, extra_index: usize) -> bool {
        if extra_index < self.extra_pets.len() {
            self.extra_pets.remove(extra_index);
            let slot = extra_index + 1;
            if slot < self.herd_overrides.len() {
                self.herd_overrides.remove(slot);
            }
            true
        } else {
            false
        }
    }

    pub fn load(path: &Path) -> Self {
        fs::read_to_string(path)
            .ok()
            .and_then(|text| toml::from_str::<Settings>(&text).ok())
            .unwrap_or_default()
    }

    pub fn save(&self, path: &Path) -> Result<(), String> {
        if let Some(parent) = path.parent()
            && !parent.as_os_str().is_empty()
        {
            fs::create_dir_all(parent)
                .map_err(|e| format!("cannot create {}: {e}", parent.display()))?;
        }
        let body =
            toml::to_string_pretty(self).map_err(|e| format!("cannot encode settings: {e}"))?;
        let text = format!(
            "# diypets settings. The tray menu and settings UI write this file; editing by hand is fine.\n{body}"
        );

        let tmp_path = path.with_extension(format!("toml.{}.tmp", std::process::id()));
        fs::write(&tmp_path, &text)
            .map_err(|e| format!("cannot write {}: {e}", tmp_path.display()))?;

        if fs::rename(&tmp_path, path).is_err() {
            let _ = fs::remove_file(&tmp_path);
            fs::write(path, &text).map_err(|e| format!("cannot write {}: {e}", path.display()))?;
        }
        Ok(())
    }

    pub fn sanitized(&self) -> Self {
        let mut s = self.clone();
        s.pet_size = s.pet_size.clamp(PET_SIZE_MIN, PET_SIZE_MAX);
        s.walk_speed = s.walk_speed.clamp(WALK_SPEED_MIN, WALK_SPEED_MAX);
        if !is_safe_pack_id(&s.pet_pack) {
            s.pet_pack = "highland_cow".to_string();
        }
        s.extra_pets.retain(|id| is_safe_pack_id(id));
        s.extra_pets.truncate(MAX_ACTIVE_PETS - 1);
        s.herd_overrides.truncate(1 + s.extra_pets.len());
        for o in &mut s.herd_overrides {
            o.size = o.size.map(|v| v.clamp(PET_SIZE_MIN, PET_SIZE_MAX));
            o.walk_speed = o.walk_speed.map(|v| v.clamp(WALK_SPEED_MIN, WALK_SPEED_MAX));
        }
        s
    }

    pub fn load_sanitized(path: &Path) -> Self {
        Self::load(path).sanitized()
    }

    pub fn with_pet_size(&self, pet_size: u32) -> Self {
        let mut s = self.clone();
        s.pet_size = pet_size;
        s.sanitized()
    }

    pub fn with_walk_speed(&self, walk_speed: i32) -> Self {
        let mut s = self.clone();
        s.walk_speed = walk_speed;
        s.sanitized()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn temp_path(dir: &TempDir) -> PathBuf {
        dir.path().join("settings.toml")
    }

    #[test]
    fn defaults_are_inside_supported_ranges() {
        let s = Settings::default();
        assert_eq!(Settings::default(), s.sanitized());
        assert!(s.pet_size >= PET_SIZE_MIN && s.pet_size <= PET_SIZE_MAX);
        assert!(s.walk_speed >= WALK_SPEED_MIN && s.walk_speed <= WALK_SPEED_MAX);
        assert_eq!(s.active_herd(), vec!["highland_cow".to_string()]);
    }

    #[test]
    fn load_missing_file_returns_defaults() {
        let dir = TempDir::new().unwrap();
        assert_eq!(Settings::load(&temp_path(&dir)), Settings::default());
    }

    #[test]
    fn save_then_load_roundtrip_with_per_pet_overrides() {
        let dir = TempDir::new().unwrap();
        let path = temp_path(&dir);
        let mut wanted = Settings {
            pet_pack: "highland_cow".to_string(),
            extra_pets: vec!["dog".to_string(), "polar_bear".to_string()],
            herd_overrides: Vec::new(),
            pet_size: 128,
            walk_speed: 5,
            always_on_top: false,
            climb_windows: false,
            click_through: false,
            start_on_login: true,
            follow_cursor: true,
            auto_hide_fullscreen: false,
        };
        wanted.set_slot_size(0, Some(168));
        wanted.set_slot_size(1, Some(72));
        wanted.set_slot_walk_speed(1, Some(11));
        wanted.save(&path).expect("save settings");
        let loaded = Settings::load_sanitized(&path);
        assert_eq!(loaded, wanted);
        assert_eq!(loaded.slot_pet_size(0), 168);
        assert_eq!(loaded.slot_pet_size(1), 72);
        assert_eq!(loaded.slot_walk_speed(1), 11);
        assert_eq!(loaded.slot_pet_size(2), 128);
    }

    #[test]
    fn herd_clamps_to_max_and_strips_unsafe_paths() {
        let mut s = Settings::default();
        s.extra_pets = vec![
            "../bad".to_string(),
            "dog".to_string(),
            "goat".to_string(),
            "polar_bear".to_string(),
            "stegosaurus".to_string(),
            "esheep64".to_string(),
            "fox".to_string(),
            "neko".to_string(),
            "extra_overflow".to_string(),
            "extra_overflow_2".to_string(),
        ];
        let clean = s.sanitized();
        assert_eq!(clean.active_herd().len(), MAX_ACTIVE_PETS);
        assert!(!clean.extra_pets.iter().any(|p| p.contains("..")));
    }

    #[test]
    fn a_settings_file_with_removed_legacy_keys_still_loads() {
        let dir = TempDir::new().unwrap();
        let path = temp_path(&dir);
        fs::write(
            &path,
            "pet_size = 64\nwalk_speed = 2\ndiagonal_walk = true\nfootprints = true\n",
        )
        .expect("write old settings");
        let loaded = Settings::load_sanitized(&path);
        assert_eq!(loaded.pet_size, 64);
        assert_eq!(loaded.walk_speed, 2);
        assert!(loaded.click_through);
    }

    #[test]
    fn saved_file_is_plain_toml_with_comment() {
        let dir = TempDir::new().unwrap();
        let path = temp_path(&dir);
        Settings::default().save(&path).expect("save settings");
        let text = fs::read_to_string(&path).unwrap();
        assert!(text.starts_with('#'));
        assert!(text.contains(&format!("pet_size = {}", Settings::default().pet_size)));
    }

    #[test]
    fn broken_file_returns_defaults_instead_of_panicking() {
        let dir = TempDir::new().unwrap();
        let path = temp_path(&dir);
        fs::write(&path, "this is not = = toml [[[").unwrap();
        assert_eq!(Settings::load(&path), Settings::default());
    }

    #[test]
    fn partial_file_keeps_defaults_for_missing_keys() {
        let dir = TempDir::new().unwrap();
        let path = temp_path(&dir);
        fs::write(&path, "walk_speed = 7\n").unwrap();
        let loaded = Settings::load(&path);
        assert_eq!(loaded.walk_speed, 7);
        assert_eq!(loaded.pet_size, Settings::default().pet_size);
    }

    #[test]
    fn sanitize_clamps_out_of_range_values() {
        let wild = Settings {
            pet_size: 0,
            walk_speed: -40,
            ..Settings::default()
        };
        let fixed = wild.sanitized();
        assert_eq!(fixed.pet_size, PET_SIZE_MIN);
        assert_eq!(fixed.walk_speed, WALK_SPEED_MIN);

        let huge = Settings {
            pet_size: 100_000,
            walk_speed: 900,
            ..Settings::default()
        };
        let fixed = huge.sanitized();
        assert_eq!(fixed.pet_size, PET_SIZE_MAX);
        assert_eq!(fixed.walk_speed, WALK_SPEED_MAX);
    }

    #[test]
    fn load_sanitized_repairs_a_hand_edited_file() {
        let dir = TempDir::new().unwrap();
        let path = temp_path(&dir);
        fs::write(&path, "pet_size = 9999\nwalk_speed = 0\n").unwrap();
        let loaded = Settings::load_sanitized(&path);
        assert_eq!(loaded.pet_size, PET_SIZE_MAX);
        assert_eq!(loaded.walk_speed, WALK_SPEED_MIN);
    }

    #[test]
    fn with_helpers_clamp_and_keep_other_fields() {
        let s = Settings::default().with_pet_size(4096).with_walk_speed(0);
        assert_eq!(s.pet_size, PET_SIZE_MAX);
        assert_eq!(s.walk_speed, WALK_SPEED_MIN);
        assert_eq!(s.always_on_top, Settings::default().always_on_top);
    }
}