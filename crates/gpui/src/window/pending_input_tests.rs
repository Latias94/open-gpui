//! Pending-input regressions from Zed 635026fabc38440fce5e1767d125026a17b12012.
//!
//! Upstream fixes:
//! - bc7041c600e3da82713849f4d65aa4a3cac89f67
//! - ba42ab9d9851f82a7d8f5ae40b941bf5c515fc10
//! - f66ed399cdde86092af8af3dc7b418abf45f37f8
//! Direct prerequisites:
//! - dc1e815e47835095748cbb036541992abd9ac826
//! - a8cae3bd77f6e1c1dde98bb1dcebba0d254dbd71
//! - e463c5ab74e016f6a16e02a8b21925b98643f9c7
//!
//! Source mapping (paths under crates/gpui/src in the upstream repository):
//! - window.rs: standalone timeout/recovery and completion before timeout (bc7041c600).
//! - key_dispatch.rs: six modifier interception/observation cases (ba42ab9d98).
//! - interactive.rs: multi-modifier gesture recovery (a8cae3bd77).
//! - key_dispatch.rs: complete InputHandler pending/IME test (dc1e815e47, f66ed399cd).
//! - key_dispatch.rs: focus/blur/drop notifications (e463c5ab74, f66ed399cd).
//!
//! Fork adaptations use rendered sibling targets, accepted candidate transitions, leased native
//! input callbacks and exact window retirement. No target-only Keystroke or pending-input
//! status/pause API is required.

use super::cached_paint_atlas_tests::RejectOnceAtlas;
use crate::{
    Action, App, AppContext as _, AtlasTextureKind, AtlasTile, Bounds, Context, DispatchPhase,
    Element, ElementId, Entity, FocusHandle, GlobalElementId, InputHandler, InspectorElementId,
    InteractiveElement as _, IntoElement, KeyBinding, KeyContext, Keystroke, LayoutId, Modifiers,
    MonochromeSprite, ParentElement as _, Pixels, Point, PrimitiveTransform, Render, Style,
    Styled as _, Subscription, SubtreePresentation, SubtreePresentationExt as _, TestAppContext,
    UTF16Selection, VisualContext as _, VisualTestContext, Window, canvas, div, red,
};
use std::{
    cell::{Cell, RefCell},
    ops::Range,
    panic,
    rc::Rc,
    sync::Arc,
    time::Duration,
};

crate::actions!(
    pending_input_regression,
    [TestAction, SecondaryTestAction, StandaloneModifierAction]
);

const PENDING_INPUT_TIMEOUT: Duration = Duration::from_secs(1);

fn close_window(cx: &mut VisualTestContext) {
    let handle = cx.window_handle();
    cx.update(|window, cx| window.remove_window(cx));
    cx.run_until_parked();
    assert!(!cx.windows().contains(&handle));
    assert!(cx.cx.update_window(handle, |_, _, _| ()).is_err());
}

struct StandaloneModifierView {
    focus_handle: FocusHandle,
    action_count: usize,
}

impl Render for StandaloneModifierView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .key_context("StandaloneModifier")
            .track_focus(&self.focus_handle)
            .on_action(cx.listener(|this, _: &StandaloneModifierAction, _, _| {
                this.action_count += 1;
            }))
    }
}

#[crate::test]
fn test_standalone_modifier_sequence_timeout_and_recovery(cx: &mut TestAppContext) {
    cx.update(|cx| {
        cx.bind_keys([KeyBinding::new(
            "shift shift",
            StandaloneModifierAction,
            Some("StandaloneModifier"),
        )]);
    });
    let (view, cx) = cx.add_window_view(|window, cx| {
        let focus_handle = cx.focus_handle();
        window.focus(&focus_handle, cx);
        StandaloneModifierView {
            focus_handle,
            action_count: 0,
        }
    });
    let shift = Modifiers::shift();
    simulate_modifier_tap(cx, shift);
    cx.update(|window, _| {
        assert_eq!(
            window.pending_input_keystrokes(),
            Some([Keystroke::parse("shift").expect("valid keystroke")].as_slice())
        );
    });
    cx.background_executor.advance_clock(Duration::from_secs(2));
    cx.run_until_parked();
    cx.update(|window, cx| {
        assert!(!window.has_pending_keystrokes());
        assert_eq!(view.read(cx).action_count, 0);
    });

    simulate_modifier_tap(cx, shift);
    cx.update(|window, cx| {
        assert!(window.has_pending_keystrokes());
        assert_eq!(view.read(cx).action_count, 0);
    });

    simulate_modifier_tap(cx, shift);
    cx.update(|window, cx| {
        assert!(!window.has_pending_keystrokes());
        assert_eq!(view.read(cx).action_count, 1);
    });
    close_window(cx);
}

#[crate::test]
fn test_standalone_modifier_sequence_dispatches_before_timeout(cx: &mut TestAppContext) {
    cx.update(|cx| {
        cx.bind_keys([KeyBinding::new(
            "shift shift",
            StandaloneModifierAction,
            Some("StandaloneModifier"),
        )]);
    });
    let (view, cx) = cx.add_window_view(|window, cx| {
        let focus_handle = cx.focus_handle();
        window.focus(&focus_handle, cx);
        StandaloneModifierView {
            focus_handle,
            action_count: 0,
        }
    });
    let shift = Modifiers::shift();
    simulate_modifier_tap(cx, shift);
    cx.background_executor
        .advance_clock(Duration::from_millis(500));
    cx.update(|window, cx| {
        assert!(window.has_pending_keystrokes());
        assert_eq!(view.read(cx).action_count, 0);
    });
    simulate_modifier_tap(cx, shift);
    cx.update(|window, cx| {
        assert!(!window.has_pending_keystrokes());
        assert_eq!(view.read(cx).action_count, 1);
    });
    cx.background_executor.advance_clock(Duration::from_secs(2));
    cx.run_until_parked();
    cx.update(|window, cx| {
        assert!(!window.has_pending_keystrokes());
        assert_eq!(view.read(cx).action_count, 1);
    });
    close_window(cx);
}

struct PendingInputTestView {
    focus_handle: FocusHandle,
    action_count: Rc<Cell<usize>>,
    secondary_action_count: Rc<Cell<usize>>,
}

impl Render for PendingInputTestView {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let action_count = self.action_count.clone();
        let secondary_action_count = self.secondary_action_count.clone();
        div()
            .key_context("Terminal")
            .track_focus(&self.focus_handle)
            .size_full()
            .on_action(move |_: &TestAction, _, _| {
                action_count.set(action_count.get() + 1);
            })
            .on_action(move |_: &SecondaryTestAction, _, _| {
                secondary_action_count.set(secondary_action_count.get() + 1);
            })
    }
}

fn setup_pending_input_test(
    cx: &mut TestAppContext,
    bindings: impl IntoIterator<Item = KeyBinding>,
) -> (&mut VisualTestContext, Rc<Cell<usize>>, Rc<Cell<usize>>) {
    cx.update(|cx| cx.bind_keys(bindings));
    let action_count = Rc::new(Cell::new(0));
    let secondary_action_count = Rc::new(Cell::new(0));
    let (view, cx) = cx.add_window_view(|_, cx| PendingInputTestView {
        focus_handle: cx.focus_handle(),
        action_count: action_count.clone(),
        secondary_action_count: secondary_action_count.clone(),
    });
    cx.update(|window, cx| {
        window.focus(&view.read(cx).focus_handle.clone(), cx);
        let _ = window.activate_window();
    });
    (cx, action_count, secondary_action_count)
}

