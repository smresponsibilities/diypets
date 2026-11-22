#[cfg(test)]
use image::{Rgba, RgbaImage};
use std::fs;
use std::path::{Path, PathBuf};
use toml::Value;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PackError {
    IoError(String),
    ParseError(String),
    MissingFrames { expected: usize, found: usize },
    InvalidRange { state: String, range: String },
    UnsafePath(PathBuf),
}

impl std::fmt::Display for PackError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PackError::IoError(msg) => write!(f, "IO error: {msg}"),
            PackError::ParseError(msg) => write!(f, "parse error: {msg}"),
            PackError::MissingFrames { expected, found } => {
                write!(f, "missing frames: expected {expected}, found {found}")
            }
            PackError::InvalidRange { state, range } => {
                write!(f, "invalid range for state '{state}': {range}")
            }
            PackError::UnsafePath(path) => {
                write!(f, "unsafe path: {path:?}")
            }
        }
    }
}

impl std::error::Error for PackError {}

#[derive(Debug, Clone)]
pub struct FrameRange {
    pub start: usize,
    pub end: usize,
}

#[derive(Debug, Clone)]
pub struct Pack {
    pub name: String,
    pub behavior: String,
    pub frame_width: u32,
    pub frame_height: u32,
    pub fps: u32,
    pub frame_count: usize,
    pub state_ranges: std::collections::HashMap<String, FrameRange>,
    pub strip_path: PathBuf,
}

pub trait PackLoader {
    fn load(&self, path: &Path) -> Result<Pack, PackError>;
}

pub struct FormatV0Loader;
impl PackLoader for FormatV0Loader {
    fn load(&self, path: &Path) -> Result<Pack, PackError> {
        if path
            .components()
            .any(|c| matches!(c, std::path::Component::ParentDir))
        {
            return Err(PackError::UnsafePath(path.to_path_buf()));
        }

        let meta_path = path.join("meta.toml");
        let meta_content = fs::read_to_string(&meta_path)
            .map_err(|e| PackError::IoError(format!("cannot read meta.toml: {e}")))?;

        let meta: Value = meta_content
            .parse::<Value>()
            .map_err(|e| PackError::ParseError(format!("invalid meta.toml: {e}")))?;

        let frame_width = meta
            .get("frame_width")
            .and_then(|v| v.as_integer())
            .ok_or_else(|| PackError::ParseError("missing frame_width".into()))?
            as u32;

        let frame_height = meta
            .get("frame_height")
            .and_then(|v| v.as_integer())
            .ok_or_else(|| PackError::ParseError("missing frame_height".into()))?
            as u32;

        let fps = meta
            .get("fps")
            .and_then(|v| v.as_integer())
            .ok_or_else(|| PackError::ParseError("missing fps".into()))? as u32;

        let frame_count = meta
            .get("frame_count")
            .and_then(|v| v.as_integer())
            .ok_or_else(|| PackError::ParseError("missing frame_count".into()))?
            as usize;

        let mut state_ranges = std::collections::HashMap::new();
        if let Some(states) = meta.get("states").and_then(|v| v.as_table()) {
            for (name, range_val) in states {
                let range_str = range_val
                    .as_str()
                    .ok_or_else(|| PackError::ParseError(format!("invalid range for {name}")))?;
                let parts: Vec<&str> = range_str.split('-').collect();
                if parts.len() != 2 {
                    return Err(PackError::InvalidRange {
                        state: name.clone(),
                        range: range_str.into(),
                    });
                }
                let start =
                    parts[0]
                        .trim()
                        .parse::<usize>()
                        .map_err(|_| PackError::InvalidRange {
                            state: name.clone(),
                            range: range_str.into(),
                        })?;
                let end =
                    parts[1]
                        .trim()
                        .parse::<usize>()
                        .map_err(|_| PackError::InvalidRange {
                            state: name.clone(),
                            range: range_str.into(),
                        })?;
                state_ranges.insert(name.clone(), FrameRange { start, end });
            }
        }

        let strip_path = path.join("strip.png");
        if !strip_path.exists() {
            return Err(PackError::IoError("missing strip.png".into()));
        }

        let img = image::open(&strip_path)
            .map_err(|e| PackError::IoError(format!("cannot open strip.png: {e}")))?;
        let rgba = img.to_rgba8();
        let actual_frame_count = if frame_width > 0 {
            rgba.width() as usize / frame_width as usize
        } else {
            0
        };

        if actual_frame_count != frame_count {
            return Err(PackError::MissingFrames {
                expected: frame_count,
                found: actual_frame_count,
            });
        }

        let behavior = meta
            .get("behavior")
            .and_then(|v| v.as_str())
            .unwrap_or("deskpet")
            .to_string();

        Ok(Pack {
            name: path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default(),
            behavior,
            frame_width,
            frame_height,
            fps,
            frame_count,
            state_ranges,
            strip_path,
        })
    }
}

