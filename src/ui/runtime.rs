use super::{Message, Tab, Ui};
use crate::{
    app::{Command, Event, Snapshot},
    desktop::portal,
    library::ALL,
    preferences::Text,
};
use async_channel::{Receiver, Sender};
use iced::{Subscription, Task, keyboard, window};

// UI futures wait on channels and portals. Decoding and file I/O have separate workers.
struct UiExecutor(iced::futures::executor::ThreadPool);
impl iced::Executor for UiExecutor {
    fn new() -> std::io::Result<Self> {
        iced::futures::executor::ThreadPoolBuilder::new()
            .pool_size(1)
            .name_prefix("ubrus-ui-")
            .create()
            .map(Self)
    }
    fn spawn(&self, future: impl Future<Output = ()> + Send + 'static) {
        self.0.spawn_ok(future);
    }
    fn block_on<T>(&self, future: impl Future<Output = T>) -> T {
        iced::futures::executor::block_on(future)
    }
}

pub fn run(initial: Snapshot, events: Receiver<Event>, commands: Sender<Command>) -> anyhow::Result<()> {
    iced::daemon(
        move || {
            let library_order = initial.profile.sorted_ids();
            let selected = initial
                .profile
                .screens
                .get(ALL)
                .map(|c| c.id.clone())
                .or_else(|| library_order.first().cloned());
            let mut ui = Ui {
                data: initial.clone(),
                system_appearance: Default::default(),
                commands: commands.clone(),
                window: None,
                tab: Tab::Library,
                screen: ALL.into(),
                selected,
                delete_confirmation: None,
                library_order,
                thumbnails: Default::default(),
                framing: Default::default(),
                size: iced::Size::new(900., 600.),
                display_size: iced::Size::new(1920., 1080.),
                preview_monitor: String::new(),
                editing: false,
                more: false,
                offset: Default::default(),
                drag: None,
                pointer: iced::Point::ORIGIN,
                poster: None,
                adjusted: None,
                update_available: false,
                shell_update: false,
                fit: initial.profile.screens.get(ALL).is_some_and(|c| c.fit),
                feedback: (!initial.writable).then_some(Text::Damaged),
                report: Default::default(),
                saved_report: None,
                busy: false,
                progress: None,
                closing: false,
                generation: 0,
                playback: None,
                frame: None,
            };
            ui.sync_thumbnails();
            ui.load_choice();
            let task = Task::batch([ui.open(), Task::run(events.clone(), |e| Message::Event(Box::new(e)))]);
            (ui, task)
        },
        Ui::update,
        Ui::view,
    )
    .executor::<UiExecutor>()
    .title("Ubrus")
    .theme(|ui: &Ui, _| super::style::theme(ui.dark(), ui.system_appearance.high_contrast))
    .subscription(|ui| {
        Subscription::batch([
            if ui.window.is_some() {
                Subscription::run(portal::system_appearance).map(Message::SystemAppearance)
            } else {
                Subscription::none()
            },
            window::resize_events().map(|(_, size)| Message::Resize(size)),
            window::events()
                .filter_map(|(_, event)| matches!(event, window::Event::Focused).then_some(Message::CheckUpdates)),
            window::close_requests().map(Message::Close),
            keyboard::listen().filter_map(shortcut),
        ])
    })
    .run()?;
    Ok(())
}

fn shortcut(event: keyboard::Event) -> Option<Message> {
    if let keyboard::Event::KeyPressed { key, modifiers, .. } = event {
        if key == keyboard::Key::Named(keyboard::key::Named::Escape) {
            return Some(Message::Back);
        }
        if key == keyboard::Key::Named(keyboard::key::Named::Tab) {
            return Some(Message::Focus(modifiers.shift()));
        }
        if modifiers.is_empty() {
            match key.as_ref() {
                keyboard::Key::Named(keyboard::key::Named::Delete) => return Some(Message::RequestDelete),
                keyboard::Key::Named(keyboard::key::Named::Enter) => return Some(Message::AcceptDelete),
                _ => {}
            }
        }
        if modifiers.shift() {
            use keyboard::key::Named;
            match key.as_ref() {
                keyboard::Key::Named(Named::ArrowLeft) => return Some(Message::Nudge(-2, 0)),
                keyboard::Key::Named(Named::ArrowRight) => return Some(Message::Nudge(2, 0)),
                keyboard::Key::Named(Named::ArrowUp) => return Some(Message::Nudge(0, -2)),
                keyboard::Key::Named(Named::ArrowDown) => return Some(Message::Nudge(0, 2)),
                _ => {}
            }
        }
        if !modifiers.control() {
            return None;
        }

        return match key.as_ref() {
            keyboard::Key::Character("d") | keyboard::Key::Character("в") => Some(Message::Tab(Tab::Diagnostics)),
            keyboard::Key::Character("o") | keyboard::Key::Character("щ") => Some(Message::Import),
            keyboard::Key::Named(keyboard::key::Named::Enter) => Some(Message::Apply),
            keyboard::Key::Character(",") | keyboard::Key::Character("б") => Some(Message::Tab(Tab::Preferences)),
            _ => None,
        };
    }
    None
}
