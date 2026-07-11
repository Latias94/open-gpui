use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    sync::Arc,
};

use crate::{
    AnyDrag, AnyView, AppContext as _, Context, DispatchPhase, Empty, Entity, HitboxBehavior,
    InteractiveElement, InteractiveText, IntoElement, KeyDownEvent, Keystroke, Modifiers,
    MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, ParentElement, PlatformInput,
    PointerCancelEvent, PointerCancelReason, PointerCaptureError, PointerCaptureHandle, Render,
    StatefulInteractiveElement, StyleRefinement, Styled, StyledText, TestAppContext, VisualContext,
    Window, WindowMouseEvent, canvas, deferred, div, point, px, size,
};

fn mouse_down(button: MouseButton, x: f32, y: f32) -> PlatformInput {
    PlatformInput::MouseDown(MouseDownEvent {
        button,
        position: point(px(x), px(y)),
        modifiers: Modifiers::none(),
        click_count: 1,
        first_mouse: false,
    })
}

fn mouse_up(button: MouseButton, x: f32, y: f32) -> PlatformInput {
    PlatformInput::MouseUp(MouseUpEvent {
        button,
        position: point(px(x), px(y)),
        modifiers: Modifiers::none(),
        click_count: 1,
    })
}

fn pointer_cancel() -> PlatformInput {
    PlatformInput::PointerCanceled(PointerCancelEvent {
        reason: PointerCancelReason::PlatformCaptureLost,
    })
}

struct PointerCancelJournalProbe {
    renders: Rc<Cell<usize>>,
    events: Rc<RefCell<Vec<(&'static str, DispatchPhase)>>>,
}

struct PointerCancelJournalRoot {
    child: Entity<PointerCancelJournalProbe>,
}

impl Render for PointerCancelJournalRoot {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().size_full().child(deferred(
            AnyView::from(self.child.clone()).cached(StyleRefinement::default().size_full()),
        ))
    }
}

impl Render for PointerCancelJournalProbe {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        self.renders.set(self.renders.get() + 1);
        let first_events = self.events.clone();
        let second_events = self.events.clone();
        canvas(
            |_, _, _| (),
            move |_, _, window, _| {
                window.on_pointer_cancel({
                    let events = first_events.clone();
                    move |_, phase, window, cx| {
                        events.borrow_mut().push(("first", phase));
                        cx.stop_propagation();
                        window.prevent_default();
                    }
                });
                window.on_pointer_cancel({
                    let events = second_events.clone();
                    move |_, phase, _, _| events.borrow_mut().push(("second", phase))
                });
            },
        )
        .size_full()
    }
}

struct CompanionButtonRoutingProbe {
    handle: PointerCaptureHandle,
    owner_right_ups: Rc<Cell<usize>>,
    underlay_right_ups: Rc<Cell<usize>>,
}

impl Render for CompanionButtonRoutingProbe {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let handle = self.handle;
        let owner_right_ups = self.owner_right_ups.clone();
        let underlay_right_ups = self.underlay_right_ups.clone();
        canvas(
            move |_, window, _| {
                let owner = window.insert_hitbox(
                    crate::Bounds::new(point(px(0.0), px(0.0)), size(px(100.0), px(100.0))),
                    HitboxBehavior::Normal,
                );
                let underlay = window.insert_hitbox(
                    crate::Bounds::new(point(px(200.0), px(0.0)), size(px(100.0), px(100.0))),
                    HitboxBehavior::Normal,
                );
                window
                    .bind_pointer_capture(&handle, owner.id)
                    .expect("the companion-button capture owner should bind");
                (owner, underlay)
            },
            move |_, hitboxes, window, _| {
                let owner = hitboxes.0.id;
                window.on_mouse_event(move |event: &MouseDownEvent, phase, window, _| {
                    if phase == DispatchPhase::Bubble
                        && event.button == MouseButton::Left
                        && owner.is_mouse_event_target(window)
                    {
                        window
                            .capture_pointer(&handle, MouseButton::Left)
                            .expect("the left-button session should capture");
                    }
                });

                let owner = hitboxes.0.id;
                window.on_mouse_event(move |event: &MouseUpEvent, phase, window, _| {
                    if phase == DispatchPhase::Bubble
                        && event.button == MouseButton::Right
                        && owner.is_mouse_event_target(window)
                    {
                        owner_right_ups.set(owner_right_ups.get() + 1);
                    }
                });

                let underlay = hitboxes.1.id;
                window.on_mouse_event(move |event: &MouseUpEvent, phase, window, _| {
                    if phase == DispatchPhase::Bubble
                        && event.button == MouseButton::Right
                        && underlay.is_mouse_event_target(window)
                    {
                        underlay_right_ups.set(underlay_right_ups.get() + 1);
                    }
                });
            },
        )
        .size_full()
    }
}

