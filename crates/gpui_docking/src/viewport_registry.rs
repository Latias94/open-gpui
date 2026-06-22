use crate::DockSpaceId;
use crate::DockViewportIdentity;
use open_gpui::{AnyWindowHandle, App, Bounds, DisplayId, Pixels, Window, WindowBounds, WindowId};
use std::collections::{BTreeMap, HashMap};

/// State of a registered platform viewport from the routing model's perspective.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DockViewportLifecycleState {
    /// The space/window binding exists, but no rendered host scene has published route facts yet.
    RegisteredNotReady,
    /// The latest rendered host scene and platform window facts can be used for routing.
    RouteReady,
    /// Previously published facts were invalidated and a fresh render frame must republish them.
    Stale(DockViewportStaleReason),
}

/// Reason route facts were demoted from ready to stale.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DockViewportStaleReason {
    /// GPUI reported platform window facts changed after the last rendered host scene.
    WindowFactsChanged,
    /// GPUI accepted a platform close request and the window is waiting for the close callback.
    PlatformCloseRequested,
}

/// Why a registered viewport cannot currently authorize routing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DockViewportRouteUnavailableReason {
    /// No host scene has published current route facts for this binding.
    RegisteredNotReady,
    /// Route facts exist but were invalidated by a platform change.
    Stale(DockViewportStaleReason),
    /// The latest platform facts say the window is minimized.
    Minimized,
    /// The latest platform facts say the window is a native no-input/click-through viewport.
    NoInputPassThrough,
    /// Lifecycle claims readiness, but one of the required platform/host fact snapshots is absent.
    MissingRouteFacts,
}

/// Lifecycle state machine for one registered viewport binding.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct DockViewportLifecycleMachine {
    state: DockViewportLifecycleState,
    facts_generation: u64,
    platform_focus_order_stamp: Option<u64>,
}

impl Default for DockViewportLifecycleMachine {
    fn default() -> Self {
        Self {
            state: DockViewportLifecycleState::RegisteredNotReady,
            facts_generation: 0,
            platform_focus_order_stamp: None,
        }
    }
}

impl DockViewportLifecycleMachine {
    #[cfg(test)]
    fn state(&self) -> DockViewportLifecycleState {
        self.state
    }

    fn facts_generation(&self) -> u64 {
        self.facts_generation
    }

    fn platform_focus_order_stamp(&self) -> u64 {
        self.platform_focus_order_stamp.unwrap_or(0)
    }

    fn route_unavailable_reason(&self) -> Option<DockViewportRouteUnavailableReason> {
        match self.state {
            DockViewportLifecycleState::RegisteredNotReady => {
                Some(DockViewportRouteUnavailableReason::RegisteredNotReady)
            }
            DockViewportLifecycleState::RouteReady => None,
            DockViewportLifecycleState::Stale(reason) => {
                Some(DockViewportRouteUnavailableReason::Stale(reason))
            }
        }
    }

    fn is_route_ready(&self) -> bool {
        matches!(self.state, DockViewportLifecycleState::RouteReady)
    }

    fn is_platform_close_requested(&self) -> bool {
        matches!(
            self.state,
            DockViewportLifecycleState::Stale(DockViewportStaleReason::PlatformCloseRequested)
        )
    }

    fn mark_route_ready(&mut self) {
        self.state = DockViewportLifecycleState::RouteReady;
        self.advance_generation();
    }

    fn mark_stale(&mut self, reason: DockViewportStaleReason) -> bool {
        if matches!(self.state, DockViewportLifecycleState::Stale(existing) if existing == reason) {
            return false;
        }
        self.state = DockViewportLifecycleState::Stale(reason);
        self.advance_generation();
        true
    }

    fn cancel_platform_close_request(&mut self) -> bool {
        if !self.is_platform_close_requested() {
            return false;
        }
        self.state = DockViewportLifecycleState::Stale(DockViewportStaleReason::WindowFactsChanged);
        self.advance_generation();
        true
    }

    fn record_platform_focus_order(&mut self, platform_focus_order_stamp: u64) {
        self.platform_focus_order_stamp = Some(platform_focus_order_stamp);
    }

