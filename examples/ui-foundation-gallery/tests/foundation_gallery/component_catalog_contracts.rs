use super::*;

#[test]
fn components_page_samples_expose_component_metadata() {
    let tokens = ThemeTokens::default();
    let catalog = pages::components::COMPONENT_CATALOG;
    let gates = pages::components::COMPONENT_CONFORMANCE_GATES;
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
    let expected_official_names = COMPONENT_API_INVENTORY
        .iter()
        .filter(|entry| {
            component_contract_gallery_status(entry.component)
                == SurfaceGalleryStatus::OfficialComponent
        })
        .map(|entry| entry.component)
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

    assert_eq!(gates.len(), 12);
    assert_eq!(gates[0].id, "public-api-exports");
    assert!(
        gates[0]
            .evidence
            .contains(&"crates/ui_components/src/component_contract/rows.rs")
    );
    assert!(
        gates[0]
            .evidence
            .contains(&"crates/ui_components/src/component_contract/projections.rs")
    );
    assert!(
        gates[0]
            .evidence
            .contains(&"crates/ui_components/src/component_contract/api_inventory.rs")
    );
    assert!(
        gates[0]
            .evidence
            .contains(&"crates/ui_components/src/lib.rs")
    );
    assert!(
        gates.iter().any(|gate| {
            gate.id == "choice-surfaces"
                && gate.evidence.contains(&"choice.rs")
                && gate.evidence.contains(&"roving_focus.rs")
        }),
        "expected Components conformance gates to expose the shared choice/navigation seam"
    );
    assert_eq!(gates[1].id, "gallery-metadata");
    assert_eq!(gates[2].id, "scroll-redraw");
    assert!(
        gates[2]
            .evidence
            .contains(&"scroll_area_default_handle_survives_reconstructed_component_values")
    );
    assert_eq!(gates[3].id, "splitter-runtime");
    assert_eq!(gates[4].id, "tabs-overflow");
    assert_eq!(gates[5].id, "table-virtualization");
    assert!(gates[5].evidence.contains(&"TableFacetedFilter"));
    assert!(gates[5].evidence.contains(&"TableGlobalFilter"));
    assert!(
        gates[5]
            .evidence
            .contains(&"components_gallery_smoke_global_filter_updates_table_rows")
    );
    assert!(gates[5].evidence.contains(&"TablePredicateFilter"));
    assert!(
        gates[5]
            .evidence
            .contains(&"components_gallery_smoke_predicate_filter_updates_table_rows")
    );
    assert!(
        gates[5]
            .evidence
            .contains(&"components_gallery_smoke_faceted_filter_updates_table_rows")
    );
    assert_eq!(gates[6].id, "tree-renderer");
    assert_eq!(gates[7].id, "virtualized-list-renderer");
    assert_eq!(gates[8].id, "state-contract-readouts");
    assert_eq!(gates[9].id, "choice-surfaces");
    assert_eq!(gates[10].id, "a11y-labels");
    assert!(gates[10].evidence.contains(&"ComponentA11yContract"));
    assert!(gates[10].evidence.contains(&"COMPONENT_A11Y_EVIDENCE"));
    assert!(gates[10].evidence.contains(&"COMPONENT_A11Y_CLAIMS"));
    assert!(
        gates[10]
            .evidence
            .contains(&"crates/ui_components/tests/a11y.rs")
    );
    assert!(
        gates[10]
            .evidence
            .contains(&"representative_component_a11y_contracts_are_valid")
    );
    assert_eq!(gates[11].id, "theme-schema");
    assert!(
        gates[11]
            .evidence
            .contains(&"crates/ui_components/src/theme/schema.rs")
    );
    assert!(
        gates[11]
            .evidence
            .contains(&"crates/ui_components/tests/theme.rs")
    );
    assert!(
        gates[11]
            .evidence
            .contains(&"cargo run -p xtask -- scan-theme-drift")
    );

    let a11y_claims = pages::components::COMPONENT_A11Y_CLAIMS;
    assert_eq!(a11y_claims.len(), 11);
    assert!(a11y_claims.iter().all(|claim| {
        claim.selector_prefix.starts_with("gallery:component-")
            && claim.evidence().label_source.provides_name()
    }));
    let claim_names = a11y_claims
        .iter()
        .map(|claim| claim.component)
        .collect::<std::collections::BTreeSet<_>>();
    for expected in [
        "Button",
        "IconButton",
        "Checkbox",
        "Slider",
        "NumberInput",
        "Progress",
        "Listbox option",
        "Tree item",
        "Table",
        "VirtualizedList row",
        "Splitter handle",
    ] {
        assert!(
            claim_names.contains(expected),
            "expected `{expected}` in component a11y claims"
        );
    }
    assert!(a11y_claims.iter().any(|claim| {
        let evidence = claim.evidence();
        claim.component == "IconButton"
            && evidence.role == Role::Button
            && evidence.label_source == A11yLabelSource::ExplicitLabel
            && evidence.actions.contains(&AccessibleAction::Click)
    }));
    assert!(a11y_claims.iter().any(|claim| {
        let evidence = claim.evidence();
        claim.component == "Slider"
            && evidence.role == Role::Slider
            && evidence.value_kind == Some(A11yValueKind::Percent)
            && evidence.orientation == Some(Orientation::Horizontal)
            && evidence.actions.contains(&AccessibleAction::SetValue)
    }));
    assert!(a11y_claims.iter().any(|claim| {
        let evidence = claim.evidence();
        claim.component == "Table"
            && evidence.role == Role::Table
            && evidence.value_kind == Some(A11yValueKind::Count)
    }));
    assert!(a11y_claims.iter().any(|claim| {
        let evidence = claim.evidence();
        claim.component == "Splitter handle"
            && evidence.role == Role::Splitter
            && evidence.orientation == Some(Orientation::Vertical)
            && evidence.actions.contains(&AccessibleAction::Increment)
            && evidence.actions.contains(&AccessibleAction::Decrement)
    }));

    assert_eq!(buttons.len(), 6);
    assert_eq!(buttons[0].id, "default");
    assert_eq!(buttons[0].state.role(), Role::Button);
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
    assert_eq!(status_cues[0].state.role(), Role::Label);
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

    assert_eq!(commands.len(), 4);
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

    assert_eq!(labels.len(), 4);
    assert_eq!(labels[0].state.role(), Role::Label);
    assert_eq!(labels[0].state.control_id(), Some("email-input"));
    assert!(labels[1].state.required());
    assert!(labels[2].state.disabled());
    assert!(!labels[3].state.associated());

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
    assert_eq!(textareas[0].state.role(), Role::TextInput);
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
    assert_eq!(field_textareas[0].textarea_state.role(), Role::TextInput);

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
    assert_eq!(release_plan.role(), Role::Table);
    assert_eq!(release_plan.column_header_role(), Role::ColumnHeader);
    assert_eq!(release_plan.cell_role(), Role::Cell);
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
            .map(|row| row.id().as_str())
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
            .map(|row| row.id().as_str())
            .collect::<Vec<_>>(),
        ["row-pinning-row-003"]
    );
    assert_eq!(
        row_pinning_plan
            .rows_for_region(TableRowRegion::Center)
            .map(|row| row.id().as_str())
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
            .map(|row| row.id().as_str())
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
            .map(|row| row.id().as_str())
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
            .row(&TableRowId::new("dependency-ui"))
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
            .map(|row| row.id().as_str())
            .collect::<Vec<_>>(),
        ["server-workspace", "server-cache", "server-failed"]
    );

    assert_eq!(virtualized_lists.len(), 1);
    let release_navigation = &virtualized_lists[0];
    assert_eq!(release_navigation.id, "release-navigation");
    assert_eq!(release_navigation.items.len(), 10_000);
    assert_eq!(release_navigation.state.item_count(), 10_000);
    assert_eq!(release_navigation.state.active_index(), Some(0));
    assert_eq!(release_navigation.state.selected_index(), Some(0));
    let release_navigation_snapshot = release_navigation.behavior_snapshot();
    let release_navigation_summary = release_navigation.state_summary();
    assert_eq!(release_navigation_snapshot.role(), Role::ListBox);
    assert_eq!(release_navigation_snapshot.row_role(), Role::ListBoxOption);
    assert_eq!(release_navigation_summary.item_count, 10_000);
    assert_eq!(release_navigation_summary.visible_start, 0);
    assert_eq!(release_navigation_summary.active_index, Some(0));
    assert_eq!(release_navigation_summary.selected_index, Some(0));
    assert!(
        release_navigation_snapshot.rendered_row_count()
            <= release_navigation_snapshot.visible_row_count() + release_navigation.overscan
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
    assert_eq!(virtualized.state.selected_index(), Some(40));
    assert_eq!(virtualized.state.viewport_item_count(), 12);
    assert_eq!(virtualized.state.navigation_target("pageup"), Some(30));
    assert_eq!(virtualized.state.navigation_target("pagedown"), Some(54));
    assert_eq!(
        virtualized
            .state
            .activation_for_key("space")
            .map(|activation| activation.index()),
        Some(42)
    );
}

