//! Public close-request callbacks and the logical/native terminal distinction.

use open_gpui::PlatformWindow;

#[test]
fn public_required_close_and_terminal_hooks_accept_single_shot_callbacks() {
    let _: fn(&dyn PlatformWindow, Box<dyn FnOnce()>) = |window, callback| {
        window.on_close_required(callback);
    };
    let _: fn(&dyn PlatformWindow, Box<dyn FnOnce()>) = |window, callback| {
        window.on_close(callback);
    };
    #[cfg(feature = "test-support")]
    {
        let _: fn(&open_gpui::TestAppContext, open_gpui::AnyWindowHandle) -> bool =
            open_gpui::TestAppContext::simulate_window_close_required;
        let _: fn(&mut dyn PlatformWindow) -> Option<bool> =
            |window| window.as_test().map(|window| window.fire_close_required());
    }
}

#[cfg(feature = "test-support")]
#[open_gpui::test]
fn public_required_close_bypasses_veto_and_defers_native_terminal(
    cx: &mut open_gpui::TestAppContext,
) {
    use open_gpui::{AppContext as _, Empty, QuitMode, px, size};
    use std::{
        cell::{Cell, RefCell},
        rc::Rc,
    };

    cx.update(|app| app.set_quit_mode(QuitMode::Explicit));
    let handle = cx.open_window(size(px(160.0), px(120.0)), |_, _| Empty);
    cx.run_until_parked();
    let should_close_calls = Rc::new(Cell::new(0));
    cx.update_window(handle.into(), |_, window, app| {
        let should_close_calls = should_close_calls.clone();
        window.on_window_should_close(app, move |_, _| {
            should_close_calls.set(should_close_calls.get() + 1);
            false
        });
    })
    .expect("the public window must accept a vetoing user close handler");
    let observations = Rc::new(RefCell::new(Vec::new()));
    let _logical = cx.update(|app| {
        let observations = observations.clone();
        app.on_window_closed(move |_, window_id| {
            observations.borrow_mut().push(("logical", window_id));
        })
    });
    let _terminal = cx.update(|app| {
        let observations = observations.clone();
        app.on_window_native_terminal(move |_, window_id| {
            observations.borrow_mut().push(("terminal", window_id));
        })
    });
    let outcome = cx.simulate_window_close_request(handle.into());
    assert!(!outcome.native_close_allowed());
    assert!(!outcome.terminal_transition_started());
    assert_eq!(should_close_calls.get(), 1);
    assert_eq!(cx.windows(), [handle.into()]);
    assert!(observations.borrow().is_empty());

    let terminal_hold = cx.hold_window_native_terminal(handle.into());
    assert!(cx.simulate_window_close_required(handle.into()));
    cx.run_until_parked();
    assert!(cx.windows().is_empty());
    assert_eq!(should_close_calls.get(), 1);
    assert_eq!(
        observations.borrow().as_slice(),
        &[("logical", handle.window_id())]
    );

    let replacement = cx.open_window(size(px(80.0), px(60.0)), |_, _| Empty);
    assert_ne!(replacement.window_id(), handle.window_id());
    cx.update(|app| {
        assert!(terminal_hold.release());
        assert!(app.windows().contains(&replacement.into()));
        assert_eq!(
            observations.borrow().as_slice(),
            &[("logical", handle.window_id())]
        );
    });
    cx.run_until_parked();
    assert_eq!(cx.windows(), [replacement.into()]);
    assert_eq!(
        observations.borrow().as_slice(),
        &[
            ("logical", handle.window_id()),
            ("terminal", handle.window_id())
        ]
    );
    assert_eq!(should_close_calls.get(), 1);
    cx.update_window(replacement.into(), |_, window, app| {
        window.remove_window(app)
    })
    .expect("delivery for the old native authority must preserve the replacement window");
}
