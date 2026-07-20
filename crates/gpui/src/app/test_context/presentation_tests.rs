use std::{
    any::TypeId,
    cell::{Cell, RefCell},
    ops::Range,
    rc::Rc,
};

use accesskit::{
    Action as AccessibleAction, ActionRequest, Node, NodeId, Role, TreeId, TreeUpdate,
};
use open_gpui_refineable::Refineable as _;

use crate::{
    AnyElement, AnyView, AnyWindowHandle, App, AppContext as _, Bounds, Context, CursorStyle,
    DispatchPhase, Element, ElementId, Entity, FocusClaimOutcome, FocusHandle, GlobalElementId,
    Hitbox, HitboxBehavior, InputHandler, InspectorElementId, InteractiveElement, IntoElement,
    Keystroke, LayoutId, Modifiers, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent,
    ParentElement, Pixels, PlatformInput, PointerCancelEvent, PointerCancelReason,
    PointerCaptureError, PointerCaptureHandle, Render, ScrollHandle, StatefulInteractiveElement,
    Style, StyleRefinement, Styled, Subscription, SubtreePresentation, SubtreePresentationExt,
    TestAppContext, UTF16Selection, Window, WindowMouseEvent, canvas, deferred, div, fill, point,
    px, red, size, window_portal,
};

crate::actions!(presentation_probe_actions, [PresentationProbeAction]);

#[derive(Default)]
struct PresentationCounters {
    layouts: Cell<usize>,
    prepaints: Cell<usize>,
    paints: Cell<usize>,
    pointer_bindings: Cell<usize>,
    autoscrolls: Cell<usize>,
    mouse_downs: Cell<usize>,
    key_downs: Cell<usize>,
    actions: Cell<usize>,
    accessibility_actions: Cell<usize>,
    pointer_cancellations: Cell<usize>,
}

#[derive(Clone, Copy)]
struct PresentationCounterSnapshot {
    layouts: usize,
    prepaints: usize,
    paints: usize,
    pointer_bindings: usize,
    autoscrolls: usize,
    mouse_downs: usize,
    key_downs: usize,
    actions: usize,
    accessibility_actions: usize,
    pointer_cancellations: usize,
}

impl PresentationCounters {
    fn snapshot(&self) -> PresentationCounterSnapshot {
        PresentationCounterSnapshot {
            layouts: self.layouts.get(),
            prepaints: self.prepaints.get(),
            paints: self.paints.get(),
            pointer_bindings: self.pointer_bindings.get(),
            autoscrolls: self.autoscrolls.get(),
            mouse_downs: self.mouse_downs.get(),
            key_downs: self.key_downs.get(),
            actions: self.actions.get(),
            accessibility_actions: self.accessibility_actions.get(),
            pointer_cancellations: self.pointer_cancellations.get(),
        }
    }
}

#[derive(Clone, Default)]
struct PresentationImeState {
    marked: Rc<Cell<bool>>,
    marked_updates: Rc<Cell<usize>>,
    unmarks: Rc<Cell<usize>>,
    focus_on_unmark: Rc<RefCell<Option<FocusHandle>>>,
    platform_handler_present_on_unmark: Rc<Cell<bool>>,
}

#[derive(Clone)]
struct PresentationInputHandler {
    state: PresentationImeState,
    bounds: Bounds<Pixels>,
}

impl InputHandler for PresentationInputHandler {
    fn selected_text_range(
        &mut self,
        _: bool,
        _: &mut Window,
        _: &mut App,
    ) -> Option<UTF16Selection> {
        Some(UTF16Selection {
            range: 0..0,
            reversed: false,
        })
    }

    fn marked_text_range(&mut self, _: &mut Window, _: &mut App) -> Option<Range<usize>> {
        self.state.marked.get().then_some(0..1)
    }

    fn text_for_range(
        &mut self,
        _: Range<usize>,
        _: &mut Option<Range<usize>>,
        _: &mut Window,
        _: &mut App,
    ) -> Option<String> {
        Some(String::new())
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
        self.state.marked.set(true);
        self.state
            .marked_updates
            .set(self.state.marked_updates.get() + 1);
    }

    fn unmark_text(&mut self, window: &mut Window, cx: &mut App) {
        self.state.marked.set(false);
        self.state.unmarks.set(self.state.unmarks.get() + 1);
        let installed = window.platform_window.take_input_handler();
        self.state
            .platform_handler_present_on_unmark
            .set(installed.is_some());
        if let Some(installed) = installed {
            window.platform_window.set_input_handler(installed);
        }
        let focus = self.state.focus_on_unmark.borrow().clone();
        if let Some(focus) = focus {
            focus.focus(window, cx);
        }
    }

    fn bounds_for_range(
        &mut self,
        _: Range<usize>,
        _: &mut Window,
        _: &mut App,
    ) -> Option<Bounds<Pixels>> {
        Some(self.bounds)
    }

    fn character_index_for_point(
        &mut self,
        _: crate::Point<Pixels>,
        _: &mut Window,
        _: &mut App,
    ) -> Option<usize> {
        Some(0)
    }
}

struct RawPresentationProbe {
    style: StyleRefinement,
    focus: FocusHandle,
    capture: PointerCaptureHandle,
    counters: Rc<PresentationCounters>,
    ime: PresentationImeState,
}

impl IntoElement for RawPresentationProbe {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Styled for RawPresentationProbe {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl Element for RawPresentationProbe {
    type RequestLayoutState = Style;
    type PrepaintState = Hitbox;

    fn id(&self) -> Option<ElementId> {
        Some(ElementId::from("raw-presentation-probe"))
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn a11y_role(&self) -> Option<Role> {
        Some(Role::Button)
    }

    fn write_a11y_info(&self, node: &mut Node) {
        node.set_label("Raw presentation probe");
        node.add_action(AccessibleAction::Click);
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        self.counters.layouts.set(self.counters.layouts.get() + 1);
        let mut style = Style::default();
        style.refine(&self.style);
        let layout_id = window.request_layout(style.clone(), [], cx);
        (layout_id, style)
    }

    fn prepaint(
        &mut self,
        id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _style: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        self.counters
            .prepaints
            .set(self.counters.prepaints.get() + 1);
        let hitbox = window.insert_hitbox(bounds, HitboxBehavior::Normal);
        if window
            .bind_pointer_capture(&self.capture, hitbox.id)
            .is_ok()
            && hitbox.is_active()
        {
            self.counters
                .pointer_bindings
                .set(self.counters.pointer_bindings.get() + 1);
        }
        window.request_autoscroll(bounds);
        if window.take_autoscroll().is_some() {
            self.counters
                .autoscrolls
                .set(self.counters.autoscrolls.get() + 1);
        }
        window.set_focus_handle(&self.focus, cx);

        if let Some(id) = id {
            let counters = self.counters.clone();
            window.on_a11y_action(
                id.accesskit_node_id(),
                AccessibleAction::Click,
                move |_, _, _| {
                    counters
                        .accessibility_actions
                        .set(counters.accessibility_actions.get() + 1);
                },
            );
        }

        hitbox
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _style: &mut Self::RequestLayoutState,
        hitbox: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        self.counters.paints.set(self.counters.paints.get() + 1);
        window.paint_quad(fill(bounds, red()));
        window.set_cursor_style(CursorStyle::PointingHand, hitbox);

        let capture = self.capture;
        let target = hitbox.id;
        let counters = self.counters.clone();
        window.on_mouse_event(move |event: &MouseDownEvent, phase, window, _| {
            if phase == DispatchPhase::Bubble
                && event.button == MouseButton::Left
                && target.is_mouse_event_target(window)
            {
                counters.mouse_downs.set(counters.mouse_downs.get() + 1);
                window
                    .capture_pointer(&capture, MouseButton::Left)
                    .expect("the visible raw probe should own its pointer binding");
            }
        });

        let counters = self.counters.clone();
        window.on_pointer_cancel(move |_: &PointerCancelEvent, phase, _, _| {
            if phase == DispatchPhase::Bubble {
                counters
                    .pointer_cancellations
                    .set(counters.pointer_cancellations.get() + 1);
            }
        });

        let counters = self.counters.clone();
        window.on_key_event(move |_: &crate::KeyDownEvent, phase, _, _| {
            if phase == DispatchPhase::Bubble {
                counters.key_downs.set(counters.key_downs.get() + 1);
            }
        });

        let counters = self.counters.clone();
        window.on_action(
            TypeId::of::<PresentationProbeAction>(),
            move |_, phase, _, _| {
                if phase == DispatchPhase::Bubble {
                    counters.actions.set(counters.actions.get() + 1);
                }
            },
        );

        window.handle_input(
            &self.focus,
            PresentationInputHandler {
                state: self.ime.clone(),
                bounds,
            },
            cx,
        );
    }
}

struct PresentationProbeView {
    presentation: SubtreePresentation,
    descendant_presentation: SubtreePresentation,
    focus: FocusHandle,
    capture: PointerCaptureHandle,
    counters: Rc<PresentationCounters>,
    ime: PresentationImeState,
}

impl Render for PresentationProbeView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let probe = RawPresentationProbe {
            style: StyleRefinement::default(),
            focus: self.focus.clone(),
            capture: self.capture,
            counters: self.counters.clone(),
            ime: self.ime.clone(),
        }
        .w(px(80.0))
        .h(px(40.0))
        .with_subtree_presentation(self.descendant_presentation)
        .with_subtree_presentation(self.presentation);

        div().flex().child(probe).child(
            div()
                .id("presentation-layout-sibling")
                .debug_selector(|| "presentation-layout-sibling".to_owned())
                .w(px(30.0))
                .h(px(40.0)),
        )
    }
}

fn update_presentation(
    view: &Entity<PresentationProbeView>,
    cx: &mut TestAppContext,
    presentation: SubtreePresentation,
    descendant_presentation: SubtreePresentation,
) {
    view.update(cx, |view, cx| {
        view.presentation = presentation;
        view.descendant_presentation = descendant_presentation;
        cx.notify();
    });
    cx.run_until_parked();
}

fn debug_bounds(
    cx: &mut TestAppContext,
    window: AnyWindowHandle,
    selector: &str,
) -> Bounds<Pixels> {
    cx.update_window(window, |_, window, _| {
        window.rendered_frame.debug_bounds.get(selector).copied()
    })
    .unwrap()
    .unwrap_or_else(|| panic!("missing debug bounds for {selector}"))
}

fn node_id_with_label(update: &TreeUpdate, label: &str) -> Option<NodeId> {
    update
        .nodes
        .iter()
        .find_map(|(id, node)| (node.label() == Some(label)).then_some(*id))
}

fn platform_has_input_handler(cx: &mut TestAppContext, window: AnyWindowHandle) -> bool {
    cx.update_window(window, |_, window, _| {
        let handler = window.platform_window.take_input_handler();
        let present = handler.is_some();
        if let Some(handler) = handler {
            window.platform_window.set_input_handler(handler);
        }
        present
    })
    .unwrap()
}

fn draw_focus_followup_frame(cx: &mut TestAppContext, window: AnyWindowHandle) {
    cx.update_window(window, |_, window, cx| {
        assert!(
            window.refresh_pending_for_test(),
            "a sealed focus request must leave one candidate frame pending"
        );
        window.draw(cx).clear();
    })
    .unwrap();
    cx.run_until_parked();
}

struct DelayedMeasurePresentationProbe {
    measure_calls: Rc<Cell<usize>>,
    hitbox_active: Rc<Cell<Option<bool>>>,
}

impl IntoElement for DelayedMeasurePresentationProbe {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for DelayedMeasurePresentationProbe {
    type RequestLayoutState = ();
    type PrepaintState = ();

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        _cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let measure_calls = self.measure_calls.clone();
        let hitbox_active = self.hitbox_active.clone();
        let layout_id =
            window.request_measured_layout(Style::default(), move |_, _, window, cx| {
                measure_calls.set(measure_calls.get() + 1);
                let hitbox = window.insert_hitbox(
                    Bounds::new(point(px(0.0), px(0.0)), size(px(80.0), px(40.0))),
                    HitboxBehavior::Normal,
                );
                hitbox_active.set(Some(hitbox.is_active()));
                let _ = cx;
                size(px(80.0), px(40.0))
            });
        (layout_id, ())
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        _window: &mut Window,
        _cx: &mut App,
    ) {
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        _prepaint: &mut Self::PrepaintState,
        _window: &mut Window,
        _cx: &mut App,
    ) {
    }
}

struct DelayedMeasurePresentationView {
    inert_calls: Rc<Cell<usize>>,
    hidden_calls: Rc<Cell<usize>>,
    inert_hitbox_active: Rc<Cell<Option<bool>>>,
    hidden_hitbox_active: Rc<Cell<Option<bool>>>,
}

impl Render for DelayedMeasurePresentationView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .child(
                DelayedMeasurePresentationProbe {
                    measure_calls: self.inert_calls.clone(),
                    hitbox_active: self.inert_hitbox_active.clone(),
                }
                .with_subtree_presentation(SubtreePresentation::Inert),
            )
            .child(
                DelayedMeasurePresentationProbe {
                    measure_calls: self.hidden_calls.clone(),
                    hitbox_active: self.hidden_hitbox_active.clone(),
                }
                .with_subtree_presentation(SubtreePresentation::Hidden),
            )
    }
}

#[open_gpui::test]
fn delayed_measure_callbacks_reenter_their_presentation_scope(cx: &mut TestAppContext) {
    let inert_calls = Rc::new(Cell::new(0));
    let hidden_calls = Rc::new(Cell::new(0));
    let inert_hitbox_active = Rc::new(Cell::new(None));
    let hidden_hitbox_active = Rc::new(Cell::new(None));
    let typed_window = cx.open_window(size(px(320.0), px(200.0)), {
        let inert_calls = inert_calls.clone();
        let hidden_calls = hidden_calls.clone();
        let inert_hitbox_active = inert_hitbox_active.clone();
        let hidden_hitbox_active = hidden_hitbox_active.clone();
        move |_, _| DelayedMeasurePresentationView {
            inert_calls,
            hidden_calls,
            inert_hitbox_active,
            hidden_hitbox_active,
        }
    });
    let window = typed_window.into();
    cx.run_until_parked();

    assert!(inert_calls.get() > 0);
    assert!(hidden_calls.get() > 0);
    assert_eq!(inert_hitbox_active.get(), Some(false));
    assert_eq!(hidden_hitbox_active.get(), Some(false));
    assert_eq!(
        cx.update_window(window, |_, window, _| window.rendered_frame.hitboxes.len())
            .unwrap(),
        0
    );
}

struct SuppressedFocusMutationView {
    sibling_focus: FocusHandle,
    suppressed_focus: FocusHandle,
}

struct SuppressedFocusControlView {
    scope_focus: FocusHandle,
    target_focus: FocusHandle,
    run_controls: Rc<Cell<bool>>,
    traversal_results: Rc<Cell<Option<(bool, bool)>>>,
    blur_outcomes: Rc<RefCell<Vec<FocusClaimOutcome>>>,
    blur_subscriptions: Rc<RefCell<Vec<Subscription>>>,
}

struct FocusCompletionTreeView {
    root_focus: FocusHandle,
    child_focus: FocusHandle,
    sibling_focus: FocusHandle,
}

struct FocusHandleTransitionView {
    focus: Option<FocusHandle>,
}

struct CommitPhaseFocusView {
    first_focus: FocusHandle,
    committed_focus: FocusHandle,
    commit_focus_on_next_frame: Rc<Cell<bool>>,
    commit_focus_every_frame: bool,
    commit_calls: Rc<Cell<usize>>,
}

