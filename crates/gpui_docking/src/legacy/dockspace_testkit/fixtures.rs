use crate::legacy::dockspace_testkit::{
    contract::{
        DockspaceAdapterCapability, DockspaceAdapterTag, DockspaceCommitObservation,
        DockspaceDragTerminal, DockspacePrepareDecision, DockspaceReceiptExpectation,
        DockspaceStepExpectation,
    },
    ops::{DockspaceInteractionAction, DockspaceInteractionScript, DockspaceScenarioStep},
    topology::{DockspaceCheckpointPhase, DockspaceScenario},
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

const DOCKSPACE_SCENARIO_FIXTURE_SCHEMA_VERSION: u32 = 2;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
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
    #[error("dockspace scenario fixture repeats scenario id {0}")]
    DuplicateScenario(String),
    #[error("dockspace scenario fixture {0} has no adapters")]
    MissingAdapters(String),
    #[error("dockspace scenario fixture {0} has no oracles")]
    MissingOracles(String),
    #[error("dockspace scenario fixture {0} has an empty step id")]
    EmptyStepId(String),
    #[error("dockspace scenario fixture {scenario} repeats step id {step}")]
    DuplicateStepId { scenario: String, step: String },
    #[error("dockspace scenario fixture {scenario} repeats adapter contract {adapter}")]
    DuplicateAdapter { scenario: String, adapter: String },
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
    let raw = include_str!("../fixtures/dockspace_scenarios_v2.json");
    load_scenario_fixtures(raw)
}

