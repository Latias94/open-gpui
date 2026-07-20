mod support;

use open_gpui::prelude::FluentBuilder;
use open_gpui::{
    Anchor, Context, FocusHandle, InteractiveElement, IntoElement, MouseButton, ParentElement,
    Render, ScrollDelta, ScrollWheelEvent, StatefulInteractiveElement, Styled, SubtreePresentation,
    SubtreePresentationExt, VisualContext, Window, accesskit, actions, div, point, px,
};
use open_gpui_ui_components::{
    AlertDialog, AlertDialogActionKind, AlertDialogIntent, AlertDialogOpenMode, ButtonVariant,
    ColorState, ContextMenu, Dialog, DialogOpenMode, HoverCard, HoverCardContentKind,
    HoverCardDelayPolicy, HoverCardOpenIntent, HoverCardOpenMode, Menu, MenuItem,
    MenuItemDescriptor, MenuItemKind, MenuOpenMode, MenuSelection, MenuSubmenuSurface, Popover,
    PopoverOpenMode, Sheet, SheetCloseAffordance, SheetModalMode, SheetOpenMode, SheetSide,
    Tooltip, TooltipContentKind, TooltipDelayPolicy, TooltipOpenIntent,
    gpui_adapter::{
        DEFAULT_OVERLAY_SAFE_MARGIN, FocusTargetRegistration, GpuiOverlayAdapterConfig,
        GpuiOverlayPlacement, OverlayLayerPhase, OverlayOpenIntent, WindowFocusFallbackLease,
        WindowOverlayRuntime, WindowOverlayRuntimeError, default_deferred_priority, gpui_anchor,
        point_anchor_placement,
    },
    menu_navigation_target,
    theme::ThemeResolver,
};
use open_gpui_ui_core::{
    DismissReason, EscapeKeyPolicy, FocusRestoreIntent, FocusTargetAvailability, FocusTargetId,
    InitialFocusIntent, OutsidePressParticipation, OutsidePressPolicy, OverlayAnchorInput,
    OverlayLayerKind, OverlayPlacementAlignment, OverlayPlacementInput, OverlayPlacementSide,
    OverlayPresence, Role, Sizable, Size, Toggled, rect, semantic, ui_point, ui_px, ui_size,
};
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::Duration;
use support::a11y::{assert_exact_actions, node_with_label};

actions!(overlay_tooltip_runtime_test, [TooltipRuntimeAction]);

#[test]
fn overlay_adapter_config_defaults_follow_overlay_kind_policy() {
    let tooltip =
        GpuiOverlayAdapterConfig::new(OverlayLayerKind::Tooltip, OverlayPresence::open()).state();
    let popover = GpuiOverlayAdapterConfig::new(
        OverlayLayerKind::NonModalDismissible,
        OverlayPresence::open(),
    )
    .state();
    let dialog =
        GpuiOverlayAdapterConfig::new(OverlayLayerKind::Modal, OverlayPresence::open()).state();
    let menu =
        GpuiOverlayAdapterConfig::new(OverlayLayerKind::Menu, OverlayPresence::open()).state();

    assert_eq!(tooltip.policy().kind(), OverlayLayerKind::Tooltip);
    assert_eq!(
        tooltip.deferred_priority(),
        default_deferred_priority(OverlayLayerKind::Tooltip)
    );
    assert_eq!(tooltip.snap_margin(), DEFAULT_OVERLAY_SAFE_MARGIN);
    assert!(tooltip.should_render_deferred_layer());
    assert!(!tooltip.layer_state().hit_testable());

    assert_eq!(
        popover.policy().kind(),
        OverlayLayerKind::NonModalDismissible
    );
    assert_eq!(
        popover.deferred_priority(),
        default_deferred_priority(OverlayLayerKind::NonModalDismissible)
    );
    assert!(popover.layer_state().visible());
    assert!(popover.wants_outside_press_handler());

    assert_eq!(dialog.policy().kind(), OverlayLayerKind::Modal);
    assert_eq!(
        dialog.deferred_priority(),
        default_deferred_priority(OverlayLayerKind::Modal)
    );
    assert!(dialog.layer_state().blocks_underlay_input());

    assert_eq!(menu.policy().kind(), OverlayLayerKind::Menu);
    assert_eq!(
        menu.deferred_priority(),
        default_deferred_priority(OverlayLayerKind::Menu)
    );
    assert!(menu.layer_state().visible());
}

#[test]
fn overlay_adapter_config_can_override_focus_and_dismiss_policy() {
    let state = GpuiOverlayAdapterConfig::new(
        OverlayLayerKind::NonModalDismissible,
        OverlayPresence::open(),
    )
    .outside_press_policy(OutsidePressPolicy::DismissAndConsume)
    .escape_key_policy(EscapeKeyPolicy::Dismiss)
    .focus_restore_intent(open_gpui_ui_core::FocusRestoreIntent::TriggerOrFallback(
        open_gpui_ui_core::FocusTargetId::new("fallback"),
    ))
    .initial_focus_intent(InitialFocusIntent::FirstFocusable)
    .deferred_priority(9)
    .snap_margin(px(12.0))
    .state();

    assert_eq!(
        state.policy().outside_press_policy(),
        OutsidePressPolicy::DismissAndConsume
    );
    assert_eq!(state.policy().escape_key_policy(), EscapeKeyPolicy::Dismiss);
    assert_eq!(state.deferred_priority(), 9);
    assert_eq!(state.snap_margin(), px(12.0));
}

#[test]
fn overlay_placement_maps_to_gpui_anchor_and_margin() {
    let input = OverlayPlacementInput::new(
        OverlayAnchorInput::from_visual_and_layout_bounds(
            Some(rect(
                ui_point(ui_px(10.0), ui_px(20.0)),
                ui_size(ui_px(100.0), ui_px(40.0)),
            )),
            Some(rect(
                ui_point(ui_px(30.0), ui_px(40.0)),
                ui_size(ui_px(120.0), ui_px(60.0)),
            )),
        ),
        ui_size(ui_px(180.0), ui_px(120.0)),
    )
    .with_side(OverlayPlacementSide::Bottom)
    .with_alignment(OverlayPlacementAlignment::End)
    .with_offset(ui_px(6.0))
    .with_safe_bounds(rect(
        ui_point(ui_px(0.0), ui_px(0.0)),
        ui_size(ui_px(300.0), ui_px(220.0)),
    ));

    let placement = GpuiOverlayPlacement::resolve(input, DEFAULT_OVERLAY_SAFE_MARGIN);

    assert_eq!(placement.anchor(), Anchor::TopLeft);
    assert_eq!(placement.snap_margin(), DEFAULT_OVERLAY_SAFE_MARGIN);
    assert!(placement.position().is_some());
    assert_eq!(placement.safe_bounds(), input.safe_bounds());
    assert_eq!(
        placement.fit().as_str(),
        "aligned",
        "adapter should consume the shared placement solver before mapping to a GPUI anchor"
    );
    assert_eq!(
        placement.trace().selected().alignment(),
        OverlayPlacementAlignment::Start
    );
}

#[test]
fn overlay_placement_helpers_map_shared_coordinates() {
    assert_eq!(
        gpui_anchor(OverlayPlacementSide::Top, OverlayPlacementAlignment::Start),
        Anchor::BottomLeft
    );
    let point_placement =
        point_anchor_placement(point(px(5.0), px(6.0)), ui_size(ui_px(80.0), ui_px(40.0)));
    assert_eq!(
        GpuiOverlayPlacement::resolve(point_placement, DEFAULT_OVERLAY_SAFE_MARGIN).anchor(),
        Anchor::TopLeft
    );
}

#[test]
fn overlay_label_helpers_are_stable() {
    assert_eq!(MenuOpenMode::Uncontrolled.as_str(), "uncontrolled");
    assert_eq!(TooltipOpenIntent::Manual.as_str(), "manual");
    assert_eq!(TooltipContentKind::Element.as_str(), "element");
    assert_eq!(HoverCardOpenMode::Controlled.as_str(), "controlled");
    assert_eq!(HoverCardOpenIntent::HoverOrFocus.as_str(), "hover or focus");
    assert_eq!(HoverCardContentKind::Text.as_str(), "text");
    assert_eq!(PopoverOpenMode::Uncontrolled.as_str(), "uncontrolled");
    assert_eq!(DialogOpenMode::Controlled.as_str(), "controlled");
    assert_eq!(AlertDialogOpenMode::Controlled.as_str(), "controlled");
    assert_eq!(AlertDialogIntent::Destructive.as_str(), "destructive");
    assert_eq!(SheetOpenMode::Controlled.as_str(), "controlled");
    assert_eq!(SheetSide::Left.as_str(), "left");
    assert_eq!(SheetModalMode::NonModal.as_str(), "non-modal");
    assert_eq!(OverlayLayerKind::Menu.as_str(), "menu");
    assert_eq!(
        OutsidePressPolicy::DismissAndPassThrough.as_str(),
        "dismiss + pass-through"
    );
    assert_eq!(EscapeKeyPolicy::Ignore.as_str(), "ignore");
    assert_eq!(FocusRestoreIntent::None.as_str(), "none");
    assert_eq!(
        InitialFocusIntent::TargetOrFirstFocusable(open_gpui_ui_core::FocusTargetId::new("x"))
            .as_str(),
        "target or first focusable"
    );
}

#[test]
fn tooltip_state_records_descriptive_overlay_policy() {
    let state = Tooltip::new("tip", "Save changes").open(true).state();

    assert_eq!(state.content_kind(), TooltipContentKind::Text);
    assert_eq!(state.role(), Role::Label);
    assert!(state.open());
    assert!(state.descriptive());
    assert!(!state.interactive_content());
    assert!(state.open_intent().opens_on_hover());
    assert!(state.open_intent().opens_on_focus());
    assert_eq!(state.placement_side(), OverlayPlacementSide::Top);
    assert_eq!(
        state.placement_alignment(),
        OverlayPlacementAlignment::Center
    );
    assert_eq!(state.delay().open_delay(), Duration::from_millis(500));
    assert_eq!(state.colors().background().token(), semantic::OVERLAY);
    assert_eq!(state.overlay().policy().kind(), OverlayLayerKind::Tooltip);
    assert!(state.overlay().should_render_deferred_layer());
    assert!(!state.overlay().layer_state().hit_testable());
}

#[test]
fn tooltip_state_models_disabled_element_content_and_delay_overrides() {
    let delay = TooltipDelayPolicy::new(
        Duration::from_millis(120),
        Duration::from_millis(40),
        Duration::from_millis(250),
    );
    let state = Tooltip::element("rich-tip", div().child("Rich"))
        .open(true)
        .disabled(true)
        .open_intent(TooltipOpenIntent::Focus)
        .placement_side(OverlayPlacementSide::Bottom)
        .placement_alignment(OverlayPlacementAlignment::End)
        .delay(delay)
        .small()
        .state();

    assert_eq!(state.content_kind(), TooltipContentKind::Element);
    assert!(state.disabled());
    assert!(!state.open());
    assert!(!state.open_intent().opens_on_hover());
    assert!(state.open_intent().opens_on_focus());
    assert_eq!(state.placement_side(), OverlayPlacementSide::Bottom);
    assert_eq!(state.placement_alignment(), OverlayPlacementAlignment::End);
    assert_eq!(state.delay(), delay);
    assert_eq!(state.size(), Size::Small);
    assert!(!state.overlay().should_render_deferred_layer());
}

#[test]
fn hover_card_state_records_interactive_hover_focus_overlay_policy() {
    let state = HoverCard::new("profile-card", "Open profile", "Profile details")
        .open(true)
        .placement_side(OverlayPlacementSide::Right)
        .placement_alignment(OverlayPlacementAlignment::End)
        .state();

    assert_eq!(state.content_kind(), HoverCardContentKind::Text);
    assert!(state.open());
    assert_eq!(state.open_mode(), HoverCardOpenMode::Controlled);
    assert_eq!(state.trigger_role(), Role::Button);
    assert_eq!(state.content_role(), Role::Window);
    assert!(!state.descriptive());
    assert!(state.interactive_content());
    assert!(state.open_intent().opens_on_hover());
    assert!(state.open_intent().opens_on_focus());
    assert!(!state.open_intent().opens_manually());
    assert!(state.trigger_selected());
    assert_eq!(state.placement_side(), OverlayPlacementSide::Right);
    assert_eq!(state.placement_alignment(), OverlayPlacementAlignment::End);
    assert_eq!(state.delay().open_delay(), Duration::from_millis(700));
    assert_eq!(state.delay().close_delay(), Duration::from_millis(300));
    assert_eq!(
        state.overlay().policy().kind(),
        OverlayLayerKind::NonModalDismissible
    );
    assert_eq!(state.outside_press_policy(), OutsidePressPolicy::Ignore);
    assert_eq!(
        state.overlay().policy().outside_press_participation(),
        OutsidePressParticipation::Transparent
    );
    assert_eq!(state.initial_focus_intent(), &InitialFocusIntent::None);
    assert_eq!(state.focus_restore_intent(), &FocusRestoreIntent::None);
    assert!(!state.overlay().wants_outside_press_handler());
    assert!(state.overlay().layer_state().hit_testable());
    assert_eq!(state.colors().background().token(), semantic::SURFACE);
    assert_eq!(
        state.colors().trigger_background().state(),
        ColorState::Selected
    );
}

#[test]
fn hover_card_state_models_manual_disabled_and_fixed_passive_policy() {
    let delay = HoverCardDelayPolicy::new(Duration::from_millis(80), Duration::from_millis(20));
    let state = HoverCard::element("rich-hover-card", "Details", div().child("Rich"))
        .default_open(true)
        .disabled(true)
        .open_intent(HoverCardOpenIntent::Manual)
        .delay(delay)
        .small()
        .state();

    assert_eq!(state.content_kind(), HoverCardContentKind::Element);
    assert_eq!(state.open_mode(), HoverCardOpenMode::Uncontrolled);
    assert!(state.default_open());
    assert!(state.disabled());
    assert!(!state.open());
    assert!(!state.activation_enabled());
    assert!(!state.open_intent().opens_on_hover());
    assert!(!state.open_intent().opens_on_focus());
    assert!(state.open_intent().opens_manually());
    assert_eq!(state.delay(), delay);
    assert_eq!(state.size(), Size::Small);
    assert_eq!(state.outside_press_policy(), OutsidePressPolicy::Ignore);
    assert_eq!(
        state.overlay().policy().outside_press_participation(),
        OutsidePressParticipation::Transparent
    );
    assert_eq!(state.initial_focus_intent(), &InitialFocusIntent::None);
    assert_eq!(state.focus_restore_intent(), &FocusRestoreIntent::None);
    assert!(!state.overlay().should_render_deferred_layer());
}

#[test]
fn popover_state_records_interactive_overlay_policy() {
    let state = Popover::new("settings-popover", "Settings", "Panel")
        .open(true)
        .placement_side(OverlayPlacementSide::Right)
        .placement_alignment(OverlayPlacementAlignment::End)
        .state();

    assert!(state.open());
    assert_eq!(state.open_mode(), PopoverOpenMode::Controlled);
    assert_eq!(state.trigger_role(), Role::Button);
    assert_eq!(state.content_role(), Role::Window);
    assert!(state.trigger_selected());
    assert!(state.activation_enabled());
    assert_eq!(state.placement_side(), OverlayPlacementSide::Right);
    assert_eq!(state.placement_alignment(), OverlayPlacementAlignment::End);
    assert_eq!(
        state.overlay().policy().kind(),
        OverlayLayerKind::NonModalDismissible
    );
    assert_eq!(
        state.outside_press_policy(),
        OutsidePressPolicy::DismissAndPassThrough
    );
    assert_eq!(state.focus_restore_intent(), &FocusRestoreIntent::Trigger);
    assert_eq!(state.initial_focus_intent(), &InitialFocusIntent::None);
    assert!(state.overlay().wants_outside_press_handler());
    assert!(state.overlay().layer_state().hit_testable());
    assert_eq!(state.colors().background().token(), semantic::SURFACE);
    assert_eq!(
        state.colors().trigger_background().state(),
        ColorState::Selected
    );
}

#[test]
fn popover_state_models_default_open_disabled_and_policy_overrides() {
    let state = Popover::element("help-popover", "Help", div().child("Rich"))
        .default_open(true)
        .disabled(true)
        .outside_press_policy(OutsidePressPolicy::DismissAndConsume)
        .initial_focus_intent(InitialFocusIntent::None)
        .focus_restore_intent(FocusRestoreIntent::None)
        .small()
        .state();

    assert_eq!(state.open_mode(), PopoverOpenMode::Uncontrolled);
    assert!(state.default_open());
    assert!(state.disabled());
    assert!(!state.open());
    assert!(!state.activation_enabled());
    assert_eq!(state.size(), Size::Small);
    assert_eq!(
        state.outside_press_policy(),
        OutsidePressPolicy::DismissAndConsume
    );
    assert_eq!(state.initial_focus_intent(), &InitialFocusIntent::None);
    assert_eq!(state.focus_restore_intent(), &FocusRestoreIntent::None);
    assert!(!state.overlay().should_render_deferred_layer());
}

#[test]
fn dialog_state_records_modal_title_and_focus_policy() {
    let state = Dialog::new("confirm-dialog", "Open", "Confirm changes", "Body")
        .description("This cannot be undone.")
        .open(true)
        .state();

    assert!(state.open());
    assert_eq!(state.open_mode(), DialogOpenMode::Controlled);
    assert_eq!(state.title(), "Confirm changes");
    assert_eq!(state.description(), Some("This cannot be undone."));
    assert_eq!(state.trigger_role(), Role::Button);
    assert_eq!(state.content_role(), Role::Window);
    assert!(state.trigger_selected());
    assert_eq!(state.overlay().policy().kind(), OverlayLayerKind::Modal);
    assert!(state.overlay().layer_state().blocks_underlay_input());
    assert_eq!(
        state.outside_press_policy(),
        OutsidePressPolicy::DismissAndConsume
    );
    assert_eq!(state.escape_key_policy(), EscapeKeyPolicy::Dismiss);
    assert_eq!(
        state.initial_focus_intent(),
        &InitialFocusIntent::FirstFocusable
    );
    assert_eq!(state.focus_restore_intent(), &FocusRestoreIntent::Trigger);
    assert_eq!(state.colors().barrier().token(), semantic::MODAL_OVERLAY);
}

#[open_gpui::test]
fn dialog_final_tree_projects_modal_disabled_and_exact_actions(cx: &mut open_gpui::TestAppContext) {
    struct DialogAccessibilityProbe {
        open: bool,
        disabled: bool,
    }

    impl Render for DialogAccessibilityProbe {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            div().size_full().child(
                Dialog::new(
                    "semantic-dialog",
                    "Open semantic dialog",
                    "Semantic dialog",
                    "Dialog body",
                )
                .open(self.open)
                .disabled(self.disabled),
            )
        }
    }

    let (view, cx) = cx.add_window_view(|_, _| DialogAccessibilityProbe {
        open: false,
        disabled: false,
    });
    assert!(cx.activate_accessibility());

    let closed_update = cx
        .latest_accessibility_tree_update()
        .expect("closed dialog accessibility tree should publish");
    let (trigger_id, trigger) = node_with_label(&closed_update, "Open semantic dialog");
    assert_eq!(trigger.role(), accesskit::Role::Button);
    assert_eq!(trigger.is_selected(), Some(false));
    assert_eq!(trigger.is_expanded(), Some(false));
    assert!(!trigger.is_disabled());
    assert_exact_actions(
        trigger,
        &[accesskit::Action::Click, accesskit::Action::Focus],
    );

    view.update(cx, |probe, cx| {
        probe.open = true;
        cx.notify();
    });
    cx.run_until_parked();
    cx.update(|window, cx| window.draw(cx).clear());

    let open_update = cx
        .latest_accessibility_tree_update()
        .expect("open dialog accessibility tree should publish");
    assert!(
        open_update.nodes.iter().all(|(id, _)| *id != trigger_id),
        "the modal tree scope must exclude its underlay trigger"
    );
    let (_, surface) = node_with_label(&open_update, "Semantic dialog");
    assert_eq!(surface.role(), accesskit::Role::Window);
    assert!(surface.is_modal());
    assert_exact_actions(surface, &[accesskit::Action::Focus]);

    view.update(cx, |probe, cx| {
        probe.open = false;
        probe.disabled = true;
        cx.notify();
    });
    cx.run_until_parked();
    cx.update(|window, cx| window.draw(cx).clear());

    let disabled_update = cx
        .latest_accessibility_tree_update()
        .expect("disabled dialog accessibility tree should publish");
    let (disabled_trigger_id, disabled_trigger) =
        node_with_label(&disabled_update, "Open semantic dialog");
    assert_eq!(disabled_trigger_id, trigger_id);
    assert_eq!(disabled_trigger.role(), accesskit::Role::Button);
    assert_eq!(disabled_trigger.is_selected(), Some(false));
    assert_eq!(disabled_trigger.is_expanded(), Some(false));
    assert!(disabled_trigger.is_disabled());
    assert_exact_actions(disabled_trigger, &[]);
}

#[test]
fn dialog_state_models_disabled_default_open_and_policy_overrides() {
    let state = Dialog::element("modal", "Open", "Blocked dialog", div().child("Rich"))
        .default_open(true)
        .disabled(true)
        .outside_press_policy(OutsidePressPolicy::Ignore)
        .escape_key_policy(EscapeKeyPolicy::Ignore)
        .initial_focus_intent(InitialFocusIntent::None)
        .focus_restore_intent(FocusRestoreIntent::None)
        .small()
        .state();

    assert_eq!(state.open_mode(), DialogOpenMode::Uncontrolled);
    assert!(state.default_open());
    assert!(state.disabled());
    assert!(!state.open());
    assert!(!state.activation_enabled());
    assert_eq!(state.size(), Size::Small);
    assert_eq!(state.outside_press_policy(), OutsidePressPolicy::Ignore);
    assert_eq!(state.escape_key_policy(), EscapeKeyPolicy::Ignore);
    assert_eq!(state.initial_focus_intent(), &InitialFocusIntent::None);
    assert_eq!(state.focus_restore_intent(), &FocusRestoreIntent::None);
    assert!(!state.overlay().should_render_deferred_layer());
}

#[open_gpui::test]
fn dialog_resolves_declared_live_target_and_unavailable_fallback(
    cx: &mut open_gpui::TestAppContext,
) {
    struct TestView {
        preferred_focus: FocusHandle,
        fallback_focus: FocusHandle,
        preferred_available: bool,
    }

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let preferred_focus = self.preferred_focus.clone();
            let fallback_focus = self.fallback_focus.clone();
            let availability = if self.preferred_available {
                FocusTargetAvailability::Available
            } else {
                FocusTargetAvailability::Hidden
            };
            let intent = if self.preferred_available {
                InitialFocusIntent::Target(FocusTargetId::new("primary"))
            } else {
                InitialFocusIntent::TargetOrFirstFocusable(FocusTargetId::new("primary"))
            };

            Dialog::element(
                "declared-focus-target-dialog",
                "Open dialog",
                "Declared focus target",
                div()
                    .child(
                        div()
                            .id("dialog-test-preferred-target")
                            .debug_selector(|| "dialog-test:preferred-target".to_owned())
                            .focusable()
                            .tab_stop(true)
                            .track_focus(&preferred_focus)
                            .child("Preferred"),
                    )
                    .child(
                        div()
                            .id("dialog-test-fallback-target")
                            .debug_selector(|| "dialog-test:fallback-target".to_owned())
                            .focusable()
                            .tab_stop(true)
                            .track_focus(&fallback_focus)
                            .child("Fallback"),
                    ),
            )
            .default_open(true)
            .initial_focus_intent(intent)
            .focus_target(
                FocusTargetRegistration::new("primary", &self.preferred_focus)
                    .with_availability(availability),
            )
        }
    }

    let (view, cx) = cx.add_window_view(|_, cx| TestView {
        preferred_focus: cx.focus_handle(),
        fallback_focus: cx.focus_handle(),
        preferred_available: true,
    });
    cx.update(|window, cx| window.draw(cx).clear());
    cx.run_until_parked();
    assert!(
        cx.debug_selector_is_focused("dialog-test:preferred-target"),
        "an exact declared target should resolve through the live window registry"
    );

    cx.update_window_entity(&view, |view, _, cx| {
        view.preferred_available = false;
        cx.notify();
    });
    cx.update(|window, cx| window.draw(cx).clear());
    cx.run_until_parked();
    assert!(
        cx.debug_selector_is_focused("dialog-test:fallback-target"),
        "TargetOrFirstFocusable should skip an unavailable registered handle"
    );

    cx.update(|window, cx| window.draw(cx).clear());
    assert!(cx.debug_selector_is_focused("dialog-test:fallback-target"));
}

