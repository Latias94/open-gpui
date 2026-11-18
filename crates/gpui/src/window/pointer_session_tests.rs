use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

use crate::{
    Bounds, Context, DispatchPhase, HitboxBehavior, IntoElement, MouseButton, MouseDownEvent,
    MouseUpEvent, PlatformInput, PointerCancelReason, PointerCaptureHandle, Render, Styled as _,
    TestAppContext, VisualContext as _, Window, WindowMouseEvent, canvas, point, px, size,
};

struct PointerSessionProbe {
    handles: [PointerCaptureHandle; 2],
    listener_revision: Rc<Cell<usize>>,
    cancellations: Rc<RefCell<Vec<(usize, usize, PointerCancelReason)>>>,
    releases: Rc<Cell<usize>>,
}

impl Render for PointerSessionProbe {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let handles = self.handles;
        let revision = self.listener_revision.get();
        let cancellations = self.cancellations.clone();
        let releases = self.releases.clone();
        canvas(
            move |_, window, _| {
                [0, 1].map(|owner| {
                    let handle = handles[owner];
                    let hitbox = window.insert_hitbox(
                        Bounds::new(
                            point(px(owner as f32 * 100.0), px(0.0)),
                            size(px(80.0), px(80.0)),
                        ),
                        HitboxBehavior::Normal,
                    );
                    window.bind_pointer_capture(&handle, hitbox.id).unwrap();
                    hitbox
                })
            },
            move |_, hitboxes, window, _| {
                for (owner, hitbox) in hitboxes.iter().enumerate() {
                    let target = hitbox.id;
                    let handle = handles[owner];
                    window.on_mouse_event(move |event: &MouseDownEvent, phase, window, _| {
                        if phase == DispatchPhase::Bubble && target.is_mouse_event_target(window) {
                            window.capture_pointer(&handle, event.button).unwrap();
                        }
                    });
                    let releases = releases.clone();
                    window.on_mouse_event(move |_: &MouseUpEvent, phase, window, _| {
                        if phase == DispatchPhase::Bubble && target.is_mouse_event_target(window) {
                            releases.set(releases.get() + 1);
                        }
                    });
                    let cancellations = cancellations.clone();
                    window.on_pointer_cancel(move |event, phase, window, _| {
                        if phase == DispatchPhase::Bubble && target.is_mouse_event_target(window) {
                            cancellations
                                .borrow_mut()
                                .push((owner, revision, event.reason));
                        }
                    });
                }
            },
        )
        .size_full()
    }
}

fn mouse_down(owner: usize) -> PlatformInput {
    PlatformInput::MouseDown(MouseDownEvent {
        button: MouseButton::Left,
        position: point(px(owner as f32 * 100.0 + 10.0), px(10.0)),
        ..Default::default()
    })
}

fn mouse_up() -> PlatformInput {
    PlatformInput::MouseUp(MouseUpEvent {
        button: MouseButton::Left,
        position: point(px(250.0), px(150.0)),
        ..Default::default()
    })
}

