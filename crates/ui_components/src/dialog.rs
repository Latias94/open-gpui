//! Dialog component.

use crate::geometry::gpui_px_from_ui;
use std::rc::Rc;

use open_gpui::prelude::*;
use open_gpui::{
    AnyElement, App, ClickEvent, ElementId, InteractiveElement, IntoElement, ParentElement, Pixels,
    RenderOnce, SharedString, StatefulInteractiveElement, Styled, Window, div, px,
};
use open_gpui_ui_core::{
    AccessibleAction, DismissReason, EscapeKeyPolicy, FocusRestoreIntent, InitialFocusIntent,
    OutsidePressPolicy, OverlayLayerKind, Role, SemanticDescriptor, Sizable, Size, ThemeTokens,
    UiPx, ui_px,
};

use crate::a11y::UiA11yElementExt;
use crate::color::ColorIntent;
use crate::focus::{FocusRing, focus_ring_shadow_with_theme};
use crate::overlay::{
    FocusTargetRegistration, OverlayDisclosureConfig, OverlayDisclosureOpenMode,
    OverlayFocusTargetSet, OverlayInsideRegionId, OverlayLayerBinding, OverlayLayerRegistration,
    OverlayOpenIntent, OverlayOwnership, OverlayResolvedState, WindowOverlayRuntime,
    gpui_full_window_overlay_layer, gpui_overlay_state, resolve_overlay_open_state,
};
use crate::theme::{ThemeContext, ThemeResolver, gpui_elevation_shadow};

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
    focus_targets: Vec<FocusTargetRegistration>,
    tokens: ThemeTokens,
    on_open_change: Option<Rc<dyn Fn(OverlayOpenIntent, &mut Window, &mut App)>>,
}

enum DialogContent {
    Text(SharedString),
    Element(AnyElement),
}

#[derive(Clone)]
struct DialogRuntime {
    open: bool,
    overlay_binding: Option<OverlayLayerBinding>,
    focus_targets: OverlayFocusTargetSet,
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
            outside_press_policy: OutsidePressPolicy::DismissAndConsume,
            escape_key_policy: EscapeKeyPolicy::Dismiss,
            initial_focus_intent: InitialFocusIntent::FirstFocusable,
            focus_restore_intent: FocusRestoreIntent::Trigger,
            focus_targets: Vec::new(),
            tokens: ThemeTokens::default(),
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
            outside_press_policy: OutsidePressPolicy::DismissAndConsume,
            escape_key_policy: EscapeKeyPolicy::Dismiss,
            initial_focus_intent: InitialFocusIntent::FirstFocusable,
            focus_restore_intent: FocusRestoreIntent::Trigger,
            focus_targets: Vec::new(),
            tokens: ThemeTokens::default(),
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

    /// Declares a live focus target owned by this dialog layer.
    pub fn focus_target(mut self, target: FocusTargetRegistration) -> Self {
        self.focus_targets.push(target);
        self
    }

    /// Applies a token bundle.
    pub fn tokens(mut self, tokens: ThemeTokens) -> Self {
        self.tokens = tokens;
        self
    }

