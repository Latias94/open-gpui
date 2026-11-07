use super::*;
use accesskit::{Node, Role};
use serde_json::Value;

fn active_a11y() -> A11y {
    let state = Arc::new(AtomicU64::new(0));
    set_requested_active(&state, true);
    let mut a11y = A11y::new(state, false, WindowId::from(17));
    a11y.sync_active_flag();
    a11y
}

#[track_caller]
fn subtree<'a>(a11y: &'a mut A11y, parent: NodeId, _element_id: &str) -> A11ySubtreeBuilder<'a> {
    let builder = A11ySubtreeBuilder::new(parent, &mut a11y.nodes);
    #[cfg(debug_assertions)]
    let builder = builder.with_creator(debug::NodeCreator {
        view: Some("tests::SyntheticView"),
        element_id: Some(_element_id.into()),
        source_location: Some(core::panic::Location::caller()),
    });
    builder
}

fn publish(a11y: &mut A11y, update: &TreeUpdate, frame_generation: u64) {
    let activation_generation = a11y.activation_generation();
    a11y.publish(
        update,
        activation_generation,
        debug::FrameDebugInfo {
            frame_generation,
            activation_generation,
            ..Default::default()
        },
    );
}

fn node(update: &TreeUpdate, id: NodeId) -> &Node {
    &update
        .nodes
        .iter()
        .find(|(candidate, _)| *candidate == id)
        .unwrap()
        .1
}

fn snapshot(a11y: &A11y) -> Value {
    serde_json::from_str(&a11y.debug_tree_json().expect("captured publication")).unwrap()
}

fn snapshot_node(snapshot: &Value, id: NodeId) -> &Value {
    snapshot["nodes"]
        .as_object()
        .unwrap()
        .values()
        .find(|node| node["accesskit_id"] == id.0.to_string())
        .unwrap()
}

#[::std::prelude::v1::test]
#[cfg_attr(debug_assertions, should_panic(expected = "Duplicate a11y node id"))]
fn synthetic_children_cannot_reuse_the_reserved_window_root_id() {
    let mut a11y = active_a11y();
    a11y.begin_frame();
    let pushed = subtree(&mut a11y, ROOT_NODE_ID, "root-collision")
        .push_child(ROOT_NODE_ID, Node::new(Role::Label));
    assert!(!pushed, "the window root is already a member of the tree");
    let update = a11y.end_frame();
    assert_eq!(update.nodes.len(), 1);
    assert_eq!(update.nodes[0].0, ROOT_NODE_ID);
    assert!(update.nodes[0].1.children().is_empty());
}

#[::std::prelude::v1::test]
fn synthetic_ids_and_leaf_parentage_survive_rebuilding() {
    let mut a11y = active_a11y();
    let mut previous = None;
    for frame in 1..=2 {
        a11y.begin_frame();
        let mut identities = Vec::new();
        for parent in [NodeId(1), NodeId(2)] {
            assert!(a11y.nodes.push(parent, Node::new(Role::Group)));
            let children = {
                let mut builder = subtree(&mut a11y, parent, "parent");
                let children = [
                    builder.synthetic_node_id("first"),
                    builder.synthetic_node_id("second"),
                ];
                assert_eq!(children[0], builder.synthetic_node_id("first"));
                assert_ne!(children[0], children[1]);
                builder.parent_node().set_row_count(7);
                builder
                    .parent_node()
                    .set_description("  Arbitrary parent property\n日本語  ");
                for child in children {
                    assert!(builder.push_child(child, Node::new(Role::Label)));
                }
                children
            };
            assert!(a11y.nodes.is_current_node(parent));
            assert_eq!(a11y.nodes.ids_stack.as_slice(), &[ROOT_NODE_ID, parent]);
            a11y.nodes.pop();
            identities.push((parent, children));
        }
        assert_ne!(identities[0].1[0], identities[1].1[0]);
        if let Some(previous) = &previous {
            assert_eq!(&identities, previous);
        }
        let update = a11y.end_frame();
        assert_eq!(update.nodes.len(), 7);
        assert_eq!(
            node(&update, ROOT_NODE_ID).children(),
            &[NodeId(1), NodeId(2)]
        );
        for (parent, children) in &identities {
            assert_eq!(node(&update, *parent).children(), children);
            assert_eq!(node(&update, *parent).row_count(), Some(7));
            for child in children {
                assert!(node(&update, *child).children().is_empty());
            }
        }
        publish(&mut a11y, &update, frame);
        assert_eq!(snapshot(&a11y)["frame"]["frame_generation"], frame);
        previous = Some(identities);
    }
}

