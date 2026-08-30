use crate::canonical_runtime::CanonicalGraphRuntime;
use crate::graph_assembly::{CanonicalNode, DockGraphAssembly};
use crate::graph_patch::{
    GraphCommitOutcome, GraphPatchOperation, GraphPrepareError, GraphPrepareOutcome,
    GraphSupersededReason,
};
use crate::{DockGroupId, DockItemId, DockLayoutRect, DockSpaceId, SplitAxis};

fn item(id: &str) -> DockItemId {
    DockItemId::new(id)
}

fn runtime() -> CanonicalGraphRuntime {
    let mut assembly = DockGraphAssembly::new();
    assembly
        .add_space("main")
        .expect("fixture space should be valid");
    let editor = assembly
        .add_group("editor", [item("A"), item("B")], item("A"))
        .expect("fixture group should be valid");
    let inspector = assembly
        .add_group("inspector", [item("C"), item("D")], item("C"))
        .expect("fixture group should be valid");
    let root = assembly
        .add_branch(SplitAxis::Horizontal, editor, inspector, 0.5)
        .expect("fixture branch should be valid");
    assembly
        .set_root("main", root)
        .expect("fixture root should be valid");
    CanonicalGraphRuntime::from_assembly(assembly).expect("fixture publication should succeed")
}

fn three_tab_runtime() -> CanonicalGraphRuntime {
    let mut assembly = DockGraphAssembly::new();
    assembly.add_space("main").unwrap();
    let editor = assembly
        .add_group("editor", [item("A"), item("B"), item("C")], item("A"))
        .unwrap();
    assembly.set_root("main", editor).unwrap();
    CanonicalGraphRuntime::from_assembly(assembly).unwrap()
}

fn multiview_runtime() -> CanonicalGraphRuntime {
    let mut assembly = DockGraphAssembly::new();
    assembly.add_space("main").unwrap();
    assembly.add_space("detached").unwrap();
    let editor = assembly
        .add_group("editor", [item("A"), item("B")], item("A"))
        .unwrap();
    let inspector = assembly
        .add_group("inspector", [item("C")], item("C"))
        .unwrap();
    let detached_target = assembly
        .add_group("detached-target", [item("X")], item("X"))
        .unwrap();
    let main_root = assembly
        .add_branch(SplitAxis::Horizontal, editor, inspector, 0.5)
        .unwrap();
    assembly.set_root("main", main_root).unwrap();
    assembly.set_root("detached", detached_target).unwrap();
    CanonicalGraphRuntime::from_assembly(assembly).unwrap()
}

fn empty_target_runtime() -> CanonicalGraphRuntime {
    let mut assembly = DockGraphAssembly::new();
    assembly.add_space("main").unwrap();
    assembly.add_space("empty").unwrap();
    let editor = assembly
        .add_group("editor", [item("A"), item("B")], item("A"))
        .unwrap();
    let inspector = assembly
        .add_group("inspector", [item("C")], item("C"))
        .unwrap();
    let main_root = assembly
        .add_branch(SplitAxis::Horizontal, editor, inspector, 0.5)
        .unwrap();
    assembly.set_root("main", main_root).unwrap();
    CanonicalGraphRuntime::from_assembly(assembly).unwrap()
}

fn floating_only_runtime(include_other_floating: bool) -> CanonicalGraphRuntime {
    let mut assembly = DockGraphAssembly::new();
    assembly.add_space("main").unwrap();
    let source = assembly
        .add_group("source", [item("S")], item("S"))
        .unwrap();
    assembly
        .add_floating_root(
            "floating-source",
            "main",
            source,
            crate::DockLayoutRect {
                x: 10.0,
                y: 10.0,
                width: 320.0,
                height: 240.0,
            },
            1,
        )
        .unwrap();
    if include_other_floating {
        let other = assembly.add_group("other", [item("O")], item("O")).unwrap();
        assembly
            .add_floating_root(
                "floating-other",
                "main",
                other,
                crate::DockLayoutRect {
                    x: 40.0,
                    y: 40.0,
                    width: 320.0,
                    height: 240.0,
                },
                2,
            )
            .unwrap();
    }
    CanonicalGraphRuntime::from_assembly(assembly).unwrap()
}

fn floating_runtime() -> CanonicalGraphRuntime {
    let mut assembly = DockGraphAssembly::new();
    assembly.add_space("main").unwrap();
    let target = assembly
        .add_group("target", [item("T")], item("T"))
        .unwrap();
    let source = assembly
        .add_group("source", [item("S")], item("S"))
        .unwrap();
    assembly.set_root("main", source).unwrap();
    assembly
        .add_floating_root(
            "floating-target",
            "main",
            target,
            crate::DockLayoutRect {
                x: 10.0,
                y: 10.0,
                width: 320.0,
                height: 240.0,
            },
            1,
        )
        .unwrap();
    CanonicalGraphRuntime::from_assembly(assembly).unwrap()
}

fn floating_source_runtime() -> CanonicalGraphRuntime {
    let mut assembly = DockGraphAssembly::new();
    assembly.add_space("main").unwrap();
    let target = assembly
        .add_group("target", [item("T")], item("T"))
        .unwrap();
    let source = assembly
        .add_group("source", [item("S")], item("S"))
        .unwrap();
    assembly.set_root("main", target).unwrap();
    assembly
        .add_floating_root(
            "floating-source",
            "main",
            source,
            crate::DockLayoutRect {
                x: 10.0,
                y: 10.0,
                width: 320.0,
                height: 240.0,
            },
            1,
        )
        .unwrap();
    CanonicalGraphRuntime::from_assembly(assembly).unwrap()
}

fn floating_subtree_runtime() -> CanonicalGraphRuntime {
    let mut assembly = DockGraphAssembly::new();
    assembly.add_space("main").unwrap();
    let target = assembly
        .add_group("target", [item("T")], item("T"))
        .unwrap();
    let source_left = assembly
        .add_group("source-left", [item("L")], item("L"))
        .unwrap();
    let source_right = assembly
        .add_group("source-right", [item("R")], item("R"))
        .unwrap();
    let floating_tree = assembly
        .add_branch(SplitAxis::Vertical, source_left, source_right, 0.6)
        .unwrap();
    assembly.set_root("main", target).unwrap();
    assembly
        .add_floating_root(
            "floating-subtree",
            "main",
            floating_tree,
            crate::DockLayoutRect {
                x: 20.0,
                y: 20.0,
                width: 480.0,
                height: 320.0,
            },
            1,
        )
        .unwrap();
    CanonicalGraphRuntime::from_assembly(assembly).unwrap()
}

fn floating_bounds() -> DockLayoutRect {
    DockLayoutRect {
        x: -120.0,
        y: 40.0,
        width: 360.0,
        height: 240.0,
    }
}

#[test]
fn real_runtime_commits_selection_and_mru_without_a_graph_copy() {
    let mut runtime = runtime();
    let prepared = match runtime
        .prepare_select_tab("editor", "B")
        .expect("selection preparation should succeed")
    {
        GraphPrepareOutcome::Prepared(prepared) => prepared,
        GraphPrepareOutcome::NoOp(_) => panic!("selection should create a runtime patch"),
    };

    let outcome = runtime.commit(prepared);
    assert!(matches!(outcome, GraphCommitOutcome::Committed(_)));
    let group = runtime
        .graph()
        .group(&"editor".into())
        .expect("editor group should remain present");
    assert_eq!(group.selected, item("B"));
    assert_eq!(group.mru, vec![item("B")]);
    assert_eq!(runtime.graph_revision().selection(), 1);
    assert_eq!(runtime.receipt_count(), 1);
}

#[test]
fn current_tab_selection_is_a_real_mru_commit() {
    let mut runtime = runtime();
    let prepared = match runtime
        .prepare_select_tab("editor", "A")
        .expect("selection preparation should succeed")
    {
        GraphPrepareOutcome::Prepared(prepared) => prepared,
        GraphPrepareOutcome::NoOp(_) => panic!("current selection still records MRU usage"),
    };
    assert!(matches!(
        runtime.commit(prepared),
        GraphCommitOutcome::Committed(_)
    ));
    let group = runtime
        .graph()
        .group(&"editor".into())
        .expect("editor group should remain present");
    assert_eq!(group.selected, item("A"));
    assert_eq!(group.mru, vec![item("A")]);
}

