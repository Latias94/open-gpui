use open_gpui::px;
use open_gpui_ui_components::{
    BadgeVariant, ButtonVariant, DEFAULT_OVERLAY_SAFE_MARGIN, TabsActivationMode, ThemeMode,
    ToggleVariant, default_deferred_priority,
};
use open_gpui_ui_core::{
    Density, DeviceAdaptiveClass, DeviceShellMode, EscapeKeyPolicy, FocusRestoreIntent,
    InitialFocusIntent, Orientation, OutsidePressPolicy, OverlayLayerKind, PanelAdaptiveClass,
    Role, Size, ThemeTokens, Toggled, semantic,
};
use open_gpui_ui_foundation_gallery::{
    DEFAULT_GALLERY_WIDTH, GALLERY_SECTIONS, GalleryPage, density_label, device_class_label,
    foundation_snapshot, pages, panel_class_label, shell_mode_label, size_label,
};

#[test]
fn gallery_sections_cover_the_foundation_slices() {
    let ids = GALLERY_SECTIONS
        .iter()
        .map(|section| section.id)
        .collect::<Vec<_>>();

    assert_eq!(
        ids,
        vec![
            "tokens",
            "sizing-density",
            "adaptive",
            "focus-a11y",
            "overlay",
            "components"
        ]
    );
}

#[test]
fn default_shell_consumes_ui_core_foundation_vocabulary() {
    let snapshot = foundation_snapshot(DEFAULT_GALLERY_WIDTH, GalleryPage::Tokens);

    assert_eq!(snapshot.selected_page, GalleryPage::Tokens);
    assert_eq!(snapshot.shell_mode, DeviceShellMode::Desktop);
    assert_eq!(snapshot.density, Density::Comfortable);
    assert_eq!(snapshot.control_size, Size::Medium);
    assert_eq!(snapshot.tokens.surface, semantic::SURFACE);
    assert_eq!(snapshot.tokens.focus_ring, semantic::FOCUS_RING);
}

#[test]
fn compact_width_uses_mobile_shell_and_compact_density() {
    let snapshot = foundation_snapshot(px(720.0), GalleryPage::Adaptive);

    assert_eq!(snapshot.selected_page, GalleryPage::Adaptive);
    assert_eq!(snapshot.shell_mode, DeviceShellMode::Mobile);
    assert_eq!(snapshot.density, Density::Compact);
    assert_eq!(snapshot.control_size, Size::Small);
}

#[test]
fn labels_are_stable_for_manual_dogfood_output() {
    assert_eq!(
        GalleryPage::from_id("components"),
        Some(GalleryPage::Components)
    );
    assert_eq!(GalleryPage::from_id("missing"), None);
    assert_eq!(shell_mode_label(DeviceShellMode::Desktop), "desktop");
    assert_eq!(shell_mode_label(DeviceShellMode::Mobile), "mobile");
    assert_eq!(density_label(Density::Spacious), "spacious");
    assert_eq!(size_label(Size::XSmall), "xs");
    assert_eq!(
        device_class_label(DeviceAdaptiveClass::Expanded),
        "expanded device"
    );
    assert_eq!(panel_class_label(PanelAdaptiveClass::Wide), "wide panel");
}

#[test]
fn package_manifest_stays_foundation_scoped() {
    let manifest = include_str!("../Cargo.toml");

    assert!(manifest.contains("open_gpui.workspace = true"));
    assert!(manifest.contains("open_gpui_ui_core.workspace = true"));
    assert!(manifest.contains("open_gpui_ui_components.workspace = true"));
    assert!(manifest.contains("open_gpui_platform.workspace = true"));
    assert!(!manifest.contains("open_gpui_canvas"));
    assert!(!manifest.contains("open_gpui_docking"));
    assert!(!manifest.contains("open_gpui_ui ="));
}

#[test]
fn token_page_samples_follow_theme_token_order() {
    let tokens = ThemeTokens::default();
    let samples = pages::tokens::token_samples(tokens);

    assert_eq!(samples.len(), 12);
    assert_eq!(samples[0].key, semantic::SURFACE);
    assert_eq!(samples[0].preview_rgb, 0xffffff);
    assert_eq!(samples[7].key, semantic::FOCUS_RING);
    assert_eq!(samples[11].key, semantic::MODAL_OVERLAY);
    assert!(pages::tokens::matches_semantic_registry(tokens));
}

