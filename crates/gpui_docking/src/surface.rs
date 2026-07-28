mod activation;
mod builder;
mod owner;
mod panel;
mod state;
mod viewport;
mod viewport_readiness;
pub(crate) mod window_session;

#[cfg(test)]
mod window_session_tests;

pub use activation::{DockSurfaceActivationOutcome, DockSurfaceActivationRequestId};
pub use builder::{DockSurfaceBuildError, DockSurfaceBuilder};
pub use owner::{DockSurfaceChangeCategory, DockSurfaceChangeEvent};
pub use panel::{
    DockSurfaceChange, DockSurfaceFloatingPanelSnapshot, DockSurfacePanelError,
    DockSurfacePanelLocation, DockSurfacePanelLocationKind, DockSurfacePanelOutcome,
    DockSurfacePanelSnapshot,
};
pub use state::DockSurfaceSnapshot;
pub use viewport::{
    DockSurfaceViewportCloseOutcome, DockSurfaceViewportCloseStatus,
    DockSurfaceViewportOpenOutcome, DockSurfaceViewportOpenReport, DockSurfaceViewportOpenStatus,
    DockSurfaceViewportOpened, DockSurfaceViewportRestoreOutcome, DockSurfaceViewportRestoreReport,
    DockSurfaceViewportShouldCloseOutcome, DockSurfaceViewportShouldCloseStatus,
    DockSurfaceViewportSpec, DockSurfaceViewportSpecError, DockSurfaceViewportUnavailable,
    DockSurfaceViewports,
};
pub use viewport_readiness::{
    DockSurfaceViewportFlagWarning, DockSurfaceViewportInputStatus,
    DockSurfaceViewportLifecycleReadiness, DockSurfaceViewportPlatformCapabilities,
    DockSurfaceViewportPlatformReadiness, DockSurfaceViewportReadiness,
    DockSurfaceViewportReadinessReport, DockSurfaceViewportReadinessStatus,
    DockSurfaceViewportRouteStatus, DockSurfaceViewportStaleReason,
    DockSurfaceViewportUnsupportedFlag,
};
pub use window_session::{
    DockSurfacePrimaryWindowOpenConflict, DockSurfacePrimaryWindowOpenOutcome,
    DockSurfacePrimaryWindowOpened, DockSurfacePrimaryWindowUnavailable,
    DockSurfaceWindowSessionOpeningRollbackReason, DockSurfaceWindowSessionPhase,
    DockSurfaceWindowSessionReason, DockSurfaceWindowSessionShutdownReason,
    DockSurfaceWindowSessionStatus,
};

use crate::{
    DockController, DockHost, DockSpaceId, DockViewportClosePolicy,
    DockViewportRuntimeCommitAuthority, DockViewportRuntimeHandle, DockViewportRuntimeLineage,
    DockViewportWindowRole, DockVisualStyleResolver,
};
pub(crate) use activation::{
    DockSurfaceActivationBinding, DockSurfaceActivationHostRegistration,
    DockSurfaceActivationHostRegistrationStatus, DockSurfaceActivationSettlements,
    DockSurfaceActivationState,
};
#[cfg(test)]
pub(crate) use activation::{DockSurfaceActivationDispatch, DockSurfaceActivationHostLookup};
use open_gpui::{
    AnyView, App, AppContext, Bounds, Context, Entity, Pixels, Subscription, Window, WindowBounds,
    WindowId, WindowInitialPresentationStatus, WindowOpenFailureStage, WindowOptions,
};
pub(crate) use owner::{
    DockSurfaceOwner, DockSurfaceTransactionId, with_detached_root_transaction,
    with_root_transaction,
};
use std::{
    cell::Cell,
    panic::{AssertUnwindSafe, catch_unwind, resume_unwind},
    rc::Rc,
};

/// Application-level owner for one docked workspace and its viewport runtime.
///
/// `DockSurface` is the common app seam for docking. It keeps controller state, host creation, and
/// viewport runtime wiring together so ordinary applications do not need to assemble
/// [`runtime::DockHost`](crate::runtime::DockHost) and
/// [`runtime::DockViewportRuntimeHandle`](crate::runtime::DockViewportRuntimeHandle) directly.
#[derive(Clone, Debug)]
pub struct DockSurface {
    owner: Entity<DockSurfaceOwner>,
    primary_space: DockSpaceId,
}