struct RejectedCommitPhaseFocusView {
    invalid_focus: FocusHandle,
    commit_calls: Rc<Cell<usize>>,
    outcomes: Rc<RefCell<Vec<FocusClaimOutcome>>>,
    subscriptions: Rc<RefCell<Vec<Subscription>>>,
}

struct SealedFocusRoundTripView {
    committed_focus: FocusHandle,
    transient_focus: FocusHandle,
    commit_calls: Rc<Cell<usize>>,
}

struct FocusPhaseRoundTripView {
    committed_focus: FocusHandle,
    transient_focus: FocusHandle,
    frame_commits: Rc<Cell<usize>>,
}

struct FocusPhaseInvalidationView {
    committed_focus: FocusHandle,
    transient_focus: FocusHandle,
    revision: usize,
    rendered_revision: Rc<Cell<usize>>,
}

struct CandidateFocusQueryView {
    committed_focus: FocusHandle,
    candidate_focus: FocusHandle,
    claim_candidate: Rc<Cell<bool>>,
    observed: Rc<RefCell<Option<(Option<FocusHandle>, Option<FocusHandle>)>>>,
}

struct AlternatingRejectedFocusView {
    first_invalid_focus: FocusHandle,
    second_invalid_focus: FocusHandle,
    commit_calls: Rc<Cell<usize>>,
}

struct FreshRejectedFocusView {
    retained_invalid_focuses: Rc<RefCell<Vec<FocusHandle>>>,
    commit_calls: Rc<Cell<usize>>,
}

struct CommitPhaseBlurView {
    focus: FocusHandle,
    blur_on_next_frame: Rc<Cell<bool>>,
    blur_every_frame: bool,
    commit_calls: Rc<Cell<usize>>,
}

struct FocusCompletionDropProbe(Rc<Cell<usize>>);

impl Drop for FocusCompletionDropProbe {
    fn drop(&mut self) {
        self.0.set(self.0.get() + 1);
    }
}

struct RolledBackFocusClaimView {
    committed_focus: FocusHandle,
    rejected_focus: FocusHandle,
}

struct FocusCompletionTransactionView {
    committed_focus: FocusHandle,
    rejected_focus: FocusHandle,
    attempt_rollback: bool,
    rejected_outcomes: Rc<RefCell<Vec<FocusClaimOutcome>>>,
    rejected_subscriptions: Rc<RefCell<Vec<Subscription>>>,
    rejected_callback_drops: Rc<Cell<usize>>,
}

struct RolledBackFocusCompletion {
    focus: FocusHandle,
    outcomes: Rc<RefCell<Vec<FocusClaimOutcome>>>,
    subscriptions: Rc<RefCell<Vec<Subscription>>>,
    callback_drops: Rc<Cell<usize>>,
}

struct RolledBackFocusElement {
    child: Option<AnyElement>,
    completion: Option<RolledBackFocusCompletion>,
}

impl IntoElement for RolledBackFocusElement {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for RolledBackFocusElement {
    type RequestLayoutState = AnyElement;
    type PrepaintState = ();

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let mut child = self.child.take().expect("focus test child requested once");
        let layout_id = child.request_layout(window, cx);
        (layout_id, child)
    }

    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        child: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        let rejected: Result<(), ()> = window.transact(|window| {
            child.prepaint(window, cx);
            if let Some(completion) = self.completion.as_ref() {
                let outcomes = completion.outcomes.clone();
                let drop_probe = FocusCompletionDropProbe(completion.callback_drops.clone());
                let subscription =
                    window.focus_with_completion(&completion.focus, cx, move |outcome, _, _| {
                        let _drop_probe = drop_probe;
                        outcomes.borrow_mut().push(outcome)
                    });
                completion.subscriptions.borrow_mut().push(subscription);
            }
            Err(())
        });
        debug_assert!(rejected.is_err());
    }

    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        _: &mut Self::RequestLayoutState,
        _: &mut Self::PrepaintState,
        _: &mut Window,
        _: &mut App,
    ) {
    }
}

impl Render for RolledBackFocusClaimView {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let rejected_focus = self.rejected_focus.clone();
        div()
            .flex()
            .child(
                div()
                    .id("committed-focus-before-rollback")
                    .w(px(80.0))
                    .h(px(40.0))
                    .focusable()
                    .track_focus(&self.committed_focus),
            )
            .child(RolledBackFocusElement {
                child: Some(
                    div()
                        .id("rejected-focus-in-transaction")
                        .w(px(80.0))
                        .h(px(40.0))
                        .focusable()
                        .track_focus(&rejected_focus)
                        .into_any_element(),
                ),
                completion: None,
            })
    }
}

impl Render for FocusCompletionTransactionView {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let rejected_focus = self.rejected_focus.clone();
        let completion = self.attempt_rollback.then(|| RolledBackFocusCompletion {
            focus: rejected_focus.clone(),
            outcomes: self.rejected_outcomes.clone(),
            subscriptions: self.rejected_subscriptions.clone(),
            callback_drops: self.rejected_callback_drops.clone(),
        });
        div()
            .flex()
            .child(
                div()
                    .id("focus-completion-before-rollback")
                    .w(px(80.0))
                    .h(px(40.0))
                    .focusable()
                    .track_focus(&self.committed_focus),
            )
            .child(RolledBackFocusElement {
                child: Some(
                    div()
                        .id("focus-completion-rejected-in-transaction")
                        .w(px(80.0))
                        .h(px(40.0))
                        .focusable()
                        .track_focus(&rejected_focus)
                        .into_any_element(),
                ),
                completion,
            })
    }
}

impl Render for SuppressedFocusMutationView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let suppressed_focus = self.suppressed_focus.clone();
        div()
            .flex()
            .child(
                div()
                    .id("suppressed-focus-sibling")
                    .w(px(80.0))
                    .h(px(40.0))
                    .focusable()
                    .track_focus(&self.sibling_focus),
            )
            .child(
                canvas(
                    move |_, window, cx| {
                        suppressed_focus.focus(window, cx);
                        window.blur(cx);
                    },
                    |_, _, _, _| {},
                )
                .w(px(80.0))
                .h(px(40.0))
                .with_subtree_presentation(SubtreePresentation::Inert),
            )
    }
}

impl Render for SuppressedFocusControlView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let scope_focus = self.scope_focus.clone();
        let target_focus = self.target_focus.clone();
        let run_controls = self.run_controls.clone();
        let traversal_results = self.traversal_results.clone();
        let blur_outcomes = self.blur_outcomes.clone();
        let blur_subscriptions = self.blur_subscriptions.clone();
        div()
            .child(
                div()
                    .id("suppressed-focus-control-scope")
                    .focusable()
                    .track_focus(&self.scope_focus)
                    .child(
                        div()
                            .id("suppressed-focus-control-target")
                            .tab_index(0)
                            .track_focus(&self.target_focus),
                    ),
            )
            .child(
                canvas(
                    move |_, window, cx| {
                        if !run_controls.get() {
                            return;
                        }
                        let next = window.focus_next_where_within(
                            &scope_focus,
                            |candidate| candidate == &target_focus,
                            cx,
                        );
                        let previous = window.focus_prev_where_within(
                            &scope_focus,
                            |candidate| candidate == &target_focus,
                            cx,
                        );
                        traversal_results.set(Some((next, previous)));
                        let blur_outcomes = blur_outcomes.clone();
                        let subscription = window.blur_with_completion(cx, move |outcome, _, _| {
                            blur_outcomes.borrow_mut().push(outcome);
                        });
                        blur_subscriptions.borrow_mut().push(subscription);
                        window.disable_focus(cx);
                    },
                    |_, _, _, _| {},
                )
                .with_subtree_presentation(SubtreePresentation::Inert),
            )
    }
}

impl Render for FocusCompletionTreeView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .child(
                div()
                    .id("focus-completion-root")
                    .w(px(80.0))
                    .h(px(40.0))
                    .focusable()
                    .track_focus(&self.root_focus)
                    .child(
                        div()
                            .id("focus-completion-child")
                            .w(px(40.0))
                            .h(px(20.0))
                            .focusable()
                            .track_focus(&self.child_focus),
                    ),
            )
            .child(
                div()
                    .id("focus-completion-sibling")
                    .w(px(80.0))
                    .h(px(40.0))
                    .focusable()
                    .track_focus(&self.sibling_focus),
            )
    }
}

impl Render for FocusHandleTransitionView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div().children(self.focus.as_ref().map(|focus| {
            div()
                .id("focus-handle-transition-target")
                .w(px(80.0))
                .h(px(40.0))
                .focusable()
                .track_focus(focus)
        }))
    }
}

impl Render for CommitPhaseFocusView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let commit_focus_on_next_frame = self.commit_focus_on_next_frame.clone();
        let commit_focus_every_frame = self.commit_focus_every_frame;
        let commit_calls = self.commit_calls.clone();
        let committed_focus = self.committed_focus.clone();
        div()
            .flex()
            .child(
                div()
                    .id("commit-phase-first-focus")
                    .w(px(80.0))
                    .h(px(40.0))
                    .focusable()
                    .track_focus(&self.first_focus),
            )
            .child(
                div()
                    .id("commit-phase-final-focus")
                    .w(px(80.0))
                    .h(px(40.0))
                    .focusable()
                    .track_focus(&self.committed_focus),
            )
            .child(canvas(
                move |_, window, _| {
                    if commit_focus_every_frame || commit_focus_on_next_frame.replace(false) {
                        let committed_focus = committed_focus.clone();
                        let commit_calls = commit_calls.clone();
                        window.record_prepaint_window_commit(move |_, window, cx| {
                            commit_calls.set(commit_calls.get() + 1);
                            committed_focus.focus(window, cx);
                        });
                    }
                },
                |_, _, _, _| {},
            ))
    }
}

impl Render for RejectedCommitPhaseFocusView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let invalid_focus = self.invalid_focus.clone();
        let commit_calls = self.commit_calls.clone();
        let outcomes = self.outcomes.clone();
        let subscriptions = self.subscriptions.clone();
        canvas(
            move |_, window, _| {
                let invalid_focus = invalid_focus.clone();
                let commit_calls = commit_calls.clone();
                let outcomes = outcomes.clone();
                let subscriptions = subscriptions.clone();
                window.record_prepaint_window_commit(move |_, window, cx| {
                    let attempt = commit_calls.get() + 1;
                    commit_calls.set(attempt);
                    if attempt == 1 {
                        let outcomes = outcomes.clone();
                        let subscription = window.focus_with_completion(
                            &invalid_focus,
                            cx,
                            move |outcome, _, _| outcomes.borrow_mut().push(outcome),
                        );
                        subscriptions.borrow_mut().push(subscription);
                    } else if attempt <= 3 {
                        invalid_focus.focus(window, cx);
                    }
                });
            },
            |_, _, _, _| {},
        )
    }
}

impl Render for SealedFocusRoundTripView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let committed_focus = self.committed_focus.clone();
        let transient_focus = self.transient_focus.clone();
        let commit_calls = self.commit_calls.clone();
        div()
            .child(
                div()
                    .id("sealed-round-trip-committed")
                    .focusable()
                    .track_focus(&self.committed_focus),
            )
            .child(
                div()
                    .id("sealed-round-trip-transient")
                    .focusable()
                    .track_focus(&self.transient_focus),
            )
            .child(canvas(
                move |_, window, _| {
                    let committed_focus = committed_focus.clone();
                    let transient_focus = transient_focus.clone();
                    let commit_calls = commit_calls.clone();
                    window.record_prepaint_window_commit(move |_, window, cx| {
                        let attempt = commit_calls.get() + 1;
                        commit_calls.set(attempt);
                        if attempt <= 3 {
                            transient_focus.focus(window, cx);
                            committed_focus.focus(window, cx);
                        }
                    });
                },
                |_, _, _, _| {},
            ))
    }
}

impl Render for FocusPhaseRoundTripView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let frame_commits = self.frame_commits.clone();
        div()
            .child(
                div()
                    .id("focus-phase-round-trip-committed")
                    .focusable()
                    .track_focus(&self.committed_focus),
            )
            .child(
                div()
                    .id("focus-phase-round-trip-transient")
                    .focusable()
                    .track_focus(&self.transient_focus),
            )
            .child(canvas(
                move |_, window, _| {
                    let frame_commits = frame_commits.clone();
                    window.record_prepaint_window_commit(move |_, _, _| {
                        frame_commits.set(frame_commits.get() + 1);
                    });
                },
                |_, _, _, _| {},
            ))
    }
}

impl Render for FocusPhaseInvalidationView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        self.rendered_revision.set(self.revision);
        div()
            .child(
                div()
                    .id("focus-phase-invalidation-committed")
                    .focusable()
                    .track_focus(&self.committed_focus),
            )
            .child(
                div()
                    .id("focus-phase-invalidation-transient")
                    .focusable()
                    .track_focus(&self.transient_focus),
            )
    }
}

impl Render for CandidateFocusQueryView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let candidate_focus = self.candidate_focus.clone();
        let claim_candidate = self.claim_candidate.clone();
        let observed = self.observed.clone();
        div()
            .child(
                div()
                    .id("committed-focus-query-target")
                    .focusable()
                    .track_focus(&self.committed_focus),
            )
            .child(
                div()
                    .id("candidate-focus-query-target")
                    .focusable()
                    .track_focus(&self.candidate_focus),
            )
            .child(canvas(
                move |_, window, cx| {
                    if !claim_candidate.replace(false) {
                        return;
                    }
                    candidate_focus.focus(window, cx);
                    observed
                        .borrow_mut()
                        .replace((window.focused(cx), window.committed_focus(cx)));
                },
                |_, _, _, _| {},
            ))
    }
}

impl Render for AlternatingRejectedFocusView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let first_invalid_focus = self.first_invalid_focus.clone();
        let second_invalid_focus = self.second_invalid_focus.clone();
        let commit_calls = self.commit_calls.clone();
        canvas(
            move |_, window, _| {
                let first_invalid_focus = first_invalid_focus.clone();
                let second_invalid_focus = second_invalid_focus.clone();
                let commit_calls = commit_calls.clone();
                window.record_prepaint_window_commit(move |_, window, cx| {
                    let attempt = commit_calls.get() + 1;
                    commit_calls.set(attempt);
                    match attempt {
                        1 | 3 => first_invalid_focus.focus(window, cx),
                        2 => second_invalid_focus.focus(window, cx),
                        _ => {}
                    }
                });
            },
            |_, _, _, _| {},
        )
    }
}

impl Render for FreshRejectedFocusView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let retained_invalid_focuses = self.retained_invalid_focuses.clone();
        let commit_calls = self.commit_calls.clone();
        canvas(
            move |_, window, _| {
                let retained_invalid_focuses = retained_invalid_focuses.clone();
                let commit_calls = commit_calls.clone();
                window.record_prepaint_window_commit(move |_, window, cx| {
                    let invalid_focus = cx.focus_handle();
                    // Keep the target alive until its candidate frame rejects it.
                    retained_invalid_focuses
                        .borrow_mut()
                        .push(invalid_focus.clone());
                    commit_calls.set(commit_calls.get() + 1);
                    invalid_focus.focus(window, cx);
                });
            },
            |_, _, _, _| {},
        )
    }
}

