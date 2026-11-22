use crate::{
    library::{self, ALL},
    preferences::{Locale, Text},
};
pub fn wallpaper_status(profile: &library::Profile, status: &super::control::Status) -> Text {
    if profile.disabled {
        return Text::Disabled;
    }
    if profile.screens.is_empty() {
        return Text::NoWallpaper;
    }
    if profile.hidden {
        return Text::Hidden;
    }
    if !status.connected {
        return if profile.system_wallpaper.is_some() && !profile.has_animation() {
            Text::StillWallpaper
        } else {
            Text::Offline
        };
    }
    if !status.error.is_empty() {
        return Text::RendererError;
    }
    let mut animated = status
        .outputs
        .iter()
        .filter(|m| {
            profile
                .screens
                .get(&m.id)
                .or_else(|| profile.screens.get(ALL))
                .is_some_and(|c| profile.items.get(&c.id).is_some_and(|item| item.animated()))
        })
        .peekable();
    if animated.peek().is_none() {
        if status.outputs.is_empty() {
            return Text::RendererWaiting;
        }
        return if status
            .outputs
            .iter()
            .any(|m| profile.screens.contains_key(&m.id) || profile.screens.contains_key(ALL))
        {
            Text::StillWallpaper
        } else {
            Text::NoWallpaper
        };
    }
    let streams = status.engine["streams"].as_array();
    if streams.is_some_and(|streams| streams.iter().any(|s| s["playing"] == true)) {
        return Text::Running;
    }
    if profile.paused {
        return Text::Paused;
    }
    if streams.is_none_or(|streams| streams.is_empty() || streams.iter().any(|s| s["paused"] != true)) {
        return Text::RendererWaiting;
    }
    let first = animated.next().expect("animated output checked");
    match first.reason.as_str() {
        "power-saver" => Text::PowerPaused,
        "overview" => Text::OverviewPaused,
        "covered" if animated.all(|m| m.reason == "covered") => Text::Covered,
        _ => Text::Paused,
    }
}

// Construct public reports from typed fields. Paths, monitor identities, user filenames,
// diagnostic logs and free-form service error messages belong only in the private report.
pub fn public_report(profile: &library::Profile, status: &super::control::Status) -> String {
    let session = match std::env::var("XDG_SESSION_TYPE").as_deref() {
        Ok("x11") => "X11",
        Ok("wayland") => "Wayland",
        _ => "unknown",
    };
    let mut lines = vec![
        format!("Ubrus {}", env!("CARGO_PKG_VERSION")),
        format!("Session: {session}"),
        format!(
            "Package: {}",
            if std::env::var_os("SNAP").is_some() {
                "Snap"
            } else {
                "native"
            }
        ),
        format!("Extension connected: {}", status.connected),
        format!("Wallpaper: {}", Locale::English.get(wallpaper_status(profile, status))),
        format!("Displays: {}", status.outputs.len()),
    ];
    for (index, output) in status.outputs.iter().enumerate() {
        let reason = match output.reason.as_str() {
            reason @ ("manual" | "power-saver" | "overview" | "covered" | "visible" | "static") => reason,
            _ => "unknown",
        };
        lines.push(format!(
            "Display {}: {}×{}, {reason}",
            index + 1,
            output.width,
            output.height
        ));
    }
    if let Some(streams) = status.engine["streams"].as_array() {
        for (index, stream) in streams.iter().enumerate() {
            let count = |field: &str| {
                stream[field]
                    .as_u64()
                    .map_or_else(|| "unknown".into(), |v| v.to_string())
            };
            lines.push(format!(
                "Stream {}: {} decoded, {} rendered, {} dropped",
                index + 1,
                count("decoded"),
                count("rendered"),
                count("dropped")
            ));
        }
    }
    lines.join("\n")
}

pub fn settings(locale: Locale, p: &library::Profile, status: &super::control::Status) -> String {
    let on = |flag: bool| {
        if flag {
            locale.get(Text::ReportOn)
        } else {
            locale.get(Text::ReportOff)
        }
    };
    let mut lines = vec![
        format!(
            "{} {}",
            locale.get(Text::ReportSession),
            std::env::var("XDG_SESSION_TYPE").unwrap_or_default()
        ),
        format!("{} — {}", locale.get(Text::ReportShellExtension), on(status.connected)),
        format!("{} — {}", locale.get(Text::ReportRestoreAtLogin), on(p.login_enabled())),
        format!("{} — {}", locale.get(Text::ReportManualPause), on(p.paused)),
        format!("{} — {}", locale.get(Text::ReportFollowPowerSaver), on(p.power)),
    ];
    let assignment = |screen: &str| {
        if let Some(choice) = p.screens.get(screen).or_else(|| p.screens.get(ALL)) {
            let fit = if choice.fit {
                locale.get(Text::Fit)
            } else {
                locale.get(Text::Fill)
            };
            format!("{} · {fit}", p.items[&choice.id].name)
        } else {
            locale.get(Text::ReportSystemWallpaper).to_string()
        }
    };
    if p.screens.contains_key(ALL) {
        lines.push(format!("{} · {}", locale.get(Text::All), assignment(ALL)));
    }
    for m in &status.outputs {
        let reason = match m.reason.as_str() {
            "static" => locale.get(Text::ReportStillImage),
            "covered" => locale.get(Text::ReportCoveredByWindows),
            "power-saver" => locale.get(Text::ReportPowerSaver),
            "manual" => locale.get(Text::ReportPausedManually),
            "overview" => locale.get(Text::ReportPausedInOverview),
            _ => locale.get(Text::ReportVisible),
        };
        lines.push(format!(
            "{} · {}×{} · {} · {}",
            m.name,
            m.width,
            m.height,
            reason,
            assignment(&m.id)
        ));
    }
    for id in p
        .screens
        .keys()
        .filter(|id| *id != ALL && !status.outputs.iter().any(|m| m.id == **id))
    {
        lines.push(format!(
            "{} · {} · {}",
            id,
            locale.get(Text::ReportDisconnected),
            assignment(id)
        ));
    }
    if let Some(streams) = status.engine["streams"].as_array() {
        for s in streams {
            lines.push(format!(
                "{} {} · {} {} · {} {}",
                locale.get(Text::ReportDecoder),
                s["decoder"].as_str().unwrap_or("—"),
                locale.get(Text::ReportDecoded),
                s["decoded"],
                locale.get(Text::ReportDropped),
                s["dropped"]
                    .as_u64()
                    .map_or_else(|| "—".into(), |value| value.to_string())
            ));
        }
    }
    if !status.error.is_empty() {
        lines.push(status.error.clone());
    }
    lines.join("\n")
}

/// Write a new private file. Snap confinement or a missing Downloads directory may
/// require saving under the application's own data directory instead.
pub(super) fn save_report(
    report: &str,
    downloads: Option<&std::path::Path>,
    fallback: &std::path::Path,
) -> anyhow::Result<std::path::PathBuf> {
    use std::io::Write;
    let create = |dir: &std::path::Path| {
        tempfile::Builder::new()
            .prefix("ubrus-report-")
            .suffix(".txt")
            .tempfile_in(dir)
    };
    let mut file = match downloads.map(create) {
        Some(Ok(file)) => file,
        _ => {
            std::fs::create_dir_all(fallback)?;
            create(fallback)?
        }
    };
    file.write_all(report.as_bytes())?;
    file.as_file().sync_all()?;
    Ok(file.keep()?.1)
}

#[cfg(test)]
#[path = "../../tests/rust/diagnostics.rs"]
mod tests;
