use super::*;
use crate::media;

#[test]
fn only_one_session_can_recover_or_update_a_library_at_a_time() -> Result<()> {
    let directory = tempfile::tempdir()?;
    let first = Store::at(directory.path().into())?;
    let second = Store::at(directory.path().into())?;
    let writer = first.try_writer()?.unwrap();
    assert!(second.try_writer()?.is_none());
    drop(writer);
    assert!(second.try_writer()?.is_some());
    Ok(())
}

#[test]
fn deleting_global_still_restores_the_desktop_and_keeps_per_monitor_overrides() -> Result<()> {
    let mut p = Profile::default();
    let item = Item {
        motion: None,
        name: "photo".into(),
        ext: "png".into(),
    };
    p.items.insert("global".into(), item.clone());
    p.items.insert("override".into(), item);
    p.assign(
        ALL,
        Choice {
            frame: Default::default(),
            id: "global".into(),
            fit: false,
        },
    )?;
    p.assign(
        "left",
        Choice {
            frame: Default::default(),
            id: "override".into(),
            fit: true,
        },
    )?;
    p.system_wallpaper = Some(Default::default());
    assert!(!p.restores_desktop_on_delete("override"));
    assert!(p.restores_desktop_on_delete("global"));
    p.delete_item("global")?;
    assert!(p.system_wallpaper.is_none());
    assert_eq!(p.screens["left"].id, "override");
    assert!(!p.screens.contains_key(ALL));
    Ok(())
}

#[test]
fn deleting_an_active_item_preserves_other_outputs_and_retries_file_cleanup() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let store = Store::at(dir.path().join("library"))?;
    let sources = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/media");
    let original = sources.join("loop.gif");
    let original_bytes = fs::read(&original)?;
    let (id, item) = store.import(&original, |_, _| {}, media::thumbnail)?;
    let (other, still) = store.import(&sources.join("landscape.png"), |_, _| {}, media::thumbnail)?;
    let mut p = Profile::default();
    p.items.insert(id.clone(), item.clone());
    p.items.insert(other.clone(), still.clone());
    p.assign(
        ALL,
        Choice {
            frame: Default::default(),
            id: id.clone(),
            fit: false,
        },
    )?;
    p.assign(
        "HDMI",
        Choice {
            frame: Default::default(),
            id: other.clone(),
            fit: true,
        },
    )?;
    p.delete_item(&id)?;
    store.save(&p)?;
    fs::remove_file(store.thumb(&id))?;
    let mut reopened = store.load()?;
    store.finish_deletions(&mut reopened)?;
    let p = store.load()?;
    assert!(!p.items.contains_key(&id) && !p.screens.contains_key(ALL));
    assert!(p.pending_deletions.is_empty());
    assert_eq!(p.screens["HDMI"].id, other);
    assert!(store.media(&other, &still).exists() && store.thumb(&other).exists());
    assert!(!store.media(&id, &item).exists() && !store.thumb(&id).exists());
    assert_eq!(fs::read(original)?, original_bytes);
    Ok(())
}

#[test]
fn importing_a_copy_survives_source_removal_and_deduplicates() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let store = Store::at(dir.path().join("library"))?;
    let src = dir.path().join("sample.png");
    let pix = gtk::gdk_pixbuf::Pixbuf::new(gtk::gdk_pixbuf::Colorspace::Rgb, false, 8, 16, 10).unwrap();
    pix.fill(0xff0000ff);
    pix.savev(&src, "png", &[])?;
    let (id, item) = store.import(&src, |_, _| (), media::thumbnail)?;
    let (dup, _) = store.import(&src, |_, _| (), |_, _, _| panic!("duplicate was decoded"))?;
    assert_eq!(dup, id);
    fs::remove_file(src)?;
    assert!(store.media(&id, &item).is_file());
    assert!(store.thumb(&id).is_file());
    Ok(())
}

