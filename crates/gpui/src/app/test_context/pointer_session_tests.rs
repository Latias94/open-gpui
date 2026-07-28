use std::{
    cell::{Cell, RefCell},
    ops::Range,
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
    sync::Arc,
};

use crate::{
    AnyDrag, AnyView, App, AppContext as _, Bounds, Context, DispatchPhase, Empty, Entity,
    EventEmitter, FocusHandle, Focusable, HitboxBehavior, InputHandler, InteractiveElement,
    InteractiveText, IntoElement, KeyBinding, KeyDownEvent, Keystroke, Modifiers, MouseButton,
    MouseDownEvent, MouseMoveEvent, MouseUpEvent, NativeBoundaryDiagnosticCursor,
    NativeBoundaryDisposition, NativeBoundaryKind, NativeBoundaryTarget, NativePlatformCommandKind,
    ParentElement, Pixels, PlatformInput, Point, PointerCancelEvent, PointerCancelReason,
    PointerCaptureError, PointerCaptureHandle, PromptLevel, PromptResponse, Render,
    StatefulInteractiveElement, StyleRefinement, Styled, StyledText, SubtreePresentation,
    SubtreePresentationExt, TestAppContext, UTF16Selection, VisualContext, Window,
    WindowMouseEvent, canvas, deferred, div, point, px, size,
};

crate::actions!(pointer_session_actions, [RemoveWindowWithPointer]);

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

fn record_window_close(
    cx: &mut TestAppContext,
    lifecycle: Rc<RefCell<Vec<&'static str>>>,
    close_count: Rc<Cell<usize>>,
) -> crate::Subscription {
    cx.update(|cx| {
        cx.on_window_closed(move |cx, window_id| {
            close_count.set(close_count.get() + 1);
            lifecycle.borrow_mut().push("closed");
            assert!(
                cx.update_window_id(window_id, |_, _, _| ()).is_err(),
                "close observers must run after registry removal"
            );
        })
    })
}

struct PointerCancelJournalProbe {
    renders: Rc<Cell<usize>>,
    events: Rc<RefCell<Vec<(&'static str, DispatchPhase)>>>,
}

struct PointerCancelJournalRoot {
    child: Entity<PointerCancelJournalProbe>,
}

struct WindowLocalDragSource;

struct DraggablePrompt {
    focus: FocusHandle,
    drag_starts: Rc<Cell<usize>>,
}

impl EventEmitter<PromptResponse> for DraggablePrompt {}

impl Focusable for DraggablePrompt {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl Render for DraggablePrompt {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let drag_starts = self.drag_starts.clone();
        div()
            .id("draggable-custom-prompt")
            .size_full()
            .focusable()
            .track_focus(&self.focus)
            .on_drag(11_u32, move |_, _, _, cx| {
                drag_starts.set(drag_starts.get() + 1);
                cx.new(|_| Empty)
            })
    }
}

impl Render for WindowLocalDragSource {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("window-local-drag-source")
            .size_full()
            .on_drag(7_u32, |_, _, _, cx| cx.new(|_| Empty))
    }
}

struct WindowLocalDropTarget {
    drag_moves: Rc<Cell<usize>>,
    drops: Rc<Cell<usize>>,
    can_drop_checks: Rc<Cell<usize>>,
    drag_over_styles: Rc<Cell<usize>>,
}

impl Render for WindowLocalDropTarget {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let drag_moves = self.drag_moves.clone();
        let drops = self.drops.clone();
        let can_drop_checks = self.can_drop_checks.clone();
        let drag_over_styles = self.drag_over_styles.clone();
        div()
            .size_full()
            .can_drop(move |_, _, _| {
                can_drop_checks.set(can_drop_checks.get() + 1);
                true
            })
            .drag_over::<u32>(move |style, _, _, _| {
                drag_over_styles.set(drag_over_styles.get() + 1);
                style
            })
            .on_drag_move::<u32>(move |_, _, _| drag_moves.set(drag_moves.get() + 1))
            .on_drop::<u32>(move |_, _, _| drops.set(drops.get() + 1))
    }
}

struct RepeatedDropTarget {
    can_drop_checks: Rc<Cell<usize>>,
    first_drops: Rc<Cell<usize>>,
    second_drops: Rc<Cell<usize>>,
}

impl Render for RepeatedDropTarget {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let can_drop_checks = self.can_drop_checks.clone();
        let first_drops = self.first_drops.clone();
        let second_drops = self.second_drops.clone();
        div()
            .size_full()
            .can_drop(move |_, _, _| {
                can_drop_checks.set(can_drop_checks.get() + 1);
                true
            })
            .on_drop::<u32>(move |_, _, _| first_drops.set(first_drops.get() + 1))
            .on_drop::<u32>(move |_, _, _| second_drops.set(second_drops.get() + 1))
    }
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

struct MixedCachedPointerCancelRoot {
    presentation: SubtreePresentation,
    capture: PointerCaptureHandle,
    cached_child: Entity<PointerCancelJournalProbe>,
    events: Rc<RefCell<Vec<(&'static str, DispatchPhase)>>>,
}

impl Render for MixedCachedPointerCancelRoot {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let capture = self.capture;
        let events = self.events.clone();
        let owner = canvas(
            move |bounds, window, _| {
                let hitbox = window.insert_hitbox(bounds, HitboxBehavior::Normal);
                window.bind_pointer_capture(&capture, hitbox.id).unwrap();
            },
            move |_, _, window, _| {
                let events = events.clone();
                window.on_pointer_cancel(move |_, phase, _, _| {
                    events.borrow_mut().push(("owner", phase));
                });
            },
        )
        .size_full()
        .with_subtree_presentation(self.presentation);

        div()
            .size_full()
            .child(deferred(
                AnyView::from(self.cached_child.clone())
                    .cached(StyleRefinement::default().size_full()),
            ))
            .child(owner)
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

struct DragPreviewProbe {
    renders: Rc<Cell<usize>>,
}

struct ActionWindowRemovalProbe {
    handle: PointerCaptureHandle,
    focus: FocusHandle,
    lifecycle: Rc<RefCell<Vec<&'static str>>>,
}

struct TextInputWindowRemovalProbe {
    handle: PointerCaptureHandle,
    focus: FocusHandle,
    lifecycle: Rc<RefCell<Vec<&'static str>>>,
    remove_on_key: bool,
}

struct MarkedTextWindowRemovalProbe {
    focus: FocusHandle,
    lifecycle: Rc<RefCell<Vec<&'static str>>>,
    remove_on_marked_text: bool,
    activate_on_marked_text: bool,
}

struct SwitchingMarkedTextProbe {
    first_focus: FocusHandle,
    second_focus: FocusHandle,
    lifecycle: Rc<RefCell<Vec<&'static str>>>,
}

#[derive(Clone)]
struct SwitchingMarkedTextInputHandler {
    name: &'static str,
    next_focus: Option<FocusHandle>,
    redraw_target: crate::WeakEntity<SwitchingMarkedTextProbe>,
    lifecycle: Rc<RefCell<Vec<&'static str>>>,
}

impl InputHandler for SwitchingMarkedTextInputHandler {
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
        window: &mut Window,
        cx: &mut App,
    ) {
        self.lifecycle.borrow_mut().push(self.name);
        let Some(next_focus) = self.next_focus.as_ref() else {
            return;
        };

        next_focus.focus(window, cx);
        self.redraw_target
            .update(cx, |_, cx| cx.notify())
            .expect("the marked-text probe should remain alive during its callback");
        window.draw(cx).clear();
    }

