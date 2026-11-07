#![cfg(feature = "test-support")]

use open_gpui::{
    App, AppContext as _, Bounds, Context, InteractiveElement as _, IntoElement, MouseButton,
    MouseDownEvent, PlatformInput, PlatformWindowPresentOutcome, PowerMode, QuitMode, Render,
    RequestFrameOptions, Styled as _, TestAppContext, ThermalState, VisualTestContext, Window,
    WindowBounds, WindowHandle, WindowInitialPresentationStatus, WindowOptions,
    WindowPresentationFacts, div, point, px, size,
};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    time::Duration,
};

const LONG_INTERVAL: Duration = Duration::from_secs(3_600);

#[derive(Default)]
struct Observations {
    renders: Cell<usize>,
    inputs: Cell<usize>,
}

struct CadenceView(Rc<Observations>);

impl Render for CadenceView {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        self.0.renders.set(self.0.renders.get() + 1);
        let observed = Rc::clone(&self.0);
        div()
            .size_full()
            .bg(if observed.inputs.get().is_multiple_of(2) {
                open_gpui::blue()
            } else {
                open_gpui::green()
            })
            .on_mouse_down(MouseButton::Left, move |_, window, _| {
                observed.inputs.set(observed.inputs.get() + 1);
                window.refresh();
            })
    }
}

fn unlimited_options() -> WindowOptions {
    WindowOptions {
        inactive_frame_interval: None,
        low_power_frame_interval: None,
        thermal_frame_interval: None,
        ..Default::default()
    }
}

fn facts(visual: &mut VisualTestContext) -> WindowPresentationFacts {
    visual.update(|window, _| window.presentation_facts())
}

fn assert_submitted(facts: WindowPresentationFacts, generation: u64) {
    assert!(facts.native_created);
    assert!(facts.native_visible);
    assert_eq!(
        facts.initial_presentation,
        WindowInitialPresentationStatus::Completed
    );
    assert_eq!(facts.frame_accepted_generation, Some(generation));
    assert_eq!(facts.present_submitted_generation, Some(generation));
    assert_eq!(facts.non_empty_presented_generation, Some(generation));
    let attempt = facts.latest_present_attempt.unwrap();
    assert_eq!(attempt.generation, generation);
    assert_eq!(attempt.outcome, PlatformWindowPresentOutcome::Submitted);
    assert!(attempt.contained_valid_primitives);
}

fn assert_compositor_callback_parks(visual: &mut VisualTestContext, observed: &Observations) {
    let submitted = facts(visual);
    let renders = observed.renders.get();
    assert!(visual.frame_scheduled());
    assert!(visual.simulate_scheduled_frame());
    assert!(!visual.frame_scheduled());
    assert!(!visual.simulate_scheduled_frame());
    assert_eq!(observed.renders.get(), renders);
    assert_eq!(facts(visual), submitted);
}

fn open_parked_window(
    cx: &mut TestAppContext,
    options: WindowOptions,
) -> (
    WindowHandle<CadenceView>,
    VisualTestContext,
    Rc<Observations>,
) {
    let observed = Rc::new(Observations::default());
    cx.defer_next_window_frame_requests();
    let handle = cx.update(|app| {
        app.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds::new(
                    point(px(0.), px(0.)),
                    size(px(96.), px(96.)),
                ))),
                focus_on_appearing: false,
                ..options
            },
            |_, app| app.new(|_| CadenceView(Rc::clone(&observed))),
        )
        .unwrap()
    });
    let mut visual = VisualTestContext::from_window(handle.into(), cx);
    visual.run_until_parked();
    visual.update(|window, app| window.draw(app).clear(app));
    assert!(visual.simulate_frame(RequestFrameOptions {
        require_presentation: true,
        ..Default::default()
    }));
    let submitted = facts(&mut visual);
    assert_submitted(submitted, submitted.frame_accepted_generation.unwrap());
    assert_compositor_callback_parks(&mut visual, &observed);
    (handle, visual, observed)
}

