use std::{
    cell::{Cell, RefCell},
    ops::Range,
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
    sync::Arc,
    time::Duration,
};

use crate::{
    AnyDrag, AnyView, App, AppContext as _, Bounds, Context, DispatchPhase, Empty, Entity,
    EventEmitter, FocusHandle, Focusable, HitboxBehavior, InputHandler, InteractiveElement,
    InteractiveText, IntoElement, KeyBinding, KeyDownEvent, Keystroke, Modifiers, MouseButton,
    MouseDownEvent, MouseExitEvent, MouseMoveEvent, MouseUpEvent, NativeBoundaryDiagnosticCursor,
    NativeBoundaryDisposition, NativeBoundaryGeneration, NativeBoundaryKind, NativeBoundaryTarget,
    NativeCallbackKind, NativeCapturedDragGeneration, NativeCapturedDragPhase,
    NativeCapturedDragReleaseBarrier, NativeCapturedDragReleaseTerminal, NativePlatformCommandKind,
    ParentElement, Pixels, PlatformInput, PlatformPointerCaptureReleaseOutcome, PlatformWindow,
    PlatformWindowCommand, Point, PointerCancelEvent, PointerCancelReason, PointerCaptureError,
    PointerCaptureHandle, PromptLevel, PromptResponse, QuitMode, Render, RequestFrameOptions,
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

struct NativeCapturedDragMouseUpProbe;

struct NativeCapturedDragConsumerPanicProbe {
    handle: PointerCaptureHandle,
    cancellations: Rc<Cell<usize>>,
    panic_on_cancel: Rc<Cell<bool>>,
}

struct NativeCapturedDragStartReentryProbe {
    platform_window: Rc<RefCell<Option<crate::TestWindow>>>,
    reserved_generation: Rc<Cell<Option<NativeCapturedDragGeneration>>>,
    prepared_consumer: Rc<RefCell<Option<crate::PreparedNativeCapturedDragConsumer>>>,
}

#[derive(Clone, Copy)]
enum NativeCapturedDragStartInterruption {
    None,
    CancelPointerSession,
    RemoveWindow,
    Panic,
}

struct NativeCapturedDragStartInvalidationProbe {
    interruption: Rc<Cell<NativeCapturedDragStartInterruption>>,
    reserved_generations: Rc<RefCell<Vec<NativeCapturedDragGeneration>>>,
    prepared_consumer: Rc<RefCell<Option<crate::PreparedNativeCapturedDragConsumer>>>,
}

struct NativeCapturedDragStartInvalidationFixture {
    source: crate::AnyWindowHandle,
    platform_window: crate::TestWindow,
    interruption: Rc<Cell<NativeCapturedDragStartInterruption>>,
    reserved_generations: Rc<RefCell<Vec<NativeCapturedDragGeneration>>>,
    prepared_consumer: Rc<RefCell<Option<crate::PreparedNativeCapturedDragConsumer>>>,
    deliveries: Rc<Cell<usize>>,
    _subscription: crate::Subscription,
}

struct NativeWindowUpdateProvenanceProbe {
    target: crate::AnyWindowHandle,
    observations: Rc<RefCell<Vec<bool>>>,
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

struct ShutdownPointerCancelProbe {
    handle: PointerCaptureHandle,
    cancellations: Rc<Cell<usize>>,
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

impl Render for ShutdownPointerCancelProbe {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let handle = self.handle;
        let cancellations = self.cancellations.clone();
        canvas(
            move |bounds, window, _| {
                let hitbox = window.insert_hitbox(bounds, HitboxBehavior::Normal);
                window
                    .bind_pointer_capture(&handle, hitbox.id)
                    .expect("the shutdown probe should bind its capture owner");
                hitbox
            },
            move |_, hitbox, window, _| {
                let target = hitbox.id;
                window.on_mouse_event(move |event: &MouseDownEvent, phase, window, _| {
                    if phase == DispatchPhase::Bubble
                        && event.button == MouseButton::Left
                        && target.is_mouse_event_target(window)
                    {
                        window
                            .capture_pointer(&handle, MouseButton::Left)
                            .expect("the shutdown probe should capture its pointer");
                    }
                });
                window.on_pointer_cancel(move |_, phase, _, _| {
                    if phase == DispatchPhase::Bubble {
                        cancellations.set(cancellations.get() + 1);
                    }
                });
            },
        )
        .size_full()
    }
}

impl Render for DragPreviewProbe {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        self.renders.set(self.renders.get() + 1);
        div().w(px(1.0)).h(px(1.0))
    }
}

impl Render for NativeCapturedDragMouseUpProbe {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .on_mouse_up(MouseButton::Left, |_, window, cx| {
                assert!(
                    cx.stop_active_drag(window),
                    "the user listener should be able to clear the drag before native delivery"
                );
            })
    }
}

impl Render for NativeCapturedDragConsumerPanicProbe {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let handle = self.handle;
        let cancellations = self.cancellations.clone();
        let panic_on_cancel = self.panic_on_cancel.clone();
        canvas(
            move |bounds, window, _| {
                let hitbox = window.insert_hitbox(bounds, HitboxBehavior::Normal);
                window
                    .bind_pointer_capture(&handle, hitbox.id)
                    .expect("the panic-recovery capture owner should bind");
                hitbox
            },
            move |_, hitbox, window, _| {
                let target = hitbox.id;
                window.on_mouse_event(move |event: &MouseDownEvent, phase, window, _| {
                    if phase == DispatchPhase::Bubble
                        && event.button == MouseButton::Left
                        && target.is_mouse_event_target(window)
                    {
                        window
                            .capture_pointer(&handle, MouseButton::Left)
                            .expect("the panic-recovery gesture should capture its pointer");
                    }
                });
                window.on_pointer_cancel(move |_, phase, _, _| {
                    if phase == DispatchPhase::Bubble {
                        cancellations.set(cancellations.get() + 1);
                        if panic_on_cancel.replace(false) {
                            panic!("injected pointer-cancel listener panic");
                        }
                    }
                });
            },
        )
        .size_full()
    }
}

impl Render for NativeCapturedDragStartReentryProbe {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let platform_window = self.platform_window.clone();
        let reserved_generation = self.reserved_generation.clone();
        let prepared_consumer = self.prepared_consumer.clone();
        div()
            .id("native-captured-drag-start-reentry-probe")
            .size_full()
            .on_drag("native-captured-drag-reentry", move |_, geometry, _, cx| {
                reserved_generation.set(Some(geometry.native_captured_drag_generation()));
                let consumer = geometry.prepare_native_captured_drag_consumer();
                assert!(!consumer.is_active());
                assert!(!consumer.is_revoked());
                prepared_consumer.borrow_mut().replace(consumer);
                platform_window
                    .borrow()
                    .as_ref()
                    .expect("the test platform window must be installed before dragging")
                    .simulate_active_status_change(false);
                assert_eq!(
                    prepared_consumer
                        .borrow()
                        .as_ref()
                        .map(crate::PreparedNativeCapturedDragConsumer::is_active),
                    Some(false),
                    "the reentrant platform fact must wait for the atomic start commit"
                );
                cx.new(|_| Empty)
            })
    }
}

impl Render for NativeCapturedDragStartInvalidationProbe {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let interruption = self.interruption.clone();
        let reserved_generations = self.reserved_generations.clone();
        let prepared_consumer = self.prepared_consumer.clone();
        div()
            .id("native-captured-drag-start-invalidation-probe")
            .size_full()
            .on_drag(
                "native-captured-drag-start-invalidation",
                move |_, geometry, window, cx| {
                    reserved_generations
                        .borrow_mut()
                        .push(geometry.native_captured_drag_generation());
                    if prepared_consumer.borrow().is_none()
                        && !matches!(
                            interruption.get(),
                            NativeCapturedDragStartInterruption::None
                        )
                    {
                        let consumer = geometry.prepare_native_captured_drag_consumer();
                        assert!(!consumer.is_active());
                        assert!(!consumer.is_revoked());
                        prepared_consumer.borrow_mut().replace(consumer);
                    }

                    match interruption.get() {
                        NativeCapturedDragStartInterruption::None => {}
                        NativeCapturedDragStartInterruption::CancelPointerSession => {
                            window.cancel_pointer_session(PointerCancelReason::CaptureRevoked, cx)
                        }
                        NativeCapturedDragStartInterruption::RemoveWindow => {
                            window.remove_window(cx)
                        }
                        NativeCapturedDragStartInterruption::Panic => {
                            panic!("injected drag listener panic after consumer preparation")
                        }
                    }
                    cx.new(|_| Empty)
                },
            )
    }
}

