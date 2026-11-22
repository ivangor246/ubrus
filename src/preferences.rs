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
pub enum Language {
    #[default]
    #[serde(rename = "system")]
    System,
    #[serde(untagged)]
    Selected(Locale),
}

macro_rules! locales {
    ($($variant:ident => ($tag:literal, $stored:literal, $name:literal)),+ $(,)?) => {
        #[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
        pub enum Locale {
            $(#[serde(rename = $stored)] $variant),+
        }
        impl Locale {
            const ALL: &'static [Self] = &[$(Self::$variant),+];
            pub fn tag(self) -> &'static str {
                match self { $(Self::$variant => $tag),+ }
            }
        }
        impl Language {
            pub fn options(locale: Locale) -> [(Self, &'static str); 1 + Locale::ALL.len()] {
                [
                    (Self::System, locale.get(Text::SystemLanguage)),
                    $((Self::Selected(Locale::$variant), $name)),+
                ]
            }
        }
    };
}

// The two original choices keep the spelling used in existing profiles.
locales! {
    English => ("en", "english", "English"),
    Russian => ("ru", "russian", "Русский"),
    Czech => ("cs", "cs", "Čeština"),
    German => ("de", "de", "Deutsch"),
    Indonesian => ("id", "id", "Bahasa Indonesia"),
    Italian => ("it", "it", "Italiano"),
    Dutch => ("nl", "nl", "Nederlands"),
    Portuguese => ("pt-PT", "pt-PT", "Português (Portugal)"),
    Turkish => ("tr", "tr", "Türkçe"),
    Japanese => ("ja", "ja", "日本語"),
    Korean => ("ko", "ko", "한국어"),
    SimplifiedChinese => ("zh-Hans", "zh-Hans", "简体中文"),
    TraditionalChinese => ("zh-Hant", "zh-Hant", "繁體中文"),
}

impl Locale {
    pub fn percent(self, value: u16) -> String {
        match self {
            Self::Turkish => format!("%{value}"),
            Self::Czech | Self::German | Self::Russian => format!("{value}\u{a0}%"),
            _ => format!("{value}%"),
        }
    }

    pub fn speed(self, hundredths: u16) -> String {
        let value = (f32::from(hundredths) / 100.).to_string();
        match self {
            Self::English | Self::Japanese | Self::Korean | Self::SimplifiedChinese | Self::TraditionalChinese => {
                format!("{value}×")
            }
            _ => format!("{}×", value.replace('.', ",")),
        }
    }

    fn detect<'a>(languages: impl IntoIterator<Item = &'a str>) -> Self {
        for language in languages {
            let language = language.to_ascii_lowercase();
            let (name, modifier) = language.split_once('@').unwrap_or((&language, ""));
            let mut parts = name.split('.').next().unwrap_or_default().split(['-', '_']);
            match parts.next().unwrap_or_default() {
                "c" | "posix" => return Self::English,
                "zh" => {
                    let mut regional = Self::SimplifiedChinese;
                    for part in parts {
                        match part {
                            "hans" => return Self::SimplifiedChinese,
                            "hant" => return Self::TraditionalChinese,
                            "tw" | "hk" | "mo" => regional = Self::TraditionalChinese,
                            _ => {}
                        }
                    }
                    return match modifier {
                        "hant" | "traditional" => Self::TraditionalChinese,
                        "hans" | "simplified" => Self::SimplifiedChinese,
                        _ => regional,
                    };
                }
                "pt" => return Self::Portuguese,
                "in" => return Self::Indonesian,
                base => {
                    if let Some(&locale) = Self::ALL.iter().find(|locale| locale.tag() == base) {
                        return locale;
                    }
                }
            }
        }
        Self::English
    }
}

impl Language {
    pub fn locale(self) -> Locale {
        match self {
            Self::Selected(locale) => locale,
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
    Offline, RendererError, RendererWaiting, NoWallpaper, StillWallpaper, PowerPaused, Covered, OverviewPaused,
    Search, NoMatches, ReplacesDisplays, CopyPublicReport, PublicReportCopied, CopyCommand, CommandCopied,
    GuideDeb, GuideStandalone, GuideSnap, CancelImport, ImportStopped, ImportSummary,
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
    ReportDecoded, ReportDropped, ReportPackage, ReportNative, ReportUnknown, ReportWallpaper, ReportDisplays, ReportStream, ReportRendered,
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
