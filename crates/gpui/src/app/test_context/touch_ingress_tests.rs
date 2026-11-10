use std::{
    cell::{Cell, RefCell},
    ops::Range,
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
    sync::Arc,
};

use crate::{
    AnyDrag, AnyWindowHandle, App, AppContext as _, Bounds, Context, Empty, FocusHandle,
    InputHandler, InteractiveElement, IntoElement, MouseButton, NativeBoundaryDiagnosticCursor,
    NativeBoundaryDisposition, NativeBoundaryGeneration, NativeBoundaryKind, NativeBoundaryTarget,
    NativeCallbackKind, NativeInputBoundary, NativeInputHandlerOperation,
    NativePointerCancelReservation, ParentElement, Pixels, PlatformInput, PlatformInputCallback,
    PlatformInputCallbackSlot, PlatformInputHandler, Render, Styled, SubtreeTransform,
    SubtreeTransformExt, SubtreeTransformOrigin, TestAppContext, TouchEvent, TouchId, TouchPhase,
    UTF16Selection, VisualContext as _, Window, WindowMouseEvent, canvas, div, point, px, size,
};

fn touch(id: u64, phase: TouchPhase, x: f32) -> TouchEvent {
    TouchEvent {
        id: TouchId(id),
        phase,
        position: point(px(x), px(20.0)),
        ..Default::default()
    }
}

fn leased_input_slot(cx: &TestAppContext, window: AnyWindowHandle) -> PlatformInputCallbackSlot {
    let slot = PlatformInputCallbackSlot::default();
    let async_app = cx.to_async();
    slot.set(PlatformInputCallback::new_for_window(
        async_app.clone(),
        window.window_id(),
        Box::new(move |event| {
            async_app
                .dispatch_native_window_input(window.window_id(), event)
                .expect("the leased callback must reach its live window")
        }),
    ));
    slot
}