pub fn load_pack(path: &Path) -> Result<Pack, PackError> {
    let loader = FormatV0Loader;
    loader.load(path)
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn create_test_pack(dir: &Path, frame_count: usize, width: u32, height: u32) {
        let mut img = RgbaImage::new(width * frame_count as u32, height);
        for i in 0..frame_count {
            for x in 0..width {
                for y in 0..height {
                    img.put_pixel(i as u32 * width + x, y, Rgba([255, 0, 0, 255]));
                }
            }
        }
        img.save(dir.join("strip.png")).unwrap();

        let meta = format!(
            r#"frame_width = {width}
frame_height = {height}
fps = 12
frame_count = {frame_count}

[states]
idle = "0-{start}"
walk = "{start}-{end}"
"#,
            start = frame_count / 2,
            end = frame_count - 1
        );
        fs::write(dir.join("meta.toml"), meta).unwrap();
    }

    #[test]
    fn load_valid_pack() {
        let dir = TempDir::new().unwrap();
        create_test_pack(dir.path(), 4, 32, 32);
        let pack = load_pack(dir.path()).unwrap();
        assert_eq!(pack.frame_count, 4);
        assert_eq!(pack.frame_width, 32);
        assert_eq!(pack.fps, 12);
        assert!(pack.state_ranges.contains_key("idle"));
        assert!(pack.state_ranges.contains_key("walk"));
    }

    #[test]
    fn missing_strip_returns_error() {
        let dir = TempDir::new().unwrap();
        let meta = r#"frame_width = 32
frame_height = 32
fps = 12
frame_count = 4

[states]
idle = "0-1"
walk = "2-3"
"#;
        fs::write(dir.path().join("meta.toml"), meta).unwrap();
        let result = load_pack(dir.path());
        assert!(result.is_err());
    }

    #[test]
    fn unsafe_path_returns_error() {
        let dir = TempDir::new().unwrap();
        create_test_pack(dir.path(), 4, 32, 32);
        let unsafe_path = dir.path().join("..").join("strip.png");
        let result = FormatV0Loader.load(&unsafe_path);
        assert!(result.is_err());
    }

    #[test]
    fn every_shipped_pack_loads() {
        let assets = Path::new(env!("CARGO_MANIFEST_DIR")).join("assets");
        let mut checked = 0usize;

        let entries = fs::read_dir(&assets).expect("the assets folder ships with the crate");
        for entry in entries {
            let path = entry.expect("readable directory entry").path();
            if !path.is_dir() {
                continue;
            }
            if path.join("deskpet.xml").is_file() {
                let xml = fs::read_to_string(path.join("deskpet.xml")).unwrap();
                let pack = crate::deskpet_xml::DeskpetPack::parse(&xml)
                    .unwrap_or_else(|error| panic!("{} does not load: {error}", path.display()));
                assert!(!pack.frames.is_empty(), "{} has no frames", path.display());
                checked += 1;
            } else if path.join("meta.toml").is_file() {
                let pack = load_pack(&path)
                    .unwrap_or_else(|error| panic!("{} does not load: {error}", path.display()));
                assert!(
                    pack.frame_count > 0,
                    "{} declares no frames",
                    path.display()
                );
                assert!(
                    pack.state_ranges.contains_key("idle"),
                    "{} has no idle range",
                    path.display()
                );
                assert!(
                    pack.state_ranges.contains_key("walk"),
                    "{} has no walk range",
                    path.display()
                );
                checked += 1;
            }
        }

        assert!(
            checked >= 10,
            "expected the shipped packs to be checked, found {checked}"
        );
    }
}
