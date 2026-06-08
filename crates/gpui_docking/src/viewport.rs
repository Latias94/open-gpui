#[cfg(test)]
use crate::{DOCK_VIEWPORT_PLACEMENT_VERSION, DockViewportWindowState};
use crate::{
    DockController, DockHost, DockItemId, DockLayoutRect, DockNodeId, DockPolicy, DockPolicyError,
    DockSpaceId, DockViewportPlacement, DockViewportPlacementLayout,
    DockViewportPlacementValidationError, DockViewportRestoreOutcome, DockViewportWindowBounds,
    viewport_target::{
        DockViewportHit, DockViewportHitCandidate, DockViewportTargetContext,
        resolve_viewport_target,
    },
};
use open_gpui::{
    AnyWindowHandle, App, AppContext as _, Bounds, DisplayId, Entity, Pixels, Point, Result,
    Subscription, Window, WindowBounds, WindowId, WindowOptions, point,
};
use std::{
    cell::{Cell, Ref, RefCell, RefMut},
    collections::{BTreeMap, HashMap},
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
    viewports: BTreeMap<DockSpaceId, DockViewportSnapshot>,
    windows: HashMap<WindowId, DockSpaceId>,
}

/// Runtime snapshot for one rendered dock viewport.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DockViewportSnapshot {
    /// GPUI window currently rendering the logical dock space.
    pub window: AnyWindowHandle,
    /// Display containing the window, when the application has recorded one.
    pub display_id: Option<DisplayId>,
    /// Last known platform window bounds in screen coordinates.
    pub window_bounds: Option<WindowBounds>,
    /// Last known dock host bounds in window-local coordinates.
    pub host_bounds: Option<Bounds<Pixels>>,
}

impl DockViewportSnapshot {
    /// Creates a snapshot for a newly registered viewport window.
    pub fn new(window: AnyWindowHandle) -> Self {
        Self {
            window,
            display_id: None,
            window_bounds: None,
            host_bounds: None,
        }
    }
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

/// Default behavior for a platform viewport close request.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum DockViewportClosePolicy {
    /// Unregister the runtime window and keep the logical dock layout available for reopen.
    #[default]
    RetainLayout,
    /// Reject the close request and leave the runtime mapping intact.
    ///
    /// This policy prevents platform closes only when viewports are opened through
    /// [`DockViewportRuntime`] or [`DockViewportRuntimeHandle`], which install GPUI
    /// should-close hooks. Adapter-level cleanup methods can report a veto outcome, but they run
    /// after the platform close decision has already happened.
    Prevent,
}

/// Runtime result of closing a platform viewport.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DockViewportCloseOutcome {
    /// Logical dock space that was associated with the closed window, when known.
    pub space: Option<DockSpaceId>,
    /// GPUI window id received from the close callback.
    pub window_id: WindowId,
    /// How the close request resolved.
    pub status: DockViewportCloseStatus,
}

/// How a close request resolved.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DockViewportCloseStatus {
    /// The window closed and its runtime mapping was removed.
    Closed,
    /// Policy rejected the close request before the window closed.
    Vetoed,
    /// The runtime did not know the closed window id.
    UnknownWindow,
}

/// Runtime result of a platform should-close query.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DockViewportShouldCloseOutcome {
    /// Logical dock space associated with the queried window, when known.
    pub space: Option<DockSpaceId>,
    /// GPUI window id received from the should-close callback.
    pub window_id: WindowId,
    /// Whether the close should be allowed, vetoed, or ignored as unknown.
    pub status: DockViewportShouldCloseStatus,
}

impl DockViewportShouldCloseOutcome {
    /// Returns true when GPUI should continue closing the platform window.
    pub fn allows_close(&self) -> bool {
        !matches!(self.status, DockViewportShouldCloseStatus::Vetoed)
    }
}

/// How a should-close query resolved.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DockViewportShouldCloseStatus {
    /// Runtime policy allows the platform close to continue.
    Allowed,
    /// Runtime policy rejects the platform close before the window closes.
    Vetoed,
    /// Runtime does not know this window id, so docking should not block GPUI.
    UnknownWindow,
}

