use super::*;
#[test]
fn unplugging_and_reconnecting_preserves_the_selected_monitor() {
    let output = Output {
        id: "DP-1".into(),
        name: "Monitor".into(),
        ..Default::default()
    };
    let (_, selected) = monitors(Locale::English, &[], "DP-1");
    assert!(!selected.available);
    assert_eq!(selected.id, "DP-1");
    let (_, restored) = monitors(Locale::Russian, &[output], &selected.id);
    assert!(restored.available);
    assert_eq!(restored.label, "Monitor");
}
