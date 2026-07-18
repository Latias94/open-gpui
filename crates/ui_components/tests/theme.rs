mod support;

use open_gpui::{ParentElement, div};
use open_gpui_ui_components::theme::{
    THEME_JSON_SCHEMA_VERSION, ThemeDefinition, ThemeFileField, ThemeLoadError, ThemeRegistry,
    ThemeSelectionError, ThemeValidationError, app_theme_context, install_theme_registry,
    register_theme, register_theme_json_file, register_theme_json_str, set_app_theme,
    theme_definition_from_json_file, theme_definition_from_json_str, theme_json_schema,
    theme_json_string,
};
use open_gpui_ui_components::{
    AlertDialog, AlertDialogIntent, Avatar, Badge, BadgeVariant, Button, ButtonVariant, Checkbox,
    ColorIntent, ColorState, Combobox, ComboboxOption, Command, CommandItem, EmptyState,
    FeedbackIntent, Field, HoverCard, IconButton, Kbd, Label, Listbox, Menu, MenuItem, Progress,
    RadioGroup, RadioItem, Select, Separator, Sheet, Skeleton, StatusCue, Switch, TableToolbar,
    TextInput, ThemeColor, ThemeContext, ThemeMode, ThemeResolver, ThemeSnapshot, Toggle,
    ToggleVariant, listbox::ListboxOption,
};
use open_gpui_ui_core::{
    Sizable, ThemeDesignScales, ThemeElevationLayer, ThemeElevationScale, semantic,
};

use support::tokens::custom_tokens;

const OLD_COLOR_ONLY_THEME_JSON: &str = r##"{
  "schema_version": 1,
  "id": "legacy-color-only",
  "label": "Legacy color only",
  "mode": "dark",
  "revision": 1,
  "fallback_mode": "light",
  "colors": [
    { "token": "semantic.accent", "state": "default", "rgb": "#227755" }
  ]
}"##;

fn portable_theme_definition(id: &str) -> ThemeDefinition {
    let source = ThemeSnapshot::dark();
    let colors = source.colors().iter().copied().map(|color| {
        if color.token() == semantic::ACCENT && color.state() == ColorState::Default {
            ThemeColor::new(semantic::ACCENT, ColorState::Default, 0x227755)
        } else if color.token() == semantic::ACCENT && color.state() == ColorState::Hover {
            ThemeColor::new(semantic::ACCENT, ColorState::Hover, 0x1b6044)
        } else {
            color
        }
    });
    ThemeDefinition::new(id, "Forest JSON", ThemeMode::Dark, 9001)
        .design_scales(ThemeDesignScales::default())
        .colors(colors)
}

fn valid_theme_json() -> String {
    let mut registry = ThemeRegistry::new();
    let entry = registry
        .register(portable_theme_definition("forest-json"))
        .expect("complete portable theme should register");
    theme_json_string(entry).expect("complete portable theme should serialize")
}

fn complete_theme_with_color(
    id: &str,
    label: &str,
    mode: ThemeMode,
    source_revision: u64,
    token: open_gpui_ui_core::TokenKey,
    state: ColorState,
    rgb: u32,
) -> ThemeDefinition {
    let source = match mode {
        ThemeMode::Light => ThemeSnapshot::light(),
        ThemeMode::Dark => ThemeSnapshot::dark(),
        ThemeMode::HighContrast => ThemeSnapshot::high_contrast(),
    };
    let colors = source.colors().iter().copied().map(|color| {
        if color.token() == token && color.state() == state {
            ThemeColor::new(token, state, rgb)
        } else {
            color
        }
    });
    ThemeDefinition::new(id, label, mode, source_revision)
        .design_scales(source.design_scales())
        .colors(colors)
}

#[test]
fn button_accepts_custom_token_bundle() {
    let tokens = custom_tokens();
    let state = Button::new("outline", "Outline")
        .variant(ButtonVariant::Outline)
        .tokens(tokens)
        .state();

    assert_eq!(state.colors().border().token(), tokens.border);
    assert_eq!(state.colors().focus_ring().token(), tokens.focus_ring);
    assert_eq!(state.focus_ring().color().token(), tokens.focus_ring);
}

#[test]
fn theme_resolver_keeps_token_intent_and_resolves_fallback_color() {
    let tokens = custom_tokens();
    let state = Button::new("default", "Default").tokens(tokens).state();
    let background = state.colors().background();

    assert_eq!(background.token(), tokens.accent);
    assert_eq!(background.state(), ColorState::Default);
    assert_eq!(background.fallback_rgb(), 0x1f7a66);
    assert_eq!(
        u32::from(ThemeContext::light().resolve(background)),
        0x1f7a66ff
    );
    assert_eq!(
        u32::from(ThemeResolver::resolve_with(
            background,
            &ThemeSnapshot::dark()
        )),
        0x1f7a66ff
    );
}

#[test]
fn theme_resolver_prefers_runtime_theme_table_for_known_tokens() {
    let state = Button::new("default", "Default").state();
    let background = state.colors().background();
    let mut registry = ThemeRegistry::new();
    let snapshot = registry
        .register(complete_theme_with_color(
            "custom-resolver",
            "Custom resolver",
            ThemeMode::Light,
            42,
            semantic::ACCENT,
            ColorState::Default,
            0x123456,
        ))
        .expect("complete custom theme should register")
        .snapshot()
        .clone();

    assert_eq!(background.fallback_rgb(), 0x1f7a66);
    assert_eq!(
        u32::from(ThemeResolver::resolve_with(background, &snapshot)),
        0x123456ff
    );
    assert_eq!(snapshot.mode(), ThemeMode::Light);
    assert_eq!(snapshot.source_revision(), 42);
}

#[test]
fn theme_context_wraps_snapshot_for_render_resolution() {
    let context = ThemeContext::new(ThemeSnapshot::dark());
    let cloned = context.clone();

    assert_eq!(context.mode(), ThemeMode::Dark);
    assert_eq!(
        context.source_revision(),
        ThemeSnapshot::dark().source_revision()
    );
    assert!(context.effective_revision() > 0);
    assert_eq!(
        u32::from(cloned.resolve(ColorIntent::new(semantic::SURFACE, 0xffffff))),
        0x121417ff
    );
}

