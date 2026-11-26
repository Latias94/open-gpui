use crate::{
    CompositorGpuHint, DeviceErrorState, SharedWgpuContext, WgpuAtlas,
    surface_lifecycle::{
        SurfaceAcquireDecision, SurfaceAcquireEvent, SurfaceConfigureTicket, SurfaceRecreateTicket,
        SurfaceRenderPlan, WgpuSurfaceShutdownProgress, WindowSurfaceRuntime,
    },
    wgpu_atlas::WgpuAtlasPresentationReceipt,
};
use anyhow::Context as _;
use bytemuck::{Pod, Zeroable};
use log::warn;
use open_gpui::collections::FxHashMap;
use open_gpui::{
    AtlasTextureInstanceId, AtlasTextureLeaseError, DevicePixels, GpuClipShape, GpuSpecs,
    PaintedPath, PathCurve, PathPaint, PathTile, PlatformWindowPresentOutcome, PrimitiveBatch,
    Scene, Size, WindowPresentationShutdownTicket, get_gamma_correction_ratios, quad_depth,
    quad_depth_scale,
};
#[cfg(test)]
use open_gpui::{Bounds, ClipEnvelope, PrimitiveTransform, ScaledPixels};
#[cfg(not(target_family = "wasm"))]
use raw_window_handle::{HasDisplayHandle, HasWindowHandle};
use std::cell::RefCell;
use std::num::NonZeroU64;
use std::ops::Range;
use std::rc::Rc;
use std::sync::Arc;
#[cfg(target_family = "wasm")]
use std::sync::atomic::{AtomicBool, Ordering};
#[cfg(not(target_family = "wasm"))]
use std::time::Duration;

const MAX_INSTANCE_BUFFER_SIZE: u64 = 256 * 1024 * 1024;
#[cfg(not(target_family = "wasm"))]
const FRAME_STAGING_CHUNK_SIZE: u64 = 256 * 1024;
#[cfg(not(target_family = "wasm"))]
const MAX_CACHED_FRAME_UPLOAD_SIZE: usize = 256 * 1024;
const INSTANCE_TEXTURE_TEXEL_SIZE: u64 = 16;
const STORAGE_BUFFER_SHADERS: &str = concat!(
    include_str!("shaders.wgsl"),
    include_str!("shaders_storage.wgsl"),
);
const WEBGL_SHADERS: &str = concat!(
    include_str!("shaders.wgsl"),
    include_str!("shaders_webgl.wgsl"),
);
const SUBPIXEL_SHADERS: &str = concat!(
    "enable dual_source_blending;\n",
    include_str!("shaders.wgsl"),
    include_str!("shaders_storage.wgsl"),
    include_str!("shaders_subpixel.wgsl"),
);
mod gpu_data;
use gpu_data::instance_allocation;
mod scene_renderer;
pub use scene_renderer::{WgpuSceneRecording, WgpuSceneRenderer};

/// The RGB working domain used to compose GPUI scene primitives.
///
/// This selects UI composition, independently of target storage encoding, GPU ownership,
/// display gamut or HDR policy. Imported images retain their declared sampled transfer and alpha.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum WgpuSceneColorSpace {
    /// Preserves GPUI's authored sRGB-numeric composition and existing renderer behavior.
    #[default]
    SrgbEncoded,
    /// Composes in linear light with sRGB primaries, decoding authored sRGB before premultiplication.
    ///
    /// An sRGB attachment encodes these values once. Unorm and float attachments store linear RGB.
    LinearSrgb,
}

const COMPOSITING_PREMULTIPLIED_ALPHA: u32 = 1 << 0;
const COMPOSITING_LINEAR_SRGB: u32 = 1 << 1;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct GlobalParams {
    viewport_size: [f32; 2],
    /// Bit 0 premultiplies output RGB by alpha; bit 1 selects linear-sRGB composition.
    compositing_flags: u32,
    clip_shape_count: u32,
    path_paint_base: u32,
    path_curve_base: u32,
    depth_scale: f32,
    quad_count: u32,
}

const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

struct DepthTexture {
    texture: wgpu::Texture,
    view: wgpu::TextureView,
}

fn atlas_receipt_failure_outcome(_error: &AtlasTextureLeaseError) -> PlatformWindowPresentOutcome {
    PlatformWindowPresentOutcome::RepaintRequired
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct GammaParams {
    gamma_ratios: [f32; 4],
    grayscale_enhanced_contrast: f32,
    subpixel_enhanced_contrast: f32,
    is_bgr: u32,
    _pad: u32,
}

pub struct WgpuSurfaceConfig {
    pub size: Size<DevicePixels>,
    pub transparent: bool,
    /// Preferred presentation mode. When `Some`, the renderer will use this
    /// mode if supported by the surface, falling back to `Fifo`.
    /// When `None`, defaults to `Fifo` (VSync).
    ///
    /// Mobile platforms may prefer `Mailbox` (triple-buffering) to avoid
    /// blocking in `get_current_texture()` during lifecycle transitions.
    pub preferred_present_mode: Option<wgpu::PresentMode>,
}

struct WgpuPipelines {
    quads: wgpu::RenderPipeline,
    opaque_quads: wgpu::RenderPipeline,
    shadows: wgpu::RenderPipeline,
    path_tiles: wgpu::RenderPipeline,
    underlines: wgpu::RenderPipeline,
    mono_sprites: wgpu::RenderPipeline,
    subpixel_sprites: wgpu::RenderPipeline,
    poly_sprites: wgpu::RenderPipeline,
}

struct WgpuBindGroupLayouts {
    globals: wgpu::BindGroupLayout,
    instances: wgpu::BindGroupLayout,
    path_tiles: wgpu::BindGroupLayout,
    texture: wgpu::BindGroupLayout,
}

/// Device selection shared by the windows of one native application.
#[derive(Clone, Default)]
pub struct GpuContext {
    current: Rc<RefCell<Option<SharedWgpuContext>>>,
    application_owned: bool,
}

impl GpuContext {
    /// Uses an application-owned device for every window, including later windows.
    /// Device loss is reported to the application so it can rebuild all renderers together.
    pub fn shared(context: SharedWgpuContext) -> Self {
        Self {
            current: Rc::new(RefCell::new(Some(context))),
            application_owned: true,
        }
    }
}

#[derive(Clone)]
enum GpuData {
    Storage(wgpu::Buffer),
    Texture {
        texture: wgpu::Texture,
        view: wgpu::TextureView,
        width: u32,
        height: u32,
    },
}

enum SurfaceCreationTarget {
    #[cfg(not(target_family = "wasm"))]
    RawWindow(raw_window_handle::RawWindowHandle),
    #[cfg(target_family = "wasm")]
    Canvas(web_sys::HtmlCanvasElement),
}

struct SurfaceFactory {
    instance: wgpu::Instance,
    target: SurfaceCreationTarget,
    #[cfg(not(target_family = "wasm"))]
    display: Option<raw_window_handle::RawDisplayHandle>,
}

impl SurfaceFactory {
    #[cfg(not(target_family = "wasm"))]
    fn for_raw_window(
        instance: wgpu::Instance,
        raw_window_handle: raw_window_handle::RawWindowHandle,
        display: Option<raw_window_handle::RawDisplayHandle>,
    ) -> Self {
        Self {
            instance,
            target: SurfaceCreationTarget::RawWindow(raw_window_handle),
            display,
        }
    }

    #[cfg(target_family = "wasm")]
    fn for_canvas(instance: wgpu::Instance, canvas: web_sys::HtmlCanvasElement) -> Self {
        Self {
            instance,
            target: SurfaceCreationTarget::Canvas(canvas),
        }
    }

    fn create(&self) -> anyhow::Result<wgpu::Surface<'static>> {
        match &self.target {
            #[cfg(not(target_family = "wasm"))]
            SurfaceCreationTarget::RawWindow(raw_window_handle) => unsafe {
                // Safety: renderer construction requires this handle to remain valid until the
                // renderer is destroyed or its surface target is replaced.
                self.instance
                    .create_surface_unsafe(wgpu::SurfaceTargetUnsafe::RawHandle {
                        raw_display_handle: self.display,
                        raw_window_handle: *raw_window_handle,
                    })
                    .map_err(|error| anyhow::anyhow!("Failed to create surface: {error}"))
            },
            #[cfg(target_family = "wasm")]
            SurfaceCreationTarget::Canvas(canvas) => self
                .instance
                .create_surface(wgpu::SurfaceTarget::Canvas(canvas.clone()))
                .map_err(|error| anyhow::anyhow!("Failed to create surface: {error}")),
        }
    }
}

/// GPU resources that must be dropped together during device recovery.
struct WgpuResources {
    device: Arc<wgpu::Device>,
    queue: Arc<wgpu::Queue>,
    pipelines: WgpuPipelines,
    bind_group_layouts: WgpuBindGroupLayouts,
    atlas_sampler: wgpu::Sampler,
    atlas_texture_bind_groups: FxHashMap<AtlasTextureInstanceId, AtlasTextureBindGroup>,
    #[cfg(any(test, feature = "test-support"))]
    atlas_bind_group_creations: u64,
    #[cfg(any(test, feature = "test-support"))]
    atlas_bind_group_hits: u64,
    #[cfg(any(test, feature = "test-support"))]
    submitted_scene_count: u64,
    globals_buffer: wgpu::Buffer,
    clip_buffer: GpuData,
    globals_bind_group: wgpu::BindGroup,
    instance_buffer: GpuData,
    depth_texture: Option<DepthTexture>,
}

struct AtlasTextureBindGroup {
    view: wgpu::TextureView,
    bind_group: wgpu::BindGroup,
}

struct InstanceBinding {
    bind_group: wgpu::BindGroup,
    first_instance: u32,
}

struct InstanceBindings {
    quads: InstanceBinding,
    shadows: InstanceBinding,
    path_tiles: PathTileBinding,
    underlines: InstanceBinding,
    monochrome_sprites: InstanceBinding,
    subpixel_sprites: InstanceBinding,
    polychrome_sprites: InstanceBinding,
}

struct PathTileBinding {
    tiles: InstanceBinding,
    paint_base: u32,
    curve_base: u32,
}

struct InstanceAllocation {
    offset: u64,
    size: u64,
    first_instance: u32,
}

struct EncoderUpload {
    source: wgpu::Buffer,
    source_offset: u64,
    target: wgpu::Buffer,
    offset: u64,
    size: u64,
}

#[derive(Default)]
struct EncoderUploads {
    copies: Vec<EncoderUpload>,
    bytes: u64,
    retained_data: Vec<GpuData>,
    retained_bindings: Vec<wgpu::BindGroup>,
    #[cfg(not(target_family = "wasm"))]
    staging: Option<NativeFrameStaging>,
}

#[cfg(not(target_family = "wasm"))]
struct CachedFrameUpload {
    target: wgpu::Buffer,
    offset: u64,
    bytes: Vec<u8>,
}

/// Tracks the previous submitted bytes for the native frame's disjoint private buffer ranges.
/// Frame-local slots follow upload order, with address and byte equality required for reuse.
#[cfg(not(target_family = "wasm"))]
struct NativeFrameStaging {
    belt: wgpu::util::StagingBelt,
    previous_uploads: Vec<CachedFrameUpload>,
    next_upload: usize,
}

#[cfg(not(target_family = "wasm"))]
impl NativeFrameStaging {
    fn stage(
        &mut self,
        target: &wgpu::Buffer,
        offset: u64,
        data: &[u8],
        size: NonZeroU64,
    ) -> Result<Option<EncoderUpload>, wgpu::MapRangeError> {
        let cache_index = (data.len() <= MAX_CACHED_FRAME_UPLOAD_SIZE).then_some(self.next_upload);
        self.next_upload += usize::from(cache_index.is_some());
        if let Some(previous) = cache_index.and_then(|index| self.previous_uploads.get(index))
            && previous.target == *target
            && previous.offset == offset
            && previous.bytes.as_slice() == data
        {
            return Ok(None);
        }

        let source = self.belt.allocate(
            size,
            const { NonZeroU64::new(wgpu::COPY_BUFFER_ALIGNMENT).unwrap() },
        );
        source.get_mapped_range_mut()?.copy_from_slice(data);
        let upload = EncoderUpload {
            source: source.buffer().clone(),
            source_offset: source.offset(),
            target: target.clone(),
            offset,
            size: size.get(),
        };
        if let Some(index) = cache_index {
            if let Some(previous) = self.previous_uploads.get_mut(index) {
                previous.target.clone_from(target);
                previous.offset = offset;
                previous.bytes.clear();
                previous.bytes.extend_from_slice(data);
            } else {
                self.previous_uploads.push(CachedFrameUpload {
                    target: target.clone(),
                    offset,
                    bytes: data.to_vec(),
                });
            }
        }
        Ok(Some(upload))
    }
}

impl EncoderUploads {
    #[cfg(not(target_family = "wasm"))]
    fn for_native_frame(device: &wgpu::Device) -> Self {
        let chunk_size = FRAME_STAGING_CHUNK_SIZE.min(device.limits().max_buffer_size);
        Self {
            staging: Some(NativeFrameStaging {
                belt: wgpu::util::StagingBelt::new(
                    device.clone(),
                    chunk_size - chunk_size % wgpu::COPY_BUFFER_ALIGNMENT,
                ),
                previous_uploads: Vec::new(),
                next_upload: 0,
            }),
            ..Self::default()
        }
    }

    fn retains_resources(&self) -> bool {
        #[cfg(not(target_family = "wasm"))]
        if self.staging.is_some() {
            return false;
        }
        true
    }

    fn encode_copies(&self, encoder: &mut wgpu::CommandEncoder) {
        for upload in &self.copies {
            encoder.copy_buffer_to_buffer(
                &upload.source,
                upload.source_offset,
                &upload.target,
                upload.offset,
                upload.size,
            );
        }
    }
}

struct WgpuRendererCore {
    resources: WgpuResources,
    atlas: Arc<WgpuAtlas>,
    gamma_offset: u64,
    instance_buffer_capacity: u64,
    clip_buffer_capacity: u64,
    max_buffer_size: u64,
    max_clip_buffer_size: u64,
    storage_buffer_alignment: u64,
    rendering_params: RenderingParameters,
    is_bgr: bool,
    dual_source_blending: bool,
    adapter_info: wgpu::AdapterInfo,
    target_format: wgpu::TextureFormat,
    color_space: WgpuSceneColorSpace,
    max_texture_size: u32,
    uses_webgl_instance_data: bool,
    encoder_uploads: Option<EncoderUploads>,
    #[cfg(not(target_family = "wasm"))]
    native_frame_uploads: Option<EncoderUploads>,
    frame_size: Size<DevicePixels>,
    premultiplied_alpha: bool,
    clip_shape_count: u32,
    transient_attachments: bool,
}

impl WgpuResources {
    fn invalidate_depth_texture(&mut self) {
        self.depth_texture = None;
    }
}

pub struct WgpuRenderer {
    /// Shared GPU context for device recovery coordination (unused on WASM).
    #[allow(dead_code)]
    context: Option<GpuContext>,
    /// Compositor GPU hint for adapter selection (unused on WASM).
    #[allow(dead_code)]
    compositor_gpu: Option<CompositorGpuHint>,
    surface_factory: SurfaceFactory,
    surface: Option<wgpu::Surface<'static>>,
    core: Option<WgpuRendererCore>,
    surface_config: wgpu::SurfaceConfiguration,
    requested_transparency: bool,
    is_bgr: bool,
    atlas: Arc<WgpuAtlas>,
    transparent_alpha_mode: wgpu::CompositeAlphaMode,
    opaque_alpha_mode: wgpu::CompositeAlphaMode,
    max_texture_size: u32,
    device_errors: Arc<DeviceErrorState>,
    observed_error_generation: u64,
    surface_lifecycle: WindowSurfaceRuntime,
    last_submission: Option<wgpu::SubmissionIndex>,
}

impl WgpuRenderer {
    fn core(&self) -> &WgpuRendererCore {
        self.core.as_ref().expect("GPU resources not available")
    }

    fn core_mut(&mut self) -> &mut WgpuRendererCore {
        self.core.as_mut().expect("GPU resources not available")
    }

    fn resources(&self) -> &WgpuResources {
        &self.core().resources
    }

