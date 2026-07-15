use super::*;

#[test]
fn component_api_inventory_covers_official_gallery_catalog() {
    use std::collections::BTreeSet;

    let inventory_names = COMPONENT_API_INVENTORY
        .iter()
        .map(|entry| entry.component.to_string())
        .collect::<BTreeSet<_>>();
    let contract_official_names = official_component_rows()
        .chain(official_overlay_component_rows())
        .map(|entry| entry.name.to_string())
        .collect::<BTreeSet<_>>();

    let missing = contract_official_names
        .difference(&inventory_names)
        .cloned()
        .collect::<Vec<_>>();
    assert!(
        missing.is_empty(),
        "contract official component entries need public API inventory rows: {missing:?}"
    );
}

#[test]
fn component_contract_rows_cover_inventory_and_adjacent_surfaces() {
    use std::collections::BTreeSet;

    let contract_names = COMPONENT_CONTRACT_ROWS
        .iter()
        .map(|entry| entry.name)
        .collect::<BTreeSet<_>>();
    let inventory_names = COMPONENT_API_INVENTORY
        .iter()
        .map(|entry| entry.component)
        .collect::<BTreeSet<_>>();
    let adjacent_names = PUBLIC_SURFACE_OWNER_MAP
        .iter()
        .map(|entry| entry.name)
        .collect::<BTreeSet<_>>();

    let missing_inventory = inventory_names
        .difference(&contract_names)
        .copied()
        .collect::<Vec<_>>();
    assert_eq!(
        missing_inventory,
        Vec::<&str>::new(),
        "component inventory rows must have canonical contract row metadata"
    );

    let missing_adjacent = adjacent_names
        .difference(&contract_names)
        .copied()
        .collect::<Vec<_>>();
    assert_eq!(
        missing_adjacent,
        Vec::<&str>::new(),
        "adjacent public-surface rows must have canonical contract row metadata"
    );
}

#[test]
fn component_contract_entry_returns_canonical_rows() {
    for entry in COMPONENT_CONTRACT_ROWS {
        let projected = component_contract_entry(entry.name)
            .unwrap_or_else(|| panic!("missing canonical contract row for `{}`", entry.name));
        assert_eq!(
            projected, entry,
            "{} projection should return the canonical contract row",
            entry.name
        );
        assert_eq!(component_source_inputs(entry.name), entry.source_inputs);
    }

    assert_eq!(
        official_component_rows()
            .map(|entry| entry.gallery_status)
            .collect::<std::collections::BTreeSet<_>>(),
        std::collections::BTreeSet::from([SurfaceGalleryStatus::OfficialComponent])
    );
    assert_eq!(
        official_overlay_component_rows()
            .map(|entry| entry.gallery_status)
            .collect::<std::collections::BTreeSet<_>>(),
        std::collections::BTreeSet::from([SurfaceGalleryStatus::OfficialOverlay])
    );
}

#[test]
fn component_contract_rows_are_split_by_responsibility() {
    let contract_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("src")
        .join("component_contract");
    let expected_owners = [
        "types.rs",
        "rows.rs",
        "rows/catalog.rs",
        "projections.rs",
        "surfaces.rs",
        "api_inventory.rs",
        "source_mapping.rs",
        "evidence.rs",
    ];

    for owner in expected_owners {
        assert!(
            contract_dir.join(owner).is_file(),
            "component contract owner `{owner}` should exist"
        );
    }

    let facade = read_source_file(&contract_dir.join("mod.rs"));
    for stale_single_file_fact in [
        "ComponentContractEntry {",
        "ComponentApiInventoryEntry {",
        "PublicSurfaceOwnerEntry {",
        "ComponentA11yEvidence {",
        "ComponentConformanceGate {",
        "pub fn component_public_methods",
        "pub fn component_source_inputs",
    ] {
        assert!(
            !facade.contains(stale_single_file_fact),
            "component_contract/mod.rs should stay a facade, not own `{stale_single_file_fact}`"
        );
    }

    let rows_facade = read_source_file(&contract_dir.join("rows.rs"));
    for stale_single_file_fact in ["ComponentContractEntry {"] {
        assert!(
            !rows_facade.contains(stale_single_file_fact),
            "component_contract/rows.rs should stay a facade, not own `{stale_single_file_fact}`"
        );
    }

    let rows = read_source_file(&contract_dir.join("rows.rs"));
    assert!(rows.contains("COMPONENT_CONTRACT_ROWS"));
    let row_catalog = read_source_file(&contract_dir.join("rows").join("catalog.rs"));
    assert!(row_catalog.contains("ComponentContractEntry {"));
    let inventory = read_source_file(&contract_dir.join("api_inventory.rs"));
    assert!(inventory.contains("COMPONENT_API_INVENTORY"));
    assert!(inventory.contains("component_public_methods"));
    let projections = read_source_file(&contract_dir.join("projections.rs"));
    assert!(projections.contains("gallery_surface_rows"));
    assert!(projections.contains("official_component_rows"));
    let source_mapping = read_source_file(&contract_dir.join("source_mapping.rs"));
    assert!(source_mapping.contains("component_source_inputs"));
    let evidence = read_source_file(&contract_dir.join("evidence.rs"));
    assert!(evidence.contains("COMPONENT_A11Y_EVIDENCE"));
    assert!(evidence.contains("COMPONENT_CONFORMANCE_GATES"));
    assert!(!evidence.contains("ComponentA11yEvidence {"));
    assert!(!evidence.contains("component_a11y_evidence"));
}

