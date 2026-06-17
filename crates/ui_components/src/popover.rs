//! Popover component.

use crate::geometry::gpui_px_from_ui;
use std::rc::Rc;

use open_gpui::prelude::*;
use open_gpui::{
    AnyElement, App, ClickEvent, ElementId, IntoElement, KeyDownEvent, ParentElement, RenderOnce,
    SharedString, StatefulInteractiveElement, Styled, Window, anchored, deferred, div,
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
    GpuiOverlayAdapterConfig, GpuiOverlayPlacement, OverlayResolvedState, gpui_overlay_state,
    outside_press_open_change,
};
use crate::theme::ThemeResolver;

/// Popover open-state ownership.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PopoverOpenMode {
    /// Open state is owned by the component adapter after initialization.
    #[default]
    Uncontrolled,
    /// Open state is provided by the caller.
    Controlled,
}

/// Resolved popover color intents.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PopoverColors {
    pub(crate) background: ColorIntent,
    pub(crate) foreground: ColorIntent,
    pub(crate) border: ColorIntent,
    pub(crate) trigger_background: ColorIntent,
    pub(crate) trigger_hover_background: ColorIntent,
    pub(crate) trigger_foreground: ColorIntent,
    pub(crate) trigger_border: ColorIntent,
    pub(crate) focus_ring: ColorIntent,
}

impl PopoverColors {
    /// Returns content background color intent.
    pub const fn background(self) -> ColorIntent {
        self.background
    }

