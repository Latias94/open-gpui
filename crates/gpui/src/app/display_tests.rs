use super::Application;
use crate::{
    App, AppContext as _, BackgroundExecutor, Bounds, Context, DisplayChanges, DisplayEvent,
    DisplayId, Empty, ForegroundExecutor, IntoElement, Platform, PlatformWindow, QuitMode, Render,
    Subscription, TestAppContext, TestDispatcher, TestPlatform, Window, WindowHandle, point, px,
    size,
};
use std::{
    cell::{Cell, RefCell},
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
    sync::Arc,
    time::Duration,
};

fn interval(hz: u32) -> Duration {
    Duration::from_secs(1) / hz
}

struct DisplayTracker {
    refresh_intervals: Vec<Option<Duration>>,
    _subscription: Subscription,
}

impl Render for DisplayTracker {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        Empty
    }
}

fn tracked_window(cx: &mut TestAppContext) -> WindowHandle<DisplayTracker> {
    cx.add_window(|window, cx| DisplayTracker {
        refresh_intervals: Vec::new(),
        _subscription: cx.observe_window_display(
            window,
            |tracker: &mut DisplayTracker, window, cx| {
                tracker.refresh_intervals.push(window.refresh_interval(cx));
            },
        ),
    })
}

#[crate::test]
fn display_tracking(cx: &mut TestAppContext) {
    let display_events = Rc::new(RefCell::new(Vec::new()));
    let _display_subscription = cx.update({
        let display_events = display_events.clone();
        move |cx| cx.observe_displays(move |event, _| display_events.borrow_mut().push(event))
    });
    let window = tracked_window(cx);
    let window_notifications = Rc::new(RefCell::new(Vec::new()));
    let _window_subscription = window
        .update(cx, {
            let window_notifications = window_notifications.clone();
            move |_, window, cx| {
                assert_eq!(window.refresh_interval(cx), Some(interval(60)));
                window.observe_window_display(move |window, cx| {
                    window_notifications.borrow_mut().push((
                        window.display(cx).map(|display| display.id()),
                        window.refresh_interval(cx),
                    ));
                })
            }
        })
        .unwrap();
    let test_window = cx.test_window(window.into());

    cx.test_platform()
        .connect_display(DisplayId::new(2), Some(interval(120)));
    test_window.simulate_move_to_display(DisplayId::new(2));
    cx.run_until_parked();
    assert_eq!(
        window_notifications.take(),
        [(Some(DisplayId::new(2)), Some(interval(120)))]
    );
    assert_eq!(
        display_events.take(),
        [DisplayEvent::Added(DisplayId::new(2))]
    );
    cx.simulate_display_refresh_interval_change(DisplayId::new(2), Some(interval(120)));
    assert!(window_notifications.borrow().is_empty());
    assert!(display_events.borrow().is_empty());

    test_window.simulate_move_to_display(DisplayId::new(2));
    cx.run_until_parked();
    assert!(window_notifications.borrow().is_empty());
    cx.simulate_display_refresh_interval_change(DisplayId::new(2), Some(interval(144)));
    assert_eq!(
        window_notifications.take(),
        [(Some(DisplayId::new(2)), Some(interval(144)))]
    );

    cx.simulate_display_refresh_interval_change(DisplayId::new(1), Some(interval(120)));
    cx.simulate_display_removed(DisplayId::new(1));
    assert!(window_notifications.borrow().is_empty());
    assert_eq!(
        display_events.take(),
        [
            DisplayEvent::Changed {
                id: DisplayId::new(2),
                changes: DisplayChanges::REFRESH_INTERVAL,
            },
            DisplayEvent::Changed {
                id: DisplayId::new(1),
                changes: DisplayChanges::REFRESH_INTERVAL,
            },
            DisplayEvent::Removed(DisplayId::new(1)),
        ]
    );
    window
        .update(cx, |tracker, _, _| {
            assert_eq!(
                tracker.refresh_intervals,
                [Some(interval(120)), Some(interval(144))]
            );
        })
        .unwrap();
}

