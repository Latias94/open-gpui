use super::*;

#[test]
fn components_page_samples_expose_component_metadata() {
    let tokens = ThemeTokens::default();
    let catalog = pages::components::COMPONENT_CATALOG;
    let buttons = pages::components::button_samples(tokens);
    let badges = pages::components::badge_samples(tokens);
    let accordions = pages::components::accordion_samples(tokens);
    let collapsibles = pages::components::collapsible_samples(tokens);
    let sliders = pages::components::slider_samples(tokens);
    let number_inputs = pages::components::number_input_samples(tokens);
    let toggle_groups = pages::components::toggle_group_samples(tokens);
    let links = pages::components::link_samples(tokens);
    let breadcrumbs = pages::components::breadcrumb_samples(tokens);
    let tags = pages::components::tag_samples(tokens);
    let toast_stacks = pages::components::toast_stack_samples(tokens);
    let icon_buttons = pages::components::icon_button_samples(tokens);
    let separators = pages::components::separator_samples(tokens);
    let kbds = pages::components::kbd_samples(tokens);
    let progress = pages::components::progress_samples(tokens);
    let skeletons = pages::components::skeleton_samples(tokens);
    let avatars = pages::components::avatar_samples(tokens);
    let status_cues = pages::components::status_cue_samples(tokens);
    let empty_states = pages::components::empty_state_samples(tokens);
    let switches = pages::components::switch_samples(tokens);
    let checkboxes = pages::components::checkbox_samples(tokens);
    let radio_groups = pages::components::radio_group_samples(tokens);
    let toggles = pages::components::toggle_samples(tokens);
    let toolbars = pages::components::toolbar_samples(tokens);
    let sidebars = pages::components::sidebar_samples(tokens);
    let trees = pages::components::tree_samples(tokens);
    let listboxes = pages::components::listbox_samples(tokens);
    let selects = pages::components::select_samples(tokens);
    let comboboxes = pages::components::combobox_samples(tokens);
    let commands = pages::components::command_samples(tokens);
    let labels = pages::components::label_samples(tokens);
    let text_inputs = pages::components::text_input_samples(tokens);
    let textareas = pages::components::textarea_samples(tokens);
    let fields = pages::components::field_samples(tokens);
    let field_textareas = pages::components::field_textarea_samples(tokens);
    let scroll_areas = pages::components::scroll_area_samples(tokens);
    let splitters = pages::components::splitter_samples(tokens);
    let tables = pages::components::table_samples(tokens);
    let virtualized_lists = pages::components::virtualized_list_samples(tokens);

    let official_names = catalog
        .iter()
        .filter(|entry| entry.status == pages::components::ComponentCatalogStatus::Official)
        .map(|entry| entry.name)
        .collect::<std::collections::BTreeSet<_>>();
    let expected_official_names = COMPONENT_CONTRACT_ROWS
        .iter()
        .filter(|entry| entry.family().as_str() != "overlay")
        .map(|entry| entry.id().as_str())
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(
        official_names, expected_official_names,
        "Components catalog official rows should follow the component contract rows order"
    );
    assert!(catalog.iter().all(|entry| !entry.name.trim().is_empty()));
    assert!(
        catalog
            .iter()
            .filter(|entry| entry.status == pages::components::ComponentCatalogStatus::Official)
            .all(|entry| entry.state.is_some())
    );
    assert!(
        catalog
            .iter()
            .any(|entry| entry.name == "TextInputController"
                && entry.status == pages::components::ComponentCatalogStatus::AdapterOnly
                && entry.state.is_none())
    );
    assert!(catalog.iter().any(|entry| entry.name == "ToolbarItem"
        && entry.status == pages::components::ComponentCatalogStatus::InternalAnatomy));
    assert!(
        ["Separator", "Kbd", "Progress", "Skeleton", "Avatar"]
            .iter()
            .all(|name| catalog.iter().any(|entry| entry.name == *name
                && entry.status == pages::components::ComponentCatalogStatus::Official
                && entry.coverage == "exports / gallery / state tests"))
    );
    assert!(
        [
            "Accordion",
            "Collapsible",
            "Slider",
            "NumberInput",
            "ToggleGroup",
            "Link",
            "Breadcrumb",
            "Tag",
            "ToastStack"
        ]
        .iter()
        .all(|name| catalog.iter().any(|entry| entry.name == *name
            && entry.status == pages::components::ComponentCatalogStatus::Official
            && entry.sample_section_id() == "foundation-components"))
    );
    let state_contract_names: Vec<_> = catalog
        .iter()
        .filter(|entry| entry.status == pages::components::ComponentCatalogStatus::StateContract)
        .map(|entry| entry.name)
        .collect();
    assert_eq!(
        state_contract_names,
        vec!["TreeState", "VirtualizedListState"]
    );
    assert!(
        catalog
            .iter()
            .filter(|entry| entry.status == pages::components::ComponentCatalogStatus::StateContract)
            .all(|entry| entry.sample_selector.is_none() && entry.state_contract_selector.is_some())
    );

    assert_eq!(buttons.len(), 6);
    assert_eq!(buttons[0].id, "default");
    assert_eq!(
        buttons[3].state.colors().background().token(),
        semantic::DESTRUCTIVE
    );
    assert!(!buttons[5].state.activation_enabled());

    assert_eq!(badges.len(), 4);
    assert_eq!(badges[0].state.variant(), BadgeVariant::Default);
    assert!(badges[0].state.display_only());
    assert_eq!(badges[0].state.role(), None);
    assert_eq!(
        badges[2].state.colors().background().token(),
        semantic::DESTRUCTIVE
    );
    assert_eq!(badges[3].state.variant(), BadgeVariant::Outline);
    assert_eq!(badges[3].state.size(), Size::Small);

    assert_eq!(accordions.len(), 1);
    assert_eq!(accordions[0].id, "shipping");
    assert_eq!(accordions[0].state.role(), Role::Group);
    assert_eq!(accordions[0].state.mode().as_str(), "multiple");
    assert!(accordions[0].state.collapsible());
    assert_eq!(accordions[0].state.open_values(), ["scope", "risk"]);
    assert!(accordions[0].state.items()[2].disabled());
    assert!(!accordions[0].state.items()[2].activation_enabled());

    assert_eq!(collapsibles.len(), 1);
    assert_eq!(collapsibles[0].id, "release-notes");
    assert!(collapsibles[0].state.open());
    assert_eq!(collapsibles[0].state.trigger_role(), Role::Button);
    assert_eq!(collapsibles[0].state.content_role(), Role::Group);

    assert_eq!(sliders.len(), 2);
    assert_eq!(sliders[0].id, "volume");
    assert_eq!(sliders[0].state.role(), Role::Slider);
    assert_eq!(sliders[0].state.value(), 72.0);
    assert_eq!(sliders[0].state.min(), 0.0);
    assert_eq!(sliders[0].state.max(), 100.0);
    assert!(sliders[0].state.activation_enabled());
    assert_eq!(sliders[1].state.step(), 5.0);
    assert!(sliders[1].state.disabled());

    assert_eq!(number_inputs.len(), 2);
    assert_eq!(number_inputs[0].id, "workers");
    assert_eq!(number_inputs[0].state.role(), Role::SpinButton);
    assert_eq!(number_inputs[0].state.display_value(), "6");
    assert!(number_inputs[0].state.input_enabled());
    assert!(number_inputs[1].state.invalid());

    assert_eq!(toggle_groups.len(), 2);
    assert_eq!(toggle_groups[0].id, "alignment");
    assert_eq!(toggle_groups[0].state.role(), Role::Group);
    assert_eq!(toggle_groups[0].state.mode().as_str(), "single");
    assert!(toggle_groups[0].state.selection_required());
    assert_eq!(toggle_groups[0].state.selected_values(), ["left"]);
    assert_eq!(toggle_groups[0].state.focused_value(), Some("center"));
    assert!(toggle_groups[0].state.items()[2].disabled());
    assert_eq!(toggle_groups[1].state.mode().as_str(), "multiple");
    assert_eq!(toggle_groups[1].state.selected_values(), ["bold", "code"]);

    assert_eq!(links.len(), 2);
    assert_eq!(links[0].id, "docs");
    assert_eq!(links[0].state.role(), Role::Link);
    assert!(links[0].state.external());
    assert!(links[0].state.activation().is_some());
    assert!(links[1].state.disabled());
    assert!(links[1].state.activation().is_none());

    assert_eq!(breadcrumbs.len(), 1);
    assert_eq!(breadcrumbs[0].id, "project");
    assert_eq!(breadcrumbs[0].state.role(), Role::Navigation);
    assert_eq!(breadcrumbs[0].state.current_index(), Some(2));
    assert_eq!(breadcrumbs[0].state.items()[2].role(), Role::Label);
    assert!(breadcrumbs[0].state.items()[2].current());

    assert_eq!(tags.len(), 3);
    assert_eq!(tags[0].id, "ready");
    assert_eq!(tags[0].state.role(), Role::Label);
    assert!(tags[0].state.removable());
    assert!(tags[0].state.remove().is_some());
    assert_eq!(tags[1].state.variant(), BadgeVariant::Destructive);
    assert!(tags[2].state.disabled());
    assert!(tags[2].state.remove().is_none());

    assert_eq!(toast_stacks.len(), 1);
    assert_eq!(toast_stacks[0].id, "notifications");
    assert_eq!(toast_stacks[0].state.role(), Role::Section);
    assert_eq!(toast_stacks[0].state.max_visible(), 2);
    assert_eq!(toast_stacks[0].state.visible_toasts().len(), 2);
    assert_eq!(toast_stacks[0].state.overflow_count(), 0);
    assert_eq!(toast_stacks[0].state.expired_dismissals().len(), 1);
    assert_eq!(
        toast_stacks[0].state.expired_dismissals()[0]
            .reason()
            .as_str(),
        "timeout"
    );
    assert_eq!(toast_stacks[0].state.visible_toasts()[0].id(), "queued");
    assert_eq!(toast_stacks[0].state.visible_toasts()[1].id(), "saved");
    assert!(toast_stacks[0].state.visible_toasts()[1].action().is_some());

    assert_eq!(separators.len(), 3);
    assert_eq!(separators[0].id, "section-rule");
    assert_eq!(separators[0].state.role(), Some(Role::Separator));
    assert_eq!(separators[1].state.orientation(), Orientation::Vertical);
    assert_eq!(separators[1].state.metrics().thickness(), ui_px(2.0));
    assert!(separators[2].state.decorative());
    assert_eq!(separators[2].state.role(), None);

    assert_eq!(kbds.len(), 3);
    assert_eq!(kbds[0].id, "command-palette");
    assert_eq!(kbds[0].state.label(), "Ctrl+K");
    assert!(kbds[0].state.display_only());
    assert_eq!(kbds[2].state.size(), Size::Large);

    assert_eq!(progress.len(), 3);
    assert_eq!(progress[0].state.role(), Role::ProgressIndicator);
    assert_eq!(progress[0].state.value_percent(), Some(64.0));
    assert_eq!(progress[1].state.normalized_value(), Some(1.0));
    assert!(progress[2].state.indeterminate());

    assert_eq!(skeletons.len(), 3);
    assert_eq!(skeletons[0].id, "body-line");
    assert!(skeletons[0].state.display_only());
    assert!(skeletons[1].state.subtle());
    assert_eq!(skeletons[2].state.size(), Size::Large);

    assert_eq!(avatars.len(), 4);
    assert_eq!(avatars[0].state.fallback(), "AL");
    assert_eq!(avatars[1].state.fallback(), "ME");
    assert_eq!(avatars[1].state.accessible_label(), "Current user");
    assert!(avatars[2].state.has_source());
    assert_eq!(
        avatars[2].state.source().map(|source| source.uri()),
        Some("asset://avatars/katherine.png")
    );
    assert_eq!(avatars[3].state.fallback(), "?");

    assert_eq!(status_cues.len(), 3);
    assert_eq!(status_cues[0].id, "sync-warning");
    assert_eq!(status_cues[0].state.intent(), FeedbackIntent::Warning);
    assert_eq!(status_cues[0].state.role(), Role::Status);
    assert!(status_cues[0].state.display_only());
    assert_eq!(status_cues[0].state.size(), Size::Small);
    assert_eq!(status_cues[1].state.intent(), FeedbackIntent::Success);
    assert_eq!(status_cues[2].state.intent(), FeedbackIntent::Info);

    assert_eq!(empty_states.len(), 2);
    assert_eq!(empty_states[0].id, "no-results");
    assert_eq!(empty_states[0].state.intent(), FeedbackIntent::Neutral);
    assert_eq!(empty_states[0].state.role(), Role::Section);
    assert_eq!(
        empty_states[0].state.description(),
        Some("Adjust filters or clear the current query.")
    );
    assert_eq!(empty_states[1].state.intent(), FeedbackIntent::Danger);

    assert_eq!(icon_buttons.len(), 4);
    assert_eq!(icon_buttons[0].state.accessible_label(), "Search");
    assert_eq!(icon_buttons[0].state.role(), Role::Button);
    assert_eq!(icon_buttons[1].state.variant(), ButtonVariant::Outline);
    assert_eq!(
        icon_buttons[1].state.metrics().size(),
        Size::Small.icon_button_size()
    );
    assert_eq!(
        icon_buttons[2].state.colors().background().token(),
        semantic::DESTRUCTIVE
    );
    assert!(!icon_buttons[3].state.activation_enabled());

    assert_eq!(switches.len(), 4);
    assert_eq!(switches[0].state.role(), Role::Switch);
    assert_eq!(switches[0].state.toggled(), Toggled::False);
    assert_eq!(switches[1].state.toggled(), Toggled::True);
    assert!(!switches[3].state.activation_enabled());

    assert_eq!(checkboxes.len(), 6);
    assert_eq!(checkboxes[0].state.role(), Role::CheckBox);
    assert_eq!(checkboxes[0].state.toggled(), Toggled::False);
    assert_eq!(checkboxes[1].state.toggled(), Toggled::True);
    assert_eq!(checkboxes[2].state.toggled(), Toggled::Mixed);
    assert!(checkboxes[3].state.required());
    assert!(checkboxes[4].state.invalid());
    assert!(!checkboxes[5].state.activation_enabled());

    assert_eq!(radio_groups.len(), 2);
    assert_eq!(radio_groups[0].state.role(), Role::RadioGroup);
    assert!(radio_groups[0].state.required());
    assert_eq!(radio_groups[0].state.selected_value(), Some("team"));
    assert_eq!(radio_groups[0].state.focused_value(), Some("team"));
    assert!(radio_groups[0].state.items()[2].disabled());
    assert_eq!(radio_groups[0].state.items()[0].role(), Role::RadioButton);
    assert_eq!(radio_groups[1].state.orientation(), Orientation::Horizontal);
    assert_eq!(radio_groups[1].state.selected_value(), Some("europe"));

    assert_eq!(toggles.len(), 4);
    assert_eq!(toggles[0].state.role(), Role::Button);
    assert_eq!(toggles[0].state.toggled(), Toggled::False);
    assert_eq!(toggles[1].state.toggled(), Toggled::True);
    assert_eq!(toggles[2].state.variant(), ToggleVariant::Outline);
    assert!(!toggles[3].state.activation_enabled());

    assert_eq!(toolbars.len(), 2);
    assert_eq!(toolbars[0].id, "editor-toolbar");
    assert_eq!(toolbars[0].state.role(), Role::Toolbar);
    assert_eq!(toolbars[0].state.orientation(), Orientation::Horizontal);
    assert_eq!(toolbars[0].state.focused_value(), Some("bold"));
    assert_eq!(toolbars[0].state.items()[2].kind().as_str(), "separator");
    assert!(!toolbars[0].state.items()[2].focusable());
    assert!(toolbars[0].state.items()[3].pressed());
    assert_eq!(toolbars[1].state.orientation(), Orientation::Vertical);

    assert_eq!(sidebars.len(), 3);
    assert_eq!(sidebars[0].id, "workspace-sidebar");
    assert_eq!(sidebars[0].state.role(), Role::Navigation);
    assert_eq!(sidebars[0].state.side().as_str(), "left");
    assert_eq!(sidebars[0].state.variant().as_str(), "docked");
    assert_eq!(sidebars[0].state.collapse_mode().as_str(), "icon");
    assert_eq!(sidebars[0].state.size(), Size::Medium);
    assert_eq!(sidebars[0].state.label(), "Workspace navigation");
    assert_eq!(sidebars[0].state.selected_value(), Some("projects"));
    assert_eq!(sidebars[0].state.focused_value(), Some("projects"));
    assert_eq!(sidebars[0].state.sections()[0].role(), Role::Section);
    assert_eq!(sidebars[0].state.items()[1].badge_label(), Some("12"));
    assert!(!sidebars[0].state.items()[3].activation_enabled());
    assert_eq!(
        sidebars[0]
            .state
            .items()
            .iter()
            .filter(|item| item.duplicate_value())
            .count(),
        2
    );
    assert!(sidebars[1].state.icon_collapsed());
    assert_eq!(sidebars[1].state.items()[0].label(), "Home");
    assert_eq!(sidebars[2].state.side().as_str(), "right");
    assert!(sidebars[2].state.scrollable());
    assert!(sidebars[2].state.items().len() > 8);

    assert_eq!(trees.len(), 4);
    let tree = &trees[0];
    assert_eq!(tree.id, "document-outline");
    assert_eq!(tree.state.role(), Role::Tree);
    assert_eq!(tree.state.item_role(), Role::TreeItem);
    assert_eq!(tree.state.selected_value(), Some("paper"));
    assert_eq!(tree.state.focused_value(), Some("paper"));
    assert!(tree.state.items().len() > 12);
    assert!(matches!(
        tree.state.keyboard_action_for_key("right"),
        Some(TreeKeyboardAction::Toggle(toggle)) if toggle.value() == "paper" && toggle.expanded()
    ));
    assert_eq!(tree.build_tree().state().role(), Role::Tree);
    let remote_tree = &trees[1];
    assert_eq!(remote_tree.id, "remote-workspace");
    assert_eq!(remote_tree.state.selected_value(), Some("remote-src"));
    assert!(
        remote_tree
            .state
            .item_by_value("remote-src")
            .is_some_and(|item| item.children_unloaded() && item.loaded_child_count() == 0)
    );
    assert!(
        remote_tree
            .state
            .item_by_value("remote-crates")
            .is_some_and(|item| item.children_loading()
                && item.children_load_state().message() == Some("Loading child packages"))
    );
    assert!(
        remote_tree
            .state
            .item_by_value("remote-build")
            .is_some_and(|item| item.children_load_failed()
                && item.children_load_state().message() == Some("Network unavailable"))
    );
    let release_tree = &trees[2];
    assert_eq!(release_tree.id, "release-outline");
    assert!(release_tree.virtualized);
    assert_eq!(release_tree.viewport_item_count, 8);
    assert_eq!(release_tree.overscan_count, 4);
    assert_eq!(release_tree.state.items().len(), 240);
    let release_tree_snapshot = release_tree.behavior_snapshot();
    assert_eq!(release_tree_snapshot.state().items().len(), 240);
    assert_eq!(release_tree_snapshot.visible_row_count(), 8);
    assert_eq!(release_tree_snapshot.rendered_row_count(), 12);
    assert_eq!(
        release_tree_snapshot.rows()[0].render_key(),
        "0:release-node-0000"
    );
    let editable_tree = &trees[3];
    assert_eq!(editable_tree.id, "editable-outline");
    assert!(editable_tree.draggable);
    assert_eq!(editable_tree.state.selected_value(), Some("root"));
    assert_eq!(editable_tree.state.focused_value(), Some("root"));
    assert_eq!(editable_tree.state.items().len(), 4);

    assert_eq!(listboxes.len(), 2);
    assert_eq!(listboxes[0].id, "assignee-listbox");
    assert_eq!(listboxes[0].state.role(), Role::ListBox);
    assert_eq!(listboxes[0].state.selected_value(), Some("owen"));
    assert_eq!(listboxes[0].state.active_value(), Some("maya"));
    assert_eq!(
        listboxes[0].state.options()[0].role(),
        Some(Role::ListBoxOption)
    );
    assert!(
        listboxes[0]
            .state
            .options()
            .iter()
            .any(|option| option.disabled())
    );
    assert!(listboxes[1].state.empty());

    assert_eq!(selects.len(), 3);
    assert_eq!(selects[0].id, "priority-select");
    assert_eq!(selects[0].state.open_mode(), SelectOpenMode::Controlled);
    assert!(selects[0].state.open());
    assert_eq!(selects[0].state.trigger_role(), Role::Button);
    assert_eq!(selects[0].state.content_role(), Role::ListBox);
    assert!(selects[0].state.scrollable_content());
    assert_eq!(selects[1].state.trigger_label(), "Doing");
    assert!(selects[2].state.disabled());
    assert!(!selects[2].state.open());

    assert_eq!(comboboxes.len(), 3);
    assert_eq!(comboboxes[0].id, "framework-combobox");
    assert_eq!(
        comboboxes[0].state.open_mode(),
        ComboboxOpenMode::Controlled
    );
    assert!(comboboxes[0].state.open());
    assert_eq!(comboboxes[0].state.input_role(), Role::EditableComboBox);
    assert_eq!(comboboxes[0].state.content_role(), Role::ListBox);
    assert_eq!(comboboxes[0].state.filtered_option_count(), 3);
    assert_eq!(comboboxes[0].state.selected_value(), Some("solid"));
    assert_eq!(comboboxes[0].state.listbox().selected_value(), None);
    assert_eq!(comboboxes[1].state.filtered_option_count(), 0);
    assert!(comboboxes[1].state.listbox().empty());
    assert!(comboboxes[2].state.disabled());
    assert!(!comboboxes[2].state.open());

    assert_eq!(commands.len(), 9);
    assert_eq!(commands[0].id, "ranked-search");
    assert_eq!(commands[0].state.open_mode(), CommandOpenMode::Controlled);
    assert!(commands[0].state.loading().is_none());
    assert!(commands[0].state.open());
    assert!(commands[0].state.dialog().is_some());
    assert_eq!(commands[0].state.list_role(), Role::ListBox);
    assert_eq!(commands[0].state.selected_value(), Some("open-file"));
    assert_eq!(commands[0].state.filtered_item_count(), 3);
    assert!(
        commands[0]
            .state
            .items()
            .iter()
            .any(|item| item.shortcut().is_some())
    );
    assert_eq!(
        commands[1].state.selection_mode(),
        CommandSelectionMode::Multiple
    );
    assert_eq!(commands[1].state.selected_chips().len(), 2);
    assert_eq!(commands[1].state.filtered_item_count(), 1);
    assert_eq!(commands[2].state.total_item_count(), 10_000);
    assert_eq!(commands[2].state.filtered_item_count(), 10_000);
    assert_eq!(commands[2].viewport_item_count, 7);
    assert!(commands[3].state.loading().is_some());
    assert_eq!(
        commands[3].state.loading().unwrap().message(),
        "Refreshing command index"
    );
    assert_eq!(
        commands[3].state.index_revision(),
        Some("workspace-index-v3")
    );
    assert_eq!(
        commands[3].state.index_mode(),
        CommandIndexSnapshotMode::PreRankedFilter
    );
    assert_eq!(commands[4].id, "registry-dispatch");
    assert_eq!(
        commands[4].state.index_revision(),
        Some("gallery-command-center-v1")
    );
    assert_eq!(
        commands[4].state.index_mode(),
        CommandIndexSnapshotMode::PreRankedFilter
    );
    assert_eq!(
        commands[4].dispatched_command_id.as_deref(),
        Some("workspace.open")
    );
    assert!(commands[4].shortcut_diagnostics.is_empty());
    assert_eq!(
        commands[4]
            .state
            .group_items(0)
            .next()
            .and_then(|item| item.shortcut()),
        Some(display_shortcut("ctrl-shift-p").as_str())
    );
    assert_eq!(commands[5].id, "provider-search");
    assert_eq!(
        commands[5].state.index_revision(),
        Some("gallery-provider-center-v1")
    );
    assert_eq!(
        commands[5].state.index_mode(),
        CommandIndexSnapshotMode::PreFiltered
    );
    let provider_status = commands[5]
        .provider_status
        .as_ref()
        .expect("provider sample records provider status");
    assert_eq!(provider_status.provider_id().as_str(), "recent-provider");
    assert_eq!(
        provider_status
            .request_id()
            .map(|request_id| request_id.get()),
        Some(1)
    );
    assert_eq!(provider_status.query(), Some("alpha"));
    assert_eq!(provider_status.state(), CommandProviderState::Ready);
    assert_eq!(provider_status.source_count(), 1);
    assert_eq!(provider_status.command_count(), 2);
    assert!(commands[5].shortcut_diagnostics.is_empty());
    assert!(commands[5].state.status_items().is_empty());
    assert_eq!(commands[5].state.query(), "alpha");
    assert_eq!(commands[5].state.filtered_item_count(), 2);
    assert_eq!(commands[5].state.groups()[0].label(), "Provider");
    assert_eq!(
        commands[5]
            .state
            .group_items(0)
            .next()
            .map(|item| item.value()),
        Some("provider.open.alpha")
    );
    assert_eq!(
        commands[5]
            .state
            .group_items(0)
            .next()
            .and_then(|item| item.shortcut()),
        Some(display_shortcut("ctrl-alt-o").as_str())
    );
    assert_eq!(commands[6].id, "diagnostics-empty");
    assert_eq!(
        commands[6].state.index_revision(),
        Some("gallery-diagnostics-center-v1")
    );
    assert_eq!(commands[6].state.query(), "offline");
    assert_eq!(commands[6].state.filtered_item_count(), 0);
    assert_eq!(commands[6].state.status_error_count(), 1);
    assert_eq!(commands[6].state.status_warning_count(), 2);
    assert_eq!(
        commands[6].state.status_items()[0].intent(),
        CommandStatusIntent::Error
    );
    assert_eq!(commands[6].shortcut_diagnostics.len(), 2);
    let diagnostics_provider_status = commands[6]
        .provider_status
        .as_ref()
        .expect("diagnostics sample records failed provider status");
    assert_eq!(
        diagnostics_provider_status.provider_id().as_str(),
        "diagnostics-provider"
    );
    assert_eq!(
        diagnostics_provider_status.state(),
        CommandProviderState::Failed
    );
    assert_eq!(commands[7].id, "context-stack");
    assert_eq!(
        commands[7].state.index_revision(),
        Some("gallery-context-center-v1")
    );
    assert_eq!(
        commands[7].state.index_mode(),
        CommandIndexSnapshotMode::PreRankedFilter
    );
    assert_eq!(commands[7].state.query(), "focused");
    assert_eq!(commands[7].state.filtered_item_count(), 2);
    assert_eq!(
        commands[7].dispatched_command_id.as_deref(),
        Some("workspace.open")
    );
    assert!(commands[7].shortcut_diagnostics.is_empty());
    assert_eq!(
        commands[7]
            .state
            .items()
            .iter()
            .find(|item| item.value() == "workspace.open")
            .and_then(|item| item.shortcut()),
        Some(display_shortcut("ctrl-e").as_str())
    );
    assert_eq!(
        commands[7]
            .state
            .items()
            .iter()
            .find(|item| item.value() == "editor.format")
            .and_then(|item| item.shortcut()),
        Some(display_shortcut("ctrl-shift-f").as_str())
    );
    assert_eq!(commands[8].id, "keymap-resolution");
    assert_eq!(
        commands[8].state.index_revision(),
        Some("gallery-keymap-resolution-center-v1")
    );
    assert_eq!(
        commands[8].state.index_mode(),
        CommandIndexSnapshotMode::PreRankedFilter
    );
    assert_eq!(commands[8].state.query(), "keymap");
    assert_eq!(commands[8].state.filtered_item_count(), 2);
    assert_eq!(
        commands[8].dispatched_command_id.as_deref(),
        Some("workspace.open")
    );
    assert_eq!(commands[8].shortcut_diagnostics.len(), 1);
    assert_eq!(commands[8].state.status_warning_count(), 1);
    assert_eq!(commands[8].keymap_resolutions.len(), 5);

    let pending_chord = &commands[8].keymap_resolutions[0];
    assert_eq!(pending_chord.input_label(), "ctrl-k");
    assert!(pending_chord.is_pending());
    assert!(pending_chord.matched_commands().is_empty());
    assert!(
        pending_chord
            .pending_commands()
            .iter()
            .any(|command| command.command_id() == "workspace.open" && command.is_dispatchable())
    );
    assert!(
        pending_chord
            .pending_commands()
            .iter()
            .any(
                |command| command.command_id() == "workspace.save" && command.state().is_disabled()
            )
    );

    let open_chord = &commands[8].keymap_resolutions[1];
    assert_eq!(open_chord.input_label(), "ctrl-k ctrl-o");
    assert!(!open_chord.is_pending());
    assert_eq!(
        open_chord
            .primary_dispatchable_command()
            .map(|command| command.command_id()),
        Some("workspace.open")
    );

    let disabled_save = commands[8].keymap_resolutions[2]
        .primary_command()
        .expect("ctrl-s should resolve to the disabled save command");
    assert_eq!(disabled_save.command_id(), "workspace.save");
    assert!(disabled_save.state().is_disabled());
    assert_eq!(
        disabled_save.state().reason_ref(),
        Some("Workspace is read-only")
    );
    assert!(
        commands[8].keymap_resolutions[2]
            .primary_dispatchable_command()
            .is_none()
    );

    let hidden_command = commands[8].keymap_resolutions[3]
        .primary_command()
        .expect("ctrl-h should resolve to the hidden command");
    assert_eq!(hidden_command.command_id(), "workspace.hidden");
    assert!(hidden_command.state().is_hidden());
    assert!(
        commands[8].keymap_resolutions[3]
            .primary_dispatchable_command()
            .is_none()
    );

    let missing_command = commands[8].keymap_resolutions[4]
        .primary_command()
        .expect("ctrl-m should resolve to the missing command action");
    assert_eq!(missing_command.command_id(), "workspace.missing");
    assert!(missing_command.state().is_missing_command());

    let shortcut_inspector = commands[8]
        .shortcut_inspector
        .as_ref()
        .expect("keymap sample should expose shortcut inspector state");
    assert_eq!(shortcut_inspector.query(), "keymap");
    assert_eq!(shortcut_inspector.input_label(), "ctrl-k ctrl-o");
    assert!(!shortcut_inspector.is_pending());
    assert_eq!(
        shortcut_inspector.primary_dispatchable_command_id(),
        Some("workspace.open")
    );
    assert_eq!(shortcut_inspector.matched_commands().len(), 1);
    assert!(shortcut_inspector.pending_commands().is_empty());

    let keybinding_editor = commands[8]
        .keybinding_editor
        .as_ref()
        .expect("keymap sample should expose keybinding editor state");
    assert_eq!(
        keybinding_editor.mode(),
        CommandKeyBindingEditorFilterMode::ConflictsOnly
    );
    assert_eq!(keybinding_editor.query(), "workspace");
    assert_eq!(keybinding_editor.total_binding_count(), 2);
    assert_eq!(keybinding_editor.filtered_binding_count(), 2);
    assert_eq!(keybinding_editor.conflicts().len(), 1);
    assert_eq!(keybinding_editor.diagnostics().len(), 2);
    assert_eq!(
        keybinding_editor
            .rows()
            .iter()
            .map(|row| {
                (
                    row.source_id().to_owned(),
                    row.command_id().to_owned(),
                    row.keystrokes().to_owned(),
                    row.context_ref().map(str::to_owned),
                    row.conflict_count(),
                )
            })
            .collect::<Vec<_>>(),
        vec![
            (
                "gallery-defaults".to_string(),
                "workspace.open".to_string(),
                display_shortcut("ctrl-p"),
                Some("Workspace".to_string()),
                1,
            ),
            (
                "gallery-plugin".to_string(),
                "workspace.save".to_string(),
                display_shortcut("ctrl-p"),
                Some("Workspace".to_string()),
                1,
            ),
        ]
    );

    let keybinding_capture = commands[8]
        .keybinding_capture
        .as_ref()
        .expect("keymap sample should expose captured keybinding input");
    assert_eq!(keybinding_capture.raw_sequence(), "ctrl-k ctrl-s");
    assert_eq!(keybinding_capture.input_label(), Some("ctrl-k ctrl-s"));
    assert!(keybinding_capture.is_valid());

    let keybinding_edit_preview = commands[8]
        .keybinding_edit_preview
        .as_ref()
        .expect("keymap sample should expose keybinding edit preview");
    assert_eq!(
        keybinding_edit_preview.operation(),
        CommandKeyBindingPatchOperation::Replace
    );
    assert_eq!(
        keybinding_edit_preview.outcome(),
        CommandKeyBindingPatchOutcome::Replaced
    );
    assert!(keybinding_edit_preview.changed());
    assert!(!keybinding_edit_preview.is_strictly_clean());
    assert!(keybinding_edit_preview.editor().conflicts().is_empty());
    assert_eq!(keybinding_edit_preview.editor().diagnostics().len(), 2);
    assert_eq!(
        keybinding_edit_preview
            .editor()
            .rows()
            .iter()
            .find(|row| row.command_id() == "workspace.save")
            .map(|row| row.raw_keystrokes()),
        Some("ctrl-k ctrl-s")
    );

    assert_eq!(labels.len(), 4);
    assert_eq!(labels[0].state.role(), Role::Label);
    assert_eq!(labels[0].state.text(), "Email");
    assert!(labels[1].state.required());
    assert!(labels[2].state.disabled());
    assert_eq!(labels[3].state.size(), Size::Small);

    assert_eq!(text_inputs.len(), 6);
    assert_eq!(text_inputs[0].state.role(), Role::TextInput);
    assert!(text_inputs[0].state.controller_driven());
    assert!(
        text_inputs[1..]
            .iter()
            .all(|sample| !sample.state.controller_driven())
    );
    assert!(text_inputs[0].state.displaying_placeholder());
    assert!(text_inputs[1].state.has_value());
    assert_eq!(
        text_inputs[2].state.colors().border().token(),
        semantic::DESTRUCTIVE
    );
    assert!(!text_inputs[3].state.editable());
    assert!(!text_inputs[4].state.editable());
    assert_eq!(
        text_inputs[5].state.display_mode(),
        TextInputDisplayMode::Password
    );
    assert_eq!(text_inputs[5].state.display_text().as_ref(), "•••");

    assert_eq!(textareas.len(), 4);
    assert_eq!(textareas[0].state.role(), Role::MultilineTextInput);
    assert!(textareas[0].state.displaying_placeholder());
    assert!(!textareas[0].state.controller_driven());
    assert_eq!(textareas[1].state.value(), "Line 1\nLine 2");
    assert_eq!(textareas[1].state.rows(), 4);
    assert_eq!(textareas[2].state.rows(), 3);
    assert!(textareas[2].state.value().contains("Line 8"));
    assert!(textareas[3].state.required());
    assert!(textareas[3].state.invalid());

    assert_eq!(fields.len(), 3);
    assert!(fields[0].state.required());
    assert_eq!(
        fields[0].state.support_text().unwrap(),
        "Use a work address."
    );
    assert!(fields[1].state.support_is_error());
    assert_eq!(
        fields[1].state.support_text().unwrap(),
        "Enter a valid email."
    );
    assert!(fields[2].state.disabled());
    assert!(!fields[2].input_state.editable());

    assert_eq!(field_textareas.len(), 1);
    assert!(field_textareas[0].state.required());
    assert!(field_textareas[0].state.invalid());
    assert_eq!(field_textareas[0].textarea_state.rows(), 4);
    assert_eq!(
        field_textareas[0].textarea_state.role(),
        Role::MultilineTextInput
    );

    assert_eq!(scroll_areas.len(), 3);
    assert_eq!(scroll_areas[0].id, "activity-log");
    assert_eq!(scroll_areas[0].state.axis(), ScrollAreaAxis::Vertical);
    assert_eq!(
        scroll_areas[0].state.reset_policy(),
        ScrollResetPolicy::Preserve
    );
    assert_eq!(
        scroll_areas[0].state.metrics().scrollbar_width(),
        ui_px(10.0)
    );
    assert_eq!(scroll_areas[1].state.axis(), ScrollAreaAxis::Horizontal);
    assert_eq!(scroll_areas[1].state.reset_key(), None);
    assert_eq!(scroll_areas[2].state.axis(), ScrollAreaAxis::Both);
    assert_eq!(
        scroll_areas[2].state.reset_policy(),
        ScrollResetPolicy::ResetOnKeyChange
    );
    assert_eq!(scroll_areas[2].state.reset_key(), Some("components"));

    assert_eq!(splitters.len(), 2);
    assert_eq!(splitters[0].id, "workspace-split");
    assert_eq!(splitters[0].state.orientation(), Orientation::Horizontal);
    assert_eq!(splitters[0].state.size(), Size::Medium);
    assert_eq!(splitters[0].state.panels().len(), 3);
    assert_eq!(splitters[0].state.handles().len(), 2);
    assert_eq!(splitters[0].state.panels()[0].id(), "navigator");
    assert_eq!(splitters[0].state.panels()[0].min_fraction(), 0.18);
    assert_eq!(splitters[0].state.panels()[0].max_fraction(), 0.34);
    assert!(!splitters[0].state.handles()[0].disabled());
    assert_eq!(splitters[1].state.orientation(), Orientation::Vertical);
    assert_eq!(splitters[1].state.size(), Size::Small);
    assert!(splitters[1].state.panels()[0].collapsed());
    assert_eq!(splitters[1].state.panels()[0].collapsed_fraction(), 0.08);

    assert_eq!(tables.len(), 15);
    let release_queue = table_sample(tables, "release-queue");
    assert_eq!(release_queue.state.rows().len(), 10_000);
    assert_eq!(
        release_queue.state.sorting()[0].direction().as_str(),
        "descending"
    );
    let release_plan = release_queue.behavior_snapshot();
    assert_eq!(release_plan.aria_row_count(), 10_001);
    assert_eq!(release_plan.aria_column_count(), 4);
    assert!(release_plan.rendered_row_count() <= release_plan.visible_row_count() + 5);

    let filter_board = table_sample(tables, "filter-board");
    let filter_plan = filter_board.behavior_snapshot();
    let filter_summary = filter_board.state_summary();
    assert_eq!(filter_plan.row_counts().filtered_rows(), 60);
    assert_eq!(filter_plan.row_counts().final_rows(), 24);
    assert_eq!(filter_plan.row_counts().selected_rows(), 1);
    assert_eq!(filter_summary.facet_columns, 4);
    assert_eq!(filter_summary.manual_facet_columns, 0);
    assert_eq!(filter_summary.status_facet_values, 4);
    assert_eq!(filter_summary.status_facet_total_count, 60);
    assert_eq!(filter_summary.score_facet_min, Some(0));
    assert_eq!(filter_summary.score_facet_max, Some(177));

    let status_facet = filter_plan
        .column_facet(&TableColumnId::new("status"))
        .expect("filter-board status facet should resolve");
    assert_eq!(status_facet.mode(), TableStageMode::Client);
    assert_eq!(status_facet.row_count(), 60);
    assert_eq!(
        text_facet_counts(status_facet),
        [
            ("Doing".to_string(), 15),
            ("Done".to_string(), 15),
            ("Review".to_string(), 15),
            ("Todo".to_string(), 15),
        ],
        "client facets should ignore the target column filter and stay deterministic"
    );

    let score_facet = filter_plan
        .column_facet(&TableColumnId::new("score"))
        .expect("filter-board score facet should resolve");
    assert_eq!(score_facet.mode(), TableStageMode::Client);
    assert_eq!(score_facet.row_count(), 60);
    let score_range = score_facet
        .numeric_range()
        .expect("score facet should expose a numeric range");
    assert_eq!(score_range.min(), 0.0);
    assert_eq!(score_range.max(), 177.0);

    let server_paged = table_sample(tables, "server-paged");
    let server_page_plan = server_paged.behavior_snapshot();
    let server_page_summary = server_paged.state_summary();
    assert_eq!(server_paged.state.rows().len(), 8);
    assert_eq!(server_page_plan.filtering_mode(), TableStageMode::Manual);
    assert_eq!(server_page_plan.sorting_mode(), TableStageMode::Manual);
    assert_eq!(server_page_plan.pagination_mode(), TableStageMode::Manual);
    assert_eq!(server_page_summary.core_rows, 8);
    assert_eq!(server_page_summary.filtered_rows, 8);
    assert_eq!(server_page_summary.final_rows, 8);
    assert!(server_page_summary.manual_filtering);
    assert!(server_page_summary.manual_sorting);
    assert!(server_page_summary.manual_pagination);
    assert_eq!(server_page_summary.pagination_page_index, 2);
    assert_eq!(server_page_summary.pagination_page_size, 8);
    assert_eq!(server_page_summary.pagination_row_count, Some(64));
    assert_eq!(server_page_summary.pagination_page_count, Some(8));
    assert_eq!(server_page_summary.facet_columns, 4);
    assert_eq!(server_page_summary.manual_facet_columns, 2);
    assert_eq!(server_page_summary.status_facet_values, 4);
    assert_eq!(server_page_summary.status_facet_total_count, 64);
    assert_eq!(server_page_summary.score_facet_min, Some(1));
    assert_eq!(server_page_summary.score_facet_max, Some(64));
    assert_eq!(server_page_plan.pagination_row_count(), Some(64));
    assert_eq!(server_page_plan.pagination_page_count(), Some(8));
    assert_eq!(server_page_plan.faceting_mode(), TableStageMode::Client);
    assert_eq!(server_page_plan.column_facets().len(), 4);
    let server_status_facet = server_page_plan
        .column_facet(&TableColumnId::new("status"))
        .expect("server-paged status facet should resolve");
    assert_eq!(server_status_facet.mode(), TableStageMode::Manual);
    assert_eq!(server_status_facet.row_count(), 64);
    assert_eq!(
        text_facet_counts(server_status_facet),
        [
            ("Blocked".to_string(), 16),
            ("Queued".to_string(), 16),
            ("Ready".to_string(), 16),
            ("Review".to_string(), 16),
        ],
        "server facet payloads should stay visible in render-plan metadata"
    );
    let server_score_facet = server_page_plan
        .column_facet(&TableColumnId::new("score"))
        .expect("server-paged score facet should resolve");
    assert_eq!(server_score_facet.mode(), TableStageMode::Manual);
    assert_eq!(server_score_facet.row_count(), 64);
    let server_score_range = server_score_facet
        .numeric_range()
        .expect("server score facet should expose a numeric range");
    assert_eq!(server_score_range.min(), 1.0);
    assert_eq!(server_score_range.max(), 64.0);
    assert_eq!(
        server_page_plan
            .rows()
            .iter()
            .map(|row| {
                row.source_row_id()
                    .expect("server page row should be source-backed")
                    .as_str()
            })
            .collect::<Vec<_>>(),
        [
            "server-paged-row-0016",
            "server-paged-row-0017",
            "server-paged-row-0018",
            "server-paged-row-0019",
            "server-paged-row-0020",
            "server-paged-row-0021",
            "server-paged-row-0022",
            "server-paged-row-0023",
        ]
    );

    let release_resize = table_sample(tables, "release-resize");
    let resize_plan = release_resize.behavior_snapshot();
    assert_eq!(release_resize.state.rows().len(), 160);
    assert_eq!(
        resize_plan.columns()[0].width(),
        ui_px(188.0),
        "release-resize should expose committed sizing metadata"
    );
    assert!(
        resize_plan.columns()[0].resizable(),
        "name column should expose a resize handle"
    );
    assert!(
        !resize_plan.columns()[3].resizable(),
        "score column should prove per-column resize disablement"
    );

    let editable_release = table_sample(tables, "editable-release");
    let editable_plan = editable_release.behavior_snapshot();
    assert_eq!(editable_release.state.rows().len(), 32);
    assert!(
        editable_plan.columns()[0].text_editable(),
        "editable-release name column should expose text editing metadata"
    );
    assert!(
        editable_plan.columns()[1].text_editable(),
        "editable-release team column should expose text editing metadata"
    );
    assert!(
        !editable_plan.columns()[2].text_editable(),
        "editable-release status column should stay read-only"
    );
    assert!(
        editable_plan.rows()[0].cells()[0].text_editable(),
        "editable-release first visible name cell should render as editable"
    );

    let toggle_release = table_sample(tables, "toggle-release");
    let toggle_plan = toggle_release.behavior_snapshot();
    assert_eq!(toggle_release.state.rows().len(), 28);
    assert_eq!(
        toggle_plan.columns()[1].editor(),
        Some(TableCellEditor::Checkbox),
        "toggle-release enabled column should expose checkbox editing metadata"
    );
    assert_eq!(
        toggle_plan.rows()[0].cells()[1].editor(),
        Some(TableCellEditor::Checkbox),
        "toggle-release first visible enabled cell should render as a checkbox editor"
    );
    assert_eq!(
        toggle_plan.rows()[0]
            .cell(&TableColumnId::new("enabled"))
            .map(|cell| cell.text())
            .as_deref(),
        Some("true")
    );

    let multiline_release = table_sample(tables, "multiline-release");
    let multiline_plan = multiline_release.behavior_snapshot();
    assert_eq!(multiline_release.state.rows().len(), 24);
    assert_eq!(
        multiline_plan.columns()[1].editor(),
        Some(TableCellEditor::MultilineText { rows: 3 }),
        "multiline-release notes column should expose fixed-row multiline editing metadata"
    );
    assert_eq!(
        multiline_plan.rows()[0].cells()[1].editor(),
        Some(TableCellEditor::MultilineText { rows: 3 }),
        "multiline-release first visible notes cell should render as a multiline editor"
    );
    assert!(
        !multiline_plan.columns()[2].text_editable(),
        "multiline-release status column should stay read-only"
    );

    let select_release = table_sample(tables, "select-release");
    let select_plan = select_release.behavior_snapshot();
    assert_eq!(select_release.state.rows().len(), 28);
    assert_eq!(
        select_plan.columns()[1].editor(),
        Some(TableCellEditor::Select),
        "select-release status column should expose fixed-option select editing metadata"
    );
    assert_eq!(
        select_plan.rows()[0].cells()[1].editor(),
        Some(TableCellEditor::Select),
        "select-release first visible status cell should render as a select editor"
    );
    assert_eq!(
        select_plan.rows()[0].cells()[1].text(),
        "Ready",
        "select-release first visible status cell should resolve the display label from select options"
    );
    assert_eq!(
        select_plan.rows()[0].cells()[1].select_options().len(),
        2,
        "select-release visible select cell should carry the fixed option list"
    );

    let grouped_release = table_sample(tables, "release-rollup");
    let grouped_plan = grouped_release.behavior_snapshot();
    assert_eq!(grouped_release.state.grouping()[0].as_str(), "team");
    assert_eq!(grouped_release.state.aggregations().len(), 2);
    assert_eq!(
        grouped_release.state.column_pinning().left()[0].as_str(),
        "name"
    );
    assert_eq!(
        grouped_release.state.column_pinning().right()[0].as_str(),
        "status"
    );
    assert_eq!(
        grouped_plan.column_region_width(TableColumnRegion::Left),
        ui_px(188.0)
    );
    assert_eq!(
        grouped_plan.column_region_width(TableColumnRegion::Center),
        ui_px(400.0)
    );
    assert_eq!(
        grouped_plan.column_region_width(TableColumnRegion::Right),
        ui_px(164.0)
    );
    assert!(grouped_plan.uses_split_pinned_columns());
    assert_eq!(grouped_plan.column_regions().total_width(), ui_px(752.0));
    assert!(grouped_plan.row_counts().group_rows() >= 1);
    assert!(grouped_plan.row_counts().leaf_rows() > 0);
    assert!(
        grouped_plan.rendered_row_count()
            <= grouped_plan.visible_row_count() + grouped_release.overscan
    );

    let release_matrix = table_sample(tables, "release-matrix");
    let matrix_plan = release_matrix.behavior_snapshot();
    assert_eq!(release_matrix.state.rows().len(), 480);
    assert_eq!(
        release_matrix.state.column_pinning().left()[0].as_str(),
        "name"
    );
    assert_eq!(
        release_matrix.state.column_pinning().right()[0].as_str(),
        "status"
    );
    assert_eq!(
        release_matrix.state.sorting()[0].column().as_str(),
        "metric_13"
    );
    assert_eq!(
        matrix_plan.column_region_width(TableColumnRegion::Left),
        ui_px(172.0)
    );
    assert_eq!(
        matrix_plan.column_region_width(TableColumnRegion::Center),
        ui_px(1516.0)
    );
    assert_eq!(
        matrix_plan.column_region_width(TableColumnRegion::Right),
        ui_px(148.0)
    );
    assert!(matrix_plan.uses_split_pinned_columns());
    assert_eq!(matrix_plan.column_regions().total_width(), ui_px(1836.0));
    assert_eq!(matrix_plan.aria_column_count(), 16);
    assert_eq!(matrix_plan.columns().len(), 16);
    assert!(
        matrix_plan
            .columns()
            .iter()
            .any(|column| column.id().as_str() == "metric_13")
    );
    assert_eq!(matrix_plan.row_counts().selected_rows(), 1);

    let row_pinning = table_sample(tables, "row-pinning");
    let row_pinning_plan = row_pinning.behavior_snapshot();
    let row_pinning_summary = row_pinning.state_summary();
    assert_eq!(row_pinning.state.rows().len(), 96);
    assert_eq!(row_pinning_summary.core_rows, 96);
    assert_eq!(row_pinning_summary.final_rows, 14);
    assert_eq!(row_pinning_summary.pinned_top_rows, 1);
    assert_eq!(row_pinning_summary.pinned_center_rows, 11);
    assert_eq!(row_pinning_summary.pinned_bottom_rows, 2);
    assert!(!row_pinning_summary.row_pinning_page_only);
    assert_eq!(row_pinning_summary.pinned_left_columns, 1);
    assert_eq!(row_pinning_summary.pinned_center_columns, 14);
    assert_eq!(row_pinning_summary.pinned_right_columns, 1);
    assert_eq!(row_pinning_summary.pinned_left_width_px, 172);
    assert_eq!(row_pinning_summary.pinned_center_width_px, 1516);
    assert_eq!(row_pinning_summary.pinned_right_width_px, 148);
    assert_eq!(row_pinning_summary.total_column_width_px, 1836);
    assert!(row_pinning_plan.uses_split_pinned_columns());
    assert_eq!(row_pinning_plan.row_counts().pinned_center_rows(), 11);
    assert_eq!(row_pinning_plan.aria_row_count(), 15);
    assert_eq!(
        row_pinning_plan
            .rows_for_region(TableRowRegion::Top)
            .map(|row| {
                row.source_row_id()
                    .expect("top-pinned row should be source-backed")
                    .as_str()
            })
            .collect::<Vec<_>>(),
        ["row-pinning-row-003"]
    );
    assert_eq!(
        row_pinning_plan
            .rows_for_region(TableRowRegion::Center)
            .map(|row| {
                row.source_row_id()
                    .expect("center row should be source-backed")
                    .as_str()
            })
            .collect::<Vec<_>>(),
        [
            "row-pinning-row-024",
            "row-pinning-row-025",
            "row-pinning-row-026",
            "row-pinning-row-027",
            "row-pinning-row-028",
            "row-pinning-row-029",
            "row-pinning-row-031",
            "row-pinning-row-032",
            "row-pinning-row-033",
            "row-pinning-row-034",
            "row-pinning-row-035",
        ]
    );
    assert_eq!(
        row_pinning_plan
            .rows_for_region(TableRowRegion::Bottom)
            .map(|row| {
                row.source_row_id()
                    .expect("bottom-pinned row should be source-backed")
                    .as_str()
            })
            .collect::<Vec<_>>(),
        ["row-pinning-row-030", "row-pinning-row-070"]
    );

    let dependency_tree = table_sample(tables, "dependency-tree");
    let tree_plan = dependency_tree.behavior_snapshot();
    let tree_summary = dependency_tree.state_summary();
    assert_eq!(dependency_tree.state.rows().len(), 1);
    assert_eq!(tree_summary.core_rows, 7);
    assert_eq!(tree_summary.final_rows, 4);
    assert_eq!(tree_summary.tree_rows, 4);
    assert_eq!(tree_summary.tree_branch_rows, 3);
    assert_eq!(tree_summary.tree_depth, 1);
    assert_eq!(tree_summary.expanded_tree_inputs, 1);
    assert_eq!(tree_summary.pinned_left_columns, 1);
    assert_eq!(tree_summary.pinned_center_columns, 5);
    assert_eq!(tree_summary.pinned_right_columns, 1);
    assert_eq!(tree_summary.total_column_width_px, 956);
    assert_eq!(tree_plan.aria_column_count(), 7);
    assert_eq!(tree_plan.aria_row_count(), 5);
    assert_eq!(
        tree_plan
            .rows()
            .iter()
            .map(|row| {
                row.source_row_id()
                    .expect("dependency tree row should be source-backed")
                    .as_str()
            })
            .collect::<Vec<_>>(),
        [
            "dependency-workspace",
            "dependency-ui",
            "dependency-core",
            "dependency-docs"
        ]
    );
    assert_eq!(
        tree_plan
            .unique_source_row(&TableRowId::new("dependency-ui"))
            .and_then(|row| row.tree_expanded()),
        Some(false)
    );

    let server_tree = table_sample(tables, "server-tree");
    let server_plan = server_tree.behavior_snapshot();
    let server_summary = server_tree.state_summary();
    assert_eq!(
        server_tree.state.expansion_mode(),
        TableExpansionMode::Manual
    );
    assert_eq!(server_tree.state.rows().len(), 3);
    assert_eq!(server_summary.core_rows, 3);
    assert_eq!(server_summary.final_rows, 3);
    assert_eq!(server_summary.tree_rows, 3);
    assert_eq!(server_summary.tree_branch_rows, 3);
    assert_eq!(server_summary.unloaded_tree_branches, 1);
    assert_eq!(server_summary.loading_tree_rows, 1);
    assert_eq!(server_summary.failed_tree_rows, 1);
    assert!(server_summary.manual_expansion);
    assert_eq!(server_summary.expanded_tree_inputs, 0);
    assert_eq!(server_summary.pinned_left_columns, 1);
    assert_eq!(server_summary.pinned_center_columns, 5);
    assert_eq!(server_summary.pinned_right_columns, 1);
    assert_eq!(server_summary.total_column_width_px, 956);
    assert_eq!(server_plan.aria_column_count(), 7);
    assert_eq!(server_plan.aria_row_count(), 4);
    assert_eq!(
        server_plan
            .rows()
            .iter()
            .map(|row| {
                row.source_row_id()
                    .expect("server tree row should be source-backed")
                    .as_str()
            })
            .collect::<Vec<_>>(),
        ["server-workspace", "server-cache", "server-failed"]
    );

    assert_eq!(virtualized_lists.len(), 6);
    assert_eq!(
        virtualized_lists
            .iter()
            .map(|sample| sample.id)
            .collect::<Vec<_>>(),
        [
            "release-navigation",
            "primary-options",
            "section-status",
            "custom-renderer",
            "host-controlled-actions",
            "measured-notes"
        ]
    );
    let release_navigation = virtualized_lists
        .iter()
        .find(|sample| sample.id == "release-navigation")
        .expect("release navigation virtualized list sample");
    assert_eq!(release_navigation.id, "release-navigation");
    assert_eq!(release_navigation.items.len(), 10_000);
    assert_eq!(release_navigation.state.item_count(), 10_000);
    assert_eq!(release_navigation.state.active_index(), Some(0));
    assert_eq!(
        release_navigation.state.active_key(),
        Some("release-nav-0000")
    );
    assert_eq!(release_navigation.state.selected_index(), Some(0));
    assert_eq!(
        release_navigation.state.selected_keys(),
        ["release-nav-0000"]
    );
    let release_navigation_snapshot = release_navigation.behavior_snapshot();
    let release_navigation_summary = release_navigation.state_summary();
    let first_virtualized_row = &release_navigation_snapshot.rows()[0];
    assert_eq!(release_navigation_snapshot.role(), Role::ListBox);
    assert_eq!(release_navigation_snapshot.row_role(), Role::ListBoxOption);
    assert_eq!(first_virtualized_row.label(), "Release #0000");
    assert_eq!(
        first_virtualized_row.secondary_text(),
        Some("UI lane / Ready")
    );
    assert_eq!(first_virtualized_row.leading_metadata(), Some("UI"));
    assert_eq!(first_virtualized_row.badge(), Some("Ready"));
    assert_eq!(first_virtualized_row.status(), Some("On track"));
    assert_eq!(release_navigation_summary.item_count, 10_000);
    assert_eq!(release_navigation_summary.visible_start, 0);
    assert_eq!(release_navigation_summary.active_index, Some(0));
    assert_eq!(
        release_navigation_summary.active_key.as_deref(),
        Some("release-nav-0000")
    );
    assert_eq!(release_navigation_summary.selected_index, Some(0));
    assert_eq!(
        release_navigation_summary.selected_keys,
        vec!["release-nav-0000".to_owned()]
    );
    assert!(
        release_navigation_snapshot.rendered_row_count()
            <= release_navigation_snapshot.visible_row_count() + release_navigation.overscan
    );

    let custom_renderer = virtualized_lists
        .iter()
        .find(|sample| sample.id == "custom-renderer")
        .expect("custom renderer virtualized list sample");
    assert_eq!(
        custom_renderer.renderer,
        pages::components::VirtualizedListSampleRenderer::CompactMetadata
    );
    assert_eq!(
        custom_renderer.behavior_snapshot().row_role(),
        Role::ListBoxOption
    );

    let host_controlled_actions = virtualized_lists
        .iter()
        .find(|sample| sample.id == "host-controlled-actions")
        .expect("host-controlled actions virtualized list sample");
    assert_eq!(
        host_controlled_actions.renderer,
        pages::components::VirtualizedListSampleRenderer::NestedAction
    );
    assert_eq!(
        host_controlled_actions.host_reveal_key,
        Some("host-action-0010")
    );
    assert_eq!(
        host_controlled_actions.host_reveal_strategy,
        VirtualizedListScrollStrategy::Top
    );

    let measured_notes = virtualized_lists
        .iter()
        .find(|sample| sample.id == "measured-notes")
        .expect("measured virtualized list sample");
    assert_eq!(
        measured_notes.row_measure_mode,
        VirtualizedListRowMeasureMode::Measured
    );
    assert!(measured_notes.snapshot.is_some());
    assert!(
        measured_notes
            .behavior_snapshot()
            .rows()
            .iter()
            .any(|row| row.measured())
    );
}