#[open_gpui::test]
fn installed_registry_controls_app_fallback(cx: &mut open_gpui::TestAppContext) {
    let mut registry = ThemeRegistry::with_builtins();
    registry
        .register(complete_theme_with_color(
            "brand",
            "Brand",
            ThemeMode::Dark,
            77,
            semantic::ACCENT,
            ColorState::Default,
            0x445566,
        ))
        .expect("brand theme should register");
    cx.update(|app| {
        install_theme_registry(app, registry, "brand").expect("brand theme id should be available");
        let resolver = app_theme_context(app);
        assert_eq!(resolver.mode(), ThemeMode::Dark);
        assert_eq!(resolver.source_revision(), 77);
        assert!(resolver.effective_revision() > 0);
        assert_eq!(
            u32::from(resolver.resolve(ColorIntent::new(semantic::ACCENT, 0x1f7a66))),
            0x445566ff
        );
    });
}

#[open_gpui::test]
fn registry_install_rejects_unknown_app_selection_atomically(cx: &mut open_gpui::TestAppContext) {
    cx.update(|app| {
        let before = app_theme_context(app);
        assert_eq!(
            install_theme_registry(app, ThemeRegistry::with_builtins(), "missing").unwrap_err(),
            ThemeSelectionError::UnknownThemeId("missing".to_owned())
        );
        assert_eq!(app_theme_context(app), before);
    });
}

#[open_gpui::test]
fn app_effective_revision_is_monotonic_for_selection_and_content_not_metadata(
    cx: &mut open_gpui::TestAppContext,
) {
    let mut registry = ThemeRegistry::new();
    registry
        .register(complete_theme_with_color(
            "brand-a",
            "Brand A",
            ThemeMode::Light,
            1,
            semantic::ACCENT,
            ColorState::Default,
            0x123456,
        ))
        .expect("brand A should register");
    registry
        .register(complete_theme_with_color(
            "brand-b",
            "Brand B",
            ThemeMode::Light,
            1,
            semantic::ACCENT,
            ColorState::Default,
            0x123456,
        ))
        .expect("brand B should register");

    cx.update(|app| {
        install_theme_registry(app, registry, "brand-a").expect("brand A should install");
        let brand_a = app_theme_context(app);

        set_app_theme(app, "brand-b").expect("brand B should select");
        let brand_b = app_theme_context(app);
        assert!(brand_b.effective_revision() > brand_a.effective_revision());

        set_app_theme(app, "brand-b").expect("brand B no-op should succeed");
        assert_eq!(app_theme_context(app), brand_b);

        register_theme(
            app,
            complete_theme_with_color(
                "brand-b",
                "Brand B metadata",
                ThemeMode::Light,
                2,
                semantic::ACCENT,
                ColorState::Default,
                0x123456,
            ),
        )
        .expect("metadata-only reload should register");
        let metadata_only = app_theme_context(app);
        assert_eq!(
            metadata_only.effective_revision(),
            brand_b.effective_revision()
        );
        assert_eq!(metadata_only.source_revision(), 2);

        register_theme(
            app,
            complete_theme_with_color(
                "brand-b",
                "Brand B content",
                ThemeMode::Light,
                2,
                semantic::ACCENT,
                ColorState::Default,
                0x654321,
            ),
        )
        .expect("effective reload should register");
        let changed = app_theme_context(app);
        assert!(changed.effective_revision() > metadata_only.effective_revision());
        assert_eq!(changed.source_revision(), 2);
    });
}

#[test]
fn default_theme_snapshots_expose_distinct_modes_and_revisions() {
    let light = ThemeSnapshot::light();
    let dark = ThemeSnapshot::dark();
    let high_contrast = ThemeSnapshot::high_contrast();

    assert_eq!(light.mode().as_str(), "light");
    assert_eq!(dark.mode().as_str(), "dark");
    assert_eq!(high_contrast.mode().as_str(), "high-contrast");
    assert!(light.source_revision() < dark.source_revision());
    assert!(dark.source_revision() < high_contrast.source_revision());
    for snapshot in [&light, &dark, &high_contrast] {
        assert_eq!(snapshot.design_scales(), ThemeDesignScales::default());
    }
    assert_ne!(
        light.color_rgb(semantic::SURFACE, ColorState::Default),
        dark.color_rgb(semantic::SURFACE, ColorState::Default)
    );
    assert_ne!(
        dark.color_rgb(semantic::FOCUS_RING, ColorState::FocusVisible),
        high_contrast.color_rgb(semantic::FOCUS_RING, ColorState::FocusVisible)
    );
}

#[test]
fn theme_registry_preloads_builtin_snapshots_without_global_theme_state() {
    let registry = ThemeRegistry::with_builtins();

    assert_eq!(
        registry
            .entries()
            .iter()
            .map(|entry| (
                entry.id(),
                entry.snapshot().mode(),
                entry.snapshot().source_revision()
            ))
            .collect::<Vec<_>>(),
        vec![
            (
                "light",
                ThemeMode::Light,
                ThemeSnapshot::light().source_revision()
            ),
            (
                "dark",
                ThemeMode::Dark,
                ThemeSnapshot::dark().source_revision()
            ),
            (
                "high-contrast",
                ThemeMode::HighContrast,
                ThemeSnapshot::high_contrast().source_revision()
            ),
        ]
    );
    assert_eq!(
        registry
            .snapshot("dark")
            .and_then(|snapshot| snapshot.color_rgb(semantic::SURFACE, ColorState::Default)),
        ThemeSnapshot::dark().color_rgb(semantic::SURFACE, ColorState::Default)
    );
}

#[test]
fn built_in_complete_themes_round_trip_through_the_v1_schema() {
    let source = ThemeRegistry::with_builtins();
    let serialized = source
        .entries()
        .iter()
        .map(|entry| {
            (
                entry.id().to_owned(),
                theme_json_string(entry).expect("built-in theme should serialize"),
            )
        })
        .collect::<Vec<_>>();
    let mut round_tripped = ThemeRegistry::new();

    for (id, json) in serialized {
        register_theme_json_str(&mut round_tripped, &json)
            .expect("serialized built-in theme should register");
        let expected = source.snapshot(&id).expect("source built-in should exist");
        let actual = round_tripped
            .snapshot(&id)
            .expect("round-tripped built-in should exist");
        assert_eq!(actual.mode(), expected.mode());
        assert_eq!(actual.source_revision(), expected.source_revision());
        assert_eq!(actual.colors(), expected.colors());
        assert_eq!(actual.design_scales(), expected.design_scales());
    }
}

