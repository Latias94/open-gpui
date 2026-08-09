//! Generation-bound presentation-window authority for opt-in view roots.
//!
//! Ordinary entities may be shared across windows and continue to use GPUI's last-rendered-window
//! fallback. This module is intentionally opt-in: it fences only view roots whose live
//! presentation must move between windows without ever rendering in both at once.

use crate::{
    AnyElement, AnyView, App, AppContext, Bounds, Element, ElementId, Empty, Entity, EntityId,
    GlobalElementId, InspectorElementId, IntoElement, LayoutId, Pixels, PrepaintPublicationId,
    Window, WindowId,
};
use open_gpui_collections::{FxHashMap, FxHashSet};
use parking_lot::Mutex;
use std::{fmt, sync::Arc};

/// Exact authority to present one view root in one window generation.
#[doc(hidden)]
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct Lease {
    entity_id: EntityId,
    generation: u64,
    window_id: WindowId,
}

impl Lease {
    /// Returns the governed view-root entity.
    pub const fn entity_id(self) -> EntityId {
        self.entity_id
    }

    /// Returns the exact presentation generation.
    pub const fn generation(self) -> u64 {
        self.generation
    }

    /// Returns the only window admitted by this lease.
    pub const fn window_id(self) -> WindowId {
        self.window_id
    }
}

/// Exact leases for one atomic presentation location.
#[doc(hidden)]
#[derive(Clone, Debug)]
pub struct LeaseBatch {
    window_id: WindowId,
    leases: Arc<[Lease]>,
}

impl LeaseBatch {
    fn new(window_id: WindowId, leases: Vec<Lease>) -> Self {
        Self {
            window_id,
            leases: leases.into(),
        }
    }

    /// Returns the common presentation window.
    pub const fn window_id(&self) -> WindowId {
        self.window_id
    }

    /// Returns the exact per-root leases.
    pub fn leases(&self) -> &[Lease] {
        &self.leases
    }

    /// Returns the lease for one root entity.
    pub fn lease_for(&self, entity_id: EntityId) -> Option<Lease> {
        self.leases
            .iter()
            .copied()
            .find(|lease| lease.entity_id == entity_id)
    }

    fn common_generation(&self) -> Option<u64> {
        let generation = self.leases.first()?.generation;
        self.leases
            .iter()
            .all(|lease| lease.generation == generation)
            .then_some(generation)
    }

    /// Returns whether both batches represent the same immutable presentation authority.
    #[doc(hidden)]
    pub fn matches_exactly(&self, other: &Self) -> bool {
        self.window_id == other.window_id && self.leases == other.leases
    }
}

/// Stable phase of one prepared presentation rehost.
#[doc(hidden)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RehostPhase {
    /// The destination is reserved while the source remains authoritative.
    AwaitingSourceRelease,
    /// The source proxy frame committed and destination leases are authoritative.
    DestinationAdmitted,
    /// Every destination root mounted in one accepted staging frame and awaits `finish` exposure.
    DestinationMounted,
    /// The destination is visibly presenting while the prepared rehost remains reversible.
    DestinationExposed,
    /// A pre-mount cancellation minted fresh source leases and awaits their frame.
    RestoringSource,
    /// Every restored source root mounted in one accepted staging frame and awaits `finish` exposure.
    SourceRestored,
    /// The rehost was cancelled before source release.
    Cancelled,
    /// Exact-generation validation failed or a required owner became terminal.
    Invalidated,
}

/// Typed reason why a prepared rehost failed or entered source recovery.
#[doc(hidden)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Invalidation {
    /// A lease no longer matched the current entity binding.
    StaleLease,
    /// A governed source root was still mounted in the source-release frame.
    SourceStillMounted,
    /// Destination roots did not mount atomically in one accepted frame.
    DestinationFrameMismatch,
    /// Restored source roots did not mount atomically in one accepted frame.
    SourceRestoreFrameMismatch,
    /// The source window closed before authority could safely leave it.
    SourceWindowClosed,
    /// The destination window closed before the handoff completed.
    DestinationWindowClosed,
    /// One governed root entity was released.
    EntityReleased,
}

/// Source-side cleanup disposition for one invalidated prepared rehost.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SourceInvalidationDisposition {
    /// The original source batch remains authoritative and can be finished unchanged.
    SourceAuthorityUnchanged,
    /// Logical closure was observed; cleanup must await the exact native source terminal.
    AwaitingSourceNativeTerminal,
    /// GPUI released ordinary source-restoration authority and topology recovery must take over.
    PresentationAuthorityReleased,
}

/// Exact evidence that a source-proxy replay succeeded in one candidate handoff frame.
#[doc(hidden)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SourceProxyReplayReceipt {
    rehost_generation: u64,
    source_window: WindowId,
    frame_generation: u64,
    evidence: SourceProxyEvidence,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SourceProxyEvidence {
    FrameworkPainted,
    RetainedVisual(crate::window::retained_visual::ReplayReceipt),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SourceProxyEvidenceRequirement {
    FrameworkPainted,
    RetainedVisual(crate::window::retained_visual::TicketIdentity),
}

impl SourceProxyEvidenceRequirement {
    fn accepts(self, evidence: SourceProxyEvidence) -> bool {
        match (self, evidence) {
            (Self::FrameworkPainted, SourceProxyEvidence::FrameworkPainted) => true,
            (Self::RetainedVisual(expected), SourceProxyEvidence::RetainedVisual(replay)) => {
                expected == replay.ticket()
            }
            _ => false,
        }
    }
}

impl SourceProxyReplayReceipt {
    /// Returns the prepared rehost generation that produced this receipt.
    pub const fn rehost_generation(self) -> u64 {
        self.rehost_generation
    }

    /// Returns the source window whose proxy was accepted.
    pub const fn source_window(self) -> WindowId {
        self.source_window
    }

    /// Returns the committed source frame containing the proxy replay.
    pub const fn frame_generation(self) -> u64 {
        self.frame_generation
    }

    /// Returns exact retained-visual evidence when that mechanism produced the proxy.
    pub const fn retained_visual_replay(
        self,
    ) -> Option<crate::window::retained_visual::ReplayReceipt> {
        match self.evidence {
            SourceProxyEvidence::RetainedVisual(receipt) => Some(receipt),
            SourceProxyEvidence::FrameworkPainted => None,
        }
    }
}

/// Exact accepted-frame receipt that switched presentation authority away from the source.
#[doc(hidden)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SourceProxyCommitReceipt {
    replay: SourceProxyReplayReceipt,
}

impl SourceProxyCommitReceipt {
    /// Returns the candidate replay consumed by the accepted-frame barrier.
    pub const fn replay(self) -> SourceProxyReplayReceipt {
        self.replay
    }

    /// Returns the prepared rehost generation committed by the barrier.
    pub const fn rehost_generation(self) -> u64 {
        self.replay.rehost_generation
    }

    /// Returns the source window whose authority was released.
    pub const fn source_window(self) -> WindowId {
        self.replay.source_window
    }

    /// Returns the accepted source-proxy frame generation.
    pub const fn frame_generation(self) -> u64 {
        self.replay.frame_generation
    }

    /// Returns retained-visual evidence when the exact barrier required it.
    pub const fn retained_visual_replay(
        self,
    ) -> Option<crate::window::retained_visual::ReplayReceipt> {
        self.replay.retained_visual_replay()
    }
}

/// Exact evidence that every destination root mounted in one accepted staging frame.
#[doc(hidden)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DestinationMountReceipt {
    source_proxy: SourceProxyCommitReceipt,
    rehost_generation: u64,
    destination_window: WindowId,
    destination_lease_generation: u64,
    root_count: usize,
    frame_generation: u64,
}

impl DestinationMountReceipt {
    /// Returns the accepted source-proxy receipt that admitted destination authority.
    pub const fn source_proxy(self) -> SourceProxyCommitReceipt {
        self.source_proxy
    }

    /// Returns the prepared rehost generation that admitted the destination batch.
    pub const fn rehost_generation(self) -> u64 {
        self.rehost_generation
    }

    /// Returns the destination window that mounted every root.
    pub const fn destination_window(self) -> WindowId {
        self.destination_window
    }

    /// Returns the exact common lease generation of the destination roots.
    pub const fn destination_lease_generation(self) -> u64 {
        self.destination_lease_generation
    }

    /// Returns the number of roots proven by this receipt.
    pub const fn root_count(self) -> usize {
        self.root_count
    }

    /// Returns the accepted staging frame in which every root mounted.
    pub const fn frame_generation(self) -> u64 {
        self.frame_generation
    }
}

/// Exact evidence that `finish` exposed one fully mounted destination batch.
#[doc(hidden)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DestinationExposureReceipt {
    mount: DestinationMountReceipt,
}

impl DestinationExposureReceipt {
    /// Returns the staging mount receipt consumed by the exposure transition.
    pub const fn mount(self) -> DestinationMountReceipt {
        self.mount
    }
}

/// Exact evidence that one authoritative lease batch mounted in one visible candidate frame.
#[doc(hidden)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PresentedBatchReceipt {
    exposure: DestinationExposureReceipt,
    frame_generation: u64,
}

impl PresentedBatchReceipt {
    /// Returns the window presenting the exact lease batch.
    pub const fn window_id(self) -> WindowId {
        self.exposure.mount.destination_window
    }

    /// Returns the exact common lease generation of the presented roots.
    pub const fn lease_generation(self) -> u64 {
        self.exposure.mount.destination_lease_generation
    }

    /// Returns the number of roots proven by this receipt.
    pub const fn root_count(self) -> usize {
        self.exposure.mount.root_count
    }

    /// Returns the accepted candidate frame in which every root mounted.
    pub const fn frame_generation(self) -> u64 {
        self.frame_generation
    }

    /// Returns the exact destination exposure proven by this presented frame.
    pub const fn exposure(self) -> DestinationExposureReceipt {
        self.exposure
    }
}

/// Exact evidence that one stable lease batch mounted in one visible accepted frame.
#[doc(hidden)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StableBatchPresentationReceipt {
    window_id: WindowId,
    lease_generation: u64,
    root_count: usize,
    frame_generation: u64,
}

impl StableBatchPresentationReceipt {
    /// Returns the authoritative presentation window.
    pub const fn window_id(self) -> WindowId {
        self.window_id
    }

    /// Returns the exact common generation of the presented leases.
    pub const fn lease_generation(self) -> u64 {
        self.lease_generation
    }

    /// Returns the number of roots proven by the receipt.
    pub const fn root_count(self) -> usize {
        self.root_count
    }

    /// Returns the accepted frame in which every root visibly mounted.
    pub const fn frame_generation(self) -> u64 {
        self.frame_generation
    }
}

/// Candidate-local handle used to report one successful source-proxy replay.
#[doc(hidden)]
#[derive(Debug)]
pub struct SourceProxyReplayAttempt {
    rehost_generation: u64,
    source_window: WindowId,
    expected_evidence: SourceProxyEvidenceRequirement,
    receipt: Arc<Mutex<Option<SourceProxyReplayReceipt>>>,
}

/// Read-only state of one prepared presentation rehost.
#[doc(hidden)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RehostSnapshot {
    phase: RehostPhase,
    source_proxy_receipt: Option<SourceProxyCommitReceipt>,
    destination_frame_generation: Option<u64>,
    destination_mount_receipt: Option<DestinationMountReceipt>,
    invalidation: Option<Invalidation>,
}

impl RehostSnapshot {
    /// Returns the current rehost phase.
    pub const fn phase(self) -> RehostPhase {
        self.phase
    }

    /// Returns the accepted source-proxy frame that switched authority.
    pub const fn source_frame_generation(self) -> Option<u64> {
        match self.source_proxy_receipt {
            Some(receipt) => Some(receipt.frame_generation()),
            None => None,
        }
    }

    /// Returns the exact accepted source-proxy replay receipt.
    pub const fn source_proxy_receipt(self) -> Option<SourceProxyCommitReceipt> {
        self.source_proxy_receipt
    }

    /// Returns the accepted destination or restored-source mount frame.
    pub const fn destination_frame_generation(self) -> Option<u64> {
        self.destination_frame_generation
    }

    /// Returns exact evidence that every reserved destination root mounted atomically.
    pub const fn destination_mount_receipt(self) -> Option<DestinationMountReceipt> {
        self.destination_mount_receipt
    }

    /// Returns the failure or recovery reason, when present.
    pub const fn invalidation(self) -> Option<Invalidation> {
        self.invalidation
    }

    /// Returns the source-side cleanup disposition for this exact invalidated snapshot.
    const fn source_invalidation_disposition(self) -> Option<SourceInvalidationDisposition> {
        if !matches!(self.phase, RehostPhase::Invalidated) {
            return None;
        }
        match self.invalidation {
            Some(Invalidation::SourceWindowClosed) => {
                Some(SourceInvalidationDisposition::AwaitingSourceNativeTerminal)
            }
            Some(Invalidation::SourceStillMounted | Invalidation::DestinationWindowClosed)
                if self.source_proxy_receipt.is_none() =>
            {
                Some(SourceInvalidationDisposition::SourceAuthorityUnchanged)
            }
            Some(_) => Some(SourceInvalidationDisposition::PresentationAuthorityReleased),
            None => None,
        }
    }
}

/// Prepared atomic move of one or more presentation roots between windows.
#[doc(hidden)]
#[derive(Clone)]
pub struct PreparedRehost {
    generation: u64,
    source: LeaseBatch,
    destination: LeaseBatch,
    restored_source: Arc<Mutex<Option<LeaseBatch>>>,
    snapshot: Arc<Mutex<RehostSnapshot>>,
    terminal: Arc<Mutex<Option<RehostTerminalTombstone>>>,
}

impl fmt::Debug for PreparedRehost {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PreparedRehost")
            .field("generation", &self.generation)
            .field("source", &self.source)
            .field("destination", &self.destination)
            .field("restored_source", &self.restored_source())
            .field("snapshot", &self.snapshot())
            .finish()
    }
}

impl PreparedRehost {
    /// Returns the exact rehost generation.
    pub const fn generation(&self) -> u64 {
        self.generation
    }

    /// Returns the still-authoritative source leases captured at prepare time.
    pub const fn source(&self) -> &LeaseBatch {
        &self.source
    }

    /// Returns the reserved destination leases.
    pub const fn destination(&self) -> &LeaseBatch {
        &self.destination
    }

    /// Returns fresh source leases minted after destination authority was revoked.
    pub fn restored_source(&self) -> Option<LeaseBatch> {
        self.restored_source.lock().clone()
    }

    /// Returns the latest lock-free-from-App state snapshot.
    pub fn snapshot(&self) -> RehostSnapshot {
        *self.snapshot.lock()
    }

    /// Returns whether both handles refer to the same prepared rehost generation and state cells.
    #[doc(hidden)]
    pub fn matches_exactly(&self, other: &Self) -> bool {
        self.generation == other.generation
            && self.source.matches_exactly(&other.source)
            && self.destination.matches_exactly(&other.destination)
            && Arc::ptr_eq(&self.restored_source, &other.restored_source)
            && Arc::ptr_eq(&self.snapshot, &other.snapshot)
            && Arc::ptr_eq(&self.terminal, &other.terminal)
    }

    /// Returns source-proxy evidence only while destination presentation remains active.
    #[doc(hidden)]
    pub fn committed_source_proxy(&self) -> Option<SourceProxyCommitReceipt> {
        let snapshot = self.snapshot();
        matches!(
            snapshot.phase(),
            RehostPhase::DestinationAdmitted
                | RehostPhase::DestinationMounted
                | RehostPhase::DestinationExposed
        )
        .then(|| snapshot.source_proxy_receipt())
        .flatten()
    }

    /// Returns destination mount evidence only before the destination has been exposed.
    #[doc(hidden)]
    pub fn destination_ready_for_exposure(&self) -> Option<DestinationMountReceipt> {
        let snapshot = self.snapshot();
        (snapshot.phase() == RehostPhase::DestinationMounted)
            .then(|| snapshot.destination_mount_receipt())
            .flatten()
    }

    /// Returns fresh source leases while source restoration still owns the rehost session.
    #[doc(hidden)]
    pub fn active_source_restoration(&self) -> Option<LeaseBatch> {
        matches!(
            self.snapshot().phase(),
            RehostPhase::RestoringSource | RehostPhase::SourceRestored
        )
        .then(|| self.restored_source())
        .flatten()
    }

    /// Returns fresh source leases after their exact accepted staging frame.
    #[doc(hidden)]
    pub fn accepted_source_restoration(&self) -> Option<LeaseBatch> {
        (self.snapshot().phase() == RehostPhase::SourceRestored)
            .then(|| self.restored_source())
            .flatten()
    }

    /// Returns whether ordinary source settlement has started or reached a provider terminal.
    #[doc(hidden)]
    pub fn source_settlement_started(&self) -> bool {
        matches!(
            self.snapshot().phase(),
            RehostPhase::RestoringSource
                | RehostPhase::SourceRestored
                | RehostPhase::Cancelled
                | RehostPhase::Invalidated
        )
    }

    /// Returns whether the original source batch remained authoritative through settlement.
    #[doc(hidden)]
    pub fn source_authority_remained_unchanged(&self) -> bool {
        let snapshot = self.snapshot();
        snapshot.phase() == RehostPhase::Cancelled
            || snapshot.source_invalidation_disposition()
                == Some(SourceInvalidationDisposition::SourceAuthorityUnchanged)
    }

    /// Returns the invalidation that released ordinary presentation authority.
    #[doc(hidden)]
    pub fn presentation_authority_loss(&self) -> Option<Invalidation> {
        let snapshot = self.snapshot();
        (snapshot.source_invalidation_disposition()
            == Some(SourceInvalidationDisposition::PresentationAuthorityReleased))
        .then(|| snapshot.invalidation())
        .flatten()
    }

    fn terminal_tombstone(&self) -> Option<RehostTerminalTombstone> {
        self.terminal.lock().clone()
    }

    fn record_terminal(&self, terminal: RehostTerminalTombstone) {
        let mut current = self.terminal.lock();
        assert!(
            current.is_none(),
            "one rehost generation cannot publish two terminal outcomes"
        );
        *current = Some(terminal);
    }
}

/// Cloneable render projection for one exact rehost generation.
///
/// A projection can render the source barrier and the currently selected lease batches, but it
/// cannot inspect or advance the provider phase machine. The owning [`RehostSession`] remains the
/// only transition authority.
#[doc(hidden)]
#[derive(Clone)]
pub struct RehostProjection {
    prepared: PreparedRehost,
}

impl fmt::Debug for RehostProjection {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RehostProjection")
            .field("generation", &self.generation())
            .field("source", self.source())
            .field("destination", self.destination())
            .finish()
    }
}

impl RehostProjection {
    fn new(prepared: PreparedRehost) -> Self {
        Self { prepared }
    }

    /// Returns the exact rehost generation.
    pub const fn generation(&self) -> u64 {
        self.prepared.generation()
    }

    /// Returns the source leases captured when this generation was prepared.
    pub const fn source(&self) -> &LeaseBatch {
        self.prepared.source()
    }

    /// Returns the reserved destination leases.
    pub const fn destination(&self) -> &LeaseBatch {
        self.prepared.destination()
    }

    /// Returns whether both projections identify the same exact provider generation.
    pub fn matches_exactly(&self, other: &Self) -> bool {
        self.prepared.matches_exactly(&other.prepared)
    }

    /// Returns whether this exact generation no longer owns any presentation binding.
    pub fn authority_is_retired(&self, cx: &App) -> bool {
        rehost_authority_is_absent(cx, &self.prepared)
    }

    /// Wraps a source proxy in the accepted-frame authority barrier for this exact generation.
    #[track_caller]
    pub fn source_release_barrier<E>(
        &self,
        child: impl FnOnce(SourceProxyReplayAttempt) -> E,
    ) -> SourceReleaseBarrier
    where
        E: IntoElement,
    {
        source_release_barrier(self.prepared.clone(), child)
    }

    /// Wraps a retained visual in the accepted-frame authority barrier for this exact generation.
    #[track_caller]
    pub fn retained_visual_source_release_barrier<E>(
        &self,
        ticket: &crate::window::retained_visual::Ticket,
        child: impl FnOnce(SourceProxyReplayAttempt) -> E,
    ) -> SourceReleaseBarrier
    where
        E: IntoElement,
    {
        retained_visual_source_release_barrier(self.prepared.clone(), ticket, child)
    }
}

/// Accepted source-proxy commit for one exact rehost generation.
#[doc(hidden)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RehostSourceProxyCommit {
    receipt: SourceProxyCommitReceipt,
}

impl RehostSourceProxyCommit {
    /// Returns the committed source frame.
    pub const fn frame_generation(self) -> u64 {
        self.receipt.frame_generation()
    }

    /// Returns retained-visual evidence when that mechanism produced the source proxy.
    pub const fn retained_visual_replay(
        self,
    ) -> Option<crate::window::retained_visual::ReplayReceipt> {
        self.receipt.retained_visual_replay()
    }
}

/// Reversible destination exposure produced from one exact accepted staging frame.
#[doc(hidden)]
#[derive(Clone, Debug)]
pub struct RehostDestinationExposure {
    batch: LeaseBatch,
    receipt: DestinationExposureReceipt,
}

impl RehostDestinationExposure {
    /// Returns the exact exposed destination batch.
    pub const fn batch(&self) -> &LeaseBatch {
        &self.batch
    }

    /// Returns the accepted staging frame that produced this exposure.
    pub const fn frame_generation(&self) -> u64 {
        self.receipt.mount().frame_generation()
    }
}

/// Exact visible frame for an exposed destination batch.
#[doc(hidden)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RehostDestinationPresentation {
    receipt: PresentedBatchReceipt,
}

impl RehostDestinationPresentation {
    /// Returns the visible destination frame.
    pub const fn frame_generation(self) -> u64 {
        self.receipt.frame_generation()
    }

    /// Returns the destination window.
    pub const fn window_id(self) -> WindowId {
        self.receipt.window_id()
    }

    /// Returns the exact destination lease generation.
    pub const fn lease_generation(self) -> u64 {
        self.receipt.lease_generation()
    }

    /// Returns the number of roots proven by this presentation.
    pub const fn root_count(self) -> usize {
        self.receipt.root_count()
    }
}

/// Requested terminal transition for a larger synchronous authority transaction.
#[doc(hidden)]
#[derive(Clone, Copy, Debug)]
pub enum RehostTerminalIntent<'a> {
    /// Retire one exact visible destination as the stable presentation authority.
    CommitDestination(&'a RehostDestinationPresentation),
    /// Release every still-owned binding after the source endpoint or topology was lost.
    AbandonAfterSourceLoss,
}

/// Evidence that source-loss abandonment terminally released one rehost generation.
#[doc(hidden)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RehostAbandonmentReceipt {
    generation: u64,
    source_window: WindowId,
    destination_window: WindowId,
}

impl RehostAbandonmentReceipt {
    fn from_prepared(prepared: &PreparedRehost) -> Self {
        Self {
            generation: prepared.generation(),
            source_window: prepared.source().window_id(),
            destination_window: prepared.destination().window_id(),
        }
    }

    /// Returns the retired rehost generation.
    pub const fn generation(self) -> u64 {
        self.generation
    }

    /// Returns the former source window.
    pub const fn source_window(self) -> WindowId {
        self.source_window
    }

    /// Returns the former destination window.
    pub const fn destination_window(self) -> WindowId {
        self.destination_window
    }
}

#[derive(Clone, Debug)]
enum RehostTerminalTombstone {
    DestinationCommitted {
        batch: LeaseBatch,
        exposure: DestinationExposureReceipt,
        accepted_frame: Option<u64>,
    },
    SourceCommitted {
        batch: LeaseBatch,
        accepted_frame: Option<u64>,
    },
    Abandoned(RehostAbandonmentReceipt),
    RetiredWithoutPresentationAuthority,
}

impl RehostTerminalTombstone {
    fn disposition(&self) -> RehostTerminalDisposition {
        match self {
            Self::DestinationCommitted { .. } => RehostTerminalDisposition::DestinationCommitted,
            Self::SourceCommitted { .. } => RehostTerminalDisposition::SourceCommitted,
            Self::Abandoned(_) => RehostTerminalDisposition::Abandoned,
            Self::RetiredWithoutPresentationAuthority => {
                RehostTerminalDisposition::PresentationAuthorityReleased
            }
        }
    }

    fn destination_outcome(
        &self,
        expected: &LeaseBatch,
        presentation: &RehostDestinationPresentation,
    ) -> Option<RehostTerminalOutcome> {
        let Self::DestinationCommitted {
            batch,
            exposure,
            accepted_frame: Some(accepted_frame),
        } = self
        else {
            return None;
        };
        (batch.matches_exactly(expected)
            && *exposure == presentation.receipt.exposure()
            && *accepted_frame == presentation.receipt.frame_generation())
        .then(|| RehostTerminalOutcome::DestinationCommitted(batch.clone()))
    }

    fn abandonment_receipt(&self) -> Option<RehostAbandonmentReceipt> {
        match self {
            Self::Abandoned(receipt) => Some(*receipt),
            Self::DestinationCommitted { .. }
            | Self::SourceCommitted { .. }
            | Self::RetiredWithoutPresentationAuthority => None,
        }
    }

    fn source_finish_matches(&self, source: &LeaseBatch) -> bool {
        matches!(
            self,
            Self::SourceCommitted { batch, .. } if batch.matches_exactly(source)
        )
    }

    fn source_frame_matches(&self, source: &LeaseBatch, accepted_frame: u64) -> bool {
        matches!(
            self,
            Self::SourceCommitted {
                batch,
                accepted_frame: Some(committed_frame),
            } if batch.matches_exactly(source) && *committed_frame == accepted_frame
        )
    }
}

/// Result of preparing one terminal rehost transition.
#[doc(hidden)]
#[derive(Debug)]
pub enum RehostTerminalPreparation {
    /// Every provider precondition was validated and is held by one single-use token.
    Prepared(PreparedRehostTerminal),
    /// An earlier attempt already committed this exact requested terminal outcome.
    AlreadyCommitted(RehostTerminalOutcome),
}

impl RehostTerminalPreparation {
    /// Returns whether the requested terminal outcome can be committed without further recovery.
    pub fn can_commit(&self, cx: &App) -> bool {
        match self {
            Self::Prepared(prepared) => prepared.can_commit(cx),
            Self::AlreadyCommitted(_) => true,
        }
    }

    /// Commits the prepared provider transition or replays its exact idempotent outcome.
    pub fn try_commit(self, cx: &mut App) -> Result<RehostTerminalOutcome, TransitionError> {
        match self {
            Self::Prepared(prepared) => prepared.try_commit(cx),
            Self::AlreadyCommitted(outcome) => Ok(outcome),
        }
    }

    /// Consumes one aggregate-preflighted provider authority without another fallible branch.
    ///
    /// Callers must invoke [`Self::can_commit`] as part of the same synchronous transaction before
    /// crossing any irreversible authority boundary.
    pub fn commit_prepared(
        self,
        cx: &mut App,
    ) -> (RehostTerminalOutcome, RehostTerminalPostCommit) {
        match self {
            Self::Prepared(prepared) => prepared.commit_prepared(cx),
            Self::AlreadyCommitted(outcome) => (outcome, RehostTerminalPostCommit::default()),
        }
    }
}

/// Single-use provider authority for the final step of a larger synchronous transaction.
#[doc(hidden)]
#[derive(Debug)]
#[must_use = "a prepared terminal must be committed or discarded before retrying"]
pub struct PreparedRehostTerminal {
    transition: PreparedRehostTerminalTransition,
}

#[derive(Debug)]
enum PreparedRehostTerminalTransition {
    CommitDestination(PreparedFinishDestination),
    AbandonAfterSourceLoss(PreparedAbandonRehostAfterSourceLoss),
}

impl PreparedRehostTerminal {
    /// Returns whether this exact token can still commit without provider validation failure.
    pub fn can_commit(&self, cx: &App) -> bool {
        match &self.transition {
            PreparedRehostTerminalTransition::CommitDestination(prepared) => {
                can_commit_prepared_finish_destination(cx, prepared)
            }
            PreparedRehostTerminalTransition::AbandonAfterSourceLoss(prepared) => {
                can_commit_prepared_abandon_rehost_after_source_loss(cx, prepared)
            }
        }
    }

