use crate::viewport_registry::{DockViewportRegistrationKey, DockViewportRouteUnavailableReason};
use crate::{DockSpaceId, DockViewportTargetContext, DockViewportWindowStackSource};
use open_gpui::{AnyWindowHandle, Pixels, Point, WindowId};

/// Immutable proof that route facts belong to one exact viewport registration.
///
/// Facts generations only order coordinate snapshots within a registration. The registration key
/// prevents delayed routes from becoming valid again after the same space and window id are bound
/// to a replacement generation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DockViewportRouteProof {
    registration_key: DockViewportRegistrationKey,
    facts_generation: u64,
}

impl DockViewportRouteProof {
    pub(crate) fn new(
        registration_key: DockViewportRegistrationKey,
        facts_generation: u64,
    ) -> Self {
        Self {
            registration_key,
            facts_generation,
        }
    }

    #[cfg(test)]
    pub(crate) fn for_test_registration_generation(
        space: DockSpaceId,
        window_id: WindowId,
        registration_generation: u64,
        facts_generation: u64,
    ) -> Self {
        Self::new(
            DockViewportRegistrationKey::for_test_generation(
                space,
                window_id,
                registration_generation,
            ),
            facts_generation,
        )
    }

    pub(crate) fn registration_key(&self) -> &DockViewportRegistrationKey {
        &self.registration_key
    }

    pub(crate) fn space(&self) -> &DockSpaceId {
        self.registration_key.space()
    }

    pub(crate) fn window_id(&self) -> WindowId {
        self.registration_key.window_id()
    }

    pub(crate) fn facts_generation(&self) -> u64 {
        self.facts_generation
    }
}

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
    /// GPUI window currently rendering the logical dock space.
    window: AnyWindowHandle,
    /// Point relative to the dock host bounds.
    host_position: Point<Pixels>,
    /// Exact registration and route-facts generation used to derive `host_position`.
    route_proof: DockViewportRouteProof,
}

impl DockViewportTargetHit {
    #[cfg(test)]
    pub(crate) fn new(
        space: impl Into<DockSpaceId>,
        window: AnyWindowHandle,
        host_position: Point<Pixels>,
    ) -> Self {
        Self::with_route_proof(
            window,
            host_position,
            DockViewportRouteProof::for_test_registration_generation(
                space.into(),
                window.window_id(),
                1,
                0,
            ),
        )
    }

    #[cfg(test)]
    pub(crate) fn with_facts_generation(
        space: impl Into<DockSpaceId>,
        window: AnyWindowHandle,
        host_position: Point<Pixels>,
        facts_generation: u64,
    ) -> Self {
        Self::with_route_proof(
            window,
            host_position,
            DockViewportRouteProof::for_test_registration_generation(
                space.into(),
                window.window_id(),
                1,
                facts_generation,
            ),
        )
    }

    pub(crate) fn with_route_proof(
        window: AnyWindowHandle,
        host_position: Point<Pixels>,
        route_proof: DockViewportRouteProof,
    ) -> Self {
        debug_assert_eq!(route_proof.window_id(), window.window_id());
        Self {
            window,
            host_position,
            route_proof,
        }
    }

    pub(crate) fn space(&self) -> &DockSpaceId {
        self.route_proof.space()
    }

    pub(crate) fn window_id(&self) -> WindowId {
        self.route_proof.window_id()
    }

    pub(crate) fn host_position(&self) -> Point<Pixels> {
        self.host_position
    }

    pub(crate) fn route_proof(&self) -> &DockViewportRouteProof {
        &self.route_proof
    }

    #[cfg(test)]
    pub(crate) fn into_hit(self) -> DockViewportHit {
        DockViewportHit::new(self.route_proof.space().clone(), self.host_position)
    }
}

