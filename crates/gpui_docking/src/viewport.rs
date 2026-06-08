use crate::{
    DockController, DockHost, DockItemId, DockNodeId, DockPolicy, DockPolicyError, DockSpaceId,
    DockViewportCloseOutcome, DockViewportClosePolicy, DockViewportPlacementLayout,
    DockViewportPlacementValidationError, DockViewportRestoreOutcome,
    DockViewportShouldCloseOutcome,
    viewport_registry::{DockViewportRegistry, DockViewportSnapshot},
    viewport_target::{
        DockViewportHit, DockViewportHitCandidate, DockViewportTargetContext,
        resolve_viewport_target,
    },
};
use open_gpui::{
    AnyWindowHandle, App, AppContext as _, Entity, Pixels, Point, Result, Subscription, Window,
    WindowBounds, WindowId, WindowOptions,
};
#[cfg(test)]
use open_gpui::{Bounds, DisplayId, point};
use std::{
    cell::{Cell, Ref, RefCell},
    rc::Rc,
};

/// Runtime adapter state that maps logical dock spaces to GPUI windows.
///
/// This type owns platform-window facts for docking: window handles, display ids, and the latest
/// bounds snapshots used for coordinate conversion. None of this state belongs in
/// [`DockGraph`](crate::DockGraph) or [`DockLayout`](crate::DockLayout).
///
/// A typical restore flow imports [`DockLayout`](crate::DockLayout) into a controller, opens or
/// reuses GPUI windows for each logical dock space, registers those windows here, then applies a
/// [`DockViewportPlacementLayout`] to rehydrate placement snapshots for coordinate conversion.
#[derive(Debug, Default)]
pub struct DockViewportAdapter {
    registry: DockViewportRegistry,
}

/// Request to open a new platform viewport for a tab released outside known dock viewports.
#[derive(Debug, Clone, PartialEq)]
pub struct DockViewportTearOffRequest {
    /// Source dock space containing the dragged item.
    pub source_space: DockSpaceId,
    /// Source tabs node where the drag started.
    pub source_tabs: DockNodeId,
    /// Item being torn off.
    pub item: DockItemId,
    /// Release position in screen coordinates.
    pub release_position: Point<Pixels>,
    /// Suggested platform window bounds for the new viewport, when known.
    pub suggested_window_bounds: Option<WindowBounds>,
}

/// Result of resolving a tab release against registered platform viewports.
#[derive(Debug, Clone, PartialEq)]
pub enum DockViewportTearOffOutcome {
    /// The release landed inside a known viewport; normal drop handling should continue.
    KnownViewport(DockViewportHit),
    /// The release can open a new platform viewport.
    Requested(DockViewportTearOffRequest),
    /// The request was rejected by docking policy.
    Rejected(DockPolicyError),
}

/// Runtime result of opening or reopening a platform viewport.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DockViewportOpenOutcome {
    /// Logical dock space rendered by the window.
    pub space: DockSpaceId,
    /// GPUI window that renders the logical dock space.
    pub window: AnyWindowHandle,
    /// Whether the runtime opened, reused, or replaced a window.
    pub status: DockViewportOpenStatus,
}

/// How an open or reopen request resolved.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DockViewportOpenStatus {
    /// A new GPUI window was opened and registered.
    Opened,
    /// An existing live GPUI window was reused.
    Reused,
    /// A stale or superseded mapping was replaced by a new window.
    Replaced,
}

/// Owner for controller-backed platform viewport lifecycle.
///
/// The runtime keeps the shared [`DockController`] together with the low-level
/// [`DockViewportAdapter`] so applications do not have to pass the controller into every open call
/// or duplicate close-callback cleanup logic. The adapter remains the place for window mappings,
/// coordinate snapshots, and placement import/export.
pub struct DockViewportRuntime {
    controller: Entity<DockController>,
    adapter: DockViewportAdapter,
    close_policy: Rc<Cell<DockViewportClosePolicy>>,
}

impl DockViewportRuntime {
    /// Creates a runtime with the default close policy.
    pub fn new(controller: Entity<DockController>) -> Self {
        Self::with_close_policy(controller, DockViewportClosePolicy::default())
    }

