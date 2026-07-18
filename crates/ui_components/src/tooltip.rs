//! Tooltip component.

use crate::geometry::gpui_px_from_ui;
use crate::kbd::Kbd;
use std::time::Duration;

use open_gpui::prelude::*;
use open_gpui::{
    Action, AnyElement, AnyView, App, Context, ElementId, IntoElement, KeyBinding, KeyContext,
    ParentElement, Render, RenderOnce, SharedString, Styled, Window, div,
};
use open_gpui_ui_core::{
    InitialFocusIntent, OverlayAnchorInput, OverlayLayerKind, OverlayPlacementAlignment,
    OverlayPlacementInput, OverlayPlacementSide, OverlayPresence, Role, SemanticDescriptor,
    Sizable, Size, ThemeTokens, UiPx, rect, ui_point, ui_px, ui_size,
};

use crate::a11y::UiA11yElementExt;
use crate::color::ColorIntent;
use crate::overlay::{
    GpuiOverlayAdapterConfig, GpuiOverlayPlacement, OverlayInsideRegionId, OverlayLayerBinding,
    OverlayLayerRegistration, OverlayOwnership, OverlayResolvedState, WindowOverlayRuntime,
    gpui_overlay_state, gpui_relative_overlay_layer,
};
use crate::theme::{
    ThemeContext, ThemeResolver, ThemeScope, gpui_elevation_shadow, scoped_theme_view_builder,
};

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
    /// Returns a stable label.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::HoverOrFocus => "hover or focus",
            Self::Hover => "hover",
            Self::Focus => "focus",
            Self::Manual => "manual",
        }
    }

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

impl TooltipContentKind {
    /// Returns a stable label.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Text => "text",
            Self::Element => "element",
        }
    }
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
    active_tooltip_surface: bool,
}

enum TooltipContent {
    Text(SharedString),
    Element(AnyElement),
}

struct TooltipRuntime {
    overlay_binding: Option<OverlayLayerBinding>,
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
            active_tooltip_surface: false,
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
            active_tooltip_surface: false,
        }
    }

    /// Creates a GPUI tooltip-builder closure for attaching text tooltips to interactive elements.
    pub fn text(
        text: impl Into<SharedString>,
    ) -> impl Fn(&mut Window, &mut App) -> AnyView + 'static {
        let text = text.into();
        move |_, cx| cx.new(|_| TextTooltipView { text: text.clone() }).into()
    }

    /// Captures an explicit opening theme for a builder attached directly to GPUI interactivity.
    ///
    /// Theme-aware components such as [`crate::Button`] and [`crate::IconButton`] apply this
    /// capture automatically. Direct GPUI tooltip callers must opt in because GPUI invokes the
    /// builder after the trigger's render scope has exited.
    pub fn scoped(
        context: ThemeContext,
        build: impl Fn(&mut Window, &mut App) -> AnyView + 'static,
    ) -> impl Fn(&mut Window, &mut App) -> AnyView + 'static {
        scoped_theme_view_builder(context, build)
    }

    /// Creates a tooltip builder that appends the active keybinding for an action when available.
    pub fn for_action(
        label: impl Into<SharedString>,
        action: impl Action + 'static,
    ) -> impl Fn(&mut Window, &mut App) -> AnyView + 'static {
        action_tooltip_builder(label, Box::new(action), None)
    }

    /// Creates a tooltip builder that appends the keybinding for an action in a specific key context.
    pub fn for_action_in_context<C, E>(
        label: impl Into<SharedString>,
        action: impl Action + 'static,
        key_context: C,
    ) -> impl Fn(&mut Window, &mut App) -> AnyView + 'static
    where
        C: TryInto<KeyContext, Error = E>,
    {
        action_tooltip_builder(label, Box::new(action), key_context.try_into().ok())
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

    fn active_tooltip_surface(mut self) -> Self {
        self.active_tooltip_surface = true;
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

fn action_tooltip_builder(
    label: impl Into<SharedString>,
    action: Box<dyn Action>,
    key_context: Option<KeyContext>,
) -> impl Fn(&mut Window, &mut App) -> AnyView + 'static {
    let label = label.into();
    move |window, cx| {
        let key_binding =
            key_binding_text_for_action(window, action.as_ref(), key_context.as_ref());
        cx.new(|_| ActionTooltipView {
            label: label.clone(),
            key_binding,
        })
        .into()
    }
}

fn key_binding_text_for_action(
    window: &Window,
    action: &dyn Action,
    key_context: Option<&KeyContext>,
) -> Option<SharedString> {
    let binding = match key_context {
        Some(key_context) => {
            window.highest_precedence_binding_for_action_in_context(action, key_context.clone())
        }
        None => window.highest_precedence_binding_for_action(action),
    }?;

    Some(key_binding_text(&binding).into())
}

fn key_binding_text(binding: &KeyBinding) -> String {
    binding
        .keystrokes()
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(" ")
}

struct TextTooltipView {
    text: SharedString,
}

impl Render for TextTooltipView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        Tooltip::new("tooltip", self.text.clone())
            .open(true)
            .active_tooltip_surface()
    }
}

struct ActionTooltipView {
    label: SharedString,
    key_binding: Option<SharedString>,
}

