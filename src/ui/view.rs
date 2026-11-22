use super::{Message, Tab, Ui, artwork, displays::monitors, focus, style};
use crate::{
    app::Preference,
    library::ALL,
    preferences::{Appearance, Language, Locale, Text},
};
use iced::{
    Alignment, Color, ContentFit, Element, Fill, Rectangle, Theme,
    widget::{
        button, checkbox, column, container, grid, image, mouse_area, pick_list, pin, progress_bar, responsive, row,
        rule, scrollable, slider, space, text, text_editor, text_input,
    },
    window,
};

type View<'a> = Element<'a, Message>;

impl Ui {
    pub(super) fn dark(&self) -> bool {
        match self.data.profile.theme {
            Appearance::Dark => true,
            Appearance::Light => false,
            Appearance::System => self.system_appearance.dark,
        }
    }

    fn command<'a>(&self, label: Text, icon: &'static str, action: Option<Message>) -> View<'a> {
        let locale = self.data.profile.language.locale();
        let b = button(
            row![artwork::symbol(icon, 18.), text(locale.get(label)).size(14)]
                .spacing(8)
                .align_y(Alignment::Center),
        )
        .padding([8, 12])
        .style(style::subtle)
        .on_press_maybe(action.clone());
        focus::action(b, action)
    }

    pub(super) fn view(&self, _: window::Id) -> View<'_> {
        let locale = self.data.profile.language.locale();
        let p = &self.data.profile;
        if let Some(id) = &self.delete_confirmation
            && let Some(item) = p.items.get(id)
        {
            let question = if p.screens.values().any(|c| &c.id == id) {
                Text::ConfirmDeleteActive
            } else {
                Text::ConfirmDelete
            };
            let cancel = Message::CancelDelete;
            let delete = Message::ConfirmDelete(id.clone());
            let dialog = column![
                filename(&item.name, 21, 30),
                text(locale.get(question)).size(16),
                row![
                    focus::action(
                        button(locale.get(Text::Cancel))
                            .style(style::subtle)
                            .on_press(cancel.clone()),
                        Some(cancel)
                    ),
                    focus::action(
                        button(locale.get(Text::Delete))
                            .style(style::danger)
                            .on_press(delete.clone()),
                        Some(delete)
                    )
                ]
                .spacing(10)
            ]
            .spacing(16);
            return container(
                container(dialog)
                    .padding(24)
                    .max_width(420)
                    .style(container::rounded_box),
            )
            .center(Fill)
            .into();
        }
        let label = crate::app::diagnostics::wallpaper_status(p, &self.data.status);
        let color: fn(&Theme) -> Color = match label {
            Text::Running => |theme: &Theme| theme.palette().success,
            Text::RendererError => |theme: &Theme| theme.palette().danger,
            Text::RendererWaiting | Text::Offline => |theme: &Theme| theme.palette().warning,
            _ => style::muted,
        };
        let mut header = row![
            image(artwork::brand()).width(36).height(36),
            text("Ubrus")
                .font(iced::Font {
                    weight: iced::font::Weight::Semibold,
                    ..Default::default()
                })
                .size(25),
            text(locale.get(label)).size(14).style(move |theme| text::Style {
                color: Some(color(theme))
            }),
            space::horizontal()
        ]
        .spacing(9)
        .align_y(Alignment::Center);
        for (tab, label, icon) in [
            (Tab::Library, Text::Library, "library"),
            (Tab::Preferences, Text::Preferences, "settings"),
        ] {
            if tab != self.tab {
                header = header.push(self.command(label, icon, Some(Message::Tab(tab))));
            }
        }
        let content = match self.tab {
            Tab::Library => responsive(move |area| self.library(locale, area.width)).into(),
            Tab::Preferences => self.settings(locale),
            Tab::Diagnostics => self.diagnostics(locale),
        };
        let mut page = column![header, rule::horizontal(1), content].spacing(12);
        if let Some(n) = self.progress {
            page = page.push(progress_bar(0.0..=100., n.into()).girth(4));
        }
        if self.importing {
            page = page.push(self.command(
                Text::CancelImport,
                "back",
                (!self.import_cancel_requested).then_some(Message::CancelImport),
            ));
        }
        if let Some(feedback) = self.feedback.or_else(|| {
            (!self.data.status.error.is_empty()).then_some(crate::app::Feedback::Message(Text::FailedService))
        }) {
            let msg = feedback.key();
            let mut notice = row![text(feedback.localize(locale)).size(13)]
                .align_y(Alignment::Center)
                .spacing(8);
            if self.tab != Tab::Diagnostics
                && (self.feedback.is_none()
                    || matches!(feedback, crate::app::Feedback::ImportBatch { failed, .. } if failed > 0)
                    || matches!(
                        msg,
                        Text::Extension
                            | Text::FailedApply
                            | Text::FailedImport
                            | Text::FailedSave
                            | Text::FailedPreview
                            | Text::FailedDelete
                            | Text::FailedExport
                            | Text::FailedClear
                            | Text::Damaged
                            | Text::ShellUpdate
                    ))
            {
                notice = notice.push(self.command(Text::Diagnostics, "report", Some(Message::Tab(Tab::Diagnostics))));
            }
            page = page.push(notice);
        }
        container(page).padding([14, 20]).width(Fill).height(Fill).into()
    }

    fn diagnostics(&self, l: Locale) -> View<'_> {
        let action = |label, message: Option<Message>| {
            focus::action(
                button(l.get(label))
                    .style(style::primary)
                    .on_press_maybe(message.clone()),
                message,
            )
        };
        column![
            row![
                text(l.get(Text::Diagnostics)).size(23),
                space::horizontal(),
                action(Text::CopyPublicReport, Some(Message::CopyPublicReport))
            ]
            .spacing(8)
            .align_y(Alignment::Center),
            row![
                action(Text::Refresh, Some(Message::Inspect)),
                action(Text::CopyAll, Some(Message::CopyReport)),
                action(Text::Export, (!self.busy).then_some(Message::Export)),
                action(Text::SaveAs, (!self.busy).then_some(Message::ExportAs))
            ]
            .spacing(8)
            .wrap(),
            text_editor(&self.report.content)
                .on_action(Message::ReportInteraction)
                .font(iced::Font::MONOSPACE)
                .size(14)
                .height(Fill)
        ]
        .spacing(12)
        .height(Fill)
        .into()
    }

    fn settings(&self, l: Locale) -> View<'_> {
        let p = &self.data.profile;
        let t = |k| l.get(k);
        let edit = self.data.writable && !self.busy;
        let theme_options = [
            (Appearance::System, t(Text::SystemTheme)),
            (Appearance::Light, t(Text::Light)),
            (Appearance::Dark, t(Text::Dark)),
        ];
        let language_options = Language::options(l);
        let picker = |key, menu: View<'static>| column![text(t(key)).size(14), menu].spacing(5);
        let theme = labeled_picker(theme_options, p.theme, |v| Message::Set(Preference::Theme(v)));
        let language = labeled_picker(language_options, p.language, |v| Message::Set(Preference::Language(v)));
        let mut desktop = column![
            text(t(Text::Desktop)).size(20),
            self.command(
                if p.disabled { Text::Enable } else { Text::Disable },
                "power",
                edit.then_some(Message::Enabled(p.disabled))
            ),
            toggle(p.login_enabled(), t(Text::Login), edit && !p.disabled, |v| {
                Message::Set(Preference::Login(v))
            }),
            toggle(!p.hide_indicator, t(Text::Panel), edit, |v| Message::Set(
                Preference::Panel(v)
            ))
        ]
        .spacing(10);
        if !p.screens.is_empty() {
            desktop = desktop.push(self.visibility(edit));
        }
        let playback = column![
            text(t(Text::Playback)).size(20),
            self.command(
                if p.paused { Text::Resume } else { Text::Pause },
                if p.paused { "play" } else { "pause" },
                (edit && p.visible() && p.has_animation()).then_some(Message::Set(Preference::Pause(!p.paused)))
            ),
            toggle(p.power, t(Text::Power), edit, |v| Message::Set(Preference::Power(v))),
            toggle(p.pause_in_overview, t(Text::PauseInOverview), edit, |v| Message::Set(
                Preference::PauseInOverview(v)
            ))
        ]
        .spacing(10);
        let settings = column![
            row![picker(Text::Appearance, theme), picker(Text::Language, language)]
                .spacing(20)
                .wrap(),
            rule::horizontal(1),
            desktop,
            rule::horizontal(1),
            playback,
            rule::horizontal(1),
            self.command(Text::Diagnostics, "report", Some(Message::Tab(Tab::Diagnostics)))
        ]
        .spacing(16);
        let mut notice = column![].spacing(8);
        if self.update_available {
            notice = notice.push(self.command(Text::Update, "update", (!self.busy).then_some(Message::Restart)));
        }
        if self.shell_update || !self.data.status.connected {
            notice = notice.push(self.extension_guide(l));
        }
        scrollable(container(notice.push(settings)).max_width(680).padding(12))
            .height(Fill)
            .into()
    }

    fn visibility<'a>(&self, edit: bool) -> View<'a> {
        let p = &self.data.profile;
        self.command(
            if p.hidden {
                Text::ShowWallpaper
            } else {
                Text::ClearWallpapers
            },
            if p.hidden { "show" } else { "hide" },
            (edit && !p.disabled && !p.screens.is_empty()).then_some(Message::Visible(p.hidden)),
        )
    }

    fn library(&self, l: Locale, width: f32) -> View<'_> {
        let edit = self.data.writable && !self.busy;
        let import = self.command(Text::Import, "add", edit.then_some(Message::Import));
        let p = &self.data.profile;
        if self.library_order.is_empty() {
            return container(
                column![
                    image(artwork::brand()).width(84).height(84),
                    text(l.get(Text::Empty)).size(24),
                    import
                ]
                .spacing(16)
                .align_x(Alignment::Center),
            )
            .center(Fill)
            .into();
        }
        // The page has 20 logical pixels of padding on each side.
        let wide = width >= 800.;
        if !wide && self.editing {
            return self.editor(l, false);
        }
        let page = self.gallery_page();
        let mut tiles = grid([])
            .columns(page.columns)
            .spacing(super::gallery::GAP)
            .height(iced::Length::Shrink);
        for id in &self.matching[page.first..page.end] {
            let item = &p.items[id];
            let mut caption = row![filename(&item.name, 14, 21)].spacing(4);
            if p.visible() && p.screens.values().any(|c| &c.id == id) {
                caption = caption.push(artwork::symbol("check", 16.));
            }
            let tile = column![
                iced::widget::stack![
                    space().width(Fill).height(Fill),
                    image(self.thumbnails.get(id).cloned().unwrap_or_else(artwork::brand))
                        .content_fit(ContentFit::Cover)
                        .width(Fill)
                        .height(Fill)
                        .border_radius(6)
                ]
                .clip(true),
                caption
            ]
            .spacing(7);
            let selected = self.selected.as_ref() == Some(id);
            let select = Message::Select(id.clone());
            tiles = tiles.push(focus::named_action(
                id.clone(),
                button(tile)
                    .padding(7)
                    .width(Fill)
                    .height(super::gallery::TILE_HEIGHT)
                    .clip(true)
                    .style(move |theme, status| {
                        let mut s = style::subtle(theme, status);
                        s.border = iced::Border::default()
                            .rounded(10)
                            .width(if selected { 2 } else { 1 })
                            .color(if selected {
                                theme.palette().primary
                            } else {
                                theme.extended_palette().background.strong.color
                            });
                        s
                    })
                    .on_press(select.clone()),
                select,
            ));
        }
        let gallery = column![
            row![text(l.get(Text::Library)).size(23), space::horizontal(), import].align_y(Alignment::Center),
            text_input(l.get(Text::Search), &self.search).on_input(Message::Search),
            scrollable(if self.matching.is_empty() {
                column![text(l.get(Text::NoMatches)).size(15)]
            } else {
                column![space().height(page.before), tiles, space().height(page.after)]
            })
            .id("library")
            .on_scroll(|v| Message::Scrolled(v.absolute_offset(), v.bounds().height))
            .height(Fill)
        ]
        .spacing(12)
        .width(Fill)
        .height(Fill);
        if wide && self.selected.is_some() {
            row![
                gallery,
                rule::vertical(1),
                container(self.editor(l, true)).width(((width + 40.) * 0.39).clamp(352., 540.))
            ]
            .spacing(15)
            .height(Fill)
            .into()
        } else {
            gallery.into()
        }
    }

    pub(super) fn extension_command(&self) -> &'static str {
        if std::env::var_os("SNAP").is_some() {
            "gnome-extensions install --force /snap/ubrus/current/share/ubrus/ubrus-extension.zip"
        } else if crate::desktop::updates::installed_executable().is_none() {
            "gnome-extensions install --force ./ubrus-extension.zip"
        } else {
            "gnome-extensions enable wallpaper@ubrus.org"
        }
    }

    fn extension_guide(&self, locale: Locale) -> View<'_> {
        let snap = std::env::var_os("SNAP").is_some();
        let installed = crate::desktop::updates::installed_executable().is_some();
        let guide = if snap {
            Text::GuideSnap
        } else if !installed {
            Text::GuideStandalone
        } else if self.shell_update {
            Text::ShellUpdate
        } else {
            Text::GuideDeb
        };
        let mut controls = column![text(locale.get(guide)).size(14)].spacing(4);
        if snap || !installed || !self.shell_update {
            controls = controls
                .push(text(self.extension_command()).font(iced::Font::MONOSPACE).size(12))
                .push(self.command(Text::CopyCommand, "report", Some(Message::CopyExtensionCommand)));
        }
        controls.into()
    }

    fn editor(&self, l: Locale, wide: bool) -> View<'_> {
        let Some(item) = self.selected.as_ref().and_then(|id| self.data.profile.items.get(id)) else {
            return space::vertical().into();
        };
        let t = |k| l.get(k);
        let edit = self.data.writable && !self.busy;
        let mut heading = row![].spacing(8).align_y(Alignment::Center);
        if !wide {
            heading = heading.push(self.command(Text::Back, "back", Some(Message::Back)));
        }
        heading = heading.push(filename(&item.name, 19, 28));
        let (options, selected) = monitors(l, &self.data.status.outputs, &self.screen);
        let mut controls = column![
            self.preview_view(),
            row![
                column![
                    text(t(Text::Screen)).size(13),
                    display_picker(options, selected, Message::Monitor)
                ]
                .spacing(4)
                .width(Fill),
                column![
                    text(t(Text::Scaling)).size(13),
                    labeled_picker([(false, t(Text::Fill)), (true, t(Text::Fit))], self.fit, Message::Fit)
                ]
                .spacing(4)
            ]
            .spacing(9)
        ]
        .spacing(12);
        let outputs = &self.data.status.outputs;
        if self.screen == ALL && outputs.len() > 1 {
            let id = outputs
                .iter()
                .find(|m| m.id == self.preview_monitor)
                .unwrap_or(&outputs[0])
                .id
                .as_str();
            let (mut opts, selected) = monitors(l, outputs, id);
            opts.retain(|m| m.id != ALL);
            controls = controls.push(
                column![
                    text(t(Text::PreviewOn)).size(13),
                    display_picker(opts, selected, Message::PreviewMonitor)
                ]
                .spacing(4),
            );
        }
        fn setting<'a>(
            label: &'static str,
            current: u16,
            range: std::ops::RangeInclusive<u16>,
            step: u16,
            value: String,
            set: fn(u16) -> Message,
        ) -> View<'a> {
            let forward = set(current.saturating_add(step).min(*range.end()));
            let backward = set(current.saturating_sub(step).max(*range.start()));
            focus::cycle(
                column![
                    row![text(label).size(14), space::horizontal(), text(value).size(14)],
                    slider(range, current, set).style(style::slider_rail).step(step)
                ]
                .spacing(3),
                Some(forward),
                Some(backward),
            )
        }
        let frame = self.framing;
        controls = controls
            .push(setting(
                t(Text::Zoom),
                frame.zoom,
                100..=400,
                1,
                l.percent(frame.zoom),
                Message::Zoom,
            ))
            .push(setting(
                t(Text::Brightness),
                frame.brightness,
                0..=150,
                1,
                l.percent(frame.brightness),
                Message::Brightness,
            ));
        if item.animated() {
            controls = controls
                .push(setting(
                    t(Text::Speed),
                    frame.speed,
                    25..=400,
                    25,
                    l.speed(frame.speed),
                    Message::Speed,
                ))
                .push(self.command(
                    if self.playback.is_some() {
                        Text::Stop
                    } else {
                        Text::Preview
                    },
                    if self.playback.is_some() { "pause" } else { "play" },
                    Some(Message::Preview),
                ));
        }
        let needs_extension = item.animated() || self.screen != ALL;
        let mut menu = column![].spacing(3);
        if (needs_extension && !self.data.status.connected) || self.shell_update {
            menu = menu.push(self.extension_guide(l));
        }
        if self.screen == ALL && self.data.profile.screens.keys().any(|id| id != ALL) {
            menu = menu.push(text(l.get(Text::ReplacesDisplays)).size(14));
        }
        if self.more {
            menu = menu
                .push(self.command(Text::Reset, "reset", Some(Message::Reset)))
                .push(self.visibility(edit))
                .push(self.command(Text::DeleteItem, "trash", edit.then_some(Message::RequestDelete)));
        }
        let footer: View<'_> = row![
            self.command(Text::More, "more", Some(Message::More)),
            space::horizontal(),
            {
                let press = (edit
                    && !self.data.profile.disabled
                    && self.available()
                    && (!needs_extension || self.data.status.connected))
                    .then_some(Message::Apply);
                focus::action(
                    button(t(Text::Apply))
                        .style(style::primary)
                        .padding([9, 18])
                        .on_press_maybe(press.clone()),
                    press,
                )
            }
        ]
        .align_y(Alignment::Center)
        .into();
        column![
            heading,
            scrollable(container(controls).padding([2, 5]).width(Fill))
                .spacing(2)
                .height(Fill),
            rule::horizontal(1),
            menu.push(footer)
        ]
        .spacing(8)
        .height(Fill)
        .into()
    }

    fn preview_view(&self) -> View<'_> {
        let id = if self.screen == ALL {
            &self.preview_monitor
        } else {
            &self.screen
        };
        let outputs = &self.data.status.outputs;
        let ratio = outputs
            .iter()
            .find(|m| &m.id == id)
            .or_else(|| outputs.first())
            .filter(|m| m.width > 0 && m.height > 0)
            .map_or(
                f64::from(self.display_size.width / self.display_size.height.max(1.)),
                |m| f64::from(m.width) / f64::from(m.height),
            );
        let height = (self.size.height * 0.32).clamp(130., 300.);
        container(responsive(move |size| {
            let w = f64::from(size.width).min(f64::from(size.height) * ratio).max(1.);
            let h = w / ratio;
            let Some(handle @ image::Handle::Rgba { width, height, .. }) = &self.adjusted else {
                return space::vertical().into();
            };
            let (sw, sh) = (f64::from(*width), f64::from(*height));
            let (x, y, dw, dh) = self.framing.bounds(self.fit, (sw, sh), (w, h));
            let (vw, vh) = (w.min(dw), h.min(dh));
            let region = Rectangle {
                x: (-x * sw / dw).max(0.).round() as u32,
                y: (-y * sh / dh).max(0.).round() as u32,
                width: (vw * sw / dw).round().max(1.) as u32,
                height: (vh * sh / dh).round().max(1.) as u32,
            };
            let pic = image(handle.clone())
                .crop(region)
                .content_fit(ContentFit::Fill)
                .width(vw as f32)
                .height(vh as f32);
            let pic = iced::widget::stack![space().width(vw as f32).height(vh as f32), pic].clip(true);
            let surface = container(pin(pic).x(x.max(0.) as f32).y(y.max(0.) as f32))
                .style(|_| container::Style::default().background(Color::BLACK))
                .clip(true)
                .width(w as f32)
                .height(h as f32);
            let drag = mouse_area(surface)
                .interaction(iced::mouse::Interaction::Grab)
                .on_press(Message::Drag(true))
                .on_release(Message::Drag(false))
                .on_exit(Message::Drag(false))
                .on_move(move |pt| Message::Move(pt, w - dw, h - dh));
            let hint = self.data.profile.language.locale().get(Text::Position);
            let drag = iced::widget::tooltip(
                drag,
                container(text(hint).size(13)).padding(6).style(container::rounded_box),
                iced::widget::tooltip::Position::Bottom,
            )
            .delay(std::time::Duration::from_millis(850));
            container(drag).center(Fill).into()
        }))
        .width(Fill)
        .height(height)
        .style(|theme: &Theme| {
            container::Style::default()
                .background(theme.extended_palette().background.weak.color)
                .border(iced::Border::default().rounded(8))
        })
        .into()
    }
}