    /// Creates a runtime with an explicit close policy.
    pub fn with_close_policy(
        controller: Entity<DockController>,
        close_policy: DockViewportClosePolicy,
    ) -> Self {
        Self {
            controller,
            adapter: DockViewportAdapter::new(),
            close_policy: Rc::new(Cell::new(close_policy)),
        }
    }

    /// Creates a runtime from an existing adapter.
    pub fn from_adapter(
        controller: Entity<DockController>,
        adapter: DockViewportAdapter,
        close_policy: DockViewportClosePolicy,
    ) -> Self {
        Self {
            controller,
            adapter,
            close_policy: Rc::new(Cell::new(close_policy)),
        }
    }

    /// Wraps this runtime in a cloneable handle for GPUI application callbacks.
    pub fn into_handle(self) -> DockViewportRuntimeHandle {
        DockViewportRuntimeHandle::from_runtime(self)
    }

    /// Returns the shared docking controller.
    pub fn controller(&self) -> &Entity<DockController> {
        &self.controller
    }

    /// Returns the low-level viewport adapter.
    pub fn adapter(&self) -> &DockViewportAdapter {
        &self.adapter
    }

    /// Returns the close policy used by [`handle_window_should_close`](Self::handle_window_should_close).
    pub fn close_policy(&self) -> DockViewportClosePolicy {
        self.close_policy.get()
    }

    /// Replaces the close policy used by [`handle_window_should_close`](Self::handle_window_should_close).
    pub fn set_close_policy(&mut self, close_policy: DockViewportClosePolicy) {
        self.close_policy.set(close_policy);
    }

    /// Opens or reuses a controller-backed viewport window for a logical dock space.
    ///
    /// Runtime-opened windows install a GPUI should-close hook so
    /// [`DockViewportClosePolicy::Prevent`] can veto a platform close before
    /// [`Self::handle_window_closed`] performs post-close cleanup.
    pub fn open_viewport(
        &mut self,
        space: impl Into<DockSpaceId>,
        options: WindowOptions,
        cx: &mut App,
    ) -> Result<DockViewportOpenOutcome> {
        let close_policy = self.close_policy.clone();
        self.open_viewport_with_should_close(space, options, cx, move |_| {
            close_policy.get() != DockViewportClosePolicy::Prevent
        })
    }

    /// Handles a GPUI window-closed notification by removing stale runtime mapping.
    ///
    /// Close policy is applied by [`Self::handle_window_should_close`] before GPUI accepts a close.
    /// Once a closed notification arrives, the platform window is already gone and docking must
    /// discard the runtime mapping even when the current policy is [`DockViewportClosePolicy::Prevent`].
    pub fn handle_window_closed(&mut self, window_id: WindowId) -> DockViewportCloseOutcome {
        self.adapter.handle_window_closed(window_id)
    }

    /// Handles a GPUI window should-close query by applying this runtime's close policy.
    pub fn handle_window_should_close(
        &self,
        window_id: WindowId,
    ) -> DockViewportShouldCloseOutcome {
        self.adapter
            .should_close_viewport(window_id, self.close_policy())
    }

    fn open_viewport_with_should_close(
        &mut self,
        space: impl Into<DockSpaceId>,
        options: WindowOptions,
        cx: &mut App,
        should_close: impl Fn(WindowId) -> bool + 'static,
    ) -> Result<DockViewportOpenOutcome> {
        self.adapter.open_viewport_with_window_setup(
            self.controller.clone(),
            space,
            options,
            cx,
            move |window, cx| {
                let window_id = window.window_handle().window_id();
                window.on_window_should_close(cx, move |_, _| should_close(window_id));
            },
        )
    }

    /// Exports serializable placement snapshots from the adapter.
    pub fn export_placement(&self) -> DockViewportPlacementLayout {
        self.adapter.export_placement()
    }

    /// Applies saved placement snapshots to registered viewport windows.
    pub fn apply_placement(
        &mut self,
        placement: &DockViewportPlacementLayout,
    ) -> Result<DockViewportRestoreOutcome, DockViewportPlacementValidationError> {
        self.adapter.apply_placement(placement)
    }
}

