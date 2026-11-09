use std::{cell::RefCell, rc::Rc, time::Duration};

use crate::{
    AppContext, Context, InteractiveElement, Pixels, Render, TestAppContext, WindowHandle, div,
    prelude::*, px, size,
};

use super::*;

mod regressions;

struct SpringAnimationTestView {
    target: Pixels,
    initial: Option<Pixels>,
    playback: SpringPlayback,
    rendered_values: Rc<RefCell<Vec<Pixels>>>,
}

impl Render for SpringAnimationTestView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let rendered_values = self.rendered_values.clone();
        let mut animation = SpringAnimation::new(SpringConfig::new(100.0, 2.0, 1.0))
            .to(self.target)
            .with_epsilon(0.01)
            .playback(self.playback);
        if let Some(initial) = self.initial {
            animation = animation.from(initial);
        }
        div().with_spring("spring-animation", animation, move |this, value| {
            rendered_values.borrow_mut().push(value);
            this.left(value)
        })
    }
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

#[test]
fn test_spring_animation_parent() {
    div()
        .id("id")
        .with_spring(
            "spring-animation",
            SpringAnimation::new(SpringConfig::new(100.0, 10.0, 1.0))
                .to(px(10.0))
                .from(px(0.0)),
            |element, value| element.left(value),
        )
        .child(div());
}

#[open_gpui::test]
fn test_spring_animation_preserves_velocity_when_retargeted(cx: &mut TestAppContext) {
    let rendered_values = Rc::new(RefCell::new(Vec::new()));
    let window = cx.open_window(size(px(100.0), px(100.0)), {
        let rendered_values = rendered_values.clone();
        move |_, _| SpringAnimationTestView {
            target: px(0.0),
            initial: None,
            playback: SpringPlayback::Running,
            rendered_values,
        }
    });
    cx.run_until_parked();
    assert_eq!(*rendered_values.borrow(), vec![px(0.0)]);

    window
        .update(cx, |view, _, cx| {
            view.target = px(100.0);
            cx.notify();
        })
        .unwrap();
    cx.run_until_parked();

    cx.executor().advance_clock(Duration::from_millis(50));
    assert!(simulate_next_frame(&window, cx) > 0);
    let value_before_retargeting = *rendered_values.borrow().last().unwrap();
    assert!(value_before_retargeting > px(0.0));
    assert!(value_before_retargeting < px(100.0));

    window
        .update(cx, |view, _, cx| {
            view.target = px(0.0);
            cx.notify();
        })
        .unwrap();
    cx.run_until_parked();

    cx.executor().advance_clock(Duration::from_millis(5));
    assert!(simulate_next_frame(&window, cx) > 0);
    let value_after_retargeting = *rendered_values.borrow().last().unwrap();
    assert!(value_after_retargeting > value_before_retargeting);
}

#[open_gpui::test]
fn test_paused_spring_resumes_with_its_velocity(cx: &mut TestAppContext) {
    let rendered_values = Rc::new(RefCell::new(Vec::new()));
    let window = cx.open_window(size(px(100.0), px(100.0)), {
        let rendered_values = rendered_values.clone();
        move |_, _| SpringAnimationTestView {
            target: px(0.0),
            initial: None,
            playback: SpringPlayback::Running,
            rendered_values,
        }
    });
    cx.run_until_parked();

    window
        .update(cx, |view, _, cx| {
            view.target = px(100.0);
            cx.notify();
        })
        .unwrap();
    cx.run_until_parked();
    cx.executor().advance_clock(Duration::from_millis(50));
    assert!(simulate_next_frame(&window, cx) > 0);

    window
        .update(cx, |view, _, cx| {
            view.target = px(0.0);
            view.playback = SpringPlayback::Paused;
            cx.notify();
        })
        .unwrap();
    cx.run_until_parked();
    let paused_value = *rendered_values.borrow().last().unwrap();

    cx.executor().advance_clock(Duration::from_millis(500));
    assert!(simulate_next_frame(&window, cx) > 0);
    assert_eq!(*rendered_values.borrow().last().unwrap(), paused_value);
    assert_eq!(simulate_next_frame(&window, cx), 0);

    window
        .update(cx, |view, _, cx| {
            view.playback = SpringPlayback::Running;
            cx.notify();
        })
        .unwrap();
    cx.run_until_parked();
    cx.executor().advance_clock(Duration::from_millis(5));
    assert!(simulate_next_frame(&window, cx) > 0);
    assert!(*rendered_values.borrow().last().unwrap() > paused_value);
}

