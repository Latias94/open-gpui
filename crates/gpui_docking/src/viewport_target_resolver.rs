use crate::{DockSpaceId, DockViewportTargetContext};
use open_gpui::{AnyWindowHandle, Pixels, Point, WindowId};

/// Result of resolving a screen point into a registered dock viewport.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct DockViewportHit {
    /// Logical dock space that contains the point.
    space: DockSpaceId,
    /// Point relative to the dock host bounds.
    host_position: Point<Pixels>,
}

impl DockViewportHit {
    #[cfg(test)]
    pub(crate) fn new(space: impl Into<DockSpaceId>, host_position: Point<Pixels>) -> Self {
        Self {
            space: space.into(),
            host_position,
        }
    }

    pub(crate) fn host_position(&self) -> Point<Pixels> {
        self.host_position
    }
}

/// A registered viewport hit with the runtime window that owns it.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct DockViewportTargetHit {
    /// Logical dock space that contains the point.
    space: DockSpaceId,
    /// GPUI window currently rendering the logical dock space.
    window: AnyWindowHandle,
    /// Point relative to the dock host bounds.
    host_position: Point<Pixels>,
}

impl DockViewportTargetHit {
    pub(crate) fn new(
        space: impl Into<DockSpaceId>,
        window: AnyWindowHandle,
        host_position: Point<Pixels>,
    ) -> Self {
        Self {
            space: space.into(),
            window,
            host_position,
        }
    }

    pub(crate) fn space(&self) -> &DockSpaceId {
        &self.space
    }

    pub(crate) fn window_id(&self) -> WindowId {
        self.window.window_id()
    }

    pub(crate) fn host_position(&self) -> Point<Pixels> {
        self.host_position
    }

    #[cfg(test)]
    pub(crate) fn into_hit(self) -> DockViewportHit {
        DockViewportHit::new(self.space, self.host_position)
    }
}

pub(crate) fn choose_viewport_target(
    hits: Vec<DockViewportTargetHit>,
    context: &DockViewportTargetContext,
) -> Option<DockViewportTargetHit> {
    hits.into_iter()
        .enumerate()
        .min_by_key(|(index, hit)| {
            let window_id = hit.window_id();
            (
                context
                    .hovered_window()
                    .map(|hovered| usize::from(hovered != window_id))
                    .unwrap_or(1),
                context
                    .active_window()
                    .map(|active| usize::from(active != window_id))
                    .unwrap_or(1),
                context
                    .window_stack()
                    .iter()
                    .position(|stacked| *stacked == window_id)
                    .unwrap_or(usize::MAX),
                *index,
            )
        })
        .map(|(_, hit)| hit)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::viewport_test_support::{handle, space};
    use open_gpui::{point, px};

    fn candidate(space: &str, window: AnyWindowHandle) -> DockViewportTargetHit {
        DockViewportTargetHit::new(self::space(space), window, point(px(5.0), px(5.0)))
    }

    #[test]
    fn viewport_target_prefers_hovered_active_then_window_stack() {
        let first = handle(1);
        let second = handle(2);
        let hits = || vec![candidate("alpha", first), candidate("zeta", second)];

        assert_eq!(
            choose_viewport_target(hits(), &DockViewportTargetContext::new())
                .map(|hit| hit.space().clone()),
            Some(space("alpha")),
            "default fallback should preserve deterministic candidate order"
        );
        assert_eq!(
            choose_viewport_target(
                hits(),
                &DockViewportTargetContext::new().with_active_window(second),
            )
            .map(|hit| hit.space().clone()),
            Some(space("zeta"))
        );
        assert_eq!(
            choose_viewport_target(
                hits(),
                &DockViewportTargetContext::new().with_window_stack([second, first]),
            )
            .map(|hit| hit.space().clone()),
            Some(space("zeta"))
        );
        assert_eq!(
            choose_viewport_target(
                hits(),
                &DockViewportTargetContext::new()
                    .with_hovered_window(first)
                    .with_active_window(second)
                    .with_window_stack([second, first]),
            )
            .map(|hit| hit.space().clone()),
            Some(space("alpha"))
        );
    }
}
