//! Compensating presentation executor for committed payload recovery.
//!
//! Durable recovery records remain owned by `DockPayloadRecoveryRegistry`. This executor owns
//! only the single transient attempt which rehosts already-resolved panel roots before the graph
//! and recovery record cross their final commit boundary.

use super::{
    DockSurfaceOwner,
    owner::with_detached_root_transaction,
    payload_recovery::{
        DockPayloadRecoveryRestoreAction, DockPayloadRecoveryRestoreError,
        DockPayloadRecoveryRestorePrepared, DockPayloadRecoveryRestoreReceipt,
    },
};
use crate::{
    DockController, DockHost, DockSpaceId,
    host::{
        DockHostRecoveryPresentationKey, DockHostRecoveryPresentationMode,
        DockHostRecoverySourceRestorationInstallOutcome, DockHostRecoverySourceRestorationPhase,
        DockHostWindowBinding,
    },
    host_render_session::DockHostPresentationSession,
    viewport_registry::DockViewportRegistrationKey,
};
use open_gpui::{
    AnyView, App, AppContext as _, Entity, WeakEntity, WindowHandle, view_presentation_window,
};
use std::num::NonZeroU64;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct DockPayloadRecoveryExecutionKey {
    sequence: NonZeroU64,
    action: DockPayloadRecoveryRestoreAction,
}

impl DockPayloadRecoveryExecutionKey {
    pub(crate) const fn sequence(self) -> NonZeroU64 {
        self.sequence
    }

    pub(crate) const fn action(self) -> DockPayloadRecoveryRestoreAction {
        self.action
    }
}

#[derive(Clone, Debug)]
pub(crate) struct DockPayloadRecoveryEndpoint {
    host: WeakEntity<DockHost>,
    window: WindowHandle<DockHost>,
    binding: DockHostWindowBinding,
    space: DockSpaceId,
    registration: Option<DockViewportRegistrationKey>,
}

impl DockPayloadRecoveryEndpoint {
    pub(crate) fn new(
        host: WeakEntity<DockHost>,
        window: WindowHandle<DockHost>,
        binding: DockHostWindowBinding,
        space: DockSpaceId,
        registration: Option<DockViewportRegistrationKey>,
    ) -> Option<Self> {
        let window_id = window.window_id();
        if binding.window_id() != window_id
            || registration
                .as_ref()
                .is_some_and(|registration| registration.window_id() != window_id)
        {
            return None;
        }
        Some(Self {
            host,
            window,
            binding,
            space,
            registration,
        })
    }

    pub(crate) fn host(&self) -> &WeakEntity<DockHost> {
        &self.host
    }

    pub(crate) const fn window(&self) -> WindowHandle<DockHost> {
        self.window
    }

    pub(crate) const fn binding(&self) -> DockHostWindowBinding {
        self.binding
    }

    pub(crate) fn space(&self) -> &DockSpaceId {
        &self.space
    }

    pub(crate) fn registration(&self) -> Option<&DockViewportRegistrationKey> {
        self.registration.as_ref()
    }
}

#[derive(Clone, Debug)]
pub(crate) struct DockPayloadRecoveryTransfer {
    key: DockPayloadRecoveryExecutionKey,
    restore: DockPayloadRecoveryRestorePrepared,
    source: DockPayloadRecoveryEndpoint,
    destination: DockPayloadRecoveryEndpoint,
    roots: Vec<AnyView>,
    prepared: view_presentation_window::PreparedRehost,
    source_presentation: DockHostRecoveryPresentationKey,
    destination_presentation: DockHostRecoveryPresentationKey,
}

impl DockPayloadRecoveryTransfer {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        key: DockPayloadRecoveryExecutionKey,
        restore: DockPayloadRecoveryRestorePrepared,
        source: DockPayloadRecoveryEndpoint,
        destination: DockPayloadRecoveryEndpoint,
        roots: Vec<AnyView>,
        prepared: view_presentation_window::PreparedRehost,
        source_presentation: DockHostRecoveryPresentationKey,
        destination_presentation: DockHostRecoveryPresentationKey,
    ) -> Option<Self> {
        if roots.is_empty()
            || restore.action() != key.action()
            || prepared.source().window_id() != source.window().window_id()
            || prepared.destination().window_id() != destination.window().window_id()
            || source_presentation.rehost_generation() != prepared.generation()
            || destination_presentation.rehost_generation() != prepared.generation()
        {
            return None;
        }
        Some(Self {
            key,
            restore,
            source,
            destination,
            roots,
            prepared,
            source_presentation,
            destination_presentation,
        })
    }

    pub(crate) const fn key(&self) -> DockPayloadRecoveryExecutionKey {
        self.key
    }

    pub(crate) fn restore(&self) -> &DockPayloadRecoveryRestorePrepared {
        &self.restore
    }

    pub(crate) fn source(&self) -> &DockPayloadRecoveryEndpoint {
        &self.source
    }

    pub(crate) fn destination(&self) -> &DockPayloadRecoveryEndpoint {
        &self.destination
    }

    pub(crate) fn roots(&self) -> &[AnyView] {
        &self.roots
    }

    pub(crate) fn prepared(&self) -> &view_presentation_window::PreparedRehost {
        &self.prepared
    }

    pub(crate) const fn source_presentation(&self) -> DockHostRecoveryPresentationKey {
        self.source_presentation
    }

    pub(crate) const fn destination_presentation(&self) -> DockHostRecoveryPresentationKey {
        self.destination_presentation
    }

    fn matches_presentation(&self, key: DockHostRecoveryPresentationKey) -> bool {
        self.source_presentation == key || self.destination_presentation == key
    }
}

#[derive(Debug)]
struct PreparedPayloadRecoveryInstallation {
    prepared: view_presentation_window::PreparedRehost,
    source: Option<DockPayloadRecoveryEndpoint>,
    destination: DockPayloadRecoveryEndpoint,
    source_presentation: Option<DockHostRecoveryPresentationKey>,
    destination_presentation: Option<DockHostRecoveryPresentationKey>,
}

impl PreparedPayloadRecoveryInstallation {
    fn new(
        prepared: view_presentation_window::PreparedRehost,
        destination: DockPayloadRecoveryEndpoint,
    ) -> Self {
        Self {
            prepared,
            source: None,
            destination,
            source_presentation: None,
            destination_presentation: None,
        }
    }

    fn prepared(&self) -> &view_presentation_window::PreparedRehost {
        &self.prepared
    }

    fn set_source(&mut self, source: DockPayloadRecoveryEndpoint) {
        debug_assert!(self.source.is_none());
        self.source = Some(source);
    }

    fn record_source_presentation(&mut self, key: DockHostRecoveryPresentationKey) {
        debug_assert!(self.source_presentation.is_none());
        self.source_presentation = Some(key);
    }

    fn record_destination_presentation(&mut self, key: DockHostRecoveryPresentationKey) {
        debug_assert!(self.destination_presentation.is_none());
        self.destination_presentation = Some(key);
    }