#[test]
fn unrelated_group_selection_does_not_stale_a_local_runtime_patch() {
    let mut runtime = runtime();
    let prepared = match runtime
        .prepare_select_tab("editor", "B")
        .expect("selection preparation should succeed")
    {
        GraphPrepareOutcome::Prepared(prepared) => prepared,
        GraphPrepareOutcome::NoOp(_) => panic!("selection should create a runtime patch"),
    };
    let unrelated = match runtime
        .prepare_select_tab("inspector", "D")
        .expect("selection preparation should succeed")
    {
        GraphPrepareOutcome::Prepared(prepared) => prepared,
        GraphPrepareOutcome::NoOp(_) => panic!("selection should create a runtime patch"),
    };
    assert!(matches!(
        runtime.commit(unrelated),
        GraphCommitOutcome::Committed(_)
    ));
    assert!(matches!(
        runtime.commit(prepared),
        GraphCommitOutcome::Committed(_)
    ));
}

#[test]
fn touched_group_change_is_rejected_before_a_second_runtime_write() {
    let mut runtime = runtime();
    let first = match runtime
        .prepare_select_tab("editor", "B")
        .expect("selection preparation should succeed")
    {
        GraphPrepareOutcome::Prepared(prepared) => prepared,
        GraphPrepareOutcome::NoOp(_) => panic!("selection should create a runtime patch"),
    };
    let second = match runtime
        .prepare_select_tab("editor", "B")
        .expect("selection preparation should succeed")
    {
        GraphPrepareOutcome::Prepared(prepared) => prepared,
        GraphPrepareOutcome::NoOp(_) => panic!("selection should create a runtime patch"),
    };
    assert!(matches!(
        runtime.commit(second),
        GraphCommitOutcome::Committed(_)
    ));
    let stale = runtime.commit(first);
    assert!(matches!(
        stale,
        GraphCommitOutcome::Superseded(receipt)
            if receipt.reason() == &GraphSupersededReason::GroupSelectionChanged
    ));
    assert_eq!(runtime.receipt_count(), 1);
}

#[test]
fn nested_edge_split_rewrites_only_the_target_parent_path() {
    let mut runtime = runtime();
    let space = DockSpaceId::from("main");
    let target_group = DockGroupId::from("editor");
    let inspector_group = DockGroupId::from("inspector");
    let root_before = runtime
        .graph()
        .space(&space)
        .and_then(|space| space.root)
        .expect("fixture root should exist");
    let target_before = runtime
        .graph()
        .index()
        .node_for_group(&target_group)
        .expect("target group should have a leaf");
    let inspector_before = runtime
        .graph()
        .index()
        .node_for_group(&inspector_group)
        .expect("inspector group should have a leaf");
    let node_count_before = runtime.graph().nodes().len();

    let prepared = match runtime
        .prepare_split_group(
            "main",
            "editor",
            "explorer",
            [item("E")],
            "E",
            SplitAxis::Vertical,
            true,
            0.25,
        )
        .expect("edge split should prepare")
    {
        GraphPrepareOutcome::Prepared(prepared) => prepared,
        GraphPrepareOutcome::NoOp(_) => panic!("edge split must produce a structural patch"),
    };
    assert!(matches!(
        prepared.operation(),
        GraphPatchOperation::SplitGroup { target_group, new_group, .. }
            if target_group == &DockGroupId::from("editor")
                && new_group == &DockGroupId::from("explorer")
    ));

    let receipt = match runtime.commit(prepared) {
        GraphCommitOutcome::Committed(receipt) => receipt,
        other => panic!("edge split should commit, got {other:?}"),
    };
    assert_eq!(receipt.after().topology(), receipt.before().topology() + 1);
    assert_eq!(receipt.after().selection(), receipt.before().selection());
    runtime
        .graph()
        .validate()
        .expect("committed graph should remain canonical");
    assert_eq!(
        runtime.graph().space(&space).and_then(|space| space.root),
        Some(root_before)
    );
    assert_eq!(
        runtime.graph().index().node_for_group(&target_group),
        Some(target_before)
    );
    assert_eq!(
        runtime.graph().index().node_for_group(&inspector_group),
        Some(inspector_before)
    );
    assert_eq!(runtime.graph().nodes().len(), node_count_before + 2);

    let inserted_parent = runtime
        .graph()
        .index()
        .parent_of(target_before)
        .flatten()
        .expect("target should gain a local branch parent");
    let explorer = runtime
        .graph()
        .index()
        .node_for_group(&DockGroupId::from("explorer"))
        .expect("new group should have a leaf");
    assert!(matches!(
        runtime.graph().node(inserted_parent),
        Some(CanonicalNode::Branch {
            axis: SplitAxis::Vertical,
            first,
            second,
            ratio,
            ..
        }) if *first == explorer && *second == target_before && *ratio == 0.25
    ));
}

#[test]
fn target_selection_change_does_not_supersede_structural_split() {
    let mut runtime = runtime();
    let split = match runtime
        .prepare_split_group(
            "main",
            "editor",
            "explorer",
            [item("E")],
            "E",
            SplitAxis::Vertical,
            false,
            0.4,
        )
        .expect("edge split should prepare")
    {
        GraphPrepareOutcome::Prepared(prepared) => prepared,
        GraphPrepareOutcome::NoOp(_) => panic!("edge split must produce a structural patch"),
    };
    let selection = match runtime
        .prepare_select_tab("editor", "B")
        .expect("selection should prepare")
    {
        GraphPrepareOutcome::Prepared(prepared) => prepared,
        GraphPrepareOutcome::NoOp(_) => panic!("selection should produce a patch"),
    };
    assert!(matches!(
        runtime.commit(selection),
        GraphCommitOutcome::Committed(_)
    ));
    assert!(matches!(
        runtime.commit(split),
        GraphCommitOutcome::Committed(_)
    ));
}

#[test]
fn split_preparation_rejects_duplicate_or_already_owned_items() {
    let mut runtime = runtime();
    let duplicate = runtime.prepare_split_group(
        "main",
        "editor",
        "explorer",
        [item("E"), item("E")],
        "E",
        SplitAxis::Vertical,
        true,
        0.5,
    );
    assert!(matches!(
        duplicate,
        Err(GraphPrepareError::DuplicateNewGroupItem { .. })
    ));

    let owned = runtime.prepare_split_group(
        "main",
        "editor",
        "explorer",
        [item("C")],
        "C",
        SplitAxis::Vertical,
        true,
        0.5,
    );
    assert!(matches!(
        owned,
        Err(GraphPrepareError::ItemAlreadyExists { .. })
    ));
    assert_eq!(runtime.graph().nodes().len(), 3);
    assert_eq!(runtime.graph_revision().topology(), 1);
}

#[test]
fn move_group_to_edge_preserves_group_identity_across_dockspaces() {
    let mut runtime = multiview_runtime();
    let editor = DockGroupId::from("editor");
    let inspector = DockGroupId::from("inspector");
    let detached_target = DockGroupId::from("detached-target");
    let original_main_root = runtime
        .graph()
        .space(&DockSpaceId::from("main"))
        .and_then(|space| space.root)
        .unwrap();
    let editor_node = runtime.graph().index().node_for_group(&editor).unwrap();
    let inspector_node = runtime.graph().index().node_for_group(&inspector).unwrap();
    let detached_target_node = runtime
        .graph()
        .index()
        .node_for_group(&detached_target)
        .unwrap();

    let prepared = match runtime
        .prepare_move_group_to_edge(
            editor.clone(),
            detached_target.clone(),
            "main",
            "detached",
            SplitAxis::Vertical,
            true,
            0.25,
        )
        .expect("cross-space group move should prepare")
    {
        GraphPrepareOutcome::Prepared(prepared) => prepared,
        GraphPrepareOutcome::NoOp(_) => panic!("group move must produce a structural patch"),
    };
    let receipt = match runtime.commit(prepared) {
        GraphCommitOutcome::Committed(receipt) => receipt,
        other => panic!("group move should commit, got {other:?}"),
    };

    assert_eq!(receipt.after().topology(), receipt.before().topology() + 1);
    assert_eq!(
        runtime
            .graph()
            .space(&DockSpaceId::from("main"))
            .unwrap()
            .root,
        Some(original_main_root)
    );
    assert_eq!(
        runtime.graph().index().node_for_group(&inspector),
        Some(original_main_root)
    );
    assert!(runtime.graph().node(inspector_node).is_none());
    let detached_root = runtime
        .graph()
        .space(&DockSpaceId::from("detached"))
        .and_then(|space| space.root)
        .unwrap();
    assert!(matches!(
        runtime.graph().node(detached_root),
        Some(CanonicalNode::Branch {
            axis: SplitAxis::Vertical,
            first,
            second,
            ratio,
            ..
        }) if *first == editor_node && *second == detached_target_node && *ratio == 0.25
    ));
    assert_eq!(
        runtime.graph().index().node_for_group(&editor),
        Some(editor_node)
    );
    assert_eq!(
        runtime.graph().index().node_for_group(&detached_target),
        Some(detached_target_node)
    );
    runtime
        .graph()
        .validate()
        .expect("cross-space group move should preserve canonical indexes");
}