/// A registered platform viewport window that contains the pointer.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct DockViewportWindowHit {
    /// Exact logical-space-to-window registration that produced this hit.
    registration_key: DockViewportRegistrationKey,
    /// GPUI window currently rendering the logical dock space.
    window: AnyWindowHandle,
    /// Point relative to the dock host bounds, when the pointer is inside the dock host.
    host_position: Option<Point<Pixels>>,
    /// Exact route proof when current route facts can provide a host target.
    route_proof: Option<DockViewportRouteProof>,
    /// Why this window contains the pointer but cannot currently provide a host route.
    route_unavailable_reason: Option<DockViewportRouteUnavailableReason>,
}

impl DockViewportWindowHit {
    pub(crate) fn with_route_proof(
        window: AnyWindowHandle,
        host_position: Option<Point<Pixels>>,
        route_proof: DockViewportRouteProof,
    ) -> Self {
        debug_assert_eq!(route_proof.window_id(), window.window_id());
        Self {
            registration_key: route_proof.registration_key().clone(),
            window,
            host_position,
            route_proof: Some(route_proof),
            route_unavailable_reason: None,
        }
    }

    pub(crate) fn blocking(
        registration_key: DockViewportRegistrationKey,
        window: AnyWindowHandle,
        route_unavailable_reason: DockViewportRouteUnavailableReason,
    ) -> Self {
        debug_assert_eq!(registration_key.window_id(), window.window_id());
        Self {
            registration_key,
            window,
            host_position: None,
            route_proof: None,
            route_unavailable_reason: Some(route_unavailable_reason),
        }
    }

    #[cfg(test)]
    pub(crate) fn space(&self) -> &DockSpaceId {
        self.registration_key.space()
    }

    pub(crate) fn window_id(&self) -> WindowId {
        self.registration_key.window_id()
    }

    pub(crate) fn blocks_host_target(&self) -> bool {
        self.route_unavailable_reason.is_some()
            || self.host_position.is_none()
            || self.route_proof.is_none()
    }

    pub(crate) fn target_hit(&self) -> Option<DockViewportTargetHit> {
        if self.route_unavailable_reason.is_some() {
            return None;
        }
        Some(DockViewportTargetHit::with_route_proof(
            self.window,
            self.host_position?,
            self.route_proof.clone()?,
        ))
    }

    pub(crate) fn into_target_hit(self) -> Option<DockViewportTargetHit> {
        if self.route_unavailable_reason.is_some() {
            return None;
        }
        Some(DockViewportTargetHit::with_route_proof(
            self.window,
            self.host_position?,
            self.route_proof?,
        ))
    }
}

impl From<DockViewportTargetHit> for DockViewportWindowHit {
    fn from(target: DockViewportTargetHit) -> Self {
        Self::with_route_proof(
            target.window,
            Some(target.host_position),
            target.route_proof,
        )
    }
}

/// Source that selected a viewport route candidate.
///
/// This does not imply release delivery authority. The current route facts decide delivery later.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DockViewportRouteSelectionSource {
    /// Current backend hovered-window signal selected this viewport.
    TrustedHoveredWindow,
    /// The GPUI drag/drop event was delivered by this same registered viewport window and the host
    /// supplied explicit local drop-scene proof. This is reserved for in-window overlays whose
    /// target was accepted by the rendered host scene; it is not a generic event-receiver fallback.
    EventReceiverLocalScene,
    /// Backend hovered-window signal is unavailable or was discarded for a no-input viewport, and
    /// the platform front-to-back window stack selected this viewport.
    FrontToBackWindowStackFallback,
    /// Backend hovered-window and platform stack signals are unavailable, and the ImGui-style
    /// focused-viewport stamp stack selected this viewport.
    FocusStampWindowStackFallback,
    /// Drag/drop is active, hovered-window signal is unavailable, and the runtime reused the last
    /// hovered viewport as ImGui's mouse reference viewport.
    DragLastHoveredViewportFallback,
}

/// A viewport target selected by a concrete route selection source.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct DockViewportRouteSelection {
    target: DockViewportWindowHit,
    source: DockViewportRouteSelectionSource,
}

