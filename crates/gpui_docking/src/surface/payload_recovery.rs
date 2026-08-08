//! Generation-bound payload recovery owned by one docking surface.
//!
//! The module proves payload survival from the current graph at both preparation and commit. It
//! retains identities and proof locations, never an old graph or a projected move plan.

use super::live_undock::{
    DockLiveUndockIdentity, DockLiveUndockPayloadLeaseReceipt, DockLiveUndockPromotionDestination,
    DockLiveUndockPromotionToken, DockLiveUndockSourceFocusSnapshot,
};
use super::window_session::DockSurfaceWindowSessionLease;
use crate::{
    DockGraph, DockGraphDropTarget, DockGraphMutationError, DockHost, DockItemId, DockNode,
    DockNodeId, DockOp, DockSpaceId, host::DockHostWindowBinding,
    locked_drop_identity::DockLockedPayloadIdentity,
    viewport_registry::DockViewportRegistrationKey,
};
use open_gpui::{FocusHandle, WindowHandle, view_presentation_window};

/// Why a live payload needs a durable surface-owned recovery entry.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum DockPayloadRecoveryReason {
    /// A provisional viewport failed before the payload became durable there.
    PreCommitOrphan,
    /// A committed payload outlived the viewport that presented it.
    LostViewportRecovery,
}

/// Exact authority for one surface-owned payload recovery record.
///
/// Pre-commit orphan recovery is authorized by the presentation lease which moved the payload
/// out of its source presentation. Post-boundary recovery is instead authorized by the committed
/// destination identity. That identity may come from a complete Dock promotion or directly from
/// the GPUI presentation provider after it crossed its irreversible commit boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum DockPayloadRecoveryAuthority {
    PresentationLease(DockLiveUndockPayloadLeaseReceipt),
    CommittedDestination {
        identity: DockLiveUndockIdentity,
        token: DockLiveUndockPromotionToken,
        destination: DockLiveUndockPromotionDestination,
    },
}

impl DockPayloadRecoveryAuthority {
    pub(crate) const fn presentation_lease(lease: DockLiveUndockPayloadLeaseReceipt) -> Self {
        Self::PresentationLease(lease)
    }

    pub(crate) const fn committed_destination(
        identity: DockLiveUndockIdentity,
        token: DockLiveUndockPromotionToken,
        destination: DockLiveUndockPromotionDestination,
    ) -> Self {
        Self::CommittedDestination {
            identity,
            token,
            destination,
        }
    }

    pub(crate) const fn live_identity(self) -> DockLiveUndockIdentity {
        match self {
            Self::PresentationLease(lease) => lease.identity(),
            Self::CommittedDestination { identity, .. } => identity,
        }
    }

    pub(crate) const fn presentation(self) -> Option<DockLiveUndockPayloadLeaseReceipt> {
        match self {
            Self::PresentationLease(lease) => Some(lease),
            Self::CommittedDestination { .. } => None,
        }
    }

    pub(crate) const fn promotion(
        self,
    ) -> Option<(
        DockLiveUndockIdentity,
        DockLiveUndockPromotionToken,
        DockLiveUndockPromotionDestination,
    )> {
        match self {
            Self::PresentationLease(_) => None,
            Self::CommittedDestination {
                identity,
                token,
                destination,
            } => Some((identity, token, destination)),
        }
    }

    const fn admits_reason(self, reason: DockPayloadRecoveryReason) -> bool {
        matches!(
            (self, reason),
            (
                Self::PresentationLease(_),
                DockPayloadRecoveryReason::PreCommitOrphan
            ) | (
                Self::CommittedDestination { .. },
                DockPayloadRecoveryReason::LostViewportRecovery
            )
        )
    }
}

/// Monotonic identity of one surface-owned recovery reservation.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct DockPayloadRecoveryGeneration(u64);

impl DockPayloadRecoveryGeneration {
    pub(crate) const fn get(self) -> u64 {
        self.0
    }
}

/// Durable result of committing one payload recovery reservation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum DockPayloadRecoveryDisposition {
    /// The surface must expose a recovery entry for payload that is not visible in its primary
    /// space.
    VisibleRecoveryEntry,
    /// Merge-back already placed the payload in the live primary space.
    AlreadyRehomed,
    /// The locked payload is missing from, or ambiguous in, the current authoritative graph.
    /// The durable record remains visible for diagnosis instead of silently retiring recovery.
    Unresolved,
}

/// Exact presentation authority that owned one committed payload when recovery was recorded.
///
/// A registered Host origin must revalidate its root Host, binding, and viewport registration. A
/// provider-terminal origin instead carries the exact immutable lease batch that crossed GPUI's
/// commit boundary. Neither variant keeps the window alive or permits guessing another source.
#[derive(Clone, Debug)]
pub(crate) enum DockPayloadRecoveryPresentationOrigin {
    RegisteredHost {
        window: WindowHandle<DockHost>,
        binding: DockHostWindowBinding,
        registration: DockViewportRegistrationKey,
    },
    ProviderTerminal {
        window: WindowHandle<DockHost>,
        leases: view_presentation_window::LeaseBatch,
    },
}

impl DockPayloadRecoveryPresentationOrigin {
    pub(crate) fn new(
        window: WindowHandle<DockHost>,
        binding: DockHostWindowBinding,
        registration: DockViewportRegistrationKey,
    ) -> Option<Self> {
        let window_id = window.window_id();
        (binding.window_id() == window_id && registration.window_id() == window_id).then_some(
            Self::RegisteredHost {
                window,
                binding,
                registration,
            },
        )
    }

