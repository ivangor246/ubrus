use super::*;

#[test]
fn removing_a_portal_wallpaper_restores_both_themes_and_scaling() -> Result<()> {
    let settings = gio::Settings::with_backend(SCHEMA, &gio::memory_settings_backend_new());
    let file = tempfile::NamedTempFile::new()?;
    let uri = gio::File::for_path(file.path()).uri();
    settings.set_string(LIGHT, "file:///light.png")?;
    settings.set_string(DARK, "file:///dark.png")?;
    settings.set_string(STYLE, "scaled")?;
    let first = SystemWallpaper::capture_settings(&settings, &uri, None);
    settings.set_string(LIGHT, &uri)?;
    settings.set_string(DARK, &uri)?;
    settings.set_string(STYLE, "zoom")?;
    let mut second = SystemWallpaper::capture_settings(&settings, &uri, Some(&first));
    second.digest = Some(checksum(file.path())?);
    second.restore_native(&settings, file.path())?;
    assert_eq!(settings.string(LIGHT), "file:///light.png");
    assert_eq!(settings.string(DARK), "file:///dark.png");
    assert_eq!(settings.string(STYLE), "scaled");
    Ok(())
}

#[test]
fn removing_leaves_another_apps_portal_image_and_manual_changes_in_place() -> Result<()> {
    let settings = gio::Settings::with_backend(SCHEMA, &gio::memory_settings_backend_new());
    let file = tempfile::NamedTempFile::new()?;
    let uri = gio::File::for_path(file.path()).uri();
    let mut saved = SystemWallpaper {
        digest: Some(checksum(file.path())?),
        ..Default::default()
    };
    settings.set_string(LIGHT, &uri)?;
    settings.set_string(DARK, "file:///custom.png")?;
    std::fs::write(file.path(), "another portal image")?;
    saved.restore_native(&settings, file.path())?;
    assert_eq!(settings.string(LIGHT), uri);
    saved.digest = None; // An older installation only recorded the wallpaper assignment.
    saved.restore_native(&settings, file.path())?;
    assert_eq!(settings.string(LIGHT), uri);
    assert_eq!(settings.string(DARK), "file:///custom.png");
    Ok(())
}

#[test]
fn another_portal_clients_previous_picture_survives_the_shared_cache_being_overwritten() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let settings = gio::Settings::with_backend(SCHEMA, &gio::memory_settings_backend_new());
    let cache = dir.path().join("background");
    let uri = gio::File::for_path(&cache).uri();
    settings.set_string(LIGHT, &uri)?;
    std::fs::write(&cache, b"previous image")?;
    let mut saved = SystemWallpaper::capture_native(&settings, &cache, None, dir.path())?;
    std::fs::write(&cache, b"Ubrus image")?;
    saved.digest = Some(checksum(&cache)?);
    saved.restore_native(&settings, &cache)?;
    let restored = gio::File::for_uri(&settings.string(LIGHT)).path().unwrap();
    assert_eq!(std::fs::read(restored)?, b"previous image");
    Ok(())
}

#[test]
fn portal_photo_uses_the_selected_edge_and_brightness() -> Result<()> {
    use gtk::gdk_pixbuf::{Colorspace, Pixbuf};
    let tmp = tempfile::tempdir()?;
    let path = tmp.path().join("stripes.png");
    let src = Pixbuf::from_mut_slice(vec![240, 0, 0, 0, 0, 200], Colorspace::Rgb, false, 8, 2, 1, 6);
    src.savev(&path, "png", &[])?;
    for (x, expected) in [(0, [120, 0, 0]), (100, [0, 0, 100])] {
        let c = crate::library::Choice {
            id: String::new(),
            fit: false,
            frame: crate::framing::Framing {
                x,
                brightness: 50,
                ..Default::default()
            },
        };
        let picture = crate::desktop::portal::system_picture(&path, &c, 1, 1)?;
        assert_eq!(&Pixbuf::from_file(picture.path())?.read_pixel_bytes()[..3], &expected);
    }
    Ok(())
}
