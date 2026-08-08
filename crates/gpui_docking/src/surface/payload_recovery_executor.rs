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
use std::{
    num::NonZeroU64,
    panic::{AssertUnwindSafe, catch_unwind, resume_unwind},
    sync::{Arc, Mutex, MutexGuard},
};

const MAX_AUTOMATIC_FINALIZATION_RETRIES: u8 = 1;

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

#[cfg(test)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum DockPayloadRecoveryFinalizationPanicStage {
    Provider,
    SourceHost,
    DestinationHost,
    Owner,
}

#[cfg(test)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum DockPayloadRecoveryInstallationPanicStage {
    PreparedSession,
    DestinationHost,
    SourceHost,
    Executor,
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
    projection: view_presentation_window::RehostProjection,
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
        projection: view_presentation_window::RehostProjection,
        source_presentation: DockHostRecoveryPresentationKey,
        destination_presentation: DockHostRecoveryPresentationKey,
    ) -> Option<Self> {
        if roots.is_empty()
            || restore.action() != key.action()
            || projection.source().window_id() != source.window().window_id()
            || projection.destination().window_id() != destination.window().window_id()
            || source_presentation.rehost_generation() != projection.generation()
            || destination_presentation.rehost_generation() != projection.generation()
        {
            return None;
        }
        Some(Self {
            key,
            restore,
            source,
            destination,
            roots,
            projection,
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

    pub(crate) fn projection(&self) -> &view_presentation_window::RehostProjection {
        &self.projection
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

#[derive(Clone, Debug)]
pub(crate) struct DockPayloadRecoveryFinalization {
    presented: view_presentation_window::RehostDestinationPresentation,
    progress: Arc<Mutex<DockPayloadRecoveryFinalizationProgress>>,
}

#[derive(Clone, Debug, Default)]
struct DockPayloadRecoveryFinalizationProgress {
    provider_committed: bool,
    source_host_settled: bool,
    destination_host_settled: bool,
    owner_receipt: Option<DockPayloadRecoveryRestoreReceipt>,
    focus_installed: bool,
    automatic_retries: u8,
}

impl DockPayloadRecoveryFinalization {
    fn new(presented: view_presentation_window::RehostDestinationPresentation) -> Self {
        Self {
            presented,
            progress: Arc::new(Mutex::new(
                DockPayloadRecoveryFinalizationProgress::default(),
            )),
        }
    }

    fn progress(&self) -> MutexGuard<'_, DockPayloadRecoveryFinalizationProgress> {
        self.progress
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    const fn presented(&self) -> view_presentation_window::RehostDestinationPresentation {
        self.presented
    }

    fn snapshot(&self) -> DockPayloadRecoveryFinalizationProgress {
        self.progress().clone()
    }

    fn mark_provider_committed(&self) {
        self.progress().provider_committed = true;
    }

    fn mark_source_host_settled(&self) {
        self.progress().source_host_settled = true;
    }

    fn mark_destination_host_settled(&self) {
        self.progress().destination_host_settled = true;
    }

    fn mark_owner_committed(&self, receipt: DockPayloadRecoveryRestoreReceipt) {
        self.progress().owner_receipt = Some(receipt);
    }

    fn mark_focus_installed(&self) {
        self.progress().focus_installed = true;
    }

    fn record_failure_and_should_retry(&self) -> bool {
        let mut progress = self.progress();
        if progress.automatic_retries >= MAX_AUTOMATIC_FINALIZATION_RETRIES {
            return false;
        }
        progress.automatic_retries += 1;
        true
    }
}

#[derive(Debug)]
struct PreparedPayloadRecoveryInstallation {
    execution: DockPayloadRecoveryExecutionKey,
    session: Option<view_presentation_window::RehostSession>,
    source: Option<DockPayloadRecoveryEndpoint>,
    destination: DockPayloadRecoveryEndpoint,
    source_presentation: Option<DockHostRecoveryPresentationKey>,
    destination_presentation: Option<DockHostRecoveryPresentationKey>,
}

impl PreparedPayloadRecoveryInstallation {
    fn new(
        execution: DockPayloadRecoveryExecutionKey,
        session: view_presentation_window::RehostSession,
        destination: DockPayloadRecoveryEndpoint,
    ) -> Self {
        Self {
            execution,
            session: Some(session),
            source: None,
            destination,
            source_presentation: None,
            destination_presentation: None,
        }
    }

    fn projection(&self) -> view_presentation_window::RehostProjection {
        self.session
            .as_ref()
            .expect("prepared payload recovery installation must own its rehost session")
            .projection()
    }

    fn session_slot_mut(&mut self) -> &mut Option<view_presentation_window::RehostSession> {
        &mut self.session
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

    fn discover_installed_presentations(&mut self, cx: &App) {
        let generation = self.projection().generation();
        let matches = |state: &crate::host::DockHostRecoveryPresentationState| {
            state.key.action() == self.execution.action()
                && state.key.rehost_generation() == generation
        };
        if self.destination_presentation.is_none() {
            self.destination_presentation = self
                .destination
                .host()
                .upgrade()
                .and_then(|host| {
                    cx.read_entity(&host, |host, _| host.payload_recovery_presentation_state())
                })
                .filter(|state| {
                    matches(state)
                        && matches!(
                            state.mode,
                            DockHostRecoveryPresentationMode::DestinationProjection { .. }
                        )
                })
                .map(|state| state.key);
        }
        if self.source_presentation.is_none() {
            self.source_presentation = self
                .source
                .as_ref()
                .and_then(|source| source.host().upgrade())
                .and_then(|host| {
                    cx.read_entity(&host, |host, _| host.payload_recovery_presentation_state())
                })
                .filter(|state| {
                    matches(state)
                        && matches!(
                            state.mode,
                            DockHostRecoveryPresentationMode::SourceProjection { .. }
                        )
                })
                .map(|state| state.key);
        }
    }

    fn compensate(
        mut self,
        owner: &Entity<DockSurfaceOwner>,
        controller: &Entity<DockController>,
        cx: &mut App,
    ) {
        let source_is_exact = self
            .source
            .as_ref()
            .is_some_and(|source| source_endpoint_is_exact(source, owner, controller, cx));
        let session = self
            .session
            .as_mut()
            .expect("failed payload recovery installation must retain its rehost session");
        let authority_retired = if source_is_exact {
            retire_rehost_session_to_source(session, cx)
        } else {
            abandon_rehost_session_after_source_loss(session, cx)
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
        session: Option<view_presentation_window::RehostSession>,
        finalization: Option<DockPayloadRecoveryFinalization>,
        source_settlement_started: bool,
        #[cfg(test)]
        phase: view_presentation_window::RehostPhase,
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
    #[cfg(test)]
    panic_after_finalization_stage: Option<(DockPayloadRecoveryFinalizationPanicStage, u8)>,
    #[cfg(test)]
    pause_before_finalization_once: bool,
    #[cfg(test)]
    pause_finalization_retry_once: bool,
    #[cfg(test)]
    panic_after_installation_stage_once: Option<DockPayloadRecoveryInstallationPanicStage>,
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
        session: &mut Option<view_presentation_window::RehostSession>,
    ) -> bool {
        let Some(candidate) = session.as_ref() else {
            return false;
        };
        if !matches!(self.state, DockPayloadRecoveryExecutionState::Reserved(current) if current == key)
            || transfer.key() != key
            || !transfer
                .projection()
                .matches_exactly(&candidate.projection())
        {
            return false;
        }
        #[cfg(test)]
        if std::mem::take(&mut self.reject_next_transfer_install_once) {
            return false;
        }
        let session = session
            .take()
            .expect("validated payload recovery installation must retain its rehost session");
        self.state = DockPayloadRecoveryExecutionState::Rehosting {
            transfer,
            session: Some(session),
            finalization: None,
            source_settlement_started: false,
            #[cfg(test)]
            phase: view_presentation_window::RehostPhase::AwaitingSourceRelease,
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

    pub(crate) fn accept_source_proxy_frame(
        &mut self,
        key: DockHostRecoveryPresentationKey,
        accepted_frame: u64,
    ) -> Option<
        Result<
            view_presentation_window::RehostSourceProxyCommit,
            view_presentation_window::TransitionError,
        >,
    > {
        let DockPayloadRecoveryExecutionState::Rehosting {
            transfer,
            session: Some(session),
            #[cfg(test)]
            phase,
            ..
        } = &mut self.state
        else {
            return None;
        };
        if transfer.source_presentation() != key {
            return None;
        }
        let result = session.accept_source_proxy_frame(accepted_frame);
        #[cfg(test)]
        if result.is_ok() {
            *phase = view_presentation_window::RehostPhase::DestinationAdmitted;
        }
        Some(result)
    }

    pub(crate) fn accept_destination_frame(
        &mut self,
        key: DockHostRecoveryPresentationKey,
        accepted_frame: u64,
        cx: &mut App,
    ) -> Option<
        Result<
            view_presentation_window::RehostDestinationExposure,
            view_presentation_window::TransitionError,
        >,
    > {
        let DockPayloadRecoveryExecutionState::Rehosting {
            transfer,
            session: Some(session),
            #[cfg(test)]
            phase,
            ..
        } = &mut self.state
        else {
            return None;
        };
        if transfer.destination_presentation() != key {
            return None;
        }
        let result = session.accept_destination_frame(cx, accepted_frame);
        #[cfg(test)]
        if result.is_ok() {
            *phase = view_presentation_window::RehostPhase::DestinationExposed;
        }
        Some(result)
    }

    pub(crate) fn accept_destination_presentation_frame(
        &self,
        key: DockHostRecoveryPresentationKey,
        accepted_frame: u64,
        cx: &App,
    ) -> Option<
        Result<
            view_presentation_window::RehostDestinationPresentation,
            view_presentation_window::TransitionError,
        >,
    > {
        let DockPayloadRecoveryExecutionState::Rehosting {
            transfer,
            session: Some(session),
            ..
        } = &self.state
        else {
            return None;
        };
        (transfer.destination_presentation() == key)
            .then(|| session.accept_destination_presentation_frame(cx, accepted_frame))
    }

    pub(crate) fn settle_source(
        &mut self,
        key: DockPayloadRecoveryExecutionKey,
        cx: &mut App,
    ) -> Option<
        Result<
            view_presentation_window::SourceSettlement,
            view_presentation_window::TransitionError,
        >,
    > {
        let DockPayloadRecoveryExecutionState::Rehosting {
            transfer,
            session,
            source_settlement_started,
            #[cfg(test)]
            phase,
            ..
        } = &mut self.state
        else {
            return None;
        };
        if transfer.key() != key {
            return None;
        }
        *source_settlement_started = true;
        let result = session.as_mut()?.settle_source(cx);
        #[cfg(test)]
        match &result {
            Ok(
                view_presentation_window::SourceSettlement::RetiredToSource(_)
                | view_presentation_window::SourceSettlement::AlreadyRetired,
            ) => {
                *phase = view_presentation_window::RehostPhase::Cancelled;
            }
            Ok(view_presentation_window::SourceSettlement::RenderSource(_)) => {
                *phase = view_presentation_window::RehostPhase::RestoringSource;
            }
            Ok(view_presentation_window::SourceSettlement::PresentationAuthorityReleased(_))
            | Err(_) => {
                *phase = view_presentation_window::RehostPhase::Invalidated;
            }
            Ok(view_presentation_window::SourceSettlement::AwaitingSourceNativeTerminal) => {}
        }
        Some(result)
    }

    pub(crate) fn accept_source_restoration_frame(
        &mut self,
        key: DockHostRecoveryPresentationKey,
        accepted_frame: u64,
        cx: &mut App,
    ) -> Option<
        Result<
            view_presentation_window::SourcePresentationFinish,
            view_presentation_window::TransitionError,
        >,
    > {
        let DockPayloadRecoveryExecutionState::Rehosting {
            transfer,
            session,
            #[cfg(test)]
            phase,
            ..
        } = &mut self.state
        else {
            return None;
        };
        if transfer.source_presentation() != key {
            return None;
        }
        let session = session.as_mut()?;
        let result = match session.committed_source_finish() {
            Some(outcome) => Ok(outcome),
            None => session.accept_source_restoration_frame(cx, accepted_frame),
        };
        if matches!(
            &result,
            Ok(view_presentation_window::SourcePresentationFinish::Finished(_))
        ) {
            #[cfg(test)]
            {
                *phase = view_presentation_window::RehostPhase::SourceRestored;
            }
        }
        Some(result)
    }

    pub(crate) fn abandon_after_source_loss(
        &mut self,
        key: DockPayloadRecoveryExecutionKey,
        cx: &mut App,
    ) -> Option<
        Result<
            view_presentation_window::RehostAbandonmentOutcome,
            view_presentation_window::TransitionError,
        >,
    > {
        let DockPayloadRecoveryExecutionState::Rehosting {
            transfer, session, ..
        } = &mut self.state
        else {
            return None;
        };
        if transfer.key() != key {
            return None;
        }
        Some(session.as_mut()?.abandon_after_source_loss(cx))
    }

    pub(crate) fn prepare_terminal(
        &mut self,
        key: DockPayloadRecoveryExecutionKey,
        cx: &App,
        intent: view_presentation_window::RehostTerminalIntent<'_>,
    ) -> Option<
        Result<
            view_presentation_window::RehostTerminalPreparation,
            view_presentation_window::TransitionError,
        >,
    > {
        let DockPayloadRecoveryExecutionState::Rehosting {
            transfer, session, ..
        } = &mut self.state
        else {
            return None;
        };
        if transfer.key() != key {
            return None;
        }
        Some(session.as_ref()?.prepare_terminal(cx, intent))
    }

    pub(crate) fn finish_terminal(&mut self, key: DockPayloadRecoveryExecutionKey) -> bool {
        let terminal = matches!(
            &self.state,
            DockPayloadRecoveryExecutionState::Rehosting {
                transfer,
                session: Some(session),
                ..
            } if transfer.key() == key && session.is_terminal()
        );
        if !terminal {
            return false;
        }
        self.state = DockPayloadRecoveryExecutionState::Idle;
        true
    }

    pub(crate) fn queue_finalization(
        &mut self,
        key: DockHostRecoveryPresentationKey,
        presented: view_presentation_window::RehostDestinationPresentation,
    ) -> Option<DockPayloadRecoveryTransfer> {
        let DockPayloadRecoveryExecutionState::Rehosting {
            transfer,
            finalization,
            ..
        } = &mut self.state
        else {
            return None;
        };
        if transfer.destination_presentation() != key
            || finalization
                .as_ref()
                .is_some_and(|current| current.presented() != presented)
        {
            return None;
        }
        if finalization.is_none() {
            *finalization = Some(DockPayloadRecoveryFinalization::new(presented));
        }
        Some(transfer.clone())
    }

    pub(crate) fn finalization(
        &self,
        key: DockPayloadRecoveryExecutionKey,
    ) -> Option<(DockPayloadRecoveryTransfer, DockPayloadRecoveryFinalization)> {
        match &self.state {
            DockPayloadRecoveryExecutionState::Rehosting {
                transfer,
                finalization: Some(finalization),
                ..
            } if transfer.key() == key => Some((transfer.clone(), finalization.clone())),
            DockPayloadRecoveryExecutionState::Idle
            | DockPayloadRecoveryExecutionState::Reserved(_)
            | DockPayloadRecoveryExecutionState::Rehosting { .. } => None,
        }
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
            #[cfg(test)]
            phase,
            ..
        } = &mut self.state
        else {
            return None;
        };
        if transfer.source().window().window_id() != window_id {
            return None;
        }
        *source_logically_closed = true;
        #[cfg(test)]
        if matches!(
            *phase,
            view_presentation_window::RehostPhase::AwaitingSourceRelease
                | view_presentation_window::RehostPhase::RestoringSource
        ) {
            *phase = view_presentation_window::RehostPhase::Invalidated;
        }
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

    pub(crate) fn source_settlement_started(&self, key: DockPayloadRecoveryExecutionKey) -> bool {
        matches!(
            &self.state,
            DockPayloadRecoveryExecutionState::Rehosting {
                transfer,
                source_settlement_started: true,
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

    #[cfg(test)]
    pub(crate) fn panic_after_finalization_stage_once_for_test(
        &mut self,
        stage: DockPayloadRecoveryFinalizationPanicStage,
    ) {
        self.panic_after_finalization_stage_for_test(stage, 1);
    }

    #[cfg(test)]
    pub(crate) fn panic_after_finalization_stage_for_test(
        &mut self,
        stage: DockPayloadRecoveryFinalizationPanicStage,
        attempts: u8,
    ) {
        assert!(
            attempts > 0,
            "a finalization panic injection needs one attempt"
        );
        self.panic_after_finalization_stage = Some((stage, attempts));
    }

    #[cfg(test)]
    pub(crate) fn take_finalization_panic_for_test(
        &mut self,
        stage: DockPayloadRecoveryFinalizationPanicStage,
    ) -> bool {
        let Some((configured, remaining)) = self.panic_after_finalization_stage.take() else {
            return false;
        };
        if configured != stage {
            self.panic_after_finalization_stage = Some((configured, remaining));
            return false;
        }
        if remaining > 1 {
            self.panic_after_finalization_stage = Some((configured, remaining - 1));
        }
        true
    }

    #[cfg(test)]
    pub(crate) fn pause_before_finalization_once_for_test(&mut self) {
        self.pause_before_finalization_once = true;
    }

    #[cfg(test)]
    pub(crate) fn take_pause_before_finalization_for_test(&mut self) -> bool {
        std::mem::take(&mut self.pause_before_finalization_once)
    }

    #[cfg(test)]
    pub(crate) fn pause_finalization_retry_once_for_test(&mut self) {
        self.pause_finalization_retry_once = true;
    }

    #[cfg(test)]
    pub(crate) fn take_pause_finalization_retry_for_test(&mut self) -> bool {
        std::mem::take(&mut self.pause_finalization_retry_once)
    }

    #[cfg(test)]
    pub(crate) fn panic_after_installation_stage_once_for_test(
        &mut self,
        stage: DockPayloadRecoveryInstallationPanicStage,
    ) {
        self.panic_after_installation_stage_once = Some(stage);
    }

    #[cfg(test)]
    pub(crate) fn take_installation_panic_for_test(
        &mut self,
        stage: DockPayloadRecoveryInstallationPanicStage,
    ) -> bool {
        if self.panic_after_installation_stage_once == Some(stage) {
            self.panic_after_installation_stage_once = None;
            true
        } else {
            false
        }
    }

    pub(crate) fn is_reserved(&self, key: DockPayloadRecoveryExecutionKey) -> bool {
        matches!(self.state, DockPayloadRecoveryExecutionState::Reserved(current) if current == key)
    }

    pub(crate) fn session_terminal_disposition(
        &self,
        key: DockPayloadRecoveryExecutionKey,
    ) -> Option<view_presentation_window::RehostTerminalDisposition> {
        match &self.state {
            DockPayloadRecoveryExecutionState::Rehosting {
                transfer,
                session: Some(session),
                ..
            } if transfer.key() == key => session.terminal_disposition(),
            DockPayloadRecoveryExecutionState::Idle
            | DockPayloadRecoveryExecutionState::Reserved(_)
            | DockPayloadRecoveryExecutionState::Rehosting { .. } => None,
        }
    }

    pub(crate) fn finish(&mut self, key: DockPayloadRecoveryExecutionKey) -> bool {
        if !matches!(self.state, DockPayloadRecoveryExecutionState::Reserved(current) if current == key)
        {
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
            phase,
            source_logically_closed,
            source_native_terminal_seen,
            ..
        } = &self.state
        else {
            return None;
        };
        Some((
            transfer.key(),
            *phase,
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
    let result = catch_unwind(AssertUnwindSafe(|| {
        start_prepared_payload_recovery_restore(
            &owner,
            primary_host,
            primary_window,
            primary_binding,
            execution,
            restore,
            cx,
        )
    }));
    match result {
        Ok(result) => {
            if result.is_err() {
                let _ = cx.update_entity(&owner, |owner, _| {
                    owner.cancel_payload_recovery_execution(execution)
                });
            }
            result
        }
        Err(payload) => {
            let _ = cx.update_entity(&owner, |owner, _| {
                owner.cancel_payload_recovery_execution(execution)
            });
            resume_unwind(payload)
        }
    }
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

    let source_registration = origin.registration().clone();
    let source_entity = origin.window().entity(cx).ok();
    let source_is_exact = source_entity.as_ref().is_some_and(|source_entity| {
        cx.read_entity(source_entity, |host, _| {
            host.controller_entity() == controller
                && host.accepts_payload_recovery_source_endpoint(
                    owner.entity_id(),
                    restore.source_space(),
                    origin.binding(),
                    &source_registration,
                )
                && host.live_presentation_state().is_none()
                && host.payload_recovery_presentation_state().is_none()
        })
    });
    if !source_is_exact {
        let stale_source_leases = resolved_roots
            .iter()
            .filter_map(|view| {
                view_presentation_window::stable_lease_for_window(
                    cx,
                    view.entity_id(),
                    origin.window().window_id(),
                )
            })
            .collect::<Vec<_>>();
        view_presentation_window::release_stable_leases_after_endpoint_loss(
            cx,
            &stale_source_leases,
        );
    }

    let session = match view_presentation_window::prepare_resolved_view_rehost(
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
        view_presentation_window::ResolvedViewRehostOutcome::Prepared(session) => session,
    };

    let mut installation =
        PreparedPayloadRecoveryInstallation::new(execution, session, destination.clone());
    let transfer = catch_unwind(AssertUnwindSafe(|| {
        #[cfg(test)]
        panic_after_payload_recovery_installation_stage_for_test(
            owner,
            DockPayloadRecoveryInstallationPanicStage::PreparedSession,
            cx,
        );
        let source_entity = source_entity
            .filter(|_| source_is_exact)
            .ok_or(DockPayloadRecoveryRestoreError::PresentationEndpointUnavailable)?;
        let source_remains_exact = cx.read_entity(&source_entity, |host, _| {
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
        if !source_remains_exact {
            return Err(DockPayloadRecoveryRestoreError::PresentationEndpointUnavailable);
        }
        let source = DockPayloadRecoveryEndpoint::new(
            source_entity.downgrade(),
            origin.window(),
            origin.binding(),
            restore.source_space().clone(),
            Some(source_registration),
        )
        .ok_or(DockPayloadRecoveryRestoreError::PresentationEndpointUnavailable)?;
        installation.set_source(source.clone());
        let projection = installation.projection();

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
                        projection.clone(),
                        projection.destination().clone(),
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
        #[cfg(test)]
        panic_after_payload_recovery_installation_stage_for_test(
            owner,
            DockPayloadRecoveryInstallationPanicStage::DestinationHost,
            cx,
        );

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
                        projection.clone(),
                        host_cx,
                    )
                })
                .flatten()
            })
            .ok()
            .flatten()
            .ok_or(DockPayloadRecoveryRestoreError::PresentationInstallRejected)?;
        installation.record_source_presentation(source_key);
        #[cfg(test)]
        panic_after_payload_recovery_installation_stage_for_test(
            owner,
            DockPayloadRecoveryInstallationPanicStage::SourceHost,
            cx,
        );

        let transfer = DockPayloadRecoveryTransfer::new(
            execution,
            restore,
            source,
            destination,
            resolved_roots,
            projection,
            source_key,
            destination_key,
        )
        .ok_or(DockPayloadRecoveryRestoreError::PresentationInstallRejected)?;
        let installed = cx.update_entity(owner, |owner, _| {
            owner.install_payload_recovery_transfer(
                execution,
                transfer.clone(),
                installation.session_slot_mut(),
            )
        });
        if installed {
            #[cfg(test)]
            panic_after_payload_recovery_installation_stage_for_test(
                owner,
                DockPayloadRecoveryInstallationPanicStage::Executor,
                cx,
            );
            Ok(transfer)
        } else {
            Err(DockPayloadRecoveryRestoreError::PresentationInstallRejected)
        }
    }));
    let transfer = match transfer {
        Ok(Ok(transfer)) => transfer,
        Ok(Err(error)) => {
            installation.compensate(owner, &controller, cx);
            return Err(error);
        }
        Err(payload) => {
            if let Some(transfer) =
                cx.read_entity(owner, |owner, _| owner.payload_recovery_transfer(execution))
            {
                log::error!(
                    "payload recovery installation panicked after executor admission; continuing exact execution {execution:?}"
                );
                transfer
            } else {
                installation.discover_installed_presentations(cx);
                installation.compensate(owner, &controller, cx);
                let _ = cx.update_entity(owner, |owner, _| {
                    owner.cancel_payload_recovery_execution(execution)
                });
                resume_unwind(payload);
            }
        }
    };

    refresh_endpoint(transfer.destination(), cx);
    refresh_endpoint(transfer.source(), cx);
    Ok(())
}

#[cfg(test)]
fn panic_after_payload_recovery_installation_stage_for_test(
    owner: &Entity<DockSurfaceOwner>,
    stage: DockPayloadRecoveryInstallationPanicStage,
    cx: &mut App,
) {
    if cx.update_entity(owner, |owner, _| {
        owner.take_payload_recovery_installation_panic_for_test(stage)
    }) {
        panic!("injected panic after payload recovery installation stage {stage:?}");
    }
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
    let prepared = cx.update_entity(owner, |owner, owner_cx| {
        owner.prepare_payload_recovery_restore_final(execution, restore, owner_cx)
    })?;
    let receipt = with_detached_root_transaction(owner, cx, |transaction, cx| {
        cx.update_entity(owner, |owner, owner_cx| {
            owner.commit_payload_recovery_restore_final(transaction, prepared, owner_cx)
        })
    });
    let focus = catch_unwind(AssertUnwindSafe(|| {
        install_recovery_focus(owner, destination, execution.action(), &receipt, cx);
    }));
    let (tombstone_retired, execution_retired) = cx.update_entity(owner, |owner, _| {
        (
            owner.retire_committed_payload_recovery_restore(execution.action(), &receipt),
            owner.cancel_payload_recovery_execution(execution),
        )
    });
    assert!(
        tombstone_retired,
        "committed non-rehost recovery must retire its exact replay tombstone"
    );
    assert!(
        execution_retired,
        "committed non-rehost recovery must retire its exact executor reservation"
    );
    if let Err(payload) = focus {
        resume_unwind(payload);
    }
    Ok(())
}

pub(crate) fn payload_recovery_source_proxy_committed(
    owner: WeakEntity<DockSurfaceOwner>,
    host: WeakEntity<DockHost>,
    key: DockHostRecoveryPresentationKey,
    accepted_frame: u64,
    cx: &mut App,
) {
    cx.defer(move |cx| {
        handle_payload_recovery_source_proxy_committed(owner, host, key, accepted_frame, cx);
    });
}

fn handle_payload_recovery_source_proxy_committed(
    owner: WeakEntity<DockSurfaceOwner>,
    host: WeakEntity<DockHost>,
    key: DockHostRecoveryPresentationKey,
    accepted_frame: u64,
    cx: &mut App,
) {
    let Some((owner, transfer)) = transfer_for_presentation(&owner, key, cx) else {
        return;
    };
    if key != transfer.source_presentation() || transfer.source().host() != &host {
        return;
    }
    let accepted = cx.update_entity(&owner, |owner, _| {
        owner.accept_payload_recovery_source_proxy_frame(key, accepted_frame)
    });
    let Some(Ok(commit)) = accepted else {
        payload_recovery_presentation_failed(owner.downgrade(), host, key, cx);
        return;
    };
    if commit.frame_generation() != accepted_frame {
        payload_recovery_presentation_failed(owner.downgrade(), host, key, cx);
        return;
    }
    let controller = cx.read_entity(&owner, |owner, _| owner.controller());
    if !source_endpoint_is_exact(transfer.source(), &owner, &controller, cx) {
        payload_recovery_presentation_failed(owner.downgrade(), host, key, cx);
        return;
    }
    let frozen = host
        .update(cx, |host, host_cx| {
            host.mark_payload_recovery_source_frozen(key, host_cx)
        })
        .unwrap_or(false);
    if !frozen {
        payload_recovery_presentation_failed(owner.downgrade(), host, key, cx);
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
    leases: view_presentation_window::LeaseBatch,
    accepted_frame: u64,
    cx: &mut App,
) {
    cx.defer(move |cx| {
        handle_payload_recovery_destination_mounted(owner, host, key, leases, accepted_frame, cx);
    });
}

fn handle_payload_recovery_destination_mounted(
    owner: WeakEntity<DockSurfaceOwner>,
    host: WeakEntity<DockHost>,
    key: DockHostRecoveryPresentationKey,
    leases: view_presentation_window::LeaseBatch,
    accepted_frame: u64,
    cx: &mut App,
) {
    let Some((owner, transfer)) = transfer_for_presentation(&owner, key, cx) else {
        return;
    };
    if key != transfer.destination_presentation()
        || transfer.destination().host() != &host
        || !leases.matches_exactly(transfer.projection().destination())
    {
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
        payload_recovery_presentation_failed(owner.downgrade(), host, key, cx);
        return;
    }
    let exposed = cx.update_entity(&owner, |owner, owner_cx| {
        owner.accept_payload_recovery_destination_frame(key, accepted_frame, owner_cx)
    });
    let Some(Ok(exposed)) = exposed else {
        payload_recovery_presentation_failed(owner.downgrade(), host, key, cx);
        return;
    };
    if exposed.frame_generation() != accepted_frame || !exposed.batch().matches_exactly(&leases) {
        payload_recovery_presentation_failed(owner.downgrade(), host, key, cx);
        return;
    }
    let installed = host
        .update(cx, |host, host_cx| {
            host.expose_payload_recovery_destination_projection(key, exposed, host_cx)
        })
        .unwrap_or(false);
    if !installed {
        payload_recovery_presentation_failed(owner.downgrade(), host, key, cx);
    }
}

pub(crate) fn payload_recovery_destination_presented(
    owner: WeakEntity<DockSurfaceOwner>,
    host: WeakEntity<DockHost>,
    key: DockHostRecoveryPresentationKey,
    leases: view_presentation_window::LeaseBatch,
    accepted_frame: u64,
    cx: &mut App,
) {
    cx.defer(move |cx| {
        handle_payload_recovery_destination_presented(owner, host, key, leases, accepted_frame, cx);
    });
}

fn handle_payload_recovery_destination_presented(
    owner: WeakEntity<DockSurfaceOwner>,
    host: WeakEntity<DockHost>,
    key: DockHostRecoveryPresentationKey,
    leases: view_presentation_window::LeaseBatch,
    accepted_frame: u64,
    cx: &mut App,
) {
    let Some((owner, transfer)) = transfer_for_presentation(&owner, key, cx) else {
        return;
    };
    if key != transfer.destination_presentation()
        || transfer.destination().host() != &host
        || !leases.matches_exactly(transfer.projection().destination())
    {
        return;
    }
    let presented = cx.update_entity(&owner, |owner, owner_cx| {
        owner.accept_payload_recovery_destination_presentation_frame(key, accepted_frame, owner_cx)
    });
    let Some(Ok(presented)) = presented else {
        payload_recovery_presentation_failed(owner.downgrade(), host, key, cx);
        return;
    };
    if presented.frame_generation() != accepted_frame
        || presented.window_id() != leases.window_id()
        || presented.root_count() != leases.leases().len()
    {
        payload_recovery_presentation_failed(owner.downgrade(), host, key, cx);
        return;
    }
    let Some(queued) = cx.update_entity(&owner, |owner, _| {
        owner.queue_payload_recovery_finalization(key, presented)
    }) else {
        return;
    };
    if queued.key() != transfer.key() {
        return;
    }
    let execution = queued.key();
    #[cfg(test)]
    if cx.update_entity(&owner, |owner, _| {
        owner.take_payload_recovery_finalization_pause_for_test()
    }) {
        return;
    }
    cx.defer(move |cx| {
        finalize_payload_recovery_restore(owner, execution, cx);
    });
}

pub(crate) fn payload_recovery_presentation_failed(
    owner: WeakEntity<DockSurfaceOwner>,
    host: WeakEntity<DockHost>,
    key: DockHostRecoveryPresentationKey,
    cx: &mut App,
) {
    let Some((owner, transfer)) = transfer_for_presentation(&owner, key, cx) else {
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
            let _ = settle_payload_recovery_after_source_failure(&owner, &transfer, cx);
        } else if cx.read_entity(&owner, |owner, _| {
            owner.payload_recovery_source_settlement_started(transfer.key())
        }) {
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
    let _ = settle_payload_recovery_after_source_failure(owner, &transfer, cx);
}

pub(crate) fn payload_recovery_source_restoration_frame_committed(
    owner: WeakEntity<DockSurfaceOwner>,
    host: WeakEntity<DockHost>,
    key: DockHostRecoveryPresentationKey,
    accepted_frame: u64,
    cx: &mut App,
) {
    cx.defer(move |cx| {
        let Some((owner, transfer)) = transfer_for_presentation(&owner, key, cx) else {
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
        complete_payload_recovery_source_restoration(&owner, &transfer, accepted_frame, cx);
    });
}

fn finalize_payload_recovery_restore(
    owner: Entity<DockSurfaceOwner>,
    execution: DockPayloadRecoveryExecutionKey,
    cx: &mut App,
) {
    let Some((transfer, finalization)) = cx.read_entity(&owner, |owner, _| {
        owner.payload_recovery_finalization(execution)
    }) else {
        return;
    };
    match catch_unwind(AssertUnwindSafe(|| {
        try_finalize_payload_recovery_restore(&owner, &transfer, &finalization, cx)
    })) {
        Ok(Ok(())) => {}
        Ok(Err(())) => handle_payload_recovery_finalization_failure(
            &owner,
            &transfer,
            &finalization,
            false,
            cx,
        ),
        Err(_) => {
            handle_payload_recovery_finalization_failure(&owner, &transfer, &finalization, true, cx)
        }
    }
}

fn handle_payload_recovery_finalization_failure(
    owner: &Entity<DockSurfaceOwner>,
    transfer: &DockPayloadRecoveryTransfer,
    finalization: &DockPayloadRecoveryFinalization,
    panicked: bool,
    cx: &mut App,
) {
    let terminal = cx.read_entity(owner, |owner, _| {
        owner.payload_recovery_rehost_terminal_disposition(transfer.key())
    });
    if terminal != Some(view_presentation_window::RehostTerminalDisposition::DestinationCommitted) {
        rollback_payload_recovery_transfer(owner, transfer, cx);
        return;
    }

    if finalization.record_failure_and_should_retry() {
        if panicked {
            log::error!(
                "payload recovery finalization panicked; retrying exact execution {:?} within its bounded budget",
                transfer.key()
            );
        }
        #[cfg(test)]
        if cx.update_entity(owner, |owner, _| {
            owner.take_payload_recovery_finalization_retry_pause_for_test()
        }) {
            return;
        }
        let owner = owner.clone();
        let execution = transfer.key();
        cx.defer(move |cx| finalize_payload_recovery_restore(owner, execution, cx));
        return;
    }

    log::error!(
        "payload recovery finalization exhausted its retry budget; retiring exact execution {:?}",
        transfer.key()
    );
    if catch_unwind(AssertUnwindSafe(|| {
        terminate_failed_payload_recovery_finalization(owner, transfer, finalization, cx);
    }))
    .is_err()
    {
        log::error!(
            "payload recovery terminal compensation panicked for execution {:?}",
            transfer.key()
        );
        let receipt = finalization.snapshot().owner_receipt;
        let _ = cx.update_entity(owner, |owner, _| {
            if let Some(receipt) = receipt.as_ref() {
                let _ = owner
                    .retire_committed_payload_recovery_restore(transfer.key().action(), receipt);
            }
            owner.cancel_payload_recovery_execution(transfer.key())
        });
    }
}

fn terminate_failed_payload_recovery_finalization(
    owner: &Entity<DockSurfaceOwner>,
    transfer: &DockPayloadRecoveryTransfer,
    finalization: &DockPayloadRecoveryFinalization,
    cx: &mut App,
) {
    let progress = finalization.snapshot();
    let receipt = progress.owner_receipt.or_else(|| {
        cx.read_entity(owner, |owner, _| {
            owner.committed_payload_recovery_restore_receipt(transfer.key().action())
        })
    });
    let controller = cx.read_entity(owner, |owner, _| owner.controller());
    let destination_is_exact = destination_endpoint_is_exact(
        transfer.destination(),
        owner,
        &controller,
        transfer.key().action(),
        cx,
    );
    let retired = cx.update_entity(owner, |owner, _| {
        if let Some(receipt) = receipt.as_ref() {
            let _ =
                owner.retire_committed_payload_recovery_restore(transfer.key().action(), receipt);
        }
        owner.cancel_payload_recovery_execution(transfer.key())
    });
    if !retired {
        log::error!(
            "payload recovery terminal compensation could not retire execution {:?}",
            transfer.key()
        );
        return;
    }

    if receipt.is_none() || !destination_is_exact {
        view_presentation_window::release_stable_batch_after_endpoint_loss(
            cx,
            transfer.projection().destination(),
        );
    }
    abandon_transfer_host_presentations(transfer, cx);
    if let Some(receipt) = receipt.filter(|_| destination_is_exact) {
        install_recovery_focus(
            owner,
            transfer.destination(),
            transfer.key().action(),
            &receipt,
            cx,
        );
    }
}

#[cfg(test)]
pub(crate) fn resume_payload_recovery_finalization_for_test(
    owner: Entity<DockSurfaceOwner>,
    execution: DockPayloadRecoveryExecutionKey,
    cx: &mut App,
) {
    finalize_payload_recovery_restore(owner, execution, cx);
}

fn try_finalize_payload_recovery_restore(
    owner: &Entity<DockSurfaceOwner>,
    transfer: &DockPayloadRecoveryTransfer,
    finalization: &DockPayloadRecoveryFinalization,
    cx: &mut App,
) -> Result<(), ()> {
    let presented = finalization.presented();
    let progress = finalization.snapshot();
    let controller = cx.read_entity(owner, |owner, _| owner.controller());
    if presented.window_id() != transfer.projection().destination().window_id()
        || presented.root_count() != transfer.projection().destination().leases().len()
    {
        return Err(());
    }

    let provider_committed = progress.provider_committed
        || cx.read_entity(owner, |owner, _| {
            owner.payload_recovery_rehost_terminal_disposition(transfer.key())
                == Some(view_presentation_window::RehostTerminalDisposition::DestinationCommitted)
        });
    if provider_committed {
        finalization.mark_provider_committed();
    }
    let source_is_exact = source_endpoint_is_exact(transfer.source(), owner, &controller, cx);
    let destination_is_exact = destination_endpoint_is_exact(
        transfer.destination(),
        owner,
        &controller,
        transfer.key().action(),
        cx,
    );
    if !provider_committed && (!source_is_exact || !destination_is_exact) {
        return Err(());
    }

    let mut committed_receipt = progress.owner_receipt.or_else(|| {
        cx.read_entity(owner, |owner, _| {
            owner.committed_payload_recovery_restore_receipt(transfer.key().action())
        })
    });
    if let Some(receipt) = committed_receipt.clone() {
        finalization.mark_owner_committed(receipt);
    }
    if committed_receipt.is_none() && !destination_is_exact {
        return Err(());
    }

    let source_commit = if progress.source_host_settled {
        None
    } else if !source_is_exact {
        finalization.mark_source_host_settled();
        None
    } else {
        let source = transfer.source().host().upgrade().ok_or(())?;
        if cx.read_entity(&source, |host, _| {
            host.payload_recovery_source_retirement_is_committed(
                transfer.source_presentation(),
                transfer.projection().source(),
            )
        }) {
            finalization.mark_source_host_settled();
            None
        } else {
            Some((
                source.clone(),
                cx.read_entity(&source, |host, _| {
                    host.prepare_payload_recovery_source_retirement(transfer.source_presentation())
                })
                .ok_or(())?,
            ))
        }
    };
    let destination_commit = if progress.destination_host_settled {
        None
    } else if !destination_is_exact {
        finalization.mark_destination_host_settled();
        None
    } else {
        let destination = transfer.destination().host().upgrade().ok_or(())?;
        if cx.read_entity(&destination, |host, _| {
            host.payload_recovery_destination_is_committed(
                transfer.destination_presentation(),
                transfer.projection().destination(),
            )
        }) {
            finalization.mark_destination_host_settled();
            None
        } else {
            Some((
                destination.clone(),
                cx.read_entity(&destination, |host, _| {
                    host.prepare_payload_recovery_destination_commit(
                        transfer.destination_presentation(),
                        presented,
                    )
                })
                .ok_or(())?,
            ))
        }
    };
    let owner_commit = if committed_receipt.is_some() {
        None
    } else {
        Some(
            cx.update_entity(owner, |owner, owner_cx| {
                owner.prepare_payload_recovery_restore_final(
                    transfer.key(),
                    transfer.restore().clone(),
                    owner_cx,
                )
            })
            .map_err(|_| ())?,
        )
    };
    let presentation_commit = cx
        .update_entity(owner, |owner, owner_cx| {
            owner.prepare_payload_recovery_destination_terminal(
                transfer.key(),
                &presented,
                owner_cx,
            )
        })
        .ok_or(())?
        .map_err(|_| ())?;
    if !source_commit
        .as_ref()
        .is_none_or(|(source, source_commit)| {
            cx.read_entity(source, |host, _| {
                host.can_commit_prepared_payload_recovery_source_retirement(source_commit)
            })
        })
        || !destination_commit
            .as_ref()
            .is_none_or(|(destination, destination_commit)| {
                cx.read_entity(destination, |host, _| {
                    host.can_commit_prepared_payload_recovery_destination(destination_commit)
                })
            })
        || !owner_commit.as_ref().is_none_or(|owner_commit| {
            cx.update_entity(owner, |owner, owner_cx| {
                owner.can_commit_payload_recovery_restore_final(owner_commit, owner_cx)
            })
        })
        || !presentation_commit.can_commit(cx)
    {
        return Err(());
    }

    let expected_destination = transfer.projection().destination().clone();
    let outcome = presentation_commit
        .try_commit(cx)
        .expect("preflighted payload recovery terminal must remain exact");
    let view_presentation_window::RehostTerminalOutcome::DestinationCommitted(
        presentation_destination,
    ) = outcome
    else {
        panic!("destination finalization must commit destination presentation authority");
    };
    assert!(presentation_destination.matches_exactly(&expected_destination));
    finalization.mark_provider_committed();
    #[cfg(test)]
    panic_after_payload_recovery_finalization_stage_for_test(
        owner,
        DockPayloadRecoveryFinalizationPanicStage::Provider,
        cx,
    );

    if let Some((source, source_commit)) = source_commit {
        cx.update_entity(&source, |host, host_cx| {
            host.commit_prepared_payload_recovery_source_retirement(source_commit, host_cx);
        });
    }
    finalization.mark_source_host_settled();
    #[cfg(test)]
    panic_after_payload_recovery_finalization_stage_for_test(
        owner,
        DockPayloadRecoveryFinalizationPanicStage::SourceHost,
        cx,
    );
    if let Some((destination, destination_commit)) = destination_commit {
        cx.update_entity(&destination, |host, host_cx| {
            host.commit_prepared_payload_recovery_destination(destination_commit, host_cx);
        });
    }
    finalization.mark_destination_host_settled();
    #[cfg(test)]
    panic_after_payload_recovery_finalization_stage_for_test(
        owner,
        DockPayloadRecoveryFinalizationPanicStage::DestinationHost,
        cx,
    );
    let receipt = if let Some(owner_commit) = owner_commit {
        with_detached_root_transaction(owner, cx, |transaction, cx| {
            cx.update_entity(owner, |owner, owner_cx| {
                owner.commit_payload_recovery_restore_final(transaction, owner_commit, owner_cx)
            })
        })
    } else {
        committed_receipt
            .take()
            .expect("committed recovery must retain its exact receipt")
    };
    finalization.mark_owner_committed(receipt.clone());
    #[cfg(test)]
    panic_after_payload_recovery_finalization_stage_for_test(
        owner,
        DockPayloadRecoveryFinalizationPanicStage::Owner,
        cx,
    );
    if !progress.focus_installed {
        install_recovery_focus(
            owner,
            transfer.destination(),
            transfer.key().action(),
            &receipt,
            cx,
        );
        finalization.mark_focus_installed();
    }
    let execution_retired = cx.update_entity(owner, |owner, _| {
        let _ = owner.retire_committed_payload_recovery_restore(transfer.key().action(), &receipt);
        owner.cancel_payload_recovery_execution(transfer.key())
    });
    assert!(
        execution_retired,
        "fully committed payload recovery must retire its exact terminal executor"
    );
    Ok(())
}

#[cfg(test)]
fn panic_after_payload_recovery_finalization_stage_for_test(
    owner: &Entity<DockSurfaceOwner>,
    stage: DockPayloadRecoveryFinalizationPanicStage,
    cx: &mut App,
) {
    if cx.update_entity(owner, |owner, _| {
        owner.take_payload_recovery_finalization_panic_for_test(stage)
    }) {
        panic!("injected panic after payload recovery finalization stage {stage:?}");
    }
}

fn transfer_for_presentation(
    owner: &WeakEntity<DockSurfaceOwner>,
    key: DockHostRecoveryPresentationKey,
    cx: &App,
) -> Option<(Entity<DockSurfaceOwner>, DockPayloadRecoveryTransfer)> {
    let owner = owner.upgrade()?;
    let transfer = cx.read_entity(&owner, |owner, _| {
        owner.payload_recovery_transfer_for_presentation(key)
    })?;
    (transfer.key().action() == key.action()
        && key.rehost_generation() == transfer.projection().generation())
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

fn abandon_rehost_session_after_source_loss(
    session: &mut view_presentation_window::RehostSession,
    cx: &mut App,
) -> bool {
    matches!(
        session.abandon_after_source_loss(cx),
        Ok(
            view_presentation_window::RehostAbandonmentOutcome::Abandoned(_)
                | view_presentation_window::RehostAbandonmentOutcome::AlreadyAbandoned(_)
        )
    )
}

fn retire_rehost_session_to_source(
    session: &mut view_presentation_window::RehostSession,
    cx: &mut App,
) -> bool {
    match session.settle_source(cx) {
        Ok(
            view_presentation_window::SourceSettlement::RetiredToSource(_)
            | view_presentation_window::SourceSettlement::AlreadyRetired,
        ) => true,
        Ok(
            view_presentation_window::SourceSettlement::RenderSource(_)
            | view_presentation_window::SourceSettlement::AwaitingSourceNativeTerminal
            | view_presentation_window::SourceSettlement::PresentationAuthorityReleased(_),
        )
        | Err(_) => abandon_rehost_session_after_source_loss(session, cx),
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

    match cx.update_entity(owner, |owner, owner_cx| {
        owner.settle_payload_recovery_source(transfer.key(), owner_cx)
    }) {
        Some(Ok(
            view_presentation_window::SourceSettlement::RetiredToSource(_)
            | view_presentation_window::SourceSettlement::AlreadyRetired,
        )) => {
            abandon_host_presentation(transfer.source().host(), transfer.source_presentation(), cx);
            abandon_host_presentation(
                transfer.destination().host(),
                transfer.destination_presentation(),
                cx,
            );
            finish_execution(owner, transfer.key(), cx);
        }
        Some(Ok(view_presentation_window::SourceSettlement::RenderSource(restored))) => {
            begin_payload_recovery_source_restoration(owner, transfer, restored, cx);
        }
        Some(Ok(view_presentation_window::SourceSettlement::AwaitingSourceNativeTerminal)) => {
            let terminal_seen = cx.read_entity(owner, |owner, _| {
                owner.payload_recovery_source_native_terminal_seen(transfer.key())
            });
            if terminal_seen {
                let _ = abandon_payload_recovery_after_source_loss(owner, transfer, cx);
            }
        }
        Some(Ok(view_presentation_window::SourceSettlement::PresentationAuthorityReleased(_)))
        | Some(Err(_)) => {
            let _ = abandon_payload_recovery_after_source_loss(owner, transfer, cx);
        }
        None => {}
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
    let expected_source = transfer.projection().source();
    let restored_is_exact = restored.window_id() == expected_source.window_id()
        && restored.leases().len() == expected_source.leases().len()
        && expected_source
            .leases()
            .iter()
            .all(|lease| restored.lease_for(lease.entity_id()).is_some());
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
    accepted_frame: u64,
    cx: &mut App,
) {
    let Some(source_host) = transfer.source().host().upgrade() else {
        settle_payload_recovery_after_source_failure(owner, transfer, cx);
        return;
    };
    let Some((phase, restored)) = cx.read_entity(&source_host, |host, _| {
        let state = host.payload_recovery_presentation_state()?;
        if state.key != transfer.source_presentation() {
            return None;
        }
        match &state.mode {
            DockHostRecoveryPresentationMode::SourceRestoration { leases, phase, .. } => {
                Some((*phase, leases.clone()))
            }
            DockHostRecoveryPresentationMode::SourceProjection { .. }
            | DockHostRecoveryPresentationMode::DestinationProjection { .. } => None,
        }
    }) else {
        settle_payload_recovery_after_source_failure(owner, transfer, cx);
        return;
    };

    if phase == DockHostRecoverySourceRestorationPhase::Staging {
        let finished = cx.update_entity(owner, |owner, owner_cx| {
            owner.accept_payload_recovery_source_restoration_frame(
                transfer.source_presentation(),
                accepted_frame,
                owner_cx,
            )
        });
        let Some(Ok(view_presentation_window::SourcePresentationFinish::Finished(finished))) =
            finished
        else {
            settle_payload_recovery_after_source_failure(owner, transfer, cx);
            return;
        };
        if !finished.matches_exactly(&restored) {
            settle_payload_recovery_after_source_failure(owner, transfer, cx);
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
            settle_payload_recovery_after_source_failure(owner, transfer, cx);
            return;
        }
        refresh_endpoint(transfer.source(), cx);
        return;
    }

    let Some(presented) =
        view_presentation_window::stable_batch_presentation_receipt(cx, &restored)
    else {
        settle_payload_recovery_after_source_failure(owner, transfer, cx);
        return;
    };
    let Some(source_commit) = cx.read_entity(&source_host, |host, _| {
        host.prepare_payload_recovery_source_restoration_commit(
            transfer.source_presentation(),
            presented,
        )
    }) else {
        settle_payload_recovery_after_source_failure(owner, transfer, cx);
        return;
    };
    if !cx.read_entity(&source_host, |host, _| {
        host.can_commit_prepared_payload_recovery_source_restoration(&source_commit)
    }) {
        settle_payload_recovery_after_source_failure(owner, transfer, cx);
        return;
    }
    cx.update_entity(&source_host, |host, host_cx| {
        host.commit_prepared_payload_recovery_source_restoration(source_commit, host_cx);
    });
    let _ = finish_payload_recovery_source_terminal(owner, transfer, cx);
    refresh_endpoint(transfer.source(), cx);
}

fn settle_payload_recovery_after_source_failure(
    owner: &Entity<DockSurfaceOwner>,
    transfer: &DockPayloadRecoveryTransfer,
    cx: &mut App,
) -> bool {
    match cx.read_entity(owner, |owner, _| {
        owner.payload_recovery_rehost_terminal_disposition(transfer.key())
    }) {
        Some(
            view_presentation_window::RehostTerminalDisposition::SourceCommitted
            | view_presentation_window::RehostTerminalDisposition::Abandoned
            | view_presentation_window::RehostTerminalDisposition::PresentationAuthorityReleased,
        ) => finish_payload_recovery_source_terminal(owner, transfer, cx),
        Some(view_presentation_window::RehostTerminalDisposition::DestinationCommitted) => false,
        None => abandon_payload_recovery_after_source_loss(owner, transfer, cx),
    }
}

fn finish_payload_recovery_source_terminal(
    owner: &Entity<DockSurfaceOwner>,
    transfer: &DockPayloadRecoveryTransfer,
    cx: &mut App,
) -> bool {
    let source_retired =
        abandon_host_presentation(transfer.source().host(), transfer.source_presentation(), cx);
    let destination_retired = abandon_host_presentation(
        transfer.destination().host(),
        transfer.destination_presentation(),
        cx,
    );
    if !source_retired || !destination_retired {
        return false;
    }
    cx.update_entity(owner, |owner, _| {
        owner.cancel_payload_recovery_execution(transfer.key())
    })
}

fn abandon_payload_recovery_after_source_loss(
    owner: &Entity<DockSurfaceOwner>,
    transfer: &DockPayloadRecoveryTransfer,
    cx: &mut App,
) -> bool {
    let authority_released = matches!(
        cx.update_entity(owner, |owner, owner_cx| {
            owner.abandon_payload_recovery_rehost_after_source_loss(transfer.key(), owner_cx)
        }),
        Some(Ok(
            view_presentation_window::RehostAbandonmentOutcome::Abandoned(_)
                | view_presentation_window::RehostAbandonmentOutcome::AlreadyAbandoned(_)
        ))
    );
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
