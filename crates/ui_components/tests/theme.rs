mod support;

use open_gpui::{ParentElement, div};
use open_gpui_ui_components::{
    AlertDialog, AlertDialogIntent, Avatar, Badge, BadgeVariant, Button, ButtonVariant, Checkbox,
    ColorIntent, ColorState, Combobox, ComboboxOption, Command, CommandItem, EmptyState,
    FeedbackIntent, Field, HoverCard, IconButton, Kbd, Label, Listbox, ListboxOption, Menu,
    MenuItem, Progress, RadioGroup, RadioItem, Select, Separator, Sheet, Skeleton, StatusCue,
    Switch, THEME_JSON_SCHEMA_VERSION, TableToolbar, TextInput, ThemeColor, ThemeDefinition,
    ThemeFileField, ThemeLoadError, ThemeMode, ThemeRegistry, ThemeResolver, ThemeSnapshot,
    ThemeValidationError, Toggle, ToggleVariant, register_theme_json_file, register_theme_json_str,
    theme_definition_from_json_file, theme_definition_from_json_str, theme_json_schema,
};
use open_gpui_ui_core::{Sizable, semantic};

use support::tokens::custom_tokens;

const VALID_THEME_JSON: &str = r##"{
  "schema_version": 1,
  "id": "forest-json",
  "label": "Forest JSON",
  "mode": "dark",
  "revision": 9001,
  "fallback_mode": "light",
  "colors": [
    { "token": "semantic.accent", "state": "default", "rgb": "#227755" },
    { "token": "semantic.accent", "state": "hover", "rgb": "#1b6044" },
    { "token": "semantic.surface_muted", "state": "selected", "rgb": "#173c32" },
    { "token": "semantic.surface_muted", "state": "disabled", "rgb": "#102820" },
    { "token": "semantic.destructive", "state": "invalid", "rgb": "#ff5544" },
    { "token": "semantic.focus_ring", "state": "focus-visible", "rgb": "#77b8ff" }
  ]
}"##;

fn portable_theme_definition(id: &str) -> ThemeDefinition {
    ThemeDefinition::new(id, "Forest JSON", ThemeMode::Dark, 9001)
        .fallback_mode(ThemeMode::Light)
        .colors([
            ThemeColor::new(semantic::ACCENT, ColorState::Default, 0x227755),
            ThemeColor::new(semantic::ACCENT, ColorState::Hover, 0x1b6044),
            ThemeColor::new(semantic::SURFACE_MUTED, ColorState::Selected, 0x173c32),
            ThemeColor::new(semantic::SURFACE_MUTED, ColorState::Disabled, 0x102820),
            ThemeColor::new(semantic::DESTRUCTIVE, ColorState::Invalid, 0xff5544),
            ThemeColor::new(semantic::FOCUS_RING, ColorState::FocusVisible, 0x77b8ff),
        ])
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
    assert_eq!(u32::from(ThemeResolver::resolve(background)), 0x1f7a66ff);
    assert_eq!(
        u32::from(ThemeResolver::resolve_with(
            background,
            ThemeSnapshot::dark()
        )),
        0x1f7a66ff
    );
}

#[test]
fn theme_resolver_prefers_runtime_theme_table_for_known_tokens() {
    let state = Button::new("default", "Default").state();
    let background = state.colors().background();
    let custom_colors = [ThemeColor::new(
        semantic::ACCENT,
        ColorState::Default,
        0x123456,
    )];
    let snapshot = ThemeSnapshot::new(ThemeMode::Light, 42, &custom_colors);

    assert_eq!(background.fallback_rgb(), 0x1f7a66);
    assert_eq!(
        u32::from(ThemeResolver::resolve_with(background, snapshot)),
        0x123456ff
    );
    assert_eq!(snapshot.mode(), ThemeMode::Light);
    assert_eq!(snapshot.revision(), 42);
}

