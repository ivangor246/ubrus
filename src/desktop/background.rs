use anyhow::{Context, Result, ensure};
use gtk::{gio, glib, prelude::*};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File},
    path::{Path, PathBuf},
};

const SCHEMA: &str = "org.gnome.desktop.background";
const LIGHT: &str = "picture-uri";
const DARK: &str = "picture-uri-dark";
const STYLE: &str = "picture-options";

/// The portal overwrites one shared file. Keep the original GNOME settings
/// and identify our copy before restoring them.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct SystemWallpaper {
    light: Option<String>,
    dark: Option<String>,
    style: Option<String>,
    digest: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    transaction: Option<String>,
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

    /// Record how to undo this specific installation separately from the user's original wallpaper.
    pub fn prepare_install(previous: Option<&Self>, picture: &Path, library: &Path) -> Result<Self> {
        let (mut installed, mut before) = if std::env::var_os("SNAP").is_some() {
            (Self::default(), Self::default())
        } else {
            let settings = gio::Settings::new(SCHEMA);
            let cache = glib::user_config_dir().join("background");
            let before = Self::capture_native(&settings, &cache, None, library)?;
            let installed = if previous.is_some() {
                Self::capture_native(&settings, &cache, previous, library)?
            } else {
                before.clone()
            };
            (installed, before)
        };
        let transaction = glib::uuid_string_random().to_string();
        installed.transaction = Some(transaction.clone());
        installed.digest = Some(checksum(picture)?);
        before.digest.clone_from(&installed.digest);
        WallpaperChange::Install { transaction, before }.write(library)?;
        Ok(installed)
    }

    fn capture_native(settings: &gio::Settings, cache: &Path, previous: Option<&Self>, library: &Path) -> Result<Self> {
        let uri = gio::File::for_path(cache).uri();
        let light = settings.string(LIGHT) == uri;
        let dark = settings.string(DARK) == uri;
        let mut captured = Self::capture_settings(settings, &uri, previous);
        // After an interrupted replacement the previous picture may be displayed from
        // its backup URI. Keep the original settings that preceded that installation.
        if let Some(prior) = previous
            && let Some(backup) = prior.backup_uri(library)
        {
            let alias_light = settings.string(LIGHT) == backup;
            let alias_dark = settings.string(DARK) == backup;
            if alias_light {
                captured.light.clone_from(&prior.light);
            }
            if alias_dark {
                captured.dark.clone_from(&prior.dark);
            }
            if (alias_light || alias_dark) && settings.string(STYLE) == "zoom" {
                captured.style.clone_from(&prior.style);
            }
        }
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
            File::open(&backups)?.sync_all()?;
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
            ..Self::default()
        }
    }

    fn backup_uri(&self, library: &Path) -> Option<String> {
        let digest = self.digest.as_ref()?;
        if !backup_name(digest) {
            return None;
        }
        Some(
            gio::File::for_path(library.join("backgrounds").join(digest))
                .uri()
                .to_string(),
        )
    }

    fn restore_native(&self, settings: &gio::Settings, cache: &Path, library: &Path) -> Result<()> {
        let cache_uri = gio::File::for_path(cache).uri();
        let alias = self.backup_uri(library);
        let owns = |uri: &str| -> Result<bool> {
            if uri != cache_uri.as_str() && Some(uri) != alias.as_deref() {
                return Ok(false);
            }
            let Some(expected) = self.digest.as_ref() else {
                return Ok(false);
            };
            let path = gio::File::for_uri(uri)
                .path()
                .context("Expected a local wallpaper URI")?;
            Ok(!path.try_exists()? || &checksum(&path)? == expected)
        };
        let light_uri = settings.string(LIGHT);
        let dark_uri = settings.string(DARK);
        let light = owns(&light_uri)?;
        let dark = if dark_uri == light_uri { light } else { owns(&dark_uri)? };
        if !light && !dark {
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
}

#[derive(Serialize, Deserialize)]
enum WallpaperChange {
    Install {
        transaction: String,
        before: SystemWallpaper,
    },
    Remove {
        previous: SystemWallpaper,
    },
}

const JOURNAL: &str = "wallpaper-change.json";

pub fn pending(library: &Path) -> std::io::Result<bool> {
    library.join(JOURNAL).try_exists()
}

impl WallpaperChange {
    fn write(&self, library: &Path) -> Result<()> {
        crate::files::write_atomic(&library.join(JOURNAL), &serde_json::to_vec(self)?)
    }

    fn restore_for(self, saved: Option<&SystemWallpaper>) -> Option<SystemWallpaper> {
        match self {
            Self::Install { transaction, before } => {
                (saved.and_then(|s| s.transaction.as_ref()) != Some(&transaction)).then_some(before)
            }
            Self::Remove { previous } => (saved != Some(&previous)).then_some(previous),
        }
    }
}

pub fn prepare_removal(library: &Path, previous: &SystemWallpaper) -> Result<()> {
    WallpaperChange::Remove {
        previous: previous.clone(),
    }
    .write(library)
}

fn backup_name(name: &str) -> bool {
    name.len() == 64 && name.bytes().all(|b| b.is_ascii_hexdigit())
}

fn cleanup_backups(library: &Path, saved: Option<&SystemWallpaper>, settings: &gio::Settings) -> Result<()> {
    let dir = library.join("backgrounds");
    let mut keep = std::collections::HashSet::<PathBuf>::new();
    let mut keep_uri = |uri: &str| {
        if let Some(path) = gio::File::for_uri(uri).path() {
            keep.insert(path);
        }
    };
    for key in [LIGHT, DARK] {
        keep_uri(&settings.string(key));
    }
    if let Some(saved) = saved {
        for uri in [&saved.light, &saved.dark].into_iter().flatten() {
            keep_uri(uri);
        }
    }
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        if entry.file_type()?.is_file()
            && entry.file_name().to_str().is_some_and(backup_name)
            && !keep.contains(&entry.path())
        {
            fs::remove_file(entry.path())?;
        }
    }
    Ok(())
}

/// Resolve an interrupted portal call against the atomic profile. Finish its rollback
/// before collecting backups; the journal may be their only remaining reference.
pub fn recover(library: &Path, saved: Option<&SystemWallpaper>) -> Result<bool> {
    if !pending(library)? && !library.join("backgrounds").is_dir() {
        return Ok(true);
    }
    let settings = std::env::var_os("SNAP").is_none().then(|| gio::Settings::new(SCHEMA));
    recover_with_settings(
        library,
        saved,
        settings.as_ref(),
        &glib::user_config_dir().join("background"),
    )
}

fn recover_with_settings(
    library: &Path,
    saved: Option<&SystemWallpaper>,
    settings: Option<&gio::Settings>,
    cache: &Path,
) -> Result<bool> {
    let path = library.join(JOURNAL);
    let mut restored = true;
    if path.try_exists()? {
        let change: WallpaperChange = serde_json::from_reader(File::open(&path)?)?;
        if let Some(before) = change.restore_for(saved) {
            if let Some(settings) = settings {
                before.restore_native(settings, cache, library)?;
            } else {
                restored = false;
            }
        }
        fs::remove_file(&path)?;
        File::open(library)?.sync_all()?;
    }
    if let Some(settings) = settings
        && library.join("backgrounds").is_dir()
    {
        cleanup_backups(library, saved, settings)?;
    }
    Ok(restored)
}

#[cfg(test)]
#[path = "../../tests/rust/background.rs"]
mod tests;