fn load_scenario_fixtures(
    raw: &str,
) -> Result<Vec<DockspaceScenario>, DockspaceScenarioFixtureError> {
    let suite: DockspaceScenarioFixtureSuite = serde_json::from_str(raw)?;
    if suite.schema_version != DOCKSPACE_SCENARIO_FIXTURE_SCHEMA_VERSION {
        return Err(DockspaceScenarioFixtureError::UnsupportedSchema(
            suite.schema_version,
        ));
    }
    if suite.scenarios.is_empty() {
        return Err(DockspaceScenarioFixtureError::EmptySuite);
    }
    let mut scenario_ids = BTreeSet::new();
    for scenario in &suite.scenarios {
        if !scenario_ids.insert(scenario.id.clone()) {
            return Err(DockspaceScenarioFixtureError::DuplicateScenario(
                scenario.id.clone(),
            ));
        }
        if scenario.adapter_contracts.is_empty() {
            return Err(DockspaceScenarioFixtureError::MissingAdapters(
                scenario.id.clone(),
            ));
        }
        if scenario.oracles().is_empty() {
            return Err(DockspaceScenarioFixtureError::MissingOracles(
                scenario.id.clone(),
            ));
        }
        validate_adapter_contracts(scenario)?;
        let declared_spaces = scenario
            .spaces
            .iter()
            .map(|space| space.id.clone())
            .collect::<BTreeSet<_>>();
        let declared_items = scenario.declared_items();
        let mut step_ids = BTreeSet::new();
        let mut receipt_aliases = BTreeSet::new();
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
            validate_step_contract(scenario, step, &receipt_aliases)?;
            step.validate_fixture_references(&declared_spaces, &declared_items)
                .map_err(|message| DockspaceScenarioFixtureError::InvalidStep {
                    scenario: scenario.id.clone(),
                    step: step_id.to_owned(),
                    message,
                })?;
            if let DockspaceReceiptExpectation::New { alias } = &step.expected.receipt {
                receipt_aliases.insert(alias.clone());
            }
        }
        let steps_by_id = scenario
            .steps
            .iter()
            .enumerate()
            .map(|(index, step)| (step.id.as_str(), (index, step)))
            .collect::<BTreeMap<_, _>>();
        let mut seen_checkpoint_phases = BTreeSet::new();
        let mut previous_checkpoint_order = None;
        for checkpoint in &scenario.checkpoints {
            let Some((step_index, step)) = steps_by_id.get(checkpoint.after_step.as_str()) else {
                return Err(DockspaceScenarioFixtureError::InvalidStep {
                    scenario: scenario.id.clone(),
                    step: checkpoint.after_step.clone(),
                    message: "checkpoint references an unknown step".to_owned(),
                });
            };
            if !seen_checkpoint_phases.insert((checkpoint.after_step.clone(), checkpoint.phase)) {
                return Err(DockspaceScenarioFixtureError::InvalidStep {
                    scenario: scenario.id.clone(),
                    step: checkpoint.after_step.clone(),
                    message: format!("checkpoint repeats phase {:?}", checkpoint.phase),
                });
            }
            let order = (*step_index, checkpoint.phase);
            if previous_checkpoint_order.is_some_and(|previous| previous > order) {
                return Err(DockspaceScenarioFixtureError::InvalidStep {
                    scenario: scenario.id.clone(),
                    step: checkpoint.after_step.clone(),
                    message: "checkpoints are not ordered by step and phase".to_owned(),
                });
            }
            previous_checkpoint_order = Some(order);
            if !checkpoint
                .spaces
                .iter()
                .map(|space| &space.id)
                .eq(scenario.spaces.iter().map(|space| &space.id))
            {
                return Err(DockspaceScenarioFixtureError::InvalidStep {
                    scenario: scenario.id.clone(),
                    step: checkpoint.after_step.clone(),
                    message: "checkpoint spaces do not match the scenario space order".to_owned(),
                });
            }
            validate_checkpoint_contract(step, checkpoint).map_err(|message| {
                DockspaceScenarioFixtureError::InvalidStep {
                    scenario: scenario.id.clone(),
                    step: step.id.clone(),
                    message,
                }
            })?;
        }

        scenario
            .build_graph()
            .map_err(|error| DockspaceScenarioFixtureError::InvalidScenario {
                scenario: scenario.id.clone(),
                message: error.to_string(),
            })?;
        scenario.terminal_checkpoints().map_err(|error| {
            DockspaceScenarioFixtureError::InvalidScenario {
                scenario: scenario.id.clone(),
                message: error.to_string(),
            }
        })?;
        for checkpoint in &scenario.checkpoints {
            scenario
                .build_checkpoint_graph(checkpoint)
                .map_err(|error| DockspaceScenarioFixtureError::InvalidStep {
                    scenario: scenario.id.clone(),
                    step: checkpoint.after_step.clone(),
                    message: format!("checkpoint topology is invalid: {error}"),
                })?;
        }
    }
    Ok(suite.scenarios)
}

fn validate_checkpoint_contract(
    step: &DockspaceScenarioStep,
    checkpoint: &crate::legacy::dockspace_testkit::topology::DockspaceScenarioCheckpoint,
) -> Result<(), String> {
    use crate::legacy::dockspace_testkit::contract::DockspaceProgressState;

    match checkpoint.phase {
        DockspaceCheckpointPhase::AfterPrepare => {
            if checkpoint.facts.terminal.is_some() {
                return Err("after_prepare checkpoint cannot declare a terminal".to_owned());
            }
        }
        DockspaceCheckpointPhase::AfterProviderReceipt => {
            if checkpoint.facts.progress != Some(DockspaceProgressState::ForwardOnlyPrepared) {
                return Err(
                    "after_provider_receipt checkpoint must be forward_only_prepared".to_owned(),
                );
            }
            if checkpoint.facts.terminal.is_some() {
                return Err(
                    "after_provider_receipt checkpoint cannot declare a terminal".to_owned(),
                );
            }
        }
        DockspaceCheckpointPhase::AfterRecoveryTransfer => {
            if checkpoint.facts.progress != Some(DockspaceProgressState::RecoveryPending) {
                return Err(
                    "after_recovery_transfer checkpoint must be recovery_pending".to_owned(),
                );
            }
            if checkpoint.facts.terminal.is_some() {
                return Err(
                    "after_recovery_transfer checkpoint cannot declare a terminal".to_owned(),
                );
            }
        }
        DockspaceCheckpointPhase::Terminal => {
            if checkpoint.facts.progress.is_some() {
                return Err("terminal checkpoint cannot retain non-terminal progress".to_owned());
            }
            if checkpoint.facts.terminal != Some(step.expected.terminal) {
                return Err(format!(
                    "terminal checkpoint must declare {:?}",
                    step.expected.terminal
                ));
            }
            if checkpoint.facts.receipt.as_ref() != Some(&step.expected.receipt) {
                return Err(
                    "terminal checkpoint receipt must match the step expectation".to_owned(),
                );
            }
        }
        DockspaceCheckpointPhase::AfterTeardown => {
            if checkpoint.facts.progress.is_some() {
                return Err("after_teardown checkpoint cannot retain pending progress".to_owned());
            }
        }
    }
    Ok(())
}

