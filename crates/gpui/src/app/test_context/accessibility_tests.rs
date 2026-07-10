use crate::{
    AccessibleAction, Context, FocusHandle, InteractiveElement, IntoElement, ParentElement, Render,
    Role, StatefulInteractiveElement, TestAppContext, Window, div, px, size,
};
use accesskit::{ActionRequest, Node, NodeId, TreeId, TreeUpdate};
use std::collections::HashSet;

struct AccessibilityProbeView {
    root_id: &'static str,
    count: usize,
    show_details: bool,
    relations: Option<(NodeId, NodeId)>,
    focus: FocusHandle,
}

impl Render for AccessibilityProbeView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let mut control = div()
            .id("control")
            .role(Role::SpinButton)
            .aria_label(format!("Counter {}", self.count))
            .aria_numeric_value(self.count as f64)
            .focusable()
            .tab_stop(true)
            .track_focus(&self.focus)
            .on_a11y_action(AccessibleAction::Increment, {
                let this = cx.entity().downgrade();
                move |_, _, cx| {
                    this.update(cx, |this, cx| {
                        this.count += 1;
                        cx.notify();
                    })
                    .ok();
                }
            });

        if let Some((label, details)) = self.relations {
            control = control.aria_labelled_by([label]).aria_controls([details]);
        }

        let mut root = div()
            .id(self.root_id)
            .role(Role::Group)
            .child(
                div()
                    .id("label")
                    .role(Role::Label)
                    .aria_label("Counter label"),
            )
            .child(control);
        if self.show_details {
            root = root.child(
                div()
                    .id("details")
                    .role(Role::List)
                    .aria_label("Counter details"),
            );
        }
        root
    }
}

fn node_with_role(update: &TreeUpdate, role: Role) -> (NodeId, &Node) {
    update
        .nodes
        .iter()
        .find(|(_, node)| node.role() == role)
        .map(|(id, node)| (*id, node))
        .unwrap_or_else(|| panic!("missing {role:?} node"))
}

fn assert_accessibility_tree_is_normalized_and_closed(update: &TreeUpdate) {
    assert!(
        update.nodes.windows(2).all(|nodes| nodes[0].0 < nodes[1].0),
        "test-facing accessibility updates must be sorted by node ID"
    );
    let node_ids = update
        .nodes
        .iter()
        .map(|(id, _)| *id)
        .collect::<HashSet<_>>();
    let root = update
        .tree
        .as_ref()
        .expect("full update must include a tree")
        .root;
    assert!(node_ids.contains(&root));
    assert!(node_ids.contains(&update.focus));
    for (node_id, node) in &update.nodes {
        for referenced_id in node
            .children()
            .iter()
            .chain(node.controls())
            .chain(node.labelled_by())
        {
            assert!(
                node_ids.contains(referenced_id),
                "node {node_id:?} contains dangling reference {referenced_id:?}"
            );
        }
    }
}

#[open_gpui::test]
fn accessibility_harness_observes_final_tree_lifecycle(cx: &mut TestAppContext) {
    let typed_window = cx.open_window(size(px(320.0), px(200.0)), |_, cx| AccessibilityProbeView {
        root_id: "accessibility-probe",
        count: 0,
        show_details: true,
        relations: None,
        focus: cx.focus_handle(),
    });
    let view = typed_window.root(cx).unwrap();
    let window = typed_window.into();

    assert_eq!(cx.latest_accessibility_tree_update(window), None);
    assert!(cx.activate_accessibility(window));

    let initial = cx
        .latest_accessibility_tree_update(window)
        .expect("activation must render a full accessibility tree");
    assert_accessibility_tree_is_normalized_and_closed(&initial);
    assert_eq!(initial.tree.as_ref().unwrap().root, NodeId(0));
    assert!(
        initial.nodes.len() > 1,
        "the final tree must not be a placeholder"
    );

    let activation_update_count = cx.accessibility_tree_update_history(window).len();
    assert!(cx.activate_accessibility(window));
    assert_eq!(
        cx.accessibility_tree_update_history(window).len(),
        activation_update_count + 1
    );

    let (control_id, control) = node_with_role(&initial, Role::SpinButton);
    let (label_id, _) = node_with_role(&initial, Role::Label);
    let (details_id, _) = node_with_role(&initial, Role::List);
    assert_eq!(control.numeric_value(), Some(0.0));

    view.update(cx, |view, cx| {
        view.relations = Some((label_id, details_id));
        cx.notify();
    });
    cx.run_until_parked();
    let related = cx.latest_accessibility_tree_update(window).unwrap();
    let (related_control_id, related_control) = node_with_role(&related, Role::SpinButton);
    assert_eq!(related_control_id, control_id);
    assert_eq!(related_control.labelled_by(), &[label_id]);
    assert_eq!(related_control.controls(), &[details_id]);
    assert_accessibility_tree_is_normalized_and_closed(&related);

    typed_window
        .update(cx, |view, window, cx| view.focus.focus(window, cx))
        .unwrap();
    cx.run_until_parked();
    let focused = cx.latest_accessibility_tree_update(window).unwrap();
    assert_eq!(focused.focus, control_id);
    assert_accessibility_tree_is_normalized_and_closed(&focused);

    assert!(cx.dispatch_accessibility_action(
        window,
        ActionRequest {
            action: AccessibleAction::Increment,
            target_tree: TreeId::ROOT,
            target_node: control_id,
            data: None,
        },
    ));
    let incremented = cx.latest_accessibility_tree_update(window).unwrap();
    let (incremented_id, incremented_control) = node_with_role(&incremented, Role::SpinButton);
    assert_eq!(incremented_id, control_id);
    assert_eq!(incremented_control.numeric_value(), Some(1.0));

    view.update(cx, |view, cx| {
        view.show_details = false;
        cx.notify();
    });
    cx.run_until_parked();
    let unmounted = cx.latest_accessibility_tree_update(window).unwrap();
    assert!(!unmounted.nodes.iter().any(|(id, _)| *id == details_id));
    let (_, unmounted_control) = node_with_role(&unmounted, Role::SpinButton);
    assert!(unmounted_control.controls().is_empty());
    assert_eq!(unmounted_control.labelled_by(), &[label_id]);
    assert_accessibility_tree_is_normalized_and_closed(&unmounted);

    let update_count = cx.accessibility_tree_update_history(window).len();
    assert!(cx.deactivate_accessibility(window));
    view.update(cx, |view, cx| {
        view.count += 1;
        cx.notify();
    });
    cx.run_until_parked();
    assert_eq!(
        cx.accessibility_tree_update_history(window).len(),
        update_count
    );
    assert!(!cx.dispatch_accessibility_action(
        window,
        ActionRequest {
            action: AccessibleAction::Increment,
            target_tree: TreeId::ROOT,
            target_node: control_id,
            data: None,
        },
    ));

    assert!(cx.activate_accessibility(window));
    let reactivated = cx.latest_accessibility_tree_update(window).unwrap();
    let (reactivated_id, reactivated_control) = node_with_role(&reactivated, Role::SpinButton);
    assert_eq!(reactivated_id, control_id);
    assert_eq!(reactivated_control.numeric_value(), Some(2.0));
    assert_eq!(
        cx.accessibility_tree_update_history(window).len(),
        update_count + 1
    );
}

