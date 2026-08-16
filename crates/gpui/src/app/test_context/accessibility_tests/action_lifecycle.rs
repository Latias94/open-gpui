use super::*;
use crate::{
    Modifiers, MouseButton, MouseDownEvent, PointerCancelReason, PointerCaptureHandle,
    WindowMouseEvent, point,
};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

struct TransactionalAccessibilityProbeView {
    leaked_activations: usize,
    focus: FocusHandle,
}

struct PublishedActionProbeView {
    activations: usize,
}

struct PublishedBoundsClickProbeView {
    activations: usize,
}

struct GenerationFlipAccessibilityProbeView {
    flip_generation_during_prepaint: bool,
    activations: usize,
}

struct ExactActionProjectionProbeView {
    activations: usize,
}

struct AccessibilityWindowRemovalProbeView {
    pointer_capture: PointerCaptureHandle,
    lifecycle: Rc<RefCell<Vec<&'static str>>>,
}

struct BuiltinClickWindowRemovalProbeView {
    lifecycle: Rc<RefCell<Vec<&'static str>>>,
}

struct InteractionQuiescenceAccessibilityProbeView {
    lifecycle: Rc<RefCell<Vec<&'static str>>>,
}

impl Render for InteractionQuiescenceAccessibilityProbeView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let fallback_lifecycle = self.lifecycle.clone();
        let quiescer_lifecycle = self.lifecycle.clone();
        let later_lifecycle = self.lifecycle.clone();

        div()
            .id("interaction-quiescence-accessibility-probe")
            .size_full()
            .role(Role::Button)
            .aria_label("Interaction quiescence accessibility probe")
            .aria_action(AccessibleAction::Click)
            .on_mouse_down(MouseButton::Left, move |_, _, _| {
                fallback_lifecycle.borrow_mut().push("fallback-down");
            })
            .on_a11y_action(AccessibleAction::Click, move |_, window, cx| {
                quiescer_lifecycle.borrow_mut().push("quiescer");
                assert!(window.quiesce_interaction(cx));
                quiescer_lifecycle.borrow_mut().push("quiescer-returned");
            })
            .on_a11y_action(AccessibleAction::Click, move |_, _, _| {
                later_lifecycle.borrow_mut().push("later-listener");
            })
    }
}

impl Render for BuiltinClickWindowRemovalProbeView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let mouse_down_lifecycle = self.lifecycle.clone();
        let mouse_up_lifecycle = self.lifecycle.clone();

        div()
            .id("builtin-click-window-removal-probe")
            .size_full()
            .role(Role::Button)
            .aria_label("Built-in click window removal probe")
            .aria_action(AccessibleAction::Click)
            .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                mouse_down_lifecycle.borrow_mut().push("down");
                window.remove_window(cx);
                assert!(
                    !window.removed,
                    "removal must wait for the click transaction"
                );
                mouse_down_lifecycle.borrow_mut().push("down-returned");
            })
            .on_mouse_up(MouseButton::Left, move |_, _, _| {
                mouse_up_lifecycle.borrow_mut().push("up");
            })
    }
}

impl Render for AccessibilityWindowRemovalProbeView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let pointer_capture = self.pointer_capture;
        let lifecycle = self.lifecycle.clone();

        div()
            .id("accessibility-window-removal-probe")
            .size_full()
            .role(Role::Button)
            .aria_label("Accessibility window removal probe")
            .track_pointer_capture(&self.pointer_capture)
            .on_mouse_down(MouseButton::Left, move |_, window, _| {
                window
                    .capture_pointer(&pointer_capture, MouseButton::Left)
                    .expect("mouse down should establish pointer capture before the action");
            })
            .on_a11y_action(AccessibleAction::Click, move |_, window, cx| {
                lifecycle.borrow_mut().push("action");
                window.remove_window(cx);
                window.remove_window(cx);
                assert!(!window.removed, "removal must wait for the action callback");
                lifecycle.borrow_mut().push("action-returned");
            })
    }
}

