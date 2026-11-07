use super::Application;
use crate::{
    DisplayId, Empty, NativeAppNotificationHandler, NativeBoundaryDiagnosticCursor,
    NativeBoundaryDisposition, NativeBoundaryKind, NativeBoundaryTarget, NativeCallbackKind,
    NativeInvariantFailure, Platform, PowerMode, QuitMode, RequestFrameOptions, Subscription,
    TestAppContext, TestPlatform, ThermalState,
};
use std::{
    cell::{Cell, RefCell},
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
};

#[crate::test]
fn power_mode_initializes_from_actual_platform_state(cx: &mut TestAppContext) {
    assert_eq!(cx.read(|app| app.power_mode()), PowerMode::Unknown);
    for mode in [
        PowerMode::LowPower,
        PowerMode::Balanced,
        PowerMode::HighPower,
        PowerMode::Unknown,
    ] {
        let platform = TestPlatform::new(
            cx.background_executor.clone(),
            cx.foreground_executor.clone(),
        );
        platform.simulate_power_mode_change(mode);
        let application = Application::with_platform(platform);
        assert_eq!(application.0.borrow().power_mode(), mode);
        application.0.borrow_mut().shutdown();
    }
}

#[crate::test]
fn busy_app_updates_power_before_observers_and_preserves_native_order(cx: &mut TestAppContext) {
    let observed = Rc::new(RefCell::new(Vec::new()));
    let _subscriptions = cx.read(|app| {
        let power = app.on_power_mode_change({
            let observed = observed.clone();
            move |app| {
                assert_eq!(app.power_mode(), app.platform.power_mode());
                observed.borrow_mut().push(("power", app.power_mode()));
                let observed = observed.clone();
                app.defer(move |app| {
                    observed
                        .borrow_mut()
                        .push(("power-effect", app.power_mode()));
                });
            }
        });
        let thermal = app.on_thermal_state_change({
            let observed = observed.clone();
            move |app| {
                assert_eq!(app.thermal_state(), ThermalState::Serious);
                observed.borrow_mut().push(("thermal", app.power_mode()));
            }
        });
        let sleep = app.on_system_sleep({
            let observed = observed.clone();
            move |app| observed.borrow_mut().push(("sleep", app.power_mode()))
        });
        let wake = app.on_system_wake({
            let observed = observed.clone();
            move |app| observed.borrow_mut().push(("wake", app.power_mode()))
        });
        let display = app.observe_displays({
            let observed = observed.clone();
            move |_, app| {
                assert!(app.knows_display(DisplayId::new(2)));
                observed.borrow_mut().push(("display", app.power_mode()));
            }
        });
        [power, thermal, sleep, wake, display]
    });
    Application(cx.app.clone()).on_reopen({
        let observed = observed.clone();
        move |app| observed.borrow_mut().push(("reopen", app.power_mode()))
    });
    let platform = cx.test_platform();
    cx.update(|app| {
        platform.simulate_power_mode_change(PowerMode::LowPower);
        platform.simulate_power_mode_change(PowerMode::Balanced);
        platform.simulate_thermal_state_change(ThermalState::Serious);
        platform.simulate_power_mode_change(PowerMode::HighPower);
        platform.simulate_system_sleep();
        platform.simulate_system_wake();
        platform.simulate_display_added(DisplayId::new(2), None);
        platform.simulate_reopen();
        assert!(observed.borrow().is_empty());
        assert_eq!(app.power_mode(), PowerMode::Unknown);
        assert_eq!(platform.power_mode(), PowerMode::HighPower);
    });
    cx.run_until_parked();
    assert_eq!(
        observed.take(),
        [
            ("power", PowerMode::HighPower),
            ("power-effect", PowerMode::HighPower),
            ("thermal", PowerMode::HighPower),
            ("power", PowerMode::HighPower),
            ("power-effect", PowerMode::HighPower),
            ("sleep", PowerMode::HighPower),
            ("wake", PowerMode::HighPower),
            ("display", PowerMode::HighPower),
            ("reopen", PowerMode::HighPower),
        ]
    );
    let diagnostics =
        cx.read(|app| app.native_boundary_diagnostics(NativeBoundaryDiagnosticCursor::default()));
    assert!(diagnostics.pending.is_empty());
    let mut power = diagnostics
        .terminal
        .iter()
        .filter(|entry| {
            entry.kind == NativeBoundaryKind::Callback(NativeCallbackKind::PowerModeChanged)
        })
        .collect::<Vec<_>>();
    power.sort_by_key(|entry| entry.sequence);
    assert_eq!(power.len(), 3);
    assert!(
        power
            .iter()
            .all(|entry| entry.target == NativeBoundaryTarget::Application)
    );
    assert_eq!(
        power[0].disposition,
        NativeBoundaryDisposition::Coalesced {
            into_sequence: power[1].sequence,
        }
    );
    assert_eq!(power[1].disposition, NativeBoundaryDisposition::DELIVERED);
    assert_eq!(power[2].disposition, NativeBoundaryDisposition::DELIVERED);
}

