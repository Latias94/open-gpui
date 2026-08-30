use crate::{
    CanonicalNodeId, DockActionOutcome, DockFloatingId, DockGraph, DockGroupId, DockNode,
    DockSpaceId, SplitAxis,
    dockspace_testkit::{
        canonical_headless::CanonicalDockspaceStepEvidence,
        contract::{DockspaceDragTerminal, DockspaceReceiptExpectation},
        ops::{DockspaceScenarioStep, DockspaceScenarioZone, DockspaceSemanticOperation},
        topology::{
            DockspaceScenario, DockspaceScenarioBuildError, DockspaceScenarioCheckpoint,
            DockspaceTopologySpace,
        },
    },
    graph_assembly::{CanonicalDockGraph, CanonicalNode},
    graph_patch::{GraphCommitId, GraphRevision},
};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct CanonicalEdgeTargetProbe {
    target: CanonicalNodeId,
    axis: SplitAxis,
    target_is_second: bool,
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

pub(crate) fn assert_canonical_checkpoint(
    scenario: &DockspaceScenario,
    step: &DockspaceScenarioStep,
    checkpoint: &DockspaceScenarioCheckpoint,
    graph: &CanonicalDockGraph,
    evidence: &CanonicalDockspaceStepEvidence,
    receipt_aliases: &mut BTreeMap<String, GraphCommitId>,
    pre_state: &[DockspaceTopologySpace],
) -> Result<(), DockspaceCheckpointAssertionError> {
    let expected_spaces = serde_json::to_value(&checkpoint.spaces)?;
    let actual_spaces = serde_json::to_value(scenario.observe_canonical_spaces(graph)?)?;
    let pre_state = serde_json::to_value(pre_state)?;
    assert_json_matches(
        scenario,
        checkpoint,
        "spaces",
        &expected_spaces,
        &actual_spaces,
        &pre_state,
    )?;

    let expected_space_ids = scenario
        .spaces
        .iter()
        .map(|space| space.id.as_str())
        .collect::<BTreeSet<_>>();
    let actual_space_ids = graph
        .spaces()
        .keys()
        .map(DockSpaceId::as_str)
        .collect::<BTreeSet<_>>();
    assert_json_matches(
        scenario,
        checkpoint,
        "canonical.spaces",
        &serde_json::to_value(expected_space_ids)?,
        &serde_json::to_value(actual_space_ids)?,
        &pre_state,
    )?;

    if let Err(error) = graph.validate() {
        assert_json_matches(
            scenario,
            checkpoint,
            "canonical.validation",
            &json!("valid"),
            &json!(error.to_string()),
            &pre_state,
        )?;
    }

    assert_json_matches(
        scenario,
        checkpoint,
        "step.expected.prepare",
        &serde_json::to_value(step.expected.prepare)?,
        &serde_json::to_value(evidence.prepare)?,
        &pre_state,
    )?;
    assert_json_matches(
        scenario,
        checkpoint,
        "step.expected.commit",
        &serde_json::to_value(step.expected.commit)?,
        &serde_json::to_value(evidence.commit)?,
        &pre_state,
    )?;
    assert_json_matches(
        scenario,
        checkpoint,
        "step.expected.terminal",
        &serde_json::to_value(step.expected.terminal)?,
        &serde_json::to_value(evidence.terminal)?,
        &pre_state,
    )?;
    assert_canonical_receipt(
        scenario,
        checkpoint,
        "step.expected.receipt",
        &step.expected.receipt,
        evidence.receipt_id,
        receipt_aliases,
        &pre_state,
    )?;

    if let Some(expected_terminal) = checkpoint.facts.terminal {
        assert_json_matches(
            scenario,
            checkpoint,
            "facts.terminal",
            &serde_json::to_value(expected_terminal)?,
            &serde_json::to_value(evidence.terminal)?,
            &pre_state,
        )?;
    }
    if let Some(expected_progress) = checkpoint.facts.progress {
        assert_json_matches(
            scenario,
            checkpoint,
            "facts.progress",
            &serde_json::to_value(expected_progress)?,
            &Value::Null,
            &pre_state,
        )?;
    }
    if let Some(expected_receipt) = &checkpoint.facts.receipt {
        assert_canonical_receipt(
            scenario,
            checkpoint,
            "facts.receipt",
            expected_receipt,
            evidence.receipt_id,
            receipt_aliases,
            &pre_state,
        )?;
    }

    let actual_revisions = revision_delta(evidence.before, evidence.after);
    assert_json_matches(
        scenario,
        checkpoint,
        "facts.revisions",
        &serde_json::to_value(checkpoint.facts.revisions)?,
        &actual_revisions,
        &pre_state,
    )?;

    for expected in &checkpoint.facts.groups {
        let group_id = DockGroupId::from(expected.group.as_str());
        let actual = graph.group(&group_id);
        assert_json_matches(
            scenario,
            checkpoint,
            &format!("facts.groups.{}.present", expected.group),
            &json!(true),
            &json!(actual.is_some()),
            &pre_state,
        )?;
        let Some(actual) = actual else {
            continue;
        };
        if let Some(selected) = &expected.selected {
            assert_json_matches(
                scenario,
                checkpoint,
                &format!("facts.groups.{}.selected", expected.group),
                &serde_json::to_value(selected)?,
                &serde_json::to_value(&actual.selected)?,
                &pre_state,
            )?;
        }
        if !expected.mru.is_empty() {
            assert_json_matches(
                scenario,
                checkpoint,
                &format!("facts.groups.{}.mru", expected.group),
                &serde_json::to_value(&expected.mru)?,
                &serde_json::to_value(&actual.mru)?,
                &pre_state,
            )?;
        }
        if let Some(visible) = &expected.visible {
            assert_json_matches(
                scenario,
                checkpoint,
                &format!("facts.groups.{}.visible", expected.group),
                &serde_json::to_value(visible)?,
                &Value::Null,
                &pre_state,
            )?;
        }
    }

    for expected in &checkpoint.facts.floating_ownership {
        let floating = graph
            .floating_roots()
            .get(&DockFloatingId::from(expected.floating.as_str()));
        let actual_space = floating.map(|floating| &floating.space);
        assert_json_matches(
            scenario,
            checkpoint,
            &format!("facts.floating_ownership.{}", expected.floating),
            &serde_json::to_value(&expected.space)?,
            &serde_json::to_value(actual_space)?,
            &pre_state,
        )?;
    }

    if let Some(focused_item) = &checkpoint.facts.focused_item {
        assert_json_matches(
            scenario,
            checkpoint,
            "facts.focused_item",
            &serde_json::to_value(focused_item)?,
            &Value::Null,
            &pre_state,
        )?;
    }

    let identities = &checkpoint.facts.identities;
    if !identities.preserved.is_empty()
        || !identities.created.is_empty()
        || !identities.retired.is_empty()
        || !identities.remapped.is_empty()
    {
        assert_json_matches(
            scenario,
            checkpoint,
            "facts.identities",
            &serde_json::to_value(identities)?,
            &json!({ "unsupported_oracle": "canonical identity receipt" }),
            &pre_state,
        )?;
    }

    if let Some(expected_unreachable) = checkpoint.facts.unreachable_node_count {
        assert_json_matches(
            scenario,
            checkpoint,
            "facts.unreachable_node_count",
            &json!(expected_unreachable),
            &json!(0),
            &pre_state,
        )?;
    }

    Ok(())
}

pub(crate) fn capture_canonical_edge_target(
    graph: &CanonicalDockGraph,
    step: &DockspaceScenarioStep,
) -> Option<CanonicalEdgeTargetProbe> {
    let (target_item, zone) = match &step.operation {
        DockspaceSemanticOperation::MoveItemToItem {
            target_item, zone, ..
        }
        | DockspaceSemanticOperation::MoveGroupToItem {
            target_item, zone, ..
        }
        | DockspaceSemanticOperation::MoveFloatingToItem {
            target_item, zone, ..
        } => (target_item, *zone),
        DockspaceSemanticOperation::MoveItemToTab { .. }
        | DockspaceSemanticOperation::MoveItemToEmptySpace { .. }
        | DockspaceSemanticOperation::FloatItemInWindow { .. } => return None,
    };
    let (axis, target_is_second) = match zone {
        DockspaceScenarioZone::Left => (SplitAxis::Horizontal, true),
        DockspaceScenarioZone::Right => (SplitAxis::Horizontal, false),
        DockspaceScenarioZone::Top => (SplitAxis::Vertical, true),
        DockspaceScenarioZone::Bottom => (SplitAxis::Vertical, false),
        DockspaceScenarioZone::Center => return None,
    };
    let target = graph
        .index()
        .group_for_item(target_item)
        .and_then(|group| graph.index().node_for_group(group))?;
    Some(CanonicalEdgeTargetProbe {
        target,
        axis,
        target_is_second,
    })
}

pub(crate) fn assert_canonical_edge_locality(
    scenario: &DockspaceScenario,
    checkpoint: &DockspaceScenarioCheckpoint,
    graph: &CanonicalDockGraph,
    probe: Option<CanonicalEdgeTargetProbe>,
    pre_state: &[DockspaceTopologySpace],
) -> Result<(), DockspaceCheckpointAssertionError> {
    let Some(probe) = probe else {
        return Ok(());
    };
    let parent = graph.index().parent_of(probe.target).flatten();
    let actual = parent
        .and_then(|parent| graph.node(parent))
        .and_then(|node| match node {
            CanonicalNode::Branch {
                axis,
                first,
                second,
                ..
            } => Some(json!({
                "axis": axis,
                "target_is_first": *first == probe.target,
                "target_is_second": *second == probe.target,
            })),
            CanonicalNode::Group { .. } => None,
        })
        .unwrap_or_else(|| json!({ "direct_wrapper": "missing" }));
    let expected = json!({
        "axis": probe.axis,
        "target_is_first": !probe.target_is_second,
        "target_is_second": probe.target_is_second,
    });
    assert_json_matches(
        scenario,
        checkpoint,
        "canonical.binary_edge_wrapper",
        &expected,
        &actual,
        &serde_json::to_value(pre_state)?,
    )
}

#[allow(clippy::too_many_arguments)]
fn assert_canonical_receipt(
    scenario: &DockspaceScenario,
    checkpoint: &DockspaceScenarioCheckpoint,
    field: &str,
    expected: &DockspaceReceiptExpectation,
    actual: Option<GraphCommitId>,
    aliases: &mut BTreeMap<String, GraphCommitId>,
    pre_state: &Value,
) -> Result<(), DockspaceCheckpointAssertionError> {
    match expected {
        DockspaceReceiptExpectation::None => assert_json_matches(
            scenario,
            checkpoint,
            field,
            &Value::Null,
            &serde_json::to_value(actual.map(|receipt| receipt.to_string()))?,
            pre_state,
        ),
        DockspaceReceiptExpectation::New { alias } => {
            let Some(actual) = actual else {
                return assert_json_matches(
                    scenario,
                    checkpoint,
                    field,
                    &json!({ "new": alias }),
                    &Value::Null,
                    pre_state,
                );
            };
            if let Some(previous) = aliases.get(alias) {
                assert_json_matches(
                    scenario,
                    checkpoint,
                    field,
                    &json!(previous.to_string()),
                    &json!(actual.to_string()),
                    pre_state,
                )?;
            } else {
                aliases.insert(alias.clone(), actual);
            }
            Ok(())
        }
        DockspaceReceiptExpectation::SameAs { alias } => {
            let expected = aliases.get(alias).map(ToString::to_string);
            let actual = actual.map(|receipt| receipt.to_string());
            assert_json_matches(
                scenario,
                checkpoint,
                field,
                &serde_json::to_value(expected)?,
                &serde_json::to_value(actual)?,
                pre_state,
            )
        }
    }
}

fn revision_delta(before: GraphRevision, after: GraphRevision) -> Value {
    if before.authority_epoch() != after.authority_epoch() {
        return json!({
            "authority_changed": {
                "before": before.authority_epoch(),
                "after": after.authority_epoch(),
            }
        });
    }
    json!({
        "topology_delta": after.topology().checked_sub(before.topology()),
        "selection_delta": after.selection().checked_sub(before.selection()),
        "interaction_delta": after.interaction().checked_sub(before.interaction()),
    })
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
