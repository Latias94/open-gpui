use open_gpui::{AppContext, div, px};
use open_gpui_ui_components::{
    Badge, BadgeVariant, Button, ButtonVariant, Checkbox, ColorState, DEFAULT_FOCUS_RING_WIDTH,
    Field, FocusRing, IconButton, Label, RadioGroup, RadioGroupState, RadioItem,
    RadioItemDescriptor, Switch, Tabs, TabsActivationMode, TabsItem, TabsItemDescriptor, TabsState,
    TextInput, TextInputController, ThemeColor, ThemeMode, ThemeResolver, ThemeSnapshot, Toggle,
    ToggleVariant, active_index_from_str_keys, first_enabled, focus_ring_shadow, last_enabled,
    next_enabled,
};
use open_gpui_ui_core::{
    Orientation, Role, Sizable, Size, ThemeTokens, Toggled, TokenKey, semantic,
};

const TEST_SURFACE: TokenKey = TokenKey::new("test.surface");
const TEST_SURFACE_MUTED: TokenKey = TokenKey::new("test.surface_muted");
const TEST_BORDER: TokenKey = TokenKey::new("test.border");
const TEST_TEXT: TokenKey = TokenKey::new("test.text");
const TEST_TEXT_MUTED: TokenKey = TokenKey::new("test.text_muted");
const TEST_ACCENT: TokenKey = TokenKey::new("test.accent");
const TEST_FOCUS_RING: TokenKey = TokenKey::new("test.focus_ring");
const TEST_DESTRUCTIVE: TokenKey = TokenKey::new("test.destructive");

fn custom_tokens() -> ThemeTokens {
    ThemeTokens {
        surface: TEST_SURFACE,
        surface_muted: TEST_SURFACE_MUTED,
        border: TEST_BORDER,
        text: TEST_TEXT,
        text_muted: TEST_TEXT_MUTED,
        accent: TEST_ACCENT,
        focus_ring: TEST_FOCUS_RING,
        destructive: TEST_DESTRUCTIVE,
        ..ThemeTokens::default()
    }
}

#[test]
fn default_button_state_uses_button_role_and_medium_metrics() {
    let state = Button::new("save", "Save").state();

    assert_eq!(state.role(), Role::Button);
    assert_eq!(state.variant(), ButtonVariant::Default);
    assert_eq!(state.size(), Size::Medium);
    assert_eq!(state.metrics().height(), Size::Medium.button_h());
    assert_eq!(state.metrics().padding_x(), Size::Medium.button_px());
    assert_eq!(state.colors().background().token(), semantic::ACCENT);
    assert_eq!(state.focus_ring().color().token(), semantic::FOCUS_RING);
    assert_eq!(state.focus_ring().width(), DEFAULT_FOCUS_RING_WIDTH);
    assert!(!state.focus_ring().changes_layout());
    assert!(state.activation_enabled());
}

#[test]
fn destructive_button_uses_destructive_token_intent() {
    let state = Button::new("delete", "Delete")
        .variant(ButtonVariant::Destructive)
        .state();

    assert_eq!(state.colors().background().token(), semantic::DESTRUCTIVE);
    assert_eq!(
        state.colors().foreground().token(),
        semantic::DESTRUCTIVE_FOREGROUND
    );
}

#[test]
fn disabled_button_blocks_activation_metadata() {
    let state = Button::new("disabled", "Disabled").disabled(true).state();

    assert!(state.disabled());
    assert!(!state.activation_enabled());
}

#[test]
fn button_size_helpers_apply_foundation_size_metrics() {
    let state = Button::new("large", "Large").large().state();

    assert_eq!(state.size(), Size::Large);
    assert_eq!(state.metrics().height(), px(36.0));
    assert_eq!(state.metrics().text_size(), Size::Large.control_text_px());
}

