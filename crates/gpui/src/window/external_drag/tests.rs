use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    sync::Arc,
};

use crate::{
    AnyDrag, AnyWindowHandle, App, AppContext as _, Context, CursorStyle, Empty,
    ExternalDragPayload, ExternalPaths, FileDragPaths, FileDropEvent, InputEvent as _,
    InteractiveElement as _, IntoElement, MouseButton, MouseDownEvent, MouseMoveEvent,
    ParentElement as _, PlatformExternalDragSession, PointerCancelReason, Render,
    StatefulInteractiveElement as _, Styled as _, TestAppContext, Window, WindowMouseEvent, div,
    point, px, size,
};

#[derive(Clone, Copy, Default)]
enum ResolverAction {
    #[default]
    Payload,
    Decline,
    Replace,
    Quiesce,
    Close,
}

#[derive(Default)]
struct Observed {
    resolutions: Cell<usize>,
    action: Cell<ResolverAction>,
    order: RefCell<Vec<&'static str>>,
}

struct DragView(Rc<Observed>);

impl Render for DragView {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let observed = self.0.clone();
        div().size_full().cursor_text().child(
            div()
                .id("file-drag-source")
                .w(px(4.))
                .h_full()
                .cursor_col_resize()
                .on_drag_with_external(
                    7_u32,
                    |_, _, _, cx| cx.new(|_| Empty),
                    move |_, window, cx| {
                        observed.resolutions.set(observed.resolutions.get() + 1);
                        observed.order.borrow_mut().push("resolve");
                        match observed.action.get() {
                            ResolverAction::Decline => return None,
                            ResolverAction::Replace => {
                                cx.stop_active_drag(window);
                                let view = cx.new(|_| Empty).into();
                                cx.start_active_drag(AnyDrag {
                                    window_id: window.window_handle().window_id(),
                                    source: None,
                                    view,
                                    value: Arc::new(99_u32),
                                    window_preview_offset: point(px(0.), px(0.)),
                                    cursor_style: None,
                                    button: MouseButton::Left,
                                    external_payload_source: None,
                                });
                            }
                            ResolverAction::Quiesce => {
                                window.quiesce_interaction(cx);
                            }
                            ResolverAction::Close => window.remove_window(cx),
                            ResolverAction::Payload => {}
                        }
                        Some(payload())
                    },
                ),
        )
    }
}

fn payload() -> ExternalDragPayload {
    ExternalDragPayload::Files(FileDragPaths::new([("/tmp/drag.txt".into(), false)]))
}

fn open(cx: &mut TestAppContext, observed: Rc<Observed>) -> AnyWindowHandle {
    let handle = cx
        .open_window(size(px(200.), px(200.)), move |_, _| DragView(observed))
        .into();
    cx.test_window(handle).configure_external_drag(true, true);
    cx.update_window(handle, |_, window, cx| {
        let _ = window.activate_window();
        window.draw(cx).clear(cx);
    })
    .unwrap();
    cx.run_until_parked();
    handle
}

fn start(window: &mut Window, cx: &mut App) {
    for event in [
        MouseMoveEvent {
            position: point(px(1.), px(10.)),
            ..Default::default()
        }
        .to_platform_input(),
        MouseDownEvent {
            button: MouseButton::Left,
            position: point(px(1.), px(10.)),
            ..Default::default()
        }
        .to_platform_input(),
        MouseMoveEvent {
            position: point(px(20.), px(10.)),
            pressed_button: Some(MouseButton::Left),
            ..Default::default()
        }
        .to_platform_input(),
    ] {
        window.dispatch_event(event, cx);
    }
    assert_eq!(cx.active_drag_value::<u32>(), Some(&7));
}

fn leave(window: &mut Window, cx: &mut App) {
    window.dispatch_event(
        MouseMoveEvent {
            position: point(px(220.), px(10.)),
            pressed_button: Some(MouseButton::Left),
            ..Default::default()
        }
        .to_platform_input(),
        cx,
    );
}

fn promote(cx: &mut TestAppContext, handle: AnyWindowHandle) -> PlatformExternalDragSession {
    cx.update_window(handle, |_, window, cx| {
        start(window, cx);
        leave(window, cx);
        assert!(cx.has_active_platform_drag());
        assert!(!cx.has_active_drag());
        assert!(!window.has_active_pointer_session(cx));
    })
    .unwrap();
    let sessions = cx.test_window(handle).external_drag_sessions();
    assert_eq!(sessions.len(), 1);
    assert_eq!(sessions[0].0, payload());
    sessions[0].1.clone()
}

