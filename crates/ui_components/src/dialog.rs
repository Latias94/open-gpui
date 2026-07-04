//! Dialog component.

use crate::geometry::gpui_px_from_ui;
use std::rc::Rc;

use open_gpui::prelude::*;
use open_gpui::{
    AnyElement, App, ClickEvent, ElementId, Entity, FocusHandle, InteractiveElement, IntoElement,
    KeyDownEvent, ParentElement, Pixels, RenderOnce, SharedString, StatefulInteractiveElement,
    Styled, Window, div, px,
};
use open_gpui_ui_core::{
    EscapeKeyPolicy, FocusRestoreIntent, InitialFocusIntent, OutsidePressPolicy, OverlayLayerKind,
    Role, Sizable, Size, ThemeTokens, UiPx, ui_px,
};

use crate::a11y::UiA11yElementExt;
use crate::color::ColorIntent;
use crate::focus::{FocusRing, focus_ring_shadow_with_theme};
use crate::overlay::{
    OverlayCloseRuntimeRequest, OverlayDisclosureConfig, OverlayDisclosureOpenMode,
    OverlayOpenRuntimeRequest, OverlayResolvedState, apply_overlay_open_change_with_after_update,
    close_overlay_runtime, consume_overlay_event, escape_open_change,
    gpui_full_window_overlay_layer, gpui_overlay_state, outside_press_open_change,
    resolve_overlay_open_state, set_overlay_open,
};
use crate::theme::{ThemeContext, ThemeResolver};

/// Dialog open-state ownership.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DialogOpenMode {
    /// Open state is owned by the component adapter after initialization.
    #[default]
    Uncontrolled,
    /// Open state is provided by the caller.
    Controlled,
}

impl DialogOpenMode {
    /// Returns a stable label.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Uncontrolled => "uncontrolled",
            Self::Controlled => "controlled",
        }
    }
}

const fn dialog_open_mode_from_disclosure(mode: OverlayDisclosureOpenMode) -> DialogOpenMode {
    match mode {
        OverlayDisclosureOpenMode::Uncontrolled => DialogOpenMode::Uncontrolled,
        OverlayDisclosureOpenMode::Controlled => DialogOpenMode::Controlled,
    }
}

/// Resolved dialog color intents.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DialogColors {
    pub(crate) barrier: ColorIntent,
    pub(crate) surface: ColorIntent,
    pub(crate) foreground: ColorIntent,
    pub(crate) border: ColorIntent,
    pub(crate) trigger_background: ColorIntent,
    pub(crate) trigger_hover_background: ColorIntent,
    pub(crate) trigger_foreground: ColorIntent,
    pub(crate) trigger_border: ColorIntent,
    pub(crate) focus_ring: ColorIntent,
}

impl DialogColors {
    /// Returns barrier color intent.
    pub const fn barrier(self) -> ColorIntent {
        self.barrier
    }

    /// Returns surface color intent.
    pub const fn surface(self) -> ColorIntent {
        self.surface
    }

    /// Returns foreground color intent.
    pub const fn foreground(self) -> ColorIntent {
        self.foreground
    }

    /// Returns border color intent.
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

/// Resolved dialog metrics.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DialogMetrics {
    trigger_height: UiPx,
    trigger_padding_x: UiPx,
    trigger_padding_y: UiPx,
    padding: UiPx,
    radius: UiPx,
    title_size: UiPx,
    text_size: UiPx,
    width: UiPx,
    max_width: UiPx,
}

