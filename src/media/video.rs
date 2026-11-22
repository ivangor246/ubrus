use super::{
    VideoPipeline,
    gif::{Frames, Gif},
};
use anyhow::{Context, Result};
use gst::prelude::*;
use gtk::{gdk, glib};
use std::{
    cell::{Cell, OnceCell},
    path::Path,
    rc::Rc,
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    },
};

pub enum VideoEvent {
    Ready,
    StateChanged,
}

struct GstVideo {
    pipeline: VideoPipeline,
    speed: u16,
    sink: gst::Element,
    pub paintable: gdk::Paintable,
    decoder: Arc<Mutex<String>>,
    decoded: Arc<AtomicU64>,
    playback: Rc<Playback>,
    _watch: gst::bus::BusWatchGuard,
}

#[derive(Default)]
struct Playback {
    active: Cell<bool>,
    at_end: Cell<bool>,
    last_stable_state: Cell<Option<gst::State>>,
}

impl GstVideo {
    pub fn new(path: &Path, speed: u16, notice: impl Fn(Result<VideoEvent>) + 'static) -> Result<Self> {
        let sink = gst::ElementFactory::make("gtk4paintablesink").build()?;
        let paintable = sink.property::<gdk::Paintable>("paintable");
        let output = if paintable.property::<Option<gdk::GLContext>>("gl-context").is_some() {
            gst::ElementFactory::make("glsinkbin")
                .property("sink", &sink)
                .build()
                .unwrap_or_else(|_| sink.clone())
        } else {
            sink.clone()
        };
        let pipeline = VideoPipeline::open(path, &output)?;
        let decoder = Arc::new(Mutex::new(String::from("…")));
        let current = decoder.clone();
        let decoded = Arc::new(AtomicU64::new(0));
        let frames = decoded.clone();
        pipeline
            .downcast_ref::<gst::Bin>()
            .unwrap()
            .connect_deep_element_added(move |_, _, element| {
                if let Some(f) = element.factory()
                    && f.klass().contains("Decoder")
                    && f.klass().contains("Video")
                {
                    *current.lock().unwrap() = f.name().to_string();
                    if let Some(pad) = element.static_pad("src") {
                        let frames = frames.clone();
                        pad.add_probe(gst::PadProbeType::BUFFER, move |_, _| {
                            frames.fetch_add(1, Ordering::Relaxed);
                            gst::PadProbeReturn::Ok
                        });
                    }
                }
            });
        let weak = pipeline.downgrade();
        let playback = Rc::new(Playback::default());
        let requested = playback.clone();
        let mut ready = false;
        let watch = pipeline
            .bus()
            .context("Missing GStreamer bus")?
            .add_watch_local(move |_, msg| {
                if let Some(p) = weak.upgrade() {
                    match msg.view() {
                        gst::MessageView::Eos(_) => {
                            if !requested.active.get() {
                                requested.at_end.set(true);
                            } else if let Err(e) = super::seek_start(&p, speed) {
                                notice(Err(e));
                            }
                        }
                        gst::MessageView::AsyncDone(_) if !ready => {
                            ready = true;
                            notice(if speed == 100 {
                                Ok(VideoEvent::Ready)
                            } else {
                                super::seek_start(&p, speed).map(|()| VideoEvent::Ready)
                            });
                        }
                        gst::MessageView::StateChanged(change)
                            if ready
                                && msg.src() == Some(p.upcast_ref::<gst::Object>())
                                && change.pending() == gst::State::VoidPending
                                && matches!(change.current(), gst::State::Paused | gst::State::Playing)
                                && requested.last_stable_state.get() != Some(change.current()) =>
                        {
                            requested.last_stable_state.set(Some(change.current()));
                            notice(Ok(VideoEvent::StateChanged));
                        }
                        gst::MessageView::Error(e) => notice(Err(anyhow::anyhow!("{}", e.error()))),
                        _ => {}
                    }
                }
                glib::ControlFlow::Continue
            })?;
        pipeline.set_state(gst::State::Paused)?;
        Ok(Self {
            pipeline,
            speed,
            sink,
            paintable,
            decoder,
            decoded,
            playback,
            _watch: watch,
        })
    }
    pub fn play(&self, active: bool) -> Result<()> {
        if active == self.playback.active.get() {
            return Ok(());
        }
        if active && self.playback.at_end.get() {
            super::seek_start(&self.pipeline, self.speed)?;
            self.playback.at_end.set(false);
        }
        self.pipeline.set_state(if active {
            gst::State::Playing
        } else {
            gst::State::Paused
        })?;
        // A rapid play/pause can settle back in the same state as the previous request.
        self.playback.last_stable_state.set(None);
        self.playback.active.set(active);
        Ok(())
    }
    pub fn stats(&self) -> serde_json::Value {
        let s = self.sink.property::<gst::Structure>("stats");
        let (result, state, pending) = self.pipeline.state(gst::ClockTime::ZERO);
        let paused = !self.playback.active.get()
            && state == gst::State::Paused
            && pending == gst::State::VoidPending
            && result == Ok(gst::StateChangeSuccess::Success);
        serde_json::json!({"playing":state==gst::State::Playing, "paused":paused, "decoded":self.decoded.load(Ordering::Relaxed),"decoder":*self.decoder.lock().unwrap(), "rendered":s.get::<u64>("rendered").unwrap_or(0),
            "dropped":s.get::<u64>("dropped").unwrap_or(0)})
    }
}
struct AnimatedImage {
    loaded: Rc<OnceCell<Rc<Gif>>>,
    load: glib::JoinHandle<()>,
}
impl Drop for AnimatedImage {
    fn drop(&mut self) {
        self.load.abort();
    }
}
enum Backend {
    Stream(GstVideo),
    Gif(AnimatedImage),
}
pub struct Video {
    pub paintable: gdk::Paintable,
    backend: Backend,
}
impl Video {
    pub fn new(path: &Path, speed: u16, notice: impl Fn(Result<VideoEvent>) + 'static) -> Result<Self> {
        if path.extension().is_some_and(|ext| ext == "gif" || ext == "webp") {
            let frames: Frames = glib::Object::new();
            let paintable = frames.clone().upcast();
            let path = path.to_owned();
            let loaded = Rc::new(OnceCell::new());
            let receiver = loaded.clone();
            let load = glib::spawn_future_local(async move {
                let result = Gif::load(&path, speed, frames).await.map(|gif| {
                    let _ = receiver.set(gif);
                    VideoEvent::Ready
                });
                notice(result);
            });
            Ok(Self {
                paintable,
                backend: Backend::Gif(AnimatedImage { loaded, load }),
            })
        } else {
            let stream = GstVideo::new(path, speed, notice)?;
            Ok(Self {
                paintable: stream.paintable.clone(),
                backend: Backend::Stream(stream),
            })
        }
    }
    pub fn play(&self, active: bool) -> Result<()> {
        match &self.backend {
            Backend::Stream(stream) => stream.play(active),
            Backend::Gif(gif) => {
                if let Some(gif) = gif.loaded.get() {
                    gif.play(active);
                }
                Ok(())
            }
        }
    }
    pub fn stats(&self) -> serde_json::Value {
        match &self.backend {
            Backend::Stream(stream) => stream.stats(),
            Backend::Gif(image) => {
                let loaded = image.loaded.get();
                let playing = loaded.is_some_and(|gif| gif.is_playing());
                let frames = loaded.map_or(0, |gif| gif.decoded.get());
                serde_json::json!({"decoder":"GdkPixbuf GIF/WebP","playing":playing,"paused":loaded.is_some() && !playing,"decoded":frames,"rendered":frames,"dropped":null})
            }
        }
    }
}