#[test]
fn token_page_exposes_runtime_theme_mode_metadata() {
    let samples = pages::tokens::theme_mode_samples(ThemeTokens::default());

    assert_eq!(samples.len(), 3);
    assert_eq!(samples[0].mode, ThemeMode::Light);
    assert_eq!(samples[1].mode, ThemeMode::Dark);
    assert_eq!(samples[2].mode, ThemeMode::HighContrast);
    assert!(samples[0].revision < samples[1].revision);
    assert!(samples[1].revision < samples[2].revision);
    assert_ne!(samples[0].surface_rgb, samples[1].surface_rgb);
    assert_ne!(samples[1].focus_ring_rgb, samples[2].focus_ring_rgb);
}

#[test]
fn sizing_page_samples_expose_core_metrics() {
    let sizes = pages::sizing::SIZE_SAMPLES;
    let densities = pages::sizing::DENSITY_SAMPLES;

    assert_eq!(sizes[0].label, "xs");
    assert_eq!(sizes[2].button_h, px(32.0));
    assert_eq!(sizes[3].icon_button_size, px(36.0));
    assert_eq!(densities[0].default_size, Size::Small);
    assert_eq!(densities[2].default_size, Size::Large);
}

#[test]
fn adaptive_page_samples_cover_default_thresholds() {
    let devices = pages::adaptive::device_samples();
    let panels = pages::adaptive::panel_samples();

    assert_eq!(devices[0].shell_mode, DeviceShellMode::Mobile);
    assert_eq!(devices[0].class, DeviceAdaptiveClass::Compact);
    assert_eq!(devices[1].shell_mode, DeviceShellMode::Desktop);
    assert_eq!(devices[1].class, DeviceAdaptiveClass::Regular);
    assert_eq!(devices[2].class, DeviceAdaptiveClass::Expanded);

    assert_eq!(panels[0].class, PanelAdaptiveClass::Compact);
    assert_eq!(panels[1].class, PanelAdaptiveClass::Medium);
    assert_eq!(panels[2].class, PanelAdaptiveClass::Wide);
}

#[test]
fn focus_a11y_page_models_focus_order_and_roles() {
    let controls = pages::focus_a11y::FOCUS_CONTROLS;
    let state = pages::focus_a11y::a11y_demo_state(3, true);

    assert_eq!(controls[0].id, "focus-primary");
    assert_eq!(controls[0].tab_index, 1);
    assert_eq!(controls[1].role, Role::SpinButton);
    assert_eq!(controls[2].role, Role::Switch);
    assert_eq!(state.counter, 3);
    assert_eq!(state.toggled, Toggled::True);
    assert_eq!(state.counter_role, Role::SpinButton);
}

#[test]
fn overlay_page_geometry_prefers_visual_bounds_and_insets_window() {
    let geometry = pages::overlay::demo_geometry();

    assert_eq!(geometry.anchor_rect.size.width, px(1.0));
    assert_eq!(geometry.anchor_rect.size.height, px(1.0));
    assert_eq!(geometry.preferred_rect, geometry.visual_rect);
    assert_eq!(geometry.safe_window_rect.origin.x, px(12.0));
    assert_eq!(geometry.safe_window_rect.origin.y, px(12.0));
    assert_eq!(geometry.safe_window_rect.size.width, px(616.0));
    assert_eq!(geometry.safe_window_rect.size.height, px(336.0));
}

#[test]
fn overlay_page_samples_expose_behavior_contracts() {
    let samples = pages::overlay::behavior_samples();

    assert_eq!(samples.len(), 4);
    assert_eq!(samples[0].id, "tooltip");
    assert_eq!(samples[0].policy.kind(), OverlayLayerKind::Tooltip);
    assert_eq!(
        samples[0].adapter.deferred_priority(),
        default_deferred_priority(OverlayLayerKind::Tooltip)
    );
    assert_eq!(
        samples[0].adapter.snap_margin(),
        DEFAULT_OVERLAY_SAFE_MARGIN
    );
    assert_eq!(
        samples[0].policy.outside_press_policy(),
        OutsidePressPolicy::Ignore
    );
    assert_eq!(
        samples[0].policy.escape_key_policy(),
        EscapeKeyPolicy::Ignore
    );
    assert_eq!(
        samples[0].policy.focus_restore_intent(),
        &FocusRestoreIntent::None
    );
    assert_eq!(
        samples[0].policy.initial_focus_intent(),
        &InitialFocusIntent::None
    );

    assert_eq!(samples[1].id, "popover");
    assert_eq!(
        samples[1].policy.kind(),
        OverlayLayerKind::NonModalDismissible
    );
    assert_eq!(
        samples[1].policy.outside_press_policy(),
        OutsidePressPolicy::DismissAndPassThrough
    );
    assert_eq!(
        samples[1].policy.focus_restore_intent(),
        &FocusRestoreIntent::Trigger
    );
    assert!(samples[1].policy.layer_state().wants_outside_press());
    assert!(samples[1].adapter.wants_outside_press_handler());

    assert_eq!(samples[2].id, "dialog");
    assert_eq!(samples[2].policy.kind(), OverlayLayerKind::Modal);
    assert_eq!(
        samples[2].policy.outside_press_policy(),
        OutsidePressPolicy::Consume
    );
    assert_eq!(
        samples[2].policy.initial_focus_intent(),
        &InitialFocusIntent::FirstFocusable
    );
    assert!(samples[2].policy.layer_state().blocks_underlay_input());
    assert_eq!(
        samples[2].adapter.deferred_priority(),
        default_deferred_priority(OverlayLayerKind::Modal)
    );

    assert_eq!(samples[3].id, "menu");
    assert_eq!(samples[3].policy.kind(), OverlayLayerKind::Menu);
    assert_eq!(
        samples[3].policy.outside_press_policy(),
        OutsidePressPolicy::DismissAndConsume
    );
    assert_eq!(
        pages::overlay::layer_kind_label(samples[3].policy.kind()),
        "menu"
    );
}

