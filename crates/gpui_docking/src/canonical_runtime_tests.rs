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
