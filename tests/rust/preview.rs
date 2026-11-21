use super::*;

#[test]
fn animated_previews_produce_rgba_frames_and_stop_on_close() -> Result<()> {
    let ctx = gtk::glib::MainContext::new();
    for ext in ["gif", "mp4", "webm"] {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("tests/media/loop.{ext}"));
        let playback = start(path, ext == "gif");
        let rx = playback.frames.clone();
        ctx.block_on(gtk::glib::future_with_timeout(Duration::from_secs(10), async {
            for _ in 0..2 {
                let handle = rx.recv().await?.map_err(anyhow::Error::msg)?;
                let Handle::Rgba {
                    width, height, pixels, ..
                } = handle
                else {
                    bail!("Expected RGBA preview")
                };
                ensure!(width > 0 && width <= 640 && height > 0 && height <= 360);
                ensure!(pixels.len() == (width * height * 4) as usize);
            }
            drop(playback);
            while rx.recv().await.is_ok() {}
            Ok::<_, anyhow::Error>(())
        }))??;
    }
    Ok(())
}
