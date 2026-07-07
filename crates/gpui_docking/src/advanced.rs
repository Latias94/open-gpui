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
    DockViewportPlatformCapabilityRecord, DockViewportPlatformFlagCapabilityRecord,
    DockViewportPlatformRequestStatus, DockViewportPlatformSyncAction,
    DockViewportPlatformSyncRecord, DockViewportPlatformSyncRequest,
    DockViewportPlatformSyncSkipped, DockViewportPlatformSyncSkippedReason,
    DockViewportPlatformSyncUnsupported, DockViewportPlatformSyncUnsupportedReason,
    DockViewportPlatformWindowState, DockViewportReleaseUnavailableRecord,
    DockViewportRestoreReadinessRecord, DockViewportRouteRecord, DockViewportRouteSelectionRecord,
    DockViewportRouteStatus, DockViewportRouteTarget, DockViewportRuntimeStatus,
    DockViewportStaleStatusReason, DockViewportTearOffOutcomeKind,
    DockViewportTearOffPlacementRecord, DockViewportTearOffRecord,
    DockViewportVisualAffordanceRecord,
};
pub use crate::viewport_tear_off::DockViewportTearOffCancelReason;
