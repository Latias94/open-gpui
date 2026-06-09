use crate::{
    DockItemId, DockNodeId, DockPolicy, DockPolicyError, DockSpaceId, DockViewportAdapter,
    DockViewportHit, DockViewportTargetContext,
};
use open_gpui::{Pixels, Point, WindowBounds};

/// Request to open a new platform viewport for a tab released outside known dock viewports.
#[derive(Debug, Clone, PartialEq)]
pub struct DockViewportTearOffRequest {
    /// Source dock space containing the dragged item.
    pub source_space: DockSpaceId,
    /// Source tabs node where the drag started.
    pub source_tabs: DockNodeId,
    /// Item being torn off.
    pub item: DockItemId,
    /// Release position in screen coordinates.
    pub release_position: Point<Pixels>,
    /// Suggested platform window bounds for the new viewport, when known.
    pub suggested_window_bounds: Option<WindowBounds>,
}

/// Result of resolving a tab release against registered platform viewports.
#[derive(Debug, Clone, PartialEq)]
pub enum DockViewportTearOffOutcome {
    /// The release landed inside a known viewport; normal drop handling should continue.
    KnownViewport(DockViewportHit),
    /// The release can open a new platform viewport.
    Requested(DockViewportTearOffRequest),
    /// The request was rejected by docking policy.
    Rejected(DockPolicyError),
}

impl DockViewportAdapter {
    /// Resolves a tab release into either an existing viewport hit or a platform tear-off request.
    ///
    /// This method never mutates the docking graph. Callers should open/register a destination
    /// viewport first, then commit a move action after runtime setup succeeds.
    pub fn resolve_tear_off_request(
        &self,
        source_space: impl Into<DockSpaceId>,
        source_tabs: DockNodeId,
        item: impl Into<DockItemId>,
        release_position: Point<Pixels>,
        suggested_window_bounds: Option<WindowBounds>,
        policy: &DockPolicy,
    ) -> DockViewportTearOffOutcome {
        self.resolve_tear_off_request_with_context(
            source_space,
            source_tabs,
            item,
            release_position,
            suggested_window_bounds,
            policy,
            &DockViewportTargetContext::new(),
        )
    }

    /// Resolves a tab release using explicit viewport target arbitration inputs.
    #[allow(clippy::too_many_arguments)]
    pub fn resolve_tear_off_request_with_context(
        &self,
        source_space: impl Into<DockSpaceId>,
        source_tabs: DockNodeId,
        item: impl Into<DockItemId>,
        release_position: Point<Pixels>,
        suggested_window_bounds: Option<WindowBounds>,
        policy: &DockPolicy,
        target_context: &DockViewportTargetContext,
    ) -> DockViewportTearOffOutcome {
        if let Some(hit) = self.hit_test_screen_with_context(release_position, target_context) {
            return DockViewportTearOffOutcome::KnownViewport(hit);
        }

        if let Err(reason) = policy.validate_platform_viewports() {
            return DockViewportTearOffOutcome::Rejected(reason);
        }

        DockViewportTearOffOutcome::Requested(DockViewportTearOffRequest {
            source_space: source_space.into(),
            source_tabs,
            item: item.into(),
            release_position,
            suggested_window_bounds,
        })
    }
}