fn dispatch_input(window: &mut Window, app: &mut App) {
    window.dispatch_event(
        PlatformInput::MouseDown(MouseDownEvent {
            position: point(px(40.), px(40.)),
            button: MouseButton::Left,
            ..Default::default()
        }),
        app,
    );
}

#[open_gpui::test]
fn public_power_query_updates_without_observers_before_delivering_committed_modes(
    cx: &mut TestAppContext,
) {
    assert_eq!(cx.read(App::power_mode), PowerMode::Unknown);
    for mode in [
        PowerMode::Balanced,
        PowerMode::HighPower,
        PowerMode::Unknown,
        PowerMode::LowPower,
    ] {
        cx.simulate_power_mode(mode);
        assert_eq!(cx.read(App::power_mode), mode);
    }

    let observed = Rc::new(RefCell::new(Vec::new()));
    let _subscription = cx.read(|app| {
        app.on_power_mode_change({
            let observed = Rc::clone(&observed);
            move |app| observed.borrow_mut().push(app.power_mode())
        })
    });
    assert!(observed.borrow().is_empty());
    assert_eq!(cx.read(App::power_mode), PowerMode::LowPower);

    let modes = [
        PowerMode::HighPower,
        PowerMode::Balanced,
        PowerMode::Unknown,
        PowerMode::LowPower,
    ];
    for mode in modes {
        cx.simulate_power_mode(mode);
        assert_eq!(cx.read(App::power_mode), mode);
        assert_eq!(observed.borrow().last(), Some(&mode));
    }
    assert_eq!(*observed.borrow(), modes);
}

#[open_gpui::test]
fn public_power_observers_have_independent_drop_and_detach_lifetimes(cx: &mut TestAppContext) {
    let first = Rc::new(RefCell::new(Vec::new()));
    let second = Rc::new(RefCell::new(Vec::new()));
    let owner = Rc::new(());
    let lifetime = Rc::downgrade(&owner);
    let (first_subscription, second_subscription) = cx.read(|app| {
        (
            app.on_power_mode_change({
                let first = Rc::clone(&first);
                move |app| {
                    let _ = Rc::strong_count(&owner);
                    first.borrow_mut().push(app.power_mode());
                }
            }),
            app.on_power_mode_change({
                let second = Rc::clone(&second);
                move |app| second.borrow_mut().push(app.power_mode())
            }),
        )
    });
    assert!(lifetime.upgrade().is_some());
    cx.simulate_power_mode(PowerMode::LowPower);
    assert_eq!(*first.borrow(), [PowerMode::LowPower]);
    assert_eq!(*second.borrow(), [PowerMode::LowPower]);

    drop(first_subscription);
    assert!(lifetime.upgrade().is_none());
    second_subscription.detach();
    cx.simulate_power_mode(PowerMode::HighPower);
    assert_eq!(*first.borrow(), [PowerMode::LowPower]);
    assert_eq!(
        *second.borrow(),
        [PowerMode::LowPower, PowerMode::HighPower]
    );
    assert_eq!(cx.read(App::power_mode), PowerMode::HighPower);
}