    pub(crate) fn provider_terminal(
        window: WindowHandle<DockHost>,
        leases: view_presentation_window::LeaseBatch,
    ) -> Option<Self> {
        (window.window_id() == leases.window_id())
            .then_some(Self::ProviderTerminal { window, leases })
    }

    pub(crate) const fn window(&self) -> WindowHandle<DockHost> {
        match self {
            Self::RegisteredHost { window, .. } | Self::ProviderTerminal { window, .. } => *window,
        }
    }

    pub(crate) fn registered_host(
        &self,
    ) -> Option<(DockHostWindowBinding, &DockViewportRegistrationKey)> {
        match self {
            Self::RegisteredHost {
                binding,
                registration,
                ..
            } => Some((*binding, registration)),
            Self::ProviderTerminal { .. } => None,
        }
    }

    pub(crate) fn provider_terminal_leases(&self) -> Option<&view_presentation_window::LeaseBatch> {
        match self {
            Self::ProviderTerminal { leases, .. } => Some(leases),
            Self::RegisteredHost { .. } => None,
        }
    }
}

impl PartialEq for DockPayloadRecoveryPresentationOrigin {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (
                Self::RegisteredHost {
                    window: left_window,
                    binding: left_binding,
                    registration: left_registration,
                },
                Self::RegisteredHost {
                    window: right_window,
                    binding: right_binding,
                    registration: right_registration,
                },
            ) => {
                left_window == right_window
                    && left_binding == right_binding
                    && left_registration == right_registration
            }
            (
                Self::ProviderTerminal {
                    window: left_window,
                    leases: left_leases,
                },
                Self::ProviderTerminal {
                    window: right_window,
                    leases: right_leases,
                },
            ) => left_window == right_window && left_leases.matches_exactly(right_leases),
            (Self::RegisteredHost { .. }, Self::ProviderTerminal { .. })
            | (Self::ProviderTerminal { .. }, Self::RegisteredHost { .. }) => false,
        }
    }
}

impl Eq for DockPayloadRecoveryPresentationOrigin {}

/// Opaque, generation-bound authorization to attempt one recovery commit.
///
/// The token deliberately remains cloneable so asynchronous holders can hand it across effect
/// boundaries. The registry still admits exactly one matching generation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct DockPayloadRecoveryPrepared {
    generation: DockPayloadRecoveryGeneration,
    owner_revision: u64,
    authority: DockPayloadRecoveryAuthority,
    payload_identity: DockLockedPayloadIdentity,
    evidence: DockPayloadRecoveryEvidence,
    reason: DockPayloadRecoveryReason,
    focus: Option<DockPayloadRecoveryFocus>,
    presentation_origin: Option<DockPayloadRecoveryPresentationOrigin>,
}

impl DockPayloadRecoveryPrepared {
    pub(crate) const fn generation(&self) -> DockPayloadRecoveryGeneration {
        self.generation
    }

    pub(crate) const fn owner_revision(&self) -> u64 {
        self.owner_revision
    }

    pub(crate) const fn live_identity(&self) -> DockLiveUndockIdentity {
        self.authority.live_identity()
    }

    pub(crate) const fn authority(&self) -> DockPayloadRecoveryAuthority {
        self.authority
    }

    pub(crate) const fn reason(&self) -> DockPayloadRecoveryReason {
        self.reason
    }
}

/// Exact proof that a recovery reservation committed into the surface-owned registry.
///
/// Private fields and the absence of a public constructor prevent a payload lease alone from
/// being promoted into a commit receipt.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct DockPayloadRecoveryCommitReceipt {
    generation: DockPayloadRecoveryGeneration,
    authority: DockPayloadRecoveryAuthority,
    disposition: DockPayloadRecoveryDisposition,
}

impl DockPayloadRecoveryCommitReceipt {
    pub(crate) const fn generation(self) -> DockPayloadRecoveryGeneration {
        self.generation
    }

    pub(crate) const fn live_identity(self) -> DockLiveUndockIdentity {
        self.authority.live_identity()
    }

    pub(crate) const fn authority(self) -> DockPayloadRecoveryAuthority {
        self.authority
    }

    pub(crate) const fn disposition(self) -> DockPayloadRecoveryDisposition {
        self.disposition
    }
}

/// Why a payload recovery reservation could not be prepared.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum DockPayloadRecoveryPrepareError {
    /// The authority kind does not match the requested recovery phase.
    AuthorityReasonMismatch,
    /// The exact recovery authority already produced a durable recovery record.
    PayloadAlreadyCommitted,
    /// No current graph location contains the locked payload.
    PayloadMissing,
    /// More than one current graph location could contain the locked payload.
    PayloadAmbiguous,
    /// The graph no longer has the unresolved condition captured by this preparation attempt.
    UnresolvedEvidenceChanged,
}

/// Why a prepared payload recovery could not cross its final commit boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum DockPayloadRecoveryCommitError {
    /// The reservation was replaced, consumed, or never belonged to this registry.
    StalePreparedToken,
    /// Another committed surface transaction advanced the owner revision.
    OwnerRevisionChanged,
    /// The current graph no longer proves the exact survival location captured at preparation.
    PayloadSurvivalChanged,
}

/// Exact focus evidence retained with a committed lost-viewport payload.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct DockPayloadRecoveryFocus {
    item: Option<DockItemId>,
    descendant: Option<DockLiveUndockSourceFocusSnapshot>,
}

impl DockPayloadRecoveryFocus {
    pub(crate) fn new(
        item: Option<DockItemId>,
        descendant: Option<DockLiveUndockSourceFocusSnapshot>,
    ) -> Self {
        Self { item, descendant }
    }