    fn advance_generation(&mut self) {
        self.facts_generation = self.facts_generation.wrapping_add(1);
    }
}

/// Platform window facts captured from a live rendered viewport.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct DockViewportWindowFacts {
    /// Display currently containing the window.
    pub(crate) display_id: Option<DisplayId>,
    /// Platform window state suitable for placement persistence.
    pub(crate) window_bounds: WindowBounds,
    /// Current window rectangle tagged with the coordinate space the backend can actually report.
    pub(crate) current_bounds: DockViewportWindowBoundsFrame,
    /// Current platform pointer-routing state.
    pub(crate) pointer_routing: DockViewportPointerRouting,
}

/// Coordinate frame for a live viewport window rectangle.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum DockViewportWindowBoundsFrame {
    /// Bounds are in a shared desktop coordinate space and may authorize global hit testing.
    GlobalScreen(Bounds<Pixels>),
    /// Bounds are only meaningful in the receiver window's local coordinate space.
    WindowLocal(Bounds<Pixels>),
}

impl DockViewportWindowBoundsFrame {
    pub(crate) fn global_screen_bounds(self) -> Option<Bounds<Pixels>> {
        match self {
            Self::GlobalScreen(bounds) => Some(bounds),
            Self::WindowLocal(_) => None,
        }
    }

    pub(crate) fn size(self) -> open_gpui::Size<Pixels> {
        match self {
            Self::GlobalScreen(bounds) | Self::WindowLocal(bounds) => bounds.size,
        }
    }
}

/// Current platform pointer-routing state for a viewport window.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DockViewportPointerRouting {
    /// The window can be hit-tested and routed normally.
    Routable,
    /// The window is minimized and must not authorize a route or no-input underlay.
    Minimized,
    /// The window is explicitly click-through, so routing may pass through it with backend support.
    NoInputPassThrough,
}

/// Platform-window requests reported by the backend and not yet consumed by a fresh host scene.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct DockViewportPlatformRequests {
    /// The platform has reported an authoritative live resize for this viewport window.
    pub(crate) resize_requested: bool,
}

impl DockViewportWindowFacts {
    #[cfg(test)]
    pub(crate) fn new(
        display_id: Option<DisplayId>,
        window_bounds: WindowBounds,
        screen_bounds: Bounds<Pixels>,
    ) -> Self {
        Self::with_current_bounds(
            display_id,
            window_bounds,
            DockViewportWindowBoundsFrame::GlobalScreen(screen_bounds),
        )
    }

    pub(crate) fn with_current_bounds(
        display_id: Option<DisplayId>,
        window_bounds: WindowBounds,
        current_bounds: DockViewportWindowBoundsFrame,
    ) -> Self {
        Self {
            display_id,
            window_bounds,
            current_bounds,
            pointer_routing: DockViewportPointerRouting::Routable,
        }
    }

    pub(crate) fn from_window(window: &Window, cx: &App) -> Self {
        let current_bounds = if cx.viewport_capabilities().global_window_bounds {
            DockViewportWindowBoundsFrame::GlobalScreen(window.bounds())
        } else {
            DockViewportWindowBoundsFrame::WindowLocal(window.bounds())
        };
        let mut facts = Self::with_current_bounds(
            window.display(cx).map(|display| display.id()),
            window.window_bounds(),
            current_bounds,
        );
        if window.is_minimized() {
            facts.pointer_routing = DockViewportPointerRouting::Minimized;
        } else if !window.accepts_pointer_input() {
            facts.pointer_routing = DockViewportPointerRouting::NoInputPassThrough;
        }
        facts
    }

    #[cfg(test)]
    pub(crate) fn with_pointer_routing(
        mut self,
        pointer_routing: DockViewportPointerRouting,
    ) -> Self {
        self.pointer_routing = pointer_routing;
        self
    }

    #[cfg(test)]
    pub(crate) fn from_window_bounds(window_bounds: WindowBounds) -> Self {
        Self::new(None, window_bounds, window_bounds.get_bounds())
    }

    #[cfg(test)]
    pub(crate) fn trusted_global_window_bounds_for_test(window_bounds: WindowBounds) -> Self {
        Self::from_window_bounds(window_bounds)
    }

