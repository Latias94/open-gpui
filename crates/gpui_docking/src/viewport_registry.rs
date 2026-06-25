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
}

/// Why a registered viewport cannot currently provide route facts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DockViewportRouteUnavailableReason {
    /// GPUI accepted a platform close request and the window is waiting for the close callback.
    PlatformCloseRequested,
    /// No host scene has published current route facts for this binding.
    RegisteredNotReady,
    /// Route facts exist but were invalidated by a platform change.
    Stale(DockViewportStaleReason),
    /// The latest platform facts say the window is minimized.
    Minimized,
    /// Lifecycle claims readiness, but one of the required platform/host fact snapshots is absent.
    MissingRouteFacts,
}

/// Lifecycle state machine for one registered viewport binding.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct DockViewportLifecycleMachine {
    state: DockViewportLifecycleState,
    facts_generation: u64,
}

impl Default for DockViewportLifecycleMachine {
    fn default() -> Self {
        Self {
            state: DockViewportLifecycleState::RegisteredNotReady,
            facts_generation: 0,
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
    /// Current platform input mask.
    pub(crate) input_mask: DockViewportInputMask,
}

/// Coordinate frame for a live viewport window rectangle.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum DockViewportWindowBoundsFrame {
    /// Bounds are in a shared desktop coordinate space and may provide global hit testing.
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

/// Current platform input mask for a viewport window.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DockViewportInputMask {
    /// The window receives pointer input and may be selected by hovered-window hit testing.
    ReceivesInput,
    /// The window is minimized and must not select a route or no-input underlay routing.
    Minimized,
    /// The window is explicitly click-through, so hovered-window hit testing should skip it.
    NoInputPassThrough,
}

impl DockViewportInputMask {
    pub(crate) fn drag_restore_accepts_pointer_input(self) -> bool {
        match self {
            Self::ReceivesInput | Self::Minimized => true,
            Self::NoInputPassThrough => false,
        }
    }

    pub(crate) fn participates_in_hover_hit_testing(self) -> bool {
        matches!(self, Self::ReceivesInput)
    }