    /// Returns content foreground color intent.
    pub const fn foreground(self) -> ColorIntent {
        self.foreground
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

/// Resolved popover metrics.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PopoverMetrics {
    trigger_height: UiPx,
    trigger_padding_x: UiPx,
    trigger_padding_y: UiPx,
    content_padding: UiPx,
    radius: UiPx,
    text_size: UiPx,
    min_width: UiPx,
    max_width: UiPx,
}

impl PopoverMetrics {
    /// Resolves metrics from the shared foundation size vocabulary.
    pub const fn from_size(size: Size) -> Self {
        Self {
            trigger_height: size.button_h(),
            trigger_padding_x: size.button_px(),
            trigger_padding_y: size.button_py(),
            content_padding: size.button_px(),
            radius: size.control_radius(),
            text_size: size.control_text_px(),
            min_width: ui_px(220.0),
            max_width: ui_px(360.0),
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

    /// Returns text size.
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
}

/// Resolved popover state used by tests, demos, and rendering.
#[derive(Debug, Clone, PartialEq)]
pub struct PopoverState {
    size: Size,
    disabled: bool,
    open: bool,
    default_open: bool,
    open_mode: PopoverOpenMode,
    placement_side: OverlayPlacementSide,
    placement_alignment: OverlayPlacementAlignment,
    outside_press_policy: OutsidePressPolicy,
    initial_focus_intent: InitialFocusIntent,
    focus_restore_intent: FocusRestoreIntent,
    trigger_selected: bool,
    metrics: PopoverMetrics,
    colors: PopoverColors,
    focus_ring: FocusRing,
    overlay: OverlayResolvedState,
}

impl PopoverState {
    /// Resolves the public state for a popover.
    #[allow(clippy::too_many_arguments)]
    pub fn resolve(
        size: Size,
        disabled: bool,
        open: Option<bool>,
        default_open: bool,
        placement_side: OverlayPlacementSide,
        placement_alignment: OverlayPlacementAlignment,
        outside_press_policy: OutsidePressPolicy,
        initial_focus_intent: InitialFocusIntent,
        focus_restore_intent: FocusRestoreIntent,
        tokens: ThemeTokens,
    ) -> Self {
        let open_mode = if open.is_some() {
            PopoverOpenMode::Controlled
        } else {
            PopoverOpenMode::Uncontrolled
        };
        let open = open.unwrap_or(default_open) && !disabled;
        let presence = if open {
            OverlayPresence::open()
        } else {
            OverlayPresence::hidden()
        };
        let overlay =
            GpuiOverlayAdapterConfig::new(OverlayLayerKind::NonModalDismissible, presence)
                .outside_press_policy(outside_press_policy)
                .initial_focus_intent(initial_focus_intent.clone())
                .focus_restore_intent(focus_restore_intent.clone())
                .resolved_state();
        let colors = ThemeResolver::popover_colors(tokens, open);

        Self {
            size,
            disabled,
            open,
            default_open,
            open_mode,
            placement_side,
            placement_alignment,
            outside_press_policy,
            initial_focus_intent,
            focus_restore_intent,
            trigger_selected: open,
            metrics: PopoverMetrics::from_size(size),
            colors,
            focus_ring: FocusRing::from_color(colors.focus_ring()),
            overlay,
        }
    }

    /// Returns foundation size.
    pub const fn size(&self) -> Size {
        self.size
    }

    /// Returns whether the trigger is disabled.
    pub const fn disabled(&self) -> bool {
        self.disabled
    }

    /// Returns whether popover content is open.
    pub const fn open(&self) -> bool {
        self.open
    }

    /// Returns uncontrolled initial open state.
    pub const fn default_open(&self) -> bool {
        self.default_open
    }

    /// Returns open-state ownership.
    pub const fn open_mode(&self) -> PopoverOpenMode {
        self.open_mode
    }

    /// Returns preferred placement side.
    pub const fn placement_side(&self) -> OverlayPlacementSide {
        self.placement_side
    }

    /// Returns preferred placement alignment.
    pub const fn placement_alignment(&self) -> OverlayPlacementAlignment {
        self.placement_alignment
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
    pub const fn metrics(&self) -> PopoverMetrics {
        self.metrics
    }

    /// Returns resolved color intents.
    pub const fn colors(&self) -> PopoverColors {
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

/// A concrete GPUI popover component.
#[derive(IntoElement)]
pub struct Popover {
    id: ElementId,
    trigger_label: SharedString,
    content: PopoverContent,
    size: Size,
    disabled: bool,
    open: Option<bool>,
    default_open: bool,
    placement_side: OverlayPlacementSide,
    placement_alignment: OverlayPlacementAlignment,
    outside_press_policy: OutsidePressPolicy,
    initial_focus_intent: InitialFocusIntent,
    focus_restore_intent: FocusRestoreIntent,
    tokens: ThemeTokens,
    on_escape_close: Option<Rc<dyn Fn(bool, &mut Window, &mut App)>>,
    on_open_change: Option<Rc<dyn Fn(bool, &mut Window, &mut App)>>,
}

enum PopoverContent {
    Text(SharedString),
    Element(AnyElement),
}

#[derive(Debug, Clone, Copy)]
struct PopoverRuntime {
    open: bool,
}

impl Popover {
    /// Creates a text popover.
    pub fn new(
        id: impl Into<ElementId>,
        trigger_label: impl Into<SharedString>,
        content: impl Into<SharedString>,
    ) -> Self {
        Self {
            id: id.into(),
            trigger_label: trigger_label.into(),
            content: PopoverContent::Text(content.into()),
            size: Size::Medium,
            disabled: false,
            open: None,
            default_open: false,
            placement_side: OverlayPlacementSide::Bottom,
            placement_alignment: OverlayPlacementAlignment::Start,
            outside_press_policy: OutsidePressPolicy::DismissAndPassThrough,
            initial_focus_intent: InitialFocusIntent::FirstFocusable,
            focus_restore_intent: FocusRestoreIntent::Trigger,
            tokens: ThemeTokens::default(),
            on_escape_close: None,
            on_open_change: None,
        }
    }

    /// Creates a popover with simple element content.
    pub fn element(
        id: impl Into<ElementId>,
        trigger_label: impl Into<SharedString>,
        content: impl IntoElement,
    ) -> Self {
        Self {
            id: id.into(),
            trigger_label: trigger_label.into(),
            content: PopoverContent::Element(content.into_any_element()),
            size: Size::Medium,
            disabled: false,
            open: None,
            default_open: false,
            placement_side: OverlayPlacementSide::Bottom,
            placement_alignment: OverlayPlacementAlignment::Start,
            outside_press_policy: OutsidePressPolicy::DismissAndPassThrough,
            initial_focus_intent: InitialFocusIntent::FirstFocusable,
            focus_restore_intent: FocusRestoreIntent::Trigger,
            tokens: ThemeTokens::default(),
            on_escape_close: None,
            on_open_change: None,
        }
    }

    /// Marks the popover trigger as disabled.
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
        let handler = Rc::new(handler);
        self.on_escape_close = Some(handler.clone());
        self.on_open_change = Some(handler);
        self
    }

    /// Returns resolved popover state.
    pub fn state(&self) -> PopoverState {
        PopoverState::resolve(
            self.size,
            self.disabled,
            self.open,
            self.default_open,
            self.placement_side,
            self.placement_alignment,
            self.outside_press_policy,
            self.initial_focus_intent.clone(),
            self.focus_restore_intent.clone(),
            self.tokens,
        )
    }
}

impl Sizable for Popover {
    fn with_size(mut self, size: Size) -> Self {
        self.size = size;
        self
    }
}

impl RenderOnce for Popover {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let runtime = window.use_keyed_state(self.id.clone(), cx, |_, _| PopoverRuntime {
            open: self.default_open,
        });
        let runtime_open = runtime.read(cx).open;
        let controlled_open = self.open;
        let resolved_open = controlled_open.unwrap_or(runtime_open);

        if controlled_open.is_some() && runtime_open != resolved_open {
            runtime.update(cx, |runtime, _| {
                runtime.open = resolved_open;
            });
        }

        let state = PopoverState::resolve(
            self.size,
            self.disabled,
            Some(resolved_open),
            self.default_open,
            self.placement_side,
            self.placement_alignment,
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
        let on_escape_close = self.on_escape_close;
        let on_open_change = self.on_open_change;
        let metrics = state.metrics();
        let colors = state.colors();
        let focus_ring = state.focus_ring();
        let disabled = state.disabled();
        let open = state.open();
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
            .with_offset(ui_px(6.0)),
            overlay_adapter.snap_margin(),
        );

        div()
            .id(id.clone())
            .debug_selector({
                let debug_id = debug_id.clone();
                move || format!("popover:{debug_id}:root")
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
                        move || format!("popover:{debug_id}:trigger")
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
                    .tab_stop(!disabled)
                    .ui_role(state.trigger_role())
                    .aria_label(trigger_label.clone())
                    .aria_selected(state.trigger_selected())
                    .aria_expanded(open)
                    .aria_disabled(disabled)
                    .focus_visible(move |style| style.shadow(focus_ring_shadow(focus_ring)))
                    .when(open, |this| {
                        let runtime = runtime.clone();
                        let on_escape_close = on_escape_close.clone();
                        this.on_key_down(move |event: &KeyDownEvent, window, cx| {
                            if event.keystroke.key.as_str() == "escape" {
                                cx.stop_propagation();
                                window.prevent_default();
                                close_popover(runtime.clone(), on_escape_close.clone(), window, cx);
                            }
                        })
                    })
                    .when(disabled, |this| this.opacity(0.56).cursor_not_allowed())
                    .when(!disabled, |this| {
                        let runtime = runtime.clone();
                        let on_open_change = on_open_change.clone();
                        this.cursor_pointer()
                            .hover(move |style| {
                                style.bg(ThemeResolver::resolve(colors.trigger_hover_background()))
                            })
                            .on_click(move |_event: &ClickEvent, window, cx| {
                                cx.stop_propagation();
                                let next_open = !open;
                                runtime.update(cx, |runtime, _| {
                                    runtime.open = next_open;
                                });
                                if let Some(on_open_change) = on_open_change.as_ref() {
                                    on_open_change(next_open, window, cx);
                                }
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
                            .child(popover_content_element(
                                content,
                                content_id.clone(),
                                debug_id.clone(),
                                state.clone(),
                                runtime.clone(),
                                on_escape_close.clone(),
                                on_open_change.clone(),
                            )),
                    )
                    .priority(overlay_adapter.deferred_priority()),
                )
            })
    }
}

fn popover_content_element(
    content: PopoverContent,
    content_id: ElementId,
    debug_id: String,
    state: PopoverState,
    runtime: open_gpui::Entity<PopoverRuntime>,
    on_escape_close: Option<Rc<dyn Fn(bool, &mut Window, &mut App)>>,
    on_open_change: Option<Rc<dyn Fn(bool, &mut Window, &mut App)>>,
) -> impl IntoElement {
    let metrics = state.metrics();
    let colors = state.colors();
    let outside_change = outside_press_open_change(state.overlay().policy());
    let escape_runtime = runtime.clone();
    let escape_open_change = on_escape_close.clone();

    div()
        .id(content_id)
        .debug_selector(move || format!("popover:{debug_id}:content"))
        .min_w(gpui_px_from_ui(metrics.min_width()))
        .max_w(gpui_px_from_ui(metrics.max_width()))
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
        .ui_role(state.content_role())
        .on_key_down(move |event: &KeyDownEvent, window, cx| {
            if event.keystroke.key.as_str() == "escape" {
                cx.stop_propagation();
                window.prevent_default();
                close_popover(
                    escape_runtime.clone(),
                    escape_open_change.clone(),
                    window,
                    cx,
                );
            }
        })
        .when(outside_change.is_some(), |this| {
            let runtime = runtime.clone();
            let on_open_change = on_open_change.clone();
            this.on_mouse_down_out(move |_, window, cx| {
                close_popover(runtime.clone(), on_open_change.clone(), window, cx);
            })
        })
        .children(children_from_content(content))
}

fn close_popover(
    runtime: open_gpui::Entity<PopoverRuntime>,
    on_open_change: Option<Rc<dyn Fn(bool, &mut Window, &mut App)>>,
    window: &mut Window,
    cx: &mut App,
) {
    runtime.update(cx, |runtime, _| {
        runtime.open = false;
    });
    if let Some(on_open_change) = on_open_change.as_ref() {
        on_open_change(false, window, cx);
    }
}

fn children_from_content(content: PopoverContent) -> Vec<AnyElement> {
    match content {
        PopoverContent::Text(text) => vec![div().child(text).into_any_element()],
        PopoverContent::Element(element) => vec![element],
    }
}
