#![cfg(feature = "ui-components")]

use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

use open_gpui_devtools::{
    DevtoolsArtifact, DevtoolsArtifactMetadata, DevtoolsArtifactRecord, DevtoolsInspectorState,
    DevtoolsRegistry, DevtoolsReport, DevtoolsSession, SnapshotKind,
    ui_components::{
        ComponentSemanticIdentity, OpaqueSemanticNodeId, ResolvedSemanticNode,
        resolved_semantics_probe_snapshot,
    },
};
use open_gpui_ui_components::{FieldState, TextInputState, TextareaState};
use open_gpui_ui_core::{
    AccessibleAction, AccessibleTextPosition, AccessibleTextSelection, Role, SemanticDescriptor,
    Size, ThemeTokens,
};

const LABEL_CANARY: &str = "u5-label-canary-019f4ad7";
const DESCRIPTION_CANARY: &str = "u5-description-canary-4d33-7573";
const PLACEHOLDER_CANARY: &str = "u5-placeholder-canary-ac26";
const ERROR_CANARY: &str = "u5-error-canary-94bc135cc634";
const PASSWORD_CANARY: &str = "u5-password-canary-capture";
const USER_INPUT_CANARY: &str = "u5-user-input-canary-history";
const USER_INPUT_NEXT_CANARY: &str = "u5-user-input-canary-diff";
const CLIPBOARD_CANARY: &str = "u5-clipboard-canary-export";
const TEXT_RUN_VALUE_CANARY: &str = "u5-text-run-canary-character-lengths";
const NUMERIC_VALUE_CANARY: f64 = 9_876_543_210.125;
const NUMERIC_MINIMUM_CANARY: f64 = -8_765_432_109.25;
const NUMERIC_MAXIMUM_CANARY: f64 = 7_654_321_098.5;
const RELATION_CONTROLS_NODE_ID: u64 = 18_446_744_073_709_550_001;
const RELATION_LABEL_NODE_ID: u64 = 18_446_744_073_709_550_002;
const RELATION_DESCRIPTION_NODE_ID: u64 = 18_446_744_073_709_550_003;
const RELATION_ERROR_NODE_ID: u64 = 18_446_744_073_709_550_004;
const SELECTION_ANCHOR_NODE_ID: u64 = 18_446_744_073_709_550_005;
const SELECTION_FOCUS_NODE_ID: u64 = 18_446_744_073_709_550_006;