#[test]
fn tabs_navigation_helpers_skip_disabled_tabs() {
    let keys = vec![
        "overview".to_string(),
        "details".to_string(),
        "history".to_string(),
    ];
    let disabled = [false, true, false];

    assert_eq!(first_enabled(&disabled), Some(0));
    assert_eq!(last_enabled(&disabled), Some(2));
    assert_eq!(next_enabled(&disabled, 0, true, true), Some(2));
    assert_eq!(next_enabled(&disabled, 2, false, true), Some(0));
    assert_eq!(
        active_index_from_str_keys(&keys, Some("details"), &disabled),
        Some(0)
    );
    assert_eq!(
        active_index_from_str_keys(&keys, Some("missing"), &disabled),
        Some(0)
    );
}

#[test]
fn tabs_state_resolution_tracks_selected_focus_and_tab_stop() {
    let state = TabsState::resolve(
        Orientation::Vertical,
        TabsActivationMode::Manual,
        Size::Small,
        Some("security"),
        Some("billing"),
        [
            TabsItemDescriptor::new("profile", "Profile"),
            TabsItemDescriptor::new("security", "Security"),
            TabsItemDescriptor::new("billing", "Billing").disabled(true),
            TabsItemDescriptor::new("integrations", "Integrations"),
        ],
        ThemeTokens::default(),
    );

    assert_eq!(state.orientation(), Orientation::Vertical);
    assert_eq!(state.activation_mode(), TabsActivationMode::Manual);
    assert_eq!(state.size(), Size::Small);
    assert_eq!(state.selected_value(), Some("security"));
    assert_eq!(state.focused_value(), Some("security"));
    assert_eq!(state.tab_stop_value(), Some("security"));
    assert!(state.items()[1].selected());
    assert!(state.items()[1].focused());
    assert!(state.items()[1].tab_stop());
    assert!(state.items()[2].disabled());
    assert!(!state.items()[2].tab_stop());
}

#[test]
fn tabs_builder_state_falls_back_to_first_enabled_tab() {
    let state = Tabs::new("settings")
        .orientation(Orientation::Horizontal)
        .activation_mode(TabsActivationMode::Automatic)
        .with_size(Size::Large)
        .selected("history")
        .item(TabsItem::new("overview", "Overview", div()))
        .item(TabsItem::new("details", "Details", div()))
        .item(TabsItem::new("history", "History", div()).disabled(true))
        .state();

    assert_eq!(state.orientation(), Orientation::Horizontal);
    assert_eq!(state.activation_mode(), TabsActivationMode::Automatic);
    assert_eq!(state.size(), Size::Large);
    assert_eq!(state.selected_value(), Some("overview"));
    assert_eq!(state.focused_value(), Some("overview"));
    assert_eq!(state.tab_stop_value(), Some("overview"));
    assert_eq!(state.items().len(), 3);
    assert!(state.items()[2].disabled());
    assert!(!state.items()[2].selected());
}

#[test]
fn radio_group_state_exposes_selection_required_and_disabled_items() {
    let state = RadioGroupState::resolve(
        Orientation::Vertical,
        Size::Medium,
        false,
        true,
        Some("team"),
        None,
        [
            RadioItemDescriptor::new("personal", "Personal"),
            RadioItemDescriptor::new("team", "Team"),
            RadioItemDescriptor::new("enterprise", "Enterprise").disabled(true),
        ],
        ThemeTokens::default(),
    );

    assert_eq!(state.role(), Role::RadioGroup);
    assert!(state.required());
    assert_eq!(state.selected_value(), Some("team"));
    assert_eq!(state.focused_value(), Some("team"));
    assert_eq!(state.tab_stop_value(), Some("team"));
    assert_eq!(state.items().len(), 3);
    assert!(state.items()[1].selected());
    assert!(state.items()[1].focused());
    assert!(state.items()[1].tab_stop());
    assert!(state.items()[2].disabled());
    assert!(!state.items()[2].activation_enabled());
    assert_eq!(state.items()[0].role(), Role::RadioButton);
}

