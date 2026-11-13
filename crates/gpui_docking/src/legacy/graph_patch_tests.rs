use crate::legacy::graph_assembly::{CanonicalDockGraph, DockGraphAssembly};
use crate::legacy::graph_patch::{
    GraphAppliedPatch, GraphCommitEffects, GraphCommitOutcome, GraphGroupRead, GraphPatchOperation,
    GraphPatchTarget, GraphPrepareOutcome, GraphReadView, GraphRevision, GraphSupersededReason,
};
use crate::legacy::graph_transaction::GraphTransactionEngine;
use crate::legacy::ids::CanonicalGraphAuthority;
use crate::legacy::{DockGroupId, DockItemId};
use std::collections::BTreeMap;

fn item(id: &str) -> DockItemId {
    DockItemId::new(id)
}

fn canonical_graph() -> CanonicalDockGraph {
    let mut authority = CanonicalGraphAuthority::new();
    let mut assembly = DockGraphAssembly::new();
    assembly.add_space("main").unwrap();
    let editor = assembly
        .add_group("editor", [item("A"), item("B")], item("A"))
        .unwrap();
    let inspector = assembly
        .add_group("inspector", [item("C"), item("D")], item("C"))
        .unwrap();
    let root = assembly
        .add_branch(crate::legacy::SplitAxis::Horizontal, editor, inspector, 0.5)
        .unwrap();
    assembly.set_root("main", root).unwrap();
    assembly.publish(&mut authority).unwrap()
}

#[derive(Debug)]
struct SelectionTarget {
    revision: GraphRevision,
    groups: BTreeMap<DockGroupId, GraphGroupRead>,
    applied: usize,
}

impl SelectionTarget {
    fn from_graph(graph: &CanonicalDockGraph) -> Self {
        let groups = graph
            .groups()
            .keys()
            .map(|id| {
                let read = GraphReadView::read_group(graph, id)
                    .expect("published fixture group should remain readable");
                (id.clone(), read)
            })
            .collect();
        Self {
            revision: GraphReadView::graph_revision(graph),
            groups,
            applied: 0,
        }
    }

    fn select_unrelated_group(&mut self, group: &DockGroupId, item: &DockItemId) {
        let group_state = self
            .groups
            .get_mut(group)
            .expect("fixture group should exist");
        group_state.selected = item.clone();
        group_state.mru.retain(|entry| entry != item);
        group_state.mru.insert(0, item.clone());
        self.revision = self
            .revision
            .with_selection(self.revision.selection() + 1)
            .with_interaction(self.revision.interaction() + 1);
    }

    fn change_topology(&mut self) {
        self.revision = self.revision.with_topology(self.revision.topology() + 1);
    }

    fn replace_authority(&mut self) {
        self.revision = GraphRevision::new(
            self.revision.authority_epoch() + 1,
            self.revision.topology(),
            self.revision.selection(),
            self.revision.interaction(),
        );
    }
}

impl GraphReadView for SelectionTarget {
    fn graph_revision(&self) -> GraphRevision {
        self.revision
    }

    fn read_group(&self, group: &DockGroupId) -> Option<GraphGroupRead> {
        self.groups.get(group).cloned()
    }
}

impl GraphPatchTarget for SelectionTarget {
    fn validate_capacity_proof(
        &self,
        _commit_id: crate::legacy::graph_patch::GraphCommitId,
        _operation: &GraphPatchOperation,
        _mutation: &crate::legacy::graph_patch::GraphPreparedMutation,
        _proof: Option<&crate::legacy::graph_patch::GraphCapacityProof>,
    ) -> Result<(), GraphSupersededReason> {
        Ok(())
    }