    #[cfg(test)]
    pub(crate) fn local_only_window_bounds_for_test(window_bounds: WindowBounds) -> Self {
        Self::with_current_bounds(
            None,
            window_bounds,
            DockViewportWindowBoundsFrame::WindowLocal(window_bounds.get_bounds()),
        )
    }

    #[cfg(test)]
    pub(crate) fn with_display_id(mut self, display_id: Option<DisplayId>) -> Self {
        self.display_id = display_id;
        self
    }
}

/// Runtime snapshot for one rendered dock viewport.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct DockViewportSnapshot {
    /// GPUI window currently rendering the logical dock space.
    pub(crate) window: AnyWindowHandle,
    /// Display containing the window, when the application has recorded one.
    pub(crate) display_id: Option<DisplayId>,
    /// Last known platform window state used for placement persistence.
    pub(crate) window_bounds: Option<WindowBounds>,
    /// Last known current window rectangle with its backend coordinate frame.
    pub(crate) current_bounds: Option<DockViewportWindowBoundsFrame>,
    /// Last known dock host bounds in window-local coordinates.
    pub(crate) host_bounds: Option<Bounds<Pixels>>,
    /// Last known platform pointer-routing state.
    pub(crate) pointer_routing: DockViewportPointerRouting,
    platform_requests: DockViewportPlatformRequests,
    lifecycle: DockViewportLifecycleMachine,
}

impl DockViewportSnapshot {
    /// Creates a snapshot for a newly registered viewport window.
    pub(crate) fn new(window: AnyWindowHandle) -> Self {
        Self {
            window,
            display_id: None,
            window_bounds: None,
            current_bounds: None,
            host_bounds: None,
            pointer_routing: DockViewportPointerRouting::Minimized,
            platform_requests: DockViewportPlatformRequests::default(),
            lifecycle: DockViewportLifecycleMachine::default(),
        }
    }

    fn identity(&self, space: &DockSpaceId) -> DockViewportIdentity {
        DockViewportIdentity::new(space.clone(), self.window.window_id())
    }

    #[cfg(test)]
    pub(crate) fn lifecycle_state(&self) -> DockViewportLifecycleState {
        self.lifecycle.state()
    }

    pub(crate) fn route_unavailable_reason(&self) -> Option<DockViewportRouteUnavailableReason> {
        if let Some(reason) = self.lifecycle.route_unavailable_reason() {
            return Some(reason);
        }
        if self.has_missing_route_facts() {
            return Some(DockViewportRouteUnavailableReason::MissingRouteFacts);
        }
        match self.pointer_routing {
            DockViewportPointerRouting::Routable => {}
            DockViewportPointerRouting::Minimized => {
                return Some(DockViewportRouteUnavailableReason::Minimized);
            }
            DockViewportPointerRouting::NoInputPassThrough => {
                return Some(DockViewportRouteUnavailableReason::NoInputPassThrough);
            }
        }
        None
    }

    pub(crate) fn is_route_ready(&self) -> bool {
        self.has_current_route_facts()
    }

    pub(crate) fn is_platform_close_requested(&self) -> bool {
        self.lifecycle.is_platform_close_requested()
    }

    pub(crate) fn facts_generation(&self) -> u64 {
        self.lifecycle.facts_generation()
    }

    pub(crate) fn platform_focus_order_stamp(&self) -> u64 {
        self.lifecycle.platform_focus_order_stamp()
    }

    pub(crate) fn platform_requests(&self) -> DockViewportPlatformRequests {
        self.platform_requests
    }

    pub(crate) fn global_screen_bounds(&self) -> Option<Bounds<Pixels>> {
        self.current_bounds?.global_screen_bounds()
    }

    pub(crate) fn facts_generation_if_current(&self, window_id: WindowId) -> Option<u64> {
        (self.window.window_id() == window_id && self.has_current_route_facts())
            .then(|| self.facts_generation())
    }

