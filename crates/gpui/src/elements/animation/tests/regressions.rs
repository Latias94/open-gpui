use super::*;

struct ChainTestView {
    animations: Vec<Animation>,
    samples: Rc<RefCell<Vec<(usize, f32)>>>,
}

impl Render for ChainTestView {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let samples = self.samples.clone();
        div()
            .size_full()
            .with_animations(
                "animation-chain",
                self.animations.clone(),
                move |element, animation_ix, delta| {
                    samples.borrow_mut().push((animation_ix, delta));
                    element
                },
            )
            .child(div().size(px(12.0)))
    }
}

fn open_chain(
    cx: &mut TestAppContext,
    animations: Vec<Animation>,
) -> (Rc<RefCell<Vec<(usize, f32)>>>, WindowHandle<ChainTestView>) {
    let samples = Rc::new(RefCell::new(Vec::new()));
    let window = cx.open_window(size(px(100.0), px(100.0)), {
        let samples = samples.clone();
        move |_, _| ChainTestView {
            animations,
            samples,
        }
    });
    cx.run_until_parked();
    (samples, window)
}

fn delayed_frame_flags<V: Render>(
    window: WindowHandle<V>,
    cx: &mut TestAppContext,
) -> Vec<Rc<Cell<bool>>> {
    cx.update_window(window.into(), |_, window, _| {
        window
            .rendered_frame
            .element_states
            .values()
            .filter_map(|state| {
                state
                    .inner
                    .downcast_ref::<Option<AnimationState>>()
                    .and_then(Option::as_ref)
                    .map(|state| state.delayed_frame_pending.clone())
            })
            .collect()
    })
    .unwrap()
}

#[open_gpui::test]
fn oneshot_progress_and_completion_follow_the_executor_clock(cx: &mut TestAppContext) {
    let (samples, window) = open_chain(cx, vec![Animation::new(Duration::from_secs(1))]);
    assert_eq!(*samples.borrow(), vec![(0, 0.0)]);

    cx.executor().advance_clock(Duration::from_millis(250));
    assert_eq!(simulate_next_frame(&window, cx), 1);
    assert_eq!(*samples.borrow().last().unwrap(), (0, 0.25));

    cx.executor().advance_clock(Duration::from_millis(875));
    assert_eq!(simulate_next_frame(&window, cx), 1);
    assert_eq!(*samples.borrow().last().unwrap(), (0, 1.0));
    let completed = samples.borrow().len();
    assert_eq!(simulate_next_frame(&window, cx), 0);
    cx.executor().advance_clock(Duration::from_secs(1));
    cx.run_until_parked();
    assert_eq!(samples.borrow().len(), completed);
}

#[open_gpui::test]
fn chain_renders_each_endpoint_and_restarts_the_next_segment_clock(cx: &mut TestAppContext) {
    let (samples, window) = open_chain(
        cx,
        vec![
            Animation::new(Duration::from_millis(100)),
            Animation::new(Duration::from_millis(200)),
        ],
    );
    assert_eq!(*samples.borrow(), vec![(0, 0.0)]);

    cx.executor().advance_clock(Duration::from_millis(125));
    assert_eq!(simulate_next_frame(&window, cx), 1);
    assert_eq!(*samples.borrow().last().unwrap(), (0, 1.0));

    assert_eq!(simulate_next_frame(&window, cx), 1);
    assert_eq!(*samples.borrow().last().unwrap(), (1, 0.0));

    cx.executor().advance_clock(Duration::from_millis(50));
    assert_eq!(simulate_next_frame(&window, cx), 1);
    assert_eq!(*samples.borrow().last().unwrap(), (1, 0.25));

    cx.executor().advance_clock(Duration::from_millis(175));
    assert_eq!(simulate_next_frame(&window, cx), 1);
    assert_eq!(*samples.borrow().last().unwrap(), (1, 1.0));
    assert_eq!(simulate_next_frame(&window, cx), 0);
}