#[crate::test]
fn reentrant_touch_cancel_preserves_ingress_order_contact_and_mouse_session(
    cx: &mut TestAppContext,
) {
    let source: AnyWindowHandle = cx
        .open_window(size(px(320.0), px(200.0)), |_, _| Empty)
        .into();
    let other: AnyWindowHandle = cx
        .open_window(size(px(320.0), px(200.0)), |_, _| Empty)
        .into();
    let slot = leased_input_slot(cx, source);
    let other_slot = leased_input_slot(cx, other);
    let order = Rc::new(RefCell::new(Vec::new()));
    let cancelled_positions = Rc::new(RefCell::new(Vec::new()));
    let platform_window = cx.test_window(source);
    let activation_subscription = cx
        .update_window(source, {
            let order = order.clone();
            move |_, window, _| {
                let (subscription, activate) = window.activation_observers.insert(
                    (),
                    Box::new(move |_, _| {
                        order.borrow_mut().push("earlier-native-event");
                        true
                    }),
                );
                activate();
                subscription
            }
        })
        .unwrap();
    let source_interceptor = cx
        .update_window(source, {
            let slot = slot.clone();
            let order = order.clone();
            let cancelled_positions = cancelled_positions.clone();
            move |_, window, _| {
                window.intercept_window_mouse_events(move |event, window, _| {
                    let WindowMouseEvent::TouchDrag(event) = event else {
                        return;
                    };
                    match event.phase {
                        TouchPhase::Started => window.prevent_default(),
                        TouchPhase::Moved => {
                            order.borrow_mut().push("move");
                            platform_window.simulate_active_status_change(true);
                            for event in [
                                touch(99, TouchPhase::Cancelled, 99.0),
                                touch(41, TouchPhase::Cancelled, 70.0),
                            ] {
                                assert_eq!(
                                    slot.reserve_reentrant_touch_cancel(event),
                                    NativePointerCancelReservation::Reserved
                                );
                            }
                            assert!(cancelled_positions.borrow().is_empty());
                            order.borrow_mut().push("reentry-returned");
                        }
                        TouchPhase::Cancelled => {
                            cancelled_positions.borrow_mut().push(event.position);
                            order.borrow_mut().push("cancel");
                        }
                        TouchPhase::Ended => {}
                    }
                })
            }
        })
        .unwrap();
    let other_phases = Rc::new(RefCell::new(Vec::new()));
    let other_interceptor = cx
        .update_window(other, {
            let other_phases = other_phases.clone();
            move |_, window, _| {
                window.intercept_window_mouse_events(move |event, window, _| {
                    if let WindowMouseEvent::TouchDrag(event) = event {
                        other_phases.borrow_mut().push(event.phase);
                        if event.phase == TouchPhase::Started {
                            window.prevent_default();
                        }
                    }
                })
            }
        })
        .unwrap();
    slot.dispatch(PlatformInput::Touch(touch(41, TouchPhase::Started, 10.0)));
    other_slot.dispatch(PlatformInput::Touch(touch(41, TouchPhase::Started, 15.0)));
    cx.update_window(source, |_, window, cx| {
        let view = cx.new(|_| Empty).into();
        cx.start_active_drag(AnyDrag {
            window_id: source.window_id(),
            source: None,
            value: Arc::new("mouse-session-independent-of-touch"),
            view,
            window_preview_offset: point(px(0.0), px(0.0)),
            cursor_style: None,
            button: MouseButton::Left,
            external_payload_source: None,
        });
        assert!(window.has_active_pointer_session(cx));
    })
    .unwrap();
    cx.run_until_parked();
    order.borrow_mut().clear();
    let cursor = cx
        .app
        .native_boundary_diagnostics(NativeBoundaryDiagnosticCursor::default())
        .cursor;

    slot.dispatch(PlatformInput::Touch(touch(41, TouchPhase::Moved, 30.0)));
    cx.run_until_parked();

    assert_eq!(
        order.borrow().as_slice(),
        &["move", "reentry-returned", "earlier-native-event", "cancel"]
    );
    assert_eq!(
        cancelled_positions.borrow().as_slice(),
        &[point(px(70.0), px(20.0))],
        "the unrelated contact must not cancel the active contact or coalesce its terminal"
    );
    let diagnostics = cx.app.native_boundary_diagnostics(cursor);
    let terminals = diagnostics
        .terminal
        .iter()
        .filter(|diagnostic| {
            diagnostic.target == NativeBoundaryTarget::Window(source.window_id())
                && diagnostic.kind
                    == NativeBoundaryKind::Callback(NativeCallbackKind::PlatformInput)
        })
        .collect::<Vec<_>>();
    assert_eq!(
        terminals.len(),
        3,
        "one movement and two distinct cancellations"
    );
    assert!(terminals.iter().all(|diagnostic| {
        diagnostic.domain_generation
            == Some(NativeBoundaryGeneration::InputSlot {
                boundary: NativeInputBoundary::PlatformInput,
                generation: 1,
            })
    }));
    assert!(
        terminals
            .windows(2)
            .all(|pair| pair[0].sequence < pair[1].sequence)
    );
    assert!(cx.read(|app| app.active_drag.is_some()));
    cx.update_window(source, |_, window, cx| {
        assert!(window.has_active_pointer_session(cx));
        assert!(cx.stop_active_drag(window));
    })
    .unwrap();
    other_slot.dispatch(PlatformInput::Touch(touch(41, TouchPhase::Moved, 35.0)));
    assert_eq!(
        other_phases.borrow().as_slice(),
        &[TouchPhase::Started, TouchPhase::Moved]
    );
    let _ = (
        activation_subscription,
        source_interceptor,
        other_interceptor,
    );
}

