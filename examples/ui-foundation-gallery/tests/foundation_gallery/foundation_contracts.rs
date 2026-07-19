use super::*;

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
            "presentation",
            "components",
            "devtools"
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
    assert_eq!(
        GalleryPage::from_id("devtools"),
        Some(GalleryPage::Devtools)
    );
    assert_eq!(
        GalleryPage::from_id("presentation"),
        Some(GalleryPage::Presentation)
    );
    assert_eq!(GalleryPage::from_id("missing"), None);
    assert_eq!(DeviceShellMode::Desktop.as_str(), "desktop");
    assert_eq!(DeviceShellMode::Mobile.as_str(), "mobile");
    assert_eq!(Density::Spacious.as_str(), "spacious");
    assert_eq!(Size::XSmall.as_str(), "xs");
    assert_eq!(DeviceAdaptiveClass::Expanded.as_str(), "expanded device");
    assert_eq!(PanelAdaptiveClass::Wide.as_str(), "wide panel");
}

#[test]
fn package_manifest_stays_foundation_scoped() {
    let manifest = include_str!("../../Cargo.toml");

    assert!(manifest.contains("open_gpui.workspace = true"));
    assert!(manifest.contains("open_gpui_devtools = { workspace = true"));
    assert!(manifest.contains(r#"features = ["gpui", "form", "resource", "motion", "command"]"#));
    assert!(manifest.contains("open_gpui_form.workspace = true"));
    assert!(manifest.contains("open_gpui_resource.workspace = true"));
    assert!(manifest.contains("open_gpui_ui_core.workspace = true"));
    assert!(manifest.contains("open_gpui_ui_components.workspace = true"));
    assert!(manifest.contains("open_gpui_platform = { workspace = true"));
    assert!(manifest.contains("font-kit"));
    assert!(!manifest.contains("open_gpui_canvas"));
    assert!(!manifest.contains("open_gpui_docking"));
    assert!(!manifest.contains("open_gpui_ui ="));
}

#[test]
fn productization_checkpoint_keeps_extraction_deferred_and_boundary_refs_available() {
    let workspace_manifest = include_str!("../../../../Cargo.toml");
    let adr =
        include_str!("../../../../docs/adr/0006-open-gpui-ui-headless-extraction-checkpoint.md");
    let design = include_str!("../../../../docs/adr/0007-open-gpui-ui-headless-boundary-design.md");
    let productization =
        include_str!("../../../../docs/adr/0008-open-gpui-ui-component-productization-roadmap.md");
    let component_contract = include_str!("../../../../docs/ui/component-contract.md");

    assert!(!workspace_manifest.contains("open-gpui-ui-headless"));
    assert!(!workspace_manifest.contains("open_gpui_ui_headless"));
    assert!(productization.contains("Treat the current UI crates as the product boundary"));
    assert!(productization.contains(
        "Do not create a standalone `open-gpui-ui-headless` crate in the active roadmap."
    ));
    assert!(productization.contains("This ADR does not invalidate either document."));
    assert!(adr.contains("Do **not** create `open-gpui-ui-headless` yet"));
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
    assert!(design.contains("`focus_ring_shadow`"));
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
    assert!(component_contract.contains("focus_ring_shadow_with_theme"));
    assert!(!component_contract.contains("focus_ring_shadow("));
}

#[test]
fn token_page_samples_follow_theme_token_order() {
    let tokens = ThemeTokens::default();
    let light = open_gpui_ui_components::theme::ThemeContext::light();
    let samples = pages::tokens::token_samples_for_theme(tokens, &light);

    assert_eq!(samples.len(), 12);
    assert_eq!(samples[0].key, semantic::SURFACE);
    assert_eq!(samples[0].preview_rgb, 0xffffff);
    assert_eq!(samples[7].key, semantic::FOCUS_RING);
    assert_eq!(samples[11].key, semantic::MODAL_OVERLAY);
    assert!(pages::tokens::matches_semantic_registry(tokens));

    let dark = open_gpui_ui_components::theme::ThemeContext::dark();
    let dark_samples = pages::tokens::token_samples_for_theme(tokens, &dark);
    assert_ne!(dark_samples[0].preview_rgb, samples[0].preview_rgb);
    assert_eq!(
        dark_samples[0].preview_rgb,
        dark.snapshot()
            .color_rgb(
                semantic::SURFACE,
                open_gpui_ui_components::ColorState::Default,
            )
            .expect("the complete dark theme should define the surface token")
    );
}

#[test]
fn token_page_exposes_runtime_theme_mode_metadata() {
    let samples = pages::tokens::theme_mode_samples(ThemeTokens::default());

    assert_eq!(samples.len(), 3);
    assert_eq!(samples[0].mode, ThemeMode::Light);
    assert_eq!(samples[1].mode, ThemeMode::Dark);
    assert_eq!(samples[2].mode, ThemeMode::HighContrast);
    assert!(samples[0].source_revision < samples[1].source_revision);
    assert!(samples[1].source_revision < samples[2].source_revision);
    let effective_revisions = [
        samples[0].effective_revision,
        samples[1].effective_revision,
        samples[2].effective_revision,
    ];
    assert!(effective_revisions.iter().all(|revision| *revision > 0));
    assert_ne!(effective_revisions[0], effective_revisions[1]);
    assert_ne!(effective_revisions[0], effective_revisions[2]);
    assert_ne!(effective_revisions[1], effective_revisions[2]);
    assert_eq!(samples[0].density.as_str(), "comfortable");
    assert_eq!(samples[0].motion_policy.as_str(), "animated");
    assert_eq!(samples[0].control_text.len(), 4);
    assert_eq!(samples[0].control_radius.len(), 4);
    assert_ne!(samples[0].surface_rgb, samples[1].surface_rgb);
    assert_ne!(samples[1].focus_ring_rgb, samples[2].focus_ring_rgb);
    assert!(pages::tokens::SIGNALS.contains(&"ThemeScope::new(stable_id, context, child)"));
    assert!(pages::tokens::SIGNALS.contains(&"deferred overlay opening ThemeContext"));
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

    assert_eq!(
        controls,
        [
            pages::focus_a11y::PRIMARY_FOCUS_CONTROL,
            pages::focus_a11y::COUNTER_FOCUS_CONTROL,
            pages::focus_a11y::SWITCH_FOCUS_CONTROL,
        ]
    );
    assert_eq!(
        controls.map(|control| control.tab_index),
        [1, 2, 3],
        "Focus/A11y controls must remain in keyboard focus order"
    );
    assert_eq!(pages::focus_a11y::PRIMARY_FOCUS_CONTROL.id, "focus-primary");
    assert_eq!(
        pages::focus_a11y::COUNTER_FOCUS_CONTROL.role,
        Role::SpinButton
    );
    assert_eq!(pages::focus_a11y::SWITCH_FOCUS_CONTROL.role, Role::Switch);
    assert_eq!(state.counter, 3);
    assert_eq!(state.toggled, Toggled::True);
    assert_eq!(state.counter_role, Role::SpinButton);
}

#[test]
fn focus_a11y_scenarios_bind_story_contracts_to_unique_component_ids() {
    let scenarios = pages::focus_a11y::FOCUS_A11Y_SCENARIOS;
    let stories = pages::focus_a11y::focus_a11y_story_contracts();
    let expected_scenarios = [
        pages::focus_a11y::TEXT_INPUT_VALUE_SELECTION_SCENARIO,
        pages::focus_a11y::TEXTAREA_FIELD_RELATIONS_SCENARIO,
        pages::focus_a11y::PASSWORD_FREE_TEXT_REDACTION_SCENARIO,
    ];

    assert_eq!(
        scenarios.len(),
        expected_scenarios.len(),
        "Focus/A11y scenario fixture must contain exactly the named scenarios"
    );
    assert_eq!(
        scenarios
            .iter()
            .map(|scenario| scenario.id)
            .collect::<std::collections::BTreeSet<_>>(),
        expected_scenarios
            .iter()
            .map(|scenario| scenario.id)
            .collect::<std::collections::BTreeSet<_>>(),
        "Focus/A11y scenario fixture is missing a named scenario or contains an unknown one"
    );
    for expected in expected_scenarios {
        let listed = scenarios
            .iter()
            .find(|scenario| scenario.id == expected.id)
            .unwrap_or_else(|| panic!("missing named Focus/A11y scenario `{}`", expected.id));
        assert_eq!(
            *listed, expected,
            "Focus/A11y scenario `{}` drifted from its named definition",
            expected.id
        );
    }

    assert_eq!(
        pages::focus_a11y::TEXT_INPUT_VALUE_SELECTION_SCENARIO.component_ids,
        &[pages::focus_a11y::TEXT_INPUT_COMPONENT_ID]
    );
    assert_eq!(
        pages::focus_a11y::TEXTAREA_FIELD_RELATIONS_SCENARIO.component_ids,
        &[
            pages::focus_a11y::TEXTAREA_FIELD_COMPONENT_ID,
            pages::focus_a11y::TEXTAREA_COMPONENT_ID,
        ]
    );
    assert_eq!(
        pages::focus_a11y::PASSWORD_FREE_TEXT_REDACTION_SCENARIO.component_ids,
        &[pages::focus_a11y::PASSWORD_COMPONENT_ID]
    );
    assert_eq!(
        pages::focus_a11y::TEXT_INPUT_VALUE_SELECTION_SCENARIO.sample_selector,
        "text-input:focus-a11y-text-input:root"
    );
    assert_eq!(
        pages::focus_a11y::TEXTAREA_FIELD_RELATIONS_SCENARIO.sample_selector,
        "textarea:focus-a11y-field-textarea:root"
    );
    assert_eq!(
        pages::focus_a11y::TEXTAREA_FIELD_RELATIONS_SCENARIO.control_selector,
        Some(pages::focus_a11y::TEXTAREA_FIELD_ERROR_TOGGLE_SELECTOR)
    );
    assert_eq!(
        pages::focus_a11y::PASSWORD_FREE_TEXT_REDACTION_SCENARIO.sample_selector,
        "text-input:focus-a11y-password-input:root"
    );
    assert_eq!(stories.len(), scenarios.len());

    let mut scenario_ids = std::collections::BTreeSet::new();
    let mut component_ids = std::collections::BTreeSet::new();
    for scenario in scenarios {
        assert!(
            scenario_ids.insert(scenario.id),
            "duplicate Focus/A11y scenario id `{}`",
            scenario.id
        );
        assert!(!scenario.component_ids.is_empty());
        for component_id in scenario.component_ids {
            assert!(
                component_ids.insert(*component_id),
                "component id `{component_id}` belongs to more than one Focus/A11y scenario"
            );
        }

        let story = stories
            .iter()
            .find(|story| story.owner_name() == scenario.id)
            .unwrap_or_else(|| panic!("missing story for Focus/A11y scenario `{}`", scenario.id));
        assert_eq!(story.page(), GalleryPage::FocusAccessibility);
        assert_eq!(
            story.kind(),
            open_gpui_ui_foundation_gallery::StoryContractKind::FocusAccessibility
        );
        assert_eq!(
            story.selectors().sample_selector(),
            Some(scenario.sample_selector)
        );
        assert_eq!(
            story.selectors().control_selector(),
            scenario.control_selector
        );
        assert!(story.selectors().catalog_selector().is_none());
        assert!(story.has_operation(StoryProbeOperation::ReadPublicPayload));
    }
}
