//! Private regressions for Zed changes 83d4847462 and 2268045a11, checked against
//! read-only reference source 635026fabc38440fce5e1767d125026a17b12012.

use super::*;
use accesskit::{Node, Role};
use slotmap::SlotMap;

const MENU: NodeId = NodeId(1);
const GROUP: NodeId = NodeId(2);
const ITEM: NodeId = NodeId(3);
const OUTSIDE: NodeId = NodeId(4);

fn fixture() -> (A11y, SlotMap<FocusId, ()>) {
    let state = Arc::new(AtomicU64::new(0));
    set_requested_active(&state, true);
    let mut a11y = A11y::new(state, false, WindowId::from(19));
    a11y.sync_active_flag();
    (a11y, SlotMap::with_key())
}

fn push(a11y: &mut A11y, id: NodeId, role: Role, focus: Option<FocusId>) {
    let mut node = Node::new(role);
    if focus.is_some() {
        node.add_action(Action::Focus);
    }
    assert!(a11y.nodes.push(id, node));
    if let Some(focus) = focus {
        a11y.record_focus_id(id, focus);
    }
}

fn claim(a11y: &mut A11y, id: NodeId) {
    push(a11y, id, Role::MenuItem, None);
    a11y.record_active_descendant(id);
    a11y.nodes.pop();
}

