use super::VideoPipeline;
use crate::library::Item;
use anyhow::{Context, Result, bail};
use gst::prelude::*;
use gtk::{
    gdk_pixbuf::{Colorspace, InterpType, Pixbuf, PixbufAnimation},
    glib,
    prelude::PixbufAnimationExt,
};
use std::path::Path;

// A bounded first-frame decode runs on the import worker. Files become visible in the library only after this succeeds.
pub fn thumbnail(path: &Path, item: &Item, output: &Path) -> Result<Option<bool>> {
    let mut motion = None;
    let pixbuf = if matches!(item.ext.as_str(), "mp4" | "webm") {
        let bin = gst::parse::bin_from_description(
            "videoconvert ! videoscale ! video/x-raw,format=RGB,width=(int)[1,384],height=(int)[1,256],pixel-aspect-ratio=1/1 ! appsink name=thumb max-buffers=1 drop=true",
            true,
        )?;
        let sink = bin.by_name("thumb").unwrap().downcast::<gst_app::AppSink>().unwrap();
        let pipeline = VideoPipeline::open(path, bin.upcast_ref())?;
        pipeline.set_state(gst::State::Paused)?;
        // playbin attaches its dynamic sink after typefinding. Wait for its PAUSED transition first.
        let _ = pipeline.state(gst::ClockTime::from_seconds(8));
        let sample = sink
            .try_pull_preroll(gst::ClockTime::ZERO)
            .or_else(|| sink.try_pull_sample(gst::ClockTime::ZERO));
        let Some(sample) = sample else {
            if let Some(message) = pipeline.bus().unwrap().pop_filtered(&[gst::MessageType::Error])
                && let gst::MessageView::Error(e) = message.view()
            {
                bail!("{}", e.error());
            }
            bail!("Could not decode the first frame within 8 seconds");
        };
        let caps = sample
            .caps()
            .context("No video format")?
            .structure(0)
            .context("No video format")?;
        let w = caps.get::<i32>("width")?;
        let h = caps.get::<i32>("height")?;
        let buffer = sample.buffer().context("Empty video frame")?.map_readable()?;
        let stride = (w * 3 + 3) & !3;
        anyhow::ensure!(
            buffer.len() >= (stride * (h - 1) + w * 3) as usize,
            "Incomplete video frame"
        );
        Pixbuf::from_bytes(
            &glib::Bytes::from_owned(buffer.to_vec()),
            Colorspace::Rgb,
            false,
            8,
            w,
            h,
            stride,
        )
    } else if matches!(item.ext.as_str(), "gif" | "webp") {
        // The scaled still-image loader may stop reading GIFs before the trailer.
        let animation = PixbufAnimation::from_file(path)?;
        motion = Some(!animation.is_static_image());
        let first = animation.static_image().context("GIF has no frame")?;
        let scale = (384.0 / f64::from(first.width())).min(256.0 / f64::from(first.height()));
        if scale >= 1.0 {
            first
        } else {
            let width = (f64::from(first.width()) * scale).round().max(1.) as i32;
            let height = (f64::from(first.height()) * scale).round().max(1.) as i32;
            first
                .scale_simple(width, height, InterpType::Bilinear)
                .context("Cannot scale GIF poster")?
        }
    } else {
        let p = Pixbuf::from_file_at_scale(path, 384, 256, true)?;
        p.apply_embedded_orientation().unwrap_or(p)
    };
    pixbuf.savev(output, "png", &[])?;
    Ok(motion)
}

#[cfg(test)]
#[path = "../../tests/rust/media.rs"]
mod tests;
