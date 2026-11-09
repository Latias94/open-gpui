use std::{cell::RefCell, rc::Rc, time::Duration};

use crate::{
    Animation, Context, InteractiveElement, Render, TestAppContext, WindowHandle, div, prelude::*,
    px, size,
};

use super::*;

mod regressions;

struct AnimationTestView {
    rendered_deltas: Rc<RefCell<Vec<f32>>>,
    max_fps: Option<f32>,
}

struct SyncedAnimationTestView {
    show_second: bool,
    first_deltas: Rc<RefCell<Vec<f32>>>,
    second_deltas: Rc<RefCell<Vec<f32>>>,
}

impl Render for SyncedAnimationTestView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let record_deltas = |deltas: Rc<RefCell<Vec<f32>>>| {
            move |this, delta| {
                deltas.borrow_mut().push(delta);
                this
            }
        };
        div()
            .size_full()
            .child(div().with_animation(
                "first-synced-animation",
                Animation::new(Duration::from_secs(1)).repeat_synced(),
                record_deltas(self.first_deltas.clone()),
            ))
            .when(self.show_second, |this| {
                this.child(div().with_animation(
                    "second-synced-animation",
                    Animation::new(Duration::from_secs(1)).repeat_synced(),
                    record_deltas(self.second_deltas.clone()),
                ))
            })
    }
}

impl Render for AnimationTestView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let rendered_deltas = self.rendered_deltas.clone();
        // The throttled variant syncs to the shared clock so the deltas
        // follow the test scheduler's clock rather than wall time.
        let mut animation = Animation::new(Duration::from_secs(1));
        if let Some(max_fps) = self.max_fps {
            animation = animation.repeat_synced().with_max_fps(max_fps);
        } else {
            animation = animation.repeat();
        }
        div().size_full().child(div().with_animation(
            "repeating-animation",
            animation,
            move |this, delta| {
                rendered_deltas.borrow_mut().push(delta);
                this
            },
        ))
    }
}

fn open_test_window(
    cx: &mut TestAppContext,
) -> (Rc<RefCell<Vec<f32>>>, WindowHandle<AnimationTestView>) {
    open_test_window_with_max_fps(cx, None)
}

fn open_test_window_with_max_fps(
    cx: &mut TestAppContext,
    max_fps: Option<f32>,
) -> (Rc<RefCell<Vec<f32>>>, WindowHandle<AnimationTestView>) {
    let rendered_deltas = Rc::new(RefCell::new(Vec::new()));
    let window = cx.open_window(size(px(100.), px(100.)), {
        let rendered_deltas = rendered_deltas.clone();
        move |_, _| AnimationTestView {
            rendered_deltas,
            max_fps,
        }
    });
    cx.run_until_parked();
    (rendered_deltas, window)
}

fn simulate_next_frame<V: Render>(window: &WindowHandle<V>, cx: &mut TestAppContext) -> usize {
    let callback_count = cx
        .update_window((*window).into(), |_, window, cx| {
            window.drain_next_frame_callbacks_for_test(cx)
        })
        .unwrap();
    cx.run_until_parked();
    callback_count
}

// Before parent-animation-element, using .with_animation
// would not allow chaining .parent after. This is just a
// build check that we can call div().id().with_animation().child()
#[test]
fn test_animation_parent() {
    div()
        .id("id")
        //
        .with_animation(
            "animation",
            Animation::new(Duration::from_secs(1)),
            |el, _t| {
                //
                el
            },
        )
        .child(
            //
            div(),
        );
}

#[open_gpui::test]
fn test_repeating_animation_schedules_animation_frames(cx: &mut TestAppContext) {
    let (rendered_deltas, window) = open_test_window(cx);

    assert_eq!(rendered_deltas.borrow().len(), 1);

    for expected_frames in 2..=3 {
        assert_eq!(simulate_next_frame(&window, cx), 1);
        assert_eq!(rendered_deltas.borrow().len(), expected_frames);
    }
}