    pub(crate) fn update_route_facts(
        &mut self,
        window_facts: DockViewportWindowFacts,
        host_bounds: Bounds<Pixels>,
    ) -> bool {
        if self.lifecycle.is_platform_close_requested() {
            return false;
        }

        let display_id = window_facts.display_id;
        let window_bounds = Some(window_facts.window_bounds);
        let current_bounds = Some(window_facts.current_bounds);
        let host_bounds = Some(host_bounds);
        let pointer_routing = window_facts.pointer_routing;
        if self.display_id == display_id
            && self.window_bounds == window_bounds
            && self.current_bounds == current_bounds
            && self.host_bounds == host_bounds
            && self.pointer_routing == pointer_routing
            && self.lifecycle.is_route_ready()
        {
            if self.platform_requests == DockViewportPlatformRequests::default() {
                return false;
            }
            self.platform_requests = DockViewportPlatformRequests::default();
            return true;
        }

        self.display_id = display_id;
        self.window_bounds = window_bounds;
        self.current_bounds = current_bounds;
        self.host_bounds = host_bounds;
        self.pointer_routing = pointer_routing;
        self.platform_requests = DockViewportPlatformRequests::default();
        self.lifecycle.mark_route_ready();
        true
    }

    pub(crate) fn apply_platform_window_facts(
        &mut self,
        window_facts: DockViewportWindowFacts,
    ) -> bool {
        if self.lifecycle.is_platform_close_requested() {
            return false;
        }

        self.platform_requests = self.platform_requests_after_window_facts(window_facts);

        if self.can_preserve_route_facts_for_platform_move(window_facts) {
            return self.replace_window_facts_without_generation(window_facts);
        }

        let changed = self.replace_window_facts_without_generation(window_facts);
        self.lifecycle
            .mark_stale(DockViewportStaleReason::WindowFactsChanged)
            || changed
    }

    fn can_preserve_route_facts_for_platform_move(
        &self,
        window_facts: DockViewportWindowFacts,
    ) -> bool {
        if !self.lifecycle.is_route_ready()
            || self.host_bounds.is_none()
            || self.pointer_routing != DockViewportPointerRouting::Routable
            || window_facts.pointer_routing != DockViewportPointerRouting::Routable
        {
            return false;
        }

        let Some(DockViewportWindowBoundsFrame::GlobalScreen(current)) = self.current_bounds else {
            return false;
        };
        let DockViewportWindowBoundsFrame::GlobalScreen(next) = window_facts.current_bounds else {
            return false;
        };

        current.size == next.size
    }

    fn platform_requests_after_window_facts(
        &self,
        window_facts: DockViewportWindowFacts,
    ) -> DockViewportPlatformRequests {
        let Some(current_bounds) = self.current_bounds else {
            return DockViewportPlatformRequests {
                resize_requested: true,
            };
        };

        DockViewportPlatformRequests {
            resize_requested: self.platform_requests.resize_requested
                || current_bounds.size() != window_facts.current_bounds.size(),
        }
    }

    fn replace_window_facts_without_generation(
        &mut self,
        window_facts: DockViewportWindowFacts,
    ) -> bool {
        let display_id = window_facts.display_id;
        let window_bounds = Some(window_facts.window_bounds);
        let current_bounds = Some(window_facts.current_bounds);
        let pointer_routing = window_facts.pointer_routing;
        if self.display_id == display_id
            && self.window_bounds == window_bounds
            && self.current_bounds == current_bounds
            && self.pointer_routing == pointer_routing
        {
            return false;
        }

        self.display_id = display_id;
        self.window_bounds = window_bounds;
        self.current_bounds = current_bounds;
        self.pointer_routing = pointer_routing;
        true
    }

    pub(crate) fn refresh_pointer_routing(
        &mut self,
        pointer_routing: DockViewportPointerRouting,
    ) -> bool {
        if self.lifecycle.is_platform_close_requested() || self.pointer_routing == pointer_routing {
            return false;
        }

        self.pointer_routing = pointer_routing;
        if self.lifecycle.is_route_ready() {
            self.lifecycle.mark_route_ready();
        }
        true
    }

    pub(crate) fn mark_route_facts_stale(&mut self, reason: DockViewportStaleReason) -> bool {
        self.lifecycle.mark_stale(reason)
    }

    pub(crate) fn cancel_platform_close_request(&mut self) -> bool {
        self.lifecycle.cancel_platform_close_request()
    }