struct StatefulDivCancelProbe {
    activations: Rc<Cell<usize>>,
}

impl Render for StatefulDivCancelProbe {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let activations = self.activations.clone();
        div()
            .id("stateful-div-cancel-probe")
            .w(px(100.0))
            .h(px(100.0))
            .on_click(move |_, _, _| activations.set(activations.get() + 1))
    }
}

struct InteractiveTextCancelProbe {
    activations: Rc<Cell<usize>>,
}

impl Render for InteractiveTextCancelProbe {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let activations = self.activations.clone();
        div().child(
            InteractiveText::new("interactive-text-cancel-probe", StyledText::new("click"))
                .on_click(vec![0..5], move |_, _, _| {
                    activations.set(activations.get() + 1)
                }),
        )
    }
}

struct PointerCaptureProbe {
    handle: PointerCaptureHandle,
    visible: Rc<Cell<bool>>,
    render_count: Rc<Cell<usize>>,
    events: Rc<RefCell<Vec<(&'static str, usize)>>>,
}

struct PointerCaptureBindingProbe {
    first: PointerCaptureHandle,
    second: PointerCaptureHandle,
    duplicate_handle_error: Rc<RefCell<Option<PointerCaptureError>>>,
    duplicate_hitbox_error: Rc<RefCell<Option<PointerCaptureError>>>,
}

struct ForeignPointerCaptureBindingProbe {
    handle: PointerCaptureHandle,
    error: Rc<RefCell<Option<PointerCaptureError>>>,
}

struct PointerCaptureRoutingProbe {
    handle: PointerCaptureHandle,
    observations: Rc<RefCell<Vec<(bool, bool, bool, bool)>>>,
}

struct PointerCaptureOwnersProbe {
    first: PointerCaptureHandle,
    second: PointerCaptureHandle,
}

impl Render for PointerCaptureProbe {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let render = self.render_count.get() + 1;
        self.render_count.set(render);

        if !self.visible.get() {
            return div().into_any_element();
        }

        let handle = self.handle;
        let events = self.events.clone();
        div()
            .w(px(100.0))
            .h(px(100.0))
            .track_pointer_capture(&self.handle)
            .on_mouse_down(MouseButton::Left, move |_, window, _| {
                window
                    .capture_pointer(&handle, MouseButton::Left)
                    .expect("the rendered pointer capture handle should be bound");
                events.borrow_mut().push(("down", render));
            })
            .on_mouse_move({
                let events = self.events.clone();
                move |_, _, _| events.borrow_mut().push(("move", render))
            })
            .on_mouse_up(MouseButton::Left, {
                let events = self.events.clone();
                move |_, _, _| events.borrow_mut().push(("up", render))
            })
            .into_any_element()
    }
}

impl Render for PointerCaptureBindingProbe {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let first = self.first;
        let second = self.second;
        let duplicate_handle_error = self.duplicate_handle_error.clone();
        let duplicate_hitbox_error = self.duplicate_hitbox_error.clone();
        canvas(
            move |bounds, window, _| {
                let first_hitbox = window.insert_hitbox(bounds, HitboxBehavior::Normal);
                window
                    .bind_pointer_capture(&first, first_hitbox.id)
                    .expect("the first binding should succeed");

                let second_hitbox = window.insert_hitbox(bounds, HitboxBehavior::Normal);
                *duplicate_handle_error.borrow_mut() =
                    window.bind_pointer_capture(&first, second_hitbox.id).err();
                *duplicate_hitbox_error.borrow_mut() =
                    window.bind_pointer_capture(&second, first_hitbox.id).err();
            },
            |_, _, _, _| {},
        )
        .size_full()
    }
}

impl Render for ForeignPointerCaptureBindingProbe {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let handle = self.handle;
        let error = self.error.clone();
        canvas(
            move |bounds, window, _| {
                let hitbox = window.insert_hitbox(bounds, HitboxBehavior::Normal);
                *error.borrow_mut() = window.bind_pointer_capture(&handle, hitbox.id).err();
            },
            |_, _, _, _| {},
        )
        .size_full()
    }
}