    /// Consumes the validated provider transition if its exact receipt still matches.
    pub fn try_commit(self, cx: &mut App) -> Result<RehostTerminalOutcome, TransitionError> {
        if !self.can_commit(cx) {
            return Err(TransitionError::StalePrepared);
        }
        let (outcome, post_commit) = self.commit_prepared(cx);
        post_commit.publish(cx);
        Ok(outcome)
    }

    /// Consumes this exact provider token after an enclosing aggregate preflight succeeds.
    pub fn commit_prepared(
        self,
        cx: &mut App,
    ) -> (RehostTerminalOutcome, RehostTerminalPostCommit) {
        match self.transition {
            PreparedRehostTerminalTransition::CommitDestination(prepared) => {
                let outcome = cx
                    .view_presentation_windows
                    .commit_prepared_finish_destination(prepared);
                let FinishOutcome::Destination { batch, exposure: _ } = outcome else {
                    unreachable!("prepared destination finish must commit a destination outcome")
                };
                let post_commit = RehostTerminalPostCommit::one(batch.window_id());
                (
                    RehostTerminalOutcome::DestinationCommitted(batch),
                    post_commit,
                )
            }
            PreparedRehostTerminalTransition::AbandonAfterSourceLoss(prepared) => {
                let receipt = RehostAbandonmentReceipt::from_prepared(&prepared.prepared);
                let source_window = prepared.prepared.source.window_id;
                let destination_window = prepared.prepared.destination.window_id;
                let _ = cx
                    .view_presentation_windows
                    .commit_prepared_abandon_rehost_after_source_loss(
                        prepared,
                        &mut cx.current_window_by_entity,
                    );
                (
                    RehostTerminalOutcome::Abandoned(receipt),
                    RehostTerminalPostCommit::two(source_window, destination_window),
                )
            }
        }
    }
}

/// Deferred window refreshes produced by one aggregate-preflighted rehost terminal commit.
#[doc(hidden)]
#[derive(Clone, Debug, Default)]
#[must_use = "provider post-commit refreshes must run after the enclosing authority swap"]
pub struct RehostTerminalPostCommit {
    windows: [Option<WindowId>; 2],
}

impl RehostTerminalPostCommit {
    fn one(window: WindowId) -> Self {
        Self {
            windows: [Some(window), None],
        }
    }

    fn two(first: WindowId, second: WindowId) -> Self {
        Self {
            windows: [Some(first), (second != first).then_some(second)],
        }
    }

    /// Publishes renderer refreshes after the enclosing transaction installs durable authority.
    pub fn publish(&self, cx: &mut App) {
        for window in self.windows.iter().copied().flatten() {
            refresh_window(cx, window);
        }
    }
}

/// Terminal result of one prepared provider transition.
#[doc(hidden)]
#[derive(Clone, Debug)]
pub enum RehostTerminalOutcome {
    /// Destination presentation became stable and the provider session was retired.
    DestinationCommitted(LeaseBatch),
    /// Source-loss cleanup released the exact provider generation.
    Abandoned(RehostAbandonmentReceipt),
}

/// Atomic source-loss abandonment result outside a larger prepared transaction.
#[doc(hidden)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RehostAbandonmentOutcome {
    /// This call released the exact provider generation.
    Abandoned(RehostAbandonmentReceipt),
    /// An earlier idempotent attempt already abandoned this exact provider generation.
    AlreadyAbandoned(RehostAbandonmentReceipt),
}

/// Exact terminal disposition published by one rehost session.
#[doc(hidden)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RehostTerminalDisposition {
    /// Destination presentation became the stable authority.
    DestinationCommitted,
    /// Source presentation remained or became the stable authority.
    SourceCommitted,
    /// Source loss released every binding owned by the rehost generation.
    Abandoned,
    /// Provider authority was released without a surviving presentation endpoint.
    PresentationAuthorityReleased,
}

/// Unique transition owner for one prepared presentation rehost.
///
/// The session is deliberately not cloneable. Clone [`RehostProjection`] values for rendering or
/// asynchronous observation, while retaining this owner in the executor that must settle,
/// abandon, or atomically commit the provider generation.
#[doc(hidden)]
#[must_use = "a rehost session must be transferred to an executor or explicitly settled"]
pub struct RehostSession {
    prepared: PreparedRehost,
    _lifecycle: Entity<RehostSessionLifecycle>,
    accepted_destination_exposure: Option<(u64, RehostDestinationExposure)>,
    accepted_source_finish: Option<(u64, SourcePresentationFinish)>,
}

struct RehostSessionLifecycle {
    prepared: PreparedRehost,
}

impl fmt::Debug for RehostSession {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RehostSession")
            .field("projection", &self.projection())
            .finish()
    }
}

impl RehostSession {
    fn new(prepared: PreparedRehost, cx: &mut App) -> Self {
        let lifecycle = cx.new(|_| RehostSessionLifecycle {
            prepared: prepared.clone(),
        });
        cx.observe_release(&lifecycle, |lifecycle, cx| {
            cx.retire_released_rehost_session(&lifecycle.prepared);
        })
        .detach();
        Self {
            prepared,
            _lifecycle: lifecycle,
            accepted_destination_exposure: None,
            accepted_source_finish: None,
        }
    }

    /// Returns a cloneable render projection for this exact session.
    pub fn projection(&self) -> RehostProjection {
        RehostProjection::new(self.prepared.clone())
    }

    /// Returns the exact rehost generation.
    pub const fn generation(&self) -> u64 {
        self.prepared.generation()
    }

    /// Returns the source leases captured when this generation was prepared.
    pub const fn source(&self) -> &LeaseBatch {
        self.prepared.source()
    }

    /// Returns the reserved destination leases.
    pub const fn destination(&self) -> &LeaseBatch {
        self.prepared.destination()
    }

    /// Returns the exact provider terminal disposition, when one has been published.
    pub fn terminal_disposition(&self) -> Option<RehostTerminalDisposition> {
        self.prepared
            .terminal_tombstone()
            .as_ref()
            .map(RehostTerminalTombstone::disposition)
    }

    /// Returns whether this session has published one exact provider terminal outcome.
    pub fn is_terminal(&self) -> bool {
        self.terminal_disposition().is_some()
    }

    /// Returns the exact source finish already committed by this session, when present.
    pub fn committed_source_finish(&self) -> Option<SourcePresentationFinish> {
        if self.terminal_disposition() != Some(RehostTerminalDisposition::SourceCommitted) {
            return None;
        }
        self.accepted_source_finish
            .as_ref()
            .map(|(_, outcome)| outcome.clone())
    }

    /// Accepts the exact source-proxy frame committed by the provider barrier.
    pub fn accept_source_proxy_frame(
        &mut self,
        accepted_frame: u64,
    ) -> Result<RehostSourceProxyCommit, TransitionError> {
        if self.is_terminal() {
            return Err(TransitionError::ConflictingTerminalOutcome);
        }
        let receipt = self
            .prepared
            .committed_source_proxy()
            .ok_or(TransitionError::WrongState)?;
        if receipt.frame_generation() != accepted_frame {
            return Err(TransitionError::StaleCandidateFrameAttempt);
        }
        Ok(RehostSourceProxyCommit { receipt })
    }

    /// Exposes the destination only when every root mounted in the supplied accepted frame.
    pub fn accept_destination_frame(
        &mut self,
        cx: &mut App,
        accepted_frame: u64,
    ) -> Result<RehostDestinationExposure, TransitionError> {
        if self.is_terminal() {
            return Err(TransitionError::ConflictingTerminalOutcome);
        }
        if let Some((committed_frame, exposure)) = &self.accepted_destination_exposure {
            return (*committed_frame == accepted_frame)
                .then(|| exposure.clone())
                .ok_or(TransitionError::StaleCandidateFrameAttempt);
        }
        let mount = self
            .prepared
            .destination_ready_for_exposure()
            .ok_or(TransitionError::WrongState)?;
        if mount.frame_generation() != accepted_frame {
            return Err(TransitionError::StaleCandidateFrameAttempt);
        }
        let DestinationExposureOutcome { batch, exposure } =
            expose_destination(cx, &self.prepared)?;
        let exposure = RehostDestinationExposure {
            batch,
            receipt: exposure,
        };
        self.accepted_destination_exposure = Some((accepted_frame, exposure.clone()));
        Ok(exposure)
    }

    /// Accepts one visible frame for the already exposed destination batch.
    pub fn accept_destination_presentation_frame(
        &self,
        cx: &App,
        accepted_frame: u64,
    ) -> Result<RehostDestinationPresentation, TransitionError> {
        let receipt =
            presented_batch_receipt(cx, self.destination()).ok_or(TransitionError::WrongState)?;
        if receipt.frame_generation() != accepted_frame {
            return Err(TransitionError::StaleCandidateFrameAttempt);
        }
        Ok(RehostDestinationPresentation { receipt })
    }

    /// Returns the currently accepted visible frame for the exposed destination batch.
    pub fn current_destination_presentation(
        &self,
        cx: &App,
    ) -> Result<RehostDestinationPresentation, TransitionError> {
        let receipt =
            presented_batch_receipt(cx, self.destination()).ok_or(TransitionError::WrongState)?;
        Ok(RehostDestinationPresentation { receipt })
    }

    /// Requests ordinary source settlement without exposing the provider phase machine.
    pub fn settle_source(&mut self, cx: &mut App) -> Result<SourceSettlement, TransitionError> {
        settle_rehost_source(cx, &self.prepared)
    }

    /// Finishes the source restoration mounted in the supplied accepted staging frame.
    pub fn accept_source_restoration_frame(
        &mut self,
        cx: &mut App,
        accepted_frame: u64,
    ) -> Result<SourcePresentationFinish, TransitionError> {
        if let Some(disposition) = self.terminal_disposition() {
            if disposition != RehostTerminalDisposition::SourceCommitted {
                return Err(TransitionError::ConflictingTerminalOutcome);
            }
            return self
                .accepted_source_finish
                .as_ref()
                .and_then(|(committed_frame, outcome)| {
                    (*committed_frame == accepted_frame).then(|| outcome.clone())
                })
                .ok_or(TransitionError::StaleCandidateFrameAttempt);
        }
        if let Some((committed_frame, outcome)) = &self.accepted_source_finish {
            return (*committed_frame == accepted_frame)
                .then(|| outcome.clone())
                .ok_or(TransitionError::StaleCandidateFrameAttempt);
        }
        let source = cx
            .view_presentation_windows
            .source_restoration_for_frame(&self.prepared, accepted_frame)?;
        let outcome = finish_rendered_rehost_source(cx, &self.prepared, &source)?;
        self.accepted_source_finish = Some((accepted_frame, outcome.clone()));
        Ok(outcome)
    }

    /// Validates the final provider step for a larger synchronous authority transaction.
    pub fn prepare_terminal(
        &self,
        cx: &App,
        intent: RehostTerminalIntent<'_>,
    ) -> Result<RehostTerminalPreparation, TransitionError> {
        match intent {
            RehostTerminalIntent::CommitDestination(presentation) => {
                let prepared = match prepare_finish_destination(cx, &self.prepared) {
                    Ok(prepared) => prepared,
                    Err(TransitionError::StalePrepared) => {
                        return match self.prepared.terminal_tombstone() {
                            Some(terminal) => terminal
                                .destination_outcome(self.destination(), presentation)
                                .map(RehostTerminalPreparation::AlreadyCommitted)
                                .ok_or(TransitionError::ConflictingTerminalOutcome),
                            None => Err(TransitionError::StalePrepared),
                        };
                    }
                    Err(error) => return Err(error),
                };
                if prepared.presentation != presentation.receipt
                    || prepared.exposure != presentation.receipt.exposure()
                    || !prepared.batch.matches_exactly(self.destination())
                {
                    return Err(TransitionError::StalePrepared);
                }
                Ok(RehostTerminalPreparation::Prepared(
                    PreparedRehostTerminal {
                        transition: PreparedRehostTerminalTransition::CommitDestination(prepared),
                    },
                ))
            }
            RehostTerminalIntent::AbandonAfterSourceLoss => {
                match prepare_abandon_rehost_after_source_loss(cx, &self.prepared) {
                    Ok(prepared) => Ok(RehostTerminalPreparation::Prepared(
                        PreparedRehostTerminal {
                            transition: PreparedRehostTerminalTransition::AbandonAfterSourceLoss(
                                prepared,
                            ),
                        },
                    )),
                    Err(TransitionError::StalePrepared) => match self.prepared.terminal_tombstone()
                    {
                        Some(terminal) => terminal
                            .abandonment_receipt()
                            .map(|receipt| {
                                RehostTerminalPreparation::AlreadyCommitted(
                                    RehostTerminalOutcome::Abandoned(receipt),
                                )
                            })
                            .ok_or(TransitionError::ConflictingTerminalOutcome),
                        None => Err(TransitionError::StalePrepared),
                    },
                    Err(error) => Err(error),
                }
            }
        }
    }

    /// Atomically abandons this exact provider generation after source topology loss.
    pub fn abandon_after_source_loss(
        &mut self,
        cx: &mut App,
    ) -> Result<RehostAbandonmentOutcome, TransitionError> {
        let receipt = RehostAbandonmentReceipt::from_prepared(&self.prepared);
        match abandon_rehost_after_source_loss(cx, &self.prepared)? {
            AbandonRehostOutcome::Abandoned(_) => Ok(RehostAbandonmentOutcome::Abandoned(receipt)),
            AbandonRehostOutcome::AlreadyAbandoned => {
                Ok(RehostAbandonmentOutcome::AlreadyAbandoned(receipt))
            }
        }
    }
}

/// Error returned while claiming initial presentation authority.
#[doc(hidden)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ClaimError {
    /// No presentation roots were supplied.
    Empty,
    /// The same entity appeared more than once in one atomic claim.
    DuplicateEntity(EntityId),
    /// The requested window is no longer registered.
    WindowUnavailable,
    /// The root is already governed by another window.
    AlreadyBound { current: Lease },
    /// Existing roots belong to different atomic lease generations.
    MixedBatchGenerations { first: Lease, current: Lease },
    /// The root already participates in a prepared rehost.
    RehostInFlight,
}

/// Error returned while preparing an atomic rehost.
#[doc(hidden)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PrepareError {
    /// No presentation roots were supplied.
    Empty,
    /// The same entity appeared more than once.
    DuplicateEntity(EntityId),
    /// Source leases did not all belong to one window.
    MixedSourceWindows,
    /// Source and destination windows were identical.
    SameWindow,
    /// The destination window is no longer registered.
    DestinationUnavailable,
    /// A supplied source lease was stale.
    StaleLease(Lease),
    /// A source root has not yet committed an accepted mount frame.
    SourceNotMounted(Lease),
    /// One source root already participates in another rehost.
    RehostInFlight(Lease),
}

/// Outcome of preparing a rehost from already-resolved view roots.
///
/// Only roots that retain stable authority in the exact expected source window participate in a
/// prepared rehost. Ungoverned roots and roots already governed by the destination remain
/// unchanged for the surrounding topology transaction to reconcile.
#[doc(hidden)]
#[derive(Debug)]
pub enum ResolvedViewRehostOutcome {
    /// No supplied root requires a presentation-window transfer.
    NoTransfer,
    /// The exact source-bound subset entered the ordinary prepared-rehost protocol.
    Prepared(RehostSession),
}

#[derive(Debug)]
enum ResolvedPreparedViewRehostOutcome {
    NoTransfer,
    Prepared(PreparedRehost),
}

/// Returns the stable lease already owned by one exact window, without claiming a new root.
///
/// This is used by reversible projections to render roots that were already authoritative in the
/// destination before the projection started. An ungoverned root deliberately returns `None`.
#[doc(hidden)]
pub fn stable_lease_for_window(
    cx: &App,
    entity_id: EntityId,
    window_id: WindowId,
) -> Option<Lease> {
    cx.view_presentation_windows
        .stable_lease_for_window(entity_id, window_id)
}

/// Error returned while resolving view roots into one exact prepared rehost.
#[doc(hidden)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResolvedViewRehostError {
    /// No view roots were supplied.
    Empty,
    /// The same root entity appeared more than once.
    DuplicateEntity(EntityId),
    /// The expected source and destination windows were identical.
    SameWindow,
    /// The destination window is no longer registered.
    DestinationUnavailable,
    /// One root already participates in another prepared rehost.
    RehostInFlight { current: Lease },
    /// One root drifted to a window other than the exact source or destination.
    UnexpectedWindow { current: Lease },
    /// The exact source-bound subset failed ordinary prepared-rehost validation.
    Prepare(PrepareError),
}

/// Error returned by an exact prepared-rehost transition.
#[doc(hidden)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TransitionError {
    /// The prepared generation is no longer registered.
    StalePrepared,
    /// The requested domain operation is not available from the current provider state.
    WrongState,
    /// The callback came from the wrong window.
    WrongWindow,
    /// The source proxy did not prove the mechanism and exact lease bound by the barrier.
    WrongSourceProxyEvidence,
    /// Retained replay evidence came from a different candidate attempt.
    StaleCandidateFrameAttempt,
    /// The generation already reached a different terminal outcome.
    ConflictingTerminalOutcome,
    /// Exact source leases were still present in the release frame.
    SourceStillMounted,
    /// A governed lease no longer matched current authority.
    StaleLease,
}

/// Terminal result returned when a prepared rehost is retired.
#[derive(Clone, Debug)]
enum FinishOutcome {
    /// Destination authority mounted successfully.
    Destination {
        /// Exact leases exposed in the destination window.
        batch: LeaseBatch,
        /// Exact mount-to-exposure transition consumed by `finish`.
        exposure: DestinationExposureReceipt,
    },
    /// Source authority remained or was restored.
    Source(LeaseBatch),
    /// The rehost invalidated before a stable mount completed.
    Invalidated(Invalidation),
}

/// Atomic outcome of finishing a source-side rehost obligation.
#[derive(Clone, Debug)]
enum FinishSourceOutcome {
    /// The exact source batch remained authoritative and the rehost record was consumed.
    Finished(FinishOutcome),
    /// Exact source bindings drifted, so ordinary restoration authority was released for recovery.
    PresentationAuthorityReleased(Invalidation),
}

/// Result of retiring a source-restoration batch after its accepted staging frame.
#[doc(hidden)]
#[derive(Clone, Debug)]
pub enum SourcePresentationFinish {
    /// The exact source batch is stable and the rehost session is retired.
    Finished(LeaseBatch),
    /// Exact source bindings drifted and GPUI released presentation authority.
    PresentationAuthorityReleased(Invalidation),
}

/// Next source-side action after requesting cancellation of one exact rehost session.
///
/// This is the ordinary rollback interface for rehost consumers. GPUI owns the internal phase
/// transitions and terminal record retirement; callers only render a returned source batch, wait
/// for the owning platform terminal, or continue with topology recovery after presentation
/// authority was released.
#[doc(hidden)]
#[derive(Clone, Debug)]
pub enum SourceSettlement {
    /// The source is authoritative and the rehost record has been retired.
    RetiredToSource(LeaseBatch),
    /// Fresh source leases must be rendered in one accepted frame before terminal retirement.
    RenderSource(LeaseBatch),
    /// Logical source closure was observed and exact native termination remains outstanding.
    AwaitingSourceNativeTerminal,
    /// Presentation authority was released and durable topology recovery must continue.
    PresentationAuthorityReleased(Invalidation),
    /// The exact rehost generation was already retired by an earlier idempotent attempt.
    AlreadyRetired,
}

/// Result of atomically abandoning one exact rehost after source ownership was lost.
#[doc(hidden)]
#[derive(Clone, Debug)]
pub enum AbandonRehostOutcome {
    /// The exact rehost authority was released by this call.
    Abandoned(AbandonedRehostReceipt),
    /// The exact rehost generation had already been abandoned.
    AlreadyAbandoned,
}

/// Reversible destination exposure retained under one prepared rehost generation.
#[doc(hidden)]
#[derive(Clone, Debug)]
pub struct DestinationExposureOutcome {
    /// Exact leases exposed in the destination window.
    pub batch: LeaseBatch,
    /// Exact mount-to-exposure transition retained by the prepared rehost.
    pub exposure: DestinationExposureReceipt,
}

/// Single-use authority to retire one exactly validated exposed destination.
///
/// Prepare and commit must run in the same synchronous App turn without another presentation
/// mutation between them. Private fields make this token unforgeable outside GPUI, and the lack of
/// `Clone` makes committing it a single-consumption operation.
#[doc(hidden)]
#[derive(Debug)]
pub struct PreparedFinishDestination {
    prepared: PreparedRehost,
    batch: LeaseBatch,
    exposure: DestinationExposureReceipt,
    presentation: PresentedBatchReceipt,
}

impl PreparedFinishDestination {
    /// Returns whether this exact token can still commit without provider validation failure.
    pub fn can_commit(&self, cx: &App) -> bool {
        can_commit_prepared_finish_destination(cx, self)
    }

    /// Consumes this exactly validated provider commit.
    pub fn commit(self, cx: &mut App) -> DestinationExposureOutcome {
        commit_prepared_finish_destination(cx, self)
    }
}

/// Single-use authority to abandon one exact rehost after its source topology is lost.
///
/// The token snapshots every governed binding and the rehost phase. Commit it in the same
/// synchronous App turn as the surrounding recovery transaction so a replacement presentation
/// generation cannot be mistaken for the abandoned one.
#[doc(hidden)]
#[derive(Debug)]
pub struct PreparedAbandonRehostAfterSourceLoss {
    prepared: PreparedRehost,
    phase: RehostPhase,
    bindings: Vec<(EntityId, Option<Binding>)>,
}

impl PreparedAbandonRehostAfterSourceLoss {
    /// Returns whether this exact abandonment token can still commit.
    pub fn can_commit(&self, cx: &App) -> bool {
        can_commit_prepared_abandon_rehost_after_source_loss(cx, self)
    }

    /// Consumes this exact source-loss abandonment token.
    pub fn commit(self, cx: &mut App) -> AbandonedRehostReceipt {
        commit_prepared_abandon_rehost_after_source_loss(cx, self)
    }
}

/// Receipt proving that one exact rehost generation no longer owns presentation authority.
#[doc(hidden)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AbandonedRehostReceipt {
    generation: u64,
    source_window: WindowId,
    destination_window: WindowId,
    released_entities: Vec<EntityId>,
}

impl AbandonedRehostReceipt {
    /// Returns the abandoned rehost generation.
    pub const fn generation(&self) -> u64 {
        self.generation
    }

    /// Returns the source window governed by the abandoned rehost.
    pub const fn source_window(&self) -> WindowId {
        self.source_window
    }

    /// Returns the destination window governed by the abandoned rehost.
    pub const fn destination_window(&self) -> WindowId {
        self.destination_window
    }