#[test]
fn component_contract_queries_derive_overlay_and_recipe_rows() {
    let inventory = COMPONENT_API_INVENTORY
        .iter()
        .map(|entry| entry.component)
        .collect::<std::collections::BTreeSet<_>>();

    let overlays = official_overlay_component_rows()
        .map(|entry| entry.name)
        .collect::<std::collections::BTreeSet<_>>();
    let expected_overlays = COMPONENT_CONTRACT_ROWS
        .iter()
        .filter(|entry| entry.gallery_status == SurfaceGalleryStatus::OfficialOverlay)
        .map(|entry| entry.name)
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(
        overlays, expected_overlays,
        "official overlay query must derive from contract gallery status"
    );

    let official_components = official_component_rows()
        .map(|entry| entry.name)
        .collect::<std::collections::BTreeSet<_>>();
    let expected_official_components = COMPONENT_CONTRACT_ROWS
        .iter()
        .filter(|entry| entry.gallery_status == SurfaceGalleryStatus::OfficialComponent)
        .map(|entry| entry.name)
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(
        official_components, expected_official_components,
        "official component query must derive from contract gallery status"
    );

    let gallery_rows = gallery_surface_rows()
        .map(|entry| entry.name)
        .collect::<std::collections::BTreeSet<_>>();
    let expected_gallery_rows = COMPONENT_CONTRACT_ROWS
        .iter()
        .filter(|entry| entry.gallery_status != SurfaceGalleryStatus::NotInGallery)
        .map(|entry| entry.name)
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(
        gallery_rows, expected_gallery_rows,
        "gallery row query must derive from contract gallery status"
    );

    let default_rows = default_surface_rows()
        .map(|entry| entry.name)
        .collect::<std::collections::BTreeSet<_>>();
    let expected_default_rows = COMPONENT_CONTRACT_ROWS
        .iter()
        .filter(|entry| entry.default_export)
        .map(|entry| entry.name)
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(
        default_rows, expected_default_rows,
        "default surface query must derive from contract default export intent"
    );

    let recipes = component_recipe_component_rows()
        .map(|entry| entry.name)
        .collect::<std::collections::BTreeSet<_>>();
    let expected_recipes = COMPONENT_CONTRACT_ROWS
        .iter()
        .filter(|entry| entry.owner == PublicSurfaceOwnerClass::OfficialComponentRecipe)
        .map(|entry| entry.name)
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(
        recipes, expected_recipes,
        "component recipe query must derive from contract owner class"
    );

    for overlay in &overlays {
        assert!(
            inventory.contains(overlay),
            "official overlay `{overlay}` should have an API inventory row"
        );
    }
    for recipe in &recipes {
        assert!(
            inventory.contains(recipe),
            "component recipe `{recipe}` should have an API inventory row"
        );
    }
}

#[test]
fn component_recipe_inventory_rows_are_classified_once() {
    use std::collections::BTreeSet;

    let recipe_names = component_recipe_component_rows()
        .map(|entry| entry.name)
        .collect::<BTreeSet<_>>();

    for recipe in &recipe_names {
        assert!(
            COMPONENT_API_INVENTORY
                .iter()
                .any(|entry| entry.component == *recipe),
            "component recipe `{recipe}` needs a public API inventory row"
        );
        assert_eq!(
            public_owner_for_component_inventory(recipe),
            PublicSurfaceOwnerClass::OfficialComponentRecipe,
            "component recipe `{recipe}` must use the contract recipe owner"
        );
        assert_eq!(
            component_contract_entry(recipe)
                .expect("recipe should have a contract row")
                .gallery_status,
            SurfaceGalleryStatus::NotInGallery,
            "component recipe `{recipe}` should not become a standalone official gallery row"
        );
    }

    for entry in COMPONENT_API_INVENTORY {
        assert_eq!(
            public_owner_for_component_inventory(entry.component)
                == PublicSurfaceOwnerClass::OfficialComponentRecipe,
            recipe_names.contains(entry.component),
            "{} owner classification drifted from component_recipe_component_rows()",
            entry.component
        );
    }
}

