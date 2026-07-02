use super::*;

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
    let adapter = gpui_overlay_state(&OverlayResolvedState::resolve(samples[0].policy.clone()));
    assert_eq!(
        adapter.deferred_priority(),
        default_deferred_priority(OverlayLayerKind::Tooltip)
    );
    assert_eq!(adapter.snap_margin(), DEFAULT_OVERLAY_SAFE_MARGIN);
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
    let adapter = gpui_overlay_state(&OverlayResolvedState::resolve(samples[1].policy.clone()));
    assert!(adapter.wants_outside_press_handler());

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
    let adapter = gpui_overlay_state(&OverlayResolvedState::resolve(samples[2].policy.clone()));
    assert_eq!(
        adapter.deferred_priority(),
        default_deferred_priority(OverlayLayerKind::Modal)
    );

    assert_eq!(samples[3].id, "menu");
    assert_eq!(samples[3].policy.kind(), OverlayLayerKind::Menu);
    assert_eq!(
        samples[3].policy.outside_press_policy(),
        OutsidePressPolicy::DismissAndConsume
    );
    assert_eq!(samples[3].policy.kind().as_str(), "menu");
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
    assert_eq!(samples[0].state.title(), "Controlled dialog");
    assert_eq!(
        samples[0].state.description(),
        Some("Escape and the modal barrier can close it.")
    );
    assert_eq!(samples[0].state.open_mode(), DialogOpenMode::Controlled);
    assert!(!samples[0].state.open());
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
    assert_eq!(samples[0].state.title(), "Delete this project?");
    assert_eq!(
        samples[0].state.description(),
        "This permanently removes project data and cannot be undone."
    );
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
    assert_eq!(samples[1].state.title(), "Archive this item?");
    assert_eq!(
        samples[1].state.description(),
        "The item moves out of the active list and can be restored later."
    );
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
    assert_eq!(samples[0].state.title(), "Workspace filters");
    assert_eq!(
        samples[0].state.description(),
        Some("Filter active work without leaving the page.")
    );
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

    assert_eq!(samples.len(), 7);
    assert_eq!(samples[0].id, "default-open");
    assert_eq!(samples[0].focused_value, Some("save"));
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
    assert_eq!(samples[1].focused_value, Some("copy"));
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
    assert_eq!(samples[2].focused_value, None);
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
    assert_eq!(samples[3].focused_value, None);
    assert!(samples[3].state.disabled());
    assert!(!samples[3].state.open());

    assert_eq!(samples[4].id, "rich-items");
    assert_eq!(samples[4].focused_value, Some("show-hidden"));
    assert!(samples[4].state.open());
    assert_eq!(samples[4].state.items()[0].kind(), MenuItemKind::Checkbox);
    assert_eq!(samples[4].state.items()[0].toggled(), Some(Toggled::True));
    assert_eq!(samples[4].state.items()[1].kind(), MenuItemKind::Radio);
    assert_eq!(samples[4].state.items()[1].toggled(), Some(Toggled::False));
    assert_eq!(samples[4].state.items()[2].toggled(), Some(Toggled::True));
    assert!(samples[4].state.items()[3].has_submenu());
    assert!(samples[4].state.items()[4].has_submenu());
    assert!(!samples[4].state.items()[5].focusable());

    assert_eq!(samples[5].id, "typeahead");
    assert_eq!(
        samples[5]
            .state
            .typeahead_target("br")
            .map(|item| item.value()),
        Some("bravo")
    );

    assert_eq!(samples[6].id, "long-scroll");
    assert!(samples[6].state.scrollable_content());
    assert_eq!(samples[6].state.visible_items().len(), 12);
}

#[test]
fn overlay_page_context_menu_samples_expose_point_anchor_contracts() {
    let samples = pages::overlay::context_menu_samples(ThemeTokens::default());

    assert_eq!(samples.len(), 5);
    assert_eq!(samples[0].id, "point-anchor");
    assert_eq!(samples[0].focused_value, Some("duplicate"));
    assert!(samples[0].state.default_open());
    assert!(samples[0].state.open());
    assert_eq!(samples[0].state.open_mode(), MenuOpenMode::Uncontrolled);
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
    assert_eq!(samples[1].focused_value, Some("inspect"));
    assert!(!samples[1].state.open());
    assert_eq!(samples[1].state.open_mode(), MenuOpenMode::Controlled);

    assert_eq!(samples[2].id, "default-open");
    assert_eq!(samples[2].focused_value, None);
    assert!(samples[2].state.default_open());
    assert!(samples[2].state.open());
    assert_eq!(samples[2].state.open_mode(), MenuOpenMode::Uncontrolled);

    assert_eq!(samples[3].id, "rich-items");
    assert_eq!(samples[3].focused_value, Some("snap-grid"));
    assert!(samples[3].state.open());
    assert_eq!(
        samples[3].state.menu().items()[0].kind(),
        MenuItemKind::Checkbox
    );
    assert_eq!(
        samples[3].state.menu().items()[0].toggled(),
        Some(Toggled::True)
    );
    assert!(samples[3].state.menu().items()[3].has_submenu());
    assert_eq!(
        samples[3].state.anchor_point(),
        ui_point(ui_px(620.0), ui_px(340.0))
    );

    assert_eq!(samples[4].id, "edge-long");
    assert!(samples[4].state.menu().scrollable_content());
    assert_eq!(
        samples[4].state.anchor_point(),
        ui_point(ui_px(960.0), ui_px(560.0))
    );
}