    fn compensate(
        self,
        owner: &Entity<DockSurfaceOwner>,
        controller: &Entity<DockController>,
        cx: &mut App,
    ) {
        let source_is_exact = self
            .source
            .as_ref()
            .is_some_and(|source| source_endpoint_is_exact(source, owner, controller, cx));
        let authority_retired = if source_is_exact {
            retire_prepared_rehost_to_source(&self.prepared, cx)
        } else {
            abandon_prepared_rehost_after_source_loss(&self.prepared, cx)
        };
        assert!(
            authority_retired,
            "prepared payload recovery must retire presentation authority before installation fails"
        );

        if let (Some(source), Some(key)) = (&self.source, self.source_presentation) {
            let _ = abandon_host_presentation(source.host(), key, cx);
        }
        if let Some(key) = self.destination_presentation {
            let _ = abandon_host_presentation(self.destination.host(), key, cx);
        }
    }
}

#[derive(Debug)]
enum DockPayloadRecoveryExecutionState {
    Idle,
    Reserved(DockPayloadRecoveryExecutionKey),
    Rehosting {
        transfer: DockPayloadRecoveryTransfer,
        finalization_queued: bool,
        source_logically_closed: bool,
        source_native_terminal_seen: bool,
    },
}

impl Default for DockPayloadRecoveryExecutionState {
    fn default() -> Self {
        Self::Idle
    }
}

#[derive(Debug, Default)]
pub(crate) struct DockPayloadRecoveryExecutor {
    last_sequence: u64,
    state: DockPayloadRecoveryExecutionState,
    #[cfg(test)]
    pause_after_source_release_once: bool,
    #[cfg(test)]
    pause_after_source_restoration_once: bool,
    #[cfg(test)]
    replace_source_host_after_finish_once: bool,
    #[cfg(test)]
    reject_next_transfer_install_once: bool,
}

impl DockPayloadRecoveryExecutor {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn reserve(
        &mut self,
        action: DockPayloadRecoveryRestoreAction,
    ) -> Result<DockPayloadRecoveryExecutionKey, DockPayloadRecoveryRestoreError> {
        match &self.state {
            DockPayloadRecoveryExecutionState::Idle => {}
            DockPayloadRecoveryExecutionState::Reserved(current) if current.action() == action => {
                return Err(DockPayloadRecoveryRestoreError::AlreadyInFlight);
            }
            DockPayloadRecoveryExecutionState::Rehosting { transfer, .. }
                if transfer.key().action() == action =>
            {
                return Err(DockPayloadRecoveryRestoreError::AlreadyInFlight);
            }
            DockPayloadRecoveryExecutionState::Reserved(_)
            | DockPayloadRecoveryExecutionState::Rehosting { .. } => {
                return Err(DockPayloadRecoveryRestoreError::Busy);
            }
        }

        self.last_sequence = self
            .last_sequence
            .checked_add(1)
            .expect("payload recovery execution identity space exhausted");
        let key = DockPayloadRecoveryExecutionKey {
            sequence: NonZeroU64::new(self.last_sequence)
                .expect("payload recovery execution sequence must be non-zero"),
            action,
        };
        self.state = DockPayloadRecoveryExecutionState::Reserved(key);
        Ok(key)
    }

    pub(crate) fn install_transfer(
        &mut self,
        key: DockPayloadRecoveryExecutionKey,
        transfer: DockPayloadRecoveryTransfer,
    ) -> bool {
        if !matches!(self.state, DockPayloadRecoveryExecutionState::Reserved(current) if current == key)
            || transfer.key() != key
        {
            return false;
        }
        #[cfg(test)]
        if std::mem::take(&mut self.reject_next_transfer_install_once) {
            return false;
        }
        self.state = DockPayloadRecoveryExecutionState::Rehosting {
            transfer,
            finalization_queued: false,
            source_logically_closed: false,
            source_native_terminal_seen: false,
        };
        true
    }

    pub(crate) fn transfer(
        &self,
        key: DockPayloadRecoveryExecutionKey,
    ) -> Option<&DockPayloadRecoveryTransfer> {
        match &self.state {
            DockPayloadRecoveryExecutionState::Rehosting { transfer, .. }
                if transfer.key() == key =>
            {
                Some(transfer)
            }
            DockPayloadRecoveryExecutionState::Idle
            | DockPayloadRecoveryExecutionState::Reserved(_)
            | DockPayloadRecoveryExecutionState::Rehosting { .. } => None,
        }
    }

    pub(crate) fn transfer_for_presentation(
        &self,
        key: DockHostRecoveryPresentationKey,
    ) -> Option<&DockPayloadRecoveryTransfer> {
        match &self.state {
            DockPayloadRecoveryExecutionState::Rehosting { transfer, .. }
                if transfer.matches_presentation(key) =>
            {
                Some(transfer)
            }
            DockPayloadRecoveryExecutionState::Idle
            | DockPayloadRecoveryExecutionState::Reserved(_)
            | DockPayloadRecoveryExecutionState::Rehosting { .. } => None,
        }
    }

    pub(crate) fn reserve_finalization(
        &mut self,
        key: DockHostRecoveryPresentationKey,
    ) -> Option<DockPayloadRecoveryTransfer> {
        let DockPayloadRecoveryExecutionState::Rehosting {
            transfer,
            finalization_queued,
            ..
        } = &mut self.state
        else {
            return None;
        };
        if *finalization_queued || transfer.destination_presentation() != key {
            return None;
        }
        *finalization_queued = true;
        Some(transfer.clone())
    }

    pub(crate) fn record_source_native_terminal(
        &mut self,
        window_id: open_gpui::WindowId,
    ) -> Option<DockPayloadRecoveryTransfer> {
        let DockPayloadRecoveryExecutionState::Rehosting {
            transfer,
            source_native_terminal_seen,
            ..
        } = &mut self.state
        else {
            return None;
        };
        if transfer.source().window().window_id() != window_id {
            return None;
        }
        *source_native_terminal_seen = true;
        Some(transfer.clone())
    }

    pub(crate) fn record_source_logical_close(
        &mut self,
        window_id: open_gpui::WindowId,
    ) -> Option<DockPayloadRecoveryTransfer> {
        let DockPayloadRecoveryExecutionState::Rehosting {
            transfer,
            source_logically_closed,
            ..
        } = &mut self.state
        else {
            return None;
        };
        if transfer.source().window().window_id() != window_id {
            return None;
        }
        *source_logically_closed = true;
        Some(transfer.clone())
    }

    pub(crate) fn source_close_state(
        &self,
        key: DockPayloadRecoveryExecutionKey,
    ) -> Option<(bool, bool)> {
        match &self.state {
            DockPayloadRecoveryExecutionState::Rehosting {
                transfer,
                source_logically_closed,
                source_native_terminal_seen,
                ..
            } if transfer.key() == key => {
                Some((*source_logically_closed, *source_native_terminal_seen))
            }
            DockPayloadRecoveryExecutionState::Idle
            | DockPayloadRecoveryExecutionState::Reserved(_)
            | DockPayloadRecoveryExecutionState::Rehosting { .. } => None,
        }
    }