#[test]
fn default_theme_snapshots_expose_distinct_modes_and_revisions() {
    let light = ThemeSnapshot::light();
    let dark = ThemeSnapshot::dark();
    let high_contrast = ThemeSnapshot::high_contrast();

    assert_eq!(light.mode().as_str(), "light");
    assert_eq!(dark.mode().as_str(), "dark");
    assert_eq!(high_contrast.mode().as_str(), "high-contrast");
    assert!(light.revision() < dark.revision());
    assert!(dark.revision() < high_contrast.revision());
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
                entry.snapshot().revision()
            ))
            .collect::<Vec<_>>(),
        vec![
            ("light", ThemeMode::Light, ThemeSnapshot::light().revision()),
            ("dark", ThemeMode::Dark, ThemeSnapshot::dark().revision()),
            (
                "high-contrast",
                ThemeMode::HighContrast,
                ThemeSnapshot::high_contrast().revision()
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
fn theme_registry_registers_user_definition_with_fallback_diagnostics() {
    let mut registry = ThemeRegistry::with_builtins();
    let entry = registry
        .register(
            ThemeDefinition::new("forest", "Forest", ThemeMode::Dark, 9001)
                .fallback_mode(ThemeMode::Light)
                .color(ThemeColor::new(
                    semantic::ACCENT,
                    ColorState::Default,
                    0x227755,
                ))
                .color(ThemeColor::new(
                    semantic::ACCENT,
                    ColorState::Hover,
                    0x1b6044,
                )),
        )
        .expect("valid user theme definition should register");
    let snapshot = entry.snapshot();

    assert_eq!(entry.id(), "forest");
    assert_eq!(entry.label(), "Forest");
    assert_eq!(snapshot.mode(), ThemeMode::Dark);
    assert_eq!(snapshot.revision(), 9001);
    assert_eq!(
        entry.diagnostics().fallback_mode(),
        ThemeMode::Light,
        "the registry should record which built-in table filled omitted optional tokens"
    );
    assert!(
        entry.diagnostics().fallback_color_count() > 0,
        "omitted optional token/state entries should be filled from the fallback snapshot"
    );
    assert_eq!(
        snapshot.color_rgb(semantic::ACCENT, ColorState::Default),
        Some(0x227755)
    );
    assert_eq!(
        snapshot.color_rgb(semantic::SURFACE, ColorState::Default),
        ThemeSnapshot::light().color_rgb(semantic::SURFACE, ColorState::Default)
    );
    assert_eq!(
        u32::from(ThemeResolver::resolve_with(
            ColorIntent::new(semantic::ACCENT, 0x1f7a66),
            snapshot
        )),
        0x227755ff
    );
}

#[test]
fn theme_json_loader_registers_portable_definition_like_code_built_theme() {
    let json_definition = theme_definition_from_json_str(VALID_THEME_JSON)
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
    assert_eq!(json_entry.snapshot().revision(), 9001);
    assert_eq!(json_entry.diagnostics().fallback_mode(), ThemeMode::Light);
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
        Some(0x102820)
    );
    assert_eq!(
        json_entry
            .snapshot()
            .color_rgb(semantic::DESTRUCTIVE, ColorState::Invalid),
        Some(0xff5544)
    );

    let mut direct_registry = ThemeRegistry::with_builtins();
    let direct_entry = register_theme_json_str(&mut direct_registry, VALID_THEME_JSON)
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
        "fallback_mode",
        "colors",
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
}

#[test]
fn theme_json_loader_reports_structured_errors_before_registration() {
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
}

#[test]
fn theme_json_file_facade_reads_and_registers_theme_files() {
    let path = std::env::temp_dir().join(format!(
        "open-gpui-theme-loader-{}-{}.json",
        std::process::id(),
        ThemeSnapshot::light().revision()
    ));
    std::fs::write(&path, VALID_THEME_JSON).expect("temporary theme file should be writable");

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
    assert_eq!(direct_entry.snapshot().revision(), 9001);

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
        ThemeValidationError::MissingRevision
    );
}

#[test]
fn theme_registry_replaces_existing_definition_by_stable_id() {
    let mut registry = ThemeRegistry::new();

    registry
        .register(
            ThemeDefinition::new("brand", "Brand", ThemeMode::Light, 1).color(ThemeColor::new(
                semantic::ACCENT,
                ColorState::Default,
                0x111111,
            )),
        )
        .expect("initial theme should register");
    registry
        .register(
            ThemeDefinition::new("brand", "Brand refreshed", ThemeMode::Light, 2).color(
                ThemeColor::new(semantic::ACCENT, ColorState::Default, 0x222222),
            ),
        )
        .expect("theme refresh should replace by id");

    assert_eq!(registry.entries().len(), 1);
    let snapshot = registry
        .snapshot("brand")
        .expect("brand snapshot should exist");
    assert_eq!(snapshot.revision(), 2);
    assert_eq!(
        snapshot.color_rgb(semantic::ACCENT, ColorState::Default),
        Some(0x222222)
    );
}