#[crate::test]
fn display_events_report_bounds_refresh_and_spurious_notifications(cx: &mut TestAppContext) {
    let events = Rc::new(RefCell::new(Vec::new()));
    let _subscription = cx.update({
        let events = events.clone();
        move |cx| cx.observe_displays(move |event, _| events.borrow_mut().push(event))
    });
    let display_id = DisplayId::new(2);
    let bounds = Bounds::new(point(px(1920.), px(100.)), size(px(2560.), px(1440.)));

    cx.simulate_display_added(display_id, None);
    cx.simulate_display_bounds_change(display_id, bounds);
    cx.simulate_display_refresh_interval_change(display_id, Some(interval(120)));
    cx.simulate_display_properties_change(display_id, Bounds::default(), None);
    cx.simulate_displays_changed();
    cx.simulate_display_added(display_id, None);
    cx.simulate_display_bounds_change(display_id, Bounds::default());
    cx.simulate_display_refresh_interval_change(display_id, None);
    cx.simulate_display_removed(display_id);
    cx.simulate_display_removed(display_id);
    cx.simulate_display_bounds_change(display_id, bounds);
    cx.simulate_display_refresh_interval_change(display_id, Some(interval(144)));

    assert_eq!(
        events.take(),
        [
            DisplayEvent::Added(display_id),
            DisplayEvent::Changed {
                id: display_id,
                changes: DisplayChanges::BOUNDS,
            },
            DisplayEvent::Changed {
                id: display_id,
                changes: DisplayChanges::REFRESH_INTERVAL,
            },
            DisplayEvent::Changed {
                id: display_id,
                changes: DisplayChanges::BOUNDS | DisplayChanges::REFRESH_INTERVAL,
            },
            DisplayEvent::Removed(display_id),
        ]
    );
}

#[crate::test]
fn complete_app_display_state_is_fresh_before_app_and_window_observers(cx: &mut TestAppContext) {
    let first = DisplayId::new(1);
    let second = DisplayId::new(2);
    cx.simulate_display_added(second, Some(interval(60)));
    let first_window = cx.add_window(|_, _| Empty);
    let second_window = cx.add_window(|_, _| Empty);
    cx.simulate_window_move_to_display(second_window.into(), second);
    let observations = Rc::new(RefCell::new(Vec::new()));
    let check = |app: &App| {
        assert_eq!(
            app.display_refresh_interval(DisplayId::new(1)),
            Some(interval(120))
        );
        assert_eq!(
            app.display_refresh_interval(DisplayId::new(2)),
            Some(interval(144))
        );
        assert!(app.knows_display(DisplayId::new(1)));
        assert!(app.knows_display(DisplayId::new(2)));
    };
    let _app_subscription = cx.update({
        let observations = observations.clone();
        move |app| {
            app.observe_displays(move |event, app| {
                check(app);
                assert!(matches!(event, DisplayEvent::Changed { .. }));
                observations.borrow_mut().push("app");
            })
        }
    });
    let mut window_subscriptions = Vec::new();
    for handle in [first_window, second_window] {
        window_subscriptions.push(
            handle
                .update(cx, {
                    let observations = observations.clone();
                    move |_, window, _| {
                        window.observe_window_display(move |window, app| {
                            check(app);
                            let id = window.display(app).unwrap().id();
                            assert_eq!(
                                window.refresh_interval(app),
                                app.display_refresh_interval(id)
                            );
                            observations.borrow_mut().push("window");
                        })
                    }
                })
                .unwrap(),
        );
    }

    cx.test_platform()
        .connect_display(first, Some(interval(120)));
    cx.test_platform()
        .connect_display(second, Some(interval(144)));
    cx.simulate_displays_changed();
    assert_eq!(
        observations
            .borrow()
            .iter()
            .filter(|kind| **kind == "window")
            .count(),
        2
    );
    assert_eq!(
        observations
            .borrow()
            .iter()
            .filter(|kind| **kind == "app")
            .count(),
        2
    );
    cx.simulate_displays_changed();
    assert_eq!(observations.borrow().len(), 4);
}