    pub(crate) fn source_native_terminal_seen(&self, key: DockPayloadRecoveryExecutionKey) -> bool {
        matches!(
            &self.state,
            DockPayloadRecoveryExecutionState::Rehosting {
                transfer,
                source_native_terminal_seen: true,
                ..
            } if transfer.key() == key
        )
    }

    #[cfg(test)]
    pub(crate) fn pause_after_source_release_once_for_test(&mut self) {
        self.pause_after_source_release_once = true;
    }

    #[cfg(test)]
    pub(crate) fn take_pause_after_source_release_for_test(&mut self) -> bool {
        std::mem::take(&mut self.pause_after_source_release_once)
    }

    #[cfg(test)]
    pub(crate) fn pause_after_source_restoration_once_for_test(&mut self) {
        self.pause_after_source_restoration_once = true;
    }

    #[cfg(test)]
    pub(crate) fn take_pause_after_source_restoration_for_test(&mut self) -> bool {
        std::mem::take(&mut self.pause_after_source_restoration_once)
    }

    #[cfg(test)]
    pub(crate) fn replace_source_host_after_finish_once_for_test(&mut self) {
        self.replace_source_host_after_finish_once = true;
    }

    #[cfg(test)]
    pub(crate) fn take_replace_source_host_after_finish_for_test(&mut self) -> bool {
        std::mem::take(&mut self.replace_source_host_after_finish_once)
    }

    #[cfg(test)]
    pub(crate) fn reject_next_transfer_install_once_for_test(&mut self) {
        self.reject_next_transfer_install_once = true;
    }

    pub(crate) fn is_reserved(&self, key: DockPayloadRecoveryExecutionKey) -> bool {
        matches!(self.state, DockPayloadRecoveryExecutionState::Reserved(current) if current == key)
    }

    pub(crate) fn finish(&mut self, key: DockPayloadRecoveryExecutionKey) -> bool {
        let current = match &self.state {
            DockPayloadRecoveryExecutionState::Reserved(current) => *current,
            DockPayloadRecoveryExecutionState::Rehosting { transfer, .. } => transfer.key(),
            DockPayloadRecoveryExecutionState::Idle => return false,
        };
        if current != key {
            return false;
        }
        self.state = DockPayloadRecoveryExecutionState::Idle;
        true
    }

    #[cfg(test)]
    pub(crate) fn execution_snapshot_for_test(
        &self,
    ) -> Option<(
        DockPayloadRecoveryExecutionKey,
        view_presentation_window::RehostPhase,
        bool,
        open_gpui::WindowId,
        open_gpui::WindowId,
        bool,
    )> {
        let DockPayloadRecoveryExecutionState::Rehosting {
            transfer,
            source_logically_closed,
            source_native_terminal_seen,
            ..
        } = &self.state
        else {
            return None;
        };
        Some((
            transfer.key(),
            transfer.prepared().snapshot().phase(),
            *source_native_terminal_seen,
            transfer.source().window().window_id(),
            transfer.destination().window().window_id(),
            *source_logically_closed,
        ))
    }
}

pub(crate) fn start_payload_recovery_restore(
    owner: Entity<DockSurfaceOwner>,
    primary_host: WeakEntity<DockHost>,
    primary_window: WindowHandle<DockHost>,
    primary_binding: DockHostWindowBinding,
    action: DockPayloadRecoveryRestoreAction,
    cx: &mut App,
) -> Result<(), DockPayloadRecoveryRestoreError> {
    let (execution, restore) = cx.update_entity(&owner, |owner, owner_cx| {
        owner.reserve_payload_recovery_restore(action, owner_cx)
    })?;
    let result = start_prepared_payload_recovery_restore(
        &owner,
        primary_host,
        primary_window,
        primary_binding,
        execution,
        restore,
        cx,
    );
    if result.is_err() {
        let _ = cx.update_entity(&owner, |owner, _| {
            owner.cancel_payload_recovery_execution(execution)
        });
    }
    result
}