#[crate::test]
fn power_cache_stays_consistent_for_live_readers_until_native_delivery(cx: &mut TestAppContext) {
    let observed = Rc::new(RefCell::new(Vec::new()));
    let _subscription = cx.read(|app| {
        app.on_power_mode_change({
            let observed = observed.clone();
            move |app| observed.borrow_mut().push(app.power_mode())
        })
    });
    let simulator = cx.clone();
    cx.read(|app| {
        simulator.simulate_power_mode(PowerMode::LowPower);
        assert_eq!(app.power_mode(), PowerMode::Unknown);
        assert_eq!(app.platform.power_mode(), PowerMode::LowPower);
        assert_eq!(simulator.read(|app| app.power_mode()), PowerMode::Unknown);
        assert!(observed.borrow().is_empty());
    });
    cx.run_until_parked();
    assert_eq!(observed.take(), [PowerMode::LowPower]);
    assert_eq!(cx.read(|app| app.power_mode()), PowerMode::LowPower);
}

#[crate::test]
fn busy_app_delivers_power_before_later_frame_reads_cached_mode(cx: &mut TestAppContext) {
    let handle = cx.add_window(|_, _| Empty);
    let native = cx.test_window(handle.into());
    cx.run_until_parked();
    let cursor = cx.read(|app| {
        app.native_boundary_diagnostics(NativeBoundaryDiagnosticCursor::default())
            .cursor
    });
    let observed = Rc::new(RefCell::new(Vec::new()));
    let _subscription = cx.read(|app| {
        app.on_power_mode_change({
            let observed = observed.clone();
            move |app| observed.borrow_mut().push(("observer", app.power_mode()))
        })
    });
    let simulator = cx.clone();
    handle
        .update(cx, |_, window, app| {
            simulator
                .test_platform()
                .simulate_power_mode_change(PowerMode::LowPower);
            window.on_next_frame({
                let observed = observed.clone();
                move |_, app| observed.borrow_mut().push(("frame", app.power_mode()))
            });
            assert!(native.simulate_frame(RequestFrameOptions {
                require_presentation: true,
                ..Default::default()
            }));
            assert_eq!(app.power_mode(), PowerMode::Unknown);
            assert!(observed.borrow().is_empty());
            let diagnostics = app.native_boundary_diagnostics(cursor);
            let power = diagnostics
                .pending
                .iter()
                .find(|entry| {
                    entry.kind == NativeBoundaryKind::Callback(NativeCallbackKind::PowerModeChanged)
                })
                .expect("the power notification must retain its pending native sequence");
            let frame = diagnostics
                .pending
                .iter()
                .find(|entry| {
                    entry.kind == NativeBoundaryKind::Callback(NativeCallbackKind::RequestFrame)
                })
                .expect("the later frame must retain its pending native sequence");
            assert_eq!(power.target, NativeBoundaryTarget::Application);
            assert_eq!(
                frame.target,
                NativeBoundaryTarget::Window(handle.window_id())
            );
            assert!(power.sequence < frame.sequence);
        })
        .unwrap();
    cx.run_until_parked();
    assert_eq!(
        observed.take(),
        [
            ("observer", PowerMode::LowPower),
            ("frame", PowerMode::LowPower)
        ]
    );
    assert!(cx.read(|app| app.native_boundary_diagnostics(cursor).pending.is_empty()));
}

