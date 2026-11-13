//! Physical GPUI transport for the session owned by `SharedHost`.
//!
//! Integration order:
//! 1. Enroll with `NativeBridge::enable` instead of `enable_host_pointer`.
//! 2. Bind application windows with `bind_window`; use `NativeCallbacks::open`
//!    inside the child-window factory to install callbacks before the first draw.
//! 3. Call `collect(&mut session)` before `begin_native_host_frame`. Resolve native
//!    receiver queries against U5's original presented artifacts in that frame.
//! 4. Pass each committed report to `accept_report`. Outside every `SharedHost`
//!    borrow, call `dispatch_effects(factory, cx)` and `drive_shutdown(cx)`.
//! 5. Keep U5's one exact submission ticket per real window. `watch_submission`
//!    only forwards its actual terminal snapshot; it never acknowledges outputs.
//! 6. After U5 has retired a binding's output/callback owners, seal its callbacks
//!    and call `quiesce_binding`. Native terminal alone cannot prove this boundary.
//!
//! This module owns no layout, selection, drag reducer, renderer, or event loop.
//! Create preparation, first-live admission, logical removal, capture release,
//! renderer shutdown and actual native destruction remain distinct facts.

mod gpui;
mod input;
mod transport;

#[cfg(test)]
mod tests;

use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    rc::Rc,
};

use dockspace::runtime::{
    DockspaceRuntimeError, DockspaceSession, HostFrameReport, NativeEffectRequest,
    NativeHostCapabilities, NativeHostCapability, NativePointerRoster, NativeSurfaceBinding,
    NativeSurfaceCloseAction, NativeSurfaceCloseRequest, NativeWindowBinding, NativeWindowFacts,
    NativeWorkAreaRoster,
};
use open_gpui::{
    AnyWindowHandle, App, NativeCapturedDragReleaseBarrier, NativeCapturedDragReleaseTerminal,
    WindowFrameSubmissionSnapshot, WindowOpenFailureStage,
};
use thiserror::Error;

pub use gpui::{NativeCallbacks, NativeGeometry, NativeOpenFailure, NativeWindowRequest};
pub use input::{NativeCapture, NativePointerFacts};
pub use transport::NativeTransport;

/// Facts delivered to the host without acknowledging its semantic outputs.
#[derive(Debug)]
pub enum NativeReport {
    /// GPUI completed the original window-owned submission ticket.
    Submission {
        /// Original core-owned native window lifetime.
        binding: NativeWindowBinding,
        /// Actual GPUI ticket outcome, including its exact accepted/submitted generations.
        snapshot: WindowFrameSubmissionSnapshot,
    },
    /// Core admitted the exact first-live output; staging can now retire.
    Admitted(NativeSurfaceBinding),
    /// An ordinary native close is awaiting the application's allow/veto/defer policy.
    CloseRequested(NativeSurfaceCloseRequest),
    /// GPUI removed the logical window; native destruction remains outstanding.
    LogicalClosed(NativeWindowBinding),
    /// GPUI delivered the exact native terminal callback.
    NativeTerminal(NativeWindowBinding),
    /// The original GPUI capture-release authority completed independently.
    CaptureTerminal {
        /// Lifetime whose capture operation produced this callback.
        binding: NativeWindowBinding,
        /// GPUI's original source, drag and release identities.
        barrier: NativeCapturedDragReleaseBarrier,
        /// Actual release result; Failed supplies no native release proof.
        terminal: NativeCapturedDragReleaseTerminal,
    },
    /// A native create failed at the original GPUI transaction stage.
    OpenFailed {
        /// Exact core request affected by the failed transaction.
        binding: NativeSurfaceBinding,
        /// Original GPUI create/rollback stage, when supplied by the factory.
        stage: Option<WindowOpenFailureStage>,
        /// Diagnostic from the original window factory.
        message: String,
        /// Original GPUI rollback receipt, kept through renderer and native retirement.
        retirement_ticket: Option<open_gpui::WindowPresentationShutdownTicket>,
    },
    /// A callback still carries an earlier provider/window incarnation.
    StaleCallback(NativeWindowBinding),
    /// The application must retire U5's exact outputs before host quiescence.
    RetirePlacement(NativeSurfaceBinding),
    /// The anchor froze admission before dependent windows start draining.
    GroupShutdownRequested,
    /// A real host operation lacks the original platform authority it requires.
    Unsupported {
        /// Operation that the host could not prove or execute.
        operation: &'static str,
    },
}

