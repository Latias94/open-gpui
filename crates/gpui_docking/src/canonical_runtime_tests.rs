use crate::canonical_runtime::CanonicalGraphRuntime;
use crate::graph_assembly::DockGraphAssembly;
use crate::graph_patch::{GraphCommitOutcome, GraphPrepareOutcome, GraphSupersededReason};
use crate::{DockItemId, SplitAxis};

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
