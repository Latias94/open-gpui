use super::*;
use crate::{Empty, TestDispatcher, TestPlatform, WindowBounds, WindowOptions, px, size};

fn application() -> (Application, Rc<TestPlatform>, TestDispatcher) {
    let dispatcher = TestDispatcher::new(0);
    let scheduler = Arc::new(dispatcher.clone());
    let platform = TestPlatform::new(
        BackgroundExecutor::new(scheduler.clone()),
        ForegroundExecutor::new(scheduler),
    );
    let app = Application::with_platform(platform.clone()).with_quit_mode(QuitMode::Explicit);
    (app, platform, dispatcher)
}

#[test]
fn embedded_launch_returns_a_strong_owner_and_updates_flush_the_whole_batch() {
    let (app, platform, _) = application();
    let lifetime = Rc::downgrade(&app.0);
    let launched = Rc::new(Cell::new(0));
    let handle = app.run_embedded({
        let launched = launched.clone();
        move |_| launched.set(launched.get() + 1)
    });
    assert_eq!(launched.get(), 1);
    assert!(lifetime.upgrade().is_some());

    let effects = Rc::new(RefCell::new(Vec::new()));
    let observed = Rc::new(Cell::new(0));
    let entity = handle.update(|cx| cx.new(|_| 0usize));
    let subscription = handle.update(|cx| {
        let effects = effects.clone();
        let observed = observed.clone();
        cx.observe(&entity, move |_, _| {
            effects.borrow_mut().push("notify");
            observed.set(observed.get() + 1);
        })
    });
    let generation = handle.update(|cx| {
        let generation = cx.current_update_generation();
        cx.defer({
            let effects = effects.clone();
            move |cx| {
                assert_eq!(cx.current_update_generation(), generation);
                effects.borrow_mut().push("defer");
            }
        });
        entity.update(cx, |value, cx| {
            *value = 42;
            cx.notify();
        });
        assert!(
            effects.borrow().is_empty(),
            "nested entity updates must stay in the host batch"
        );
        assert_eq!(cx.current_update_generation(), generation);
        generation
    });
    assert_eq!(effects.borrow().as_slice(), ["defer", "notify"]);
    assert_eq!(observed.get(), 1);
    assert_eq!(handle.update(|cx| *entity.read(cx)), 42);
    assert!(handle.update(|cx| cx.current_update_generation()) > generation);
    assert_eq!(launched.get(), 1);
    drop(subscription);
    drop(entity);
    handle.update(|cx| cx.shutdown());
    drop(handle);
    assert!(lifetime.upgrade().is_none());
    drop(platform);
}

#[test]
fn embedded_defer_only_updates_flush_before_returning() {
    let (app, _, _) = application();
    let handle = app.run_embedded(|_| {});
    let ran = Rc::new(Cell::new(false));
    handle.update(|cx| {
        cx.defer({
            let ran = ran.clone();
            move |_| ran.set(true)
        });
        assert!(!ran.get());
    });
    assert!(ran.get());
    handle.update(|cx| cx.shutdown());
}

#[test]
fn embedded_reentrant_entry_panics_without_poisoning_the_existing_update_owner() {
    let (app, _, dispatcher) = application();
    let handle = app.run_embedded(|_| {});
    let reentry = catch_unwind(AssertUnwindSafe(|| {
        handle.update(|_| handle.update(|_| {}));
    }));
    assert!(reentry.is_err());
    let async_reentry = catch_unwind(AssertUnwindSafe(|| {
        handle.update(|_| handle.to_async());
    }));
    assert!(async_reentry.is_err());
    dispatcher.run_until_parked();
    assert_eq!(
        handle.update(|cx| (cx.pending_updates, cx.flushing_effects)),
        (1, false)
    );
    assert_eq!(handle.app.borrow().pending_updates, 0);
    assert_eq!(handle.update(|_| 42), 42);
    handle.update(|cx| cx.shutdown());
}

#[test]
fn embedded_panicking_update_recovers_and_defers_are_not_implicitly_rolled_back() {
    let (app, _, dispatcher) = application();
    let handle = app.run_embedded(|_| {});
    let ran = Rc::new(Cell::new(0));
    let result = catch_unwind(AssertUnwindSafe(|| {
        handle.update(|cx| {
            cx.defer({
                let ran = ran.clone();
                move |_| ran.set(ran.get() + 1)
            });
            panic!("host update panic");
        });
    }));
    assert!(result.is_err());
    assert_eq!(ran.get(), 0);
    dispatcher.run_until_parked();
    handle.update(|cx| assert_eq!(cx.pending_updates, 1));
    assert_eq!(ran.get(), 1);
    handle.update(|_| {});
    assert_eq!(ran.get(), 1);
    handle.update(|cx| cx.shutdown());
}

#[test]
fn embedded_native_callback_waits_for_the_host_borrow_then_runs_once() {
    let (app, platform, dispatcher) = application();
    let delivered = Rc::new(Cell::new(0));
    let handle = app.run_embedded(|_| {});
    let _subscription = handle.update(|cx| {
        cx.on_system_wake({
            let delivered = delivered.clone();
            move |_| delivered.set(delivered.get() + 1)
        })
    });
    handle.update(|_| {
        platform.simulate_system_wake();
        assert_eq!(delivered.get(), 0);
    });
    dispatcher.run_until_parked();
    assert_eq!(delivered.get(), 1);
    handle.update(|cx| cx.shutdown());
}

