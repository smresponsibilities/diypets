use diypets::{
    deskpet_state::{DeskpetState, DeskpetStateId},
    deskpet_xml::DeskpetPack,
    studio_model::StudioProject,
};

#[test]
fn exported_snake_turns_away_from_screen_edge() {
    let (_, _, xml) = StudioProject::template_snake()
        .build_export_artifacts()
        .unwrap();
    let nested = "<Frame>8</Frame><Frame>9</Frame><Action>flip</Action>";
    assert!(
        xml.contains(nested),
        "turn action must work with older players"
    );
    let sibling = xml
        .replace(nested, "<Frame>8</Frame><Frame>9</Frame>")
        .replace(
            "<Name>turn</Name>",
            "<Name>turn</Name><Action>flip</Action>",
        );
    for xml in [&xml, &sibling] {
        let mut snake = DeskpetState::new(DeskpetPack::parse(xml).unwrap());
        snake.climb_enabled = false;
        snake.x = 0.0;
        snake.y = 800.0;
        snake.is_moving_left = true;
        snake.set_state(DeskpetStateId::Custom("1".into()));
        for _ in 0..200 {
            snake.tick(16, 800.0, 1920.0);
        }
        assert!(
            snake.x > 0.0,
            "snake remained at the left edge: state {}",
            snake.current_anim_id
        );
    }
}
