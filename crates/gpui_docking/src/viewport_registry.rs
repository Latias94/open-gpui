use crate::DockSpaceId;
use crate::DockViewportIdentity;
use open_gpui::{AnyWindowHandle, Bounds, DisplayId, Pixels, WindowBounds, WindowId};
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
}

/// Why a registered viewport cannot currently authorize routing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DockViewportRouteUnavailableReason {
    /// No host scene has published current route facts for this binding.
    RegisteredNotReady,
    /// Route facts exist but were invalidated by a platform change.
    Stale(DockViewportStaleReason),
    /// Lifecycle claims readiness, but one of the required platform/host fact snapshots is absent.
    MissingRouteFacts,
}

/// Lifecycle state machine for one registered viewport binding.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct DockViewportLifecycleMachine {
    state: DockViewportLifecycleState,
    facts_generation: u64,
    focus_stamp: Option<u64>,
}

impl Default for DockViewportLifecycleMachine {
    fn default() -> Self {
        Self {
            state: DockViewportLifecycleState::RegisteredNotReady,
            facts_generation: 0,
            focus_stamp: None,
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

    fn focus_stamp(&self) -> u64 {
        self.focus_stamp.unwrap_or(0)
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

    fn record_focus(&mut self, focus_stamp: u64) {
        self.focus_stamp = Some(focus_stamp);
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
    /// Current window rectangle in global screen coordinates.
    pub(crate) screen_bounds: Bounds<Pixels>,
}

impl DockViewportWindowFacts {
    pub(crate) fn new(
        display_id: Option<DisplayId>,
        window_bounds: WindowBounds,
        screen_bounds: Bounds<Pixels>,
    ) -> Self {
        Self {
            display_id,
            window_bounds,
            screen_bounds,
        }
    }

    #[cfg(test)]
    pub(crate) fn from_window_bounds(window_bounds: WindowBounds) -> Self {
        Self::new(None, window_bounds, window_bounds.get_bounds())
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
    /// Last known current window rectangle in global screen coordinates.
    pub(crate) screen_bounds: Option<Bounds<Pixels>>,
    /// Last known dock host bounds in window-local coordinates.
    pub(crate) host_bounds: Option<Bounds<Pixels>>,
    lifecycle: DockViewportLifecycleMachine,
}

impl DockViewportSnapshot {
    /// Creates a snapshot for a newly registered viewport window.
    pub(crate) fn new(window: AnyWindowHandle) -> Self {
        Self {
            window,
            display_id: None,
            window_bounds: None,
            screen_bounds: None,
            host_bounds: None,
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
        if self.has_current_route_facts() {
            return None;
        }

        self.lifecycle
            .route_unavailable_reason()
            .or(Some(DockViewportRouteUnavailableReason::MissingRouteFacts))
    }

    pub(crate) fn is_route_ready(&self) -> bool {
        self.has_current_route_facts()
    }

    pub(crate) fn facts_generation(&self) -> u64 {
        self.lifecycle.facts_generation()
    }

    pub(crate) fn focus_stamp(&self) -> u64 {
        self.lifecycle.focus_stamp()
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
        let display_id = window_facts.display_id;
        let window_bounds = Some(window_facts.window_bounds);
        let screen_bounds = Some(window_facts.screen_bounds);
        let host_bounds = Some(host_bounds);
        if self.display_id == display_id
            && self.window_bounds == window_bounds
            && self.screen_bounds == screen_bounds
            && self.host_bounds == host_bounds
            && self.lifecycle.is_route_ready()
        {
            return false;
        }

        self.display_id = display_id;
        self.window_bounds = window_bounds;
        self.screen_bounds = screen_bounds;
        self.host_bounds = host_bounds;
        self.lifecycle.mark_route_ready();
        true
    }

    pub(crate) fn mark_route_facts_stale(&mut self, reason: DockViewportStaleReason) -> bool {
        self.lifecycle.mark_stale(reason)
    }

    pub(crate) fn record_focus(&mut self, focus_stamp: u64) {
        self.lifecycle.record_focus(focus_stamp);
    }

    fn has_current_route_facts(&self) -> bool {
        self.lifecycle.is_route_ready()
            && self.window_bounds.is_some()
            && self.screen_bounds.is_some()
            && self.host_bounds.is_some()
    }
}

/// Internal one-to-one mapping between logical dock spaces and GPUI windows.
#[derive(Debug, Default)]
pub(crate) struct DockViewportRegistry {
    viewports: BTreeMap<DockSpaceId, DockViewportSnapshot>,
    windows: HashMap<WindowId, DockSpaceId>,
    next_focus_stamp: u64,
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

    pub(crate) fn record_window_focus(&mut self, window_id: WindowId) {
        let Some(space) = self.space_for_window_id(window_id).cloned() else {
            return;
        };
        self.next_focus_stamp = self.next_focus_stamp.wrapping_add(1);
        if let Some(snapshot) = self.viewports.get_mut(&space) {
            snapshot.record_focus(self.next_focus_stamp);
        }
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

    pub(crate) fn spaces_by_diagnostic_hit_order(&self) -> Vec<DockSpaceId> {
        let mut spaces = self.spaces();
        spaces.sort_by(|left, right| {
            let left_stamp = self
                .viewports
                .get(left)
                .map(DockViewportSnapshot::focus_stamp)
                .unwrap_or_default();
            let right_stamp = self
                .viewports
                .get(right)
                .map(DockViewportSnapshot::focus_stamp)
                .unwrap_or_default();
            right_stamp.cmp(&left_stamp).then_with(|| left.cmp(right))
        });
        spaces
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
    fn replacing_a_viewport_clears_the_previous_focus_stamp() {
        let mut registry = DockViewportRegistry::default();
        let alpha = space("alpha");
        let zeta = space("zeta");
        let alpha_window = handle(1);
        let zeta_window = handle(2);
        let rebound_window = handle(3);

        registry.register(alpha.clone(), alpha_window);
        registry.register(zeta.clone(), zeta_window);
        registry.record_window_focus(alpha_window.window_id());
        registry.record_window_focus(zeta_window.window_id());
        assert_eq!(
            registry.spaces_by_diagnostic_hit_order(),
            vec![zeta.clone(), alpha.clone()]
        );

        registry.register(zeta.clone(), rebound_window);

        assert_eq!(registry.space_for_window_id(zeta_window.window_id()), None);
        assert_eq!(registry.spaces_by_diagnostic_hit_order(), vec![alpha, zeta]);
    }
}
