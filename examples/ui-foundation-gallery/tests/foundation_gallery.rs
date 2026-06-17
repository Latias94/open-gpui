use open_gpui::px;
use open_gpui_ui_components::{
    AlertDialogIntent, AlertDialogOpenMode, BadgeVariant, ButtonVariant, ComboboxOpenMode,
    CommandOpenMode, DEFAULT_OVERLAY_SAFE_MARGIN, DialogOpenMode, HoverCardOpenIntent,
    HoverCardOpenMode, MenuItemKind, MenuOpenMode, PopoverOpenMode, ScrollAreaAxis,
    ScrollResetPolicy, SelectOpenMode, SheetCloseAffordance, SheetModalMode, SheetOpenMode,
    SheetSide, TabsActivationMode, ThemeMode, ToggleVariant, TooltipOpenIntent,
    default_deferred_priority,
};
use open_gpui_ui_core::{
    Density, DeviceAdaptiveClass, DeviceShellMode, EscapeKeyPolicy, FocusRestoreIntent,
    InitialFocusIntent, Orientation, OutsidePressPolicy, OverlayLayerKind,
    OverlayPlacementAlignment, OverlayPlacementSide, PanelAdaptiveClass, Role, Size, ThemeTokens,
    Toggled, semantic, ui_px,
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
fn productization_checkpoint_keeps_extraction_deferred_and_boundary_refs_available() {
    let workspace_manifest = include_str!("../../../Cargo.toml");
    let adr = include_str!("../../../docs/adr/0006-open-gpui-ui-headless-extraction-checkpoint.md");
    let design = include_str!("../../../docs/adr/0007-open-gpui-ui-headless-boundary-design.md");
    let productization =
        include_str!("../../../docs/adr/0008-open-gpui-ui-component-productization-roadmap.md");
    let component_contract = include_str!("../../../docs/ui/component-contract.md");

    assert!(!workspace_manifest.contains("open-gpui-ui-headless"));
    assert!(!workspace_manifest.contains("open_gpui_ui_headless"));
    assert!(productization.contains("Treat the current UI crates as the product boundary"));
    assert!(productization.contains(
        "Do not create a standalone `open-gpui-ui-headless` crate in the active roadmap."
    ));
    assert!(productization.contains("This ADR does not invalidate either document."));
    assert!(adr.contains("Do **not** create `open-gpui-ui-headless` yet."));
    assert!(adr.contains("The strict UI-core boundary is clean"));
    assert!(adr.contains("ListboxState"));
    assert!(adr.contains("ComboboxState"));
    assert!(adr.contains("CommandState"));
    assert!(adr.contains("GpuiOverlayState"));
    assert!(adr.contains("TextInputController"));
    assert!(adr.contains("ADR 0007 records that design gate."));
    assert!(!adr.contains("adaptive viewport `Pixels as Px`"));
    assert!(!adr.contains("UiPx` still has GPUI style-conversion impls"));
    assert!(design.contains("This ADR is a design gate only."));
    assert!(design.contains("It does not create `open-gpui-ui-headless`"));
    assert!(design.contains("overlay policy and placement vocabulary"));
    assert!(design.contains("roving-focus navigation helpers"));
    assert!(design.contains("listbox navigation and typeahead target resolution"));
    assert!(design.contains("scroll viewport intent"));
    assert!(design.contains("splitter resize constraints"));
    assert!(design.contains("AccessKit node wiring"));
    assert!(design.contains("TextInputController"));
    assert!(design.contains("focus_ring_shadow"));
    assert!(design.contains("Interaction Ownership Matrix"));
    assert!(
        component_contract
            .contains("ADR 0008 keeps current-crate productization as the active roadmap.")
    );
    assert!(component_contract.contains("ADR 0007 records the"));
    assert!(component_contract.contains("post-boundary extraction design without creating"));
    assert!(component_contract.contains("open_gpui_ui_components::gpui_adapter"));
    assert!(component_contract.contains("TextInputController"));
    assert!(component_contract.contains("ScrollHandle"));
    assert!(component_contract.contains("focus_ring_shadow"));
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
    assert_eq!(sizes[2].button_h, ui_px(32.0));
    assert_eq!(sizes[3].icon_button_size, ui_px(36.0));
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

    assert_eq!(geometry.anchor_rect.size.width, ui_px(1.0));
    assert_eq!(geometry.anchor_rect.size.height, ui_px(1.0));
    assert_eq!(geometry.preferred_rect, geometry.visual_rect);
    assert_eq!(geometry.safe_window_rect.origin.x, ui_px(12.0));
    assert_eq!(geometry.safe_window_rect.origin.y, ui_px(12.0));
    assert_eq!(geometry.safe_window_rect.size.width, ui_px(616.0));
    assert_eq!(geometry.safe_window_rect.size.height, ui_px(336.0));
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
fn overlay_page_tooltip_samples_expose_focus_hover_and_disabled_contracts() {
    let samples = pages::overlay::tooltip_samples(ThemeTokens::default());

    assert_eq!(samples.len(), 4);
    assert_eq!(samples[0].id, "hover-focus");
    assert_eq!(
        samples[0].state.open_intent(),
        TooltipOpenIntent::HoverOrFocus
    );
    assert!(samples[0].state.open_intent().opens_on_hover());
    assert!(samples[0].state.open_intent().opens_on_focus());
    assert!(!samples[0].state.open());
    assert_eq!(
        samples[0].state.overlay().policy().kind(),
        OverlayLayerKind::Tooltip
    );

    assert_eq!(samples[1].id, "focus-only");
    assert_eq!(samples[1].state.open_intent(), TooltipOpenIntent::Focus);
    assert!(!samples[1].state.open_intent().opens_on_hover());
    assert!(samples[1].state.open_intent().opens_on_focus());

    assert_eq!(samples[2].id, "delayed-manual");
    assert_eq!(samples[2].state.open_intent(), TooltipOpenIntent::Manual);
    assert!(samples[2].state.open());
    assert_eq!(
        samples[2].state.delay().open_delay(),
        std::time::Duration::from_millis(120)
    );
    assert_eq!(
        samples[2].state.delay().close_delay(),
        std::time::Duration::from_millis(40)
    );

    assert_eq!(samples[3].id, "disabled");
    assert!(samples[3].state.disabled());
    assert!(!samples[3].state.open());
    assert!(samples[3].state.descriptive());
    assert!(!samples[3].state.interactive_content());
}

#[test]
fn overlay_page_hover_card_samples_expose_interactive_hover_contracts() {
    let samples = pages::overlay::hover_card_samples(ThemeTokens::default());

    assert_eq!(samples.len(), 3);
    assert_eq!(samples[0].id, "profile-preview");
    assert_eq!(
        samples[0].state.open_mode(),
        HoverCardOpenMode::Uncontrolled
    );
    assert!(samples[0].state.default_open());
    assert!(samples[0].state.open());
    assert_eq!(
        samples[0].state.open_intent(),
        HoverCardOpenIntent::HoverOrFocus
    );
    assert!(samples[0].state.open_intent().opens_on_hover());
    assert!(samples[0].state.open_intent().opens_on_focus());
    assert!(samples[0].state.interactive_content());
    assert!(!samples[0].state.descriptive());
    assert_eq!(
        samples[0].state.overlay().policy().kind(),
        OverlayLayerKind::NonModalDismissible
    );
    assert!(samples[0].state.overlay().wants_outside_press_handler());
    assert_eq!(
        samples[0].state.focus_restore_intent(),
        &FocusRestoreIntent::None
    );

    assert_eq!(samples[1].id, "focus-preview");
    assert_eq!(samples[1].state.open_intent(), HoverCardOpenIntent::Focus);
    assert!(!samples[1].state.open_intent().opens_on_hover());
    assert!(samples[1].state.open_intent().opens_on_focus());
    assert_eq!(
        samples[1].state.placement_side(),
        OverlayPlacementSide::Right
    );

    assert_eq!(samples[2].id, "manual-controlled");
    assert_eq!(samples[2].state.open_mode(), HoverCardOpenMode::Controlled);
    assert_eq!(samples[2].state.open_intent(), HoverCardOpenIntent::Manual);
    assert!(!samples[2].state.open());
    assert_eq!(samples[2].state.delay().open_delay().as_millis(), 80);
    assert_eq!(
        samples[2].state.outside_press_policy(),
        OutsidePressPolicy::DismissAndConsume
    );
}

#[test]
fn overlay_page_popover_samples_expose_controlled_and_dismissal_contracts() {
    let samples = pages::overlay::popover_samples(ThemeTokens::default());

    assert_eq!(samples.len(), 4);
    assert_eq!(samples[0].id, "default-open");
    assert_eq!(samples[0].state.open_mode(), PopoverOpenMode::Uncontrolled);
    assert!(samples[0].state.default_open());
    assert!(samples[0].state.open());
    assert_eq!(
        samples[0].state.overlay().policy().kind(),
        OverlayLayerKind::NonModalDismissible
    );
    assert_eq!(
        samples[0].state.focus_restore_intent(),
        &FocusRestoreIntent::Trigger
    );

    assert_eq!(samples[1].id, "controlled");
    assert_eq!(samples[1].state.open_mode(), PopoverOpenMode::Controlled);
    assert!(!samples[1].state.open());
    assert_eq!(
        samples[1].state.placement_side(),
        open_gpui_ui_core::OverlayPlacementSide::Right
    );

    assert_eq!(samples[2].id, "consume-outside");
    assert!(samples[2].state.open());
    assert_eq!(
        samples[2].state.outside_press_policy(),
        OutsidePressPolicy::DismissAndConsume
    );
    assert!(
        !samples[2]
            .state
            .outside_press_policy()
            .resolve()
            .allows_underlay_dispatch()
    );

    assert_eq!(samples[3].id, "disabled");
    assert!(samples[3].state.disabled());
    assert!(!samples[3].state.open());
    assert!(!samples[3].state.activation_enabled());
}

#[test]
fn overlay_page_dialog_samples_expose_modal_and_close_contracts() {
    let samples = pages::overlay::dialog_samples(ThemeTokens::default());

    assert_eq!(samples.len(), 4);
    assert_eq!(samples[0].id, "controlled-modal");
    assert_eq!(samples[0].state.open_mode(), DialogOpenMode::Controlled);
    assert!(!samples[0].state.open());
    assert_eq!(samples[0].state.title(), "Controlled dialog");
    assert_eq!(
        samples[0].state.description(),
        Some("Escape and the modal barrier can close it.")
    );
    assert_eq!(
        samples[0].state.overlay().policy().kind(),
        OverlayLayerKind::Modal
    );
    assert_eq!(
        samples[0].state.outside_press_policy(),
        OutsidePressPolicy::DismissAndConsume
    );
    assert!(
        !samples[0]
            .state
            .overlay()
            .policy()
            .layer_state()
            .blocks_underlay_input()
    );

    assert_eq!(samples[1].id, "default-open");
    assert_eq!(samples[1].state.open_mode(), DialogOpenMode::Uncontrolled);
    assert!(samples[1].state.default_open());
    assert!(samples[1].state.open());
    assert_eq!(
        samples[1].state.escape_key_policy(),
        EscapeKeyPolicy::Dismiss
    );
    assert!(
        samples[1]
            .state
            .overlay()
            .policy()
            .layer_state()
            .blocks_underlay_input()
    );

    assert_eq!(samples[2].id, "outside-ignore");
    assert!(samples[2].state.open());
    assert_eq!(
        samples[2].state.outside_press_policy(),
        OutsidePressPolicy::Ignore
    );
    assert!(
        !samples[2]
            .state
            .outside_press_policy()
            .resolve()
            .dismisses()
    );

    assert_eq!(samples[3].id, "disabled");
    assert!(samples[3].state.disabled());
    assert!(!samples[3].state.open());
    assert!(!samples[3].state.activation_enabled());
}

#[test]
fn overlay_page_alert_dialog_samples_expose_critical_action_contracts() {
    let samples = pages::overlay::alert_dialog_samples(ThemeTokens::default());

    assert_eq!(samples.len(), 2);
    assert_eq!(samples[0].id, "destructive-confirm");
    assert_eq!(
        samples[0].state.open_mode(),
        AlertDialogOpenMode::Controlled
    );
    assert!(!samples[0].state.open());
    assert_eq!(samples[0].state.intent(), AlertDialogIntent::Destructive);
    assert_eq!(samples[0].state.content_role(), Role::AlertDialog);
    assert_eq!(samples[0].state.action().label(), "Delete");
    assert_eq!(samples[0].state.cancel().label(), "Keep project");
    assert!(samples[0].state.cancel().default_focus());
    assert_eq!(
        samples[0].state.outside_press_policy(),
        OutsidePressPolicy::Consume
    );
    assert!(
        !samples[0]
            .state
            .outside_press_policy()
            .resolve()
            .dismisses()
    );
    assert_eq!(
        samples[0].state.overlay().policy().kind(),
        OverlayLayerKind::Modal
    );

    assert_eq!(samples[1].id, "safe-cancel");
    assert_eq!(
        samples[1].state.open_mode(),
        AlertDialogOpenMode::Uncontrolled
    );
    assert!(samples[1].state.default_open());
    assert!(samples[1].state.open());
    assert_eq!(samples[1].state.intent(), AlertDialogIntent::Default);
    assert_eq!(samples[1].state.action().label(), "Archive");
    assert!(
        samples[1]
            .state
            .overlay()
            .layer_state()
            .blocks_underlay_input()
    );
    assert_eq!(
        samples[1].state.focus_restore_intent(),
        &FocusRestoreIntent::Trigger
    );
}

#[test]
fn overlay_page_sheet_samples_expose_edge_and_policy_contracts() {
    let samples = pages::overlay::sheet_samples(ThemeTokens::default());

    assert_eq!(samples.len(), 3);
    assert_eq!(samples[0].id, "left-modal");
    assert_eq!(samples[0].state.open_mode(), SheetOpenMode::Uncontrolled);
    assert!(samples[0].state.default_open());
    assert!(samples[0].state.open());
    assert_eq!(samples[0].state.side(), SheetSide::Left);
    assert_eq!(samples[0].state.modal_mode(), SheetModalMode::Modal);
    assert_eq!(
        samples[0].state.close_affordance(),
        SheetCloseAffordance::Visible
    );
    assert_eq!(
        samples[0].state.outside_press_policy(),
        OutsidePressPolicy::DismissAndConsume
    );
    assert!(
        samples[0]
            .state
            .overlay()
            .layer_state()
            .blocks_underlay_input()
    );

    assert_eq!(samples[1].id, "right-non-modal");
    assert_eq!(samples[1].state.open_mode(), SheetOpenMode::Controlled);
    assert!(!samples[1].state.open());
    assert_eq!(samples[1].state.side(), SheetSide::Right);
    assert_eq!(samples[1].state.modal_mode(), SheetModalMode::NonModal);
    assert_eq!(
        samples[1].state.overlay().policy().kind(),
        OverlayLayerKind::NonModalDismissible
    );
    assert!(
        !samples[1]
            .state
            .overlay()
            .layer_state()
            .blocks_underlay_input()
    );
    assert_eq!(
        samples[1].state.outside_press_policy(),
        OutsidePressPolicy::DismissAndPassThrough
    );
    assert!(
        samples[1]
            .state
            .outside_press_policy()
            .resolve()
            .allows_underlay_dispatch()
    );

    assert_eq!(samples[2].id, "bottom-sticky");
    assert_eq!(samples[2].state.side(), SheetSide::Bottom);
    assert_eq!(
        samples[2].state.close_affordance(),
        SheetCloseAffordance::Hidden
    );
    assert_eq!(
        samples[2].state.outside_press_policy(),
        OutsidePressPolicy::Ignore
    );
    assert!(!samples[2].state.overlay().wants_outside_press_handler());
}

#[test]
fn overlay_page_menu_samples_expose_roving_focus_and_dismiss_contracts() {
    let samples = pages::overlay::menu_samples(ThemeTokens::default());

    assert_eq!(samples.len(), 4);
    assert_eq!(samples[0].id, "default-open");
    assert_eq!(samples[0].state.open_mode(), MenuOpenMode::Uncontrolled);
    assert!(samples[0].state.default_open());
    assert!(samples[0].state.open());
    assert_eq!(samples[0].state.focused_value(), Some("save"));
    assert_eq!(samples[0].state.items()[2].kind(), MenuItemKind::Separator);
    assert!(!samples[0].state.items()[3].activation_enabled());
    assert_eq!(
        samples[0].state.overlay().policy().kind(),
        OverlayLayerKind::Menu
    );
    assert!(samples[0].state.overlay().wants_outside_press_handler());

    assert_eq!(samples[1].id, "controlled");
    assert_eq!(samples[1].state.open_mode(), MenuOpenMode::Controlled);
    assert!(!samples[1].state.open());
    assert_eq!(
        samples[1].state.outside_press_policy(),
        OutsidePressPolicy::DismissAndConsume
    );
    assert_eq!(
        samples[1].state.escape_key_policy(),
        EscapeKeyPolicy::Dismiss
    );

    assert_eq!(samples[2].id, "outside-ignore");
    assert!(samples[2].state.open());
    assert_eq!(
        samples[2].state.outside_press_policy(),
        OutsidePressPolicy::Ignore
    );
    assert!(
        !samples[2]
            .state
            .outside_press_policy()
            .resolve()
            .dismisses()
    );

    assert_eq!(samples[3].id, "disabled");
    assert!(samples[3].state.disabled());
    assert!(!samples[3].state.open());
}

#[test]
fn overlay_page_context_menu_samples_expose_point_anchor_contracts() {
    let samples = pages::overlay::context_menu_samples(ThemeTokens::default());

    assert_eq!(samples.len(), 3);
    assert_eq!(samples[0].id, "point-anchor");
    assert!(samples[0].state.open());
    assert_eq!(samples[0].state.open_mode(), MenuOpenMode::Controlled);
    assert_eq!(samples[0].state.menu().focused_value(), Some("duplicate"));
    assert_eq!(
        samples[0].state.menu().items()[1].kind(),
        MenuItemKind::Separator
    );
    assert!(!samples[0].state.menu().items()[2].activation_enabled());
    assert_eq!(
        samples[0].state.overlay().policy().kind(),
        OverlayLayerKind::Menu
    );
    assert_eq!(
        samples[0].state.placement_input().side(),
        OverlayPlacementSide::Bottom
    );
    assert_eq!(
        samples[0].state.placement_input().alignment(),
        OverlayPlacementAlignment::Start
    );
    assert_eq!(samples[0].state.placement_input().safe_bounds(), None);

    assert_eq!(samples[1].id, "controlled");
    assert!(!samples[1].state.open());
    assert_eq!(samples[1].state.open_mode(), MenuOpenMode::Controlled);

    assert_eq!(samples[2].id, "default-open");
    assert!(samples[2].state.default_open());
    assert!(samples[2].state.open());
    assert_eq!(samples[2].state.open_mode(), MenuOpenMode::Uncontrolled);
}

#[test]
fn components_page_samples_expose_component_metadata() {
    let tokens = ThemeTokens::default();
    let gates = pages::components::CONFORMANCE_GATES;
    let buttons = pages::components::button_samples(tokens);
    let badges = pages::components::badge_samples(tokens);
    let icon_buttons = pages::components::icon_button_samples(tokens);
    let switches = pages::components::switch_samples(tokens);
    let checkboxes = pages::components::checkbox_samples(tokens);
    let radio_groups = pages::components::radio_group_samples(tokens);
    let toggles = pages::components::toggle_samples(tokens);
    let toolbars = pages::components::toolbar_samples(tokens);
    let sidebars = pages::components::sidebar_samples(tokens);
    let listboxes = pages::components::listbox_samples(tokens);
    let selects = pages::components::select_samples(tokens);
    let comboboxes = pages::components::combobox_samples(tokens);
    let commands = pages::components::command_samples(tokens);
    let labels = pages::components::label_samples(tokens);
    let text_inputs = pages::components::text_input_samples(tokens);
    let fields = pages::components::field_samples(tokens);
    let scroll_areas = pages::components::scroll_area_samples(tokens);
    let splitters = pages::components::splitter_samples(tokens);

    assert_eq!(gates.len(), 6);
    assert_eq!(gates[0].id, "public-api-exports");
    assert!(
        gates[0]
            .evidence
            .contains(&"crates/ui_components/src/lib.rs")
    );
    assert_eq!(gates[1].id, "gallery-metadata");
    assert_eq!(gates[2].id, "scroll-redraw");
    assert!(
        gates[2]
            .evidence
            .contains(&"scroll_area_default_handle_survives_reconstructed_component_values")
    );
    assert_eq!(gates[3].id, "splitter-runtime");
    assert_eq!(gates[4].id, "tabs-overflow");
    assert_eq!(gates[5].id, "a11y-labels");

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

    assert_eq!(toolbars.len(), 2);
    assert_eq!(toolbars[0].id, "editor-toolbar");
    assert_eq!(toolbars[0].state.role(), Role::Toolbar);
    assert_eq!(toolbars[0].state.orientation(), Orientation::Horizontal);
    assert_eq!(toolbars[0].state.focused_value(), Some("bold"));
    assert_eq!(toolbars[0].state.tab_stop_value(), Some("bold"));
    assert_eq!(toolbars[0].state.items()[2].kind().as_str(), "separator");
    assert!(!toolbars[0].state.items()[2].focusable());
    assert!(toolbars[0].state.items()[3].pressed());
    assert_eq!(toolbars[1].state.orientation(), Orientation::Vertical);

    assert_eq!(sidebars.len(), 3);
    assert_eq!(sidebars[0].id, "workspace-sidebar");
    assert_eq!(sidebars[0].state.role(), Role::Navigation);
    assert_eq!(sidebars[0].state.side().as_str(), "left");
    assert_eq!(sidebars[0].state.variant().as_str(), "docked");
    assert_eq!(sidebars[0].state.collapse_mode().as_str(), "icon");
    assert_eq!(sidebars[0].state.selected_value(), Some("projects"));
    assert_eq!(sidebars[0].state.focused_value(), Some("projects"));
    assert_eq!(sidebars[0].state.sections()[0].role(), Role::Section);
    assert_eq!(sidebars[0].state.items()[1].badge_label(), Some("12"));
    assert!(!sidebars[0].state.items()[3].activation_enabled());
    assert!(sidebars[1].state.icon_collapsed());
    assert!(!sidebars[1].state.items()[0].text_visible());
    assert_eq!(sidebars[1].state.items()[0].label(), "Home");
    assert_eq!(sidebars[2].state.side().as_str(), "right");
    assert!(sidebars[2].state.scrollable());
    assert!(sidebars[2].state.items().len() > 8);

    assert_eq!(listboxes.len(), 2);
    assert_eq!(listboxes[0].id, "assignee-listbox");
    assert_eq!(listboxes[0].state.role(), Role::ListBox);
    assert_eq!(listboxes[0].state.selected_value(), Some("owen"));
    assert_eq!(listboxes[0].state.active_value(), Some("maya"));
    assert_eq!(
        listboxes[0].state.options()[0].role(),
        Some(Role::ListBoxOption)
    );
    assert!(
        listboxes[0]
            .state
            .options()
            .iter()
            .any(|option| option.disabled())
    );
    assert!(listboxes[1].state.empty());
    assert_eq!(listboxes[1].state.tab_stop_value(), None);

    assert_eq!(selects.len(), 3);
    assert!(
        selects.iter().all(|sample| !sample.interactive_open),
        "select samples keep popup state metadata separate from default gallery interactivity"
    );
    assert_eq!(selects[0].id, "priority-select");
    assert_eq!(selects[0].state.open_mode(), SelectOpenMode::Controlled);
    assert!(selects[0].state.open());
    assert_eq!(selects[0].state.trigger_role(), Role::Button);
    assert_eq!(selects[0].state.content_role(), Role::ListBox);
    assert!(selects[0].state.scrollable_content());
    assert_eq!(selects[1].state.trigger_label(), "Doing");
    assert!(selects[2].state.disabled());
    assert!(!selects[2].state.open());

    assert_eq!(comboboxes.len(), 3);
    assert!(
        comboboxes.iter().all(|sample| !sample.interactive_open),
        "combobox samples should not mount popups open on page load"
    );
    assert_eq!(comboboxes[0].id, "framework-combobox");
    assert_eq!(
        comboboxes[0].state.open_mode(),
        ComboboxOpenMode::Controlled
    );
    assert!(comboboxes[0].state.open());
    assert_eq!(comboboxes[0].state.input_role(), Role::EditableComboBox);
    assert_eq!(comboboxes[0].state.content_role(), Role::ListBox);
    assert_eq!(comboboxes[0].state.filtered_option_count(), 3);
    assert_eq!(comboboxes[0].state.selected_value(), Some("solid"));
    assert_eq!(comboboxes[0].state.listbox().selected_value(), None);
    assert_eq!(comboboxes[1].state.filtered_option_count(), 0);
    assert!(comboboxes[1].state.listbox().empty());
    assert!(comboboxes[2].state.disabled());
    assert!(!comboboxes[2].state.open());

    assert_eq!(commands.len(), 3);
    assert!(
        commands.iter().all(|sample| !sample.interactive_open),
        "command samples should not mount popups open on page load"
    );
    assert_eq!(commands[0].id, "workspace-command");
    assert_eq!(commands[0].state.open_mode(), CommandOpenMode::Controlled);
    assert!(commands[0].state.open());
    assert!(commands[0].state.dialog().is_some());
    assert_eq!(commands[0].state.list_role(), Role::ListBox);
    assert_eq!(commands[0].state.selected_value(), Some("new-file"));
    assert_eq!(commands[0].state.filtered_item_count(), 2);
    assert!(
        commands[0]
            .state
            .items()
            .iter()
            .any(|item| item.shortcut().is_some())
    );
    assert!(commands[1].state.loading().is_some());
    assert!(commands[1].state.empty());
    assert!(commands[2].state.disabled());
    assert!(!commands[2].state.open());

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

    assert_eq!(scroll_areas.len(), 3);
    assert_eq!(scroll_areas[0].id, "activity-log");
    assert_eq!(scroll_areas[0].state.axis(), ScrollAreaAxis::Vertical);
    assert_eq!(
        scroll_areas[0].state.reset_policy(),
        ScrollResetPolicy::Preserve
    );
    assert_eq!(
        scroll_areas[0].state.metrics().scrollbar_width(),
        ui_px(10.0)
    );
    assert_eq!(scroll_areas[1].state.axis(), ScrollAreaAxis::Horizontal);
    assert_eq!(scroll_areas[1].state.reset_key(), None);
    assert_eq!(scroll_areas[2].state.axis(), ScrollAreaAxis::Both);
    assert_eq!(
        scroll_areas[2].state.reset_policy(),
        ScrollResetPolicy::ResetOnKeyChange
    );
    assert_eq!(scroll_areas[2].state.reset_key(), Some("components"));

    assert_eq!(splitters.len(), 2);
    assert_eq!(splitters[0].id, "workspace-split");
    assert_eq!(splitters[0].state.orientation(), Orientation::Horizontal);
    assert_eq!(splitters[0].state.panels().len(), 3);
    assert_eq!(splitters[0].state.handles().len(), 2);
    assert_eq!(splitters[0].state.panels()[0].id(), "navigator");
    assert!(!splitters[0].state.handles()[0].disabled());
    assert_eq!(splitters[1].state.orientation(), Orientation::Vertical);
    assert!(splitters[1].state.panels()[0].collapsed());
    assert_eq!(splitters[1].state.panels()[0].collapsed_fraction(), 0.08);
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
fn components_page_sidebar_samples_expose_navigation_contract() {
    let samples = pages::components::sidebar_samples(ThemeTokens::default());
    let workspace = &samples[0].state;
    let icon = &samples[1].state;
    let long = &samples[2].state;

    assert_eq!(workspace.role(), Role::Navigation);
    assert_eq!(workspace.selected_value(), Some("projects"));
    assert_eq!(workspace.focused_value(), Some("projects"));
    assert_eq!(
        workspace.navigation_target("down").map(|item| item.value()),
        Some("inbox")
    );
    assert_eq!(
        workspace
            .activation_for_key("enter")
            .map(|selection| selection.value().to_owned()),
        Some("projects".to_string())
    );
    assert!(workspace.items().iter().any(|item| item.disabled()));

    assert!(icon.icon_collapsed());
    assert_eq!(
        icon.metrics().resolved_width(),
        icon.metrics().collapsed_width()
    );
    assert!(icon.items().iter().all(|item| !item.text_visible()));
    assert!(icon.items().iter().all(|item| !item.label().is_empty()));

    assert_eq!(long.side().as_str(), "right");
    assert_eq!(long.focused_value(), Some("quality"));
    assert_eq!(
        long.navigation_target("down").map(|item| item.value()),
        Some("alerts")
    );
    assert!(long.scrollable());
}

#[test]
fn components_page_toolbar_samples_expose_roving_focus_contract() {
    let tokens = ThemeTokens::default();
    let samples = pages::components::toolbar_samples(tokens);
    let editor = &samples[0].state;
    let inspector = &samples[1].state;

    assert_eq!(editor.role(), Role::Toolbar);
    assert_eq!(editor.focused_value(), Some("bold"));
    assert_eq!(
        editor.navigation_target("right").map(|item| item.value()),
        Some("italic")
    );
    assert_eq!(
        editor
            .activation_for_key("space")
            .map(|selection| selection.value().to_owned()),
        Some("bold".to_string())
    );
    assert_eq!(editor.items()[2].role(), None);
    assert_eq!(editor.items()[3].toggled(), Some(Toggled::True));
    assert_eq!(inspector.orientation(), Orientation::Vertical);
    assert_eq!(
        inspector.navigation_target("down").map(|item| item.value()),
        Some("refresh")
    );
}

#[test]
fn components_page_choice_samples_expose_listbox_and_select_contracts() {
    let tokens = ThemeTokens::default();
    let listboxes = pages::components::listbox_samples(tokens);
    let selects = pages::components::select_samples(tokens);
    let assignee = &listboxes[0].state;
    let empty = &listboxes[1].state;
    let priority = &selects[0].state;
    let status = &selects[1].state;
    let disabled = &selects[2].state;

    assert_eq!(assignee.role(), Role::ListBox);
    assert_eq!(
        assignee
            .navigation_target("down")
            .map(|option| option.value()),
        Some("owen")
    );
    assert_eq!(
        assignee
            .activation_for_key("enter")
            .map(|selection| selection.value().to_owned()),
        Some("maya".to_string())
    );
    assert_eq!(
        assignee.typeahead_target("no").map(|option| option.value()),
        Some("nora")
    );
    assert!(assignee.options().iter().any(|option| !option.focusable()));

    assert!(empty.empty());
    assert_eq!(empty.active_value(), None);
    assert_eq!(empty.tab_stop_value(), None);

    assert_eq!(priority.open_mode(), SelectOpenMode::Controlled);
    assert!(priority.open());
    assert_eq!(priority.selected_value(), Some("critical"));
    assert_eq!(priority.trigger_label(), "Critical");
    assert_eq!(
        priority.overlay().policy().kind(),
        OverlayLayerKind::NonModalDismissible
    );
    assert_eq!(
        priority.outside_press_policy(),
        OutsidePressPolicy::DismissAndConsume
    );
    assert_eq!(
        priority.initial_focus_intent(),
        &InitialFocusIntent::FirstFocusable
    );
    assert_eq!(
        priority.focus_restore_intent(),
        &FocusRestoreIntent::Trigger
    );
    assert_eq!(priority.listbox().role(), Role::ListBox);
    assert!(priority.scrollable_content());
    assert!(priority.scroll_area().scrolls_y());

    assert_eq!(status.open_mode(), SelectOpenMode::Uncontrolled);
    assert!(!status.open());
    assert_eq!(status.trigger_label(), "Doing");
    assert_eq!(disabled.trigger_label(), "Unavailable");
    assert!(disabled.disabled());
    assert!(!disabled.overlay().should_render_deferred_layer());
}

#[test]
fn components_page_search_samples_expose_combobox_and_command_contracts() {
    let tokens = ThemeTokens::default();
    let comboboxes = pages::components::combobox_samples(tokens);
    let commands = pages::components::command_samples(tokens);

    let framework = &comboboxes[0].state;
    let empty_combo = &comboboxes[1].state;
    let disabled_combo = &comboboxes[2].state;
    let workspace = &commands[0].state;
    let empty_command = &commands[1].state;
    let disabled_command = &commands[2].state;

    assert_eq!(framework.open_mode(), ComboboxOpenMode::Controlled);
    assert!(framework.open());
    assert_eq!(framework.input_role(), Role::EditableComboBox);
    assert_eq!(framework.content_role(), Role::ListBox);
    assert_eq!(framework.total_option_count(), 5);
    assert_eq!(framework.filtered_option_count(), 3);
    assert_eq!(framework.selected_value(), Some("solid"));
    assert_eq!(framework.selected_label(), Some("Solid"));
    assert_eq!(framework.listbox().selected_value(), None);
    assert_eq!(framework.active_value(), Some("react"));
    assert_eq!(framework.listbox().typeahead_query(), Some("re"));
    assert_eq!(
        framework.overlay().policy().kind(),
        OverlayLayerKind::NonModalDismissible
    );

    assert!(empty_combo.empty());
    assert_eq!(empty_combo.selected_value(), None);
    assert!(empty_combo.listbox().empty());
    assert!(disabled_combo.disabled());
    assert!(!disabled_combo.open());
    assert!(!disabled_combo.input().editable());

    assert_eq!(workspace.open_mode(), CommandOpenMode::Controlled);
    assert!(workspace.open());
    assert_eq!(workspace.input_role(), Role::TextInput);
    assert_eq!(workspace.list_role(), Role::ListBox);
    assert_eq!(workspace.selected_value(), Some("new-file"));
    assert_eq!(workspace.filtered_item_count(), 2);
    assert_eq!(workspace.groups().len(), 2);
    assert!(
        workspace
            .items()
            .iter()
            .any(|item| item.shortcut().is_some())
    );
    let dialog = workspace
        .dialog()
        .expect("workspace command is dialog-backed");
    assert!(dialog.open());
    assert_eq!(dialog.overlay().policy().kind(), OverlayLayerKind::Modal);
    assert_eq!(dialog.description(), Some("Run a workspace command"));

    assert!(empty_command.loading().is_some());
    assert_eq!(
        empty_command.loading().unwrap().role(),
        Role::ProgressIndicator
    );
    assert!(empty_command.empty());
    assert!(empty_command.content_visible());
    assert!(disabled_command.disabled());
    assert!(!disabled_command.open());
    assert!(!disabled_command.input().editable());
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

#[test]
fn components_page_conformance_gates_reference_core_and_gallery_contracts() {
    let gates = pages::components::CONFORMANCE_GATES;
    let signals = pages::components::SIGNALS;

    assert!(gates.iter().all(|gate| !gate.title.trim().is_empty()));
    assert!(gates.iter().all(|gate| !gate.summary.trim().is_empty()));
    assert!(
        gates
            .iter()
            .all(|gate| gate.evidence.iter().all(|item| !item.trim().is_empty()))
    );
    assert!(gates.iter().any(|gate| gate.id == "scroll-redraw"));
    assert!(gates.iter().any(|gate| gate.id == "tabs-overflow"));
    assert!(signals.contains(&"open_gpui_ui_components::Listbox"));
    assert!(signals.contains(&"open_gpui_ui_components::ListboxState"));
    assert!(signals.contains(&"open_gpui_ui_components::Select"));
    assert!(signals.contains(&"open_gpui_ui_components::SelectState"));
    assert!(signals.contains(&"open_gpui_ui_components::Combobox"));
    assert!(signals.contains(&"open_gpui_ui_components::ComboboxState"));
    assert!(signals.contains(&"open_gpui_ui_components::Command"));
    assert!(signals.contains(&"open_gpui_ui_components::CommandState"));
    assert!(signals.contains(&"Role::ListBox"));
    assert!(signals.contains(&"Role::ListBoxOption"));
    assert!(signals.contains(&"Role::EditableComboBox"));
    assert!(signals.contains(&"Role::ProgressIndicator"));
}
