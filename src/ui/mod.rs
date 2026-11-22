mod artwork;
mod displays;
mod focus;
mod preview;
mod report;
mod runtime;
mod style;
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
    SystemAppearance(portal::Appearance),
    Tab(Tab),
    Select(String),
    Monitor(Display),
    Fit(bool),
    Preview,
    Frame(u64, Option<preview::Frame>),
    Set(Preference),
    Import,
    Apply,
    Visible(bool),
    Enabled(bool),
    More,
    Focus(bool),
    Back,
    Resize(Size),
    DisplaySize(Size),
    PreviewMonitor(Display),
    Zoom(u16),
    Brightness(u16),
    Speed(u16),
    Reset,
    Drag(bool),
    Move(iced::Point, f64, f64),
    Nudge(i16, i16),
    Scrolled(iced::widget::operation::AbsoluteOffset),
    Restart,
    CheckUpdates,
    RequestDelete,
    AcceptDelete,
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
    system_appearance: portal::Appearance,
    commands: Sender<Command>,
    window: Option<window::Id>,
    tab: Tab,
    screen: String,
    selected: Option<String>,
    delete_confirmation: Option<String>,
    library_order: Vec<String>,
    thumbnails: std::collections::BTreeMap<String, image::Handle>,
    fit: bool,
    framing: crate::framing::Framing,
    size: Size,
    display_size: Size,
    preview_monitor: String,
    editing: bool,
    more: bool,
    offset: iced::widget::operation::AbsoluteOffset,
    drag: Option<(iced::Point, crate::framing::Framing)>,
    pointer: iced::Point,
    poster: Option<image::Handle>,
    adjusted: Option<image::Handle>,
    update_available: bool,
    shell_update: bool,
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
            icon: match artwork::brand() {
                image::Handle::Rgba {
                    width, height, pixels, ..
                } => window::icon::from_rgba(pixels.to_vec(), width, height).ok(),
                _ => None,
            },
            exit_on_close_request: false,
            platform_specific: window::settings::PlatformSpecific {
                application_id: BUS.into(),
                ..Default::default()
            },
            ..Default::default()
        });
        self.window = Some(id);
        task.map(|_| Message::Idle)
            .chain(window::monitor_size(id).map(|s| s.map_or(Message::Idle, Message::DisplaySize)))
    }

    fn stop(&mut self) {
        self.playback.take();
        self.frame.take();
        self.refresh_picture();
        self.generation = self.generation.wrapping_add(1);
    }

    fn sync_thumbnails(&mut self) {
        self.thumbnails.retain(|id, _| self.data.profile.items.contains_key(id));
        for id in &self.library_order {
            self.thumbnails.entry(id.clone()).or_insert_with(|| {
                // The service can unlink a deleted item before an outstanding UI frame is drawn.
                // Own validated PNG bytes until every widget using the thumbnail has gone away.
                std::fs::read(self.data.store.thumb(id))
                    .ok()
                    .and_then(|bytes| {
                        ::image::load_from_memory_with_format(&bytes, ::image::ImageFormat::Png).ok()?;
                        Some(image::Handle::from_bytes(bytes))
                    })
                    .unwrap_or_else(artwork::brand)
            });
        }
    }

    fn load_choice(&mut self) {
        let choice = self
            .data
            .profile
            .screens
            .get(&self.screen)
            .or_else(|| self.data.profile.screens.get(ALL))
            .filter(|c| Some(&c.id) == self.selected.as_ref());
        self.fit = choice.is_some_and(|c| c.fit);
        self.framing = choice.map(|c| c.frame).unwrap_or_default();
        self.drag = None;
        self.more = false;
        self.poster = self.selected.as_ref().and_then(|id| {
            let pixels = ::image::open(self.data.store.thumb(id)).ok()?.into_rgba8();
            Some(image::Handle::from_rgba(
                pixels.width(),
                pixels.height(),
                pixels.into_raw(),
            ))
        });
        self.refresh_picture();
    }

    fn refresh_picture(&mut self) {
        self.adjusted = self.frame.as_ref().or(self.poster.as_ref()).map(|handle| {
            if self.framing.brightness != 100
                && let image::Handle::Rgba {
                    width, height, pixels, ..
                } = handle
            {
                let mut pixels = pixels.to_vec();
                self.framing.brighten(&mut pixels, 4);
                image::Handle::from_rgba(*width, *height, pixels)
            } else {
                handle.clone()
            }
        });
    }

    fn check_updates(&mut self) {
        self.update_available = crate::desktop::updates::binary_changed();
        self.shell_update =
            self.data.status.connected && crate::desktop::updates::extension_changed(&self.data.status.extension_build);
    }

    fn play_preview(&mut self) -> Task<Message> {
        if let Some(id) = &self.selected
            && let Some(item) = self.data.profile.items.get(id)
            && item.animated()
        {
            self.playback = Some(preview::start(
                self.data.store.media(id, item),
                matches!(item.ext.as_str(), "gif" | "webp"),
                self.framing.speed,
            ));
            return self.next_frame();
        }
        Task::none()
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
        self.thumbnails.clear();
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
            Action::Export | Action::Delete(_) | Action::Visible(false) | Action::Enabled(false) => false,
            Action::Visible(true) | Action::Enabled(true) => self
                .data
                .profile
                .screens
                .get(ALL)
                .is_some_and(|c| !self.data.profile.items[&c.id].animated()),
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
        let size = if matches!(
            action,
            Action::Apply(..) | Action::Visible(true) | Action::Enabled(true)
        ) {
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
            Message::SystemAppearance(value) => self.system_appearance = value,
            Message::Event(event) => match *event {
                Event::Open(data) => {
                    self.library_order = data.profile.sorted_ids();
                    self.data = data;
                    self.sync_thumbnails();
                    return self.open();
                }
                Event::Updates => self.check_updates(),
                Event::Shutdown => return iced::exit(),
                Event::Status(status) => {
                    self.data.status = status;
                    if self.tab == Tab::Preferences {
                        self.check_updates();
                    }
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
                    from_panel,
                    profile,
                    selected,
                    feedback,
                } => {
                    let library_changed = profile.items.len() != self.library_order.len() || selected.is_some();
                    if library_changed {
                        self.library_order = profile.sorted_ids();
                    }
                    let removed = self.selected.as_ref().is_some_and(|id| !profile.items.contains_key(id));
                    if removed {
                        self.stop();
                        self.selected = self.library_order.first().cloned();
                    }
                    self.data.profile = profile;
                    if library_changed && self.window.is_some() {
                        self.sync_thumbnails();
                    }
                    self.feedback = feedback;
                    if let Some(id) = selected {
                        self.stop();
                        self.selected = Some(id);
                        self.tab = Tab::Library;
                        self.editing = true;
                        self.load_choice();
                    } else if removed {
                        self.load_choice();
                    }
                    if !from_panel {
                        self.progress = None;
                        self.busy = false;
                    }
                    if !self.busy && self.closing {
                        return self.close();
                    }
                }
            },
            Message::Tab(tab) => {
                self.delete_confirmation = None;
                self.stop();
                self.tab = tab;
                self.more = false;
                if tab == Tab::Preferences {
                    self.check_updates();
                }
                if tab == Tab::Diagnostics {
                    self.update_report(None);
                    let _ = self.commands.try_send(Command::Inspect);
                }
            }
            Message::Select(id) => {
                self.delete_confirmation = None;
                self.stop();
                self.selected = Some(id);
                self.editing = true;
                self.load_choice();
                self.feedback = None;
            }
            Message::Monitor(display) => {
                self.stop();
                self.screen = display.id;
                self.load_choice();
                self.feedback = None;
            }
            Message::Fit(value) => self.fit = value,
            Message::Set(pref) if self.data.writable && !self.busy => {
                self.busy = self.commands.try_send(Command::Set(pref)).is_ok();
            }
            Message::Preview => {
                if self.playback.is_some() {
                    self.stop();
                } else {
                    return self.play_preview();
                }
            }
            Message::Frame(generation, frame) if generation == self.generation => match frame {
                Some(Ok(frame)) => {
                    self.frame = Some(frame);
                    self.refresh_picture();
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
            Message::RequestDelete if self.tab == Tab::Library && !self.busy && self.data.writable => {
                self.delete_confirmation.clone_from(&self.selected);
            }
            Message::AcceptDelete if let Some(id) = self.delete_confirmation.clone() => {
                return self.update(Message::ConfirmDelete(id));
            }
            Message::CancelDelete => self.delete_confirmation = None,
            Message::ConfirmDelete(id)
                if self.delete_confirmation.as_ref() == Some(&id) && self.selected.as_ref() == Some(&id) =>
            {
                self.stop();
                return self.action(Action::Delete(id));
            }
            Message::Visible(v) => return self.action(Action::Visible(v)),
            Message::Enabled(v) => return self.action(Action::Enabled(v)),
            Message::Resize(size) => self.size = size,
            Message::DisplaySize(size) => self.display_size = size,
            Message::PreviewMonitor(m) => self.preview_monitor = m.id,
            Message::Focus(back) => {
                return if back {
                    iced::widget::operation::focus_previous()
                } else {
                    iced::widget::operation::focus_next()
                };
            }
            Message::More => self.more = !self.more,
            Message::Back if self.delete_confirmation.is_some() => self.delete_confirmation = None,
            Message::Back => {
                self.stop();
                self.editing = false;
                self.more = false;
                self.tab = Tab::Library;
                return iced::widget::operation::scroll_to("library", self.offset);
            }
            Message::Scrolled(offset) => self.offset = offset,
            Message::Zoom(v) => self.framing.zoom = v,
            Message::Brightness(v) => {
                self.framing.brightness = v;
                self.refresh_picture();
            }
            Message::Speed(v) => {
                self.framing.speed = v;
                if let Some(player) = &self.playback {
                    player.set_speed(self.framing.speed);
                }
            }
            Message::Reset => {
                self.framing = Default::default();
                self.fit = false;
                self.refresh_picture();
                if let Some(player) = &self.playback {
                    player.set_speed(self.framing.speed);
                }
            }
            Message::Drag(pressed) => self.drag = pressed.then_some((self.pointer, self.framing)),
            Message::Move(point, dx, dy) => {
                self.pointer = point;
                if let Some((origin, frame)) = self.drag {
                    let position = |p, delta: f32, travel: f64| {
                        if travel.abs() < 0.5 {
                            p
                        } else {
                            (f64::from(p) + f64::from(delta) * 100. / travel)
                                .round()
                                .clamp(0., 100.) as u16
                        }
                    };
                    self.framing.x = position(frame.x, point.x - origin.x, dx);
                    self.framing.y = position(frame.y, point.y - origin.y, dy);
                }
            }
            Message::Nudge(x, y) if self.tab == Tab::Library && (self.editing || self.size.width >= 840.) => {
                self.framing.x = (self.framing.x as i16 + x).clamp(0, 100) as u16;
                self.framing.y = (self.framing.y as i16 + y).clamp(0, 100) as u16;
            }
            Message::CheckUpdates => self.check_updates(),
            Message::Restart if !self.busy && self.update_available => {
                let _ = self.commands.try_send(Command::Restart);
            }
            Message::Apply if self.available() && !self.data.profile.disabled => {
                if let Some(id) = &self.selected {
                    return self.action(Action::Apply(
                        self.screen.clone(),
                        Choice {
                            id: id.clone(),
                            fit: self.fit,
                            frame: self.framing,
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
