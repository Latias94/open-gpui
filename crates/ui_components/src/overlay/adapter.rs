//! GPUI adapter state and layer builders for overlays.

use open_gpui::{
    AnyElement, Edges, IntoElement, ParentElement, Pixels, Point, anchored, deferred, point, px,
};
use open_gpui_ui_core::{
    EscapeKeyPolicy, FocusRestoreIntent, InitialFocusIntent, OutsidePressPolicy, OverlayLayerKind,
    OverlayLayerPolicy, OverlayLayerState, OverlayPresence,
};

use super::OverlayResolvedState;
use super::placement::GpuiOverlayPlacement;

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
