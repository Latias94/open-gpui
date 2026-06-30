//! GPUI adapter helpers for shared overlay behavior.

use open_gpui::{Anchor, App, Edges, Pixels, Point, Window, point, px};
use open_gpui_ui_core::{
    DismissReason, EscapeKeyPolicy, FocusRestoreIntent, InitialFocusIntent, OutsidePressPolicy,
    OverlayAnchorInput, OverlayLayerKind, OverlayLayerPolicy, OverlayLayerState,
    OverlayPlacementAlignment, OverlayPlacementInput, OverlayPlacementSide, OverlayPresence, Rect,
    UiPx,
};

pub(crate) use crate::geometry::{gpui_point_from_ui, gpui_px_from_ui, ui_point_from_gpui};
pub use open_gpui_ui_core::OverlayResolvedState;

/// Default margin used when snapping an anchored overlay inside the window.
pub const DEFAULT_OVERLAY_SAFE_MARGIN: Pixels = px(8.0);

/// Renderer-facing adapter state resolved from the shared overlay policy.
#[derive(Debug, Clone, PartialEq)]
pub struct GpuiOverlayState {
    policy: OverlayLayerPolicy,
    layer_state: OverlayLayerState,
    deferred_priority: usize,
    snap_margin: Pixels,
}

impl GpuiOverlayState {
    /// Resolves adapter state from a shared overlay policy.
    pub fn resolve(
        policy: OverlayLayerPolicy,
        deferred_priority: usize,
        snap_margin: Pixels,
    ) -> Self {
        let layer_state = policy.layer_state();

        Self {
            policy,
            layer_state,
            deferred_priority,
            snap_margin,
        }
    }

    /// Resolves adapter state from renderer-neutral overlay state.
    pub fn from_resolved(
        overlay: &OverlayResolvedState,
        deferred_priority: usize,
        snap_margin: Pixels,
    ) -> Self {
        let layer_state = overlay.layer_state();
        let policy = overlay.policy().clone();

        Self {
            policy,
            layer_state,
            deferred_priority,
            snap_margin,
        }
    }

    /// Returns the shared overlay policy.
    pub const fn policy(&self) -> &OverlayLayerPolicy {
        &self.policy
    }

    /// Returns the resolved layer state.
    pub const fn layer_state(&self) -> OverlayLayerState {
        self.layer_state
    }

    /// Returns the deferred paint priority to pass to GPUI.
    pub const fn deferred_priority(&self) -> usize {
        self.deferred_priority
    }

    /// Returns the snap-to-window margin.
    pub const fn snap_margin(&self) -> Pixels {
        self.snap_margin
    }

    /// Returns the snap margin as GPUI edges.
    pub fn snap_edges(&self) -> Edges<Pixels> {
        self.snap_margin.into()
    }

    /// Returns whether the adapter should render a deferred anchored layer.
    pub const fn should_render_deferred_layer(&self) -> bool {
        self.layer_state.visible()
    }

    /// Returns whether the adapter should attach outside-press handling.
    pub const fn wants_outside_press_handler(&self) -> bool {
        self.layer_state.wants_outside_press()
    }
}

/// Builder for resolving a GPUI overlay adapter state.
#[derive(Debug, Clone, PartialEq)]
pub struct GpuiOverlayAdapterConfig {
    kind: OverlayLayerKind,
    presence: OverlayPresence,
    outside_press: Option<OutsidePressPolicy>,
    escape_key: Option<EscapeKeyPolicy>,
    focus_restore: Option<FocusRestoreIntent>,
    initial_focus: Option<InitialFocusIntent>,
    deferred_priority: usize,
    snap_margin: Pixels,
}

impl GpuiOverlayAdapterConfig {
    /// Creates a config with kind-specific overlay policy defaults.
    pub fn new(kind: OverlayLayerKind, presence: OverlayPresence) -> Self {
        Self {
            kind,
            presence,
            outside_press: None,
            escape_key: None,
            focus_restore: None,
            initial_focus: None,
            deferred_priority: default_deferred_priority(kind),
            snap_margin: DEFAULT_OVERLAY_SAFE_MARGIN,
        }
    }