    fn apply_prepared_patch(
        &mut self,
        operation: &GraphPatchOperation,
        mutation: crate::legacy::graph_patch::GraphPreparedMutation,
    ) -> GraphAppliedPatch {
        let (
            GraphPatchOperation::SelectTab { group, item },
            crate::legacy::graph_patch::GraphPreparedMutation::SelectTab {
                selection_changed,
                interaction_changed,
            },
        ) = (operation, mutation)
        else {
            panic!("selection target does not support structural patches")
        };
        let Some(group_state) = self.groups.get_mut(group) else {
            return GraphAppliedPatch::new(self.revision, GraphCommitEffects::default());
        };
        group_state.selected = item.clone();
        group_state.mru.retain(|entry| entry != item);
        group_state.mru.insert(0, item.clone());
        self.applied += 1;
        if selection_changed {
            self.revision = self.revision.with_selection(self.revision.selection() + 1);
        }
        if interaction_changed {
            self.revision = self
                .revision
                .with_interaction(self.revision.interaction() + 1);
        }
        GraphAppliedPatch::new(self.revision, GraphCommitEffects::default())
    }
}

#[test]
fn prepare_is_read_only_and_captures_only_the_touched_group() {
    let graph = canonical_graph();
    let mut engine = GraphTransactionEngine::new();
    let prepared = match engine.prepare_select_tab(&graph, "editor", "B").unwrap() {
        GraphPrepareOutcome::Prepared(prepared) => prepared,
        GraphPrepareOutcome::NoOp(_) => panic!("selection should change"),
    };

    assert_eq!(
        graph.group(&DockGroupId::from("editor")).unwrap().selected,
        item("A")
    );
    assert_eq!(
        prepared
            .read_set()
            .group()
            .expect("selection read set should include a group")
            .group,
        DockGroupId::from("editor")
    );
    assert_eq!(
        prepared
            .read_set()
            .group()
            .expect("selection read set should include a group")
            .items,
        vec![item("A"), item("B")]
    );
    assert!(prepared.read_set().domains().topology);
    assert!(!prepared.read_set().domains().selection);
    assert!(matches!(
        prepared.operation(),
        GraphPatchOperation::SelectTab { group, item: selected_item }
            if group == &DockGroupId::from("editor") && selected_item == &item("B")
    ));
}

#[test]
fn releasing_the_latest_unpublished_preparation_reuses_its_generation() {
    let graph = canonical_graph();
    let mut engine = GraphTransactionEngine::new();
    let prepared = match engine.prepare_select_tab(&graph, "editor", "B").unwrap() {
        GraphPrepareOutcome::Prepared(prepared) => prepared,
        GraphPrepareOutcome::NoOp(_) => panic!("selection should change"),
    };
    let released_commit_id = prepared.commit_id();

    assert!(engine.release_latest_prepared_slot(prepared));

    let replacement = match engine.prepare_select_tab(&graph, "editor", "B").unwrap() {
        GraphPrepareOutcome::Prepared(prepared) => prepared,
        GraphPrepareOutcome::NoOp(_) => panic!("selection should still change"),
    };
    assert_eq!(replacement.commit_id(), released_commit_id);
}

#[test]
fn capacity_proof_revalidates_remaining_headroom() {
    let mut graph = canonical_graph();
    let mut engine = GraphTransactionEngine::new();
    let prepared = match engine.prepare_select_tab(&graph, "editor", "B").unwrap() {
        GraphPrepareOutcome::Prepared(prepared) => prepared,
        GraphPrepareOutcome::NoOp(_) => panic!("selection should change"),
    };
    let proof = graph
        .try_reserve_for_patch(
            prepared.commit_id(),
            prepared.operation(),
            prepared.mutation(),
        )
        .expect("selection headroom should reserve");

    let group = graph
        .group_mut_for_test(&DockGroupId::from("editor"))
        .expect("editor group should exist");
    while group.mru.len() < group.mru.capacity() {
        group.mru.push(item("capacity-padding"));
    }

    assert_eq!(
        graph.validate_capacity_proof(
            prepared.commit_id(),
            prepared.operation(),
            prepared.mutation(),
            &proof,
        ),
        Err(GraphSupersededReason::AllocatorChanged)
    );
}