fn node(update: &TreeUpdate, id: NodeId) -> &Node {
    &update
        .nodes
        .iter()
        .find(|(candidate, _)| *candidate == id)
        .unwrap()
        .1
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

#[::std::prelude::v1::test]
fn only_actual_container_focus_grants_a_strict_active_descendant() {
    let (mut a11y, mut focuses) = fixture();
    let menu = focuses.insert(());
    let outside = focuses.insert(());
    let missing = focuses.insert(());
    let mut frame = 0;
    for deep in [false, true] {
        for (keyboard, accessibility, expected, selected) in [
            (Some(menu), None, ITEM, true),
            (Some(menu), Some(menu), ITEM, true),
            (None, None, ROOT_NODE_ID, true),
            (Some(outside), None, OUTSIDE, true),
            (Some(missing), None, ROOT_NODE_ID, true),
            (None, Some(menu), MENU, true),
            (Some(outside), Some(menu), MENU, true),
            (Some(menu), Some(outside), OUTSIDE, true),
            (Some(menu), Some(missing), ROOT_NODE_ID, true),
            (Some(menu), None, MENU, false),
        ] {
            a11y.begin_frame();
            push(&mut a11y, MENU, Role::Menu, Some(menu));
            if deep {
                push(&mut a11y, GROUP, Role::Group, None);
            }
            push(&mut a11y, ITEM, Role::MenuItem, None);
            if selected {
                a11y.record_active_descendant(ITEM);
                a11y.record_active_descendant(ITEM);
            }
            a11y.nodes.pop();
            if deep {
                a11y.nodes.pop();
            }
            a11y.nodes.pop();
            push(&mut a11y, OUTSIDE, Role::Button, Some(outside));
            a11y.nodes.pop();
            a11y.resolve_focus(keyboard, accessibility);
            let update = a11y.end_frame();
            assert_eq!(
                update.focus, expected,
                "{deep:?}, {keyboard:?}, {accessibility:?}"
            );
            assert_eq!(
                a11y.nodes.active_descendant,
                (expected == ITEM).then_some(ITEM)
            );
            frame += 1;
            publish(&mut a11y, &update, frame);
            let dump: serde_json::Value =
                serde_json::from_str(&a11y.debug_tree_json().unwrap()).unwrap();
            let displayed = dump["gpui_focus"].as_str().unwrap();
            let container = if expected == ITEM { MENU } else { expected };
            assert_eq!(
                dump["nodes"][displayed]["accesskit_id"],
                container.0.to_string()
            );
            if expected == ITEM {
                let active = dump["active_descendant_focus"].as_str().unwrap();
                assert_eq!(dump["nodes"][active]["accesskit_id"], ITEM.0.to_string());
            } else {
                assert!(dump["active_descendant_focus"].is_null());
            }
            assert_eq!(a11y.published_focus_id(MENU), Some(menu));
        }
    }
}

#[::std::prelude::v1::test]
fn deferred_children_but_not_window_portals_remain_semantic_descendants() {
    let (mut a11y, mut focuses) = fixture();
    let menu = focuses.insert(());
    for portal in [false, true] {
        a11y.begin_frame();
        push(&mut a11y, MENU, Role::Menu, Some(menu));
        let parent = if portal {
            a11y.nodes.reserve_window_portal_parent().unwrap()
        } else {
            a11y.nodes.reserve_deferred_parent().unwrap()
        };
        a11y.nodes.pop();
        if portal {
            let _portal = a11y.nodes.enter_window_portal_scope(parent);
            claim(&mut a11y, ITEM);
        } else {
            let _deferred = a11y.nodes.enter_deferred_parent_scope(parent);
            claim(&mut a11y, ITEM);
        }
        a11y.resolve_focus(Some(menu), None);
        let update = a11y.end_frame();
        assert_eq!(update.focus, if portal { MENU } else { ITEM });
        assert_eq!(a11y.nodes.active_descendant, (!portal).then_some(ITEM));
        let (children, roots): (&[NodeId], &[NodeId]) = if portal {
            (&[], &[MENU, ITEM])
        } else {
            (&[ITEM], &[MENU])
        };
        assert_eq!(node(&update, MENU).children(), children);
        assert_eq!(node(&update, ROOT_NODE_ID).children(), roots);
    }
}

#[::std::prelude::v1::test]
fn filtered_and_rolled_back_claims_do_not_conflict_with_the_surviving_item() {
    let (mut a11y, mut focuses) = fixture();
    let menu = focuses.insert(());
    for filter in ["hidden", "modal", "geometry", "claim-geometry", "rollback"] {
        a11y.begin_frame();
        {
            let _scope = a11y.nodes.enter_scope(if filter == "modal" {
                AccessibilityTreeScope::ModalRoot
            } else {
                AccessibilityTreeScope::Unrestricted
            });
            push(&mut a11y, MENU, Role::Menu, Some(menu));
        }
        let checkpoint = a11y.prepaint_checkpoint();
        let source = SubtreeGeometryValidity::new(None);
        {
            let _geometry = a11y.nodes.enter_geometry_validity(
                (filter == "geometry").then(|| SubtreeGeometryValidity::new(Some(source.clone()))),
            );
            push(&mut a11y, GROUP, Role::MenuItem, None);
            if filter == "hidden" {
                a11y.nodes.current_node_mut().unwrap().set_hidden();
            }
            if filter == "claim-geometry" {
                let _claim_geometry = a11y.nodes.enter_geometry_validity(Some(source.clone()));
                a11y.record_active_descendant(GROUP);
            } else {
                a11y.record_active_descendant(GROUP);
            }
            a11y.nodes.pop();
        }
        if filter == "rollback" {
            a11y.rollback_prepaint(checkpoint);
            assert!(a11y.nodes.active_descendant_candidates.is_empty());
        }
        {
            let _scope = a11y.nodes.enter_scope(if filter == "modal" {
                AccessibilityTreeScope::ModalDescendant
            } else {
                AccessibilityTreeScope::Unrestricted
            });
            claim(&mut a11y, ITEM);
        }
        a11y.nodes.pop();
        a11y.resolve_focus(Some(menu), None);
        source.invalidate(crate::geometry::SubtreeGeometryError::DeviceConversion);
        let update = a11y.end_frame();
        assert_eq!(update.focus, ITEM, "{filter}");
        assert_eq!(a11y.nodes.active_descendant, Some(ITEM), "{filter}");
        let children: &[NodeId] = if filter == "claim-geometry" {
            &[GROUP, ITEM]
        } else {
            &[ITEM]
        };
        assert_eq!(node(&update, MENU).children(), children, "{filter}");
        assert_eq!(
            update.nodes.iter().any(|(id, _)| *id == GROUP),
            filter == "claim-geometry"
        );
    }
}

#[::std::prelude::v1::test]
fn final_reparenting_and_source_removal_revoke_the_original_ancestry() {
    let (mut a11y, mut focuses) = fixture();
    let menu = focuses.insert(());
    for removed in ["group", "geometry", "hidden", "modal"] {
        a11y.begin_frame();
        let source = SubtreeGeometryValidity::new(None);
        {
            let _modal = a11y.nodes.enter_scope(if removed == "modal" {
                AccessibilityTreeScope::Unrestricted
            } else {
                AccessibilityTreeScope::ModalRoot
            });
            let parent = {
                let _geometry = a11y
                    .nodes
                    .enter_geometry_validity((removed == "geometry").then(|| source.clone()));
                push(&mut a11y, MENU, Role::Menu, Some(menu));
                if removed == "hidden" {
                    a11y.nodes.current_node_mut().unwrap().set_hidden();
                }
                let _group_geometry = a11y.nodes.enter_geometry_validity(Some(source.clone()));
                push(&mut a11y, GROUP, Role::Group, None);
                let parent = a11y.nodes.reserve_deferred_parent().unwrap();
                a11y.nodes.pop();
                parent
            };
            {
                let _deferred = a11y.nodes.enter_deferred_parent_scope(parent);
                let _modal = a11y.nodes.enter_scope(if removed == "modal" {
                    AccessibilityTreeScope::ModalRoot
                } else {
                    AccessibilityTreeScope::Unrestricted
                });
                claim(&mut a11y, ITEM);
            }
            a11y.nodes.pop();
        }
        a11y.resolve_focus(Some(menu), None);
        assert_eq!(a11y.nodes.keyboard_focus, Some(MENU));
        if matches!(removed, "group" | "geometry") {
            source.invalidate(crate::geometry::SubtreeGeometryError::DeviceConversion);
        }
        let update = a11y.end_frame();
        assert_eq!(a11y.nodes.active_descendant, None, "{removed}");
        let (focus, children): (_, &[NodeId]) = match removed {
            "group" => (MENU, &[MENU, ITEM]),
            "modal" => (ITEM, &[ITEM]),
            "geometry" => (ROOT_NODE_ID, &[ITEM]),
            _ => (ROOT_NODE_ID, &[]),
        };
        assert_eq!(update.focus, focus, "{removed}");
        assert_eq!(
            node(&update, ROOT_NODE_ID).children(),
            children,
            "{removed}"
        );
        assert!(!update.nodes.iter().any(|(id, _)| *id == GROUP));
        assert_eq!(
            update.nodes.iter().any(|(id, _)| *id == MENU),
            removed == "group"
        );
        if removed == "group" {
            assert!(node(&update, MENU).children().is_empty());
        }
    }
}

#[::std::prelude::v1::test]
#[cfg_attr(
    debug_assertions,
    should_panic(expected = "set_active_descendant called on the focused node")
)]
fn self_claim_preserves_the_official_debug_assertion_and_release_fallback() {
    let (mut a11y, mut focuses) = fixture();
    let menu = focuses.insert(());
    a11y.begin_frame();
    push(&mut a11y, MENU, Role::Menu, Some(menu));
    a11y.record_active_descendant(MENU);
    a11y.nodes.pop();
    a11y.resolve_focus(Some(menu), None);
    assert_eq!(a11y.end_frame().focus, MENU);
    assert_eq!(a11y.nodes.active_descendant, None);
}

