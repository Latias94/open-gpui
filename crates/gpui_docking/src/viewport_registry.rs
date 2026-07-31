use crate::{
    DockSpaceId, DockViewportHostGeometry, DockViewportIdentity, DockViewportRuntimeLineage,
};
use open_gpui::{
    AnyWindowHandle, App, Bounds, DisplayId, Pixels, Window, WindowBounds, WindowCoordinateSpace,
    WindowId, WindowPlatformFacts,
};
use std::collections::{BTreeMap, HashMap};

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) struct DockViewportRegistrationConflict {
    incumbent: DockViewportRuntimeLineage,
    requested: DockViewportRuntimeLineage,
}

impl DockViewportRegistrationConflict {
    fn lineage(
        incumbent: DockViewportRuntimeLineage,
        requested: DockViewportRuntimeLineage,
    ) -> Self {
        Self {
            incumbent,
            requested,
        }
    }
}

/// Stable token for one exact logical-space-to-window registration.
///
/// Route-facts generations describe coordinate freshness and may advance many times while a
/// registration stays alive. This separate generation prevents delayed runtime effects from
/// finalizing against a replacement that happens to reuse the same space or window id.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DockViewportRegistrationKey {
    space: DockSpaceId,
    window_id: WindowId,
    generation: u64,
    lineage: DockViewportRuntimeLineage,
}

impl DockViewportRegistrationKey {
    fn new(
        space: DockSpaceId,
        window_id: WindowId,
        generation: u64,
        lineage: DockViewportRuntimeLineage,
    ) -> Self {
        Self {
            space,
            window_id,
            generation,
            lineage,
        }
    }

    #[cfg(test)]
    pub(crate) fn for_test(space: DockSpaceId, window_id: WindowId) -> Self {
        Self::new(space, window_id, 0, DockViewportRuntimeLineage::Unmanaged)
    }

    #[cfg(test)]
    pub(crate) fn for_test_generation(
        space: DockSpaceId,
        window_id: WindowId,
        generation: u64,
    ) -> Self {
        Self::new(
            space,
            window_id,
            generation,
            DockViewportRuntimeLineage::Unmanaged,
        )
    }

    pub(crate) fn space(&self) -> &DockSpaceId {
        &self.space
    }

    pub(crate) fn window_id(&self) -> WindowId {
        self.window_id
    }

    pub(crate) const fn lineage(&self) -> DockViewportRuntimeLineage {
        self.lineage
    }
}

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

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct DockViewportWindowFactsChange {
    pub(crate) changed: bool,
    pub(crate) placement_changed: bool,
}

/// Coordinate frame for a live viewport window rectangle.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum DockViewportWindowBoundsFrame {
    /// Bounds are in a shared desktop coordinate space and may provide global hit testing.
    GlobalScreen(Bounds<Pixels>),
    /// Bounds are only meaningful in the receiver window's local coordinate space.
    WindowLocal(Bounds<Pixels>),
}

/// Coordinate space backing the latest viewport route facts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DockViewportCoordinateSpace {
    /// Current bounds are in a shared desktop coordinate space.
    GlobalScreen,
    /// Current bounds are only meaningful in the receiver window's local coordinate space.
    WindowLocal,
}

/// Latest coordinate facts published for a registered viewport.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct DockViewportCoordinateSnapshot {
    /// Display currently containing the window.
    pub(crate) display_id: Option<DisplayId>,
    /// Platform window state suitable for placement persistence.
    pub(crate) window_bounds: WindowBounds,
    /// Current window rectangle in the backend-reported coordinate space.
    pub(crate) current_bounds: Bounds<Pixels>,
    /// Coordinate space backing `current_bounds`.
    pub(crate) coordinate_space: DockViewportCoordinateSpace,
    /// Current dock host mapping between layout, host-local, and window coordinates.
    pub(crate) host_geometry: DockViewportHostGeometry,
    /// Route-facts generation that owns these coordinate facts.
    pub(crate) facts_generation: u64,
}

impl DockViewportWindowBoundsFrame {
    pub(crate) fn global_screen_bounds(self) -> Option<Bounds<Pixels>> {
        match self {
            Self::GlobalScreen(bounds) => Some(bounds),
            Self::WindowLocal(_) => None,
        }
    }

    pub(crate) fn bounds(self) -> Bounds<Pixels> {
        match self {
            Self::GlobalScreen(bounds) | Self::WindowLocal(bounds) => bounds,
        }
    }

