#[cfg(test)]
use crate::debug::DockDebugInstrumentation;
use crate::{
    DockActionApplyError, DockActionOutcome, DockController, DockItemId, DockSpaceId,
    DockViewportFocusCommand, DockViewportFocusRequest, DockViewportPlatformFocusRestoreGate,
    DockViewportRuntimeHandle, DockViewportRuntimeLineage, DockViewportRuntimeWorkContext,
    DockVisualStyle, DockVisualStyleResolver,
    drag::{DockDragPayload, DockDragPayloadIdentity, DockDragPayloadKind},
    geometry::DockDropGuideMetrics,
    host_render_session::{DockHostPresentationSession, DockHostRenderSession},
    interaction::{DockInteractionRuntime, DockPendingFocusCommand},
    presentation_scene::DockPresentationScene,
    surface::{
        DockSurfaceActivationHostRegistration, DockSurfaceActivationHostRegistrationStatus,
        DockSurfaceActivationOutcome, DockSurfaceActivationSettlements, DockSurfaceChangeCategory,
        DockSurfaceOwner,
        live_payload_carrier::DockLivePayloadCarrier,
        live_undock::{
            DockLiveUndockIdentity, DockLiveUndockPayloadLeaseReceipt,
            DockLiveUndockPayloadMountReceipt, DockLiveUndockPayloadPresentationReceipt,
            DockLiveUndockPromotionToken, DockLiveUndockSourceFocusSnapshot,
            DockLiveUndockSourceProxyReceipt,
        },
        payload_recovery::{
            DockPayloadRecoveryEntry, DockPayloadRecoveryRestoreAction,
            DockPayloadRecoveryRestoreError, DockPayloadRecoveryRestoreReceipt,
        },
        window_session::{DockSurfaceWindowSessionLease, DockSurfaceWindowSessionOpeningToken},
        with_root_transaction,
    },
    transition_executor::DockTransitionExecutor,
    viewport_registry::DockViewportRegistrationKey,
    visual_affordance_scene::DockVisualAffordanceScene,
    workspace::DockWorkspace,
    zoom_state::DockZoomState,
};
use open_gpui::{
    AnyElement, AnyView, App, AppContext as _, Context, Entity, EntityId, FocusClaimOutcome,
    FocusHandle, IntoElement, Pixels, PointerCaptureHandle, PrepaintPublicationId, SharedString,
    Subscription, Window, WindowId, WindowProvisionalRevealTicket, px, retained_visual,
    view_presentation_window,
};
use open_gpui_motion::MotionPreference;
use std::{collections::HashMap, rc::Rc};

#[derive(Debug)]
struct DockPanelFocusTracker {
    focus_handle: FocusHandle,
    _subscription: Subscription,
}

#[derive(Debug)]
struct DockPendingFocusCompletion {
    ticket: DockPendingFocusCommand,
    target: Option<FocusHandle>,
    _subscription: Subscription,
}

#[derive(Debug)]
struct DockPendingRecoveryEntryFocusCompletion {
    action: DockPayloadRecoveryRestoreAction,
    target: FocusHandle,
    _subscription: Subscription,
}

#[derive(Debug)]
struct DockPendingRecoveryRestoreFocus {
    generation: u64,
    item: DockItemId,
    descendant: Option<DockLiveUndockSourceFocusSnapshot>,
    completion_target: Option<FocusHandle>,
    completion: Option<Subscription>,
}

enum DockNoPanelFocusSettlement {
    Focus(FocusHandle),
    Blur,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct DockHostWindowBinding {
    window_id: WindowId,
    generation: u64,
}

impl DockHostWindowBinding {
    pub(crate) const fn window_id(self) -> WindowId {
        self.window_id
    }

