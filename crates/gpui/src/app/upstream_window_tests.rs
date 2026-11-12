use crate::{
    AppContext as _, Bounds, Context, DisplayId, Empty, FocusHandle, InteractiveElement as _,
    IntoElement, ParentElement as _, Platform as _, PlatformWindowPresentOutcome, Render,
    Styled as _, TestAppContext, Window, WindowBounds, WindowOptions, div, point, px, size,
};
use std::{
    cell::Cell,
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
};

#[crate::test]
fn new_window_uses_the_active_display_while_its_window_is_checked_out(cx: &mut TestAppContext) {
    let display_id = DisplayId::new(2);
    cx.simulate_display_added(display_id, None);
    cx.simulate_display_bounds_change(
        display_id,
        Bounds::new(point(px(-1920.), px(0.)), size(px(1920.), px(1080.))),
    );
    let original = cx
        .update(|app| {
            app.open_window(
                WindowOptions {
                    display_id: Some(display_id),
                    window_bounds: Some(WindowBounds::Windowed(Bounds::new(
                        point(px(-1600.), px(100.)),
                        size(px(500.), px(300.)),
                    ))),
                    ..Default::default()
                },
                |_, app| app.new(|_| Empty),
            )
        })
        .unwrap();
    let _ = original
        .update(cx, |_, window, _| window.activate_window())
        .unwrap();
    cx.run_until_parked();

    let opened = original
        .update(cx, |_, _, app| {
            assert_eq!(app.active_display().unwrap().id(), display_id);
            app.open_window(WindowOptions::default(), |_, app| app.new(|_| Empty))
                .unwrap()
        })
        .unwrap();
    opened
        .update(cx, |_, window, app| {
            assert_eq!(window.display(app).unwrap().id(), display_id);
            assert_eq!(window.bounds().size, size(px(500.), px(300.)));
            assert_eq!(window.bounds().origin, point(px(-1575.), px(125.)));
        })
        .unwrap();

    let explicit = original
        .update(cx, |_, _, app| {
            app.open_window(
                WindowOptions {
                    display_id: Some(DisplayId::new(1)),
                    ..Default::default()
                },
                |_, app| app.new(|_| Empty),
            )
            .unwrap()
        })
        .unwrap();
    explicit
        .update(cx, |_, window, app| {
            assert_eq!(window.display(app).unwrap().id(), DisplayId::new(1));
            assert!(window.bounds().origin.x >= px(0.));
        })
        .unwrap();
}

#[crate::test]
fn bounds_observer_opening_a_window_uses_the_new_committed_size(cx: &mut TestAppContext) {
    let original = cx.add_window(|_, _| Empty);
    let _ = original
        .update(cx, |_, window, _| window.activate_window())
        .unwrap();
    cx.run_until_parked();
    let opened = Rc::new(Cell::new(None));
    let _subscription = original
        .update(cx, {
            let opened = opened.clone();
            move |_, window, app| {
                app.observe_window_bounds(window, move |_, _, app| {
                    let handle = app
                        .open_window(WindowOptions::default(), |_, app| app.new(|_| Empty))
                        .unwrap();
                    opened.set(Some(handle));
                })
            }
        })
        .unwrap();
    let expected = size(px(400.), px(300.));
    cx.simulate_window_resize(original.into(), expected);
    opened
        .take()
        .expect("the bounds observer must open a window")
        .update(cx, |_, window, _| {
            assert_eq!(window.bounds().size, expected)
        })
        .unwrap();
}

#[crate::test]
fn explicit_display_centering_preserves_the_legacy_primary_helper(cx: &mut TestAppContext) {
    let display_id = DisplayId::new(2);
    cx.simulate_display_added(display_id, None);
    cx.simulate_display_bounds_change(
        display_id,
        Bounds::new(point(px(-1200.), px(50.)), size(px(1200.), px(800.))),
    );
    cx.update(|app| {
        let requested = size(px(600.), px(400.));
        assert_eq!(
            WindowBounds::centered_on_display(Some(display_id), requested, app),
            WindowBounds::Windowed(Bounds::new(point(px(-900.), px(250.)), requested))
        );
        assert_eq!(
            WindowBounds::centered(requested, app),
            WindowBounds::centered_on_display(None, requested, app)
        );
    });
}

#[crate::test]
fn nested_updates_preserve_ancestor_placement_changes_on_return_and_unwind(
    cx: &mut TestAppContext,
) {
    for fail in [false, true] {
        let original = cx.add_window(|_, _| Empty);
        let nested = cx.add_window(|_, _| Empty);
        let native = cx.test_window(original.into());
        let _ = original
            .update(cx, |_, window, _| window.activate_window())
            .unwrap();
        cx.run_until_parked();
        let expected = size(px(450.), px(330.));
        let opened = cx
            .update_window(original.into(), |_, original_window, app| {
                let result = catch_unwind(AssertUnwindSafe(|| {
                    app.update_window(nested.into(), |_, _, app| {
                        native.clone().simulate_resize(expected);
                        original_window.bounds_changed(app);
                        if fail {
                            panic!("injected nested placement update panic");
                        }
                    })
                    .unwrap();
                }));
                assert_eq!(result.is_err(), fail);
                app.open_window(WindowOptions::default(), |_, app| app.new(|_| Empty))
                    .unwrap()
            })
            .unwrap();
        opened
            .update(cx, |_, window, _| {
                assert_eq!(window.bounds().size, expected)
            })
            .unwrap();
    }
}

