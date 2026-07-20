use super::*;

fn focus_a11y_story(id: &str) -> StoryContract {
    pages::focus_a11y::focus_a11y_story_contracts()
        .into_iter()
        .find(|story| story.owner_name() == id)
        .unwrap_or_else(|| panic!("expected Focus/A11y story `{id}`"))
}

fn a11y_node_by_id(update: &accesskit::TreeUpdate, id: accesskit::NodeId) -> &accesskit::Node {
    update
        .nodes
        .iter()
        .find(|(node_id, _)| *node_id == id)
        .map(|(_, node)| node)
        .unwrap_or_else(|| panic!("missing accessibility node {id:?}"))
}

fn a11y_node_with_role_and_label<'a>(
    update: &'a accesskit::TreeUpdate,
    role: accesskit::Role,
    label: &str,
) -> (accesskit::NodeId, &'a accesskit::Node) {
    update
        .nodes
        .iter()
        .find(|(_, node)| node.role() == role && node.label() == Some(label))
        .map(|(id, node)| (*id, node))
        .unwrap_or_else(|| panic!("missing {role:?} accessibility node labelled `{label}`"))
}

fn a11y_nodes_with_label(update: &accesskit::TreeUpdate, label: &str) -> Vec<accesskit::NodeId> {
    update
        .nodes
        .iter()
        .filter_map(|(node_id, node)| (node.label() == Some(label)).then_some(*node_id))
        .collect()
}

fn accesskit_live_label(live: Option<accesskit::Live>) -> Option<&'static str> {
    live.map(|live| match live {
        accesskit::Live::Off => "off",
        accesskit::Live::Polite => "polite",
        accesskit::Live::Assertive => "assertive",
    })
}

fn a11y_text_run_child(
    update: &accesskit::TreeUpdate,
    control: &accesskit::Node,
) -> accesskit::NodeId {
    control
        .children()
        .iter()
        .copied()
        .find(|id| a11y_node_by_id(update, *id).role() == accesskit::Role::TextRun)
        .expect("editable text control should publish a TextRun child")
}

fn action_request(
    action: accesskit::Action,
    target_node: accesskit::NodeId,
    data: Option<accesskit::ActionData>,
) -> accesskit::ActionRequest {
    accesskit::ActionRequest {
        action,
        target_tree: accesskit::TreeId::ROOT,
        target_node,
        data,
    }
}

fn assert_tree_excludes_text(update: &accesskit::TreeUpdate, canary: &str) {
    assert!(
        !format!("{update:#?}").contains(canary),
        "Gallery accessibility tree leaked password canary {canary:?}"
    );
}

fn devtools_semantic_payload(scenario_id: &str, contract_id: &str) -> serde_json::Value {
    let collection = pages::devtools::devtools_gallery_collection();
    let accessibility = collection
        .snapshots
        .iter()
        .find(|snapshot| snapshot.probe_id.as_str() == "accessibility")
        .expect("Gallery DevTools accessibility snapshot");
    let root = accessibility
        .tree
        .nodes
        .first()
        .expect("Gallery DevTools accessibility root");
    let scenario = root
        .children
        .iter()
        .find(|node| {
            node.payload
                .as_ref()
                .and_then(|payload| payload["scenario_id"].as_str())
                == Some(scenario_id)
        })
        .unwrap_or_else(|| panic!("missing DevTools accessibility scenario `{scenario_id}`"));

    scenario
        .children
        .iter()
        .find_map(|node| {
            node.payload
                .as_ref()
                .filter(|payload| payload["contract_id"].as_str() == Some(contract_id))
        })
        .cloned()
        .unwrap_or_else(|| {
            panic!("missing DevTools semantic payload `{contract_id}` in `{scenario_id}`")
        })
}

