use crate::{
    DockFloatingId, DockGroupId, DockItemId, DockLayoutRect, DockSpaceId, SplitAxis,
    canonical_runtime::CanonicalGraphRuntime,
    dockspace_testkit::{
        contract::{
            DockspaceAdapterCapability, DockspaceCommitObservation, DockspaceDragTerminal,
            DockspaceFaultInjection, DockspacePrepareDecision,
        },
        ops::{
            DockspaceScenarioStep, DockspaceScenarioTabPosition, DockspaceScenarioZone,
            DockspaceSemanticOperation,
        },
    },
    graph_assembly::CanonicalNode,
    graph_patch::{
        GraphCommitId, GraphCommitOutcome, GraphPrepareError, GraphPrepareOutcome, GraphRevision,
    },
};
use std::collections::BTreeSet;
use thiserror::Error;

/// Headless runner backed directly by the canonical binary docking kernel.
///
/// The runner deliberately accepts the fixture's semantic operation vocabulary and resolves it
/// to prepared graph patches. It does not maintain a second mutable topology or replay legacy
/// `DockGraph` operations, so the shared fixture remains an independent behavioral oracle.
#[derive(Debug)]
pub(crate) struct CanonicalDockspaceHeadlessRunner {
    scenario_id: String,
    runtime: CanonicalGraphRuntime,
    next_identity: u64,
}

/// Complete transaction evidence emitted by one canonical fixture step.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CanonicalDockspaceStepEvidence {
    pub(crate) prepare: DockspacePrepareDecision,
    pub(crate) commit: Option<DockspaceCommitObservation>,
    pub(crate) terminal: DockspaceDragTerminal,
    pub(crate) receipt_id: Option<GraphCommitId>,
    pub(crate) before: GraphRevision,
    pub(crate) after: GraphRevision,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub(crate) enum CanonicalDockspaceScenarioRunError {
    #[error(
        "canonical scenario {scenario} step {step} requires unsupported capabilities {capabilities:?} or fault injections {faults:?}"
    )]
    UnsupportedStepContract {
        scenario: String,
        step: String,
        capabilities: BTreeSet<DockspaceAdapterCapability>,
        faults: Vec<DockspaceFaultInjection>,
    },
    #[error(
        "canonical scenario {scenario} step {step} cannot resolve item {item} in space {space}"
    )]
    MissingSourceItem {
        scenario: String,
        step: String,
        item: DockItemId,
        space: DockSpaceId,
    },
    #[error(
        "canonical scenario {scenario} step {step} cannot resolve target item {item} in space {space}"
    )]
    MissingTargetItem {
        scenario: String,
        step: String,
        item: DockItemId,
        space: DockSpaceId,
    },
    #[error(
        "canonical scenario {scenario} step {step} cannot resolve floating item {item} in space {space}"
    )]
    MissingFloatingItem {
        scenario: String,
        step: String,
        item: DockItemId,
        space: DockSpaceId,
    },
    #[error(
        "canonical scenario {scenario} step {step} cannot merge split floating payload for item {item}"
    )]
    SplitFloatingCenterUnsupported {
        scenario: String,
        step: String,
        item: DockItemId,
    },
    #[error("canonical scenario {scenario} step {step} could not prepare: {source}")]
    Prepare {
        scenario: String,
        step: String,
        #[source]
        source: GraphPrepareError,
    },
}

impl CanonicalDockspaceHeadlessRunner {
    pub(crate) fn from_scenario(
        scenario: &crate::dockspace_testkit::topology::DockspaceScenario,
    ) -> Result<Self, crate::dockspace_testkit::topology::DockspaceScenarioBuildError> {
        Ok(Self {
            scenario_id: scenario.id.clone(),
            runtime: scenario.build_canonical_runtime()?,
            next_identity: 0,
        })
    }

    pub(crate) fn graph(&self) -> &crate::graph_assembly::CanonicalDockGraph {
        self.runtime.graph()
    }

