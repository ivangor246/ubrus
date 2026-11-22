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

fn snap_extension_changed(mount: &Path, loaded: &str) -> bool {
    // A running process retains its old revision in SNAP after a refresh.
    let build = Path::new("share/ubrus/extension-build-id");
    let current = mount.parent().unwrap_or(mount).join("current").join(build);
    fs::read_to_string(current)
        .or_else(|_| fs::read_to_string(mount.join(build)))
        .is_ok_and(|installed| !installed.trim().is_empty() && installed.trim() != loaded)
}

pub fn extension_changed(loaded: &str) -> bool {
    if let Some(mount) = std::env::var_os("SNAP") {
        return snap_extension_changed(Path::new(&mount), loaded);
    }
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
    let paths = if let Some(mount) = std::env::var_os("SNAP") {
        Path::new(&mount)
            .parent()
            .map(|p| vec![p.join("current")])
            .unwrap_or_default()
    } else if let Some(install) = installation() {
        vec![install.executable.clone(), install.metadata.clone()]
    } else {
        return Vec::new();
    };
    paths
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

#[cfg(test)]
#[path = "../../tests/rust/updates.rs"]
mod tests;