#[test]
fn theme_registry_requires_and_retains_complete_v1_content() {
    let mut registry = ThemeRegistry::with_builtins();
    let entry = registry
        .register(complete_theme_with_color(
            "forest",
            "Forest",
            ThemeMode::Dark,
            9001,
            semantic::ACCENT,
            ColorState::Default,
            0x227755,
        ))
        .expect("valid user theme definition should register");
    let snapshot = entry.snapshot();

    assert_eq!(entry.id(), "forest");
    assert_eq!(entry.label(), "Forest");
    assert_eq!(snapshot.mode(), ThemeMode::Dark);
    assert_eq!(snapshot.source_revision(), 9001);
    assert!(entry.effective_revision() > 0);
    assert_eq!(snapshot.design_scales(), ThemeDesignScales::default());
    assert_eq!(
        snapshot.colors().len(),
        ThemeSnapshot::dark().colors().len()
    );
    assert_eq!(
        snapshot.color_rgb(semantic::ACCENT, ColorState::Default),
        Some(0x227755)
    );
    assert_eq!(
        snapshot.color_rgb(semantic::SURFACE, ColorState::Default),
        ThemeSnapshot::dark().color_rgb(semantic::SURFACE, ColorState::Default)
    );
    assert_eq!(
        u32::from(ThemeResolver::resolve_with(
            ColorIntent::new(semantic::ACCENT, 0x1f7a66),
            snapshot
        )),
        0x227755ff
    );

    let before_invalid_replacement = registry.clone();
    assert!(matches!(
        registry.register(
            ThemeDefinition::new("forest", "Invalid replacement", ThemeMode::Dark, 1)
                .design_scales(ThemeDesignScales::default())
                .color(ThemeColor::new(
                    semantic::ACCENT,
                    ColorState::Default,
                    0x123456,
                ))
        ),
        Err(ThemeValidationError::MissingColor { .. })
    ));
    assert_eq!(registry, before_invalid_replacement);
}

#[test]
fn theme_json_loader_registers_portable_definition_like_code_built_theme() {
    let valid_theme_json = valid_theme_json();
    let json_definition = theme_definition_from_json_str(&valid_theme_json)
        .expect("valid theme JSON should produce a ThemeDefinition");
    let mut json_registry = ThemeRegistry::with_builtins();
    let json_entry = json_registry
        .register(json_definition)
        .expect("valid loaded theme should register")
        .clone();

    let mut code_registry = ThemeRegistry::with_builtins();
    let code_entry = code_registry
        .register(portable_theme_definition("forest-json"))
        .expect("equivalent code-built theme should register")
        .clone();

    assert_eq!(json_entry.id(), "forest-json");
    assert_eq!(json_entry.label(), "Forest JSON");
    assert_eq!(json_entry.snapshot().mode(), ThemeMode::Dark);
    assert_eq!(json_entry.snapshot().source_revision(), 9001);
    assert_eq!(
        json_entry.snapshot().design_scales(),
        ThemeDesignScales::default()
    );
    assert_eq!(
        json_entry
            .snapshot()
            .color_rgb(semantic::ACCENT, ColorState::Default),
        code_entry
            .snapshot()
            .color_rgb(semantic::ACCENT, ColorState::Default)
    );
    assert_eq!(
        json_entry
            .snapshot()
            .color_rgb(semantic::SURFACE_MUTED, ColorState::Disabled),
        ThemeSnapshot::dark().color_rgb(semantic::SURFACE_MUTED, ColorState::Disabled)
    );
    assert_eq!(
        json_entry
            .snapshot()
            .color_rgb(semantic::DESTRUCTIVE, ColorState::Invalid),
        ThemeSnapshot::dark().color_rgb(semantic::DESTRUCTIVE, ColorState::Invalid)
    );

    let mut direct_registry = ThemeRegistry::with_builtins();
    let direct_entry = register_theme_json_str(&mut direct_registry, &valid_theme_json)
        .expect("string facade should parse and register")
        .clone();
    assert_eq!(direct_entry.id(), "forest-json");
}

#[test]
fn theme_json_schema_exposes_portable_theme_contract() {
    let schema =
        serde_json::to_string(&theme_json_schema()).expect("theme JSON schema should serialize");

    assert_eq!(THEME_JSON_SCHEMA_VERSION, 1);
    for token in [
        "schema_version",
        "colors",
        "design",
        "typography",
        "control_text",
        "control_line_height",
        "spacing",
        "control_inline",
        "control_block",
        "radius",
        "elevation",
        "density",
        "motion_policy",
        "compact",
        "reduced",
        "semantic.focus_ring",
        "focus-visible",
        "disabled",
        "hover",
        "selected",
        "invalid",
        "high-contrast",
    ] {
        assert!(
            schema.contains(token),
            "theme JSON schema should mention `{token}`"
        );
    }
    assert!(!schema.contains("fallback_mode"));
}

