use super::*;
use open_gpui_command as command_core;
use open_gpui_ui_core as ui_core;

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
    root_a11y_contract.validate().unwrap();
    prelude_a11y_contract.validate().unwrap();

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
    let root_command = root::Command::new("command", "Commands")
        .item(root::CommandItem::new("open", "Open"))
        .status_item(root::CommandStatusItem::warning("Shortcut diagnostics"));
    let root_action = root::ActionDescriptor::new("workspace.open", "Open Workspace")
        .icon(root::ActionIconDescriptor::new("missing-workspace").fallback_label("O"))
        .resolve_with(&|icon: &root::ActionIconDescriptor| {
            root::ResolvedActionIcon::missing(icon.clone(), "icon asset is not registered")
        });
    let root_action_icon_diagnostic: root::ActionIconDiagnostic =
        root_action.diagnostics()[0].clone();
    assert_eq!(root_action_icon_diagnostic.icon_name(), "missing-workspace");
    assert!(root_action.has_diagnostics());
    let prelude_action = prelude::ActionDescriptor::new("workspace.save", "Save Workspace")
        .icon(prelude::ActionIconDescriptor::new("workspace-save").fallback_label("S"))
        .resolve_with(&|icon: &prelude::ActionIconDescriptor| {
            prelude::ResolvedActionIcon::resolved(icon.clone(), "S")
        });
    let prelude_action_icon: prelude::ResolvedActionIcon =
        prelude_action.icon().expect("resolved icon").clone();
    assert_eq!(prelude_action_icon.label(), Some("S"));
    let root_command_navigation = root::CommandNavigationBehavior::new()
        .with_loop_navigation(false)
        .with_group_navigation(true);
    let root_menu_state = root::Menu::new("root-menu", "Menu")
        .default_open(true)
        .default_focused_value("more")
        .item(root::MenuItem::submenu(
            "more",
            "More",
            [root::MenuItem::action("nested", "Nested")],
        ))
        .state();
    let root_menu_submenu_navigation = root_menu_state
        .submenu_navigation_target("right")
        .expect("root MenuSubmenuNavigation should be exported");
    let root_menu_submenu_surface = root::MenuSubmenuSurface::resolve(
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
    let root_menu_safe_hover_corridor = root_menu_submenu_surface.hover_corridor();
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
    let root_form_control = root::FormControlState::new(ui_core::Size::Medium).with_required(true);
    let root_theme_context = root::ThemeContext::light();

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
    let prelude_command = prelude::Command::new("command", "Commands")
        .item(prelude::CommandItem::new("open", "Open"))
        .status_item(prelude::CommandStatusItem::error("Provider failed"));
    let root_command_navigation_for_prelude_case = root::CommandNavigationBehavior::new()
        .with_loop_navigation(false)
        .with_group_navigation(true);
    let prelude_menu_state = prelude::Menu::new("prelude-menu", "Menu")
        .default_open(true)
        .default_focused_value("more")
        .item(prelude::MenuItem::submenu(
            "more",
            "More",
            [prelude::MenuItem::action("nested", "Nested")],
        ))
        .state();
    let prelude_menu_submenu_navigation = prelude_menu_state
        .submenu_navigation_target("right")
        .expect("prelude MenuSubmenuNavigation should be exported");
    let prelude_menu_submenu_surface = root::MenuSubmenuSurface::resolve(
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
    let prelude_menu_safe_hover_corridor = prelude_menu_submenu_surface.hover_corridor();
    let prelude_scroll = prelude::ScrollArea::new("scroll", div());
    let prelude_splitter = prelude::Splitter::new("split");
    let prelude_tabs = prelude::Tabs::new("tabs");
    let root_global_filter_for_prelude_case =
        root::TableGlobalFilter::new("global-filter", "Search");
    let root_predicate_filter_for_prelude_case =
        root::TablePredicateFilter::new("predicate-filter", "Name", "name");
    let root_table_toolbar_for_prelude_case =
        root::TableToolbar::new("table-toolbar", "Filters").summary("2 rows visible");
    let root_faceted_filter_for_prelude_case =
        root::TableFacetedFilter::new("status-filter", "Status", "status");
    let root_column_visibility_for_prelude_case =
        root::TableColumnVisibility::new("column-visibility", "Columns")
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
    let prelude_form_control =
        prelude::FormControlState::new(ui_core::Size::Small).with_invalid(true);
    let prelude_theme_context = prelude::ThemeContext::dark();

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
        root_command_navigation.group_navigation(),
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
        root_form_control.required(),
        root_theme_context.mode(),
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
        root_command_navigation_for_prelude_case.group_navigation(),
        prelude_menu_submenu_navigation.focused_value(),
        prelude_menu_safe_hover_corridor.bounds(),
        prelude_scroll.state(),
        prelude_splitter.state(),
        prelude_tabs.state(),
        root_global_filter_for_prelude_case.state(),
        root_predicate_filter_for_prelude_case.state(),
        root_table_toolbar_for_prelude_case.state(),
        root_faceted_filter_for_prelude_case.state(),
        root_column_visibility_for_prelude_case.state(),
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
        prelude_form_control.invalid(),
        prelude_theme_context.mode(),
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
fn advanced_owner_surfaces_use_explicit_import_paths() {
    use open_gpui_ui_components as root;

    let command_descriptor =
        command_core::CommandDescriptor::new("owner.open", "Open").shortcut("Ctrl+O");
    let mut command_registry = command_core::CommandRegistry::new("owner-registry-v1");
    command_registry
        .register_contribution(
            command_core::CommandContribution::new(command_descriptor.clone())
                .source("owner-workspace"),
        )
        .unwrap();
    let registry_snapshot = command_registry.snapshot();
    let command_snapshot =
        root::command::CommandIndexSnapshot::from_registry_snapshot(&registry_snapshot);
    let command_state = root::command::Command::new("owner-command", "Commands")
        .index_snapshot(command_snapshot)
        .state();
    assert_eq!(command_state.items().len(), 1);

    let _table_column = ui_core::TableColumn::new("status", "Status");
    let _table_select_option = ui_core::TableSelectOption::new("ready", "Ready");
    let _virtualizer_state: Option<ui_core::VirtualizerState> = None;
    let _virtualizer_snapshot: Option<ui_core::VirtualizerSnapshot> = None;

    let _theme_schema = root::theme::theme_json_schema();
    let _theme_registry = root::theme::ThemeRegistry::with_builtins();
    let _theme_runtime = root::theme::ThemeRuntime::with_builtins();
}

#[test]
fn advanced_owner_surfaces_do_not_leak_from_default_exports() {
    let forbidden = [
        "CommandRegistry",
        "CommandCenter",
        "CommandKeyBindingRegistry",
        "CommandProvider",
        "CommandDescriptor",
        "GpuiCommandActionMap",
        "TableColumn",
        "TableState",
        "TableRow",
        "VirtualizerState",
        "VirtualizerSnapshot",
        "GridViewport2D",
        "ThemeRegistry",
        "ThemeRuntime",
        "theme_json_schema",
        "register_theme_json_str",
    ];

    for file_name in ["lib.rs", "prelude.rs"] {
        let exports = default_reexport_tokens(file_name);
        let leaked = forbidden
            .iter()
            .filter(|token| exports.contains(**token))
            .copied()
            .collect::<Vec<_>>();
        assert_eq!(
            leaked,
            Vec::<&str>::new(),
            "{file_name} default exports leaked advanced owner surfaces"
        );
    }
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
                    "pub use public_api::default::*;"
                        | "pub use crate::public_api::default::*;"
                        | "pub use crate::public_api::common::*;"
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

    let root_exports = default_reexport_tokens("lib.rs");
    let missing_root_defaults = contract_defaults
        .difference(&root_exports)
        .cloned()
        .collect::<Vec<_>>();
    assert_eq!(
        missing_root_defaults,
        Vec::<String>::new(),
        "crate root default exports are missing contract default surfaces"
    );

    let leaked_root_non_defaults = contract_non_defaults
        .intersection(&root_exports)
        .cloned()
        .collect::<Vec<_>>();
    assert_eq!(
        leaked_root_non_defaults,
        Vec::<String>::new(),
        "crate root default exports leaked contract non-default surfaces"
    );

    let prelude_exports = default_reexport_tokens("prelude.rs");
    let forbidden_prelude_exports = [
        "GpuiOverlayAdapterConfig",
        "GpuiOverlayState",
        "TextInputController",
        "UiA11yElementExt",
        "VirtualizedListGpuiExt",
        "TableGlobalFilter",
        "TablePredicateFilter",
        "TableFacetedFilter",
        "TableColumnVisibility",
        "TableRangeFilter",
        "TableToolbar",
        "ToolbarItem",
        "SidebarItem",
        "ListboxOption",
    ];
    let leaked_prelude_exports = forbidden_prelude_exports
        .iter()
        .filter(|token| prelude_exports.contains(**token))
        .copied()
        .collect::<Vec<_>>();
    assert_eq!(
        leaked_prelude_exports,
        Vec::<&str>::new(),
        "prelude/common exports leaked adapter-only, recipe, or internal anatomy surfaces"
    );

    for required in [
        "Button",
        "Dialog",
        "Listbox",
        "Table",
        "Tree",
        "VirtualizedList",
        "ThemeContext",
    ] {
        assert!(
            prelude_exports.contains(required),
            "prelude/common should keep common component token `{required}`"
        );
    }
}

#[test]
fn prelude_reexports_stay_a_curated_subset_of_crate_root_plus_core_helpers() {
    let root_exports = default_reexport_tokens("lib.rs");
    let prelude_exports = default_reexport_tokens("prelude.rs");
    let prelude_only = prelude_exports
        .difference(&root_exports)
        .cloned()
        .collect::<Vec<_>>();

    assert_eq!(
        prelude_only,
        vec![
            "ActiveDescendant".to_string(),
            "CollectionPosition".to_string(),
            "ControllableState".to_string(),
            "Sizable".to_string(),
            "Size".to_string(),
            "ThemeTokens".to_string(),
        ],
        "prelude-only exports must stay intentional; update the allowlist when the convenience prelude grows"
    );
}

#[test]
fn gpui_adapter_helpers_keep_single_public_import_paths() {
    let virtualized_list_module = std::fs::read_to_string(format!(
        "{}/src/virtualized_list/mod.rs",
        env!("CARGO_MANIFEST_DIR")
    ))
    .expect("read virtualized_list/mod.rs");
    assert!(
        !virtualized_list_module.contains("VirtualizedListGpuiExt"),
        "VirtualizedListGpuiExt must stay out of open_gpui_ui_components::virtualized_list"
    );

    let primitives_module = std::fs::read_to_string(format!(
        "{}/src/primitives/mod.rs",
        env!("CARGO_MANIFEST_DIR")
    ))
    .expect("read primitives/mod.rs");
    for token in [
        "trigger_a11y",
        "UiA11yElementExt",
        "gpui_role_from_ui",
        "gpui_orientation_from_ui",
        "gpui_accessible_action_from_ui",
        "gpui_toggled_from_ui",
    ] {
        assert!(
            !primitives_module.contains(token),
            "GPUI adapter helper `{token}` must stay out of open_gpui_ui_components::primitives"
        );
    }

    let lib = std::fs::read_to_string(format!("{}/src/lib.rs", env!("CARGO_MANIFEST_DIR")))
        .expect("read lib.rs");
    let adapter_source = public_module_source(&lib, "gpui_adapter")
        .expect("gpui_adapter module should remain public");
    for token in ["UiA11yElementExt", "VirtualizedListGpuiExt"] {
        assert!(
            adapter_source.contains(token),
            "gpui_adapter should remain the public import path for `{token}`"
        );
    }
}
