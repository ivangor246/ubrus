use super::{Item, Profile, Store, supported};
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fs::{self, File},
    io::{ErrorKind, Read, Write},
    path::Path,
};
use tempfile::{Builder, NamedTempFile};

const TEMP_PREFIX: &str = ".ubrus-import-";
const JOURNAL: &str = "import-change.json";

#[derive(Serialize, Deserialize)]
struct Publication {
    id: String,
    ext: String,
    new_media: bool,
    new_thumbnail: bool,
}

pub struct PreparedImport {
    id: String,
    item: Item,
    media: NamedTempFile,
    thumbnail: NamedTempFile,
    synced: bool,
}

impl Store {
    /// Keep files private to this operation until the controller can commit against the current profile.
    pub fn prepare_import(
        &self,
        path: &Path,
        known: &BTreeMap<String, Item>,
        progress: impl Fn(u64, u64) -> Result<()>,
        preview: impl FnOnce(&Path, &Item, &Path) -> Result<Option<bool>>,
    ) -> Result<PreparedImport> {
        let ext = path.extension().and_then(|v| v.to_str()).unwrap_or("").to_lowercase();
        ensure!(supported(&ext), "Unsupported media format");
        let mut input = File::open(path)?;
        let meta = input.metadata()?;
        ensure!(meta.is_file() && meta.len() > 0, "Expected a non-empty regular file");
        let mut media = Builder::new()
            .prefix(TEMP_PREFIX)
            .tempfile_in(self.root().join("media"))?;
        let mut digest = Sha256::new();
        let mut buf = vec![0; 256 * 1024];
        let mut done = 0;
        progress(done, meta.len())?;
        loop {
            let n = input.read(&mut buf)?;
            if n == 0 {
                break;
            }
            media.write_all(&buf[..n])?;
            digest.update(&buf[..n]);
            done += n as u64;
            progress(done, meta.len())?;
        }
        let id = format!("{:x}", digest.finalize());
        let mut item = known.get(&id).cloned().unwrap_or_else(|| Item {
            ext: if ext == "jpeg" { "jpg".into() } else { ext },
            name: path.file_name().unwrap_or_default().to_string_lossy().into_owned(),
            motion: None,
        });
        let mut thumbnail = Builder::new()
            .prefix(TEMP_PREFIX)
            .tempfile_in(self.root().join("thumbs"))?;
        // Keep a staged poster even for a duplicate: a concurrent delete may have removed
        // the original by the time the main context accepts this import.
        let existing = known.get(&id).and_then(|_| File::open(self.thumb(&id)).ok());
        if let Some(mut existing) = existing {
            std::io::copy(&mut existing, &mut thumbnail)?;
        } else {
            item.motion = preview(media.path(), &item, thumbnail.path())?;
        }
        progress(done, meta.len())?;
        let synced = !known.contains_key(&id) || !self.media(&id, &item).is_file() || !self.thumb(&id).is_file();
        if synced {
            media.as_file().sync_all()?;
            thumbnail.as_file().sync_all()?;
        }
        progress(done, meta.len())?;
        Ok(PreparedImport {
            id,
            item,
            media,
            thumbnail,
            synced,
        })
    }

    pub fn commit_import(&self, profile: &mut Profile, prepared: PreparedImport) -> Result<String> {
        self.recover_publication(profile)?;
        let PreparedImport {
            id,
            item,
            media,
            thumbnail,
            synced,
        } = prepared;
        if let Some(known) = profile.items.get(&id)
            && self.media(&id, known).is_file()
            && self.thumb(&id).is_file()
        {
            return Ok(id);
        }
        let mut next = profile.clone();
        let entry = next.items.entry(id.clone()).or_insert(item);
        let media_path = self.media(&id, entry);
        let thumb_path = self.thumb(&id);
        let publication = Publication {
            id: id.clone(),
            ext: entry.ext.clone(),
            new_media: !media_path.try_exists()?,
            new_thumbnail: !thumb_path.try_exists()?,
        };
        crate::files::write_atomic(&self.root().join(JOURNAL), &serde_json::to_vec(&publication)?)?;
        let publish = || -> Result<()> {
            for (file, path) in [(media, media_path), (thumbnail, thumb_path)] {
                match file.persist_noclobber(&path) {
                    Ok(file) => {
                        if !synced {
                            file.sync_all()?;
                        }
                        File::open(path.parent().unwrap())?.sync_all()?;
                    }
                    Err(e) if e.error.kind() == ErrorKind::AlreadyExists && path.is_file() => {}
                    Err(e) => return Err(e.error.into()),
                }
            }
            self.save(&next)
        };
        let result = publish();
        if result.is_ok() {
            *profile = next;
        }
        // If cleanup fails, the recorded intent stays available for the next import or startup.
        if let Err(error) = self.recover_publication(profile) {
            self.log(&format!("Import cleanup: {error:#}"));
        }
        result.map(|()| id)
    }

    fn recover_publication(&self, profile: &Profile) -> Result<()> {
        let journal = self.root().join(JOURNAL);
        let file = match File::open(&journal) {
            Ok(file) => file,
            Err(e) if e.kind() == ErrorKind::NotFound => return Ok(()),
            Err(e) => return Err(e.into()),
        };
        let published: Publication = serde_json::from_reader(file)?;
        let id = &published.id;
        ensure!(
            id.len() == 64 && id.bytes().all(|b| b.is_ascii_hexdigit()) && supported(&published.ext),
            "Invalid import journal"
        );
        let current = profile.items.get(id);
        let remove = |path: &Path| -> Result<()> {
            match fs::remove_file(path) {
                Ok(()) => File::open(path.parent().unwrap())?.sync_all()?,
                Err(e) if e.kind() == ErrorKind::NotFound => {}
                Err(e) => return Err(e).with_context(|| format!("Remove {}", path.display())),
            }
            Ok(())
        };
        if published.new_media && current.is_none_or(|item| item.ext != published.ext) {
            remove(&self.root().join("media").join(format!("{id}.{}", published.ext)))?;
        }
        if published.new_thumbnail && current.is_none() {
            remove(&self.thumb(id))?;
        }
        fs::remove_file(journal)?;
        File::open(self.root())?.sync_all()?;
        Ok(())
    }

    /// Never infer media ownership from a missing or older profile. Only an interrupted
    /// publication records which final paths were absent before this operation.
    pub fn cleanup_imports(&self, profile: &Profile) -> Result<()> {
        self.recover_publication(profile)?;
        for (directory, prefix) in [("", ".ubrus-profile-"), ("media", TEMP_PREFIX), ("thumbs", TEMP_PREFIX)] {
            for entry in fs::read_dir(self.root().join(directory))? {
                let entry = entry?;
                if entry.file_name().to_str().is_some_and(|name| name.starts_with(prefix))
                    && entry.file_type()?.is_file()
                {
                    fs::remove_file(entry.path())?;
                }
            }
        }
        Ok(())
    }

    #[cfg(test)]
    pub fn import(
        &self,
        path: &Path,
        progress: impl Fn(u64, u64),
        preview: impl FnOnce(&Path, &Item, &Path) -> Result<Option<bool>>,
    ) -> Result<(String, Item)> {
        let mut profile = self.load()?;
        let prepared = self.prepare_import(
            path,
            &profile.items,
            |done, total| {
                progress(done, total);
                Ok(())
            },
            preview,
        )?;
        let id = self.commit_import(&mut profile, prepared)?;
        Ok((id.clone(), profile.items.remove(&id).unwrap()))
    }
}