#[open_gpui::test]
fn dialog_focus_target_sync_accepts_declared_id_rename(cx: &mut open_gpui::TestAppContext) {
    struct TestView {
        focus: FocusHandle,
        target_id: &'static str,
    }

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            Dialog::element(
                "focus-target-rename-dialog",
                "Open dialog",
                "Focus target rename",
                div()
                    .id("focus-target-rename-node")
                    .debug_selector(|| "dialog-focus-sync:renamed-node".to_owned())
                    .focusable()
                    .tab_stop(true)
                    .track_focus(&self.focus),
            )
            .default_open(true)
            .initial_focus_intent(InitialFocusIntent::Target(FocusTargetId::new(
                self.target_id,
            )))
            .focus_target(FocusTargetRegistration::new(self.target_id, &self.focus))
        }
    }

    let (view, cx) = cx.add_window_view(|_, cx| TestView {
        focus: cx.focus_handle(),
        target_id: "before-rename",
    });
    cx.update(|window, cx| window.draw(cx).clear());
    cx.run_until_parked();
    assert!(cx.debug_selector_is_focused("dialog-focus-sync:renamed-node"));

    cx.update_window_entity(&view, |view, _, cx| {
        view.target_id = "after-rename";
        cx.notify();
    });
    cx.update(|window, cx| window.draw(cx).clear());
    cx.run_until_parked();

    assert!(cx.debug_selector_is_focused("dialog-focus-sync:renamed-node"));
}

#[open_gpui::test]
fn dialog_focus_target_sync_atomically_swaps_handles_and_rearbitrates_focus(
    cx: &mut open_gpui::TestAppContext,
) {
    struct TestView {
        first_focus: FocusHandle,
        second_focus: FocusHandle,
        swapped: bool,
    }

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let (alpha_focus, beta_focus) = if self.swapped {
                (&self.second_focus, &self.first_focus)
            } else {
                (&self.first_focus, &self.second_focus)
            };
            Dialog::element(
                "focus-target-swap-dialog",
                "Open dialog",
                "Focus target swap",
                div()
                    .child(
                        div()
                            .id("focus-target-swap-first")
                            .debug_selector(|| "dialog-focus-sync:first-node".to_owned())
                            .focusable()
                            .tab_stop(true)
                            .track_focus(&self.first_focus),
                    )
                    .child(
                        div()
                            .id("focus-target-swap-second")
                            .debug_selector(|| "dialog-focus-sync:second-node".to_owned())
                            .focusable()
                            .tab_stop(true)
                            .track_focus(&self.second_focus),
                    ),
            )
            .default_open(true)
            .initial_focus_intent(InitialFocusIntent::Target(FocusTargetId::new("alpha")))
            .focus_target(FocusTargetRegistration::new("alpha", alpha_focus))
            .focus_target(FocusTargetRegistration::new("beta", beta_focus))
        }
    }

    let (view, cx) = cx.add_window_view(|_, cx| TestView {
        first_focus: cx.focus_handle(),
        second_focus: cx.focus_handle(),
        swapped: false,
    });
    cx.update(|window, cx| window.draw(cx).clear());
    cx.run_until_parked();
    assert!(cx.debug_selector_is_focused("dialog-focus-sync:first-node"));

    cx.update_window_entity(&view, |view, _, cx| {
        view.swapped = true;
        cx.notify();
    });
    cx.update(|window, cx| window.draw(cx).clear());
    cx.run_until_parked();

    assert!(
        cx.debug_selector_is_focused("dialog-focus-sync:second-node"),
        "the logical alpha target must follow its replacement handle"
    );
}

#[open_gpui::test]
fn dialog_focus_target_sync_rearbitrates_when_focused_target_is_removed(
    cx: &mut open_gpui::TestAppContext,
) {
    struct TestView {
        preferred_focus: FocusHandle,
        fallback_focus: FocusHandle,
        include_preferred: bool,
    }

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let content = div()
                .children(self.include_preferred.then(|| {
                    div()
                        .id("focus-target-remove-preferred")
                        .debug_selector(|| "dialog-focus-sync:removed-node".to_owned())
                        .focusable()
                        .tab_stop(true)
                        .track_focus(&self.preferred_focus)
                }))
                .child(
                    div()
                        .id("focus-target-remove-fallback")
                        .debug_selector(|| "dialog-focus-sync:remove-fallback".to_owned())
                        .focusable()
                        .tab_stop(true)
                        .track_focus(&self.fallback_focus),
                );
            let dialog = Dialog::element(
                "focus-target-remove-dialog",
                "Open dialog",
                "Focus target removal",
                content,
            )
            .default_open(true)
            .initial_focus_intent(InitialFocusIntent::TargetOrFirstFocusable(
                FocusTargetId::new("preferred"),
            ));
            if self.include_preferred {
                dialog.focus_target(FocusTargetRegistration::new(
                    "preferred",
                    &self.preferred_focus,
                ))
            } else {
                dialog
            }
        }
    }

    let (view, cx) = cx.add_window_view(|_, cx| TestView {
        preferred_focus: cx.focus_handle(),
        fallback_focus: cx.focus_handle(),
        include_preferred: true,
    });
    cx.update(|window, cx| window.draw(cx).clear());
    cx.run_until_parked();
    assert!(cx.debug_selector_is_focused("dialog-focus-sync:removed-node"));

    cx.update_window_entity(&view, |view, _, cx| {
        view.include_preferred = false;
        cx.notify();
    });
    cx.update(|window, cx| window.draw(cx).clear());
    cx.run_until_parked();

    assert!(
        cx.debug_selector_is_focused("dialog-focus-sync:remove-fallback"),
        "removing the focused logical target must re-run the live initial-focus policy"
    );
}

#[open_gpui::test]
fn dialog_focus_target_sync_validates_retained_target_against_the_final_rendered_tree(
    cx: &mut open_gpui::TestAppContext,
) {
    struct TestView {
        preferred_focus: FocusHandle,
        fallback_focus: FocusHandle,
        show_preferred: bool,
    }

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let content = div()
                .children(self.show_preferred.then(|| {
                    div()
                        .id("focus-target-retained-preferred")
                        .debug_selector(|| "dialog-focus-sync:retained-preferred".to_owned())
                        .focusable()
                        .tab_stop(true)
                        .track_focus(&self.preferred_focus)
                }))
                .child(
                    div()
                        .id("focus-target-retained-fallback")
                        .debug_selector(|| "dialog-focus-sync:retained-fallback".to_owned())
                        .focusable()
                        .tab_stop(true)
                        .track_focus(&self.fallback_focus),
                );

            Dialog::element(
                "focus-target-retained-dialog",
                "Open dialog",
                "Retained focus target",
                content,
            )
            .default_open(true)
            .initial_focus_intent(InitialFocusIntent::TargetOrFirstFocusable(
                FocusTargetId::new("preferred"),
            ))
            .focus_target(FocusTargetRegistration::new(
                "preferred",
                &self.preferred_focus,
            ))
        }
    }

    let (view, cx) = cx.add_window_view(|_, cx| TestView {
        preferred_focus: cx.focus_handle(),
        fallback_focus: cx.focus_handle(),
        show_preferred: true,
    });
    cx.update(|window, cx| window.draw(cx).clear());
    cx.run_until_parked();
    assert!(cx.debug_selector_is_focused("dialog-focus-sync:retained-preferred"));

    cx.update_window_entity(&view, |view, _, cx| {
        view.show_preferred = false;
        cx.notify();
    });
    cx.update(|window, cx| window.draw(cx).clear());
    cx.run_until_parked();

    assert!(
        cx.debug_selector_is_focused("dialog-focus-sync:retained-fallback"),
        "a retained registration must be validated against the completed rendered tree"
    );
}

#[open_gpui::test]
fn dialog_exact_focus_target_becoming_disabled_safely_clears_focus(
    cx: &mut open_gpui::TestAppContext,
) {
    struct TestView {
        preferred_focus: FocusHandle,
        preferred_available: bool,
    }

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let availability = if self.preferred_available {
                FocusTargetAvailability::Available
            } else {
                FocusTargetAvailability::Disabled
            };

            Dialog::element(
                "focus-target-exact-dialog",
                "Open dialog",
                "Exact focus target",
                div()
                    .id("focus-target-exact-preferred")
                    .debug_selector(|| "dialog-focus-sync:exact-preferred".to_owned())
                    .focusable()
                    .tab_stop(true)
                    .track_focus(&self.preferred_focus),
            )
            .default_open(true)
            .initial_focus_intent(InitialFocusIntent::Target(FocusTargetId::new("preferred")))
            .focus_target(
                FocusTargetRegistration::new("preferred", &self.preferred_focus)
                    .with_availability(availability),
            )
        }
    }

    let (view, cx) = cx.add_window_view(|_, cx| TestView {
        preferred_focus: cx.focus_handle(),
        preferred_available: true,
    });
    cx.update(|window, cx| window.draw(cx).clear());
    cx.run_until_parked();
    assert!(cx.debug_selector_is_focused("dialog-focus-sync:exact-preferred"));

    cx.update_window_entity(&view, |view, _, cx| {
        view.preferred_available = false;
        cx.notify();
    });
    cx.update(|window, cx| window.draw(cx).clear());
    cx.run_until_parked();

    assert_eq!(
        cx.focused_debug_selector(),
        None,
        "an unavailable exact target has no fallback and must not retain focus"
    );
}

#[open_gpui::test]
fn dialog_runtime_respects_escape_policy_and_restores_trigger_focus(
    cx: &mut open_gpui::TestAppContext,
) {
    struct TestView {
        escape_policy: Rc<RefCell<EscapeKeyPolicy>>,
        open_events: Rc<RefCell<Vec<bool>>>,
    }

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let escape_policy = *self.escape_policy.borrow();
            let open_events = self.open_events.clone();

            div().size_full().child(
                Dialog::new("runtime-dialog", "Open dialog", "Runtime dialog", "Body")
                    .escape_key_policy(escape_policy)
                    .on_open_change(move |intent, _, _| {
                        open_events.borrow_mut().push(intent.desired_open());
                    }),
            )
        }
    }

    let escape_policy = Rc::new(RefCell::new(EscapeKeyPolicy::Ignore));
    let open_events = Rc::new(RefCell::new(Vec::new()));
    let (_, cx) = cx.add_window_view(|_, _| TestView {
        escape_policy: escape_policy.clone(),
        open_events: open_events.clone(),
    });
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    let trigger = cx
        .debug_bounds("dialog:runtime-dialog:trigger")
        .expect("dialog trigger should expose a stable debug selector");
    cx.simulate_click(trigger.center(), Default::default());
    cx.run_until_parked();
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });
    assert!(
        cx.debug_selector_is_focused("dialog:runtime-dialog:surface"),
        "opened dialog should move focus to the surface"
    );

    cx.simulate_keystrokes("escape");
    cx.run_until_parked();
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });
    assert!(
        cx.debug_bounds("dialog:runtime-dialog:surface").is_some(),
        "EscapeKeyPolicy::Ignore should keep dialog content mounted"
    );

    *escape_policy.borrow_mut() = EscapeKeyPolicy::Dismiss;
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });
    cx.simulate_keystrokes("escape");
    cx.run_until_parked();
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    assert!(
        cx.debug_bounds("dialog:runtime-dialog:surface").is_none(),
        "EscapeKeyPolicy::Dismiss should close dialog content"
    );
    assert!(
        cx.debug_selector_is_focused("dialog:runtime-dialog:trigger"),
        "Escape dismissal should restore focus to the dialog trigger"
    );
    assert_eq!(open_events.borrow().as_slice(), &[true, false]);
}

#[open_gpui::test]
fn controlled_dialog_refusal_keeps_modal_focus_authority(cx: &mut open_gpui::TestAppContext) {
    struct TestView {
        open_intents: Rc<RefCell<Vec<OverlayOpenIntent>>>,
        underlay_clicks: Rc<Cell<usize>>,
        underlay_focus: FocusHandle,
    }

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let open_intents = self.open_intents.clone();
            let underlay_clicks = self.underlay_clicks.clone();
            div()
                .relative()
                .size_full()
                .child(
                    div()
                        .id("controlled-dialog-underlay")
                        .debug_selector(|| "dialog-test:underlay".to_owned())
                        .absolute()
                        .left(px(4.0))
                        .top(px(180.0))
                        .w(px(96.0))
                        .h(px(32.0))
                        .focusable()
                        .track_focus(&self.underlay_focus)
                        .tab_index(0)
                        .on_click(move |_, _, _| {
                            underlay_clicks.set(underlay_clicks.get() + 1);
                        })
                        .child("Underlay"),
                )
                .child(
                    Dialog::new(
                        "controlled-dialog",
                        "Open dialog",
                        "Controlled dialog",
                        "Body",
                    )
                    .open(true)
                    .on_open_change(move |intent, _, _| {
                        open_intents.borrow_mut().push(intent);
                    }),
                )
        }
    }

    let open_intents = Rc::new(RefCell::new(Vec::new()));
    let underlay_clicks = Rc::new(Cell::new(0));
    let (_, cx) = cx.add_window_view(|_, cx| TestView {
        open_intents: open_intents.clone(),
        underlay_clicks: underlay_clicks.clone(),
        underlay_focus: cx.focus_handle(),
    });
    cx.update(|window, cx| window.draw(cx).clear());

    let surface = cx
        .debug_bounds("dialog:controlled-dialog:surface")
        .expect("controlled dialog should be mounted");
    cx.simulate_click(surface.center(), Default::default());
    assert!(cx.debug_selector_is_focused("dialog:controlled-dialog:surface"));

    cx.simulate_keystrokes("escape");
    cx.run_until_parked();
    cx.update(|window, cx| window.draw(cx).clear());

    assert_eq!(open_intents.borrow().len(), 1);
    assert!(!open_intents.borrow()[0].desired_open());
    assert_eq!(open_intents.borrow()[0].reason(), DismissReason::EscapeKey);
    let first_revision = open_intents.borrow()[0]
        .revision()
        .expect("controlled close should carry a revision");
    assert!(
        cx.debug_bounds("dialog:controlled-dialog:surface")
            .is_some(),
        "controlled refusal should keep the dialog mounted"
    );
    assert!(
        cx.debug_selector_is_focused("dialog:controlled-dialog:surface"),
        "a close intent must not restore focus before the owner commits closed"
    );

    cx.simulate_keystrokes("escape");
    cx.simulate_keystrokes("tab");
    cx.update(|window, cx| window.draw(cx).clear());
    assert_eq!(
        open_intents.borrow().len(),
        1,
        "a pending controlled close intent must be emitted only once"
    );
    assert!(
        cx.debug_selector_is_focused("dialog:controlled-dialog:surface"),
        "controlled refusal must keep the modal focus scope active"
    );

    let underlay = cx
        .debug_bounds("dialog-test:underlay")
        .expect("underlay probe should render");
    cx.simulate_click(underlay.center(), Default::default());
    assert_eq!(
        underlay_clicks.get(),
        0,
        "controlled refusal must keep the modal pointer barrier active"
    );
    assert!(
        cx.debug_selector_is_focused("dialog:controlled-dialog:surface"),
        "a blocked underlay press must not move focus out of the dialog"
    );

    let first_intent = open_intents.borrow()[0].clone();
    cx.update(|window, cx| {
        first_intent
            .reject(window, cx)
            .expect("the owner should reject the exact pending intent");
    });
    let reopened = cx.update(|window, cx| {
        WindowOverlayRuntime::for_window(window, cx)
            .snapshot(window, cx)
            .expect("rejected dialog snapshot should resolve")
    });
    assert_eq!(
        reopened
            .layers()
            .iter()
            .find(|layer| layer.id().as_str() == "dialog:controlled-dialog")
            .expect("rejected dialog should remain registered")
            .phase(),
        OverlayLayerPhase::Open
    );

    cx.simulate_keystrokes("escape");
    assert_eq!(open_intents.borrow().len(), 2);
    let second_revision = open_intents.borrow()[1]
        .revision()
        .expect("a repeated controlled close should carry a revision");
    assert_ne!(first_revision, second_revision);
}

#[open_gpui::test]
fn presentation_inert_dialog_keeps_open_lifecycle_but_releases_modal_input_authority(
    cx: &mut open_gpui::TestAppContext,
) {
    struct TestView {
        presentation: SubtreePresentation,
        open_intents: Rc<RefCell<Vec<OverlayOpenIntent>>>,
        underlay_clicks: Rc<Cell<usize>>,
    }

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let underlay_clicks = self.underlay_clicks.clone();
            let open_intents = self.open_intents.clone();
            div()
                .relative()
                .size_full()
                .child(
                    div()
                        .id("inert-dialog-underlay")
                        .debug_selector(|| "dialog-test:inert-underlay".to_owned())
                        .absolute()
                        .left(px(4.0))
                        .top(px(180.0))
                        .w(px(96.0))
                        .h(px(32.0))
                        .on_click(move |_, _, _| {
                            underlay_clicks.set(underlay_clicks.get() + 1);
                        })
                        .child("Underlay"),
                )
                .child(
                    Dialog::new("inert-dialog", "Open dialog", "Inert dialog", "Body")
                        .open(true)
                        .on_open_change(move |intent, _, _| {
                            open_intents.borrow_mut().push(intent);
                        })
                        .with_subtree_presentation(self.presentation),
                )
        }
    }

    let open_intents = Rc::new(RefCell::new(Vec::new()));
    let underlay_clicks = Rc::new(Cell::new(0));
    let (view, cx) = cx.add_window_view(|_, _| TestView {
        presentation: SubtreePresentation::Visible,
        open_intents: open_intents.clone(),
        underlay_clicks: underlay_clicks.clone(),
    });
    cx.update(|window, cx| window.draw(cx).clear());
    assert!(cx.debug_selector_is_focused("dialog:inert-dialog:surface"));

    cx.update_window_entity(&view, |view, _, cx| {
        view.presentation = SubtreePresentation::Inert;
        cx.notify();
    });
    cx.update(|window, cx| window.draw(cx).clear());
    cx.run_until_parked();

    let inert = cx.update(|window, cx| {
        WindowOverlayRuntime::for_window(window, cx)
            .snapshot(window, cx)
            .unwrap()
    });
    let inert = inert
        .layers()
        .iter()
        .find(|layer| layer.id().as_str() == "dialog:inert-dialog")
        .expect("inert dialog should remain registered and open");
    assert_eq!(inert.phase(), OverlayLayerPhase::Open);
    assert_eq!(inert.presence(), OverlayPresence::open());
    assert_eq!(inert.presentation(), SubtreePresentation::Inert);
    assert!(!inert.keyboard_eligible());
    assert!(!inert.modal_pointer_barrier());
    assert!(!inert.focus_active());

    let underlay = cx
        .debug_bounds("dialog-test:inert-underlay")
        .expect("visible underlay should remain mounted");
    cx.simulate_click(underlay.center(), Default::default());
    assert_eq!(underlay_clicks.get(), 1);
    cx.simulate_keystrokes("escape");
    assert!(
        open_intents.borrow().is_empty(),
        "an inert open dialog must not consume Escape or emit close intent"
    );

    cx.update_window_entity(&view, |view, _, cx| {
        view.presentation = SubtreePresentation::Visible;
        cx.notify();
    });
    cx.update(|window, cx| window.draw(cx).clear());
    cx.run_until_parked();
    assert!(
        !cx.debug_selector_is_focused("dialog:inert-dialog:surface"),
        "restoring presentation must not replay the opening focus claim"
    );
    let restored = cx.update(|window, cx| {
        WindowOverlayRuntime::for_window(window, cx)
            .snapshot(window, cx)
            .unwrap()
    });
    let restored = restored
        .layers()
        .iter()
        .find(|layer| layer.id().as_str() == "dialog:inert-dialog")
        .expect("restored dialog should remain registered");
    assert!(restored.focus_active());
    cx.simulate_keystrokes("tab");
    assert!(
        cx.debug_selector_is_focused("dialog:inert-dialog:surface"),
        "user traversal should resume the visible modal focus loop"
    );
    cx.simulate_keystrokes("escape");
    assert_eq!(open_intents.borrow().len(), 1);
    assert_eq!(open_intents.borrow()[0].reason(), DismissReason::EscapeKey);
}

#[open_gpui::test]
fn presentation_menu_releases_input_authority_without_replaying_opening_focus(
    cx: &mut open_gpui::TestAppContext,
) {
    struct TestView {
        presentation: SubtreePresentation,
        open_intents: Rc<RefCell<Vec<OverlayOpenIntent>>>,
    }

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let open_intents = self.open_intents.clone();
            div().size_full().child(
                Menu::new("presentation-menu", "Presentation menu")
                    .open(true)
                    .default_focused_value("action")
                    .item(MenuItem::action("action", "Action"))
                    .on_open_change(move |intent, _, _| {
                        open_intents.borrow_mut().push(intent);
                    })
                    .with_subtree_presentation(self.presentation),
            )
        }
    }

    let open_intents = Rc::new(RefCell::new(Vec::new()));
    let (view, cx) = cx.add_window_view(|_, _| TestView {
        presentation: SubtreePresentation::Visible,
        open_intents: open_intents.clone(),
    });
    cx.update(|window, cx| window.draw(cx).clear());
    assert!(cx.debug_bounds("menu:presentation-menu:content").is_some());

    let trigger = cx
        .debug_bounds("menu:presentation-menu:trigger")
        .expect("the controlled menu trigger should render");
    cx.simulate_click(trigger.center(), Default::default());
    let stale_intent = open_intents
        .borrow()
        .first()
        .expect("the visible trigger should request a controlled close")
        .clone();
    let stale_revision = stale_intent
        .revision()
        .expect("controlled close intents should carry a revision");
    assert_eq!(stale_intent.reason(), DismissReason::Trigger);

    let requested = cx.update(|window, cx| {
        WindowOverlayRuntime::for_window(window, cx)
            .snapshot(window, cx)
            .unwrap()
    });
    let requested = requested
        .layers()
        .iter()
        .find(|layer| layer.id().as_str() == "menu:presentation-menu")
        .expect("the controlled menu should retain its requested registration");
    assert_eq!(requested.phase(), OverlayLayerPhase::CloseRequested);
    assert_eq!(requested.pending_intent(), Some(DismissReason::Trigger));

    for presentation in [SubtreePresentation::Inert, SubtreePresentation::Hidden] {
        cx.update_window_entity(&view, |view, _, cx| {
            view.presentation = presentation;
            cx.notify();
        });
        cx.update(|window, cx| window.draw(cx).clear());
        cx.run_until_parked();

        let snapshot = cx.update(|window, cx| {
            WindowOverlayRuntime::for_window(window, cx)
                .snapshot(window, cx)
                .unwrap()
        });
        let layer = snapshot
            .layers()
            .iter()
            .find(|layer| layer.id().as_str() == "menu:presentation-menu")
            .expect("the controlled menu should retain its reusable registration");
        assert_eq!(layer.phase(), OverlayLayerPhase::Open);
        assert_eq!(layer.pending_intent(), None);
        assert_eq!(layer.presentation(), presentation);
        assert!(!layer.keyboard_eligible());
        assert!(!layer.focus_active());
        cx.simulate_keystrokes("escape");
        assert_eq!(open_intents.borrow().len(), 1);
    }

    let stale_rejection = cx.update(|window, cx| stale_intent.reject(window, cx));
    assert!(matches!(
        stale_rejection,
        Err(WindowOverlayRuntimeError::StaleIntent(_))
    ));

    cx.update_window_entity(&view, |view, _, cx| {
        view.presentation = SubtreePresentation::Visible;
        cx.notify();
    });
    cx.update(|window, cx| window.draw(cx).clear());
    cx.run_until_parked();
    assert!(cx.update(|window, cx| window.focused(cx).is_none()));

    let restored = cx.update(|window, cx| {
        WindowOverlayRuntime::for_window(window, cx)
            .snapshot(window, cx)
            .unwrap()
    });
    let restored = restored
        .layers()
        .iter()
        .find(|layer| layer.id().as_str() == "menu:presentation-menu")
        .expect("the restored menu should remain registered");
    assert!(restored.keyboard_eligible());
    assert!(restored.focus_active());
    cx.simulate_click(trigger.center(), Default::default());
    assert_eq!(open_intents.borrow().len(), 2);
    assert_eq!(open_intents.borrow()[1].reason(), DismissReason::Trigger);
    assert_ne!(open_intents.borrow()[1].revision(), Some(stale_revision));
}