fn assert_devtools_semantics_match_final_node(
    payload: &serde_json::Value,
    expected_role: &str,
    node: &accesskit::Node,
) {
    assert_eq!(payload["role"].as_str(), Some(expected_role));
    for (field, final_value_present) in [
        ("label", node.label().is_some()),
        ("description", node.description().is_some()),
        ("value", node.value().is_some()),
        ("placeholder", node.placeholder().is_some()),
    ] {
        assert_eq!(
            payload["text"][field]["kind"].as_str() != Some("absent"),
            final_value_present,
            "DevTools/final-tree sensitive marker parity failed for `{field}`"
        );
    }
    assert_eq!(
        payload["state"]["required"].as_bool().unwrap_or(false),
        node.is_required()
    );
    assert_eq!(
        payload["state"]["invalid"].as_bool().unwrap_or(false),
        matches!(node.invalid(), Some(accesskit::Invalid::True))
    );
    assert_eq!(
        payload["state"]["busy"].as_bool().unwrap_or(false),
        node.is_busy()
    );
    assert_eq!(
        payload["state"]["live"].as_str(),
        accesskit_live_label(node.live())
    );
    assert_eq!(
        payload["state"]["live_atomic"].as_bool().unwrap_or(false),
        node.is_live_atomic()
    );
    assert_eq!(
        payload["state"]["read_only"].as_bool().unwrap_or(false),
        node.is_read_only()
    );
    assert_eq!(
        payload["state"]["disabled"].as_bool().unwrap_or(false),
        node.is_disabled()
    );
    assert_eq!(
        payload["relations"]["controls_count"].as_u64(),
        Some(node.controls().len() as u64)
    );
    assert_eq!(
        payload["relations"]["labelled_by_count"].as_u64(),
        Some(node.labelled_by().len() as u64)
    );
    assert_eq!(
        payload["relations"]["described_by_count"].as_u64(),
        Some(node.described_by().len() as u64)
    );
    assert_eq!(
        payload["relations"]["error_message"]["kind"].as_str() == Some("present"),
        node.error_message().is_some()
    );

    let projected_actions = payload["actions"]
        .as_array()
        .expect("DevTools semantic actions");
    for (label, action) in [
        ("focus", accesskit::Action::Focus),
        (
            "replace-selected-text",
            accesskit::Action::ReplaceSelectedText,
        ),
        ("set-text-selection", accesskit::Action::SetTextSelection),
        ("set-value", accesskit::Action::SetValue),
    ] {
        assert_eq!(
            projected_actions
                .iter()
                .any(|projected| projected.as_str() == Some(label)),
            node.supports_action(action),
            "DevTools/final-tree action parity failed for `{label}`"
        );
    }
}

#[open_gpui::test]
fn focus_a11y_devtools_allowlist_matches_final_tree_structure(cx: &mut open_gpui::TestAppContext) {
    let cx = open_gallery_page(cx, GalleryPage::FocusAccessibility);
    assert!(cx.activate_accessibility());
    let update = cx
        .latest_accessibility_tree_update()
        .expect("Focus/A11y page should publish a final accessibility tree");

    let (_, input) = a11y_node_with_role_and_label(
        &update,
        accesskit::Role::TextInput,
        pages::focus_a11y::TEXT_INPUT_LABEL,
    );
    let input_payload = devtools_semantic_payload(
        pages::focus_a11y::FocusA11yScenarioId::TextInputValueSelection.as_str(),
        "TextInput",
    );
    assert_devtools_semantics_match_final_node(&input_payload, "text-input", input);

    let textarea = update
        .nodes
        .iter()
        .find(|(_, node)| node.role() == accesskit::Role::MultilineTextInput)
        .map(|(_, node)| node)
        .expect("Textarea Field should publish its control node");
    let textarea_payload = devtools_semantic_payload(
        pages::focus_a11y::FocusA11yScenarioId::TextareaFieldRelations.as_str(),
        "Textarea",
    );
    assert_devtools_semantics_match_final_node(&textarea_payload, "multiline-text-input", textarea);

    let (_, password) = a11y_node_with_role_and_label(
        &update,
        accesskit::Role::PasswordInput,
        pages::focus_a11y::PASSWORD_LABEL,
    );
    let password_payload = devtools_semantic_payload(
        pages::focus_a11y::FocusA11yScenarioId::PasswordFreeTextRedaction.as_str(),
        "TextInput",
    );
    assert_devtools_semantics_match_final_node(&password_payload, "password-input", password);
    let (_, status) = a11y_node_with_role_and_label(
        &update,
        accesskit::Role::Status,
        pages::focus_a11y::LIVE_STATUS_IDLE_TEXT,
    );
    let status_payload = devtools_semantic_payload(
        pages::focus_a11y::FocusA11yScenarioId::LiveRegionsAndAnnouncements.as_str(),
        "StatusCue",
    );
    assert_devtools_semantics_match_final_node(&status_payload, "status", status);
    let serialized = serde_json::to_string(&[
        input_payload,
        textarea_payload,
        password_payload,
        status_payload,
    ])
    .expect("DevTools semantic payloads serialize");
    for sensitive_text in pages::focus_a11y::FOCUS_A11Y_SENSITIVE_TEXT {
        assert!(
            !serialized.contains(sensitive_text),
            "DevTools semantic payload leaked `{sensitive_text}`"
        );
    }
}