impl Render for PointerCaptureRoutingProbe {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let handle = self.handle;
        let observations = self.observations.clone();
        canvas(
            move |_, window, _| {
                let captured = window.insert_hitbox(
                    crate::Bounds::new(point(px(0.0), px(0.0)), size(px(100.0), px(100.0))),
                    HitboxBehavior::Normal,
                );
                let physical = window.insert_hitbox(
                    crate::Bounds::new(point(px(200.0), px(0.0)), size(px(100.0), px(100.0))),
                    HitboxBehavior::Normal,
                );
                window
                    .bind_pointer_capture(&handle, captured.id)
                    .expect("captured routing hitbox should bind");
                (captured, physical)
            },
            move |_, hitboxes, window, _| {
                let captured_id = hitboxes.0.id;
                window.on_mouse_event(move |event: &MouseDownEvent, phase, window, _| {
                    if phase == crate::DispatchPhase::Bubble
                        && event.button == MouseButton::Left
                        && captured_id.is_mouse_event_target(window)
                    {
                        window
                            .capture_pointer(&handle, MouseButton::Left)
                            .expect("captured routing hitbox should own the gesture");
                    }
                });
                let captured_id = hitboxes.0.id;
                let physical_id = hitboxes.1.id;
                window.on_mouse_event(move |_: &MouseMoveEvent, phase, window, _| {
                    if phase == crate::DispatchPhase::Bubble {
                        observations.borrow_mut().push((
                            captured_id.is_mouse_event_target(window),
                            captured_id.is_hovered(window),
                            physical_id.is_mouse_event_target(window),
                            physical_id.is_hovered(window),
                        ));
                    }
                });
            },
        )
        .size_full()
    }
}

impl Render for PointerCaptureOwnersProbe {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .child(
                div()
                    .w(px(100.0))
                    .h(px(100.0))
                    .track_pointer_capture(&self.first),
            )
            .child(
                div()
                    .w(px(100.0))
                    .h(px(100.0))
                    .track_pointer_capture(&self.second),
            )
    }
}

#[open_gpui::test]
fn pointer_capture_routes_current_frame_listeners_across_redraw_and_modality_refresh(
    cx: &mut TestAppContext,
) {
    let visible = Rc::new(Cell::new(true));
    let render_count = Rc::new(Cell::new(0));
    let events = Rc::new(RefCell::new(Vec::new()));
    let (view, cx) = cx.add_window_view({
        let visible = visible.clone();
        let render_count = render_count.clone();
        let events = events.clone();
        move |window, _| PointerCaptureProbe {
            handle: window.new_pointer_capture_handle(),
            visible,
            render_count,
            events,
        }
    });
    cx.update(|window, _| window.activate_window());
    cx.run_until_parked();
    cx.update_window_entity(&view, |_, _, cx| cx.notify());
    cx.update(|window, cx| window.draw(cx).clear());
    let handle = cx.update_window_entity(&view, |view, _, _| view.handle);

    cx.update(|window, cx| {
        window.dispatch_event(
            PlatformInput::MouseDown(MouseDownEvent {
                button: MouseButton::Left,
                position: point(px(10.0), px(10.0)),
                modifiers: Modifiers::none(),
                click_count: 1,
                first_mouse: false,
            }),
            cx,
        )
    });
    assert_eq!(
        cx.update(|window, _| window.captured_pointer().map(|capture| capture.handle())),
        Some(handle)
    );

    // A redraw that reuses the view must carry its frame-scoped binding forward.
    cx.update(|window, cx| {
        window.refresh();
        window.draw(cx).clear();
    });
    assert_eq!(
        cx.update(|window, _| window.captured_pointer().map(|capture| capture.handle())),
        Some(handle)
    );

    // Switching to keyboard modality triggers the normal refresh path while capture is active.
    cx.update(|window, cx| {
        window.dispatch_event(
            PlatformInput::KeyDown(KeyDownEvent {
                keystroke: Keystroke::parse("shift").expect("shift should parse"),
                is_held: false,
                prefer_character_input: false,
            }),
            cx,
        )
    });
    assert_eq!(
        cx.update(|window, _| window.captured_pointer().map(|capture| capture.handle())),
        Some(handle)
    );

    // Force a fresh render so the event records which current-frame listener handled it.
    cx.update_window_entity(&view, |_, _, cx| cx.notify());
    cx.update(|window, cx| window.draw(cx).clear());
    let first_move_render = render_count.get();
    cx.update(|window, cx| {
        window.dispatch_event(
            PlatformInput::MouseMove(MouseMoveEvent {
                position: point(px(300.0), px(300.0)),
                pressed_button: Some(MouseButton::Left),
                modifiers: Modifiers::none(),
            }),
            cx,
        )
    });
    assert_eq!(events.borrow().last(), Some(&("move", first_move_render)));

    cx.update_window_entity(&view, |_, _, cx| cx.notify());
    cx.update(|window, cx| window.draw(cx).clear());
    let second_move_render = render_count.get();
    assert!(second_move_render > first_move_render);
    cx.update(|window, cx| {
        window.dispatch_event(
            PlatformInput::MouseMove(MouseMoveEvent {
                position: point(px(350.0), px(350.0)),
                pressed_button: Some(MouseButton::Left),
                modifiers: Modifiers::none(),
            }),
            cx,
        )
    });
    assert_eq!(events.borrow().last(), Some(&("move", second_move_render)));

    cx.update(|window, cx| {
        window.dispatch_event(
            PlatformInput::MouseUp(MouseUpEvent {
                button: MouseButton::Right,
                position: point(px(350.0), px(350.0)),
                modifiers: Modifiers::none(),
                click_count: 1,
            }),
            cx,
        )
    });
    assert_eq!(
        cx.update(|window, _| window.captured_pointer().map(|capture| capture.handle())),
        Some(handle),
        "releasing another button must not end the captured gesture"
    );

    cx.update(|window, cx| {
        window.dispatch_event(
            PlatformInput::MouseUp(MouseUpEvent {
                button: MouseButton::Left,
                position: point(px(350.0), px(350.0)),
                modifiers: Modifiers::none(),
                click_count: 1,
            }),
            cx,
        )
    });
    assert_eq!(events.borrow().last(), Some(&("up", second_move_render)));
    assert!(cx.update(|window, _| window.captured_pointer().is_none()));
}

