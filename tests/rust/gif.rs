use super::*;

#[test]
fn late_timer_and_pause_use_elapsed_playback_time() -> Result<()> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/media/loop.gif");
    let gif = Gif::open(&path, 100)?;
    let start = Instant::now();
    let late = Duration::from_millis(410);
    gif.play_at(true, start);
    let first = gif.frames.current_image();

    // Deliver the already scheduled callback at a known point in the animation.
    gif.timer.take().unwrap().remove();
    gif.tick(start + late);
    let reference = gif.animation.iter(Some(SystemTime::UNIX_EPOCH));
    reference.advance(SystemTime::UNIX_EPOCH + late);
    assert_eq!(
        gif.iter.borrow().pixbuf().read_pixel_bytes(),
        reference.pixbuf().read_pixel_bytes()
    );
    let frozen = gif.frames.current_image();
    assert_ne!(frozen, first);

    gif.play_at(false, start + late);
    assert!(!gif.is_playing());
    let resume = start + Duration::from_secs(10);
    gif.tick(resume);
    gif.play_at(true, resume);
    assert!(gif.is_playing());
    assert_eq!(gif.frames.current_image(), frozen);
    gif.timer.take().unwrap().remove();
    gif.tick(resume + Duration::from_millis(160));
    assert_ne!(gif.frames.current_image(), frozen);
    gif.play_at(false, resume + Duration::from_millis(160));
    Ok(())
}