#[open_gpui::test]
fn public_configured_limits_are_independent_between_two_real_windows(cx: &mut TestAppContext) {
    cx.update(|app| app.set_quit_mode(QuitMode::Explicit));
    cx.simulate_power_mode(PowerMode::LowPower);
    cx.simulate_thermal_state_change(ThermalState::Critical);

    for (inactive_frame_interval, low_power_frame_interval, thermal_frame_interval) in [
        (Some(LONG_INTERVAL), None, None),
        (None, Some(LONG_INTERVAL), None),
        (None, None, Some(LONG_INTERVAL)),
        (
            Some(LONG_INTERVAL),
            Some(LONG_INTERVAL * 2),
            Some(LONG_INTERVAL * 3),
        ),
    ] {
        let (limited, mut limited_visual, limited_observed) = open_parked_window(
            cx,
            WindowOptions {
                inactive_frame_interval,
                low_power_frame_interval,
                thermal_frame_interval,
                ..Default::default()
            },
        );
        let (unlimited, mut unlimited_visual, unlimited_observed) =
            open_parked_window(cx, unlimited_options());
        assert!(!limited_visual.update(|window, _| window.is_window_active()));
        assert!(!unlimited_visual.update(|window, _| window.is_window_active()));
        let limited_baseline = facts(&mut limited_visual);
        let unlimited_baseline = facts(&mut unlimited_visual);
        let limited_renders = limited_observed.renders.get();
        let unlimited_renders = unlimited_observed.renders.get();
        let limited_callbacks = Rc::new(Cell::new(0));
        let unlimited_callbacks = Rc::new(Cell::new(0));
        limited_visual.update(|window, _| {
            let callbacks = Rc::clone(&limited_callbacks);
            window.on_next_frame(move |_, _| callbacks.set(callbacks.get() + 1));
        });
        unlimited_visual.update(|window, _| {
            let callbacks = Rc::clone(&unlimited_callbacks);
            window.on_next_frame(move |_, _| callbacks.set(callbacks.get() + 1));
        });

        assert!(limited_visual.simulate_scheduled_frame());
        assert!(unlimited_visual.simulate_scheduled_frame());
        assert_eq!(limited_callbacks.get(), 0);
        assert_eq!(unlimited_callbacks.get(), 1);
        assert!(limited_visual.frame_scheduled());
        assert!(!unlimited_visual.frame_scheduled());
        assert_eq!(facts(&mut limited_visual), limited_baseline);
        assert_eq!(facts(&mut unlimited_visual), unlimited_baseline);
        assert_eq!(limited_observed.renders.get(), limited_renders);
        assert_eq!(unlimited_observed.renders.get(), unlimited_renders);

        assert!(limited_visual.simulate_scheduled_frame());
        assert_eq!(limited_callbacks.get(), 0);
        assert_eq!(facts(&mut limited_visual), limited_baseline);
        assert!(limited_visual.simulate_frame(RequestFrameOptions {
            require_presentation: true,
            ..Default::default()
        }));
        assert_eq!(limited_callbacks.get(), 1);
        assert_eq!(limited_observed.renders.get(), limited_renders);
        assert_submitted(
            facts(&mut limited_visual),
            limited_baseline.frame_accepted_generation.unwrap(),
        );
        assert_compositor_callback_parks(&mut limited_visual, &limited_observed);
        assert_eq!(cx.read(App::power_mode), PowerMode::LowPower);
        assert_eq!(cx.read(App::thermal_state), ThermalState::Critical);
        assert!(cx.simulate_window_close(limited.into()));
        assert!(cx.simulate_window_close(unlimited.into()));
    }
}

#[open_gpui::test]
fn public_power_changes_release_deferred_force_render_without_an_observer(cx: &mut TestAppContext) {
    cx.simulate_power_mode(PowerMode::Balanced);
    let (_, mut visual, observed) = open_parked_window(
        cx,
        WindowOptions {
            low_power_frame_interval: Some(LONG_INTERVAL),
            ..unlimited_options()
        },
    );
    let _activation = visual.update(|window, _| window.activate_window());
    visual.run_until_parked();
    assert!(visual.update(|window, _| window.is_window_active()));

    for mode in [
        PowerMode::Unknown,
        PowerMode::Balanced,
        PowerMode::HighPower,
    ] {
        assert!(visual.simulate_frame(RequestFrameOptions {
            require_presentation: true,
            ..Default::default()
        }));
        assert_compositor_callback_parks(&mut visual, &observed);
        let baseline = facts(&mut visual);
        let renders = observed.renders.get();
        let callbacks = Rc::new(Cell::new(0));
        cx.simulate_power_mode(PowerMode::LowPower);
        visual.update(|window, _| {
            let callbacks = Rc::clone(&callbacks);
            window.on_next_frame(move |_, _| callbacks.set(callbacks.get() + 1));
        });
        assert!(visual.simulate_frame(RequestFrameOptions {
            force_render: true,
            ..Default::default()
        }));
        assert_eq!(callbacks.get(), 0);
        assert_eq!(observed.renders.get(), renders);
        assert_eq!(facts(&mut visual), baseline);
        assert!(visual.frame_scheduled());

        cx.simulate_power_mode(mode);
        assert_eq!(cx.read(App::power_mode), mode);
        assert!(visual.simulate_scheduled_frame());
        assert_eq!(callbacks.get(), 1);
        assert_eq!(observed.renders.get(), renders + 1);
        assert_submitted(
            facts(&mut visual),
            baseline.frame_accepted_generation.unwrap() + 1,
        );
        assert_compositor_callback_parks(&mut visual, &observed);
    }
}

