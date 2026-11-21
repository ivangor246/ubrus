use super::*;
#[test]
fn locale_uses_supported_language_preferences() {
    assert_eq!(Locale::detect(["ru_RU.UTF-8"]), Locale::Russian);
    assert_eq!(Locale::detect(["fr", "ru-RU", "en"]), Locale::Russian);
    assert_eq!(Locale::detect(["en_GB", "ru"]), Locale::English);
    assert_eq!(Locale::detect(["ja_JP"]), Locale::English);
}
#[test]
fn old_profiles_follow_the_system_and_choices_round_trip() {
    let mut profile: crate::library::Profile =
        serde_json::from_str(r#"{"items":{},"screens":{},"autostart":null,"power":false,"paused":false}"#).unwrap();
    assert_eq!(profile.theme, Appearance::System);
    assert_eq!(profile.language, Language::System);
    profile.theme = Appearance::Dark;
    profile.language = Language::Russian;
    let restored: crate::library::Profile = serde_json::from_value(serde_json::to_value(profile).unwrap()).unwrap();
    assert_eq!(restored.theme, Appearance::Dark);
    assert_eq!(restored.language.locale(), Locale::Russian);
}

#[test]
fn window_and_panel_translations_stay_in_sync() {
    let catalog: std::collections::BTreeMap<String, std::collections::BTreeMap<String, String>> =
        serde_json::from_str(include_str!("../../extension/locale.json")).unwrap();
    let english = &catalog["en"];
    for entries in catalog.values() {
        assert!(entries.keys().eq(english.keys()));
        assert!(entries.values().all(|value| !value.trim().is_empty()));
        assert!(Text::ALL.iter().all(|key| entries.contains_key(key.key())));
    }
}
