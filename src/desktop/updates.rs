use std::{
    fs,
    os::unix::fs::MetadataExt,
    path::{Path, PathBuf},
    sync::OnceLock,
    sync::atomic::{AtomicBool, Ordering},
};

const METADATA: &str = "share/gnome-shell/extensions/wallpaper@ubrus.org/metadata.json";
static RESTART: AtomicBool = AtomicBool::new(false);
struct Installation {
    executable: PathBuf,
    metadata: PathBuf,
}

fn installation() -> Option<&'static Installation> {
    static INSTALLATION: OnceLock<Option<Installation>> = OnceLock::new();
    INSTALLATION
        .get_or_init(|| {
            if std::env::var_os("SNAP").is_some() {
                return None;
            }
            let executable = std::env::current_exe().ok()?;
            if !executable.ends_with("bin/ubrus") {
                return None;
            }
            let metadata = executable.parent()?.parent()?.join(METADATA);
            metadata.is_file().then_some(Installation { executable, metadata })
        })
        .as_ref()
}
pub fn installed_executable() -> Option<&'static Path> {
    installation().map(|i| i.executable.as_path())
}

pub fn binary_changed() -> bool {
    let Some(path) = installed_executable() else {
        return false;
    };
    match (fs::metadata("/proc/self/exe"), fs::metadata(path)) {
        (Ok(old), Ok(new)) => (old.dev(), old.ino()) != (new.dev(), new.ino()),
        _ => false,
    }
}

pub fn extension_changed(loaded: &str) -> bool {
    let Some(install) = installation() else { return false };
    fs::read(&install.metadata)
        .ok()
        .and_then(|data| serde_json::from_slice::<serde_json::Value>(&data).ok())
        .and_then(|m| m["build-id"].as_str().map(str::to_owned))
        .is_some_and(|installed| installed != loaded)
}
pub fn request_restart() {
    RESTART.store(true, Ordering::Relaxed);
}
pub fn relaunch_if_requested() -> std::io::Result<()> {
    use std::os::unix::process::CommandExt;
    if RESTART.load(Ordering::Relaxed)
        && let Some(path) = installed_executable()
    {
        return Err(std::process::Command::new(path).exec());
    }
    Ok(())
}

pub fn watch(events: async_channel::Sender<crate::app::Event>) -> Vec<gtk::gio::FileMonitor> {
    use gtk::gio::{self, prelude::*};
    let Some(install) = installation() else { return vec![] };
    [&install.executable, &install.metadata]
        .into_iter()
        .filter_map(|path| {
            let file = gio::File::for_path(path);
            let monitor = file
                .parent()?
                .monitor_directory(gio::FileMonitorFlags::WATCH_MOVES, gio::Cancellable::NONE)
                .ok()?;
            let tx = events.clone();
            monitor.connect_changed(move |_, changed, other, _| {
                if file.equal(changed) || other.is_some_and(|moved| file.equal(moved)) {
                    let _ = tx.try_send(crate::app::Event::Updates);
                }
            });
            Some(monitor)
        })
        .collect()
}
