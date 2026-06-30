//! Hover card component.

use crate::geometry::gpui_px_from_ui;
use std::rc::Rc;

use open_gpui::prelude::*;
use open_gpui::{
    AnyElement, App, ClickEvent, ElementId, Entity, FocusHandle, InteractiveElement, IntoElement,
    KeyDownEvent, ParentElement, RenderOnce, SharedString, StatefulInteractiveElement, Styled,
    Task, Window, anchored, deferred, div,
};
use open_gpui_ui_core::{
    FocusRestoreIntent, InitialFocusIntent, OutsidePressPolicy, OverlayLayerKind,
    OverlayPlacementAlignment, OverlayPlacementInput, OverlayPlacementSide, OverlayPresence, Role,
    Sizable, Size, ThemeTokens, UiPx, ui_point, ui_px, ui_size,
};

use crate::a11y::UiA11yElementExt;
use crate::color::ColorIntent;
use crate::focus::{FocusRing, focus_ring_shadow};
use crate::overlay::{
    GpuiOverlayAdapterConfig, GpuiOverlayPlacement, OverlayResolvedState, consume_overlay_event,
    emit_overlay_open_change, escape_open_change, gpui_overlay_state, outside_press_open_change,
    resolve_overlay_open_state,
};
use crate::theme::ThemeResolver;

type HoverCardOpenChangeHandler = Rc<dyn Fn(bool, &mut Window, &mut App)>;

/// Hover card open-state ownership.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum HoverCardOpenMode {
    /// Open state is owned by the component adapter after initialization.
    #[default]
    Uncontrolled,
    /// Open state is provided by the caller.
    Controlled,
}

impl HoverCardOpenMode {
    /// Returns a stable label.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Uncontrolled => "uncontrolled",
            Self::Controlled => "controlled",
        }
    }
}

/// Trigger affordance that can open a hover card.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum HoverCardOpenIntent {
    /// Hover card opens from either pointer hover or keyboard focus.
    #[default]
    HoverOrFocus,
    /// Hover card opens from pointer hover only.
    Hover,
    /// Hover card opens from keyboard focus only.
    Focus,
    /// Hover card is controlled or opened by explicit adapter events.
    Manual,
}

impl HoverCardOpenIntent {
    /// Returns a stable label.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::HoverOrFocus => "hover or focus",
            Self::Hover => "hover",
            Self::Focus => "focus",
            Self::Manual => "manual",
        }
    }

    /// Returns whether pointer hover may open the hover card.
    pub const fn opens_on_hover(self) -> bool {
        matches!(self, Self::HoverOrFocus | Self::Hover)
    }

    /// Returns whether keyboard focus may open the hover card.
    pub const fn opens_on_focus(self) -> bool {
        matches!(self, Self::HoverOrFocus | Self::Focus)
    }

    /// Returns whether trigger click may toggle the hover card.
    pub const fn opens_manually(self) -> bool {
        matches!(self, Self::Manual)
    }
}

/// Hover card content kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum HoverCardContentKind {
    /// Plain text content.
    #[default]
    Text,
    /// Simple element content.
    Element,
}

impl HoverCardContentKind {
    /// Returns a stable label.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Text => "text",
            Self::Element => "element",
        }
    }
}

/// Hover card delay policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HoverCardDelayPolicy {
    open_delay: std::time::Duration,
    close_delay: std::time::Duration,
}

impl HoverCardDelayPolicy {
    /// Creates a hover card delay policy.
    pub const fn new(open_delay: std::time::Duration, close_delay: std::time::Duration) -> Self {
        Self {
            open_delay,
            close_delay,
        }
    }

    /// Returns the delay before showing the hover card.
    pub const fn open_delay(self) -> std::time::Duration {
        self.open_delay
    }

    /// Returns the delay before hiding the hover card after hover leaves.
    pub const fn close_delay(self) -> std::time::Duration {
        self.close_delay
    }
}

impl Default for HoverCardDelayPolicy {
    fn default() -> Self {
        Self {
            open_delay: std::time::Duration::from_millis(700),
            close_delay: std::time::Duration::from_millis(300),
        }
    }
}

/// Resolved hover card color intents.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HoverCardColors {
    pub(crate) background: ColorIntent,
    pub(crate) foreground: ColorIntent,
    pub(crate) muted_foreground: ColorIntent,
    pub(crate) border: ColorIntent,
    pub(crate) trigger_background: ColorIntent,
    pub(crate) trigger_hover_background: ColorIntent,
    pub(crate) trigger_foreground: ColorIntent,
    pub(crate) trigger_border: ColorIntent,
    pub(crate) focus_ring: ColorIntent,
}