#[open_gpui::test]
fn test_stopped_spring_resumes_without_velocity(cx: &mut TestAppContext) {
    let rendered_values = Rc::new(RefCell::new(Vec::new()));
    let window = cx.open_window(size(px(100.0), px(100.0)), {
        let rendered_values = rendered_values.clone();
        move |_, _| SpringAnimationTestView {
            target: px(0.0),
            initial: None,
            playback: SpringPlayback::Running,
            rendered_values,
        }
    });
    cx.run_until_parked();

    window
        .update(cx, |view, _, cx| {
            view.target = px(1_000_000.0);
            cx.notify();
        })
        .unwrap();
    cx.run_until_parked();
    cx.executor().advance_clock(Duration::from_millis(50));
    assert!(simulate_next_frame(&window, cx) > 0);

    window
        .update(cx, |view, _, cx| {
            view.target = px(0.0);
            view.playback = SpringPlayback::Stopped;
            cx.notify();
        })
        .unwrap();
    cx.run_until_parked();
    let stopped_value = *rendered_values.borrow().last().unwrap();

    cx.executor().advance_clock(Duration::from_millis(500));
    assert!(simulate_next_frame(&window, cx) > 0);
    assert_eq!(*rendered_values.borrow().last().unwrap(), stopped_value);
    assert_eq!(simulate_next_frame(&window, cx), 0);

    window
        .update(cx, |view, _, cx| {
            view.target = stopped_value;
            view.playback = SpringPlayback::Running;
            cx.notify();
        })
        .unwrap();
    cx.run_until_parked();
    assert_eq!(*rendered_values.borrow().last().unwrap(), stopped_value);
    assert_eq!(simulate_next_frame(&window, cx), 0);
}

#[open_gpui::test]
fn test_cancelled_and_completed_springs_resolve_their_endpoints(cx: &mut TestAppContext) {
    let rendered_values = Rc::new(RefCell::new(Vec::new()));
    let window = cx.open_window(size(px(100.0), px(100.0)), {
        let rendered_values = rendered_values.clone();
        move |_, _| SpringAnimationTestView {
            target: px(100.0),
            initial: Some(px(20.0)),
            playback: SpringPlayback::Running,
            rendered_values,
        }
    });
    cx.run_until_parked();
    assert_eq!(*rendered_values.borrow(), vec![px(20.0)]);

    cx.executor().advance_clock(Duration::from_millis(50));
    assert!(simulate_next_frame(&window, cx) > 0);
    assert!(*rendered_values.borrow().last().unwrap() > px(20.0));

    window
        .update(cx, |view, _, cx| {
            view.playback = SpringPlayback::Cancelled;
            cx.notify();
        })
        .unwrap();
    cx.run_until_parked();
    assert_eq!(*rendered_values.borrow().last().unwrap(), px(20.0));
    assert!(simulate_next_frame(&window, cx) > 0);
    assert_eq!(simulate_next_frame(&window, cx), 0);

    window
        .update(cx, |view, _, cx| {
            view.playback = SpringPlayback::Completed;
            cx.notify();
        })
        .unwrap();
    cx.run_until_parked();
    assert_eq!(*rendered_values.borrow().last().unwrap(), px(100.0));
    assert_eq!(simulate_next_frame(&window, cx), 0);
}

#[open_gpui::test]
fn test_spring_animation_respects_reduced_motion(cx: &mut TestAppContext) {
    cx.update(|cx| cx.set_reduce_motion(true));
    let rendered_values = Rc::new(RefCell::new(Vec::new()));
    let window = cx.open_window(size(px(100.0), px(100.0)), {
        let rendered_values = rendered_values.clone();
        move |_, _| SpringAnimationTestView {
            target: px(100.0),
            initial: None,
            playback: SpringPlayback::Running,
            rendered_values,
        }
    });
    cx.run_until_parked();

    assert_eq!(*rendered_values.borrow(), vec![px(100.0)]);
    assert_eq!(simulate_next_frame(&window, cx), 0);
}
