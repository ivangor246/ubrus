use super::{Event, Feedback, control::Control};
use crate::{desktop::portal, media, preferences::Text};
use anyhow::{Context, Result, ensure};
use ashpd::WindowIdentifier;
use async_channel::Sender;
use gtk::{gio, prelude::*};
use std::path::{Path, PathBuf};

type Outcome = (Option<Feedback>, Option<String>);

pub(super) async fn run(
    control: &Control,
    events: &Sender<Event>,
    parent: Option<WindowIdentifier>,
    cancel: &gio::Cancellable,
    changes: &iced::futures::lock::Mutex<()>,
) -> Result<Outcome> {
    ensure!(control.writable, "Repair the saved profile before importing");
    let locale = control.profile.borrow().language.locale();
    let paths = portal::choose_files(parent, locale, cancel).await?;
    let count = paths.len() as u64;
    import_paths(
        paths,
        cancel,
        async |index, path| {
            let store = control.store.clone();
            let known = control.profile.borrow().items.clone();
            let progress = events.clone();
            let stop = cancel.clone();
            let path = path.to_owned();
            let prepared = gio::spawn_blocking(move || {
                gst::init()?;
                let last = std::cell::Cell::new(None);
                store.prepare_import(
                    &path,
                    &known,
                    |done, total| {
                        ensure!(!stop.is_cancelled(), "Import cancelled");
                        let percent =
                            ((index as u64 * 100 + done.saturating_mul(100) / total.max(1)) / count).min(100) as u8;
                        if last.replace(Some(percent)) != Some(percent) {
                            let _ = progress.try_send(Event::Progress(percent));
                        }
                        Ok(())
                    },
                    media::thumbnail,
                )
            })
            .await
            .ok()
            .context("Import worker failed")?;
            let _guard = changes.lock().await;
            if cancel.is_cancelled() {
                return Ok(None);
            }
            control.import(prepared?).map(Some)
        },
        |path, err| control.store.log(&format!("Import {}: {err:#}", path.display())),
    )
    .await
}

async fn import_paths(
    paths: Vec<PathBuf>,
    cancel: &gio::Cancellable,
    mut import: impl AsyncFnMut(usize, &Path) -> Result<Option<String>>,
    log: impl Fn(&Path, &anyhow::Error),
) -> Result<Outcome> {
    let total = paths.len();
    let mut imported = 0;
    let mut failed = 0;
    let mut selected = None;
    let mut stopped = None;
    for (index, path) in paths.into_iter().enumerate() {
        if cancel.is_cancelled() {
            break;
        }
        match import(index, &path).await {
            Ok(Some(id)) => {
                selected = Some(id);
                imported += 1;
            }
            Ok(None) => break,
            Err(err) if total == 1 => return Err(err),
            Err(err) => {
                failed += 1;
                log(&path, &err);
                if err
                    .downcast_ref::<std::io::Error>()
                    .is_some_and(|e| e.kind() == std::io::ErrorKind::StorageFull)
                {
                    stopped = Some(Text::DiskFull);
                    break;
                }
            }
        }
    }
    if cancel.is_cancelled() {
        stopped = Some(Text::ImportStopped);
    }
    let feedback = if total > 1 {
        Some(Feedback::ImportBatch {
            imported,
            failed,
            skipped: total - imported - failed,
            stopped,
        })
    } else {
        stopped
            .or_else(|| selected.as_ref().map(|_| Text::Imported))
            .map(Feedback::Message)
    };
    Ok((feedback, selected))
}

#[cfg(test)]
#[path = "../../tests/rust/import.rs"]
mod tests;
