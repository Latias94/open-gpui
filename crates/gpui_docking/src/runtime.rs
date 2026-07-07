//! Explicit low-level docking runtime APIs.
//!
//! Normal applications should use [`crate::DockSurface`] for host windows and typed viewport
//! outcomes. This module is for integrations that need direct runtime handle control.

pub use crate::{
    host::DockHost, host::DockHostOptions, viewport_close::DockViewportCloseOutcome,
    viewport_close::DockViewportClosePolicy, viewport_close::DockViewportCloseStatus,
    viewport_close::DockViewportShouldCloseOutcome, viewport_close::DockViewportShouldCloseStatus,
    viewport_close::DockViewportUnregisterOutcome, viewport_close::DockViewportUnregisterReason,
    viewport_focus::DockViewportFocusRequest, viewport_open::DockViewportOpenOutcome,
    viewport_open::DockViewportOpenStatus,
    viewport_placement_adapter::DockViewportRestoreReadiness,
    viewport_placement_validation::DockViewportPlacementValidationError,
    viewport_runtime_handle::DockViewportRuntimeHandle,
};
