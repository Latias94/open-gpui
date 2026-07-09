#![doc = include_str!("../README.md")]
#![warn(missing_docs)]

pub mod adapters;
#[cfg(feature = "command")]
pub mod command;
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
#[cfg(feature = "resource")]
pub mod resource;
mod snapshot;
mod target;
pub mod timeline;
#[cfg(feature = "ui-components")]
pub mod ui_components;

pub use domain::{DevtoolsCapture, DevtoolsDomainId, DevtoolsDomainKind, DevtoolsDomainSnapshot};
pub use event::{
    DEFAULT_DEVTOOLS_EVENT_LIMIT, DevtoolsEventBatch, DevtoolsEventKind, DevtoolsEventRecord,
    DevtoolsEventRecorder,
};
#[cfg(feature = "gpui")]
pub use gpui::DevtoolsInspector;
pub use inspector::{
    DevtoolsDomainRow, DevtoolsEventRow, DevtoolsInspectorDetail, DevtoolsInspectorDetailKind,
    DevtoolsInspectorError, DevtoolsInspectorState, DevtoolsSnapshotCategory,
    DevtoolsSnapshotCategorySummary, DevtoolsSnapshotRow, DevtoolsTargetRow,
};
pub use layout::{
    LayoutBoundsSnapshot, LayoutNodeSnapshot, LayoutPointSnapshot, LayoutSizeSnapshot,
    LayoutSnapshot,
};
pub use probe::{DevtoolsProbe, ProbeId, ProbeSnapshotError, SnapshotProbe, SnapshotProbeSnapshot};
pub use redaction::SnapshotRedactionSummary;
pub use registry::{DevtoolsRegistry, DevtoolsRegistryError};
pub use snapshot::{
    SnapshotCollection, SnapshotDiagnostic, SnapshotEnvelope, SnapshotKind, SnapshotNode,
    SnapshotTree,
};
pub use target::{
    DevtoolsTargetId, DevtoolsTargetKind, DevtoolsTargetSnapshot, DevtoolsTargetTree,
};
pub use timeline::{DEFAULT_TIMELINE_EVENT_LIMIT, TimelineEventSnapshot, TimelineSnapshot};
