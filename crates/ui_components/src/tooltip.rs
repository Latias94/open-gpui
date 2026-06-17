//! Tooltip component.

use std::time::Duration;

use open_gpui::prelude::*;
use open_gpui::{
    AnyElement, App, ElementId, IntoElement, ParentElement, RenderOnce, SharedString,
    StatefulInteractiveElement, Styled, Window, div,
};
use open_gpui_ui_core::{
    InitialFocusIntent, OverlayLayerKind, OverlayPlacementAlignment, OverlayPlacementSide,
    OverlayPresence, Role, Sizable, Size, ThemeTokens, UiPx, ui_px,
};

use crate::a11y::UiA11yElementExt;
use crate::color::ColorIntent;
use crate::overlay::{GpuiOverlayAdapterConfig, OverlayResolvedState};
use crate::theme::ThemeResolver;

/// Open affordance for a tooltip trigger.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TooltipOpenIntent {
    /// Tooltip opens from either pointer hover or keyboard focus.
    #[default]
    HoverOrFocus,
    /// Tooltip opens from pointer hover only.
    Hover,
    /// Tooltip opens from keyboard focus only.
    Focus,
    /// Tooltip is controlled externally.
    Manual,
}

impl TooltipOpenIntent {
    /// Returns whether pointer hover may open the tooltip.
    pub const fn opens_on_hover(self) -> bool {
        matches!(self, Self::HoverOrFocus | Self::Hover)
    }

    /// Returns whether keyboard focus may open the tooltip.
    pub const fn opens_on_focus(self) -> bool {
        matches!(self, Self::HoverOrFocus | Self::Focus)
    }
}

/// Tooltip delay policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TooltipDelayPolicy {
    open_delay: Duration,
    close_delay: Duration,
    skip_delay: Duration,
}

impl TooltipDelayPolicy {
    /// Creates a tooltip delay policy.
    pub const fn new(open_delay: Duration, close_delay: Duration, skip_delay: Duration) -> Self {
        Self {
            open_delay,
            close_delay,
            skip_delay,
        }
    }

    /// Returns the delay before showing the tooltip.
    pub const fn open_delay(self) -> Duration {
        self.open_delay
    }

    /// Returns the delay before hiding the tooltip after hover leaves.
    pub const fn close_delay(self) -> Duration {
        self.close_delay
    }

    /// Returns the delay-group window where reopening can skip the show delay.
    pub const fn skip_delay(self) -> Duration {
        self.skip_delay
    }
}

impl Default for TooltipDelayPolicy {
    fn default() -> Self {
        Self {
            open_delay: Duration::from_millis(500),
            close_delay: Duration::from_millis(100),
            skip_delay: Duration::from_millis(300),
        }
    }
}

/// Tooltip content kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TooltipContentKind {
    /// Plain text content.
    #[default]
    Text,
    /// Simple element content.
    Element,
}

/// Resolved tooltip color intents.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TooltipColors {
    pub(crate) background: ColorIntent,
    pub(crate) foreground: ColorIntent,
    pub(crate) border: ColorIntent,
}

impl TooltipColors {
    /// Returns the background color intent.
    pub const fn background(self) -> ColorIntent {
        self.background
    }

    /// Returns the foreground color intent.
    pub const fn foreground(self) -> ColorIntent {
        self.foreground
    }

    /// Returns the border color intent.
    pub const fn border(self) -> ColorIntent {
        self.border
    }
}

/// Resolved tooltip metrics.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TooltipMetrics {
    padding_x: UiPx,
    padding_y: UiPx,
    radius: UiPx,
    text_size: UiPx,
    max_width: UiPx,
}

impl TooltipMetrics {
    /// Resolves metrics from the shared foundation size vocabulary.
    pub const fn from_size(size: Size) -> Self {
        Self {
            padding_x: size.button_px(),
            padding_y: ui_px(6.0),
            radius: size.control_radius(),
            text_size: size.control_text_px(),
            max_width: ui_px(320.0),
        }
    }

    /// Returns horizontal padding.
    pub const fn padding_x(self) -> UiPx {
        self.padding_x
    }

    /// Returns vertical padding.
    pub const fn padding_y(self) -> UiPx {
        self.padding_y
    }

    /// Returns corner radius.
    pub const fn radius(self) -> UiPx {
        self.radius
    }

    /// Returns text size.
    pub const fn text_size(self) -> UiPx {
        self.text_size
    }

    /// Returns max content width.
    pub const fn max_width(self) -> UiPx {
        self.max_width
    }
}

/// Resolved tooltip state used by tests, demos, and rendering.
#[derive(Debug, Clone, PartialEq)]
pub struct TooltipState {
    content_kind: TooltipContentKind,
    size: Size,
    disabled: bool,
    open: bool,
    open_intent: TooltipOpenIntent,
    placement_side: OverlayPlacementSide,
    placement_alignment: OverlayPlacementAlignment,
    delay: TooltipDelayPolicy,
    metrics: TooltipMetrics,
    colors: TooltipColors,
    overlay: OverlayResolvedState,
}

impl TooltipState {
    /// Resolves the public state for a tooltip.
    pub fn resolve(
        content_kind: TooltipContentKind,
        size: Size,
        disabled: bool,
        open: bool,
        open_intent: TooltipOpenIntent,
        placement_side: OverlayPlacementSide,
        placement_alignment: OverlayPlacementAlignment,
        delay: TooltipDelayPolicy,
        tokens: ThemeTokens,
    ) -> Self {
        let presence = if open && !disabled {
            OverlayPresence::open()
        } else {
            OverlayPresence::hidden()
        };
        let overlay = GpuiOverlayAdapterConfig::new(OverlayLayerKind::Tooltip, presence)
            .initial_focus_intent(InitialFocusIntent::None)
            .resolved_state();

        Self {
            content_kind,
            size,
            disabled,
            open: open && !disabled,
            open_intent,
            placement_side,
            placement_alignment,
            delay,
            metrics: TooltipMetrics::from_size(size),
            colors: ThemeResolver::tooltip_colors(tokens),
            overlay,
        }
    }