#[open_gpui::test]
fn reduced_motion_resolves_the_last_segment_before_easing_without_scheduling(
    cx: &mut TestAppContext,
) {
    cx.update(|app| app.set_reduce_motion(true));
    for (last, expected) in [
        (Animation::new(Duration::from_secs(1)), 1.5),
        (Animation::new(Duration::from_secs(1)).repeat(), -0.5),
        (Animation::new(Duration::from_secs(1)).repeat_synced(), -0.5),
    ] {
        let (samples, window) = open_chain(
            cx,
            vec![
                Animation::new(Duration::from_secs(1)),
                last.with_max_fps(10.0)
                    .with_easing(|delta| 2.0 * delta - 0.5),
            ],
        );
        assert_eq!(*samples.borrow(), vec![(1, expected)]);
        assert_eq!(simulate_next_frame(&window, cx), 0);
        assert!(
            delayed_frame_flags(window, cx)
                .iter()
                .all(|pending| !pending.get())
        );
        cx.executor().advance_clock(Duration::from_secs(1));
        cx.run_until_parked();
        assert_eq!(*samples.borrow(), vec![(1, expected)]);
    }
}

#[open_gpui::test]
fn finite_easing_overshoot_and_undershoot_reach_the_animator_unchanged(cx: &mut TestAppContext) {
    let (samples, window) = open_chain(
        cx,
        vec![Animation::new(Duration::from_secs(1)).with_easing(|delta| 8.0 * delta - 2.0)],
    );
    assert_eq!(*samples.borrow(), vec![(0, -2.0)]);

    cx.executor().advance_clock(Duration::from_millis(500));
    assert_eq!(simulate_next_frame(&window, cx), 1);
    assert_eq!(*samples.borrow().last().unwrap(), (0, 2.0));

    cx.executor().advance_clock(Duration::from_millis(625));
    assert_eq!(simulate_next_frame(&window, cx), 1);
    assert_eq!(*samples.borrow().last().unwrap(), (0, 6.0));
    assert_eq!(simulate_next_frame(&window, cx), 0);
}

#[open_gpui::test]
fn overlapping_renders_keep_one_original_animation_timer(cx: &mut TestAppContext) {
    let (samples, window) = open_test_window_with_max_fps(cx, Some(10.0));
    let pending = delayed_frame_flags(window, cx).pop().unwrap();
    assert!(pending.get());

    for _ in 0..2 {
        cx.executor().advance_clock(Duration::from_millis(20));
        window.update(cx, |_, _, cx| cx.notify()).unwrap();
        cx.run_until_parked();
        assert!(pending.get());
        let current = delayed_frame_flags(window, cx).pop().unwrap();
        assert!(Rc::ptr_eq(&pending, &current));
        assert_eq!(simulate_next_frame(&window, cx), 0);
    }
    assert_eq!(samples.borrow().len(), 3);

    cx.executor().advance_clock(Duration::from_millis(65));
    cx.run_until_parked();
    assert_eq!(samples.borrow().len(), 4);
    assert!((*samples.borrow().last().unwrap() - 0.1).abs() < 1e-2);

    cx.executor().advance_clock(Duration::from_millis(40));
    cx.run_until_parked();
    assert_eq!(samples.borrow().len(), 4);
    assert_eq!(simulate_next_frame(&window, cx), 0);

    cx.executor().advance_clock(Duration::from_millis(65));
    cx.run_until_parked();
    assert_eq!(samples.borrow().len(), 5);
}

struct IndependentTimersView {
    renders: Rc<RefCell<Vec<(usize, f32)>>>,
}

impl Render for IndependentTimersView {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let sample = |animation_ix| {
            let renders = self.renders.clone();
            move |element, delta| {
                renders.borrow_mut().push((animation_ix, delta));
                element
            }
        };
        div()
            .child(
                div().with_animation(
                    "fast-timer",
                    Animation::new(Duration::from_secs(1))
                        .repeat_synced()
                        .with_max_fps(10.0),
                    sample(0),
                ),
            )
            .child(
                div().with_animation(
                    "slow-timer",
                    Animation::new(Duration::from_secs(1))
                        .repeat_synced()
                        .with_max_fps(4.0),
                    sample(1),
                ),
            )
    }
}