#[test]
fn theme_registry_types_are_exported_from_root_and_prelude() {
    use open_gpui_ui_components::{self as root, prelude};

    let mut root_registry: root::ThemeRegistry = root::ThemeRegistry::with_builtins();
    let root_definition: root::ThemeDefinition =
        root::ThemeDefinition::new("root-brand", "Root brand", root::ThemeMode::Light, 7);
    let root_entry: root::ThemeRegistryEntry = root_registry
        .register(root_definition)
        .expect("root ThemeRegistry should register exported ThemeDefinition")
        .clone();
    let root_diagnostics: root::ThemeRegistrationDiagnostics = root_entry.diagnostics();
    let root_error: root::ThemeValidationError = root::ThemeValidationError::MissingId;
    let root_load_error: root::ThemeLoadError =
        root::theme_definition_from_json_str("{}").unwrap_err();
    let root_file_field: root::ThemeFileField = root::ThemeFileField::SchemaVersion;
    let _root_schema = root::theme_json_schema();

    let mut prelude_registry: prelude::ThemeRegistry = prelude::ThemeRegistry::with_builtins();
    let prelude_definition: prelude::ThemeDefinition = prelude::ThemeDefinition::new(
        "prelude-brand",
        "Prelude brand",
        prelude::ThemeMode::Dark,
        8,
    );
    let prelude_entry: prelude::ThemeRegistryEntry = prelude_registry
        .register(prelude_definition)
        .expect("prelude ThemeRegistry should register exported ThemeDefinition")
        .clone();
    let prelude_diagnostics: prelude::ThemeRegistrationDiagnostics = prelude_entry.diagnostics();
    let prelude_error: prelude::ThemeValidationError = prelude::ThemeValidationError::MissingLabel;
    let prelude_load_error: prelude::ThemeLoadError =
        prelude::theme_definition_from_json_str("{}").unwrap_err();
    let prelude_file_field: prelude::ThemeFileField = prelude::ThemeFileField::SchemaVersion;
    let _prelude_schema = prelude::theme_json_schema();

    assert_eq!(root_entry.snapshot().revision(), 7);
    assert_eq!(prelude_entry.snapshot().revision(), 8);
    assert_eq!(root::THEME_JSON_SCHEMA_VERSION, 1);
    assert_eq!(prelude::THEME_JSON_SCHEMA_VERSION, 1);
    assert_eq!(root_diagnostics.fallback_mode(), root::ThemeMode::Light);
    assert!(root_diagnostics.fallback_color_count() > 0);
    assert_eq!(
        prelude_diagnostics.fallback_mode(),
        prelude::ThemeMode::Dark
    );
    assert_eq!(root_error, root::ThemeValidationError::MissingId);
    assert_eq!(prelude_error, prelude::ThemeValidationError::MissingLabel);
    assert_eq!(
        root_load_error,
        root::ThemeLoadError::MissingField(root_file_field)
    );
    assert_eq!(
        prelude_load_error,
        prelude::ThemeLoadError::MissingField(prelude_file_field)
    );
    root::register_theme_json_str(&mut root_registry, VALID_THEME_JSON)
        .expect("root register_theme_json_str should register exported loader output");
    prelude::register_theme_json_str(&mut prelude_registry, VALID_THEME_JSON)
        .expect("prelude register_theme_json_str should register exported loader output");
}

#[test]
fn default_theme_resolves_all_current_component_color_intents() {
    let theme = [
        ThemeSnapshot::light(),
        ThemeSnapshot::dark(),
        ThemeSnapshot::high_contrast(),
    ];
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
        Field::new("field", "control", "Field").state(),
        Field::new("required", "control", "Required")
            .required(true)
            .state(),
        Field::new("disabled", "control", "Disabled")
            .disabled(true)
            .state(),
        Field::new("invalid", "control", "Invalid")
            .invalid(true)
            .state(),
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
            .selected("one")
            .option(ListboxOption::new("one", "One"))
            .option(ListboxOption::new("two", "Two").disabled(true))
            .state(),
        Listbox::new("empty-listbox", "Empty").state(),
    ];
    let selects = [
        Select::new("select", "Choice")
            .open(true)
            .selected("one")
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
    themes: [ThemeSnapshot<'_>; 3],
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
    let required_field = Field::new("email-field", "email", "Email")
        .required(true)
        .state();
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
            theme
        )),
        0xdfe6dcff
    );
    assert_eq!(
        u32::from(ThemeResolver::resolve_with(
            disabled_input.colors().background(),
            theme
        )),
        0xf1f5eeff
    );
    assert_eq!(
        u32::from(ThemeResolver::resolve_with(
            invalid_input.colors().focus_ring(),
            theme
        )),
        0x2f80edff
    );
}
