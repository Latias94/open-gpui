use crate::{
    DockGraph, DockGraphMutationError, DockItemId, DockNodeId, DockOp, DockPanel,
    DockPanelDescriptor, DockPanelRegistry, DockPolicy, DockSpaceId, DockViewportDropPayload,
    drag::DockDragPayload,
    host::DockHostOptions,
    workspace_drop_transaction::{
        DockWorkspaceDropPayload, DockWorkspaceLockedPayloadDropCommitId,
        DockWorkspaceLockedPayloadDropCommitReceipt, DockWorkspacePayloadDropOutcome,
    },
};
use std::{cell::Cell, collections::HashMap};

/// Owner for one logical docking workspace.
///
/// A workspace coordinates the pure graph, selected dock space, panel registry, host options, and
/// future interaction state. GPUI render adapters such as
/// [`runtime::DockHost`](crate::runtime::DockHost) should render a workspace instead of owning
/// every piece of docking state directly.
#[derive(Debug)]
pub struct DockWorkspace {
    graph: DockGraph,
    graph_revision: u64,
    space: DockSpaceId,
    panels: DockPanelRegistry,
    options: DockHostOptions,
    policy: DockPolicy,
    next_locked_payload_drop_commit_generation: Cell<u64>,
    locked_payload_drop_commits: HashMap<
        DockWorkspaceLockedPayloadDropCommitId,
        DockWorkspaceLockedPayloadDropCommitReceipt,
    >,
    next_graph_commit_generation: Cell<u64>,
    graph_commits: HashMap<DockWorkspaceGraphCommitId, DockWorkspaceGraphCommitReceipt>,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) struct DockWorkspaceGraphCommitId(u64);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct DockWorkspaceGraphCommitReceipt {
    commit_id: DockWorkspaceGraphCommitId,
    graph_revision: u64,
}

#[must_use = "a prepared graph commit must be committed or discarded before retrying"]
pub(crate) struct DockWorkspacePreparedGraphCommit {
    commit_id: DockWorkspaceGraphCommitId,
    expected_revision: u64,
    operation: DockOp,
}

pub(crate) enum DockWorkspaceGraphCommitPreparation {
    Prepared(DockWorkspacePreparedGraphCommit),
    AlreadyCommitted(DockWorkspaceGraphCommitReceipt),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum DockWorkspaceGraphCommitObservation {
    Exact,
    Superseded,
}

impl DockWorkspaceGraphCommitReceipt {
    pub(crate) const fn commit_id(self) -> DockWorkspaceGraphCommitId {
        self.commit_id
    }
}

impl DockWorkspace {
    /// Creates a workspace for one dock space and graph.
    pub fn new(space: impl Into<DockSpaceId>, graph: DockGraph) -> Self {
        Self::with_options(space, graph, DockHostOptions::default())
    }

    /// Creates a workspace with explicit static rendering options.
    pub fn with_options(
        space: impl Into<DockSpaceId>,
        graph: DockGraph,
        options: DockHostOptions,
    ) -> Self {
        Self {
            graph,
            graph_revision: 0,
            space: space.into(),
            panels: DockPanelRegistry::new(),
            options,
            policy: DockPolicy::default(),
            next_locked_payload_drop_commit_generation: Cell::new(0),
            locked_payload_drop_commits: HashMap::new(),
            next_graph_commit_generation: Cell::new(0),
            graph_commits: HashMap::new(),
        }
    }

    pub(crate) fn allocate_graph_commit_id(&self) -> DockWorkspaceGraphCommitId {
        let generation = self
            .next_graph_commit_generation
            .get()
            .checked_add(1)
            .expect("dock workspace graph commit identity space exhausted");
        self.next_graph_commit_generation.set(generation);
        DockWorkspaceGraphCommitId(generation)
    }

    pub(crate) const fn graph_revision(&self) -> u64 {
        self.graph_revision
    }

    pub(crate) fn commit_or_replay_graph(
        &mut self,
        commit_id: DockWorkspaceGraphCommitId,
        expected_revision: u64,
        operation: DockOp,
    ) -> Option<DockWorkspaceGraphCommitReceipt> {
        if let Some(receipt) = self.graph_commits.get(&commit_id) {
            return Some(*receipt);
        }
        if self.graph_revision != expected_revision {
            return None;
        }
        if self.graph.validate_op_checked(&operation).is_err() {
            return None;
        }

        self.apply_op_checked(&operation)
            .expect("a validated graph commit operation must remain applicable");
        let receipt = DockWorkspaceGraphCommitReceipt {
            commit_id,
            graph_revision: self.graph_revision,
        };
        self.graph_commits.insert(commit_id, receipt);
        Some(receipt)
    }

