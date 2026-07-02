use open_gpui_ui_components::component_contract::{
    ComponentRegistryGalleryStatus, ComponentRegistryOwnerClass, component_registry_manifest,
};

fn manifest_entry(
    name: &str,
) -> open_gpui_ui_components::component_contract::ComponentRegistryEntry {
    component_registry_manifest()
        .entries
        .into_iter()
        .find(|entry| entry.name == name)
        .unwrap_or_else(|| panic!("component registry manifest should include `{name}`"))
}

#[test]
fn component_registry_manifest_is_versioned_sorted_and_unique() {
    let manifest = component_registry_manifest();
    assert_eq!(manifest.schema_version, 1);

    let names = manifest
        .entries
        .iter()
        .map(|entry| entry.name.as_str())
        .collect::<Vec<_>>();
    let mut sorted = names.clone();
    sorted.sort_unstable();
    assert_eq!(names, sorted, "manifest entries should be sorted by name");

    let unique = names
        .iter()
        .copied()
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(
        names.len(),
        unique.len(),
        "manifest entries should have unique names"
    );
}

#[test]
fn component_registry_manifest_tracks_representative_surface_classes() {
    let button = manifest_entry("Button");
    assert_eq!(button.owner, ComponentRegistryOwnerClass::OfficialComponent);
    assert_eq!(button.family.as_deref(), Some("action"));
    assert_eq!(
        button.gallery.status,
        ComponentRegistryGalleryStatus::OfficialComponent
    );
    assert_eq!(button.docs.token.as_deref(), Some("Button"));
    assert_eq!(button.source.home, "button.rs");
    assert!(button.public_export.root);
    assert!(button.public_export.prelude);
    let button_api = button.api.expect("Button should expose API inventory");
    assert!(button_api.render_inputs.contains(&"variant".to_string()));
    assert!(
        button_api
            .callbacks
            .iter()
            .any(|callback| callback.name == "on_click" && callback.payload == "ClickEvent")
    );

    let table_recipe = manifest_entry("TableFacetedFilter");
    assert_eq!(
        table_recipe.owner,
        ComponentRegistryOwnerClass::OfficialComponentRecipe
    );
    assert_eq!(
        table_recipe.gallery.status,
        ComponentRegistryGalleryStatus::NotInGallery
    );

    let theme = manifest_entry("ThemeDefinition");
    assert_eq!(
        theme.owner,
        ComponentRegistryOwnerClass::RendererNeutralStateContract
    );

    let adapter = manifest_entry("TextInputController");
    assert_eq!(
        adapter.owner,
        ComponentRegistryOwnerClass::GpuiAdapterHelper
    );
    assert_eq!(
        adapter.gallery.status,
        ComponentRegistryGalleryStatus::AdapterOnly
    );
    assert!(!adapter.public_export.prelude);

    let anatomy = manifest_entry("ToolbarItem");
    assert_eq!(
        anatomy.owner,
        ComponentRegistryOwnerClass::InternalImplementationDetail
    );

    let removed = manifest_entry("primitives::overlay");
    assert_eq!(
        removed.owner,
        ComponentRegistryOwnerClass::DeprecatedRemovalTarget
    );
    assert_eq!(removed.source.home, "removed");
}

#[test]
fn component_registry_manifest_is_renderer_neutral_json() {
    let json = serde_json::to_string_pretty(&component_registry_manifest())
        .expect("component registry manifest should serialize");

    for runtime_token in [
        "Window",
        "App",
        "Context",
        "Element",
        "FocusHandle",
        "ScrollHandle",
    ] {
        let runtime_value = format!("\"{runtime_token}\"");
        assert!(
            !json.contains(&runtime_value),
            "manifest JSON should not expose GPUI runtime token `{runtime_token}`"
        );
    }
}