impl Render for ExactActionProjectionProbeView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let click_target = cx.entity().downgrade();
        let increment_target = click_target.clone();

        div()
            .id("exact-action-projection-probe")
            .role(Role::Group)
            .child(
                div()
                    .id("exact-empty-actions")
                    .role(Role::Button)
                    .aria_label("Exact empty actions")
                    .focusable()
                    .aria_actions([])
                    .on_click(move |_, _, cx| {
                        click_target
                            .update(cx, |this, _| this.activations += 1)
                            .ok();
                    })
                    .on_a11y_action(AccessibleAction::Increment, move |_, _, cx| {
                        increment_target
                            .update(cx, |this, _| this.activations += 1)
                            .ok();
                    }),
            )
            .child(
                div()
                    .id("declared-listener-free-action")
                    .role(Role::SpinButton)
                    .aria_label("Declared listener-free action")
                    .aria_action(AccessibleAction::Increment),
            )
            .child(
                div()
                    .id("single-then-set-actions")
                    .role(Role::Button)
                    .aria_label("Single then set actions")
                    .aria_action(AccessibleAction::Click)
                    .aria_actions([AccessibleAction::Focus]),
            )
            .child(
                div()
                    .id("set-then-single-actions")
                    .role(Role::SpinButton)
                    .aria_label("Set then single actions")
                    .aria_actions([AccessibleAction::Focus])
                    .aria_action(AccessibleAction::Increment),
            )
    }
}

impl Render for GenerationFlipAccessibilityProbeView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let this = cx.entity().downgrade();
        let flip_generation = self.flip_generation_during_prepaint;
        div()
            .id("generation-flip-a11y-probe")
            .role(Role::Group)
            .child(
                div()
                    .id("generation-flip-action")
                    .role(Role::Button)
                    .aria_label("Generation flip action")
                    .aria_value(format!("activation-{}", self.activations))
                    .on_a11y_action(AccessibleAction::Click, move |_, _, cx| {
                        this.update(cx, |this, cx| {
                            this.activations += 1;
                            cx.notify();
                        })
                        .ok();
                    }),
            )
            .child(canvas(
                move |_, window, _| {
                    if flip_generation {
                        window.set_accessibility_active_for_test(false);
                        window.set_accessibility_active_for_test(true);
                    }
                },
                |_, _, _, _| {},
            ))
    }
}

#[open_gpui::test]
fn accessibility_action_quiescence_stops_same_dispatch_listeners_and_fallback(
    cx: &mut TestAppContext,
) {
    let lifecycle = Rc::new(RefCell::new(Vec::new()));
    let typed_window = cx.open_window(size(px(320.0), px(200.0)), {
        let lifecycle = lifecycle.clone();
        move |_, _| InteractionQuiescenceAccessibilityProbeView { lifecycle }
    });
    let window = typed_window.into();

    assert!(cx.activate_accessibility(window));
    let update = cx.latest_accessibility_tree_update(window).unwrap();
    let (node_id, node) = node_with_label(&update, "Interaction quiescence accessibility probe");
    assert!(node.supports_action(AccessibleAction::Click));

    assert!(cx.dispatch_accessibility_action(
        window,
        ActionRequest {
            action: AccessibleAction::Click,
            target_tree: TreeId::ROOT,
            target_node: node_id,
            data: None,
        },
    ));
    assert_eq!(
        lifecycle.borrow().as_slice(),
        &["quiescer", "quiescer-returned"],
        "quiescence must stop later AccessKit listeners and the built-in click fallback"
    );

    cx.run_until_parked();
    assert!(
        cx.latest_accessibility_tree_update(window)
            .unwrap()
            .nodes
            .iter()
            .all(|(_, node)| {
                node.label() != Some("Interaction quiescence accessibility probe")
            }),
        "the inert replacement frame must remove the action target from the published tree"
    );
}

