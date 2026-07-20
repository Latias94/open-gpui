use super::*;

use crate::{SubtreePresentation, SubtreePresentationExt};

struct HardHiddenModalAccessibilityProbeView {
    hidden: bool,
    activations: usize,
}

impl Render for HardHiddenModalAccessibilityProbeView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let direct_target = cx.entity().downgrade();
        let deferred_target = direct_target.clone();

        div()
            .id("hard-hidden-modal-probe")
            .role(Role::Group)
            .child(
                div()
                    .id("visible-outside-hidden-subtree")
                    .role(Role::Button)
                    .aria_label("Visible outside hidden subtree"),
            )
            .child(
                div()
                    .id("hard-hidden-modal-subtree")
                    .child(accessibility_scope(
                        AccessibilityTreeScope::ModalRoot,
                        div()
                            .id("hard-hidden-modal-root")
                            .role(Role::Dialog)
                            .aria_label("Hard hidden modal root")
                            .on_a11y_action(AccessibleAction::Click, move |_, _, cx| {
                                direct_target
                                    .update(cx, |this, cx| {
                                        this.activations += 1;
                                        cx.notify();
                                    })
                                    .ok();
                            }),
                    ))
                    .child(deferred(accessibility_scope(
                        AccessibilityTreeScope::ModalDescendant,
                        div()
                            .id("hard-hidden-modal-descendant")
                            .role(Role::Button)
                            .aria_label("Hard hidden modal descendant")
                            .on_a11y_action(AccessibleAction::Click, move |_, _, cx| {
                                deferred_target
                                    .update(cx, |this, cx| {
                                        this.activations += 1;
                                        cx.notify();
                                    })
                                    .ok();
                            }),
                    )))
                    .with_subtree_presentation(if self.hidden {
                        SubtreePresentation::Inert
                    } else {
                        SubtreePresentation::Visible
                    }),
            )
    }
}

struct ExtendedAccessibilityProbeView {
    disabled: bool,
    read_only: bool,
    activations: usize,
    focus: FocusHandle,
}

impl Render for ExtendedAccessibilityProbeView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        window.with_id("extended-accessibility-probe", |window| {
            let description_id = window.with_global_id("description".into(), |global_id, _| {
                global_id.accesskit_node_id()
            });
            let this = cx.entity().downgrade();
            let increment_target = this.clone();

            div()
                .id("extended-accessibility-probe")
                .role(Role::Group)
                .child(
                    div()
                        .id("description")
                        .role(Role::Label)
                        .aria_label("Account description"),
                )
                .child(
                    div()
                        .id("control")
                        .role(Role::TextInput)
                        .aria_label("Account")
                        .aria_description("Private account details")
                        .aria_described_by([description_id])
                        .aria_value(format!("activation-{}", self.activations))
                        .aria_invalid(true)
                        .aria_busy(true)
                        .aria_read_only(self.read_only)
                        .aria_required(true)
                        .omit_accessibility_node(false)
                        .aria_modal(false)
                        .aria_disabled(self.disabled)
                        .focusable()
                        .track_focus(&self.focus)
                        .on_a11y_action(AccessibleAction::Click, move |_, _, cx| {
                            this.update(cx, |this, cx| {
                                this.activations += 1;
                                cx.notify();
                            })
                            .ok();
                        })
                        .on_a11y_action(AccessibleAction::Increment, move |_, _, cx| {
                            increment_target
                                .update(cx, |this, cx| {
                                    this.activations += 1;
                                    cx.notify();
                                })
                                .ok();
                        }),
                )
        })
    }
}

struct RolelessHiddenAccessibilityProbeView {
    hidden: bool,
    activations: usize,
}

impl Render for RolelessHiddenAccessibilityProbeView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let direct_target = cx.entity().downgrade();
        let deferred_target = direct_target.clone();

        div().id("roleless-hidden-probe").role(Role::Group).child(
            div()
                .id("roleless-hidden-container")
                .child(
                    div()
                        .id("roleless-hidden-direct")
                        .role(Role::Button)
                        .aria_label("Roleless hidden direct action")
                        .on_a11y_action(AccessibleAction::Click, move |_, _, cx| {
                            direct_target
                                .update(cx, |this, cx| {
                                    this.activations += 1;
                                    cx.notify();
                                })
                                .ok();
                        }),
                )
                .child(deferred(
                    div()
                        .id("roleless-hidden-deferred")
                        .role(Role::Button)
                        .aria_label("Roleless hidden deferred action")
                        .on_a11y_action(AccessibleAction::Click, move |_, _, cx| {
                            deferred_target
                                .update(cx, |this, cx| {
                                    this.activations += 1;
                                    cx.notify();
                                })
                                .ok();
                        }),
                ))
                .with_subtree_presentation(if self.hidden {
                    SubtreePresentation::Inert
                } else {
                    SubtreePresentation::Visible
                }),
        )
    }
}