#[derive(Debug, PartialEq)]
struct ObservedKeystroke {
    keystroke: Keystroke,
    action_name: Option<&'static str>,
    context_stack: Vec<KeyContext>,
}

fn capture_observed_keystrokes(
    cx: &mut VisualTestContext,
) -> (Rc<RefCell<Vec<ObservedKeystroke>>>, Subscription) {
    let observed_keystrokes = Rc::new(RefCell::new(Vec::new()));
    let subscription = cx.update(|_, cx| {
        cx.observe_keystrokes({
            let observed_keystrokes = observed_keystrokes.clone();
            move |event, _, _| {
                observed_keystrokes.borrow_mut().push(ObservedKeystroke {
                    keystroke: event.keystroke.clone(),
                    action_name: event.action.as_ref().map(|action| action.name()),
                    context_stack: event.context_stack.clone(),
                });
            }
        })
    });
    (observed_keystrokes, subscription)
}

fn simulate_modifier_tap(cx: &mut VisualTestContext, modifiers: Modifiers) {
    cx.simulate_modifiers_change(modifiers);
    cx.simulate_modifiers_change(Modifiers::none());
}

fn terminal_context() -> KeyContext {
    KeyContext::parse("Terminal").expect("valid key context")
}

#[crate::test]
fn test_standalone_modifier_dispatch(cx: &mut TestAppContext) {
    let (cx, action_count, _) =
        setup_pending_input_test(cx, [KeyBinding::new("shift", TestAction, Some("Terminal"))]);
    let intercepted_keystrokes = Rc::new(RefCell::new(Vec::new()));
    let should_consume = Rc::new(Cell::new(true));
    let terminal_context = terminal_context();
    let (observed_keystrokes, _observer) = capture_observed_keystrokes(cx);
    let _interceptor = cx.update(|_, cx| {
        cx.intercept_keystrokes({
            let intercepted_keystrokes = intercepted_keystrokes.clone();
            let should_consume = should_consume.clone();
            let terminal_context = terminal_context.clone();
            move |event, _, cx| {
                assert!(event.action.is_none());
                assert_eq!(event.context_stack, vec![terminal_context.clone()]);
                intercepted_keystrokes
                    .borrow_mut()
                    .push(event.keystroke.clone());
                if should_consume.get() {
                    cx.stop_propagation();
                }
            }
        })
    });
    let shift = Keystroke::parse("shift").expect("valid keystroke");

    cx.simulate_modifiers_change(Modifiers::shift());
    assert!(intercepted_keystrokes.borrow().is_empty());
    cx.simulate_modifiers_change(Modifiers::none());
    assert_eq!(
        intercepted_keystrokes.borrow().as_slice(),
        std::slice::from_ref(&shift)
    );
    assert_eq!(action_count.get(), 0);
    assert!(observed_keystrokes.borrow().is_empty());

    should_consume.set(false);
    intercepted_keystrokes.borrow_mut().clear();
    cx.simulate_modifiers_change(Modifiers::shift());
    assert!(intercepted_keystrokes.borrow().is_empty());
    cx.simulate_modifiers_change(Modifiers::none());
    assert_eq!(
        intercepted_keystrokes.borrow().as_slice(),
        std::slice::from_ref(&shift)
    );
    assert_eq!(action_count.get(), 1);
    assert_eq!(
        observed_keystrokes.borrow().as_slice(),
        &[ObservedKeystroke {
            keystroke: shift,
            action_name: Some(TestAction.name()),
            context_stack: vec![terminal_context],
        }]
    );
    close_window(cx);
}

#[crate::test]
fn test_unbound_standalone_modifier_observation(cx: &mut TestAppContext) {
    let (cx, _, _) = setup_pending_input_test(cx, []);
    let terminal_context = terminal_context();
    let (observed_keystrokes, _observer) = capture_observed_keystrokes(cx);
    for (modifiers, key) in [
        (Modifiers::shift(), "shift"),
        (Modifiers::control(), "ctrl"),
        (Modifiers::alt(), "alt"),
        (Modifiers::command(), "cmd"),
        (Modifiers::function(), "fn"),
    ] {
        observed_keystrokes.borrow_mut().clear();
        cx.simulate_modifiers_change(modifiers);
        assert!(
            observed_keystrokes.borrow().is_empty(),
            "modifier press must not notify observers"
        );
        cx.simulate_modifiers_change(Modifiers::none());
        assert_eq!(
            observed_keystrokes.borrow().as_slice(),
            &[ObservedKeystroke {
                keystroke: Keystroke::parse(key).expect("valid modifier"),
                action_name: None,
                context_stack: vec![terminal_context.clone()],
            }]
        );
    }
    close_window(cx);
}

#[crate::test]
fn test_raw_modifier_handler_can_suppress_observation(cx: &mut TestAppContext) {
    struct ModifierListenerTestView {
        focus_handle: FocusHandle,
        consume: Rc<Cell<bool>>,
        releases: Rc<Cell<usize>>,
    }

    impl Render for ModifierListenerTestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let consume = self.consume.clone();
            let releases = self.releases.clone();
            div()
                .key_context("Terminal")
                .track_focus(&self.focus_handle)
                .size_full()
                .on_action(|_: &TestAction, _, cx| cx.propagate())
                .on_modifiers_changed(move |event, _, cx| {
                    if !event.modifiers.modified() {
                        releases.set(releases.get() + 1);
                        if consume.get() {
                            cx.stop_propagation();
                        }
                    }
                })
        }
    }

    cx.update(|cx| cx.bind_keys([KeyBinding::new("shift", TestAction, Some("Terminal"))]));
    let consume = Rc::new(Cell::new(true));
    let releases = Rc::new(Cell::new(0));
    let (view, cx) = cx.add_window_view(|_, cx| ModifierListenerTestView {
        focus_handle: cx.focus_handle(),
        consume: consume.clone(),
        releases: releases.clone(),
    });
    cx.update(|window, cx| {
        window.focus(&view.read(cx).focus_handle.clone(), cx);
        let _ = window.activate_window();
    });
    let (observed_keystrokes, _observer) = capture_observed_keystrokes(cx);

    simulate_modifier_tap(cx, Modifiers::shift());
    assert_eq!(releases.get(), 1);
    assert!(observed_keystrokes.borrow().is_empty());

    consume.set(false);
    simulate_modifier_tap(cx, Modifiers::shift());
    assert_eq!(releases.get(), 2);
    assert_eq!(
        observed_keystrokes.borrow().as_slice(),
        &[ObservedKeystroke {
            keystroke: Keystroke::parse("shift").expect("valid modifier"),
            action_name: None,
            context_stack: vec![terminal_context()],
        }]
    );
    close_window(cx);
}

