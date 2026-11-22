use super::*;
#[test]
fn snap_refresh_compares_the_loaded_extension_to_the_current_revision() -> std::io::Result<()> {
    let root = tempfile::tempdir()?;
    for (rev, build) in [("1", "old"), ("2", "new")] {
        let directory = root.path().join(rev).join("share/ubrus");
        fs::create_dir_all(&directory)?;
        fs::write(directory.join("extension-build-id"), format!("{build}\n"))?;
    }
    let running = root.path().join("1");
    assert!(!snap_extension_changed(&running, "old"));
    std::os::unix::fs::symlink(root.path().join("2"), root.path().join("current"))?;
    assert!(snap_extension_changed(&running, "old"));
    assert!(!snap_extension_changed(&running, "new"));
    Ok(())
}