#[crate::test]
fn power_observers_have_independent_lifetimes_and_release_captures(cx: &mut TestAppContext) {
    let observed = Rc::new(RefCell::new(Vec::new()));
    let owner = Rc::new(());
    let weak_owner = Rc::downgrade(&owner);
    let (first, second) = cx.read(|app| {
        let first = app.on_power_mode_change({
            let observed = observed.clone();
            move |app| {
                let _ = Rc::strong_count(&owner);
                observed.borrow_mut().push(("first", app.power_mode()));
            }
        });
        let second = app.on_power_mode_change({
            let observed = observed.clone();
            move |app| observed.borrow_mut().push(("second", app.power_mode()))
        });
        (first, second)
    });
    cx.simulate_power_mode(PowerMode::LowPower);
    assert_eq!(
        observed.take(),
        [
            ("first", PowerMode::LowPower),
            ("second", PowerMode::LowPower)
        ]
    );
    drop(first);
    assert!(weak_owner.upgrade().is_none());
    second.detach();
    cx.simulate_power_mode(PowerMode::Balanced);
    cx.simulate_power_mode(PowerMode::Unknown);
    assert_eq!(
        observed.take(),
        [
            ("second", PowerMode::Balanced),
            ("second", PowerMode::Unknown)
        ]
    );
    assert_eq!(cx.read(|app| app.power_mode()), PowerMode::Unknown);
}

#[crate::test]
fn power_observer_can_replace_itself_and_enqueue_another_notification(cx: &mut TestAppContext) {
    let observed = Rc::new(RefCell::new(Vec::new()));
    let current = Rc::new(RefCell::new(None::<Subscription>));
    let later = Rc::new(RefCell::new(None::<Subscription>));
    let ingress = cx.app.native_application_ingress();
    let first = cx.read(|app| {
        app.on_power_mode_change({
            let current = current.clone();
            let later = later.clone();
            let observed = observed.clone();
            move |app| {
                observed.borrow_mut().push("first");
                current.borrow_mut().take();
                later.borrow_mut().take();
                let observed = observed.clone();
                *current.borrow_mut() = Some(app.on_power_mode_change(move |_| {
                    observed.borrow_mut().push("replacement");
                }));
                ingress.power_mode_changed();
            }
        })
    });
    *current.borrow_mut() = Some(first);
    *later.borrow_mut() =
        Some(cx.read(|app| {
            app.on_power_mode_change(|_| panic!("an unsubscribed observer must not run"))
        }));
    cx.simulate_power_mode(PowerMode::LowPower);
    assert_eq!(observed.take(), ["first", "replacement"]);
    cx.simulate_power_mode(PowerMode::Balanced);
    assert_eq!(observed.take(), ["replacement"]);
    current.borrow_mut().take();
}

#[crate::test]
fn power_observer_unwind_keeps_cached_state_subscriptions_and_diagnostics(cx: &mut TestAppContext) {
    let observed = Rc::new(RefCell::new(Vec::new()));
    let failed = cx.read(|app| {
        app.on_power_mode_change(|app| {
            assert_eq!(app.power_mode(), PowerMode::LowPower);
            panic!("injected power observer panic");
        })
    });
    let _survivor = cx.read(|app| {
        app.on_power_mode_change({
            let observed = observed.clone();
            move |app| observed.borrow_mut().push(app.power_mode())
        })
    });
    assert!(
        catch_unwind(AssertUnwindSafe(|| {
            cx.simulate_power_mode(PowerMode::LowPower);
        }))
        .is_err()
    );
    assert_eq!(cx.read(|app| app.power_mode()), PowerMode::LowPower);
    assert!(observed.borrow().is_empty());
    drop(failed);
    cx.simulate_power_mode(PowerMode::HighPower);
    assert_eq!(observed.take(), [PowerMode::HighPower]);
    let diagnostics =
        cx.read(|app| app.native_boundary_diagnostics(NativeBoundaryDiagnosticCursor::default()));
    assert!(diagnostics.pending.is_empty());
    let power = diagnostics
        .terminal
        .iter()
        .filter(|entry| {
            entry.kind == NativeBoundaryKind::Callback(NativeCallbackKind::PowerModeChanged)
        })
        .collect::<Vec<_>>();
    assert_eq!(power.len(), 2);
    assert_eq!(
        power[0].disposition,
        NativeBoundaryDisposition::InvariantFailure(NativeInvariantFailure::CallbackPanicked)
    );
    assert_eq!(power[1].disposition, NativeBoundaryDisposition::DELIVERED);
}

#[crate::test]
fn busy_app_unwind_retains_staged_power_notification(cx: &mut TestAppContext) {
    let observed = Rc::new(RefCell::new(Vec::new()));
    let _subscription = cx.read(|app| {
        app.on_power_mode_change({
            let observed = observed.clone();
            move |app| observed.borrow_mut().push(app.power_mode())
        })
    });
    assert!(
        catch_unwind(AssertUnwindSafe(|| {
            cx.update(|_| {
                cx.test_platform()
                    .simulate_power_mode_change(PowerMode::LowPower);
                panic!("injected busy App unwind");
            });
        }))
        .is_err()
    );
    assert!(observed.borrow().is_empty());
    cx.run_until_parked();
    assert_eq!(observed.take(), [PowerMode::LowPower]);
    assert_eq!(cx.read(|app| app.power_mode()), PowerMode::LowPower);
}

