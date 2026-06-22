use crate::viewport_registry::DockViewportRouteUnavailableReason;
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

/// A registered platform viewport window that contains the pointer.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct DockViewportWindowHit {
    /// Logical dock space bound to the platform window.
    space: DockSpaceId,
    /// GPUI window currently rendering the logical dock space.
    window: AnyWindowHandle,
    /// Point relative to the dock host bounds, when the pointer is inside the dock host.
    host_position: Option<Point<Pixels>>,
    /// Live window-facts generation used to derive this hit.
    facts_generation: Option<u64>,
    /// Why this window contains the pointer but cannot currently authorize a host route.
    route_unavailable_reason: Option<DockViewportRouteUnavailableReason>,
}

impl DockViewportWindowHit {
    pub(crate) fn with_facts_generation(
        space: impl Into<DockSpaceId>,
        window: AnyWindowHandle,
        host_position: Option<Point<Pixels>>,
        facts_generation: u64,
    ) -> Self {
        Self {
            space: space.into(),
            window,
            host_position,
            facts_generation: Some(facts_generation),
            route_unavailable_reason: None,
        }
    }

    pub(crate) fn blocking(
        space: impl Into<DockSpaceId>,
        window: AnyWindowHandle,
        route_unavailable_reason: DockViewportRouteUnavailableReason,
    ) -> Self {
        Self {
            space: space.into(),
            window,
            host_position: None,
            facts_generation: None,
            route_unavailable_reason: Some(route_unavailable_reason),
        }
    }

    #[cfg(test)]
    pub(crate) fn space(&self) -> &DockSpaceId {
        &self.space
    }

    pub(crate) fn window_id(&self) -> WindowId {
        self.window.window_id()
    }

    pub(crate) fn blocks_host_target(&self) -> bool {
        self.route_unavailable_reason.is_some()
            || self.host_position.is_none()
            || self.facts_generation.is_none()
    }

    pub(crate) fn target_hit(&self) -> Option<DockViewportTargetHit> {
        if self.route_unavailable_reason.is_some() {
            return None;
        }
        Some(DockViewportTargetHit::with_facts_generation(
            self.space.clone(),
            self.window,
            self.host_position?,
            self.facts_generation?,
        ))
    }

    pub(crate) fn into_target_hit(self) -> Option<DockViewportTargetHit> {
        if self.route_unavailable_reason.is_some() {
            return None;
        }
        Some(DockViewportTargetHit::with_facts_generation(
            self.space,
            self.window,
            self.host_position?,
            self.facts_generation?,
        ))
    }
}

impl From<DockViewportTargetHit> for DockViewportWindowHit {
    fn from(target: DockViewportTargetHit) -> Self {
        Self::with_facts_generation(
            target.space,
            target.window,
            Some(target.host_position),
            target.facts_generation,
        )
    }
}

/// Route authority that is allowed to construct a commit-capable viewport route.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DockViewportAuthorizedRouteAuthority {
    /// Current backend hovered-window signal authorizes this viewport.
    TrustedHoveredWindow,
    /// The GPUI drag/drop event was delivered by this same registered viewport window and the host
    /// supplied explicit local drop-scene authority. This is reserved for in-window overlays whose
    /// target was accepted by the rendered host scene; it is not a generic event-receiver fallback.
    EventReceiverLocalScene,
    /// Backend hovered-window signal is unavailable or was discarded for a no-input viewport, and
    /// the platform front-to-back window stack selected this viewport.
    PlatformWindowStackFallback,
    /// A previously rendered routed preview accepted this target, so release may replay that
    /// accepted target without asking backend-hover fallback to pick a new viewport.
    AcceptedRoutedPreview,
}

/// A viewport target that is allowed to authorize a commit route.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct DockAuthorizedViewportRouteTarget {
    target: DockViewportWindowHit,
    authority: DockViewportAuthorizedRouteAuthority,
}

impl DockAuthorizedViewportRouteTarget {
    fn trusted_hovered(target: DockViewportWindowHit) -> Self {
        Self {
            target,
            authority: DockViewportAuthorizedRouteAuthority::TrustedHoveredWindow,
        }
    }

    fn platform_window_stack_fallback(target: DockViewportWindowHit) -> Self {
        Self {
            target,
            authority: DockViewportAuthorizedRouteAuthority::PlatformWindowStackFallback,
        }
    }

    pub(crate) fn event_receiver_local_scene(target: DockViewportTargetHit) -> Self {
        Self {
            target: target.into(),
            authority: DockViewportAuthorizedRouteAuthority::EventReceiverLocalScene,
        }
    }

    pub(crate) fn authority(&self) -> DockViewportAuthorizedRouteAuthority {
        self.authority
    }

    pub(crate) fn into_target(self) -> DockViewportWindowHit {
        self.target
    }
}

impl DockViewportAuthorizedRouteAuthority {
    pub(crate) fn records_routed_viewport_identity(self) -> bool {
        matches!(
            self,
            Self::TrustedHoveredWindow
                | Self::EventReceiverLocalScene
                | Self::PlatformWindowStackFallback
        )
    }
}

