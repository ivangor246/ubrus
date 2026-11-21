use crate::{
    library::{self, ALL},
    preferences::{Locale, Text},
};
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
        format!(
            "{} — {}",
            locale.get(Text::ReportRestoreAtLogin),
            on(p.autostart.unwrap_or(false))
        ),
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
