use super::*;

#[test]
fn diagnostics_can_be_selected_and_copied_but_not_edited() {
    let mut view = ReportView::default();
    view.update("Сеанс Wayland".into(), Some("Error details".into()));
    view.interact(text_editor::Action::SelectAll);
    view.update("Сеанс Wayland".into(), None);
    assert_eq!(
        view.content.selection().as_deref(),
        Some("Сеанс Wayland\n\nError details")
    );
    view.interact(text_editor::Action::Edit(text_editor::Edit::Insert('x')));
    assert_eq!(view.content.text(), "Сеанс Wayland\n\nError details");
}
