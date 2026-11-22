//! Read-only widget inspection for the isolated X11 controller test.
use super::{Message, Preference, Tab, Ui};
use iced::{
    Event, Rectangle, Task, Vector,
    advanced::widget::{
        Id, Operation, operate,
        operation::{Focusable, Outcome, Scrollable},
    },
    event, keyboard, mouse, window,
};
use serde_json::{Value, json};
use std::{any::Any, collections::BTreeMap, path::PathBuf, sync::Mutex};

#[derive(Clone, Copy, Default, serde::Serialize)]
struct Pointer {
    position: Option<[f32; 2]>,
    pressed: bool,
}

static POINTER: Mutex<Pointer> = Mutex::new(Pointer {
    position: None,
    pressed: false,
});

pub(super) fn request(event: Event, _: event::Status, _: window::Id) -> Option<Message> {
    match event {
        Event::Mouse(mouse::Event::CursorMoved { position }) => {
            POINTER.lock().unwrap().position = Some([position.x, position.y]);
        }
        Event::Mouse(mouse::Event::CursorLeft) => POINTER.lock().unwrap().position = None,
        Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => POINTER.lock().unwrap().pressed = true,
        Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) => POINTER.lock().unwrap().pressed = false,
        Event::Window(window::Event::Unfocused) => *POINTER.lock().unwrap() = Pointer::default(),
        Event::Keyboard(keyboard::Event::KeyPressed {
            key: keyboard::Key::Named(keyboard::key::Named::F12),
            ..
        }) => return Some(Message::InspectWidgets),
        _ => {}
    }
    None
}

struct Control(String, bool);

pub(super) fn control(op: &mut dyn Operation, bounds: Rectangle, message: &Message, focused: bool) {
    let name = match message {
        Message::Import => "import",
        Message::Tab(Tab::Preferences) => "settings",
        Message::Tab(Tab::Library) => "library",
        Message::Set(Preference::Theme(_)) => "theme",
        Message::Set(Preference::Language(_)) => "language",
        Message::Set(Preference::Login(_)) => "login",
        Message::Set(Preference::Panel(_)) => "indicator",
        Message::Zoom(_) => "zoom",
        Message::Brightness(_) => "brightness",
        Message::Inspect => "refresh",
        Message::CopyPublicReport => "public",
        Message::CopyReport => "copy",
        Message::Export => "export",
        Message::ExportAs => "save-as",
        Message::Apply => "apply",
        Message::Back => "back",
        Message::ConfirmDelete(_) => "confirm-delete",
        Message::Select(id) => {
            op.custom(None, bounds, &mut Control(format!("tile-{id}"), focused));
            return;
        }
        _ => return,
    };
    op.custom(None, bounds, &mut Control(name.into(), focused));
}

#[derive(Default)]
struct Widgets {
    controls: BTreeMap<String, [f32; 4]>,
    text: Vec<String>,
    focus: Option<String>,
    focus_index: Option<usize>,
    focusables: usize,
    offset: Vector,
    clip: Rectangle,
    scroll: Option<(Vector, Rectangle)>,
}

impl Widgets {
    fn record(&mut self, name: &str, bounds: Rectangle, focused: bool) {
        if let Some(r) = (bounds - self.offset).intersection(&self.clip) {
            self.controls.insert(name.into(), [r.x, r.y, r.width, r.height]);
            if focused {
                self.focus = Some(name.into());
            }
        }
    }
}

impl Operation<Value> for Widgets {
    fn traverse(&mut self, visit: &mut dyn FnMut(&mut dyn Operation<Value>)) {
        let previous = (self.offset, self.clip);
        if let Some((offset, clip)) = self.scroll.take() {
            self.offset = offset;
            self.clip = clip;
        }
        visit(self);
        (self.offset, self.clip) = previous;
    }
    fn scrollable(&mut self, _: Option<&Id>, bounds: Rectangle, _: Rectangle, offset: Vector, _: &mut dyn Scrollable) {
        self.scroll = Some((
            self.offset + offset,
            self.clip.intersection(&(bounds - self.offset)).unwrap_or_default(),
        ));
    }
    fn custom(&mut self, _: Option<&Id>, bounds: Rectangle, state: &mut dyn Any) {
        if let Some(Control(name, focused)) = state.downcast_ref() {
            self.record(name, bounds, *focused);
        }
    }
    fn container(&mut self, id: Option<&Id>, bounds: Rectangle) {
        if id == Some(&Id::new("preview")) {
            self.record("preview", bounds, false);
        }
    }
    fn focusable(&mut self, id: Option<&Id>, bounds: Rectangle, state: &mut dyn Focusable) {
        if state.is_focused() {
            self.focus_index = Some(self.focusables);
        }
        self.focusables += 1;
        if id == Some(&Id::new("report")) {
            self.record("report", bounds, state.is_focused());
        }
    }
    fn text(&mut self, _: Option<&Id>, bounds: Rectangle, text: &str) {
        if (bounds - self.offset).intersection(&self.clip).is_some() {
            self.text.push(text.into());
        }
    }
    fn finish(&self) -> Outcome<Value> {
        Outcome::Some(
            json!({"controls": self.controls, "focus": self.focus, "focus_index": self.focus_index, "text": self.text}),
        )
    }
}

pub(super) fn inspect(ui: &Ui) -> Task<Message> {
    let Some(dir) = std::env::var_os("UBRUS_TEST_WIDGETS").map(PathBuf::from) else {
        return Task::none();
    };
    let Ok(request) = std::fs::read_to_string(dir.join("request")) else {
        return Task::none();
    };
    let size = ui.size;
    let info = json!({
        "request": request, "size": [size.width, size.height], "busy": ui.busy, "selected": ui.selected,
        "theme": ui.data.profile.theme, "language": ui.data.profile.language,
        "indicator": !ui.data.profile.hide_indicator, "login": ui.data.profile.login_enabled(),
        "report_selection": ui.report.content.selection(), "pointer": *POINTER.lock().unwrap(),
    });
    operate(Widgets {
        clip: Rectangle::with_size(size),
        ..Widgets::default()
    })
    .map(move |widgets| {
        let mut info = info.clone();
        info["widgets"] = widgets;
        let out = dir.join("reply.tmp");
        std::fs::write(&out, info.to_string())
            .and_then(|()| std::fs::rename(&out, dir.join("reply")))
            .expect("write X11 test widget snapshot");
        Message::Idle
    })
}
