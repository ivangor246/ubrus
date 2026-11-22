use super::{Action, Event, Feedback, control::Control, diagnostics};
use crate::{
    desktop::{background::SystemWallpaper, portal},
    library::ALL,
    preferences::Text,
};
use anyhow::{Context, Result, anyhow, ensure};
use ashpd::WindowIdentifier;
use async_channel::Sender;
use gtk::{gio, glib, prelude::*};

type Outcome = (Option<Text>, Option<String>);
pub(super) fn finish(
    c: &Control,
    events: &Sender<Event>,
    result: Result<(Option<Feedback>, Option<String>)>,
    fallback: Text,
    from_panel: bool,
) {
    let (feedback, selected) = result.unwrap_or_else(|e| {
        if matches!(
            e.downcast_ref::<ashpd::Error>(),
            Some(ashpd::Error::Response(ashpd::desktop::ResponseError::Cancelled))
        ) {
            return (
                (fallback == Text::FailedApply).then_some(Feedback::Message(Text::Declined)),
                None,
            );
        }
        let technical = format!("{e:#}");
        c.store.log(&technical);
        let message = match technical.as_str() {
            "Unsupported media format" => Text::Unsupported,
            "Expected a non-empty regular file" => Text::EmptyFile,
            "Library file is missing" => Text::Missing,
            "Extension update required" => Text::ShellUpdate,
            "Extension required" => Text::Extension,
            "Selected monitor disconnected" => Text::Disconnected,
            _ if e.downcast_ref::<std::io::Error>().and_then(|e| e.raw_os_error()) == Some(28) => Text::DiskFull,
            _ => fallback,
        };
        (Some(Feedback::Message(message)), None)
    });
    let _ = events.try_send(Event::Done {
        from_panel,
        profile: c.profile.borrow().clone(),
        feedback,
        selected,
    });
}

fn use_portal(c: &Control) -> bool {
    std::env::var_os("SNAP").is_none() || !c.status.borrow().connected
}

async fn install_picture(
    c: &Control,
    path: std::path::PathBuf,
    choice: crate::library::Choice,
    next: &mut crate::library::Profile,
    parent: Option<WindowIdentifier>,
    size: (i32, i32),
) -> Result<tempfile::NamedTempFile> {
    let previous = c.profile.borrow().system_wallpaper.clone();
    let store = c.store.clone();
    let (tmp, background) = gio::spawn_blocking(move || -> Result<_> {
        let tmp = portal::system_picture(&path, &choice, size.0, size.1)?;
        let background = SystemWallpaper::prepare_install(previous.as_ref(), tmp.path(), store.root())?;
        Ok((tmp, background))
    })
    .await
    .map_err(|_| anyhow!("Image worker failed"))??;
    next.system_wallpaper = Some(background);
    // Reserve and flush the complete candidate before the portal changes the shared host image.
    let staged = c.store.stage_profile(next)?;
    glib::future_with_timeout(
        std::time::Duration::from_secs(180),
        portal::wallpaper(parent, tmp.path()),
    )
    .await??;
    Ok(staged)
}

pub(super) async fn perform(
    c: &Control,
    events: &Sender<Event>,
    action: Action,
    parent: Option<WindowIdentifier>,
    size: (i32, i32),
) -> Result<Outcome> {
    let changes_wallpaper = matches!(
        action,
        Action::Apply(..) | Action::Visible(_) | Action::Enabled(_) | Action::Delete(_)
    );
    ensure!(!changes_wallpaper || c.writable, "Read-only saved profile");
    if changes_wallpaper && crate::desktop::background::pending(c.store.root())? {
        c.finish_wallpaper_change().await?;
    }
    let result = perform_action(c, events, action, parent, size).await;
    if changes_wallpaper && crate::desktop::background::pending(c.store.root())? {
        let restored = c.finish_wallpaper_change().await?;
        if !restored {
            return result.map(|(feedback, selected)| {
                (
                    Some(if feedback == Some(Text::DeleteCleanup) {
                        Text::DeleteCleanup
                    } else {
                        Text::OwnershipUnknown
                    }),
                    selected,
                )
            });
        }
    }
    result
}

async fn perform_action(
    c: &Control,
    events: &Sender<Event>,
    action: Action,
    parent: Option<WindowIdentifier>,
    size: (i32, i32),
) -> Result<Outcome> {
    let locale = c.profile.borrow().language.locale();
    match action {
        Action::Import => unreachable!("Imports are dispatched separately"),
        Action::Apply(screen, choice) => {
            ensure!(!c.profile.borrow().disabled, "Ubrus is disabled");
            let item = c
                .profile
                .borrow()
                .items
                .get(&choice.id)
                .cloned()
                .context("Unknown library item")?;
            let path = c.store.media(&choice.id, &item);
            ensure!(path.is_file(), "Library file is missing");
            if screen == ALL && !item.animated() && use_portal(c) {
                ensure!(
                    !c.status.borrow().connected || c.status.borrow().framing || choice.frame == Default::default(),
                    "Extension update required"
                );
                let mut next = c.profile.borrow().clone();
                next.assign(&screen, choice.clone())?;
                let staged = install_picture(c, path, choice, &mut next, parent, size).await?;
                c.commit_staged(next, staged)?;
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
                crate::desktop::background::prepare_removal(c.store.root(), &background)?;
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
        Action::Visible(_) | Action::Enabled(_) => {
            let mut next = c.profile.borrow().clone();
            let was_visible = next.visible();
            match action {
                Action::Enabled(value) => {
                    next.disabled = !value;
                }
                Action::Visible(value) => next.hidden = !value,
                _ => unreachable!(),
            }
            let mut confirm = false;
            let mut staged = None;
            if was_visible && !next.visible() {
                if let Some(background) = next.system_wallpaper.take() {
                    crate::desktop::background::prepare_removal(c.store.root(), &background)?;
                }
            } else if !was_visible && next.visible() {
                for choice in next.screens.values() {
                    ensure!(
                        c.store.media(&choice.id, &next.items[&choice.id]).is_file(),
                        "Library file is missing"
                    );
                }
                let portal_choice = next
                    .screens
                    .get(ALL)
                    .filter(|choice| !next.items[&choice.id].animated() && use_portal(c));
                confirm = next.screens.len() > usize::from(portal_choice.is_some());
                ensure!(!confirm || c.status.borrow().connected, "Extension required");
                ensure!(
                    !c.status.borrow().connected
                        || c.status.borrow().framing
                        || next.screens.values().all(|c| c.frame == Default::default()),
                    "Extension update required"
                );
                if let Some(choice) = portal_choice {
                    let path = c.store.media(&choice.id, &next.items[&choice.id]);
                    staged = Some(install_picture(c, path, choice.clone(), &mut next, parent, size).await?);
                }
            }
            let staged = match staged {
                Some(file) => file,
                None => c.store.stage_profile(&next)?,
            };
            if confirm {
                c.publish(ALL.into(), next, staged).await?;
            } else {
                c.commit_staged(next, staged)?;
            }
            Ok((None, None))
        }
        Action::Export | Action::ExportAs => {
            let destination = if matches!(action, Action::ExportAs) {
                Some(
                    glib::future_with_timeout(
                        std::time::Duration::from_secs(180),
                        portal::choose_report(parent, locale),
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