#[test]
fn overlay_page_catalog_entries_have_signals_and_sample_selectors() {
    use std::collections::BTreeSet;

    let tokens = ThemeTokens::default();
    let catalog = pages::overlay::OVERLAY_CATALOG;
    let names = catalog.iter().map(|entry| entry.name).collect::<Vec<_>>();

    assert_eq!(
        names,
        vec![
            "Tooltip",
            "HoverCard",
            "Popover",
            "Dialog",
            "AlertDialog",
            "Sheet",
            "Menu",
            "ContextMenu",
        ]
    );
    assert!(
        catalog
            .iter()
            .all(|entry| entry.status == pages::overlay::OverlayCatalogStatus::Official)
    );
    assert!(catalog.iter().all(|entry| !entry.family.trim().is_empty()));
    assert!(catalog.iter().all(|entry| !entry.state.trim().is_empty()));
    assert!(
        catalog
            .iter()
            .all(|entry| !entry.coverage.trim().is_empty())
    );
    assert!(catalog.iter().all(|entry| {
        !entry.behavior_gates.is_empty()
            && entry
                .behavior_gates
                .iter()
                .all(|gate| !gate.trim().is_empty())
    }));

    let catalog_names = catalog
        .iter()
        .map(|entry| entry.name)
        .collect::<BTreeSet<_>>();
    let selector_names = pages::overlay::overlay_sample_selector_pairs()
        .map(|(name, _)| name)
        .collect::<BTreeSet<_>>();
    assert_eq!(selector_names, catalog_names);

    let selector_values = pages::overlay::overlay_sample_selector_pairs()
        .map(|(_, selector)| selector)
        .collect::<Vec<_>>();
    let unique_selectors = selector_values.iter().copied().collect::<BTreeSet<_>>();
    assert_eq!(unique_selectors.len(), selector_values.len());

    let tooltip_samples = pages::overlay::tooltip_samples(tokens);
    let hover_card_samples = pages::overlay::hover_card_samples(tokens);
    let popover_samples = pages::overlay::popover_samples(tokens);
    let dialog_samples = pages::overlay::dialog_samples(tokens);
    let alert_dialog_samples = pages::overlay::alert_dialog_samples(tokens);
    let sheet_samples = pages::overlay::sheet_samples(tokens);
    let menu_samples = pages::overlay::menu_samples(tokens);
    let context_menu_samples = pages::overlay::context_menu_samples(tokens);
    let expected_selectors = [
        ("Tooltip", tooltip_samples[0].debug_selector()),
        ("HoverCard", hover_card_samples[0].debug_selector()),
        ("Popover", popover_samples[0].debug_selector()),
        ("Dialog", dialog_samples[0].debug_selector()),
        ("AlertDialog", alert_dialog_samples[0].debug_selector()),
        ("Sheet", sheet_samples[0].debug_selector()),
        ("Menu", menu_samples[0].debug_selector()),
        ("ContextMenu", context_menu_samples[0].debug_selector()),
    ];

    for (name, selector) in expected_selectors {
        let entry = catalog
            .iter()
            .find(|entry| entry.name == name)
            .unwrap_or_else(|| panic!("expected overlay catalog entry `{name}`"));
        assert_eq!(entry.sample_selector, selector.as_str());
    }

    for signal in [
        "open_gpui_ui_foundation_gallery::pages::overlay::OVERLAY_CATALOG",
        "open_gpui_ui_foundation_gallery::pages::overlay::OverlayCatalogEntry",
        "open_gpui_ui_foundation_gallery::pages::overlay::OverlayCatalogStatus",
        "open_gpui_ui_foundation_gallery::pages::overlay::overlay_sample_selector_pairs",
    ] {
        assert!(
            pages::overlay::SIGNALS.contains(&signal),
            "expected overlay catalog signal `{signal}`"
        );
    }

    let mut missing = Vec::new();
    for entry in catalog {
        let component_signal = format!("open_gpui_ui_components::{}", entry.name);
        if !pages::overlay::SIGNALS.contains(&component_signal.as_str()) {
            missing.push(format!(
                "{} component signal `{component_signal}`",
                entry.name
            ));
        }
        let state_signal = format!("open_gpui_ui_components::{}", entry.state);
        if !pages::overlay::SIGNALS.contains(&state_signal.as_str()) {
            missing.push(format!("{} state signal `{state_signal}`", entry.name));
        }
    }

    assert!(
        missing.is_empty(),
        "official overlay catalog entries must have matching signals: {missing:?}"
    );
}