    pub(crate) fn prepare_graph_commit(
        &self,
        commit_id: DockWorkspaceGraphCommitId,
        operation: DockOp,
    ) -> Result<DockWorkspaceGraphCommitPreparation, DockGraphMutationError> {
        if let Some(receipt) = self.graph_commits.get(&commit_id) {
            return Ok(DockWorkspaceGraphCommitPreparation::AlreadyCommitted(
                *receipt,
            ));
        }
        self.graph.validate_op_checked(&operation)?;
        Ok(DockWorkspaceGraphCommitPreparation::Prepared(
            DockWorkspacePreparedGraphCommit {
                commit_id,
                expected_revision: self.graph_revision,
                operation,
            },
        ))
    }

    pub(crate) fn can_commit_prepared_graph(
        &self,
        prepared: &DockWorkspacePreparedGraphCommit,
    ) -> bool {
        self.graph_revision == prepared.expected_revision
            && !self.graph_commits.contains_key(&prepared.commit_id)
    }

    pub(crate) fn commit_prepared_graph(
        &mut self,
        prepared: DockWorkspacePreparedGraphCommit,
    ) -> DockWorkspaceGraphCommitReceipt {
        assert!(
            self.can_commit_prepared_graph(&prepared),
            "a prepared workspace graph commit must remain exact until commit"
        );
        self.apply_op_checked(&prepared.operation)
            .expect("a validated workspace graph operation must remain applicable");
        let receipt = DockWorkspaceGraphCommitReceipt {
            commit_id: prepared.commit_id,
            graph_revision: self.graph_revision,
        };
        self.graph_commits.insert(prepared.commit_id, receipt);
        receipt
    }

    pub(crate) fn commit_prevalidated_graph_operations(&mut self, operations: &[DockOp]) {
        for operation in operations {
            self.apply_op_checked(operation)
                .expect("a prepared graph operation sequence must remain applicable");
        }
    }

    pub(crate) fn graph_commit(
        &self,
        commit_id: DockWorkspaceGraphCommitId,
    ) -> Option<DockWorkspaceGraphCommitReceipt> {
        self.graph_commits.get(&commit_id).copied()
    }

    pub(crate) fn observe_graph_commit(
        &self,
        receipt: DockWorkspaceGraphCommitReceipt,
    ) -> Option<DockWorkspaceGraphCommitObservation> {
        let committed = self.graph_commits.get(&receipt.commit_id())?;
        if *committed != receipt {
            return None;
        }
        Some(if self.graph_revision == receipt.graph_revision {
            DockWorkspaceGraphCommitObservation::Exact
        } else {
            DockWorkspaceGraphCommitObservation::Superseded
        })
    }

    pub(crate) fn retire_graph_commit(&mut self, receipt: DockWorkspaceGraphCommitReceipt) {
        self.graph_commits.remove(&receipt.commit_id());
    }

    pub(crate) fn allocate_locked_payload_drop_commit_id(
        &self,
    ) -> DockWorkspaceLockedPayloadDropCommitId {
        let generation = self
            .next_locked_payload_drop_commit_generation
            .get()
            .checked_add(1)
            .expect("dock workspace drop commit identity space exhausted");
        self.next_locked_payload_drop_commit_generation
            .set(generation);
        DockWorkspaceLockedPayloadDropCommitId::new(generation)
    }

    pub(crate) fn commit_or_replay_locked_payload_drop(
        &mut self,
        commit_id: DockWorkspaceLockedPayloadDropCommitId,
        expected_revision: u64,
        operation: DockOp,
        outcome: DockWorkspacePayloadDropOutcome,
    ) -> Option<DockWorkspaceLockedPayloadDropCommitReceipt> {
        if let Some(receipt) = self.locked_payload_drop_commits.get(&commit_id) {
            return Some(receipt.clone());
        }
        if self.graph_revision != expected_revision {
            return None;
        }

        // Compare the prepared observation before the first authoritative write. A failed
        // observation must never leave a partially committed graph behind.
        let expected_changed = self.graph.validate_op_checked(&operation).ok()?;
        if expected_changed != outcome.changed() {
            return None;
        }

        let changed = self
            .apply_op_checked(&operation)
            .expect("a validated locked payload drop must remain applicable");
        debug_assert_eq!(changed, expected_changed);
        let receipt = DockWorkspaceLockedPayloadDropCommitReceipt::new(commit_id, outcome);
        self.locked_payload_drop_commits
            .insert(commit_id, receipt.clone());
        Some(receipt)
    }