    fn resources_mut(&mut self) -> &mut WgpuResources {
        &mut self.core_mut().resources
    }

    /// Creates a new WgpuRenderer from raw window handles.
    ///
    /// The `gpu_context` is a shared reference that coordinates GPU context across
    /// multiple windows. The first window to create a renderer will initialize the
    /// context; subsequent windows will share it.
    ///
    /// # Safety
    /// The caller must ensure that the window handle remains valid for the lifetime
    /// of the returned renderer.
    #[cfg(not(target_family = "wasm"))]
    pub fn new<W>(
        gpu_context: GpuContext,
        window: &W,
        config: WgpuSurfaceConfig,
        compositor_gpu: Option<CompositorGpuHint>,
    ) -> anyhow::Result<Self>
    where
        W: HasWindowHandle + HasDisplayHandle + std::fmt::Debug + Send + Sync + Clone + 'static,
    {
        let window_handle = window
            .window_handle()
            .map_err(|e| anyhow::anyhow!("Failed to get window handle: {e}"))?;

        // Use the existing context's instance if available, otherwise create a new one.
        // The surface must be created with the same instance that will be used for
        // adapter selection, otherwise wgpu will panic.
        let instance = gpu_context
            .current
            .borrow()
            .as_ref()
            .map(|ctx| ctx.instance.clone())
            .unwrap_or_else(|| SharedWgpuContext::instance(Box::new(window.clone())));

        let display = if gpu_context.application_owned {
            Some(window.display_handle()?.as_raw())
        } else {
            None
        };
        let surface_factory =
            SurfaceFactory::for_raw_window(instance.clone(), window_handle.as_raw(), display);
        let surface = surface_factory.create()?;

        let mut ctx_ref = gpu_context.current.borrow_mut();
        let surface_configured = ctx_ref.is_none();
        let context = match ctx_ref.as_mut() {
            Some(context) => {
                anyhow::ensure!(!context.device_lost(), "shared GPU device was lost");
                context.check_compatible_with_surface(&surface)?;
                context
            }
            None => ctx_ref.insert(SharedWgpuContext::new(
                instance,
                &surface,
                &config,
                compositor_gpu,
            )?),
        };

        let atlas = Arc::new(WgpuAtlas::from_context(context));

        Self::new_internal(
            Some(gpu_context.clone()),
            context,
            (surface_factory, surface),
            config,
            surface_configured,
            compositor_gpu,
            atlas,
        )
    }

    #[cfg(target_family = "wasm")]
    pub fn new_from_canvas(
        context: &SharedWgpuContext,
        canvas: &web_sys::HtmlCanvasElement,
        config: WgpuSurfaceConfig,
    ) -> anyhow::Result<Self> {
        let surface_factory = SurfaceFactory::for_canvas(context.instance.clone(), canvas.clone());
        let surface = surface_factory.create()?;

        Self::new_from_prepared_surface(context, canvas, surface, config, surface_factory)
    }

    #[cfg(target_family = "wasm")]
    pub fn new_from_surface(
        context: &SharedWgpuContext,
        canvas: &web_sys::HtmlCanvasElement,
        surface: wgpu::Surface<'static>,
        config: WgpuSurfaceConfig,
    ) -> anyhow::Result<Self> {
        let factory = SurfaceFactory::for_canvas(context.instance.clone(), canvas.clone());
        Self::new_from_prepared_surface(context, canvas, surface, config, factory)
    }

    #[cfg(target_family = "wasm")]
    fn new_from_prepared_surface(
        context: &SharedWgpuContext,
        _canvas: &web_sys::HtmlCanvasElement,
        surface: wgpu::Surface<'static>,
        config: WgpuSurfaceConfig,
        factory: SurfaceFactory,
    ) -> anyhow::Result<Self> {
        context.check_compatible_with_surface(&surface)?;
        let atlas = Arc::new(WgpuAtlas::from_context(context));
        Self::new_internal(
            None,
            context,
            (factory, surface),
            config,
            false,
            None,
            atlas,
        )
    }

    pub(crate) fn surface_configuration(
        surface: &wgpu::Surface<'_>,
        adapter: &wgpu::Adapter,
        device: &wgpu::Device,
        config: &WgpuSurfaceConfig,
    ) -> anyhow::Result<(
        wgpu::SurfaceConfiguration,
        wgpu::CompositeAlphaMode,
        wgpu::CompositeAlphaMode,
    )> {
        let surface_caps = surface.get_capabilities(adapter);
        let preferred_formats = [
            wgpu::TextureFormat::Bgra8Unorm,
            wgpu::TextureFormat::Rgba8Unorm,
        ];
        let surface_format = preferred_formats
            .iter()
            .find(|format| surface_caps.formats.contains(format))
            .copied()
            .or_else(|| {
                surface_caps
                    .formats
                    .iter()
                    .find(|format| !format.has_srgb_suffix())
                    .copied()
            })
            .or_else(|| surface_caps.formats.first().copied())
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "Surface reports no supported texture formats for adapter {:?}",
                    adapter.get_info().name
                )
            })?;

        let pick_alpha_mode =
            |preferences: &[wgpu::CompositeAlphaMode]| -> anyhow::Result<wgpu::CompositeAlphaMode> {
                preferences
                    .iter()
                    .find(|p| surface_caps.alpha_modes.contains(p))
                    .copied()
                    .or_else(|| surface_caps.alpha_modes.first().copied())
                    .ok_or_else(|| {
                        anyhow::anyhow!(
                            "Surface reports no supported alpha modes for adapter {:?}",
                            adapter.get_info().name
                        )
                    })
            };

        let transparent_alpha_mode = pick_alpha_mode(&[
            wgpu::CompositeAlphaMode::PreMultiplied,
            wgpu::CompositeAlphaMode::Inherit,
        ])?;

        let opaque_alpha_mode = pick_alpha_mode(&[
            wgpu::CompositeAlphaMode::Opaque,
            wgpu::CompositeAlphaMode::Inherit,
        ])?;

        let alpha_mode = if config.transparent {
            transparent_alpha_mode
        } else {
            opaque_alpha_mode
        };

        let max_texture_size = device.limits().max_texture_dimension_2d;
        let (_, initial_size) = WindowSurfaceRuntime::new(config.size, max_texture_size);
        // The native chrome blends sRGB-encoded values. If a compositor exposes only the
        // sRGB attachment variant, render through its byte-compatible unorm view.
        let target_format = surface_format.remove_srgb_suffix();
        if target_format != surface_format {
            anyhow::ensure!(
                adapter
                    .get_downlevel_capabilities()
                    .flags
                    .contains(wgpu::DownlevelFlags::SURFACE_VIEW_FORMATS),
                "the WGPU adapter cannot expose an unorm view of its sRGB surface"
            );
        }

        if initial_size.was_clamped {
            warn!(
                "Requested surface size ({}, {}) exceeds maximum texture dimension {}. \
                 Clamping to ({}, {}). Window content may not fill the entire window.",
                config.size.width.0.max(0),
                config.size.height.0.max(0),
                max_texture_size,
                initial_size.width,
                initial_size.height
            );
        }

        let surface_config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: initial_size.width.max(1),
            height: initial_size.height.max(1),
            present_mode: config
                .preferred_present_mode
                .filter(|mode| surface_caps.present_modes.contains(mode))
                .unwrap_or(wgpu::PresentMode::Fifo),
            desired_maximum_frame_latency: 2,
            alpha_mode,
            view_formats: (target_format != surface_format)
                .then_some(target_format)
                .into_iter()
                .collect(),
            color_space: wgpu::SurfaceColorSpace::Auto,
        };
        Ok((surface_config, transparent_alpha_mode, opaque_alpha_mode))
    }

    fn new_internal(
        gpu_context: Option<GpuContext>,
        context: &SharedWgpuContext,
        surface_target: (SurfaceFactory, wgpu::Surface<'static>),
        config: WgpuSurfaceConfig,
        surface_configured: bool,
        compositor_gpu: Option<CompositorGpuHint>,
        atlas: Arc<WgpuAtlas>,
    ) -> anyhow::Result<Self> {
        let observed_error_generation = context.errors().current_generation();
        let (surface_factory, surface) = surface_target;
        let (mut surface_config, transparent_alpha_mode, opaque_alpha_mode) =
            Self::surface_configuration(&surface, &context.adapter, &context.device, &config)?;
        let max_texture_size = context.device.limits().max_texture_dimension_2d;
        let (mut surface_lifecycle, initial_size) =
            WindowSurfaceRuntime::new(config.size, max_texture_size);
        surface_config.width = initial_size.width;
        surface_config.height = initial_size.height;
        if let SurfaceRenderPlan::Configure(ticket) = surface_lifecycle.prepare_render() {
            if !surface_configured {
                surface.configure(&context.device, &surface_config);
            }
            anyhow::ensure!(
                surface_lifecycle.accept_configuration(ticket),
                "initial WGPU surface configuration ticket became stale"
            );
        }

        let core = WgpuRendererCore::new(
            context,
            atlas.clone(),
            surface_config.format.remove_srgb_suffix(),
            surface_config.alpha_mode,
        )?;
        Ok(Self {
            context: gpu_context,
            compositor_gpu,
            surface_factory,
            surface: Some(surface),
            core: Some(core),
            surface_config,
            requested_transparency: config.transparent,
            is_bgr: false,
            atlas,
            transparent_alpha_mode,
            opaque_alpha_mode,
            max_texture_size,
            device_errors: Arc::clone(context.errors()),
            observed_error_generation,
            surface_lifecycle,
            last_submission: None,
        })
    }
}

impl WgpuRendererCore {
    fn resources(&self) -> &WgpuResources {
        &self.resources
    }

    fn resources_mut(&mut self) -> &mut WgpuResources {
        &mut self.resources
    }

    fn new(
        context: &SharedWgpuContext,
        atlas: Arc<WgpuAtlas>,
        surface_format: wgpu::TextureFormat,
        alpha_mode: wgpu::CompositeAlphaMode,
    ) -> anyhow::Result<Self> {
        Self::new_with_transport(
            context,
            atlas,
            surface_format,
            alpha_mode,
            context.uses_webgl_instance_data(),
        )
    }

    fn new_with_transport(
        context: &SharedWgpuContext,
        atlas: Arc<WgpuAtlas>,
        surface_format: wgpu::TextureFormat,
        alpha_mode: wgpu::CompositeAlphaMode,
        uses_webgl_instance_data: bool,
    ) -> anyhow::Result<Self> {
        Self::from_device(
            &context.adapter,
            Arc::clone(&context.device),
            Arc::clone(&context.queue),
            atlas,
            surface_format,
            alpha_mode,
            uses_webgl_instance_data,
        )
    }

    fn from_device(
        adapter: &wgpu::Adapter,
        device: Arc<wgpu::Device>,
        queue: Arc<wgpu::Queue>,
        atlas: Arc<WgpuAtlas>,
        surface_format: wgpu::TextureFormat,
        alpha_mode: wgpu::CompositeAlphaMode,
        uses_webgl_instance_data: bool,
    ) -> anyhow::Result<Self> {
        Self::from_device_with_color_space(
            adapter,
            device,
            queue,
            atlas,
            surface_format,
            alpha_mode,
            uses_webgl_instance_data,
            WgpuSceneColorSpace::SrgbEncoded,
        )
    }

