use open_gpui::{
    div, point, px, Anchor, AppContext, Context, InteractiveElement, IntoElement, MouseButton,
    ParentElement, Render, ScrollDelta, ScrollWheelEvent, Styled, Window,
};
use open_gpui_ui_components::{
    active_index_from_str_keys, apply_tree_move, first_enabled,
    gpui_adapter::{
        default_deferred_priority, escape_open_change, focus_ring_shadow, gpui_anchor,
        init_text_input, outside_press_open_change, point_anchor_placement,
        GpuiOverlayAdapterConfig, GpuiOverlayPlacement, TextInputController,
        DEFAULT_OVERLAY_SAFE_MARGIN,
    },
    last_enabled, listbox_navigation_target, menu_navigation_target, next_enabled,
    sidebar_navigation_target, toolbar_navigation_target, virtualized_list_scroll_target,
    AlertDialog, AlertDialogActionKind, AlertDialogIntent, AlertDialogOpenMode, Avatar,
    AvatarGroup, AvatarGroupCount, Badge, BadgeVariant, Button, ButtonVariant, Checkbox,
    ColorIntent, ColorState, Combobox, ComboboxGroup, ComboboxOpenMode, ComboboxOption,
    ComboboxSelection, Command, CommandGroup, CommandGroupDescriptor, CommandIndexSnapshot,
    CommandIndexSnapshotMode, CommandItem, CommandItemDescriptor, CommandLoadingState,
    CommandMatchSource, CommandOpenMode, CommandQueryMode, CommandSelection,
    CommandSelectionChange, CommandSelectionMode, ContextMenu, Dialog, DialogOpenMode, EmptyState,
    FeedbackIntent, Field, FocusRing, HoverCard, HoverCardContentKind, HoverCardDelayPolicy,
    HoverCardOpenIntent, HoverCardOpenMode, IconButton, Kbd, Label, Listbox, ListboxGroup,
    ListboxGroupDescriptor, ListboxOption, ListboxOptionDescriptor, ListboxOptionKind,
    ListboxSelection, ListboxState, Menu, MenuItem, MenuItemKind, MenuOpenMode, MenuSelection,
    MenuSubmenuSurface, Popover, PopoverOpenMode, Progress, ProgressVisualMode, RadioGroup,
    RadioGroupState, RadioItem, RadioItemDescriptor, RadioSelection, ScrollArea, ScrollAreaAxis,
    ScrollAreaState, ScrollResetPolicy, Select, SelectOpenMode, SelectSelection, Separator, Sheet,
    SheetCloseAffordance, SheetModalMode, SheetOpenMode, SheetSide, Sidebar, SidebarCollapseMode,
    SidebarItem, SidebarItemDescriptor, SidebarSection, SidebarSectionDescriptor, SidebarSide,
    SidebarState, SidebarVariant, Skeleton, Splitter, SplitterPanel, SplitterPanelDescriptor,
    SplitterState, StatusCue, Switch, TableToolbar, Tabs, TabsActivationMode, TabsItem,
    TabsItemDescriptor, TabsSelection, TabsState, TextInput, TextInputDisplayMode, Textarea,
    ThemeColor, ThemeDefinition, ThemeMode, ThemeRegistry, ThemeResolver, ThemeSnapshot,
    ThemeValidationError, Toggle, ToggleGroup, ToggleGroupItem, ToggleVariant, Toolbar,
    ToolbarItem, ToolbarItemDescriptor, ToolbarItemKind, ToolbarSelection, ToolbarState, Tooltip,
    TooltipContentKind, TooltipDelayPolicy, TooltipOpenIntent, Tree, TreeChildrenLoadState,
    TreeDropPosition, TreeItemDescriptor, TreeMove, TreeMoveTarget, VirtualizedList,
    VirtualizedListActivation, VirtualizedListItemDescriptor, VirtualizedListScrollStrategy,
    VirtualizerRange, DEFAULT_FOCUS_RING_WIDTH,
};
use open_gpui_ui_core::{
    rect, semantic, ui_point, ui_px, ui_size, DismissReason, EscapeKeyPolicy, FocusRestoreIntent,
    InitialFocusIntent, Orientation, OutsidePressPolicy, OverlayAnchorInput, OverlayLayerKind,
    OverlayLayerPolicy, OverlayPlacementAlignment, OverlayPlacementInput, OverlayPlacementSide,
    OverlayPresence, Role, Sizable, Size, ThemeTokens, Toggled, TokenKey,
};
use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

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
    assert_eq!(state.metrics().height(), ui_px(36.0));
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
    assert_eq!(state.tab_stop_index(), state.focused_index());
    assert!(state.items()[1].selected());
    assert!(state.items()[1].focused());
    assert!(state.items()[2].disabled());
    assert!(!state.items()[2].focused());
}

#[test]
fn tabs_builder_state_falls_back_to_first_enabled_tab() {
    let state = Tabs::new("settings")
        .orientation(Orientation::Horizontal)
        .activation_mode(TabsActivationMode::Automatic)
        .with_size(Size::Large)
        .default_selected("history")
        .item(TabsItem::new("overview", "Overview", div()))
        .item(TabsItem::new("details", "Details", div()))
        .item(TabsItem::new("history", "History", div()).disabled(true))
        .state();

    assert_eq!(state.orientation(), Orientation::Horizontal);
    assert_eq!(state.activation_mode(), TabsActivationMode::Automatic);
    assert_eq!(state.size(), Size::Large);
    assert_eq!(state.selected_value(), Some("overview"));
    assert_eq!(state.focused_value(), Some("overview"));
    assert_eq!(state.tab_stop_index(), state.focused_index());
    assert_eq!(state.items().len(), 3);
    assert!(state.items()[2].disabled());
    assert!(!state.items()[2].selected());
}

#[test]
fn scroll_area_state_exposes_axis_metrics_and_reset_policy() {
    let state = ScrollAreaState::resolve(
        "activity-log",
        ScrollAreaAxis::Both,
        Size::Small,
        ScrollResetPolicy::ResetOnKeyChange,
        Some("components".to_string()),
    );

    assert_eq!(state.viewport_id(), "activity-log");
    assert_eq!(state.axis(), ScrollAreaAxis::Both);
    assert_eq!(state.axis().as_str(), "both");
    assert_eq!(state.size(), Size::Small);
    assert!(state.scrolls_x());
    assert!(state.scrolls_y());
    assert_eq!(state.reset_policy(), ScrollResetPolicy::ResetOnKeyChange);
    assert_eq!(state.reset_policy().as_str(), "reset-on-key-change");
    assert_eq!(state.reset_key(), Some("components"));
    assert_eq!(state.metrics().scrollbar_width(), ui_px(8.0));
    assert!(state.should_reset_for_key_change(Some("tokens")));
    assert!(!state.should_reset_for_key_change(Some("components")));
    assert!(!state.should_reset_for_key_change(None));
}

#[test]
fn scroll_area_builder_state_keeps_gpui_handle_out_of_resolved_state() {
    let external_handle = open_gpui::ScrollHandle::new();
    let state = ScrollArea::new("component-scroll", div())
        .horizontal()
        .large()
        .reset_on_key("settings")
        .state();
    let preserved = ScrollArea::new("preserved-scroll", div())
        .both()
        .scroll_handle(&external_handle)
        .preserve_scroll()
        .state();

    assert_eq!(state.viewport_id(), "component-scroll");
    assert_eq!(state.axis(), ScrollAreaAxis::Horizontal);
    assert!(state.scrolls_x());
    assert!(!state.scrolls_y());
    assert_eq!(state.size(), Size::Large);
    assert_eq!(state.metrics().scrollbar_width(), ui_px(12.0));
    assert_eq!(state.reset_key(), Some("settings"));
    assert!(state.should_reset_for_key_change(Some("overview")));
    assert_eq!(preserved.reset_policy(), ScrollResetPolicy::Preserve);
    assert_eq!(preserved.reset_key(), None);
    assert!(!preserved.should_reset_for_key_change(Some("overview")));
}

#[test]
fn virtualized_list_behavior_snapshot_uses_item_descriptors_and_virtualizer_contracts() {
    let items = (0..10_000)
        .map(|index| {
            VirtualizedListItemDescriptor::new(
                format!("item-{index:04}"),
                format!("Item {index:04}"),
            )
        })
        .collect::<Vec<_>>();
    let snapshot = VirtualizedList::new("contracts-list", "Contracts list", items)
        .with_size(Size::Small)
        .default_active_index(104)
        .default_selected_index(101)
        .viewport_item_count(7)
        .behavior_snapshot_with_viewport(ui_px(2_800.0), ui_px(196.0));

    assert_eq!(snapshot.role(), Role::ListBox);
    assert_eq!(snapshot.row_role(), Role::ListBoxOption);
    assert_eq!(snapshot.state().item_count(), 10_000);
    assert_eq!(snapshot.total_size(), ui_px(280_000.0));
    assert_eq!(*snapshot.visible_range(), VirtualizerRange::new(100, 107));
    assert_eq!(*snapshot.overscan_range(), VirtualizerRange::new(98, 109));
    assert_eq!(snapshot.visible_row_count(), 7);
    assert_eq!(snapshot.rendered_row_count(), 11);
    assert_eq!(snapshot.rows()[0].index(), 98);
    assert_eq!(snapshot.rows()[0].render_key(), "item-0098");

    let active_row = snapshot
        .active_row()
        .expect("active row should be rendered");
    assert_eq!(active_row.index(), 104);
    assert_eq!(active_row.key(), "item-0104");
    assert_eq!(active_row.label(), "Item 0104");
    assert!(active_row.active());
    assert!(!active_row.selected());
    assert_eq!(active_row.role(), Role::ListBoxOption);
    assert_eq!(active_row.position_in_set(), 105);
    assert_eq!(active_row.size_of_set(), 10_000);
    assert_eq!(active_row.virtual_start(), ui_px(2_912.0));
    assert_eq!(active_row.virtual_size(), ui_px(28.0));

    let selected_row = snapshot
        .selected_row()
        .expect("selected row should be rendered");
    assert_eq!(selected_row.index(), 101);
    assert!(selected_row.selected());

    let activation = VirtualizedListActivation::new(active_row.index());
    assert_eq!(activation.index(), 104);
    assert_eq!(
        virtualized_list_scroll_target(
            VirtualizedListScrollStrategy::Top,
            activation.index(),
            snapshot.state().item_count(),
            snapshot.metrics().row_height(),
            snapshot.viewport_extent(),
            snapshot.scroll_offset(),
        ),
        ui_px(2_912.0)
    );
}

#[test]
fn virtualized_list_behavior_snapshot_applies_builder_metrics() {
    let items = (0..32)
        .map(|index| {
            VirtualizedListItemDescriptor::new(
                format!("item-{index:04}"),
                format!("Item {index:04}"),
            )
        })
        .collect::<Vec<_>>();
    let snapshot = VirtualizedList::new("builder-list", "Builder list", items)
        .with_size(Size::Small)
        .row_height(ui_px(24.0))
        .overscan(2)
        .default_active_index(5)
        .default_selected_index(3)
        .viewport_item_count(4)
        .behavior_snapshot_with_viewport(ui_px(48.0), ui_px(96.0));

    assert_eq!(snapshot.metrics().row_height(), ui_px(24.0));
    assert_eq!(snapshot.overscan_count(), 2);
    assert_eq!(snapshot.visible_row_count(), 4);
    assert_eq!(*snapshot.visible_range(), VirtualizerRange::new(2, 6));
    assert_eq!(*snapshot.overscan_range(), VirtualizerRange::new(1, 7));
    assert_eq!(snapshot.active_row().map(|row| row.index()), Some(5));
    assert_eq!(snapshot.selected_row().map(|row| row.index()), Some(3));
}

#[open_gpui::test]
fn tree_runtime_expands_reveals_and_selects_items(cx: &mut open_gpui::TestAppContext) {
    struct TestView {
        selections: Rc<RefCell<Vec<String>>>,
        toggles: Rc<RefCell<Vec<(String, bool)>>>,
    }

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let selections = self.selections.clone();
            let toggles = self.toggles.clone();
            let tree = Tree::new(
                "runtime-tree",
                "Runtime tree",
                vec![
                    TreeItemDescriptor::new("paper", "Paper")
                        .child(TreeItemDescriptor::new("intro", "Introduction"))
                        .child(
                            TreeItemDescriptor::new("figures", "Figures")
                                .child(TreeItemDescriptor::new("figure-1", "Figure 1")),
                        ),
                    TreeItemDescriptor::new("notes", "Notes"),
                ],
            )
            .with_size(Size::Small)
            .default_focused("paper")
            .on_select(move |selection, _, _| {
                selections.borrow_mut().push(selection.value().to_owned());
            })
            .on_toggle(move |toggle, _, _| {
                toggles
                    .borrow_mut()
                    .push((toggle.value().to_owned(), toggle.expanded()));
            });

            div()
                .size_full()
                .child(div().w(px(280.0)).h(px(180.0)).child(tree))
        }
    }

    let selections = Rc::new(RefCell::new(Vec::new()));
    let toggles = Rc::new(RefCell::new(Vec::new()));
    let (_, cx) = cx.add_window_view(|_, _| TestView {
        selections: selections.clone(),
        toggles: toggles.clone(),
    });
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    assert!(
        cx.debug_bounds("tree:runtime-tree:item:paper").is_some(),
        "expected the root tree item to render before expansion"
    );
    assert!(
        cx.debug_bounds("tree:runtime-tree:item:intro").is_none(),
        "expected collapsed descendants to stay hidden before expansion"
    );

    let root = cx
        .debug_bounds("tree:runtime-tree:root")
        .expect("tree root should render as a focusable interaction region");
    cx.simulate_click(
        point(root.left() + px(2.0), root.top() + px(2.0)),
        Default::default(),
    );
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });
    assert!(
        cx.debug_selector_is_focused("tree:runtime-tree:item:paper"),
        "clicking Tree chrome should focus the current roving item for keyboard navigation"
    );

    let paper = cx
        .debug_bounds("tree:runtime-tree:item:paper")
        .expect("paper row should be visible");
    cx.simulate_click(paper.center(), Default::default());
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });
    selections.borrow_mut().clear();

    cx.simulate_keystrokes("right");
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    assert_eq!(
        toggles.borrow().as_slice(),
        [("paper".to_owned(), true)],
        "expected right arrow to expand the focused root branch"
    );
    assert!(
        cx.debug_bounds("tree:runtime-tree:item:intro").is_some(),
        "expected expanded descendants to render after toggling open"
    );

    cx.simulate_keystrokes("down");
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });
    cx.simulate_keystrokes("enter");
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    assert_eq!(selections.borrow().as_slice(), ["intro".to_owned()]);
}

#[open_gpui::test]
fn tree_runtime_typeahead_focuses_visible_matching_row(cx: &mut open_gpui::TestAppContext) {
    struct TestView {
        selections: Rc<RefCell<Vec<String>>>,
    }

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let selections = self.selections.clone();
            let tree = Tree::new(
                "runtime-typeahead-tree",
                "Runtime typeahead tree",
                vec![
                    TreeItemDescriptor::new("paper", "Paper")
                        .child(TreeItemDescriptor::new("figures", "Figures")),
                    TreeItemDescriptor::new("disabled", "Disabled").disabled(true),
                    TreeItemDescriptor::new("notes", "Notes"),
                ],
            )
            .with_size(Size::Small)
            .default_focused("paper")
            .on_select(move |selection, _, _| {
                selections.borrow_mut().push(selection.value().to_owned());
            });

            div()
                .size_full()
                .child(div().w(px(280.0)).h(px(180.0)).child(tree))
        }
    }

    let selections = Rc::new(RefCell::new(Vec::new()));
    let (_, cx) = cx.add_window_view(|_, _| TestView {
        selections: selections.clone(),
    });
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    let root = cx
        .debug_bounds("tree:runtime-typeahead-tree:root")
        .expect("tree root should render");
    cx.simulate_click(
        point(root.left() + px(2.0), root.top() + px(2.0)),
        Default::default(),
    );
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    cx.simulate_keystrokes("n o");
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    assert!(
        cx.debug_selector_is_focused("tree:runtime-typeahead-tree:item:notes"),
        "expected typeahead to focus the visible Notes row; focused={:?}",
        cx.focused_debug_selector()
    );
    assert!(
        selections.borrow().is_empty(),
        "typeahead should move focus without selecting a row"
    );
}

#[open_gpui::test]
fn tree_runtime_drag_move_emits_controlled_payload(cx: &mut open_gpui::TestAppContext) {
    struct TestView {
        moves: Rc<RefCell<Vec<TreeMove>>>,
        selections: Rc<RefCell<Vec<String>>>,
    }

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let moves = self.moves.clone();
            let selections = self.selections.clone();
            let tree = Tree::new(
                "runtime-drag-tree",
                "Runtime drag tree",
                vec![
                    TreeItemDescriptor::new("root", "Root")
                        .expanded(true)
                        .child(TreeItemDescriptor::new("child", "Child"))
                        .child(TreeItemDescriptor::new("peer", "Peer")),
                    TreeItemDescriptor::new("sibling", "Sibling"),
                ],
            )
            .with_size(Size::Small)
            .default_focused("root")
            .draggable(true)
            .on_select(move |selection, _, _| {
                selections.borrow_mut().push(selection.value().to_owned());
            })
            .on_move(move |tree_move, _, _| {
                moves.borrow_mut().push(tree_move);
            });

            div()
                .size_full()
                .child(div().w(px(320.0)).h(px(220.0)).child(tree))
        }
    }

    let moves = Rc::new(RefCell::new(Vec::new()));
    let selections = Rc::new(RefCell::new(Vec::new()));
    let (_, cx) = cx.add_window_view(|_, _| TestView {
        moves: moves.clone(),
        selections: selections.clone(),
    });
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    let child = cx
        .debug_bounds("tree:runtime-drag-tree:item:child")
        .expect("expanded child row should render");
    cx.simulate_click(child.center(), Default::default());
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });
    assert_eq!(
        selections.borrow().as_slice(),
        ["child".to_owned()],
        "enabling tree drag affordances should not break regular row clicks"
    );
    assert!(
        moves.borrow().is_empty(),
        "regular clicks should not emit controlled tree moves"
    );
    selections.borrow_mut().clear();

    let source = cx
        .debug_bounds("tree:runtime-drag-tree:item:child")
        .expect("child row should remain rendered")
        .center();
    let target = cx
        .debug_bounds("tree:runtime-drag-tree:drop:before:sibling")
        .expect("before-sibling drop zone should render")
        .center();

    cx.simulate_mouse_down(source, MouseButton::Left, Default::default());
    cx.simulate_mouse_move(
        point(source.x + px(18.0), source.y),
        MouseButton::Left,
        Default::default(),
    );
    cx.simulate_mouse_move(target, MouseButton::Left, Default::default());
    cx.simulate_mouse_up(target, MouseButton::Left, Default::default());
    cx.run_until_parked();
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    let moves = moves.borrow();
    assert_eq!(
        moves.len(),
        1,
        "expected one controlled move after dropping child before sibling"
    );
    let tree_move = &moves[0];
    assert_eq!(tree_move.value(), "child");
    assert_eq!(tree_move.label(), "Child");
    assert_eq!(tree_move.source_parent_value(), Some("root"));
    assert_eq!(tree_move.position(), TreeDropPosition::Before);
    assert_eq!(tree_move.target().target_value(), "sibling");
    assert_eq!(tree_move.target_parent_value(), None);
    assert_eq!(tree_move.sibling_anchor_value(), Some("sibling"));
    assert!(
        selections.borrow().is_empty(),
        "drag drops should not also emit row selections"
    );
}