    pub(crate) fn locked_payload_drop_commit(
        &self,
        commit_id: DockWorkspaceLockedPayloadDropCommitId,
    ) -> Option<DockWorkspaceLockedPayloadDropCommitReceipt> {
        self.locked_payload_drop_commits.get(&commit_id).cloned()
    }

    pub(crate) fn retire_locked_payload_drop_commit(
        &mut self,
        receipt: &DockWorkspaceLockedPayloadDropCommitReceipt,
    ) {
        self.locked_payload_drop_commits
            .remove(&receipt.commit_id());
    }

    #[cfg(test)]
    pub(crate) fn locked_payload_drop_commit_count(&self) -> usize {
        self.locked_payload_drop_commits.len()
    }

    /// Returns the logical dock space owned by this workspace.
    pub fn space(&self) -> &DockSpaceId {
        &self.space
    }

    /// Returns the workspace graph.
    pub fn graph(&self) -> &DockGraph {
        &self.graph
    }

    /// Replaces the workspace graph.
    pub fn set_graph(&mut self, graph: DockGraph) {
        self.replace_graph(graph);
    }

    pub(crate) fn apply_op_checked(&mut self, op: &DockOp) -> Result<bool, DockGraphMutationError> {
        let changed = self.graph.apply_op_checked(op)?;
        if changed {
            self.advance_graph_revision();
        }
        Ok(changed)
    }

    pub(crate) fn commit_infallible_graph_mutation(
        &mut self,
        mutation: impl FnOnce(&mut DockGraph) -> bool,
    ) -> bool {
        let changed = mutation(&mut self.graph);
        if changed {
            self.advance_graph_revision();
            debug_assert!(
                self.graph.validate().is_ok(),
                "a prevalidated graph mutation must preserve graph invariants"
            );
        }
        changed
    }

    fn replace_graph(&mut self, graph: DockGraph) {
        self.graph = graph;
        self.advance_graph_revision();
    }

    fn advance_graph_revision(&mut self) {
        self.graph_revision = self
            .graph_revision
            .checked_add(1)
            .expect("dock workspace graph revision space exhausted");
    }

    /// Returns the panel registry.
    pub fn panels(&self) -> &DockPanelRegistry {
        &self.panels
    }

    pub(crate) fn panels_mut(&mut self) -> &mut DockPanelRegistry {
        &mut self.panels
    }

    /// Registers a panel for a dock item, returning any previous registration.
    pub fn register_panel(
        &mut self,
        item: impl Into<DockItemId>,
        panel: DockPanel,
    ) -> Option<DockPanel> {
        self.panels.register(item, panel)
    }

    /// Registers panel metadata without binding GPUI view lifecycle state.
    pub fn register_panel_descriptor(
        &mut self,
        item: impl Into<DockItemId>,
        descriptor: DockPanelDescriptor,
    ) -> Option<DockPanelDescriptor> {
        self.panels.register_descriptor(item, descriptor)
    }

    /// Registers a GPUI view as panel content for a dock item.
    pub fn register_panel_view(
        &mut self,
        item: impl Into<DockItemId>,
        title: impl Into<String>,
        view: impl Into<open_gpui::AnyView>,
    ) -> Option<DockPanel> {
        self.panels
            .register(item, DockPanel::new(title, view.into()))
    }

    /// Registers a focusable GPUI view as panel content for a dock item.
    pub fn register_focusable_panel_view<V>(
        &mut self,
        item: impl Into<DockItemId>,
        title: impl Into<String>,
        view: open_gpui::Entity<V>,
    ) -> Option<DockPanel>
    where
        V: open_gpui::Focusable + open_gpui::Render,
    {
        self.panels
            .register(item, DockPanel::focusable(title, view))
    }

    /// Registers a GPUI view factory as lazy panel content for a dock item.
    pub fn register_panel_factory(
        &mut self,
        item: impl Into<DockItemId>,
        title: impl Into<String>,
        factory: impl Fn(&mut open_gpui::App) -> open_gpui::AnyView + 'static,
    ) -> Option<DockPanel> {
        self.panels.register(item, DockPanel::lazy(title, factory))
    }

    /// Returns the workspace rendering options.
    pub fn options(&self) -> &DockHostOptions {
        &self.options
    }