#[test]
fn components_catalog_metadata_is_separate_from_rendering() {
    let components_source = include_str!("../../src/pages/components.rs");
    let catalog_source = include_str!("../../src/pages/components/catalog.rs");
    let conformance_source = include_str!("../../src/pages/components/conformance.rs");
    let component_evidence_source =
        include_str!("../../../../crates/ui_components/src/component_contract/evidence.rs");
    let render_source = include_str!("../../src/pages/components/render.rs");
    let render_choice_source = include_str!("../../src/pages/components/render/choice.rs");
    let render_families_source = include_str!("../../src/pages/components/render/families.rs");
    let render_focus_source = include_str!("../../src/pages/components/render/focus.rs");
    let render_forms_source = include_str!("../../src/pages/components/render/forms.rs");
    let render_layout_source = include_str!("../../src/pages/components/render/layout.rs");
    let render_metadata_source = include_str!("../../src/pages/components/render/metadata.rs");
    let render_readouts_source = include_str!("../../src/pages/components/render/readouts.rs");
    let render_sections_source = include_str!("../../src/pages/components/render/sections.rs");
    let render_support_source = include_str!("../../src/pages/components/render/support.rs");
    let runtime_source = include_str!("../../src/pages/components/runtime.rs");
    let runtime_table_source = include_str!("../../src/pages/components/runtime/table.rs");
    let runtime_tree_source = include_str!("../../src/pages/components/runtime/tree.rs");
    let runtime_virtualized_list_source =
        include_str!("../../src/pages/components/runtime/virtualized_list.rs");
    let samples_source = include_str!("../../src/pages/components/samples.rs");
    let foundation_samples_source =
        include_str!("../../src/pages/components/samples/foundation.rs");
    let feedback_samples_source = include_str!("../../src/pages/components/samples/feedback.rs");
    let tree_samples_source = include_str!("../../src/pages/components/samples/tree.rs");
    let virtualized_list_samples_source =
        include_str!("../../src/pages/components/samples/virtualized_list.rs");
    let choice_samples_source = include_str!("../../src/pages/components/samples/choice.rs");
    let text_samples_source = include_str!("../../src/pages/components/samples/text.rs");
    let navigation_samples_source =
        include_str!("../../src/pages/components/samples/navigation.rs");
    let table_samples_source = include_str!("../../src/pages/components/samples/table.rs");
    let layout_samples_source = include_str!("../../src/pages/components/samples/layout.rs");

    assert!(components_source.contains("pub mod catalog;"));
    assert!(components_source.contains("pub use catalog::{"));
    assert!(components_source.contains("pub mod conformance;"));
    assert!(components_source.contains("mod runtime;"));
    assert!(components_source.contains("mod samples;"));
    assert!(components_source.contains("pub use runtime::{"));
    assert!(components_source.contains("pub use samples::{"));
    assert!(!components_source.contains("pub mod runtime;"));
    assert!(!components_source.contains("pub mod samples;"));
    assert!(!components_source.contains("pub use runtime::*;"));
    assert!(!components_source.contains("pub use samples::*;"));
    assert!(components_source.contains("TableSampleRuntimeLog"));
    assert!(components_source.contains("table_samples"));
    assert!(catalog_source.contains("pub const COMPONENT_CATALOG"));
    assert!(catalog_source.contains("ComponentCatalogEntry::contract_sample("));
    assert!(catalog_source.contains("ComponentCatalogEntry::state_contract("));
    assert!(conformance_source.contains("COMPONENT_CONFORMANCE_GATES, ComponentConformanceGate"));
    assert!(component_evidence_source.contains("pub const COMPONENT_CONFORMANCE_GATES"));
    for module_path in [
        "#[path = \"runtime/table.rs\"]",
        "#[path = \"runtime/tree.rs\"]",
        "#[path = \"runtime/virtualized_list.rs\"]",
    ] {
        assert!(
            runtime_source.contains(module_path),
            "runtime facade should declare owner module `{module_path}`"
        );
    }
    assert!(runtime_table_source.contains("pub struct TableSampleRuntimeLog"));
    assert!(runtime_tree_source.contains("pub struct TreeSampleRuntimeLog"));
    assert!(runtime_virtualized_list_source.contains("pub struct VirtualizedListSampleRuntimeLog"));
    assert!(!runtime_source.contains("pub struct TableSampleRuntimeLog"));
    assert!(!runtime_source.contains("pub struct TreeSampleRuntimeLog"));
    assert!(!runtime_source.contains("pub struct VirtualizedListSampleRuntimeLog"));
    for module_path in [
        "#[path = \"samples/foundation.rs\"]",
        "#[path = \"samples/feedback.rs\"]",
        "#[path = \"samples/tree.rs\"]",
        "#[path = \"samples/virtualized_list.rs\"]",
        "#[path = \"samples/choice.rs\"]",
        "#[path = \"samples/text.rs\"]",
        "#[path = \"samples/navigation.rs\"]",
        "#[path = \"samples/table.rs\"]",
        "#[path = \"samples/layout.rs\"]",
    ] {
        assert!(
            samples_source.contains(module_path),
            "samples facade should declare family module `{module_path}`"
        );
    }
    assert!(samples_source.contains("pub use foundation::{"));
    assert!(samples_source.contains("pub use table::{"));
    assert!(foundation_samples_source.contains("pub struct ButtonSample"));
    assert!(feedback_samples_source.contains("pub struct StatusCueSample"));
    assert!(tree_samples_source.contains("pub struct TreeSample"));
    assert!(virtualized_list_samples_source.contains("pub struct VirtualizedListSample"));
    assert!(choice_samples_source.contains("pub struct CheckboxSample"));
    assert!(text_samples_source.contains("pub struct TextInputSample"));
    assert!(navigation_samples_source.contains("pub struct TabsSample"));
    assert!(table_samples_source.contains("pub struct TableSample"));
    assert!(layout_samples_source.contains("pub struct ScrollAreaSample"));
    assert!(!samples_source.contains("pub struct ButtonSample"));
    assert!(!samples_source.contains("static TABLE_SAMPLES"));
    for module_path in [
        "#[path = \"render/choice.rs\"]",
        "#[path = \"render/families.rs\"]",
        "#[path = \"render/focus.rs\"]",
        "#[path = \"render/forms.rs\"]",
        "#[path = \"render/layout.rs\"]",
        "#[path = \"render/metadata.rs\"]",
        "#[path = \"render/readouts.rs\"]",
        "#[path = \"render/sections.rs\"]",
        "#[path = \"render/support.rs\"]",
    ] {
        assert!(
            render_source.contains(module_path),
            "render facade should declare owner module `{module_path}`"
        );
    }
    assert!(render_choice_source.contains("fn render_component_choice_sections"));
    assert!(render_choice_source.contains("fn render_switch_section"));
    assert!(render_choice_source.contains("fn render_checkbox_section"));
    assert!(render_choice_source.contains("fn render_radio_group_section"));
    assert!(render_choice_source.contains("fn render_toggle_section"));
    assert!(render_families_source.contains("fn component_tree_samples_section"));
    assert!(render_focus_source.contains("fn render_component_focus_mode"));
    assert!(render_forms_source.contains("fn render_component_text_input_section"));
    assert!(render_forms_source.contains("fn render_component_textarea_section"));
    assert!(render_forms_source.contains("fn render_component_field_section"));
    assert!(render_layout_source.contains("fn render_component_scroll_area_section"));
    assert!(render_layout_source.contains("fn render_scroll_area_sample"));
    assert!(render_metadata_source.contains("fn render_component_catalog_section"));
    assert!(render_metadata_source.contains("fn render_component_gates_section"));
    assert!(render_readouts_source.contains("fn component_table_state_row"));
    assert!(render_sections_source.contains("fn render_components_section"));
    assert!(render_support_source.contains("fn component_gallery_card_shell"));
    assert!(!render_source.contains("fn component_tree_samples_section"));
    assert!(!render_sections_source.contains("fn render_switch_section"));
    assert!(!render_sections_source.contains("pages::components::switch_samples"));
    assert!(!render_sections_source.contains("pages::components::scroll_area_samples"));
    assert!(!render_sections_source.contains("pages::components::text_input_samples"));
    assert!(!render_sections_source.contains("pages::components::textarea_samples"));
    assert!(!render_sections_source.contains("pages::components::field_samples"));
    assert!(!render_source.contains("fn component_table_state_row"));
    assert!(!render_source.contains("fn render_components_section"));
    assert!(!render_source.contains("fn component_gallery_card_shell"));
    assert!(!components_source.contains("pub struct ButtonSample"));
    assert!(!components_source.contains("pub struct TableSampleRuntimeLog"));
    assert!(!render_source.contains("pub const COMPONENT_CATALOG"));
    assert!(!render_sections_source.contains("pub const COMPONENT_CATALOG"));
    assert!(!render_metadata_source.contains("pub const COMPONENT_CATALOG"));
    assert!(!render_sections_source.contains("pages::components::COMPONENT_CATALOG"));
    assert!(
        render_metadata_source.contains("pages::components::COMPONENT_CATALOG")
            && render_metadata_source.contains("pages::components::COMPONENT_CONFORMANCE_GATES"),
        "rendering should consume catalog metadata instead of owning it"
    );
}

