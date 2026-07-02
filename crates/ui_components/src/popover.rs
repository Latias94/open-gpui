//! Popover component.

use crate::geometry::gpui_px_from_ui;
use std::rc::Rc;

use open_gpui::prelude::*;
use open_gpui::{
    AnyElement, App, ClickEvent, ElementId, Entity, FocusHandle, InteractiveElement, IntoElement,
    KeyDownEvent, ParentElement, RenderOnce, SharedString, StatefulInteractiveElement, Styled,
    Window, anchored, deferred, div,
};
use open_gpui_ui_core::{
    FocusRestoreIntent, InitialFocusIntent, OutsidePressPolicy, OverlayLayerKind,
    OverlayPlacementAlignment, OverlayPlacementInput, OverlayPlacementSide, Role, Sizable, Size,
    ThemeTokens, UiPx, ui_point, ui_px, ui_size,
};

use crate::a11y::UiA11yElementExt;
use crate::color::ColorIntent;
use crate::focus::{FocusRing, focus_ring_shadow_with_theme};
use crate::overlay::{
    GpuiOverlayPlacement, OverlayDisclosureConfig, OverlayDisclosureOpenMode, OverlayResolvedState,
    consume_overlay_event, emit_overlay_open_change, escape_open_change, gpui_overlay_state,
    outside_press_open_change, resolve_overlay_open_state, restore_overlay_focus, set_overlay_open,
};
use crate::theme::{ThemeContext, ThemeResolver};

/// Popover open-state ownership.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PopoverOpenMode {
    /// Open state is owned by the component adapter after initialization.
    #[default]
    Uncontrolled,
    /// Open state is provided by the caller.
    Controlled,
}

impl PopoverOpenMode {
    /// Returns a stable label.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Uncontrolled => "uncontrolled",
            Self::Controlled => "controlled",
        }
    }
}