    /// Returns the entities whose exact rehost authority was released.
    pub fn released_entities(&self) -> &[EntityId] {
        &self.released_entities
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Binding {
    current: Lease,
    publication: PrepaintPublicationId,
    last_mounted_frame: Option<u64>,
    pending_rehost: Option<u64>,
    destination_exposure: Option<DestinationExposureReceipt>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PresentationAdmission {
    Rejected,
    Staging,
    Presented,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum MountCommitOutcome {
    Stable,
    AwaitingBatch,
    BatchCompleted,
}

#[derive(Debug)]
struct RehostRecord {
    prepared: PreparedRehost,
    phase: RehostPhase,
    source_proxy_receipt: Option<SourceProxyCommitReceipt>,
    source_window_closed: bool,
    destination_mount_frame: Option<u64>,
    destination_mounted: FxHashSet<EntityId>,
    restore: Option<LeaseBatch>,
    restore_mount_frame: Option<u64>,
    restore_mounted: FxHashSet<EntityId>,
}

impl RehostRecord {
    fn destination_mount_receipt(&self) -> Option<DestinationMountReceipt> {
        if !matches!(
            self.phase,
            RehostPhase::DestinationMounted | RehostPhase::DestinationExposed
        ) {
            return None;
        }
        Some(DestinationMountReceipt {
            source_proxy: self
                .source_proxy_receipt
                .expect("mounted destination must retain its accepted source proxy"),
            rehost_generation: self.prepared.generation,
            destination_window: self.prepared.destination.window_id,
            destination_lease_generation: self
                .prepared
                .destination
                .common_generation()
                .expect("prepared destination leases must share one generation"),
            root_count: self.prepared.destination.leases.len(),
            frame_generation: self
                .destination_mount_frame
                .expect("mounted destination must retain its accepted frame"),
        })
    }

    fn publish(&self, invalidation: Option<Invalidation>) {
        *self.prepared.snapshot.lock() = RehostSnapshot {
            phase: self.phase,
            source_proxy_receipt: self.source_proxy_receipt,
            destination_frame_generation: match self.phase {
                RehostPhase::DestinationMounted | RehostPhase::DestinationExposed => {
                    self.destination_mount_frame
                }
                RehostPhase::SourceRestored => self.restore_mount_frame,
                _ => None,
            },
            destination_mount_receipt: self.destination_mount_receipt(),
            invalidation,
        };
    }
}

/// App-local registry for opt-in presentation roots.
#[derive(Debug, Default)]
pub(crate) struct Registry {
    next_generation: u64,
    bindings: FxHashMap<EntityId, Binding>,
    rehosts: FxHashMap<u64, RehostRecord>,
    accepted_presentation_frames: FxHashMap<WindowId, u64>,
}

impl Registry {
    fn allocate_generation(&mut self) -> u64 {
        self.next_generation = self
            .next_generation
            .checked_add(1)
            .expect("view presentation generation space exhausted");
        self.next_generation
    }

    fn claim_batch(
        &mut self,
        entity_ids: &[EntityId],
        window_id: WindowId,
    ) -> Result<LeaseBatch, ClaimError> {
        if entity_ids.is_empty() {
            return Err(ClaimError::Empty);
        }
        let mut first_existing = None;
        let mut unique = FxHashSet::default();
        for entity_id in entity_ids.iter().copied() {
            if !unique.insert(entity_id) {
                return Err(ClaimError::DuplicateEntity(entity_id));
            }
            let Some(binding) = self.bindings.get(&entity_id).copied() else {
                continue;
            };
            if binding.pending_rehost.is_some() {
                return Err(ClaimError::RehostInFlight);
            }
            if binding.current.window_id != window_id {
                return Err(ClaimError::AlreadyBound {
                    current: binding.current,
                });
            }
            match first_existing {
                None => first_existing = Some(binding.current),
                Some(first) if first.generation == binding.current.generation => {}
                Some(first) => {
                    return Err(ClaimError::MixedBatchGenerations {
                        first,
                        current: binding.current,
                    });
                }
            }
        }

        let batch_generation = first_existing
            .map(Lease::generation)
            .unwrap_or_else(|| self.allocate_generation());
        let mut leases = Vec::with_capacity(entity_ids.len());
        for entity_id in entity_ids.iter().copied() {
            if let Some(binding) = self.bindings.get(&entity_id).copied() {
                leases.push(binding.current);
                continue;
            }
            let lease = Lease {
                entity_id,
                generation: batch_generation,
                window_id,
            };
            self.bindings.insert(
                entity_id,
                Binding {
                    current: lease,
                    publication: PrepaintPublicationId::new(),
                    last_mounted_frame: None,
                    pending_rehost: None,
                    destination_exposure: None,
                },
            );
            leases.push(lease);
        }
        Ok(LeaseBatch::new(window_id, leases))
    }

    #[cfg(test)]
    fn claim(&mut self, entity_id: EntityId, window_id: WindowId) -> Result<Lease, ClaimError> {
        self.claim_batch(&[entity_id], window_id).map(|batch| {
            batch
                .lease_for(entity_id)
                .expect("a successful singleton claim must return its lease")
        })
    }

    fn prepare(
        &mut self,
        source: &[Lease],
        destination_window: WindowId,
    ) -> Result<PreparedRehost, PrepareError> {
        let Some(first) = source.first().copied() else {
            return Err(PrepareError::Empty);
        };
        if first.window_id == destination_window {
            return Err(PrepareError::SameWindow);
        }

        let mut entity_ids = FxHashSet::default();
        for lease in source.iter().copied() {
            if !entity_ids.insert(lease.entity_id) {
                return Err(PrepareError::DuplicateEntity(lease.entity_id));
            }
            if lease.window_id != first.window_id {
                return Err(PrepareError::MixedSourceWindows);
            }
            let Some(binding) = self.bindings.get(&lease.entity_id) else {
                return Err(PrepareError::StaleLease(lease));
            };
            if binding.current != lease {
                return Err(PrepareError::StaleLease(lease));
            }
            if binding.last_mounted_frame.is_none() {
                return Err(PrepareError::SourceNotMounted(lease));
            }
            if binding.pending_rehost.is_some() {
                return Err(PrepareError::RehostInFlight(lease));
            }
        }

        let generation = self.allocate_generation();
        let destination = LeaseBatch::new(
            destination_window,
            source
                .iter()
                .map(|lease| Lease {
                    entity_id: lease.entity_id,
                    generation,
                    window_id: destination_window,
                })
                .collect(),
        );
        let source = LeaseBatch::new(first.window_id, source.to_vec());
        for lease in source.leases.iter() {
            self.bindings
                .get_mut(&lease.entity_id)
                .expect("validated presentation binding must remain registered")
                .pending_rehost = Some(generation);
        }
        let snapshot = Arc::new(Mutex::new(RehostSnapshot {
            phase: RehostPhase::AwaitingSourceRelease,
            source_proxy_receipt: None,
            destination_frame_generation: None,
            destination_mount_receipt: None,
            invalidation: None,
        }));
        let prepared = PreparedRehost {
            generation,
            source,
            destination,
            restored_source: Arc::new(Mutex::new(None)),
            snapshot,
            terminal: Arc::new(Mutex::new(None)),
        };
        self.rehosts.insert(
            generation,
            RehostRecord {
                prepared: prepared.clone(),
                phase: RehostPhase::AwaitingSourceRelease,
                source_proxy_receipt: None,
                source_window_closed: false,
                destination_mount_frame: None,
                destination_mounted: FxHashSet::default(),
                restore: None,
                restore_mount_frame: None,
                restore_mounted: FxHashSet::default(),
            },
        );
        Ok(prepared)
    }

    fn prepare_resolved_view_rehost(
        &mut self,
        entity_ids: &[EntityId],
        expected_source_window: WindowId,
        destination_window: WindowId,
    ) -> Result<ResolvedPreparedViewRehostOutcome, ResolvedViewRehostError> {
        if entity_ids.is_empty() {
            return Err(ResolvedViewRehostError::Empty);
        }
        if expected_source_window == destination_window {
            return Err(ResolvedViewRehostError::SameWindow);
        }

        let mut unique = FxHashSet::default();
        let mut exact_source = Vec::new();
        for entity_id in entity_ids.iter().copied() {
            if !unique.insert(entity_id) {
                return Err(ResolvedViewRehostError::DuplicateEntity(entity_id));
            }
            let Some(binding) = self.bindings.get(&entity_id).copied() else {
                continue;
            };
            if binding.pending_rehost.is_some() {
                return Err(ResolvedViewRehostError::RehostInFlight {
                    current: binding.current,
                });
            }
            if binding.current.window_id == destination_window {
                continue;
            }
            if binding.current.window_id == expected_source_window {
                exact_source.push(binding.current);
                continue;
            }
            return Err(ResolvedViewRehostError::UnexpectedWindow {
                current: binding.current,
            });
        }

        if exact_source.is_empty() {
            return Ok(ResolvedPreparedViewRehostOutcome::NoTransfer);
        }
        self.prepare(&exact_source, destination_window)
            .map(ResolvedPreparedViewRehostOutcome::Prepared)
            .map_err(ResolvedViewRehostError::Prepare)
    }

    #[cfg(test)]
    fn admits(&self, lease: Lease, window_id: WindowId) -> bool {
        self.presentation_admission(lease, window_id) != PresentationAdmission::Rejected
    }

    fn presentation_admission(&self, lease: Lease, window_id: WindowId) -> PresentationAdmission {
        if lease.window_id != window_id {
            return PresentationAdmission::Rejected;
        }
        let Some(binding) = self.bindings.get(&lease.entity_id) else {
            return PresentationAdmission::Rejected;
        };
        if binding.current != lease {
            return PresentationAdmission::Rejected;
        }
        let Some(rehost_generation) = binding.pending_rehost else {
            return PresentationAdmission::Presented;
        };
        let Some(record) = self.rehosts.get(&rehost_generation) else {
            return PresentationAdmission::Rejected;
        };
        match record.phase {
            RehostPhase::DestinationAdmitted | RehostPhase::DestinationMounted => {
                if record.prepared.destination.lease_for(lease.entity_id) == Some(lease) {
                    PresentationAdmission::Staging
                } else {
                    PresentationAdmission::Rejected
                }
            }
            RehostPhase::RestoringSource | RehostPhase::SourceRestored => {
                if record
                    .restore
                    .as_ref()
                    .is_some_and(|restore| restore.lease_for(lease.entity_id) == Some(lease))
                {
                    PresentationAdmission::Staging
                } else {
                    PresentationAdmission::Rejected
                }
            }
            RehostPhase::DestinationExposed => {
                if record.prepared.destination.lease_for(lease.entity_id) == Some(lease) {
                    PresentationAdmission::Presented
                } else {
                    PresentationAdmission::Rejected
                }
            }
            RehostPhase::AwaitingSourceRelease
            | RehostPhase::Cancelled
            | RehostPhase::Invalidated => PresentationAdmission::Presented,
        }
    }

    fn presentation_publication(
        &self,
        lease: Lease,
        window_id: WindowId,
    ) -> Option<PrepaintPublicationId> {
        (self.presentation_admission(lease, window_id) != PresentationAdmission::Rejected)
            .then(|| {
                self.bindings
                    .get(&lease.entity_id)
                    .map(|binding| binding.publication)
            })
            .flatten()
    }

    fn begin_accepted_presentation_frame(
        &mut self,
        window_id: WindowId,
        frame_generation: u64,
    ) -> bool {
        if let Some(current) = self.accepted_presentation_frames.get(&window_id) {
            if *current > frame_generation {
                return false;
            }
            if *current == frame_generation {
                return true;
            }
        }
        self.accepted_presentation_frames
            .insert(window_id, frame_generation);

        for binding in self.bindings.values_mut() {
            if binding.current.window_id == window_id {
                binding.last_mounted_frame = None;
            }
        }

        for record in self.rehosts.values_mut() {
            match record.phase {
                RehostPhase::DestinationAdmitted
                    if record.prepared.destination.window_id == window_id =>
                {
                    record.destination_mount_frame = None;
                    record.destination_mounted.clear();
                }
                RehostPhase::DestinationMounted
                    if record.prepared.destination.window_id == window_id =>
                {
                    record.phase = RehostPhase::DestinationAdmitted;
                    record.destination_mount_frame = None;
                    record.destination_mounted.clear();
                    record.publish(None);
                }
                RehostPhase::RestoringSource
                    if record
                        .restore
                        .as_ref()
                        .is_some_and(|restore| restore.window_id == window_id) =>
                {
                    record.restore_mount_frame = None;
                    record.restore_mounted.clear();
                }
                RehostPhase::SourceRestored
                    if record
                        .restore
                        .as_ref()
                        .is_some_and(|restore| restore.window_id == window_id) =>
                {
                    let invalidation = record.prepared.snapshot().invalidation();
                    record.phase = RehostPhase::RestoringSource;
                    record.restore_mount_frame = None;
                    record.restore_mounted.clear();
                    record.publish(invalidation);
                }
                RehostPhase::AwaitingSourceRelease
                | RehostPhase::DestinationAdmitted
                | RehostPhase::DestinationMounted
                | RehostPhase::DestinationExposed
                | RehostPhase::RestoringSource
                | RehostPhase::SourceRestored
                | RehostPhase::Cancelled
                | RehostPhase::Invalidated => {}
            }
        }
        true
    }

    fn presented_batch_receipt(&self, batch: &LeaseBatch) -> Option<PresentedBatchReceipt> {
        let lease_generation = batch.common_generation()?;
        let mut frame_generation = None;
        let mut exposure = None;
        for lease in batch.leases.iter().copied() {
            let binding = self.bindings.get(&lease.entity_id)?;
            if binding.current != lease {
                return None;
            }
            if let Some(rehost_generation) = binding.pending_rehost {
                let record = self.rehosts.get(&rehost_generation)?;
                if record.phase != RehostPhase::DestinationExposed
                    || record.prepared.destination.lease_for(lease.entity_id) != Some(lease)
                {
                    return None;
                }
            }
            let current_exposure = binding.destination_exposure?;
            match exposure {
                None => exposure = Some(current_exposure),
                Some(expected) if expected == current_exposure => {}
                Some(_) => return None,
            }
            let mounted_frame = binding.last_mounted_frame?;
            match frame_generation {
                None => frame_generation = Some(mounted_frame),
                Some(expected) if expected == mounted_frame => {}
                Some(_) => return None,
            }
        }
        let exposure = exposure?;
        let mount = exposure.mount;
        if mount.destination_window != batch.window_id
            || mount.destination_lease_generation != lease_generation
            || mount.root_count != batch.leases.len()
            || self.accepted_presentation_frames.get(&batch.window_id) != Some(&frame_generation?)
        {
            return None;
        }
        Some(PresentedBatchReceipt {
            exposure,
            frame_generation: frame_generation?,
        })
    }

    fn stable_batch_presentation_receipt(
        &self,
        batch: &LeaseBatch,
    ) -> Option<StableBatchPresentationReceipt> {
        let lease_generation = batch.common_generation()?;
        let mut frame_generation = None;
        for lease in batch.leases.iter().copied() {
            let binding = self.bindings.get(&lease.entity_id)?;
            if binding.current != lease || binding.pending_rehost.is_some() {
                return None;
            }
            let mounted_frame = binding.last_mounted_frame?;
            match frame_generation {
                None => frame_generation = Some(mounted_frame),
                Some(expected) if expected == mounted_frame => {}
                Some(_) => return None,
            }
        }
        let frame_generation = frame_generation?;
        if self.accepted_presentation_frames.get(&batch.window_id) != Some(&frame_generation) {
            return None;
        }
        Some(StableBatchPresentationReceipt {
            window_id: batch.window_id,
            lease_generation,
            root_count: batch.leases.len(),
            frame_generation,
        })
    }

    pub(crate) fn governs(&self, entity_id: EntityId) -> bool {
        self.bindings.contains_key(&entity_id)
    }

    pub(crate) fn resolved_window(&self, entity_id: EntityId) -> Option<WindowId> {
        self.bindings
            .get(&entity_id)
            .map(|binding| binding.current.window_id)
    }

    fn stable_lease_for_window(&self, entity_id: EntityId, window_id: WindowId) -> Option<Lease> {
        self.bindings.get(&entity_id).and_then(|binding| {
            (binding.current.window_id == window_id && binding.pending_rehost.is_none())
                .then_some(binding.current)
        })
    }

    fn validate_batch_authority(
        &self,
        expected: &LeaseBatch,
        rehost_generation: u64,
    ) -> Result<(), TransitionError> {
        for lease in expected.leases.iter().copied() {
            let Some(binding) = self.bindings.get(&lease.entity_id) else {
                return Err(TransitionError::StaleLease);
            };
            if binding.current != lease || binding.pending_rehost != Some(rehost_generation) {
                return Err(TransitionError::StaleLease);
            }
        }
        Ok(())
    }

    fn replace_batch_authority(
        &mut self,
        expected: &LeaseBatch,
        replacement: &LeaseBatch,
        rehost_generation: u64,
        current_windows: &mut FxHashMap<EntityId, WindowId>,
    ) -> Result<(), TransitionError> {
        self.validate_batch_authority(expected, rehost_generation)?;
        if expected.leases.len() != replacement.leases.len()
            || expected
                .leases
                .iter()
                .any(|lease| replacement.lease_for(lease.entity_id).is_none())
        {
            return Err(TransitionError::StalePrepared);
        }

        for lease in replacement.leases.iter().copied() {
            let binding = self
                .bindings
                .get_mut(&lease.entity_id)
                .expect("validated presentation batch must remain registered");
            binding.current = lease;
            binding.last_mounted_frame = None;
            binding.destination_exposure = None;
        }
        for lease in replacement.leases.iter().copied() {
            current_windows.insert(lease.entity_id, lease.window_id);
        }
        Ok(())
    }

    fn release_rehost_authority(
        &mut self,
        generation: u64,
        current_windows: Option<&mut FxHashMap<EntityId, WindowId>>,
    ) -> Vec<EntityId> {
        let (source_window, destination_window, governed) =
            if let Some(record) = self.rehosts.get(&generation) {
                (
                    Some(record.prepared.source.window_id),
                    Some(record.prepared.destination.window_id),
                    record
                        .prepared
                        .source
                        .leases
                        .iter()
                        .map(|lease| lease.entity_id)
                        .collect::<Vec<_>>(),
                )
            } else {
                (
                    None,
                    None,
                    self.bindings
                        .iter()
                        .filter_map(|(entity_id, binding)| {
                            (binding.pending_rehost == Some(generation)).then_some(*entity_id)
                        })
                        .collect::<Vec<_>>(),
                )
            };
        let mut removed = Vec::with_capacity(governed.len());
        for entity_id in governed {
            match self.bindings.get(&entity_id).copied() {
                Some(binding) if binding.pending_rehost == Some(generation) => {
                    self.bindings.remove(&entity_id);
                    removed.push((entity_id, Some(binding.current.window_id)));
                }
                None => removed.push((entity_id, None)),
                Some(_) => {}
            }
        }
        if let Some(current_windows) = current_windows {
            for (entity_id, current_window) in &removed {
                let matches_released_authority = match current_window {
                    Some(current_window) => current_windows.get(entity_id) == Some(current_window),
                    None => current_windows.get(entity_id).is_some_and(|window_id| {
                        Some(*window_id) == source_window || Some(*window_id) == destination_window
                    }),
                };
                if matches_released_authority {
                    current_windows.remove(entity_id);
                }
            }
        }
        removed
            .into_iter()
            .map(|(entity_id, _)| entity_id)
            .collect()
    }

    fn commit_mount(
        &mut self,
        lease: Lease,
        frame_generation: u64,
        current_windows: &mut FxHashMap<EntityId, WindowId>,
    ) -> Result<MountCommitOutcome, TransitionError> {
        if !self.begin_accepted_presentation_frame(lease.window_id, frame_generation) {
            return Err(TransitionError::StaleCandidateFrameAttempt);
        }
        let Some(binding) = self.bindings.get(&lease.entity_id) else {
            return Err(TransitionError::StaleLease);
        };
        if binding.current != lease {
            return Err(TransitionError::StaleLease);
        }
        let Some(rehost_generation) = binding.pending_rehost else {
            self.bindings
                .get_mut(&lease.entity_id)
                .expect("validated presentation binding must remain registered")
                .last_mounted_frame = Some(frame_generation);
            return Ok(MountCommitOutcome::Stable);
        };

        if !self.rehosts.contains_key(&rehost_generation) {
            let _ = self.release_rehost_authority(rehost_generation, Some(current_windows));
            return Err(TransitionError::StalePrepared);
        }
        self.bindings
            .get_mut(&lease.entity_id)
            .expect("validated presentation binding must remain registered")
            .last_mounted_frame = Some(frame_generation);

        let mut invalidation = None;
        let mut outcome = MountCommitOutcome::Stable;
        let record = self
            .rehosts
            .get_mut(&rehost_generation)
            .expect("validated rehost must remain registered");
        match record.phase {
            RehostPhase::AwaitingSourceRelease => {}
            RehostPhase::DestinationAdmitted => {
                outcome = MountCommitOutcome::AwaitingBatch;
                if record.prepared.destination.lease_for(lease.entity_id) != Some(lease) {
                    return Err(TransitionError::StaleLease);
                }
                match record.destination_mount_frame {
                    None => record.destination_mount_frame = Some(frame_generation),
                    Some(expected) if expected != frame_generation => {
                        invalidation = Some(Invalidation::DestinationFrameMismatch)
                    }
                    Some(_) => {}
                }
                record.destination_mounted.insert(lease.entity_id);
                if invalidation.is_none()
                    && record.destination_mounted.len() == record.prepared.destination.leases.len()
                {
                    record.phase = RehostPhase::DestinationMounted;
                    outcome = MountCommitOutcome::BatchCompleted;
                    record.publish(None);
                }
            }
            RehostPhase::RestoringSource => {
                outcome = MountCommitOutcome::AwaitingBatch;
                let Some(restore) = record.restore.as_ref() else {
                    return Err(TransitionError::StalePrepared);
                };
                if restore.lease_for(lease.entity_id) != Some(lease) {
                    return Err(TransitionError::StaleLease);
                }
                match record.restore_mount_frame {
                    None => record.restore_mount_frame = Some(frame_generation),
                    Some(expected) if expected != frame_generation => {
                        invalidation = Some(Invalidation::SourceRestoreFrameMismatch)
                    }
                    Some(_) => {}
                }
                record.restore_mounted.insert(lease.entity_id);
                if invalidation.is_none() && record.restore_mounted.len() == restore.leases.len() {
                    record.phase = RehostPhase::SourceRestored;
                    outcome = MountCommitOutcome::BatchCompleted;
                    let recovery_reason = record.prepared.snapshot().invalidation();
                    record.publish(recovery_reason);
                }
            }
            RehostPhase::DestinationMounted
            | RehostPhase::DestinationExposed
            | RehostPhase::SourceRestored
            | RehostPhase::Cancelled
            | RehostPhase::Invalidated => {}
        }

        if let Some(invalidation) = invalidation {
            match invalidation {
                Invalidation::DestinationFrameMismatch => {
                    self.begin_source_restore(
                        rehost_generation,
                        Some(invalidation),
                        current_windows,
                    )?;
                }
                Invalidation::SourceRestoreFrameMismatch => {
                    self.invalidate_rehost_and_release_authority(
                        rehost_generation,
                        invalidation,
                        current_windows,
                    );
                }
                _ => unreachable!("mount commits only detect batch frame mismatches"),
            }
            return Err(TransitionError::StalePrepared);
        }
        Ok(outcome)
    }

    fn commit_source_release(
        &mut self,
        prepared: &PreparedRehost,
        receipt: SourceProxyReplayReceipt,
        current_windows: &mut FxHashMap<EntityId, WindowId>,
    ) -> Result<WindowId, TransitionError> {
        let Some(record) = self.rehosts.get(&prepared.generation) else {
            return Err(TransitionError::StalePrepared);
        };
        if record.phase != RehostPhase::AwaitingSourceRelease {
            return Err(TransitionError::WrongState);
        }
        if receipt.rehost_generation != prepared.generation {
            return Err(TransitionError::StalePrepared);
        }
        if prepared.source.window_id != receipt.source_window {
            return Err(TransitionError::WrongWindow);
        }

        if self
            .validate_batch_authority(&prepared.source, prepared.generation)
            .is_err()
        {
            self.invalidate_rehost_and_release_authority(
                prepared.generation,
                Invalidation::StaleLease,
                current_windows,
            );
            return Err(TransitionError::StaleLease);
        }
        for source in prepared.source.leases.iter().copied() {
            if self
                .bindings
                .get(&source.entity_id)
                .is_some_and(|binding| binding.last_mounted_frame == Some(receipt.frame_generation))
            {
                self.invalidate_rehost(prepared.generation, Invalidation::SourceStillMounted);
                return Err(TransitionError::SourceStillMounted);
            }
        }

        self.replace_batch_authority(
            &prepared.source,
            &prepared.destination,
            prepared.generation,
            current_windows,
        )?;
        let record = self
            .rehosts
            .get_mut(&prepared.generation)
            .expect("validated rehost must remain registered");
        record.phase = RehostPhase::DestinationAdmitted;
        record.source_proxy_receipt = Some(SourceProxyCommitReceipt { replay: receipt });
        record.publish(None);
        Ok(prepared.destination.window_id)
    }

    fn cancel_before_source_release(
        &mut self,
        prepared: &PreparedRehost,
    ) -> Result<(), TransitionError> {
        let Some(record) = self.rehosts.get(&prepared.generation) else {
            return Err(TransitionError::StalePrepared);
        };
        if record.phase != RehostPhase::AwaitingSourceRelease {
            return Err(TransitionError::WrongState);
        }
        if self
            .validate_batch_authority(&prepared.source, prepared.generation)
            .is_err()
        {
            let _ = self.release_rehost_authority(prepared.generation, None);
            let record = self
                .rehosts
                .get_mut(&prepared.generation)
                .expect("validated rehost must remain registered");
            record.phase = RehostPhase::Invalidated;
            record.publish(Some(Invalidation::StaleLease));
            return Err(TransitionError::StaleLease);
        }
        for lease in prepared.source.leases.iter() {
            self.bindings
                .get_mut(&lease.entity_id)
                .expect("validated presentation binding must remain registered")
                .pending_rehost = None;
        }
        let record = self
            .rehosts
            .get_mut(&prepared.generation)
            .expect("validated rehost must remain registered");
        record.phase = RehostPhase::Cancelled;
        record.publish(None);
        Ok(())
    }

    fn cancel_after_source_release(
        &mut self,
        prepared: &PreparedRehost,
        current_windows: &mut FxHashMap<EntityId, WindowId>,
    ) -> Result<LeaseBatch, TransitionError> {
        let Some(record) = self.rehosts.get(&prepared.generation) else {
            return Err(TransitionError::StalePrepared);
        };
        if !matches!(
            record.phase,
            RehostPhase::DestinationAdmitted
                | RehostPhase::DestinationMounted
                | RehostPhase::DestinationExposed
        ) {
            return Err(TransitionError::WrongState);
        }
        self.begin_source_restore(prepared.generation, None, current_windows)
    }

    fn begin_source_restore(
        &mut self,
        generation: u64,
        recovery_reason: Option<Invalidation>,
        current_windows: &mut FxHashMap<EntityId, WindowId>,
    ) -> Result<LeaseBatch, TransitionError> {
        let (source_window, source_entities, destination) = {
            let Some(record) = self.rehosts.get(&generation) else {
                return Err(TransitionError::StalePrepared);
            };
            if !matches!(
                record.phase,
                RehostPhase::DestinationAdmitted
                    | RehostPhase::DestinationMounted
                    | RehostPhase::DestinationExposed
            ) {
                return Err(TransitionError::WrongState);
            }
            if record.source_window_closed {
                self.invalidate_rehost_and_release_authority(
                    generation,
                    Invalidation::SourceWindowClosed,
                    current_windows,
                );
                return Err(TransitionError::StalePrepared);
            }
            (
                record.prepared.source.window_id,
                record
                    .prepared
                    .source
                    .leases
                    .iter()
                    .map(|lease| lease.entity_id)
                    .collect::<Vec<_>>(),
                record.prepared.destination.clone(),
            )
        };
        if self
            .validate_batch_authority(&destination, generation)
            .is_err()
        {
            self.invalidate_rehost_and_release_authority(
                generation,
                Invalidation::StaleLease,
                current_windows,
            );
            return Err(TransitionError::StaleLease);
        }

        let restore_generation = self.allocate_generation();
        let restore = LeaseBatch::new(
            source_window,
            source_entities
                .into_iter()
                .map(|entity_id| Lease {
                    entity_id,
                    generation: restore_generation,
                    window_id: source_window,
                })
                .collect(),
        );
        self.replace_batch_authority(&destination, &restore, generation, current_windows)?;
        let record = self
            .rehosts
            .get_mut(&generation)
            .expect("validated rehost must remain registered");
        record.phase = RehostPhase::RestoringSource;
        record.restore = Some(restore.clone());
        record.restore_mount_frame = None;
        record.restore_mounted.clear();
        *record.prepared.restored_source.lock() = Some(restore.clone());
        record.publish(recovery_reason);
        Ok(restore)
    }

    fn expose_destination(
        &mut self,
        prepared: &PreparedRehost,
    ) -> Result<DestinationExposureOutcome, TransitionError> {
        let (batch, exposure) = {
            let Some(record) = self.rehosts.get(&prepared.generation) else {
                return Err(TransitionError::StalePrepared);
            };
            if record.phase != RehostPhase::DestinationMounted {
                return Err(TransitionError::WrongState);
            }
            (
                record.prepared.destination.clone(),
                DestinationExposureReceipt {
                    mount: record
                        .destination_mount_receipt()
                        .expect("destination-mounted phase must retain an exact mount receipt"),
                },
            )
        };
        self.validate_batch_authority(&batch, prepared.generation)?;
        for lease in batch.leases.iter() {
            let binding = self
                .bindings
                .get_mut(&lease.entity_id)
                .expect("validated presentation batch must remain registered");
            binding.last_mounted_frame = None;
            binding.destination_exposure = Some(exposure);
        }
        let record = self
            .rehosts
            .get_mut(&prepared.generation)
            .expect("validated rehost must remain registered");
        record.phase = RehostPhase::DestinationExposed;
        record.publish(None);
        Ok(DestinationExposureOutcome { batch, exposure })
    }

    fn prepare_finish_destination(
        &self,
        prepared: &PreparedRehost,
    ) -> Result<PreparedFinishDestination, TransitionError> {
        let Some(record) = self.rehosts.get(&prepared.generation) else {
            return Err(TransitionError::StalePrepared);
        };
        if !record.prepared.matches_exactly(prepared) {
            return Err(TransitionError::StalePrepared);
        }
        if record.phase != RehostPhase::DestinationExposed {
            return Err(TransitionError::WrongState);
        }

        let batch = record.prepared.destination.clone();
        let exposure = DestinationExposureReceipt {
            mount: record
                .destination_mount_receipt()
                .expect("destination-exposed phase must retain an exact mount receipt"),
        };
        self.validate_batch_authority(&batch, prepared.generation)?;
        for lease in batch.leases.iter() {
            let Some(binding) = self.bindings.get(&lease.entity_id) else {
                return Err(TransitionError::StaleLease);
            };
            if binding.destination_exposure != Some(exposure) {
                return Err(TransitionError::StaleLease);
            }
        }
        let presentation = self
            .presented_batch_receipt(&batch)
            .filter(|presentation| presentation.exposure() == exposure)
            .ok_or(TransitionError::WrongState)?;

        Ok(PreparedFinishDestination {
            prepared: record.prepared.clone(),
            batch,
            exposure,
            presentation,
        })
    }

    fn commit_prepared_finish_destination(
        &mut self,
        prepared_finish: PreparedFinishDestination,
    ) -> FinishOutcome {
        assert!(
            self.can_commit_prepared_finish_destination(&prepared_finish),
            "prepared destination finish must remain exact until commit"
        );
        let PreparedFinishDestination {
            prepared,
            batch,
            exposure,
            presentation,
        } = prepared_finish;
        let generation = prepared.generation;

        self.rehosts
            .remove(&generation)
            .expect("validated destination finish must remain registered");
        for lease in batch.leases.iter() {
            let binding = self
                .bindings
                .get_mut(&lease.entity_id)
                .expect("validated destination finish must retain every binding");
            binding.pending_rehost = None;
            binding.last_mounted_frame = Some(presentation.frame_generation());
            binding.destination_exposure = Some(exposure);
        }
        prepared.record_terminal(RehostTerminalTombstone::DestinationCommitted {
            batch: batch.clone(),
            exposure,
            accepted_frame: Some(presentation.frame_generation()),
        });

        FinishOutcome::Destination { batch, exposure }
    }

    fn can_commit_prepared_finish_destination(
        &self,
        prepared_finish: &PreparedFinishDestination,
    ) -> bool {
        let Some(record) = self.rehosts.get(&prepared_finish.prepared.generation) else {
            return false;
        };
        record.prepared.matches_exactly(&prepared_finish.prepared)
            && record.phase == RehostPhase::DestinationExposed
            && record
                .prepared
                .destination
                .matches_exactly(&prepared_finish.batch)
            && record.destination_mount_receipt() == Some(prepared_finish.exposure.mount)
            && self.presented_batch_receipt(&prepared_finish.batch)
                == Some(prepared_finish.presentation)
            && prepared_finish.batch.leases.iter().all(|lease| {
                self.bindings.get(&lease.entity_id).is_some_and(|binding| {
                    binding.current == *lease
                        && binding.pending_rehost == Some(prepared_finish.prepared.generation)
                        && binding.destination_exposure == Some(prepared_finish.exposure)
                })
            })
    }

    fn source_finish_is_committed(&self, prepared: &PreparedRehost, source: &LeaseBatch) -> bool {
        !self.rehosts.contains_key(&prepared.generation())
            && prepared
                .terminal_tombstone()
                .is_some_and(|terminal| terminal.source_finish_matches(source))
    }

    fn source_restoration_for_frame(
        &self,
        prepared: &PreparedRehost,
        accepted_frame: u64,
    ) -> Result<LeaseBatch, TransitionError> {
        let Some(record) = self.rehosts.get(&prepared.generation) else {
            let source = prepared
                .restored_source()
                .ok_or(TransitionError::StalePrepared)?;
            let committed_in_frame = prepared
                .terminal_tombstone()
                .is_some_and(|terminal| terminal.source_frame_matches(&source, accepted_frame));
            return committed_in_frame
                .then_some(source)
                .ok_or(TransitionError::StalePrepared);
        };
        if !record.prepared.matches_exactly(prepared) {
            return Err(TransitionError::StalePrepared);
        }
        if record.phase != RehostPhase::SourceRestored {
            return Err(TransitionError::WrongState);
        }
        if record.restore_mount_frame != Some(accepted_frame) {
            return Err(TransitionError::StaleCandidateFrameAttempt);
        }
        record.restore.clone().ok_or(TransitionError::WrongState)
    }

    fn release_stable_leases_after_endpoint_loss(
        &mut self,
        leases: &[Lease],
        current_windows: &mut FxHashMap<EntityId, WindowId>,
    ) -> Vec<EntityId> {
        let mut released = Vec::new();
        for lease in leases {
            let releases_exact_stable_binding =
                self.bindings.get(&lease.entity_id).is_some_and(|binding| {
                    binding.current == *lease && binding.pending_rehost.is_none()
                });
            if releases_exact_stable_binding {
                self.bindings.remove(&lease.entity_id);
                if current_windows.get(&lease.entity_id) == Some(&lease.window_id) {
                    current_windows.remove(&lease.entity_id);
                }
                released.push(lease.entity_id);
            }
        }
        released
    }

    fn prepare_abandon_rehost_after_source_loss(
        &self,
        prepared: &PreparedRehost,
    ) -> Result<PreparedAbandonRehostAfterSourceLoss, TransitionError> {
        let Some(record) = self.rehosts.get(&prepared.generation) else {
            return Err(TransitionError::StalePrepared);
        };
        if !record.prepared.matches_exactly(prepared) {
            return Err(TransitionError::StalePrepared);
        }

        let mut bindings = record
            .prepared
            .source
            .leases
            .iter()
            .map(|lease| {
                (
                    lease.entity_id,
                    self.bindings.get(&lease.entity_id).copied(),
                )
            })
            .collect::<Vec<_>>();
        bindings.sort_unstable_by_key(|(entity_id, _)| *entity_id);
        Ok(PreparedAbandonRehostAfterSourceLoss {
            prepared: record.prepared.clone(),
            phase: record.phase,
            bindings,
        })
    }

    fn can_commit_prepared_abandon_rehost_after_source_loss(
        &self,
        abandonment: &PreparedAbandonRehostAfterSourceLoss,
    ) -> bool {
        let Some(record) = self.rehosts.get(&abandonment.prepared.generation) else {
            return false;
        };
        if !record.prepared.matches_exactly(&abandonment.prepared)
            || record.phase != abandonment.phase
        {
            return false;
        }

        abandonment
            .bindings
            .iter()
            .all(|(entity_id, expected)| self.bindings.get(entity_id).copied() == *expected)
    }

    fn rehost_authority_is_absent(&self, prepared: &PreparedRehost) -> bool {
        !self.rehosts.contains_key(&prepared.generation)
            && prepared
                .source
                .leases
                .iter()
                .chain(prepared.destination.leases.iter())
                .all(|lease| {
                    self.bindings
                        .get(&lease.entity_id)
                        .is_none_or(|binding| binding.pending_rehost != Some(prepared.generation))
                })
    }

    fn commit_prepared_abandon_rehost_after_source_loss(
        &mut self,
        abandonment: PreparedAbandonRehostAfterSourceLoss,
        current_windows: &mut FxHashMap<EntityId, WindowId>,
    ) -> AbandonedRehostReceipt {
        assert!(
            self.can_commit_prepared_abandon_rehost_after_source_loss(&abandonment),
            "prepared source-loss abandonment must remain exact until commit"
        );
        let generation = abandonment.prepared.generation;
        let source_window = abandonment.prepared.source.window_id;
        let destination_window = abandonment.prepared.destination.window_id;
        let terminal_receipt = RehostAbandonmentReceipt::from_prepared(&abandonment.prepared);
        let released_entities = self.release_rehost_authority(generation, Some(current_windows));
        self.rehosts
            .remove(&generation)
            .expect("validated source-loss abandonment must remain registered");
        abandonment
            .prepared
            .record_terminal(RehostTerminalTombstone::Abandoned(terminal_receipt));

        AbandonedRehostReceipt {
            generation,
            source_window,
            destination_window,
            released_entities,
        }
    }

    fn finish(&mut self, prepared: &PreparedRehost) -> Result<FinishOutcome, TransitionError> {
        if self
            .rehosts
            .get(&prepared.generation)
            .is_some_and(|record| record.phase == RehostPhase::DestinationExposed)
        {
            let prepared_finish = self.prepare_finish_destination(prepared)?;
            return Ok(self.commit_prepared_finish_destination(prepared_finish));
        }
        let Some(record) = self.rehosts.remove(&prepared.generation) else {
            return Err(TransitionError::StalePrepared);
        };
        let finished_staging = matches!(
            record.phase,
            RehostPhase::DestinationMounted | RehostPhase::SourceRestored
        );
        let outcome = match record.phase {
            RehostPhase::DestinationMounted => FinishOutcome::Destination {
                batch: record.prepared.destination.clone(),
                exposure: DestinationExposureReceipt {
                    mount: record
                        .destination_mount_receipt()
                        .expect("destination-mounted phase must retain an exact mount receipt"),
                },
            },
            RehostPhase::SourceRestored => FinishOutcome::Source(
                record
                    .restore
                    .clone()
                    .expect("restored phase must retain fresh source leases"),
            ),
            RehostPhase::Cancelled => FinishOutcome::Source(record.prepared.source.clone()),
            RehostPhase::Invalidated => FinishOutcome::Invalidated(
                record
                    .prepared
                    .snapshot()
                    .invalidation
                    .expect("invalidated phase must retain a reason"),
            ),
            _ => {
                self.rehosts.insert(prepared.generation, record);
                return Err(TransitionError::WrongState);
            }
        };
        for lease in record.prepared.source.leases.iter() {
            if let Some(binding) = self.bindings.get_mut(&lease.entity_id)
                && binding.pending_rehost == Some(prepared.generation)
            {
                binding.pending_rehost = None;
                if finished_staging {
                    binding.last_mounted_frame = None;
                }
                if let FinishOutcome::Destination { exposure, .. } = &outcome {
                    binding.destination_exposure = Some(*exposure);
                }
            }
        }
        let terminal = match &outcome {
            FinishOutcome::Destination { batch, exposure } => {
                RehostTerminalTombstone::DestinationCommitted {
                    batch: batch.clone(),
                    exposure: *exposure,
                    accepted_frame: None,
                }
            }
            FinishOutcome::Source(batch) => RehostTerminalTombstone::SourceCommitted {
                batch: batch.clone(),
                accepted_frame: (record.phase == RehostPhase::SourceRestored)
                    .then_some(record.restore_mount_frame)
                    .flatten(),
            },
            FinishOutcome::Invalidated(invalidation)
                if record.prepared.snapshot().source_invalidation_disposition()
                    == Some(SourceInvalidationDisposition::SourceAuthorityUnchanged) =>
            {
                RehostTerminalTombstone::SourceCommitted {
                    batch: record.prepared.source.clone(),
                    accepted_frame: None,
                }
            }
            FinishOutcome::Invalidated(_) => {
                RehostTerminalTombstone::RetiredWithoutPresentationAuthority
            }
        };
        record.prepared.record_terminal(terminal);
        Ok(outcome)
    }

    fn finish_source_or_release_authority(
        &mut self,
        prepared: &PreparedRehost,
        source: &LeaseBatch,
        current_windows: &mut FxHashMap<EntityId, WindowId>,
    ) -> Result<FinishSourceOutcome, TransitionError> {
        let Some(record) = self.rehosts.get(&prepared.generation) else {
            return Err(TransitionError::StalePrepared);
        };
        if !record.prepared.matches_exactly(prepared) {
            return Err(TransitionError::StalePrepared);
        }
        let (expected, pending_rehost) = match record.phase {
            RehostPhase::Cancelled => (record.prepared.source.clone(), None),
            RehostPhase::SourceRestored => (
                record
                    .restore
                    .clone()
                    .ok_or(TransitionError::StalePrepared)?,
                Some(prepared.generation),
            ),
            RehostPhase::Invalidated
                if record.prepared.snapshot().source_invalidation_disposition()
                    == Some(SourceInvalidationDisposition::SourceAuthorityUnchanged) =>
            {
                (record.prepared.source.clone(), None)
            }
            _ => return Err(TransitionError::WrongState),
        };
        if !expected.matches_exactly(source) {
            return Err(TransitionError::StalePrepared);
        }
        let exact_bindings = source.leases.iter().all(|lease| {
            self.bindings.get(&lease.entity_id).is_some_and(|binding| {
                binding.current == *lease && binding.pending_rehost == pending_rehost
            })
        });
        if !exact_bindings {
            for lease in source.leases.iter() {
                let releases_exact_source =
                    self.bindings.get(&lease.entity_id).is_some_and(|binding| {
                        binding.current == *lease && binding.pending_rehost == pending_rehost
                    });
                if releases_exact_source {
                    self.bindings.remove(&lease.entity_id);
                    if current_windows.get(&lease.entity_id) == Some(&lease.window_id) {
                        current_windows.remove(&lease.entity_id);
                    }
                }
            }
            self.invalidate_rehost_and_release_authority(
                prepared.generation,
                Invalidation::StaleLease,
                current_windows,
            );
            return Ok(FinishSourceOutcome::PresentationAuthorityReleased(
                Invalidation::StaleLease,
            ));
        }
        let outcome = self.finish(prepared)?;
        debug_assert!(matches!(
            outcome,
            FinishOutcome::Source(_) | FinishOutcome::Invalidated(_)
        ));
        Ok(FinishSourceOutcome::Finished(outcome))
    }

    fn abandon_rehost_after_source_loss(
        &mut self,
        prepared: &PreparedRehost,
        current_windows: &mut FxHashMap<EntityId, WindowId>,
    ) -> Result<AbandonRehostOutcome, TransitionError> {
        let abandonment = match self.prepare_abandon_rehost_after_source_loss(prepared) {
            Ok(abandonment) => abandonment,
            Err(TransitionError::StalePrepared) => {
                return match prepared.terminal_tombstone() {
                    Some(terminal) if terminal.abandonment_receipt().is_some() => {
                        Ok(AbandonRehostOutcome::AlreadyAbandoned)
                    }
                    Some(_) => Err(TransitionError::ConflictingTerminalOutcome),
                    None => Err(TransitionError::StalePrepared),
                };
            }
            Err(error) => return Err(error),
        };
        Ok(AbandonRehostOutcome::Abandoned(
            self.commit_prepared_abandon_rehost_after_source_loss(abandonment, current_windows),
        ))
    }

    fn settle_rehost_source(
        &mut self,
        prepared: &PreparedRehost,
        current_windows: &mut FxHashMap<EntityId, WindowId>,
    ) -> Result<SourceSettlement, TransitionError> {
        let Some(record) = self.rehosts.get(&prepared.generation) else {
            return match prepared.terminal_tombstone() {
                Some(RehostTerminalTombstone::SourceCommitted { .. }) => {
                    Ok(SourceSettlement::AlreadyRetired)
                }
                Some(_) => Err(TransitionError::ConflictingTerminalOutcome),
                None => Err(TransitionError::StalePrepared),
            };
        };
        if !record.prepared.matches_exactly(prepared) {
            return Err(TransitionError::StalePrepared);
        }

        match record.phase {
            RehostPhase::AwaitingSourceRelease => {
                match self.cancel_before_source_release(prepared) {
                    Ok(()) => self.finish_settled_source(
                        prepared,
                        prepared.source().clone(),
                        current_windows,
                    ),
                    Err(_error) if prepared.snapshot().phase() == RehostPhase::Invalidated => {
                        self.settle_invalidated_source(prepared, current_windows)
                    }
                    Err(error) => Err(error),
                }
            }
            RehostPhase::DestinationAdmitted
            | RehostPhase::DestinationMounted
            | RehostPhase::DestinationExposed => {
                match self.cancel_after_source_release(prepared, current_windows) {
                    Ok(source) => Ok(SourceSettlement::RenderSource(source)),
                    Err(_error) if prepared.snapshot().phase() == RehostPhase::Invalidated => {
                        self.settle_invalidated_source(prepared, current_windows)
                    }
                    Err(error) => Err(error),
                }
            }
            RehostPhase::RestoringSource | RehostPhase::SourceRestored => prepared
                .restored_source()
                .map(SourceSettlement::RenderSource)
                .ok_or(TransitionError::StalePrepared),
            RehostPhase::Cancelled => {
                self.finish_settled_source(prepared, prepared.source().clone(), current_windows)
            }
            RehostPhase::Invalidated => self.settle_invalidated_source(prepared, current_windows),
        }
    }

    fn settle_invalidated_source(
        &mut self,
        prepared: &PreparedRehost,
        current_windows: &mut FxHashMap<EntityId, WindowId>,
    ) -> Result<SourceSettlement, TransitionError> {
        let snapshot = prepared.snapshot();
        let invalidation = snapshot
            .invalidation()
            .ok_or(TransitionError::StalePrepared)?;
        match snapshot.source_invalidation_disposition() {
            Some(SourceInvalidationDisposition::SourceAuthorityUnchanged) => {
                self.finish_settled_source(prepared, prepared.source().clone(), current_windows)
            }
            Some(SourceInvalidationDisposition::AwaitingSourceNativeTerminal) => {
                Ok(SourceSettlement::AwaitingSourceNativeTerminal)
            }
            Some(SourceInvalidationDisposition::PresentationAuthorityReleased) => {
                let _ = self.abandon_rehost_after_source_loss(prepared, current_windows)?;
                Ok(SourceSettlement::PresentationAuthorityReleased(
                    invalidation,
                ))
            }
            None => Err(TransitionError::WrongState),
        }
    }

    fn finish_settled_source(
        &mut self,
        prepared: &PreparedRehost,
        source: LeaseBatch,
        current_windows: &mut FxHashMap<EntityId, WindowId>,
    ) -> Result<SourceSettlement, TransitionError> {
        match self.finish_source_or_release_authority(prepared, &source, current_windows)? {
            FinishSourceOutcome::Finished(FinishOutcome::Source(_)) => {
                Ok(SourceSettlement::RetiredToSource(source))
            }
            FinishSourceOutcome::Finished(FinishOutcome::Invalidated(invalidation)) => {
                debug_assert_eq!(prepared.snapshot().invalidation(), Some(invalidation));
                Ok(SourceSettlement::RetiredToSource(source))
            }
            FinishSourceOutcome::PresentationAuthorityReleased(invalidation) => {
                let _ = self.abandon_rehost_after_source_loss(prepared, current_windows)?;
                Ok(SourceSettlement::PresentationAuthorityReleased(
                    invalidation,
                ))
            }
            FinishSourceOutcome::Finished(FinishOutcome::Destination { .. }) => {
                Err(TransitionError::StalePrepared)
            }
        }
    }

    fn retire_released_rehost_session(
        &mut self,
        prepared: &PreparedRehost,
        current_windows: &mut FxHashMap<EntityId, WindowId>,
    ) -> Result<(), TransitionError> {
        if prepared.terminal_tombstone().is_some() {
            return Ok(());
        }

        let requires_abandonment = match self.settle_rehost_source(prepared, current_windows) {
            Ok(
                SourceSettlement::RetiredToSource(_)
                | SourceSettlement::PresentationAuthorityReleased(_)
                | SourceSettlement::AlreadyRetired,
            ) => false,
            Ok(
                SourceSettlement::RenderSource(_) | SourceSettlement::AwaitingSourceNativeTerminal,
            ) => true,
            Err(TransitionError::ConflictingTerminalOutcome) => false,
            Err(TransitionError::StalePrepared) if self.rehost_authority_is_absent(prepared) => {
                false
            }
            Err(_) => true,
        };

        if requires_abandonment {
            match self.abandon_rehost_after_source_loss(prepared, current_windows) {
                Ok(_) | Err(TransitionError::ConflictingTerminalOutcome) => {}
                Err(TransitionError::StalePrepared)
                    if self.rehost_authority_is_absent(prepared) => {}
                Err(error) => return Err(error),
            }
        }

        Ok(())
    }

    fn invalidate_rehost(&mut self, generation: u64, invalidation: Invalidation) {
        let Some(record) = self.rehosts.get_mut(&generation) else {
            return;
        };
        for lease in record.prepared.source.leases.iter() {
            if let Some(binding) = self.bindings.get_mut(&lease.entity_id)
                && binding.pending_rehost == Some(generation)
            {
                binding.pending_rehost = None;
            }
        }
        record.phase = RehostPhase::Invalidated;
        record.publish(Some(invalidation));
    }

    fn invalidate_rehost_and_release_authority(
        &mut self,
        generation: u64,
        invalidation: Invalidation,
        current_windows: &mut FxHashMap<EntityId, WindowId>,
    ) {
        let _ = self.release_rehost_authority(generation, Some(current_windows));
        self.publish_rehost_invalidation(generation, invalidation);
    }

    fn invalidate_rehost_and_release_authority_unmapped(
        &mut self,
        generation: u64,
        invalidation: Invalidation,
    ) -> Vec<EntityId> {
        let released = self.release_rehost_authority(generation, None);
        self.publish_rehost_invalidation(generation, invalidation);
        released
    }

    fn publish_rehost_invalidation(&mut self, generation: u64, invalidation: Invalidation) {
        let Some(record) = self.rehosts.get_mut(&generation) else {
            return;
        };
        record.phase = RehostPhase::Invalidated;
        record.publish(Some(invalidation));
    }

    pub(crate) fn entity_released(&mut self, entity_id: EntityId) -> Vec<EntityId> {
        let pending = self
            .bindings
            .get(&entity_id)
            .and_then(|binding| binding.pending_rehost);
        if let Some(generation) = pending {
            self.invalidate_rehost_and_release_authority_unmapped(
                generation,
                Invalidation::EntityReleased,
            )
        } else {
            self.bindings.remove(&entity_id);
            vec![entity_id]
        }
    }

    pub(crate) fn window_closed(
        &mut self,
        window_id: WindowId,
        current_windows: &mut FxHashMap<EntityId, WindowId>,
    ) {
        self.accepted_presentation_frames.remove(&window_id);
        let affected = self
            .rehosts
            .iter()
            .filter_map(|(generation, record)| {
                (record.prepared.source.window_id == window_id
                    || record.prepared.destination.window_id == window_id)
                    .then_some(*generation)
            })
            .collect::<Vec<_>>();
        for generation in affected {
            let Some(record) = self.rehosts.get(&generation) else {
                continue;
            };
            let source_closed = record.prepared.source.window_id == window_id;
            let destination_closed = record.prepared.destination.window_id == window_id;
            if source_closed
                && matches!(
                    record.phase,
                    RehostPhase::DestinationAdmitted
                        | RehostPhase::DestinationMounted
                        | RehostPhase::DestinationExposed
                )
            {
                self.rehosts
                    .get_mut(&generation)
                    .expect("affected rehost must remain registered")
                    .source_window_closed = true;
                continue;
            }
            let (restore_source, invalidation) = match record.phase {
                RehostPhase::AwaitingSourceRelease => {
                    if source_closed {
                        (false, Some(Invalidation::SourceWindowClosed))
                    } else if destination_closed {
                        (false, Some(Invalidation::DestinationWindowClosed))
                    } else {
                        (false, None)
                    }
                }
                RehostPhase::DestinationAdmitted
                | RehostPhase::DestinationMounted
                | RehostPhase::DestinationExposed => (
                    destination_closed && !record.source_window_closed,
                    destination_closed.then_some(Invalidation::DestinationWindowClosed),
                ),
                RehostPhase::RestoringSource
                | RehostPhase::SourceRestored
                | RehostPhase::Cancelled => (
                    false,
                    source_closed.then_some(Invalidation::SourceWindowClosed),
                ),
                RehostPhase::Invalidated => (false, None),
            };
            if let Some(invalidation) = invalidation {
                if restore_source {
                    let _ =
                        self.begin_source_restore(generation, Some(invalidation), current_windows);
                } else {
                    self.invalidate_rehost(generation, invalidation);
                }
            }
        }

        let governed = self
            .bindings
            .iter()
            .filter_map(|(entity_id, binding)| {
                (binding.current.window_id == window_id).then_some(*entity_id)
            })
            .collect::<Vec<_>>();
        for entity_id in governed {
            self.bindings.remove(&entity_id);
            if current_windows.get(&entity_id) == Some(&window_id) {
                current_windows.remove(&entity_id);
            }
        }
    }
}

/// Claims initial presentation authority for one view root.
#[doc(hidden)]
pub fn claim(cx: &mut App, view: &AnyView, window_id: WindowId) -> Result<Lease, ClaimError> {
    claim_batch(cx, std::slice::from_ref(view), window_id).map(|batch| {
        batch
            .lease_for(view.entity_id())
            .expect("a successful singleton claim must return its lease")
    })
}

/// Atomically claims initial presentation authority for one or more view roots.
#[doc(hidden)]
pub fn claim_batch(
    cx: &mut App,
    views: &[AnyView],
    window_id: WindowId,
) -> Result<LeaseBatch, ClaimError> {
    if !cx.window_handles.contains_key(&window_id) {
        return Err(ClaimError::WindowUnavailable);
    }
    let entity_ids = views.iter().map(AnyView::entity_id).collect::<Vec<_>>();
    let batch = cx
        .view_presentation_windows
        .claim_batch(&entity_ids, window_id)?;
    for lease in batch.leases.iter().copied() {
        cx.current_window_by_entity
            .insert(lease.entity_id, lease.window_id);
    }
    Ok(batch)
}

/// Prepares an atomic move while leaving every source lease authoritative.
#[doc(hidden)]
pub fn prepare_rehost(
    cx: &mut App,
    source: &[Lease],
    destination_window: WindowId,
) -> Result<RehostSession, PrepareError> {
    if !cx.window_handles.contains_key(&destination_window) {
        return Err(PrepareError::DestinationUnavailable);
    }
    let prepared = cx
        .view_presentation_windows
        .prepare(source, destination_window)?;
    Ok(RehostSession::new(prepared, cx))
}

/// Resolves view roots against current presentation authority and prepares the exact source-bound
/// subset for rehost.
///
/// Ungoverned roots and roots already governed by `destination_window` do not acquire new leases.
/// A root governed by any third window, or participating in any pending rehost, fails the entire
/// operation before registry mutation. The prepared variant returns one unique [`RehostSession`]
/// transition owner plus cloneable projections for endpoint rendering.
#[doc(hidden)]
pub fn prepare_resolved_view_rehost(
    cx: &mut App,
    views: &[AnyView],
    expected_source_window: WindowId,
    destination_window: WindowId,
) -> Result<ResolvedViewRehostOutcome, ResolvedViewRehostError> {
    if !cx.window_handles.contains_key(&destination_window) {
        return Err(ResolvedViewRehostError::DestinationUnavailable);
    }
    let entity_ids = views.iter().map(AnyView::entity_id).collect::<Vec<_>>();
    let outcome = cx.view_presentation_windows.prepare_resolved_view_rehost(
        &entity_ids,
        expected_source_window,
        destination_window,
    )?;
    Ok(match outcome {
        ResolvedPreparedViewRehostOutcome::NoTransfer => ResolvedViewRehostOutcome::NoTransfer,
        ResolvedPreparedViewRehostOutcome::Prepared(prepared) => {
            ResolvedViewRehostOutcome::Prepared(RehostSession::new(prepared, cx))
        }
    })
}

impl App {
    fn retire_released_rehost_session(&mut self, prepared: &PreparedRehost) {
        let source_window = prepared.source().window_id();
        let destination_window = prepared.destination().window_id();
        if let Err(error) = self
            .view_presentation_windows
            .retire_released_rehost_session(prepared, &mut self.current_window_by_entity)
        {
            log::error!(
                "failed to compensate released rehost session generation {}: {error:?}",
                prepared.generation()
            );
        }
        refresh_window(self, source_window);
        refresh_window(self, destination_window);
    }
}

/// Cancels an admitted or mounted-but-unexposed destination and returns fresh source leases.
#[cfg(test)]
fn cancel_after_source_release(
    cx: &mut App,
    prepared: &PreparedRehost,
) -> Result<LeaseBatch, TransitionError> {
    let source_window = prepared.source.window_id;
    let destination_window = prepared.destination.window_id;
    let restore = cx
        .view_presentation_windows
        .cancel_after_source_release(prepared, &mut cx.current_window_by_entity)?;
    refresh_window(cx, source_window);
    refresh_window(cx, destination_window);
    Ok(restore)
}

/// Exposes a fully mounted destination while retaining exact rollback authority.
///
/// Prepare and commit a destination finish only after the surrounding durable transaction can no
/// longer compensate back to the source. Until then, ordinary source settlement remains valid.
#[doc(hidden)]
pub fn expose_destination(
    cx: &mut App,
    prepared: &PreparedRehost,
) -> Result<DestinationExposureOutcome, TransitionError> {
    let outcome = cx.view_presentation_windows.expose_destination(prepared)?;
    refresh_window(cx, outcome.batch.window_id);
    Ok(outcome)
}

/// Validates every fallible precondition for retiring one exposed destination.
///
/// The returned authority keeps the rehost reversible. Commit it in the same synchronous App turn
/// with [`commit_prepared_finish_destination`], without another presentation mutation between the
/// two calls.
#[doc(hidden)]
pub fn prepare_finish_destination(
    cx: &App,
    prepared: &PreparedRehost,
) -> Result<PreparedFinishDestination, TransitionError> {
    cx.view_presentation_windows
        .prepare_finish_destination(prepared)
}

/// Returns whether an exact prepared destination finish can still commit without validation
/// failure.
#[doc(hidden)]
pub fn can_commit_prepared_finish_destination(
    cx: &App,
    prepared_finish: &PreparedFinishDestination,
) -> bool {
    cx.view_presentation_windows
        .can_commit_prepared_finish_destination(prepared_finish)
}

/// Consumes one exactly validated destination finish without another fallible transition.
///
/// This is the final presentation step for a larger already-prepared synchronous transaction.
#[doc(hidden)]
pub fn commit_prepared_finish_destination(
    cx: &mut App,
    prepared_finish: PreparedFinishDestination,
) -> DestinationExposureOutcome {
    let outcome = cx
        .view_presentation_windows
        .commit_prepared_finish_destination(prepared_finish);
    let FinishOutcome::Destination { batch, exposure } = outcome else {
        unreachable!("prepared destination finish must commit a destination outcome");
    };
    refresh_window(cx, batch.window_id);
    DestinationExposureOutcome { batch, exposure }
}

/// Validates every presentation precondition for abandoning one exact rehost after source loss.
///
/// The returned token does not mutate presentation authority. Commit it in the same synchronous
/// App turn as the surface recovery record, without another presentation mutation between the two
/// calls.
#[doc(hidden)]
pub fn prepare_abandon_rehost_after_source_loss(
    cx: &App,
    prepared: &PreparedRehost,
) -> Result<PreparedAbandonRehostAfterSourceLoss, TransitionError> {
    cx.view_presentation_windows
        .prepare_abandon_rehost_after_source_loss(prepared)
}

/// Returns whether an exact prepared source-loss abandonment can still commit.
#[doc(hidden)]
pub fn can_commit_prepared_abandon_rehost_after_source_loss(
    cx: &App,
    abandonment: &PreparedAbandonRehostAfterSourceLoss,
) -> bool {
    cx.view_presentation_windows
        .can_commit_prepared_abandon_rehost_after_source_loss(abandonment)
}

/// Returns whether one exact rehost generation has already relinquished every presentation
/// binding it governed.
///
/// This is the idempotent counterpart to source-loss abandonment. It lets a larger recovery
/// transaction distinguish an already-cancelled generation from a stale generation that still
/// owns a binding.
#[doc(hidden)]
pub fn rehost_authority_is_absent(cx: &App, prepared: &PreparedRehost) -> bool {
    cx.view_presentation_windows
        .rehost_authority_is_absent(prepared)
}

/// Returns whether one cancelled or restored source batch has already consumed its exact rehost.
///
/// This is the idempotent checkpoint for a larger source-restoration executor. It distinguishes a
/// committed source finish from a stale transition without replaying the single-use `finish` step.
/// Releases the exact still-stable members of one presentation batch after its endpoint is lost.
///
/// A member that has moved, entered another rehost, or no longer matches the supplied lease is
/// preserved. This makes endpoint-loss cleanup idempotent without touching replacement authority.
#[doc(hidden)]
pub fn release_stable_batch_after_endpoint_loss(cx: &mut App, batch: &LeaseBatch) -> Vec<EntityId> {
    release_stable_leases_after_endpoint_loss(cx, batch.leases())
}

/// Releases exact stable leases whose presentation endpoint has been lost.
///
/// Leases that already moved, entered another rehost, or were replaced remain untouched. This is
/// the endpoint-lifecycle counterpart to [`release_stable_batch_after_endpoint_loss`] for hosts
/// whose ordinary roots may have independent lease generations.
#[doc(hidden)]
pub fn release_stable_leases_after_endpoint_loss(cx: &mut App, leases: &[Lease]) -> Vec<EntityId> {
    let released = cx
        .view_presentation_windows
        .release_stable_leases_after_endpoint_loss(leases, &mut cx.current_window_by_entity);
    if !released.is_empty() {
        let released = released.iter().copied().collect::<FxHashSet<_>>();
        for window_id in leases
            .iter()
            .filter(|lease| released.contains(&lease.entity_id()))
            .map(|lease| lease.window_id())
            .collect::<FxHashSet<_>>()
        {
            refresh_window(cx, window_id);
        }
    }
    released
}

/// Releases only the exact rehost generation captured by a source-loss abandonment token.
#[doc(hidden)]
pub fn commit_prepared_abandon_rehost_after_source_loss(
    cx: &mut App,
    abandonment: PreparedAbandonRehostAfterSourceLoss,
) -> AbandonedRehostReceipt {
    let source_window = abandonment.prepared.source.window_id;
    let destination_window = abandonment.prepared.destination.window_id;
    let receipt = cx
        .view_presentation_windows
        .commit_prepared_abandon_rehost_after_source_loss(
            abandonment,
            &mut cx.current_window_by_entity,
        );
    refresh_window(cx, source_window);
    refresh_window(cx, destination_window);
    receipt
}

/// Atomically releases one exact rehost after its source endpoint or topology was lost.
///
/// Ordinary recovery paths should use this operation instead of assembling the provider's
/// prepare, validation, and commit protocol. The prepared token API remains available only for a
/// larger synchronous transaction that must validate multiple independent authorities before any
/// of them commits.
#[doc(hidden)]
pub fn abandon_rehost_after_source_loss(
    cx: &mut App,
    prepared: &PreparedRehost,
) -> Result<AbandonRehostOutcome, TransitionError> {
    let source_window = prepared.source.window_id;
    let destination_window = prepared.destination.window_id;
    let outcome = cx
        .view_presentation_windows
        .abandon_rehost_after_source_loss(prepared, &mut cx.current_window_by_entity)?;
    refresh_window(cx, source_window);
    refresh_window(cx, destination_window);
    Ok(outcome)
}

/// Requests source-side settlement without exposing the provider's internal phase protocol.
///
/// A returned source batch remains governed by the same session and must be rendered in one
/// accepted frame before the caller finishes source presentation. Every other successful outcome
/// has either retired the exact rehost or explicitly requires the source's native terminal.
#[doc(hidden)]
pub fn settle_rehost_source(
    cx: &mut App,
    prepared: &PreparedRehost,
) -> Result<SourceSettlement, TransitionError> {
    let source_window = prepared.source.window_id;
    let destination_window = prepared.destination.window_id;
    let outcome = cx
        .view_presentation_windows
        .settle_rehost_source(prepared, &mut cx.current_window_by_entity)?;
    refresh_window(cx, source_window);
    refresh_window(cx, destination_window);
    Ok(outcome)
}

/// Retires a terminal prepared move and exposes its stable presentation location.
#[cfg(test)]
fn finish(cx: &mut App, prepared: &PreparedRehost) -> Result<FinishOutcome, TransitionError> {
    let outcome = cx.view_presentation_windows.finish(prepared)?;
    match &outcome {
        FinishOutcome::Destination { batch, .. } | FinishOutcome::Source(batch) => {
            refresh_window(cx, batch.window_id)
        }
        FinishOutcome::Invalidated(_) => {}
    }
    Ok(outcome)
}

/// Atomically finishes one accepted source-restoration batch or converts drift into authority
/// loss. Repeating the operation after its exact successful commit is idempotent.
#[doc(hidden)]
pub fn finish_rendered_rehost_source(
    cx: &mut App,
    prepared: &PreparedRehost,
    source: &LeaseBatch,
) -> Result<SourcePresentationFinish, TransitionError> {
    if cx
        .view_presentation_windows
        .source_finish_is_committed(prepared, source)
    {
        return Ok(SourcePresentationFinish::Finished(source.clone()));
    }
    let outcome = cx
        .view_presentation_windows
        .finish_source_or_release_authority(prepared, source, &mut cx.current_window_by_entity)?;
    match outcome {
        FinishSourceOutcome::Finished(FinishOutcome::Source(batch)) => {
            refresh_window(cx, batch.window_id());
            Ok(SourcePresentationFinish::Finished(batch))
        }
        FinishSourceOutcome::Finished(FinishOutcome::Invalidated(invalidation)) => {
            debug_assert_eq!(prepared.snapshot().invalidation(), Some(invalidation));
            refresh_window(cx, source.window_id());
            Ok(SourcePresentationFinish::Finished(source.clone()))
        }
        FinishSourceOutcome::PresentationAuthorityReleased(invalidation) => Ok(
            SourcePresentationFinish::PresentationAuthorityReleased(invalidation),
        ),
        FinishSourceOutcome::Finished(FinishOutcome::Destination { .. }) => {
            Err(TransitionError::StalePrepared)
        }
    }
}

/// Returns exact evidence after every root in one authoritative batch mounted in the same visible
/// candidate frame.
///
/// A destination staging frame is deliberately ineligible. Finishing a prepared rehost clears its
/// staging mount observation, so callers must wait for a later accepted presented frame.
#[doc(hidden)]
pub fn presented_batch_receipt(cx: &App, batch: &LeaseBatch) -> Option<PresentedBatchReceipt> {
    cx.view_presentation_windows.presented_batch_receipt(batch)
}

/// Returns exact accepted-frame evidence for one stable, non-staging presentation batch.
#[doc(hidden)]
pub fn stable_batch_presentation_receipt(
    cx: &App,
    batch: &LeaseBatch,
) -> Option<StableBatchPresentationReceipt> {
    cx.view_presentation_windows
        .stable_batch_presentation_receipt(batch)
}

fn commit_source_release(
    cx: &mut App,
    prepared: &PreparedRehost,
    receipt: SourceProxyReplayReceipt,
) -> Result<(), TransitionError> {
    let destination = cx.view_presentation_windows.commit_source_release(
        prepared,
        receipt,
        &mut cx.current_window_by_entity,
    )?;
    refresh_window(cx, destination);
    Ok(())
}

fn commit_mount(cx: &mut App, lease: Lease, frame_generation: u64) {
    let result = cx.view_presentation_windows.commit_mount(
        lease,
        frame_generation,
        &mut cx.current_window_by_entity,
    );
    match result {
        Ok(
            MountCommitOutcome::Stable
            | MountCommitOutcome::AwaitingBatch
            | MountCommitOutcome::BatchCompleted,
        ) => {}
        Err(_) => {
            refresh_window(cx, lease.window_id);
            if let Some(authoritative_window) = cx
                .view_presentation_windows
                .resolved_window(lease.entity_id)
                && authoritative_window != lease.window_id
            {
                refresh_window(cx, authoritative_window);
            }
        }
    }
}

fn discard_mount(cx: &mut App, window_id: WindowId, frame_generation: u64) {
    let _ = cx
        .view_presentation_windows
        .begin_accepted_presentation_frame(window_id, frame_generation);
}

fn record_mount_transaction(window: &mut Window, cx: &App, lease: Lease) {
    let Some(publication) = cx
        .view_presentation_windows
        .presentation_publication(lease, window.handle.window_id())
    else {
        return;
    };
    window.record_prepaint_window_transaction(
        publication,
        move |accepted, _, cx| {
            debug_assert_eq!(accepted.window_id(), lease.window_id);
            commit_mount(cx, lease, accepted.generation());
        },
        move |accepted, _, cx| {
            discard_mount(cx, accepted.window_id(), accepted.generation());
        },
    );
}

fn refresh_window(cx: &mut App, window_id: WindowId) {
    let Some(handle) = cx.window_handles.get(&window_id).copied() else {
        return;
    };
    cx.defer(move |cx| {
        let _ = handle.update(cx, |_, window, _| window.refresh());
    });
}

/// Layout-neutral element that renders a view only under its exact current lease.
#[doc(hidden)]
pub struct PresentedAnyView {
    view: Option<AnyView>,
    lease: Lease,
    element_id: ElementId,
    source: &'static core::panic::Location<'static>,
}

#[doc(hidden)]
pub struct PresentedRequestState {
    child: AnyElement,
    admission: PresentationAdmission,
}

/// Wraps a view root with exact presentation-window admission.
#[doc(hidden)]
#[track_caller]
pub fn present(view: AnyView, lease: Lease) -> PresentedAnyView {
    PresentedAnyView {
        element_id: ElementId::from(format!(
            "presented-view:{}:{}",
            lease.entity_id, lease.generation
        )),
        view: Some(view),
        lease,
        source: core::panic::Location::caller(),
    }
}

impl Element for PresentedAnyView {
    type RequestLayoutState = PresentedRequestState;
    type PrepaintState = ();

    fn id(&self) -> Option<ElementId> {
        Some(self.element_id.clone())
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        Some(self.source)
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let entity_matches = self.lease.entity_id
            == self
                .view
                .as_ref()
                .map(AnyView::entity_id)
                .unwrap_or(self.lease.entity_id);
        let admission = if entity_matches {
            cx.view_presentation_windows
                .presentation_admission(self.lease, window.handle.window_id())
        } else {
            PresentationAdmission::Rejected
        };
        let mut child = if admission != PresentationAdmission::Rejected {
            self.view
                .take()
                .expect("presented view child missing")
                .into_any_element()
        } else {
            self.view.take();
            Empty.into_any_element()
        };
        let layout_id = if admission == PresentationAdmission::Staging {
            window.with_subtree_presentation(crate::SubtreePresentation::Hidden, |window| {
                child.request_layout(window, cx)
            })
        } else {
            child.request_layout(window, cx)
        };
        (layout_id, PresentedRequestState { child, admission })
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        state: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) {
        state.admission = cx
            .view_presentation_windows
            .presentation_admission(self.lease, window.handle.window_id());
        match state.admission {
            PresentationAdmission::Rejected => {}
            PresentationAdmission::Staging => {
                window.with_subtree_presentation(crate::SubtreePresentation::Hidden, |window| {
                    state.child.prepaint(window, cx);
                    record_mount_transaction(window, cx, self.lease);
                });
            }
            PresentationAdmission::Presented => {
                state.child.prepaint(window, cx);
                record_mount_transaction(window, cx, self.lease);
            }
        }
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        state: &mut Self::RequestLayoutState,
        _prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        if state.admission == PresentationAdmission::Presented
            && cx
                .view_presentation_windows
                .presentation_admission(self.lease, window.handle.window_id())
                == PresentationAdmission::Presented
        {
            state.child.paint(window, cx);
        }
    }
}

impl IntoElement for PresentedAnyView {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

/// Records that the source proxy replay completed successfully in the frame being painted.
///
/// Call this only after the source's retained visual or equivalent proxy content has replayed
/// without error. The release barrier consumes only an exact same-frame receipt.
#[doc(hidden)]
pub fn source_proxy_replay_succeeded(
    attempt: &SourceProxyReplayAttempt,
    window: &Window,
) -> Result<SourceProxyReplayReceipt, TransitionError> {
    if attempt.expected_evidence != SourceProxyEvidenceRequirement::FrameworkPainted {
        return Err(TransitionError::WrongSourceProxyEvidence);
    }
    let source_window = window.handle.window_id();
    if source_window != attempt.source_window {
        return Err(TransitionError::WrongWindow);
    }
    let receipt = SourceProxyReplayReceipt {
        rehost_generation: attempt.rehost_generation,
        source_window,
        frame_generation: window.preparing_frame_generation(),
        evidence: SourceProxyEvidence::FrameworkPainted,
    };
    *attempt.receipt.lock() = Some(receipt);
    Ok(receipt)
}

/// Records that an exact retained visual supplied the source proxy in the candidate frame.
///
/// Unlike [`source_proxy_replay_succeeded`], this binds the rehost barrier to the immutable visual
/// lease and source frame that were actually replayed. The barrier still releases authority only
/// after the candidate frame commits.
#[doc(hidden)]
pub fn retained_visual_source_proxy_replay_succeeded(
    attempt: &SourceProxyReplayAttempt,
    replay: crate::window::retained_visual::ReplayReceipt,
    window: &Window,
) -> Result<SourceProxyReplayReceipt, TransitionError> {
    if replay.source_window() != attempt.source_window {
        return Err(TransitionError::WrongWindow);
    }
    if !replay.matches_candidate(window) {
        return Err(TransitionError::StaleCandidateFrameAttempt);
    }
    let evidence = SourceProxyEvidence::RetainedVisual(replay);
    if !attempt.expected_evidence.accepts(evidence) {
        return Err(TransitionError::WrongSourceProxyEvidence);
    }
    let receipt = SourceProxyReplayReceipt {
        rehost_generation: attempt.rehost_generation,
        source_window: replay.source_window(),
        frame_generation: replay.replay_frame_generation(),
        evidence,
    };
    *attempt.receipt.lock() = Some(receipt);
    Ok(receipt)
}

/// Layout-neutral source proxy barrier that releases a prepared rehost only after frame commit.
#[doc(hidden)]
pub struct SourceReleaseBarrier {
    prepared: PreparedRehost,
    expected_evidence: SourceProxyEvidenceRequirement,
    replay_receipt: Arc<Mutex<Option<SourceProxyReplayReceipt>>>,
    child: Option<AnyElement>,
    element_id: ElementId,
    source: &'static core::panic::Location<'static>,
}

/// Wraps the source proxy/frozen visual in the exact accepted-frame release barrier.
///
/// The child builder receives one candidate-local [`SourceProxyReplayAttempt`] and must pass it
/// to [`source_proxy_replay_succeeded`] after replay succeeds. Merely painting the barrier never
/// releases source authority.
#[doc(hidden)]
#[track_caller]
pub fn source_release_barrier<E>(
    prepared: PreparedRehost,
    child: impl FnOnce(SourceProxyReplayAttempt) -> E,
) -> SourceReleaseBarrier
where
    E: IntoElement,
{
    source_release_barrier_with_requirement(
        prepared,
        SourceProxyEvidenceRequirement::FrameworkPainted,
        child,
    )
}

/// Wraps a retained-visual proxy in a barrier pre-bound to its exact lease identity.
#[doc(hidden)]
#[track_caller]
pub fn retained_visual_source_release_barrier<E>(
    prepared: PreparedRehost,
    ticket: &crate::window::retained_visual::Ticket,
    child: impl FnOnce(SourceProxyReplayAttempt) -> E,
) -> SourceReleaseBarrier
where
    E: IntoElement,
{
    source_release_barrier_with_requirement(
        prepared,
        SourceProxyEvidenceRequirement::RetainedVisual(ticket.identity()),
        child,
    )
}

fn source_release_barrier_with_requirement<E>(
    prepared: PreparedRehost,
    expected_evidence: SourceProxyEvidenceRequirement,
    child: impl FnOnce(SourceProxyReplayAttempt) -> E,
) -> SourceReleaseBarrier
where
    E: IntoElement,
{
    let replay_receipt = Arc::new(Mutex::new(None));
    let replay_attempt = SourceProxyReplayAttempt {
        rehost_generation: prepared.generation,
        source_window: prepared.source.window_id,
        expected_evidence,
        receipt: replay_receipt.clone(),
    };
    SourceReleaseBarrier {
        element_id: ElementId::from(format!(
            "view-presentation-source-release:{}",
            prepared.generation
        )),
        prepared,
        expected_evidence,
        replay_receipt,
        child: Some(child(replay_attempt).into_any_element()),
        source: core::panic::Location::caller(),
    }
}

impl Element for SourceReleaseBarrier {
    type RequestLayoutState = AnyElement;
    type PrepaintState = ();

    fn id(&self) -> Option<ElementId> {
        Some(self.element_id.clone())
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        Some(self.source)
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let mut child = self
            .child
            .take()
            .expect("view presentation source proxy child missing");
        let layout_id = child.request_layout(window, cx);
        (layout_id, child)
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        child: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) {
        child.prepaint(window, cx);
        let prepared = self.prepared.clone();
        let expected_evidence = self.expected_evidence;
        let replay_receipt = self.replay_receipt.clone();
        let source_window = window.handle.window_id();
        window.record_prepaint_focus_stable_commit(move |frame_generation, _, cx| {
            let Some(receipt) = replay_receipt.lock().take() else {
                return;
            };
            if receipt.rehost_generation != prepared.generation
                || receipt.source_window != source_window
                || receipt.frame_generation != frame_generation
                || !expected_evidence.accepts(receipt.evidence)
            {
                return;
            }
            let _ = commit_source_release(cx, &prepared, receipt);
        });
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        child: &mut Self::RequestLayoutState,
        _prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        child.paint(window, cx);
    }
}

impl IntoElement for SourceReleaseBarrier {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        AnyWindowHandle, AtlasKey, AtlasTextureId, AtlasTextureInstanceId, AtlasTextureKind,
        AtlasTextureLeaseEpoch, AtlasTextureLeaseError, AtlasTile, Context, DevicePixels,
        HeadlessAppContext, ImageSource, InteractiveElement as _, Modifiers, MouseButton,
        MouseDownEvent, MouseUpEvent, NoopTextSystem, ParentElement as _, PlatformAtlas,
        PlatformHeadlessRenderer, PlatformInput, Render, RenderImage, Scene, Size, Styled as _,
        TestAppContext, TileId, canvas, div, img, point, px, red, size,
    };
    use std::{
        borrow::Cow,
        cell::{Cell, RefCell},
        rc::Rc,
    };

    fn prepare_rehost(
        cx: &mut App,
        source: &[Lease],
        destination_window: WindowId,
    ) -> Result<PreparedRehost, PrepareError> {
        if !cx.window_handles.contains_key(&destination_window) {
            return Err(PrepareError::DestinationUnavailable);
        }
        cx.view_presentation_windows
            .prepare(source, destination_window)
    }

    struct PresentationProbe {
        rendered_in: Rc<RefCell<Vec<WindowId>>>,
    }

    impl Render for PresentationProbe {
        fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            let rendered_in = self.rendered_in.clone();
            div().size_full().bg(red()).child(canvas(
                |_, _, _| (),
                move |_, _, window, _| {
                    rendered_in.borrow_mut().push(window.handle.window_id());
                },
            ))
        }
    }

    #[derive(Clone)]
    enum HostMode {
        Empty,
        Presented(Lease),
        Releasing(PreparedRehost),
        ReleasingWithAtlasImage {
            prepared: PreparedRehost,
            image: Arc<RenderImage>,
        },
        UnprovenRelease(PreparedRehost),
        StaleRelease(PreparedRehost),
        ConflictingRelease {
            lease: Lease,
            prepared: PreparedRehost,
            barrier_first: bool,
        },
    }

    struct PresentationHost {
        view: AnyView,
        mode: HostMode,
    }

    impl Render for PresentationHost {
        fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            match &self.mode {
                HostMode::Empty => Empty.into_any_element(),
                HostMode::Presented(lease) => present(self.view.clone(), *lease).into_any_element(),
                HostMode::Releasing(prepared) => {
                    source_release_barrier(prepared.clone(), successful_source_proxy)
                        .into_any_element()
                }
                HostMode::ReleasingWithAtlasImage { prepared, image } => div()
                    .size_full()
                    .children([
                        source_release_barrier(prepared.clone(), successful_source_proxy)
                            .into_any_element(),
                        img(ImageSource::Render(image.clone()))
                            .size_full()
                            .into_any_element(),
                    ])
                    .into_any_element(),
                HostMode::UnprovenRelease(prepared) => {
                    source_release_barrier(prepared.clone(), |_| div().size_full())
                        .into_any_element()
                }
                HostMode::StaleRelease(prepared) => {
                    source_release_barrier(prepared.clone(), |attempt| {
                        *attempt.receipt.lock() = Some(SourceProxyReplayReceipt {
                            rehost_generation: attempt.rehost_generation,
                            source_window: attempt.source_window,
                            frame_generation: u64::MAX,
                            evidence: SourceProxyEvidence::FrameworkPainted,
                        });
                        div().size_full()
                    })
                    .into_any_element()
                }
                HostMode::ConflictingRelease {
                    lease,
                    prepared,
                    barrier_first,
                } => {
                    let source = present(self.view.clone(), *lease).into_any_element();
                    let proxy = source_release_barrier(prepared.clone(), successful_source_proxy)
                        .into_any_element();
                    let children = if *barrier_first {
                        vec![proxy, source]
                    } else {
                        vec![source, proxy]
                    };
                    div().size_full().children(children).into_any_element()
                }
            }
        }
    }

    struct RejectOnceAtlasState {
        reject_next_lease: bool,
        rejection_count: usize,
    }

    struct RejectOnceAtlas(Mutex<RejectOnceAtlasState>);

    impl RejectOnceAtlas {
        fn new() -> Self {
            Self(Mutex::new(RejectOnceAtlasState {
                reject_next_lease: false,
                rejection_count: 0,
            }))
        }

        fn reject_next_lease(&self) {
            self.0.lock().reject_next_lease = true;
        }

        fn rejection_count(&self) -> usize {
            self.0.lock().rejection_count
        }

        fn tile(kind: AtlasTextureKind) -> AtlasTile {
            AtlasTile {
                texture_id: AtlasTextureId { index: 1, kind },
                tile_id: TileId(1),
                padding: 0,
                bounds: Bounds::new(Default::default(), size(DevicePixels(1), DevicePixels(1))),
                texture_generation: 1,
                texture_generation_padding: 0,
            }
        }
    }

    impl PlatformAtlas for RejectOnceAtlas {
        fn get_or_insert_with<'a>(
            &self,
            key: &AtlasKey,
            _build: &mut dyn FnMut() -> anyhow::Result<Option<(Size<DevicePixels>, Cow<'a, [u8]>)>>,
        ) -> anyhow::Result<Option<AtlasTile>> {
            Ok(Some(Self::tile(key.texture_kind())))
        }

        fn remove(&self, _key: &AtlasKey) {}

        fn atlas_texture_lease_epoch(&self) -> AtlasTextureLeaseEpoch {
            AtlasTextureLeaseEpoch::INITIAL
        }

        unsafe fn acquire_atlas_texture_leases(
            &self,
            textures: &[AtlasTextureInstanceId],
        ) -> Result<AtlasTextureLeaseEpoch, AtlasTextureLeaseError> {
            let mut state = self.0.lock();
            if state.reject_next_lease {
                state.reject_next_lease = false;
                state.rejection_count += 1;
                return Err(AtlasTextureLeaseError::TextureUnavailable {
                    texture: *textures
                        .first()
                        .expect("an image lease must name one texture instance"),
                    epoch: AtlasTextureLeaseEpoch::INITIAL,
                });
            }
            Ok(AtlasTextureLeaseEpoch::INITIAL)
        }

        unsafe fn release_atlas_texture_leases(
            &self,
            _epoch: AtlasTextureLeaseEpoch,
            _textures: &[AtlasTextureInstanceId],
        ) {
        }
    }

    struct AtlasTestRenderer {
        atlas: Arc<RejectOnceAtlas>,
    }

    impl PlatformHeadlessRenderer for AtlasTestRenderer {
        fn render_scene_to_image(
            &mut self,
            _scene: &Scene,
            size: Size<DevicePixels>,
        ) -> anyhow::Result<image::RgbaImage> {
            Ok(image::RgbaImage::new(
                size.width.0 as u32,
                size.height.0 as u32,
            ))
        }

        fn sprite_atlas(&self) -> Arc<dyn PlatformAtlas> {
            self.atlas.clone()
        }
    }

    struct InteractivePresentationProbe {
        paints: Rc<Cell<usize>>,
        clicks: Rc<Cell<usize>>,
    }

    impl Render for InteractivePresentationProbe {
        fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            let paints = self.paints.clone();
            let clicks = self.clicks.clone();
            div()
                .w(px(100.0))
                .h(px(100.0))
                .on_mouse_down(MouseButton::Left, move |_, _, _| {
                    clicks.set(clicks.get() + 1);
                })
                .child(
                    canvas(|_, _, _| (), move |_, _, _, _| paints.set(paints.get() + 1))
                        .size_full(),
                )
        }
    }

    #[derive(Clone)]
    enum BatchHostMode {
        Empty,
        Presented(Vec<(AnyView, Lease)>),
        Releasing(PreparedRehost),
    }

    struct BatchPresentationHost {
        mode: BatchHostMode,
    }

    impl Render for BatchPresentationHost {
        fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            match &self.mode {
                BatchHostMode::Empty => Empty.into_any_element(),
                BatchHostMode::Presented(roots) => div()
                    .flex()
                    .size_full()
                    .children(
                        roots
                            .iter()
                            .cloned()
                            .map(|(view, lease)| present(view, lease)),
                    )
                    .into_any_element(),
                BatchHostMode::Releasing(prepared) => {
                    source_release_barrier(prepared.clone(), successful_source_proxy)
                        .into_any_element()
                }
            }
        }
    }