#[open_gpui::test]
fn accessibility_action_declarations_publish_exact_sets(cx: &mut TestAppContext) {
    let typed_window = cx.open_window(size(px(320.0), px(200.0)), |_, _| {
        ExactActionProjectionProbeView { activations: 0 }
    });
    let view = typed_window.root(cx).unwrap();
    let window = typed_window.into();

    assert!(cx.activate_accessibility(window));
    let update = cx.latest_accessibility_tree_update(window).unwrap();

    let (empty_id, empty) = node_with_label(&update, "Exact empty actions");
    assert!(!empty.supports_action(AccessibleAction::Click));
    assert!(!empty.supports_action(AccessibleAction::Focus));
    assert!(!empty.supports_action(AccessibleAction::Increment));
    assert!(
        crate::window::a11y::ACCESSKIT_ACTIONS
            .iter()
            .all(|action| !empty.supports_action(*action)),
        "an explicit empty action set must not receive inferred actions"
    );

    assert!(cx.dispatch_accessibility_action(
        window,
        ActionRequest {
            action: AccessibleAction::Click,
            target_tree: TreeId::ROOT,
            target_node: empty_id,
            data: None,
        },
    ));
    assert!(cx.dispatch_accessibility_action(
        window,
        ActionRequest {
            action: AccessibleAction::Increment,
            target_tree: TreeId::ROOT,
            target_node: empty_id,
            data: None,
        },
    ));
    assert_eq!(cx.read(|cx| view.read(cx).activations), 0);

    let (_, listener_free) = node_with_label(&update, "Declared listener-free action");
    assert!(listener_free.supports_action(AccessibleAction::Increment));
    assert!(!listener_free.supports_action(AccessibleAction::Click));
    assert!(!listener_free.supports_action(AccessibleAction::Focus));
    assert!(!listener_free.supports_action(AccessibleAction::ScrollIntoView));

    let (_, replaced) = node_with_label(&update, "Single then set actions");
    assert!(!replaced.supports_action(AccessibleAction::Click));
    assert!(replaced.supports_action(AccessibleAction::Focus));
    assert!(!replaced.supports_action(AccessibleAction::Increment));
    assert!(!replaced.supports_action(AccessibleAction::ScrollIntoView));

    let (_, extended) = node_with_label(&update, "Set then single actions");
    assert!(!extended.supports_action(AccessibleAction::Click));
    assert!(extended.supports_action(AccessibleAction::Focus));
    assert!(extended.supports_action(AccessibleAction::Increment));
    assert!(!extended.supports_action(AccessibleAction::ScrollIntoView));
}

struct EarlyA11yRegistrationProbeView {
    activations: usize,
}

impl Render for EarlyA11yRegistrationProbeView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        window.with_id("early-a11y-registration-probe", |window| {
            let node_id = window.with_global_id("action".into(), |global_id, _| {
                global_id.accesskit_node_id()
            });
            let this = cx.entity().downgrade();
            window.on_a11y_action(node_id, AccessibleAction::Click, move |_, _, cx| {
                this.update(cx, |this, cx| {
                    this.activations += 1;
                    cx.notify();
                })
                .ok();
            });

            div()
                .id("early-a11y-registration-probe")
                .role(Role::Group)
                .child(
                    div()
                        .id("action")
                        .role(Role::Button)
                        .aria_label("Early registered action")
                        .on_a11y_action(AccessibleAction::Click, |_, _, _| {}),
                )
        })
    }
}

impl Render for PublishedBoundsClickProbeView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let this = cx.entity().downgrade();
        div()
            .id("published-bounds-click-probe")
            .role(Role::Button)
            .aria_label("Published bounds click probe")
            .aria_value(format!("activation-{}", self.activations))
            .size_full()
            .on_click(move |_, _, cx| {
                this.update(cx, |this, cx| {
                    this.activations += 1;
                    cx.notify();
                })
                .ok();
            })
    }
}