    /// Applies a custom outside-press policy.
    pub fn outside_press_policy(mut self, policy: OutsidePressPolicy) -> Self {
        self.outside_press = Some(policy);
        self
    }

    /// Applies a custom Escape-key policy.
    pub fn escape_key_policy(mut self, policy: EscapeKeyPolicy) -> Self {
        self.escape_key = Some(policy);
        self
    }

    /// Applies a custom focus-restore intent.
    pub fn focus_restore_intent(mut self, intent: FocusRestoreIntent) -> Self {
        self.focus_restore = Some(intent);
        self
    }

    /// Applies a custom initial-focus intent.
    pub fn initial_focus_intent(mut self, intent: InitialFocusIntent) -> Self {
        self.initial_focus = Some(intent);
        self
    }

    /// Applies a deferred paint priority.
    pub fn deferred_priority(mut self, priority: usize) -> Self {
        self.deferred_priority = priority;
        self
    }

    /// Applies a snap-to-window margin.
    pub fn snap_margin(mut self, margin: Pixels) -> Self {
        self.snap_margin = margin;
        self
    }

    /// Resolves the renderer-neutral overlay state.
    pub fn resolved_state(self) -> OverlayResolvedState {
        let mut policy = OverlayLayerPolicy::new(self.kind, self.presence);

        if let Some(outside_press) = self.outside_press {
            policy = policy.with_outside_press_policy(outside_press);
        }
        if let Some(escape_key) = self.escape_key {
            policy = policy.with_escape_key_policy(escape_key);
        }
        if let Some(focus_restore) = self.focus_restore {
            policy = policy.with_focus_restore_intent(focus_restore);
        }
        if let Some(initial_focus) = self.initial_focus {
            policy = policy.with_initial_focus_intent(initial_focus);
        }

        OverlayResolvedState::resolve(policy)
    }

    /// Resolves the adapter state.
    pub fn state(self) -> GpuiOverlayState {
        let deferred_priority = self.deferred_priority;
        let snap_margin = self.snap_margin;
        let overlay = self.resolved_state();
        GpuiOverlayState::from_resolved(&overlay, deferred_priority, snap_margin)
    }
}

/// Derives the default GPUI adapter state from renderer-neutral overlay state.
pub fn gpui_overlay_state(overlay: &OverlayResolvedState) -> GpuiOverlayState {
    GpuiOverlayState::from_resolved(
        overlay,
        default_deferred_priority(overlay.policy().kind()),
        DEFAULT_OVERLAY_SAFE_MARGIN,
    )
}

/// Resolved GPUI placement state for an anchored overlay.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GpuiOverlayPlacement {
    anchor: Anchor,
    position: Option<Point<Pixels>>,
    offset: Point<Pixels>,
    snap_margin: Pixels,
    safe_bounds: Option<Rect>,
}

impl GpuiOverlayPlacement {
    /// Resolves GPUI placement fields from renderer-neutral placement input.
    pub fn resolve(input: OverlayPlacementInput, snap_margin: Pixels) -> Self {
        Self {
            anchor: gpui_anchor(input.side(), input.alignment()),
            position: input
                .preferred_anchor_bounds()
                .map(|bounds| gpui_anchor_position(bounds, input.side(), input.alignment())),
            offset: gpui_offset(input.side(), input.offset()),
            snap_margin,
            safe_bounds: input.safe_bounds(),
        }
    }

    /// Returns the GPUI anchor.
    pub const fn anchor(self) -> Anchor {
        self.anchor
    }

    /// Returns the preferred window position.
    pub const fn position(self) -> Option<Point<Pixels>> {
        self.position
    }

    /// Returns the GPUI offset.
    pub const fn offset(self) -> Point<Pixels> {
        self.offset
    }

    /// Returns the snap-to-window margin.
    pub const fn snap_margin(self) -> Pixels {
        self.snap_margin
    }