#[open_gpui::test]
fn overlay_fleet_can_switch_controlled_ownership_without_remounting(
    cx: &mut open_gpui::TestAppContext,
) {
    struct TestView {
        controlled: bool,
        open: bool,
    }

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let dialog = Dialog::new(
                "ownership-dialog",
                "Dialog",
                "Ownership dialog",
                "Dialog body",
            );
            let popover = Popover::new("ownership-popover", "Popover", "Popover body");
            let menu =
                Menu::new("ownership-menu", "Menu").item(MenuItem::action("action", "Action"));

            let dialog = if self.controlled {
                dialog.open(self.open)
            } else {
                dialog
            };
            let popover = if self.controlled {
                popover.open(self.open)
            } else {
                popover
            };
            let menu = if self.controlled {
                menu.open(self.open)
            } else {
                menu
            };

            div().size_full().child(dialog).child(popover).child(menu)
        }
    }

    let (view, cx) = cx.add_window_view(|_, _| TestView {
        controlled: false,
        open: false,
    });
    cx.update(|window, cx| window.draw(cx).clear());

    cx.update_window_entity(&view, |view, _, cx| {
        view.controlled = true;
        view.open = true;
        cx.notify();
    });
    cx.update(|window, cx| window.draw(cx).clear());
    cx.simulate_keystrokes("escape");

    let requested = cx.update(|window, cx| {
        WindowOverlayRuntime::for_window(window, cx)
            .snapshot(window, cx)
            .expect("controlled overlay snapshot should resolve")
    });
    assert_eq!(
        requested
            .layers()
            .iter()
            .find(|layer| layer.id().as_str() == "menu:ownership-menu")
            .expect("controlled Menu should remain registered")
            .phase(),
        OverlayLayerPhase::CloseRequested,
    );

    cx.update_window_entity(&view, |view, _, cx| {
        view.controlled = false;
        cx.notify();
    });
    cx.update(|window, cx| window.draw(cx).clear());

    let switched = cx.update(|window, cx| {
        WindowOverlayRuntime::for_window(window, cx)
            .snapshot(window, cx)
            .expect("uncontrolled overlay snapshot should resolve")
    });
    for id in [
        "dialog:ownership-dialog",
        "popover:ownership-popover",
        "menu:ownership-menu",
    ] {
        assert_eq!(
            switched
                .layers()
                .iter()
                .find(|layer| layer.id().as_str() == id)
                .unwrap_or_else(|| panic!("missing switched overlay layer `{id}`"))
                .phase(),
            OverlayLayerPhase::Open,
            "switching to uncontrolled ownership should adopt committed presence and clear stale intent",
        );
    }

    cx.update_window_entity(&view, |view, _, cx| {
        view.controlled = true;
        view.open = false;
        cx.notify();
    });
    cx.update(|window, cx| window.draw(cx).clear());
}

#[open_gpui::test]
fn popover_menu_dialog_escape_is_lifo_and_restores_focus_through_real_components(
    cx: &mut open_gpui::TestAppContext,
) {
    struct TestView {
        dialog_open: bool,
        popover_events: Rc<RefCell<Vec<bool>>>,
        menu_events: Rc<RefCell<Vec<bool>>>,
        dialog_events: Rc<RefCell<Vec<bool>>>,
        popover_inside_clicks: Rc<Cell<usize>>,
    }

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            let popover_events = self.popover_events.clone();
            let menu_events = self.menu_events.clone();
            let dialog_events = self.dialog_events.clone();
            let popover_inside_clicks = self.popover_inside_clicks.clone();
            let view = cx.entity().downgrade();

            div()
                .relative()
                .size_full()
                .child(
                    div()
                        .id("lifo-outside-target")
                        .debug_selector(|| "overlay-test:lifo-outside".to_owned())
                        .absolute()
                        .left(px(520.0))
                        .top(px(320.0))
                        .w(px(120.0))
                        .h(px(40.0))
                        .child("Outside"),
                )
                .child(
                    Popover::element(
                        "lifo-popover",
                        "Open popover",
                        div()
                            .flex()
                            .flex_col()
                            .gap_2()
                            .child(
                                Menu::new("lifo-menu", "Open menu")
                                    .item(MenuItem::action("action", "Action"))
                                    .placement(
                                        OverlayPlacementSide::Right,
                                        OverlayPlacementAlignment::Start,
                                    )
                                    .on_open_change(move |intent, _, _| {
                                        menu_events.borrow_mut().push(intent.desired_open());
                                    })
                                    .overlay_child(
                                        Dialog::new(
                                            "lifo-dialog",
                                            "Dialog trigger",
                                            "Nested dialog",
                                            "Dialog body",
                                        )
                                        .open(self.dialog_open)
                                        .on_open_change(
                                            move |intent, _, cx| {
                                                let open = intent.desired_open();
                                                dialog_events.borrow_mut().push(open);
                                                view.update(cx, |view, cx| {
                                                    view.dialog_open = open;
                                                    cx.notify();
                                                })
                                                .ok();
                                            },
                                        ),
                                    ),
                            )
                            .child(
                                div()
                                    .id("lifo-popover-inside")
                                    .debug_selector(|| {
                                        "overlay-test:lifo-popover-inside".to_owned()
                                    })
                                    .on_click(move |_, _, _| {
                                        popover_inside_clicks.set(popover_inside_clicks.get() + 1);
                                    })
                                    .child("Inside popover"),
                            ),
                    )
                    .on_open_change(move |intent, _, _| {
                        popover_events.borrow_mut().push(intent.desired_open());
                    }),
                )
        }
    }

    let popover_events = Rc::new(RefCell::new(Vec::new()));
    let menu_events = Rc::new(RefCell::new(Vec::new()));
    let dialog_events = Rc::new(RefCell::new(Vec::new()));
    let popover_inside_clicks = Rc::new(Cell::new(0));
    let (_, cx) = cx.add_window_view(|_, _| TestView {
        dialog_open: false,
        popover_events: popover_events.clone(),
        menu_events: menu_events.clone(),
        dialog_events: dialog_events.clone(),
        popover_inside_clicks: popover_inside_clicks.clone(),
    });
    cx.update(|window, cx| window.draw(cx).clear());

    let popover_trigger = cx
        .debug_bounds("popover:lifo-popover:trigger")
        .expect("popover trigger should render");
    cx.simulate_click(popover_trigger.center(), Default::default());
    cx.update(|window, cx| window.draw(cx).clear());

    let menu_trigger = cx
        .debug_bounds("menu:lifo-menu:trigger")
        .expect("menu trigger should render inside the popover");
    cx.simulate_click(menu_trigger.center(), Default::default());
    cx.update(|window, cx| window.draw(cx).clear());
    assert!(cx.debug_selector_is_focused("menu:lifo-menu:content"));

    let dialog_trigger = cx
        .debug_bounds("dialog:lifo-dialog:trigger")
        .expect("dialog trigger should render inside the menu");
    cx.simulate_click(dialog_trigger.center(), Default::default());
    cx.run_until_parked();
    cx.update(|window, cx| window.draw(cx).clear());
    cx.update(|window, cx| {
        window.drain_next_frame_callbacks_for_test(cx);
        window.draw(cx).clear();
    });
    cx.run_until_parked();
    assert_eq!(
        dialog_events.borrow().as_slice(),
        &[true],
        "the inline Dialog trigger should reach its owner exactly once; menu events={:?}",
        menu_events.borrow().as_slice()
    );
    assert_eq!(
        menu_events.borrow().as_slice(),
        &[true],
        "opening the logical child must not dismiss its Menu parent"
    );
    assert!(cx.debug_bounds("dialog:lifo-dialog:surface").is_some());
    assert!(
        cx.debug_selector_is_focused("dialog:lifo-dialog:surface"),
        "nested Dialog should claim focus after its opening frame; focused={:?}",
        cx.focused_debug_selector()
    );

    let snapshot = cx.update(|window, cx| {
        WindowOverlayRuntime::for_window(window, cx)
            .snapshot(window, cx)
            .expect("nested overlay snapshot should resolve")
    });
    let active_layers = snapshot
        .layers()
        .iter()
        .filter(|layer| layer.phase() == OverlayLayerPhase::Open)
        .map(|layer| {
            (
                layer.id().as_str(),
                layer.kind(),
                layer.parent().map(|parent| parent.as_str()),
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(
        active_layers,
        vec![
            (
                "popover:lifo-popover",
                OverlayLayerKind::NonModalDismissible,
                None,
            ),
            (
                "menu:lifo-menu",
                OverlayLayerKind::Menu,
                Some("popover:lifo-popover"),
            ),
            (
                "dialog:lifo-dialog",
                OverlayLayerKind::Modal,
                Some("menu:lifo-menu"),
            ),
        ]
    );
    assert!(
        snapshot
            .layers()
            .iter()
            .find(|layer| layer.id().as_str() == "popover:lifo-popover")
            .is_some_and(|layer| layer.focus_entered()),
        "focus owned by nested layers should arm the popover's conditional restore"
    );

    cx.simulate_click(dialog_trigger.center(), Default::default());
    cx.run_until_parked();
    cx.update(|window, cx| window.draw(cx).clear());
    assert!(cx.debug_bounds("dialog:lifo-dialog:surface").is_none());
    assert!(cx.debug_bounds("menu:lifo-menu:content").is_some());
    assert!(cx.debug_bounds("popover:lifo-popover:content").is_some());
    assert!(cx.debug_selector_is_focused("menu:lifo-menu:content"));

    let popover_inside = cx
        .debug_bounds("overlay-test:lifo-popover-inside")
        .expect("popover sibling should remain rendered outside the menu");
    cx.simulate_click(popover_inside.center(), Default::default());
    cx.run_until_parked();
    cx.update(|window, cx| window.draw(cx).clear());
    assert!(cx.debug_bounds("menu:lifo-menu:content").is_none());
    assert!(cx.debug_bounds("popover:lifo-popover:content").is_some());
    assert!(
        cx.debug_selector_is_focused("menu:lifo-menu:trigger"),
        "outside dismissal should restore the Menu trigger inside the Popover"
    );
    assert_eq!(
        popover_inside_clicks.get(),
        0,
        "the Menu outside policy should consume the click before the Popover sibling"
    );

    let outside = cx
        .debug_bounds("overlay-test:lifo-outside")
        .expect("outside target should remain rendered below the overlay tree");
    cx.simulate_click(outside.center(), Default::default());
    cx.run_until_parked();
    cx.update(|window, cx| window.draw(cx).clear());
    assert!(cx.debug_bounds("popover:lifo-popover:content").is_none());
    assert!(
        cx.debug_selector_is_focused("popover:lifo-popover:trigger"),
        "outside dismissal should restore the Popover trigger after descendant focus entered"
    );

    cx.simulate_click(popover_trigger.center(), Default::default());
    cx.update(|window, cx| window.draw(cx).clear());
    let menu_trigger = cx
        .debug_bounds("menu:lifo-menu:trigger")
        .expect("menu trigger should render after reopening the popover");
    cx.simulate_click(menu_trigger.center(), Default::default());
    cx.update(|window, cx| window.draw(cx).clear());
    let dialog_trigger = cx
        .debug_bounds("dialog:lifo-dialog:trigger")
        .expect("dialog trigger should render after reopening the menu");
    cx.simulate_click(dialog_trigger.center(), Default::default());
    cx.run_until_parked();
    cx.update(|window, cx| window.draw(cx).clear());
    cx.update(|window, cx| {
        window.drain_next_frame_callbacks_for_test(cx);
        window.draw(cx).clear();
    });
    cx.run_until_parked();
    assert!(cx.debug_bounds("dialog:lifo-dialog:surface").is_some());
    assert!(
        cx.debug_selector_is_focused("dialog:lifo-dialog:surface"),
        "reopened nested Dialog should claim focus after its opening frame; focused={:?}",
        cx.focused_debug_selector()
    );

    cx.simulate_keystrokes("escape");
    cx.run_until_parked();
    cx.update(|window, cx| window.draw(cx).clear());
    assert!(cx.debug_bounds("dialog:lifo-dialog:surface").is_none());
    assert!(cx.debug_bounds("menu:lifo-menu:content").is_some());
    assert!(
        cx.debug_selector_is_focused("menu:lifo-menu:content"),
        "closing the dialog should restore focus inside the open menu"
    );

    cx.simulate_keystrokes("escape");
    cx.run_until_parked();
    cx.update(|window, cx| window.draw(cx).clear());
    assert!(cx.debug_bounds("menu:lifo-menu:content").is_none());
    assert!(
        cx.debug_selector_is_focused("menu:lifo-menu:trigger"),
        "closing the menu should restore its trigger inside the popover"
    );

    cx.simulate_keystrokes("escape");
    cx.run_until_parked();
    cx.update(|window, cx| {
        window.drain_next_frame_callbacks_for_test(cx);
        window.draw(cx).clear();
    });
    cx.update(|window, cx| {
        window.drain_next_frame_callbacks_for_test(cx);
    });
    cx.run_until_parked();
    assert!(
        cx.debug_bounds("popover:lifo-popover:content").is_none(),
        "the final Escape should close the popover"
    );
    assert_eq!(
        cx.focused_debug_selector().as_deref(),
        Some("popover:lifo-popover:trigger"),
        "closing the popover should restore its trigger",
    );
    assert_eq!(
        popover_events.borrow().as_slice(),
        &[true, false, true, false]
    );
    assert_eq!(menu_events.borrow().as_slice(), &[true, false, true, false]);
    assert_eq!(
        dialog_events.borrow().as_slice(),
        &[true, false, true, false]
    );
}

#[open_gpui::test]
fn alert_dialog_above_dialog_receives_escape_first_and_restores_focus_lifo(
    cx: &mut open_gpui::TestAppContext,
) {
    struct TestView {
        dialog_open: bool,
        dialog_events: Rc<RefCell<Vec<bool>>>,
        alert_events: Rc<RefCell<Vec<bool>>>,
    }

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            let dialog_events = self.dialog_events.clone();
            let alert_events = self.alert_events.clone();
            let view = cx.entity().downgrade();

            Dialog::element(
                "stacked-dialog",
                "Open dialog",
                "Dialog",
                AlertDialog::new(
                    "stacked-alert",
                    "Open alert dialog",
                    "Confirm action?",
                    "The alert dialog is above the dialog in the window overlay stack.",
                    "Continue",
                )
                .escape_key_policy(EscapeKeyPolicy::Dismiss)
                .on_open_change(move |intent, _, _| {
                    alert_events.borrow_mut().push(intent.desired_open());
                }),
            )
            .open(self.dialog_open)
            .on_open_change(move |intent, _, cx| {
                let open = intent.desired_open();
                dialog_events.borrow_mut().push(open);
                view.update(cx, |view, cx| {
                    view.dialog_open = open;
                    cx.notify();
                })
                .ok();
            })
        }
    }

    let dialog_events = Rc::new(RefCell::new(Vec::new()));
    let alert_events = Rc::new(RefCell::new(Vec::new()));
    let (_, cx) = cx.add_window_view(|_, _| TestView {
        dialog_open: true,
        dialog_events: dialog_events.clone(),
        alert_events: alert_events.clone(),
    });
    cx.update(|window, cx| window.draw(cx).clear());

    let alert_trigger = cx
        .debug_bounds("alert-dialog:stacked-alert:trigger")
        .expect("alert trigger should render inside the dialog");
    cx.simulate_click(alert_trigger.center(), Default::default());
    cx.run_until_parked();
    cx.update(|window, cx| window.draw(cx).clear());
    assert!(
        cx.debug_selector_is_focused("alert-dialog:stacked-alert:cancel"),
        "alert should own focus above the dialog; focused={:?}",
        cx.focused_debug_selector()
    );
    assert_eq!(alert_events.borrow().as_slice(), &[true]);
    assert!(dialog_events.borrow().is_empty());

    cx.simulate_keystrokes("escape");
    cx.run_until_parked();
    cx.update(|window, cx| window.draw(cx).clear());

    assert!(
        cx.debug_bounds("alert-dialog:stacked-alert:surface")
            .is_none(),
        "the first Escape should close the alert dialog"
    );
    assert!(
        cx.debug_bounds("dialog:stacked-dialog:surface").is_some(),
        "the dialog must remain open below the alert"
    );
    assert_eq!(alert_events.borrow().as_slice(), &[true, false]);
    assert!(
        dialog_events.borrow().is_empty(),
        "the lower dialog must not intercept Escape owned by the alert dialog"
    );
    assert!(
        cx.debug_selector_is_focused("alert-dialog:stacked-alert:trigger"),
        "closing the alert dialog should restore its trigger inside the dialog"
    );

    cx.simulate_keystrokes("escape");
    cx.run_until_parked();
    cx.update(|window, cx| window.draw(cx).clear());

    assert!(
        cx.debug_bounds("dialog:stacked-dialog:surface").is_none(),
        "the second Escape should close the lower dialog"
    );
    assert!(
        cx.debug_selector_is_focused("dialog:stacked-dialog:trigger"),
        "closing the dialog should restore its trigger"
    );
    assert_eq!(dialog_events.borrow().as_slice(), &[false]);
}

#[open_gpui::test]
fn alert_dialog_ignores_escape_by_default(cx: &mut open_gpui::TestAppContext) {
    struct TestView {
        open_events: Rc<RefCell<Vec<bool>>>,
    }

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let open_events = self.open_events.clone();
            AlertDialog::new(
                "default-escape-policy",
                "Open alert dialog",
                "Confirm action?",
                "Escape must not dismiss an alert dialog unless explicitly enabled.",
                "Continue",
            )
            .default_open(true)
            .on_open_change(move |intent, _, _| {
                open_events.borrow_mut().push(intent.desired_open());
            })
        }
    }

    let open_events = Rc::new(RefCell::new(Vec::new()));
    let (_, cx) = cx.add_window_view(|_, _| TestView {
        open_events: open_events.clone(),
    });
    cx.update(|window, cx| window.draw(cx).clear());
    assert!(
        cx.debug_bounds("alert-dialog:default-escape-policy:surface")
            .is_some()
    );

    cx.simulate_keystrokes("escape");
    cx.run_until_parked();
    cx.update(|window, cx| window.draw(cx).clear());

    assert!(
        cx.debug_bounds("alert-dialog:default-escape-policy:surface")
            .is_some(),
        "the default AlertDialog Escape policy must keep the surface open"
    );
    assert!(
        open_events.borrow().is_empty(),
        "ignored Escape must not emit an open-change request"
    );
}

#[open_gpui::test]
fn alert_dialog_cancel_effect_may_synchronously_unmount_owner(cx: &mut open_gpui::TestAppContext) {
    struct TestView {
        mounted: bool,
        observed_phases: Rc<RefCell<Vec<OverlayLayerPhase>>>,
    }

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            if self.mounted {
                let observed_phases = self.observed_phases.clone();
                let view = cx.entity().downgrade();
                div().size_full().child(
                    AlertDialog::new(
                        "cancel-unmount",
                        "Open alert",
                        "Unmount this alert?",
                        "The cancel handler removes its owner immediately.",
                        "Continue",
                    )
                    .default_open(true)
                    .on_cancel(move |window, cx| {
                        let phase = WindowOverlayRuntime::for_window(window, cx)
                            .snapshot(window, cx)
                            .expect("cancel effect should observe its window runtime")
                            .layers()
                            .iter()
                            .find(|layer| layer.id().as_str() == "alert-dialog:cancel-unmount")
                            .expect("alert dialog should remain registered during its effect")
                            .phase();
                        observed_phases.borrow_mut().push(phase);
                        view.update(cx, |view, cx| {
                            view.mounted = false;
                            cx.notify();
                        })
                        .expect("test view should remain mounted");
                        window.draw(cx).clear();
                    }),
                )
            } else {
                div().size_full().child("Alert owner unmounted")
            }
        }
    }

    let observed_phases = Rc::new(RefCell::new(Vec::new()));
    let (_, cx) = cx.add_window_view(|_, _| TestView {
        mounted: true,
        observed_phases: observed_phases.clone(),
    });
    cx.update(|window, cx| window.draw(cx).clear());

    let cancel = cx
        .debug_bounds("alert-dialog:cancel-unmount:cancel")
        .expect("default-open alert dialog should render its cancel action");
    cx.simulate_click(cancel.center(), Default::default());
    cx.run_until_parked();

    assert_eq!(
        observed_phases.borrow().as_slice(),
        &[OverlayLayerPhase::Closing],
        "cancel effects must observe the committed close before they can unmount the owner"
    );
    assert!(
        cx.debug_bounds("alert-dialog:cancel-unmount:surface")
            .is_none(),
        "the cancel effect should synchronously unmount the alert dialog"
    );
}

#[open_gpui::test]
fn alert_dialog_action_effect_may_synchronously_replace_owner_overlay(
    cx: &mut open_gpui::TestAppContext,
) {
    struct TestView {
        replacement: bool,
        observed_phases: Rc<RefCell<Vec<OverlayLayerPhase>>>,
    }

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            if self.replacement {
                div().size_full().child(
                    Dialog::new(
                        "alert-replacement",
                        "Open replacement",
                        "Replacement dialog",
                        "The alert action installed this dialog synchronously.",
                    )
                    .default_open(true),
                )
            } else {
                let observed_phases = self.observed_phases.clone();
                let view = cx.entity().downgrade();
                div().size_full().child(
                    AlertDialog::new(
                        "action-replace",
                        "Open alert",
                        "Replace this alert?",
                        "The action handler swaps the active overlay immediately.",
                        "Replace",
                    )
                    .default_open(true)
                    .on_action(move |window, cx| {
                        let phase = WindowOverlayRuntime::for_window(window, cx)
                            .snapshot(window, cx)
                            .expect("action effect should observe its window runtime")
                            .layers()
                            .iter()
                            .find(|layer| layer.id().as_str() == "alert-dialog:action-replace")
                            .expect("alert dialog should remain registered during its effect")
                            .phase();
                        observed_phases.borrow_mut().push(phase);
                        view.update(cx, |view, cx| {
                            view.replacement = true;
                            cx.notify();
                        })
                        .expect("test view should remain mounted");
                        window.draw(cx).clear();
                    }),
                )
            }
        }
    }

    let observed_phases = Rc::new(RefCell::new(Vec::new()));
    let (_, cx) = cx.add_window_view(|_, _| TestView {
        replacement: false,
        observed_phases: observed_phases.clone(),
    });
    cx.update(|window, cx| window.draw(cx).clear());

    let action = cx
        .debug_bounds("alert-dialog:action-replace:action")
        .expect("default-open alert dialog should render its primary action");
    cx.simulate_click(action.center(), Default::default());
    cx.run_until_parked();

    assert_eq!(
        observed_phases.borrow().as_slice(),
        &[OverlayLayerPhase::Closing]
    );
    assert!(
        cx.debug_bounds("alert-dialog:action-replace:surface")
            .is_none()
    );
    assert!(
        cx.debug_bounds("dialog:alert-replacement:surface")
            .is_some()
    );
    let snapshot = cx.update(|window, cx| {
        WindowOverlayRuntime::for_window(window, cx)
            .snapshot(window, cx)
            .expect("replacement dialog snapshot should resolve")
    });
    assert_eq!(
        snapshot
            .layers()
            .iter()
            .find(|layer| layer.id().as_str() == "dialog:alert-replacement")
            .expect("replacement dialog should own a registered layer")
            .phase(),
        OverlayLayerPhase::Open
    );
}

#[open_gpui::test]
fn alert_dialog_action_effect_reopen_wins_over_closing_generation(
    cx: &mut open_gpui::TestAppContext,
) {
    struct TestView {
        controlled_reopen: bool,
        observed_phases: Rc<RefCell<Vec<OverlayLayerPhase>>>,
    }

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            let observed_phases = self.observed_phases.clone();
            let view = cx.entity().downgrade();
            let alert = AlertDialog::new(
                "action-reopen",
                "Open alert",
                "Reopen this alert?",
                "The action handler reopens the same layer during its closing effect.",
                "Reopen",
            )
            .default_open(true)
            .on_action(move |window, cx| {
                let phase = WindowOverlayRuntime::for_window(window, cx)
                    .snapshot(window, cx)
                    .expect("reopen effect should observe its window runtime")
                    .layers()
                    .iter()
                    .find(|layer| layer.id().as_str() == "alert-dialog:action-reopen")
                    .expect("alert dialog should remain registered during its effect")
                    .phase();
                observed_phases.borrow_mut().push(phase);
                view.update(cx, |view, cx| {
                    view.controlled_reopen = true;
                    cx.notify();
                })
                .expect("test view should remain mounted");
                window.draw(cx).clear();
            });
            let alert = if self.controlled_reopen {
                alert.open(true)
            } else {
                alert
            };
            div().size_full().child(alert)
        }
    }

    let observed_phases = Rc::new(RefCell::new(Vec::new()));
    let (_, cx) = cx.add_window_view(|_, _| TestView {
        controlled_reopen: false,
        observed_phases: observed_phases.clone(),
    });
    cx.update(|window, cx| window.draw(cx).clear());

    let action = cx
        .debug_bounds("alert-dialog:action-reopen:action")
        .expect("default-open alert dialog should render its primary action");
    cx.simulate_click(action.center(), Default::default());
    cx.run_until_parked();

    assert_eq!(
        observed_phases.borrow().as_slice(),
        &[OverlayLayerPhase::Closing]
    );
    assert!(
        cx.debug_bounds("alert-dialog:action-reopen:surface")
            .is_some(),
        "the reentrant open must supersede the older closing generation"
    );
    let snapshot = cx.update(|window, cx| {
        WindowOverlayRuntime::for_window(window, cx)
            .snapshot(window, cx)
            .expect("reopened alert dialog snapshot should resolve")
    });
    assert_eq!(
        snapshot
            .layers()
            .iter()
            .find(|layer| layer.id().as_str() == "alert-dialog:action-reopen")
            .expect("reopened alert dialog should retain its registration")
            .phase(),
        OverlayLayerPhase::Open
    );
    assert!(
        cx.debug_selector_is_focused("alert-dialog:action-reopen:cancel"),
        "the reopened generation should own modal focus"
    );
}

