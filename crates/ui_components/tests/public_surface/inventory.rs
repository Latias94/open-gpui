use super::*;

#[test]
fn component_api_inventory_covers_official_gallery_catalog() {
    use std::collections::BTreeSet;

    let inventory_names = COMPONENT_API_INVENTORY
        .iter()
        .map(|entry| entry.component.to_string())
        .collect::<BTreeSet<_>>();
    let registry_official_names = COMPONENT_API_INVENTORY
        .iter()
        .filter(|entry| {
            matches!(
                component_contract_gallery_status(entry.component),
                SurfaceGalleryStatus::OfficialComponent | SurfaceGalleryStatus::OfficialOverlay
            )
        })
        .map(|entry| entry.component.to_string())
        .collect::<BTreeSet<_>>();

    let missing = registry_official_names
        .difference(&inventory_names)
        .cloned()
        .collect::<Vec<_>>();
    assert!(
        missing.is_empty(),
        "registry official component entries need public API inventory rows: {missing:?}"
    );

    for overlay in OFFICIAL_OVERLAY_COMPONENTS {
        assert!(
            inventory_names.contains(*overlay),
            "overlay component `{overlay}` needs a public API inventory row"
        );
    }
}

#[test]
fn component_contract_registry_covers_inventory_and_adjacent_surfaces() {
    use std::collections::BTreeSet;

    let registry_names = COMPONENT_CONTRACT_REGISTRY
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
        .difference(&registry_names)
        .copied()
        .collect::<Vec<_>>();
    assert_eq!(
        missing_inventory,
        Vec::<&str>::new(),
        "component inventory rows must have canonical contract registry metadata"
    );

    let missing_adjacent = adjacent_names
        .difference(&registry_names)
        .copied()
        .collect::<Vec<_>>();
    assert_eq!(
        missing_adjacent,
        Vec::<&str>::new(),
        "adjacent public-surface rows must have canonical contract registry metadata"
    );
}

#[test]
fn component_contract_projection_functions_delegate_to_registry_rows() {
    for entry in COMPONENT_CONTRACT_REGISTRY {
        let projected = component_contract_entry(entry.name)
            .unwrap_or_else(|| panic!("missing canonical registry row for `{}`", entry.name));
        assert_eq!(
            projected, entry,
            "{} projection should return the canonical registry row",
            entry.name
        );
        assert_eq!(component_contract_family(entry.name), entry.family);
        assert_eq!(
            component_contract_gallery_status(entry.name),
            entry.gallery_status
        );
        assert_eq!(
            component_contract_default_export(entry.name),
            entry.default_export
        );
        assert_eq!(
            component_contract_docs_status(entry.name),
            Some(entry.docs_status)
        );
        assert_eq!(component_contract_docs_token(entry.name), entry.docs_token);
        assert_eq!(
            component_contract_source_home(entry.name),
            Some(entry.source_home)
        );
        assert_eq!(component_source_inputs(entry.name), entry.source_inputs);
    }
}

#[test]
fn component_contract_registry_is_split_by_responsibility() {
    let contract_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("src")
        .join("component_contract");
    let expected_owners = [
        "types.rs",
        "rows.rs",
        "projections.rs",
        "surfaces.rs",
        "api_inventory.rs",
        "source_mapping.rs",
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
        "pub fn component_public_methods",
        "pub fn component_source_inputs",
    ] {
        assert!(
            !facade.contains(stale_single_file_fact),
            "component_contract/mod.rs should stay a facade, not own `{stale_single_file_fact}`"
        );
    }

    let rows = read_source_file(&contract_dir.join("rows.rs"));
    assert!(rows.contains("COMPONENT_CONTRACT_REGISTRY"));
    let inventory = read_source_file(&contract_dir.join("api_inventory.rs"));
    assert!(inventory.contains("COMPONENT_API_INVENTORY"));
    assert!(inventory.contains("component_public_methods"));
    let projections = read_source_file(&contract_dir.join("projections.rs"));
    assert!(projections.contains("component_contract_gallery_status"));
    let source_mapping = read_source_file(&contract_dir.join("source_mapping.rs"));
    assert!(source_mapping.contains("component_source_inputs"));
}

