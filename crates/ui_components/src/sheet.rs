//! Sheet component.

use crate::geometry::gpui_px_from_ui;
use std::rc::Rc;

use open_gpui::prelude::*;
use open_gpui::{
    AnyElement, App, ClickEvent, ElementId, FocusHandle, InteractiveElement, IntoElement,
    ParentElement, Pixels, RenderOnce, SharedString, StatefulInteractiveElement, Styled, Window,
    div, px,
};
#[cfg(test)]
use open_gpui_ui_core::FocusTargetId;
use open_gpui_ui_core::{
    DismissReason, EscapeKeyPolicy, FocusRestoreIntent, FocusTargetAvailability,
    InitialFocusIntent, OutsidePressPolicy, OverlayLayerKind, Role, Sizable, Size, ThemeTokens,
    UiPx, UiSize, ui_px,
};

use crate::a11y::UiA11yElementExt;
use crate::color::ColorIntent;
use crate::focus::{FocusRing, focus_ring_shadow_with_theme};
use crate::geometry::ui_size_from_gpui_size;
use crate::overlay::{
    FocusTargetRegistration, OverlayDisclosureConfig, OverlayDisclosureOpenMode,
    OverlayFocusTargetSet, OverlayInsideRegionId, OverlayLayerBinding, OverlayLayerRegistration,
    OverlayOpenIntent, OverlayOwnership, OverlayResolvedState, WindowOverlayRuntime,
    gpui_full_window_overlay_layer, gpui_overlay_state, resolve_overlay_open_state,
};
use crate::theme::{ThemeContext, ThemeResolver};

type SheetOpenChangeHandler = Rc<dyn Fn(OverlayOpenIntent, &mut Window, &mut App)>;

const CLOSE_FOCUS_TARGET: &str = "sheet.close";

/// Sheet open-state ownership.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SheetOpenMode {
    /// Open state is owned by the component adapter after initialization.
    #[default]
    Uncontrolled,
    /// Open state is provided by the caller.
    Controlled,
}

impl SheetOpenMode {
    /// Returns a stable label.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Uncontrolled => "uncontrolled",
            Self::Controlled => "controlled",
        }
    }
}

const fn sheet_open_mode_from_disclosure(mode: OverlayDisclosureOpenMode) -> SheetOpenMode {
    match mode {
        OverlayDisclosureOpenMode::Uncontrolled => SheetOpenMode::Uncontrolled,
        OverlayDisclosureOpenMode::Controlled => SheetOpenMode::Controlled,
    }
}

/// Edge where the sheet surface attaches.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SheetSide {
    /// Sheet enters from the top edge.
    Top,
    /// Sheet enters from the right edge.
    #[default]
    Right,
    /// Sheet enters from the bottom edge.
    Bottom,
    /// Sheet enters from the left edge.
    Left,
}

impl SheetSide {
    /// Returns whether this side uses width as its primary size.
    pub const fn is_horizontal(self) -> bool {
        matches!(self, Self::Left | Self::Right)
    }

    /// Returns a stable label.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Top => "top",
            Self::Right => "right",
            Self::Bottom => "bottom",
            Self::Left => "left",
        }
    }
}

/// Interaction mode for the sheet layer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SheetModalMode {
    /// Modal sheet blocks underlay input while present.
    #[default]
    Modal,
    /// Non-modal sheet keeps underlay input policy explicit in resolved state.
    NonModal,
}

impl SheetModalMode {
    /// Returns the overlay layer kind for this mode.
    pub const fn overlay_kind(self) -> OverlayLayerKind {
        match self {
            Self::Modal => OverlayLayerKind::Modal,
            Self::NonModal => OverlayLayerKind::NonModalDismissible,
        }
    }

    /// Returns the default outside-press policy for this mode.
    pub const fn default_outside_press_policy(self) -> OutsidePressPolicy {
        match self {
            Self::Modal => OutsidePressPolicy::DismissAndConsume,
            Self::NonModal => OutsidePressPolicy::DismissAndPassThrough,
        }
    }

    /// Returns a stable label.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Modal => "modal",
            Self::NonModal => "non-modal",
        }
    }
}