#[crate::test]
fn displayless_window_notifies_once_and_can_return_to_a_display(cx: &mut TestAppContext) {
    let window = tracked_window(cx);
    let native_window = cx.test_window(window.into());
    cx.simulate_window_no_display(window.into());
    assert_eq!(native_window.platform_facts().display_id, None);
    window
        .update(cx, |tracker, window, app| {
            assert_eq!(window.refresh_interval(app), None);
            assert!(window.display(app).is_none());
            assert_eq!(window.platform_facts().display_id, None);
            assert_eq!(tracker.refresh_intervals, [None]);
        })
        .unwrap();
    native_window.simulate_no_display();
    cx.run_until_parked();
    cx.simulate_display_refresh_interval_change(DisplayId::new(1), Some(interval(120)));
    native_window.simulate_move_to_display(DisplayId::new(1));
    native_window.simulate_move_to_display(DisplayId::new(1));
    cx.run_until_parked();
    window
        .update(cx, |tracker, window, app| {
            assert_eq!(window.refresh_interval(app), Some(interval(120)));
            assert_eq!(tracker.refresh_intervals, [None, Some(interval(120))]);
        })
        .unwrap();
}

#[crate::test]
fn removed_window_display_has_no_refresh_interval(cx: &mut TestAppContext) {
    let window = cx.add_window(|_, _| Empty);
    cx.simulate_display_removed(DisplayId::new(1));
    window
        .update(cx, |_, window, app| {
            assert_eq!(window.refresh_interval(app), None);
            assert!(window.display(app).is_none());
            assert!(!app.knows_display(DisplayId::new(1)));
        })
        .unwrap();
    cx.simulate_window_no_display(window.into());
    cx.simulate_display_added(DisplayId::new(2), None);
    cx.simulate_window_move_to_display(window.into(), DisplayId::new(2));
    window
        .update(cx, |_, window, app| {
            assert_eq!(window.refresh_interval(app), None);
            assert_eq!(window.display(app).unwrap().id(), DisplayId::new(2));
        })
        .unwrap();
}

#[crate::test]
fn dropping_display_subscriptions_and_releasing_an_entity_stops_callbacks(cx: &mut TestAppContext) {
    let window = cx.add_window(|_, _| Empty);
    let app_count = Rc::new(Cell::new(0));
    let window_count = Rc::new(Cell::new(0));
    let entity_count = Rc::new(Cell::new(0));
    let app_subscription = cx.update({
        let count = app_count.clone();
        move |app| app.observe_displays(move |_, _| count.set(count.get() + 1))
    });
    let window_subscription = window
        .update(cx, {
            let count = window_count.clone();
            move |_, window, _| {
                window.observe_window_display(move |_, _| count.set(count.get() + 1))
            }
        })
        .unwrap();
    let owner = cx.new(|_| ());
    window
        .update(cx, {
            let count = entity_count.clone();
            let owner = owner.clone();
            move |_, window, app| {
                owner.update(app, |_, cx| {
                    cx.observe_window_display(window, move |_, _, _| count.set(count.get() + 1))
                        .detach();
                });
            }
        })
        .unwrap();
    cx.simulate_display_refresh_interval_change(DisplayId::new(1), Some(interval(120)));
    assert_eq!(
        (app_count.get(), window_count.get(), entity_count.get()),
        (1, 1, 1)
    );
    drop(app_subscription);
    drop(window_subscription);
    let lifetime = owner.downgrade();
    drop(owner);
    cx.update(|_| {
        assert!(
            lifetime.upgrade().is_none(),
            "an observer must not retain its entity"
        )
    });
    cx.simulate_display_refresh_interval_change(DisplayId::new(1), Some(interval(144)));
    assert_eq!(
        (app_count.get(), window_count.get(), entity_count.get()),
        (1, 1, 1)
    );
}

