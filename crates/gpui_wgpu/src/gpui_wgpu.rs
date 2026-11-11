mod cosmic_text_system;
mod surface_lifecycle;
mod wgpu_atlas;
mod wgpu_context;
mod wgpu_renderer;

pub use cosmic_text_system::*;
#[doc(hidden)]
pub use surface_lifecycle::WgpuSurfaceShutdownProgress;
pub use wgpu;
pub use wgpu_atlas::*;
pub use wgpu_context::{CompositorGpuHint, DeviceErrorState, WgpuBackend, WgpuContext};
#[cfg(target_family = "wasm")]
pub use wgpu_context::{PreparedWebGraphics, WebBackendPreference, WebGpuContextOptions};
#[cfg(all(
    not(target_family = "wasm"),
    any(test, feature = "bench-support", feature = "test-support")
))]
pub use wgpu_renderer::WgpuHeadlessRenderer;
pub use wgpu_renderer::{
    GpuContext, WgpuRenderer, WgpuSceneRecording, WgpuSceneRenderer, WgpuSurfaceConfig,
};