#[::std::prelude::v1::test]
#[cfg_attr(
    debug_assertions,
    should_panic(expected = "active descendant claimed by multiple nodes in one frame")
)]
fn sibling_claims_preserve_the_official_debug_assertion_and_release_last_wins() {
    let (mut a11y, mut focuses) = fixture();
    let menu = focuses.insert(());
    a11y.begin_frame();
    push(&mut a11y, MENU, Role::Menu, Some(menu));
    claim(&mut a11y, GROUP);
    claim(&mut a11y, ITEM);
    a11y.nodes.pop();
    a11y.resolve_focus(Some(menu), None);
    assert_eq!(a11y.end_frame().focus, ITEM);
    assert_eq!(a11y.nodes.active_descendant, Some(ITEM));
}

#[::std::prelude::v1::test]
fn missing_focus_dedup_and_recovery_change_only_when_a_frame_publishes() {
    let recorders: [(fn(&mut A11y, FocusId), &str); 2] = [
        (A11y::record_focus_without_id, "it has no element id"),
        (A11y::record_focus_without_role, "it has an id but no role"),
    ];
    for (record, reason) in recorders {
        let (mut a11y, mut focuses) = fixture();
        let missing = focuses.insert(());
        let other = focuses.insert(());
        for frame in 1..=2 {
            a11y.begin_frame();
            record(&mut a11y, missing);
            assert_eq!(a11y.candidate_missing_focus[0].1, reason);
            a11y.resolve_focus(Some(missing), None);
            let update = a11y.end_frame();
            assert_eq!(update.focus, ROOT_NODE_ID);
            assert_eq!(
                a11y.last_focus_without_node,
                (frame == 2).then_some(missing)
            );
            publish(&mut a11y, &update, frame);
            assert_eq!(a11y.last_focus_without_node, Some(missing));
        }
        let old_snapshot = a11y.debug_tree_json().unwrap();
        let old_revision = a11y.published_revision_for_test();
        for valid in [false, true] {
            a11y.begin_frame();
            if valid {
                push(&mut a11y, MENU, Role::Menu, Some(missing));
                a11y.nodes.pop();
            } else {
                record(&mut a11y, other);
            }
            a11y.resolve_focus(Some(if valid { missing } else { other }), None);
            let _discarded = a11y.end_frame();
            assert_eq!(a11y.last_focus_without_node, Some(missing));
            a11y.discard_candidate_frame();
            assert!(a11y.candidate_missing_focus.is_empty());
            assert!(a11y.candidate_focus_ids.is_empty());
            assert_eq!(a11y.candidate_keyboard_focus, None);
            assert_eq!(a11y.last_focus_without_node, Some(missing));
            assert_eq!(
                a11y.debug_tree_json().as_deref(),
                Some(old_snapshot.as_str())
            );
            assert_eq!(a11y.published_revision_for_test(), old_revision);
        }
        a11y.begin_frame();
        push(&mut a11y, OUTSIDE, Role::Button, Some(other));
        a11y.nodes.pop();
        record(&mut a11y, missing);
        a11y.resolve_focus(Some(missing), Some(other));
        let update = a11y.end_frame();
        assert_eq!(update.focus, OUTSIDE);
        publish(&mut a11y, &update, 3);
        assert_eq!(a11y.last_focus_without_node, Some(missing));
        a11y.begin_frame();
        push(&mut a11y, MENU, Role::Menu, Some(missing));
        a11y.nodes.pop();
        a11y.resolve_focus(Some(missing), None);
        let update = a11y.end_frame();
        assert_eq!(a11y.last_focus_without_node, Some(missing));
        publish(&mut a11y, &update, 4);
        assert_eq!(a11y.last_focus_without_node, None);
        assert_eq!(a11y.published_focus_id(MENU), Some(missing));
        a11y.begin_frame();
        record(&mut a11y, missing);
        a11y.resolve_focus(Some(missing), None);
        let update = a11y.end_frame();
        publish(&mut a11y, &update, 5);
        assert_eq!(a11y.last_focus_without_node, Some(missing));
        a11y.begin_frame();
        record(&mut a11y, other);
        a11y.resolve_focus(Some(other), None);
        let update = a11y.end_frame();
        assert_eq!(a11y.last_focus_without_node, Some(missing));
        publish(&mut a11y, &update, 6);
        assert_eq!(a11y.last_focus_without_node, Some(other));
    }
}