#[crate::test]
fn test_pending_modifier_observed_when_binding_resolves(cx: &mut TestAppContext) {
    let (cx, action_count, secondary_action_count) = setup_pending_input_test(
        cx,
        [
            KeyBinding::new("shift", TestAction, Some("Terminal")),
            KeyBinding::new("shift f1", SecondaryTestAction, Some("Terminal")),
        ],
    );
    let (observed_keystrokes, _observer) = capture_observed_keystrokes(cx);
    simulate_modifier_tap(cx, Modifiers::shift());
    assert!(observed_keystrokes.borrow().is_empty());
    assert_eq!(action_count.get(), 0);

    cx.executor().advance_clock(PENDING_INPUT_TIMEOUT);
    cx.run_until_parked();
    assert_eq!(action_count.get(), 1);
    assert_eq!(
        observed_keystrokes.borrow().as_slice(),
        &[ObservedKeystroke {
            keystroke: Keystroke::parse("shift").expect("valid modifier"),
            action_name: Some(TestAction.name()),
            context_stack: Vec::new(),
        }]
    );

    observed_keystrokes.borrow_mut().clear();
    simulate_modifier_tap(cx, Modifiers::shift());
    assert!(observed_keystrokes.borrow().is_empty());
    cx.simulate_keystrokes("f1");
    assert_eq!(action_count.get(), 1);
    assert_eq!(secondary_action_count.get(), 1);
    assert_eq!(
        observed_keystrokes.borrow().as_slice(),
        &[ObservedKeystroke {
            keystroke: Keystroke::parse("f1").expect("valid key"),
            action_name: Some(SecondaryTestAction.name()),
            context_stack: vec![terminal_context()],
        }]
    );
    close_window(cx);
}

#[crate::test]
fn test_combined_shortcut_has_no_standalone_modifier(cx: &mut TestAppContext) {
    let (cx, action_count, _) = setup_pending_input_test(
        cx,
        [
            KeyBinding::new("shift", TestAction, Some("Terminal")),
            KeyBinding::new("shift-f1", TestAction, Some("Terminal")),
        ],
    );
    let intercepted_keystrokes = Rc::new(RefCell::new(Vec::new()));
    let (observed_keystrokes, _observer) = capture_observed_keystrokes(cx);
    let _interceptor = cx.update(|_, cx| {
        cx.intercept_keystrokes({
            let intercepted_keystrokes = intercepted_keystrokes.clone();
            move |event, _, _| {
                intercepted_keystrokes
                    .borrow_mut()
                    .push(event.keystroke.clone());
            }
        })
    });

    cx.simulate_modifiers_change(Modifiers::shift());
    assert!(intercepted_keystrokes.borrow().is_empty());
    cx.simulate_keystrokes("shift-f1");
    cx.simulate_modifiers_change(Modifiers::none());
    let shift_f1 = Keystroke::parse("shift-f1").expect("valid keystroke");
    assert_eq!(
        intercepted_keystrokes.borrow().as_slice(),
        std::slice::from_ref(&shift_f1)
    );
    assert_eq!(
        observed_keystrokes.borrow().as_slice(),
        &[ObservedKeystroke {
            keystroke: shift_f1,
            action_name: Some(TestAction.name()),
            context_stack: vec![terminal_context()],
        }]
    );
    assert_eq!(action_count.get(), 1);
    close_window(cx);
}

#[crate::test]
fn test_multi_modifier_gesture_does_not_dispatch_standalone_modifier_binding(
    cx: &mut TestAppContext,
) {
    let (cx, action_count, _) =
        setup_pending_input_test(cx, [KeyBinding::new("shift", TestAction, None)]);
    cx.simulate_modifiers_change(Modifiers::alt());
    cx.simulate_modifiers_change(Modifiers::alt() | Modifiers::shift());
    cx.simulate_modifiers_change(Modifiers::shift());
    cx.simulate_modifiers_change(Modifiers::none());
    assert_eq!(action_count.get(), 0);

    cx.simulate_modifiers_change(Modifiers::shift());
    cx.simulate_modifiers_change(Modifiers::none());
    assert_eq!(action_count.get(), 1);
    close_window(cx);
}

#[derive(Clone)]
struct PendingTextInputTestView {
    focus_handle: FocusHandle,
    key_context: KeyContext,
    text: Rc<RefCell<String>>,
    action_count: Rc<Cell<usize>>,
}

#[crate::test]
fn test_timeout_action_that_moves_focus_is_still_observed(cx: &mut TestAppContext) {
    struct FocusMovingAction {
        source: FocusHandle,
        target: FocusHandle,
        actions: Rc<Cell<usize>>,
    }

    impl Render for FocusMovingAction {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let target = self.target.clone();
            let actions = self.actions.clone();
            div()
                .key_context("Terminal")
                .child(
                    div()
                        .id("action-source")
                        .track_focus(&self.source)
                        .on_action(move |_: &TestAction, window, cx| {
                            actions.set(actions.get() + 1);
                            window.focus(&target, cx);
                        }),
                )
                .child(div().id("action-target").track_focus(&self.target))
        }
    }

    cx.update(|cx| {
        cx.bind_keys([
            KeyBinding::new("shift", TestAction, Some("Terminal")),
            KeyBinding::new("shift f1", SecondaryTestAction, Some("Terminal")),
        ]);
    });
    let actions = Rc::new(Cell::new(0));
    let (root, cx) = cx.add_window_view(|_, cx| FocusMovingAction {
        source: cx.focus_handle(),
        target: cx.focus_handle(),
        actions: actions.clone(),
    });
    let target = root.update_in(cx, |root, window, cx| {
        window.focus(&root.source, cx);
        let _ = window.activate_window();
        root.target.clone()
    });
    let (observed, _observer) = capture_observed_keystrokes(cx);
    simulate_modifier_tap(cx, Modifiers::shift());
    assert!(observed.borrow().is_empty());
    assert_eq!(actions.get(), 0);
    cx.executor().advance_clock(PENDING_INPUT_TIMEOUT);
    cx.run_until_parked();
    assert_eq!(actions.get(), 1);
    assert_eq!(
        observed.borrow().as_slice(),
        &[ObservedKeystroke {
            keystroke: Keystroke::parse("shift").expect("valid standalone modifier"),
            action_name: Some(TestAction.name()),
            context_stack: Vec::new(),
        }],
    );
    cx.update(|window, cx| {
        assert_eq!(window.committed_focus(cx), Some(target));
        assert!(window.pending_input.is_none());
    });
    cx.executor().advance_clock(PENDING_INPUT_TIMEOUT);
    cx.run_until_parked();
    assert_eq!(actions.get(), 1);
    assert_eq!(observed.borrow().len(), 1);
    close_window(cx);
}

impl PendingTextInputTestView {
    fn new(cx: &mut App) -> Self {
        Self {
            focus_handle: cx.focus_handle(),
            key_context: terminal_context(),
            text: Rc::default(),
            action_count: Rc::default(),
        }
    }
}

impl Element for PendingTextInputTestView {
    type RequestLayoutState = ();
    type PrepaintState = ();

    fn id(&self) -> Option<ElementId> {
        Some("pending-text-input-test".into())
    }