impl Render for PublishedActionProbeView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let click_target = cx.entity().downgrade();
        let unsupported_target = click_target.clone();
        let activations = self.activations;

        canvas(
            move |bounds, window, _| {
                if !window.a11y.is_active() {
                    return;
                }

                let node_id = NodeId(u64::MAX - 2);
                let mut node = Node::new(Role::Button);
                node.set_label("Published action probe");
                node.set_numeric_value(activations as f64);
                node.add_action(AccessibleAction::Click);
                assert!(window.a11y.nodes.push(node_id, node));
                window
                    .a11y
                    .record_node_bounds(node_id, bounds, Some(bounds.center()));
                window.on_a11y_action(node_id, AccessibleAction::Click, move |_, _, cx| {
                    click_target
                        .update(cx, |this, cx| {
                            this.activations += 1;
                            cx.notify();
                        })
                        .ok();
                });
                window.on_a11y_action(node_id, AccessibleAction::Increment, move |_, _, cx| {
                    unsupported_target
                        .update(cx, |this, cx| {
                            this.activations += 1;
                            cx.notify();
                        })
                        .ok();
                });
                window.a11y.nodes.pop();
            },
            |_, _, _, _| {},
        )
        .size_full()
    }
}

impl Render for TransactionalAccessibilityProbeView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let this = cx.entity().downgrade();
        let leaked_focus_id = self.focus.id;
        div()
            .id("transactional-accessibility-probe")
            .role(Role::Group)
            .child(
                canvas(
                    move |bounds, window, _| {
                        if !window.a11y.is_active() {
                            return;
                        }

                        let node_id = NodeId(u64::MAX - 1);
                        let live_node_id = NodeId(u64::MAX - 3);
                        let rejected: Result<(), ()> = window.transact(|window| {
                            window.with_accessibility_tree_scope(
                                AccessibilityTreeScope::ModalRoot,
                                |window| {
                                    let mut node = Node::new(Role::Button);
                                    node.set_label("Discarded transaction node");
                                    node.add_action(AccessibleAction::Click);
                                    assert!(window.a11y.nodes.push(node_id, node));
                                    window.a11y.record_node_bounds(
                                        node_id,
                                        bounds,
                                        Some(bounds.center()),
                                    );
                                    window.a11y.record_focus_id(node_id, leaked_focus_id);
                                    window.on_a11y_action(
                                        node_id,
                                        AccessibleAction::Click,
                                        move |_, _, cx| {
                                            this.update(cx, |this, cx| {
                                                this.leaked_activations += 1;
                                                cx.notify();
                                            })
                                            .ok();
                                        },
                                    );
                                    window.a11y.nodes.pop();

                                    let mut live_node = Node::new(Role::Status);
                                    live_node.set_label("Discarded transaction live region");
                                    live_node.set_value("Discarded transaction live region");
                                    live_node.set_live(accesskit::Live::Polite);
                                    live_node.set_live_atomic();
                                    assert!(window.a11y.nodes.push(live_node_id, live_node));
                                    window.a11y.nodes.pop();
                                    Err(())
                                },
                            )
                        });
                        assert!(rejected.is_err());
                        assert!(!window.a11y.nodes.has_node(node_id));
                        assert!(!window.a11y.has_candidate_node_bounds(node_id));
                        assert!(!window.a11y.has_candidate_focus_id(node_id));

                        let mut node = Node::new(Role::Button);
                        node.set_label("Committed transaction node");
                        node.add_action(AccessibleAction::Click);
                        assert!(window.a11y.nodes.push(node_id, node));
                        window
                            .a11y
                            .record_node_bounds(node_id, bounds, Some(bounds.center()));
                        window.a11y.nodes.pop();
                    },
                    |_, _, _, _| {},
                )
                .size_full(),
            )
    }
}