#[test]
fn radio_group_reuses_roving_focus_helpers_and_skips_disabled_items() {
    let state = RadioGroupState::resolve(
        Orientation::Horizontal,
        Size::Small,
        false,
        false,
        Some("missing"),
        Some("enterprise"),
        [
            RadioItemDescriptor::new("starter", "Starter"),
            RadioItemDescriptor::new("pro", "Pro").disabled(true),
            RadioItemDescriptor::new("enterprise", "Enterprise"),
        ],
        ThemeTokens::default(),
    );

    assert_eq!(state.orientation(), Orientation::Horizontal);
    assert_eq!(state.size(), Size::Small);
    assert_eq!(state.selected_value(), Some("starter"));
    assert_eq!(state.focused_value(), Some("enterprise"));
    assert_eq!(state.tab_stop_value(), Some("enterprise"));
    assert!(state.items()[1].disabled());
    assert!(!state.items()[1].tab_stop());
}

#[test]
fn radio_group_builder_state_falls_back_to_first_enabled_item() {
    let state = RadioGroup::new("plan")
        .label("Plan")
        .orientation(Orientation::Horizontal)
        .with_size(Size::Large)
        .required(true)
        .selected("enterprise")
        .item(RadioItem::new("starter", "Starter"))
        .item(RadioItem::new("pro", "Pro"))
        .item(RadioItem::new("enterprise", "Enterprise").disabled(true))
        .state();

    assert_eq!(state.orientation(), Orientation::Horizontal);
    assert_eq!(state.size(), Size::Large);
    assert!(state.required());
    assert_eq!(state.selected_value(), Some("starter"));
    assert_eq!(state.focused_value(), Some("starter"));
    assert_eq!(state.tab_stop_value(), Some("starter"));
    assert!(state.items()[2].disabled());
    assert!(!state.items()[2].selected());
}

#[test]
fn toggle_pressed_state_maps_to_button_role_and_toggled_state() {
    let state = Toggle::new("notifications", "Notifications")
        .variant(ToggleVariant::Outline)
        .pressed(true)
        .small()
        .state();

    assert_eq!(state.role(), Role::Button);
    assert_eq!(state.toggled(), Toggled::True);
    assert!(state.pressed());
    assert_eq!(state.variant(), ToggleVariant::Outline);
    assert_eq!(state.size(), Size::Small);
    assert_eq!(state.colors().background().token(), semantic::ACCENT);
    assert_eq!(state.focus_ring().color().token(), semantic::FOCUS_RING);
    assert!(state.activation_enabled());
}

#[test]
fn disabled_toggle_blocks_activation_without_checkbox_semantics() {
    let state = Toggle::new("locked", "Locked")
        .pressed(false)
        .disabled(true)
        .state();

    assert_eq!(state.role(), Role::Button);
    assert_eq!(state.toggled(), Toggled::False);
    assert!(!state.pressed());
    assert!(state.disabled());
    assert!(!state.activation_enabled());
}

#[test]
fn badge_variants_resolve_display_only_token_intents() {
    let default = Badge::new("status", "Live").state();
    let secondary = Badge::new("beta", "Beta")
        .variant(BadgeVariant::Secondary)
        .small()
        .state();
    let destructive = Badge::new("risk", "Risk")
        .variant(BadgeVariant::Destructive)
        .state();
    let outline = Badge::new("neutral", "Neutral")
        .variant(BadgeVariant::Outline)
        .state();

    assert_eq!(default.variant(), BadgeVariant::Default);
    assert!(default.display_only());
    assert_eq!(default.role(), None);
    assert_eq!(default.colors().background().token(), semantic::ACCENT);
    assert_eq!(secondary.size(), Size::Small);
    assert_eq!(
        secondary.colors().background().token(),
        semantic::SURFACE_MUTED
    );
    assert_eq!(
        destructive.colors().background().token(),
        semantic::DESTRUCTIVE
    );
    assert_eq!(outline.colors().border().token(), semantic::BORDER);
}

