use crate::graph_assembly::{CanonicalDockGraph, DockGraphAssembly};
use crate::graph_patch::{
    GraphCommitOutcome, GraphGroupRead, GraphPatchOperation, GraphPatchTarget, GraphPrepareOutcome,
    GraphReadView, GraphRevision, GraphSupersededReason,
};
use crate::graph_transaction::GraphTransactionEngine;
use crate::ids::CanonicalGraphAuthority;
use crate::{DockGroupId, DockItemId, DockSpaceId};
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
        .add_branch(crate::SplitAxis::Horizontal, editor, inspector, 0.5)
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
        self.revision = self.revision.with_selection(self.revision.selection() + 1);
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
    fn apply_select_tab(&mut self, group: &DockGroupId, item: &DockItemId) -> GraphRevision {
        let Some(group_state) = self.groups.get_mut(group) else {
            return self.revision;
        };
        group_state.selected = item.clone();
        group_state.mru.retain(|entry| entry != item);
        group_state.mru.insert(0, item.clone());
        self.applied += 1;
        self.revision = self.revision.with_selection(self.revision.selection() + 1);
        self.revision
    }

    fn apply_split_group(
        &mut self,
        _target_group: &DockGroupId,
        _new_group: &DockGroupId,
        _items: &[DockItemId],
        _selected: &DockItemId,
        _axis: crate::SplitAxis,
        _insert_before: bool,
        _ratio: f32,
        _space: &crate::DockSpaceId,
    ) -> GraphRevision {
        panic!("selection target does not support structural patches")
    }

    fn apply_move_group_to_edge(
        &mut self,
        _source_group: &DockGroupId,
        _target_group: &DockGroupId,
        _source_space: &DockSpaceId,
        _target_space: &DockSpaceId,
        _axis: crate::SplitAxis,
        _insert_before: bool,
        _ratio: f32,
    ) -> GraphRevision {
        panic!("selection target does not support structural patches")
    }

    fn apply_move_group_to_center(
        &mut self,
        _source_group: &DockGroupId,
        _target_group: &DockGroupId,
        _source_space: &DockSpaceId,
        _target_space: &DockSpaceId,
    ) -> GraphRevision {
        panic!("selection target does not support structural patches")
    }

    fn apply_move_group_to_empty_space(
        &mut self,
        _source_group: &DockGroupId,
        _source_space: &DockSpaceId,
        _target_space: &DockSpaceId,
    ) -> GraphRevision {
        panic!("selection target does not support structural patches")
    }

    fn apply_move_item_to_group(
        &mut self,
        _item: &DockItemId,
        _source_group: &DockGroupId,
        _target_group: &DockGroupId,
        _source_space: &DockSpaceId,
        _target_space: &DockSpaceId,
        _insert_index: Option<usize>,
    ) -> GraphRevision {
        panic!("selection target does not support structural patches")
    }

    fn apply_move_floating_root_to_edge(
        &mut self,
        _floating: &crate::DockFloatingId,
        _source_space: &DockSpaceId,
        _target_group: &DockGroupId,
        _target_space: &DockSpaceId,
        _axis: crate::SplitAxis,
        _insert_before: bool,
        _ratio: f32,
    ) -> GraphRevision {
        panic!("selection target does not support structural patches")
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
        prepared.read_set().group().group,
        DockGroupId::from("editor")
    );
    assert_eq!(
        prepared.read_set().group().items,
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
