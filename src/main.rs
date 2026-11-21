mod app;
mod desktop;
mod files;
mod framing;
mod library;
mod media;
mod preferences;
mod renderer;
mod ui;

use anyhow::{Result, anyhow};
use gtk::glib;

fn main() -> glib::ExitCode {
    match run() {
        Ok(code) => code,
        Err(e) => {
            eprintln!("Ubrus: {e:#}");
            glib::ExitCode::FAILURE
        }
    }
}

fn run() -> Result<glib::ExitCode> {
    if std::env::args_os().nth(1).as_deref() == Some(std::ffi::OsStr::new("--renderer")) {
        renderer::run()?;
        return Ok(glib::ExitCode::SUCCESS);
    }
    let (events, inbox) = async_channel::unbounded();
    let (commands, requests) = async_channel::unbounded();
    let panel = commands.clone();
    let service = std::thread::spawn(move || app::run(events, requests, panel));
    // Background and forwarded launches do not initialize a display or a renderer.
    let gui = loop {
        match inbox.recv_blocking() {
            Ok(app::Event::Open(initial)) => break ui::run(initial, inbox, commands.clone()),
            Ok(app::Event::Shutdown) | Err(_) => break Ok(()),
            _ => continue,
        }
    };
    commands.close();
    let result = service
        .join()
        .map_err(|_| anyhow!("Session service stopped unexpectedly"))?;
    gui?;
    desktop::updates::relaunch_if_requested()?;
    Ok(result)
}