fn settle_surface_window_terminal(
    owner: &Entity<DockSurfaceOwner>,
    runtime: &DockViewportRuntimeHandle,
    lease: window_session::DockSurfaceWindowSessionLease,
    window_id: WindowId,
    disposition: window_session::DockSurfaceWindowSessionTerminalDisposition,
    cx: &mut App,
) {
    let _ = runtime.settle_surface_window_terminal(lease, window_id);
    let runtime_empty = runtime.surface_generation_empty(lease);
    cx.update_entity(owner, |owner, owner_cx| {
        let terminal = owner
            .window_session_mut()
            .settle_terminal(lease, window_id, disposition);
        let runtime = runtime_empty.then(|| owner.window_session_mut().mark_runtime_empty(lease));
        let convergence = owner.window_session_mut().complete_shutdown(lease);
        if matches!(
            terminal,
            window_session::DockSurfaceWindowSessionTerminalOutcome::Settled
        ) || matches!(
            runtime,
            Some(window_session::DockSurfaceWindowSessionRuntimeEmptyOutcome::Marked)
        ) || matches!(
            convergence,
            window_session::DockSurfaceWindowSessionShutdownConvergenceOutcome::Closed
        ) {
            owner_cx.notify();
        }
    });
}

fn claim_surface_window_close(
    owner: &Entity<DockSurfaceOwner>,
    lease: window_session::DockSurfaceWindowSessionLease,
    window_id: WindowId,
    cx: &mut App,
) -> bool {
    matches!(
        cx.update_entity(owner, |owner, owner_cx| {
            let outcome = owner
                .window_session_mut()
                .claim_close_dispatch(lease, window_id);
            if matches!(
                outcome,
                window_session::DockSurfaceWindowSessionCloseDispatchOutcome::Claimed
            ) {
                owner_cx.notify();
            }
            outcome
        }),
        window_session::DockSurfaceWindowSessionCloseDispatchOutcome::Claimed
    )
}

fn close_surface_window(
    owner: &Entity<DockSurfaceOwner>,
    lease: window_session::DockSurfaceWindowSessionLease,
    window: open_gpui::AnyWindowHandle,
    cx: &mut App,
) {
    if claim_surface_window_close(owner, lease, window.window_id(), cx) {
        let _ = window.update(cx, |_, window, cx| window.remove_window(cx));
    }
}

fn confirm_pending_surface_terminals_absent(
    owner: &Entity<DockSurfaceOwner>,
    runtime: &DockViewportRuntimeHandle,
    lease: window_session::DockSurfaceWindowSessionLease,
    cx: &mut App,
) {
    let pending = cx.read_entity(owner, |owner, _| {
        owner.window_session().pending_terminal_window_ids(lease)
    });
    for window_id in pending.unwrap_or_default() {
        settle_surface_window_terminal(
            owner,
            runtime,
            lease,
            window_id,
            window_session::DockSurfaceWindowSessionTerminalDisposition::ConfirmedAbsentAfterAppShutdown,
            cx,
        );
    }
}

fn close_surface_anchor_after_dependents(
    owner: &Entity<DockSurfaceOwner>,
    runtime: &DockViewportRuntimeHandle,
    lease: window_session::DockSurfaceWindowSessionLease,
    cx: &mut App,
) {
    let Some(pending) = cx.read_entity(owner, |owner, _| {
        owner.window_session().pending_terminal_window_ids(lease)
    }) else {
        return;
    };
    if pending.iter().any(|window_id| *window_id != lease.anchor()) {
        return;
    }
    if !pending.contains(&lease.anchor()) {
        return;
    }

    let anchor = runtime
        .windows_for_surface(lease)
        .into_iter()
        .find_map(|(role, window)| {
            (role == DockViewportWindowRole::PrimaryAnchor).then_some(window)
        });
    if let Some(anchor) = anchor {
        close_surface_window(owner, lease, anchor, cx);
    }
}

struct DockSurfaceShutdownCloseEffects {
    runtime: DockViewportRuntimeHandle,
    lease: window_session::DockSurfaceWindowSessionLease,
    windows: Vec<(DockViewportWindowRole, open_gpui::AnyWindowHandle)>,
}