#[test]
fn move_group_to_edge_promotes_the_sibling_into_the_existing_parent_identity() {
    let mut runtime = runtime();
    let editor = DockGroupId::from("editor");
    let inspector = DockGroupId::from("inspector");
    let original_root = runtime
        .graph()
        .space(&DockSpaceId::from("main"))
        .and_then(|space| space.root)
        .unwrap();
    let editor_node = runtime.graph().index().node_for_group(&editor).unwrap();
    let inspector_node = runtime.graph().index().node_for_group(&inspector).unwrap();

    let prepared = match runtime
        .prepare_move_group_to_edge(
            editor.clone(),
            inspector.clone(),
            "main",
            "main",
            SplitAxis::Vertical,
            true,
            0.35,
        )
        .expect("same-space group move should prepare")
    {
        GraphPrepareOutcome::Prepared(prepared) => prepared,
        GraphPrepareOutcome::NoOp(_) => panic!("group move must produce a structural patch"),
    };
    let receipt = match runtime.commit(prepared) {
        GraphCommitOutcome::Committed(receipt) => receipt,
        other => panic!("same-space group move should commit, got {other:?}"),
    };

    assert_eq!(receipt.after().topology(), receipt.before().topology() + 1);
    let root = runtime
        .graph()
        .space(&DockSpaceId::from("main"))
        .and_then(|space| space.root)
        .unwrap();
    assert!(matches!(
        runtime.graph().node(root),
        Some(CanonicalNode::Branch {
            axis: SplitAxis::Vertical,
            first,
            second,
            ratio,
            ..
        }) if *first == editor_node && *second == original_root && *ratio == 0.35
    ));
    assert_eq!(
        runtime.graph().index().node_for_group(&editor),
        Some(editor_node)
    );
    assert_eq!(
        runtime.graph().index().node_for_group(&inspector),
        Some(original_root)
    );
    assert!(runtime.graph().node(inspector_node).is_none());
    runtime
        .graph()
        .validate()
        .expect("same-space group move should preserve canonical indexes");
}

#[test]
fn move_group_to_center_merges_tabs_and_preserves_the_source_group_identity() {
    let mut runtime = runtime();
    let editor = DockGroupId::from("editor");
    let inspector = DockGroupId::from("inspector");
    let original_root = runtime
        .graph()
        .space(&DockSpaceId::from("main"))
        .and_then(|space| space.root)
        .unwrap();
    let editor_node = runtime.graph().index().node_for_group(&editor).unwrap();
    let inspector_node = runtime.graph().index().node_for_group(&inspector).unwrap();

    let prepared = match runtime
        .prepare_move_group_to_center(editor.clone(), inspector.clone(), "main", "main")
        .expect("same-space group merge should prepare")
    {
        GraphPrepareOutcome::Prepared(prepared) => prepared,
        GraphPrepareOutcome::NoOp(_) => panic!("different groups must produce a merge patch"),
    };
    assert!(matches!(
        prepared.operation(),
        GraphPatchOperation::MoveGroupToCenter {
            source_group,
            target_group,
            ..
        } if source_group == &editor && target_group == &inspector
    ));
    let receipt = match runtime.commit(prepared) {
        GraphCommitOutcome::Committed(receipt) => receipt,
        other => panic!("same-space group merge should commit, got {other:?}"),
    };

    assert_eq!(receipt.after().topology(), receipt.before().topology() + 1);
    assert!(runtime.graph().group(&inspector).is_none());
    let moved = runtime
        .graph()
        .group(&editor)
        .expect("whole-group payload identity should remain present");
    assert_eq!(
        moved.items,
        vec![item("C"), item("D"), item("A"), item("B")]
    );
    assert_eq!(moved.selected, item("A"));
    assert_eq!(moved.mru, vec![item("A"), item("C")]);
    assert_eq!(
        runtime
            .graph()
            .space(&DockSpaceId::from("main"))
            .and_then(|space| space.root),
        Some(original_root)
    );
    assert_eq!(
        runtime.graph().index().node_for_group(&editor),
        Some(original_root)
    );
    assert!(runtime.graph().node(editor_node).is_none());
    assert!(runtime.graph().node(inspector_node).is_none());
    assert_eq!(runtime.graph().nodes().len(), 1);
    assert_eq!(
        runtime.graph().index().group_for_item(&item("A")),
        Some(&editor)
    );
    assert_eq!(
        runtime.graph().index().group_for_item(&item("C")),
        Some(&editor)
    );
    runtime
        .graph()
        .validate()
        .expect("same-space merge should preserve canonical indexes");
}

#[test]
fn move_group_to_center_preserves_the_remaining_source_space_across_dockspaces() {
    let mut runtime = multiview_runtime();
    let editor = DockGroupId::from("editor");
    let inspector = DockGroupId::from("inspector");
    let detached_target = DockGroupId::from("detached-target");
    let original_main_root = runtime
        .graph()
        .space(&DockSpaceId::from("main"))
        .and_then(|space| space.root)
        .unwrap();
    let inspector_node = runtime.graph().index().node_for_group(&inspector).unwrap();
    let detached_target_node = runtime
        .graph()
        .index()
        .node_for_group(&detached_target)
        .unwrap();

    let prepared = match runtime
        .prepare_move_group_to_center(editor.clone(), detached_target.clone(), "main", "detached")
        .expect("cross-space group merge should prepare")
    {
        GraphPrepareOutcome::Prepared(prepared) => prepared,
        GraphPrepareOutcome::NoOp(_) => panic!("cross-space merge must be structural"),
    };
    assert!(matches!(
        runtime.commit(prepared),
        GraphCommitOutcome::Committed(_)
    ));

    assert_eq!(
        runtime
            .graph()
            .space(&DockSpaceId::from("main"))
            .and_then(|space| space.root),
        Some(original_main_root)
    );
    assert_eq!(
        runtime.graph().index().node_for_group(&inspector),
        Some(original_main_root)
    );
    assert!(runtime.graph().node(inspector_node).is_none());
    assert_eq!(
        runtime
            .graph()
            .space(&DockSpaceId::from("detached"))
            .and_then(|space| space.root),
        Some(detached_target_node)
    );
    assert!(runtime.graph().group(&detached_target).is_none());
    assert_eq!(
        runtime.graph().group(&editor).unwrap().items,
        vec![item("X"), item("A"), item("B")]
    );
    assert_eq!(
        runtime.graph().index().node_for_group(&editor),
        Some(detached_target_node)
    );
    runtime
        .graph()
        .validate()
        .expect("cross-space merge should preserve canonical indexes");
}

#[test]
fn move_group_to_center_rehomes_item_ownership_into_a_floating_target() {
    let mut runtime = floating_runtime();
    let source = DockGroupId::from("source");
    let target = DockGroupId::from("target");
    let floating = crate::DockFloatingId::from("floating-target");

    let prepared = match runtime
        .prepare_move_group_to_center(source.clone(), target.clone(), "main", "main")
        .expect("floating-target group merge should prepare")
    {
        GraphPrepareOutcome::Prepared(prepared) => prepared,
        GraphPrepareOutcome::NoOp(_) => panic!("floating-target merge must be structural"),
    };
    assert!(matches!(
        runtime.commit(prepared),
        GraphCommitOutcome::Committed(_)
    ));

    assert!(runtime.graph().group(&target).is_none());
    assert_eq!(
        runtime.graph().group(&source).unwrap().items,
        vec![item("T"), item("S")]
    );
    assert_eq!(
        runtime.graph().index().floating_for_item(&item("S")),
        Some(&floating)
    );
    runtime
        .graph()
        .validate()
        .expect("floating-target merge should preserve canonical indexes");
}

