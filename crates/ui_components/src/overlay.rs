//! GPUI adapter helpers for shared overlay behavior.

use open_gpui::{
    Anchor, AnyElement, Edges, IntoElement, ParentElement, Pixels, Point, anchored, deferred,
    point, px,
};
use open_gpui_ui_core::{
    EscapeKeyPolicy, FocusRestoreIntent, InitialFocusIntent, OutsidePressPolicy,
    OverlayAnchorInput, OverlayLayerKind, OverlayLayerPolicy, OverlayLayerState,
    OverlayPlacementAlignment, OverlayPlacementFit, OverlayPlacementInput,
    OverlayPlacementResolution, OverlayPlacementSide, OverlayPlacementTrace, OverlayPresence, Rect,
    UiPx, resolve_overlay_placement,
};

mod runtime;

pub(crate) use crate::geometry::{gpui_point_from_ui, gpui_px_from_ui, ui_point_from_gpui};
pub use open_gpui_ui_core::OverlayResolvedState;
pub(crate) use runtime::{
    OverlayDisclosureConfig, OverlayDisclosureOpenMode, consume_overlay_event,
    emit_overlay_open_change, resolve_overlay_open_state, restore_overlay_focus, set_overlay_open,
};
pub use runtime::{OverlayOpenChange, escape_open_change, outside_press_open_change};

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
#[derive(Debug, Clone, PartialEq)]
pub struct GpuiOverlayPlacement {
    anchor: Anchor,
    position: Option<Point<Pixels>>,
    offset: Point<Pixels>,
    snap_margin: Pixels,
    resolution: OverlayPlacementResolution,
}

impl GpuiOverlayPlacement {
    /// Resolves GPUI placement fields from renderer-neutral placement input.
    pub fn resolve(input: OverlayPlacementInput, snap_margin: Pixels) -> Self {
        let has_anchor_position = input.preferred_anchor_bounds().is_some();
        let resolution = resolve_overlay_placement(input);

        Self {
            anchor: gpui_anchor(resolution.side(), resolution.alignment()),
            position: has_anchor_position.then(|| gpui_point_from_ui(resolution.anchor_point())),
            offset: gpui_offset(resolution.side(), resolution.offset()),
            snap_margin,
            resolution,
        }
    }

    /// Returns the GPUI anchor.
    pub const fn anchor(&self) -> Anchor {
        self.anchor
    }

    /// Returns the preferred window position.
    pub const fn position(&self) -> Option<Point<Pixels>> {
        self.position
    }

    /// Returns the GPUI offset.
    pub const fn offset(&self) -> Point<Pixels> {
        self.offset
    }

    /// Returns the snap-to-window margin.
    pub const fn snap_margin(&self) -> Pixels {
        self.snap_margin
    }

    /// Returns the snap margin as GPUI edges.
    pub fn snap_edges(&self) -> Edges<Pixels> {
        self.snap_margin.into()
    }

    /// Returns the original safe bounds, when provided.
    pub const fn safe_bounds(&self) -> Option<Rect> {
        self.resolution.safe_bounds()
    }

    /// Returns the renderer-neutral placement resolution.
    pub const fn resolution(&self) -> &OverlayPlacementResolution {
        &self.resolution
    }

    /// Returns the selected fit category.
    pub const fn fit(&self) -> OverlayPlacementFit {
        self.resolution.fit()
    }

    /// Returns the diagnostic placement trace.
    pub const fn trace(&self) -> &OverlayPlacementTrace {
        self.resolution.trace()
    }
}

/// Builds a deferred GPUI anchored overlay without forcing a window position.
pub(crate) fn gpui_relative_overlay_layer(
    adapter: &GpuiOverlayState,
    placement: &GpuiOverlayPlacement,
    child: impl IntoElement,
) -> AnyElement {
    deferred(
        anchored()
            .anchor(placement.anchor())
            .offset(placement.offset())
            .snap_to_window_with_margin(placement.snap_edges())
            .child(child),
    )
    .priority(adapter.deferred_priority())
    .into_any_element()
}

/// Builds a deferred GPUI anchored overlay at the resolved window position.
pub(crate) fn gpui_positioned_overlay_layer(
    adapter: &GpuiOverlayState,
    placement: &GpuiOverlayPlacement,
    fallback_position: Point<Pixels>,
    child: impl IntoElement,
) -> AnyElement {
    deferred(
        anchored()
            .position(placement.position().unwrap_or(fallback_position))
            .anchor(placement.anchor())
            .offset(placement.offset())
            .snap_to_window_with_margin(placement.snap_edges())
            .child(child),
    )
    .priority(adapter.deferred_priority())
    .into_any_element()
}

/// Builds a deferred GPUI full-window overlay layer.
pub(crate) fn gpui_full_window_overlay_layer(
    adapter: &GpuiOverlayState,
    child: impl IntoElement,
) -> AnyElement {
    deferred(
        anchored()
            .position(point(px(0.0), px(0.0)))
            .snap_to_window()
            .child(child),
    )
    .priority(adapter.deferred_priority())
    .into_any_element()
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