#[test]
fn theme_json_loader_reports_structured_errors_before_registration() {
    assert!(matches!(
        theme_definition_from_json_str(OLD_COLOR_ONLY_THEME_JSON),
        Err(ThemeLoadError::InvalidJson { .. })
    ));
    assert_eq!(
        theme_definition_from_json_str("{}").unwrap_err(),
        ThemeLoadError::MissingField(ThemeFileField::SchemaVersion)
    );
    assert_eq!(
        theme_definition_from_json_str(
            r##"{
                "schema_version": 2,
                "id": "forest",
                "label": "Forest",
                "mode": "dark",
                "revision": 1,
                "colors": [{"token": "semantic.accent", "state": "default", "rgb": "#123456"}]
            }"##
        )
        .unwrap_err(),
        ThemeLoadError::UnsupportedSchemaVersion {
            version: 2,
            supported: THEME_JSON_SCHEMA_VERSION
        }
    );
    assert_eq!(
        theme_definition_from_json_str(
            r##"{
                "schema_version": 1,
                "label": "Forest",
                "mode": "dark",
                "revision": 1,
                "colors": [{"token": "semantic.accent", "state": "default", "rgb": "#123456"}]
            }"##
        )
        .unwrap_err(),
        ThemeLoadError::MissingField(ThemeFileField::Id)
    );
    assert_eq!(
        theme_definition_from_json_str(
            r##"{
                "schema_version": 1,
                "id": "forest",
                "label": "Forest",
                "mode": "dark",
                "revision": 1,
                "colors": [{"token": "semantic.unknown", "state": "default", "rgb": "#123456"}]
            }"##
        )
        .unwrap_err(),
        ThemeLoadError::UnsupportedToken {
            token: "semantic.unknown".to_owned()
        }
    );
    assert_eq!(
        theme_definition_from_json_str(
            r##"{
                "schema_version": 1,
                "id": "forest",
                "label": "Forest",
                "mode": "dark",
                "revision": 1,
                "colors": [{"token": "semantic.accent", "state": "pressed", "rgb": "#123456"}]
            }"##
        )
        .unwrap_err(),
        ThemeLoadError::UnsupportedColorState {
            state: "pressed".to_owned()
        }
    );
    assert_eq!(
        theme_definition_from_json_str(
            r##"{
                "schema_version": 1,
                "id": "forest",
                "label": "Forest",
                "mode": "dark",
                "revision": 1,
                "colors": [{"token": "semantic.accent", "state": "default", "rgb": "#12"}]
            }"##
        )
        .unwrap_err(),
        ThemeLoadError::InvalidRgb {
            value: "#12".to_owned()
        }
    );
    assert_eq!(
        theme_definition_from_json_str(
            r##"{
                "schema_version": 1,
                "id": "forest",
                "label": "Forest",
                "mode": "dark",
                "revision": 1,
                "colors": [
                    {"token": "semantic.accent", "state": "default", "rgb": "#123456"},
                    {"token": "semantic.accent", "state": "default", "rgb": "#654321"}
                ]
            }"##
        )
        .unwrap_err(),
        ThemeLoadError::DuplicateColor {
            token: "semantic.accent".to_owned(),
            state: "default".to_owned()
        }
    );

    let mut missing_design = serde_json::from_str::<serde_json::Value>(&valid_theme_json())
        .expect("complete fixture should parse as JSON");
    missing_design
        .as_object_mut()
        .expect("fixture root should be an object")
        .remove("design");
    assert_eq!(
        theme_definition_from_json_str(&missing_design.to_string()).unwrap_err(),
        ThemeLoadError::MissingField(ThemeFileField::Design)
    );

    let mut unsupported_density = serde_json::from_str::<serde_json::Value>(&valid_theme_json())
        .expect("complete fixture should parse as JSON");
    unsupported_density["design"]["density"] = serde_json::Value::String("dense".to_owned());
    assert_eq!(
        theme_definition_from_json_str(&unsupported_density.to_string()).unwrap_err(),
        ThemeLoadError::UnsupportedDensity {
            density: "dense".to_owned()
        }
    );

    let mut padded_mode = serde_json::from_str::<serde_json::Value>(&valid_theme_json())
        .expect("complete fixture should parse as JSON");
    padded_mode["mode"] = serde_json::Value::String(" dark ".to_owned());
    assert_eq!(
        theme_definition_from_json_str(&padded_mode.to_string()).unwrap_err(),
        ThemeLoadError::UnsupportedMode {
            mode: " dark ".to_owned()
        }
    );

    let mut padded_token = serde_json::from_str::<serde_json::Value>(&valid_theme_json())
        .expect("complete fixture should parse as JSON");
    padded_token["colors"][0]["token"] = serde_json::Value::String(" semantic.surface ".to_owned());
    assert_eq!(
        theme_definition_from_json_str(&padded_token.to_string()).unwrap_err(),
        ThemeLoadError::UnsupportedToken {
            token: " semantic.surface ".to_owned()
        }
    );

    let mut prefixed_rgb = serde_json::from_str::<serde_json::Value>(&valid_theme_json())
        .expect("complete fixture should parse as JSON");
    prefixed_rgb["colors"][0]["rgb"] = serde_json::Value::String("0x123456".to_owned());
    assert_eq!(
        theme_definition_from_json_str(&prefixed_rgb.to_string()).unwrap_err(),
        ThemeLoadError::InvalidRgb {
            value: "0x123456".to_owned()
        }
    );

    let mut missing_elevation_opacity =
        serde_json::from_str::<serde_json::Value>(&valid_theme_json())
            .expect("complete fixture should parse as JSON");
    missing_elevation_opacity["design"]["elevation"]["overlay"][0]
        .as_object_mut()
        .expect("elevation layer should be an object")
        .remove("opacity_percent");
    assert_eq!(
        theme_definition_from_json_str(&missing_elevation_opacity.to_string()).unwrap_err(),
        ThemeLoadError::MissingElevationField {
            index: 0,
            field: ThemeFileField::ElevationOpacityPercent,
        }
    );

    let mut incomplete_colors = serde_json::from_str::<serde_json::Value>(&valid_theme_json())
        .expect("complete fixture should parse as JSON");
    incomplete_colors["colors"]
        .as_array_mut()
        .expect("colors should be an array")
        .pop();
    assert!(matches!(
        theme_definition_from_json_str(&incomplete_colors.to_string()),
        Err(ThemeLoadError::Registration(
            ThemeValidationError::MissingColor { .. }
        ))
    ));
}

#[test]
fn theme_json_file_facade_reads_and_registers_theme_files() {
    let valid_theme_json = valid_theme_json();
    let path = std::env::temp_dir().join(format!(
        "open-gpui-theme-loader-{}-{}.json",
        std::process::id(),
        ThemeSnapshot::light().source_revision()
    ));
    std::fs::write(&path, valid_theme_json).expect("temporary theme file should be writable");

    let definition =
        theme_definition_from_json_file(&path).expect("file facade should load theme definition");
    let mut registry = ThemeRegistry::with_builtins();
    let loaded_entry = registry
        .register(definition)
        .expect("loaded theme definition should register")
        .clone();
    assert_eq!(loaded_entry.id(), "forest-json");

    let mut direct_registry = ThemeRegistry::with_builtins();
    let direct_entry = register_theme_json_file(&mut direct_registry, &path)
        .expect("file register facade should load and register")
        .clone();
    assert_eq!(direct_entry.snapshot().source_revision(), 9001);

    std::fs::remove_file(&path).expect("temporary theme file should be removable");
}