fn prepare_surface_shutdown(
    owner: &Entity<DockSurfaceOwner>,
    lease: window_session::DockSurfaceWindowSessionLease,
    reason: DockSurfaceWindowSessionShutdownReason,
    cx: &mut App,
) -> Option<DockSurfaceShutdownCloseEffects> {
    let runtime = cx.read_entity(owner, |owner, _| owner.runtime());
    let snapshot = runtime.windows_for_surface(lease);
    let (begin, activation_settlements) = cx.update_entity(owner, |owner, owner_cx| {
        let begin = owner.window_session_mut().begin_shutdown(
            lease,
            reason,
            snapshot.iter().map(|(_, window)| window.window_id()),
        );
        let activation_settlements = if matches!(
            begin,
            window_session::DockSurfaceWindowSessionBeginShutdownOutcome::Started { .. }
        ) {
            owner_cx.notify();
            owner.activation_mut().freeze_lease(lease)
        } else {
            DockSurfaceActivationSettlements::default()
        };
        (begin, activation_settlements)
    });

    match begin {
        window_session::DockSurfaceWindowSessionBeginShutdownOutcome::Started { .. } => {}
        window_session::DockSurfaceWindowSessionBeginShutdownOutcome::AlreadyShuttingDown => {
            return None;
        }
        window_session::DockSurfaceWindowSessionBeginShutdownOutcome::StaleLease
        | window_session::DockSurfaceWindowSessionBeginShutdownOutcome::NotActive => return None,
    }

    let shutdown_effects = runtime
        .begin_surface_shutdown(lease)
        .expect("an active DockSurface lease must own the matching runtime generation");
    let mut windows = runtime.commit_surface_shutdown(shutdown_effects, cx);
    activation_settlements.deliver(cx);

    let mut seen = Vec::new();
    windows.retain(|(_, window)| {
        let window_id = window.window_id();
        if seen.contains(&window_id) {
            false
        } else {
            seen.push(window_id);
            true
        }
    });
    Some(DockSurfaceShutdownCloseEffects {
        runtime,
        lease,
        windows,
    })
}

fn apply_surface_shutdown_close_effects(
    owner: &Entity<DockSurfaceOwner>,
    effects: DockSurfaceShutdownCloseEffects,
    cx: &mut App,
) {
    for (_, window) in effects
        .windows
        .iter()
        .filter(|(role, _)| *role == DockViewportWindowRole::ManagedViewport)
    {
        close_surface_window(owner, effects.lease, *window, cx);
    }
    close_surface_anchor_after_dependents(owner, &effects.runtime, effects.lease, cx);
}

pub(crate) fn handle_surface_window_closed(
    owner: &Entity<DockSurfaceOwner>,
    window_id: WindowId,
    cx: &mut App,
) {
    let Some(lease) = cx.read_entity(owner, |owner, _| {
        owner.window_session().active_lease_for_anchor(window_id)
    }) else {
        return;
    };
    if let Some(effects) = prepare_surface_shutdown(
        owner,
        lease,
        DockSurfaceWindowSessionShutdownReason::AnchorDestroyed,
        cx,
    ) {
        let owner = owner.clone();
        cx.defer(move |cx| {
            apply_surface_shutdown_close_effects(&owner, effects, cx);
        });
    }
}

fn handle_surface_window_native_terminal(
    owner: &Entity<DockSurfaceOwner>,
    window_id: WindowId,
    cx: &mut App,
) {
    let runtime = cx.read_entity(owner, |owner, _| owner.runtime());
    let lease = cx.read_entity(owner, |owner, _| {
        owner
            .window_session()
            .shutting_down_lease_for_window(window_id)
    });
    let Some(lease) = lease else {
        return;
    };
    settle_surface_window_terminal(
        owner,
        &runtime,
        lease,
        window_id,
        window_session::DockSurfaceWindowSessionTerminalDisposition::ObservedClosed,
        cx,
    );
    if window_id != lease.anchor() {
        let owner = owner.clone();
        cx.defer(move |cx| {
            let runtime = cx.read_entity(&owner, |owner, _| owner.runtime());
            close_surface_anchor_after_dependents(&owner, &runtime, lease, cx);
        });
    }
}