#[open_gpui::test]
fn pointer_capture_separates_the_event_target_from_physical_hover(cx: &mut TestAppContext) {
    let observations = Rc::new(RefCell::new(Vec::new()));
    let (_view, cx) = cx.add_window_view({
        let observations = observations.clone();
        move |window, _| PointerCaptureRoutingProbe {
            handle: window.new_pointer_capture_handle(),
            observations,
        }
    });
    cx.update(|window, _| window.activate_window());
    cx.run_until_parked();
    cx.update(|window, cx| window.draw(cx).clear());

    cx.update(|window, cx| {
        window.dispatch_event(
            PlatformInput::MouseDown(MouseDownEvent {
                button: MouseButton::Left,
                position: point(px(10.0), px(10.0)),
                modifiers: Modifiers::none(),
                click_count: 1,
                first_mouse: false,
            }),
            cx,
        );
        window.dispatch_event(
            PlatformInput::MouseMove(MouseMoveEvent {
                position: point(px(250.0), px(10.0)),
                pressed_button: Some(MouseButton::Left),
                modifiers: Modifiers::none(),
            }),
            cx,
        );
    });

    assert_eq!(
        observations.borrow().as_slice(),
        &[(true, false, false, true)],
        "capture must own event dispatch without replacing physical hover"
    );
}

#[open_gpui::test]
fn pointer_capture_requires_a_pressed_button_and_rejects_competing_owners(cx: &mut TestAppContext) {
    let (view, cx) = cx.add_window_view(|window, _| PointerCaptureOwnersProbe {
        first: window.new_pointer_capture_handle(),
        second: window.new_pointer_capture_handle(),
    });
    cx.update(|window, _| window.activate_window());
    cx.run_until_parked();
    cx.update(|window, cx| window.draw(cx).clear());
    let (first, second) = cx.update_window_entity(&view, |view, _, _| (view.first, view.second));

    assert_eq!(
        cx.update(|window, _| window.capture_pointer(&first, MouseButton::Left)),
        Err(PointerCaptureError::ButtonNotPressed {
            button: MouseButton::Left,
        })
    );

    cx.update(|window, cx| {
        window.dispatch_event(
            PlatformInput::MouseDown(MouseDownEvent {
                button: MouseButton::Left,
                position: point(px(10.0), px(10.0)),
                modifiers: Modifiers::none(),
                click_count: 1,
                first_mouse: false,
            }),
            cx,
        );
        window
            .capture_pointer(&first, MouseButton::Left)
            .expect("a pressed button should be allowed to start capture");
        window
            .capture_pointer(&first, MouseButton::Left)
            .expect("repeating capture for the same session should be idempotent");
    });
    let capture = cx
        .update(|window, _| window.captured_pointer())
        .expect("the first handle should own capture");
    assert_eq!(capture.handle(), first);
    assert_eq!(capture.button(), MouseButton::Left);
    assert_eq!(
        cx.update(|window, _| window.capture_pointer(&second, MouseButton::Left)),
        Err(PointerCaptureError::PointerAlreadyCaptured {
            captured: first,
            requested: second,
        })
    );

    cx.update(|window, cx| {
        cx.active_drag = Some(AnyDrag {
            window_id: window.window_handle().window_id(),
            value: Arc::new("drag"),
            view: cx.new(|_| Empty).into(),
            cursor_offset: point(px(0.0), px(0.0)),
            cursor_style: None,
            button: MouseButton::Left,
        });
        window.dispatch_event(
            PlatformInput::MouseDown(MouseDownEvent {
                button: MouseButton::Right,
                position: point(px(10.0), px(10.0)),
                modifiers: Modifiers::none(),
                click_count: 1,
                first_mouse: false,
            }),
            cx,
        );
        window.dispatch_event(
            PlatformInput::MouseUp(MouseUpEvent {
                button: MouseButton::Right,
                position: point(px(10.0), px(10.0)),
                modifiers: Modifiers::none(),
                click_count: 1,
            }),
            cx,
        );
        assert!(cx.active_drag.is_some());
        assert_eq!(
            window.captured_pointer().map(|capture| capture.handle()),
            Some(first)
        );

        window.dispatch_event(
            PlatformInput::MouseUp(MouseUpEvent {
                button: MouseButton::Left,
                position: point(px(10.0), px(10.0)),
                modifiers: Modifiers::none(),
                click_count: 1,
            }),
            cx,
        );
        assert!(cx.active_drag.is_none());
        assert!(window.captured_pointer().is_none());
    });
}