#[open_gpui::test]
fn focus_a11y_text_input_dispatches_set_value_and_selection_on_the_same_final_node(
    cx: &mut open_gpui::TestAppContext,
) {
    let cx = open_gallery_page(cx, GalleryPage::FocusAccessibility);
    let story = focus_a11y_story("text-input-value-selection");
    {
        let mut probe = StoryRuntimeProbe::new(cx);
        probe.assert_story_declares(&story, StoryProbeOperation::Edit);
        probe.assert_story_declares(&story, StoryProbeOperation::Focus);
        let selector = story
            .selectors()
            .sample_selector()
            .expect("TextInput story should own its component selector");
        probe.scroll_page_to(selector);
        probe.assert_rendered(selector, "Focus/A11y TextInput");
    }

    assert!(cx.activate_accessibility());
    let initial = cx
        .latest_accessibility_tree_update()
        .expect("Focus/A11y TextInput should publish a final accessibility tree");
    let (input_id, input) = a11y_node_with_role_and_label(
        &initial,
        accesskit::Role::TextInput,
        "Editable account name",
    );
    assert!(input.supports_action(accesskit::Action::SetValue));
    assert!(input.supports_action(accesskit::Action::SetTextSelection));

    const UPDATED_VALUE: &str = "gallery account updated";
    assert!(cx.dispatch_accessibility_action(action_request(
        accesskit::Action::SetValue,
        input_id,
        Some(accesskit::ActionData::Value(UPDATED_VALUE.into())),
    )));
    settle(cx);
    let updated = cx
        .latest_accessibility_tree_update()
        .expect("TextInput SetValue should publish an updated final tree");
    let updated_input = a11y_node_by_id(&updated, input_id);
    assert_eq!(updated_input.value(), Some(UPDATED_VALUE));

    let text_run_id = a11y_text_run_child(&updated, updated_input);
    let selection = accesskit::TextSelection {
        anchor: accesskit::TextPosition {
            node: text_run_id,
            character_index: 8,
        },
        focus: accesskit::TextPosition {
            node: text_run_id,
            character_index: 15,
        },
    };
    assert!(cx.dispatch_accessibility_action(action_request(
        accesskit::Action::SetTextSelection,
        input_id,
        Some(accesskit::ActionData::SetTextSelection(selection)),
    )));
    settle(cx);
    let selected = cx
        .latest_accessibility_tree_update()
        .expect("TextInput SetTextSelection should publish an updated final tree");
    assert_eq!(
        a11y_node_by_id(&selected, input_id).text_selection(),
        Some(&selection)
    );
}