    fn set_host_mode(
        cx: &mut TestAppContext,
        window: crate::WindowHandle<PresentationHost>,
        mode: HostMode,
    ) {
        window
            .update(cx, |host, _, _| host.mode = mode)
            .expect("test presentation window should remain open");
    }

    fn set_batch_host_mode(
        cx: &mut TestAppContext,
        window: crate::WindowHandle<BatchPresentationHost>,
        mode: BatchHostMode,
    ) {
        window
            .update(cx, |host, _, _| host.mode = mode)
            .expect("test batch presentation window should remain open");
    }

    fn draw_window(cx: &mut TestAppContext, window: AnyWindowHandle) -> u64 {
        cx.update_window(window, |_, window, cx| {
            window.draw(cx).clear();
            window.rendered_frame_revision()
        })
        .expect("test presentation window should remain open")
    }

    fn draw_headless_window(cx: &mut HeadlessAppContext, window: AnyWindowHandle) -> u64 {
        cx.update_window(window, |_, window, cx| {
            window.draw(cx).clear();
            window.rendered_frame_revision()
        })
        .expect("test headless presentation window should remain open")
    }

    fn resolved_window(cx: &mut TestAppContext, entity_id: EntityId) -> Option<WindowId> {
        cx.update(|cx| cx.with_window(entity_id, |window, _| window.handle.window_id()))
    }

