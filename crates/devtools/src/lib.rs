#![doc = include_str!("../README.md")]
#![warn(missing_docs)]

pub mod adapters;
#[cfg(feature = "command")]
pub mod command;
#[cfg(feature = "docking")]
pub mod docking;
#[cfg(feature = "form")]
pub mod form;
#[cfg(feature = "gpui")]
pub mod gpui;
mod inspector;
#[cfg(feature = "motion")]
pub mod motion;
mod probe;
mod redaction;
mod registry;
#[cfg(feature = "resource")]
pub mod resource;
mod snapshot;
#[cfg(feature = "ui-components")]
pub mod ui_components;

#[cfg(feature = "gpui")]
pub use gpui::DevtoolsInspector;
pub use inspector::{DevtoolsInspectorError, DevtoolsInspectorState, DevtoolsSnapshotRow};
pub use probe::{DevtoolsProbe, ProbeId, ProbeSnapshotError, SnapshotProbe, SnapshotProbeSnapshot};
pub use redaction::SnapshotRedactionSummary;
pub use registry::{DevtoolsRegistry, DevtoolsRegistryError};
pub use snapshot::{
    SnapshotCollection, SnapshotDiagnostic, SnapshotEnvelope, SnapshotKind, SnapshotNode,
    SnapshotTree,
};