#[allow(clippy::too_many_arguments)]
fn start_prepared_payload_recovery_restore(
    owner: &Entity<DockSurfaceOwner>,
    primary_host: WeakEntity<DockHost>,
    primary_window: WindowHandle<DockHost>,
    primary_binding: DockHostWindowBinding,
    execution: DockPayloadRecoveryExecutionKey,
    restore: DockPayloadRecoveryRestorePrepared,
    cx: &mut App,
) -> Result<(), DockPayloadRecoveryRestoreError> {
    let controller = cx.read_entity(owner, |owner, _| owner.controller());
    let primary_entity = primary_window
        .entity(cx)
        .map_err(|_| DockPayloadRecoveryRestoreError::PresentationEndpointUnavailable)?;
    let Some(primary_weak_entity) = primary_host.upgrade() else {
        return Err(DockPayloadRecoveryRestoreError::PresentationEndpointUnavailable);
    };
    if primary_entity != primary_weak_entity {
        return Err(DockPayloadRecoveryRestoreError::PresentationEndpointUnavailable);
    }

    let primary_registration = cx.read_entity(&primary_entity, |host, _| {
        let exact = host.controller_entity() == controller
            && host.accepts_payload_recovery_destination_endpoint(
                owner.entity_id(),
                execution.action(),
                primary_binding,
            )
            && host.live_presentation_state().is_none()
            && host.payload_recovery_presentation_state().is_none();
        exact.then(|| host.current_viewport_registration())
    });
    let Some(primary_registration) = primary_registration else {
        return Err(DockPayloadRecoveryRestoreError::PresentationEndpointUnavailable);
    };
    let destination = DockPayloadRecoveryEndpoint::new(
        primary_host,
        primary_window,
        primary_binding,
        restore.primary_space().clone(),
        primary_registration,
    )
    .ok_or(DockPayloadRecoveryRestoreError::PresentationEndpointUnavailable)?;

    let origin = restore
        .presentation_origin()
        .cloned()
        .ok_or(DockPayloadRecoveryRestoreError::PresentationOriginUnavailable)?;
    let (resolved_roots, source_session, destination_session) =
        cx.read_entity(&controller, |controller, _| {
            let workspace = controller.workspace();
            let resolved_roots = restore
                .items()
                .iter()
                .filter_map(|item| workspace.panels().resolved_render_view(item))
                .collect::<Vec<_>>();
            let source_session = DockHostPresentationSession::payload_recovery_projection(
                restore.source_space().clone(),
                restore.projected_graph(),
                workspace,
            );
            let destination_session = DockHostPresentationSession::payload_recovery_projection(
                restore.primary_space().clone(),
                restore.projected_graph(),
                workspace,
            );
            (resolved_roots, source_session, destination_session)
        });

    if resolved_roots.is_empty() || origin.window().window_id() == primary_window.window_id() {
        return commit_payload_recovery_without_rehost(
            owner,
            &controller,
            &destination,
            execution,
            restore,
            cx,
        );
    }

    let prepared = match view_presentation_window::prepare_resolved_view_rehost(
        cx,
        &resolved_roots,
        origin.window().window_id(),
        primary_window.window_id(),
    )
    .map_err(DockPayloadRecoveryRestoreError::PresentationPrepare)?
    {
        view_presentation_window::ResolvedViewRehostOutcome::NoTransfer => {
            return commit_payload_recovery_without_rehost(
                owner,
                &controller,
                &destination,
                execution,
                restore,
                cx,
            );
        }
        view_presentation_window::ResolvedViewRehostOutcome::Prepared(prepared) => prepared,
    };

    let mut installation = PreparedPayloadRecoveryInstallation::new(prepared, destination.clone());
    let source_entity = match origin.window().entity(cx) {
        Ok(source_entity) => source_entity,
        Err(_) => {
            installation.compensate(owner, &controller, cx);
            return Err(DockPayloadRecoveryRestoreError::PresentationEndpointUnavailable);
        }
    };
    let source_registration = origin.registration().clone();
    let source_is_exact = cx.read_entity(&source_entity, |host, _| {
        host.controller_entity() == controller
            && host.accepts_payload_recovery_source_endpoint(
                owner.entity_id(),
                restore.source_space(),
                origin.binding(),
                &source_registration,
            )
            && host.live_presentation_state().is_none()
            && host.payload_recovery_presentation_state().is_none()
    });
    if !source_is_exact {
        installation.compensate(owner, &controller, cx);
        return Err(DockPayloadRecoveryRestoreError::PresentationEndpointUnavailable);
    }
    let Some(source) = DockPayloadRecoveryEndpoint::new(
        source_entity.downgrade(),
        origin.window(),
        origin.binding(),
        restore.source_space().clone(),
        Some(source_registration),
    ) else {
        installation.compensate(owner, &controller, cx);
        return Err(DockPayloadRecoveryRestoreError::PresentationEndpointUnavailable);
    };
    installation.set_source(source.clone());
    let prepared = installation.prepared().clone();

    let transfer = (|| {
        let destination_key = primary_window
            .update(cx, |host, window, host_cx| {
                (window.window_handle().window_id() == primary_binding.window_id()
                    && host.controller_entity() == controller
                    && host.accepts_payload_recovery_destination_endpoint(
                        owner.entity_id(),
                        execution.action(),
                        primary_binding,
                    ))
                .then(|| {
                    host.install_payload_recovery_destination_projection(
                        primary_binding,
                        execution.action(),
                        destination_session,
                        prepared.clone(),
                        prepared.destination().clone(),
                        resolved_roots.clone(),
                        host_cx,
                    )
                })
                .flatten()
            })
            .ok()
            .flatten()
            .ok_or(DockPayloadRecoveryRestoreError::PresentationInstallRejected)?;
        installation.record_destination_presentation(destination_key);

        let source_key = origin
            .window()
            .update(cx, |host, window, host_cx| {
                (window.window_handle().window_id() == origin.binding().window_id()
                    && host.controller_entity() == controller
                    && host.accepts_payload_recovery_source_endpoint(
                        owner.entity_id(),
                        restore.source_space(),
                        origin.binding(),
                        origin.registration(),
                    ))
                .then(|| {
                    host.install_payload_recovery_source_projection(
                        origin.binding(),
                        execution.action(),
                        source_session,
                        prepared.clone(),
                        host_cx,
                    )
                })
                .flatten()
            })
            .ok()
            .flatten()
            .ok_or(DockPayloadRecoveryRestoreError::PresentationInstallRejected)?;
        installation.record_source_presentation(source_key);

        let transfer = DockPayloadRecoveryTransfer::new(
            execution,
            restore,
            source,
            destination,
            resolved_roots,
            prepared,
            source_key,
            destination_key,
        )
        .ok_or(DockPayloadRecoveryRestoreError::PresentationInstallRejected)?;
        cx.update_entity(owner, |owner, _| {
            owner.install_payload_recovery_transfer(execution, transfer.clone())
        })
        .then_some(transfer)
        .ok_or(DockPayloadRecoveryRestoreError::PresentationInstallRejected)
    })();
    let transfer = match transfer {
        Ok(transfer) => transfer,
        Err(error) => {
            installation.compensate(owner, &controller, cx);
            return Err(error);
        }
    };

    refresh_endpoint(transfer.destination(), cx);
    refresh_endpoint(transfer.source(), cx);
    Ok(())
}

fn commit_payload_recovery_without_rehost(
    owner: &Entity<DockSurfaceOwner>,
    controller: &Entity<DockController>,
    destination: &DockPayloadRecoveryEndpoint,
    execution: DockPayloadRecoveryExecutionKey,
    restore: DockPayloadRecoveryRestorePrepared,
    cx: &mut App,
) -> Result<(), DockPayloadRecoveryRestoreError> {
    if !destination_endpoint_is_exact(destination, owner, controller, execution.action(), cx) {
        return Err(DockPayloadRecoveryRestoreError::PresentationEndpointUnavailable);
    }
    let projected_graph = restore.projected_graph().clone();
    let prepared = cx.update_entity(owner, |owner, owner_cx| {
        owner.prepare_payload_recovery_restore_final(execution, restore, owner_cx)
    })?;
    let receipt = with_detached_root_transaction(owner, cx, |transaction, cx| {
        cx.update_entity(controller, |controller, controller_cx| {
            controller.workspace_mut().set_graph(projected_graph);
            controller_cx.notify();
        });
        cx.update_entity(owner, |owner, owner_cx| {
            owner.commit_payload_recovery_restore_final(transaction, prepared, owner_cx)
        })
    });
    install_recovery_focus(owner, destination, execution.action(), &receipt, cx);
    Ok(())
}

pub(crate) fn payload_recovery_source_proxy_committed(
    owner: WeakEntity<DockSurfaceOwner>,
    host: WeakEntity<DockHost>,
    key: DockHostRecoveryPresentationKey,
    prepared: view_presentation_window::PreparedRehost,
    accepted_frame: u64,
    cx: &mut App,
) {
    cx.defer(move |cx| {
        handle_payload_recovery_source_proxy_committed(
            owner,
            host,
            key,
            prepared,
            accepted_frame,
            cx,
        );
    });
}