fn validate_adapter_contracts(
    scenario: &DockspaceScenario,
) -> Result<(), DockspaceScenarioFixtureError> {
    let mut adapters = BTreeSet::new();
    for contract in &scenario.adapter_contracts {
        if !adapters.insert(contract.adapter) {
            return Err(DockspaceScenarioFixtureError::DuplicateAdapter {
                scenario: scenario.id.clone(),
                adapter: contract.adapter.fixture_name().to_owned(),
            });
        }
        if !contract
            .required_capabilities
            .is_disjoint(&contract.optional_capabilities)
        {
            return Err(DockspaceScenarioFixtureError::InvalidScenario {
                scenario: scenario.id.clone(),
                message: format!(
                    "{} adapter has overlapping required and optional capabilities",
                    contract.adapter.fixture_name()
                ),
            });
        }
        if !contract
            .required_oracles
            .is_disjoint(&contract.optional_oracles)
        {
            return Err(DockspaceScenarioFixtureError::InvalidScenario {
                scenario: scenario.id.clone(),
                message: format!(
                    "{} adapter has overlapping required and optional oracles",
                    contract.adapter.fixture_name()
                ),
            });
        }
        if contract.required_oracles.is_empty() {
            return Err(DockspaceScenarioFixtureError::InvalidScenario {
                scenario: scenario.id.clone(),
                message: format!(
                    "{} adapter has no required oracle",
                    contract.adapter.fixture_name()
                ),
            });
        }
        for step in &scenario.steps {
            if let crate::legacy::dockspace_testkit::contract::DockspaceAdapterStepSupport::ContractMissing {
                capabilities,
            } = contract.step_support(&step.required_capabilities())
            {
                return Err(DockspaceScenarioFixtureError::InvalidScenario {
                    scenario: scenario.id.clone(),
                    message: format!(
                        "{} adapter contract does not declare step {} capabilities {capabilities:?}",
                        contract.adapter.fixture_name(),
                        step.id
                    ),
                });
            }
        }
        let supported = contract.adapter.supported_capabilities();
        if !contract.required_capabilities.is_subset(&supported) {
            let missing = contract
                .required_capabilities
                .difference(&supported)
                .copied()
                .collect::<Vec<_>>();
            return Err(DockspaceScenarioFixtureError::InvalidScenario {
                scenario: scenario.id.clone(),
                message: format!(
                    "{} adapter does not implement required capabilities {missing:?}",
                    contract.adapter.fixture_name()
                ),
            });
        }
        if matches!(
            contract.adapter,
            DockspaceAdapterTag::GpuiHost | DockspaceAdapterTag::Native
        ) && !contract
            .required_capabilities
            .contains(&DockspaceAdapterCapability::InteractionScript)
        {
            return Err(DockspaceScenarioFixtureError::InvalidScenario {
                scenario: scenario.id.clone(),
                message: format!(
                    "{} adapter must require interaction_script",
                    contract.adapter.fixture_name()
                ),
            });
        }
    }
    Ok(())
}

