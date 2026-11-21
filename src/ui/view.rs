use super::{Message, Tab, Ui, displays::monitors};
use crate::{
    app::Preference,
    preferences::{Appearance, Language, Locale, Text},
};
use iced::{
    Alignment, ContentFit, Element, Fill, Length,
    widget::{
        button, checkbox, column, container, image, pick_list, progress_bar, radio, row, rule, scrollable, space, text,
        text_editor,
    },
    window,
};

impl Ui {
    pub(super) fn view(&self, _: window::Id) -> Element<'_, Message> {
        let locale = self.data.profile.language.locale();
        let t = |key| locale.get(key);
        let mut nav = row![text("Ubrus").size(28), space::horizontal()]
            .spacing(8)
            .align_y(Alignment::Center);
        for (tab, label) in [
            (Tab::Library, Text::Library),
            (Tab::Preferences, Text::Preferences),
            (Tab::Diagnostics, Text::Diagnostics),
        ] {
            nav = nav.push(
                button(t(label))
                    .style(if self.tab == tab { button::primary } else { button::text })
                    .on_press(Message::Tab(tab)),
            );
        }
        let content = match self.tab {
            Tab::Preferences => self.settings(locale),
            Tab::Diagnostics => self.diagnostics(locale),
            Tab::Library => self.library(locale),
        };
        let mut page = column![nav, rule::horizontal(1), container(content).width(Fill).height(Fill)].spacing(16);
        if let Some(progress) = self.progress {
            page = page.push(progress_bar(0.0..=100., f32::from(progress)).length(Fill));
        }
        if let Some(feedback) = self
            .feedback
            .or_else(|| (!self.data.status.error.is_empty()).then_some(Text::FailedService))
        {
            page = page.push(text(t(feedback)).size(15));
        }
        container(page).padding(20).width(Fill).height(Fill).into()
    }

    fn diagnostics(&self, locale: Locale) -> Element<'_, Message> {
        let t = |key| locale.get(key);
        let panel = column![
            text(t(Text::Diagnostics)).size(24),
            row![
                button(t(Text::Refresh)).on_press(Message::Inspect),
                button(t(Text::Export)).on_press_maybe((!self.busy).then_some(Message::Export)),
                button(t(Text::SaveAs)).on_press_maybe((!self.busy).then_some(Message::ExportAs)),
                button(t(Text::CopyAll)).on_press(Message::CopyReport),
            ]
            .spacing(8),
        ]
        .spacing(16);
        panel
            .push(
                text_editor(&self.report.content)
                    .on_action(Message::ReportInteraction)
                    .font(iced::Font::MONOSPACE)
                    .size(14)
                    .height(Fill),
            )
            .height(Fill)
            .into()
    }

    fn settings(&self, locale: Locale) -> Element<'_, Message> {
        let t = |id| locale.get(id);
        let p = &self.data.profile;
        let mut themes = row![].spacing(24);
        for (value, label) in [
            (Appearance::System, Text::SystemTheme),
            (Appearance::Light, Text::Light),
            (Appearance::Dark, Text::Dark),
        ] {
            themes =
                themes.push(radio(t(label), value, Some(p.theme), |v| Message::Set(Preference::Theme(v))).width(145));
        }
        let mut languages = row![].spacing(24);
        for (value, label) in [
            (Language::System, t(Text::SystemLanguage)),
            (Language::English, "English"),
            (Language::Russian, "Русский"),
        ] {
            languages = languages.push(
                radio(label, value, Some(p.language), |v| {
                    Message::Set(Preference::Language(v))
                })
                .width(145),
            );
        }
        let edit = self.data.writable && !self.busy;
        let panel = column![
            text(t(Text::Preferences)).size(24),
            text(t(Text::Appearance)),
            themes,
            text(t(Text::Language)),
            languages,
            rule::horizontal(1),
            checkbox(p.autostart.unwrap_or(false))
                .label(t(Text::Login))
                .on_toggle_maybe(edit.then_some(|v| Message::Set(Preference::Login(v)))),
            checkbox(p.power)
                .label(t(Text::Power))
                .on_toggle_maybe(edit.then_some(|v| Message::Set(Preference::Power(v)))),
            checkbox(p.paused)
                .label(t(Text::Pause))
                .on_toggle_maybe(edit.then_some(|v| Message::Set(Preference::Pause(v)))),
            checkbox(p.pause_in_overview)
                .label(t(Text::PauseInOverview))
                .on_toggle_maybe(edit.then_some(|v| Message::Set(Preference::PauseInOverview(v)))),
        ]
        .spacing(15);
        scrollable(panel).into()
    }

    fn library(&self, locale: Locale) -> Element<'_, Message> {
        let t = |id| locale.get(id);
        let edit = self.data.writable && !self.busy;
        let mut list = column![].spacing(6);
        for id in &self.library_order {
            let item = &self.data.profile.items[id];
            let entry = row![
                image(self.data.store.thumb(id))
                    .width(52)
                    .height(36)
                    .content_fit(ContentFit::Cover),
                text(&item.name).size(15)
            ]
            .spacing(10)
            .align_y(Alignment::Center);
            let active = self.selected.as_ref() == Some(id);
            list = list.push(
                button(entry)
                    .style(if active { button::secondary } else { button::text })
                    .width(Fill)
                    .on_press(Message::Select(id.clone())),
            );
        }
        let sidebar = column![
            button(t(Text::Import)).on_press_maybe(edit.then_some(Message::Import)),
            scrollable(list).height(Fill),
            button(t(Text::ClearWallpapers))
                .style(button::secondary)
                .on_press_maybe((edit && !self.data.profile.screens.is_empty()).then_some(Message::ClearWallpapers))
        ]
        .spacing(14)
        .width(Length::FillPortion(2));
        let mut detail = column![].spacing(12).width(Length::FillPortion(4));
        if let Some(id) = &self.selected
            && let Some(item) = self.data.profile.items.get(id)
        {
            let handle = self
                .frame
                .clone()
                .unwrap_or_else(|| image::Handle::from_path(self.data.store.thumb(id)));
            detail = detail
                .push(
                    container(image(handle).content_fit(ContentFit::Contain).width(Fill).height(Fill))
                        .style(container::rounded_box)
                        .padding(6)
                        .width(Fill)
                        .height(Fill),
                )
                .push(text(&item.name).size(20));
            if item.animated() {
                detail = detail.push(
                    button(t(if self.playback.is_some() {
                        Text::Stop
                    } else {
                        Text::Preview
                    }))
                    .on_press(Message::Preview),
                );
            }
            let (options, selected) = monitors(locale, &self.data.status.outputs, &self.screen);
            detail = detail.push(
                row![
                    column![
                        text(t(Text::Screen)).size(14),
                        pick_list(options, Some(selected), Message::Monitor)
                    ]
                    .spacing(4),
                    column![
                        text(t(Text::Scaling)).size(14),
                        pick_list(
                            [t(Text::Fill), t(Text::Fit)],
                            Some(t(if self.fit { Text::Fit } else { Text::Fill })),
                            move |label| Message::Fit(label == locale.get(Text::Fit))
                        )
                    ]
                    .spacing(4)
                ]
                .spacing(12),
            );
            if self.delete_confirmation.as_ref() == Some(id) {
                let in_use = self.data.profile.screens.values().any(|choice| &choice.id == id);
                detail = detail
                    .push(text(t(if in_use {
                        Text::ConfirmDeleteActive
                    } else {
                        Text::ConfirmDelete
                    })))
                    .push(
                        row![
                            button(t(Text::Cancel)).on_press(Message::CancelDelete),
                            button(t(Text::Delete))
                                .style(button::danger)
                                .on_press_maybe(edit.then(|| Message::ConfirmDelete(id.clone())))
                        ]
                        .spacing(8),
                    );
            } else {
                detail = detail.push(
                    row![
                        button(t(Text::Apply)).on_press_maybe((edit && self.available()).then_some(Message::Apply)),
                        button(t(Text::DeleteItem))
                            .style(button::secondary)
                            .on_press_maybe(edit.then_some(Message::RequestDelete))
                    ]
                    .spacing(8),
                );
            }
        } else {
            detail = detail.push(
                container(text(t(Text::Empty)))
                    .padding(20)
                    .center_x(Fill)
                    .center_y(Fill),
            );
        }
        row![sidebar, rule::vertical(1), detail].spacing(16).height(Fill).into()
    }
}