#[open_gpui::test]
fn popover_outside_pass_through_preserves_focus_when_surface_never_owned_it(
    cx: &mut open_gpui::TestAppContext,
) {
    struct TestView {
        outside_focus: FocusHandle,
        outside_clicks: Rc<Cell<usize>>,
    }

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let outside_clicks = self.outside_clicks.clone();
            div()
                .size_full()
                .child(
                    div()
                        .id("popover-outside-focus")
                        .debug_selector(|| "popover-test:outside".to_owned())
                        .focusable()
                        .track_focus(&self.outside_focus)
                        .tab_index(0)
                        .on_click(move |_, _, _| {
                            outside_clicks.set(outside_clicks.get() + 1);
                        })
                        .child("Outside"),
                )
                .child(Popover::new(
                    "conditional-restore",
                    "Open popover",
                    "Popover body",
                ))
        }
    }

    let outside_clicks = Rc::new(Cell::new(0));
    let (_, cx) = cx.add_window_view(|_, cx| TestView {
        outside_focus: cx.focus_handle(),
        outside_clicks: outside_clicks.clone(),
    });
    cx.update(|window, cx| window.draw(cx).clear());

    let trigger = cx
        .debug_bounds("popover:conditional-restore:trigger")
        .expect("popover trigger should render");
    cx.simulate_click(trigger.center(), Default::default());
    cx.update(|window, cx| window.draw(cx).clear());
    assert!(
        cx.debug_bounds("popover:conditional-restore:content")
            .is_some()
    );
    assert!(cx.debug_selector_is_focused("popover:conditional-restore:trigger"));
    assert!(!cx.debug_selector_is_focused("popover:conditional-restore:content"));

    let outside = cx
        .debug_bounds("popover-test:outside")
        .expect("outside focus target should render");
    cx.simulate_click(outside.center(), Default::default());
    cx.run_until_parked();
    cx.update(|window, cx| window.draw(cx).clear());

    assert!(
        cx.debug_bounds("popover:conditional-restore:content")
            .is_none()
    );
    assert!(
        cx.debug_selector_is_focused("popover-test:outside"),
        "popover must not restore its trigger when focus never entered the surface"
    );
    assert_eq!(
        outside_clicks.get(),
        1,
        "pass-through dismissal must dispatch the original click exactly once"
    );
}

#[test]
fn alert_dialog_state_records_required_actions_and_destructive_intent() {
    let state = AlertDialog::new(
        "delete-project",
        "Delete project",
        "Delete this project?",
        "This action permanently removes project data.",
        "Delete",
    )
    .cancel_label("Keep project")
    .intent(AlertDialogIntent::Destructive)
    .open(true)
    .state();

    assert!(state.open());
    assert_eq!(state.open_mode(), AlertDialogOpenMode::Controlled);
    assert_eq!(state.title(), "Delete this project?");
    assert_eq!(
        state.description(),
        "This action permanently removes project data."
    );
    assert_eq!(state.content_role(), Role::AlertDialog);
    assert_eq!(state.intent(), AlertDialogIntent::Destructive);
    assert_eq!(state.cancel().kind(), AlertDialogActionKind::Cancel);
    assert_eq!(state.cancel().label(), "Keep project");
    assert_eq!(state.action().kind(), AlertDialogActionKind::Action);
    assert_eq!(state.action().label(), "Delete");
    assert_eq!(state.action().variant(), ButtonVariant::Destructive);
    assert_eq!(
        state.colors().action_background().token(),
        semantic::DESTRUCTIVE
    );
}

#[open_gpui::test]
fn modal_dialog_trigger_coordinate_is_outside_the_surface_and_cannot_bypass_the_barrier(
    cx: &mut open_gpui::TestAppContext,
) {
    struct TestView {
        open_events: Rc<RefCell<Vec<bool>>>,
    }

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let open_events = self.open_events.clone();
            div().size_full().child(
                Dialog::new(
                    "modal-trigger-barrier",
                    "Open dialog",
                    "Modal trigger barrier",
                    "Body",
                )
                .open(true)
                .on_open_change(move |intent, _, _| {
                    open_events.borrow_mut().push(intent.desired_open());
                }),
            )
        }
    }

    let open_events = Rc::new(RefCell::new(Vec::new()));
    let (_, cx) = cx.add_window_view(|_, _| TestView {
        open_events: open_events.clone(),
    });
    cx.update(|window, cx| window.draw(cx).clear());

    let trigger = cx
        .debug_bounds("dialog:modal-trigger-barrier:trigger")
        .expect("dialog trigger should remain measurable below the modal layer");
    let surface = cx
        .debug_bounds("dialog:modal-trigger-barrier:surface")
        .expect("controlled dialog surface should be mounted");
    cx.simulate_click(surface.center(), Default::default());
    assert!(cx.debug_selector_is_focused("dialog:modal-trigger-barrier:surface"));

    cx.simulate_click(trigger.center(), Default::default());
    cx.update(|window, cx| window.draw(cx).clear());
    let snapshot = cx.update(|window, cx| {
        WindowOverlayRuntime::for_window(window, cx)
            .snapshot(window, cx)
            .expect("dialog barrier snapshot should resolve")
    });
    let layer = snapshot
        .layers()
        .iter()
        .find(|layer| layer.id().as_str() == "dialog:modal-trigger-barrier")
        .expect("controlled dialog should remain registered after refusing close");
    assert_eq!(layer.phase(), OverlayLayerPhase::CloseRequested);
    assert_eq!(layer.pending_intent(), Some(DismissReason::OutsidePress));
    assert_eq!(open_events.borrow().as_slice(), &[false]);
    assert!(
        cx.debug_bounds("dialog:modal-trigger-barrier:surface")
            .is_some()
    );
    assert!(
        cx.debug_selector_is_focused("dialog:modal-trigger-barrier:surface"),
        "the covered trigger must not take focus through the modal barrier"
    );
}

#[test]
fn alert_dialog_state_blocks_underlay_and_restores_focus_to_trigger() {
    let state = AlertDialog::new(
        "confirm",
        "Open",
        "Archive item?",
        "It can be restored.",
        "Archive",
    )
    .default_open(true)
    .state();

    assert_eq!(state.open_mode(), AlertDialogOpenMode::Uncontrolled);
    assert!(state.default_open());
    assert!(state.open());
    assert_eq!(state.trigger_role(), Role::Button);
    assert!(state.trigger_selected());
    assert_eq!(state.overlay().policy().kind(), OverlayLayerKind::Modal);
    assert!(state.overlay().layer_state().blocks_underlay_input());
    assert_eq!(state.outside_press_policy(), OutsidePressPolicy::Consume);
    assert!(!state.outside_press_policy().resolve().dismisses());
    assert_eq!(state.escape_key_policy(), EscapeKeyPolicy::Ignore);
    assert_eq!(state.focus_restore_intent(), &FocusRestoreIntent::Trigger);
    assert_eq!(state.colors().barrier().token(), semantic::MODAL_OVERLAY);
}

#[test]
fn sheet_state_records_side_modal_mode_size_and_close_affordance() {
    let state = Sheet::new(
        "settings-sheet",
        "Open settings",
        "Settings",
        "Configure workspace",
    )
    .description("Workspace preferences")
    .default_open(true)
    .side(SheetSide::Left)
    .state();

    assert_eq!(state.open_mode(), SheetOpenMode::Uncontrolled);
    assert!(state.default_open());
    assert!(state.open());
    assert_eq!(state.side(), SheetSide::Left);
    assert!(state.side().is_horizontal());
    assert_eq!(state.modal_mode(), SheetModalMode::Modal);
    assert_eq!(state.close_affordance(), SheetCloseAffordance::Visible);
    assert!(state.close_affordance().visible());
    assert_eq!(state.title(), "Settings");
    assert_eq!(state.description(), Some("Workspace preferences"));
    assert_eq!(state.trigger_role(), Role::Button);
    assert_eq!(state.content_role(), Role::Dialog);
    assert!(state.trigger_selected());
    assert_eq!(state.overlay().policy().kind(), OverlayLayerKind::Modal);
    assert!(state.overlay().layer_state().blocks_underlay_input());
    assert_eq!(
        state.outside_press_policy(),
        OutsidePressPolicy::DismissAndConsume
    );
    assert!(state.outside_press_policy().resolve().dismisses());
    assert_eq!(state.colors().surface().token(), semantic::SURFACE);
    assert!(state.metrics().surface_size() > ui_px(0.0));
}

#[test]
fn sheet_state_models_non_modal_and_explicit_dismiss_policy() {
    let state = Sheet::new(
        "bottom-sheet",
        "Open details",
        "Details",
        "Non-modal information",
    )
    .open(true)
    .side(SheetSide::Bottom)
    .modal_mode(SheetModalMode::NonModal)
    .close_affordance(SheetCloseAffordance::Hidden)
    .outside_press_policy(OutsidePressPolicy::Ignore)
    .escape_key_policy(EscapeKeyPolicy::Ignore)
    .initial_focus_intent(InitialFocusIntent::None)
    .focus_restore_intent(FocusRestoreIntent::None)
    .small()
    .state();

    assert_eq!(state.open_mode(), SheetOpenMode::Controlled);
    assert!(state.open());
    assert_eq!(state.size(), Size::Small);
    assert_eq!(state.side(), SheetSide::Bottom);
    assert!(!state.side().is_horizontal());
    assert_eq!(state.modal_mode(), SheetModalMode::NonModal);
    assert_eq!(
        state.overlay().policy().kind(),
        OverlayLayerKind::NonModalDismissible
    );
    assert!(!state.overlay().layer_state().blocks_underlay_input());
    assert_eq!(state.close_affordance(), SheetCloseAffordance::Hidden);
    assert!(!state.close_affordance().visible());
    assert_eq!(state.outside_press_policy(), OutsidePressPolicy::Ignore);
    assert!(!state.overlay().wants_outside_press_handler());
    assert_eq!(state.escape_key_policy(), EscapeKeyPolicy::Ignore);
    assert_eq!(state.initial_focus_intent(), &InitialFocusIntent::None);
    assert_eq!(state.focus_restore_intent(), &FocusRestoreIntent::None);
}

fn settle_sheet_overlay(cx: &mut open_gpui::VisualTestContext) {
    cx.run_until_parked();
    cx.update(|window, cx| window.draw(cx).clear());
    cx.update(|window, cx| {
        window.drain_next_frame_callbacks_for_test(cx);
        window.draw(cx).clear();
    });
    cx.run_until_parked();
}

#[open_gpui::test]
fn modal_sheet_defaults_trap_focus_block_underlay_and_close_once(
    cx: &mut open_gpui::TestAppContext,
) {
    struct TestView {
        underlay_focus: FocusHandle,
        content_focus: FocusHandle,
        underlay_clicks: Rc<Cell<usize>>,
        open_intents: Rc<RefCell<Vec<(bool, DismissReason)>>>,
        close_intents: Rc<Cell<usize>>,
    }

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let underlay_clicks = self.underlay_clicks.clone();
            let open_intents = self.open_intents.clone();
            let close_intents = self.close_intents.clone();

            div()
                .relative()
                .size_full()
                .child(
                    div()
                        .id("default-sheet-underlay")
                        .debug_selector(|| "sheet-test:default-underlay".to_owned())
                        .absolute()
                        .left(px(4.0))
                        .top(px(220.0))
                        .w(px(112.0))
                        .h(px(36.0))
                        .focusable()
                        .track_focus(&self.underlay_focus)
                        .tab_index(0)
                        .on_click(move |_, _, _| {
                            underlay_clicks.set(underlay_clicks.get() + 1);
                        })
                        .child("Underlay"),
                )
                .child(
                    Sheet::element(
                        "default-modal-sheet",
                        "Open sheet",
                        "Default modal sheet",
                        div()
                            .id("default-sheet-content")
                            .debug_selector(|| "sheet-test:default-content".to_owned())
                            .w(px(160.0))
                            .h(px(36.0))
                            .focusable()
                            .track_focus(&self.content_focus)
                            .tab_index(0)
                            .child("Focusable content"),
                    )
                    .on_open_change(move |intent, _, _| {
                        if !intent.desired_open() {
                            close_intents.set(close_intents.get() + 1);
                        }
                        open_intents
                            .borrow_mut()
                            .push((intent.desired_open(), intent.reason()));
                    }),
                )
        }
    }

    let underlay_clicks = Rc::new(Cell::new(0));
    let open_intents = Rc::new(RefCell::new(Vec::new()));
    let close_intents = Rc::new(Cell::new(0));
    let (_, cx) = cx.add_window_view(|_, cx| TestView {
        underlay_focus: cx.focus_handle(),
        content_focus: cx.focus_handle(),
        underlay_clicks: underlay_clicks.clone(),
        open_intents: open_intents.clone(),
        close_intents: close_intents.clone(),
    });
    cx.update(|window, cx| window.draw(cx).clear());

    let trigger = cx
        .debug_bounds("sheet:default-modal-sheet:trigger")
        .expect("default Sheet trigger should render");
    cx.simulate_click(trigger.center(), Default::default());
    settle_sheet_overlay(cx);

    assert!(
        cx.debug_bounds("sheet:default-modal-sheet:surface")
            .is_some()
    );
    assert!(
        cx.debug_selector_is_focused("sheet:default-modal-sheet:close"),
        "the default visible close affordance should receive initial focus"
    );

    cx.simulate_keystrokes("tab");
    assert!(cx.debug_selector_is_focused("sheet-test:default-content"));
    cx.simulate_keystrokes("tab");
    assert!(cx.debug_selector_is_focused("sheet:default-modal-sheet:close"));
    cx.simulate_keystrokes("shift-tab");
    assert!(cx.debug_selector_is_focused("sheet-test:default-content"));
    cx.simulate_keystrokes("shift-tab");
    assert!(cx.debug_selector_is_focused("sheet:default-modal-sheet:close"));

    let underlay = cx
        .debug_bounds("sheet-test:default-underlay")
        .expect("default Sheet underlay probe should render");
    cx.simulate_click(underlay.center(), Default::default());
    settle_sheet_overlay(cx);

    assert_eq!(
        underlay_clicks.get(),
        0,
        "the default modal outside press must be consumed before the underlay"
    );
    assert_eq!(close_intents.get(), 1);
    assert!(
        cx.debug_bounds("sheet:default-modal-sheet:surface")
            .is_none()
    );
    assert!(cx.debug_selector_is_focused("sheet:default-modal-sheet:trigger"));

    let trigger = cx
        .debug_bounds("sheet:default-modal-sheet:trigger")
        .expect("default Sheet trigger should remain mounted after outside dismissal");
    cx.simulate_click(trigger.center(), Default::default());
    settle_sheet_overlay(cx);
    assert!(cx.debug_selector_is_focused("sheet:default-modal-sheet:close"));

    cx.simulate_keystrokes("escape");
    settle_sheet_overlay(cx);

    assert_eq!(close_intents.get(), 2);
    assert!(
        cx.debug_bounds("sheet:default-modal-sheet:surface")
            .is_none()
    );
    assert!(cx.debug_selector_is_focused("sheet:default-modal-sheet:trigger"));
    assert_eq!(
        open_intents.borrow().as_slice(),
        &[
            (true, DismissReason::Trigger),
            (false, DismissReason::OutsidePress),
            (true, DismissReason::Trigger),
            (false, DismissReason::EscapeKey),
        ]
    );
}

#[open_gpui::test]
fn controlled_modal_sheet_refusal_retains_authority_and_retry_revision(
    cx: &mut open_gpui::TestAppContext,
) {
    struct TestView {
        open: bool,
        underlay_focus: FocusHandle,
        content_focus: FocusHandle,
        underlay_clicks: Rc<Cell<usize>>,
        open_intents: Rc<RefCell<Vec<OverlayOpenIntent>>>,
        close_intents: Rc<Cell<usize>>,
    }

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let underlay_clicks = self.underlay_clicks.clone();
            let open_intents = self.open_intents.clone();
            let close_intents = self.close_intents.clone();

            div()
                .relative()
                .size_full()
                .child(
                    div()
                        .id("controlled-sheet-underlay")
                        .debug_selector(|| "sheet-test:controlled-underlay".to_owned())
                        .absolute()
                        .left(px(4.0))
                        .top(px(220.0))
                        .w(px(112.0))
                        .h(px(36.0))
                        .focusable()
                        .track_focus(&self.underlay_focus)
                        .tab_index(0)
                        .on_click(move |_, _, _| {
                            underlay_clicks.set(underlay_clicks.get() + 1);
                        })
                        .child("Underlay"),
                )
                .child(
                    Sheet::element(
                        "controlled-modal-sheet",
                        "Open controlled sheet",
                        "Controlled modal sheet",
                        div()
                            .id("controlled-sheet-content")
                            .debug_selector(|| "sheet-test:controlled-content".to_owned())
                            .w(px(160.0))
                            .h(px(36.0))
                            .focusable()
                            .track_focus(&self.content_focus)
                            .tab_index(0)
                            .child("Focusable content"),
                    )
                    .open(self.open)
                    .on_open_change(move |intent, _, _| {
                        if !intent.desired_open() {
                            close_intents.set(close_intents.get() + 1);
                        }
                        open_intents.borrow_mut().push(intent);
                    }),
                )
        }
    }

    let underlay_clicks = Rc::new(Cell::new(0));
    let open_intents = Rc::new(RefCell::new(Vec::new()));
    let close_intents = Rc::new(Cell::new(0));
    let (view, cx) = cx.add_window_view(|_, cx| TestView {
        open: true,
        underlay_focus: cx.focus_handle(),
        content_focus: cx.focus_handle(),
        underlay_clicks: underlay_clicks.clone(),
        open_intents: open_intents.clone(),
        close_intents: close_intents.clone(),
    });
    settle_sheet_overlay(cx);

    assert!(
        cx.debug_bounds("sheet:controlled-modal-sheet:surface")
            .is_some()
    );
    assert!(cx.debug_selector_is_focused("sheet:controlled-modal-sheet:close"));

    cx.simulate_keystrokes("escape");
    settle_sheet_overlay(cx);

    assert_eq!(open_intents.borrow().len(), 1);
    assert_eq!(close_intents.get(), 1);
    let first_intent = open_intents
        .borrow()
        .first()
        .cloned()
        .expect("controlled Sheet should emit a close intent");
    assert!(!first_intent.desired_open());
    assert_eq!(first_intent.reason(), DismissReason::EscapeKey);
    let first_revision = first_intent
        .revision()
        .expect("controlled Sheet close should carry a revision");
    let requested = cx.update(|window, cx| {
        WindowOverlayRuntime::for_window(window, cx)
            .snapshot(window, cx)
            .expect("controlled Sheet snapshot should resolve")
    });
    assert_eq!(
        requested
            .layers()
            .iter()
            .find(|layer| layer.id().as_str() == "sheet:controlled-modal-sheet")
            .expect("controlled Sheet should remain registered")
            .phase(),
        OverlayLayerPhase::CloseRequested
    );

    cx.simulate_keystrokes("escape");
    assert_eq!(
        open_intents.borrow().len(),
        1,
        "a pending controlled close must suppress duplicate intent"
    );
    assert_eq!(close_intents.get(), 1);

    cx.simulate_keystrokes("tab");
    assert!(cx.debug_selector_is_focused("sheet-test:controlled-content"));
    cx.simulate_keystrokes("tab");
    assert!(cx.debug_selector_is_focused("sheet:controlled-modal-sheet:close"));
    cx.simulate_keystrokes("shift-tab");
    assert!(cx.debug_selector_is_focused("sheet-test:controlled-content"));
    cx.simulate_keystrokes("shift-tab");
    assert!(cx.debug_selector_is_focused("sheet:controlled-modal-sheet:close"));

    let underlay = cx
        .debug_bounds("sheet-test:controlled-underlay")
        .expect("controlled Sheet underlay probe should render");
    cx.simulate_click(underlay.center(), Default::default());
    assert_eq!(underlay_clicks.get(), 0);
    assert_eq!(open_intents.borrow().len(), 1);
    assert_eq!(close_intents.get(), 1);
    assert!(cx.debug_selector_is_focused("sheet:controlled-modal-sheet:close"));

    cx.update(|window, cx| {
        first_intent
            .reject(window, cx)
            .expect("the owner should reject the exact pending Sheet intent");
    });
    settle_sheet_overlay(cx);
    let reopened = cx.update(|window, cx| {
        WindowOverlayRuntime::for_window(window, cx)
            .snapshot(window, cx)
            .expect("rejected controlled Sheet snapshot should resolve")
    });
    assert_eq!(
        reopened
            .layers()
            .iter()
            .find(|layer| layer.id().as_str() == "sheet:controlled-modal-sheet")
            .expect("rejected controlled Sheet should remain registered")
            .phase(),
        OverlayLayerPhase::Open
    );
    assert!(
        cx.debug_bounds("sheet:controlled-modal-sheet:surface")
            .is_some()
    );
    assert!(cx.debug_selector_is_focused("sheet:controlled-modal-sheet:close"));

    cx.simulate_keystrokes("escape");
    settle_sheet_overlay(cx);

    assert_eq!(open_intents.borrow().len(), 2);
    assert_eq!(close_intents.get(), 2);
    let second_revision = open_intents.borrow()[1]
        .revision()
        .expect("retried controlled Sheet close should carry a revision");
    assert_eq!(open_intents.borrow()[1].reason(), DismissReason::EscapeKey);
    assert_ne!(first_revision, second_revision);

    cx.update_window_entity(&view, |view, _, cx| {
        view.open = false;
        cx.notify();
    });
    settle_sheet_overlay(cx);

    assert!(
        cx.debug_bounds("sheet:controlled-modal-sheet:surface")
            .is_none()
    );
    assert!(cx.debug_selector_is_focused("sheet:controlled-modal-sheet:trigger"));
    assert_eq!(underlay_clicks.get(), 0);
    assert_eq!(close_intents.get(), 2);
}

#[test]
fn menu_state_records_items_roving_focus_and_overlay_policy() {
    let state = Menu::new("file-menu", "File")
        .open(true)
        .default_focused_value("save")
        .item(MenuItem::action("new", "New"))
        .item(MenuItem::action("save", "Save"))
        .item(MenuItem::separator("separator"))
        .item(MenuItem::action("delete", "Delete").disabled(true))
        .state();

    assert!(state.open());
    assert_eq!(state.open_mode(), MenuOpenMode::Controlled);
    assert_eq!(state.trigger_role(), Role::Button);
    assert_eq!(state.content_role(), Role::Menu);
    assert!(state.trigger_selected());
    assert_eq!(state.overlay().policy().kind(), OverlayLayerKind::Menu);
    assert!(state.overlay().wants_outside_press_handler());
    assert!(state.overlay().layer_state().hit_testable());
    assert_eq!(
        state.outside_press_policy(),
        OutsidePressPolicy::DismissAndConsume
    );
    assert_eq!(state.escape_key_policy(), EscapeKeyPolicy::Dismiss);
    assert_eq!(state.focused_value(), Some("save"));
    assert_eq!(state.items().len(), 4);
    assert_eq!(state.items()[0].role(), Some(Role::MenuItem));
    assert_eq!(state.items()[2].kind(), MenuItemKind::Separator);
    assert!(!state.items()[2].focusable());
    assert!(state.items()[3].disabled());
    assert!(!state.items()[3].activation_enabled());
    assert_eq!(state.colors().surface().token(), semantic::SURFACE);
    assert_eq!(
        state.colors().trigger_background().state(),
        ColorState::Selected
    );
}

#[open_gpui::test]
fn menu_final_tree_projects_structural_roles_disabled_state_and_exact_actions(
    cx: &mut open_gpui::TestAppContext,
) {
    struct MenuAccessibilityProbe;

    impl Render for MenuAccessibilityProbe {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            Menu::new("semantic-menu", "Open semantic menu")
                .default_open(true)
                .item(MenuItem::header("file-heading", "File actions"))
                .item(MenuItem::action("open", "Open file"))
                .item(MenuItem::separator("file-separator"))
                .item(MenuItem::action("delete", "Delete file").disabled(true))
        }
    }

    let (_, cx) = cx.add_window_view(|_, _| MenuAccessibilityProbe);
    assert!(cx.activate_accessibility());
    cx.run_until_parked();
    cx.update(|window, cx| window.draw(cx).clear());

    let update = cx
        .latest_accessibility_tree_update()
        .expect("menu accessibility tree should publish");
    let (_, trigger) = node_with_label(&update, "Open semantic menu");
    assert_eq!(trigger.role(), accesskit::Role::Button);
    assert_eq!(trigger.is_selected(), Some(true));
    assert_eq!(trigger.is_expanded(), Some(true));
    assert_exact_actions(
        trigger,
        &[accesskit::Action::Click, accesskit::Action::Focus],
    );

    let (_, header) = node_with_label(&update, "File actions");
    assert_eq!(header.role(), accesskit::Role::Label);
    assert_exact_actions(header, &[]);

    let (_, enabled_item) = node_with_label(&update, "Open file");
    assert_eq!(enabled_item.role(), accesskit::Role::MenuItem);
    assert!(!enabled_item.is_disabled());
    assert_exact_actions(enabled_item, &[accesskit::Action::Click]);

    let (_, disabled_item) = node_with_label(&update, "Delete file");
    assert_eq!(disabled_item.role(), accesskit::Role::MenuItem);
    assert!(disabled_item.is_disabled());
    assert_exact_actions(disabled_item, &[]);

    let menu = update
        .nodes
        .iter()
        .find_map(|(_, node)| (node.role() == accesskit::Role::Menu).then_some(node))
        .expect("menu surface should publish a Menu node");
    assert_exact_actions(menu, &[accesskit::Action::Focus]);
    let separator = update
        .nodes
        .iter()
        .find_map(|(_, node)| (node.role() == accesskit::Role::Group).then_some(node))
        .expect("menu separator should publish the structural Group role");
    assert_exact_actions(separator, &[]);
}