fn validate_step_contract(
    scenario: &DockspaceScenario,
    step: &DockspaceScenarioStep,
    receipt_aliases: &BTreeSet<String>,
) -> Result<(), DockspaceScenarioFixtureError> {
    let invalid = |message: String| DockspaceScenarioFixtureError::InvalidStep {
        scenario: scenario.id.clone(),
        step: step.id.clone(),
        message,
    };

    validate_step_expectation(&step.expected, receipt_aliases).map_err(invalid)?;
    if let Some(interaction) = &step.interaction {
        validate_interaction_script(interaction).map_err(invalid)?;
    }
    if scenario.adapter_contracts.iter().any(|contract| {
        matches!(
            contract.adapter,
            DockspaceAdapterTag::GpuiHost | DockspaceAdapterTag::Native
        )
    }) && step.interaction.is_none()
    {
        return Err(invalid(
            "interactive adapter step requires an interaction script".to_owned(),
        ));
    }
    Ok(())
}

fn validate_step_expectation(
    expected: &DockspaceStepExpectation,
    receipt_aliases: &BTreeSet<String>,
) -> Result<(), String> {
    use DockspaceCommitObservation as Commit;
    use DockspaceDragTerminal as Terminal;
    use DockspacePrepareDecision as Prepare;
    use DockspaceReceiptExpectation as Receipt;

    let valid = match (
        &expected.prepare,
        &expected.commit,
        &expected.terminal,
        &expected.receipt,
    ) {
        (
            Prepare::Prepared,
            Some(Commit::Committed),
            Terminal::Committed,
            Receipt::New { alias },
        ) => !alias.trim().is_empty() && !receipt_aliases.contains(alias),
        (
            Prepare::Prepared,
            Some(Commit::AlreadyCommitted),
            Terminal::Committed,
            Receipt::SameAs { alias },
        ) => receipt_aliases.contains(alias),
        (Prepare::Prepared, Some(Commit::Superseded), Terminal::Superseded, Receipt::None)
        | (Prepare::Superseded, None, Terminal::Superseded, Receipt::None)
        | (Prepare::Cancelled, None, Terminal::Cancelled, Receipt::None)
        | (Prepare::NoOp, None, Terminal::Unchanged, Receipt::None) => true,
        (
            Prepare::Prepared,
            Some(Commit::Committed),
            Terminal::Recovered,
            Receipt::New { alias },
        ) => !alias.trim().is_empty() && !receipt_aliases.contains(alias),
        _ => false,
    };
    if valid {
        Ok(())
    } else {
        Err(format!(
            "invalid prepare/commit/terminal/receipt combination: {expected:?}"
        ))
    }
}

