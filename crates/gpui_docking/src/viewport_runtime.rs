use crate::{
    DockController, DockSpaceId, DockViewportAdapter, DockViewportCloseOutcome,
    DockViewportClosePolicy, DockViewportOpenOutcome, DockViewportPlacementLayout,
    DockViewportPlacementValidationError, DockViewportRestoreOutcome,
    DockViewportShouldCloseOutcome, viewport_close_gate::DockViewportCloseGate,
};
use open_gpui::{AnyWindowHandle, App, Entity, Result, Subscription, WindowId, WindowOptions};
use std::{
    cell::{Ref, RefCell},
    rc::Rc,
};

/// Owner for controller-backed platform viewport lifecycle.
///
/// The runtime keeps the shared [`DockController`] together with the low-level
/// [`DockViewportAdapter`] so applications do not have to pass the controller into every open call
/// or duplicate close-callback cleanup logic. The adapter remains the place for window mappings,
/// coordinate snapshots, and placement import/export.
pub struct DockViewportRuntime {
    controller: Entity<DockController>,
    adapter: DockViewportAdapter,
    close_gate: DockViewportCloseGate,
}

fn install_should_close_hook(
    window: AnyWindowHandle,
    cx: &mut App,
    should_close: Rc<dyn Fn(WindowId) -> bool>,
) -> Result<()> {
    let window_id = window.window_id();
    window.update(cx, move |_, window, cx| {
        window.on_window_should_close(cx, move |_, _| should_close(window_id));
    })
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
            close_gate: DockViewportCloseGate::new(close_policy),
        }
    }

    /// Creates a runtime from an existing adapter.
    pub fn from_adapter(
        controller: Entity<DockController>,
        adapter: DockViewportAdapter,
        close_policy: DockViewportClosePolicy,
    ) -> Self {
        let close_gate = DockViewportCloseGate::new(close_policy);
        close_gate.sync_adapter(&adapter);
        Self {
            controller,
            adapter,
            close_gate,
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
        self.close_gate.close_policy()
    }

    /// Replaces the close policy used by [`handle_window_should_close`](Self::handle_window_should_close).
    pub fn set_close_policy(&mut self, close_policy: DockViewportClosePolicy) {
        self.close_gate.set_close_policy(close_policy);
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
        let close_gate = self.close_gate.clone();
        self.open_viewport_with_should_close(space, options, cx, move |window_id| {
            close_gate.should_allow_close(window_id)
        })
    }

    /// Handles a GPUI window-closed notification by removing stale runtime mapping.
    ///
    /// Close policy is applied by [`Self::handle_window_should_close`] before GPUI accepts a close.
    /// Once a closed notification arrives, the platform window is already gone and docking must
    /// discard the runtime mapping even when the current policy is [`DockViewportClosePolicy::Prevent`].
    pub fn handle_window_closed(&mut self, window_id: WindowId) -> DockViewportCloseOutcome {
        let outcome = self.adapter.handle_window_closed(window_id);
        self.close_gate.sync_adapter(&self.adapter);
        outcome
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
        let should_close = Rc::new(should_close);
        let outcome = self
            .adapter
            .open_viewport(self.controller.clone(), space, options, cx);
        self.close_gate.sync_adapter(&self.adapter);
        let outcome = outcome?;
        if let Err(error) = install_should_close_hook(outcome.window, cx, should_close) {
            self.adapter
                .handle_window_closed(outcome.window.window_id());
            self.close_gate.sync_adapter(&self.adapter);
            return Err(error);
        }

        Ok(outcome)
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
