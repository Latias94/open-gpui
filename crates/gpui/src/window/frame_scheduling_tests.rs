use crate::{
    AppContext as _, Context, IntoElement, ParentElement as _, PlatformWindow as _, Render,
    Styled as _, TestAppContext, Window, div,
};
use std::{cell::Cell, rc::Rc};

struct RenderCounter(Rc<Cell<usize>>);

impl Render for RenderCounter {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        self.0.set(self.0.get() + 1);
        div().size_full().bg(crate::blue()).child("Scheduled frame")
    }
}

#[crate::test]
fn queued_frame_callback_wakes_a_parked_render_loop(cx: &mut TestAppContext) {
    cx.defer_next_window_frame_requests();
    let handle = cx.add_window(|_, _| RenderCounter(Rc::new(Cell::new(0))));
    let platform = cx.test_window(handle.into());
    cx.update_window(handle.into(), |_, window, cx| {
        window.active.set(true);
        window.draw(cx).clear(cx);
    })
    .unwrap();
    assert!(platform.simulate_scheduled_frame());
    cx.run_until_parked();
    assert!(platform.simulate_scheduled_frame());
    cx.run_until_parked();
    assert!(!platform.frame_scheduled());

    cx.update_window(handle.into(), |_, window, _| {
        window.active.set(true);
        window.on_next_frame(|window, _| window.refresh());
    })
    .unwrap();
    assert!(platform.frame_scheduled());
    assert!(platform.simulate_scheduled_frame());
    cx.run_until_parked();
    assert!(platform.frame_scheduled());
    assert!(platform.simulate_scheduled_frame());
    cx.run_until_parked();
    assert!(!platform.frame_scheduled());
}

#[crate::test]
fn pending_presentation_wakes_a_parked_render_loop(cx: &mut TestAppContext) {
    cx.defer_next_window_frame_requests();
    let handle = cx.add_window(|_, _| RenderCounter(Rc::new(Cell::new(0))));
    let platform = cx.test_window(handle.into());
    cx.update_window(handle.into(), |_, window, cx| {
        window.active.set(true);
        window.draw(cx).clear(cx);
    })
    .unwrap();
    assert!(platform.simulate_scheduled_frame());
    cx.run_until_parked();
    assert!(platform.simulate_scheduled_frame());
    cx.run_until_parked();
    assert!(!platform.frame_scheduled());
    cx.update_window(handle.into(), |_, window, cx| window.draw(cx).clear(cx))
        .unwrap();
    assert!(platform.frame_scheduled());
}

#[crate::test]
fn callback_queued_during_a_frame_requests_a_follow_up(cx: &mut TestAppContext) {
    let handle = cx.add_window(|_, _| RenderCounter(Rc::new(Cell::new(0))));
    let platform = cx.test_window(handle.into());
    let callback_ran = Rc::new(Cell::new(false));
    cx.update_window(handle.into(), |_, window, _| {
        window.active.set(true);
        let callback_ran = callback_ran.clone();
        window.on_next_frame(move |window, _| {
            window.on_next_frame(move |_, _| callback_ran.set(true));
        });
    })
    .unwrap();
    assert!(platform.simulate_scheduled_frame());
    cx.run_until_parked();
    assert!(!callback_ran.get());
    assert!(platform.frame_scheduled());
    assert!(platform.simulate_scheduled_frame());
    cx.run_until_parked();
    assert!(callback_ran.get());
}

#[crate::test]
fn async_app_refresh_flushes_refresh_effect(cx: &mut TestAppContext) {
    let renders = Rc::new(Cell::new(0));
    let _handle = cx.add_window({
        let renders = renders.clone();
        move |_, _| RenderCounter(renders)
    });
    cx.run_until_parked();
    let before = renders.get();
    cx.to_async().refresh();
    assert_eq!(renders.get(), before + 1);
}

#[test]
fn retained_frame_waker_does_not_keep_a_closed_backend_alive() {
    let mut cx = TestAppContext::single();
    let handle = cx.add_window(|_, _| RenderCounter(Rc::new(Cell::new(0))));
    let platform = cx.test_window(handle.into());
    let lifetime = Rc::downgrade(&platform.0);
    let owners_before = Rc::strong_count(&platform.0);
    let waker = platform.frame_waker().unwrap();
    assert_eq!(Rc::strong_count(&platform.0), owners_before);
    cx.update_window(handle.into(), |_, window, cx| window.remove_window(cx))
        .unwrap();
    cx.run_until_parked();
    assert!(!platform.frame_scheduled());
    waker();
    assert!(!platform.frame_scheduled());
    assert!(!platform.simulate_scheduled_frame());
    drop(platform);
    drop(cx);
    assert!(lifetime.upgrade().is_none());
    waker();
}