/// Application policy for an ordinary close; forced group shutdown bypasses it.
#[derive(Clone, Copy, Debug)]
pub enum NativeCloseDecision {
    /// Starts the core-owned close plan with the selected content disposition.
    Allow(NativeSurfaceCloseAction),
    /// Cancels the exact intercepted native close edge.
    Veto,
    /// Retains the original unresolved edge for a later application decision.
    Defer,
}

/// A bridge error retains its queued input or effect for explicit recovery.
#[derive(Debug, Error)]
pub enum NativeBridgeError {
    /// The sole semantic runtime rejected an ingress or lifecycle operation.
    #[error(transparent)]
    Core(#[from] DockspaceRuntimeError),
    /// The affine native result remains queued for a later successful prefix.
    #[error("native effect report could not be recorded: {0:?}")]
    EffectReport(dockspace::runtime::NativeHostErrorKind),
    /// No exact current physical callback route matches the supplied identity.
    #[error("native binding is unavailable or belongs to another bridge")]
    UnknownBinding,
    /// A physical window cannot belong to two native lifetimes in one bridge.
    #[error("the GPUI window is already bound to another native lifetime")]
    WindowAlreadyBound,
    /// Group shutdown has frozen admission for this provider lifetime.
    #[error("the native bridge is draining its window group")]
    Draining,
    /// At least one original output, callback, capture or native owner remains.
    #[error("native callbacks, outputs, effects or capture are not yet quiescent")]
    NotQuiescent,
    /// GPUI refused the original dependent-before-anchor retirement relation.
    #[error("GPUI rejected the exact native retirement dependencies: {0:?}")]
    Dependencies(open_gpui::NativeWindowRetirementDependencyError),
    /// The owning GPUI update or pointer operation returned an error.
    #[error("the GPUI window update could not run: {0}")]
    WindowUpdate(String),
}

/// A factory receives the exact core request and installs callbacks during root construction.
///
/// `NativeCallbacks::open` provides the ordinary GPUI implementation. Custom factories
/// must retain GPUI's original create/rollback and presentation-shutdown authority.
pub type NativeWindowFactory<'factory> = dyn FnMut(
        NativeWindowRequest,
        &NativeCallbacks,
        &mut App,
    ) -> Result<AnyWindowHandle, NativeOpenFailure>
    + 'factory;

/// Physical bindings and ordered transport for one managed native provider.
pub struct NativeBridge {
    transport: NativeTransport,
    windows: BTreeMap<NativeWindowBinding, NativeCallbacks>,
    effects: VecDeque<NativeEffectRequest>,
    reports: VecDeque<NativeReport>,
    geometry: Option<Rc<NativeGeometry>>,
    capabilities: NativeHostCapabilities,
    retired_placements: BTreeSet<NativeSurfaceBinding>,
    anchor: Option<NativeWindowBinding>,
    draining: bool,
    pending: Option<gpui::PendingDispatch>,
    work_areas: NativeWorkAreaRoster,
}

impl NativeBridge {
    /// Enrolls the sole managed producer. A local host-pointer session cannot be reused.
    ///
    /// Pass an independently observed initial pointer roster, or `Unknown`. Do not
    /// fabricate an empty roster. Optional desktop capabilities require their real
    /// platform adapters; `gpui_capabilities` gives a conservative starting point.
    pub fn enable(
        session: &mut DockspaceSession,
        transport: NativeTransport,
        capabilities: NativeHostCapabilities,
        geometry: Option<Rc<NativeGeometry>>,
    ) -> Result<Self, NativeBridgeError> {
        Self::enable_with_roster(
            session,
            transport,
            NativePointerRoster::Unknown,
            capabilities,
            geometry,
        )
    }

