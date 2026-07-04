use super::*;

use open_gpui_command as command_core;
use open_gpui_ui_core as ui_core;

open_gpui::actions!(
    public_surface_command_actions,
    [RootOpen, RootSave, PreludeOpen, PreludeSave]
);

#[test]
fn crate_root_and_prelude_exports_remain_explicit() {
    use open_gpui_ui_components::{self as root, prelude};

    let root_overlay: root::OverlayResolvedState = root::OverlayResolvedState::resolve(
        OverlayLayerPolicy::new(OverlayLayerKind::Tooltip, OverlayPresence::open()),
    );
    let prelude_overlay: prelude::OverlayResolvedState = prelude::OverlayResolvedState::resolve(
        OverlayLayerPolicy::new(OverlayLayerKind::Tooltip, OverlayPresence::open()),
    );
    let root_a11y_contract = root::ComponentA11yContract::new("Button", Role::Button)
        .with_label_source(root::A11yLabelSource::VisibleText)
        .with_description_source(root::A11yDescriptionSource::None)
        .selected_state(false)
        .disabled_state(false)
        .with_actions(&[AccessibleAction::Click]);
    let prelude_a11y_contract = prelude::ComponentA11yContract::new("Slider", Role::Slider)
        .with_label_source(prelude::A11yLabelSource::VisibleText)
        .with_value_metadata(prelude::A11yValueMetadata::present(
            prelude::A11yValueKind::Percent,
        ))
        .with_orientation(Orientation::Horizontal)
        .with_actions(&[
            AccessibleAction::Increment,
            AccessibleAction::Decrement,
            AccessibleAction::SetValue,
        ]);
    let _root_a11y_error: root::A11yContractError =
        root::ComponentA11yContract::new("IconButton", Role::Button)
            .validate()
            .unwrap_err()
            .error();
    let _prelude_a11y_violation: prelude::A11yContractViolation =
        prelude::ComponentA11yContract::new("NumberInput", Role::SpinButton)
            .with_label_source(prelude::A11yLabelSource::ExplicitLabel)
            .validate()
            .unwrap_err();
    assert!(root::A11yLabelSource::ExplicitLabel.provides_name());
    assert!(
        prelude::A11yValueMetadata::absent(prelude::A11yValueKind::Number).kind()
            == prelude::A11yValueKind::Number
    );
    root_a11y_contract.validate().unwrap();
    prelude_a11y_contract.validate().unwrap();
    let _root_a11y_evidence: root::ComponentA11yEvidence = root::COMPONENT_A11Y_EVIDENCE[0];
    let _prelude_a11y_evidence: prelude::ComponentA11yEvidence =
        prelude::COMPONENT_A11Y_EVIDENCE[0];
    let _root_conformance_gate: root::ComponentConformanceGate =
        root::COMPONENT_CONFORMANCE_GATES[0];
    let _prelude_conformance_gate: prelude::ComponentConformanceGate =
        prelude::COMPONENT_CONFORMANCE_GATES[0];
    assert_eq!(
        root::component_a11y_evidence("Button").map(|evidence| evidence.role),
        Some(Role::Button)
    );
    let root_button = root::Button::new("save", "Save");
    let root_accordion = root::Accordion::new("accordion")
        .mode(root::AccordionMode::Multiple)
        .item(root::AccordionItem::new("one", "One", "One content"));
    let root_alert_dialog = root::AlertDialog::new(
        "delete",
        "Delete",
        "Delete item?",
        "This removes it.",
        "Delete",
    );
    let root_sheet = root::Sheet::new("sheet", "Open sheet", "Sheet", "Sheet content");
    let root_hover_card = root::HoverCard::new("hover-card", "Profile", "Profile details");
    let root_sidebar = root::Sidebar::new("sidebar", "Primary navigation");
    let root_toolbar = root::Toolbar::new("toolbar", "Editor");
    let root_listbox = root::Listbox::new("listbox", "Choices");
    let root_select = root::Select::new("select", "Choice");
    let root_select_option = ui_core::TableSelectOption::new("ready", "Ready");
    let root_combobox = root::Combobox::new("combobox", "Search");
    let root_command = root::Command::new("command", "Commands");
    let root_command_status_item: root::CommandStatusItem =
        root::CommandStatusItem::warning("Shortcut diagnostics");
    let _root_command_status_intent: root::CommandStatusIntent = root_command_status_item.intent();
    let root_command_navigation: root::CommandNavigationBehavior =
        root::CommandNavigationBehavior::new()
            .with_loop_navigation(false)
            .with_group_navigation(true);
    assert!(!root_command_navigation.loop_navigation());
    assert!(root_command_navigation.group_navigation());
    let root_core_command =
        command_core::CommandDescriptor::new("root.open", "Open").shortcut("Ctrl+O");
    let mut root_command_registry = command_core::CommandRegistry::new("root-registry-v1");
    root_command_registry
        .register_contribution(
            command_core::CommandContribution::new(root_core_command.clone())
                .source("root-workspace"),
        )
        .unwrap();
    let root_registry_snapshot: command_core::CommandRegistrySnapshot =
        root_command_registry.snapshot();
    let root_shortcut_diagnostics: Vec<command_core::CommandShortcutDiagnostic> =
        command_core::GpuiCommandActionMap::new().shortcut_diagnostics_for_keymap(
            &root_registry_snapshot,
            &open_gpui::Keymap::default(),
        );
    let _root_shortcut_diagnostic_kind: command_core::CommandShortcutDiagnosticKind =
        root_shortcut_diagnostics[0].kind();
    let _root_registry_duplicate: command_core::CommandRegistryError = root_command_registry
        .register(root_core_command.clone())
        .unwrap_err();
    let mut root_command_center = command_core::CommandCenter::new("root-center-v1");
    let root_context_stack: command_core::CommandContextStack =
        command_core::CommandContextStack::new()
            .scope("root")
            .key_context(open_gpui::KeyContext::parse("Root").unwrap());
    root_command_center.set_context_stack(root_context_stack);
    assert_eq!(root_command_center.active_scopes()[0].as_str(), "root");
    assert_eq!(
        root_command_center.key_contexts()[0]
            .primary()
            .map(|entry| entry.key.as_ref()),
        Some("Root")
    );
    let root_command_source: command_core::CommandSourceHandle = root_command_center
        .register_source(
            "root",
            "root-source",
            [command_core::CommandContribution::new(
                root_core_command.clone(),
            )],
        )
        .unwrap();
    let _root_command_source_registration: command_core::CommandSourceRegistration =
        root_command_source.clone();
    root_command_center
        .register_action("root.open", RootOpen)
        .register_action("root.save", RootSave);
    let root_key_binding =
        command_core::CommandKeyBinding::new("root.open", "ctrl-o").context("Root");
    let mut root_key_binding_registry = command_core::CommandKeyBindingRegistry::new();
    let root_key_binding_handle: command_core::CommandKeyBindingHandle = root_key_binding_registry
        .register(
            "root-shortcuts",
            [
                root_key_binding,
                command_core::CommandKeyBinding::new("root.save", "ctrl-o").context("Root"),
                command_core::CommandKeyBinding::new("root.missing", "ctrl-m"),
            ],
        );
    let root_key_binding_entry: &command_core::CommandKeyBindingEntry =
        &root_key_binding_registry.entries()[0];
    assert_eq!(root_key_binding_entry.binding().command_id(), "root.open");
    assert_eq!(
        root_key_binding_handle.source_id().as_str(),
        "root-shortcuts"
    );
    let root_key_binding_projection: command_core::CommandKeyBindingProjection =
        root_key_binding_registry.project(root_command_center.actions());
    let root_key_binding_conflict: &command_core::CommandKeyBindingConflict =
        &root_key_binding_projection.conflicts()[0];
    let root_key_binding_conflict_entry: &command_core::CommandKeyBindingConflictEntry =
        &root_key_binding_conflict.entries()[0];
    assert_eq!(root_key_binding_conflict_entry.command_id(), "root.open");
    let root_projected_entry: &command_core::CommandKeyBindingProjectedEntry =
        &root_key_binding_projection.projected_entries()[0];
    assert_eq!(root_projected_entry.command_id(), "root.open");
    assert_eq!(root_projected_entry.raw_keystrokes(), "ctrl-o");
    let root_edit_target: command_core::CommandKeyBindingEditTarget =
        root_projected_entry.edit_target();
    let root_key_binding_patch: command_core::CommandKeyBindingPatch =
        command_core::CommandKeyBindingPatch::replace(
            root_edit_target.clone(),
            command_core::CommandKeyBinding::new("root.open", "ctrl-shift-o").context("Root"),
        );
    let _root_patch_operation: command_core::CommandKeyBindingPatchOperation =
        root_key_binding_patch.operation();
    let root_key_binding_patch_preview: command_core::CommandKeyBindingPatchPreview =
        root_key_binding_registry.preview_patch(
            root_command_center.actions(),
            root_key_binding_patch.clone(),
        );
    let _root_patch_outcome: command_core::CommandKeyBindingPatchOutcome =
        root_key_binding_patch_preview.outcome();
    let mut root_keymap = open_gpui::Keymap::default();
    let root_key_binding_report: command_core::CommandKeyBindingInstallReport =
        root_key_binding_registry
            .install_into_keymap(root_command_center.actions(), &mut root_keymap);
    assert_eq!(root_key_binding_report.installed_count(), 2);
    let _root_key_sequence = command_core::parse_command_key_sequence("ctrl-o").unwrap();
    let root_keymap_resolution: command_core::CommandKeymapResolution = root_command_center
        .resolve_key_sequence_for_keymap("ctrl-o", &root_keymap)
        .unwrap();
    let root_keymap_command: &command_core::CommandKeymapResolvedCommand = root_keymap_resolution
        .primary_dispatchable_command()
        .unwrap();
    let _root_keymap_command_state: &command_core::CommandKeymapCommandState =
        root_keymap_command.state();
    let _root_key_binding_diagnostic: &command_core::CommandKeyBindingDiagnostic =
        &root_key_binding_projection.diagnostics()[0];
    let _root_key_binding_diagnostic_kind: command_core::CommandKeyBindingDiagnosticKind =
        _root_key_binding_diagnostic.kind();
    fn root_provider_fn(
        _: &command_core::CommandProviderRequest,
    ) -> command_core::CommandProviderResponse {
        command_core::CommandProviderResponse::ready()
    }
    fn assert_root_provider<T: command_core::CommandProvider>(_: &T) {}
    let _root_provider_id = command_core::CommandProviderId::new("root-provider");
    let _root_manual_provider_request =
        command_core::CommandProviderRequest::new("open").active_scopes(["root"]);
    let _root_provider_request_id = command_core::CommandProviderRequestId::new(1);
    let root_provider_request = root_command_center.begin_provider_request("root-provider", "open");
    let root_provider_response = command_core::CommandProviderResponse::loading("Loading").source(
        command_core::CommandProviderSource::new(
            "root",
            "root-provider-source",
            [command_core::CommandContribution::new(
                command_core::CommandDescriptor::new("root.provider", "Provider"),
            )],
        ),
    );
    let root_provider_outcome: command_core::CommandProviderApplyOutcome = root_command_center
        .apply_provider_response_for_request(
            "root-provider",
            &root_provider_request,
            root_provider_response,
        )
        .unwrap();
    let _root_provider_stale: Option<&command_core::CommandProviderStaleResponse> =
        root_provider_outcome.stale_response();
    let _root_provider_status: &command_core::CommandProviderStatus =
        root_provider_outcome.status().unwrap();
    let root_provider_controller =
        command_core::CommandProviderRefreshController::new("root-provider")
            .with_loading_message("Loading");
    let root_provider_projection: command_core::CommandProviderRefreshProjection =
        root_provider_controller.snapshot(&root_command_center);
    let root_provider_palette_projection: root::CommandProviderPaletteProjection =
        root::CommandProviderPaletteProjection::from_refresh_projection(&root_provider_projection);
    let _root_provider_palette_query = root_provider_palette_projection.query();
    let _root_provider_palette_snapshot: root::CommandIndexSnapshot =
        root_provider_palette_projection.into_index_snapshot();
    let root_palette_projection: root::CommandPaletteProjection =
        root::CommandPaletteProjection::from_center_for_keymap(
            &root_command_center,
            "open",
            &open_gpui::Keymap::default(),
        );
    let _root_palette_projection_diagnostics: &[command_core::CommandShortcutDiagnostic] =
        root_palette_projection.shortcut_diagnostics();
    let _root_palette_projection_status_items: &[root::CommandStatusItem] =
        root_palette_projection.status_items();
    let mut root_palette_controller: root::CommandPaletteController =
        root::CommandPaletteController::new()
            .with_query("open")
            .provider_with_loading("root-provider", "Loading");
    let root_palette_update: root::CommandPaletteControllerUpdate = root_palette_controller
        .set_query_for_keymap(
            &mut root_command_center,
            "open",
            &open_gpui::Keymap::default(),
        )
        .unwrap();
    let _root_palette_update_projection: &root::CommandPaletteProjection =
        root_palette_update.palette_projection();
    let root_palette_preflight: root::CommandPaletteKeymapPreflight = root_palette_controller
        .preflight_key_sequence_for_keymap(&root_command_center, "ctrl-o", &root_keymap)
        .unwrap();
    let _root_palette_preflight_resolution: &command_core::CommandKeymapResolution =
        root_palette_preflight.resolution();
    let _root_palette_preflight_command: Option<&command_core::CommandKeymapResolvedCommand> =
        root_palette_preflight.primary_dispatchable_command();
    let root_shortcut_inspector: root::CommandShortcutInspectorState =
        root::CommandShortcutInspectorState::from_preflight(&root_palette_preflight);
    let _root_shortcut_inspector_command: Option<&root::CommandShortcutInspectorCommand> =
        root_shortcut_inspector.matched_commands().first();
    let root_keybinding_editor: root::CommandKeyBindingEditorState =
        root::CommandKeyBindingEditorState::from_projection(
            &root_key_binding_projection,
            root::CommandKeyBindingEditorFilter::new()
                .query("root")
                .conflicts_only(),
        );
    let _root_keybinding_editor_mode: root::CommandKeyBindingEditorFilterMode =
        root_keybinding_editor.mode();
    let _root_keybinding_editor_row: Option<&root::CommandKeyBindingEditorRow> =
        root_keybinding_editor.rows().first();
    let _root_keybinding_capture: root::CommandKeyBindingCaptureState =
        root::CommandKeyBindingCaptureState::from_sequence("ctrl-shift-o");
    let _root_keybinding_preview: root::CommandKeyBindingEditorPreviewState =
        root::CommandKeyBindingEditorPreviewState::from_patch_preview(
            &root_key_binding_patch_preview,
            root::CommandKeyBindingEditorFilter::new(),
        );
    let _root_pending_provider_request: root::CommandPalettePendingProviderRequest =
        root::CommandPalettePendingProviderRequest::new(
            "root-provider",
            command_core::CommandProviderRequest::new("open"),
        );
    let root_pending_provider_requests: &[root::CommandPalettePendingProviderRequest] =
        root_palette_update.pending_provider_requests();
    if let Some(pending) = root_palette_update.pending_provider_request("root-provider") {
        let _root_pending_provider_id: &command_core::CommandProviderId = pending.provider_id();
        let _root_pending_request: &command_core::CommandProviderRequest = pending.request();
    }
    assert_eq!(root_pending_provider_requests.len(), 1);
    let _root_provider_state = command_core::CommandProviderState::Ready;
    assert_root_provider(
        &(root_provider_fn
            as fn(&command_core::CommandProviderRequest) -> command_core::CommandProviderResponse),
    );
    let root_provider_handle: command_core::CommandProviderHandle =
        root_command_center.register_provider("root-provider-callback", root_provider_fn);
    let _root_provider_registration: command_core::CommandProviderRegistration =
        root_provider_handle.clone();
    let _root_command_actions = command_core::GpuiCommandActionMap::new();
    let _root_command_outcome = command_core::CommandDispatchOutcome::MissingAction;
    let root_command_items = vec![root::CommandItem::new("open", "Open")];
    let root_command_snapshot = root::CommandIndexSnapshot::new("root-v1")
        .mode(root::CommandIndexSnapshotMode::PreRankedFilter)
        .item(root::CommandItemDescriptor::from_command_descriptor(
            &root_core_command,
        ))
        .command_descriptors(root_registry_snapshot.descriptors());
    let root_command_snapshot: root::CommandBehaviorSnapshot =
        root::Command::new("root-command-plan", "Commands")
            .items(root_command_items)
            .index_snapshot(root_command_snapshot)
            .behavior_snapshot();
    let _root_provider_command_state = root::Command::new("root-provider-plan", "Provider")
        .provider_refresh_projection(&root_provider_projection)
        .state();
    let _root_palette_command_state = root::Command::new("root-palette-plan", "Palette")
        .palette_projection(&root_palette_projection)
        .state();
    let _root_status_command_state = root::Command::new("root-status-plan", "Status")
        .status_item(root_command_status_item)
        .state();
    let _root_navigation_command_state = root::Command::new("root-navigation-plan", "Navigation")
        .navigation_behavior(root_command_navigation)
        .loop_navigation(true)
        .group_navigation(true)
        .state();
    let _root_command_row: Option<&root::CommandRowBehaviorSnapshot> =
        root_command_snapshot.rows().first();
    let root_menu_state = root::Menu::new("root-menu", "Menu")
        .default_open(true)
        .default_focused_value("more")
        .item(root::MenuItem::submenu(
            "more",
            "More",
            [root::MenuItem::action("nested", "Nested")],
        ))
        .state();
    let root_menu_submenu_navigation: root::MenuSubmenuNavigation = root_menu_state
        .submenu_navigation_target("right")
        .expect("root MenuSubmenuNavigation should be exported");
    let root_menu_submenu_surface: root::MenuSubmenuSurface = root::MenuSubmenuSurface::resolve(
        rect(
            ui_point(ui_px(0.0), ui_px(0.0)),
            ui_size(ui_px(120.0), ui_px(32.0)),
        ),
        ui_size(ui_px(180.0), ui_px(96.0)),
        OverlayPlacementSide::Right,
        OverlayPlacementAlignment::Start,
        UiPx::ZERO,
        None,
    );
    let root_menu_safe_hover_corridor: root::MenuSafeHoverCorridor =
        root_menu_submenu_surface.hover_corridor();
    let root_scroll = root::ScrollArea::new("scroll", div());
    let root_splitter = root::Splitter::new("split");
    let root_tabs = root::Tabs::new("tabs");
    let root_global_filter = root::TableGlobalFilter::new("global-filter", "Search");
    let root_predicate_filter = root::TablePredicateFilter::new("predicate-filter", "Name", "name");
    let root_table_toolbar =
        root::TableToolbar::new("table-toolbar", "Filters").summary("2 rows visible");
    let root_faceted_filter = root::TableFacetedFilter::new("status-filter", "Status", "status");
    let root_column_visibility = root::TableColumnVisibility::new("column-visibility", "Columns")
        .columns([ui_core::TableColumn::new("status", "Status")]);
    let root_avatar = root::Avatar::new("avatar", "Ada Lovelace");
    let root_separator = root::Separator::new("separator");
    let root_kbd = root::Kbd::new("kbd", "Ctrl+K");
    let root_progress = root::Progress::new("progress", "Progress");
    let root_skeleton = root::Skeleton::new("skeleton");
    let root_status_cue = root::StatusCue::new("status", "Ready");
    let root_empty_state = root::EmptyState::new("empty", "No results");
    let root_collapsible = root::Collapsible::new("collapsible", "Details").default_open(true);
    let root_slider = root::Slider::new("slider", "Volume").value(40.0);
    let root_number_input = root::NumberInput::new("number", "Quantity").value(3.0);
    let root_link = root::Link::new("link", "Docs", "/docs").external(true);
    let root_breadcrumb = root::Breadcrumb::new("breadcrumb", "Path")
        .item(root::BreadcrumbItemDescriptor::new("home", "Home").href("/"))
        .item(root::BreadcrumbItemDescriptor::new("docs", "Docs").current(true));
    let root_tag = root::Tag::new("tag", "ready", "Ready").removable(true);
    let root_toast_stack = root::ToastStack::new("toasts", "Notifications")
        .toast(root::Toast::new("saved", "Saved").intent(root::ToastIntent::Success));
    let root_toggle_group = root::ToggleGroup::new("toggle-group", "Alignment")
        .item(root::ToggleGroupItem::new("left", "Left"))
        .item(root::ToggleGroupItem::new("right", "Right"))
        .selected_values(["left"]);
    let prelude_button = prelude::Button::new("save", "Save");
    let prelude_accordion = prelude::Accordion::new("accordion")
        .mode(prelude::AccordionMode::Single)
        .item(prelude::AccordionItem::new("one", "One", "One content"));
    let prelude_alert_dialog = prelude::AlertDialog::new(
        "delete",
        "Delete",
        "Delete item?",
        "This removes it.",
        "Delete",
    );
    let prelude_sheet = prelude::Sheet::new("sheet", "Open sheet", "Sheet", "Sheet content");
    let prelude_hover_card = prelude::HoverCard::new("hover-card", "Profile", "Profile details");
    let prelude_sidebar = prelude::Sidebar::new("sidebar", "Primary navigation");
    let prelude_toolbar = prelude::Toolbar::new("toolbar", "Editor");
    let prelude_listbox = prelude::Listbox::new("listbox", "Choices");
    let prelude_select = prelude::Select::new("select", "Choice");
    let prelude_select_option = ui_core::TableSelectOption::new("blocked", "Blocked");
    let prelude_combobox = prelude::Combobox::new("combobox", "Search");
    let prelude_command = prelude::Command::new("command", "Commands");
    let prelude_command_status_item: prelude::CommandStatusItem =
        prelude::CommandStatusItem::error("Provider failed");
    let _prelude_command_status_intent: prelude::CommandStatusIntent =
        prelude_command_status_item.intent();
    let prelude_command_navigation: prelude::CommandNavigationBehavior =
        prelude::CommandNavigationBehavior::new()
            .with_loop_navigation(false)
            .with_group_navigation(true);
    assert!(!prelude_command_navigation.loop_navigation());
    assert!(prelude_command_navigation.group_navigation());
    let prelude_core_command =
        command_core::CommandDescriptor::new("prelude.open", "Open").shortcut("Ctrl+O");
    let prelude_registry_snapshot = command_core::CommandRegistrySnapshot::new(
        "prelude-registry-v1",
        [
            command_core::CommandContribution::new(prelude_core_command.clone())
                .source("prelude-workspace"),
        ],
    );
    let prelude_shortcut_diagnostics: Vec<command_core::CommandShortcutDiagnostic> =
        command_core::GpuiCommandActionMap::new().shortcut_diagnostics_for_keymap(
            &prelude_registry_snapshot,
            &open_gpui::Keymap::default(),
        );
    let _prelude_shortcut_diagnostic_kind: command_core::CommandShortcutDiagnosticKind =
        prelude_shortcut_diagnostics[0].kind();
    let mut prelude_command_center = command_core::CommandCenter::new("prelude-center-v1");
    let prelude_context_stack: command_core::CommandContextStack =
        command_core::CommandContextStack::new()
            .scope("prelude")
            .key_context(open_gpui::KeyContext::parse("Prelude").unwrap());
    prelude_command_center.set_context_stack(prelude_context_stack);
    assert_eq!(
        prelude_command_center.active_scopes()[0].as_str(),
        "prelude"
    );
    assert_eq!(
        prelude_command_center.key_contexts()[0]
            .primary()
            .map(|entry| entry.key.as_ref()),
        Some("Prelude")
    );
    let prelude_command_source: command_core::CommandSourceHandle = prelude_command_center
        .register_source(
            "prelude",
            "prelude-source",
            [command_core::CommandContribution::new(
                prelude_core_command.clone(),
            )],
        )
        .unwrap();
    let _prelude_command_source_registration: command_core::CommandSourceRegistration =
        prelude_command_source.clone();
    prelude_command_center
        .register_action("prelude.open", PreludeOpen)
        .register_action("prelude.save", PreludeSave);
    let prelude_key_binding =
        command_core::CommandKeyBinding::new("prelude.open", "ctrl-o").context("Prelude");
    let mut prelude_key_binding_registry = command_core::CommandKeyBindingRegistry::new();
    let prelude_key_binding_handle: command_core::CommandKeyBindingHandle =
        prelude_key_binding_registry.register(
            "prelude-shortcuts",
            [
                prelude_key_binding,
                command_core::CommandKeyBinding::new("prelude.save", "ctrl-o").context("Prelude"),
                command_core::CommandKeyBinding::new("prelude.missing", "ctrl-m"),
            ],
        );
    let prelude_key_binding_entry: &command_core::CommandKeyBindingEntry =
        &prelude_key_binding_registry.entries()[0];
    assert_eq!(
        prelude_key_binding_entry.binding().command_id(),
        "prelude.open"
    );
    assert_eq!(
        prelude_key_binding_handle.source_id().as_str(),
        "prelude-shortcuts"
    );
    let prelude_key_binding_projection: command_core::CommandKeyBindingProjection =
        prelude_key_binding_registry.project(prelude_command_center.actions());
    let prelude_key_binding_conflict: &command_core::CommandKeyBindingConflict =
        &prelude_key_binding_projection.conflicts()[0];
    let prelude_key_binding_conflict_entry: &command_core::CommandKeyBindingConflictEntry =
        &prelude_key_binding_conflict.entries()[0];
    assert_eq!(
        prelude_key_binding_conflict_entry.command_id(),
        "prelude.open"
    );
    let prelude_projected_entry: &command_core::CommandKeyBindingProjectedEntry =
        &prelude_key_binding_projection.projected_entries()[0];
    assert_eq!(prelude_projected_entry.command_id(), "prelude.open");
    assert_eq!(prelude_projected_entry.raw_keystrokes(), "ctrl-o");
    let prelude_edit_target: command_core::CommandKeyBindingEditTarget =
        prelude_projected_entry.edit_target();
    let prelude_key_binding_patch: command_core::CommandKeyBindingPatch =
        command_core::CommandKeyBindingPatch::replace(
            prelude_edit_target.clone(),
            command_core::CommandKeyBinding::new("prelude.open", "ctrl-shift-o").context("Prelude"),
        );
    let _prelude_patch_operation: command_core::CommandKeyBindingPatchOperation =
        prelude_key_binding_patch.operation();
    let prelude_key_binding_patch_preview: command_core::CommandKeyBindingPatchPreview =
        prelude_key_binding_registry.preview_patch(
            prelude_command_center.actions(),
            prelude_key_binding_patch.clone(),
        );
    let _prelude_patch_outcome: command_core::CommandKeyBindingPatchOutcome =
        prelude_key_binding_patch_preview.outcome();
    let mut prelude_keymap = open_gpui::Keymap::default();
    let prelude_key_binding_report: command_core::CommandKeyBindingInstallReport =
        prelude_key_binding_registry
            .install_into_keymap(prelude_command_center.actions(), &mut prelude_keymap);
    assert_eq!(prelude_key_binding_report.installed_count(), 2);
    let _prelude_key_sequence = command_core::parse_command_key_sequence("ctrl-o").unwrap();
    let prelude_keymap_resolution: command_core::CommandKeymapResolution = prelude_command_center
        .resolve_key_sequence_for_keymap("ctrl-o", &prelude_keymap)
        .unwrap();
    let prelude_keymap_command: &command_core::CommandKeymapResolvedCommand =
        prelude_keymap_resolution
            .primary_dispatchable_command()
            .unwrap();
    let _prelude_keymap_command_state: &command_core::CommandKeymapCommandState =
        prelude_keymap_command.state();
    let prelude_key_binding_diagnostic: &command_core::CommandKeyBindingDiagnostic =
        &prelude_key_binding_projection.diagnostics()[0];
    let _prelude_key_binding_diagnostic_kind: command_core::CommandKeyBindingDiagnosticKind =
        prelude_key_binding_diagnostic.kind();
    fn prelude_provider_fn(
        _: &command_core::CommandProviderRequest,
    ) -> command_core::CommandProviderResponse {
        command_core::CommandProviderResponse::ready()
    }
    fn assert_prelude_provider<T: command_core::CommandProvider>(_: &T) {}
    let _prelude_provider_id = command_core::CommandProviderId::new("prelude-provider");
    let _prelude_manual_provider_request =
        command_core::CommandProviderRequest::new("open").active_scopes(["prelude"]);
    let _prelude_provider_request_id = command_core::CommandProviderRequestId::new(1);
    let prelude_provider_request =
        prelude_command_center.begin_provider_request("prelude-provider", "open");
    let prelude_provider_response = command_core::CommandProviderResponse::failed("Unavailable")
        .source(command_core::CommandProviderSource::new(
            "prelude",
            "prelude-provider-source",
            [command_core::CommandContribution::new(
                command_core::CommandDescriptor::new("prelude.provider", "Provider"),
            )],
        ));
    let prelude_provider_outcome: command_core::CommandProviderApplyOutcome =
        prelude_command_center
            .apply_provider_response_for_request(
                "prelude-provider",
                &prelude_provider_request,
                prelude_provider_response,
            )
            .unwrap();
    let _prelude_provider_stale: Option<&command_core::CommandProviderStaleResponse> =
        prelude_provider_outcome.stale_response();
    let _prelude_provider_status: &command_core::CommandProviderStatus =
        prelude_provider_outcome.status().unwrap();
    let prelude_provider_controller =
        command_core::CommandProviderRefreshController::new("prelude-provider")
            .with_loading_message("Loading");
    let prelude_provider_projection: command_core::CommandProviderRefreshProjection =
        prelude_provider_controller.snapshot(&prelude_command_center);
    let prelude_provider_palette_projection: prelude::CommandProviderPaletteProjection =
        prelude::CommandProviderPaletteProjection::from_refresh_projection(
            &prelude_provider_projection,
        );
    let _prelude_provider_palette_query = prelude_provider_palette_projection.query();
    let _prelude_provider_palette_snapshot: prelude::CommandIndexSnapshot =
        prelude_provider_palette_projection.into_index_snapshot();
    let prelude_palette_projection: prelude::CommandPaletteProjection =
        prelude::CommandPaletteProjection::from_center_for_keymap(
            &prelude_command_center,
            "open",
            &open_gpui::Keymap::default(),
        );
    let _prelude_palette_projection_diagnostics: &[command_core::CommandShortcutDiagnostic] =
        prelude_palette_projection.shortcut_diagnostics();
    let _prelude_palette_projection_status_items: &[prelude::CommandStatusItem] =
        prelude_palette_projection.status_items();
    let mut prelude_palette_controller: prelude::CommandPaletteController =
        prelude::CommandPaletteController::new()
            .with_query("open")
            .provider_with_loading("prelude-provider", "Loading");
    let prelude_palette_update: prelude::CommandPaletteControllerUpdate =
        prelude_palette_controller
            .set_query_for_keymap(
                &mut prelude_command_center,
                "open",
                &open_gpui::Keymap::default(),
            )
            .unwrap();
    let _prelude_palette_update_projection: &prelude::CommandPaletteProjection =
        prelude_palette_update.palette_projection();
    let prelude_palette_preflight: prelude::CommandPaletteKeymapPreflight =
        prelude_palette_controller
            .preflight_key_sequence_for_keymap(&prelude_command_center, "ctrl-o", &prelude_keymap)
            .unwrap();
    let _prelude_palette_preflight_resolution: &command_core::CommandKeymapResolution =
        prelude_palette_preflight.resolution();
    let _prelude_palette_preflight_command: Option<&command_core::CommandKeymapResolvedCommand> =
        prelude_palette_preflight.primary_dispatchable_command();
    let prelude_shortcut_inspector: prelude::CommandShortcutInspectorState =
        prelude::CommandShortcutInspectorState::from_preflight(&prelude_palette_preflight);
    let _prelude_shortcut_inspector_command: Option<&prelude::CommandShortcutInspectorCommand> =
        prelude_shortcut_inspector.matched_commands().first();
    let prelude_keybinding_editor: prelude::CommandKeyBindingEditorState =
        prelude::CommandKeyBindingEditorState::from_projection(
            &prelude_key_binding_projection,
            prelude::CommandKeyBindingEditorFilter::new()
                .query("prelude")
                .conflicts_only(),
        );
    let _prelude_keybinding_editor_mode: prelude::CommandKeyBindingEditorFilterMode =
        prelude_keybinding_editor.mode();
    let _prelude_keybinding_editor_row: Option<&prelude::CommandKeyBindingEditorRow> =
        prelude_keybinding_editor.rows().first();
    let _prelude_keybinding_capture: prelude::CommandKeyBindingCaptureState =
        prelude::CommandKeyBindingCaptureState::from_sequence("ctrl-shift-o");
    let _prelude_keybinding_preview: prelude::CommandKeyBindingEditorPreviewState =
        prelude::CommandKeyBindingEditorPreviewState::from_patch_preview(
            &prelude_key_binding_patch_preview,
            prelude::CommandKeyBindingEditorFilter::new(),
        );
    let _prelude_pending_provider_request: prelude::CommandPalettePendingProviderRequest =
        prelude::CommandPalettePendingProviderRequest::new(
            "prelude-provider",
            command_core::CommandProviderRequest::new("open"),
        );
    let prelude_pending_provider_requests: &[prelude::CommandPalettePendingProviderRequest] =
        prelude_palette_update.pending_provider_requests();
    if let Some(pending) = prelude_palette_update.pending_provider_request("prelude-provider") {
        let _prelude_pending_provider_id: &command_core::CommandProviderId = pending.provider_id();
        let _prelude_pending_request: &command_core::CommandProviderRequest = pending.request();
    }
    assert_eq!(prelude_pending_provider_requests.len(), 1);
    let _prelude_provider_state = command_core::CommandProviderState::Loading;
    assert_prelude_provider(
        &(prelude_provider_fn
            as fn(&command_core::CommandProviderRequest) -> command_core::CommandProviderResponse),
    );
    let prelude_provider_handle: command_core::CommandProviderHandle =
        prelude_command_center.register_provider("prelude-provider-callback", prelude_provider_fn);
    let _prelude_provider_registration: command_core::CommandProviderRegistration =
        prelude_provider_handle.clone();
    let _prelude_command_actions = command_core::GpuiCommandActionMap::new();
    let _prelude_command_outcome = command_core::CommandDispatchOutcome::MissingAction;
    let prelude_command_items = vec![prelude::CommandItem::new("open", "Open")];
    let prelude_command_snapshot =
        prelude::CommandIndexSnapshot::from_registry_snapshot(&prelude_registry_snapshot)
            .mode(prelude::CommandIndexSnapshotMode::PreFiltered)
            .item(prelude::CommandItemDescriptor::from_command_descriptor(
                &prelude_core_command,
            ));
    let prelude_command_snapshot: prelude::CommandBehaviorSnapshot =
        prelude::Command::new("prelude-command-plan", "Commands")
            .items(prelude_command_items)
            .index_snapshot(prelude_command_snapshot)
            .behavior_snapshot();
    let _prelude_provider_command_state =
        prelude::Command::new("prelude-provider-plan", "Provider")
            .provider_refresh_projection(&prelude_provider_projection)
            .state();
    let _prelude_palette_command_state = prelude::Command::new("prelude-palette-plan", "Palette")
        .palette_projection(&prelude_palette_projection)
        .state();
    let _prelude_status_command_state = prelude::Command::new("prelude-status-plan", "Status")
        .status_item(prelude_command_status_item)
        .state();
    let _prelude_navigation_command_state =
        prelude::Command::new("prelude-navigation-plan", "Navigation")
            .navigation_behavior(prelude_command_navigation)
            .loop_navigation(true)
            .group_navigation(true)
            .state();
    let _prelude_command_row: Option<&prelude::CommandRowBehaviorSnapshot> =
        prelude_command_snapshot.rows().first();
    let prelude_menu_state = prelude::Menu::new("prelude-menu", "Menu")
        .default_open(true)
        .default_focused_value("more")
        .item(prelude::MenuItem::submenu(
            "more",
            "More",
            [prelude::MenuItem::action("nested", "Nested")],
        ))
        .state();
    let prelude_menu_submenu_navigation: prelude::MenuSubmenuNavigation = prelude_menu_state
        .submenu_navigation_target("right")
        .expect("prelude MenuSubmenuNavigation should be exported");
    let prelude_menu_submenu_surface: prelude::MenuSubmenuSurface =
        prelude::MenuSubmenuSurface::resolve(
            rect(
                ui_point(ui_px(0.0), ui_px(0.0)),
                ui_size(ui_px(120.0), ui_px(32.0)),
            ),
            ui_size(ui_px(180.0), ui_px(96.0)),
            OverlayPlacementSide::Right,
            OverlayPlacementAlignment::Start,
            UiPx::ZERO,
            None,
        );
    let prelude_menu_safe_hover_corridor: prelude::MenuSafeHoverCorridor =
        prelude_menu_submenu_surface.hover_corridor();
    let prelude_scroll = prelude::ScrollArea::new("scroll", div());
    let prelude_splitter = prelude::Splitter::new("split");
    let prelude_tabs = prelude::Tabs::new("tabs");
    let prelude_global_filter = prelude::TableGlobalFilter::new("global-filter", "Search");
    let prelude_predicate_filter =
        prelude::TablePredicateFilter::new("predicate-filter", "Name", "name");
    let prelude_table_toolbar =
        prelude::TableToolbar::new("table-toolbar", "Filters").summary("2 rows visible");
    let prelude_faceted_filter =
        prelude::TableFacetedFilter::new("status-filter", "Status", "status");
    let prelude_column_visibility =
        prelude::TableColumnVisibility::new("column-visibility", "Columns")
            .columns([ui_core::TableColumn::new("status", "Status")]);
    let prelude_avatar = prelude::Avatar::new("avatar", "Ada Lovelace");
    let prelude_separator = prelude::Separator::new("separator");
    let prelude_kbd = prelude::Kbd::new("kbd", "Ctrl+K");
    let prelude_progress = prelude::Progress::new("progress", "Progress");
    let prelude_skeleton = prelude::Skeleton::new("skeleton");
    let prelude_status_cue = prelude::StatusCue::new("status", "Ready");
    let prelude_empty_state = prelude::EmptyState::new("empty", "No results");
    let prelude_collapsible =
        prelude::Collapsible::new("collapsible", "Details").default_open(false);
    let prelude_slider = prelude::Slider::new("slider", "Volume").value(20.0);
    let prelude_number_input = prelude::NumberInput::new("number", "Quantity").value(5.0);
    let prelude_link = prelude::Link::new("link", "Docs", "/docs");
    let prelude_breadcrumb = prelude::Breadcrumb::new("breadcrumb", "Path")
        .items([prelude::BreadcrumbItemDescriptor::new("home", "Home")]);
    let prelude_tag =
        prelude::Tag::new("tag", "ready", "Ready").variant(prelude::TagVariant::Outline);
    let prelude_toast_stack =
        prelude::ToastStack::new("toasts", "Notifications").toasts([prelude::Toast::new(
            "saved", "Saved",
        )
        .action("Undo")
        .pinned()]);
    let prelude_toggle_group = prelude::ToggleGroup::new("toggle-group", "Alignment")
        .mode(prelude::ToggleGroupSelectionMode::Multiple)
        .items([
            prelude::ToggleGroupItem::new("bold", "Bold"),
            prelude::ToggleGroupItem::new("italic", "Italic"),
        ])
        .default_selected_values(["bold"]);

    let _ = (
        root_button.state(),
        root_accordion.state(),
        root_alert_dialog.state(),
        root_sheet.state(),
        root_hover_card.state(),
        root_sidebar.state(),
        root_toolbar.state(),
        root_listbox.state(),
        root_select.state(),
        root_select_option.value(),
        root_combobox.state(),
        root_command.state(),
        root_command_snapshot.role(),
        root_menu_submenu_navigation.focused_value(),
        root_menu_safe_hover_corridor.bounds(),
        root_scroll.state(),
        root_splitter.state(),
        root_tabs.state(),
        root_global_filter.state(),
        root_predicate_filter.state(),
        root_table_toolbar.state(),
        root_faceted_filter.state(),
        root_column_visibility.state(),
        root_avatar.state(),
        root_separator.state(),
        root_kbd.state(),
        root_progress.state(),
        root_skeleton.state(),
        root_status_cue.state(),
        root_empty_state.state(),
        root_collapsible.state(),
        root_slider.state(),
        root_number_input.state(),
        root_link.state(),
        root_breadcrumb.state(),
        root_tag.state(),
        root_toast_stack.state(),
        root_toggle_group.state(),
        prelude_button.state(),
        prelude_accordion.state(),
        prelude_alert_dialog.state(),
        prelude_sheet.state(),
        prelude_hover_card.state(),
        prelude_sidebar.state(),
        prelude_toolbar.state(),
        prelude_listbox.state(),
        prelude_select.state(),
        prelude_select_option.value(),
        prelude_combobox.state(),
        prelude_command.state(),
        prelude_command_snapshot.row_role(),
        prelude_menu_submenu_navigation.focused_value(),
        prelude_menu_safe_hover_corridor.bounds(),
        prelude_scroll.state(),
        prelude_splitter.state(),
        prelude_tabs.state(),
        prelude_global_filter.state(),
        prelude_predicate_filter.state(),
        prelude_table_toolbar.state(),
        prelude_faceted_filter.state(),
        prelude_column_visibility.state(),
        prelude_avatar.state(),
        prelude_separator.state(),
        prelude_kbd.state(),
        prelude_progress.state(),
        prelude_skeleton.state(),
        prelude_status_cue.state(),
        prelude_empty_state.state(),
        prelude_collapsible.state(),
        prelude_slider.state(),
        prelude_number_input.state(),
        prelude_link.state(),
        prelude_breadcrumb.state(),
        prelude_tag.state(),
        prelude_toast_stack.state(),
        prelude_toggle_group.state(),
        root::toggle_group_navigation_target(Orientation::Horizontal, "right", 0, &[false, false]),
        prelude::toggle_group_navigation_target(
            Orientation::Horizontal,
            "right",
            0,
            &[false, false],
        ),
        root_overlay.policy().kind(),
        prelude_overlay.policy().kind(),
    );
}

