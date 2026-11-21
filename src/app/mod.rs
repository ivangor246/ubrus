mod actions;
pub mod control;
pub mod diagnostics;
mod messages;
pub use messages::{Action, Command, Event, Preference, Snapshot};

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
    hold: RefCell<Option<gio::ApplicationHoldGuard>>,
    epoch: Cell<u64>,
}

impl SettingsSession {
    fn open(&self, control: &Control, events: &Sender<Event>) {
        self.hold.borrow_mut().get_or_insert_with(|| control.hold());
        control.keep_alive();
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

    fn close(&self, epoch: u64) {
        if self.epoch.get() == epoch {
            self.hold.borrow_mut().take();
        }
    }

    async fn dispatch(&self, control: &Control, events: &Sender<Event>, command: Command) {
        match command {
            Command::Close(epoch) => self.close(epoch),
            Command::Inspect => {
                control.signal("Inspect");
                let _ = events.try_send(Event::Log(control.store.report("")));
            }
            Command::Error(message) => control.store.log(&message),
            Command::Set(preference) => {
                let result = control.save(|profile| {
                    preference.apply(profile);
                    Ok(())
                });
                if result.is_ok() && preference.affects_playback() {
                    control.signal("Changed");
                }
                finish(control, events, result.map(|()| (None, None)), Text::FailedSave);
            }
            Command::Action(action, parent, size) => {
                let _hold = control.hold();
                let fallback = action.failure_message();
                let result = perform(control, events, action, parent, size).await;
                finish(control, events, result, fallback);
            }
        }
    }
}

pub fn run(events: Sender<Event>, commands: Receiver<Command>) -> glib::ExitCode {
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
        let c = match Control::attach(app) {
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
        let commands = commands.clone();
        let events = ready.clone();
        let window = active.clone();
        let app = app.downgrade();
        glib::spawn_future_local(async move {
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
            if c.profile.borrow().autostart == Some(true) {
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