#[test]
fn components_page_state_contract_samples_expose_tree_and_virtualized_list_contracts() {
    let tree_contracts = pages::components::tree_state_contract_samples();
    let virtualized_list_contracts = pages::components::virtualized_list_state_contract_samples();

    assert_eq!(tree_contracts.len(), 1);
    let tree = &tree_contracts[0].state;
    let tree_values = tree
        .items()
        .iter()
        .map(|item| item.value())
        .collect::<Vec<_>>();
    assert_eq!(
        tree_values,
        ["paper", "intro", "figures", "disabled", "notes"]
    );
    assert_eq!(tree.selected_index(), Some(1));
    assert_eq!(tree.focused_index(), Some(2));
    assert_eq!(tree.items()[3].position_in_set(), None);
    assert_eq!(
        tree.navigation_target("down").map(|item| item.value()),
        Some("notes")
    );
    assert!(matches!(
        tree.keyboard_action_for_key("right"),
        Some(TreeKeyboardAction::Toggle(_))
    ));
    assert!(matches!(
        tree.keyboard_action_for_key("enter"),
        Some(TreeKeyboardAction::Select(_))
    ));

    assert_eq!(virtualized_list_contracts.len(), 1);
    let virtualized = &virtualized_list_contracts[0];
    assert_eq!(
        virtualized.scroll_strategy,
        VirtualizedListScrollStrategy::Center
    );
    assert_eq!(virtualized.state.item_count(), 10_000);
    assert_eq!(virtualized.state.active_index(), Some(42));
    assert_eq!(virtualized.state.active_key(), Some("release-nav-0042"));
    assert_eq!(virtualized.state.selected_index(), Some(40));
    assert_eq!(virtualized.state.selected_keys(), ["release-nav-0040"]);
    assert_eq!(virtualized.state.viewport_item_count(), 12);
    assert_eq!(virtualized.state.navigation_target("pageup"), Some(30));
    assert_eq!(virtualized.state.navigation_target("pagedown"), Some(54));
    assert_eq!(
        virtualized
            .state
            .activation_for_key("space")
            .map(|activation| (activation.index(), activation.key().to_owned())),
        Some((42, "release-nav-0042".to_owned()))
    );
}