#[crate::test]
fn drag_cursor_applies_in_its_start_event_and_stays_window_local(cx: &mut TestAppContext) {
    let handle = open(cx, Rc::default());
    let platform = cx.test_window(handle);
    let other = cx
        .open_window(size(px(100.), px(100.)), |_, _| Empty)
        .into();
    let other_platform = cx.test_window(other);
    cx.update_window(handle, |_, window, cx| {
        start(window, cx);
        assert_eq!(platform.0.lock().cursor_style, CursorStyle::ResizeColumn);
        assert_eq!(other_platform.0.lock().cursor_style, CursorStyle::Arrow);
    })
    .unwrap();
}

#[crate::test]
fn external_drag_suspends_restores_and_finishes_one_exact_session(cx: &mut TestAppContext) {
    let observed = Rc::new(Observed::default());
    let handle = open(cx, observed.clone());
    let session = promote(cx, handle);
    assert_eq!(observed.resolutions.get(), 1);
    session.dispatch(FileDropEvent::Entered {
        position: point(px(30.), px(10.)),
        paths: ExternalPaths::default(),
    });
    cx.run_until_parked();
    cx.update_window(handle, |_, _, cx| {
        assert_eq!(cx.active_drag_value::<u32>(), Some(&7));
        assert!(cx.has_active_platform_drag());
    })
    .unwrap();
    session.dispatch(FileDropEvent::Exited);
    cx.run_until_parked();
    cx.update_window(handle, |_, _, cx| {
        assert!(!cx.has_active_drag());
        assert!(cx.has_active_platform_drag());
    })
    .unwrap();
    session.dispatch(FileDropEvent::Ended);
    cx.run_until_parked();
    cx.update_window(handle, |_, _, cx| {
        assert!(!cx.has_active_drag());
        assert!(!cx.has_active_platform_drag());
    })
    .unwrap();
    assert_eq!(observed.resolutions.get(), 1);
}

#[crate::test]
fn stale_external_completion_and_reentry_preserve_the_replacement_drag(cx: &mut TestAppContext) {
    let handle = open(cx, Rc::default());
    let old = promote(cx, handle);
    cx.update_window(handle, |_, window, cx| {
        let view = cx.new(|_| Empty).into();
        cx.start_active_drag(AnyDrag {
            window_id: handle.window_id(),
            source: None,
            view,
            value: Arc::new(99_u32),
            window_preview_offset: point(px(0.), px(0.)),
            cursor_style: None,
            button: MouseButton::Left,
            external_payload_source: None,
        });
        window.refresh();
    })
    .unwrap();
    old.dispatch(FileDropEvent::Entered {
        position: point(px(50.), px(10.)),
        paths: ExternalPaths::default(),
    });
    old.dispatch(FileDropEvent::Ended);
    cx.run_until_parked();
    cx.update_window(handle, |_, _, cx| {
        assert_eq!(cx.active_drag_value::<u32>(), Some(&99));
        assert!(!cx.has_active_platform_drag());
    })
    .unwrap();
}

#[crate::test]
fn restored_platform_drag_retires_through_pointer_and_exact_native_cancellation(
    cx: &mut TestAppContext,
) {
    for exact in [false, true] {
        let handle = open(cx, Rc::default());
        let session = promote(cx, handle);
        session.dispatch(FileDropEvent::Entered {
            position: point(px(30.), px(10.)),
            paths: ExternalPaths::default(),
        });
        cx.run_until_parked();
        cx.update_window(handle, |_, window, cx| {
            if exact {
                let generation = cx
                    .active_drag_generation_for_window(handle.window_id())
                    .unwrap();
                assert!(cx.cancel_native_captured_drag(
                    handle.window_id(),
                    generation,
                    PointerCancelReason::CaptureRevoked
                ));
            } else {
                window.cancel_pointer_session(PointerCancelReason::PlatformCaptureLost, cx);
            }
            assert!(!cx.has_active_drag());
            assert!(!cx.has_active_platform_drag());
        })
        .unwrap();
        session.dispatch(FileDropEvent::Entered {
            position: point(px(30.), px(10.)),
            paths: ExternalPaths::default(),
        });
        session.dispatch(FileDropEvent::Ended);
        cx.run_until_parked();
        cx.update_window(handle, |_, _, cx| {
            assert!(!cx.has_active_drag());
            assert!(!cx.has_active_platform_drag());
        })
        .unwrap();
    }
}

