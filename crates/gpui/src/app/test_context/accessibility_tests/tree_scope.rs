use super::*;

use crate::{
    SubtreePresentation, SubtreePresentationExt, SubtreeTransform, SubtreeTransformExt, fill,
    point, red,
};

struct LateFailedModalAccessibilityView;

impl Render for LateFailedModalAccessibilityView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("late-failed-modal-root")
            .role(Role::Group)
            .aria_label("Valid accessibility root")
            .child(
                div()
                    .id("late-failed-modal-sibling")
                    .role(Role::Button)
                    .aria_label("Valid accessibility sibling"),
            )
            .child(
                accessibility_scope(
                    AccessibilityTreeScope::ModalRoot,
                    div()
                        .id("late-failed-modal")
                        .role(Role::Dialog)
                        .aria_label("Late failed modal")
                        .aria_modal(true)
                        .child(canvas(
                            |_, _, _| {},
                            |_, _, window, _| {
                                window.paint_quad(fill(
                                    Bounds::new(
                                        point(px(f32::MAX), px(0.0)),
                                        size(px(10.0), px(10.0)),
                                    ),
                                    red(),
                                ));
                            },
                        )),
                )
                .with_subtree_transform(
                    SubtreeTransform::try_uniform_scale(2.0).expect("valid transform"),
                ),
            )
    }
}

#[open_gpui::test]
fn late_failed_modal_does_not_restrict_valid_accessibility_siblings(cx: &mut TestAppContext) {
    let window = cx.open_window(size(px(320.0), px(200.0)), |_, _| {
        LateFailedModalAccessibilityView
    });
    let window = window.into();

    assert!(cx.activate_accessibility(window));
    let update = cx.latest_accessibility_tree_update(window).unwrap();
    node_with_label(&update, "Valid accessibility sibling");
    assert!(
        update
            .nodes
            .iter()
            .all(|(_, node)| node.label() != Some("Late failed modal"))
    );
}

struct ModalAccessibilityProbeView {
    modal_open: bool,
    auxiliary_hidden: bool,
    underlay_activations: usize,
    underlay_focus: FocusHandle,
}

struct CachedModalAccessibilityChild {
    activations: usize,
}

impl Render for CachedModalAccessibilityChild {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let this = cx.entity().downgrade();
        accessibility_scope(
            AccessibilityTreeScope::ModalRoot,
            div()
                .id("cached-modal")
                .role(Role::Dialog)
                .aria_label("Cached modal")
                .aria_modal(true)
                .child(
                    div()
                        .id("cached-modal-action")
                        .role(Role::Button)
                        .aria_label("Cached modal action")
                        .aria_value(format!("activation-{}", self.activations))
                        .on_a11y_action(AccessibleAction::Click, move |_, _, cx| {
                            this.update(cx, |this, cx| {
                                this.activations += 1;
                                cx.notify();
                            })
                            .ok();
                        }),
                ),
        )
    }
}

struct CachedModalAccessibilityRoot {
    child: Entity<CachedModalAccessibilityChild>,
    revision: usize,
}

impl Render for CachedModalAccessibilityRoot {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("cached-modal-root")
            .role(Role::Group)
            .child(
                div()
                    .id("cached-underlay")
                    .role(Role::Button)
                    .aria_label(format!("Cached underlay {}", self.revision))
                    .on_a11y_action(AccessibleAction::Click, |_, _, _| {}),
            )
            .child(AnyView::from(self.child.clone()).cached(StyleRefinement::default().size_full()))
    }
}

impl Render for ModalAccessibilityProbeView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let this = cx.entity().downgrade();
        let mut root = div()
            .id("modal-accessibility-probe")
            .role(Role::Group)
            .child(
                div()
                    .id("underlay")
                    .role(Role::Button)
                    .aria_label("Underlay action")
                    .aria_value(format!("activation-{}", self.underlay_activations))
                    .focusable()
                    .track_focus(&self.underlay_focus)
                    .on_a11y_action(AccessibleAction::Click, move |_, _, cx| {
                        this.update(cx, |this, cx| {
                            this.underlay_activations += 1;
                            cx.notify();
                        })
                        .ok();
                    }),
            )
            .child(
                div()
                    .id("auxiliary")
                    .role(Role::Group)
                    .aria_label("Auxiliary group")
                    .child(
                        div()
                            .id("auxiliary-action")
                            .role(Role::Button)
                            .aria_label("Auxiliary action")
                            .on_a11y_action(AccessibleAction::Click, |_, _, _| {}),
                    )
                    .with_subtree_presentation(if self.auxiliary_hidden {
                        SubtreePresentation::Inert
                    } else {
                        SubtreePresentation::Visible
                    }),
            );

        if self.modal_open {
            root = root.child(accessibility_scope(
                AccessibilityTreeScope::ModalRoot,
                div()
                    .id("modal")
                    .role(Role::Dialog)
                    .aria_label("Active modal")
                    .aria_modal(true)
                    .child(
                        div()
                            .id("modal-action")
                            .role(Role::Button)
                            .aria_label("Modal action")
                            .on_a11y_action(AccessibleAction::Click, |_, _, _| {}),
                    ),
            ));
        }

        root
    }
}

