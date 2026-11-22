use iced::{
    Background, Color, Theme, color,
    theme::Palette,
    widget::{button, checkbox, pick_list, slider},
};

pub fn theme(dark: bool, high_contrast: bool) -> Theme {
    let for_scheme = |light, night| if dark { night } else { light };
    let palette = Palette {
        background: if high_contrast {
            for_scheme(Color::WHITE, Color::BLACK)
        } else {
            for_scheme(color!(0xf7f8f3), color!(0x192320))
        },
        text: if high_contrast {
            for_scheme(Color::BLACK, Color::WHITE)
        } else {
            for_scheme(color!(0x20332e), color!(0xe6eee7))
        },
        primary: if high_contrast {
            for_scheme(color!(0x063e33), color!(0xceffe9))
        } else {
            for_scheme(color!(0x145c4e), color!(0x9de5c7))
        },
        success: for_scheme(color!(0x175b42), color!(0x9ce2b7)),
        warning: for_scheme(color!(0x714300), color!(0xffcf80)),
        danger: if high_contrast {
            for_scheme(color!(0x761121), color!(0xffd1d5))
        } else {
            for_scheme(color!(0x9b2933), color!(0xffa7a7))
        },
    };
    Theme::custom("Ubrus", palette)
}

pub fn muted(theme: &Theme) -> Color {
    if theme.extended_palette().is_dark {
        color!(0xb9c9c2)
    } else {
        color!(0x35463e)
    }
}

fn foreground(surface: Color, preferred: Color) -> Color {
    if surface.relative_contrast(preferred) >= 7.0 {
        preferred
    } else if surface.relative_contrast(Color::WHITE) > surface.relative_contrast(Color::BLACK) {
        Color::WHITE
    } else {
        Color::BLACK
    }
}

fn readable_button(theme: &Theme, status: button::Status, mut style: button::Style) -> button::Style {
    if status == button::Status::Disabled {
        style.background = Some(theme.extended_palette().background.weak.color.into());
        style.text_color = muted(theme);
    } else if let Some(Background::Color(surface)) = style.background {
        style.text_color = foreground(surface, theme.palette().text);
    }
    style
}

pub fn subtle(theme: &Theme, status: button::Status) -> button::Style {
    readable_button(theme, status, button::subtle(theme, status))
}
pub fn primary(theme: &Theme, status: button::Status) -> button::Style {
    readable_button(theme, status, button::primary(theme, status))
}
pub fn danger(theme: &Theme, status: button::Status) -> button::Style {
    readable_button(theme, status, button::danger(theme, status))
}

pub fn check_box(theme: &Theme, status: checkbox::Status) -> checkbox::Style {
    let mut style = checkbox::primary(theme, status);
    style.border.color = muted(theme);
    if let Background::Color(surface) = style.background {
        style.icon_color = foreground(surface, theme.palette().text);
    }
    if matches!(status, checkbox::Status::Disabled { .. }) {
        style.text_color = Some(muted(theme));
    }
    style
}

pub fn picker(theme: &Theme, status: pick_list::Status) -> pick_list::Style {
    let mut style = pick_list::default(theme, status);
    if matches!(status, pick_list::Status::Active) {
        style.border.color = muted(theme);
    }
    style
}

pub fn slider_rail(theme: &Theme, status: slider::Status) -> slider::Style {
    let mut style = slider::default(theme, status);
    let accent = match status {
        slider::Status::Active => theme.palette().primary,
        slider::Status::Hovered => theme.extended_palette().primary.strong.color,
        slider::Status::Dragged => theme.palette().text,
    };
    let track = if theme.extended_palette().is_dark {
        color!(0x718078)
    } else {
        color!(0x78857e)
    };
    style.rail.backgrounds = (accent.into(), track.into());
    style.handle.background = accent.into();
    style.handle.border_color = theme.palette().background;
    style.handle.border_width = 2.0;
    style
}

#[cfg(test)]
#[path = "../../tests/rust/contrast.rs"]
mod tests;