fn labeled_picker<'a, T: Copy + PartialEq + 'a, const N: usize>(
    items: [(T, &'static str); N],
    current: T,
    set: fn(T) -> Message,
) -> View<'a> {
    let index = items.iter().position(|(v, _)| *v == current).unwrap_or(0);
    let widget = pick_list(items.map(|(_, name)| name), Some(items[index].1), move |name| {
        set(items.iter().find(|(_, n)| *n == name).unwrap().0)
    });
    focus::cycle(
        widget.style(style::picker),
        Some(set(items[(index + 1) % N].0)),
        Some(set(items[(index + N - 1) % N].0)),
    )
}

fn toggle(value: bool, label: &'static str, enabled: bool, set: fn(bool) -> Message) -> View<'static> {
    focus::action(
        checkbox(value)
            .style(style::check_box)
            .label(label)
            .on_toggle_maybe(enabled.then_some(set)),
        enabled.then(|| set(!value)),
    )
}

fn filename(name: &str, font_size: u16, height: u16) -> View<'_> {
    let line = iced::widget::stack![
        space().width(Fill).height(f32::from(height)),
        text(name)
            .size(f32::from(font_size))
            .width(Fill)
            .height(f32::from(height))
            .wrapping(text::Wrapping::None)
    ]
    .clip(true);
    iced::widget::tooltip(
        line,
        container(text(name).size(14))
            .padding(6)
            .max_width(400)
            .style(container::rounded_box),
        iced::widget::tooltip::Position::FollowCursor,
    )
    .delay(std::time::Duration::from_millis(650))
    .into()
}

fn display_picker<'a>(
    options: Vec<super::displays::Display>,
    selected: super::displays::Display,
    set: fn(super::displays::Display) -> Message,
) -> View<'a> {
    let index = options.iter().position(|item| item == &selected).unwrap_or(0);
    let next = set(options[(index + 1) % options.len()].clone());
    let previous = set(options[(index + options.len() - 1) % options.len()].clone());
    focus::cycle(
        pick_list(options, Some(selected), set).style(style::picker).width(Fill),
        Some(next),
        Some(previous),
    )
}