#[test]
fn resolved_semantic_canaries_never_cross_any_devtools_artifact_boundary() {
    let fixture_snapshot = resolved_text_form_snapshot(false);
    let fixture_json = include_str!("fixtures/resolved-semantic-redaction.json");
    let fixture_value: serde_json::Value = serde_json::from_str(fixture_json).unwrap();
    let actual_fixture_value = resolved_semantic_fixture_value(&fixture_snapshot);
    assert_eq!(actual_fixture_value, fixture_value);
    assert_eq!(fixture_snapshot.redaction().redacted_values, 11);

    let password_payload = fixture_snapshot.tree().nodes[0].children[2]
        .payload
        .as_ref()
        .expect("password semantic payload");
    assert_eq!(password_payload["relations"]["labelled_by_count"], 1);
    assert_eq!(password_payload["relations"]["described_by_count"], 1);
    assert_eq!(password_payload["relations"]["controls_count"], 1);
    assert_eq!(
        password_payload["relations"]["error_message"]["kind"],
        "present"
    );
    assert_eq!(
        password_payload["text"]["value"]["kind"],
        "password-redacted"
    );
    assert_eq!(
        password_payload["actions"],
        serde_json::json!(["focus", "set-value"])
    );
    let input_payload = fixture_snapshot.tree().nodes[0].children[3]
        .payload
        .as_ref()
        .expect("text input semantic payload");
    assert_eq!(
        input_payload["text_structure"]["selection"]["kind"],
        "present"
    );
    let numeric_payload = fixture_snapshot.tree().nodes[0].children[5]
        .payload
        .as_ref()
        .expect("numeric semantic payload");
    assert_eq!(numeric_payload["numeric"]["value"]["kind"], "redacted");
    assert_eq!(numeric_payload["numeric"]["minimum"]["kind"], "redacted");
    assert_eq!(numeric_payload["numeric"]["maximum"]["kind"], "redacted");
    let text_run_payload = fixture_snapshot.tree().nodes[0].children[6]
        .payload
        .as_ref()
        .expect("TextRun semantic payload");
    assert_eq!(text_run_payload["role"], "text-run");
    assert_eq!(text_run_payload["text"]["value"]["kind"], "redacted");
    assert_eq!(
        text_run_payload["text_structure"]["character_lengths"]["kind"],
        "present"
    );

    let invocation = Arc::new(AtomicUsize::new(0));
    let mut registry = DevtoolsRegistry::default();
    registry
        .register_snapshot_probe(
            "ui-components.resolved-semantics",
            SnapshotKind::Accessibility,
            {
                let invocation = Arc::clone(&invocation);
                move || {
                    let changed = invocation.fetch_add(1, Ordering::SeqCst) > 0;
                    Ok(resolved_text_form_snapshot(changed))
                }
            },
        )
        .unwrap();

    let mut session = DevtoolsSession::new("resolved-semantic-redaction", registry);
    let first = session.refresh().unwrap();
    let second = session.refresh().unwrap();
    let export = session.export();
    let report = DevtoolsReport::from_session_export(&export);
    let diff = second
        .diff_from_previous
        .as_ref()
        .expect("second frame must include a diff");
    assert!(!diff.is_empty());
    assert!(diff.summary.changed > 0);

    let capture_json = serde_json::to_string(&first.capture).unwrap();
    let history_json = serde_json::to_string(&session.frames().collect::<Vec<_>>()).unwrap();
    let diff_json = serde_json::to_string(diff).unwrap();
    let export_json = serde_json::to_string(&export).unwrap();
    let capture_artifact_json = DevtoolsArtifactRecord::new(
        DevtoolsArtifactMetadata::new("resolved-semantic-canary"),
        DevtoolsArtifact::capture(&first.capture),
    )
    .to_pretty_json()
    .unwrap();
    let export_artifact_json = DevtoolsArtifactRecord::new(
        DevtoolsArtifactMetadata::new("resolved-semantic-canary"),
        DevtoolsArtifact::session_export(&export),
    )
    .to_pretty_json()
    .unwrap();
    let report_json = serde_json::to_string(&report).unwrap();
    let report_markdown = report.to_markdown();
    let report_artifact_json = DevtoolsArtifactRecord::new(
        DevtoolsArtifactMetadata::new("resolved-semantic-canary"),
        DevtoolsArtifact::report(&report),
    )
    .to_pretty_json()
    .unwrap();
    let inspector = DevtoolsInspectorState::from_capture(first.capture.clone());
    let inspector_detail_json =
        serde_json::to_string(&inspector.selected_detail_json().unwrap()).unwrap();
    let inspector_copy_json = inspector.copy_selected_detail().unwrap().pretty_json;

    let channels = [
        ("capture", capture_json.as_str()),
        ("history", history_json.as_str()),
        ("diff", diff_json.as_str()),
        ("Inspector detail", inspector_detail_json.as_str()),
        ("Inspector copy", inspector_copy_json.as_str()),
        ("export", export_json.as_str()),
        ("capture artifact", capture_artifact_json.as_str()),
        ("export artifact", export_artifact_json.as_str()),
        ("report", report_json.as_str()),
        ("report markdown", report_markdown.as_str()),
        ("report artifact", report_artifact_json.as_str()),
        ("fixture", fixture_json),
    ];
    let numeric_canaries = [
        NUMERIC_VALUE_CANARY,
        NUMERIC_MINIMUM_CANARY,
        NUMERIC_MAXIMUM_CANARY,
    ]
    .map(|value| value.to_string());
    let renderer_node_canaries = [
        RELATION_CONTROLS_NODE_ID,
        RELATION_LABEL_NODE_ID,
        RELATION_DESCRIPTION_NODE_ID,
        RELATION_ERROR_NODE_ID,
        SELECTION_ANCHOR_NODE_ID,
        SELECTION_FOCUS_NODE_ID,
    ]
    .map(|value| value.to_string());
    for canary in [
        LABEL_CANARY,
        DESCRIPTION_CANARY,
        PLACEHOLDER_CANARY,
        ERROR_CANARY,
        PASSWORD_CANARY,
        USER_INPUT_CANARY,
        USER_INPUT_NEXT_CANARY,
        CLIPBOARD_CANARY,
        TEXT_RUN_VALUE_CANARY,
    ]
    .into_iter()
    .chain(numeric_canaries.iter().map(String::as_str))
    .chain(renderer_node_canaries.iter().map(String::as_str))
    {
        for (channel, output) in channels {
            assert!(
                !output.contains(canary),
                "{channel} leaked resolved semantic canary `{canary}`"
            );
        }
    }

    assert!(capture_json.contains("\"contract_id\":\"TextInput\""));
    assert!(capture_json.contains("\"contract_id\":\"Textarea\""));
    assert!(capture_json.contains("\"contract_id\":\"Field\""));
    assert!(capture_json.contains("\"family\":\"form\""));
    assert!(capture_json.contains("\"kind\":\"redacted\""));
    assert!(capture_json.contains("\"kind\":\"password-redacted\""));
    assert_eq!(report.summary.redacted_value_count, 11);
}

