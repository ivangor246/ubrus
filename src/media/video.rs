use super::{VideoPipeline, gif::Gif};
use anyhow::{Context, Result};
use gst::prelude::*;
use gtk::{gdk, glib};
use std::{
    cell::Cell,
    path::Path,
    rc::Rc,
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    },
};

struct GstVideo {
    pipeline: VideoPipeline,
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
}

impl GstVideo {
    pub fn new(path: &Path, notice: impl Fn(Result<()>) + 'static) -> Result<Self> {
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
                            } else if let Err(e) =
                                p.seek_simple(gst::SeekFlags::FLUSH | gst::SeekFlags::KEY_UNIT, gst::ClockTime::ZERO)
                            {
                                notice(Err(e.into()));
                            }
                        }
                        gst::MessageView::AsyncDone(_) if !ready => {
                            ready = true;
                            notice(Ok(()));
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
            self.pipeline
                .seek_simple(gst::SeekFlags::FLUSH | gst::SeekFlags::KEY_UNIT, gst::ClockTime::ZERO)?;
            self.playback.at_end.set(false);
        }
        self.pipeline.set_state(if active {
            gst::State::Playing
        } else {
            gst::State::Paused
        })?;
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
enum Backend {
    Stream(GstVideo),
    Gif(Rc<Gif>),
}
pub struct Video {
    pub paintable: gdk::Paintable,
    backend: Backend,
}
impl Video {
    pub fn new(path: &Path, notice: impl Fn(Result<()>) + 'static) -> Result<Self> {
        if path.extension().is_some_and(|ext| ext == "gif") {
            let gif = Gif::open(path)?;
            let paintable = gif.frames.clone().upcast();
            glib::idle_add_local_once(move || notice(Ok(())));
            Ok(Self {
                paintable,
                backend: Backend::Gif(gif),
            })
        } else {
            let stream = GstVideo::new(path, notice)?;
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
                gif.play(active);
                Ok(())
            }
        }
    }
    pub fn stats(&self) -> serde_json::Value {
        match &self.backend {
            Backend::Stream(stream) => stream.stats(),
            Backend::Gif(g) => {
                serde_json::json!({"decoder":"GdkPixbuf GIF","playing":g.is_playing(),"paused":!g.is_playing(),"decoded":g.decoded.get(),"rendered":g.decoded.get(),"dropped":0})
            }
        }
    }
}