    pub(crate) fn record_platform_focus_order(&mut self, platform_focus_order_stamp: u64) {
        self.lifecycle
            .record_platform_focus_order(platform_focus_order_stamp);
    }

    fn has_current_route_facts(&self) -> bool {
        self.lifecycle.is_route_ready()
            && !self.has_missing_route_facts()
            && self.pointer_routing == DockViewportPointerRouting::Routable
    }

    fn has_missing_route_facts(&self) -> bool {
        self.window_bounds.is_none() || self.current_bounds.is_none() || self.host_bounds.is_none()
    }
}

/// Internal one-to-one mapping between logical dock spaces and GPUI windows.
#[derive(Debug, Default)]
pub(crate) struct DockViewportRegistry {
    viewports: BTreeMap<DockSpaceId, DockViewportSnapshot>,
    windows: HashMap<WindowId, DockSpaceId>,
    next_platform_focus_order_stamp: u64,
}

impl DockViewportRegistry {
    #[cfg(test)]
    pub(crate) fn register(
        &mut self,
        space: DockSpaceId,
        window: AnyWindowHandle,
    ) -> Option<DockViewportSnapshot> {
        self.register_with_replacements(space.clone(), window)
            .into_iter()
            .find(|(removed_space, _)| *removed_space == space)
            .map(|(_, snapshot)| snapshot)
    }

    pub(crate) fn register_with_replacements(
        &mut self,
        space: DockSpaceId,
        window: AnyWindowHandle,
    ) -> Vec<(DockSpaceId, DockViewportSnapshot)> {
        let window_id = window.window_id();
        if let Some(snapshot) = self.viewports.get(&space)
            && snapshot.identity(&space).matches(&space, window_id)
            && self
                .windows
                .get(&window_id)
                .is_none_or(|registered_space| registered_space == &space)
        {
            self.windows.insert(window_id, space);
            return Vec::new();
        }

        let mut replaced = Vec::new();

        if let Some(previous) = self.viewports.remove(&space) {
            self.windows.remove(&previous.window.window_id());
            replaced.push((space.clone(), previous));
        }
        if let Some(previous_space) = self.windows.remove(&window_id)
            && previous_space != space
            && let Some(previous) = self.viewports.remove(&previous_space)
        {
            replaced.push((previous_space, previous));
        }

        self.windows.insert(window_id, space.clone());
        self.viewports
            .insert(space, DockViewportSnapshot::new(window));
        replaced
    }

    pub(crate) fn record_platform_focus_order_window(&mut self, window_id: WindowId) -> bool {
        let Some(space) = self.space_for_window_id(window_id).cloned() else {
            return false;
        };
        self.record_platform_focus_order_space(&space)
    }

    fn record_platform_focus_order_space(&mut self, space: &DockSpaceId) -> bool {
        let Some(snapshot) = self.viewports.get(space) else {
            return false;
        };
        if self.next_platform_focus_order_stamp != 0
            && snapshot.platform_focus_order_stamp() == self.next_platform_focus_order_stamp
        {
            return false;
        }
        self.next_platform_focus_order_stamp = self.next_platform_focus_order_stamp.wrapping_add(1);
        if let Some(snapshot) = self.viewports.get_mut(space) {
            snapshot.record_platform_focus_order(self.next_platform_focus_order_stamp);
        }
        true
    }

    pub(crate) fn unregister_space(&mut self, space: &DockSpaceId) -> Option<DockViewportSnapshot> {
        let snapshot = self.viewports.remove(space)?;
        self.windows.remove(&snapshot.window.window_id());
        Some(snapshot)
    }

    pub(crate) fn unregister_window_id(
        &mut self,
        window_id: WindowId,
    ) -> Option<(DockSpaceId, DockViewportSnapshot)> {
        let space = self.windows.remove(&window_id)?;
        if !self
            .viewports
            .get(&space)
            .is_some_and(|snapshot| snapshot.identity(&space).matches(&space, window_id))
        {
            return None;
        }
        let snapshot = self.viewports.remove(&space)?;
        Some((space, snapshot))
    }