#[test]
fn components_page_samples_expose_component_metadata() {
    let tokens = ThemeTokens::default();
    let buttons = pages::components::button_samples(tokens);
    let badges = pages::components::badge_samples(tokens);
    let icon_buttons = pages::components::icon_button_samples(tokens);
    let switches = pages::components::switch_samples(tokens);
    let checkboxes = pages::components::checkbox_samples(tokens);
    let radio_groups = pages::components::radio_group_samples(tokens);
    let toggles = pages::components::toggle_samples(tokens);
    let labels = pages::components::label_samples(tokens);
    let text_inputs = pages::components::text_input_samples(tokens);
    let fields = pages::components::field_samples(tokens);

    assert_eq!(buttons.len(), 6);
    assert_eq!(buttons[0].id, "default");
    assert_eq!(buttons[0].state.role(), Role::Button);
    assert_eq!(
        buttons[3].state.colors().background().token(),
        semantic::DESTRUCTIVE
    );
    assert!(!buttons[5].state.activation_enabled());

    assert_eq!(badges.len(), 4);
    assert_eq!(badges[0].state.variant(), BadgeVariant::Default);
    assert!(badges[0].state.display_only());
    assert_eq!(badges[0].state.role(), None);
    assert_eq!(
        badges[2].state.colors().background().token(),
        semantic::DESTRUCTIVE
    );
    assert_eq!(badges[3].state.variant(), BadgeVariant::Outline);
    assert_eq!(badges[3].state.size(), Size::Small);

    assert_eq!(icon_buttons.len(), 4);
    assert_eq!(icon_buttons[0].accessible_label, "Search");
    assert_eq!(icon_buttons[0].state.role(), Role::Button);
    assert_eq!(icon_buttons[1].state.variant(), ButtonVariant::Outline);
    assert_eq!(
        icon_buttons[1].state.metrics().size(),
        Size::Small.icon_button_size()
    );
    assert_eq!(
        icon_buttons[2].state.colors().background().token(),
        semantic::DESTRUCTIVE
    );
    assert!(!icon_buttons[3].state.activation_enabled());

    assert_eq!(switches.len(), 4);
    assert_eq!(switches[0].state.role(), Role::Switch);
    assert_eq!(switches[0].state.toggled(), Toggled::False);
    assert_eq!(switches[1].state.toggled(), Toggled::True);
    assert!(!switches[3].state.activation_enabled());

    assert_eq!(checkboxes.len(), 6);
    assert_eq!(checkboxes[0].state.role(), Role::CheckBox);
    assert_eq!(checkboxes[0].state.toggled(), Toggled::False);
    assert_eq!(checkboxes[1].state.toggled(), Toggled::True);
    assert_eq!(checkboxes[2].state.toggled(), Toggled::Mixed);
    assert!(checkboxes[3].state.required());
    assert!(checkboxes[4].state.invalid());
    assert!(!checkboxes[5].state.activation_enabled());

    assert_eq!(radio_groups.len(), 2);
    assert_eq!(radio_groups[0].state.role(), Role::RadioGroup);
    assert!(radio_groups[0].state.required());
    assert_eq!(radio_groups[0].state.selected_value(), Some("team"));
    assert_eq!(radio_groups[0].state.focused_value(), Some("team"));
    assert_eq!(radio_groups[0].state.tab_stop_value(), Some("team"));
    assert!(radio_groups[0].state.items()[2].disabled());
    assert_eq!(radio_groups[0].state.items()[0].role(), Role::RadioButton);
    assert_eq!(radio_groups[1].state.orientation(), Orientation::Horizontal);
    assert_eq!(radio_groups[1].state.selected_value(), Some("europe"));

    assert_eq!(toggles.len(), 4);
    assert_eq!(toggles[0].state.role(), Role::Button);
    assert_eq!(toggles[0].state.toggled(), Toggled::False);
    assert_eq!(toggles[1].state.toggled(), Toggled::True);
    assert_eq!(toggles[2].state.variant(), ToggleVariant::Outline);
    assert!(!toggles[3].state.activation_enabled());

    assert_eq!(labels.len(), 4);
    assert_eq!(labels[0].state.role(), Role::Label);
    assert_eq!(labels[0].state.control_id(), Some("email-input"));
    assert!(labels[1].state.required());
    assert!(labels[2].state.disabled());
    assert!(!labels[3].state.associated());

    assert_eq!(text_inputs.len(), 5);
    assert_eq!(text_inputs[0].state.role(), Role::TextInput);
    assert!(text_inputs[0].controller_driven);
    assert!(
        text_inputs[1..]
            .iter()
            .all(|sample| !sample.controller_driven)
    );
    assert!(text_inputs[0].state.displaying_placeholder());
    assert!(text_inputs[1].state.has_value());
    assert_eq!(
        text_inputs[2].state.colors().border().token(),
        semantic::DESTRUCTIVE
    );
    assert!(!text_inputs[3].state.editable());
    assert!(!text_inputs[4].state.editable());

    assert_eq!(fields.len(), 3);
    assert!(fields[0].state.required());
    assert_eq!(
        fields[0].state.support_text().unwrap(),
        "Use a work address."
    );
    assert!(fields[1].state.support_is_error());
    assert_eq!(
        fields[1].state.support_text().unwrap(),
        "Enter a valid email."
    );
    assert!(fields[2].state.disabled());
    assert!(!fields[2].input_state.editable());
}