#[cfg(test)]
pub(crate) fn choose_diagnostic_viewport_target(
    hits: Vec<DockViewportTargetHit>,
    context: &DockViewportTargetContext,
) -> Option<DockViewportTargetHit> {
    let window_hits = hits
        .into_iter()
        .map(DockViewportWindowHit::from)
        .collect::<Vec<_>>();
    choose_trusted_hovered_viewport_target(&window_hits, context)
        .or_else(|| {
            choose_backend_hover_fallback_viewport_target(&window_hits, context)
                .map(DockAuthorizedViewportRouteTarget::into_target)
        })
        .and_then(DockViewportWindowHit::into_target_hit)
        .or_else(|| {
            window_hits
                .into_iter()
                .find_map(DockViewportWindowHit::into_target_hit)
        })
}

fn choose_trusted_hovered_viewport_target(
    hits: &[DockViewportWindowHit],
    context: &DockViewportTargetContext,
) -> Option<DockViewportWindowHit> {
    let hovered_window = context.trusted_hovered_window()?;
    hits.iter()
        .find(|hit| hit.window_id() == hovered_window)
        .cloned()
}

fn choose_backend_hover_fallback_viewport_target(
    hits: &[DockViewportWindowHit],
    context: &DockViewportTargetContext,
) -> Option<DockAuthorizedViewportRouteTarget> {
    let stacked = context
        .backend_hover_fallback_window_stack()
        .iter()
        .find_map(|window_id| {
            hits.iter()
                .find(|hit| hit.window_id() == *window_id)
                .cloned()
        });
    if let Some(target) = stacked {
        return Some(DockAuthorizedViewportRouteTarget::platform_window_stack_fallback(target));
    }

    None
}