#[test]
fn feedback_tree_and_virtualized_list_public_exports_remain_explicit() {
    use open_gpui_ui_components::{self as root, prelude};

    let root_status_cue: root::StatusCue = root::StatusCue::new("status", "Ready");
    let prelude_status_cue: prelude::StatusCue = prelude::StatusCue::new("status", "Ready");
    let root_empty_state: root::EmptyState = root::EmptyState::new("empty", "No results");
    let prelude_empty_state: prelude::EmptyState = prelude::EmptyState::new("empty", "No results");
    let root_tree_descriptor: root::TreeItemDescriptor =
        root::TreeItemDescriptor::new("root", "Root")
            .with_children_unloaded()
            .child(root::TreeItemDescriptor::new("child", "Child"));
    let prelude_tree_descriptor: prelude::TreeItemDescriptor =
        prelude::TreeItemDescriptor::new("root", "Root").with_children_load_failed("Offline");
    let root_tree_load_state: root::TreeChildrenLoadState =
        root::TreeChildrenLoadState::loading("Loading children");
    let prelude_tree_load_state: prelude::TreeChildrenLoadState =
        prelude::TreeChildrenLoadState::unloaded();
    let direct_tree_load_state: TreeChildrenLoadState = TreeChildrenLoadState::loaded();
    let root_tree: root::Tree =
        root::Tree::new("root-tree", "Root tree", [root_tree_descriptor.clone()])
            .default_selected("root")
            .default_focused("root")
            .virtualized(true)
            .viewport_item_count(2)
            .overscan_count(1);
    let prelude_tree: prelude::Tree = prelude::Tree::new(
        "prelude-tree",
        "Prelude tree",
        [prelude::TreeItemDescriptor::new("root", "Root")],
    )
    .default_focused("root")
    .virtualized(true)
    .viewport_item_count(2)
    .overscan_count(1);
    let root_tree_state: root::TreeState = root::TreeState::resolve(
        Size::Medium,
        "Tree",
        None,
        None,
        [root_tree_descriptor.clone()],
    );
    let prelude_tree_state: prelude::TreeState =
        prelude::TreeState::resolve(Size::Medium, "Tree", None, None, [prelude_tree_descriptor]);
    let move_items = [
        root::TreeItemDescriptor::new("root", "Root")
            .expanded(true)
            .child(root::TreeItemDescriptor::new("child", "Child")),
        root::TreeItemDescriptor::new("sibling", "Sibling"),
    ];
    let move_state: root::TreeState =
        root::TreeState::resolve(Size::Medium, "Move tree", None, None, move_items.clone());
    let root_tree_move: root::TreeMove = move_state
        .move_for_drop("child", "sibling", root::TreeDropPosition::Before)
        .expect("public Tree move payload should resolve");
    let _root_tree_move_target: &root::TreeMoveTarget = root_tree_move.target();
    let prelude_tree_position: prelude::TreeDropPosition = prelude::TreeDropPosition::Inside;
    let _direct_tree_move: TreeMove = root_tree_move.clone();
    let _direct_tree_move_target: &TreeMoveTarget = root_tree_move.target();
    let moved_tree = root::apply_tree_move(move_items, &root_tree_move)
        .expect("public apply_tree_move helper should apply valid payload");
    let _direct_moved_tree = apply_tree_move(moved_tree.clone(), &root_tree_move);
    let prelude_move_state: prelude::TreeState =
        prelude::TreeState::resolve(Size::Medium, "Move tree", None, None, moved_tree);
    let root_virtualized_state: root::VirtualizedListState =
        root::VirtualizedListState::resolve(Size::Small, false, 12, Some(4), Some(4), Some(3));
    let prelude_virtualized_state: prelude::VirtualizedListState =
        prelude::VirtualizedListState::resolve(Size::Small, false, 12, Some(4), Some(4), Some(3));
    let root_virtualized_items = (0..12)
        .map(|index| {
            root::VirtualizedListItemDescriptor::new(
                format!("root-item-{index}"),
                format!("Root item {index}"),
            )
        })
        .collect::<Vec<_>>();
    let root_virtualized_list: root::VirtualizedList = root::VirtualizedList::new(
        "root-virtualized-component",
        "Root virtualized component",
        root_virtualized_items.clone(),
    )
    .with_size(Size::Small)
    .default_active_index(4)
    .default_selected_index(4)
    .viewport_item_count(3);
    let prelude_virtualized_items = (0..12)
        .map(|index| {
            prelude::VirtualizedListItemDescriptor::new(
                format!("prelude-item-{index}"),
                format!("Prelude item {index}"),
            )
        })
        .collect::<Vec<_>>();
    let prelude_virtualized_list: prelude::VirtualizedList = prelude::VirtualizedList::new(
        "prelude-virtualized-component",
        "Prelude virtualized component",
        prelude_virtualized_items.clone(),
    )
    .with_size(Size::Small)
    .default_active_index(4)
    .default_selected_index(4)
    .viewport_item_count(3);
    let root_virtualized_snapshot: root::VirtualizedListBehaviorSnapshot =
        root_virtualized_list.behavior_snapshot_with_viewport(ui_px(28.0), ui_px(56.0));
    let prelude_virtualized_snapshot: prelude::VirtualizedListBehaviorSnapshot =
        prelude_virtualized_list.behavior_snapshot_with_viewport(ui_px(28.0), ui_px(56.0));
    let _root_virtualized_row: &root::VirtualizedListRowBehaviorSnapshot =
        root_virtualized_snapshot.active_row().unwrap();
    let _prelude_virtualized_row: &prelude::VirtualizedListRowBehaviorSnapshot =
        prelude_virtualized_snapshot.active_row().unwrap();
    let root_virtualized_component_state = root_virtualized_list.state();
    let prelude_virtualized_component_state = prelude_virtualized_list.state();
    let root_tree_component_state = root_tree.state();
    let root_tree_component_snapshot: root::TreeBehaviorSnapshot =
        root_tree.behavior_snapshot(ui_px(0.0), ui_px(32.0));
    let prelude_tree_component_state = prelude_tree.state();
    let prelude_tree_component_snapshot: prelude::TreeBehaviorSnapshot =
        prelude_tree.behavior_snapshot(ui_px(0.0), ui_px(32.0));
    let _root_tree_row: &root::TreeRowBehaviorSnapshot =
        root_tree_component_snapshot.rows().first().unwrap();
    let _prelude_tree_row: &prelude::TreeRowBehaviorSnapshot =
        prelude_tree_component_snapshot.rows().first().unwrap();
    let _root_tree_toggle: Option<root::TreeToggle> =
        root::TreeToggle::from_item(&root_tree_state.items()[0]);
    let _prelude_tree_toggle: Option<prelude::TreeToggle> =
        prelude::TreeToggle::from_item(&prelude_tree_state.items()[0]);
    let _root_tree_selection: Option<root::TreeSelection> =
        root::TreeSelection::from_item(&root_tree_state.items()[0]);
    let _prelude_tree_selection: Option<prelude::TreeSelection> =
        prelude::TreeSelection::from_item(&prelude_tree_state.items()[0]);
    let _root_tree_focus: root::TreeFocusTarget = root::TreeFocusTarget::new(0, "root");
    let _prelude_tree_focus: prelude::TreeFocusTarget = prelude::TreeFocusTarget::new(0, "root");
    let _root_tree_action: Option<root::TreeKeyboardAction> =
        root_tree_state.keyboard_action_for_key("right");
    let _prelude_tree_action: Option<prelude::TreeKeyboardAction> =
        prelude_tree_state.keyboard_action_for_key("right");
    let _root_virtualized_activation: root::VirtualizedListActivation =
        root::VirtualizedListActivation::new(4);
    let _prelude_virtualized_activation: prelude::VirtualizedListActivation =
        prelude::VirtualizedListActivation::new(4);
    let _root_scroll_strategy: root::VirtualizedListScrollStrategy =
        root::VirtualizedListScrollStrategy::Center;
    let _prelude_scroll_strategy: prelude::VirtualizedListScrollStrategy =
        prelude::VirtualizedListScrollStrategy::Center;

    assert_eq!(root_status_cue.state().role(), Role::Label);
    assert_eq!(prelude_status_cue.state().role(), Role::Label);
    assert_eq!(root_empty_state.state().role(), Role::Section);
    assert_eq!(prelude_empty_state.state().role(), Role::Section);
    assert_eq!(root_tree_component_state.role(), Role::Tree);
    assert_eq!(prelude_tree_component_state.item_role(), Role::TreeItem);
    assert_eq!(root_tree_component_state.focused_value(), Some("root"));
    assert_eq!(root_tree_component_snapshot.role(), Role::Tree);
    assert_eq!(prelude_tree_component_snapshot.row_role(), Role::TreeItem);
    assert_eq!(root_tree_state.items().len(), 1);
    assert_eq!(prelude_tree_state.items().len(), 1);
    assert_eq!(root_tree_state.role(), Role::Tree);
    assert_eq!(root_tree_state.items()[0].role(), Role::TreeItem);
    assert!(root_tree_state.items()[0].has_children());
    assert_eq!(
        root_tree_state.items()[0].children_load_state().as_str(),
        "unloaded"
    );
    assert!(prelude_tree_state.items()[0].children_load_failed());
    assert!(root_tree_load_state.is_loading());
    assert!(prelude_tree_load_state.is_unloaded());
    assert!(direct_tree_load_state.is_loaded());
    assert_eq!(root::tree_navigation_target("home", 0, &[false]), Some(0));
    assert_eq!(
        prelude::tree_navigation_target("home", 0, &[false]),
        Some(0)
    );
    assert_eq!(
        root_tree_component_snapshot.rows()[0].render_key(),
        "0:root"
    );
    assert_eq!(prelude_tree_component_snapshot.state().items().len(), 1);
    assert_eq!(root_tree_component_snapshot.rendered_row_count(), 1);
    assert_eq!(root_tree_move.position(), TreeDropPosition::Before);
    assert_eq!(root_tree_move.target_parent_value(), None);
    assert_eq!(root_tree_move.sibling_anchor_value(), Some("sibling"));
    assert_eq!(prelude_tree_position.as_str(), "inside");
    assert_eq!(prelude_move_state.items()[0].value(), "root");
    assert_eq!(prelude_move_state.items()[1].value(), "child");
    assert_eq!(
        root_virtualized_state.navigation_target("pagedown"),
        Some(7)
    );
    assert_eq!(
        prelude_virtualized_state.navigation_target("pagedown"),
        Some(7)
    );
    assert_eq!(root_virtualized_component_state.active_index(), Some(4));
    assert_eq!(
        prelude_virtualized_component_state.selected_index(),
        Some(4)
    );
    assert_eq!(root_virtualized_snapshot.role(), Role::ListBox);
    assert_eq!(prelude_virtualized_snapshot.row_role(), Role::ListBoxOption);
    assert_eq!(
        root::virtualized_list_scroll_target(
            root::VirtualizedListScrollStrategy::Top,
            4,
            root_virtualized_snapshot.state().item_count(),
            root_virtualized_snapshot.metrics().row_height(),
            root_virtualized_snapshot.viewport_extent(),
            root_virtualized_snapshot.scroll_offset(),
        ),
        ui_px(112.0)
    );
    assert_eq!(
        prelude::virtualized_list_scroll_target(
            prelude::VirtualizedListScrollStrategy::Top,
            4,
            prelude_virtualized_snapshot.state().item_count(),
            prelude_virtualized_snapshot.metrics().row_height(),
            prelude_virtualized_snapshot.viewport_extent(),
            prelude_virtualized_snapshot.scroll_offset(),
        ),
        ui_px(112.0)
    );
    assert_eq!(
        root::virtualized_list_navigation_target("end", 4, 12, 3),
        Some(11)
    );
    assert_eq!(
        prelude::virtualized_list_navigation_target("end", 4, 12, 3),
        Some(11)
    );
}

#[open_gpui::test]
fn virtualized_list_runtime_reveals_active_row_and_emits_activation(
    cx: &mut open_gpui::TestAppContext,
) {
    struct TestView {
        activations: Rc<RefCell<Vec<usize>>>,
    }

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let activations = self.activations.clone();
            let items = (0..100).map(|index| {
                VirtualizedListItemDescriptor::new(
                    format!("item-{index:04}"),
                    format!("Item {index:04}"),
                )
            });

            div().size_full().child(
                div().w(px(240.0)).h(px(112.0)).child(
                    VirtualizedList::new("runtime-list", "Runtime list", items)
                        .with_size(Size::Small)
                        .row_height(ui_px(28.0))
                        .viewport_item_count(4)
                        .overscan(2)
                        .on_activate(move |activation, _, _| {
                            activations.borrow_mut().push(activation.index());
                        }),
                ),
            )
        }
    }

    let activations = Rc::new(RefCell::new(Vec::new()));
    let (_, cx) = cx.add_window_view(|_, _| TestView {
        activations: activations.clone(),
    });
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    let root = cx
        .debug_bounds("virtualized-list:runtime-list:root")
        .expect("virtualized list root should render as a focusable target");
    cx.simulate_click(root.center(), Default::default());
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });
    assert!(
        cx.debug_selector_is_focused("virtualized-list:runtime-list:root"),
        "clicking the VirtualizedList root should focus it for keyboard navigation"
    );

    let row_0 = cx
        .debug_bounds("virtualized-list:runtime-list:row:item-0000")
        .expect("row 0 should render before keyboard navigation");
    cx.simulate_click(row_0.center(), Default::default());
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });
    activations.borrow_mut().clear();

    let row_4_before = cx
        .debug_bounds("virtualized-list:runtime-list:row:item-0004")
        .expect("row 4 should be present in the overscan window before PageDown");
    cx.simulate_keystrokes("pagedown");
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    let row_4_after = cx
        .debug_bounds("virtualized-list:runtime-list:row:item-0004")
        .expect("row 4 should stay rendered after PageDown reveal");
    assert!(
        row_4_after.top() < row_4_before.top(),
        "expected PageDown to scroll the new active row upward; before={row_4_before:?} after={row_4_after:?}"
    );

    cx.simulate_keystrokes("enter");
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    assert_eq!(activations.borrow().as_slice(), &[4]);
}

#[open_gpui::test]
fn scroll_area_default_handle_survives_reconstructed_component_values(
    cx: &mut open_gpui::TestAppContext,
) {
    struct TestView;

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let rows = (0..10).map(|index| {
                div()
                    .debug_selector(move || format!("scroll-row-{index}"))
                    .h(px(24.0))
                    .w_full()
                    .child(format!("Row {index}"))
            });

            div().size_full().child(
                div().w(px(180.0)).h(px(60.0)).child(
                    ScrollArea::new(
                        "default-runtime-scroll",
                        div().flex().flex_col().children(rows),
                    )
                    .vertical(),
                ),
            )
        }
    }

    let (_, cx) = cx.add_window_view(|_, _| TestView);
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    let before = cx
        .debug_bounds("scroll-row-2")
        .expect("row should be rendered before scrolling");

    cx.simulate_event(ScrollWheelEvent {
        position: point(px(10.0), px(10.0)),
        delta: ScrollDelta::Pixels(point(px(0.0), px(-40.0))),
        ..Default::default()
    });
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    let after = cx
        .debug_bounds("scroll-row-2")
        .expect("row should still be rendered after scrolling");

    assert!(
        after.top() < before.top(),
        "expected row bounds to move upward after wheel scrolling; before={before:?} after={after:?}"
    );
}

#[open_gpui::test]
fn scroll_area_reset_key_resets_default_runtime_handle(cx: &mut open_gpui::TestAppContext) {
    struct TestView {
        reset_key: String,
    }

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let rows = (0..10).map(|index| {
                div()
                    .debug_selector(move || format!("reset-row-{index}"))
                    .h(px(24.0))
                    .w_full()
                    .child(format!("Row {index}"))
            });

            div().size_full().child(
                div().w(px(180.0)).h(px(60.0)).child(
                    ScrollArea::new(
                        "reset-runtime-scroll",
                        div().flex().flex_col().children(rows),
                    )
                    .vertical()
                    .reset_on_key(self.reset_key.clone()),
                ),
            )
        }
    }

    let (view, cx) = cx.add_window_view(|_, _| TestView {
        reset_key: "overview".to_string(),
    });
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    let initial = cx
        .debug_bounds("reset-row-2")
        .expect("row should be rendered before scrolling");

    cx.simulate_event(ScrollWheelEvent {
        position: point(px(10.0), px(10.0)),
        delta: ScrollDelta::Pixels(point(px(0.0), px(-40.0))),
        ..Default::default()
    });
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    let scrolled = cx
        .debug_bounds("reset-row-2")
        .expect("row should still be rendered after scrolling");
    assert!(
        scrolled.top() < initial.top(),
        "expected row bounds to move upward after wheel scrolling; initial={initial:?} scrolled={scrolled:?}"
    );

    view.update(cx, |view, cx| {
        view.reset_key = "details".to_string();
        cx.notify();
    });
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    let reset = cx
        .debug_bounds("reset-row-2")
        .expect("row should still be rendered after reset");
    assert_eq!(
        reset.top(),
        initial.top(),
        "expected reset key change to restore the scroll origin; initial={initial:?} reset={reset:?}"
    );
}

#[open_gpui::test]
fn scroll_area_runtime_scrolls_horizontal_and_two_axis_content(cx: &mut open_gpui::TestAppContext) {
    struct TestView;

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let horizontal_cells = (0..8).map(|index| {
                div()
                    .debug_selector(move || format!("horizontal-cell-{index}"))
                    .w(px(96.0))
                    .h(px(40.0))
                    .flex_none()
                    .child(format!("Column {index}"))
            });
            let grid_rows = (0..8).map(|index| {
                div()
                    .debug_selector(move || format!("grid-row-{index}"))
                    .w(px(520.0))
                    .h(px(36.0))
                    .flex_none()
                    .child(format!("Grid row {index}"))
            });

            div()
                .size_full()
                .flex()
                .flex_col()
                .gap_4()
                .child(
                    div().w(px(180.0)).h(px(64.0)).child(
                        ScrollArea::new(
                            "horizontal-runtime-scroll",
                            div()
                                .flex()
                                .gap_2()
                                .min_w(px(820.0))
                                .children(horizontal_cells),
                        )
                        .horizontal(),
                    ),
                )
                .child(
                    div().w(px(180.0)).h(px(70.0)).child(
                        ScrollArea::new(
                            "two-axis-runtime-scroll",
                            div().flex().flex_col().min_w(px(520.0)).children(grid_rows),
                        )
                        .both(),
                    ),
                )
        }
    }

    let (_, cx) = cx.add_window_view(|_, _| TestView);
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    let horizontal_before = cx
        .debug_bounds("horizontal-cell-2")
        .expect("horizontal cell should be rendered before scrolling");
    let grid_before_x = cx
        .debug_bounds("grid-row-2")
        .expect("grid row should be rendered before scrolling");

    cx.simulate_event(ScrollWheelEvent {
        position: point(px(40.0), px(24.0)),
        delta: ScrollDelta::Pixels(point(px(-70.0), px(0.0))),
        ..Default::default()
    });
    cx.simulate_event(ScrollWheelEvent {
        position: point(px(40.0), px(108.0)),
        delta: ScrollDelta::Pixels(point(px(-60.0), px(0.0))),
        ..Default::default()
    });
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    let horizontal_after = cx
        .debug_bounds("horizontal-cell-2")
        .expect("horizontal cell should remain rendered after scrolling");
    let grid_after_x = cx
        .debug_bounds("grid-row-2")
        .expect("grid row should remain rendered after scrolling");

    assert!(
        horizontal_after.left() < horizontal_before.left(),
        "expected horizontal content to move left after wheel scrolling; before={horizontal_before:?} after={horizontal_after:?}"
    );
    assert!(
        grid_after_x.left() < grid_before_x.left(),
        "expected two-axis content to move left after horizontal wheel scrolling; before={grid_before_x:?} after={grid_after_x:?}"
    );

    cx.simulate_event(ScrollWheelEvent {
        position: point(px(40.0), px(108.0)),
        delta: ScrollDelta::Pixels(point(px(0.0), px(-42.0))),
        ..Default::default()
    });
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    let grid_after_y = cx
        .debug_bounds("grid-row-2")
        .expect("grid row should remain rendered after vertical scrolling");
    assert!(
        grid_after_y.top() < grid_after_x.top(),
        "expected two-axis content to move up after vertical wheel scrolling; before={grid_after_x:?} after={grid_after_y:?}"
    );
}

#[open_gpui::test]
fn scroll_area_nested_scroll_keeps_parent_static(cx: &mut open_gpui::TestAppContext) {
    struct TestView;

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let queue_lanes = (0..8).map(|index| {
                div()
                    .debug_selector(move || format!("nested-lane-{index}"))
                    .w(px(128.0))
                    .h(px(32.0))
                    .flex_none()
                    .child(format!("Lane {index}"))
            });
            let outer_rows = (0..10).map(|index| {
                div()
                    .debug_selector(move || format!("nested-outer-row-{index}"))
                    .h(px(24.0))
                    .w_full()
                    .child(format!("Outer row {index}"))
            });

            div().size_full().child(
                div().w(px(240.0)).h(px(120.0)).child(
                    ScrollArea::new(
                        "nested-outer-scroll",
                        div()
                            .flex()
                            .flex_col()
                            .gap_3()
                            .child(
                                div()
                                    .debug_selector(|| "nested-outer-header".into())
                                    .h(px(24.0))
                                    .w_full()
                                    .child("Outer header"),
                            )
                            .child(
                                div().h(px(52.0)).min_h(px(0.0)).overflow_hidden().child(
                                    ScrollArea::new(
                                        "nested-inner-scroll",
                                        div()
                                            .flex()
                                            .gap_2()
                                            .min_w(px(1024.0))
                                            .children(queue_lanes),
                                    )
                                    .horizontal()
                                    .with_size(Size::Small),
                                ),
                            )
                            .child(
                                div()
                                    .debug_selector(|| "nested-outer-bottom".into())
                                    .h(px(24.0))
                                    .w_full()
                                    .child("Outer bottom marker"),
                            )
                            .child(div().flex().flex_col().gap_1().children(outer_rows)),
                    )
                    .vertical()
                    .with_size(Size::Small),
                ),
            )
        }
    }

    let (_, cx) = cx.add_window_view(|_, _| TestView);
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    let lane_before = cx
        .debug_bounds("nested-lane-2")
        .expect("inner lane should be rendered before scrolling");
    let outer_before = cx
        .debug_bounds("nested-outer-bottom")
        .expect("outer marker should be rendered before scrolling");
    let inner_viewport = cx
        .debug_bounds("scroll-area:nested-inner-scroll")
        .expect("inner scroll viewport should be rendered before scrolling");

    cx.simulate_event(ScrollWheelEvent {
        position: inner_viewport.center(),
        delta: ScrollDelta::Pixels(point(px(0.0), px(-48.0))),
        ..Default::default()
    });
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    let lane_after = cx
        .debug_bounds("nested-lane-2")
        .expect("inner lane should remain rendered after scrolling");
    let outer_after = cx
        .debug_bounds("nested-outer-bottom")
        .expect("outer marker should remain rendered after scrolling");

    assert!(
        lane_after.left() < lane_before.left(),
        "expected nested horizontal ScrollArea to move after wheel scrolling; before={lane_before:?} after={lane_after:?}"
    );
    assert_eq!(
        outer_after.top(),
        outer_before.top(),
        "expected wheel scrolling inside the nested ScrollArea to leave the parent viewport in place; before={outer_before:?} after={outer_after:?}"
    );
}