    pub(crate) fn run_step(
        &mut self,
        step: &DockspaceScenarioStep,
    ) -> Result<CanonicalDockspaceStepEvidence, CanonicalDockspaceScenarioRunError> {
        self.ensure_step_supported(step)?;
        let before = GraphRevision::from_version(self.runtime.graph().version());
        let prepared = self.prepare_step(step)?;
        match prepared {
            GraphPrepareOutcome::NoOp(receipt) => {
                debug_assert_eq!(receipt.revision(), before);
                Ok(CanonicalDockspaceStepEvidence {
                    prepare: DockspacePrepareDecision::NoOp,
                    commit: None,
                    terminal: DockspaceDragTerminal::Unchanged,
                    receipt_id: None,
                    before,
                    after: GraphRevision::from_version(self.runtime.graph().version()),
                })
            }
            GraphPrepareOutcome::Prepared(prepared) => match self.runtime.commit(prepared) {
                GraphCommitOutcome::Committed(receipt) => {
                    debug_assert_eq!(receipt.before(), before);
                    Ok(CanonicalDockspaceStepEvidence {
                        prepare: DockspacePrepareDecision::Prepared,
                        commit: Some(DockspaceCommitObservation::Committed),
                        terminal: DockspaceDragTerminal::Committed,
                        receipt_id: Some(receipt.commit_id()),
                        before,
                        after: receipt.after(),
                    })
                }
                GraphCommitOutcome::AlreadyCommitted(receipt) => {
                    Ok(CanonicalDockspaceStepEvidence {
                        prepare: DockspacePrepareDecision::Prepared,
                        commit: Some(DockspaceCommitObservation::AlreadyCommitted),
                        terminal: DockspaceDragTerminal::Committed,
                        receipt_id: Some(receipt.commit_id()),
                        before,
                        after: GraphRevision::from_version(self.runtime.graph().version()),
                    })
                }
                GraphCommitOutcome::Superseded(receipt) => Ok(CanonicalDockspaceStepEvidence {
                    prepare: DockspacePrepareDecision::Prepared,
                    commit: Some(DockspaceCommitObservation::Superseded),
                    terminal: DockspaceDragTerminal::Superseded,
                    receipt_id: Some(receipt.commit_id()),
                    before,
                    after: receipt.actual_revision(),
                }),
            },
        }
    }

    fn ensure_step_supported(
        &self,
        step: &DockspaceScenarioStep,
    ) -> Result<(), CanonicalDockspaceScenarioRunError> {
        use DockspaceAdapterCapability as Capability;

        let supported = BTreeSet::from([
            Capability::SemanticOperation,
            Capability::MoveItemToItem,
            Capability::MoveItemToTab,
            Capability::MoveItemToEmptySpace,
            Capability::FloatItem,
            Capability::MoveFloating,
            Capability::ItemPayload,
            Capability::WholeGroupPayload,
            Capability::WholeFloatingPayload,
            Capability::CrossViewport,
            Capability::CommitReceipt,
        ]);
        let capabilities = step
            .required_capabilities()
            .difference(&supported)
            .copied()
            .collect::<BTreeSet<_>>();
        if capabilities.is_empty() && step.faults.is_empty() {
            return Ok(());
        }
        Err(
            CanonicalDockspaceScenarioRunError::UnsupportedStepContract {
                scenario: self.scenario_id.clone(),
                step: step.id.clone(),
                capabilities,
                faults: step.faults.clone(),
            },
        )
    }