    fn source_location(&self) -> Option<&'static panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        (window.request_layout(Style::default(), [], cx), ())
    }

    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        _: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        window.set_focus_handle(&self.focus_handle, cx);
    }

    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        _: &mut Self::RequestLayoutState,
        _: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        window.set_key_context(self.key_context.clone());
        window.handle_input(&self.focus_handle, self.clone(), cx);
        let action_count = self.action_count.clone();
        window.on_action(
            std::any::TypeId::of::<TestAction>(),
            move |_, phase, _, _| {
                if phase == DispatchPhase::Bubble {
                    action_count.set(action_count.get() + 1);
                }
            },
        );
    }
}

impl IntoElement for PendingTextInputTestView {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Render for PendingTextInputTestView {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        self.clone()
    }
}

impl InputHandler for PendingTextInputTestView {
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
        replacement_range: Option<Range<usize>>,
        text: &str,
        _: &mut Window,
        _: &mut App,
    ) {
        assert!(replacement_range.is_none());
        self.text.borrow_mut().push_str(text);
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        replacement_range: Option<Range<usize>>,
        text: &str,
        _: Option<Range<usize>>,
        _: &mut Window,
        _: &mut App,
    ) {
        assert!(replacement_range.is_none());
        self.text.borrow_mut().push_str(text);
    }

    fn unmark_text(&mut self, _: &mut Window, _: &mut App) {}

    fn prefers_ime_for_printable_keys(&mut self, _: &mut Window, _: &mut App) -> bool {
        true
    }

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

fn query_prefers_ime_for_printable_keys(cx: &mut VisualTestContext) -> Option<bool> {
    let slot = cx.update(|window, _| {
        window
            .platform_window
            .input_handler_slot_for_test()
            .expect("test input-handler slot")
    });
    slot.with_handler(|handler| handler.query_prefers_ime_for_printable_keys())
}

#[crate::test]
fn test_consumed_modifier_preserves_pending_input(cx: &mut TestAppContext) {
    let (test, cx) = cx.add_window_view(|_, cx| PendingTextInputTestView::new(cx));
    test.update_in(cx, |test, window, cx| {
        window.focus(&test.focus_handle, cx);
        let _ = window.activate_window();
    });
    let _subscription = cx.update(|_, cx| {
        cx.intercept_keystrokes(|event, _, cx| {
            if event.keystroke.key == "shift" {
                cx.stop_propagation();
            }
        })
    });

    for binding in ["j shift", "j k"] {
        cx.update(|_, cx| {
            cx.clear_key_bindings();
            cx.bind_keys([KeyBinding::new(binding, TestAction, Some("Terminal"))]);
        });
        test.update(cx, |test, _| test.text.borrow_mut().clear());
        cx.simulate_keystrokes("j");
        cx.executor().advance_clock(PENDING_INPUT_TIMEOUT / 2);
        cx.run_until_parked();
        let pending_keystrokes = cx.update(|window, _| {
            window
                .pending_input_keystrokes()
                .expect("pending input")
                .to_vec()
        });

        simulate_modifier_tap(cx, Modifiers::shift());
        cx.update(|window, _| {
            let pending = window.pending_input.as_ref().expect("pending input");
            assert_eq!(pending.keystrokes.as_slice(), pending_keystrokes.as_slice());
            assert!(pending.timer.is_some());
        });
        test.update(cx, |test, _| {
            assert_eq!(test.action_count.get(), 0);
            assert_eq!(test.text.borrow().as_str(), "");
        });

        cx.executor().advance_clock(PENDING_INPUT_TIMEOUT / 2);
        cx.run_until_parked();
        cx.update(|window, _| assert!(!window.has_pending_keystrokes()));
        test.update(cx, |test, _| {
            assert_eq!(test.action_count.get(), 0);
            assert_eq!(test.text.borrow().as_str(), "j");
        });
    }
    close_window(cx);
}

#[crate::test]
fn test_input_handler_pending(cx: &mut TestAppContext) {
    cx.update(|cx| {
        cx.bind_keys([
            KeyBinding::new("ctrl-b", TestAction, Some("Terminal")),
            KeyBinding::new("ctrl-b h", TestAction, Some("Terminal")),
            KeyBinding::new("ctrl-x k", TestAction, Some("Terminal")),
        ]);
    });
    let (test, cx) = cx.add_window_view(|_, cx| PendingTextInputTestView::new(cx));
    let focus_handle = test.update(cx, |test, _| test.focus_handle.clone());
    cx.update(|window, cx| {
        window.focus(&focus_handle, cx);
        let _ = window.activate_window();
    });

    assert_eq!(query_prefers_ime_for_printable_keys(cx), Some(true));
    cx.simulate_keystrokes("ctrl-x");
    cx.update(|window, _| assert!(window.has_pending_keystrokes()));
    assert_eq!(query_prefers_ime_for_printable_keys(cx), Some(false));

    let slot = cx.update(|window, _| {
        window
            .platform_window
            .input_handler_slot_for_test()
            .expect("test input-handler slot")
    });
    let prefers_ime_after_blur = slot.with_handler(|input_handler| {
        cx.update(|window, cx| window.blur(cx));
        cx.run_until_parked();
        cx.update(|window, cx| {
            assert!(window.focused(cx).is_none());
            assert!(!window.has_pending_keystrokes());
            assert!(window.pending_input.is_none());
        });
        input_handler.query_prefers_ime_for_printable_keys()
    });
    assert_eq!(prefers_ime_after_blur, Some(true));
    cx.update(|window, cx| window.focus(&focus_handle, cx));

    cx.simulate_keystrokes("ctrl-x");
    assert_eq!(query_prefers_ime_for_printable_keys(cx), Some(false));
    cx.simulate_keystrokes("k");
    cx.update(|window, _| assert!(!window.has_pending_keystrokes()));
    assert_eq!(query_prefers_ime_for_printable_keys(cx), Some(true));
    test.update(cx, |test, _| {
        assert_eq!(test.action_count.get(), 1);
        assert_eq!(test.text.borrow().as_str(), "");
    });

    cx.simulate_keystrokes("ctrl-b [");
    test.update(cx, |test, _| assert_eq!(test.text.borrow().as_str(), "["));
    close_window(cx);
}

