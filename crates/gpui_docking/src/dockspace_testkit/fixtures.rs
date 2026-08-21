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
    #[error(
        "dockspace scenario fixture {scenario} has {checkpoints} checkpoints for {steps} steps"
    )]
    CheckpointCount {
        scenario: String,
        checkpoints: usize,
        steps: usize,
    },
    #[error("dockspace scenario fixture {scenario} is invalid: {message}")]
    InvalidScenario { scenario: String, message: String },
    #[error("dockspace scenario fixture {scenario} step {step} is invalid: {message}")]
    InvalidStep {
        scenario: String,
        step: String,
        message: String,
    },
}

pub(crate) fn load_builtin_scenario_fixtures()
-> Result<Vec<DockspaceScenario>, DockspaceScenarioFixtureError> {
    let raw = include_str!("../fixtures/dockspace_scenarios_v1.json");
    load_scenario_fixtures(raw)
}

fn load_scenario_fixtures(
    raw: &str,
) -> Result<Vec<DockspaceScenario>, DockspaceScenarioFixtureError> {
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
        if !scenario.checkpoints.is_empty() && scenario.checkpoints.len() != scenario.steps.len() {
            return Err(DockspaceScenarioFixtureError::CheckpointCount {
                scenario: scenario.id.clone(),
                checkpoints: scenario.checkpoints.len(),
                steps: scenario.steps.len(),
            });
        }
        for (index, (step, checkpoint)) in
            scenario.steps.iter().zip(&scenario.checkpoints).enumerate()
        {
            let Some(step_id) = step.id() else {
                return Err(DockspaceScenarioFixtureError::InvalidStep {
                    scenario: scenario.id.clone(),
                    step: format!("#{index}"),
                    message: "checkpoint requires a stable step id".to_owned(),
                });
            };
            if checkpoint.after_step != step_id {
                return Err(DockspaceScenarioFixtureError::InvalidStep {
                    scenario: scenario.id.clone(),
                    step: step_id.to_owned(),
                    message: format!(
                        "checkpoint after_step {:?} does not match step id {:?}",
                        checkpoint.after_step, step_id
                    ),
                });
            }
            if !checkpoint
                .spaces
                .iter()
                .map(|space| &space.id)
                .eq(scenario.spaces.iter().map(|space| &space.id))
            {
                return Err(DockspaceScenarioFixtureError::InvalidStep {
                    scenario: scenario.id.clone(),
                    step: step_id.to_owned(),
                    message: "checkpoint spaces do not match the scenario space order".to_owned(),
                });
            }
        }

        let mut graph = scenario
            .build_graph()
            .map_err(|error| DockspaceScenarioFixtureError::InvalidScenario {
                scenario: scenario.id.clone(),
                message: error.to_string(),
            })?
            .graph;
        for (index, step) in scenario.steps.iter().enumerate() {
            let step_id = step
                .id()
                .map(str::to_owned)
                .unwrap_or_else(|| format!("#{index}"));
            for adapter in &scenario.adapters {
                if !step.supports_adapter(adapter) {
                    return Err(DockspaceScenarioFixtureError::InvalidStep {
                        scenario: scenario.id.clone(),
                        step: step_id.clone(),
                        message: format!(
                            "step is not supported by the {} adapter",
                            adapter.fixture_name()
                        ),
                    });
                }
            }
            let op =
                step.to_op(&graph)
                    .map_err(|error| DockspaceScenarioFixtureError::InvalidStep {
                        scenario: scenario.id.clone(),
                        step: step_id.clone(),
                        message: error.to_string(),
                    })?;
            graph.apply_op_checked(&op).map_err(|error| {
                DockspaceScenarioFixtureError::InvalidStep {
                    scenario: scenario.id.clone(),
                    step: step_id.clone(),
                    message: error.to_string(),
                }
            })?;
            if let Some(checkpoint) = scenario.checkpoints.get(index) {
                let expected = scenario
                    .build_checkpoint_graph(checkpoint)
                    .map_err(|error| DockspaceScenarioFixtureError::InvalidStep {
                        scenario: scenario.id.clone(),
                        step: step_id.clone(),
                        message: format!("checkpoint topology is invalid: {error}"),
                    })?
                    .graph
                    .export_layout();
                let actual = graph.export_layout();
                if actual != expected {
                    return Err(DockspaceScenarioFixtureError::InvalidStep {
                        scenario: scenario.id.clone(),
                        step: step_id,
                        message: format!(
                            "graph layout does not match the declared checkpoint: expected {expected:?}, actual {actual:?}"
                        ),
                    });
                }
            }
        }
    }
    Ok(suite.scenarios)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};

    fn minimal_fixture_suite() -> Value {
        json!({
            "schema_version": 1,
            "scenarios": [
                {
                    "id": "minimal",
                    "spaces": [
                        {
                            "id": "main",
                            "root": {
                                "kind": "tabs",
                                "items": ["A"],
                                "selected": "A"
                            }
                        }
                    ],
                    "steps": [],
                    "adapters": ["headless"],
                    "oracles": ["graph"],
                    "imgui_categories": []
                }
            ]
        })
    }

    fn load_fixture_value(value: &Value) -> DockspaceScenarioFixtureError {
        load_scenario_fixtures(&value.to_string()).expect_err("fixture should be rejected")
    }

    #[test]
    fn fixture_loader_rejects_suite_contract_violations() {
        let mut unsupported_schema = minimal_fixture_suite();
        unsupported_schema["schema_version"] = json!(2);
        assert!(matches!(
            load_fixture_value(&unsupported_schema),
            DockspaceScenarioFixtureError::UnsupportedSchema(2)
        ));

        let mut empty_suite = minimal_fixture_suite();
        empty_suite["scenarios"] = json!([]);
        assert!(matches!(
            load_fixture_value(&empty_suite),
            DockspaceScenarioFixtureError::EmptySuite
        ));

        let mut missing_adapters = minimal_fixture_suite();
        missing_adapters["scenarios"][0]["adapters"] = json!([]);
        assert!(matches!(
            load_fixture_value(&missing_adapters),
            DockspaceScenarioFixtureError::MissingAdapters(scenario)
                if scenario == "minimal"
        ));

        let mut missing_oracles = minimal_fixture_suite();
        missing_oracles["scenarios"][0]["oracles"] = json!([]);
        assert!(matches!(
            load_fixture_value(&missing_oracles),
            DockspaceScenarioFixtureError::MissingOracles(scenario)
                if scenario == "minimal"
        ));

        let step = json!({
            "id": "move-a",
            "type": "move_item_to_item",
            "source_space": "main",
            "item": "A",
            "target_space": "main",
            "target_item": "A",
            "zone": "center"
        });
        let mut empty_step_id = minimal_fixture_suite();
        let mut empty_id_step = step.clone();
        empty_id_step["id"] = json!("  ");
        empty_step_id["scenarios"][0]["steps"] = json!([empty_id_step]);
        assert!(matches!(
            load_fixture_value(&empty_step_id),
            DockspaceScenarioFixtureError::EmptyStepId(scenario)
                if scenario == "minimal"
        ));

        let mut duplicate_step_id = minimal_fixture_suite();
        duplicate_step_id["scenarios"][0]["steps"] = json!([step.clone(), step]);
        assert!(matches!(
            load_fixture_value(&duplicate_step_id),
            DockspaceScenarioFixtureError::DuplicateStepId { scenario, step }
                if scenario == "minimal" && step == "move-a"
        ));

        let mut unsupported_gpui_host_step = minimal_fixture_suite();
        unsupported_gpui_host_step["scenarios"][0]["adapters"] = json!(["headless", "gpui_host"]);
        unsupported_gpui_host_step["scenarios"][0]["steps"] = json!([{
            "id": "same-space-host-drag",
            "type": "move_item_to_item",
            "source_space": "main",
            "item": "A",
            "target_space": "main",
            "target_item": "A",
            "zone": "center"
        }]);
        assert!(matches!(
            load_fixture_value(&unsupported_gpui_host_step),
            DockspaceScenarioFixtureError::InvalidStep { scenario, step, message }
                if scenario == "minimal"
                    && step == "same-space-host-drag"
                    && message.contains("GPUI host adapter")
        ));

        let mut unsupported_native_step = minimal_fixture_suite();
        unsupported_native_step["scenarios"][0]["adapters"] = json!(["headless", "native"]);
        unsupported_native_step["scenarios"][0]["steps"] = json!([{
            "id": "native-float",
            "type": "float_item_in_window",
            "source_space": "main",
            "item": "A",
            "target_space": "main",
            "bounds": [10.0, 20.0, 200.0, 120.0]
        }]);
        assert!(matches!(
            load_fixture_value(&unsupported_native_step),
            DockspaceScenarioFixtureError::InvalidStep { scenario, step, message }
                if scenario == "minimal"
                    && step == "native-float"
                    && message.contains("native adapter")
        ));
    }

    #[test]
    fn fixture_loader_rejects_checkpoint_contract_violations() {
        let fixture = json!({
            "schema_version": 1,
            "scenarios": [
                {
                    "id": "checkpoint-mismatch",
                    "spaces": [
                        {
                            "id": "source",
                            "root": {
                                "kind": "tabs",
                                "items": ["A"],
                                "selected": "A"
                            }
                        },
                        {
                            "id": "target",
                            "root": {
                                "kind": "tabs",
                                "items": ["B"],
                                "selected": "B"
                            }
                        }
                    ],
                    "steps": [
                        {
                            "id": "move-a",
                            "type": "move_item_to_item",
                            "source_space": "source",
                            "item": "A",
                            "target_space": "target",
                            "target_item": "B",
                            "zone": "center"
                        }
                    ],
                    "checkpoints": [
                        {
                            "after_step": "move-a",
                            "spaces": [
                                { "id": "source" },
                                {
                                    "id": "target",
                                    "root": {
                                        "kind": "tabs",
                                        "items": ["B", "A"],
                                        "selected": "B"
                                    }
                                }
                            ]
                        }
                    ],
                    "adapters": ["headless"],
                    "oracles": ["graph"],
                    "imgui_categories": []
                }
            ]
        });

        assert!(matches!(
            load_fixture_value(&fixture),
            DockspaceScenarioFixtureError::InvalidStep { scenario, step, message }
                if scenario == "checkpoint-mismatch"
                    && step == "move-a"
                    && message.contains("does not match the declared checkpoint")
        ));

        let mut wrong_order = fixture;
        wrong_order["scenarios"][0]["checkpoints"][0]["after_step"] = json!("another-step");
        assert!(matches!(
            load_fixture_value(&wrong_order),
            DockspaceScenarioFixtureError::InvalidStep { scenario, step, message }
                if scenario == "checkpoint-mismatch"
                    && step == "move-a"
                    && message.contains("does not match step id")
        ));

        let mut wrong_count = wrong_order;
        wrong_count["scenarios"][0]["checkpoints"] = json!([
            {
                "after_step": "move-a",
                "spaces": []
            },
            {
                "after_step": "extra",
                "spaces": []
            }
        ]);
        assert!(matches!(
            load_fixture_value(&wrong_count),
            DockspaceScenarioFixtureError::CheckpointCount {
                scenario,
                checkpoints: 2,
                steps: 1
            } if scenario == "checkpoint-mismatch"
        ));
    }

    #[test]
    fn fixture_loader_rejects_a_step_that_references_a_missing_target_item() {
        let error = load_scenario_fixtures(
            r#"{
              "schema_version": 1,
              "scenarios": [
                {
                  "id": "missing-target",
                  "spaces": [
                    {
                      "id": "main",
                      "root": {
                        "kind": "tabs",
                        "items": ["A"],
                        "selected": "A"
                      }
                    }
                  ],
                  "steps": [
                    {
                      "id": "bad-target",
                      "type": "move_item_to_item",
                      "source_space": "main",
                      "item": "A",
                      "target_space": "main",
                      "target_item": "missing",
                      "zone": "center"
                    }
                  ],
                  "adapters": ["headless"],
                  "oracles": ["graph"],
                  "imgui_categories": []
                }
              ]
            }"#,
        )
        .expect_err("missing target items should fail fixture loading");

        assert!(matches!(
            error,
            DockspaceScenarioFixtureError::InvalidStep {
                scenario,
                step,
                ..
            } if scenario == "missing-target" && step == "bad-target"
        ));
    }
}