/// Runtime result of unregistering a platform viewport mapping.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DockViewportUnregisterOutcome {
    /// Logical dock space removed from the adapter mapping.
    pub space: DockSpaceId,
    /// GPUI window removed from the adapter mapping.
    pub window: AnyWindowHandle,
    /// Why the mapping was removed.
    pub reason: DockViewportUnregisterReason,
}

/// Reason a platform viewport mapping was removed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DockViewportUnregisterReason {
    /// The platform window closed.
    Closed,
    /// A new window replaced the previous mapping.
    Replaced,
    /// The application discarded runtime placement for the space.
    Discarded,
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

    /// Returns mutable access to the low-level viewport adapter.
    pub fn adapter_mut(&mut self) -> &mut DockViewportAdapter {
        &mut self.adapter
    }

    /// Returns the close policy used by [`handle_window_closed`](Self::handle_window_closed).
    pub fn close_policy(&self) -> DockViewportClosePolicy {
        self.close_policy.get()
    }

    /// Replaces the close policy used by [`handle_window_closed`](Self::handle_window_closed).
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

    /// Handles a GPUI window-closed notification by applying this runtime's close policy.
    pub fn handle_window_closed(&mut self, window_id: WindowId) -> DockViewportCloseOutcome {
        self.adapter
            .close_viewport_mapping(window_id, self.close_policy())
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

    /// Mutably borrows the shared runtime.
    pub fn borrow_mut(&self) -> RefMut<'_, DockViewportRuntime> {
        self.runtime.borrow_mut()
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
}

impl DockViewportAdapter {
    /// Creates an empty viewport adapter.
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns true when no viewport mappings are registered.
    pub fn is_empty(&self) -> bool {
        self.viewports.is_empty()
    }