#[test]
fn commit_changes_the_target_once_and_replays_the_original_receipt() {
    let graph = canonical_graph();
    let mut target = SelectionTarget::from_graph(&graph);
    let mut engine = GraphTransactionEngine::new();
    let prepared = match engine.prepare_select_tab(&graph, "editor", "B").unwrap() {
        GraphPrepareOutcome::Prepared(prepared) => prepared,
        GraphPrepareOutcome::NoOp(_) => panic!("selection should change"),
    };
    let commit_id = prepared.commit_id();
    let committed = engine.commit(&mut target, prepared);
    let receipt = match committed {
        GraphCommitOutcome::Committed(receipt) => receipt,
        other => panic!("expected committed receipt, got {other:?}"),
    };
    assert_eq!(receipt.commit_id(), commit_id);
    assert_eq!(
        target.groups[&DockGroupId::from("editor")].selected,
        item("B")
    );
    assert_eq!(target.applied, 1);

    let replayed = engine
        .replay(commit_id)
        .expect("receipt should be retained");
    assert_eq!(replayed, GraphCommitOutcome::AlreadyCommitted(receipt));
    assert_eq!(target.applied, 1);
    assert_eq!(engine.receipt_count(), 1);
}

#[test]
fn replay_remains_addressable_after_a_long_commit_history() {
    let graph = canonical_graph();
    let mut target = SelectionTarget::from_graph(&graph);
    let mut engine = GraphTransactionEngine::new();
    let mut first_commit_id = None;
    let mut last_commit_id = None;

    for round in 0..512 {
        let selected = if round % 2 == 0 { "B" } else { "A" };
        let prepared = match engine
            .prepare_select_tab(&target, "editor", selected)
            .unwrap()
        {
            GraphPrepareOutcome::Prepared(prepared) => prepared,
            GraphPrepareOutcome::NoOp(_) => panic!("selection should change"),
        };
        let commit_id = prepared.commit_id();
        if first_commit_id.is_none() {
            first_commit_id = Some(commit_id);
        }
        last_commit_id = Some(commit_id);
        assert!(matches!(
            engine.commit(&mut target, prepared),
            GraphCommitOutcome::Committed(_)
        ));
    }

    assert_eq!(engine.receipt_count(), 512);
    assert!(matches!(
        engine.replay(first_commit_id.expect("first commit should exist")),
        Some(GraphCommitOutcome::AlreadyCommitted(_))
    ));
    assert!(matches!(
        engine.replay(last_commit_id.expect("last commit should exist")),
        Some(GraphCommitOutcome::AlreadyCommitted(_))
    ));
    assert_eq!(target.applied, 512);
}

#[test]
fn abandoned_and_superseded_preparations_leave_inert_receipt_holes() {
    let graph = canonical_graph();
    let mut target = SelectionTarget::from_graph(&graph);
    let mut engine = GraphTransactionEngine::new();

    let abandoned = match engine.prepare_select_tab(&target, "editor", "B").unwrap() {
        GraphPrepareOutcome::Prepared(prepared) => prepared,
        GraphPrepareOutcome::NoOp(_) => panic!("selection should change"),
    };
    let abandoned_id = abandoned.commit_id();
    drop(abandoned);

    let superseded = match engine.prepare_select_tab(&target, "editor", "B").unwrap() {
        GraphPrepareOutcome::Prepared(prepared) => prepared,
        GraphPrepareOutcome::NoOp(_) => panic!("selection should change"),
    };
    let superseded_id = superseded.commit_id();
    target.change_topology();
    assert!(matches!(
        engine.commit(&mut target, superseded),
        GraphCommitOutcome::Superseded(receipt)
            if receipt.reason() == &GraphSupersededReason::TopologyChanged
    ));

    let prepared = match engine.prepare_select_tab(&target, "editor", "B").unwrap() {
        GraphPrepareOutcome::Prepared(prepared) => prepared,
        GraphPrepareOutcome::NoOp(_) => panic!("selection should change"),
    };
    let committed_id = prepared.commit_id();
    let receipt = match engine.commit(&mut target, prepared) {
        GraphCommitOutcome::Committed(receipt) => receipt,
        other => panic!("expected committed receipt, got {other:?}"),
    };

    assert_eq!(abandoned_id.generation(), 1);
    assert_eq!(superseded_id.generation(), 2);
    assert_eq!(committed_id.generation(), 3);
    assert!(engine.replay(abandoned_id).is_none());
    assert!(engine.replay(superseded_id).is_none());
    assert_eq!(
        engine.replay(committed_id),
        Some(GraphCommitOutcome::AlreadyCommitted(receipt))
    );
    assert_eq!(engine.receipt_count(), 1);
    assert_eq!(target.applied, 1);
}