fn handle_payload_recovery_source_proxy_committed(
    owner: WeakEntity<DockSurfaceOwner>,
    host: WeakEntity<DockHost>,
    key: DockHostRecoveryPresentationKey,
    prepared: view_presentation_window::PreparedRehost,
    accepted_frame: u64,
    cx: &mut App,
) {
    let Some((owner, transfer)) = transfer_for_presentation(&owner, key, &prepared, cx) else {
        return;
    };
    if key != transfer.source_presentation() || transfer.source().host() != &host {
        return;
    }
    let Some(receipt) = prepared.committed_source_proxy() else {
        payload_recovery_presentation_failed(owner.downgrade(), host, key, prepared, cx);
        return;
    };
    if receipt.rehost_generation() != prepared.generation()
        || receipt.source_window() != transfer.source().window().window_id()
        || receipt.frame_generation() != accepted_frame
    {
        payload_recovery_presentation_failed(owner.downgrade(), host, key, prepared, cx);
        return;
    }
    let controller = cx.read_entity(&owner, |owner, _| owner.controller());
    if !source_endpoint_is_exact(transfer.source(), &owner, &controller, cx) {
        payload_recovery_presentation_failed(owner.downgrade(), host, key, prepared, cx);
        return;
    }
    let frozen = host
        .update(cx, |host, host_cx| {
            host.mark_payload_recovery_source_frozen(key, host_cx)
        })
        .unwrap_or(false);
    if !frozen {
        payload_recovery_presentation_failed(owner.downgrade(), host, key, prepared, cx);
        return;
    }
    #[cfg(test)]
    if cx.update_entity(&owner, |owner, _| {
        owner.take_payload_recovery_pause_after_source_release_for_test()
    }) {
        return;
    }
    let destination_armed = transfer
        .destination()
        .host()
        .update(cx, |host, host_cx| {
            host.arm_payload_recovery_destination_projection(
                transfer.destination_presentation(),
                host_cx,
            )
        })
        .unwrap_or(false);
    if !destination_armed {
        payload_recovery_presentation_failed(
            owner.downgrade(),
            transfer.destination().host().clone(),
            transfer.destination_presentation(),
            prepared,
            cx,
        );
        return;
    }
    refresh_endpoint(transfer.destination(), cx);
}

pub(crate) fn payload_recovery_destination_mounted(
    owner: WeakEntity<DockSurfaceOwner>,
    host: WeakEntity<DockHost>,
    key: DockHostRecoveryPresentationKey,
    prepared: view_presentation_window::PreparedRehost,
    leases: view_presentation_window::LeaseBatch,
    accepted_frame: u64,
    cx: &mut App,
) {
    cx.defer(move |cx| {
        handle_payload_recovery_destination_mounted(
            owner,
            host,
            key,
            prepared,
            leases,
            accepted_frame,
            cx,
        );
    });
}

fn handle_payload_recovery_destination_mounted(
    owner: WeakEntity<DockSurfaceOwner>,
    host: WeakEntity<DockHost>,
    key: DockHostRecoveryPresentationKey,
    prepared: view_presentation_window::PreparedRehost,
    leases: view_presentation_window::LeaseBatch,
    accepted_frame: u64,
    cx: &mut App,
) {
    let Some((owner, transfer)) = transfer_for_presentation(&owner, key, &prepared, cx) else {
        return;
    };
    if key != transfer.destination_presentation()
        || transfer.destination().host() != &host
        || !leases.matches_exactly(prepared.destination())
    {
        return;
    }
    let Some(mount) = prepared.destination_ready_for_exposure() else {
        payload_recovery_presentation_failed(owner.downgrade(), host, key, prepared, cx);
        return;
    };
    if mount.rehost_generation() != prepared.generation()
        || mount.destination_window() != leases.window_id()
        || mount.root_count() != leases.leases().len()
        || mount.frame_generation() != accepted_frame
    {
        payload_recovery_presentation_failed(owner.downgrade(), host, key, prepared, cx);
        return;
    }
    let controller = cx.read_entity(&owner, |owner, _| owner.controller());
    if !destination_endpoint_is_exact(
        transfer.destination(),
        &owner,
        &controller,
        transfer.key().action(),
        cx,
    ) {
        payload_recovery_presentation_failed(owner.downgrade(), host, key, prepared, cx);
        return;
    }
    let exposed = match view_presentation_window::expose_destination(cx, &prepared) {
        Ok(exposed) => exposed,
        Err(_) => {
            payload_recovery_presentation_failed(owner.downgrade(), host, key, prepared, cx);
            return;
        }
    };
    let installed = host
        .update(cx, |host, host_cx| {
            host.expose_payload_recovery_destination_projection(
                key,
                exposed.batch,
                exposed.exposure.mount(),
                host_cx,
            )
        })
        .unwrap_or(false);
    if !installed {
        payload_recovery_presentation_failed(owner.downgrade(), host, key, prepared, cx);
    }
}

pub(crate) fn payload_recovery_destination_presented(
    owner: WeakEntity<DockSurfaceOwner>,
    host: WeakEntity<DockHost>,
    key: DockHostRecoveryPresentationKey,
    prepared: view_presentation_window::PreparedRehost,
    leases: view_presentation_window::LeaseBatch,
    accepted_frame: u64,
    cx: &mut App,
) {
    cx.defer(move |cx| {
        handle_payload_recovery_destination_presented(
            owner,
            host,
            key,
            prepared,
            leases,
            accepted_frame,
            cx,
        );
    });
}

fn handle_payload_recovery_destination_presented(
    owner: WeakEntity<DockSurfaceOwner>,
    host: WeakEntity<DockHost>,
    key: DockHostRecoveryPresentationKey,
    prepared: view_presentation_window::PreparedRehost,
    leases: view_presentation_window::LeaseBatch,
    accepted_frame: u64,
    cx: &mut App,
) {
    let Some((owner, transfer)) = transfer_for_presentation(&owner, key, &prepared, cx) else {
        return;
    };
    if key != transfer.destination_presentation()
        || transfer.destination().host() != &host
        || !leases.matches_exactly(prepared.destination())
    {
        return;
    }
    let Some(presented) = view_presentation_window::presented_batch_receipt(cx, &leases) else {
        payload_recovery_presentation_failed(owner.downgrade(), host, key, prepared, cx);
        return;
    };
    if presented.frame_generation() != accepted_frame
        || prepared.snapshot().destination_mount_receipt() != Some(presented.exposure().mount())
    {
        payload_recovery_presentation_failed(owner.downgrade(), host, key, prepared, cx);
        return;
    }
    let Some(queued) = cx.update_entity(&owner, |owner, _| {
        owner.reserve_payload_recovery_finalization(key)
    }) else {
        return;
    };
    if queued.key() != transfer.key() {
        return;
    }
    let execution = queued.key();
    cx.defer(move |cx| {
        finalize_payload_recovery_restore(owner, execution, presented, cx);
    });
}

pub(crate) fn payload_recovery_presentation_failed(
    owner: WeakEntity<DockSurfaceOwner>,
    host: WeakEntity<DockHost>,
    key: DockHostRecoveryPresentationKey,
    prepared: view_presentation_window::PreparedRehost,
    cx: &mut App,
) {
    let Some((owner, transfer)) = transfer_for_presentation(&owner, key, &prepared, cx) else {
        return;
    };
    if transfer.source().host() != &host && transfer.destination().host() != &host {
        return;
    }
    let execution = transfer.key();
    cx.defer(move |cx| {
        let Some(transfer) = cx.read_entity(&owner, |owner, _| {
            owner.payload_recovery_transfer(execution)
        }) else {
            return;
        };
        rollback_payload_recovery_transfer(&owner, &transfer, cx);
    });
}