fn validate_interaction_script(script: &DockspaceInteractionScript) -> Result<(), String> {
    let Some(first) = script.actions.first() else {
        return Err("interaction script has no actions".to_owned());
    };
    if *first != DockspaceInteractionAction::BeginDrag {
        return Err("interaction script must begin with begin_drag".to_owned());
    }
    let terminal_count = script
        .actions
        .iter()
        .filter(|action| {
            matches!(
                action,
                DockspaceInteractionAction::Release | DockspaceInteractionAction::Cancel
            )
        })
        .count();
    if terminal_count != 1
        || !matches!(
            script.actions.last(),
            Some(DockspaceInteractionAction::Release | DockspaceInteractionAction::Cancel)
        )
    {
        return Err(
            "interaction script must end with exactly one release or cancel action".to_owned(),
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};

    fn headless_contract(capabilities: Value) -> Value {
        json!({
            "adapter": "headless",
            "required_capabilities": capabilities,
            "optional_capabilities": [
                "retained_subtree_payload",
                "provider_first_reservation",
                "recovery",
                "shutdown_drain"
            ],
            "required_oracles": ["graph"]
        })
    }

    fn committed_step(id: &str, operation: Value) -> Value {
        json!({
            "id": id,
            "operation": operation,
            "expected": {
                "prepare": "prepared",
                "commit": "committed",
                "terminal": "committed",
                "receipt": { "kind": "new", "alias": id }
            }
        })
    }

    fn minimal_fixture_suite() -> Value {
        json!({
            "schema_version": 2,
            "scenarios": [
                {
                    "id": "minimal",
                    "phases": {
                        "fixture": { "arrangement": "direct_deterministic" },
                        "harness": { "ready": "canonical_graph_published" },
                        "warmup": { "iterations": 0 },
                        "teardown": {
                            "require_canonical_graph": true,
                            "require_no_pending_progress": true
                        }
                    },
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
                    "adapter_contracts": [
                        headless_contract(json!(["semantic_operation"]))
                    ],
                    "imgui_categories": []
                }
            ]
        })
    }

    fn load_fixture_value(value: &Value) -> DockspaceScenarioFixtureError {
        load_scenario_fixtures(&value.to_string()).expect_err("fixture should be rejected")
    }

    #[test]
    fn fixture_loader_accepts_schema_v2_and_rejects_v1() {
        let scenarios = load_scenario_fixtures(&minimal_fixture_suite().to_string())
            .expect("fixture schema v2 should be authoritative");
        let suite = DockspaceScenarioFixtureSuite {
            schema_version: DOCKSPACE_SCENARIO_FIXTURE_SCHEMA_VERSION,
            scenarios,
        };
        let encoded = serde_json::to_string(&suite).expect("fixture v2 should serialize");
        let decoded: DockspaceScenarioFixtureSuite =
            serde_json::from_str(&encoded).expect("serialized fixture v2 should parse");
        assert_eq!(decoded, suite, "fixture v2 should round-trip without drift");

        let mut schema_v1 = minimal_fixture_suite();
        schema_v1["schema_version"] = json!(1);
        assert!(matches!(
            load_fixture_value(&schema_v1),
            DockspaceScenarioFixtureError::UnsupportedSchema(1)
        ));

        let mut unknown_field = minimal_fixture_suite();
        unknown_field["scenarios"][0]["legacy_adapters"] = json!(["headless"]);
        assert!(matches!(
            load_fixture_value(&unknown_field),
            DockspaceScenarioFixtureError::Parse(_)
        ));
    }

    #[test]
    fn fixture_loader_rejects_suite_contract_violations() {
        let mut unsupported_schema = minimal_fixture_suite();
        unsupported_schema["schema_version"] = json!(3);
        assert!(matches!(
            load_fixture_value(&unsupported_schema),
            DockspaceScenarioFixtureError::UnsupportedSchema(3)
        ));

        let mut empty_suite = minimal_fixture_suite();
        empty_suite["scenarios"] = json!([]);
        assert!(matches!(
            load_fixture_value(&empty_suite),
            DockspaceScenarioFixtureError::EmptySuite
        ));

        let mut missing_adapters = minimal_fixture_suite();
        missing_adapters["scenarios"][0]["adapter_contracts"] = json!([]);
        assert!(matches!(
            load_fixture_value(&missing_adapters),
            DockspaceScenarioFixtureError::MissingAdapters(scenario)
                if scenario == "minimal"
        ));

        let mut missing_oracles = minimal_fixture_suite();
        missing_oracles["scenarios"][0]["adapter_contracts"][0]["required_oracles"] = json!([]);
        assert!(matches!(
            load_fixture_value(&missing_oracles),
            DockspaceScenarioFixtureError::MissingOracles(scenario)
                if scenario == "minimal"
        ));

        let operation = json!({
            "type": "move_item_to_item",
            "source_space": "main",
            "item": "A",
            "target_space": "main",
            "target_item": "A",
            "zone": "center"
        });
        let step = committed_step("move-a", operation);
        let mut empty_step_id = minimal_fixture_suite();
        empty_step_id["scenarios"][0]["adapter_contracts"][0]["required_capabilities"] =
            json!(["semantic_operation", "move_item_to_item", "item_payload"]);
        let mut empty_id_step = step.clone();
        empty_id_step["id"] = json!("  ");
        empty_step_id["scenarios"][0]["steps"] = json!([empty_id_step]);
        assert!(matches!(
            load_fixture_value(&empty_step_id),
            DockspaceScenarioFixtureError::EmptyStepId(scenario)
                if scenario == "minimal"
        ));

        let mut duplicate_step_id = minimal_fixture_suite();
        duplicate_step_id["scenarios"][0]["adapter_contracts"][0]["required_capabilities"] =
            json!(["semantic_operation", "move_item_to_item", "item_payload"]);
        duplicate_step_id["scenarios"][0]["steps"] = json!([step.clone(), step]);
        assert!(matches!(
            load_fixture_value(&duplicate_step_id),
            DockspaceScenarioFixtureError::DuplicateStepId { scenario, step }
                if scenario == "minimal" && step == "move-a"
        ));
    }

    #[test]
    fn fixture_loader_enforces_adapter_capability_contracts() {
        let mut overlap = minimal_fixture_suite();
        overlap["scenarios"][0]["adapter_contracts"][0]["optional_capabilities"] =
            json!(["semantic_operation"]);
        assert!(matches!(
            load_fixture_value(&overlap),
            DockspaceScenarioFixtureError::InvalidScenario { scenario, message }
                if scenario == "minimal" && message.contains("overlapping")
        ));

        let mut unsupported_required = minimal_fixture_suite();
        unsupported_required["scenarios"][0]["adapter_contracts"][0]["required_capabilities"] =
            json!(["semantic_operation", "provider_first_reservation"]);
        unsupported_required["scenarios"][0]["adapter_contracts"][0]["optional_capabilities"] =
            json!(["retained_subtree_payload", "recovery", "shutdown_drain"]);
        assert!(matches!(
            load_fixture_value(&unsupported_required),
            DockspaceScenarioFixtureError::InvalidScenario { scenario, message }
                if scenario == "minimal"
                    && message.contains("does not implement required capabilities")
        ));

        let mut duplicate_adapter = minimal_fixture_suite();
        let contract = duplicate_adapter["scenarios"][0]["adapter_contracts"][0].clone();
        duplicate_adapter["scenarios"][0]["adapter_contracts"] =
            json!([contract.clone(), contract]);
        assert!(matches!(
            load_fixture_value(&duplicate_adapter),
            DockspaceScenarioFixtureError::DuplicateAdapter { scenario, adapter }
                if scenario == "minimal" && adapter == "headless"
        ));
    }

    #[test]
    fn fixture_loader_rejects_invalid_result_and_interaction_algebra() {
        let operation = json!({
            "type": "move_item_to_item",
            "source_space": "main",
            "item": "A",
            "target_space": "main",
            "target_item": "A",
            "zone": "center"
        });
        let mut invalid_result = minimal_fixture_suite();
        invalid_result["scenarios"][0]["adapter_contracts"][0]["required_capabilities"] =
            json!(["semantic_operation", "move_item_to_item", "item_payload"]);
        let mut step = committed_step("move-a", operation.clone());
        step["expected"]["prepare"] = json!("no_op");
        invalid_result["scenarios"][0]["steps"] = json!([step]);
        assert!(matches!(
            load_fixture_value(&invalid_result),
            DockspaceScenarioFixtureError::InvalidStep { scenario, step, message }
                if scenario == "minimal"
                    && step == "move-a"
                    && message.contains("invalid prepare/commit/terminal/receipt")
        ));

        let mut invalid_interaction = minimal_fixture_suite();
        invalid_interaction["scenarios"][0]["adapter_contracts"][0]["required_capabilities"] =
            json!(["semantic_operation", "move_item_to_item", "item_payload"]);
        let mut step = committed_step("move-a", operation);
        step["interaction"] = json!({
            "actions": [{ "type": "hover_target" }, { "type": "release" }]
        });
        invalid_interaction["scenarios"][0]["steps"] = json!([step]);
        assert!(matches!(
            load_fixture_value(&invalid_interaction),
            DockspaceScenarioFixtureError::InvalidStep { scenario, step, message }
                if scenario == "minimal"
                    && step == "move-a"
                    && message.contains("must begin with begin_drag")
        ));

        let mut recovery_pending_terminal = invalid_result;
        recovery_pending_terminal["scenarios"][0]["steps"][0]["expected"]["terminal"] =
            json!("recovery_pending");
        assert!(matches!(
            load_fixture_value(&recovery_pending_terminal),
            DockspaceScenarioFixtureError::Parse(_)
        ));
    }

    #[test]
    fn fixture_loader_rejects_checkpoint_contract_violations() {
        let fixture = json!({
            "schema_version": 2,
            "scenarios": [
                {
                    "id": "checkpoint-mismatch",
                    "phases": {
                        "fixture": { "arrangement": "direct_deterministic" },
                        "harness": { "ready": "canonical_graph_published" },
                        "warmup": { "iterations": 0 },
                        "teardown": {
                            "require_canonical_graph": true,
                            "require_no_pending_progress": true
                        }
                    },
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
                        committed_step("move-a", json!({
                            "type": "move_item_to_item",
                            "source_space": "source",
                            "item": "A",
                            "target_space": "target",
                            "target_item": "B",
                            "zone": "center"
                        }))
                    ],
                    "checkpoints": [
                        {
                            "after_step": "move-a",
                            "phase": "terminal",
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
                            ],
                            "facts": {
                                "terminal": "committed",
                                "receipt": { "kind": "new", "alias": "move-a" }
                            }
                        }
                    ],
                    "adapter_contracts": [
                        headless_contract(json!([
                            "semantic_operation",
                            "move_item_to_item",
                            "item_payload",
                            "cross_viewport"
                        ]))
                    ],
                    "imgui_categories": []
                }
            ]
        });

        load_scenario_fixtures(&fixture.to_string()).expect(
            "the fixture loader should validate checkpoint structure without executing production operations",
        );

        let mut wrong_order = fixture.clone();
        wrong_order["scenarios"][0]["checkpoints"][0]["after_step"] = json!("another-step");
        assert!(matches!(
            load_fixture_value(&wrong_order),
            DockspaceScenarioFixtureError::InvalidStep { scenario, step, message }
                if scenario == "checkpoint-mismatch"
                    && step == "another-step"
                    && message.contains("unknown step")
        ));

        let mut wrong_count = fixture;
        let duplicate_checkpoint = wrong_count["scenarios"][0]["checkpoints"][0].clone();
        wrong_count["scenarios"][0]["checkpoints"] =
            json!([duplicate_checkpoint.clone(), duplicate_checkpoint]);
        let wrong_count_error = load_fixture_value(&wrong_count);
        let wrong_count_diagnostic = format!("{wrong_count_error:?}");
        assert!(
            matches!(
                wrong_count_error,
            DockspaceScenarioFixtureError::InvalidStep { scenario, step, message }
                if scenario == "checkpoint-mismatch"
                    && step == "move-a"
                    && message.contains("repeats phase")
            ),
            "unexpected wrong-count error: {wrong_count_diagnostic}"
        );
    }

    #[test]
    fn fixture_loader_rejects_a_step_that_references_a_missing_target_item() {
        let mut fixture = minimal_fixture_suite();
        fixture["scenarios"][0]["id"] = json!("missing-target");
        fixture["scenarios"][0]["adapter_contracts"][0]["required_capabilities"] =
            json!(["semantic_operation", "move_item_to_item", "item_payload"]);
        fixture["scenarios"][0]["steps"] = json!([committed_step(
            "bad-target",
            json!({
                "type": "move_item_to_item",
                "source_space": "main",
                "item": "A",
                "target_space": "main",
                "target_item": "missing",
                "zone": "center"
            })
        )]);
        let error = load_fixture_value(&fixture);

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