    fn unmark_text(&mut self, _: &mut Window, _: &mut App) {}

    fn bounds_for_range(
        &mut self,
        _: Range<usize>,
        _: &mut Window,
        _: &mut App,
    ) -> Option<Bounds<Pixels>> {
        None
    }

    fn character_index_for_point(
        &mut self,
        _: Point<Pixels>,
        _: &mut Window,
        _: &mut App,
    ) -> Option<usize> {
        None
    }
}

#[derive(Clone)]
struct WindowRemovalInputHandler {
    lifecycle: Rc<RefCell<Vec<&'static str>>>,
    remove_on_marked_text: bool,
    activate_on_marked_text: bool,
}

impl InputHandler for WindowRemovalInputHandler {
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
        window: &mut Window,
        cx: &mut App,
    ) {
        self.lifecycle.borrow_mut().push("input");
        window.remove_window(cx);
        window.remove_window(cx);
        assert!(!window.removed, "removal must wait for the input callback");
        self.lifecycle.borrow_mut().push("input-returned");
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        _: Option<Range<usize>>,
        _: &str,
        _: Option<Range<usize>>,
        window: &mut Window,
        cx: &mut App,
    ) {
        self.lifecycle.borrow_mut().push("marked-input");
        if self.activate_on_marked_text {
            window.activate_window();
        }
        if self.remove_on_marked_text {
            window.remove_window(cx);
            self.lifecycle.borrow_mut().push("marked-input-returned");
        }
    }

    fn unmark_text(&mut self, _: &mut Window, _: &mut App) {}

    fn bounds_for_range(
        &mut self,
        _: Range<usize>,
        _: &mut Window,
        _: &mut App,
    ) -> Option<Bounds<Pixels>> {
        None
    }

    fn character_index_for_point(
        &mut self,
        _: Point<Pixels>,
        _: &mut Window,
        _: &mut App,
    ) -> Option<usize> {
        None
    }
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

impl Render for DragPreviewProbe {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        self.renders.set(self.renders.get() + 1);
        div().w(px(1.0)).h(px(1.0))
    }
}

impl Render for ActionWindowRemovalProbe {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let handle = self.handle;
        let lifecycle = self.lifecycle.clone();
        div()
            .size_full()
            .track_focus(&self.focus)
            .track_pointer_capture(&self.handle)
            .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                window
                    .capture_pointer(&handle, MouseButton::Left)
                    .expect("mouse down should establish pointer capture before the action");
                cx.active_drag = Some(AnyDrag {
                    window_id: window.window_handle().window_id(),
                    source: None,
                    value: Arc::new("drag"),
                    view: cx.new(|_| Empty).into(),
                    window_preview_offset: point(px(0.0), px(0.0)),
                    cursor_style: None,
                    button: MouseButton::Left,
                });
            })
            .on_action(move |_: &RemoveWindowWithPointer, window, cx| {
                lifecycle.borrow_mut().push("action");
                window.remove_window(cx);
                window.remove_window(cx);
                lifecycle.borrow_mut().push("action-returned");
            })
    }
}

impl Render for TextInputWindowRemovalProbe {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let handle = self.handle;
        let key_lifecycle = self.lifecycle.clone();
        let remove_on_key = self.remove_on_key;
        let input_focus = self.focus.clone();
        let input_handler = WindowRemovalInputHandler {
            lifecycle: self.lifecycle.clone(),
            remove_on_marked_text: false,
            activate_on_marked_text: false,
        };
        div()
            .id("text-input-window-removal-probe")
            .size_full()
            .focusable()
            .track_focus(&self.focus)
            .track_pointer_capture(&self.handle)
            .on_mouse_down(MouseButton::Left, move |_, window, _| {
                window
                    .capture_pointer(&handle, MouseButton::Left)
                    .expect("mouse down should establish pointer capture before text input");
            })
            .on_key_down(move |_, window, cx| {
                key_lifecycle.borrow_mut().push("key");
                if remove_on_key {
                    window.remove_window(cx);
                    window.remove_window(cx);
                    assert!(!window.removed, "removal must wait for the key callback");
                }
                key_lifecycle.borrow_mut().push("key-returned");
            })
            .child(
                canvas(
                    |_, _, _| (),
                    move |_, _, window, cx| {
                        window.handle_input(&input_focus, input_handler.clone(), cx);
                    },
                )
                .absolute()
                .size_full(),
            )
    }
}

impl Render for MarkedTextWindowRemovalProbe {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let input_focus = self.focus.clone();
        let input_handler = WindowRemovalInputHandler {
            lifecycle: self.lifecycle.clone(),
            remove_on_marked_text: self.remove_on_marked_text,
            activate_on_marked_text: self.activate_on_marked_text,
        };

        div()
            .id("marked-text-window-removal-probe")
            .size_full()
            .focusable()
            .track_focus(&self.focus)
            .child(
                canvas(
                    |_, _, _| (),
                    move |_, _, window, cx| {
                        window.handle_input(&input_focus, input_handler.clone(), cx);
                    },
                )
                .absolute()
                .size_full(),
            )
    }
}

impl Render for SwitchingMarkedTextProbe {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let redraw_target = cx.entity().downgrade();
        let first_handler = SwitchingMarkedTextInputHandler {
            name: "first",
            next_focus: Some(self.second_focus.clone()),
            redraw_target: redraw_target.clone(),
            lifecycle: self.lifecycle.clone(),
        };
        let second_handler = SwitchingMarkedTextInputHandler {
            name: "second",
            next_focus: None,
            redraw_target,
            lifecycle: self.lifecycle.clone(),
        };
        let first_focus = self.first_focus.clone();
        let second_focus = self.second_focus.clone();