#[crate::test]
fn reentrant_touch_cancel_after_quiescence_delivers_only_terminal_cleanup(cx: &mut TestAppContext) {
    let source: AnyWindowHandle = cx
        .open_window(size(px(320.0), px(200.0)), |_, _| Empty)
        .into();
    let slot = leased_input_slot(cx, source);
    let phases = Rc::new(RefCell::new(Vec::new()));
    let nested_rejected = Rc::new(Cell::new(false));
    let interceptor = cx
        .update_window(source, {
            let slot = slot.clone();
            let phases = phases.clone();
            let nested_rejected = nested_rejected.clone();
            move |_, window, _| {
                window.intercept_window_mouse_events(move |event, window, cx| {
                    let WindowMouseEvent::TouchDrag(event) = event else {
                        return;
                    };
                    phases.borrow_mut().push(event.phase);
                    match event.phase {
                        TouchPhase::Started => window.prevent_default(),
                        TouchPhase::Moved => {
                            // Admission is about to retire the whole Window. The queued terminal
                            // must settle through its cleanup entry even for another contact.
                            assert_eq!(
                                slot.reserve_reentrant_touch_cancel(touch(
                                    99,
                                    TouchPhase::Cancelled,
                                    50.0
                                )),
                                NativePointerCancelReservation::Reserved
                            );
                            assert!(window.quiesce_interaction(cx));
                        }
                        TouchPhase::Cancelled => {
                            let nested = window.dispatch_event(
                                PlatformInput::Touch(touch(2, TouchPhase::Started, 60.0)),
                                cx,
                            );
                            nested_rejected.set(!nested.propagate && nested.default_prevented);
                        }
                        TouchPhase::Ended => {}
                    }
                })
            }
        })
        .unwrap();
    slot.dispatch(PlatformInput::Touch(touch(1, TouchPhase::Started, 10.0)));
    let cursor = cx
        .app
        .native_boundary_diagnostics(NativeBoundaryDiagnosticCursor::default())
        .cursor;

    slot.dispatch(PlatformInput::Touch(touch(1, TouchPhase::Moved, 30.0)));
    cx.run_until_parked();

    assert_eq!(
        phases.borrow().as_slice(),
        &[
            TouchPhase::Started,
            TouchPhase::Moved,
            TouchPhase::Cancelled
        ]
    );
    assert!(nested_rejected.get());
    let diagnostics = cx.app.native_boundary_diagnostics(cursor);
    assert_eq!(
        diagnostics
            .terminal
            .iter()
            .filter(|diagnostic| {
                diagnostic.target == NativeBoundaryTarget::Window(source.window_id())
                    && diagnostic.kind
                        == NativeBoundaryKind::Callback(NativeCallbackKind::PlatformInput)
                    && diagnostic.disposition == NativeBoundaryDisposition::DELIVERED
                    && diagnostic.domain_generation
                        == Some(NativeBoundaryGeneration::InputSlot {
                            boundary: NativeInputBoundary::PlatformInput,
                            generation: 1,
                        })
            })
            .count(),
        1,
        "the reserved terminal must be delivered after admission is revoked, not discarded as stale"
    );
    let fresh = slot.dispatch(PlatformInput::Touch(touch(2, TouchPhase::Started, 60.0)));
    assert!(!fresh.propagate && fresh.default_prevented);
    assert_eq!(
        phases.borrow().len(),
        3,
        "terminal cleanup cannot admit a new touch"
    );
    let _ = interceptor;
}

