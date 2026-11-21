use anyhow::Result;
use gtk::{
    gdk,
    gdk_pixbuf::{PixbufAnimation, PixbufAnimationIter},
    glib,
    prelude::*,
    subclass::prelude::*,
};
use std::{
    cell::{Cell, RefCell},
    path::Path,
    rc::Rc,
    time::{Duration, Instant, SystemTime},
};

mod imp {
    use super::*;
    #[derive(Default)]
    pub struct Frames(pub RefCell<Option<gdk::Texture>>);
    #[glib::object_subclass]
    impl ObjectSubclass for Frames {
        const NAME: &'static str = "UbrusGifFrames";
        type Type = super::Frames;
        type Interfaces = (gdk::Paintable,);
    }
    impl ObjectImpl for Frames {}
    impl PaintableImpl for Frames {
        fn snapshot(&self, s: &gdk::Snapshot, w: f64, h: f64) {
            if let Some(texture) = self.0.borrow().as_ref() {
                texture.snapshot(s, w, h);
            }
        }
        fn current_image(&self) -> gdk::Paintable {
            self.0
                .borrow()
                .as_ref()
                .expect("initialized GIF frame")
                .clone()
                .upcast()
        }
        fn intrinsic_width(&self) -> i32 {
            self.0.borrow().as_ref().map_or(0, TextureExt::width)
        }
        fn intrinsic_height(&self) -> i32 {
            self.0.borrow().as_ref().map_or(0, TextureExt::height)
        }
    }
}
glib::wrapper! {pub struct Frames(ObjectSubclass<imp::Frames>) @implements gdk::Paintable;}
pub struct Gif {
    rate: f64,
    animation: PixbufAnimation,
    iter: RefCell<PixbufAnimationIter>,
    pub frames: Frames,
    timer: RefCell<Option<glib::SourceId>>,
    elapsed: Cell<Duration>,
    started: Cell<Option<Instant>>,
    pub decoded: Cell<u64>,
}
impl Gif {
    pub fn open(path: &Path, speed: u16) -> Result<Rc<Self>> {
        let animation = PixbufAnimation::from_file(path)?;
        let iter = animation.iter(Some(SystemTime::UNIX_EPOCH));
        let this = Rc::new(Self {
            rate: f64::from(speed) / 100.,
            animation,
            iter: iter.into(),
            frames: glib::Object::new(),
            timer: RefCell::default(),
            elapsed: Cell::new(Duration::ZERO),
            started: Cell::new(None),
            decoded: Cell::new(0),
        });
        this.frame();
        Ok(this)
    }
    fn frame(&self) {
        let pixbuf = self.iter.borrow().pixbuf();
        // The iterator reuses its compositing buffer. Each published texture owns a stable frame.
        let texture = gdk::Texture::for_pixbuf(&pixbuf.copy().unwrap_or(pixbuf));
        *self.frames.imp().0.borrow_mut() = Some(texture);
        self.frames.invalidate_contents();
        self.decoded.set(self.decoded.get() + 1);
    }
    pub fn is_playing(&self) -> bool {
        self.timer.borrow().is_some()
    }
    pub fn play(self: &Rc<Self>, active: bool) {
        if !active {
            if let Some(start) = self.started.take() {
                self.elapsed
                    .set(self.elapsed.get() + start.elapsed().mul_f64(self.rate));
            }
            if let Some(id) = self.timer.take() {
                id.remove();
            }
            return;
        }
        if self.started.get().is_some() || self.animation.is_static_image() {
            return;
        }
        self.started.set(Some(Instant::now()));
        self.tick();
    }

    fn tick(self: &Rc<Self>) {
        let Some(start) = self.started.get() else { return };
        let now = Instant::now();
        let position = self.elapsed.get() + now.duration_since(start).mul_f64(self.rate);
        let mut changed = self.iter.borrow().advance(SystemTime::UNIX_EPOCH + position);
        if self.iter.borrow().delay_time().is_none() {
            self.elapsed.set(Duration::ZERO);
            self.started.set(Some(now));
            self.iter.replace(self.animation.iter(Some(SystemTime::UNIX_EPOCH)));
            changed = true;
        }
        if changed {
            self.frame();
        }
        let delay = self
            .iter
            .borrow()
            .delay_time()
            .unwrap_or(Duration::from_millis(100))
            .div_f64(self.rate)
            .saturating_sub(now.elapsed())
            .max(Duration::from_millis(1));
        let weak = Rc::downgrade(self);
        *self.timer.borrow_mut() = Some(glib::timeout_add_local_once(delay, move || {
            if let Some(g) = weak.upgrade() {
                g.timer.take();
                g.tick();
            }
        }));
    }
}
impl Drop for Gif {
    fn drop(&mut self) {
        if let Some(id) = self.timer.take() {
            id.remove();
        }
    }
}

#[cfg(test)]
#[path = "../../tests/rust/gif.rs"]
mod tests;
