use open_gpui::{
    div, point, px, Anchor, Context, IntoElement, MouseButton, ParentElement, Render, ScrollDelta,
    ScrollWheelEvent, Styled, Window,
};
use open_gpui_ui_components::{
    gpui_adapter::{
        default_deferred_priority, escape_open_change, gpui_anchor, outside_press_open_change,
        point_anchor_placement, GpuiOverlayAdapterConfig, GpuiOverlayPlacement,
        DEFAULT_OVERLAY_SAFE_MARGIN,
    },
    menu_navigation_target, AlertDialog, AlertDialogActionKind, AlertDialogIntent,
    AlertDialogOpenMode, ButtonVariant, ColorState, ContextMenu, Dialog, DialogOpenMode, HoverCard,
    HoverCardContentKind, HoverCardDelayPolicy, HoverCardOpenIntent, HoverCardOpenMode, Menu,
    MenuItem, MenuItemKind, MenuOpenMode, MenuSelection, MenuSubmenuSurface, Popover,
    PopoverOpenMode, Sheet, SheetCloseAffordance, SheetModalMode, SheetOpenMode, SheetSide,
    Tooltip, TooltipContentKind, TooltipDelayPolicy, TooltipOpenIntent,
};
use open_gpui_ui_core::{
    rect, semantic, ui_point, ui_px, ui_size, DismissReason, EscapeKeyPolicy, FocusRestoreIntent,
    InitialFocusIntent, OutsidePressPolicy, OverlayAnchorInput, OverlayLayerKind,
    OverlayLayerPolicy, OverlayPlacementAlignment, OverlayPlacementInput, OverlayPlacementSide,
    OverlayPresence, Role, Sizable, Size, Toggled,
};
use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

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
        open_gpui_ui_core::OverlayFocusTarget::new("fallback"),
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

    assert_eq!(placement.anchor(), Anchor::TopRight);
    assert_eq!(placement.snap_margin(), DEFAULT_OVERLAY_SAFE_MARGIN);
    assert!(placement.position().is_some());
    assert_eq!(placement.safe_bounds(), input.safe_bounds());
}

#[test]
fn overlay_open_change_helpers_match_core_policies() {
    let dismissible = OverlayLayerPolicy::new(
        OverlayLayerKind::NonModalDismissible,
        OverlayPresence::open(),
    );
    let tooltip = OverlayLayerPolicy::new(OverlayLayerKind::Tooltip, OverlayPresence::open());

    let escape =
        escape_open_change(&dismissible).expect("dismissible overlay should close on escape");
    assert_eq!(escape.reason(), DismissReason::EscapeKey);
    assert!(escape.consumes_event());
    assert!(!escape.allows_underlay_dispatch());

    let outside = outside_press_open_change(&dismissible)
        .expect("dismissible overlay should close on outside press");
    assert_eq!(outside.reason(), DismissReason::OutsidePress);
    assert!(outside.allows_underlay_dispatch());

    assert_eq!(escape_open_change(&tooltip), None);
    assert_eq!(outside_press_open_change(&tooltip), None);
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
        InitialFocusIntent::TargetOrFirstFocusable(open_gpui_ui_core::OverlayFocusTarget::new("x"))
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
    assert_eq!(
        state.outside_press_policy(),
        OutsidePressPolicy::DismissAndPassThrough
    );
    assert_eq!(state.initial_focus_intent(), &InitialFocusIntent::None);
    assert_eq!(state.focus_restore_intent(), &FocusRestoreIntent::None);
    assert!(state.overlay().wants_outside_press_handler());
    assert!(state.overlay().layer_state().hit_testable());
    assert_eq!(state.colors().background().token(), semantic::SURFACE);
    assert_eq!(
        state.colors().trigger_background().state(),
        ColorState::Selected
    );
}

#[test]
fn hover_card_state_models_manual_disabled_and_policy_overrides() {
    let delay = HoverCardDelayPolicy::new(Duration::from_millis(80), Duration::from_millis(20));
    let state = HoverCard::element("rich-hover-card", "Details", div().child("Rich"))
        .default_open(true)
        .disabled(true)
        .open_intent(HoverCardOpenIntent::Manual)
        .delay(delay)
        .outside_press_policy(OutsidePressPolicy::DismissAndConsume)
        .initial_focus_intent(InitialFocusIntent::FirstFocusable)
        .focus_restore_intent(FocusRestoreIntent::Trigger)
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
    assert_eq!(
        state.outside_press_policy(),
        OutsidePressPolicy::DismissAndConsume
    );
    assert_eq!(
        state.initial_focus_intent(),
        &InitialFocusIntent::FirstFocusable
    );
    assert_eq!(state.focus_restore_intent(), &FocusRestoreIntent::Trigger);
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
    assert_eq!(state.outside_press_policy(), OutsidePressPolicy::Consume);
    assert_eq!(state.escape_key_policy(), EscapeKeyPolicy::Dismiss);
    assert_eq!(
        state.initial_focus_intent(),
        &InitialFocusIntent::FirstFocusable
    );
    assert_eq!(state.focus_restore_intent(), &FocusRestoreIntent::Trigger);
    assert_eq!(state.colors().barrier().token(), semantic::MODAL_OVERLAY);
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
                    .on_open_change(move |open, _, _| {
                        open_events.borrow_mut().push(open);
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
    assert!(state.cancel().default_focus());
    assert_eq!(state.action().kind(), AlertDialogActionKind::Action);
    assert_eq!(state.action().label(), "Delete");
    assert_eq!(state.action().variant(), ButtonVariant::Destructive);
    assert!(!state.action().default_focus());
    assert_eq!(
        state.colors().action_background().token(),
        semantic::DESTRUCTIVE
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
    assert_eq!(state.escape_key_policy(), EscapeKeyPolicy::Dismiss);
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
    assert!(surface
        .hover_corridor()
        .contains_point(ui_point(ui_px(210.0), ui_px(60.0))));
    assert!(!surface
        .hover_corridor()
        .contains_point(ui_point(ui_px(20.0), ui_px(20.0))));
    assert!(state
        .submenu_surface_for_trigger(
            &[String::from("2:close")],
            trigger_bounds,
            content_size,
            None
        )
        .is_none());

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

    let rename = cx
        .debug_bounds("menu:hover-switch-submenu:item:3:rename")
        .expect("next root item should render");
    cx.simulate_mouse_move(rename.center(), None, Default::default());
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });
    assert!(
        cx.debug_bounds("menu:hover-switch-submenu:item:2:group/0:kind")
            .is_some(),
        "hovering a plain root item should keep the open submenu visible until the close delay elapses"
    );
    cx.executor().advance_clock(Duration::from_millis(200));
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });
    assert!(
        cx.debug_bounds("menu:hover-switch-submenu:item:2:group/0:kind")
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
fn context_menu_runtime_keyboard_navigation_preserves_focused_value_after_rerender(
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