struct ScopedAccessibilityTreeProbeView;

impl Render for ScopedAccessibilityTreeProbeView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("scope-probe-root")
            .role(Role::Group)
            .aria_label("Underlay ancestor")
            .child(
                div()
                    .id("scope-probe-underlay")
                    .role(Role::Button)
                    .aria_label("Scoped underlay action"),
            )
            .child(accessibility_scope(
                AccessibilityTreeScope::Excluded,
                accessibility_scope(
                    AccessibilityTreeScope::ModalRoot,
                    div()
                        .id("scope-probe-modal")
                        .role(Role::Dialog)
                        .aria_label("Scoped modal root")
                        .aria_modal(true)
                        .child(accessibility_scope(
                            AccessibilityTreeScope::Excluded,
                            div()
                                .id("scope-probe-nested-excluded")
                                .role(Role::Button)
                                .aria_label("Nested excluded action"),
                        )),
                ),
            ))
            .child(deferred(accessibility_scope(
                AccessibilityTreeScope::ModalDescendant,
                div()
                    .id("scope-probe-descendant")
                    .role(Role::Menu)
                    .aria_label("Deferred modal descendant"),
            )))
            .child(deferred(accessibility_scope(
                AccessibilityTreeScope::Excluded,
                div()
                    .id("scope-probe-unrelated")
                    .role(Role::Button)
                    .aria_label("Unrelated deferred surface"),
            )))
    }
}

struct ModalSemanticOnlyProbeView;

impl Render for ModalSemanticOnlyProbeView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("modal-semantic-only-root")
            .role(Role::Group)
            .aria_label("Semantic-only root")
            .child(
                div()
                    .id("modal-semantic-only-underlay")
                    .role(Role::Button)
                    .aria_label("Semantic-only underlay"),
            )
            .child(
                div()
                    .id("modal-semantic-only-dialog")
                    .role(Role::Dialog)
                    .aria_label("Semantic-only modal")
                    .aria_modal(true),
            )
    }
}

#[open_gpui::test]
fn accessibility_harness_filters_hidden_and_modal_subtrees(cx: &mut TestAppContext) {
    let typed_window = cx.open_window(size(px(320.0), px(200.0)), |_, cx| {
        ModalAccessibilityProbeView {
            modal_open: false,
            auxiliary_hidden: false,
            underlay_activations: 0,
            underlay_focus: cx.focus_handle(),
        }
    });
    let view = typed_window.root(cx).unwrap();
    let window = typed_window.into();

    assert!(cx.activate_accessibility(window));
    let initial = cx.latest_accessibility_tree_update(window).unwrap();
    let (underlay_id, _) = node_with_label(&initial, "Underlay action");
    let (auxiliary_id, _) = node_with_label(&initial, "Auxiliary group");
    let (auxiliary_action_id, _) = node_with_label(&initial, "Auxiliary action");

    view.update(cx, |view, cx| {
        view.auxiliary_hidden = true;
        cx.notify();
    });
    cx.run_until_parked();
    let hidden = cx.latest_accessibility_tree_update(window).unwrap();
    assert!(!hidden.nodes.iter().any(|(id, _)| *id == auxiliary_id));
    assert!(
        !hidden
            .nodes
            .iter()
            .any(|(id, _)| *id == auxiliary_action_id)
    );

    assert!(cx.dispatch_accessibility_action(
        window,
        ActionRequest {
            action: AccessibleAction::Focus,
            target_tree: TreeId::ROOT,
            target_node: underlay_id,
            data: None,
        },
    ));
    let focused = cx.latest_accessibility_tree_update(window).unwrap();
    assert_eq!(focused.focus, underlay_id);

    view.update(cx, |view, cx| {
        view.auxiliary_hidden = false;
        view.modal_open = true;
        cx.notify();
    });
    cx.run_until_parked();
    let modal = cx.latest_accessibility_tree_update(window).unwrap();
    let (modal_id, modal_node) = node_with_label(&modal, "Active modal");
    assert!(modal_node.is_modal());
    assert_eq!(modal.focus, modal_id);
    assert!(!modal.nodes.iter().any(|(id, _)| *id == underlay_id));
    assert!(!modal.nodes.iter().any(|(id, _)| *id == auxiliary_id));

    let modal_history_len = cx.accessibility_tree_update_history(window).len();
    assert!(cx.dispatch_accessibility_action(
        window,
        ActionRequest {
            action: AccessibleAction::Click,
            target_tree: TreeId::ROOT,
            target_node: underlay_id,
            data: None,
        },
    ));
    assert_eq!(
        cx.accessibility_tree_update_history(window).len(),
        modal_history_len,
        "an underlay action must be rejected while the modal is active"
    );

    view.update(cx, |view, cx| {
        view.modal_open = false;
        cx.notify();
    });
    cx.run_until_parked();
    let restored = cx.latest_accessibility_tree_update(window).unwrap();
    let (restored_underlay_id, restored_underlay) = node_with_label(&restored, "Underlay action");
    let (restored_auxiliary_id, _) = node_with_label(&restored, "Auxiliary group");
    assert_eq!(restored_underlay_id, underlay_id);
    assert_eq!(restored_auxiliary_id, auxiliary_id);
    assert_eq!(restored_underlay.value(), Some("activation-0"));

    assert!(cx.dispatch_accessibility_action(
        window,
        ActionRequest {
            action: AccessibleAction::Click,
            target_tree: TreeId::ROOT,
            target_node: underlay_id,
            data: None,
        },
    ));
    let activated = cx.latest_accessibility_tree_update(window).unwrap();
    assert_eq!(
        node_with_label(&activated, "Underlay action").1.value(),
        Some("activation-1")
    );
}