#[::std::prelude::v1::test]
fn nested_checkpoints_restore_the_complete_parent_and_release_synthetic_provenance() {
    let mut a11y = active_a11y();
    let parent = NodeId(1);
    a11y.begin_frame();
    assert!(a11y.nodes.push(parent, Node::new(Role::Table)));
    let kept = {
        let mut builder = subtree(&mut a11y, parent, "kept");
        builder.parent_node().set_label("Original parent");
        builder.parent_node().set_row_count(2);
        let id = builder.synthetic_node_id("kept");
        assert!(builder.push_child(id, Node::new(Role::Label)));
        id
    };
    let original = a11y.nodes.nodes_stack.last().unwrap().clone();
    let outer = a11y.prepaint_checkpoint();
    let rolled_back = {
        let mut builder = subtree(&mut a11y, parent, "rolled-back");
        let node = builder.parent_node();
        node.set_role(Role::TextInput);
        node.set_description("Speculative description");
        node.set_numeric_value(42.5);
        node.set_read_only();
        node.add_action(Action::SetValue);
        node.set_children([]);
        let id = builder.synthetic_node_id("reusable");
        assert!(builder.push_child(id, Node::new(Role::Button)));
        id
    };
    let outer_parent = a11y.nodes.nodes_stack.last().unwrap().clone();
    let inner = a11y.prepaint_checkpoint();
    let inner_id = {
        let mut builder = subtree(&mut a11y, parent, "inner");
        builder.parent_node().set_children([kept]);
        builder.parent_node().clear_numeric_value();
        builder.parent_node().set_value("Inner speculative value");
        let id = builder.synthetic_node_id("inner");
        assert!(builder.push_child(id, Node::new(Role::Label)));
        id
    };
    a11y.rollback_prepaint(inner);
    assert_eq!(a11y.nodes.nodes_stack.last().unwrap(), &outer_parent);
    assert!(a11y.nodes.has_node(rolled_back));
    assert!(!a11y.nodes.has_node(inner_id));
    #[cfg(debug_assertions)]
    assert!(!a11y.nodes.node_info.contains_key(&inner_id));
    a11y.rollback_prepaint(outer);
    assert_eq!(a11y.nodes.nodes_stack.last().unwrap(), &original);
    assert!(!a11y.nodes.has_node(rolled_back));
    #[cfg(debug_assertions)]
    {
        assert!(!a11y.nodes.node_info.contains_key(&rolled_back));
        assert!(a11y.nodes.node_info.contains_key(&kept));
    }
    let mut builder = subtree(&mut a11y, parent, "replacement");
    assert_eq!(builder.synthetic_node_id("reusable"), rolled_back);
    assert!(builder.push_child(rolled_back, Node::new(Role::Label)));
    a11y.nodes.pop();
    let update = a11y.end_frame();
    assert_eq!(update.nodes.len(), 4);
    assert_eq!(node(&update, parent).children(), &[kept, rolled_back]);
    assert!(!update.nodes.iter().any(|(id, _)| *id == inner_id));
    publish(&mut a11y, &update, 1);
    let dump = snapshot(&a11y);
    let reused = snapshot_node(&dump, rolled_back);
    assert_eq!(reused["aria"]["role"], "Label");
    #[cfg(debug_assertions)]
    {
        assert_eq!(reused["synthetic"], true);
        assert_eq!(reused["element_id"], "replacement");
        assert_eq!(reused["view"], "tests::SyntheticView");
        assert!(
            reused["source_location"]
                .as_str()
                .unwrap()
                .contains("synthetic_tests.rs")
        );
    }
    #[cfg(not(debug_assertions))]
    for key in ["synthetic", "element_id", "view", "source_location"] {
        assert!(reused.get(key).is_none());
    }
}

#[::std::prelude::v1::test]
fn rewriting_new_children_cannot_break_complete_parent_rollback() {
    for rewritten in [vec![], vec![NodeId(2), NodeId(1)]] {
        let mut a11y = active_a11y();
        a11y.begin_frame();
        let original = a11y.nodes.nodes_stack.last().unwrap().clone();
        let checkpoint = a11y.prepaint_checkpoint();
        let mut builder = subtree(&mut a11y, ROOT_NODE_ID, "rewritten");
        for id in [NodeId(1), NodeId(2)] {
            assert!(builder.push_child(id, Node::new(Role::Label)));
        }
        builder.parent_node().set_children(rewritten);
        a11y.rollback_prepaint(checkpoint);
        assert_eq!(a11y.nodes.nodes_stack.last().unwrap(), &original);
        for id in [NodeId(1), NodeId(2)] {
            assert!(!a11y.nodes.has_node(id));
        }
        assert_eq!(a11y.end_frame().nodes, vec![(ROOT_NODE_ID, original)]);
    }
}