    /// Returns the snap margin as GPUI edges.
    pub fn snap_edges(self) -> Edges<Pixels> {
        self.snap_margin.into()
    }

    /// Returns the original safe bounds, when provided.
    pub const fn safe_bounds(self) -> Option<Rect> {
        self.safe_bounds
    }
}

/// Resolved open-change request emitted by overlay adapters.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OverlayOpenChange {
    open: bool,
    reason: DismissReason,
    consume_event: bool,
    allow_underlay: bool,
}

impl OverlayOpenChange {
    /// Creates an open-change request.
    pub const fn new(
        open: bool,
        reason: DismissReason,
        consume_event: bool,
        allow_underlay: bool,
    ) -> Self {
        Self {
            open,
            reason,
            consume_event,
            allow_underlay,
        }
    }

    /// Returns the requested open state.
    pub const fn open(self) -> bool {
        self.open
    }

    /// Returns the dismiss or open-change reason.
    pub const fn reason(self) -> DismissReason {
        self.reason
    }

    /// Returns whether the source event should be consumed.
    pub const fn consumes_event(self) -> bool {
        self.consume_event
    }

    /// Returns whether underlay dispatch may continue.
    pub const fn allows_underlay_dispatch(self) -> bool {
        self.allow_underlay
    }
}

/// Shared open-state ownership for overlay-like component adapters.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum OverlayDisclosureOpenMode {
    /// Open state is owned by the component adapter after initialization.
    #[default]
    Uncontrolled,
    /// Open state is provided by the caller.
    Controlled,
}

/// Input used to resolve renderer-neutral disclosure state.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct OverlayDisclosureConfig {
    controlled_open: Option<bool>,
    default_open: bool,
    disabled: bool,
    openable: bool,
    layer_kind: OverlayLayerKind,
    outside_press_policy: Option<OutsidePressPolicy>,
    escape_key_policy: Option<EscapeKeyPolicy>,
    initial_focus_intent: Option<InitialFocusIntent>,
    focus_restore_intent: Option<FocusRestoreIntent>,
}

impl OverlayDisclosureConfig {
    /// Creates a disclosure config for an overlay layer kind.
    pub(crate) fn new(layer_kind: OverlayLayerKind) -> Self {
        Self {
            controlled_open: None,
            default_open: false,
            disabled: false,
            openable: true,
            layer_kind,
            outside_press_policy: None,
            escape_key_policy: None,
            initial_focus_intent: None,
            focus_restore_intent: None,
        }
    }

    /// Applies caller-owned open state.
    pub(crate) const fn controlled_open(mut self, open: Option<bool>) -> Self {
        self.controlled_open = open;
        self
    }

    /// Applies uncontrolled initial open state.
    pub(crate) const fn default_open(mut self, open: bool) -> Self {
        self.default_open = open;
        self
    }

    /// Applies disabled state.
    pub(crate) const fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Applies whether this disclosure has content that can be opened.
    pub(crate) const fn openable(mut self, openable: bool) -> Self {
        self.openable = openable;
        self
    }

    /// Applies outside-press dismissal policy.
    pub(crate) fn outside_press_policy(mut self, policy: OutsidePressPolicy) -> Self {
        self.outside_press_policy = Some(policy);
        self
    }

    /// Applies Escape-key dismissal policy.
    pub(crate) fn escape_key_policy(mut self, policy: EscapeKeyPolicy) -> Self {
        self.escape_key_policy = Some(policy);
        self
    }

    /// Applies initial-focus intent.
    pub(crate) fn initial_focus_intent(mut self, intent: InitialFocusIntent) -> Self {
        self.initial_focus_intent = Some(intent);
        self
    }

    /// Applies focus-restore intent.
    pub(crate) fn focus_restore_intent(mut self, intent: FocusRestoreIntent) -> Self {
        self.focus_restore_intent = Some(intent);
        self
    }

