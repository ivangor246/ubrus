use crate::{
    desktop::autostart::set_login,
    library::{Choice, Profile, Store},
};
use anyhow::{Context, Result};
use gtk::{gio, prelude::*};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

pub const BUS: &str = "org.ubrus.Ubrus";
const PATH: &str = "/org/ubrus/Ubrus/Wallpaper";
const IFACE: &str = "org.ubrus.Ubrus.Wallpaper1";
const XML: &str = r#"<node><interface name="org.ubrus.Ubrus.Wallpaper1">
<method name="Panel"><arg type="s" direction="in"/></method>
<method name="Scene"><arg type="s" direction="out"/></method>
<method name="Status"><arg type="s" direction="out"/></method><method name="Refresh"/>
<method name="Report"><arg type="s" direction="in"/></method>
<signal name="Changed"/><signal name="Inspect"/>
</interface></node>"#;

#[derive(Default, Clone, Debug, Deserialize, Serialize)]
#[serde(default)]
pub struct Output {
    pub id: String,
    pub name: String,
    pub reason: String,
    pub width: i32,
    pub height: i32,
}
#[derive(Default, Clone, Debug, Deserialize, Serialize)]
#[serde(default)]
pub struct Status {
    pub connected: bool,
    pub preview_visible: Option<bool>,
    pub outputs: Vec<Output>,
    pub engine: serde_json::Value,
    pub error: String,
    pub applied: u64,
    pub failed: u64,
    pub extension_build: String,
    pub framing: bool,
}
struct Pending {
    serial: u64,
    screen: String,
    profile: Profile,
    ack: Option<futures_channel::oneshot::Sender<Result<(), String>>>,
}
pub struct Control {
    pending: RefCell<Option<Pending>>,
    serial: Cell<u64>,
    peer: RefCell<String>,
    watcher: RefCell<Option<Box<dyn FnOnce()>>>,
    pub store: Store,
    pub profile: RefCell<Profile>,
    pub status: RefCell<Status>,
    pub notify: RefCell<Option<Box<dyn Fn()>>>,
    app: gio::Application,
    hold: RefCell<Option<gio::ApplicationHoldGuard>>,
    registration: RefCell<Option<gio::RegistrationId>>,
    native_launcher: Option<tempfile::TempDir>,
    pub writable: bool,
    settings_open: Cell<bool>,
    _writer: Option<std::fs::File>,
}

fn native_launcher() -> Result<Option<tempfile::TempDir>> {
    if std::env::var_os("SNAP").is_some() {
        return Ok(None);
    }
    let exe = std::env::current_exe()?;
    if exe == std::path::Path::new("/usr/bin/ubrus") {
        return Ok(None);
    }
    // Shell accepts only a host runtime-directory entry, never an executable path from a scene.
    let dir = tempfile::Builder::new()
        .prefix("ubrus-native-")
        .tempdir_in(gtk::glib::user_runtime_dir())?;
    std::os::unix::fs::symlink(exe, dir.path().join("ubrus"))?;
    Ok(Some(dir))
}