#[crate::test]
fn external_payload_resolution_is_lazy_once_and_revalidates_user_code(cx: &mut TestAppContext) {
    let _keep_alive = cx.open_window(size(px(10.), px(10.)), |_, _| Empty);
    for action in [
        ResolverAction::Decline,
        ResolverAction::Replace,
        ResolverAction::Quiesce,
        ResolverAction::Close,
    ] {
        let observed = Rc::new(Observed::default());
        observed.action.set(action);
        let handle = open(cx, observed.clone());
        let platform = cx.test_window(handle);
        cx.update_window(handle, |_, window, cx| {
            start(window, cx);
            assert_eq!(observed.resolutions.get(), 0);
            leave(window, cx);
            if matches!(action, ResolverAction::Decline | ResolverAction::Replace) {
                leave(window, cx);
            }
            if matches!(action, ResolverAction::Replace) {
                assert_eq!(cx.active_drag_value::<u32>(), Some(&99));
            }
        })
        .unwrap();
        assert_eq!(observed.resolutions.get(), 1);
        assert!(platform.external_drag_sessions().is_empty());
        if !matches!(action, ResolverAction::Close) {
            cx.update_window(handle, |_, window, cx| window.remove_window(cx))
                .unwrap();
        }
        cx.run_until_parked();
    }
}

#[crate::test]
fn external_drag_visits_another_window_then_restores_the_original_source(cx: &mut TestAppContext) {
    let handle = open(cx, Rc::default());
    let destination: AnyWindowHandle = cx
        .open_window(size(px(200.), px(200.)), |_, _| Empty)
        .into();
    let session = promote(cx, handle);
    cx.update_window(destination, |_, window, cx| {
        window.dispatch_event(
            FileDropEvent::Entered {
                position: point(px(50.), px(50.)),
                paths: ExternalPaths::default(),
            }
            .to_platform_input(),
            cx,
        );
        assert!(cx.has_active_platform_drag());
        assert!(cx.active_drag_value::<ExternalPaths>().is_some());
        window.dispatch_event(FileDropEvent::Exited.to_platform_input(), cx);
        assert!(!cx.has_active_drag());
        assert!(cx.has_active_platform_drag());
    })
    .unwrap();
    session.dispatch(FileDropEvent::Entered {
        position: point(px(50.), px(10.)),
        paths: ExternalPaths::default(),
    });
    cx.run_until_parked();
    cx.update_window(handle, |_, _, cx| {
        assert_eq!(cx.active_drag_value::<u32>(), Some(&7));
        assert!(cx.has_active_platform_drag());
    })
    .unwrap();
    session.dispatch(FileDropEvent::Ended);
    cx.run_until_parked();
}

#[crate::test]
fn outbound_promotion_happens_after_leave_listeners_and_retirement_drops_session(
    cx: &mut TestAppContext,
) {
    let observed = Rc::new(Observed::default());
    let handle = open(cx, observed.clone());
    let _subscription = cx
        .update_window(handle, {
            let observed = observed.clone();
            move |_, window, _| {
                window.intercept_window_mouse_events(move |event, _, _| {
                    if matches!(event, WindowMouseEvent::Move(event) if event.position.x > px(200.))
                    {
                        observed.order.borrow_mut().push("leave");
                    }
                })
            }
        })
        .unwrap();
    let session = promote(cx, handle);
    assert_eq!(observed.order.borrow().as_slice(), &["leave", "resolve"]);
    cx.update_window(handle, |_, window, cx| {
        window.quiesce_interaction(cx);
    })
    .unwrap();
    session.dispatch(FileDropEvent::Entered {
        position: point(px(10.), px(10.)),
        paths: ExternalPaths::default(),
    });
    session.dispatch(FileDropEvent::Ended);
    cx.run_until_parked();
    cx.update_window(handle, |_, _, cx| {
        assert!(!cx.has_active_drag());
        assert!(!cx.has_active_platform_drag());
    })
    .unwrap();
}