#[open_gpui::test]
fn accessibility_harness_isolates_window_trees_and_actions(cx: &mut TestAppContext) {
    let first_window = cx.open_window(size(px(320.0), px(200.0)), |_, cx| AccessibilityProbeView {
        root_id: "shared-probe",
        count: 0,
        show_details: false,
        relations: None,
        focus: cx.focus_handle(),
    });
    let second_window =
        cx.open_window(size(px(320.0), px(200.0)), |_, cx| AccessibilityProbeView {
            root_id: "shared-probe",
            count: 10,
            show_details: false,
            relations: None,
            focus: cx.focus_handle(),
        });
    let first = first_window.into();
    let second = second_window.into();

    assert!(cx.activate_accessibility(first));
    assert!(cx.activate_accessibility(second));
    let first_tree = cx.latest_accessibility_tree_update(first).unwrap();
    let second_tree = cx.latest_accessibility_tree_update(second).unwrap();
    let (first_control_id, first_control) = node_with_role(&first_tree, Role::SpinButton);
    let (_, second_control) = node_with_role(&second_tree, Role::SpinButton);
    assert_eq!(first_control.numeric_value(), Some(0.0));
    assert_eq!(second_control.numeric_value(), Some(10.0));

    let second_history_len = cx.accessibility_tree_update_history(second).len();
    assert!(cx.dispatch_accessibility_action(
        first,
        ActionRequest {
            action: AccessibleAction::Increment,
            target_tree: TreeId::ROOT,
            target_node: first_control_id,
            data: None,
        },
    ));
    assert_eq!(
        node_with_role(
            &cx.latest_accessibility_tree_update(first).unwrap(),
            Role::SpinButton,
        )
        .1
        .numeric_value(),
        Some(1.0)
    );
    assert_eq!(
        node_with_role(
            &cx.latest_accessibility_tree_update(second).unwrap(),
            Role::SpinButton,
        )
        .1
        .numeric_value(),
        Some(10.0)
    );
    assert_eq!(
        cx.accessibility_tree_update_history(second).len(),
        second_history_len
    );
}

#[open_gpui::test]
fn accessibility_inaccessible_test_window_remains_inert(cx: &mut TestAppContext) {
    cx.update(|app| app.accessibility_force_disabled = true);
    let window = cx
        .open_window(size(px(320.0), px(200.0)), |_, cx| AccessibilityProbeView {
            root_id: "inaccessible-probe",
            count: 0,
            show_details: false,
            relations: None,
            focus: cx.focus_handle(),
        })
        .into();

    assert!(!cx.activate_accessibility(window));
    assert_eq!(cx.latest_accessibility_tree_update(window), None);
    assert!(cx.accessibility_tree_update_history(window).is_empty());
    assert!(!cx.dispatch_accessibility_action(
        window,
        ActionRequest {
            action: AccessibleAction::Increment,
            target_tree: TreeId::ROOT,
            target_node: NodeId(1),
            data: None,
        },
    ));
    assert!(!cx.deactivate_accessibility(window));
}