#[test]
fn move_group_to_center_retires_a_single_group_floating_source() {
    let mut runtime = floating_source_runtime();
    let source = DockGroupId::from("source");
    let target = DockGroupId::from("target");
    let floating = crate::DockFloatingId::from("floating-source");

    let prepared = match runtime
        .prepare_move_group_to_center(source.clone(), target.clone(), "main", "main")
        .expect("floating-source group merge should prepare")
    {
        GraphPrepareOutcome::Prepared(prepared) => prepared,
        GraphPrepareOutcome::NoOp(_) => panic!("floating-source merge must be structural"),
    };
    assert!(matches!(
        runtime.commit(prepared),
        GraphCommitOutcome::Committed(_)
    ));

    assert!(runtime.graph().floating_roots().get(&floating).is_none());
    assert!(runtime.graph().group(&target).is_none());
    assert_eq!(
        runtime.graph().group(&source).unwrap().items,
        vec![item("T"), item("S")]
    );
    assert!(
        runtime
            .graph()
            .index()
            .floating_for_item(&item("S"))
            .is_none()
    );
    runtime
        .graph()
        .validate()
        .expect("floating-source merge should preserve canonical indexes");
}

#[test]
fn group_merge_is_noop_for_the_same_group_and_stale_after_source_selection_changes() {
    let mut runtime = runtime();
    let no_op = runtime
        .prepare_move_group_to_center("editor", "editor", "main", "main")
        .expect("same-group center move should resolve");
    assert!(matches!(
        no_op,
        GraphPrepareOutcome::NoOp(receipt)
            if matches!(receipt.operation(), GraphPatchOperation::MoveGroupToCenter { .. })
    ));

    let merge = match runtime
        .prepare_move_group_to_center("editor", "inspector", "main", "main")
        .expect("group merge should prepare")
    {
        GraphPrepareOutcome::Prepared(prepared) => prepared,
        GraphPrepareOutcome::NoOp(_) => panic!("different groups must produce a merge patch"),
    };
    let selection = match runtime
        .prepare_select_tab("editor", "B")
        .expect("source selection should prepare")
    {
        GraphPrepareOutcome::Prepared(prepared) => prepared,
        GraphPrepareOutcome::NoOp(_) => panic!("selection should produce a patch"),
    };
    assert!(matches!(
        runtime.commit(selection),
        GraphCommitOutcome::Committed(_)
    ));
    assert!(matches!(
        runtime.commit(merge),
        GraphCommitOutcome::Superseded(receipt)
            if receipt.reason() == &GraphSupersededReason::GroupSelectionChanged
    ));
}

#[test]
fn move_group_to_empty_space_preserves_leaf_identity_and_binds_the_central_region() {
    let mut runtime = empty_target_runtime();
    let editor = DockGroupId::from("editor");
    let inspector = DockGroupId::from("inspector");
    let original_main_root = runtime
        .graph()
        .space(&DockSpaceId::from("main"))
        .and_then(|space| space.root)
        .unwrap();
    let editor_node = runtime.graph().index().node_for_group(&editor).unwrap();
    let inspector_node = runtime.graph().index().node_for_group(&inspector).unwrap();

    let prepared = match runtime
        .prepare_move_group_to_empty_space(editor.clone(), "main", "empty")
        .expect("cross-space empty-root move should prepare")
    {
        GraphPrepareOutcome::Prepared(prepared) => prepared,
        GraphPrepareOutcome::NoOp(_) => panic!("empty-root move must be structural"),
    };
    assert!(matches!(
        prepared.operation(),
        GraphPatchOperation::MoveGroupToEmptySpace { source_group, .. }
            if source_group == &editor
    ));
    assert!(matches!(
        runtime.commit(prepared),
        GraphCommitOutcome::Committed(_)
    ));

    assert_eq!(
        runtime
            .graph()
            .space(&DockSpaceId::from("main"))
            .and_then(|space| space.root),
        Some(original_main_root)
    );
    assert_eq!(
        runtime.graph().index().node_for_group(&inspector),
        Some(original_main_root)
    );
    assert!(runtime.graph().node(inspector_node).is_none());
    let target = runtime
        .graph()
        .space(&DockSpaceId::from("empty"))
        .expect("target space should remain present");
    assert_eq!(target.root, Some(editor_node));
    assert_eq!(target.central.node, Some(editor_node));
    assert_eq!(
        runtime.graph().index().node_for_group(&editor),
        Some(editor_node)
    );
    assert_eq!(
        runtime.graph().index().space_for_node(editor_node),
        Some(&DockSpaceId::from("empty"))
    );
    runtime
        .graph()
        .validate()
        .expect("empty-root move should preserve canonical indexes");
}

#[test]
fn move_group_to_empty_space_can_promote_the_only_floating_group_to_main() {
    let mut runtime = floating_only_runtime(false);
    let source = DockGroupId::from("source");
    let source_node = runtime.graph().index().node_for_group(&source).unwrap();

    let prepared = match runtime
        .prepare_move_group_to_empty_space(source.clone(), "main", "main")
        .expect("same-space floating promotion should prepare")
    {
        GraphPrepareOutcome::Prepared(prepared) => prepared,
        GraphPrepareOutcome::NoOp(_) => panic!("floating promotion must be structural"),
    };
    assert!(matches!(
        runtime.commit(prepared),
        GraphCommitOutcome::Committed(_)
    ));

    let space = runtime
        .graph()
        .space(&DockSpaceId::from("main"))
        .expect("main space should remain present");
    assert_eq!(space.root, Some(source_node));
    assert_eq!(space.central.node, Some(source_node));
    assert!(space.floating_roots.is_empty());
    assert!(runtime.graph().floating_roots().is_empty());
    assert!(
        runtime
            .graph()
            .index()
            .floating_for_item(&item("S"))
            .is_none()
    );
    runtime
        .graph()
        .validate()
        .expect("floating promotion should preserve canonical indexes");
}

#[test]
fn move_group_to_empty_space_rejects_non_empty_or_missing_targets() {
    let mut rooted = runtime();
    assert!(matches!(
        rooted.prepare_move_group_to_empty_space("editor", "main", "main"),
        Err(GraphPrepareError::TargetSpaceNotEmpty { .. })
    ));
    assert!(matches!(
        rooted.prepare_move_group_to_empty_space("editor", "main", "missing"),
        Err(GraphPrepareError::MissingSpace { .. })
    ));

    let mut floating = floating_only_runtime(true);
    assert!(matches!(
        floating.prepare_move_group_to_empty_space("source", "main", "main"),
        Err(GraphPrepareError::TargetSpaceNotEmpty { .. })
    ));
}

#[test]
fn empty_space_patch_preserves_newer_source_selection_but_rejects_target_topology_changes() {
    let mut runtime = empty_target_runtime();
    let editor_move = match runtime
        .prepare_move_group_to_empty_space("editor", "main", "empty")
        .expect("editor move should prepare")
    {
        GraphPrepareOutcome::Prepared(prepared) => prepared,
        GraphPrepareOutcome::NoOp(_) => panic!("editor move must be structural"),
    };
    let selection = match runtime
        .prepare_select_tab("editor", "B")
        .expect("editor selection should prepare")
    {
        GraphPrepareOutcome::Prepared(prepared) => prepared,
        GraphPrepareOutcome::NoOp(_) => panic!("selection should produce a patch"),
    };
    assert!(matches!(
        runtime.commit(selection),
        GraphCommitOutcome::Committed(_)
    ));
    assert!(matches!(
        runtime.commit(editor_move),
        GraphCommitOutcome::Committed(_)
    ));
    assert_eq!(
        runtime
            .graph()
            .group(&DockGroupId::from("editor"))
            .unwrap()
            .selected,
        item("B")
    );

    let mut runtime = empty_target_runtime();
    let editor_move = match runtime
        .prepare_move_group_to_empty_space("editor", "main", "empty")
        .unwrap()
    {
        GraphPrepareOutcome::Prepared(prepared) => prepared,
        GraphPrepareOutcome::NoOp(_) => panic!("editor move must be structural"),
    };
    let inspector_move = match runtime
        .prepare_move_group_to_empty_space("inspector", "main", "empty")
        .unwrap()
    {
        GraphPrepareOutcome::Prepared(prepared) => prepared,
        GraphPrepareOutcome::NoOp(_) => panic!("inspector move must be structural"),
    };
    assert!(matches!(
        runtime.commit(inspector_move),
        GraphCommitOutcome::Committed(_)
    ));
    assert!(matches!(
        runtime.commit(editor_move),
        GraphCommitOutcome::Superseded(receipt)
            if receipt.reason() == &GraphSupersededReason::TopologyChanged
    ));
}