#[open_gpui::test]
fn accessibility_harness_rolls_back_failed_prepaint_transactions(cx: &mut TestAppContext) {
    let typed_window = cx.open_window(size(px(320.0), px(200.0)), |_, cx| {
        TransactionalAccessibilityProbeView {
            leaked_activations: 0,
            focus: cx.focus_handle(),
        }
    });
    let view = typed_window.root(cx).unwrap();
    let window = typed_window.into();

    assert!(cx.activate_accessibility(window));
    let update = cx.latest_accessibility_tree_update(window).unwrap();
    let (node_id, node) = node_with_label(&update, "Committed transaction node");
    assert_eq!(node_id, NodeId(u64::MAX - 1));
    assert!(node.supports_action(AccessibleAction::Click));
    assert!(
        !update
            .nodes
            .iter()
            .any(|(_, node)| node.label() == Some("Discarded transaction node"))
    );
    assert!(
        !update
            .nodes
            .iter()
            .any(|(_, node)| node.label() == Some("Discarded transaction live region"))
    );
    assert!(cx.dispatch_accessibility_action(
        window,
        ActionRequest {
            action: AccessibleAction::Click,
            target_tree: TreeId::ROOT,
            target_node: node_id,
            data: None,
        },
    ));
    assert_eq!(cx.read(|cx| view.read(cx).leaked_activations), 0);
}

#[open_gpui::test]
fn accessibility_harness_preserves_listener_registration_before_node_prepaint(
    cx: &mut TestAppContext,
) {
    let typed_window = cx.open_window(size(px(320.0), px(200.0)), |_, _| {
        EarlyA11yRegistrationProbeView { activations: 0 }
    });
    let view = typed_window.root(cx).unwrap();
    let window = typed_window.into();

    assert!(cx.activate_accessibility(window));
    let update = cx.latest_accessibility_tree_update(window).unwrap();
    let (node_id, node) = node_with_label(&update, "Early registered action");
    assert!(node.supports_action(AccessibleAction::Click));
    assert!(cx.dispatch_accessibility_action(
        window,
        ActionRequest {
            action: AccessibleAction::Click,
            target_tree: TreeId::ROOT,
            target_node: node_id,
            data: None,
        },
    ));
    assert_eq!(cx.read(|cx| view.read(cx).activations), 1);
}

#[open_gpui::test]
fn accessibility_harness_uses_published_bounds_for_click_fallback(cx: &mut TestAppContext) {
    let typed_window = cx.open_window(size(px(320.0), px(200.0)), |_, _| {
        PublishedBoundsClickProbeView { activations: 0 }
    });
    let view = typed_window.root(cx).unwrap();
    let window = typed_window.into();

    assert!(cx.activate_accessibility(window));
    let initial = cx.latest_accessibility_tree_update(window).unwrap();
    let (node_id, node) = node_with_label(&initial, "Published bounds click probe");
    assert!(node.supports_action(AccessibleAction::Click));

    assert!(cx.dispatch_accessibility_action(
        window,
        ActionRequest {
            action: AccessibleAction::Click,
            target_tree: TreeId::ROOT,
            target_node: node_id,
            data: None,
        },
    ));
    assert_eq!(cx.read(|cx| view.read(cx).activations), 1);
    let activated = cx.latest_accessibility_tree_update(window).unwrap();
    assert_eq!(
        node_with_label(&activated, "Published bounds click probe")
            .1
            .value(),
        Some("activation-1")
    );
}

#[open_gpui::test]
fn accessibility_click_fallback_skips_mouse_up_after_mouse_down_requests_window_removal(
    cx: &mut TestAppContext,
) {
    let lifecycle = Rc::new(RefCell::new(Vec::new()));
    let typed_window = cx.open_window(size(px(320.0), px(200.0)), {
        let lifecycle = lifecycle.clone();
        move |_, _| BuiltinClickWindowRemovalProbeView { lifecycle }
    });
    let window = typed_window.into();

    assert!(cx.activate_accessibility(window));
    let update = cx.latest_accessibility_tree_update(window).unwrap();
    let (node_id, node) = node_with_label(&update, "Built-in click window removal probe");
    assert!(node.supports_action(AccessibleAction::Click));

    assert!(cx.dispatch_accessibility_action(
        window,
        ActionRequest {
            action: AccessibleAction::Click,
            target_tree: TreeId::ROOT,
            target_node: node_id,
            data: None,
        },
    ));

    assert_eq!(lifecycle.borrow().as_slice(), &["down", "down-returned"]);
    assert!(cx.windows().is_empty());
}