#[test]
fn components_catalog_consumes_component_contract_rows() {
    use std::collections::BTreeSet;

    for entry in pages::components::COMPONENT_CATALOG {
        let expected_status = pages::components::ComponentCatalogStatus::from_contract(
            component_contract_gallery_status(entry.name),
        );
        assert_eq!(
            entry.status, expected_status,
            "catalog entry `{}` should derive status from the component contract rows",
            entry.name
        );

        if let Some(expected_family) = component_contract_family(entry.name) {
            assert_eq!(
                entry.family, expected_family,
                "catalog entry `{}` should derive family from the component contract rows",
                entry.name
            );
        }
    }

    let catalog_names = pages::components::COMPONENT_CATALOG
        .iter()
        .map(|entry| entry.name)
        .collect::<BTreeSet<_>>();
    let contract_official_names = COMPONENT_API_INVENTORY
        .iter()
        .filter(|entry| {
            component_contract_gallery_status(entry.component)
                == SurfaceGalleryStatus::OfficialComponent
        })
        .map(|entry| entry.component)
        .collect::<BTreeSet<_>>();
    let catalog_official_names = pages::components::COMPONENT_CATALOG
        .iter()
        .filter(|entry| entry.status == pages::components::ComponentCatalogStatus::Official)
        .map(|entry| entry.name)
        .collect::<BTreeSet<_>>();
    assert_eq!(
        catalog_official_names, contract_official_names,
        "Components catalog official rows should be contract-owned"
    );

    let missing_adjacent_surfaces = PUBLIC_SURFACE_OWNER_MAP
        .iter()
        .filter(|entry| {
            matches!(
                component_contract_gallery_status(entry.name),
                SurfaceGalleryStatus::AdapterOnly
                    | SurfaceGalleryStatus::InternalAnatomy
                    | SurfaceGalleryStatus::StateContract
            )
        })
        .filter(|entry| !catalog_names.contains(entry.name))
        .map(|entry| entry.name)
        .collect::<Vec<_>>();
    assert!(
        missing_adjacent_surfaces.is_empty(),
        "Components catalog should include contract gallery-adjacent surfaces: {missing_adjacent_surfaces:?}"
    );
}