#[open_gpui::test]
fn tabs_vertical_tablist_scrolls_when_constrained(cx: &mut open_gpui::TestAppContext) {
    struct TestView;

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let tabs = (0..12).fold(
                Tabs::new("overflow-tabs")
                    .orientation(Orientation::Vertical)
                    .small()
                    .default_selected("tab-0"),
                |tabs, index| {
                    tabs.item(TabsItem::new(
                        format!("tab-{index}"),
                        format!("Tab {index}"),
                        div().child(format!("Panel {index}")),
                    ))
                },
            );

            div()
                .size_full()
                .child(div().w(px(280.0)).h(px(120.0)).child(tabs))
        }
    }

    let (_, cx) = cx.add_window_view(|_, _| TestView);
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    let tab_before = cx
        .debug_bounds("tabs:overflow-tabs:trigger:tab-3")
        .expect("tab trigger should be rendered before scrolling");
    let tablist = cx
        .debug_bounds("tabs:overflow-tabs:tablist")
        .expect("tablist should be rendered");
    let tablist_viewport = cx
        .debug_bounds("scroll-area:tabs:overflow-tabs:tablist-scroll")
        .expect("vertical tablist should use the shared ScrollArea viewport");

    assert!(
        tablist.contains(&tablist_viewport.center()),
        "expected ScrollArea viewport to stay inside the tablist shell; tablist={tablist:?} viewport={tablist_viewport:?}"
    );

    cx.simulate_event(ScrollWheelEvent {
        position: tablist_viewport.center(),
        delta: ScrollDelta::Pixels(point(px(0.0), px(-64.0))),
        ..Default::default()
    });
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    let tab_after = cx
        .debug_bounds("tabs:overflow-tabs:trigger:tab-3")
        .expect("tab trigger should remain rendered after scrolling");

    assert!(
        tab_after.top() < tab_before.top(),
        "expected constrained vertical tablist to scroll; before={tab_before:?} after={tab_after:?}"
    );
}

#[open_gpui::test]
fn tabs_runtime_manual_keyboard_activation_preserves_selected_seed_and_payloads(
    cx: &mut open_gpui::TestAppContext,
) {
    struct TestView {
        selections: Rc<RefCell<Vec<TabsSelection>>>,
    }

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let selections = self.selections.clone();

            div().size_full().child(
                Tabs::new("runtime-tabs")
                    .activation_mode(TabsActivationMode::Manual)
                    .default_selected("details")
                    .item(TabsItem::new(
                        "overview",
                        "Overview",
                        div()
                            .debug_selector(|| "tabs-panel:overview".to_string())
                            .child("Overview panel"),
                    ))
                    .item(
                        TabsItem::new(
                            "billing",
                            "Billing",
                            div()
                                .debug_selector(|| "tabs-panel:billing".to_string())
                                .child("Billing panel"),
                        )
                        .disabled(true),
                    )
                    .item(TabsItem::new(
                        "details",
                        "Details",
                        div()
                            .debug_selector(|| "tabs-panel:details".to_string())
                            .child("Details panel"),
                    ))
                    .item(TabsItem::new(
                        "history",
                        "History",
                        div()
                            .debug_selector(|| "tabs-panel:history".to_string())
                            .child("History panel"),
                    ))
                    .on_selection_change(move |selection, _, _| {
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

    assert!(
        cx.debug_bounds("tabs-panel:details").is_some(),
        "expected seeded selected tab to render the Details panel"
    );

    let disabled_billing = cx
        .debug_bounds("tabs:runtime-tabs:trigger:billing")
        .expect("disabled Billing tab trigger should be rendered");
    cx.simulate_click(disabled_billing.center(), Default::default());
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });
    assert!(
        selections.borrow().is_empty(),
        "disabled tab click should not emit a selection change"
    );
    assert!(
        cx.debug_bounds("tabs-panel:details").is_some(),
        "disabled tab click should keep the current selected panel"
    );

    let overview = cx
        .debug_bounds("tabs:runtime-tabs:trigger:overview")
        .expect("Overview tab trigger should be rendered");
    cx.simulate_click(overview.center(), Default::default());
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });
    let after_click = selections.borrow().clone();
    assert_eq!(after_click.len(), 1);
    assert_eq!(after_click[0].index(), 0);
    assert_eq!(after_click[0].value(), "overview");
    assert_eq!(after_click[0].label(), "Overview");
    assert!(
        cx.debug_bounds("tabs-panel:overview").is_some(),
        "enabled tab click should render the selected panel"
    );

    cx.simulate_keystrokes("right");
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });
    assert_eq!(
        selections.borrow().len(),
        1,
        "manual activation should move roving focus without selecting on arrow key"
    );
    assert!(
        cx.debug_bounds("tabs-panel:overview").is_some(),
        "manual activation should keep the selected panel until Enter or Space"
    );

    cx.simulate_keystrokes("enter");
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });
    let after_enter = selections.borrow().clone();
    assert_eq!(after_enter.len(), 2);
    assert_eq!(after_enter[1].index(), 2);
    assert_eq!(after_enter[1].value(), "details");
    assert_eq!(after_enter[1].label(), "Details");
    assert!(
        cx.debug_bounds("tabs-panel:details").is_some(),
        "Enter should activate the focused tab after keyboard navigation skips disabled tabs"
    );

    cx.simulate_keystrokes("home enter");
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });
    let after_home = selections.borrow().clone();
    assert_eq!(after_home.len(), 3);
    assert_eq!(after_home[2].index(), 0);
    assert_eq!(after_home[2].value(), "overview");
    assert_eq!(after_home[2].label(), "Overview");
    assert!(
        cx.debug_bounds("tabs-panel:overview").is_some(),
        "Home plus Enter should activate the first enabled tab in manual mode"
    );

    cx.simulate_keystrokes("end space");
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });
    let after_space = selections.borrow().clone();
    assert_eq!(after_space.len(), 4);
    assert_eq!(after_space[3].index(), 3);
    assert_eq!(after_space[3].value(), "history");
    assert_eq!(after_space[3].label(), "History");
    assert!(
        cx.debug_bounds("tabs-panel:history").is_some(),
        "End plus Space should activate the last enabled tab in manual mode"
    );
}

#[open_gpui::test]
fn toolbar_runtime_keyboard_navigation_skips_disabled_and_separator_items(
    cx: &mut open_gpui::TestAppContext,
) {
    struct TestView {
        selections: Rc<RefCell<Vec<ToolbarSelection>>>,
    }

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let selections = self.selections.clone();

            div().size_full().child(
                Toolbar::new("keyboard-toolbar", "Keyboard toolbar")
                    .small()
                    .default_focused("bold")
                    .item(ToolbarItem::icon("undo", "U", "Undo"))
                    .item(ToolbarItem::icon("redo", "R", "Redo").disabled(true))
                    .item(ToolbarItem::separator("history-separator"))
                    .item(ToolbarItem::toggle_icon("bold", "B", "Bold").pressed(true))
                    .item(ToolbarItem::toggle_icon("italic", "I", "Italic"))
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

    let undo = cx
        .debug_bounds("toolbar:keyboard-toolbar:item:undo")
        .expect("undo toolbar item should be rendered");
    cx.simulate_click(undo.center(), Default::default());
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    cx.simulate_keystrokes("right enter");
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });
    let after_right = selections.borrow().clone();
    assert_eq!(after_right.len(), 2);
    assert_eq!(after_right[0].value(), "undo");
    assert_eq!(after_right[0].kind(), ToolbarItemKind::Action);
    assert_eq!(after_right[1].value(), "bold");
    assert_eq!(after_right[1].kind(), ToolbarItemKind::Toggle);

    cx.simulate_keystrokes("right enter");
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });
    let after_second_right = selections.borrow().clone();
    assert_eq!(after_second_right.len(), 3);
    assert_eq!(after_second_right[2].value(), "italic");
    assert_eq!(after_second_right[2].kind(), ToolbarItemKind::Toggle);

    cx.simulate_keystrokes("home enter");
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });
    let after_home = selections.borrow().clone();
    assert_eq!(after_home.len(), 4);
    assert_eq!(after_home[3].value(), "undo");
    assert_eq!(after_home[3].kind(), ToolbarItemKind::Action);
}

#[open_gpui::test]
fn toggle_group_controlled_values_override_runtime_selection(cx: &mut open_gpui::TestAppContext) {
    struct TestView {
        changes: Rc<RefCell<Vec<Vec<String>>>>,
    }

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let changes = self.changes.clone();

            div().size_full().child(
                ToggleGroup::new("controlled-toggle-group", "Alignment")
                    .default_selected_values(["right"])
                    .selected_values(Vec::<String>::new())
                    .item(ToggleGroupItem::new("left", "Left"))
                    .item(ToggleGroupItem::new("right", "Right"))
                    .on_change(move |change, _, _| {
                        changes.borrow_mut().push(change.selected_values().to_vec());
                    }),
            )
        }
    }

    let changes = Rc::new(RefCell::new(Vec::new()));
    let (_, cx) = cx.add_window_view(|_, _| TestView {
        changes: changes.clone(),
    });
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    let left = cx
        .debug_bounds("toggle-group:controlled-toggle-group:item:left")
        .expect("left toggle item should expose a stable debug selector");
    cx.simulate_click(left.center(), Default::default());
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    assert_eq!(changes.borrow().as_slice(), &[vec!["left".to_string()]]);

    let left = cx
        .debug_bounds("toggle-group:controlled-toggle-group:item:left")
        .expect("left toggle item should remain rendered after controlled redraw");
    cx.simulate_click(left.center(), Default::default());
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    assert_eq!(
        changes.borrow().as_slice(),
        &[vec!["left".to_string()], vec!["left".to_string()]],
        "controlled empty selection should reset adapter runtime before each activation"
    );
}

#[open_gpui::test]
fn splitter_runtime_drag_resizes_horizontal_and_vertical_panels(
    cx: &mut open_gpui::TestAppContext,
) {
    struct TestView;

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let horizontal = Splitter::new("horizontal-drag-split")
                .horizontal()
                .panel(SplitterPanel::new(
                    SplitterPanelDescriptor::new("left", 0.5).min_fraction(0.2),
                    div(),
                ))
                .panel(SplitterPanel::new(
                    SplitterPanelDescriptor::new("right", 0.5).min_fraction(0.2),
                    div(),
                ));
            let vertical = Splitter::new("vertical-drag-split")
                .vertical()
                .panel(SplitterPanel::new(
                    SplitterPanelDescriptor::new("top", 0.5).min_fraction(0.2),
                    div(),
                ))
                .panel(SplitterPanel::new(
                    SplitterPanelDescriptor::new("bottom", 0.5).min_fraction(0.2),
                    div(),
                ));

            div()
                .size_full()
                .flex()
                .flex_col()
                .gap_4()
                .child(div().w(px(400.0)).h(px(120.0)).child(horizontal))
                .child(div().w(px(240.0)).h(px(360.0)).child(vertical))
        }
    }

    let (_, cx) = cx.add_window_view(|_, _| TestView);
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    let left_before = cx
        .debug_bounds("splitter-panel:left")
        .expect("left panel should be rendered");
    let right_before = cx
        .debug_bounds("splitter-panel:right")
        .expect("right panel should be rendered");
    let horizontal_handle = cx
        .debug_bounds("splitter:horizontal-drag-split:handle:0")
        .expect("horizontal handle should be rendered")
        .center();
    let top_before = cx
        .debug_bounds("splitter-panel:top")
        .expect("top panel should be rendered");
    let bottom_before = cx
        .debug_bounds("splitter-panel:bottom")
        .expect("bottom panel should be rendered");
    let vertical_handle = cx
        .debug_bounds("splitter:vertical-drag-split:handle:0")
        .expect("vertical handle should be rendered")
        .center();

    cx.simulate_mouse_down(horizontal_handle, MouseButton::Left, Default::default());
    cx.simulate_mouse_move(
        point(horizontal_handle.x + px(4.0), horizontal_handle.y),
        MouseButton::Left,
        Default::default(),
    );
    cx.simulate_mouse_move(
        point(horizontal_handle.x + px(24.0), horizontal_handle.y),
        MouseButton::Left,
        Default::default(),
    );
    cx.simulate_mouse_move(
        point(horizontal_handle.x + px(80.0), horizontal_handle.y),
        MouseButton::Left,
        Default::default(),
    );
    cx.simulate_mouse_up(
        point(horizontal_handle.x + px(80.0), horizontal_handle.y),
        MouseButton::Left,
        Default::default(),
    );
    cx.simulate_mouse_down(vertical_handle, MouseButton::Left, Default::default());
    cx.simulate_mouse_move(
        point(vertical_handle.x, vertical_handle.y + px(4.0)),
        MouseButton::Left,
        Default::default(),
    );
    cx.simulate_mouse_move(
        point(vertical_handle.x, vertical_handle.y + px(24.0)),
        MouseButton::Left,
        Default::default(),
    );
    cx.simulate_mouse_move(
        point(vertical_handle.x, vertical_handle.y + px(72.0)),
        MouseButton::Left,
        Default::default(),
    );
    cx.simulate_mouse_up(
        point(vertical_handle.x, vertical_handle.y + px(72.0)),
        MouseButton::Left,
        Default::default(),
    );
    cx.run_until_parked();
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    let left_after = cx
        .debug_bounds("splitter-panel:left")
        .expect("left panel should remain rendered");
    let right_after = cx
        .debug_bounds("splitter-panel:right")
        .expect("right panel should remain rendered");
    let top_after = cx
        .debug_bounds("splitter-panel:top")
        .expect("top panel should remain rendered");
    let bottom_after = cx
        .debug_bounds("splitter-panel:bottom")
        .expect("bottom panel should remain rendered");

    assert!(
        left_after.size.width > left_before.size.width
            && right_after.size.width < right_before.size.width,
        "expected horizontal drag to grow the first panel and shrink the second; before=({left_before:?}, {right_before:?}) after=({left_after:?}, {right_after:?})"
    );
    assert!(
        top_after.size.height > top_before.size.height
            && bottom_after.size.height < bottom_before.size.height,
        "expected vertical drag to grow the first panel and shrink the second; before=({top_before:?}, {bottom_before:?}) after=({top_after:?}, {bottom_after:?})"
    );
}

#[test]
fn splitter_state_normalizes_panel_fractions_and_constraints() {
    let state = SplitterState::resolve(
        "workspace",
        Orientation::Horizontal,
        Size::Medium,
        false,
        [
            SplitterPanelDescriptor::new("nav", 0.2)
                .min_fraction(0.18)
                .max_fraction(0.32),
            SplitterPanelDescriptor::new("main", 0.65)
                .min_fraction(0.42)
                .max_fraction(0.7),
            SplitterPanelDescriptor::new("inspector", 0.35)
                .min_fraction(0.12)
                .max_fraction(0.28),
        ],
    );

    let sum: f32 = state.panels().iter().map(|panel| panel.fraction()).sum();
    assert_eq!(state.group_id(), "workspace");
    assert_eq!(state.orientation(), Orientation::Horizontal);
    assert_eq!(state.size(), Size::Medium);
    assert!((sum - 1.0).abs() < 0.001);
    assert_eq!(state.panels().len(), 3);
    assert!(state.panels()[0].fraction() >= 0.18);
    assert!(state.panels()[1].fraction() <= 0.7);
    assert!(state.panels()[2].fraction() <= 0.28);
    assert_eq!(state.handles().len(), 2);
    assert_eq!(state.handles()[0].before_id(), "nav");
    assert_eq!(state.handles()[0].after_id(), "main");
    assert_eq!(state.metrics().handle_hit_size(), ui_px(12.0));
}

#[test]
fn splitter_resize_delta_clamps_to_adjacent_min_max() {
    let state = SplitterState::resolve(
        "editor",
        Orientation::Horizontal,
        Size::Small,
        false,
        [
            SplitterPanelDescriptor::new("left", 0.35)
                .min_fraction(0.2)
                .max_fraction(0.4),
            SplitterPanelDescriptor::new("right", 0.65)
                .min_fraction(0.5)
                .max_fraction(0.8),
        ],
    );
    let grown = state.resized_by(0, 0.3);
    let shrunk = grown.resized_by(0, -0.5);

    assert!((grown.panels()[0].fraction() - 0.4).abs() < 0.001);
    assert!((grown.panels()[1].fraction() - 0.6).abs() < 0.001);
    assert!((shrunk.panels()[0].fraction() - 0.2).abs() < 0.001);
    assert!((shrunk.panels()[1].fraction() - 0.8).abs() < 0.001);
}

#[test]
fn splitter_runtime_fraction_overrides_still_use_resize_constraints() {
    let state = SplitterState::resolve(
        "runtime-editor",
        Orientation::Horizontal,
        Size::Medium,
        false,
        [
            SplitterPanelDescriptor::new("left", 0.3)
                .min_fraction(0.15)
                .max_fraction(0.75),
            SplitterPanelDescriptor::new("right", 0.7)
                .min_fraction(0.25)
                .max_fraction(0.85),
        ],
    );

    let overridden = state.with_panel_fractions(&[0.45, 0.55]);
    let grown = overridden.resized_by(0, 0.5);
    let invalid = overridden.with_panel_fractions(&[0.2]);

    assert!((overridden.panels()[0].fraction() - 0.45).abs() < 0.001);
    assert!((overridden.panels()[1].fraction() - 0.55).abs() < 0.001);
    assert!((grown.panels()[0].fraction() - 0.75).abs() < 0.001);
    assert!((grown.panels()[1].fraction() - 0.25).abs() < 0.001);
    assert_eq!(invalid, overridden);
}

#[test]
fn splitter_collapsed_panel_uses_collapsed_fraction() {
    let state = Splitter::new("collapsed-split")
        .vertical()
        .small()
        .panel(SplitterPanel::new(
            SplitterPanelDescriptor::new("summary", 0.3)
                .min_fraction(0.2)
                .collapsible(true)
                .collapsed(true)
                .collapsed_fraction(0.05),
            div(),
        ))
        .panel(SplitterPanel::new(
            SplitterPanelDescriptor::new("details", 0.7).min_fraction(0.4),
            div(),
        ))
        .state();

    assert_eq!(state.orientation(), Orientation::Vertical);
    assert!(state.panels()[0].collapsible());
    assert!(state.panels()[0].collapsed());
    assert!((state.panels()[0].fraction() - 0.05).abs() < 0.001);
    assert_eq!(state.panels()[0].collapsed_fraction(), 0.05);
    assert_eq!(state.handles().len(), 1);
    assert!(!state.handles()[0].disabled());

    let unchanged = state.resized_by(0, 0.1);
    let restored = state.resized_by(0, 0.16);
    let runtime_restored = state.with_panel_fractions(&[0.22, 0.78]);

    assert_eq!(unchanged, state);
    assert!(!restored.panels()[0].collapsed());
    assert!(restored.panels()[0].fraction() >= 0.2);
    assert!(!runtime_restored.panels()[0].collapsed());
    assert!((runtime_restored.panels()[0].fraction() - 0.22).abs() < 0.001);
}