    /// Returns the number of registered logical viewports.
    pub fn len(&self) -> usize {
        self.viewports.len()
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
        let window_id = window.window_id();

        if let Some(previous) = self.viewports.get(&space) {
            self.windows.remove(&previous.window.window_id());
        }
        if let Some(previous_space) = self.windows.remove(&window_id)
            && previous_space != space
        {
            self.viewports.remove(&previous_space);
        }

        self.windows.insert(window_id, space.clone());
        self.viewports
            .insert(space, DockViewportSnapshot::new(window))
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

    /// Applies viewport close policy before a GPUI platform window closes.
    ///
    /// Unknown windows are allowed to close because docking has no mapping to protect.
    pub fn should_close_viewport(
        &self,
        window_id: WindowId,
        policy: DockViewportClosePolicy,
    ) -> DockViewportShouldCloseOutcome {
        let Some(space) = self.space_for_window_id(window_id).cloned() else {
            return DockViewportShouldCloseOutcome {
                space: None,
                window_id,
                status: DockViewportShouldCloseStatus::UnknownWindow,
            };
        };

        let status = match policy {
            DockViewportClosePolicy::RetainLayout => DockViewportShouldCloseStatus::Allowed,
            DockViewportClosePolicy::Prevent => DockViewportShouldCloseStatus::Vetoed,
        };
        DockViewportShouldCloseOutcome {
            space: Some(space),
            window_id,
            status,
        }
    }

    /// Removes a viewport by logical dock space.
    pub fn unregister_space(&mut self, space: &DockSpaceId) -> Option<DockViewportSnapshot> {
        let snapshot = self.viewports.remove(space)?;
        self.windows.remove(&snapshot.window.window_id());
        Some(snapshot)
    }

    /// Removes a viewport by GPUI window handle.
    pub fn unregister_window(
        &mut self,
        window: impl Into<AnyWindowHandle>,
    ) -> Option<(DockSpaceId, DockViewportSnapshot)> {
        let window = window.into();
        let space = self.windows.remove(&window.window_id())?;
        let snapshot = self.viewports.remove(&space)?;
        Some((space, snapshot))
    }

    /// Removes a viewport by GPUI window id and returns a lifecycle outcome.
    ///
    /// This is the cleanup path for close callbacks that report only [`WindowId`].
    pub fn unregister_window_id(
        &mut self,
        window_id: WindowId,
        reason: DockViewportUnregisterReason,
    ) -> Option<DockViewportUnregisterOutcome> {
        let space = self.windows.remove(&window_id)?;
        let snapshot = self.viewports.remove(&space)?;
        Some(DockViewportUnregisterOutcome {
            space,
            window: snapshot.window,
            reason,
        })
    }

    /// Applies viewport close policy to the adapter mapping for a window id.
    ///
    /// `RetainLayout` removes only the runtime mapping. It does not mutate the docking graph.
    /// `Prevent` returns a veto outcome and leaves the mapping intact.
    pub fn close_viewport_mapping(
        &mut self,
        window_id: WindowId,
        policy: DockViewportClosePolicy,
    ) -> DockViewportCloseOutcome {
        let Some(space) = self.windows.get(&window_id).cloned() else {
            return DockViewportCloseOutcome {
                space: None,
                window_id,
                status: DockViewportCloseStatus::UnknownWindow,
            };
        };

        if !self.viewports.contains_key(&space) {
            self.windows.remove(&window_id);
            return DockViewportCloseOutcome {
                space: None,
                window_id,
                status: DockViewportCloseStatus::UnknownWindow,
            };
        }

        match policy {
            DockViewportClosePolicy::Prevent => DockViewportCloseOutcome {
                space: Some(space),
                window_id,
                status: DockViewportCloseStatus::Vetoed,
            },
            DockViewportClosePolicy::RetainLayout => {
                if let Some(outcome) =
                    self.unregister_window_id(window_id, DockViewportUnregisterReason::Closed)
                {
                    DockViewportCloseOutcome {
                        space: Some(outcome.space),
                        window_id,
                        status: DockViewportCloseStatus::Closed,
                    }
                } else {
                    DockViewportCloseOutcome {
                        space: None,
                        window_id,
                        status: DockViewportCloseStatus::UnknownWindow,
                    }
                }
            }
        }
    }

    /// Returns the snapshot for a logical dock space.
    pub fn snapshot(&self, space: &DockSpaceId) -> Option<&DockViewportSnapshot> {
        self.viewports.get(space)
    }

    /// Returns the window rendering a logical dock space.
    pub fn window_for_space(&self, space: &DockSpaceId) -> Option<AnyWindowHandle> {
        self.snapshot(space).map(|snapshot| snapshot.window)
    }

    /// Returns the logical dock space rendered by a window.
    pub fn space_for_window(&self, window: impl Into<AnyWindowHandle>) -> Option<&DockSpaceId> {
        let window = window.into();
        self.space_for_window_id(window.window_id())
    }

    /// Returns the logical dock space rendered by a window id.
    pub fn space_for_window_id(&self, window_id: WindowId) -> Option<&DockSpaceId> {
        self.windows
            .get(&window_id)
            .and_then(|space| self.viewports.get_key_value(space).map(|(space, _)| space))
    }

    /// Returns known dock spaces in stable lexical order.
    pub fn spaces(&self) -> Vec<DockSpaceId> {
        self.viewports.keys().cloned().collect()
    }

    /// Updates the display id snapshot for a logical dock space.
    pub fn set_display_id(&mut self, space: &DockSpaceId, display_id: Option<DisplayId>) -> bool {
        let Some(snapshot) = self.viewports.get_mut(space) else {
            return false;
        };
        snapshot.display_id = display_id;
        true
    }

    /// Updates the platform window bounds snapshot for a logical dock space.
    pub fn set_window_bounds(&mut self, space: &DockSpaceId, bounds: WindowBounds) -> bool {
        let Some(snapshot) = self.viewports.get_mut(space) else {
            return false;
        };
        snapshot.window_bounds = Some(bounds);
        true
    }

    /// Updates the dock host bounds snapshot for a logical dock space.
    pub fn set_host_bounds(&mut self, space: &DockSpaceId, bounds: Bounds<Pixels>) -> bool {
        let Some(snapshot) = self.viewports.get_mut(space) else {
            return false;
        };
        snapshot.host_bounds = Some(bounds);
        true
    }

    /// Updates display id, window bounds, and host bounds in one snapshot write.
    pub fn update_snapshot(
        &mut self,
        space: &DockSpaceId,
        display_id: Option<DisplayId>,
        window_bounds: WindowBounds,
        host_bounds: Bounds<Pixels>,
    ) -> bool {
        let Some(snapshot) = self.viewports.get_mut(space) else {
            return false;
        };
        snapshot.display_id = display_id;
        snapshot.window_bounds = Some(window_bounds);
        snapshot.host_bounds = Some(host_bounds);
        true
    }

    /// Converts a window-local point into host-local coordinates.
    ///
    /// Returns `None` when the viewport is unknown, host bounds are stale, or the point is outside
    /// the host bounds.
    pub fn window_to_host(
        &self,
        space: &DockSpaceId,
        position: Point<Pixels>,
    ) -> Option<Point<Pixels>> {
        let host_bounds = self.snapshot(space)?.host_bounds?;
        if !host_bounds.contains(&position) {
            return None;
        }

        Some(point(
            position.x - host_bounds.origin.x,
            position.y - host_bounds.origin.y,
        ))
    }

    /// Converts a screen point into host-local coordinates.
    ///
    /// Returns `None` when the viewport is unknown, bounds snapshots are stale, or the point is
    /// outside the host bounds.
    pub fn screen_to_host(
        &self,
        space: &DockSpaceId,
        position: Point<Pixels>,
    ) -> Option<Point<Pixels>> {
        let snapshot = self.snapshot(space)?;
        let window_bounds = snapshot.window_bounds?.get_bounds();
        let window_position = point(
            position.x - window_bounds.origin.x,
            position.y - window_bounds.origin.y,
        );
        self.window_to_host(space, window_position)
    }

    /// Converts a host-local point into screen coordinates.
    pub fn host_to_screen(
        &self,
        space: &DockSpaceId,
        position: Point<Pixels>,
    ) -> Option<Point<Pixels>> {
        let snapshot = self.snapshot(space)?;
        let window_bounds = snapshot.window_bounds?.get_bounds();
        let host_bounds = snapshot.host_bounds?;
        Some(point(
            window_bounds.origin.x + host_bounds.origin.x + position.x,
            window_bounds.origin.y + host_bounds.origin.y + position.y,
        ))
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
        self.viewports
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

    /// Exports serializable placement snapshots for all registered viewports.
    pub fn export_placement(&self) -> DockViewportPlacementLayout {
        DockViewportPlacementLayout::new(
            self.viewports
                .iter()
                .map(|(space, snapshot)| DockViewportPlacement {
                    space: space.clone(),
                    display_id: snapshot.display_id.map(u64::from),
                    window_bounds: snapshot
                        .window_bounds
                        .map(DockViewportWindowBounds::from_window_bounds),
                    host_bounds: snapshot.host_bounds.map(DockLayoutRect::from_bounds),
                })
                .collect(),
        )
    }

    /// Applies placement snapshots to already registered viewport windows.
    ///
    /// This does not open windows or create viewport mappings. Applications should first register
    /// the windows they restored, then apply placement data to rehydrate adapter snapshots.
    pub fn apply_placement(
        &mut self,
        placement: &DockViewportPlacementLayout,
    ) -> Result<DockViewportRestoreOutcome, DockViewportPlacementValidationError> {
        placement.validate()?;

        let mut applied = 0;
        let mut skipped = 0;
        for viewport in &placement.viewports {
            let Some(snapshot) = self.viewports.get_mut(&viewport.space) else {
                skipped += 1;
                continue;
            };
            snapshot.display_id = viewport.display_id.map(DisplayId::from);
            snapshot.window_bounds = viewport
                .window_bounds
                .map(DockViewportWindowBounds::to_window_bounds);
            snapshot.host_bounds = viewport.host_bounds.map(DockLayoutRect::to_bounds);
            applied += 1;
        }

        Ok(DockViewportRestoreOutcome { applied, skipped })
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
    fn unregistering_by_window_id_clears_close_callback_mapping() {
        let mut adapter = DockViewportAdapter::new();
        let main = space("main");
        let secondary = space("secondary");
        let first = handle(1);
        let second = handle(2);

        adapter.register_viewport(main.clone(), first);
        adapter.register_viewport(secondary.clone(), second);

        let removed = adapter
            .unregister_window_id(first.window_id(), DockViewportUnregisterReason::Closed)
            .expect("window id should be registered");
        assert_eq!(removed.space, main);
        assert_eq!(removed.window, first);
        assert_eq!(removed.reason, DockViewportUnregisterReason::Closed);
        assert_eq!(adapter.space_for_window_id(first.window_id()), None);
        assert_eq!(adapter.window_for_space(&removed.space), None);
        assert_eq!(adapter.window_for_space(&secondary), Some(second));

        assert_eq!(
            adapter.unregister_window_id(first.window_id(), DockViewportUnregisterReason::Closed),
            None
        );
    }

    #[test]
    fn close_policy_retain_layout_removes_only_runtime_mapping() {
        let mut graph = DockGraph::new();
        let main = space("main");
        let tabs = graph.insert_node(DockNode::Tabs {
            items: vec![DockItemId::from("a")],
            active: 0,
        });
        graph.set_root(main.clone(), tabs);

        let mut adapter = DockViewportAdapter::new();
        let window = handle(1);
        adapter.register_viewport(main.clone(), window);

        let outcome = adapter
            .close_viewport_mapping(window.window_id(), DockViewportClosePolicy::RetainLayout);
        assert_eq!(
            outcome,
            DockViewportCloseOutcome {
                space: Some(main.clone()),
                window_id: window.window_id(),
                status: DockViewportCloseStatus::Closed,
            }
        );
        assert!(adapter.is_empty());
        assert!(
            graph.root(&main).is_some(),
            "runtime cleanup must not mutate the logical docking graph"
        );

        let reopened = handle(2);
        adapter.register_viewport(main.clone(), reopened);
        assert_eq!(adapter.window_for_space(&main), Some(reopened));
        assert_eq!(
            adapter.space_for_window_id(reopened.window_id()),
            Some(&main)
        );
    }

    #[test]
    fn close_policy_prevent_vetoes_and_preserves_mapping() {
        let mut adapter = DockViewportAdapter::new();
        let main = space("main");
        let window = handle(1);
        adapter.register_viewport(main.clone(), window);

        let outcome =
            adapter.close_viewport_mapping(window.window_id(), DockViewportClosePolicy::Prevent);
        assert_eq!(
            outcome,
            DockViewportCloseOutcome {
                space: Some(main.clone()),
                window_id: window.window_id(),
                status: DockViewportCloseStatus::Vetoed,
            }
        );
        assert_eq!(adapter.window_for_space(&main), Some(window));
        assert_eq!(adapter.space_for_window_id(window.window_id()), Some(&main));
    }

    #[test]
    fn close_mapping_unknown_window_is_not_reported_as_vetoed() {
        let mut adapter = DockViewportAdapter::new();
        let unknown = WindowId::from(99);

        assert_eq!(
            adapter.close_viewport_mapping(unknown, DockViewportClosePolicy::Prevent),
            DockViewportCloseOutcome {
                space: None,
                window_id: unknown,
                status: DockViewportCloseStatus::UnknownWindow,
            }
        );
        assert_eq!(
            adapter.close_viewport_mapping(unknown, DockViewportClosePolicy::RetainLayout),
            DockViewportCloseOutcome {
                space: None,
                window_id: unknown,
                status: DockViewportCloseStatus::UnknownWindow,
            }
        );
    }

    #[test]
    fn close_mapping_discards_stale_window_index() {
        let mut adapter = DockViewportAdapter::new();
        let main = space("main");
        let window_id = WindowId::from(1);
        adapter.windows.insert(window_id, main);

        assert_eq!(
            adapter.close_viewport_mapping(window_id, DockViewportClosePolicy::Prevent),
            DockViewportCloseOutcome {
                space: None,
                window_id,
                status: DockViewportCloseStatus::UnknownWindow,
            }
        );
        assert_eq!(adapter.windows.get(&window_id), None);
        assert!(adapter.is_empty());
    }

    #[test]
    fn viewport_lifecycle_types_preserve_runtime_boundaries() {
        let main = space("main");
        let window = handle(7);
        let open = DockViewportOpenOutcome {
            space: main.clone(),
            window,
            status: DockViewportOpenStatus::Opened,
        };
        assert_eq!(open.space, main.clone());
        assert_eq!(open.window, window);
        assert_eq!(open.status, DockViewportOpenStatus::Opened);
        assert_eq!(
            DockViewportClosePolicy::default(),
            DockViewportClosePolicy::RetainLayout
        );

        let close = DockViewportCloseOutcome {
            space: Some(main.clone()),
            window_id: window.window_id(),
            status: DockViewportCloseStatus::Closed,
        };
        assert_eq!(close.space, Some(main.clone()));
        assert_eq!(close.window_id, window.window_id());
        assert_eq!(close.status, DockViewportCloseStatus::Closed);

        let unregister = DockViewportUnregisterOutcome {
            space: main,
            window,
            reason: DockViewportUnregisterReason::Closed,
        };
        assert_eq!(unregister.reason, DockViewportUnregisterReason::Closed);
    }

    #[test]
    fn coordinate_conversion_requires_current_bounds_snapshots() {
        let mut adapter = DockViewportAdapter::new();
        let main = space("main");
        adapter.register_viewport(main.clone(), handle(1));

        assert!(
            adapter
                .screen_to_host(&main, point(px(115.0), px(225.0)))
                .is_none()
        );

        assert!(adapter.update_snapshot(
            &main,
            Some(DisplayId::new(7)),
            WindowBounds::Windowed(bounds(100.0, 200.0, 800.0, 600.0)),
            bounds(10.0, 20.0, 300.0, 200.0),
        ));

        assert_eq!(
            adapter.window_to_host(&main, point(px(15.0), px(25.0))),
            Some(point(px(5.0), px(5.0)))
        );
        assert_eq!(
            adapter.screen_to_host(&main, point(px(115.0), px(225.0))),
            Some(point(px(5.0), px(5.0)))
        );
        assert_eq!(
            adapter.host_to_screen(&main, point(px(5.0), px(5.0))),
            Some(point(px(115.0), px(225.0)))
        );
        assert_eq!(
            adapter.hit_test_screen(point(px(115.0), px(225.0))),
            Some(DockViewportHit {
                space: main.clone(),
                host_position: point(px(5.0), px(5.0)),
            })
        );
        assert!(
            adapter
                .screen_to_host(&main, point(px(500.0), px(500.0)))
                .is_none()
        );
    }

    #[test]
    fn should_close_policy_reports_pre_close_veto_without_mutating_mapping() {
        let mut adapter = DockViewportAdapter::new();
        let main = space("main");
        let window = handle(1);
        adapter.register_viewport(main.clone(), window);

        let allowed = adapter
            .should_close_viewport(window.window_id(), DockViewportClosePolicy::RetainLayout);
        assert_eq!(
            allowed,
            DockViewportShouldCloseOutcome {
                space: Some(main.clone()),
                window_id: window.window_id(),
                status: DockViewportShouldCloseStatus::Allowed,
            }
        );
        assert!(allowed.allows_close());
        assert_eq!(adapter.window_for_space(&main), Some(window));

        let vetoed =
            adapter.should_close_viewport(window.window_id(), DockViewportClosePolicy::Prevent);
        assert_eq!(
            vetoed,
            DockViewportShouldCloseOutcome {
                space: Some(main.clone()),
                window_id: window.window_id(),
                status: DockViewportShouldCloseStatus::Vetoed,
            }
        );
        assert!(!vetoed.allows_close());
        assert_eq!(adapter.window_for_space(&main), Some(window));

        let unknown =
            adapter.should_close_viewport(WindowId::from(99), DockViewportClosePolicy::Prevent);
        assert_eq!(unknown.status, DockViewportShouldCloseStatus::UnknownWindow);
        assert!(unknown.allows_close());
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
    fn viewport_placement_roundtrips_without_runtime_window_handles() {
        let mut adapter = DockViewportAdapter::new();
        let main = space("main");
        let secondary = space("secondary");
        adapter.register_viewport(main.clone(), handle(1));
        adapter.register_viewport(secondary, handle(2));
        assert!(adapter.update_snapshot(
            &main,
            Some(DisplayId::new(7)),
            WindowBounds::Maximized(bounds(100.0, 200.0, 800.0, 600.0)),
            bounds(10.0, 20.0, 300.0, 200.0),
        ));

        let placement = adapter.export_placement();
        let json = serde_json::to_string(&placement).expect("placement should serialize");
        assert!(json.contains("placement_version"));
        assert!(json.contains("maximized"));
        assert!(!json.contains("WindowHandle"));
        assert!(!json.contains("WindowId"));
        assert!(!json.contains("AnyWindowHandle"));

        let placement: DockViewportPlacementLayout =
            serde_json::from_str(&json).expect("placement should deserialize");
        let mut restored = DockViewportAdapter::new();
        restored.register_viewport(main.clone(), handle(99));
        assert_eq!(
            restored
                .apply_placement(&placement)
                .expect("placement should apply"),
            DockViewportRestoreOutcome {
                applied: 1,
                skipped: 1,
            }
        );

        let snapshot = restored
            .snapshot(&main)
            .expect("main viewport should be restored");
        assert_eq!(snapshot.window, handle(99));
        assert_eq!(snapshot.display_id, Some(DisplayId::new(7)));
        assert_eq!(
            snapshot.window_bounds,
            Some(WindowBounds::Maximized(bounds(100.0, 200.0, 800.0, 600.0)))
        );
        assert_eq!(snapshot.host_bounds, Some(bounds(10.0, 20.0, 300.0, 200.0)));
    }

    #[test]
    fn viewport_restore_workflow_uses_new_runtime_windows_with_saved_placement() {
        let mut adapter = DockViewportAdapter::new();
        let main = space("main");
        let secondary = space("secondary");
        adapter.register_viewport(main.clone(), handle(1));
        adapter.register_viewport(secondary.clone(), handle(2));
        adapter.update_snapshot(
            &main,
            Some(DisplayId::new(7)),
            WindowBounds::Windowed(bounds(100.0, 200.0, 800.0, 600.0)),
            bounds(10.0, 20.0, 300.0, 200.0),
        );
        adapter.update_snapshot(
            &secondary,
            Some(DisplayId::new(8)),
            WindowBounds::Windowed(bounds(900.0, 200.0, 500.0, 400.0)),
            bounds(30.0, 40.0, 240.0, 180.0),
        );
        let placement = adapter.export_placement();

        let mut restored = DockViewportAdapter::new();
        restored.register_viewport(main.clone(), handle(101));
        restored.register_viewport(secondary.clone(), handle(102));

        assert_eq!(
            restored
                .apply_placement(&placement)
                .expect("saved placement should apply to registered restore windows"),
            DockViewportRestoreOutcome {
                applied: 2,
                skipped: 0,
            }
        );
        assert_eq!(restored.window_for_space(&main), Some(handle(101)));
        assert_eq!(restored.space_for_window(handle(102)), Some(&secondary));
        assert_eq!(
            restored.hit_test_screen(point(px(935.0), px(245.0))),
            Some(DockViewportHit {
                space: secondary,
                host_position: point(px(5.0), px(5.0)),
            })
        );
    }

    #[test]
    fn placement_window_options_use_saved_bounds_and_display_hint() {
        let main = space("main");
        let saved_bounds = DockViewportWindowBounds {
            state: DockViewportWindowState::Maximized,
            bounds: DockLayoutRect::from_bounds(bounds(100.0, 200.0, 800.0, 600.0)),
        };
        let fallback_bounds = WindowBounds::Windowed(bounds(0.0, 0.0, 320.0, 240.0));
        let placement = DockViewportPlacementLayout::new(vec![DockViewportPlacement {
            space: main.clone(),
            display_id: Some(7),
            window_bounds: Some(saved_bounds),
            host_bounds: None,
        }]);

        let options = placement
            .window_options_for_space(
                &main,
                WindowOptions {
                    window_bounds: Some(fallback_bounds),
                    focus: false,
                    ..Default::default()
                },
            )
            .expect("valid placement should produce window options");

        assert_eq!(
            placement
                .placement_for_space(&main)
                .map(|p| p.space.clone()),
            Some(main)
        );
        assert_eq!(options.window_bounds, Some(saved_bounds.to_window_bounds()));
        assert_eq!(options.display_id, Some(DisplayId::from(7)));
        assert!(
            !options.focus,
            "fallback options should preserve non-placement fields"
        );
    }

    #[test]
    fn placement_window_options_keep_fallback_for_missing_space() {
        let main = space("main");
        let secondary = space("secondary");
        let fallback_bounds = WindowBounds::Windowed(bounds(0.0, 0.0, 320.0, 240.0));
        let placement = DockViewportPlacementLayout::new(vec![DockViewportPlacement {
            space: main.clone(),
            display_id: None,
            window_bounds: None,
            host_bounds: None,
        }]);

        let matching_options = placement
            .window_options_for_space(
                &main,
                WindowOptions {
                    window_bounds: Some(fallback_bounds),
                    display_id: Some(DisplayId::from(9)),
                    ..Default::default()
                },
            )
            .expect("missing saved fields should keep fallback options");
        assert_eq!(matching_options.window_bounds, Some(fallback_bounds));
        assert_eq!(matching_options.display_id, Some(DisplayId::from(9)));

        let options = placement
            .window_options_for_space(
                &secondary,
                WindowOptions {
                    window_bounds: Some(fallback_bounds),
                    display_id: Some(DisplayId::from(9)),
                    ..Default::default()
                },
            )
            .expect("valid placement should preserve fallback for missing spaces");

        assert!(placement.placement_for_space(&secondary).is_none());
        assert_eq!(options.window_bounds, Some(fallback_bounds));
        assert_eq!(options.display_id, Some(DisplayId::from(9)));
    }

    #[test]
    fn invalid_placement_rejects_window_options_before_runtime_mutation() {
        let main = space("main");
        let mut adapter = DockViewportAdapter::new();
        adapter.register_viewport(main.clone(), handle(1));
        let placement = DockViewportPlacementLayout::new(vec![
            DockViewportPlacement {
                space: main.clone(),
                display_id: None,
                window_bounds: None,
                host_bounds: None,
            },
            DockViewportPlacement {
                space: main.clone(),
                display_id: None,
                window_bounds: None,
                host_bounds: None,
            },
        ]);

        let error = placement
            .window_options_for_space(&main, WindowOptions::default())
            .expect_err("invalid placement should fail before options are returned");
        assert_eq!(
            error,
            DockViewportPlacementValidationError::DuplicateSpace {
                space: main.clone()
            }
        );
        assert_eq!(adapter.window_for_space(&main), Some(handle(1)));
        assert_eq!(adapter.spaces(), vec![main]);
    }

    #[test]
    fn viewport_placement_validation_rejects_bad_version_and_duplicate_spaces() {
        let main = space("main");
        let mut placement = DockViewportPlacementLayout::new(vec![DockViewportPlacement {
            space: main.clone(),
            display_id: None,
            window_bounds: None,
            host_bounds: None,
        }]);
        placement.placement_version = 99;
        assert_eq!(
            placement.validate(),
            Err(DockViewportPlacementValidationError::UnsupportedVersion {
                expected: DOCK_VIEWPORT_PLACEMENT_VERSION,
                found: 99,
            })
        );

        let placement = DockViewportPlacementLayout::new(vec![
            DockViewportPlacement {
                space: main.clone(),
                display_id: None,
                window_bounds: None,
                host_bounds: None,
            },
            DockViewportPlacement {
                space: main.clone(),
                display_id: None,
                window_bounds: None,
                host_bounds: None,
            },
        ]);
        assert_eq!(
            placement.validate(),
            Err(DockViewportPlacementValidationError::DuplicateSpace { space: main })
        );
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