    pub(crate) const fn generation(self) -> u64 {
        self.generation
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum DockHostPrimaryAnchorAuthority {
    Opening(DockSurfaceWindowSessionOpeningToken),
    Active(DockSurfaceWindowSessionLease),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum DockHostRole {
    Unmanaged,
    Embedded,
    PrimaryAnchor(DockHostPrimaryAnchorAuthority),
    ProvisionalViewport(crate::surface::live_undock::DockLiveUndockOpeningKey),
    ManagedViewport(DockSurfaceWindowSessionLease),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct DockHostLivePresentationKey {
    identity: DockLiveUndockIdentity,
    rehost_generation: u64,
    binding: DockHostWindowBinding,
    epoch: u64,
}

impl DockHostLivePresentationKey {
    pub(crate) const fn identity(self) -> DockLiveUndockIdentity {
        self.identity
    }

    pub(crate) const fn binding(self) -> DockHostWindowBinding {
        self.binding
    }

    pub(crate) const fn rehost_generation(self) -> u64 {
        self.rehost_generation
    }

    pub(crate) const fn epoch(self) -> u64 {
        self.epoch
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct DockHostRecoveryPresentationKey {
    action: DockPayloadRecoveryRestoreAction,
    rehost_generation: u64,
    binding: DockHostWindowBinding,
    epoch: u64,
}

impl DockHostRecoveryPresentationKey {
    pub(crate) const fn action(self) -> DockPayloadRecoveryRestoreAction {
        self.action
    }

    pub(crate) const fn binding(self) -> DockHostWindowBinding {
        self.binding
    }

    pub(crate) const fn rehost_generation(self) -> u64 {
        self.rehost_generation
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum DockHostLiveSourcePhase {
    Releasing,
    Frozen,
    Retired,
}

#[derive(Debug, Clone)]
pub(crate) enum DockHostLiveDestinationPhase {
    Staging,
    Exposed(DockLiveUndockPayloadMountReceipt),
    Presented(DockLiveUndockPayloadPresentationReceipt),
    RevealArmed {
        presentation: DockLiveUndockPayloadPresentationReceipt,
        ticket: WindowProvisionalRevealTicket,
    },
    RevealObserving {
        presentation: DockLiveUndockPayloadPresentationReceipt,
        candidate_frame: DockLiveUndockPayloadPresentationReceipt,
        submitted_frame: Option<DockLiveUndockPayloadPresentationReceipt>,
        ticket: WindowProvisionalRevealTicket,
    },
    RevealSettled,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct DockHostLiveDestinationGeometry {
    current_bounds: crate::viewport_registry::DockViewportWindowBoundsFrame,
    host_geometry: crate::DockViewportHostGeometry,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum DockHostLiveSourceRestorationPhase {
    Staging,
    AwaitingVisibleFrame,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum DockHostLiveSourceRestorationInstallOutcome {
    Installed,
    AlreadyInstalled,
    PresentationAuthorityLost,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum DockHostRecoverySourcePhase {
    Releasing,
    Frozen,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum DockHostRecoveryDestinationPhase {
    AwaitingSourceRelease,
    Staging,
    Exposed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum DockHostRecoverySourceRestorationPhase {
    Staging,
    AwaitingVisibleFrame,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum DockHostRecoverySourceRestorationInstallOutcome {
    Installed,
    AlreadyInstalled,
    PresentationAuthorityLost,
}

#[derive(Debug, Clone)]
pub(crate) enum DockHostLivePresentationMode {
    SourceProjection {
        lease: DockLiveUndockPayloadLeaseReceipt,
        projection: view_presentation_window::RehostProjection,
        retained: retained_visual::Ticket,
        carrier: DockLivePayloadCarrier,
        phase: DockHostLiveSourcePhase,
    },
    DestinationProjection {
        proxy: DockLiveUndockSourceProxyReceipt,
        projection: view_presentation_window::RehostProjection,
        leases: view_presentation_window::LeaseBatch,
        accepted_geometry: Option<DockHostLiveDestinationGeometry>,
        phase: DockHostLiveDestinationPhase,
    },
    SourceRestoration {
        lease: DockLiveUndockPayloadLeaseReceipt,
        projection: view_presentation_window::RehostProjection,
        leases: view_presentation_window::LeaseBatch,
        retained: Option<(retained_visual::Ticket, DockLivePayloadCarrier)>,
        phase: DockHostLiveSourceRestorationPhase,
    },
}

#[derive(Debug, Clone)]
pub(crate) struct DockHostLivePresentationState {
    pub(crate) key: DockHostLivePresentationKey,
    pub(crate) presentation: DockHostPresentationSession,
    pub(crate) mode: DockHostLivePresentationMode,
}

#[derive(Debug, Clone)]
pub(crate) enum DockHostRecoveryPresentationMode {
    SourceProjection {
        projection: view_presentation_window::RehostProjection,
        phase: DockHostRecoverySourcePhase,
    },
    DestinationProjection {
        projection: view_presentation_window::RehostProjection,
        leases: view_presentation_window::LeaseBatch,
        resolved_roots: Vec<AnyView>,
        phase: DockHostRecoveryDestinationPhase,
    },
    SourceRestoration {
        projection: view_presentation_window::RehostProjection,
        leases: view_presentation_window::LeaseBatch,
        resolved_roots: Vec<AnyView>,
        phase: DockHostRecoverySourceRestorationPhase,
    },
}

#[derive(Debug, Clone)]
pub(crate) struct DockHostRecoveryPresentationState {
    pub(crate) key: DockHostRecoveryPresentationKey,
    pub(crate) presentation: DockHostPresentationSession,
    pub(crate) mode: DockHostRecoveryPresentationMode,
}

#[derive(Debug, Clone)]
enum DockHostPresentationState {
    Live(DockHostLivePresentationState),
    Recovery(DockHostRecoveryPresentationState),
}

impl DockHostPresentationState {
    fn presentation(&self) -> &DockHostPresentationSession {
        match self {
            Self::Live(state) => &state.presentation,
            Self::Recovery(state) => &state.presentation,
        }
    }

    fn as_live(&self) -> Option<&DockHostLivePresentationState> {
        match self {
            Self::Live(state) => Some(state),
            Self::Recovery(_) => None,
        }
    }

    fn as_live_mut(&mut self) -> Option<&mut DockHostLivePresentationState> {
        match self {
            Self::Live(state) => Some(state),
            Self::Recovery(_) => None,
        }
    }

    fn as_recovery(&self) -> Option<&DockHostRecoveryPresentationState> {
        match self {
            Self::Live(_) => None,
            Self::Recovery(state) => Some(state),
        }
    }

    fn as_recovery_mut(&mut self) -> Option<&mut DockHostRecoveryPresentationState> {
        match self {
            Self::Live(_) => None,
            Self::Recovery(state) => Some(state),
        }
    }

    fn source_restoration_batch(&self) -> Option<view_presentation_window::LeaseBatch> {
        match self {
            Self::Live(DockHostLivePresentationState {
                mode: DockHostLivePresentationMode::SourceRestoration { leases, .. },
                ..
            })
            | Self::Recovery(DockHostRecoveryPresentationState {
                mode: DockHostRecoveryPresentationMode::SourceRestoration { leases, .. },
                ..
            }) => Some(leases.clone()),
            Self::Live(_) | Self::Recovery(_) => None,
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct DockHostLiveSourceSemanticProxy {
    key: DockHostLivePresentationKey,
    lease: DockLiveUndockPayloadLeaseReceipt,
    carrier: DockLivePayloadCarrier,
    accessible_name: SharedString,
    source_focus: Option<DockLiveUndockSourceFocusSnapshot>,
}

#[derive(Clone, Debug)]
pub(crate) struct DockHostNativeDragTransportProxy {
    transport: crate::native_captured_drag::DockNativeCapturedDragTransportLease,
    payload_identity: DockDragPayloadIdentity,
    pointer_capture: PointerCaptureHandle,
}

impl DockHostNativeDragTransportProxy {
    pub(crate) const fn key(
        &self,
    ) -> crate::native_captured_drag::DockNativeCapturedDragTransportKey {
        self.transport.key()
    }

    pub(crate) fn is_active(&self) -> bool {
        self.transport.is_active()
    }

    pub(crate) fn matches_payload(&self, payload: &DockDragPayload) -> bool {
        self.payload_identity == payload.identity()
    }

    pub(crate) const fn pointer_capture(&self) -> PointerCaptureHandle {
        self.pointer_capture
    }
}

impl DockHostLiveSourceSemanticProxy {
    pub(crate) const fn key(&self) -> DockHostLivePresentationKey {
        self.key
    }

    pub(crate) const fn lease(&self) -> DockLiveUndockPayloadLeaseReceipt {
        self.lease
    }

    pub(crate) const fn carrier(&self) -> &DockLivePayloadCarrier {
        &self.carrier
    }

    pub(crate) const fn accessible_name(&self) -> &SharedString {
        &self.accessible_name
    }

    pub(crate) const fn source_focus(&self) -> Option<&DockLiveUndockSourceFocusSnapshot> {
        self.source_focus.as_ref()
    }
}

#[derive(Clone, Debug)]
pub(crate) struct DockHostPreparedLiveSourceRetirement {
    key: DockHostLivePresentationKey,
    source: view_presentation_window::LeaseBatch,
}

impl DockHostPreparedLiveSourceRetirement {
    pub(crate) const fn key(&self) -> DockHostLivePresentationKey {
        self.key
    }
}

#[derive(Clone, Debug)]
pub(crate) struct DockHostLiveSourceRetirementReceipt {
    key: DockHostLivePresentationKey,
    source: view_presentation_window::LeaseBatch,
}

impl DockHostLiveSourceRetirementReceipt {
    fn matches_prepared(&self, prepared: &DockHostPreparedLiveSourceRetirement) -> bool {
        self.key == prepared.key
            && self.source.window_id() == prepared.source.window_id()
            && self.source.leases() == prepared.source.leases()
    }

    fn matches_exactly(&self, other: &Self) -> bool {
        self.key == other.key
            && self.source.window_id() == other.source.window_id()
            && self.source.leases() == other.source.leases()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct DockHostPreparedLiveSourceSemanticRetirement {
    key: DockHostLivePresentationKey,
    lease: DockLiveUndockPayloadLeaseReceipt,
}

#[derive(Clone, Debug)]
pub(crate) struct DockHostPreparedLiveDestinationPromotion {
    key: DockHostLivePresentationKey,
    opening: crate::surface::live_undock::DockLiveUndockOpeningKey,
    token: DockLiveUndockPromotionToken,
    committed_surface_revision: u64,
    space: DockSpaceId,
    registration: DockViewportRegistrationKey,
    destination: view_presentation_window::LeaseBatch,
    window_facts: crate::DockViewportWindowFacts,
    host_geometry: crate::DockViewportHostGeometry,
}

impl DockHostPreparedLiveDestinationPromotion {
    pub(crate) fn host_geometry(&self) -> &crate::DockViewportHostGeometry {
        &self.host_geometry
    }
}

#[derive(Clone, Debug)]
pub(crate) struct DockHostLiveDestinationPromotionReceipt {
    semantics: DockHostLiveDestinationSemantics,
}

impl DockHostLiveDestinationPromotionReceipt {
    pub(crate) const fn semantics(&self) -> &DockHostLiveDestinationSemantics {
        &self.semantics
    }

    fn matches_prepared(&self, prepared: &DockHostPreparedLiveDestinationPromotion) -> bool {
        self.semantics.identity == prepared.key.identity
            && self.semantics.token == prepared.token
            && self.semantics.registration == prepared.registration
            && self.semantics.surface_revision == prepared.committed_surface_revision
            && self.semantics.destination.window_id() == prepared.destination.window_id()
            && self.semantics.destination.leases() == prepared.destination.leases()
    }

    fn matches_exactly(&self, other: &Self) -> bool {
        self.semantics.matches_exactly(&other.semantics)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DockHostLivePresentationStage {
    Source(DockHostLiveSourcePhase),
    DestinationStaging,
    DestinationExposed,
    DestinationPresented,
    DestinationRevealArmed,
    DestinationRevealObserving,
    DestinationRevealSettled,
    SourceRestoration(DockHostLiveSourceRestorationPhase),
}

#[derive(Clone, Debug)]
pub(crate) struct DockHostPreparedLivePresentationAbandonment {
    key: DockHostLivePresentationKey,
    stage: DockHostLivePresentationStage,
    publication: DockHostPresentationPublicationSnapshot,
}

impl DockHostPreparedLivePresentationAbandonment {
    pub(crate) const fn key(&self) -> DockHostLivePresentationKey {
        self.key
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct DockHostLivePresentationCleanupReceipt {
    key: DockHostLivePresentationKey,
}

impl DockHostLivePresentationCleanupReceipt {
    pub(crate) const fn key(self) -> DockHostLivePresentationKey {
        self.key
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DockHostRecoveryPresentationStage {
    Source(DockHostRecoverySourcePhase),
    Destination(DockHostRecoveryDestinationPhase),
    SourceRestoration(DockHostRecoverySourceRestorationPhase),
}

#[derive(Debug)]
pub(crate) struct DockHostPreparedPayloadRecoverySourceRetirement {
    key: DockHostRecoveryPresentationKey,
    source: view_presentation_window::LeaseBatch,
}

#[derive(Debug)]
pub(crate) struct DockHostPreparedPayloadRecoveryDestinationCommit {
    key: DockHostRecoveryPresentationKey,
    destination: view_presentation_window::LeaseBatch,
    presented: view_presentation_window::RehostDestinationPresentation,
}

#[derive(Clone, Debug)]
struct DockHostCommittedPayloadRecoverySourceRetirement {
    key: DockHostRecoveryPresentationKey,
    source: view_presentation_window::LeaseBatch,
}

#[derive(Clone, Debug)]
struct DockHostCommittedPayloadRecoveryDestination {
    key: DockHostRecoveryPresentationKey,
    destination: view_presentation_window::LeaseBatch,
}

#[derive(Debug)]
pub(crate) struct DockHostPreparedPayloadRecoverySourceRestorationCommit {
    key: DockHostRecoveryPresentationKey,
    source: view_presentation_window::LeaseBatch,
    presented: view_presentation_window::StableBatchPresentationReceipt,
}

#[derive(Debug)]
pub(crate) struct DockHostPreparedPayloadRecoveryPresentationAbandonment {
    key: DockHostRecoveryPresentationKey,
    stage: DockHostRecoveryPresentationStage,
    publication: DockHostPresentationPublicationSnapshot,
}

#[derive(Clone, Debug)]
struct DockHostPresentationPublicationSnapshot {
    logical: view_presentation_window::LeaseBatch,
    prior: Option<view_presentation_window::LeaseBatch>,
    // Render construction updates this journal before a candidate frame is accepted. These slots
    // bind abandonment to the exact Host-local observation; they are not presentation receipts.
    observed: Vec<Option<view_presentation_window::Lease>>,
}

impl DockHostPresentationPublicationSnapshot {
    fn capture(
        logical: view_presentation_window::LeaseBatch,
        prior: Option<view_presentation_window::LeaseBatch>,
        current: &HashMap<EntityId, view_presentation_window::Lease>,
    ) -> Self {
        let observed = logical
            .leases()
            .iter()
            .map(|lease| current.get(&lease.entity_id()).copied())
            .collect();
        Self {
            logical,
            prior,
            observed,
        }
    }

    fn matches_exactly(&self, other: &Self) -> bool {
        self.logical.matches_exactly(&other.logical)
            && match (&self.prior, &other.prior) {
                (Some(prior), Some(other_prior)) => prior.matches_exactly(other_prior),
                (None, None) => true,
                _ => false,
            }
            && self.observed == other.observed
    }

    fn remove_observed_owned(
        self,
        current: &mut HashMap<EntityId, view_presentation_window::Lease>,
    ) {
        for (logical_lease, observed) in self.logical.leases().iter().zip(self.observed) {
            let Some(observed) = observed else {
                continue;
            };
            let was_owned = observed == *logical_lease
                || self
                    .prior
                    .as_ref()
                    .and_then(|prior| prior.lease_for(logical_lease.entity_id()))
                    == Some(observed);
            if was_owned && current.get(&logical_lease.entity_id()) == Some(&observed) {
                current.remove(&logical_lease.entity_id());
            }
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct DockHostLiveDestinationSemantics {
    identity: DockLiveUndockIdentity,
    token: DockLiveUndockPromotionToken,
    binding: DockHostWindowBinding,
    registration: DockViewportRegistrationKey,
    surface_revision: u64,
    destination: view_presentation_window::LeaseBatch,
}

impl DockHostLiveDestinationSemantics {
    pub(crate) const fn identity(&self) -> DockLiveUndockIdentity {
        self.identity
    }

    pub(crate) const fn token(&self) -> DockLiveUndockPromotionToken {
        self.token
    }

    pub(crate) const fn binding(&self) -> DockHostWindowBinding {
        self.binding
    }

    pub(crate) fn registration(&self) -> &DockViewportRegistrationKey {
        &self.registration
    }

    pub(crate) const fn surface_revision(&self) -> u64 {
        self.surface_revision
    }

    pub(crate) fn destination(&self) -> &view_presentation_window::LeaseBatch {
        &self.destination
    }

    fn matches_exactly(&self, other: &Self) -> bool {
        self.identity == other.identity
            && self.token == other.token
            && self.binding == other.binding
            && self.registration == other.registration
            && self.surface_revision == other.surface_revision
            && self.destination.window_id() == other.destination.window_id()
            && self.destination.leases() == other.destination.leases()
    }
}

/// Static host rendering options.
#[derive(Debug, Clone)]
pub struct DockHostOptions {
    /// Message rendered when the selected dock space has no root node.
    pub empty_message: String,
    /// Message prefix rendered when a selected panel is missing from the registry.
    pub missing_panel_prefix: String,
    /// Minimum rendered size for a split pane during splitter resizing.
    pub split_min_size: Pixels,
    /// Hit target and visual thickness for rendered splitter handles.
    pub splitter_handle_size: Pixels,
    /// Structural metrics used to size and hit-test dock drop guides.
    pub drop_guide_metrics: DockDropGuideMetrics,
    /// Host-owned motion preference applied before constructing docking transition specs.
    pub motion_preference: MotionPreference,
}

impl Default for DockHostOptions {
    fn default() -> Self {
        Self {
            empty_message: "Empty dock space".to_string(),
            missing_panel_prefix: "Missing panel".to_string(),
            split_min_size: px(96.0),
            splitter_handle_size: px(6.0),
            drop_guide_metrics: DockDropGuideMetrics::default(),
            motion_preference: MotionPreference::Animated,
        }
    }
}

/// Retained GPUI host that renders one logical dock workspace.
///
/// `DockHost` is the GPUI render adapter for a dock space. Durable graph state belongs to
/// [`model::DockWorkspace`](crate::model::DockWorkspace) or
/// [`model::DockController`](crate::model::DockController), while transient pointer sessions are
/// kept behind the crate's interaction runtime.
#[derive(Debug)]
pub struct DockHost {
    controller: Entity<DockController>,
    surface_owner: Option<Entity<DockSurfaceOwner>>,
    role: DockHostRole,
    live_presentation: Option<DockHostPresentationState>,
    committed_live_presentation_abandonment: Option<DockHostLivePresentationCleanupReceipt>,
    committed_live_source_retirement: Option<DockHostLiveSourceRetirementReceipt>,
    committed_live_destination_promotion: Option<DockHostLiveDestinationPromotionReceipt>,
    committed_payload_recovery_source_retirement:
        Option<DockHostCommittedPayloadRecoverySourceRetirement>,
    committed_payload_recovery_destination: Option<DockHostCommittedPayloadRecoveryDestination>,
    live_source_semantic_proxy: Option<DockHostLiveSourceSemanticProxy>,
    native_drag_transport_proxy: Option<DockHostNativeDragTransportProxy>,
    live_destination_semantics: Option<DockHostLiveDestinationSemantics>,
    live_presentation_epoch: u64,
    panel_presentation_leases: HashMap<EntityId, view_presentation_window::Lease>,
    surface_activation_registration: Option<DockSurfaceActivationHostRegistration>,
    space: DockSpaceId,
    focus_handle: FocusHandle,
    viewport_runtime: DockViewportRuntimeHandle,
    visual_style_resolver: Option<DockVisualStyleResolver>,
    fallback_visual_style: Rc<DockVisualStyle>,
    bound_window_id: Option<WindowId>,
    bound_viewport_registration: Option<DockViewportRegistrationKey>,
    window_binding_generation: u64,
    viewport_scene_publication: PrepaintPublicationId,
    raw_drag_pointer_capture: Option<PointerCaptureHandle>,
    viewport_activation_subscription: Option<Subscription>,
    viewport_bounds_subscription: Option<Subscription>,
    viewport_release_subscription: Option<Subscription>,
    panel_focus_trackers: HashMap<DockItemId, DockPanelFocusTracker>,
    pending_focus_completion: Option<DockPendingFocusCompletion>,
    pending_recovery_entry_focus_completion: Option<DockPendingRecoveryEntryFocusCompletion>,
    pending_recovery_restore_focus: Option<DockPendingRecoveryRestoreFocus>,
    #[cfg(test)]
    debug: DockDebugInstrumentation,
    #[cfg(test)]
    pub(crate) debug_recording_suppression_depth: usize,
    #[cfg(test)]
    last_resolved_visual_style: Option<Rc<DockVisualStyle>>,
    #[cfg(test)]
    reject_next_payload_recovery_source_install: bool,
    interaction: DockInteractionRuntime,
    zoom: DockZoomState,
    transitions: DockTransitionExecutor,
    visual_affordance_transitions: DockTransitionExecutor,
    last_visual_affordance_scene: Option<DockVisualAffordanceScene>,
    last_presentation_scene: Option<DockPresentationScene>,
}

impl DockHost {
    /// Creates a host that renders one dock space from a shared controller and viewport runtime.
    pub fn from_controller(
        controller: Entity<DockController>,
        space: impl Into<DockSpaceId>,
        viewport_runtime: DockViewportRuntimeHandle,
        cx: &mut Context<Self>,
    ) -> Self {
        let visual_style_resolver = viewport_runtime.visual_style_resolver();
        Self::from_controller_with_optional_visual_style_resolver(
            controller,
            space,
            viewport_runtime,
            visual_style_resolver,
            None,
            DockHostRole::Unmanaged,
            cx,
        )
    }

    /// Creates a low-level host with an explicit immutable visual-style resolver.
    pub fn from_controller_with_visual_style_resolver(
        controller: Entity<DockController>,
        space: impl Into<DockSpaceId>,
        viewport_runtime: DockViewportRuntimeHandle,
        visual_style_resolver: DockVisualStyleResolver,
        cx: &mut Context<Self>,
    ) -> Self {
        Self::from_controller_with_optional_visual_style_resolver(
            controller,
            space,
            viewport_runtime,
            Some(visual_style_resolver),
            None,
            DockHostRole::Unmanaged,
            cx,
        )
    }

    pub(crate) fn from_embedded_surface_owner(
        controller: Entity<DockController>,
        space: impl Into<DockSpaceId>,
        viewport_runtime: DockViewportRuntimeHandle,
        surface_owner: &Entity<DockSurfaceOwner>,
        cx: &mut Context<Self>,
    ) -> Self {
        let visual_style_resolver = viewport_runtime.visual_style_resolver();
        Self::from_controller_with_optional_visual_style_resolver(
            controller,
            space,
            viewport_runtime,
            visual_style_resolver,
            Some(surface_owner.clone()),
            DockHostRole::Embedded,
            cx,
        )
    }

    pub(crate) fn from_opening_primary_surface_owner(
        controller: Entity<DockController>,
        space: impl Into<DockSpaceId>,
        viewport_runtime: DockViewportRuntimeHandle,
        surface_owner: &Entity<DockSurfaceOwner>,
        opening: DockSurfaceWindowSessionOpeningToken,
        cx: &mut Context<Self>,
    ) -> Self {
        let visual_style_resolver = viewport_runtime.visual_style_resolver();
        Self::from_controller_with_optional_visual_style_resolver(
            controller,
            space,
            viewport_runtime,
            visual_style_resolver,
            Some(surface_owner.clone()),
            DockHostRole::PrimaryAnchor(DockHostPrimaryAnchorAuthority::Opening(opening)),
            cx,
        )
    }

    pub(crate) fn from_managed_surface_owner(
        controller: Entity<DockController>,
        space: impl Into<DockSpaceId>,
        viewport_runtime: DockViewportRuntimeHandle,
        surface_owner: &Entity<DockSurfaceOwner>,
        lease: DockSurfaceWindowSessionLease,
        cx: &mut Context<Self>,
    ) -> Self {
        let visual_style_resolver = viewport_runtime.visual_style_resolver();
        Self::from_controller_with_optional_visual_style_resolver(
            controller,
            space,
            viewport_runtime,
            visual_style_resolver,
            Some(surface_owner.clone()),
            DockHostRole::ManagedViewport(lease),
            cx,
        )
    }

    pub(crate) fn from_provisional_surface_owner(
        controller: Entity<DockController>,
        space: impl Into<DockSpaceId>,
        viewport_runtime: DockViewportRuntimeHandle,
        surface_owner: &Entity<DockSurfaceOwner>,
        opening: crate::surface::live_undock::DockLiveUndockOpeningKey,
        cx: &mut Context<Self>,
    ) -> Self {
        let visual_style_resolver = viewport_runtime.visual_style_resolver();
        Self::from_controller_with_optional_visual_style_resolver(
            controller,
            space,
            viewport_runtime,
            visual_style_resolver,
            Some(surface_owner.clone()),
            DockHostRole::ProvisionalViewport(opening),
            cx,
        )
    }

    fn from_controller_with_optional_visual_style_resolver(
        controller: Entity<DockController>,
        space: impl Into<DockSpaceId>,
        viewport_runtime: DockViewportRuntimeHandle,
        visual_style_resolver: Option<DockVisualStyleResolver>,
        surface_owner: Option<Entity<DockSurfaceOwner>>,
        role: DockHostRole,
        cx: &mut Context<Self>,
    ) -> Self {
        crate::native_captured_drag::ensure_native_captured_drag_router(cx);
        cx.observe(&controller, |_, _, cx| cx.notify()).detach();
        if let Some(surface_owner) = surface_owner.as_ref() {
            cx.observe(&surface_owner, |_, _, cx| cx.notify()).detach();
        }
        Self {
            controller,
            surface_owner,
            role,
            live_presentation: None,
            committed_live_presentation_abandonment: None,
            committed_live_source_retirement: None,
            committed_live_destination_promotion: None,
            committed_payload_recovery_source_retirement: None,
            committed_payload_recovery_destination: None,
            live_source_semantic_proxy: None,
            native_drag_transport_proxy: None,
            live_destination_semantics: None,
            live_presentation_epoch: 0,
            panel_presentation_leases: HashMap::new(),
            surface_activation_registration: None,
            space: space.into(),
            focus_handle: cx.focus_handle(),
            viewport_runtime,
            visual_style_resolver,
            fallback_visual_style: Rc::new(DockVisualStyle::built_in()),
            bound_window_id: None,
            bound_viewport_registration: None,
            window_binding_generation: 0,
            viewport_scene_publication: PrepaintPublicationId::new(),
            raw_drag_pointer_capture: None,
            viewport_activation_subscription: None,
            viewport_bounds_subscription: None,
            viewport_release_subscription: None,
            panel_focus_trackers: HashMap::new(),
            pending_focus_completion: None,
            pending_recovery_entry_focus_completion: None,
            pending_recovery_restore_focus: None,
            #[cfg(test)]
            debug: DockDebugInstrumentation::default(),
            #[cfg(test)]
            debug_recording_suppression_depth: 0,
            #[cfg(test)]
            last_resolved_visual_style: None,
            #[cfg(test)]
            reject_next_payload_recovery_source_install: false,
            interaction: DockInteractionRuntime::default(),
            zoom: DockZoomState::default(),
            transitions: DockTransitionExecutor::default(),
            visual_affordance_transitions: DockTransitionExecutor::default(),
            last_visual_affordance_scene: None,
            last_presentation_scene: None,
        }
    }

    pub(crate) fn promote_primary_anchor(
        &mut self,
        opening: DockSurfaceWindowSessionOpeningToken,
        lease: DockSurfaceWindowSessionLease,
        anchor: WindowId,
        cx: &mut Context<Self>,
    ) -> bool {
        if !self.can_promote_primary_anchor(opening, anchor) || !lease.activates(opening, anchor) {
            return false;
        }
        self.role = DockHostRole::PrimaryAnchor(DockHostPrimaryAnchorAuthority::Active(lease));
        cx.notify();
        true
    }

    pub(crate) fn can_promote_primary_anchor(
        &self,
        opening: DockSurfaceWindowSessionOpeningToken,
        anchor: WindowId,
    ) -> bool {
        self.role == DockHostRole::PrimaryAnchor(DockHostPrimaryAnchorAuthority::Opening(opening))
            && self.bound_window_id == Some(anchor)
    }

    pub(crate) fn runtime_lineage(&self, cx: &Context<Self>) -> Option<DockViewportRuntimeLineage> {
        match self.role {
            DockHostRole::Unmanaged => Some(DockViewportRuntimeLineage::Unmanaged),
            DockHostRole::Embedded
            | DockHostRole::PrimaryAnchor(DockHostPrimaryAnchorAuthority::Opening(_))
            | DockHostRole::ProvisionalViewport(_) => None,
            DockHostRole::PrimaryAnchor(DockHostPrimaryAnchorAuthority::Active(lease))
            | DockHostRole::ManagedViewport(lease) => self
                .surface_owner
                .as_ref()
                .is_some_and(|owner| {
                    cx.read_entity(owner, |owner, _| owner.window_session().admits(lease))
                })
                .then_some(DockViewportRuntimeLineage::Surface(lease)),
        }
    }

    pub(crate) const fn is_provisional_viewport(&self) -> bool {
        matches!(self.role, DockHostRole::ProvisionalViewport(_))
    }

    pub(crate) fn is_provisional_viewport_for(
        &self,
        opening: crate::surface::live_undock::DockLiveUndockOpeningKey,
    ) -> bool {
        matches!(self.role, DockHostRole::ProvisionalViewport(current) if current == opening)
    }

    pub(crate) fn live_presentation_session(&self) -> Option<&DockHostPresentationSession> {
        self.live_presentation
            .as_ref()
            .map(DockHostPresentationState::presentation)
    }

    pub(crate) fn live_presentation_state(&self) -> Option<DockHostLivePresentationState> {
        self.live_presentation
            .as_ref()
            .and_then(DockHostPresentationState::as_live)
            .cloned()
    }

    #[cfg(test)]
    pub(crate) fn live_destination_geometry_for_test(
        &self,
    ) -> Option<(
        crate::viewport_registry::DockViewportWindowBoundsFrame,
        open_gpui::Bounds<Pixels>,
    )> {
        let DockHostLivePresentationMode::DestinationProjection {
            accepted_geometry: Some(geometry),
            ..
        } = &self.live_presentation.as_ref()?.as_live()?.mode
        else {
            return None;
        };
        Some((
            geometry.current_bounds,
            geometry.host_geometry.layout_bounds(),
        ))
    }

    pub(crate) fn payload_recovery_presentation_state(
        &self,
    ) -> Option<DockHostRecoveryPresentationState> {
        self.live_presentation
            .as_ref()
            .and_then(DockHostPresentationState::as_recovery)
            .cloned()
    }

    pub(crate) fn live_source_semantic_proxy(&self) -> Option<DockHostLiveSourceSemanticProxy> {
        self.live_source_semantic_proxy.clone()
    }

    pub(crate) fn native_drag_transport_proxy(&self) -> Option<DockHostNativeDragTransportProxy> {
        self.native_drag_transport_proxy
            .as_ref()
            .filter(|proxy| proxy.is_active())
            .cloned()
    }

    pub(crate) fn has_native_drag_transport_proxy_key(
        &self,
        key: crate::native_captured_drag::DockNativeCapturedDragTransportKey,
    ) -> bool {
        self.native_drag_transport_proxy
            .as_ref()
            .is_some_and(|proxy| proxy.key() == key)
    }

    #[cfg(test)]
    pub(crate) const fn has_native_drag_transport_proxy_slot_for_test(&self) -> bool {
        self.native_drag_transport_proxy.is_some()
    }

    pub(crate) fn native_drag_transport_suppresses_payload(
        &self,
        payload: &DockDragPayload,
    ) -> bool {
        self.native_drag_transport_proxy
            .as_ref()
            .is_some_and(|proxy| proxy.is_active() && proxy.matches_payload(payload))
    }

    pub(crate) fn accepts_live_source_semantic_proxy(
        &self,
        key: DockHostLivePresentationKey,
        lease: DockLiveUndockPayloadLeaseReceipt,
    ) -> bool {
        self.accepts_bound_window(Some(key.binding))
            && self
                .live_source_semantic_proxy
                .as_ref()
                .is_some_and(|proxy| {
                    proxy.key == key && proxy.lease == lease && lease.identity() == key.identity
                })
    }

    fn clear_live_source_semantic_proxy_for_key(&mut self, key: DockHostLivePresentationKey) {
        if self
            .live_source_semantic_proxy
            .as_ref()
            .is_some_and(|proxy| proxy.key == key)
        {
            self.live_source_semantic_proxy = None;
        }
    }

    pub(crate) fn install_native_drag_transport_proxy(
        &mut self,
        transport: crate::native_captured_drag::DockNativeCapturedDragTransportLease,
        payload: DockDragPayload,
        pointer_capture: PointerCaptureHandle,
        cx: &mut Context<Self>,
    ) -> bool {
        let key = transport.key();
        if !transport.is_active()
            || pointer_capture.window_id() != key.source_window()
            || !self.accepts_bound_window(Some(key.source_binding()))
            || self.viewport_runtime.identity() != key.runtime_identity()
        {
            return false;
        }
        let previous = self
            .native_drag_transport_proxy
            .replace(DockHostNativeDragTransportProxy {
                transport,
                payload_identity: payload.identity(),
                pointer_capture,
            });
        if let Some(previous) = previous {
            previous.transport.retire();
        }
        cx.notify();
        true
    }

    pub(crate) fn retire_native_drag_transport_proxy(
        &mut self,
        key: crate::native_captured_drag::DockNativeCapturedDragTransportKey,
        cx: &mut Context<Self>,
    ) -> bool {
        if self
            .native_drag_transport_proxy
            .as_ref()
            .is_none_or(|proxy| proxy.key() != key)
        {
            return false;
        }
        if let Some(proxy) = self.native_drag_transport_proxy.take() {
            proxy.transport.retire();
        }
        cx.notify();
        true
    }

    fn next_live_presentation_key(
        &mut self,
        expected_binding: DockHostWindowBinding,
        identity: DockLiveUndockIdentity,
        rehost_generation: u64,
    ) -> Option<DockHostLivePresentationKey> {
        if rehost_generation == 0
            || !self.accepts_bound_window(Some(expected_binding))
            || self.live_presentation.is_some()
        {
            return None;
        }
        self.live_presentation_epoch = self.live_presentation_epoch.wrapping_add(1).max(1);
        Some(DockHostLivePresentationKey {
            identity,
            rehost_generation,
            binding: expected_binding,
            epoch: self.live_presentation_epoch,
        })
    }

    fn next_payload_recovery_presentation_key(
        &mut self,
        expected_binding: DockHostWindowBinding,
        action: DockPayloadRecoveryRestoreAction,
        rehost_generation: u64,
    ) -> Option<DockHostRecoveryPresentationKey> {
        if rehost_generation == 0
            || !self.accepts_bound_window(Some(expected_binding))
            || self.live_presentation.is_some()
        {
            return None;
        }
        self.live_presentation_epoch = self.live_presentation_epoch.wrapping_add(1).max(1);
        Some(DockHostRecoveryPresentationKey {
            action,
            rehost_generation,
            binding: expected_binding,
            epoch: self.live_presentation_epoch,
        })
    }

    fn recovery_roots_cover_batch(
        roots: &[AnyView],
        batch: &view_presentation_window::LeaseBatch,
    ) -> bool {
        let mut seen = std::collections::HashSet::with_capacity(roots.len());
        roots.iter().all(|root| seen.insert(root.entity_id()))
            && batch
                .leases()
                .iter()
                .all(|lease| seen.contains(&lease.entity_id()))
    }

    pub(crate) fn accepts_live_presentation_key(&self, key: DockHostLivePresentationKey) -> bool {
        self.accepts_bound_window(Some(key.binding))
            && self
                .live_presentation
                .as_ref()
                .and_then(DockHostPresentationState::as_live)
                .is_some_and(|state| state.key == key)
    }

    pub(crate) fn accepts_payload_recovery_presentation_key(
        &self,
        key: DockHostRecoveryPresentationKey,
    ) -> bool {
        self.accepts_bound_window(Some(key.binding))
            && self
                .live_presentation
                .as_ref()
                .and_then(DockHostPresentationState::as_recovery)
                .is_some_and(|state| state.key == key)
    }

    pub(crate) fn install_live_source_projection(
        &mut self,
        expected_binding: DockHostWindowBinding,
        identity: DockLiveUndockIdentity,
        lease: DockLiveUndockPayloadLeaseReceipt,
        presentation: DockHostPresentationSession,
        projection: view_presentation_window::RehostProjection,
        retained: retained_visual::Ticket,
        carrier: DockLivePayloadCarrier,
        accessible_name: SharedString,
        source_focus: Option<DockLiveUndockSourceFocusSnapshot>,
        cx: &mut Context<Self>,
    ) -> Option<DockHostLivePresentationKey> {
        if lease.identity() != identity
            || lease.rehost_generation() != projection.generation()
            || lease.source().window_id() != expected_binding.window_id()
            || projection.source().window_id() != expected_binding.window_id()
            || lease.retained_visual() != Some(retained.identity())
            || retained.source_window() != expected_binding.window_id()
            || retained.bounds() != carrier.bounds
            || self.live_source_semantic_proxy.is_some()
        {
            return None;
        }
        let key =
            self.next_live_presentation_key(expected_binding, identity, projection.generation())?;
        self.live_presentation = Some(DockHostPresentationState::Live(
            DockHostLivePresentationState {
                key,
                presentation,
                mode: DockHostLivePresentationMode::SourceProjection {
                    lease,
                    projection,
                    retained,
                    carrier: carrier.clone(),
                    phase: DockHostLiveSourcePhase::Releasing,
                },
            },
        ));
        self.live_source_semantic_proxy = Some(DockHostLiveSourceSemanticProxy {
            key,
            lease,
            carrier,
            accessible_name,
            source_focus,
        });
        cx.notify();
        Some(key)
    }

    pub(crate) fn install_live_destination_projection(
        &mut self,
        expected_binding: DockHostWindowBinding,
        identity: DockLiveUndockIdentity,
        proxy: DockLiveUndockSourceProxyReceipt,
        presentation: DockHostPresentationSession,
        projection: view_presentation_window::RehostProjection,
        leases: view_presentation_window::LeaseBatch,
        cx: &mut Context<Self>,
    ) -> Option<DockHostLivePresentationKey> {
        if self.role != DockHostRole::ProvisionalViewport(identity.opening())
            || presentation.space() != &self.space
            || proxy.lease().identity() != identity
            || proxy.lease().rehost_generation() != projection.generation()
            || leases.window_id() != expected_binding.window_id()
            || projection.destination().window_id() != expected_binding.window_id()
            || leases.leases() != projection.destination().leases()
        {
            return None;
        }
        let key =
            self.next_live_presentation_key(expected_binding, identity, projection.generation())?;
        self.live_presentation = Some(DockHostPresentationState::Live(
            DockHostLivePresentationState {
                key,
                presentation,
                mode: DockHostLivePresentationMode::DestinationProjection {
                    proxy,
                    projection,
                    leases,
                    accepted_geometry: None,
                    phase: DockHostLiveDestinationPhase::Staging,
                },
            },
        ));
        cx.notify();
        Some(key)
    }

    pub(crate) fn install_payload_recovery_source_projection(
        &mut self,
        expected_binding: DockHostWindowBinding,
        action: DockPayloadRecoveryRestoreAction,
        presentation: DockHostPresentationSession,
        projection: view_presentation_window::RehostProjection,
        cx: &mut Context<Self>,
    ) -> Option<DockHostRecoveryPresentationKey> {
        if presentation.kind()
            != crate::host_render_session::DockHostPresentationKind::PayloadRecoveryProjection
            || presentation.space() != &self.space
            || projection.source().window_id() != expected_binding.window_id()
        {
            return None;
        }
        #[cfg(test)]
        if std::mem::take(&mut self.reject_next_payload_recovery_source_install) {
            return None;
        }
        let key = self.next_payload_recovery_presentation_key(
            expected_binding,
            action,
            projection.generation(),
        )?;
        self.live_presentation = Some(DockHostPresentationState::Recovery(
            DockHostRecoveryPresentationState {
                key,
                presentation,
                mode: DockHostRecoveryPresentationMode::SourceProjection {
                    projection,
                    phase: DockHostRecoverySourcePhase::Releasing,
                },
            },
        ));
        self.last_visual_affordance_scene = None;
        self.last_presentation_scene = None;
        cx.notify();
        Some(key)
    }

    #[cfg(test)]
    pub(crate) fn reject_next_payload_recovery_source_install_for_test(&mut self) {
        self.reject_next_payload_recovery_source_install = true;
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn install_payload_recovery_destination_projection(
        &mut self,
        expected_binding: DockHostWindowBinding,
        action: DockPayloadRecoveryRestoreAction,
        presentation: DockHostPresentationSession,
        projection: view_presentation_window::RehostProjection,
        leases: view_presentation_window::LeaseBatch,
        resolved_roots: Vec<AnyView>,
        cx: &mut Context<Self>,
    ) -> Option<DockHostRecoveryPresentationKey> {
        if !matches!(
            self.role,
            DockHostRole::PrimaryAnchor(DockHostPrimaryAnchorAuthority::Active(anchor))
                if anchor == action.anchor_lease()
        ) || presentation.kind()
            != crate::host_render_session::DockHostPresentationKind::PayloadRecoveryProjection
            || presentation.space() != &self.space
            || leases.window_id() != expected_binding.window_id()
            || projection.destination().window_id() != expected_binding.window_id()
            || leases.window_id() != projection.destination().window_id()
            || leases.leases() != projection.destination().leases()
            || !Self::recovery_roots_cover_batch(&resolved_roots, &leases)
        {
            return None;
        }
        let key = self.next_payload_recovery_presentation_key(
            expected_binding,
            action,
            projection.generation(),
        )?;
        self.live_presentation = Some(DockHostPresentationState::Recovery(
            DockHostRecoveryPresentationState {
                key,
                presentation,
                mode: DockHostRecoveryPresentationMode::DestinationProjection {
                    projection,
                    leases,
                    resolved_roots,
                    phase: DockHostRecoveryDestinationPhase::AwaitingSourceRelease,
                },
            },
        ));
        self.last_visual_affordance_scene = None;
        self.last_presentation_scene = None;
        cx.notify();
        Some(key)
    }

    pub(crate) fn mark_payload_recovery_source_frozen(
        &mut self,
        key: DockHostRecoveryPresentationKey,
        cx: &mut Context<Self>,
    ) -> bool {
        if !self.accepts_payload_recovery_presentation_key(key) {
            return false;
        }
        let Some(DockHostRecoveryPresentationState {
            mode: DockHostRecoveryPresentationMode::SourceProjection { phase, .. },
            ..
        }) = self
            .live_presentation
            .as_mut()
            .and_then(DockHostPresentationState::as_recovery_mut)
        else {
            return false;
        };
        if *phase != DockHostRecoverySourcePhase::Releasing {
            return false;
        }
        *phase = DockHostRecoverySourcePhase::Frozen;
        cx.notify();
        true
    }

    pub(crate) fn arm_payload_recovery_destination_projection(
        &mut self,
        key: DockHostRecoveryPresentationKey,
        cx: &mut Context<Self>,
    ) -> bool {
        if !self.accepts_payload_recovery_presentation_key(key) {
            return false;
        }
        let Some(DockHostRecoveryPresentationState {
            mode: DockHostRecoveryPresentationMode::DestinationProjection { phase, .. },
            ..
        }) = self
            .live_presentation
            .as_mut()
            .and_then(DockHostPresentationState::as_recovery_mut)
        else {
            return false;
        };
        if *phase == DockHostRecoveryDestinationPhase::Staging {
            return true;
        }
        if *phase != DockHostRecoveryDestinationPhase::AwaitingSourceRelease {
            return false;
        }
        *phase = DockHostRecoveryDestinationPhase::Staging;
        cx.notify();
        true
    }

    pub(crate) fn expose_payload_recovery_destination_projection(
        &mut self,
        key: DockHostRecoveryPresentationKey,
        exposure: view_presentation_window::RehostDestinationExposure,
        cx: &mut Context<Self>,
    ) -> bool {
        if !self.accepts_payload_recovery_presentation_key(key) {
            return false;
        }
        let Some(DockHostRecoveryPresentationState {
            mode:
                DockHostRecoveryPresentationMode::DestinationProjection {
                    projection,
                    leases: current,
                    phase,
                    ..
                },
            ..
        }) = self
            .live_presentation
            .as_mut()
            .and_then(DockHostPresentationState::as_recovery_mut)
        else {
            return false;
        };
        let leases = exposure.batch();
        if *phase != DockHostRecoveryDestinationPhase::Staging
            || projection.generation() != key.rehost_generation()
            || exposure.frame_generation() == 0
            || leases.window_id() != key.binding().window_id()
            || current.window_id() != leases.window_id()
            || current.leases() != leases.leases()
        {
            return false;
        }
        *phase = DockHostRecoveryDestinationPhase::Exposed;
        cx.notify();
        true
    }

    pub(crate) fn begin_payload_recovery_source_restoration(
        &mut self,
        key: DockHostRecoveryPresentationKey,
        presentation: DockHostPresentationSession,
        leases: view_presentation_window::LeaseBatch,
        resolved_roots: Vec<AnyView>,
        cx: &mut Context<Self>,
    ) -> DockHostRecoverySourceRestorationInstallOutcome {
        if !self.accepts_payload_recovery_presentation_key(key)
            || presentation.kind()
                != crate::host_render_session::DockHostPresentationKind::PayloadRecoveryProjection
            || presentation.space() != &self.space
            || leases.window_id() != key.binding().window_id()
            || !Self::recovery_roots_cover_batch(&resolved_roots, &leases)
        {
            return DockHostRecoverySourceRestorationInstallOutcome::PresentationAuthorityLost;
        }
        let Some(state) = self
            .live_presentation
            .as_mut()
            .and_then(DockHostPresentationState::as_recovery_mut)
        else {
            return DockHostRecoverySourceRestorationInstallOutcome::PresentationAuthorityLost;
        };
        if matches!(
            &state.mode,
            DockHostRecoveryPresentationMode::SourceRestoration {
                projection,
                leases: current,
                resolved_roots: current_roots,
                ..
            } if projection.generation() == key.rehost_generation()
                && current.window_id() == leases.window_id()
                && current.leases() == leases.leases()
                && current_roots.iter().map(AnyView::entity_id).eq(
                    resolved_roots.iter().map(AnyView::entity_id)
                )
        ) {
            return DockHostRecoverySourceRestorationInstallOutcome::AlreadyInstalled;
        }
        let DockHostRecoveryPresentationMode::SourceProjection {
            projection,
            phase: DockHostRecoverySourcePhase::Frozen,
        } = &state.mode
        else {
            return DockHostRecoverySourceRestorationInstallOutcome::PresentationAuthorityLost;
        };
        if projection.generation() != key.rehost_generation()
            || projection.source().window_id() != leases.window_id()
            || projection.source().leases().len() != leases.leases().len()
            || projection
                .source()
                .leases()
                .iter()
                .any(|lease| leases.lease_for(lease.entity_id()).is_none())
        {
            return DockHostRecoverySourceRestorationInstallOutcome::PresentationAuthorityLost;
        }
        state.presentation = presentation;
        state.mode = DockHostRecoveryPresentationMode::SourceRestoration {
            projection: projection.clone(),
            leases,
            resolved_roots,
            phase: DockHostRecoverySourceRestorationPhase::Staging,
        };
        self.last_visual_affordance_scene = None;
        self.last_presentation_scene = None;
        cx.notify();
        DockHostRecoverySourceRestorationInstallOutcome::Installed
    }

    pub(crate) fn mark_payload_recovery_source_restoration_visible_pending(
        &mut self,
        key: DockHostRecoveryPresentationKey,
        leases: &view_presentation_window::LeaseBatch,
        cx: &mut Context<Self>,
    ) -> bool {
        if !self.accepts_payload_recovery_presentation_key(key) {
            return false;
        }
        let Some(DockHostRecoveryPresentationState {
            mode:
                DockHostRecoveryPresentationMode::SourceRestoration {
                    leases: current,
                    phase,
                    ..
                },
            ..
        }) = self
            .live_presentation
            .as_mut()
            .and_then(DockHostPresentationState::as_recovery_mut)
        else {
            return false;
        };
        if current.window_id() != leases.window_id() || current.leases() != leases.leases() {
            return false;
        }
        if *phase == DockHostRecoverySourceRestorationPhase::AwaitingVisibleFrame {
            return true;
        }
        if *phase != DockHostRecoverySourceRestorationPhase::Staging {
            return false;
        }
        *phase = DockHostRecoverySourceRestorationPhase::AwaitingVisibleFrame;
        cx.notify();
        true
    }

    pub(crate) fn prepare_payload_recovery_source_retirement(
        &self,
        key: DockHostRecoveryPresentationKey,
    ) -> Option<DockHostPreparedPayloadRecoverySourceRetirement> {
        if !self.accepts_payload_recovery_presentation_key(key) {
            return None;
        }
        let DockHostRecoveryPresentationState {
            mode:
                DockHostRecoveryPresentationMode::SourceProjection {
                    projection,
                    phase: DockHostRecoverySourcePhase::Frozen,
                },
            ..
        } = self
            .live_presentation
            .as_ref()
            .and_then(DockHostPresentationState::as_recovery)?
        else {
            return None;
        };
        if projection.generation() != key.rehost_generation()
            || projection.source().window_id() != key.binding().window_id()
        {
            return None;
        }
        Some(DockHostPreparedPayloadRecoverySourceRetirement {
            key,
            source: projection.source().clone(),
        })
    }

    pub(crate) fn can_commit_prepared_payload_recovery_source_retirement(
        &self,
        prepared: &DockHostPreparedPayloadRecoverySourceRetirement,
    ) -> bool {
        let Some(current) = self.prepare_payload_recovery_source_retirement(prepared.key) else {
            return false;
        };
        current.source.window_id() == prepared.source.window_id()
            && current.source.leases() == prepared.source.leases()
    }

    pub(crate) fn payload_recovery_source_retirement_is_committed(
        &self,
        key: DockHostRecoveryPresentationKey,
        source: &view_presentation_window::LeaseBatch,
    ) -> bool {
        self.committed_payload_recovery_source_retirement
            .as_ref()
            .is_some_and(|committed| {
                committed.key == key && committed.source.matches_exactly(source)
            })
    }

    pub(crate) fn commit_prepared_payload_recovery_source_retirement(
        &mut self,
        prepared: DockHostPreparedPayloadRecoverySourceRetirement,
        cx: &mut Context<Self>,
    ) {
        assert!(
            self.can_commit_prepared_payload_recovery_source_retirement(&prepared),
            "prepared payload-recovery source authority must remain exact until commit"
        );
        self.live_presentation = None;
        self.panel_presentation_leases
            .retain(|_, lease| !prepared.source.leases().contains(lease));
        self.committed_payload_recovery_source_retirement =
            Some(DockHostCommittedPayloadRecoverySourceRetirement {
                key: prepared.key,
                source: prepared.source,
            });
        self.last_visual_affordance_scene = None;
        self.last_presentation_scene = None;
        cx.notify();
    }

    pub(crate) fn prepare_payload_recovery_destination_commit(
        &self,
        key: DockHostRecoveryPresentationKey,
        presented: view_presentation_window::RehostDestinationPresentation,
    ) -> Option<DockHostPreparedPayloadRecoveryDestinationCommit> {
        if !self.accepts_payload_recovery_presentation_key(key) {
            return None;
        }
        let DockHostRecoveryPresentationState {
            mode:
                DockHostRecoveryPresentationMode::DestinationProjection {
                    projection,
                    leases,
                    phase: DockHostRecoveryDestinationPhase::Exposed,
                    ..
                },
            ..
        } = self
            .live_presentation
            .as_ref()
            .and_then(DockHostPresentationState::as_recovery)?
        else {
            return None;
        };
        let generation = leases.leases().first()?.generation();
        if projection.generation() != key.rehost_generation()
            || projection.destination().window_id() != leases.window_id()
            || projection.destination().leases() != leases.leases()
            || presented.window_id() != leases.window_id()
            || presented.lease_generation() != generation
            || presented.root_count() != leases.leases().len()
            || leases
                .leases()
                .iter()
                .any(|lease| self.panel_presentation_leases.get(&lease.entity_id()) != Some(lease))
        {
            return None;
        }
        Some(DockHostPreparedPayloadRecoveryDestinationCommit {
            key,
            destination: leases.clone(),
            presented,
        })
    }

    pub(crate) fn can_commit_prepared_payload_recovery_destination(
        &self,
        prepared: &DockHostPreparedPayloadRecoveryDestinationCommit,
    ) -> bool {
        let Some(current) =
            self.prepare_payload_recovery_destination_commit(prepared.key, prepared.presented)
        else {
            return false;
        };
        current.destination.window_id() == prepared.destination.window_id()
            && current.destination.leases() == prepared.destination.leases()
    }

    pub(crate) fn payload_recovery_destination_is_committed(
        &self,
        key: DockHostRecoveryPresentationKey,
        destination: &view_presentation_window::LeaseBatch,
    ) -> bool {
        self.committed_payload_recovery_destination
            .as_ref()
            .is_some_and(|committed| {
                committed.key == key && committed.destination.matches_exactly(destination)
            })
    }

    pub(crate) fn commit_prepared_payload_recovery_destination(
        &mut self,
        prepared: DockHostPreparedPayloadRecoveryDestinationCommit,
        cx: &mut Context<Self>,
    ) {
        assert!(
            self.can_commit_prepared_payload_recovery_destination(&prepared),
            "prepared payload-recovery destination authority must remain exact until commit"
        );
        self.live_presentation = None;
        self.committed_payload_recovery_destination =
            Some(DockHostCommittedPayloadRecoveryDestination {
                key: prepared.key,
                destination: prepared.destination,
            });
        self.last_visual_affordance_scene = None;
        self.last_presentation_scene = None;
        cx.notify();
    }

    pub(crate) fn prepare_payload_recovery_source_restoration_commit(
        &self,
        key: DockHostRecoveryPresentationKey,
        presented: view_presentation_window::StableBatchPresentationReceipt,
    ) -> Option<DockHostPreparedPayloadRecoverySourceRestorationCommit> {
        if !self.accepts_payload_recovery_presentation_key(key) {
            return None;
        }
        let DockHostRecoveryPresentationState {
            mode:
                DockHostRecoveryPresentationMode::SourceRestoration {
                    projection,
                    leases,
                    phase: DockHostRecoverySourceRestorationPhase::AwaitingVisibleFrame,
                    ..
                },
            ..
        } = self
            .live_presentation
            .as_ref()
            .and_then(DockHostPresentationState::as_recovery)?
        else {
            return None;
        };
        let generation = leases.leases().first()?.generation();
        if projection.generation() != key.rehost_generation()
            || projection.source().window_id() != leases.window_id()
            || projection.source().leases().len() != leases.leases().len()
            || projection
                .source()
                .leases()
                .iter()
                .any(|lease| leases.lease_for(lease.entity_id()).is_none())
            || presented.window_id() != leases.window_id()
            || presented.lease_generation() != generation
            || presented.root_count() != leases.leases().len()
            || leases
                .leases()
                .iter()
                .any(|lease| self.panel_presentation_leases.get(&lease.entity_id()) != Some(lease))
        {
            return None;
        }
        Some(DockHostPreparedPayloadRecoverySourceRestorationCommit {
            key,
            source: leases.clone(),
            presented,
        })
    }

    pub(crate) fn can_commit_prepared_payload_recovery_source_restoration(
        &self,
        prepared: &DockHostPreparedPayloadRecoverySourceRestorationCommit,
    ) -> bool {
        let Some(current) = self
            .prepare_payload_recovery_source_restoration_commit(prepared.key, prepared.presented)
        else {
            return false;
        };
        current.source.window_id() == prepared.source.window_id()
            && current.source.leases() == prepared.source.leases()
    }

    pub(crate) fn commit_prepared_payload_recovery_source_restoration(
        &mut self,
        prepared: DockHostPreparedPayloadRecoverySourceRestorationCommit,
        cx: &mut Context<Self>,
    ) {
        assert!(
            self.can_commit_prepared_payload_recovery_source_restoration(&prepared),
            "prepared payload-recovery source restoration must remain exact until commit"
        );
        self.live_presentation = None;
        self.last_visual_affordance_scene = None;
        self.last_presentation_scene = None;
        cx.notify();
    }

    pub(crate) fn prepare_payload_recovery_presentation_abandonment(
        &self,
        key: DockHostRecoveryPresentationKey,
    ) -> Option<DockHostPreparedPayloadRecoveryPresentationAbandonment> {
        if !self.accepts_payload_recovery_presentation_key(key) {
            return None;
        }
        let state = self
            .live_presentation
            .as_ref()
            .and_then(DockHostPresentationState::as_recovery)?;
        let (stage, leases, generation, prior) = match &state.mode {
            DockHostRecoveryPresentationMode::SourceProjection { projection, phase } => (
                DockHostRecoveryPresentationStage::Source(*phase),
                projection.source().clone(),
                projection.generation(),
                None,
            ),
            DockHostRecoveryPresentationMode::DestinationProjection {
                projection,
                leases,
                phase,
                ..
            } => (
                DockHostRecoveryPresentationStage::Destination(*phase),
                leases.clone(),
                projection.generation(),
                None,
            ),
            DockHostRecoveryPresentationMode::SourceRestoration {
                projection,
                leases,
                phase,
                ..
            } => (
                DockHostRecoveryPresentationStage::SourceRestoration(*phase),
                leases.clone(),
                projection.generation(),
                Some(projection.source().clone()),
            ),
        };
        if generation != key.rehost_generation() {
            return None;
        }
        let publication = DockHostPresentationPublicationSnapshot::capture(
            leases,
            prior,
            &self.panel_presentation_leases,
        );
        Some(DockHostPreparedPayloadRecoveryPresentationAbandonment {
            key,
            stage,
            publication,
        })
    }

    pub(crate) fn can_commit_prepared_payload_recovery_presentation_abandonment(
        &self,
        prepared: &DockHostPreparedPayloadRecoveryPresentationAbandonment,
    ) -> bool {
        let Some(current) = self.prepare_payload_recovery_presentation_abandonment(prepared.key)
        else {
            return false;
        };
        current.stage == prepared.stage
            && current.publication.matches_exactly(&prepared.publication)
    }

    pub(crate) fn commit_prepared_payload_recovery_presentation_abandonment(
        &mut self,
        prepared: DockHostPreparedPayloadRecoveryPresentationAbandonment,
        cx: &mut Context<Self>,
    ) {
        assert!(
            self.can_commit_prepared_payload_recovery_presentation_abandonment(&prepared),
            "prepared payload-recovery presentation abandonment must remain exact until commit"
        );
        let publication = prepared.publication;
        self.live_presentation = None;
        publication.remove_observed_owned(&mut self.panel_presentation_leases);
        self.last_visual_affordance_scene = None;
        self.last_presentation_scene = None;
        cx.notify();
    }

    pub(crate) fn mark_live_source_frozen(
        &mut self,
        key: DockHostLivePresentationKey,
        cx: &mut Context<Self>,
    ) -> bool {
        if !self.accepts_live_presentation_key(key) {
            return false;
        }
        let Some(DockHostLivePresentationState {
            mode: DockHostLivePresentationMode::SourceProjection { phase, .. },
            ..
        }) = self
            .live_presentation
            .as_mut()
            .and_then(DockHostPresentationState::as_live_mut)
        else {
            return false;
        };
        if *phase != DockHostLiveSourcePhase::Releasing {
            return false;
        }
        *phase = DockHostLiveSourcePhase::Frozen;
        cx.notify();
        true
    }

    pub(crate) fn expose_live_destination_projection(
        &mut self,
        key: DockHostLivePresentationKey,
        leases: view_presentation_window::LeaseBatch,
        mount: DockLiveUndockPayloadMountReceipt,
        cx: &mut Context<Self>,
    ) -> bool {
        if !self.accepts_live_presentation_key(key) {
            return false;
        }
        let Some(DockHostLivePresentationState {
            mode:
                DockHostLivePresentationMode::DestinationProjection {
                    leases: current,
                    phase,
                    ..
                },
            ..
        }) = self
            .live_presentation
            .as_mut()
            .and_then(DockHostPresentationState::as_live_mut)
        else {
            return false;
        };
        if !matches!(phase, DockHostLiveDestinationPhase::Staging)
            || current.leases() != leases.leases()
            || current.window_id() != leases.window_id()
        {
            return false;
        }
        *current = leases;
        *phase = DockHostLiveDestinationPhase::Exposed(mount);
        cx.notify();
        true
    }

    pub(crate) fn mark_live_destination_presented(
        &mut self,
        key: DockHostLivePresentationKey,
        presentation: DockLiveUndockPayloadPresentationReceipt,
        cx: &mut Context<Self>,
    ) -> bool {
        if !self.accepts_live_presentation_key(key) {
            return false;
        }
        let Some(DockHostLivePresentationState {
            mode: DockHostLivePresentationMode::DestinationProjection { phase, .. },
            ..
        }) = self
            .live_presentation
            .as_mut()
            .and_then(DockHostPresentationState::as_live_mut)
        else {
            return false;
        };
        let DockHostLiveDestinationPhase::Exposed(mount) = phase else {
            return false;
        };
        if presentation.mount() != *mount {
            return false;
        }
        *phase = DockHostLiveDestinationPhase::Presented(presentation);
        cx.notify();
        true
    }

    pub(crate) fn arm_live_destination_reveal(
        &mut self,
        key: DockHostLivePresentationKey,
        presentation: DockLiveUndockPayloadPresentationReceipt,
        ticket: WindowProvisionalRevealTicket,
        cx: &mut Context<Self>,
    ) -> bool {
        if !self.accepts_live_presentation_key(key) {
            return false;
        }
        let Some(DockHostLivePresentationState {
            mode: DockHostLivePresentationMode::DestinationProjection { phase, .. },
            ..
        }) = self
            .live_presentation
            .as_mut()
            .and_then(DockHostPresentationState::as_live_mut)
        else {
            return false;
        };
        if !matches!(phase, DockHostLiveDestinationPhase::Presented(current) if *current == presentation)
        {
            return false;
        }
        *phase = DockHostLiveDestinationPhase::RevealArmed {
            presentation,
            ticket,
        };
        cx.notify();
        true
    }

    pub(crate) fn can_arm_live_destination_reveal(
        &self,
        key: DockHostLivePresentationKey,
        presentation: DockLiveUndockPayloadPresentationReceipt,
    ) -> bool {
        self.accepts_live_presentation_key(key)
            && self
                .live_presentation
                .as_ref()
                .and_then(DockHostPresentationState::as_live)
                .is_some_and(|state| {
                    matches!(
                        &state.mode,
                        DockHostLivePresentationMode::DestinationProjection {
                            phase: DockHostLiveDestinationPhase::Presented(current),
                            ..
                        } if *current == presentation
                    )
                })
    }

    pub(crate) fn begin_live_destination_reveal_observation(
        &mut self,
        key: DockHostLivePresentationKey,
        presentation: DockLiveUndockPayloadPresentationReceipt,
        candidate_frame: DockLiveUndockPayloadPresentationReceipt,
        cx: &mut Context<Self>,
    ) -> Option<WindowProvisionalRevealTicket> {
        if !self.accepts_live_presentation_key(key) {
            return None;
        }
        let Some(DockHostLivePresentationState {
            mode: DockHostLivePresentationMode::DestinationProjection { phase, .. },
            ..
        }) = self
            .live_presentation
            .as_mut()
            .and_then(DockHostPresentationState::as_live_mut)
        else {
            return None;
        };
        let DockHostLiveDestinationPhase::RevealArmed {
            presentation: current,
            ticket,
        } = phase
        else {
            return None;
        };
        let ticket_snapshot = ticket.snapshot();
        if *current != presentation
            || candidate_frame.mount() != presentation.mount()
            || candidate_frame.frame_generation() <= presentation.frame_generation()
            || candidate_frame.frame_generation()
                < ticket_snapshot.minimum_presentation_generation()
            || candidate_frame.window_id() != ticket_snapshot.window_id()
            || candidate_frame
                .mount()
                .proxy()
                .lease()
                .provisional_session_generation()
                != ticket_snapshot.session_generation()
        {
            return None;
        }
        let ticket = ticket.clone();
        *phase = DockHostLiveDestinationPhase::RevealObserving {
            presentation,
            candidate_frame,
            submitted_frame: None,
            ticket: ticket.clone(),
        };
        cx.notify();
        Some(ticket)
    }

    pub(crate) fn bind_live_destination_reveal_submission(
        &mut self,
        key: DockHostLivePresentationKey,
        presentation: DockLiveUndockPayloadPresentationReceipt,
        submitted_frame: DockLiveUndockPayloadPresentationReceipt,
        cx: &mut Context<Self>,
    ) -> bool {
        if !self.accepts_live_presentation_key(key) {
            return false;
        }
        let Some(DockHostLivePresentationState {
            mode: DockHostLivePresentationMode::DestinationProjection { phase, .. },
            ..
        }) = self
            .live_presentation
            .as_mut()
            .and_then(DockHostPresentationState::as_live_mut)
        else {
            return false;
        };
        let DockHostLiveDestinationPhase::RevealObserving {
            presentation: current,
            candidate_frame,
            submitted_frame: current_submitted_frame,
            ticket,
        } = phase
        else {
            return false;
        };
        let snapshot = ticket.snapshot();
        if *current != presentation
            || candidate_frame.mount() != presentation.mount()
            || submitted_frame.mount() != presentation.mount()
            || submitted_frame.frame_generation() <= presentation.frame_generation()
            || submitted_frame.frame_generation() < candidate_frame.frame_generation()
            || snapshot.window_id() != submitted_frame.window_id()
            || snapshot.session_generation()
                != submitted_frame
                    .mount()
                    .proxy()
                    .lease()
                    .provisional_session_generation()
            || snapshot.presentation_generation() != Some(submitted_frame.frame_generation())
        {
            return false;
        }
        match current_submitted_frame {
            Some(current) => *current == submitted_frame,
            slot @ None => {
                *slot = Some(submitted_frame);
                cx.notify();
                true
            }
        }
    }

    pub(crate) fn settle_live_destination_reveal(
        &mut self,
        key: DockHostLivePresentationKey,
        presentation: DockLiveUndockPayloadPresentationReceipt,
        submitted_frame: Option<DockLiveUndockPayloadPresentationReceipt>,
        cx: &mut Context<Self>,
    ) -> bool {
        if !self.accepts_live_presentation_key(key) {
            return false;
        }
        let Some(DockHostLivePresentationState {
            mode: DockHostLivePresentationMode::DestinationProjection { phase, .. },
            ..
        }) = self
            .live_presentation
            .as_mut()
            .and_then(DockHostPresentationState::as_live_mut)
        else {
            return false;
        };
        if !matches!(
            phase,
            DockHostLiveDestinationPhase::RevealObserving {
                presentation: current,
                candidate_frame: _,
                submitted_frame: current_submitted_frame,
                ..
            } if *current == presentation && *current_submitted_frame == submitted_frame
        ) {
            return false;
        }
        *phase = DockHostLiveDestinationPhase::RevealSettled;
        cx.notify();
        true
    }

    pub(crate) fn commit_live_destination_geometry_from_accepted_frame(
        &mut self,
        binding: DockHostWindowBinding,
        work_context: DockViewportRuntimeWorkContext,
        current_bounds: crate::viewport_registry::DockViewportWindowBoundsFrame,
        host_geometry: crate::DockViewportHostGeometry,
        window_id: WindowId,
        cx: &Context<Self>,
    ) -> bool {
        if !self.is_current_window_binding(binding, window_id)
            || self.live_destination_runtime_work_context(cx) != Some(work_context)
        {
            return false;
        }
        let Some(state) = self
            .live_presentation
            .as_mut()
            .and_then(DockHostPresentationState::as_live_mut)
        else {
            return false;
        };
        if state.key.binding != binding
            || self.role != DockHostRole::ProvisionalViewport(state.key.identity.opening())
        {
            return false;
        }
        let DockHostLivePresentationMode::DestinationProjection {
            accepted_geometry, ..
        } = &mut state.mode
        else {
            return false;
        };
        let next = DockHostLiveDestinationGeometry {
            current_bounds,
            host_geometry,
        };
        if accepted_geometry.as_ref() == Some(&next) {
            return false;
        }
        *accepted_geometry = Some(next);
        true
    }

    pub(crate) fn retire_live_source_visual(
        &mut self,
        key: DockHostLivePresentationKey,
        cx: &mut Context<Self>,
    ) -> bool {
        if !self.accepts_live_presentation_key(key) {
            return false;
        }
        let Some(DockHostLivePresentationState {
            mode: DockHostLivePresentationMode::SourceProjection { phase, .. },
            ..
        }) = self
            .live_presentation
            .as_mut()
            .and_then(DockHostPresentationState::as_live_mut)
        else {
            return false;
        };
        if *phase != DockHostLiveSourcePhase::Frozen {
            return false;
        }
        *phase = DockHostLiveSourcePhase::Retired;
        cx.notify();
        true
    }

    pub(crate) fn begin_live_source_restoration(
        &mut self,
        key: DockHostLivePresentationKey,
        presentation: DockHostPresentationSession,
        leases: view_presentation_window::LeaseBatch,
        cx: &mut Context<Self>,
    ) -> DockHostLiveSourceRestorationInstallOutcome {
        if !self.accepts_live_presentation_key(key)
            || !self
                .live_source_semantic_proxy
                .as_ref()
                .is_some_and(|proxy| proxy.key == key)
            || matches!(
                presentation.kind(),
                crate::host_render_session::DockHostPresentationKind::LivePayloadProjection
                    | crate::host_render_session::DockHostPresentationKind::PayloadRecoveryProjection
            )
            || leases.window_id() != key.binding.window_id()
        {
            return DockHostLiveSourceRestorationInstallOutcome::PresentationAuthorityLost;
        }
        let Some(state) = self
            .live_presentation
            .as_mut()
            .and_then(DockHostPresentationState::as_live_mut)
        else {
            return DockHostLiveSourceRestorationInstallOutcome::PresentationAuthorityLost;
        };
        if matches!(
            &state.mode,
            DockHostLivePresentationMode::SourceRestoration {
                lease,
                projection,
                leases: current,
                ..
            } if lease.identity() == key.identity
                && projection.generation() == key.rehost_generation
                && current.window_id() == leases.window_id()
                && current.leases() == leases.leases()
        ) {
            return DockHostLiveSourceRestorationInstallOutcome::AlreadyInstalled;
        }
        let DockHostLivePresentationMode::SourceProjection {
            lease,
            projection,
            retained,
            carrier,
            phase: _,
        } = &state.mode
        else {
            return DockHostLiveSourceRestorationInstallOutcome::PresentationAuthorityLost;
        };
        if lease.rehost_generation() != projection.generation()
            || projection.generation() != key.rehost_generation()
            || projection.source().window_id() != leases.window_id()
            || projection.source().leases().len() != leases.leases().len()
            || projection
                .source()
                .leases()
                .iter()
                .any(|lease| leases.lease_for(lease.entity_id()).is_none())
        {
            return DockHostLiveSourceRestorationInstallOutcome::PresentationAuthorityLost;
        }
        let retained = Some((*retained, carrier.clone()));
        state.presentation = presentation;
        state.mode = DockHostLivePresentationMode::SourceRestoration {
            lease: *lease,
            projection: projection.clone(),
            leases,
            retained,
            phase: DockHostLiveSourceRestorationPhase::Staging,
        };
        cx.notify();
        DockHostLiveSourceRestorationInstallOutcome::Installed
    }

    pub(crate) fn mark_live_source_restoration_visible_pending(
        &mut self,
        key: DockHostLivePresentationKey,
        leases: &view_presentation_window::LeaseBatch,
        cx: &mut Context<Self>,
    ) -> bool {
        if !self.accepts_live_presentation_key(key) {
            return false;
        }
        let Some(DockHostLivePresentationState {
            mode:
                DockHostLivePresentationMode::SourceRestoration {
                    leases: current,
                    phase,
                    ..
                },
            ..
        }) = self
            .live_presentation
            .as_mut()
            .and_then(DockHostPresentationState::as_live_mut)
        else {
            return false;
        };
        if current.window_id() != leases.window_id() || current.leases() != leases.leases() {
            return false;
        }
        if *phase == DockHostLiveSourceRestorationPhase::AwaitingVisibleFrame {
            return true;
        }
        if *phase != DockHostLiveSourceRestorationPhase::Staging {
            return false;
        }
        *phase = DockHostLiveSourceRestorationPhase::AwaitingVisibleFrame;
        cx.notify();
        true
    }

    pub(crate) fn clear_live_presentation(
        &mut self,
        key: DockHostLivePresentationKey,
        cx: &mut Context<Self>,
    ) -> bool {
        if !self.accepts_live_presentation_key(key) {
            return false;
        }
        self.live_presentation = None;
        self.clear_live_source_semantic_proxy_for_key(key);
        cx.notify();
        true
    }

    pub(crate) fn retire_live_source_semantic_proxy(
        &mut self,
        key: DockHostLivePresentationKey,
        lease: DockLiveUndockPayloadLeaseReceipt,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(prepared) = self.prepare_live_source_semantic_retirement(key, lease) else {
            return false;
        };
        self.commit_prepared_live_source_semantic_retirement(prepared, cx);
        true
    }

    pub(crate) fn prepare_live_source_semantic_retirement(
        &self,
        key: DockHostLivePresentationKey,
        lease: DockLiveUndockPayloadLeaseReceipt,
    ) -> Option<DockHostPreparedLiveSourceSemanticRetirement> {
        self.accepts_live_source_semantic_proxy(key, lease)
            .then_some(DockHostPreparedLiveSourceSemanticRetirement { key, lease })
    }

    pub(crate) fn can_commit_prepared_live_source_semantic_retirement(
        &self,
        prepared: &DockHostPreparedLiveSourceSemanticRetirement,
    ) -> bool {
        self.accepts_live_source_semantic_proxy(prepared.key, prepared.lease)
    }

    pub(crate) fn commit_prepared_live_source_semantic_retirement(
        &mut self,
        prepared: DockHostPreparedLiveSourceSemanticRetirement,
        cx: &mut Context<Self>,
    ) {
        assert!(
            self.can_commit_prepared_live_source_semantic_retirement(&prepared),
            "prepared live source semantic authority must remain exact until commit"
        );
        self.live_source_semantic_proxy = None;
        cx.notify();
    }

    pub(crate) fn prepare_live_presentation_abandonment(
        &self,
        key: DockHostLivePresentationKey,
    ) -> Option<DockHostPreparedLivePresentationAbandonment> {
        if !self.accepts_live_presentation_key(key) || self.live_destination_semantics.is_some() {
            return None;
        }
        let state = self
            .live_presentation
            .as_ref()
            .and_then(DockHostPresentationState::as_live)?;
        let (stage, leases, prepared_generation, prior) = match &state.mode {
            DockHostLivePresentationMode::SourceProjection {
                projection, phase, ..
            } => (
                DockHostLivePresentationStage::Source(*phase),
                projection.source().clone(),
                projection.generation(),
                None,
            ),
            DockHostLivePresentationMode::DestinationProjection {
                projection,
                leases,
                phase,
                ..
            } => {
                let stage = match phase {
                    DockHostLiveDestinationPhase::Staging => {
                        DockHostLivePresentationStage::DestinationStaging
                    }
                    DockHostLiveDestinationPhase::Exposed(_) => {
                        DockHostLivePresentationStage::DestinationExposed
                    }
                    DockHostLiveDestinationPhase::Presented(_) => {
                        DockHostLivePresentationStage::DestinationPresented
                    }
                    DockHostLiveDestinationPhase::RevealArmed { .. } => {
                        DockHostLivePresentationStage::DestinationRevealArmed
                    }
                    DockHostLiveDestinationPhase::RevealObserving { .. } => {
                        DockHostLivePresentationStage::DestinationRevealObserving
                    }
                    DockHostLiveDestinationPhase::RevealSettled => {
                        DockHostLivePresentationStage::DestinationRevealSettled
                    }
                };
                (stage, leases.clone(), projection.generation(), None)
            }
            DockHostLivePresentationMode::SourceRestoration {
                projection,
                leases,
                phase,
                ..
            } => (
                DockHostLivePresentationStage::SourceRestoration(*phase),
                leases.clone(),
                projection.generation(),
                Some(projection.source().clone()),
            ),
        };
        if prepared_generation != key.rehost_generation {
            return None;
        }
        let publication = DockHostPresentationPublicationSnapshot::capture(
            leases,
            prior,
            &self.panel_presentation_leases,
        );
        Some(DockHostPreparedLivePresentationAbandonment {
            key,
            stage,
            publication,
        })
    }

    pub(crate) fn can_commit_prepared_live_presentation_abandonment(
        &self,
        prepared: &DockHostPreparedLivePresentationAbandonment,
    ) -> bool {
        let Some(current) = self.prepare_live_presentation_abandonment(prepared.key) else {
            return false;
        };
        current.stage == prepared.stage
            && current.publication.matches_exactly(&prepared.publication)
    }

    pub(crate) fn committed_live_presentation_abandonment(
        &self,
        key: DockHostLivePresentationKey,
    ) -> Option<DockHostLivePresentationCleanupReceipt> {
        self.committed_live_presentation_abandonment
            .filter(|receipt| receipt.key() == key)
    }

    pub(crate) fn commit_prepared_live_presentation_abandonment(
        &mut self,
        prepared: DockHostPreparedLivePresentationAbandonment,
        cx: &mut Context<Self>,
    ) -> DockHostLivePresentationCleanupReceipt {
        let receipt = self.commit_prepared_live_presentation_abandonment_without_notify(prepared);
        cx.notify();
        receipt
    }

    pub(crate) fn commit_prepared_live_presentation_abandonment_without_notify(
        &mut self,
        prepared: DockHostPreparedLivePresentationAbandonment,
    ) -> DockHostLivePresentationCleanupReceipt {
        if let Some(receipt) = self.committed_live_presentation_abandonment(prepared.key()) {
            return receipt;
        }
        assert!(
            self.can_commit_prepared_live_presentation_abandonment(&prepared),
            "prepared live-presentation abandonment must remain exact until commit"
        );
        let key = prepared.key;
        let publication = prepared.publication;
        self.live_presentation = None;
        self.clear_live_source_semantic_proxy_for_key(key);
        publication.remove_observed_owned(&mut self.panel_presentation_leases);
        self.last_visual_affordance_scene = None;
        self.last_presentation_scene = None;
        let receipt = DockHostLivePresentationCleanupReceipt { key };
        self.committed_live_presentation_abandonment = Some(receipt);
        receipt
    }

    pub(crate) fn prepare_live_source_retirement(
        &self,
        key: DockHostLivePresentationKey,
    ) -> Option<DockHostPreparedLiveSourceRetirement> {
        if !self.accepts_live_presentation_key(key) {
            return None;
        }
        let DockHostLivePresentationState {
            mode:
                DockHostLivePresentationMode::SourceProjection {
                    lease,
                    projection,
                    phase: DockHostLiveSourcePhase::Retired,
                    ..
                },
            ..
        } = self
            .live_presentation
            .as_ref()
            .and_then(DockHostPresentationState::as_live)?
        else {
            return None;
        };
        if lease.identity() != key.identity
            || lease.rehost_generation() != projection.generation()
            || projection.source().window_id() != key.binding.window_id()
        {
            return None;
        }
        Some(DockHostPreparedLiveSourceRetirement {
            key,
            source: projection.source().clone(),
        })
    }

    pub(crate) fn can_commit_prepared_live_source_retirement(
        &self,
        prepared: &DockHostPreparedLiveSourceRetirement,
    ) -> bool {
        let Some(current) = self.prepare_live_source_retirement(prepared.key) else {
            return false;
        };
        current.source.window_id() == prepared.source.window_id()
            && current.source.leases() == prepared.source.leases()
    }

    pub(crate) fn committed_live_source_retirement(
        &self,
        prepared: &DockHostPreparedLiveSourceRetirement,
    ) -> Option<DockHostLiveSourceRetirementReceipt> {
        self.committed_live_source_retirement
            .as_ref()
            .filter(|committed| committed.matches_prepared(prepared))
            .cloned()
    }

    pub(crate) fn commit_or_replay_prepared_live_source_retirement_without_notify(
        &mut self,
        prepared: DockHostPreparedLiveSourceRetirement,
    ) -> Option<DockHostLiveSourceRetirementReceipt> {
        if let Some(receipt) = self.committed_live_source_retirement(&prepared) {
            return Some(receipt);
        }
        if !self.can_commit_prepared_live_source_retirement(&prepared) {
            return None;
        }
        Some(self.commit_prepared_live_source_retirement_without_notify(prepared))
    }

    pub(crate) fn commit_prepared_live_source_retirement_without_notify(
        &mut self,
        prepared: DockHostPreparedLiveSourceRetirement,
    ) -> DockHostLiveSourceRetirementReceipt {
        if let Some(receipt) = self.committed_live_source_retirement(&prepared) {
            return receipt;
        }
        assert!(
            self.can_commit_prepared_live_source_retirement(&prepared),
            "prepared live source retirement must remain exact until commit"
        );
        let receipt = DockHostLiveSourceRetirementReceipt {
            key: prepared.key,
            source: prepared.source.clone(),
        };
        self.committed_live_source_retirement = Some(receipt.clone());
        self.live_presentation = None;
        self.panel_presentation_leases
            .retain(|_, lease| !prepared.source.leases().contains(lease));
        self.last_visual_affordance_scene = None;
        self.last_presentation_scene = None;
        receipt
    }

    pub(crate) fn retire_live_source_retirement(
        &mut self,
        receipt: &DockHostLiveSourceRetirementReceipt,
    ) -> bool {
        match self.committed_live_source_retirement.as_ref() {
            None => true,
            Some(committed) if committed.matches_exactly(receipt) => {
                self.committed_live_source_retirement = None;
                true
            }
            Some(_) => false,
        }
    }

    pub(crate) fn prepare_live_destination_promotion(
        &self,
        key: DockHostLivePresentationKey,
        opening: crate::surface::live_undock::DockLiveUndockOpeningKey,
        token: DockLiveUndockPromotionToken,
        committed_surface_revision: u64,
        target_space: &DockSpaceId,
        registration: DockViewportRegistrationKey,
        window_facts: crate::DockViewportWindowFacts,
    ) -> Option<DockHostPreparedLiveDestinationPromotion> {
        if !self.accepts_live_presentation_key(key)
            || key.identity.opening() != opening
            || self.role != DockHostRole::ProvisionalViewport(opening)
            || &self.space != target_space
            || self.bound_viewport_registration.is_some()
            || self.live_destination_semantics.is_some()
            || committed_surface_revision == 0
            || registration.space() != target_space
            || registration.window_id() != key.binding.window_id()
            || registration.lineage() != DockViewportRuntimeLineage::Surface(opening.lease())
        {
            return None;
        }
        let DockHostLivePresentationState {
            mode:
                DockHostLivePresentationMode::DestinationProjection {
                    projection,
                    leases,
                    accepted_geometry: Some(accepted_geometry),
                    phase: DockHostLiveDestinationPhase::RevealSettled,
                    ..
                },
            ..
        } = self
            .live_presentation
            .as_ref()
            .and_then(DockHostPresentationState::as_live)?
        else {
            return None;
        };
        if projection.destination().window_id() != key.binding.window_id()
            || projection.destination().window_id() != leases.window_id()
            || projection.destination().leases() != leases.leases()
            || leases
                .leases()
                .iter()
                .any(|lease| self.panel_presentation_leases.get(&lease.entity_id()) != Some(lease))
            || accepted_geometry.current_bounds != window_facts.current_bounds
        {
            return None;
        }
        Some(DockHostPreparedLiveDestinationPromotion {
            key,
            opening,
            token,
            committed_surface_revision,
            space: target_space.clone(),
            registration,
            destination: leases.clone(),
            window_facts,
            host_geometry: accepted_geometry.host_geometry.clone(),
        })
    }

    pub(crate) fn can_commit_prepared_live_destination_promotion(
        &self,
        prepared: &DockHostPreparedLiveDestinationPromotion,
    ) -> bool {
        let Some(current) = self.prepare_live_destination_promotion(
            prepared.key,
            prepared.opening,
            prepared.token,
            prepared.committed_surface_revision,
            &prepared.space,
            prepared.registration.clone(),
            prepared.window_facts,
        ) else {
            return false;
        };
        current.destination.window_id() == prepared.destination.window_id()
            && current.destination.leases() == prepared.destination.leases()
            && current.host_geometry == prepared.host_geometry
    }

    pub(crate) fn commit_or_replay_prepared_live_destination_promotion_without_notify(
        &mut self,
        prepared: DockHostPreparedLiveDestinationPromotion,
    ) -> Option<DockHostLiveDestinationPromotionReceipt> {
        if let Some(receipt) = self
            .committed_live_destination_promotion
            .as_ref()
            .filter(|receipt| receipt.matches_prepared(&prepared))
        {
            return Some(receipt.clone());
        }
        if !self.can_commit_prepared_live_destination_promotion(&prepared) {
            return None;
        }
        Some(self.commit_prepared_live_destination_promotion_without_notify(prepared))
    }

    pub(crate) fn commit_prepared_live_destination_promotion_without_notify(
        &mut self,
        prepared: DockHostPreparedLiveDestinationPromotion,
    ) -> DockHostLiveDestinationPromotionReceipt {
        if let Some(receipt) = self
            .committed_live_destination_promotion
            .as_ref()
            .filter(|receipt| receipt.matches_prepared(&prepared))
        {
            return receipt.clone();
        }
        assert!(
            self.can_commit_prepared_live_destination_promotion(&prepared),
            "prepared live destination promotion must remain exact until commit"
        );

        self.role = DockHostRole::ManagedViewport(prepared.opening.lease());
        self.bound_viewport_registration = Some(prepared.registration);
        self.window_binding_generation = self.window_binding_generation.wrapping_add(1).max(1);
        let binding = DockHostWindowBinding {
            window_id: prepared.key.binding.window_id(),
            generation: self.window_binding_generation,
        };
        self.live_presentation = None;
        self.panel_presentation_leases
            .retain(|entity_id, lease| prepared.destination.lease_for(*entity_id) == Some(*lease));
        self.last_visual_affordance_scene = None;
        self.last_presentation_scene = None;
        let semantics = DockHostLiveDestinationSemantics {
            identity: prepared.key.identity,
            token: prepared.token,
            binding,
            registration: self
                .bound_viewport_registration
                .clone()
                .expect("promoted destination host must retain its exact registration"),
            surface_revision: prepared.committed_surface_revision,
            destination: prepared.destination,
        };
        self.live_destination_semantics = Some(semantics.clone());
        let receipt = DockHostLiveDestinationPromotionReceipt { semantics };
        self.committed_live_destination_promotion = Some(receipt.clone());
        receipt
    }

    pub(crate) fn retire_live_destination_promotion(
        &mut self,
        receipt: &DockHostLiveDestinationPromotionReceipt,
    ) -> bool {
        match self.committed_live_destination_promotion.as_ref() {
            None => true,
            Some(committed) if committed.matches_exactly(receipt) => {
                self.committed_live_destination_promotion = None;
                true
            }
            Some(_) => false,
        }
    }

    pub(crate) fn live_destination_semantics(&self) -> Option<DockHostLiveDestinationSemantics> {
        self.live_destination_semantics.clone()
    }

    pub(crate) fn accepts_live_destination_semantics(
        &self,
        semantics: &DockHostLiveDestinationSemantics,
    ) -> bool {
        self.live_destination_semantics
            .as_ref()
            .is_some_and(|current| current.matches_exactly(semantics))
            && self.live_presentation.is_none()
            && self.role == DockHostRole::ManagedViewport(semantics.identity.opening().lease())
            && self.current_window_binding() == Some(semantics.binding)
            && self.bound_viewport_registration.as_ref() == Some(&semantics.registration)
            && self.space == *semantics.registration.space()
    }

    pub(crate) fn complete_live_destination_semantics(
        &mut self,
        semantics: &DockHostLiveDestinationSemantics,
        cx: &mut Context<Self>,
    ) -> bool {
        if !self.accepts_live_destination_semantics(semantics) {
            return false;
        }
        self.live_destination_semantics = None;
        cx.notify();
        true
    }

    pub(crate) fn present_panel_view(
        &mut self,
        panel_view: AnyView,
        window: &Window,
        cx: &mut App,
    ) -> AnyElement {
        // Ordinary hosts keep GPUI's last-rendered-window behavior. Only a managed surface or an
        // active live rehost opts panel roots into exact presentation-window authority.
        if self.live_presentation.is_none()
            && !matches!(
                self.role,
                DockHostRole::PrimaryAnchor(DockHostPrimaryAnchorAuthority::Active(_))
                    | DockHostRole::ManagedViewport(_)
            )
        {
            return panel_view.into_any_element();
        }

        let entity_id = panel_view.entity_id();
        let window_id = window.window_handle().window_id();
        let presentation_lease = self
            .live_presentation
            .as_ref()
            .and_then(|state| match state {
                DockHostPresentationState::Live(state) => match &state.mode {
                    DockHostLivePresentationMode::DestinationProjection { leases, .. } => {
                        Some((true, leases.lease_for(entity_id)))
                    }
                    DockHostLivePresentationMode::SourceRestoration { leases, .. } => leases
                        .lease_for(entity_id)
                        .map(|lease| (false, Some(lease))),
                    DockHostLivePresentationMode::SourceProjection { .. } => None,
                },
                DockHostPresentationState::Recovery(state) => {
                    let lease = match &state.mode {
                        DockHostRecoveryPresentationMode::SourceProjection {
                            projection, ..
                        } => projection.source().lease_for(entity_id),
                        DockHostRecoveryPresentationMode::DestinationProjection {
                            leases,
                            resolved_roots,
                            ..
                        } => resolved_roots
                            .iter()
                            .any(|root| root.entity_id() == entity_id)
                            .then(|| {
                                leases.lease_for(entity_id).or_else(|| {
                                    view_presentation_window::stable_lease_for_window(
                                        cx, entity_id, window_id,
                                    )
                                })
                            })
                            .flatten(),
                        DockHostRecoveryPresentationMode::SourceRestoration {
                            leases,
                            resolved_roots,
                            ..
                        } => resolved_roots
                            .iter()
                            .any(|root| root.entity_id() == entity_id)
                            .then(|| leases.lease_for(entity_id))
                            .flatten(),
                    };
                    Some((true, lease))
                }
            });
        let lease = if let Some((required, presentation_lease)) = presentation_lease {
            let Some(lease) = presentation_lease else {
                debug_assert!(
                    required,
                    "required presentation projection is missing its lease"
                );
                return open_gpui::Empty.into_any_element();
            };
            self.panel_presentation_leases.insert(entity_id, lease);
            Some(lease)
        } else {
            match view_presentation_window::claim(cx, &panel_view, window_id) {
                Ok(lease) => {
                    self.panel_presentation_leases.insert(entity_id, lease);
                    Some(lease)
                }
                Err(view_presentation_window::ClaimError::AlreadyBound { current }) => {
                    (current.window_id() == window_id).then(|| {
                        self.panel_presentation_leases.insert(entity_id, current);
                        current
                    })
                }
                Err(view_presentation_window::ClaimError::RehostInFlight) => {
                    self.panel_presentation_leases.get(&entity_id).copied()
                }
                Err(
                    view_presentation_window::ClaimError::Empty
                    | view_presentation_window::ClaimError::DuplicateEntity(_)
                    | view_presentation_window::ClaimError::MixedBatchGenerations { .. }
                    | view_presentation_window::ClaimError::WindowUnavailable,
                ) => None,
            }
        };

        lease
            .map(|lease| view_presentation_window::present(panel_view, lease).into_any_element())
            .unwrap_or_else(|| open_gpui::Empty.into_any_element())
    }

    pub(crate) fn mounted_panel_presentation_roots(
        &self,
        session: &DockHostPresentationSession,
        source_window: WindowId,
        cx: &mut Context<Self>,
    ) -> Option<Vec<(AnyView, view_presentation_window::Lease)>> {
        session
            .visible_panel_items()
            .iter()
            .map(|item| {
                let view = session.visible_panel_registration(item)?.resolve_view(cx);
                let lease = self
                    .panel_presentation_leases
                    .get(&view.entity_id())
                    .copied()
                    .filter(|lease| lease.window_id() == source_window)?;
                Some((view, lease))
            })
            .collect()
    }

    pub(crate) fn runtime_work_context(
        &self,
        cx: &Context<Self>,
    ) -> Option<DockViewportRuntimeWorkContext> {
        self.runtime_lineage(cx)
            .map(|lineage| DockViewportRuntimeWorkContext::new(lineage, None))
    }

    pub(crate) fn live_destination_runtime_work_context(
        &self,
        cx: &Context<Self>,
    ) -> Option<DockViewportRuntimeWorkContext> {
        let DockHostRole::ProvisionalViewport(opening) = self.role else {
            return None;
        };
        let state = self
            .live_presentation
            .as_ref()
            .and_then(DockHostPresentationState::as_live)?;
        if state.key.identity.opening() != opening
            || !matches!(
                state.mode,
                DockHostLivePresentationMode::DestinationProjection { .. }
            )
        {
            return None;
        }
        let owner = self.surface_owner.as_ref()?;
        let admitted = cx.read_entity(owner, |owner, _| {
            owner.window_session().admits(opening.lease())
                && owner.accepts_live_undock_identity(state.key.identity)
        });
        let context = DockViewportRuntimeWorkContext::new(
            DockViewportRuntimeLineage::Surface(opening.lease()),
            None,
        );
        (admitted && self.viewport_runtime.admits_work_context(context)).then_some(context)
    }

    pub(crate) fn resolve_visual_style(
        &self,
        window: &mut Window,
        cx: &mut open_gpui::App,
    ) -> Rc<DockVisualStyle> {
        self.visual_style_resolver
            .as_ref()
            .map(|resolver| resolver.resolve(window, cx))
            .unwrap_or_else(|| self.fallback_visual_style.clone())
    }

    #[cfg(test)]
    pub(crate) fn last_resolved_visual_style(&self) -> Option<&DockVisualStyle> {
        self.last_resolved_visual_style.as_deref()
    }

    #[cfg(test)]
    pub(crate) fn record_resolved_visual_style_for_test(&mut self, style: Rc<DockVisualStyle>) {
        self.last_resolved_visual_style = Some(style);
    }

    pub(crate) fn space(&self) -> &DockSpaceId {
        &self.space
    }

    pub(crate) fn host_focus_handle(&self) -> FocusHandle {
        self.focus_handle.clone()
    }

    #[cfg(test)]
    pub(crate) fn controller(&self) -> &Entity<DockController> {
        &self.controller
    }

    pub(crate) fn controller_entity(&self) -> Entity<DockController> {
        self.controller.clone()
    }

    pub(crate) fn surface_owner_entity(&self) -> Option<Entity<DockSurfaceOwner>> {
        self.surface_owner.clone()
    }

    pub(crate) fn visible_payload_recovery_entries(
        &self,
        cx: &Context<Self>,
    ) -> Vec<DockPayloadRecoveryEntry> {
        let DockHostRole::PrimaryAnchor(DockHostPrimaryAnchorAuthority::Active(anchor_lease)) =
            self.role
        else {
            return Vec::new();
        };
        let Some(owner) = self.surface_owner.as_ref() else {
            return Vec::new();
        };
        cx.read_entity(owner, |owner, _| {
            if owner.window_session().active_lease() != Some(anchor_lease) {
                return Vec::new();
            }
            owner.visible_payload_recovery_entries()
        })
    }

    pub(crate) fn restore_payload_recovery_from_render(
        &mut self,
        action: DockPayloadRecoveryRestoreAction,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Result<(), DockPayloadRecoveryRestoreError> {
        let Some(owner) = self.surface_owner.clone() else {
            return Err(DockPayloadRecoveryRestoreError::AnchorUnavailable);
        };
        let Some(primary_binding) = self.current_window_binding() else {
            return Err(DockPayloadRecoveryRestoreError::AnchorUnavailable);
        };
        let Some(primary_window) = window.window_handle().downcast::<DockHost>() else {
            return Err(DockPayloadRecoveryRestoreError::AnchorUnavailable);
        };
        if !self.accepts_payload_recovery_destination_endpoint(
            owner.entity_id(),
            action,
            primary_binding,
        ) {
            return Err(DockPayloadRecoveryRestoreError::AnchorUnavailable);
        }
        let primary_host = cx.entity().downgrade();
        cx.defer(move |cx| {
            let _ = crate::surface::payload_recovery_executor::start_payload_recovery_restore(
                owner,
                primary_host,
                primary_window,
                primary_binding,
                action,
                cx,
            );
        });
        Ok(())
    }

    pub(crate) fn install_payload_recovery_restore_focus(
        &mut self,
        receipt: &DockPayloadRecoveryRestoreReceipt,
        cx: &mut Context<Self>,
    ) -> bool {
        if !matches!(
            self.role,
            DockHostRole::PrimaryAnchor(DockHostPrimaryAnchorAuthority::Active(anchor))
                if anchor == receipt.recovery().live_identity().opening().lease()
        ) {
            return false;
        }
        if let Some(generation) = self
            .interaction
            .pending_focus_command_ticket()
            .map(|ticket| ticket.generation())
        {
            self.settle_pending_focus_command_generation(
                generation,
                DockSurfaceActivationOutcome::Superseded,
                cx,
            );
        }
        self.pending_recovery_entry_focus_completion = None;
        self.pending_recovery_restore_focus =
            receipt
                .focus_item()
                .cloned()
                .map(|item| DockPendingRecoveryRestoreFocus {
                    generation: receipt.recovery().generation().get(),
                    item,
                    descendant: receipt.descendant_focus().cloned(),
                    completion_target: None,
                    completion: None,
                });
        cx.notify();
        true
    }

    pub(crate) fn with_workspace<R>(
        &self,
        cx: &Context<Self>,
        read: impl FnOnce(&DockWorkspace) -> R,
    ) -> R {
        cx.read_entity(&self.controller, |controller, _| {
            read(controller.workspace())
        })
    }

    pub(crate) fn mutate_controller_from_host(
        &mut self,
        cx: &mut Context<Self>,
        categories: &[DockSurfaceChangeCategory],
        mutate: impl FnOnce(&mut DockController) -> Result<DockActionOutcome, DockActionApplyError>,
    ) -> Result<DockActionOutcome, DockActionApplyError> {
        self.mutate_controller_from_host_with(cx, categories, mutate, |outcome| outcome.changed())
    }

    pub(crate) fn mutate_controller_from_host_with<R>(
        &mut self,
        cx: &mut Context<Self>,
        categories: &[DockSurfaceChangeCategory],
        mutate: impl FnOnce(&mut DockController) -> Result<R, DockActionApplyError>,
        changed: impl FnOnce(&R) -> bool,
    ) -> Result<R, DockActionApplyError> {
        let controller = self.controller.clone();
        let Some(owner) = self.surface_owner.clone() else {
            return cx.update_entity(&controller, |controller, cx| {
                let outcome = mutate(controller);
                let did_change = outcome.as_ref().map(changed).unwrap_or(false);
                if did_change {
                    cx.notify();
                }
                outcome
            });
        };
        with_root_transaction(&owner, cx, |owner, transaction, cx| {
            let (outcome, did_change) = cx.update_entity(&controller, |controller, cx| {
                let outcome = mutate(controller);
                let did_change = outcome.as_ref().map(changed).unwrap_or(false);
                if did_change {
                    cx.notify();
                }
                (outcome, did_change)
            });
            if did_change {
                owner.record_changes(transaction, categories.iter().copied());
            }
            outcome
        })
    }

    pub(crate) fn interaction(&self) -> &DockInteractionRuntime {
        &self.interaction
    }

    pub(crate) fn interaction_mut(&mut self) -> &mut DockInteractionRuntime {
        &mut self.interaction
    }

    pub(crate) fn zoom_state(&self) -> &DockZoomState {
        &self.zoom
    }

    pub(crate) fn zoom_state_mut(&mut self) -> &mut DockZoomState {
        &mut self.zoom
    }

    pub(crate) fn transition_executor_mut(&mut self) -> &mut DockTransitionExecutor {
        &mut self.transitions
    }

    pub(crate) fn visual_affordance_transition_executor_mut(
        &mut self,
    ) -> &mut DockTransitionExecutor {
        &mut self.visual_affordance_transitions
    }

    pub(crate) fn visual_affordance_transition_executor_for_debug(
        &self,
    ) -> &DockTransitionExecutor {
        &self.visual_affordance_transitions
    }

    pub(crate) fn last_visual_affordance_scene(&self) -> Option<&DockVisualAffordanceScene> {
        self.last_visual_affordance_scene.as_ref()
    }

    pub(crate) fn set_last_visual_affordance_scene(&mut self, scene: DockVisualAffordanceScene) {
        self.last_visual_affordance_scene = Some(scene);
    }

    pub(crate) fn clear_last_visual_affordance_scene(&mut self) -> bool {
        self.last_visual_affordance_scene.take().is_some()
    }

    pub(crate) fn last_presentation_scene(&self) -> Option<&DockPresentationScene> {
        self.last_presentation_scene.as_ref()
    }

    pub(crate) fn set_last_presentation_scene(&mut self, scene: DockPresentationScene) {
        self.last_presentation_scene = Some(scene);
    }

    pub(crate) fn clear_last_presentation_scene(&mut self) -> bool {
        self.last_presentation_scene.take().is_some()
    }

    pub(crate) fn viewport_runtime(&self) -> &DockViewportRuntimeHandle {
        &self.viewport_runtime
    }

    pub(crate) fn viewport_scene_publication(&self) -> PrepaintPublicationId {
        self.viewport_scene_publication
    }

    pub(crate) fn motion_preference(&self, cx: &Context<Self>) -> MotionPreference {
        self.with_workspace(cx, |workspace| workspace.options().motion_preference)
    }

    pub(crate) fn current_window_binding(&self) -> Option<DockHostWindowBinding> {
        self.bound_window_id.map(|window_id| DockHostWindowBinding {
            window_id,
            generation: self.window_binding_generation,
        })
    }

    #[cfg(test)]
    pub(crate) fn invalidate_window_binding_for_test(&mut self) {
        self.window_binding_generation = self.window_binding_generation.wrapping_add(1).max(1);
    }

    pub(crate) fn current_viewport_registration(&self) -> Option<DockViewportRegistrationKey> {
        self.bound_viewport_registration.clone()
    }

    pub(crate) fn accepts_payload_recovery_source_endpoint(
        &self,
        owner_id: EntityId,
        space: &DockSpaceId,
        binding: DockHostWindowBinding,
        registration: &DockViewportRegistrationKey,
    ) -> bool {
        self.surface_owner
            .as_ref()
            .is_some_and(|owner| owner.entity_id() == owner_id)
            && &self.space == space
            && self.current_window_binding() == Some(binding)
            && self.bound_viewport_registration.as_ref() == Some(registration)
            && registration.space() == space
            && registration.window_id() == binding.window_id()
    }

    pub(crate) fn accepts_payload_recovery_destination_endpoint(
        &self,
        owner_id: EntityId,
        action: DockPayloadRecoveryRestoreAction,
        binding: DockHostWindowBinding,
    ) -> bool {
        self.surface_owner
            .as_ref()
            .is_some_and(|owner| owner.entity_id() == owner_id)
            && self.current_window_binding() == Some(binding)
            && matches!(
                self.role,
                DockHostRole::PrimaryAnchor(DockHostPrimaryAnchorAuthority::Active(anchor))
                    if anchor == action.anchor_lease()
            )
    }

    fn is_current_window_binding(
        &self,
        binding: DockHostWindowBinding,
        window_id: WindowId,
    ) -> bool {
        self.bound_window_id == Some(binding.window_id)
            && self.window_binding_generation == binding.generation
            && window_id == binding.window_id
    }

    pub(crate) fn accepts_window_callback(
        &self,
        binding: Option<DockHostWindowBinding>,
        window_id: WindowId,
    ) -> bool {
        binding.is_none_or(|binding| self.is_current_window_binding(binding, window_id))
    }

    pub(crate) fn accepts_bound_window(&self, binding: Option<DockHostWindowBinding>) -> bool {
        binding.is_none_or(|binding| {
            self.bound_window_id == Some(binding.window_id)
                && self.window_binding_generation == binding.generation
        })
    }

    pub(crate) fn accepts_viewport_scene_candidate(
        &self,
        binding: DockHostWindowBinding,
        registration: Option<&DockViewportRegistrationKey>,
        work_context: DockViewportRuntimeWorkContext,
        window_id: WindowId,
        cx: &Context<Self>,
    ) -> bool {
        self.is_current_window_binding(binding, window_id)
            && self.bound_viewport_registration.as_ref() == registration
            && self.runtime_work_context(cx) == Some(work_context)
    }

    pub(crate) fn adopt_viewport_scene_registration(
        &mut self,
        binding: DockHostWindowBinding,
        expected: Option<&DockViewportRegistrationKey>,
        registration: DockViewportRegistrationKey,
        work_context: DockViewportRuntimeWorkContext,
        window_id: WindowId,
        cx: &Context<Self>,
    ) -> bool {
        if !self.accepts_viewport_scene_candidate(binding, expected, work_context, window_id, cx) {
            return false;
        }
        self.bound_viewport_registration = Some(registration);
        true
    }

    fn is_current_bound_window_id(&self, window_id: WindowId) -> bool {
        self.bound_window_id.is_none_or(|bound| bound == window_id)
    }

    fn release_surface_activation_registration(
        &mut self,
        registration: DockSurfaceActivationHostRegistration,
        cx: &mut App,
    ) {
        if let Some(owner) = self.surface_owner.clone() {
            let settlements = cx.update_entity(&owner, |owner, owner_cx| {
                let settlements = owner.activation_mut().release_host(&registration);
                owner_cx.notify();
                settlements
            });
            Self::defer_activation_settlements(settlements, cx);
        }
    }

    fn release_window_bound_state(
        &mut self,
        viewport_registration: Option<DockViewportRegistrationKey>,
        window: &Window,
        cx: &mut App,
    ) {
        let live_source_release = self
            .live_presentation
            .as_ref()
            .and_then(DockHostPresentationState::as_live)
            .filter(|state| {
                matches!(
                    state.mode,
                    DockHostLivePresentationMode::SourceRestoration { .. }
                )
            })
            .and_then(|state| {
                self.surface_owner
                    .as_ref()
                    .map(|owner| (owner.downgrade(), state.key))
            });
        let recovery_release = self
            .live_presentation
            .as_ref()
            .and_then(DockHostPresentationState::as_recovery)
            .and_then(|state| {
                self.surface_owner
                    .as_ref()
                    .map(|owner| (owner.downgrade(), state.key))
            });
        let live_destination_reveal_release = self
            .live_presentation
            .as_ref()
            .and_then(DockHostPresentationState::as_live)
            .and_then(|state| {
                let DockHostLivePresentationMode::DestinationProjection {
                    phase:
                        DockHostLiveDestinationPhase::RevealObserving {
                            candidate_frame,
                            submitted_frame,
                            ..
                        },
                    ..
                } = &state.mode
                else {
                    return None;
                };
                self.surface_owner.as_ref().map(|owner| {
                    (
                        owner.downgrade(),
                        state.key,
                        submitted_frame.unwrap_or(*candidate_frame),
                    )
                })
            });
        let pending_stable_source = self
            .live_presentation
            .as_ref()
            .and_then(DockHostPresentationState::source_restoration_batch);
        let registration = self.surface_activation_registration.take();
        let pending_command = self.interaction.take_pending_focus_command();
        self.pending_focus_completion = None;
        self.pending_recovery_entry_focus_completion = None;
        self.pending_recovery_restore_focus = None;
        self.panel_focus_trackers.clear();
        self.interaction.reset_window_bound_state();
        self.raw_drag_pointer_capture = None;
        self.last_visual_affordance_scene = None;
        self.last_presentation_scene = None;
        self.live_presentation = None;
        self.live_destination_semantics = None;
        self.live_source_semantic_proxy = None;
        if let Some(proxy) = self.native_drag_transport_proxy.take() {
            proxy.transport.retire();
        }
        let panel_presentation_leases = self
            .panel_presentation_leases
            .drain()
            .map(|(_, lease)| lease)
            .collect::<Vec<_>>();

        let _ = view_presentation_window::release_stable_leases_after_endpoint_loss(
            cx,
            &panel_presentation_leases,
        );

        if let Some(source) = pending_stable_source {
            let _ = view_presentation_window::release_stable_batch_after_endpoint_loss(cx, &source);
        }

        if let Some(registration) = registration {
            self.release_surface_activation_registration(registration, cx);
        }
        if let Some(viewport_registration) = viewport_registration {
            self.viewport_runtime.release_host_binding_from_window(
                &viewport_registration,
                window,
                cx,
            );
        }

        if let Some(command) = pending_command {
            if let Some(binding) = command.surface_activation_binding() {
                binding.settle(DockSurfaceActivationOutcome::Unavailable, cx);
            }
        }
        if let Some((owner, key)) = recovery_release {
            crate::surface::payload_recovery_executor::payload_recovery_host_presentation_released(
                owner, key, cx,
            );
        }
        if let Some((owner, key)) = live_source_release {
            crate::surface::live_undock_runtime::live_undock_host_presentation_released(
                owner, key, cx,
            );
        }
        if let Some((owner, key, receipt)) = live_destination_reveal_release {
            crate::surface::live_undock_runtime::live_undock_destination_reveal_released(
                owner, key, receipt, cx,
            );
        }
    }

    pub(crate) fn ensure_window_binding(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let window_id = window.window_handle().window_id();
        let viewport_registration = self
            .viewport_runtime
            .registration_key_for_space_window(self.space(), window_id);
        if self.bound_window_id == Some(window_id)
            && self.bound_viewport_registration == viewport_registration
        {
            return;
        }

        let previous_window_id = self.bound_window_id;
        let previous_viewport_registration = self.bound_viewport_registration.take();
        let previous_activation_subscription = self.viewport_activation_subscription.take();
        let previous_bounds_subscription = self.viewport_bounds_subscription.take();
        let previous_release_subscription = self.viewport_release_subscription.take();
        let previous_focus_completion = self.pending_focus_completion.take();
        let previous_recovery_entry_focus_completion =
            self.pending_recovery_entry_focus_completion.take();
        let previous_recovery_restore_focus = self.pending_recovery_restore_focus.take();
        let previous_panel_focus_trackers = std::mem::take(&mut self.panel_focus_trackers);

        self.window_binding_generation = self.window_binding_generation.wrapping_add(1).max(1);
        self.bound_window_id = Some(window_id);
        self.bound_viewport_registration = viewport_registration;

        if previous_window_id.is_none() {
            return;
        }

        // Change the binding before dropping observers so a callback queued by the old window
        // cannot mutate state belonging to the new window.
        drop(previous_activation_subscription);
        drop(previous_bounds_subscription);
        drop(previous_release_subscription);
        drop(previous_focus_completion);
        drop(previous_recovery_entry_focus_completion);
        drop(previous_recovery_restore_focus);
        drop(previous_panel_focus_trackers);
        self.release_window_bound_state(previous_viewport_registration, window, cx);
    }

    pub(crate) fn ensure_pointer_session(&mut self, window: &mut Window) -> PointerCaptureHandle {
        let window_id = window.window_handle().window_id();
        if let Some(handle) = self
            .raw_drag_pointer_capture
            .filter(|handle| handle.window_id() == window_id)
        {
            return handle;
        }

        let handle = window.new_pointer_capture_handle();
        self.raw_drag_pointer_capture = Some(handle);
        handle
    }

    pub(crate) fn ensure_viewport_activation_subscription(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.viewport_activation_subscription.is_some() {
            return;
        }

        let binding = self.current_window_binding();
        self.viewport_activation_subscription = Some(cx.observe_window_activation(
            window,
            move |host, window, cx| {
                let window_id = window.window_handle().window_id();
                if !host.accepts_window_callback(binding, window_id) {
                    return;
                }
                if window.is_window_active() {
                    host.apply_confirmed_backend_window_focus(
                        window_id,
                        DockViewportPlatformFocusRestoreGate::from_app(cx),
                        cx,
                    );
                }
            },
        ));
    }

    pub(crate) fn ensure_surface_activation_host_registration(
        &mut self,
        work_context: DockViewportRuntimeWorkContext,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.ensure_window_binding(window, cx);
        let DockViewportRuntimeLineage::Surface(lease) = work_context.lineage() else {
            return;
        };
        let Some(owner) = self.surface_owner.clone() else {
            return;
        };
        let window_handle = window.window_handle();
        let host_id = cx.entity().entity_id();
        let registration_status = self
            .surface_activation_registration
            .as_ref()
            .filter(|registration| {
                registration.lease() == lease
                    && registration.host_id() == host_id
                    && registration.space() == self.space()
                    && registration.window() == window_handle
            })
            .map(DockSurfaceActivationHostRegistration::status);
        match registration_status {
            Some(DockSurfaceActivationHostRegistrationStatus::Committed) => return,
            Some(DockSurfaceActivationHostRegistrationStatus::DuplicateHostConflict) => {
                let host = cx.entity().downgrade();
                let space = self.space().clone();
                let result = cx.update_entity(&owner, |owner, _| {
                    if !owner.window_session().admits(lease) {
                        return None;
                    }
                    owner
                        .activation_mut()
                        .register_host(lease, space, host, window_handle)
                });
                let Some(result) = result else {
                    return;
                };
                let (registration, settlements) = result.into_parts();
                self.surface_activation_registration = Some(registration);
                Self::defer_activation_settlements(settlements, cx);
                return;
            }
            None => {}
        }

        if let Some(previous) = self.surface_activation_registration.take() {
            self.release_surface_activation_registration(previous, cx);
        }

        let host = cx.entity().downgrade();
        let space = self.space().clone();
        let result = cx.update_entity(&owner, |owner, _| {
            if !owner.window_session().admits(lease) {
                return None;
            }
            owner
                .activation_mut()
                .register_host(lease, space, host, window_handle)
        });
        let Some(result) = result else {
            return;
        };
        let (registration, settlements) = result.into_parts();
        self.surface_activation_registration = Some(registration);
        Self::defer_activation_settlements(settlements, cx);
    }

    fn apply_confirmed_backend_window_focus(
        &mut self,
        window_id: WindowId,
        platform_focus_restore_gate: DockViewportPlatformFocusRestoreGate,
        cx: &mut Context<Self>,
    ) -> bool {
        if !self.is_current_bound_window_id(window_id) {
            return false;
        }
        let outcome = self
            .viewport_runtime
            .confirmed_backend_window_focus_outcome(
                self.space(),
                window_id,
                platform_focus_restore_gate,
                cx,
            );
        let changed = outcome.changed();
        let focus_command_queued = outcome
            .into_focus_command()
            .is_some_and(|command| self.request_viewport_focus_command_in_context(command, cx));
        let applied = changed || focus_command_queued;
        if applied {
            cx.notify();
        }
        applied
    }

    pub(crate) fn ensure_viewport_bounds_subscription(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.viewport_bounds_subscription.is_some() {
            return;
        }

        let runtime = self.viewport_runtime().clone();
        let binding = self.current_window_binding();

        self.viewport_bounds_subscription =
            Some(cx.observe_window_bounds(window, move |host, window, cx| {
                let window_id = window.window_handle().window_id();
                if !host.accepts_window_callback(binding, window_id) {
                    return;
                }
                if runtime.apply_platform_window_facts_from_window(
                    crate::DockViewportWindowFacts::from_window(window, cx),
                    window,
                    cx,
                ) {
                    window.refresh();
                }
            }));
    }

    pub(crate) fn ensure_viewport_release_subscription(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.viewport_release_subscription.is_some() {
            return;
        }

        let binding = self.current_window_binding();
        self.viewport_release_subscription =
            Some(cx.on_release_in(window, move |host, window, cx| {
                let window_id = window.window_handle().window_id();
                if !host.accepts_window_callback(binding, window_id) {
                    return;
                }
                host.bound_window_id = None;
                let viewport_registration = host.bound_viewport_registration.take();
                host.window_binding_generation =
                    host.window_binding_generation.wrapping_add(1).max(1);
                host.release_window_bound_state(viewport_registration, window, cx);
            }));
    }

    fn defer_activation_settlements(settlements: DockSurfaceActivationSettlements, cx: &mut App) {
        if settlements.is_empty() {
            return;
        }
        cx.defer(move |cx| settlements.deliver(cx));
    }

    #[cfg(test)]
    pub(crate) fn request_viewport_focus_command(
        &mut self,
        command: DockViewportFocusCommand,
    ) -> bool {
        let changed = self.interaction.request_viewport_focus_command(command);
        if changed {
            self.pending_focus_completion = None;
        }
        changed
    }

    pub(crate) fn request_viewport_focus_command_in_context(
        &mut self,
        command: DockViewportFocusCommand,
        cx: &mut Context<Self>,
    ) -> bool {
        if command
            .surface_activation_binding()
            .is_some_and(|binding| !binding.is_current(cx))
        {
            if let Some(binding) = command.surface_activation_binding() {
                binding.settle(DockSurfaceActivationOutcome::Unavailable, cx);
            }
            return false;
        }

        let previous = self.interaction.pending_focus_command_ticket();
        let requested_binding = command.surface_activation_binding().cloned();
        let changed = self.interaction.request_viewport_focus_command(command);
        if changed {
            self.pending_focus_completion = None;
            if let Some(binding) = previous
                .as_ref()
                .and_then(|ticket| ticket.command().surface_activation_binding())
            {
                binding.settle(DockSurfaceActivationOutcome::Superseded, cx);
            }
        } else if let Some(binding) = requested_binding {
            let same_request_is_pending = self
                .interaction
                .pending_focus_command_ticket()
                .as_ref()
                .and_then(|ticket| ticket.command().surface_activation_binding())
                == Some(&binding);
            if !same_request_is_pending {
                binding.settle(DockSurfaceActivationOutcome::Rejected, cx);
            }
        }
        changed
    }

    pub(crate) fn prepare_pending_focus_selection_from_render(
        &mut self,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if !self.is_current_bound_window_id(window.window_handle().window_id()) {
            return false;
        }
        let Some(ticket) = self.interaction.pending_focus_command_ticket() else {
            return false;
        };
        let ticket_generation = ticket.generation();
        if ticket
            .command()
            .surface_activation_binding()
            .is_some_and(|binding| !binding.is_current(cx))
        {
            self.settle_pending_focus_command_generation(
                ticket_generation,
                DockSurfaceActivationOutcome::Unavailable,
                cx,
            );
            return false;
        }
        if !window.subtree_presentation().is_interactive() {
            self.settle_pending_focus_command_generation(
                ticket_generation,
                DockSurfaceActivationOutcome::Rejected,
                cx,
            );
            return false;
        }
        let command = ticket.command();
        if command.source() != crate::DockViewportFocusCommandSource::ViewportActivation {
            return false;
        }
        let DockViewportFocusRequest::Panel(item) = command.request() else {
            return false;
        };
        let space = self.space().clone();

        match self.mutate_controller_from_host(
            cx,
            &[DockSurfaceChangeCategory::Selection],
            |controller| {
                controller
                    .workspace_mut()
                    .select_item_in_space(space, item.clone())
            },
        ) {
            Ok(outcome) => outcome.changed(),
            Err(_) => {
                self.settle_pending_focus_command_generation(
                    ticket_generation,
                    DockSurfaceActivationOutcome::Unavailable,
                    cx,
                );
                false
            }
        }
    }

    pub(crate) fn apply_pending_focus_from_render(
        &mut self,
        session: &DockHostRenderSession,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.is_current_bound_window_id(window.window_handle().window_id()) {
            return;
        }
        let Some(ticket) = self.interaction.pending_focus_command_ticket() else {
            return;
        };
        let ticket_generation = ticket.generation();
        if ticket
            .command()
            .surface_activation_binding()
            .is_some_and(|binding| !binding.is_current(cx))
        {
            self.settle_pending_focus_command_generation(
                ticket_generation,
                DockSurfaceActivationOutcome::Unavailable,
                cx,
            );
            return;
        }
        if !window.subtree_presentation().is_interactive() {
            self.settle_pending_focus_command_generation(
                ticket_generation,
                DockSurfaceActivationOutcome::Rejected,
                cx,
            );
            return;
        }
        let command = ticket.command();
        match command.request().clone() {
            DockViewportFocusRequest::Panel(item) => {
                match session
                    .visible_panel_registration(&item)
                    .and_then(|panel| panel.focus_handle(cx))
                {
                    Some(focus_handle) => {
                        let focus_target = window
                            .committed_focus(cx)
                            .filter(|focused| focus_handle.contains(focused, window))
                            .unwrap_or(focus_handle);
                        self.ensure_pending_panel_focus_completion(
                            &ticket,
                            &item,
                            &focus_target,
                            window,
                            cx,
                        );
                    }
                    None => {
                        self.record_no_panel_focus_for_gone_platform_panel(command, &item, cx);
                        self.settle_pending_focus_command_generation(
                            ticket_generation,
                            DockSurfaceActivationOutcome::Unavailable,
                            cx,
                        );
                    }
                }
            }
            DockViewportFocusRequest::NoPanelFocus => {
                match self.no_panel_focus_settlement(window, cx) {
                    DockNoPanelFocusSettlement::Focus(focus_handle) => {
                        self.ensure_pending_no_panel_focus_completion(
                            &ticket,
                            Some(&focus_handle),
                            window,
                            cx,
                        );
                    }
                    DockNoPanelFocusSettlement::Blur => {
                        self.ensure_pending_no_panel_focus_completion(&ticket, None, window, cx);
                    }
                }
            }
        }
    }

    pub(crate) fn apply_payload_recovery_entry_focus_from_render(
        &mut self,
        entries: &[DockPayloadRecoveryEntry],
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !window.subtree_presentation().is_interactive() {
            self.pending_recovery_entry_focus_completion = None;
            return;
        }
        let Some(entry) = entries.iter().find(|entry| entry.focus_pending()) else {
            self.pending_recovery_entry_focus_completion = None;
            return;
        };
        if self
            .pending_recovery_entry_focus_completion
            .as_ref()
            .is_some_and(|completion| {
                completion.action == entry.action() && completion.target == *entry.focus_handle()
            })
        {
            return;
        }

        self.pending_recovery_entry_focus_completion = None;
        let action = entry.action();
        let target = entry.focus_handle().clone();
        let completion_binding = self.current_window_binding();
        let completion_window_id = window.window_handle().window_id();
        let subscription =
            cx.focus_with_completion(&target, window, move |_, host, callback_window, cx| {
                if !host.accepts_window_callback(
                    completion_binding,
                    callback_window.window_handle().window_id(),
                ) || !host.is_current_bound_window_id(completion_window_id)
                {
                    return;
                }
                if !host
                    .pending_recovery_entry_focus_completion
                    .as_ref()
                    .is_some_and(|completion| completion.action == action)
                {
                    return;
                }
                host.pending_recovery_entry_focus_completion = None;
                if let Some(owner) = host.surface_owner.clone() {
                    cx.update_entity(&owner, |owner, owner_cx| {
                        owner.settle_payload_recovery_entry_focus(action, owner_cx);
                    });
                }
                cx.notify();
            });
        self.pending_recovery_entry_focus_completion =
            Some(DockPendingRecoveryEntryFocusCompletion {
                action,
                target,
                _subscription: subscription,
            });
    }

    pub(crate) fn apply_payload_recovery_restore_focus_from_render(
        &mut self,
        session: &DockHostRenderSession,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !window.subtree_presentation().is_interactive() {
            self.pending_recovery_restore_focus = None;
            return;
        }
        let Some(pending) = self.pending_recovery_restore_focus.as_ref() else {
            return;
        };
        let generation = pending.generation;
        let item = pending.item.clone();
        let (target, descendant_attempt) = if let Some(descendant) = pending.descendant.as_ref() {
            (descendant.focus_handle().clone(), true)
        } else {
            let Some(target) = session
                .visible_panel_registration(&item)
                .and_then(|panel| panel.focus_handle(cx))
            else {
                self.pending_recovery_restore_focus = None;
                return;
            };
            (target, false)
        };
        if pending.completion_target.as_ref() == Some(&target) && pending.completion.is_some() {
            return;
        }

        if let Some(pending) = self.pending_recovery_restore_focus.as_mut() {
            pending.completion = None;
            pending.completion_target = None;
        }
        let completion_target = target.clone();
        let completion_item = item.clone();
        let completion_binding = self.current_window_binding();
        let completion_window_id = window.window_handle().window_id();
        let subscription = cx.focus_with_completion(
            &target,
            window,
            move |outcome, host, callback_window, cx| {
                if !host.accepts_window_callback(
                    completion_binding,
                    callback_window.window_handle().window_id(),
                ) || !host.is_current_bound_window_id(completion_window_id)
                {
                    return;
                }
                if !host
                    .pending_recovery_restore_focus
                    .as_ref()
                    .is_some_and(|pending| pending.generation == generation)
                {
                    return;
                }
                match outcome {
                    FocusClaimOutcome::Committed => {
                        host.pending_recovery_restore_focus = None;
                        host.remember_panel_focus(completion_item.clone(), cx);
                    }
                    FocusClaimOutcome::Rejected if descendant_attempt => {
                        if let Some(pending) = host.pending_recovery_restore_focus.as_mut() {
                            pending.descendant = None;
                            pending.completion_target = None;
                            pending.completion = None;
                        }
                    }
                    FocusClaimOutcome::Rejected | FocusClaimOutcome::Superseded => {
                        host.pending_recovery_restore_focus = None;
                    }
                }
                cx.notify();
            },
        );
        let Some(pending) = self.pending_recovery_restore_focus.as_mut() else {
            drop(subscription);
            return;
        };
        if pending.generation != generation {
            drop(subscription);
            return;
        }
        pending.completion_target = Some(completion_target);
        pending.completion = Some(subscription);
    }

    pub(crate) fn remember_panel_focus(&mut self, item: DockItemId, cx: &mut Context<Self>) {
        let space = self.space().clone();
        self.viewport_runtime()
            .record_panel_focus(space.clone(), item.clone());
        let _ = self.mutate_controller_from_host(
            cx,
            &[DockSurfaceChangeCategory::Selection],
            |controller| {
                controller
                    .workspace_mut()
                    .select_item_in_space(space, item.clone())
            },
        );
    }

    fn record_no_panel_focus_for_gone_platform_panel(
        &self,
        command: &DockViewportFocusCommand,
        item: &DockItemId,
        cx: &mut Context<Self>,
    ) {
        if command.source() != crate::DockViewportFocusCommandSource::PlatformActivation {
            return;
        }
        if !self
            .viewport_runtime()
            .recorded_panel_focus_matches(self.space(), item)
        {
            return;
        }
        if self.panel_is_reachable_in_space(item, cx) {
            return;
        }
        self.viewport_runtime().record_no_panel_focus(self.space());
    }

    fn panel_is_reachable_in_space(&self, item: &DockItemId, cx: &mut Context<Self>) -> bool {
        let space = self.space().clone();
        let controller = self.controller.clone();
        cx.read_entity(&controller, |controller, _| {
            controller
                .graph()
                .find_item_in_space(&space, item)
                .is_some()
        })
    }

    #[cfg(test)]
    pub(crate) fn pending_focus_command(&self) -> Option<&DockViewportFocusCommand> {
        self.interaction.pending_focus_command()
    }

    fn take_pending_focus_command_generation(
        &mut self,
        generation: u64,
    ) -> Option<DockViewportFocusCommand> {
        let command = self
            .interaction
            .take_pending_focus_command_if_generation(generation);
        if command.is_some() {
            self.pending_focus_completion = None;
        }
        command
    }

    fn settle_pending_focus_command_generation(
        &mut self,
        generation: u64,
        outcome: DockSurfaceActivationOutcome,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(command) = self.take_pending_focus_command_generation(generation) else {
            return false;
        };
        if let Some(binding) = command.surface_activation_binding() {
            binding.settle(outcome, cx);
        }
        true
    }

    fn ensure_pending_panel_focus_completion(
        &mut self,
        ticket: &DockPendingFocusCommand,
        item: &DockItemId,
        focus_handle: &FocusHandle,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self
            .pending_focus_completion
            .as_ref()
            .is_some_and(|completion| {
                completion.ticket == *ticket && completion.target.as_ref() == Some(focus_handle)
            })
        {
            return;
        }

        self.pending_focus_completion = None;
        let focus_ticket = ticket.clone();
        let focus_item = item.clone();
        let completion_generation = ticket.generation();
        let completion_binding = self.current_window_binding();
        let completion_window_id = window.window_handle().window_id();
        let subscription = cx.focus_with_completion(
            focus_handle,
            window,
            move |outcome, host, callback_window, cx| {
                if !host.accepts_window_callback(
                    completion_binding,
                    callback_window.window_handle().window_id(),
                ) || !host.is_current_bound_window_id(completion_window_id)
                {
                    return;
                }
                let Some(ticket) = host.interaction.pending_focus_command_ticket() else {
                    return;
                };
                if ticket.generation() != completion_generation {
                    return;
                }
                if ticket
                    .command()
                    .surface_activation_binding()
                    .is_some_and(|binding| !binding.is_current(cx))
                {
                    host.settle_pending_focus_command_generation(
                        completion_generation,
                        DockSurfaceActivationOutcome::Unavailable,
                        cx,
                    );
                    return;
                }
                let Some(command) =
                    host.take_pending_focus_command_generation(completion_generation)
                else {
                    return;
                };
                if outcome == FocusClaimOutcome::Committed {
                    host.remember_panel_focus(focus_item, cx);
                }
                if let Some(binding) = command.surface_activation_binding() {
                    binding.settle(outcome.into(), cx);
                }
                cx.notify();
            },
        );
        if self.interaction.pending_focus_command_ticket().as_ref() != Some(ticket) {
            drop(subscription);
            return;
        }
        self.pending_focus_completion = Some(DockPendingFocusCompletion {
            ticket: focus_ticket,
            target: Some(focus_handle.clone()),
            _subscription: subscription,
        });
    }

    fn ensure_pending_no_panel_focus_completion(
        &mut self,
        ticket: &DockPendingFocusCommand,
        target: Option<&FocusHandle>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let target = target.cloned();
        if self
            .pending_focus_completion
            .as_ref()
            .is_some_and(|completion| completion.ticket == *ticket && completion.target == target)
        {
            return;
        }

        self.pending_focus_completion = None;
        let focus_ticket = ticket.clone();
        let completion_generation = ticket.generation();
        let completion_binding = self.current_window_binding();
        let completion_window_id = window.window_handle().window_id();
        let completion = move |outcome: FocusClaimOutcome,
                               host: &mut DockHost,
                               window: &mut Window,
                               cx: &mut Context<DockHost>| {
            if !host.accepts_window_callback(completion_binding, window.window_handle().window_id())
                || !host.is_current_bound_window_id(completion_window_id)
            {
                return;
            }
            let Some(ticket) = host.interaction.pending_focus_command_ticket() else {
                return;
            };
            if ticket.generation() != completion_generation {
                return;
            }
            if ticket
                .command()
                .surface_activation_binding()
                .is_some_and(|binding| !binding.is_current(cx))
            {
                host.settle_pending_focus_command_generation(
                    completion_generation,
                    DockSurfaceActivationOutcome::Unavailable,
                    cx,
                );
                return;
            }
            let Some(command) = host.take_pending_focus_command_generation(completion_generation)
            else {
                return;
            };
            let committed_focus = window.committed_focus(cx);
            if !host.focus_belongs_to_panel(committed_focus.as_ref(), window) {
                host.viewport_runtime().record_no_panel_focus(host.space());
            }
            if let Some(binding) = command.surface_activation_binding() {
                binding.settle(outcome.into(), cx);
            }
            cx.notify();
        };
        let subscription = match target.as_ref() {
            Some(target) => cx.focus_with_completion(target, window, completion),
            None => cx.blur_with_completion(window, completion),
        };
        if self.interaction.pending_focus_command_ticket().as_ref() != Some(ticket) {
            drop(subscription);
            return;
        }
        self.pending_focus_completion = Some(DockPendingFocusCompletion {
            ticket: focus_ticket,
            target,
            _subscription: subscription,
        });
    }

    fn no_panel_focus_settlement(
        &self,
        window: &Window,
        cx: &Context<Self>,
    ) -> DockNoPanelFocusSettlement {
        let current_focus = window.focused(cx);
        let committed_focus = window.committed_focus(cx);
        let current_is_panel = self.focus_belongs_to_panel(current_focus.as_ref(), window);

        match current_focus {
            Some(current) if !current_is_panel => DockNoPanelFocusSettlement::Focus(current),
            Some(_) => committed_focus
                .filter(|focus| !self.focus_belongs_to_panel(Some(focus), window))
                .map(DockNoPanelFocusSettlement::Focus)
                .unwrap_or(DockNoPanelFocusSettlement::Blur),
            None => DockNoPanelFocusSettlement::Blur,
        }
    }

    fn focus_belongs_to_panel(&self, focus: Option<&FocusHandle>, window: &Window) -> bool {
        focus.is_some_and(|focus| {
            self.panel_focus_trackers
                .values()
                .any(|tracker| tracker.focus_handle.contains(focus, window))
        })
    }

    pub(crate) fn capture_payload_focus_snapshot(
        &self,
        payload: &DockDragPayload,
        window: &Window,
        cx: &Context<Self>,
    ) -> Option<DockLiveUndockSourceFocusSnapshot> {
        let focused = window.committed_focus(cx)?;
        let payload_items = match &payload.kind {
            DockDragPayloadKind::Item { item } => vec![item.clone()],
            DockDragPayloadKind::Tabs | DockDragPayloadKind::Floating { .. } => self
                .with_workspace(cx, |workspace| {
                    workspace
                        .graph()
                        .collect_items_in_subtree(payload.source_node)
                }),
        };
        payload_items
            .iter()
            .any(|item| {
                self.panel_focus_trackers
                    .get(item)
                    .is_some_and(|tracker| tracker.focus_handle.contains(&focused, window))
            })
            .then(|| DockLiveUndockSourceFocusSnapshot::new(focused, window.focus_claim_revision()))
    }

    #[cfg(test)]
    pub(crate) fn recorded_had_panel_focus(&self) -> Option<bool> {
        self.viewport_runtime()
            .recorded_had_panel_focus_for_test(self.space())
    }

    pub(crate) fn ensure_panel_focus_tracker(
        &mut self,
        item: &DockItemId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> FocusHandle {
        if let Some(tracker) = self.panel_focus_trackers.get(item) {
            return tracker.focus_handle.clone();
        }

        let focus_handle = cx.focus_handle();
        let focus_item = item.clone();
        let binding = self.current_window_binding();
        let subscription = cx.on_focus_in(&focus_handle, window, move |host, window, cx| {
            if !host.accepts_window_callback(binding, window.window_handle().window_id()) {
                return;
            }
            host.remember_panel_focus(focus_item.clone(), cx);
            cx.notify();
        });
        self.panel_focus_trackers.insert(
            item.clone(),
            DockPanelFocusTracker {
                focus_handle: focus_handle.clone(),
                _subscription: subscription,
            },
        );
        focus_handle
    }

    pub(crate) fn sync_panel_focus_trackers(
        &mut self,
        visible_items: &[DockItemId],
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.panel_focus_trackers
            .retain(|item, _| visible_items.contains(item));

        for item in visible_items {
            self.ensure_panel_focus_tracker(item, window, cx);
        }
    }

    #[cfg(test)]
    pub(crate) fn debug_instrumentation(&self) -> &DockDebugInstrumentation {
        &self.debug
    }

    #[cfg(test)]
    pub(crate) fn debug_instrumentation_mut(&mut self) -> &mut DockDebugInstrumentation {
        &mut self.debug
    }
}