#[test]
fn gallery_contract_metadata_matches_component_rows() {
    use std::collections::{BTreeMap, BTreeSet};

    let canonical = COMPONENT_CONTRACT_ROWS
        .iter()
        .map(|entry| (entry.id().as_str(), entry.metadata()))
        .collect::<BTreeMap<_, _>>();
    assert_eq!(COMPONENT_CONTRACT_ROWS.len(), canonical.len());
    assert_eq!(canonical.len(), 48);

    let official_components = pages::components::COMPONENT_CATALOG
        .iter()
        .filter(|entry| entry.status == pages::components::ComponentCatalogStatus::Official)
        .collect::<Vec<_>>();
    let local_components = pages::components::COMPONENT_CATALOG
        .iter()
        .filter(|entry| entry.status != pages::components::ComponentCatalogStatus::Official)
        .collect::<Vec<_>>();
    assert_eq!(official_components.len(), 40);
    for entry in &official_components {
        let metadata = entry
            .component_contract()
            .expect("official component entry should carry canonical metadata");
        assert_eq!(entry.name, metadata.id().as_str());
        assert_eq!(entry.family, metadata.family().as_str());
    }
    assert!(
        local_components
            .iter()
            .all(|entry| entry.component_contract().is_none())
    );

    let projected = official_components
        .iter()
        .filter_map(|entry| entry.component_contract())
        .chain(
            pages::overlay::OVERLAY_CATALOG
                .iter()
                .map(|entry| entry.component_contract()),
        )
        .collect::<Vec<_>>();
    let projected_ids = projected
        .iter()
        .map(|metadata| metadata.id().as_str())
        .collect::<BTreeSet<_>>();
    assert_eq!(projected.len(), 48);
    assert_eq!(projected_ids.len(), projected.len());
    assert_eq!(projected_ids, canonical.keys().copied().collect());

    for metadata in projected {
        assert_eq!(
            Some(&metadata),
            canonical.get(metadata.id().as_str()),
            "Gallery metadata drifted for `{}`",
            metadata.id().as_str()
        );
    }

    for entry in official_components {
        let story = pages::components::component_story_contract_for(entry.name)
            .unwrap_or_else(|| panic!("missing official component story `{}`", entry.name));
        assert_eq!(story.component_contract(), entry.component_contract());
    }
    for story in pages::components::component_story_contracts()
        .into_iter()
        .filter(|story| {
            story.kind() == open_gpui_ui_foundation_gallery::StoryContractKind::StateContract
        })
    {
        assert_eq!(story.component_contract(), None);
    }

    let overlay_stories = pages::overlay::overlay_story_contracts();
    for entry in pages::overlay::OVERLAY_CATALOG {
        let metadata = entry.component_contract();
        assert_eq!(entry.name, metadata.id().as_str());
        assert_eq!(entry.family, metadata.family().as_str());
        assert_eq!(entry.family, "overlay");
        assert!(!entry.gallery_group.trim().is_empty());
        let story = overlay_stories
            .iter()
            .find(|story| story.owner_name() == entry.name)
            .unwrap_or_else(|| panic!("missing official overlay story `{}`", entry.name));
        assert_eq!(story.component_contract(), Some(entry.component_contract()));
    }
}