#[test]
fn theme_registry_rejects_missing_required_identity_fields() {
    let mut registry = ThemeRegistry::new();

    assert_eq!(
        registry.register(ThemeDefinition::draft()).unwrap_err(),
        ThemeValidationError::MissingId
    );
    assert_eq!(
        registry
            .register(ThemeDefinition::draft().id("  "))
            .unwrap_err(),
        ThemeValidationError::MissingId
    );
    assert_eq!(
        registry
            .register(ThemeDefinition::draft().id("brand"))
            .unwrap_err(),
        ThemeValidationError::MissingLabel
    );
    assert_eq!(
        registry
            .register(ThemeDefinition::draft().id("brand").label("Brand"))
            .unwrap_err(),
        ThemeValidationError::MissingMode
    );
    assert_eq!(
        registry
            .register(
                ThemeDefinition::draft()
                    .id("brand")
                    .label("Brand")
                    .mode(ThemeMode::Light)
            )
            .unwrap_err(),
        ThemeValidationError::MissingSourceRevision
    );
    assert_eq!(
        registry
            .register(
                ThemeDefinition::draft()
                    .id("brand")
                    .label("Brand")
                    .mode(ThemeMode::Light)
                    .source_revision(1)
            )
            .unwrap_err(),
        ThemeValidationError::MissingDesignScales
    );
}

#[test]
fn theme_registry_rejects_duplicate_and_unsupported_programmatic_colors_atomically() {
    let source = ThemeSnapshot::light();
    let mut registry = ThemeRegistry::new();

    assert_eq!(
        registry
            .register(
                ThemeDefinition::from_snapshot("duplicate", "Duplicate", &source).color(
                    ThemeColor::new(semantic::ACCENT, ColorState::Default, 0x123456),
                )
            )
            .unwrap_err(),
        ThemeValidationError::DuplicateColor {
            token: semantic::ACCENT,
            state: ColorState::Default,
        }
    );
    assert!(registry.entries().is_empty());

    let unsupported = open_gpui_ui_core::TokenKey::new("semantic.unsupported");
    assert_eq!(
        registry
            .register(
                ThemeDefinition::from_snapshot("unsupported", "Unsupported", &source)
                    .color(ThemeColor::new(unsupported, ColorState::Default, 0x123456),)
            )
            .unwrap_err(),
        ThemeValidationError::UnsupportedColor {
            token: unsupported,
            state: ColorState::Default,
        }
    );
    assert!(registry.entries().is_empty());

    assert_eq!(
        registry
            .register(complete_theme_with_color(
                "invalid-rgb",
                "Invalid RGB",
                ThemeMode::Light,
                1,
                semantic::ACCENT,
                ColorState::Default,
                0x0100_0000,
            ))
            .unwrap_err(),
        ThemeValidationError::InvalidColorRgb {
            token: semantic::ACCENT,
            state: ColorState::Default,
            rgb: 0x0100_0000,
        }
    );
    assert!(registry.entries().is_empty());

    let defaults = ThemeDesignScales::default();
    let invalid_elevation = ThemeDesignScales::new(
        defaults.typography(),
        defaults.spacing(),
        defaults.radius(),
        ThemeElevationScale::new([
            ThemeElevationLayer::new(0, 10, 15, -3, 101),
            defaults.elevation().overlay()[1],
        ]),
        defaults.density(),
        defaults.motion(),
    );
    assert_eq!(
        registry
            .register(
                ThemeDefinition::from_snapshot("invalid-elevation", "Invalid elevation", &source)
                    .design_scales(invalid_elevation)
            )
            .unwrap_err(),
        ThemeValidationError::InvalidElevationOpacity {
            layer: 0,
            opacity_percent: 101,
        }
    );
    assert!(registry.entries().is_empty());
}

#[test]
fn theme_registry_replaces_existing_definition_by_stable_id() {
    let mut registry = ThemeRegistry::new();

    let initial_revision = registry
        .register(complete_theme_with_color(
            "brand",
            "Brand",
            ThemeMode::Light,
            1,
            semantic::ACCENT,
            ColorState::Default,
            0x111111,
        ))
        .expect("initial theme should register")
        .effective_revision();
    let metadata_only_revision = registry
        .register(complete_theme_with_color(
            "brand",
            "Brand metadata refreshed",
            ThemeMode::Light,
            2,
            semantic::ACCENT,
            ColorState::Default,
            0x111111,
        ))
        .expect("metadata-only refresh should register")
        .effective_revision();
    assert_eq!(metadata_only_revision, initial_revision);

    let changed_revision = registry
        .register(complete_theme_with_color(
            "brand",
            "Brand content refreshed",
            ThemeMode::Light,
            2,
            semantic::ACCENT,
            ColorState::Default,
            0x222222,
        ))
        .expect("effective theme refresh should replace by id")
        .effective_revision();
    assert!(changed_revision > metadata_only_revision);

    assert_eq!(registry.entries().len(), 1);
    let snapshot = registry
        .snapshot("brand")
        .expect("brand snapshot should exist");
    assert_eq!(snapshot.source_revision(), 2);
    assert_eq!(
        snapshot.color_rgb(semantic::ACCENT, ColorState::Default),
        Some(0x222222)
    );
}

#[test]
fn theme_registry_types_live_on_explicit_theme_owner_surface() {
    use open_gpui_ui_components::{self as root, prelude, theme as theme_owner};

    let mut root_registry: theme_owner::ThemeRegistry = theme_owner::ThemeRegistry::with_builtins();
    let root_definition: theme_owner::ThemeDefinition =
        theme_owner::ThemeDefinition::from_snapshot(
            "root-brand",
            "Root brand",
            &root::ThemeSnapshot::light(),
        )
        .source_revision(7);
    let root_entry: theme_owner::ThemeRegistryEntry = root_registry
        .register(root_definition)
        .expect("theme owner ThemeRegistry should register exported ThemeDefinition")
        .clone();
    let root_scales: theme_owner::ThemeDesignScales = root_entry.snapshot().design_scales();
    let root_error: theme_owner::ThemeValidationError =
        theme_owner::ThemeValidationError::MissingId;
    let root_load_error: theme_owner::ThemeLoadError =
        theme_owner::theme_definition_from_json_str("{}").unwrap_err();
    let root_file_field: theme_owner::ThemeFileField = theme_owner::ThemeFileField::SchemaVersion;
    let _root_schema = theme_owner::theme_json_schema();

    let mut prelude_registry: theme_owner::ThemeRegistry =
        theme_owner::ThemeRegistry::with_builtins();
    let prelude_definition: theme_owner::ThemeDefinition =
        theme_owner::ThemeDefinition::from_snapshot(
            "prelude-brand",
            "Prelude brand",
            &prelude::ThemeSnapshot::dark(),
        )
        .source_revision(8);
    let prelude_entry: theme_owner::ThemeRegistryEntry = prelude_registry
        .register(prelude_definition)
        .expect("theme owner ThemeRegistry should register exported ThemeDefinition")
        .clone();
    let prelude_scales: theme_owner::ThemeDesignScales = prelude_entry.snapshot().design_scales();
    let prelude_error: theme_owner::ThemeValidationError =
        theme_owner::ThemeValidationError::MissingLabel;
    let prelude_load_error: theme_owner::ThemeLoadError =
        theme_owner::theme_definition_from_json_str("{}").unwrap_err();
    let prelude_file_field: theme_owner::ThemeFileField =
        theme_owner::ThemeFileField::SchemaVersion;
    let _prelude_schema = theme_owner::theme_json_schema();

    assert_eq!(root_entry.snapshot().source_revision(), 7);
    assert_eq!(prelude_entry.snapshot().source_revision(), 8);
    assert_eq!(theme_owner::THEME_JSON_SCHEMA_VERSION, 1);
    assert_eq!(root_scales, theme_owner::ThemeDesignScales::default());
    assert_eq!(prelude_scales, theme_owner::ThemeDesignScales::default());
    assert_eq!(root_error, theme_owner::ThemeValidationError::MissingId);
    assert_eq!(
        prelude_error,
        theme_owner::ThemeValidationError::MissingLabel
    );
    assert_eq!(
        root_load_error,
        theme_owner::ThemeLoadError::MissingField(root_file_field)
    );
    assert_eq!(
        prelude_load_error,
        theme_owner::ThemeLoadError::MissingField(prelude_file_field)
    );
    let valid_theme_json = valid_theme_json();
    theme_owner::register_theme_json_str(&mut root_registry, &valid_theme_json)
        .expect("theme owner register_theme_json_str should register exported loader output");
    theme_owner::register_theme_json_str(&mut prelude_registry, &valid_theme_json)
        .expect("theme owner register_theme_json_str should register exported loader output");
}

