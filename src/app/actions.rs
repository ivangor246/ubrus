use super::{Action, Event, control::Control, diagnostics};
use crate::{
    desktop::{background::SystemWallpaper, portal},
    library::ALL,
    media,
    preferences::Text,
};
use anyhow::{Context, Result, anyhow, ensure};
use ashpd::WindowIdentifier;
use async_channel::Sender;
use gtk::{gio, glib, prelude::*};
use std::cell::Cell;

type Outcome = (Option<Text>, Option<String>);
pub(super) fn finish(c: &Control, events: &Sender<Event>, result: Result<Outcome>, fallback: Text) {
    let (feedback, selected) = result.unwrap_or_else(|e| {
        if matches!(
            e.downcast_ref::<ashpd::Error>(),
            Some(ashpd::Error::Response(ashpd::desktop::ResponseError::Cancelled))
        ) {
            return ((fallback == Text::FailedApply).then_some(Text::Declined), None);
        }
        let technical = format!("{e:#}");
        c.store.log(&technical);
        let message = match technical.as_str() {
            "Unsupported media format" => Text::Unsupported,
            "Expected a non-empty regular file" => Text::EmptyFile,
            "Library file is missing" => Text::Missing,
            "Extension required" => Text::Extension,
            "Selected monitor disconnected" => Text::Disconnected,
            _ if e.downcast_ref::<std::io::Error>().and_then(|e| e.raw_os_error()) == Some(28) => Text::DiskFull,
            _ => fallback,
        };
        (Some(message), None)
    });
    let _ = events.try_send(Event::Done {
        profile: c.profile.borrow().clone(),
        feedback,
        selected,
    });
}

pub(super) async fn perform(
    c: &Control,
    events: &Sender<Event>,
    action: Action,
    parent: Option<WindowIdentifier>,
    size: (i32, i32),
) -> Result<Outcome> {
    let locale = c.profile.borrow().language.locale();
    match action {
        Action::Import => {
            let path = portal::choose(parent, locale, false).await?;
            let store = c.store.clone();
            let progress = events.clone();
            let (id, item) = gio::spawn_blocking(move || {
                gst::init()?;
                let last = Cell::new(None);
                store.import(
                    &path,
                    |done, total| {
                        let percent = (done.saturating_mul(100) / total).min(100) as u8;
                        if last.replace(Some(percent)) != Some(percent) {
                            let _ = progress.try_send(Event::Progress(percent));
                        }
                    },
                    media::thumbnail,
                )
            })
            .await
            .map_err(|_| anyhow!("Import worker failed"))??;
            c.save(|p| {
                p.items.entry(id.clone()).or_insert(item);
                Ok(())
            })?;
            Ok((Some(Text::Imported), Some(id)))
        }
        Action::Apply(screen, choice) => {
            let item = c
                .profile
                .borrow()
                .items
                .get(&choice.id)
                .cloned()
                .context("Unknown library item")?;
            let path = c.store.media(&choice.id, &item);
            ensure!(path.is_file(), "Library file is missing");
            if screen == ALL && !item.animated() {
                let previous = c.profile.borrow().system_wallpaper.clone();
                let library = c.store.clone();
                let (tmp, background) = gio::spawn_blocking(move || -> Result<_> {
                    let tmp = portal::system_picture(&path, choice.fit, size.0, size.1)?;
                    let background = SystemWallpaper::capture(previous.as_ref(), tmp.path(), library.root())?;
                    Ok((tmp, background))
                })
                .await
                .map_err(|_| anyhow!("Image worker failed"))??;
                glib::future_with_timeout(
                    std::time::Duration::from_secs(180),
                    portal::wallpaper(parent, tmp.path()),
                )
                .await??;
                c.commit(|p| {
                    p.system_wallpaper = Some(background);
                    p.assign(&screen, choice)
                })?;
            } else {
                ensure!(c.status.borrow().connected, "Extension required");
                ensure!(
                    screen == ALL || c.status.borrow().outputs.iter().any(|m| m.id == screen),
                    "Selected monitor disconnected"
                );
                if !item.animated() {
                    gio::spawn_blocking(move || {
                        gtk::gdk_pixbuf::Pixbuf::from_file_at_scale(path, 32, 32, true).map(|_| ())
                    })
                    .await
                    .map_err(|_| anyhow!("Image worker failed"))??;
                }
                c.apply(screen, choice).await?;
            }
            c.store.log("Wallpaper selection changed");
            Ok((Some(Text::Applied), None))
        }
        Action::Delete(id) => {
            let background = {
                let p = c.profile.borrow();
                ensure!(p.items.contains_key(&id), "Unknown library item");
                p.system_wallpaper.clone().filter(|_| p.restores_desktop_on_delete(&id))
            };
            if let Some(background) = background {
                background.restore(parent).await?;
            }
            c.commit(|p| p.delete_item(&id))?;
            let feedback = match c.store.finish_deletions(&mut c.profile.borrow_mut()) {
                Ok(()) => Text::Deleted,
                Err(e) => {
                    c.store.log(&format!("Library cleanup: {e:#}"));
                    Text::DeleteCleanup
                }
            };
            Ok((Some(feedback), None))
        }
        Action::ClearWallpapers => {
            let background = c.profile.borrow().system_wallpaper.clone();
            if let Some(background) = background {
                background.restore(parent).await?;
            }
            c.commit(|p| {
                p.clear_wallpapers();
                Ok(())
            })?;
            c.store.log("Wallpaper assignments cleared");
            Ok((Some(Text::Cleared), None))
        }
        Action::Export | Action::ExportAs => {
            let destination = if matches!(action, Action::ExportAs) {
                Some(
                    glib::future_with_timeout(
                        std::time::Duration::from_secs(180),
                        portal::choose(parent, locale, true),
                    )
                    .await??,
                )
            } else {
                None
            };
            let store = c.store.clone();
            let profile = c.profile.borrow().clone();
            let status = c.status.borrow().clone();
            let path = gio::spawn_blocking(move || -> Result<_> {
                let summary = diagnostics::settings(locale, &profile, &status);
                let report = format!(
                    "{}\n\n{}",
                    store.report(&summary),
                    serde_json::to_string_pretty(&status)?
                );
                if let Some(path) = destination {
                    gio::File::for_path(&path).replace_contents(
                        report.as_bytes(),
                        None,
                        false,
                        gio::FileCreateFlags::PRIVATE,
                        gio::Cancellable::NONE,
                    )?;
                    Ok(path)
                } else {
                    let downloads = glib::user_special_dir(glib::UserDirectory::Downloads);
                    diagnostics::save_report(&report, downloads.as_deref(), &store.root().join("reports"))
                }
            })
            .await
            .map_err(|_| anyhow!("Report worker failed"))??;
            let _ = events.try_send(Event::ReportSaved(path));
            Ok((Some(Text::Exported), None))
        }
    }
}
