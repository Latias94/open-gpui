use crate::{DockSpaceId, DockViewportTargetContext};
use open_gpui::{AnyWindowHandle, Pixels, Point, WindowId};

/// Result of resolving a screen point into a registered dock viewport.
#[cfg(test)]
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct DockViewportHit {
    /// Logical dock space that contains the point.
    space: DockSpaceId,
    /// Point relative to the dock host bounds.
    host_position: Point<Pixels>,
}

#[cfg(test)]
impl DockViewportHit {
    pub(crate) fn new(space: impl Into<DockSpaceId>, host_position: Point<Pixels>) -> Self {
        Self {
            space: space.into(),
            host_position,
        }
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
    /// Live window-facts generation used to derive `host_position`.
    facts_generation: u64,
}

impl DockViewportTargetHit {
    #[cfg(test)]
    pub(crate) fn new(
        space: impl Into<DockSpaceId>,
        window: AnyWindowHandle,
        host_position: Point<Pixels>,
    ) -> Self {
        Self::with_facts_generation(space, window, host_position, 0)
    }

    pub(crate) fn with_facts_generation(
        space: impl Into<DockSpaceId>,
        window: AnyWindowHandle,
        host_position: Point<Pixels>,
        facts_generation: u64,
    ) -> Self {
        Self {
            space: space.into(),
            window,
            host_position,
            facts_generation,
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

    pub(crate) fn facts_generation(&self) -> u64 {
        self.facts_generation
    }

    #[cfg(test)]
    pub(crate) fn into_hit(self) -> DockViewportHit {
        DockViewportHit::new(self.space, self.host_position)
    }
}

/// Confidence assigned to a resolved viewport target.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DockViewportTargetConfidence {
    /// Backend/platform hovered-window authority matched the target.
    TrustedHovered,
    /// The event receiver owns the only live hit and no stronger signal conflicts.
    TrustedEventReceiver,
    /// A single live rectangle hit exists and no stronger signal conflicts.
    TrustedSingleHit,
    /// A platform window-stack/focus fallback selected a diagnostic target.
    WindowStackFallback,
    /// Multiple live hits overlap and no platform signal can arbitrate them.
    Ambiguous,
    /// Platform signals existed but none matched the live hits; the result is only stable fallback.
    FallbackOnly,
}

/// A viewport target plus whether it may be used as commit authority.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct DockViewportTargetResolution {
    target: DockViewportTargetHit,
    confidence: DockViewportTargetConfidence,
}

/// Explicit hover arbitration outcome for live viewport hits.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum DockViewportHoverArbitration {
    /// A route-authoritative viewport target was selected.
    TrustedHovered(DockViewportTargetHit),
    /// The event receiver owns the only live hit and can commit locally.
    TrustedEventReceiver(DockViewportTargetHit),
    /// A single live rectangle hit can commit because no stronger signal conflicts.
    TrustedSingleHit(DockViewportTargetHit),
    /// Window-stack/focus fallback selected a diagnostic target, but it is not commit authority.
    WindowStackFallback(DockViewportTargetHit),
    /// Multiple live hits overlap and no trusted signal can pick one.
    Ambiguous(DockViewportTargetHit),
    /// A deterministic diagnostic target exists, but current signals do not authorize it.
    FallbackOnly(DockViewportTargetHit),
    /// No live viewport contains the point.
    Unavailable,
}

/// Arbiter that resolves live viewport hits using trusted platform hover/topmost signals.
#[derive(Debug, Clone, Copy)]
pub(crate) struct DockViewportHoverArbiter<'a> {
    context: &'a DockViewportTargetContext,
}

impl DockViewportTargetResolution {
    fn new(target: DockViewportTargetHit, confidence: DockViewportTargetConfidence) -> Self {
        Self { target, confidence }
    }

    #[cfg(test)]
    pub(crate) fn target(&self) -> &DockViewportTargetHit {
        &self.target
    }

    pub(crate) fn into_target(self) -> DockViewportTargetHit {
        self.target
    }

    #[cfg(test)]
    pub(crate) fn confidence(&self) -> DockViewportTargetConfidence {
        self.confidence
    }

    pub(crate) fn is_trusted(&self) -> bool {
        matches!(
            self.confidence,
            DockViewportTargetConfidence::TrustedHovered
                | DockViewportTargetConfidence::TrustedEventReceiver
                | DockViewportTargetConfidence::TrustedSingleHit
        )
    }
}