    fn from_device_with_color_space(
        adapter: &wgpu::Adapter,
        device: Arc<wgpu::Device>,
        queue: Arc<wgpu::Queue>,
        atlas: Arc<WgpuAtlas>,
        surface_format: wgpu::TextureFormat,
        alpha_mode: wgpu::CompositeAlphaMode,
        uses_webgl_instance_data: bool,
        color_space: WgpuSceneColorSpace,
    ) -> anyhow::Result<Self> {
        let dual_source_blending = device
            .features()
            .contains(wgpu::Features::DUAL_SOURCE_BLENDING)
            && !uses_webgl_instance_data;
        let rendering_params = RenderingParameters::new();
        let bind_group_layouts = Self::create_bind_group_layouts(&device, uses_webgl_instance_data);
        let pipelines = Self::create_pipelines(
            &device,
            &bind_group_layouts,
            surface_format,
            alpha_mode,
            dual_source_blending,
            uses_webgl_instance_data,
        );

        let atlas_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("atlas_sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        let uniform_alignment = device.limits().min_uniform_buffer_offset_alignment as u64;
        let globals_size = std::mem::size_of::<GlobalParams>() as u64;
        let gamma_size = std::mem::size_of::<GammaParams>() as u64;
        let gamma_offset = globals_size.next_multiple_of(uniform_alignment);

        let globals_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("globals_buffer"),
            size: gamma_offset + gamma_size,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let max_buffer_size = GpuData::maximum_capacity(&device, uses_webgl_instance_data);
        let clip_shape_stride = std::mem::size_of::<GpuClipShape>() as u64;
        let max_clip_buffer_size = max_buffer_size / clip_shape_stride * clip_shape_stride;
        anyhow::ensure!(
            max_clip_buffer_size >= clip_shape_stride,
            "adapter storage-buffer limit cannot represent one GPU clip shape"
        );
        let requested_clip_buffer_capacity = (64 * 1024_u64)
            .next_multiple_of(clip_shape_stride)
            .min(max_clip_buffer_size);
        let (clip_buffer, initial_clip_buffer_capacity) = GpuData::create(
            &device,
            "clip_buffer",
            requested_clip_buffer_capacity,
            uses_webgl_instance_data,
        )?;

        let storage_buffer_alignment = if uses_webgl_instance_data {
            INSTANCE_TEXTURE_TEXEL_SIZE
        } else {
            device.limits().min_storage_buffer_offset_alignment as u64
        };
        let (instance_buffer, initial_instance_buffer_capacity) = GpuData::create(
            &device,
            "instance_buffer",
            (2 * 1024 * 1024).min(max_buffer_size),
            uses_webgl_instance_data,
        )?;

        let globals_bind_group = Self::create_globals_bind_group(
            &device,
            &bind_group_layouts.globals,
            &globals_buffer,
            &clip_buffer,
            &atlas_sampler,
            0,
            gamma_offset,
            initial_clip_buffer_capacity,
            "globals_bind_group",
        );

        let adapter_info = adapter.get_info();
        let max_texture_size = device.limits().max_texture_dimension_2d;

        let resources = WgpuResources {
            device,
            queue,
            pipelines,
            bind_group_layouts,
            atlas_sampler,
            atlas_texture_bind_groups: FxHashMap::default(),
            #[cfg(any(test, feature = "test-support"))]
            atlas_bind_group_creations: 0,
            #[cfg(any(test, feature = "test-support"))]
            atlas_bind_group_hits: 0,
            #[cfg(any(test, feature = "test-support"))]
            submitted_scene_count: 0,
            globals_buffer,
            clip_buffer,
            globals_bind_group,
            instance_buffer,
            depth_texture: None,
        };

        Ok(Self {
            resources,
            atlas,
            gamma_offset,
            instance_buffer_capacity: initial_instance_buffer_capacity,
            clip_buffer_capacity: initial_clip_buffer_capacity,
            max_buffer_size,
            max_clip_buffer_size,
            storage_buffer_alignment,
            rendering_params,
            is_bgr: false,
            dual_source_blending,
            adapter_info,
            target_format: surface_format,
            color_space,
            max_texture_size,
            uses_webgl_instance_data,
            encoder_uploads: None,
            #[cfg(not(target_family = "wasm"))]
            native_frame_uploads: None,
            frame_size: Size::default(),
            premultiplied_alpha: false,
            clip_shape_count: 0,
            transient_attachments: SharedWgpuContext::adapter_supports_transient_attachments(
                adapter,
            ),
        })
    }

    fn create_globals_bind_group(
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
        globals_buffer: &wgpu::Buffer,
        clip_buffer: &GpuData,
        atlas_sampler: &wgpu::Sampler,
        globals_offset: u64,
        gamma_offset: u64,
        clip_buffer_size: u64,
        label: &str,
    ) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some(label),
            layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                        buffer: globals_buffer,
                        offset: globals_offset,
                        size: NonZeroU64::new(std::mem::size_of::<GlobalParams>() as u64),
                    }),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                        buffer: globals_buffer,
                        offset: gamma_offset,
                        size: NonZeroU64::new(std::mem::size_of::<GammaParams>() as u64),
                    }),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: clip_buffer.binding(0, clip_buffer_size),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::Sampler(atlas_sampler),
                },
            ],
        })
    }

    fn create_bind_group_layouts(
        device: &wgpu::Device,
        uses_webgl_instance_data: bool,
    ) -> WgpuBindGroupLayouts {
        let data_binding = |minimum_size| {
            if uses_webgl_instance_data {
                wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Uint,
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                }
            } else {
                wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: true },
                    has_dynamic_offset: false,
                    min_binding_size: minimum_size,
                }
            }
        };
        let globals =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("globals_layout"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: NonZeroU64::new(
                                std::mem::size_of::<GlobalParams>() as u64
                            ),
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: NonZeroU64::new(
                                std::mem::size_of::<GammaParams>() as u64
                            ),
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 2,
                        visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                        ty: data_binding(NonZeroU64::new(
                            std::mem::size_of::<GpuClipShape>() as u64
                        )),
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 3,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                ],
            });

        let storage_buffer_entry = |binding: u32| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
            ty: data_binding(None),
            count: None,
        };

        let instances = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("instances_layout"),
            entries: &[storage_buffer_entry(0)],
        });

        let path_entries = [
            storage_buffer_entry(0),
            storage_buffer_entry(1),
            storage_buffer_entry(2),
        ];
        let path_tiles = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("path_tiles_layout"),
            entries: if uses_webgl_instance_data {
                &path_entries[..1]
            } else {
                &path_entries
            },
        });

        let texture = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("texture_layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            }],
        });

        WgpuBindGroupLayouts {
            globals,
            instances,
            path_tiles,
            texture,
        }
    }

    fn create_pipelines(
        device: &wgpu::Device,
        layouts: &WgpuBindGroupLayouts,
        surface_format: wgpu::TextureFormat,
        alpha_mode: wgpu::CompositeAlphaMode,
        dual_source_blending: bool,
        uses_webgl_instance_data: bool,
    ) -> WgpuPipelines {
        // Diagnostic guard: verify the device actually has
        // DUAL_SOURCE_BLENDING. We have a crash report (ZED-5G1) where a
        // feature mismatch caused a wgpu-hal abort, but we haven't
        // identified the code path that produces the mismatch. This
        // guard prevents the crash and logs more evidence.
        // Remove this check once:
        // a) We find and fix the root cause, or
        // b) There are no reports of this warning appearing for some time.
        let device_has_feature = device
            .features()
            .contains(wgpu::Features::DUAL_SOURCE_BLENDING);
        if dual_source_blending && !device_has_feature {
            log::error!(
                "BUG: dual_source_blending flag is true but device does not \
                 have DUAL_SOURCE_BLENDING enabled (device features: {:?}). \
                 Falling back to mono text rendering. Please report this at \
                 https://github.com/Latias94/open-gpui/issues",
                device.features(),
            );
        }
        let dual_source_blending =
            dual_source_blending && device_has_feature && !uses_webgl_instance_data;

        let base_shader_source = if uses_webgl_instance_data {
            WEBGL_SHADERS
        } else {
            STORAGE_BUFFER_SHADERS
        };
        let shader_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("gpui_shaders"),
            source: wgpu::ShaderSource::Wgsl(std::borrow::Cow::Borrowed(base_shader_source)),
        });

        let subpixel_shader_module = if dual_source_blending {
            Some(device.create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("gpui_subpixel_shaders"),
                source: wgpu::ShaderSource::Wgsl(std::borrow::Cow::Borrowed(SUBPIXEL_SHADERS)),
            }))
        } else {
            None
        };

        let blend_mode = match alpha_mode {
            wgpu::CompositeAlphaMode::PreMultiplied => {
                wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING
            }
            _ => wgpu::BlendState::ALPHA_BLENDING,
        };

        let color_target = wgpu::ColorTargetState {
            format: surface_format,
            blend: Some(blend_mode),
            write_mask: wgpu::ColorWrites::ALL,
        };
        let create_pipeline = |name: &str,
                               vs_entry: &str,
                               fs_entry: &str,
                               globals_layout: &wgpu::BindGroupLayout,
                               data_layout: &wgpu::BindGroupLayout,
                               texture_layout: Option<&wgpu::BindGroupLayout>,
                               topology: wgpu::PrimitiveTopology,
                               color_targets: &[Option<wgpu::ColorTargetState>],
                               depth_write_enabled: bool,
                               module: &wgpu::ShaderModule| {
            let mut bind_group_layouts = vec![Some(globals_layout), Some(data_layout)];
            bind_group_layouts.extend(texture_layout.map(Some));
            let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some(&format!("{name}_layout")),
                bind_group_layouts: &bind_group_layouts,
                immediate_size: 0,
            });

            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(name),
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module,
                    entry_point: Some(vs_entry),
                    buffers: &[],
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module,
                    entry_point: Some(fs_entry),
                    targets: color_targets,
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                }),
                primitive: wgpu::PrimitiveState {
                    topology,
                    strip_index_format: None,
                    front_face: wgpu::FrontFace::Ccw,
                    cull_mode: None,
                    polygon_mode: wgpu::PolygonMode::Fill,
                    unclipped_depth: false,
                    conservative: false,
                },
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: DEPTH_FORMAT,
                    depth_write_enabled: Some(depth_write_enabled),
                    depth_compare: Some(wgpu::CompareFunction::Greater),
                    stencil: Default::default(),
                    bias: Default::default(),
                }),
                multisample: wgpu::MultisampleState {
                    count: 1,
                    mask: !0,
                    alpha_to_coverage_enabled: false,
                },
                multiview_mask: None,
                cache: None,
            })
        };

        let quads = create_pipeline(
            "quads",
            "vs_quad",
            "fs_quad",
            &layouts.globals,
            &layouts.instances,
            None,
            wgpu::PrimitiveTopology::TriangleStrip,
            &[Some(color_target.clone())],
            false,
            &shader_module,
        );

        let opaque_quads = create_pipeline(
            "opaque_quads",
            "vs_opaque_quad",
            "fs_opaque_quad",
            &layouts.globals,
            &layouts.instances,
            None,
            wgpu::PrimitiveTopology::TriangleStrip,
            &[Some(wgpu::ColorTargetState {
                blend: None,
                ..color_target.clone()
            })],
            true,
            &shader_module,
        );

        let shadows = create_pipeline(
            "shadows",
            "vs_shadow",
            "fs_shadow",
            &layouts.globals,
            &layouts.instances,
            None,
            wgpu::PrimitiveTopology::TriangleStrip,
            &[Some(color_target.clone())],
            false,
            &shader_module,
        );

        let path_tiles = create_pipeline(
            "path_tiles",
            "vs_path_tile",
            "fs_path_tile",
            &layouts.globals,
            &layouts.path_tiles,
            None,
            wgpu::PrimitiveTopology::TriangleStrip,
            &[Some(color_target.clone())],
            false,
            &shader_module,
        );

        let underlines = create_pipeline(
            "underlines",
            "vs_underline",
            "fs_underline",
            &layouts.globals,
            &layouts.instances,
            None,
            wgpu::PrimitiveTopology::TriangleStrip,
            &[Some(color_target.clone())],
            false,
            &shader_module,
        );

        let mono_sprites = create_pipeline(
            "mono_sprites",
            "vs_mono_sprite",
            "fs_mono_sprite",
            &layouts.globals,
            &layouts.instances,
            Some(&layouts.texture),
            wgpu::PrimitiveTopology::TriangleStrip,
            &[Some(color_target.clone())],
            false,
            &shader_module,
        );

        let subpixel_sprites = if let Some(subpixel_module) = &subpixel_shader_module {
            let subpixel_blend = wgpu::BlendState {
                color: wgpu::BlendComponent {
                    src_factor: wgpu::BlendFactor::Src1,
                    dst_factor: wgpu::BlendFactor::OneMinusSrc1,
                    operation: wgpu::BlendOperation::Add,
                },
                alpha: wgpu::BlendComponent {
                    src_factor: wgpu::BlendFactor::One,
                    dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                    operation: wgpu::BlendOperation::Add,
                },
            };

            create_pipeline(
                "subpixel_sprites",
                "vs_subpixel_sprite",
                "fs_subpixel_sprite",
                &layouts.globals,
                &layouts.instances,
                Some(&layouts.texture),
                wgpu::PrimitiveTopology::TriangleStrip,
                &[Some(wgpu::ColorTargetState {
                    format: surface_format,
                    blend: Some(subpixel_blend),
                    write_mask: wgpu::ColorWrites::COLOR,
                })],
                false,
                subpixel_module,
            )
        } else {
            create_pipeline(
                "subpixel_sprites_grayscale",
                "vs_subpixel_sprite",
                "fs_mono_sprite",
                &layouts.globals,
                &layouts.instances,
                Some(&layouts.texture),
                wgpu::PrimitiveTopology::TriangleStrip,
                &[Some(color_target.clone())],
                false,
                &shader_module,
            )
        };

        let poly_sprites = create_pipeline(
            "poly_sprites",
            "vs_poly_sprite",
            "fs_poly_sprite",
            &layouts.globals,
            &layouts.instances,
            Some(&layouts.texture),
            wgpu::PrimitiveTopology::TriangleStrip,
            &[Some(color_target.clone())],
            false,
            &shader_module,
        );

        WgpuPipelines {
            quads,
            opaque_quads,
            shadows,
            path_tiles,
            underlines,
            mono_sprites,
            subpixel_sprites,
            poly_sprites,
        }
    }
}

impl WgpuRenderer {
    pub fn update_drawable_size(&mut self, size: Size<DevicePixels>) {
        if self.surface_lifecycle.shutdown_active() {
            return;
        }

        let requested = self.surface_lifecycle.request_resize(size);
        if requested.was_clamped {
            warn!(
                "Requested surface size ({}, {}) exceeds maximum texture dimension {}. \
                 Clamping to ({}, {}). Window content may not fill the entire window.",
                size.width.0.max(0),
                size.height.0.max(0),
                self.max_texture_size,
                requested.width,
                requested.height
            );
        }
        self.surface_config.width = requested.width;
        self.surface_config.height = requested.height;
    }
}

impl WgpuRendererCore {
    fn ensure_depth_texture(&mut self, size: Size<DevicePixels>) {
        if self.resources.depth_texture.as_ref().is_some_and(|depth| {
            depth.texture.width() == size.width.0 as u32
                && depth.texture.height() == size.height.0 as u32
        }) {
            return;
        }
        let width = size.width.0 as u32;
        let height = size.height.0 as u32;
        let usage = if self.transient_attachments {
            wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TRANSIENT_ATTACHMENT
        } else {
            wgpu::TextureUsages::RENDER_ATTACHMENT
        };
        let texture = self
            .resources
            .device
            .create_texture(&wgpu::TextureDescriptor {
                label: Some("gpui_private_depth"),
                size: wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: DEPTH_FORMAT,
                usage,
                view_formats: &[],
            });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        self.resources.depth_texture = Some(DepthTexture { texture, view });
    }
}

impl WgpuRenderer {
    pub fn set_subpixel_layout(&mut self, is_bgr: bool) {
        self.is_bgr = is_bgr;
        if let Some(core) = self.core.as_mut() {
            core.is_bgr = is_bgr;
        }
    }

    /// Sets the native text system's gamma, contrast and subpixel order for this window.
    pub fn set_text_rendering_params(
        &mut self,
        gamma_ratios: [f32; 4],
        grayscale_enhanced_contrast: f32,
        subpixel_enhanced_contrast: f32,
        is_bgr: bool,
    ) {
        self.is_bgr = is_bgr;
        if let Some(core) = &mut self.core {
            core.rendering_params = RenderingParameters {
                gamma_ratios,
                grayscale_enhanced_contrast,
                subpixel_enhanced_contrast,
            };
            core.is_bgr = is_bgr;
        }
    }

    /// Gives the native window scheduler access to the current device's loss observer.
    pub fn device_errors(&self) -> &Arc<DeviceErrorState> {
        &self.device_errors
    }

    pub fn update_transparency(&mut self, transparent: bool) {
        if self.surface_lifecycle.shutdown_active() {
            return;
        }

        self.requested_transparency = transparent;
        let Some(core) = self.core.as_mut() else {
            return;
        };

        let new_alpha_mode = if self.requested_transparency {
            self.transparent_alpha_mode
        } else {
            self.opaque_alpha_mode
        };

        if new_alpha_mode != self.surface_config.alpha_mode {
            self.surface_config.alpha_mode = new_alpha_mode;
            self.surface_lifecycle.request_reconfigure();
            let resources = &mut core.resources;
            resources.pipelines = WgpuRendererCore::create_pipelines(
                &resources.device,
                &resources.bind_group_layouts,
                self.surface_config.format.remove_srgb_suffix(),
                new_alpha_mode,
                core.dual_source_blending,
                core.uses_webgl_instance_data,
            );
        }
    }

    #[allow(dead_code)]
    pub fn viewport_size(&self) -> Size<DevicePixels> {
        Size {
            width: DevicePixels(self.surface_config.width as i32),
            height: DevicePixels(self.surface_config.height as i32),
        }
    }

    pub fn sprite_atlas(&self) -> &Arc<WgpuAtlas> {
        &self.atlas
    }

    pub fn supports_dual_source_blending(&self) -> bool {
        self.core
            .as_ref()
            .is_some_and(|core| core.dual_source_blending)
    }

    /// Returns adapter details while the renderer has GPU resources.
    pub fn gpu_specs(&self) -> Option<GpuSpecs> {
        if self.surface_lifecycle.shutdown_active() || self.device_errors.device_lost() {
            return None;
        }
        let adapter_info = &self.core.as_ref()?.adapter_info;
        Some(GpuSpecs {
            is_software_emulated: adapter_info.device_type == wgpu::DeviceType::Cpu,
            device_name: adapter_info.name.clone(),
            driver_name: adapter_info.driver.clone(),
            driver_info: adapter_info.driver_info.clone(),
        })
    }

    pub fn max_texture_size(&self) -> u32 {
        self.max_texture_size
    }

    fn configure_surface(&mut self, ticket: SurfaceConfigureTicket) -> bool {
        if self.surface_lifecycle.shutdown_active() || self.core.is_none() {
            return false;
        }

        self.surface_config.width = ticket.extent.width;
        self.surface_config.height = ticket.extent.height;
        let surface_config = self.surface_config.clone();
        let Some(surface) = self.surface.as_ref() else {
            return false;
        };
        surface.configure(&self.resources().device, &surface_config);
        self.resources_mut().invalidate_depth_texture();
        self.surface_lifecycle.accept_configuration(ticket)
    }

    fn recreate_surface(&mut self, ticket: SurfaceRecreateTicket) -> anyhow::Result<()> {
        anyhow::ensure!(
            !self.surface_lifecycle.shutdown_active(),
            "cannot recreate a draining presentation surface"
        );
        anyhow::ensure!(
            self.core.is_some(),
            "cannot recreate a presentation surface without active GPU resources"
        );

        self.surface_config.width = ticket.extent.width;
        self.surface_config.height = ticket.extent.height;
        let surface_config = self.surface_config.clone();
        {
            // A lost WGPU surface must be destroyed before another surface is created for the target.
            drop(self.surface.take());
            self.resources_mut().invalidate_depth_texture();
        }
        let surface = self.surface_factory.create()?;
        {
            surface.configure(&self.resources().device, &surface_config);
            self.surface = Some(surface);
        }
        if !self.surface_lifecycle.accept_recreation(ticket) {
            drop(self.surface.take());
            self.resources_mut().invalidate_depth_texture();
            self.surface_lifecycle.reject();
            anyhow::bail!("surface recreation ticket became stale");
        }
        Ok(())
    }