    fn prepare_step(
        &mut self,
        step: &DockspaceScenarioStep,
    ) -> Result<GraphPrepareOutcome, CanonicalDockspaceScenarioRunError> {
        let result = match &step.operation {
            DockspaceSemanticOperation::MoveItemToItem {
                source_space,
                item,
                target_space,
                target_item,
                zone,
            } => {
                self.require_item(step, source_space, item, true)?;
                let target_group =
                    self.require_item_group(step, target_space, target_item, false)?;
                match zone {
                    DockspaceScenarioZone::Center => self.runtime.prepare_move_item_to_group(
                        item.clone(),
                        target_group,
                        source_space.clone(),
                        target_space.clone(),
                        None,
                    ),
                    zone => {
                        let (axis, insert_before, ratio) = edge_parameters(*zone);
                        let source_group =
                            self.require_item_group(step, source_space, item, true)?;
                        let new_group = self.item_destination_group(step, &source_group, item);
                        self.runtime.prepare_move_item_to_edge(
                            item.clone(),
                            new_group,
                            target_group,
                            source_space.clone(),
                            target_space.clone(),
                            axis,
                            insert_before,
                            ratio,
                        )
                    }
                }
            }
            DockspaceSemanticOperation::MoveGroupToItem {
                source_space,
                group_item,
                target_space,
                target_item,
                zone,
            } => {
                let source_group = self.require_item_group(step, source_space, group_item, true)?;
                let target_group =
                    self.require_item_group(step, target_space, target_item, false)?;
                match zone {
                    DockspaceScenarioZone::Center => self.runtime.prepare_move_group_to_center(
                        source_group,
                        target_group,
                        source_space.clone(),
                        target_space.clone(),
                    ),
                    zone => {
                        let (axis, insert_before, ratio) = edge_parameters(*zone);
                        self.runtime.prepare_move_group_to_edge(
                            source_group,
                            target_group,
                            source_space.clone(),
                            target_space.clone(),
                            axis,
                            insert_before,
                            ratio,
                        )
                    }
                }
            }
            DockspaceSemanticOperation::MoveItemToTab {
                source_space,
                item,
                target_space,
                target_item,
                position,
            } => {
                self.require_item(step, source_space, item, true)?;
                let target_group =
                    self.require_item_group(step, target_space, target_item, false)?;
                let target_state = self
                    .runtime
                    .graph()
                    .group(&target_group)
                    .expect("item group index and group state must agree");
                let target_index = target_state
                    .items
                    .iter()
                    .position(|candidate| candidate == target_item)
                    .expect("item group index and membership must agree");
                let insert_index = match position {
                    DockspaceScenarioTabPosition::Before => target_index,
                    DockspaceScenarioTabPosition::After => target_index.saturating_add(1),
                };
                self.runtime.prepare_move_item_to_group(
                    item.clone(),
                    target_group,
                    source_space.clone(),
                    target_space.clone(),
                    Some(insert_index),
                )
            }
            DockspaceSemanticOperation::MoveItemToEmptySpace {
                source_space,
                item,
                target_space,
            } => {
                self.require_item(step, source_space, item, true)?;
                let source_group = self.require_item_group(step, source_space, item, true)?;
                let new_group = self.item_destination_group(step, &source_group, item);
                self.runtime.prepare_move_item_to_empty_space(
                    item.clone(),
                    source_space.clone(),
                    target_space.clone(),
                    new_group,
                )
            }
            DockspaceSemanticOperation::MoveFloatingToItem {
                source_space,
                floating_item,
                target_space,
                target_item,
                zone,
            } => {
                let floating = self.require_floating(step, source_space, floating_item)?;
                let target_group =
                    self.require_item_group(step, target_space, target_item, false)?;
                match zone {
                    DockspaceScenarioZone::Center => {
                        let root = self
                            .runtime
                            .graph()
                            .floating_roots()
                            .get(&floating)
                            .expect("floating index and floating root must agree")
                            .root;
                        let CanonicalNode::Group { group, .. } = self
                            .runtime
                            .graph()
                            .node(root)
                            .expect("floating root must reference a canonical node")
                        else {
                            return Err(CanonicalDockspaceScenarioRunError::SplitFloatingCenterUnsupported {
                                scenario: self.scenario_id.clone(),
                                step: step.id.clone(),
                                item: floating_item.clone(),
                            });
                        };
                        self.runtime.prepare_move_group_to_center(
                            group.clone(),
                            target_group,
                            source_space.clone(),
                            target_space.clone(),
                        )
                    }
                    zone => {
                        let (axis, insert_before, ratio) = edge_parameters(*zone);
                        self.runtime.prepare_move_floating_root_to_edge(
                            floating,
                            source_space.clone(),
                            target_group,
                            target_space.clone(),
                            axis,
                            insert_before,
                            ratio,
                        )
                    }
                }
            }
            DockspaceSemanticOperation::FloatItemInWindow {
                source_space,
                item,
                target_space,
                bounds,
            } => {
                let source_group = self.require_item_group(step, source_space, item, true)?;
                let new_group = self.item_destination_group(step, &source_group, item);
                let floating = self.next_floating_identity(step, item);
                self.runtime.prepare_float_item_in_window(
                    item.clone(),
                    new_group,
                    floating,
                    source_space.clone(),
                    target_space.clone(),
                    DockLayoutRect {
                        x: bounds[0],
                        y: bounds[1],
                        width: bounds[2],
                        height: bounds[3],
                    },
                )
            }
        };
        result.map_err(|source| CanonicalDockspaceScenarioRunError::Prepare {
            scenario: self.scenario_id.clone(),
            step: step.id.clone(),
            source,
        })
    }