#[crate::test]
fn display_callbacks_wait_for_a_busy_app_and_keep_native_fifo(cx: &mut TestAppContext) {
    let delivered = Rc::new(RefCell::new(Vec::new()));
    let _subscription = cx.update({
        let delivered = delivered.clone();
        move |app| app.observe_displays(move |_, _| delivered.borrow_mut().push("display"))
    });
    let _wake = cx.read(|app| {
        app.on_system_wake({
            let delivered = delivered.clone();
            move |_| delivered.borrow_mut().push("wake")
        })
    });
    let simulator = cx.clone();
    cx.update(|app| {
        simulator.simulate_system_wake();
        simulator.simulate_display_added(DisplayId::new(2), Some(interval(120)));
        simulator.simulate_system_wake();
        assert!(delivered.borrow().is_empty());
        assert!(!app.knows_display(DisplayId::new(2)));
    });
    cx.run_until_parked();
    assert_eq!(delivered.take(), ["wake", "display", "wake"]);
    cx.read(|app| {
        assert_eq!(
            app.display_refresh_interval(DisplayId::new(2)),
            Some(interval(120))
        )
    });
}

#[crate::test]
fn display_observer_reentry_is_queued_after_the_current_notification(cx: &mut TestAppContext) {
    let observed = Rc::new(RefCell::new(Vec::new()));
    let inside = Rc::new(Cell::new(false));
    let _subscription = cx.update({
        let simulator = cx.clone();
        let observed = observed.clone();
        let inside = inside.clone();
        move |app| {
            app.observe_displays(move |event, app| {
                assert!(!inside.replace(true));
                observed.borrow_mut().push(event);
                if event == DisplayEvent::Added(DisplayId::new(2)) {
                    simulator.simulate_display_added(DisplayId::new(3), Some(interval(144)));
                    assert!(!app.knows_display(DisplayId::new(3)));
                }
                inside.set(false);
            })
        }
    });
    cx.simulate_display_added(DisplayId::new(2), Some(interval(120)));
    cx.run_until_parked();
    assert_eq!(
        observed.take(),
        [
            DisplayEvent::Added(DisplayId::new(2)),
            DisplayEvent::Added(DisplayId::new(3))
        ]
    );
}

#[crate::test]
fn window_display_callback_waits_for_a_busy_app_without_resizing(cx: &mut TestAppContext) {
    cx.simulate_display_added(DisplayId::new(2), Some(interval(120)));
    let window = tracked_window(cx);
    let native_window = cx.test_window(window.into());
    let original_bounds = window.update(cx, |_, window, _| window.bounds()).unwrap();
    cx.update(|app| {
        native_window.simulate_move_to_display(DisplayId::new(2));
        assert_eq!(native_window.display().unwrap().id(), DisplayId::new(2));
        assert!(window.read(app).unwrap().refresh_intervals.is_empty());
    });
    cx.run_until_parked();
    window
        .update(cx, |tracker, window, app| {
            assert_eq!(tracker.refresh_intervals, [Some(interval(120))]);
            assert_eq!(window.bounds(), original_bounds);
            assert_eq!(window.refresh_interval(app), Some(interval(120)));
            assert_eq!(window.platform_facts().display_id, Some(DisplayId::new(2)));
        })
        .unwrap();
}