impl DockViewportRouteSelection {
    fn trusted_hovered(target: DockViewportWindowHit) -> Self {
        Self {
            target,
            source: DockViewportRouteSelectionSource::TrustedHoveredWindow,
        }
    }

    fn front_to_back_window_stack_fallback(target: DockViewportWindowHit) -> Self {
        Self {
            target,
            source: DockViewportRouteSelectionSource::FrontToBackWindowStackFallback,
        }
    }

    fn focus_stamp_window_stack_fallback(target: DockViewportWindowHit) -> Self {
        Self {
            target,
            source: DockViewportRouteSelectionSource::FocusStampWindowStackFallback,
        }
    }

    fn drag_last_hovered_viewport_fallback(target: DockViewportWindowHit) -> Self {
        Self {
            target,
            source: DockViewportRouteSelectionSource::DragLastHoveredViewportFallback,
        }
    }

    pub(crate) fn event_receiver_local_scene(target: DockViewportTargetHit) -> Self {
        Self {
            target: target.into(),
            source: DockViewportRouteSelectionSource::EventReceiverLocalScene,
        }
    }

    pub(crate) fn source(&self) -> DockViewportRouteSelectionSource {
        self.source
    }

    pub(crate) fn into_target(self) -> DockViewportWindowHit {
        self.target
    }
}

impl DockViewportRouteSelectionSource {
    pub(crate) fn requires_current_route_facts(self) -> bool {
        !matches!(self, Self::EventReceiverLocalScene)
    }