impl Render for CommitPhaseBlurView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let blur_on_next_frame = self.blur_on_next_frame.clone();
        let blur_every_frame = self.blur_every_frame;
        let commit_calls = self.commit_calls.clone();
        div()
            .child(
                div()
                    .id("commit-phase-blur-focus")
                    .w(px(80.0))
                    .h(px(40.0))
                    .focusable()
                    .track_focus(&self.focus),
            )
            .child(canvas(
                move |_, window, _| {
                    if blur_every_frame || blur_on_next_frame.replace(false) {
                        let commit_calls = commit_calls.clone();
                        window.record_prepaint_window_commit(move |_, window, cx| {
                            commit_calls.set(commit_calls.get() + 1);
                            window.blur(cx);
                        });
                    }
                },
                |_, _, _, _| {},
            ))
    }
}

#[open_gpui::test]
fn raw_focus_and_blur_are_noops_inside_suppressed_prepaint(cx: &mut TestAppContext) {
    let typed_window = cx.open_window(size(px(320.0), px(200.0)), |_, cx| {
        SuppressedFocusMutationView {
            sibling_focus: cx.focus_handle(),
            suppressed_focus: cx.focus_handle(),
        }
    });
    let view = typed_window.root(cx).unwrap();
    let window = typed_window.into();
    cx.run_until_parked();

    let sibling_focus = cx.read(|cx| view.read(cx).sibling_focus.clone());
    let focus_outs = Rc::new(Cell::new(0));
    let _subscription = cx
        .update_window(window, |_, window, cx| {
            let focus_outs = focus_outs.clone();
            window.on_focus_out(&sibling_focus, cx, move |_, _, _| {
                focus_outs.set(focus_outs.get() + 1)
            })
        })
        .unwrap();
    cx.run_until_parked();
    cx.update_window(window, |_, window, cx| sibling_focus.focus(window, cx))
        .unwrap();
    cx.run_until_parked();
    let revision = cx
        .update_window(window, |_, window, _| window.focus_claim_revision())
        .unwrap();

    view.update(cx, |_, cx| cx.notify());
    cx.run_until_parked();

    cx.update_window(window, |_, window, cx| {
        assert_eq!(window.focused(cx).as_ref(), Some(&sibling_focus));
        assert_eq!(window.focus_claim_revision(), revision);
    })
    .unwrap();
    assert_eq!(focus_outs.get(), 0);
}

#[open_gpui::test]
fn suppressed_focus_controls_fail_closed_without_disabling_the_window(cx: &mut TestAppContext) {
    let run_controls = Rc::new(Cell::new(false));
    let traversal_results = Rc::new(Cell::new(None));
    let blur_outcomes = Rc::new(RefCell::new(Vec::new()));
    let blur_subscriptions = Rc::new(RefCell::new(Vec::new()));
    let typed_window = cx.open_window(size(px(320.0), px(200.0)), {
        let run_controls = run_controls.clone();
        let traversal_results = traversal_results.clone();
        let blur_outcomes = blur_outcomes.clone();
        let blur_subscriptions = blur_subscriptions.clone();
        move |_, cx| SuppressedFocusControlView {
            scope_focus: cx.focus_handle(),
            target_focus: cx.focus_handle(),
            run_controls,
            traversal_results,
            blur_outcomes,
            blur_subscriptions,
        }
    });
    let view = typed_window.root(cx).unwrap();
    let window = typed_window.into();
    cx.run_until_parked();

    traversal_results.set(None);
    run_controls.set(true);
    view.update(cx, |_, cx| cx.notify());
    cx.run_until_parked();
    assert_eq!(
        traversal_results.get(),
        Some((false, false)),
        "suppressed traversal must not report a focus transfer that was rejected"
    );
    assert_eq!(
        blur_outcomes.borrow().as_slice(),
        &[FocusClaimOutcome::Rejected],
        "suppressed blur completion must fail closed"
    );
    assert_eq!(blur_subscriptions.borrow().len(), 1);

    let target_focus = cx.read(|cx| view.read(cx).target_focus.clone());
    cx.update_window(window, |_, window, cx| target_focus.focus(window, cx))
        .unwrap();
    cx.run_until_parked();
    cx.update_window(window, |_, window, cx| {
        assert_eq!(window.focused(cx).as_ref(), Some(&target_focus));
    })
    .unwrap();
}

#[open_gpui::test]
fn committed_focus_observers_are_independent_of_platform_activation(cx: &mut TestAppContext) {
    let typed_window = cx.open_window(size(px(320.0), px(200.0)), |_, cx| {
        SuppressedFocusMutationView {
            sibling_focus: cx.focus_handle(),
            suppressed_focus: cx.focus_handle(),
        }
    });
    let view = typed_window.root(cx).unwrap();
    let window = typed_window.into();
    cx.run_until_parked();

    let focus = cx.read(|cx| view.read(cx).sibling_focus.clone());
    let committed_focus_events = Rc::new(Cell::new(0));
    let active_focus_events = Rc::new(Cell::new(0));
    let completion_outcomes = Rc::new(RefCell::new(Vec::new()));
    let subscriptions = cx
        .update_window(window, |_, window, cx| {
            assert!(!window.is_window_active());
            let committed_focus_events_for_listener = committed_focus_events.clone();
            let committed = window.on_focus_committed(&focus, cx, move |_, _| {
                committed_focus_events_for_listener
                    .set(committed_focus_events_for_listener.get() + 1);
            });
            let active_focus_events_for_listener = active_focus_events.clone();
            let active = window.on_focus_in(&focus, cx, move |_, _| {
                active_focus_events_for_listener.set(active_focus_events_for_listener.get() + 1);
            });
            (committed, active)
        })
        .unwrap();
    cx.run_until_parked();

    let completion_subscription = cx
        .update_window(window, |_, window, cx| {
            let completion_outcomes = completion_outcomes.clone();
            window.focus_with_completion(&focus, cx, move |outcome, _, _| {
                completion_outcomes.borrow_mut().push(outcome);
            })
        })
        .unwrap();
    cx.run_until_parked();

    cx.update_window(window, |_, window, cx| {
        assert_eq!(window.focused(cx).as_ref(), Some(&focus));
        assert!(!window.is_window_active());
    })
    .unwrap();
    assert_eq!(committed_focus_events.get(), 1);
    assert_eq!(active_focus_events.get(), 0);
    assert_eq!(
        completion_outcomes.borrow().as_slice(),
        &[FocusClaimOutcome::Committed]
    );

    cx.update_window(window, |_, window, _| window.activate_window())
        .unwrap();
    cx.run_until_parked();

    cx.update_window(window, |_, window, _| {
        assert!(window.is_window_active());
    })
    .unwrap();
    assert_eq!(
        committed_focus_events.get(),
        1,
        "platform activation must not replay an already-committed local focus event"
    );
    assert_eq!(active_focus_events.get(), 1);
    assert_eq!(
        completion_outcomes.borrow().as_slice(),
        &[FocusClaimOutcome::Committed],
        "platform activation must not replay a completed focus claim"
    );
    drop(completion_subscription);
    drop(subscriptions);
}

#[open_gpui::test]
fn committed_focus_observers_distinguish_exact_and_descendant_focus(cx: &mut TestAppContext) {
    let typed_window = cx.open_window(size(px(320.0), px(200.0)), |_, cx| {
        FocusCompletionTreeView {
            root_focus: cx.focus_handle(),
            child_focus: cx.focus_handle(),
            sibling_focus: cx.focus_handle(),
        }
    });
    let view = typed_window.root(cx).unwrap();
    let window = typed_window.into();
    cx.run_until_parked();

    let (root_focus, child_focus) = cx.read(|cx| {
        let view = view.read(cx);
        (view.root_focus.clone(), view.child_focus.clone())
    });
    let exact_root = Rc::new(Cell::new(0));
    let committed_in_root = Rc::new(Cell::new(0));
    let exact_child = Rc::new(Cell::new(0));
    let subscriptions = cx
        .update_window(window, |_, window, cx| {
            let exact_root_events = exact_root.clone();
            let exact_root = window.on_focus_committed(&root_focus, cx, move |_, _| {
                exact_root_events.set(exact_root_events.get() + 1);
            });
            let committed_in_root_events = committed_in_root.clone();
            let committed_in = window.on_focus_committed_in(&root_focus, cx, move |_, _| {
                committed_in_root_events.set(committed_in_root_events.get() + 1);
            });
            let exact_child_events = exact_child.clone();
            let exact_child = window.on_focus_committed(&child_focus, cx, move |_, _| {
                exact_child_events.set(exact_child_events.get() + 1);
            });
            (exact_root, committed_in, exact_child)
        })
        .unwrap();

    cx.update_window(window, |_, window, cx| child_focus.focus(window, cx))
        .unwrap();
    cx.run_until_parked();

    assert_eq!(exact_root.get(), 0);
    assert_eq!(committed_in_root.get(), 1);
    assert_eq!(exact_child.get(), 1);
    drop(subscriptions);
}

#[open_gpui::test]
fn committed_focus_query_excludes_candidate_frame_intent(cx: &mut TestAppContext) {
    let claim_candidate = Rc::new(Cell::new(false));
    let observed = Rc::new(RefCell::new(None));
    let typed_window = cx.open_window(size(px(320.0), px(200.0)), {
        let claim_candidate = claim_candidate.clone();
        let observed = observed.clone();
        move |_, cx| CandidateFocusQueryView {
            committed_focus: cx.focus_handle(),
            candidate_focus: cx.focus_handle(),
            claim_candidate,
            observed,
        }
    });
    let view = typed_window.root(cx).unwrap();
    let window = typed_window.into();
    cx.run_until_parked();

    let (committed_focus, candidate_focus) = cx.read(|cx| {
        let view = view.read(cx);
        (view.committed_focus.clone(), view.candidate_focus.clone())
    });
    cx.update_window(window, |_, window, cx| {
        committed_focus.focus(window, cx);
    })
    .unwrap();
    cx.run_until_parked();

    claim_candidate.set(true);
    view.update(cx, |_, cx| cx.notify());
    cx.run_until_parked();
    assert_eq!(
        observed.borrow().clone(),
        Some((Some(candidate_focus.clone()), Some(committed_focus))),
        "candidate intent must not leak through the committed focus query"
    );
    cx.update_window(window, |_, window, cx| {
        assert_eq!(window.committed_focus(cx).as_ref(), Some(&candidate_focus));
    })
    .unwrap();
}

#[open_gpui::test]
fn dropping_previous_focus_preserves_a_new_unbound_claim(cx: &mut TestAppContext) {
    let typed_window = cx.open_window(size(px(320.0), px(200.0)), |_, cx| {
        FocusHandleTransitionView {
            focus: Some(cx.focus_handle()),
        }
    });
    let view = typed_window.root(cx).unwrap();
    let window = typed_window.into();
    cx.run_until_parked();

    let previous_focus = cx.read(|cx| {
        view.read(cx)
            .focus
            .as_ref()
            .expect("initial focus handle must exist")
            .clone()
    });
    cx.update_window(window, |_, window, cx| previous_focus.focus(window, cx))
        .unwrap();
    cx.run_until_parked();
    drop(previous_focus);

    let outcomes = Rc::new(RefCell::new(Vec::new()));
    let (next_focus, subscription) = cx
        .update_window(window, |_, window, cx| {
            let next_focus = view.update(cx, |view, cx| {
                let next_focus = cx.focus_handle();
                view.focus = Some(next_focus.clone());
                cx.notify();
                next_focus
            });
            let outcomes_for_completion = outcomes.clone();
            let subscription =
                window.focus_with_completion(&next_focus, cx, move |outcome, _, _| {
                    outcomes_for_completion.borrow_mut().push(outcome);
                });
            (next_focus, subscription)
        })
        .unwrap();
    cx.run_until_parked();

    assert_eq!(
        outcomes.borrow().as_slice(),
        &[FocusClaimOutcome::Committed]
    );
    cx.update_window(window, |_, window, cx| {
        assert_eq!(window.focused(cx).as_ref(), Some(&next_focus));
        assert_eq!(window.retained_focus_claim_count_for_test(), 0);
    })
    .unwrap();
    drop(subscription);
}

#[open_gpui::test]
fn reasserting_committed_focus_wins_over_an_earlier_same_update_claim(cx: &mut TestAppContext) {
    let typed_window = cx.open_window(size(px(320.0), px(200.0)), |_, cx| {
        FocusCompletionTreeView {
            root_focus: cx.focus_handle(),
            child_focus: cx.focus_handle(),
            sibling_focus: cx.focus_handle(),
        }
    });
    let view = typed_window.root(cx).unwrap();
    let window = typed_window.into();
    cx.run_until_parked();

    let (committed_focus, competing_focus) = cx.read(|cx| {
        let view = view.read(cx);
        (view.root_focus.clone(), view.sibling_focus.clone())
    });
    cx.update_window(window, |_, window, cx| committed_focus.focus(window, cx))
        .unwrap();
    cx.run_until_parked();

    let outcomes = Rc::new(RefCell::new(Vec::new()));
    let subscription = cx
        .update_window(window, |_, window, cx| {
            competing_focus.focus(window, cx);
            let outcomes_for_completion = outcomes.clone();
            window.focus_with_completion(&committed_focus, cx, move |outcome, _, _| {
                outcomes_for_completion.borrow_mut().push(outcome);
            })
        })
        .unwrap();
    cx.run_until_parked();

    assert_eq!(
        outcomes.borrow().as_slice(),
        &[FocusClaimOutcome::Committed]
    );
    cx.update_window(window, |_, window, cx| {
        assert_eq!(
            window.focused(cx).as_ref(),
            Some(&committed_focus),
            "the final focus request in one update must win even when an older frame already committed it"
        );
    })
    .unwrap();
    drop(subscription);
}

#[open_gpui::test]
fn later_focus_and_blur_supersede_pending_focus_completions(cx: &mut TestAppContext) {
    let typed_window = cx.open_window(size(px(320.0), px(200.0)), |_, cx| {
        FocusCompletionTreeView {
            root_focus: cx.focus_handle(),
            child_focus: cx.focus_handle(),
            sibling_focus: cx.focus_handle(),
        }
    });
    let view = typed_window.root(cx).unwrap();
    let window = typed_window.into();
    cx.run_until_parked();

    let (root_focus, sibling_focus) = cx.read(|cx| {
        let view = view.read(cx);
        (view.root_focus.clone(), view.sibling_focus.clone())
    });
    let outcomes = Rc::new(RefCell::new(Vec::new()));
    let focus_subscription = cx
        .update_window(window, |_, window, cx| {
            let outcomes = outcomes.clone();
            let subscription =
                window.focus_with_completion(&root_focus, cx, move |outcome, _, _| {
                    outcomes.borrow_mut().push(outcome);
                });
            sibling_focus.focus(window, cx);
            subscription
        })
        .unwrap();
    cx.run_until_parked();
    assert_eq!(
        outcomes.borrow().as_slice(),
        &[FocusClaimOutcome::Superseded]
    );
    cx.update_window(window, |_, window, cx| {
        assert_eq!(window.focused(cx).as_ref(), Some(&sibling_focus));
    })
    .unwrap();

    let blur_subscription = cx
        .update_window(window, |_, window, cx| {
            let outcomes = outcomes.clone();
            let subscription =
                window.focus_with_completion(&root_focus, cx, move |outcome, _, _| {
                    outcomes.borrow_mut().push(outcome);
                });
            window.blur(cx);
            subscription
        })
        .unwrap();
    cx.run_until_parked();
    assert_eq!(
        outcomes.borrow().as_slice(),
        &[FocusClaimOutcome::Superseded, FocusClaimOutcome::Superseded]
    );
    cx.update_window(window, |_, window, cx| {
        assert!(window.focused(cx).is_none());
    })
    .unwrap();
    drop((focus_subscription, blur_subscription));
}

