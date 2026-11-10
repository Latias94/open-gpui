//! Public App consumers of Zed's 8e4e5a39eed932d943cb7a3e7078885b75d1ea54 API
//! at fixed target 635026fabc38440fce5e1767d125026a17b12012.
//! TestPlatform retains its default no-op setter; these controls do not qualify
//! macOS appearance overrides or native titlebars.

#![cfg(feature = "test-support")]

use open_gpui::{
    App, Empty, QuitMode, TestAppContext, VisualTestContext, WindowAppearance, px, size,
};
use std::{cell::Cell, rc::Rc};

#[test]
fn public_app_appearance_requests_leave_default_platform_windows_and_observers_unchanged() {
    let mut cx = TestAppContext::single();
    cx.update(|app| app.set_quit_mode(QuitMode::Explicit));
    let initial = cx.read(App::window_appearance);
    let handle = cx.open_window(size(px(160.), px(120.)), |_, _| Empty);
    let mut visual = VisualTestContext::from_window(handle.into(), &cx);
    visual.run_until_parked();
    visual.update(|window, app| window.draw(app).clear(app));
    visual.run_until_parked();

    let notifications = Rc::new(Cell::new(0));
    let (revision, subscription) = visual.update(|window, _| {
        assert_eq!(window.appearance(), initial);
        let notifications = Rc::clone(&notifications);
        (
            window.rendered_frame_revision(),
            window.observe_window_appearance(move |_, _| {
                notifications.set(notifications.get() + 1);
            }),
        )
    });
    assert!(revision > 0, "the baseline must include a rendered frame");

    for request in [
        None,
        Some(WindowAppearance::Dark),
        Some(WindowAppearance::Dark),
        Some(WindowAppearance::VibrantLight),
        Some(WindowAppearance::VibrantLight),
        Some(WindowAppearance::VibrantDark),
        Some(WindowAppearance::VibrantDark),
        Some(WindowAppearance::Light),
        Some(WindowAppearance::Light),
        None,
        None,
        Some(WindowAppearance::Dark),
        None,
    ] {
        cx.read(|app: &App| app.set_window_appearance(request));
        cx.run_until_parked();
        assert_eq!(cx.read(App::window_appearance), initial, "{request:?}");
        visual.update(|window, _| {
            assert_eq!(window.appearance(), initial, "{request:?}");
            assert_eq!(window.rendered_frame_revision(), revision, "{request:?}");
        });
        assert_eq!(notifications.get(), 0, "{request:?}");
    }

    drop(subscription);
    visual.update(|window, app| window.remove_window(app));
    cx.run_until_parked();
    assert!(cx.windows().is_empty());
    cx.quit();
}
