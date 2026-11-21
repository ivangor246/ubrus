use anyhow::{Context, Result, bail, ensure};
use async_channel::Sender;
use gst::prelude::*;
use gtk::{
    gdk_pixbuf::{Colorspace, InterpType, PixbufAnimation},
    prelude::*,
};
use iced::widget::image::Handle;
use std::{
    path::{Path, PathBuf},
    sync::mpsc,
    time::{Duration, SystemTime},
};

pub type Frame = Result<Handle, String>;

// Dropping the sender wakes the GIF timer. Video checks for cancellation at least every 100 ms.
pub struct Playback {
    _stop: mpsc::Sender<()>,
    pub frames: async_channel::Receiver<Frame>,
}

pub fn start(path: PathBuf, gif: bool) -> Playback {
    let (stop, cancel) = mpsc::channel();
    let (frames, received) = async_channel::bounded(1);
    std::thread::spawn(move || {
        let result = if gif {
            play_gif(&path, &cancel, &frames)
        } else {
            play_video(&path, &cancel, &frames)
        };
        if let Err(e) = result {
            let _ = frames.send_blocking(Err(format!("{e:#}")));
        }
    });
    Playback {
        _stop: stop,
        frames: received,
    }
}

fn rgba(width: u32, height: u32, stride: i32, pixels: &[u8]) -> Result<Handle> {
    ensure!(
        (1..=640).contains(&width) && (1..=360).contains(&height),
        "Invalid preview dimensions"
    );
    let row = width as usize * 4;
    let stride = usize::try_from(stride)?;
    ensure!(
        stride >= row && pixels.len() >= stride * (height as usize - 1) + row,
        "Incomplete preview frame"
    );
    let mut packed = Vec::with_capacity(row * height as usize);
    for line in pixels.chunks(stride).take(height as usize) {
        packed.extend_from_slice(&line[..row]);
    }
    Ok(Handle::from_rgba(width, height, packed))
}

fn play_gif(path: &Path, cancel: &mpsc::Receiver<()>, frames: &Sender<Frame>) -> Result<()> {
    let animation = PixbufAnimation::from_file(path)?;
    let mut iter = animation.iter(Some(SystemTime::now()));
    loop {
        if !frames.is_full() {
            let p = iter.pixbuf();
            let factor = (640. / f64::from(p.width())).min(360. / f64::from(p.height())).min(1.);
            let (w, h) = (
                (f64::from(p.width()) * factor).round().max(1.) as i32,
                (f64::from(p.height()) * factor).round().max(1.) as i32,
            );
            let p = if factor < 1. {
                p.scale_simple(w, h, InterpType::Bilinear).context("Cannot scale GIF")?
            } else {
                p
            };
            ensure!(
                p.colorspace() == Colorspace::Rgb && p.bits_per_sample() == 8,
                "Invalid GIF pixel format"
            );
            let p = if p.has_alpha() { p } else { p.add_alpha(false, 0, 0, 0)? };
            let pixels = p.read_pixel_bytes();
            let handle = rgba(w as u32, h as u32, p.rowstride(), &pixels)?;
            let _ = frames.try_send(Ok(handle));
        }
        let delay = iter.delay_time();
        if animation.is_static_image() {
            return Ok(());
        }
        if !matches!(
            cancel.recv_timeout(
                delay
                    .unwrap_or(Duration::from_millis(20))
                    .max(Duration::from_millis(10))
            ),
            Err(mpsc::RecvTimeoutError::Timeout)
        ) {
            return Ok(());
        }
        if delay.is_none() {
            iter = animation.iter(Some(SystemTime::now()));
        } else {
            iter.advance(SystemTime::now());
        }
    }
}

fn play_video(path: &Path, cancel: &mpsc::Receiver<()>, frames: &Sender<Frame>) -> Result<()> {
    gst::init()?;
    let bin = gst::parse::bin_from_description(
        "videoconvert ! videoscale ! video/x-raw,format=RGBA,width=(int)[1,640],height=(int)[1,360],pixel-aspect-ratio=1/1 ! appsink name=frames max-buffers=1 drop=true",
        true,
    )?;
    let sink = bin.by_name("frames").unwrap().downcast::<gst_app::AppSink>().unwrap();
    let player = crate::media::VideoPipeline::open(path, bin.upcast_ref())?;
    let bus = player.bus().context("No playback bus")?;
    player.set_state(gst::State::Playing)?;
    while matches!(cancel.try_recv(), Err(mpsc::TryRecvError::Empty)) {
        if let Some(sample) = sink.try_pull_sample(gst::ClockTime::from_mseconds(100))
            && !frames.is_full()
        {
            use gst_video::VideoFrameExt;
            let info = gst_video::VideoInfo::from_caps(sample.caps().context("Missing video format")?)?;
            let frame = gst_video::VideoFrameRef::from_buffer_ref_readable(
                sample.buffer().context("Missing video frame")?,
                &info,
            )?;
            let handle = rgba(
                frame.width(),
                frame.height(),
                frame.plane_stride()[0],
                frame.plane_data(0)?,
            )?;
            let _ = frames.try_send(Ok(handle));
        }
        if let Some(message) = bus.pop_filtered(&[gst::MessageType::Error, gst::MessageType::Eos]) {
            match message.view() {
                gst::MessageView::Error(e) => bail!("{}", e.error()),
                gst::MessageView::Eos(_) => {
                    player.seek_simple(gst::SeekFlags::FLUSH | gst::SeekFlags::KEY_UNIT, gst::ClockTime::ZERO)?;
                }
                _ => {}
            }
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "../../tests/rust/preview.rs"]
mod tests;