#[open_gpui::test]
fn pointer_cancellation_is_unpreventable_and_clears_the_entire_session(cx: &mut TestAppContext) {
    let (view, cx) = cx.add_window_view(|window, _| PointerCaptureOwnersProbe {
        first: window.new_pointer_capture_handle(),
        second: window.new_pointer_capture_handle(),
    });
    cx.update(|window, _| window.activate_window());
    cx.run_until_parked();
    cx.update(|window, cx| window.draw(cx).clear());
    let first = cx.update_window_entity(&view, |view, _, _| view.first);
    let events = Rc::new(RefCell::new(Vec::new()));
    let first_events = events.clone();
    let _first_interceptor = cx.update(|window, _| {
        window.intercept_mouse_events(move |event, window, cx| {
            if let WindowMouseEvent::Cancel(event) = event {
                assert_eq!(event.reason, PointerCancelReason::WindowDeactivated);
                first_events.borrow_mut().push("first");
                cx.stop_propagation();
                window.prevent_default();
            }
        })
    });
    let second_events = events.clone();
    let _second_interceptor = cx.update(|window, _| {
        window.intercept_mouse_events(move |event, _, _| {
            if matches!(event, WindowMouseEvent::Cancel(_)) {
                second_events.borrow_mut().push("second");
            }
        })
    });

    cx.update(|window, cx| {
        window.dispatch_event(
            PlatformInput::MouseDown(MouseDownEvent {
                button: MouseButton::Left,
                position: point(px(10.0), px(10.0)),
                modifiers: Modifiers::none(),
                click_count: 1,
                first_mouse: false,
            }),
            cx,
        );
        window
            .capture_pointer(&first, MouseButton::Left)
            .expect("the pressed pointer session should capture");
        cx.active_drag = Some(AnyDrag {
            window_id: window.window_handle().window_id(),
            value: Arc::new("drag"),
            view: cx.new(|_| Empty).into(),
            cursor_offset: point(px(0.0), px(0.0)),
            cursor_style: None,
            button: MouseButton::Left,
        });
    });

    cx.deactivate_window();
    assert_eq!(events.borrow().as_slice(), &["first", "second"]);
    cx.update(|window, cx| {
        assert!(window.captured_pointer().is_none());
        assert!(cx.active_drag.is_none());
    });

    cx.update(|window, _| window.activate_window());
    cx.run_until_parked();
    assert_eq!(
        cx.update(|window, _| window.capture_pointer(&first, MouseButton::Left)),
        Err(PointerCaptureError::ButtonNotPressed {
            button: MouseButton::Left,
        })
    );
}

#[open_gpui::test]
fn pointer_capture_releases_when_owner_is_absent_from_the_next_frame(cx: &mut TestAppContext) {
    let visible = Rc::new(Cell::new(true));
    let render_count = Rc::new(Cell::new(0));
    let events = Rc::new(RefCell::new(Vec::new()));
    let (view, cx) = cx.add_window_view({
        let visible = visible.clone();
        let render_count = render_count.clone();
        let events = events.clone();
        move |window, _| PointerCaptureProbe {
            handle: window.new_pointer_capture_handle(),
            visible,
            render_count,
            events,
        }
    });
    cx.update(|window, _| window.activate_window());
    cx.run_until_parked();
    cx.update_window_entity(&view, |_, _, cx| cx.notify());
    cx.update(|window, cx| window.draw(cx).clear());

    cx.update(|window, cx| {
        window.dispatch_event(
            PlatformInput::MouseDown(MouseDownEvent {
                button: MouseButton::Left,
                position: point(px(10.0), px(10.0)),
                modifiers: Modifiers::none(),
                click_count: 1,
                first_mouse: false,
            }),
            cx,
        )
    });
    assert!(cx.update(|window, _| window.captured_pointer().is_some()));

    visible.set(false);
    cx.update_window_entity(&view, |_, _, cx| cx.notify());
    cx.update(|window, cx| window.draw(cx).clear());
    assert!(cx.update(|window, _| window.captured_pointer().is_none()));

    events.borrow_mut().clear();
    cx.update(|window, cx| {
        window.dispatch_event(
            PlatformInput::MouseMove(MouseMoveEvent {
                position: point(px(300.0), px(300.0)),
                pressed_button: Some(MouseButton::Left),
                modifiers: Modifiers::none(),
            }),
            cx,
        )
    });
    assert!(events.borrow().is_empty());
}

