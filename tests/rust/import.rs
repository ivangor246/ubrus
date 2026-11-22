use super::*;
use crate::library::{Profile, Store};

#[test]
fn batch_keeps_successes_on_both_sides_of_a_corrupt_file() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let store = Store::at(dir.path().join("library"))?;
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/media");
    let bad = dir.path().join("broken.png");
    std::fs::write(&bad, b"not an image")?;
    let paths = vec![fixtures.join("landscape.png"), bad.clone(), fixtures.join("single.gif")];
    let mut profile = Profile::default();
    let errors = std::cell::RefCell::new(Vec::new());
    let (feedback, selected) = iced::futures::executor::block_on(import_paths(
        paths,
        &gio::Cancellable::new(),
        async |_, path| {
            let prepared = store.prepare_import(path, &profile.items, |_, _| Ok(()), media::thumbnail)?;
            store.commit_import(&mut profile, prepared).map(Some)
        },
        |path, _| errors.borrow_mut().push(path.to_owned()),
    ))?;
    assert_eq!(
        feedback,
        Some(Feedback::ImportBatch {
            imported: 2,
            failed: 1,
            skipped: 0,
            stopped: None
        })
    );
    assert_eq!(store.load()?.items.len(), 2);
    assert_eq!(profile.items[&selected.unwrap()].name, "single.gif");
    assert_eq!(errors.into_inner(), vec![bad]);
    Ok(())
}

#[test]
fn disk_full_counts_the_remaining_files_as_skipped() -> Result<()> {
    let paths = ["a", "b", "c"].map(PathBuf::from).to_vec();
    let (feedback, selected) = iced::futures::executor::block_on(import_paths(
        paths,
        &gio::Cancellable::new(),
        async |index, _| match index {
            0 => Ok(Some("saved".into())),
            1 => Err(std::io::Error::from_raw_os_error(28).into()),
            _ => panic!("batch continued after exhausting storage"),
        },
        |_, _| {},
    ))?;
    assert_eq!(selected.as_deref(), Some("saved"));
    assert_eq!(
        feedback,
        Some(Feedback::ImportBatch {
            imported: 1,
            failed: 1,
            skipped: 1,
            stopped: Some(Text::DiskFull)
        })
    );
    Ok(())
}