#[::std::prelude::v1::test]
fn only_current_valid_missing_focus_metadata_is_admitted() {
    let (mut a11y, mut focuses) = fixture();
    let missing = focuses.insert(());
    let unrelated = focuses.insert(());
    let baseline = focuses.insert(());
    a11y.begin_frame();
    a11y.record_focus_without_role(baseline);
    a11y.resolve_focus(Some(baseline), None);
    let update = a11y.end_frame();
    publish(&mut a11y, &update, 0);
    for rejected in [
        "geometry",
        "rollback",
        "unrelated",
        "no-focus",
        "accessibility-only",
    ] {
        let previous = a11y.debug_tree_json().unwrap();
        a11y.begin_frame();
        a11y.record_focus_without_role(baseline);
        let checkpoint = a11y.prepaint_checkpoint();
        let source = SubtreeGeometryValidity::new(None);
        {
            let _geometry = a11y.nodes.enter_geometry_validity(
                (rejected == "geometry")
                    .then(|| SubtreeGeometryValidity::new(Some(source.clone()))),
            );
            a11y.record_focus_without_id(missing);
            a11y.record_focus_without_role(missing);
        }
        if rejected == "rollback" {
            a11y.rollback_prepaint(checkpoint);
            assert_eq!(a11y.candidate_missing_focus.len(), 1);
            assert_eq!(a11y.candidate_missing_focus[0].0, baseline);
        }
        if rejected == "accessibility-only" {
            push(&mut a11y, OUTSIDE, Role::Button, Some(unrelated));
            a11y.nodes.pop();
        }
        let keyboard = match rejected {
            "no-focus" | "accessibility-only" => None,
            "unrelated" => Some(unrelated),
            _ => Some(missing),
        };
        a11y.resolve_focus(
            keyboard,
            (rejected == "accessibility-only").then_some(unrelated),
        );
        source.invalidate(crate::geometry::SubtreeGeometryError::DeviceConversion);
        let update = a11y.end_frame();
        assert_eq!(a11y.last_focus_without_node, Some(baseline), "{rejected}");
        assert_eq!(a11y.debug_tree_json().as_deref(), Some(previous.as_str()));
        publish(&mut a11y, &update, 1);
        assert_eq!(a11y.last_focus_without_node, Some(baseline), "{rejected}");
    }
}

