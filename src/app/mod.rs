mod actions;
pub mod control;
pub mod diagnostics;
mod import;
mod messages;
pub use messages::{Action, Command, Event, Feedback, Preference, Snapshot};

use crate::preferences::Text;
use actions::{finish, perform};
use async_channel::{Receiver, Sender};
use control::{BUS, Control};
use gtk::{gio, glib, prelude::*};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

#[derive(Default)]
struct SettingsSession {
    importing: RefCell<Option<gio::Cancellable>>,
    changes: iced::futures::lock::Mutex<()>,
    hold: RefCell<Option<gio::ApplicationHoldGuard>>,
    epoch: Cell<u64>,
}

impl SettingsSession {
    fn open(&self, control: &Control, events: &Sender<Event>) {
        self.hold.borrow_mut().get_or_insert_with(|| control.hold());
        control.keep_alive();
        control.settings_open(true);
        let epoch = self.epoch.get() + 1;
        self.epoch.set(epoch);
        let _ = events.try_send(Event::Open(Snapshot {
            profile: control.profile.borrow().clone(),
            status: control.status.borrow().clone(),
            store: control.store.clone(),
            writable: control.writable,
            epoch,
        }));
    }

    fn close(&self, control: &Control, epoch: u64) {
        if self.epoch.get() == epoch {
            self.hold.borrow_mut().take();
            control.settings_open(false);
        }
    }

    async fn dispatch(self: &Rc<Self>, control: &Rc<Control>, events: &Sender<Event>, command: Command) {
        let (command, from_panel) = match command {
            Command::Panel(command) => (*command, true),
            command => (command, false),
        };
        match command {
            Command::Panel(_) => unreachable!(),
            Command::CancelImport => {
                if let Some(cancel) = self.importing.borrow().as_ref() {
                    cancel.cancel();
                }
            }
            Command::Action(Action::Import, parent, _) => {
                if self.importing.borrow().is_some() {
                    return;
                }
                let cancel = gio::Cancellable::new();
                *self.importing.borrow_mut() = Some(cancel.clone());
                let control = control.clone();
                let events = events.clone();
                let session = self.clone();
                let hold = control.hold();
                glib::spawn_future_local(async move {
                    let _hold = hold;
                    let result = import::run(&control, &events, parent, &cancel, &session.changes).await;
                    session.importing.borrow_mut().take();
                    finish(&control, &events, result, Text::FailedImport, from_panel);
                });
            }
            Command::Open => self.open(control, events),
            Command::Restart => control.restart(),
            Command::Close(epoch) => self.close(control, epoch),
            Command::Inspect => {
                control.signal("Inspect");
                let _ = events.try_send(Event::Log(control.store.report("")));
            }
            Command::Error(message) => control.store.log(&message),
            Command::Set(preference) => {
                let _guard = self.changes.lock().await;
                let result = control.save(|profile| {
                    preference.apply(profile);
                    Ok(())
                });
                if result.is_ok() {
                    control.signal("Changed");
                }
                finish(
                    control,
                    events,
                    result.map(|()| (None, None)),
                    Text::FailedSave,
                    from_panel,
                );
            }
            Command::Action(action, parent, size) => {
                let _guard = self.changes.lock().await;
                let _hold = control.hold();
                let fallback = action.failure_message();
                let result = perform(control, events, action, parent, size)
                    .await
                    .map(|(message, selected)| (message.map(Feedback::Message), selected));
                finish(control, events, result, fallback, from_panel);
            }
        }
    }
}

pub fn run(events: Sender<Event>, commands: Receiver<Command>, panel: Sender<Command>) -> glib::ExitCode {
    glib::set_application_name("Ubrus");
    let app = gio::Application::builder()
        .application_id(BUS)
        .flags(gio::ApplicationFlags::HANDLES_COMMAND_LINE)
        .build();
    app.add_main_option(
        "background",
        glib::Char::from(0),
        glib::OptionFlags::NONE,
        glib::OptionArg::None,
        "Restore the wallpaper at login",
        None,
    );
    let state = Rc::new(RefCell::new(None));
    let window = Rc::new(SettingsSession::default());
    let init = state.clone();
    let active = window.clone();
    let ready = events.clone();
    app.connect_startup(move |app| {
        let c = match Control::attach(app, panel.clone()) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("Ubrus: {e:#}");
                return;
            }
        };
        let weak = Rc::downgrade(&c);
        let output = ready.clone();
        let active_window = active.clone();
        *c.notify.borrow_mut() = Some(Box::new(move || {
            if active_window.hold.borrow().is_some()
                && let Some(c) = weak.upgrade()
            {
                let _ = output.try_send(Event::Status(c.status.borrow().clone()));
            }
        }));
        let weak = Rc::downgrade(&c);
        *init.borrow_mut() = Some(c);
        let monitors = crate::desktop::updates::watch(ready.clone());
        let commands = commands.clone();
        let events = ready.clone();
        let window = active.clone();
        let app = app.downgrade();
        glib::spawn_future_local(async move {
            let _monitors = monitors;
            while let Ok(request) = commands.recv().await {
                let Some(c) = weak.upgrade() else { break };
                window.dispatch(&c, &events, request).await;
            }
            if let Some(app) = app.upgrade() {
                app.quit();
            }
        });
    });
    let shutdown = state.clone();
    app.connect_shutdown(move |_| {
        shutdown.borrow_mut().take();
    });
    let output = events.clone();
    app.connect_command_line(move |_, line| {
        let Some(c) = state.borrow().clone() else {
            return glib::ExitCode::FAILURE;
        };
        if line.options_dict().contains("background") {
            if c.profile.borrow().login_enabled() {
                c.keep_alive();
            }
        } else {
            window.open(&c, &output);
        }
        glib::ExitCode::SUCCESS
    });
    let result = app.run();
    let _ = events.try_send(Event::Shutdown);
    result
}