#[test]
fn failed_import_leaves_previous_assignment_and_no_partial_media() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let store = Store::at(dir.path().join("library"))?;
    let src = dir.path().join("clip.mp4");
    fs::write(&src, b"fixture")?;
    let mut profile = Profile::default();
    let id = "a".repeat(64);
    profile.items.insert(
        id.clone(),
        Item {
            motion: None,
            name: "working".into(),
            ext: "png".into(),
        },
    );
    profile.assign(
        ALL,
        Choice {
            id,
            frame: Default::default(),
            fit: false,
        },
    )?;
    store.save(&profile)?;
    let error = store
        .import(
            &src,
            |_, _| (),
            |_, _, _| Err(std::io::Error::from_raw_os_error(28).into()),
        )
        .unwrap_err();
    assert!(error.downcast_ref::<std::io::Error>().is_some());
    assert_eq!(store.load()?.screens, profile.screens);
    assert_eq!(fs::read_dir(store.0.join("media"))?.count(), 0);
    assert_eq!(fs::read_dir(store.0.join("thumbs"))?.count(), 0);
    Ok(())
}
#[test]
fn reconnect_preserves_overrides_and_login_opt_out() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let store = Store::at(dir.path().into())?;
    let mut p = Profile::default();
    let id = "b".repeat(64);
    p.items.insert(
        id.clone(),
        Item {
            motion: None,
            name: "clip".into(),
            ext: "webm".into(),
        },
    );
    let c = Choice {
        id,
        frame: Default::default(),
        fit: true,
    };
    p.assign(ALL, c.clone())?;
    assert_eq!(p.autostart, Some(true));
    p.autostart = Some(false);
    p.assign("HDMI-A", c.clone())?;
    store.save(&p)?;
    let json: serde_json::Value = serde_json::from_slice(&fs::read(store.0.join("profile.json"))?)?;
    let still = &json["wallpaper_stills"]["screens"]["HDMI-A"];
    assert_eq!(still["animated"], false);
    assert_eq!(still["fit"], true);
    assert_eq!(still["path"], serde_json::to_value(store.thumb(&c.id))?);
    let mut loaded = store.load()?;
    assert_eq!(loaded.autostart, Some(false));
    assert_eq!(loaded.screens["HDMI-A"], c);
    loaded.assign(ALL, c)?;
    assert_eq!(loaded.screens.len(), 1);
    Ok(())
}

#[test]
fn rotates_before_a_multibyte_log_entry_crosses_the_limit() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let store = Store::at(dir.path().into())?;
    let log = dir.path().join("ubrus.log");
    fs::write(&log, vec![b'x'; 512 * 1024 - 40])?;
    store.log(&"Я".repeat(48));
    assert_eq!(fs::metadata(dir.path().join("ubrus.log.1"))?.len(), 512 * 1024 - 40);
    assert!(fs::read_to_string(log)?.ends_with(&format!("{}\n", "Я".repeat(48))));
    Ok(())
}

#[test]
fn clearing_all_screens_keeps_imports_and_removes_recovery_posters() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let store = Store::at(dir.path().to_owned())?;
    let mut profile = Profile::default();
    for ext in ["png", "gif"] {
        let name = if ext == "png" { "landscape.png" } else { "loop.gif" };
        let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/media").join(name);
        let (id, item) = store.import(&src, |_, _| {}, media::thumbnail)?;
        profile.items.insert(id.clone(), item);
        profile.assign(
            if ext == "png" { ALL } else { "HDMI-A" },
            Choice {
                id,
                frame: Default::default(),
                fit: false,
            },
        )?;
    }
    store.save(&profile)?;
    let mut loaded = store.load()?;
    assert!(loaded.system_wallpaper.is_none());
    loaded.hidden = true;
    loaded.system_wallpaper = None;
    store.save(&loaded)?;
    let cleared = store.load()?;
    assert!(cleared.screens == loaded.screens && !cleared.visible() && cleared.system_wallpaper.is_none());
    assert_eq!(cleared.items, profile.items);
    for (id, item) in &cleared.items {
        assert!(store.media(id, item).exists() && store.thumb(id).exists());
    }
    let saved: serde_json::Value = serde_json::from_slice(&fs::read(store.profile_path())?)?;
    assert_eq!(saved["wallpaper_stills"]["screens"], serde_json::json!({}));
    Ok(())
}

#[test]
fn legacy_single_frame_gif_preserves_its_assignment_without_claiming_the_portal() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let store = Store::at(dir.path().to_owned())?;
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/media/single.gif");
    let (id, item) = store.import(&source, |_, _| {}, media::thumbnail)?;
    let input = serde_json::json!({
        "items": { &id: {"ext": item.ext, "name": item.name} },
        "screens": {"*": {"id": id, "fit": true}},
        "autostart": false, "power": false, "paused": true
    });
    fs::write(store.profile_path(), serde_json::to_vec(&input)?)?;
    let profile = store.load()?;
    assert!(!profile.items[&id].animated());
    assert_eq!(profile.screens[ALL].frame, Default::default());
    assert!(profile.system_wallpaper.is_none() && profile.visible());
    assert_eq!(profile.autostart, Some(false));
    Ok(())
}

