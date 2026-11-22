use super::*;
#[test]
fn locale_uses_supported_language_preferences() {
    assert_eq!(Locale::detect(["ru_RU.UTF-8"]), Locale::Russian);
    assert_eq!(Locale::detect(["fr", "ru-RU", "en"]), Locale::Russian);
    assert_eq!(Locale::detect(["en_GB", "ru"]), Locale::English);
    for locale in Locale::ALL {
        assert_eq!(Locale::detect(["xx", locale.tag()]), *locale);
    }
    for (tag, expected) in [
        ("ja_JP", Locale::Japanese),
        ("zh_TW.UTF-8", Locale::TraditionalChinese),
        ("zh-HK", Locale::TraditionalChinese),
        ("zh_MO", Locale::TraditionalChinese),
        ("zh-Hans-TW", Locale::SimplifiedChinese),
        ("zh-Hant-CN", Locale::TraditionalChinese),
        ("zh_CN", Locale::SimplifiedChinese),
        ("zh_SG", Locale::SimplifiedChinese),
        ("zh", Locale::SimplifiedChinese),
        ("zh.UTF-8@traditional", Locale::TraditionalChinese),
        ("pt_BR", Locale::Portuguese),
        ("in_ID", Locale::Indonesian),
        ("xx", Locale::English),
        ("C.UTF-8", Locale::English),
    ] {
        assert_eq!(Locale::detect([tag]), expected, "{tag}");
    }
}
#[test]
fn adjustment_labels_follow_the_selected_locale() {
    assert_eq!(Locale::Turkish.percent(125), "%125");
    assert_eq!(Locale::German.percent(125), "125\u{a0}%");
    assert_eq!(Locale::Japanese.speed(125), "1.25×");
    assert_eq!(Locale::Portuguese.speed(125), "1,25×");
}

#[test]
fn old_profiles_follow_the_system_and_choices_round_trip() {
    let mut profile: crate::library::Profile =
        serde_json::from_str(r#"{"items":{},"screens":{},"autostart":null,"power":false,"paused":false}"#).unwrap();
    assert_eq!(profile.theme, Appearance::System);
    assert_eq!(profile.language, Language::System);
    profile.theme = Appearance::Dark;
    profile.language = serde_json::from_str(r#""russian""#).unwrap();
    assert_eq!(serde_json::to_string(&profile.language).unwrap(), r#""russian""#);
    for (language, _) in Language::options(Locale::English) {
        assert_eq!(
            serde_json::from_value::<Language>(serde_json::to_value(language).unwrap()).unwrap(),
            language
        );
    }
    let restored: crate::library::Profile = serde_json::from_value(serde_json::to_value(profile).unwrap()).unwrap();
    assert_eq!(restored.theme, Appearance::Dark);
    assert_eq!(restored.language.locale(), Locale::Russian);
}

#[test]
fn window_and_panel_translations_stay_in_sync() {
    let catalog: std::collections::BTreeMap<String, std::collections::BTreeMap<String, String>> =
        serde_json::from_str(include_str!("../../extension/locale.json")).unwrap();
    let english = &catalog["en"];
    assert_eq!(catalog.len(), Locale::ALL.len());
    for locale in Locale::ALL {
        let entries = &catalog[locale.tag()];
        for token in ["{added}", "{failed}", "{skipped}"] {
            assert_eq!(entries["ImportSummary"].matches(token).count(), 1, "{}", locale.tag());
        }
        assert!(entries.keys().eq(english.keys()));
        assert!(entries.values().all(|value| !value.trim().is_empty()));
        assert!(Text::ALL.iter().all(|key| entries.contains_key(key.key())));
    }
}