    /// Uses an actual enrollment-time button/capture checkpoint.
    pub fn enable_with_roster(
        session: &mut DockspaceSession,
        transport: NativeTransport,
        roster: NativePointerRoster,
        capabilities: NativeHostCapabilities,
        geometry: Option<Rc<NativeGeometry>>,
    ) -> Result<Self, NativeBridgeError> {
        session.enable_managed_native_host(roster)?;
        session.configure_managed_native_capabilities(capabilities)?;
        Ok(Self {
            transport,
            windows: BTreeMap::new(),
            effects: VecDeque::new(),
            reports: VecDeque::new(),
            geometry,
            capabilities,
            retired_placements: BTreeSet::new(),
            anchor: None,
            draining: false,
            pending: None,
            work_areas: NativeWorkAreaRoster::Unknown,
        })
    }

    /// Returns supported GPUI operations without inventing outer geometry or desktop input.
    pub fn gpui_capabilities(cx: &App) -> NativeHostCapabilities {
        let mut capabilities = NativeHostCapabilities::none_supported()
            .with(NativeHostCapability::AuthoritativeInventory)
            .with(NativeHostCapability::CloseCancellation);
        if cx.viewport_capabilities().platform_viewport_windows {
            capabilities = capabilities.with(NativeHostCapability::NativeWindowLifecycle);
        }
        capabilities
    }

    /// Returns the callback queue; it carries no `SharedHost` or session borrow.
    pub fn transport(&self) -> NativeTransport {
        self.transport.clone()
    }

    /// Reads the physical route without lending a mutable semantic owner.
    pub fn callbacks(&self, binding: NativeWindowBinding) -> Option<NativeCallbacks> {
        self.windows.get(&binding).cloned()
    }

    /// Looks up the exact native lifetime using the full GPUI window id.
    pub fn binding_for_window(&self, window: open_gpui::WindowId) -> Option<NativeWindowBinding> {
        self.windows.iter().find_map(|(binding, callbacks)| {
            (callbacks.window_id() == Some(window)).then_some(*binding)
        })
    }

    /// Returns callback custody without borrowing the host's session.
    pub fn callbacks_for_window(&self, window: open_gpui::WindowId) -> Option<NativeCallbacks> {
        self.binding_for_window(window)
            .and_then(|binding| self.callbacks(binding))
    }

    /// Binds a committed application window to its exact core-minted lifetime.
    ///
    /// Register roots in the session and commit that registration before binding.
    /// Several surface placements may share this one physical binding.
    pub fn bind_window(
        &mut self,
        binding: NativeWindowBinding,
        window: AnyWindowHandle,
        anchor: bool,
        cx: &mut App,
    ) -> Result<NativeCallbacks, NativeBridgeError> {
        window
            .update(cx, |_, window, cx| {
                self.bind_window_current(binding, window, anchor, cx)
            })
            .map_err(|error| NativeBridgeError::WindowUpdate(error.to_string()))?
    }

    /// Installs a root's callbacks inside its original GPUI root construction/update.
    pub fn bind_window_current(
        &mut self,
        binding: NativeWindowBinding,
        window: &mut open_gpui::Window,
        anchor: bool,
        cx: &mut App,
    ) -> Result<NativeCallbacks, NativeBridgeError> {
        if self.draining || self.transport.is_frozen() {
            return Err(NativeBridgeError::Draining);
        }
        if self.windows.contains_key(&binding)
            || self
                .windows
                .values()
                .any(|entry| entry.window_id() == Some(window.window_handle().window_id()))
        {
            return Err(NativeBridgeError::WindowAlreadyBound);
        }
        if anchor && self.anchor.is_some() {
            return Err(NativeBridgeError::WindowAlreadyBound);
        }
        let callbacks =
            NativeCallbacks::new(binding, self.transport.clone(), self.geometry.clone());
        callbacks.allow_preparation();
        if anchor {
            callbacks.set_anchor();
        }
        callbacks.install(window, cx)?;
        self.windows.insert(binding, callbacks.clone());
        if anchor {
            self.anchor = Some(binding);
        }
        self.refresh_dependencies(cx)?;
        Ok(callbacks)
    }