#[test]
fn foreign_transaction_engine_rejects_a_prepared_slot_before_target_write() {
    let graph = canonical_graph();
    let mut target = SelectionTarget::from_graph(&graph);
    let mut source_engine = GraphTransactionEngine::new();
    let mut foreign_engine = GraphTransactionEngine::new();

    let source_prepared = match source_engine
        .prepare_select_tab(&target, "editor", "B")
        .unwrap()
    {
        GraphPrepareOutcome::Prepared(prepared) => prepared,
        GraphPrepareOutcome::NoOp(_) => panic!("selection should change"),
    };
    let foreign_prepared = match foreign_engine
        .prepare_select_tab(&target, "editor", "B")
        .unwrap()
    {
        GraphPrepareOutcome::Prepared(prepared) => prepared,
        GraphPrepareOutcome::NoOp(_) => panic!("selection should change"),
    };

    assert_ne!(source_prepared.commit_id(), foreign_prepared.commit_id());
    assert!(matches!(
        foreign_engine.commit(&mut target, source_prepared),
        GraphCommitOutcome::Superseded(receipt)
            if receipt.reason() == &GraphSupersededReason::TransactionOwnerChanged
    ));
    assert_eq!(target.applied, 0);
    assert_eq!(foreign_engine.receipt_count(), 0);

    assert!(matches!(
        foreign_engine.commit(&mut target, foreign_prepared),
        GraphCommitOutcome::Committed(_)
    ));
    assert_eq!(target.applied, 1);
    assert_eq!(foreign_engine.receipt_count(), 1);
}

#[test]
fn stale_topology_is_rejected_before_the_target_write() {
    let graph = canonical_graph();
    let mut target = SelectionTarget::from_graph(&graph);
    let mut engine = GraphTransactionEngine::new();
    let prepared = match engine.prepare_select_tab(&graph, "editor", "B").unwrap() {
        GraphPrepareOutcome::Prepared(prepared) => prepared,
        GraphPrepareOutcome::NoOp(_) => panic!("selection should change"),
    };
    target.change_topology();

    let result = engine.commit(&mut target, prepared);
    assert!(matches!(
        result,
        GraphCommitOutcome::Superseded(receipt)
            if receipt.reason() == &GraphSupersededReason::TopologyChanged
    ));
    assert_eq!(target.applied, 0);
    assert_eq!(engine.receipt_count(), 0);
}

#[test]
fn replacement_authority_rejects_an_old_commit_even_when_local_revisions_match() {
    let graph = canonical_graph();
    let mut target = SelectionTarget::from_graph(&graph);
    let mut engine = GraphTransactionEngine::new();
    let prepared = match engine.prepare_select_tab(&graph, "editor", "B").unwrap() {
        GraphPrepareOutcome::Prepared(prepared) => prepared,
        GraphPrepareOutcome::NoOp(_) => panic!("selection should change"),
    };
    let commit_id = prepared.commit_id();
    target.replace_authority();

    let result = engine.commit(&mut target, prepared);
    assert!(matches!(
        result,
        GraphCommitOutcome::Superseded(receipt)
            if receipt.commit_id() == commit_id
                && receipt.reason() == &GraphSupersededReason::AuthorityChanged
    ));
    assert_eq!(target.applied, 0);
    assert_eq!(engine.receipt_count(), 0);
}