#[test]
fn import_cancels_before_publication_and_rolls_back_on_profile_write_failure() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let store = Store::at(dir.path().join("data"))?;
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/media/loop.gif");
    let mut profile = Profile::default();
    let decoded = std::cell::Cell::new(false);
    let cancelled = store.prepare_import(
        &source,
        &profile.items,
        |_, _| {
            ensure!(!decoded.get(), "cancelled");
            Ok(())
        },
        |path, item, poster| {
            let result = media::thumbnail(path, item, poster)?;
            decoded.set(true);
            Ok(result)
        },
    );
    assert!(matches!(cancelled, Err(e) if e.to_string() == "cancelled"));
    let prepared = store.prepare_import(&source, &profile.items, |_, _| Ok(()), media::thumbnail)?;
    fs::create_dir(store.profile_path())?;
    assert!(store.commit_import(&mut profile, prepared).is_err());
    assert!(profile.items.is_empty());
    for name in ["media", "thumbs"] {
        assert_eq!(fs::read_dir(store.root().join(name))?.count(), 0);
    }
    Ok(())
}

#[test]
fn duplicate_extensions_share_the_first_item_and_recovery_keeps_unclaimed_media() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let store = Store::at(dir.path().join("data"))?;
    let original = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/media/landscape.png");
    let duplicate = dir.path().join("same.jpg");
    fs::copy(&original, &duplicate)?;
    let (id, item) = store.import(&original, |_, _| (), media::thumbnail)?;
    let (other, canonical) = store.import(&duplicate, |_, _| (), |_, _, _| panic!("duplicate decoded"))?;
    assert_eq!(id, other);
    assert_eq!(canonical, item);
    let orphan = store.root().join("media").join(format!("{id}.jpg"));
    assert!(!orphan.exists());
    fs::copy(&duplicate, &orphan)?;
    let tmp = store.root().join("thumbs/.ubrus-import-abandoned");
    fs::write(&tmp, b"partial")?;
    let foreign = store.root().join("media/local-notes.txt");
    fs::write(&foreign, b"keep")?;
    store.cleanup_imports(&store.load()?)?;
    assert!(orphan.is_file() && !tmp.exists() && foreign.is_file());
    assert!(store.media(&id, &item).is_file() && store.thumb(&id).is_file());
    Ok(())
}

#[test]
fn newer_profile_remains_readable_without_losing_unknown_fields() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let store = Store::at(dir.path().into())?;
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/media/single.gif");
    let (id, item) = store.import(&source, |_, _| (), media::thumbnail)?;
    let mut json: serde_json::Value = serde_json::from_slice(&fs::read(store.profile_path())?)?;
    json["storage_version"] = 3.into();
    json["future_preference"] = "unchanged".into();
    let bytes = serde_json::to_vec(&json)?;
    fs::write(store.profile_path(), &bytes)?;
    let (p, writable) = store.read_profile()?;
    assert!(!writable && p.items[&id] == item);
    assert_eq!(fs::read(store.profile_path())?, bytes);
    Ok(())
}

#[test]
fn disabling_and_reenabling_preserves_an_explicit_login_opt_out() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let store = Store::at(dir.path().into())?;
    let p = Profile {
        autostart: Some(false),
        disabled: true,
        ..Default::default()
    };
    store.save(&p)?;
    let mut restored = store.load()?;
    restored.disabled = false;
    assert!(!restored.login_enabled());
    restored.autostart = Some(true);
    restored.disabled = true;
    assert!(!restored.login_enabled());
    restored.disabled = false;
    assert!(restored.login_enabled());
    Ok(())
}

#[test]
fn missing_profile_and_interrupted_publication_preserve_preexisting_managed_files() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let store = Store::at(dir.path().join("data"))?;
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/media/landscape.png");
    let (id, item) = store.import(&source, |_, _| (), media::thumbnail)?;
    fs::remove_file(store.profile_path())?;
    let missing = store.load()?;
    store.cleanup_imports(&missing)?;
    assert!(store.media(&id, &item).is_file() && store.thumb(&id).is_file());

    let partial = "a".repeat(64);
    let original = store.root().join("media").join(format!("{partial}.png"));
    fs::write(&original, b"present before import")?;
    fs::write(store.thumb(&partial), b"newly published")?;
    fs::write(
        store.root().join("import-change.json"),
        serde_json::to_vec(&serde_json::json!({
            "id": partial, "ext": "png", "new_media": false, "new_thumbnail": true
        }))?,
    )?;
    store.cleanup_imports(&missing)?;
    assert!(original.is_file() && !store.thumb(&partial).exists());
    assert!(store.media(&id, &item).is_file() && store.thumb(&id).is_file());
    Ok(())
}