/// Cloneable application handle for a shared [`DockViewportRuntime`].
///
/// GPUI application-level callbacks such as [`App::on_window_closed`] require `'static` closures.
/// This handle hides the required interior mutability while keeping the runtime itself testable as
/// a normal Rust value.
#[derive(Clone)]
pub struct DockViewportRuntimeHandle {
    runtime: Rc<RefCell<DockViewportRuntime>>,
}

impl DockViewportRuntimeHandle {
    /// Creates a handle around a runtime with the default close policy.
    pub fn new(controller: Entity<DockController>) -> Self {
        DockViewportRuntime::new(controller).into_handle()
    }

    /// Creates a handle around a runtime with an explicit close policy.
    pub fn with_close_policy(
        controller: Entity<DockController>,
        close_policy: DockViewportClosePolicy,
    ) -> Self {
        DockViewportRuntime::with_close_policy(controller, close_policy).into_handle()
    }

    /// Creates a handle from a prepared runtime.
    pub fn from_runtime(runtime: DockViewportRuntime) -> Self {
        Self {
            runtime: Rc::new(RefCell::new(runtime)),
        }
    }

    /// Borrows the shared runtime.
    pub fn borrow(&self) -> Ref<'_, DockViewportRuntime> {
        self.runtime.borrow()
    }

    /// Returns the shared close policy used by runtime-opened viewport windows.
    pub fn close_policy(&self) -> DockViewportClosePolicy {
        self.runtime.borrow().close_policy()
    }

    /// Replaces the shared close policy used by runtime-opened viewport windows.
    pub fn set_close_policy(&self, close_policy: DockViewportClosePolicy) {
        self.runtime.borrow_mut().set_close_policy(close_policy);
    }

    /// Opens or reuses a controller-backed viewport window for a logical dock space.
    ///
    /// The handle installs a should-close hook that consults the shared runtime at close time, so
    /// later close-policy changes are observed by already-open windows.
    pub fn open_viewport(
        &self,
        space: impl Into<DockSpaceId>,
        options: WindowOptions,
        cx: &mut App,
    ) -> Result<DockViewportOpenOutcome> {
        let runtime = self.clone();
        self.runtime.borrow_mut().open_viewport_with_should_close(
            space,
            options,
            cx,
            move |window_id| runtime.handle_window_should_close(window_id).allows_close(),
        )
    }

    /// Handles a GPUI window-closed notification through the shared runtime.
    pub fn handle_window_closed(&self, window_id: WindowId) -> DockViewportCloseOutcome {
        self.runtime.borrow_mut().handle_window_closed(window_id)
    }

    /// Handles a GPUI window should-close query through the shared runtime.
    pub fn handle_window_should_close(
        &self,
        window_id: WindowId,
    ) -> DockViewportShouldCloseOutcome {
        self.runtime.borrow().handle_window_should_close(window_id)
    }

    /// Registers an application-level close observer that cleans up viewport mappings by
    /// [`WindowId`].
    ///
    /// This observer runs after a close has been accepted. It complements the should-close hook
    /// installed by [`Self::open_viewport`].
    ///
    /// Keep or detach the returned subscription according to the application's lifetime policy.
    pub fn observe_window_closed(&self, cx: &mut App) -> Subscription {
        let runtime = self.clone();
        cx.on_window_closed(move |_, window_id| {
            runtime.handle_window_closed(window_id);
        })
    }

    /// Exports serializable placement snapshots from the shared runtime.
    pub fn export_placement(&self) -> DockViewportPlacementLayout {
        self.runtime.borrow().export_placement()
    }

    /// Applies saved placement snapshots through the shared runtime.
    pub fn apply_placement(
        &self,
        placement: &DockViewportPlacementLayout,
    ) -> Result<DockViewportRestoreOutcome, DockViewportPlacementValidationError> {
        self.runtime.borrow_mut().apply_placement(placement)
    }
}

impl DockViewportAdapter {
    /// Creates an empty viewport adapter.
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns true when no viewport mappings are registered.
    pub fn is_empty(&self) -> bool {
        self.registry.is_empty()
    }

    /// Returns the number of registered logical viewports.
    pub fn len(&self) -> usize {
        self.registry.len()
    }

