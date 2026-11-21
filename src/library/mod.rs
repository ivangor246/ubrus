mod profile;
use crate::files::{write_atomic, xdg_directory};
use anyhow::{Context, Result, ensure};
pub use profile::{ALL, Choice, Item, Profile};
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File, OpenOptions},
    io::{BufReader, Read, Write},
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};
use tempfile::NamedTempFile;

#[derive(Clone, Debug)]
pub struct Store(PathBuf);
impl Store {
    pub fn open() -> Result<Self> {
        let root = if let Some(path) = std::env::var_os("SNAP_USER_COMMON") {
            PathBuf::from(path)
        } else {
            xdg_directory("XDG_DATA_HOME", ".local/share")?.join("ubrus")
        };
        Self::at(root)
    }
    pub fn at(root: PathBuf) -> Result<Self> {
        fs::create_dir_all(root.join("media"))?;
        fs::create_dir_all(root.join("thumbs"))?;
        Ok(Self(root))
    }
    pub fn root(&self) -> &Path {
        &self.0
    }
    pub fn profile_path(&self) -> PathBuf {
        self.0.join("profile.json")
    }
    pub fn media(&self, id: &str, item: &Item) -> PathBuf {
        self.0.join("media").join(format!("{id}.{}", item.ext))
    }
    pub fn thumb(&self, id: &str) -> PathBuf {
        self.0.join("thumbs").join(format!("{id}.png"))
    }
    pub fn load(&self) -> Result<Profile> {
        let file = match File::open(self.profile_path()) {
            Ok(file) => file,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Profile::default()),
            Err(e) => return Err(e.into()),
        };
        let data: serde_json::Value = serde_json::from_reader(BufReader::new(file)).context("Invalid saved profile")?;
        let legacy = data.get("storage_version").is_none();
        let mut profile: Profile = serde_json::from_value(data).context("Invalid saved profile")?;
        for (id, item) in profile.items.iter().chain(&profile.pending_deletions) {
            ensure!(
                id.len() == 64 && id.bytes().all(|b| b.is_ascii_hexdigit()),
                "Invalid media ID"
            );
            ensure!(supported(&item.ext), "Invalid library format");
        }
        ensure!(
            profile.screens.values().all(|c| profile.items.contains_key(&c.id)),
            "Missing library reference"
        );
        for (id, item) in &mut profile.items {
            if item.motion.is_none() && matches!(item.ext.as_str(), "gif" | "webp") {
                item.motion = image_motion(&self.media(id, item), &item.ext).ok().flatten();
            }
        }
        ensure!(
            profile.screens.values().all(|choice| choice.frame.valid()),
            "Invalid wallpaper adjustments"
        );
        // Older builds applied every all-monitor still through the wallpaper portal.
        if legacy
            && profile.visible()
            && profile.system_wallpaper.is_none()
            && let Some(choice) = profile.screens.get(ALL)
            && let Some(item) = profile.items.get(&choice.id)
            && matches!(item.ext.as_str(), "png" | "jpg" | "jpeg" | "webp")
        {
            profile.system_wallpaper =
                crate::desktop::background::SystemWallpaper::identify_legacy(&self.media(&choice.id, item), choice);
        }
        Ok(profile)
    }
    pub fn save(&self, profile: &Profile) -> Result<()> {
        // Publish the Shell's recovery image in the same atomic write as its assignment.
        let mut saved = serde_json::to_value(profile)?;
        saved["storage_version"] = 2.into();
        saved["wallpaper_stills"] = serde_json::json!({
            "version": 1, "panel":profile.panel(), "screens": self.screen_paths(profile, true)
        });
        write_atomic(&self.profile_path(), &serde_json::to_vec_pretty(&saved)?)
    }

    /// The profile commit precedes unlinking. An interrupted deletion is
    /// retried at startup, and a subsequent import always owns its files.
    pub fn finish_deletions(&self, profile: &mut Profile) -> Result<()> {
        if profile.pending_deletions.is_empty() {
            return Ok(());
        }
        for (id, removed) in &profile.pending_deletions {
            let current = profile.items.get(id);
            let paths = [
                current.is_none().then(|| self.thumb(id)),
                current
                    .is_none_or(|item| item.ext != removed.ext)
                    .then(|| self.media(id, removed)),
            ];
            for path in paths.into_iter().flatten() {
                match fs::remove_file(&path) {
                    Ok(()) => {}
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                    Err(e) => return Err(e).with_context(|| format!("Remove {}", path.display())),
                }
            }
        }
        let pending = std::mem::take(&mut profile.pending_deletions);
        if let Err(e) = self.save(profile) {
            profile.pending_deletions = pending;
            return Err(e);
        }
        Ok(())
    }
    pub fn screen_paths(&self, p: &Profile, stills: bool) -> serde_json::Value {
        let mut screens = serde_json::Map::new();
        if !p.visible() {
            return screens.into();
        }
        for (screen, choice) in &p.screens {
            let item = &p.items[&choice.id];
            let poster = self.thumb(&choice.id);
            let path = if stills && item.animated() {
                poster.clone()
            } else {
                self.media(&choice.id, item)
            };
            let dimensions = gtk::gdk_pixbuf::Pixbuf::file_info(&poster).map(|(_, w, h)| [w, h]);
            screens.insert(
                screen.clone(),
                serde_json::json!({
                    "path":path, "poster":poster, "fit":choice.fit, "frame":choice.frame,"dimensions":dimensions, "animated":!stills && item.animated()
                }),
            );
        }
        screens.into()
    }
    pub fn import(
        &self,
        path: &Path,
        progress: impl Fn(u64, u64),
        preview: impl FnOnce(&Path, &Item, &Path) -> Result<()>,
    ) -> Result<(String, Item)> {
        let ext = path.extension().and_then(|v| v.to_str()).unwrap_or("").to_lowercase();
        ensure!(supported(&ext), "Unsupported media format");
        let ext = if ext == "jpeg" { String::from("jpg") } else { ext };
        let mut item = Item {
            motion: None,
            name: path.file_name().unwrap_or_default().to_string_lossy().into(),
            ext,
        };
        let mut src = File::open(path)?;
        let meta = src.metadata()?;
        ensure!(meta.is_file() && meta.len() > 0, "Expected a non-empty regular file");
        let mut copy = NamedTempFile::new_in(self.0.join("media"))?;
        let mut digest = Sha256::new();
        let mut buf = vec![0; 256 * 1024];
        let mut done = 0;
        loop {
            let n = src.read(&mut buf)?;
            if n == 0 {
                break;
            }
            copy.write_all(&buf[..n])?;
            digest.update(&buf[..n]);
            done += n as u64;
            progress(done, meta.len());
        }
        let id = format!("{:x}", digest.finalize());
        item.motion = image_motion(copy.path(), &item.ext)?;
        let media = self.media(&id, &item);
        let thumb = self.thumb(&id);
        if !media.is_file() || !thumb.is_file() {
            let thumbnail = NamedTempFile::new_in(self.0.join("thumbs"))?;
            preview(copy.path(), &item, thumbnail.path())?;
            thumbnail.as_file().sync_all()?;
            copy.as_file().sync_all()?;
            copy.persist(&media)?;
            thumbnail.persist(thumb)?;
            File::open(self.0.join("media"))?.sync_all()?;
            File::open(self.0.join("thumbs"))?.sync_all()?;
        }
        Ok((id, item))
    }
    pub fn log(&self, message: &str) {
        let log = self.0.join("ubrus.log");
        let time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let message: String = message
            .chars()
            .take(2048)
            .map(|c| if matches!(c, '\n' | '\r') { ' ' } else { c })
            .collect();
        let record = format!("{time} {message}\n");
        if log.metadata().is_ok_and(|m| m.len() + record.len() as u64 > 512 * 1024)
            && fs::rename(&log, self.0.join("ubrus.log.1")).is_err()
        {
            return;
        }
        if let Ok(mut f) = OpenOptions::new().append(true).create(true).open(log) {
            let _ = f.write_all(record.as_bytes());
        }
    }
    pub fn report(&self, status: &str) -> String {
        let tail = ["ubrus.log.1", "ubrus.log"].map(|n| fs::read_to_string(self.0.join(n)).unwrap_or_default());
        format!(
            "Ubrus {}\n{}\n\n{}{}",
            env!("CARGO_PKG_VERSION"),
            status,
            tail[0],
            tail[1]
        )
    }
}
fn image_motion(path: &Path, ext: &str) -> Result<Option<bool>> {
    use gtk::prelude::PixbufAnimationExt;
    if matches!(ext, "gif" | "webp") {
        return Ok(Some(
            !gtk::gdk_pixbuf::PixbufAnimation::from_file(path)?.is_static_image(),
        ));
    }
    Ok(None)
}
fn supported(ext: &str) -> bool {
    matches!(ext, "png" | "jpg" | "jpeg" | "webp" | "gif" | "mp4" | "webm")
}

#[cfg(test)]
#[path = "../../tests/rust/store.rs"]
mod tests;