/// Whether a close affordance is visible inside the sheet surface.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SheetCloseAffordance {
    /// Render a visible close control.
    #[default]
    Visible,
    /// Do not render a visible close control.
    Hidden,
}

impl SheetCloseAffordance {
    /// Returns whether the close control should be rendered.
    pub const fn visible(self) -> bool {
        matches!(self, Self::Visible)
    }
}

/// Resolved sheet color intents.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SheetColors {
    pub(crate) barrier: ColorIntent,
    pub(crate) surface: ColorIntent,
    pub(crate) foreground: ColorIntent,
    pub(crate) muted_foreground: ColorIntent,
    pub(crate) border: ColorIntent,
    pub(crate) trigger_background: ColorIntent,
    pub(crate) trigger_hover_background: ColorIntent,
    pub(crate) trigger_foreground: ColorIntent,
    pub(crate) trigger_border: ColorIntent,
    pub(crate) close_background: ColorIntent,
    pub(crate) close_hover_background: ColorIntent,
    pub(crate) close_foreground: ColorIntent,
    pub(crate) close_border: ColorIntent,
    pub(crate) focus_ring: ColorIntent,
}

impl SheetColors {
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

    /// Returns muted foreground color intent.
    pub const fn muted_foreground(self) -> ColorIntent {
        self.muted_foreground
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

    /// Returns close affordance background color intent.
    pub const fn close_background(self) -> ColorIntent {
        self.close_background
    }

    /// Returns close affordance hover background color intent.
    pub const fn close_hover_background(self) -> ColorIntent {
        self.close_hover_background
    }

    /// Returns close affordance foreground color intent.
    pub const fn close_foreground(self) -> ColorIntent {
        self.close_foreground
    }

    /// Returns close affordance border color intent.
    pub const fn close_border(self) -> ColorIntent {
        self.close_border
    }

    /// Returns focus-ring color intent.
    pub const fn focus_ring(self) -> ColorIntent {
        self.focus_ring
    }
}

/// Resolved sheet metrics.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SheetMetrics {
    trigger_height: UiPx,
    trigger_padding_x: UiPx,
    trigger_padding_y: UiPx,
    padding: UiPx,
    radius: UiPx,
    title_size: UiPx,
    text_size: UiPx,
    surface_size: UiPx,
    inset: UiPx,
    close_size: UiPx,
}