    pub(crate) fn records_routed_viewport_identity(self) -> bool {
        matches!(
            self,
            Self::TrustedHoveredWindow
                | Self::EventReceiverLocalScene
                | Self::FrontToBackWindowStackFallback
                | Self::FocusStampWindowStackFallback
                | Self::DragLastHoveredViewportFallback
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
            choose_front_to_back_window_stack_fallback_target(&window_hits, context)
                .map(DockViewportRouteSelection::into_target)
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

fn choose_front_to_back_window_stack_fallback_target(
    hits: &[DockViewportWindowHit],
    context: &DockViewportTargetContext,
) -> Option<DockViewportRouteSelection> {
    let stacked = context
        .front_to_back_window_stack_for_hover_fallback()
        .iter()
        .find_map(|window_id| {
            hits.iter()
                .find(|hit| hit.window_id() == *window_id)
                .cloned()
        });
    if let Some(target) = stacked {
        let target = match context.window_stack_source() {
            DockViewportWindowStackSource::Platform => {
                DockViewportRouteSelection::front_to_back_window_stack_fallback(target)
            }
            DockViewportWindowStackSource::FocusStampFallback => {
                DockViewportRouteSelection::focus_stamp_window_stack_fallback(target)
            }
            DockViewportWindowStackSource::Unavailable => return None,
        };
        return Some(target);
    }

    None
}

fn choose_drag_last_hovered_viewport_fallback_target(
    hits: &[DockViewportWindowHit],
    context: &DockViewportTargetContext,
) -> Option<DockViewportRouteSelection> {
    let last_hovered_window = context.drag_last_hovered_window()?;
    hits.iter()
        .find(|hit| hit.window_id() == last_hovered_window)
        .cloned()
        .map(DockViewportRouteSelection::drag_last_hovered_viewport_fallback)
}

pub(crate) fn resolve_viewport_route_selection<I, H>(
    hits: I,
    context: &DockViewportTargetContext,
) -> Option<DockViewportRouteSelection>
where
    I: IntoIterator<Item = H>,
    H: Into<DockViewportWindowHit>,
{
    let hits = hits.into_iter().map(Into::into).collect::<Vec<_>>();
    if let Some(target) = choose_trusted_hovered_viewport_target(&hits, context) {
        return Some(DockViewportRouteSelection::trusted_hovered(target));
    }

    match context.trusted_hovered_signal() {
        crate::DockViewportTrustedHoveredSignal::Unavailable => {
            if let Some(target) = choose_front_to_back_window_stack_fallback_target(&hits, context)
            {
                return Some(target);
            }
            if let Some(target) = choose_drag_last_hovered_viewport_fallback_target(&hits, context)
            {
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
            resolve_viewport_route_selection(hits(), &DockViewportTargetContext::new()),
            None,
            "ambiguous geometry is diagnostic-only"
        );

        let hovered = choose_diagnostic_viewport_target(
            hits(),
            &DockViewportTargetContext::new().with_trusted_hovered_window(second),
        )
        .expect("hovered window should resolve a target");
        assert_eq!(hovered.space(), &space("zeta"));
        let selection = resolve_viewport_route_selection(
            hits(),
            &DockViewportTargetContext::new().with_trusted_hovered_window(second),
        )
        .expect("trusted hovered window should select a route target");
        assert_eq!(
            selection.source(),
            DockViewportRouteSelectionSource::TrustedHoveredWindow
        );
        assert_eq!(selection.into_target().space(), &space("zeta"));

        let stacked = choose_diagnostic_viewport_target(
            hits(),
            &DockViewportTargetContext::new().with_window_stack([second, first]),
        )
        .expect("window stack should produce a fallback candidate");
        assert_eq!(stacked.space(), &space("zeta"));
        assert_eq!(
            resolve_viewport_route_selection(
                hits(),
                &DockViewportTargetContext::new().with_window_stack([second, first]),
            )
            .map(|target| (target.source(), target.into_target().space().clone())),
            Some((
                DockViewportRouteSelectionSource::FrontToBackWindowStackFallback,
                space("zeta"),
            )),
            "front-to-back window stack fallback selects a route only when hovered-window signal is unavailable"
        );
        assert_eq!(
            resolve_viewport_route_selection(
                hits(),
                &DockViewportTargetContext::new()
                    .with_focus_stamp_window_stack([second.window_id(), first.window_id(),]),
            )
            .map(|target| (target.source(), target.into_target().space().clone())),
            Some((
                DockViewportRouteSelectionSource::FocusStampWindowStackFallback,
                space("zeta"),
            )),
            "ImGui-style focus stamps remain a fallback route selection source but do not masquerade as a platform window stack"
        );

        assert_eq!(
            resolve_viewport_route_selection(
                hits(),
                &DockViewportTargetContext::new()
                    .with_drag_last_hovered_viewport_window(second.window_id()),
            )
            .map(|target| (target.source(), target.into_target().space().clone())),
            Some((
                DockViewportRouteSelectionSource::DragLastHoveredViewportFallback,
                space("zeta"),
            )),
            "drag last-hovered fallback is explicit route selection, not trusted backend hover"
        );
        assert_eq!(
            resolve_viewport_route_selection(
                hits(),
                &DockViewportTargetContext::new()
                    .with_drag_last_hovered_viewport_window(second.window_id())
                    .with_window_stack([first, second]),
            )
            .map(|target| (target.source(), target.into_target().space().clone())),
            Some((
                DockViewportRouteSelectionSource::FrontToBackWindowStackFallback,
                space("alpha"),
            )),
            "fresh platform window-stack fallback has priority over the drag's last hovered viewport"
        );

        let fallback = choose_diagnostic_viewport_target(
            hits(),
            &DockViewportTargetContext::new().with_window_stack([second, first]),
        )
        .expect("window stack should produce an ImGui fallback candidate");
        assert_eq!(fallback.space(), &space("zeta"));
        assert_eq!(
            resolve_viewport_route_selection(
                hits(),
                &DockViewportTargetContext::new().with_window_stack([second, first]),
            )
            .map(|target| (target.source(), target.into_target().space().clone())),
            Some((
                DockViewportRouteSelectionSource::FrontToBackWindowStackFallback,
                space("zeta"),
            ))
        );

        assert_eq!(
            resolve_viewport_route_selection(
                hits(),
                &DockViewportTargetContext::new()
                    .with_window_stack([first, second])
                    .with_window_stack([second, first]),
            )
            .map(|target| target.into_target().space().clone()),
            Some(space("zeta")),
            "platform window stack ordering remains the fallback route selection source"
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
            resolve_viewport_route_selection(
                hits(),
                &DockViewportTargetContext::new()
                    .with_trusted_hovered_window_known_empty()
                    .with_window_stack([second, first]),
            ),
            None
        );
        assert_eq!(
            resolve_viewport_route_selection(
                hits(),
                &DockViewportTargetContext::new()
                    .with_trusted_hovered_window_known_empty()
                    .with_drag_last_hovered_viewport_window(second.window_id()),
            ),
            None,
            "trusted hovered=None vetoes drag last-hovered fallback"
        );

        let single = choose_diagnostic_viewport_target(
            vec![candidate("alpha", first)],
            &DockViewportTargetContext::new(),
        )
        .expect("single hit should resolve");
        assert_eq!(single.space(), &space("alpha"));
        assert_eq!(
            resolve_viewport_route_selection(
                vec![candidate("alpha", first)],
                &DockViewportTargetContext::new(),
            ),
            None,
            "a single geometry hit remains diagnostic-only without backend hover or stack route selection"
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
            resolve_viewport_route_selection(
                vec![candidate("alpha", first)],
                &DockViewportTargetContext::new()
                    .with_trusted_hovered_window_known_empty()
                    .with_window_stack([first]),
            )
            .map(|target| (target.source(), target.into_target().space().clone())),
            None
        );

        let single_mismatched_window_stack = choose_diagnostic_viewport_target(
            vec![candidate("alpha", first)],
            &DockViewportTargetContext::new().with_window_stack([second]),
        )
        .expect("single hit should still be reported for diagnostics");
        assert_eq!(single_mismatched_window_stack.space(), &space("alpha"));
        assert_eq!(
            resolve_viewport_route_selection(
                vec![candidate("alpha", first)],
                &DockViewportTargetContext::new().with_window_stack([second]),
            )
            .map(|target| (target.source(), target.into_target().space().clone())),
            None,
            "a stack entry that does not match the live hit must not select a route"
        );
    }

    #[test]
    fn route_selection_requires_backend_signal() {
        let first = handle(1);
        let second = handle(2);
        let hits = || vec![candidate("alpha", first), candidate("zeta", second)];

        assert_eq!(
            resolve_viewport_route_selection(hits(), &DockViewportTargetContext::new()),
            None
        );
        assert_eq!(
            resolve_viewport_route_selection(
                hits(),
                &DockViewportTargetContext::new().with_window_stack([second, first]),
            )
            .map(|target| (target.source(), target.into_target().space().clone())),
            Some((
                DockViewportRouteSelectionSource::FrontToBackWindowStackFallback,
                space("zeta"),
            )),
            "window stack fallback is route selection when trusted hovered-window data is unavailable"
        );
        assert_eq!(
            resolve_viewport_route_selection(
                hits(),
                &DockViewportTargetContext::new().with_window_stack([second, first]),
            )
            .map(|target| (target.source(), target.into_target().space().clone())),
            Some((
                DockViewportRouteSelectionSource::FrontToBackWindowStackFallback,
                space("zeta"),
            )),
            "window stack fallback is route selection in the backend-hover-unavailable path"
        );
        assert_eq!(
            resolve_viewport_route_selection(
                hits(),
                &DockViewportTargetContext::new().with_trusted_hovered_window_known_empty(),
            ),
            None,
            "trusted hovered=None must not select any app viewport"
        );
        let selection = resolve_viewport_route_selection(
            hits(),
            &DockViewportTargetContext::new().with_trusted_hovered_window(second),
        )
        .expect("trusted hovered should select the matching live hit");
        assert_eq!(selection.into_target().space(), &space("zeta"));
    }
}