#[open_gpui::test]
fn radio_group_runtime_keyboard_navigation_skips_disabled_items_and_payloads(
    cx: &mut open_gpui::TestAppContext,
) {
    struct TestView {
        selections: Rc<RefCell<Vec<RadioSelection>>>,
    }

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let selections = self.selections.clone();

            div().size_full().child(
                RadioGroup::new("runtime-radio")
                    .label("Runtime radio")
                    .orientation(Orientation::Horizontal)
                    .default_selected("personal")
                    .item(RadioItem::new("personal", "Personal"))
                    .item(RadioItem::new("team", "Team").disabled(true))
                    .item(RadioItem::new("enterprise", "Enterprise"))
                    .on_selection_change(move |selection, _, _| {
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

    assert!(
        cx.debug_bounds("radio-group:runtime-radio").is_some(),
        "radio group root should expose a stable debug selector"
    );

    let disabled_team = cx
        .debug_bounds("radio-group:runtime-radio:item:team")
        .expect("disabled Team radio item should be rendered");
    cx.simulate_click(disabled_team.center(), Default::default());
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });
    assert!(
        selections.borrow().is_empty(),
        "disabled radio click should not emit a selection change"
    );

    let enterprise = cx
        .debug_bounds("radio-group:runtime-radio:item:enterprise")
        .expect("Enterprise radio item should be rendered");
    cx.simulate_click(enterprise.center(), Default::default());
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });
    let after_click = selections.borrow().clone();
    assert_eq!(after_click.len(), 1);
    assert_eq!(after_click[0].index(), 2);
    assert_eq!(after_click[0].value(), "enterprise");
    assert_eq!(after_click[0].label(), "Enterprise");

    cx.simulate_keystrokes("left");
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });
    let after_left = selections.borrow().clone();
    assert_eq!(after_left.len(), 2);
    assert_eq!(after_left[1].index(), 0);
    assert_eq!(after_left[1].value(), "personal");
    assert_eq!(after_left[1].label(), "Personal");

    cx.simulate_keystrokes("space");
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });
    let after_space = selections.borrow().clone();
    assert_eq!(
        after_space.len(),
        2,
        "Space on the already selected radio should not emit a duplicate selection change"
    );

    cx.simulate_keystrokes("right");
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });
    let after_right = selections.borrow().clone();
    assert_eq!(after_right.len(), 3);
    assert_eq!(after_right[2].index(), 2);
    assert_eq!(after_right[2].value(), "enterprise");
    assert_eq!(after_right[2].label(), "Enterprise");
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
    assert_eq!(state.tab_stop_index(), state.focused_index());
    assert_eq!(state.items().len(), 3);
    assert!(state.items()[1].selected());
    assert!(state.items()[1].focused());
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
    assert_eq!(state.tab_stop_index(), state.focused_index());
    assert!(state.items()[1].disabled());
    assert!(!state.items()[1].focused());
}

#[test]
fn radio_group_builder_state_falls_back_to_first_enabled_item() {
    let state = RadioGroup::new("plan")
        .label("Plan")
        .orientation(Orientation::Horizontal)
        .with_size(Size::Large)
        .required(true)
        .default_selected("enterprise")
        .item(RadioItem::new("starter", "Starter"))
        .item(RadioItem::new("pro", "Pro"))
        .item(RadioItem::new("enterprise", "Enterprise").disabled(true))
        .state();

    assert_eq!(state.orientation(), Orientation::Horizontal);
    assert_eq!(state.size(), Size::Large);
    assert!(state.required());
    assert_eq!(state.selected_value(), Some("starter"));
    assert_eq!(state.focused_value(), Some("starter"));
    assert_eq!(state.tab_stop_index(), state.focused_index());
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
fn sidebar_state_exposes_shell_navigation_contract() {
    let state = SidebarState::resolve(
        SidebarSide::Left,
        SidebarVariant::Docked,
        SidebarCollapseMode::Icon,
        false,
        false,
        "Primary navigation",
        Some("projects"),
        None,
        [
            SidebarSectionDescriptor::new("workspace", "Workspace").items([
                SidebarItemDescriptor::new("home", "Home").icon("H"),
                SidebarItemDescriptor::new("projects", "Projects")
                    .icon("P")
                    .badge("12"),
                SidebarItemDescriptor::new("archive", "Archive")
                    .icon("A")
                    .disabled(true),
            ]),
            SidebarSectionDescriptor::new("account", "Account").items([
                SidebarItemDescriptor::new("settings", "Settings").icon("S"),
                SidebarItemDescriptor::new("billing", "Billing")
                    .icon("B")
                    .action_label("new"),
            ]),
        ],
        Size::Medium,
        ThemeTokens::default(),
    );

    assert_eq!(state.role(), Role::Navigation);
    assert_eq!(state.side(), SidebarSide::Left);
    assert_eq!(state.variant(), SidebarVariant::Docked);
    assert_eq!(state.collapse_mode(), SidebarCollapseMode::Icon);
    assert!(!state.collapsed());
    assert_eq!(state.sections().len(), 2);
    assert_eq!(state.sections()[0].role(), Role::Section);
    assert_eq!(state.items().len(), 5);
    assert_eq!(state.selected_value(), Some("projects"));
    assert_eq!(state.focused_value(), Some("projects"));
    assert_eq!(state.focused_index(), Some(1));
    assert!(state.scrollable());
    assert!(state.items()[1].selected());
    assert_eq!(state.items()[1].badge_label(), Some("12"));
    assert!(!state.items()[2].activation_enabled());
    assert_eq!(state.items()[1].role(), Role::Button);
    assert_eq!(state.items()[1].position_in_set(), Some(2));
    assert_eq!(state.items()[1].size_of_set(), 4);
    assert_eq!(
        state.navigation_target("down").map(|item| item.value()),
        Some("settings")
    );
    assert_eq!(
        state
            .activation_for_key("enter")
            .map(|selection| selection.value().to_owned()),
        Some("projects".to_string())
    );
}

#[test]
fn sidebar_icon_collapse_keeps_accessible_items_but_hides_text() {
    let state = Sidebar::new("app-sidebar", "Application")
        .collapse_mode(SidebarCollapseMode::Icon)
        .collapsed(true)
        .selected("dashboard")
        .section(
            SidebarSection::new("main", "Main")
                .item(SidebarItem::new("dashboard", "Dashboard").icon("D"))
                .item(SidebarItem::new("inbox", "Inbox").icon("I").badge("4")),
        )
        .state();

    assert!(state.collapsed());
    assert!(state.icon_collapsed());
    assert!(!state.offcanvas_collapsed());
    assert_eq!(
        state.metrics().resolved_width(),
        state.metrics().collapsed_width()
    );
    assert_eq!(state.selected_value(), Some("dashboard"));
    assert_eq!(state.focused_value(), Some("dashboard"));
    assert!(state.scrollable());
    assert!(state.items()[0].focusable());
    assert_eq!(state.items()[0].label(), "Dashboard");
    assert_eq!(state.items()[1].badge_label(), Some("4"));
}

#[test]
fn sidebar_offcanvas_collapse_removes_items_from_roving_focus() {
    let state = SidebarState::resolve(
        SidebarSide::Right,
        SidebarVariant::Floating,
        SidebarCollapseMode::Offcanvas,
        true,
        false,
        "Secondary navigation",
        Some("reports"),
        None,
        [SidebarSectionDescriptor::new("main", "Main").items([
            SidebarItemDescriptor::new("overview", "Overview"),
            SidebarItemDescriptor::new("reports", "Reports"),
        ])],
        Size::Small,
        ThemeTokens::default(),
    );

    assert!(state.collapsed());
    assert!(state.offcanvas_collapsed());
    assert_eq!(state.metrics().resolved_width(), ui_px(0.0));
    assert_eq!(state.selected_value(), None);
    assert_eq!(state.focused_value(), None);
    assert_eq!(state.focused_index(), None);
    assert!(!state.scrollable());
    assert!(!state.items()[0].focusable());
    assert!(state.activation_for_key("space").is_none());
}

#[open_gpui::test]
fn sidebar_long_navigation_scrolls_inside_shared_scroll_area(cx: &mut open_gpui::TestAppContext) {
    struct TestView;

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let section = (0..14).fold(SidebarSection::new("main", "Main"), |section, index| {
                section.item(
                    SidebarItem::new(format!("item-{index}"), format!("Item {index}"))
                        .icon(index.to_string()),
                )
            });

            div().size_full().child(
                div()
                    .h(px(120.0))
                    .child(Sidebar::new("long-sidebar", "Long navigation").section(section)),
            )
        }
    }

    let (_, cx) = cx.add_window_view(|_, _| TestView);
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    let item_before = cx
        .debug_bounds("sidebar:long-sidebar:item:item-2")
        .expect("sidebar item should be rendered before scrolling");
    let sidebar = cx
        .debug_bounds("sidebar:long-sidebar")
        .expect("sidebar shell should be rendered");
    let sidebar_viewport = cx
        .debug_bounds("scroll-area:long-sidebar-scroll")
        .expect("long Sidebar should use the shared ScrollArea viewport");

    assert!(
        sidebar.contains(&sidebar_viewport.center()),
        "expected Sidebar ScrollArea viewport to stay inside the sidebar shell; sidebar={sidebar:?} viewport={sidebar_viewport:?}"
    );

    cx.simulate_event(ScrollWheelEvent {
        position: sidebar_viewport.center(),
        delta: ScrollDelta::Pixels(point(px(0.0), px(-72.0))),
        ..Default::default()
    });
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    let item_after = cx
        .debug_bounds("sidebar:long-sidebar:item:item-2")
        .expect("sidebar item should remain rendered after scrolling");

    assert!(
        item_after.top() < item_before.top(),
        "expected long Sidebar navigation to scroll inside its ScrollArea; before={item_before:?} after={item_after:?}"
    );
}

#[test]
fn sidebar_navigation_helper_skips_disabled_items() {
    assert_eq!(
        sidebar_navigation_target("down", 0, &[false, true, false]),
        Some(2)
    );
    assert_eq!(
        sidebar_navigation_target("up", 0, &[false, true, false]),
        Some(2)
    );
    assert_eq!(
        sidebar_navigation_target("home", 2, &[false, true, false]),
        Some(0)
    );
    assert_eq!(sidebar_navigation_target("right", 0, &[false, false]), None);
}

#[test]
fn toolbar_state_exposes_roving_focus_and_toggle_metadata() {
    let state = ToolbarState::resolve(
        Orientation::Horizontal,
        Size::Small,
        false,
        "Editor toolbar",
        Some("bold"),
        [
            ToolbarItemDescriptor::action("undo", "Undo"),
            ToolbarItemDescriptor::separator("history-separator"),
            ToolbarItemDescriptor::toggle("bold", "Bold").pressed(true),
            ToolbarItemDescriptor::toggle("italic", "Italic").disabled(true),
            ToolbarItemDescriptor::action("save", "Save"),
        ],
        ThemeTokens::default(),
    );

    assert_eq!(state.role(), Role::Toolbar);
    assert_eq!(state.orientation(), Orientation::Horizontal);
    assert_eq!(state.size(), Size::Small);
    assert_eq!(state.label(), "Editor toolbar");
    assert_eq!(state.items().len(), 5);
    assert_eq!(state.focused_value(), Some("bold"));
    assert_eq!(state.tab_stop_index(), state.focused_index());
    assert_eq!(state.items()[0].role(), Some(Role::Button));
    assert_eq!(state.items()[1].kind(), ToolbarItemKind::Separator);
    assert_eq!(state.items()[1].role(), None);
    assert!(!state.items()[1].focusable());
    assert!(state.items()[2].pressed());
    assert_eq!(state.items()[2].toggled(), Some(Toggled::True));
    assert!(!state.items()[3].activation_enabled());
    assert_eq!(
        state.navigation_target("right").map(|item| item.value()),
        Some("save")
    );
    assert_eq!(
        state
            .activation_for_key("space")
            .map(|selection| (selection.value().to_owned(), selection.kind())),
        Some(("bold".to_string(), ToolbarItemKind::Toggle))
    );
}

#[test]
fn toolbar_builder_state_skips_disabled_and_separator_items() {
    let state = Toolbar::new("editor-tools", "Editor")
        .orientation(Orientation::Vertical)
        .large()
        .default_focused("missing")
        .item(ToolbarItem::action("cut", "Cut").disabled(true))
        .item(ToolbarItem::separator("clipboard-separator"))
        .item(ToolbarItem::icon("copy", "C", "Copy"))
        .item(ToolbarItem::toggle("wrap", "Wrap").pressed(true))
        .state();

    assert_eq!(state.orientation(), Orientation::Vertical);
    assert_eq!(state.size(), Size::Large);
    assert_eq!(state.focused_value(), Some("copy"));
    assert_eq!(state.tab_stop_index(), state.focused_index());
    assert!(state.items()[0].disabled());
    assert_eq!(state.items()[1].kind(), ToolbarItemKind::Separator);
    assert!(state.items()[3].pressed());
    assert_eq!(
        toolbar_navigation_target(
            Orientation::Vertical,
            "down",
            state.focused_index().unwrap(),
            &[true, true, false, false],
        ),
        Some(3)
    );
}

#[test]
fn listbox_state_resolves_grouped_options_navigation_and_typeahead() {
    let state = ListboxState::resolve(
        Size::Small,
        false,
        "Assignee",
        Some("bravo"),
        Some("missing"),
        Some("ch"),
        "No assignees",
        [ListboxGroupDescriptor::new("team", "Team")
            .option(ListboxOptionDescriptor::option("charlie", "Charlie"))
            .option(ListboxOptionDescriptor::option("delta", "Delta").disabled(true))
            .option(ListboxOptionDescriptor::option("bravo", "Bravo"))],
        [
            ListboxOptionDescriptor::option("alpha", "Alpha"),
            ListboxOptionDescriptor::separator("standalone-separator"),
        ],
        ThemeTokens::default(),
    );

    assert_eq!(state.role(), Role::ListBox);
    assert_eq!(state.label(), "Assignee");
    assert_eq!(state.typeahead_query(), Some("ch"));
    assert_eq!(state.groups().len(), 1);
    assert_eq!(state.groups()[0].role(), Role::Group);
    assert_eq!(state.groups()[0].option_count(), 3);
    assert_eq!(state.options().len(), 5);
    assert_eq!(state.selected_value(), Some("bravo"));
    assert_eq!(state.active_value(), Some("bravo"));
    assert_eq!(state.options()[1].kind(), ListboxOptionKind::Separator);
    assert_eq!(state.options()[1].role(), None);
    assert!(!state.options()[1].focusable());
    assert!(state.options()[3].disabled());
    assert!(!state.options()[3].focusable());
    assert_eq!(state.options()[4].role(), Some(Role::ListBoxOption));
    assert_eq!(state.options()[4].position_in_set(), Some(4));
    assert_eq!(state.options()[4].size_of_set(), 4);
    assert_eq!(
        state.navigation_target("down").map(|option| option.value()),
        Some("alpha")
    );
    assert_eq!(
        state.typeahead_target("ch").map(|option| option.value()),
        Some("charlie")
    );
    assert_eq!(
        state
            .activation_for_key("enter")
            .map(|selection| selection.value().to_owned()),
        Some("bravo".to_string())
    );
    assert_eq!(
        listbox_navigation_target(
            "down",
            state.active_index().unwrap(),
            &[false, true, false, true, false]
        ),
        Some(0)
    );
}

#[test]
fn choice_surfaces_share_stable_value_resolution_and_query_normalization() {
    let listbox = ListboxState::resolve(
        Size::Small,
        false,
        "Shared choices",
        Some("disabled"),
        Some("missing"),
        Some("  AL "),
        "No choices",
        [ListboxGroupDescriptor::new("group", "Group")
            .option(ListboxOptionDescriptor::option("grouped", "Grouped"))],
        [
            ListboxOptionDescriptor::option("alpha", "Alpha"),
            ListboxOptionDescriptor::option("disabled", "Disabled").disabled(true),
        ],
        ThemeTokens::default(),
    );

    let select = Select::new("shared-select", "Shared choices")
        .placeholder("Pick one")
        .selected("disabled")
        .option(ListboxOption::new("alpha", "Alpha"))
        .option(ListboxOption::new("disabled", "Disabled").disabled(true))
        .group(ListboxGroup::new("group", "Group").option(ListboxOption::new("grouped", "Grouped")))
        .state();

    let combobox = Combobox::new("shared-combobox", "Shared choices")
        .default_query("  AL ")
        .selected("disabled")
        .option(ComboboxOption::new("alpha", "Alpha"))
        .option(ComboboxOption::new("disabled", "Disabled").disabled(true))
        .group(
            ComboboxGroup::new("group", "Group").option(ComboboxOption::new("grouped", "Grouped")),
        )
        .state();

    let command = Command::new("shared-command", "Shared choices")
        .default_query("  AL ")
        .selected("disabled")
        .item(CommandItem::new("alpha", "Alpha"))
        .item(CommandItem::new("disabled", "Disabled").disabled(true))
        .group(CommandGroup::new("group", "Group").item(CommandItem::new("grouped", "Grouped")))
        .state();

    assert_eq!(listbox.typeahead_query(), Some("al"));
    assert_eq!(listbox.selected_value(), None);
    assert_eq!(listbox.active_value(), Some("alpha"));
    assert_eq!(
        listbox
            .typeahead_target("  AL ")
            .map(|option| option.value()),
        Some("alpha")
    );

    assert_eq!(select.selected_value(), None);
    assert_eq!(select.active_value(), Some("alpha"));
    assert_eq!(select.trigger_label(), "Pick one");

    assert_eq!(combobox.query(), "  AL ");
    assert_eq!(combobox.filtered_option_count(), 1);
    assert_eq!(combobox.selected_value(), None);
    assert_eq!(combobox.active_value(), Some("alpha"));
    assert_eq!(combobox.listbox().typeahead_query(), Some("al"));

    assert_eq!(command.query(), "  AL ");
    assert_eq!(command.filtered_item_count(), 1);
    assert_eq!(command.selected_value(), None);
    assert_eq!(command.active_value(), Some("alpha"));
    assert_eq!(command.listbox().typeahead_query(), Some("al"));
}

#[test]
fn listbox_select_and_combobox_project_equivalent_choice_semantics() {
    let listbox = ListboxState::resolve(
        Size::Small,
        false,
        "Shared choices",
        Some("bravo"),
        Some("charlie"),
        None,
        "No choices",
        [],
        [
            ListboxOptionDescriptor::option("alpha", "Alpha"),
            ListboxOptionDescriptor::option("bravo", "Bravo"),
            ListboxOptionDescriptor::option("disabled", "Disabled").disabled(true),
            ListboxOptionDescriptor::option("charlie", "Charlie"),
        ],
        ThemeTokens::default(),
    );
    let select = Select::new("shared-select-semantics", "Shared choices")
        .placeholder("Pick one")
        .selected("bravo")
        .active("charlie")
        .option(ListboxOption::new("alpha", "Alpha"))
        .option(ListboxOption::new("bravo", "Bravo"))
        .option(ListboxOption::new("disabled", "Disabled").disabled(true))
        .option(ListboxOption::new("charlie", "Charlie"))
        .state();
    let combobox = Combobox::new("shared-combobox-semantics", "Shared choices")
        .placeholder("Search choices")
        .selected("bravo")
        .active("charlie")
        .option(ComboboxOption::new("alpha", "Alpha"))
        .option(ComboboxOption::new("bravo", "Bravo"))
        .option(ComboboxOption::new("disabled", "Disabled").disabled(true))
        .option(ComboboxOption::new("charlie", "Charlie"))
        .state();

    for state in [
        listbox,
        select.listbox().clone(),
        combobox.listbox().clone(),
    ] {
        assert_eq!(state.selected_value(), Some("bravo"));
        assert_eq!(state.active_value(), Some("charlie"));
        assert_eq!(
            state.selected_option().map(|option| option.value()),
            Some("bravo")
        );
        assert_eq!(
            state.active_option().map(|option| option.value()),
            Some("charlie")
        );
        assert_eq!(
            state.typeahead_target(" al").map(|option| option.value()),
            Some("alpha")
        );
        assert!(state.options()[1].selected());
        assert!(state.options()[2].disabled());
        assert!(!state.options()[2].focusable());
        assert!(state.options()[3].active());
    }

    assert_eq!(select.trigger_label(), "Bravo");
    assert_eq!(combobox.selected_value(), Some("bravo"));
}

#[test]
fn listbox_state_scrollable_content_tracks_flattened_option_count_threshold() {
    let scrollable = ListboxState::resolve(
        Size::Small,
        false,
        "Scrollable",
        None,
        None,
        None,
        "No options",
        [],
        (0..7).map(|index| {
            ListboxOptionDescriptor::option(format!("item-{index}"), format!("Item {index}"))
        }),
        ThemeTokens::default(),
    );
    let not_scrollable = ListboxState::resolve(
        Size::Small,
        false,
        "Compact",
        None,
        None,
        None,
        "No options",
        [],
        (0..6).map(|index| {
            ListboxOptionDescriptor::option(format!("item-{index}"), format!("Item {index}"))
        }),
        ThemeTokens::default(),
    );

    assert!(scrollable.scrollable_content());
    assert!(!not_scrollable.scrollable_content());
}

#[test]
fn listbox_builder_state_models_empty_disabled_and_tokens() {
    let tokens = custom_tokens();
    let empty = Listbox::new("empty-listbox", "Empty")
        .empty_label("Nothing available")
        .tokens(tokens)
        .state();
    let disabled = Listbox::new("disabled-listbox", "Disabled")
        .disabled(true)
        .selected("one")
        .option(ListboxOption::new("one", "One"))
        .state();

    assert!(empty.empty());
    assert_eq!(empty.empty_label(), "Nothing available");
    assert_eq!(empty.colors().surface().token(), tokens.surface);
    assert!(disabled.disabled());
    assert_eq!(disabled.selected_value(), None);
    assert_eq!(disabled.active_value(), None);
    assert_eq!(disabled.activation_for_key("space"), None);
}

