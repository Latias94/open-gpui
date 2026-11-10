//! Public consumers of native window services at Zed target
//! 635026fabc38440fce5e1767d125026a17b12012. Runtime controls use TestPlatform's
//! unsupported native-state fallback, without qualifying native OS restoration or attention.

use open_gpui::{PlatformWindow, Window};

#[test]
fn public_native_window_service_signatures_are_type_only() {
    let _: fn(&Window) -> Option<Vec<u8>> = Window::native_window_state;
    let _: fn(&Window, &[u8]) = Window::restore_native_window_state;
    let _: fn(&Window) = Window::request_attention;
    let _: fn(&dyn PlatformWindow) -> Option<Vec<u8>> = |window| window.native_window_state();
    let _: fn(&dyn PlatformWindow, &[u8]) =
        |window, state| window.restore_native_window_state(state);
    let _: fn(&dyn PlatformWindow) = |window| window.request_attention();
}

#[cfg(feature = "test-support")]
#[test]
fn public_window_native_state_is_unsupported_and_attention_preserves_the_window() {
    use open_gpui::{AppContext as _, Empty, QuitMode, TestAppContext, px, size};

    let mut cx = TestAppContext::single();
    cx.update(|app| app.set_quit_mode(QuitMode::Explicit));
    let handle = cx.open_window(size(px(160.0), px(120.0)), |_, _| Empty);
    cx.run_until_parked();
    let initial_bounds = cx
        .update_window(handle.into(), |_, window, _| {
            assert_eq!(window.native_window_state(), None);
            let initial_bounds = window.bounds();
            let caller_buffer = vec![0x00, 0xff, 0x80, 0x01];
            window.restore_native_window_state(&caller_buffer);
            drop(caller_buffer);
            window.restore_native_window_state(&[]);
            window.request_attention();
            window.request_attention();
            assert_eq!(window.native_window_state(), None);
            initial_bounds
        })
        .expect("public native service calls must accept a live Window");
    cx.run_until_parked();

    cx.update_window(handle.into(), |_, window, app| {
        assert_eq!(window.native_window_state(), None);
        assert_eq!(window.bounds(), initial_bounds);
        window.remove_window(app);
    })
    .expect("unsupported restoration must leave the Window usable and closable");
    cx.run_until_parked();
    assert!(cx.windows().is_empty());
    cx.quit();
}