#[open_gpui::test]
fn focus_a11y_textarea_field_switches_help_and_error_relations_on_the_same_final_node(
    cx: &mut open_gpui::TestAppContext,
) {
    let cx = open_gallery_page(cx, GalleryPage::FocusAccessibility);
    let story = focus_a11y_story("textarea-field-relations");
    let control_selector = story
        .selectors()
        .control_selector()
        .expect("Textarea Field story should own its transition control");
    {
        let mut probe = StoryRuntimeProbe::new(cx);
        probe.assert_story_declares(&story, StoryProbeOperation::Activate);
        probe.scroll_page_to(control_selector);
        probe.assert_rendered(control_selector, "Textarea Field relation toggle");
    }

    assert!(cx.activate_accessibility());
    let initial = cx
        .latest_accessibility_tree_update()
        .expect("Textarea Field should publish a final accessibility tree");
    let (label_id, _) =
        a11y_node_with_role_and_label(&initial, accesskit::Role::Label, "Release notes");
    let (help_id, _) = a11y_node_with_role_and_label(
        &initial,
        accesskit::Role::Label,
        "Summarize user-visible changes.",
    );
    let (textarea_id, textarea) = initial
        .nodes
        .iter()
        .find(|(_, node)| node.role() == accesskit::Role::MultilineTextInput)
        .map(|(id, node)| (*id, node))
        .expect("Textarea Field should publish its control node");
    assert_eq!(textarea.labelled_by(), &[label_id]);
    assert_eq!(textarea.described_by(), &[help_id]);
    assert_eq!(textarea.error_message(), None);
    assert_eq!(textarea.invalid(), None);

    {
        let mut probe = StoryRuntimeProbe::new(cx);
        probe.click(control_selector);
        probe.settle();
    }
    let invalid = cx
        .latest_accessibility_tree_update()
        .expect("Textarea Field error transition should publish a final tree");
    let invalid_textarea = a11y_node_by_id(&invalid, textarea_id);
    let (error_id, _) = a11y_node_with_role_and_label(
        &invalid,
        accesskit::Role::Alert,
        "Add a concise release note.",
    );
    let invalid_error = a11y_node_by_id(&invalid, error_id);
    assert_eq!(invalid_error.value(), Some("Add a concise release note."));
    assert_eq!(invalid_error.live(), Some(accesskit::Live::Assertive));
    assert!(invalid_error.is_live_atomic());
    assert_eq!(invalid_textarea.labelled_by(), &[label_id]);
    assert!(invalid_textarea.described_by().is_empty());
    assert_eq!(invalid_textarea.error_message(), Some(error_id));
    assert_eq!(invalid_textarea.invalid(), Some(accesskit::Invalid::True));
    assert!(!invalid.nodes.iter().any(|(id, _)| *id == help_id));

    {
        let mut probe = StoryRuntimeProbe::new(cx);
        probe.click(control_selector);
        probe.settle();
    }
    let restored = cx
        .latest_accessibility_tree_update()
        .expect("Textarea Field restore should publish a final tree");
    let restored_textarea = a11y_node_by_id(&restored, textarea_id);
    assert_eq!(restored_textarea.labelled_by(), &[label_id]);
    assert_eq!(restored_textarea.described_by(), &[help_id]);
    assert_eq!(restored_textarea.error_message(), None);
    assert!(!restored.nodes.iter().any(|(id, _)| *id == error_id));
}