#[test]
fn menu_state_skips_header_items_for_focus_and_activation() {
    let state = Menu::new("selection-menu", "Selection")
        .open(true)
        .default_focused_value("organize-heading")
        .items([
            MenuItem::header("organize-heading", "Organize"),
            MenuItem::separator("separator"),
            MenuItem::action("create-card", "Create Card"),
        ])
        .state();

    assert!(state.open());
    assert_eq!(state.items().len(), 3);
    assert_eq!(state.items()[0].kind(), MenuItemKind::Header);
    assert_eq!(state.items()[0].role(), None);
    assert!(!state.items()[0].disabled());
    assert!(!state.items()[0].focusable());
    assert!(!state.items()[0].activation_enabled());
    assert_eq!(state.focused_index(), Some(2));
    assert_eq!(state.focused_value(), Some("create-card"));
}

#[open_gpui::test]
fn menu_icon_trigger_keeps_accessible_menu_behavior_and_compacts_trigger(
    cx: &mut open_gpui::TestAppContext,
) {
    struct TestView;

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            div()
                .size_full()
                .flex()
                .gap_2()
                .child(
                    Menu::new("text-menu", "More")
                        .small()
                        .item(MenuItem::action("copy", "Copy")),
                )
                .child(
                    Menu::new("icon-menu", "More")
                        .trigger_icon("...")
                        .trigger_tooltip(Tooltip::text("More"))
                        .small()
                        .item(MenuItem::action("copy", "Copy")),
                )
        }
    }

    let (_, cx) = cx.add_window_view(|_, _| TestView);
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    let text_trigger = cx
        .debug_bounds("menu:text-menu:trigger")
        .expect("text menu trigger should render");
    let icon_trigger = cx
        .debug_bounds("menu:icon-menu:trigger")
        .expect("icon menu trigger should render");

    assert!(
        icon_trigger.size.width < text_trigger.size.width,
        "icon trigger should be narrower than text trigger"
    );

    let state = Menu::new("state-icon-menu", "More")
        .trigger_icon("...")
        .open(true)
        .item(MenuItem::action("copy", "Copy"))
        .state();

    assert_eq!(state.trigger_role(), Role::Button);
    assert_eq!(state.content_role(), Role::Menu);
    assert!(state.trigger_selected());
    assert_eq!(state.focused_value(), Some("copy"));
}

#[test]
fn menu_items_project_core_command_descriptors() {
    let descriptor = open_gpui_command::CommandDescriptor::new("workspace.open", "Open Workspace")
        .shortcut("Ctrl+Shift+O")
        .when("workspace")
        .menu_path(["File", "Open"]);
    let state = Menu::new("file-menu", "File")
        .open(true)
        .item(MenuItem::from_command_descriptor(&descriptor))
        .state();

    assert_eq!(descriptor.menu_path_ref(), ["File", "Open"]);
    assert_eq!(state.items()[0].value(), "workspace.open");
    assert_eq!(state.items()[0].label(), "Open Workspace");
    assert_eq!(state.items()[0].shortcut(), Some("Ctrl+Shift+O"));
    assert_eq!(state.items()[0].when_ref(), Some("workspace"));
    assert!(state.items()[0].activation_enabled());

    let context_state = ContextMenu::new("context-menu", "Context menu")
        .open(true)
        .item(MenuItem::from_command_descriptor(&descriptor))
        .state();
    assert_eq!(
        context_state.menu().items()[0].shortcut(),
        Some("Ctrl+Shift+O")
    );
    assert_eq!(
        context_state.menu().items()[0].when_ref(),
        Some("workspace")
    );

    let menu_descriptor = MenuItemDescriptor::from_command_descriptor(&descriptor);
    assert_eq!(menu_descriptor.shortcut_ref(), Some("Ctrl+Shift+O"));
    assert_eq!(menu_descriptor.when_ref(), Some("workspace"));
}

#[test]
fn menu_state_defaults_focus_to_first_focusable_item_when_open() {
    let state = Menu::new("file-menu", "File")
        .open(true)
        .item(MenuItem::separator("separator"))
        .item(MenuItem::action("save", "Save"))
        .item(MenuItem::action("delete", "Delete").disabled(true))
        .state();

    assert!(state.open());
    assert_eq!(state.focused_value(), Some("save"));
    assert_eq!(state.items()[0].kind(), MenuItemKind::Separator);
    assert!(state.items()[2].disabled());
}

#[test]
fn menu_navigation_and_activation_skip_disabled_and_separator_items() {
    let state = Menu::new("edit-menu", "Edit")
        .open(true)
        .default_focused_value("copy")
        .items([
            MenuItem::action("cut", "Cut"),
            MenuItem::action("copy", "Copy"),
            MenuItem::separator("separator"),
            MenuItem::action("paste", "Paste").disabled(true),
            MenuItem::action("select-all", "Select all"),
        ])
        .state();
    let disabled = [false, false, true, true, false];

    assert_eq!(menu_navigation_target("down", 1, &disabled), Some(4));
    assert_eq!(menu_navigation_target("up", 1, &disabled), Some(0));
    assert_eq!(menu_navigation_target("home", 4, &disabled), Some(0));
    assert_eq!(menu_navigation_target("end", 0, &disabled), Some(4));
    assert_eq!(
        state.navigation_target("down").map(|item| item.value()),
        Some("select-all")
    );
    assert_eq!(
        state.activation_for_key("enter").map(|selection| {
            (
                selection.index(),
                selection.value().to_owned(),
                selection.label().to_owned(),
            )
        }),
        Some((1, "copy".to_owned(), "Copy".to_owned()))
    );
    assert!(state.activation_for_key("space").is_some());
    assert!(state.activation_for_key("escape").is_none());
}

#[test]
fn menu_state_resolves_checked_radio_and_submenu_item_contracts() {
    let state = Menu::new("view-menu", "View")
        .open(true)
        .default_focused_value("show-hidden")
        .items([
            MenuItem::checkbox("show-hidden", "Show hidden files", true),
            MenuItem::radio("density-compact", "Compact", false),
            MenuItem::radio("density-comfortable", "Comfortable", true),
            MenuItem::submenu(
                "sort",
                "Sort by",
                [
                    MenuItem::action("name", "Name"),
                    MenuItem::action("name", "Nested duplicate name"),
                ],
            ),
            MenuItem::submenu("empty", "Empty submenu", []),
            MenuItem::separator("separator"),
        ])
        .state();

    assert_eq!(state.items()[0].kind(), MenuItemKind::Checkbox);
    assert!(state.items()[0].checked());
    assert_eq!(state.items()[0].toggled(), Some(Toggled::True));
    assert!(state.items()[0].activation_enabled());
    let checkbox_selection = state
        .activation_for_key("enter")
        .expect("focused checkbox should activate");
    assert_eq!(checkbox_selection.kind(), MenuItemKind::Checkbox);
    assert!(checkbox_selection.checked());
    assert_eq!(checkbox_selection.path_key(), "0:show-hidden");

    assert_eq!(state.items()[1].kind(), MenuItemKind::Radio);
    assert_eq!(state.items()[1].toggled(), Some(Toggled::False));
    assert_eq!(state.items()[2].toggled(), Some(Toggled::True));

    let submenu = &state.items()[3];
    assert_eq!(submenu.kind(), MenuItemKind::Submenu);
    assert!(submenu.focusable());
    assert!(!submenu.activation_enabled());
    assert_eq!(submenu.child_count(), 2);
    assert_eq!(submenu.children()[0].parent_value(), Some("sort"));
    assert_eq!(submenu.path_key(), "3:sort");
    assert_eq!(submenu.children()[0].path_key(), "3:sort/0:name");
    assert_eq!(submenu.children()[1].path_key(), "3:sort/1:name");

    let empty_submenu = &state.items()[4];
    assert_eq!(empty_submenu.kind(), MenuItemKind::Submenu);
    assert!(!empty_submenu.focusable());
    assert!(!empty_submenu.activation_enabled());
    assert!(!state.items()[5].focusable());
}

#[test]
fn menu_public_path_keys_escape_percent_and_segment_delimiters() {
    let state = Menu::new("escaped-path-menu", "Escaped paths")
        .open(true)
        .default_focused_value("parent/%")
        .item(MenuItem::submenu(
            "parent/%",
            "Parent",
            [MenuItem::action("child/%", "Child")],
        ))
        .state();

    let parent = &state.items()[0];
    let child = &parent.children()[0];
    assert_eq!(parent.path_key(), "0:parent%2F%25");
    assert_eq!(child.path_key(), "0:parent%2F%25/0:child%2F%25");
    assert_eq!(
        state.focused_path_key().as_deref(),
        Some(parent.path_key().as_str())
    );

    let opened = state
        .submenu_navigation_target("right")
        .expect("Right should open the escaped submenu path");
    assert_eq!(opened.open_path_key().as_deref(), Some("0:parent%2F%25"));
    assert_eq!(opened.focused_path_key(), "0:parent%2F%25/0:child%2F%25");
}

#[test]
fn menu_state_resolves_typeahead_without_runtime_timer_state() {
    let state = Menu::new("search-menu", "Search")
        .open(true)
        .default_focused_value("beta")
        .items([
            MenuItem::action("alpha", "Alpha"),
            MenuItem::action("beta", "Beta"),
            MenuItem::separator("separator"),
            MenuItem::action("blocked", "Bravo blocked").disabled(true),
            MenuItem::checkbox("bravo", "Bravo visible", false),
            MenuItem::submenu("empty", "Bravo empty submenu", []),
        ])
        .state();

    assert_eq!(
        state.typeahead_target(" br").map(|item| item.value()),
        Some("bravo")
    );
    assert_eq!(
        state.typeahead_target("AL").map(|item| item.value()),
        Some("alpha")
    );
    assert!(state.typeahead_target("").is_none());
    assert!(state.typeahead_target("missing").is_none());
}

#[test]
fn menu_state_resolves_visible_submenu_navigation_and_local_scroll_contract() {
    let state = Menu::new("nested-menu", "Nested")
        .open(true)
        .default_focused_value("sort")
        .items([
            MenuItem::action("open", "Open"),
            MenuItem::submenu(
                "sort",
                "Sort by",
                [
                    MenuItem::action("name", "Name"),
                    MenuItem::action("modified", "Modified"),
                ],
            ),
            MenuItem::action("close", "Close"),
        ])
        .state();

    assert_eq!(state.focused_value(), Some("sort"));
    assert_eq!(state.focused_path_key().as_deref(), Some("1:sort"));
    assert_eq!(state.visible_items().len(), 3);
    let opened = state
        .submenu_navigation_target("right")
        .expect("Right should open a focused submenu");
    let _: open_gpui_ui_components::MenuSubmenuNavigation = opened.clone();
    assert_eq!(opened.open_path_key().as_deref(), Some("1:sort"));
    assert_eq!(opened.focused_path_key(), "1:sort/0:name");
    assert_eq!(opened.focused_value(), "name");

    let long_state = Menu::new("long-menu", "Long")
        .open(true)
        .items(
            (0..10).map(|index| MenuItem::action(format!("item-{index}"), format!("Item {index}"))),
        )
        .state();
    assert!(long_state.scrollable_content());
    assert_eq!(long_state.visible_items().len(), 10);
}

#[test]
fn menu_state_resolves_submenu_surface_and_safe_hover_contract() {
    let state = Menu::new("nested-menu", "Nested")
        .open(true)
        .default_focused_value("sort")
        .items([
            MenuItem::action("open", "Open"),
            MenuItem::submenu(
                "sort",
                "Sort by",
                [
                    MenuItem::action("name", "Name"),
                    MenuItem::action("modified", "Modified"),
                ],
            ),
            MenuItem::action("close", "Close"),
        ])
        .state();
    let trigger_path = state
        .submenu_navigation_target("right")
        .expect("submenu navigation should resolve")
        .open_path()
        .to_vec();
    let trigger_bounds = rect(
        ui_point(ui_px(40.0), ui_px(48.0)),
        ui_size(ui_px(160.0), ui_px(32.0)),
    );
    let content_size = ui_size(ui_px(200.0), ui_px(96.0));
    let safe_bounds = rect(
        ui_point(ui_px(0.0), ui_px(0.0)),
        ui_size(ui_px(640.0), ui_px(360.0)),
    );

    let surface = state
        .submenu_surface_for_trigger(
            &trigger_path,
            trigger_bounds,
            content_size,
            Some(safe_bounds),
        )
        .expect("open menu submenu trigger should resolve a floating surface plan");
    let _: open_gpui_ui_components::MenuSubmenuSurface = surface;
    let _: open_gpui_ui_components::MenuSafeHoverCorridor = surface.hover_corridor();
    assert_eq!(surface.trigger_bounds(), trigger_bounds);
    assert_eq!(
        surface.placement_input().preferred_anchor_bounds(),
        Some(trigger_bounds)
    );
    assert_eq!(
        surface.placement_input().side(),
        OverlayPlacementSide::Right
    );
    assert_eq!(
        surface.placement_input().alignment(),
        OverlayPlacementAlignment::Start
    );
    assert_eq!(surface.placement_input().safe_bounds(), Some(safe_bounds));
    assert_eq!(
        surface.content_bounds(),
        rect(
            ui_point(ui_px(200.0), ui_px(48.0)),
            ui_size(ui_px(200.0), ui_px(96.0)),
        )
    );
    assert!(
        surface
            .hover_corridor()
            .contains_point(ui_point(ui_px(210.0), ui_px(60.0)))
    );
    assert!(
        !surface
            .hover_corridor()
            .contains_point(ui_point(ui_px(20.0), ui_px(20.0)))
    );
    assert!(
        state
            .submenu_surface_for_trigger(
                &[String::from("2:close")],
                trigger_bounds,
                content_size,
                None
            )
            .is_none()
    );

    let closed_nested_submenu = Menu::new("closed-nested-menu", "Closed nested")
        .open(true)
        .default_focused_value("sort")
        .items([MenuItem::submenu(
            "sort",
            "Sort by",
            [MenuItem::submenu(
                "then",
                "Then by",
                [MenuItem::action("owner", "Owner")],
            )],
        )])
        .state();
    assert!(
        closed_nested_submenu
            .submenu_surface_for_trigger(
                &[String::from("0:sort"), String::from("0:then")],
                trigger_bounds,
                content_size,
                None
            )
            .is_none(),
        "hidden nested submenu triggers should not resolve floating surfaces"
    );
}

#[test]
fn menu_submenu_surface_resolves_left_placement_without_renderer_state() {
    let trigger_bounds = rect(
        ui_point(ui_px(240.0), ui_px(80.0)),
        ui_size(ui_px(120.0), ui_px(32.0)),
    );
    let content_size = ui_size(ui_px(180.0), ui_px(120.0));

    let surface = MenuSubmenuSurface::resolve(
        trigger_bounds,
        content_size,
        OverlayPlacementSide::Left,
        OverlayPlacementAlignment::End,
        ui_px(4.0),
        None,
    );

    assert_eq!(surface.placement_input().side(), OverlayPlacementSide::Left);
    assert_eq!(
        surface.placement_input().alignment(),
        OverlayPlacementAlignment::End
    );
    assert_eq!(surface.placement_input().offset(), ui_px(4.0));
    assert_eq!(
        surface.content_bounds(),
        rect(
            ui_point(ui_px(56.0), ui_px(-8.0)),
            ui_size(ui_px(180.0), ui_px(120.0)),
        )
    );
    assert!(
        surface
            .hover_corridor()
            .contains_point(ui_point(ui_px(238.0), ui_px(84.0))),
        "corridor should include the horizontal gap between trigger and left submenu"
    );
}

#[test]
fn menu_state_discards_invalid_runtime_submenu_paths_after_items_change() {
    let state = Menu::new("changed-menu", "Changed")
        .open(true)
        .default_focused_value("sort")
        .item(MenuItem::submenu("sort", "Sort by", []))
        .state();

    assert_eq!(state.visible_items().len(), 1);
    assert_eq!(state.focused_value(), None);
    assert_eq!(state.open_path_key(), None);
    assert!(!state.items()[0].focusable());
    assert!(state.submenu_navigation_target("right").is_none());
}

#[test]
fn menu_runtime_keyboard_navigation_keeps_runtime_focused_value_after_rerender() {
    let state = Menu::new("runtime-menu", "Runtime menu")
        .open(true)
        .default_focused_value("copy")
        .items([
            MenuItem::action("cut", "Cut"),
            MenuItem::action("copy", "Copy"),
            MenuItem::action("select-all", "Select all"),
        ])
        .state();

    assert_eq!(state.focused_value(), Some("copy"));
    assert_eq!(
        state.navigation_target("down").map(|item| item.value()),
        Some("select-all")
    );
}

#[open_gpui::test]
fn menu_runtime_keyboard_navigation_preserves_focused_value_after_rerender(
    cx: &mut open_gpui::TestAppContext,
) {
    struct TestView {
        selections: Rc<RefCell<Vec<MenuSelection>>>,
        item_selections: Rc<RefCell<Vec<MenuSelection>>>,
    }

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let selections = self.selections.clone();
            let item_selections = self.item_selections.clone();

            div().size_full().child(
                Menu::new("runtime-menu", "Runtime menu")
                    .default_focused_value("copy")
                    .item(MenuItem::action("cut", "Cut"))
                    .item(MenuItem::action("copy", "Copy"))
                    .item(MenuItem::action("select-all", "Select all").on_select(
                        move |selection, _, _| {
                            item_selections.borrow_mut().push(selection);
                        },
                    ))
                    .on_select(move |selection, _, _| {
                        selections.borrow_mut().push(selection);
                    }),
            )
        }
    }

    let selections = Rc::new(RefCell::new(Vec::new()));
    let item_selections = Rc::new(RefCell::new(Vec::new()));
    let (_, cx) = cx.add_window_view(|_, _| TestView {
        selections: selections.clone(),
        item_selections: item_selections.clone(),
    });
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    let trigger = cx
        .debug_bounds("menu:runtime-menu:trigger")
        .expect("runtime menu trigger should render");
    cx.simulate_click(trigger.center(), Default::default());
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    assert!(
        cx.debug_bounds("menu:runtime-menu:content").is_some(),
        "runtime menu content should render when opened"
    );

    cx.simulate_keystrokes("down");
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });
    assert!(
        selections.borrow().is_empty(),
        "arrow navigation should move the runtime focus without selecting"
    );

    cx.simulate_keystrokes("enter");
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    let after_enter = selections.borrow().clone();
    let after_item_enter = item_selections.borrow().clone();
    assert_eq!(after_enter.len(), 1);
    assert_eq!(after_enter[0].index(), 2);
    assert_eq!(after_enter[0].value(), "select-all");
    assert_eq!(after_enter[0].label(), "Select all");
    assert_eq!(after_item_enter.len(), 1);
    assert_eq!(after_item_enter[0].path_key(), "2:select-all");
}

#[open_gpui::test]
fn menu_selection_callback_focus_wins_over_the_older_close_restore(
    cx: &mut open_gpui::TestAppContext,
) {
    struct TestView {
        destination_focus: FocusHandle,
        selections: Rc<Cell<usize>>,
    }

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let destination_focus = self.destination_focus.clone();
            let selections = self.selections.clone();
            div()
                .relative()
                .size_full()
                .child(
                    div()
                        .id("menu-selection-destination")
                        .debug_selector(|| "menu-test:selection-destination".to_owned())
                        .absolute()
                        .left(px(420.0))
                        .top(px(280.0))
                        .w(px(120.0))
                        .h(px(32.0))
                        .focusable()
                        .track_focus(&self.destination_focus)
                        .tab_index(0)
                        .child("Destination"),
                )
                .child(
                    Menu::new("selection-reentrant-menu", "Selection reentrant")
                        .default_focused_value("select")
                        .item(MenuItem::action("select", "Select").on_select(
                            move |_, window, cx| {
                                selections.set(selections.get() + 1);
                                destination_focus.focus(window, cx);
                            },
                        )),
                )
        }
    }

    let selections = Rc::new(Cell::new(0));
    let (_, cx) = cx.add_window_view(|_, cx| TestView {
        destination_focus: cx.focus_handle(),
        selections: selections.clone(),
    });
    cx.update(|window, cx| window.draw(cx).clear());
    let trigger = cx
        .debug_bounds("menu:selection-reentrant-menu:trigger")
        .expect("menu trigger should render");
    cx.simulate_click(trigger.center(), Default::default());
    cx.update(|window, cx| window.draw(cx).clear());

    cx.simulate_keystrokes("enter");
    cx.run_until_parked();
    cx.update(|window, cx| window.draw(cx).clear());
    assert_eq!(selections.get(), 1);
    assert!(
        cx.debug_bounds("menu:selection-reentrant-menu:content")
            .is_none()
    );
    assert!(
        cx.debug_selector_is_focused("menu-test:selection-destination"),
        "a newer focus claim from the selection callback must beat menu restoration"
    );
}

#[open_gpui::test]
fn menu_tab_closes_the_active_root_instead_of_leaving_it_mounted(
    cx: &mut open_gpui::TestAppContext,
) {
    struct TestView {
        open_events: Rc<RefCell<Vec<bool>>>,
    }

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let open_events = self.open_events.clone();
            div().size_full().child(
                Menu::new("tab-menu", "Tab menu")
                    .item(MenuItem::action("first", "First"))
                    .item(MenuItem::action("second", "Second"))
                    .on_open_change(move |intent, _, _| {
                        open_events.borrow_mut().push(intent.desired_open());
                    }),
            )
        }
    }

    let open_events = Rc::new(RefCell::new(Vec::new()));
    let (_, cx) = cx.add_window_view(|_, _| TestView {
        open_events: open_events.clone(),
    });
    cx.update(|window, cx| window.draw(cx).clear());
    let trigger = cx
        .debug_bounds("menu:tab-menu:trigger")
        .expect("menu trigger should render");
    cx.simulate_click(trigger.center(), Default::default());
    cx.update(|window, cx| window.draw(cx).clear());
    assert!(cx.debug_bounds("menu:tab-menu:content").is_some());
    assert!(
        cx.debug_selector_is_focused("menu:tab-menu:content"),
        "the active menu surface should own focus before Tab is dispatched"
    );

    cx.simulate_keystrokes("tab");
    cx.run_until_parked();
    cx.update(|window, cx| window.draw(cx).clear());

    assert_eq!(
        open_events.borrow().as_slice(),
        &[true, false],
        "Tab should emit one semantic close after the trigger open"
    );
    assert!(
        cx.debug_selector_is_focused("menu:tab-menu:trigger"),
        "Tab dismissal should restore the root trigger after the owner commits closed"
    );
}