    pub(crate) fn item(&self) -> Option<&DockItemId> {
        self.item.as_ref()
    }

    pub(crate) fn descendant(&self) -> Option<&DockLiveUndockSourceFocusSnapshot> {
        self.descendant.as_ref()
    }

    fn requests_entry_focus(&self) -> bool {
        self.descendant.is_some()
    }
}

/// Exact, anchor-generation-bound authorization exposed by one visible lost-viewport record.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct DockPayloadRecoveryRestoreAction {
    recovery: DockPayloadRecoveryCommitReceipt,
    anchor_lease: DockSurfaceWindowSessionLease,
}

impl DockPayloadRecoveryRestoreAction {
    pub(crate) const fn anchor_lease(self) -> DockSurfaceWindowSessionLease {
        self.anchor_lease
    }
}

/// Fully projected graph replacement for one exact recovery action.
///
/// Every fallible lookup and graph mutation happens before this value exists. The owning surface
/// may therefore swap `projected_graph` and consume the matching registry record without another
/// graph-level failure boundary.
#[derive(Clone, Debug)]
pub(crate) struct DockPayloadRecoveryRestorePrepared {
    action: DockPayloadRecoveryRestoreAction,
    owner_revision: u64,
    primary_space: DockSpaceId,
    source_location: DockPayloadRecoveryLocation,
    target_tabs: Option<DockNodeId>,
    projected_graph: DockGraph,
    items: Vec<DockItemId>,
    focus_item: Option<DockItemId>,
    descendant_focus: Option<DockLiveUndockSourceFocusSnapshot>,
    presentation_origin: Option<DockPayloadRecoveryPresentationOrigin>,
}

impl DockPayloadRecoveryRestorePrepared {
    pub(crate) const fn action(&self) -> DockPayloadRecoveryRestoreAction {
        self.action
    }

    pub(crate) fn projected_graph(&self) -> &DockGraph {
        &self.projected_graph
    }

    pub(crate) fn source_space(&self) -> &DockSpaceId {
        self.source_location.space()
    }

    pub(crate) fn primary_space(&self) -> &DockSpaceId {
        &self.primary_space
    }

    pub(crate) fn items(&self) -> &[DockItemId] {
        &self.items
    }

    pub(crate) fn presentation_origin(&self) -> Option<&DockPayloadRecoveryPresentationOrigin> {
        self.presentation_origin.as_ref()
    }
}

/// Durable acknowledgement that one exact recovery record was consumed.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct DockPayloadRecoveryRestoreReceipt {
    recovery: DockPayloadRecoveryCommitReceipt,
    focus_item: Option<DockItemId>,
    descendant_focus: Option<DockLiveUndockSourceFocusSnapshot>,
}

impl DockPayloadRecoveryRestoreReceipt {
    pub(crate) const fn recovery(&self) -> DockPayloadRecoveryCommitReceipt {
        self.recovery
    }

    pub(crate) fn focus_item(&self) -> Option<&DockItemId> {
        self.focus_item.as_ref()
    }

    pub(crate) fn descendant_focus(&self) -> Option<&DockLiveUndockSourceFocusSnapshot> {
        self.descendant_focus.as_ref()
    }
}

/// Why a visible recovery action could not prepare or cross its final commit boundary.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum DockPayloadRecoveryRestoreError {
    /// The record was already consumed, replaced, or never belonged to this registry.
    StaleAction,
    /// The action belongs to another anchor generation, or the owning surface has no live anchor.
    AnchorUnavailable,
    /// The durable record retained diagnostic evidence rather than a recoverable payload.
    PayloadUnresolved,
    /// No current graph location contains the payload.
    PayloadMissing,
    /// More than one current graph location could contain the payload.
    PayloadAmbiguous,
    /// The projected repair could not preserve graph invariants.
    GraphMutation(DockGraphMutationError),
    /// The exact recovery action already owns the single in-flight restore execution.
    AlreadyInFlight,
    /// Another recovery action currently owns the single restore execution slot.
    Busy,
    /// The durable recovery record did not retain an exact presentation origin.
    PresentationOriginUnavailable,
    /// The exact source or destination Host generation is no longer available.
    PresentationEndpointUnavailable,
    /// GPUI could not prepare the exact resolved-root presentation transfer.
    PresentationPrepare(open_gpui::view_presentation_window::ResolvedViewRehostError),
    /// A Host rejected installation of the exact recovery presentation generation.
    PresentationInstallRejected,
}

/// One durable recovery fact retained for the lifetime of the owning surface.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct DockPayloadRecoveryRecord {
    receipt: DockPayloadRecoveryCommitReceipt,
    base_owner_revision: u64,
    reason: DockPayloadRecoveryReason,
    payload_identity: DockLockedPayloadIdentity,
    evidence: DockPayloadRecoveryEvidence,
    focus: Option<DockPayloadRecoveryFocus>,
    presentation_origin: Option<DockPayloadRecoveryPresentationOrigin>,
    entry_focus: Option<DockPayloadRecoveryEntryFocus>,
}

impl DockPayloadRecoveryRecord {
    pub(crate) const fn receipt(&self) -> DockPayloadRecoveryCommitReceipt {
        self.receipt
    }

    pub(crate) const fn reason(&self) -> DockPayloadRecoveryReason {
        self.reason
    }

    pub(crate) const fn base_owner_revision(&self) -> u64 {
        self.base_owner_revision
    }

    pub(crate) fn payload_identity(&self) -> &DockLockedPayloadIdentity {
        &self.payload_identity
    }

    pub(crate) const fn disposition(&self) -> DockPayloadRecoveryDisposition {
        self.receipt.disposition
    }