#[test]
fn verification_docs_list_current_ui_architecture_focused_gates() {
    let verification = include_str!("../../../../docs/verification.md");

    for required in [
        "primitive_deletion_target_inventory_blocks_removed_shallow_reexports",
        "primitive_modules_do_not_reexport_ui_core_as_pass_through_aliases",
        "surface_manifest_classifies_public_surface_once",
        "surface_manifest_aligns_adjacent_gallery_statuses",
        "surface_manifest_tracks_exports_gallery_and_docs_contracts",
        "overlay_open_change_helpers_match_core_policies",
        "dialog_runtime_respects_escape_policy_and_restores_trigger_focus",
        "choice_surfaces_share_stable_value_resolution_and_query_normalization",
        "table_behavior_snapshot_exposes_faceting_metadata",
        "table_behavior_snapshot_exposes_editable_leaf_cell_kinds_for_leaf_cells_only",
        "table_component_source_mapping_tracks_split_render_owners",
        "row_window",
        "virtualized_list_behavior_snapshot_uses_item_descriptors_and_virtualizer_contracts",
        "theme_registry",
        "theme_resolver",
        "theme_snapshots",
        "components_catalog_metadata_is_separate_from_rendering",
        "official_component_catalog_entries_have_signals_and_sample_selectors",
        "state_contract_catalog_entries_have_signals_and_readout_selectors",
        "gallery_story_contracts_cover_components_state_readouts_and_overlays",
    ] {
        assert!(
            verification.contains(required),
            "verification docs should list focused UI architecture gate `{required}`"
        );
    }

    assert!(
        !verification.contains("table_render_plan_exposes_"),
        "verification docs should use Table diagnostics gates, not removed render-plan test names"
    );
    assert!(
        verification.contains("cargo run -p xtask -- verify"),
        "verification docs should keep the final integration gate visible"
    );
}

