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

/// Route authority that is allowed to construct a commit-capable viewport route.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DockViewportAuthorizedRouteAuthority {
    /// Current backend hovered-window signal authorizes this viewport.
    TrustedHoveredWindow,
    /// Active drag fallback reuses the last viewport identity that was genuinely hovered.
    DragLastHoveredViewport,
    /// Backend hovered-window signal is unavailable or was discarded for a no-input viewport, so
    /// fallback z-order authority authorizes the route the same way ImGui derives
    /// `MouseViewport`: prefer platform window-stack order when available, otherwise rely on
    /// recent platform focus order and only allow a lone remaining hit when no overlap remains.
    BackendHoverFallback,
}

/// A viewport target that is allowed to authorize a commit route.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct DockAuthorizedViewportRouteTarget {
    target: DockViewportTargetHit,
    authority: DockViewportAuthorizedRouteAuthority,
}

impl DockAuthorizedViewportRouteTarget {
    fn trusted_hovered(target: DockViewportTargetHit) -> Self {
        Self {
            target,
            authority: DockViewportAuthorizedRouteAuthority::TrustedHoveredWindow,
        }
    }

    fn backend_hover_fallback(target: DockViewportTargetHit) -> Self {
        Self {
            target,
            authority: DockViewportAuthorizedRouteAuthority::BackendHoverFallback,
        }
    }

    pub(crate) fn authority(&self) -> DockViewportAuthorizedRouteAuthority {
        self.authority
    }

    pub(crate) fn into_target(self) -> DockViewportTargetHit {
        self.target
    }
}

#[cfg(test)]
pub(crate) fn choose_diagnostic_viewport_target(
    hits: Vec<DockViewportTargetHit>,
    context: &DockViewportTargetContext,
) -> Option<DockViewportTargetHit> {
    choose_trusted_hovered_viewport_target(&hits, context)
        .or_else(|| choose_backend_hover_fallback_viewport_target(&hits, context, &[]))
        .or_else(|| hits.into_iter().next())
}

fn choose_trusted_hovered_viewport_target(
    hits: &[DockViewportTargetHit],
    context: &DockViewportTargetContext,
) -> Option<DockViewportTargetHit> {
    let hovered_window = context.trusted_hovered_window()?;
    hits.iter()
        .find(|hit| hit.window_id() == hovered_window)
        .cloned()
}

fn choose_backend_hover_fallback_viewport_target(
    hits: &[DockViewportTargetHit],
    context: &DockViewportTargetContext,
    recent_focus_order: &[WindowId],
) -> Option<DockViewportTargetHit> {
    let stacked = context
        .backend_hover_fallback_window_stack()
        .iter()
        .find_map(|window_id| {
            hits.iter()
                .find(|hit| hit.window_id() == *window_id)
                .cloned()
        });
    if stacked.is_some() {
        return stacked;
    }

    for window_id in recent_focus_order {
        if let Some(hit) = hits.iter().find(|hit| hit.window_id() == *window_id) {
            return Some(hit.clone());
        }
    }

    (hits.len() == 1).then(|| hits[0].clone())
}