#[open_gpui::test]
fn test_max_fps_schedules_timer_driven_frames(cx: &mut TestAppContext) {
    let (rendered_deltas, window) = open_test_window_with_max_fps(cx, Some(10.0));

    // The test scheduler's clock jitters forward slightly on each poll,
    // so compare against expectations loosely.
    let assert_deltas_approx_eq = |expected: &[f32]| {
        let actual = rendered_deltas.borrow();
        assert_eq!(actual.len(), expected.len(), "deltas: {actual:?}");
        for (actual, expected) in actual.iter().zip(expected) {
            assert!(
                (actual - expected).abs() < 1e-2,
                "expected {expected}, got {actual}"
            );
        }
    };

    assert_deltas_approx_eq(&[0.0]);

    // No per-frame callback is scheduled; re-renders are timer-driven.
    assert_eq!(simulate_next_frame(&window, cx), 0);
    assert_deltas_approx_eq(&[0.0]);

    cx.executor().advance_clock(Duration::from_millis(105));
    cx.run_until_parked();
    assert_deltas_approx_eq(&[0.0, 0.105]);

    cx.executor().advance_clock(Duration::from_millis(105));
    cx.run_until_parked();
    assert_deltas_approx_eq(&[0.0, 0.105, 0.21]);
}

#[open_gpui::test]
fn test_synced_animations_share_phase_across_elements(cx: &mut TestAppContext) {
    let first_deltas = Rc::new(RefCell::new(Vec::new()));
    let second_deltas = Rc::new(RefCell::new(Vec::new()));
    let window = cx.open_window(size(px(100.), px(100.)), {
        let first_deltas = first_deltas.clone();
        let second_deltas = second_deltas.clone();
        move |_, _| SyncedAnimationTestView {
            show_second: false,
            first_deltas,
            second_deltas,
        }
    });
    cx.run_until_parked();

    assert_eq!(*first_deltas.borrow(), vec![0.0]);

    cx.executor().advance_clock(Duration::from_millis(250));
    simulate_next_frame(&window, cx);
    assert_eq!(*first_deltas.borrow(), vec![0.0, 0.25]);

    // The second element mounts a quarter through the cycle, yet renders
    // the shared phase rather than starting at zero.
    window
        .update(cx, |view, _, cx| {
            view.show_second = true;
            cx.notify();
        })
        .unwrap();
    cx.run_until_parked();
    cx.executor().advance_clock(Duration::from_millis(250));
    simulate_next_frame(&window, cx);

    assert_eq!(*second_deltas.borrow().last().unwrap(), 0.5);
    assert_eq!(
        *first_deltas.borrow().last().unwrap(),
        *second_deltas.borrow().last().unwrap()
    );
    assert!(second_deltas.borrow().iter().all(|delta| *delta > 0.0));

    // The phase wraps around each full cycle.
    cx.executor().advance_clock(Duration::from_millis(2250));
    simulate_next_frame(&window, cx);
    assert_eq!(*first_deltas.borrow().last().unwrap(), 0.75);

    // Sub-second precision survives months of uptime: converting the raw
    // elapsed time to f32 would round 0.25 away entirely.
    cx.executor()
        .advance_clock(Duration::from_secs(300 * 24 * 60 * 60) + Duration::from_millis(500));
    simulate_next_frame(&window, cx);
    assert_eq!(*first_deltas.borrow().last().unwrap(), 0.25);
}

#[open_gpui::test]
fn test_reduce_motion_renders_single_static_frame(cx: &mut TestAppContext) {
    cx.update(|cx| cx.set_reduce_motion(true));
    let (rendered_deltas, window) = open_test_window(cx);

    assert_eq!(*rendered_deltas.borrow(), vec![0.0]);

    assert_eq!(simulate_next_frame(&window, cx), 0);
    assert_eq!(*rendered_deltas.borrow(), vec![0.0]);
}