#[test]
fn default_theme_resolves_all_current_component_color_intents() {
    let light = ThemeSnapshot::light();
    let dark = ThemeSnapshot::dark();
    let high_contrast = ThemeSnapshot::high_contrast();
    let theme = [&light, &dark, &high_contrast];
    let buttons = [
        Button::new("default", "Default").state(),
        Button::new("secondary", "Secondary")
            .variant(ButtonVariant::Secondary)
            .state(),
        Button::new("outline", "Outline")
            .variant(ButtonVariant::Outline)
            .state(),
        Button::new("ghost", "Ghost")
            .variant(ButtonVariant::Ghost)
            .state(),
        Button::new("destructive", "Destructive")
            .variant(ButtonVariant::Destructive)
            .state(),
        Button::new("selected", "Selected").selected(true).state(),
    ];
    let badges = [
        Badge::new("default-badge", "Default").state(),
        Badge::new("secondary-badge", "Secondary")
            .variant(BadgeVariant::Secondary)
            .state(),
        Badge::new("destructive-badge", "Destructive")
            .variant(BadgeVariant::Destructive)
            .state(),
        Badge::new("outline-badge", "Outline")
            .variant(BadgeVariant::Outline)
            .state(),
    ];
    let avatars = [
        Avatar::new("avatar", "Ada Lovelace").state(),
        Avatar::new("source-avatar", "Ada Lovelace")
            .source("asset://avatars/ada.png")
            .state(),
    ];
    let status_cues = [
        StatusCue::new("status-neutral", "Neutral").state(),
        StatusCue::new("status-info", "Info")
            .intent(FeedbackIntent::Info)
            .state(),
        StatusCue::new("status-success", "Success")
            .intent(FeedbackIntent::Success)
            .state(),
        StatusCue::new("status-warning", "Warning")
            .intent(FeedbackIntent::Warning)
            .state(),
        StatusCue::new("status-danger", "Danger")
            .intent(FeedbackIntent::Danger)
            .state(),
    ];
    let empty_states = [
        EmptyState::new("empty-neutral", "Neutral").state(),
        EmptyState::new("empty-danger", "Danger")
            .description("Needs action")
            .intent(FeedbackIntent::Danger)
            .state(),
    ];
    let icon_buttons = [
        IconButton::new("search", "?", "Search").state(),
        IconButton::new("outline-icon", "+", "Add")
            .variant(ButtonVariant::Outline)
            .state(),
        IconButton::new("danger-icon", "!", "Delete")
            .variant(ButtonVariant::Destructive)
            .state(),
        IconButton::new("selected-icon", "?", "Selected")
            .selected(true)
            .state(),
    ];
    let switches = [
        Switch::new("off").state(),
        Switch::new("on").checked(true).state(),
    ];
    let checkboxes = [
        Checkbox::new("unchecked").state(),
        Checkbox::new("checked").checked(true).state(),
        Checkbox::new("mixed").indeterminate(true).state(),
        Checkbox::new("invalid").invalid(true).state(),
    ];
    let radio_groups = [
        RadioGroup::new("plan")
            .default_selected("team")
            .item(RadioItem::new("personal", "Personal"))
            .item(RadioItem::new("team", "Team"))
            .state(),
        RadioGroup::new("disabled-plan")
            .disabled(true)
            .item(RadioItem::new("personal", "Personal"))
            .state(),
    ];
    let toggles = [
        Toggle::new("ghost-off", "Ghost off").state(),
        Toggle::new("ghost-on", "Ghost on").pressed(true).state(),
        Toggle::new("outline-on", "Outline on")
            .variant(ToggleVariant::Outline)
            .pressed(true)
            .state(),
    ];
    let text_inputs = [
        TextInput::new("default", "Default").state(),
        TextInput::new("disabled", "Disabled")
            .disabled(true)
            .state(),
        TextInput::new("readonly", "Read only")
            .read_only(true)
            .state(),
        TextInput::new("invalid", "Invalid").invalid(true).state(),
    ];
    let fields = [
        Field::new("field", "Field").state(),
        Field::new("required", "Required").required(true).state(),
        Field::new("disabled", "Disabled").disabled(true).state(),
        Field::new("invalid", "Invalid").invalid(true).state(),
    ];
    let labels = [
        Label::new("label", "Label").state(),
        Label::new("required-label", "Required")
            .required(true)
            .state(),
        Label::new("disabled-label", "Disabled")
            .disabled(true)
            .state(),
    ];
    let separators = [
        Separator::new("separator").state(),
        Separator::new("vertical-separator").vertical().state(),
    ];
    let kbds = [
        Kbd::new("kbd", "Ctrl+K").state(),
        Kbd::new("large-kbd", "Enter").large().state(),
    ];
    let progress = [
        Progress::new("progress", "Progress").value(50.0).state(),
        Progress::new("indeterminate-progress", "Progress")
            .indeterminate()
            .state(),
    ];
    let skeletons = [
        Skeleton::new("skeleton").state(),
        Skeleton::new("subtle-skeleton").subtle(true).state(),
    ];
    let menus = [
        Menu::new("menu", "Menu")
            .open(true)
            .item(MenuItem::action("open", "Open"))
            .state(),
        Menu::new("closed-menu", "Closed")
            .item(MenuItem::action("open", "Open"))
            .state(),
    ];
    let alert_dialogs = [
        AlertDialog::new(
            "alert",
            "Open",
            "Confirm",
            "Continue with changes.",
            "Continue",
        )
        .open(true)
        .state(),
        AlertDialog::new(
            "danger-alert",
            "Delete",
            "Delete item?",
            "This removes it.",
            "Delete",
        )
        .intent(AlertDialogIntent::Destructive)
        .open(true)
        .state(),
    ];
    let sheets = [
        Sheet::new("sheet", "Open sheet", "Sheet", "Sheet content")
            .open(true)
            .state(),
        Sheet::new("closed-sheet", "Closed sheet", "Closed", "Closed content").state(),
    ];
    let hover_cards = [
        HoverCard::new("hover-card", "Profile", "Profile details")
            .open(true)
            .state(),
        HoverCard::element("closed-hover-card", "Details", div().child("Rich")).state(),
    ];
    let listboxes = [
        Listbox::new("listbox", "Choices")
            .selected(Some("one".to_owned()))
            .option(ListboxOption::new("one", "One"))
            .option(ListboxOption::new("two", "Two").disabled(true))
            .state(),
        Listbox::new("empty-listbox", "Empty").state(),
    ];
    let selects = [
        Select::new("select", "Choice")
            .open(true)
            .selected(Some("one".to_owned()))
            .option(ListboxOption::new("one", "One"))
            .state(),
        Select::new("closed-select", "Choice").state(),
    ];
    let comboboxes = [
        Combobox::new("combobox", "Search")
            .open(true)
            .default_query("one")
            .option(ComboboxOption::new("one", "One"))
            .state(),
        Combobox::new("closed-combobox", "Search").state(),
    ];
    let commands = [
        Command::new("command", "Commands")
            .open(true)
            .default_query("open")
            .item(CommandItem::new("open", "Open"))
            .state(),
        Command::new("closed-command", "Commands").state(),
    ];
    let table_toolbars = [
        TableToolbar::new("table-toolbar", "Filters")
            .summary("2 filtered")
            .state(),
        TableToolbar::new("small-table-toolbar", "Filters")
            .small()
            .state(),
    ];

    for state in buttons {
        let colors = state.colors();
        for intent in [
            colors.background(),
            colors.foreground(),
            colors.border(),
            colors.hover_background(),
            colors.focus_ring(),
        ] {
            assert_theme_has_exact_color(theme, intent);
        }
    }

    for state in badges {
        let colors = state.colors();
        for intent in [colors.background(), colors.foreground(), colors.border()] {
            assert_theme_has_exact_color(theme, intent);
        }
    }

    for state in avatars {
        let colors = state.colors();
        for intent in [colors.background(), colors.foreground(), colors.border()] {
            assert_theme_has_exact_color(theme, intent);
        }
    }

    for state in status_cues {
        let colors = state.colors();
        for intent in [
            colors.background(),
            colors.foreground(),
            colors.muted_foreground(),
            colors.border(),
            colors.marker(),
        ] {
            assert_theme_has_exact_color(theme, intent);
        }
    }

    for state in empty_states {
        let colors = state.colors();
        for intent in [
            colors.background(),
            colors.foreground(),
            colors.muted_foreground(),
            colors.border(),
            colors.marker(),
        ] {
            assert_theme_has_exact_color(theme, intent);
        }
    }

    for state in icon_buttons {
        let colors = state.colors();
        for intent in [
            colors.background(),
            colors.foreground(),
            colors.border(),
            colors.hover_background(),
            colors.focus_ring(),
        ] {
            assert_theme_has_exact_color(theme, intent);
        }
    }

    for state in switches {
        let colors = state.colors();
        for intent in [
            colors.track(),
            colors.thumb(),
            colors.border(),
            colors.label(),
            colors.focus_ring(),
        ] {
            assert_theme_has_exact_color(theme, intent);
        }
    }

    for state in checkboxes {
        let colors = state.colors();
        for intent in [
            colors.background(),
            colors.hover_background(),
            colors.border(),
            colors.indicator(),
            colors.label(),
            colors.focus_ring(),
        ] {
            assert_theme_has_exact_color(theme, intent);
        }
    }

    for state in radio_groups {
        let colors = state.colors();
        for intent in [
            colors.control_background(),
            colors.control_background_selected(),
            colors.control_border(),
            colors.control_border_selected(),
            colors.indicator(),
            colors.label(),
            colors.label_muted(),
            colors.hover_background(),
            colors.focus_ring(),
        ] {
            assert_theme_has_exact_color(theme, intent);
        }
    }

    for state in toggles {
        let colors = state.colors();
        for intent in [
            colors.background(),
            colors.foreground(),
            colors.border(),
            colors.hover_background(),
            colors.focus_ring(),
        ] {
            assert_theme_has_exact_color(theme, intent);
        }
    }

    for state in text_inputs {
        let colors = state.colors();
        for intent in [
            colors.background(),
            colors.foreground(),
            colors.placeholder(),
            colors.border(),
            colors.focus_ring(),
        ] {
            assert_theme_has_exact_color(theme, intent);
        }
    }

    for state in fields {
        let colors = state.colors();
        for intent in [colors.label(), colors.message(), colors.required_marker()] {
            assert_theme_has_exact_color(theme, intent);
        }
    }

    for state in labels {
        let colors = state.colors();
        for intent in [colors.text(), colors.required_marker()] {
            assert_theme_has_exact_color(theme, intent);
        }
    }

    for state in separators {
        let colors = state.colors();
        for intent in [colors.line()] {
            assert_theme_has_exact_color(theme, intent);
        }
    }

    for state in kbds {
        let colors = state.colors();
        for intent in [colors.background(), colors.foreground(), colors.border()] {
            assert_theme_has_exact_color(theme, intent);
        }
    }

    for state in progress {
        let colors = state.colors();
        for intent in [colors.track(), colors.indicator()] {
            assert_theme_has_exact_color(theme, intent);
        }
    }

    for state in skeletons {
        let colors = state.colors();
        for intent in [colors.background()] {
            assert_theme_has_exact_color(theme, intent);
        }
    }

    for state in menus {
        let colors = state.colors();
        for intent in [
            colors.surface(),
            colors.foreground(),
            colors.border(),
            colors.item_background(),
            colors.item_hover_background(),
            colors.item_focus_background(),
            colors.item_disabled_foreground(),
            colors.header_foreground(),
            colors.separator(),
            colors.trigger_background(),
            colors.trigger_hover_background(),
            colors.trigger_foreground(),
            colors.trigger_border(),
            colors.focus_ring(),
        ] {
            assert_theme_has_exact_color(theme, intent);
        }
    }

    for state in alert_dialogs {
        let colors = state.colors();
        for intent in [
            colors.barrier(),
            colors.surface(),
            colors.foreground(),
            colors.muted_foreground(),
            colors.border(),
            colors.trigger_background(),
            colors.trigger_hover_background(),
            colors.trigger_foreground(),
            colors.trigger_border(),
            colors.action_background(),
            colors.action_hover_background(),
            colors.action_foreground(),
            colors.action_border(),
            colors.cancel_background(),
            colors.cancel_hover_background(),
            colors.cancel_foreground(),
            colors.cancel_border(),
            colors.focus_ring(),
        ] {
            assert_theme_has_exact_color(theme, intent);
        }
    }

    for state in sheets {
        let colors = state.colors();
        for intent in [
            colors.barrier(),
            colors.surface(),
            colors.foreground(),
            colors.muted_foreground(),
            colors.border(),
            colors.trigger_background(),
            colors.trigger_hover_background(),
            colors.trigger_foreground(),
            colors.trigger_border(),
            colors.close_background(),
            colors.close_hover_background(),
            colors.close_foreground(),
            colors.close_border(),
            colors.focus_ring(),
        ] {
            assert_theme_has_exact_color(theme, intent);
        }
    }

    for state in hover_cards {
        let colors = state.colors();
        for intent in [
            colors.background(),
            colors.foreground(),
            colors.muted_foreground(),
            colors.border(),
            colors.trigger_background(),
            colors.trigger_hover_background(),
            colors.trigger_foreground(),
            colors.trigger_border(),
            colors.focus_ring(),
        ] {
            assert_theme_has_exact_color(theme, intent);
        }
    }

    for state in listboxes {
        let colors = state.colors();
        for intent in [
            colors.surface(),
            colors.foreground(),
            colors.muted_foreground(),
            colors.border(),
            colors.option_background(),
            colors.option_hover_background(),
            colors.option_active_background(),
            colors.option_selected_background(),
            colors.option_disabled_foreground(),
            colors.separator(),
            colors.focus_ring(),
        ] {
            assert_theme_has_exact_color(theme, intent);
        }
    }

    for state in selects {
        let colors = state.colors();
        for intent in [
            colors.trigger_background(),
            colors.trigger_hover_background(),
            colors.trigger_foreground(),
            colors.trigger_placeholder_foreground(),
            colors.trigger_border(),
            colors.content_background(),
            colors.content_foreground(),
            colors.content_border(),
            colors.focus_ring(),
        ] {
            assert_theme_has_exact_color(theme, intent);
        }
    }

    for state in comboboxes {
        let colors = state.colors();
        for intent in [
            colors.popup_background(),
            colors.popup_foreground(),
            colors.popup_border(),
            colors.focus_ring(),
        ] {
            assert_theme_has_exact_color(theme, intent);
        }
    }

    for state in commands {
        let colors = state.colors();
        for intent in [
            colors.surface(),
            colors.foreground(),
            colors.muted_foreground(),
            colors.border(),
            colors.focus_ring(),
        ] {
            assert_theme_has_exact_color(theme, intent);
        }
    }

    for state in table_toolbars {
        let colors = state.colors();
        for intent in [colors.foreground(), colors.muted_foreground()] {
            assert_theme_has_exact_color(theme, intent);
        }
    }
}