    /// Resolves disclosure state.
    pub(crate) fn resolve(self) -> OverlayDisclosureState {
        let open_mode = if self.controlled_open.is_some() {
            OverlayDisclosureOpenMode::Controlled
        } else {
            OverlayDisclosureOpenMode::Uncontrolled
        };
        let requested_open = self.controlled_open.unwrap_or(self.default_open);
        let open = requested_open && !self.disabled && self.openable;
        let presence = OverlayPresence::from_open(open);
        let mut config = GpuiOverlayAdapterConfig::new(self.layer_kind, presence);
        if let Some(outside_press_policy) = self.outside_press_policy {
            config = config.outside_press_policy(outside_press_policy);
        }
        if let Some(escape_key_policy) = self.escape_key_policy {
            config = config.escape_key_policy(escape_key_policy);
        }
        if let Some(initial_focus_intent) = self.initial_focus_intent {
            config = config.initial_focus_intent(initial_focus_intent);
        }
        if let Some(focus_restore_intent) = self.focus_restore_intent {
            config = config.focus_restore_intent(focus_restore_intent);
        }
        let overlay = config.resolved_state();

        OverlayDisclosureState {
            open,
            open_mode,
            overlay,
        }
    }
}

/// Resolved shared disclosure state for overlay-like adapters.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct OverlayDisclosureState {
    open: bool,
    open_mode: OverlayDisclosureOpenMode,
    overlay: OverlayResolvedState,
}

impl OverlayDisclosureState {
    /// Returns the gated open value adapters should render.
    pub(crate) const fn open(&self) -> bool {
        self.open
    }

    /// Returns open-state ownership.
    pub(crate) const fn open_mode(&self) -> OverlayDisclosureOpenMode {
        self.open_mode
    }

    /// Returns renderer-neutral overlay state.
    pub(crate) const fn overlay(&self) -> &OverlayResolvedState {
        &self.overlay
    }
}

/// Resolves an Escape-key open-change request.
pub const fn escape_open_change(policy: &OverlayLayerPolicy) -> Option<OverlayOpenChange> {
    match policy.escape_key_policy() {
        EscapeKeyPolicy::Ignore => None,
        EscapeKeyPolicy::Dismiss if policy.presence().interactive() => Some(OverlayOpenChange {
            open: false,
            reason: DismissReason::EscapeKey,
            consume_event: true,
            allow_underlay: false,
        }),
        EscapeKeyPolicy::Dismiss => None,
    }
}

/// Resolves an outside-press open-change request.
pub const fn outside_press_open_change(policy: &OverlayLayerPolicy) -> Option<OverlayOpenChange> {
    if !policy.presence().interactive() {
        return None;
    }

    let outcome = policy.outside_press_policy().resolve();
    if let Some(reason) = outcome.dismiss_reason() {
        Some(OverlayOpenChange {
            open: false,
            reason,
            consume_event: outcome.consumes_event(),
            allow_underlay: outcome.allows_underlay_dispatch(),
        })
    } else {
        None
    }
}

/// Consumes a GPUI event that was handled by overlay open, close, or barrier behavior.
pub(crate) fn consume_overlay_event(window: &mut Window, cx: &mut App) {
    cx.stop_propagation();
    window.prevent_default();
}

/// Returns whether the overlay should restore focus back to the trigger.
pub const fn focus_restore_requests_trigger(intent: &FocusRestoreIntent) -> bool {
    matches!(
        intent,
        FocusRestoreIntent::Trigger | FocusRestoreIntent::TriggerOrFallback(_)
    )
}

/// Resolved adapter-owned open state for controlled and uncontrolled overlays.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct OverlayRuntimeState {
    open: bool,
    controlled: bool,
    runtime_changed: bool,
}

impl OverlayRuntimeState {
    /// Returns the resolved open value the adapter should render.
    pub(crate) const fn open(self) -> bool {
        self.open
    }

    /// Returns whether the open value was provided by the caller.
    pub(crate) const fn controlled(self) -> bool {
        self.controlled
    }