#[crate::test]
fn test_pending_chord_cannot_suppress_or_replay_into_a_new_text_handler(cx: &mut TestAppContext) {
    struct TwoInputs {
        first: PendingTextInputTestView,
        second: PendingTextInputTestView,
    }

    impl Render for TwoInputs {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            div()
                .child(div().id("first-input").child(self.first.clone()))
                .child(div().id("second-input").child(self.second.clone()))
        }
    }

    cx.update(|cx| {
        cx.bind_keys([KeyBinding::new("j k", TestAction, Some("Terminal"))]);
    });
    let (root, cx) = cx.add_window_view(|_, cx| TwoInputs {
        first: PendingTextInputTestView::new(cx),
        second: PendingTextInputTestView::new(cx),
    });
    let second = root.update_in(cx, |root, window, cx| {
        window.focus(&root.first.focus_handle, cx);
        let _ = window.activate_window();
        root.second.focus_handle.clone()
    });
    assert_eq!(query_prefers_ime_for_printable_keys(cx), Some(true));
    cx.simulate_keystrokes("j");
    assert_eq!(query_prefers_ime_for_printable_keys(cx), Some(false));
    cx.executor().advance_clock(PENDING_INPUT_TIMEOUT / 2);
    cx.run_until_parked();

    cx.update(|window, cx| window.focus(&second, cx));
    cx.run_until_parked();
    cx.update(|window, cx| {
        assert_eq!(window.committed_focus(cx), Some(second.clone()));
        assert!(window.pending_input.is_none());
    });
    assert_eq!(query_prefers_ime_for_printable_keys(cx), Some(true));
    cx.executor().advance_clock(PENDING_INPUT_TIMEOUT);
    cx.run_until_parked();
    root.update(cx, |root, _| {
        assert_eq!(root.first.text.borrow().as_str(), "");
        assert_eq!(root.second.text.borrow().as_str(), "");
        assert_eq!(root.first.action_count.get(), 0);
        assert_eq!(root.second.action_count.get(), 0);
    });

    cx.simulate_keystrokes("z");
    root.update(cx, |root, _| {
        assert_eq!(root.first.text.borrow().as_str(), "");
        assert_eq!(root.second.text.borrow().as_str(), "z");
    });

    root.update_in(cx, |root, window, cx| {
        root.second.text.borrow_mut().clear();
        window.focus(&root.first.focus_handle, cx);
    });
    cx.simulate_keystrokes("j");
    let switches = Rc::new(Cell::new(0));
    let _observer = cx.update(|window, cx| {
        cx.new(|cx| PendingInputObserver {
            _subscription: cx.observe_pending_input(window, {
                let second = second.clone();
                let switches = switches.clone();
                move |_, window, cx| {
                    if window.pending_input.is_none() && switches.get() == 0 {
                        switches.set(1);
                        window.focus(&second, cx);
                    }
                }
            }),
        })
    });
    cx.executor().advance_clock(PENDING_INPUT_TIMEOUT);
    cx.run_until_parked();
    assert_eq!(switches.get(), 1);
    assert_eq!(query_prefers_ime_for_printable_keys(cx), Some(true));
    root.update(cx, |root, _| {
        assert_eq!(root.first.text.borrow().as_str(), "");
        assert_eq!(root.second.text.borrow().as_str(), "");
        assert_eq!(root.first.action_count.get(), 0);
        assert_eq!(root.second.action_count.get(), 0);
    });
    close_window(cx);
}

#[crate::test]
fn test_modifier_inside_a_long_chord_does_not_add_a_timeout(cx: &mut TestAppContext) {
    let (cx, action_count, _) = setup_pending_input_test(
        cx,
        [KeyBinding::new(
            "ctrl-x shift f1",
            TestAction,
            Some("Terminal"),
        )],
    );
    cx.simulate_keystrokes("ctrl-x");
    cx.update(|window, _| {
        assert!(window.has_pending_keystrokes());
        assert!(window.pending_input.as_ref().unwrap().timer.is_none());
    });
    simulate_modifier_tap(cx, Modifiers::shift());
    cx.update(|window, _| {
        assert_eq!(window.pending_input.as_ref().unwrap().keystrokes.len(), 2);
        assert!(window.pending_input.as_ref().unwrap().timer.is_none());
    });
    cx.executor().advance_clock(PENDING_INPUT_TIMEOUT * 2);
    cx.run_until_parked();
    cx.update(|window, _| assert!(window.has_pending_keystrokes()));
    cx.simulate_keystrokes("f1");
    assert_eq!(action_count.get(), 1);
    cx.update(|window, _| assert!(window.pending_input.is_none()));
    close_window(cx);
}

#[crate::test]
fn test_context_change_restores_ime_preference_without_replaying_the_old_prefix(
    cx: &mut TestAppContext,
) {
    cx.update(|cx| {
        cx.bind_keys([KeyBinding::new("j k", TestAction, Some("Terminal"))]);
    });
    let (root, cx) = cx.add_window_view(|_, cx| PendingTextInputTestView::new(cx));
    let focus = root.update_in(cx, |root, window, cx| {
        window.focus(&root.focus_handle, cx);
        let _ = window.activate_window();
        root.focus_handle.clone()
    });
    cx.simulate_keystrokes("j");
    assert_eq!(query_prefers_ime_for_printable_keys(cx), Some(false));
    root.update(cx, |root, cx| {
        root.key_context = KeyContext::parse("Other").expect("valid replacement key context");
        cx.notify();
    });
    cx.run_until_parked();
    cx.update(|window, cx| {
        assert_eq!(window.committed_focus(cx), Some(focus.clone()));
        assert!(!window.has_pending_keystrokes());
        assert!(window.pending_input_keystrokes().is_none());
    });
    assert_eq!(query_prefers_ime_for_printable_keys(cx), Some(true));
    cx.executor().advance_clock(PENDING_INPUT_TIMEOUT);
    cx.run_until_parked();
    root.update(cx, |root, _| {
        assert_eq!(root.text.borrow().as_str(), "");
        assert_eq!(root.action_count.get(), 0);
    });
    cx.simulate_keystrokes("z");
    root.update(cx, |root, _| assert_eq!(root.text.borrow().as_str(), "z"));
    close_window(cx);
}

#[derive(Clone, Copy)]
enum FocusMutation {
    Sibling,
    Blur,
    SealedBlur,
    FocusStableBlur,
    SuppressedBlur,
}

struct FocusTestRoot {
    input: PendingTextInputTestView,
    sibling: FocusHandle,
    temporary: Option<FocusHandle>,
    sibling_presentation: SubtreePresentation,
    mutation: Rc<Cell<Option<FocusMutation>>>,
    tile: Option<AtlasTile>,
}

impl FocusTestRoot {
    fn new(cx: &mut App) -> Self {
        Self {
            input: PendingTextInputTestView::new(cx),
            sibling: cx.focus_handle(),
            temporary: None,
            sibling_presentation: SubtreePresentation::Visible,
            mutation: Rc::default(),
            tile: None,
        }
    }
}

impl Render for FocusTestRoot {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let mutation = self.mutation.clone();
        let sibling = self.sibling.clone();
        let tile = self.tile;
        div()
            .size_full()
            .child(self.input.clone())
            .child(
                div()
                    .track_focus(&self.sibling)
                    .with_subtree_presentation(self.sibling_presentation),
            )
            .children(
                self.temporary
                    .as_ref()
                    .map(|focus| div().track_focus(focus)),
            )
            .child(
                canvas(
                    move |_, window, cx| match mutation.take() {
                        Some(FocusMutation::Sibling) => window.focus(&sibling, cx),
                        Some(FocusMutation::Blur) => window.blur(cx),
                        Some(FocusMutation::SealedBlur) => {
                            window.record_prepaint_window_commit(|_, window, cx| window.blur(cx));
                        }
                        Some(FocusMutation::FocusStableBlur) => {
                            window.record_prepaint_focus_stable_commit(|_, window, cx| {
                                window.blur(cx);
                            });
                        }
                        Some(FocusMutation::SuppressedBlur) => {
                            window.with_subtree_presentation(
                                SubtreePresentation::Inert,
                                |window| {
                                    window.blur(cx);
                                },
                            );
                        }
                        None => {}
                    },
                    move |bounds, _, window, _| {
                        if let Some(tile) = tile {
                            let bounds = window.device_local_bounds(bounds);
                            let _ = window.insert_scene_primitive(MonochromeSprite {
                                order: 0,
                                pad: 0,
                                bounds,
                                clip: Default::default(),
                                color: red(),
                                tile,
                                transform: PrimitiveTransform::IDENTITY,
                            });
                        }
                    },
                )
                .size_full(),
            )
    }
}