impl Render for ActionTooltipView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        Tooltip::element(
            "tooltip",
            div()
                .flex()
                .items_center()
                .gap_2()
                .child(div().child(self.label.clone()))
                .when_some(self.key_binding.clone(), |this, key_binding| {
                    this.child(Kbd::new("tooltip-keybinding", key_binding).xsmall())
                }),
        )
        .open(true)
        .active_tooltip_surface()
    }
}

impl Sizable for Tooltip {
    fn with_size(mut self, size: Size) -> Self {
        self.size = size;
        self
    }
}

impl RenderOnce for Tooltip {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let runtime = window.use_keyed_state(self.id.clone(), cx, |_, _| TooltipRuntime {
            overlay_binding: None,
        });
        let state = self.state();
        let metrics = state.metrics();
        let active_tooltip_surface = self.active_tooltip_surface;
        let id = self.id;
        let debug_id = id.to_string();
        let accessible_label = accessible_label_for_content(&self.content);
        let children = children_from_content(self.content);
        let content_id: ElementId = (id.clone(), "content").into();
        let window_overlay_runtime = WindowOverlayRuntime::for_window(window, cx);
        let registration = OverlayLayerRegistration::new(
            format!("tooltip:{debug_id}"),
            state.overlay().policy().clone(),
            OverlayOwnership::Controlled,
        );
        let existing_binding = runtime.read(cx).overlay_binding.clone();
        let overlay_binding = window_overlay_runtime
            .bind_component_layer(
                &runtime,
                existing_binding.as_ref(),
                registration,
                window,
                cx,
            )
            .expect("tooltip overlay registration should remain valid");
        if existing_binding.is_none() {
            runtime.update(cx, |runtime, _| {
                runtime.overlay_binding = Some(overlay_binding.clone());
            });
        }
        let overlay_adapter = gpui_overlay_state(state.overlay());
        let placement = GpuiOverlayPlacement::resolve(
            OverlayPlacementInput::new(
                OverlayAnchorInput::from_layout_bounds(rect(
                    ui_point(UiPx::ZERO, UiPx::ZERO),
                    ui_size(UiPx::ONE, UiPx::ONE),
                )),
                ui_size(
                    metrics.max_width(),
                    metrics.text_size() + metrics.padding_y() * 2.0,
                ),
            )
            .with_side(state.placement_side())
            .with_alignment(state.placement_alignment())
            .with_offset(ui_px(4.0)),
            overlay_adapter.snap_margin(),
        );
        let layer = overlay_adapter.should_render_deferred_layer().then(|| {
            if active_tooltip_surface {
                let opening_theme = overlay_binding
                    .opening_theme()
                    .expect("an open tooltip must capture its opening theme");
                ThemeScope::captured(
                    format!(
                        "overlay-theme:{}",
                        overlay_binding.lease().layer_id().as_str()
                    ),
                    opening_theme.clone(),
                    window_overlay_runtime.surface(
                        &overlay_binding,
                        OverlayInsideRegionId::new("surface"),
                        format!("tooltip:{debug_id}:surface-runtime"),
                        tooltip_surface_element(
                            content_id,
                            debug_id.clone(),
                            state,
                            accessible_label,
                            children,
                            &opening_theme,
                        ),
                    ),
                )
                .into_any_element()
            } else {
                gpui_relative_overlay_layer(
                    &overlay_adapter,
                    &placement,
                    &overlay_binding,
                    |opening_theme| {
                        window_overlay_runtime
                            .surface(
                                &overlay_binding,
                                OverlayInsideRegionId::new("surface"),
                                format!("tooltip:{debug_id}:surface-runtime"),
                                tooltip_surface_element(
                                    content_id,
                                    debug_id.clone(),
                                    state,
                                    accessible_label,
                                    children,
                                    opening_theme,
                                ),
                            )
                            .into_any_element()
                    },
                )
            }
        });

        div()
            .id(id)
            .debug_selector({
                let debug_id = debug_id.clone();
                move || format!("tooltip:{debug_id}:root")
            })
            .relative()
            .when_some(layer, |this, layer| this.child(layer))
    }
}

fn tooltip_surface_element(
    content_id: ElementId,
    debug_id: String,
    state: TooltipState,
    accessible_label: SharedString,
    children: Vec<AnyElement>,
    theme: &ThemeContext,
) -> impl IntoElement {
    let metrics = state.metrics();
    let colors = state.colors();
    let semantics = SemanticDescriptor::new(state.role()).with_label(accessible_label.as_ref());

    div()
        .id(content_id)
        .debug_selector(move || format!("tooltip:{debug_id}:content"))
        .max_w(gpui_px_from_ui(metrics.max_width()))
        .px(gpui_px_from_ui(metrics.padding_x()))
        .py(gpui_px_from_ui(metrics.padding_y()))
        .rounded(gpui_px_from_ui(metrics.radius()))
        .border_1()
        .border_color(theme.resolve(colors.border()))
        .bg(theme.resolve(colors.background()))
        .text_color(theme.resolve(colors.foreground()))
        .text_size(gpui_px_from_ui(metrics.text_size()))
        .line_height(gpui_px_from_ui(metrics.text_size()))
        .shadow(gpui_elevation_shadow(ThemeResolver::tooltip_elevation(
            theme,
        )))
        .ui_semantics(&semantics)
        .children(children)
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
