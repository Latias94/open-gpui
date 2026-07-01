use super::*;

#[test]
fn crate_root_and_prelude_exports_remain_explicit() {
    use open_gpui_ui_components::{self as root, prelude};

    let root_overlay: root::OverlayResolvedState = root::OverlayResolvedState::resolve(
        OverlayLayerPolicy::new(OverlayLayerKind::Tooltip, OverlayPresence::open()),
    );
    let prelude_overlay: prelude::OverlayResolvedState = prelude::OverlayResolvedState::resolve(
        OverlayLayerPolicy::new(OverlayLayerKind::Tooltip, OverlayPresence::open()),
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
    let root_select_option = root::TableSelectOption::new("ready", "Ready");
    let root_combobox = root::Combobox::new("combobox", "Search");
    let root_command = root::Command::new("command", "Commands");
    let root_command_items = vec![root::CommandItem::new("open", "Open")];
    let root_command_snapshot = root::CommandIndexSnapshot::new("root-v1")
        .mode(root::CommandIndexSnapshotMode::PreRankedFilter)
        .item(root::CommandItemDescriptor::new("open", "Open"));
    let root_command_snapshot: root::CommandBehaviorSnapshot =
        root::Command::new("root-command-plan", "Commands")
            .items(root_command_items)
            .index_snapshot(root_command_snapshot)
            .behavior_snapshot();
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
        .columns([root::TableColumn::new("status", "Status")]);
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
    let prelude_select_option = prelude::TableSelectOption::new("blocked", "Blocked");
    let prelude_combobox = prelude::Combobox::new("combobox", "Search");
    let prelude_command = prelude::Command::new("command", "Commands");
    let prelude_command_items = vec![prelude::CommandItem::new("open", "Open")];
    let prelude_command_snapshot = prelude::CommandIndexSnapshot::new("prelude-v1")
        .mode(prelude::CommandIndexSnapshotMode::PreFiltered)
        .item(prelude::CommandItemDescriptor::new("open", "Open"));
    let prelude_command_snapshot: prelude::CommandBehaviorSnapshot =
        prelude::Command::new("prelude-command-plan", "Commands")
            .items(prelude_command_items)
            .index_snapshot(prelude_command_snapshot)
            .behavior_snapshot();
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
            .columns([prelude::TableColumn::new("status", "Status")]);
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