#[open_gpui::test]
fn public_pending_scene_is_paced_without_force_or_next_frame_callbacks(cx: &mut TestAppContext) {
    cx.simulate_power_mode(PowerMode::LowPower);
    let (_, mut visual, observed) = open_parked_window(
        cx,
        WindowOptions {
            low_power_frame_interval: Some(LONG_INTERVAL),
            ..unlimited_options()
        },
    );
    let baseline = facts(&mut visual);
    visual.update(|window, _| window.refresh());
    let pending = facts(&mut visual);
    assert!(pending.frame_accepted_generation > baseline.frame_accepted_generation);
    assert_eq!(
        pending.present_submitted_generation,
        baseline.present_submitted_generation
    );
    assert_eq!(
        pending.latest_present_attempt,
        baseline.latest_present_attempt
    );
    let renders = observed.renders.get();

    for _ in 0..2 {
        assert!(visual.simulate_scheduled_frame());
        assert!(visual.frame_scheduled());
        assert_eq!(facts(&mut visual), pending);
        assert_eq!(observed.renders.get(), renders);
    }
    assert!(visual.simulate_frame(RequestFrameOptions {
        require_presentation: true,
        ..Default::default()
    }));
    assert_submitted(
        facts(&mut visual),
        pending.frame_accepted_generation.unwrap(),
    );
    assert_eq!(observed.renders.get(), renders);
    assert_compositor_callback_parks(&mut visual, &observed);
}