        div()
            .size_full()
            .child(
                div()
                    .id("first-marked-text-handler")
                    .focusable()
                    .track_focus(&self.first_focus),
            )
            .child(
                div()
                    .id("second-marked-text-handler")
                    .focusable()
                    .track_focus(&self.second_focus),
            )
            .child(
                canvas(
                    |_, _, _| (),
                    move |_, _, window, cx| {
                        window.handle_input(&first_focus, first_handler.clone(), cx);
                        window.handle_input(&second_focus, second_handler.clone(), cx);
                    },
                )
                .absolute()
                .size_full(),
            )
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
            source: None,
            value: Arc::new("drag"),
            view: cx.new(|_| Empty).into(),
            window_preview_offset: point(px(0.0), px(0.0)),
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
fn stopping_an_active_drag_releases_its_source_pointer_capture(cx: &mut TestAppContext) {
    let (view, cx) = cx.add_window_view(|window, _| PointerCaptureOwnersProbe {
        first: window.new_pointer_capture_handle(),
        second: window.new_pointer_capture_handle(),
    });
    cx.update(|window, _| window.activate_window());
    cx.run_until_parked();
    cx.update(|window, cx| window.draw(cx).clear());
    let source = cx.update_window_entity(&view, |view, _, _| view.first);

    cx.update(|window, cx| {
        window.dispatch_event(mouse_down(MouseButton::Left, 10.0, 10.0), cx);
        window
            .capture_pointer(&source, MouseButton::Left)
            .expect("the drag source should capture the pressed pointer");
        cx.active_drag = Some(AnyDrag {
            window_id: window.window_handle().window_id(),
            source: Some(source),
            value: Arc::new("drag"),
            view: cx.new(|_| Empty).into(),
            window_preview_offset: point(px(0.0), px(0.0)),
            cursor_style: None,
            button: MouseButton::Left,
        });

        assert!(cx.stop_active_drag(window));
        assert!(cx.active_drag.is_none());
        assert!(window.captured_pointer().is_none());
        assert!(!window.has_active_pointer_session(cx));
    });
}

#[open_gpui::test]
fn stopping_a_cross_window_drag_releases_capture_in_its_source_window(cx: &mut TestAppContext) {
    let source_handle = Rc::new(Cell::new(None));
    let source_window: crate::AnyWindowHandle = cx
        .open_window(size(px(320.0), px(200.0)), {
            let source_handle = source_handle.clone();
            move |window, _| {
                let first = window.new_pointer_capture_handle();
                source_handle.set(Some(first));
                PointerCaptureOwnersProbe {
                    first,
                    second: window.new_pointer_capture_handle(),
                }
            }
        })
        .into();
    let target_window: crate::AnyWindowHandle = cx
        .open_window(size(px(320.0), px(200.0)), |_, _| Empty)
        .into();
    let source = source_handle
        .get()
        .expect("the source window should create its capture handle");

    cx.update_window(source_window, |_, window, cx| {
        window.activate_window();
        window.draw(cx).clear();
    })
    .expect("the source window should remain open");
    cx.run_until_parked();
    cx.update_window(source_window, |_, window, cx| {
        window.draw(cx).clear();
        window.dispatch_event(mouse_down(MouseButton::Left, 10.0, 10.0), cx);
        window
            .capture_pointer(&source, MouseButton::Left)
            .expect("the source window should capture the drag pointer");
        cx.active_drag = Some(AnyDrag {
            window_id: source_window.window_id(),
            source: Some(source),
            value: Arc::new("cross-window-drag"),
            view: cx.new(|_| Empty).into(),
            window_preview_offset: point(px(0.0), px(0.0)),
            cursor_style: None,
            button: MouseButton::Left,
        });
    })
    .expect("the source window should start the drag");

    cx.update_window(target_window, |_, window, cx| {
        assert!(cx.stop_active_drag(window));
    })
    .expect("the target window should stop the drag");

    assert!(cx.read(|cx| cx.active_drag.is_none()));
    assert!(
        cx.update_window(source_window, |_, window, cx| {
            window.captured_pointer().is_none() && !window.has_active_pointer_session(cx)
        })
        .expect("the source window should remain open")
    );
}

#[open_gpui::test]
fn standard_drag_targets_reject_drags_from_another_window(cx: &mut TestAppContext) {
    let drag_moves = Rc::new(Cell::new(0));
    let drops = Rc::new(Cell::new(0));
    let can_drop_checks = Rc::new(Cell::new(0));
    let drag_over_styles = Rc::new(Cell::new(0));
    let source_window: crate::AnyWindowHandle = cx
        .open_window(size(px(320.0), px(200.0)), |_, _| WindowLocalDragSource)
        .into();
    let target_window: crate::AnyWindowHandle = cx
        .open_window(size(px(320.0), px(200.0)), {
            let drag_moves = drag_moves.clone();
            let drops = drops.clone();
            let can_drop_checks = can_drop_checks.clone();
            let drag_over_styles = drag_over_styles.clone();
            move |_, _| WindowLocalDropTarget {
                drag_moves,
                drops,
                can_drop_checks,
                drag_over_styles,
            }
        })
        .into();

    cx.update_window(source_window, |_, window, cx| {
        window.activate_window();
        window.draw(cx).clear();
    })
    .expect("the source window should remain open");
    cx.run_until_parked();
    cx.update_window(target_window, |_, window, cx| window.draw(cx).clear())
        .expect("the target window should remain open");
    cx.update_window(source_window, |_, window, cx| {
        window.draw(cx).clear();
        window.dispatch_event(mouse_down(MouseButton::Left, 10.0, 10.0), cx);
        window.dispatch_event(
            PlatformInput::MouseMove(MouseMoveEvent {
                position: point(px(30.0), px(10.0)),
                pressed_button: Some(MouseButton::Left),
                modifiers: Modifiers::none(),
            }),
            cx,
        );
        assert!(cx.active_drag.is_some(), "the source should start a drag");
        assert!(
            window.captured_pointer().is_some(),
            "the source should own pointer capture"
        );
    })
    .expect("the source window should remain open");

    cx.update_window(target_window, |_, window, cx| {
        window.refresh();
        window.draw(cx).clear();
        window.dispatch_event(
            PlatformInput::MouseMove(MouseMoveEvent {
                position: point(px(10.0), px(10.0)),
                pressed_button: Some(MouseButton::Left),
                modifiers: Modifiers::none(),
            }),
            cx,
        );
        window.dispatch_event(mouse_up(MouseButton::Left, 10.0, 10.0), cx);
    })
    .expect("the target window should accept isolated input");

    assert_eq!(drag_moves.get(), 0);
    assert_eq!(drops.get(), 0);
    assert_eq!(can_drop_checks.get(), 0);
    assert_eq!(drag_over_styles.get(), 0);
    assert_eq!(
        cx.read(|cx| cx.active_drag.as_ref().map(|drag| drag.window_id)),
        Some(source_window.window_id()),
        "the unrelated target must not consume the source drag"
    );
    assert!(
        cx.update_window(source_window, |_, window, cx| {
            window.captured_pointer().is_some() && window.has_active_pointer_session(cx)
        })
        .expect("the source window should remain open"),
        "the source session must remain intact after unrelated input"
    );

    cx.update_window(source_window, |_, window, cx| {
        window.dispatch_event(mouse_up(MouseButton::Left, 30.0, 10.0), cx);
        assert!(window.captured_pointer().is_none());
        assert!(!window.has_active_pointer_session(cx));
    })
    .expect("the source window should finish its drag");
    assert!(cx.read(|cx| cx.active_drag.is_none()));
}

#[open_gpui::test]
fn repeated_drop_listeners_receive_one_matching_drop_each(cx: &mut TestAppContext) {
    let can_drop_checks = Rc::new(Cell::new(0));
    let first_drops = Rc::new(Cell::new(0));
    let second_drops = Rc::new(Cell::new(0));
    let window_handle: crate::AnyWindowHandle = cx
        .open_window(size(px(320.0), px(200.0)), {
            let can_drop_checks = can_drop_checks.clone();
            let first_drops = first_drops.clone();
            let second_drops = second_drops.clone();
            move |_, _| RepeatedDropTarget {
                can_drop_checks,
                first_drops,
                second_drops,
            }
        })
        .into();

    cx.update_window(window_handle, |_, window, cx| {
        window.activate_window();
        window.draw(cx).clear();
    })
    .expect("the drop target window should remain open");
    cx.run_until_parked();
    cx.update_window(window_handle, |_, window, cx| {
        window.draw(cx).clear();
        cx.active_drag = Some(AnyDrag {
            window_id: window_handle.window_id(),
            source: None,
            value: Arc::new(7_u32),
            view: cx.new(|_| Empty).into(),
            window_preview_offset: point(px(0.0), px(0.0)),
            cursor_style: None,
            button: MouseButton::Left,
        });
        window.dispatch_event(mouse_up(MouseButton::Left, 10.0, 10.0), cx);
    })
    .expect("the drop target window should dispatch the drop");

    assert_eq!(can_drop_checks.get(), 1);
    assert_eq!(first_drops.get(), 1);
    assert_eq!(second_drops.get(), 1);
    assert!(cx.read(|cx| cx.active_drag.is_none()));
}

#[open_gpui::test]
fn drag_source_in_custom_prompt_survives_its_first_drag_frame(cx: &mut TestAppContext) {
    let drag_starts = Rc::new(Cell::new(0));
    cx.update({
        let drag_starts = drag_starts.clone();
        move |cx| {
            cx.set_prompt_builder(move |_, _, _, _, handle, window, cx| {
                let drag_starts = drag_starts.clone();
                let view = cx.new(|cx| DraggablePrompt {
                    focus: cx.focus_handle(),
                    drag_starts,
                });
                handle.with_view(view, window, cx)
            });
        }
    });
    let window_handle: crate::AnyWindowHandle = cx
        .open_window(size(px(320.0), px(200.0)), |_, _| Empty)
        .into();
    let _response = cx
        .update_window(window_handle, |_, window, cx| {
            window.activate_window();
            window.draw(cx).clear();
            window.prompt(PromptLevel::Info, "Drag prompt", None, &["OK"], cx)
        })
        .expect("the prompt window should remain open");
    cx.run_until_parked();

    cx.update_window(window_handle, |_, window, cx| {
        window.draw(cx).clear();
        window.dispatch_event(mouse_down(MouseButton::Left, 10.0, 10.0), cx);
        window.dispatch_event(
            PlatformInput::MouseMove(MouseMoveEvent {
                position: point(px(30.0), px(10.0)),
                pressed_button: Some(MouseButton::Left),
                modifiers: Modifiers::none(),
            }),
            cx,
        );
        assert!(cx.active_drag.is_some());
        assert!(window.captured_pointer().is_some());
    })
    .expect("the prompt should start its drag");

    cx.run_until_parked();
    cx.update_window(window_handle, |_, window, cx| {
        assert_eq!(drag_starts.get(), 1);
        assert!(cx.active_drag.is_some());
        assert!(window.captured_pointer().is_some());
        assert!(window.has_active_pointer_session(cx));
        window.dispatch_event(mouse_up(MouseButton::Left, 30.0, 10.0), cx);
        assert!(cx.active_drag.is_none());
        assert!(window.captured_pointer().is_none());
    })
    .expect("the prompt drag should remain owned until release");
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
        window.intercept_window_mouse_events(move |event, window, cx| {
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
        window.intercept_window_mouse_events(move |event, _, _| {
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
            source: None,
            value: Arc::new("drag"),
            view: cx.new(|_| Empty).into(),
            window_preview_offset: point(px(0.0), px(0.0)),
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
    let cancellations = Rc::new(RefCell::new(Vec::new()));
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
    let _cancel_subscription = cx.update(|window, _| {
        let cancellations = cancellations.clone();
        window.intercept_window_mouse_events(move |event, _, _| {
            if let WindowMouseEvent::Cancel(event) = event {
                cancellations.borrow_mut().push(event.reason);
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
        cx.active_drag = Some(AnyDrag {
            window_id: window.window_handle().window_id(),
            source: None,
            value: Arc::new("drag"),
            view: cx.new(|_| Empty).into(),
            window_preview_offset: point(px(0.0), px(0.0)),
            cursor_style: None,
            button: MouseButton::Left,
        });
    });
    assert!(cx.update(|window, _| window.captured_pointer().is_some()));

    visible.set(false);
    cx.update_window_entity(&view, |_, _, cx| cx.notify());
    cx.update(|window, cx| {
        window.draw(cx).clear();
        window.draw(cx).clear();
    });
    assert_eq!(
        cancellations.borrow().as_slice(),
        &[PointerCancelReason::CaptureRevoked]
    );
    cx.update(|window, cx| {
        assert!(window.captured_pointer().is_none());
        assert!(cx.active_drag.is_none());
        assert!(!window.has_active_pointer_session(cx));
        assert_eq!(
            window.capture_pointer(&handle, MouseButton::Left),
            Err(PointerCaptureError::ButtonNotPressed {
                button: MouseButton::Left,
            })
        );
    });

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
fn completed_missing_owner_frame_revokes_before_a_later_rebind(cx: &mut TestAppContext) {
    let visible = Rc::new(Cell::new(true));
    let render_count = Rc::new(Cell::new(0));
    let events = Rc::new(RefCell::new(Vec::new()));
    let cancellations = Rc::new(RefCell::new(Vec::new()));
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
    let _cancel_subscription = cx.update(|window, _| {
        let cancellations = cancellations.clone();
        window.intercept_window_mouse_events(move |event, _, _| {
            if let WindowMouseEvent::Cancel(event) = event {
                cancellations.borrow_mut().push(event.reason);
            }
        })
    });
    cx.update(|window, cx| {
        window.dispatch_event(mouse_down(MouseButton::Left, 10.0, 10.0), cx);
    });
    assert!(cx.update(|window, _| window.captured_pointer().is_some()));

    visible.set(false);
    cx.update_window_entity(&view, |_, _, cx| cx.notify());
    let rebound_visible = visible.clone();
    let rebound_view = view.clone();
    cx.update(|window, cx| {
        window.defer(cx, move |window, cx| {
            rebound_visible.set(true);
            rebound_view.update(cx, |_, cx| cx.notify());
            window.draw(cx).clear();
        });
        window.draw(cx).clear();
    });

    assert_eq!(
        cancellations.borrow().as_slice(),
        &[PointerCancelReason::CaptureRevoked]
    );
    assert!(cx.update(|window, _| window.captured_pointer().is_none()));
}

#[open_gpui::test]
fn completed_missing_owner_frame_does_not_duplicate_cancellation_on_later_window_close(
    cx: &mut TestAppContext,
) {
    let visible = Rc::new(Cell::new(true));
    let render_count = Rc::new(Cell::new(0));
    let events = Rc::new(RefCell::new(Vec::new()));
    let cancellations = Rc::new(RefCell::new(Vec::new()));
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
    let _cancel_subscription = cx.update(|window, _| {
        let cancellations = cancellations.clone();
        window.intercept_window_mouse_events(move |event, _, _| {
            if let WindowMouseEvent::Cancel(event) = event {
                cancellations.borrow_mut().push(event.reason);
            }
        })
    });
    cx.update(|window, cx| {
        window.dispatch_event(mouse_down(MouseButton::Left, 10.0, 10.0), cx);
    });

    visible.set(false);
    cx.update_window_entity(&view, |_, _, cx| cx.notify());
    cx.update(|window, cx| {
        window.defer(cx, |window, cx| window.remove_window(cx));
        window.draw(cx).clear();
        window.draw(cx).clear();
    });

    assert_eq!(
        cancellations.borrow().as_slice(),
        &[PointerCancelReason::CaptureRevoked]
    );
    assert!(cx.windows().is_empty());
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
fn remove_window_from_input_callback_cancels_after_dispatch_before_removal(
    cx: &mut TestAppContext,
) {
    let visible = Rc::new(Cell::new(true));
    let render_count = Rc::new(Cell::new(0));
    let pointer_events = Rc::new(RefCell::new(Vec::new()));
    let lifecycle = Rc::new(RefCell::new(Vec::new()));
    let close_count = Rc::new(Cell::new(0));
    let _close_subscription = record_window_close(cx, lifecycle.clone(), close_count.clone());
    let (view, cx) = cx.add_window_view({
        let visible = visible.clone();
        let render_count = render_count.clone();
        let pointer_events = pointer_events.clone();
        move |window, _| PointerCaptureProbe {
            handle: window.new_pointer_capture_handle(),
            visible,
            render_count,
            events: pointer_events,
        }
    });
    cx.update(|window, _| window.activate_window());
    cx.run_until_parked();
    cx.update_window_entity(&view, |_, _, cx| cx.notify());
    cx.update(|window, cx| window.draw(cx).clear());
    let handle = cx.update_window_entity(&view, |view, _, _| view.handle);
    let _subscription = cx.update(|window, _| {
        let lifecycle = lifecycle.clone();
        window.intercept_window_mouse_events(move |event, window, cx| match event {
            WindowMouseEvent::Down(_) => {
                window
                    .capture_pointer(&handle, MouseButton::Left)
                    .expect("the input callback should establish pointer capture");
                cx.active_drag = Some(AnyDrag {
                    window_id: window.window_handle().window_id(),
                    source: None,
                    value: Arc::new("drag"),
                    view: cx.new(|_| Empty).into(),
                    window_preview_offset: point(px(0.0), px(0.0)),
                    cursor_style: None,
                    button: MouseButton::Left,
                });
                lifecycle.borrow_mut().push("input");
                window.remove_window(cx);
                window.remove_window(cx);
                lifecycle.borrow_mut().push("input-returned");
                cx.stop_propagation();
            }
            WindowMouseEvent::Cancel(event) => {
                assert_eq!(event.reason, PointerCancelReason::WindowClosed);
                assert!(!window.removed, "cancellation must precede window removal");
                lifecycle.borrow_mut().push("cancel");
                window.remove_window(cx);
                window.remove_window(cx);
                lifecycle.borrow_mut().push("cancel-returned");
            }
            _ => {}
        })
    });

    cx.update(|window, cx| window.dispatch_event(mouse_down(MouseButton::Left, 10.0, 10.0), cx));

    assert_eq!(
        lifecycle.borrow().as_slice(),
        &[
            "input",
            "input-returned",
            "cancel",
            "cancel-returned",
            "closed",
        ]
    );
    assert_eq!(close_count.get(), 1);
    assert!(cx.windows().is_empty());
}

#[open_gpui::test]
fn remove_window_from_action_callback_cancels_after_dispatch_before_removal(
    cx: &mut TestAppContext,
) {
    cx.update(|cx| {
        cx.bind_keys([KeyBinding::new("escape", RemoveWindowWithPointer, None)]);
    });
    let lifecycle = Rc::new(RefCell::new(Vec::new()));
    let (view, cx) = cx.add_window_view({
        let lifecycle = lifecycle.clone();
        move |window, cx| ActionWindowRemovalProbe {
            handle: window.new_pointer_capture_handle(),
            focus: cx.focus_handle(),
            lifecycle,
        }
    });
    cx.update(|window, _| window.activate_window());
    cx.run_until_parked();
    cx.update_window_entity(&view, |_, _, cx| cx.notify());
    cx.update(|window, cx| window.draw(cx).clear());
    cx.update_window_entity(&view, |view, window, cx| view.focus.focus(window, cx));
    let _subscription = cx.update(|window, _| {
        let lifecycle = lifecycle.clone();
        window.intercept_window_mouse_events(move |event, window, _| {
            if let WindowMouseEvent::Cancel(event) = event {
                assert_eq!(event.reason, PointerCancelReason::WindowClosed);
                assert!(!window.removed, "cancellation must precede window removal");
                lifecycle.borrow_mut().push("cancel");
            }
        })
    });

    cx.update(|window, cx| {
        window.dispatch_event(mouse_down(MouseButton::Left, 10.0, 10.0), cx);
        assert!(window.has_active_pointer_session(cx));
        window.dispatch_event(
            PlatformInput::KeyDown(KeyDownEvent {
                keystroke: Keystroke::parse("escape").expect("escape should parse"),
                is_held: false,
                prefer_character_input: false,
            }),
            cx,
        );
    });

    assert_eq!(
        lifecycle.borrow().as_slice(),
        &["action", "action-returned", "cancel"]
    );
    assert!(cx.windows().is_empty());
}

#[open_gpui::test]
fn simulated_marked_text_preserves_the_platform_input_handler_without_redraw(
    cx: &mut TestAppContext,
) {
    let lifecycle = Rc::new(RefCell::new(Vec::new()));
    let (view, mut cx) = cx.add_window_view({
        let lifecycle = lifecycle.clone();
        move |window, cx| TextInputWindowRemovalProbe {
            handle: window.new_pointer_capture_handle(),
            focus: cx.focus_handle(),
            lifecycle,
            remove_on_key: false,
        }
    });
    cx.update(|window, _| window.activate_window());
    cx.run_until_parked();
    cx.update_window_entity(&view, |_, _, cx| cx.notify());
    cx.update(|window, cx| window.draw(cx).clear());
    cx.update_window_entity(&view, |view, window, cx| {
        view.focus.focus(window, cx);
        cx.notify();
    });
    cx.update(|window, cx| window.draw(cx).clear());

    cx.simulate_marked_text(None, "first", None);
    cx.simulate_marked_text(None, "second", None);

    assert_eq!(
        lifecycle.borrow().as_slice(),
        &["marked-input", "marked-input"]
    );
}

#[open_gpui::test]
fn platform_input_handler_root_barrier_drains_queued_window_command(cx: &mut TestAppContext) {
    let lifecycle = Rc::new(RefCell::new(Vec::new()));
    let (view, mut cx) = cx.add_window_view({
        let lifecycle = lifecycle.clone();
        move |_, cx| MarkedTextWindowRemovalProbe {
            focus: cx.focus_handle(),
            lifecycle,
            remove_on_marked_text: false,
            activate_on_marked_text: true,
        }
    });
    cx.update_window_entity(&view, |_, _, cx| cx.notify());
    cx.update(|window, cx| window.draw(cx).clear());
    cx.update_window_entity(&view, |view, window, cx| {
        view.focus.focus(window, cx);
        cx.notify();
    });
    cx.update(|window, cx| window.draw(cx).clear());
    let window_id = cx.window_handle().window_id();
    let diagnostic_cursor = cx.update(|_, app| {
        app.native_boundary_diagnostics(NativeBoundaryDiagnosticCursor::default())
            .cursor
    });
    let input_handler_slot = cx
        .update(|window, _| window.platform_window.input_handler_slot_for_test())
        .expect("test platform should expose its input-handler slot");

    input_handler_slot
        .with_handler(|input_handler| {
            input_handler.replace_and_mark_text_in_range(None, "activate", None)
        })
        .expect("focused probe should install a platform input handler");

    assert_eq!(lifecycle.borrow().as_slice(), &["marked-input"]);
    let diagnostic_delta = cx.update(|_, app| app.native_boundary_diagnostics(diagnostic_cursor));
    let activation = diagnostic_delta
        .terminal
        .iter()
        .find(|diagnostic| {
            diagnostic.target == NativeBoundaryTarget::Window(window_id)
                && diagnostic.kind
                    == NativeBoundaryKind::Command(NativePlatformCommandKind::Activate)
        })
        .expect("the root input-handler barrier must deliver its queued activate command");
    assert_eq!(
        activation.disposition,
        NativeBoundaryDisposition::Delivered { input_result: None }
    );
}

#[open_gpui::test]
fn retired_platform_input_handler_slot_records_one_typed_terminal_diagnostic(
    cx: &mut TestAppContext,
) {
    let lifecycle = Rc::new(RefCell::new(Vec::new()));
    let (view, mut cx) = cx.add_window_view({
        let lifecycle = lifecycle.clone();
        move |_, cx| MarkedTextWindowRemovalProbe {
            focus: cx.focus_handle(),
            lifecycle,
            remove_on_marked_text: false,
            activate_on_marked_text: false,
        }
    });
    cx.update_window_entity(&view, |_, _, cx| cx.notify());
    cx.update(|window, cx| window.draw(cx).clear());
    cx.update_window_entity(&view, |view, window, cx| {
        view.focus.focus(window, cx);
        cx.notify();
    });
    cx.update(|window, cx| window.draw(cx).clear());
    let window_id = cx.window_handle().window_id();
    let input_handler_slot = cx
        .update(|window, _| window.platform_window.input_handler_slot_for_test())
        .expect("test platform should expose its input-handler slot");
    let diagnostic_cursor = cx
        .app
        .native_boundary_diagnostics(NativeBoundaryDiagnosticCursor::default())
        .cursor;

    cx.update(|window, app| window.remove_window(app));
    assert!(cx.windows().is_empty());
    let panic = catch_unwind(AssertUnwindSafe(|| input_handler_slot.with_handler(|_| ())))
        .expect_err("entering a retired platform input-handler slot must panic");
    let violation = panic
        .downcast_ref::<crate::NativeInputInvariantViolation>()
        .expect("retired slot panic must preserve the typed invariant violation");
    assert_eq!(violation.window_id, window_id);
    assert_eq!(violation.boundary, crate::NativeInputBoundary::InputHandler);
    assert_eq!(
        violation.failure,
        crate::NativeInvariantFailure::RetiredSlot
    );

    let diagnostic_delta = cx.app.native_boundary_diagnostics(diagnostic_cursor);
    let retired_diagnostics = diagnostic_delta
        .terminal
        .iter()
        .filter(|diagnostic| {
            diagnostic.target == NativeBoundaryTarget::Window(window_id)
                && diagnostic.kind
                    == NativeBoundaryKind::Callback(
                        crate::NativeCallbackKind::PlatformInputHandlerSlot,
                    )
                && diagnostic.disposition
                    == NativeBoundaryDisposition::InvariantFailure(
                        crate::NativeInvariantFailure::RetiredSlot,
                    )
        })
        .collect::<Vec<_>>();
    assert_eq!(
        retired_diagnostics.len(),
        1,
        "one retired input-handler slot entry must publish exactly one terminal diagnostic"
    );
    assert!(matches!(
        retired_diagnostics[0].domain_generation,
        Some(crate::NativeBoundaryGeneration::InputSlot {
            boundary: crate::NativeInputBoundary::InputHandler,
            generation,
        }) if Some(generation) == violation.slot_generation
    ));
}

#[open_gpui::test]
fn simulated_marked_text_preserves_handler_installed_during_callback(cx: &mut TestAppContext) {
    let lifecycle = Rc::new(RefCell::new(Vec::new()));
    let (view, mut cx) = cx.add_window_view({
        let lifecycle = lifecycle.clone();
        move |_, cx| SwitchingMarkedTextProbe {
            first_focus: cx.focus_handle(),
            second_focus: cx.focus_handle(),
            lifecycle,
        }
    });
    cx.update(|window, _| window.activate_window());
    cx.run_until_parked();
    cx.update_window_entity(&view, |_, _, cx| cx.notify());
    cx.update(|window, cx| window.draw(cx).clear());
    cx.update_window_entity(&view, |view, window, cx| {
        view.first_focus.focus(window, cx);
        cx.notify();
    });
    cx.update(|window, cx| window.draw(cx).clear());

    cx.simulate_marked_text(None, "first", None);
    cx.simulate_marked_text(None, "second", None);

    assert_eq!(lifecycle.borrow().as_slice(), &["first", "second"]);
}

#[open_gpui::test]
fn simulated_marked_text_returns_after_callback_removes_window(cx: &mut TestAppContext) {
    let lifecycle = Rc::new(RefCell::new(Vec::new()));
    let close_count = Rc::new(Cell::new(0));
    let _close_subscription = record_window_close(cx, lifecycle.clone(), close_count.clone());
    let (view, mut cx) = cx.add_window_view({
        let lifecycle = lifecycle.clone();
        move |_, cx| MarkedTextWindowRemovalProbe {
            focus: cx.focus_handle(),
            lifecycle,
            remove_on_marked_text: true,
            activate_on_marked_text: false,
        }
    });
    cx.update(|window, _| window.activate_window());
    cx.run_until_parked();
    cx.update_window_entity(&view, |_, _, cx| cx.notify());
    cx.update(|window, cx| window.draw(cx).clear());
    cx.update_window_entity(&view, |view, window, cx| {
        view.focus.focus(window, cx);
        cx.notify();
    });
    cx.update(|window, cx| window.draw(cx).clear());

    cx.simulate_marked_text(None, "closing composition", None);

    assert_eq!(
        lifecycle.borrow().as_slice(),
        &["marked-input", "marked-input-returned", "closed"]
    );
    assert_eq!(close_count.get(), 1);
    assert!(cx.windows().is_empty());
}

#[open_gpui::test]
fn key_listener_close_skips_synthetic_text_input(cx: &mut TestAppContext) {
    let lifecycle = Rc::new(RefCell::new(Vec::new()));
    let close_count = Rc::new(Cell::new(0));
    let _close_subscription = record_window_close(cx, lifecycle.clone(), close_count.clone());
    let (view, cx) = cx.add_window_view({
        let lifecycle = lifecycle.clone();
        move |window, cx| TextInputWindowRemovalProbe {
            handle: window.new_pointer_capture_handle(),
            focus: cx.focus_handle(),
            lifecycle,
            remove_on_key: true,
        }
    });
    cx.update(|window, _| window.activate_window());
    cx.run_until_parked();
    cx.update_window_entity(&view, |_, _, cx| cx.notify());
    cx.update(|window, cx| window.draw(cx).clear());
    cx.update_window_entity(&view, |view, window, cx| {
        view.focus.focus(window, cx);
        cx.notify();
    });
    cx.update(|window, cx| window.draw(cx).clear());
    let _subscription = cx.update(|window, _| {
        let lifecycle = lifecycle.clone();
        window.intercept_window_mouse_events(move |event, window, cx| {
            if let WindowMouseEvent::Cancel(event) = event {
                assert_eq!(event.reason, PointerCancelReason::WindowClosed);
                assert!(!window.removed, "cancellation must precede window removal");
                lifecycle.borrow_mut().push("cancel");
                window.remove_window(cx);
                window.remove_window(cx);
                lifecycle.borrow_mut().push("cancel-returned");
            }
        })
    });

    cx.update(|window, cx| {
        window.dispatch_event(mouse_down(MouseButton::Left, 10.0, 10.0), cx);
        assert!(window.has_active_pointer_session(cx));
        assert!(window.dispatch_keystroke(Keystroke::parse("a").expect("a should parse"), cx));
    });

    assert_eq!(
        lifecycle.borrow().as_slice(),
        &["key", "key-returned", "cancel", "cancel-returned", "closed"]
    );
    assert_eq!(close_count.get(), 1);
    assert!(cx.windows().is_empty());
}

#[open_gpui::test]
fn synthetic_text_handler_close_waits_for_handler_return(cx: &mut TestAppContext) {
    let lifecycle = Rc::new(RefCell::new(Vec::new()));
    let close_count = Rc::new(Cell::new(0));
    let _close_subscription = record_window_close(cx, lifecycle.clone(), close_count.clone());
    let (view, cx) = cx.add_window_view({
        let lifecycle = lifecycle.clone();
        move |window, cx| TextInputWindowRemovalProbe {
            handle: window.new_pointer_capture_handle(),
            focus: cx.focus_handle(),
            lifecycle,
            remove_on_key: false,
        }
    });
    cx.update(|window, _| window.activate_window());
    cx.run_until_parked();
    cx.update_window_entity(&view, |_, _, cx| cx.notify());
    cx.update(|window, cx| window.draw(cx).clear());
    cx.update_window_entity(&view, |view, window, cx| {
        view.focus.focus(window, cx);
        cx.notify();
    });
    cx.update(|window, cx| window.draw(cx).clear());
    let _subscription = cx.update(|window, _| {
        let lifecycle = lifecycle.clone();
        window.intercept_window_mouse_events(move |event, window, cx| {
            if let WindowMouseEvent::Cancel(event) = event {
                assert_eq!(event.reason, PointerCancelReason::WindowClosed);
                assert!(!window.removed, "cancellation must precede window removal");
                lifecycle.borrow_mut().push("cancel");
                window.remove_window(cx);
                window.remove_window(cx);
                lifecycle.borrow_mut().push("cancel-returned");
            }
        })
    });

    cx.update(|window, cx| {
        window.dispatch_event(mouse_down(MouseButton::Left, 10.0, 10.0), cx);
        assert!(window.has_active_pointer_session(cx));
        assert!(window.dispatch_keystroke(Keystroke::parse("a").expect("a should parse"), cx));
    });

    assert_eq!(
        lifecycle.borrow().as_slice(),
        &[
            "key",
            "key-returned",
            "input",
            "input-returned",
            "cancel",
            "cancel-returned",
            "closed",
        ]
    );
    assert_eq!(close_count.get(), 1);
    assert!(cx.windows().is_empty());
}

#[open_gpui::test]
fn platform_ime_insert_text_close_waits_for_callback_and_notifies_once(cx: &mut TestAppContext) {
    let lifecycle = Rc::new(RefCell::new(Vec::new()));
    let close_count = Rc::new(Cell::new(0));
    let _close_subscription = record_window_close(cx, lifecycle.clone(), close_count.clone());
    let (view, cx) = cx.add_window_view({
        let lifecycle = lifecycle.clone();
        move |window, cx| TextInputWindowRemovalProbe {
            handle: window.new_pointer_capture_handle(),
            focus: cx.focus_handle(),
            lifecycle,
            remove_on_key: false,
        }
    });
    cx.update(|window, _| window.activate_window());
    cx.run_until_parked();
    cx.update_window_entity(&view, |_, _, cx| cx.notify());
    cx.update(|window, cx| window.draw(cx).clear());
    cx.update_window_entity(&view, |view, window, cx| {
        view.focus.focus(window, cx);
        cx.notify();
    });
    cx.update(|window, cx| window.draw(cx).clear());
    let _subscription = cx.update(|window, _| {
        let lifecycle = lifecycle.clone();
        window.intercept_window_mouse_events(move |event, window, cx| {
            if let WindowMouseEvent::Cancel(event) = event {
                assert_eq!(event.reason, PointerCancelReason::WindowClosed);
                assert!(!window.removed, "cancellation must precede window removal");
                lifecycle.borrow_mut().push("cancel");
                window.remove_window(cx);
                window.remove_window(cx);
                lifecycle.borrow_mut().push("cancel-returned");
            }
        })
    });
    cx.update(|window, cx| {
        window.dispatch_event(mouse_down(MouseButton::Left, 10.0, 10.0), cx);
        assert!(window.has_active_pointer_session(cx));
    });
    let input_handler_slot = cx
        .update(|window, _| window.platform_window.input_handler_slot_for_test())
        .expect("test platform should expose its input-handler slot");
    input_handler_slot
        .with_handler(|input_handler| input_handler.replace_text_in_range(None, "ime"))
        .expect("focused probe should install a platform input handler");

    assert_eq!(
        lifecycle.borrow().as_slice(),
        &[
            "input",
            "input-returned",
            "cancel",
            "cancel-returned",
            "closed",
        ]
    );
    assert_eq!(close_count.get(), 1);
    assert!(cx.windows().is_empty());
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
            source: None,
            value: Arc::new("second-window-drag"),
            view: app.new(|_| Empty).into(),
            window_preview_offset: point(px(0.0), px(0.0)),
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
fn cached_listener_replay_and_capture_revocation_share_one_complete_cancel_dispatch(
    cx: &mut TestAppContext,
) {
    let renders = Rc::new(Cell::new(0));
    let events = Rc::new(RefCell::new(Vec::new()));
    let (view, cx) = cx.add_window_view({
        let renders = renders.clone();
        let events = events.clone();
        move |window, cx| MixedCachedPointerCancelRoot {
            presentation: SubtreePresentation::Visible,
            capture: window.new_pointer_capture_handle(),
            cached_child: cx.new(|_| PointerCancelJournalProbe {
                renders,
                events: events.clone(),
            }),
            events,
        }
    });
    cx.update(|window, cx| window.draw(cx).clear());
    let capture = cx.update_window_entity(&view, |view, _, _| view.capture);
    cx.update(|window, _| window.activate_window());
    cx.run_until_parked();
    cx.update(|window, cx| {
        window.draw(cx).clear();
        window.dispatch_event(mouse_down(MouseButton::Left, 10.0, 10.0), cx);
        window
            .capture_pointer(&capture, MouseButton::Left)
            .expect("visible capture owner should be bound");
    });
    let cached_renders = renders.get();

    cx.update_window_entity(&view, |view, _, cx| {
        view.presentation = SubtreePresentation::Inert;
        cx.notify();
    });
    cx.update(|window, cx| window.draw(cx).clear());
    cx.run_until_parked();

    assert_eq!(
        renders.get(),
        cached_renders,
        "the unrelated listener subtree must replay its cached paint journal"
    );
    assert!(cx.update(|window, _| window.captured_pointer().is_none()));
    let events = events.borrow();
    for label in ["first", "second", "owner"] {
        assert_eq!(
            events
                .iter()
                .filter(|(candidate, _)| *candidate == label)
                .map(|(_, phase)| *phase)
                .collect::<Vec<_>>(),
            [DispatchPhase::Capture, DispatchPhase::Bubble],
            "{label} must receive both terminal cancellation phases exactly once"
        );
    }
}

#[open_gpui::test]
fn stale_drag_revocation_before_root_paint_preserves_cached_cancel_journal(
    cx: &mut TestAppContext,
) {
    let renders = Rc::new(Cell::new(0));
    let events = Rc::new(RefCell::new(Vec::new()));
    let (view, cx) = cx.add_window_view({
        let renders = renders.clone();
        let events = events.clone();
        move |window, cx| MixedCachedPointerCancelRoot {
            presentation: SubtreePresentation::Visible,
            capture: window.new_pointer_capture_handle(),
            cached_child: cx.new(|_| PointerCancelJournalProbe {
                renders,
                events: events.clone(),
            }),
            events,
        }
    });
    cx.update(|window, cx| window.draw(cx).clear());
    let capture = cx.update_window_entity(&view, |view, _, _| view.capture);
    let cached_renders = renders.get();

    cx.update(|window, cx| {
        cx.active_drag = Some(AnyDrag {
            window_id: window.window_handle().window_id(),
            source: Some(capture),
            value: Arc::new("drag"),
            view: cx.new(|_| Empty).into(),
            window_preview_offset: point(px(0.0), px(0.0)),
            cursor_style: None,
            button: MouseButton::Left,
        });
    });
    cx.update_window_entity(&view, |view, _, cx| {
        view.presentation = SubtreePresentation::Inert;
        cx.notify();
    });

    cx.update(|window, cx| window.draw(cx).clear());
    cx.run_until_parked();

    assert_eq!(
        renders.get(),
        cached_renders,
        "the listener subtree should retain a valid cached paint journal"
    );
    assert!(cx.read(|cx| cx.active_drag.is_none()));
    let events = events.borrow();
    for label in ["first", "second", "owner"] {
        assert_eq!(
            events
                .iter()
                .filter(|(candidate, _)| *candidate == label)
                .map(|(_, phase)| *phase)
                .collect::<Vec<_>>(),
            [DispatchPhase::Capture, DispatchPhase::Bubble],
            "{label} must receive the stale drag's terminal cancellation exactly once"
        );
    }
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
        window.intercept_window_mouse_events(|event, window, cx| {
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

#[open_gpui::test]
fn active_drag_preview_and_pointer_events_are_isolated_to_its_window(cx: &mut TestAppContext) {
    let first: crate::AnyWindowHandle = cx
        .open_window(size(px(320.0), px(200.0)), |_, _| Empty)
        .into();
    let second: crate::AnyWindowHandle = cx
        .open_window(size(px(320.0), px(200.0)), |_, _| Empty)
        .into();
    let preview_renders = Rc::new(Cell::new(0));

    cx.update_window(first, |_, window, cx| window.draw(cx).clear())
        .expect("the drag-owning window should remain open");
    cx.update_window(second, |_, window, cx| window.draw(cx).clear())
        .expect("the unrelated window should remain open");
    cx.update({
        let preview_renders = preview_renders.clone();
        move |app| {
            app.active_drag = Some(AnyDrag {
                window_id: first.window_id(),
                source: None,
                value: Arc::new("first-window-drag"),
                view: app
                    .new(move |_| DragPreviewProbe {
                        renders: preview_renders,
                    })
                    .into(),
                window_preview_offset: point(px(0.0), px(0.0)),
                cursor_style: None,
                button: MouseButton::Left,
            });
        }
    });

    cx.update_window(second, |_, window, cx| {
        window.refresh();
        window.draw(cx).clear();
    })
    .expect("the unrelated window should be drawable");
    assert_eq!(
        preview_renders.get(),
        0,
        "an unrelated window must not render another window's drag preview"
    );

    cx.update_window(first, |_, window, cx| {
        window.refresh();
        window.draw(cx).clear();
    })
    .expect("the drag-owning window should be drawable");
    assert_eq!(
        preview_renders.get(),
        1,
        "the drag-owning window should render its own preview"
    );

    cx.update_window(second, |_, window, cx| {
        assert!(!window.invalidator.is_dirty());
        window.dispatch_event(
            PlatformInput::MouseMove(MouseMoveEvent {
                position: point(px(10.0), px(10.0)),
                pressed_button: Some(MouseButton::Left),
                modifiers: Modifiers::none(),
            }),
            cx,
        );
        assert!(
            !window.invalidator.is_dirty(),
            "a drag from another window must not refresh this window on mouse move"
        );
        window.dispatch_event(mouse_up(MouseButton::Left, 10.0, 10.0), cx);
    })
    .expect("the unrelated window should accept isolated input");
    cx.update(|app| {
        assert_eq!(
            app.active_drag.as_ref().map(|drag| drag.window_id),
            Some(first.window_id()),
            "a mouse up from another window must not terminate the owner drag"
        );
    });

    cx.update_window(first, |_, window, cx| {
        window.dispatch_event(mouse_up(MouseButton::Left, 10.0, 10.0), cx);
    })
    .expect("the drag-owning window should accept the terminating mouse up");
    cx.update(|app| assert!(app.active_drag.is_none()));
}