impl DockViewportHoverArbitration {
    fn into_resolution(self) -> Option<DockViewportTargetResolution> {
        match self {
            Self::TrustedHovered(target) => Some(DockViewportTargetResolution::new(
                target,
                DockViewportTargetConfidence::TrustedHovered,
            )),
            Self::TrustedEventReceiver(target) => Some(DockViewportTargetResolution::new(
                target,
                DockViewportTargetConfidence::TrustedEventReceiver,
            )),
            Self::TrustedSingleHit(target) => Some(DockViewportTargetResolution::new(
                target,
                DockViewportTargetConfidence::TrustedSingleHit,
            )),
            Self::WindowStackFallback(target) => Some(DockViewportTargetResolution::new(
                target,
                DockViewportTargetConfidence::WindowStackFallback,
            )),
            Self::Ambiguous(target) => Some(DockViewportTargetResolution::new(
                target,
                DockViewportTargetConfidence::Ambiguous,
            )),
            Self::FallbackOnly(target) => Some(DockViewportTargetResolution::new(
                target,
                DockViewportTargetConfidence::FallbackOnly,
            )),
            Self::Unavailable => None,
        }
    }
}

impl<'a> DockViewportHoverArbiter<'a> {
    pub(crate) fn new(context: &'a DockViewportTargetContext) -> Self {
        Self { context }
    }

    pub(crate) fn resolve(self, hits: Vec<DockViewportTargetHit>) -> DockViewportHoverArbitration {
        let hit_count = hits.len();
        let Some(target) = choose_diagnostic_viewport_target(hits, self.context) else {
            return DockViewportHoverArbitration::Unavailable;
        };

        if self.context.hovered_window() == Some(target.window_id()) {
            return DockViewportHoverArbitration::TrustedHovered(target);
        }

        if hit_count == 1 {
            if self
                .context
                .event_receiver_window_matches(target.window_id())
                && !self
                    .context
                    .has_conflicting_hovered_window(target.window_id())
            {
                return DockViewportHoverArbitration::TrustedEventReceiver(target);
            }

            if self.context.hovered_window_known_empty() {
                return DockViewportHoverArbitration::FallbackOnly(target);
            }

            if !self
                .context
                .has_conflicting_single_hit_signal(target.window_id())
            {
                return DockViewportHoverArbitration::TrustedSingleHit(target);
            }
        }

        if self.context.hovered_window_known_empty() {
            return DockViewportHoverArbitration::FallbackOnly(target);
        }

        if self.context.window_stack_contains(target.window_id()) {
            return DockViewportHoverArbitration::WindowStackFallback(target);
        }

        if self.context.has_arbitration_signal() {
            DockViewportHoverArbitration::FallbackOnly(target)
        } else {
            DockViewportHoverArbitration::Ambiguous(target)
        }
    }
}

pub(crate) fn choose_diagnostic_viewport_target(
    hits: Vec<DockViewportTargetHit>,
    context: &DockViewportTargetContext,
) -> Option<DockViewportTargetHit> {
    hits.into_iter()
        .enumerate()
        .min_by_key(|(index, hit)| context.priority_for_window(hit.window_id(), *index))
        .map(|(_, hit)| hit)
}

