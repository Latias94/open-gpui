use crate::{
    DockActionOutcome, DockGraph, DockGraphMutationError, DockSpaceId, DockWorkspace,
    dockspace_testkit::{
        ops::{DockspaceScenarioStep, DockspaceStepResolveError},
        topology::{DockspaceScenario, DockspaceScenarioBuildError},
    },
};
use thiserror::Error;

#[derive(Debug)]
pub(crate) struct DockspaceHeadlessRunner {
    workspace: DockWorkspace,
}

#[derive(Debug, Clone, PartialEq, Error)]
pub(crate) enum DockspaceScenarioRunError {
    #[error(transparent)]
    Build(#[from] DockspaceScenarioBuildError),
    #[error(transparent)]
    Resolve(#[from] DockspaceStepResolveError),
    #[error(transparent)]
    Graph(#[from] DockGraphMutationError),
}

impl DockspaceHeadlessRunner {
    pub(crate) fn from_scenario(
        scenario: &DockspaceScenario,
        primary_space: impl Into<DockSpaceId>,
    ) -> Result<Self, DockspaceScenarioRunError> {
        let scenario_graph = scenario.build_graph()?;
        Ok(Self::from_graph(primary_space, scenario_graph.graph))
    }

    pub(crate) fn from_graph(primary_space: impl Into<DockSpaceId>, graph: DockGraph) -> Self {
        Self {
            workspace: DockWorkspace::new(primary_space, graph),
        }
    }

    pub(crate) fn graph(&self) -> &DockGraph {
        self.workspace.graph()
    }

    pub(crate) fn run_step(
        &mut self,
        step: &DockspaceScenarioStep,
    ) -> Result<DockActionOutcome, DockspaceScenarioRunError> {
        let op = step.to_op(self.workspace.graph())?;
        let changed = self.workspace.apply_op_checked(&op)?;
        Ok(if changed {
            DockActionOutcome::Changed
        } else {
            DockActionOutcome::Unchanged
        })
    }

    pub(crate) fn run_all(
        &mut self,
        steps: &[DockspaceScenarioStep],
    ) -> Result<Vec<DockActionOutcome>, DockspaceScenarioRunError> {
        steps.iter().map(|step| self.run_step(step)).collect()
    }
}