#[open_gpui::test]
fn menu_escape_closes_the_deepest_registered_branch_before_the_root(
    cx: &mut open_gpui::TestAppContext,
) {
    struct TestView {
        open_events: Rc<RefCell<Vec<bool>>>,
    }

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let open_events = self.open_events.clone();
            div().size_full().child(
                Menu::new("layered-menu", "Layered menu")
                    .default_focused_value("sort")
                    .item(MenuItem::submenu(
                        "sort",
                        "Sort by",
                        [MenuItem::submenu(
                            "then",
                            "Then by",
                            [MenuItem::action("name", "Name")],
                        )],
                    ))
                    .item(MenuItem::action("close", "Close"))
                    .on_open_change(move |intent, _, _| {
                        open_events.borrow_mut().push(intent.desired_open());
                    }),
            )
        }
    }

    let open_events = Rc::new(RefCell::new(Vec::new()));
    let (_, cx) = cx.add_window_view(|_, _| TestView {
        open_events: open_events.clone(),
    });
    cx.update(|window, cx| window.draw(cx).clear());

    let trigger = cx
        .debug_bounds("menu:layered-menu:trigger")
        .expect("menu trigger should render");
    cx.simulate_click(trigger.center(), Default::default());
    cx.update(|window, cx| window.draw(cx).clear());
    cx.simulate_keystrokes("right");
    cx.update(|window, cx| window.draw(cx).clear());
    cx.simulate_keystrokes("right");
    cx.update(|window, cx| window.draw(cx).clear());

    let snapshot = cx.update(|window, cx| {
        WindowOverlayRuntime::for_window(window, cx)
            .snapshot(window, cx)
            .expect("menu overlay snapshot should resolve")
    });
    let root = snapshot
        .layers()
        .iter()
        .find(|layer| layer.id().as_str() == "menu:layered-menu")
        .expect("menu root should be registered");
    let branch = snapshot
        .layers()
        .iter()
        .find(|layer| layer.id().as_str() == "menu:layered-menu:branch:0:sort")
        .expect("open submenu should own a registered child layer");
    let nested_branch = snapshot
        .layers()
        .iter()
        .find(|layer| layer.id().as_str() == "menu:layered-menu:branch:0:sort/0:then")
        .expect("nested submenu should own a registered grandchild layer");
    assert_eq!(
        branch.parent().map(|parent| parent.as_str()),
        Some(root.id().as_str())
    );
    assert_eq!(branch.phase(), OverlayLayerPhase::Open);
    assert_eq!(
        nested_branch.parent().map(|parent| parent.as_str()),
        Some(branch.id().as_str())
    );
    assert_eq!(nested_branch.phase(), OverlayLayerPhase::Open);
    assert!(
        cx.debug_selector_is_focused("menu:layered-menu:panel:0:sort/0:then"),
        "the deepest branch surface should own focus"
    );

    cx.simulate_keystrokes("escape");
    cx.run_until_parked();
    cx.update(|window, cx| window.draw(cx).clear());
    assert!(cx.debug_bounds("menu:layered-menu:content").is_some());
    assert!(
        cx.debug_bounds("menu:layered-menu:item:0:sort/0:then/0:name")
            .is_none(),
        "the first Escape should close only the deepest submenu branch"
    );
    assert!(
        cx.debug_selector_is_focused("menu:layered-menu:item:0:sort/0:then"),
        "closing a branch should restore its parent menu item"
    );
    let snapshot = cx.update(|window, cx| {
        WindowOverlayRuntime::for_window(window, cx)
            .snapshot(window, cx)
            .expect("menu snapshot after nested close should resolve")
    });
    let nested_branch = snapshot
        .layers()
        .iter()
        .find(|layer| layer.id().as_str() == "menu:layered-menu:branch:0:sort/0:then")
        .expect("closed nested branch should retain its reusable registration");
    assert_eq!(nested_branch.phase(), OverlayLayerPhase::Hidden);
    assert_eq!(open_events.borrow().as_slice(), &[true]);

    cx.simulate_keystrokes("escape");
    cx.run_until_parked();
    cx.update(|window, cx| window.draw(cx).clear());
    assert!(cx.debug_bounds("menu:layered-menu:content").is_some());
    assert!(
        cx.debug_bounds("menu:layered-menu:item:0:sort/0:then")
            .is_none(),
        "the second Escape should close the parent submenu branch"
    );
    assert!(cx.debug_selector_is_focused("menu:layered-menu:item:0:sort"));
    let snapshot = cx.update(|window, cx| {
        WindowOverlayRuntime::for_window(window, cx)
            .snapshot(window, cx)
            .expect("menu snapshot after parent branch close should resolve")
    });
    let branch = snapshot
        .layers()
        .iter()
        .find(|layer| layer.id().as_str() == "menu:layered-menu:branch:0:sort")
        .expect("closed parent branch should retain its reusable registration");
    assert_eq!(branch.phase(), OverlayLayerPhase::Hidden);
    assert_eq!(open_events.borrow().as_slice(), &[true]);

    cx.simulate_keystrokes("escape");
    cx.run_until_parked();
    cx.update(|window, cx| window.draw(cx).clear());
    assert!(cx.debug_bounds("menu:layered-menu:content").is_none());
    assert!(cx.debug_selector_is_focused("menu:layered-menu:trigger"));
    assert_eq!(open_events.borrow().as_slice(), &[true, false]);
}

#[open_gpui::test]
fn controlled_menu_trigger_records_one_trigger_close_intent(cx: &mut open_gpui::TestAppContext) {
    struct TestView {
        open_intents: Rc<RefCell<Vec<OverlayOpenIntent>>>,
    }

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let open_intents = self.open_intents.clone();
            div().size_full().child(
                Menu::new("controlled-trigger-menu", "Controlled menu")
                    .open(true)
                    .item(MenuItem::action("first", "First"))
                    .on_open_change(move |intent, _, _| {
                        open_intents.borrow_mut().push(intent);
                    }),
            )
        }
    }

    let open_intents = Rc::new(RefCell::new(Vec::new()));
    let (_, cx) = cx.add_window_view(|_, _| TestView {
        open_intents: open_intents.clone(),
    });
    cx.update(|window, cx| window.draw(cx).clear());

    let trigger = cx
        .debug_bounds("menu:controlled-trigger-menu:trigger")
        .expect("controlled menu trigger should render");
    let content = cx
        .debug_bounds("menu:controlled-trigger-menu:content")
        .expect("controlled menu content should render");
    assert!(
        !content.contains(&trigger.center()),
        "menu content {content:?} must not occlude its trigger {trigger:?}"
    );
    cx.simulate_click(trigger.center(), Default::default());
    let requested = cx.update(|window, cx| {
        WindowOverlayRuntime::for_window(window, cx)
            .snapshot(window, cx)
            .expect("requested menu snapshot should resolve")
    });
    let requested_layer = requested
        .layers()
        .iter()
        .find(|layer| layer.id().as_str() == "menu:controlled-trigger-menu")
        .expect("requested menu root should remain registered");
    assert_eq!(requested_layer.phase(), OverlayLayerPhase::CloseRequested);
    assert_eq!(
        requested_layer.pending_intent(),
        Some(DismissReason::Trigger)
    );
    assert_eq!(open_intents.borrow().len(), 1);
    assert!(!open_intents.borrow()[0].desired_open());
    assert_eq!(open_intents.borrow()[0].reason(), DismissReason::Trigger);
    assert!(open_intents.borrow()[0].revision().is_some());

    cx.update(|window, cx| window.draw(cx).clear());
    let refused = cx.update(|window, cx| {
        WindowOverlayRuntime::for_window(window, cx)
            .snapshot(window, cx)
            .expect("refused menu snapshot should resolve")
    });
    let refused_layer = refused
        .layers()
        .iter()
        .find(|layer| layer.id().as_str() == "menu:controlled-trigger-menu")
        .expect("refused menu root should remain registered");
    assert_eq!(refused_layer.phase(), OverlayLayerPhase::CloseRequested);

    cx.simulate_click(trigger.center(), Default::default());
    cx.update(|window, cx| window.draw(cx).clear());

    let snapshot = cx.update(|window, cx| {
        WindowOverlayRuntime::for_window(window, cx)
            .snapshot(window, cx)
            .expect("controlled menu snapshot should resolve")
    });
    let layer = snapshot
        .layers()
        .iter()
        .find(|layer| layer.id().as_str() == "menu:controlled-trigger-menu")
        .expect("controlled menu root should remain registered");
    assert_eq!(layer.phase(), OverlayLayerPhase::CloseRequested);
    assert_eq!(layer.pending_intent(), Some(DismissReason::Trigger));
    assert_eq!(open_intents.borrow().len(), 1);
    assert!(
        cx.debug_bounds("menu:controlled-trigger-menu:content")
            .is_some()
    );
}

#[open_gpui::test]
fn menu_owner_unmount_allows_same_id_remount_without_pre_settling_cleanup(
    cx: &mut open_gpui::TestAppContext,
) {
    struct TestView {
        mounted: bool,
        fallback_focus: FocusHandle,
        fallback_lease: Option<WindowFocusFallbackLease>,
    }

    impl Render for TestView {
        fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            if self.fallback_lease.is_none() {
                self.fallback_lease = Some(
                    WindowOverlayRuntime::for_window(window, cx)
                        .register_window_fallback(
                            FocusTargetRegistration::new(
                                "menu-remount-window-fallback",
                                &self.fallback_focus,
                            ),
                            window,
                            cx,
                        )
                        .expect("menu remount fallback should register in its window"),
                );
            }

            div()
                .size_full()
                .child(
                    div()
                        .id("menu-remount-window-fallback")
                        .debug_selector(|| "menu-test:remount-window-fallback".to_owned())
                        .focusable()
                        .track_focus(&self.fallback_focus)
                        .tab_index(0)
                        .child("Fallback"),
                )
                .when(self.mounted, |this| {
                    this.child(
                        Menu::new("remounted-menu", "Remounted menu")
                            .default_focused_value("branch")
                            .item(MenuItem::submenu(
                                "branch",
                                "Branch",
                                [MenuItem::action("child", "Child")],
                            )),
                    )
                })
        }
    }

    let (view, cx) = cx.add_window_view(|_, cx| TestView {
        mounted: true,
        fallback_focus: cx.focus_handle(),
        fallback_lease: None,
    });
    cx.update(|window, cx| window.draw(cx).clear());
    cx.update_window_entity(&view, |view, window, cx| {
        view.fallback_focus.focus(window, cx);
    });
    let trigger = cx
        .debug_bounds("menu:remounted-menu:trigger")
        .expect("mounted menu trigger should render");
    cx.simulate_click(trigger.center(), Default::default());
    cx.update(|window, cx| window.draw(cx).clear());
    cx.simulate_keystrokes("right");
    cx.update(|window, cx| window.draw(cx).clear());

    cx.update_window_entity(&view, |view, _, cx| {
        view.mounted = false;
        cx.notify();
    });
    cx.update(|window, cx| window.draw(cx).clear());
    cx.run_until_parked();

    cx.update_window_entity(&view, |view, _, cx| {
        view.mounted = true;
        cx.notify();
    });
    cx.update(|window, cx| window.draw(cx).clear());
    cx.run_until_parked();
    cx.update(|window, cx| {
        window.drain_next_frame_callbacks_for_test(cx);
        window.draw(cx).clear();
    });
    assert!(cx.debug_bounds("menu:remounted-menu:trigger").is_some());
    let remounted = cx.update(|window, cx| {
        WindowOverlayRuntime::for_window(window, cx)
            .snapshot(window, cx)
            .expect("remounted menu snapshot should resolve")
    });
    assert!(
        remounted
            .layers()
            .iter()
            .any(|layer| layer.id().as_str() == "menu:remounted-menu")
    );

    let trigger = cx
        .debug_bounds("menu:remounted-menu:trigger")
        .expect("remounted menu trigger should remain interactive");
    cx.simulate_click(trigger.center(), Default::default());
    cx.update(|window, cx| window.draw(cx).clear());
    cx.simulate_keystrokes("right");
    cx.update(|window, cx| window.draw(cx).clear());
    assert!(
        cx.debug_bounds("menu:remounted-menu:item:0:branch/0:child")
            .is_some(),
        "same-ID remount should register and reopen its submenu branch"
    );

    cx.update_window_entity(&view, |view, _, cx| {
        view.mounted = false;
        cx.notify();
    });
    cx.update(|window, cx| window.draw(cx).clear());
    cx.run_until_parked();
    cx.update(|window, cx| {
        window.drain_next_frame_callbacks_for_test(cx);
        window.draw(cx).clear();
    });
    cx.run_until_parked();
    cx.update(|window, cx| {
        window.drain_next_frame_callbacks_for_test(cx);
        window.draw(cx).clear();
    });
    cx.run_until_parked();

    assert!(
        cx.debug_selector_is_focused("menu-test:remount-window-fallback"),
        "owner release should restore the window fallback after the trigger and subtree vanish"
    );
    let released = cx.update(|window, cx| {
        WindowOverlayRuntime::for_window(window, cx)
            .snapshot(window, cx)
            .expect("released Menu snapshot should resolve")
    });
    assert!(
        released
            .layers()
            .iter()
            .all(|layer| !layer.id().as_str().starts_with("menu:remounted-menu")),
        "owner release should remove the Menu root and every descendant registration"
    );
}

#[open_gpui::test]
fn menu_submenu_outside_press_preserves_the_root_consumption_boundary(
    cx: &mut open_gpui::TestAppContext,
) {
    struct TestView {
        outside_policy: OutsidePressPolicy,
        underlay_clicks: Rc<Cell<usize>>,
    }

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let underlay_clicks = self.underlay_clicks.clone();
            div()
                .relative()
                .size_full()
                .child(
                    div()
                        .id("menu-outside-underlay")
                        .debug_selector(|| "menu-test:outside-underlay".to_owned())
                        .absolute()
                        .left(px(520.0))
                        .top(px(320.0))
                        .w(px(120.0))
                        .h(px(40.0))
                        .on_click(move |_, _, _| {
                            underlay_clicks.set(underlay_clicks.get() + 1);
                        })
                        .child("Underlay"),
                )
                .child(
                    Menu::new("outside-boundary-menu", "Outside boundary")
                        .default_focused_value("branch")
                        .outside_press_policy(self.outside_policy)
                        .item(MenuItem::submenu(
                            "branch",
                            "Branch",
                            [MenuItem::action("child", "Child")],
                        )),
                )
        }
    }

    let underlay_clicks = Rc::new(Cell::new(0));
    let (view, cx) = cx.add_window_view(|_, _| TestView {
        outside_policy: OutsidePressPolicy::DismissAndConsume,
        underlay_clicks: underlay_clicks.clone(),
    });
    cx.update(|window, cx| window.draw(cx).clear());
    let trigger = cx
        .debug_bounds("menu:outside-boundary-menu:trigger")
        .expect("menu trigger should render");
    let underlay = cx
        .debug_bounds("menu-test:outside-underlay")
        .expect("underlay target should render");

    cx.simulate_click(trigger.center(), Default::default());
    cx.update(|window, cx| window.draw(cx).clear());
    cx.simulate_keystrokes("right");
    cx.update(|window, cx| window.draw(cx).clear());
    assert!(
        cx.debug_bounds("menu:outside-boundary-menu:item:0:branch/0:child")
            .is_some()
    );
    cx.simulate_click(trigger.center(), Default::default());
    cx.update(|window, cx| window.draw(cx).clear());
    assert!(
        cx.debug_bounds("menu:outside-boundary-menu:content")
            .is_none(),
        "a submenu must not consume a click owned by the root Menu trigger"
    );

    cx.simulate_click(trigger.center(), Default::default());
    cx.update(|window, cx| window.draw(cx).clear());
    cx.simulate_keystrokes("right");
    cx.update(|window, cx| window.draw(cx).clear());
    cx.simulate_click(underlay.center(), Default::default());
    cx.update(|window, cx| window.draw(cx).clear());
    assert_eq!(underlay_clicks.get(), 0);
    assert!(
        cx.debug_bounds("menu:outside-boundary-menu:content")
            .is_some()
    );
    assert!(
        cx.debug_bounds("menu:outside-boundary-menu:item:0:branch/0:child")
            .is_none()
    );

    cx.simulate_click(trigger.center(), Default::default());
    cx.update(|window, cx| window.draw(cx).clear());
    cx.update_window_entity(&view, |view, _, cx| {
        view.outside_policy = OutsidePressPolicy::DismissAndPassThrough;
        cx.notify();
    });
    cx.update(|window, cx| window.draw(cx).clear());
    cx.simulate_click(trigger.center(), Default::default());
    cx.update(|window, cx| window.draw(cx).clear());
    cx.simulate_keystrokes("right");
    cx.update(|window, cx| window.draw(cx).clear());
    cx.simulate_click(underlay.center(), Default::default());
    cx.update(|window, cx| window.draw(cx).clear());
    assert_eq!(
        underlay_clicks.get(),
        1,
        "an explicit root pass-through policy should reach the underlay once"
    );
    assert!(
        cx.debug_bounds("menu:outside-boundary-menu:content")
            .is_some()
    );
}

#[open_gpui::test]
fn dynamic_menu_branch_churn_releases_stale_registrations(cx: &mut open_gpui::TestAppContext) {
    struct TestView {
        generation: usize,
    }

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            div()
                .size_full()
                .child(Menu::new("dynamic-branch-menu", "Dynamic branches").item(
                    MenuItem::submenu(
                        format!("branch/{}/%", self.generation),
                        format!("Branch {}", self.generation),
                        [MenuItem::action("child", "Child")],
                    ),
                ))
        }
    }

    let (view, cx) = cx.add_window_view(|_, _| TestView { generation: 0 });
    cx.update(|window, cx| window.draw(cx).clear());

    let trigger = cx
        .debug_bounds("menu:dynamic-branch-menu:trigger")
        .expect("dynamic menu trigger should render");
    cx.simulate_click(trigger.center(), Default::default());
    cx.update(|window, cx| window.draw(cx).clear());
    cx.simulate_keystrokes("right");
    cx.update(|window, cx| window.draw(cx).clear());
    assert!(
        cx.debug_selector_is_focused("menu:dynamic-branch-menu:panel:0:branch%2F0%2F%25"),
        "the original dynamic branch should own focus before it is removed"
    );

    for generation in 1..=16 {
        cx.update_window_entity(&view, |view, _, cx| {
            view.generation = generation;
            cx.notify();
        });
        cx.update(|window, cx| window.draw(cx).clear());
        let callbacks = cx.update(|window, cx| {
            let callbacks = window.drain_next_frame_callbacks_for_test(cx);
            window.draw(cx).clear();
            callbacks
        });
        assert!(
            callbacks > 0,
            "stale branch cleanup should request one frame"
        );
        if generation == 1 {
            cx.run_until_parked();
            cx.update(|window, cx| window.draw(cx).clear());
            assert!(
                cx.debug_selector_is_focused("menu:dynamic-branch-menu:content"),
                "removing the focused branch should restore the still-open root menu surface"
            );
        }

        let snapshot = cx.update(|window, cx| {
            WindowOverlayRuntime::for_window(window, cx)
                .snapshot(window, cx)
                .expect("dynamic menu snapshot should resolve")
        });
        let layers = snapshot
            .layers()
            .iter()
            .filter(|layer| layer.id().as_str().starts_with("menu:dynamic-branch-menu"))
            .collect::<Vec<_>>();
        assert_eq!(
            layers.len(),
            2,
            "runtime should retain only the root and current dynamic branch: {layers:?}"
        );
        let branch_id = format!("menu:dynamic-branch-menu:branch:0:branch%2F{generation}%2F%25");
        assert!(layers.iter().any(|layer| layer.id().as_str() == branch_id));
    }
}

#[open_gpui::test]
fn menu_branch_ids_do_not_collide_when_values_contain_path_delimiters(
    cx: &mut open_gpui::TestAppContext,
) {
    struct TestView {
        nested: bool,
    }

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let item = if self.nested {
                MenuItem::submenu(
                    "a",
                    "A",
                    [MenuItem::submenu(
                        "b",
                        "B",
                        [MenuItem::action("child", "Child")],
                    )],
                )
            } else {
                MenuItem::submenu(
                    "a/0:b",
                    "Flat delimiter",
                    [MenuItem::action("child", "Child")],
                )
            };
            div()
                .size_full()
                .child(Menu::new("path-key-menu", "Path keys").item(item))
        }
    }

    let (view, cx) = cx.add_window_view(|_, _| TestView { nested: false });
    cx.update(|window, cx| window.draw(cx).clear());
    let initial = cx.update(|window, cx| {
        WindowOverlayRuntime::for_window(window, cx)
            .snapshot(window, cx)
            .expect("initial path-key snapshot should resolve")
    });
    assert!(
        initial
            .layers()
            .iter()
            .any(|layer| { layer.id().as_str() == "menu:path-key-menu:branch:0:a%2F0:b" })
    );

    cx.update_window_entity(&view, |view, _, cx| {
        view.nested = true;
        cx.notify();
    });
    cx.update(|window, cx| window.draw(cx).clear());

    let transitional = cx.update(|window, cx| {
        WindowOverlayRuntime::for_window(window, cx)
            .snapshot(window, cx)
            .expect("path-key transition snapshot should resolve")
    });
    let transitional_ids = transitional
        .layers()
        .iter()
        .filter(|layer| layer.id().as_str().starts_with("menu:path-key-menu"))
        .map(|layer| layer.id().as_str())
        .collect::<Vec<_>>();
    assert!(
        transitional_ids.contains(&"menu:path-key-menu:branch:0:a/0:b"),
        "nested branch should use its structural path identity: {transitional_ids:?}"
    );

    cx.update(|window, cx| {
        window.drain_next_frame_callbacks_for_test(cx);
        window.draw(cx).clear();
    });
    let settled = cx.update(|window, cx| {
        WindowOverlayRuntime::for_window(window, cx)
            .snapshot(window, cx)
            .expect("settled path-key snapshot should resolve")
    });
    let ids = settled
        .layers()
        .iter()
        .filter(|layer| layer.id().as_str().starts_with("menu:path-key-menu"))
        .map(|layer| layer.id().as_str())
        .collect::<Vec<_>>();
    assert_eq!(
        ids.len(),
        3,
        "only root and two nested branches should remain"
    );
    assert!(!ids.contains(&"menu:path-key-menu:branch:0:a%2F0:b"));
}

#[open_gpui::test]
fn menu_runtime_keyboard_submenu_opens_and_selects_child(cx: &mut open_gpui::TestAppContext) {
    struct TestView {
        selections: Rc<RefCell<Vec<MenuSelection>>>,
    }

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let selections = self.selections.clone();

            div().size_full().child(
                Menu::new("runtime-submenu", "Runtime submenu")
                    .default_focused_value("sort")
                    .item(MenuItem::action("open", "Open"))
                    .item(MenuItem::submenu(
                        "sort",
                        "Sort by",
                        [
                            MenuItem::action("name", "Name"),
                            MenuItem::action("modified", "Modified"),
                        ],
                    ))
                    .on_select(move |selection, _, _| {
                        selections.borrow_mut().push(selection);
                    }),
            )
        }
    }

    let selections = Rc::new(RefCell::new(Vec::new()));
    let (_, cx) = cx.add_window_view(|_, _| TestView {
        selections: selections.clone(),
    });
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    let trigger = cx
        .debug_bounds("menu:runtime-submenu:trigger")
        .expect("runtime submenu trigger should render");
    cx.simulate_click(trigger.center(), Default::default());
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });
    assert!(
        cx.debug_bounds("menu:runtime-submenu:item:1:sort/0:name")
            .is_none(),
        "submenu child should not render before opening the branch"
    );

    cx.simulate_keystrokes("right");
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });
    assert!(
        cx.debug_bounds("menu:runtime-submenu:item:1:sort/0:name")
            .is_some(),
        "Right should open the focused submenu branch"
    );

    cx.simulate_keystrokes("left");
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });
    assert!(
        cx.debug_bounds("menu:runtime-submenu:item:1:sort/0:name")
            .is_none(),
        "Left should close the active submenu branch and return focus to the trigger row"
    );

    cx.simulate_keystrokes("right");
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });
    assert!(
        cx.debug_bounds("menu:runtime-submenu:item:1:sort/0:name")
            .is_some(),
        "Right should reopen the submenu branch after closing it"
    );

    cx.simulate_keystrokes("enter");
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    let after_enter = selections.borrow().clone();
    assert_eq!(after_enter.len(), 1);
    assert_eq!(after_enter[0].path_key(), "1:sort/0:name");
    assert_eq!(after_enter[0].value(), "name");
}

#[open_gpui::test]
fn menu_runtime_hover_opens_submenu_and_preserves_child_focus(cx: &mut open_gpui::TestAppContext) {
    struct TestView;

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            div().size_full().child(
                Menu::new("hover-submenu", "Hover submenu")
                    .item(MenuItem::action("open", "Open"))
                    .item(MenuItem::submenu(
                        "sort",
                        "Sort by",
                        [
                            MenuItem::action("name", "Name"),
                            MenuItem::action("modified", "Modified"),
                        ],
                    ))
                    .item(MenuItem::action("rename", "Rename")),
            )
        }
    }

    let (_, cx) = cx.add_window_view(|_, _| TestView);
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    let trigger = cx
        .debug_bounds("menu:hover-submenu:trigger")
        .expect("hover submenu trigger should render");
    cx.simulate_click(trigger.center(), Default::default());
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });
    assert!(
        cx.debug_bounds("menu:hover-submenu:item:1:sort/0:name")
            .is_none(),
        "submenu child should not render before hovering the submenu trigger"
    );

    let sort = cx
        .debug_bounds("menu:hover-submenu:item:1:sort")
        .expect("submenu trigger item should render");
    cx.simulate_mouse_move(sort.center(), None, Default::default());
    cx.executor().advance_clock(Duration::from_millis(200));
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });
    assert!(
        cx.debug_bounds("menu:hover-submenu:item:1:sort/0:name")
            .is_some(),
        "hovering a submenu trigger should open its branch after the hover delay"
    );

    let child = cx
        .debug_bounds("menu:hover-submenu:item:1:sort/0:name")
        .expect("submenu child should render after hover");
    assert!(
        child.origin.x > sort.bottom_right().x,
        "submenu child should open in a floating panel to the right"
    );
    assert!(
        child.origin.y < sort.bottom_right().y,
        "submenu child should not be stacked below the trigger row"
    );
    cx.simulate_mouse_move(child.center(), None, Default::default());
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });
    assert!(
        cx.debug_bounds("menu:hover-submenu:item:1:sort/0:name")
            .is_some(),
        "hovering inside the open submenu branch should preserve that branch"
    );

    let rename = cx
        .debug_bounds("menu:hover-submenu:item:2:rename")
        .expect("next root item should render");
    cx.simulate_mouse_move(rename.center(), None, Default::default());
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });
    assert!(
        cx.debug_bounds("menu:hover-submenu:item:1:sort/0:name")
            .is_some(),
        "hovering another root item should not close the submenu before the close delay"
    );
    cx.executor().advance_clock(Duration::from_millis(200));
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });
    assert!(
        cx.debug_bounds("menu:hover-submenu:item:1:sort/0:name")
            .is_none(),
        "hovering another root item should close the previous submenu branch after the close delay"
    );
}