    fn resolved_headless_window(
        cx: &mut HeadlessAppContext,
        entity_id: EntityId,
    ) -> Option<WindowId> {
        cx.update(|cx| cx.with_window(entity_id, |window, _| window.handle.window_id()))
    }

    fn leased_roots(views: &[AnyView], batch: &LeaseBatch) -> Vec<(AnyView, Lease)> {
        views
            .iter()
            .cloned()
            .map(|view| {
                let lease = batch
                    .lease_for(view.entity_id())
                    .expect("test batch should contain every presented root");
                (view, lease)
            })
            .collect()
    }

    fn dispatch_primary_click(
        cx: &mut TestAppContext,
        window: AnyWindowHandle,
        position: crate::Point<Pixels>,
    ) {
        cx.update_window(window, |_, window, cx| {
            let _ = window.dispatch_event(
                PlatformInput::MouseDown(MouseDownEvent {
                    button: MouseButton::Left,
                    position,
                    modifiers: Modifiers::none(),
                    click_count: 1,
                    first_mouse: false,
                }),
                cx,
            );
            let _ = window.dispatch_event(
                PlatformInput::MouseUp(MouseUpEvent {
                    button: MouseButton::Left,
                    position,
                    modifiers: Modifiers::none(),
                    click_count: 1,
                }),
                cx,
            );
        })
        .expect("test presentation window should remain open");
    }

