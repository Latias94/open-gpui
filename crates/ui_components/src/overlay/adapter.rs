//! GPUI adapter state and layer builders for overlays.

use open_gpui::{
    AnyElement, Edges, IntoElement, ParentElement, Pixels, Point, SubtreePresentation, anchored,
    point, portal_anchor_follower, px, window_portal,
};
use open_gpui_ui_core::{
    EscapeKeyPolicy, FocusRestoreIntent, InitialFocusIntent, OutsidePressPolicy,
    OverlayAnchorInput, OverlayLayerKind, OverlayLayerPolicy, OverlayLayerState,
    OverlayPlacementInput, OverlayPresence,
};

use crate::geometry::ui_rect_from_gpui_bounds;
use crate::theme::{ThemeContext, ThemeScope};

use super::placement::GpuiOverlayPlacement;
use super::{OverlayLayerBinding, OverlayResolvedState, WindowOverlayRuntime};

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

fn themed_window_portal_overlay_layer(
    adapter: &GpuiOverlayState,
    binding: &OverlayLayerBinding,
    build_layer: impl FnOnce(&ThemeContext) -> AnyElement,
) -> AnyElement {
    let opening_theme = binding
        .opening_theme()
        .expect("an open component overlay must capture its opening theme");
    window_portal(ThemeScope::captured(
        format!("overlay-theme:{}", binding.lease().layer_id().as_str()),
        opening_theme.clone(),
        build_layer(&opening_theme),
    ))
    .priority(adapter.deferred_priority())
    .into_any_element()
}

/// Builds a window-space overlay that follows one committed portal anchor.
pub(crate) fn gpui_portal_anchor_overlay_layer(
    adapter: &GpuiOverlayState,
    runtime: &WindowOverlayRuntime,
    binding: &OverlayLayerBinding,
    window: &open_gpui::Window,
    cx: &open_gpui::App,
    build_placement: impl FnOnce(OverlayAnchorInput) -> OverlayPlacementInput + 'static,
    build_child: impl FnOnce(&ThemeContext, &mut open_gpui::Window, &mut open_gpui::App) -> AnyElement
    + 'static,
) -> AnyElement {
    let handle = binding
        .portal_anchor()
        .expect("a portal-anchored overlay must own an anchor handle");
    let publication = binding
        .portal_anchor_publication()
        .expect("a portal-anchored overlay must own a publication identity");
    let expected_generation = runtime
        .portal_anchor_generation(binding, window, cx)
        .expect("a portal-anchored overlay binding must remain current");
    let opening_theme = binding.opening_theme();
    let priority = adapter.deferred_priority();
    let snap_margin = adapter.snap_margin();
    let runtime = runtime.clone();
    let binding = binding.clone();

    portal_anchor_follower(&handle, move |snapshot, window, cx| {
        let eligible =
            snapshot.filter(|snapshot| snapshot.presentation() == SubtreePresentation::Visible);
        record_portal_anchor_eligibility(
            publication,
            eligible.is_some(),
            expected_generation,
            runtime.clone(),
            binding.clone(),
            window,
        );

        let snapshot = eligible?;
        let opening_theme = opening_theme?;
        let geometry = snapshot.geometry();
        let anchor = OverlayAnchorInput::from_visual_and_layout_bounds(
            Some(ui_rect_from_gpui_bounds(geometry.displayed_bounds())),
            Some(ui_rect_from_gpui_bounds(geometry.layout_bounds())),
        );
        let placement = GpuiOverlayPlacement::resolve(build_placement(anchor), snap_margin);
        let mut layer = anchored()
            .anchor(placement.anchor())
            .offset(placement.offset())
            .snap_to_window_with_margin(placement.snap_edges());
        if let Some(position) = placement.position() {
            layer = layer.position(position);
        }
        Some(
            ThemeScope::captured(
                format!("overlay-theme:{}", binding.lease().layer_id().as_str()),
                opening_theme.clone(),
                layer.child(build_child(&opening_theme, window, cx)),
            )
            .into_any_element(),
        )
    })
    .priority(priority)
    .into_any_element()
}

/// Builds a deferred GPUI anchored overlay at the resolved window position.
pub(crate) fn gpui_positioned_overlay_layer(
    adapter: &GpuiOverlayState,
    placement: &GpuiOverlayPlacement,
    fallback_position: Point<Pixels>,
    binding: &OverlayLayerBinding,
    build_child: impl FnOnce(&ThemeContext) -> AnyElement,
) -> AnyElement {
    themed_window_portal_overlay_layer(adapter, binding, |opening_theme| {
        anchored()
            .position(placement.position().unwrap_or(fallback_position))
            .anchor(placement.anchor())
            .offset(placement.offset())
            .snap_to_window_with_margin(placement.snap_edges())
            .child(build_child(opening_theme))
            .into_any_element()
    })
}

/// Builds a deferred GPUI full-window overlay layer.
pub(crate) fn gpui_full_window_overlay_layer(
    adapter: &GpuiOverlayState,
    binding: &OverlayLayerBinding,
    build_child: impl FnOnce(&ThemeContext) -> AnyElement,
) -> AnyElement {
    themed_window_portal_overlay_layer(adapter, binding, |opening_theme| {
        anchored()
            .position(point(px(0.0), px(0.0)))
            .snap_to_window()
            .child(build_child(opening_theme))
            .into_any_element()
    })
}

fn record_portal_anchor_eligibility(
    publication: open_gpui::PrepaintPublicationId,
    linked: bool,
    expected_generation: super::OverlayLayerGeneration,
    runtime: WindowOverlayRuntime,
    binding: OverlayLayerBinding,
    window: &mut open_gpui::Window,
) {
    let commit_runtime = runtime.clone();
    let commit_binding = binding.clone();
    window.record_prepaint_window_transaction(
        publication,
        move |_, window, cx| {
            if linked {
                let _ = commit_runtime.mark_portal_anchor_linked(
                    &commit_binding,
                    expected_generation,
                    window,
                    cx,
                );
            } else {
                let _ = commit_runtime.mark_portal_anchor_unlinked(
                    &commit_binding,
                    expected_generation,
                    window,
                    cx,
                );
            }
        },
        move |_, window, cx| {
            let _ = runtime.mark_portal_anchor_unlinked(&binding, expected_generation, window, cx);
        },
    );
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