#[open_gpui::test]
fn focus_a11y_live_regions_commit_busy_content_and_alert_without_stealing_focus(
    cx: &mut open_gpui::TestAppContext,
) {
    let cx = open_gallery_page(cx, GalleryPage::FocusAccessibility);
    let story = focus_a11y_story("live-regions-and-announcements");
    {
        let mut probe = StoryRuntimeProbe::new(cx);
        probe.assert_story_declares(&story, StoryProbeOperation::Activate);
        probe.assert_story_declares(&story, StoryProbeOperation::Focus);
        probe.assert_story_declares(&story, StoryProbeOperation::ReadPublicPayload);
        probe.scroll_page_to(pages::focus_a11y::LIVE_STATUS_UPDATE_SELECTOR);
        probe.assert_rendered(
            pages::focus_a11y::LIVE_STATUS_UPDATE_SELECTOR,
            "live status update control",
        );
    }

    assert!(cx.activate_accessibility());
    let initial = cx
        .latest_accessibility_tree_update()
        .expect("initial live-region story tree");
    let (status_id, initial_status) = a11y_node_with_role_and_label(
        &initial,
        accesskit::Role::Status,
        pages::focus_a11y::LIVE_STATUS_IDLE_TEXT,
    );
    assert_eq!(
        initial_status.value(),
        Some(pages::focus_a11y::LIVE_STATUS_IDLE_TEXT)
    );
    assert_eq!(initial_status.live(), Some(accesskit::Live::Off));
    assert!(initial_status.is_live_atomic());
    assert!(!initial_status.is_busy());

    {
        let mut probe = StoryRuntimeProbe::new(cx);
        probe.click(pages::focus_a11y::LIVE_STATUS_UPDATE_SELECTOR);
        probe.settle();
    }
    let first_update = cx.latest_accessibility_tree_update().unwrap();
    let first_text = "Background synchronization update 1.";
    let (first_id, first_status) =
        a11y_node_with_role_and_label(&first_update, accesskit::Role::Status, first_text);
    assert_eq!(first_id, status_id);
    assert_eq!(first_status.value(), Some(first_text));
    assert_eq!(first_status.live(), Some(accesskit::Live::Polite));
    assert!(first_status.is_live_atomic());
    assert!(!first_status.is_busy());
    let (update_button_id, _) =
        a11y_node_with_role_and_label(&first_update, accesskit::Role::Button, "Update status");
    assert_eq!(first_update.focus, update_button_id);

    {
        let mut probe = StoryRuntimeProbe::new(cx);
        probe.click(pages::focus_a11y::LIVE_BUSY_TOGGLE_SELECTOR);
        probe.settle();
    }
    let busy = cx.latest_accessibility_tree_update().unwrap();
    let (busy_id, busy_status) =
        a11y_node_with_role_and_label(&busy, accesskit::Role::Status, first_text);
    assert_eq!(busy_id, status_id);
    assert!(busy_status.is_busy());

    {
        let mut probe = StoryRuntimeProbe::new(cx);
        probe.click(pages::focus_a11y::LIVE_STATUS_UPDATE_SELECTOR);
        probe.settle();
    }
    let busy_changed = cx.latest_accessibility_tree_update().unwrap();
    let second_text = "Background synchronization update 2.";
    let (busy_changed_id, busy_changed_status) =
        a11y_node_with_role_and_label(&busy_changed, accesskit::Role::Status, second_text);
    assert_eq!(busy_changed_id, status_id);
    assert!(busy_changed_status.is_busy());
    assert_eq!(busy_changed.focus, update_button_id);

    {
        let mut probe = StoryRuntimeProbe::new(cx);
        probe.click(pages::focus_a11y::LIVE_BUSY_TOGGLE_SELECTOR);
        probe.settle();
    }
    let settled = cx.latest_accessibility_tree_update().unwrap();
    let (settled_id, settled_status) =
        a11y_node_with_role_and_label(&settled, accesskit::Role::Status, second_text);
    assert_eq!(settled_id, status_id);
    assert!(!settled_status.is_busy());

    {
        let mut probe = StoryRuntimeProbe::new(cx);
        probe.click(pages::focus_a11y::LIVE_ALERT_TOGGLE_SELECTOR);
        probe.settle();
    }
    let alerted = cx.latest_accessibility_tree_update().unwrap();
    let (alert_id, alert) = a11y_node_with_role_and_label(
        &alerted,
        accesskit::Role::Alert,
        pages::focus_a11y::LIVE_ALERT_TEXT,
    );
    assert_eq!(alert.value(), Some(pages::focus_a11y::LIVE_ALERT_TEXT));
    assert_eq!(alert.live(), Some(accesskit::Live::Assertive));
    assert!(alert.is_live_atomic());
    assert!(!alert.supports_action(accesskit::Action::Focus));
    assert!(!alert.supports_action(accesskit::Action::Click));
    assert!(!settled_status.supports_action(accesskit::Action::Focus));
    assert!(!settled_status.supports_action(accesskit::Action::Click));
    assert_ne!(alerted.focus, alert_id);
    assert_ne!(alerted.focus, status_id);
    let (alert_button_id, _) =
        a11y_node_with_role_and_label(&alerted, accesskit::Role::Button, "Clear alert");
    assert_eq!(alerted.focus, alert_button_id);
}