#[open_gpui::test]
fn accessibility_harness_authorizes_actions_from_the_published_root_tree(cx: &mut TestAppContext) {
    let typed_window = cx.open_window(size(px(320.0), px(200.0)), |_, _| {
        PublishedActionProbeView { activations: 0 }
    });
    let view = typed_window.root(cx).unwrap();
    let window = typed_window.into();

    assert!(cx.activate_accessibility(window));
    let initial = cx.latest_accessibility_tree_update(window).unwrap();
    let (node_id, node) = node_with_label(&initial, "Published action probe");
    assert!(node.supports_action(AccessibleAction::Click));
    assert!(!node.supports_action(AccessibleAction::Increment));
    let initial_history_len = cx.accessibility_tree_update_history(window).len();

    assert!(cx.dispatch_accessibility_action(
        window,
        ActionRequest {
            action: AccessibleAction::Click,
            target_tree: TreeId(uuid::Uuid::from_u128(1)),
            target_node: node_id,
            data: None,
        },
    ));
    assert_eq!(cx.read(|cx| view.read(cx).activations), 0);
    assert_eq!(
        cx.accessibility_tree_update_history(window).len(),
        initial_history_len
    );

    assert!(cx.dispatch_accessibility_action(
        window,
        ActionRequest {
            action: AccessibleAction::Increment,
            target_tree: TreeId::ROOT,
            target_node: node_id,
            data: None,
        },
    ));
    assert_eq!(cx.read(|cx| view.read(cx).activations), 0);
    assert_eq!(
        cx.accessibility_tree_update_history(window).len(),
        initial_history_len
    );

    assert!(cx.dispatch_accessibility_action(
        window,
        ActionRequest {
            action: AccessibleAction::Click,
            target_tree: TreeId::ROOT,
            target_node: node_id,
            data: None,
        },
    ));
    assert_eq!(cx.read(|cx| view.read(cx).activations), 1);
    assert_eq!(
        node_with_label(
            &cx.latest_accessibility_tree_update(window).unwrap(),
            "Published action probe",
        )
        .1
        .numeric_value(),
        Some(1.0)
    );
}

#[open_gpui::test]
fn accessibility_harness_publishes_only_matching_frame_generations(cx: &mut TestAppContext) {
    let typed_window = cx.open_window(size(px(320.0), px(200.0)), |_, _| {
        GenerationFlipAccessibilityProbeView {
            flip_generation_during_prepaint: false,
            activations: 0,
        }
    });
    let view = typed_window.root(cx).unwrap();
    let window = typed_window.into();

    assert!(cx.activate_accessibility(window));
    let initial = cx.latest_accessibility_tree_update(window).unwrap();
    let (node_id, _) = node_with_label(&initial, "Generation flip action");
    let initial_history_len = cx.accessibility_tree_update_history(window).len();

    view.update(cx, |view, cx| {
        view.flip_generation_during_prepaint = true;
        cx.notify();
    });
    cx.run_until_parked();
    assert_eq!(
        cx.accessibility_tree_update_history(window).len(),
        initial_history_len,
        "a candidate built across activation generations must not publish"
    );
    assert!(cx.dispatch_accessibility_action(
        window,
        ActionRequest {
            action: AccessibleAction::Click,
            target_tree: TreeId::ROOT,
            target_node: node_id,
            data: None,
        },
    ));
    assert_eq!(cx.read(|cx| view.read(cx).activations), 0);

    view.update(cx, |view, cx| {
        view.flip_generation_during_prepaint = false;
        cx.notify();
    });
    cx.run_until_parked();
    let matching = cx.latest_accessibility_tree_update(window).unwrap();
    assert_eq!(
        node_with_label(&matching, "Generation flip action").0,
        node_id
    );
    assert_eq!(
        cx.accessibility_tree_update_history(window).len(),
        initial_history_len + 1
    );
}