#[open_gpui::test]
fn pointer_capture_supports_explicit_release_and_clears_on_window_deactivation(
    cx: &mut TestAppContext,
) {
    let visible = Rc::new(Cell::new(true));
    let render_count = Rc::new(Cell::new(0));
    let events = Rc::new(RefCell::new(Vec::new()));
    let (view, cx) = cx.add_window_view({
        let visible = visible.clone();
        let render_count = render_count.clone();
        let events = events.clone();
        move |window, _| PointerCaptureProbe {
            handle: window.new_pointer_capture_handle(),
            visible,
            render_count,
            events,
        }
    });
    cx.update(|window, _| window.activate_window());
    cx.run_until_parked();
    cx.update_window_entity(&view, |_, _, cx| cx.notify());
    cx.update(|window, cx| window.draw(cx).clear());
    let handle = cx.update_window_entity(&view, |view, _, _| view.handle);

    let mouse_down = || {
        PlatformInput::MouseDown(MouseDownEvent {
            button: MouseButton::Left,
            position: point(px(10.0), px(10.0)),
            modifiers: Modifiers::none(),
            click_count: 1,
            first_mouse: false,
        })
    };
    cx.update(|window, cx| window.dispatch_event(mouse_down(), cx));
    assert_eq!(
        cx.update(|window, _| window.release_pointer(&handle)),
        Ok(true)
    );
    assert_eq!(
        cx.update(|window, _| window.release_pointer(&handle)),
        Ok(false)
    );

    events.borrow_mut().clear();
    cx.update(|window, cx| {
        window.dispatch_event(
            PlatformInput::MouseMove(MouseMoveEvent {
                position: point(px(300.0), px(300.0)),
                pressed_button: Some(MouseButton::Left),
                modifiers: Modifiers::none(),
            }),
            cx,
        )
    });
    assert!(events.borrow().is_empty());

    cx.update(|window, cx| window.dispatch_event(mouse_down(), cx));
    assert_eq!(
        cx.update(|window, _| window.captured_pointer().map(|capture| capture.handle())),
        Some(handle)
    );
    cx.deactivate_window();
    assert!(cx.update(|window, _| window.captured_pointer().is_none()));
    assert!(matches!(
        cx.update(|window, _| window.capture_pointer(&handle, MouseButton::Left)),
        Err(PointerCaptureError::WindowInactive { .. })
    ));
}

#[open_gpui::test]
fn pointer_capture_clears_when_the_window_is_removed(cx: &mut TestAppContext) {
    let visible = Rc::new(Cell::new(true));
    let render_count = Rc::new(Cell::new(0));
    let events = Rc::new(RefCell::new(Vec::new()));
    let (view, cx) = cx.add_window_view({
        let visible = visible.clone();
        let render_count = render_count.clone();
        let events = events.clone();
        move |window, _| PointerCaptureProbe {
            handle: window.new_pointer_capture_handle(),
            visible,
            render_count,
            events,
        }
    });
    cx.update(|window, _| window.activate_window());
    cx.run_until_parked();
    cx.update_window_entity(&view, |_, _, cx| cx.notify());
    cx.update(|window, cx| window.draw(cx).clear());
    cx.update(|window, cx| {
        window.dispatch_event(
            PlatformInput::MouseDown(MouseDownEvent {
                button: MouseButton::Left,
                position: point(px(10.0), px(10.0)),
                modifiers: Modifiers::none(),
                click_count: 1,
                first_mouse: false,
            }),
            cx,
        );
        assert!(window.captured_pointer().is_some());
        window.remove_window(cx);
        assert!(window.captured_pointer().is_none());
    });
}

#[open_gpui::test]
fn pointer_capture_binding_rejects_duplicate_handles_and_hitboxes(cx: &mut TestAppContext) {
    let duplicate_handle_error = Rc::new(RefCell::new(None));
    let duplicate_hitbox_error = Rc::new(RefCell::new(None));
    let (view, cx) = cx.add_window_view({
        let duplicate_handle_error = duplicate_handle_error.clone();
        let duplicate_hitbox_error = duplicate_hitbox_error.clone();
        move |window, _| PointerCaptureBindingProbe {
            first: window.new_pointer_capture_handle(),
            second: window.new_pointer_capture_handle(),
            duplicate_handle_error,
            duplicate_hitbox_error,
        }
    });
    cx.update_window_entity(&view, |_, _, cx| cx.notify());
    cx.update(|window, cx| window.draw(cx).clear());
    let first = cx.update_window_entity(&view, |view, _, _| view.first);

    assert_eq!(
        *duplicate_handle_error.borrow(),
        Some(PointerCaptureError::HandleAlreadyBound { handle: first })
    );
    assert!(matches!(
        *duplicate_hitbox_error.borrow(),
        Some(PointerCaptureError::HitboxAlreadyBound { .. })
    ));
}