    /// Returns mutable workspace rendering options.
    pub fn options_mut(&mut self) -> &mut DockHostOptions {
        &mut self.options
    }

    /// Returns the workspace docking interaction policy.
    pub fn policy(&self) -> &DockPolicy {
        &self.policy
    }

    /// Returns mutable workspace docking interaction policy.
    pub fn policy_mut(&mut self) -> &mut DockPolicy {
        &mut self.policy
    }

    /// Replaces the workspace docking interaction policy.
    pub fn set_policy(&mut self, policy: DockPolicy) {
        self.policy = policy;
    }

    pub(crate) fn activation_focus_item_for_workspace_payload(
        &self,
        payload: &DockWorkspaceDropPayload<'_>,
        target_space: Option<&DockSpaceId>,
        frozen_focus_item: Option<&DockItemId>,
    ) -> Option<DockItemId> {
        let item = match payload {
            DockWorkspaceDropPayload::Item { item, .. } => (*item).clone(),
            DockWorkspaceDropPayload::Tabs { .. } | DockWorkspaceDropPayload::Floating { .. } => {
                frozen_focus_item?.clone()
            }
        };

        match target_space {
            Some(space) if self.graph.find_item_in_space(space, &item).is_some() => Some(item),
            Some(_) => None,
            None => Some(item),
        }
    }

    pub(crate) fn activation_focus_item_for_viewport_payload(
        &self,
        payload: &DockViewportDropPayload,
        source_node: DockNodeId,
        frozen_focus_item: Option<&DockItemId>,
    ) -> Option<DockItemId> {
        let workspace_payload = payload.as_workspace_payload(source_node);
        self.resolve_payload_focus_item(&workspace_payload, None, frozen_focus_item)
    }

    pub(crate) fn drag_focus_item_for_payload(
        &self,
        payload: &DockDragPayload,
        recorded_focus_item: Option<&DockItemId>,
    ) -> Option<DockItemId> {
        let workspace_payload = payload.as_workspace_payload();
        self.resolve_payload_focus_item(&workspace_payload, None, recorded_focus_item)
    }

    fn resolve_payload_focus_item(
        &self,
        payload: &DockWorkspaceDropPayload<'_>,
        target_space: Option<&DockSpaceId>,
        frozen_focus_item: Option<&DockItemId>,
    ) -> Option<DockItemId> {
        let item = match payload {
            DockWorkspaceDropPayload::Item { item, .. } => (*item).clone(),
            DockWorkspaceDropPayload::Tabs { .. } | DockWorkspaceDropPayload::Floating { .. } => {
                let item = frozen_focus_item?;
                self.payload_contains_workspace_focus_item(payload, item)
                    .then_some(item.clone())?
            }
        };

        match target_space {
            Some(space) if self.graph.find_item_in_space(space, &item).is_some() => Some(item),
            Some(_) => None,
            None => Some(item),
        }
    }