    /// Registers an open-change handler with the runtime-issued intent.
    pub fn on_open_change(
        mut self,
        handler: impl Fn(OverlayOpenIntent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_open_change = Some(Rc::new(handler));
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
        let runtime = window.use_keyed_state(self.id.clone(), cx, |_, _| DialogRuntime {
            open: self.default_open,
            overlay_binding: None,
            focus_targets: OverlayFocusTargetSet::default(),
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
        let focus_targets = self.focus_targets;
        let on_open_change = self.on_open_change;
        let window_overlay_runtime = WindowOverlayRuntime::for_window(window, cx);
        let ownership = if open_state.controlled() {
            OverlayOwnership::Controlled
        } else {
            OverlayOwnership::Uncontrolled
        };
        let mut registration = OverlayLayerRegistration::new(
            format!("dialog:{debug_id}"),
            state.overlay().policy().clone(),
            ownership,
        );
        if let Some(on_open_change) = on_open_change {
            registration = registration.on_open_change(move |intent, window, cx| {
                on_open_change(intent, window, cx);
            });
        }
        if ownership == OverlayOwnership::Uncontrolled {
            let runtime = runtime.downgrade();
            registration = registration.uncontrolled_commit(move |open, _, cx| {
                let _ = runtime.update(cx, |runtime, _| {
                    runtime.open = open;
                });
            });
        }
        let existing_binding = runtime.read(cx).overlay_binding.clone();
        let overlay_binding = window_overlay_runtime
            .bind_component_layer(
                &runtime,
                existing_binding.as_ref(),
                registration,
                window,
                cx,
            )
            .expect("dialog overlay registration should remain valid");
        if existing_binding.is_none() {
            runtime.update(cx, |runtime, _| {
                runtime.overlay_binding = Some(overlay_binding.clone());
            });
        }
        let mut registered_focus_targets = runtime.read(cx).focus_targets.clone();
        registered_focus_targets
            .sync(
                &window_overlay_runtime,
                &overlay_binding,
                focus_targets,
                window,
                cx,
            )
            .expect("dialog focus targets should remain valid");
        runtime.update(cx, |runtime, _| {
            runtime.focus_targets = registered_focus_targets;
        });
        let metrics = state.metrics();
        let colors = state.colors();
        let focus_ring = state.focus_ring();
        let theme = ThemeResolver::current(window, cx);
        let trigger_focus_shadow = focus_ring_shadow_with_theme(focus_ring, &theme);
        let trigger_border = theme.resolve(colors.trigger_border());
        let trigger_background = theme.resolve(colors.trigger_background());
        let trigger_foreground = theme.resolve(colors.trigger_foreground());
        let trigger_hover_background = theme.resolve(colors.trigger_hover_background());
        let disabled = state.disabled();
        let open = state.open();
        let overlay_adapter = gpui_overlay_state(state.overlay());
        let trigger_semantics = SemanticDescriptor::new(state.trigger_role())
            .with_label(trigger_label.as_ref())
            .with_selected(state.trigger_selected())
            .with_expanded(open)
            .with_disabled(disabled)
            .with_actions(&[AccessibleAction::Click, AccessibleAction::Focus]);

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
                window_overlay_runtime.focus_target(
                    &overlay_binding,
                    format!("dialog:{debug_id}:trigger-focus-target"),
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
                        .tab_stop(!disabled)
                        .ui_semantics(&trigger_semantics)
                        .focus_visible(move |style| style.shadow(trigger_focus_shadow.clone()))
                        .track_focus(overlay_binding.trigger_focus())
                        .when(disabled, |this| this.opacity(0.56).cursor_not_allowed())
                        .when(!disabled, |this| {
                            let window_overlay_runtime = window_overlay_runtime.clone();
                            let overlay_binding = overlay_binding.clone();
                            this.cursor_pointer()
                                .hover(move |style| style.bg(trigger_hover_background))
                                .on_click(move |_event: &ClickEvent, window, cx| {
                                    cx.stop_propagation();
                                    window_overlay_runtime
                                        .request_open_change(
                                            &overlay_binding,
                                            !open,
                                            DismissReason::Trigger,
                                            window,
                                            cx,
                                        )
                                        .expect(
                                            "dialog trigger should own its overlay registration",
                                        );
                                })
                        })
                        .child(trigger_label),
                ),
            )
            .when(open, |this| {
                this.child(gpui_full_window_overlay_layer(
                    &overlay_adapter,
                    &overlay_binding,
                    |opening_theme| {
                        dialog_layer_element(
                            content,
                            content_id.clone(),
                            debug_id.clone(),
                            state.clone(),
                            opening_theme,
                            viewport,
                            window_overlay_runtime.clone(),
                            overlay_binding.clone(),
                        )
                        .into_any_element()
                    },
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
    window_overlay_runtime: WindowOverlayRuntime,
    overlay_binding: OverlayLayerBinding,
) -> impl IntoElement {
    let metrics = state.metrics();
    let colors = state.colors();
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
    let content_semantics = SemanticDescriptor::new(state.content_role())
        .with_label(state.title())
        .with_modal(true)
        .with_actions(&[AccessibleAction::Focus]);

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
        .child(
            window_overlay_runtime.surface(
                &overlay_binding,
                OverlayInsideRegionId::new("surface"),
                format!("dialog:{debug_id}:surface-region"),
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
                    .shadow(gpui_elevation_shadow(
                        ThemeResolver::overlay_surface_elevation(theme),
                    ))
                    .occlude()
                    .tab_group()
                    .focusable()
                    .track_focus(overlay_binding.surface_focus())
                    .ui_semantics(&content_semantics)
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
            ),
        )
}

fn children_from_content(content: DialogContent) -> Vec<AnyElement> {
    match content {
        DialogContent::Text(text) => vec![div().child(text).into_any_element()],
        DialogContent::Element(element) => vec![element],
    }
}
