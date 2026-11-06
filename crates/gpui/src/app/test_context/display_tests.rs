use crate::{
    AppContext as _, Bounds, DevicePixels, DisplayId, Empty, Platform, PlatformWindow,
    TestAppContext, WindowOptions, point, px, size,
};
use std::time::Duration;

#[crate::test]
fn display_publication_preserves_exact_immutable_properties(cx: &mut TestAppContext) {
    let first = DisplayId::new(1);
    let second = DisplayId::new(2);
    let original = cx.test_platform().display_snapshot();
    let original_display = original.find_display(first).unwrap();
    let uuid = original_display.uuid().unwrap();
    let original_bounds = original_display.bounds();
    let interval = Duration::from_secs(1) / 144;
    let bounds = Bounds::new(point(px(-2560.), px(100.)), size(px(2560.), px(1440.)));

    cx.simulate_display_properties_change(first, bounds, Some(interval));
    cx.simulate_display_added(second, None);
    let updated = cx.test_platform().display_snapshot();
    let updated_display = updated.find_display(first).unwrap();
    assert_ne!(original.generation(), updated.generation());
    assert_eq!(updated_display.uuid().unwrap(), uuid);
    assert_eq!(updated_display.bounds(), bounds);
    assert_eq!(updated_display.visible_bounds(), bounds);
    assert_eq!(updated_display.refresh_interval(), Some(interval));
    assert_eq!(updated.primary_display().unwrap().id(), first);
    assert_eq!(
        updated.find_display(second).unwrap().refresh_interval(),
        None
    );
    assert_eq!(original_display.bounds(), original_bounds);
    assert_eq!(
        original_display.refresh_interval(),
        Some(Duration::from_secs(1) / 60)
    );
    assert!(original.find_display(second).is_none());

    cx.simulate_displays_changed();
    assert_eq!(
        cx.test_platform().display_snapshot().generation(),
        updated.generation()
    );
    cx.simulate_display_removed(first);
    let remaining = cx.test_platform().display_snapshot();
    assert_eq!(remaining.primary_display().unwrap().id(), second);
    assert!(remaining.find_display(first).is_none());
    cx.simulate_display_removed(second);
    let empty = cx.test_platform().display_snapshot();
    assert!(empty.displays().is_empty());
    assert!(empty.primary_display().is_none());
    assert_eq!(updated_display.bounds(), bounds);
}

#[crate::test]
fn window_creation_and_moves_use_the_selected_displays_actual_properties(cx: &mut TestAppContext) {
    let id = DisplayId::new(2);
    let bounds = Bounds::new(point(px(1920.), px(0.)), size(px(2560.), px(1440.)));
    let interval = Duration::from_secs(1) / 144;
    cx.simulate_display_added(id, Some(interval));
    cx.simulate_display_bounds_change(id, bounds);
    let display = cx
        .test_platform()
        .display_snapshot()
        .find_display(id)
        .unwrap();
    let window = cx
        .update(|app| {
            app.open_window(
                WindowOptions {
                    display_id: Some(id),
                    focus_on_appearing: false,
                    ..Default::default()
                },
                |_, app| app.new(|_| Empty),
            )
        })
        .unwrap();
    let native = cx.test_window(window.into());
    let selected = native.display().unwrap();
    assert_eq!(selected.id(), id);
    assert_eq!(selected.bounds(), bounds);
    assert_eq!(selected.refresh_interval(), Some(interval));
    assert_eq!(selected.uuid().unwrap(), display.uuid().unwrap());
    native.simulate_move_to_display(DisplayId::new(1));
    native.simulate_move_to_display(id);
    let moved = native.display().unwrap();
    assert_eq!(moved.uuid().unwrap(), display.uuid().unwrap());
    assert_eq!(moved.bounds(), bounds);
    assert_eq!(moved.refresh_interval(), Some(interval));
}

#[crate::test]
fn unknown_display_does_not_fabricate_a_physical_topology_observation(cx: &mut TestAppContext) {
    let window = cx.add_window(|_, _| Empty);
    let native = cx.test_window(window.into());
    cx.set_platform_window_physical_client_geometry(
        window.into(),
        Some(Bounds::new(
            point(DevicePixels(0), DevicePixels(0)),
            size(DevicePixels(640), DevicePixels(480)),
        )),
        2.0,
    );
    native.simulate_move_to_display(DisplayId::new(999));
    cx.run_until_parked();
    let facts = native.platform_facts();
    assert_eq!(facts.display_id, Some(DisplayId::new(999)));
    assert!(
        facts
            .physical_geometry
            .unwrap()
            .display_observation()
            .is_none()
    );
    window
        .update(cx, |_, window, app| {
            assert_eq!(window.refresh_interval(app), None);
            assert!(window.display(app).is_none());
        })
        .unwrap();
}