    fn source_proxy_receipt(
        prepared: &PreparedRehost,
        source_window: WindowId,
        frame_generation: u64,
    ) -> SourceProxyReplayReceipt {
        SourceProxyReplayReceipt {
            rehost_generation: prepared.generation,
            source_window,
            frame_generation,
            evidence: SourceProxyEvidence::FrameworkPainted,
        }
    }

    fn successful_source_proxy(attempt: SourceProxyReplayAttempt) -> impl IntoElement {
        canvas(
            |_, _, _| (),
            move |_, _, window, _| {
                source_proxy_replay_succeeded(&attempt, window)
                    .expect("test source proxy replay should be accepted");
            },
        )
    }

    fn probe_entity_id(cx: &mut TestAppContext) -> EntityId {
        cx.update(|cx| {
            cx.new(|_| PresentationProbe {
                rendered_in: Rc::new(RefCell::new(Vec::new())),
            })
        })
        .entity_id()
    }

    struct ExposedRehostFixture {
        source: WindowId,
        destination: WindowId,
        entity_id: EntityId,
        registry: Registry,
        current_windows: FxHashMap<EntityId, WindowId>,
        prepared: PreparedRehost,
        destination_lease: Lease,
        exposure: DestinationExposureReceipt,
    }

    fn exposed_rehost_fixture(cx: &mut TestAppContext) -> ExposedRehostFixture {
        let source = cx
            .open_window(size(px(100.0), px(100.0)), |_, _| Empty)
            .window_id();
        let destination = cx
            .open_window(size(px(100.0), px(100.0)), |_, _| Empty)
            .window_id();
        let entity_id = probe_entity_id(cx);
        let mut registry = Registry::default();
        let mut current_windows = FxHashMap::from_iter([(entity_id, source)]);
        let source_lease = registry.claim(entity_id, source).unwrap();
        registry
            .commit_mount(source_lease, 1, &mut current_windows)
            .unwrap();
        let prepared = registry.prepare(&[source_lease], destination).unwrap();
        registry
            .commit_source_release(
                &prepared,
                source_proxy_receipt(&prepared, source, 2),
                &mut current_windows,
            )
            .unwrap();
        let destination_lease = prepared.destination().lease_for(entity_id).unwrap();
        registry
            .commit_mount(destination_lease, 3, &mut current_windows)
            .unwrap();
        let exposed = registry.expose_destination(&prepared).unwrap();
        registry
            .commit_mount(destination_lease, 4, &mut current_windows)
            .unwrap();
        ExposedRehostFixture {
            source,
            destination,
            entity_id,
            registry,
            current_windows,
            prepared,
            destination_lease,
            exposure: exposed.exposure,
        }
    }

    #[test]
    fn resolved_view_rehost_registry_rejects_empty_duplicate_and_same_window_inputs() {
        let mut cx = TestAppContext::single();
        let source = cx
            .open_window(size(px(100.0), px(100.0)), |_, _| Empty)
            .window_id();
        let destination = cx
            .open_window(size(px(100.0), px(100.0)), |_, _| Empty)
            .window_id();
        let entity_id = probe_entity_id(&mut cx);
        let mut registry = Registry::default();

        assert_eq!(
            registry
                .prepare_resolved_view_rehost(&[], source, destination)
                .unwrap_err(),
            ResolvedViewRehostError::Empty
        );
        assert_eq!(
            registry
                .prepare_resolved_view_rehost(&[entity_id, entity_id], source, destination,)
                .unwrap_err(),
            ResolvedViewRehostError::DuplicateEntity(entity_id)
        );
        assert_eq!(
            registry
                .prepare_resolved_view_rehost(&[entity_id], source, source)
                .unwrap_err(),
            ResolvedViewRehostError::SameWindow
        );
        assert!(registry.bindings.is_empty());
        assert!(registry.rehosts.is_empty());
    }

    #[test]
    fn resolved_view_rehost_registry_prepares_only_the_exact_source_bound_subset() {
        let mut cx = TestAppContext::single();
        let source = cx
            .open_window(size(px(100.0), px(100.0)), |_, _| Empty)
            .window_id();
        let destination = cx
            .open_window(size(px(100.0), px(100.0)), |_, _| Empty)
            .window_id();
        let source_bound = probe_entity_id(&mut cx);
        let ungoverned = probe_entity_id(&mut cx);
        let already_destination = probe_entity_id(&mut cx);
        let mut registry = Registry::default();
        let mut current_windows = FxHashMap::default();
        let source_lease = registry.claim(source_bound, source).unwrap();
        let destination_lease = registry.claim(already_destination, destination).unwrap();
        registry
            .commit_mount(source_lease, 1, &mut current_windows)
            .unwrap();
        registry
            .commit_mount(destination_lease, 1, &mut current_windows)
            .unwrap();

        let outcome = registry
            .prepare_resolved_view_rehost(
                &[source_bound, ungoverned, already_destination],
                source,
                destination,
            )
            .expect("the exact source subset should prepare");
        let ResolvedPreparedViewRehostOutcome::Prepared(prepared) = outcome else {
            panic!("the source-bound root should require a prepared rehost");
        };

        assert_eq!(prepared.source().leases(), &[source_lease]);
        assert_eq!(prepared.source().window_id(), source);
        assert_eq!(prepared.destination().window_id(), destination);
        assert_eq!(prepared.destination().leases().len(), 1);
        assert!(prepared.destination().lease_for(source_bound).is_some());
        assert!(!registry.governs(ungoverned));
        let binding = registry
            .bindings
            .get(&already_destination)
            .expect("the destination-owned root must remain bound");
        assert_eq!(binding.current, destination_lease);
        assert_eq!(binding.last_mounted_frame, Some(1));
        assert_eq!(binding.pending_rehost, None);
        assert_eq!(binding.destination_exposure, None);
    }

    #[test]
    fn resolved_view_rehost_registry_fails_closed_on_third_window_drift() {
        let mut cx = TestAppContext::single();
        let source = cx
            .open_window(size(px(100.0), px(100.0)), |_, _| Empty)
            .window_id();
        let destination = cx
            .open_window(size(px(100.0), px(100.0)), |_, _| Empty)
            .window_id();
        let third = cx
            .open_window(size(px(100.0), px(100.0)), |_, _| Empty)
            .window_id();
        let exact_source = probe_entity_id(&mut cx);
        let drifted = probe_entity_id(&mut cx);
        let mut registry = Registry::default();
        let mut current_windows = FxHashMap::default();
        let exact_source_lease = registry.claim(exact_source, source).unwrap();
        let drifted_lease = registry.claim(drifted, third).unwrap();
        registry
            .commit_mount(exact_source_lease, 1, &mut current_windows)
            .unwrap();
        registry
            .commit_mount(drifted_lease, 1, &mut current_windows)
            .unwrap();

        assert_eq!(
            registry
                .prepare_resolved_view_rehost(&[exact_source, drifted], source, destination,)
                .unwrap_err(),
            ResolvedViewRehostError::UnexpectedWindow {
                current: drifted_lease,
            }
        );
        assert_eq!(
            registry
                .bindings
                .get(&exact_source)
                .and_then(|binding| binding.pending_rehost),
            None
        );
        assert!(registry.rehosts.is_empty());
    }

    #[test]
    fn resolved_view_rehost_registry_fails_closed_on_pending_rehost() {
        let mut cx = TestAppContext::single();
        let source = cx
            .open_window(size(px(100.0), px(100.0)), |_, _| Empty)
            .window_id();
        let first_destination = cx
            .open_window(size(px(100.0), px(100.0)), |_, _| Empty)
            .window_id();
        let recovery_destination = cx
            .open_window(size(px(100.0), px(100.0)), |_, _| Empty)
            .window_id();
        let entity_id = probe_entity_id(&mut cx);
        let mut registry = Registry::default();
        let mut current_windows = FxHashMap::default();
        let source_lease = registry.claim(entity_id, source).unwrap();
        registry
            .commit_mount(source_lease, 1, &mut current_windows)
            .unwrap();
        let existing = registry
            .prepare(&[source_lease], first_destination)
            .unwrap();

        assert_eq!(
            registry
                .prepare_resolved_view_rehost(&[entity_id], source, recovery_destination,)
                .unwrap_err(),
            ResolvedViewRehostError::RehostInFlight {
                current: source_lease,
            }
        );
        assert_eq!(registry.rehosts.len(), 1);
        assert!(registry.rehosts.contains_key(&existing.generation()));
    }

    #[test]
    fn resolved_view_rehost_public_interface_returns_no_transfer_without_claiming_roots() {
        let mut cx = TestAppContext::single();
        let destination_bound = cx.update(|cx| {
            cx.new(|_| PresentationProbe {
                rendered_in: Rc::new(RefCell::new(Vec::new())),
            })
        });
        let ungoverned = cx.update(|cx| {
            cx.new(|_| PresentationProbe {
                rendered_in: Rc::new(RefCell::new(Vec::new())),
            })
        });
        let views = [
            AnyView::from(destination_bound.clone()),
            AnyView::from(ungoverned.clone()),
        ];
        let source = cx
            .open_window(size(px(100.0), px(100.0)), |_, _| Empty)
            .window_id();
        let destination = cx
            .open_window(size(px(100.0), px(100.0)), |_, _| Empty)
            .window_id();
        let destination_lease =
            cx.update(|cx| claim(cx, &views[0], destination).expect("destination claim"));

        let outcome = cx.update(|cx| {
            prepare_resolved_view_rehost(cx, &views, source, destination)
                .expect("no root should require transfer")
        });
        assert!(matches!(outcome, ResolvedViewRehostOutcome::NoTransfer));
        cx.update(|cx| {
            assert_eq!(
                cx.view_presentation_windows
                    .bindings
                    .get(&destination_bound.entity_id())
                    .map(|binding| binding.current),
                Some(destination_lease)
            );
            assert!(!cx.view_presentation_windows.governs(ungoverned.entity_id()));
            assert!(cx.view_presentation_windows.rehosts.is_empty());
        });
    }

    #[test]
    fn resolved_view_rehost_public_interface_maps_views_to_the_prepared_subset() {
        let mut cx = TestAppContext::single();
        let source_bound = cx.update(|cx| {
            cx.new(|_| PresentationProbe {
                rendered_in: Rc::new(RefCell::new(Vec::new())),
            })
        });
        let ungoverned = cx.update(|cx| {
            cx.new(|_| PresentationProbe {
                rendered_in: Rc::new(RefCell::new(Vec::new())),
            })
        });
        let views = [
            AnyView::from(source_bound.clone()),
            AnyView::from(ungoverned.clone()),
        ];
        let source = cx
            .open_window(size(px(100.0), px(100.0)), |_, _| Empty)
            .window_id();
        let destination = cx
            .open_window(size(px(100.0), px(100.0)), |_, _| Empty)
            .window_id();
        let source_lease = cx.update(|cx| claim(cx, &views[0], source).expect("source claim"));
        cx.update(|cx| {
            cx.view_presentation_windows
                .commit_mount(source_lease, 1, &mut cx.current_window_by_entity)
                .expect("source mount");
        });

        let outcome = cx.update(|cx| {
            prepare_resolved_view_rehost(cx, &views, source, destination)
                .expect("the exact source-bound root should prepare")
        });
        let ResolvedViewRehostOutcome::Prepared(prepared) = outcome else {
            panic!("the source-bound root should require transfer");
        };
        assert_eq!(prepared.source().leases(), &[source_lease]);
        assert_eq!(prepared.destination().leases().len(), 1);
        assert!(
            prepared
                .destination()
                .lease_for(source_bound.entity_id())
                .is_some()
        );
        cx.update(|cx| {
            assert!(!cx.view_presentation_windows.governs(ungoverned.entity_id()));
        });
    }

    #[test]
    fn dropping_an_unreleased_rehost_session_restores_stable_source_authority() {
        let mut cx = TestAppContext::single();
        let panel = cx.update(|cx| {
            cx.new(|_| PresentationProbe {
                rendered_in: Rc::new(RefCell::new(Vec::new())),
            })
        });
        let panel_view = AnyView::from(panel.clone());
        let source = cx
            .open_window(size(px(100.0), px(100.0)), |_, _| Empty)
            .window_id();
        let destination = cx
            .open_window(size(px(100.0), px(100.0)), |_, _| Empty)
            .window_id();
        let source_lease = cx.update(|cx| claim(cx, &panel_view, source).expect("source claim"));
        cx.update(|cx| {
            cx.view_presentation_windows
                .commit_mount(source_lease, 1, &mut cx.current_window_by_entity)
                .expect("source mount");
        });
        let session = cx.update(|cx| {
            super::prepare_rehost(cx, &[source_lease], destination).expect("prepared rehost")
        });
        let generation = session.generation();

        drop(session);
        cx.update(|_| {});

        cx.update(|cx| {
            assert!(
                !cx.view_presentation_windows
                    .rehosts
                    .contains_key(&generation)
            );
            let binding = cx
                .view_presentation_windows
                .bindings
                .get(&panel.entity_id())
                .expect("source authority must remain registered");
            assert_eq!(binding.current, source_lease);
            assert_eq!(binding.pending_rehost, None);
            assert_eq!(
                cx.current_window_by_entity.get(&panel.entity_id()),
                Some(&source)
            );
        });

        let retry = cx.update(|cx| {
            super::prepare_rehost(cx, &[source_lease], destination)
                .expect("released session must not block a later rehost")
        });
        assert_ne!(retry.generation(), generation);
        drop(retry);
        cx.update(|_| {});
    }

    #[test]
    fn dropping_a_released_rehost_session_relinquishes_unrendered_restore_authority() {
        let mut cx = TestAppContext::single();
        let panel = cx.update(|cx| {
            cx.new(|_| PresentationProbe {
                rendered_in: Rc::new(RefCell::new(Vec::new())),
            })
        });
        let panel_view = AnyView::from(panel.clone());
        let source = cx
            .open_window(size(px(100.0), px(100.0)), |_, _| Empty)
            .window_id();
        let destination = cx
            .open_window(size(px(100.0), px(100.0)), |_, _| Empty)
            .window_id();
        let source_lease = cx.update(|cx| claim(cx, &panel_view, source).expect("source claim"));
        cx.update(|cx| {
            cx.view_presentation_windows
                .commit_mount(source_lease, 1, &mut cx.current_window_by_entity)
                .expect("source mount");
        });
        let session = cx.update(|cx| {
            super::prepare_rehost(cx, &[source_lease], destination).expect("prepared rehost")
        });
        let generation = session.generation();
        cx.update(|cx| {
            cx.view_presentation_windows
                .commit_source_release(
                    &session.prepared,
                    source_proxy_receipt(&session.prepared, source, 2),
                    &mut cx.current_window_by_entity,
                )
                .expect("source release");
        });

        drop(session);
        cx.update(|_| {});

        cx.update(|cx| {
            assert!(
                !cx.view_presentation_windows
                    .rehosts
                    .contains_key(&generation)
            );
            assert!(!cx.view_presentation_windows.governs(panel.entity_id()));
            assert!(!cx.current_window_by_entity.contains_key(&panel.entity_id()));
        });
        let replacement =
            cx.update(|cx| claim(cx, &panel_view, source).expect("fresh source claim"));
        assert_ne!(replacement.generation(), source_lease.generation());
    }

    #[test]
    fn accepted_source_release_switches_window_authority_before_destination_mount() {
        let mut cx = TestAppContext::single();
        let rendered_in = Rc::new(RefCell::new(Vec::new()));
        let panel = cx.update({
            let rendered_in = rendered_in.clone();
            move |cx| cx.new(|_| PresentationProbe { rendered_in })
        });
        let panel_view = AnyView::from(panel.clone());
        let source_window = cx.open_window(size(px(320.0), px(200.0)), {
            let panel_view = panel_view.clone();
            move |_, _| PresentationHost {
                view: panel_view,
                mode: HostMode::Empty,
            }
        });
        let destination_window = cx.open_window(size(px(320.0), px(200.0)), {
            let panel_view = panel_view.clone();
            move |_, _| PresentationHost {
                view: panel_view,
                mode: HostMode::Empty,
            }
        });
        let source_id = source_window.window_id();
        let destination_id = destination_window.window_id();
        let lease = cx.update(|cx| claim(cx, &panel_view, source_id).unwrap());

        set_host_mode(&mut cx, source_window, HostMode::Presented(lease));
        draw_window(&mut cx, source_window.into());
        assert_eq!(resolved_window(&mut cx, panel.entity_id()), Some(source_id));

        let prepared = cx.update(|cx| prepare_rehost(cx, &[lease], destination_id).unwrap());
        let destination_lease = prepared
            .destination()
            .lease_for(panel.entity_id())
            .expect("prepared batch should include the panel root");

        set_host_mode(
            &mut cx,
            destination_window,
            HostMode::Presented(destination_lease),
        );
        draw_window(&mut cx, destination_window.into());
        assert_eq!(
            rendered_in.borrow().last().copied(),
            Some(source_id),
            "a reserved destination lease must fail closed before source release"
        );
        assert_eq!(
            prepared.snapshot().phase(),
            RehostPhase::AwaitingSourceRelease
        );

        set_host_mode(&mut cx, destination_window, HostMode::Empty);
        set_host_mode(
            &mut cx,
            source_window,
            HostMode::Releasing(prepared.clone()),
        );
        draw_window(&mut cx, source_window.into());
        let released = prepared.snapshot();
        assert_eq!(released.phase(), RehostPhase::DestinationAdmitted);
        let receipt = released
            .source_proxy_receipt()
            .expect("accepted proxy replay must publish an exact receipt");
        assert_eq!(receipt.rehost_generation(), prepared.generation());
        assert_eq!(receipt.source_window(), source_id);
        assert_eq!(
            released.source_frame_generation(),
            Some(receipt.frame_generation())
        );
        assert_eq!(
            resolved_window(&mut cx, panel.entity_id()),
            Some(destination_id)
        );

        set_host_mode(&mut cx, source_window, HostMode::Presented(lease));
        draw_window(&mut cx, source_window.into());
        assert_eq!(
            rendered_in.borrow().last().copied(),
            Some(source_id),
            "the stale source lease must not render after authority switches"
        );

        set_host_mode(
            &mut cx,
            destination_window,
            HostMode::Presented(destination_lease),
        );
        let destination_frame = draw_window(&mut cx, destination_window.into());
        let mounted = prepared.snapshot();
        assert_eq!(mounted.phase(), RehostPhase::DestinationMounted);
        assert_eq!(
            mounted.destination_frame_generation(),
            Some(destination_frame)
        );
        let mount_receipt = mounted
            .destination_mount_receipt()
            .expect("mounted destination must publish an exact batch receipt");
        assert_eq!(mount_receipt.rehost_generation(), prepared.generation());
        assert_eq!(mount_receipt.destination_window(), destination_id);
        assert_eq!(
            mount_receipt.destination_lease_generation(),
            destination_lease.generation()
        );
        assert_eq!(mount_receipt.root_count(), 1);
        assert_eq!(mount_receipt.frame_generation(), destination_frame);
        assert!(
            cx.update(|cx| presented_batch_receipt(cx, prepared.destination()))
                .is_none(),
            "the hidden staging mount cannot masquerade as visible payload presentation"
        );
        assert_eq!(
            rendered_in.borrow().last().copied(),
            Some(source_id),
            "a mounted batch must remain non-presenting until finish exposes it"
        );

        let (finish_outcome, receipt_at_finish) = cx.update(|cx| {
            let outcome = finish(cx, &prepared).unwrap();
            let receipt = presented_batch_receipt(cx, prepared.destination());
            (outcome, receipt)
        });
        match finish_outcome {
            FinishOutcome::Destination { batch, exposure } => {
                assert_eq!(batch.window_id(), destination_id);
                assert_eq!(batch.lease_for(panel.entity_id()), Some(destination_lease));
                assert_eq!(exposure.mount(), mount_receipt);
            }
            outcome => panic!("unexpected rehost outcome: {outcome:?}"),
        }
        assert!(
            receipt_at_finish.is_none(),
            "finish must require a new accepted presented frame"
        );
        let presented_frame = draw_window(&mut cx, destination_window.into());
        assert_eq!(rendered_in.borrow().last().copied(), Some(destination_id));
        let presented = cx
            .update(|cx| presented_batch_receipt(cx, prepared.destination()))
            .expect("the exact destination batch should publish after visible presentation");
        assert_eq!(presented.window_id(), destination_id);
        assert_eq!(presented.lease_generation(), destination_lease.generation());
        assert_eq!(presented.root_count(), 1);
        assert_eq!(presented.frame_generation(), presented_frame);
    }

    #[test]
    fn deep_rehost_session_owns_frame_acceptance_and_terminal_commit() {
        let mut cx = TestAppContext::single();
        let panel = cx.update(|cx| {
            cx.new(|_| PresentationProbe {
                rendered_in: Rc::new(RefCell::new(Vec::new())),
            })
        });
        let panel_view = AnyView::from(panel.clone());
        let source_window = cx.open_window(size(px(320.0), px(200.0)), {
            let panel_view = panel_view.clone();
            move |_, _| PresentationHost {
                view: panel_view,
                mode: HostMode::Empty,
            }
        });
        let destination_window = cx.open_window(size(px(320.0), px(200.0)), {
            let panel_view = panel_view.clone();
            move |_, _| PresentationHost {
                view: panel_view,
                mode: HostMode::Empty,
            }
        });
        let source_id = source_window.window_id();
        let destination_id = destination_window.window_id();
        let source_lease = cx.update(|cx| claim(cx, &panel_view, source_id).unwrap());

        set_host_mode(&mut cx, source_window, HostMode::Presented(source_lease));
        draw_window(&mut cx, source_window.into());

        let mut session =
            cx.update(|cx| super::prepare_rehost(cx, &[source_lease], destination_id).unwrap());
        let projection = session.projection();
        let destination_lease = projection
            .destination()
            .lease_for(panel.entity_id())
            .expect("prepared projection should contain the destination root");

        set_host_mode(
            &mut cx,
            source_window,
            HostMode::Releasing(session.prepared.clone()),
        );
        let source_frame = draw_window(&mut cx, source_window.into());
        let source_commit = session
            .accept_source_proxy_frame(source_frame)
            .expect("the exact accepted source frame should be recognized");
        assert_eq!(source_commit.frame_generation(), source_frame);
        assert_eq!(
            session.accept_source_proxy_frame(source_frame.saturating_add(1)),
            Err(TransitionError::StaleCandidateFrameAttempt)
        );

        set_host_mode(
            &mut cx,
            destination_window,
            HostMode::Presented(destination_lease),
        );
        let staging_frame = draw_window(&mut cx, destination_window.into());
        let exposure = cx
            .update(|cx| session.accept_destination_frame(cx, staging_frame))
            .expect("the exact accepted staging frame should expose the destination");
        assert!(exposure.batch().matches_exactly(projection.destination()));
        assert_eq!(exposure.frame_generation(), staging_frame);
        let repeated_exposure = cx
            .update(|cx| session.accept_destination_frame(cx, staging_frame))
            .expect("the same accepted staging callback should be idempotent");
        assert!(repeated_exposure.batch().matches_exactly(exposure.batch()));
        assert_eq!(repeated_exposure.frame_generation(), staging_frame);
        assert!(matches!(
            cx.update(|cx| {
                session.accept_destination_frame(cx, staging_frame.saturating_add(1))
            }),
            Err(TransitionError::StaleCandidateFrameAttempt)
        ));

        let visible_frame = draw_window(&mut cx, destination_window.into());
        let stale_presentation = cx
            .update(|cx| session.accept_destination_presentation_frame(cx, visible_frame))
            .expect("the later visible frame should become terminal commit evidence");
        assert_eq!(stale_presentation.frame_generation(), visible_frame);

        set_host_mode(&mut cx, destination_window, HostMode::Empty);
        let omitted_frame = draw_window(&mut cx, destination_window.into());
        assert!(omitted_frame > visible_frame);
        assert_eq!(
            cx.update(|cx| session.current_destination_presentation(cx)),
            Err(TransitionError::WrongState)
        );
        assert!(matches!(
            cx.update(|cx| session.prepare_terminal(
                cx,
                RehostTerminalIntent::CommitDestination(&stale_presentation),
            )),
            Err(TransitionError::WrongState)
        ));

        set_host_mode(
            &mut cx,
            destination_window,
            HostMode::Presented(destination_lease),
        );
        let replacement_visible_frame = draw_window(&mut cx, destination_window.into());
        let presentation = cx
            .update(|cx| {
                session.accept_destination_presentation_frame(cx, replacement_visible_frame)
            })
            .expect("a later complete accepted frame should restore terminal evidence");

        let prepared = cx
            .update(|cx| {
                session.prepare_terminal(cx, RehostTerminalIntent::CommitDestination(&presentation))
            })
            .expect("the visible destination should prepare its final provider commit");
        let RehostTerminalPreparation::Prepared(prepared) = prepared else {
            panic!("destination commit cannot resolve as already absent");
        };
        assert!(cx.update(|cx| prepared.can_commit(cx)));
        drop(prepared);

        let prepared = cx
            .update(|cx| {
                session.prepare_terminal(cx, RehostTerminalIntent::CommitDestination(&presentation))
            })
            .expect("discarding a terminal token must leave the session retryable");
        assert!(cx.update(|cx| prepared.can_commit(cx)));
        let outcome = cx.update(|cx| prepared.try_commit(cx).unwrap());
        let RehostTerminalOutcome::DestinationCommitted(batch) = outcome else {
            panic!("destination terminal token must commit destination authority");
        };
        assert!(batch.matches_exactly(projection.destination()));
        assert_eq!(
            session.terminal_disposition(),
            Some(RehostTerminalDisposition::DestinationCommitted)
        );
        assert!(
            matches!(
                cx.update(|cx| session.accept_destination_frame(cx, staging_frame)),
                Err(TransitionError::ConflictingTerminalOutcome)
            ),
            "a cached reversible exposure must not replay after destination commit"
        );
        let replayed = cx
            .update(|cx| {
                session.prepare_terminal(cx, RehostTerminalIntent::CommitDestination(&presentation))
            })
            .expect("the exact committed destination terminal should be idempotent");
        let RehostTerminalPreparation::AlreadyCommitted(
            RehostTerminalOutcome::DestinationCommitted(replayed_batch),
        ) = replayed
        else {
            panic!("destination retry must replay only its exact terminal outcome")
        };
        assert!(replayed_batch.matches_exactly(projection.destination()));
        assert_eq!(
            cx.update(|cx| session.abandon_after_source_loss(cx)),
            Err(TransitionError::ConflictingTerminalOutcome)
        );
        assert_eq!(
            resolved_window(&mut cx, panel.entity_id()),
            Some(destination_id)
        );
    }

    #[test]
    fn source_release_waits_for_all_same_frame_mount_receipts() {
        for barrier_first in [false, true] {
            let mut cx = TestAppContext::single();
            let panel = cx.update(|cx| {
                cx.new(|_| PresentationProbe {
                    rendered_in: Rc::new(RefCell::new(Vec::new())),
                })
            });
            let panel_view = AnyView::from(panel.clone());
            let source_window = cx.open_window(size(px(320.0), px(200.0)), {
                let panel_view = panel_view.clone();
                move |_, _| PresentationHost {
                    view: panel_view,
                    mode: HostMode::Empty,
                }
            });
            let destination_window = cx.open_window(size(px(320.0), px(200.0)), {
                let panel_view = panel_view.clone();
                move |_, _| PresentationHost {
                    view: panel_view,
                    mode: HostMode::Empty,
                }
            });
            let source_id = source_window.window_id();
            let source_lease = cx.update(|cx| claim(cx, &panel_view, source_id).unwrap());
            set_host_mode(&mut cx, source_window, HostMode::Presented(source_lease));
            draw_window(&mut cx, source_window.into());
            let prepared = cx.update(|cx| {
                prepare_rehost(cx, &[source_lease], destination_window.window_id()).unwrap()
            });

            set_host_mode(
                &mut cx,
                source_window,
                HostMode::ConflictingRelease {
                    lease: source_lease,
                    prepared: prepared.clone(),
                    barrier_first,
                },
            );
            draw_window(&mut cx, source_window.into());

            assert_eq!(prepared.snapshot().phase(), RehostPhase::Invalidated);
            assert_eq!(
                prepared.snapshot().invalidation(),
                Some(Invalidation::SourceStillMounted)
            );
            assert_eq!(prepared.snapshot().source_proxy_receipt(), None);
            assert_eq!(resolved_window(&mut cx, panel.entity_id()), Some(source_id));
        }
    }

