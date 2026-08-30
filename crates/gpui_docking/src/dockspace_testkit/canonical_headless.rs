use crate::{
    DockActionOutcome, DockFloatingId, DockGroupId, DockItemId, DockSpaceId, SplitAxis,
    canonical_runtime::CanonicalGraphRuntime,
    dockspace_testkit::ops::{
        DockspaceScenarioStep, DockspaceScenarioTabPosition, DockspaceScenarioZone,
        DockspaceSemanticOperation,
    },
    graph_assembly::CanonicalNode,
    graph_patch::{
        GraphCommitOutcome, GraphPrepareError, GraphPrepareOutcome, GraphSupersededReason,
    },
};
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
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub(crate) enum CanonicalDockspaceScenarioRunError {
    #[error("canonical scenario {scenario} step {step} does not support operation {operation}")]
    UnsupportedOperation {
        scenario: String,
        step: String,
        operation: &'static str,
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
    #[error("canonical scenario {scenario} step {step} was superseded before commit: {reason:?}")]
    Superseded {
        scenario: String,
        step: String,
        reason: GraphSupersededReason,
    },
}

impl CanonicalDockspaceHeadlessRunner {
    pub(crate) fn from_scenario(
        scenario: &crate::dockspace_testkit::topology::DockspaceScenario,
    ) -> Result<Self, crate::dockspace_testkit::topology::DockspaceScenarioBuildError> {
        Ok(Self {
            scenario_id: scenario.id.clone(),
            runtime: scenario.build_canonical_runtime()?,
        })
    }

    pub(crate) fn graph(&self) -> &crate::graph_assembly::CanonicalDockGraph {
        self.runtime.graph()
    }

    pub(crate) fn run_step(
        &mut self,
        step: &DockspaceScenarioStep,
    ) -> Result<DockActionOutcome, CanonicalDockspaceScenarioRunError> {
        let prepared = self.prepare_step(step)?;
        match prepared {
            GraphPrepareOutcome::NoOp(_) => Ok(DockActionOutcome::Unchanged),
            GraphPrepareOutcome::Prepared(prepared) => match self.runtime.commit(prepared) {
                GraphCommitOutcome::Committed(_) | GraphCommitOutcome::AlreadyCommitted(_) => {
                    Ok(DockActionOutcome::Changed)
                }
                GraphCommitOutcome::Superseded(receipt) => {
                    Err(CanonicalDockspaceScenarioRunError::Superseded {
                        scenario: self.scenario_id.clone(),
                        step: step.id.clone(),
                        reason: receipt.reason().clone(),
                    })
                }
            },
        }
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
            DockspaceSemanticOperation::FloatItemInWindow { .. } => {
                return Err(CanonicalDockspaceScenarioRunError::UnsupportedOperation {
                    scenario: self.scenario_id.clone(),
                    step: step.id.clone(),
                    operation: "float_item_in_window",
                });
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
        &self,
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
            DockGroupId::new(format!(
                "fixture:{}:step:{}:item-group:{}",
                self.scenario_id,
                step.id,
                item.as_str()
            ))
        }
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