fn handle_surface_app_shutdown(owner: &Entity<DockSurfaceOwner>, cx: &mut App) {
    let runtime = cx.read_entity(owner, |owner, _| owner.runtime());
    let (opening, active, shutting_down) = cx.read_entity(owner, |owner, _| {
        (
            owner.window_session().opening_token(),
            owner.window_session().active_lease(),
            owner.window_session().shutting_down_lease(),
        )
    });
    if let Some(opening) = opening {
        let windows = runtime.abort_surface_opening(opening);
        cx.update_entity(owner, |owner, owner_cx| {
            let _ = owner.window_session_mut().rollback_opening(
                opening,
                DockSurfaceWindowSessionOpeningRollbackReason::AppShutdown,
            );
            owner_cx.notify();
        });
        for window in windows {
            let _ = window.update(cx, |_, window, cx| window.remove_window(cx));
        }
        return;
    }
    let lease = active.or(shutting_down);
    let Some(lease) = lease else {
        return;
    };
    let effects = active
        .and_then(|lease| {
            prepare_surface_shutdown(
                owner,
                lease,
                DockSurfaceWindowSessionShutdownReason::AppShutdown,
                cx,
            )
        })
        .unwrap_or_else(|| DockSurfaceShutdownCloseEffects {
            runtime: runtime.clone(),
            lease,
            windows: runtime.windows_for_surface(lease),
        });
    apply_surface_shutdown_close_effects(owner, effects, cx);

    let owner = owner.clone();
    cx.defer(move |cx| {
        let runtime = cx.read_entity(&owner, |owner, _| owner.runtime());
        confirm_pending_surface_terminals_absent(&owner, &runtime, lease, cx);
    });
}

fn install_primary_window_lifecycle_hooks(
    owner: Entity<DockSurfaceOwner>,
    window: &mut Window,
    cx: &mut App,
) {
    let anchor = window.window_handle().window_id();
    let close_owner = owner.clone();
    window.on_window_should_close(cx, move |_, cx| {
        let lease = cx.read_entity(&close_owner, |owner, _| {
            owner.window_session().active_lease_for_anchor(anchor)
        });
        if let Some(lease) = lease {
            if let Some(effects) = prepare_surface_shutdown(
                &close_owner,
                lease,
                DockSurfaceWindowSessionShutdownReason::AnchorCloseRequested,
                cx,
            ) {
                let owner = close_owner.clone();
                cx.defer(move |cx| {
                    apply_surface_shutdown_close_effects(&owner, effects, cx);
                });
            }
            return false;
        }
        !cx.read_entity(&close_owner, |owner, _| {
            owner
                .window_session()
                .protects_anchor_from_native_close(anchor)
        })
    });

    let presentation_owner = owner;
    window
        .observe_window_initial_presentation(move |window, cx| {
            if window.presentation_facts().initial_presentation
                != WindowInitialPresentationStatus::Rejected
            {
                return;
            }
            let Some(lease) = cx.read_entity(&presentation_owner, |owner, _| {
                owner.window_session().active_lease_for_anchor(anchor)
            }) else {
                return;
            };
            if let Some(effects) = prepare_surface_shutdown(
                &presentation_owner,
                lease,
                DockSurfaceWindowSessionShutdownReason::PresentationFailed,
                cx,
            ) {
                let owner = presentation_owner.clone();
                cx.defer(move |cx| {
                    apply_surface_shutdown_close_effects(&owner, effects, cx);
                });
            }
        })
        .detach();
}

impl DockSurface {
    /// Starts a facade-first docking surface builder for a logical dock space.
    pub fn builder(space: impl Into<DockSpaceId>) -> DockSurfaceBuilder {
        DockSurfaceBuilder::new(space)
    }

    #[cfg(test)]
    pub(crate) fn from_controller(controller: Entity<DockController>, cx: &mut App) -> Self {
        Self::from_controller_with_close_policy_and_visual_style_resolver(
            controller,
            DockViewportClosePolicy::default(),
            None,
            cx,
        )
    }