    #[test]
    fn source_release_requires_an_exact_successful_proxy_replay_receipt() {
        let mut cx = TestAppContext::single();
        let panel = cx.update(|cx| {
            cx.new(|_| PresentationProbe {
                rendered_in: Rc::new(RefCell::new(Vec::new())),
            })
        });
        let panel_view = AnyView::from(panel.clone());
        let source_window = cx.open_window(size(px(320.0), px(200.0)), {
            let panel_view = panel_view.clone();
            move |_, _| PresentationHost {
                view: panel_view,
                mode: HostMode::Empty,
            }
        });
        let destination_window = cx.open_window(size(px(320.0), px(200.0)), {
            let panel_view = panel_view.clone();
            move |_, _| PresentationHost {
                view: panel_view,
                mode: HostMode::Empty,
            }
        });
        let source_id = source_window.window_id();
        let source_lease = cx.update(|cx| claim(cx, &panel_view, source_id).unwrap());
        set_host_mode(&mut cx, source_window, HostMode::Presented(source_lease));
        draw_window(&mut cx, source_window.into());
        let prepared = cx.update(|cx| {
            prepare_rehost(cx, &[source_lease], destination_window.window_id()).unwrap()
        });

        set_host_mode(
            &mut cx,
            source_window,
            HostMode::UnprovenRelease(prepared.clone()),
        );
        draw_window(&mut cx, source_window.into());
        assert_eq!(
            prepared.snapshot().phase(),
            RehostPhase::AwaitingSourceRelease
        );
        assert_eq!(prepared.snapshot().source_proxy_receipt(), None);
        assert_eq!(resolved_window(&mut cx, panel.entity_id()), Some(source_id));

        set_host_mode(
            &mut cx,
            source_window,
            HostMode::StaleRelease(prepared.clone()),
        );
        draw_window(&mut cx, source_window.into());
        assert_eq!(
            prepared.snapshot().phase(),
            RehostPhase::AwaitingSourceRelease
        );
        assert_eq!(prepared.snapshot().source_proxy_receipt(), None);

        set_host_mode(
            &mut cx,
            source_window,
            HostMode::Releasing(prepared.clone()),
        );
        draw_window(&mut cx, source_window.into());
        assert_eq!(
            prepared.snapshot().phase(),
            RehostPhase::DestinationAdmitted
        );
        assert!(prepared.snapshot().source_proxy_receipt().is_some());
    }

    #[test]
    fn atlas_rejected_source_candidate_cannot_leak_replay_receipt_into_same_generation_retry() {
        let atlas = Arc::new(RejectOnceAtlas::new());
        let renderer_atlas = atlas.clone();
        let mut cx =
            HeadlessAppContext::with_platform(Arc::new(NoopTextSystem), Arc::new(()), move || {
                Some(Box::new(AtlasTestRenderer {
                    atlas: renderer_atlas.clone(),
                }))
            });
        let rendered_in = Rc::new(RefCell::new(Vec::new()));
        let panel = cx.update({
            let rendered_in = rendered_in.clone();
            move |cx| cx.new(|_| PresentationProbe { rendered_in })
        });
        let panel_view = AnyView::from(panel.clone());
        let source_window = cx
            .open_window(size(px(320.0), px(200.0)), {
                let panel_view = panel_view.clone();
                move |_, cx| {
                    cx.new(|_| PresentationHost {
                        view: panel_view,
                        mode: HostMode::Empty,
                    })
                }
            })
            .unwrap();
        let destination_window = cx
            .open_window(size(px(320.0), px(200.0)), {
                let panel_view = panel_view.clone();
                move |_, cx| {
                    cx.new(|_| PresentationHost {
                        view: panel_view,
                        mode: HostMode::Empty,
                    })
                }
            })
            .unwrap();
        let source_id = source_window.window_id();
        let destination_id = destination_window.window_id();
        let source_lease = cx.update(|cx| claim(cx, &panel_view, source_id).unwrap());
        source_window
            .update(&mut cx, |host, _, _| {
                host.mode = HostMode::Presented(source_lease)
            })
            .unwrap();
        let baseline_frame = draw_headless_window(&mut cx, source_window.into());
        let prepared = cx.update(|cx| prepare_rehost(cx, &[source_lease], destination_id).unwrap());
        let image = Arc::new(RenderImage::new(smallvec::smallvec![image::Frame::new(
            image::RgbaImage::from_pixel(1, 1, image::Rgba([0xff, 0, 0, 0xff])),
        )]));
        atlas.reject_next_lease();
        let (rejected_frame, rejected, rejected_window) = cx.update(|cx| {
            cx.update(|cx| {
                source_window
                    .update(cx, |host, _, _| {
                        host.mode = HostMode::ReleasingWithAtlasImage {
                            prepared: prepared.clone(),
                            image,
                        };
                    })
                    .unwrap();
                let rejected_frame = cx
                    .update_window(source_window.into(), |_, window, cx| {
                        window.draw(cx).clear();
                        window.rendered_frame_revision()
                    })
                    .unwrap();
                (
                    rejected_frame,
                    prepared.snapshot(),
                    cx.current_window_by_entity.get(&panel.entity_id()).copied(),
                )
            })
        });
        assert_eq!(rejected_frame, baseline_frame);
        assert_eq!(atlas.rejection_count(), 1);
        assert_eq!(rejected.phase(), RehostPhase::AwaitingSourceRelease);
        assert_eq!(rejected.source_proxy_receipt(), None);
        assert_eq!(rejected_window, Some(source_id));

        let accepted = prepared.snapshot();
        assert_eq!(accepted.phase(), RehostPhase::DestinationAdmitted);
        let accepted_retry_frame = accepted
            .source_proxy_receipt()
            .expect("the fresh retry must publish its own replay receipt")
            .frame_generation();
        assert_eq!(accepted_retry_frame, baseline_frame + 1);
        assert_eq!(
            accepted
                .source_proxy_receipt()
                .expect("the fresh retry must publish its own replay receipt")
                .frame_generation(),
            accepted_retry_frame
        );
        assert_eq!(
            resolved_headless_window(&mut cx, panel.entity_id()),
            Some(destination_id)
        );
    }

    #[test]
    fn incomplete_destination_batch_stays_hidden_and_retryable() {
        let mut cx = TestAppContext::single();
        let first_paints = Rc::new(RefCell::new(Vec::new()));
        let first = cx.update({
            let rendered_in = first_paints.clone();
            move |cx| cx.new(|_| PresentationProbe { rendered_in })
        });
        let second = cx.update(|cx| {
            cx.new(|_| PresentationProbe {
                rendered_in: Rc::new(RefCell::new(Vec::new())),
            })
        });
        let first_view = AnyView::from(first.clone());
        let second_view = AnyView::from(second.clone());
        let source_window = cx.open_window(size(px(320.0), px(200.0)), |_, _| Empty);
        let destination_window = cx.open_window(size(px(320.0), px(200.0)), {
            let first_view = first_view.clone();
            move |_, _| PresentationHost {
                view: first_view,
                mode: HostMode::Empty,
            }
        });
        let source_id = source_window.window_id();
        let destination_id = destination_window.window_id();
        let source_batch = cx.update(|cx| {
            claim_batch(cx, &[first_view.clone(), second_view.clone()], source_id).unwrap()
        });
        cx.update(|cx| {
            for lease in source_batch.leases().iter().copied() {
                cx.view_presentation_windows
                    .commit_mount(lease, 1, &mut cx.current_window_by_entity)
                    .unwrap();
            }
        });
        let prepared =
            cx.update(|cx| prepare_rehost(cx, source_batch.leases(), destination_id).unwrap());
        cx.update(|cx| {
            cx.view_presentation_windows
                .commit_source_release(
                    &prepared,
                    source_proxy_receipt(&prepared, source_id, 2),
                    &mut cx.current_window_by_entity,
                )
                .unwrap();
        });
        let first_destination = prepared.destination().lease_for(first.entity_id()).unwrap();
        set_host_mode(
            &mut cx,
            destination_window,
            HostMode::Presented(first_destination),
        );

        draw_window(&mut cx, destination_window.into());
        assert_eq!(
            prepared.snapshot().phase(),
            RehostPhase::DestinationAdmitted
        );
        assert!(first_paints.borrow().is_empty());

        draw_window(&mut cx, destination_window.into());
        assert_eq!(
            prepared.snapshot().phase(),
            RehostPhase::DestinationAdmitted
        );
        assert_eq!(prepared.snapshot().invalidation(), None);
        assert!(first_paints.borrow().is_empty());

        let settlement = cx
            .update(|cx| settle_rehost_source(cx, &prepared))
            .expect("an incomplete destination batch must remain compensatable");
        let SourceSettlement::RenderSource(restore) = settlement else {
            panic!("incomplete destination staging must return fresh source authority")
        };
        assert_eq!(restore.window_id(), source_id);
    }

    #[test]
    fn two_root_staging_is_inert_until_one_frame_mounts_the_batch_and_finish_exposes_it() {
        let mut cx = TestAppContext::single();
        let first_paints = Rc::new(Cell::new(0));
        let first_clicks = Rc::new(Cell::new(0));
        let second_paints = Rc::new(Cell::new(0));
        let second_clicks = Rc::new(Cell::new(0));
        let first = cx.update({
            let paints = first_paints.clone();
            let clicks = first_clicks.clone();
            move |cx| cx.new(|_| InteractivePresentationProbe { paints, clicks })
        });
        let second = cx.update({
            let paints = second_paints.clone();
            let clicks = second_clicks.clone();
            move |cx| cx.new(|_| InteractivePresentationProbe { paints, clicks })
        });
        let views = vec![AnyView::from(first.clone()), AnyView::from(second.clone())];
        let source_window =
            cx.open_window(size(px(320.0), px(200.0)), |_, _| BatchPresentationHost {
                mode: BatchHostMode::Empty,
            });
        let destination_window =
            cx.open_window(size(px(320.0), px(200.0)), |_, _| BatchPresentationHost {
                mode: BatchHostMode::Empty,
            });
        let source_id = source_window.window_id();
        let destination_id = destination_window.window_id();
        let source_batch =
            cx.update(|cx| claim_batch(cx, &views, source_id).expect("source batch claim"));
        set_batch_host_mode(
            &mut cx,
            source_window,
            BatchHostMode::Presented(leased_roots(&views, &source_batch)),
        );
        draw_window(&mut cx, source_window.into());
        assert_eq!((first_paints.get(), second_paints.get()), (1, 1));

        let prepared = cx.update(|cx| {
            prepare_rehost(cx, source_batch.leases(), destination_id).expect("prepare batch rehost")
        });
        set_batch_host_mode(
            &mut cx,
            source_window,
            BatchHostMode::Releasing(prepared.clone()),
        );
        draw_window(&mut cx, source_window.into());
        assert_eq!(
            prepared.snapshot().phase(),
            RehostPhase::DestinationAdmitted
        );
        first_paints.set(0);
        second_paints.set(0);

        let first_destination = prepared
            .destination()
            .lease_for(first.entity_id())
            .expect("destination batch should contain the first root");
        let second_destination = prepared
            .destination()
            .lease_for(second.entity_id())
            .expect("destination batch should contain the second root");
        set_batch_host_mode(
            &mut cx,
            destination_window,
            BatchHostMode::Presented(vec![(views[0].clone(), first_destination)]),
        );
        draw_window(&mut cx, destination_window.into());
        assert_eq!(
            prepared.snapshot().phase(),
            RehostPhase::DestinationAdmitted
        );
        assert_eq!((first_paints.get(), second_paints.get()), (0, 0));
        dispatch_primary_click(
            &mut cx,
            destination_window.into(),
            point(px(25.0), px(25.0)),
        );
        dispatch_primary_click(
            &mut cx,
            destination_window.into(),
            point(px(125.0), px(25.0)),
        );
        assert_eq!((first_clicks.get(), second_clicks.get()), (0, 0));

        set_batch_host_mode(
            &mut cx,
            destination_window,
            BatchHostMode::Presented(vec![(views[1].clone(), second_destination)]),
        );
        draw_window(&mut cx, destination_window.into());
        assert_eq!(
            prepared.snapshot().phase(),
            RehostPhase::DestinationAdmitted
        );
        assert_eq!(prepared.snapshot().invalidation(), None);
        assert_eq!((first_paints.get(), second_paints.get()), (0, 0));
        dispatch_primary_click(
            &mut cx,
            destination_window.into(),
            point(px(25.0), px(25.0)),
        );
        dispatch_primary_click(
            &mut cx,
            destination_window.into(),
            point(px(125.0), px(25.0)),
        );
        assert_eq!((first_clicks.get(), second_clicks.get()), (0, 0));

        set_batch_host_mode(
            &mut cx,
            destination_window,
            BatchHostMode::Presented(leased_roots(&views, prepared.destination())),
        );
        let staging_frame = draw_window(&mut cx, destination_window.into());
        assert_eq!(prepared.snapshot().phase(), RehostPhase::DestinationMounted);
        assert_eq!(
            prepared.snapshot().destination_frame_generation(),
            Some(staging_frame)
        );
        assert_eq!((first_paints.get(), second_paints.get()), (0, 0));
        dispatch_primary_click(
            &mut cx,
            destination_window.into(),
            point(px(25.0), px(25.0)),
        );
        dispatch_primary_click(
            &mut cx,
            destination_window.into(),
            point(px(125.0), px(25.0)),
        );
        assert_eq!((first_clicks.get(), second_clicks.get()), (0, 0));

        cx.update(|cx| expose_destination(cx, &prepared).expect("expose destination batch"));
        let visible_frame = draw_window(&mut cx, destination_window.into());
        assert!(first_paints.get() > 0);
        assert_eq!(first_paints.get(), second_paints.get());
        let visible = cx
            .update(|cx| presented_batch_receipt(cx, prepared.destination()))
            .expect("the complete visible batch should publish a receipt");
        assert_eq!(visible.frame_generation(), visible_frame);

        let prepared_finish = cx
            .update(|cx| prepare_finish_destination(cx, &prepared))
            .expect("the visible destination batch should prepare its finish");
        cx.update(|cx| commit_prepared_finish_destination(cx, prepared_finish));
        let stable_visible = cx
            .update(|cx| presented_batch_receipt(cx, prepared.destination()))
            .expect("terminal commit must preserve the accepted visible frame");
        assert_eq!(stable_visible.exposure(), visible.exposure());
        assert!(stable_visible.frame_generation() >= visible.frame_generation());

        dispatch_primary_click(
            &mut cx,
            destination_window.into(),
            point(px(25.0), px(25.0)),
        );
        dispatch_primary_click(
            &mut cx,
            destination_window.into(),
            point(px(125.0), px(25.0)),
        );
        assert_eq!((first_clicks.get(), second_clicks.get()), (1, 1));
    }

    #[test]
    fn cancellation_after_exposed_destination_never_duplicates_presented_content() {
        let mut cx = TestAppContext::single();
        let rendered_in = Rc::new(RefCell::new(Vec::new()));
        let panel = cx.update({
            let rendered_in = rendered_in.clone();
            move |cx| cx.new(|_| PresentationProbe { rendered_in })
        });
        let panel_view = AnyView::from(panel.clone());
        let source_window = cx.open_window(size(px(320.0), px(200.0)), {
            let panel_view = panel_view.clone();
            move |_, _| PresentationHost {
                view: panel_view,
                mode: HostMode::Empty,
            }
        });
        let destination_window = cx.open_window(size(px(320.0), px(200.0)), {
            let panel_view = panel_view.clone();
            move |_, _| PresentationHost {
                view: panel_view,
                mode: HostMode::Empty,
            }
        });
        let source_id = source_window.window_id();
        let destination_id = destination_window.window_id();
        let source_lease = cx.update(|cx| claim(cx, &panel_view, source_id).unwrap());
        set_host_mode(&mut cx, source_window, HostMode::Presented(source_lease));
        draw_window(&mut cx, source_window.into());

        let prepared = cx.update(|cx| prepare_rehost(cx, &[source_lease], destination_id).unwrap());
        let destination_lease = prepared.destination().lease_for(panel.entity_id()).unwrap();
        set_host_mode(
            &mut cx,
            source_window,
            HostMode::Releasing(prepared.clone()),
        );
        draw_window(&mut cx, source_window.into());
        assert_eq!(
            prepared.snapshot().phase(),
            RehostPhase::DestinationAdmitted
        );

        rendered_in.borrow_mut().clear();
        set_host_mode(
            &mut cx,
            destination_window,
            HostMode::Presented(destination_lease),
        );
        draw_window(&mut cx, destination_window.into());
        assert_eq!(prepared.snapshot().phase(), RehostPhase::DestinationMounted);
        assert!(
            rendered_in.borrow().is_empty(),
            "a mounted destination stays non-presenting until exposure"
        );

        let (exposed, receipt_at_exposure) = cx.update(|cx| {
            let exposed = expose_destination(cx, &prepared).unwrap();
            let receipt = presented_batch_receipt(cx, &exposed.batch);
            (exposed, receipt)
        });
        assert_eq!(
            exposed.batch.lease_for(panel.entity_id()),
            Some(destination_lease)
        );
        assert_eq!(prepared.snapshot().phase(), RehostPhase::DestinationExposed);
        assert!(
            receipt_at_exposure.is_none(),
            "exposure must require a later accepted visible frame"
        );
        let destination_frame = draw_window(&mut cx, destination_window.into());
        let visible = cx
            .update(|cx| presented_batch_receipt(cx, &exposed.batch))
            .expect("the exposed destination should publish visible-frame evidence");
        assert_eq!(visible.frame_generation(), destination_frame);
        assert!(
            !rendered_in.borrow().is_empty()
                && rendered_in
                    .borrow()
                    .iter()
                    .all(|window_id| *window_id == destination_id),
            "only the exposed destination may paint the payload"
        );
        rendered_in.borrow_mut().clear();

        let restore = cx.update(|cx| cancel_after_source_release(cx, &prepared).unwrap());
        let restored_lease = restore.lease_for(panel.entity_id()).unwrap();
        assert_ne!(restored_lease.generation(), source_lease.generation());
        assert_ne!(restored_lease.generation(), destination_lease.generation());
        assert_eq!(resolved_window(&mut cx, panel.entity_id()), Some(source_id));

        set_host_mode(
            &mut cx,
            destination_window,
            HostMode::Presented(destination_lease),
        );
        draw_window(&mut cx, destination_window.into());
        assert_eq!(prepared.snapshot().phase(), RehostPhase::RestoringSource);
        assert!(rendered_in.borrow().is_empty());

        set_host_mode(&mut cx, source_window, HostMode::Presented(restored_lease));
        let source_frame = draw_window(&mut cx, source_window.into());
        assert_eq!(prepared.snapshot().phase(), RehostPhase::SourceRestored);
        assert_eq!(
            prepared.snapshot().destination_frame_generation(),
            Some(source_frame)
        );
        assert!(rendered_in.borrow().is_empty());
        match cx.update(|cx| finish(cx, &prepared).unwrap()) {
            FinishOutcome::Source(batch) => {
                assert_eq!(batch.lease_for(panel.entity_id()), Some(restored_lease));
            }
            outcome => panic!("unexpected rehost outcome: {outcome:?}"),
        }
        cx.run_until_parked();
        assert_eq!(rendered_in.borrow().as_slice(), &[source_id]);
        let restored_visible = cx
            .update(|cx| stable_batch_presentation_receipt(cx, &restore))
            .expect("the restored source should publish one stable visible batch receipt");
        assert_eq!(restored_visible.window_id(), source_id);
        assert_eq!(restored_visible.root_count(), 1);
    }

    #[test]
    fn destination_finish_prepare_rejects_the_wrong_phase() {
        let mut cx = TestAppContext::single();
        let source = cx
            .open_window(size(px(100.0), px(100.0)), |_, _| Empty)
            .window_id();
        let destination = cx
            .open_window(size(px(100.0), px(100.0)), |_, _| Empty)
            .window_id();
        let entity_id = probe_entity_id(&mut cx);
        let mut registry = Registry::default();
        let mut current_windows = FxHashMap::from_iter([(entity_id, source)]);
        let source_lease = registry.claim(entity_id, source).unwrap();
        registry
            .commit_mount(source_lease, 1, &mut current_windows)
            .unwrap();
        let prepared = registry.prepare(&[source_lease], destination).unwrap();
        registry
            .commit_source_release(
                &prepared,
                source_proxy_receipt(&prepared, source, 2),
                &mut current_windows,
            )
            .unwrap();
        let destination_lease = prepared.destination().lease_for(entity_id).unwrap();
        registry
            .commit_mount(destination_lease, 3, &mut current_windows)
            .unwrap();

        assert!(matches!(
            registry.prepare_finish_destination(&prepared),
            Err(TransitionError::WrongState)
        ));
        assert_eq!(prepared.snapshot().phase(), RehostPhase::DestinationMounted);
        assert!(registry.admits(destination_lease, destination));
    }

    #[test]
    fn destination_finish_prepare_preserves_rollback_authority() {
        let mut cx = TestAppContext::single();
        let mut fixture = exposed_rehost_fixture(&mut cx);

        let prepared_finish = fixture
            .registry
            .prepare_finish_destination(&fixture.prepared)
            .expect("an exact exposed destination should prepare its finish");

        assert_eq!(
            fixture.prepared.snapshot().phase(),
            RehostPhase::DestinationExposed
        );
        assert!(
            fixture
                .registry
                .admits(fixture.destination_lease, fixture.destination)
        );
        drop(prepared_finish);

        let restore = fixture
            .registry
            .cancel_after_source_release(&fixture.prepared, &mut fixture.current_windows)
            .expect("preparing a finish must leave source rollback available");
        let restored_lease = restore.lease_for(fixture.entity_id).unwrap();
        assert_eq!(
            fixture.prepared.snapshot().phase(),
            RehostPhase::RestoringSource
        );
        assert!(fixture.registry.admits(restored_lease, fixture.source));
        assert!(
            !fixture
                .registry
                .admits(fixture.destination_lease, fixture.destination)
        );
    }

    #[test]
    fn exact_prepared_destination_finish_commits_once() {
        let mut cx = TestAppContext::single();
        let mut fixture = exposed_rehost_fixture(&mut cx);
        let prepared_finish = fixture
            .registry
            .prepare_finish_destination(&fixture.prepared)
            .expect("an exact exposed destination should prepare its finish");

        match fixture
            .registry
            .commit_prepared_finish_destination(prepared_finish)
        {
            FinishOutcome::Destination { batch, exposure } => {
                assert!(batch.matches_exactly(fixture.prepared.destination()));
                assert_eq!(exposure, fixture.exposure);
            }
            outcome => panic!("unexpected prepared finish outcome: {outcome:?}"),
        }

        assert!(
            !fixture
                .registry
                .rehosts
                .contains_key(&fixture.prepared.generation())
        );
        assert!(
            fixture
                .registry
                .admits(fixture.destination_lease, fixture.destination)
        );
        assert!(matches!(
            fixture
                .registry
                .prepare_finish_destination(&fixture.prepared),
            Err(TransitionError::StalePrepared)
        ));
    }

    #[test]
    fn committed_destination_finish_cannot_restore_the_source() {
        let mut cx = TestAppContext::single();
        let mut fixture = exposed_rehost_fixture(&mut cx);
        let prepared_finish = fixture
            .registry
            .prepare_finish_destination(&fixture.prepared)
            .expect("an exact exposed destination should prepare its finish");
        let outcome = fixture
            .registry
            .commit_prepared_finish_destination(prepared_finish);
        assert!(matches!(outcome, FinishOutcome::Destination { .. }));

        assert!(matches!(
            fixture
                .registry
                .cancel_after_source_release(&fixture.prepared, &mut fixture.current_windows),
            Err(TransitionError::StalePrepared)
        ));
        assert_eq!(
            fixture.current_windows.get(&fixture.entity_id),
            Some(&fixture.destination)
        );
        assert!(
            fixture
                .registry
                .admits(fixture.destination_lease, fixture.destination)
        );
    }

    #[test]
    fn source_loss_abandonment_releases_only_the_exact_rehost_generation() {
        let mut cx = TestAppContext::single();
        let mut fixture = exposed_rehost_fixture(&mut cx);
        let abandonment = fixture
            .registry
            .prepare_abandon_rehost_after_source_loss(&fixture.prepared)
            .expect("an exact exposed rehost should prepare source-loss abandonment");
        assert!(
            fixture
                .registry
                .can_commit_prepared_abandon_rehost_after_source_loss(&abandonment)
        );
        assert!(
            !fixture
                .registry
                .rehost_authority_is_absent(&fixture.prepared),
            "a prepared abandonment still owns its exact rehost authority"
        );

        let receipt = fixture
            .registry
            .commit_prepared_abandon_rehost_after_source_loss(
                abandonment,
                &mut fixture.current_windows,
            );

        assert_eq!(receipt.generation(), fixture.prepared.generation());
        assert_eq!(receipt.source_window(), fixture.source);
        assert_eq!(receipt.destination_window(), fixture.destination);
        assert_eq!(receipt.released_entities(), &[fixture.entity_id]);
        assert!(!fixture.registry.governs(fixture.entity_id));
        assert!(!fixture.current_windows.contains_key(&fixture.entity_id));
        assert!(
            fixture
                .registry
                .rehost_authority_is_absent(&fixture.prepared),
            "committed abandonment must leave an idempotent exact-generation absence proof"
        );
        assert!(matches!(
            fixture
                .registry
                .prepare_abandon_rehost_after_source_loss(&fixture.prepared),
            Err(TransitionError::StalePrepared)
        ));
        assert!(matches!(
            fixture
                .registry
                .abandon_rehost_after_source_loss(&fixture.prepared, &mut fixture.current_windows,),
            Ok(AbandonRehostOutcome::AlreadyAbandoned)
        ));
        assert!(matches!(
            fixture
                .registry
                .settle_rehost_source(&fixture.prepared, &mut fixture.current_windows),
            Err(TransitionError::ConflictingTerminalOutcome)
        ));
    }

    #[test]
    fn source_loss_abandonment_token_rejects_a_later_rehost_phase() {
        let mut cx = TestAppContext::single();
        let source = cx
            .open_window(size(px(100.0), px(100.0)), |_, _| Empty)
            .window_id();
        let destination = cx
            .open_window(size(px(100.0), px(100.0)), |_, _| Empty)
            .window_id();
        let entity_id = probe_entity_id(&mut cx);
        let mut registry = Registry::default();
        let mut current_windows = FxHashMap::from_iter([(entity_id, source)]);
        let source_lease = registry.claim(entity_id, source).unwrap();
        registry
            .commit_mount(source_lease, 1, &mut current_windows)
            .unwrap();
        let prepared = registry.prepare(&[source_lease], destination).unwrap();
        let abandonment = registry
            .prepare_abandon_rehost_after_source_loss(&prepared)
            .expect("the awaiting-source-release generation should prepare abandonment");

        registry
            .commit_source_release(
                &prepared,
                source_proxy_receipt(&prepared, source, 2),
                &mut current_windows,
            )
            .unwrap();

        assert!(
            !registry.can_commit_prepared_abandon_rehost_after_source_loss(&abandonment),
            "a phase transition must invalidate the old abandonment token"
        );
        assert!(registry.governs(entity_id));
        assert_eq!(current_windows.get(&entity_id), Some(&destination));
    }