const fn popover_open_mode_from_disclosure(mode: OverlayDisclosureOpenMode) -> PopoverOpenMode {
    match mode {
        OverlayDisclosureOpenMode::Uncontrolled => PopoverOpenMode::Uncontrolled,
        OverlayDisclosureOpenMode::Controlled => PopoverOpenMode::Controlled,
    }
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
        let disclosure = OverlayDisclosureConfig::new(OverlayLayerKind::NonModalDismissible)
            .controlled_open(open)
            .default_open(default_open)
            .disabled(disabled)
            .outside_press_policy(outside_press_policy)
            .initial_focus_intent(initial_focus_intent.clone())
            .focus_restore_intent(focus_restore_intent.clone())
            .resolve();
        let open = disclosure.open();
        let open_mode = popover_open_mode_from_disclosure(disclosure.open_mode());
        let overlay = disclosure.overlay().clone();
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

#[derive(Debug, Clone)]
struct PopoverRuntime {
    open: bool,
    trigger_focus: FocusHandle,
    content_focus: FocusHandle,
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
            initial_focus_intent: InitialFocusIntent::None,
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
            initial_focus_intent: InitialFocusIntent::None,
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
        let runtime = window.use_keyed_state(self.id.clone(), cx, |_, cx| PopoverRuntime {
            open: self.default_open,
            trigger_focus: cx.focus_handle(),
            content_focus: cx.focus_handle(),
        });
        let open_state = resolve_overlay_open_state(self.open, runtime.read(cx).open);
        let resolved_open = open_state.open();

        if open_state.runtime_changed() {
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
        let theme = ThemeResolver::current(cx);
        let trigger_focus_shadow = focus_ring_shadow_with_theme(focus_ring, &theme);
        let trigger_border = theme.resolve(colors.trigger_border());
        let trigger_background = theme.resolve(colors.trigger_background());
        let trigger_foreground = theme.resolve(colors.trigger_foreground());
        let trigger_hover_background = theme.resolve(colors.trigger_hover_background());
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
                    .border_color(trigger_border)
                    .bg(trigger_background)
                    .text_color(trigger_foreground)
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
                    .focus_visible(move |style| style.shadow(trigger_focus_shadow.clone()))
                    .when(open, |this| {
                        let runtime = runtime.clone();
                        let on_escape_close = on_escape_close.clone();
                        let focus_restore = state.focus_restore_intent().clone();
                        let escape_policy = state.overlay().policy().clone();
                        this.on_key_down(move |event: &KeyDownEvent, window, cx| {
                            if event.keystroke.key.as_str() == "escape"
                                && escape_open_change(&escape_policy).is_some()
                            {
                                consume_overlay_event(window, cx);
                                close_popover(
                                    runtime.clone(),
                                    focus_restore.clone(),
                                    on_escape_close.clone(),
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
                        let initial_focus = state.initial_focus_intent().clone();
                        this.cursor_pointer()
                            .hover(move |style| style.bg(trigger_hover_background))
                            .on_click(move |_event: &ClickEvent, window, cx| {
                                cx.stop_propagation();
                                let next_open = !open;
                                runtime.update(cx, |runtime, _| {
                                    set_overlay_open(&mut runtime.open, next_open);
                                });
                                if next_open
                                    && let Some(focus) =
                                        popover_initial_focus_handle(&runtime, &initial_focus, cx)
                                {
                                    window.defer(cx, move |window, cx| focus.focus(window, cx));
                                }
                                emit_overlay_open_change(
                                    next_open,
                                    on_open_change.as_deref(),
                                    window,
                                    cx,
                                );
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
                                &theme,
                                runtime.clone(),
                                content_focus.clone(),
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
    theme: &ThemeContext,
    runtime: Entity<PopoverRuntime>,
    content_focus: FocusHandle,
    on_escape_close: Option<Rc<dyn Fn(bool, &mut Window, &mut App)>>,
    on_open_change: Option<Rc<dyn Fn(bool, &mut Window, &mut App)>>,
) -> impl IntoElement {
    let metrics = state.metrics();
    let colors = state.colors();
    let outside_change = outside_press_open_change(state.overlay().policy());
    let escape_runtime = runtime.clone();
    let escape_open_change = on_escape_close.clone();
    let escape_focus_restore = state.focus_restore_intent().clone();
    let border = theme.resolve(colors.border());
    let background = theme.resolve(colors.background());
    let foreground = theme.resolve(colors.foreground());

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
        .border_color(border)
        .bg(background)
        .text_color(foreground)
        .text_size(gpui_px_from_ui(metrics.text_size()))
        .line_height(gpui_px_from_ui(metrics.text_size()))
        .shadow_lg()
        .occlude()
        .tab_group()
        .focusable()
        .track_focus(&content_focus)
        .ui_role(state.content_role())
        .on_key_down(move |event: &KeyDownEvent, window, cx| {
            if event.keystroke.key.as_str() == "escape" {
                consume_overlay_event(window, cx);
                close_popover(
                    escape_runtime.clone(),
                    escape_focus_restore.clone(),
                    escape_open_change.clone(),
                    window,
                    cx,
                );
            }
        })
        .when(outside_change.is_some(), |this| {
            let runtime = runtime.clone();
            let on_open_change = on_open_change.clone();
            let focus_restore = state.focus_restore_intent().clone();
            this.on_mouse_down_out(move |_, window, cx| {
                close_popover(
                    runtime.clone(),
                    focus_restore.clone(),
                    on_open_change.clone(),
                    window,
                    cx,
                );
            })
        })
        .children(children_from_content(content))
}

fn close_popover(
    runtime: Entity<PopoverRuntime>,
    focus_restore: FocusRestoreIntent,
    on_open_change: Option<Rc<dyn Fn(bool, &mut Window, &mut App)>>,
    window: &mut Window,
    cx: &mut App,
) {
    let trigger_focus = runtime.read(cx).trigger_focus.clone();
    runtime.update(cx, |runtime, _| {
        set_overlay_open(&mut runtime.open, false);
    });
    emit_overlay_open_change(false, on_open_change.as_deref(), window, cx);
    restore_overlay_focus(&focus_restore, Some(trigger_focus), true, window, cx);
}

fn popover_initial_focus_handle(
    runtime: &Entity<PopoverRuntime>,
    intent: &InitialFocusIntent,
    cx: &App,
) -> Option<FocusHandle> {
    match intent {
        InitialFocusIntent::None => None,
        InitialFocusIntent::FirstFocusable => Some(runtime.read(cx).content_focus.clone()),
        InitialFocusIntent::Target(_) => None,
        InitialFocusIntent::TargetOrFirstFocusable(_) => {
            Some(runtime.read(cx).content_focus.clone())
        }
    }
}

fn children_from_content(content: PopoverContent) -> Vec<AnyElement> {
    match content {
        PopoverContent::Text(text) => vec![div().child(text).into_any_element()],
        PopoverContent::Element(element) => vec![element],
    }
}