#[test]
fn unrelated_group_selection_does_not_supersede_a_local_select_tab_patch() {
    let graph = canonical_graph();
    let mut target = SelectionTarget::from_graph(&graph);
    let mut engine = GraphTransactionEngine::new();
    let prepared = match engine.prepare_select_tab(&graph, "editor", "B").unwrap() {
        GraphPrepareOutcome::Prepared(prepared) => prepared,
        GraphPrepareOutcome::NoOp(_) => panic!("selection should change"),
    };
    target.select_unrelated_group(&DockGroupId::from("inspector"), &item("D"));

    assert!(matches!(
        engine.commit(&mut target, prepared),
        GraphCommitOutcome::Committed(_)
    ));
    assert_eq!(
        target.groups[&DockGroupId::from("editor")].selected,
        item("B")
    );
    assert_eq!(
        target.groups[&DockGroupId::from("inspector")].selected,
        item("D")
    );
    assert_eq!(target.applied, 1);
}

#[test]
fn touched_group_selection_change_supersedes_before_mutation() {
    let graph = canonical_graph();
    let mut target = SelectionTarget::from_graph(&graph);
    let mut engine = GraphTransactionEngine::new();
    let prepared = match engine.prepare_select_tab(&graph, "editor", "B").unwrap() {
        GraphPrepareOutcome::Prepared(prepared) => prepared,
        GraphPrepareOutcome::NoOp(_) => panic!("selection should change"),
    };
    target.select_unrelated_group(&DockGroupId::from("editor"), &item("B"));

    let result = engine.commit(&mut target, prepared);
    assert!(matches!(
        result,
        GraphCommitOutcome::Superseded(receipt)
            if receipt.reason() == &GraphSupersededReason::GroupSelectionChanged
    ));
    assert_eq!(target.applied, 0);
}

#[test]
fn selecting_the_current_item_is_a_runtime_selection_action() {
    let graph = canonical_graph();
    let mut target = SelectionTarget::from_graph(&graph);
    let mut engine = GraphTransactionEngine::new();
    let prepared = match engine.prepare_select_tab(&graph, "editor", "A").unwrap() {
        GraphPrepareOutcome::Prepared(prepared) => prepared,
        GraphPrepareOutcome::NoOp(_) => panic!("selecting a tab records a runtime MRU action"),
    };
    let result = engine.commit(&mut target, prepared);
    assert!(matches!(result, GraphCommitOutcome::Committed(_)));
    assert_eq!(
        target.groups[&DockGroupId::from("editor")].selected,
        item("A")
    );
    assert_eq!(
        target.groups[&DockGroupId::from("editor")].mru,
        vec![item("A")]
    );
    assert_eq!(target.applied, 1);
}

#[test]
fn prepared_patch_is_consumed_by_move_and_cannot_be_reused() {
    let graph = canonical_graph();
    let mut target = SelectionTarget::from_graph(&graph);
    let mut engine = GraphTransactionEngine::new();
    let prepared = match engine.prepare_select_tab(&graph, "editor", "B").unwrap() {
        GraphPrepareOutcome::Prepared(prepared) => prepared,
        GraphPrepareOutcome::NoOp(_) => panic!("selection should change"),
    };
    let mut slot = Some(prepared);
    let first = engine.commit(
        &mut target,
        slot.take().expect("prepared patch should exist"),
    );
    assert!(matches!(first, GraphCommitOutcome::Committed(_)));
    assert!(
        slot.is_none(),
        "the linear prepared value must not remain available"
    );
    assert_eq!(target.applied, 1);
}