#[open_gpui::test]
fn accessibility_harness_projects_extended_semantics_and_blocks_disabled_actions(
    cx: &mut TestAppContext,
) {
    let typed_window = cx.open_window(size(px(320.0), px(200.0)), |_, cx| {
        ExtendedAccessibilityProbeView {
            disabled: false,
            read_only: true,
            activations: 0,
            focus: cx.focus_handle(),
        }
    });
    let view = typed_window.root(cx).unwrap();
    let window = typed_window.into();

    assert!(cx.activate_accessibility(window));
    let initial = cx.latest_accessibility_tree_update(window).unwrap();
    let (control_id, control) = node_with_role(&initial, Role::TextInput);
    let (description_id, _) = node_with_label(&initial, "Account description");
    assert_eq!(control.label(), Some("Account"));
    assert_eq!(control.description(), Some("Private account details"));
    assert_eq!(control.described_by(), &[description_id]);
    assert_eq!(control.value(), Some("activation-0"));
    assert_eq!(control.invalid(), Some(Invalid::True));
    assert!(control.is_busy());
    assert!(control.is_read_only());
    assert!(control.is_required());
    assert!(!control.is_hidden());
    assert!(!control.is_modal());
    assert!(control.supports_action(AccessibleAction::Click));
    assert!(control.supports_action(AccessibleAction::Focus));
    assert!(!control.supports_action(AccessibleAction::Increment));

    let initial_history_len = cx.accessibility_tree_update_history(window).len();
    assert!(cx.dispatch_accessibility_action(
        window,
        ActionRequest {
            action: AccessibleAction::Increment,
            target_tree: TreeId::ROOT,
            target_node: control_id,
            data: None,
        },
    ));
    assert_eq!(cx.read(|cx| view.read(cx).activations), 0);
    assert_eq!(
        cx.accessibility_tree_update_history(window).len(),
        initial_history_len,
        "read-only value actions must remain inert"
    );

    assert!(cx.dispatch_accessibility_action(
        window,
        ActionRequest {
            action: AccessibleAction::Click,
            target_tree: TreeId::ROOT,
            target_node: control_id,
            data: None,
        },
    ));
    let activated = cx.latest_accessibility_tree_update(window).unwrap();
    let (activated_id, activated_control) = node_with_role(&activated, Role::TextInput);
    assert_eq!(activated_id, control_id);
    assert_eq!(activated_control.value(), Some("activation-1"));

    view.update(cx, |view, cx| {
        view.read_only = false;
        cx.notify();
    });
    cx.run_until_parked();
    let writable = cx.latest_accessibility_tree_update(window).unwrap();
    let (writable_id, writable_control) = node_with_role(&writable, Role::TextInput);
    assert_eq!(writable_id, control_id);
    assert!(writable_control.supports_action(AccessibleAction::Increment));
    assert!(cx.dispatch_accessibility_action(
        window,
        ActionRequest {
            action: AccessibleAction::Increment,
            target_tree: TreeId::ROOT,
            target_node: control_id,
            data: None,
        },
    ));
    assert_eq!(cx.read(|cx| view.read(cx).activations), 2);

    view.update(cx, |view, cx| {
        view.disabled = true;
        cx.notify();
    });
    cx.run_until_parked();
    let disabled = cx.latest_accessibility_tree_update(window).unwrap();
    let (disabled_id, disabled_control) = node_with_role(&disabled, Role::TextInput);
    assert_eq!(disabled_id, control_id);
    assert!(disabled_control.is_disabled());
    assert!(!disabled_control.supports_action(AccessibleAction::Click));
    assert!(!disabled_control.supports_action(AccessibleAction::Focus));

    let history_len = cx.accessibility_tree_update_history(window).len();
    assert!(cx.dispatch_accessibility_action(
        window,
        ActionRequest {
            action: AccessibleAction::Click,
            target_tree: TreeId::ROOT,
            target_node: control_id,
            data: None,
        },
    ));
    assert!(cx.dispatch_accessibility_action(
        window,
        ActionRequest {
            action: AccessibleAction::Focus,
            target_tree: TreeId::ROOT,
            target_node: control_id,
            data: None,
        },
    ));
    assert_eq!(
        cx.accessibility_tree_update_history(window).len(),
        history_len,
        "disabled actions must not mutate state or focus"
    );
}