#[crate::test]
fn foreign_thread_power_notification_orders_before_local_work(cx: &mut TestAppContext) {
    cx.simulate_power_mode(PowerMode::LowPower);
    let observed = Rc::new(RefCell::new(Vec::new()));
    let _subscriptions = cx.read(|app| {
        let power = app.on_power_mode_change({
            let observed = observed.clone();
            move |app| {
                assert_eq!(app.power_mode(), PowerMode::LowPower);
                observed.borrow_mut().push("power");
            }
        });
        let sleep = app.on_system_sleep({
            let observed = observed.clone();
            move |_| observed.borrow_mut().push("sleep")
        });
        Subscription::join(power, sleep)
    });
    Application(cx.app.clone()).on_reopen({
        let observed = observed.clone();
        move |_| observed.borrow_mut().push("reopen")
    });
    let ingress = cx.app.native_application_ingress();
    let callback: NativeAppNotificationHandler = cx
        .test_platform()
        .power_mode_notification_handler_for_test();
    cx.update(|_| {
        callback();
        std::thread::spawn(move || {
            callback();
            ingress.system_sleep();
        })
        .join()
        .expect("foreign-thread power notification must stage without borrowing App");
        cx.test_platform().simulate_reopen();
        assert!(observed.borrow().is_empty());
    });
    cx.run_until_parked();
    assert_eq!(observed.take(), ["power", "sleep", "reopen"]);
}

#[crate::test]
fn power_and_thermal_notifications_leave_other_apps_and_windows_independent(
    cx: &mut TestAppContext,
) {
    let other = cx.new_app();
    other.simulate_power_mode(PowerMode::Balanced);
    let first = cx.add_window(|_, _| Empty);
    let second = cx.add_window(|_, _| Empty);
    let first_native = cx.test_window(first.into());
    let second_native = cx.test_window(second.into());
    cx.run_until_parked();
    let initial = [
        (first_native.draw_count(), first_native.frame_wake_count()),
        (second_native.draw_count(), second_native.frame_wake_count()),
    ];
    let power_calls = Rc::new(Cell::new(0));
    let thermal_calls = Rc::new(Cell::new(0));
    let other_calls = Rc::new(Cell::new(0));
    let _power = cx.read(|app| {
        app.on_power_mode_change({
            let power_calls = power_calls.clone();
            move |_| power_calls.set(power_calls.get() + 1)
        })
    });
    let _thermal = cx.read(|app| {
        app.on_thermal_state_change({
            let thermal_calls = thermal_calls.clone();
            move |_| thermal_calls.set(thermal_calls.get() + 1)
        })
    });
    let _other_power = other.read(|app| {
        app.on_power_mode_change({
            let other_calls = other_calls.clone();
            move |_| other_calls.set(other_calls.get() + 1)
        })
    });
    for mode in [
        PowerMode::LowPower,
        PowerMode::Balanced,
        PowerMode::HighPower,
        PowerMode::Unknown,
    ] {
        cx.simulate_power_mode(mode);
        assert_eq!(cx.read(|app| app.thermal_state()), ThermalState::Nominal);
        assert_eq!(thermal_calls.get(), 0);
        for window in [first, second] {
            assert_eq!(
                window.update(cx, |_, _, app| app.power_mode()).unwrap(),
                mode
            );
        }
    }
    cx.simulate_thermal_state_change(ThermalState::Critical);
    assert_eq!(power_calls.get(), 4);
    assert_eq!(thermal_calls.get(), 1);
    assert_eq!(cx.read(|app| app.power_mode()), PowerMode::Unknown);
    assert_eq!(other.read(|app| app.power_mode()), PowerMode::Balanced);
    assert_eq!(other.read(|app| app.thermal_state()), ThermalState::Nominal);
    assert_eq!(other_calls.get(), 0);
    assert_eq!(
        initial,
        [
            (first_native.draw_count(), first_native.frame_wake_count()),
            (second_native.draw_count(), second_native.frame_wake_count()),
        ]
    );
    other.simulate_power_mode(PowerMode::HighPower);
    assert_eq!(other_calls.get(), 1);
    assert_eq!(power_calls.get(), 4);
    assert_eq!(cx.read(|app| app.power_mode()), PowerMode::Unknown);
}