#[open_gpui::test]
fn public_input_and_next_frame_captures_survive_throttling_and_renderer_retry(
    cx: &mut TestAppContext,
) {
    cx.update(|app| app.set_quit_mode(QuitMode::Explicit));
    for outcome in [
        PlatformWindowPresentOutcome::Deferred,
        PlatformWindowPresentOutcome::Rejected,
    ] {
        cx.simulate_power_mode(PowerMode::LowPower);
        let (handle, mut visual, observed) = open_parked_window(
            cx,
            WindowOptions {
                low_power_frame_interval: Some(LONG_INTERVAL),
                ..unlimited_options()
            },
        );
        let baseline = facts(&mut visual);
        let renders = observed.renders.get();
        let callbacks = Rc::new(Cell::new(0));
        let capture = Rc::new(());
        let lifetime = Rc::downgrade(&capture);
        visual.update(|window, _| {
            let callbacks = Rc::clone(&callbacks);
            window.on_next_frame(move |window, app| {
                callbacks.set(callbacks.get() + 1);
                dispatch_input(window, app);
                window.on_next_frame(move |_, _| {
                    let _ = Rc::strong_count(&capture);
                    callbacks.set(callbacks.get() + 1);
                });
            });
        });
        assert!(visual.simulate_frame(RequestFrameOptions {
            force_render: true,
            ..Default::default()
        }));
        assert!(visual.simulate_scheduled_frame());
        assert_eq!(callbacks.get(), 0);
        assert_eq!(observed.inputs.get(), 0);
        assert_eq!(observed.renders.get(), renders);
        assert_eq!(facts(&mut visual), baseline);
        assert!(lifetime.upgrade().is_some());

        cx.set_window_present_outcome(handle.into(), outcome);
        assert!(visual.simulate_frame(RequestFrameOptions {
            require_presentation: true,
            ..Default::default()
        }));
        let pending = facts(&mut visual);
        let generation = pending.frame_accepted_generation.unwrap();
        assert_eq!(generation, baseline.frame_accepted_generation.unwrap() + 1);
        assert_eq!(
            pending.present_submitted_generation,
            baseline.present_submitted_generation
        );
        assert_eq!(
            pending.non_empty_presented_generation,
            baseline.non_empty_presented_generation
        );
        let attempt = pending.latest_present_attempt.unwrap();
        assert_eq!(attempt.generation, generation);
        assert_eq!(attempt.outcome, outcome);
        assert!(attempt.contained_valid_primitives);
        assert_eq!(callbacks.get(), 1);
        assert_eq!(observed.inputs.get(), 1);
        assert_eq!(observed.renders.get(), renders + 1);
        assert!(lifetime.upgrade().is_some());

        for _ in 0..2 {
            assert!(visual.simulate_scheduled_frame());
            assert!(visual.frame_scheduled());
            assert_eq!(facts(&mut visual), pending);
            assert_eq!(callbacks.get(), 1);
            assert_eq!(observed.inputs.get(), 1);
            assert_eq!(observed.renders.get(), renders + 1);
            assert!(lifetime.upgrade().is_some());
        }

        cx.set_window_present_outcome(handle.into(), PlatformWindowPresentOutcome::Submitted);
        cx.simulate_power_mode(PowerMode::HighPower);
        assert_eq!(cx.read(App::power_mode), PowerMode::HighPower);
        assert!(visual.simulate_scheduled_frame());
        assert_eq!(callbacks.get(), 2);
        assert!(lifetime.upgrade().is_none());
        assert_eq!(observed.inputs.get(), 1);
        assert_eq!(observed.renders.get(), renders + 1);
        assert_submitted(facts(&mut visual), generation);
        assert_compositor_callback_parks(&mut visual, &observed);

        visual.simulate_mouse_down(
            point(px(40.), px(40.)),
            MouseButton::Left,
            Default::default(),
        );
        assert_eq!(observed.inputs.get(), 2);
        let after_input = facts(&mut visual);
        assert!(after_input.frame_accepted_generation > Some(generation));
        assert!(visual.simulate_scheduled_frame());
        assert_submitted(
            facts(&mut visual),
            after_input.frame_accepted_generation.unwrap(),
        );
        assert_compositor_callback_parks(&mut visual, &observed);
        assert!(cx.simulate_window_close(handle.into()));
    }
}