    fn require_item(
        &self,
        step: &DockspaceScenarioStep,
        space: &DockSpaceId,
        item: &DockItemId,
        source: bool,
    ) -> Result<(), CanonicalDockspaceScenarioRunError> {
        let is_in_space = self
            .runtime
            .graph()
            .index()
            .group_for_item(item)
            .and_then(|group| self.runtime.graph().index().node_for_group(group))
            .and_then(|node| self.runtime.graph().index().space_for_node(node))
            == Some(space);
        if is_in_space {
            return Ok(());
        }
        if source {
            Err(CanonicalDockspaceScenarioRunError::MissingSourceItem {
                scenario: self.scenario_id.clone(),
                step: step.id.clone(),
                item: item.clone(),
                space: space.clone(),
            })
        } else {
            Err(CanonicalDockspaceScenarioRunError::MissingTargetItem {
                scenario: self.scenario_id.clone(),
                step: step.id.clone(),
                item: item.clone(),
                space: space.clone(),
            })
        }
    }

    fn require_item_group(
        &self,
        step: &DockspaceScenarioStep,
        space: &DockSpaceId,
        item: &DockItemId,
        source: bool,
    ) -> Result<DockGroupId, CanonicalDockspaceScenarioRunError> {
        self.require_item(step, space, item, source)?;
        Ok(self
            .runtime
            .graph()
            .index()
            .group_for_item(item)
            .expect("validated item must have a group")
            .clone())
    }

    fn require_floating(
        &self,
        step: &DockspaceScenarioStep,
        space: &DockSpaceId,
        item: &DockItemId,
    ) -> Result<DockFloatingId, CanonicalDockspaceScenarioRunError> {
        let Some(floating) = self
            .runtime
            .graph()
            .index()
            .floating_for_item(item)
            .cloned()
        else {
            return Err(CanonicalDockspaceScenarioRunError::MissingFloatingItem {
                scenario: self.scenario_id.clone(),
                step: step.id.clone(),
                item: item.clone(),
                space: space.clone(),
            });
        };
        if self
            .runtime
            .graph()
            .floating_roots()
            .get(&floating)
            .is_none_or(|root| &root.space != space)
        {
            return Err(CanonicalDockspaceScenarioRunError::MissingFloatingItem {
                scenario: self.scenario_id.clone(),
                step: step.id.clone(),
                item: item.clone(),
                space: space.clone(),
            });
        }
        Ok(floating)
    }

    fn item_destination_group(
        &mut self,
        step: &DockspaceScenarioStep,
        source_group: &DockGroupId,
        item: &DockItemId,
    ) -> DockGroupId {
        let state = self
            .runtime
            .graph()
            .group(source_group)
            .expect("source group must be present");
        if state.items.len() == 1 {
            source_group.clone()
        } else {
            loop {
                let generation = self.take_identity_generation();
                let candidate = DockGroupId::new(format!(
                    "fixture:{}:step:{}:item-group:{}:{generation}",
                    self.scenario_id,
                    step.id,
                    item.as_str()
                ));
                if candidate != *source_group && self.runtime.graph().group(&candidate).is_none() {
                    return candidate;
                }
            }
        }
    }

    fn next_floating_identity(
        &mut self,
        step: &DockspaceScenarioStep,
        item: &DockItemId,
    ) -> DockFloatingId {
        loop {
            let generation = self.take_identity_generation();
            let candidate = DockFloatingId::new(format!(
                "fixture:{}:step:{}:floating:{}:{generation}",
                self.scenario_id,
                step.id,
                item.as_str()
            ));
            if !self
                .runtime
                .graph()
                .floating_roots()
                .contains_key(&candidate)
            {
                return candidate;
            }
        }
    }

    fn take_identity_generation(&mut self) -> u64 {
        let generation = self.next_identity;
        self.next_identity = self
            .next_identity
            .checked_add(1)
            .expect("canonical fixture identity space exhausted");
        generation
    }
}

fn edge_parameters(zone: DockspaceScenarioZone) -> (SplitAxis, bool, f32) {
    match zone {
        DockspaceScenarioZone::Left => (SplitAxis::Horizontal, true, 0.5),
        DockspaceScenarioZone::Right => (SplitAxis::Horizontal, false, 0.5),
        DockspaceScenarioZone::Top => (SplitAxis::Vertical, true, 0.5),
        DockspaceScenarioZone::Bottom => (SplitAxis::Vertical, false, 0.5),
        DockspaceScenarioZone::Center => unreachable!("center is not an edge parameter"),
    }
}