    pub(crate) fn from_controller_with_close_policy_and_visual_style_resolver(
        controller: Entity<DockController>,
        close_policy: DockViewportClosePolicy,
        visual_style_resolver: Option<DockVisualStyleResolver>,
        cx: &mut App,
    ) -> Self {
        let primary_space = cx.read_entity(&controller, |controller, _| controller.space().clone());
        let owner = cx.new(|cx| {
            let viewport_runtime = DockViewportRuntimeHandle::for_surface(
                controller.clone(),
                cx.entity_id(),
                close_policy,
                visual_style_resolver,
            );
            DockSurfaceOwner::new(
                controller,
                viewport_runtime,
                primary_space.clone(),
                cx.entity_id(),
            )
        });
        let weak_owner = owner.downgrade();
        let runtime = cx.read_entity(&owner, |owner, _| owner.runtime());
        runtime.install_surface_owner(owner.downgrade());
        runtime.install_surface_commit_sink(move |authority, transaction, categories, cx| {
            let Some(owner) = weak_owner.upgrade() else {
                return;
            };
            cx.update_entity(&owner, |owner, owner_cx| {
                let admitted = match authority {
                    DockViewportRuntimeCommitAuthority::Active(work_context) => {
                        match work_context.lineage() {
                            DockViewportRuntimeLineage::Surface(lease) => {
                                owner.window_session().admits(lease)
                            }
                            DockViewportRuntimeLineage::Unmanaged => false,
                        }
                    }
                    DockViewportRuntimeCommitAuthority::FrozenSurfaceShutdown(work_context) => {
                        match work_context.lineage() {
                            DockViewportRuntimeLineage::Surface(lease) => {
                                owner.window_session().shutting_down_lease() == Some(lease)
                            }
                            DockViewportRuntimeLineage::Unmanaged => false,
                        }
                    }
                };
                if !admitted {
                    return;
                }
                if let Some(transaction) = transaction {
                    owner.record_changes(transaction, categories.iter().copied());
                } else {
                    let transaction = owner.begin_root_transaction();
                    owner.record_changes(transaction, categories.iter().copied());
                    owner.finish_root_transaction(transaction, owner_cx);
                }
            });
        });
        let primary_space = cx.read_entity(&owner, |owner, _| owner.primary_space().clone());
        let activation_owner = owner.downgrade();
        cx.on_window_closed(move |cx, window_id| {
            let Some(owner) = activation_owner.upgrade() else {
                return;
            };
            handle_surface_window_closed(&owner, window_id, cx);
            let settlements = cx.update_entity(&owner, |owner, owner_cx| {
                let settlements = owner.activation_mut().window_closed(window_id);
                owner_cx.notify();
                settlements
            });
            settlements.deliver(cx);
        })
        .detach();
        let terminal_owner = owner.downgrade();
        cx.on_window_native_terminal(move |cx, window_id| {
            if let Some(owner) = terminal_owner.upgrade() {
                handle_surface_window_native_terminal(&owner, window_id, cx);
            }
        })
        .detach();
        let shutdown_owner = owner.downgrade();
        cx.on_app_quit(move |cx| {
            if let Some(owner) = shutdown_owner.upgrade() {
                handle_surface_app_shutdown(&owner, cx);
            }
            std::future::ready(())
        })
        .detach();
        Self {
            owner,
            primary_space,
        }
    }

    pub(crate) fn controller<C: AppContext>(&self, cx: &C) -> Entity<DockController> {
        cx.read_entity(&self.owner, |owner, _| owner.controller())
    }

    pub(crate) fn viewport_runtime<C: AppContext>(&self, cx: &C) -> DockViewportRuntimeHandle {
        cx.read_entity(&self.owner, |owner, _| owner.runtime())
    }

    pub(crate) fn owner(&self) -> &Entity<DockSurfaceOwner> {
        &self.owner
    }

    /// Returns a read-only snapshot of this surface's primary-window session.
    pub fn window_session_status(&self, cx: &App) -> DockSurfaceWindowSessionStatus {
        cx.read_entity(&self.owner, |owner, _| owner.window_session().status())
    }

    /// Returns the default logical dock space for primary host windows.
    pub fn primary_space(&self) -> &DockSpaceId {
        &self.primary_space
    }

    #[cfg(test)]
    pub(crate) fn primary_host(&self, cx: &mut Context<DockHost>) -> DockHost {
        self.host(self.primary_space.clone(), cx)
    }