#[test]
fn components_page_tabs_samples_expose_roving_focus_contract() {
    let tokens = ThemeTokens::default();
    let tabs = pages::components::tabs_samples(tokens);

    assert_eq!(tabs.len(), 2);
    assert_eq!(tabs[0].id, "overview-tabs");
    assert_eq!(tabs[0].orientation, Orientation::Horizontal);
    assert_eq!(tabs[0].activation_mode, TabsActivationMode::Automatic);
    assert_eq!(tabs[0].state.selected_value(), Some("overview"));
    assert_eq!(tabs[0].state.focused_value(), Some("overview"));
    assert_eq!(tabs[0].state.tab_stop_value(), Some("overview"));
    assert!(tabs[0].items.iter().any(|item| item.disabled));

    assert_eq!(tabs[1].id, "workspace-tabs");
    assert_eq!(tabs[1].orientation, Orientation::Vertical);
    assert_eq!(tabs[1].activation_mode, TabsActivationMode::Manual);
    assert_eq!(tabs[1].items.len(), 7);
    assert_eq!(tabs[1].state.selected_value(), Some("profile"));
    assert_eq!(tabs[1].state.focused_value(), Some("profile"));
    assert_eq!(tabs[1].state.tab_stop_value(), Some("profile"));
    assert!(tabs[1].items[3].disabled);
}

#[test]
fn components_page_samples_keep_explicit_a11y_metadata() {
    use std::collections::BTreeSet;

    let tokens = ThemeTokens::default();
    let icon_buttons = pages::components::icon_button_samples(tokens);
    let labels = pages::components::label_samples(tokens);

    assert!(
        icon_buttons
            .iter()
            .all(|sample| !sample.accessible_label.trim().is_empty())
    );
    assert!(
        icon_buttons
            .iter()
            .all(|sample| sample.state.role() == Role::Button)
    );

    let control_ids = labels
        .iter()
        .filter_map(|sample| sample.state.control_id())
        .collect::<Vec<_>>();
    let unique_control_ids = control_ids.iter().copied().collect::<BTreeSet<_>>();

    assert_eq!(
        control_ids,
        vec!["email-input", "terms-checkbox", "disabled-control"]
    );
    assert_eq!(unique_control_ids.len(), control_ids.len());
    assert!(
        labels
            .iter()
            .filter(|sample| sample.id != "standalone")
            .all(|sample| sample.state.associated())
    );
    assert!(
        !labels
            .iter()
            .find(|sample| sample.id == "standalone")
            .unwrap()
            .state
            .associated()
    );
}