    fn prepare_surface_for_draw(&mut self) -> Option<PlatformWindowPresentOutcome> {
        loop {
            match self.surface_lifecycle.prepare_render() {
                SurfaceRenderPlan::Deferred => {
                    return Some(PlatformWindowPresentOutcome::Deferred);
                }
                SurfaceRenderPlan::Rejected => {
                    return Some(PlatformWindowPresentOutcome::Rejected);
                }
                SurfaceRenderPlan::Configure(ticket) => {
                    if !self.configure_surface(ticket) {
                        self.surface_lifecycle.reject();
                        return Some(PlatformWindowPresentOutcome::Rejected);
                    }
                }
                SurfaceRenderPlan::Recreate(ticket) => {
                    if let Err(error) = self.recreate_surface(ticket) {
                        self.surface_lifecycle.reject_recreation(ticket);
                        log::error!("Failed to recreate lost WGPU surface: {error}");
                        return Some(PlatformWindowPresentOutcome::Rejected);
                    }
                }
                SurfaceRenderPlan::Acquire => {
                    if self.surface.is_some() {
                        return None;
                    }
                    self.surface_lifecycle.reject();
                    return Some(PlatformWindowPresentOutcome::Rejected);
                }
            }
        }
    }

    fn observe_surface_acquire(&mut self, event: SurfaceAcquireEvent) -> SurfaceAcquireDecision {
        self.surface_lifecycle.observe_acquire(event)
    }

    fn settle_surface_acquire_failure(
        &mut self,
        event: SurfaceAcquireEvent,
    ) -> PlatformWindowPresentOutcome {
        match self.observe_surface_acquire(event) {
            SurfaceAcquireDecision::Deferred => PlatformWindowPresentOutcome::Deferred,
            SurfaceAcquireDecision::Rejected => {
                if event == SurfaceAcquireEvent::Validation {
                    log::error!("WGPU surface texture validation failed");
                }
                PlatformWindowPresentOutcome::Rejected
            }
            SurfaceAcquireDecision::Recreate(ticket) => {
                if let Err(error) = self.recreate_surface(ticket) {
                    self.surface_lifecycle.reject_recreation(ticket);
                    log::error!("Failed to recreate lost WGPU surface: {error}");
                    PlatformWindowPresentOutcome::Rejected
                } else {
                    PlatformWindowPresentOutcome::Deferred
                }
            }
            SurfaceAcquireDecision::UseFrame => {
                debug_assert!(false, "surface failure cannot produce a usable frame");
                PlatformWindowPresentOutcome::Rejected
            }
        }
    }

    pub fn draw(&mut self, scene: &Scene) -> PlatformWindowPresentOutcome {
        if self.surface_lifecycle.shutdown_active() {
            return PlatformWindowPresentOutcome::Deferred;
        }
        if self.core.is_none() {
            return PlatformWindowPresentOutcome::Rejected;
        }

        if let Some(outcome) = self.prepare_surface_for_draw() {
            return outcome;
        }

        let last_error = self
            .device_errors
            .observe_error(&mut self.observed_error_generation);
        if let Some(error) = last_error {
            #[cfg(not(target_family = "wasm"))]
            self.core_mut().invalidate_native_uploads();
            log::error!("Uncaptured WGPU error terminated the window surface: {error}");
            self.surface_lifecycle.reject();
            return PlatformWindowPresentOutcome::Rejected;
        }

        let (clip_shape_count, atlas_presentation) = match self.core_mut().prepare_scene(scene) {
            Ok(prepared) => prepared,
            Err(error) => {
                self.last_submission = Some(self.resources().queue.submit(std::iter::empty()));
                if let Some(error) = error.downcast_ref::<AtlasTextureLeaseError>() {
                    warn!(
                        "Atlas texture unavailable while preparing frame; requesting a fresh framework frame: {error}"
                    );
                    return atlas_receipt_failure_outcome(error);
                }
                log::error!("Failed to prepare WGPU scene: {error:#}");
                return PlatformWindowPresentOutcome::Rejected;
            }
        };

        let acquired = self
            .surface
            .as_ref()
            .expect("configured surface is available")
            .get_current_texture();
        let frame = match acquired {
            wgpu::CurrentSurfaceTexture::Success(frame) => {
                if self.observe_surface_acquire(SurfaceAcquireEvent::Success)
                    != SurfaceAcquireDecision::UseFrame
                {
                    return PlatformWindowPresentOutcome::Rejected;
                }
                frame
            }
            wgpu::CurrentSurfaceTexture::Suboptimal(frame) => {
                if self.observe_surface_acquire(SurfaceAcquireEvent::Suboptimal)
                    != SurfaceAcquireDecision::UseFrame
                {
                    return PlatformWindowPresentOutcome::Rejected;
                }
                frame
            }
            wgpu::CurrentSurfaceTexture::Outdated => {
                return self.settle_surface_acquire_failure(SurfaceAcquireEvent::Outdated);
            }
            wgpu::CurrentSurfaceTexture::Timeout => {
                return self.settle_surface_acquire_failure(SurfaceAcquireEvent::Timeout);
            }
            wgpu::CurrentSurfaceTexture::Occluded => {
                return self.settle_surface_acquire_failure(SurfaceAcquireEvent::Occluded);
            }
            wgpu::CurrentSurfaceTexture::Lost => {
                return self.settle_surface_acquire_failure(SurfaceAcquireEvent::Lost);
            }
            wgpu::CurrentSurfaceTexture::Validation => {
                return self.settle_surface_acquire_failure(SurfaceAcquireEvent::Validation);
            }
        };

        let size = Size {
            width: DevicePixels(frame.texture.width() as i32),
            height: DevicePixels(frame.texture.height() as i32),
        };
        let frame_view = frame.texture.create_view(&wgpu::TextureViewDescriptor {
            format: Some(self.core().target_format),
            ..Default::default()
        });

        let premultiplied_alpha =
            self.surface_config.alpha_mode == wgpu::CompositeAlphaMode::PreMultiplied;
        let submission = match self.core_mut().render_prepared_frame(
            scene,
            &frame_view,
            size,
            premultiplied_alpha,
            clip_shape_count,
            &atlas_presentation,
            wgpu::Color::TRANSPARENT,
        ) {
            Ok(submission) => submission,
            Err(error) => {
                self.last_submission = Some(self.resources().queue.submit(std::iter::empty()));
                log::error!("Failed to render WGPU scene: {error:#}");
                return PlatformWindowPresentOutcome::Rejected;
            }
        };
        self.last_submission = Some(submission);
        self.resources().queue.present(frame);
        PlatformWindowPresentOutcome::Submitted
    }

    /// Renders a diagnostic snapshot of the accepted scene using this window's atlas.
    #[cfg(all(not(target_family = "wasm"), any(test, feature = "test-support")))]
    pub fn render_to_image(&mut self, scene: &Scene) -> anyhow::Result<image::RgbaImage> {
        anyhow::ensure!(
            !self.presentation_shutdown_active(),
            "window renderer is retiring"
        );
        anyhow::ensure!(!self.device_lost(), "window GPU device was lost");
        let size = Size {
            width: DevicePixels(self.surface_config.width as i32),
            height: DevicePixels(self.surface_config.height as i32),
        };
        let core = self
            .core
            .as_mut()
            .context("window renderer has no active WGPU device")?;
        let target = HeadlessRenderTarget::new(&core.resources.device, size, core.target_format)?;
        let premultiplied =
            self.surface_config.alpha_mode == wgpu::CompositeAlphaMode::PreMultiplied;
        self.last_submission = Some(core.render_frame(
            scene,
            &target.view,
            size,
            premultiplied,
            wgpu::Color::TRANSPARENT,
        )?);
        let image = target
            .read_image(
                &core.resources.device,
                &core.resources.queue,
                &mut self.last_submission,
            )
            .inspect_err(|_| core.invalidate_native_uploads())?;
        anyhow::ensure!(
            !self.device_lost(),
            "window GPU device was lost during readback"
        );
        if let Some(error) = self
            .device_errors
            .observe_error(&mut self.observed_error_generation)
        {
            self.core_mut().invalidate_native_uploads();
            anyhow::bail!("failed to capture the window scene: {error}");
        }
        Ok(image)
    }
}

impl WgpuRendererCore {
    fn write_gpu_data(
        resources: &WgpuResources,
        uploads: &mut Option<EncoderUploads>,
        target: &GpuData,
        offset: u64,
        data: &[u8],
    ) -> anyhow::Result<()> {
        if uploads.is_none() {
            return target.write(&resources.queue, offset, data);
        }
        let GpuData::Storage(target) = target else {
            anyhow::bail!("external scene recording requires storage instance data");
        };
        Self::write_buffer(resources, uploads, target, offset, data)
    }

