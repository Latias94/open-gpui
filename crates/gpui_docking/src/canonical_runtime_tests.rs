use crate::canonical_runtime::CanonicalGraphRuntime;
use crate::graph_assembly::{CanonicalNode, DockGraphAssembly};
use crate::graph_patch::{
    GraphCommitOutcome, GraphPatchOperation, GraphPrepareError, GraphPrepareOutcome,
    GraphSupersededReason,
};
use crate::{DockGroupId, DockItemId, DockSpaceId, SplitAxis};

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
        Some(inspector_node)
    );
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
fn move_group_to_edge_relinks_a_same_space_sibling_without_replacing_leaf_ids() {
    let mut runtime = runtime();
    let editor = DockGroupId::from("editor");
    let inspector = DockGroupId::from("inspector");
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
        }) if *first == editor_node && *second == inspector_node && *ratio == 0.35
    ));
    assert_eq!(
        runtime.graph().index().node_for_group(&editor),
        Some(editor_node)
    );
    assert_eq!(
        runtime.graph().index().node_for_group(&inspector),
        Some(inspector_node)
    );
    runtime
        .graph()
        .validate()
        .expect("same-space group move should preserve canonical indexes");
}

#[test]
fn move_group_to_center_merges_tabs_and_retires_the_source_leaf() {
    let mut runtime = runtime();
    let editor = DockGroupId::from("editor");
    let inspector = DockGroupId::from("inspector");
    let inspector_node = runtime.graph().index().node_for_group(&inspector).unwrap();

    let prepared = match runtime
        .prepare_move_group_to_center(
            editor.clone(),
            inspector.clone(),
            "main",
            "main",
        )
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
    assert!(runtime.graph().group(&editor).is_none());
    let target = runtime
        .graph()
        .group(&inspector)
        .expect("target group should remain present");
    assert_eq!(target.items, vec![item("C"), item("D"), item("A"), item("B")]);
    assert_eq!(target.selected, item("A"));
    assert_eq!(target.mru, vec![item("A"), item("C")]);
    assert_eq!(
        runtime
            .graph()
            .space(&DockSpaceId::from("main"))
            .and_then(|space| space.root),
        Some(inspector_node)
    );
    assert_eq!(runtime.graph().nodes().len(), 1);
    assert_eq!(
        runtime.graph().index().group_for_item(&item("A")),
        Some(&inspector)
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
    let inspector_node = runtime.graph().index().node_for_group(&inspector).unwrap();
    let detached_target_node = runtime
        .graph()
        .index()
        .node_for_group(&detached_target)
        .unwrap();

    let prepared = match runtime
        .prepare_move_group_to_center(
            editor.clone(),
            detached_target.clone(),
            "main",
            "detached",
        )
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
        Some(inspector_node)
    );
    assert_eq!(
        runtime
            .graph()
            .space(&DockSpaceId::from("detached"))
            .and_then(|space| space.root),
        Some(detached_target_node)
    );
    assert!(runtime.graph().group(&editor).is_none());
    assert_eq!(
        runtime.graph().group(&detached_target).unwrap().items,
        vec![item("X"), item("A"), item("B")]
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

    assert!(runtime.graph().group(&source).is_none());
    assert_eq!(
        runtime.graph().group(&target).unwrap().items,
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
    assert!(runtime.graph().group(&source).is_none());
    assert_eq!(
        runtime.graph().group(&target).unwrap().items,
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
