mod displays;
mod preview;
mod report;
mod runtime;
mod view;
pub use runtime::run;

use crate::{
    app::{Action, Command, Event, Preference, Snapshot, control::BUS},
    desktop::portal,
    library::{ALL, Choice},
    preferences::Text,
};
use async_channel::Sender;
use displays::Display;
use iced::{Size, Task, widget::image, window};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Tab {
    Library,
    Preferences,
    Diagnostics,
}

#[derive(Clone, Debug)]
enum Message {
    Event(Box<Event>),
    SystemDark(bool),
    Tab(Tab),
    Select(String),
    Monitor(Display),
    Fit(bool),
    Preview,
    Frame(u64, Option<preview::Frame>),
    Set(Preference),
    Import,
    Apply,
    ClearWallpapers,
    RequestDelete,
    CancelDelete,
    ConfirmDelete(String),
    Export,
    ExportAs,
    ReportInteraction(iced::widget::text_editor::Action),
    CopyReport,
    Inspect,
    Close(window::Id),
    Dispatch(Result<(), String>, Text),
    Idle,
}

struct Ui {
    data: Snapshot,
    system_dark: Option<bool>,
    commands: Sender<Command>,
    window: Option<window::Id>,
    tab: Tab,
    screen: String,
    selected: Option<String>,
    delete_confirmation: Option<String>,
    library_order: Vec<String>,
    fit: bool,
    feedback: Option<Text>,
    report: report::ReportView,
    saved_report: Option<std::path::PathBuf>,
    busy: bool,
    progress: Option<u8>,
    closing: bool,
    generation: u64,
    playback: Option<preview::Playback>,
    frame: Option<image::Handle>,
}

impl Ui {
    fn open(&mut self) -> Task<Message> {
        self.closing = false;
        if let Some(id) = self.window {
            return window::set_mode(id, window::Mode::Windowed).chain(window::gain_focus(id));
        }
        let (id, task) = window::open(window::Settings {
            size: Size::new(900., 600.),
            min_size: Some(Size::new(620., 430.)),
            exit_on_close_request: false,
            platform_specific: window::settings::PlatformSpecific {
                application_id: BUS.into(),
                ..Default::default()
            },
            ..Default::default()
        });
        self.window = Some(id);
        task.map(|_| Message::Idle)
    }

    fn stop(&mut self) {
        self.playback.take();
        self.frame.take();
        self.generation = self.generation.wrapping_add(1);
    }

    fn next_frame(&self) -> Task<Message> {
        let Some(playback) = &self.playback else {
            return Task::none();
        };
        let generation = self.generation;
        let frames = playback.frames.clone();
        Task::perform(async move { frames.recv().await.ok() }, move |frame| {
            Message::Frame(generation, frame)
        })
    }

    fn close(&mut self) -> Task<Message> {
        self.delete_confirmation = None;
        self.stop();
        let Some(id) = self.window else { return Task::none() };
        if self.busy {
            self.closing = true;
            // Portals need a live, visible parent until the interaction finishes.
            return Task::none();
        }
        self.window = None;
        self.closing = false;
        let _ = self.commands.try_send(Command::Close(self.data.epoch));
        window::close(id)
    }

    fn action(&mut self, action: Action) -> Task<Message> {
        let Some(id) = self.window else { return Task::none() };
        let fallback = action.failure_message();
        if self.busy || (!self.data.writable && !matches!(action, Action::Export | Action::ExportAs)) {
            return Task::none();
        }
        self.busy = true;
        self.delete_confirmation = None;
        self.feedback = None;
        let commands = self.commands.clone();
        let needs_parent = match &action {
            Action::Export => false,
            Action::Delete(id) => {
                std::env::var_os("SNAP").is_some() && self.data.profile.restores_desktop_on_delete(id)
            }
            Action::ClearWallpapers => {
                self.data.profile.system_wallpaper.is_some() && std::env::var_os("SNAP").is_some()
            }
            Action::Apply(screen, choice) => {
                screen == ALL
                    && !self
                        .data
                        .profile
                        .items
                        .get(&choice.id)
                        .is_some_and(|item| item.animated())
            }
            Action::Import | Action::ExportAs => true,
        };
        if !needs_parent {
            let result = commands
                .try_send(Command::Action(action, None, (0, 0)))
                .map_err(|e| e.to_string());
            return Task::done(Message::Dispatch(result, fallback));
        }
        let size = if matches!(action, Action::Apply(..)) {
            window::scale_factor(id).then(move |scale| {
                window::monitor_size(id).map(move |size| {
                    size.map_or((1920, 1080), |s| {
                        ((s.width * scale).round() as i32, (s.height * scale).round() as i32)
                    })
                })
            })
        } else {
            Task::done((0, 0))
        };
        size.then(move |size| {
            let commands = commands.clone();
            let action = action.clone();
            window::run(id, move |w| {
                let parent = portal::parent(w).map_err(|e| format!("{e:#}"))?;
                commands
                    .try_send(Command::Action(action, parent, size))
                    .map_err(|e| e.to_string())
            })
        })
        .map(move |r| Message::Dispatch(r, fallback))
    }