    fn write_buffer(
        resources: &WgpuResources,
        uploads: &mut Option<EncoderUploads>,
        target: &wgpu::Buffer,
        offset: u64,
        data: &[u8],
    ) -> anyhow::Result<()> {
        let byte_len = data.len() as u64;
        anyhow::ensure!(
            offset
                .checked_add(byte_len)
                .is_some_and(|end| end <= target.size()),
            "scene upload exceeds its GPU buffer"
        );
        let Some(size) = NonZeroU64::new(byte_len) else {
            return Ok(());
        };
        if let Some(uploads) = uploads {
            anyhow::ensure!(
                offset.is_multiple_of(wgpu::COPY_BUFFER_ALIGNMENT)
                    && byte_len.is_multiple_of(wgpu::COPY_BUFFER_ALIGNMENT),
                "scene upload offset and size must be aligned for WGPU buffer copies"
            );
            #[cfg(not(target_family = "wasm"))]
            if let Some(staging) = &mut uploads.staging {
                match staging.stage(target, offset, data, size) {
                    Ok(upload) => uploads.copies.extend(upload),
                    Err(error) => {
                        uploads.staging = None;
                        return Err(error).context("could not map native frame staging data");
                    }
                }
                return Ok(());
            }
            use wgpu::util::DeviceExt as _;
            let bytes = uploads
                .bytes
                .checked_add(size.get())
                .ok_or_else(|| anyhow::anyhow!("scene upload size overflow"))?;
            anyhow::ensure!(
                bytes <= MAX_INSTANCE_BUFFER_SIZE,
                "recorded scene uploads exceed the renderer's byte limit"
            );
            let source = resources
                .device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("gpui_scene_upload"),
                    contents: data,
                    usage: wgpu::BufferUsages::COPY_SRC,
                });
            uploads.copies.push(EncoderUpload {
                source,
                source_offset: 0,
                target: target.clone(),
                offset,
                size: byte_len,
            });
            uploads.bytes = bytes;
        } else {
            resources.queue.write_buffer(target, offset, data);
        }
        Ok(())
    }

    fn prepare_scene(
        &mut self,
        scene: &Scene,
    ) -> anyhow::Result<(u32, WgpuAtlasPresentationReceipt)> {
        let clip_shape_count = self
            .prepare_clip_buffer(scene.clip_shapes())
            .ok_or_else(|| anyhow::anyhow!("scene clip arena exceeds WGPU buffer limits"))?;
        self.atlas.before_frame();
        let atlas_presentation =
            self.atlas
                .prepare_presentation(scene.batches().filter_map(|batch| match batch {
                    PrimitiveBatch::MonochromeSprites { texture, .. }
                    | PrimitiveBatch::SubpixelSprites { texture, .. }
                    | PrimitiveBatch::PolychromeSprites { texture, .. } => Some(texture),
                    _ => None,
                }))?;
        Ok((clip_shape_count, atlas_presentation))
    }

    #[cfg(all(not(target_family = "wasm"), any(test, feature = "test-support")))]
    fn render_frame(
        &mut self,
        scene: &Scene,
        frame_view: &wgpu::TextureView,
        size: Size<DevicePixels>,
        premultiplied_alpha: bool,
        clear_color: wgpu::Color,
    ) -> anyhow::Result<wgpu::SubmissionIndex> {
        let (clip_shape_count, atlas_presentation) = self.prepare_scene(scene)?;
        self.render_prepared_frame(
            scene,
            frame_view,
            size,
            premultiplied_alpha,
            clip_shape_count,
            &atlas_presentation,
            clear_color,
        )
    }

    fn render_prepared_frame(
        &mut self,
        scene: &Scene,
        frame_view: &wgpu::TextureView,
        size: Size<DevicePixels>,
        premultiplied_alpha: bool,
        clip_shape_count: u32,
        atlas_presentation: &WgpuAtlasPresentationReceipt,
        clear_color: wgpu::Color,
    ) -> anyhow::Result<wgpu::SubmissionIndex> {
        self.prepare_frame_uniforms(size, premultiplied_alpha, clip_shape_count)?;
        self.record_frame(scene, frame_view, atlas_presentation, clear_color)
    }

    fn prepare_frame_uniforms(
        &mut self,
        size: Size<DevicePixels>,
        premultiplied_alpha: bool,
        clip_shape_count: u32,
    ) -> anyhow::Result<()> {
        anyhow::ensure!(
            size.width.0 > 0
                && size.height.0 > 0
                && size.width.0 as u32 <= self.max_texture_size
                && size.height.0 as u32 <= self.max_texture_size,
            "invalid render target size: {size:?}"
        );
        self.ensure_depth_texture(size);
        self.frame_size = size;
        self.premultiplied_alpha = premultiplied_alpha;
        self.clip_shape_count = clip_shape_count;
        Ok(())
    }

    fn record_frame(
        &mut self,
        scene: &Scene,
        frame_view: &wgpu::TextureView,
        atlas_presentation: &WgpuAtlasPresentationReceipt,
        clear_color: wgpu::Color,
    ) -> anyhow::Result<wgpu::SubmissionIndex> {
        let mut encoder =
            self.resources
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("main_encoder"),
                });
        #[cfg(not(target_family = "wasm"))]
        self.begin_native_frame_uploads();
        if let Err(error) = self.encode_frame(
            scene,
            frame_view,
            atlas_presentation,
            &mut encoder,
            wgpu::LoadOp::Clear(clear_color),
        ) {
            drop(encoder);
            #[cfg(not(target_family = "wasm"))]
            {
                self.native_frame_uploads = self.finish_native_frame_uploads(None);
            }
            return Err(error);
        }
        #[cfg(not(target_family = "wasm"))]
        let native_frame_uploads = self.finish_native_frame_uploads(Some(&encoder));
        let submission = self
            .resources
            .queue
            .submit(std::iter::once(encoder.finish()));
        #[cfg(not(target_family = "wasm"))]
        {
            self.native_frame_uploads = native_frame_uploads;
        }
        atlas_presentation.retain_imported_until_completed(&self.resources.queue);
        #[cfg(any(test, feature = "test-support"))]
        {
            self.resources.submitted_scene_count += 1;
        }
        Ok(submission)
    }

    #[cfg(not(target_family = "wasm"))]
    fn begin_native_frame_uploads(&mut self) {
        if !self.uses_webgl_instance_data {
            self.encoder_uploads = Some(
                self.native_frame_uploads
                    .take()
                    .unwrap_or_else(|| EncoderUploads::for_native_frame(&self.resources.device)),
            );
        }
    }

    #[cfg(not(target_family = "wasm"))]
    fn invalidate_native_uploads(&mut self) {
        if let Some(staging) = self
            .native_frame_uploads
            .as_mut()
            .and_then(|uploads| uploads.staging.as_mut())
        {
            staging.previous_uploads.clear();
        }
    }

    #[cfg(not(target_family = "wasm"))]
    fn finish_native_frame_uploads(
        &mut self,
        encoder: Option<&wgpu::CommandEncoder>,
    ) -> Option<EncoderUploads> {
        let mut uploads = self.encoder_uploads.take()?;
        let staging = uploads.staging.as_mut()?;
        if let Some(encoder) = encoder {
            staging.belt.finish_and_recall_on_submit(encoder);
            staging.previous_uploads.truncate(staging.next_upload);
        } else {
            staging.belt.finish();
            staging.belt.recall();
            staging.previous_uploads.clear();
        }
        staging.next_upload = 0;
        uploads.copies.clear();
        Some(uploads)
    }

    fn encode_frame(
        &mut self,
        scene: &Scene,
        frame_view: &wgpu::TextureView,
        atlas_presentation: &WgpuAtlasPresentationReceipt,
        encoder: &mut wgpu::CommandEncoder,
        load: wgpu::LoadOp<wgpu::Color>,
    ) -> anyhow::Result<()> {
        let resources = &self.resources;
        Self::write_gpu_data(
            resources,
            &mut self.encoder_uploads,
            &resources.clip_buffer,
            0,
            unsafe { Self::instance_bytes(scene.clip_shapes()) },
        )
        .context("failed to upload complete clip arena")?;
        let gamma_params = GammaParams {
            gamma_ratios: self.rendering_params.gamma_ratios,
            grayscale_enhanced_contrast: self.rendering_params.grayscale_enhanced_contrast,
            subpixel_enhanced_contrast: self.rendering_params.subpixel_enhanced_contrast,
            is_bgr: self.is_bgr as u32,
            _pad: 0,
        };
        Self::write_buffer(
            resources,
            &mut self.encoder_uploads,
            &resources.globals_buffer,
            self.gamma_offset,
            bytemuck::bytes_of(&gamma_params),
        )?;
        let mut instance_offset = 0;
        let instance_bindings = self
            .write_instances(scene, &mut instance_offset)
            .context("scene instance data exceeds WGPU buffer limits")?;
        self.prepare_texture_bind_groups(atlas_presentation);
        let depth_scale = quad_depth_scale(scene.quads.len());
        let mut compositing_flags = 0;
        if self.premultiplied_alpha {
            compositing_flags |= COMPOSITING_PREMULTIPLIED_ALPHA;
        }
        if self.color_space == WgpuSceneColorSpace::LinearSrgb {
            compositing_flags |= COMPOSITING_LINEAR_SRGB;
        }
        let globals = GlobalParams {
            viewport_size: [
                self.frame_size.width.0 as f32,
                self.frame_size.height.0 as f32,
            ],
            compositing_flags,
            clip_shape_count: self.clip_shape_count,
            path_paint_base: instance_bindings.path_tiles.paint_base,
            path_curve_base: instance_bindings.path_tiles.curve_base,
            depth_scale,
            quad_count: u32::try_from(scene.quads.len())?,
        };
        Self::write_buffer(
            &self.resources,
            &mut self.encoder_uploads,
            &self.resources.globals_buffer,
            0,
            bytemuck::bytes_of(&globals),
        )?;
        if let Some(uploads) = &self.encoder_uploads {
            uploads.encode_copies(encoder);
        }
        let depth = self
            .resources
            .depth_texture
            .as_ref()
            .context("private depth texture missing")?;
        debug_assert_eq!(instance_bindings.quads.first_instance, 0);
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("main_pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: frame_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load,
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &depth.view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(0.0),
                        store: wgpu::StoreOp::Discard,
                    }),
                    stencil_ops: None,
                }),
                ..Default::default()
            });
            let instance_range = |range: Range<usize>| range.start as u32..range.end as u32;
            self.draw_instances(
                &instance_bindings.quads,
                &self.resources.pipelines.opaque_quads,
                0..globals.quad_count,
                &mut pass,
            );
            let mut quad_cursor = 0;
            let mut path_tile_cursor = 0;
            for batch in scene.batches() {
                let (minimum_depth, maximum_depth) = if matches!(&batch, PrimitiveBatch::Quads(_)) {
                    (0.0, 1.0)
                } else {
                    let depth = quad_depth(quad_cursor, depth_scale);
                    (depth, depth)
                };
                pass.set_viewport(
                    0.0,
                    0.0,
                    globals.viewport_size[0],
                    globals.viewport_size[1],
                    minimum_depth,
                    maximum_depth,
                );
                match batch {
                    PrimitiveBatch::Quads(range) => {
                        quad_cursor += range.len() as u32;
                        self.draw_instances(
                            &instance_bindings.quads,
                            &self.resources.pipelines.quads,
                            instance_range(range),
                            &mut pass,
                        );
                    }
                    PrimitiveBatch::Shadows(range) => self.draw_instances(
                        &instance_bindings.shadows,
                        &self.resources.pipelines.shadows,
                        instance_range(range),
                        &mut pass,
                    ),
                    PrimitiveBatch::Paths(range) => {
                        let start = path_tile_cursor;
                        path_tile_cursor += scene.paths[range]
                            .iter()
                            .map(PaintedPath::tile_count)
                            .sum::<usize>();
                        self.draw_instances(
                            &instance_bindings.path_tiles.tiles,
                            &self.resources.pipelines.path_tiles,
                            instance_range(start..path_tile_cursor),
                            &mut pass,
                        );
                    }
                    PrimitiveBatch::Underlines(range) => self.draw_instances(
                        &instance_bindings.underlines,
                        &self.resources.pipelines.underlines,
                        instance_range(range),
                        &mut pass,
                    ),
                    PrimitiveBatch::MonochromeSprites { texture, range } => self.draw_sprites(
                        &instance_bindings.monochrome_sprites,
                        texture,
                        &self.resources.pipelines.mono_sprites,
                        instance_range(range),
                        &mut pass,
                    )?,
                    PrimitiveBatch::SubpixelSprites { texture, range } => self.draw_sprites(
                        &instance_bindings.subpixel_sprites,
                        texture,
                        &self.resources.pipelines.subpixel_sprites,
                        instance_range(range),
                        &mut pass,
                    )?,
                    PrimitiveBatch::PolychromeSprites { texture, range } => self.draw_sprites(
                        &instance_bindings.polychrome_sprites,
                        texture,
                        &self.resources.pipelines.poly_sprites,
                        instance_range(range),
                        &mut pass,
                    )?,
                }
            }
        }
        Ok(())
    }

    fn write_instances(
        &mut self,
        scene: &Scene,
        instance_offset: &mut u64,
    ) -> anyhow::Result<InstanceBindings> {
        Ok(InstanceBindings {
            quads: self.write_instance_binding(
                "quads_bind_group",
                instance_offset,
                &scene.quads,
            )?,
            shadows: self.write_instance_binding(
                "shadows_bind_group",
                instance_offset,
                &scene.shadows,
            )?,
            path_tiles: self.write_path_tiles(scene, instance_offset)?,
            underlines: self.write_instance_binding(
                "underlines_bind_group",
                instance_offset,
                &scene.underlines,
            )?,
            monochrome_sprites: self.write_instance_binding(
                "monochrome_sprites_bind_group",
                instance_offset,
                &scene.monochrome_sprites,
            )?,
            subpixel_sprites: self.write_instance_binding(
                "subpixel_sprites_bind_group",
                instance_offset,
                &scene.subpixel_sprites,
            )?,
            polychrome_sprites: self.write_instance_binding(
                "polychrome_sprites_bind_group",
                instance_offset,
                &scene.polychrome_sprites,
            )?,
        })
    }

    fn prepare_texture_bind_groups(&mut self, receipt: &WgpuAtlasPresentationReceipt) {
        self.resources
            .atlas_texture_bind_groups
            .retain(|texture, cached| receipt.texture_view(*texture) == Some(&cached.view));
        for (texture, view) in receipt.texture_views() {
            if self
                .resources
                .atlas_texture_bind_groups
                .contains_key(&texture)
            {
                #[cfg(any(test, feature = "test-support"))]
                {
                    self.resources.atlas_bind_group_hits += 1;
                }
                continue;
            }
            let bind_group = self.create_texture_bind_group("atlas_texture_bind_group", view);
            #[cfg(any(test, feature = "test-support"))]
            {
                self.resources.atlas_bind_group_creations += 1;
            }
            self.resources.atlas_texture_bind_groups.insert(
                texture,
                AtlasTextureBindGroup {
                    view: view.clone(),
                    bind_group,
                },
            );
        }
    }

    fn create_texture_bind_group(&self, label: &str, view: &wgpu::TextureView) -> wgpu::BindGroup {
        self.resources
            .device
            .create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some(label),
                layout: &self.resources.bind_group_layouts.texture,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(view),
                }],
            })
    }

    fn draw_instances(
        &self,
        instances: &InstanceBinding,
        pipeline: &wgpu::RenderPipeline,
        range: Range<u32>,
        pass: &mut wgpu::RenderPass<'_>,
    ) {
        if range.is_empty() {
            return;
        }
        let resources = self.resources();
        pass.set_pipeline(pipeline);
        pass.set_bind_group(0, &resources.globals_bind_group, &[]);
        pass.set_bind_group(1, &instances.bind_group, &[]);
        pass.draw(
            0..4,
            instances.first_instance + range.start..instances.first_instance + range.end,
        );
    }

    fn draw_sprites(
        &self,
        instances: &InstanceBinding,
        texture: AtlasTextureInstanceId,
        pipeline: &wgpu::RenderPipeline,
        range: Range<u32>,
        pass: &mut wgpu::RenderPass<'_>,
    ) -> anyhow::Result<()> {
        if range.is_empty() {
            return Ok(());
        }
        let resources = self.resources();
        let bind_group = resources
            .atlas_texture_bind_groups
            .get(&texture)
            .context("prepared scene atlas texture bind group is missing")?;
        pass.set_pipeline(pipeline);
        pass.set_bind_group(0, &resources.globals_bind_group, &[]);
        pass.set_bind_group(1, &instances.bind_group, &[]);
        pass.set_bind_group(2, &bind_group.bind_group, &[]);
        pass.draw(
            0..4,
            instances.first_instance + range.start..instances.first_instance + range.end,
        );
        Ok(())
    }

    unsafe fn instance_bytes<T>(instances: &[T]) -> &[u8] {
        unsafe {
            std::slice::from_raw_parts(
                instances.as_ptr() as *const u8,
                std::mem::size_of_val(instances),
            )
        }
    }

    fn write_path_tiles(
        &mut self,
        scene: &Scene,
        instance_offset: &mut u64,
    ) -> anyhow::Result<PathTileBinding> {
        let mut paints = Vec::with_capacity(scene.paths.len());
        let mut curves = Vec::new();
        let mut tiles = Vec::new();
        for path in &scene.paths {
            path.append_to(&mut paints, &mut curves, &mut tiles);
        }
        u32::try_from(paints.len())?;
        u32::try_from(curves.len())?;
        u32::try_from(tiles.len())?;
        let layout = [
            (
                std::mem::size_of_val(paints.as_slice()) as u64,
                std::mem::size_of::<PathPaint>() as u64,
            ),
            (
                std::mem::size_of_val(curves.as_slice()) as u64,
                std::mem::size_of::<PathCurve>() as u64,
            ),
            (
                std::mem::size_of_val(tiles.as_slice()) as u64,
                std::mem::size_of::<PathTile>() as u64,
            ),
        ];
        let end_offset = |mut offset| -> anyhow::Result<u64> {
            for (size, stride) in layout {
                let (start, allocation, _) = instance_allocation(
                    offset,
                    size.max(stride),
                    stride,
                    self.storage_buffer_alignment,
                    self.uses_webgl_instance_data,
                )?;
                offset = start
                    .checked_add(allocation)
                    .context("path allocation overflow")?;
            }
            Ok(offset)
        };
        let end = end_offset(*instance_offset)?;
        if end > self.instance_buffer_capacity {
            let required = end_offset(0)?;
            self.grow_instance_buffer(required)?;
            *instance_offset = 0;
        }
        let paints = self.write_instance_data(instance_offset, &paints)?;
        let curves = self.write_instance_data(instance_offset, &curves)?;
        let tiles = self.write_instance_data(instance_offset, &tiles)?;
        let data = &self.resources.instance_buffer;
        let mut entries = vec![wgpu::BindGroupEntry {
            binding: 0,
            resource: data.binding(tiles.offset, tiles.size),
        }];
        if !self.uses_webgl_instance_data {
            entries.extend([
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: data.binding(curves.offset, curves.size),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: data.binding(paints.offset, paints.size),
                },
            ]);
        }
        let bind_group = self
            .resources
            .device
            .create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("path_tiles_bind_group"),
                layout: &self.resources.bind_group_layouts.path_tiles,
                entries: &entries,
            });
        if let Some(uploads) = self
            .encoder_uploads
            .as_mut()
            .filter(|uploads| uploads.retains_resources())
        {
            uploads.retained_bindings.push(bind_group.clone());
        }
        Ok(PathTileBinding {
            tiles: InstanceBinding {
                bind_group,
                first_instance: tiles.first_instance,
            },
            paint_base: paints.first_instance,
            curve_base: curves.first_instance,
        })
    }

    fn prepare_clip_buffer(&mut self, clip_shapes: &[GpuClipShape]) -> Option<u32> {
        let clip_shape_count = match u32::try_from(clip_shapes.len()) {
            Ok(count) => count,
            Err(_) => {
                log::error!(
                    "clip arena contains too many shapes for the renderer ABI: {}",
                    clip_shapes.len()
                );
                return None;
            }
        };
        let clip_shape_stride = std::mem::size_of::<GpuClipShape>() as u64;
        let required_size = match u64::from(clip_shape_count).checked_mul(clip_shape_stride) {
            Some(size) => size,
            None => {
                log::error!("clip arena byte size overflowed the renderer ABI");
                return None;
            }
        };

        if required_size > self.max_clip_buffer_size {
            log::error!(
                "clip arena requires {} bytes, exceeding the adapter storage-buffer limit {}",
                required_size,
                self.max_clip_buffer_size
            );
            return None;
        }

        if required_size > self.clip_buffer_capacity && !self.grow_clip_buffer(required_size) {
            return None;
        }

        Some(clip_shape_count)
    }

    fn grow_clip_buffer(&mut self, required_size: u64) -> bool {
        let new_capacity = self
            .clip_buffer_capacity
            .saturating_mul(2)
            .max(required_size)
            .min(self.max_clip_buffer_size);
        if new_capacity < required_size {
            log::error!(
                "clip buffer cannot grow to the required size: {}",
                required_size
            );
            return false;
        }

        log::info!("increased clip buffer size to {}", new_capacity);
        let gamma_offset = self.gamma_offset;
        let uses_webgl_instance_data = self.uses_webgl_instance_data;
        let (clip_buffer, new_capacity) = match GpuData::create(
            &self.resources.device,
            "clip_buffer",
            new_capacity,
            uses_webgl_instance_data,
        ) {
            Ok(created) => created,
            Err(error) => {
                log::error!("Failed to grow clip data: {error:#}");
                return false;
            }
        };
        {
            let resources = self.resources_mut();
            resources.clip_buffer = clip_buffer;
            let globals_bind_group = Self::create_globals_bind_group(
                &resources.device,
                &resources.bind_group_layouts.globals,
                &resources.globals_buffer,
                &resources.clip_buffer,
                &resources.atlas_sampler,
                0,
                gamma_offset,
                new_capacity,
                "globals_bind_group",
            );
            resources.globals_bind_group = globals_bind_group;
        }
        self.clip_buffer_capacity = new_capacity;
        true
    }

    fn grow_instance_buffer(&mut self, required_size: u64) -> anyhow::Result<()> {
        anyhow::ensure!(
            required_size <= self.max_buffer_size,
            "instance allocation requires {required_size} bytes, exceeding maximum {}",
            self.max_buffer_size
        );
        let new_capacity = self
            .instance_buffer_capacity
            .saturating_mul(2)
            .max(required_size)
            .min(self.max_buffer_size);
        anyhow::ensure!(
            new_capacity > self.instance_buffer_capacity,
            "instance buffer cannot grow beyond {} bytes",
            self.instance_buffer_capacity
        );
        log::info!("increased instance buffer size to {}", new_capacity);
        let (data, new_capacity) = GpuData::create(
            &self.resources.device,
            "instance_buffer",
            new_capacity,
            self.uses_webgl_instance_data,
        )?;
        self.resources_mut().instance_buffer = data;
        self.instance_buffer_capacity = new_capacity;
        Ok(())
    }

    fn write_instance_data<Instance>(
        &mut self,
        instance_offset: &mut u64,
        instances: &[Instance],
    ) -> anyhow::Result<InstanceAllocation> {
        let data = unsafe { Self::instance_bytes(instances) };
        let stride = std::mem::size_of::<Instance>() as u64;
        let size = (data.len() as u64).max(stride).max(16);
        let (mut offset, allocation, mut first_instance) = instance_allocation(
            *instance_offset,
            size,
            stride,
            self.storage_buffer_alignment,
            self.uses_webgl_instance_data,
        )?;
        if offset
            .checked_add(allocation)
            .is_none_or(|end| end > self.instance_buffer_capacity)
        {
            self.grow_instance_buffer(allocation)?;
            let fresh = instance_allocation(
                0,
                size,
                stride,
                self.storage_buffer_alignment,
                self.uses_webgl_instance_data,
            )?;
            offset = fresh.0;
            first_instance = fresh.2;
        }
        #[cfg(not(target_family = "wasm"))]
        if self.encoder_uploads.is_none() && !data.is_empty() {
            self.invalidate_native_uploads();
        }
        let resources = &self.resources;
        Self::write_gpu_data(
            resources,
            &mut self.encoder_uploads,
            &resources.instance_buffer,
            offset,
            data,
        )?;
        *instance_offset = offset
            .checked_add(allocation)
            .ok_or_else(|| anyhow::anyhow!("instance allocation offset overflow"))?;
        if let Some(uploads) = self
            .encoder_uploads
            .as_mut()
            .filter(|uploads| uploads.retains_resources())
        {
            uploads
                .retained_data
                .push(resources.instance_buffer.clone());
        }
        Ok(InstanceAllocation {
            offset,
            size,
            first_instance,
        })
    }

    fn write_instance_binding<Instance>(
        &mut self,
        label: &str,
        instance_offset: &mut u64,
        instances: &[Instance],
    ) -> anyhow::Result<InstanceBinding> {
        let allocation = self.write_instance_data(instance_offset, instances)?;
        let resources = &self.resources;
        let bind_group = resources
            .device
            .create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some(label),
                layout: &resources.bind_group_layouts.instances,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: resources
                        .instance_buffer
                        .binding(allocation.offset, allocation.size),
                }],
            });
        if let Some(uploads) = self
            .encoder_uploads
            .as_mut()
            .filter(|uploads| uploads.retains_resources())
        {
            uploads.retained_bindings.push(bind_group.clone());
        }
        Ok(InstanceBinding {
            bind_group,
            first_instance: allocation.first_instance,
        })
    }
}