#[crate::test]
fn reentrant_touch_cancel_cannot_cross_callback_replacement(cx: &mut TestAppContext) {
    let source: AnyWindowHandle = cx
        .open_window(size(px(320.0), px(200.0)), |_, _| Empty)
        .into();
    let slot = leased_input_slot(cx, source);
    let phases = Rc::new(RefCell::new(Vec::new()));
    let interceptor = cx
        .update_window(source, {
            let slot = slot.clone();
            let phases = phases.clone();
            move |_, window, _| {
                window.intercept_window_mouse_events(move |event, window, cx| {
                    let WindowMouseEvent::TouchDrag(event) = event else {
                        return;
                    };
                    phases.borrow_mut().push(event.phase);
                    if event.phase == TouchPhase::Started {
                        window.prevent_default();
                    } else if event.phase == TouchPhase::Moved {
                        assert_eq!(
                            slot.reserve_reentrant_touch_cancel(touch(
                                1,
                                TouchPhase::Cancelled,
                                50.0
                            )),
                            NativePointerCancelReservation::Reserved
                        );
                        let async_app = cx.to_async();
                        slot.set(PlatformInputCallback::new_for_window(
                            async_app.clone(),
                            source.window_id(),
                            Box::new(move |input| {
                                async_app
                                    .dispatch_native_window_input(source.window_id(), input)
                                    .unwrap()
                            }),
                        ));
                        assert_eq!(
                            slot.reserve_reentrant_touch_cancel(touch(
                                1,
                                TouchPhase::Cancelled,
                                60.0
                            )),
                            NativePointerCancelReservation::RetiredSlot,
                            "the checked-out callback cannot reserve against its replacement"
                        );
                    }
                })
            }
        })
        .unwrap();
    slot.dispatch(PlatformInput::Touch(touch(1, TouchPhase::Started, 10.0)));
    let cursor = cx
        .app
        .native_boundary_diagnostics(NativeBoundaryDiagnosticCursor::default())
        .cursor;
    slot.dispatch(PlatformInput::Touch(touch(1, TouchPhase::Moved, 30.0)));
    cx.run_until_parked();

    assert_eq!(
        phases.borrow().as_slice(),
        &[TouchPhase::Started, TouchPhase::Moved]
    );
    let diagnostics = cx.app.native_boundary_diagnostics(cursor);
    assert!(diagnostics.terminal.iter().any(|diagnostic| {
        diagnostic.target == NativeBoundaryTarget::Window(source.window_id())
            && diagnostic.kind == NativeBoundaryKind::Callback(NativeCallbackKind::PlatformInput)
            && diagnostic.disposition == NativeBoundaryDisposition::Stale
            && diagnostic.domain_generation
                == Some(NativeBoundaryGeneration::InputSlot {
                    boundary: NativeInputBoundary::PlatformInput,
                    generation: 1,
                })
    }));
    slot.dispatch(PlatformInput::Touch(touch(1, TouchPhase::Ended, 30.0)));
    slot.dispatch(PlatformInput::Touch(touch(2, TouchPhase::Started, 10.0)));
    assert_eq!(
        phases.borrow().as_slice(),
        &[
            TouchPhase::Started,
            TouchPhase::Moved,
            TouchPhase::Ended,
            TouchPhase::Started
        ]
    );
    let _ = interceptor;
}

#[crate::test]
fn reentrant_touch_cancel_rejects_ordinary_input_and_retired_slots(cx: &mut TestAppContext) {
    let source: AnyWindowHandle = cx
        .open_window(size(px(320.0), px(200.0)), |_, _| Empty)
        .into();
    let slot = PlatformInputCallbackSlot::default();
    slot.set(PlatformInputCallback::new_for_window(
        cx.to_async(),
        source.window_id(),
        Box::new({
            let slot = slot.clone();
            move |_| {
                for phase in [TouchPhase::Started, TouchPhase::Moved, TouchPhase::Ended] {
                    assert_eq!(
                        slot.reserve_reentrant_touch_cancel(touch(1, phase, 10.0)),
                        NativePointerCancelReservation::InvalidTouchPhase
                    );
                }
                Default::default()
            }
        }),
    ));
    let cursor = cx
        .app
        .native_boundary_diagnostics(NativeBoundaryDiagnosticCursor::default())
        .cursor;
    slot.dispatch(PlatformInput::Touch(touch(1, TouchPhase::Started, 10.0)));
    assert!(
        cx.app
            .native_boundary_diagnostics(cursor)
            .pending
            .is_empty()
    );
    assert_eq!(
        slot.reserve_reentrant_touch_cancel(touch(1, TouchPhase::Cancelled, 10.0)),
        NativePointerCancelReservation::NoActiveCallback
    );
    slot.terminate();
    assert_eq!(
        slot.reserve_reentrant_touch_cancel(touch(1, TouchPhase::Cancelled, 10.0)),
        NativePointerCancelReservation::RetiredSlot
    );
}