impl HoverCardColors {
    /// Returns content background color intent.
    pub const fn background(self) -> ColorIntent {
        self.background
    }

    /// Returns content foreground color intent.
    pub const fn foreground(self) -> ColorIntent {
        self.foreground
    }

    /// Returns muted content foreground color intent.
    pub const fn muted_foreground(self) -> ColorIntent {
        self.muted_foreground
    }

    /// Returns content border color intent.
    pub const fn border(self) -> ColorIntent {
        self.border
    }

    /// Returns trigger background color intent.
    pub const fn trigger_background(self) -> ColorIntent {
        self.trigger_background
    }

    /// Returns trigger hover background color intent.
    pub const fn trigger_hover_background(self) -> ColorIntent {
        self.trigger_hover_background
    }

    /// Returns trigger foreground color intent.
    pub const fn trigger_foreground(self) -> ColorIntent {
        self.trigger_foreground
    }

    /// Returns trigger border color intent.
    pub const fn trigger_border(self) -> ColorIntent {
        self.trigger_border
    }

    /// Returns trigger focus-ring color intent.
    pub const fn focus_ring(self) -> ColorIntent {
        self.focus_ring
    }
}

/// Resolved hover card metrics.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HoverCardMetrics {
    trigger_height: UiPx,
    trigger_padding_x: UiPx,
    trigger_padding_y: UiPx,
    content_padding: UiPx,
    radius: UiPx,
    title_size: UiPx,
    text_size: UiPx,
    min_width: UiPx,
    max_width: UiPx,
    max_height: UiPx,
    offset: UiPx,
}

impl HoverCardMetrics {
    /// Resolves metrics from the shared foundation size vocabulary.
    pub const fn from_size(size: Size) -> Self {
        Self {
            trigger_height: size.button_h(),
            trigger_padding_x: size.button_px(),
            trigger_padding_y: size.button_py(),
            content_padding: size.button_px(),
            radius: size.control_radius(),
            title_size: ui_px(15.0),
            text_size: size.control_text_px(),
            min_width: ui_px(240.0),
            max_width: ui_px(360.0),
            max_height: ui_px(280.0),
            offset: ui_px(8.0),
        }
    }

    /// Returns trigger height.
    pub const fn trigger_height(self) -> UiPx {
        self.trigger_height
    }

    /// Returns trigger horizontal padding.
    pub const fn trigger_padding_x(self) -> UiPx {
        self.trigger_padding_x
    }

    /// Returns trigger vertical padding.
    pub const fn trigger_padding_y(self) -> UiPx {
        self.trigger_padding_y
    }

    /// Returns content padding.
    pub const fn content_padding(self) -> UiPx {
        self.content_padding
    }

    /// Returns corner radius.
    pub const fn radius(self) -> UiPx {
        self.radius
    }

    /// Returns title text size.
    pub const fn title_size(self) -> UiPx {
        self.title_size
    }

    /// Returns body text size.
    pub const fn text_size(self) -> UiPx {
        self.text_size
    }

    /// Returns minimum content width.
    pub const fn min_width(self) -> UiPx {
        self.min_width
    }

    /// Returns maximum content width.
    pub const fn max_width(self) -> UiPx {
        self.max_width
    }

    /// Returns maximum content height.
    pub const fn max_height(self) -> UiPx {
        self.max_height
    }

    /// Returns trigger-to-content placement offset.
    pub const fn offset(self) -> UiPx {
        self.offset
    }
}

/// Resolved hover card state used by tests, demos, and rendering.
#[derive(Debug, Clone, PartialEq)]
pub struct HoverCardState {
    content_kind: HoverCardContentKind,
    size: Size,
    disabled: bool,
    open: bool,
    default_open: bool,
    open_mode: HoverCardOpenMode,
    open_intent: HoverCardOpenIntent,
    placement_side: OverlayPlacementSide,
    placement_alignment: OverlayPlacementAlignment,
    delay: HoverCardDelayPolicy,
    outside_press_policy: OutsidePressPolicy,
    initial_focus_intent: InitialFocusIntent,
    focus_restore_intent: FocusRestoreIntent,
    trigger_selected: bool,
    metrics: HoverCardMetrics,
    colors: HoverCardColors,
    focus_ring: FocusRing,
    overlay: OverlayResolvedState,
}