struct PendingInputObserver {
    _subscription: Subscription,
}

fn capture_pending_input_changes(
    cx: &mut VisualTestContext,
) -> (Rc<Cell<usize>>, Entity<PendingInputObserver>) {
    let count = Rc::new(Cell::new(0));
    let observer = cx.update(|window, cx| {
        cx.new(|cx| PendingInputObserver {
            _subscription: cx.observe_pending_input(window, {
                let count = count.clone();
                move |_, _, _| count.set(count.get() + 1)
            }),
        })
    });
    (count, observer)
}

#[crate::test]
fn test_pending_input_observers_notified_on_focus_change_and_blur(cx: &mut TestAppContext) {
    cx.update(|cx| {
        cx.bind_keys([
            KeyBinding::new("ctrl-b", TestAction, Some("Terminal")),
            KeyBinding::new("ctrl-b h", TestAction, Some("Terminal")),
            KeyBinding::new("ctrl-d", TestAction, None),
            KeyBinding::new("ctrl-d h", TestAction, None),
        ]);
    });
    let (root, cx) = cx.add_window_view(|_, cx| FocusTestRoot::new(cx));
    let (focus_handle, sibling) = root.update(cx, |root, _| {
        (root.input.focus_handle.clone(), root.sibling.clone())
    });
    let (changed, _observer) = capture_pending_input_changes(cx);
    let focus_changes = Rc::new(RefCell::new(Vec::new()));
    let (_focus_in, _focus_out) = cx.update(|window, cx| {
        let gained = focus_changes.clone();
        let lost = focus_changes.clone();
        (
            window.on_focus_in(&focus_handle, cx, move |_, _| {
                gained.borrow_mut().push(true)
            }),
            window.on_focus_out(&focus_handle, cx, move |_, _, _| {
                lost.borrow_mut().push(false)
            }),
        )
    });
    cx.update(|window, cx| {
        window.focus(&focus_handle, cx);
        let _ = window.activate_window();
    });
    assert_eq!(query_prefers_ime_for_printable_keys(cx), Some(true));

    cx.simulate_keystrokes("ctrl-b");
    let count_after_pending = changed.get();
    assert!(count_after_pending > 0);
    cx.update(|window, cx| {
        assert!(window.has_pending_keystrokes());
        window.focus(&cx.focus_handle(), cx);
        assert!(
            window.has_pending_keystrokes(),
            "an unbound target must retain the chord"
        );
        assert_eq!(changed.get(), count_after_pending);

        window.focus(&sibling, cx);
        assert!(!window.has_pending_keystrokes());
        assert_eq!(
            changed.get(),
            count_after_pending,
            "focus notification must be deferred"
        );
    });
    assert!(changed.get() > count_after_pending);
    assert_eq!(focus_changes.borrow().as_slice(), &[true, false]);
    assert_eq!(query_prefers_ime_for_printable_keys(cx), None);

    cx.update(|window, cx| window.focus(&focus_handle, cx));
    cx.simulate_keystrokes("ctrl-b");
    let count_before_blur = changed.get();
    cx.update(|window, cx| {
        assert!(window.has_pending_keystrokes());
        window.blur(cx);
    });
    cx.run_until_parked();
    cx.update(|window, cx| {
        assert!(window.focused(cx).is_none());
        assert!(!window.has_pending_keystrokes());
        assert!(window.pending_input.is_none());
    });
    assert!(changed.get() > count_before_blur);

    root.update(cx, |root, cx| {
        root.temporary = Some(cx.focus_handle());
        cx.notify();
    });
    let temporary = root.update(cx, |root, _| root.temporary.as_ref().unwrap().clone());
    cx.update(|window, cx| window.focus(&temporary, cx));
    drop(temporary);
    cx.simulate_keystrokes("ctrl-d");
    cx.update(|window, _| assert!(window.has_pending_keystrokes()));
    let count_before_drop = changed.get();
    root.update(cx, |root, cx| {
        drop(root.temporary.take());
        cx.notify();
    });
    cx.run_until_parked();
    cx.update(|window, _| assert!(window.pending_input.is_none()));
    assert_eq!(changed.get(), count_before_drop + 1);

    cx.update(|window, cx| window.disable_focus(cx));
    cx.simulate_keystrokes("ctrl-d");
    cx.update(|window, cx| {
        assert!(window.has_pending_keystrokes());
        window.blur(cx);
    });
    cx.run_until_parked();
    cx.update(|window, _| assert!(window.pending_input.is_none()));
    close_window(cx);
}

fn setup_focus_pending_test(
    cx: &mut TestAppContext,
) -> (Entity<FocusTestRoot>, &mut VisualTestContext) {
    cx.update(|cx| {
        cx.bind_keys([
            KeyBinding::new("ctrl-b", TestAction, Some("Terminal")),
            KeyBinding::new("ctrl-b h", TestAction, Some("Terminal")),
        ]);
    });
    let (root, cx) = cx.add_window_view(|_, cx| FocusTestRoot::new(cx));
    root.update_in(cx, |root, window, cx| {
        window.focus(&root.input.focus_handle, cx);
        let _ = window.activate_window();
    });
    (root, cx)
}

fn assert_rejected_transition_preserves_pending_input(
    cx: &mut TestAppContext,
    transition: FocusMutation,
) {
    let (root, cx) = setup_focus_pending_test(cx);
    let atlas = Arc::new(RejectOnceAtlas::new());
    root.update_in(cx, |root, window, cx| {
        window.sprite_atlas = atlas.clone();
        root.tile = Some(RejectOnceAtlas::tile(AtlasTextureKind::Monochrome));
        cx.notify();
    });
    let (mutation, actions, text) = root.update(cx, |root, _| {
        (
            root.mutation.clone(),
            root.input.action_count.clone(),
            root.input.text.clone(),
        )
    });
    let (changed, _observer) = capture_pending_input_changes(cx);
    cx.simulate_keystrokes("ctrl-b");
    cx.executor().advance_clock(PENDING_INPUT_TIMEOUT / 2);
    cx.run_until_parked();
    let before_rejection = changed.get();

    cx.update(|window, cx| {
        let generation = window.rendered_frame.generation;
        let focus = window.focused(cx);
        let pending = window
            .pending_input_keystrokes()
            .expect("pending chord")
            .to_vec();
        mutation.set(Some(transition));
        atlas.fail_next_lease();
        window.refresh();
        window.draw(cx).clear(cx);

        assert_eq!(
            window.rendered_frame.generation, generation,
            "the candidate must be refused"
        );
        assert_eq!(window.focused(cx), focus);
        assert_eq!(window.pending_input_keystrokes(), Some(pending.as_slice()));
        assert!(window.pending_input.as_ref().unwrap().timer.is_some());
        assert_eq!(changed.get(), before_rejection);
    });
    assert_eq!(changed.get(), before_rejection);

    cx.executor().advance_clock(PENDING_INPUT_TIMEOUT / 2);
    cx.run_until_parked();
    cx.update(|window, _| assert!(window.pending_input.is_none()));
    assert_eq!(actions.get(), 1);
    assert_eq!(text.borrow().as_str(), "");
    assert_eq!(changed.get(), before_rejection + 1);

    cx.simulate_keystrokes("ctrl-b");
    let before_acceptance = changed.get();
    cx.update(|window, cx| {
        let generation = window.rendered_frame.generation;
        mutation.set(Some(transition));
        window.refresh();
        window.draw(cx).clear(cx);
        assert_eq!(window.rendered_frame.generation, generation + 1);
        assert!(window.pending_input.is_none());
        assert_eq!(
            changed.get(),
            before_acceptance,
            "candidate notification must be deferred"
        );
        match transition {
            FocusMutation::Sibling => {
                assert_eq!(window.focused(cx), Some(root.read(cx).sibling.clone()));
            }
            FocusMutation::Blur => assert!(window.focused(cx).is_none()),
            _ => unreachable!("only direct candidate focus/blur transitions are exercised here"),
        }
    });
    assert_eq!(changed.get(), before_acceptance + 1);
    cx.executor().advance_clock(PENDING_INPUT_TIMEOUT);
    cx.run_until_parked();
    assert_eq!(
        actions.get(),
        1,
        "the accepted transition must cancel the second chord's timer"
    );
    assert_eq!(text.borrow().as_str(), "");
    close_window(cx);
}

