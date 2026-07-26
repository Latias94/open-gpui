//! Advanced diagnostics and transition internals for explicit opt-in tooling.
//!
//! Types in this module are public so applications can build diagnostics and tests, but they are
//! intentionally absent from the crate root and [`crate::prelude`].

pub use crate::debug::{DockVisualAffordanceDebugLayer, DockVisualAffordanceDebugSummary};
pub use crate::transition_executor::DockTransitionExecutionState;
pub use crate::transition_geometry::DockTransitionPlan;
pub use crate::viewport_runtime_status::{
    DockViewportActivationRecord, DockViewportCoordinateSpaceRecord,
    DockViewportCoordinateStatusRecord, DockViewportDropOutcomeKind, DockViewportDropOutcomeRecord,
    DockViewportInputStatus, DockViewportLifecycleRecord, DockViewportPayloadRecord,
    DockViewportPlatformCapabilityRecord, DockViewportPlatformRequestStatus,
    DockViewportPlatformSyncAction, DockViewportPlatformSyncDispatch,
    DockViewportPlatformSyncDomain, DockViewportPlatformSyncObservation,
    DockViewportPlatformSyncObservationOutcome, DockViewportPlatformSyncObservedRecord,
    DockViewportPlatformSyncRecord, DockViewportPlatformSyncRejected,
    DockViewportPlatformSyncRejectedReason, DockViewportPlatformSyncRequest,
    DockViewportPlatformSyncUnsupported, DockViewportPlatformSyncUnsupportedReason,
    DockViewportReleaseUnavailableRecord, DockViewportRestoreReadinessRecord,
    DockViewportRouteRecord, DockViewportRouteSelectionRecord, DockViewportRouteStatus,
    DockViewportRouteTarget, DockViewportRuntimeStatus, DockViewportStaleStatusReason,
    DockViewportTearOffOutcomeKind, DockViewportTearOffPlacementRecord, DockViewportTearOffRecord,
    DockViewportVisualAffordanceRecord, DockViewportWindowMutationCapabilityRecord,
};
pub use crate::viewport_tear_off::DockViewportTearOffCancelReason;