    fn update_report(&mut self, log: Option<String>) {
        let locale = self.data.profile.language.locale();
        let mut summary = crate::app::diagnostics::settings(locale, &self.data.profile, &self.data.status);
        if !self.data.status.connected {
            summary.push_str("\n\n");
            summary.push_str(locale.get(Text::Guide));
        }
        if let Some(path) = &self.saved_report {
            summary = format!("{} · {}\n\n{summary}", locale.get(Text::Exported), path.display());
        }
        self.report.update(summary, log);
    }

    fn available(&self) -> bool {
        self.screen == ALL || self.data.status.outputs.iter().any(|m| m.id == self.screen)
    }
    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::SystemDark(dark) => self.system_dark = Some(dark),
            Message::Event(event) => match *event {
                Event::Open(data) => {
                    self.library_order = data.profile.sorted_ids();
                    self.data = data;
                    return self.open();
                }
                Event::Shutdown => return iced::exit(),
                Event::Status(status) => {
                    self.data.status = status;
                    if self.tab == Tab::Diagnostics && self.report.content.cursor().selection.is_none() {
                        self.update_report(None);
                    }
                }
                Event::Progress(n) => self.progress = Some(n),
                Event::Log(log) => self.update_report(Some(log)),
                Event::ReportSaved(path) => {
                    self.saved_report = Some(path);
                    self.update_report(None);
                }
                Event::Done {
                    profile,
                    selected,
                    feedback,
                } => {
                    if profile.items.len() != self.library_order.len() || selected.is_some() {
                        self.library_order = profile.sorted_ids();
                    }
                    if self.selected.as_ref().is_some_and(|id| !profile.items.contains_key(id)) {
                        self.stop();
                        self.selected = self.library_order.first().cloned();
                    }
                    self.data.profile = profile;
                    self.feedback = feedback;
                    if let Some(id) = selected {
                        self.stop();
                        self.selected = Some(id);
                        self.tab = Tab::Library;
                    }
                    self.progress = None;
                    self.busy = false;
                    if self.closing {
                        return self.close();
                    }
                }
            },
            Message::Tab(tab) => {
                self.delete_confirmation = None;
                self.stop();
                self.tab = tab;
                if tab == Tab::Diagnostics {
                    self.update_report(None);
                    let _ = self.commands.try_send(Command::Inspect);
                }
            }
            Message::Select(id) => {
                self.delete_confirmation = None;
                self.stop();
                self.selected = Some(id);
                self.feedback = None;
            }
            Message::Monitor(display) => {
                self.screen = display.id;
                self.feedback = None;
            }
            Message::Fit(value) => self.fit = value,
            Message::Set(pref) if self.data.writable && !self.busy => {
                self.busy = self.commands.try_send(Command::Set(pref)).is_ok();
            }
            Message::Preview => {
                if self.playback.is_some() {
                    self.stop();
                } else if let Some(id) = &self.selected
                    && let Some(item) = self.data.profile.items.get(id)
                    && item.animated()
                {
                    self.playback = Some(preview::start(self.data.store.media(id, item), item.ext == "gif"));
                    return self.next_frame();
                }
            }
            Message::Frame(generation, frame) if generation == self.generation => match frame {
                Some(Ok(frame)) => {
                    self.frame = Some(frame);
                    return self.next_frame();
                }
                Some(Err(e)) => {
                    self.stop();
                    self.feedback = Some(Text::FailedPreview);
                    let _ = self.commands.try_send(Command::Error(e));
                }
                None => self.stop(),
            },
            Message::Import => return self.action(Action::Import),
            Message::RequestDelete if !self.busy && self.data.writable => {
                self.delete_confirmation.clone_from(&self.selected);
            }
            Message::CancelDelete => self.delete_confirmation = None,
            Message::ConfirmDelete(id)
                if self.delete_confirmation.as_ref() == Some(&id) && self.selected.as_ref() == Some(&id) =>
            {
                self.stop();
                return self.action(Action::Delete(id));
            }
            Message::ClearWallpapers => {
                self.stop();
                return self.action(Action::ClearWallpapers);
            }
            Message::Apply if self.available() => {
                if let Some(id) = &self.selected {
                    return self.action(Action::Apply(
                        self.screen.clone(),
                        Choice {
                            id: id.clone(),
                            fit: self.fit,
                        },
                    ));
                }
            }
            Message::ReportInteraction(action) => self.report.interact(action),
            Message::CopyReport => return iced::clipboard::write(self.report.content.text()),
            Message::Export => return self.action(Action::Export),
            Message::ExportAs => return self.action(Action::ExportAs),
            Message::Inspect => {
                let _ = self.commands.try_send(Command::Inspect);
            }
            Message::Close(id) if Some(id) == self.window => return self.close(),
            Message::Dispatch(Err(e), fallback) => {
                let _ = self.commands.try_send(Command::Error(e));
                self.feedback = Some(fallback);
                self.busy = false;
                if self.closing {
                    return self.close();
                }
            }
            _ => {}
        }
        Task::none()
    }
}