#[test]
fn move_item_to_empty_space_creates_a_new_leaf_for_a_multi_item_source() {
    let mut runtime = empty_target_runtime();
    let source = DockGroupId::from("editor");
    let destination = DockGroupId::from("moved-a");

    let prepared = match runtime
        .prepare_move_item_to_empty_space("A", "main", "empty", destination.clone())
        .expect("multi-item extraction should prepare")
    {
        GraphPrepareOutcome::Prepared(prepared) => prepared,
        GraphPrepareOutcome::NoOp(_) => panic!("item extraction must be structural"),
    };
    assert!(matches!(
        runtime.commit(prepared),
        GraphCommitOutcome::Committed(_)
    ));

    let source_group = runtime.graph().group(&source).unwrap();
    assert_eq!(source_group.items, vec![item("B")]);
    assert_eq!(source_group.selected, item("B"));
    let destination_group = runtime.graph().group(&destination).unwrap();
    assert_eq!(destination_group.items, vec![item("A")]);
    assert_eq!(destination_group.selected, item("A"));
    assert_eq!(destination_group.mru, vec![item("A")]);
    let destination_node = runtime
        .graph()
        .index()
        .node_for_group(&destination)
        .unwrap();
    let target = runtime.graph().space(&DockSpaceId::from("empty")).unwrap();
    assert_eq!(target.root, Some(destination_node));
    assert_eq!(target.central.node, Some(destination_node));
    assert_eq!(
        runtime.graph().index().group_for_item(&item("A")),
        Some(&destination)
    );
    runtime
        .graph()
        .validate()
        .expect("multi-item extraction should preserve canonical indexes");
}

#[test]
fn move_item_to_empty_space_preserves_a_singleton_leaf_identity() {
    let mut runtime = empty_target_runtime();
    let source = DockGroupId::from("inspector");
    let source_node = runtime.graph().index().node_for_group(&source).unwrap();

    let prepared = match runtime
        .prepare_move_item_to_empty_space("C", "main", "empty", source.clone())
        .expect("singleton item move should prepare")
    {
        GraphPrepareOutcome::Prepared(prepared) => prepared,
        GraphPrepareOutcome::NoOp(_) => panic!("singleton item move must be structural"),
    };
    assert!(matches!(
        runtime.commit(prepared),
        GraphCommitOutcome::Committed(_)
    ));

    assert_eq!(
        runtime.graph().index().node_for_group(&source),
        Some(source_node)
    );
    let target = runtime.graph().space(&DockSpaceId::from("empty")).unwrap();
    assert_eq!(target.root, Some(source_node));
    assert_eq!(target.central.node, Some(source_node));
    runtime
        .graph()
        .validate()
        .expect("singleton item move should preserve canonical indexes");
}

#[test]
fn move_item_to_empty_space_can_promote_the_only_floating_singleton() {
    let mut runtime = floating_only_runtime(false);
    let source = DockGroupId::from("source");
    let source_node = runtime.graph().index().node_for_group(&source).unwrap();

    let prepared = match runtime
        .prepare_move_item_to_empty_space("S", "main", "main", source.clone())
        .expect("floating singleton promotion should prepare")
    {
        GraphPrepareOutcome::Prepared(prepared) => prepared,
        GraphPrepareOutcome::NoOp(_) => panic!("floating singleton promotion must be structural"),
    };
    assert!(matches!(
        runtime.commit(prepared),
        GraphCommitOutcome::Committed(_)
    ));

    let space = runtime.graph().space(&DockSpaceId::from("main")).unwrap();
    assert_eq!(space.root, Some(source_node));
    assert_eq!(space.central.node, Some(source_node));
    assert!(space.floating_roots.is_empty());
    assert!(runtime.graph().floating_roots().is_empty());
    runtime
        .graph()
        .validate()
        .expect("floating singleton promotion should preserve canonical indexes");
}

#[test]
fn move_item_to_empty_space_rejects_invalid_group_identity() {
    let mut singleton = empty_target_runtime();
    assert!(matches!(
        singleton.prepare_move_item_to_empty_space("C", "main", "empty", "new-inspector"),
        Err(GraphPrepareError::InvalidItemMoveGroupIdentity { .. })
    ));

    let mut multi = empty_target_runtime();
    assert!(matches!(
        multi.prepare_move_item_to_empty_space("A", "main", "empty", "editor"),
        Err(GraphPrepareError::InvalidItemMoveGroupIdentity { .. })
    ));

    let mut occupied = runtime();
    assert!(matches!(
        occupied.prepare_move_item_to_empty_space("A", "main", "main", "moved-a"),
        Err(GraphPrepareError::TargetSpaceNotEmpty { .. })
    ));
}

#[test]
fn float_item_in_window_extracts_a_multi_item_source_into_a_new_floating_group() {
    let mut runtime = three_tab_runtime();
    let source = DockGroupId::from("editor");
    let destination = DockGroupId::from("floating-a-group");
    let floating = crate::DockFloatingId::from("floating-a");
    let source_node = runtime.graph().index().node_for_group(&source).unwrap();

    let prepared = match runtime
        .prepare_float_item_in_window(
            item("A"),
            destination.clone(),
            floating.clone(),
            "main",
            "main",
            floating_bounds(),
        )
        .expect("item floating should prepare")
    {
        GraphPrepareOutcome::Prepared(prepared) => prepared,
        GraphPrepareOutcome::NoOp(_) => panic!("creating a floating root must be structural"),
    };
    assert!(matches!(
        prepared.operation(),
        GraphPatchOperation::FloatItemInWindow {
            item: prepared_item,
            source_group,
            new_group,
            floating: prepared_floating,
            bounds,
            z_order: 0,
            ..
        } if prepared_item == &item("A")
            && source_group == &source
            && new_group == &destination
            && prepared_floating == &floating
            && bounds.get() == floating_bounds()
    ));
    let receipt = match runtime.commit(prepared) {
        GraphCommitOutcome::Committed(receipt) => receipt,
        other => panic!("item floating should commit, got {other:?}"),
    };
    assert_eq!(receipt.after().topology(), receipt.before().topology() + 1);

    assert_eq!(
        runtime.graph().group(&source).unwrap().items,
        vec![item("B"), item("C")]
    );
    let floating_group = runtime.graph().group(&destination).unwrap();
    assert_eq!(floating_group.items, vec![item("A")]);
    assert_eq!(floating_group.selected, item("A"));
    assert_eq!(floating_group.mru, vec![item("A")]);
    let floating_root = runtime.graph().floating_roots().get(&floating).unwrap();
    assert_eq!(floating_root.space, DockSpaceId::from("main"));
    assert_eq!(floating_root.bounds, floating_bounds());
    assert_eq!(floating_root.z_order, 0);
    assert_ne!(floating_root.root, source_node);
    assert_eq!(
        runtime.graph().index().floating_for_item(&item("A")),
        Some(&floating)
    );
    runtime
        .graph()
        .validate()
        .expect("item floating should preserve canonical indexes");
}