#[test]
fn component_gallery_shell_reads_splitter_behavior_from_resolved_state() {
    let samples_source = include_str!("../../src/pages/components/samples.rs");
    let layout_samples_source = include_str!("../../src/pages/components/samples/layout.rs");
    let render_sections_source = include_str!("../../src/pages/components/render/sections.rs");
    let splitter_struct_start = layout_samples_source
        .find("pub struct SplitterSample {")
        .expect("expected SplitterSample struct to exist");
    let splitter_struct_end = layout_samples_source[splitter_struct_start..]
        .find("/// Returns scroll area samples backed by real component state.")
        .map(|offset| splitter_struct_start + offset)
        .expect("expected SplitterSample sample builder to follow struct declarations");
    let splitter_struct = &layout_samples_source[splitter_struct_start..splitter_struct_end];
    let splitter_section = render_sections_source
        .split("component_page_section(\"splitter\")")
        .nth(1)
        .and_then(|section| section.split("render_component_scroll_area_section").next())
        .expect("expected Splitter section in components render source");

    assert!(samples_source.contains(
        "impl_component_sample_selectors!(SplitterSample, \"component-splitter-sample\");"
    ));
    assert!(!splitter_struct.contains("pub orientation: Orientation,"));
    assert!(!splitter_struct.contains("pub size: Size,"));
    assert!(splitter_section.contains(".orientation(state.orientation())"));
    assert!(splitter_section.contains(".with_size(state.size())"));
    assert!(!splitter_section.contains(".orientation(sample.orientation)"));
    assert!(!splitter_section.contains(".with_size(sample.size)"));
}

#[test]
fn component_gallery_shell_reads_choice_active_metadata_from_resolved_state() {
    let shell_components_source = include_str!("../../src/shell/components.rs");
    let select_section = shell_components_source
        .split("fn component_select_samples_section(")
        .nth(1)
        .and_then(|section| {
            section
                .split("fn component_combobox_samples_section")
                .next()
        })
        .expect("expected Select sample section in shell components source");
    let combobox_section = shell_components_source
        .split("fn component_combobox_samples_section(")
        .nth(1)
        .and_then(|section| section.split("fn component_command_samples_section").next())
        .expect("expected Combobox sample section in shell components source");
    let command_section = shell_components_source
        .split("fn component_command_samples_section(")
        .nth(1)
        .and_then(|section| section.split("fn resolved_listbox_option").next())
        .expect("expected Command sample section in shell components source");
    let listbox_readout = shell_components_source
        .split("fn component_listbox_state_row(")
        .nth(1)
        .and_then(|section| section.split("fn component_select_state_row").next())
        .expect("expected Listbox state row in shell components source");
    let select_readout = shell_components_source
        .split("fn component_select_state_row(")
        .nth(1)
        .and_then(|section| section.split("fn component_combobox_state_row").next())
        .expect("expected Select state row in shell components source");
    let combobox_readout = shell_components_source
        .split("fn component_combobox_state_row(")
        .nth(1)
        .and_then(|section| section.split("fn component_command_state_row").next())
        .expect("expected Combobox state row in shell components source");
    let command_readout = shell_components_source
        .split("fn component_command_state_row(")
        .nth(1)
        .and_then(|section| {
            section
                .split("pub(crate) fn component_radio_state_row")
                .next()
        })
        .expect("expected Command state row in shell components source");

    assert!(select_section.contains("if let Some(active) = state.active_value()"));
    assert!(select_section.contains("select = select.active(active);"));
    assert!(combobox_section.contains("if let Some(active) = state.active_value()"));
    assert!(combobox_section.contains("combobox = combobox.active(active);"));
    assert!(command_section.contains("if let Some(active) = state.active_value()"));
    assert!(command_section.contains("command = command.active(active);"));
    assert!(listbox_readout.contains("typeahead_label"));
    assert!(listbox_readout.contains("first_typeahead_target"));
    assert!(select_readout.contains("listbox selected"));
    assert!(combobox_readout.contains("visible {} of {} / typeahead"));
    assert!(command_readout.contains("selected_values {:?}"));
}

