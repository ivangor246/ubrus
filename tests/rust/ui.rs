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

#[test]
fn gallery_windows_keep_spacing_and_limit_work_to_visible_rows() {
    use crate::ui::gallery::{GAP, TILE_HEIGHT, page};
    let total = 2_000;
    let first = page(380., 600., 0., total);
    let near_end = page(380., 600., 999999., total);
    for (page, at_end) in [(first, false), (near_end, true)] {
        assert_eq!(page.columns, 2);
        assert!(page.end - page.first < 20);
        let shown_rows = (page.end - page.first).div_ceil(page.columns);
        let full_height = 1_000. * (TILE_HEIGHT + GAP) - GAP;
        assert_eq!(
            page.before + shown_rows as f32 * (TILE_HEIGHT + GAP) - GAP + page.after,
            full_height
        );
        assert_eq!(page.end == total, at_end);
    }
    let mut p = crate::library::Profile::default();
    for (id, name) in [("a", "ЗЕЛЕНЫЙ.png"), ("b", "forest.PNG"), ("c", "forest.webm")] {
        p.items.insert(
            id.into(),
            crate::library::Item {
                name: name.into(),
                ext: "png".into(),
                motion: None,
            },
        );
    }
    let sorted = p.sorted_ids();
    assert_eq!(crate::ui::gallery::matches(&p, &sorted, " зел "), ["a"]);
    assert_eq!(crate::ui::gallery::matches(&p, &sorted, "FOREST"), ["b", "c"]);
}

#[test]
fn keyboard_paging_reaches_offscreen_tiles_and_preserves_a_focus_target_at_both_ends() {
    use crate::ui::gallery::{keyboard_page, page};
    let count = 501;
    let width = 380.;
    let height = 395.;
    let window_height = 600.;
    let initial = page(width, window_height, 0., count);
    let mut offset = 0.;
    let mut reached_offscreen = false;
    let mut previous = 0;
    loop {
        let (next, focus) = keyboard_page(initial.columns, count, height, offset, true).unwrap();
        let mounted = page(width, window_height, next, count);
        assert!((mounted.first..mounted.end).contains(&focus));
        assert!(focus >= previous && focus < count);
        reached_offscreen |= focus >= initial.end;
        if next == offset {
            assert_eq!(mounted.end, count);
            break;
        }
        offset = next;
        previous = focus;
    }
    assert!(reached_offscreen);
    while offset > 0. {
        let (next, focus) = keyboard_page(initial.columns, count, height, offset, false).unwrap();
        assert!(next < offset);
        if next == 0. {
            assert_eq!(focus, 0);
        }
        offset = next;
    }
    assert_eq!(
        keyboard_page(initial.columns, count, height, offset, false),
        Some((0., 0))
    );
    assert_eq!(keyboard_page(1, 0, height, 0., true), None);
}
