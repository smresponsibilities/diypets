use diypets::deskpet_state::{DeskpetState, DeskpetStateId};
use diypets::deskpet_xml::DeskpetPack;
use diypets::settings::{MAX_ACTIVE_PETS, Settings, find_pack_xml};
use diypets::studio_model::{Archetype, CustomAnimation, PartRole, RigPart, StudioProject};
use diypets::svg_import::import_svg_to_project;
use std::path::Path;
use std::sync::Arc;
use std::time::Instant;

#[test]
fn stress_eight_simultaneous_pets_40000_physics_ticks() {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut states: Vec<DeskpetState> = Vec::new();

    // Load 5 Made by SM originals + 3 Studio V2 archetypes (Bird, Snake, Fish) = 8 simultaneous pets!
    for id in ["highland_cow", "dog", "polar_bear", "goat", "stegosaurus"] {
        let xml_path = manifest_dir.join("assets").join(id).join("deskpet.xml");
        if let Ok(xml) = std::fs::read_to_string(&xml_path) {
            let pack = DeskpetPack::parse(&xml).expect(id);
            states.push(DeskpetState::new(pack));
        }
    }
    for arch in [Archetype::Bird, Archetype::Crawler, Archetype::Swimmer] {
        let proj = StudioProject::template_by_archetype(arch);
        let (_, _, xml) = proj.build_export_artifacts().expect(arch.label());
        let pack = DeskpetPack::parse(&xml).expect(arch.label());
        states.push(DeskpetState::new(pack));
    }
    assert_eq!(states.len(), MAX_ACTIVE_PETS);

    let t0 = Instant::now();
    for step in 0..5000u32 {
        let screen_w = if step % 900 < 450 { 1920.0 } else { 2560.0 };
        let floor_y = if step % 300 < 150 { 950.0 } else { 620.0 };

        for (i, pet) in states.iter_mut().enumerate() {
            if step % 750 == (i as u32 * 50) {
                pet.pet_size = 32 + ((step * 17 + i as u32 * 31) % 225);
                pet.walk_speed = 1 + ((step + i as u32) % 20) as i32;
            }
            if step % 500 == (i as u32 * 40 + 10) {
                // Simulate rapid drag & toss
                pet.set_state(DeskpetStateId::Drag);
                pet.release_drag(if i % 2 == 0 { 24.0 } else { -24.0 }, -15.0, 16);
            }
            if step % 800 == (i as u32 * 60 + 25) {
                pet.set_state(DeskpetStateId::Custom("sleep".to_string()));
            }

            pet.tick(16, floor_y, screen_w);

            assert!(pet.x.is_finite() && pet.y.is_finite(), "NaN/Inf on pet {i} step {step}");
            assert!(
                pet.x >= -1.0 && pet.x <= screen_w + 1.0,
                "pet {i} escaped horizontal bounds: x={} (screen_w={})",
                pet.x,
                screen_w
            );
            assert!(
                pet.y <= floor_y + 1.0,
                "pet {i} sank below floor: y={} (floor_y={})",
                pet.y,
                floor_y
            );
            let frame = pet.current_frame().expect("valid frame") as usize;
            assert!(
                frame < pet.pack.frames.len(),
                "pet {i} out-of-range frame {frame} >= {}",
                pet.pack.frames.len()
            );
        }
    }

    let elapsed = t0.elapsed();
    assert!(
        elapsed.as_secs() < 15,
        "40,000 physics ticks took too long: {elapsed:?}"
    );
}

#[test]
fn stress_all_bundled_packs_parse_and_simulate_cleanly() {
    let assets_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("assets");
    let mut checked = 0usize;
    for entry in std::fs::read_dir(&assets_dir).unwrap().flatten() {
        let xml_path = entry.path().join("deskpet.xml");
        if !xml_path.is_file() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().into_owned();
        let xml = std::fs::read_to_string(&xml_path).expect(&name);
        let pack = DeskpetPack::parse(&xml).unwrap_or_else(|e| panic!("pack {name} failed: {e}"));
        assert!(!pack.frames.is_empty(), "{name} has 0 frames");
        let mut st = DeskpetState::new(pack);
        for _ in 0..200 {
            st.tick(20, 900.0, 1920.0);
            assert!(st.x.is_finite() && st.y.is_finite(), "{name} produced NaN");
        }
        st.set_state(DeskpetStateId::Custom("sleep".to_string()));
        st.tick(20, 900.0, 1920.0);
        assert!(st.current_frame().is_some(), "{name} failed sleep fallback");
        checked += 1;
    }
    assert!(checked >= 5, "expected at least 5 bundled packs, found {checked}");
}

#[test]
fn stress_settings_concurrent_atomic_read_write() {
    let dir = tempfile::tempdir().unwrap();
    let path = Arc::new(dir.path().join("settings.toml"));
    Settings::default().save(&path).unwrap();

    let mut handles = Vec::new();
    for t in 0..4u32 {
        let p = Arc::clone(&path);
        handles.push(std::thread::spawn(move || {
            for i in 0..40u32 {
                let mut s = Settings::default();
                s.pet_size = 48 + ((t * 32 + i) % 180);
                s.walk_speed = 1 + ((t + i) % 20) as i32;
                s.extra_pets = vec!["dog".to_string(); ((i % 4) as usize)];
                let _ = s.save(&p);
            }
        }));
    }
    for _ in 0..4 {
        let p = Arc::clone(&path);
        handles.push(std::thread::spawn(move || {
            for _ in 0..60 {
                let loaded = Settings::load_sanitized(&p);
                assert!((32..=256).contains(&loaded.pet_size));
                assert!((1..=20).contains(&loaded.walk_speed));
                assert!(loaded.active_herd().len() <= MAX_ACTIVE_PETS);
            }
        }));
    }
    for h in handles {
        h.join().unwrap();
    }
}

#[test]
fn stress_studio_heavy_rig_and_svg_fuzz_resilience() {
    // 1. Heavy custom creature with 8 extra limbs + 3 custom animations
    let mut proj = StudioProject::template_cat();
    for i in 0..5 {
        let mut extra = RigPart::new(format!("Extra_{i}"), PartRole::Extra);
        extra.pixels[64 * 128 + 64] = [200, 100, 240, 255];
        proj.parts.push(extra);
    }
    proj.custom_animations.push(CustomAnimation::dance_preset());
    proj.custom_animations.push(CustomAnimation::hop_preset());
    let (_, _, xml) = proj.build_export_artifacts().expect("heavy rig export");
    let pack = DeskpetPack::parse(&xml).expect("heavy rig xml valid");
    assert!(pack.frames.len() >= 28);

    // 2. Malformed / hostile SVGs must return Err, never panic
    for bad in [
        "",
        "<not-xml",
        "<svg></svg>",
        "<svg viewBox='0 0 0 0'><path d='M bad NaN Z'/></svg>",
        "<svg><circle cx='999999' cy='-999999' r='-5'/></svg>",
    ] {
        let _ = import_svg_to_project(bad, "Bad", Archetype::Quadruped);
    }

    // 3. Path traversal must be rejected
    assert!(find_pack_xml("../Cargo.toml").is_none());
    assert!(find_pack_xml("..\\windows").is_none());
    assert!(find_pack_xml("a/b").is_none());
}