impl SheetMetrics {
    /// Resolves metrics from the shared foundation size vocabulary.
    pub const fn from_size(size: Size) -> Self {
        Self {
            trigger_height: size.button_h(),
            trigger_padding_x: size.button_px(),
            trigger_padding_y: size.button_py(),
            padding: size.button_px(),
            radius: size.control_radius(),
            title_size: ui_px(16.0),
            text_size: size.control_text_px(),
            surface_size: ui_px(360.0),
            inset: ui_px(12.0),
            close_size: size.icon_button_size(),
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

    /// Returns preferred sheet main-axis size.
    pub const fn surface_size(self) -> UiPx {
        self.surface_size
    }

    /// Returns viewport inset used around edge-attached surfaces.
    pub const fn inset(self) -> UiPx {
        self.inset
    }

    /// Returns close affordance square size.
    pub const fn close_size(self) -> UiPx {
        self.close_size
    }
}

/// Resolved sheet state used by tests, demos, and rendering.
#[derive(Debug, Clone, PartialEq)]
pub struct SheetState {
    size: Size,
    disabled: bool,
    open: bool,
    default_open: bool,
    open_mode: SheetOpenMode,
    side: SheetSide,
    modal_mode: SheetModalMode,
    close_affordance: SheetCloseAffordance,
    title: SharedString,
    description: Option<SharedString>,
    outside_press_policy: OutsidePressPolicy,
    escape_key_policy: EscapeKeyPolicy,
    initial_focus_intent: InitialFocusIntent,
    focus_restore_intent: FocusRestoreIntent,
    trigger_selected: bool,
    metrics: SheetMetrics,
    colors: SheetColors,
    focus_ring: FocusRing,
    overlay: OverlayResolvedState,
}

impl SheetState {
    /// Resolves the public state for a sheet.
    #[allow(clippy::too_many_arguments)]
    pub fn resolve(
        size: Size,
        disabled: bool,
        open: Option<bool>,
        default_open: bool,
        side: SheetSide,
        modal_mode: SheetModalMode,
        close_affordance: SheetCloseAffordance,
        title: SharedString,
        description: Option<SharedString>,
        outside_press_policy: OutsidePressPolicy,
        escape_key_policy: EscapeKeyPolicy,
        initial_focus_intent: InitialFocusIntent,
        focus_restore_intent: FocusRestoreIntent,
        tokens: ThemeTokens,
    ) -> Self {
        let open_mode = sheet_open_mode_from_disclosure(
            OverlayDisclosureConfig::new(modal_mode.overlay_kind())
                .controlled_open(open)
                .resolve()
                .open_mode(),
        );
        Self::resolve_with_open_mode(
            size,
            disabled,
            open.unwrap_or(default_open),
            default_open,
            open_mode,
            side,
            modal_mode,
            close_affordance,
            title,
            description,
            outside_press_policy,
            escape_key_policy,
            initial_focus_intent,
            focus_restore_intent,
            tokens,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn resolve_with_open_mode(
        size: Size,
        disabled: bool,
        open: bool,
        default_open: bool,
        open_mode: SheetOpenMode,
        side: SheetSide,
        modal_mode: SheetModalMode,
        close_affordance: SheetCloseAffordance,
        title: SharedString,
        description: Option<SharedString>,
        outside_press_policy: OutsidePressPolicy,
        escape_key_policy: EscapeKeyPolicy,
        initial_focus_intent: InitialFocusIntent,
        focus_restore_intent: FocusRestoreIntent,
        tokens: ThemeTokens,
    ) -> Self {
        let disclosure = OverlayDisclosureConfig::new(modal_mode.overlay_kind())
            .controlled_open(Some(open))
            .default_open(default_open)
            .disabled(disabled)
            .outside_press_policy(outside_press_policy)
            .escape_key_policy(escape_key_policy)
            .initial_focus_intent(initial_focus_intent.clone())
            .focus_restore_intent(focus_restore_intent.clone())
            .resolve();
        let open = disclosure.open();
        let overlay = disclosure.overlay().clone();
        let colors = ThemeResolver::sheet_colors(tokens, open);

        Self {
            size,
            disabled,
            open,
            default_open,
            open_mode,
            side,
            modal_mode,
            close_affordance,
            title,
            description,
            outside_press_policy,
            escape_key_policy,
            initial_focus_intent,
            focus_restore_intent,
            trigger_selected: open,
            metrics: SheetMetrics::from_size(size),
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

    /// Returns whether sheet content is open.
    pub const fn open(&self) -> bool {
        self.open
    }

    /// Returns uncontrolled initial open state.
    pub const fn default_open(&self) -> bool {
        self.default_open
    }

    /// Returns open-state ownership.
    pub const fn open_mode(&self) -> SheetOpenMode {
        self.open_mode
    }

    /// Returns the attached side.
    pub const fn side(&self) -> SheetSide {
        self.side
    }

    /// Returns modal behavior mode.
    pub const fn modal_mode(&self) -> SheetModalMode {
        self.modal_mode
    }

    /// Returns close affordance behavior.
    pub const fn close_affordance(&self) -> SheetCloseAffordance {
        self.close_affordance
    }

    /// Returns sheet title.
    pub fn title(&self) -> &str {
        &self.title
    }

    /// Returns optional sheet description.
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
        Role::Dialog
    }

    /// Returns whether activation handlers should run.
    pub const fn activation_enabled(&self) -> bool {
        !self.disabled
    }

    /// Returns resolved metrics.
    pub const fn metrics(&self) -> SheetMetrics {
        self.metrics
    }

    /// Returns resolved color intents.
    pub const fn colors(&self) -> SheetColors {
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

/// A concrete GPUI sheet component.
#[derive(IntoElement)]
pub struct Sheet {
    id: ElementId,
    trigger_label: SharedString,
    title: SharedString,
    description: Option<SharedString>,
    content: SheetContent,
    size: Size,
    disabled: bool,
    open: Option<bool>,
    default_open: bool,
    side: SheetSide,
    modal_mode: SheetModalMode,
    close_affordance: SheetCloseAffordance,
    outside_press_policy: Option<OutsidePressPolicy>,
    escape_key_policy: EscapeKeyPolicy,
    initial_focus_intent: InitialFocusIntent,
    focus_restore_intent: FocusRestoreIntent,
    focus_targets: Vec<FocusTargetRegistration>,
    tokens: ThemeTokens,
    on_open_change: Option<SheetOpenChangeHandler>,
}

enum SheetContent {
    Text(SharedString),
    Element(AnyElement),
}

#[derive(Clone)]
struct SheetRuntime {
    open: bool,
    close_focus: FocusHandle,
    overlay_binding: Option<OverlayLayerBinding>,
    focus_targets: OverlayFocusTargetSet,
}

impl Sheet {
    /// Creates a text sheet.
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
            content: SheetContent::Text(content.into()),
            size: Size::Medium,
            disabled: false,
            open: None,
            default_open: false,
            side: SheetSide::Right,
            modal_mode: SheetModalMode::Modal,
            close_affordance: SheetCloseAffordance::Visible,
            outside_press_policy: None,
            escape_key_policy: EscapeKeyPolicy::Dismiss,
            initial_focus_intent: InitialFocusIntent::FirstFocusable,
            focus_restore_intent: FocusRestoreIntent::Trigger,
            focus_targets: Vec::new(),
            tokens: ThemeTokens::default(),
            on_open_change: None,
        }
    }

    /// Creates a sheet with simple element content.
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
            content: SheetContent::Element(content.into_any_element()),
            size: Size::Medium,
            disabled: false,
            open: None,
            default_open: false,
            side: SheetSide::Right,
            modal_mode: SheetModalMode::Modal,
            close_affordance: SheetCloseAffordance::Visible,
            outside_press_policy: None,
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

    /// Marks the sheet trigger as disabled.
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

    /// Applies the attached side.
    pub fn side(mut self, side: SheetSide) -> Self {
        self.side = side;
        self
    }

    /// Applies modal behavior mode.
    pub fn modal_mode(mut self, modal_mode: SheetModalMode) -> Self {
        self.modal_mode = modal_mode;
        self
    }

    /// Applies close affordance behavior.
    pub fn close_affordance(mut self, close_affordance: SheetCloseAffordance) -> Self {
        self.close_affordance = close_affordance;
        self
    }

    /// Applies outside-press policy.
    pub fn outside_press_policy(mut self, policy: OutsidePressPolicy) -> Self {
        self.outside_press_policy = Some(policy);
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

    /// Declares a live focus target owned by this sheet layer.
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

    /// Returns resolved sheet state.
    pub fn state(&self) -> SheetState {
        SheetState::resolve(
            self.size,
            self.disabled,
            self.open,
            self.default_open,
            self.side,
            self.modal_mode,
            self.close_affordance,
            self.title.clone(),
            self.description.clone(),
            self.resolved_outside_press_policy(),
            self.escape_key_policy,
            self.initial_focus_intent.clone(),
            self.focus_restore_intent.clone(),
            self.tokens,
        )
    }

    fn resolved_outside_press_policy(&self) -> OutsidePressPolicy {
        self.outside_press_policy
            .unwrap_or_else(|| self.modal_mode.default_outside_press_policy())
    }
}

impl Sizable for Sheet {
    fn with_size(mut self, size: Size) -> Self {
        self.size = size;
        self
    }
}

impl RenderOnce for Sheet {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = ThemeResolver::current(cx);
        let runtime = window.use_keyed_state(self.id.clone(), cx, |_, cx| SheetRuntime {
            open: self.default_open,
            close_focus: cx.focus_handle(),
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

        let state = SheetState::resolve(
            self.size,
            self.disabled,
            self.open,
            self.default_open,
            self.side,
            self.modal_mode,
            self.close_affordance,
            self.title.clone(),
            self.description.clone(),
            self.resolved_outside_press_policy(),
            self.escape_key_policy,
            self.initial_focus_intent.clone(),
            self.focus_restore_intent.clone(),
            self.tokens,
        );
        let state = if state.open() == resolved_open {
            state
        } else {
            SheetState::resolve_with_open_mode(
                self.size,
                self.disabled,
                resolved_open,
                self.default_open,
                state.open_mode(),
                self.side,
                self.modal_mode,
                self.close_affordance,
                self.title.clone(),
                self.description.clone(),
                self.resolved_outside_press_policy(),
                self.escape_key_policy,
                self.initial_focus_intent.clone(),
                self.focus_restore_intent.clone(),
                self.tokens,
            )
        };
        let viewport = window.viewport_size();
        let id = self.id;
        let debug_id = id.to_string();
        let trigger_id: ElementId = (id.clone(), "trigger").into();
        let content_id: ElementId = (id.clone(), "content").into();
        let trigger_label = self.trigger_label;
        let content = self.content;
        let focus_targets = self.focus_targets;
        let metrics = state.metrics();
        let colors = state.colors();
        let focus_ring = state.focus_ring();
        let disabled = state.disabled();
        let open = state.open();
        let trigger_border = theme.resolve(colors.trigger_border());
        let trigger_background = theme.resolve(colors.trigger_background());
        let trigger_foreground = theme.resolve(colors.trigger_foreground());
        let trigger_hover_background = theme.resolve(colors.trigger_hover_background());
        let trigger_focus_shadow = focus_ring_shadow_with_theme(focus_ring, &theme);
        let close_focus = runtime.read(cx).close_focus.clone();
        let on_open_change = self.on_open_change;
        let window_overlay_runtime = WindowOverlayRuntime::for_window(window, cx);
        let ownership = if open_state.controlled() {
            OverlayOwnership::Controlled
        } else {
            OverlayOwnership::Uncontrolled
        };
        let mut registration = OverlayLayerRegistration::new(
            format!("sheet:{debug_id}"),
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
            .expect("sheet overlay registration should remain valid");
        if existing_binding.is_none() {
            runtime.update(cx, |runtime, _| {
                runtime.overlay_binding = Some(overlay_binding.clone());
            });
        }
        let close_registration = FocusTargetRegistration::new(CLOSE_FOCUS_TARGET, &close_focus)
            .with_availability(if state.close_affordance().visible() {
                FocusTargetAvailability::Available
            } else {
                FocusTargetAvailability::Hidden
            });
        let mut registered_focus_targets = runtime.read(cx).focus_targets.clone();
        registered_focus_targets
            .sync(
                &window_overlay_runtime,
                &overlay_binding,
                focus_targets
                    .into_iter()
                    .chain(std::iter::once(close_registration)),
                window,
                cx,
            )
            .expect("sheet focus targets should remain valid");
        runtime.update(cx, |runtime, _| {
            runtime.focus_targets = registered_focus_targets;
        });
        let overlay_adapter = gpui_overlay_state(state.overlay());

        div()
            .id(id.clone())
            .debug_selector({
                let debug_id = debug_id.clone();
                move || format!("sheet:{debug_id}:root")
            })
            .relative()
            .flex()
            .flex_col()
            .items_start()
            .child(
                window_overlay_runtime.focus_target(
                    &overlay_binding,
                    format!("sheet:{debug_id}:trigger-focus-target"),
                    div()
                        .id(trigger_id)
                        .debug_selector({
                            let debug_id = debug_id.clone();
                            move || format!("sheet:{debug_id}:trigger")
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
                        .track_focus(overlay_binding.trigger_focus())
                        .tab_stop(!disabled)
                        .ui_role(state.trigger_role())
                        .aria_label(trigger_label.clone())
                        .aria_selected(state.trigger_selected())
                        .aria_expanded(open)
                        .aria_disabled(disabled)
                        .focus_visible(move |style| style.shadow(trigger_focus_shadow.clone()))
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
                                            "sheet trigger should own its overlay registration",
                                        );
                                })
                        })
                        .child(trigger_label),
                ),
            )
            .when(open, |this| {
                this.child(gpui_full_window_overlay_layer(
                    &overlay_adapter,
                    sheet_layer_element(
                        content,
                        content_id.clone(),
                        debug_id.clone(),
                        state.clone(),
                        viewport,
                        window_overlay_runtime.clone(),
                        overlay_binding.clone(),
                        close_focus.clone(),
                        &theme,
                    ),
                ))
            })
    }
}

fn sheet_surface_element(
    content: SheetContent,
    debug_id: String,
    state: SheetState,
    geometry: SheetSurfaceGeometry,
    window_overlay_runtime: WindowOverlayRuntime,
    overlay_binding: OverlayLayerBinding,
    close_focus: FocusHandle,
    theme: &ThemeContext,
) -> impl IntoElement {
    let metrics = state.metrics();
    let colors = state.colors();
    let surface_debug_id = debug_id.clone();

    window_overlay_runtime.surface(
        &overlay_binding,
        OverlayInsideRegionId::new("surface"),
        format!("sheet:{debug_id}:surface-region"),
        div()
            .id("sheet-surface")
            .debug_selector(move || format!("sheet:{surface_debug_id}:surface"))
            .absolute()
            .left(gpui_px_from_ui(geometry.left))
            .top(gpui_px_from_ui(geometry.top))
            .w(gpui_px_from_ui(geometry.width))
            .h(gpui_px_from_ui(geometry.height))
            .p(gpui_px_from_ui(metrics.padding()))
            .flex()
            .flex_col()
            .gap_3()
            .rounded(gpui_px_from_ui(metrics.radius()))
            .border_1()
            .border_color(theme.resolve(colors.border()))
            .bg(theme.resolve(colors.surface()))
            .text_color(theme.resolve(colors.foreground()))
            .text_size(gpui_px_from_ui(metrics.text_size()))
            .line_height(gpui_px_from_ui(metrics.text_size()))
            .shadow_lg()
            .occlude()
            .tab_group()
            .focusable()
            .track_focus(overlay_binding.surface_focus())
            .ui_role(state.content_role())
            .aria_label(state.title().to_owned())
            .child(
                div()
                    .flex()
                    .items_start()
                    .justify_between()
                    .gap_3()
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(
                                div()
                                    .text_size(gpui_px_from_ui(metrics.title_size()))
                                    .font_weight(open_gpui::FontWeight::BOLD)
                                    .line_height(px(22.0))
                                    .child(state.title().to_owned()),
                            )
                            .when_some(
                                state.description().map(ToOwned::to_owned),
                                |this, description| {
                                    this.child(
                                        div()
                                            .text_color(theme.resolve(colors.muted_foreground()))
                                            .child(description),
                                    )
                                },
                            ),
                    )
                    .when(state.close_affordance().visible(), |this| {
                        this.child(sheet_close_button(
                            &state,
                            debug_id.clone(),
                            close_focus.clone(),
                            window_overlay_runtime.clone(),
                            overlay_binding.clone(),
                            theme,
                        ))
                    }),
            )
            .child(div().flex_1().children(children_from_content(content))),
    )
}

fn sheet_layer_element(
    content: SheetContent,
    content_id: ElementId,
    debug_id: String,
    state: SheetState,
    viewport: open_gpui::Size<Pixels>,
    window_overlay_runtime: WindowOverlayRuntime,
    overlay_binding: OverlayLayerBinding,
    close_focus: FocusHandle,
    theme: &ThemeContext,
) -> impl IntoElement {
    let metrics = state.metrics();
    let geometry = sheet_surface_geometry(state.side(), metrics, ui_size_from_gpui_size(viewport));

    if state.modal_mode() == SheetModalMode::Modal {
        return modal_sheet_layer_element(
            content,
            content_id,
            debug_id,
            state,
            viewport,
            geometry,
            window_overlay_runtime,
            overlay_binding,
            close_focus,
            theme,
        )
        .into_any_element();
    }

    div()
        .id(content_id)
        .debug_selector({
            let debug_id = debug_id.clone();
            move || format!("sheet:{debug_id}:layer")
        })
        .absolute()
        .left(px(0.0))
        .top(px(0.0))
        .w(viewport.width)
        .h(viewport.height)
        .child(
            sheet_surface_element(
                content,
                debug_id,
                state,
                geometry,
                window_overlay_runtime,
                overlay_binding,
                close_focus,
                theme,
            )
            .into_any_element(),
        )
        .into_any_element()
}

fn modal_sheet_layer_element(
    content: SheetContent,
    content_id: ElementId,
    debug_id: String,
    state: SheetState,
    viewport: open_gpui::Size<Pixels>,
    geometry: SheetSurfaceGeometry,
    window_overlay_runtime: WindowOverlayRuntime,
    overlay_binding: OverlayLayerBinding,
    close_focus: FocusHandle,
    theme: &ThemeContext,
) -> impl IntoElement {
    let colors = state.colors();

    div()
        .id(content_id)
        .debug_selector({
            let debug_id = debug_id.clone();
            move || format!("sheet:{debug_id}:layer")
        })
        .absolute()
        .left(px(0.0))
        .top(px(0.0))
        .w(viewport.width)
        .h(viewport.height)
        .bg(theme.resolve(colors.barrier()))
        .child(sheet_surface_element(
            content,
            debug_id,
            state,
            geometry,
            window_overlay_runtime,
            overlay_binding,
            close_focus,
            theme,
        ))
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct SheetSurfaceGeometry {
    left: UiPx,
    top: UiPx,
    width: UiPx,
    height: UiPx,
}

fn sheet_surface_geometry(
    side: SheetSide,
    metrics: SheetMetrics,
    viewport: UiSize,
) -> SheetSurfaceGeometry {
    let inset = metrics.inset();
    match side {
        SheetSide::Left => SheetSurfaceGeometry {
            left: inset,
            top: inset,
            width: metrics.surface_size().min(viewport.width - inset * 2.0),
            height: viewport.height - inset * 2.0,
        },
        SheetSide::Right => {
            let width = metrics.surface_size().min(viewport.width - inset * 2.0);
            SheetSurfaceGeometry {
                left: viewport.width - width - inset,
                top: inset,
                width,
                height: viewport.height - inset * 2.0,
            }
        }
        SheetSide::Top => SheetSurfaceGeometry {
            left: inset,
            top: inset,
            width: viewport.width - inset * 2.0,
            height: metrics.surface_size().min(viewport.height - inset * 2.0),
        },
        SheetSide::Bottom => {
            let height = metrics.surface_size().min(viewport.height - inset * 2.0);
            SheetSurfaceGeometry {
                left: inset,
                top: viewport.height - height - inset,
                width: viewport.width - inset * 2.0,
                height,
            }
        }
    }
}

fn sheet_close_button(
    state: &SheetState,
    debug_id: String,
    close_focus: FocusHandle,
    window_overlay_runtime: WindowOverlayRuntime,
    overlay_binding: OverlayLayerBinding,
    theme: &ThemeContext,
) -> impl IntoElement {
    let metrics = state.metrics();
    let colors = state.colors();
    let focus_ring = state.focus_ring();
    let close_border = theme.resolve(colors.close_border());
    let close_background = theme.resolve(colors.close_background());
    let close_foreground = theme.resolve(colors.close_foreground());
    let close_hover_background = theme.resolve(colors.close_hover_background());
    let close_focus_shadow = focus_ring_shadow_with_theme(focus_ring, theme);

    div()
        .id("sheet-close")
        .debug_selector(move || format!("sheet:{debug_id}:close"))
        .w(gpui_px_from_ui(metrics.close_size()))
        .h(gpui_px_from_ui(metrics.close_size()))
        .flex()
        .items_center()
        .justify_center()
        .rounded(gpui_px_from_ui(metrics.radius()))
        .border_1()
        .border_color(close_border)
        .bg(close_background)
        .text_color(close_foreground)
        .text_size(gpui_px_from_ui(metrics.text_size()))
        .line_height(gpui_px_from_ui(metrics.text_size()))
        .focusable()
        .track_focus(&close_focus)
        .tab_stop(true)
        .ui_role(Role::Button)
        .aria_label("Close sheet")
        .focus_visible(move |style| style.shadow(close_focus_shadow.clone()))
        .cursor_pointer()
        .hover(move |style| style.bg(close_hover_background))
        .on_click(move |_event: &ClickEvent, window, cx| {
            cx.stop_propagation();
            window_overlay_runtime
                .request_open_change(
                    &overlay_binding,
                    false,
                    DismissReason::CloseAction,
                    window,
                    cx,
                )
                .expect("sheet close action should own its overlay registration");
        })
        .child("x")
}

#[cfg(test)]
fn sheet_close_is_initial_focus_target(state: &SheetState, intent: &InitialFocusIntent) -> bool {
    if !state.close_affordance().visible() {
        return false;
    }

    let close_target = |target: &FocusTargetId| target.as_str() == CLOSE_FOCUS_TARGET;

    match intent {
        InitialFocusIntent::None => false,
        InitialFocusIntent::FirstFocusable => true,
        InitialFocusIntent::Target(target) => close_target(target),
        InitialFocusIntent::TargetOrFirstFocusable(_) => true,
    }
}

fn children_from_content(content: SheetContent) -> Vec<AnyElement> {
    match content {
        SheetContent::Text(text) => vec![div().child(text).into_any_element()],
        SheetContent::Element(element) => vec![element],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_resolver_preserves_uncontrolled_open_mode_with_runtime_open() {
        let state = SheetState::resolve_with_open_mode(
            Size::Medium,
            false,
            true,
            false,
            SheetOpenMode::Uncontrolled,
            SheetSide::Right,
            SheetModalMode::Modal,
            SheetCloseAffordance::Visible,
            "Inspector".into(),
            None,
            OutsidePressPolicy::DismissAndConsume,
            EscapeKeyPolicy::Dismiss,
            InitialFocusIntent::FirstFocusable,
            FocusRestoreIntent::Trigger,
            ThemeTokens::default(),
        );

        assert!(state.open());
        assert!(!state.default_open());
        assert_eq!(state.open_mode(), SheetOpenMode::Uncontrolled);
    }

    #[test]
    fn hidden_close_affordance_is_not_a_focusable_target() {
        let state = SheetState::resolve(
            Size::Medium,
            false,
            Some(true),
            false,
            SheetSide::Bottom,
            SheetModalMode::Modal,
            SheetCloseAffordance::Hidden,
            "Queue".into(),
            None,
            OutsidePressPolicy::Ignore,
            EscapeKeyPolicy::Dismiss,
            InitialFocusIntent::FirstFocusable,
            FocusRestoreIntent::Trigger,
            ThemeTokens::default(),
        );

        assert_eq!(state.close_affordance(), SheetCloseAffordance::Hidden);
        assert!(!state.close_affordance().visible());
        assert!(!sheet_close_is_initial_focus_target(
            &state,
            state.initial_focus_intent()
        ));
    }

    #[test]
    fn exact_sheet_focus_target_does_not_fall_back_to_close() {
        let state = SheetState::resolve(
            Size::Medium,
            false,
            Some(true),
            false,
            SheetSide::Right,
            SheetModalMode::Modal,
            SheetCloseAffordance::Visible,
            "Inspector".into(),
            None,
            OutsidePressPolicy::DismissAndConsume,
            EscapeKeyPolicy::Dismiss,
            InitialFocusIntent::Target(FocusTargetId::new("sheet.content")),
            FocusRestoreIntent::Trigger,
            ThemeTokens::default(),
        );

        assert!(!sheet_close_is_initial_focus_target(
            &state,
            state.initial_focus_intent()
        ));
        assert!(sheet_close_is_initial_focus_target(
            &state,
            &InitialFocusIntent::TargetOrFirstFocusable(FocusTargetId::new("sheet.content"))
        ));
        assert!(sheet_close_is_initial_focus_target(
            &state,
            &InitialFocusIntent::Target(FocusTargetId::new(CLOSE_FOCUS_TARGET))
        ));
    }
}