impl WgpuRenderer {
    /// Mark the surface as unconfigured so rendering is skipped until a new
    /// surface is provided via [`replace_surface`](Self::replace_surface).
    ///
    /// This does **not** drop the renderer — the device, queue, atlas, and
    /// pipelines stay alive.  Use this when the native window is destroyed
    /// (e.g. Android `TerminateWindow`) but you intend to re-create the
    /// surface later without losing cached atlas textures.
    pub fn unconfigure_surface(&mut self) {
        if self.surface_lifecycle.shutdown_active() {
            return;
        }

        self.surface_lifecycle.detach_surface();
        self.last_submission = None;
        // Drop surface-bound resources before the native window becomes invalid.
        drop(self.surface.take());
        if let Some(core) = self.core.as_mut() {
            core.resources.invalidate_depth_texture();
        }
    }

    /// Replace the wgpu surface with a new one (e.g. after Android destroys
    /// and recreates the native window).  Keeps the device, queue, atlas, and
    /// all pipelines intact so cached `AtlasTextureId`s remain valid.
    ///
    /// The `instance` **must** be the same [`wgpu::Instance`] that was used to
    /// create the adapter and device (i.e. from the [`SharedWgpuContext`]). Using a
    /// different instance will cause a "Device does not exist" panic because
    /// the wgpu device is bound to its originating instance.
    #[cfg(not(target_family = "wasm"))]
    pub fn replace_surface<W: HasWindowHandle>(
        &mut self,
        window: &W,
        config: WgpuSurfaceConfig,
        instance: &wgpu::Instance,
    ) -> anyhow::Result<()> {
        anyhow::ensure!(
            !self.surface_lifecycle.shutdown_active(),
            "cannot replace a quiesced presentation surface"
        );
        anyhow::ensure!(
            self.core.is_some(),
            "cannot replace a presentation surface without active GPU resources"
        );

        let window_handle = window
            .window_handle()
            .map_err(|e| anyhow::anyhow!("Failed to get window handle: {e}"))?;

        anyhow::ensure!(
            &self.surface_factory.instance == instance,
            "Replacement surface must use the renderer's wgpu instance"
        );
        let surface_factory = SurfaceFactory::for_raw_window(
            instance.clone(),
            window_handle.as_raw(),
            self.surface_factory.display,
        );
        let surface = surface_factory.create()?;

        let alpha_mode = if config.transparent {
            self.transparent_alpha_mode
        } else {
            self.opaque_alpha_mode
        };

        self.requested_transparency = config.transparent;
        self.surface_config.alpha_mode = alpha_mode;
        if let Some(mode) = config.preferred_present_mode {
            self.surface_config.present_mode = mode;
        }

        let requested = self
            .surface_lifecycle
            .attach_surface(config.size)
            .ok_or_else(|| anyhow::anyhow!("cannot attach a terminal WGPU surface"))?;
        if requested.was_clamped {
            warn!(
                "Requested replacement surface size ({}, {}) exceeds maximum texture dimension {}. \
                 Clamping to ({}, {}). Window content may not fill the entire window.",
                config.size.width.0.max(0),
                config.size.height.0.max(0),
                self.max_texture_size,
                requested.width,
                requested.height
            );
        }
        self.surface_config.width = requested.width;
        self.surface_config.height = requested.height;

        self.surface = Some(surface);
        self.resources_mut().invalidate_depth_texture();

        self.surface_factory = surface_factory;
        self.last_submission = None;

        Ok(())
    }

    pub fn destroy(&mut self) {
        // Release surface-bound GPU resources eagerly so the underlying native
        // window can be destroyed before the renderer itself is dropped.
        drop(self.surface.take());
        self.core.take();
        self.surface_lifecycle.reject();
        self.last_submission = None;
    }

    /// Claims renderer presentation shutdown for one exact ticket without releasing resources.
    ///
    /// The caller must drive a draining ticket to [`WgpuSurfaceShutdownProgress::Quiesced`]
    /// before retiring the native surface owner.
    #[doc(hidden)]
    pub fn begin_surface_shutdown(
        &mut self,
        shutdown: &WindowPresentationShutdownTicket,
    ) -> WgpuSurfaceShutdownProgress {
        self.surface_lifecycle.begin_shutdown(shutdown)
    }

    #[cfg(not(target_family = "wasm"))]
    /// Drains submitted native GPU work, then releases surface-bound WGPU resources.
    ///
    /// A timeout or poll error leaves the exact ticket in `Draining` so the platform retirement
    /// operation can retry without releasing its native owner.
    #[doc(hidden)]
    pub fn quiesce_surface(
        &mut self,
        shutdown: &WindowPresentationShutdownTicket,
    ) -> WgpuSurfaceShutdownProgress {
        match self.begin_surface_shutdown(shutdown) {
            WgpuSurfaceShutdownProgress::Rejected | WgpuSurfaceShutdownProgress::Quiesced => {
                return self.shutdown_progress_for(shutdown);
            }
            WgpuSurfaceShutdownProgress::EnteredDraining
            | WgpuSurfaceShutdownProgress::Draining => {}
        }

        let Some(submission) = self.last_submission.clone() else {
            return self.finish_surface_shutdown(shutdown);
        };
        let poll_result = self.core.as_ref().map(|core| {
            core.resources.device.poll(wgpu::PollType::Wait {
                submission_index: Some(submission),
                timeout: Some(Duration::from_millis(50)),
            })
        });
        match poll_result {
            None | Some(Ok(_)) => self.finish_surface_shutdown(shutdown),
            Some(Err(error)) => {
                warn!(
                    "presentation shutdown is waiting for submitted GPU work before surface release: {error:?}"
                );
                WgpuSurfaceShutdownProgress::Draining
            }
        }
    }

    #[cfg(target_family = "wasm")]
    /// Advances an already-claimed WebGPU surface shutdown after the browser reports queue work.
    ///
    /// The first call for a submitted frame only registers a completion callback and returns
    /// `Draining`. A later retry completes release after the callback publishes readiness.
    #[doc(hidden)]
    pub fn advance_web_surface_shutdown(
        &mut self,
        shutdown: &WindowPresentationShutdownTicket,
    ) -> WgpuSurfaceShutdownProgress {
        if self.surface_lifecycle.is_quiesced_for(shutdown) {
            return self.shutdown_progress_for(shutdown);
        }
        if !self.surface_lifecycle.is_draining_for(shutdown) {
            return WgpuSurfaceShutdownProgress::Rejected;
        }
        if self.last_submission.is_none() {
            return self.finish_surface_shutdown(shutdown);
        }

        if let Some(completion) = self.surface_lifecycle.web_completion(shutdown) {
            return if completion.load(Ordering::Acquire) {
                self.finish_surface_shutdown(shutdown)
            } else {
                WgpuSurfaceShutdownProgress::Draining
            };
        }

        let Some(queue) = self.core.as_ref().map(|core| core.resources.queue.clone()) else {
            return self.finish_surface_shutdown(shutdown);
        };
        let completion = Arc::new(AtomicBool::new(false));
        if !self
            .surface_lifecycle
            .install_web_completion(shutdown, completion.clone())
        {
            return WgpuSurfaceShutdownProgress::Rejected;
        }
        queue.on_submitted_work_done(move || {
            completion.store(true, Ordering::Release);
        });
        WgpuSurfaceShutdownProgress::Draining
    }

    fn finish_surface_shutdown(
        &mut self,
        shutdown: &WindowPresentationShutdownTicket,
    ) -> WgpuSurfaceShutdownProgress {
        if !self.surface_lifecycle.is_draining_for(shutdown)
            || shutdown.snapshot().native_terminal()
        {
            return WgpuSurfaceShutdownProgress::Rejected;
        }

        drop(self.surface.take());
        if let Some(core) = self.core.as_mut() {
            core.resources.invalidate_depth_texture();
        }
        if !self
            .surface_lifecycle
            .release_surface_for_shutdown(shutdown)
        {
            return WgpuSurfaceShutdownProgress::Rejected;
        }

        if !shutdown.acknowledge_quiesced() || !self.surface_lifecycle.mark_quiesced(shutdown) {
            return WgpuSurfaceShutdownProgress::Rejected;
        }
        self.last_submission = None;
        self.shutdown_progress_for(shutdown)
    }

    fn shutdown_progress_for(
        &self,
        shutdown: &WindowPresentationShutdownTicket,
    ) -> WgpuSurfaceShutdownProgress {
        if self.is_quiesced_for(shutdown) {
            WgpuSurfaceShutdownProgress::Quiesced
        } else if self.is_draining_for(shutdown) {
            WgpuSurfaceShutdownProgress::Draining
        } else {
            WgpuSurfaceShutdownProgress::Rejected
        }
    }

    /// Returns whether the renderer has accepted any presentation shutdown ticket.
    #[doc(hidden)]
    pub fn presentation_shutdown_active(&self) -> bool {
        self.surface_lifecycle.shutdown_active()
    }

    /// Returns whether this exact ticket is still draining submitted GPU work.
    #[doc(hidden)]
    pub fn is_draining_for(&self, shutdown: &WindowPresentationShutdownTicket) -> bool {
        self.surface_lifecycle.is_draining_for(shutdown)
    }

    /// Returns whether this exact ticket completed GPU draining and released the WGPU surface.
    #[doc(hidden)]
    pub fn is_quiesced_for(&self, shutdown: &WindowPresentationShutdownTicket) -> bool {
        self.surface_lifecycle.is_quiesced_for(shutdown)
            && self.last_submission.is_none()
            && self.surface.is_none()
    }

    /// Returns whether dropping the backend owner can release the surface safely.
    ///
    /// A renderer with no submitted work may be dropped before an exact shutdown ticket is
    /// prepared. Once a submission exists, only a completed exact shutdown may release the
    /// surface; an in-flight or rejected ticket fails closed.
    #[doc(hidden)]
    pub fn surface_owner_release_is_safe(&self) -> bool {
        self.surface_lifecycle
            .owner_release_is_safe(self.last_submission.is_some())
            && (!self.surface_lifecycle.shutdown_active() || self.surface.is_none())
    }

    /// Returns true if the GPU device was lost and recovery is needed.
    pub fn device_lost(&self) -> bool {
        !self.surface_lifecycle.shutdown_active() && self.device_errors.device_lost()
    }

    #[cfg(not(target_family = "wasm"))]
    fn recovery_surface_config(&self) -> WgpuSurfaceConfig {
        WgpuSurfaceConfig {
            size: self.viewport_size(),
            transparent: self.requested_transparency,
            preferred_present_mode: Some(self.surface_config.present_mode),
        }
    }

    /// Recovers from a lost GPU device by recreating the renderer with a new context.
    ///
    /// Call this after detecting `device_lost()` returns true.
    ///
    /// This method coordinates recovery across multiple windows:
    /// - The first window to call this will recreate the shared context
    /// - Subsequent windows will adopt the already-recovered context
    #[cfg(not(target_family = "wasm"))]
    pub fn recover<W>(&mut self, window: &W) -> anyhow::Result<()>
    where
        W: HasWindowHandle + HasDisplayHandle + std::fmt::Debug + Send + Sync + Clone + 'static,
    {
        anyhow::ensure!(
            !self.surface_lifecycle.shutdown_active(),
            "cannot recover a quiesced presentation surface"
        );

        let is_bgr = self.is_bgr;
        let gpu_context = self.context.as_ref().expect("recover requires gpu_context");
        anyhow::ensure!(
            !gpu_context.application_owned,
            "application-owned GPU device was lost"
        );

        // Check if another window already recovered the context
        let needs_new_context = gpu_context
            .current
            .borrow()
            .as_ref()
            .is_none_or(|ctx| ctx.device_lost());

        let window_handle = window
            .window_handle()
            .map_err(|e| anyhow::anyhow!("Failed to get window handle: {e}"))?;

        let config = self.recovery_surface_config();
        let (surface_factory, surface) = if needs_new_context {
            log::warn!("GPU device lost, recreating context...");

            // Drop old resources to release Arc<Device>/Arc<Queue> and GPU resources
            drop(self.surface.take());
            self.core = None;
            *gpu_context.current.borrow_mut() = None;

            let instance = SharedWgpuContext::instance(Box::new(window.clone()));
            let surface_factory =
                SurfaceFactory::for_raw_window(instance.clone(), window_handle.as_raw(), None);
            let surface = surface_factory.create()?;
            let new_context = SharedWgpuContext::new_rejecting_software(
                instance,
                &surface,
                &config,
                self.compositor_gpu,
            )?;
            *gpu_context.current.borrow_mut() = Some(new_context);
            (surface_factory, surface)
        } else {
            let ctx_ref = gpu_context.current.borrow();
            let instance = ctx_ref.as_ref().unwrap().instance.clone();
            let surface_factory =
                SurfaceFactory::for_raw_window(instance, window_handle.as_raw(), None);
            let surface = surface_factory.create()?;
            (surface_factory, surface)
        };

        let gpu_context = gpu_context.clone();
        let ctx_ref = gpu_context.current.borrow();
        let context = ctx_ref.as_ref().expect("context should exist");

        drop(self.surface.take());
        self.core = None;
        self.atlas.handle_device_lost(context);

        *self = Self::new_internal(
            Some(gpu_context.clone()),
            context,
            (surface_factory, surface),
            config,
            needs_new_context,
            self.compositor_gpu,
            self.atlas.clone(),
        )?;
        self.set_subpixel_layout(is_bgr);

        log::info!("GPU recovery complete");
        Ok(())
    }
}

#[cfg(all(not(target_family = "wasm"), any(test, feature = "test-support")))]
struct HeadlessRenderTarget {
    texture: wgpu::Texture,
    view: wgpu::TextureView,
}

#[cfg(all(not(target_family = "wasm"), any(test, feature = "test-support")))]
impl HeadlessRenderTarget {
    fn size(&self) -> Size<DevicePixels> {
        Size {
            width: DevicePixels(self.texture.width() as i32),
            height: DevicePixels(self.texture.height() as i32),
        }
    }