impl Control {
    pub fn attach(app: &gio::Application, panel: async_channel::Sender<super::Command>) -> Result<Rc<Self>> {
        let store = Store::open()?;
        let native_launcher = native_launcher().unwrap_or_else(|error| {
            store.log(&format!("Cannot register the native renderer: {error:#}"));
            None
        });
        let writer = store.try_writer();
        let (profile, writable, error) = match store.read_profile() {
            Ok((mut p, true)) if matches!(writer, Ok(Some(_))) => {
                if let Err(e) = crate::desktop::background::recover(store.root(), p.system_wallpaper.as_ref()) {
                    store.log(&format!("Wallpaper recovery: {e:#}"));
                }
                if let Err(e) = store.cleanup_imports(&p) {
                    store.log(&format!("Interrupted import cleanup: {e:#}"));
                }
                if let Err(e) = store.finish_deletions(&mut p) {
                    store.log(&format!("Library cleanup: {e:#}"));
                }
                (p, true, String::new())
            }
            Ok((p, false)) => (p, false, String::from("Unsupported saved profile version")),
            Ok((p, true)) => {
                let reason = match &writer {
                    Err(error) => format!("Cannot lock the wallpaper library: {error:#}"),
                    _ => String::from(
                        "Another login session is using the wallpaper library. Reopen Ubrus after it exits.",
                    ),
                };
                (p, false, reason)
            }
            Err(e) => (Profile::default(), false, format!("{e:#}")),
        };
        if !writable && !error.is_empty() {
            store.log(&error);
        }
        let this = Rc::new(Self {
            pending: RefCell::default(),
            serial: Cell::new(0),
            peer: RefCell::default(),
            watcher: RefCell::default(),
            store,
            profile: profile.into(),
            status: Status {
                error,
                ..Status::default()
            }
            .into(),
            app: app.clone(),
            hold: RefCell::default(),
            registration: RefCell::default(),
            native_launcher,
            notify: RefCell::default(),
            writable,
            settings_open: Cell::new(false),
            _writer: writer.ok().flatten().filter(|_| writable),
        });
        let bus = app.dbus_connection().context("A graphical session bus is required")?;
        let info = gio::DBusNodeInfo::for_xml(XML)?.lookup_interface(IFACE).unwrap();
        let weak = Rc::downgrade(&this);
        let appeared = weak.clone();
        let vanished = weak.clone();
        let watcher = gio::bus_watch_name_on_connection(
            &bus,
            "org.gnome.Shell",
            gio::BusNameWatcherFlags::NONE,
            move |_, _, owner| {
                if let Some(c) = appeared.upgrade() {
                    *c.peer.borrow_mut() = owner.into();
                    c.signal("Changed");
                }
            },
            move |_, _| {
                if let Some(c) = vanished.upgrade() {
                    c.peer.borrow_mut().clear();
                    c.disconnected();
                }
            },
        );
        *this.watcher.borrow_mut() = Some(Box::new(move || gio::bus_unwatch_name(watcher)));
        let reg = bus
            .register_object(PATH, &info)
            .method_call(move |_, sender, _, _, name, args, reply| {
                let Some(c) = weak.upgrade() else {
                    reply.return_dbus_error("org.ubrus.Closed", "Closing");
                    return;
                };
                match name {
                    "Scene" if !c.writable => {
                        reply.return_dbus_error("org.ubrus.InvalidProfile", "Invalid saved profile")
                    }
                    "Report" | "Panel" if sender != Some(&*c.peer.borrow()) => {
                        reply.return_dbus_error("org.ubrus.WrongPeer", "Expected GNOME Shell")
                    }
                    "Panel" => {
                        use super::{Action, Command, Preference};
                        let name = args.get::<(String,)>().map(|(v,)| v).unwrap_or_default();
                        let size = c
                            .status
                            .borrow()
                            .outputs
                            .first()
                            .map_or((1920, 1080), |m| (m.width, m.height));
                        let command = match name.as_str() {
                            "open" => Command::Open,
                            "pause" | "resume" => Command::Set(Preference::Pause(name == "pause")),
                            "hide-icon" => Command::Set(Preference::Panel(false)),
                            "enable" | "disable" => Command::Action(Action::Enabled(name == "enable"), None, size),
                            "show" | "hide" => Command::Action(Action::Visible(name == "show"), None, size),
                            _ => {
                                reply.return_dbus_error("org.ubrus.UnknownCommand", "Unknown panel action");
                                return;
                            }
                        };
                        if panel.try_send(super::Command::Panel(Box::new(command))).is_ok() {
                            reply.return_value(None);
                        } else {
                            reply.return_dbus_error("org.ubrus.Closed", "Controller closing");
                        }
                    }
                    "Scene" => reply.return_value(Some(&(c.scene().to_string(),).to_variant())),
                    "Status" => reply.return_value(Some(
                        &(serde_json::to_string(&*c.status.borrow()).unwrap(),).to_variant(),
                    )),
                    "Refresh" => {
                        c.signal("Inspect");
                        reply.return_value(None);
                    }
                    "Report" => {
                        if let Some((wire,)) = args.get::<(String,)>()
                            && wire.len() < 128 * 1024
                            && let Ok(status) = serde_json::from_str::<Status>(&wire)
                        {
                            c.accept_report(status);
                        }
                        reply.return_value(None);
                    }
                    _ => reply.return_dbus_error("org.ubrus.UnknownMethod", name),
                }
            })
            .build()?;
        *this.registration.borrow_mut() = Some(reg);
        this.signal("Changed");
        Ok(this)
    }
    fn accept_report(&self, status: Status) {
        if status.error != self.status.borrow().error && !status.error.is_empty() {
            self.store.log(&status.error);
        }
        if let Some(p) = self.pending.borrow_mut().as_mut() {
            let failed = !status.connected || status.failed == p.serial;
            if (failed || status.applied == p.serial)
                && let Some(tx) = p.ack.take()
            {
                let _ = tx.send(if failed {
                    Err(if status.error.is_empty() {
                        "Shell disconnected".into()
                    } else {
                        status.error.clone()
                    })
                } else {
                    Ok(())
                });
            }
        }
        *self.status.borrow_mut() = status;
        if let Some(notify) = self.notify.borrow().as_ref() {
            notify();
        }
    }