    /// Installs the committed binding roster and retains affine effects in core order.
    pub fn accept_report(&mut self, report: &mut HostFrameReport) {
        for continuation in report.native_root_continuations() {
            let before = continuation.predecessor();
            let after = continuation.successor();
            if let Some(callbacks) = self.windows.remove(&before.window()) {
                callbacks.continue_root(after.window());
                self.windows.insert(after.window(), callbacks);
                if self.anchor == Some(before.window()) {
                    self.anchor = Some(after.window());
                }
            }
            if self.retired_placements.insert(before) {
                self.reports
                    .push_back(NativeReport::RetirePlacement(before));
            }
        }
        for binding in report.native_placement_detachments() {
            if self.retired_placements.insert(*binding) {
                self.reports
                    .push_back(NativeReport::RetirePlacement(*binding));
            }
        }
        for binding in report.native_admissions() {
            self.reports.push_back(NativeReport::Admitted(*binding));
        }
        for outcome in report.inputs() {
            match outcome {
                dockspace::runtime::HostInputOutcome::NativePlatformSnapshotApplied {
                    close_requests,
                }
                | dockspace::runtime::HostInputOutcome::NativeCloseObservationApplied {
                    close_requests,
                } => {
                    self.reports.extend(
                        close_requests
                            .iter()
                            .copied()
                            .map(NativeReport::CloseRequested),
                    );
                }
                _ => {}
            }
        }
        self.effects.extend(report.take_native_effects());
    }

    /// Appends already extracted requests without deduplication or reordering.
    pub fn enqueue_effects(&mut self, effects: impl IntoIterator<Item = NativeEffectRequest>) {
        self.effects.extend(effects);
    }

    /// Resolves one core-issued close edge; `Defer` retains the original edge unchanged.
    pub fn decide_close(
        &mut self,
        session: &mut DockspaceSession,
        close: NativeSurfaceCloseRequest,
        decision: NativeCloseDecision,
    ) -> Result<(), NativeBridgeError> {
        if self.draining || self.transport.is_frozen() {
            return Err(NativeBridgeError::Draining);
        }
        match decision {
            NativeCloseDecision::Allow(action) => {
                session.request_native_surface_close(close, action)?
            }
            NativeCloseDecision::Veto => session.cancel_native_surface_close(close)?,
            NativeCloseDecision::Defer => {}
        }
        Ok(())
    }

    /// Drains reports independently of ingress retry failures.
    pub fn take_reports(&mut self) -> Vec<NativeReport> {
        self.reports.drain(..).collect()
    }

    /// Reports whether every physical and transport owner in this generation has retired.
    pub fn is_quiescent(&self) -> bool {
        self.windows.is_empty()
            && self.effects.is_empty()
            && self.pending.is_none()
            && self.retired_placements.is_empty()
            && self.reports.is_empty()
            && self.transport.is_empty()
    }

    pub(super) fn snapshot(
        &self,
        session: &DockspaceSession,
        replacement: Option<(NativeWindowBinding, NativeWindowFacts)>,
    ) -> Option<Vec<(NativeWindowBinding, NativeWindowFacts)>> {
        let mut observations = Vec::new();
        for binding in session.native_window_bindings() {
            let facts = replacement
                .filter(|(window, _)| *window == binding)
                .map(|(_, facts)| facts)
                .or_else(|| self.windows.get(&binding).and_then(NativeCallbacks::facts))?;
            observations.push((binding, facts));
        }
        if let Some((binding, facts)) = replacement
            && !observations.iter().any(|(window, _)| *window == binding)
            && session.recognizes_native_window_binding(binding)
        {
            observations.push((binding, facts));
        }
        Some(observations)
    }
}
