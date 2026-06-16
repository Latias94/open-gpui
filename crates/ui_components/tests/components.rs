use open_gpui::{
    Anchor, AppContext, Context, InteractiveElement, IntoElement, ParentElement, Render,
    ScrollDelta, ScrollWheelEvent, Styled, Window, div, point, px, size,
};
use open_gpui_ui_components::{
    Badge, BadgeVariant, Button, ButtonVariant, Checkbox, ColorState, ContextMenu,
    DEFAULT_FOCUS_RING_WIDTH, DEFAULT_OVERLAY_SAFE_MARGIN, Dialog, DialogOpenMode, Field,
    FocusRing, GpuiOverlayAdapterConfig, GpuiOverlayPlacement, IconButton, Label, Menu, MenuItem,
    MenuItemKind, MenuOpenMode, Popover, PopoverOpenMode, RadioGroup, RadioGroupState, RadioItem,
    RadioItemDescriptor, ScrollArea, ScrollAreaAxis, ScrollAreaState, ScrollResetPolicy, Splitter,
    SplitterPanel, SplitterPanelDescriptor, SplitterState, Switch, Tabs, TabsActivationMode,
    TabsItem, TabsItemDescriptor, TabsState, TextInput, TextInputController, ThemeColor, ThemeMode,
    ThemeResolver, ThemeSnapshot, Toggle, ToggleVariant, Tooltip, TooltipContentKind,
    TooltipDelayPolicy, TooltipOpenIntent, active_index_from_str_keys, default_deferred_priority,
    escape_open_change, first_enabled, focus_ring_shadow, gpui_anchor, last_enabled,
    menu_navigation_target, next_enabled, outside_press_open_change, point_anchor_placement,
};
use open_gpui_ui_core::{
    DismissReason, EscapeKeyPolicy, FocusRestoreIntent, InitialFocusIntent, Orientation,
    OutsidePressPolicy, OverlayAnchorInput, OverlayLayerKind, OverlayLayerPolicy,
    OverlayPlacementAlignment, OverlayPlacementInput, OverlayPlacementSide, OverlayPresence, Role,
    Sizable, Size, ThemeTokens, Toggled, TokenKey, rect, semantic,
};
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
            Some(rect(point(px(10.0), px(20.0)), size(px(100.0), px(40.0)))),
            Some(rect(point(px(30.0), px(40.0)), size(px(120.0), px(60.0)))),
        ),
        size(px(180.0), px(120.0)),
    )
    .with_side(OverlayPlacementSide::Bottom)
    .with_alignment(OverlayPlacementAlignment::End)
    .with_offset(px(6.0))
    .with_safe_bounds(rect(point(px(0.0), px(0.0)), size(px(300.0), px(220.0))));

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
    let point_placement = point_anchor_placement(point(px(5.0), px(6.0)), size(px(80.0), px(40.0)));
    assert_eq!(
        GpuiOverlayPlacement::resolve(point_placement, DEFAULT_OVERLAY_SAFE_MARGIN).anchor(),
        Anchor::TopLeft
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
    assert_eq!(
        state.initial_focus_intent(),
        &InitialFocusIntent::FirstFocusable
    );
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

#[test]
fn menu_state_records_items_roving_focus_and_overlay_policy() {
    let state = Menu::new("file-menu", "File")
        .open(true)
        .focused_value("save")
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
    assert_eq!(state.tab_stop_value(), Some("save"));
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
fn menu_navigation_and_activation_skip_disabled_and_separator_items() {
    let state = Menu::new("edit-menu", "Edit")
        .open(true)
        .focused_value("copy")
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
        .focused_value("duplicate")
        .item(MenuItem::action("duplicate", "Duplicate"))
        .item(MenuItem::separator("separator"))
        .item(MenuItem::action("delete", "Delete").disabled(true))
        .state();

    assert!(state.open());
    assert_eq!(state.open_mode(), MenuOpenMode::Controlled);
    assert_eq!(state.anchor_point(), anchor);
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
    assert_eq!(placement_input.offset(), px(0.0));
    assert_eq!(
        placement_input.content_size(),
        open_gpui_ui_core::OverlaySize::new(
            state.metrics().min_width(),
            state.metrics().item_height()
        )
    );
    let placement = GpuiOverlayPlacement::resolve(placement_input, DEFAULT_OVERLAY_SAFE_MARGIN);
    assert_eq!(placement.anchor(), Anchor::TopLeft);
    assert_eq!(
        placement_input.preferred_anchor_bounds(),
        Some(open_gpui_ui_core::anchor_rect_from_point(anchor))
    );
    assert_eq!(
        placement.position(),
        Some(open_gpui_ui_core::anchor_rect_from_point(anchor).bottom_left())
    );
    assert_eq!(placement.snap_margin(), DEFAULT_OVERLAY_SAFE_MARGIN);
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
    assert_eq!(state.metrics().scrollbar_width(), px(8.0));
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
    assert_eq!(state.metrics().scrollbar_width(), px(12.0));
    assert_eq!(state.reset_key(), Some("settings"));
    assert!(state.should_reset_for_key_change(Some("overview")));
    assert_eq!(preserved.reset_policy(), ScrollResetPolicy::Preserve);
    assert_eq!(preserved.reset_key(), None);
    assert!(!preserved.should_reset_for_key_change(Some("overview")));
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
    assert_eq!(state.metrics().handle_hit_size(), px(12.0));
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
    let menus = [
        Menu::new("menu", "Menu")
            .open(true)
            .item(MenuItem::action("open", "Open"))
            .state(),
        Menu::new("closed-menu", "Closed")
            .item(MenuItem::action("open", "Open"))
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