#[open_gpui::test]
fn accessibility_harness_excludes_roleless_hidden_direct_and_deferred_subtrees(
    cx: &mut TestAppContext,
) {
    let typed_window = cx.open_window(size(px(320.0), px(200.0)), |_, _| {
        RolelessHiddenAccessibilityProbeView {
            hidden: false,
            activations: 0,
        }
    });
    let view = typed_window.root(cx).unwrap();
    let window = typed_window.into();

    assert!(cx.activate_accessibility(window));
    let visible = cx.latest_accessibility_tree_update(window).unwrap();
    let (direct_id, _) = node_with_label(&visible, "Roleless hidden direct action");
    let (deferred_id, _) = node_with_label(&visible, "Roleless hidden deferred action");

    view.update(cx, |view, cx| {
        view.hidden = true;
        cx.notify();
    });
    cx.run_until_parked();
    let hidden = cx.latest_accessibility_tree_update(window).unwrap();
    for id in [direct_id, deferred_id] {
        assert!(!hidden.nodes.iter().any(|(candidate, _)| *candidate == id));
    }

    let hidden_history_len = cx.accessibility_tree_update_history(window).len();
    for target_node in [direct_id, deferred_id] {
        assert!(cx.dispatch_accessibility_action(
            window,
            ActionRequest {
                action: AccessibleAction::Click,
                target_tree: TreeId::ROOT,
                target_node,
                data: None,
            },
        ));
    }
    assert_eq!(cx.read(|cx| view.read(cx).activations), 0);
    assert_eq!(
        cx.accessibility_tree_update_history(window).len(),
        hidden_history_len,
        "hidden direct and deferred actions must not schedule frames"
    );

    view.update(cx, |view, cx| {
        view.hidden = false;
        cx.notify();
    });
    cx.run_until_parked();
    let restored = cx.latest_accessibility_tree_update(window).unwrap();
    assert_eq!(
        node_with_label(&restored, "Roleless hidden direct action").0,
        direct_id
    );
    assert_eq!(
        node_with_label(&restored, "Roleless hidden deferred action").0,
        deferred_id
    );
}

#[open_gpui::test]
fn accessibility_harness_keeps_hidden_subtrees_closed_across_modal_scopes(cx: &mut TestAppContext) {
    let typed_window = cx.open_window(size(px(320.0), px(200.0)), |_, _| {
        HardHiddenModalAccessibilityProbeView {
            hidden: false,
            activations: 0,
        }
    });
    let view = typed_window.root(cx).unwrap();
    let window = typed_window.into();

    assert!(cx.activate_accessibility(window));
    let visible = cx.latest_accessibility_tree_update(window).unwrap();
    let (modal_id, _) = node_with_label(&visible, "Hard hidden modal root");
    let (descendant_id, _) = node_with_label(&visible, "Hard hidden modal descendant");

    view.update(cx, |view, cx| {
        view.hidden = true;
        cx.notify();
    });
    cx.run_until_parked();
    let hidden = cx.latest_accessibility_tree_update(window).unwrap();
    node_with_label(&hidden, "Visible outside hidden subtree");
    for id in [modal_id, descendant_id] {
        assert!(!hidden.nodes.iter().any(|(candidate, _)| *candidate == id));
    }

    let hidden_history_len = cx.accessibility_tree_update_history(window).len();
    for target_node in [modal_id, descendant_id] {
        assert!(cx.dispatch_accessibility_action(
            window,
            ActionRequest {
                action: AccessibleAction::Click,
                target_tree: TreeId::ROOT,
                target_node,
                data: None,
            },
        ));
    }
    assert_eq!(cx.read(|cx| view.read(cx).activations), 0);
    assert_eq!(
        cx.accessibility_tree_update_history(window).len(),
        hidden_history_len,
        "hard-hidden modal descendants must not schedule action frames"
    );
}