    pub(crate) fn snapshot(&self, space: &DockSpaceId) -> Option<&DockViewportSnapshot> {
        self.viewports.get(space)
    }

    pub(crate) fn snapshot_mut(
        &mut self,
        space: &DockSpaceId,
    ) -> Option<&mut DockViewportSnapshot> {
        self.viewports.get_mut(space)
    }

    pub(crate) fn window_for_space(&self, space: &DockSpaceId) -> Option<AnyWindowHandle> {
        self.snapshot(space).map(|snapshot| snapshot.window)
    }

    pub(crate) fn space_for_window_id(&self, window_id: WindowId) -> Option<&DockSpaceId> {
        let space = self.windows.get(&window_id)?;
        let (space, snapshot) = self.viewports.get_key_value(space)?;
        snapshot
            .identity(space)
            .matches(space, window_id)
            .then_some(space)
    }

    pub(crate) fn spaces(&self) -> Vec<DockSpaceId> {
        self.viewports.keys().cloned().collect()
    }

    pub(crate) fn snapshots(&self) -> impl Iterator<Item = (&DockSpaceId, &DockViewportSnapshot)> {
        self.viewports.iter()
    }

    #[cfg(test)]
    pub(crate) fn snapshots_by_platform_focus_order(
        &self,
    ) -> Vec<(&DockSpaceId, &DockViewportSnapshot)> {
        let mut snapshots = self.viewports.iter().collect::<Vec<_>>();
        snapshots.retain(|(_, snapshot)| snapshot.platform_focus_order_stamp() != 0);
        snapshots.sort_by(|(left_space, left), (right_space, right)| {
            right
                .platform_focus_order_stamp()
                .cmp(&left.platform_focus_order_stamp())
                .then_with(|| left_space.cmp(right_space))
        });
        snapshots
    }

    #[cfg(test)]
    pub(crate) fn spaces_by_platform_focus_order(&self) -> Vec<DockSpaceId> {
        self.snapshots_by_platform_focus_order()
            .into_iter()
            .map(|(space, _)| space.clone())
            .collect()
    }