    /// Returns whether the stored runtime value should be synchronized to the resolved value.
    pub(crate) const fn runtime_changed(self) -> bool {
        self.runtime_changed
    }
}

/// Resolves controlled/uncontrolled open state without emitting callbacks.
pub(crate) const fn resolve_overlay_open_state(
    controlled_open: Option<bool>,
    runtime_open: bool,
) -> OverlayRuntimeState {
    match controlled_open {
        Some(open) => OverlayRuntimeState {
            open,
            controlled: true,
            runtime_changed: runtime_open != open,
        },
        None => OverlayRuntimeState {
            open: runtime_open,
            controlled: false,
            runtime_changed: false,
        },
    }
}

/// Updates runtime open state without invoking component callbacks.
pub(crate) fn set_overlay_open(runtime_open: &mut bool, open: bool) {
    *runtime_open = open;
}

/// Emits the bool open-change callback after runtime state has been updated.
pub(crate) fn emit_overlay_open_change(
    open: bool,
    on_open_change: Option<&dyn Fn(bool, &mut Window, &mut App)>,
    window: &mut Window,
    cx: &mut App,
) {
    if let Some(on_open_change) = on_open_change {
        on_open_change(open, window, cx);
    }
}

/// Restores focus to a trigger handle when the focus-restore policy requests it.
pub(crate) fn restore_overlay_focus(
    focus_restore: &FocusRestoreIntent,
    trigger_focus: Option<open_gpui::FocusHandle>,
    defer_focus_restore: bool,
    window: &mut Window,
    cx: &mut App,
) {
    if focus_restore_requests_trigger(focus_restore)
        && let Some(trigger_focus) = trigger_focus
    {
        if defer_focus_restore {
            window.defer(cx, move |window, cx| trigger_focus.focus(window, cx));
        } else {
            trigger_focus.focus(window, cx);
        }
    }
}

/// Returns the default GPUI deferred priority for an overlay kind.
pub const fn default_deferred_priority(kind: OverlayLayerKind) -> usize {
    match kind {
        OverlayLayerKind::Tooltip => 1,
        OverlayLayerKind::NonModalDismissible => 2,
        OverlayLayerKind::Menu => 3,
        OverlayLayerKind::Modal => 4,
    }
}

/// Converts renderer-neutral placement into a GPUI anchor.
pub const fn gpui_anchor(
    side: OverlayPlacementSide,
    alignment: OverlayPlacementAlignment,
) -> Anchor {
    match (side, alignment) {
        (OverlayPlacementSide::Top, OverlayPlacementAlignment::Start) => Anchor::BottomLeft,
        (OverlayPlacementSide::Top, OverlayPlacementAlignment::Center) => Anchor::BottomCenter,
        (OverlayPlacementSide::Top, OverlayPlacementAlignment::End) => Anchor::BottomRight,
        (OverlayPlacementSide::Right, _) => Anchor::LeftCenter,
        (OverlayPlacementSide::Bottom, OverlayPlacementAlignment::Start) => Anchor::TopLeft,
        (OverlayPlacementSide::Bottom, OverlayPlacementAlignment::Center) => Anchor::TopCenter,
        (OverlayPlacementSide::Bottom, OverlayPlacementAlignment::End) => Anchor::TopRight,
        (OverlayPlacementSide::Left, _) => Anchor::RightCenter,
    }
}

fn gpui_anchor_position(
    bounds: Rect,
    side: OverlayPlacementSide,
    alignment: OverlayPlacementAlignment,
) -> Point<Pixels> {
    let point = match (side, alignment) {
        (OverlayPlacementSide::Top, OverlayPlacementAlignment::Start) => bounds.top_left(),
        (OverlayPlacementSide::Top, OverlayPlacementAlignment::Center) => bounds.top_center(),
        (OverlayPlacementSide::Top, OverlayPlacementAlignment::End) => bounds.top_right(),
        (OverlayPlacementSide::Right, _) => bounds.right_center(),
        (OverlayPlacementSide::Bottom, OverlayPlacementAlignment::Start) => bounds.bottom_left(),
        (OverlayPlacementSide::Bottom, OverlayPlacementAlignment::Center) => bounds.bottom_center(),
        (OverlayPlacementSide::Bottom, OverlayPlacementAlignment::End) => bounds.bottom_right(),
        (OverlayPlacementSide::Left, _) => bounds.left_center(),
    };
    gpui_point_from_ui(point)
}