type BoundsQuery =
    Rc<RefCell<dyn FnMut(Bounds<Pixels>, &mut Window, &mut App) -> Option<Bounds<Pixels>>>>;

struct BoundsQueryHandler {
    bounds: Bounds<Pixels>,
    query: BoundsQuery,
}

impl InputHandler for BoundsQueryHandler {
    fn selected_text_range(
        &mut self,
        _: bool,
        _: &mut Window,
        _: &mut App,
    ) -> Option<UTF16Selection> {
        None
    }

    fn marked_text_range(&mut self, _: &mut Window, _: &mut App) -> Option<Range<usize>> {
        None
    }

    fn text_for_range(
        &mut self,
        _: Range<usize>,
        _: &mut Option<Range<usize>>,
        _: &mut Window,
        _: &mut App,
    ) -> Option<String> {
        None
    }

    fn replace_text_in_range(
        &mut self,
        _: Option<Range<usize>>,
        _: &str,
        _: &mut Window,
        _: &mut App,
    ) {
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        _: Option<Range<usize>>,
        _: &str,
        _: Option<Range<usize>>,
        _: &mut Window,
        _: &mut App,
    ) {
    }

    fn unmark_text(&mut self, _: &mut Window, _: &mut App) {}

    fn bounds_for_range(
        &mut self,
        _: Range<usize>,
        _: &mut Window,
        _: &mut App,
    ) -> Option<Bounds<Pixels>> {
        Some(self.bounds)
    }

    fn element_bounds(&mut self, window: &mut Window, cx: &mut App) -> Option<Bounds<Pixels>> {
        (self.query.borrow_mut())(self.bounds, window, cx)
    }

    fn character_index_for_point(
        &mut self,
        _: crate::Point<Pixels>,
        _: &mut Window,
        _: &mut App,
    ) -> Option<usize> {
        None
    }
}

struct BoundsQueryView {
    focus: FocusHandle,
    query: BoundsQuery,
    translated: bool,
}

impl Render for BoundsQueryView {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let focus = self.focus.clone();
        let query = self.query.clone();
        div()
            .track_focus(&self.focus)
            .w(px(40.0))
            .h(px(20.0))
            .child(
                canvas(
                    |_, _, _| (),
                    move |bounds, (), window, cx| {
                        window.handle_input(
                            &focus,
                            BoundsQueryHandler {
                                bounds,
                                query: query.clone(),
                            },
                            cx,
                        );
                    },
                )
                .size_full(),
            )
            .with_subtree_transform(
                SubtreeTransform::try_new(
                    size(2.0, 3.0),
                    if self.translated {
                        point(px(100.0), px(50.0))
                    } else {
                        point(px(0.0), px(0.0))
                    },
                    SubtreeTransformOrigin::TOP_LEFT,
                )
                .unwrap(),
            )
    }
}

