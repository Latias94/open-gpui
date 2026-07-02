#[test]
fn component_contract_docs_match_current_public_surface_vocabulary() {
    let contract = include_str!("../../../../docs/ui/component-contract.md");

    for required in [
        "components::catalog::COMPONENT_CATALOG",
        "open_gpui_ui_components::component_contract",
        "component_contract/rows.rs",
        "component_contract/projections.rs",
        "component_contract/api_inventory.rs",
        "components/catalog.rs",
        "components/render.rs",
        "components/samples/",
        "components/runtime/",
        "components/render/",
        "`adapter-only`",
        "`internal-anatomy`",
        "`state-contract`",
        "`TableBehaviorSnapshot`",
        "not component facades",
        "default application",
        "state API",
        "`ThemeRegistry` is the app-level owner",
        "Virtualized adapters share a crate-private row-window projection",
        "`open_gpui_ui_components::choice`",
        "`open_gpui_ui_components::gpui_adapter`",
        "public_api/default.rs",
        "root_and_prelude_exports_match_contract_default_surface_intent",
        "command_component_source_mapping_tracks_split_owners",
        "Menu`, `ContextMenu`, and `Tree` follow that shape",
        "`menu/runtime.rs` owns submenu hover timing",
        "`table/behavior/mod.rs`",
        "`table/behavior/counts.rs`",
        "`table/behavior/columns.rs`",
        "`table/behavior/header.rs`",
        "`table/behavior/rows.rs`",
        "`table/behavior/tree.rs`",
        "components_catalog_consumes_component_contract_rows",
        "must not use wildcard facade exports such as",
        "pub use runtime::*",
        "pub use samples::*",
    ] {
        assert!(
            contract.contains(required),
            "component contract docs should mention `{required}`"
        );
    }

    for removed in [
        "`ui_components::primitives::active_descendant`",
        "`ui_components::primitives::collection`",
        "`ui_components::primitives::controllable_state`",
        "`ui_components::primitives::overlay`",
        "theme registry gap",
        "COMPONENT_CATALOG` owns the current",
        "gallery constructors for official status",
        concat!("`menu_", "runtime.rs` owns submenu hover timing"),
    ] {
        assert!(
            !contract.contains(removed),
            "component contract docs should not preserve removed or stale contract `{removed}`"
        );
    }
}