#[open_gpui::test]
fn different_animations_keep_their_own_timer_deadlines(cx: &mut TestAppContext) {
    let renders = Rc::new(RefCell::new(Vec::new()));
    let window = cx.open_window(size(px(100.0), px(100.0)), {
        let renders = renders.clone();
        move |_, _| IndependentTimersView { renders }
    });
    cx.run_until_parked();
    assert_eq!(renders.borrow().len(), 2);
    assert_eq!(delayed_frame_flags(window, cx).len(), 2);
    assert_eq!(simulate_next_frame(&window, cx), 0);

    for expected in [4, 6] {
        cx.executor().advance_clock(Duration::from_millis(105));
        cx.run_until_parked();
        assert_eq!(renders.borrow().len(), expected);
        assert_eq!(simulate_next_frame(&window, cx), 0);
    }

    cx.executor().advance_clock(Duration::from_millis(45));
    cx.run_until_parked();
    assert_eq!(renders.borrow().len(), 8);
    let renders = renders.borrow();
    let last = &renders[renders.len() - 2..];
    assert_eq!(last[0].0, 0);
    assert_eq!(last[1].0, 1);
    assert!((last[0].1 - 0.25).abs() < 1e-2);
    assert_eq!(last[0].1, last[1].1);
}

#[open_gpui::test]
fn elapsed_animation_timer_waits_through_repeated_app_borrows_without_losing_its_notify(
    cx: &mut TestAppContext,
) {
    let (samples, window) = open_test_window_with_max_fps(cx, Some(10.0));
    let pending = delayed_frame_flags(window, cx).pop().unwrap();
    let executor = cx.executor();
    let dispatcher = cx.dispatcher.clone();

    cx.update(|_| {
        executor.advance_clock(Duration::from_millis(105));
        dispatcher.run_until_parked();
        assert_eq!(samples.borrow().len(), 1);
        assert!(
            pending.get(),
            "the original timer must remain pending while its notification waits for App"
        );
    });
    cx.update(|_| {
        dispatcher.run_until_parked();
        assert_eq!(samples.borrow().len(), 1);
        assert!(pending.get());
    });

    cx.run_until_parked();
    assert_eq!(samples.borrow().len(), 2);
    assert!((*samples.borrow().last().unwrap() - 0.105).abs() < 1e-2);
    assert_eq!(simulate_next_frame(&window, cx), 0);

    cx.executor().advance_clock(Duration::from_millis(50));
    cx.run_until_parked();
    assert_eq!(samples.borrow().len(), 2);
    cx.executor().advance_clock(Duration::from_millis(60));
    cx.run_until_parked();
    assert_eq!(samples.borrow().len(), 3);
}

#[open_gpui::test]
fn reduced_motion_stops_renewing_an_existing_timer_and_can_resume(cx: &mut TestAppContext) {
    let (samples, window) = open_test_window_with_max_fps(cx, Some(10.0));
    cx.update(|app| app.set_reduce_motion(true));
    cx.run_until_parked();
    assert_eq!(*samples.borrow().last().unwrap(), 0.0);
    let after_change = samples.borrow().len();

    cx.executor().advance_clock(Duration::from_millis(105));
    cx.run_until_parked();
    assert_eq!(samples.borrow().len(), after_change + 1);
    assert_eq!(*samples.borrow().last().unwrap(), 0.0);
    assert!(
        delayed_frame_flags(window, cx)
            .iter()
            .all(|pending| !pending.get())
    );
    assert_eq!(simulate_next_frame(&window, cx), 0);

    cx.executor().advance_clock(Duration::from_secs(1));
    cx.run_until_parked();
    assert_eq!(samples.borrow().len(), after_change + 1);

    cx.update(|app| app.set_reduce_motion(false));
    cx.run_until_parked();
    assert_eq!(samples.borrow().len(), after_change + 2);
    assert!(
        delayed_frame_flags(window, cx)
            .iter()
            .all(|pending| pending.get())
    );
    cx.executor().advance_clock(Duration::from_millis(105));
    cx.run_until_parked();
    assert_eq!(samples.borrow().len(), after_change + 3);
    assert_eq!(simulate_next_frame(&window, cx), 0);
}