    /// Registers or replaces the window for a logical dock space.
    ///
    /// A window can belong to only one dock space at a time. Registering the same window for a
    /// different space removes its previous space mapping.
    pub fn register_viewport(
        &mut self,
        space: impl Into<DockSpaceId>,
        window: impl Into<AnyWindowHandle>,
    ) -> Option<DockViewportSnapshot> {
        let space = space.into();
        let window = window.into();
        self.registry.register(space, window)
    }

    /// Opens or reuses a GPUI window that renders a logical dock space.
    ///
    /// The returned window root is a controller-backed [`DockHost`]. If the dock space already has
    /// a live registered window, that window is activated and reused. If the existing mapping is
    /// stale, it is removed before opening a replacement window.
    pub fn open_viewport(
        &mut self,
        controller: Entity<DockController>,
        space: impl Into<DockSpaceId>,
        options: WindowOptions,
        cx: &mut App,
    ) -> Result<DockViewportOpenOutcome> {
        self.open_viewport_with_window_setup(controller, space, options, cx, |_, _| {})
    }

    fn open_viewport_with_window_setup(
        &mut self,
        controller: Entity<DockController>,
        space: impl Into<DockSpaceId>,
        options: WindowOptions,
        cx: &mut App,
        setup_window: impl FnOnce(&mut Window, &mut App) + 'static,
    ) -> Result<DockViewportOpenOutcome> {
        let space = space.into();
        let mut status = DockViewportOpenStatus::Opened;

        if let Some(window) = self.window_for_space(&space) {
            if window
                .update(cx, |_, window, _| window.activate_window())
                .is_ok()
            {
                return Ok(DockViewportOpenOutcome {
                    space,
                    window,
                    status: DockViewportOpenStatus::Reused,
                });
            }

            self.unregister_space(&space);
            status = DockViewportOpenStatus::Replaced;
        }

        let host_space = space.clone();
        let window = cx
            .open_window(options, move |window, cx| {
                setup_window(window, cx);
                cx.new(move |cx| DockHost::from_controller(controller, host_space, cx))
            })?
            .into();
        self.register_viewport(space.clone(), window);

        Ok(DockViewportOpenOutcome {
            space,
            window,
            status,
        })
    }

    /// Removes a viewport by logical dock space.
    pub fn unregister_space(&mut self, space: &DockSpaceId) -> Option<DockViewportSnapshot> {
        self.registry.unregister_space(space)
    }

    /// Removes a viewport by GPUI window handle.
    pub fn unregister_window(
        &mut self,
        window: impl Into<AnyWindowHandle>,
    ) -> Option<(DockSpaceId, DockViewportSnapshot)> {
        let window = window.into();
        self.registry.unregister_window(window)
    }

    /// Returns the snapshot for a logical dock space.
    pub fn snapshot(&self, space: &DockSpaceId) -> Option<&DockViewportSnapshot> {
        self.registry.snapshot(space)
    }

    pub(crate) fn snapshot_mut(
        &mut self,
        space: &DockSpaceId,
    ) -> Option<&mut DockViewportSnapshot> {
        self.registry.snapshot_mut(space)
    }

    pub(crate) fn unregister_window_id_snapshot(
        &mut self,
        window_id: WindowId,
    ) -> Option<(DockSpaceId, DockViewportSnapshot)> {
        self.registry.unregister_window_id(window_id)
    }

    /// Returns the window rendering a logical dock space.
    pub fn window_for_space(&self, space: &DockSpaceId) -> Option<AnyWindowHandle> {
        self.registry.window_for_space(space)
    }

    /// Returns the logical dock space rendered by a window.
    pub fn space_for_window(&self, window: impl Into<AnyWindowHandle>) -> Option<&DockSpaceId> {
        let window = window.into();
        self.space_for_window_id(window.window_id())
    }

    /// Returns the logical dock space rendered by a window id.
    pub fn space_for_window_id(&self, window_id: WindowId) -> Option<&DockSpaceId> {
        self.registry.space_for_window_id(window_id)
    }

    /// Returns known dock spaces in stable lexical order.
    pub fn spaces(&self) -> Vec<DockSpaceId> {
        self.registry.spaces()
    }