#[::std::prelude::v1::test]
fn rewriting_deferred_children_restores_the_checkpoint_parent() {
    let mut a11y = active_a11y();
    a11y.begin_frame();
    let parent = a11y.nodes.reserve_deferred_parent().unwrap();
    assert!(a11y.nodes.push_leaf(NodeId(9), Node::new(Role::Label)));
    let original = a11y.nodes.nodes_stack.last().unwrap().clone();
    let checkpoint = a11y.prepaint_checkpoint();
    {
        let _scope = a11y.nodes.enter_deferred_parent_scope(parent);
        assert!(
            subtree(&mut a11y, ROOT_NODE_ID, "deferred")
                .push_child(NodeId(1), Node::new(Role::Label))
        );
    }
    assert!(!a11y.nodes.parent_child_mutations.is_empty());
    subtree(&mut a11y, ROOT_NODE_ID, "rewriter")
        .parent_node()
        .clear_children();
    a11y.rollback_prepaint(checkpoint);
    assert_eq!(a11y.nodes.nodes_stack.last().unwrap(), &original);
    assert!(!a11y.nodes.has_node(NodeId(1)));
    let update = a11y.end_frame();
    assert_eq!(node(&update, ROOT_NODE_ID), &original);
    assert_eq!(update.nodes.len(), 2);
}

#[::std::prelude::v1::test]
fn rejected_synthetic_children_and_their_provenance_never_enter_the_snapshot() {
    for filter in ["geometry", "modal", "excluded", "hidden"] {
        let mut a11y = active_a11y();
        a11y.begin_frame();
        let kept = NodeId(1);
        let rejected = NodeId(2);
        {
            let scope = if filter == "modal" {
                AccessibilityTreeScope::ModalRoot
            } else {
                AccessibilityTreeScope::Unrestricted
            };
            let _scope = a11y.nodes.enter_scope(scope);
            assert!(
                subtree(&mut a11y, ROOT_NODE_ID, "kept").push_child(kept, Node::new(Role::Label))
            );
        }
        let validity = SubtreeGeometryValidity::new(None);
        {
            let _geometry = a11y.nodes.enter_geometry_validity(
                (filter == "geometry")
                    .then(|| SubtreeGeometryValidity::new(Some(validity.clone()))),
            );
            let scope = if filter == "excluded" {
                AccessibilityTreeScope::Excluded
            } else {
                AccessibilityTreeScope::Unrestricted
            };
            let _scope = a11y.nodes.enter_scope(scope);
            let mut child = Node::new(Role::Label);
            if filter == "hidden" {
                child.set_hidden();
            }
            let admitted = subtree(&mut a11y, ROOT_NODE_ID, "rejected").push_child(rejected, child);
            assert_eq!(admitted, filter != "excluded", "{filter}");
        }
        validity.invalidate(crate::geometry::SubtreeGeometryError::DeviceConversion);
        let update = a11y.end_frame();
        assert_eq!(update.nodes.len(), 2, "{filter}");
        assert_eq!(node(&update, ROOT_NODE_ID).children(), &[kept], "{filter}");
        assert!(
            !update.nodes.iter().any(|(id, _)| *id == rejected),
            "{filter}"
        );
        publish(&mut a11y, &update, 1);
        let dump = snapshot(&a11y);
        assert_eq!(dump["nodes"].as_object().unwrap().len(), 2, "{filter}");
        assert!(!dump.to_string().contains("rejected"), "{filter}");
        #[cfg(debug_assertions)]
        {
            assert_eq!(a11y.nodes.node_info.len(), 1, "{filter}");
            assert_eq!(snapshot_node(&dump, kept)["element_id"], "kept");
            assert_eq!(snapshot_node(&dump, kept)["synthetic"], true);
        }
    }
}

