use diypets::deskpet_state::{DeskpetState, DeskpetStateId};
use diypets::deskpet_xml::DeskpetPack;
use std::path::Path;

#[test]
fn approved_roster_plays_without_rejected_or_missing_states() {
    let assets = Path::new(env!("CARGO_MANIFEST_DIR")).join("assets");
    assert!(
        !assets.join("triceratops").exists(),
        "triceratops is rejected"
    );
    for (name, count) in [
        ("dog", 64),
        ("polar_bear", 60),
        ("goat", 56),
        ("stegosaurus", 36),
    ] {
        let xml = std::fs::read_to_string(assets.join(name).join("deskpet.xml")).unwrap();
        let pack = DeskpetPack::parse(&xml).unwrap();
        assert_eq!((pack.tile_w, pack.tile_h), (128, 128), "{name}");
        assert_eq!(pack.frames.len(), count, "{name}: approved frames only");
        for animation in pack.animations.values() {
            assert!(!animation.sequence.is_empty(), "{name}: empty animation");
            for &frame in &animation.sequence {
                assert!(
                    pack.frames[frame as usize].iter().any(|p| p >> 24 > 0),
                    "{name}: blank frame"
                );
            }
            for next in animation
                .next
                .iter()
                .chain(&animation.next_border)
                .chain(&animation.next_gravity)
            {
                assert!(
                    pack.animations.contains_key(&next.animation_id),
                    "{name}: missing transition"
                );
            }
        }
        assert!(
            pack.animations.contains_key("19"),
            "{name}: missing climb (id=19)"
        );
        assert!(
            pack.animations["19"].next_gravity.is_empty(),
            "{name}: climb (id=19) must not have Gravity"
        );
        assert!(
            pack.animations.contains_key("13"),
            "{name}: missing sleep (id=13)"
        );
        if name == "stegosaurus" {
            assert!(!pack.animations.contains_key("12"), "rejected sit");
            assert!(!pack.animations.contains_key("14"), "rejected jump");
            assert_eq!(
                pack.animations["4"].sequence,
                vec![pack.animations["2"].sequence[0]]
            );
        }
        let mut state = DeskpetState::new(pack);
        for mode in [
            DeskpetStateId::Drag,
            DeskpetStateId::Fall,
            DeskpetStateId::FallSoft,
            DeskpetStateId::FallHard,
        ] {
            state.set_state(mode);
            assert!(
                state.current_frame().is_some(),
                "{name}: unavailable interaction"
            );
        }
        state.y = 0.0;
        state.release_drag(0.0, 0.0, 20);
        for _ in 0..200 {
            state.tick(100, 300.0, 1920.0);
            assert!(state.current_frame().is_some(), "{name}: stalled playback");
        }
        assert_eq!(state.y, 300.0, "{name}: fall did not land");
    }
}