    pub(crate) fn was_rehomed(&self) -> bool {
        matches!(
            &self.evidence,
            DockPayloadRecoveryEvidence::Survived(DockPayloadSurvivalProof::Rehomed(_))
        )
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct DockPayloadRecoveryEntryFocus {
    handle: FocusHandle,
    pending: bool,
}

/// Immutable render projection of one visible surface-owned recovery record.
#[derive(Clone, Debug)]
pub(crate) struct DockPayloadRecoveryEntry {
    action: DockPayloadRecoveryRestoreAction,
    items: Vec<DockItemId>,
    focus_handle: FocusHandle,
    focus_pending: bool,
}

impl DockPayloadRecoveryEntry {
    pub(crate) const fn action(&self) -> DockPayloadRecoveryRestoreAction {
        self.action
    }

    pub(crate) fn items(&self) -> &[DockItemId] {
        &self.items
    }

    pub(crate) const fn generation(&self) -> u64 {
        self.action.recovery.generation.get()
    }

    pub(crate) fn focus_handle(&self) -> &FocusHandle {
        &self.focus_handle
    }

    pub(crate) const fn focus_pending(&self) -> bool {
        self.focus_pending
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum DockPayloadRecoveryEvidence {
    Survived(DockPayloadSurvivalProof),
    Unresolved(DockPayloadRecoveryPrepareError),
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct DockPayloadRecoveryReservation {
    prepared: DockPayloadRecoveryPrepared,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct DockPayloadRecoveryCommittedRestore {
    action: DockPayloadRecoveryRestoreAction,
    receipt: DockPayloadRecoveryRestoreReceipt,
}

/// Surface-owned authority for payload recovery preparation and durable commit.
#[derive(Debug, Default)]
pub(crate) struct DockPayloadRecoveryRegistry {
    last_generation: u64,
    reservations: Vec<DockPayloadRecoveryReservation>,
    records: Vec<DockPayloadRecoveryRecord>,
    committed_restores: Vec<DockPayloadRecoveryCommittedRestore>,
}

impl DockPayloadRecoveryRegistry {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn prepare(
        &mut self,
        graph: &DockGraph,
        owner_revision: u64,
        authority: DockPayloadRecoveryAuthority,
        payload_identity: &DockLockedPayloadIdentity,
        reason: DockPayloadRecoveryReason,
    ) -> Result<DockPayloadRecoveryPrepared, DockPayloadRecoveryPrepareError> {
        self.prepare_with_focus(
            graph,
            owner_revision,
            authority,
            payload_identity,
            reason,
            None,
        )
    }

    pub(crate) fn prepare_with_focus(
        &mut self,
        graph: &DockGraph,
        owner_revision: u64,
        authority: DockPayloadRecoveryAuthority,
        payload_identity: &DockLockedPayloadIdentity,
        reason: DockPayloadRecoveryReason,
        focus: Option<DockPayloadRecoveryFocus>,
    ) -> Result<DockPayloadRecoveryPrepared, DockPayloadRecoveryPrepareError> {
        self.prepare_with_focus_and_origin(
            graph,
            owner_revision,
            authority,
            payload_identity,
            reason,
            focus,
            None,
        )
    }

    pub(crate) fn prepare_with_focus_and_origin(
        &mut self,
        graph: &DockGraph,
        owner_revision: u64,
        authority: DockPayloadRecoveryAuthority,
        payload_identity: &DockLockedPayloadIdentity,
        reason: DockPayloadRecoveryReason,
        focus: Option<DockPayloadRecoveryFocus>,
        presentation_origin: Option<DockPayloadRecoveryPresentationOrigin>,
    ) -> Result<DockPayloadRecoveryPrepared, DockPayloadRecoveryPrepareError> {
        self.begin_preparation(authority, reason)?;
        let survival = DockPayloadSurvivalProof::resolve(graph, payload_identity)?;
        Ok(self.reserve(
            owner_revision,
            authority,
            payload_identity,
            DockPayloadRecoveryEvidence::Survived(survival),
            reason,
            focus,
            presentation_origin,
        ))
    }

    pub(crate) fn prepare_unresolved(
        &mut self,
        graph: &DockGraph,
        owner_revision: u64,
        authority: DockPayloadRecoveryAuthority,
        payload_identity: &DockLockedPayloadIdentity,
        reason: DockPayloadRecoveryReason,
        unresolved: DockPayloadRecoveryPrepareError,
    ) -> Result<DockPayloadRecoveryPrepared, DockPayloadRecoveryPrepareError> {
        self.prepare_unresolved_with_origin(
            graph,
            owner_revision,
            authority,
            payload_identity,
            reason,
            unresolved,
            None,
        )
    }

    pub(crate) fn prepare_unresolved_with_origin(
        &mut self,
        graph: &DockGraph,
        owner_revision: u64,
        authority: DockPayloadRecoveryAuthority,
        payload_identity: &DockLockedPayloadIdentity,
        reason: DockPayloadRecoveryReason,
        unresolved: DockPayloadRecoveryPrepareError,
        presentation_origin: Option<DockPayloadRecoveryPresentationOrigin>,
    ) -> Result<DockPayloadRecoveryPrepared, DockPayloadRecoveryPrepareError> {
        if !matches!(
            unresolved,
            DockPayloadRecoveryPrepareError::PayloadMissing
                | DockPayloadRecoveryPrepareError::PayloadAmbiguous
        ) {
            return Err(unresolved);
        }
        self.begin_preparation(authority, reason)?;
        if DockPayloadSurvivalProof::resolve(graph, payload_identity).err() != Some(unresolved) {
            return Err(DockPayloadRecoveryPrepareError::UnresolvedEvidenceChanged);
        }
        Ok(self.reserve(
            owner_revision,
            authority,
            payload_identity,
            DockPayloadRecoveryEvidence::Unresolved(unresolved),
            reason,
            None,
            presentation_origin,
        ))
    }

    fn begin_preparation(
        &mut self,
        authority: DockPayloadRecoveryAuthority,
        reason: DockPayloadRecoveryReason,
    ) -> Result<(), DockPayloadRecoveryPrepareError> {
        if !authority.admits_reason(reason) {
            return Err(DockPayloadRecoveryPrepareError::AuthorityReasonMismatch);
        }
        if self
            .records
            .iter()
            .any(|record| record.receipt.authority == authority)
        {
            return Err(DockPayloadRecoveryPrepareError::PayloadAlreadyCommitted);
        }

        // A new attempt for the same live generation makes every older token terminal, including
        // when the new graph proof fails.
        self.reservations.retain(|reservation| {
            reservation.prepared.live_identity() != authority.live_identity()
        });
        Ok(())
    }

    fn reserve(
        &mut self,
        owner_revision: u64,
        authority: DockPayloadRecoveryAuthority,
        payload_identity: &DockLockedPayloadIdentity,
        evidence: DockPayloadRecoveryEvidence,
        reason: DockPayloadRecoveryReason,
        focus: Option<DockPayloadRecoveryFocus>,
        presentation_origin: Option<DockPayloadRecoveryPresentationOrigin>,
    ) -> DockPayloadRecoveryPrepared {
        self.last_generation = self
            .last_generation
            .checked_add(1)
            .expect("dock payload recovery generation space exhausted");
        let prepared = DockPayloadRecoveryPrepared {
            generation: DockPayloadRecoveryGeneration(self.last_generation),
            owner_revision,
            authority,
            payload_identity: payload_identity.clone(),
            evidence,
            reason,
            focus,
            presentation_origin,
        };
        self.reservations.push(DockPayloadRecoveryReservation {
            prepared: prepared.clone(),
        });
        prepared
    }

    pub(crate) fn can_commit(
        &self,
        graph: &DockGraph,
        owner_revision: u64,
        prepared: &DockPayloadRecoveryPrepared,
    ) -> bool {
        self.validate_commit(graph, owner_revision, prepared)
            .is_ok()
    }

    pub(crate) fn commit(
        &mut self,
        graph: &DockGraph,
        owner_revision: u64,
        primary_space: &DockSpaceId,
        primary_space_is_live: bool,
        prepared: &DockPayloadRecoveryPrepared,
    ) -> Result<DockPayloadRecoveryCommitReceipt, DockPayloadRecoveryCommitError> {
        let Some(reservation_index) = self
            .reservations
            .iter()
            .position(|reservation| reservation.prepared.generation == prepared.generation)
        else {
            return Err(DockPayloadRecoveryCommitError::StalePreparedToken);
        };

        if let Err(error) = self.validate_commit(graph, owner_revision, prepared) {
            self.reservations.remove(reservation_index);
            return Err(error);
        }

        let disposition = match &prepared.evidence {
            DockPayloadRecoveryEvidence::Survived(survival)
                if primary_space_is_live && survival.is_rehomed_into(primary_space) =>
            {
                DockPayloadRecoveryDisposition::AlreadyRehomed
            }
            DockPayloadRecoveryEvidence::Survived(_) => {
                DockPayloadRecoveryDisposition::VisibleRecoveryEntry
            }
            DockPayloadRecoveryEvidence::Unresolved(_) => {
                DockPayloadRecoveryDisposition::Unresolved
            }
        };
        let receipt = DockPayloadRecoveryCommitReceipt {
            generation: prepared.generation,
            authority: prepared.authority,
            disposition,
        };
        self.reservations.remove(reservation_index);
        self.records.push(DockPayloadRecoveryRecord {
            receipt,
            base_owner_revision: prepared.owner_revision,
            reason: prepared.reason,
            payload_identity: prepared.payload_identity.clone(),
            evidence: prepared.evidence.clone(),
            focus: prepared.focus.clone(),
            presentation_origin: prepared.presentation_origin.clone(),
            entry_focus: None,
        });
        Ok(receipt)
    }

    pub(crate) fn record(
        &self,
        receipt: DockPayloadRecoveryCommitReceipt,
    ) -> Option<&DockPayloadRecoveryRecord> {
        self.records.iter().find(|record| record.receipt == receipt)
    }

    pub(crate) fn committed_receipt(
        &self,
        authority: DockPayloadRecoveryAuthority,
        reason: DockPayloadRecoveryReason,
    ) -> Option<DockPayloadRecoveryCommitReceipt> {
        self.records
            .iter()
            .find(|record| record.receipt.authority == authority && record.reason == reason)
            .map(DockPayloadRecoveryRecord::receipt)
    }

    pub(crate) fn visible_records(&self) -> impl Iterator<Item = &DockPayloadRecoveryRecord> {
        self.records.iter().filter(|record| {
            record.receipt.disposition != DockPayloadRecoveryDisposition::AlreadyRehomed
        })
    }

    pub(crate) fn restore_action(
        &self,
        recovery: DockPayloadRecoveryCommitReceipt,
        anchor_lease: DockSurfaceWindowSessionLease,
    ) -> Option<DockPayloadRecoveryRestoreAction> {
        let record = self.record(recovery)?;
        (record.reason == DockPayloadRecoveryReason::LostViewportRecovery
            && record.disposition() == DockPayloadRecoveryDisposition::VisibleRecoveryEntry
            && recovery.live_identity().opening().lease() == anchor_lease)
            .then_some(DockPayloadRecoveryRestoreAction {
                recovery,
                anchor_lease,
            })
    }

    pub(crate) fn bind_visible_entry_focus(
        &mut self,
        recovery: DockPayloadRecoveryCommitReceipt,
        focus_handle: FocusHandle,
    ) -> bool {
        let Some(record) = self.records.iter_mut().find(|record| {
            record.receipt == recovery
                && record.reason == DockPayloadRecoveryReason::LostViewportRecovery
                && record.disposition() == DockPayloadRecoveryDisposition::VisibleRecoveryEntry
        }) else {
            return false;
        };
        if record.entry_focus.is_some() {
            return false;
        }
        record.entry_focus = Some(DockPayloadRecoveryEntryFocus {
            handle: focus_handle,
            pending: record
                .focus
                .as_ref()
                .is_some_and(DockPayloadRecoveryFocus::requests_entry_focus),
        });
        true
    }

    pub(crate) fn visible_entries(
        &self,
        anchor_lease: DockSurfaceWindowSessionLease,
    ) -> Vec<DockPayloadRecoveryEntry> {
        self.visible_records()
            .filter_map(|record| {
                let entry_focus = record.entry_focus.as_ref()?;
                let action = self.restore_action(record.receipt, anchor_lease)?;
                Some(DockPayloadRecoveryEntry {
                    action,
                    items: payload_items(&record.payload_identity),
                    focus_handle: entry_focus.handle.clone(),
                    focus_pending: entry_focus.pending,
                })
            })
            .collect()
    }

    pub(crate) fn settle_entry_focus(&mut self, action: DockPayloadRecoveryRestoreAction) -> bool {
        let Some(record) = self
            .records
            .iter_mut()
            .find(|record| record.receipt == action.recovery)
        else {
            return false;
        };
        let Some(entry_focus) = record.entry_focus.as_mut() else {
            return false;
        };
        if !entry_focus.pending {
            return false;
        }
        entry_focus.pending = false;
        true
    }

    pub(crate) fn prepare_restore(
        &self,
        graph: &DockGraph,
        owner_revision: u64,
        primary_space: &DockSpaceId,
        active_anchor: Option<DockSurfaceWindowSessionLease>,
        action: DockPayloadRecoveryRestoreAction,
    ) -> Result<DockPayloadRecoveryRestorePrepared, DockPayloadRecoveryRestoreError> {
        let record = self.validate_restore_action(active_anchor, action)?;
        if matches!(record.evidence, DockPayloadRecoveryEvidence::Unresolved(_)) {
            return Err(DockPayloadRecoveryRestoreError::PayloadUnresolved);
        }
        let source_location =
            DockPayloadRecoveryLocation::resolve_unique(graph, &record.payload_identity)?;
        let target_tabs = graph.recovery_target_tabs(primary_space);
        let preferred_focus_item = record
            .focus
            .as_ref()
            .and_then(DockPayloadRecoveryFocus::item);
        let (projected_graph, focus_item) = project_restore_graph(
            graph,
            primary_space,
            target_tabs,
            &record.payload_identity,
            preferred_focus_item,
        )?;
        Ok(DockPayloadRecoveryRestorePrepared {
            action,
            owner_revision,
            primary_space: primary_space.clone(),
            source_location,
            target_tabs,
            projected_graph,
            items: payload_items(&record.payload_identity),
            focus_item,
            descendant_focus: record
                .focus
                .as_ref()
                .and_then(DockPayloadRecoveryFocus::descendant)
                .cloned(),
            presentation_origin: record.presentation_origin.clone(),
        })
    }

    pub(crate) fn can_commit_restore(
        &self,
        graph: &DockGraph,
        owner_revision: u64,
        active_anchor: Option<DockSurfaceWindowSessionLease>,
        prepared: &DockPayloadRecoveryRestorePrepared,
    ) -> bool {
        if self.committed_restore_receipt(prepared.action).is_some() {
            return true;
        }
        if owner_revision != prepared.owner_revision
            || self
                .validate_restore_action(active_anchor, prepared.action)
                .is_err()
        {
            return false;
        }
        let Some(record) = self.record(prepared.action.recovery) else {
            return false;
        };
        graph.matches_exactly(&prepared.projected_graph)
            || (graph.recovery_target_tabs(&prepared.primary_space) == prepared.target_tabs
                && DockPayloadRecoveryLocation::resolve_unique(graph, &record.payload_identity)
                    .as_ref()
                    == Ok(&prepared.source_location))
    }

    pub(crate) fn commit_prepared_restore(
        &mut self,
        prepared: DockPayloadRecoveryRestorePrepared,
    ) -> DockPayloadRecoveryRestoreReceipt {
        if let Some(receipt) = self.committed_restore_receipt(prepared.action) {
            return receipt.clone();
        }
        let action = prepared.action;
        let focus_item = prepared.focus_item.clone();
        let descendant_focus = prepared.descendant_focus.clone();
        let index = self
            .records
            .iter()
            .position(|record| record.receipt == prepared.action.recovery)
            .expect("preflighted payload recovery restore must retain its exact record");
        self.records.remove(index);
        let receipt = DockPayloadRecoveryRestoreReceipt {
            recovery: prepared.action.recovery,
            focus_item,
            descendant_focus,
        };
        self.committed_restores
            .push(DockPayloadRecoveryCommittedRestore {
                action,
                receipt: receipt.clone(),
            });
        receipt
    }

    pub(crate) fn committed_restore_receipt(
        &self,
        action: DockPayloadRecoveryRestoreAction,
    ) -> Option<&DockPayloadRecoveryRestoreReceipt> {
        self.committed_restores
            .iter()
            .find(|committed| committed.action == action)
            .map(|committed| &committed.receipt)
    }

    pub(crate) fn retire_committed_restore(
        &mut self,
        action: DockPayloadRecoveryRestoreAction,
        receipt: &DockPayloadRecoveryRestoreReceipt,
    ) -> bool {
        let Some(index) = self
            .committed_restores
            .iter()
            .position(|committed| committed.action == action && committed.receipt == *receipt)
        else {
            return false;
        };
        self.committed_restores.remove(index);
        true
    }

    #[cfg(test)]
    pub(crate) fn committed_restore_count_for_test(&self) -> usize {
        self.committed_restores.len()
    }

    fn validate_restore_action(
        &self,
        active_anchor: Option<DockSurfaceWindowSessionLease>,
        action: DockPayloadRecoveryRestoreAction,
    ) -> Result<&DockPayloadRecoveryRecord, DockPayloadRecoveryRestoreError> {
        let record = self
            .record(action.recovery)
            .ok_or(DockPayloadRecoveryRestoreError::StaleAction)?;
        if record.reason != DockPayloadRecoveryReason::LostViewportRecovery
            || record.disposition() != DockPayloadRecoveryDisposition::VisibleRecoveryEntry
        {
            return Err(DockPayloadRecoveryRestoreError::StaleAction);
        }
        if active_anchor != Some(action.anchor_lease)
            || action.recovery.live_identity().opening().lease() != action.anchor_lease
        {
            return Err(DockPayloadRecoveryRestoreError::AnchorUnavailable);
        }
        Ok(record)
    }

    fn validate_commit(
        &self,
        graph: &DockGraph,
        owner_revision: u64,
        prepared: &DockPayloadRecoveryPrepared,
    ) -> Result<(), DockPayloadRecoveryCommitError> {
        let reservation = self
            .reservations
            .iter()
            .find(|reservation| reservation.prepared.generation == prepared.generation)
            .ok_or(DockPayloadRecoveryCommitError::StalePreparedToken)?;
        if reservation.prepared != *prepared {
            return Err(DockPayloadRecoveryCommitError::StalePreparedToken);
        }
        if owner_revision != prepared.owner_revision {
            return Err(DockPayloadRecoveryCommitError::OwnerRevisionChanged);
        }
        let current = DockPayloadSurvivalProof::resolve(graph, &prepared.payload_identity);
        let evidence_matches = match &prepared.evidence {
            DockPayloadRecoveryEvidence::Survived(survival) => current.as_ref() == Ok(survival),
            DockPayloadRecoveryEvidence::Unresolved(unresolved) => {
                current.as_ref().err().copied() == Some(*unresolved)
            }
        };
        if !evidence_matches {
            return Err(DockPayloadRecoveryCommitError::PayloadSurvivalChanged);
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum DockPayloadSurvivalProof {
    OriginalLocation,
    Rehomed(DockPayloadRecoveryLocation),
}

impl DockPayloadSurvivalProof {
    fn resolve(
        graph: &DockGraph,
        payload: &DockLockedPayloadIdentity,
    ) -> Result<Self, DockPayloadRecoveryPrepareError> {
        if payload.validate(graph).is_ok() {
            return Ok(Self::OriginalLocation);
        }

        let candidates = DockPayloadRecoveryLocation::find_all(graph, payload);
        match candidates.as_slice() {
            [] => Err(DockPayloadRecoveryPrepareError::PayloadMissing),
            [location] => Ok(Self::Rehomed(location.clone())),
            _ => Err(DockPayloadRecoveryPrepareError::PayloadAmbiguous),
        }
    }

    fn is_rehomed_into(&self, space: &DockSpaceId) -> bool {
        matches!(self, Self::Rehomed(location) if location.space() == space)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum DockPayloadRecoveryLocation {
    Item {
        space: DockSpaceId,
        tabs: DockNodeId,
        index: usize,
    },
    Tabs {
        space: DockSpaceId,
        tabs: DockNodeId,
        start: usize,
    },
    Floating {
        space: DockSpaceId,
        forest_root: DockNodeId,
        start: usize,
    },
}

impl DockPayloadRecoveryLocation {
    fn resolve_unique(
        graph: &DockGraph,
        payload: &DockLockedPayloadIdentity,
    ) -> Result<Self, DockPayloadRecoveryRestoreError> {
        match Self::find_all(graph, payload).as_slice() {
            [] => Err(DockPayloadRecoveryRestoreError::PayloadMissing),
            [location] => Ok(location.clone()),
            _ => Err(DockPayloadRecoveryRestoreError::PayloadAmbiguous),
        }
    }

    fn find_all(graph: &DockGraph, payload: &DockLockedPayloadIdentity) -> Vec<Self> {
        match payload {
            DockLockedPayloadIdentity::Item { item, .. } => Self::find_item(graph, item),
            DockLockedPayloadIdentity::Tabs { ordered_items, .. } => {
                Self::find_ordered_tabs(graph, ordered_items)
            }
            DockLockedPayloadIdentity::Floating { ordered_items, .. } => {
                Self::find_floating(graph, ordered_items)
            }
        }
    }

    fn find_item(graph: &DockGraph, item: &DockItemId) -> Vec<Self> {
        let mut candidates = Vec::new();
        for space in graph.spaces() {
            for tabs in graph.tabs_in_space(&space) {
                let Some(DockNode::Tabs { items, .. }) = graph.node(tabs) else {
                    continue;
                };
                for (index, candidate) in items.iter().enumerate() {
                    if candidate == item {
                        candidates.push(Self::Item {
                            space: space.clone(),
                            tabs,
                            index,
                        });
                    }
                }
            }
        }
        candidates
    }

    fn find_ordered_tabs(graph: &DockGraph, ordered_items: &[DockItemId]) -> Vec<Self> {
        if ordered_items.is_empty() {
            return Vec::new();
        }

        let mut candidates = Vec::new();
        for space in graph.spaces() {
            for tabs in graph.tabs_in_space(&space) {
                let Some(DockNode::Tabs { items, .. }) = graph.node(tabs) else {
                    continue;
                };
                for (start, window) in items.windows(ordered_items.len()).enumerate() {
                    if window == ordered_items {
                        candidates.push(Self::Tabs {
                            space: space.clone(),
                            tabs,
                            start,
                        });
                    }
                }
            }
        }
        candidates
    }

    fn find_floating(graph: &DockGraph, ordered_items: &[DockItemId]) -> Vec<Self> {
        if ordered_items.is_empty() {
            return Vec::new();
        }

        let mut candidates = Vec::new();
        for space in graph.spaces() {
            if let Some(root) = graph.root(&space) {
                Self::push_floating_item_ranges(
                    graph,
                    &space,
                    root,
                    ordered_items,
                    &mut candidates,
                );
            }
            for container in graph.floating_containers(&space) {
                Self::push_floating_item_ranges(
                    graph,
                    &space,
                    container.node,
                    ordered_items,
                    &mut candidates,
                );
            }
        }
        candidates
    }

    fn push_floating_item_ranges(
        graph: &DockGraph,
        space: &DockSpaceId,
        forest_root: DockNodeId,
        ordered_items: &[DockItemId],
        candidates: &mut Vec<Self>,
    ) {
        let items = graph.collect_items_in_subtree(forest_root);
        for (start, window) in items.windows(ordered_items.len()).enumerate() {
            if window == ordered_items {
                candidates.push(Self::Floating {
                    space: space.clone(),
                    forest_root,
                    start,
                });
            }
        }
    }

    fn space(&self) -> &DockSpaceId {
        match self {
            Self::Item { space, .. } | Self::Tabs { space, .. } | Self::Floating { space, .. } => {
                space
            }
        }
    }
}

fn project_restore_graph(
    graph: &DockGraph,
    primary_space: &DockSpaceId,
    initial_target_tabs: Option<DockNodeId>,
    payload: &DockLockedPayloadIdentity,
    preferred_focus_item: Option<&DockItemId>,
) -> Result<(DockGraph, Option<DockItemId>), DockPayloadRecoveryRestoreError> {
    let items = payload_items(payload);
    let focus_item = preferred_focus_item
        .filter(|preferred| items.contains(preferred))
        .cloned()
        .or_else(|| items.first().cloned());
    let mut projected = graph.clone();
    let mut target_tabs = initial_target_tabs;

    for item in &items {
        if target_tabs.is_some_and(|tabs| {
            projected
                .find_item_in_space(primary_space, item)
                .map(|(current, _)| current)
                == Some(tabs)
        }) {
            continue;
        }
        let source_space = unique_item_space(&projected, item)?;
        let target = match target_tabs {
            Some(tabs) => {
                let insert_index = match projected.node(tabs) {
                    Some(DockNode::Tabs { items, .. }) => items.len(),
                    _ => {
                        return Err(DockPayloadRecoveryRestoreError::GraphMutation(
                            DockGraphMutationError::NodeIsNotTabs { node: tabs },
                        ));
                    }
                };
                DockGraphDropTarget::tab_bar(tabs, insert_index)
            }
            None => DockGraphDropTarget::empty_space(),
        };
        projected
            .apply_op_checked(&DockOp::MoveItem {
                source_space,
                item: item.clone(),
                target_space: primary_space.clone(),
                target,
            })
            .map_err(DockPayloadRecoveryRestoreError::GraphMutation)?;
        if target_tabs.is_none() {
            target_tabs = projected
                .find_item_in_space(primary_space, item)
                .map(|(tabs, _)| tabs);
        }
    }

    if let (Some(tabs), Some(item)) = (target_tabs, focus_item.as_ref())
        && projected
            .find_item_in_space(primary_space, item)
            .map(|(current, _)| current)
            == Some(tabs)
    {
        projected
            .apply_op_checked(&DockOp::SelectTab {
                tabs,
                item: item.clone(),
            })
            .map_err(DockPayloadRecoveryRestoreError::GraphMutation)?;
    }
    Ok((projected, focus_item))
}

fn payload_items(payload: &DockLockedPayloadIdentity) -> Vec<DockItemId> {
    match payload {
        DockLockedPayloadIdentity::Item { item, .. } => vec![item.clone()],
        DockLockedPayloadIdentity::Tabs { ordered_items, .. }
        | DockLockedPayloadIdentity::Floating { ordered_items, .. } => ordered_items.clone(),
    }
}

fn unique_item_space(
    graph: &DockGraph,
    item: &DockItemId,
) -> Result<DockSpaceId, DockPayloadRecoveryRestoreError> {
    let spaces: Vec<_> = graph
        .spaces()
        .into_iter()
        .filter(|space| graph.find_item_in_space(space, item).is_some())
        .collect();
    match spaces.as_slice() {
        [] => Err(DockPayloadRecoveryRestoreError::PayloadMissing),
        [space] => Ok((*space).clone()),
        _ => Err(DockPayloadRecoveryRestoreError::PayloadAmbiguous),
    }
}