#[::std::prelude::v1::test]
fn inactive_or_force_disabled_frames_cannot_admit_missing_focus() {
    for force_disabled in [false, true] {
        let state = Arc::new(AtomicU64::new(0));
        set_requested_active(&state, force_disabled);
        let mut a11y = A11y::new(state, force_disabled, WindowId::from(19));
        let mut focuses = SlotMap::<FocusId, ()>::with_key();
        let missing = focuses.insert(());
        a11y.sync_active_flag();
        assert!(!a11y.is_active());
        a11y.begin_frame();
        a11y.record_focus_without_id(missing);
        a11y.record_focus_without_role(missing);
        a11y.resolve_focus(Some(missing), None);
        assert_eq!(a11y.candidate_keyboard_focus, None);
        assert_eq!(a11y.end_frame().focus, ROOT_NODE_ID);
        a11y.discard_candidate_frame();
        assert_eq!(a11y.last_focus_without_node, None);
        assert_eq!(a11y.published_revision_for_test(), None);
        assert!(a11y.debug_tree_json().is_none());
    }
}

#[::std::prelude::v1::test]
fn irreversible_close_and_quiescence_do_not_publish_late_missing_focus_diagnostics() {
    for closed in [false, true] {
        let (mut a11y, mut focuses) = fixture();
        let missing = focuses.insert(());
        a11y.begin_frame();
        a11y.record_focus_without_id(missing);
        a11y.resolve_focus(Some(missing), None);
        let update = a11y.end_frame();
        if closed {
            a11y.clear_announcements_for_window_close();
        } else {
            a11y.clear_announcements_for_interaction_quiescence();
        }
        publish(&mut a11y, &update, 1);
        assert_eq!(a11y.last_focus_without_node, None);
        assert!(a11y.debug_tree_json().is_none());
    }
}