#[test]
fn component_api_inventory_rows_are_unique_and_classified() {
    let mut seen = std::collections::BTreeSet::new();
    for entry in COMPONENT_API_INVENTORY {
        assert!(
            seen.insert(entry.component),
            "component API inventory contains duplicate row for `{}`",
            entry.component
        );
        assert!(
            entry.has_classification(),
            "{} must document at least one API ownership bucket or no-interaction note",
            entry.component
        );
        assert!(
            entry.renderer_neutral_state,
            "{} resolved state must remain renderer-neutral",
            entry.component
        );
    }
}

#[test]
fn component_api_inventory_tracks_public_method_surface() {
    for entry in COMPONENT_API_INVENTORY {
        let source_methods = component_public_methods_from_source(entry.component);
        let expected_methods = component_public_methods(entry.component)
            .iter()
            .map(|method| method.to_string())
            .collect::<Vec<_>>();
        assert_eq!(
            source_methods, expected_methods,
            "{} public method surface drifted; update COMPONENT_API_INVENTORY and the method baseline together",
            entry.component
        );
    }
}

#[test]
fn component_api_inventory_uses_stable_ownership_vocabulary() {
    const CURRENT_CALLBACK_NAMES: &[&str] = &[
        "on_activate",
        "on_action",
        "on_cancel",
        "on_change",
        "on_click",
        "on_cell_edit_change",
        "on_column_order_change",
        "on_column_sizing_change",
        "on_dismiss",
        "on_move",
        "on_open_change",
        "on_query_change",
        "on_remove",
        "on_row_activate",
        "on_row_selection_change",
        "on_row_expansion_request",
        "on_select",
        "on_selected_values_change",
        "on_selection_change",
        "on_scroll_viewport_changed",
        "on_sort_requested",
        "on_scroll_viewport_changed",
        "on_toggle",
    ];
    for entry in COMPONENT_API_INVENTORY {
        for seed in entry.default_seeds {
            assert!(
                seed.builder.starts_with("default_"),
                "{} default seed `{}` must use default_* naming",
                entry.component,
                seed.builder
            );
            assert!(
                !seed.runtime_value.is_empty(),
                "{} default seed `{}` must name the adapter-owned runtime value it seeds",
                entry.component,
                seed.builder
            );
        }

        for callback in entry.callbacks {
            assert!(
                CURRENT_CALLBACK_NAMES.contains(&callback.name),
                "{} callback `{}` is not part of the current inventory vocabulary",
                entry.component,
                callback.name
            );
            assert!(
                !callback.payload.is_empty(),
                "{} callback `{}` must document its payload type",
                entry.component,
                callback.name
            );
        }
    }
}