#[test]
fn float_singleton_in_window_preserves_group_and_leaf_identity_across_spaces() {
    let mut assembly = DockGraphAssembly::new();
    assembly.add_space("source").unwrap();
    assembly.add_space("target").unwrap();
    let source = assembly
        .add_group("source-group", [item("A")], item("A"))
        .unwrap();
    let target = assembly
        .add_group("target-group", [item("T")], item("T"))
        .unwrap();
    assembly.set_root("source", source).unwrap();
    assembly.set_root("target", target).unwrap();
    let mut runtime = CanonicalGraphRuntime::from_assembly(assembly).unwrap();
    let source_group = DockGroupId::from("source-group");
    let source_node = runtime
        .graph()
        .index()
        .node_for_group(&source_group)
        .unwrap();
    let target_node = runtime
        .graph()
        .index()
        .node_for_group(&DockGroupId::from("target-group"))
        .unwrap();
    let floating = crate::DockFloatingId::from("floating-a");

    let prepared = match runtime
        .prepare_float_item_in_window(
            item("A"),
            source_group.clone(),
            floating.clone(),
            "source",
            "target",
            floating_bounds(),
        )
        .expect("singleton floating should prepare")
    {
        GraphPrepareOutcome::Prepared(prepared) => prepared,
        GraphPrepareOutcome::NoOp(_) => panic!("creating a floating root must be structural"),
    };
    assert!(matches!(
        runtime.commit(prepared),
        GraphCommitOutcome::Committed(_)
    ));

    assert_eq!(
        runtime
            .graph()
            .space(&DockSpaceId::from("source"))
            .unwrap()
            .root,
        None
    );
    assert_eq!(
        runtime
            .graph()
            .space(&DockSpaceId::from("target"))
            .unwrap()
            .root,
        Some(target_node)
    );
    assert_eq!(
        runtime.graph().index().node_for_group(&source_group),
        Some(source_node)
    );
    assert_eq!(
        runtime
            .graph()
            .floating_roots()
            .get(&floating)
            .unwrap()
            .root,
        source_node
    );
    assert_eq!(
        runtime.graph().index().floating_for_item(&item("A")),
        Some(&floating)
    );
    runtime
        .graph()
        .validate()
        .expect("singleton floating should preserve canonical indexes");
}

#[test]
fn float_item_in_window_appends_after_the_highest_existing_z_order() {
    let mut runtime = floating_runtime();
    let floating = crate::DockFloatingId::from("floating-source");
    let prepared = match runtime
        .prepare_float_item_in_window(
            item("S"),
            "source",
            floating.clone(),
            "main",
            "main",
            floating_bounds(),
        )
        .expect("floating above an existing root should prepare")
    {
        GraphPrepareOutcome::Prepared(prepared) => prepared,
        GraphPrepareOutcome::NoOp(_) => panic!("creating a floating root must be structural"),
    };
    assert!(matches!(
        prepared.operation(),
        GraphPatchOperation::FloatItemInWindow { z_order: 2, .. }
    ));
    assert!(matches!(
        runtime.commit(prepared),
        GraphCommitOutcome::Committed(_)
    ));

    let space = runtime.graph().space(&DockSpaceId::from("main")).unwrap();
    assert_eq!(
        space.floating_roots,
        vec![
            crate::DockFloatingId::from("floating-target"),
            floating.clone()
        ]
    );
    assert_eq!(runtime.graph().floating_roots()[&floating].z_order, 2);
    runtime
        .graph()
        .validate()
        .expect("floating z-order append should remain canonical");
}

#[test]
fn refloating_a_singleton_excludes_its_retired_root_from_z_order() {
    let mut runtime = floating_only_runtime(false);
    let retired = crate::DockFloatingId::from("floating-source");
    let replacement = crate::DockFloatingId::from("floating-replacement");
    let prepared = match runtime
        .prepare_float_item_in_window(
            item("S"),
            "source",
            replacement.clone(),
            "main",
            "main",
            floating_bounds(),
        )
        .expect("refloating the only singleton should prepare")
    {
        GraphPrepareOutcome::Prepared(prepared) => prepared,
        GraphPrepareOutcome::NoOp(_) => panic!("refloating must be structural"),
    };
    assert!(matches!(
        prepared.operation(),
        GraphPatchOperation::FloatItemInWindow {
            retired_floating: Some(prepared_retired),
            z_order: 0,
            ..
        } if prepared_retired == &retired
    ));
    assert!(matches!(
        runtime.commit(prepared),
        GraphCommitOutcome::Committed(_)
    ));

    assert!(!runtime.graph().floating_roots().contains_key(&retired));
    assert_eq!(runtime.graph().floating_roots()[&replacement].z_order, 0);
    assert_eq!(
        runtime
            .graph()
            .space(&DockSpaceId::from("main"))
            .unwrap()
            .floating_roots,
        vec![replacement]
    );
    runtime
        .graph()
        .validate()
        .expect("refloating should replace the root without inflating z-order");
}

#[test]
fn float_item_prepare_rejects_invalid_identity_bounds_and_floating_collisions() {
    let mut runtime = three_tab_runtime();
    assert!(matches!(
        runtime.prepare_float_item_in_window(
            item("A"),
            "editor",
            "floating-a",
            "main",
            "main",
            floating_bounds(),
        ),
        Err(GraphPrepareError::InvalidItemMoveGroupIdentity { .. })
    ));
    assert_eq!(
        runtime
            .prepare_float_item_in_window(
                item("A"),
                "floating-a-group",
                "floating-a",
                "main",
                "main",
                DockLayoutRect {
                    width: 0.0,
                    ..floating_bounds()
                },
            )
            .expect_err("zero-width floating bounds must fail during prepare"),
        GraphPrepareError::InvalidFloatingBounds
    );

    let mut runtime = floating_runtime();
    assert_eq!(
        runtime
            .prepare_float_item_in_window(
                item("S"),
                "source",
                "floating-target",
                "main",
                "main",
                floating_bounds(),
            )
            .expect_err("floating identity collision must fail during prepare"),
        GraphPrepareError::FloatingAlreadyExists {
            floating: crate::DockFloatingId::from("floating-target"),
        }
    );

    let mut assembly = DockGraphAssembly::new();
    assembly.add_space("main").unwrap();
    let source = assembly
        .add_group("source", [item("S")], item("S"))
        .unwrap();
    let existing = assembly
        .add_group("existing", [item("E")], item("E"))
        .unwrap();
    assembly.set_root("main", source).unwrap();
    assembly
        .add_floating_root(
            "floating-max",
            "main",
            existing,
            floating_bounds(),
            i32::MAX,
        )
        .unwrap();
    let mut runtime = CanonicalGraphRuntime::from_assembly(assembly).unwrap();
    assert_eq!(
        runtime
            .prepare_float_item_in_window(
                item("S"),
                "source",
                "floating-overflow",
                "main",
                "main",
                floating_bounds(),
            )
            .expect_err("floating z-order exhaustion must fail during prepare"),
        GraphPrepareError::FloatingZOrderExhausted {
            space: DockSpaceId::from("main"),
        }
    );
}

#[test]
fn move_item_to_group_preserves_source_group_and_updates_target_selection() {
    let mut runtime = runtime();
    let prepared = match runtime
        .prepare_move_item_to_group("A", "inspector", "main", "main", None)
        .expect("center item move should prepare")
    {
        GraphPrepareOutcome::Prepared(prepared) => prepared,
        GraphPrepareOutcome::NoOp(_) => panic!("item move must be structural"),
    };
    assert!(matches!(
        prepared.operation(),
        GraphPatchOperation::MoveItemToGroup {
            item: moved_item,
            target_group,
            insert_index: None,
            ..
        } if moved_item == &item("A") && target_group == &DockGroupId::from("inspector")
    ));
    assert!(matches!(
        runtime.commit(prepared),
        GraphCommitOutcome::Committed(_)
    ));

    assert_eq!(
        runtime
            .graph()
            .group(&DockGroupId::from("editor"))
            .unwrap()
            .items,
        vec![item("B")]
    );
    assert_eq!(
        runtime
            .graph()
            .group(&DockGroupId::from("editor"))
            .unwrap()
            .selected,
        item("B")
    );
    let target = runtime
        .graph()
        .group(&DockGroupId::from("inspector"))
        .unwrap();
    assert_eq!(target.items, vec![item("C"), item("D"), item("A")]);
    assert_eq!(target.selected, item("A"));
    assert_eq!(target.mru, vec![item("A")]);
    assert_eq!(
        runtime.graph().index().group_for_item(&item("A")),
        Some(&DockGroupId::from("inspector"))
    );
    runtime
        .graph()
        .validate()
        .expect("center item move should preserve canonical indexes");
}