fn gpui_offset(side: OverlayPlacementSide, offset: UiPx) -> Point<Pixels> {
    let offset = gpui_px_from_ui(offset);
    match side {
        OverlayPlacementSide::Top => point(px(0.0), -offset),
        OverlayPlacementSide::Right => point(offset, px(0.0)),
        OverlayPlacementSide::Bottom => point(px(0.0), offset),
        OverlayPlacementSide::Left => point(-offset, px(0.0)),
    }
}

/// Creates a point anchor placement input for context-menu-like adapters.
pub fn point_anchor_placement(
    point: Point<Pixels>,
    content_size: open_gpui_ui_core::OverlaySize,
) -> OverlayPlacementInput {
    OverlayPlacementInput::new(
        OverlayAnchorInput::from_point(ui_point_from_gpui(point)),
        content_size,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn overlay_runtime_state_resolves_controlled_without_emitting() {
        let uncontrolled = resolve_overlay_open_state(None, true);
        assert!(uncontrolled.open());
        assert!(!uncontrolled.controlled());
        assert!(!uncontrolled.runtime_changed());

        let controlled_same = resolve_overlay_open_state(Some(true), true);
        assert!(controlled_same.open());
        assert!(controlled_same.controlled());
        assert!(!controlled_same.runtime_changed());

        let controlled_changed = resolve_overlay_open_state(Some(false), true);
        assert!(!controlled_changed.open());
        assert!(controlled_changed.controlled());
        assert!(controlled_changed.runtime_changed());
    }

    #[test]
    fn overlay_disclosure_state_resolves_open_mode_and_policy() {
        let state = OverlayDisclosureConfig::new(OverlayLayerKind::Modal)
            .controlled_open(Some(true))
            .default_open(false)
            .outside_press_policy(OutsidePressPolicy::Ignore)
            .escape_key_policy(EscapeKeyPolicy::Dismiss)
            .initial_focus_intent(InitialFocusIntent::FirstFocusable)
            .focus_restore_intent(FocusRestoreIntent::Trigger)
            .resolve();

        assert!(state.open());
        assert_eq!(state.open_mode(), OverlayDisclosureOpenMode::Controlled);
        assert_eq!(state.overlay().policy().kind(), OverlayLayerKind::Modal);
        assert_eq!(
            state.overlay().policy().outside_press_policy(),
            OutsidePressPolicy::Ignore
        );
        assert_eq!(
            state.overlay().policy().escape_key_policy(),
            EscapeKeyPolicy::Dismiss
        );
        assert_eq!(
            state.overlay().policy().initial_focus_intent(),
            &InitialFocusIntent::FirstFocusable
        );
        assert_eq!(
            state.overlay().policy().focus_restore_intent(),
            &FocusRestoreIntent::Trigger
        );
    }

    #[test]
    fn overlay_disclosure_state_gates_disabled_and_unopenable_surfaces() {
        let disabled = OverlayDisclosureConfig::new(OverlayLayerKind::NonModalDismissible)
            .default_open(true)
            .disabled(true)
            .resolve();
        let unopenable = OverlayDisclosureConfig::new(OverlayLayerKind::Menu)
            .default_open(true)
            .openable(false)
            .resolve();

        assert!(!disabled.open());
        assert_eq!(
            disabled.open_mode(),
            OverlayDisclosureOpenMode::Uncontrolled
        );
        assert!(!disabled.overlay().policy().presence().interactive());

        assert!(!unopenable.open());
        assert_eq!(unopenable.overlay().policy().kind(), OverlayLayerKind::Menu);
        assert!(!unopenable.overlay().policy().presence().interactive());
    }
}