#[::std::prelude::v1::test]
fn candidate_construction_and_discard_preserve_the_exact_previous_capture() {
    let mut a11y = active_a11y();
    a11y.window_title = Some("Published title".into());
    a11y.begin_frame();
    assert!(
        subtree(&mut a11y, ROOT_NODE_ID, "published").push_child(NodeId(1), Node::new(Role::Label))
    );
    let update = a11y.end_frame();
    assert!(a11y.debug_tree_json().is_none());
    publish(&mut a11y, &update, 41);
    let captured = a11y.debug_tree_json().unwrap();
    let revision = a11y.published_revision_for_test();
    a11y.window_title = Some("Discarded title".into());
    a11y.begin_frame();
    assert!(
        subtree(&mut a11y, ROOT_NODE_ID, "discarded")
            .push_child(NodeId(1), Node::new(Role::Button))
    );
    let mut candidate = a11y.end_frame();
    assert_eq!(a11y.debug_tree_json().as_deref(), Some(captured.as_str()));
    a11y.discard_candidate_frame();
    candidate.nodes.clear();
    assert_eq!(a11y.debug_tree_json().as_deref(), Some(captured.as_str()));
    assert_eq!(a11y.published_revision_for_test(), revision);
    let dump = snapshot(&a11y);
    assert_eq!(dump["frame"]["frame_number"], 1);
    assert_eq!(dump["frame"]["frame_generation"], 41);
    assert_eq!(dump["frame"]["window_title"], "Published title");
}

#[::std::prelude::v1::test]
fn lifecycle_boundaries_hide_stale_captures_and_release_the_payload() {
    for boundary in ["deactivate", "replace", "close", "quiesce"] {
        let mut a11y = active_a11y();
        a11y.begin_frame();
        assert!(
            subtree(&mut a11y, ROOT_NODE_ID, "old-generation")
                .push_child(NodeId(1), Node::new(Role::Label))
        );
        let update = a11y.end_frame();
        publish(&mut a11y, &update, 1);
        assert!(a11y.debug_tree_json().is_some(), "{boundary}");
        match boundary {
            "deactivate" | "replace" => {
                set_requested_active(&a11y.active_state, boundary == "replace");
                assert!(a11y.debug_tree_json().is_none(), "{boundary}");
                a11y.sync_active_flag();
            }
            "close" => a11y.clear_announcements_for_window_close(),
            "quiesce" => a11y.clear_announcements_for_interaction_quiescence(),
            _ => unreachable!(),
        }
        assert!(a11y.debug_tree_json().is_none(), "{boundary}");
        assert!(a11y.debug.to_json().is_none(), "{boundary}");
        assert_eq!(a11y.debug_activation_generation, None, "{boundary}");
        if matches!(boundary, "close" | "quiesce") {
            publish(&mut a11y, &update, 2);
            assert!(a11y.debug_tree_json().is_none(), "{boundary}");
            assert!(a11y.debug.to_json().is_none(), "{boundary}");
        }
    }
}

#[::std::prelude::v1::test]
fn explicit_capture_keeps_raw_announcement_text_while_diagnostics_remain_metadata_only() {
    const CANARY: &str = "synthetic-explicit-announcement-canary-5d3e";
    let message = format!("  {CANARY} \"raw\"\n日本語 🦀  ");
    let mut a11y = active_a11y();
    let outcome = a11y.enqueue_announcement(AccessibilityAnnouncement::polite(message.clone()));
    assert!(outcome.is_accepted());
    a11y.begin_frame();
    let update = a11y.end_frame();
    let (id, announcement) = update
        .nodes
        .iter()
        .find(|(_, node)| node.live().is_some())
        .unwrap();
    assert_eq!(announcement.label(), Some(message.as_str()));
    assert_eq!(announcement.value(), Some(message.as_str()));
    publish(&mut a11y, &update, 1);
    let dump = snapshot(&a11y);
    assert_eq!(snapshot_node(&dump, *id)["aria"]["label"], message);
    assert_eq!(snapshot_node(&dump, *id)["aria"]["value"], message);
    assert!(matches!(
        a11y.announcements.front(),
        Some(QueuedAnnouncement::Retained(_))
    ));
    let lifecycles = a11y
        .announcement_diagnostics()
        .iter()
        .map(|entry| entry.lifecycle())
        .collect::<Vec<_>>();
    assert_eq!(
        lifecycles,
        [
            AccessibilityAnnouncementLifecycle::Accepted,
            AccessibilityAnnouncementLifecycle::Committed,
        ]
    );
    assert!(!format!("{:?}", a11y.announcement_diagnostics()).contains(CANARY));
    a11y.begin_frame();
    let removed = a11y.end_frame();
    assert!(removed.nodes.iter().all(|(_, node)| node.live().is_none()));
    assert_eq!(
        snapshot_node(&snapshot(&a11y), *id)["aria"]["value"],
        message
    );
    publish(&mut a11y, &removed, 2);
    assert!(!a11y.debug_tree_json().unwrap().contains(CANARY));
    assert_eq!(
        a11y.announcement_diagnostics().last().unwrap().lifecycle(),
        AccessibilityAnnouncementLifecycle::Removed,
    );
    assert!(!format!("{:?}", a11y.announcement_diagnostics()).contains(CANARY));
}