#[open_gpui::test]
fn pointer_capture_handles_cannot_cross_windows(cx: &mut TestAppContext) {
    let first_handle = Rc::new(Cell::new(None));
    let first_window: crate::AnyWindowHandle = cx
        .open_window(size(px(320.0), px(200.0)), {
            let first_handle = first_handle.clone();
            move |window, _| {
                first_handle.set(Some(window.new_pointer_capture_handle()));
                Empty
            }
        })
        .into();
    let handle = first_handle
        .get()
        .expect("the first window should create its pointer capture handle");
    let bind_error = Rc::new(RefCell::new(None));
    let second_window: crate::AnyWindowHandle = cx
        .open_window(size(px(320.0), px(200.0)), {
            let bind_error = bind_error.clone();
            move |_, _| ForeignPointerCaptureBindingProbe {
                handle,
                error: bind_error,
            }
        })
        .into();

    cx.update_window(second_window, |_, window, cx| window.draw(cx).clear())
        .expect("the second window should remain open");
    assert!(matches!(
        *bind_error.borrow(),
        Some(PointerCaptureError::WrongWindow { .. })
    ));
    assert!(matches!(
        cx.update_window(second_window, |_, window, _| window
            .capture_pointer(&handle, MouseButton::Left)),
        Ok(Err(PointerCaptureError::WrongWindow { .. }))
    ));
    assert!(matches!(
        cx.update_window(second_window, |_, window, _| window
            .release_pointer(&handle)),
        Ok(Err(PointerCaptureError::WrongWindow { .. }))
    ));

    assert_ne!(first_window, second_window);
}

#[open_gpui::test]
fn removing_an_unrelated_window_preserves_the_drag_owned_by_another_window(
    cx: &mut TestAppContext,
) {
    let first: crate::AnyWindowHandle = cx
        .open_window(size(px(320.0), px(200.0)), |_, _| Empty)
        .into();
    let second: crate::AnyWindowHandle = cx
        .open_window(size(px(320.0), px(200.0)), |_, _| Empty)
        .into();

    cx.update(|app| {
        app.active_drag = Some(AnyDrag {
            window_id: second.window_id(),
            value: Arc::new("second-window-drag"),
            view: app.new(|_| Empty).into(),
            cursor_offset: point(px(0.0), px(0.0)),
            cursor_style: None,
            button: MouseButton::Left,
        });
    });

    cx.update_window(first, |_, window, cx| window.remove_window(cx))
        .expect("the unrelated window should close");
    cx.update(|app| {
        assert_eq!(
            app.active_drag.as_ref().map(|drag| drag.window_id),
            Some(second.window_id())
        );
    });

    cx.update_window(second, |_, window, cx| window.remove_window(cx))
        .expect("the drag-owning window should close");
    cx.update(|app| assert!(app.active_drag.is_none()));
}

#[open_gpui::test]
fn pointer_cancel_listeners_are_isolated_unpreventable_and_replayed_from_cached_paint(
    cx: &mut TestAppContext,
) {
    let renders = Rc::new(Cell::new(0));
    let events = Rc::new(RefCell::new(Vec::new()));
    let (_view, cx) = cx.add_window_view({
        let renders = renders.clone();
        let events = events.clone();
        move |_, cx| PointerCancelJournalRoot {
            child: cx.new(|_| PointerCancelJournalProbe { renders, events }),
        }
    });
    cx.update(|window, cx| window.draw(cx).clear());
    let initial_renders = renders.get();

    cx.update(|window, cx| {
        window.dispatch_event(
            PlatformInput::MouseMove(MouseMoveEvent {
                position: point(px(20.0), px(20.0)),
                pressed_button: None,
                modifiers: Modifiers::none(),
            }),
            cx,
        );
        window.dispatch_event(mouse_down(MouseButton::Left, 20.0, 20.0), cx);
        window.dispatch_event(mouse_up(MouseButton::Left, 20.0, 20.0), cx);
    });
    assert!(
        events.borrow().is_empty(),
        "ordinary mouse dispatch must not call cancellation-only listeners"
    );

    cx.update(|window, cx| {
        window.draw(cx).clear();
    });
    assert_eq!(
        renders.get(),
        initial_renders,
        "the unchanged view should replay its cached paint journal"
    );

    cx.update(|window, cx| window.dispatch_event(pointer_cancel(), cx));
    assert_eq!(
        events.borrow().as_slice(),
        &[
            ("first", DispatchPhase::Capture),
            ("second", DispatchPhase::Capture),
            ("second", DispatchPhase::Bubble),
            ("first", DispatchPhase::Bubble),
        ],
        "stopping propagation must not suppress any cancellation listener or phase"
    );
}