    fn is_minimized(self) -> bool {
        matches!(self, Self::Minimized)
    }
}

/// Platform-window requests reported by the backend and not yet consumed by a fresh host scene.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct DockViewportPlatformRequests {
    /// The platform has reported a close request that is waiting for a close callback or explicit
    /// cancellation.
    pub(crate) close_requested: bool,
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
            input_mask: DockViewportInputMask::ReceivesInput,
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
            facts.input_mask = DockViewportInputMask::Minimized;
        } else if !window.accepts_pointer_input() {
            facts.input_mask = DockViewportInputMask::NoInputPassThrough;
        }
        facts
    }

    #[cfg(test)]
    pub(crate) fn with_input_mask(mut self, input_mask: DockViewportInputMask) -> Self {
        self.input_mask = input_mask;
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
    /// Last known platform input mask.
    pub(crate) input_mask: DockViewportInputMask,
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
            input_mask: DockViewportInputMask::Minimized,
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
        if self.platform_requests.close_requested {
            return Some(DockViewportRouteUnavailableReason::PlatformCloseRequested);
        }
        self.route_facts_unavailable_reason()
    }

    pub(crate) fn route_facts_unavailable_reason(
        &self,
    ) -> Option<DockViewportRouteUnavailableReason> {
        if let Some(reason) = self.lifecycle.route_unavailable_reason() {
            return Some(reason);
        }
        if self.has_missing_route_facts() {
            return Some(DockViewportRouteUnavailableReason::MissingRouteFacts);
        }
        if self.input_mask.is_minimized() {
            return Some(DockViewportRouteUnavailableReason::Minimized);
        }
        None
    }

    pub(crate) fn is_route_ready(&self) -> bool {
        self.route_unavailable_reason().is_none()
    }

    pub(crate) fn can_route_hover_hit(&self) -> bool {
        self.is_route_ready() && self.input_mask.participates_in_hover_hit_testing()
    }

    pub(crate) fn is_platform_close_requested(&self) -> bool {
        self.platform_requests.close_requested
    }

    pub(crate) fn facts_generation(&self) -> u64 {
        self.lifecycle.facts_generation()
    }

    pub(crate) fn platform_requests(&self) -> DockViewportPlatformRequests {
        self.platform_requests
    }

    pub(crate) fn global_screen_bounds(&self) -> Option<Bounds<Pixels>> {
        self.current_bounds?.global_screen_bounds()
    }

    pub(crate) fn facts_generation_if_current(&self, window_id: WindowId) -> Option<u64> {
        (self.window.window_id() == window_id && self.is_route_ready())
            .then(|| self.facts_generation())
    }

    pub(crate) fn update_route_facts(
        &mut self,
        window_facts: DockViewportWindowFacts,
        host_bounds: Bounds<Pixels>,
    ) -> bool {
        let display_id = window_facts.display_id;
        let window_bounds = Some(window_facts.window_bounds);
        let current_bounds = Some(window_facts.current_bounds);
        let host_bounds = Some(host_bounds);
        let input_mask = window_facts.input_mask;
        if self.lifecycle.is_route_ready()
            && self.display_id == display_id
            && self.window_bounds == window_bounds
            && self.current_bounds == current_bounds
            && self.host_bounds == host_bounds
        {
            let changed = self.input_mask != input_mask || self.platform_requests.resize_requested;
            self.input_mask = input_mask;
            self.platform_requests.resize_requested = false;
            return changed;
        }

        self.display_id = display_id;
        self.window_bounds = window_bounds;
        self.current_bounds = current_bounds;
        self.host_bounds = host_bounds;
        self.input_mask = input_mask;
        self.platform_requests.resize_requested = false;
        self.lifecycle.mark_route_ready();
        true
    }

    pub(crate) fn apply_platform_window_facts(
        &mut self,
        window_facts: DockViewportWindowFacts,
    ) -> bool {
        if self.platform_requests.close_requested {
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
            || self.input_mask.is_minimized()
            || window_facts.input_mask.is_minimized()
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
                close_requested: self.platform_requests.close_requested,
                resize_requested: true,
            };
        };

        DockViewportPlatformRequests {
            close_requested: self.platform_requests.close_requested,
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
        let input_mask = window_facts.input_mask;
        if self.display_id == display_id
            && self.window_bounds == window_bounds
            && self.current_bounds == current_bounds
            && self.input_mask == input_mask
        {
            return false;
        }

        self.display_id = display_id;
        self.window_bounds = window_bounds;
        self.current_bounds = current_bounds;
        self.input_mask = input_mask;
        true
    }

    pub(crate) fn refresh_input_mask(&mut self, input_mask: DockViewportInputMask) -> bool {
        if self.platform_requests.close_requested || self.input_mask == input_mask {
            return false;
        }

        self.input_mask = input_mask;
        true
    }

    pub(crate) fn mark_route_facts_stale(&mut self, reason: DockViewportStaleReason) -> bool {
        self.lifecycle.mark_stale(reason)
    }

    pub(crate) fn mark_platform_close_requested(&mut self) -> bool {
        if self.platform_requests.close_requested {
            return false;
        }
        self.platform_requests.close_requested = true;
        true
    }

    pub(crate) fn cancel_platform_close_request(&mut self) -> bool {
        if !self.platform_requests.close_requested {
            return false;
        }
        self.platform_requests.close_requested = false;
        true
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
    fn register_swaps_two_populated_spaces_and_clears_both_old_indexes() {
        let mut registry = DockViewportRegistry::default();
        let source_space = space("source");
        let target_space = space("target");
        let source_window = handle(1);
        let target_window = handle(2);

        assert!(
            registry
                .register(source_space.clone(), source_window)
                .is_none()
        );
        assert!(
            registry
                .register(target_space.clone(), target_window)
                .is_none()
        );

        let replaced = registry.register_with_replacements(target_space.clone(), source_window);
        assert_eq!(replaced.len(), 2);
        assert!(replaced.iter().any(|(space, snapshot)| {
            *space == target_space && snapshot.window == target_window
        }));
        assert!(replaced.iter().any(|(space, snapshot)| {
            *space == source_space && snapshot.window == source_window
        }));
        assert_eq!(registry.window_for_space(&source_space), None);
        assert_eq!(
            registry.window_for_space(&target_space),
            Some(source_window)
        );
        assert_eq!(
            registry.space_for_window_id(source_window.window_id()),
            Some(&target_space)
        );
        assert_eq!(
            registry.space_for_window_id(target_window.window_id()),
            None
        );
        assert_eq!(registry.spaces(), vec![target_space]);
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
                .with_input_mask(DockViewportInputMask::Minimized),
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
    fn no_input_window_facts_keep_route_facts_ready_but_are_hover_ineligible() {
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
                .with_input_mask(DockViewportInputMask::NoInputPassThrough),
                Bounds::new(
                    open_gpui::point(open_gpui::px(0.0), open_gpui::px(0.0)),
                    open_gpui::size(open_gpui::px(320.0), open_gpui::px(240.0)),
                ),
            )
        );

        assert!(snapshot.is_route_ready());
        assert!(!snapshot.can_route_hover_hit());
        assert_eq!(snapshot.route_unavailable_reason(), None);
    }

    #[test]
    fn platform_close_request_blocks_route_without_staling_route_facts() {
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
        let generation = snapshot.facts_generation();

        assert!(snapshot.mark_platform_close_requested());
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
        assert_eq!(snapshot.facts_generation(), generation + 1);
        assert_eq!(
            snapshot.lifecycle_state(),
            DockViewportLifecycleState::RouteReady
        );
        assert_eq!(
            snapshot.route_facts_unavailable_reason(),
            None,
            "close requests are platform request flags, not stale route facts"
        );
        assert_eq!(
            snapshot.route_unavailable_reason(),
            Some(DockViewportRouteUnavailableReason::PlatformCloseRequested)
        );
        assert!(!snapshot.is_route_ready());
    }

    #[test]
    fn platform_close_request_requires_explicit_cancellation() {
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
        let generation = snapshot.facts_generation();
        assert!(snapshot.mark_platform_close_requested());
        assert_eq!(snapshot.facts_generation(), generation);
        assert!(!snapshot.is_route_ready());

        assert!(snapshot.cancel_platform_close_request());
        assert_eq!(
            snapshot.lifecycle_state(),
            DockViewportLifecycleState::RouteReady
        );
        assert!(snapshot.is_route_ready());
        assert_eq!(snapshot.facts_generation(), generation);
        assert!(!snapshot.cancel_platform_close_request());
    }
}