pub(crate) fn resolve_viewport_target_with_confidence(
    hits: Vec<DockViewportTargetHit>,
    context: &DockViewportTargetContext,
) -> Option<DockViewportTargetResolution> {
    DockViewportHoverArbiter::new(context)
        .resolve(hits)
        .into_resolution()
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
    fn diagnostic_viewport_target_prefers_hovered_then_window_stack() {
        let first = handle(1);
        let second = handle(2);
        let hits = || vec![candidate("alpha", first), candidate("zeta", second)];

        assert_eq!(
            choose_diagnostic_viewport_target(hits(), &DockViewportTargetContext::new())
                .map(|hit| hit.space().clone()),
            Some(space("alpha")),
            "default fallback should preserve deterministic candidate order"
        );
        assert_eq!(
            choose_diagnostic_viewport_target(
                hits(),
                &DockViewportTargetContext::new().with_window_stack([second, first]),
            )
            .map(|hit| hit.space().clone()),
            Some(space("zeta"))
        );
        assert_eq!(
            choose_diagnostic_viewport_target(
                hits(),
                &DockViewportTargetContext::new()
                    .with_hovered_window(first)
                    .with_window_stack([second, first]),
            )
            .map(|hit| hit.space().clone()),
            Some(space("alpha"))
        );
    }

    #[test]
    fn viewport_hover_arbitration_distinguishes_trusted_ambiguous_and_fallback_hits() {
        let first = handle(1);
        let second = handle(2);
        let hits = || vec![candidate("alpha", first), candidate("zeta", second)];

        let ambiguous =
            resolve_viewport_target_with_confidence(hits(), &DockViewportTargetContext::new())
                .expect("overlapping candidates should still resolve a fallback target");
        assert_eq!(ambiguous.target().space(), &space("alpha"));
        assert_eq!(
            ambiguous.confidence(),
            DockViewportTargetConfidence::Ambiguous
        );
        assert!(!ambiguous.is_trusted());

        let hovered = resolve_viewport_target_with_confidence(
            hits(),
            &DockViewportTargetContext::new().with_hovered_window(second),
        )
        .expect("hovered window should resolve a target");
        assert_eq!(hovered.target().space(), &space("zeta"));
        assert_eq!(
            hovered.confidence(),
            DockViewportTargetConfidence::TrustedHovered
        );
        assert!(hovered.is_trusted());

        let stacked = resolve_viewport_target_with_confidence(
            hits(),
            &DockViewportTargetContext::new().with_window_stack([second, first]),
        )
        .expect("window stack should still produce a diagnostic candidate");
        assert_eq!(stacked.target().space(), &space("zeta"));
        assert_eq!(
            stacked.confidence(),
            DockViewportTargetConfidence::WindowStackFallback
        );
        assert!(!stacked.is_trusted());

        let hovered_known_empty = resolve_viewport_target_with_confidence(
            hits(),
            &DockViewportTargetContext::new()
                .with_hovered_window_known_empty()
                .with_window_stack([second, first]),
        )
        .expect("window stack should still produce a diagnostic candidate");
        assert_eq!(hovered_known_empty.target().space(), &space("zeta"));
        assert_eq!(
            hovered_known_empty.confidence(),
            DockViewportTargetConfidence::FallbackOnly,
            "window stack must not authorize a route when the platform says no app window is hovered"
        );
        assert!(!hovered_known_empty.is_trusted());

        let single = resolve_viewport_target_with_confidence(
            vec![candidate("alpha", first)],
            &DockViewportTargetContext::new(),
        )
        .expect("single hit should resolve");
        assert_eq!(
            single.confidence(),
            DockViewportTargetConfidence::TrustedSingleHit
        );

        let single_hovered_known_empty = resolve_viewport_target_with_confidence(
            vec![candidate("alpha", first)],
            &DockViewportTargetContext::new()
                .with_hovered_window_known_empty()
                .with_window_stack([first]),
        )
        .expect("single hit should still be reported for diagnostics");
        assert_eq!(
            single_hovered_known_empty.confidence(),
            DockViewportTargetConfidence::FallbackOnly,
            "trusted hovered=None must block even a single matching app-window hit"
        );
        assert!(!single_hovered_known_empty.is_trusted());

        let single_unmatched_signal = resolve_viewport_target_with_confidence(
            vec![candidate("alpha", first)],
            &DockViewportTargetContext::new().with_window_stack([second]),
        )
        .expect("single hit should still be reported for diagnostics");
        assert_eq!(
            single_unmatched_signal.confidence(),
            DockViewportTargetConfidence::FallbackOnly,
            "a single rectangle hit should be diagnostic-only when trusted arbitration points elsewhere"
        );
        assert!(!single_unmatched_signal.is_trusted());
    }

    #[test]
    fn event_receiver_window_does_not_authorize_overlap_commit() {
        let first = handle(1);
        let second = handle(2);
        let hits = || vec![candidate("alpha", first), candidate("zeta", second)];

        let resolved = resolve_viewport_target_with_confidence(
            hits(),
            &DockViewportTargetContext::new().with_event_receiver_window(second),
        )
        .expect("event receiver window should still produce a diagnostic candidate");

        assert_eq!(resolved.target().space(), &space("alpha"));
        assert_eq!(
            resolved.confidence(),
            DockViewportTargetConfidence::FallbackOnly
        );
        assert!(!resolved.is_trusted());
    }

    #[test]
    fn event_receiver_window_authorizes_single_hit_even_when_hovered_window_is_known_empty() {
        let window = handle(1);

        let resolved = resolve_viewport_target_with_confidence(
            vec![candidate("alpha", window)],
            &DockViewportTargetContext::new()
                .with_hovered_window_known_empty()
                .with_event_receiver_window(window),
        )
        .expect("event receiver should resolve its single hit");

        assert_eq!(
            resolved.confidence(),
            DockViewportTargetConfidence::TrustedEventReceiver
        );
        assert!(resolved.is_trusted());
    }

    #[test]
    fn event_receiver_window_does_not_override_conflicting_hovered_window() {
        let first = handle(1);
        let second = handle(2);

        let resolved = resolve_viewport_target_with_confidence(
            vec![candidate("alpha", first)],
            &DockViewportTargetContext::new()
                .with_event_receiver_window(first)
                .with_hovered_window(second),
        )
        .expect("event receiver should still resolve diagnostics");

        assert_eq!(
            resolved.confidence(),
            DockViewportTargetConfidence::FallbackOnly
        );
        assert!(!resolved.is_trusted());
    }
}