#[open_gpui::test]
fn accessibility_harness_projects_modal_scope_membership_across_deferred_roots(
    cx: &mut TestAppContext,
) {
    let window = cx
        .open_window(size(px(320.0), px(200.0)), |_, _| {
            ScopedAccessibilityTreeProbeView
        })
        .into();

    assert!(cx.activate_accessibility(window));
    let update = cx.latest_accessibility_tree_update(window).unwrap();
    let (modal_id, modal) = node_with_label(&update, "Scoped modal root");
    let (descendant_id, _) = node_with_label(&update, "Deferred modal descendant");
    assert!(modal.is_modal());
    for excluded_label in [
        "Underlay ancestor",
        "Scoped underlay action",
        "Nested excluded action",
        "Unrelated deferred surface",
    ] {
        assert!(
            !update
                .nodes
                .iter()
                .any(|(_, node)| node.label() == Some(excluded_label)),
            "{excluded_label:?} must not be published"
        );
    }

    let root = update
        .nodes
        .iter()
        .find(|(id, _)| *id == NodeId(0))
        .map(|(_, node)| node)
        .expect("published tree must retain the AccessKit root");
    assert_eq!(root.children(), &[modal_id, descendant_id]);
    assert_accessibility_tree_is_normalized_and_closed(&update);
}

#[open_gpui::test]
fn accessibility_harness_does_not_infer_modal_authority_from_node_semantics(
    cx: &mut TestAppContext,
) {
    let window = cx
        .open_window(size(px(320.0), px(200.0)), |_, _| {
            ModalSemanticOnlyProbeView
        })
        .into();

    assert!(cx.activate_accessibility(window));
    let update = cx.latest_accessibility_tree_update(window).unwrap();
    node_with_label(&update, "Semantic-only root");
    node_with_label(&update, "Semantic-only underlay");
    let (_, modal) = node_with_label(&update, "Semantic-only modal");
    assert!(modal.is_modal());
    assert_accessibility_tree_is_normalized_and_closed(&update);
}

#[open_gpui::test]
fn accessibility_harness_does_not_reuse_incomplete_cached_a11y_subtrees(cx: &mut TestAppContext) {
    let typed_window = cx.open_window(size(px(320.0), px(200.0)), |_, cx| {
        let child = cx.new(|_| CachedModalAccessibilityChild { activations: 0 });
        CachedModalAccessibilityRoot { child, revision: 0 }
    });
    let view = typed_window.root(cx).unwrap();
    let child = cx.read(|cx| view.read(cx).child.clone());
    let window = typed_window.into();

    assert!(cx.activate_accessibility(window));
    let initial = cx.latest_accessibility_tree_update(window).unwrap();
    let (modal_id, modal) = node_with_label(&initial, "Cached modal");
    let (action_id, action) = node_with_label(&initial, "Cached modal action");
    assert!(modal.is_modal());
    assert_eq!(action.value(), Some("activation-0"));
    assert!(
        !initial
            .nodes
            .iter()
            .any(|(_, node)| node.label() == Some("Cached underlay 0"))
    );
    assert!(cx.dispatch_accessibility_action(
        window,
        ActionRequest {
            action: AccessibleAction::Click,
            target_tree: TreeId::ROOT,
            target_node: action_id,
            data: None,
        },
    ));
    assert_eq!(cx.read(|cx| child.read(cx).activations), 1);

    view.update(cx, |view, cx| {
        view.revision += 1;
        cx.notify();
    });
    cx.run_until_parked();
    let refreshed = cx.latest_accessibility_tree_update(window).unwrap();
    let (refreshed_modal_id, refreshed_modal) = node_with_label(&refreshed, "Cached modal");
    assert_eq!(refreshed_modal_id, modal_id);
    assert!(refreshed_modal.is_modal());
    let (refreshed_action_id, refreshed_action) =
        node_with_label(&refreshed, "Cached modal action");
    assert_eq!(refreshed_action_id, action_id);
    assert_eq!(refreshed_action.value(), Some("activation-1"));
    assert!(
        !refreshed
            .nodes
            .iter()
            .any(|(_, node)| node.label() == Some("Cached underlay 1"))
    );
    assert!(cx.dispatch_accessibility_action(
        window,
        ActionRequest {
            action: AccessibleAction::Click,
            target_tree: TreeId::ROOT,
            target_node: refreshed_action_id,
            data: None,
        },
    ));
    assert_eq!(cx.read(|cx| child.read(cx).activations), 2);
}