pub(crate) fn payload_recovery_host_presentation_released(
    owner: WeakEntity<DockSurfaceOwner>,
    key: DockHostRecoveryPresentationKey,
    cx: &mut App,
) {
    cx.defer(move |cx| {
        let Some(owner) = owner.upgrade() else {
            return;
        };
        let Some(transfer) = cx.read_entity(&owner, |owner, _| {
            owner.payload_recovery_transfer_for_presentation(key)
        }) else {
            return;
        };
        if key == transfer.source_presentation() {
            let source_close_state = cx.read_entity(&owner, |owner, _| {
                owner.payload_recovery_source_close_state(transfer.key())
            });
            if matches!(source_close_state, Some((true, false))) {
                abandon_transfer_host_presentations(&transfer, cx);
                return;
            }
            let _ = abandon_payload_recovery_after_source_loss(&owner, &transfer, cx);
        } else if transfer.prepared().source_settlement_started() {
            // Destination retirement is part of source restoration and must not recursively
            // reinterpret the executor's own compensation as a new presentation failure.
            return;
        } else {
            rollback_payload_recovery_transfer(&owner, &transfer, cx);
        }
    });
}

pub(crate) fn payload_recovery_source_window_closed(
    owner: &Entity<DockSurfaceOwner>,
    window_id: open_gpui::WindowId,
    cx: &mut App,
) {
    let Some(transfer) = cx.update_entity(owner, |owner, _| {
        owner.record_payload_recovery_source_logical_close(window_id)
    }) else {
        return;
    };
    let owner = owner.clone();
    let execution = transfer.key();
    cx.defer(move |cx| {
        let Some(transfer) = cx.read_entity(&owner, |owner, _| {
            owner.payload_recovery_transfer(execution)
        }) else {
            return;
        };
        rollback_payload_recovery_transfer(&owner, &transfer, cx);
    });
}

pub(crate) fn payload_recovery_source_window_native_terminal(
    owner: &Entity<DockSurfaceOwner>,
    window_id: open_gpui::WindowId,
    cx: &mut App,
) {
    let Some(transfer) = cx.update_entity(owner, |owner, _| {
        owner.record_payload_recovery_source_native_terminal(window_id)
    }) else {
        return;
    };
    let _ = abandon_payload_recovery_after_source_loss(owner, &transfer, cx);
}

pub(crate) fn payload_recovery_source_restoration_frame_committed(
    owner: WeakEntity<DockSurfaceOwner>,
    host: WeakEntity<DockHost>,
    key: DockHostRecoveryPresentationKey,
    prepared: view_presentation_window::PreparedRehost,
    cx: &mut App,
) {
    cx.defer(move |cx| {
        let Some((owner, transfer)) = transfer_for_presentation(&owner, key, &prepared, cx) else {
            return;
        };
        if key != transfer.source_presentation() || transfer.source().host() != &host {
            return;
        }
        let controller = cx.read_entity(&owner, |owner, _| owner.controller());
        if !source_endpoint_is_exact(transfer.source(), &owner, &controller, cx) {
            rollback_payload_recovery_transfer(&owner, &transfer, cx);
            return;
        }
        if prepared.accepted_source_restoration().is_some() {
            complete_payload_recovery_source_restoration(&owner, &transfer, cx);
        } else {
            rollback_payload_recovery_transfer(&owner, &transfer, cx);
        }
    });
}

fn finalize_payload_recovery_restore(
    owner: Entity<DockSurfaceOwner>,
    execution: DockPayloadRecoveryExecutionKey,
    presented: view_presentation_window::PresentedBatchReceipt,
    cx: &mut App,
) {
    let Some(transfer) = cx.read_entity(&owner, |owner, _| {
        owner.payload_recovery_transfer(execution)
    }) else {
        return;
    };
    if try_finalize_payload_recovery_restore(&owner, &transfer, presented, cx).is_err() {
        rollback_payload_recovery_transfer(&owner, &transfer, cx);
    }
}

fn try_finalize_payload_recovery_restore(
    owner: &Entity<DockSurfaceOwner>,
    transfer: &DockPayloadRecoveryTransfer,
    presented: view_presentation_window::PresentedBatchReceipt,
    cx: &mut App,
) -> Result<(), ()> {
    let controller = cx.read_entity(owner, |owner, _| owner.controller());
    if !source_endpoint_is_exact(transfer.source(), owner, &controller, cx)
        || !destination_endpoint_is_exact(
            transfer.destination(),
            owner,
            &controller,
            transfer.key().action(),
            cx,
        )
        || presented.window_id() != transfer.prepared().destination().window_id()
        || presented.root_count() != transfer.prepared().destination().leases().len()
    {
        return Err(());
    }
    let source = transfer.source().host().upgrade().ok_or(())?;
    let destination = transfer.destination().host().upgrade().ok_or(())?;
    let source_commit = cx
        .read_entity(&source, |host, _| {
            host.prepare_payload_recovery_source_retirement(transfer.source_presentation())
        })
        .ok_or(())?;
    let destination_commit = cx
        .read_entity(&destination, |host, _| {
            host.prepare_payload_recovery_destination_commit(
                transfer.destination_presentation(),
                presented,
            )
        })
        .ok_or(())?;
    let owner_commit = cx
        .update_entity(owner, |owner, owner_cx| {
            owner.prepare_payload_recovery_restore_final(
                transfer.key(),
                transfer.restore().clone(),
                owner_cx,
            )
        })
        .map_err(|_| ())?;
    let presentation_commit =
        view_presentation_window::prepare_finish_destination(cx, transfer.prepared())
            .map_err(|_| ())?;

    if !cx.read_entity(&source, |host, _| {
        host.can_commit_prepared_payload_recovery_source_retirement(&source_commit)
    }) || !cx.read_entity(&destination, |host, _| {
        host.can_commit_prepared_payload_recovery_destination(&destination_commit)
    }) || !cx.update_entity(owner, |owner, owner_cx| {
        owner.can_commit_payload_recovery_restore_final(&owner_commit, owner_cx)
    }) || !view_presentation_window::can_commit_prepared_finish_destination(
        cx,
        &presentation_commit,
    ) {
        return Err(());
    }

    let projected_graph = transfer.restore().projected_graph().clone();
    let expected_destination = transfer.prepared().destination().clone();
    let receipt = with_detached_root_transaction(owner, cx, |transaction, cx| {
        cx.update_entity(&controller, |controller, controller_cx| {
            controller.workspace_mut().set_graph(projected_graph);
            controller_cx.notify();
        });
        cx.update_entity(&source, |host, host_cx| {
            host.commit_prepared_payload_recovery_source_retirement(source_commit, host_cx);
        });
        cx.update_entity(&destination, |host, host_cx| {
            host.commit_prepared_payload_recovery_destination(destination_commit, host_cx);
        });
        let finish =
            view_presentation_window::commit_prepared_finish_destination(cx, presentation_commit);
        assert!(finish.batch.matches_exactly(&expected_destination));
        cx.update_entity(owner, |owner, owner_cx| {
            owner.commit_payload_recovery_restore_final(transaction, owner_commit, owner_cx)
        })
    });
    install_recovery_focus(
        owner,
        transfer.destination(),
        transfer.key().action(),
        &receipt,
        cx,
    );
    Ok(())
}