    fn new(
        device: &wgpu::Device,
        size: Size<DevicePixels>,
        format: wgpu::TextureFormat,
    ) -> anyhow::Result<Self> {
        let maximum = device.limits().max_texture_dimension_2d;
        anyhow::ensure!(
            size.width.0 > 0 && size.height.0 > 0,
            "invalid readback size: {size:?}"
        );
        anyhow::ensure!(
            size.width.0 as u32 <= maximum && size.height.0 as u32 <= maximum,
            "readback size {size:?} exceeds the device limit {maximum}"
        );
        anyhow::ensure!(
            matches!(
                format.remove_srgb_suffix(),
                wgpu::TextureFormat::Bgra8Unorm | wgpu::TextureFormat::Rgba8Unorm
            ),
            "unsupported readback format {format:?}"
        );
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("gpui_scene_readback_target"),
            size: wgpu::Extent3d {
                width: size.width.0 as u32,
                height: size.height.0 as u32,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        Ok(Self { texture, view })
    }
}

/// An offscreen renderer using the same device, atlas and scene submission path as WGPU windows.
#[cfg(all(not(target_family = "wasm"), any(test, feature = "test-support")))]
pub struct WgpuHeadlessRenderer {
    context: SharedWgpuContext,
    core: WgpuRendererCore,
    render_target: Option<HeadlessRenderTarget>,
    observed_error_generation: u64,
    last_submission: Option<wgpu::SubmissionIndex>,
}

#[cfg(all(not(target_family = "wasm"), any(test, feature = "test-support")))]
impl WgpuHeadlessRenderer {
    /// Creates an offscreen renderer, sharing the process's headless device until device loss.
    pub fn new() -> anyhow::Result<Self> {
        let (context, target_format) = SharedWgpuContext::new_headless()?;
        // The device may be shared with earlier renderers; observe only errors raised from
        // here on, including any while this renderer builds its pipelines.
        let observed_error_generation = context.errors().current_generation();
        let atlas = Arc::new(WgpuAtlas::from_context(&context));
        let core = WgpuRendererCore::new(
            &context,
            atlas,
            target_format,
            wgpu::CompositeAlphaMode::Opaque,
        )?;
        let mut headless = Self {
            context,
            core,
            render_target: None,
            observed_error_generation,
            last_submission: None,
        };
        headless.check_gpu_errors()?;
        Ok(headless)
    }

    /// Returns cumulative atlas bind-group creations and cache hits for this renderer.
    #[doc(hidden)]
    #[cfg(any(test, feature = "test-support"))]
    pub fn atlas_bind_group_cache_counts(&self) -> (u64, u64) {
        (
            self.core.resources.atlas_bind_group_creations,
            self.core.resources.atlas_bind_group_hits,
        )
    }

    fn ensure_render_target(&mut self, size: Size<DevicePixels>) -> anyhow::Result<()> {
        if self
            .render_target
            .as_ref()
            .is_some_and(|target| target.size() == size)
        {
            return Ok(());
        }

        self.render_target = Some(HeadlessRenderTarget::new(
            &self.context.device,
            size,
            self.core.target_format,
        )?);
        Ok(())
    }

    fn check_gpu_errors(&mut self) -> anyhow::Result<()> {
        anyhow::ensure!(
            !self.context.device_lost(),
            "GPU device was lost during headless rendering"
        );
        if let Some(error) = self
            .context
            .errors()
            .observe_error(&mut self.observed_error_generation)
        {
            self.core.invalidate_native_uploads();
            anyhow::bail!("GPU error during headless rendering: {error}");
        }
        Ok(())
    }

    fn render(&mut self, scene: &Scene, size: Size<DevicePixels>) -> anyhow::Result<()> {
        self.check_gpu_errors()?;
        self.ensure_render_target(size)?;
        let view = self
            .render_target
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Headless render target was not created"))?
            .view
            .clone();
        let submission = self
            .core
            .render_frame(scene, &view, size, false, wgpu::Color::BLACK)
            .inspect_err(|_| {
                self.context.queue.submit(std::iter::empty());
            })?;
        self.last_submission = Some(submission);
        self.check_gpu_errors()
    }

    fn read_image(&mut self) -> anyhow::Result<image::RgbaImage> {
        let image = self
            .render_target
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Headless render target was not created"))?
            .read_image(
                &self.context.device,
                &self.context.queue,
                &mut self.last_submission,
            )?;
        self.check_gpu_errors()?;
        Ok(image)
    }
}

#[cfg(all(not(target_family = "wasm"), any(test, feature = "test-support")))]
impl HeadlessRenderTarget {
    fn read_image(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        last_submission: &mut Option<wgpu::SubmissionIndex>,
    ) -> anyhow::Result<image::RgbaImage> {
        let width = self.texture.width();
        let height = self.texture.height();
        let bytes_per_row = width
            .checked_mul(4)
            .ok_or_else(|| anyhow::anyhow!("Headless render target row size overflowed"))?;
        let padded_bytes_per_row = bytes_per_row
            .checked_next_multiple_of(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT)
            .ok_or_else(|| anyhow::anyhow!("Headless padded row size overflowed"))?;
        let buffer_size = u64::from(padded_bytes_per_row)
            .checked_mul(u64::from(height))
            .ok_or_else(|| anyhow::anyhow!("Headless readback buffer size overflowed"))?;
        anyhow::ensure!(
            buffer_size <= device.limits().max_buffer_size,
            "Headless readback buffer size {buffer_size} exceeds maximum buffer size {}",
            device.limits().max_buffer_size
        );
        let readback_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("headless_readback_buffer"),
            size: buffer_size,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("headless_readback_encoder"),
        });
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &self.texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &readback_buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(padded_bytes_per_row),
                    rows_per_image: Some(height),
                },
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );
        let submission = queue.submit(std::iter::once(encoder.finish()));
        *last_submission = Some(submission.clone());
        let (sender, receiver) = std::sync::mpsc::channel();
        readback_buffer
            .slice(..)
            .map_async(wgpu::MapMode::Read, move |result| {
                if sender.send(result).is_err() {
                    log::error!("Headless readback receiver was dropped before mapping completed");
                }
            });
        device
            .poll(wgpu::PollType::Wait {
                submission_index: Some(submission),
                timeout: Some(Duration::from_secs(30)),
            })
            .map_err(|error| anyhow::anyhow!("Failed to wait for headless rendering: {error}"))?;
        receiver
            .recv_timeout(Duration::from_secs(30))
            .map_err(|error| anyhow::anyhow!("Failed to receive headless mapping result: {error}"))?
            .map_err(|error| anyhow::anyhow!("Failed to map headless readback buffer: {error}"))?;

        let result = (|| {
            let mapped_data = readback_buffer.slice(..).get_mapped_range()?;
            let pixel_capacity = usize::try_from(u64::from(bytes_per_row) * u64::from(height))
                .map_err(|_| anyhow::anyhow!("Headless image size exceeds addressable memory"))?;
            let mut pixels = Vec::with_capacity(pixel_capacity);
            for row in mapped_data
                .chunks_exact(padded_bytes_per_row as usize)
                .take(height as usize)
            {
                pixels.extend_from_slice(&row[..bytes_per_row as usize]);
            }
            drop(mapped_data);
            if self.texture.format().remove_srgb_suffix() == wgpu::TextureFormat::Bgra8Unorm {
                for pixel in pixels.chunks_exact_mut(4) {
                    pixel.swap(0, 2);
                }
            }
            image::RgbaImage::from_raw(width, height, pixels)
                .ok_or_else(|| anyhow::anyhow!("Failed to create image from headless pixel data"))
        })();
        readback_buffer.unmap();
        result
    }
}

#[cfg(all(not(target_family = "wasm"), any(test, feature = "test-support")))]
impl open_gpui::PlatformHeadlessRenderer for WgpuHeadlessRenderer {
    fn render_scene_to_image(
        &mut self,
        scene: &Scene,
        size: Size<DevicePixels>,
    ) -> anyhow::Result<image::RgbaImage> {
        self.render(scene, size)?;
        self.read_image()
    }

    fn render_scene(&mut self, scene: &Scene, size: Size<DevicePixels>) -> anyhow::Result<()> {
        self.render(scene, size)
    }

    fn sprite_atlas(&self) -> Arc<dyn open_gpui::PlatformAtlas> {
        self.core.atlas.clone()
    }
}

struct RenderingParameters {
    gamma_ratios: [f32; 4],
    grayscale_enhanced_contrast: f32,
    subpixel_enhanced_contrast: f32,
}

#[cfg(all(test, not(target_family = "wasm")))]
mod atlas_tests;
#[cfg(test)]
mod webgl_tests;

impl RenderingParameters {
    fn new() -> Self {
        use std::env;

        let gamma = env::var("OPEN_GPUI_FONTS_GAMMA")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(1.8_f32)
            .clamp(1.0, 2.2);
        let gamma_ratios = get_gamma_correction_ratios(gamma);

        let grayscale_enhanced_contrast = env::var("OPEN_GPUI_FONTS_GRAYSCALE_ENHANCED_CONTRAST")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(1.0_f32)
            .max(0.0);

        let subpixel_enhanced_contrast = env::var("OPEN_GPUI_FONTS_SUBPIXEL_ENHANCED_CONTRAST")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(0.5_f32)
            .max(0.0);

        Self {
            gamma_ratios,
            grayscale_enhanced_contrast,
            subpixel_enhanced_contrast,
        }
    }
}

#[cfg(test)]
mod abi_layout_tests {
    use super::*;
    use open_gpui::{
        MonochromeSprite, PaddedBool32, PolychromeSprite, Quad, Shadow, SpriteAnimationId,
        SubpixelSprite, TransformationMatrix, Underline,
    };
    use std::mem::{align_of, offset_of, size_of};

    #[test]
    fn stale_atlas_receipt_requires_a_fresh_framework_scene() {
        let error = AtlasTextureLeaseError::TextureUnavailable {
            texture: AtlasTextureInstanceId {
                texture_id: open_gpui::AtlasTextureId {
                    index: 7,
                    kind: open_gpui::AtlasTextureKind::Polychrome,
                },
                generation: 2,
            },
            epoch: open_gpui::AtlasTextureLeaseEpoch::INITIAL,
        };

        assert_eq!(
            atlas_receipt_failure_outcome(&error),
            PlatformWindowPresentOutcome::RepaintRequired
        );
    }

    #[test]
    fn primitive_and_svg_transforms_have_wgsl_storage_layouts() {
        assert_eq!(align_of::<PrimitiveTransform>(), align_of::<f32>());
        assert_eq!(size_of::<PrimitiveTransform>(), 4 * size_of::<f32>());
        assert_eq!(
            PrimitiveTransform::IDENTITY.components(),
            [1.0, 1.0, 0.0, 0.0]
        );
        assert_eq!(align_of::<TransformationMatrix>(), align_of::<f32>());
        assert_eq!(size_of::<TransformationMatrix>(), 6 * size_of::<f32>());
        assert_eq!(offset_of!(TransformationMatrix, rotation_scale), 0);
        assert_eq!(offset_of!(TransformationMatrix, translation), 16);
        assert_eq!(align_of::<SpriteAnimationId>(), align_of::<u32>());
        assert_eq!(size_of::<SpriteAnimationId>(), size_of::<u32>());
    }

    /// Transparent `u32` flags have no padding, so their complete upload bytes are initialized.
    #[test]
    fn primitive_flags_have_initialized_wgsl_u32_storage() {
        assert_eq!(size_of::<PaddedBool32>(), size_of::<u32>());
        assert_eq!(align_of::<PaddedBool32>(), align_of::<u32>());

        for (flag, expected) in [
            (PaddedBool32::default(), 0u32),
            (PaddedBool32::from(false), 0),
            (PaddedBool32::from(true), 1),
        ] {
            let bytes = unsafe { WgpuRendererCore::instance_bytes(std::slice::from_ref(&flag)) };
            assert_eq!(bytes, expected.to_ne_bytes());
        }
    }

    #[test]
    fn exact_clip_types_have_canonical_renderer_abi() {
        assert_eq!(size_of::<GlobalParams>(), 32);
        assert_eq!(offset_of!(GlobalParams, compositing_flags), 8);
        assert_eq!(offset_of!(GlobalParams, clip_shape_count), 12);

        assert_eq!(align_of::<GpuClipShape>(), align_of::<f32>());
        assert_eq!(size_of::<GpuClipShape>(), 48);
        assert_eq!(offset_of!(GpuClipShape, bounds), 0);
        assert_eq!(offset_of!(GpuClipShape, radii_x), 16);
        assert_eq!(offset_of!(GpuClipShape, radii_y), 32);

        assert_eq!(align_of::<ClipEnvelope>(), align_of::<f32>());
        assert_eq!(size_of::<ClipEnvelope>(), 24);
        assert_eq!(offset_of!(ClipEnvelope, conservative_bounds), 0);
        assert_eq!(offset_of!(ClipEnvelope, first_clip), 16);
        assert_eq!(offset_of!(ClipEnvelope, clip_count), 20);
    }

    #[test]
    fn compositing_uniform_matches_shader_layouts() {
        let expected = [
            (
                Some("viewport_size"),
                offset_of!(GlobalParams, viewport_size),
            ),
            (
                Some("compositing_flags"),
                offset_of!(GlobalParams, compositing_flags),
            ),
            (
                Some("clip_shape_count"),
                offset_of!(GlobalParams, clip_shape_count),
            ),
            (
                Some("path_paint_base"),
                offset_of!(GlobalParams, path_paint_base),
            ),
            (
                Some("path_curve_base"),
                offset_of!(GlobalParams, path_curve_base),
            ),
            (Some("depth_scale"), offset_of!(GlobalParams, depth_scale)),
            (Some("quad_count"), offset_of!(GlobalParams, quad_count)),
        ];
        for source in [STORAGE_BUFFER_SHADERS, WEBGL_SHADERS, SUBPIXEL_SHADERS] {
            let module = naga::front::wgsl::parse_str(source).expect("shader must parse");
            let globals = module
                .global_variables
                .iter()
                .find_map(|(_, global)| {
                    global
                        .binding
                        .as_ref()
                        .is_some_and(|binding| binding.group == 0 && binding.binding == 0)
                        .then_some(global)
                })
                .expect("the globals uniform must be bound at group 0, binding 0");
            assert_eq!(globals.space, naga::AddressSpace::Uniform);
            let naga::TypeInner::Struct { members, span } = &module.types[globals.ty].inner else {
                panic!("the globals uniform must be a parameter struct");
            };
            assert_eq!(*span as usize, size_of::<GlobalParams>());
            let actual = members
                .iter()
                .map(|member| (member.name.as_deref(), member.offset as usize))
                .collect::<Vec<_>>();
            assert_eq!(actual, expected);
        }
    }

    #[test]
    fn transformed_primitives_preserve_wgsl_storage_abi() {
        assert_eq!(offset_of!(Quad, clip), 24);
        assert_eq!(size_of::<Quad>(), 184);

        assert_eq!(offset_of!(Shadow, clip), 40);
        assert_eq!(size_of::<Shadow>(), 136);

        let _: fn(Underline) -> PaddedBool32 = |underline| underline.wavy;
        assert_eq!(offset_of!(Underline, clip), 24);
        assert_eq!(offset_of!(Underline, wavy), 68);
        assert_eq!(size_of::<Underline>(), 88);

        assert_eq!(offset_of!(MonochromeSprite, animation), 4);
        assert_eq!(offset_of!(MonochromeSprite, tile), 8);
        assert_eq!(offset_of!(MonochromeSprite, bounds), 48);
        assert_eq!(offset_of!(MonochromeSprite, clip), 64);
        assert_eq!(offset_of!(MonochromeSprite, color), 88);
        assert_eq!(offset_of!(MonochromeSprite, transformation), 120);
        assert_eq!(size_of::<MonochromeSprite>(), 144);

        assert_eq!(offset_of!(SubpixelSprite, tile), 8);
        assert_eq!(offset_of!(SubpixelSprite, clip), 64);
        assert_eq!(size_of::<SubpixelSprite>(), 120);

        let _: fn(PolychromeSprite) -> PaddedBool32 = |sprite| sprite.grayscale;
        assert_eq!(offset_of!(PolychromeSprite, tile), 8);
        assert_eq!(offset_of!(PolychromeSprite, grayscale), 48);
        assert_eq!(offset_of!(PolychromeSprite, opacity), 52);
        assert_eq!(offset_of!(PolychromeSprite, clip), 72);
        assert_eq!(size_of::<PolychromeSprite>(), 128);
        for array_stride in [
            size_of::<Quad>(),
            size_of::<Shadow>(),
            size_of::<Underline>(),
            size_of::<MonochromeSprite>(),
            size_of::<SubpixelSprite>(),
            size_of::<PolychromeSprite>(),
        ] {
            assert_eq!(array_stride % 8, 0);
        }
    }

