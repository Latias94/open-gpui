//! Scaffold recipe metadata for app-owned component compositions.

/// App-owned scaffold recipes published beside the component registry manifest.
pub const COMPONENT_SCAFFOLD_RECIPES: &[ScaffoldRecipe] = &[
    ScaffoldRecipe {
        id: "table-filters-toolbar",
        title: "Table filter toolbar",
        family: "data",
        source_components: &[
            "Table",
            "TableToolbar",
            "TableGlobalFilter",
            "TableFacetedFilter",
            "TableRangeFilter",
            "TablePredicateFilter",
            "TableColumnVisibility",
        ],
        generated_files: &[ScaffoldRecipeGeneratedFile {
            path_hint: "src/ui/table_filters_toolbar.rs",
            intent: "compose a caller-owned table toolbar with search, facets, range filters, predicate filters, and sparse column visibility state",
        }],
        required_imports: &[
            "open_gpui_ui_components::{Table, TableToolbar, TableGlobalFilter, TableFacetedFilter, TableRangeFilter, TablePredicateFilter, TableColumnVisibility}",
        ],
        customization_boundary: "applications own row data, filter predicates, persistence, URL sync, saved views, and async fetching; the component crate owns renderer-neutral filter payloads and controlled callbacks",
        verification_gates: &[
            "cargo nextest run -p open-gpui-ui-components table --no-fail-fast",
            "cargo nextest run -p open-gpui-ui-foundation-gallery table --no-fail-fast",
            "cargo run -p xtask -- scan-ui-registry",
        ],
        output_ownership: ScaffoldRecipeOutputOwnership::AppOwnedSource,
    },
    ScaffoldRecipe {
        id: "field-control-composition",
        title: "Field control composition",
        family: "form",
        source_components: &["Field", "Label", "TextInput", "Textarea"],
        generated_files: &[ScaffoldRecipeGeneratedFile {
            path_hint: "src/ui/form_field.rs",
            intent: "wrap a controlled text input or textarea in shared label, description, message, required, and invalid field chrome",
        }],
        required_imports: &["open_gpui_ui_components::{Field, Label, TextInput, Textarea}"],
        customization_boundary: "applications own value state, validation engines, completion, rich text, and submit behavior; Field owns accessible description/message wiring",
        verification_gates: &[
            "cargo nextest run -p open-gpui-ui-components form text_input --no-fail-fast",
            "cargo run -p xtask -- scan-ui-registry",
        ],
        output_ownership: ScaffoldRecipeOutputOwnership::AppOwnedSource,
    },
    ScaffoldRecipe {
        id: "themed-surface-wrapper",
        title: "Themed surface wrapper",
        family: "theme",
        source_components: &[
            "ThemeRegistry",
            "ThemeDefinition",
            "ThemeSnapshot",
            "ThemeResolver",
            "theme_definition_from_json_str",
        ],
        generated_files: &[ScaffoldRecipeGeneratedFile {
            path_hint: "src/ui/theme_surface.rs",
            intent: "load a portable theme definition, register it, and pass an explicit snapshot into component color resolution",
        }],
        required_imports: &[
            "open_gpui_ui_components::{ThemeRegistry, ThemeDefinition, ThemeSnapshot, ThemeResolver, theme_definition_from_json_str}",
        ],
        customization_boundary: "applications own theme files, registry lifetime, persistence, and user preference routing; component recipes only consume validated snapshots",
        verification_gates: &[
            "cargo nextest run -p open-gpui-ui-components theme --no-fail-fast",
            "cargo run -p xtask -- scan-theme-schema",
            "cargo run -p xtask -- scan-ui-registry",
        ],
        output_ownership: ScaffoldRecipeOutputOwnership::CargoDependencySnippet,
    },
    ScaffoldRecipe {
        id: "gallery-story-sample",
        title: "Gallery story sample",
        family: "gallery",
        source_components: &["Button", "StatusCue", "ThemeSnapshot"],
        generated_files: &[ScaffoldRecipeGeneratedFile {
            path_hint: "examples/ui-foundation-gallery/src/story_samples/generated_component_story.rs",
            intent: "seed a gallery-owned story sample with stable selectors, visible state readouts, and a compact probe contract",
        }],
        required_imports: &["open_gpui_ui_components::{Button, StatusCue, ThemeSnapshot}"],
        customization_boundary: "the gallery owns selector ids, story probes, and visual sample data; component crates own public state, callbacks, and token vocabulary",
        verification_gates: &[
            "cargo nextest run -p open-gpui-ui-foundation-gallery gallery_story_contracts_cover_components_state_readouts_and_overlays --no-fail-fast",
            "cargo run -p xtask -- scan-ui-registry",
        ],
        output_ownership: ScaffoldRecipeOutputOwnership::GalleryStorySample,
    },
];

/// A scaffold recipe row for app-owned composition starter code.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScaffoldRecipe {
    /// Stable recipe identifier used by registry artifacts and docs.
    pub id: &'static str,
    /// Human-facing recipe title.
    pub title: &'static str,
    /// Owning component family.
    pub family: &'static str,
    /// Registry entries that this recipe composes.
    pub source_components: &'static [&'static str],
    /// Generated file purposes, not a source-copy package contract.
    pub generated_files: &'static [ScaffoldRecipeGeneratedFile],
    /// Imports expected in the generated snippet.
    pub required_imports: &'static [&'static str],
    /// Boundary between generated starter code and application-owned behavior.
    pub customization_boundary: &'static str,
    /// Focused gates that keep the recipe aligned with component contracts.
    pub verification_gates: &'static [&'static str],
    /// Ownership classification for the generated output.
    pub output_ownership: ScaffoldRecipeOutputOwnership,
}

/// Generated file intent for a scaffold recipe.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScaffoldRecipeGeneratedFile {
    /// Suggested path for generated starter code.
    pub path_hint: &'static str,
    /// Purpose of the generated starter file.
    pub intent: &'static str,
}

/// Ownership class for scaffold recipe output.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, schemars::JsonSchema, serde::Serialize, serde::Deserialize,
)]
#[serde(rename_all = "snake_case")]
pub enum ScaffoldRecipeOutputOwnership {
    /// Generated files are application source and may be freely changed.
    AppOwnedSource,
    /// Generated text is a dependency-oriented snippet around official crates.
    CargoDependencySnippet,
    /// Generated text belongs to gallery story samples and probes.
    GalleryStorySample,
}