#[open_gpui::test]
fn listbox_runtime_click_and_keyboard_selection_skip_disabled_items(
    cx: &mut open_gpui::TestAppContext,
) {
    #[derive(Clone, Debug, PartialEq, Eq)]
    struct SelectionEvent {
        source: &'static str,
        selection: ListboxSelection,
    }

    struct TestView {
        events: Rc<RefCell<Vec<SelectionEvent>>>,
    }

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let listbox_events = self.events.clone();
            let alpha_events = self.events.clone();
            let charlie_events = self.events.clone();

            div().size_full().child(
                Listbox::new("runtime-listbox", "Runtime listbox")
                    .selected("alpha")
                    .option(ListboxOption::new("alpha", "Alpha").on_select(
                        move |selection, _, _| {
                            alpha_events.borrow_mut().push(SelectionEvent {
                                source: "option:alpha",
                                selection,
                            });
                        },
                    ))
                    .option(ListboxOption::separator("standalone-separator"))
                    .option(ListboxOption::new("bravo", "Bravo").disabled(true))
                    .group(
                        ListboxGroup::new("team", "Team")
                            .option(ListboxOption::new("charlie", "Charlie").on_select(
                                move |selection, _, _| {
                                    charlie_events.borrow_mut().push(SelectionEvent {
                                        source: "option:charlie",
                                        selection,
                                    });
                                },
                            ))
                            .option(ListboxOption::new("delta", "Delta")),
                    )
                    .on_select(move |selection, _, _| {
                        listbox_events.borrow_mut().push(SelectionEvent {
                            source: "listbox",
                            selection,
                        });
                    }),
            )
        }
    }

    let events = Rc::new(RefCell::new(Vec::new()));
    let (_, cx) = cx.add_window_view(|_, _| TestView {
        events: events.clone(),
    });
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    assert!(
        cx.debug_bounds("listbox:runtime-listbox").is_some(),
        "listbox root should expose a stable debug selector"
    );
    assert!(
        cx.debug_bounds("listbox:runtime-listbox:separator:standalone-separator")
            .is_some(),
        "listbox separator should expose a stable debug selector"
    );
    assert!(
        cx.debug_bounds("listbox:runtime-listbox:group:team")
            .is_some(),
        "listbox group label should expose a stable debug selector"
    );

    let disabled_bravo = cx
        .debug_bounds("listbox:runtime-listbox:option:bravo")
        .expect("disabled Bravo option should be rendered");
    cx.simulate_click(disabled_bravo.center(), Default::default());
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });
    assert!(
        events.borrow().is_empty(),
        "disabled option click should not emit selection callbacks"
    );

    let delta = cx
        .debug_bounds("listbox:runtime-listbox:option:delta")
        .expect("Delta option should be rendered");
    cx.simulate_click(delta.center(), Default::default());
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });
    let after_delta_click = events.borrow().clone();
    assert_eq!(after_delta_click.len(), 1);
    assert_eq!(after_delta_click[0].source, "listbox");
    assert_eq!(after_delta_click[0].selection.index(), 4);
    assert_eq!(after_delta_click[0].selection.value(), "delta");
    assert_eq!(after_delta_click[0].selection.label(), "Delta");

    let alpha = cx
        .debug_bounds("listbox:runtime-listbox:option:alpha")
        .expect("Alpha option should be rendered");
    cx.simulate_click(alpha.center(), Default::default());
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });
    let after_alpha_click = events.borrow().clone();
    assert_eq!(after_alpha_click.len(), 3);
    assert_eq!(after_alpha_click[1].source, "option:alpha");
    assert_eq!(after_alpha_click[1].selection.index(), 0);
    assert_eq!(after_alpha_click[1].selection.value(), "alpha");
    assert_eq!(after_alpha_click[2].source, "listbox");
    assert_eq!(after_alpha_click[2].selection.value(), "alpha");

    cx.simulate_keystrokes("down");
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });
    assert_eq!(
        events.borrow().len(),
        3,
        "arrow navigation should move active option without selecting"
    );

    cx.simulate_keystrokes("enter");
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });
    let after_enter = events.borrow().clone();
    assert_eq!(after_enter.len(), 5);
    assert_eq!(after_enter[3].source, "option:charlie");
    assert_eq!(after_enter[3].selection.index(), 3);
    assert_eq!(after_enter[3].selection.value(), "charlie");
    assert_eq!(after_enter[3].selection.label(), "Charlie");
    assert_eq!(after_enter[4].source, "listbox");
    assert_eq!(after_enter[4].selection.value(), "charlie");

    cx.simulate_keystrokes("up");
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });
    assert_eq!(
        events.borrow().len(),
        5,
        "arrow navigation after selection should still move active option without selecting"
    );

    cx.simulate_keystrokes("space");
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });
    let after_space = events.borrow().clone();
    assert_eq!(after_space.len(), 7);
    assert_eq!(after_space[5].source, "option:alpha");
    assert_eq!(after_space[5].selection.index(), 0);
    assert_eq!(after_space[5].selection.value(), "alpha");
    assert_eq!(after_space[5].selection.label(), "Alpha");
    assert_eq!(after_space[6].source, "listbox");
    assert_eq!(after_space[6].selection.value(), "alpha");
}

#[test]
fn select_state_records_popup_listbox_overlay_and_scroll_contract() {
    let state = Select::new("priority-select", "Priority")
        .placeholder("Choose priority")
        .open(true)
        .selected("high")
        .placement(OverlayPlacementSide::Right, OverlayPlacementAlignment::End)
        .option(ListboxOption::new("low", "Low"))
        .option(ListboxOption::new("medium", "Medium").disabled(true))
        .group(
            ListboxGroup::new("recommended", "Recommended")
                .option(ListboxOption::new("high", "High"))
                .option(ListboxOption::new("urgent", "Urgent"))
                .option(ListboxOption::new("normal", "Normal"))
                .option(ListboxOption::new("later", "Later"))
                .option(ListboxOption::new("someday", "Someday")),
        )
        .state();

    assert!(state.open());
    assert_eq!(state.open_mode(), SelectOpenMode::Controlled);
    assert_eq!(state.trigger_role(), Role::Button);
    assert_eq!(state.content_role(), Role::ListBox);
    assert!(state.trigger_selected());
    assert_eq!(state.trigger_label(), "High");
    assert_eq!(state.selected_value(), Some("high"));
    assert_eq!(state.active_value(), Some("high"));
    assert_eq!(state.placement_side(), OverlayPlacementSide::Right);
    assert_eq!(state.placement_alignment(), OverlayPlacementAlignment::End);
    assert_eq!(
        state.overlay().policy().kind(),
        OverlayLayerKind::NonModalDismissible
    );
    assert_eq!(
        state.outside_press_policy(),
        OutsidePressPolicy::DismissAndConsume
    );
    assert_eq!(
        state.initial_focus_intent(),
        &InitialFocusIntent::FirstFocusable
    );
    assert_eq!(state.focus_restore_intent(), &FocusRestoreIntent::Trigger);
    assert_eq!(state.listbox().role(), Role::ListBox);
    assert_eq!(state.listbox().selected_value(), Some("high"));
    assert!(state.scrollable_content());
    assert!(state.scroll_area().scrolls_y());
}

#[test]
fn select_state_models_disabled_empty_and_policy_overrides() {
    let state = Select::new("empty-select", "Empty")
        .placeholder("Nothing to choose")
        .default_open(true)
        .disabled(true)
        .outside_press_policy(OutsidePressPolicy::DismissAndPassThrough)
        .initial_focus_intent(InitialFocusIntent::None)
        .focus_restore_intent(FocusRestoreIntent::None)
        .small()
        .state();

    assert_eq!(state.open_mode(), SelectOpenMode::Uncontrolled);
    assert!(state.default_open());
    assert!(state.disabled());
    assert!(!state.open());
    assert_eq!(state.trigger_label(), "Nothing to choose");
    assert_eq!(state.selected_value(), None);
    assert_eq!(state.active_value(), None);
    assert!(!state.scrollable_content());
    assert_eq!(
        state.outside_press_policy(),
        OutsidePressPolicy::DismissAndPassThrough
    );
    assert_eq!(state.initial_focus_intent(), &InitialFocusIntent::None);
    assert_eq!(state.focus_restore_intent(), &FocusRestoreIntent::None);
    assert!(!state.overlay().should_render_deferred_layer());
}

#[open_gpui::test]
fn select_runtime_click_and_keyboard_selection_close_popup_and_emit_payloads(
    cx: &mut open_gpui::TestAppContext,
) {
    #[derive(Clone, Debug, PartialEq, Eq)]
    enum SelectRuntimeEvent {
        Open(bool),
        Select(SelectSelection),
    }

    struct TestView {
        events: Rc<RefCell<Vec<SelectRuntimeEvent>>>,
    }

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let open_events = self.events.clone();
            let select_events = self.events.clone();

            div().size_full().child(
                Select::new("runtime-select", "Runtime select")
                    .placeholder("Choose item")
                    .option(ListboxOption::new("alpha", "Alpha"))
                    .option(ListboxOption::new("bravo", "Bravo").disabled(true))
                    .group(
                        ListboxGroup::new("team", "Team")
                            .option(ListboxOption::new("charlie", "Charlie"))
                            .option(ListboxOption::new("delta", "Delta")),
                    )
                    .on_open_change(move |open, _, _| {
                        open_events
                            .borrow_mut()
                            .push(SelectRuntimeEvent::Open(open));
                    })
                    .on_select(move |selection, _, _| {
                        select_events
                            .borrow_mut()
                            .push(SelectRuntimeEvent::Select(selection));
                    }),
            )
        }
    }

    let events = Rc::new(RefCell::new(Vec::new()));
    let (_, cx) = cx.add_window_view(|_, _| TestView {
        events: events.clone(),
    });
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    assert!(
        cx.debug_bounds("select:runtime-select:root").is_some(),
        "select root should expose a stable debug selector"
    );

    let trigger = cx
        .debug_bounds("select:runtime-select:trigger")
        .expect("select trigger should be rendered");
    cx.simulate_click(trigger.center(), Default::default());
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });
    assert_eq!(
        events.borrow().clone(),
        vec![SelectRuntimeEvent::Open(true)]
    );
    assert!(
        cx.debug_bounds("select:Runtime select:select-content-scroll:content")
            .is_some(),
        "select content should open from the real trigger"
    );

    let disabled_bravo = cx
        .debug_bounds("listbox:runtime-select-listbox:option:bravo")
        .expect("disabled Bravo option should be rendered in the popup listbox");
    cx.simulate_click(disabled_bravo.center(), Default::default());
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });
    assert_eq!(
        events.borrow().clone(),
        vec![SelectRuntimeEvent::Open(true)],
        "disabled popup option click should not emit selection callbacks or close the popup"
    );

    let alpha = cx
        .debug_bounds("listbox:runtime-select-listbox:option:alpha")
        .expect("Alpha option should be rendered in the popup listbox");
    cx.simulate_click(alpha.center(), Default::default());
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });
    assert_eq!(
        events.borrow().clone(),
        vec![
            SelectRuntimeEvent::Open(true),
            SelectRuntimeEvent::Select(SelectSelection::new(0, "alpha", "Alpha")),
            SelectRuntimeEvent::Open(false),
        ]
    );
    assert!(
        cx.debug_bounds("select:Runtime select:select-content-scroll:content")
            .is_none(),
        "enabled popup option click should close the content"
    );

    let trigger = cx
        .debug_bounds("select:runtime-select:trigger")
        .expect("select trigger should still be rendered after selection");
    cx.simulate_click(trigger.center(), Default::default());
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });
    assert_eq!(
        events.borrow().clone(),
        vec![
            SelectRuntimeEvent::Open(true),
            SelectRuntimeEvent::Select(SelectSelection::new(0, "alpha", "Alpha")),
            SelectRuntimeEvent::Open(false),
            SelectRuntimeEvent::Open(true),
        ]
    );
    assert!(
        cx.debug_bounds("select:Runtime select:select-content-scroll:content")
            .is_some(),
        "select content should reopen from the trigger after a prior selection"
    );

    let alpha = cx
        .debug_bounds("listbox:runtime-select-listbox:option:alpha")
        .expect("Alpha option should be rendered after reopening");
    cx.simulate_mouse_down(alpha.center(), MouseButton::Left, Default::default());
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });
    assert_eq!(
        events.borrow().clone(),
        vec![
            SelectRuntimeEvent::Open(true),
            SelectRuntimeEvent::Select(SelectSelection::new(0, "alpha", "Alpha")),
            SelectRuntimeEvent::Open(false),
            SelectRuntimeEvent::Open(true),
        ],
        "mouse down should focus the option without selecting until mouse up or keyboard activation"
    );
    assert!(
        cx.debug_bounds("select:Runtime select:select-content-scroll:content")
            .is_some(),
        "mouse down focus should keep the popup open for keyboard activation"
    );

    cx.simulate_keystrokes("down");
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });
    assert_eq!(
        events.borrow().clone(),
        vec![
            SelectRuntimeEvent::Open(true),
            SelectRuntimeEvent::Select(SelectSelection::new(0, "alpha", "Alpha")),
            SelectRuntimeEvent::Open(false),
            SelectRuntimeEvent::Open(true),
        ],
        "arrow navigation in the popup listbox should not select"
    );

    cx.simulate_keystrokes("enter");
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });
    assert_eq!(
        events.borrow().clone(),
        vec![
            SelectRuntimeEvent::Open(true),
            SelectRuntimeEvent::Select(SelectSelection::new(0, "alpha", "Alpha")),
            SelectRuntimeEvent::Open(false),
            SelectRuntimeEvent::Open(true),
            SelectRuntimeEvent::Select(SelectSelection::new(2, "charlie", "Charlie")),
            SelectRuntimeEvent::Open(false),
        ]
    );
    assert!(
        cx.debug_bounds("select:Runtime select:select-content-scroll:content")
            .is_none(),
        "keyboard popup selection should close the content"
    );
}

#[test]
fn combobox_state_filters_query_without_clearing_selection() {
    let state = Combobox::new("framework-combobox", "Framework")
        .placeholder("Search frameworks")
        .open(true)
        .default_query("re")
        .selected("solid")
        .option(ComboboxOption::new("react", "React").keyword("library"))
        .option(ComboboxOption::new("solid", "Solid"))
        .option(ComboboxOption::new("ember", "Ember").disabled(true))
        .group(
            ComboboxGroup::new("meta", "Meta")
                .option(ComboboxOption::new("remix", "Remix").keyword("react"))
                .option(ComboboxOption::new("relay", "Relay").keyword("graphql")),
        )
        .state();

    assert!(state.open());
    assert_eq!(state.open_mode(), ComboboxOpenMode::Controlled);
    assert_eq!(state.input_role(), Role::EditableComboBox);
    assert_eq!(state.content_role(), Role::ListBox);
    assert_eq!(state.query(), "re");
    assert_eq!(state.total_option_count(), 5);
    assert_eq!(state.filtered_option_count(), 3);
    assert!(state.filtered());
    assert_eq!(state.selected_value(), Some("solid"));
    assert_eq!(state.active_value(), Some("react"));
    assert_eq!(state.listbox().role(), Role::ListBox);
    assert_eq!(state.listbox().selected_value(), None);
    assert_eq!(state.listbox().typeahead_query(), Some("re"));
    assert_eq!(
        state.listbox().options()[0].role(),
        Some(Role::ListBoxOption)
    );
    assert_eq!(
        state.overlay().policy().kind(),
        OverlayLayerKind::NonModalDismissible
    );
    assert_eq!(state.input().placeholder(), Some("Search frameworks"));
}

#[test]
fn combobox_state_normalizes_query_with_text_input_policy() {
    let state = Combobox::new("newline-combobox", "Framework")
        .default_query("re\r\nmix")
        .option(ComboboxOption::new("remix", "Remix"))
        .state();

    assert_eq!(state.query(), "re  mix");
    assert_eq!(state.input().value(), "re  mix");
}

#[test]
fn combobox_state_scrollable_content_tracks_filtered_option_count() {
    let scrollable = Combobox::new("scrolling-combobox", "Scrolling combobox")
        .placeholder("Search frameworks")
        .open(true)
        .option(ComboboxOption::new("react", "React").keyword("library"))
        .option(ComboboxOption::new("solid", "Solid"))
        .option(ComboboxOption::new("ember", "Ember"))
        .option(ComboboxOption::new("svelte", "Svelte"))
        .option(ComboboxOption::new("angular", "Angular"))
        .option(ComboboxOption::new("vue", "Vue"))
        .group(
            ComboboxGroup::new("meta", "Meta")
                .option(ComboboxOption::new("remix", "Remix").keyword("react")),
        )
        .state();
    let not_scrollable = Combobox::new("filtered-combobox", "Filtered combobox")
        .placeholder("Search frameworks")
        .open(true)
        .default_query("re")
        .option(ComboboxOption::new("react", "React").keyword("library"))
        .option(ComboboxOption::new("solid", "Solid"))
        .option(ComboboxOption::new("ember", "Ember"))
        .group(
            ComboboxGroup::new("meta", "Meta")
                .option(ComboboxOption::new("remix", "Remix").keyword("react"))
                .option(ComboboxOption::new("relay", "Relay").keyword("graphql")),
        )
        .state();

    assert_eq!(scrollable.total_option_count(), 7);
    assert_eq!(scrollable.filtered_option_count(), 7);
    assert!(scrollable.scrollable_content());

    assert_eq!(not_scrollable.total_option_count(), 5);
    assert_eq!(not_scrollable.filtered_option_count(), 3);
    assert!(!not_scrollable.scrollable_content());
}

#[test]
fn combobox_disabled_empty_state_blocks_popup_and_input() {
    let state = Combobox::new("empty-combobox", "Empty")
        .placeholder("Search")
        .default_open(true)
        .disabled(true)
        .default_query("zzz")
        .option(ComboboxOption::new("react", "React"))
        .empty_label("No frameworks")
        .outside_press_policy(OutsidePressPolicy::DismissAndPassThrough)
        .state();

    assert_eq!(state.open_mode(), ComboboxOpenMode::Uncontrolled);
    assert!(state.default_open());
    assert!(state.disabled());
    assert!(!state.open());
    assert_eq!(state.filtered_option_count(), 0);
    assert!(state.listbox().empty());
    assert_eq!(state.listbox().empty_label(), "No frameworks");
    assert!(!state.input().editable());
    assert_eq!(
        state.outside_press_policy(),
        OutsidePressPolicy::DismissAndPassThrough
    );
    assert!(!state.overlay().should_render_deferred_layer());
}

#[open_gpui::test]
fn combobox_runtime_filters_input_and_selects_filtered_option(cx: &mut open_gpui::TestAppContext) {
    #[derive(Clone, Debug, PartialEq, Eq)]
    enum ComboboxRuntimeEvent {
        Open(bool),
        Select(ComboboxSelection),
    }

    struct TestView {
        events: Rc<RefCell<Vec<ComboboxRuntimeEvent>>>,
    }

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let open_events = self.events.clone();
            let select_events = self.events.clone();

            div().size_full().child(
                Combobox::new("runtime-combobox", "Runtime combobox")
                    .placeholder("Search frameworks")
                    .option(ComboboxOption::new("react", "React").keyword("library"))
                    .option(ComboboxOption::new("solid", "Solid"))
                    .option(ComboboxOption::new("ember", "Ember").disabled(true))
                    .group(
                        ComboboxGroup::new("meta", "Meta")
                            .option(ComboboxOption::new("remix", "Remix").keyword("react"))
                            .option(ComboboxOption::new("relay", "Relay").keyword("graphql")),
                    )
                    .on_open_change(move |open, _, _| {
                        open_events
                            .borrow_mut()
                            .push(ComboboxRuntimeEvent::Open(open));
                    })
                    .on_select(move |selection, _, _| {
                        select_events
                            .borrow_mut()
                            .push(ComboboxRuntimeEvent::Select(selection));
                    }),
            )
        }
    }

    cx.update(init_text_input);
    let events = Rc::new(RefCell::new(Vec::new()));
    let (_, cx) = cx.add_window_view(|_, _| TestView {
        events: events.clone(),
    });
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    let input = cx
        .debug_bounds("text-input:runtime-combobox-input:root")
        .expect("combobox text input should expose a stable debug selector");
    cx.simulate_click(input.center(), Default::default());
    cx.simulate_input("re");
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    assert!(
        cx.debug_bounds("combobox:runtime-combobox:content")
            .is_none(),
        "typing text should filter input without implicitly opening the popup"
    );

    let toggle = cx
        .debug_bounds("combobox:runtime-combobox:toggle")
        .expect("combobox toggle should be rendered");
    cx.simulate_click(toggle.center(), Default::default());
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    assert_eq!(
        events.borrow().clone(),
        vec![ComboboxRuntimeEvent::Open(true)]
    );
    assert!(
        cx.debug_bounds("combobox:runtime-combobox:content")
            .is_some(),
        "toggle click should open filtered popup content"
    );
    assert!(
        cx.debug_bounds("listbox:runtime-combobox-listbox:option:react")
            .is_some(),
        "React should match query text"
    );
    assert!(
        cx.debug_bounds("listbox:runtime-combobox-listbox:option:remix")
            .is_some(),
        "Remix should match query keyword"
    );
    assert!(
        cx.debug_bounds("listbox:runtime-combobox-listbox:option:solid")
            .is_none(),
        "Solid should be filtered out by query text"
    );
    assert!(
        cx.debug_bounds("listbox:runtime-combobox-listbox:option:ember")
            .is_none(),
        "disabled Ember should still be filtered out when it does not match"
    );

    let remix = cx
        .debug_bounds("listbox:runtime-combobox-listbox:option:remix")
        .expect("filtered Remix option should be rendered");
    cx.simulate_click(remix.center(), Default::default());
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    assert_eq!(
        events.borrow().clone(),
        vec![
            ComboboxRuntimeEvent::Open(true),
            ComboboxRuntimeEvent::Select(ComboboxSelection::new("remix", "Remix")),
            ComboboxRuntimeEvent::Open(false),
        ]
    );
    assert!(
        cx.debug_bounds("combobox:runtime-combobox:content")
            .is_none(),
        "combobox selection should close popup content"
    );
}