#[crate::test]
fn bounds_facts_before_display_notification_do_not_swallow_an_unknown_target(
    cx: &mut TestAppContext,
) {
    let target = DisplayId::new(2);
    let window = tracked_window(cx);
    let mut native_window = cx.test_window(window.into());
    let events = Rc::new(RefCell::new(Vec::new()));
    let _subscription = cx.update({
        let events = events.clone();
        move |app| app.observe_displays(move |event, _| events.borrow_mut().push(event))
    });
    cx.test_platform()
        .connect_display(target, Some(interval(120)));
    let target_display = cx
        .test_platform()
        .display_snapshot()
        .find_display(target)
        .unwrap();
    native_window.set_display(Some(target_display));
    native_window.simulate_resize(size(px(800.), px(600.)));
    cx.run_until_parked();
    window
        .update(cx, |tracker, window, app| {
            assert_eq!(window.platform_facts().display_id, Some(target));
            assert_eq!(window.refresh_interval(app), Some(interval(60)));
            assert!(tracker.refresh_intervals.is_empty());
            assert!(!app.knows_display(target));
        })
        .unwrap();

    native_window.simulate_move_to_display(target);
    cx.run_until_parked();
    window
        .update(cx, |tracker, window, app| {
            assert_eq!(window.platform_facts().display_id, Some(target));
            assert_eq!(window.refresh_interval(app), Some(interval(120)));
            assert_eq!(tracker.refresh_intervals, [Some(interval(120))]);
        })
        .unwrap();
    assert_eq!(events.take(), [DisplayEvent::Added(target)]);
    cx.simulate_displays_changed();
    native_window.simulate_move_to_display(target);
    assert!(events.borrow().is_empty());
    window
        .update(cx, |tracker, _, _| {
            assert_eq!(tracker.refresh_intervals.len(), 1)
        })
        .unwrap();
}

#[crate::test]
fn window_display_observer_unwind_preserves_the_callback_and_association(cx: &mut TestAppContext) {
    cx.simulate_display_added(DisplayId::new(2), Some(interval(120)));
    let window = tracked_window(cx);
    let native_window = cx.test_window(window.into());
    let calls = Rc::new(Cell::new(0));
    let _subscription = window
        .update(cx, {
            let calls = calls.clone();
            move |_, window, _| {
                window.observe_window_display(move |_, _| {
                    calls.set(calls.get() + 1);
                    if calls.get() == 1 {
                        panic!("injected window display observer panic");
                    }
                })
            }
        })
        .unwrap();
    let result = catch_unwind(AssertUnwindSafe(|| {
        native_window.simulate_move_to_display(DisplayId::new(2));
        cx.run_until_parked();
    }));
    assert!(result.is_err());
    cx.run_until_parked();
    window
        .update(cx, |_, window, app| {
            assert_eq!(window.refresh_interval(app), Some(interval(120)))
        })
        .unwrap();
    native_window.simulate_move_to_display(DisplayId::new(1));
    cx.run_until_parked();
    window
        .update(cx, |tracker, _, _| {
            assert_eq!(
                tracker.refresh_intervals,
                [Some(interval(120)), Some(interval(60))]
            );
        })
        .unwrap();
    assert_eq!(calls.get(), 2);
}

#[crate::test]
fn display_observer_unwind_preserves_published_state_and_later_delivery(cx: &mut TestAppContext) {
    let calls = Rc::new(Cell::new(0));
    let _subscription = cx.update({
        let calls = calls.clone();
        move |app| {
            app.observe_displays(move |event, app| {
                calls.set(calls.get() + 1);
                let DisplayEvent::Added(id) = event else {
                    panic!("expected a connection")
                };
                assert!(app.knows_display(id));
                if calls.get() == 1 {
                    panic!("injected display observer panic");
                }
            })
        }
    });
    let result = catch_unwind(AssertUnwindSafe(|| {
        cx.simulate_display_added(DisplayId::new(2), Some(interval(120)));
    }));
    assert!(result.is_err());
    cx.run_until_parked();
    cx.read(|app| {
        assert_eq!(
            app.display_refresh_interval(DisplayId::new(2)),
            Some(interval(120))
        )
    });
    cx.simulate_display_added(DisplayId::new(3), Some(interval(144)));
    cx.run_until_parked();
    assert_eq!(calls.get(), 2);
}