#[test]
fn embedded_async_updates_share_the_live_owner_without_retaining_it() {
    let (app, _, dispatcher) = application();
    let lifetime = Rc::downgrade(&app.0);
    let handle = app.run_embedded(|_| {});
    let asynchronous = handle.to_async();
    let ran = Rc::new(Cell::new(0));
    let task = handle.update(|cx| {
        cx.spawn({
            let ran = ran.clone();
            async move |cx| {
                cx.background_executor()
                    .timer(Duration::from_millis(10))
                    .await;
                cx.update(|cx| {
                    cx.defer(move |_| ran.set(1));
                });
            }
        })
    });
    dispatcher.run_until_parked();
    assert_eq!(ran.get(), 0);
    dispatcher.advance_clock(Duration::from_millis(10));
    dispatcher.run_until_parked();
    assert_eq!(ran.get(), 1);
    asynchronous.update(|_| assert_eq!(ran.get(), 1));
    drop(task);
    handle.update(|cx| cx.shutdown());
    drop(handle);
    assert!(lifetime.upgrade().is_none());
    assert!(asynchronous.app.upgrade().is_none());
    let released = catch_unwind(AssertUnwindSafe(|| asynchronous.update(|_| {})));
    assert!(released.is_err());
}

#[test]
fn embedded_shutdown_observers_and_window_retirement_finish_before_owner_release() {
    let (app, _, dispatcher) = application();
    let lifetime = Rc::downgrade(&app.0);
    let window = Rc::new(Cell::new(None));
    let handle = app.run_embedded({
        let window = window.clone();
        move |cx| {
            window.set(Some(
                cx.open_window(
                    WindowOptions {
                        window_bounds: Some(WindowBounds::Windowed(Bounds::new(
                            Point::default(),
                            size(px(120.), px(80.)),
                        ))),
                        focus_on_appearing: false,
                        show: false,
                        ..WindowOptions::default()
                    },
                    |_, cx| cx.new(|_| Empty),
                )
                .unwrap(),
            ));
        }
    });
    let window = window.get().unwrap();
    let finished = Rc::new(Cell::new(0));
    let subscription = handle.update(|cx| {
        let finished = finished.clone();
        cx.on_app_quit(move |cx| {
            let error = cx
                .open_window_detailed(WindowOptions::default(), |_, cx| cx.new(|_| Empty))
                .expect_err("shutdown preparation must refuse new windows");
            assert_eq!(error.stage(), WindowOpenFailureStage::AppShutdown);
            let finished = finished.clone();
            async move { finished.set(finished.get() + 1) }
        })
    });
    let root = handle.update(|cx| window.root(cx)).unwrap();
    let root_lifetime = root.downgrade();
    drop(root);
    handle.update(|cx| cx.shutdown());
    dispatcher.run_until_parked();
    assert_eq!(finished.get(), 1);
    handle.update(|cx| {
        assert!(cx.windows().is_empty());
        assert!(!cx.quitting);
        assert_eq!(cx.window_open_barrier_depth, 0);
    });
    assert!(root_lifetime.upgrade().is_none());
    drop(subscription);
    drop(handle);
    assert!(lifetime.upgrade().is_none());
}

#[test]
fn embedded_delayed_launch_is_not_a_false_synchronous_ready_or_cancel_promise() {
    let (app, platform, dispatcher) = application();
    platform.defer_next_launch();
    let lifetime = Rc::downgrade(&app.0);
    let launched = Rc::new(Cell::new(0));
    let handle = app.run_embedded({
        let launched = launched.clone();
        move |_| launched.set(launched.get() + 1)
    });
    assert_eq!(launched.get(), 0);
    assert_eq!(handle.update(|_| 7), 7);
    drop(handle);
    assert!(
        lifetime.upgrade().is_some(),
        "the pending platform launch retains its callback App"
    );
    dispatcher.run_until_parked();
    assert_eq!(launched.get(), 1);
    assert!(lifetime.upgrade().is_none());
}

#[test]
fn embedded_delayed_launch_stays_live_under_its_host_handle() {
    let (app, platform, dispatcher) = application();
    platform.defer_next_launch();
    let lifetime = Rc::downgrade(&app.0);
    let launched = Rc::new(Cell::new(0));
    let handle = app.run_embedded({
        let launched = launched.clone();
        move |_| launched.set(launched.get() + 1)
    });
    let asynchronous = handle.to_async();
    assert_eq!(launched.get(), 0);
    dispatcher.run_until_parked();
    assert_eq!(launched.get(), 1);
    assert_eq!(handle.update(|_| 42), 42);
    assert!(lifetime.upgrade().is_some());
    handle.update(|cx| cx.shutdown());
    drop(handle);
    assert!(lifetime.upgrade().is_none());
    assert!(asynchronous.app.upgrade().is_none());
}

#[test]
#[cfg(not(target_family = "wasm"))]
fn ordinary_and_returning_test_launch_keep_their_existing_owner_lifetimes() {
    let (app, _, _) = application();
    let lifetime = Rc::downgrade(&app.0);
    let launched = Rc::new(Cell::new(0));
    app.run({
        let launched = launched.clone();
        move |_| launched.set(launched.get() + 1)
    });
    assert_eq!(launched.get(), 1);
    assert!(lifetime.upgrade().is_none());

    let (mut app, _, _) = application();
    let lifetime = Rc::downgrade(&app.0);
    app.run_returning_for_test({
        let launched = launched.clone();
        move |_| launched.set(launched.get() + 1)
    });
    assert_eq!(launched.get(), 2);
    assert!(lifetime.upgrade().is_some());
    app.update_for_test(|cx| cx.shutdown());
    drop(app);
    assert!(lifetime.upgrade().is_none());
}