#[open_gpui::test]
fn combobox_runtime_keyboard_selects_filtered_option(cx: &mut open_gpui::TestAppContext) {
    #[derive(Clone, Debug, PartialEq, Eq)]
    enum ComboboxRuntimeEvent {
        Open(bool),
        Select(ComboboxSelection),
    }

    struct TestView {
        events: Rc<RefCell<Vec<ComboboxRuntimeEvent>>>,
    }

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let open_events = self.events.clone();
            let select_events = self.events.clone();

            div().size_full().child(
                Combobox::new("keyboard-combobox", "Keyboard combobox")
                    .placeholder("Search frameworks")
                    .option(ComboboxOption::new("react", "React").keyword("library"))
                    .option(ComboboxOption::new("solid", "Solid"))
                    .group(
                        ComboboxGroup::new("meta", "Meta")
                            .option(ComboboxOption::new("remix", "Remix").keyword("react"))
                            .option(ComboboxOption::new("relay", "Relay").keyword("graphql")),
                    )
                    .on_open_change(move |open, _, _| {
                        open_events
                            .borrow_mut()
                            .push(ComboboxRuntimeEvent::Open(open));
                    })
                    .on_select(move |selection, _, _| {
                        select_events
                            .borrow_mut()
                            .push(ComboboxRuntimeEvent::Select(selection));
                    }),
            )
        }
    }

    cx.update(init_text_input);
    let events = Rc::new(RefCell::new(Vec::new()));
    let (_, cx) = cx.add_window_view(|_, _| TestView {
        events: events.clone(),
    });
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    let input = cx
        .debug_bounds("text-input:keyboard-combobox-input:root")
        .expect("combobox text input should expose a stable debug selector");
    cx.simulate_click(input.center(), Default::default());
    cx.simulate_input("re");
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    cx.simulate_keystrokes("down");
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    assert_eq!(
        events.borrow().clone(),
        vec![ComboboxRuntimeEvent::Open(true)]
    );
    assert!(
        cx.debug_bounds("combobox:keyboard-combobox:content")
            .is_some(),
        "down arrow should open filtered combobox content from the input row"
    );

    cx.simulate_keystrokes("enter");
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    assert_eq!(
        events.borrow().clone(),
        vec![
            ComboboxRuntimeEvent::Open(true),
            ComboboxRuntimeEvent::Select(ComboboxSelection::new("remix", "Remix")),
            ComboboxRuntimeEvent::Open(false),
        ]
    );
    assert!(
        cx.debug_bounds("combobox:keyboard-combobox:content")
            .is_none(),
        "keyboard selection should close filtered combobox content"
    );
}

#[test]
fn command_state_filters_groups_shortcuts_loading_and_dialog_policy() {
    let state = Command::new("command-palette", "Command palette")
        .placeholder("Type a command")
        .open(true)
        .default_query("file")
        .selected("new-file")
        .loading("Indexing commands", Some(45))
        .dialog("Command palette")
        .dialog_description("Run a workspace command")
        .item(CommandItem::new("open-file", "Open File").shortcut("Ctrl+O"))
        .group(
            CommandGroup::new("file", "File")
                .item(CommandItem::new("new-file", "New File").shortcut("Ctrl+N"))
                .item(CommandItem::new("close-window", "Close Window").shortcut("Alt+F4")),
        )
        .group(
            CommandGroup::new("view", "View")
                .item(CommandItem::new("toggle-sidebar", "Toggle Sidebar").keyword("layout")),
        )
        .state();

    assert!(state.open());
    assert_eq!(state.open_mode(), CommandOpenMode::Controlled);
    assert_eq!(state.input_role(), Role::TextInput);
    assert_eq!(state.list_role(), Role::ListBox);
    assert_eq!(state.query(), "file");
    assert_eq!(state.total_item_count(), 4);
    assert_eq!(state.filtered_item_count(), 2);
    assert!(state.filtered());
    assert_eq!(state.selected_value(), Some("new-file"));
    assert_eq!(state.active_value(), Some("new-file"));
    assert_eq!(state.groups().len(), 2);
    assert_eq!(state.groups()[0].label(), "Commands");
    assert_eq!(state.groups()[1].label(), "File");
    assert!(state.groups()[0].match_score() > 0);
    assert!(state.groups()[1].match_score() > 0);
    assert_eq!(state.items().len(), 2);
    assert_eq!(state.items()[1].shortcut(), Some("Ctrl+N"));
    assert!(state.items()[1].selected());
    let activation = state.activation_for_key("enter").unwrap();
    assert_eq!(activation.value(), "new-file");
    assert_eq!(activation.shortcut(), Some("Ctrl+N"));
    assert!(state.loading().is_some());
    assert_eq!(state.loading().unwrap().role(), Role::ProgressIndicator);
    assert_eq!(state.loading().unwrap().progress_percent(), Some(45));
    assert!(state.scroll_area().scrolls_y());
    assert_eq!(
        state.scroll_area().reset_policy(),
        ScrollResetPolicy::Preserve
    );
    assert_eq!(
        state.overlay().policy().kind(),
        OverlayLayerKind::NonModalDismissible
    );
    let dialog = state.dialog().unwrap();
    assert!(dialog.open());
    assert_eq!(dialog.content_role(), Role::Window);
    assert_eq!(dialog.overlay().policy().kind(), OverlayLayerKind::Modal);
    assert_eq!(dialog.description(), Some("Run a workspace command"));
}

#[test]
fn command_state_reports_match_sources_for_label_value_keyword_and_shortcut() {
    let label_state = Command::new("label-command", "Commands")
        .default_query("open")
        .item(CommandItem::new("open-file", "Open File"))
        .state();
    assert_eq!(label_state.items()[0].value(), "open-file");
    assert_eq!(
        label_state.items()[0].match_source(),
        Some(CommandMatchSource::Label)
    );

    let value_state = Command::new("value-command", "Commands")
        .default_query("open-file")
        .item(CommandItem::new("open-file", "Open File"))
        .state();
    assert_eq!(
        value_state.items()[0].match_source(),
        Some(CommandMatchSource::Value)
    );

    let keyword_state = Command::new("keyword-command", "Commands")
        .default_query("prefs")
        .item(CommandItem::new("settings", "Settings").keyword("prefs"))
        .state();
    assert_eq!(keyword_state.items()[0].value(), "settings");
    assert_eq!(
        keyword_state.items()[0].match_source(),
        Some(CommandMatchSource::Keyword)
    );

    let shortcut_state = Command::new("shortcut-command", "Commands")
        .default_query("ctrl+p")
        .item(CommandItem::new("palette", "Command Palette").shortcut("Ctrl+P"))
        .state();
    assert_eq!(shortcut_state.items()[0].value(), "palette");
    assert_eq!(
        shortcut_state.items()[0].match_source(),
        Some(CommandMatchSource::Shortcut)
    );
}

#[test]
fn command_state_empty_query_preserves_caller_order() {
    let state = Command::new("ordered-command", "Commands")
        .item(CommandItem::new("root-two", "Root Two"))
        .item(CommandItem::new("root-one", "Root One"))
        .group(
            CommandGroup::new("group", "Group")
                .item(CommandItem::new("group-two", "Group Two"))
                .item(CommandItem::new("group-one", "Group One")),
        )
        .state();
    let values = state
        .items()
        .iter()
        .map(|item| item.value().to_owned())
        .collect::<Vec<_>>();

    assert_eq!(
        values,
        vec![
            "root-two".to_string(),
            "root-one".to_string(),
            "group-two".to_string(),
            "group-one".to_string(),
        ]
    );
    assert!(state
        .items()
        .iter()
        .all(|item| item.match_source().is_none() && item.match_score() == 0));
    assert!(state.groups().iter().all(|group| group.match_score() == 0));
}

#[test]
fn command_state_ranks_label_and_value_matches_before_keyword_only_matches() {
    let state = Command::new("ranked-command", "Commands")
        .default_query("file")
        .item(CommandItem::new("archive", "Archive").keyword("file"))
        .item(CommandItem::new("open-file", "Open File"))
        .item(CommandItem::new("file-action", "Launcher"))
        .item(CommandItem::new("bulk-action", "Bulk Action").keyword("file"))
        .state();
    let values = state
        .items()
        .iter()
        .map(|item| item.value().to_owned())
        .collect::<Vec<_>>();

    assert_eq!(
        values,
        vec![
            "open-file".to_string(),
            "file-action".to_string(),
            "archive".to_string(),
            "bulk-action".to_string(),
        ]
    );
    assert_eq!(
        state.items()[0].match_source(),
        Some(CommandMatchSource::Label)
    );
    assert_eq!(
        state.items()[1].match_source(),
        Some(CommandMatchSource::Value)
    );
    assert_eq!(
        state.items()[2].match_source(),
        Some(CommandMatchSource::Keyword)
    );
    assert!(state.items()[1].match_score() > state.items()[2].match_score());
}

#[test]
fn command_state_tracks_active_and_selected_by_value_after_reorder() {
    let first = Command::new("first-command", "Commands")
        .selected("target")
        .active("target")
        .item(CommandItem::new("other", "Other"))
        .item(CommandItem::new("target", "Target"))
        .state();
    let reordered = Command::new("reordered-command", "Commands")
        .selected("target")
        .active("target")
        .item(CommandItem::new("target", "Target"))
        .item(CommandItem::new("other", "Other"))
        .state();

    assert_eq!(first.selected_value(), Some("target"));
    assert_eq!(first.active_value(), Some("target"));
    assert!(first.items()[1].selected());
    assert!(first.items()[1].active());
    assert_eq!(reordered.selected_value(), Some("target"));
    assert_eq!(reordered.active_value(), Some("target"));
    assert!(reordered.items()[0].selected());
    assert!(reordered.items()[0].active());
}

#[test]
fn command_state_keeps_disabled_matches_visible_but_non_activatable() {
    let state = Command::new("disabled-command", "Commands")
        .default_query("delete")
        .selected("delete-project")
        .active("delete-project")
        .item(CommandItem::new("delete-project", "Delete Project").disabled(true))
        .state();

    assert_eq!(state.filtered_item_count(), 1);
    assert_eq!(state.items()[0].value(), "delete-project");
    assert!(state.items()[0].disabled());
    assert_eq!(state.selected_value(), None);
    assert_eq!(state.active_value(), None);
    assert_eq!(state.activation_for_key("enter"), None);
}

#[test]
fn command_state_models_controlled_and_default_query_ownership() {
    let controlled = Command::new("controlled-query-command", "Commands")
        .query("open\r\n")
        .default_query("ignored")
        .item(CommandItem::new("open-file", "Open File"))
        .state();
    let seeded = Command::new("seeded-query-command", "Commands")
        .default_query("new\n")
        .item(CommandItem::new("new-file", "New File"))
        .item(CommandItem::new("open-file", "Open File"))
        .state();

    assert_eq!(controlled.query(), "open  ");
    assert_eq!(controlled.input().value(), "open  ");
    assert_eq!(controlled.query_mode(), CommandQueryMode::Controlled);
    assert_eq!(controlled.filtered_item_count(), 1);
    assert_eq!(seeded.query(), "new ");
    assert_eq!(seeded.input().value(), "new ");
    assert_eq!(seeded.query_mode(), CommandQueryMode::Uncontrolled);
}

#[test]
fn command_state_models_multi_selected_values_and_hidden_chips() {
    let state = Command::new("multi-command", "Commands")
        .default_query("new")
        .multi_select(true)
        .selected_values(["open-file", "new-file", "missing", "delete-file"])
        .item(CommandItem::new("open-file", "Open File"))
        .item(CommandItem::new("delete-file", "Delete File").disabled(true))
        .group(CommandGroup::new("file", "File").item(CommandItem::new("new-file", "New File")))
        .state();

    assert_eq!(state.selection_mode(), CommandSelectionMode::Multiple);
    assert_eq!(
        state.selected_values(),
        &["open-file".to_string(), "new-file".to_string()]
    );
    assert_eq!(state.selected_value(), None);
    assert_eq!(state.filtered_item_count(), 1);
    assert_eq!(
        state
            .selected_chips()
            .iter()
            .map(|chip| chip.value().to_owned())
            .collect::<Vec<_>>(),
        vec!["open-file".to_string(), "new-file".to_string()]
    );
    assert_eq!(state.selected_chips()[0].label(), "Open File");
    assert!(state.items()[0].selected());
}

#[test]
fn command_index_snapshot_matches_equivalent_local_descriptors() {
    let snapshot = CommandIndexSnapshot::new("commands-v1")
        .item(CommandItemDescriptor::new("open-file", "Open File").shortcut("Ctrl+O"))
        .group(
            CommandGroupDescriptor::new("file", "File")
                .item(CommandItemDescriptor::new("new-file", "New File").shortcut("Ctrl+N"))
                .item(
                    CommandItemDescriptor::new("close-window", "Close Window").shortcut("Alt+F4"),
                ),
        );
    let local = Command::new("local-command", "Commands")
        .default_query("file")
        .selected("new-file")
        .active("new-file")
        .item(CommandItem::new("open-file", "Open File").shortcut("Ctrl+O"))
        .group(
            CommandGroup::new("file", "File")
                .item(CommandItem::new("new-file", "New File").shortcut("Ctrl+N"))
                .item(CommandItem::new("close-window", "Close Window").shortcut("Alt+F4")),
        )
        .state();
    let indexed = Command::new("indexed-command", "Commands")
        .default_query("file")
        .selected("new-file")
        .active("new-file")
        .index_snapshot(snapshot)
        .state();

    assert_eq!(indexed.index_revision(), Some("commands-v1"));
    assert_eq!(indexed.index_mode(), CommandIndexSnapshotMode::LocalRanked);
    assert_eq!(indexed.total_item_count(), local.total_item_count());
    assert_eq!(indexed.filtered_item_count(), local.filtered_item_count());
    assert_eq!(
        indexed
            .items()
            .iter()
            .map(|item| (
                item.value().to_owned(),
                item.label().to_owned(),
                item.match_source(),
                item.match_score(),
                item.selected(),
                item.active(),
            ))
            .collect::<Vec<_>>(),
        local
            .items()
            .iter()
            .map(|item| (
                item.value().to_owned(),
                item.label().to_owned(),
                item.match_source(),
                item.match_score(),
                item.selected(),
                item.active(),
            ))
            .collect::<Vec<_>>()
    );
}

#[test]
fn command_index_snapshot_revision_preserves_selection_by_value_after_reorder() {
    let first = CommandIndexSnapshot::new("commands-v1")
        .item(CommandItemDescriptor::new("other", "Other"))
        .item(CommandItemDescriptor::new("target", "Target"));
    let second = CommandIndexSnapshot::new("commands-v2")
        .item(CommandItemDescriptor::new("target", "Target"))
        .item(CommandItemDescriptor::new("other", "Other"));
    let first_state = Command::new("snapshot-revision-command", "Commands")
        .selected("target")
        .active("target")
        .index_snapshot(first)
        .state();
    let second_state = Command::new("snapshot-revision-command", "Commands")
        .selected("target")
        .active("target")
        .index_snapshot(second)
        .state();

    assert_eq!(first_state.index_revision(), Some("commands-v1"));
    assert_eq!(second_state.index_revision(), Some("commands-v2"));
    assert_eq!(first_state.items()[1].value(), "target");
    assert!(first_state.items()[1].selected());
    assert!(first_state.items()[1].active());
    assert_eq!(second_state.items()[0].value(), "target");
    assert!(second_state.items()[0].selected());
    assert!(second_state.items()[0].active());
}

#[test]
fn command_index_snapshot_modes_preserve_pre_ranked_and_pre_filtered_order() {
    let pre_ranked = CommandIndexSnapshot::new("pre-ranked")
        .mode(CommandIndexSnapshotMode::PreRankedFilter)
        .item(CommandItemDescriptor::new("archive", "Archive").keyword("file"))
        .item(CommandItemDescriptor::new("open-file", "Open File"))
        .item(CommandItemDescriptor::new("file-action", "Launcher"))
        .item(CommandItemDescriptor::new("bulk-action", "Bulk Action").keyword("file"));
    let pre_filtered = CommandIndexSnapshot::new("pre-filtered")
        .mode(CommandIndexSnapshotMode::PreFiltered)
        .item(CommandItemDescriptor::new("archive", "Archive").keyword("file"))
        .item(CommandItemDescriptor::new("unmatched", "Unmatched"));

    let pre_ranked_state = Command::new("pre-ranked-command", "Commands")
        .query("file")
        .index_snapshot(pre_ranked)
        .state();
    let pre_filtered_state = Command::new("pre-filtered-command", "Commands")
        .query("file")
        .index_snapshot(pre_filtered)
        .state();

    assert_eq!(
        pre_ranked_state
            .items()
            .iter()
            .map(|item| item.value().to_owned())
            .collect::<Vec<_>>(),
        vec![
            "archive".to_string(),
            "open-file".to_string(),
            "file-action".to_string(),
            "bulk-action".to_string(),
        ]
    );
    assert_eq!(
        pre_ranked_state
            .items()
            .iter()
            .map(|item| item.match_source())
            .collect::<Vec<_>>(),
        vec![
            Some(CommandMatchSource::Keyword),
            Some(CommandMatchSource::Label),
            Some(CommandMatchSource::Value),
            Some(CommandMatchSource::Keyword),
        ]
    );
    assert_eq!(pre_filtered_state.filtered_item_count(), 2);
    assert_eq!(
        pre_filtered_state
            .items()
            .iter()
            .map(|item| (
                item.value().to_owned(),
                item.match_source(),
                item.match_score()
            ))
            .collect::<Vec<_>>(),
        vec![
            ("archive".to_string(), None, 0),
            ("unmatched".to_string(), None, 0),
        ]
    );
}

#[test]
fn command_index_snapshot_loading_coexists_with_visible_and_empty_results() {
    let visible = CommandIndexSnapshot::new("loading-visible")
        .mode(CommandIndexSnapshotMode::PreFiltered)
        .loading(CommandLoadingState::new(
            "Refreshing command index",
            Some(30),
        ))
        .item(CommandItemDescriptor::new(
            "stale-open",
            "Open from stale index",
        ));
    let empty = CommandIndexSnapshot::new("loading-empty")
        .loading(CommandLoadingState::new("Indexing commands", None));

    let visible_state = Command::new("snapshot-loading-visible", "Commands")
        .query("anything")
        .loading("Builder loading is overridden", Some(99))
        .index_snapshot(visible)
        .state();
    let empty_state = Command::new("snapshot-loading-empty", "Commands")
        .query("anything")
        .index_snapshot(empty)
        .state();

    assert_eq!(visible_state.filtered_item_count(), 1);
    assert_eq!(
        visible_state.loading().map(CommandLoadingState::message),
        Some("Refreshing command index")
    );
    assert_eq!(
        visible_state
            .loading()
            .and_then(CommandLoadingState::progress_percent),
        Some(30)
    );
    assert!(empty_state.empty());
    assert_eq!(
        empty_state.loading().map(CommandLoadingState::message),
        Some("Indexing commands")
    );
    assert_eq!(empty_state.loading().unwrap().progress_percent(), None);
}

#[test]
fn command_behavior_snapshot_virtualizes_large_result_sets_with_stable_rows() {
    let command =
        Command::new("large-command", "Commands")
            .with_size(Size::Small)
            .row_height(ui_px(28.0))
            .overscan(4)
            .active("item-0104")
            .selected("item-0101")
            .items((0..10_000).map(|index| {
                CommandItem::new(format!("item-{index:04}"), format!("Item {index:04}"))
            }));
    let snapshot = command.behavior_snapshot_with_viewport(ui_px(2_800.0), ui_px(196.0));

    assert_eq!(snapshot.role(), Role::ListBox);
    assert_eq!(snapshot.row_role(), Role::ListBoxOption);
    assert_eq!(snapshot.state().total_item_count(), 10_000);
    assert_eq!(snapshot.state().filtered_item_count(), 10_000);
    assert_eq!(*snapshot.visible_range(), VirtualizerRange::new(100, 107));
    assert_eq!(*snapshot.overscan_range(), VirtualizerRange::new(98, 109));
    assert_eq!(snapshot.visible_row_count(), 7);
    assert_eq!(snapshot.rendered_row_count(), 11);
    assert_eq!(snapshot.rows()[0].index(), 98);
    assert_eq!(snapshot.rows()[0].render_key(), "item-0098");

    let active = snapshot
        .active_row()
        .expect("active command row should render");
    assert_eq!(active.index(), 104);
    assert_eq!(active.value(), "item-0104");
    assert!(active.active());
    assert_eq!(active.virtual_start(), ui_px(2_912.0));
    assert_eq!(active.virtual_size(), ui_px(28.0));
    assert_eq!(
        snapshot
            .selected_rows()
            .map(|row| row.value().to_owned())
            .collect::<Vec<_>>(),
        vec!["item-0101".to_string()]
    );

    let scrolled = command.behavior_snapshot_with_viewport(ui_px(5_600.0), ui_px(196.0));
    assert_eq!(*scrolled.visible_range(), VirtualizerRange::new(200, 207));
    assert_eq!(scrolled.rows()[0].value(), "item-0198");
}

