use super::*;

fn check(ink: Color, background: Background, minimum: f32) {
    let Background::Color(surface) = background else {
        panic!("Expected a flat surface")
    };
    assert_eq!(ink.a, 1.);
    assert_eq!(surface.a, 1.);
    let ratio = ink.relative_contrast(surface);
    assert!(
        ratio >= minimum,
        "{ink:?} on {surface:?}: {ratio:.2}, expected {minimum}"
    );
}

#[test]
fn labels_and_control_symbols_contrast_with_their_actual_surfaces() {
    for dark in [false, true] {
        for high_contrast in [false, true] {
            let theme = theme(dark, high_contrast);
            let palette = theme.palette();
            for ink in [palette.text, muted(&theme), palette.success, palette.warning] {
                check(ink, palette.background.into(), if high_contrast { 7. } else { 4.5 });
            }
            for state in [
                button::Status::Active,
                button::Status::Hovered,
                button::Status::Pressed,
                button::Status::Disabled,
            ] {
                for style in [subtle(&theme, state), primary(&theme, state), danger(&theme, state)] {
                    check(
                        style.text_color,
                        style.background.unwrap(),
                        if high_contrast { 7. } else { 4.5 },
                    );
                }
            }
            for state in [
                checkbox::Status::Active { is_checked: true },
                checkbox::Status::Hovered { is_checked: true },
                checkbox::Status::Disabled { is_checked: true },
            ] {
                let style = check_box(&theme, state);
                check(style.icon_color, style.background, 4.5);
                check(style.border.color, palette.background.into(), 3.);
            }
            let style = picker(&theme, pick_list::Status::Active);
            check(style.handle_color, style.background, 4.5);
            for state in [slider::Status::Active, slider::Status::Hovered, slider::Status::Dragged] {
                let rail = slider_rail(&theme, state).rail.backgrounds;
                check(palette.background, rail.0, 3.);
                check(palette.background, rail.1, 3.);
            }
        }
    }
}
