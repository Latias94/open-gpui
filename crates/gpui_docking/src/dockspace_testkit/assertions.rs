use crate::{
    DockActionOutcome, DockGraph, DockNode, DockSpaceId, SplitAxis,
    dockspace_testkit::{
        contract::DockspaceDragTerminal,
        topology::{
            DockspaceScenario, DockspaceScenarioBuildError, DockspaceScenarioCheckpoint,
            DockspaceTopologySpace,
        },
    },
};
use serde_json::{Value, json};
use std::collections::BTreeSet;
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Error)]
#[error(
    "dockspace scenario {scenario} action {action} checkpoint mismatch at {field}: expected {expected}, actual {actual}; pre-state {pre_state}"
)]
pub(crate) struct DockspaceCheckpointMismatch {
    pub(crate) scenario: String,
    pub(crate) action: String,
    pub(crate) field: String,
    pub(crate) expected: Value,
    pub(crate) actual: Value,
    pub(crate) pre_state: Value,
}

#[derive(Debug, Error)]
pub(crate) enum DockspaceCheckpointAssertionError {
    #[error(transparent)]
    Observe(#[from] DockspaceScenarioBuildError),
    #[error(transparent)]
    Serialize(#[from] serde_json::Error),
    #[error(transparent)]
    Mismatch(#[from] DockspaceCheckpointMismatch),
}

pub(crate) fn assert_headless_checkpoint(
    scenario: &DockspaceScenario,
    checkpoint: &DockspaceScenarioCheckpoint,
    graph: &DockGraph,
    outcome: DockActionOutcome,
    pre_state: &[DockspaceTopologySpace],
) -> Result<(), DockspaceCheckpointAssertionError> {
    let actual_spaces = scenario.observe_graph_spaces(graph)?;
    let expected_spaces = serde_json::to_value(&checkpoint.spaces)?;
    let actual_spaces = serde_json::to_value(actual_spaces)?;
    let pre_state = serde_json::to_value(pre_state)?;

    assert_json_matches(
        scenario,
        checkpoint,
        "spaces",
        &expected_spaces,
        &actual_spaces,
        &pre_state,
    )?;

    if let Some(expected_terminal) = checkpoint.facts.terminal {
        let actual_terminal = match outcome {
            DockActionOutcome::Changed => DockspaceDragTerminal::Committed,
            DockActionOutcome::Unchanged => DockspaceDragTerminal::Unchanged,
        };
        assert_json_matches(
            scenario,
            checkpoint,
            "facts.terminal",
            &serde_json::to_value(expected_terminal)?,
            &serde_json::to_value(actual_terminal)?,
            &pre_state,
        )?;
    }

    assert_json_matches(
        scenario,
        checkpoint,
        "facts.revisions.topology_delta",
        &json!(checkpoint.facts.revisions.topology_delta),
        &json!(u64::from(outcome == DockActionOutcome::Changed)),
        &pre_state,
    )?;

    if let Some(expected_unreachable) = checkpoint.facts.unreachable_node_count {
        let actual_unreachable = match graph.validate_canonical() {
            Ok(()) => json!(0),
            Err(error) => json!({ "validation_error": error.to_string() }),
        };
        assert_json_matches(
            scenario,
            checkpoint,
            "facts.unreachable_node_count",
            &json!(expected_unreachable),
            &actual_unreachable,
            &pre_state,
        )?;
    }

    Ok(())
}

fn assert_json_matches(
    scenario: &DockspaceScenario,
    checkpoint: &DockspaceScenarioCheckpoint,
    field: &str,
    expected: &Value,
    actual: &Value,
    pre_state: &Value,
) -> Result<(), DockspaceCheckpointAssertionError> {
    let Some((field, expected, actual)) = first_json_diff(expected, actual, field) else {
        return Ok(());
    };
    Err(DockspaceCheckpointMismatch {
        scenario: scenario.id.clone(),
        action: checkpoint.after_step.clone(),
        field,
        expected,
        actual,
        pre_state: pre_state.clone(),
    }
    .into())
}

fn first_json_diff(expected: &Value, actual: &Value, path: &str) -> Option<(String, Value, Value)> {
    if expected == actual {
        return None;
    }
    match (expected, actual) {
        (Value::Object(expected), Value::Object(actual)) => {
            for (key, expected) in expected {
                let child_path = format!("{path}.{key}");
                let Some(actual) = actual.get(key) else {
                    return Some((child_path, expected.clone(), json!("<missing>")));
                };
                if let Some(diff) = first_json_diff(expected, actual, &child_path) {
                    return Some(diff);
                }
            }
            for (key, actual) in actual {
                if !expected.contains_key(key) {
                    return Some((format!("{path}.{key}"), json!("<absent>"), actual.clone()));
                }
            }
            None
        }
        (Value::Array(expected), Value::Array(actual)) => {
            for (index, (expected, actual)) in expected.iter().zip(actual).enumerate() {
                if let Some(diff) = first_json_diff(expected, actual, &format!("{path}[{index}]")) {
                    return Some(diff);
                }
            }
            if expected.len() != actual.len() {
                return Some((
                    format!("{path}.length"),
                    json!(expected.len()),
                    json!(actual.len()),
                ));
            }
            None
        }
        _ => Some((path.to_owned(), expected.clone(), actual.clone())),
    }
}

pub(crate) fn assert_space_items(
    graph: &DockGraph,
    space: impl Into<DockSpaceId>,
    expected: &[&str],
) {
    let space = space.into();
    let actual = graph
        .collect_items_in_space(&space)
        .into_iter()
        .map(|item| item.to_string())
        .collect::<Vec<_>>();
    assert_eq!(actual, expected, "space {space} item order mismatch");
}

pub(crate) fn assert_unique_items(graph: &DockGraph) {
    let mut seen = BTreeSet::new();
    for space in graph.spaces() {
        for item in graph.collect_items_in_space(&space) {
            assert!(
                seen.insert(item.clone()),
                "dockspace scenario graph duplicated item {item}"
            );
        }
    }
}

pub(crate) fn assert_root_horizontal_split(
    graph: &DockGraph,
    space: impl Into<DockSpaceId>,
    child_count: usize,
) {
    let space = space.into();
    let root = graph
        .root(&space)
        .unwrap_or_else(|| panic!("space {space} should have a root"));
    let DockNode::Split {
        axis,
        children,
        fractions,
    } = graph
        .node(root)
        .unwrap_or_else(|| panic!("space {space} root should exist"))
    else {
        panic!("space {space} root should be a split");
    };
    assert_eq!(*axis, SplitAxis::Horizontal);
    assert_eq!(children.len(), child_count);
    assert_eq!(fractions.len(), child_count);
}

pub(crate) fn assert_layout_roundtrip_preserves_space_items(graph: &DockGraph) {
    let layout = graph.export_layout();
    let imported = DockGraph::import_layout(&layout).expect("scenario layout should roundtrip");
    for space in graph.spaces() {
        assert_eq!(
            imported.collect_items_in_space(&space),
            graph.collect_items_in_space(&space),
            "space {space} layout roundtrip item mismatch"
        );
    }
}
