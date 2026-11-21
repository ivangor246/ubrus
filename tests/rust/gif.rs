use super::*;

#[test]
fn late_timer_presents_the_current_gif_frame() -> Result<()> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/media/loop.gif");
    let gif = Gif::open(&path)?;
    let reference = gif.animation.iter(Some(SystemTime::UNIX_EPOCH));
    let start = std::time::Instant::now();
    gif.play(true);
    let before = gif.decoded.get();
    // Block dispatch across two frame boundaries, as can happen during a Shell transition.
    std::thread::sleep(Duration::from_millis(410));
    while gif.decoded.get() == before {
        glib::MainContext::default().iteration(true);
    }
    reference.advance(SystemTime::UNIX_EPOCH + start.elapsed());
    gif.play(false);
    assert!(gif.iter.borrow().pixbuf().read_pixel_bytes() == reference.pixbuf().read_pixel_bytes());
    assert!(!gif.is_playing());
    Ok(())
}

#[test]
fn pausing_keeps_the_displayed_texture_and_resumes_the_same_stream() -> Result<()> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/media/loop.gif");
    let gif = Gif::open(&path)?;
    let context = glib::MainContext::default();
    gif.play(true);
    let first = gif.decoded.get();
    while gif.decoded.get() == first {
        context.iteration(true);
    }
    gif.play(false);
    let frozen = gif.frames.current_image();
    std::thread::sleep(Duration::from_millis(220));
    while context.iteration(false) {}
    assert_eq!(gif.frames.current_image(), frozen);
    gif.play(true);
    assert_eq!(gif.frames.current_image(), frozen);
    while gif.frames.current_image() == frozen {
        context.iteration(true);
    }
    gif.play(false);
    Ok(())
}
