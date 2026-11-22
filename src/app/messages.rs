use super::control::Status;
use crate::{
    library::{Choice, Profile, Store},
    preferences::{Appearance, Language, Locale, Text},
};
use ashpd::WindowIdentifier;

#[derive(Clone, Debug)]
pub struct Snapshot {
    pub profile: Profile,
    pub status: Status,
    pub store: Store,
    pub writable: bool,
    pub epoch: u64,
}

#[derive(Clone, Debug)]
pub enum Event {
    Open(Snapshot),
    Status(Status),
    Progress(u8),
    Updates,
    Done {
        from_panel: bool,
        profile: Profile,
        feedback: Option<Feedback>,
        selected: Option<String>,
    },
    Log(String),
    ReportSaved(std::path::PathBuf),
    Shutdown,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Feedback {
    Message(Text),
    ImportBatch {
        imported: usize,
        failed: usize,
        skipped: usize,
        stopped: Option<Text>,
    },
}

impl Feedback {
    pub fn key(self) -> Text {
        match self {
            Self::Message(text) => text,
            Self::ImportBatch { .. } => Text::ImportSummary,
        }
    }

    pub fn localize(self, locale: Locale) -> String {
        match self {
            Self::Message(text) => locale.get(text).into(),
            Self::ImportBatch {
                imported,
                failed,
                skipped,
                stopped,
            } => {
                let summary = locale
                    .get(Text::ImportSummary)
                    .replace("{added}", &imported.to_string())
                    .replace("{failed}", &failed.to_string())
                    .replace("{skipped}", &skipped.to_string());
                match stopped {
                    Some(reason) => format!("{} {summary}", locale.get(reason)),
                    None => summary,
                }
            }
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub enum Preference {
    Theme(Appearance),
    Language(Language),
    Login(bool),
    Panel(bool),
    Power(bool),
    Pause(bool),
    PauseInOverview(bool),
}

impl Preference {
    pub fn apply(self, profile: &mut Profile) {
        match self {
            Self::Theme(value) => profile.theme = value,
            Self::Language(value) => profile.language = value,
            Self::Panel(value) => profile.hide_indicator = !value,
            Self::Login(value) => profile.autostart = Some(value),
            Self::Power(value) => profile.power = value,
            Self::Pause(value) => profile.paused = value,
            Self::PauseInOverview(value) => profile.pause_in_overview = value,
        }
    }
}

#[derive(Clone, Debug)]
pub enum Action {
    Import,
    Delete(String),
    Apply(String, Choice),
    Visible(bool),
    Enabled(bool),
    Export,
    ExportAs,
}
pub enum Command {
    CancelImport,
    Panel(Box<Command>),
    Close(u64),
    Set(Preference),
    Action(Action, Option<WindowIdentifier>, (i32, i32)),
    Inspect,
    Open,
    Restart,
    Error(String),
}

impl Action {
    pub fn failure_message(&self) -> Text {
        match self {
            Self::Import => Text::FailedImport,
            Self::Delete(_) => Text::FailedDelete,
            Self::Apply(..) | Self::Visible(true) => Text::FailedApply,
            Self::Visible(false) => Text::FailedClear,
            Self::Enabled(_) => Text::FailedSave,
            Self::Export | Self::ExportAs => Text::FailedExport,
        }
    }
}