#[open_gpui::test]
fn blur_completion_tracks_empty_focus_and_later_focus_supersession(cx: &mut TestAppContext) {
    let typed_window = cx.open_window(size(px(320.0), px(200.0)), |_, cx| {
        FocusCompletionTreeView {
            root_focus: cx.focus_handle(),
            child_focus: cx.focus_handle(),
            sibling_focus: cx.focus_handle(),
        }
    });
    let view = typed_window.root(cx).unwrap();
    let window = typed_window.into();
    cx.run_until_parked();

    let (root_focus, sibling_focus) = cx.read(|cx| {
        let view = view.read(cx);
        (view.root_focus.clone(), view.sibling_focus.clone())
    });
    cx.update_window(window, |_, window, cx| root_focus.focus(window, cx))
        .unwrap();
    cx.run_until_parked();

    let outcomes = Rc::new(RefCell::new(Vec::new()));
    let committed_blur = cx
        .update_window(window, |_, window, cx| {
            let outcomes = outcomes.clone();
            window.blur_with_completion(cx, move |outcome, _, _| {
                outcomes.borrow_mut().push(outcome);
            })
        })
        .unwrap();
    cx.run_until_parked();
    assert_eq!(
        outcomes.borrow().as_slice(),
        &[FocusClaimOutcome::Committed]
    );
    cx.update_window(window, |_, window, cx| {
        assert!(window.focused(cx).is_none());
    })
    .unwrap();

    cx.update_window(window, |_, window, cx| root_focus.focus(window, cx))
        .unwrap();
    cx.run_until_parked();
    let superseded_blur = cx
        .update_window(window, |_, window, cx| {
            let outcomes = outcomes.clone();
            let subscription = window.blur_with_completion(cx, move |outcome, _, _| {
                outcomes.borrow_mut().push(outcome);
            });
            sibling_focus.focus(window, cx);
            subscription
        })
        .unwrap();
    cx.run_until_parked();
    assert_eq!(
        outcomes.borrow().as_slice(),
        &[FocusClaimOutcome::Committed, FocusClaimOutcome::Superseded,]
    );
    cx.update_window(window, |_, window, cx| {
        assert_eq!(window.focused(cx).as_ref(), Some(&sibling_focus));
    })
    .unwrap();
    drop((committed_blur, superseded_blur));
}

#[open_gpui::test]
fn dropping_focus_completion_subscription_only_cancels_observation(cx: &mut TestAppContext) {
    let typed_window = cx.open_window(size(px(320.0), px(200.0)), |_, cx| {
        FocusCompletionTreeView {
            root_focus: cx.focus_handle(),
            child_focus: cx.focus_handle(),
            sibling_focus: cx.focus_handle(),
        }
    });
    let view = typed_window.root(cx).unwrap();
    let window = typed_window.into();
    cx.run_until_parked();

    let root_focus = cx.read(|cx| view.read(cx).root_focus.clone());
    let outcomes = Rc::new(RefCell::new(Vec::new()));
    let subscription = cx
        .update_window(window, |_, window, cx| {
            let outcomes = outcomes.clone();
            window.focus_with_completion(&root_focus, cx, move |outcome, _, _| {
                outcomes.borrow_mut().push(outcome);
            })
        })
        .unwrap();
    drop(subscription);
    cx.run_until_parked();

    cx.update_window(window, |_, window, cx| {
        assert_eq!(window.focused(cx).as_ref(), Some(&root_focus));
    })
    .unwrap();
    assert!(outcomes.borrow().is_empty());
}

#[open_gpui::test]
fn closing_window_from_focus_completion_discards_later_resolutions(cx: &mut TestAppContext) {
    let typed_window = cx.open_window(size(px(320.0), px(200.0)), |_, cx| {
        FocusCompletionTreeView {
            root_focus: cx.focus_handle(),
            child_focus: cx.focus_handle(),
            sibling_focus: cx.focus_handle(),
        }
    });
    let view = typed_window.root(cx).unwrap();
    let window = typed_window.into();
    cx.run_until_parked();

    let root_focus = cx.read(|cx| view.read(cx).root_focus.clone());
    cx.update_window(window, |_, window, cx| root_focus.focus(window, cx))
        .unwrap();
    cx.run_until_parked();

    let first_calls = Rc::new(Cell::new(0));
    let second_calls = Rc::new(Cell::new(0));
    let second_callback_drops = Rc::new(Cell::new(0));
    let subscriptions = cx
        .update_window(window, |_, window, cx| {
            let first_calls_for_completion = first_calls.clone();
            let first = window.focus_with_completion(&root_focus, cx, move |_, window, cx| {
                first_calls_for_completion.set(first_calls_for_completion.get() + 1);
                window.remove_window(cx);
            });
            let second_calls_for_completion = second_calls.clone();
            let drop_probe = FocusCompletionDropProbe(second_callback_drops.clone());
            let second = window.focus_with_completion(&root_focus, cx, move |_, _, _| {
                let _drop_probe = drop_probe;
                second_calls_for_completion.set(second_calls_for_completion.get() + 1);
            });
            (first, second)
        })
        .unwrap();
    cx.run_until_parked();

    assert_eq!(first_calls.get(), 1);
    assert_eq!(
        second_calls.get(),
        0,
        "closing a window must discard later callbacks already moved into the dispatch batch"
    );
    assert_eq!(
        second_callback_drops.get(),
        1,
        "window close must release discarded callback captures while its subscription is retained"
    );
    drop(subscriptions);
}

#[open_gpui::test]
fn sealed_commit_focus_defers_and_preserves_the_final_fallback(cx: &mut TestAppContext) {
    let commit_focus_on_next_frame = Rc::new(Cell::new(false));
    let commit_calls = Rc::new(Cell::new(0));
    let typed_window = cx.open_window(size(px(320.0), px(200.0)), {
        let commit_focus_on_next_frame = commit_focus_on_next_frame.clone();
        let commit_calls = commit_calls.clone();
        move |_, cx| CommitPhaseFocusView {
            first_focus: cx.focus_handle(),
            committed_focus: cx.focus_handle(),
            commit_focus_on_next_frame,
            commit_focus_every_frame: false,
            commit_calls,
        }
    });
    let view = typed_window.root(cx).unwrap();
    let window = typed_window.into();
    cx.run_until_parked();

    let (first_focus, committed_focus) = cx.read(|cx| {
        let view = view.read(cx);
        (view.first_focus.clone(), view.committed_focus.clone())
    });
    cx.update_window(window, |_, window, cx| first_focus.focus(window, cx))
        .unwrap();
    cx.run_until_parked();

    let rejected_focus = cx.read(|cx| cx.focus_handle());
    let rejected_outcomes = Rc::new(RefCell::new(Vec::new()));
    let rejected_subscriptions = Rc::new(RefCell::new(Vec::new()));
    let observer = cx
        .update_window(window, |_, window, cx| {
            let rejected_focus = rejected_focus.clone();
            let rejected_outcomes = rejected_outcomes.clone();
            let rejected_subscriptions = rejected_subscriptions.clone();
            window.on_focus_committed(&committed_focus, cx, move |window, cx| {
                let rejected_outcomes = rejected_outcomes.clone();
                let subscription =
                    window.focus_with_completion(&rejected_focus, cx, move |outcome, _, _| {
                        rejected_outcomes.borrow_mut().push(outcome)
                    });
                rejected_subscriptions.borrow_mut().push(subscription);
            })
        })
        .unwrap();

    commit_focus_on_next_frame.set(true);
    view.update(cx, |_, cx| cx.notify());
    cx.run_until_parked();
    draw_focus_followup_frame(cx, window);
    draw_focus_followup_frame(cx, window);

    assert_eq!(commit_calls.get(), 1);
    assert_eq!(
        rejected_outcomes.borrow().as_slice(),
        &[FocusClaimOutcome::Rejected]
    );
    assert_eq!(rejected_subscriptions.borrow().len(), 1);
    cx.update_window(window, |_, window, cx| {
        assert_eq!(
            window.focused(cx).as_ref(),
            Some(&committed_focus),
            "a rejected later claim must fall back to the focus committed after the sealed phase"
        );
        assert_eq!(window.retained_focus_claim_count_for_test(), 0);
    })
    .unwrap();
    drop(observer);
    rejected_subscriptions.borrow_mut().clear();
}

#[open_gpui::test]
fn sealed_commit_reasserting_current_focus_does_not_redraw_forever(cx: &mut TestAppContext) {
    let commit_calls = Rc::new(Cell::new(0));
    let typed_window = cx.open_window(size(px(320.0), px(200.0)), {
        let commit_calls = commit_calls.clone();
        move |_, cx| CommitPhaseFocusView {
            first_focus: cx.focus_handle(),
            committed_focus: cx.focus_handle(),
            commit_focus_on_next_frame: Rc::new(Cell::new(false)),
            commit_focus_every_frame: true,
            commit_calls,
        }
    });
    let view = typed_window.root(cx).unwrap();
    let window = typed_window.into();
    cx.run_until_parked();
    draw_focus_followup_frame(cx, window);

    let committed_focus = cx.read(|cx| view.read(cx).committed_focus.clone());
    assert_eq!(
        commit_calls.get(),
        2,
        "the first sealed focus request needs one follow-up generation; reassertion must be a no-op"
    );
    cx.update_window(window, |_, window, cx| {
        assert_eq!(window.focused(cx).as_ref(), Some(&committed_focus));
        assert_eq!(window.retained_focus_claim_count_for_test(), 0);
    })
    .unwrap();
}

#[open_gpui::test]
fn sealed_commit_does_not_renew_a_rejected_focus_claim(cx: &mut TestAppContext) {
    let commit_calls = Rc::new(Cell::new(0));
    let outcomes = Rc::new(RefCell::new(Vec::new()));
    let subscriptions = Rc::new(RefCell::new(Vec::new()));
    let typed_window = cx.open_window(size(px(320.0), px(200.0)), {
        let commit_calls = commit_calls.clone();
        let outcomes = outcomes.clone();
        let subscriptions = subscriptions.clone();
        move |_, cx| RejectedCommitPhaseFocusView {
            invalid_focus: cx.focus_handle(),
            commit_calls,
            outcomes,
            subscriptions,
        }
    });
    let window = typed_window.into();
    cx.run_until_parked();
    draw_focus_followup_frame(cx, window);

    assert_eq!(
        commit_calls.get(),
        2,
        "a rejected sealed request gets one candidate generation and cannot renew itself"
    );
    assert_eq!(outcomes.borrow().as_slice(), &[FocusClaimOutcome::Rejected]);
    assert_eq!(subscriptions.borrow().len(), 1);
    cx.update_window(window, |_, window, _| {
        assert_eq!(window.retained_focus_claim_count_for_test(), 0);
        assert!(!window.refresh_pending_for_test());
    })
    .unwrap();
}

#[open_gpui::test]
fn sealed_focus_round_trip_does_not_retain_an_orphaned_followup_frame(cx: &mut TestAppContext) {
    let commit_calls = Rc::new(Cell::new(0));
    let typed_window = cx.open_window(size(px(320.0), px(200.0)), {
        let commit_calls = commit_calls.clone();
        move |_, cx| SealedFocusRoundTripView {
            committed_focus: cx.focus_handle(),
            transient_focus: cx.focus_handle(),
            commit_calls,
        }
    });
    let view = typed_window.root(cx).unwrap();
    let window = typed_window.into();
    cx.run_until_parked();
    draw_focus_followup_frame(cx, window);

    assert_eq!(
        commit_calls.get(),
        2,
        "the final committed request must cancel the transient request's follow-up wakeup"
    );
    let committed_focus = cx.read(|cx| view.read(cx).committed_focus.clone());
    cx.update_window(window, |_, window, cx| {
        assert_eq!(window.focused(cx).as_ref(), Some(&committed_focus));
        assert_eq!(window.retained_focus_claim_count_for_test(), 0);
        assert!(!window.refresh_pending_for_test());
    })
    .unwrap();
}

#[open_gpui::test]
fn reasserting_committed_focus_cancels_a_queued_focus_only_frame(cx: &mut TestAppContext) {
    let commit_focus_on_next_frame = Rc::new(Cell::new(false));
    let commit_calls = Rc::new(Cell::new(0));
    let typed_window = cx.open_window(size(px(320.0), px(200.0)), {
        let commit_focus_on_next_frame = commit_focus_on_next_frame.clone();
        let commit_calls = commit_calls.clone();
        move |_, cx| CommitPhaseFocusView {
            first_focus: cx.focus_handle(),
            committed_focus: cx.focus_handle(),
            commit_focus_on_next_frame,
            commit_focus_every_frame: false,
            commit_calls,
        }
    });
    let view = typed_window.root(cx).unwrap();
    let window = typed_window.into();
    cx.run_until_parked();

    let first_focus = cx.read(|cx| view.read(cx).first_focus.clone());
    cx.update_window(window, |_, window, cx| first_focus.focus(window, cx))
        .unwrap();
    cx.run_until_parked();
    commit_focus_on_next_frame.set(true);
    view.update(cx, |_, cx| cx.notify());
    cx.run_until_parked();
    cx.update_window(window, |_, window, _| {
        assert!(window.refresh_pending_for_test());
    })
    .unwrap();

    cx.update_window(window, |_, window, cx| first_focus.focus(window, cx))
        .unwrap();
    cx.run_until_parked();
    assert_eq!(commit_calls.get(), 1);
    cx.update_window(window, |_, window, cx| {
        assert_eq!(window.focused(cx).as_ref(), Some(&first_focus));
        assert_eq!(window.retained_focus_claim_count_for_test(), 0);
        assert!(!window.refresh_pending_for_test());
    })
    .unwrap();
}

#[open_gpui::test]
fn already_empty_blur_cancels_a_queued_focus_only_frame_and_dispatches_supersession(
    cx: &mut TestAppContext,
) {
    let commit_calls = Rc::new(Cell::new(0));
    let outcomes = Rc::new(RefCell::new(Vec::new()));
    let subscriptions = Rc::new(RefCell::new(Vec::new()));
    let typed_window = cx.open_window(size(px(320.0), px(200.0)), {
        let commit_calls = commit_calls.clone();
        let outcomes = outcomes.clone();
        let subscriptions = subscriptions.clone();
        move |_, cx| RejectedCommitPhaseFocusView {
            invalid_focus: cx.focus_handle(),
            commit_calls,
            outcomes,
            subscriptions,
        }
    });
    let window = typed_window.into();
    cx.run_until_parked();
    cx.update_window(window, |_, window, _| {
        assert!(window.refresh_pending_for_test());
    })
    .unwrap();

    cx.update_window(window, |_, window, cx| window.blur(cx))
        .unwrap();
    cx.run_until_parked();
    assert_eq!(commit_calls.get(), 1);
    assert_eq!(
        outcomes.borrow().as_slice(),
        &[FocusClaimOutcome::Superseded]
    );
    assert_eq!(subscriptions.borrow().len(), 1);
    cx.update_window(window, |_, window, cx| {
        assert!(window.focused(cx).is_none());
        assert_eq!(window.retained_focus_claim_count_for_test(), 0);
        assert!(!window.refresh_pending_for_test());
    })
    .unwrap();
}

