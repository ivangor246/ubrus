use super::*;
use crate::library::Choice;
use anyhow::Result;
#[test]
fn diagnostics_include_inherited_explicit_and_disconnected_assignments() -> Result<()> {
    let mut profile = library::Profile::default();
    for (screen, name) in [(ALL, "forest.png"), ("DP", "sea.png"), ("HDMI", "hills.png")] {
        profile.items.insert(
            name.into(),
            library::Item {
                motion: None,
                name: name.into(),
                ext: "png".into(),
            },
        );
        profile.assign(
            screen,
            Choice {
                frame: Default::default(),
                id: name.into(),
                fit: screen == "DP",
            },
        )?;
    }
    let status = crate::app::control::Status {
        outputs: ["DP", "Internal"]
            .map(|id| crate::app::control::Output {
                id: id.into(),
                name: id.into(),
                ..Default::default()
            })
            .into(),
        ..Default::default()
    };
    let summary = settings(Locale::English, &profile, &status);
    for (id, name, fit) in [
        ("Internal", "forest.png", false),
        ("DP", "sea.png", true),
        ("HDMI", "hills.png", false),
    ] {
        let mode = if fit { "Fit" } else { "Fill" };
        assert!(
            summary
                .lines()
                .any(|line| line.starts_with(id) && line.contains(name) && line.ends_with(mode))
        );
    }
    Ok(())
}

#[test]
fn saving_reports_needs_no_portal_and_preserves_earlier_files() -> Result<()> {
    let temp = tempfile::tempdir()?;
    let fallback = temp.path().join("reports");
    let missing = temp.path().join("unavailable");
    let first = save_report("первый", Some(temp.path()), &fallback)?;
    let second = save_report("second", Some(temp.path()), &fallback)?;
    assert_ne!(first, second);
    assert_eq!(first.parent(), Some(temp.path()));
    assert_eq!(std::fs::read_to_string(first)?, "первый");
    let third = save_report("fallback", Some(&missing), &fallback)?;
    assert_eq!(third.parent(), Some(fallback.as_path()));
    assert_eq!(std::fs::read_to_string(third)?, "fallback");
    Ok(())
}

#[test]
fn public_report_uses_only_structured_fields_and_status_checks_the_renderer() -> Result<()> {
    let mut profile = library::Profile::default();
    let private = "private-user-path";
    profile.items.insert(
        private.into(),
        library::Item {
            name: private.into(),
            ext: "mp4".into(),
            motion: None,
        },
    );
    profile.assign(
        ALL,
        Choice {
            id: private.into(),
            frame: Default::default(),
            fit: false,
        },
    )?;
    let mut status = crate::app::control::Status {
        outputs: vec![crate::app::control::Output {
            id: private.into(),
            name: private.into(),
            reason: "visible".into(),
            width: 1920,
            height: 1080,
        }],
        ..Default::default()
    };
    assert_eq!(wallpaper_status(&profile, &status), Text::Offline);
    status.connected = true;
    assert_eq!(wallpaper_status(&profile, &status), Text::RendererWaiting);
    status.engine =
        serde_json::json!({"displayed":{"0": private}, "streams":[{"playing":true,"decoder":private,"dropped":null}]});
    assert_eq!(wallpaper_status(&profile, &status), Text::Running);
    status.engine["streams"][0] = serde_json::json!({"paused":true});
    status.outputs[0].reason = "covered".into();
    assert_eq!(wallpaper_status(&profile, &status), Text::Covered);
    status.error = private.into();
    status.outputs[0].reason = private.into();
    assert_eq!(wallpaper_status(&profile, &status), Text::RendererError);
    assert!(public_report(&profile, &status).contains("1920×1080"));
    assert!(!public_report(&profile, &status).contains(private));
    Ok(())
}
