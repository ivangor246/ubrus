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
    rate: mpsc::Sender<u16>,
    pub frames: async_channel::Receiver<Frame>,
}

impl Playback {
    pub fn set_speed(&self, speed: u16) {
        let _ = self.rate.send(speed);
    }
}

pub fn start(path: PathBuf, gif: bool, speed: u16) -> Playback {
    let (stop, cancel) = mpsc::channel();
    let (frames, received) = async_channel::bounded(1);
    std::thread::spawn(move || {
        let result = if gif {
            play_gif(&path, &cancel, &frames, speed)
        } else {
            play_video(&path, &cancel, &frames, speed)
        };
        if let Err(e) = result {
            let _ = frames.send_blocking(Err(format!("{e:#}")));
        }
    });
    Playback {
        rate: stop,
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

fn play_gif(path: &Path, cancel: &mpsc::Receiver<u16>, frames: &Sender<Frame>, speed: u16) -> Result<()> {
    let animation = PixbufAnimation::from_file(path)?;
    let mut last = std::time::Instant::now();
    let mut elapsed = Duration::ZERO;
    let mut rate = f64::from(speed) / 100.;
    let mut iter = animation.iter(Some(SystemTime::UNIX_EPOCH));
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
        let wait = delay
            .unwrap_or(Duration::from_millis(20))
            .div_f64(rate)
            .max(Duration::from_millis(1));
        let next = cancel.recv_timeout(wait.saturating_sub(last.elapsed()));
        let now = std::time::Instant::now();
        elapsed += now.duration_since(last).mul_f64(rate);
        last = now;
        match next {
            Ok(speed) => rate = f64::from(cancel.try_iter().last().unwrap_or(speed)) / 100.,
            Err(mpsc::RecvTimeoutError::Disconnected) => return Ok(()),
            Err(mpsc::RecvTimeoutError::Timeout) => {}
        }
        if delay.is_none() {
            elapsed = Duration::ZERO;
            iter = animation.iter(Some(SystemTime::UNIX_EPOCH));
        } else {
            iter.advance(SystemTime::UNIX_EPOCH + elapsed);
        }
    }
}

fn play_video(path: &Path, cancel: &mpsc::Receiver<u16>, frames: &Sender<Frame>, speed: u16) -> Result<()> {
    gst::init()?;
    let bin = gst::parse::bin_from_description(
        "videoconvert ! videoscale ! video/x-raw,format=RGBA,width=(int)[1,640],height=(int)[1,360],pixel-aspect-ratio=1/1 ! appsink name=frames max-buffers=1 drop=true",
        true,
    )?;
    let sink = bin.by_name("frames").unwrap().downcast::<gst_app::AppSink>().unwrap();
    let player = crate::media::VideoPipeline::open(path, bin.upcast_ref())?;
    let bus = player.bus().context("No playback bus")?;
    if speed != 100 {
        player.set_state(gst::State::Paused)?;
        player.state(gst::ClockTime::from_seconds(8)).0?;
        crate::media::seek_start(&player, speed)?;
    }
    player.set_state(gst::State::Playing)?;
    let mut speed = speed;
    loop {
        match cancel.try_recv() {
            Ok(rate) => {
                speed = cancel.try_iter().last().unwrap_or(rate);
                let position = player
                    .query_position::<gst::ClockTime>()
                    .unwrap_or(gst::ClockTime::ZERO);
                crate::media::seek_at(&player, speed, position)?;
            }
            Err(mpsc::TryRecvError::Disconnected) => break,
            Err(mpsc::TryRecvError::Empty) => {}
        }
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
                    crate::media::seek_start(&player, speed)?;
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