#[open_gpui::test]
fn a_pending_animation_timer_does_not_retain_a_removed_window_or_view(cx: &mut TestAppContext) {
    let (samples, window) = open_test_window_with_max_fps(cx, Some(10.0));
    let view = window.root(cx).unwrap().downgrade();
    assert!(view.upgrade().is_some());
    cx.update_window(window.into(), |_, window, app| window.remove_window(app))
        .unwrap();
    cx.run_until_parked();
    assert!(view.upgrade().is_none());
    assert!(!cx.windows().contains(&window.into()));

    cx.executor().advance_clock(Duration::from_millis(105));
    cx.run_until_parked();
    assert!(view.upgrade().is_none());
    assert_eq!(samples.borrow().len(), 1);
}

#[test]
fn max_fps_ignores_invalid_and_unrepresentable_timer_intervals_without_panicking() {
    let now = Instant::now();
    for max_fps in [
        0.0,
        -0.0,
        -10.0,
        f32::NAN,
        f32::INFINITY,
        f32::NEG_INFINITY,
        f32::from_bits(1),
        f32::MIN_POSITIVE,
        1e-20,
        f32::MAX,
    ] {
        assert_eq!(
            Animation::new(Duration::from_secs(1))
                .with_max_fps(max_fps)
                .frame_interval(now),
            None,
            "max_fps: {max_fps}"
        );
    }

    for max_fps in [0.5, 10.0, 60.0, 1_000_000.0, 1_000_000_000.0] {
        let interval = Animation::new(Duration::from_secs(1))
            .with_max_fps(max_fps)
            .frame_interval(now)
            .unwrap();
        assert_eq!(interval, Duration::from_secs_f32(1.0 / max_fps));
        assert!(!interval.is_zero());
        assert!(now.checked_add(interval).is_some());
    }
}

#[test]
fn max_fps_rejects_a_duration_whose_deadline_cannot_be_represented() {
    let now = Instant::now();
    let max_fps = 1e-19;
    let interval = Duration::try_from_secs_f32(1.0 / max_fps).unwrap();
    let actual = Animation::new(Duration::from_secs(1))
        .with_max_fps(max_fps)
        .frame_interval(now);
    if now.checked_add(interval).is_none() {
        assert_eq!(actual, None);
    } else {
        assert_eq!(actual, Some(interval));
    }
}

#[open_gpui::test]
fn invalid_and_unrepresentable_max_fps_use_frames_instead_of_zero_duration_timers(
    cx: &mut TestAppContext,
) {
    cx.update(|app| app.set_quit_mode(crate::QuitMode::Explicit));
    for max_fps in [
        0.0,
        -10.0,
        f32::NAN,
        f32::INFINITY,
        f32::from_bits(1),
        f32::MIN_POSITIVE,
        1e-20,
        f32::MAX,
    ] {
        let (samples, window) = open_test_window_with_max_fps(cx, Some(max_fps));
        assert_eq!(samples.borrow().len(), 1);
        assert!(
            delayed_frame_flags(window, cx)
                .iter()
                .all(|pending| !pending.get())
        );
        assert_eq!(simulate_next_frame(&window, cx), 1);
        assert_eq!(samples.borrow().len(), 2);
        cx.update_window(window.into(), |_, window, app| window.remove_window(app))
            .unwrap();
        cx.run_until_parked();
    }
}
