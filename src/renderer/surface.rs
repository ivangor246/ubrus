use super::{Screen, StreamKey, x11};
use crate::framing::Framing;
use gtk::{gdk, glib, graphene, gsk, prelude::*, subclass::prelude::*};
use std::cell::Cell;

mod imp {
    use super::*;

    #[derive(Default)]
    pub struct Viewport {
        pub picture: gtk::Picture,
        pub framing: Cell<(bool, Framing)>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Viewport {
        const NAME: &'static str = "UbrusWallpaperViewport";
        type Type = super::Viewport;
        type ParentType = gtk::Widget;
    }
    impl ObjectImpl for Viewport {
        fn constructed(&self) {
            self.parent_constructed();
            self.obj().set_overflow(gtk::Overflow::Hidden);
            self.picture.set_content_fit(gtk::ContentFit::Fill);
            self.picture.set_parent(&*self.obj());
        }
        fn dispose(&self) {
            self.picture.unparent();
        }
    }
    impl WidgetImpl for Viewport {
        fn size_allocate(&self, width: i32, height: i32, _: i32) {
            let Some(p) = self.picture.paintable() else { return };
            let (fit, frame) = self.framing.get();
            let (x, y, w, h) = frame.bounds(
                fit,
                (p.intrinsic_width().max(1).into(), p.intrinsic_height().max(1).into()),
                (width.into(), height.into()),
            );
            let transform = gsk::Transform::new().translate(&graphene::Point::new(x as f32, y as f32));
            self.picture
                .allocate(w.round().max(1.) as i32, h.round().max(1.) as i32, -1, Some(transform));
        }
        fn snapshot(&self, snapshot: &gtk::Snapshot) {
            let brightness = self.framing.get().1.brightness;
            if brightness != 100 {
                let gain = f32::from(brightness) / 100.;
                snapshot.push_color_matrix(&graphene::Matrix::new_scale(gain, gain, gain), &graphene::Vec4::zero());
            }
            self.obj().snapshot_child(&self.picture, snapshot);
            if brightness != 100 {
                snapshot.pop();
            }
        }
    }
}
glib::wrapper! {pub struct Viewport(ObjectSubclass<imp::Viewport>) @extends gtk::Widget, @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;}

pub(super) struct WallpaperSurface {
    window: gtk::Window,
    viewport: Viewport,
    displayed: Option<StreamKey>,
}

impl WallpaperSurface {
    pub fn new(index: i32) -> Self {
        let viewport: Viewport = glib::Object::new();
        let window = gtk::Window::builder()
            .title(format!("Ubrus surface {index}"))
            .decorated(false)
            .focusable(false)
            .child(&viewport)
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
            viewport,
            displayed: None,
        }
    }

    pub fn displayed(&self) -> Option<&StreamKey> {
        self.displayed.as_ref()
    }

    pub fn show(&mut self, screen: &Screen, paintable: &gdk::Paintable) {
        let key = screen.key();
        let imp = self.viewport.imp();
        if screen.active {
            imp.picture.set_paintable(Some(paintable));
        } else if self.displayed.as_ref() != Some(&key) || imp.picture.paintable().as_ref() == Some(paintable) {
            imp.picture.set_paintable(Some(&paintable.current_image()));
        }
        if imp.framing.replace((screen.fit, screen.frame)) != (screen.fit, screen.frame) {
            self.viewport.queue_allocate();
            self.viewport.queue_draw();
        }
        self.window.set_size_request(screen.width.max(1), screen.height.max(1));
        self.window.set_default_size(screen.width.max(1), screen.height.max(1));
        self.window.set_visible(true);
        self.displayed = Some(key);
    }
}
impl Drop for WallpaperSurface {
    fn drop(&mut self) {
        self.window.close();
    }
}