    pub(crate) fn host(
        &self,
        space: impl Into<DockSpaceId>,
        cx: &mut Context<DockHost>,
    ) -> DockHost {
        let controller = cx.read_entity(&self.owner, |owner, _| owner.controller());
        let viewport_runtime = cx.read_entity(&self.owner, |owner, _| owner.runtime());
        DockHost::from_embedded_surface_owner(controller, space, viewport_runtime, &self.owner, cx)
    }

    fn opening_primary_host(
        &self,
        opening: window_session::DockSurfaceWindowSessionOpeningToken,
        cx: &mut Context<DockHost>,
    ) -> DockHost {
        let controller = cx.read_entity(&self.owner, |owner, _| owner.controller());
        let viewport_runtime = cx.read_entity(&self.owner, |owner, _| owner.runtime());
        DockHost::from_opening_primary_surface_owner(
            controller,
            self.primary_space.clone(),
            viewport_runtime,
            &self.owner,
            opening,
            cx,
        )
    }

    /// Returns the latest committed persistence revision shared by all surface clones.
    pub fn revision(&self, cx: &App) -> u64 {
        cx.read_entity(&self.owner, |owner, _| owner.revision())
    }

    /// Subscribes to lightweight metadata for committed surface changes.
    ///
    /// Applications own debounce, snapshot export, storage, and file-I/O policy. Dropping the
    /// returned subscription only stops observation.
    pub fn subscribe_changes(
        &self,
        cx: &mut App,
        on_event: impl FnMut(&DockSurfaceChangeEvent, &mut App) + 'static,
    ) -> Subscription {
        owner::subscribe(&self.owner, cx, on_event)
    }

    /// Creates an erased GPUI view that renders the primary dock space inside an existing window.
    pub fn host_view(&self, cx: &mut App) -> AnyView {
        self.host_view_for_space(self.primary_space.clone(), cx)
    }

    /// Creates an erased GPUI view that renders one logical dock space inside an existing window.
    pub fn host_view_for_space(&self, space: impl Into<DockSpaceId>, cx: &mut App) -> AnyView {
        let surface = self.clone();
        let space = space.into();
        cx.new(move |cx| surface.host(space, cx)).into()
    }