pub(crate) fn resolve_authorized_viewport_route_target<I, H>(
    hits: I,
    context: &DockViewportTargetContext,
) -> Option<DockAuthorizedViewportRouteTarget>
where
    I: IntoIterator<Item = H>,
    H: Into<DockViewportWindowHit>,
{
    let hits = hits.into_iter().map(Into::into).collect::<Vec<_>>();
    if let Some(target) = choose_trusted_hovered_viewport_target(&hits, context) {
        return Some(DockAuthorizedViewportRouteTarget::trusted_hovered(target));
    }

    match context.trusted_hovered_signal() {
        crate::DockViewportTrustedHoveredSignal::Unavailable => {
            if let Some(target) = choose_backend_hover_fallback_viewport_target(&hits, context) {
                return Some(target);
            }
        }
        crate::DockViewportTrustedHoveredSignal::TrustedNone
        | crate::DockViewportTrustedHoveredSignal::Trusted(_) => {}
    }

    None
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
                    .with_trusted_hovered_window(first)
                    .with_window_stack([second, first]),
            )
            .map(|hit| hit.space().clone()),
            Some(space("alpha"))
        );
    }

    #[test]
    fn viewport_hover_arbitration_distinguishes_trusted_and_fallback_hits() {
        let first = handle(1);
        let second = handle(2);
        let hits = || vec![candidate("alpha", first), candidate("zeta", second)];

        let ambiguous =
            choose_diagnostic_viewport_target(hits(), &DockViewportTargetContext::new())
                .expect("overlapping candidates should still expose a diagnostic target");
        assert_eq!(ambiguous.space(), &space("alpha"));
        assert_eq!(
            resolve_authorized_viewport_route_target(hits(), &DockViewportTargetContext::new()),
            None,
            "ambiguous geometry is diagnostic-only"
        );

        let hovered = choose_diagnostic_viewport_target(
            hits(),
            &DockViewportTargetContext::new().with_trusted_hovered_window(second),
        )
        .expect("hovered window should resolve a target");
        assert_eq!(hovered.space(), &space("zeta"));
        let authorized = resolve_authorized_viewport_route_target(
            hits(),
            &DockViewportTargetContext::new().with_trusted_hovered_window(second),
        )
        .expect("trusted hovered window should authorize a route target");
        assert_eq!(
            authorized.authority(),
            DockViewportAuthorizedRouteAuthority::TrustedHoveredWindow
        );
        assert_eq!(authorized.into_target().space(), &space("zeta"));

        let stacked = choose_diagnostic_viewport_target(
            hits(),
            &DockViewportTargetContext::new().with_window_stack([second, first]),
        )
        .expect("window stack should produce a fallback candidate");
        assert_eq!(stacked.space(), &space("zeta"));
        assert_eq!(
            resolve_authorized_viewport_route_target(
                hits(),
                &DockViewportTargetContext::new().with_window_stack([second, first]),
            )
            .map(|target| (target.authority(), target.into_target().space().clone())),
            Some((
                DockViewportAuthorizedRouteAuthority::PlatformWindowStackFallback,
                space("zeta"),
            )),
            "backend hover fallback authorizes commits only when hovered-window authority is unavailable"
        );

        let fallback = choose_diagnostic_viewport_target(
            hits(),
            &DockViewportTargetContext::new().with_window_stack([second, first]),
        )
        .expect("window stack should produce an ImGui fallback candidate");
        assert_eq!(fallback.space(), &space("zeta"));
        assert_eq!(
            resolve_authorized_viewport_route_target(
                hits(),
                &DockViewportTargetContext::new().with_window_stack([second, first]),
            )
            .map(|target| (target.authority(), target.into_target().space().clone())),
            Some((
                DockViewportAuthorizedRouteAuthority::PlatformWindowStackFallback,
                space("zeta"),
            ))
        );

        assert_eq!(
            resolve_authorized_viewport_route_target(
                hits(),
                &DockViewportTargetContext::new()
                    .with_window_stack([first, second])
                    .with_window_stack([second, first]),
            )
            .map(|target| target.into_target().space().clone()),
            Some(space("zeta")),
            "platform window stack ordering remains the fallback authority"
        );

        let hovered_known_empty = choose_diagnostic_viewport_target(
            hits(),
            &DockViewportTargetContext::new()
                .with_trusted_hovered_window_known_empty()
                .with_window_stack([second, first]),
        )
        .expect("window stack should still produce a diagnostic candidate");
        assert_eq!(hovered_known_empty.space(), &space("zeta"));
        assert_eq!(
            resolve_authorized_viewport_route_target(
                hits(),
                &DockViewportTargetContext::new()
                    .with_trusted_hovered_window_known_empty()
                    .with_window_stack([second, first]),
            ),
            None
        );

        let single = choose_diagnostic_viewport_target(
            vec![candidate("alpha", first)],
            &DockViewportTargetContext::new(),
        )
        .expect("single hit should resolve");
        assert_eq!(single.space(), &space("alpha"));
        assert_eq!(
            resolve_authorized_viewport_route_target(
                vec![candidate("alpha", first)],
                &DockViewportTargetContext::new(),
            ),
            None,
            "a single geometry hit remains diagnostic-only without backend hover, stack, or focus-order authority"
        );

        let single_hovered_known_empty = choose_diagnostic_viewport_target(
            vec![candidate("alpha", first)],
            &DockViewportTargetContext::new()
                .with_trusted_hovered_window_known_empty()
                .with_window_stack([first]),
        )
        .expect("single hit should still be reported for diagnostics");
        assert_eq!(single_hovered_known_empty.space(), &space("alpha"));
        assert_eq!(
            resolve_authorized_viewport_route_target(
                vec![candidate("alpha", first)],
                &DockViewportTargetContext::new()
                    .with_trusted_hovered_window_known_empty()
                    .with_window_stack([first]),
            )
            .map(|target| (target.authority(), target.into_target().space().clone())),
            None
        );

        let single_mismatched_window_stack = choose_diagnostic_viewport_target(
            vec![candidate("alpha", first)],
            &DockViewportTargetContext::new().with_window_stack([second]),
        )
        .expect("single hit should still be reported for diagnostics");
        assert_eq!(single_mismatched_window_stack.space(), &space("alpha"));
        assert_eq!(
            resolve_authorized_viewport_route_target(
                vec![candidate("alpha", first)],
                &DockViewportTargetContext::new().with_window_stack([second]),
            )
            .map(|target| (target.authority(), target.into_target().space().clone())),
            None,
            "a stack entry that does not match the live hit must not authorize a route"
        );
    }

    #[test]
    fn authorized_route_target_requires_backend_authority() {
        let first = handle(1);
        let second = handle(2);
        let hits = || vec![candidate("alpha", first), candidate("zeta", second)];

        assert_eq!(
            resolve_authorized_viewport_route_target(hits(), &DockViewportTargetContext::new()),
            None
        );
        assert_eq!(
            resolve_authorized_viewport_route_target(
                hits(),
                &DockViewportTargetContext::new().with_window_stack([second, first]),
            )
            .map(|target| (target.authority(), target.into_target().space().clone())),
            Some((
                DockViewportAuthorizedRouteAuthority::PlatformWindowStackFallback,
                space("zeta"),
            )),
            "window stack fallback is commit authority when trusted hovered-window data is unavailable"
        );
        assert_eq!(
            resolve_authorized_viewport_route_target(
                hits(),
                &DockViewportTargetContext::new().with_window_stack([second, first]),
            )
            .map(|target| (target.authority(), target.into_target().space().clone())),
            Some((
                DockViewportAuthorizedRouteAuthority::PlatformWindowStackFallback,
                space("zeta"),
            )),
            "window stack fallback is commit authority in the backend-hover-unavailable path"
        );
        assert_eq!(
            resolve_authorized_viewport_route_target(
                hits(),
                &DockViewportTargetContext::new().with_trusted_hovered_window_known_empty(),
            ),
            None,
            "trusted hovered=None must not authorize any app viewport"
        );
        let authorized = resolve_authorized_viewport_route_target(
            hits(),
            &DockViewportTargetContext::new().with_trusted_hovered_window(second),
        )
        .expect("trusted hovered should authorize the matching live hit");
        assert_eq!(authorized.into_target().space(), &space("zeta"));
    }
}