    #[cfg(test)]
    pub(crate) fn insert_stale_window_index_for_test(
        &mut self,
        window_id: WindowId,
        space: DockSpaceId,
    ) {
        self.registry
            .insert_stale_window_index_for_test(window_id, space);
    }

    /// Finds the registered viewport containing a screen point.
    pub fn hit_test_screen(&self, position: Point<Pixels>) -> Option<DockViewportHit> {
        self.hit_test_screen_with_context(position, &DockViewportTargetContext::new())
    }

    /// Finds the registered viewport containing a screen point using platform arbitration inputs.
    pub fn hit_test_screen_with_context(
        &self,
        position: Point<Pixels>,
        context: &DockViewportTargetContext,
    ) -> Option<DockViewportHit> {
        self.resolve_viewport_target(position, context)
            .map(DockViewportHitCandidate::into_hit)
    }

    /// Resolves a registered viewport target using explicit platform arbitration inputs.
    pub fn resolve_viewport_target(
        &self,
        position: Point<Pixels>,
        context: &DockViewportTargetContext,
    ) -> Option<DockViewportHitCandidate> {
        let hits = self.viewport_hits(position);
        resolve_viewport_target(hits, context)
    }

    fn viewport_hits(&self, position: Point<Pixels>) -> Vec<DockViewportHitCandidate> {
        self.registry
            .iter()
            .filter_map(|(space, snapshot)| {
                self.screen_to_host(space, position)
                    .map(|host_position| DockViewportHitCandidate {
                        space: space.clone(),
                        window: snapshot.window,
                        host_position,
                    })
            })
            .collect()
    }

    /// Resolves a tab release into either an existing viewport hit or a platform tear-off request.
    ///
    /// This method never mutates the docking graph. Callers should open/register a destination
    /// viewport first, then commit a move action after runtime setup succeeds.
    pub fn resolve_tear_off_request(
        &self,
        source_space: impl Into<DockSpaceId>,
        source_tabs: DockNodeId,
        item: impl Into<DockItemId>,
        release_position: Point<Pixels>,
        suggested_window_bounds: Option<WindowBounds>,
        policy: &DockPolicy,
    ) -> DockViewportTearOffOutcome {
        self.resolve_tear_off_request_with_context(
            source_space,
            source_tabs,
            item,
            release_position,
            suggested_window_bounds,
            policy,
            &DockViewportTargetContext::new(),
        )
    }