#[test]
fn component_api_inventory_keeps_regression_sentinels_for_stateful_components() {
    assert_inventory_contains_callback("Link", "on_activate", "(LinkActivation, Activation)");
    assert_inventory_contains_callback("Tag", "on_remove", "(TagRemove, Activation)");
    assert_inventory_contains_callback("ToastStack", "on_action", "(ToastAction, Activation)");
    assert_inventory_contains_callback("ToastStack", "on_dismiss", "(ToastDismiss, Activation)");
    assert_inventory_contains_controlled_input("TextInput", "value");
    assert_inventory_contains_callback("TextInput", "on_change", "String");
    assert_inventory_contains_controlled_input("Textarea", "value");
    assert_inventory_contains_callback("Textarea", "on_change", "String");
    assert_inventory_contains_controlled_input("Switch", "checked");
    assert_inventory_contains_callback("Switch", "on_change", "bool");
    assert_inventory_contains_default_seed("Popover", "default_open", "open");
    assert_inventory_contains_callback("Popover", "on_open_change", "OverlayOpenIntent");
    assert_inventory_contains_controlled_input("Select", "selected");
    assert_inventory_contains_controlled_input("Select", "active");
    assert_inventory_contains_callback("Select", "on_select", "SelectSelection");
    assert_inventory_contains_default_seed("Combobox", "default_query", "query");
    assert_inventory_contains_controlled_input("Command", "query");
    assert_inventory_contains_controlled_input("Command", "selected_values");
    assert_inventory_contains_controlled_input("Command", "index_snapshot");
    assert_inventory_contains_default_seed("Command", "default_query", "query");
    assert_inventory_contains_callback("Command", "on_query_change", "String");
    assert_inventory_contains_callback(
        "Command",
        "on_selected_values_change",
        "CommandSelectionChange",
    );
    assert_inventory_contains_default_seed("Tabs", "default_selected", "selected");
    assert_inventory_contains_callback(
        "ScrollArea",
        "on_scroll_viewport_changed",
        "ScrollViewportChangedEvent",
    );
    assert_inventory_contains_default_seed("RadioGroup", "default_selected", "selected");
    assert_inventory_contains_controlled_input("RadioGroup", "selected");
    assert_inventory_contains_default_seed("Toolbar", "default_focused", "focused");
    assert_inventory_contains_default_seed("Sidebar", "default_focused", "focused");
    assert_inventory_contains_default_seed("Tree", "default_selected", "selected");
    assert_inventory_contains_default_seed("Tree", "default_focused", "focused");
    assert_inventory_contains_callback("Tree", "on_toggle", "TreeToggle");
    assert_inventory_contains_callback("Tree", "on_move", "TreeMove");
    assert_inventory_contains_default_seed("VirtualizedList", "default_active_key", "active_key");
    assert_inventory_contains_default_seed(
        "VirtualizedList",
        "default_selected_key",
        "selected_keys",
    );
    assert_inventory_contains_default_seed(
        "VirtualizedList",
        "default_selected_keys",
        "selected_keys",
    );
    assert_inventory_contains_controlled_input("VirtualizedList", "reveal_key");
    assert_inventory_contains_policy_hint("VirtualizedList", "from_shared_items");
    assert_inventory_contains_policy_hint("VirtualizedList", "from_data_source");
    assert_inventory_contains_policy_hint("VirtualizedList", "scroll_handle");
    assert_inventory_contains_policy_hint("VirtualizedList", "render_row");
    assert_inventory_contains_callback(
        "VirtualizedList",
        "on_activate",
        "VirtualizedListActivation",
    );
    assert_inventory_contains_callback(
        "VirtualizedList",
        "on_selection_change",
        "VirtualizedListSelectionChange",
    );
    assert_inventory_contains_default_seed("Menu", "default_focused_value", "focused_value");
    assert_inventory_contains_default_seed("ContextMenu", "default_focused_value", "focused_value");
    assert_inventory_contains_default_seed("Table", "default_focused_row", "focused_row");
    assert_inventory_contains_controlled_input("TableGlobalFilter", "query");
    assert_inventory_contains_default_seed("TableGlobalFilter", "default_query", "query");
    assert_inventory_contains_callback("TableGlobalFilter", "on_change", "TableGlobalFilterChange");
    assert_inventory_contains_controlled_input("TablePredicateFilter", "operator");
    assert_inventory_contains_controlled_input("TablePredicateFilter", "value");
    assert_inventory_contains_default_seed("TablePredicateFilter", "default_operator", "operator");
    assert_inventory_contains_default_seed("TablePredicateFilter", "default_value", "value");
    assert_inventory_contains_callback(
        "TablePredicateFilter",
        "on_change",
        "TablePredicateFilterChange",
    );
    assert_inventory_contains_controlled_input("TableColumnVisibility", "visibility");
    assert_inventory_contains_controlled_input("TableColumnVisibility", "open");
    assert_inventory_contains_default_seed(
        "TableColumnVisibility",
        "default_visibility",
        "visibility",
    );
    assert_inventory_contains_default_seed("TableColumnVisibility", "default_open", "open");
    assert_inventory_contains_callback(
        "TableColumnVisibility",
        "on_change",
        "TableColumnVisibilityChange",
    );
    assert_inventory_contains_callback("Table", "on_row_activate", "TableRowActivation");
    assert_inventory_contains_callback(
        "Table",
        "on_row_selection_change",
        "TableRowSelectionChange",
    );
    assert_inventory_contains_callback(
        "Table",
        "on_row_expansion_request",
        "TableRowExpansionToggle",
    );
    assert_inventory_contains_callback("Table", "on_column_order_change", "TableColumnOrderChange");
}