#[crate::test]
fn consumed_mouse_up_quiesces_before_cleanup_and_keeps_original_cancel_listeners(
    cx: &mut TestAppContext,
) {
    let cancellations = Rc::new(RefCell::new(Vec::new()));
    let releases = Rc::new(Cell::new(0));
    let listener_revision = Rc::new(Cell::new(1));
    let (view, cx) = cx.add_window_view({
        let cancellations = cancellations.clone();
        let releases = releases.clone();
        let listener_revision = listener_revision.clone();
        move |window, _| PointerSessionProbe {
            handles: [
                window.new_pointer_capture_handle(),
                window.new_pointer_capture_handle(),
            ],
            listener_revision,
            cancellations,
            releases,
        }
    });
    let _ = cx.update(|window, _| window.activate_window());
    cx.run_until_parked();
    cx.update(|window, cx| window.draw(cx).clear(cx));
    let platform_window = cx.test_window(cx.window_handle());
    let _subscription = cx.update(|window, _| {
        let mut block_next_up = true;
        window.intercept_window_mouse_events(move |event, window, cx| {
            if matches!(event, WindowMouseEvent::Up(_)) && block_next_up {
                block_next_up = false;
                window.quiesce_pointer_session(PointerCancelReason::CaptureRevoked, cx);
                window.quiesce_pointer_session(PointerCancelReason::CaptureRevoked, cx);
                assert!(!window.has_active_pointer_session(cx));
                assert_eq!(window.pending_pointer_cancellations.len(), 1);
                cx.stop_propagation();
                window.prevent_default();
            }
        })
    });

    cx.update(|window, cx| {
        window.dispatch_event(mouse_down(0), cx);
        assert!(window.captured_pointer().is_some());
        let result = window.dispatch_event(mouse_up(), cx);
        assert!(!result.propagate);
        assert!(result.default_prevented);
        assert!(!window.has_active_pointer_session(cx));
        assert!(cancellations.borrow().is_empty());
        assert_eq!(releases.get(), 0);

        listener_revision.set(2);
        view.update(cx, |_, cx| cx.notify());
        window.refresh();
        window.draw(cx).clear(cx);
    });
    cx.run_until_parked();

    assert_eq!(
        cancellations.borrow().as_slice(),
        &[(0, 1, PointerCancelReason::CaptureRevoked)]
    );
    assert_eq!(platform_window.native_pointer_capture_release_count(), 1);
    cx.update(|window, cx| {
        assert!(!window.flush_pending_pointer_cancellations(cx));
        window.dispatch_event(mouse_up(), cx);
    });
    cx.run_until_parked();
    assert_eq!(cancellations.borrow().len(), 1);
    assert_eq!(platform_window.native_pointer_capture_release_count(), 1);
}

#[crate::test]
fn queued_quiescence_preserves_replacement_capture_for_the_same_or_another_owner(
    cx: &mut TestAppContext,
) {
    for replacement_owner in [0, 1] {
        let cancellations = Rc::new(RefCell::new(Vec::new()));
        let releases = Rc::new(Cell::new(0));
        let (view, cx) = cx.add_window_view({
            let cancellations = cancellations.clone();
            let releases = releases.clone();
            move |window, _| PointerSessionProbe {
                handles: [
                    window.new_pointer_capture_handle(),
                    window.new_pointer_capture_handle(),
                ],
                listener_revision: Rc::new(Cell::new(1)),
                cancellations,
                releases,
            }
        });
        let _ = cx.update(|window, _| window.activate_window());
        cx.run_until_parked();
        cx.update(|window, cx| window.draw(cx).clear(cx));
        let handles = cx.update_window_entity(&view, |probe, _, _| probe.handles);
        let platform_window = cx.test_window(cx.window_handle());
        let _subscription = cx.update(|window, _| {
            let mut block_next_up = true;
            window.intercept_window_mouse_events(move |event, window, cx| {
                if matches!(event, WindowMouseEvent::Up(_)) && block_next_up {
                    block_next_up = false;
                    window.quiesce_pointer_session(PointerCancelReason::CaptureRevoked, cx);
                    cx.stop_propagation();
                    window.prevent_default();
                }
            })
        });

        let replacement = cx.update(|window, cx| {
            window.dispatch_event(mouse_down(0), cx);
            window.dispatch_event(mouse_up(), cx);
            assert!(cancellations.borrow().is_empty());

            window.dispatch_event_without_pending_pointer_cancellations(
                mouse_down(replacement_owner),
                cx,
            );
            let replacement = window.captured_pointer().unwrap();
            assert_eq!(replacement.handle(), handles[replacement_owner]);
            assert!(window.flush_pending_pointer_cancellations(cx));
            assert_eq!(window.captured_pointer(), Some(replacement));
            assert!(window.pressed_mouse_buttons.contains(MouseButton::Left));
            assert_eq!(
                cancellations.borrow().as_slice(),
                &[(0, 1, PointerCancelReason::CaptureRevoked)]
            );

            replacement
        });
        cx.run_until_parked();
        assert_eq!(
            cx.update(|window, _| window.captured_pointer()),
            Some(replacement)
        );
        assert_eq!(platform_window.native_pointer_capture_release_count(), 0);
        cx.update(|window, cx| {
            window.dispatch_event(mouse_up(), cx);
            assert!(!window.has_active_pointer_session(cx));
        });
        assert_eq!(cancellations.borrow().len(), 1);
        assert_eq!(releases.get(), 1);
    }
}