#[open_gpui::test]
fn focus_phase_round_trip_does_not_schedule_an_extra_frame(cx: &mut TestAppContext) {
    let frame_commits = Rc::new(Cell::new(0));
    let typed_window = cx.open_window(size(px(320.0), px(200.0)), {
        let frame_commits = frame_commits.clone();
        move |_, cx| FocusPhaseRoundTripView {
            committed_focus: cx.focus_handle(),
            transient_focus: cx.focus_handle(),
            frame_commits,
        }
    });
    let view = typed_window.root(cx).unwrap();
    let window = typed_window.into();
    cx.run_until_parked();

    let (committed_focus, transient_focus) = cx.read(|cx| {
        let view = view.read(cx);
        (view.committed_focus.clone(), view.transient_focus.clone())
    });
    let observer = cx
        .update_window(window, |_, window, cx| {
            let committed_focus = committed_focus.clone();
            window.on_focus_committed(&committed_focus.clone(), cx, move |window, cx| {
                transient_focus.focus(window, cx);
                committed_focus.focus(window, cx);
            })
        })
        .unwrap();
    cx.update_window(window, |_, window, cx| committed_focus.focus(window, cx))
        .unwrap();
    cx.run_until_parked();

    assert_eq!(
        frame_commits.get(),
        2,
        "returning to the committed leaf during Focus dispatch must not draw a third frame"
    );
    cx.update_window(window, |_, window, cx| {
        assert_eq!(window.focused(cx).as_ref(), Some(&committed_focus));
        assert!(!window.refresh_pending_for_test());
    })
    .unwrap();
    drop(observer);
}

#[open_gpui::test]
fn focus_phase_window_refresh_schedules_one_generic_followup_frame(cx: &mut TestAppContext) {
    let frame_commits = Rc::new(Cell::new(0));
    let typed_window = cx.open_window(size(px(320.0), px(200.0)), {
        let frame_commits = frame_commits.clone();
        move |_, cx| FocusPhaseRoundTripView {
            committed_focus: cx.focus_handle(),
            transient_focus: cx.focus_handle(),
            frame_commits,
        }
    });
    let view = typed_window.root(cx).unwrap();
    let window = typed_window.into();
    cx.run_until_parked();

    let committed_focus = cx.read(|cx| view.read(cx).committed_focus.clone());
    let observer = cx
        .update_window(window, |_, window, cx| {
            window.on_focus_committed(&committed_focus.clone(), cx, |window, _| {
                window.refresh();
            })
        })
        .unwrap();
    cx.update_window(window, |_, window, cx| committed_focus.focus(window, cx))
        .unwrap();
    cx.run_until_parked();

    assert_eq!(frame_commits.get(), 2);
    cx.update_window(window, |_, window, cx| {
        assert!(
            window.refresh_pending_for_test(),
            "a raw refresh during Focus dispatch must remain pending for the next platform frame"
        );
        window.draw(cx).clear();
    })
    .unwrap();
    cx.run_until_parked();

    assert_eq!(
        frame_commits.get(),
        3,
        "a raw refresh requested during Focus dispatch must schedule one generic follow-up frame"
    );
    cx.update_window(window, |_, window, _| {
        assert!(!window.refresh_pending_for_test());
    })
    .unwrap();
    drop(observer);
}

#[open_gpui::test]
fn focus_phase_entity_invalidation_survives_focus_followup_cancellation(cx: &mut TestAppContext) {
    let rendered_revision = Rc::new(Cell::new(0));
    let observer_notifications = Rc::new(Cell::new(0));
    let typed_window = cx.open_window(size(px(320.0), px(200.0)), {
        let rendered_revision = rendered_revision.clone();
        move |_, cx| FocusPhaseInvalidationView {
            committed_focus: cx.focus_handle(),
            transient_focus: cx.focus_handle(),
            revision: 0,
            rendered_revision,
        }
    });
    let view = typed_window.root(cx).unwrap();
    let window = typed_window.into();
    cx.run_until_parked();
    let _entity_observer = cx.update({
        let view = view.clone();
        let observer_notifications = observer_notifications.clone();
        move |cx| {
            cx.observe(&view, move |_, _| {
                observer_notifications.set(observer_notifications.get() + 1);
            })
        }
    });

    let (committed_focus, transient_focus) = cx.read(|cx| {
        let view = view.read(cx);
        (view.committed_focus.clone(), view.transient_focus.clone())
    });
    let observer = cx
        .update_window(window, |_, window, cx| {
            let view = view.clone();
            window.on_focus_committed(&committed_focus.clone(), cx, move |window, cx| {
                view.update(cx, |view, cx| {
                    view.revision += 1;
                    cx.notify();
                });
                transient_focus.focus(window, cx);
            })
        })
        .unwrap();

    cx.update_window(window, |_, window, cx| committed_focus.focus(window, cx))
        .unwrap();
    cx.run_until_parked();
    drop(observer);

    cx.update_window(window, |_, window, cx| committed_focus.focus(window, cx))
        .unwrap();
    cx.run_until_parked();

    assert_eq!(
        rendered_revision.get(),
        1,
        "an entity invalidated during Focus dispatch must retain a generic redraw even when the focus-only follow-up is cancelled"
    );
    assert_eq!(
        observer_notifications.get(),
        1,
        "a focus-phase notify must dispatch entity observers exactly once"
    );
}

#[open_gpui::test]
fn alternating_rejected_focus_claims_advance_once_per_platform_frame(cx: &mut TestAppContext) {
    let commit_calls = Rc::new(Cell::new(0));
    let typed_window = cx.open_window(size(px(320.0), px(200.0)), {
        let commit_calls = commit_calls.clone();
        move |_, cx| AlternatingRejectedFocusView {
            first_invalid_focus: cx.focus_handle(),
            second_invalid_focus: cx.focus_handle(),
            commit_calls,
        }
    });
    let window = typed_window.into();

    cx.run_until_parked();
    assert_eq!(commit_calls.get(), 1);
    for expected_commit_calls in 2..=4 {
        draw_focus_followup_frame(cx, window);
        assert_eq!(
            commit_calls.get(),
            expected_commit_calls,
            "an explicit platform frame must consume at most one focus-only follow-up"
        );
    }
    cx.update_window(window, |_, window, _| {
        assert_eq!(window.retained_focus_claim_count_for_test(), 0);
        assert!(!window.refresh_pending_for_test());
    })
    .unwrap();
}

#[open_gpui::test]
fn fresh_rejected_focus_claims_advance_once_per_platform_frame(cx: &mut TestAppContext) {
    let commit_calls = Rc::new(Cell::new(0));
    let retained_invalid_focuses = Rc::new(RefCell::new(Vec::new()));
    let typed_window = cx.open_window(size(px(320.0), px(200.0)), {
        let commit_calls = commit_calls.clone();
        let retained_invalid_focuses = retained_invalid_focuses.clone();
        move |_, _| FreshRejectedFocusView {
            retained_invalid_focuses,
            commit_calls,
        }
    });
    let window = typed_window.into();

    cx.run_until_parked();
    assert_eq!(commit_calls.get(), 1);
    for expected_commit_calls in 2..=4 {
        draw_focus_followup_frame(cx, window);
        assert_eq!(
            commit_calls.get(),
            expected_commit_calls,
            "a fresh rejected FocusId must wait for the next explicit platform frame"
        );
    }
    assert_eq!(retained_invalid_focuses.borrow().len(), 4);
    cx.update_window(window, |_, window, _| {
        assert_eq!(window.retained_focus_claim_count_for_test(), 1);
        assert!(window.refresh_pending_for_test());
        assert!(window.invalidator.is_focus_only_dirty());
    })
    .unwrap();
}

#[open_gpui::test]
fn sealed_commit_blur_defers_once_and_empty_reassertion_does_not_redraw_forever(
    cx: &mut TestAppContext,
) {
    let blur_on_next_frame = Rc::new(Cell::new(false));
    let commit_calls = Rc::new(Cell::new(0));
    let typed_window = cx.open_window(size(px(320.0), px(200.0)), {
        let blur_on_next_frame = blur_on_next_frame.clone();
        let commit_calls = commit_calls.clone();
        move |_, cx| CommitPhaseBlurView {
            focus: cx.focus_handle(),
            blur_on_next_frame,
            blur_every_frame: false,
            commit_calls,
        }
    });
    let view = typed_window.root(cx).unwrap();
    let window = typed_window.into();
    cx.run_until_parked();

    let focus = cx.read(|cx| view.read(cx).focus.clone());
    cx.update_window(window, |_, window, cx| focus.focus(window, cx))
        .unwrap();
    cx.run_until_parked();

    blur_on_next_frame.set(true);
    view.update(cx, |_, cx| cx.notify());
    cx.run_until_parked();
    draw_focus_followup_frame(cx, window);
    assert_eq!(commit_calls.get(), 1);
    cx.update_window(window, |_, window, cx| {
        assert!(window.focused(cx).is_none());
        assert_eq!(window.retained_focus_claim_count_for_test(), 0);
    })
    .unwrap();

    view.update(cx, |view, cx| {
        view.blur_every_frame = true;
        cx.notify();
    });
    cx.run_until_parked();
    assert_eq!(
        commit_calls.get(),
        2,
        "reasserting an already-empty sealed focus must not schedule another generation"
    );
    cx.update_window(window, |_, window, cx| {
        assert!(window.focused(cx).is_none());
        assert_eq!(window.retained_focus_claim_count_for_test(), 0);
    })
    .unwrap();
}

#[open_gpui::test]
fn failed_prepaint_transaction_preserves_committed_focus_authority(cx: &mut TestAppContext) {
    let typed_window = cx.open_window(size(px(320.0), px(200.0)), |_, cx| {
        RolledBackFocusClaimView {
            committed_focus: cx.focus_handle(),
            rejected_focus: cx.focus_handle(),
        }
    });
    let view = typed_window.root(cx).unwrap();
    let window = typed_window.into();
    cx.run_until_parked();

    let (committed_focus, rejected_focus) = cx.read(|cx| {
        let view = view.read(cx);
        (view.committed_focus.clone(), view.rejected_focus.clone())
    });
    cx.update_window(window, |_, window, cx| {
        window.activate_window();
        committed_focus.focus(window, cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(window, |_, window, cx| {
        assert_eq!(window.focused(cx).as_ref(), Some(&committed_focus));
        let revision = window.focus_claim_revision();
        rejected_focus.focus(window, cx);
        assert_eq!(window.focused(cx).as_ref(), Some(&committed_focus));
        assert_eq!(window.focus_claim_revision(), revision.wrapping_add(1));
        assert_eq!(window.retained_focus_claim_count_for_test(), 1);
    })
    .unwrap();

    cx.run_until_parked();
    cx.update_window(window, |_, window, cx| {
        assert_eq!(window.focused(cx).as_ref(), Some(&committed_focus));
        assert_eq!(window.retained_focus_claim_count_for_test(), 0);
    })
    .unwrap();
}

#[open_gpui::test]
fn failed_prepaint_transaction_restores_prior_focus_completion(cx: &mut TestAppContext) {
    let rejected_outcomes = Rc::new(RefCell::new(Vec::new()));
    let rejected_subscriptions = Rc::new(RefCell::new(Vec::new()));
    let rejected_callback_drops = Rc::new(Cell::new(0));
    let typed_window = cx.open_window(size(px(320.0), px(200.0)), {
        let rejected_outcomes = rejected_outcomes.clone();
        let rejected_subscriptions = rejected_subscriptions.clone();
        let rejected_callback_drops = rejected_callback_drops.clone();
        move |_, cx| FocusCompletionTransactionView {
            committed_focus: cx.focus_handle(),
            rejected_focus: cx.focus_handle(),
            attempt_rollback: false,
            rejected_outcomes,
            rejected_subscriptions,
            rejected_callback_drops,
        }
    });
    let view = typed_window.root(cx).unwrap();
    let window = typed_window.into();
    cx.run_until_parked();

    let committed_focus = cx.read(|cx| view.read(cx).committed_focus.clone());
    let committed_outcomes = Rc::new(RefCell::new(Vec::new()));
    let committed_subscription = cx
        .update_window(window, |_, window, cx| {
            view.update(cx, |view, cx| {
                view.attempt_rollback = true;
                cx.notify();
            });
            let committed_outcomes = committed_outcomes.clone();
            window.focus_with_completion(&committed_focus, cx, move |outcome, _, _| {
                committed_outcomes.borrow_mut().push(outcome);
            })
        })
        .unwrap();
    cx.run_until_parked();

    assert_eq!(
        committed_outcomes.borrow().as_slice(),
        &[FocusClaimOutcome::Committed],
        "rollback must restore the focus completion superseded inside the transaction"
    );
    assert!(
        rejected_outcomes.borrow().is_empty(),
        "a transaction-local completion must not escape rollback"
    );
    assert_eq!(rejected_subscriptions.borrow().len(), 1);
    assert_eq!(
        rejected_callback_drops.get(),
        rejected_subscriptions.borrow().len(),
        "rollback must release transaction-local callbacks even while subscriptions are retained"
    );
    cx.update_window(window, |_, window, cx| {
        assert_eq!(window.focused(cx).as_ref(), Some(&committed_focus));
        assert_eq!(window.retained_focus_claim_count_for_test(), 0);
    })
    .unwrap();

    drop(committed_subscription);
    rejected_subscriptions.borrow_mut().clear();
}

#[open_gpui::test]
fn unbound_presentation_focus_claims_are_not_exposed_or_replayed(cx: &mut TestAppContext) {
    for suppressed in [SubtreePresentation::Inert, SubtreePresentation::Hidden] {
        let counters = Rc::new(PresentationCounters::default());
        let ime = PresentationImeState::default();
        let typed_window = cx.open_window(size(px(320.0), px(200.0)), {
            let counters = counters.clone();
            let ime = ime.clone();
            move |window, cx| PresentationProbeView {
                presentation: suppressed,
                descendant_presentation: SubtreePresentation::Visible,
                focus: cx.focus_handle(),
                capture: window.new_pointer_capture_handle(),
                counters,
                ime,
            }
        });
        let view = typed_window.root(cx).unwrap();
        let window = typed_window.into();
        cx.run_until_parked();

        let focus = cx.read(|cx| view.read(cx).focus.clone());
        let outcomes = Rc::new(RefCell::new(Vec::new()));
        let subscription = cx
            .update_window(window, |_, window, cx| {
                let revision = window.focus_claim_revision();
                let outcomes = outcomes.clone();
                let subscription =
                    window.focus_with_completion(&focus, cx, move |outcome, _, _| {
                        outcomes.borrow_mut().push(outcome);
                    });
                assert!(
                    window.focused(cx).is_none(),
                    "an unbound {suppressed:?} handle must not become observable before a qualifying frame"
                );
                assert!(window.focus_claim_revision() > revision);
                subscription
            })
            .unwrap();
        cx.run_until_parked();

        cx.update_window(window, |_, window, cx| {
            assert!(window.focused(cx).is_none());
            assert_eq!(window.retained_focus_claim_count_for_test(), 0);
        })
        .unwrap();
        assert_eq!(outcomes.borrow().as_slice(), &[FocusClaimOutcome::Rejected]);

        view.update(cx, |view, cx| {
            view.presentation = SubtreePresentation::Visible;
            cx.notify();
        });
        cx.run_until_parked();

        cx.update_window(window, |_, window, cx| {
            assert!(
                window.focused(cx).is_none(),
                "restoring visibility must require fresh focus intent"
            );
        })
        .unwrap();
        assert_eq!(
            outcomes.borrow().as_slice(),
            &[FocusClaimOutcome::Rejected],
            "restoring visibility must not replay a rejected claim"
        );
        assert!(!platform_has_input_handler(cx, window));
        drop(subscription);
    }
}

#[open_gpui::test]
fn presentation_focus_claim_promotes_in_the_next_visible_candidate_frame(cx: &mut TestAppContext) {
    let counters = Rc::new(PresentationCounters::default());
    let ime = PresentationImeState::default();
    let typed_window = cx.open_window(size(px(320.0), px(200.0)), {
        let counters = counters.clone();
        let ime = ime.clone();
        move |window, cx| PresentationProbeView {
            presentation: SubtreePresentation::Visible,
            descendant_presentation: SubtreePresentation::Visible,
            focus: cx.focus_handle(),
            capture: window.new_pointer_capture_handle(),
            counters,
            ime,
        }
    });
    let view = typed_window.root(cx).unwrap();
    let window = typed_window.into();
    cx.run_until_parked();

    let focus = cx.read(|cx| view.read(cx).focus.clone());
    cx.update_window(window, |_, window, cx| focus.focus(window, cx))
        .unwrap();
    cx.run_until_parked();
    assert!(platform_has_input_handler(cx, window));

    view.update(cx, |view, cx| {
        view.presentation = SubtreePresentation::Inert;
        cx.notify();
    });
    cx.run_until_parked();
    cx.update_window(window, |_, window, cx| {
        assert!(window.focused(cx).is_none());
    })
    .unwrap();
    assert!(!platform_has_input_handler(cx, window));

    cx.update(|cx| {
        view.update(cx, |view, cx| {
            view.presentation = SubtreePresentation::Visible;
            cx.notify();
        });
        window
            .update(cx, |_, window, cx| {
                assert!(!window.is_focus_handle_rendered(&focus));
                let revision = window.focus_claim_revision();
                focus.focus(window, cx);
                assert!(window.focus_claim_revision() > revision);
                assert_eq!(window.retained_focus_claim_count_for_test(), 1);
                assert!(
                    window.focused(cx).is_none(),
                    "the retained handle must remain unqualified until the visible candidate frame"
                );
            })
            .unwrap();
    });

    cx.run_until_parked();
    cx.update_window(window, |_, window, cx| {
        assert_eq!(window.focused(cx).as_ref(), Some(&focus));
        assert_eq!(window.retained_focus_claim_count_for_test(), 0);
    })
    .unwrap();
    assert!(platform_has_input_handler(cx, window));
    cx.simulate_marked_text(window, None, "candidate", None);
    assert_eq!(ime.marked_updates.get(), 1);
}

struct FocusAuthorityChurnView {
    focus: Option<FocusHandle>,
}

impl Render for FocusAuthorityChurnView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let mut root = div();
        if let Some(focus) = self.focus.as_ref() {
            root = root.child(div().track_focus(focus));
        }
        root
    }
}

#[open_gpui::test]
fn released_focus_handles_do_not_accumulate_window_authority_state(cx: &mut TestAppContext) {
    let typed_window = cx.open_window(size(px(320.0), px(200.0)), |_, _| FocusAuthorityChurnView {
        focus: None,
    });
    let view = typed_window.root(cx).unwrap();
    let window = typed_window.into();
    cx.run_until_parked();
    let baseline_focus_handles = cx.read(|cx| cx.focus_handles.read().len());

    for _ in 0..32 {
        let focus = cx.read(|cx| cx.focus_handle());
        view.update(cx, |view, cx| {
            view.focus = Some(focus.clone());
            cx.notify();
        });
        cx.run_until_parked();
        cx.update_window(window, |_, window, cx| focus.focus(window, cx))
            .unwrap();
        cx.run_until_parked();

        view.update(cx, |view, cx| {
            view.focus = None;
            cx.notify();
        });
        cx.run_until_parked();
        drop(focus);
        cx.run_until_parked();

        cx.update_window(window, |_, window, cx| {
            assert!(window.focused(cx).is_none());
            assert_eq!(window.retained_focus_claim_count_for_test(), 0);
        })
        .unwrap();
    }

    assert_eq!(
        cx.read(|cx| cx.focus_handles.read().len()),
        baseline_focus_handles,
        "released FocusIds must not remain retained by window authority bookkeeping"
    );
}

struct PresentationCommitOrderingView {
    presentation: SubtreePresentation,
    attempt_reacquire: bool,
    focus: FocusHandle,
    capture: PointerCaptureHandle,
    observations: Rc<RefCell<Vec<(bool, bool)>>>,
    reacquire_attempts: Rc<RefCell<Vec<(Result<(), PointerCaptureError>, bool)>>>,
}

impl Render for PresentationCommitOrderingView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let capture = self.capture;
        let focus = self.focus.clone();
        let attempt_reacquire = self.attempt_reacquire;
        let reacquire_attempts = self.reacquire_attempts.clone();
        let owner = canvas(
            move |bounds, window, cx| {
                let hitbox = window.insert_hitbox(bounds, HitboxBehavior::Normal);
                window.bind_pointer_capture(&capture, hitbox.id).unwrap();
                window.set_focus_handle(&focus, cx);

                if attempt_reacquire {
                    let focus = focus.clone();
                    let reacquire_attempts = reacquire_attempts.clone();
                    window.record_prepaint_window_commit(move |_, window, cx| {
                        if !reacquire_attempts.borrow().is_empty() {
                            return;
                        }
                        let capture_result = window.capture_pointer(&capture, MouseButton::Left);
                        focus.focus(window, cx);
                        let focus_reacquired = window.focused(cx).as_ref() == Some(&focus);
                        reacquire_attempts
                            .borrow_mut()
                            .push((capture_result, focus_reacquired));
                    });
                }
            },
            |_, _, _, _| {},
        )
        .w(px(80.0))
        .h(px(40.0))
        .with_subtree_presentation(self.presentation);

        let observations = self.observations.clone();
        let observer = canvas(
            move |_, window, _| {
                let observations = observations.clone();
                window.record_prepaint_window_commit(move |_, window, cx| {
                    observations.borrow_mut().push((
                        window.captured_pointer().is_none(),
                        window.focused(cx).is_none(),
                    ));
                });
            },
            |_, _, _, _| {},
        )
        .w(px(1.0))
        .h(px(1.0));

        div().flex().child(owner).child(observer)
    }
}

