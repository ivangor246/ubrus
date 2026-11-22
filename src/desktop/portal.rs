use crate::preferences::{Locale, Text};
use anyhow::{Context, Result};
use ashpd::{
    WindowIdentifier,
    desktop::{
        file_chooser::SelectedFiles,
        wallpaper::{SetOn, WallpaperRequest},
    },
};
use std::{
    fs::File,
    path::{Path, PathBuf},
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Appearance {
    pub dark: bool,
    pub high_contrast: bool,
}

pub fn system_appearance() -> impl iced::futures::Stream<Item = Appearance> {
    use ashpd::desktop::settings::{ColorScheme, Contrast, Settings};
    use iced::futures::{SinkExt, StreamExt};
    const GNOME_A11Y: &str = "org.gnome.desktop.a11y.interface";

    iced::stream::channel(1, async |mut output| {
        let Ok(settings) = Settings::new().await else { return };
        let Ok(changes) = settings.receive_setting_changed().await else {
            return;
        };
        let mut changes = changes.filter(|setting| {
            std::future::ready(matches!(
                (setting.namespace(), setting.key()),
                ("org.freedesktop.appearance", "color-scheme" | "contrast") | (GNOME_A11Y, "high-contrast")
            ))
        });
        loop {
            let high_contrast = match settings.contrast().await {
                Ok(value) => value == Contrast::High,
                Err(_) => settings
                    .read::<bool>(GNOME_A11Y, "high-contrast")
                    .await
                    .unwrap_or(false),
            };
            let dark = settings.color_scheme().await.ok() == Some(ColorScheme::PreferDark);
            if output.send(Appearance { dark, high_contrast }).await.is_err() {
                break;
            }
            if changes.next().await.is_none() {
                break;
            }
        }
    })
}

pub fn parent(window: &dyn iced::window::Window) -> Result<Option<WindowIdentifier>> {
    let handle = window.window_handle()?.as_raw();
    let display = window.display_handle()?.as_raw();
    Ok(iced::futures::executor::block_on(WindowIdentifier::from_raw_handle(
        &handle,
        Some(&display),
    )))
}

pub async fn choose_report(parent: Option<WindowIdentifier>, locale: Locale) -> Result<PathBuf> {
    SelectedFiles::save_file()
        .identifier(parent)
        .title(locale.get(Text::SaveAs))
        .accept_label(locale.get(Text::Save))
        .current_name("ubrus-report.txt")
        .send()
        .await?
        .response()?
        .uris()
        .first()
        .and_then(|uri| uri.to_file_path().ok())
        .context("Choose a local destination")
}

pub async fn choose_files(
    parent: Option<WindowIdentifier>,
    locale: Locale,
    cancel: &gtk::gio::Cancellable,
) -> Result<Vec<PathBuf>> {
    use gtk::{gio, glib, prelude::*};
    use iced::futures::future::{Either, select};
    const BUS: &str = "org.freedesktop.portal.Desktop";
    const REQUEST: &str = "org.freedesktop.portal.Request";
    let bus = gio::bus_get_future(gio::BusType::Session).await?;
    let token = format!("ubrus_{}", glib::uuid_string_random().replace('-', "_"));
    let sender = bus.unique_name().context("Missing portal connection name")?;
    let sender = sender.trim_start_matches(':').replace('.', "_");
    let expected = format!("/org/freedesktop/portal/desktop/request/{sender}/{token}");
    let (tx, rx) = async_channel::unbounded();
    // Subscribe before OpenFile: a fast chooser can reply before the method returns.
    let _subscription = bus.subscribe_to_signal(
        Some(BUS),
        Some(REQUEST),
        Some("Response"),
        None,
        None,
        gio::DBusSignalFlags::NONE,
        move |s| {
            let _ = tx.try_send((s.object_path.to_owned(), s.parameters.clone()));
        },
    );
    let options = glib::VariantDict::new(None);
    options.insert("multiple", true);
    options.insert("handle_token", token);
    options.insert("accept_label", locale.get(Text::Open));
    let types = [
        "image/png",
        "image/jpeg",
        "image/webp",
        "image/gif",
        "video/mp4",
        "video/webm",
    ]
    .map(|mime| (1u32, mime))
    .to_vec();
    options.insert("filters", vec![(locale.get(Text::Formats), types)]);
    let parent_id = parent.as_ref().map(ToString::to_string).unwrap_or_default();
    let args = glib::Variant::tuple_from_iter([
        parent_id.to_variant(),
        locale.get(Text::Import).to_variant(),
        options.end(),
    ]);
    let reply = bus
        .call_future(
            Some(BUS),
            "/org/freedesktop/portal/desktop",
            "org.freedesktop.portal.FileChooser",
            "OpenFile",
            Some(&args),
            None,
            gio::DBusCallFlags::NONE,
            10000,
        )
        .await?;
    let path = reply.child_value(0).str().unwrap_or(&expected).to_owned();
    let response = async {
        loop {
            let (from, value) = rx.recv().await?;
            if from == path {
                break Ok::<_, anyhow::Error>(value);
            }
        }
    };
    let value = match select(Box::pin(response), cancel.future()).await {
        Either::Left((value, _)) => value?,
        Either::Right(_) => {
            let _ = bus
                .call_future(
                    Some(BUS),
                    &path,
                    REQUEST,
                    "Close",
                    None,
                    None,
                    gio::DBusCallFlags::NONE,
                    3000,
                )
                .await;
            return Ok(Vec::new());
        }
    };
    match value.child_get::<u32>(0) {
        0 => glib::VariantDict::new(Some(&value.child_value(1)))
            .lookup::<Vec<String>>("uris")?
            .context("File chooser returned no paths")?
            .iter()
            .map(|uri| gio::File::for_uri(uri).path().context("Open a local file"))
            .collect(),
        1 => Ok(Vec::new()),
        _ => anyhow::bail!("File chooser failed"),
    }
}

pub async fn wallpaper(parent: Option<WindowIdentifier>, path: &Path) -> Result<()> {
    let file = File::open(path)?;
    WallpaperRequest::default()
        .identifier(parent)
        .set_on(SetOn::Background)
        .show_preview(false)
        .build_file(&file)
        .await?
        .response()?;
    Ok(())
}

pub fn system_picture(
    path: &Path,
    choice: &crate::library::Choice,
    width: i32,
    height: i32,
) -> Result<tempfile::NamedTempFile> {
    use gtk::gdk_pixbuf::{Colorspace, InterpType, Pixbuf};
    let w = width.clamp(1, 8192);
    let h = height.clamp(1, 8192);
    let src = Pixbuf::from_file(path)?;
    let src = src.apply_embedded_orientation().unwrap_or(src);
    let (x, y, dw, dh) = choice.frame.bounds(
        choice.fit,
        (src.width().into(), src.height().into()),
        (w.into(), h.into()),
    );
    let scale = dw / f64::from(src.width());
    let dst = Pixbuf::new(Colorspace::Rgb, false, 8, w, h).context("Insufficient image memory")?;
    dst.fill(0x000000ff);
    let left = (x.max(0.).round() as i32).clamp(0, w - 1);
    let top = (y.max(0.).round() as i32).clamp(0, h - 1);
    src.composite(
        &dst,
        left,
        top,
        (dw.round() as i32).clamp(1, w - left),
        (dh.round() as i32).clamp(1, h - top),
        x,
        y,
        scale,
        scale,
        InterpType::Bilinear,
        255,
    );
    let dst = if choice.frame.brightness == 100 {
        dst
    } else {
        let mut bytes = dst.read_pixel_bytes().as_ref().to_vec();
        for row in bytes.chunks_mut(dst.rowstride() as usize) {
            choice.frame.brighten(&mut row[..w as usize * 3], 3);
        }
        Pixbuf::from_mut_slice(bytes, Colorspace::Rgb, false, 8, w, h, dst.rowstride())
    };
    let tmp = tempfile::Builder::new().suffix(".png").tempfile()?;
    dst.savev(tmp.path(), "png", &[])?;
    Ok(tmp)
}
