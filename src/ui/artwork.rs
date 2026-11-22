use gtk::{gdk_pixbuf::PixbufLoader, prelude::*};
use iced::advanced::{Layout, Widget, image as raster, layout, renderer, widget::Tree};
use iced::{Color, Element, Length, Rectangle, Renderer, Size, Theme, mouse, widget::image::Handle};
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

pub fn brand() -> Handle {
    icon("brand", Color::BLACK)
}

fn icon(name: &'static str, color: Color) -> Handle {
    type Icons = Mutex<BTreeMap<(&'static str, [u8; 3]), Handle>>;
    static CACHE: OnceLock<Icons> = OnceLock::new();
    let mut cache = CACHE.get_or_init(Default::default).lock().unwrap();
    let [r, g, b, _] = color.into_rgba8();
    cache
        .entry((name, [r, g, b]))
        .or_insert_with(|| {
            let loader = PixbufLoader::with_type("svg").expect("installed SVG loader");
            loader.set_size(64, 64);
            let svg = svg(name).replace("currentColor", &format!("#{r:02x}{g:02x}{b:02x}"));
            loader.write(svg.as_bytes()).unwrap();
            loader.close().unwrap();
            let image = loader.pixbuf().unwrap();
            Handle::from_rgba(64, 64, image.read_pixel_bytes().as_ref().to_vec())
        })
        .clone()
}

// Follow the containing widget's foreground, including hover and disabled states.
struct Symbol {
    name: &'static str,
    size: f32,
}

pub fn symbol<'a, Message: 'a>(name: &'static str, size: f32) -> Element<'a, Message> {
    Element::new(Symbol { name, size })
}
impl<Message> Widget<Message, Theme, Renderer> for Symbol {
    fn size(&self) -> Size<Length> {
        Size::new(self.size.into(), self.size.into())
    }
    fn layout(&mut self, _: &mut Tree, _: &Renderer, limits: &layout::Limits) -> layout::Node {
        layout::atomic(limits, self.size, self.size)
    }
    fn draw(
        &self,
        _: &Tree,
        renderer: &mut Renderer,
        _: &Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        _: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        use raster::Renderer as _;
        let bounds = layout.bounds();
        if let Some(clip) = bounds.intersection(viewport) {
            let image = raster::Image::new(icon(self.name, style.text_color)).opacity(style.text_color.a);
            renderer.draw_image(image, bounds, clip);
        }
    }
}