#[open_gpui::test]
fn presentation_revocation_precedes_same_frame_publication_commit(cx: &mut TestAppContext) {
    let observations = Rc::new(RefCell::new(Vec::new()));
    let reacquire_attempts = Rc::new(RefCell::new(Vec::new()));
    let typed_window = cx.open_window(size(px(320.0), px(200.0)), {
        let observations = observations.clone();
        let reacquire_attempts = reacquire_attempts.clone();
        move |window, cx| PresentationCommitOrderingView {
            presentation: SubtreePresentation::Visible,
            attempt_reacquire: false,
            focus: cx.focus_handle(),
            capture: window.new_pointer_capture_handle(),
            observations,
            reacquire_attempts,
        }
    });
    let view = typed_window.root(cx).unwrap();
    let window = typed_window.into();
    cx.run_until_parked();

    let (focus, capture) = cx.read(|cx| {
        let view = view.read(cx);
        (view.focus.clone(), view.capture)
    });
    cx.update_window(window, |_, window, _| window.activate_window())
        .unwrap();
    cx.run_until_parked();
    cx.update_window(window, |_, window, cx| {
        window.draw(cx).clear();
        focus.focus(window, cx);
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
        window.capture_pointer(&capture, MouseButton::Left).unwrap();
    })
    .unwrap();
    observations.borrow_mut().clear();
    reacquire_attempts.borrow_mut().clear();

    view.update(cx, |view, cx| {
        view.presentation = SubtreePresentation::Inert;
        view.attempt_reacquire = true;
        cx.notify();
    });
    cx.update_window(window, |_, window, cx| window.draw(cx).clear())
        .unwrap();

    let observations = observations.borrow();
    assert!(!observations.is_empty());
    assert!(
        observations
            .iter()
            .all(|observation| *observation == (true, true)),
        "every same-frame publication must observe revoked capture and focus: {observations:?}"
    );
    drop(observations);

    let reacquire_attempts = reacquire_attempts.borrow();
    assert!(!reacquire_attempts.is_empty());
    assert!(
        reacquire_attempts.iter().all(|attempt| {
            matches!(
                attempt,
                (Err(PointerCaptureError::HandleNotBound { .. }), false)
            )
        }),
        "suppressed publication commits must not reclaim input authority: {reacquire_attempts:?}"
    );
    drop(reacquire_attempts);

    cx.update_window(window, |_, window, cx| {
        let focus_revision = window.focus_claim_revision();
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

        focus.focus(window, cx);
        assert!(window.focused(cx).is_none());
        assert!(window.focus_claim_revision() > focus_revision);
        assert_eq!(window.retained_focus_claim_count_for_test(), 1);
        assert!(matches!(
            window.capture_pointer(&capture, MouseButton::Left),
            Err(PointerCaptureError::HandleNotBound { .. })
        ));

        window.dispatch_event(
            PlatformInput::MouseUp(MouseUpEvent {
                button: MouseButton::Left,
                position: point(px(10.0), px(10.0)),
                modifiers: Modifiers::none(),
                click_count: 1,
            }),
            cx,
        );
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(window, |_, window, cx| {
        assert!(window.focused(cx).is_none());
        assert_eq!(window.retained_focus_claim_count_for_test(), 0);
    })
    .unwrap();
}

#[open_gpui::test]
fn subtree_presentation_matrix_is_inherited_and_layout_neutral(cx: &mut TestAppContext) {
    let counters = Rc::new(PresentationCounters::default());
    let ime = PresentationImeState::default();
    let typed_window = cx.open_window(size(px(320.0), px(200.0)), {
        let counters = counters.clone();
        let ime = ime.clone();
        move |window, cx| PresentationProbeView {
            presentation: SubtreePresentation::Visible,
            descendant_presentation: SubtreePresentation::Visible,
            focus: cx.focus_handle(),
            capture: window.new_pointer_capture_handle(),
            counters,
            ime,
        }
    });
    let view = typed_window.root(cx).unwrap();
    let window = typed_window.into();

    assert!(cx.activate_accessibility(window));
    let visible = counters.snapshot();
    assert!(visible.layouts > 0);
    assert!(visible.prepaints > 0);
    assert!(visible.paints > 0);
    assert_eq!(visible.pointer_bindings, visible.prepaints);
    assert_eq!(visible.autoscrolls, visible.prepaints);
    let sibling_bounds = debug_bounds(cx, window, "presentation-layout-sibling");
    let visible_update = cx.latest_accessibility_tree_update(window).unwrap();
    assert!(node_id_with_label(&visible_update, "Raw presentation probe").is_some());

    update_presentation(
        &view,
        cx,
        SubtreePresentation::Inert,
        SubtreePresentation::Visible,
    );
    let inert = counters.snapshot();
    assert!(inert.layouts > visible.layouts);
    assert!(inert.prepaints > visible.prepaints);
    assert!(inert.paints > visible.paints);
    assert_eq!(inert.pointer_bindings, visible.pointer_bindings);
    assert_eq!(inert.autoscrolls, visible.autoscrolls);
    assert_eq!(
        debug_bounds(cx, window, "presentation-layout-sibling"),
        sibling_bounds
    );
    let inert_update = cx.latest_accessibility_tree_update(window).unwrap();
    assert!(node_id_with_label(&inert_update, "Raw presentation probe").is_none());

    update_presentation(
        &view,
        cx,
        SubtreePresentation::Hidden,
        SubtreePresentation::Visible,
    );
    let hidden = counters.snapshot();
    assert!(hidden.layouts > inert.layouts);
    assert_eq!(hidden.prepaints, inert.prepaints);
    assert_eq!(hidden.paints, inert.paints);
    assert_eq!(
        debug_bounds(cx, window, "presentation-layout-sibling"),
        sibling_bounds
    );

    update_presentation(
        &view,
        cx,
        SubtreePresentation::Inert,
        SubtreePresentation::Hidden,
    );
    let hidden_descendant = counters.snapshot();
    assert!(hidden_descendant.layouts > hidden.layouts);
    assert_eq!(hidden_descendant.prepaints, hidden.prepaints);
    assert_eq!(hidden_descendant.paints, hidden.paints);

    update_presentation(
        &view,
        cx,
        SubtreePresentation::Visible,
        SubtreePresentation::Inert,
    );
    let inert_descendant = counters.snapshot();
    assert!(inert_descendant.layouts > hidden_descendant.layouts);
    assert!(inert_descendant.prepaints > hidden_descendant.prepaints);
    assert!(inert_descendant.paints > hidden_descendant.paints);
    assert_eq!(
        inert_descendant.pointer_bindings,
        hidden_descendant.pointer_bindings
    );
}

#[open_gpui::test]
fn dynamic_presentation_suppression_revokes_every_raw_interaction_channel(cx: &mut TestAppContext) {
    let counters = Rc::new(PresentationCounters::default());
    let ime = PresentationImeState::default();
    let typed_window = cx.open_window(size(px(320.0), px(200.0)), {
        let counters = counters.clone();
        let ime = ime.clone();
        move |window, cx| PresentationProbeView {
            presentation: SubtreePresentation::Visible,
            descendant_presentation: SubtreePresentation::Visible,
            focus: cx.focus_handle(),
            capture: window.new_pointer_capture_handle(),
            counters,
            ime,
        }
    });
    let view = typed_window.root(cx).unwrap();
    let window = typed_window.into();
    let _cancel_subscription = cx
        .update_window(window, |_, window, _| {
            let counters = counters.clone();
            window.intercept_window_mouse_events(move |event, _, _| {
                if let WindowMouseEvent::Cancel(PointerCancelEvent {
                    reason: PointerCancelReason::CaptureRevoked,
                }) = event
                {
                    counters
                        .pointer_cancellations
                        .set(counters.pointer_cancellations.get() + 1);
                }
            })
        })
        .unwrap();

    cx.update_window(window, |_, window, _| window.activate_window())
        .unwrap();
    assert!(cx.activate_accessibility(window));
    let focus = cx.read(|cx| view.read(cx).focus.clone());
    cx.update_window(window, |_, window, cx| focus.focus(window, cx))
        .unwrap();
    view.update(cx, |_, cx| cx.notify());
    cx.run_until_parked();
    assert!(platform_has_input_handler(cx, window));

    cx.simulate_event(
        window,
        MouseDownEvent {
            button: MouseButton::Left,
            position: point(px(10.0), px(10.0)),
            modifiers: Modifiers::none(),
            click_count: 1,
            first_mouse: false,
        },
    );
    assert_eq!(counters.mouse_downs.get(), 1);
    assert!(
        cx.update_window(window, |_, window, _| window.captured_pointer().is_some())
            .unwrap()
    );

    cx.dispatch_keystroke(window, Keystroke::parse("a").unwrap());
    assert_eq!(counters.key_downs.get(), 1);
    cx.dispatch_action(window, PresentationProbeAction);
    assert_eq!(counters.actions.get(), 1);

    let visible_update = cx.latest_accessibility_tree_update(window).unwrap();
    let node_id = node_id_with_label(&visible_update, "Raw presentation probe").unwrap();
    assert!(cx.dispatch_accessibility_action(
        window,
        ActionRequest {
            action: AccessibleAction::Click,
            target_tree: TreeId::ROOT,
            target_node: node_id,
            data: None,
        },
    ));
    assert_eq!(counters.accessibility_actions.get(), 1);

    cx.simulate_marked_text(window, None, "marked", None);
    assert!(ime.marked.get());
    assert_eq!(ime.marked_updates.get(), 1);

    let before_inert = counters.snapshot();
    update_presentation(
        &view,
        cx,
        SubtreePresentation::Inert,
        SubtreePresentation::Visible,
    );
    assert_eq!(ime.unmarks.get(), 1);
    assert!(!ime.marked.get());
    assert!(!platform_has_input_handler(cx, window));
    assert!(
        cx.update_window(window, |_, window, cx| window.focused(cx).is_none())
            .unwrap()
    );
    assert!(
        cx.update_window(window, |_, window, _| window.captured_pointer().is_none())
            .unwrap()
    );
    assert_eq!(
        counters.pointer_cancellations.get(),
        before_inert.pointer_cancellations + 2,
        "the old raw listener and the window interceptor must each observe one terminal cancel"
    );

    let suppressed = counters.snapshot();
    cx.simulate_event(
        window,
        MouseDownEvent {
            button: MouseButton::Left,
            position: point(px(10.0), px(10.0)),
            modifiers: Modifiers::none(),
            click_count: 1,
            first_mouse: false,
        },
    );
    cx.simulate_event(
        window,
        MouseUpEvent {
            button: MouseButton::Left,
            position: point(px(10.0), px(10.0)),
            modifiers: Modifiers::none(),
            click_count: 1,
        },
    );
    cx.dispatch_keystroke(window, Keystroke::parse("b").unwrap());
    cx.dispatch_action(window, PresentationProbeAction);
    assert!(cx.dispatch_accessibility_action(
        window,
        ActionRequest {
            action: AccessibleAction::Click,
            target_tree: TreeId::ROOT,
            target_node: node_id,
            data: None,
        },
    ));
    let after_suppressed_input = counters.snapshot();
    assert_eq!(after_suppressed_input.mouse_downs, suppressed.mouse_downs);
    assert_eq!(after_suppressed_input.key_downs, suppressed.key_downs);
    assert_eq!(after_suppressed_input.actions, suppressed.actions);
    assert_eq!(
        after_suppressed_input.accessibility_actions,
        suppressed.accessibility_actions
    );

    update_presentation(
        &view,
        cx,
        SubtreePresentation::Visible,
        SubtreePresentation::Visible,
    );
    assert_eq!(ime.unmarks.get(), 1);
    assert!(!platform_has_input_handler(cx, window));
    assert!(
        cx.update_window(window, |_, window, cx| window.focused(cx).is_none())
            .unwrap()
    );
    let restored_update = cx.latest_accessibility_tree_update(window).unwrap();
    assert_eq!(
        node_id_with_label(&restored_update, "Raw presentation probe"),
        Some(node_id)
    );

    let before_restored_input = counters.mouse_downs.get();
    cx.simulate_event(
        window,
        MouseDownEvent {
            button: MouseButton::Left,
            position: point(px(10.0), px(10.0)),
            modifiers: Modifiers::none(),
            click_count: 1,
            first_mouse: false,
        },
    );
    cx.simulate_event(
        window,
        MouseUpEvent {
            button: MouseButton::Left,
            position: point(px(10.0), px(10.0)),
            modifiers: Modifiers::none(),
            click_count: 1,
        },
    );
    assert_eq!(counters.mouse_downs.get(), before_restored_input + 1);
}

struct PresentationFocusStyleView {
    presentation: SubtreePresentation,
    parent_focus: FocusHandle,
    child_focus: FocusHandle,
}

impl Render for PresentationFocusStyleView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .child(
                div()
                    .id("presentation-focus-parent")
                    .track_focus(&self.parent_focus)
                    .w(px(40.0))
                    .h(px(40.0))
                    .in_focus(|style| style.w(px(120.0)))
                    .child(
                        div()
                            .id("presentation-focus-child")
                            .debug_selector(|| "presentation-focus-child".to_owned())
                            .track_focus(&self.child_focus)
                            .w(px(20.0))
                            .h(px(20.0))
                            .focus(|style| style.w(px(80.0)))
                            .with_subtree_presentation(self.presentation),
                    ),
            )
            .child(
                div()
                    .id("presentation-focus-sibling")
                    .debug_selector(|| "presentation-focus-sibling".to_owned())
                    .w(px(20.0))
                    .h(px(40.0)),
            )
    }
}