fn transfer_for_presentation(
    owner: &WeakEntity<DockSurfaceOwner>,
    key: DockHostRecoveryPresentationKey,
    prepared: &view_presentation_window::PreparedRehost,
    cx: &App,
) -> Option<(Entity<DockSurfaceOwner>, DockPayloadRecoveryTransfer)> {
    let owner = owner.upgrade()?;
    let transfer = cx.read_entity(&owner, |owner, _| {
        owner.payload_recovery_transfer_for_presentation(key)
    })?;
    (transfer.prepared().matches_exactly(prepared)
        && transfer.key().action() == key.action()
        && key.rehost_generation() == prepared.generation())
    .then_some((owner, transfer))
}

fn source_endpoint_is_exact(
    endpoint: &DockPayloadRecoveryEndpoint,
    owner: &Entity<DockSurfaceOwner>,
    controller: &Entity<DockController>,
    cx: &App,
) -> bool {
    let Some(host) = endpoint.host().upgrade() else {
        return false;
    };
    let Ok(window_host) = endpoint.window().entity(cx) else {
        return false;
    };
    let Some(registration) = endpoint.registration() else {
        return false;
    };
    host == window_host
        && cx.read_entity(&host, |host, _| {
            host.controller_entity() == *controller
                && host.accepts_payload_recovery_source_endpoint(
                    owner.entity_id(),
                    endpoint.space(),
                    endpoint.binding(),
                    registration,
                )
        })
}

fn destination_endpoint_is_exact(
    endpoint: &DockPayloadRecoveryEndpoint,
    owner: &Entity<DockSurfaceOwner>,
    controller: &Entity<DockController>,
    action: DockPayloadRecoveryRestoreAction,
    cx: &App,
) -> bool {
    let Some(host) = endpoint.host().upgrade() else {
        return false;
    };
    let Ok(window_host) = endpoint.window().entity(cx) else {
        return false;
    };
    host == window_host
        && cx.read_entity(&host, |host, _| {
            host.controller_entity() == *controller
                && host.accepts_payload_recovery_destination_endpoint(
                    owner.entity_id(),
                    action,
                    endpoint.binding(),
                )
                && host.current_viewport_registration().as_ref() == endpoint.registration()
        })
}

fn refresh_endpoint(endpoint: &DockPayloadRecoveryEndpoint, cx: &mut App) {
    let _ = endpoint
        .window()
        .update(cx, |_, window, _| window.refresh());
}

fn abandon_host_presentation(
    host: &WeakEntity<DockHost>,
    key: DockHostRecoveryPresentationKey,
    cx: &mut App,
) -> bool {
    host.update(cx, |host, host_cx| {
        let Some(prepared) = host.prepare_payload_recovery_presentation_abandonment(key) else {
            return !host
                .payload_recovery_presentation_state()
                .is_some_and(|state| state.key == key);
        };
        if !host.can_commit_prepared_payload_recovery_presentation_abandonment(&prepared) {
            return false;
        }
        host.commit_prepared_payload_recovery_presentation_abandonment(prepared, host_cx);
        true
    })
    .unwrap_or(true)
}

fn install_recovery_focus(
    owner: &Entity<DockSurfaceOwner>,
    destination: &DockPayloadRecoveryEndpoint,
    action: DockPayloadRecoveryRestoreAction,
    receipt: &DockPayloadRecoveryRestoreReceipt,
    cx: &mut App,
) {
    let _ = destination.window().update(cx, |host, window, host_cx| {
        if host.accepts_payload_recovery_destination_endpoint(
            owner.entity_id(),
            action,
            destination.binding(),
        ) {
            host.install_payload_recovery_restore_focus(receipt, host_cx);
            window.refresh();
        }
    });
}

fn finish_execution(
    owner: &Entity<DockSurfaceOwner>,
    execution: DockPayloadRecoveryExecutionKey,
    cx: &mut App,
) {
    let _ = cx.update_entity(owner, |owner, _| {
        owner.cancel_payload_recovery_execution(execution)
    });
}

fn abandon_prepared_rehost_after_source_loss(
    prepared: &view_presentation_window::PreparedRehost,
    cx: &mut App,
) -> bool {
    matches!(
        view_presentation_window::abandon_rehost_after_source_loss(cx, prepared),
        Ok(view_presentation_window::AbandonRehostOutcome::Abandoned(_)
            | view_presentation_window::AbandonRehostOutcome::AlreadyAbsent)
    )
}

fn retire_prepared_rehost_to_source(
    prepared: &view_presentation_window::PreparedRehost,
    cx: &mut App,
) -> bool {
    match view_presentation_window::settle_rehost_source(cx, prepared) {
        Ok(
            view_presentation_window::SourceSettlement::RetiredToSource(_)
            | view_presentation_window::SourceSettlement::AlreadyRetired,
        ) => true,
        Ok(
            view_presentation_window::SourceSettlement::RenderSource(_)
            | view_presentation_window::SourceSettlement::AwaitingSourceNativeTerminal
            | view_presentation_window::SourceSettlement::PresentationAuthorityReleased(_),
        )
        | Err(_) => abandon_prepared_rehost_after_source_loss(prepared, cx),
    }
}

fn abandon_transfer_host_presentations(transfer: &DockPayloadRecoveryTransfer, cx: &mut App) {
    let _ = abandon_host_presentation(transfer.source().host(), transfer.source_presentation(), cx);
    let _ = abandon_host_presentation(
        transfer.destination().host(),
        transfer.destination_presentation(),
        cx,
    );
}

fn rollback_payload_recovery_transfer(
    owner: &Entity<DockSurfaceOwner>,
    transfer: &DockPayloadRecoveryTransfer,
    cx: &mut App,
) {
    if let Some((source_logically_closed, source_native_terminal_seen)) = cx
        .read_entity(owner, |owner, _| {
            owner.payload_recovery_source_close_state(transfer.key())
        })
        && source_logically_closed
    {
        abandon_transfer_host_presentations(transfer, cx);
        if source_native_terminal_seen {
            let _ = abandon_payload_recovery_after_source_loss(owner, transfer, cx);
        }
        return;
    }

    match view_presentation_window::settle_rehost_source(cx, transfer.prepared()) {
        Ok(
            view_presentation_window::SourceSettlement::RetiredToSource(_)
            | view_presentation_window::SourceSettlement::AlreadyRetired,
        ) => {
            abandon_host_presentation(transfer.source().host(), transfer.source_presentation(), cx);
            abandon_host_presentation(
                transfer.destination().host(),
                transfer.destination_presentation(),
                cx,
            );
            finish_execution(owner, transfer.key(), cx);
        }
        Ok(view_presentation_window::SourceSettlement::RenderSource(restored)) => {
            begin_payload_recovery_source_restoration(owner, transfer, restored, cx);
        }
        Ok(view_presentation_window::SourceSettlement::AwaitingSourceNativeTerminal) => {
            let terminal_seen = cx.read_entity(owner, |owner, _| {
                owner.payload_recovery_source_native_terminal_seen(transfer.key())
            });
            if terminal_seen {
                let _ = abandon_payload_recovery_after_source_loss(owner, transfer, cx);
            }
        }
        Ok(view_presentation_window::SourceSettlement::PresentationAuthorityReleased(_))
        | Err(_) => {
            let _ = abandon_payload_recovery_after_source_loss(owner, transfer, cx);
        }
    }
}