#[open_gpui::test]
fn focus_a11y_same_text_window_announcements_commit_distinct_generations(
    cx: &mut open_gpui::TestAppContext,
) {
    let cx = open_gallery_page(cx, GalleryPage::FocusAccessibility);
    {
        let mut probe = StoryRuntimeProbe::new(cx);
        probe.scroll_page_to(pages::focus_a11y::WINDOW_ANNOUNCEMENT_SELECTOR);
        probe.assert_rendered(
            pages::focus_a11y::WINDOW_ANNOUNCEMENT_SELECTOR,
            "window announcement control",
        );
    }
    assert!(cx.activate_accessibility());
    let history_start = cx.accessibility_tree_update_history().len();

    for _ in 0..2 {
        let mut probe = StoryRuntimeProbe::new(cx);
        probe.click(pages::focus_a11y::WINDOW_ANNOUNCEMENT_SELECTOR);
        probe.settle();
    }

    let committed_ids = cx.accessibility_tree_update_history()[history_start..]
        .iter()
        .flat_map(|update| {
            a11y_nodes_with_label(update, pages::focus_a11y::WINDOW_ANNOUNCEMENT_TEXT)
        })
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(
        committed_ids.len(),
        2,
        "equal announcement text must receive a new semantic identity"
    );

    let diagnostics =
        cx.update(|window, _| window.accessibility_announcement_diagnostics().to_vec());
    let accepted_sequences = diagnostics
        .iter()
        .filter(|diagnostic| {
            diagnostic.lifecycle() == open_gpui::AccessibilityAnnouncementLifecycle::Accepted
        })
        .filter_map(|diagnostic| diagnostic.sequence())
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(accepted_sequences.len(), 2);
    assert!(!format!("{diagnostics:?}").contains(pages::focus_a11y::WINDOW_ANNOUNCEMENT_TEXT));

    let latest = cx.latest_accessibility_tree_update().unwrap();
    let (button_id, _) =
        a11y_node_with_role_and_label(&latest, accesskit::Role::Button, "Announce completion");
    assert_eq!(latest.focus, button_id);
}

#[open_gpui::test]
fn focus_a11y_inactive_window_announcement_is_dropped_without_replay(
    cx: &mut open_gpui::TestAppContext,
) {
    let cx = open_gallery_page(cx, GalleryPage::FocusAccessibility);
    {
        let mut probe = StoryRuntimeProbe::new(cx);
        probe.scroll_page_to(pages::focus_a11y::WINDOW_ANNOUNCEMENT_SELECTOR);
        probe.click(pages::focus_a11y::WINDOW_ANNOUNCEMENT_SELECTOR);
        probe.settle();
    }

    let diagnostics =
        cx.update(|window, _| window.accessibility_announcement_diagnostics().to_vec());
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.sequence().is_none()
            && diagnostic.lifecycle()
                == open_gpui::AccessibilityAnnouncementLifecycle::Dropped(
                    open_gpui::AccessibilityAnnouncementDropReason::AccessibilityInactive,
                )
    }));
    assert!(cx.activate_accessibility());
    assert!(cx.accessibility_tree_update_history().iter().all(|update| {
        a11y_nodes_with_label(update, pages::focus_a11y::WINDOW_ANNOUNCEMENT_TEXT).is_empty()
    }));
}