#[test]
fn command_behavior_snapshot_clamps_filtered_scroll_and_disambiguates_duplicate_values() {
    let duplicate_snapshot = Command::new("duplicate-command", "Commands")
        .row_height(ui_px(28.0))
        .item(CommandItem::new("duplicate", "Open File"))
        .item(CommandItem::new("duplicate", "Open Recent"))
        .item(CommandItem::new("unique", "Close File"))
        .behavior_snapshot_with_viewport(ui_px(0.0), ui_px(112.0));

    assert_eq!(
        duplicate_snapshot
            .rows()
            .iter()
            .map(|row| (row.value().to_owned(), row.render_key().to_owned()))
            .collect::<Vec<_>>(),
        vec![
            ("duplicate".to_string(), "0:duplicate".to_string()),
            ("duplicate".to_string(), "1:duplicate".to_string()),
            ("unique".to_string(), "unique".to_string()),
        ]
    );

    let filtered =
        Command::new("filtered-command", "Commands")
            .default_query("item 0001")
            .row_height(ui_px(28.0))
            .items((0..10_000).map(|index| {
                CommandItem::new(format!("item-{index:04}"), format!("Item {index:04}"))
            }))
            .behavior_snapshot_with_viewport(ui_px(80_000.0), ui_px(112.0));

    assert_eq!(filtered.state().filtered_item_count(), 1);
    assert_eq!(filtered.scroll_offset(), ui_px(0.0));
    assert_eq!(filtered.rows()[0].value(), "item-0001");
}

#[test]
fn command_multi_selection_change_toggles_values_without_duplicates() {
    let add = CommandSelectionChange::new(
        vec!["open-file".to_string(), "new-file".to_string()],
        CommandSelection::new(1, "new-file", "New File", None),
        true,
    );
    let remove = CommandSelectionChange::new(
        vec!["open-file".to_string()],
        CommandSelection::new(1, "new-file", "New File", None),
        false,
    );

    assert_eq!(
        add.values(),
        &["open-file".to_string(), "new-file".to_string()]
    );
    assert!(add.selected());
    assert_eq!(add.toggled().value(), "new-file");
    assert_eq!(remove.values(), &["open-file".to_string()]);
    assert!(!remove.selected());
}

#[test]
fn command_state_models_empty_disabled_and_escape_policy() {
    let state = Command::new("empty-command", "Commands")
        .default_open(true)
        .disabled(true)
        .default_query("missing")
        .item(CommandItem::new("open", "Open"))
        .escape_key_policy(EscapeKeyPolicy::Ignore)
        .focus_restore_intent(FocusRestoreIntent::None)
        .state();

    assert_eq!(state.open_mode(), CommandOpenMode::Uncontrolled);
    assert!(state.default_open());
    assert!(state.disabled());
    assert!(!state.open());
    assert_eq!(state.filtered_item_count(), 0);
    assert!(state.listbox().empty());
    assert!(!state.input().editable());
    assert_eq!(state.escape_key_policy(), EscapeKeyPolicy::Ignore);
    assert_eq!(
        state.overlay().policy().escape_key_policy(),
        EscapeKeyPolicy::Ignore
    );
    assert_eq!(state.focus_restore_intent(), &FocusRestoreIntent::None);
    assert!(!state.overlay().should_render_deferred_layer());
}

#[open_gpui::test]
fn command_runtime_filters_input_and_selects_with_keyboard(cx: &mut open_gpui::TestAppContext) {
    struct TestView {
        selections: Rc<RefCell<Vec<CommandSelection>>>,
    }

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let selections = self.selections.clone();

            div().size_full().child(
                Command::new("runtime-command", "Runtime command")
                    .placeholder("Type a command")
                    .item(CommandItem::new("open-file", "Open File").shortcut("Ctrl+O"))
                    .group(
                        CommandGroup::new("file", "File")
                            .item(CommandItem::new("new-file", "New File").shortcut("Ctrl+N"))
                            .item(
                                CommandItem::new("close-window", "Close Window").shortcut("Alt+F4"),
                            ),
                    )
                    .group(CommandGroup::new("view", "View").item(
                        CommandItem::new("toggle-sidebar", "Toggle Sidebar").keyword("layout"),
                    ))
                    .on_select(move |selection, _, _| {
                        selections.borrow_mut().push(selection);
                    }),
            )
        }
    }

    cx.update(init_text_input);
    let selections = Rc::new(RefCell::new(Vec::new()));
    let (_, cx) = cx.add_window_view(|_, _| TestView {
        selections: selections.clone(),
    });
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    assert!(
        cx.debug_bounds("command:runtime-command:content").is_some(),
        "inline command content should render immediately"
    );
    let input = cx
        .debug_bounds("text-input:runtime-command-input:root")
        .expect("command text input should expose a stable debug selector");
    cx.simulate_click(input.center(), Default::default());
    cx.simulate_input("file");
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    assert!(
        cx.debug_bounds("listbox:runtime-command-listbox:option:open-file")
            .is_some(),
        "Open File should match query text"
    );
    assert!(
        cx.debug_bounds("listbox:runtime-command-listbox:option:new-file")
            .is_some(),
        "New File should match query text"
    );
    assert!(
        cx.debug_bounds("listbox:runtime-command-listbox:option:toggle-sidebar")
            .is_none(),
        "Toggle Sidebar should be filtered out before keyboard activation"
    );

    cx.simulate_keystrokes("down");
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    assert!(
        selections.borrow().is_empty(),
        "arrow navigation should move active command without selecting"
    );

    cx.simulate_keystrokes("enter");
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    assert_eq!(
        selections.borrow().clone(),
        vec![CommandSelection::new(
            1,
            "new-file",
            "New File",
            Some("Ctrl+N".to_string())
        )]
    );
    assert!(
        cx.debug_bounds("command:runtime-command:content").is_some(),
        "inline command selection should not close non-dialog content"
    );
}

#[open_gpui::test]
fn command_runtime_controlled_query_emits_sanitized_query_changes(
    cx: &mut open_gpui::TestAppContext,
) {
    struct TestView {
        query: Rc<RefCell<String>>,
        changes: Rc<RefCell<Vec<String>>>,
    }

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let query = self.query.borrow().clone();
            let next_query = self.query.clone();
            let changes = self.changes.clone();

            div().size_full().child(
                Command::new("controlled-query-runtime-command", "Runtime command")
                    .query(query)
                    .placeholder("Type a command")
                    .item(CommandItem::new("open-file", "Open File"))
                    .item(CommandItem::new("close-window", "Close Window"))
                    .on_query_change(move |query, _, _| {
                        *next_query.borrow_mut() = query.clone();
                        changes.borrow_mut().push(query);
                    }),
            )
        }
    }

    cx.update(init_text_input);
    let query = Rc::new(RefCell::new(String::new()));
    let changes = Rc::new(RefCell::new(Vec::new()));
    let (_, cx) = cx.add_window_view(|_, _| TestView {
        query: query.clone(),
        changes: changes.clone(),
    });
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    let input = cx
        .debug_bounds("text-input:controlled-query-runtime-command-input:root")
        .expect("controlled command input should expose a stable debug selector");
    cx.simulate_click(input.center(), Default::default());
    cx.simulate_input("open\nfile");
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    assert_eq!(query.borrow().as_str(), "open file");
    assert_eq!(
        changes.borrow().last().map(String::as_str),
        Some("open file")
    );
    assert!(
        cx.debug_bounds("listbox:controlled-query-runtime-command-listbox:option:open-file")
            .is_some(),
        "controlled query should feed filtered command rows after caller feedback"
    );
}

#[open_gpui::test]
fn command_runtime_dialog_selects_and_dismisses_without_stale_modal_layer(
    cx: &mut open_gpui::TestAppContext,
) {
    #[derive(Clone, Debug, PartialEq, Eq)]
    enum CommandDialogRuntimeEvent {
        Open(bool),
        Select(CommandSelection),
    }

    struct TestView {
        events: Rc<RefCell<Vec<CommandDialogRuntimeEvent>>>,
    }

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let open_events = self.events.clone();
            let select_events = self.events.clone();

            div().size_full().child(
                Command::new("dialog-runtime-command", "Dialog runtime command")
                    .dialog("Command palette")
                    .trigger_label("Open command")
                    .placeholder("Type a command")
                    .item(CommandItem::new("open-file", "Open File").shortcut("Ctrl+O"))
                    .group(
                        CommandGroup::new("file", "File")
                            .item(CommandItem::new("new-file", "New File").shortcut("Ctrl+N"))
                            .item(
                                CommandItem::new("close-window", "Close Window").shortcut("Alt+F4"),
                            ),
                    )
                    .group(CommandGroup::new("view", "View").item(
                        CommandItem::new("toggle-sidebar", "Toggle Sidebar").keyword("layout"),
                    ))
                    .on_open_change(move |open, _, _| {
                        open_events
                            .borrow_mut()
                            .push(CommandDialogRuntimeEvent::Open(open));
                    })
                    .on_select(move |selection, _, _| {
                        select_events
                            .borrow_mut()
                            .push(CommandDialogRuntimeEvent::Select(selection));
                    }),
            )
        }
    }

    cx.update(init_text_input);
    let events = Rc::new(RefCell::new(Vec::new()));
    let (_, cx) = cx.add_window_view(|_, _| TestView {
        events: events.clone(),
    });
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    assert!(
        cx.debug_bounds("command:dialog-runtime-command:content")
            .is_none(),
        "dialog command content should start closed"
    );

    let trigger = cx
        .debug_bounds("command:dialog-runtime-command:trigger")
        .expect("dialog command trigger should expose a stable debug selector");
    cx.simulate_click(trigger.center(), Default::default());
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    assert_eq!(
        events.borrow().clone(),
        vec![CommandDialogRuntimeEvent::Open(true)]
    );
    assert!(
        cx.debug_bounds("command:dialog-runtime-command:content")
            .is_some(),
        "trigger click should open dialog command content"
    );

    let input = cx
        .debug_bounds("text-input:dialog-runtime-command-input:root")
        .expect("dialog command text input should expose a stable debug selector");
    cx.simulate_click(input.center(), Default::default());
    cx.simulate_input("file");
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    assert!(
        cx.debug_bounds("listbox:dialog-runtime-command-listbox:option:open-file")
            .is_some(),
        "Open File should match query text in dialog mode"
    );
    assert!(
        cx.debug_bounds("listbox:dialog-runtime-command-listbox:option:new-file")
            .is_some(),
        "New File should match query text in dialog mode"
    );
    assert!(
        cx.debug_bounds("listbox:dialog-runtime-command-listbox:option:toggle-sidebar")
            .is_none(),
        "unmatched command rows should be filtered out in dialog mode"
    );

    cx.simulate_keystrokes("down");
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });
    assert_eq!(
        events.borrow().clone(),
        vec![CommandDialogRuntimeEvent::Open(true)],
        "arrow navigation should move the active command without selecting"
    );

    cx.simulate_keystrokes("enter");
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    assert_eq!(
        events.borrow().clone(),
        vec![
            CommandDialogRuntimeEvent::Open(true),
            CommandDialogRuntimeEvent::Select(CommandSelection::new(
                1,
                "new-file",
                "New File",
                Some("Ctrl+N".to_string()),
            )),
            CommandDialogRuntimeEvent::Open(false),
        ]
    );
    assert!(
        cx.debug_bounds("command:dialog-runtime-command:content")
            .is_none(),
        "dialog command selection should close the modal content"
    );

    let trigger = cx
        .debug_bounds("command:dialog-runtime-command:trigger")
        .expect("dialog command trigger should remain rendered after selection");
    cx.simulate_click(trigger.center(), Default::default());
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });
    let input = cx
        .debug_bounds("text-input:dialog-runtime-command-input:root")
        .expect("dialog command input should render after reopening");
    cx.simulate_click(input.center(), Default::default());
    cx.simulate_keystrokes("escape");
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    assert_eq!(
        events.borrow().clone(),
        vec![
            CommandDialogRuntimeEvent::Open(true),
            CommandDialogRuntimeEvent::Select(CommandSelection::new(
                1,
                "new-file",
                "New File",
                Some("Ctrl+N".to_string()),
            )),
            CommandDialogRuntimeEvent::Open(false),
            CommandDialogRuntimeEvent::Open(true),
            CommandDialogRuntimeEvent::Open(false),
        ],
        "escape should close a reopened dialog exactly once"
    );
    assert!(
        cx.debug_bounds("command:dialog-runtime-command:content")
            .is_none(),
        "escape should remove the dialog content"
    );

    let trigger = cx
        .debug_bounds("command:dialog-runtime-command:trigger")
        .expect("dialog command trigger should remain rendered after escape");
    cx.simulate_click(trigger.center(), Default::default());
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });
    cx.simulate_click(point(px(4.0), px(4.0)), Default::default());
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    assert_eq!(
        events.borrow().clone(),
        vec![
            CommandDialogRuntimeEvent::Open(true),
            CommandDialogRuntimeEvent::Select(CommandSelection::new(
                1,
                "new-file",
                "New File",
                Some("Ctrl+N".to_string()),
            )),
            CommandDialogRuntimeEvent::Open(false),
            CommandDialogRuntimeEvent::Open(true),
            CommandDialogRuntimeEvent::Open(false),
            CommandDialogRuntimeEvent::Open(true),
            CommandDialogRuntimeEvent::Open(false),
        ],
        "outside press should close a reopened dialog exactly once"
    );
    assert!(
        cx.debug_bounds("command:dialog-runtime-command:content")
            .is_none(),
        "outside press should remove the dialog content"
    );
}

#[open_gpui::test]
fn command_runtime_multi_select_toggles_chips_without_closing_dialog(
    cx: &mut open_gpui::TestAppContext,
) {
    struct TestView {
        selected_values: Rc<RefCell<Vec<String>>>,
        changes: Rc<RefCell<Vec<CommandSelectionChange>>>,
    }

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let selected_values = self.selected_values.borrow().clone();
            let next_values = self.selected_values.clone();
            let changes = self.changes.clone();

            div().size_full().child(
                Command::new("multi-runtime-command", "Runtime command")
                    .dialog("Command palette")
                    .trigger_label("Open command")
                    .multi_select(true)
                    .selected_values(selected_values)
                    .item(CommandItem::new("open-file", "Open File"))
                    .item(CommandItem::new("new-file", "New File"))
                    .item(CommandItem::new("delete-file", "Delete File").disabled(true))
                    .on_selected_values_change(move |change, _, _| {
                        *next_values.borrow_mut() = change.values().to_vec();
                        changes.borrow_mut().push(change);
                    }),
            )
        }
    }

    cx.update(init_text_input);
    let selected_values = Rc::new(RefCell::new(vec!["open-file".to_string()]));
    let changes = Rc::new(RefCell::new(Vec::new()));
    let (_, cx) = cx.add_window_view(|_, _| TestView {
        selected_values: selected_values.clone(),
        changes: changes.clone(),
    });
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    let trigger = cx
        .debug_bounds("command:multi-runtime-command:trigger")
        .expect("multi command trigger should expose a stable debug selector");
    cx.simulate_click(trigger.center(), Default::default());
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    assert!(
        cx.debug_bounds("command:multi-runtime-command:selected-chip:open-file")
            .is_some(),
        "initial selected value should render as a chip"
    );

    let new_file = cx
        .debug_bounds("listbox:multi-runtime-command-listbox:option:new-file")
        .expect("New File option should render");
    cx.simulate_click(new_file.center(), Default::default());
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    assert!(
        cx.debug_bounds("command:multi-runtime-command:content")
            .is_some(),
        "multi-select activation should not close dialog content"
    );
    assert_eq!(
        selected_values.borrow().as_slice(),
        &["open-file".to_string(), "new-file".to_string()]
    );
    assert_eq!(changes.borrow().len(), 1);
    assert!(changes.borrow()[0].selected());
    assert_eq!(changes.borrow()[0].toggled().value(), "new-file");
    assert!(
        cx.debug_bounds("command:multi-runtime-command:selected-chip:new-file")
            .is_some(),
        "newly selected value should render as a chip after controlled feedback"
    );

    let disabled = cx
        .debug_bounds("listbox:multi-runtime-command-listbox:option:delete-file")
        .expect("disabled matching option should still render");
    cx.simulate_click(disabled.center(), Default::default());
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    assert_eq!(
        selected_values.borrow().as_slice(),
        &["open-file".to_string(), "new-file".to_string()],
        "disabled command should not alter the multi-selection set"
    );
    assert_eq!(changes.borrow().len(), 1);

    let open_file = cx
        .debug_bounds("listbox:multi-runtime-command-listbox:option:open-file")
        .expect("Open File option should render");
    cx.simulate_click(open_file.center(), Default::default());
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    assert_eq!(
        selected_values.borrow().as_slice(),
        &["new-file".to_string()]
    );
    assert_eq!(changes.borrow().len(), 2);
    assert!(!changes.borrow()[1].selected());
    assert_eq!(changes.borrow()[1].toggled().value(), "open-file");
}

#[open_gpui::test]
fn command_runtime_virtualized_results_scroll_inside_viewport_and_reveal_keyboard_targets(
    cx: &mut open_gpui::TestAppContext,
) {
    struct TestView {
        selections: Rc<RefCell<Vec<CommandSelection>>>,
    }

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let selections = self.selections.clone();
            let items = (0..120).map(|index| {
                CommandItem::new(format!("item-{index:04}"), format!("Item {index:04}"))
            });

            div().size_full().child(
                div().w(px(340.0)).h(px(420.0)).child(
                    ScrollArea::new(
                        "command-parent-scroll",
                        div()
                            .flex()
                            .flex_col()
                            .child(
                                div()
                                    .debug_selector(|| "command-parent-top".into())
                                    .h(px(48.0))
                                    .child("Parent top"),
                            )
                            .child(
                                div()
                                    .debug_selector(|| "command-wrapper".into())
                                    .h(px(300.0))
                                    .min_h(px(0.0))
                                    .overflow_hidden()
                                    .child(
                                        Command::new("virtualized-runtime-command", "Commands")
                                            .with_size(Size::Small)
                                            .row_height(ui_px(28.0))
                                            .overscan(2)
                                            .viewport_item_count(4)
                                            .items(items)
                                            .on_select(move |selection, _, _| {
                                                selections.borrow_mut().push(selection);
                                            }),
                                    ),
                            )
                            .child(
                                div()
                                    .debug_selector(|| "command-parent-bottom".into())
                                    .h(px(240.0))
                                    .child("Parent bottom"),
                            ),
                    )
                    .vertical(),
                ),
            )
        }
    }

    cx.update(init_text_input);
    let selections = Rc::new(RefCell::new(Vec::new()));
    let (_, cx) = cx.add_window_view(|_, _| TestView {
        selections: selections.clone(),
    });
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    assert!(
        cx.debug_bounds("command:virtualized-runtime-command:row:item-0000")
            .is_some(),
        "initial command row should render"
    );
    assert!(
        cx.debug_bounds("command:virtualized-runtime-command:row:item-0010")
            .is_none(),
        "row 10 should stay outside the initial virtual window"
    );
    let parent_bottom_before = cx
        .debug_bounds("command-parent-bottom")
        .expect("parent bottom should render before command scrolling");
    let viewport = cx
        .debug_bounds("scroll-area:Commands:command-list-scroll")
        .expect("command result viewport should expose a stable scroll selector");

    cx.simulate_event(ScrollWheelEvent {
        position: viewport.center(),
        delta: ScrollDelta::Pixels(point(px(0.0), px(-240.0))),
        ..Default::default()
    });
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    let parent_bottom_after = cx
        .debug_bounds("command-parent-bottom")
        .expect("parent bottom should remain rendered after command scrolling");
    assert_eq!(
        parent_bottom_after.top(),
        parent_bottom_before.top(),
        "expected wheel input inside Command to stay inside the command viewport"
    );
    assert!(
        cx.debug_bounds("command:virtualized-runtime-command:row:item-0000")
            .is_none(),
        "row 0 should unmount after internal command scroll"
    );
    assert!(
        cx.debug_bounds("command:virtualized-runtime-command:row:item-0010")
            .is_some(),
        "row 10 should render after internal command scroll"
    );

    let input = cx
        .debug_bounds("text-input:virtualized-runtime-command-input:root")
        .expect("virtualized command input should render");
    cx.simulate_click(input.center(), Default::default());
    cx.simulate_keystrokes("pagedown");
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });
    assert!(
        cx.debug_bounds("command:virtualized-runtime-command:row:item-0007")
            .is_some(),
        "PageDown should reveal the newly active command row"
    );

    cx.simulate_keystrokes("enter");
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    assert_eq!(
        selections.borrow().as_slice(),
        &[CommandSelection::new(7, "item-0007", "Item 0007", None)]
    );
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
fn avatar_fallback_initials_derive_from_display_names_and_empty_names() {
    let ada = Avatar::new("ada", "Ada Lovelace").state();
    let single = Avatar::new("single", "Grace").state();
    let trio = Avatar::new("trio", "Foo Bar Dar").state();
    let empty = Avatar::new("empty", "  ").state();

    assert_eq!(ada.name(), "Ada Lovelace");
    assert_eq!(ada.fallback(), "AL");
    assert_eq!(ada.accessible_label(), "Ada Lovelace");
    assert_eq!(ada.role(), Role::Image);

    assert_eq!(single.fallback(), "GR");
    assert_eq!(trio.fallback(), "FB");
    assert_eq!(empty.fallback(), "?");
    assert_eq!(empty.accessible_label(), "Avatar");
}

