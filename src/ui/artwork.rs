use gtk::{gdk_pixbuf::PixbufLoader, prelude::*};
use iced::widget::image::Handle;
use std::{
    collections::BTreeMap,
    sync::{Mutex, OnceLock},
};

macro_rules! icons {
    ($($name:literal),*) => { fn svg(name: &str) -> &'static str { match name {
        $($name => include_str!(concat!("../../extension/icons/",$name,"-symbolic.svg")),)*
        _ => include_str!("../../packaging/desktop/org.ubrus.Ubrus.svg")
    }} };
}
icons!(
    "open", "library", "settings", "add", "back", "play", "pause", "hide", "show", "power", "panel", "check", "more",
    "trash", "reset", "report", "update"
);

pub fn icon(name: &'static str, dark: bool) -> Handle {
    type Icons = Mutex<BTreeMap<(&'static str, bool), Handle>>;
    static CACHE: OnceLock<Icons> = OnceLock::new();
    let mut cache = CACHE.get_or_init(Default::default).lock().unwrap();
    cache
        .entry((name, dark))
        .or_insert_with(|| {
            let loader = PixbufLoader::with_type("svg").expect("installed SVG loader");
            loader.set_size(64, 64);
            let svg = svg(name).replace("currentColor", if dark { "#e3eee9" } else { "#24483f" });
            loader.write(svg.as_bytes()).unwrap();
            loader.close().unwrap();
            let image = loader.pixbuf().unwrap();
            Handle::from_rgba(64, 64, image.read_pixel_bytes().as_ref().to_vec())
        })
        .clone()
}