#[open_gpui::test]
fn menu_runtime_hover_switches_between_submenu_branches(cx: &mut open_gpui::TestAppContext) {
    struct TestView;

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            div().size_full().child(
                Menu::new("hover-switch-submenu", "Hover switch submenu")
                    .item(MenuItem::action("open", "Open"))
                    .item(MenuItem::submenu(
                        "sort",
                        "Sort by",
                        [
                            MenuItem::action("name", "Name"),
                            MenuItem::action("modified", "Modified"),
                        ],
                    ))
                    .item(MenuItem::submenu(
                        "group",
                        "Group by",
                        [
                            MenuItem::action("kind", "Kind"),
                            MenuItem::action("owner", "Owner"),
                        ],
                    ))
                    .item(MenuItem::action("rename", "Rename")),
            )
        }
    }

    let (_, cx) = cx.add_window_view(|_, _| TestView);
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    let trigger = cx
        .debug_bounds("menu:hover-switch-submenu:trigger")
        .expect("hover switch submenu trigger should render");
    cx.simulate_click(trigger.center(), Default::default());
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    let sort = cx
        .debug_bounds("menu:hover-switch-submenu:item:1:sort")
        .expect("sort submenu trigger should render");
    cx.simulate_mouse_move(sort.center(), None, Default::default());
    cx.executor().advance_clock(Duration::from_millis(200));
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });
    assert!(
        cx.debug_bounds("menu:hover-switch-submenu:item:1:sort/0:name")
            .is_some(),
        "hovering the first submenu trigger should open its branch after the hover delay"
    );
    assert!(
        cx.debug_bounds("menu:hover-switch-submenu:item:2:group/0:kind")
            .is_none(),
        "sibling submenu branch should stay hidden before it is hovered"
    );

    let group = cx
        .debug_bounds("menu:hover-switch-submenu:item:2:group")
        .expect("group submenu trigger should render");
    cx.simulate_mouse_move(group.center(), None, Default::default());
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });
    assert!(
        cx.debug_bounds("menu:hover-switch-submenu:item:1:sort/0:name")
            .is_some(),
        "switching submenu triggers should keep the previous branch visible until the new hover delay elapses"
    );
    assert!(
        cx.debug_bounds("menu:hover-switch-submenu:item:2:group/0:kind")
            .is_none(),
        "sibling submenu branch should still be hidden before the hover delay elapses"
    );
    cx.executor().advance_clock(Duration::from_millis(200));
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });
    assert!(
        cx.debug_bounds("menu:hover-switch-submenu:item:2:group/0:kind")
            .is_some(),
        "hovering a sibling submenu trigger should open the sibling branch after the hover delay"
    );
    assert!(
        cx.debug_bounds("menu:hover-switch-submenu:item:1:sort/0:name")
            .is_none(),
        "switching submenu triggers should close the previous branch"
    );

    cx.simulate_mouse_move(sort.center(), None, Default::default());
    cx.executor().advance_clock(Duration::from_millis(200));
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });
    assert!(
        cx.debug_bounds("menu:hover-switch-submenu:item:1:sort/0:name")
            .is_some(),
        "switching back to an earlier sibling should open the new branch"
    );
    assert!(
        cx.debug_bounds("menu:hover-switch-submenu:item:2:group/0:kind")
            .is_none(),
        "reverse sibling switching should close the later branch first"
    );
    assert!(
        cx.debug_selector_is_focused("menu:hover-switch-submenu:panel:1:sort"),
        "the newly opened branch must win focus after reverse sibling switching"
    );

    let rename = cx
        .debug_bounds("menu:hover-switch-submenu:item:3:rename")
        .expect("next root item should render");
    cx.simulate_mouse_move(rename.center(), None, Default::default());
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });
    assert!(
        cx.debug_bounds("menu:hover-switch-submenu:item:1:sort/0:name")
            .is_some(),
        "hovering a plain root item should keep the open submenu visible until the close delay elapses"
    );
    cx.executor().advance_clock(Duration::from_millis(200));
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });
    assert!(
        cx.debug_bounds("menu:hover-switch-submenu:item:1:sort/0:name")
            .is_none(),
        "hovering a plain root item should close the open submenu branch after the close delay"
    );
}

#[test]
fn menu_state_models_default_open_disabled_and_policy_overrides() {
    let state = Menu::new("disabled-menu", "Disabled")
        .default_open(true)
        .disabled(true)
        .outside_press_policy(OutsidePressPolicy::Ignore)
        .escape_key_policy(EscapeKeyPolicy::Ignore)
        .initial_focus_intent(InitialFocusIntent::None)
        .focus_restore_intent(FocusRestoreIntent::None)
        .small()
        .item(MenuItem::action("open", "Open"))
        .state();

    assert_eq!(state.open_mode(), MenuOpenMode::Uncontrolled);
    assert!(state.default_open());
    assert!(state.disabled());
    assert!(!state.open());
    assert_eq!(state.size(), Size::Small);
    assert_eq!(state.outside_press_policy(), OutsidePressPolicy::Ignore);
    assert_eq!(state.escape_key_policy(), EscapeKeyPolicy::Ignore);
    assert_eq!(state.initial_focus_intent(), &InitialFocusIntent::None);
    assert_eq!(state.focus_restore_intent(), &FocusRestoreIntent::None);
    assert!(!state.overlay().should_render_deferred_layer());
}

#[test]
fn context_menu_state_reuses_menu_model_and_point_anchor_placement() {
    let anchor = point(px(280.0), px(160.0));
    let state = ContextMenu::new("canvas-context-menu", "Canvas menu")
        .open(true)
        .anchor_point(anchor)
        .default_focused_value("duplicate")
        .item(MenuItem::action("duplicate", "Duplicate"))
        .item(MenuItem::separator("separator"))
        .item(MenuItem::action("delete", "Delete").disabled(true))
        .state();

    assert!(state.open());
    assert_eq!(state.open_mode(), MenuOpenMode::Controlled);
    let neutral_anchor = ui_point(ui_px(280.0), ui_px(160.0));
    assert_eq!(state.anchor_point(), neutral_anchor);
    assert_eq!(state.content_role(), Role::Menu);
    assert_eq!(state.menu().focused_value(), Some("duplicate"));
    assert_eq!(state.menu().items()[1].kind(), MenuItemKind::Separator);
    assert!(!state.menu().items()[2].activation_enabled());
    assert_eq!(state.overlay().policy().kind(), OverlayLayerKind::Menu);
    assert!(state.overlay().wants_outside_press_handler());
    let placement_input = state.placement_input();
    assert_eq!(placement_input.side(), OverlayPlacementSide::Bottom);
    assert_eq!(
        placement_input.alignment(),
        OverlayPlacementAlignment::Start
    );
    assert_eq!(placement_input.offset(), ui_px(0.0));
    let placement_row_gap =
        ui_px(4.0).as_f32() * (state.menu().visible_items().len().saturating_sub(1) as f32);
    assert_eq!(
        placement_input.content_size(),
        ui_size(
            ui_px(state.metrics().min_width().as_f32()),
            ui_px(
                state.metrics().surface_padding().as_f32() * 2.0
                    + state.metrics().item_height().as_f32()
                        * state.menu().visible_items().len() as f32
                    + placement_row_gap
            )
        )
    );
    let placement = GpuiOverlayPlacement::resolve(placement_input, DEFAULT_OVERLAY_SAFE_MARGIN);
    assert_eq!(placement.anchor(), Anchor::TopLeft);
    assert_eq!(
        placement_input.preferred_anchor_bounds(),
        Some(open_gpui_ui_core::anchor_rect_from_point(neutral_anchor))
    );
    assert_eq!(placement.position(), Some(point(px(280.0), px(161.0))));
    assert_eq!(placement.snap_margin(), DEFAULT_OVERLAY_SAFE_MARGIN);
}

#[test]
fn context_menu_state_defaults_focus_to_first_focusable_item_when_open() {
    let anchor = point(px(280.0), px(160.0));
    let state = ContextMenu::new("canvas-context-menu", "Canvas menu")
        .open(true)
        .anchor_point(anchor)
        .item(MenuItem::separator("separator"))
        .item(MenuItem::action("duplicate", "Duplicate"))
        .item(MenuItem::action("delete", "Delete").disabled(true))
        .state();

    assert_eq!(state.menu().focused_value(), Some("duplicate"));
    assert!(state.menu().items()[0].kind() == MenuItemKind::Separator);
}

#[test]
fn context_menu_state_navigation_target_prefers_runtime_focused_value() {
    let anchor = point(px(280.0), px(160.0));
    let state = ContextMenu::new("runtime-context-menu", "Runtime context menu")
        .open(true)
        .anchor_point(anchor)
        .default_focused_value("copy")
        .item(MenuItem::action("cut", "Cut"))
        .item(MenuItem::action("copy", "Copy"))
        .item(MenuItem::action("select-all", "Select all"))
        .state();

    assert_eq!(state.menu().focused_value(), Some("copy"));
    assert_eq!(
        state
            .menu()
            .navigation_target("down")
            .map(|item| item.value()),
        Some("select-all")
    );
}

#[test]
fn context_menu_state_reuses_visible_submenu_navigation_contract() {
    let anchor = point(px(320.0), px(220.0));
    let state = ContextMenu::new("nested-context-menu", "Nested context menu")
        .open(true)
        .anchor_point(anchor)
        .default_focused_value("organize")
        .item(MenuItem::action("duplicate", "Duplicate"))
        .item(MenuItem::submenu(
            "organize",
            "Organize",
            [
                MenuItem::action("move", "Move"),
                MenuItem::action("tag", "Tag"),
            ],
        ))
        .state();

    assert_eq!(state.menu().focused_value(), Some("organize"));
    let opened = state
        .menu()
        .submenu_navigation_target("right")
        .expect("ContextMenu should reuse Menu submenu navigation");
    assert_eq!(opened.open_path_key().as_deref(), Some("1:organize"));
    assert_eq!(opened.focused_path_key(), "1:organize/0:move");
}

#[test]
fn context_menu_state_uses_clamped_visible_menu_size_for_point_placement() {
    let state = ContextMenu::new("edge-long-context-menu", "Edge long context menu")
        .open(true)
        .anchor_point(point(px(960.0), px(560.0)))
        .items(
            (0..12).map(|index| MenuItem::action(format!("item-{index}"), format!("Item {index}"))),
        )
        .state();

    assert!(state.menu().scrollable_content());
    assert_eq!(
        state.placement_input().content_size(),
        ui_size(state.metrics().min_width(), state.metrics().max_height())
    );
}

#[open_gpui::test]
fn context_menu_escape_closes_the_deepest_registered_branch_before_the_root(
    cx: &mut open_gpui::TestAppContext,
) {
    struct TestView {
        open_events: Rc<RefCell<Vec<bool>>>,
    }

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let open_events = self.open_events.clone();
            div().size_full().child(
                ContextMenu::new("layered-context-menu", "Layered context menu")
                    .default_focused_value("organize")
                    .item(MenuItem::submenu(
                        "organize",
                        "Organize",
                        [MenuItem::submenu(
                            "more",
                            "More",
                            [MenuItem::action("archive", "Archive")],
                        )],
                    ))
                    .item(MenuItem::action("close", "Close"))
                    .on_open_change(move |intent, _, _| {
                        open_events.borrow_mut().push(intent.desired_open());
                    }),
            )
        }
    }

    let open_events = Rc::new(RefCell::new(Vec::new()));
    let (_, cx) = cx.add_window_view(|_, _| TestView {
        open_events: open_events.clone(),
    });
    cx.update(|window, cx| window.draw(cx).clear());

    let hotspot = cx
        .debug_bounds("context-menu:layered-context-menu:hotspot")
        .expect("context menu hotspot should render");
    cx.simulate_mouse_down(hotspot.center(), MouseButton::Right, Default::default());
    cx.simulate_mouse_up(hotspot.center(), MouseButton::Right, Default::default());
    cx.run_until_parked();
    cx.update(|window, cx| window.draw(cx).clear());
    cx.simulate_keystrokes("right");
    cx.update(|window, cx| window.draw(cx).clear());
    cx.simulate_keystrokes("right");
    cx.update(|window, cx| window.draw(cx).clear());

    let snapshot = cx.update(|window, cx| {
        WindowOverlayRuntime::for_window(window, cx)
            .snapshot(window, cx)
            .expect("context menu overlay snapshot should resolve")
    });
    let root = snapshot
        .layers()
        .iter()
        .find(|layer| layer.id().as_str() == "context-menu:layered-context-menu")
        .expect("context menu root should be registered");
    let branch = snapshot
        .layers()
        .iter()
        .find(|layer| layer.id().as_str() == "context-menu:layered-context-menu:branch:0:organize")
        .expect("open context submenu should own a registered child layer");
    let nested_branch = snapshot
        .layers()
        .iter()
        .find(|layer| {
            layer.id().as_str() == "context-menu:layered-context-menu:branch:0:organize/0:more"
        })
        .expect("nested context submenu should own a registered grandchild layer");
    assert_eq!(
        branch.parent().map(|parent| parent.as_str()),
        Some(root.id().as_str())
    );
    assert_eq!(branch.phase(), OverlayLayerPhase::Open);
    assert_eq!(
        nested_branch.parent().map(|parent| parent.as_str()),
        Some(branch.id().as_str())
    );
    assert_eq!(nested_branch.phase(), OverlayLayerPhase::Open);
    assert!(
        cx.debug_selector_is_focused("context-menu:layered-context-menu:panel:0:organize/0:more"),
        "the deepest context-menu branch should own focus"
    );

    cx.simulate_keystrokes("escape");
    cx.run_until_parked();
    cx.update(|window, cx| window.draw(cx).clear());
    assert!(
        cx.debug_bounds("context-menu:layered-context-menu:surface")
            .is_some()
    );
    assert!(
        cx.debug_bounds("context-menu:layered-context-menu:item:0:organize/0:more/0:archive")
            .is_none(),
        "the first Escape should close only the deepest context-menu branch"
    );
    assert!(
        cx.debug_selector_is_focused("context-menu:layered-context-menu:item:0:organize/0:more")
    );
    assert_eq!(open_events.borrow().as_slice(), &[true]);

    cx.simulate_keystrokes("escape");
    cx.run_until_parked();
    cx.update(|window, cx| window.draw(cx).clear());
    assert!(
        cx.debug_bounds("context-menu:layered-context-menu:surface")
            .is_some()
    );
    assert!(
        cx.debug_bounds("context-menu:layered-context-menu:item:0:organize/0:more")
            .is_none(),
        "the second Escape should close the parent context-menu branch"
    );
    assert!(cx.debug_selector_is_focused("context-menu:layered-context-menu:item:0:organize"));
    assert_eq!(open_events.borrow().as_slice(), &[true]);

    cx.simulate_keystrokes("escape");
    cx.run_until_parked();
    cx.update(|window, cx| window.draw(cx).clear());
    assert!(
        cx.debug_bounds("context-menu:layered-context-menu:surface")
            .is_none()
    );
    assert!(cx.debug_selector_is_focused("context-menu:layered-context-menu:hotspot"));
    assert_eq!(open_events.borrow().as_slice(), &[true, false]);
}

#[open_gpui::test]
fn context_menu_removed_submenu_does_not_reopen_when_the_same_path_returns(
    cx: &mut open_gpui::TestAppContext,
) {
    struct TestView {
        show_branch: bool,
    }

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let branch = self.show_branch.then(|| {
                MenuItem::submenu("branch", "Branch", [MenuItem::action("child", "Child")])
            });
            ContextMenu::new("dynamic-context-menu", "Dynamic context menu")
                .default_open(true)
                .default_focused_value("branch")
                .items(branch)
                .item(MenuItem::action("fallback", "Fallback"))
        }
    }

    let (view, cx) = cx.add_window_view(|_, _| TestView { show_branch: true });
    cx.update(|window, cx| window.draw(cx).clear());

    cx.simulate_keystrokes("right");
    cx.update(|window, cx| window.draw(cx).clear());
    assert!(
        cx.debug_bounds("context-menu:dynamic-context-menu:item:0:branch/0:child")
            .is_some(),
        "the initial branch should open through real keyboard navigation"
    );

    cx.update_window_entity(&view, |view, _, cx| {
        view.show_branch = false;
        cx.notify();
    });
    cx.update(|window, cx| window.draw(cx).clear());
    cx.run_until_parked();
    cx.update(|window, cx| window.draw(cx).clear());
    assert!(
        cx.debug_bounds("context-menu:dynamic-context-menu:item:0:branch/0:child")
            .is_none(),
        "removing the branch should close and clean up its registered subtree"
    );

    cx.update_window_entity(&view, |view, _, cx| {
        view.show_branch = true;
        cx.notify();
    });
    cx.update(|window, cx| window.draw(cx).clear());
    assert!(
        cx.debug_bounds("context-menu:dynamic-context-menu:item:0:branch/0:child")
            .is_none(),
        "a restored descriptor path must not resurrect stale open-path authority"
    );

    cx.simulate_keystrokes("right");
    cx.update(|window, cx| window.draw(cx).clear());
    assert!(
        cx.debug_bounds("context-menu:dynamic-context-menu:item:0:branch/0:child")
            .is_some(),
        "the restored branch should remain explicitly openable"
    );
}

#[open_gpui::test]
fn context_menu_disabling_an_open_submenu_never_restores_focus_to_its_trigger(
    cx: &mut open_gpui::TestAppContext,
) {
    struct TestView {
        branch_disabled: bool,
    }

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            ContextMenu::new(
                "disabled-branch-context-menu",
                "Disabled branch context menu",
            )
            .default_open(true)
            .default_focused_value("branch")
            .item(
                MenuItem::submenu("branch", "Branch", [MenuItem::action("child", "Child")])
                    .disabled(self.branch_disabled),
            )
            .item(MenuItem::action("fallback", "Fallback"))
        }
    }

    let (view, cx) = cx.add_window_view(|_, _| TestView {
        branch_disabled: false,
    });
    cx.update(|window, cx| window.draw(cx).clear());

    cx.simulate_keystrokes("right");
    cx.update(|window, cx| window.draw(cx).clear());
    assert!(
        cx.debug_bounds("context-menu:disabled-branch-context-menu:item:0:branch/0:child")
            .is_some(),
        "the enabled branch should start open"
    );

    cx.update_window_entity(&view, |view, _, cx| {
        view.branch_disabled = true;
        cx.notify();
    });
    cx.update(|window, cx| window.draw(cx).clear());
    cx.run_until_parked();
    cx.update(|window, cx| window.draw(cx).clear());

    assert!(
        cx.debug_bounds("context-menu:disabled-branch-context-menu:item:0:branch/0:child")
            .is_none(),
        "disabling the branch should close its child surface"
    );
    assert!(
        !cx.debug_selector_is_focused("context-menu:disabled-branch-context-menu:item:0:branch"),
        "focus restoration must never target a disabled submenu trigger"
    );
    assert!(
        cx.debug_selector_is_focused("context-menu:disabled-branch-context-menu:surface"),
        "the root menu surface should remain the available focus fallback"
    );
}

#[open_gpui::test]
fn context_menu_right_click_reanchors_one_root_and_left_click_dismisses(
    cx: &mut open_gpui::TestAppContext,
) {
    struct TestView {
        open_events: Rc<RefCell<Vec<bool>>>,
    }

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let open_events = self.open_events.clone();
            div().size_full().child(
                ContextMenu::new("reanchor-context-menu", "Right click area")
                    .item(MenuItem::action("open", "Open"))
                    .on_open_change(move |intent, _, _| {
                        open_events.borrow_mut().push(intent.desired_open());
                    }),
            )
        }
    }

    let open_events = Rc::new(RefCell::new(Vec::new()));
    let (_, cx) = cx.add_window_view(|_, _| TestView {
        open_events: open_events.clone(),
    });
    cx.update(|window, cx| window.draw(cx).clear());

    let source = cx
        .debug_bounds("context-menu:reanchor-context-menu:root")
        .expect("context menu source should render");
    let hotspot = cx
        .debug_bounds("context-menu:reanchor-context-menu:hotspot")
        .expect("context menu hotspot should render");
    let first_anchor = point(source.right() - px(4.0), source.bottom() - px(4.0));
    let second_anchor = point(source.left() + px(4.0), source.top() + px(4.0));
    assert!(!hotspot.contains(&first_anchor));
    assert!(!hotspot.contains(&second_anchor));

    cx.simulate_mouse_down(first_anchor, MouseButton::Right, Default::default());
    cx.simulate_mouse_up(first_anchor, MouseButton::Right, Default::default());
    cx.run_until_parked();
    cx.update(|window, cx| window.draw(cx).clear());
    let first_surface = cx
        .debug_bounds("context-menu:reanchor-context-menu:surface")
        .expect("first right click should open the context menu");
    let first_snapshot = cx.update(|window, cx| {
        WindowOverlayRuntime::for_window(window, cx)
            .snapshot(window, cx)
            .expect("first context menu snapshot should resolve")
    });
    let first_root = first_snapshot
        .layers()
        .iter()
        .find(|layer| layer.id().as_str() == "context-menu:reanchor-context-menu")
        .expect("context menu root should be registered");
    assert_eq!(first_root.kind(), OverlayLayerKind::Menu);
    assert_eq!(first_root.phase(), OverlayLayerPhase::Open);
    let first_generation = first_root.generation();
    assert_eq!(open_events.borrow().as_slice(), &[true]);

    cx.simulate_mouse_down(second_anchor, MouseButton::Right, Default::default());
    cx.simulate_mouse_up(second_anchor, MouseButton::Right, Default::default());
    cx.run_until_parked();
    cx.update(|window, cx| window.draw(cx).clear());
    let second_surface = cx
        .debug_bounds("context-menu:reanchor-context-menu:surface")
        .expect("second right click should keep the context menu open");
    assert_ne!(
        second_surface.origin, first_surface.origin,
        "an open ContextMenu should redraw at the newer right-click anchor"
    );
    let second_snapshot = cx.update(|window, cx| {
        WindowOverlayRuntime::for_window(window, cx)
            .snapshot(window, cx)
            .expect("reanchored context menu snapshot should resolve")
    });
    let roots = second_snapshot
        .layers()
        .iter()
        .filter(|layer| layer.id().as_str() == "context-menu:reanchor-context-menu")
        .collect::<Vec<_>>();
    assert_eq!(roots.len(), 1);
    assert_eq!(roots[0].generation(), first_generation);
    assert_eq!(open_events.borrow().as_slice(), &[true]);

    cx.simulate_click(second_anchor, Default::default());
    cx.run_until_parked();
    cx.update(|window, cx| window.draw(cx).clear());
    assert!(
        cx.debug_bounds("context-menu:reanchor-context-menu:surface")
            .is_none(),
        "a left click in the right-click source should remain an outside dismissal"
    );
    assert_eq!(open_events.borrow().as_slice(), &[true, false]);
    assert!(cx.debug_selector_is_focused("context-menu:reanchor-context-menu:hotspot"));
}

#[open_gpui::test]
fn controlled_context_menu_escape_refusal_keeps_runtime_authority(
    cx: &mut open_gpui::TestAppContext,
) {
    struct TestView {
        open: bool,
        open_intents: Rc<RefCell<Vec<OverlayOpenIntent>>>,
    }

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let open_intents = self.open_intents.clone();
            div().size_full().child(
                ContextMenu::new("controlled-context-menu", "Controlled context menu")
                    .open(self.open)
                    .anchor_point(point(px(280.0), px(160.0)))
                    .item(MenuItem::action("open", "Open"))
                    .on_open_change(move |intent, _, _| {
                        open_intents.borrow_mut().push(intent);
                    }),
            )
        }
    }

    let open_intents = Rc::new(RefCell::new(Vec::new()));
    let (view, cx) = cx.add_window_view(|_, _| TestView {
        open: true,
        open_intents: open_intents.clone(),
    });
    cx.update(|window, cx| window.draw(cx).clear());
    cx.run_until_parked();
    cx.update(|window, cx| window.draw(cx).clear());

    cx.simulate_keystrokes("escape escape");
    let refused = cx.update(|window, cx| {
        WindowOverlayRuntime::for_window(window, cx)
            .snapshot(window, cx)
            .expect("controlled ContextMenu snapshot should resolve")
    });
    let refused = refused
        .layers()
        .iter()
        .find(|layer| layer.id().as_str() == "context-menu:controlled-context-menu")
        .expect("controlled ContextMenu root should remain registered");
    assert_eq!(open_intents.borrow().len(), 1);
    assert!(!open_intents.borrow()[0].desired_open());
    assert_eq!(open_intents.borrow()[0].reason(), DismissReason::EscapeKey);
    assert!(open_intents.borrow()[0].revision().is_some());
    assert_eq!(refused.phase(), OverlayLayerPhase::CloseRequested);
    assert_eq!(refused.pending_intent(), Some(DismissReason::EscapeKey));
    assert!(refused.keyboard_eligible());
    assert!(refused.focus_active());
    assert!(
        cx.debug_bounds("context-menu:controlled-context-menu:surface")
            .is_some()
    );
    assert!(cx.debug_selector_is_focused("context-menu:controlled-context-menu:surface"));

    let source = cx
        .debug_bounds("context-menu:controlled-context-menu:root")
        .expect("controlled ContextMenu source should render");
    let hotspot = cx
        .debug_bounds("context-menu:controlled-context-menu:hotspot")
        .expect("controlled ContextMenu hotspot should render");
    let reanchor = point(source.right() - px(4.0), source.bottom() - px(4.0));
    assert!(!hotspot.contains(&reanchor));
    let surface_before = cx
        .debug_bounds("context-menu:controlled-context-menu:surface")
        .expect("controlled ContextMenu surface should remain rendered");
    cx.simulate_mouse_down(reanchor, MouseButton::Right, Default::default());
    cx.simulate_mouse_up(reanchor, MouseButton::Right, Default::default());
    cx.run_until_parked();
    cx.update(|window, cx| window.draw(cx).clear());
    let surface_after = cx
        .debug_bounds("context-menu:controlled-context-menu:surface")
        .expect("controlled ContextMenu should remain open while reanchoring");
    assert_ne!(surface_after.origin, surface_before.origin);
    let reanchored = cx.update(|window, cx| {
        WindowOverlayRuntime::for_window(window, cx)
            .snapshot(window, cx)
            .expect("reanchored controlled ContextMenu snapshot should resolve")
    });
    let reanchored = reanchored
        .layers()
        .iter()
        .find(|layer| layer.id().as_str() == "context-menu:controlled-context-menu")
        .expect("reanchored controlled ContextMenu should remain registered");
    assert_eq!(reanchored.phase(), OverlayLayerPhase::CloseRequested);
    assert_eq!(reanchored.pending_intent(), Some(DismissReason::EscapeKey));
    assert_eq!(
        open_intents.borrow().len(),
        1,
        "reanchoring an already-open controlled menu is not owner rejection or a new open intent"
    );

    cx.update_window_entity(&view, |view, _, cx| {
        view.open = false;
        cx.notify();
    });
    cx.update(|window, cx| window.draw(cx).clear());
    cx.run_until_parked();
    let committed = cx.update(|window, cx| {
        WindowOverlayRuntime::for_window(window, cx)
            .snapshot(window, cx)
            .expect("committed ContextMenu snapshot should resolve")
    });
    let committed = committed
        .layers()
        .iter()
        .find(|layer| layer.id().as_str() == "context-menu:controlled-context-menu")
        .expect("hidden ContextMenu registration should remain reusable");
    assert_eq!(committed.phase(), OverlayLayerPhase::Hidden);
    assert!(
        cx.debug_bounds("context-menu:controlled-context-menu:surface")
            .is_none()
    );
    assert!(cx.debug_selector_is_focused("context-menu:controlled-context-menu:hotspot"));
}