#[test]
fn move_item_to_edge_extracts_a_multi_item_source_into_a_new_binary_leaf() {
    let mut runtime = multiview_runtime();
    let source = DockGroupId::from("editor");
    let destination = DockGroupId::from("moved-a");
    let target = DockGroupId::from("detached-target");
    let target_node = runtime.graph().index().node_for_group(&target).unwrap();

    let prepared = match runtime
        .prepare_move_item_to_edge(
            "A",
            destination.clone(),
            target.clone(),
            "main",
            "detached",
            SplitAxis::Horizontal,
            false,
            0.5,
        )
        .expect("cross-space item edge move should prepare")
    {
        GraphPrepareOutcome::Prepared(prepared) => prepared,
        GraphPrepareOutcome::NoOp(_) => panic!("item edge move must be structural"),
    };
    assert!(matches!(
        prepared.operation(),
        GraphPatchOperation::MoveItemToEdge { item: moved, new_group, .. }
            if moved == &item("A") && new_group == &destination
    ));
    assert!(matches!(
        runtime.commit(prepared),
        GraphCommitOutcome::Committed(_)
    ));

    assert_eq!(
        runtime.graph().group(&source).unwrap().items,
        vec![item("B")]
    );
    let moved_group = runtime.graph().group(&destination).unwrap();
    assert_eq!(moved_group.items, vec![item("A")]);
    assert_eq!(moved_group.selected, item("A"));
    assert_eq!(moved_group.mru, vec![item("A")]);
    let moved_node = runtime
        .graph()
        .index()
        .node_for_group(&destination)
        .unwrap();
    let root = runtime
        .graph()
        .space(&DockSpaceId::from("detached"))
        .and_then(|space| space.root)
        .unwrap();
    assert!(matches!(
        runtime.graph().node(root),
        Some(CanonicalNode::Branch {
            axis: SplitAxis::Horizontal,
            first,
            second,
            ratio,
            ..
        }) if *first == target_node && *second == moved_node && *ratio == 0.5
    ));
    assert_eq!(
        runtime.graph().index().space_for_node(moved_node),
        Some(&DockSpaceId::from("detached"))
    );
    runtime
        .graph()
        .validate()
        .expect("multi-item edge extraction should preserve canonical indexes");
}

#[test]
fn move_item_to_edge_preserves_a_singleton_group_and_leaf_identity() {
    let mut runtime = multiview_runtime();
    let source = DockGroupId::from("inspector");
    let target = DockGroupId::from("detached-target");
    let original_main_root = runtime
        .graph()
        .space(&DockSpaceId::from("main"))
        .and_then(|space| space.root)
        .unwrap();
    let source_node = runtime.graph().index().node_for_group(&source).unwrap();
    let target_node = runtime.graph().index().node_for_group(&target).unwrap();
    let editor_node = runtime
        .graph()
        .index()
        .node_for_group(&DockGroupId::from("editor"))
        .unwrap();

    let prepared = match runtime
        .prepare_move_item_to_edge(
            "C",
            source.clone(),
            target,
            "main",
            "detached",
            SplitAxis::Vertical,
            true,
            0.4,
        )
        .expect("singleton item edge move should prepare")
    {
        GraphPrepareOutcome::Prepared(prepared) => prepared,
        GraphPrepareOutcome::NoOp(_) => panic!("cross-space singleton move must be structural"),
    };
    assert!(matches!(
        runtime.commit(prepared),
        GraphCommitOutcome::Committed(_)
    ));

    assert_eq!(
        runtime.graph().index().node_for_group(&source),
        Some(source_node)
    );
    assert_eq!(
        runtime
            .graph()
            .space(&DockSpaceId::from("main"))
            .and_then(|space| space.root),
        Some(original_main_root)
    );
    assert_eq!(
        runtime
            .graph()
            .index()
            .node_for_group(&DockGroupId::from("editor")),
        Some(original_main_root)
    );
    assert!(runtime.graph().node(editor_node).is_none());
    let root = runtime
        .graph()
        .space(&DockSpaceId::from("detached"))
        .and_then(|space| space.root)
        .unwrap();
    assert!(matches!(
        runtime.graph().node(root),
        Some(CanonicalNode::Branch {
            axis: SplitAxis::Vertical,
            first,
            second,
            ratio,
            ..
        }) if *first == source_node && *second == target_node && *ratio == 0.4
    ));
    runtime
        .graph()
        .validate()
        .expect("singleton edge move should preserve canonical indexes");
}

#[test]
fn move_item_to_edge_can_split_an_item_from_its_own_group() {
    let mut runtime = runtime();
    let source = DockGroupId::from("editor");
    let destination = DockGroupId::from("split-a");
    let source_node = runtime.graph().index().node_for_group(&source).unwrap();

    let prepared = match runtime
        .prepare_move_item_to_edge(
            "A",
            destination.clone(),
            source.clone(),
            "main",
            "main",
            SplitAxis::Horizontal,
            true,
            0.5,
        )
        .expect("same-group edge extraction should prepare")
    {
        GraphPrepareOutcome::Prepared(prepared) => prepared,
        GraphPrepareOutcome::NoOp(_) => panic!("same-group edge extraction must be structural"),
    };
    assert!(matches!(
        runtime.commit(prepared),
        GraphCommitOutcome::Committed(_)
    ));

    assert_eq!(
        runtime.graph().group(&source).unwrap().items,
        vec![item("B")]
    );
    let destination_node = runtime
        .graph()
        .index()
        .node_for_group(&destination)
        .unwrap();
    let parent = runtime
        .graph()
        .index()
        .parent_of(source_node)
        .flatten()
        .expect("source group should be wrapped by the new split");
    assert!(matches!(
        runtime.graph().node(parent),
        Some(CanonicalNode::Branch { first, second, .. })
            if *first == destination_node && *second == source_node
    ));
    runtime
        .graph()
        .validate()
        .expect("same-group edge extraction should preserve canonical indexes");
}

#[test]
fn move_item_to_edge_rejects_invalid_group_identity_and_noops_on_a_singleton_self_target() {
    let mut multi = runtime();
    assert!(matches!(
        multi.prepare_move_item_to_edge(
            "A",
            "editor",
            "inspector",
            "main",
            "main",
            SplitAxis::Horizontal,
            false,
            0.5,
        ),
        Err(GraphPrepareError::InvalidItemMoveGroupIdentity { .. })
    ));

    let mut singleton = empty_target_runtime();
    let outcome = singleton
        .prepare_move_item_to_edge(
            "C",
            "inspector",
            "inspector",
            "main",
            "main",
            SplitAxis::Horizontal,
            false,
            0.5,
        )
        .expect("singleton self edge should resolve");
    assert!(matches!(
        outcome,
        GraphPrepareOutcome::NoOp(receipt)
            if matches!(receipt.operation(), GraphPatchOperation::MoveItemToEdge { .. })
    ));
}

#[test]
fn moving_the_last_item_retires_the_source_leaf_and_floating_owner() {
    let mut runtime = floating_source_runtime();
    let prepared = match runtime
        .prepare_move_item_to_group("S", "target", "main", "main", None)
        .expect("floating item center move should prepare")
    {
        GraphPrepareOutcome::Prepared(prepared) => prepared,
        GraphPrepareOutcome::NoOp(_) => panic!("floating item move must be structural"),
    };
    assert!(matches!(
        runtime.commit(prepared),
        GraphCommitOutcome::Committed(_)
    ));

    assert!(
        runtime
            .graph()
            .group(&DockGroupId::from("source"))
            .is_none()
    );
    assert!(runtime.graph().floating_roots().is_empty());
    assert_eq!(
        runtime
            .graph()
            .group(&DockGroupId::from("target"))
            .unwrap()
            .items,
        vec![item("T"), item("S")]
    );
    assert!(
        runtime
            .graph()
            .index()
            .floating_for_item(&item("S"))
            .is_none()
    );
    runtime
        .graph()
        .validate()
        .expect("last floating item move should preserve canonical indexes");
}