#[open_gpui::test]
fn suppressed_focus_styles_are_removed_and_do_not_revive(cx: &mut TestAppContext) {
    let typed_window = cx.open_window(size(px(320.0), px(200.0)), |_, cx| {
        PresentationFocusStyleView {
            presentation: SubtreePresentation::Visible,
            parent_focus: cx.focus_handle(),
            child_focus: cx.focus_handle(),
        }
    });
    let view = typed_window.root(cx).unwrap();
    let window = typed_window.into();
    cx.run_until_parked();
    let child_focus = cx.read(|cx| view.read(cx).child_focus.clone());
    cx.update_window(window, |_, window, cx| child_focus.focus(window, cx))
        .unwrap();
    view.update(cx, |_, cx| cx.notify());
    cx.run_until_parked();

    assert_eq!(
        debug_bounds(cx, window, "presentation-focus-child")
            .size
            .width,
        px(80.0)
    );
    assert_eq!(
        debug_bounds(cx, window, "presentation-focus-sibling")
            .origin
            .x,
        px(40.0)
    );

    view.update(cx, |view, cx| {
        view.presentation = SubtreePresentation::Inert;
        cx.notify();
    });
    cx.run_until_parked();
    assert!(
        cx.update_window(window, |_, window, cx| window.focused(cx).is_none())
            .unwrap()
    );
    assert_eq!(
        debug_bounds(cx, window, "presentation-focus-child")
            .size
            .width,
        px(20.0)
    );
    assert_eq!(
        debug_bounds(cx, window, "presentation-focus-sibling")
            .origin
            .x,
        px(40.0)
    );

    view.update(cx, |view, cx| {
        view.presentation = SubtreePresentation::Visible;
        cx.notify();
    });
    cx.run_until_parked();
    assert_eq!(
        debug_bounds(cx, window, "presentation-focus-child")
            .size
            .width,
        px(20.0)
    );
    assert_eq!(
        debug_bounds(cx, window, "presentation-focus-sibling")
            .origin
            .x,
        px(40.0)
    );
}

struct PresentationInputReplacementView {
    first_presentation: SubtreePresentation,
    claim_second_on_render: bool,
    first_focus: FocusHandle,
    second_focus: FocusHandle,
    third_focus: FocusHandle,
    first_ime: PresentationImeState,
    second_ime: PresentationImeState,
}

impl Render for PresentationInputReplacementView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.claim_second_on_render {
            self.claim_second_on_render = false;
            self.second_focus.focus(window, cx);
        }
        let first_focus = self.first_focus.clone();
        let first_handler = PresentationInputHandler {
            state: self.first_ime.clone(),
            bounds: Bounds::new(point(px(1.0), px(1.0)), size(px(1.0), px(10.0))),
        };
        let second_focus = self.second_focus.clone();
        let second_handler = PresentationInputHandler {
            state: self.second_ime.clone(),
            bounds: Bounds::new(point(px(21.0), px(1.0)), size(px(1.0), px(10.0))),
        };

        div()
            .flex()
            .child(
                div()
                    .id("first-presentation-input")
                    .track_focus(&self.first_focus)
                    .w(px(20.0))
                    .h(px(20.0))
                    .child(canvas(
                        |_, _, _| (),
                        move |_, _, window, cx| {
                            window.handle_input(&first_focus, first_handler, cx)
                        },
                    ))
                    .with_subtree_presentation(self.first_presentation),
            )
            .child(
                div()
                    .id("second-presentation-input")
                    .track_focus(&self.second_focus)
                    .w(px(20.0))
                    .h(px(20.0))
                    .child(canvas(
                        |_, _, _| (),
                        move |_, _, window, cx| {
                            window.handle_input(&second_focus, second_handler, cx)
                        },
                    )),
            )
            .child(
                div()
                    .id("third-presentation-focus")
                    .track_focus(&self.third_focus)
                    .w(px(20.0))
                    .h(px(20.0)),
            )
    }
}

#[open_gpui::test]
fn replacing_a_suppressed_input_owner_finishes_only_the_old_composition(cx: &mut TestAppContext) {
    let first_ime = PresentationImeState::default();
    let second_ime = PresentationImeState::default();
    let typed_window = cx.open_window(size(px(320.0), px(200.0)), {
        let first_ime = first_ime.clone();
        let second_ime = second_ime.clone();
        move |_, cx| PresentationInputReplacementView {
            first_presentation: SubtreePresentation::Visible,
            claim_second_on_render: false,
            first_focus: cx.focus_handle(),
            second_focus: cx.focus_handle(),
            third_focus: cx.focus_handle(),
            first_ime,
            second_ime,
        }
    });
    let view = typed_window.root(cx).unwrap();
    let window = typed_window.into();
    cx.run_until_parked();
    let first_focus = cx.read(|cx| view.read(cx).first_focus.clone());
    cx.update_window(window, |_, window, cx| first_focus.focus(window, cx))
        .unwrap();
    view.update(cx, |_, cx| cx.notify());
    cx.run_until_parked();
    cx.simulate_marked_text(window, None, "first", None);
    assert!(first_ime.marked.get());

    let second_focus = cx.read(|cx| view.read(cx).second_focus.clone());
    view.update(cx, |view, cx| {
        view.first_presentation = SubtreePresentation::Inert;
        cx.notify();
    });
    cx.update_window(window, |_, window, cx| second_focus.focus(window, cx))
        .unwrap();
    cx.run_until_parked();

    assert_eq!(first_ime.unmarks.get(), 1);
    assert!(!first_ime.marked.get());
    assert_eq!(second_ime.unmarks.get(), 0);
    assert!(platform_has_input_handler(cx, window));
    cx.simulate_marked_text(window, None, "second", None);
    assert_eq!(second_ime.marked_updates.get(), 1);
    assert!(second_ime.marked.get());

    view.update(cx, |view, cx| {
        view.first_presentation = SubtreePresentation::Visible;
        cx.notify();
    });
    cx.run_until_parked();
    assert_eq!(first_ime.unmarks.get(), 1);
    assert_eq!(second_ime.unmarks.get(), 0);
}

#[open_gpui::test]
fn input_cleanup_reselects_the_platform_handler_after_unmark_changes_focus(
    cx: &mut TestAppContext,
) {
    let first_ime = PresentationImeState::default();
    let second_ime = PresentationImeState::default();
    let typed_window = cx.open_window(size(px(320.0), px(200.0)), {
        let first_ime = first_ime.clone();
        let second_ime = second_ime.clone();
        move |_, cx| PresentationInputReplacementView {
            first_presentation: SubtreePresentation::Visible,
            claim_second_on_render: false,
            first_focus: cx.focus_handle(),
            second_focus: cx.focus_handle(),
            third_focus: cx.focus_handle(),
            first_ime,
            second_ime,
        }
    });
    let view = typed_window.root(cx).unwrap();
    let window = typed_window.into();
    cx.run_until_parked();

    let (first_focus, third_focus) = cx.read(|cx| {
        let view = view.read(cx);
        (view.first_focus.clone(), view.third_focus.clone())
    });
    cx.update_window(window, |_, window, cx| first_focus.focus(window, cx))
        .unwrap();
    view.update(cx, |_, cx| cx.notify());
    cx.run_until_parked();
    cx.simulate_marked_text(window, None, "first", None);
    assert!(first_ime.marked.get());
    *first_ime.focus_on_unmark.borrow_mut() = Some(third_focus.clone());

    view.update(cx, |view, cx| {
        view.first_presentation = SubtreePresentation::Inert;
        view.claim_second_on_render = true;
        cx.notify();
    });
    cx.run_until_parked();
    draw_focus_followup_frame(cx, window);

    assert!(
        !first_ime.platform_handler_present_on_unmark.get(),
        "the next-frame input handler must not be installed before old composition cleanup"
    );
    cx.update_window(window, |_, window, cx| {
        assert_eq!(window.focused(cx).as_ref(), Some(&third_focus));
        assert!(window.platform_window.take_input_handler().is_none());
    })
    .unwrap();
    assert_eq!(first_ime.unmarks.get(), 1);
    assert!(!first_ime.marked.get());
    assert_eq!(second_ime.unmarks.get(), 0);
}

struct CachedPresentationChild {
    renders: Rc<Cell<usize>>,
    clicks: Rc<Cell<usize>>,
}

impl Render for CachedPresentationChild {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        self.renders.set(self.renders.get() + 1);
        let clicks = self.clicks.clone();
        div()
            .id("cached-presentation-child")
            .role(Role::Button)
            .aria_label("Cached presentation child")
            .size_full()
            .on_mouse_down(MouseButton::Left, move |_, _, _| {
                clicks.set(clicks.get() + 1);
            })
    }
}

struct CachedPresentationRoot {
    presentation: SubtreePresentation,
    child: Entity<CachedPresentationChild>,
}

impl Render for CachedPresentationRoot {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        AnyView::from(self.child.clone())
            .cached(StyleRefinement::default().w(px(80.0)).h(px(40.0)))
            .with_subtree_presentation(self.presentation)
    }
}

#[open_gpui::test]
fn cached_child_is_rebuilt_when_only_ancestor_presentation_changes(cx: &mut TestAppContext) {
    let renders = Rc::new(Cell::new(0));
    let clicks = Rc::new(Cell::new(0));
    let typed_window = cx.open_window(size(px(320.0), px(200.0)), {
        let renders = renders.clone();
        let clicks = clicks.clone();
        move |_, cx| CachedPresentationRoot {
            presentation: SubtreePresentation::Visible,
            child: cx.new(|_| CachedPresentationChild { renders, clicks }),
        }
    });
    let root = typed_window.root(cx).unwrap();
    let window = typed_window.into();
    assert!(cx.activate_accessibility(window));

    let visible_renders = renders.get();
    cx.simulate_event(
        window,
        MouseDownEvent {
            button: MouseButton::Left,
            position: point(px(10.0), px(10.0)),
            modifiers: Modifiers::none(),
            click_count: 1,
            first_mouse: false,
        },
    );
    assert_eq!(clicks.get(), 1);

    root.update(cx, |root, cx| {
        root.presentation = SubtreePresentation::Inert;
        cx.notify();
    });
    cx.run_until_parked();
    assert!(renders.get() > visible_renders);
    let inert_renders = renders.get();
    let inert_update = cx.latest_accessibility_tree_update(window).unwrap();
    assert!(node_id_with_label(&inert_update, "Cached presentation child").is_none());

    cx.simulate_event(
        window,
        MouseDownEvent {
            button: MouseButton::Left,
            position: point(px(10.0), px(10.0)),
            modifiers: Modifiers::none(),
            click_count: 1,
            first_mouse: false,
        },
    );
    assert_eq!(clicks.get(), 1);

    root.update(cx, |root, cx| {
        root.presentation = SubtreePresentation::Hidden;
        cx.notify();
    });
    cx.run_until_parked();
    assert!(renders.get() > inert_renders);

    root.update(cx, |root, cx| {
        root.presentation = SubtreePresentation::Visible;
        cx.notify();
    });
    cx.run_until_parked();
    let restored_update = cx.latest_accessibility_tree_update(window).unwrap();
    assert!(node_id_with_label(&restored_update, "Cached presentation child").is_some());
}