fn assert_theme_has_exact_color(
    themes: [&ThemeSnapshot; 3],
    intent: open_gpui_ui_components::ColorIntent,
) {
    for theme in themes {
        assert!(
            theme
                .colors()
                .iter()
                .any(|entry| entry.token() == intent.token() && entry.state() == intent.state()),
            "missing {} theme color for {} / {}",
            theme.mode().as_str(),
            intent.token(),
            intent.state().as_str()
        );
    }
}

#[test]
fn theme_snapshots_resolve_state_specific_component_tokens() {
    let button = Button::new("secondary", "Secondary")
        .variant(ButtonVariant::Secondary)
        .state();
    let selected_switch = Switch::new("feature").checked(true).state();
    let mixed_checkbox = Checkbox::new("permissions").indeterminate(true).state();
    let disabled_input = TextInput::new("disabled", "Disabled")
        .disabled(true)
        .state();
    let invalid_input = TextInput::new("email", "Email").invalid(true).state();
    let required_field = Field::new("email-field", "Email").required(true).state();
    let theme = ThemeSnapshot::light();

    assert_eq!(
        button.colors().hover_background().state(),
        ColorState::Hover
    );
    assert_eq!(
        selected_switch.colors().track().state(),
        ColorState::Selected
    );
    assert_eq!(
        mixed_checkbox.colors().background().state(),
        ColorState::Selected
    );
    assert_eq!(
        disabled_input.colors().background().state(),
        ColorState::Disabled
    );
    assert_eq!(invalid_input.colors().border().state(), ColorState::Invalid);
    assert_eq!(
        invalid_input.colors().focus_ring().state(),
        ColorState::FocusVisible
    );
    assert_eq!(
        required_field.colors().required_marker().state(),
        ColorState::Required
    );
    assert_eq!(
        Label::new("required-label", "Required")
            .required(true)
            .state()
            .colors()
            .required_marker()
            .state(),
        ColorState::Required
    );

    assert_eq!(
        u32::from(ThemeResolver::resolve_with(
            button.colors().hover_background(),
            &theme
        )),
        0xdfe6dcff
    );
    assert_eq!(
        u32::from(ThemeResolver::resolve_with(
            disabled_input.colors().background(),
            &theme
        )),
        0xf1f5eeff
    );
    assert_eq!(
        u32::from(ThemeResolver::resolve_with(
            invalid_input.colors().focus_ring(),
            &theme
        )),
        0x2f80edff
    );
}
