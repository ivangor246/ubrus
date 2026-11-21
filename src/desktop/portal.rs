use crate::preferences::{Locale, Text};
use anyhow::{Context, Result};
use ashpd::{
    WindowIdentifier,
    desktop::{
        file_chooser::{FileFilter, SelectedFiles},
        wallpaper::{SetOn, WallpaperRequest},
    },
};
use std::{
    fs::File,
    path::{Path, PathBuf},
};

pub fn system_dark() -> impl iced::futures::Stream<Item = bool> {
    use ashpd::desktop::settings::{ColorScheme, Settings};
    use iced::futures::{SinkExt, StreamExt};

    iced::stream::channel(1, async |mut output| {
        let Ok(settings) = Settings::new().await else { return };
        let Ok(mut changes) = settings.receive_color_scheme_changed().await else {
            return;
        };
        if let Ok(scheme) = settings.color_scheme().await {
            let _ = output.send(scheme == ColorScheme::PreferDark).await;
        }
        while let Some(scheme) = changes.next().await {
            if output.send(scheme == ColorScheme::PreferDark).await.is_err() {
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

pub async fn choose(parent: Option<WindowIdentifier>, locale: Locale, export: bool) -> Result<PathBuf> {
    let request = if export {
        SelectedFiles::save_file()
            .identifier(parent)
            .title(locale.get(Text::SaveAs))
            .accept_label(locale.get(Text::Save))
            .current_name("ubrus-report.txt")
            .send()
            .await?
    } else {
        let filter = [
            "image/png",
            "image/jpeg",
            "image/webp",
            "image/gif",
            "video/mp4",
            "video/webm",
        ]
        .into_iter()
        .fold(FileFilter::new(locale.get(Text::Formats)), |filter, mime| {
            filter.mimetype(mime)
        });
        SelectedFiles::open_file()
            .identifier(parent)
            .title(locale.get(Text::Import))
            .accept_label(locale.get(Text::Open))
            .filter(filter)
            .multiple(false)
            .send()
            .await?
    };
    request
        .response()?
        .uris()
        .first()
        .and_then(|uri| uri.to_file_path().ok())
        .context("Open a local file")
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