#[test]
fn public_reexports_stay_explicit_without_wildcards() {
    let mut wildcard_exports = Vec::new();
    let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let files = [
        ("ui_components/src/lib.rs", manifest_dir.join("src/lib.rs")),
        (
            "ui_components/src/prelude.rs",
            manifest_dir.join("src/prelude.rs"),
        ),
        (
            "ui_core/src/lib.rs",
            manifest_dir.join("../ui_core/src/lib.rs"),
        ),
        (
            "ui_core/src/prelude.rs",
            manifest_dir.join("../ui_core/src/prelude.rs"),
        ),
    ];

    for (file_name, path) in files {
        let source = std::fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("failed to read {path:?}: {error}"));

        for (line_number, line) in source.lines().enumerate() {
            if line.contains("pub use ") && line.contains("::*") {
                let trimmed = line.trim();
                if matches!(
                    trimmed,
                    "pub use public_api::default::*;" | "pub use crate::public_api::default::*;"
                ) {
                    continue;
                }
                wildcard_exports.push(format!("{file_name}:{}", line_number + 1));
            }
        }
    }

    assert_eq!(
        wildcard_exports,
        Vec::<String>::new(),
        "public re-exports must stay explicit, including adapter-only groupings"
    );
}

#[test]
fn root_and_prelude_exports_match_contract_default_surface_intent() {
    let contract_defaults = contract_default_surface_tokens();
    let contract_non_defaults = contract_non_default_surface_tokens();

    for file_name in ["lib.rs", "prelude.rs"] {
        let exports = default_reexport_tokens(file_name);
        let missing_defaults = contract_defaults
            .difference(&exports)
            .cloned()
            .collect::<Vec<_>>();
        assert_eq!(
            missing_defaults,
            Vec::<String>::new(),
            "{file_name} default exports are missing contract default surfaces"
        );

        let leaked_non_defaults = contract_non_defaults
            .intersection(&exports)
            .cloned()
            .collect::<Vec<_>>();
        assert_eq!(
            leaked_non_defaults,
            Vec::<String>::new(),
            "{file_name} default exports leaked contract non-default surfaces"
        );
    }
}

#[test]
fn crate_root_and_prelude_reexports_stay_intentionally_aligned() {
    let root_exports = default_reexport_tokens("lib.rs");
    let prelude_exports = default_reexport_tokens("prelude.rs");
    let root_only = root_exports
        .difference(&prelude_exports)
        .cloned()
        .collect::<Vec<_>>();
    let prelude_only = prelude_exports
        .difference(&root_exports)
        .cloned()
        .collect::<Vec<_>>();

    assert_eq!(
        root_only,
        Vec::<String>::new(),
        "crate root exports tokens not exposed through prelude; update prelude.rs or document the intentional root-only token here"
    );
    assert_eq!(
        prelude_only,
        vec![
            "ActiveDescendant".to_string(),
            "CollectionPosition".to_string(),
            "ControllableState".to_string(),
            "Sizable".to_string(),
            "Size".to_string(),
            "ThemeTokens".to_string(),
            "UiA11yElementExt".to_string(),
        ],
        "prelude-only exports must stay intentional; update the allowlist when the convenience prelude grows"
    );
}