#[open_gpui::test]
fn stateful_div_cancel_prevents_activation_even_when_cancel_propagation_is_stopped(
    cx: &mut TestAppContext,
) {
    let activations = Rc::new(Cell::new(0));
    let (view, cx) = cx.add_window_view({
        let activations = activations.clone();
        move |_, _| StatefulDivCancelProbe { activations }
    });
    cx.update(|window, cx| window.draw(cx).clear());

    cx.update(|window, cx| {
        window.dispatch_event(mouse_down(MouseButton::Left, 10.0, 10.0), cx);
        window.dispatch_event(mouse_up(MouseButton::Left, 10.0, 10.0), cx);
    });
    assert_eq!(activations.get(), 1, "the control click should activate");
    activations.set(0);
    cx.update_window_entity(&view, |_, _, cx| cx.notify());
    cx.update(|window, cx| window.draw(cx).clear());

    let _cancel_interceptor = cx.update(|window, _| {
        window.intercept_mouse_events(|event, window, cx| {
            if matches!(event, WindowMouseEvent::Cancel(_)) {
                cx.stop_propagation();
                window.prevent_default();
            }
        })
    });
    cx.update(|window, cx| {
        window.dispatch_event(mouse_down(MouseButton::Left, 10.0, 10.0), cx);
        window.dispatch_event(pointer_cancel(), cx);
    });
    cx.update_window_entity(&view, |_, _, cx| cx.notify());
    cx.update(|window, cx| window.draw(cx).clear());
    cx.update(|window, cx| window.dispatch_event(mouse_up(MouseButton::Left, 10.0, 10.0), cx));

    assert_eq!(
        activations.get(),
        0,
        "a canceled stateful div must not activate on a later mouse up"
    );
}

#[open_gpui::test]
fn interactive_text_cancel_before_redraw_prevents_later_activation(cx: &mut TestAppContext) {
    let activations = Rc::new(Cell::new(0));
    let (view, cx) = cx.add_window_view({
        let activations = activations.clone();
        move |_, _| InteractiveTextCancelProbe { activations }
    });
    cx.update(|window, cx| window.draw(cx).clear());

    cx.update(|window, cx| window.dispatch_event(mouse_down(MouseButton::Left, 2.0, 2.0), cx));
    cx.update_window_entity(&view, |_, _, cx| cx.notify());
    cx.update(|window, cx| window.draw(cx).clear());
    cx.update(|window, cx| window.dispatch_event(mouse_up(MouseButton::Left, 2.0, 2.0), cx));
    assert_eq!(
        activations.get(),
        1,
        "the control text click should activate"
    );
    activations.set(0);
    cx.update_window_entity(&view, |_, _, cx| cx.notify());
    cx.update(|window, cx| window.draw(cx).clear());

    cx.update(|window, cx| {
        window.dispatch_event(mouse_down(MouseButton::Left, 2.0, 2.0), cx);
        window.dispatch_event(pointer_cancel(), cx);
    });
    cx.update_window_entity(&view, |_, _, cx| cx.notify());
    cx.update(|window, cx| window.draw(cx).clear());
    cx.update(|window, cx| window.dispatch_event(mouse_up(MouseButton::Left, 2.0, 2.0), cx));

    assert_eq!(
        activations.get(),
        0,
        "cancel must clear same-frame text press state before the redraw registers mouse up"
    );
}

#[open_gpui::test]
fn companion_button_up_routes_only_to_the_pointer_capture_owner(cx: &mut TestAppContext) {
    let owner_right_ups = Rc::new(Cell::new(0));
    let underlay_right_ups = Rc::new(Cell::new(0));
    let (view, cx) = cx.add_window_view({
        let owner_right_ups = owner_right_ups.clone();
        let underlay_right_ups = underlay_right_ups.clone();
        move |window, _| CompanionButtonRoutingProbe {
            handle: window.new_pointer_capture_handle(),
            owner_right_ups,
            underlay_right_ups,
        }
    });
    cx.update(|window, _| window.activate_window());
    cx.run_until_parked();
    cx.update(|window, cx| window.draw(cx).clear());
    let handle = cx.update_window_entity(&view, |view, _, _| view.handle);

    cx.update(|window, cx| {
        window.dispatch_event(mouse_down(MouseButton::Left, 10.0, 10.0), cx);
        window.dispatch_event(mouse_down(MouseButton::Right, 250.0, 10.0), cx);
        window.dispatch_event(mouse_up(MouseButton::Right, 250.0, 10.0), cx);
    });

    assert_eq!(owner_right_ups.get(), 1);
    assert_eq!(underlay_right_ups.get(), 0);
    assert_eq!(
        cx.update(|window, _| window.captured_pointer().map(|capture| capture.handle())),
        Some(handle),
        "releasing the companion button must preserve the left-button capture"
    );

    cx.update(|window, cx| window.dispatch_event(mouse_up(MouseButton::Left, 250.0, 10.0), cx));
    assert!(cx.update(|window, _| window.captured_pointer().is_none()));
}
