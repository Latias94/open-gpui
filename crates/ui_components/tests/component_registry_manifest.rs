use open_gpui_ui_components::component_contract::{
    COMPONENT_CONTRACT_REGISTRY, COMPONENT_SCAFFOLD_RECIPES, ComponentRegistryGalleryStatus,
    ComponentRegistryOwnerClass, PublicSurfaceOwnerClass, ScaffoldRecipeOutputOwnership,
    component_registry_manifest, component_registry_manifest_schema,
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
fn component_registry_manifest_tracks_scaffold_recipes() {
    let manifest = component_registry_manifest();
    assert_eq!(manifest.recipes.len(), COMPONENT_SCAFFOLD_RECIPES.len());

    let recipe_ids = manifest
        .recipes
        .iter()
        .map(|recipe| recipe.id.as_str())
        .collect::<Vec<_>>();
    let mut sorted_recipe_ids = recipe_ids.clone();
    sorted_recipe_ids.sort_unstable();
    assert_eq!(
        recipe_ids, sorted_recipe_ids,
        "scaffold recipe ids should be sorted"
    );

    let unique_recipe_ids = recipe_ids
        .iter()
        .copied()
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(
        recipe_ids.len(),
        unique_recipe_ids.len(),
        "scaffold recipe ids should be unique"
    );

    let registry_names = manifest
        .entries
        .iter()
        .map(|entry| entry.name.as_str())
        .collect::<std::collections::BTreeSet<_>>();

    for recipe in &manifest.recipes {
        assert!(
            !recipe.generated_files.is_empty(),
            "recipe `{}` should declare generated file intent",
            recipe.id
        );
        assert!(
            !recipe.required_imports.is_empty(),
            "recipe `{}` should declare required imports",
            recipe.id
        );
        assert!(
            !recipe.customization_boundary.is_empty(),
            "recipe `{}` should declare customization boundaries",
            recipe.id
        );
        assert!(
            !recipe.verification_gates.is_empty(),
            "recipe `{}` should declare verification gates",
            recipe.id
        );
        for source_component in &recipe.source_components {
            assert!(
                registry_names.contains(source_component.as_str()),
                "recipe `{}` references missing registry row `{source_component}`",
                recipe.id
            );
        }
        if recipe.output_ownership == ScaffoldRecipeOutputOwnership::AppOwnedSource {
            assert!(
                recipe.source_components.len() > 1,
                "app-owned source recipe `{}` should be a composition scaffold, not a single official component copy",
                recipe.id
            );
        }
    }

    let table_recipe = manifest
        .recipes
        .iter()
        .find(|recipe| recipe.id == "table-filters-toolbar")
        .expect("table filters toolbar recipe should exist");
    assert_eq!(
        table_recipe.output_ownership,
        ScaffoldRecipeOutputOwnership::AppOwnedSource
    );
    for expected_component in ["Table", "TableFacetedFilter", "TableToolbar"] {
        assert!(
            table_recipe
                .source_components
                .contains(&expected_component.to_string()),
            "table filters toolbar recipe should reference `{expected_component}`"
        );
    }
}

#[test]
fn official_component_rows_remain_cargo_owned_implementations() {
    let manifest = component_registry_manifest();
    let official_manifest_entries = manifest
        .entries
        .iter()
        .filter(|entry| entry.owner == ComponentRegistryOwnerClass::OfficialComponent)
        .map(|entry| entry.name.as_str())
        .collect::<std::collections::BTreeSet<_>>();
    let official_registry_entries = COMPONENT_CONTRACT_REGISTRY
        .iter()
        .filter(|entry| entry.owner == PublicSurfaceOwnerClass::OfficialComponent)
        .map(|entry| entry.name)
        .collect::<std::collections::BTreeSet<_>>();

    assert_eq!(
        official_manifest_entries, official_registry_entries,
        "scaffold recipes must not reclassify official Cargo-owned component implementations"
    );
}

#[test]
fn component_registry_manifest_schema_covers_registry_and_recipe_vocabulary() {
    let schema = component_registry_manifest_schema();
    let source = serde_json::to_string_pretty(&schema).expect("schema should serialize");

    for marker in [
        "schema_version",
        "package",
        "entries",
        "recipes",
        "distribution_authority",
        "official_component",
        "official_component_recipe",
        "renderer_neutral_state_contract",
        "gpui_adapter_helper",
        "deprecated_removal_target",
        "source_components",
        "verification_gates",
        "app_owned_source",
        "cargo_dependency_snippet",
        "gallery_story_sample",
    ] {
        assert!(
            source.contains(marker),
            "registry schema should include marker `{marker}`"
        );
    }
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