#[crate::test]
fn window_display_observer_can_close_its_window(cx: &mut TestAppContext) {
    cx.update(|app| app.set_quit_mode(QuitMode::Explicit));
    cx.simulate_display_added(DisplayId::new(2), Some(interval(120)));
    let window = cx.add_window(|_, _| Empty);
    let native_window = cx.test_window(window.into());
    let calls = Rc::new(Cell::new(0));
    let _subscription = window
        .update(cx, {
            let calls = calls.clone();
            move |_, window, _| {
                window.observe_window_display(move |window, app| {
                    calls.set(calls.get() + 1);
                    assert_eq!(window.refresh_interval(app), Some(interval(120)));
                    window.remove_window(app);
                })
            }
        })
        .unwrap();
    native_window.simulate_move_to_display(DisplayId::new(2));
    cx.run_until_parked();
    assert_eq!(calls.get(), 1);
    assert!(cx.windows().is_empty());
    assert!(native_window.is_native_terminal());
    native_window.simulate_move_to_display(DisplayId::new(1));
    cx.simulate_display_refresh_interval_change(DisplayId::new(2), Some(interval(144)));
    assert_eq!(calls.get(), 1);
}

#[crate::test]
fn closed_window_discards_a_queued_display_callback(cx: &mut TestAppContext) {
    cx.update(|app| app.set_quit_mode(QuitMode::Explicit));
    cx.simulate_display_added(DisplayId::new(2), Some(interval(120)));
    let window = cx.add_window(|_, _| Empty);
    let native_window = cx.test_window(window.into());
    let calls = Rc::new(Cell::new(0));
    let _subscription = window
        .update(cx, {
            let calls = calls.clone();
            move |_, window, _| {
                window.observe_window_display(move |_, _| calls.set(calls.get() + 1))
            }
        })
        .unwrap();
    window
        .update(cx, |_, window, app| {
            native_window.simulate_move_to_display(DisplayId::new(2));
            window.remove_window(app);
        })
        .unwrap();
    cx.run_until_parked();
    assert_eq!(calls.get(), 0);
    assert!(cx.windows().is_empty());
}

#[crate::test]
fn unknown_display_observer_cannot_deliver_into_a_replacement_window(cx: &mut TestAppContext) {
    use crate::{
        NativeBoundaryDiagnosticCursor, NativeBoundaryDisposition, NativeBoundaryKind,
        NativeBoundaryTarget, NativeCallbackKind,
    };

    cx.update(|app| app.set_quit_mode(QuitMode::Explicit));
    let target = DisplayId::new(2);
    let original = cx.add_window(|_, _| Empty);
    let calls = Rc::new(Cell::new(0));
    let subscriptions = Rc::new(RefCell::new(Vec::new()));
    let original_subscription = original
        .update(cx, {
            let calls = calls.clone();
            move |_, window, _| {
                window.observe_window_display(move |_, _| calls.set(calls.get() + 1))
            }
        })
        .unwrap();
    subscriptions.borrow_mut().push(original_subscription);
    let replacement = Rc::new(RefCell::new(None));
    let _subscription = cx.update({
        let replacement = replacement.clone();
        let subscriptions = subscriptions.clone();
        let calls = calls.clone();
        move |app| {
            app.observe_displays(move |event, app| {
                if event != DisplayEvent::Added(target) {
                    return;
                }
                original
                    .update(app, |_, window, app| window.remove_window(app))
                    .unwrap();
                let handle = app
                    .open_window(Default::default(), |_, app| app.new(|_| Empty))
                    .unwrap();
                let subscription = handle
                    .update(app, {
                        let calls = calls.clone();
                        move |_, window, _| {
                            window.observe_window_display(move |_, _| calls.set(calls.get() + 1))
                        }
                    })
                    .unwrap();
                subscriptions.borrow_mut().push(subscription);
                replacement.replace(Some(handle));
            })
        }
    });
    let cursor = cx.update(|app| {
        app.native_boundary_diagnostics(NativeBoundaryDiagnosticCursor::default())
            .cursor
    });
    cx.test_platform()
        .connect_display(target, Some(interval(120)));
    cx.simulate_window_move_to_display(original.into(), target);
    let replacement = replacement
        .borrow()
        .expect("the App display observer must replace the window");
    assert_ne!(replacement.window_id(), original.window_id());
    assert_eq!(calls.get(), 0);
    let diagnostics = cx.update(|app| app.native_boundary_diagnostics(cursor));
    assert_eq!(diagnostics.omitted_before_cursor, 0);
    let delivered = diagnostics
        .terminal
        .iter()
        .filter(|diagnostic| {
            diagnostic.target == NativeBoundaryTarget::Window(original.window_id())
                && diagnostic.kind
                    == NativeBoundaryKind::Callback(NativeCallbackKind::DisplayChanged)
        })
        .collect::<Vec<_>>();
    assert_eq!(delivered.len(), 1);
    assert!(matches!(
        delivered[0].disposition,
        NativeBoundaryDisposition::Stale | NativeBoundaryDisposition::Closed
    ));
    assert!(diagnostics.pending.iter().all(|diagnostic| {
        diagnostic.target != NativeBoundaryTarget::Window(original.window_id())
    }));
}