#[crate::test]
fn element_bounds_uses_the_current_committed_transform_and_native_operation(
    cx: &mut TestAppContext,
) {
    let (view, mut cx) = cx.add_window_view(|_, cx| BoundsQueryView {
        focus: cx.focus_handle(),
        query: Rc::new(RefCell::new(|bounds, _: &mut Window, _: &mut App| {
            Some(bounds)
        })),
        translated: false,
    });
    let _ = cx.update(|window, _| window.activate_window());
    cx.update_window_entity(&view, |view, window, cx| {
        view.focus.focus(window, cx);
        cx.notify();
    });
    cx.update(|window, cx| window.draw(cx).clear(cx));
    let slot = cx
        .update(|window, _| window.platform_window.input_handler_slot_for_test())
        .unwrap();
    assert_eq!(
        slot.with_handler(PlatformInputHandler::element_bounds)
            .flatten(),
        Some(Bounds::new(
            point(px(0.0), px(0.0)),
            size(px(80.0), px(60.0))
        ))
    );
    cx.update_window_entity(&view, |view, _, cx| {
        view.translated = true;
        cx.notify();
    });
    cx.update(|window, cx| window.draw(cx).clear(cx));
    let cursor = cx
        .app
        .native_boundary_diagnostics(NativeBoundaryDiagnosticCursor::default())
        .cursor;
    let expected = Some(Bounds::new(
        point(px(100.0), px(50.0)),
        size(px(80.0), px(60.0)),
    ));
    slot.with_handler(|handler| {
        assert_eq!(handler.element_bounds(), expected);
        assert_eq!(handler.bounds_for_range(0..0), expected);
    })
    .unwrap();
    let diagnostics = cx.app.native_boundary_diagnostics(cursor);
    assert!(diagnostics.terminal.iter().any(|diagnostic| {
        diagnostic.target == NativeBoundaryTarget::Window(cx.window_handle().window_id())
            && diagnostic.kind
                == NativeBoundaryKind::Callback(NativeCallbackKind::PlatformInputHandler(
                    NativeInputHandlerOperation::ElementBounds,
                ))
            && diagnostic.domain_generation.is_some_and(|generation| {
                matches!(
                    generation,
                    NativeBoundaryGeneration::InputSlot {
                        boundary: NativeInputBoundary::InputHandler,
                        ..
                    }
                )
            })
    }));
}

#[crate::test]
fn element_bounds_restores_its_lease_after_unwind_and_defers_window_removal(
    cx: &mut TestAppContext,
) {
    let panic_next = Rc::new(Cell::new(true));
    let remove_next = Rc::new(Cell::new(false));
    let (view, mut cx) = cx.add_window_view({
        let panic_next = panic_next.clone();
        let remove_next = remove_next.clone();
        move |_, cx| BoundsQueryView {
            focus: cx.focus_handle(),
            query: Rc::new(RefCell::new(
                move |bounds, window: &mut Window, cx: &mut App| {
                    assert!(!panic_next.replace(false), "injected element bounds panic");
                    if remove_next.get() {
                        window.remove_window(cx);
                        assert!(
                            !window.removed,
                            "removal must wait for the bounds query transaction"
                        );
                    }
                    Some(bounds)
                },
            )),
            translated: false,
        }
    });
    let _ = cx.update(|window, _| window.activate_window());
    cx.update_window_entity(&view, |view, window, cx| {
        view.focus.focus(window, cx);
        cx.notify();
    });
    cx.update(|window, cx| window.draw(cx).clear(cx));
    let slot = cx
        .update(|window, _| window.platform_window.input_handler_slot_for_test())
        .unwrap();
    let panic = catch_unwind(AssertUnwindSafe(|| {
        slot.with_handler(PlatformInputHandler::element_bounds)
    }))
    .expect_err("the public query must preserve its original panic");
    assert!(
        panic
            .downcast_ref::<String>()
            .is_some_and(|text| text.contains("injected element bounds panic"))
            || panic
                .downcast_ref::<&str>()
                .is_some_and(|text| text.contains("injected element bounds panic"))
    );
    assert!(
        slot.with_handler(PlatformInputHandler::element_bounds)
            .flatten()
            .is_some()
    );
    remove_next.set(true);
    slot.with_handler(PlatformInputHandler::element_bounds)
        .unwrap();
    cx.run_until_parked();
    assert!(cx.windows().is_empty());
}
