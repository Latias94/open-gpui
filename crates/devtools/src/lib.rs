#![doc = include_str!("../README.md")]
#![warn(missing_docs)]

pub mod adapters;
#[cfg(feature = "command")]
pub mod command;
mod diff;
#[cfg(feature = "docking")]
pub mod docking;
mod domain;
mod event;
#[cfg(feature = "form")]
pub mod form;
#[cfg(feature = "gpui")]
pub mod gpui;
mod inspector;
pub mod layout;
#[cfg(feature = "motion")]
pub mod motion;
mod probe;
mod redaction;
mod registry;
mod report;
#[cfg(feature = "resource")]
pub mod resource;
mod session;
mod snapshot;
mod target;
pub mod timeline;
#[cfg(feature = "ui-components")]
pub mod ui_components;
mod workbench;

pub use diff::{
    DevtoolsCaptureDiff, DevtoolsDiffKind, DevtoolsDiffRow, DevtoolsDiffStatus, DevtoolsDiffSummary,
};
pub use domain::{DevtoolsCapture, DevtoolsDomainId, DevtoolsDomainKind, DevtoolsDomainSnapshot};
pub use event::{
    DEFAULT_DEVTOOLS_EVENT_LIMIT, DEFAULT_DEVTOOLS_EVENT_SCOPE_ID,
    DEFAULT_DEVTOOLS_EVENT_SCOPE_LABEL, DevtoolsEventBatch, DevtoolsEventIdentity,
    DevtoolsEventKind, DevtoolsEventRecord, DevtoolsEventRecorder,
};
#[cfg(feature = "gpui")]
pub use gpui::{
    DevtoolsInspector, DevtoolsInspectorController, GpuiRuntimeFocusSnapshot,
    GpuiRuntimeFrameSnapshot, GpuiRuntimeInputSnapshot, GpuiRuntimePointSnapshot,
    GpuiRuntimeRectSnapshot, GpuiRuntimeScrollSnapshot, GpuiRuntimeSizeSnapshot,
    GpuiRuntimeSnapshot, GpuiRuntimeWindowSnapshot, gpui_runtime_capture,
    gpui_runtime_capture_provider, gpui_runtime_probe_snapshot,
};
pub use inspector::{
    DevtoolsDomainRow, DevtoolsEventRow, DevtoolsInspectorCaptureExport, DevtoolsInspectorDetail,
    DevtoolsInspectorDetailKind, DevtoolsInspectorError, DevtoolsInspectorJsonAction,
    DevtoolsInspectorSessionFrameSummary, DevtoolsInspectorState, DevtoolsSnapshotCategory,
    DevtoolsSnapshotCategorySummary, DevtoolsSnapshotRow, DevtoolsTargetRow,
};
pub use layout::{
    LayoutBoundsSnapshot, LayoutNodeSnapshot, LayoutPointSnapshot, LayoutSizeSnapshot,
    LayoutSnapshot,
};
pub use probe::{
    CaptureProvider, DevtoolsCaptureProvider, DevtoolsProbe, ProbeId, ProbeSnapshotError,
    SnapshotProbe, SnapshotProbeSnapshot,
};
pub use redaction::SnapshotRedactionSummary;
pub use registry::{DevtoolsRegistry, DevtoolsRegistryError};
pub use report::{
    DEVTOOLS_REPORT_SCHEMA_VERSION, DevtoolsReport, DevtoolsReportFinding, DevtoolsReportSeverity,
    DevtoolsReportSource, DevtoolsReportSourceKind, DevtoolsReportSummary,
};
pub use session::{
    DEFAULT_DEVTOOLS_SESSION_HISTORY_LIMIT, DEVTOOLS_SESSION_PROTOCOL_VERSION,
    DEVTOOLS_SESSION_SCHEMA_VERSION, DevtoolsSession, DevtoolsSessionConnectionState,
    DevtoolsSessionError, DevtoolsSessionExport, DevtoolsSessionFrame, DevtoolsSessionImportError,
    DevtoolsSessionImportLimits,
};
pub use snapshot::{
    SnapshotCollection, SnapshotDiagnostic, SnapshotEnvelope, SnapshotKind, SnapshotNode,
    SnapshotTree,
};
pub use target::{
    DevtoolsTargetId, DevtoolsTargetKind, DevtoolsTargetSnapshot, DevtoolsTargetTree,
};
pub use timeline::{DEFAULT_TIMELINE_EVENT_LIMIT, TimelineEventSnapshot, TimelineSnapshot};
pub use workbench::{
    DevtoolsWorkbench, DevtoolsWorkbenchDiffState, DevtoolsWorkbenchRefreshStatus,
};