#[open_gpui::test]
fn focus_a11y_runtime_announcement_text_never_enters_devtools_artifacts(
    cx: &mut open_gpui::TestAppContext,
) {
    const CANARY: &str = "u14-gallery-runtime-announcement-canary-019f4ad7";

    let (shell, cx) = open_gallery_page_with_shell(cx, GalleryPage::FocusAccessibility);
    assert!(cx.activate_accessibility());
    let history_start = cx.accessibility_tree_update_history().len();
    let outcome = cx.update(|window, app| {
        window.announce(open_gpui::AccessibilityAnnouncement::polite(CANARY), app)
    });
    assert!(outcome.is_accepted());
    settle(cx);
    assert!(
        cx.accessibility_tree_update_history()[history_start..]
            .iter()
            .any(|update| !a11y_nodes_with_label(update, CANARY).is_empty()),
        "the test harness must observe the canary in a committed final tree"
    );

    let diagnostics =
        cx.update(|window, _| window.accessibility_announcement_diagnostics().to_vec());
    assert!(!format!("{diagnostics:?}").contains(CANARY));

    cx.update(|window, app| {
        shell.update(app, |shell, cx| shell.refresh_devtools(window, cx));
    });
    settle(cx);
    let inspector = cx.update(|_, app| shell.read(app).devtools_workbench().inspector_state());
    let live_capture = serde_json::to_string(&inspector.current_capture()).unwrap();
    let inspector_detail =
        serde_json::to_string(&inspector.selected_detail_json().unwrap()).unwrap();
    let inspector_copy = inspector.copy_selected_detail().unwrap().pretty_json;

    let artifacts = cx.update(|_, app| shell.read(app).devtools_workbench().artifacts());
    let session_export = serde_json::to_string(&artifacts.session_export).unwrap();
    let report = serde_json::to_string(&artifacts.report).unwrap();
    let report_markdown = artifacts.report.to_markdown();
    let session_record = artifacts.session_record.to_pretty_json().unwrap();
    let report_record = artifacts.report_record.to_pretty_json().unwrap();
    let fixture_root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("gallery crate is under examples")
        .parent()
        .expect("examples has a workspace parent")
        .join("crates")
        .join("devtools")
        .join("tests")
        .join("fixtures");
    let session_fixture = std::fs::read_to_string(fixture_root.join("gallery-session.json"))
        .expect("Gallery session fixture");
    let report_fixture = std::fs::read_to_string(fixture_root.join("gallery-report.json"))
        .expect("Gallery report fixture");

    for (channel, output) in [
        ("live capture", live_capture.as_str()),
        ("Inspector detail", inspector_detail.as_str()),
        ("Inspector copy", inspector_copy.as_str()),
        ("session export", session_export.as_str()),
        ("report", report.as_str()),
        ("report markdown", report_markdown.as_str()),
        ("session artifact", session_record.as_str()),
        ("report artifact", report_record.as_str()),
        ("session fixture", session_fixture.as_str()),
        ("report fixture", report_fixture.as_str()),
    ] {
        assert!(!output.contains(CANARY), "{channel} leaked `{CANARY}`");
    }
}

#[open_gpui::test]
fn focus_a11y_password_set_value_never_exposes_accessible_free_text(
    cx: &mut open_gpui::TestAppContext,
) {
    let cx = open_gallery_page(cx, GalleryPage::FocusAccessibility);
    let story = focus_a11y_story("password-free-text-redaction");
    {
        let mut probe = StoryRuntimeProbe::new(cx);
        probe.assert_story_declares(&story, StoryProbeOperation::Edit);
        let selector = story
            .selectors()
            .sample_selector()
            .expect("Password story should own its component selector");
        probe.scroll_page_to(selector);
        probe.assert_rendered(selector, "Focus/A11y password input");
    }

    assert!(cx.activate_accessibility());
    let initial = cx
        .latest_accessibility_tree_update()
        .expect("password input should publish a final accessibility tree");
    let (password_id, password) =
        a11y_node_with_role_and_label(&initial, accesskit::Role::PasswordInput, "Gallery password");
    let initial_canary = pages::focus_a11y::PASSWORD_REDACTION_CANARY;
    let masked = password
        .value()
        .expect("password should expose a masked value");
    assert_eq!(masked.chars().count(), initial_canary.chars().count());
    assert!(masked.chars().all(|character| character == '\u{2022}'));
    assert!(password.supports_action(accesskit::Action::SetValue));
    assert!(!password.supports_action(accesskit::Action::SetTextSelection));
    assert_tree_excludes_text(&initial, initial_canary);

    const UPDATED_CANARY: &str = "updated-gallery-password-canary";
    assert!(cx.dispatch_accessibility_action(action_request(
        accesskit::Action::SetValue,
        password_id,
        Some(accesskit::ActionData::Value(UPDATED_CANARY.into())),
    )));
    settle(cx);
    let updated = cx
        .latest_accessibility_tree_update()
        .expect("password SetValue should publish a masked final tree");
    let updated_password = a11y_node_by_id(&updated, password_id);
    let updated_masked = updated_password
        .value()
        .expect("updated password should expose a masked value");
    assert_eq!(
        updated_masked.chars().count(),
        UPDATED_CANARY.chars().count()
    );
    assert!(
        updated_masked
            .chars()
            .all(|character| character == '\u{2022}')
    );
    assert_tree_excludes_text(&updated, UPDATED_CANARY);
}
