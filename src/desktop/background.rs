use anyhow::{Result, ensure};
use gtk::{gio, glib, prelude::*};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File},
    path::Path,
};

const SCHEMA: &str = "org.gnome.desktop.background";
const LIGHT: &str = "picture-uri";
const DARK: &str = "picture-uri-dark";
const STYLE: &str = "picture-options";

/// The portal overwrites one shared file. Keep the original GNOME settings
/// and identify our copy before restoring them.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct SystemWallpaper {
    light: Option<String>,
    dark: Option<String>,
    style: Option<String>,
    digest: Option<String>,
}

fn checksum(path: &Path) -> Result<String> {
    let mut hash = Sha256::new();
    std::io::copy(&mut File::open(path)?, &mut hash)?;
    Ok(format!("{:x}", hash.finalize()))
}

impl SystemWallpaper {
    pub fn identify_legacy(source: &Path, choice: &crate::library::Choice) -> Option<Self> {
        if std::env::var_os("SNAP").is_some() {
            return None;
        }
        let cache = glib::user_config_dir().join("background");
        let (_, width, height) = gtk::gdk_pixbuf::Pixbuf::file_info(&cache)?;
        let candidate = super::portal::system_picture(source, choice, width, height).ok()?;
        let digest = checksum(&cache).ok()?;
        (checksum(candidate.path()).ok()? == digest).then_some(Self {
            digest: Some(digest),
            ..Self::default()
        })
    }

    pub fn capture(previous: Option<&Self>, picture: &Path, library: &Path) -> Result<Self> {
        if std::env::var_os("SNAP").is_some() {
            return Ok(Self::default());
        }
        let settings = gio::Settings::new(SCHEMA);
        let cache = glib::user_config_dir().join("background");
        let mut snapshot = Self::capture_native(&settings, &cache, previous, library)?;
        snapshot.digest = Some(checksum(picture)?);
        Ok(snapshot)
    }

    fn capture_native(settings: &gio::Settings, cache: &Path, previous: Option<&Self>, library: &Path) -> Result<Self> {
        let uri = gio::File::for_path(cache).uri();
        let light = settings.string(LIGHT) == uri;
        let dark = settings.string(DARK) == uri;
        let mut captured = Self::capture_settings(settings, &uri, previous);
        if (!light && !dark) || !cache.try_exists()? {
            return Ok(captured);
        }
        let digest = checksum(cache)?;
        if previous.is_some_and(|p| p.digest.as_ref() == Some(&digest)) {
            return Ok(captured);
        }
        // Other portal clients use the same filename. Preserve its contents before GNOME replaces it.
        let backups = library.join("backgrounds");
        fs::create_dir_all(&backups)?;
        let path = backups.join(digest);
        if !path.try_exists()? {
            let mut copy = tempfile::NamedTempFile::new_in(&backups)?;
            std::io::copy(&mut File::open(cache)?, &mut copy)?;
            copy.as_file().sync_all()?;
            copy.persist(&path)?;
        }
        let backup = gio::File::for_path(path).uri().to_string();
        if light {
            captured.light = Some(backup.clone());
        }
        if dark {
            captured.dark = Some(backup);
        }
        captured.style = settings.user_value(STYLE).and_then(|v| v.get::<String>());
        Ok(captured)
    }

    fn capture_settings(settings: &gio::Settings, cache_uri: &str, previous: Option<&Self>) -> Self {
        let user = |key| settings.user_value(key).and_then(|v| v.get::<String>());
        let cached = |key| settings.string(key) == cache_uri;
        let prior = previous.cloned().unwrap_or_default();
        Self {
            light: if cached(LIGHT) { prior.light } else { user(LIGHT) },
            dark: if cached(DARK) { prior.dark } else { user(DARK) },
            style: if (cached(LIGHT) || cached(DARK)) && settings.string(STYLE) == "zoom" {
                prior.style
            } else {
                user(STYLE)
            },
            digest: None,
        }
    }

    fn restore_native(&self, settings: &gio::Settings, cache: &Path) -> Result<()> {
        let uri = gio::File::for_path(cache).uri();
        let light = settings.string(LIGHT) == uri;
        let dark = settings.string(DARK) == uri;
        if !light && !dark {
            return Ok(());
        }
        if self.digest.is_none() || (cache.try_exists()? && self.digest.as_ref() != Some(&checksum(cache)?)) {
            return Ok(());
        }
        let unchanged = |key, value: &Option<String>| {
            settings.user_value(key).and_then(|v| v.get::<String>()).as_ref() == value.as_ref()
        };
        let restore_style = (light || unchanged(LIGHT, &self.light))
            && (dark || unchanged(DARK, &self.dark))
            && settings.string(STYLE) == "zoom";
        settings.delay();
        let restore = |key, value: &Option<String>| -> Result<()> {
            ensure!(settings.is_writable(key), "Desktop setting {key} is locked");
            if let Some(value) = value {
                settings.set_string(key, value)?;
            } else {
                settings.reset(key);
            }
            Ok(())
        };
        if light {
            restore(LIGHT, &self.light)?;
        }
        if dark {
            restore(DARK, &self.dark)?;
        }
        if restore_style {
            restore(STYLE, &self.style)?;
        }
        settings.apply();
        gio::Settings::sync();
        Ok(())
    }

    pub async fn restore(self) -> Result<bool> {
        if std::env::var_os("SNAP").is_none() {
            return gio::spawn_blocking(move || {
                self.restore_native(&gio::Settings::new(SCHEMA), &glib::user_config_dir().join("background"))
                    .map(|()| true)
            })
            .await
            .map_err(|_| anyhow::anyhow!("Desktop settings worker failed"))?;
        }
        // A confined portal client cannot prove ownership of the host's shared image.
        // Keep the host background intact when restoring an older portal assignment.
        Ok(false)
    }
}

#[cfg(test)]
#[path = "../../tests/rust/background.rs"]
mod tests;