    fn disconnected(&self) {
        *self.status.borrow_mut() = Status::default();
        if let Some(p) = self.pending.borrow_mut().as_mut()
            && let Some(tx) = p.ack.take()
        {
            let _ = tx.send(Err("Shell disconnected".into()));
        }
        if let Some(notify) = self.notify.borrow().as_ref() {
            notify();
        }
    }
    pub async fn apply(&self, screen: String, choice: Choice) -> Result<()> {
        let mut profile = self.profile.borrow().clone();
        profile.assign(&screen, choice)?;
        let staged = self.store.stage_profile(&profile)?;
        self.publish(screen, profile, staged).await
    }

    pub async fn publish(&self, screen: String, profile: Profile, staged: tempfile::NamedTempFile) -> Result<()> {
        anyhow::ensure!(self.pending.borrow().is_none(), "Another change is in progress");
        anyhow::ensure!(
            self.status.borrow().framing || profile.screens.values().all(|c| c.frame == Default::default()),
            "Extension update required"
        );
        let serial = self.serial.get() + 1;
        self.serial.set(serial);
        let (tx, rx) = futures_channel::oneshot::channel();
        *self.pending.borrow_mut() = Some(Pending {
            serial,
            screen,
            profile: profile.clone(),
            ack: Some(tx),
        });
        self.signal("Changed");
        let confirmed = gtk::glib::future_with_timeout(std::time::Duration::from_secs(18), rx).await;
        self.pending.borrow_mut().take();
        let result = (|| {
            confirmed??.map_err(anyhow::Error::msg)?;
            self.commit_staged(profile, staged)
        })();
        if result.is_err() {
            self.signal("Changed");
        }
        result
    }
    fn scene(&self) -> serde_json::Value {
        let saved = self.profile.borrow();
        let pending = self.pending.borrow();
        let serial = pending.as_ref().map_or(0, |t| t.serial);
        let p = pending.as_ref().map_or(&*saved, |t| &t.profile);
        json!({"version":1, "serial":serial, "target":pending.as_ref().map(|t| &t.screen),
            "settings_pid":self.settings_open.get().then(std::process::id), "profile_path":self.store.profile_path(), "snap":std::env::var_os("SNAP").is_some(),
            "native_launcher":self.native_launcher.as_ref().and_then(|dir| dir.path().file_name()?.to_str()),
            "wake_dir":std::env::var_os("SNAP_USER_COMMON").map(std::path::PathBuf::from),
            "screens":self.store.screen_paths(p, false), "paused":p.paused, "power":p.power,
            "panel":p.panel(), "pause_in_overview":p.pause_in_overview})
    }
    pub fn settings_open(&self, open: bool) {
        if self.settings_open.replace(open) != open {
            self.signal("Changed");
        }
    }
    pub fn signal(&self, name: &str) {
        if std::env::var_os("SNAP").is_some() {
            // The strict D-Bus slot accepts calls from Shell. Use inotify to wake Shell without an outgoing signal.
            let file = if name == "Inspect" {
                "shell-inspect"
            } else {
                "shell-changed"
            };
            match std::fs::write(self.store.root().join(file), b"1") {
                Ok(()) => return,
                Err(e) => self.store.log(&format!("Cannot notify Shell: {e}")),
            }
        }
        if let Some(bus) = self.app.dbus_connection() {
            let _ = bus.emit_signal(None, PATH, IFACE, name, None);
        }
    }
    pub fn commit(&self, edit: impl FnOnce(&mut Profile) -> Result<()>) -> Result<()> {
        self.save(edit)?;
        self.signal("Changed");
        Ok(())
    }
    pub fn save(&self, edit: impl FnOnce(&mut Profile) -> Result<()>) -> Result<()> {
        anyhow::ensure!(self.writable, "Repair the saved profile before changing settings");
        let mut p = self.profile.borrow().clone();
        edit(&mut p)?;
        let staged = self.store.stage_profile(&p)?;
        self.save_staged(p, staged)
    }
    pub fn commit_staged(&self, p: Profile, staged: tempfile::NamedTempFile) -> Result<()> {
        self.save_staged(p, staged)?;
        self.signal("Changed");
        Ok(())
    }
    fn save_staged(&self, p: Profile, staged: tempfile::NamedTempFile) -> Result<()> {
        anyhow::ensure!(self.writable, "Read-only saved profile");
        let old = self.profile.borrow().login_enabled();
        if old != p.login_enabled() {
            set_login(p.login_enabled())?;
        }
        if let Err(err) = self.store.publish_profile(staged) {
            if old != p.login_enabled() {
                let _ = set_login(old);
            }
            return Err(err);
        }
        *self.profile.borrow_mut() = p;
        self.keep_alive();
        Ok(())
    }
    pub fn import(&self, prepared: crate::library::PreparedImport) -> Result<String> {
        anyhow::ensure!(self.writable, "Repair the saved profile before importing");
        self.store.commit_import(&mut self.profile.borrow_mut(), prepared)
    }

    pub async fn finish_wallpaper_change(&self) -> Result<bool> {
        let library = self.store.clone();
        let current = self.profile.borrow().system_wallpaper.clone();
        gio::spawn_blocking(move || crate::desktop::background::recover(library.root(), current.as_ref()))
            .await
            .map_err(|_| anyhow::anyhow!("Wallpaper recovery worker failed"))?
    }

    pub fn restart(&self) {
        if crate::desktop::updates::binary_changed() {
            crate::desktop::updates::request_restart();
            self.app.quit();
        }
    }
    pub fn hold(&self) -> gio::ApplicationHoldGuard {
        self.app.hold()
    }
    pub fn keep_alive(&self) {
        let mut hold = self.hold.borrow_mut();
        let p = self.profile.borrow();
        if !p.hide_indicator || (p.visible() && p.has_animation()) {
            hold.get_or_insert_with(|| self.app.hold());
        } else {
            hold.take();
        }
    }
}
impl Drop for Control {
    fn drop(&mut self) {
        if let Some(unwatch) = self.watcher.take() {
            unwatch();
        }
        if let Some(id) = self.registration.take()
            && let Some(bus) = self.app.dbus_connection()
        {
            let _ = bus.unregister_object(id);
        }
    }
}