    /// Returns content kind.
    pub const fn content_kind(&self) -> TooltipContentKind {
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

    /// Returns whether tooltip content is open.
    pub const fn open(&self) -> bool {
        self.open
    }

    /// Returns open intent.
    pub const fn open_intent(&self) -> TooltipOpenIntent {
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
    pub const fn delay(&self) -> TooltipDelayPolicy {
        self.delay
    }

    /// Returns whether tooltip content is descriptive-only.
    pub const fn descriptive(&self) -> bool {
        true
    }

    /// Returns whether tooltip content is interactive.
    pub const fn interactive_content(&self) -> bool {
        false
    }

    /// Returns the current accessibility role used by the GPUI adapter.
    pub const fn role(&self) -> Role {
        Role::Label
    }

    /// Returns resolved metrics.
    pub const fn metrics(&self) -> TooltipMetrics {
        self.metrics
    }

    /// Returns resolved color intents.
    pub const fn colors(&self) -> TooltipColors {
        self.colors
    }

    /// Returns renderer-neutral overlay state.
    pub const fn overlay(&self) -> &OverlayResolvedState {
        &self.overlay
    }
}

/// Concrete GPUI tooltip surface.
#[derive(IntoElement)]
pub struct Tooltip {
    id: ElementId,
    content: TooltipContent,
    size: Size,
    disabled: bool,
    open: bool,
    open_intent: TooltipOpenIntent,
    placement_side: OverlayPlacementSide,
    placement_alignment: OverlayPlacementAlignment,
    delay: TooltipDelayPolicy,
    tokens: ThemeTokens,
}

enum TooltipContent {
    Text(SharedString),
    Element(AnyElement),
}

impl Tooltip {
    /// Creates a text tooltip.
    pub fn new(id: impl Into<ElementId>, text: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            content: TooltipContent::Text(text.into()),
            size: Size::Medium,
            disabled: false,
            open: false,
            open_intent: TooltipOpenIntent::default(),
            placement_side: OverlayPlacementSide::Top,
            placement_alignment: OverlayPlacementAlignment::Center,
            delay: TooltipDelayPolicy::default(),
            tokens: ThemeTokens::default(),
        }
    }

    /// Creates a tooltip with simple element content.
    pub fn element(id: impl Into<ElementId>, content: impl IntoElement) -> Self {
        Self {
            id: id.into(),
            content: TooltipContent::Element(content.into_any_element()),
            size: Size::Medium,
            disabled: false,
            open: false,
            open_intent: TooltipOpenIntent::default(),
            placement_side: OverlayPlacementSide::Top,
            placement_alignment: OverlayPlacementAlignment::Center,
            delay: TooltipDelayPolicy::default(),
            tokens: ThemeTokens::default(),
        }
    }

    /// Marks the tooltip trigger as disabled.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Applies controlled open state.
    pub fn open(mut self, open: bool) -> Self {
        self.open = open;
        self
    }

    /// Applies open intent.
    pub fn open_intent(mut self, intent: TooltipOpenIntent) -> Self {
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
    pub fn delay(mut self, delay: TooltipDelayPolicy) -> Self {
        self.delay = delay;
        self
    }

    /// Applies a token bundle.
    pub fn tokens(mut self, tokens: ThemeTokens) -> Self {
        self.tokens = tokens;
        self
    }

    /// Returns resolved tooltip state.
    pub fn state(&self) -> TooltipState {
        TooltipState::resolve(
            self.content_kind(),
            self.size,
            self.disabled,
            self.open,
            self.open_intent,
            self.placement_side,
            self.placement_alignment,
            self.delay,
            self.tokens,
        )
    }

    fn content_kind(&self) -> TooltipContentKind {
        match self.content {
            TooltipContent::Text(_) => TooltipContentKind::Text,
            TooltipContent::Element(_) => TooltipContentKind::Element,
        }
    }
}

impl Sizable for Tooltip {
    fn with_size(mut self, size: Size) -> Self {
        self.size = size;
        self
    }
}

impl RenderOnce for Tooltip {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let state = self.state();
        let metrics = state.metrics();
        let colors = state.colors();
        let id = self.id;
        let accessible_label = accessible_label_for_content(&self.content);
        let children = children_from_content(self.content);

        div()
            .id(id)
            .max_w(metrics.max_width())
            .px(metrics.padding_x())
            .py(metrics.padding_y())
            .rounded(metrics.radius())
            .border_1()
            .border_color(ThemeResolver::resolve(colors.border()))
            .bg(ThemeResolver::resolve(colors.background()))
            .text_color(ThemeResolver::resolve(colors.foreground()))
            .text_size(metrics.text_size())
            .line_height(metrics.text_size())
            .shadow_lg()
            .ui_role(state.role())
            .aria_label(accessible_label)
            .children(children)
    }
}

fn accessible_label_for_content(content: &TooltipContent) -> SharedString {
    match content {
        TooltipContent::Text(text) => text.clone(),
        TooltipContent::Element(_) => SharedString::from("Tooltip"),
    }
}

fn children_from_content(content: TooltipContent) -> Vec<AnyElement> {
    match content {
        TooltipContent::Text(text) => vec![div().child(text).into_any_element()],
        TooltipContent::Element(element) => vec![element],
    }
}
