use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Appearance {
    #[default]
    System,
    Light,
    Dark,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Language {
    #[default]
    System,
    English,
    Russian,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Locale {
    English,
    Russian,
}
impl Locale {
    fn detect<'a>(languages: impl IntoIterator<Item = &'a str>) -> Self {
        for language in languages {
            let tag = language.split(['_', '-', '.', '@']).next().unwrap_or("");
            if tag.eq_ignore_ascii_case("ru") {
                return Self::Russian;
            }
            if tag.eq_ignore_ascii_case("en") || tag.eq_ignore_ascii_case("C") || tag == "POSIX" {
                return Self::English;
            }
        }
        Self::English
    }
    pub fn tag(self) -> &'static str {
        match self {
            Self::English => "en",
            Self::Russian => "ru",
        }
    }
}
impl Language {
    pub fn options(l: Locale) -> [(Self, &'static str); 3] {
        [
            (Self::System, l.get(Text::SystemLanguage)),
            (Self::English, "English"),
            (Self::Russian, "Русский"),
        ]
    }
    pub fn locale(self) -> Locale {
        match self {
            Self::English => Locale::English,
            Self::Russian => Locale::Russian,
            Self::System => {
                static LOCALE: std::sync::OnceLock<Locale> = std::sync::OnceLock::new();
                *LOCALE.get_or_init(|| Locale::detect(gtk::glib::language_names().iter().map(|s| s.as_str())))
            }
        }
    }
}

macro_rules! messages {
    ($($key:ident),+ $(,)?) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        pub enum Text { $($key),+ }
        impl Text {
            #[cfg(test)]
            pub const ALL: &'static [Self] = &[$(Self::$key),+];
            pub fn key(self) -> &'static str { match self { $(Self::$key => stringify!($key)),+ } } }
    }
}
messages! {
    OwnershipUnknown, Position, Disabled, Paused, Hidden, Running, Library, Preferences, Diagnostics, Import,
    Formats, Open, Save, Export, Empty, Preview, Stop, Screen, All, Disconnected, Scaling, Fill,
    Fit, Apply, ClearWallpapers, DeleteItem, Delete, Cancel, ConfirmDelete, ConfirmDeleteActive,
    Deleted, DeleteCleanup, FailedDelete, Appearance, Language, SystemTheme, SystemLanguage, Light,
    Dark, Login, Power, Pause, PauseInOverview, Imported, Applied, Exported, Damaged, FailedImport,
    FailedApply, FailedClear, FailedSave, FailedExport, FailedPreview, FailedService, DiskFull,
    Unsupported, EmptyFile, Missing, Extension, Declined, SaveAs, CopyAll, Refresh, Guide,
    ShowWallpaper, Resume, Enable, Disable, Panel, Zoom, Brightness, Speed, Reset, Back, More,
    PreviewOn, Update, ShellUpdate, Desktop, Playback, ReportOn, ReportOff, ReportSession,
    ReportShellExtension, ReportRestoreAtLogin, ReportManualPause, ReportFollowPowerSaver,
    ReportSystemWallpaper, ReportStillImage, ReportCoveredByWindows, ReportPowerSaver,
    ReportPausedManually, ReportPausedInOverview, ReportVisible, ReportDisconnected, ReportDecoder,
    ReportDecoded, ReportDropped,
}

impl Locale {
    pub fn get(self, id: Text) -> &'static str {
        use std::{collections::BTreeMap, sync::OnceLock};
        type Catalog = BTreeMap<String, BTreeMap<String, String>>;
        static CATALOG: OnceLock<Catalog> = OnceLock::new();
        let catalog = CATALOG.get_or_init(|| {
            serde_json::from_str(include_str!("../extension/locale.json")).expect("bundled translations")
        });
        &catalog[self.tag()][id.key()]
    }
}
#[cfg(test)]
#[path = "../tests/rust/preferences.rs"]
mod tests;
