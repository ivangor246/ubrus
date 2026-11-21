use super::{Message, Tab, Ui};
use crate::{
    app::{Command, Event, Snapshot},
    desktop::portal,
    library::ALL,
    preferences::{Appearance, Text},
};
use async_channel::{Receiver, Sender};
use iced::{Subscription, Task, Theme, keyboard, window};

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
                system_dark: None,
                commands: commands.clone(),
                window: None,
                tab: Tab::Library,
                screen: ALL.into(),
                selected,
                delete_confirmation: None,
                library_order,
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
            let task = Task::batch([ui.open(), Task::run(events.clone(), |e| Message::Event(Box::new(e)))]);
            (ui, task)
        },
        Ui::update,
        Ui::view,
    )
    .executor::<UiExecutor>()
    .title("Ubrus")
    .theme(|ui: &Ui, _| match ui.data.profile.theme {
        Appearance::System => ui.system_dark.map(|dark| if dark { Theme::Dark } else { Theme::Light }),
        Appearance::Light => Some(Theme::Light),
        Appearance::Dark => Some(Theme::Dark),
    })
    .subscription(|ui| {
        Subscription::batch([
            if ui.window.is_some() {
                Subscription::run(portal::system_dark).map(Message::SystemDark)
            } else {
                Subscription::none()
            },
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
            return Some(Message::Tab(Tab::Library));
        }
        if !modifiers.control() {
            return None;
        }

        return match key.as_ref() {
            keyboard::Key::Character("o") | keyboard::Key::Character("щ") => Some(Message::Import),
            keyboard::Key::Named(keyboard::key::Named::Enter) => Some(Message::Apply),
            keyboard::Key::Character(",") | keyboard::Key::Character("б") => Some(Message::Tab(Tab::Preferences)),
            _ => None,
        };
    }
    None
}