#[crate::test]
fn placement_uses_coherent_display_facts_before_the_display_observer(cx: &mut TestAppContext) {
    let target = DisplayId::new(2);
    cx.simulate_display_added(target, None);
    cx.simulate_display_bounds_change(
        target,
        Bounds::new(point(px(-1920.), px(0.)), size(px(1920.), px(1080.))),
    );
    let original = cx.add_window(|_, _| Empty);
    let _ = original
        .update(cx, |_, window, _| window.activate_window())
        .unwrap();
    cx.run_until_parked();
    let opened = Rc::new(Cell::new(None));
    let _subscription = original
        .update(cx, {
            let opened = opened.clone();
            move |_, window, app| {
                app.observe_window_bounds(window, move |_, _, app| {
                    assert_eq!(app.active_display().unwrap().id(), target);
                    opened.set(Some(
                        app.open_window(WindowOptions::default(), |_, app| app.new(|_| Empty))
                            .unwrap(),
                    ));
                })
            }
        })
        .unwrap();
    let native = cx.test_window(original.into());
    let display = cx
        .test_platform()
        .display_snapshot()
        .find_display(target)
        .unwrap();
    native.set_display(Some(display));
    native.clone().simulate_resize(size(px(450.), px(330.)));
    cx.run_until_parked();
    opened
        .take()
        .unwrap()
        .update(cx, |_, window, app| {
            assert_eq!(window.display(app).unwrap().id(), target)
        })
        .unwrap();
}

#[crate::test]
fn diagnostics_report_cached_native_and_checked_out_window_state(cx: &mut TestAppContext) {
    let handle = cx.add_window(|_, _| Empty);
    let native = cx.test_window(handle.into());
    native.defer_frame_requests_for_test();
    handle
        .update(cx, |_, window, app| {
            let original = window.scale_factor();
            native.set_physical_client_geometry(None, original + 1.);
            let state = window.debug_state();
            assert_eq!(state["scale_factor"]["cached"], original);
            assert_eq!(state["scale_factor"]["platform"], original + 1.);
            native.set_physical_client_geometry(None, original);

            let state = app.debug_state();
            let updating = state["windows"]
                .as_array()
                .unwrap()
                .iter()
                .find(|state| state["id"] == handle.window_id().as_u64())
                .unwrap();
            assert_eq!(updating["updating"], true);
        })
        .unwrap();
    let state = cx.update(|app| app.debug_state());
    assert_eq!(state["windows"][0]["visibility"]["cached"], "Visible");
    assert_eq!(state["windows"][0]["visibility"]["platform"], "Visible");
}

struct FocusRoot {
    focus: FocusHandle,
    show_child: bool,
}

impl Render for FocusRoot {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().size_full().children(
            self.show_child
                .then(|| div().id("child").track_focus(&self.focus).size(px(30.))),
        )
    }
}

#[crate::test]
fn is_rendered_tracks_submitted_focus_nodes_across_rejected_frames(cx: &mut TestAppContext) {
    let handle = cx.add_window(|_, app| FocusRoot {
        focus: app.focus_handle(),
        show_child: true,
    });
    let native = cx.test_window(handle.into());
    native.defer_frame_requests_for_test();
    let focus = handle.update(cx, |root, _, _| root.focus.clone()).unwrap();
    native.set_frame_delivery_mode(crate::PlatformFrameDeliveryMode::Deferred);
    handle
        .update(cx, |root, window, _| {
            assert!(focus.is_rendered(window));
            root.show_child = false;
            window.refresh();
        })
        .unwrap();
    cx.update_window(handle.into(), |_, window, app| {
        window.draw(app).clear(app);
        assert_eq!(window.present(app), PlatformWindowPresentOutcome::Deferred);
        assert!(focus.is_rendered(window));
    })
    .unwrap();
    let rejected = native.pending_frame_submission().unwrap();
    assert!(rejected.complete(PlatformWindowPresentOutcome::Rejected));
    cx.run_until_parked();
    cx.update_window(handle.into(), |_, window, app| {
        assert!(focus.is_rendered(window));
        window.draw(app).clear(app);
        assert_eq!(window.present(app), PlatformWindowPresentOutcome::Deferred);
        assert!(focus.is_rendered(window));
    })
    .unwrap();
    let submitted = native.pending_frame_submission().unwrap();
    assert!(submitted.complete(PlatformWindowPresentOutcome::Submitted));
    cx.run_until_parked();
    cx.update_window(handle.into(), |_, window, _| {
        assert!(!focus.is_rendered(window));
    })
    .unwrap();
}

#[crate::test]
fn custom_titlebar_drag_policy_reaches_the_native_factory_independently_of_movability(
    cx: &mut TestAppContext,
) {
    let platform = cx.test_platform();
    for is_movable in [false, true] {
        for app_owns_titlebar_drag in [false, true] {
            cx.update(|app| {
                app.open_window_with_platform_window(
                    WindowOptions {
                        is_movable,
                        app_owns_titlebar_drag,
                        ..Default::default()
                    },
                    |handle, params, displays| {
                        assert_eq!(params.is_movable, is_movable);
                        assert_eq!(params.app_owns_titlebar_drag, app_owns_titlebar_drag);
                        platform.open_window_with_display_snapshot(handle, params, displays)
                    },
                    |_, app| app.new(|_| Empty),
                )
                .unwrap();
            });
        }
    }
}
