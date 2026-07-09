//! GPUI read-only inspector surface.

mod inspector;
mod render;
mod runtime;

pub use inspector::{DevtoolsInspector, DevtoolsInspectorController};
pub use runtime::{
    GpuiRuntimeFocusSnapshot, GpuiRuntimeFrameSnapshot, GpuiRuntimeInputSnapshot,
    GpuiRuntimePointSnapshot, GpuiRuntimeRectSnapshot, GpuiRuntimeScrollSnapshot,
    GpuiRuntimeSizeSnapshot, GpuiRuntimeSnapshot, GpuiRuntimeWindowSnapshot, gpui_runtime_capture,
    gpui_runtime_capture_provider, gpui_runtime_probe_snapshot,
    scroll_viewport_layout_probe_snapshot, scroll_viewport_layout_snapshot,
    scroll_viewport_probe_snapshot, scroll_viewport_unavailable_diagnostic,
};
