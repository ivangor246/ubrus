use super::*;

#[test]
fn video_formats_can_be_imported_from_an_extensionless_tempfile() -> Result<()> {
    gst::init()?;
    let dir = tempfile::tempdir()?;
    for ext in ["gif", "mp4", "webm"] {
        let file = Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("tests/media/loop.{ext}"));
        let tmp = dir.path().join("incoming");
        std::fs::copy(file, &tmp)?;
        let item = Item {
            motion: None,
            name: "loop".into(),
            ext: ext.into(),
        };
        let poster = dir.path().join("poster.png");
        thumbnail(&tmp, &item, &poster).with_context(|| format!("Import {ext}"))?;
        assert!(Pixbuf::from_file(poster)?.width() > 0);
    }
    Ok(())
}

#[test]
fn imports_gif_whose_trailer_is_past_the_first_read_buffer() -> Result<()> {
    let tmp = tempfile::tempdir()?;
    let mut bytes = include_bytes!("../media/loop.gif").to_vec();
    assert_eq!(bytes.pop(), Some(0x3b));
    // A legal GIF comment moves the trailer past GdkPixbuf's first 64 KiB read.
    bytes.extend_from_slice(&[0x21, 0xfe]);
    for _ in 0..256 {
        bytes.push(255);
        bytes.extend_from_slice(&[b'x'; 255]);
    }
    bytes.extend_from_slice(&[0, 0x3b]);
    let source = tmp.path().join("wallpaper.gif");
    std::fs::write(&source, bytes)?;
    let store = crate::library::Store::at(tmp.path().join("library"))?;
    let (id, item) = store.import(&source, |_, _| {}, thumbnail)?;
    let poster = Pixbuf::from_file(store.thumb(&id))?;
    assert!(poster.width() > 0 && poster.width() <= 384);
    assert!(poster.height() > 0 && poster.height() <= 256);
    let animation = gtk::gdk_pixbuf::PixbufAnimation::from_file(store.media(&id, &item))?;
    assert!(!gtk::prelude::PixbufAnimationExt::is_static_image(&animation));
    Ok(())
}

#[test]
fn image_motion_survives_import_and_loading_an_old_library() -> Result<()> {
    let tmp = tempfile::tempdir()?;
    let store = crate::library::Store::at(tmp.path().into())?;
    let mut p = crate::library::Profile::default();
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/media");
    for (name, animated) in [
        ("single.gif", false),
        ("loop.gif", true),
        ("loop.webp", true),
        ("landscape.png", false),
    ] {
        let (id, mut item) = store.import(&fixtures.join(name), |_, _| {}, thumbnail)?;
        assert_eq!(item.animated(), animated);
        item.motion = None;
        p.items.insert(id.clone(), item);
        store.save(&p)?;
        assert_eq!(store.load()?.items[&id].animated(), animated);
    }
    Ok(())
}