#[test]
fn official_component_catalog_entries_have_signals_and_sample_selectors() {
    use std::collections::BTreeSet;

    let official_names = pages::components::COMPONENT_CATALOG
        .iter()
        .filter(|entry| entry.status == pages::components::ComponentCatalogStatus::Official)
        .map(|entry| entry.name)
        .collect::<BTreeSet<_>>();
    let sample_names = pages::components::official_sample_selector_pairs()
        .map(|(name, _)| name)
        .collect::<BTreeSet<_>>();

    assert_eq!(sample_names, official_names);

    let selector_values = pages::components::official_sample_selector_pairs()
        .map(|(_, selector)| selector)
        .collect::<Vec<_>>();
    let unique_selectors = selector_values.iter().copied().collect::<BTreeSet<_>>();
    assert_eq!(unique_selectors.len(), selector_values.len());

    let stray_selectors = pages::components::COMPONENT_CATALOG
        .iter()
        .filter(|entry| entry.status != pages::components::ComponentCatalogStatus::Official)
        .filter(|entry| entry.sample_selector.is_some())
        .map(|entry| entry.name)
        .collect::<Vec<_>>();
    assert!(
        stray_selectors.is_empty(),
        "non-official catalog entries must not declare sample selectors: {stray_selectors:?}"
    );
    let official_state_contract_selectors = pages::components::COMPONENT_CATALOG
        .iter()
        .filter(|entry| entry.status == pages::components::ComponentCatalogStatus::Official)
        .filter(|entry| entry.state_contract_selector.is_some())
        .map(|entry| entry.name)
        .collect::<Vec<_>>();
    assert!(
        official_state_contract_selectors.is_empty(),
        "official catalog entries must not declare state-contract readout selectors: {official_state_contract_selectors:?}"
    );

    let signals = pages::components::SIGNALS;
    let mut missing = Vec::new();
    for entry in pages::components::COMPONENT_CATALOG
        .iter()
        .filter(|entry| entry.status == pages::components::ComponentCatalogStatus::Official)
    {
        let component_signal = format!("open_gpui_ui_components::{}", entry.name);
        if !signals.contains(&component_signal.as_str()) {
            missing.push(format!(
                "{} component signal `{component_signal}`",
                entry.name
            ));
        }
        let Some(state) = entry.state else {
            missing.push(format!("{} official entry has no state type", entry.name));
            continue;
        };
        let state_signal = format!("open_gpui_ui_components::{state}");
        if !signals.contains(&state_signal.as_str()) {
            missing.push(format!("{} state signal `{state_signal}`", entry.name));
        }
    }

    assert!(
        missing.is_empty(),
        "official component catalog entries must have matching signals: {missing:?}"
    );
}

#[test]
fn state_contract_catalog_entries_have_signals_and_readout_selectors() {
    use std::collections::BTreeSet;

    let state_contract_names = pages::components::COMPONENT_CATALOG
        .iter()
        .filter(|entry| entry.status == pages::components::ComponentCatalogStatus::StateContract)
        .map(|entry| entry.name)
        .collect::<BTreeSet<_>>();
    let readout_names = pages::components::state_contract_readout_pairs()
        .map(|(name, _)| name)
        .collect::<BTreeSet<_>>();

    assert_eq!(readout_names, state_contract_names);

    let official_names = pages::components::official_sample_selector_pairs()
        .map(|(name, _)| name)
        .collect::<BTreeSet<_>>();
    assert!(
        state_contract_names.is_disjoint(&official_names),
        "state contracts must not satisfy the official rendered component selector gate"
    );

    let readout_selectors = pages::components::state_contract_readout_pairs()
        .map(|(_, selector)| selector)
        .collect::<Vec<_>>();
    let unique_readout_selectors = readout_selectors.iter().copied().collect::<BTreeSet<_>>();
    assert_eq!(unique_readout_selectors.len(), readout_selectors.len());

    let stray_readout_selectors = pages::components::COMPONENT_CATALOG
        .iter()
        .filter(|entry| entry.status != pages::components::ComponentCatalogStatus::StateContract)
        .filter(|entry| entry.state_contract_selector.is_some())
        .map(|entry| entry.name)
        .collect::<Vec<_>>();
    assert!(
        stray_readout_selectors.is_empty(),
        "non-state-contract catalog entries must not declare state-contract selectors: {stray_readout_selectors:?}"
    );

    let required_signals = [
        "open_gpui_ui_components::TreeState",
        "open_gpui_ui_components::TreeItemDescriptor",
        "open_gpui_ui_components::TreeItemState",
        "open_gpui_ui_components::TreeSelection",
        "open_gpui_ui_components::TreeToggle",
        "open_gpui_ui_components::TreeFocusTarget",
        "open_gpui_ui_components::TreeKeyboardAction",
        "open_gpui_ui_components::tree_navigation_target",
        "open_gpui_ui_components::VirtualizedListState",
        "open_gpui_ui_components::VirtualizedListActivation",
        "open_gpui_ui_components::VirtualizedListMetrics",
        "open_gpui_ui_components::VirtualizedListScrollStrategy",
        "open_gpui_ui_components::virtualized_list_navigation_target",
    ];
    for signal in required_signals {
        assert!(
            pages::components::SIGNALS.contains(&signal),
            "expected state-contract signal `{signal}`"
        );
    }
}