fn begin_payload_recovery_source_restoration(
    owner: &Entity<DockSurfaceOwner>,
    transfer: &DockPayloadRecoveryTransfer,
    restored: view_presentation_window::LeaseBatch,
    cx: &mut App,
) {
    abandon_host_presentation(
        transfer.destination().host(),
        transfer.destination_presentation(),
        cx,
    );
    let controller = cx.read_entity(owner, |owner, _| owner.controller());
    let restored_is_exact = transfer
        .prepared()
        .restored_source()
        .is_some_and(|expected| expected.matches_exactly(&restored));
    if !source_endpoint_is_exact(transfer.source(), owner, &controller, cx) || !restored_is_exact {
        abandon_payload_recovery_after_source_loss(owner, transfer, cx);
        return;
    }
    let session = cx.read_entity(&controller, |controller, _| {
        DockHostPresentationSession::payload_recovery_projection(
            transfer.restore().source_space().clone(),
            controller.graph(),
            controller.workspace(),
        )
    });
    #[cfg(test)]
    if cx.update_entity(owner, |owner, _| {
        owner.take_payload_recovery_pause_after_source_restoration_for_test()
    }) {
        return;
    }
    let outcome = transfer
        .source()
        .host()
        .update(cx, |host, host_cx| {
            let _ =
                host.mark_payload_recovery_source_frozen(transfer.source_presentation(), host_cx);
            host.begin_payload_recovery_source_restoration(
                transfer.source_presentation(),
                session,
                restored.clone(),
                transfer.roots().to_vec(),
                host_cx,
            )
        })
        .unwrap_or(DockHostRecoverySourceRestorationInstallOutcome::PresentationAuthorityLost);
    match outcome {
        DockHostRecoverySourceRestorationInstallOutcome::Installed
        | DockHostRecoverySourceRestorationInstallOutcome::AlreadyInstalled => {
            refresh_endpoint(transfer.source(), cx);
        }
        DockHostRecoverySourceRestorationInstallOutcome::PresentationAuthorityLost => {
            abandon_payload_recovery_after_source_loss(owner, transfer, cx);
        }
    }
}

fn complete_payload_recovery_source_restoration(
    owner: &Entity<DockSurfaceOwner>,
    transfer: &DockPayloadRecoveryTransfer,
    cx: &mut App,
) {
    let Some(restored) = transfer.prepared().restored_source() else {
        abandon_payload_recovery_after_source_loss(owner, transfer, cx);
        return;
    };
    let Some(source_host) = transfer.source().host().upgrade() else {
        abandon_payload_recovery_after_source_loss(owner, transfer, cx);
        return;
    };
    let Some(phase) = cx.read_entity(&source_host, |host, _| {
        let state = host.payload_recovery_presentation_state()?;
        if state.key != transfer.source_presentation() {
            return None;
        }
        match state.mode {
            DockHostRecoveryPresentationMode::SourceRestoration { phase, .. } => Some(phase),
            DockHostRecoveryPresentationMode::SourceProjection { .. }
            | DockHostRecoveryPresentationMode::DestinationProjection { .. } => None,
        }
    }) else {
        abandon_payload_recovery_after_source_loss(owner, transfer, cx);
        return;
    };

    if phase == DockHostRecoverySourceRestorationPhase::Staging {
        let finished = view_presentation_window::finish_rendered_rehost_source(
            cx,
            transfer.prepared(),
            &restored,
        );
        if !matches!(
            finished,
            Ok(view_presentation_window::SourcePresentationFinish::Finished(_))
        ) {
            abandon_payload_recovery_after_source_loss(owner, transfer, cx);
            return;
        }
        #[cfg(test)]
        if cx.update_entity(owner, |owner, _| {
            owner.take_payload_recovery_replace_source_host_after_finish_for_test()
        }) {
            let _ = transfer.source().window().update(cx, |_, window, cx| {
                window.replace_root(cx, |_, _| open_gpui::Empty);
            });
            return;
        }
        let marked = cx.update_entity(&source_host, |host, host_cx| {
            host.mark_payload_recovery_source_restoration_visible_pending(
                transfer.source_presentation(),
                &restored,
                host_cx,
            )
        });
        if !marked {
            abandon_payload_recovery_after_source_loss(owner, transfer, cx);
            return;
        }
        refresh_endpoint(transfer.source(), cx);
        return;
    }

    let Some(presented) =
        view_presentation_window::stable_batch_presentation_receipt(cx, &restored)
    else {
        abandon_payload_recovery_after_source_loss(owner, transfer, cx);
        return;
    };
    let Some(source_commit) = cx.read_entity(&source_host, |host, _| {
        host.prepare_payload_recovery_source_restoration_commit(
            transfer.source_presentation(),
            presented,
        )
    }) else {
        abandon_payload_recovery_after_source_loss(owner, transfer, cx);
        return;
    };
    if !cx.read_entity(&source_host, |host, _| {
        host.can_commit_prepared_payload_recovery_source_restoration(&source_commit)
    }) {
        abandon_payload_recovery_after_source_loss(owner, transfer, cx);
        return;
    }
    cx.update_entity(&source_host, |host, host_cx| {
        host.commit_prepared_payload_recovery_source_restoration(source_commit, host_cx);
    });
    abandon_host_presentation(
        transfer.destination().host(),
        transfer.destination_presentation(),
        cx,
    );
    finish_execution(owner, transfer.key(), cx);
    refresh_endpoint(transfer.source(), cx);
}

fn abandon_payload_recovery_after_source_loss(
    owner: &Entity<DockSurfaceOwner>,
    transfer: &DockPayloadRecoveryTransfer,
    cx: &mut App,
) -> bool {
    let authority_released = abandon_prepared_rehost_after_source_loss(transfer.prepared(), cx);
    if !authority_released {
        return false;
    }

    let source_abandoned =
        abandon_host_presentation(transfer.source().host(), transfer.source_presentation(), cx);
    let destination_abandoned = abandon_host_presentation(
        transfer.destination().host(),
        transfer.destination_presentation(),
        cx,
    );
    if !source_abandoned || !destination_abandoned {
        return false;
    }
    finish_execution(owner, transfer.key(), cx);
    true
}