    #[test]
    fn cross_frame_destination_mounts_restart_the_batch_without_revoking_authority() {
        let mut cx = TestAppContext::single();
        let source = cx
            .open_window(size(px(100.0), px(100.0)), |_, _| Empty)
            .window_id();
        let destination = cx
            .open_window(size(px(100.0), px(100.0)), |_, _| Empty)
            .window_id();
        let (first, second) = cx.update(|cx| {
            (
                cx.new(|_| PresentationProbe {
                    rendered_in: Rc::new(RefCell::new(Vec::new())),
                })
                .entity_id(),
                cx.new(|_| PresentationProbe {
                    rendered_in: Rc::new(RefCell::new(Vec::new())),
                })
                .entity_id(),
            )
        });
        let mut registry = Registry::default();
        let mut current_windows = FxHashMap::from_iter([(first, source), (second, source)]);
        let first_source = registry.claim(first, source).unwrap();
        let second_source = registry.claim(second, source).unwrap();
        registry
            .commit_mount(first_source, 1, &mut current_windows)
            .unwrap();
        registry
            .commit_mount(second_source, 1, &mut current_windows)
            .unwrap();
        let prepared = registry
            .prepare(&[first_source, second_source], destination)
            .unwrap();
        registry
            .commit_source_release(
                &prepared,
                source_proxy_receipt(&prepared, source, 2),
                &mut current_windows,
            )
            .unwrap();
        let first_destination = prepared.destination().lease_for(first).unwrap();
        let second_destination = prepared.destination().lease_for(second).unwrap();
        registry
            .commit_mount(first_destination, 3, &mut current_windows)
            .unwrap();
        assert_eq!(
            registry.commit_mount(second_destination, 4, &mut current_windows),
            Ok(MountCommitOutcome::AwaitingBatch)
        );
        registry
            .commit_mount(first_destination, 5, &mut current_windows)
            .unwrap();
        registry
            .commit_mount(second_destination, 5, &mut current_windows)
            .unwrap();
        assert_eq!(prepared.snapshot().phase(), RehostPhase::DestinationMounted);
        assert_eq!(prepared.snapshot().invalidation(), None);
        assert!(registry.admits(first_destination, destination));
        assert!(registry.admits(second_destination, destination));
    }

    #[test]
    fn source_window_loss_does_not_cancel_a_retryable_destination_batch() {
        let mut cx = TestAppContext::single();
        let source = cx
            .open_window(size(px(100.0), px(100.0)), |_, _| Empty)
            .window_id();
        let destination = cx
            .open_window(size(px(100.0), px(100.0)), |_, _| Empty)
            .window_id();
        let (first, second) = cx.update(|cx| {
            (
                cx.new(|_| PresentationProbe {
                    rendered_in: Rc::new(RefCell::new(Vec::new())),
                })
                .entity_id(),
                cx.new(|_| PresentationProbe {
                    rendered_in: Rc::new(RefCell::new(Vec::new())),
                })
                .entity_id(),
            )
        });
        let mut registry = Registry::default();
        let mut current_windows = FxHashMap::from_iter([(first, source), (second, source)]);
        let first_source = registry.claim(first, source).unwrap();
        let second_source = registry.claim(second, source).unwrap();
        registry
            .commit_mount(first_source, 1, &mut current_windows)
            .unwrap();
        registry
            .commit_mount(second_source, 1, &mut current_windows)
            .unwrap();
        let prepared = registry
            .prepare(&[first_source, second_source], destination)
            .unwrap();
        registry
            .commit_source_release(
                &prepared,
                source_proxy_receipt(&prepared, source, 2),
                &mut current_windows,
            )
            .unwrap();
        registry.window_closed(source, &mut current_windows);

        let first_destination = prepared.destination().lease_for(first).unwrap();
        let second_destination = prepared.destination().lease_for(second).unwrap();
        registry
            .commit_mount(first_destination, 3, &mut current_windows)
            .unwrap();
        assert_eq!(
            registry.commit_mount(second_destination, 4, &mut current_windows),
            Ok(MountCommitOutcome::AwaitingBatch)
        );
        registry
            .commit_mount(first_destination, 5, &mut current_windows)
            .unwrap();
        registry
            .commit_mount(second_destination, 5, &mut current_windows)
            .unwrap();
        assert_eq!(prepared.snapshot().phase(), RehostPhase::DestinationMounted);
        assert_eq!(prepared.snapshot().invalidation(), None);
        assert!(registry.admits(first_destination, destination));
        assert!(registry.admits(second_destination, destination));
    }

    #[test]
    fn invalidated_source_loss_can_retire_an_already_released_rehost_record() {
        let mut cx = TestAppContext::single();
        let source = cx
            .open_window(size(px(100.0), px(100.0)), |_, _| Empty)
            .window_id();
        let destination = cx
            .open_window(size(px(100.0), px(100.0)), |_, _| Empty)
            .window_id();
        let entity_id = probe_entity_id(&mut cx);
        let mut registry = Registry::default();
        let mut current_windows = FxHashMap::from_iter([(entity_id, source)]);
        let source_lease = registry.claim(entity_id, source).unwrap();
        registry
            .commit_mount(source_lease, 1, &mut current_windows)
            .unwrap();
        let prepared = registry.prepare(&[source_lease], destination).unwrap();
        registry.window_closed(source, &mut current_windows);
        assert_eq!(prepared.snapshot().phase(), RehostPhase::Invalidated);

        let abandonment = registry
            .prepare_abandon_rehost_after_source_loss(&prepared)
            .expect("an invalidated source-loss record should remain exactly retireable");
        let receipt = registry
            .commit_prepared_abandon_rehost_after_source_loss(abandonment, &mut current_windows);

        assert_eq!(receipt.released_entities(), &[entity_id]);
        assert!(!registry.rehosts.contains_key(&prepared.generation()));
        assert!(!registry.governs(entity_id));
        assert!(!current_windows.contains_key(&entity_id));
    }

    #[test]
    fn destination_close_after_source_release_restores_unique_source_authority() {
        let mut cx = TestAppContext::single();
        let source = cx
            .open_window(size(px(100.0), px(100.0)), |_, _| Empty)
            .window_id();
        let destination = cx
            .open_window(size(px(100.0), px(100.0)), |_, _| Empty)
            .window_id();
        let entity_id = cx
            .update(|cx| {
                cx.new(|_| PresentationProbe {
                    rendered_in: Rc::new(RefCell::new(Vec::new())),
                })
            })
            .entity_id();
        let mut registry = Registry::default();
        let mut current_windows = FxHashMap::from_iter([(entity_id, source)]);
        let source_lease = registry.claim(entity_id, source).unwrap();
        registry
            .commit_mount(source_lease, 1, &mut current_windows)
            .unwrap();
        let prepared = registry.prepare(&[source_lease], destination).unwrap();
        let destination_lease = prepared.destination().lease_for(entity_id).unwrap();
        registry
            .commit_source_release(
                &prepared,
                source_proxy_receipt(&prepared, source, 2),
                &mut current_windows,
            )
            .unwrap();

        registry.window_closed(destination, &mut current_windows);

        assert_eq!(prepared.snapshot().phase(), RehostPhase::RestoringSource);
        assert_eq!(
            prepared.snapshot().invalidation(),
            Some(Invalidation::DestinationWindowClosed)
        );
        assert!(!registry.admits(destination_lease, destination));
        let restore = prepared
            .restored_source()
            .expect("destination close should mint fresh source authority");
        let restored_lease = restore.lease_for(entity_id).unwrap();
        assert!(registry.admits(restored_lease, source));
        assert_eq!(current_windows.get(&entity_id), Some(&source));

        registry
            .commit_mount(restored_lease, 3, &mut current_windows)
            .unwrap();
        assert_eq!(prepared.snapshot().phase(), RehostPhase::SourceRestored);
        match registry.finish(&prepared).unwrap() {
            FinishOutcome::Source(batch) => {
                assert_eq!(batch.lease_for(entity_id), Some(restored_lease));
            }
            outcome => panic!("unexpected rehost outcome: {outcome:?}"),
        }
    }

    #[test]
    fn initial_batch_claim_is_atomic_when_one_root_conflicts() {
        let mut cx = TestAppContext::single();
        let source = cx
            .open_window(size(px(100.0), px(100.0)), |_, _| Empty)
            .window_id();
        let destination = cx
            .open_window(size(px(100.0), px(100.0)), |_, _| Empty)
            .window_id();
        let first = probe_entity_id(&mut cx);
        let second = probe_entity_id(&mut cx);
        let mut registry = Registry::default();
        let conflicting = registry.claim(second, destination).unwrap();

        assert!(matches!(
            registry.claim_batch(&[first, second], source),
            Err(ClaimError::AlreadyBound { current }) if current == conflicting
        ));
        assert!(!registry.governs(first));
        assert!(registry.admits(conflicting, destination));
    }

    #[test]
    fn initial_batch_claim_uses_one_generation_and_yields_a_stable_receipt() {
        let mut cx = TestAppContext::single();
        let source = cx
            .open_window(size(px(100.0), px(100.0)), |_, _| Empty)
            .window_id();
        let first = probe_entity_id(&mut cx);
        let second = probe_entity_id(&mut cx);
        let mut registry = Registry::default();
        let mut current_windows = FxHashMap::default();

        let batch = registry.claim_batch(&[first, second], source).unwrap();
        let generation = batch.leases()[0].generation();
        assert!(
            batch
                .leases()
                .iter()
                .all(|lease| lease.generation() == generation)
        );
        for lease in batch.leases().iter().copied() {
            registry
                .commit_mount(lease, 17, &mut current_windows)
                .unwrap();
        }

        let receipt = registry
            .stable_batch_presentation_receipt(&batch)
            .expect("one atomic claim mounted in one frame must yield a stable receipt");
        assert_eq!(receipt.window_id(), source);
        assert_eq!(receipt.lease_generation(), generation);
        assert_eq!(receipt.root_count(), 2);
        assert_eq!(receipt.frame_generation(), 17);
    }

    #[test]
    fn separately_claimed_generations_cannot_be_retroactively_grouped() {
        let mut cx = TestAppContext::single();
        let source = cx
            .open_window(size(px(100.0), px(100.0)), |_, _| Empty)
            .window_id();
        let first = probe_entity_id(&mut cx);
        let second = probe_entity_id(&mut cx);
        let unclaimed = probe_entity_id(&mut cx);
        let mut registry = Registry::default();
        let first_lease = registry.claim(first, source).unwrap();
        let second_lease = registry.claim(second, source).unwrap();

        assert!(matches!(
            registry.claim_batch(&[first, second, unclaimed], source),
            Err(ClaimError::MixedBatchGenerations { first, current })
                if first == first_lease && current == second_lease
        ));
        assert!(!registry.governs(unclaimed));
        assert!(registry.admits(first_lease, source));
        assert!(registry.admits(second_lease, source));
    }

    #[test]
    fn an_unclaimed_root_can_join_one_existing_atomic_batch_generation() {
        let mut cx = TestAppContext::single();
        let source = cx
            .open_window(size(px(100.0), px(100.0)), |_, _| Empty)
            .window_id();
        let first = probe_entity_id(&mut cx);
        let second = probe_entity_id(&mut cx);
        let joined = probe_entity_id(&mut cx);
        let mut registry = Registry::default();
        let initial = registry.claim_batch(&[first, second], source).unwrap();
        let generation = initial.leases()[0].generation();

        let extended = registry
            .claim_batch(&[first, second, joined], source)
            .unwrap();
        assert_eq!(extended.lease_for(first), initial.lease_for(first));
        assert_eq!(extended.lease_for(second), initial.lease_for(second));
        assert_eq!(extended.lease_for(joined).unwrap().generation(), generation);
    }

    #[test]
    fn source_release_never_partially_admits_a_stale_batch() {
        let mut cx = TestAppContext::single();
        let source = cx
            .open_window(size(px(100.0), px(100.0)), |_, _| Empty)
            .window_id();
        let destination = cx
            .open_window(size(px(100.0), px(100.0)), |_, _| Empty)
            .window_id();
        let first = probe_entity_id(&mut cx);
        let second = probe_entity_id(&mut cx);
        let mut registry = Registry::default();
        let mut current_windows = FxHashMap::from_iter([(first, source), (second, source)]);
        let source_batch = registry.claim_batch(&[first, second], source).unwrap();
        for lease in source_batch.leases().iter().copied() {
            registry
                .commit_mount(lease, 1, &mut current_windows)
                .unwrap();
        }
        let prepared = registry
            .prepare(source_batch.leases(), destination)
            .unwrap();
        registry.bindings.remove(&first);

        assert_eq!(
            registry.commit_source_release(
                &prepared,
                source_proxy_receipt(&prepared, source, 2),
                &mut current_windows,
            ),
            Err(TransitionError::StaleLease)
        );
        assert_eq!(prepared.snapshot().phase(), RehostPhase::Invalidated);
        assert_eq!(
            prepared.snapshot().invalidation(),
            Some(Invalidation::StaleLease)
        );
        assert!(!registry.governs(first));
        assert!(!registry.governs(second));
        assert!(!current_windows.contains_key(&first));
        assert!(!current_windows.contains_key(&second));
    }

    #[test]
    fn cancellation_after_destination_mount_restores_the_source_batch() {
        let mut cx = TestAppContext::single();
        let source = cx
            .open_window(size(px(100.0), px(100.0)), |_, _| Empty)
            .window_id();
        let destination = cx
            .open_window(size(px(100.0), px(100.0)), |_, _| Empty)
            .window_id();
        let entity_id = probe_entity_id(&mut cx);
        let mut registry = Registry::default();
        let mut current_windows = FxHashMap::from_iter([(entity_id, source)]);
        let source_lease = registry.claim(entity_id, source).unwrap();
        registry
            .commit_mount(source_lease, 1, &mut current_windows)
            .unwrap();
        let prepared = registry.prepare(&[source_lease], destination).unwrap();
        registry
            .commit_source_release(
                &prepared,
                source_proxy_receipt(&prepared, source, 2),
                &mut current_windows,
            )
            .unwrap();
        let destination_lease = prepared.destination().lease_for(entity_id).unwrap();
        registry
            .commit_mount(destination_lease, 3, &mut current_windows)
            .unwrap();
        assert_eq!(prepared.snapshot().phase(), RehostPhase::DestinationMounted);

        let restore = registry
            .cancel_after_source_release(&prepared, &mut current_windows)
            .unwrap();
        let restored_lease = restore.lease_for(entity_id).unwrap();
        assert_eq!(prepared.snapshot().phase(), RehostPhase::RestoringSource);
        assert!(!registry.admits(destination_lease, destination));
        assert!(registry.admits(restored_lease, source));
        assert_eq!(current_windows.get(&entity_id), Some(&source));

        registry
            .commit_mount(restored_lease, 4, &mut current_windows)
            .unwrap();
        assert_eq!(prepared.snapshot().phase(), RehostPhase::SourceRestored);
    }

    #[test]
    fn source_settlement_retires_an_unreleased_session_without_exposing_phases() {
        let mut cx = TestAppContext::single();
        let source = cx
            .open_window(size(px(100.0), px(100.0)), |_, _| Empty)
            .window_id();
        let destination = cx
            .open_window(size(px(100.0), px(100.0)), |_, _| Empty)
            .window_id();
        let entity_id = probe_entity_id(&mut cx);
        let mut registry = Registry::default();
        let mut current_windows = FxHashMap::from_iter([(entity_id, source)]);
        let source_lease = registry.claim(entity_id, source).unwrap();
        registry
            .commit_mount(source_lease, 1, &mut current_windows)
            .unwrap();
        let prepared = registry.prepare(&[source_lease], destination).unwrap();
        let outcome = registry
            .settle_rehost_source(&prepared, &mut current_windows)
            .unwrap();
        let SourceSettlement::RetiredToSource(source_batch) = outcome else {
            panic!("unreleased source settlement must retire to its source");
        };
        assert_eq!(source_batch.lease_for(entity_id), Some(source_lease));
        assert!(registry.rehost_authority_is_absent(&prepared));
        assert_eq!(
            prepared
                .terminal_tombstone()
                .as_ref()
                .map(RehostTerminalTombstone::disposition),
            Some(RehostTerminalDisposition::SourceCommitted)
        );
        assert_eq!(current_windows.get(&entity_id), Some(&source));
        assert!(matches!(
            registry
                .settle_rehost_source(&prepared, &mut current_windows)
                .unwrap(),
            SourceSettlement::AlreadyRetired
        ));
        assert!(matches!(
            registry.abandon_rehost_after_source_loss(&prepared, &mut current_windows),
            Err(TransitionError::ConflictingTerminalOutcome)
        ));
    }

    #[test]
    fn source_settlement_returns_only_the_source_render_obligation_after_release() {
        let mut cx = TestAppContext::single();
        let source = cx
            .open_window(size(px(100.0), px(100.0)), |_, _| Empty)
            .window_id();
        let destination = cx
            .open_window(size(px(100.0), px(100.0)), |_, _| Empty)
            .window_id();
        let entity_id = probe_entity_id(&mut cx);
        let mut registry = Registry::default();
        let mut current_windows = FxHashMap::from_iter([(entity_id, source)]);
        let source_lease = registry.claim(entity_id, source).unwrap();
        registry
            .commit_mount(source_lease, 1, &mut current_windows)
            .unwrap();
        let prepared = registry.prepare(&[source_lease], destination).unwrap();
        registry
            .commit_source_release(
                &prepared,
                source_proxy_receipt(&prepared, source, 2),
                &mut current_windows,
            )
            .unwrap();

        let outcome = registry
            .settle_rehost_source(&prepared, &mut current_windows)
            .unwrap();
        let SourceSettlement::RenderSource(restored) = outcome else {
            panic!("released destination authority must produce a source render obligation");
        };
        assert_eq!(restored.window_id(), source);
        assert!(
            prepared
                .restored_source()
                .is_some_and(|current| current.matches_exactly(&restored))
        );
        assert_eq!(prepared.snapshot().phase(), RehostPhase::RestoringSource);
    }

    #[test]
    fn endpoint_loss_releases_only_exact_members_of_a_stable_batch() {
        let mut cx = TestAppContext::single();
        let source = cx
            .open_window(size(px(100.0), px(100.0)), |_, _| Empty)
            .window_id();
        let replacement = cx
            .open_window(size(px(100.0), px(100.0)), |_, _| Empty)
            .window_id();
        let first = probe_entity_id(&mut cx);
        let second = probe_entity_id(&mut cx);
        let mut registry = Registry::default();
        let mut current_windows = FxHashMap::from_iter([(first, source), (second, source)]);
        let source_batch = registry.claim_batch(&[first, second], source).unwrap();
        for lease in source_batch.leases().iter().copied() {
            registry
                .commit_mount(lease, 1, &mut current_windows)
                .unwrap();
        }

        assert_eq!(registry.entity_released(first), vec![first]);
        current_windows.remove(&first);
        let replacement_lease = registry.claim(first, replacement).unwrap();
        current_windows.insert(first, replacement);
        registry
            .commit_mount(replacement_lease, 2, &mut current_windows)
            .unwrap();

        assert_eq!(
            registry.release_stable_leases_after_endpoint_loss(
                source_batch.leases(),
                &mut current_windows,
            ),
            vec![second]
        );
        assert_eq!(
            registry.stable_lease_for_window(first, replacement),
            Some(replacement_lease)
        );
        assert!(!registry.governs(second));
        assert_eq!(current_windows.get(&first), Some(&replacement));
        assert!(!current_windows.contains_key(&second));
    }

    #[test]
    fn cross_frame_source_restore_restarts_the_batch_and_can_finish_later() {
        let mut cx = TestAppContext::single();
        let source = cx
            .open_window(size(px(100.0), px(100.0)), |_, _| Empty)
            .window_id();
        let destination = cx
            .open_window(size(px(100.0), px(100.0)), |_, _| Empty)
            .window_id();
        let first = probe_entity_id(&mut cx);
        let second = probe_entity_id(&mut cx);
        let mut registry = Registry::default();
        let mut current_windows = FxHashMap::from_iter([(first, source), (second, source)]);
        let source_batch = registry.claim_batch(&[first, second], source).unwrap();
        for lease in source_batch.leases().iter().copied() {
            registry
                .commit_mount(lease, 1, &mut current_windows)
                .unwrap();
        }
        let prepared = registry
            .prepare(source_batch.leases(), destination)
            .unwrap();
        registry
            .commit_source_release(
                &prepared,
                source_proxy_receipt(&prepared, source, 2),
                &mut current_windows,
            )
            .unwrap();
        let first_destination = prepared.destination().lease_for(first).unwrap();
        let second_destination = prepared.destination().lease_for(second).unwrap();
        registry
            .commit_mount(first_destination, 3, &mut current_windows)
            .unwrap();
        assert_eq!(
            registry.commit_mount(second_destination, 4, &mut current_windows),
            Ok(MountCommitOutcome::AwaitingBatch)
        );
        let restore = registry
            .cancel_after_source_release(&prepared, &mut current_windows)
            .expect("source settlement should mint fresh restoration authority");
        let first_restore = restore.lease_for(first).unwrap();
        let second_restore = restore.lease_for(second).unwrap();
        registry
            .commit_mount(first_restore, 5, &mut current_windows)
            .unwrap();
        assert_eq!(
            registry.commit_mount(second_restore, 6, &mut current_windows),
            Ok(MountCommitOutcome::AwaitingBatch)
        );
        registry
            .commit_mount(first_restore, 7, &mut current_windows)
            .unwrap();
        registry
            .commit_mount(second_restore, 7, &mut current_windows)
            .unwrap();
        assert_eq!(prepared.snapshot().phase(), RehostPhase::SourceRestored);
        assert_eq!(prepared.snapshot().invalidation(), None);
        match registry.finish(&prepared).unwrap() {
            FinishOutcome::Source(batch) => {
                assert_eq!(batch.lease_for(first), Some(first_restore));
                assert_eq!(batch.lease_for(second), Some(second_restore));
            }
            outcome => panic!("unexpected rehost outcome: {outcome:?}"),
        }
    }

    #[test]
    fn releasing_one_rehosted_entity_revokes_all_sibling_authority() {
        let mut cx = TestAppContext::single();
        let source = cx
            .open_window(size(px(100.0), px(100.0)), |_, _| Empty)
            .window_id();
        let destination = cx
            .open_window(size(px(100.0), px(100.0)), |_, _| Empty)
            .window_id();
        let first = probe_entity_id(&mut cx);
        let second = probe_entity_id(&mut cx);
        let mut registry = Registry::default();
        let mut current_windows = FxHashMap::from_iter([(first, source), (second, source)]);
        let source_batch = registry.claim_batch(&[first, second], source).unwrap();
        for lease in source_batch.leases().iter().copied() {
            registry
                .commit_mount(lease, 1, &mut current_windows)
                .unwrap();
        }
        let prepared = registry
            .prepare(source_batch.leases(), destination)
            .unwrap();
        registry
            .commit_source_release(
                &prepared,
                source_proxy_receipt(&prepared, source, 2),
                &mut current_windows,
            )
            .unwrap();

        let released = registry.entity_released(first);
        for released_entity_id in &released {
            current_windows.remove(released_entity_id);
        }

        assert_eq!(prepared.snapshot().phase(), RehostPhase::Invalidated);
        assert_eq!(
            prepared.snapshot().invalidation(),
            Some(Invalidation::EntityReleased)
        );
        assert!(!registry.governs(first));
        assert!(!registry.governs(second));
        assert_eq!(released.len(), 2);
        assert!(released.contains(&first));
        assert!(released.contains(&second));
        assert!(!current_windows.contains_key(&first));
        assert!(!current_windows.contains_key(&second));
        match registry.finish(&prepared).unwrap() {
            FinishOutcome::Invalidated(Invalidation::EntityReleased) => {}
            outcome => panic!("unexpected rehost outcome: {outcome:?}"),
        }
        assert!(registry.claim(second, source).is_ok());
    }

    #[test]
    fn exact_invalidated_source_finish_consumes_only_the_rehost_record() {
        let mut cx = TestAppContext::single();
        let source = cx
            .open_window(size(px(100.0), px(100.0)), |_, _| Empty)
            .window_id();
        let destination = cx
            .open_window(size(px(100.0), px(100.0)), |_, _| Empty)
            .window_id();
        let entity_id = probe_entity_id(&mut cx);
        let mut registry = Registry::default();
        let mut current_windows = FxHashMap::from_iter([(entity_id, source)]);
        let source_batch = registry.claim_batch(&[entity_id], source).unwrap();
        let source_lease = source_batch.lease_for(entity_id).unwrap();
        registry
            .commit_mount(source_lease, 1, &mut current_windows)
            .unwrap();
        let prepared = registry
            .prepare(source_batch.leases(), destination)
            .unwrap();

        assert_eq!(
            registry.commit_source_release(
                &prepared,
                source_proxy_receipt(&prepared, source, 1),
                &mut current_windows,
            ),
            Err(TransitionError::SourceStillMounted)
        );
        assert_eq!(
            prepared.snapshot().source_invalidation_disposition(),
            Some(SourceInvalidationDisposition::SourceAuthorityUnchanged)
        );

        let outcome = registry
            .finish_source_or_release_authority(&prepared, &source_batch, &mut current_windows)
            .unwrap();
        assert!(matches!(
            outcome,
            FinishSourceOutcome::Finished(FinishOutcome::Invalidated(
                Invalidation::SourceStillMounted
            ))
        ));
        assert!(registry.source_finish_is_committed(&prepared, &source_batch));
        assert!(registry.governs(entity_id));
        assert_eq!(current_windows.get(&entity_id), Some(&source));
    }

    #[test]
    fn invalidated_source_finish_drift_releases_every_exact_sibling_authority() {
        let mut cx = TestAppContext::single();
        let source = cx
            .open_window(size(px(100.0), px(100.0)), |_, _| Empty)
            .window_id();
        let destination = cx
            .open_window(size(px(100.0), px(100.0)), |_, _| Empty)
            .window_id();
        let first = probe_entity_id(&mut cx);
        let second = probe_entity_id(&mut cx);
        let mut registry = Registry::default();
        let mut current_windows = FxHashMap::from_iter([(first, source), (second, source)]);
        let source_batch = registry.claim_batch(&[first, second], source).unwrap();
        for lease in source_batch.leases().iter().copied() {
            registry
                .commit_mount(lease, 1, &mut current_windows)
                .unwrap();
        }
        let prepared = registry
            .prepare(source_batch.leases(), destination)
            .unwrap();
        assert_eq!(
            registry.commit_source_release(
                &prepared,
                source_proxy_receipt(&prepared, source, 1),
                &mut current_windows,
            ),
            Err(TransitionError::SourceStillMounted)
        );

        assert_eq!(registry.entity_released(first), vec![first]);
        current_windows.remove(&first);
        let outcome = registry
            .finish_source_or_release_authority(&prepared, &source_batch, &mut current_windows)
            .unwrap();

        assert!(matches!(
            outcome,
            FinishSourceOutcome::PresentationAuthorityReleased(Invalidation::StaleLease)
        ));
        assert_eq!(
            prepared.snapshot().source_invalidation_disposition(),
            Some(SourceInvalidationDisposition::PresentationAuthorityReleased)
        );
        assert!(!registry.governs(first));
        assert!(!registry.governs(second));
        assert!(!current_windows.contains_key(&first));
        assert!(!current_windows.contains_key(&second));

        let abandonment = registry
            .prepare_abandon_rehost_after_source_loss(&prepared)
            .expect("authority loss should leave an exact recovery obligation");
        registry
            .commit_prepared_abandon_rehost_after_source_loss(abandonment, &mut current_windows);
        assert!(registry.rehost_authority_is_absent(&prepared));
    }

    #[test]
    fn closing_only_the_current_restore_window_invalidates_restoration() {
        let mut cx = TestAppContext::single();
        let source = cx
            .open_window(size(px(100.0), px(100.0)), |_, _| Empty)
            .window_id();
        let destination = cx
            .open_window(size(px(100.0), px(100.0)), |_, _| Empty)
            .window_id();
        let entity_id = cx
            .update(|cx| {
                cx.new(|_| PresentationProbe {
                    rendered_in: Rc::new(RefCell::new(Vec::new())),
                })
            })
            .entity_id();
        let mut registry = Registry::default();
        let mut current_windows = FxHashMap::from_iter([(entity_id, source)]);
        let source_lease = registry.claim(entity_id, source).unwrap();
        registry
            .commit_mount(source_lease, 1, &mut current_windows)
            .unwrap();
        let prepared = registry.prepare(&[source_lease], destination).unwrap();
        registry
            .commit_source_release(
                &prepared,
                source_proxy_receipt(&prepared, source, 2),
                &mut current_windows,
            )
            .unwrap();
        registry
            .cancel_after_source_release(&prepared, &mut current_windows)
            .unwrap();

        registry.window_closed(destination, &mut current_windows);
        assert_eq!(prepared.snapshot().phase(), RehostPhase::RestoringSource);

        registry.window_closed(source, &mut current_windows);
        assert_eq!(prepared.snapshot().phase(), RehostPhase::Invalidated);
        assert_eq!(
            prepared.snapshot().invalidation(),
            Some(Invalidation::SourceWindowClosed)
        );
    }
}
