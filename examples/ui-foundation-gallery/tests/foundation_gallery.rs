use open_gpui::px;
use open_gpui_ui_core::{
    Density, DeviceAdaptiveClass, DeviceShellMode, PanelAdaptiveClass, Role, Size, ThemeTokens,
    Toggled, semantic,
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
            "overlay"
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
fn package_manifest_stays_pure_foundation() {
    let manifest = include_str!("../Cargo.toml");

    assert!(manifest.contains("open_gpui.workspace = true"));
    assert!(manifest.contains("open_gpui_ui_core.workspace = true"));
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
    assert_eq!(samples[7].key, semantic::FOCUS_RING);
    assert_eq!(samples[11].key, semantic::MODAL_OVERLAY);
    assert!(pages::tokens::matches_semantic_registry(tokens));
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