fn resolved_semantic_fixture_value(
    snapshot: &open_gpui_devtools::SnapshotProbeSnapshot,
) -> serde_json::Value {
    let root = &snapshot.tree().nodes[0];
    let nodes = root
        .children
        .iter()
        .map(|node| {
            let payload = node.payload.as_ref().expect("resolved semantic payload");
            serde_json::json!({
                "semantic_id": payload["semantic_id"],
                "contract_id": payload["contract_id"],
                "family": payload["family"],
                "role": payload["role"],
                "text": payload["text"],
            })
        })
        .collect::<Vec<_>>();

    serde_json::json!({
        "node_count": root.payload.as_ref().expect("root payload")["node_count"],
        "nodes": nodes,
        "redacted_values": snapshot.redaction().redacted_values,
    })
}

fn resolved_text_form_snapshot(changed: bool) -> open_gpui_devtools::SnapshotProbeSnapshot {
    let tokens = ThemeTokens::default();
    let field = FieldState::resolve(
        LABEL_CANARY,
        Some(ERROR_CANARY),
        Some(ERROR_CANARY),
        Size::Medium,
        true,
        false,
        changed,
        tokens,
    )
    .with_busy(changed);
    let label_semantics = SemanticDescriptor::<u64>::new(Role::Label).with_label(field.label());
    let error_semantics = SemanticDescriptor::<u64>::new(Role::Label)
        .with_label(field.support_text().expect("field support text"));

    let controls = [RELATION_CONTROLS_NODE_ID];
    let labelled_by = [RELATION_LABEL_NODE_ID];
    let described_by = [RELATION_DESCRIPTION_NODE_ID];
    let error_message = RELATION_ERROR_NODE_ID;
    let password_actions = [AccessibleAction::Focus, AccessibleAction::SetValue];
    let password_semantics = SemanticDescriptor::<u64>::new(Role::PasswordInput)
        .with_value(PASSWORD_CANARY)
        .with_description(DESCRIPTION_CANARY)
        .with_placeholder(PLACEHOLDER_CANARY)
        .with_busy(changed)
        .with_controls(&controls)
        .with_labelled_by(&labelled_by)
        .with_described_by(&described_by)
        .with_error_message(&error_message)
        .with_actions(&password_actions);

    let selection = AccessibleTextSelection::new(
        AccessibleTextPosition::new(SELECTION_ANCHOR_NODE_ID, 2),
        AccessibleTextPosition::new(SELECTION_FOCUS_NODE_ID, 4),
    );
    let user_value = if changed {
        USER_INPUT_NEXT_CANARY
    } else {
        USER_INPUT_CANARY
    };
    let input = TextInputState::resolve(
        user_value,
        None::<String>,
        Size::Medium,
        false,
        false,
        false,
        false,
        true,
        tokens,
    );
    let input_projection = input.semantic_projection::<u64>();
    let input_semantics = input_projection
        .descriptor()
        .with_text_selection(&selection);
    let textarea = TextareaState::resolve(
        CLIPBOARD_CANARY,
        None::<String>,
        Size::Medium,
        3,
        false,
        false,
        false,
        false,
        true,
        tokens,
    );
    let textarea_projection = textarea.semantic_projection::<u64>();
    let textarea_semantics = textarea_projection.descriptor();
    let numeric_semantics = SemanticDescriptor::<u64>::new(Role::Slider)
        .with_numeric_value(NUMERIC_VALUE_CANARY)
        .with_min_numeric_value(NUMERIC_MINIMUM_CANARY)
        .with_max_numeric_value(NUMERIC_MAXIMUM_CANARY);
    let text_run_character_lengths = [1, 2, 3];
    let text_run_semantics = SemanticDescriptor::<u64>::new(Role::TextRun)
        .with_value(TEXT_RUN_VALUE_CANARY)
        .with_character_lengths(&text_run_character_lengths);

    resolved_semantics_probe_snapshot([
        ResolvedSemanticNode::new(
            ComponentSemanticIdentity::for_component("Field").unwrap(),
            OpaqueSemanticNodeId::new(11),
            label_semantics,
        ),
        ResolvedSemanticNode::new(
            ComponentSemanticIdentity::for_component("Field").unwrap(),
            OpaqueSemanticNodeId::new(12),
            error_semantics,
        ),
        ResolvedSemanticNode::new(
            ComponentSemanticIdentity::for_component("TextInput").unwrap(),
            OpaqueSemanticNodeId::new(20),
            password_semantics,
        ),
        ResolvedSemanticNode::new(
            ComponentSemanticIdentity::for_component("TextInput").unwrap(),
            OpaqueSemanticNodeId::new(21),
            input_semantics,
        ),
        ResolvedSemanticNode::new(
            ComponentSemanticIdentity::for_component("Textarea").unwrap(),
            OpaqueSemanticNodeId::new(30),
            textarea_semantics,
        ),
        ResolvedSemanticNode::new(
            ComponentSemanticIdentity::for_component("Slider").unwrap(),
            OpaqueSemanticNodeId::new(40),
            numeric_semantics,
        ),
        ResolvedSemanticNode::new(
            ComponentSemanticIdentity::for_component("TextInput").unwrap(),
            OpaqueSemanticNodeId::new(50),
            text_run_semantics,
        ),
    ])
}