#[test]
fn avatar_explicit_fallback_overrides_derived_initials() {
    let state = Avatar::new("current-user", "Ada Lovelace")
        .fallback("ME")
        .state();

    assert_eq!(state.name(), "Ada Lovelace");
    assert_eq!(state.fallback(), "ME");
}

#[test]
fn avatar_source_metadata_does_not_own_loading_state() {
    let state = Avatar::new("profile", "Ada Lovelace")
        .source("asset://avatars/ada.png")
        .state();

    assert!(state.has_source());
    assert_eq!(
        state.source().map(|source| source.uri()),
        Some("asset://avatars/ada.png")
    );
    assert_eq!(state.fallback(), "AL");
    assert_eq!(state.accessible_label(), "Ada Lovelace");
}

#[test]
fn avatar_accessible_label_can_be_explicit_for_source_and_fallback_avatars() {
    let fallback = Avatar::new("fallback-avatar", "Ada Lovelace")
        .accessible_label("Current user")
        .state();
    let source = Avatar::new("source-avatar", "Ada Lovelace")
        .source("asset://avatars/ada.png")
        .accessible_label("Ada profile photo")
        .state();

    assert_eq!(fallback.accessible_label(), "Current user");
    assert_eq!(source.accessible_label(), "Ada profile photo");
}

#[test]
fn avatar_size_metrics_and_token_intents_are_stable() {
    let tokens = custom_tokens();
    let small = Avatar::new("small-avatar", "Ada")
        .small()
        .tokens(tokens)
        .state();
    let medium = Avatar::new("medium-avatar", "Ada").tokens(tokens).state();
    let large = Avatar::new("large-avatar", "Ada")
        .large()
        .tokens(tokens)
        .state();

    assert_eq!(small.size(), Size::Small);
    assert_eq!(small.metrics().diameter(), ui_px(28.0));
    assert_eq!(small.metrics().text_size(), ui_px(11.0));

    assert_eq!(medium.metrics().diameter(), ui_px(32.0));
    assert_eq!(medium.metrics().radius(), ui_px(16.0));

    assert_eq!(large.metrics().diameter(), ui_px(40.0));
    assert_eq!(large.metrics().text_size(), ui_px(14.0));
    assert_eq!(large.colors().background().token(), tokens.surface_muted);
    assert_eq!(large.colors().foreground().token(), tokens.text);
    assert_eq!(large.colors().border().token(), tokens.border);
}

#[test]
fn avatar_group_state_tracks_visible_and_hidden_counts() {
    let group = AvatarGroup::new("team")
        .avatar(Avatar::new("ada", "Ada Lovelace"))
        .avatar(Avatar::new("grace", "Grace Hopper"))
        .avatar(Avatar::new("katherine", "Katherine Johnson"))
        .avatar(Avatar::new("margaret", "Margaret Hamilton"))
        .max_visible(3)
        .tokens(custom_tokens());
    let state = group.state();

    assert_eq!(state.size(), Size::Medium);
    assert_eq!(state.total_count(), 4);
    assert_eq!(state.visible_count(), 3);
    assert_eq!(state.hidden_count(), 1);

    let count = AvatarGroupCount::new("team-count", state.hidden_count())
        .with_size(state.size())
        .tokens(custom_tokens());
    let count_state = count.state();

    assert_eq!(count_state.count(), 1);
    assert_eq!(count_state.size(), Size::Medium);
    assert_eq!(count_state.role(), Role::Label);
}

#[open_gpui::test]
fn avatar_renders_stable_debug_selector(cx: &mut open_gpui::TestAppContext) {
    struct TestView;

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            div()
                .size_full()
                .child(Avatar::new("runtime-avatar", "Ada Lovelace"))
        }
    }

    let (_, cx) = cx.add_window_view(|_, _| TestView);
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    assert!(cx.debug_bounds("avatar:runtime-avatar:root").is_some());
}

#[test]
fn separator_state_exposes_orientation_role_and_decorative_mode() {
    let horizontal = Separator::new("section-separator").state();
    let vertical = Separator::new("panel-separator").vertical().large().state();
    let decorative = Separator::new("decorative-separator")
        .decorative(true)
        .state();

    assert_eq!(horizontal.orientation(), Orientation::Horizontal);
    assert_eq!(horizontal.role(), Some(Role::Separator));
    assert_eq!(horizontal.metrics().thickness(), ui_px(1.0));
    assert_eq!(horizontal.colors().line().token(), semantic::BORDER);

    assert_eq!(vertical.orientation(), Orientation::Vertical);
    assert_eq!(vertical.role(), Some(Role::Separator));
    assert_eq!(vertical.metrics().thickness(), ui_px(2.0));

    assert!(decorative.decorative());
    assert_eq!(decorative.role(), None);
}

#[test]
fn kbd_state_is_display_only_with_muted_token_intents() {
    let tokens = custom_tokens();
    let state = Kbd::new("command-shortcut", "Ctrl+K")
        .small()
        .tokens(tokens)
        .state();

    assert_eq!(state.label(), "Ctrl+K");
    assert_eq!(state.size(), Size::Small);
    assert!(state.display_only());
    assert_eq!(state.metrics().min_width(), ui_px(20.0));
    assert_eq!(state.colors().background().token(), tokens.surface_muted);
    assert_eq!(state.colors().foreground().token(), tokens.text_muted);
    assert_eq!(state.colors().border().token(), tokens.border);
}

#[test]
fn progress_state_clamps_values_and_preserves_indeterminate_mode() {
    let full = Progress::new("upload-progress", "Upload")
        .value(142.0)
        .large()
        .state();
    let empty = Progress::new("empty-progress", "Empty")
        .value(f32::NAN)
        .state();
    let indeterminate = Progress::new("pending-progress", "Pending")
        .indeterminate()
        .state();

    assert_eq!(full.role(), Role::ProgressIndicator);
    assert_eq!(full.value_percent(), Some(100.0));
    assert_eq!(full.normalized_value(), Some(1.0));
    assert_eq!(
        full.visual_mode(),
        ProgressVisualMode::Determinate {
            normalized_value: 1.0
        }
    );
    assert_eq!(full.indicator_start_fraction(), 0.0);
    assert_eq!(full.indicator_fraction(), 1.0);
    assert_eq!(full.metrics().height(), ui_px(10.0));
    assert_eq!(full.colors().track().token(), semantic::SURFACE_MUTED);
    assert_eq!(full.colors().indicator().token(), semantic::ACCENT);

    assert_eq!(empty.value_percent(), Some(0.0));
    assert_eq!(empty.normalized_value(), Some(0.0));
    assert_eq!(
        empty.visual_mode(),
        ProgressVisualMode::Determinate {
            normalized_value: 0.0
        }
    );
    assert!(indeterminate.indeterminate());
    assert_eq!(indeterminate.value_percent(), None);
    assert_eq!(indeterminate.normalized_value(), None);
    assert_eq!(
        indeterminate.visual_mode(),
        ProgressVisualMode::Indeterminate
    );
    assert!(
        indeterminate.indicator_start_fraction() > 0.0,
        "indeterminate progress should not look like a left-anchored determinate fill"
    );
    assert!(
        indeterminate.indicator_fraction() > 0.0 && indeterminate.indicator_fraction() < 0.5,
        "indeterminate progress should render as a short segment, not as a fixed percentage value"
    );
}

#[test]
fn skeleton_state_is_noninteractive_placeholder_with_stable_metrics() {
    let tokens = custom_tokens();
    let state = Skeleton::new("loading-line")
        .subtle(true)
        .large()
        .tokens(tokens)
        .state();

    assert_eq!(state.size(), Size::Large);
    assert!(state.subtle());
    assert!(state.display_only());
    assert_eq!(state.metrics().width(), ui_px(224.0));
    assert_eq!(state.metrics().height(), ui_px(20.0));
    assert_eq!(state.colors().background().token(), tokens.surface_muted);
}

#[open_gpui::test]
fn low_state_primitives_render_stable_debug_selectors(cx: &mut open_gpui::TestAppContext) {
    struct TestView;

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            div()
                .size_full()
                .flex()
                .flex_col()
                .gap_2()
                .child(Separator::new("runtime-separator"))
                .child(Kbd::new("runtime-kbd", "Ctrl+K"))
                .child(Progress::new("runtime-progress", "Loading").value(40.0))
                .child(Progress::new("runtime-progress-indeterminate", "Indexing").indeterminate())
                .child(Skeleton::new("runtime-skeleton"))
        }
    }

    let (_, cx) = cx.add_window_view(|_, _| TestView);
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    for selector in [
        "separator:runtime-separator:root",
        "kbd:runtime-kbd:root",
        "progress:runtime-progress:root",
        "progress:runtime-progress:indicator",
        "progress:runtime-progress-indeterminate:root",
        "progress:runtime-progress-indeterminate:indicator",
        "skeleton:runtime-skeleton:root",
    ] {
        assert!(
            cx.debug_bounds(selector).is_some(),
            "{selector} should be rendered"
        );
    }

    let determinate_root = cx
        .debug_bounds("progress:runtime-progress:root")
        .expect("determinate progress root should render");
    let determinate_indicator = cx
        .debug_bounds("progress:runtime-progress:indicator")
        .expect("determinate progress indicator should render");
    let indeterminate_root = cx
        .debug_bounds("progress:runtime-progress-indeterminate:root")
        .expect("indeterminate progress root should render");
    let indeterminate_indicator = cx
        .debug_bounds("progress:runtime-progress-indeterminate:indicator")
        .expect("indeterminate progress indicator should render");

    let determinate_width =
        determinate_indicator.size.width.as_f32() / determinate_root.size.width.as_f32();
    let indeterminate_start = (indeterminate_indicator.left().as_f32()
        - indeterminate_root.left().as_f32())
        / indeterminate_root.size.width.as_f32();
    let indeterminate_width =
        indeterminate_indicator.size.width.as_f32() / indeterminate_root.size.width.as_f32();

    assert!(
        (determinate_width - 0.4).abs() < 0.02,
        "determinate progress indicator should match the provided value"
    );
    assert!(
        indeterminate_start > 0.25,
        "indeterminate progress indicator should not be left-anchored"
    );
    assert!(
        indeterminate_width > 0.25 && indeterminate_width < 0.45,
        "indeterminate progress indicator should be a short segment"
    );
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

    assert_eq!(root_entry.snapshot().revision(), 7);
    assert_eq!(prelude_entry.snapshot().revision(), 8);
    assert_eq!(root_diagnostics.fallback_mode(), root::ThemeMode::Light);
    assert!(root_diagnostics.fallback_color_count() > 0);
    assert_eq!(
        prelude_diagnostics.fallback_mode(),
        prelude::ThemeMode::Dark
    );
    assert_eq!(root_error, root::ThemeValidationError::MissingId);
    assert_eq!(prelude_error, prelude::ThemeValidationError::MissingLabel);
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
    assert_eq!(metrics.track_width(), ui_px(32.0));
    assert_eq!(metrics.track_height(), ui_px(18.0));
    assert_eq!(metrics.thumb_size(), ui_px(14.0));
    assert_eq!(metrics.checked_thumb_x(), ui_px(16.0));
}

#[open_gpui::test]
fn switch_runtime_click_emits_on_change_with_next_checked(cx: &mut open_gpui::TestAppContext) {
    struct TestView {
        checked: Rc<RefCell<bool>>,
        changes: Rc<RefCell<Vec<bool>>>,
    }

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let checked = *self.checked.borrow();
            let next_checked = self.checked.clone();
            let changes = self.changes.clone();
            let disabled_changes = self.changes.clone();

            div()
                .size_full()
                .flex()
                .flex_col()
                .gap_2()
                .child(
                    Switch::new("runtime-switch")
                        .label("Runtime switch")
                        .checked(checked)
                        .on_change(move |checked, _, _, _| {
                            *next_checked.borrow_mut() = checked;
                            changes.borrow_mut().push(checked);
                        }),
                )
                .child(
                    Switch::new("disabled-runtime-switch")
                        .label("Disabled runtime switch")
                        .disabled(true)
                        .on_change(move |checked, _, _, _| {
                            disabled_changes.borrow_mut().push(checked);
                        }),
                )
        }
    }

    let checked = Rc::new(RefCell::new(false));
    let changes = Rc::new(RefCell::new(Vec::new()));
    let (_, cx) = cx.add_window_view(|_, _| TestView {
        checked: checked.clone(),
        changes: changes.clone(),
    });
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    let disabled_switch = cx
        .debug_bounds("switch:disabled-runtime-switch:root")
        .expect("disabled switch should expose a stable debug selector");
    cx.simulate_click(disabled_switch.center(), Default::default());
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });
    assert!(
        changes.borrow().is_empty(),
        "disabled switch click should not emit on_change"
    );

    let runtime_switch = cx
        .debug_bounds("switch:runtime-switch:root")
        .expect("runtime switch should expose a stable debug selector");
    cx.simulate_click(runtime_switch.center(), Default::default());
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });
    assert_eq!(*checked.borrow(), true);
    assert_eq!(changes.borrow().as_slice(), &[true]);

    let runtime_switch = cx
        .debug_bounds("switch:runtime-switch:root")
        .expect("runtime switch should remain rendered after controlled update");
    cx.simulate_click(runtime_switch.center(), Default::default());
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });
    assert_eq!(*checked.borrow(), false);
    assert_eq!(changes.borrow().as_slice(), &[true, false]);
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
    assert_eq!(state.display_text().as_ref(), "Email address");
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
    assert_eq!(state.display_text().as_ref(), "hello@example.com");
    assert!(!state.displaying_placeholder());
}

#[test]
fn text_input_state_normalizes_static_values_with_single_line_policy() {
    let state = TextInput::new("query", "Search")
        .value("alpha\r\nbeta\ngamma")
        .state();

    assert_eq!(state.value(), "alpha  beta gamma");
    assert_eq!(state.display_text().as_ref(), "alpha  beta gamma");
}

#[test]
fn password_text_input_masks_display_without_hiding_value() {
    let state = TextInput::new("password", "Password")
        .placeholder("Password")
        .value("a🙂中")
        .display_mode(TextInputDisplayMode::Password)
        .state();

    assert_eq!(state.value(), "a🙂中");
    assert_eq!(state.display_mode(), TextInputDisplayMode::Password);
    assert_eq!(state.display_text().as_ref(), "•••");
    assert!(state.display_mode().masks_value());
    assert!(!state.displaying_placeholder());
}

#[test]
fn controlled_text_input_on_change_marks_input_controller_driven() {
    let state = TextInput::new("email", "Email")
        .value("hello@example.com")
        .on_change(|_, _, _| {})
        .state();

    assert!(state.controller_driven());
    assert!(state.editable());
    assert_eq!(state.value(), "hello@example.com");
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
    assert_eq!(shadow[0].spread_radius, px(2.0));
    assert_eq!(shadow[0].blur_radius, px(0.0));
    assert!(!shadow[0].inset);
}

#[test]
fn text_input_size_helpers_apply_input_metrics() {
    let state = TextInput::new("query", "Search").large().state();

    assert_eq!(state.size(), Size::Large);
    assert_eq!(state.metrics().height(), ui_px(36.0));
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
fn text_input_runtime_accepts_controller_backed_simulated_input(
    cx: &mut open_gpui::TestAppContext,
) {
    struct TestView {
        controller: open_gpui::Entity<TextInputController>,
    }

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            div().size_full().child(
                TextInput::new("runtime-text-input", "Runtime text input")
                    .controller(self.controller.clone())
                    .placeholder("Type here"),
            )
        }
    }

    cx.update(init_text_input);
    let controller = cx.new(TextInputController::new);
    let (_, cx) = cx.add_window_view(|_, _| TestView {
        controller: controller.clone(),
    });
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    let input = cx
        .debug_bounds("text-input:runtime-text-input:root")
        .expect("standalone text input should expose a stable debug selector");
    cx.simulate_click(input.center(), Default::default());
    cx.simulate_input("hello\nworld");
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    cx.update_entity(&controller, |controller, _| {
        assert_eq!(controller.value(), "hello world");
        assert_eq!(
            controller.selected_range(),
            controller.value().len()..controller.value().len()
        );
    });
}

#[open_gpui::test]
fn controlled_text_input_on_change_accepts_input_without_supplied_controller(
    cx: &mut open_gpui::TestAppContext,
) {
    struct TestView {
        value: Rc<RefCell<String>>,
        changes: Rc<RefCell<Vec<String>>>,
    }

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let value = self.value.borrow().clone();
            let next_value = self.value.clone();
            let changes = self.changes.clone();

            div().size_full().child(
                TextInput::new("controlled-text-input", "Controlled text input")
                    .value(value)
                    .placeholder("Type here")
                    .on_change(move |value, _, _| {
                        *next_value.borrow_mut() = value.clone();
                        changes.borrow_mut().push(value);
                    }),
            )
        }
    }

    cx.update(init_text_input);
    let value = Rc::new(RefCell::new(String::new()));
    let changes = Rc::new(RefCell::new(Vec::new()));
    let (_, cx) = cx.add_window_view(|_, _| TestView {
        value: value.clone(),
        changes: changes.clone(),
    });
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    let input = cx
        .debug_bounds("text-input:controlled-text-input:root")
        .expect("controlled text input should expose a stable debug selector");
    cx.simulate_click(input.center(), Default::default());
    cx.simulate_input("hello\nworld");
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    assert_eq!(value.borrow().as_str(), "hello world");
    assert_eq!(
        changes.borrow().last().map(String::as_str),
        Some("hello world")
    );
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
fn default_textarea_state_uses_text_input_role_and_rows() {
    let state = Textarea::new("notes", "Notes")
        .placeholder("Release notes")
        .rows(4)
        .state();

    assert_eq!(state.role(), Role::TextInput);
    assert_eq!(state.rows(), 4);
    assert_eq!(state.metrics().rows(), 4);
    assert!(state.placeholder_visible());
    assert_eq!(state.display_text(), "Release notes");
    assert!(state.editable());
    assert!(!state.controller_driven());
}

#[test]
fn filled_textarea_preserves_newlines_in_state() {
    let state = Textarea::new("notes", "Notes")
        .value("Line 1\r\nLine 2")
        .placeholder("Release notes")
        .state();

    assert!(state.has_value());
    assert_eq!(state.value(), "Line 1\nLine 2");
    assert_eq!(state.display_text(), "Line 1\nLine 2");
    assert!(!state.displaying_placeholder());
}

#[test]
fn disabled_read_only_and_invalid_textareas_expose_control_state() {
    let tokens = custom_tokens();
    let disabled = Textarea::new("disabled-notes", "Disabled notes")
        .disabled(true)
        .tokens(tokens)
        .state();
    let read_only = Textarea::new("readonly-notes", "Read-only notes")
        .read_only(true)
        .state();
    let invalid = Textarea::new("invalid-notes", "Invalid notes")
        .invalid(true)
        .tokens(tokens)
        .state();

    assert!(disabled.disabled());
    assert!(!disabled.editable());
    assert!(read_only.read_only());
    assert!(!read_only.editable());
    assert!(invalid.invalid());
    assert_eq!(invalid.colors().border().token(), tokens.destructive);
}

#[open_gpui::test]
fn controlled_textarea_on_change_preserves_newline_input(cx: &mut open_gpui::TestAppContext) {
    struct TestView {
        value: Rc<RefCell<String>>,
        changes: Rc<RefCell<Vec<String>>>,
    }

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let value = self.value.borrow().clone();
            let next_value = self.value.clone();
            let changes = self.changes.clone();

            div().size_full().child(
                Textarea::new("controlled-textarea", "Controlled textarea")
                    .value(value)
                    .placeholder("Type notes")
                    .on_change(move |value, _, _| {
                        *next_value.borrow_mut() = value.clone();
                        changes.borrow_mut().push(value);
                    }),
            )
        }
    }

    let value = Rc::new(RefCell::new(String::new()));
    let changes = Rc::new(RefCell::new(Vec::new()));
    let (_, cx) = cx.add_window_view(|_, _| TestView {
        value: value.clone(),
        changes: changes.clone(),
    });
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    let input = cx
        .debug_bounds("textarea:controlled-textarea:root")
        .expect("controlled textarea should expose a stable debug selector");
    cx.simulate_click(input.center(), Default::default());
    cx.simulate_input("Line 1\nLine 2");
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    assert_eq!(value.borrow().as_str(), "Line 1\nLine 2");
    assert_eq!(
        changes.borrow().last().map(String::as_str),
        Some("Line 1\nLine 2")
    );
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