    /// Opens a normal GPUI window that renders the primary dock host.
    ///
    /// This is for the main application window and does not require platform viewport-window
    /// capability. Detached platform viewports are opened through the viewport-runtime path.
    pub fn open_primary_window(
        &self,
        options: WindowOptions,
        cx: &mut App,
    ) -> DockSurfacePrimaryWindowOpenOutcome {
        let opening = match cx.update_entity(&self.owner, |owner, owner_cx| {
            let result = owner.window_session_mut().reserve_opening();
            if result.is_ok() {
                owner_cx.notify();
            }
            result
        }) {
            Ok(opening) => opening,
            Err(conflict) => {
                return DockSurfacePrimaryWindowOpenOutcome::Unavailable(
                    DockSurfacePrimaryWindowUnavailable::Conflict(conflict),
                );
            }
        };

        let surface = self.clone();
        let runtime = self.viewport_runtime(cx);
        let opening_attempt = Rc::new(Cell::new(None));
        let opening_attempt_for_builder = opening_attempt.clone();
        let opening_runtime = runtime.clone();
        let open_result = catch_unwind(AssertUnwindSafe(|| {
            cx.open_window_detailed(options, move |window, cx| {
                opening_attempt_for_builder.set(
                    opening_runtime
                        .begin_primary_anchor_open_attempt(window.window_handle(), opening),
                );
                let lifecycle_owner = surface.owner().clone();
                let host = cx.new(move |cx| surface.opening_primary_host(opening, cx));
                install_primary_window_lifecycle_hooks(lifecycle_owner, window, cx);
                host
            })
        }));
        let window = match open_result {
            Err(payload) => {
                if let Some(attempt) = opening_attempt.take() {
                    let _ = runtime.abort_window_open_attempt(attempt);
                }
                cx.update_entity(&self.owner, |owner, owner_cx| {
                    let _ = owner.window_session_mut().rollback_opening(
                        opening,
                        DockSurfaceWindowSessionOpeningRollbackReason::Panicked,
                    );
                    owner_cx.notify();
                });
                resume_unwind(payload);
            }
            Ok(Ok(window)) => window,
            Ok(Err(error)) => {
                if let Some(attempt) = opening_attempt.take() {
                    let _ = runtime.abort_window_open_attempt(attempt);
                }
                let reason = match error.stage() {
                    WindowOpenFailureStage::AppShutdown => {
                        DockSurfaceWindowSessionOpeningRollbackReason::AppShutdown
                    }
                    WindowOpenFailureStage::ClosedDuringNativeCreateOrMap
                    | WindowOpenFailureStage::ClosedDuringBuild
                    | WindowOpenFailureStage::ClosedDuringInitialDraw
                    | WindowOpenFailureStage::ClosedDuringInitialPresentation => {
                        DockSurfaceWindowSessionOpeningRollbackReason::ClosedDuringOpening
                    }
                    WindowOpenFailureStage::BeforeVisibilityPresentation => {
                        DockSurfaceWindowSessionOpeningRollbackReason::PresentationFailedBeforeVisibility
                    }
                    WindowOpenFailureStage::NativeCreateOrMap
                    | WindowOpenFailureStage::CommitRejected => {
                        DockSurfaceWindowSessionOpeningRollbackReason::WindowOpenFailed
                    }
                    _ => DockSurfaceWindowSessionOpeningRollbackReason::WindowOpenFailed,
                };
                cx.update_entity(&self.owner, |owner, owner_cx| {
                    let _ = owner.window_session_mut().rollback_opening(opening, reason);
                    owner_cx.notify();
                });
                return DockSurfacePrimaryWindowOpenOutcome::Unavailable(
                    DockSurfacePrimaryWindowUnavailable::OpeningRolledBack {
                        reason,
                        message: error.to_string(),
                    },
                );
            }
        };
        let Some(opening_attempt) = opening_attempt.take() else {
            crate::close_window_quietly(window.into(), cx);
            let reason = DockSurfaceWindowSessionOpeningRollbackReason::WindowOpenFailed;
            cx.update_entity(&self.owner, |owner, owner_cx| {
                let _ = owner.window_session_mut().rollback_opening(opening, reason);
                owner_cx.notify();
            });
            return DockSurfacePrimaryWindowOpenOutcome::Unavailable(
                DockSurfacePrimaryWindowUnavailable::OpeningRolledBack {
                    reason,
                    message: "Dock primary opening handle could not be reserved".to_string(),
                },
            );
        };
        let anchor = window.window_id();
        let host = window
            .entity(cx)
            .expect("a committed Dock primary window must retain its opening host");
        let host_can_promote = cx.read_entity(&host, |host, _| {
            host.can_promote_primary_anchor(opening, anchor)
        });
        assert!(
            host_can_promote,
            "committed Dock primary window lost its exact opening host authority"
        );

        let lease = cx.update_entity(&self.owner, |owner, owner_cx| {
            let lease = owner
                .window_session_mut()
                .commit_opening(opening, anchor)
                .expect("validated Dock primary opening changed before activation");
            assert!(
                owner.activation_mut().activate_lease(lease),
                "Dock primary activation must arm the matching surface activation lease"
            );
            owner_cx.notify();
            lease
        });
        let lineage_activation = runtime.activate_surface_lineage(lease);
        assert_eq!(
            lineage_activation,
            crate::DockViewportRuntimeLineageActivationOutcome::Activated,
            "Dock primary activation must arm the matching surface runtime exactly once"
        );
        assert!(
            runtime.promote_primary_anchor_open_attempt(opening_attempt, lease),
            "Dock primary opening handle must promote into the exact active session lease"
        );
        let promoted = cx.update_entity(&host, |host, host_cx| {
            host.promote_primary_anchor(opening, lease, anchor, host_cx)
        });
        assert!(
            promoted,
            "validated Dock primary host rejected its exact active session lease"
        );
        let _ = window.update(cx, |_, window, _| window.refresh());

        DockSurfacePrimaryWindowOpenOutcome::Opened(DockSurfacePrimaryWindowOpened::new(
            window.into(),
            lease.generation(),
        ))
    }

    /// Returns default window options for a centered primary dock host.
    pub fn primary_window_options(bounds: Bounds<Pixels>) -> WindowOptions {
        WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            ..Default::default()
        }
    }
}