#[open_gpui::test]
fn accessibility_action_window_removal_commits_after_listener_and_pointer_cancellation(
    cx: &mut TestAppContext,
) {
    let lifecycle = Rc::new(RefCell::new(Vec::new()));
    let close_count = Rc::new(Cell::new(0));
    let _close_subscription = cx.update(|cx| {
        let lifecycle = lifecycle.clone();
        let close_count = close_count.clone();
        cx.on_window_closed(move |_, _| {
            close_count.set(close_count.get() + 1);
            lifecycle.borrow_mut().push("closed");
        })
    });
    let typed_window = cx.open_window(size(px(320.0), px(200.0)), {
        let lifecycle = lifecycle.clone();
        move |window, _| AccessibilityWindowRemovalProbeView {
            pointer_capture: window.new_pointer_capture_handle(),
            lifecycle,
        }
    });
    let window: crate::AnyWindowHandle = typed_window.into();

    let _ = window
        .update(cx, |_, window, _| window.activate_window())
        .unwrap();
    cx.run_until_parked();
    assert!(cx.activate_accessibility(window));
    let update = cx.latest_accessibility_tree_update(window).unwrap();
    let (node_id, node) = node_with_label(&update, "Accessibility window removal probe");
    assert!(node.supports_action(AccessibleAction::Click));

    let _pointer_subscription = window
        .update(cx, {
            let lifecycle = lifecycle.clone();
            move |_, window, _| {
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
            }
        })
        .unwrap();
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
    window
        .update(cx, |_, window, cx| {
            assert!(window.has_active_pointer_session(cx));
        })
        .unwrap();

    assert!(cx.dispatch_accessibility_action(
        window,
        ActionRequest {
            action: AccessibleAction::Click,
            target_tree: TreeId::ROOT,
            target_node: node_id,
            data: None,
        },
    ));

    assert_eq!(
        lifecycle.borrow().as_slice(),
        &[
            "action",
            "action-returned",
            "cancel",
            "cancel-returned",
            "closed"
        ]
    );
    assert_eq!(close_count.get(), 1);
    assert!(cx.windows().is_empty());
}

#[open_gpui::test]
fn accessibility_harness_rejects_actions_queued_by_an_old_activation(cx: &mut TestAppContext) {
    let typed_window = cx.open_window(size(px(320.0), px(200.0)), |_, _| {
        PublishedActionProbeView { activations: 0 }
    });
    let view = typed_window.root(cx).unwrap();
    let window = typed_window.into();

    assert!(cx.activate_accessibility(window));
    let initial = cx.latest_accessibility_tree_update(window).unwrap();
    let (node_id, _) = node_with_label(&initial, "Published action probe");
    let platform_window = cx.test_window(window);

    assert!(
        platform_window.dispatch_accessibility_action(ActionRequest {
            action: AccessibleAction::Click,
            target_tree: TreeId::ROOT,
            target_node: node_id,
            data: None,
        })
    );
    assert!(platform_window.deactivate_accessibility());
    assert!(platform_window.activate_accessibility());
    cx.run_until_parked();

    assert_eq!(cx.read(|cx| view.read(cx).activations), 0);
    let reactivated = cx.latest_accessibility_tree_update(window).unwrap();
    assert_eq!(
        node_with_label(&reactivated, "Published action probe").0,
        node_id
    );

    assert!(cx.dispatch_accessibility_action(
        window,
        ActionRequest {
            action: AccessibleAction::Click,
            target_tree: TreeId::ROOT,
            target_node: node_id,
            data: None,
        },
    ));
    assert_eq!(cx.read(|cx| view.read(cx).activations), 1);
}