#[test]
fn component_contract_registry_aligns_compatibility_lists() {
    let overlays = OFFICIAL_OVERLAY_COMPONENTS
        .iter()
        .copied()
        .collect::<std::collections::BTreeSet<_>>();
    let recipes = COMPONENT_RECIPE_COMPONENTS
        .iter()
        .copied()
        .collect::<std::collections::BTreeSet<_>>();
    let inventory = COMPONENT_API_INVENTORY
        .iter()
        .map(|entry| entry.component)
        .collect::<std::collections::BTreeSet<_>>();

    for entry in COMPONENT_CONTRACT_REGISTRY {
        if entry.gallery_status == SurfaceGalleryStatus::OfficialOverlay {
            assert!(
                overlays.contains(entry.name),
                "official overlay `{}` should stay in OFFICIAL_OVERLAY_COMPONENTS",
                entry.name
            );
        }
        if inventory.contains(entry.name) {
            assert_eq!(
                entry.owner == PublicSurfaceOwnerClass::OfficialComponentRecipe,
                recipes.contains(entry.name),
                "{} recipe ownership drifted from COMPONENT_RECIPE_COMPONENTS",
                entry.name
            );
        }
    }
}

#[test]
fn component_recipe_inventory_rows_are_classified_once() {
    use std::collections::BTreeSet;

    let recipe_names = COMPONENT_RECIPE_COMPONENTS
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();

    for recipe in COMPONENT_RECIPE_COMPONENTS {
        assert!(
            COMPONENT_API_INVENTORY
                .iter()
                .any(|entry| entry.component == *recipe),
            "component recipe `{recipe}` needs a public API inventory row"
        );
        assert_eq!(
            public_owner_for_component_inventory(recipe),
            PublicSurfaceOwnerClass::OfficialComponentRecipe,
            "component recipe `{recipe}` must use the registry recipe owner"
        );
        assert_eq!(
            component_contract_gallery_status(recipe),
            SurfaceGalleryStatus::NotInGallery,
            "component recipe `{recipe}` should not become a standalone official gallery row"
        );
    }

    for entry in COMPONENT_API_INVENTORY {
        assert_eq!(
            public_owner_for_component_inventory(entry.component)
                == PublicSurfaceOwnerClass::OfficialComponentRecipe,
            recipe_names.contains(entry.component),
            "{} owner classification drifted from COMPONENT_RECIPE_COMPONENTS",
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
        "on_close",
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
        "on_sort_requested",
        "on_toggle",
    ];
    const CURRENT_LEGACY_SEED_INPUTS: &[(&str, &str)] = &[];

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

        for legacy_seed in entry.legacy_seed_inputs {
            assert!(
                CURRENT_LEGACY_SEED_INPUTS.contains(&(entry.component, *legacy_seed)),
                "{} legacy seed `{}` needs an explicit migration decision before U2",
                entry.component,
                legacy_seed
            );
        }
    }
}

#[test]
fn component_api_inventory_keeps_regression_sentinels_for_stateful_components() {
    assert_inventory_contains_controlled_input("TextInput", "value");
    assert_inventory_contains_callback("TextInput", "on_change", "String");
    assert_inventory_contains_controlled_input("Textarea", "value");
    assert_inventory_contains_callback("Textarea", "on_change", "String");
    assert_inventory_contains_controlled_input("Switch", "checked");
    assert_inventory_contains_callback("Switch", "on_change", "bool");
    assert_inventory_contains_default_seed("Popover", "default_open", "open");
    assert_inventory_contains_callback("Popover", "on_open_change", "bool");
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
    assert_inventory_contains_default_seed("RadioGroup", "default_selected", "selected");
    assert_inventory_contains_default_seed("Toolbar", "default_focused", "focused");
    assert_inventory_contains_default_seed("Sidebar", "default_focused", "focused");
    assert_inventory_contains_default_seed("Tree", "default_selected", "selected");
    assert_inventory_contains_default_seed("Tree", "default_focused", "focused");
    assert_inventory_contains_callback("Tree", "on_toggle", "TreeToggle");
    assert_inventory_contains_callback("Tree", "on_move", "TreeMove");
    assert_inventory_contains_default_seed(
        "VirtualizedList",
        "default_active_index",
        "active_index",
    );
    assert_inventory_contains_default_seed(
        "VirtualizedList",
        "default_selected_index",
        "selected_index",
    );
    assert_inventory_contains_callback(
        "VirtualizedList",
        "on_activate",
        "VirtualizedListActivation",
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
