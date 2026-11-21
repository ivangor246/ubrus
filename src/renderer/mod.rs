mod surface;
mod x11;

use surface::WallpaperSurface;

use crate::media::Video;
use anyhow::{Context, Result};
use gtk::{gio, glib, prelude::*};
use serde::Deserialize;
use serde_json::json;
use std::{
    cell::RefCell,
    collections::{BTreeMap, BTreeSet},
    io::{self, Write},
    os::fd::AsFd,
    path::PathBuf,
    rc::Rc,
};

#[derive(Deserialize, Clone)]
struct Screen {
    index: i32,
    path: PathBuf,
    width: i32,
    height: i32,
    fit: bool,
    #[serde(default)]
    frame: crate::framing::Framing,
    active: bool,
}
type StreamKey = (PathBuf, u16);
impl Screen {
    fn key(&self) -> StreamKey {
        (self.path.clone(), self.frame.speed)
    }
}
#[derive(Deserialize)]
struct Layout {
    serial: u64,
    screens: Vec<Screen>,
}
#[derive(Default)]
struct Engine {
    streams: BTreeMap<StreamKey, Video>,
    windows: BTreeMap<i32, WallpaperSurface>,
    desired: Vec<Screen>,
    serial: u64,
    ready: BTreeSet<StreamKey>,
}
fn emit(message: serde_json::Value) {
    let _ = writeln!(io::stdout().lock(), "UBRUS/1 {message}");
}
impl Engine {
    fn configure(this: &Rc<RefCell<Self>>, layout: Layout) -> Result<()> {
        let mut e = this.borrow_mut();
        anyhow::ensure!(
            layout.screens.iter().all(|s| s.frame.valid()),
            "Invalid wallpaper adjustments"
        );
        e.desired = layout.screens;
        e.serial = layout.serial;
        let requested: BTreeSet<_> = e.desired.iter().map(Screen::key).collect();
        for path in requested {
            if e.streams.contains_key(&path) {
                continue;
            }
            let weak = Rc::downgrade(this);
            let key = path.clone();
            let video = Video::new(&path.0, path.1, move |result| {
                let Some(engine) = weak.upgrade() else {
                    return;
                };
                // Bus callbacks and surface updates both run on the GTK main context.
                let mut e = engine.borrow_mut();
                if !e.desired.iter().any(|s| s.key() == key) {
                    return;
                }
                match result {
                    Ok(()) => {
                        e.ready.insert(key.clone());
                        if let Err(err) = e.update() {
                            emit(json!({"error":err.to_string(), "serial":e.serial}));
                        }
                    }
                    Err(err) => {
                        emit(json!({"error": err.to_string(), "serial":e.serial}));
                    }
                }
            })?;
            e.streams.insert(path, video);
        }
        e.update()
    }
    fn update(&mut self) -> Result<()> {
        let indices: BTreeSet<_> = self.desired.iter().map(|s| s.index).collect();
        self.windows.retain(|index, _| indices.contains(index));
        for screen in &self.desired {
            if self.ready.contains(&screen.key()) {
                self.windows
                    .entry(screen.index)
                    .or_insert_with(|| WallpaperSurface::new(screen.index))
                    .show(screen, &self.streams[&screen.key()].paintable);
            }
        }

        // During preroll, a window can still display the previous assignment.
        self.streams.retain(|path, _| {
            self.desired.iter().any(|s| s.key() == *path)
                || self.windows.values().any(|window| window.displayed() == Some(path))
        });
        self.ready.retain(|key| self.streams.contains_key(key));
        for (key, stream) in &self.streams {
            let active = self
                .desired
                .iter()
                .any(|s| s.active && self.windows.get(&s.index).and_then(WallpaperSurface::displayed) == Some(key));
            stream.play(active)?;
        }
        self.status();
        Ok(())
    }
    fn status(&self) {
        let displayed: BTreeMap<_, _> = self
            .windows
            .iter()
            .filter_map(|(index, window)| Some((index, &window.displayed()?.0)))
            .collect();
        emit(json!({
            "pid": std::process::id(),
            "serial": self.serial,
            "displayed": displayed,
            "speeds": self.windows.iter().filter_map(|(i,w)| Some((i,w.displayed()?.1))).collect::<BTreeMap<_,_>>(),
            "streams": self.streams.values().map(Video::stats).collect::<Vec<_>>(),
            "windows": self.windows.len(),
        }));
    }
}
pub fn run() -> Result<()> {
    use sha2::{Digest, Sha256};
    use std::os::unix::ffi::OsStrExt;
    let session = std::env::var_os("DBUS_SESSION_BUS_ADDRESS").unwrap_or_default();
    let display = std::env::var_os("WAYLAND_DISPLAY")
        .or_else(|| std::env::var_os("DISPLAY"))
        .unwrap_or_default();
    let key = Sha256::digest([session.as_bytes(), b"\0", display.as_bytes()].concat());
    let lock_path = glib::user_runtime_dir().join(format!("ubrus-renderer-{key:x}.lock"));
    let lock = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(false)
        .open(lock_path)?;
    lock.try_lock()
        .context("Another wallpaper renderer already owns this display")?;
    gtk::init()?;
    crate::media::init()?;
    let provider = gtk::CssProvider::new();
    provider.load_from_string("window.ubrus-surface { background: black; box-shadow: none; }");
    gtk::style_context_add_provider_for_display(
        &gtk::gdk::Display::default().context("Missing display")?,
        &provider,
        gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );
    let event_loop = glib::MainLoop::new(None, false);
    let stop = event_loop.clone();
    let engine = Rc::new(RefCell::new(Engine::default()));
    let input = gio::DataInputStream::new(&gio::UnixInputStream::take_fd(
        io::stdin().as_fd().try_clone_to_owned()?,
    ));
    glib::spawn_future_local(async move {
        while let Ok(Some(line)) = input.read_line_utf8_future(glib::Priority::DEFAULT).await {
            if line == "status" {
                engine.borrow().status();
                continue;
            }
            let res = serde_json::from_str(&line)
                .map_err(anyhow::Error::from)
                .and_then(|s| Engine::configure(&engine, s));
            if let Err(err) = res {
                emit(json!({"error":format!("{err:#}"),"serial":engine.borrow().serial}));
            }
        }
        stop.quit();
    });
    event_loop.run();
    Ok(())
}