    #[cfg(test)]
    pub(crate) fn insert_stale_window_index_for_test(
        &mut self,
        window_id: WindowId,
        space: DockSpaceId,
    ) {
        self.windows.insert(window_id, space);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::viewport_test_support::{handle, space};

    #[test]
    fn newly_registered_viewport_starts_not_route_ready() {
        let mut registry = DockViewportRegistry::default();
        let main = space("main");
        let window = handle(1);

        registry.register(main.clone(), window);

        let snapshot = registry
            .snapshot(&main)
            .expect("registered viewport should have a snapshot");
        assert_eq!(
            snapshot.lifecycle_state(),
            DockViewportLifecycleState::RegisteredNotReady
        );
        assert!(!snapshot.is_route_ready());
        assert_eq!(
            snapshot.route_unavailable_reason(),
            Some(DockViewportRouteUnavailableReason::RegisteredNotReady)
        );
    }

    #[test]
    fn register_keeps_space_and_window_indexes_one_to_one() {
        let mut registry = DockViewportRegistry::default();
        let main = space("main");
        let secondary = space("secondary");
        let first = handle(1);
        let second = handle(2);

        assert!(registry.register(main.clone(), first).is_none());
        assert_eq!(registry.window_for_space(&main), Some(first));
        assert_eq!(registry.space_for_window_id(first.window_id()), Some(&main));

        let previous = registry
            .register(main.clone(), second)
            .expect("replacing a space should return the previous snapshot");
        assert_eq!(previous.window, first);
        assert_eq!(registry.window_for_space(&main), Some(second));
        assert_eq!(registry.space_for_window_id(first.window_id()), None);

        registry.register(secondary.clone(), second);
        assert_eq!(registry.window_for_space(&main), None);
        assert_eq!(registry.window_for_space(&secondary), Some(second));
        assert_eq!(registry.spaces(), vec![secondary]);
    }

    #[test]
    fn valid_window_lookup_ignores_and_cleanup_discards_stale_indexes() {
        let mut registry = DockViewportRegistry::default();
        let window_id = WindowId::from(7);
        registry.insert_stale_window_index_for_test(window_id, space("missing"));

        assert_eq!(registry.space_for_window_id(window_id), None);
        assert_eq!(registry.unregister_window_id(window_id), None);
        assert_eq!(registry.space_for_window_id(window_id), None);
    }

    #[test]
    fn valid_window_lookup_rejects_stale_index_to_rebound_space() {
        let mut registry = DockViewportRegistry::default();
        let main = space("main");
        let stale_window_id = WindowId::from(7);
        let current_window = handle(8);
        registry.register(main.clone(), current_window);
        registry.insert_stale_window_index_for_test(stale_window_id, main.clone());

        assert_eq!(registry.space_for_window_id(stale_window_id), None);
        assert_eq!(registry.unregister_window_id(stale_window_id), None);
        assert_eq!(registry.window_for_space(&main), Some(current_window));
        assert_eq!(
            registry.space_for_window_id(current_window.window_id()),
            Some(&main)
        );
    }

    #[test]
    fn replacing_a_viewport_clears_the_previous_platform_focus_order_stamp() {
        let mut registry = DockViewportRegistry::default();
        let alpha = space("alpha");
        let zeta = space("zeta");
        let alpha_window = handle(1);
        let zeta_window = handle(2);
        let rebound_window = handle(3);

        registry.register(alpha.clone(), alpha_window);
        registry.register(zeta.clone(), zeta_window);
        registry.record_platform_focus_order_window(alpha_window.window_id());
        registry.record_platform_focus_order_window(zeta_window.window_id());
        assert_eq!(
            registry.spaces_by_platform_focus_order(),
            vec![zeta.clone(), alpha.clone()]
        );

        registry.register(zeta.clone(), rebound_window);

        assert_eq!(registry.space_for_window_id(zeta_window.window_id()), None);
        assert_eq!(
            registry.spaces_by_platform_focus_order(),
            vec![alpha],
            "a rebound window must not inherit the previous window's platform focus order stamp"
        );
    }

    #[test]
    fn minimized_window_facts_are_not_route_ready() {
        let mut registry = DockViewportRegistry::default();
        let main = space("main");
        let window = handle(1);
        registry.register(main.clone(), window);

        let snapshot = registry
            .snapshot_mut(&main)
            .expect("registered viewport should have a mutable snapshot");
        assert!(
            snapshot.update_route_facts(
                DockViewportWindowFacts::from_window_bounds(WindowBounds::Windowed(Bounds::new(
                    open_gpui::point(open_gpui::px(100.0), open_gpui::px(100.0)),
                    open_gpui::size(open_gpui::px(320.0), open_gpui::px(240.0)),
                )))
                .with_pointer_routing(DockViewportPointerRouting::Minimized),
                Bounds::new(
                    open_gpui::point(open_gpui::px(0.0), open_gpui::px(0.0)),
                    open_gpui::size(open_gpui::px(320.0), open_gpui::px(240.0)),
                ),
            )
        );

        assert!(!snapshot.is_route_ready());
        assert_eq!(
            snapshot.route_unavailable_reason(),
            Some(DockViewportRouteUnavailableReason::Minimized)
        );
    }

    #[test]
    fn no_input_window_facts_are_not_route_ready() {
        let mut registry = DockViewportRegistry::default();
        let main = space("main");
        let window = handle(1);
        registry.register(main.clone(), window);

        let snapshot = registry
            .snapshot_mut(&main)
            .expect("registered viewport should have a mutable snapshot");
        assert!(
            snapshot.update_route_facts(
                DockViewportWindowFacts::from_window_bounds(WindowBounds::Windowed(Bounds::new(
                    open_gpui::point(open_gpui::px(100.0), open_gpui::px(100.0)),
                    open_gpui::size(open_gpui::px(320.0), open_gpui::px(240.0)),
                )))
                .with_pointer_routing(DockViewportPointerRouting::NoInputPassThrough),
                Bounds::new(
                    open_gpui::point(open_gpui::px(0.0), open_gpui::px(0.0)),
                    open_gpui::size(open_gpui::px(320.0), open_gpui::px(240.0)),
                ),
            )
        );

        assert!(!snapshot.is_route_ready());
        assert_eq!(
            snapshot.route_unavailable_reason(),
            Some(DockViewportRouteUnavailableReason::NoInputPassThrough)
        );
    }

    #[test]
    fn platform_close_requested_cannot_be_restored_by_route_fact_update() {
        let mut registry = DockViewportRegistry::default();
        let main = space("main");
        let window = handle(1);
        registry.register(main.clone(), window);

        let snapshot = registry
            .snapshot_mut(&main)
            .expect("registered viewport should have a mutable snapshot");
        assert!(snapshot.update_route_facts(
            DockViewportWindowFacts::from_window_bounds(WindowBounds::Windowed(Bounds::new(
                open_gpui::point(open_gpui::px(100.0), open_gpui::px(100.0)),
                open_gpui::size(open_gpui::px(320.0), open_gpui::px(240.0)),
            ))),
            Bounds::new(
                open_gpui::point(open_gpui::px(0.0), open_gpui::px(0.0)),
                open_gpui::size(open_gpui::px(320.0), open_gpui::px(240.0)),
            ),
        ));
        assert!(snapshot.is_route_ready());

        assert!(snapshot.mark_route_facts_stale(DockViewportStaleReason::PlatformCloseRequested));
        assert!(!snapshot.update_route_facts(
            DockViewportWindowFacts::from_window_bounds(WindowBounds::Windowed(Bounds::new(
                open_gpui::point(open_gpui::px(120.0), open_gpui::px(120.0)),
                open_gpui::size(open_gpui::px(320.0), open_gpui::px(240.0)),
            ))),
            Bounds::new(
                open_gpui::point(open_gpui::px(0.0), open_gpui::px(0.0)),
                open_gpui::size(open_gpui::px(320.0), open_gpui::px(240.0)),
            ),
        ));
        assert_eq!(
            snapshot.lifecycle_state(),
            DockViewportLifecycleState::Stale(DockViewportStaleReason::PlatformCloseRequested)
        );
        assert_eq!(
            snapshot.route_unavailable_reason(),
            Some(DockViewportRouteUnavailableReason::Stale(
                DockViewportStaleReason::PlatformCloseRequested
            ))
        );
        assert!(!snapshot.is_route_ready());
    }

    #[test]
    fn platform_close_requested_requires_explicit_cancellation_before_route_fact_update() {
        let mut registry = DockViewportRegistry::default();
        let main = space("main");
        let window = handle(1);
        registry.register(main.clone(), window);

        let snapshot = registry
            .snapshot_mut(&main)
            .expect("registered viewport should have a mutable snapshot");
        assert!(snapshot.update_route_facts(
            DockViewportWindowFacts::from_window_bounds(WindowBounds::Windowed(Bounds::new(
                open_gpui::point(open_gpui::px(100.0), open_gpui::px(100.0)),
                open_gpui::size(open_gpui::px(320.0), open_gpui::px(240.0)),
            ))),
            Bounds::new(
                open_gpui::point(open_gpui::px(0.0), open_gpui::px(0.0)),
                open_gpui::size(open_gpui::px(320.0), open_gpui::px(240.0)),
            ),
        ));
        assert!(snapshot.mark_route_facts_stale(DockViewportStaleReason::PlatformCloseRequested));

        assert!(snapshot.cancel_platform_close_request());
        assert_eq!(
            snapshot.lifecycle_state(),
            DockViewportLifecycleState::Stale(DockViewportStaleReason::WindowFactsChanged)
        );
        assert!(!snapshot.is_route_ready());

        assert!(snapshot.update_route_facts(
            DockViewportWindowFacts::from_window_bounds(WindowBounds::Windowed(Bounds::new(
                open_gpui::point(open_gpui::px(120.0), open_gpui::px(120.0)),
                open_gpui::size(open_gpui::px(320.0), open_gpui::px(240.0)),
            ))),
            Bounds::new(
                open_gpui::point(open_gpui::px(0.0), open_gpui::px(0.0)),
                open_gpui::size(open_gpui::px(320.0), open_gpui::px(240.0)),
            ),
        ));
        assert!(snapshot.is_route_ready());
        assert!(!snapshot.cancel_platform_close_request());
    }
}