#[open_gpui::test]
fn public_close_releases_throttled_callbacks_without_losing_app_power_or_the_other_window(
    cx: &mut TestAppContext,
) {
    cx.update(|app| app.set_quit_mode(QuitMode::Explicit));
    cx.simulate_power_mode(PowerMode::LowPower);
    let (handle, mut visual, observed) = open_parked_window(
        cx,
        WindowOptions {
            low_power_frame_interval: Some(LONG_INTERVAL),
            ..unlimited_options()
        },
    );
    let (_, mut other_visual, other_observed) = open_parked_window(cx, unlimited_options());
    let other_baseline = facts(&mut other_visual);
    let root = handle.entity(cx).unwrap().downgrade();
    let capture = Rc::new(Cell::new(false));
    let lifetime = Rc::downgrade(&capture);
    visual.update(|window, _| {
        window.on_next_frame(move |_, _| capture.set(true));
    });
    assert!(visual.simulate_frame(RequestFrameOptions {
        force_render: true,
        ..Default::default()
    }));
    assert!(visual.frame_scheduled());
    assert!(lifetime.upgrade().is_some());
    let renders = observed.renders.get();
    let modes = Rc::new(RefCell::new(Vec::new()));
    let _subscription = cx.read(|app| {
        app.on_power_mode_change({
            let modes = Rc::clone(&modes);
            move |app| modes.borrow_mut().push(app.power_mode())
        })
    });
    let terminal = cx.hold_window_native_terminal(handle.into());
    assert!(cx.simulate_window_close(handle.into()));
    assert!(!cx.windows().contains(&handle.into()));
    assert!(root.upgrade().is_none());
    assert!(lifetime.upgrade().is_none());
    assert_eq!(cx.read(App::power_mode), PowerMode::LowPower);
    drop(visual);

    cx.simulate_power_mode(PowerMode::Balanced);
    assert_eq!(cx.read(App::power_mode), PowerMode::Balanced);
    assert_eq!(*modes.borrow(), [PowerMode::Balanced]);
    let other_callbacks = Rc::new(Cell::new(0));
    other_visual.update(|window, _| {
        let callbacks = Rc::clone(&other_callbacks);
        window.on_next_frame(move |_, _| callbacks.set(callbacks.get() + 1));
    });
    assert!(other_visual.simulate_scheduled_frame());
    assert_eq!(other_callbacks.get(), 1);
    assert!(!other_visual.frame_scheduled());
    assert_eq!(facts(&mut other_visual), other_baseline);
    assert!(terminal.release());
    cx.run_until_parked();
    assert!(root.upgrade().is_none());
    assert!(lifetime.upgrade().is_none());
    assert_eq!(observed.renders.get(), renders);
    assert!(!cx.windows().contains(&handle.into()));
    assert_eq!(cx.read(App::power_mode), PowerMode::Balanced);
    assert!(!other_visual.frame_scheduled());
    assert_eq!(facts(&mut other_visual), other_baseline);
    assert!(other_observed.renders.get() > 0);
}

#[cfg(feature = "profiler")]
#[open_gpui::test]
fn public_skip_reason_identifies_the_longest_applicable_configured_limit(cx: &mut TestAppContext) {
    use open_gpui::profiler::journal::{ForegroundJournalEntry, FrameSkipReason, IntervalBoundary};

    cx.update(|app| app.set_quit_mode(QuitMode::Explicit));
    cx.simulate_power_mode(PowerMode::LowPower);
    cx.simulate_thermal_state_change(ThermalState::Critical);
    for (intervals, expected) in [
        ([3, 2, 1], FrameSkipReason::InactiveFrameRateLimit),
        ([1, 3, 2], FrameSkipReason::LowPowerFrameRateLimit),
        ([2, 1, 3], FrameSkipReason::ThermalFrameRateLimit),
        ([1, 1, 1], FrameSkipReason::InactiveFrameRateLimit),
    ] {
        let (handle, mut visual, _) = open_parked_window(
            cx,
            WindowOptions {
                inactive_frame_interval: Some(LONG_INTERVAL * intervals[0]),
                low_power_frame_interval: Some(LONG_INTERVAL * intervals[1]),
                thermal_frame_interval: Some(LONG_INTERVAL * intervals[2]),
                ..Default::default()
            },
        );
        let mut collector = cx.read(|app| app.foreground_journal().collector());
        visual.update(|window, app| {
            dispatch_input(window, app);
            window.on_next_frame(|_, _| {});
        });
        assert!(visual.simulate_scheduled_frame());
        let drained = collector.collect_unseen();
        assert_eq!(drained.lost, 0);
        let reasons = drained
            .entries
            .iter()
            .filter_map(|entry| match entry {
                ForegroundJournalEntry::Boundary(IntervalBoundary::FrameSkipped(skipped))
                    if skipped.window_id == handle.window_id() =>
                {
                    Some(skipped.reason)
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(reasons, [expected]);
        assert!(visual.frame_scheduled());
        assert!(cx.simulate_window_close(handle.into()));
    }
}