#[crate::test]
fn power_observer_can_close_and_retire_a_window_without_losing_later_notifications(
    cx: &mut TestAppContext,
) {
    cx.update(|app| app.set_quit_mode(QuitMode::Explicit));
    let original = cx.add_window(|_, _| Empty);
    let native = cx.test_window(original.into());
    let closed = Rc::new(Cell::new(false));
    let observed = Rc::new(RefCell::new(Vec::new()));
    let _closer = cx.read(|app| {
        app.on_power_mode_change({
            let closed = closed.clone();
            move |app| {
                if !closed.replace(true) {
                    original
                        .update(app, |_, window, app| window.remove_window(app))
                        .expect("the observer must close the original live window");
                }
            }
        })
    });
    let _survivor = cx.read(|app| {
        app.on_power_mode_change({
            let observed = observed.clone();
            move |app| observed.borrow_mut().push(app.power_mode())
        })
    });
    cx.simulate_power_mode(PowerMode::LowPower);
    assert!(cx.windows().is_empty());
    assert!(native.is_native_terminal());
    let replacement = cx.add_window(|_, _| Empty);
    assert_ne!(original.window_id(), replacement.window_id());
    cx.simulate_power_mode(PowerMode::Balanced);
    assert_eq!(observed.take(), [PowerMode::LowPower, PowerMode::Balanced]);
    assert_eq!(
        replacement
            .update(cx, |_, _, app| app.power_mode())
            .unwrap(),
        PowerMode::Balanced
    );
    assert!(original.update(cx, |_, _, _| ()).is_err());
}

#[crate::test]
fn native_quit_retires_pending_power_notifications_and_rejects_later_callbacks(
    cx: &mut TestAppContext,
) {
    let calls = Rc::new(Cell::new(0));
    let _subscription = cx.read(|app| {
        app.on_power_mode_change({
            let calls = calls.clone();
            move |_| calls.set(calls.get() + 1)
        })
    });
    cx.update(|_| {
        cx.app.enqueue_quit_for_test();
        cx.test_platform()
            .simulate_power_mode_change(PowerMode::LowPower);
    });
    cx.run_until_parked();
    assert_eq!(calls.get(), 0);
    assert_eq!(cx.read(|app| app.power_mode()), PowerMode::Unknown);
    let diagnostics =
        cx.read(|app| app.native_boundary_diagnostics(NativeBoundaryDiagnosticCursor::default()));
    assert!(diagnostics.pending.is_empty());
    let power = diagnostics
        .terminal
        .iter()
        .filter(|entry| {
            entry.kind == NativeBoundaryKind::Callback(NativeCallbackKind::PowerModeChanged)
        })
        .collect::<Vec<_>>();
    assert_eq!(power.len(), 1);
    assert_eq!(power[0].target, NativeBoundaryTarget::Application);
    assert_eq!(power[0].disposition, NativeBoundaryDisposition::Closed);
    cx.simulate_power_mode(PowerMode::HighPower);
    assert_eq!(calls.get(), 0);
    assert_eq!(cx.read(|app| app.power_mode()), PowerMode::Unknown);
    assert!(cx.read(|app| app.native_exit_authority_is_settled_for_test()));
}

#[cfg(feature = "profiler")]
#[crate::test]
fn power_mode_changes_do_not_interrupt_measurement_or_clear_pending_demand(
    cx: &mut TestAppContext,
) {
    use crate::profiler::journal::{ForegroundJournalEntry, FrameStateChange, IntervalBoundary};

    let journal = cx.read(|app| app.foreground_journal());
    let mut collector = journal.collector();
    let window_id = crate::WindowId::from(42);
    let now = open_gpui_scheduler::Instant::now();
    crate::profiler::journal::record_frame_pending(window_id, now);
    for mode in [
        PowerMode::LowPower,
        PowerMode::Balanced,
        PowerMode::HighPower,
    ] {
        cx.simulate_power_mode(mode);
        assert!(!crate::profiler::journal::power_interrupted_since(now));
        crate::profiler::journal::record_frame_pending(window_id, now);
    }
    let entries = collector.collect_unseen();
    assert_eq!(entries.lost, 0);
    assert_eq!(entries.entries.iter().filter(|entry| {
        matches!(entry, ForegroundJournalEntry::FrameState(FrameStateChange::Pending { window_id: id, .. }) if *id == window_id)
    }).count(), 1);
    assert!(entries.entries.iter().all(|entry| {
        !matches!(
            entry,
            ForegroundJournalEntry::Boundary(IntervalBoundary::PowerTransition { .. })
        )
    }));
    crate::profiler::journal::record_window_closed(window_id);
}