#[test]
fn icon_button_requires_accessible_label_and_reuses_button_primitives() {
    let button = IconButton::new("search", "?", "Search")
        .variant(ButtonVariant::Outline)
        .small();
    let state = button.state();

    assert_eq!(button.accessible_label(), "Search");
    assert_eq!(state.role(), Role::Button);
    assert_eq!(state.variant(), ButtonVariant::Outline);
    assert_eq!(state.size(), Size::Small);
    assert_eq!(state.metrics().size(), Size::Small.icon_button_size());
    assert_eq!(state.metrics().icon_size(), Size::Small.icon_size());
    assert_eq!(state.colors().border().token(), semantic::BORDER);
    assert_eq!(state.focus_ring().color().token(), semantic::FOCUS_RING);
    assert!(state.activation_enabled());
}

#[test]
fn disabled_icon_button_blocks_activation_metadata() {
    let state = IconButton::new("locked", "x", "Locked")
        .disabled(true)
        .state();

    assert_eq!(state.role(), Role::Button);
    assert!(state.disabled());
    assert!(!state.activation_enabled());
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
fn default_theme_resolves_all_current_component_color_intents() {
    let theme = ThemeSnapshot::light();
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
            .selected("team")
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
}

fn assert_theme_has_exact_color(
    theme: ThemeSnapshot<'_>,
    intent: open_gpui_ui_components::ColorIntent,
) {
    assert!(
        theme
            .colors()
            .iter()
            .any(|entry| entry.token() == intent.token() && entry.state() == intent.state()),
        "missing theme color for {} / {}",
        intent.token(),
        intent.state().as_str()
    );
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

#[test]
fn switch_label_uses_theme_text_token() {
    let tokens = custom_tokens();
    let state = Switch::new("feature").tokens(tokens).state();

    assert_eq!(state.colors().label().token(), tokens.text);
}

#[test]
fn checked_switch_maps_to_true_toggled_state() {
    let state = Switch::new("feature").checked(true).state();

    assert!(state.checked());
    assert_eq!(state.role(), Role::Switch);
    assert_eq!(state.toggled(), Toggled::True);
    assert_eq!(state.colors().track().token(), semantic::ACCENT);
    assert_eq!(state.focus_ring().color().token(), semantic::FOCUS_RING);
    assert!(!state.focus_ring().changes_layout());
    assert!(state.activation_enabled());
}

#[test]
fn unchecked_switch_maps_to_false_toggled_state() {
    let state = Switch::new("feature").state();

    assert!(!state.checked());
    assert_eq!(state.toggled(), Toggled::False);
    assert_eq!(state.colors().track().token(), semantic::SURFACE_MUTED);
}

#[test]
fn disabled_switch_keeps_role_but_blocks_activation_metadata() {
    let state = Switch::new("feature").disabled(true).state();

    assert_eq!(state.role(), Role::Switch);
    assert!(state.disabled());
    assert!(!state.activation_enabled());
}

#[test]
fn switch_size_metrics_are_deterministic() {
    let state = Switch::new("feature").small().state();
    let metrics = state.metrics();

    assert_eq!(state.size(), Size::Small);
    assert_eq!(metrics.track_width(), px(32.0));
    assert_eq!(metrics.track_height(), px(18.0));
    assert_eq!(metrics.thumb_size(), px(14.0));
    assert_eq!(metrics.checked_thumb_x(), px(16.0));
}

#[test]
fn checkbox_states_map_to_checkbox_role_and_toggled_values() {
    let unchecked = Checkbox::new("unchecked").state();
    let checked = Checkbox::new("checked").checked(true).state();
    let mixed = Checkbox::new("mixed").indeterminate(true).state();

    assert_eq!(unchecked.role(), Role::CheckBox);
    assert_eq!(unchecked.toggled(), Toggled::False);
    assert!(!unchecked.checked());
    assert!(!unchecked.indeterminate());

    assert_eq!(checked.role(), Role::CheckBox);
    assert_eq!(checked.toggled(), Toggled::True);
    assert!(checked.checked());
    assert!(!checked.indeterminate());

    assert_eq!(mixed.role(), Role::CheckBox);
    assert_eq!(mixed.toggled(), Toggled::Mixed);
    assert!(!mixed.checked());
    assert!(mixed.indeterminate());
}

#[test]
fn disabled_checkbox_blocks_activation_metadata() {
    let state = Checkbox::new("disabled").disabled(true).state();

    assert_eq!(state.role(), Role::CheckBox);
    assert!(state.disabled());
    assert!(!state.activation_enabled());
    assert!(!state.tab_stop_enabled());
    assert_eq!(state.colors().background().state(), ColorState::Disabled);
}

#[test]
fn invalid_and_required_checkbox_expose_state_and_token_intents() {
    let tokens = custom_tokens();
    let state = Checkbox::new("terms")
        .checked(true)
        .required(true)
        .invalid(true)
        .tokens(tokens)
        .state();

    assert!(state.required());
    assert!(state.invalid());
    assert_eq!(state.colors().border().token(), tokens.destructive);
    assert_eq!(state.colors().border().state(), ColorState::Invalid);
    assert_eq!(state.colors().background().token(), tokens.accent);
    assert_eq!(state.colors().focus_ring().token(), tokens.focus_ring);
    assert!(!state.focus_ring().changes_layout());
}

#[test]
fn checkbox_checked_state_builder_accepts_mixed() {
    let state = Checkbox::new("bulk").checked_state(Toggled::Mixed).state();

    assert_eq!(state.toggled(), Toggled::Mixed);
    assert!(state.indeterminate());
    assert!(!state.checked());
}

#[test]
fn label_state_records_control_association_and_required_marker() {
    let tokens = custom_tokens();
    let state = Label::new("email-label", "Email")
        .for_control("email-input")
        .required(true)
        .tokens(tokens)
        .state();

    assert_eq!(state.role(), Role::Label);
    assert_eq!(state.text(), "Email");
    assert_eq!(state.control_id(), Some("email-input"));
    assert!(state.associated());
    assert!(state.required());
    assert_eq!(state.colors().text().token(), tokens.text);
    assert_eq!(state.colors().required_marker().token(), tokens.destructive);
}

#[test]
fn disabled_label_uses_muted_text_intent() {
    let tokens = custom_tokens();
    let state = Label::new("disabled-label", "Disabled")
        .disabled(true)
        .tokens(tokens)
        .state();

    assert!(state.disabled());
    assert_eq!(state.colors().text().token(), tokens.text_muted);
    assert_eq!(state.colors().text().state(), ColorState::Disabled);
}

#[test]
fn default_text_input_state_uses_text_input_role_and_placeholder_display() {
    let state = TextInput::new("email", "Email")
        .placeholder("Email address")
        .state();

    assert_eq!(state.role(), Role::TextInput);
    assert_eq!(state.size(), Size::Medium);
    assert_eq!(state.metrics().height(), Size::Medium.input_h());
    assert_eq!(state.metrics().padding_x(), Size::Medium.input_px());
    assert!(!state.has_value());
    assert_eq!(state.display_text(), "Email address");
    assert!(state.displaying_placeholder());
    assert!(state.editable());
}

#[test]
fn filled_text_input_reports_value_state() {
    let state = TextInput::new("email", "Email")
        .placeholder("Email address")
        .value("hello@example.com")
        .state();

    assert!(state.has_value());
    assert_eq!(state.value(), "hello@example.com");
    assert_eq!(state.display_text(), "hello@example.com");
    assert!(!state.displaying_placeholder());
}

#[test]
fn disabled_and_read_only_text_inputs_block_editability() {
    let tokens = custom_tokens();
    let disabled = TextInput::new("disabled", "Disabled")
        .disabled(true)
        .tokens(tokens)
        .state();
    let read_only = TextInput::new("readonly", "Read only")
        .read_only(true)
        .state();

    assert!(disabled.disabled());
    assert!(!disabled.editable());
    assert!(!disabled.activation_enabled());
    assert_eq!(disabled.colors().background().token(), tokens.surface_muted);
    assert!(read_only.read_only());
    assert!(!read_only.editable());
    assert!(!read_only.activation_enabled());
    assert_eq!(
        read_only.colors().background().token(),
        ThemeTokens::default().surface_muted
    );
    assert_eq!(read_only.role(), Role::TextInput);
}

#[test]
fn invalid_text_input_uses_destructive_border_token() {
    let tokens = custom_tokens();
    let state = TextInput::new("email", "Email")
        .invalid(true)
        .tokens(tokens)
        .state();

    assert!(state.invalid());
    assert_eq!(state.colors().border().token(), tokens.destructive);
    assert_eq!(state.colors().focus_ring().token(), tokens.focus_ring);
    assert_eq!(state.focus_ring().color().token(), tokens.focus_ring);
    assert!(!state.focus_ring().changes_layout());
    assert_eq!(state.colors().placeholder().token(), tokens.text_muted);
}

#[test]
fn focus_ring_preserves_token_intent_without_layout_shift() {
    let ring = FocusRing::from_color(Button::new("save", "Save").state().colors().focus_ring());
    let shadow = focus_ring_shadow(ring);

    assert_eq!(ring.color().token(), semantic::FOCUS_RING);
    assert_eq!(ring.width(), DEFAULT_FOCUS_RING_WIDTH);
    assert!(!ring.changes_layout());
    assert_eq!(shadow[0].spread_radius, DEFAULT_FOCUS_RING_WIDTH);
    assert_eq!(shadow[0].blur_radius, px(0.0));
    assert!(!shadow[0].inset);
}

#[test]
fn text_input_size_helpers_apply_input_metrics() {
    let state = TextInput::new("query", "Search").large().state();

    assert_eq!(state.size(), Size::Large);
    assert_eq!(state.metrics().height(), px(36.0));
    assert_eq!(state.metrics().text_size(), Size::Large.control_text_px());
}

#[open_gpui::test]
fn text_input_controller_converts_utf16_ranges_and_replaces_selection(
    cx: &mut open_gpui::TestAppContext,
) {
    let controller = cx.new(|cx| TextInputController::with_value("a🙂中", cx));

    cx.update_entity(&controller, |controller, cx| {
        let mut adjusted = None;

        assert_eq!(
            controller
                .text_for_range_utf16(1..3, &mut adjusted)
                .as_deref(),
            Some("🙂")
        );
        assert_eq!(adjusted, Some(1..3));

        controller.select_range(1.."a🙂".len(), cx);
        controller.replace_text_in_range_utf16(None, "b\nc", cx);

        assert_eq!(controller.value(), "ab c中");
        assert_eq!(controller.selected_range(), 4..4);
        assert_eq!(controller.selected_range_utf16(), 4..4);
    });
}

#[open_gpui::test]
fn text_input_controller_updates_marked_text_and_commits_composition(
    cx: &mut open_gpui::TestAppContext,
) {
    let controller = cx.new(TextInputController::new);

    cx.update_entity(&controller, |controller, cx| {
        controller.replace_and_mark_text_in_range_utf16(None, "ni", Some(1..2), cx);

        assert_eq!(controller.value(), "ni");
        assert_eq!(controller.marked_range_utf16(), Some(0..2));
        assert_eq!(controller.selected_range_utf16(), 1..2);

        controller.replace_text_in_range_utf16(None, "你", cx);

        assert_eq!(controller.value(), "你");
        assert_eq!(controller.marked_range_utf16(), None);
        assert_eq!(controller.selected_range_utf16(), 1..1);
    });
}

#[open_gpui::test]
fn text_input_controller_delete_commands_respect_grapheme_boundaries(
    cx: &mut open_gpui::TestAppContext,
) {
    let controller = cx.new(|cx| TextInputController::with_value("a👨‍👩‍👧‍👦b", cx));

    cx.update_entity(&controller, |controller, cx| {
        controller.move_to_offset("a👨‍👩‍👧‍👦".len(), cx);
        controller.delete_backward(cx);

        assert_eq!(controller.value(), "ab");

        controller.move_to_offset(1, cx);
        controller.delete_forward(cx);

        assert_eq!(controller.value(), "a");
    });
}

#[open_gpui::test]
fn text_input_controller_rejects_editing_when_disabled_or_read_only(
    cx: &mut open_gpui::TestAppContext,
) {
    let controller = cx.new(|cx| TextInputController::with_value("locked", cx));

    cx.update_entity(&controller, |controller, cx| {
        controller.set_read_only(true, cx);
        controller.select_range(0..controller.value().len(), cx);
        controller.replace_text_in_range_utf16(None, "changed", cx);

        assert_eq!(controller.value(), "locked");

        controller.set_read_only(false, cx);
        controller.set_disabled(true, cx);
        controller.delete_backward(cx);

        assert_eq!(controller.value(), "locked");
        assert!(!controller.accepts_editing());
    });
}

#[open_gpui::test]
fn text_input_state_marks_controller_driven_inputs(cx: &mut open_gpui::TestAppContext) {
    let controller = cx.new(TextInputController::new);
    let state = TextInput::new("editable", "Editable")
        .controller(controller)
        .state();

    assert!(state.controller_driven());
    assert!(state.editable());
}

#[open_gpui::test]
fn controller_driven_text_input_state_marks_disabled_editing(cx: &mut open_gpui::TestAppContext) {
    let controller = cx.new(TextInputController::new);
    let state = TextInput::new("disabled", "Disabled")
        .controller(controller)
        .disabled(true)
        .state();

    assert!(state.controller_driven());
    assert!(state.disabled());
    assert!(!state.editable());
}

#[test]
fn default_field_state_exposes_label_help_and_metrics() {
    let state = Field::new("email-field", "email", "Email")
        .help("Use a work address.")
        .state();

    assert_eq!(state.label(), "Email");
    assert_eq!(state.help().unwrap(), "Use a work address.");
    assert_eq!(state.support_text().unwrap(), "Use a work address.");
    assert!(!state.support_is_error());
    assert_eq!(state.size(), Size::Medium);
    assert_eq!(
        state.metrics().label_text_size(),
        Size::Medium.control_text_px()
    );
}

#[test]
fn required_field_exposes_required_metadata() {
    let state = Field::new("email-field", "email", "Email")
        .required(true)
        .state();

    assert!(state.required());
    assert_eq!(
        state.colors().required_marker().token(),
        semantic::DESTRUCTIVE
    );
}

#[test]
fn invalid_field_prefers_error_support_text() {
    let tokens = custom_tokens();
    let state = Field::new("email-field", "email", "Email")
        .help("Use a work address.")
        .error("Enter a valid email.")
        .invalid(true)
        .tokens(tokens)
        .state();

    assert!(state.invalid());
    assert_eq!(state.support_text().unwrap(), "Enter a valid email.");
    assert!(state.support_is_error());
    assert_eq!(state.colors().message().token(), tokens.destructive);
}

#[test]
fn disabled_field_uses_muted_label_intent() {
    let tokens = custom_tokens();
    let state = Field::new("email-field", "email", "Email")
        .disabled(true)
        .tokens(tokens)
        .state();

    assert!(state.disabled());
    assert_eq!(state.colors().message().token(), tokens.text_muted);
}