    fn payload_contains_workspace_focus_item(
        &self,
        payload: &DockWorkspaceDropPayload<'_>,
        item: &DockItemId,
    ) -> bool {
        match payload {
            DockWorkspaceDropPayload::Item {
                source_tabs: _,
                item: payload_item,
            } => *payload_item == item,
            DockWorkspaceDropPayload::Tabs { source_tabs } => self
                .graph
                .collect_items_in_subtree(*source_tabs)
                .iter()
                .any(|candidate| candidate == item),
            DockWorkspaceDropPayload::Floating { floating } => self
                .graph
                .collect_items_in_subtree(*floating)
                .iter()
                .any(|candidate| candidate == item),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph_test_support::{main_space, root_tabs_graph};
    use crate::{DockGraph, DockOp};

    #[test]
    fn graph_commit_replay_does_not_overwrite_a_newer_graph() {
        let (initial, _) = root_tabs_graph(&["initial"]);
        let (newer, _) = root_tabs_graph(&["newer"]);
        let mut workspace = DockWorkspace::new(main_space(), initial.clone());
        let commit_id = workspace.allocate_graph_commit_id();
        let operation = DockOp::CloseItem {
            space: main_space(),
            item: "initial".into(),
        };

        let receipt = workspace
            .commit_or_replay_graph(commit_id, workspace.graph_revision(), operation.clone())
            .expect("the exact initial graph should accept the commit");
        assert!(workspace.graph().root(&main_space()).is_none());

        workspace.set_graph(newer.clone());
        let replay = workspace
            .commit_or_replay_graph(commit_id, workspace.graph_revision(), operation)
            .expect("the same commit id should replay its receipt");

        assert_eq!(replay, receipt);
        assert!(workspace.graph().matches_exactly(&newer));
        assert_eq!(
            workspace.observe_graph_commit(receipt),
            Some(DockWorkspaceGraphCommitObservation::Superseded)
        );
        workspace.retire_graph_commit(receipt);
        assert!(workspace.graph_commit(commit_id).is_none());
    }

    #[test]
    fn graph_commit_observation_rejects_an_equal_shape_after_aba_replacement() {
        let (initial, _) = root_tabs_graph(&["initial"]);
        let (intervening, _) = root_tabs_graph(&["intervening"]);
        let mut workspace = DockWorkspace::new(main_space(), initial.clone());
        let commit_id = workspace.allocate_graph_commit_id();
        let operation = DockOp::CloseItem {
            space: main_space(),
            item: "initial".into(),
        };

        let receipt = workspace
            .commit_or_replay_graph(commit_id, workspace.graph_revision(), operation)
            .expect("the exact initial graph should accept the commit");
        assert_eq!(
            workspace.observe_graph_commit(receipt),
            Some(DockWorkspaceGraphCommitObservation::Exact)
        );

        workspace.set_graph(intervening);
        workspace.set_graph(DockGraph::new());

        assert!(workspace.graph().root(&main_space()).is_none());
        assert_eq!(
            workspace.observe_graph_commit(receipt),
            Some(DockWorkspaceGraphCommitObservation::Superseded),
            "shape equality must not restore superseded transaction authority"
        );
    }

    #[test]
    fn prepared_graph_commit_rejects_an_aba_before_commit() {
        let (initial, _) = root_tabs_graph(&["initial"]);
        let (intermediate, _) = root_tabs_graph(&["intermediate"]);
        let mut workspace = DockWorkspace::new(main_space(), initial.clone());
        let commit_id = workspace.allocate_graph_commit_id();
        let operation = DockOp::CloseItem {
            space: main_space(),
            item: "initial".into(),
        };
        let DockWorkspaceGraphCommitPreparation::Prepared(prepared) = workspace
            .prepare_graph_commit(commit_id, operation)
            .expect("the operation should validate before preparing")
        else {
            panic!("a fresh graph commit must prepare one single-use token");
        };

        workspace.set_graph(intermediate);
        workspace.set_graph(initial);

        assert!(!workspace.can_commit_prepared_graph(&prepared));
    }

    #[test]
    fn unreceipted_graph_replay_requires_the_original_revision() {
        let (initial, _) = root_tabs_graph(&["initial"]);
        let mut workspace = DockWorkspace::new(main_space(), initial.clone());
        let commit_id = workspace.allocate_graph_commit_id();
        let expected_revision = workspace.graph_revision();
        let operation = DockOp::CloseItem {
            space: main_space(),
            item: "initial".into(),
        };

        workspace.set_graph(initial);
        assert!(
            workspace
                .commit_or_replay_graph(commit_id, expected_revision, operation)
                .is_none(),
            "a replay without a receipt must not cross a newer workspace revision"
        );
        assert!(workspace.graph().root(&main_space()).is_some());
    }

    #[test]
    fn prepared_graph_commit_consumes_one_exact_revision_and_replays_its_receipt() {
        let (initial, _) = root_tabs_graph(&["initial"]);
        let mut workspace = DockWorkspace::new(main_space(), initial);
        let commit_id = workspace.allocate_graph_commit_id();
        let operation = DockOp::CloseItem {
            space: main_space(),
            item: "initial".into(),
        };
        let DockWorkspaceGraphCommitPreparation::Prepared(prepared) = workspace
            .prepare_graph_commit(commit_id, operation.clone())
            .expect("the operation should validate before preparing")
        else {
            panic!("a fresh graph commit must prepare one single-use token");
        };

        assert!(workspace.can_commit_prepared_graph(&prepared));
        let receipt = workspace.commit_prepared_graph(prepared);

        assert!(workspace.graph().root(&main_space()).is_none());
        assert_eq!(
            workspace.observe_graph_commit(receipt),
            Some(DockWorkspaceGraphCommitObservation::Exact)
        );
        let DockWorkspaceGraphCommitPreparation::AlreadyCommitted(replayed) = workspace
            .prepare_graph_commit(commit_id, operation)
            .expect("a replay lookup should not fail validation")
        else {
            panic!("the same graph commit identity must replay its exact receipt");
        };
        assert_eq!(replayed, receipt);
    }
}
