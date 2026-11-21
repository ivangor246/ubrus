use iced::widget::text_editor;

/// Keep one selectable document while status notifications arrive from Shell.
#[derive(Default)]
pub(super) struct ReportView {
    summary: String,
    log: String,
    pub content: text_editor::Content,
}

impl ReportView {
    pub fn update(&mut self, summary: String, log: Option<String>) {
        if self.summary == summary && log.as_ref().is_none_or(|value| value == &self.log) {
            return;
        }
        self.summary = summary;
        if let Some(log) = log {
            self.log = log;
        }
        let text = format!("{}\n\n{}", self.summary, self.log);
        self.content = text_editor::Content::with_text(&text);
    }

    pub fn interact(&mut self, action: text_editor::Action) {
        if !action.is_edit() {
            self.content.perform(action);
        }
    }
}

#[cfg(test)]
#[path = "../../tests/rust/report_view.rs"]
mod tests;