#[test]
fn gallery_story_contracts_cover_components_state_readouts_and_overlays() {
    use std::collections::{BTreeMap, BTreeSet};

    let component_stories = pages::components::component_story_contracts();
    let overlay_stories = pages::overlay::overlay_story_contracts();

    let component_story_names = component_stories
        .iter()
        .map(|story| story.owner_name())
        .collect::<BTreeSet<_>>();
    let expected_component_story_names = pages::components::COMPONENT_CATALOG
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
    assert_eq!(component_story_names, expected_component_story_names);
    assert_eq!(
        pages::components::component_story_contracts_for_focus(
            pages::components::ComponentFocusMode::All,
        ),
        component_stories,
        "all-mode story contract helper should expose the same component contract records"
    );

    let story_sample_pairs = component_stories
        .iter()
        .filter_map(|story| {
            (story.kind() == open_gpui_ui_foundation_gallery::StoryContractKind::Component)
                .then(|| {
                    story
                        .selectors()
                        .sample_selector()
                        .map(|selector| (story.owner_name(), selector))
                })
                .flatten()
        })
        .collect::<Vec<_>>();
    let official_sample_pairs =
        pages::components::official_sample_selector_pairs().collect::<Vec<_>>();
    assert_eq!(
        official_sample_pairs, story_sample_pairs,
        "official sample selectors should be derived from story contracts"
    );

    let story_readout_pairs = component_stories
        .iter()
        .filter_map(|story| {
            (story.kind() == open_gpui_ui_foundation_gallery::StoryContractKind::StateContract)
                .then(|| {
                    story
                        .selectors()
                        .state_readout_selector()
                        .map(|selector| (story.owner_name(), selector))
                })
                .flatten()
        })
        .collect::<Vec<_>>();
    let state_contract_pairs =
        pages::components::state_contract_readout_pairs().collect::<Vec<_>>();
    assert_eq!(
        state_contract_pairs, story_readout_pairs,
        "state-contract readout selectors should be derived from story contracts"
    );

    for story in &component_stories {
        if let Some(section_id) = story.section_id() {
            let focused = pages::components::component_story_contracts_for_focus(
                pages::components::ComponentFocusMode::Section(section_id),
            );
            assert!(
                focused
                    .iter()
                    .any(|focused_story| focused_story.owner_name() == story.owner_name()),
                "focused story contracts for `{section_id}` should include `{}`",
                story.owner_name()
            );
        }
    }

    let overlay_story_names = overlay_stories
        .iter()
        .map(|story| story.owner_name())
        .collect::<BTreeSet<_>>();
    let expected_overlay_story_names = pages::overlay::OVERLAY_CATALOG
        .iter()
        .map(|entry| entry.name)
        .collect::<BTreeSet<_>>();
    assert_eq!(overlay_story_names, expected_overlay_story_names);

    for story in component_stories.iter().chain(overlay_stories.iter()) {
        assert!(
            story.selectors().catalog_selector().is_some(),
            "story `{}` should expose a catalog selector",
            story.owner_name()
        );
        assert!(
            story.selectors().primary_selector().is_some(),
            "story `{}` should expose a primary user-observable selector",
            story.owner_name()
        );
        assert!(
            !story.probes().is_empty(),
            "story `{}` should declare runtime probe operations",
            story.owner_name()
        );
        assert!(
            story.has_operation(StoryProbeOperation::ReadPublicPayload),
            "story `{}` should expose a public payload/readout probe",
            story.owner_name()
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
            "gallery stories should cover runtime probe operation `{}`",
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

    let tree_state_story = component_stories
        .iter()
        .find(|story| story.owner_name() == "TreeState")
        .expect("TreeState story contract should exist");
    assert_eq!(
        tree_state_story.kind(),
        open_gpui_ui_foundation_gallery::StoryContractKind::StateContract
    );
    assert_eq!(
        tree_state_story.selectors().state_readout_selector(),
        Some("gallery:component-tree-state-contract:document-outline")
    );

    let dialog_story = overlay_stories
        .iter()
        .find(|story| story.owner_name() == "Dialog")
        .expect("Dialog story contract should exist");
    assert_eq!(
        dialog_story.selectors().trigger_selector(),
        Some("dialog:overlay-dialog-demo:controlled-modal:trigger")
    );
    assert_eq!(
        dialog_story.selectors().surface_selector(),
        Some("dialog:overlay-dialog-demo:controlled-modal:surface")
    );
}

#[test]
fn gallery_catalog_manifest_tracks_components_and_overlay_catalogs() {
    use std::collections::BTreeSet;

    let component_names = pages::components::COMPONENT_CATALOG
        .iter()
        .map(|entry| entry.name)
        .collect::<BTreeSet<_>>();
    let component_official = pages::components::COMPONENT_CATALOG
        .iter()
        .filter(|entry| entry.status == pages::components::ComponentCatalogStatus::Official)
        .count();
    let component_adjacent = pages::components::COMPONENT_CATALOG
        .iter()
        .filter(|entry| {
            matches!(
                entry.status,
                pages::components::ComponentCatalogStatus::AdapterOnly
                    | pages::components::ComponentCatalogStatus::InternalAnatomy
                    | pages::components::ComponentCatalogStatus::StateContract
            )
        })
        .count();

    assert!(
        component_official >= 40,
        "Components catalog should keep broad official component coverage"
    );
    assert!(
        component_adjacent >= 6,
        "Components catalog should keep adjacent adapter/anatomy/state-contract rows"
    );
    for required in [
        "Button",
        "Table",
        "VirtualizedList",
        "TextInputController",
        "ToolbarItem",
        "ListboxOption",
        "TreeState",
        "VirtualizedListState",
    ] {
        assert!(
            component_names.contains(required),
            "Components catalog manifest should include `{required}`"
        );
    }

    let overlay_names = pages::overlay::OVERLAY_CATALOG
        .iter()
        .map(|entry| entry.name)
        .collect::<BTreeSet<_>>();
    assert_eq!(
        overlay_names,
        BTreeSet::from([
            "AlertDialog",
            "ContextMenu",
            "Dialog",
            "HoverCard",
            "Menu",
            "Popover",
            "Sheet",
            "Tooltip",
        ])
    );
    assert!(pages::overlay::OVERLAY_CATALOG.iter().all(|entry| {
        let component_signal = format!("open_gpui_ui_components::{}", entry.name);
        let state_signal = format!("open_gpui_ui_components::{}", entry.state);
        pages::overlay::SIGNALS.contains(&component_signal.as_str())
            && pages::overlay::SIGNALS.contains(&state_signal.as_str())
            && !entry.sample_selector.trim().is_empty()
            && !entry.catalog_selector().trim().is_empty()
    }));
}

#[test]
fn gallery_catalog_entries_satisfy_component_contract_evidence() {
    use std::collections::{BTreeMap, BTreeSet};

    let component_catalog = pages::components::COMPONENT_CATALOG
        .iter()
        .map(|entry| (entry.name, entry))
        .collect::<BTreeMap<_, _>>();
    let overlay_names = pages::overlay::OVERLAY_CATALOG
        .iter()
        .map(|entry| entry.name)
        .collect::<BTreeSet<_>>();

    for entry in COMPONENT_CONTRACT_ROWS
        .iter()
        .filter(|entry| entry.gallery_status != SurfaceGalleryStatus::NotInGallery)
    {
        match entry.gallery_status {
            SurfaceGalleryStatus::OfficialComponent
            | SurfaceGalleryStatus::AdapterOnly
            | SurfaceGalleryStatus::InternalAnatomy
            | SurfaceGalleryStatus::StateContract => {
                let catalog_entry = component_catalog
                    .get(entry.name)
                    .unwrap_or_else(|| {
                        panic!(
                            "component contract row `{}` claims Components gallery evidence but no catalog row exists",
                            entry.name
                        )
                    });
                let expected_status = match entry.gallery_status {
                    SurfaceGalleryStatus::OfficialComponent => {
                        pages::components::ComponentCatalogStatus::Official
                    }
                    SurfaceGalleryStatus::AdapterOnly => {
                        pages::components::ComponentCatalogStatus::AdapterOnly
                    }
                    SurfaceGalleryStatus::InternalAnatomy => {
                        pages::components::ComponentCatalogStatus::InternalAnatomy
                    }
                    SurfaceGalleryStatus::StateContract => {
                        pages::components::ComponentCatalogStatus::StateContract
                    }
                    SurfaceGalleryStatus::OfficialOverlay | SurfaceGalleryStatus::NotInGallery => {
                        unreachable!()
                    }
                };
                assert_eq!(
                    catalog_entry.status, expected_status,
                    "component contract row `{}` should agree with Components catalog status",
                    entry.name
                );
                if entry.gallery_status == SurfaceGalleryStatus::OfficialComponent {
                    assert!(
                        catalog_entry.sample_selector.is_some(),
                        "official component contract row `{}` needs a rendered sample selector",
                        entry.name
                    );
                }
                if entry.gallery_status == SurfaceGalleryStatus::StateContract {
                    assert!(
                        catalog_entry.state_contract_selector.is_some(),
                        "state-contract component contract row `{}` needs a readout selector",
                        entry.name
                    );
                }
            }
            SurfaceGalleryStatus::OfficialOverlay => {
                assert!(
                    overlay_names.contains(entry.name),
                    "component contract row `{}` claims overlay gallery evidence but no overlay catalog row exists",
                    entry.name
                );
            }
            SurfaceGalleryStatus::NotInGallery => unreachable!(),
        }
    }
}

#[test]
fn gallery_story_contracts_reference_component_contract_rows() {
    use std::collections::BTreeMap;

    let contract_entries = COMPONENT_CONTRACT_ROWS
        .iter()
        .map(|entry| (entry.name, entry))
        .collect::<BTreeMap<_, _>>();

    for story in pages::components::component_story_contracts() {
        let entry = contract_entries.get(story.owner_name()).unwrap_or_else(|| {
            panic!(
                "component story `{}` should reference a component contract row",
                story.owner_name()
            )
        });
        assert!(
            matches!(
                entry.gallery_status,
                SurfaceGalleryStatus::OfficialComponent | SurfaceGalleryStatus::StateContract
            ),
            "component story `{}` should be official component or state-contract evidence, got {:?}",
            story.owner_name(),
            entry.gallery_status
        );
    }

    for story in pages::overlay::overlay_story_contracts() {
        let entry = contract_entries.get(story.owner_name()).unwrap_or_else(|| {
            panic!(
                "overlay story `{}` should reference a component contract row",
                story.owner_name()
            )
        });
        assert_eq!(
            entry.gallery_status,
            SurfaceGalleryStatus::OfficialOverlay,
            "overlay story `{}` should be official overlay evidence",
            story.owner_name()
        );
    }
}