pub(crate) fn resolve_authorized_viewport_route_target(
    hits: Vec<DockViewportTargetHit>,
    context: &DockViewportTargetContext,
    recent_focus_order: &[WindowId],
) -> Option<DockAuthorizedViewportRouteTarget> {
    if let Some(target) = choose_trusted_hovered_viewport_target(&hits, context) {
        return Some(DockAuthorizedViewportRouteTarget::trusted_hovered(target));
    }

    if context.trusted_hovered_window_unavailable()
        || context.trusted_hovered_window_authority_discarded()
    {
        if let Some(target) =
            choose_backend_hover_fallback_viewport_target(&hits, context, recent_focus_order)
        {
            return Some(DockAuthorizedViewportRouteTarget::backend_hover_fallback(
                target,
            ));
        }
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
            resolve_authorized_viewport_route_target(
                hits(),
                &DockViewportTargetContext::new(),
                &[],
            ),
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
            &[],
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
                &[],
            )
            .map(|target| (target.authority(), target.into_target().space().clone())),
            Some((
                DockViewportAuthorizedRouteAuthority::BackendHoverFallback,
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
                &[],
            )
            .map(|target| (target.authority(), target.into_target().space().clone())),
            Some((
                DockViewportAuthorizedRouteAuthority::BackendHoverFallback,
                space("zeta"),
            ))
        );

        assert_eq!(
            resolve_authorized_viewport_route_target(
                hits(),
                &DockViewportTargetContext::new()
                    .with_window_stack([first, second])
                    .with_window_stack([second, first]),
                &[],
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
                &[],
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
                &[],
            )
            .map(|target| (target.authority(), target.into_target().space().clone())),
            Some((
                DockViewportAuthorizedRouteAuthority::BackendHoverFallback,
                space("alpha"),
            )),
            "a single remaining hit may still use backend fallback authority when hovered-window data is unavailable"
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
                &[],
            )
            .map(|target| (target.authority(), target.into_target().space().clone())),
            None
        );

        let single_recent_focus_fallback = choose_diagnostic_viewport_target(
            vec![candidate("alpha", first)],
            &DockViewportTargetContext::new().with_window_stack([second]),
        )
        .expect("single hit should still be reported for diagnostics");
        assert_eq!(single_recent_focus_fallback.space(), &space("alpha"));
        assert_eq!(
            resolve_authorized_viewport_route_target(
                vec![candidate("alpha", first)],
                &DockViewportTargetContext::new().with_window_stack([second]),
                &[first.window_id()],
            )
            .map(|target| (target.authority(), target.into_target().space().clone())),
            Some((
                DockViewportAuthorizedRouteAuthority::BackendHoverFallback,
                space("alpha"),
            )),
            "recent platform focus order backs the same fallback path imgui uses when no stack signal identifies the hovered viewport"
        );
    }

    #[test]
    fn authorized_route_target_requires_backend_authority() {
        let first = handle(1);
        let second = handle(2);
        let hits = || vec![candidate("alpha", first), candidate("zeta", second)];

        assert_eq!(
            resolve_authorized_viewport_route_target(
                hits(),
                &DockViewportTargetContext::new(),
                &[],
            ),
            None
        );
        assert_eq!(
            resolve_authorized_viewport_route_target(
                hits(),
                &DockViewportTargetContext::new().with_window_stack([second, first]),
                &[],
            )
            .map(|target| (target.authority(), target.into_target().space().clone())),
            Some((
                DockViewportAuthorizedRouteAuthority::BackendHoverFallback,
                space("zeta"),
            )),
            "window stack fallback is commit authority when trusted hovered-window data is unavailable"
        );
        assert_eq!(
            resolve_authorized_viewport_route_target(
                hits(),
                &DockViewportTargetContext::new().with_window_stack([second, first]),
                &[],
            )
            .map(|target| (target.authority(), target.into_target().space().clone())),
            Some((
                DockViewportAuthorizedRouteAuthority::BackendHoverFallback,
                space("zeta"),
            )),
            "window stack fallback is commit authority in the backend-hover-unavailable path"
        );
        assert_eq!(
            resolve_authorized_viewport_route_target(
                hits(),
                &DockViewportTargetContext::new().with_trusted_hovered_window_known_empty(),
                &[],
            ),
            None,
            "trusted hovered=None must not authorize any app viewport"
        );
        let authorized = resolve_authorized_viewport_route_target(
            hits(),
            &DockViewportTargetContext::new().with_trusted_hovered_window(second),
            &[],
        )
        .expect("trusted hovered should authorize the matching live hit");
        assert_eq!(authorized.into_target().space(), &space("zeta"));
    }
}