#[derive(Default)]
struct DeferredPresentationCounters {
    deferred_prepaints: Cell<usize>,
    deferred_paints: Cell<usize>,
    portal_prepaints: Cell<usize>,
    portal_paints: Cell<usize>,
}

struct DeferredPresentationView {
    presentation: SubtreePresentation,
    counters: Rc<DeferredPresentationCounters>,
}

impl Render for DeferredPresentationView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let deferred_prepaints = self.counters.clone();
        let deferred_paints = self.counters.clone();
        let portal_prepaints = self.counters.clone();
        let portal_paints = self.counters.clone();

        div()
            .flex()
            .child(deferred(
                canvas(
                    move |bounds, window, _| {
                        deferred_prepaints
                            .deferred_prepaints
                            .set(deferred_prepaints.deferred_prepaints.get() + 1);
                        window.insert_hitbox(bounds, HitboxBehavior::Normal);
                    },
                    move |bounds, _, window, _| {
                        deferred_paints
                            .deferred_paints
                            .set(deferred_paints.deferred_paints.get() + 1);
                        window.paint_quad(fill(bounds, red()));
                    },
                )
                .w(px(40.0))
                .h(px(40.0)),
            ))
            .child(window_portal(
                canvas(
                    move |bounds, window, _| {
                        portal_prepaints
                            .portal_prepaints
                            .set(portal_prepaints.portal_prepaints.get() + 1);
                        window.insert_hitbox(bounds, HitboxBehavior::Normal);
                    },
                    move |bounds, _, window, _| {
                        portal_paints
                            .portal_paints
                            .set(portal_paints.portal_paints.get() + 1);
                        window.paint_quad(fill(bounds, red()));
                    },
                )
                .w(px(40.0))
                .h(px(40.0)),
            ))
            .with_subtree_presentation(self.presentation)
    }
}

#[open_gpui::test]
fn deferred_and_window_portal_inherit_presentation(cx: &mut TestAppContext) {
    let counters = Rc::new(DeferredPresentationCounters::default());
    let typed_window = cx.open_window(size(px(320.0), px(200.0)), {
        let counters = counters.clone();
        move |_, _| DeferredPresentationView {
            presentation: SubtreePresentation::Inert,
            counters,
        }
    });
    let view = typed_window.root(cx).unwrap();

    cx.run_until_parked();
    assert!(counters.deferred_prepaints.get() > 0);
    assert!(counters.deferred_paints.get() > 0);
    assert!(counters.portal_prepaints.get() > 0);
    assert!(counters.portal_paints.get() > 0);

    let deferred_prepaints = counters.deferred_prepaints.get();
    let deferred_paints = counters.deferred_paints.get();
    let portal_prepaints = counters.portal_prepaints.get();
    let portal_paints = counters.portal_paints.get();
    view.update(cx, |view, cx| {
        view.presentation = SubtreePresentation::Hidden;
        cx.notify();
    });
    cx.run_until_parked();

    assert_eq!(counters.deferred_prepaints.get(), deferred_prepaints);
    assert_eq!(counters.deferred_paints.get(), deferred_paints);
    assert_eq!(counters.portal_prepaints.get(), portal_prepaints);
    assert_eq!(counters.portal_paints.get(), portal_paints);
}

struct PresentationHoverView {
    presentation: SubtreePresentation,
    events: Rc<RefCell<Vec<bool>>>,
}

impl Render for PresentationHoverView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let events = self.events.clone();
        div()
            .id("presentation-hover-probe")
            .w(px(100.0))
            .h(px(100.0))
            .on_hover(move |hovered, _, _| events.borrow_mut().push(*hovered))
            .with_subtree_presentation(self.presentation)
    }
}

#[open_gpui::test]
fn inert_transition_emits_terminal_hover_without_replaying_on_restore(cx: &mut TestAppContext) {
    let events = Rc::new(RefCell::new(Vec::new()));
    let typed_window = cx.open_window(size(px(320.0), px(200.0)), {
        let events = events.clone();
        move |_, _| PresentationHoverView {
            presentation: SubtreePresentation::Visible,
            events,
        }
    });
    let view = typed_window.root(cx).unwrap();
    let window = typed_window.into();
    cx.run_until_parked();

    cx.simulate_event(
        window,
        MouseMoveEvent {
            position: point(px(10.0), px(10.0)),
            pressed_button: None,
            modifiers: Modifiers::none(),
        },
    );
    assert_eq!(events.borrow().as_slice(), &[true]);

    view.update(cx, |view, cx| {
        view.presentation = SubtreePresentation::Inert;
        cx.notify();
    });
    cx.run_until_parked();
    assert_eq!(events.borrow().as_slice(), &[true, false]);

    view.update(cx, |view, cx| {
        view.presentation = SubtreePresentation::Visible;
        cx.notify();
    });
    cx.run_until_parked();
    assert_eq!(
        events.borrow().as_slice(),
        &[true, false],
        "restoring a stationary pointer must not replay stale hover intent"
    );
}

struct PresentationScrollView {
    presentation: SubtreePresentation,
    handle: ScrollHandle,
    events: Rc<Cell<usize>>,
}

impl Render for PresentationScrollView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let events = self.events.clone();
        div()
            .w(px(100.0))
            .h(px(100.0))
            .overflow_y_scroll()
            .track_scroll(&self.handle)
            .on_scroll_viewport_changed(move |_, _, _| events.set(events.get() + 1))
            .child(div().w(px(100.0)).h(px(300.0)))
            .with_subtree_presentation(self.presentation)
    }
}

#[open_gpui::test]
fn inert_scroll_changes_are_consumed_without_visible_replay(cx: &mut TestAppContext) {
    let events = Rc::new(Cell::new(0));
    let typed_window = cx.open_window(size(px(320.0), px(200.0)), {
        let events = events.clone();
        move |_, _| PresentationScrollView {
            presentation: SubtreePresentation::Visible,
            handle: ScrollHandle::new(),
            events,
        }
    });
    let view = typed_window.root(cx).unwrap();
    cx.run_until_parked();
    let initial_events = events.get();

    view.update(cx, |view, cx| {
        view.presentation = SubtreePresentation::Inert;
        view.handle.set_offset(point(px(0.0), px(-50.0)));
        cx.notify();
    });
    cx.run_until_parked();
    assert_eq!(events.get(), initial_events);

    view.update(cx, |view, cx| {
        view.presentation = SubtreePresentation::Visible;
        cx.notify();
    });
    cx.run_until_parked();
    assert_eq!(
        events.get(),
        initial_events,
        "a viewport change committed while inert must not replay after restoration"
    );
}

struct CachedPresentationInteractionChild {
    renders: Rc<Cell<usize>>,
    hover_events: Rc<RefCell<Vec<bool>>>,
    scroll_events: Rc<Cell<usize>>,
    scroll: ScrollHandle,
}

impl Render for CachedPresentationInteractionChild {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        self.renders.set(self.renders.get() + 1);
        let hover_events = self.hover_events.clone();
        let scroll_events = self.scroll_events.clone();
        div()
            .id("cached-presentation-interaction")
            .w(px(100.0))
            .h(px(100.0))
            .overflow_y_scroll()
            .track_scroll(&self.scroll)
            .on_hover(move |hovered, _, _| hover_events.borrow_mut().push(*hovered))
            .on_scroll_viewport_changed(move |_, _, _| scroll_events.set(scroll_events.get() + 1))
            .child(div().w(px(100.0)).h(px(300.0)))
    }
}

struct CachedPresentationInteractionRoot {
    presentation: SubtreePresentation,
    child: Entity<CachedPresentationInteractionChild>,
}

impl Render for CachedPresentationInteractionRoot {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        AnyView::from(self.child.clone())
            .cached(StyleRefinement::default().w(px(100.0)).h(px(100.0)))
            .with_subtree_presentation(self.presentation)
    }
}

#[open_gpui::test]
fn hidden_cached_child_reconciles_hover_and_scroll_without_replaying_on_restore(
    cx: &mut TestAppContext,
) {
    let renders = Rc::new(Cell::new(0));
    let hover_events = Rc::new(RefCell::new(Vec::new()));
    let scroll_events = Rc::new(Cell::new(0));
    let scroll = ScrollHandle::new();
    let typed_window = cx.open_window(size(px(320.0), px(200.0)), {
        let renders = renders.clone();
        let hover_events = hover_events.clone();
        let scroll_events = scroll_events.clone();
        let scroll = scroll.clone();
        move |_, cx| CachedPresentationInteractionRoot {
            presentation: SubtreePresentation::Visible,
            child: cx.new(|_| CachedPresentationInteractionChild {
                renders,
                hover_events,
                scroll_events,
                scroll,
            }),
        }
    });
    let root = typed_window.root(cx).unwrap();
    let window = typed_window.into();
    cx.run_until_parked();

    let first_render_count = renders.get();
    cx.update_window(window, |_, window, cx| window.draw(cx).clear())
        .unwrap();
    assert_eq!(
        renders.get(),
        first_render_count,
        "the unchanged child must actually use cached journal replay"
    );

    cx.simulate_event(
        window,
        MouseMoveEvent {
            position: point(px(10.0), px(10.0)),
            pressed_button: None,
            modifiers: Modifiers::none(),
        },
    );
    assert_eq!(hover_events.borrow().as_slice(), &[true]);
    let initial_scroll_events = scroll_events.get();

    root.update(cx, |root, cx| {
        root.presentation = SubtreePresentation::Hidden;
        scroll.set_offset(point(px(0.0), px(-50.0)));
        cx.notify();
    });
    cx.run_until_parked();
    assert_eq!(hover_events.borrow().as_slice(), &[true, false]);
    assert_eq!(scroll_events.get(), initial_scroll_events);

    root.update(cx, |root, cx| {
        root.presentation = SubtreePresentation::Visible;
        cx.notify();
    });
    cx.run_until_parked();
    assert_eq!(
        hover_events.borrow().as_slice(),
        &[true, false],
        "a stationary pointer must not revive cached hover state"
    );
    assert_eq!(
        scroll_events.get(),
        initial_scroll_events,
        "a hidden viewport change must not replay after restoration"
    );
}

struct PresentationDragPreview {
    renders: Rc<Cell<usize>>,
}

impl Render for PresentationDragPreview {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        self.renders.set(self.renders.get() + 1);
        div().w(px(1.0)).h(px(1.0))
    }
}

struct PresentationDragView {
    presentation: SubtreePresentation,
    capture: PointerCaptureHandle,
    preview_renders: Rc<Cell<usize>>,
    mouse_downs: Rc<Cell<usize>>,
    drag_starts: Rc<Cell<usize>>,
}

impl Render for PresentationDragView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let preview_renders = self.preview_renders.clone();
        let mouse_downs = self.mouse_downs.clone();
        let drag_starts = self.drag_starts.clone();
        div()
            .id("presentation-drag-source")
            .w(px(100.0))
            .h(px(100.0))
            .track_pointer_capture(&self.capture)
            .on_mouse_down(MouseButton::Left, move |_, _, _| {
                mouse_downs.set(mouse_downs.get() + 1)
            })
            .on_drag(7_u32, move |_, _, _, cx| {
                drag_starts.set(drag_starts.get() + 1);
                let renders = preview_renders.clone();
                cx.new(|_| PresentationDragPreview { renders })
            })
            .with_subtree_presentation(self.presentation)
    }
}

#[open_gpui::test]
fn changing_drag_source_to_inert_cancels_the_owned_pointer_session_once(cx: &mut TestAppContext) {
    let cancellations = Rc::new(RefCell::new(Vec::new()));
    let preview_renders = Rc::new(Cell::new(0));
    let mouse_downs = Rc::new(Cell::new(0));
    let drag_starts = Rc::new(Cell::new(0));
    let typed_window = cx.open_window(size(px(320.0), px(200.0)), {
        let preview_renders = preview_renders.clone();
        let mouse_downs = mouse_downs.clone();
        let drag_starts = drag_starts.clone();
        move |window, _| PresentationDragView {
            presentation: SubtreePresentation::Visible,
            capture: window.new_pointer_capture_handle(),
            preview_renders,
            mouse_downs,
            drag_starts,
        }
    });
    let view = typed_window.root(cx).unwrap();
    let window = typed_window.into();
    let _subscription = cx
        .update_window(window, |_, window, _| {
            let cancellations = cancellations.clone();
            window.intercept_window_mouse_events(move |event, _, _| {
                if let WindowMouseEvent::Cancel(event) = event {
                    cancellations.borrow_mut().push(event.reason);
                }
            })
        })
        .unwrap();
    cx.update_window(window, |_, window, cx| {
        window.activate_window();
        window.draw(cx).clear();
    })
    .unwrap();
    cx.run_until_parked();
    let frame_facts = cx
        .update_window(window, |_, window, _| {
            (
                window.rendered_frame.hitboxes.len(),
                window.rendered_frame.pointer_capture_bindings.len(),
                window
                    .rendered_frame
                    .hit_test(point(px(10.0), px(10.0)))
                    .ids
                    .len(),
            )
        })
        .unwrap();
    assert!(
        frame_facts.0 > 0 && frame_facts.1 > 0 && frame_facts.2 > 0,
        "visible drag source must publish a hittable capture binding: {frame_facts:?}"
    );

    let capture = cx.read(|cx| view.read(cx).capture);
    let manual_capture = cx
        .update_window(window, |_, window, cx| {
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
            let result = window.capture_pointer(&capture, MouseButton::Left);
            if result.is_ok() {
                window.release_pointer(&capture).unwrap();
            }
            result
        })
        .unwrap();
    assert_eq!(manual_capture, Ok(()));
    cx.update_window(window, |_, window, cx| {
        window.dispatch_event(
            PlatformInput::MouseMove(MouseMoveEvent {
                position: point(px(30.0), px(30.0)),
                pressed_button: Some(MouseButton::Left),
                modifiers: Modifiers::none(),
            }),
            cx,
        );
    })
    .unwrap();
    assert_eq!(
        mouse_downs.get(),
        1,
        "the drag source must receive mouse down"
    );
    assert_eq!(
        drag_starts.get(),
        1,
        "the drag threshold must start the drag"
    );
    assert!(
        cx.update_window(window, |_, window, _| window.captured_pointer().is_some())
            .unwrap()
    );
    assert!(cx.read(|cx| cx.active_drag.is_some()));
    cx.run_until_parked();
    assert!(
        cx.update_window(window, |_, window, _| window.captured_pointer().is_some())
            .unwrap(),
        "a visible redraw must preserve the drag source binding"
    );
    assert!(cx.read(|cx| cx.active_drag.is_some()));
    let preview_renders_before_suppression = preview_renders.get();

    view.update(cx, |view, cx| {
        view.presentation = SubtreePresentation::Inert;
        cx.notify();
    });
    cx.update_window(window, |_, window, cx| window.draw(cx).clear())
        .unwrap();
    assert!(
        cx.update_window(window, |_, window, _| window.captured_pointer().is_none())
            .unwrap()
    );
    assert!(cx.read(|cx| cx.active_drag.is_none()));
    assert_eq!(
        preview_renders.get(),
        preview_renders_before_suppression,
        "a revoked drag source must not paint one final preview frame"
    );
    cx.run_until_parked();
    assert_eq!(
        cancellations.borrow().as_slice(),
        &[PointerCancelReason::CaptureRevoked]
    );

    view.update(cx, |view, cx| {
        view.presentation = SubtreePresentation::Visible;
        cx.notify();
    });
    cx.run_until_parked();
    assert!(cx.read(|cx| cx.active_drag.is_none()));
    assert_eq!(cancellations.borrow().len(), 1);
}