#[crate::test]
fn test_rejected_focus_candidate_preserves_pending_input_deadline(cx: &mut TestAppContext) {
    assert_rejected_transition_preserves_pending_input(cx, FocusMutation::Sibling);
}

#[crate::test]
fn test_rejected_blur_candidate_preserves_pending_input_deadline(cx: &mut TestAppContext) {
    assert_rejected_transition_preserves_pending_input(cx, FocusMutation::Blur);
}

#[crate::test]
fn test_suppressed_focus_and_forbidden_blur_preserve_pending_input(cx: &mut TestAppContext) {
    let (root, cx) = setup_focus_pending_test(cx);
    root.update(cx, |root, cx| {
        root.sibling_presentation = SubtreePresentation::Inert;
        cx.notify();
    });
    let mutation = root.update(cx, |root, _| root.mutation.clone());
    let (changed, _observer) = capture_pending_input_changes(cx);
    cx.simulate_keystrokes("ctrl-b");
    cx.executor().advance_clock(PENDING_INPUT_TIMEOUT / 2);
    cx.run_until_parked();
    let before = changed.get();

    for transition in [
        FocusMutation::Sibling,
        FocusMutation::FocusStableBlur,
        FocusMutation::SuppressedBlur,
    ] {
        cx.update(|window, cx| {
            let focus = window.focused(cx);
            let pending = window
                .pending_input_keystrokes()
                .expect("pending chord")
                .to_vec();
            mutation.set(Some(transition));
            window.refresh();
            window.draw(cx).clear(cx);
            assert_eq!(window.focused(cx), focus);
            assert_eq!(window.pending_input_keystrokes(), Some(pending.as_slice()));
            assert!(window.pending_input.as_ref().unwrap().timer.is_some());
            assert_eq!(changed.get(), before);
        });
        assert_eq!(changed.get(), before);
    }

    cx.executor().advance_clock(PENDING_INPUT_TIMEOUT / 2);
    cx.run_until_parked();
    cx.update(|window, _| assert!(window.pending_input.is_none()));
    root.update(cx, |root, _| {
        assert_eq!(root.input.action_count.get(), 1);
        assert_eq!(root.input.text.borrow().as_str(), "");
    });
    close_window(cx);
}

#[crate::test]
fn test_sealed_callback_blur_settles_pending_input_only_after_admitted_retry(
    cx: &mut TestAppContext,
) {
    let (root, cx) = setup_focus_pending_test(cx);
    let atlas = Arc::new(RejectOnceAtlas::new());
    root.update_in(cx, |root, window, cx| {
        window.sprite_atlas = atlas.clone();
        root.tile = Some(RejectOnceAtlas::tile(AtlasTextureKind::Monochrome));
        cx.notify();
    });
    let mutation = root.update(cx, |root, _| root.mutation.clone());
    let (changed, _observer) = capture_pending_input_changes(cx);
    cx.simulate_keystrokes("ctrl-b");
    let before = changed.get();

    cx.update(|window, cx| {
        let focus = window.focused(cx);
        let pending = window
            .pending_input_keystrokes()
            .expect("pending chord")
            .to_vec();
        let generation = window.rendered_frame.generation;
        mutation.set(Some(FocusMutation::SealedBlur));
        window.refresh();
        window.draw(cx).clear(cx);
        assert_eq!(window.rendered_frame.generation, generation + 1);
        assert_eq!(window.focused(cx), focus);
        assert_eq!(window.pending_input_keystrokes(), Some(pending.as_slice()));
        assert_eq!(changed.get(), before);

        atlas.fail_next_lease();
        window.draw(cx).clear(cx);
        assert_eq!(window.rendered_frame.generation, generation + 1);
        assert_eq!(window.focused(cx), focus);
        assert_eq!(window.pending_input_keystrokes(), Some(pending.as_slice()));
        assert_eq!(changed.get(), before);

        window.draw(cx).clear(cx);
        assert_eq!(window.rendered_frame.generation, generation + 2);
        assert!(window.focused(cx).is_none());
        assert!(window.pending_input.is_none());
        assert_eq!(
            changed.get(),
            before,
            "settlement notification must wait for the effect boundary"
        );
    });
    assert_eq!(changed.get(), before + 1);
    cx.executor().advance_clock(PENDING_INPUT_TIMEOUT);
    cx.run_until_parked();
    root.update(cx, |root, _| assert_eq!(root.input.action_count.get(), 0));
    close_window(cx);
}

#[crate::test]
fn test_empty_and_repeated_blur_settle_each_pending_chord_once(cx: &mut TestAppContext) {
    let (root, cx) = setup_focus_pending_test(cx);
    cx.update(|window, cx| {
        cx.bind_keys([
            KeyBinding::new("ctrl-d", TestAction, None),
            KeyBinding::new("ctrl-d h", TestAction, None),
        ]);
        window.blur(cx);
    });
    cx.run_until_parked();
    cx.update(|window, cx| assert!(window.focused(cx).is_none()));
    let (changed, _observer) = capture_pending_input_changes(cx);

    cx.simulate_keystrokes("ctrl-d");
    let before = changed.get();
    cx.update(|window, cx| {
        assert!(window.has_pending_keystrokes());
        window.blur(cx);
        window.blur(cx);
    });
    cx.run_until_parked();
    cx.update(|window, _| assert!(window.pending_input.is_none()));
    assert_eq!(changed.get(), before + 1);

    cx.update(|window, cx| {
        window.blur(cx);
        window.blur(cx);
    });
    cx.run_until_parked();
    assert_eq!(
        changed.get(),
        before + 1,
        "empty repeated blur must not notify again"
    );
    cx.executor().advance_clock(PENDING_INPUT_TIMEOUT);
    cx.run_until_parked();
    root.update(cx, |root, _| assert_eq!(root.input.action_count.get(), 0));
    close_window(cx);
}