#[test]
fn gallery_story_contracts_derive_selectors_and_runtime_probes_from_gallery_owners() {
    use std::collections::{BTreeMap, BTreeSet};

    let component_stories = pages::components::component_story_contracts();
    let overlay_stories = pages::overlay::overlay_story_contracts();

    let expected_component_names = pages::components::COMPONENT_CATALOG
        .iter()
        .filter(|entry| {
            matches!(
                entry.status,
                pages::components::ComponentCatalogStatus::Official
                    | pages::components::ComponentCatalogStatus::StateContract
            )
        })
        .map(|entry| entry.name)
        .collect::<BTreeSet<_>>();
    let component_names = component_stories
        .iter()
        .map(|story| story.owner_name())
        .collect::<BTreeSet<_>>();
    assert_eq!(component_stories.len(), expected_component_names.len());
    assert_eq!(component_names, expected_component_names);
    assert_eq!(overlay_stories.len(), pages::overlay::OVERLAY_CATALOG.len());

    let official_pairs = pages::components::official_sample_selector_pairs().collect::<Vec<_>>();
    let unique_official_selectors = official_pairs
        .iter()
        .map(|(_, selector)| *selector)
        .collect::<BTreeSet<_>>();
    assert_eq!(official_pairs.len(), 40);
    assert_eq!(unique_official_selectors.len(), official_pairs.len());

    let state_pairs = pages::components::state_contract_readout_pairs().collect::<Vec<_>>();
    assert_eq!(
        state_pairs,
        vec![
            (
                "TreeState",
                "gallery:component-tree-state-contract:document-outline"
            ),
            (
                "VirtualizedListState",
                "gallery:component-virtualized-list-state-contract:release-navigation"
            ),
        ]
    );

    for story in component_stories.iter().chain(overlay_stories.iter()) {
        assert!(story.selectors().catalog_selector().is_some());
        assert!(story.selectors().primary_selector().is_some());
        assert!(!story.probes().is_empty());
        assert!(story.has_operation(StoryProbeOperation::ReadPublicPayload));
    }
    for story in &component_stories {
        assert_eq!(story.selectors().control_selector(), None);
        assert!(
            !story.has_operation(StoryProbeOperation::Focus),
            "component story `{}` cannot claim focus without a public control selector",
            story.owner_name(),
        );
    }

    let operation_counts = component_stories
        .iter()
        .chain(overlay_stories.iter())
        .flat_map(|story| story.probes().iter().map(|probe| probe.operation()))
        .fold(BTreeMap::new(), |mut counts, operation| {
            *counts.entry(operation).or_insert(0usize) += 1;
            counts
        });
    for operation in open_gpui_ui_foundation_gallery::STORY_PROBE_OPERATIONS {
        assert!(
            operation_counts.contains_key(operation),
            "Gallery stories do not exercise `{}`",
            operation.as_str()
        );
    }

    let table_story = component_stories
        .iter()
        .find(|story| story.owner_name() == "Table")
        .expect("Table story contract should exist");
    assert!(table_story.has_operation(StoryProbeOperation::Scroll));
    assert!(table_story.has_operation(StoryProbeOperation::Edit));
    assert!(table_story.has_operation(StoryProbeOperation::Open));

    for name in [
        "Badge",
        "Label",
        "StatusCue",
        "EmptyState",
        "Separator",
        "Kbd",
        "Progress",
        "Skeleton",
        "Avatar",
        "AvatarGroup",
    ] {
        let story = component_stories
            .iter()
            .find(|story| story.owner_name() == name)
            .unwrap_or_else(|| panic!("missing display story `{name}`"));
        assert_eq!(
            story
                .probes()
                .iter()
                .map(|probe| probe.operation())
                .collect::<Vec<_>>(),
            vec![StoryProbeOperation::ReadPublicPayload]
        );
    }

    for name in ["TextInput", "Textarea", "Field", "NumberInput", "Slider"] {
        let story = component_stories
            .iter()
            .find(|story| story.owner_name() == name)
            .unwrap_or_else(|| panic!("missing form control story `{name}`"));
        assert!(!story.has_operation(StoryProbeOperation::Focus));
        assert!(!story.has_operation(StoryProbeOperation::Edit));
    }

    for name in ["Accordion", "Collapsible"] {
        let story = component_stories
            .iter()
            .find(|story| story.owner_name() == name)
            .unwrap_or_else(|| panic!("missing static disclosure story `{name}`"));
        assert!(!story.has_operation(StoryProbeOperation::Open));
    }

    for name in [
        "Button",
        "IconButton",
        "Switch",
        "Checkbox",
        "Toggle",
        "Link",
        "Breadcrumb",
        "Tag",
        "ToastStack",
    ] {
        let story = component_stories
            .iter()
            .find(|story| story.owner_name() == name)
            .unwrap_or_else(|| panic!("missing static control story `{name}`"));
        assert!(!story.has_operation(StoryProbeOperation::Focus));
        assert!(!story.has_operation(StoryProbeOperation::Activate));
    }

    for name in [
        "Listbox",
        "RadioGroup",
        "ToggleGroup",
        "Tabs",
        "Select",
        "Combobox",
        "Command",
    ] {
        let story = component_stories
            .iter()
            .find(|story| story.owner_name() == name)
            .unwrap_or_else(|| panic!("missing controlled choice story `{name}`"));
        assert_eq!(
            story
                .probes()
                .iter()
                .map(|probe| probe.operation())
                .collect::<Vec<_>>(),
            vec![StoryProbeOperation::ReadPublicPayload]
        );
        assert!(!story.has_operation(StoryProbeOperation::Select));
        assert!(!story.has_operation(StoryProbeOperation::Focus));
        assert!(!story.has_operation(StoryProbeOperation::Open));
    }
    let toolbar = component_stories
        .iter()
        .find(|story| story.owner_name() == "Toolbar")
        .expect("missing Toolbar story");
    assert_eq!(
        toolbar
            .probes()
            .iter()
            .map(|probe| probe.operation())
            .collect::<Vec<_>>(),
        vec![StoryProbeOperation::ReadPublicPayload]
    );
    assert!(!toolbar.has_operation(StoryProbeOperation::Activate));
    assert!(!toolbar.has_operation(StoryProbeOperation::Open));
}

