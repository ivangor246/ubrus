use crate::{
    library::{self, ALL},
    preferences::Locale,
};
pub fn settings(locale: Locale, p: &library::Profile, status: &super::control::Status) -> String {
    let tr = |en, ru| locale.pair(en, ru);
    let on = |flag: bool| {
        if flag {
            tr("on", "вкл.")
        } else {
            tr("off", "выкл.")
        }
    };
    let mut lines = vec![
        format!(
            "{} {}",
            tr("Session", "Сеанс"),
            std::env::var("XDG_SESSION_TYPE").unwrap_or_default()
        ),
        format!(
            "{} — {}",
            tr("Shell extension", "Расширение GNOME"),
            on(status.connected)
        ),
        format!(
            "{} — {}",
            tr("Restore at login", "Запуск при входе"),
            on(p.autostart.unwrap_or(false))
        ),
        format!("{} — {}", tr("Manual pause", "Ручная пауза"), on(p.paused)),
        format!(
            "{} — {}",
            tr("Follow power saver", "Учитывать энергосбережение"),
            on(p.power)
        ),
    ];
    let assignment = |screen: &str| {
        if let Some(choice) = p.screens.get(screen).or_else(|| p.screens.get(ALL)) {
            let fit = if choice.fit {
                tr("Fit", "Вписать")
            } else {
                tr("Fill", "Заполнить")
            };
            format!("{} · {fit}", p.items[&choice.id].name)
        } else {
            tr("System wallpaper", "Системный фон").to_string()
        }
    };
    if p.screens.contains_key(ALL) {
        lines.push(format!("{} · {}", tr("All monitors", "Все мониторы"), assignment(ALL)));
    }
    for m in &status.outputs {
        let reason = match m.reason.as_str() {
            "static" => tr("still image", "статический фон"),
            "covered" => tr("covered by windows", "закрыт окнами"),
            "power-saver" => tr("power saver", "энергосбережение"),
            "manual" => tr("paused manually", "ручная пауза"),
            "overview" => tr("paused in overview", "пауза в обзоре GNOME"),
            _ => tr("visible", "виден"),
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
            tr("disconnected", "отключён"),
            assignment(id)
        ));
    }
    if let Some(streams) = status.engine["streams"].as_array() {
        for s in streams {
            lines.push(format!(
                "{} {} · {} {} · {} {}",
                tr("Decoder", "Декодер"),
                s["decoder"].as_str().unwrap_or("—"),
                tr("decoded", "декодировано"),
                s["decoded"],
                tr("dropped", "пропущено"),
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
