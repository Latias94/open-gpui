use crate::{DockSpaceId, DockViewportTargetContext};
use open_gpui::{AnyWindowHandle, Pixels, Point};

/// Result of resolving a screen point into a registered dock viewport.
#[derive(Debug, Clone, PartialEq)]
pub struct DockViewportHit {
    /// Logical dock space that contains the point.
    pub space: DockSpaceId,
    /// Point relative to the dock host bounds.
    pub host_position: Point<Pixels>,
}

/// A viewport hit with the runtime window that produced it.
#[derive(Debug, Clone, PartialEq)]
pub struct DockViewportHitCandidate {
    /// Logical dock space that contains the point.
    pub space: DockSpaceId,
    /// GPUI window currently rendering the logical dock space.
    pub window: AnyWindowHandle,
    /// Point relative to the dock host bounds.
    pub host_position: Point<Pixels>,
}

impl DockViewportHitCandidate {
    pub(crate) fn into_hit(self) -> DockViewportHit {
        DockViewportHit {
            space: self.space,
            host_position: self.host_position,
        }
    }
}

pub(crate) fn choose_viewport_target(
    hits: Vec<DockViewportHitCandidate>,
    context: &DockViewportTargetContext,
) -> Option<DockViewportHitCandidate> {
    hits.into_iter()
        .enumerate()
        .min_by_key(|(index, hit)| {
            let window_id = hit.window.window_id();
            (
                context
                    .hovered_window
                    .map(|hovered| usize::from(hovered != window_id))
                    .unwrap_or(1),
                context
                    .active_window
                    .map(|active| usize::from(active != window_id))
                    .unwrap_or(1),
                context
                    .window_stack
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

    fn candidate(space: &str, window: AnyWindowHandle) -> DockViewportHitCandidate {
        DockViewportHitCandidate {
            space: self::space(space),
            window,
            host_position: point(px(5.0), px(5.0)),
        }
    }

    #[test]
    fn viewport_target_prefers_hovered_active_then_window_stack() {
        let first = handle(1);
        let second = handle(2);
        let hits = || vec![candidate("alpha", first), candidate("zeta", second)];

        assert_eq!(
            choose_viewport_target(hits(), &DockViewportTargetContext::new()).map(|hit| hit.space),
            Some(space("alpha")),
            "default fallback should preserve deterministic candidate order"
        );
        assert_eq!(
            choose_viewport_target(
                hits(),
                &DockViewportTargetContext::new().with_active_window(second),
            )
            .map(|hit| hit.space),
            Some(space("zeta"))
        );
        assert_eq!(
            choose_viewport_target(
                hits(),
                &DockViewportTargetContext::new().with_window_stack([second, first]),
            )
            .map(|hit| hit.space),
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
            .map(|hit| hit.space),
            Some(space("alpha"))
        );
    }
}