impl HoverCardState {
    /// Resolves the public state for a hover card.
    #[allow(clippy::too_many_arguments)]
    pub fn resolve(
        content_kind: HoverCardContentKind,
        size: Size,
        disabled: bool,
        open: Option<bool>,
        default_open: bool,
        open_intent: HoverCardOpenIntent,
        placement_side: OverlayPlacementSide,
        placement_alignment: OverlayPlacementAlignment,
        delay: HoverCardDelayPolicy,
        outside_press_policy: OutsidePressPolicy,
        initial_focus_intent: InitialFocusIntent,
        focus_restore_intent: FocusRestoreIntent,
        tokens: ThemeTokens,
    ) -> Self {
        let open_mode = if open.is_some() {
            HoverCardOpenMode::Controlled
        } else {
            HoverCardOpenMode::Uncontrolled
        };
        Self::resolve_with_open_mode(
            content_kind,
            size,
            disabled,
            open.unwrap_or(default_open),
            default_open,
            open_mode,
            open_intent,
            placement_side,
            placement_alignment,
            delay,
            outside_press_policy,
            initial_focus_intent,
            focus_restore_intent,
            tokens,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn resolve_with_open_mode(
        content_kind: HoverCardContentKind,
        size: Size,
        disabled: bool,
        open: bool,
        default_open: bool,
        open_mode: HoverCardOpenMode,
        open_intent: HoverCardOpenIntent,
        placement_side: OverlayPlacementSide,
        placement_alignment: OverlayPlacementAlignment,
        delay: HoverCardDelayPolicy,
        outside_press_policy: OutsidePressPolicy,
        initial_focus_intent: InitialFocusIntent,
        focus_restore_intent: FocusRestoreIntent,
        tokens: ThemeTokens,
    ) -> Self {
        let open = open && !disabled;
        let presence = OverlayPresence::from_open(open);
        let overlay =
            GpuiOverlayAdapterConfig::new(OverlayLayerKind::NonModalDismissible, presence)
                .outside_press_policy(outside_press_policy)
                .initial_focus_intent(initial_focus_intent.clone())
                .focus_restore_intent(focus_restore_intent.clone())
                .resolved_state();
        let colors = ThemeResolver::hover_card_colors(tokens, open);

        Self {
            content_kind,
            size,
            disabled,
            open,
            default_open,
            open_mode,
            open_intent,
            placement_side,
            placement_alignment,
            delay,
            outside_press_policy,
            initial_focus_intent,
            focus_restore_intent,
            trigger_selected: open,
            metrics: HoverCardMetrics::from_size(size),
            colors,
            focus_ring: FocusRing::from_color(colors.focus_ring()),
            overlay,
        }
    }

    /// Returns content kind.
    pub const fn content_kind(&self) -> HoverCardContentKind {
        self.content_kind
    }

    /// Returns foundation size.
    pub const fn size(&self) -> Size {
        self.size
    }

    /// Returns whether the trigger is disabled.
    pub const fn disabled(&self) -> bool {
        self.disabled
    }

    /// Returns whether hover card content is open.
    pub const fn open(&self) -> bool {
        self.open
    }

    /// Returns uncontrolled initial open state.
    pub const fn default_open(&self) -> bool {
        self.default_open
    }

    /// Returns open-state ownership.
    pub const fn open_mode(&self) -> HoverCardOpenMode {
        self.open_mode
    }

    /// Returns open intent.
    pub const fn open_intent(&self) -> HoverCardOpenIntent {
        self.open_intent
    }

    /// Returns preferred placement side.
    pub const fn placement_side(&self) -> OverlayPlacementSide {
        self.placement_side
    }

    /// Returns preferred placement alignment.
    pub const fn placement_alignment(&self) -> OverlayPlacementAlignment {
        self.placement_alignment
    }

    /// Returns delay policy.
    pub const fn delay(&self) -> HoverCardDelayPolicy {
        self.delay
    }

    /// Returns outside-press policy.
    pub const fn outside_press_policy(&self) -> OutsidePressPolicy {
        self.outside_press_policy
    }

    /// Returns initial focus intent.
    pub const fn initial_focus_intent(&self) -> &InitialFocusIntent {
        &self.initial_focus_intent
    }

    /// Returns focus restore intent.
    pub const fn focus_restore_intent(&self) -> &FocusRestoreIntent {
        &self.focus_restore_intent
    }

    /// Returns whether the trigger should present selected/expanded state.
    pub const fn trigger_selected(&self) -> bool {
        self.trigger_selected
    }

    /// Returns whether hover card content is descriptive-only.
    pub const fn descriptive(&self) -> bool {
        false
    }

    /// Returns whether hover card content is interactive.
    pub const fn interactive_content(&self) -> bool {
        true
    }

    /// Returns trigger role.
    pub const fn trigger_role(&self) -> Role {
        Role::Button
    }

    /// Returns content role.
    pub const fn content_role(&self) -> Role {
        Role::Window
    }

    /// Returns whether activation handlers should run.
    pub const fn activation_enabled(&self) -> bool {
        !self.disabled
    }

    /// Returns resolved metrics.
    pub const fn metrics(&self) -> HoverCardMetrics {
        self.metrics
    }

    /// Returns resolved color intents.
    pub const fn colors(&self) -> HoverCardColors {
        self.colors
    }

    /// Returns resolved focus-ring metadata.
    pub const fn focus_ring(&self) -> FocusRing {
        self.focus_ring
    }

    /// Returns renderer-neutral overlay state.
    pub const fn overlay(&self) -> &OverlayResolvedState {
        &self.overlay
    }
}

/// A concrete GPUI hover card component.
#[derive(IntoElement)]
pub struct HoverCard {
    id: ElementId,
    trigger_label: SharedString,
    content: HoverCardContent,
    size: Size,
    disabled: bool,
    open: Option<bool>,
    default_open: bool,
    open_intent: HoverCardOpenIntent,
    placement_side: OverlayPlacementSide,
    placement_alignment: OverlayPlacementAlignment,
    delay: HoverCardDelayPolicy,
    outside_press_policy: OutsidePressPolicy,
    initial_focus_intent: InitialFocusIntent,
    focus_restore_intent: FocusRestoreIntent,
    tokens: ThemeTokens,
    on_open_change: Option<HoverCardOpenChangeHandler>,
}

enum HoverCardContent {
    Text(SharedString),
    Element(AnyElement),
}

struct HoverCardRuntime {
    open: bool,
    focus_open: bool,
    hovering_trigger: bool,
    hovering_content: bool,
    epoch: u64,
    trigger_focus: FocusHandle,
    content_focus: FocusHandle,
    delayed_task: Option<Task<()>>,
}

impl HoverCard {
    /// Creates a text hover card.
    pub fn new(
        id: impl Into<ElementId>,
        trigger_label: impl Into<SharedString>,
        content: impl Into<SharedString>,
    ) -> Self {
        Self {
            id: id.into(),
            trigger_label: trigger_label.into(),
            content: HoverCardContent::Text(content.into()),
            size: Size::Medium,
            disabled: false,
            open: None,
            default_open: false,
            open_intent: HoverCardOpenIntent::default(),
            placement_side: OverlayPlacementSide::Bottom,
            placement_alignment: OverlayPlacementAlignment::Center,
            delay: HoverCardDelayPolicy::default(),
            outside_press_policy: OutsidePressPolicy::DismissAndPassThrough,
            initial_focus_intent: InitialFocusIntent::None,
            focus_restore_intent: FocusRestoreIntent::None,
            tokens: ThemeTokens::default(),
            on_open_change: None,
        }
    }

    /// Creates a hover card with simple element content.
    pub fn element(
        id: impl Into<ElementId>,
        trigger_label: impl Into<SharedString>,
        content: impl IntoElement,
    ) -> Self {
        Self {
            id: id.into(),
            trigger_label: trigger_label.into(),
            content: HoverCardContent::Element(content.into_any_element()),
            size: Size::Medium,
            disabled: false,
            open: None,
            default_open: false,
            open_intent: HoverCardOpenIntent::default(),
            placement_side: OverlayPlacementSide::Bottom,
            placement_alignment: OverlayPlacementAlignment::Center,
            delay: HoverCardDelayPolicy::default(),
            outside_press_policy: OutsidePressPolicy::DismissAndPassThrough,
            initial_focus_intent: InitialFocusIntent::None,
            focus_restore_intent: FocusRestoreIntent::None,
            tokens: ThemeTokens::default(),
            on_open_change: None,
        }
    }

    /// Marks the hover card trigger as disabled.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Applies controlled open state.
    pub fn open(mut self, open: bool) -> Self {
        self.open = Some(open);
        self
    }

    /// Applies uncontrolled initial open state.
    pub fn default_open(mut self, default_open: bool) -> Self {
        self.default_open = default_open;
        self
    }

    /// Applies open intent.
    pub fn open_intent(mut self, intent: HoverCardOpenIntent) -> Self {
        self.open_intent = intent;
        self
    }

    /// Applies placement side.
    pub fn placement_side(mut self, side: OverlayPlacementSide) -> Self {
        self.placement_side = side;
        self
    }

    /// Applies placement alignment.
    pub fn placement_alignment(mut self, alignment: OverlayPlacementAlignment) -> Self {
        self.placement_alignment = alignment;
        self
    }

    /// Applies delay policy.
    pub fn delay(mut self, delay: HoverCardDelayPolicy) -> Self {
        self.delay = delay;
        self
    }

    /// Applies outside-press policy.
    pub fn outside_press_policy(mut self, policy: OutsidePressPolicy) -> Self {
        self.outside_press_policy = policy;
        self
    }

    /// Applies initial focus intent.
    pub fn initial_focus_intent(mut self, intent: InitialFocusIntent) -> Self {
        self.initial_focus_intent = intent;
        self
    }

    /// Applies focus restore intent.
    pub fn focus_restore_intent(mut self, intent: FocusRestoreIntent) -> Self {
        self.focus_restore_intent = intent;
        self
    }

    /// Applies a token bundle.
    pub fn tokens(mut self, tokens: ThemeTokens) -> Self {
        self.tokens = tokens;
        self
    }

    /// Registers an open-change handler with the next open value.
    pub fn on_open_change(
        mut self,
        handler: impl Fn(bool, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_open_change = Some(Rc::new(handler));
        self
    }

    /// Returns resolved hover card state.
    pub fn state(&self) -> HoverCardState {
        HoverCardState::resolve(
            self.content_kind(),
            self.size,
            self.disabled,
            self.open,
            self.default_open,
            self.open_intent,
            self.placement_side,
            self.placement_alignment,
            self.delay,
            self.outside_press_policy,
            self.initial_focus_intent.clone(),
            self.focus_restore_intent.clone(),
            self.tokens,
        )
    }

    fn content_kind(&self) -> HoverCardContentKind {
        match self.content {
            HoverCardContent::Text(_) => HoverCardContentKind::Text,
            HoverCardContent::Element(_) => HoverCardContentKind::Element,
        }
    }
}

impl Sizable for HoverCard {
    fn with_size(mut self, size: Size) -> Self {
        self.size = size;
        self
    }
}

impl RenderOnce for HoverCard {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let runtime = window.use_keyed_state(self.id.clone(), cx, |_, cx| HoverCardRuntime {
            open: self.default_open,
            focus_open: false,
            hovering_trigger: false,
            hovering_content: false,
            epoch: 0,
            trigger_focus: cx.focus_handle(),
            content_focus: cx.focus_handle(),
            delayed_task: None,
        });
        let runtime_open = runtime.read(cx).open;
        let trigger_focused = runtime.read(cx).trigger_focus.is_focused(window);
        let controlled_open = self.open;
        let focus_holds_open = controlled_open.is_none()
            && ((trigger_focused && self.open_intent.opens_on_focus())
                || runtime.read(cx).content_focus.contains_focused(window, cx));

        if focus_holds_open && (!runtime_open || !runtime.read(cx).focus_open) {
            runtime.update(cx, |runtime, _| {
                runtime.open = true;
                runtime.focus_open = true;
                runtime.epoch = runtime.epoch.wrapping_add(1);
                runtime.delayed_task = None;
            });
        }

        if controlled_open.is_none()
            && runtime.read(cx).focus_open
            && !focus_holds_open
            && !runtime.read(cx).hovering_trigger
            && !runtime.read(cx).hovering_content
        {
            runtime.update(cx, |runtime, _| {
                runtime.open = false;
                runtime.focus_open = false;
                runtime.epoch = runtime.epoch.wrapping_add(1);
                runtime.delayed_task = None;
            });
        }

        let runtime_open = runtime.read(cx).open;
        let open_state =
            resolve_overlay_open_state(controlled_open, runtime_open || focus_holds_open);
        let resolved_open = open_state.open();

        if open_state.controlled() && runtime_open != resolved_open {
            runtime.update(cx, |runtime, _| {
                runtime.open = resolved_open;
            });
        }

        let state = HoverCardState::resolve_with_open_mode(
            self.content_kind(),
            self.size,
            self.disabled,
            resolved_open,
            self.default_open,
            if controlled_open.is_some() {
                HoverCardOpenMode::Controlled
            } else {
                HoverCardOpenMode::Uncontrolled
            },
            self.open_intent,
            self.placement_side,
            self.placement_alignment,
            self.delay,
            self.outside_press_policy,
            self.initial_focus_intent.clone(),
            self.focus_restore_intent.clone(),
            self.tokens,
        );
        let id = self.id;
        let debug_id = id.to_string();
        let content_id: ElementId = (id.clone(), "content").into();
        let trigger_id: ElementId = (id.clone(), "trigger").into();
        let trigger_label = self.trigger_label;
        let content = self.content;
        let on_open_change = self.on_open_change;
        let metrics = state.metrics();
        let colors = state.colors();
        let focus_ring = state.focus_ring();
        let disabled = state.disabled();
        let open = state.open();
        let trigger_focus = runtime.read(cx).trigger_focus.clone();
        let content_focus = runtime.read(cx).content_focus.clone();
        let overlay_adapter = gpui_overlay_state(state.overlay());
        let placement = GpuiOverlayPlacement::resolve(
            OverlayPlacementInput::new(
                open_gpui_ui_core::OverlayAnchorInput::from_layout_bounds(open_gpui_ui_core::rect(
                    ui_point(ui_px(0.0), ui_px(0.0)),
                    ui_size(metrics.min_width(), metrics.trigger_height()),
                )),
                ui_size(metrics.min_width(), metrics.trigger_height()),
            )
            .with_side(state.placement_side())
            .with_alignment(state.placement_alignment())
            .with_offset(metrics.offset()),
            overlay_adapter.snap_margin(),
        );

        div()
            .id(id)
            .debug_selector({
                let debug_id = debug_id.clone();
                move || format!("hover-card:{debug_id}:root")
            })
            .relative()
            .flex()
            .flex_col()
            .items_start()
            .child(
                div()
                    .id(trigger_id)
                    .debug_selector({
                        let debug_id = debug_id.clone();
                        move || format!("hover-card:{debug_id}:trigger")
                    })
                    .min_h(gpui_px_from_ui(metrics.trigger_height()))
                    .px(gpui_px_from_ui(metrics.trigger_padding_x()))
                    .py(gpui_px_from_ui(metrics.trigger_padding_y()))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(gpui_px_from_ui(metrics.radius()))
                    .border_1()
                    .border_color(ThemeResolver::resolve(colors.trigger_border()))
                    .bg(ThemeResolver::resolve(colors.trigger_background()))
                    .text_color(ThemeResolver::resolve(colors.trigger_foreground()))
                    .text_size(gpui_px_from_ui(metrics.text_size()))
                    .line_height(gpui_px_from_ui(metrics.text_size()))
                    .focusable()
                    .track_focus(&trigger_focus)
                    .tab_stop(!disabled)
                    .ui_role(state.trigger_role())
                    .aria_label(trigger_label.clone())
                    .aria_selected(state.trigger_selected())
                    .aria_expanded(open)
                    .aria_disabled(disabled)
                    .focus_visible(move |style| style.shadow(focus_ring_shadow(focus_ring)))
                    .when(open, |this| {
                        let runtime = runtime.clone();
                        let on_open_change = on_open_change.clone();
                        let escape_policy = state.overlay().policy().clone();
                        this.on_key_down(move |event: &KeyDownEvent, window, cx| {
                            if event.keystroke.key.as_str() == "escape"
                                && escape_open_change(&escape_policy).is_some()
                            {
                                consume_overlay_event(window, cx);
                                close_hover_card(
                                    runtime.clone(),
                                    on_open_change.clone(),
                                    window,
                                    cx,
                                );
                            }
                        })
                    })
                    .when(disabled, |this| this.opacity(0.56).cursor_not_allowed())
                    .when(!disabled, |this| {
                        let runtime = runtime.clone();
                        let on_open_change = on_open_change.clone();
                        let delay = state.delay();
                        let opens_on_hover = state.open_intent().opens_on_hover();
                        let opens_manually = state.open_intent().opens_manually();
                        this.cursor_pointer()
                            .hover(move |style| {
                                style.bg(ThemeResolver::resolve(colors.trigger_hover_background()))
                            })
                            .on_hover({
                                let runtime = runtime.clone();
                                let on_open_change = on_open_change.clone();
                                move |hovered, window, cx| {
                                    if opens_on_hover {
                                        handle_hover_card_trigger_hover(
                                            runtime.clone(),
                                            *hovered,
                                            delay,
                                            on_open_change.clone(),
                                            window,
                                            cx,
                                        );
                                    }
                                }
                            })
                            .when(opens_manually, |this| {
                                let runtime = runtime.clone();
                                let on_open_change = on_open_change.clone();
                                this.on_click(move |_event: &ClickEvent, window, cx| {
                                    cx.stop_propagation();
                                    let next_open = !runtime.read(cx).open;
                                    set_hover_card_open(
                                        runtime.clone(),
                                        next_open,
                                        on_open_change.clone(),
                                        window,
                                        cx,
                                    );
                                })
                            })
                    })
                    .child(trigger_label),
            )
            .when(open, |this| {
                this.child(
                    deferred(
                        anchored()
                            .anchor(placement.anchor())
                            .offset(placement.offset())
                            .snap_to_window_with_margin(placement.snap_margin())
                            .child(hover_card_content_element(
                                content,
                                content_id.clone(),
                                state.clone(),
                                runtime.clone(),
                                content_focus.clone(),
                                on_open_change.clone(),
                                debug_id.clone(),
                            )),
                    )
                    .priority(overlay_adapter.deferred_priority()),
                )
            })
    }
}

fn hover_card_content_element(
    content: HoverCardContent,
    content_id: ElementId,
    state: HoverCardState,
    runtime: Entity<HoverCardRuntime>,
    content_focus: FocusHandle,
    on_open_change: Option<HoverCardOpenChangeHandler>,
    debug_id: String,
) -> impl IntoElement {
    let metrics = state.metrics();
    let colors = state.colors();
    let outside_change = outside_press_open_change(state.overlay().policy());
    let delay = state.delay();
    let escape_policy = state.overlay().policy().clone();

    div()
        .id(content_id)
        .debug_selector(move || format!("hover-card:{debug_id}:content"))
        .min_w(gpui_px_from_ui(metrics.min_width()))
        .max_w(gpui_px_from_ui(metrics.max_width()))
        .max_h(gpui_px_from_ui(metrics.max_height()))
        .overflow_y_scroll()
        .p(gpui_px_from_ui(metrics.content_padding()))
        .flex()
        .flex_col()
        .gap_2()
        .rounded(gpui_px_from_ui(metrics.radius()))
        .border_1()
        .border_color(ThemeResolver::resolve(colors.border()))
        .bg(ThemeResolver::resolve(colors.background()))
        .text_color(ThemeResolver::resolve(colors.foreground()))
        .text_size(gpui_px_from_ui(metrics.text_size()))
        .line_height(gpui_px_from_ui(metrics.text_size()))
        .shadow_lg()
        .occlude()
        .tab_group()
        .focusable()
        .track_focus(&content_focus)
        .ui_role(state.content_role())
        .on_hover({
            let runtime = runtime.clone();
            let on_open_change = on_open_change.clone();
            move |hovered, window, cx| {
                handle_hover_card_content_hover(
                    runtime.clone(),
                    *hovered,
                    delay,
                    on_open_change.clone(),
                    window,
                    cx,
                );
            }
        })
        .on_key_down({
            let runtime = runtime.clone();
            let on_open_change = on_open_change.clone();
            move |event: &KeyDownEvent, window, cx| {
                if event.keystroke.key.as_str() == "escape"
                    && escape_open_change(&escape_policy).is_some()
                {
                    consume_overlay_event(window, cx);
                    close_hover_card(runtime.clone(), on_open_change.clone(), window, cx);
                }
            }
        })
        .when(outside_change.is_some(), |this| {
            let runtime = runtime.clone();
            let on_open_change = on_open_change.clone();
            this.on_mouse_down_out(move |_, window, cx| {
                close_hover_card(runtime.clone(), on_open_change.clone(), window, cx);
            })
        })
        .children(children_from_content(content))
}

fn handle_hover_card_trigger_hover(
    runtime: Entity<HoverCardRuntime>,
    hovering: bool,
    delay: HoverCardDelayPolicy,
    on_open_change: Option<HoverCardOpenChangeHandler>,
    window: &mut Window,
    cx: &mut App,
) {
    runtime.update(cx, |runtime, _| {
        runtime.hovering_trigger = hovering;
    });

    if hovering {
        schedule_hover_card_open(runtime, delay.open_delay(), on_open_change, window, cx);
    } else if !runtime.read(cx).hovering_content && !hover_card_has_focus(&runtime, window, cx) {
        schedule_hover_card_close(runtime, delay.close_delay(), on_open_change, window, cx);
    }
}

fn handle_hover_card_content_hover(
    runtime: Entity<HoverCardRuntime>,
    hovering: bool,
    delay: HoverCardDelayPolicy,
    on_open_change: Option<HoverCardOpenChangeHandler>,
    window: &mut Window,
    cx: &mut App,
) {
    runtime.update(cx, |runtime, _| {
        runtime.hovering_content = hovering;
        runtime.epoch = runtime.epoch.wrapping_add(1);
        runtime.delayed_task = None;
    });

    if !hovering
        && !runtime.read(cx).hovering_trigger
        && !hover_card_has_focus(&runtime, window, cx)
    {
        schedule_hover_card_close(runtime, delay.close_delay(), on_open_change, window, cx);
    }
}

fn schedule_hover_card_open(
    runtime: Entity<HoverCardRuntime>,
    delay: std::time::Duration,
    on_open_change: Option<HoverCardOpenChangeHandler>,
    window: &mut Window,
    cx: &mut App,
) {
    runtime.update(cx, |runtime, _| {
        runtime.epoch = runtime.epoch.wrapping_add(1);
    });
    let epoch = runtime.read(cx).epoch;

    if delay.is_zero() {
        set_hover_card_open(runtime, true, on_open_change, window, cx);
        return;
    }

    let task = window.spawn(cx, {
        let runtime = runtime.clone();
        async move |cx| {
            cx.background_executor().timer(delay).await;
            cx.update(|window, cx| {
                if runtime.read(cx).epoch == epoch {
                    set_hover_card_open(runtime, true, on_open_change, window, cx);
                }
            })
            .ok();
        }
    });
    runtime.update(cx, |runtime, _| {
        runtime.delayed_task = Some(task);
    });
}

fn schedule_hover_card_close(
    runtime: Entity<HoverCardRuntime>,
    delay: std::time::Duration,
    on_open_change: Option<HoverCardOpenChangeHandler>,
    window: &mut Window,
    cx: &mut App,
) {
    runtime.update(cx, |runtime, _| {
        runtime.epoch = runtime.epoch.wrapping_add(1);
    });
    let epoch = runtime.read(cx).epoch;

    if delay.is_zero() {
        close_hover_card(runtime, on_open_change, window, cx);
        return;
    }

    let task = window.spawn(cx, {
        let runtime = runtime.clone();
        async move |cx| {
            cx.background_executor().timer(delay).await;
            cx.update(|window, cx| {
                let should_close = {
                    let runtime_state = runtime.read(cx);
                    runtime_state.epoch == epoch
                        && !runtime_state.hovering_trigger
                        && !runtime_state.hovering_content
                        && !runtime_state.trigger_focus.contains_focused(window, cx)
                        && !runtime_state.content_focus.contains_focused(window, cx)
                };
                if should_close {
                    close_hover_card(runtime, on_open_change, window, cx);
                }
            })
            .ok();
        }
    });
    runtime.update(cx, |runtime, _| {
        runtime.delayed_task = Some(task);
    });
}

fn close_hover_card(
    runtime: Entity<HoverCardRuntime>,
    on_open_change: Option<HoverCardOpenChangeHandler>,
    window: &mut Window,
    cx: &mut App,
) {
    set_hover_card_open(runtime, false, on_open_change, window, cx);
}

fn set_hover_card_open(
    runtime: Entity<HoverCardRuntime>,
    open: bool,
    on_open_change: Option<HoverCardOpenChangeHandler>,
    window: &mut Window,
    cx: &mut App,
) {
    let changed = runtime.read(cx).open != open;
    runtime.update(cx, |runtime, _| {
        runtime.open = open;
        if !open {
            runtime.focus_open = false;
        }
        runtime.delayed_task = None;
        runtime.epoch = runtime.epoch.wrapping_add(1);
    });
    if changed {
        emit_overlay_open_change(open, on_open_change.as_deref(), window, cx);
    }
}

fn hover_card_has_focus(runtime: &Entity<HoverCardRuntime>, window: &Window, cx: &App) -> bool {
    let runtime = runtime.read(cx);
    runtime.trigger_focus.contains_focused(window, cx)
        || runtime.content_focus.contains_focused(window, cx)
}

fn children_from_content(content: HoverCardContent) -> Vec<AnyElement> {
    match content {
        HoverCardContent::Text(text) => vec![div().child(text).into_any_element()],
        HoverCardContent::Element(element) => vec![element],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_resolver_preserves_uncontrolled_open_mode_with_runtime_open() {
        let state = HoverCardState::resolve_with_open_mode(
            HoverCardContentKind::Text,
            Size::Medium,
            false,
            true,
            false,
            HoverCardOpenMode::Uncontrolled,
            HoverCardOpenIntent::HoverOrFocus,
            OverlayPlacementSide::Bottom,
            OverlayPlacementAlignment::Center,
            HoverCardDelayPolicy::default(),
            OutsidePressPolicy::DismissAndPassThrough,
            InitialFocusIntent::None,
            FocusRestoreIntent::None,
            ThemeTokens::default(),
        );

        assert!(state.open());
        assert!(!state.default_open());
        assert_eq!(state.open_mode(), HoverCardOpenMode::Uncontrolled);
    }
}