#[crate::test]
fn test_deferred_pending_notification_is_exact_and_ignores_closed_window(cx: &mut TestAppContext) {
    let mut second_app = cx.clone();
    let (root, first) = setup_focus_pending_test(cx);
    let (_second_root, second) = setup_focus_pending_test(&mut second_app);
    let first_handle = first.window_handle();
    let second_handle = second.window_handle();
    let (first_changed, _first_observer) = capture_pending_input_changes(first);
    let (second_changed, _second_observer) = capture_pending_input_changes(second);
    let (focus, sibling) = root.update(first, |root, _| {
        (root.input.focus_handle.clone(), root.sibling.clone())
    });
    first.simulate_keystrokes("ctrl-b");
    second.simulate_keystrokes("ctrl-b");
    let first_before = first_changed.get();
    let second_before = second_changed.get();
    first.update(|window, cx| window.focus(&sibling, cx));
    assert_eq!(first_changed.get(), first_before + 1);
    assert_eq!(second_changed.get(), second_before);
    second.update(|window, _| assert!(window.has_pending_keystrokes()));

    first.update(|window, cx| window.focus(&focus, cx));
    first.simulate_keystrokes("ctrl-b");
    let first_before_close = first_changed.get();
    first.update(|window, cx| {
        window.focus(&sibling, cx);
        assert!(window.pending_input.is_none());
        assert_eq!(first_changed.get(), first_before_close);
        window.remove_window(cx);
    });
    first.run_until_parked();
    assert_eq!(first_changed.get(), first_before_close);
    assert_eq!(second_changed.get(), second_before);
    assert!(!first.windows().contains(&first_handle));
    assert!(first.cx.update_window(first_handle, |_, _, _| ()).is_err());
    assert!(second.windows().contains(&second_handle));
    second.update(|window, _| assert!(window.has_pending_keystrokes()));
    close_window(second);
}

#[crate::test]
fn test_closed_window_timeout_cannot_replay_into_replacement(cx: &mut TestAppContext) {
    cx.update(|cx| cx.bind_keys([KeyBinding::new("j k", TestAction, Some("Terminal"))]));
    let mut keeper_app = cx.clone();
    let (_keeper, keeper) = keeper_app.add_window_view(|_, cx| PendingTextInputTestView::new(cx));
    let (test, cx) = cx.add_window_view(|_, cx| PendingTextInputTestView::new(cx));
    let (text, actions) = test.update_in(cx, |test, window, cx| {
        window.focus(&test.focus_handle, cx);
        let _ = window.activate_window();
        (test.text.clone(), test.action_count.clone())
    });
    let retired = cx.window_handle();
    let (observed, _observer) = capture_observed_keystrokes(cx);
    cx.simulate_keystrokes("j");
    cx.update(|window, _| {
        assert!(window.has_pending_keystrokes());
        assert!(window.pending_input.as_ref().unwrap().timer.is_some());
    });
    cx.executor().advance_clock(PENDING_INPUT_TIMEOUT / 2);
    cx.run_until_parked();
    close_window(cx);

    let mut replacement_app = cx.cx.clone();
    let (replacement, next) =
        replacement_app.add_window_view(|_, cx| PendingTextInputTestView::new(cx));
    replacement.update_in(next, |replacement, window, cx| {
        window.focus(&replacement.focus_handle, cx);
        let _ = window.activate_window();
    });
    assert_ne!(retired, next.window_handle());
    next.executor().advance_clock(PENDING_INPUT_TIMEOUT * 2);
    next.run_until_parked();
    assert!(next.cx.update_window(retired, |_, _, _| ()).is_err());
    assert_eq!(text.borrow().as_str(), "");
    assert_eq!(actions.get(), 0);
    assert!(observed.borrow().is_empty());
    replacement.update(next, |replacement, _| {
        assert_eq!(replacement.text.borrow().as_str(), "");
        assert_eq!(replacement.action_count.get(), 0);
    });
    close_window(next);
    close_window(keeper);
}

#[crate::test]
fn test_close_from_pending_observer_prevents_timeout_action(cx: &mut TestAppContext) {
    let (root, cx) = setup_focus_pending_test(cx);
    let actions = root.update(cx, |root, _| root.input.action_count.clone());
    let text = root.update(cx, |root, _| root.input.text.clone());
    let handle = cx.window_handle();
    cx.simulate_keystrokes("ctrl-b");
    let calls = Rc::new(Cell::new(0));
    let _observer = cx.update(|window, cx| {
        cx.new(|cx| PendingInputObserver {
            _subscription: cx.observe_pending_input(window, {
                let calls = calls.clone();
                move |_, window, cx| {
                    calls.set(calls.get() + 1);
                    window.remove_window(cx);
                }
            }),
        })
    });
    let (later_calls, _later_observer) = capture_pending_input_changes(cx);
    cx.executor().advance_clock(PENDING_INPUT_TIMEOUT);
    cx.run_until_parked();
    assert_eq!(calls.get(), 1);
    assert_eq!(later_calls.get(), 0);
    assert_eq!(actions.get(), 0);
    assert_eq!(text.borrow().as_str(), "");
    assert!(!cx.windows().contains(&handle));
    assert!(cx.cx.update_window(handle, |_, _, _| ()).is_err());
}

#[crate::test]
fn test_close_from_replay_listener_prevents_later_keys_and_text(cx: &mut TestAppContext) {
    struct ClosingReplayRoot {
        input: PendingTextInputTestView,
        close: Rc<Cell<bool>>,
        replayed_keys: Rc<RefCell<Vec<String>>>,
    }

    impl Render for ClosingReplayRoot {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let close = self.close.clone();
            let replayed_keys = self.replayed_keys.clone();
            div()
                .on_key_down(move |event, window, cx| {
                    if close.get() {
                        replayed_keys.borrow_mut().push(event.keystroke.key.clone());
                        window.remove_window(cx);
                    }
                })
                .child(self.input.clone())
        }
    }

    cx.update(|cx| cx.bind_keys([KeyBinding::new("j k l", TestAction, Some("Terminal"))]));
    let close = Rc::new(Cell::new(false));
    let replayed_keys = Rc::new(RefCell::new(Vec::new()));
    let (root, cx) = cx.add_window_view(|_, cx| ClosingReplayRoot {
        input: PendingTextInputTestView::new(cx),
        close: close.clone(),
        replayed_keys: replayed_keys.clone(),
    });
    let (text, actions) = root.update_in(cx, |root, window, cx| {
        window.focus(&root.input.focus_handle, cx);
        let _ = window.activate_window();
        (root.input.text.clone(), root.input.action_count.clone())
    });
    let handle = cx.window_handle();
    cx.simulate_keystrokes("j k");
    cx.update(|window, _| {
        assert_eq!(
            window
                .pending_input_keystrokes()
                .expect("pending chord")
                .len(),
            2
        );
    });
    close.set(true);
    cx.executor().advance_clock(PENDING_INPUT_TIMEOUT);
    cx.run_until_parked();
    assert_eq!(replayed_keys.borrow().as_slice(), &["j"]);
    assert_eq!(text.borrow().as_str(), "");
    assert_eq!(actions.get(), 0);
    assert!(!cx.windows().contains(&handle));
    assert!(cx.cx.update_window(handle, |_, _, _| ()).is_err());
}
