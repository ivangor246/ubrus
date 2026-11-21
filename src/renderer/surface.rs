use super::{Screen, x11};
use gtk::{gdk, prelude::*};
use std::path::{Path, PathBuf};

pub(super) struct WallpaperSurface {
    window: gtk::Window,
    picture: gtk::Picture,
    displayed: Option<PathBuf>,
}

impl WallpaperSurface {
    pub fn new(index: i32) -> Self {
        let picture = gtk::Picture::new();
        let window = gtk::Window::builder()
            .title(format!("Ubrus surface {index}"))
            .decorated(false)
            .focusable(false)
            .child(&picture)
            .build();
        window.add_css_class("ubrus-surface");
        window.connect_realize(|window| {
            if let Some(surface) = window.surface() {
                surface.set_input_region(&gtk::cairo::Region::create());
                x11::desktop(&surface);
            }
        });
        Self {
            window,
            picture,
            displayed: None,
        }
    }

    pub fn displayed(&self) -> Option<&Path> {
        self.displayed.as_deref()
    }

    pub fn show(&mut self, screen: &Screen, paintable: &gdk::Paintable) {
        let changed = self.displayed() != Some(&screen.path);
        if screen.active {
            self.picture.set_paintable(Some(paintable));
        } else if changed || self.picture.paintable().as_ref() == Some(paintable) {
            // Keep this output still while another monitor plays the same stream.
            self.picture.set_paintable(Some(&paintable.current_image()));
        }
        let width = screen.width.max(1);
        let height = screen.height.max(1);
        self.window.set_size_request(width, height);
        self.picture.set_content_fit(if screen.fit {
            gtk::ContentFit::Contain
        } else {
            gtk::ContentFit::Cover
        });
        self.window.set_default_size(width, height);
        self.window.set_visible(true);
        if changed {
            self.displayed = Some(screen.path.clone());
        }
    }
}

impl Drop for WallpaperSurface {
    fn drop(&mut self) {
        self.window.close();
    }
}
