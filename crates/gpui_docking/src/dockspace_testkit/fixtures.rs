use crate::dockspace_testkit::topology::DockspaceScenario;
use serde::Deserialize;
use std::collections::BTreeSet;
use thiserror::Error;

#[derive(Debug, Deserialize)]
struct DockspaceScenarioFixtureSuite {
    schema_version: u32,
    scenarios: Vec<DockspaceScenario>,
}

#[derive(Debug, Error)]
pub(crate) enum DockspaceScenarioFixtureError {
    #[error(transparent)]
    Parse(#[from] serde_json::Error),
    #[error("unsupported dockspace scenario fixture schema version {0}")]
    UnsupportedSchema(u32),
    #[error("dockspace scenario fixture contains no scenarios")]
    EmptySuite,
    #[error("dockspace scenario fixture {0} has no adapters")]
    MissingAdapters(String),
    #[error("dockspace scenario fixture {0} has no oracles")]
    MissingOracles(String),
    #[error("dockspace scenario fixture {0} has an empty step id")]
    EmptyStepId(String),
    #[error("dockspace scenario fixture {scenario} repeats step id {step}")]
    DuplicateStepId { scenario: String, step: String },
}

pub(crate) fn load_builtin_scenario_fixtures()
-> Result<Vec<DockspaceScenario>, DockspaceScenarioFixtureError> {
    let raw = include_str!("../fixtures/dockspace_scenarios_v1.json");
    let suite: DockspaceScenarioFixtureSuite = serde_json::from_str(raw)?;
    if suite.schema_version != 1 {
        return Err(DockspaceScenarioFixtureError::UnsupportedSchema(
            suite.schema_version,
        ));
    }
    if suite.scenarios.is_empty() {
        return Err(DockspaceScenarioFixtureError::EmptySuite);
    }
    for scenario in &suite.scenarios {
        if scenario.adapters.is_empty() {
            return Err(DockspaceScenarioFixtureError::MissingAdapters(
                scenario.id.clone(),
            ));
        }
        if scenario.oracles.is_empty() {
            return Err(DockspaceScenarioFixtureError::MissingOracles(
                scenario.id.clone(),
            ));
        }
        let mut step_ids = BTreeSet::new();
        for step in &scenario.steps {
            let Some(step_id) = step.id() else {
                continue;
            };
            let step_id = step_id.trim();
            if step_id.is_empty() {
                return Err(DockspaceScenarioFixtureError::EmptyStepId(
                    scenario.id.clone(),
                ));
            }
            if !step_ids.insert(step_id.to_owned()) {
                return Err(DockspaceScenarioFixtureError::DuplicateStepId {
                    scenario: scenario.id.clone(),
                    step: step_id.to_owned(),
                });
            }
        }
    }
    Ok(suite.scenarios)
}