#[test]
fn same_group_item_move_reorders_tabs_and_adjacent_moves_are_noops() {
    let mut runtime = runtime();
    let no_op = runtime
        .prepare_move_item_to_group("B", "editor", "main", "main", Some(2))
        .expect("adjacent tab move should resolve");
    assert!(matches!(
        no_op,
        GraphPrepareOutcome::NoOp(receipt)
            if matches!(receipt.operation(), GraphPatchOperation::MoveItemToGroup { .. })
    ));

    let prepared = match runtime
        .prepare_move_item_to_group("B", "editor", "main", "main", Some(0))
        .expect("tab reorder should prepare")
    {
        GraphPrepareOutcome::Prepared(prepared) => prepared,
        GraphPrepareOutcome::NoOp(_) => panic!("non-adjacent tab move must be structural"),
    };
    assert!(matches!(
        runtime.commit(prepared),
        GraphCommitOutcome::Committed(_)
    ));
    let editor = runtime.graph().group(&DockGroupId::from("editor")).unwrap();
    assert_eq!(editor.items, vec![item("B"), item("A")]);
    assert_eq!(editor.selected, item("B"));
    assert_eq!(editor.mru, vec![item("B")]);
    runtime
        .graph()
        .validate()
        .expect("tab reorder should preserve canonical indexes");
}

#[test]
fn same_group_item_move_can_place_the_first_tab_after_the_last_tab() {
    let mut runtime = three_tab_runtime();
    let prepared = match runtime
        .prepare_move_item_to_group("A", "editor", "main", "main", Some(3))
        .expect("trailing tab reorder should prepare")
    {
        GraphPrepareOutcome::Prepared(prepared) => prepared,
        GraphPrepareOutcome::NoOp(_) => panic!("trailing tab reorder must be structural"),
    };
    assert!(matches!(
        runtime.commit(prepared),
        GraphCommitOutcome::Committed(_)
    ));

    let editor = runtime.graph().group(&DockGroupId::from("editor")).unwrap();
    assert_eq!(editor.items, vec![item("B"), item("C"), item("A")]);
    assert_eq!(editor.selected, item("A"));
    runtime
        .graph()
        .validate()
        .expect("trailing tab reorder should preserve canonical indexes");
}

#[test]
fn item_move_read_set_allows_unrelated_selection_but_rejects_touched_group_changes() {
    let mut runtime = runtime();
    let prepared = match runtime
        .prepare_move_item_to_group("A", "inspector", "main", "main", None)
        .expect("item move should prepare")
    {
        GraphPrepareOutcome::Prepared(prepared) => prepared,
        GraphPrepareOutcome::NoOp(_) => panic!("item move must be structural"),
    };
    let unrelated = match runtime
        .prepare_select_tab("inspector", "D")
        .expect("unrelated selection should prepare")
    {
        GraphPrepareOutcome::Prepared(prepared) => prepared,
        GraphPrepareOutcome::NoOp(_) => panic!("selection should produce a patch"),
    };
    assert!(matches!(
        runtime.commit(unrelated),
        GraphCommitOutcome::Committed(_)
    ));
    assert!(matches!(
        runtime.commit(prepared),
        GraphCommitOutcome::Superseded(receipt)
            if receipt.reason() == &GraphSupersededReason::GroupSelectionChanged
    ));
}

#[test]
fn move_group_to_edge_can_insert_into_a_floating_root() {
    let mut runtime = floating_runtime();
    let target = DockGroupId::from("target");
    let source = DockGroupId::from("source");
    let target_node = runtime.graph().index().node_for_group(&target).unwrap();
    let source_node = runtime.graph().index().node_for_group(&source).unwrap();

    let prepared = match runtime
        .prepare_move_group_to_edge(
            source.clone(),
            target.clone(),
            "main",
            "main",
            SplitAxis::Horizontal,
            false,
            0.5,
        )
        .expect("floating-target group move should prepare")
    {
        GraphPrepareOutcome::Prepared(prepared) => prepared,
        GraphPrepareOutcome::NoOp(_) => panic!("group move must produce a structural patch"),
    };
    assert!(matches!(
        runtime.commit(prepared),
        GraphCommitOutcome::Committed(_)
    ));

    assert!(
        runtime
            .graph()
            .space(&DockSpaceId::from("main"))
            .is_some_and(|space| space.root.is_none())
    );
    let floating_root = runtime
        .graph()
        .floating_roots()
        .get(&crate::DockFloatingId::from("floating-target"))
        .expect("floating root should remain present")
        .root;
    assert!(matches!(
        runtime.graph().node(floating_root),
        Some(CanonicalNode::Branch {
            axis: SplitAxis::Horizontal,
            first,
            second,
            ..
        }) if *first == target_node && *second == source_node
    ));
    runtime
        .graph()
        .validate()
        .expect("floating-target move should preserve canonical indexes");
}

#[test]
fn move_group_to_edge_can_move_a_floating_source_into_the_main_root() {
    let mut runtime = floating_source_runtime();
    let target = DockGroupId::from("target");
    let source = DockGroupId::from("source");
    let target_node = runtime.graph().index().node_for_group(&target).unwrap();
    let source_node = runtime.graph().index().node_for_group(&source).unwrap();

    let prepared = match runtime
        .prepare_move_group_to_edge(
            source,
            target.clone(),
            "main",
            "main",
            SplitAxis::Horizontal,
            true,
            0.5,
        )
        .expect("floating-source group move should prepare")
    {
        GraphPrepareOutcome::Prepared(prepared) => prepared,
        GraphPrepareOutcome::NoOp(_) => panic!("group move must produce a structural patch"),
    };
    assert!(matches!(
        runtime.commit(prepared),
        GraphCommitOutcome::Committed(_)
    ));

    let main_root = runtime
        .graph()
        .space(&DockSpaceId::from("main"))
        .and_then(|space| space.root)
        .expect("main root should contain the moved group");
    assert!(matches!(
        runtime.graph().node(main_root),
        Some(CanonicalNode::Branch {
            axis: SplitAxis::Horizontal,
            first,
            second,
            ..
        }) if *first == source_node && *second == target_node
    ));
    assert!(
        runtime
            .graph()
            .floating_roots()
            .get(&crate::DockFloatingId::from("floating-source"))
            .is_none()
    );
    assert!(
        runtime
            .graph()
            .space(&DockSpaceId::from("main"))
            .is_some_and(|space| space.floating_roots.is_empty())
    );
    runtime
        .graph()
        .validate()
        .expect("floating-source move should preserve canonical indexes");
}

#[test]
fn move_floating_root_to_edge_preserves_the_subtree_and_retires_floating_ownership() {
    let mut runtime = floating_subtree_runtime();
    let floating = crate::DockFloatingId::from("floating-subtree");
    let target = DockGroupId::from("target");
    let source_left = DockGroupId::from("source-left");
    let source_right = DockGroupId::from("source-right");
    let target_node = runtime.graph().index().node_for_group(&target).unwrap();
    let source_left_node = runtime
        .graph()
        .index()
        .node_for_group(&source_left)
        .unwrap();
    let source_right_node = runtime
        .graph()
        .index()
        .node_for_group(&source_right)
        .unwrap();
    let floating_root = runtime
        .graph()
        .floating_roots()
        .get(&floating)
        .unwrap()
        .root;

    let prepared = match runtime
        .prepare_move_floating_root_to_edge(
            floating.clone(),
            "main",
            target,
            "main",
            SplitAxis::Horizontal,
            true,
            0.4,
        )
        .expect("floating subtree move should prepare")
    {
        GraphPrepareOutcome::Prepared(prepared) => prepared,
        GraphPrepareOutcome::NoOp(_) => panic!("floating subtree move must be structural"),
    };
    assert!(matches!(
        runtime.commit(prepared),
        GraphCommitOutcome::Committed(_)
    ));

    let main_root = runtime
        .graph()
        .space(&DockSpaceId::from("main"))
        .and_then(|space| space.root)
        .expect("main root should contain the moved floating subtree");
    let Some(CanonicalNode::Branch {
        axis: outer_axis,
        first,
        second,
        ..
    }) = runtime.graph().node(main_root)
    else {
        panic!("main root should be the new outer branch");
    };
    assert_eq!(*outer_axis, SplitAxis::Horizontal);
    assert_eq!(*first, floating_root);
    assert_eq!(*second, target_node);
    assert!(matches!(
        runtime.graph().node(floating_root),
        Some(CanonicalNode::Branch {
            axis: SplitAxis::Vertical,
            first,
            second,
            ..
        }) if *first == source_left_node && *second == source_right_node
    ));
    assert!(runtime.graph().floating_roots().get(&floating).is_none());
    assert!(
        runtime
            .graph()
            .space(&DockSpaceId::from("main"))
            .is_some_and(|space| space.floating_roots.is_empty())
    );
    runtime
        .graph()
        .validate()
        .expect("floating subtree move should preserve canonical indexes");
}