    #[test]
    fn path_tiles_preserve_core_abi_and_background_transform() {
        assert_eq!(size_of::<PathCurve>(), 40);
        assert_eq!(offset_of!(PathCurve, p0), 0);
        assert_eq!(offset_of!(PathCurve, p1), 8);
        assert_eq!(offset_of!(PathCurve, a), 16);
        assert_eq!(offset_of!(PathCurve, b), 24);
        assert_eq!(offset_of!(PathCurve, sign), 32);
        assert_eq!(offset_of!(PathCurve, pad), 36);
        assert_eq!(size_of::<PathTile>(), 56);
        assert_eq!(offset_of!(PathTile, paint), 0);
        assert_eq!(offset_of!(PathTile, curve_start), 4);
        assert_eq!(offset_of!(PathTile, curve_count), 8);
        assert_eq!(offset_of!(PathTile, backdrop), 12);
        assert_eq!(offset_of!(PathTile, top_left), 16);
        assert_eq!(offset_of!(PathTile, bottom_right), 24);
        assert_eq!(offset_of!(PathTile, reference), 32);
        assert_eq!(offset_of!(PathTile, curves_top_left), 40);
        assert_eq!(offset_of!(PathTile, curves_bottom_right), 48);
        assert_eq!(size_of::<PathPaint>(), 136);
        assert_eq!(offset_of!(PathPaint, bounds), 0);
        assert_eq!(offset_of!(PathPaint, clip), 16);
        assert_eq!(offset_of!(PathPaint, color), 40);
        assert_eq!(offset_of!(PathPaint, even_odd), 112);
        assert_eq!(offset_of!(PathPaint, pad), 116);
        assert_eq!(offset_of!(PathPaint, transform), 120);
    }
}

#[cfg(all(test, not(target_family = "wasm")))]
mod tests {
    use super::*;
    use open_gpui::{PlatformHeadlessRenderer as _, size};

    #[test]
    fn updates_during_failed_recovery_keep_the_latest_transparency_request() -> anyhow::Result<()> {
        let (context, format) = SharedWgpuContext::new_headless()?;
        let size = size(DevicePixels(64), DevicePixels(48));
        let max_texture_size = context.device.limits().max_texture_dimension_2d;
        let (mut surface_lifecycle, extent) = WindowSurfaceRuntime::new(size, max_texture_size);
        let SurfaceRenderPlan::Configure(ticket) = surface_lifecycle.prepare_render() else {
            panic!("expected an initial surface configuration");
        };
        assert!(surface_lifecycle.accept_configuration(ticket));

        let mut renderer = WgpuRenderer {
            context: Some(GpuContext::default()),
            compositor_gpu: None,
            surface_factory: SurfaceFactory::for_raw_window(
                context.instance.clone(),
                raw_window_handle::WebWindowHandle::new(0).into(),
                None,
            ),
            surface: None,
            core: None,
            surface_config: wgpu::SurfaceConfiguration {
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                format,
                width: extent.width,
                height: extent.height,
                present_mode: wgpu::PresentMode::Fifo,
                desired_maximum_frame_latency: 2,
                alpha_mode: wgpu::CompositeAlphaMode::Inherit,
                view_formats: Vec::new(),
                color_space: wgpu::SurfaceColorSpace::Auto,
            },
            requested_transparency: false,
            is_bgr: false,
            atlas: Arc::new(WgpuAtlas::from_context(&context)),
            transparent_alpha_mode: wgpu::CompositeAlphaMode::PreMultiplied,
            opaque_alpha_mode: wgpu::CompositeAlphaMode::Inherit,
            max_texture_size,
            device_errors: Arc::clone(context.errors()),
            observed_error_generation: context.errors().current_generation(),
            surface_lifecycle,
            last_submission: None,
        };

        renderer.update_transparency(true);
        assert!(renderer.gpu_specs().is_none());
        assert!(renderer.recovery_surface_config().transparent);
        assert_eq!(
            renderer.surface_config.alpha_mode,
            wgpu::CompositeAlphaMode::Inherit
        );
        assert_eq!(
            renderer.surface_lifecycle.prepare_render(),
            SurfaceRenderPlan::Acquire
        );

        renderer.update_transparency(false);
        assert!(!renderer.recovery_surface_config().transparent);
        renderer.set_subpixel_layout(true);
        assert!(renderer.is_bgr);
        assert_eq!(
            renderer.draw(&Scene::default()),
            PlatformWindowPresentOutcome::Rejected
        );
        struct DummyWindow;
        impl HasWindowHandle for DummyWindow {
            fn window_handle(
                &self,
            ) -> Result<raw_window_handle::WindowHandle<'_>, raw_window_handle::HandleError>
            {
                let raw = raw_window_handle::RawWindowHandle::Web(
                    raw_window_handle::WebWindowHandle::new(0),
                );
                // SAFETY: `replace_surface` rejects a missing core before touching the handle.
                Ok(unsafe { raw_window_handle::WindowHandle::borrow_raw(raw) })
            }
        }
        assert!(
            renderer
                .replace_surface(
                    &DummyWindow,
                    renderer.recovery_surface_config(),
                    &context.instance,
                )
                .is_err()
        );
        Ok(())
    }

    #[test]
    fn native_staging_reuses_chunks_and_skips_unchanged_uploads() -> anyhow::Result<()> {
        let mut renderer = WgpuHeadlessRenderer::new()?;
        let core = &mut renderer.core;
        let mut previous_source = None;
        for (submit, first_byte, expected_copies) in
            [(true, 11, 2), (true, 11, 0), (false, 33, 1), (true, 33, 2)]
        {
            core.begin_native_frame_uploads();
            let resources = &core.resources;
            for (offset, byte) in [(0, first_byte), (core.gamma_offset, 22)] {
                WgpuRendererCore::write_buffer(
                    resources,
                    &mut core.encoder_uploads,
                    &resources.globals_buffer,
                    offset,
                    &[byte; 4],
                )?;
            }
            let uploads = core.encoder_uploads.as_ref().unwrap();
            assert_eq!(uploads.copies.len(), expected_copies);
            if let [first, second] = uploads.copies.as_slice() {
                assert_eq!(first.source, second.source);
                assert_ne!(first.source_offset, second.source_offset);
            }
            if let Some(first) = uploads.copies.first() {
                assert_eq!(first.offset, 0);
                if let Some(source) = &previous_source {
                    assert_eq!(&first.source, source);
                }
                previous_source = Some(first.source.clone());
            }

            let mut encoder = resources.device.create_command_encoder(&Default::default());
            uploads.encode_copies(&mut encoder);
            let submission_index = if submit {
                let uploads = core.finish_native_frame_uploads(Some(&encoder));
                let index = core.resources.queue.submit([encoder.finish()]);
                core.native_frame_uploads = uploads;
                Some(index)
            } else {
                drop(encoder);
                core.native_frame_uploads = core.finish_native_frame_uploads(None);
                None
            };
            core.resources.device.poll(wgpu::PollType::Wait {
                submission_index,
                timeout: Some(Duration::from_secs(30)),
            })?;
        }
        renderer.check_gpu_errors()
    }

    #[test]
    fn compositing_flags_preserve_alpha_and_color_space_pixels() -> anyhow::Result<()> {
        let mut renderer = WgpuHeadlessRenderer::new()?;
        let target_size = size(DevicePixels(1), DevicePixels(1));
        renderer.ensure_render_target(target_size)?;
        let view = renderer.render_target.as_ref().unwrap().view.clone();
        let device = renderer.context.device.clone();
        let source = format!(
            "{STORAGE_BUFFER_SHADERS}\n{}",
            r#"
@vertex
fn vs_compositing_flags(@builtin(vertex_index) vertex_id: u32) -> @builtin(position) vec4<f32> {
    let position = vec2<f32>(f32((vertex_id << 1u) & 2u), f32(vertex_id & 2u));
    return vec4<f32>(position * 2.0 - vec2<f32>(1.0), 0.0, 1.0);
}

@fragment
fn fs_compositing_flags() -> @location(0) vec4<f32> {
    return blend_color(vec4<f32>(0.5, 0.25, 0.75, 0.5), 1.0);
}
"#,
        );
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("compositing flags pixel probe"),
            source: wgpu::ShaderSource::Wgsl(std::borrow::Cow::Owned(source)),
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("compositing flags pixel probe"),
            bind_group_layouts: &[Some(&renderer.core.resources.bind_group_layouts.globals)],
            immediate_size: 0,
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("compositing flags pixel probe"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &module,
                entry_point: Some("vs_compositing_flags"),
                buffers: &[],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &module,
                entry_point: Some("fs_compositing_flags"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: renderer.core.target_format,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });

        for (color_space, premultiplied_alpha, expected) in [
            (
                WgpuSceneColorSpace::SrgbEncoded,
                false,
                [128u8, 64, 191, 128],
            ),
            (WgpuSceneColorSpace::SrgbEncoded, true, [64, 32, 96, 128]),
            (WgpuSceneColorSpace::LinearSrgb, false, [55, 13, 133, 128]),
            (WgpuSceneColorSpace::LinearSrgb, true, [27, 6, 67, 128]),
        ] {
            renderer.core.color_space = color_space;
            renderer.core.render_frame(
                &Scene::default(),
                &view,
                target_size,
                premultiplied_alpha,
                wgpu::Color::TRANSPARENT,
            )?;
            let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("compositing flags pixel probe"),
            });
            {
                let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("compositing flags pixel probe"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &view,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                            store: wgpu::StoreOp::Store,
                        },
                        depth_slice: None,
                    })],
                    ..Default::default()
                });
                pass.set_pipeline(&pipeline);
                pass.set_bind_group(0, &renderer.core.resources.globals_bind_group, &[]);
                pass.draw(0..3, 0..1);
            }
            renderer.last_submission = Some(renderer.context.queue.submit([encoder.finish()]));
            let image = renderer.read_image()?;
            let actual = image.get_pixel(0, 0).0;
            assert!(
                actual
                    .into_iter()
                    .zip(expected)
                    .all(|(actual, expected)| actual.abs_diff(expected) <= 1),
                "color_space={color_space:?}, premultiplied_alpha={premultiplied_alpha}: {actual:?} != {expected:?}"
            );
        }
        Ok(())
    }

    #[test]
    fn headless_submission_reuses_target_and_reads_padded_rows() -> anyhow::Result<()> {
        let mut renderer = WgpuHeadlessRenderer::new()?;
        let size = size(DevicePixels(37), DevicePixels(13));
        renderer.render_scene(&Scene::default(), size)?;
        assert!(renderer.last_submission.is_some());
        let target = renderer.render_target.as_ref().unwrap().texture.clone();
        renderer.render_scene(&Scene::default(), size)?;
        assert_eq!(target, renderer.render_target.as_ref().unwrap().texture);

        let image = renderer.read_image()?;
        assert_eq!(image.dimensions(), (37, 13));
        assert!(image.pixels().all(|pixel| pixel.0 == [0, 0, 0, 255]));
        Ok(())
    }

    #[test]
    fn invalid_headless_sizes_preserve_resources_and_admit_valid_retry() -> anyhow::Result<()> {
        let mut renderer = WgpuHeadlessRenderer::new()?;
        for size in [
            size(DevicePixels(0), DevicePixels(10)),
            size(DevicePixels(-1), DevicePixels(10)),
            size(DevicePixels(10), DevicePixels(0)),
            size(DevicePixels(10), DevicePixels(-1)),
            size(DevicePixels(i32::MAX), DevicePixels(1)),
        ] {
            assert!(renderer.render_scene(&Scene::default(), size).is_err());
            assert!(renderer.render_target.is_none());
            assert!(renderer.last_submission.is_none());
        }
        renderer.render_scene(&Scene::default(), size(DevicePixels(7), DevicePixels(3)))?;
        let target = renderer.render_target.as_ref().unwrap().texture.clone();
        assert!(
            renderer
                .render_scene(&Scene::default(), size(DevicePixels(1), DevicePixels(0)))
                .is_err()
        );
        assert_eq!(target, renderer.render_target.as_ref().unwrap().texture);
        renderer.render_scene(&Scene::default(), size(DevicePixels(11), DevicePixels(5)))?;
        assert_ne!(target, renderer.render_target.as_ref().unwrap().texture);
        assert_eq!(renderer.read_image()?.dimensions(), (11, 5));
        Ok(())
    }

    #[test]
    fn headless_reconstruction_reuses_device_and_releases_renderer_atlas() -> anyhow::Result<()> {
        let mut first = WgpuHeadlessRenderer::new()?;
        first.render_scene(&Scene::default(), size(DevicePixels(5), DevicePixels(3)))?;
        let device = Arc::clone(&first.context.device);
        let atlas = Arc::downgrade(&first.core.atlas);
        drop(first);
        assert!(atlas.upgrade().is_none());

        let mut second = WgpuHeadlessRenderer::new()?;
        assert!(Arc::ptr_eq(&device, &second.context.device));
        second.render_scene(&Scene::default(), size(DevicePixels(9), DevicePixels(7)))?;
        assert_eq!(second.read_image()?.dimensions(), (9, 7));
        Ok(())
    }

    fn smoke_device() -> anyhow::Result<(wgpu::Adapter, wgpu::Device)> {
        open_gpui::block_on(async {
            let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
                backends: wgpu::Backends::all(),
                flags: wgpu::InstanceFlags::default(),
                backend_options: wgpu::BackendOptions::default(),
                memory_budget_thresholds: wgpu::MemoryBudgetThresholds::default(),
                display: None,
            });

            let adapter = instance
                .request_adapter(&wgpu::RequestAdapterOptions {
                    power_preference: wgpu::PowerPreference::LowPower,
                    compatible_surface: None,
                    force_fallback_adapter: false,
                    apply_limit_buckets: false,
                })
                .await
                .map_err(|error| anyhow::anyhow!("failed to request adapter: {error}"))?;

            let required_features = if adapter
                .features()
                .contains(wgpu::Features::DUAL_SOURCE_BLENDING)
            {
                wgpu::Features::DUAL_SOURCE_BLENDING
            } else {
                wgpu::Features::empty()
            };

            let (device, _) = adapter
                .request_device(&wgpu::DeviceDescriptor {
                    label: Some("gpui_wgpu_renderer_smoke_device"),
                    required_features,
                    required_limits: wgpu::Limits::downlevel_defaults()
                        .using_resolution(adapter.limits())
                        .using_alignment(adapter.limits()),
                    memory_hints: wgpu::MemoryHints::MemoryUsage,
                    trace: wgpu::Trace::Off,
                    experimental_features: wgpu::ExperimentalFeatures::disabled(),
                    default_queue: wgpu::QueueDescriptor::default(),
                })
                .await
                .map_err(|error| anyhow::anyhow!("failed to request device: {error}"))?;

            Ok((adapter, device))
        })
    }

    fn supported_render_target_format(
        adapter: &wgpu::Adapter,
    ) -> anyhow::Result<wgpu::TextureFormat> {
        let required_usage = wgpu::TextureUsages::RENDER_ATTACHMENT;

        [
            wgpu::TextureFormat::Bgra8Unorm,
            wgpu::TextureFormat::Rgba8Unorm,
        ]
        .into_iter()
        .find(|format| {
            adapter
                .get_texture_format_features(*format)
                .allowed_usages
                .contains(required_usage)
        })
        .ok_or_else(|| {
            let info = adapter.get_info();
            anyhow::anyhow!(
                "adapter {} ({:?}) does not support a smoke-test render target format",
                info.name,
                info.backend
            )
        })
    }

    #[test]
    fn renderer_smoke_creates_core_pipelines() -> anyhow::Result<()> {
        let (adapter, device) = smoke_device()?;
        let surface_format = supported_render_target_format(&adapter)?;
        let layouts = WgpuRendererCore::create_bind_group_layouts(&device, false);
        let dual_source_blending = device
            .features()
            .contains(wgpu::Features::DUAL_SOURCE_BLENDING);

        let _pipelines = WgpuRendererCore::create_pipelines(
            &device,
            &layouts,
            surface_format,
            wgpu::CompositeAlphaMode::Opaque,
            dual_source_blending,
            false,
        );

        Ok(())
    }
}