#[open_gpui::test]
fn controlled_context_menu_rejected_open_does_not_seed_uncontrolled_ownership(
    cx: &mut open_gpui::TestAppContext,
) {
    struct TestView {
        controlled: bool,
        open: bool,
        open_intents: Rc<RefCell<Vec<OverlayOpenIntent>>>,
    }

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            let open_intents = self.open_intents.clone();
            let view = cx.entity().downgrade();
            let menu = ContextMenu::new("rejected-open-context-menu", "Controlled source")
                .anchor_point(point(px(280.0), px(160.0)))
                .item(MenuItem::action("open", "Open"))
                .on_open_change(move |intent, _, cx| {
                    let desired_open = intent.desired_open();
                    open_intents.borrow_mut().push(intent);
                    if desired_open {
                        view.update(cx, |view, cx| {
                            view.controlled = false;
                            cx.notify();
                        })
                        .ok();
                    }
                });
            let menu = if self.controlled {
                menu.open(self.open)
            } else {
                menu
            };
            div().size_full().child(menu)
        }
    }

    let open_intents = Rc::new(RefCell::new(Vec::new()));
    let (_, cx) = cx.add_window_view(|_, _| TestView {
        controlled: true,
        open: false,
        open_intents: open_intents.clone(),
    });
    cx.update(|window, cx| window.draw(cx).clear());

    let source = cx
        .debug_bounds("context-menu:rejected-open-context-menu:root")
        .expect("controlled ContextMenu source should render");
    let hotspot = cx
        .debug_bounds("context-menu:rejected-open-context-menu:hotspot")
        .expect("controlled ContextMenu hotspot should render");
    let open_point = point(source.right() - px(4.0), source.bottom() - px(4.0));
    assert!(!hotspot.contains(&open_point));

    cx.simulate_mouse_down(open_point, MouseButton::Right, Default::default());
    cx.simulate_mouse_up(open_point, MouseButton::Right, Default::default());

    assert_eq!(open_intents.borrow().len(), 1);
    assert!(open_intents.borrow()[0].desired_open());
    assert_eq!(open_intents.borrow()[0].reason(), DismissReason::Trigger);
    assert!(open_intents.borrow()[0].revision().is_some());

    cx.run_until_parked();
    cx.update(|window, cx| window.draw(cx).clear());

    assert!(
        cx.debug_bounds("context-menu:rejected-open-context-menu:surface")
            .is_none(),
        "switching ownership must not adopt a controlled intent the owner rejected"
    );
    let switched = cx.update(|window, cx| {
        WindowOverlayRuntime::for_window(window, cx)
            .snapshot(window, cx)
            .expect("uncontrolled ContextMenu snapshot should resolve")
    });
    let switched = switched
        .layers()
        .iter()
        .find(|layer| layer.id().as_str() == "context-menu:rejected-open-context-menu")
        .expect("uncontrolled ContextMenu registration should remain reusable");
    assert_eq!(switched.phase(), OverlayLayerPhase::Hidden);
    assert_eq!(switched.pending_open(), None);
    assert_eq!(open_intents.borrow().len(), 1);
}

#[open_gpui::test]
fn context_menu_submenu_outside_press_preserves_root_consumption_boundary(
    cx: &mut open_gpui::TestAppContext,
) {
    struct TestView {
        underlay_clicks: Rc<Cell<usize>>,
    }

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let underlay_clicks = self.underlay_clicks.clone();
            div()
                .relative()
                .size_full()
                .child(
                    div()
                        .id("context-menu-outside-underlay")
                        .debug_selector(|| "context-menu-test:outside-underlay".to_owned())
                        .absolute()
                        .left(px(520.0))
                        .top(px(320.0))
                        .w(px(120.0))
                        .h(px(40.0))
                        .on_click(move |_, _, _| {
                            underlay_clicks.set(underlay_clicks.get() + 1);
                        })
                        .child("Underlay"),
                )
                .child(
                    ContextMenu::new("outside-context-menu", "Outside context menu")
                        .default_focused_value("branch")
                        .item(MenuItem::submenu(
                            "branch",
                            "Branch",
                            [MenuItem::action("child", "Child")],
                        )),
                )
        }
    }

    let underlay_clicks = Rc::new(Cell::new(0));
    let (_, cx) = cx.add_window_view(|_, _| TestView {
        underlay_clicks: underlay_clicks.clone(),
    });
    cx.update(|window, cx| window.draw(cx).clear());
    let source = cx
        .debug_bounds("context-menu:outside-context-menu:root")
        .expect("context menu source should render");
    let anchor = point(source.right() - px(4.0), source.bottom() - px(4.0));
    let underlay = cx
        .debug_bounds("context-menu-test:outside-underlay")
        .expect("outside underlay should render");

    cx.simulate_mouse_down(anchor, MouseButton::Right, Default::default());
    cx.simulate_mouse_up(anchor, MouseButton::Right, Default::default());
    cx.update(|window, cx| window.draw(cx).clear());
    cx.simulate_keystrokes("right");
    cx.update(|window, cx| window.draw(cx).clear());
    assert!(
        cx.debug_bounds("context-menu:outside-context-menu:item:0:branch/0:child")
            .is_some()
    );

    cx.simulate_click(underlay.center(), Default::default());
    cx.run_until_parked();
    cx.update(|window, cx| window.draw(cx).clear());
    assert_eq!(underlay_clicks.get(), 0);
    assert!(
        cx.debug_bounds("context-menu:outside-context-menu:surface")
            .is_some()
    );
    assert!(
        cx.debug_bounds("context-menu:outside-context-menu:item:0:branch/0:child")
            .is_none()
    );
    assert!(cx.debug_selector_is_focused("context-menu:outside-context-menu:item:0:branch"));

    cx.simulate_click(underlay.center(), Default::default());
    cx.run_until_parked();
    cx.update(|window, cx| window.draw(cx).clear());
    assert_eq!(underlay_clicks.get(), 0);
    assert!(
        cx.debug_bounds("context-menu:outside-context-menu:surface")
            .is_none()
    );
}

#[open_gpui::test]
fn context_menu_runtime_keyboard_navigation_preserves_focused_value_after_rerender(
    cx: &mut open_gpui::TestAppContext,
) {
    struct TestView {
        selections: Rc<RefCell<Vec<MenuSelection>>>,
        item_selections: Rc<RefCell<Vec<MenuSelection>>>,
        open_events: Rc<RefCell<Vec<bool>>>,
    }

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let selections = self.selections.clone();
            let item_selections = self.item_selections.clone();
            let open_events = self.open_events.clone();

            div().size_full().child(
                ContextMenu::new("runtime-context-menu", "Runtime context menu")
                    .anchor_point(point(px(280.0), px(160.0)))
                    .default_focused_value("copy")
                    .item(MenuItem::action("cut", "Cut"))
                    .item(MenuItem::action("copy", "Copy"))
                    .item(MenuItem::action("select-all", "Select all").on_select(
                        move |selection, _, _| {
                            item_selections.borrow_mut().push(selection);
                        },
                    ))
                    .on_select(move |selection, _, _| {
                        selections.borrow_mut().push(selection);
                    })
                    .on_open_change(move |intent, _, _| {
                        open_events.borrow_mut().push(intent.desired_open());
                    }),
            )
        }
    }

    let selections = Rc::new(RefCell::new(Vec::new()));
    let item_selections = Rc::new(RefCell::new(Vec::new()));
    let open_events = Rc::new(RefCell::new(Vec::new()));
    let (_, cx) = cx.add_window_view(|_, _| TestView {
        selections: selections.clone(),
        item_selections: item_selections.clone(),
        open_events: open_events.clone(),
    });
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    let hotspot = cx
        .debug_bounds("context-menu:runtime-context-menu:hotspot")
        .expect("runtime context menu hotspot should render");
    cx.simulate_mouse_down(hotspot.center(), MouseButton::Right, Default::default());
    cx.simulate_mouse_up(hotspot.center(), MouseButton::Right, Default::default());
    cx.run_until_parked();
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    assert!(
        cx.debug_bounds("context-menu:runtime-context-menu:surface")
            .is_some(),
        "runtime context menu surface should render when opened"
    );

    cx.simulate_keystrokes("down");
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });
    assert!(
        selections.borrow().is_empty(),
        "arrow navigation should move the runtime focus without selecting"
    );

    cx.simulate_keystrokes("enter");
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    let after_enter = selections.borrow().clone();
    let after_item_enter = item_selections.borrow().clone();
    assert_eq!(after_enter.len(), 1);
    assert_eq!(after_enter[0].index(), 2);
    assert_eq!(after_enter[0].value(), "select-all");
    assert_eq!(after_enter[0].label(), "Select all");
    assert_eq!(after_item_enter.len(), 1);
    assert_eq!(after_item_enter[0].path_key(), "2:select-all");
    assert_eq!(open_events.borrow().as_slice(), &[true, false]);
    assert!(
        cx.debug_bounds("context-menu:runtime-context-menu:surface")
            .is_none(),
        "selection should commit the uncontrolled close before callbacks return"
    );
    assert!(cx.debug_selector_is_focused("context-menu:runtime-context-menu:hotspot"));
    let snapshot = cx.update(|window, cx| {
        WindowOverlayRuntime::for_window(window, cx)
            .snapshot(window, cx)
            .expect("selected ContextMenu snapshot should resolve")
    });
    let root = snapshot
        .layers()
        .iter()
        .find(|layer| layer.id().as_str() == "context-menu:runtime-context-menu")
        .expect("selected ContextMenu root should remain reusable");
    assert_eq!(root.phase(), OverlayLayerPhase::Hidden);
}

#[open_gpui::test]
fn context_menu_mouse_selection_runs_handlers_before_close_observers_and_preserves_new_focus(
    cx: &mut open_gpui::TestAppContext,
) {
    struct TestView {
        trace: Rc<RefCell<Vec<&'static str>>>,
        destination_focus: FocusHandle,
    }

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let item_trace = self.trace.clone();
            let global_trace = self.trace.clone();
            let open_trace = self.trace.clone();
            let destination_focus = self.destination_focus.clone();

            div()
                .size_full()
                .child(
                    div()
                        .id("context-menu-selection-focus-destination")
                        .debug_selector(|| {
                            "context-menu-test:selection-focus-destination".to_owned()
                        })
                        .focusable()
                        .track_focus(&self.destination_focus)
                        .child("Focus destination"),
                )
                .child(
                    ContextMenu::new("selection-context-menu", "Selection context menu")
                        .item(MenuItem::action("open", "Open").on_select(move |_, _, _| {
                            item_trace.borrow_mut().push("item");
                        }))
                        .on_select(move |_, window, cx| {
                            global_trace.borrow_mut().push("global");
                            destination_focus.focus(window, cx);
                        })
                        .on_open_change(move |intent, _, _| {
                            open_trace.borrow_mut().push(if intent.desired_open() {
                                "open"
                            } else {
                                "close"
                            });
                        }),
                )
        }
    }

    let trace = Rc::new(RefCell::new(Vec::new()));
    let (_, cx) = cx.add_window_view(|_, cx| TestView {
        trace: trace.clone(),
        destination_focus: cx.focus_handle(),
    });
    cx.update(|window, cx| window.draw(cx).clear());

    let source = cx
        .debug_bounds("context-menu:selection-context-menu:root")
        .expect("context menu source should render");
    let anchor = point(source.right() - px(4.0), source.bottom() - px(4.0));
    cx.simulate_mouse_down(anchor, MouseButton::Right, Default::default());
    cx.simulate_mouse_up(anchor, MouseButton::Right, Default::default());
    cx.update(|window, cx| window.draw(cx).clear());

    let item = cx
        .debug_bounds("context-menu:selection-context-menu:item:0:open")
        .expect("context menu action should render");
    cx.simulate_click(item.center(), Default::default());
    cx.run_until_parked();
    cx.update(|window, cx| window.draw(cx).clear());

    assert_eq!(
        trace.borrow().as_slice(),
        &["open", "item", "global", "close"],
        "selection effects should observe committed selection before open-state observers"
    );
    assert!(
        cx.debug_selector_is_focused("context-menu-test:selection-focus-destination"),
        "a selection callback's newer focus claim should win over the older close restore"
    );
}

#[open_gpui::test]
fn context_menu_runtime_long_menu_scroll_stays_inside_surface(cx: &mut open_gpui::TestAppContext) {
    struct TestView;

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            div().size_full().child(
                ContextMenu::new("runtime-long-context-menu", "Runtime long context menu")
                    .default_open(true)
                    .anchor_point(point(px(280.0), px(160.0)))
                    .default_focused_value("item-00")
                    .items((0..12).map(|index| {
                        MenuItem::action(format!("item-{index:02}"), format!("Item {index:02}"))
                    })),
            )
        }
    }

    let (_, cx) = cx.add_window_view(|_, _| TestView);
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    let surface_before = cx
        .debug_bounds("context-menu:runtime-long-context-menu:surface")
        .expect("runtime long context menu surface should render");
    let viewport = cx
        .debug_bounds("scroll-area:context-menu:runtime-long-context-menu:surface-scroll")
        .expect("runtime long context menu scroll viewport should render");
    let item_before = cx
        .debug_bounds("context-menu:runtime-long-context-menu:item:0:item-00")
        .expect("runtime long context menu first item should render");

    cx.simulate_event(ScrollWheelEvent {
        position: viewport.center(),
        delta: ScrollDelta::Pixels(point(px(0.0), px(-120.0))),
        ..Default::default()
    });
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    let surface_after = cx
        .debug_bounds("context-menu:runtime-long-context-menu:surface")
        .expect("runtime long context menu surface should still render");
    let item_after = cx
        .debug_bounds("context-menu:runtime-long-context-menu:item:0:item-00")
        .expect("runtime long context menu first item should still render");

    assert_eq!(
        surface_after, surface_before,
        "expected wheel input on the long ContextMenu to keep the surface fixed; before={surface_before:?} after={surface_after:?}"
    );
    assert!(
        item_after.top() < item_before.top(),
        "expected wheel input on the long ContextMenu to move the inner scroll viewport; before={item_before:?} after={item_after:?}"
    );
}

#[open_gpui::test]
fn tooltip_and_hover_card_register_passive_window_layers_without_focus_authority(
    cx: &mut open_gpui::TestAppContext,
) {
    struct TestView;

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            div()
                .size_full()
                .child(Tooltip::new("passive-tooltip", "Tooltip body").open(true))
                .child(
                    HoverCard::new(
                        "passive-hover-card",
                        "Hover card trigger",
                        "Hover card body",
                    )
                    .open(true),
                )
        }
    }

    let (_, cx) = cx.add_window_view(|_, _| TestView);
    cx.update(|window, cx| window.draw(cx).clear());

    let snapshot = cx.update(|window, cx| {
        WindowOverlayRuntime::for_window(window, cx)
            .snapshot(window, cx)
            .expect("passive overlay snapshot should resolve")
    });
    let tooltip = snapshot
        .layers()
        .iter()
        .find(|layer| layer.id().as_str() == "tooltip:passive-tooltip")
        .expect("concrete Tooltip should register with the window runtime");
    assert_eq!(tooltip.kind(), OverlayLayerKind::Tooltip);
    assert_eq!(tooltip.phase(), OverlayLayerPhase::Open);
    assert!(!tooltip.focus_active());

    let hover_card = snapshot
        .layers()
        .iter()
        .find(|layer| layer.id().as_str() == "hover-card:passive-hover-card")
        .expect("HoverCard should register with the window runtime");
    assert_eq!(hover_card.kind(), OverlayLayerKind::NonModalDismissible);
    assert_eq!(hover_card.phase(), OverlayLayerPhase::Open);
    assert!(hover_card.keyboard_eligible());
    assert!(!hover_card.focus_active());
}

#[open_gpui::test]
fn presentation_hidden_and_disabled_tooltips_do_not_build_deferred_surfaces(
    cx: &mut open_gpui::TestAppContext,
) {
    struct TestView;

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            div()
                .size_full()
                .child(Tooltip::new("hidden-tooltip", "Hidden tooltip"))
                .child(
                    Tooltip::new("disabled-tooltip", "Disabled tooltip")
                        .open(true)
                        .disabled(true),
                )
        }
    }

    let (_, cx) = cx.add_window_view(|_, _| TestView);
    cx.update(|window, cx| window.draw(cx).clear());

    let snapshot = cx.update(|window, cx| {
        WindowOverlayRuntime::for_window(window, cx)
            .snapshot(window, cx)
            .expect("hidden tooltip snapshot should resolve")
    });
    for id in ["tooltip:hidden-tooltip", "tooltip:disabled-tooltip"] {
        let layer = snapshot
            .layers()
            .iter()
            .find(|layer| layer.id().as_str() == id)
            .unwrap_or_else(|| panic!("{id} should remain registered"));
        assert_eq!(layer.phase(), OverlayLayerPhase::Hidden);
    }
    assert!(cx.debug_bounds("tooltip:hidden-tooltip:content").is_none());
    assert!(
        cx.debug_bounds("tooltip:disabled-tooltip:content")
            .is_none()
    );
}

#[open_gpui::test]
fn hover_card_is_transparent_to_outside_press_arbitration(cx: &mut open_gpui::TestAppContext) {
    struct TestView {
        popover_events: Rc<RefCell<Vec<bool>>>,
        hover_card_events: Rc<RefCell<Vec<bool>>>,
        underlay_clicks: Rc<Cell<usize>>,
    }

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let popover_events = self.popover_events.clone();
            let hover_card_events = self.hover_card_events.clone();
            let underlay_clicks = self.underlay_clicks.clone();

            div()
                .relative()
                .size_full()
                .child(
                    div()
                        .id("passive-overlay-underlay")
                        .debug_selector(|| "passive-overlay-test:underlay".to_owned())
                        .absolute()
                        .left(px(400.0))
                        .top(px(300.0))
                        .w(px(120.0))
                        .h(px(40.0))
                        .on_click(move |_, _, _| {
                            underlay_clicks.set(underlay_clicks.get() + 1);
                        })
                        .child("Underlay"),
                )
                .child(
                    Popover::new("passive-underlay-popover", "Popover", "Popover body")
                        .default_open(true)
                        .on_open_change(move |intent, _, _| {
                            popover_events.borrow_mut().push(intent.desired_open());
                        }),
                )
                .child(
                    HoverCard::new("transparent-hover-card", "Hover card", "Hover card body")
                        .open(true)
                        .on_open_change(move |intent, _, _| {
                            hover_card_events.borrow_mut().push(intent.desired_open());
                        }),
                )
        }
    }

    let popover_events = Rc::new(RefCell::new(Vec::new()));
    let hover_card_events = Rc::new(RefCell::new(Vec::new()));
    let underlay_clicks = Rc::new(Cell::new(0));
    let (_, cx) = cx.add_window_view(|_, _| TestView {
        popover_events: popover_events.clone(),
        hover_card_events: hover_card_events.clone(),
        underlay_clicks: underlay_clicks.clone(),
    });
    cx.update(|window, cx| window.draw(cx).clear());

    let underlay = cx
        .debug_bounds("passive-overlay-test:underlay")
        .expect("outside-press underlay should render");
    cx.simulate_click(underlay.center(), Default::default());
    cx.run_until_parked();
    cx.update(|window, cx| window.draw(cx).clear());

    assert_eq!(popover_events.borrow().as_slice(), &[false]);
    assert!(
        hover_card_events.borrow().is_empty(),
        "outside press must pass through HoverCard without becoming its ownership event"
    );
    assert_eq!(underlay_clicks.get(), 1);
    assert!(
        cx.debug_bounds("popover:passive-underlay-popover:content")
            .is_none()
    );
    assert!(
        cx.debug_bounds("hover-card:transparent-hover-card:content")
            .is_some()
    );
}

#[open_gpui::test]
fn native_text_tooltip_registers_its_visible_window_layer(cx: &mut open_gpui::TestAppContext) {
    struct TestView;

    impl Render for TestView {
        fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            let theme = ThemeResolver::current(window, cx);
            div().size_full().child(
                div()
                    .id("native-text-tooltip-trigger")
                    .debug_selector(|| "native-tooltip-test:text-trigger".to_owned())
                    .w(px(80.0))
                    .h(px(32.0))
                    .tooltip(Tooltip::scoped(theme, Tooltip::text("Native text tooltip")))
                    .tooltip_show_delay(Duration::ZERO),
            )
        }
    }

    let (_, cx) = cx.add_window_view(|_, _| TestView);
    cx.update(|window, cx| window.draw(cx).clear());
    let trigger = cx
        .debug_bounds("native-tooltip-test:text-trigger")
        .expect("native tooltip trigger should render");

    cx.simulate_mouse_move(trigger.center(), None, Default::default());
    cx.run_until_parked();
    cx.update(|window, cx| window.draw(cx).clear());

    let snapshot = cx.update(|window, cx| {
        WindowOverlayRuntime::for_window(window, cx)
            .snapshot(window, cx)
            .expect("native text tooltip snapshot should resolve")
    });
    let tooltip = snapshot
        .layers()
        .iter()
        .find(|layer| layer.id().as_str() == "tooltip:tooltip")
        .expect("Tooltip::text should register its ActiveTooltip surface");
    assert_eq!(tooltip.phase(), OverlayLayerPhase::Open);
    assert!(!tooltip.focus_active());
    assert!(cx.debug_bounds("tooltip:tooltip:content").is_some());
}

#[open_gpui::test]
fn native_action_tooltip_registers_its_visible_window_layer(cx: &mut open_gpui::TestAppContext) {
    struct TestView;

    impl Render for TestView {
        fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            let theme = ThemeResolver::current(window, cx);
            div().size_full().child(
                div()
                    .id("native-action-tooltip-trigger")
                    .debug_selector(|| "native-tooltip-test:action-trigger".to_owned())
                    .w(px(80.0))
                    .h(px(32.0))
                    .tooltip(Tooltip::scoped(
                        theme,
                        Tooltip::for_action("Native action tooltip", TooltipRuntimeAction),
                    ))
                    .tooltip_show_delay(Duration::ZERO),
            )
        }
    }

    let (_, cx) = cx.add_window_view(|_, _| TestView);
    cx.update(|window, cx| window.draw(cx).clear());
    let trigger = cx
        .debug_bounds("native-tooltip-test:action-trigger")
        .expect("native action tooltip trigger should render");

    cx.simulate_mouse_move(trigger.center(), None, Default::default());
    cx.run_until_parked();
    cx.update(|window, cx| window.draw(cx).clear());

    let snapshot = cx.update(|window, cx| {
        WindowOverlayRuntime::for_window(window, cx)
            .snapshot(window, cx)
            .expect("native action tooltip snapshot should resolve")
    });
    let tooltip = snapshot
        .layers()
        .iter()
        .find(|layer| layer.id().as_str() == "tooltip:tooltip")
        .expect("Tooltip::for_action should register its ActiveTooltip surface");
    assert_eq!(tooltip.phase(), OverlayLayerPhase::Open);
    assert!(!tooltip.focus_active());
    assert!(cx.debug_bounds("tooltip:tooltip:content").is_some());
}