    pub(crate) fn coordinate_space(self) -> DockViewportCoordinateSpace {
        match self {
            Self::GlobalScreen(_) => DockViewportCoordinateSpace::GlobalScreen,
            Self::WindowLocal(_) => DockViewportCoordinateSpace::WindowLocal,
        }
    }

    pub(crate) fn size(self) -> open_gpui::Size<Pixels> {
        self.bounds().size
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

    pub(crate) fn from_window(window: &Window, _cx: &App) -> Self {
        Self::from_platform_facts(window.platform_facts())
    }

    /// Converts one GPUI-committed platform-facts snapshot into Dock route and placement facts.
    ///
    /// Dock must not reconstruct these values from backend-facing `Window` getters because a
    /// queued live mutation is not an observed platform fact.
    pub(crate) fn from_platform_facts(platform_facts: &WindowPlatformFacts) -> Self {
        let current_bounds = match platform_facts.coordinate_space {
            WindowCoordinateSpace::GlobalScreen => {
                DockViewportWindowBoundsFrame::GlobalScreen(platform_facts.bounds)
            }
            WindowCoordinateSpace::WindowLocal => {
                DockViewportWindowBoundsFrame::WindowLocal(platform_facts.bounds)
            }
        };
        let mut facts = Self::with_current_bounds(
            platform_facts.display_id,
            platform_facts.window_bounds,
            current_bounds,
        );
        if platform_facts.is_minimized {
            facts.input_mask = DockViewportInputMask::Minimized;
        } else if !platform_facts.accepts_pointer_input {
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
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct DockViewportSnapshot {
    /// GPUI window currently rendering the logical dock space.
    pub(crate) window: AnyWindowHandle,
    /// Display containing the window, when the application has recorded one.
    pub(crate) display_id: Option<DisplayId>,
    /// Last known platform window state used for placement persistence.
    pub(crate) window_bounds: Option<WindowBounds>,
    /// Last known current window rectangle with its backend coordinate frame.
    pub(crate) current_bounds: Option<DockViewportWindowBoundsFrame>,
    /// Last known committed dock host geometry.
    pub(crate) host_geometry: Option<DockViewportHostGeometry>,
    /// Last known platform input mask.
    pub(crate) input_mask: DockViewportInputMask,
    registration_generation: u64,
    lineage: DockViewportRuntimeLineage,
    platform_requests: DockViewportPlatformRequests,
    lifecycle: DockViewportLifecycleMachine,
}

impl DockViewportSnapshot {
    /// Creates a snapshot for a newly registered viewport window.
    #[cfg(test)]
    pub(crate) fn new(window: AnyWindowHandle) -> Self {
        Self::with_registration_generation(window, 0, DockViewportRuntimeLineage::Unmanaged)
    }

    fn with_registration_generation(
        window: AnyWindowHandle,
        registration_generation: u64,
        lineage: DockViewportRuntimeLineage,
    ) -> Self {
        Self {
            window,
            display_id: None,
            window_bounds: None,
            current_bounds: None,
            host_geometry: None,
            input_mask: DockViewportInputMask::Minimized,
            registration_generation,
            lineage,
            platform_requests: DockViewportPlatformRequests::default(),
            lifecycle: DockViewportLifecycleMachine::default(),
        }
    }

    pub(crate) fn registration_key(&self, space: &DockSpaceId) -> DockViewportRegistrationKey {
        DockViewportRegistrationKey::new(
            space.clone(),
            self.window.window_id(),
            self.registration_generation,
            self.lineage,
        )
    }

    pub(crate) const fn lineage(&self) -> DockViewportRuntimeLineage {
        self.lineage
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

    pub(crate) fn coordinate_snapshot(&self) -> Option<DockViewportCoordinateSnapshot> {
        let window_bounds = self.window_bounds?;
        let current_bounds = self.current_bounds?;
        let host_geometry = self.host_geometry.clone()?;
        Some(DockViewportCoordinateSnapshot {
            display_id: self.display_id,
            window_bounds,
            current_bounds: current_bounds.bounds(),
            coordinate_space: current_bounds.coordinate_space(),
            host_geometry,
            facts_generation: self.facts_generation(),
        })
    }

    pub(crate) fn facts_generation_if_current(&self, window_id: WindowId) -> Option<u64> {
        (self.window.window_id() == window_id && self.is_route_ready())
            .then(|| self.facts_generation())
    }

    #[cfg(test)]
    pub(crate) fn update_route_facts(
        &mut self,
        window_facts: DockViewportWindowFacts,
        host_geometry: impl Into<DockViewportHostGeometry>,
    ) -> bool {
        self.update_route_facts_with_change(window_facts, host_geometry)
            .changed
    }

    pub(crate) fn update_route_facts_with_change(
        &mut self,
        window_facts: DockViewportWindowFacts,
        host_geometry: impl Into<DockViewportHostGeometry>,
    ) -> DockViewportWindowFactsChange {
        let host_geometry = Some(host_geometry.into());
        let display_id = window_facts.display_id;
        let window_bounds = Some(window_facts.window_bounds);
        let current_bounds = Some(window_facts.current_bounds);
        let input_mask = window_facts.input_mask;
        let host_geometry_unchanged = self
            .host_geometry
            .as_ref()
            .zip(host_geometry.as_ref())
            .is_some_and(|(current, next)| current.has_same_native_routing_geometry(next));
        let placement_changed = self.display_id != display_id
            || self.window_bounds != window_bounds
            || self
                .host_geometry
                .as_ref()
                .map(|geometry| geometry.layout_bounds())
                != host_geometry
                    .as_ref()
                    .map(|geometry| geometry.layout_bounds());
        if self.lifecycle.is_route_ready()
            && self.display_id == display_id
            && self.window_bounds == window_bounds
            && self.current_bounds == current_bounds
            && host_geometry_unchanged
        {
            let changed = self.input_mask != input_mask || self.platform_requests.resize_requested;
            self.host_geometry = host_geometry;
            self.input_mask = input_mask;
            self.platform_requests.resize_requested = false;
            return DockViewportWindowFactsChange {
                changed,
                placement_changed,
            };
        }

        self.display_id = display_id;
        self.window_bounds = window_bounds;
        self.current_bounds = current_bounds;
        self.host_geometry = host_geometry;
        self.input_mask = input_mask;
        self.platform_requests.resize_requested = false;
        self.lifecycle.mark_route_ready();
        DockViewportWindowFactsChange {
            changed: true,
            placement_changed,
        }
    }

    pub(crate) fn apply_platform_window_facts_with_change(
        &mut self,
        window_facts: DockViewportWindowFacts,
    ) -> DockViewportWindowFactsChange {
        if self.platform_requests.close_requested {
            return DockViewportWindowFactsChange::default();
        }

        self.platform_requests = self.platform_requests_after_window_facts(window_facts);
        let placement_changed = self.serialized_window_placement_facts_differ(window_facts);

        if self.can_preserve_route_facts_for_platform_move(window_facts) {
            return DockViewportWindowFactsChange {
                changed: self.replace_window_facts_without_generation(window_facts),
                placement_changed,
            };
        }

        let changed = self.replace_window_facts_without_generation(window_facts);
        DockViewportWindowFactsChange {
            changed: self
                .lifecycle
                .mark_stale(DockViewportStaleReason::WindowFactsChanged)
                || changed,
            placement_changed,
        }
    }

    fn serialized_window_placement_facts_differ(
        &self,
        window_facts: DockViewportWindowFacts,
    ) -> bool {
        self.display_id != window_facts.display_id
            || self.window_bounds != Some(window_facts.window_bounds)
    }

    fn can_preserve_route_facts_for_platform_move(
        &self,
        window_facts: DockViewportWindowFacts,
    ) -> bool {
        if !self.lifecycle.is_route_ready()
            || self.host_geometry.is_none()
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
        self.window_bounds.is_none()
            || self.current_bounds.is_none()
            || self.host_geometry.is_none()
    }
}

/// Internal one-to-one mapping between logical dock spaces and GPUI windows.
#[derive(Debug)]
pub(crate) struct DockViewportRegistry {
    viewports: BTreeMap<DockSpaceId, DockViewportSnapshot>,
    windows: HashMap<WindowId, DockSpaceId>,
    last_registration_generation_by_space: BTreeMap<DockSpaceId, u64>,
}

impl Default for DockViewportRegistry {
    fn default() -> Self {
        Self {
            viewports: BTreeMap::new(),
            windows: HashMap::new(),
            last_registration_generation_by_space: BTreeMap::new(),
        }
    }
}

impl DockViewportRegistry {
    #[cfg(test)]
    pub(crate) fn register(
        &mut self,
        space: DockSpaceId,
        window: AnyWindowHandle,
    ) -> Option<DockViewportSnapshot> {
        self.register_with_replacements(
            space.clone(),
            window,
            DockViewportRuntimeLineage::Unmanaged,
        )
        .expect("unmanaged test registrations cannot conflict by lineage")
        .into_iter()
        .find(|(removed_space, _)| *removed_space == space)
        .map(|(_, snapshot)| snapshot)
    }

    pub(crate) fn register_with_replacements(
        &mut self,
        space: DockSpaceId,
        window: AnyWindowHandle,
        lineage: DockViewportRuntimeLineage,
    ) -> Result<Vec<(DockSpaceId, DockViewportSnapshot)>, DockViewportRegistrationConflict> {
        let window_id = window.window_id();
        if let Some(snapshot) = self.viewports.get(&space)
            && snapshot.lineage != lineage
        {
            return Err(DockViewportRegistrationConflict::lineage(
                snapshot.lineage,
                lineage,
            ));
        }
        if let Some(previous_space) = self.windows.get(&window_id)
            && let Some(snapshot) = self.viewports.get(previous_space)
            && snapshot.lineage != lineage
        {
            return Err(DockViewportRegistrationConflict::lineage(
                snapshot.lineage,
                lineage,
            ));
        }
        if let Some(snapshot) = self.viewports.get(&space)
            && snapshot.identity(&space).matches(&space, window_id)
            && snapshot.lineage == lineage
            && self
                .windows
                .get(&window_id)
                .is_none_or(|registered_space| registered_space == &space)
        {
            self.windows.insert(window_id, space);
            return Ok(Vec::new());
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

        let registration_generation = self
            .last_registration_generation_by_space
            .get(&space)
            .copied()
            .unwrap_or(0)
            .checked_add(1)
            .expect("dock viewport registration generation space exhausted");
        self.windows.insert(window_id, space.clone());
        self.last_registration_generation_by_space
            .insert(space.clone(), registration_generation);
        self.viewports.insert(
            space,
            DockViewportSnapshot::with_registration_generation(
                window,
                registration_generation,
                lineage,
            ),
        );
        Ok(replaced)
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

    pub(crate) fn registration_key(
        &self,
        space: &DockSpaceId,
    ) -> Option<DockViewportRegistrationKey> {
        self.viewports
            .get(space)
            .map(|snapshot| snapshot.registration_key(space))
    }

    pub(crate) fn last_registration_generation(&self, space: &DockSpaceId) -> Option<u64> {
        self.last_registration_generation_by_space
            .get(space)
            .copied()
    }

    pub(crate) fn is_current_registration(&self, key: &DockViewportRegistrationKey) -> bool {
        self.viewports.get(key.space()).is_some_and(|snapshot| {
            snapshot.registration_generation == key.generation
                && snapshot.window.window_id() == key.window_id()
                && snapshot.lineage == key.lineage()
        }) && self
            .windows
            .get(&key.window_id())
            .is_some_and(|space| space == key.space())
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
    use crate::surface::window_session::DockSurfaceWindowSession;
    use crate::viewport_test_support::{handle, space};
    use open_gpui::EntityId;

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
    fn registration_generation_rejects_recreated_binding_with_same_identity() {
        let mut registry = DockViewportRegistry::default();
        let main = space("main");
        let window = handle(1);

        registry.register(main.clone(), window);
        let first = registry
            .registration_key(&main)
            .expect("first binding should have a registration key");
        assert!(registry.is_current_registration(&first));

        registry.unregister_space(&main);
        registry.register(main.clone(), window);
        let replacement = registry
            .registration_key(&main)
            .expect("replacement binding should have a registration key");

        assert_ne!(replacement, first);
        assert!(!registry.is_current_registration(&first));
        assert!(registry.is_current_registration(&replacement));
    }

    #[test]
    fn cross_lineage_registration_is_rejected_without_mutating_the_incumbent() {
        let mut registry = DockViewportRegistry::default();
        let main = space("main");
        let window = handle(1);
        registry.register(main.clone(), window);
        let incumbent = registry
            .registration_key(&main)
            .expect("incumbent registration should exist");

        let mut session = DockSurfaceWindowSession::new(EntityId::from(91));
        let opening = session.reserve_opening().expect("surface should reserve");
        let lease = session
            .commit_opening(opening, WindowId::from(99))
            .expect("surface should activate");

        assert!(
            registry
                .register_with_replacements(
                    main.clone(),
                    window,
                    DockViewportRuntimeLineage::Surface(lease),
                )
                .is_err()
        );
        assert_eq!(registry.registration_key(&main), Some(incumbent));
        assert_eq!(registry.window_for_space(&main), Some(window));
        assert_eq!(
            registry.space_for_window_id(window.window_id()),
            Some(&main)
        );
    }

    #[test]
    fn registration_generations_are_scoped_to_each_space_lineage() {
        let mut registry = DockViewportRegistry::default();
        let main = space("main");
        let secondary = space("secondary");

        registry.register(main.clone(), handle(1));
        registry.register(secondary.clone(), handle(2));

        assert_eq!(
            registry
                .registration_key(&main)
                .expect("main binding should have a registration key")
                .generation,
            1
        );
        assert_eq!(
            registry
                .registration_key(&secondary)
                .expect("secondary binding should have a registration key")
                .generation,
            1
        );
    }

    #[test]
    fn idempotent_registration_preserves_registration_generation() {
        let mut registry = DockViewportRegistry::default();
        let main = space("main");
        let window = handle(1);

        registry.register(main.clone(), window);
        let first = registry
            .registration_key(&main)
            .expect("binding should have a registration key");
        assert!(
            registry
                .register_with_replacements(
                    main.clone(),
                    window,
                    DockViewportRuntimeLineage::Unmanaged,
                )
                .expect("unmanaged idempotent registration cannot conflict by lineage")
                .is_empty()
        );

        assert_eq!(registry.registration_key(&main), Some(first));
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

        let replaced = registry
            .register_with_replacements(
                target_space.clone(),
                source_window,
                DockViewportRuntimeLineage::Unmanaged,
            )
            .expect("unmanaged replacement cannot conflict by lineage");
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
    fn route_only_scene_facts_do_not_report_persistent_placement_changes() {
        let mut registry = DockViewportRegistry::default();
        let main = space("main");
        registry.register(main.clone(), handle(1));
        let window_bounds = Bounds::new(
            open_gpui::point(open_gpui::px(100.0), open_gpui::px(100.0)),
            open_gpui::size(open_gpui::px(320.0), open_gpui::px(240.0)),
        );
        let host_bounds = Bounds::new(
            open_gpui::point(open_gpui::px(0.0), open_gpui::px(0.0)),
            open_gpui::size(open_gpui::px(320.0), open_gpui::px(240.0)),
        );
        let facts =
            DockViewportWindowFacts::from_window_bounds(WindowBounds::Windowed(window_bounds));
        let snapshot = registry
            .snapshot_mut(&main)
            .expect("registered viewport should have a mutable snapshot");

        let initial = snapshot.update_route_facts_with_change(
            facts,
            DockViewportHostGeometry::identity_with_hit_region_for_test(host_bounds, host_bounds),
        );
        assert!(initial.changed);
        assert!(initial.placement_changed);
        let initial_generation = snapshot.facts_generation();

        let semantically_identical = snapshot.update_route_facts_with_change(
            facts,
            DockViewportHostGeometry::identity_with_hit_region_for_test(host_bounds, host_bounds),
        );
        assert!(!semantically_identical.changed);
        assert!(!semantically_identical.placement_changed);
        assert_eq!(snapshot.facts_generation(), initial_generation);

        let smaller_hit_region = Bounds::new(
            open_gpui::point(open_gpui::px(8.0), open_gpui::px(8.0)),
            open_gpui::size(open_gpui::px(304.0), open_gpui::px(224.0)),
        );
        let hit_region_only = snapshot.update_route_facts_with_change(
            facts,
            DockViewportHostGeometry::identity_with_hit_region_for_test(
                host_bounds,
                smaller_hit_region,
            ),
        );
        assert!(hit_region_only.changed);
        assert_eq!(snapshot.facts_generation(), initial_generation + 1);
        assert!(
            !hit_region_only.placement_changed,
            "the serialized host bounds did not change"
        );

        let mut current_bounds_only = facts;
        current_bounds_only.current_bounds =
            DockViewportWindowBoundsFrame::WindowLocal(Bounds::new(
                open_gpui::point(open_gpui::px(4.0), open_gpui::px(6.0)),
                window_bounds.size,
            ));
        let route_frame_only = snapshot.update_route_facts_with_change(
            current_bounds_only,
            DockViewportHostGeometry::identity_with_hit_region_for_test(
                host_bounds,
                smaller_hit_region,
            ),
        );
        assert!(route_frame_only.changed);
        assert!(
            !route_frame_only.placement_changed,
            "route-coordinate freshness is not part of placement serialization"
        );

        let mut observed_route_frame_only = current_bounds_only;
        observed_route_frame_only.current_bounds =
            DockViewportWindowBoundsFrame::WindowLocal(Bounds::new(
                open_gpui::point(open_gpui::px(12.0), open_gpui::px(16.0)),
                window_bounds.size,
            ));
        let platform_route_frame_only =
            snapshot.apply_platform_window_facts_with_change(observed_route_frame_only);
        assert!(platform_route_frame_only.changed);
        assert!(
            !platform_route_frame_only.placement_changed,
            "platform route coordinates are not part of placement serialization"
        );
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