impl Render for NativeWindowUpdateProvenanceProbe {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let target = self.target;
        let observations = self.observations.clone();
        div().size_full().on_mouse_move(move |_, _, cx| {
            observations
                .borrow_mut()
                .push(cx.has_native_window_update_provenance());
            cx.update_window_id(target.window_id(), |_, _, cx| {
                observations
                    .borrow_mut()
                    .push(cx.has_native_window_update_provenance());
            })
            .expect("the nested target window should remain available");
            observations
                .borrow_mut()
                .push(cx.has_native_window_update_provenance());
        })
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
        let drag_view = cx.new(|_| Empty).into();
        cx.start_active_drag(AnyDrag {
            window_id: window.window_handle().window_id(),
            source: None,
            value: Arc::new("drag"),
            view: drag_view,
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

fn open_native_captured_drag_start_invalidation_fixture(
    cx: &mut TestAppContext,
    interruption: NativeCapturedDragStartInterruption,
) -> NativeCapturedDragStartInvalidationFixture {
    let interruption = Rc::new(Cell::new(interruption));
    let reserved_generations = Rc::new(RefCell::new(Vec::new()));
    let prepared_consumer = Rc::new(RefCell::new(None));
    let source: crate::AnyWindowHandle = cx
        .open_window(size(px(320.0), px(200.0)), {
            let interruption = interruption.clone();
            let reserved_generations = reserved_generations.clone();
            let prepared_consumer = prepared_consumer.clone();
            move |_, _| NativeCapturedDragStartInvalidationProbe {
                interruption,
                reserved_generations,
                prepared_consumer,
            }
        })
        .into();
    let platform_window = cx.test_window(source);
    let deliveries = Rc::new(Cell::new(0_usize));
    let subscription = cx.update({
        let deliveries = deliveries.clone();
        move |cx| {
            cx.observe_native_captured_drag(move |_, _| {
                deliveries.set(deliveries.get().saturating_add(1));
            })
        }
    });
    cx.update_window(source, |_, window, cx| {
        window.activate_window();
        window.draw(cx).clear();
    })
    .expect("the drag source should remain open for its initial frame");
    cx.run_until_parked();

    NativeCapturedDragStartInvalidationFixture {
        source,
        platform_window,
        interruption,
        reserved_generations,
        prepared_consumer,
        deliveries,
        _subscription: subscription,
    }
}

fn trigger_native_captured_drag_start(fixture: &mut NativeCapturedDragStartInvalidationFixture) {
    let _ =
        fixture
            .platform_window
            .simulate_input_result(mouse_down(MouseButton::Left, 10.0, 10.0));
    let _ = fixture
        .platform_window
        .simulate_input_result(PlatformInput::MouseMove(MouseMoveEvent {
            position: point(px(30.0), px(10.0)),
            pressed_button: Some(MouseButton::Left),
            modifiers: Modifiers::none(),
        }));
}

fn assert_native_captured_drag_start_was_revoked(
    cx: &mut TestAppContext,
    fixture: &NativeCapturedDragStartInvalidationFixture,
) {
    let reserved_generations = fixture.reserved_generations.borrow();
    assert_eq!(reserved_generations.len(), 1);
    {
        let prepared_consumer = fixture.prepared_consumer.borrow();
        let prepared_consumer = prepared_consumer
            .as_ref()
            .expect("the interrupted listener must prepare its native captured-drag consumer");
        assert_eq!(prepared_consumer.generation(), reserved_generations[0]);
        assert!(!prepared_consumer.is_active());
        assert!(prepared_consumer.is_revoked());
    }
    drop(reserved_generations);

    assert!(cx.read(|cx| cx.active_drag.is_none()));
    assert_eq!(
        fixture.deliveries.get(),
        0,
        "a revoked start must not publish captured native pointer facts"
    );
}

fn retry_native_captured_drag_start(
    cx: &mut TestAppContext,
    fixture: &mut NativeCapturedDragStartInvalidationFixture,
) {
    fixture
        .interruption
        .set(NativeCapturedDragStartInterruption::None);
    cx.update_window(fixture.source, |_, window, cx| {
        assert!(
            !window.has_active_pointer_session(cx),
            "an interrupted drag start must settle its pointer session before retry"
        );
        window.draw(cx).clear();
    })
    .expect("the interrupted drag source should remain available for a clean retry");

    trigger_native_captured_drag_start(fixture);

    assert_eq!(
        cx.read(|cx| cx.active_drag.as_ref().map(|drag| drag.window_id)),
        Some(fixture.source.window_id()),
        "the next uninterrupted listener must commit a normal active drag"
    );
    let reserved_generations = fixture.reserved_generations.borrow();
    assert_eq!(reserved_generations.len(), 2);
    assert_ne!(reserved_generations[0], reserved_generations[1]);
    drop(reserved_generations);
    assert_eq!(
        fixture.deliveries.get(),
        0,
        "starting a replacement drag must not replay facts from the revoked generation"
    );

    cx.update_window(fixture.source, |_, window, cx| {
        window.cancel_pointer_session(PointerCancelReason::CaptureRevoked, cx);
    })
    .expect("the replacement drag source should remain open for cleanup");
}

#[open_gpui::test]
fn drag_listener_pointer_cancel_revokes_reserved_native_capture_start_and_allows_next_drag(
    cx: &mut TestAppContext,
) {
    let mut fixture = open_native_captured_drag_start_invalidation_fixture(
        cx,
        NativeCapturedDragStartInterruption::CancelPointerSession,
    );

    trigger_native_captured_drag_start(&mut fixture);
    cx.run_until_parked();

    assert_native_captured_drag_start_was_revoked(cx, &fixture);
    assert!(
        cx.update_window(fixture.source, |_, window, cx| {
            !window.has_active_pointer_session(cx)
        })
        .expect("the cancelled drag source should remain open")
    );
    retry_native_captured_drag_start(cx, &mut fixture);
}

#[open_gpui::test]
fn drag_listener_window_removal_revokes_reserved_native_capture_start_without_delivery(
    cx: &mut TestAppContext,
) {
    let mut fixture = open_native_captured_drag_start_invalidation_fixture(
        cx,
        NativeCapturedDragStartInterruption::RemoveWindow,
    );

    trigger_native_captured_drag_start(&mut fixture);
    cx.run_until_parked();

    assert_native_captured_drag_start_was_revoked(cx, &fixture);
    assert!(
        !cx.windows().contains(&fixture.source),
        "the deferred input-transaction removal must still commit"
    );
}

#[open_gpui::test]
fn panicking_drag_listener_revokes_reserved_native_capture_start_and_allows_next_drag(
    cx: &mut TestAppContext,
) {
    let mut fixture = open_native_captured_drag_start_invalidation_fixture(
        cx,
        NativeCapturedDragStartInterruption::Panic,
    );

    let panic = catch_unwind(AssertUnwindSafe(|| {
        trigger_native_captured_drag_start(&mut fixture);
    }));
    assert!(panic.is_err());
    cx.run_until_parked();

    assert_native_captured_drag_start_was_revoked(cx, &fixture);
    assert!(cx.windows().contains(&fixture.source));
    retry_native_captured_drag_start(cx, &mut fixture);
}

#[open_gpui::test]
fn native_captured_drag_start_activates_prepared_consumer_before_reentrant_cancel(
    cx: &mut TestAppContext,
) {
    let platform_window_slot = Rc::new(RefCell::new(None));
    let reserved_generation = Rc::new(Cell::new(None));
    let prepared_consumer = Rc::new(RefCell::new(None));
    let source: crate::AnyWindowHandle = cx
        .open_window(size(px(320.0), px(200.0)), {
            let platform_window = platform_window_slot.clone();
            let reserved_generation = reserved_generation.clone();
            let prepared_consumer = prepared_consumer.clone();
            move |_, _| NativeCapturedDragStartReentryProbe {
                platform_window,
                reserved_generation,
                prepared_consumer,
            }
        })
        .into();
    let mut platform_window = cx.test_window(source);
    assert_eq!(platform_window.native_pointer_capture_release_count(), 0);
    *platform_window_slot.borrow_mut() = Some(platform_window.clone());
    let deliveries = Rc::new(RefCell::new(Vec::new()));
    let subscription = cx.update({
        let deliveries = deliveries.clone();
        let prepared_consumer = prepared_consumer.clone();
        move |cx| {
            cx.observe_native_captured_drag(move |event, _| {
                let consumer = prepared_consumer.borrow();
                let consumer = consumer
                    .as_ref()
                    .expect("the listener must prepare its consumer before native delivery");
                deliveries.borrow_mut().push((
                    event.phase(),
                    event.generation(),
                    consumer.is_active(),
                    consumer.is_revoked(),
                ));
            })
        }
    });

    cx.update_window(source, |_, window, cx| {
        window.activate_window();
        window.draw(cx).clear();
    })
    .expect("the source window should remain open");
    cx.run_until_parked();
    let _ = platform_window.simulate_input_result(mouse_down(MouseButton::Left, 10.0, 10.0));
    let _ = platform_window.simulate_input_result(PlatformInput::MouseMove(MouseMoveEvent {
        position: point(px(30.0), px(10.0)),
        pressed_button: Some(MouseButton::Left),
        modifiers: Modifiers::none(),
    }));
    cx.run_until_parked();

    let generation = reserved_generation
        .get()
        .expect("the drag listener must observe its reserved generation");
    assert!(
        prepared_consumer
            .borrow()
            .as_ref()
            .is_some_and(crate::PreparedNativeCapturedDragConsumer::is_active)
    );
    assert!(cx.read(|cx| cx.active_drag.is_none()));
    assert_eq!(
        deliveries.borrow().as_slice(),
        [(
            NativeCapturedDragPhase::Cancelled(PointerCancelReason::WindowDeactivated),
            generation,
            true,
            false,
        )]
    );
    let _ = subscription;
}

#[open_gpui::test]
fn exact_native_captured_drag_cancel_is_once_and_cannot_cancel_a_replacement_generation(
    cx: &mut TestAppContext,
) {
    let source_capture = Rc::new(Cell::new(None));
    let pointer_cancellations = Rc::new(Cell::new(0));
    let source: crate::AnyWindowHandle = cx
        .open_window(size(px(320.0), px(200.0)), {
            let source_capture = source_capture.clone();
            let pointer_cancellations = pointer_cancellations.clone();
            move |window, _| {
                let handle = window.new_pointer_capture_handle();
                source_capture.set(Some(handle));
                NativeCapturedDragConsumerPanicProbe {
                    handle,
                    cancellations: pointer_cancellations,
                    panic_on_cancel: Rc::new(Cell::new(false)),
                }
            }
        })
        .into();
    let moved_generation = Rc::new(Cell::new(None));
    let deliveries = Rc::new(RefCell::new(Vec::new()));
    let subscription = cx.update({
        let moved_generation = moved_generation.clone();
        let deliveries = deliveries.clone();
        move |cx| {
            cx.observe_native_captured_drag(move |event, _| {
                if event.phase() == NativeCapturedDragPhase::Moved {
                    moved_generation.set(Some(event.generation()));
                }
                deliveries.borrow_mut().push((
                    event.phase(),
                    event.generation(),
                    event.payload::<&'static str>().copied(),
                ));
            })
        }
    });

    cx.update_window(source, |_, window, cx| {
        window.activate_window();
        window.draw(cx).clear();
    })
    .expect("the exact-cancel source should remain open");
    cx.run_until_parked();

    let mut platform_window = cx.test_window(source);
    let _ = platform_window.simulate_input_result(mouse_down(MouseButton::Left, 10.0, 10.0));
    let capture = source_capture
        .get()
        .expect("the exact-cancel source should expose its capture handle");
    cx.update_window(source, |_, _, cx| {
        let drag_view = cx.new(|_| Empty).into();
        cx.start_active_drag(AnyDrag {
            window_id: source.window_id(),
            source: Some(capture),
            value: Arc::new("exact-g1"),
            view: drag_view,
            window_preview_offset: point(px(0.0), px(0.0)),
            cursor_style: None,
            button: MouseButton::Left,
        });
    })
    .expect("the captured source should start G1");
    let _ = platform_window.simulate_input_result(PlatformInput::MouseMove(MouseMoveEvent {
        position: point(px(30.0), px(20.0)),
        pressed_button: Some(MouseButton::Left),
        modifiers: Modifiers::none(),
    }));
    cx.run_until_parked();
    let g1 = moved_generation
        .get()
        .expect("the first captured move should expose G1");

    let source_window = source.window_id();
    let source_slot = source_window.as_u64() & u64::from(u32::MAX);
    let source_generation = source_window.as_u64() >> 32;
    let aliased_generation = if source_generation == 7 { 9 } else { 7 };
    let aliased_source = crate::WindowId::from((aliased_generation << 32) | source_slot);
    assert_ne!(aliased_source, source_window);
    assert!(!cx.update(|app| app.cancel_native_captured_drag(
        aliased_source,
        g1,
        PointerCancelReason::CaptureRevoked,
    )));
    assert!(cx.read(|cx| cx.active_drag.is_some()));
    assert!(
        cx.update_window(source, |_, window, _| window.captured_pointer().is_some())
            .expect("the real source window should remain open")
    );

    let diagnostic_cursor = cx.update(|app| {
        app.native_boundary_diagnostics(NativeBoundaryDiagnosticCursor::default())
            .cursor
    });
    assert!(
        cx.update_window(source, |_, _, app| app.cancel_native_captured_drag(
            source_window,
            g1,
            PointerCancelReason::CaptureRevoked,
        ))
        .expect("the source window update must accept its own exact cancellation")
    );
    assert!(!cx.update(|app| app.cancel_native_captured_drag(
        source_window,
        g1,
        PointerCancelReason::CaptureRevoked,
    )));
    assert!(cx.read(|cx| cx.active_drag.is_none()));
    assert!(
        cx.update_window(source, |_, window, _| window.captured_pointer().is_none())
            .expect("the exact-cancel source should remain open")
    );
    assert_eq!(
        platform_window.native_pointer_capture_release_count(),
        1,
        "exact cancellation must synchronously request native capture release"
    );
    assert_eq!(
        pointer_cancellations.get(),
        1,
        "the ordered typed cancel must deliver once at the completed outer App boundary"
    );
    cx.run_until_parked();
    assert_eq!(pointer_cancellations.get(), 1);
    let diagnostic_delta = cx.update(|app| app.native_boundary_diagnostics(diagnostic_cursor));
    let cancellation_diagnostic = diagnostic_delta
        .terminal
        .iter()
        .find(|diagnostic| {
            diagnostic.target == NativeBoundaryTarget::Window(source_window)
                && diagnostic.kind
                    == NativeBoundaryKind::Callback(NativeCallbackKind::CapturedDragCancellation)
        })
        .expect("exact cancellation must publish its own boundary diagnostic");
    assert_eq!(
        cancellation_diagnostic.domain_generation,
        Some(NativeBoundaryGeneration::CapturedDrag(g1))
    );
    assert_eq!(
        deliveries.borrow().as_slice(),
        &[
            (NativeCapturedDragPhase::Moved, g1, Some("exact-g1")),
            (
                NativeCapturedDragPhase::Cancelled(PointerCancelReason::CaptureRevoked),
                g1,
                Some("exact-g1"),
            ),
        ]
    );

    moved_generation.set(None);
    let _ = platform_window.simulate_input_result(mouse_down(MouseButton::Left, 12.0, 10.0));
    cx.update_window(source, |_, _, cx| {
        let drag_view = cx.new(|_| Empty).into();
        cx.start_active_drag(AnyDrag {
            window_id: source.window_id(),
            source: Some(capture),
            value: Arc::new("exact-g2"),
            view: drag_view,
            window_preview_offset: point(px(0.0), px(0.0)),
            cursor_style: None,
            button: MouseButton::Left,
        });
    })
    .expect("the captured source should start G2");
    let _ = platform_window.simulate_input_result(PlatformInput::MouseMove(MouseMoveEvent {
        position: point(px(34.0), px(22.0)),
        pressed_button: Some(MouseButton::Left),
        modifiers: Modifiers::none(),
    }));
    cx.run_until_parked();
    let g2 = moved_generation
        .get()
        .expect("the replacement captured move should expose G2");
    assert_ne!(g1, g2);

    assert!(!cx.update(|app| app.cancel_native_captured_drag(
        source_window,
        g1,
        PointerCancelReason::WindowClosed,
    )));
    assert!(cx.read(|cx| cx.active_drag.is_some()));
    assert!(
        cx.update_window(source, |_, window, _| window.captured_pointer().is_some())
            .expect("the replacement source should remain open")
    );
    assert_eq!(pointer_cancellations.get(), 1);
    assert_eq!(platform_window.native_pointer_capture_release_count(), 1);

    assert!(cx.update(|app| app.cancel_native_captured_drag(
        source_window,
        g2,
        PointerCancelReason::WindowClosed,
    )));
    assert!(cx.read(|cx| cx.active_drag.is_none()));
    assert!(
        cx.update_window(source, |_, window, _| window.captured_pointer().is_none())
            .expect("the replacement source should remain open for cleanup")
    );
    cx.run_until_parked();

    assert_eq!(pointer_cancellations.get(), 2);
    assert_eq!(platform_window.native_pointer_capture_release_count(), 2);
    assert_eq!(
        deliveries.borrow().as_slice(),
        &[
            (NativeCapturedDragPhase::Moved, g1, Some("exact-g1")),
            (
                NativeCapturedDragPhase::Cancelled(PointerCancelReason::CaptureRevoked),
                g1,
                Some("exact-g1"),
            ),
            (NativeCapturedDragPhase::Moved, g2, Some("exact-g2")),
            (
                NativeCapturedDragPhase::Cancelled(PointerCancelReason::WindowClosed),
                g2,
                Some("exact-g2"),
            ),
        ]
    );
    let _ = subscription;
}

#[open_gpui::test]
fn exact_native_captured_drag_cancel_preserves_a_replacement_pointer_owner(
    cx: &mut TestAppContext,
) {
    let handles = Rc::new(Cell::new(None));
    let source: crate::AnyWindowHandle = cx
        .open_window(size(px(320.0), px(200.0)), {
            let handles = handles.clone();
            move |window, _| {
                let first = window.new_pointer_capture_handle();
                let second = window.new_pointer_capture_handle();
                handles.set(Some((first, second)));
                PointerCaptureOwnersProbe { first, second }
            }
        })
        .into();
    cx.update_window(source, |_, window, cx| {
        window.activate_window();
        window.draw(cx).clear();
    })
    .expect("the replacement-owner source should remain open");
    cx.run_until_parked();
    let (first, second) = handles
        .get()
        .expect("the source should publish both capture owners");
    let platform_window = cx.test_window(source);

    let generation = cx
        .update_window(source, |_, window, cx| {
            window.dispatch_event(mouse_down(MouseButton::Left, 10.0, 10.0), cx);
            window
                .capture_pointer(&first, MouseButton::Left)
                .expect("the first owner should capture the drag pointer");
            let reservation = cx.reserve_native_captured_drag_start();
            let generation = reservation.token().generation();
            let drag_view = cx.new(|_| Empty).into();
            assert!(cx.start_reserved_active_drag(
                reservation,
                AnyDrag {
                    window_id: source.window_id(),
                    source: Some(first),
                    value: Arc::new("replacement-owner-g1"),
                    view: drag_view,
                    window_preview_offset: point(px(0.0), px(0.0)),
                    cursor_style: None,
                    button: MouseButton::Left,
                },
            ));
            assert_eq!(window.release_pointer(&first), Ok(true));
            window
                .capture_pointer(&second, MouseButton::Left)
                .expect("the replacement owner should capture the still-pressed pointer");
            generation
        })
        .expect("the first owner should start G1");

    let release_terminals = Rc::new(RefCell::new(Vec::new()));
    let release_barrier = cx
        .update({
            let release_terminals = release_terminals.clone();
            move |app| {
                app.cancel_native_captured_drag_with_release_barrier(
                    source.window_id(),
                    generation,
                    PointerCancelReason::CaptureRevoked,
                    move |barrier, terminal, _| {
                        release_terminals.borrow_mut().push((barrier, terminal));
                    },
                )
            }
        })
        .expect("the exact G1 cancellation should reserve its release barrier");
    assert!(cx.read(|app| app.active_drag.is_none()));
    assert_eq!(
        cx.update_window(source, |_, window, _| window
            .captured_pointer()
            .map(|capture| capture.handle()))
            .expect("the replacement-owner source should remain open"),
        Some(second),
        "cancelling G1 must not clear a replacement logical capture"
    );
    assert_eq!(
        platform_window.native_pointer_capture_release_count(),
        0,
        "a live replacement owner must retain the HWND capture session"
    );
    assert_eq!(
        release_terminals.borrow().as_slice(),
        &[(
            release_barrier,
            NativeCapturedDragReleaseTerminal::NotRequired
        )],
        "a replacement logical owner must settle only the cancelled generation as NotRequired"
    );

    cx.update_window(source, |_, window, cx| {
        window.dispatch_event(mouse_up(MouseButton::Left, 10.0, 10.0), cx);
    })
    .expect("the replacement owner should accept its terminal mouse up");
}

#[open_gpui::test]
fn exact_native_captured_drag_cancel_refreshes_source_after_removing_preview(
    cx: &mut TestAppContext,
) {
    let preview_renders = Rc::new(Cell::new(0));
    let source: crate::AnyWindowHandle = cx
        .open_window(size(px(320.0), px(200.0)), |_, _| Empty)
        .into();
    cx.update_window(source, |_, window, cx| {
        window.activate_window();
        window.draw(cx).clear();
    })
    .expect("the preview source should remain open");
    cx.run_until_parked();

    let generation = cx
        .update_window(source, {
            let preview_renders = preview_renders.clone();
            move |_, window, app| {
                let reservation = app.reserve_native_captured_drag_start();
                let generation = reservation.token().generation();
                let drag_view = app
                    .new(move |_| DragPreviewProbe {
                        renders: preview_renders,
                    })
                    .into();
                assert!(app.start_reserved_active_drag(
                    reservation,
                    AnyDrag {
                        window_id: source.window_id(),
                        source: None,
                        value: Arc::new("preview-refresh-g1"),
                        view: drag_view,
                        window_preview_offset: point(px(0.0), px(0.0)),
                        cursor_style: None,
                        button: MouseButton::Left,
                    },
                ));
                window.refresh();
                window.draw(app).clear();
                assert!(!window.invalidator.is_dirty());
                generation
            }
        })
        .expect("the preview source should start G1");
    assert_eq!(preview_renders.get(), 1);

    cx.update(|app| {
        assert!(app.cancel_native_captured_drag(
            source.window_id(),
            generation,
            PointerCancelReason::CaptureRevoked,
        ));
        source
            .update(app, |_, window, _| {
                assert!(
                    window.invalidator.is_dirty(),
                    "removing the active preview must schedule a source-window redraw"
                );
            })
            .expect("the preview source should remain open after cancellation");
    });
}

#[open_gpui::test]
fn app_shutdown_settles_the_active_native_drag_and_preserves_outbox_reuse(cx: &mut TestAppContext) {
    let deliveries = Rc::new(RefCell::new(Vec::new()));
    let subscription = cx.update({
        let deliveries = deliveries.clone();
        move |app| {
            app.observe_native_captured_drag(move |event, _| {
                deliveries
                    .borrow_mut()
                    .push((event.phase(), event.generation()));
            })
        }
    });
    let first: crate::AnyWindowHandle = cx
        .open_window(size(px(320.0), px(200.0)), |_, _| Empty)
        .into();
    cx.update_window(first, |_, window, cx| {
        window.activate_window();
        window.draw(cx).clear();
        let drag_view = cx.new(|_| Empty).into();
        cx.start_active_drag(AnyDrag {
            window_id: first.window_id(),
            source: None,
            value: Arc::new("shutdown-g1"),
            view: drag_view,
            window_preview_offset: point(px(0.0), px(0.0)),
            cursor_style: None,
            button: MouseButton::Left,
        });
    })
    .expect("G1 should start in the first window");
    let mut first_platform_window = cx.test_window(first);
    let _ = first_platform_window.simulate_input_result(PlatformInput::MouseMove(MouseMoveEvent {
        position: point(px(20.0), px(10.0)),
        pressed_button: Some(MouseButton::Left),
        modifiers: Modifiers::none(),
    }));
    cx.run_until_parked();
    let g1 = deliveries
        .borrow()
        .iter()
        .find_map(|(phase, generation)| {
            (*phase == NativeCapturedDragPhase::Moved).then_some(*generation)
        })
        .expect("G1 should publish a captured move");

    let release_attempts = Rc::new(Cell::new(0));
    first_platform_window.set_pointer_capture_release_callback({
        let release_attempts = release_attempts.clone();
        move |_, _| {
            let attempt = release_attempts.get();
            release_attempts.set(attempt + 1);
            if attempt == 0 {
                PlatformPointerCaptureReleaseOutcome::Rejected
            } else {
                PlatformPointerCaptureReleaseOutcome::Released
            }
        }
    });

    cx.quit();
    cx.background_executor
        .advance_clock(Duration::from_millis(8));
    cx.run_until_parked();

    assert!(cx.windows().is_empty());
    assert_eq!(
        release_attempts.get(),
        1,
        "native-window retirement must settle the capture barrier and invalidate its delayed retry"
    );
    assert!(cx.read(|app| app.active_drag.is_none()));
    assert!(deliveries.borrow().iter().any(|(phase, generation)| {
        *generation == g1
            && *phase == NativeCapturedDragPhase::Cancelled(PointerCancelReason::WindowClosed)
    }));

    let second: crate::AnyWindowHandle = cx
        .open_window(size(px(320.0), px(200.0)), |_, _| Empty)
        .into();
    cx.update_window(second, |_, window, cx| {
        window.activate_window();
        window.draw(cx).clear();
        let drag_view = cx.new(|_| Empty).into();
        cx.start_active_drag(AnyDrag {
            window_id: second.window_id(),
            source: None,
            value: Arc::new("shutdown-g2"),
            view: drag_view,
            window_preview_offset: point(px(0.0), px(0.0)),
            cursor_style: None,
            button: MouseButton::Left,
        });
    })
    .expect("G2 should start after reopening the test App");
    let mut second_platform_window = cx.test_window(second);
    let _ =
        second_platform_window.simulate_input_result(PlatformInput::MouseMove(MouseMoveEvent {
            position: point(px(25.0), px(12.0)),
            pressed_button: Some(MouseButton::Left),
            modifiers: Modifiers::none(),
        }));
    cx.run_until_parked();
    let g2 = deliveries
        .borrow()
        .iter()
        .rev()
        .find_map(|(phase, generation)| {
            (*phase == NativeCapturedDragPhase::Moved).then_some(*generation)
        })
        .expect("G2 should publish through the reused outbox");
    assert_ne!(g1, g2);

    assert!(cx.update(|app| app.cancel_native_captured_drag(
        second.window_id(),
        g2,
        PointerCancelReason::WindowClosed,
    )));
    let _ = subscription;
}

#[open_gpui::test]
fn capture_release_rejection_waits_for_fresh_app_progress_before_retry(cx: &mut TestAppContext) {
    let source: crate::AnyWindowHandle = cx
        .open_window(size(px(320.0), px(200.0)), |_, _| Empty)
        .into();
    cx.update_window(source, |_, window, cx| {
        window.activate_window();
        window.draw(cx).clear();
    })
    .expect("the release source should remain open");
    let platform_window = cx.test_window(source);
    let app = Rc::downgrade(&cx.app);
    let dispatcher_observed_idle = Rc::new(Cell::new(true));
    let attempts = Rc::new(Cell::new(0));
    platform_window.set_pointer_capture_release_callback({
        let dispatcher_observed_idle = dispatcher_observed_idle.clone();
        let attempts = attempts.clone();
        move |_, _| {
            let app = app
                .upgrade()
                .expect("the test app must outlive the release dispatcher");
            let idle = app.try_borrow_mut().is_ok();
            dispatcher_observed_idle.set(dispatcher_observed_idle.get() && idle);
            let attempt = attempts.get();
            attempts.set(attempt + 1);
            if attempt == 0 {
                PlatformPointerCaptureReleaseOutcome::Rejected
            } else {
                PlatformPointerCaptureReleaseOutcome::Released
            }
        }
    });

    let generation = cx
        .update_window(source, |_, _, app| {
            let reservation = app.reserve_native_captured_drag_start();
            let generation = reservation.token().generation();
            let drag_view = app.new(|_| Empty).into();
            assert!(app.start_reserved_active_drag(
                reservation,
                AnyDrag {
                    window_id: source.window_id(),
                    source: None,
                    value: Arc::new("release-boundary"),
                    view: drag_view,
                    window_preview_offset: point(px(0.0), px(0.0)),
                    cursor_style: None,
                    button: MouseButton::Left,
                },
            ));
            generation
        })
        .expect("the source must start its native captured drag");

    cx.update(|app| {
        assert!(app.cancel_native_captured_drag(
            source.window_id(),
            generation,
            PointerCancelReason::CaptureRevoked,
        ));
        assert!(
            platform_window
                .native_pointer_capture_release_history()
                .is_empty(),
            "the capture dispatcher must not run while the caller still owns AppRefMut"
        );
    });

    assert!(dispatcher_observed_idle.get());
    assert_eq!(
        platform_window
            .native_pointer_capture_release_prepare_history()
            .len(),
        1,
        "the exact backend pointer session must be prepared once before post-borrow dispatch"
    );
    assert_eq!(
        attempts.get(),
        1,
        "a rejected release must not spin in the same drain"
    );
    cx.background_executor
        .advance_clock(Duration::from_millis(8));
    assert_eq!(
        attempts.get(),
        2,
        "the delayed retry must run after the original native-work pump has completed"
    );
    assert_eq!(platform_window.native_pointer_capture_release_count(), 1);
    assert_eq!(
        platform_window
            .native_pointer_capture_release_prepare_history()
            .len(),
        1,
        "a rejected release retry must reuse the original prepared backend session"
    );
    assert_eq!(
        platform_window
            .native_pointer_capture_release_history()
            .len(),
        2
    );
}

#[open_gpui::test]
fn capture_release_is_prepared_before_a_budget_delayed_first_dispatch(cx: &mut TestAppContext) {
    let source: crate::AnyWindowHandle = cx
        .open_window(size(px(320.0), px(200.0)), |_, _| Empty)
        .into();
    cx.update_window(source, |_, window, cx| {
        window.activate_window();
        window.draw(cx).clear();
    })
    .expect("the release source should remain open");
    let platform_window = cx.test_window(source);
    let dispatcher = platform_window.command_dispatcher();
    let app_cell = cx.app.clone();

    let generation = cx
        .update_window(source, |_, _, app| {
            let reservation = app.reserve_native_captured_drag_start();
            let generation = reservation.token().generation();
            let drag_view = app.new(|_| Empty).into();
            assert!(app.start_reserved_active_drag(
                reservation,
                AnyDrag {
                    window_id: source.window_id(),
                    source: None,
                    value: Arc::new("budget-delayed-release"),
                    view: drag_view,
                    window_preview_offset: point(px(0.0), px(0.0)),
                    cursor_style: None,
                    button: MouseButton::Left,
                },
            ));
            generation
        })
        .expect("the source must start its captured drag");

    cx.update(|app| {
        for _ in 0..65 {
            app_cell.enqueue_platform_window_command(
                source.window_id(),
                dispatcher.clone(),
                PlatformWindowCommand::StartWindowMove,
            );
        }
        assert!(app.cancel_native_captured_drag(
            source.window_id(),
            generation,
            PointerCancelReason::CaptureRevoked,
        ));
        assert!(
            platform_window
                .native_pointer_capture_release_history()
                .is_empty(),
            "queued native work must not execute while AppRefMut is held"
        );
    });

    assert_eq!(
        platform_window
            .native_pointer_capture_release_prepare_history()
            .len(),
        1,
        "the backend pointer session must be frozen before post-borrow work can dispatch"
    );
    assert!(
        platform_window
            .native_pointer_capture_release_history()
            .is_empty(),
        "the first release dispatch must remain behind the 64-work drain budget"
    );
    cx.run_until_parked();
    assert_eq!(platform_window.native_pointer_capture_release_count(), 1);
    assert_eq!(
        platform_window
            .native_pointer_capture_release_prepare_history()
            .len(),
        1,
        "a delayed first dispatch must reuse the originally prepared backend session"
    );
}

#[open_gpui::test]
fn repeated_capture_release_rejection_stops_after_bounded_delayed_retries(cx: &mut TestAppContext) {
    let source: crate::AnyWindowHandle = cx
        .open_window(size(px(320.0), px(200.0)), |_, _| Empty)
        .into();
    cx.update_window(source, |_, window, cx| {
        window.activate_window();
        window.draw(cx).clear();
    })
    .expect("the release source should remain open");
    let platform_window = cx.test_window(source);
    let attempts = Rc::new(Cell::new(0));
    let release_allowed = Rc::new(Cell::new(false));
    platform_window.set_pointer_capture_release_callback({
        let attempts = attempts.clone();
        let release_allowed = release_allowed.clone();
        move |_, _| {
            attempts.set(attempts.get() + 1);
            if release_allowed.get() {
                PlatformPointerCaptureReleaseOutcome::Released
            } else {
                PlatformPointerCaptureReleaseOutcome::Rejected
            }
        }
    });

    let generation = cx
        .update_window(source, |_, _, app| {
            let reservation = app.reserve_native_captured_drag_start();
            let generation = reservation.token().generation();
            let drag_view = app.new(|_| Empty).into();
            assert!(app.start_reserved_active_drag(
                reservation,
                AnyDrag {
                    window_id: source.window_id(),
                    source: None,
                    value: Arc::new("release-retry-bound"),
                    view: drag_view,
                    window_preview_offset: point(px(0.0), px(0.0)),
                    cursor_style: None,
                    button: MouseButton::Left,
                },
            ));
            generation
        })
        .expect("the source must start G1");

    cx.update(|app| {
        assert!(app.cancel_native_captured_drag(
            source.window_id(),
            generation,
            PointerCancelReason::CaptureRevoked,
        ));
    });
    assert_eq!(
        attempts.get(),
        1,
        "the initial release is the only synchronous attempt"
    );

    cx.background_executor
        .advance_clock(Duration::from_millis(8));
    assert_eq!(attempts.get(), 2);
    cx.background_executor
        .advance_clock(Duration::from_millis(32));
    assert_eq!(attempts.get(), 3);
    cx.background_executor
        .advance_clock(Duration::from_millis(128));
    assert_eq!(
        attempts.get(),
        4,
        "three bounded delayed retries must be attempted before waiting for a new native fact"
    );
    cx.run_until_parked();
    assert_eq!(
        attempts.get(),
        4,
        "a saturated rejection must not leave another self-scheduled wake behind"
    );

    assert!(platform_window.simulate_frame(RequestFrameOptions {
        require_presentation: true,
        force_render: true,
    }));
    cx.run_until_parked();
    assert_eq!(
        attempts.get(),
        4,
        "ordinary frame progress must not wake a saturated capture release"
    );

    release_allowed.set(true);
    platform_window.simulate_hover_status_change(true);
    cx.run_until_parked();
    assert_eq!(
        attempts.get(),
        5,
        "a later explicit native fact may retry the saturated barrier without reintroducing a busy wake"
    );
    assert_eq!(platform_window.native_pointer_capture_release_count(), 1);

    cx.update_window(source, |_, window, app| window.remove_window(app))
        .expect("the release source should remain removable after its terminal retry");
    cx.run_until_parked();
    assert!(cx.windows().is_empty());
}

#[open_gpui::test]
fn shutdown_uses_native_window_terminal_after_capture_release_retries_are_saturated(
    cx: &mut TestAppContext,
) {
    let source: crate::AnyWindowHandle = cx
        .open_window(size(px(320.0), px(200.0)), |_, _| Empty)
        .into();
    cx.update_window(source, |_, window, app| {
        window.activate_window();
        window.draw(app).clear();
    })
    .expect("the release source should remain open");
    let platform_window = cx.test_window(source);
    let attempts = Rc::new(Cell::new(0));
    platform_window.set_pointer_capture_release_callback({
        let attempts = attempts.clone();
        move |_, _| {
            attempts.set(attempts.get() + 1);
            PlatformPointerCaptureReleaseOutcome::Rejected
        }
    });

    let generation = cx
        .update_window(source, |_, _, app| {
            let reservation = app.reserve_native_captured_drag_start();
            let generation = reservation.token().generation();
            let drag_view = app.new(|_| Empty).into();
            assert!(app.start_reserved_active_drag(
                reservation,
                AnyDrag {
                    window_id: source.window_id(),
                    source: None,
                    value: Arc::new("shutdown-after-saturated-release"),
                    view: drag_view,
                    window_preview_offset: point(px(0.0), px(0.0)),
                    cursor_style: None,
                    button: MouseButton::Left,
                },
            ));
            generation
        })
        .expect("the source must start its captured drag");
    let completions = Rc::new(RefCell::<
        Vec<(
            NativeCapturedDragReleaseBarrier,
            NativeCapturedDragReleaseTerminal,
        )>,
    >::new(Vec::new()));
    let barrier = cx
        .update({
            let completions = completions.clone();
            move |app| {
                assert!(app.cancel_native_captured_drag(
                    source.window_id(),
                    generation,
                    PointerCancelReason::WindowClosed,
                ));
                app.cancel_native_captured_drag_with_release_barrier(
                    source.window_id(),
                    generation,
                    PointerCancelReason::WindowClosed,
                    move |barrier, terminal, _| {
                        completions.borrow_mut().push((barrier, terminal));
                    },
                )
            }
        })
        .expect("the exact pending release must accept a shutdown continuation");

    cx.background_executor
        .advance_clock(Duration::from_millis(8));
    cx.background_executor
        .advance_clock(Duration::from_millis(32));
    cx.background_executor
        .advance_clock(Duration::from_millis(128));
    cx.run_until_parked();
    assert_eq!(attempts.get(), 4);
    assert!(completions.borrow().is_empty());

    cx.update(|app| app.shutdown());
    cx.run_until_parked();

    assert!(cx.windows().is_empty());
    assert_eq!(
        attempts.get(),
        4,
        "shutdown must retire the native window instead of inventing another release retry"
    );
    assert_eq!(
        platform_window
            .native_pointer_capture_release_prepare_history()
            .len(),
        1,
        "all rejected attempts must share one prepared backend session"
    );
    assert_eq!(
        completions.borrow().as_slice(),
        &[(
            barrier,
            NativeCapturedDragReleaseTerminal::NativeWindowTerminal,
        )],
        "the exact captured-drag barrier must settle from the source window's native terminal"
    );

    let reopened: crate::AnyWindowHandle = cx
        .open_window(size(px(280.0), px(180.0)), |_, _| Empty)
        .into();
    assert!(cx.windows().contains(&reopened));
    cx.update_window(reopened, |_, window, app| window.remove_window(app))
        .expect("the reopened lifecycle should remain removable");
    cx.run_until_parked();
}

#[test]
fn dropping_app_with_pending_capture_release_retry_does_not_deadlock() {
    let mut cx = TestAppContext::single();
    let source: crate::AnyWindowHandle = cx
        .open_window(size(px(320.0), px(200.0)), |_, _| Empty)
        .into();
    cx.update_window(source, |_, window, app| {
        window.activate_window();
        window.draw(app).clear();
    })
    .expect("the release source should remain open");
    let platform_window = cx.test_window(source);
    let attempts = Rc::new(Cell::new(0));
    platform_window.set_pointer_capture_release_callback({
        let attempts = attempts.clone();
        move |_, _| {
            attempts.set(attempts.get() + 1);
            PlatformPointerCaptureReleaseOutcome::Rejected
        }
    });

    let generation = cx
        .update_window(source, |_, _, app| {
            let reservation = app.reserve_native_captured_drag_start();
            let generation = reservation.token().generation();
            let drag_view = app.new(|_| Empty).into();
            assert!(app.start_reserved_active_drag(
                reservation,
                AnyDrag {
                    window_id: source.window_id(),
                    source: None,
                    value: Arc::new("pending-release-drop"),
                    view: drag_view,
                    window_preview_offset: point(px(0.0), px(0.0)),
                    cursor_style: None,
                    button: MouseButton::Left,
                },
            ));
            generation
        })
        .expect("the source must start its captured drag");

    cx.update(|app| {
        assert!(app.cancel_native_captured_drag(
            source.window_id(),
            generation,
            PointerCancelReason::CaptureRevoked,
        ));
    });
    assert_eq!(attempts.get(), 1);

    drop(cx);
}

#[open_gpui::test]
fn stale_delayed_capture_release_wake_cannot_retry_a_newer_rejection_epoch(
    cx: &mut TestAppContext,
) {
    let source: crate::AnyWindowHandle = cx
        .open_window(size(px(320.0), px(200.0)), |_, _| Empty)
        .into();
    cx.update_window(source, |_, window, cx| {
        window.activate_window();
        window.draw(cx).clear();
    })
    .expect("the release source should remain open");
    let platform_window = cx.test_window(source);
    let attempts = Rc::new(Cell::new(0));
    let release_allowed = Rc::new(Cell::new(false));
    platform_window.set_pointer_capture_release_callback({
        let attempts = attempts.clone();
        let release_allowed = release_allowed.clone();
        move |_, _| {
            attempts.set(attempts.get() + 1);
            if release_allowed.get() {
                PlatformPointerCaptureReleaseOutcome::Released
            } else {
                PlatformPointerCaptureReleaseOutcome::Rejected
            }
        }
    });

    let generation = cx
        .update_window(source, |_, _, app| {
            let reservation = app.reserve_native_captured_drag_start();
            let generation = reservation.token().generation();
            let drag_view = app.new(|_| Empty).into();
            assert!(app.start_reserved_active_drag(
                reservation,
                AnyDrag {
                    window_id: source.window_id(),
                    source: None,
                    value: Arc::new("release-retry-epoch"),
                    view: drag_view,
                    window_preview_offset: point(px(0.0), px(0.0)),
                    cursor_style: None,
                    button: MouseButton::Left,
                },
            ));
            generation
        })
        .expect("the source must start G1");

    cx.update(|app| {
        assert!(app.cancel_native_captured_drag(
            source.window_id(),
            generation,
            PointerCancelReason::CaptureRevoked,
        ));
    });
    assert_eq!(attempts.get(), 1);

    cx.app
        .retry_native_pointer_capture_release_for_native_window_progress(source.window_id());
    cx.run_until_parked();
    assert_eq!(
        attempts.get(),
        2,
        "native progress should consume the first pending retry epoch"
    );

    release_allowed.set(true);
    cx.background_executor
        .advance_clock(Duration::from_millis(8));
    assert_eq!(
        attempts.get(),
        2,
        "the stale first-epoch timer must not retry the newer rejection"
    );
    cx.background_executor
        .advance_clock(Duration::from_millis(24));
    assert_eq!(
        attempts.get(),
        3,
        "only the newer rejection epoch's own timer may dispatch its retry"
    );
    assert_eq!(platform_window.native_pointer_capture_release_count(), 1);

    cx.update_window(source, |_, window, app| window.remove_window(app))
        .expect("the source should remain removable after release");
    cx.run_until_parked();
    assert!(cx.windows().is_empty());
}

#[open_gpui::test]
fn captured_drag_release_barrier_attaches_to_the_exact_pending_cancellation(
    cx: &mut TestAppContext,
) {
    let source: crate::AnyWindowHandle = cx
        .open_window(size(px(320.0), px(200.0)), |_, _| Empty)
        .into();
    cx.update_window(source, |_, window, cx| {
        window.activate_window();
        window.draw(cx).clear();
    })
    .expect("the release source should remain open");
    let platform_window = cx.test_window(source);
    let attempts = Rc::new(Cell::new(0));
    platform_window.set_pointer_capture_release_callback({
        let attempts = attempts.clone();
        move |_, _| {
            let attempt = attempts.get();
            attempts.set(attempt + 1);
            if attempt == 0 {
                PlatformPointerCaptureReleaseOutcome::Rejected
            } else {
                PlatformPointerCaptureReleaseOutcome::Released
            }
        }
    });

    let generation = cx
        .update_window(source, |_, _, app| {
            let reservation = app.reserve_native_captured_drag_start();
            let generation = reservation.token().generation();
            let drag_view = app.new(|_| Empty).into();
            assert!(app.start_reserved_active_drag(
                reservation,
                AnyDrag {
                    window_id: source.window_id(),
                    source: None,
                    value: Arc::new("release-attachment"),
                    view: drag_view,
                    window_preview_offset: point(px(0.0), px(0.0)),
                    cursor_style: None,
                    button: MouseButton::Left,
                },
            ));
            generation
        })
        .expect("the source must start G1");

    let completions = Rc::new(RefCell::<
        Vec<(
            NativeCapturedDragReleaseBarrier,
            NativeCapturedDragReleaseTerminal,
        )>,
    >::new(Vec::new()));
    let barrier = cx
        .update({
            let completions = completions.clone();
            move |app| {
                assert!(app.cancel_native_captured_drag(
                    source.window_id(),
                    generation,
                    PointerCancelReason::WindowClosed,
                ));
                let barrier = app.cancel_native_captured_drag_with_release_barrier(
                    source.window_id(),
                    generation,
                    PointerCancelReason::WindowClosed,
                    move |barrier, terminal, _| {
                        completions.borrow_mut().push((barrier, terminal));
                    },
                );
                assert!(
                    platform_window
                        .native_pointer_capture_release_history()
                        .is_empty(),
                    "attaching a release continuation must not synchronously enter the platform"
                );
                barrier
            }
        })
        .expect("the exact pending G1 release must accept an attached continuation");

    assert_eq!(
        attempts.get(),
        1,
        "the first native release attempt should reject once"
    );
    assert!(
        completions.borrow().is_empty(),
        "Rejected is diagnostic-only and must not authorize dependent effects"
    );
    cx.background_executor
        .advance_clock(Duration::from_millis(8));
    assert_eq!(
        completions.borrow().as_slice(),
        &[(barrier, NativeCapturedDragReleaseTerminal::Released)],
        "only the exact G1 release terminal may invoke the attached continuation"
    );
    assert_eq!(
        attempts.get(),
        2,
        "the first delayed retry must release G1 exactly once"
    );

    let diagnostics =
        cx.update(|app| app.native_boundary_diagnostics(NativeBoundaryDiagnosticCursor::default()));
    assert!(diagnostics.terminal.iter().any(|diagnostic| {
        diagnostic.target == NativeBoundaryTarget::Window(source.window_id())
            && diagnostic.kind
                == NativeBoundaryKind::Command(
                    NativePlatformCommandKind::CompleteCapturedDragRelease,
                )
            && diagnostic.domain_generation
                == Some(NativeBoundaryGeneration::PointerCaptureRelease {
                    captured_drag: Some(generation),
                    release: barrier.release_generation(),
                })
            && diagnostic.disposition == NativeBoundaryDisposition::DELIVERED
    }));
}

#[open_gpui::test]
fn source_close_preserves_the_exact_capture_barrier_until_native_terminal(cx: &mut TestAppContext) {
    let capture = Rc::new(Cell::new(None));
    let pointer_cancellations = Rc::new(Cell::new(0));
    let source: crate::AnyWindowHandle = cx
        .open_window(size(px(320.0), px(200.0)), {
            let capture = capture.clone();
            let pointer_cancellations = pointer_cancellations.clone();
            move |window, _| {
                let handle = window.new_pointer_capture_handle();
                capture.set(Some(handle));
                ShutdownPointerCancelProbe {
                    handle,
                    cancellations: pointer_cancellations,
                }
            }
        })
        .into();
    cx.update_window(source, |_, window, app| {
        window.activate_window();
        window.draw(app).clear();
    })
    .expect("the source should remain open through its first frame");
    cx.run_until_parked();

    let mut platform_window = cx.test_window(source);
    let _ = platform_window.simulate_input_result(mouse_down(MouseButton::Left, 12.0, 12.0));
    let capture = capture
        .get()
        .expect("the source should publish its pointer-capture handle");
    let cancelled_generations = Rc::new(RefCell::new(Vec::new()));
    let subscription = cx.update({
        let cancelled_generations = cancelled_generations.clone();
        move |app| {
            app.observe_native_captured_drag(move |event, _| {
                if matches!(event.phase(), NativeCapturedDragPhase::Cancelled(_)) {
                    cancelled_generations.borrow_mut().push(event.generation());
                }
            })
        }
    });
    let generation = cx
        .update_window(source, |_, _, app| {
            let reservation = app.reserve_native_captured_drag_start();
            let generation = reservation.token().generation();
            let drag_view = app.new(|_| Empty).into();
            assert!(app.start_reserved_active_drag(
                reservation,
                AnyDrag {
                    window_id: source.window_id(),
                    source: Some(capture),
                    value: Arc::new("source-close"),
                    view: drag_view,
                    window_preview_offset: point(px(0.0), px(0.0)),
                    cursor_style: None,
                    button: MouseButton::Left,
                },
            ));
            generation
        })
        .expect("the source should start G1");

    let release_attempts = Rc::new(Cell::new(0));
    let native_drop_after_release_attempt = Rc::new(Cell::new(false));
    platform_window.set_pointer_capture_release_callback({
        let release_attempts = release_attempts.clone();
        move |_, _| {
            release_attempts.set(release_attempts.get() + 1);
            PlatformPointerCaptureReleaseOutcome::Rejected
        }
    });
    platform_window.set_native_drop_callback({
        let release_attempts = release_attempts.clone();
        let native_drop_after_release_attempt = native_drop_after_release_attempt.clone();
        move || {
            native_drop_after_release_attempt.set(release_attempts.get() > 0);
        }
    });

    let release_terminals = Rc::new(RefCell::<
        Vec<(
            NativeCapturedDragReleaseBarrier,
            NativeCapturedDragReleaseTerminal,
        )>,
    >::new(Vec::new()));
    let release_barrier = cx.update({
        let release_terminals = release_terminals.clone();
        move |app| {
            source
                .update(app, |_, window, app| window.remove_window(app))
                .expect("the source should remain reachable until its logical removal finishes");
            app.cancel_native_captured_drag_with_release_barrier(
                source.window_id(),
                generation,
                PointerCancelReason::WindowClosed,
                move |barrier, terminal, _| {
                    release_terminals.borrow_mut().push((barrier, terminal));
                },
            )
            .expect(
                "programmatic removal must preserve an exact capture-release barrier for attachment",
            )
        }
    });
    cx.run_until_parked();

    assert!(
        native_drop_after_release_attempt.get(),
        "logical registry removal must not bypass the post-borrow release or native-terminal barrier"
    );
    assert_eq!(pointer_cancellations.get(), 1);
    assert_eq!(cancelled_generations.borrow().as_slice(), &[generation]);
    assert_eq!(
        release_terminals.borrow().as_slice(),
        &[(
            release_barrier,
            NativeCapturedDragReleaseTerminal::NativeWindowTerminal,
        )],
        "source teardown may only release the captured-drag barrier through its native terminal"
    );
    assert!(cx.windows().is_empty());
    let _ = subscription;
}

#[open_gpui::test]
fn native_window_retirement_drops_the_platform_object_only_with_appcell_idle(
    cx: &mut TestAppContext,
) {
    let window: crate::AnyWindowHandle = cx
        .open_window(size(px(320.0), px(200.0)), |_, _| Empty)
        .into();
    let dropped_while_idle = Rc::new(Cell::new(false));
    cx.test_window(window).set_native_drop_callback({
        let app = Rc::downgrade(&cx.app);
        let dropped_while_idle = dropped_while_idle.clone();
        move || {
            let app = app
                .upgrade()
                .expect("the test app must outlive its native window retirement");
            let idle = app.try_borrow_mut().is_ok();
            dropped_while_idle.set(idle);
        }
    });

    cx.update_window(window, |_, window, app| window.remove_window(app))
        .expect("the window should be removable");
    cx.run_until_parked();

    assert!(dropped_while_idle.get());
    assert!(cx.windows().is_empty());
}

#[open_gpui::test]
fn shutdown_waits_for_a_checked_out_source_before_clearing_the_registry(cx: &mut TestAppContext) {
    let source: crate::AnyWindowHandle = cx
        .open_window(size(px(320.0), px(200.0)), |_, _| Empty)
        .into();

    cx.update_window(source, |_, _, app| {
        app.shutdown();
        assert!(
            app.windows.contains_key(source.window_id()),
            "shutdown must leave a checked-out window registered until its transaction returns"
        );
    })
    .expect("the checked-out source should finish its transaction");
    cx.run_until_parked();

    assert!(cx.windows().is_empty());
}

#[open_gpui::test]
fn native_quit_delivers_exact_cancel_before_registry_clear(cx: &mut TestAppContext) {
    let capture = Rc::new(Cell::new(None));
    let pointer_cancellations = Rc::new(Cell::new(0));
    let source: crate::AnyWindowHandle = cx
        .open_window(size(px(320.0), px(200.0)), {
            let capture = capture.clone();
            let pointer_cancellations = pointer_cancellations.clone();
            move |window, _| {
                let handle = window.new_pointer_capture_handle();
                capture.set(Some(handle));
                ShutdownPointerCancelProbe {
                    handle,
                    cancellations: pointer_cancellations,
                }
            }
        })
        .into();
    let deliveries = Rc::new(RefCell::new(Vec::new()));
    let subscription = cx.update({
        let deliveries = deliveries.clone();
        move |app| {
            app.observe_native_captured_drag(move |event, _| {
                deliveries
                    .borrow_mut()
                    .push((event.phase(), event.generation()));
            })
        }
    });
    cx.update_window(source, |_, window, app| {
        window.activate_window();
        window.draw(app).clear();
    })
    .expect("the native-quit source should remain open");
    cx.run_until_parked();
    let mut platform_window = cx.test_window(source);
    let _ = platform_window.simulate_input_result(mouse_down(MouseButton::Left, 12.0, 12.0));
    let handle = capture
        .get()
        .expect("the shutdown probe must publish its capture handle");
    let generation = cx
        .update_window(source, |_, _, app| {
            let reservation = app.reserve_native_captured_drag_start();
            let generation = reservation.token().generation();
            let drag_view = app.new(|_| Empty).into();
            assert!(app.start_reserved_active_drag(
                reservation,
                AnyDrag {
                    window_id: source.window_id(),
                    source: Some(handle),
                    value: Arc::new("native-quit"),
                    view: drag_view,
                    window_preview_offset: point(px(0.0), px(0.0)),
                    cursor_style: None,
                    button: MouseButton::Left,
                },
            ));
            generation
        })
        .expect("the native-quit source should start G1");

    cx.app.enqueue_quit_for_test();
    cx.run_until_parked();

    assert_eq!(pointer_cancellations.get(), 1);
    assert!(deliveries.borrow().iter().any(|(phase, event_generation)| {
        *event_generation == generation
            && *phase == NativeCapturedDragPhase::Cancelled(PointerCancelReason::WindowClosed)
    }));
    assert!(cx.windows().is_empty());
    let _ = subscription;
}

#[open_gpui::test]
fn native_closed_before_logical_pointer_teardown_does_not_strand_a_release_barrier(
    cx: &mut TestAppContext,
) {
    cx.update(|app| app.set_quit_mode(QuitMode::Explicit));
    let handles = Rc::new(Cell::new(None));
    let source: crate::AnyWindowHandle = cx
        .open_window(size(px(320.0), px(200.0)), {
            let handles = handles.clone();
            move |window, _| {
                let first = window.new_pointer_capture_handle();
                let second = window.new_pointer_capture_handle();
                handles.set(Some((first, second)));
                PointerCaptureOwnersProbe { first, second }
            }
        })
        .into();
    cx.update_window(source, |_, window, app| {
        window.activate_window();
        window.draw(app).clear();
    })
    .expect("the source must present before it captures a pointer");
    cx.run_until_parked();
    let (capture, _) = handles
        .get()
        .expect("the source must publish its pointer-capture handles");
    cx.update_window(source, |_, window, app| {
        window.dispatch_event(mouse_down(MouseButton::Left, 12.0, 12.0), app);
        window
            .capture_pointer(&capture, MouseButton::Left)
            .expect("the source must own the pointer before native close");
    })
    .expect("the source must remain reachable before native close");

    let platform_window = cx.test_window(source);
    cx.update_window(source, |_, _, app| {
        assert!(platform_window.simulate_close());
        assert!(
            app.windows.contains_key(source.window_id()),
            "the native Closed callback must remain queued until this logical update ends"
        );
    })
    .expect("the source must complete the update that queued native close");
    cx.run_until_parked();

    assert!(
        !cx.windows().contains(&source),
        "the queued native Closed callback must perform logical teardown after recording terminal"
    );
    assert!(
        platform_window
            .native_pointer_capture_release_history()
            .is_empty(),
        "a release reserved after native Closed must settle as NativeWindowTerminal without dispatch"
    );

    cx.app.enqueue_quit_for_test();
    cx.run_until_parked();
    assert!(
        cx.read(|app| app.quitting),
        "the later native Quit must converge instead of waiting on a release barrier created after Closed"
    );
}

#[open_gpui::test]
fn mouse_exit_is_hover_only_and_does_not_publish_captured_drag_movement(cx: &mut TestAppContext) {
    let source: crate::AnyWindowHandle = cx
        .open_window(size(px(320.0), px(200.0)), |_, _| Empty)
        .into();
    let phases = Rc::new(RefCell::new(Vec::new()));
    let subscription = cx.update({
        let phases = phases.clone();
        move |cx| {
            cx.observe_native_captured_drag(move |event, _| {
                phases.borrow_mut().push(event.phase());
            })
        }
    });

    cx.update_window(source, |_, window, cx| {
        window.activate_window();
        window.draw(cx).clear();
        let drag_view = cx.new(|_| Empty).into();
        cx.start_active_drag(AnyDrag {
            window_id: source.window_id(),
            source: None,
            value: Arc::new("hover-only-exit"),
            view: drag_view,
            window_preview_offset: point(px(0.0), px(0.0)),
            cursor_style: None,
            button: MouseButton::Left,
        });
    })
    .expect("the captured-drag source should remain open");
    cx.run_until_parked();

    let mut platform_window = cx.test_window(source);
    let _ = platform_window.simulate_input_result(PlatformInput::MouseExited(MouseExitEvent {
        position: point(px(50.0), px(40.0)),
        pressed_button: Some(MouseButton::Left),
        modifiers: Modifiers::none(),
    }));
    cx.run_until_parked();

    assert!(phases.borrow().is_empty());
    assert!(cx.read(|cx| cx.active_drag.is_some()));
    cx.update_window(source, |_, window, cx| {
        assert!(cx.stop_active_drag(window));
    })
    .expect("the captured-drag source should remain available for cleanup");
    let _ = subscription;
}

#[open_gpui::test]
fn reentrant_pointer_cancel_waits_for_earlier_native_ingress_before_outbox_delivery(
    cx: &mut TestAppContext,
) {
    let source: crate::AnyWindowHandle = cx
        .open_window(size(px(320.0), px(200.0)), |_, _| Empty)
        .into();
    let order = Rc::new(RefCell::new(Vec::new()));
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
        .expect("the source window should remain open");
    let g1 = Rc::new(Cell::new(None));
    let terminal_has_no_physical_frame = Rc::new(Cell::new(false));
    let captured_subscription = cx.update({
        let order = order.clone();
        let g1 = g1.clone();
        let terminal_has_no_physical_frame = terminal_has_no_physical_frame.clone();
        move |cx| {
            cx.observe_native_captured_drag(move |event, _| match event.phase() {
                NativeCapturedDragPhase::Moved => {
                    g1.set(Some(event.generation()));
                    order.borrow_mut().push("move");
                }
                NativeCapturedDragPhase::Cancelled(PointerCancelReason::PlatformCaptureLost) => {
                    assert_eq!(Some(event.generation()), g1.get());
                    terminal_has_no_physical_frame.set(event.physical_frame().is_none());
                    order.borrow_mut().push("cancel");
                }
                _ => {}
            })
        }
    });

    cx.update_window(source, |_, window, cx| {
        window.activate_window();
        window.draw(cx).clear();
        let drag_view = cx.new(|_| Empty).into();
        cx.start_active_drag(AnyDrag {
            window_id: source.window_id(),
            source: None,
            value: Arc::new("ordered-capture-loss"),
            view: drag_view,
            window_preview_offset: point(px(0.0), px(0.0)),
            cursor_style: None,
            button: MouseButton::Left,
        });
    })
    .expect("the source window should remain open");
    cx.run_until_parked();

    let platform_window = cx.test_window(source);
    let reserve_once = Rc::new(Cell::new(true));
    let reservation = Rc::new(Cell::new(None));
    let interceptor = cx
        .update_window(source, {
            let platform_window = platform_window.clone();
            let reserve_once = reserve_once.clone();
            let reservation = reservation.clone();
            move |_, window, _| {
                window.intercept_window_mouse_events(move |_, _, _| {
                    if reserve_once.replace(false) {
                        platform_window.simulate_active_status_change(true);
                        reservation.set(Some(
                            platform_window.reserve_reentrant_pointer_cancel_for_test(
                                PointerCancelReason::PlatformCaptureLost,
                            ),
                        ));
                    }
                })
            }
        })
        .expect("the source window should remain open");
    order.borrow_mut().clear();

    let mut platform_window = cx.test_window(source);
    let _ = platform_window.simulate_input_result(PlatformInput::MouseMove(MouseMoveEvent {
        position: point(px(30.0), px(20.0)),
        pressed_button: Some(MouseButton::Left),
        modifiers: Modifiers::none(),
    }));
    cx.run_until_parked();

    assert_eq!(
        reservation.get(),
        Some(crate::NativePointerCancelReservation::Reserved)
    );
    assert_eq!(
        order.borrow().as_slice(),
        &["move", "earlier-native-event", "cancel"]
    );
    assert!(terminal_has_no_physical_frame.get());
    assert!(cx.read(|cx| cx.active_drag.is_none()));
    let _ = (activation_subscription, captured_subscription, interceptor);
}

#[open_gpui::test]
fn captured_drag_outbox_waits_for_an_older_command_budget_before_deactivation(
    cx: &mut TestAppContext,
) {
    let source: crate::AnyWindowHandle = cx
        .open_window(size(px(320.0), px(200.0)), |_, _| Empty)
        .into();
    let phases = Rc::new(RefCell::new(Vec::new()));
    let subscription = cx.update({
        let phases = phases.clone();
        move |cx| {
            cx.observe_native_captured_drag(move |event, _| {
                phases.borrow_mut().push(event.phase());
            })
        }
    });

    cx.update_window(source, |_, window, cx| {
        window.activate_window();
        window.draw(cx).clear();
        let drag_view = cx.new(|_| Empty).into();
        cx.start_active_drag(AnyDrag {
            window_id: source.window_id(),
            source: None,
            value: Arc::new("command-budget-g1"),
            view: drag_view,
            window_preview_offset: point(px(0.0), px(0.0)),
            cursor_style: None,
            button: MouseButton::Left,
        });
    })
    .expect("the source window should remain open");
    cx.run_until_parked();

    let mut platform_window = cx.test_window(source);
    cx.update_window(source, {
        let platform_window = platform_window.clone();
        move |_, window, _| {
            for _ in 0..65 {
                window.start_window_move();
            }
            platform_window.simulate_active_status_change(false);
        }
    })
    .expect("the source window should queue its older command prefix");

    let _ = platform_window.simulate_input_result(PlatformInput::MouseMove(MouseMoveEvent {
        position: point(px(30.0), px(20.0)),
        pressed_button: Some(MouseButton::Left),
        modifiers: Modifiers::none(),
    }));
    assert_eq!(
        phases.borrow().as_slice(),
        &[NativeCapturedDragPhase::Cancelled(
            PointerCancelReason::WindowDeactivated
        )],
        "the captured move must not overtake an older command/deactivation barrier"
    );

    cx.run_until_parked();
    assert_eq!(
        phases.borrow().as_slice(),
        &[NativeCapturedDragPhase::Cancelled(
            PointerCancelReason::WindowDeactivated
        )]
    );
    assert!(cx.read(|cx| cx.active_drag.is_none()));
    let _ = subscription;
}

#[open_gpui::test]
fn native_captured_mouse_up_is_locked_before_user_cleanup_and_delivered_post_borrow(
    cx: &mut TestAppContext,
) {
    let source: crate::AnyWindowHandle = cx
        .open_window(size(px(320.0), px(200.0)), |_, _| {
            NativeCapturedDragMouseUpProbe
        })
        .into();
    let deliveries = Rc::new(RefCell::new(Vec::new()));

    let subscription = cx.update(|cx| {
        let deliveries = deliveries.clone();
        cx.observe_native_captured_drag(move |event, cx| {
            let source_present = cx
                .update_window_id(event.source_window(), |_, _, _| ())
                .is_ok();
            deliveries.borrow_mut().push((
                event.phase(),
                source_present,
                cx.active_drag.is_none(),
                event.sequence(),
                event.generation(),
                event.payload::<&'static str>().copied(),
                cx.has_native_window_update_provenance(),
            ));
        })
    });

    cx.update_window(source, |_, window, cx| {
        window.draw(cx).clear();
        let drag_view = cx.new(|_| Empty).into();
        cx.start_active_drag(AnyDrag {
            window_id: source.window_id(),
            source: None,
            value: Arc::new("native-captured-drag"),
            view: drag_view,
            window_preview_offset: point(px(0.0), px(0.0)),
            cursor_style: None,
            button: MouseButton::Left,
        });
    })
    .expect("the source window should remain open");
    let mut platform_window = cx.test_window(source);
    let _ = platform_window.simulate_input_result(mouse_up(MouseButton::Left, 24.0, 18.0));
    assert!(cx.read(|cx| cx.active_drag.is_none()));

    cx.update_window(source, |_, _, cx| {
        let drag_view = cx.new(|_| Empty).into();
        cx.start_active_drag(AnyDrag {
            window_id: source.window_id(),
            source: None,
            value: Arc::new("replacement-drag"),
            view: drag_view,
            window_preview_offset: point(px(0.0), px(0.0)),
            cursor_style: None,
            button: MouseButton::Left,
        });
    })
    .expect("the source window should accept a replacement drag");
    let _ = platform_window.simulate_input_result(mouse_up(MouseButton::Left, 28.0, 20.0));

    let deliveries = deliveries.borrow();
    assert_eq!(deliveries.len(), 2);
    assert_eq!(deliveries[0].0, NativeCapturedDragPhase::Released);
    assert_eq!(deliveries[1].0, NativeCapturedDragPhase::Released);
    assert!(
        deliveries[0].1,
        "the source window must be back in the registry before consumer delivery"
    );
    assert!(
        deliveries[0].2,
        "normal mouse cleanup must complete before consumer delivery"
    );
    assert_eq!(deliveries[0].5, Some("native-captured-drag"));
    assert_eq!(deliveries[1].5, Some("replacement-drag"));
    assert!(!deliveries[0].6);
    assert!(deliveries[0].3 < deliveries[1].3);
    assert_ne!(deliveries[0].4, deliveries[1].4);
    let _ = subscription;
}

#[open_gpui::test]
fn panicking_native_captured_drag_consumer_settles_g1_and_delivers_g2(cx: &mut TestAppContext) {
    let source_capture = Rc::new(Cell::new(None));
    let cancellations = Rc::new(Cell::new(0));
    let source: crate::AnyWindowHandle = cx
        .open_window(size(px(320.0), px(200.0)), {
            let source_capture = source_capture.clone();
            let cancellations = cancellations.clone();
            move |window, _| {
                let handle = window.new_pointer_capture_handle();
                source_capture.set(Some(handle));
                NativeCapturedDragConsumerPanicProbe {
                    handle,
                    cancellations,
                    panic_on_cancel: Rc::new(Cell::new(false)),
                }
            }
        })
        .into();
    let panic_once = Rc::new(Cell::new(true));
    let failed_generation = Rc::new(Cell::new(None));
    let deliveries = Rc::new(RefCell::new(Vec::new()));
    let subscription = cx.update({
        let panic_once = panic_once.clone();
        let failed_generation = failed_generation.clone();
        let deliveries = deliveries.clone();
        move |cx| {
            cx.observe_native_captured_drag(move |event, _| {
                if panic_once.replace(false) {
                    failed_generation.set(Some(event.generation()));
                    panic!("injected native captured-drag consumer panic");
                }
                deliveries
                    .borrow_mut()
                    .push((event.phase(), event.generation()));
            })
        }
    });

    cx.update_window(source, |_, window, cx| {
        window.activate_window();
        window.draw(cx).clear();
    })
    .expect("the source window should remain open");
    cx.run_until_parked();
    let mut platform_window = cx.test_window(source);
    let _ = platform_window.simulate_input_result(mouse_down(MouseButton::Left, 10.0, 10.0));
    let capture = source_capture
        .get()
        .expect("the panic-recovery source should expose its capture handle");
    cx.update_window(source, |_, _, cx| {
        let drag_view = cx.new(|_| Empty).into();
        cx.start_active_drag(AnyDrag {
            window_id: source.window_id(),
            source: Some(capture),
            value: Arc::new("g1"),
            view: drag_view,
            window_preview_offset: point(px(0.0), px(0.0)),
            cursor_style: None,
            button: MouseButton::Left,
        });
    })
    .expect("the captured source should start G1");
    let panic = catch_unwind(AssertUnwindSafe(|| {
        platform_window.simulate_input_result(PlatformInput::MouseMove(MouseMoveEvent {
            position: point(px(30.0), px(20.0)),
            pressed_button: Some(MouseButton::Left),
            modifiers: Modifiers::none(),
        }))
    }));
    assert!(panic.is_err());
    cx.run_until_parked();

    let g1 = failed_generation
        .get()
        .expect("the first generation must reach the consumer before it panics");
    assert!(cx.read(|cx| cx.active_drag.is_none()));
    assert_eq!(
        cancellations.get(),
        1,
        "a panicking consumer must deliver exactly one real pointer cancellation"
    );
    assert!(
        cx.update_window(source, |_, window, _| window.captured_pointer().is_none())
            .expect("the source window should remain open"),
        "panic recovery must release the source capture before G2"
    );
    assert!(deliveries.borrow().is_empty());

    cx.update_window(source, |_, _, cx| {
        let drag_view = cx.new(|_| Empty).into();
        cx.start_active_drag(AnyDrag {
            window_id: source.window_id(),
            source: None,
            value: Arc::new("g2"),
            view: drag_view,
            window_preview_offset: point(px(0.0), px(0.0)),
            cursor_style: None,
            button: MouseButton::Left,
        });
    })
    .expect("the source window should accept a second drag generation");
    let _ = platform_window.simulate_input_result(PlatformInput::MouseMove(MouseMoveEvent {
        position: point(px(34.0), px(22.0)),
        pressed_button: Some(MouseButton::Left),
        modifiers: Modifiers::none(),
    }));
    cx.run_until_parked();

    let deliveries = deliveries.borrow();
    assert_eq!(deliveries.len(), 1);
    assert_eq!(deliveries[0].0, NativeCapturedDragPhase::Moved);
    assert_ne!(deliveries[0].1, g1);
    let _ = subscription;
}

#[open_gpui::test]
fn panicking_pointer_cancel_listener_still_settles_g1_and_allows_g2(cx: &mut TestAppContext) {
    let source_capture = Rc::new(Cell::new(None));
    let cancellations = Rc::new(Cell::new(0));
    let panic_on_cancel = Rc::new(Cell::new(true));
    let source: crate::AnyWindowHandle = cx
        .open_window(size(px(320.0), px(200.0)), {
            let source_capture = source_capture.clone();
            let cancellations = cancellations.clone();
            let panic_on_cancel = panic_on_cancel.clone();
            move |window, _| {
                let handle = window.new_pointer_capture_handle();
                source_capture.set(Some(handle));
                NativeCapturedDragConsumerPanicProbe {
                    handle,
                    cancellations,
                    panic_on_cancel,
                }
            }
        })
        .into();
    let phases = Rc::new(RefCell::new(Vec::new()));
    let subscription = cx.update({
        let phases = phases.clone();
        move |cx| {
            cx.observe_native_captured_drag(move |event, _| {
                phases.borrow_mut().push(event.phase());
            })
        }
    });

    cx.update_window(source, |_, window, cx| {
        window.activate_window();
        window.draw(cx).clear();
    })
    .expect("the source window should remain open");
    cx.run_until_parked();

    let mut platform_window = cx.test_window(source);
    let _ = platform_window.simulate_input_result(mouse_down(MouseButton::Left, 10.0, 10.0));
    let capture = source_capture
        .get()
        .expect("the pointer-cancel source should expose its capture handle");
    cx.update_window(source, |_, _, cx| {
        let drag_view = cx.new(|_| Empty).into();
        cx.start_active_drag(AnyDrag {
            window_id: source.window_id(),
            source: Some(capture),
            value: Arc::new("cancel-panic-g1"),
            view: drag_view,
            window_preview_offset: point(px(0.0), px(0.0)),
            cursor_style: None,
            button: MouseButton::Left,
        });
    })
    .expect("the captured source should start G1");

    let cancel_panicked = catch_unwind(AssertUnwindSafe(|| {
        platform_window.simulate_input_result(pointer_cancel())
    }))
    .is_err();
    assert!(
        cancel_panicked,
        "pointer cancel listener did not panic: cancellations={} armed={}",
        cancellations.get(),
        panic_on_cancel.get()
    );
    cx.run_until_parked();

    assert_eq!(cancellations.get(), 1);
    assert_eq!(
        phases.borrow().as_slice(),
        &[NativeCapturedDragPhase::Cancelled(
            PointerCancelReason::PlatformCaptureLost
        )]
    );
    assert!(cx.read(|cx| cx.active_drag.is_none()));
    assert!(
        cx.update_window(source, |_, window, _| window.captured_pointer().is_none())
            .expect("the source window should remain open")
    );

    let _ = platform_window.simulate_input_result(mouse_down(MouseButton::Left, 10.0, 10.0));
    cx.update_window(source, |_, _, cx| {
        let drag_view = cx.new(|_| Empty).into();
        cx.start_active_drag(AnyDrag {
            window_id: source.window_id(),
            source: Some(capture),
            value: Arc::new("cancel-panic-g2"),
            view: drag_view,
            window_preview_offset: point(px(0.0), px(0.0)),
            cursor_style: None,
            button: MouseButton::Left,
        });
    })
    .expect("the restored pointer listeners should allow G2");
    assert!(cx.read(|cx| cx.active_drag.is_some()));
    let _ = platform_window.simulate_input_result(mouse_up(MouseButton::Left, 12.0, 10.0));
    assert!(cx.read(|cx| cx.active_drag.is_none()));
    let _ = subscription;
}

#[open_gpui::test]
fn pointer_panic_recovery_reserves_cancel_while_outer_app_borrow_is_busy(cx: &mut TestAppContext) {
    let source: crate::AnyWindowHandle = cx
        .open_window(size(px(320.0), px(200.0)), |_, _| Empty)
        .into();
    let deliveries = Rc::new(RefCell::new(Vec::new()));
    let subscription = cx.update({
        let deliveries = deliveries.clone();
        move |cx| {
            cx.observe_native_captured_drag(move |event, _| {
                deliveries
                    .borrow_mut()
                    .push((event.phase(), event.generation()));
            })
        }
    });
    cx.update_window(source, |_, window, cx| {
        window.activate_window();
        window.draw(cx).clear();
        let drag_view = cx.new(|_| Empty).into();
        cx.start_active_drag(AnyDrag {
            window_id: source.window_id(),
            source: None,
            value: Arc::new("panic-g1"),
            view: drag_view,
            window_preview_offset: point(px(0.0), px(0.0)),
            cursor_style: None,
            button: MouseButton::Left,
        });
    })
    .expect("the source window should remain open");
    cx.run_until_parked();

    let mut platform_window = cx.test_window(source);
    let reservation = Rc::new(Cell::new(None));
    cx.update_window(source, {
        let reservation = reservation.clone();
        let recovery_window = platform_window.clone();
        let mut nested_window = platform_window.clone();
        move |_, _, _| {
            let nested_panic = catch_unwind(AssertUnwindSafe(|| {
                nested_window.simulate_input_result(PlatformInput::MouseMove(MouseMoveEvent {
                    position: point(px(30.0), px(20.0)),
                    pressed_button: Some(MouseButton::Left),
                    modifiers: Modifiers::none(),
                }))
            }));
            assert!(nested_panic.is_err());
            reservation.set(Some(
                recovery_window.reserve_pointer_cancel_after_callback_panic_for_test(
                    PointerCancelReason::CaptureRevoked,
                ),
            ));
        }
    })
    .expect("the source window should remain open");
    cx.run_until_parked();

    assert_eq!(
        reservation.get(),
        Some(crate::NativePointerCancelReservation::Reserved)
    );
    assert_eq!(
        platform_window.reserve_pointer_cancel_after_callback_panic_for_test(
            PointerCancelReason::CaptureRevoked,
        ),
        crate::NativePointerCancelReservation::NoActiveCallback,
        "panic recovery may reserve its exact callback generation only once"
    );
    let g1 = deliveries
        .borrow()
        .iter()
        .find_map(|(phase, generation)| {
            (*phase == NativeCapturedDragPhase::Cancelled(PointerCancelReason::CaptureRevoked))
                .then_some(*generation)
        })
        .expect("the queued panic cancellation must settle G1");
    assert!(cx.read(|cx| cx.active_drag.is_none()));

    cx.update_window(source, |_, _, cx| {
        let drag_view = cx.new(|_| Empty).into();
        cx.start_active_drag(AnyDrag {
            window_id: source.window_id(),
            source: None,
            value: Arc::new("panic-g2"),
            view: drag_view,
            window_preview_offset: point(px(0.0), px(0.0)),
            cursor_style: None,
            button: MouseButton::Left,
        });
    })
    .expect("the source window should accept G2");
    let _ = platform_window.simulate_input_result(PlatformInput::MouseMove(MouseMoveEvent {
        position: point(px(34.0), px(22.0)),
        pressed_button: Some(MouseButton::Left),
        modifiers: Modifiers::none(),
    }));
    cx.run_until_parked();
    assert!(deliveries.borrow().iter().any(|(phase, generation)| {
        *phase == NativeCapturedDragPhase::Moved && *generation != g1
    }));
    let _ = subscription;
}

#[open_gpui::test]
fn pointer_panic_recovery_does_not_replace_a_locked_mouse_up_release(cx: &mut TestAppContext) {
    let source: crate::AnyWindowHandle = cx
        .open_window(size(px(320.0), px(200.0)), |_, _| Empty)
        .into();
    let deliveries = Rc::new(RefCell::new(Vec::new()));
    let subscription = cx.update({
        let deliveries = deliveries.clone();
        move |cx| {
            cx.observe_native_captured_drag(move |event, cx| {
                deliveries.borrow_mut().push((
                    event.phase(),
                    event.generation(),
                    cx.active_drag.is_none(),
                ));
            })
        }
    });
    cx.update_window(source, |_, window, cx| {
        window.activate_window();
        window.draw(cx).clear();
        let drag_view = cx.new(|_| Empty).into();
        cx.start_active_drag(AnyDrag {
            window_id: source.window_id(),
            source: None,
            value: Arc::new("released-g1"),
            view: drag_view,
            window_preview_offset: point(px(0.0), px(0.0)),
            cursor_style: None,
            button: MouseButton::Left,
        });
    })
    .expect("the source window should remain open");
    cx.run_until_parked();

    let panic_once = Rc::new(Cell::new(true));
    let interceptor = cx
        .update_window(source, {
            let panic_once = panic_once.clone();
            move |_, window, _| {
                window.intercept_window_mouse_events(move |event, _, _| {
                    if matches!(event, WindowMouseEvent::Up(event) if event.button == MouseButton::Left)
                        && panic_once.replace(false)
                    {
                        panic!("injected mouse-up interceptor panic");
                    }
                })
            }
        })
        .expect("the source window should remain open");

    let mut platform_window = cx.test_window(source);
    let panic = catch_unwind(AssertUnwindSafe(|| {
        platform_window.simulate_input_result(mouse_up(MouseButton::Left, 24.0, 18.0))
    }));
    assert!(panic.is_err());
    assert_eq!(
        platform_window.reserve_pointer_cancel_after_callback_panic_for_test(
            PointerCancelReason::CaptureRevoked,
        ),
        crate::NativePointerCancelReservation::Reserved
    );
    cx.run_until_parked();

    let delivery_snapshot = deliveries.borrow();
    assert_eq!(delivery_snapshot.len(), 1);
    assert_eq!(delivery_snapshot[0].0, NativeCapturedDragPhase::Released);
    assert!(
        delivery_snapshot[0].2,
        "terminal cleanup must complete before the captured release reaches a consumer"
    );
    let g1 = delivery_snapshot[0].1;
    drop(delivery_snapshot);
    assert!(cx.read(|cx| cx.active_drag.is_none()));

    cx.update_window(source, |_, _, cx| {
        let drag_view = cx.new(|_| Empty).into();
        cx.start_active_drag(AnyDrag {
            window_id: source.window_id(),
            source: None,
            value: Arc::new("released-g2"),
            view: drag_view,
            window_preview_offset: point(px(0.0), px(0.0)),
            cursor_style: None,
            button: MouseButton::Left,
        });
    })
    .expect("the source window should accept G2");
    let _ = platform_window.simulate_input_result(PlatformInput::MouseMove(MouseMoveEvent {
        position: point(px(34.0), px(22.0)),
        pressed_button: Some(MouseButton::Left),
        modifiers: Modifiers::none(),
    }));
    cx.run_until_parked();
    assert!(deliveries.borrow().iter().any(|(phase, generation, _)| {
        *phase == NativeCapturedDragPhase::Moved && *generation != g1
    }));
    let _ = (subscription, interceptor);
}

#[open_gpui::test]
fn native_window_update_provenance_does_not_leak_into_nested_target_updates(
    cx: &mut TestAppContext,
) {
    let target: crate::AnyWindowHandle = cx
        .open_window(size(px(240.0), px(160.0)), |_, _| Empty)
        .into();
    let observations = Rc::new(RefCell::new(Vec::new()));
    let source: crate::AnyWindowHandle = cx
        .open_window(size(px(320.0), px(200.0)), {
            let observations = observations.clone();
            move |_, _| NativeWindowUpdateProvenanceProbe {
                target,
                observations,
            }
        })
        .into();

    cx.update_window(source, |_, window, cx| window.draw(cx).clear())
        .expect("the source window should remain open");
    let mut platform_window = cx.test_window(source);
    let _ = platform_window.simulate_input_result(PlatformInput::MouseMove(MouseMoveEvent {
        position: point(px(20.0), px(20.0)),
        pressed_button: None,
        modifiers: Modifiers::none(),
    }));

    assert_eq!(
        observations.borrow().as_slice(),
        [true, false, true],
        "nested ordinary updates must clear and then restore the exact source provenance"
    );
}