#[crate::test]
fn queued_close_after_a_display_hint_rejects_handler_reentry(cx: &mut TestAppContext) {
    cx.update(|app| app.set_quit_mode(QuitMode::Explicit));
    let handle = cx.add_window(|_, _| Empty);
    let native = cx.test_window(handle.into());
    let calls = Rc::new(Cell::new(0));
    let nested = Rc::new(Cell::new(None));
    handle
        .update(cx, {
            let native = native.clone();
            let calls = calls.clone();
            let nested = nested.clone();
            move |_, window, app| {
                window.on_window_should_close(app, move |_, _| {
                    calls.set(calls.get() + 1);
                    if calls.get() == 1 {
                        nested.set(native.simulate_should_close());
                    }
                    false
                });
            }
        })
        .unwrap();
    cx.update(|_| {
        cx.test_platform().simulate_displays_changed();
        assert_eq!(native.simulate_should_close(), Some(false));
        assert_eq!(calls.get(), 0);
    });
    cx.run_until_parked();
    assert_eq!(nested.get(), Some(false));
    assert_eq!(calls.get(), 1);
    assert!(!native.is_native_terminal());
    assert_eq!(native.simulate_should_close(), Some(false));
    cx.run_until_parked();
    assert_eq!(
        calls.get(),
        2,
        "a later close request must remain available"
    );
}

fn launch_from_empty_displays(embedded: bool) {
    let dispatcher = TestDispatcher::new(0);
    let scheduler = Arc::new(dispatcher.clone());
    let platform = TestPlatform::new(
        BackgroundExecutor::new(scheduler.clone()),
        ForegroundExecutor::new(scheduler),
    );
    platform.simulate_display_removed(DisplayId::new(1));
    let application =
        Application::with_platform(platform.clone()).with_quit_mode(QuitMode::Explicit);
    let app_owner = application.0.clone();
    assert!(app_owner.borrow().displays().is_empty());
    platform.defer_next_launch();
    let launched = Rc::new(Cell::new(false));
    let callback = {
        let launched = launched.clone();
        move |app: &mut App| {
            assert_eq!(
                app.display_refresh_interval(DisplayId::new(2)),
                Some(interval(120))
            );
            assert!(app.knows_display(DisplayId::new(2)));
            assert_eq!(app.primary_display().unwrap().id(), DisplayId::new(2));
            launched.set(true);
        }
    };
    let handle = if embedded {
        Some(application.run_embedded(callback))
    } else {
        application.run(callback);
        None
    };
    assert!(!launched.get());
    platform.connect_display(DisplayId::new(2), Some(interval(120)));
    dispatcher.run_until_parked();
    assert!(launched.get());
    app_owner.borrow_mut().shutdown();
    drop(handle);
}

#[test]
fn regular_launch_reloads_displays_after_initially_empty_construction() {
    launch_from_empty_displays(false);
}

#[test]
fn embedded_launch_reloads_displays_after_initially_empty_construction() {
    launch_from_empty_displays(true);
}
