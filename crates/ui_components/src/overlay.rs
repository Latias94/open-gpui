//! GPUI adapter helpers for shared overlay behavior.

use open_gpui::{Anchor, Edges, Pixels, Point, point, px};
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
