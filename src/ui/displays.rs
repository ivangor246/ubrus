use crate::{
    app::control::Output,
    library::ALL,
    preferences::{Locale, Text},
};
use std::fmt;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Display {
    pub(super) id: String,
    label: String,
    available: bool,
}
impl fmt::Display for Display {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.label)
    }
}
pub(super) fn monitors(locale: Locale, outputs: &[Output], screen: &str) -> (Vec<Display>, Display) {
    let mut options = vec![Display {
        id: ALL.to_owned(),
        label: locale.get(Text::All).into(),
        available: true,
    }];
    options.extend(outputs.iter().filter(|m| !m.id.is_empty()).map(|m| Display {
        id: m.id.clone(),
        label: if m.name.is_empty() {
            m.id.clone()
        } else {
            m.name.clone()
        },
        available: true,
    }));
    let selected = if let Some(found) = options.iter().find(|o| o.id == screen) {
        found.clone()
    } else {
        let lost = Display {
            id: screen.to_owned(),
            label: format!("{} · {screen}", locale.get(Text::Disconnected)),
            available: false,
        };
        options.push(lost.clone());
        lost
    };
    (options, selected)
}

#[cfg(test)]
#[path = "../../tests/rust/ui.rs"]
mod tests;