impl DialogMetrics {
    /// Resolves metrics from the shared foundation size vocabulary.
    pub const fn from_size(size: Size) -> Self {
        Self {
            trigger_height: size.button_h(),
            trigger_padding_x: size.button_px(),
            trigger_padding_y: size.button_py(),
            padding: size.button_px(),
            radius: size.control_radius(),
            title_size: ui_px(18.0),
            text_size: size.control_text_px(),
            width: ui_px(420.0),
            max_width: ui_px(560.0),
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

    /// Returns surface padding.
    pub const fn padding(self) -> UiPx {
        self.padding
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

    /// Returns preferred surface width.
    pub const fn width(self) -> UiPx {
        self.width
    }

    /// Returns maximum surface width.
    pub const fn max_width(self) -> UiPx {
        self.max_width
    }
}

/// Resolved dialog state used by tests, demos, and rendering.
#[derive(Debug, Clone, PartialEq)]
pub struct DialogState {
    size: Size,
    disabled: bool,
    open: bool,
    default_open: bool,
    open_mode: DialogOpenMode,
    title: SharedString,
    description: Option<SharedString>,
    outside_press_policy: OutsidePressPolicy,
    escape_key_policy: EscapeKeyPolicy,
    initial_focus_intent: InitialFocusIntent,
    focus_restore_intent: FocusRestoreIntent,
    trigger_selected: bool,
    metrics: DialogMetrics,
    colors: DialogColors,
    focus_ring: FocusRing,
    overlay: OverlayResolvedState,
}

impl DialogState {
    /// Resolves the public state for a dialog.
    #[allow(clippy::too_many_arguments)]
    pub fn resolve(
        size: Size,
        disabled: bool,
        open: Option<bool>,
        default_open: bool,
        title: SharedString,
        description: Option<SharedString>,
        outside_press_policy: OutsidePressPolicy,
        escape_key_policy: EscapeKeyPolicy,
        initial_focus_intent: InitialFocusIntent,
        focus_restore_intent: FocusRestoreIntent,
        tokens: ThemeTokens,
    ) -> Self {
        let disclosure = OverlayDisclosureConfig::new(OverlayLayerKind::Modal)
            .controlled_open(open)
            .default_open(default_open)
            .disabled(disabled)
            .outside_press_policy(outside_press_policy)
            .escape_key_policy(escape_key_policy)
            .initial_focus_intent(initial_focus_intent.clone())
            .focus_restore_intent(focus_restore_intent.clone())
            .resolve();
        let open = disclosure.open();
        let open_mode = dialog_open_mode_from_disclosure(disclosure.open_mode());
        let overlay = disclosure.overlay().clone();
        let colors = ThemeResolver::dialog_colors(tokens, open);

        Self {
            size,
            disabled,
            open,
            default_open,
            open_mode,
            title,
            description,
            outside_press_policy,
            escape_key_policy,
            initial_focus_intent,
            focus_restore_intent,
            trigger_selected: open,
            metrics: DialogMetrics::from_size(size),
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

    /// Returns whether dialog content is open.
    pub const fn open(&self) -> bool {
        self.open
    }

    /// Returns uncontrolled initial open state.
    pub const fn default_open(&self) -> bool {
        self.default_open
    }

    /// Returns open-state ownership.
    pub const fn open_mode(&self) -> DialogOpenMode {
        self.open_mode
    }

    /// Returns dialog title.
    pub fn title(&self) -> &str {
        &self.title
    }

    /// Returns optional dialog description.
    pub fn description(&self) -> Option<&str> {
        self.description.as_deref()
    }

    /// Returns outside-press policy.
    pub const fn outside_press_policy(&self) -> OutsidePressPolicy {
        self.outside_press_policy
    }

    /// Returns Escape-key policy.
    pub const fn escape_key_policy(&self) -> EscapeKeyPolicy {
        self.escape_key_policy
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
    pub const fn metrics(&self) -> DialogMetrics {
        self.metrics
    }

    /// Returns resolved color intents.
    pub const fn colors(&self) -> DialogColors {
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

/// A concrete GPUI dialog component.
#[derive(IntoElement)]
pub struct Dialog {
    id: ElementId,
    trigger_label: SharedString,
    title: SharedString,
    description: Option<SharedString>,
    content: DialogContent,
    size: Size,
    disabled: bool,
    open: Option<bool>,
    default_open: bool,
    outside_press_policy: OutsidePressPolicy,
    escape_key_policy: EscapeKeyPolicy,
    initial_focus_intent: InitialFocusIntent,
    focus_restore_intent: FocusRestoreIntent,
    tokens: ThemeTokens,
    on_escape_close: Option<Rc<dyn Fn(bool, &mut Window, &mut App)>>,
    on_open_change: Option<Rc<dyn Fn(bool, &mut Window, &mut App)>>,
}

enum DialogContent {
    Text(SharedString),
    Element(AnyElement),
}

#[derive(Debug, Clone)]
struct DialogRuntime {
    open: bool,
    trigger_focus: FocusHandle,
    surface_focus: FocusHandle,
}

impl Dialog {
    /// Creates a text dialog.
    pub fn new(
        id: impl Into<ElementId>,
        trigger_label: impl Into<SharedString>,
        title: impl Into<SharedString>,
        content: impl Into<SharedString>,
    ) -> Self {
        Self {
            id: id.into(),
            trigger_label: trigger_label.into(),
            title: title.into(),
            description: None,
            content: DialogContent::Text(content.into()),
            size: Size::Medium,
            disabled: false,
            open: None,
            default_open: false,
            outside_press_policy: OutsidePressPolicy::Consume,
            escape_key_policy: EscapeKeyPolicy::Dismiss,
            initial_focus_intent: InitialFocusIntent::FirstFocusable,
            focus_restore_intent: FocusRestoreIntent::Trigger,
            tokens: ThemeTokens::default(),
            on_escape_close: None,
            on_open_change: None,
        }
    }

    /// Creates a dialog with simple element content.
    pub fn element(
        id: impl Into<ElementId>,
        trigger_label: impl Into<SharedString>,
        title: impl Into<SharedString>,
        content: impl IntoElement,
    ) -> Self {
        Self {
            id: id.into(),
            trigger_label: trigger_label.into(),
            title: title.into(),
            description: None,
            content: DialogContent::Element(content.into_any_element()),
            size: Size::Medium,
            disabled: false,
            open: None,
            default_open: false,
            outside_press_policy: OutsidePressPolicy::Consume,
            escape_key_policy: EscapeKeyPolicy::Dismiss,
            initial_focus_intent: InitialFocusIntent::FirstFocusable,
            focus_restore_intent: FocusRestoreIntent::Trigger,
            tokens: ThemeTokens::default(),
            on_escape_close: None,
            on_open_change: None,
        }
    }

    /// Applies optional description metadata.
    pub fn description(mut self, description: impl Into<SharedString>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Marks the dialog trigger as disabled.
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

    /// Applies outside-press policy.
    pub fn outside_press_policy(mut self, policy: OutsidePressPolicy) -> Self {
        self.outside_press_policy = policy;
        self
    }

    /// Applies Escape-key policy.
    pub fn escape_key_policy(mut self, policy: EscapeKeyPolicy) -> Self {
        self.escape_key_policy = policy;
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

    /// Returns resolved dialog state.
    pub fn state(&self) -> DialogState {
        DialogState::resolve(
            self.size,
            self.disabled,
            self.open,
            self.default_open,
            self.title.clone(),
            self.description.clone(),
            self.outside_press_policy,
            self.escape_key_policy,
            self.initial_focus_intent.clone(),
            self.focus_restore_intent.clone(),
            self.tokens,
        )
    }
}

impl Sizable for Dialog {
    fn with_size(mut self, size: Size) -> Self {
        self.size = size;
        self
    }
}

impl RenderOnce for Dialog {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let runtime = window.use_keyed_state(self.id.clone(), cx, |_, cx| DialogRuntime {
            open: self.default_open,
            trigger_focus: cx.focus_handle(),
            surface_focus: cx.focus_handle(),
        });
        let open_state = resolve_overlay_open_state(self.open, runtime.read(cx).open);
        let resolved_open = open_state.open();

        if open_state.runtime_changed() {
            runtime.update(cx, |runtime, _| {
                runtime.open = resolved_open;
            });
        }

        let state = DialogState::resolve(
            self.size,
            self.disabled,
            Some(resolved_open),
            self.default_open,
            self.title.clone(),
            self.description.clone(),
            self.outside_press_policy,
            self.escape_key_policy,
            self.initial_focus_intent.clone(),
            self.focus_restore_intent.clone(),
            self.tokens,
        );
        let viewport = window.viewport_size();
        let id = self.id;
        let debug_id = id.to_string();
        let trigger_id: ElementId = (id.clone(), "trigger").into();
        let content_id: ElementId = (id.clone(), "content").into();
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
        let surface_focus = runtime.read(cx).surface_focus.clone();
        let overlay_adapter = gpui_overlay_state(state.overlay());

        div()
            .id(id.clone())
            .debug_selector({
                let debug_id = debug_id.clone();
                move || format!("dialog:{debug_id}:root")
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
                        move || format!("dialog:{debug_id}:trigger")
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
                                close_dialog(
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
                                let focus_runtime = runtime.clone();
                                let initial_focus = initial_focus.clone();
                                apply_overlay_open_change_with_after_update(
                                    OverlayOpenRuntimeRequest::new(
                                        runtime.clone(),
                                        next_open,
                                        on_open_change.as_deref(),
                                    ),
                                    window,
                                    cx,
                                    |runtime| {
                                        set_overlay_open(&mut runtime.open, next_open);
                                    },
                                    move |window, cx| {
                                        if next_open
                                            && let Some(focus) = dialog_initial_focus_handle(
                                                &focus_runtime,
                                                &initial_focus,
                                                cx,
                                            )
                                        {
                                            window.defer(cx, move |window, cx| {
                                                focus.focus(window, cx)
                                            });
                                        }
                                    },
                                );
                            })
                    })
                    .child(trigger_label),
            )
            .when(open, |this| {
                this.child(gpui_full_window_overlay_layer(
                    &overlay_adapter,
                    dialog_layer_element(
                        content,
                        content_id.clone(),
                        debug_id.clone(),
                        state.clone(),
                        &theme,
                        viewport,
                        runtime.clone(),
                        surface_focus.clone(),
                        on_escape_close.clone(),
                        on_open_change.clone(),
                    ),
                ))
            })
    }
}

fn dialog_layer_element(
    content: DialogContent,
    content_id: ElementId,
    debug_id: String,
    state: DialogState,
    theme: &ThemeContext,
    viewport: open_gpui::Size<Pixels>,
    runtime: Entity<DialogRuntime>,
    surface_focus: FocusHandle,
    on_escape_close: Option<Rc<dyn Fn(bool, &mut Window, &mut App)>>,
    on_open_change: Option<Rc<dyn Fn(bool, &mut Window, &mut App)>>,
) -> impl IntoElement {
    let metrics = state.metrics();
    let colors = state.colors();
    let outside_change = outside_press_open_change(state.overlay().policy());
    let escape_change = escape_open_change(state.overlay().policy());
    let escape_runtime = runtime.clone();
    let escape_open_change = on_escape_close.clone();
    let escape_focus_restore = state.focus_restore_intent().clone();
    let barrier = theme.resolve(colors.barrier());
    let border = theme.resolve(colors.border());
    let surface = theme.resolve(colors.surface());
    let foreground = theme.resolve(colors.foreground());
    let muted_foreground = theme.resolve(ColorIntent::new(
        ThemeTokens::default().text_muted,
        0x5a6472,
    ));
    let x = ((viewport.width - gpui_px_from_ui(metrics.width())) / 2.0).max(px(12.0));
    let y = (viewport.height / 10.0).max(px(24.0));

    div()
        .id(content_id)
        .debug_selector({
            let debug_id = debug_id.clone();
            move || format!("dialog:{debug_id}:layer")
        })
        .absolute()
        .left(px(0.0))
        .top(px(0.0))
        .w(viewport.width)
        .h(viewport.height)
        .bg(barrier)
        .occlude()
        .on_any_mouse_down(|_, window, cx| {
            consume_overlay_event(window, cx);
        })
        .when(outside_change.is_some(), |this| {
            let runtime = runtime.clone();
            let on_open_change = on_open_change.clone();
            let focus_restore = state.focus_restore_intent().clone();
            this.on_click(move |_: &ClickEvent, window, cx| {
                consume_overlay_event(window, cx);
                close_dialog(
                    runtime.clone(),
                    focus_restore.clone(),
                    on_open_change.clone(),
                    window,
                    cx,
                );
            })
        })
        .child(
            div()
                .id("dialog-surface")
                .debug_selector(move || format!("dialog:{debug_id}:surface"))
                .absolute()
                .left(x)
                .top(y)
                .w(gpui_px_from_ui(metrics.width()))
                .max_w(gpui_px_from_ui(metrics.max_width()))
                .p(gpui_px_from_ui(metrics.padding()))
                .flex()
                .flex_col()
                .gap_3()
                .rounded(gpui_px_from_ui(metrics.radius()))
                .border_1()
                .border_color(border)
                .bg(surface)
                .text_color(foreground)
                .text_size(gpui_px_from_ui(metrics.text_size()))
                .line_height(gpui_px_from_ui(metrics.text_size()))
                .shadow_lg()
                .occlude()
                .on_any_mouse_down(|_, _, cx| {
                    cx.stop_propagation();
                })
                .tab_group()
                .focusable()
                .track_focus(&surface_focus)
                .ui_role(state.content_role())
                .aria_label(state.title().to_owned())
                .on_key_down(move |event: &KeyDownEvent, window, cx| {
                    if event.keystroke.key.as_str() == "escape" && escape_change.is_some() {
                        consume_overlay_event(window, cx);
                        close_dialog(
                            escape_runtime.clone(),
                            escape_focus_restore.clone(),
                            escape_open_change.clone(),
                            window,
                            cx,
                        );
                    }
                })
                .child(
                    div()
                        .text_size(gpui_px_from_ui(metrics.title_size()))
                        .font_weight(open_gpui::FontWeight::BOLD)
                        .line_height(px(24.0))
                        .child(state.title().to_owned()),
                )
                .when_some(
                    state.description().map(ToOwned::to_owned),
                    |this, description| {
                        this.child(div().text_color(muted_foreground).child(description))
                    },
                )
                .children(children_from_content(content)),
        )
}

fn close_dialog(
    runtime: Entity<DialogRuntime>,
    focus_restore: FocusRestoreIntent,
    on_open_change: Option<Rc<dyn Fn(bool, &mut Window, &mut App)>>,
    window: &mut Window,
    cx: &mut App,
) {
    let trigger_focus = runtime.read(cx).trigger_focus.clone();
    close_overlay_runtime(
        OverlayCloseRuntimeRequest::new(
            runtime,
            &focus_restore,
            trigger_focus,
            on_open_change.as_deref(),
        ),
        window,
        cx,
        |runtime| {
            set_overlay_open(&mut runtime.open, false);
        },
    );
}

fn dialog_initial_focus_handle(
    runtime: &Entity<DialogRuntime>,
    intent: &InitialFocusIntent,
    cx: &App,
) -> Option<FocusHandle> {
    match intent {
        InitialFocusIntent::None => None,
        InitialFocusIntent::FirstFocusable => Some(runtime.read(cx).surface_focus.clone()),
        InitialFocusIntent::Target(_) => None,
        InitialFocusIntent::TargetOrFirstFocusable(_) => {
            Some(runtime.read(cx).surface_focus.clone())
        }
    }
}

fn children_from_content(content: DialogContent) -> Vec<AnyElement> {
    match content {
        DialogContent::Text(text) => vec![div().child(text).into_any_element()],
        DialogContent::Element(element) => vec![element],
    }
}