#[test]
fn choice_search_story_contracts_expose_state_readouts_and_product_metadata() {
    let expected = [
        (
            "Listbox",
            "ListboxState",
            "listbox",
            "gallery:component-listbox-sample:assignee-listbox",
            "gallery:component-listbox-sample:assignee-listbox:state",
        ),
        (
            "Select",
            "SelectState",
            "select",
            "gallery:component-select-sample:priority-select",
            "gallery:component-select-sample:priority-select:state",
        ),
        (
            "Combobox",
            "ComboboxState",
            "combobox",
            "gallery:component-combobox-sample:framework-combobox",
            "gallery:component-combobox-sample:framework-combobox:state",
        ),
        (
            "Command",
            "CommandState",
            "command",
            "gallery:component-command-sample:ranked-search",
            "gallery:component-command-sample:ranked-search:state",
        ),
    ];

    for (name, state, section, sample_selector, state_readout_selector) in expected {
        let story = component_story_contract(name);
        let metadata = story
            .component_contract()
            .expect("official component story should carry canonical metadata");
        assert_eq!(story.state(), Some(state));
        assert_eq!(story.section_id(), Some(section));
        assert_eq!(story.selectors().sample_selector(), Some(sample_selector));
        assert_eq!(
            story.selectors().state_readout_selector(),
            Some(state_readout_selector)
        );

        assert!(story.has_operation(StoryProbeOperation::ReadPublicPayload));
        assert!(!story.has_operation(StoryProbeOperation::Select));
        assert!(!story.has_operation(StoryProbeOperation::Focus));
        assert!(!story.has_operation(StoryProbeOperation::Open));

        let entry = component_contract_entry(name)
            .unwrap_or_else(|| panic!("expected component contract row `{name}`"));
        assert_eq!(entry.metadata(), metadata);

        let focused = pages::components::component_story_contracts_for_focus(
            pages::components::ComponentFocusMode::Section(section),
        );
        assert_eq!(
            focused
                .iter()
                .map(|focused_story| focused_story.owner_name())
                .collect::<Vec<_>>(),
            vec![name]
        );
    }
}
