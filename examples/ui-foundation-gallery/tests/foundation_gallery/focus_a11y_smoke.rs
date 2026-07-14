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
    let serialized = serde_json::to_string(&[input_payload, textarea_payload, password_payload])
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
        probe.assert_story_can(&story, StoryProbeOperation::Edit);
        probe.assert_story_can(&story, StoryProbeOperation::Focus);
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
        probe.assert_story_can(&story, StoryProbeOperation::Activate);
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
        accesskit::Role::Label,
        "Add a concise release note.",
    );
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
fn focus_a11y_password_set_value_never_exposes_accessible_free_text(
    cx: &mut open_gpui::TestAppContext,
) {
    let cx = open_gallery_page(cx, GalleryPage::FocusAccessibility);
    let story = focus_a11y_story("password-free-text-redaction");
    {
        let mut probe = StoryRuntimeProbe::new(cx);
        probe.assert_story_can(&story, StoryProbeOperation::Edit);
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