    /// Resolves a tab release using explicit viewport target arbitration inputs.
    #[allow(clippy::too_many_arguments)]
    pub fn resolve_tear_off_request_with_context(
        &self,
        source_space: impl Into<DockSpaceId>,
        source_tabs: DockNodeId,
        item: impl Into<DockItemId>,
        release_position: Point<Pixels>,
        suggested_window_bounds: Option<WindowBounds>,
        policy: &DockPolicy,
        target_context: &DockViewportTargetContext,
    ) -> DockViewportTearOffOutcome {
        if let Some(hit) = self.hit_test_screen_with_context(release_position, target_context) {
            return DockViewportTearOffOutcome::KnownViewport(hit);
        }

        if let Err(reason) = policy.validate_platform_viewports() {
            return DockViewportTearOffOutcome::Rejected(reason);
        }

        DockViewportTearOffOutcome::Requested(DockViewportTearOffRequest {
            source_space: source_space.into(),
            source_tabs,
            item: item.into(),
            release_position,
            suggested_window_bounds,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{DockGraph, DockHost, DockItemId, DockNode};
    use open_gpui::{WindowHandle, px, size};
    use slotmap::Key;

    fn space(id: &str) -> DockSpaceId {
        DockSpaceId::from(id)
    }

    fn item(id: &str) -> DockItemId {
        DockItemId::from(id)
    }

    fn handle(id: u64) -> AnyWindowHandle {
        WindowHandle::<DockHost>::new(WindowId::from(id)).into()
    }

    fn bounds(x: f32, y: f32, width: f32, height: f32) -> Bounds<Pixels> {
        Bounds::new(point(px(x), px(y)), size(px(width), px(height)))
    }

    #[test]
    fn registering_viewports_records_and_replaces_window_mappings() {
        let mut adapter = DockViewportAdapter::new();
        let main = space("main");
        let secondary = space("secondary");
        let first = handle(1);
        let second = handle(2);

        assert!(adapter.register_viewport(main.clone(), first).is_none());
        assert_eq!(adapter.window_for_space(&main), Some(first));
        assert_eq!(adapter.space_for_window(first), Some(&main));

        let previous = adapter
            .register_viewport(main.clone(), second)
            .expect("replacing a space should return the previous snapshot");
        assert_eq!(previous.window, first);
        assert_eq!(adapter.window_for_space(&main), Some(second));
        assert_eq!(adapter.space_for_window(first), None);

        adapter.register_viewport(secondary.clone(), second);
        assert_eq!(adapter.window_for_space(&main), None);
        assert_eq!(adapter.window_for_space(&secondary), Some(second));
        assert_eq!(adapter.spaces(), vec![secondary]);
    }

    #[test]
    fn unregistering_by_space_or_window_clears_both_indexes() {
        let mut adapter = DockViewportAdapter::new();
        let main = space("main");
        let secondary = space("secondary");
        let first = handle(1);
        let second = handle(2);

        adapter.register_viewport(main.clone(), first);
        adapter.register_viewport(secondary.clone(), second);

        let removed = adapter
            .unregister_space(&main)
            .expect("space should be registered");
        assert_eq!(removed.window, first);
        assert_eq!(adapter.space_for_window(first), None);
        assert_eq!(adapter.window_for_space(&main), None);

        let (removed_space, removed) = adapter
            .unregister_window(second)
            .expect("window should be registered");
        assert_eq!(removed_space, secondary);
        assert_eq!(removed.window, second);
        assert!(adapter.is_empty());
    }

    #[test]
    fn overlapping_viewport_hits_prefer_hovered_active_then_window_stack() {
        let mut adapter = DockViewportAdapter::new();
        let alpha = space("alpha");
        let zeta = space("zeta");
        let first = handle(1);
        let second = handle(2);
        adapter.register_viewport(zeta.clone(), second);
        adapter.register_viewport(alpha.clone(), first);
        for space in [&alpha, &zeta] {
            adapter.update_snapshot(
                space,
                None,
                WindowBounds::Windowed(bounds(100.0, 100.0, 300.0, 200.0)),
                bounds(0.0, 0.0, 300.0, 200.0),
            );
        }
        let position = point(px(125.0), px(150.0));

        assert_eq!(
            adapter.hit_test_screen(position).map(|hit| hit.space),
            Some(alpha.clone()),
            "default fallback should remain deterministic by registered space order"
        );
        assert_eq!(
            adapter
                .hit_test_screen_with_context(
                    position,
                    &DockViewportTargetContext::new().with_active_window(second),
                )
                .map(|hit| hit.space),
            Some(zeta.clone())
        );
        assert_eq!(
            adapter
                .hit_test_screen_with_context(
                    position,
                    &DockViewportTargetContext::new().with_window_stack([second, first]),
                )
                .map(|hit| hit.space),
            Some(zeta.clone())
        );
        assert_eq!(
            adapter
                .hit_test_screen_with_context(
                    position,
                    &DockViewportTargetContext::new()
                        .with_hovered_window(first)
                        .with_active_window(second)
                        .with_window_stack([second, first]),
                )
                .map(|hit| hit.space),
            Some(alpha)
        );
    }

    #[test]
    fn tear_off_release_inside_known_viewport_returns_hit() {
        let mut adapter = DockViewportAdapter::new();
        let main = space("main");
        adapter.register_viewport(main.clone(), handle(1));
        adapter.update_snapshot(
            &main,
            Some(DisplayId::new(7)),
            WindowBounds::Windowed(bounds(100.0, 200.0, 800.0, 600.0)),
            bounds(10.0, 20.0, 300.0, 200.0),
        );

        assert_eq!(
            adapter.resolve_tear_off_request(
                main.clone(),
                DockNodeId::null(),
                item("a"),
                point(px(115.0), px(225.0)),
                None,
                &DockPolicy::default(),
            ),
            DockViewportTearOffOutcome::KnownViewport(DockViewportHit {
                space: main,
                host_position: point(px(5.0), px(5.0)),
            })
        );
    }

    #[test]
    fn tear_off_release_outside_viewports_respects_platform_policy() {
        let adapter = DockViewportAdapter::new();
        let main = space("main");

        assert_eq!(
            adapter.resolve_tear_off_request(
                main,
                DockNodeId::null(),
                item("a"),
                point(px(900.0), px(900.0)),
                None,
                &DockPolicy::default(),
            ),
            DockViewportTearOffOutcome::Rejected(DockPolicyError::PlatformViewportsDisabled)
        );
    }

    #[test]
    fn tear_off_release_outside_viewports_emits_request_when_enabled() {
        let adapter = DockViewportAdapter::new();
        let main = space("main");
        let item = item("a");
        let release_position = point(px(900.0), px(900.0));
        let suggested_window_bounds = WindowBounds::Windowed(bounds(880.0, 880.0, 360.0, 240.0));
        let mut policy = DockPolicy::default();
        policy.set_allow_platform_viewports(true);

        assert_eq!(
            adapter.resolve_tear_off_request(
                main.clone(),
                DockNodeId::null(),
                item.clone(),
                release_position,
                Some(suggested_window_bounds),
                &policy,
            ),
            DockViewportTearOffOutcome::Requested(DockViewportTearOffRequest {
                source_space: main,
                source_tabs: DockNodeId::null(),
                item,
                release_position,
                suggested_window_bounds: Some(suggested_window_bounds),
            })
        );
    }

    #[test]
    fn stale_viewport_bounds_do_not_block_tear_off_request() {
        let mut adapter = DockViewportAdapter::new();
        let main = space("main");
        adapter.register_viewport(main.clone(), handle(1));
        let mut policy = DockPolicy::default();
        policy.set_allow_platform_viewports(true);

        assert!(matches!(
            adapter.resolve_tear_off_request(
                main,
                DockNodeId::null(),
                item("a"),
                point(px(115.0), px(225.0)),
                None,
                &policy,
            ),
            DockViewportTearOffOutcome::Requested(_)
        ));
    }

    #[test]
    fn dock_layout_import_does_not_require_viewport_placement() {
        let mut graph = DockGraph::new();
        let main = space("main");
        let tabs = graph.insert_node(DockNode::Tabs {
            items: vec![DockItemId::from("a")],
            active: 0,
        });
        graph.set_root(main.clone(), tabs);

        let mut adapter = DockViewportAdapter::new();
        adapter.register_viewport(main.clone(), handle(1));
        adapter.update_snapshot(
            &main,
            Some(DisplayId::new(7)),
            WindowBounds::Windowed(bounds(100.0, 200.0, 800.0, 600.0)),
            bounds(10.0, 20.0, 300.0, 200.0),
        );

        let placement_json = serde_json::to_string(&adapter.export_placement())
            .expect("placement should serialize independently");
        let dock_layout = graph.export_layout();
        let layout_json =
            serde_json::to_string(&dock_layout).expect("dock layout should serialize");

        assert!(placement_json.contains("placement_version"));
        assert!(!layout_json.contains("placement_version"));
        assert!(!layout_json.contains("window_bounds"));
        let imported = DockGraph::import_layout(&dock_layout).expect("dock layout should import");
        assert!(imported.root(&main).is_some());
    }

    #[test]
    fn adapter_state_stays_out_of_layout_export() {
        let mut graph = DockGraph::new();
        let tabs = graph.insert_node(DockNode::Tabs {
            items: vec![DockItemId::from("a")],
            active: 0,
        });
        graph.set_root(space("main"), tabs);

        let mut adapter = DockViewportAdapter::new();
        let main = space("main");
        adapter.register_viewport(main.clone(), handle(42));
        adapter.update_snapshot(
            &main,
            Some(DisplayId::new(7)),
            WindowBounds::Windowed(bounds(100.0, 200.0, 800.0, 600.0)),
            bounds(10.0, 20.0, 300.0, 200.0),
        );

        let json = serde_json::to_string(&graph.export_layout()).expect("layout should serialize");
        assert!(!json.contains("WindowHandle"));
        assert!(!json.contains("WindowId"));
        assert!(!json.contains("DisplayId"));
        assert!(!json.contains("AnyWindowHandle"));
    }
}
