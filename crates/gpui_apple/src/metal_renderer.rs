//! Shared Metal rendering for AppKit and UIKit consumers.
//!
//! Window backends supply retained drawables and consume explicit presentation outcomes. The fork's
//! complete-frame preflight retains exact atlas instances and video planes until GPU completion;
//! shutdown remains bound to the window's presentation ticket. Shader records retain the admitted
//! clip arena and primitive transforms rather than upstream content masks.

use crate::metal_atlas::MetalAtlas;
use anyhow::{Context as _, Result};
use block2::RcBlock;
#[cfg(any(test, feature = "test-support", feature = "bench-support"))]
use image::RgbaImage;
use objc2::{Message, rc::Retained, runtime::ProtocolObject};
use objc2_core_foundation::{CFRetained, CGSize};
use objc2_core_graphics::{CGColorSpace, kCGColorSpaceDisplayP3};
use objc2_core_video::{
    CVMetalTexture, CVMetalTextureCache, CVMetalTextureGetTexture, CVPixelBufferGetHeight,
    CVPixelBufferGetHeightOfPlane, CVPixelBufferGetPixelFormatType, CVPixelBufferGetWidth,
    CVPixelBufferGetWidthOfPlane, kCVPixelFormatType_420YpCbCr8BiPlanarFullRange, kCVReturnSuccess,
};
#[cfg(target_os = "macos")]
use objc2_foundation::NSRange;
use objc2_foundation::{NSString, NSUInteger};
#[cfg(all(
    target_os = "macos",
    any(test, feature = "test-support", feature = "bench-support")
))]
use objc2_metal::MTLBlitCommandEncoder;
#[cfg(target_os = "macos")]
use objc2_metal::MTLCopyAllDevices;
use objc2_metal::{
    MTLBlendFactor, MTLBlendOperation, MTLBuffer, MTLClearColor, MTLCommandBuffer,
    MTLCommandEncoder, MTLCommandQueue, MTLCompareFunction, MTLCreateSystemDefaultDevice,
    MTLDepthStencilDescriptor, MTLDepthStencilState, MTLDevice, MTLDrawable, MTLGPUFamily,
    MTLLibrary, MTLLoadAction, MTLPixelFormat, MTLPrimitiveType, MTLRenderCommandEncoder,
    MTLRenderPassDescriptor, MTLRenderPipelineDescriptor, MTLRenderPipelineState,
    MTLResourceOptions, MTLStorageMode, MTLStoreAction, MTLTexture, MTLTextureDescriptor,
    MTLTextureUsage, MTLViewport,
};
#[cfg(any(test, feature = "test-support", feature = "bench-support"))]
use objc2_metal::{MTLCommandBufferStatus, MTLOrigin, MTLRegion, MTLSize};
use objc2_quartz_core::{CAMetalDrawable, CAMetalLayer};
use open_gpui::{
    AtlasTextureLease, Bounds, ClipEnvelope, DevicePixels, PaintSurface, PaintedPath, PathCurve,
    PathPaint, PathTile, PlatformWindowPresentOutcome, PrimitiveBatch, PrimitiveTransform,
    ScaledPixels, Scene, Size, WindowPresentationShutdownTicket, quad_depth, quad_depth_scale,
    size,
};

use parking_lot::Mutex;

use std::{cell::Cell, mem, ops::Range, ptr, sync::Arc};

mod instance_buffer;

use instance_buffer::{InstanceBinding, InstanceBindings, InstanceBufferWriter, write_instances};

#[cfg(not(feature = "runtime_shaders"))]
const SHADERS_METALLIB: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/shaders.metallib"));
#[cfg(feature = "runtime_shaders")]
const SHADERS_SOURCE_FILE: &str = include_str!(concat!(env!("OUT_DIR"), "/stitched_shaders.metal"));

pub type Context = Arc<Mutex<InstanceBufferPool>>;
pub type Renderer = MetalRenderer;

pub fn new_renderer(
    context: self::Context,
    transparent: bool,
    use_display_p3: bool,
) -> Result<Renderer> {
    MetalRenderer::new(context, transparent, use_display_p3)
}

pub struct InstanceBufferPool {
    buffer_size: usize,
    buffers: Vec<InstanceBuffer>,
}

impl Default for InstanceBufferPool {
    fn default() -> Self {
        Self {
            buffer_size: 2 * 1024 * 1024,
            buffers: Vec::new(),
        }
    }
}

struct InstanceBuffer {
    metal_buffer: Retained<ProtocolObject<dyn MTLBuffer>>,
    size: usize,
}

// SAFETY: Each buffer has one CPU writer and returns to the mutex-protected pool only after GPU completion.
unsafe impl Send for InstanceBuffer {}

impl InstanceBufferPool {
    pub(crate) fn reset(&mut self, buffer_size: usize) {
        self.buffer_size = buffer_size;
        self.buffers.clear();
    }

    fn acquire(
        &mut self,
        device: &ProtocolObject<dyn MTLDevice>,
        unified_memory: bool,
    ) -> Result<InstanceBuffer> {
        if let Some(buffer) = self.buffers.pop() {
            return Ok(buffer);
        }
        let options = if unified_memory {
            MTLResourceOptions::StorageModeShared
                    // Buffers are write only which can benefit from the combined cache
                    // https://developer.apple.com/documentation/metal/mtlresourceoptions/cpucachemodewritecombined
                    | MTLResourceOptions::CPUCacheModeWriteCombined
        } else {
            MTLResourceOptions::StorageModeManaged
        };
        let buffer = device
            .newBufferWithLength_options(self.buffer_size, options)
            .with_context(|| format!("allocating {}-byte instance buffer", self.buffer_size))?;
        Ok(InstanceBuffer {
            metal_buffer: buffer,
            size: self.buffer_size,
        })
    }

    fn release(&mut self, buffer: InstanceBuffer) {
        if buffer.size == self.buffer_size {
            self.buffers.push(buffer)
        }
    }
}

enum DisplayLinkMode {
    Detached,
    Attached {
        drawable: Option<Retained<ProtocolObject<dyn CAMetalDrawable>>>,
    },
}

pub struct MetalRenderer {
    device: Retained<ProtocolObject<dyn MTLDevice>>,
    layer: Option<Retained<CAMetalLayer>>,
    is_apple_gpu: bool,
    is_unified_memory: bool,
    presents_with_transaction: bool,
    display_link_mode: DisplayLinkMode,
    drawable_size_rejected: bool,
    /// For headless rendering, tracks whether output should be opaque
    opaque: bool,
    command_queue: Retained<ProtocolObject<dyn MTLCommandQueue>>,
    path_tiles_pipeline_state: Retained<ProtocolObject<dyn MTLRenderPipelineState>>,
    shadows_pipeline_state: Retained<ProtocolObject<dyn MTLRenderPipelineState>>,
    quads_pipeline_state: Retained<ProtocolObject<dyn MTLRenderPipelineState>>,
    opaque_quads_pipeline_state: Retained<ProtocolObject<dyn MTLRenderPipelineState>>,
    underlines_pipeline_state: Retained<ProtocolObject<dyn MTLRenderPipelineState>>,
    monochrome_sprites_pipeline_state: Retained<ProtocolObject<dyn MTLRenderPipelineState>>,
    polychrome_sprites_pipeline_state: Retained<ProtocolObject<dyn MTLRenderPipelineState>>,
    surfaces_pipeline_state: Retained<ProtocolObject<dyn MTLRenderPipelineState>>,
    depth_write_state: Retained<ProtocolObject<dyn MTLDepthStencilState>>,
    depth_test_state: Retained<ProtocolObject<dyn MTLDepthStencilState>>,
    depth_texture: Option<Retained<ProtocolObject<dyn MTLTexture>>>,
    instance_buffer_pool: Arc<Mutex<InstanceBufferPool>>,
    sprite_atlas: Arc<MetalAtlas>,
    core_video_texture_cache: CFRetained<CVMetalTextureCache>,
    last_submitted_command_buffer: Option<Retained<ProtocolObject<dyn MTLCommandBuffer>>>,
    #[cfg(any(test, feature = "test-support", feature = "bench-support"))]
    headless_render_target: Option<Retained<ProtocolObject<dyn MTLTexture>>>,
    presentation_shutdown: Option<WindowPresentationShutdownTicket>,
}

struct PreparedMetalScene<'a> {
    scene: &'a Scene,
    batches: Vec<PreparedMetalBatch>,
    atlas_lease: AtlasTextureLease,
    paths: PreparedMetalPaths,
}

#[derive(Default)]
struct PreparedMetalPaths {
    paints: Vec<PathPaint>,
    curves: Vec<PathCurve>,
    tiles: Vec<PathTile>,
}

struct PathInstanceBindings {
    paints: InstanceBinding,
    curves: InstanceBinding,
    tiles: InstanceBinding,
}

enum PreparedMetalBatch {
    Shadows(Range<usize>),
    Quads(Range<usize>),
    Paths(Range<usize>),
    Underlines(Range<usize>),
    MonochromeSprites {
        texture: Retained<ProtocolObject<dyn MTLTexture>>,
        range: Range<usize>,
    },
    PolychromeSprites {
        texture: Retained<ProtocolObject<dyn MTLTexture>>,
        range: Range<usize>,
    },
    Surfaces {
        first_surface: usize,
        surfaces: Vec<PreparedMetalSurface>,
    },
}

struct PreparedMetalSurface {
    texture_size: Size<DevicePixels>,
    y_texture: Retained<ProtocolObject<dyn MTLTexture>>,
    cb_cr_texture: Retained<ProtocolObject<dyn MTLTexture>>,
    _y_cv_texture: CFRetained<CVMetalTexture>,
    _cb_cr_cv_texture: CFRetained<CVMetalTexture>,
}

impl MetalRenderer {
    /// Creates a new MetalRenderer with a CAMetalLayer for window-based rendering.
    pub fn new(
        instance_buffer_pool: Arc<Mutex<InstanceBufferPool>>,
        transparent: bool,
        use_display_p3: bool,
    ) -> Result<Self> {
        let device = Self::create_device()?;
        let layer = CAMetalLayer::new();
        Self::configure_layer(&layer, &device, transparent, use_display_p3);
        Self::new_internal(device, Some(layer), !transparent, instance_buffer_pool)
    }

    /// Creates a renderer for a CAMetalLayer owned by a platform view, such as
    /// the backing layer UIKit creates for a view whose `layerClass` is
    /// `CAMetalLayer`. The renderer retains the layer.
    pub fn from_layer(
        instance_buffer_pool: Arc<Mutex<InstanceBufferPool>>,
        layer: &CAMetalLayer,
        transparent: bool,
    ) -> Result<Self> {
        let device = Self::create_device()?;
        let layer = layer.retain();
        Self::configure_layer(&layer, &device, transparent, false);
        Self::new_internal(device, Some(layer), !transparent, instance_buffer_pool)
    }

    fn configure_layer(
        layer: &CAMetalLayer,
        device: &ProtocolObject<dyn MTLDevice>,
        transparent: bool,
        use_display_p3: bool,
    ) {
        layer.setDevice(Some(device));
        layer.setPixelFormat(MTLPixelFormat::BGRA8Unorm);
        if use_display_p3 {
            // SAFETY: The constant is extern, from Core Graphics.
            let display_p3_name = unsafe { kCGColorSpaceDisplayP3 };
            if let Some(colorspace) = CGColorSpace::with_name(Some(display_p3_name)) {
                layer.setColorspace(Some(&colorspace));
            } else {
                log::error!("unable to create the Display P3 color space");
            }
        }
        // Support direct-to-display rendering if the window is not transparent
        // https://developer.apple.com/documentation/metal/managing-your-game-window-for-metal-in-macos
        layer.setOpaque(!transparent);
        layer.setMaximumDrawableCount(3);
        // Allow texture reading for visual tests and UI automation screenshots
        // (captures without ScreenCaptureKit). Only debug builds pay the
        // presentation cost, even when `test-support` is compiled in.
        #[cfg(all(feature = "test-support", debug_assertions))]
        layer.setFramebufferOnly(false);
        // A missing drawable must settle as Deferred instead of blocking the AppKit thread
        // indefinitely while the native surface is temporarily unavailable.
        layer.setAllowsNextDrawableTimeout(true);
        layer.setNeedsDisplayOnBoundsChange(true);
        #[cfg(target_os = "macos")]
        layer.setAutoresizingMask(
            objc2_quartz_core::CAAutoresizingMask::LayerWidthSizable
                | objc2_quartz_core::CAAutoresizingMask::LayerHeightSizable,
        );
    }

    /// Creates a new headless MetalRenderer for offscreen rendering without a window.
    ///
    /// This renderer can render scenes to images without requiring a CAMetalLayer,
    /// window, or AppKit. Use `render_scene_to_image()` to render scenes.
    #[cfg(any(test, feature = "test-support", feature = "bench-support"))]
    pub fn new_headless(instance_buffer_pool: Arc<Mutex<InstanceBufferPool>>) -> Result<Self> {
        let device = Self::create_device()?;
        Self::new_internal(device, None, true, instance_buffer_pool)
    }

    #[cfg(target_os = "macos")]
    fn create_device() -> Result<Retained<ProtocolObject<dyn MTLDevice>>> {
        if let Some(device) = MTLCopyAllDevices()
            .iter()
            .min_by_key(|device| (device.isRemovable(), !device.isLowPower()))
        {
            Ok(device)
        } else {
            // For some reason `all()` can return an empty list, see https://github.com/zed-industries/zed/issues/37689
            // In that case, we fall back to the system default device.
            log::error!(
                "Unable to enumerate Metal devices; attempting to use system default device"
            );
            Self::system_default_device()
        }
    }

    #[cfg(target_os = "ios")]
    fn create_device() -> Result<Retained<ProtocolObject<dyn MTLDevice>>> {
        Self::system_default_device()
    }

    fn system_default_device() -> Result<Retained<ProtocolObject<dyn MTLDevice>>> {
        MTLCreateSystemDefaultDevice().context("accessing a compatible Metal device")
    }

    fn new_internal(
        device: Retained<ProtocolObject<dyn MTLDevice>>,
        layer: Option<Retained<CAMetalLayer>>,
        opaque: bool,
        instance_buffer_pool: Arc<Mutex<InstanceBufferPool>>,
    ) -> Result<Self> {
        #[cfg(feature = "runtime_shaders")]
        let library = device
            .newLibraryWithSource_options_error(
                &NSString::from_str(SHADERS_SOURCE_FILE),
                Some(&objc2_metal::MTLCompileOptions::new()),
            )
            .context("building Metal library")?;
        #[cfg(not(feature = "runtime_shaders"))]
        let library = device
            .newLibraryWithData_error(&dispatch2::DispatchData::from_static_bytes(
                SHADERS_METALLIB,
            ))
            .context("building Metal library")?;

        // Shared memory can be used only if CPU and GPU share the same memory space.
        // https://developer.apple.com/documentation/metal/setting-resource-storage-modes
        let is_unified_memory = cfg!(target_os = "ios") || device.hasUnifiedMemory();
        // Apple GPU families support memoryless textures, which can significantly reduce
        // memory usage by keeping render targets in on-chip tile memory instead of
        // allocating backing store in system memory.
        // https://developer.apple.com/documentation/metal/mtlgpufamily
        let is_apple_gpu = device.supportsFamily(MTLGPUFamily::Apple1);

        let path_tiles_pipeline_state = build_pipeline_state(
            &device,
            &library,
            "path_tiles",
            "path_tile_vertex",
            "path_tile_fragment",
            MTLPixelFormat::BGRA8Unorm,
            true,
        )?;
        let shadows_pipeline_state = build_pipeline_state(
            &device,
            &library,
            "shadows",
            "shadow_vertex",
            "shadow_fragment",
            MTLPixelFormat::BGRA8Unorm,
            true,
        )?;
        let quads_pipeline_state = build_pipeline_state(
            &device,
            &library,
            "quads",
            "quad_vertex",
            "quad_fragment",
            MTLPixelFormat::BGRA8Unorm,
            true,
        )?;
        let opaque_quads_pipeline_state = build_pipeline_state(
            &device,
            &library,
            "opaque_quads",
            "opaque_quad_vertex",
            "opaque_quad_fragment",
            MTLPixelFormat::BGRA8Unorm,
            false,
        )?;
        let underlines_pipeline_state = build_pipeline_state(
            &device,
            &library,
            "underlines",
            "underline_vertex",
            "underline_fragment",
            MTLPixelFormat::BGRA8Unorm,
            true,
        )?;
        let monochrome_sprites_pipeline_state = build_pipeline_state(
            &device,
            &library,
            "monochrome_sprites",
            "monochrome_sprite_vertex",
            "monochrome_sprite_fragment",
            MTLPixelFormat::BGRA8Unorm,
            true,
        )?;
        let polychrome_sprites_pipeline_state = build_pipeline_state(
            &device,
            &library,
            "polychrome_sprites",
            "polychrome_sprite_vertex",
            "polychrome_sprite_fragment",
            MTLPixelFormat::BGRA8Unorm,
            true,
        )?;
        let surfaces_pipeline_state = build_pipeline_state(
            &device,
            &library,
            "surfaces",
            "surface_vertex",
            "surface_fragment",
            MTLPixelFormat::BGRA8Unorm,
            true,
        )?;

        let depth_write_state = build_depth_stencil_state(&device, true)?;
        let depth_test_state = build_depth_stencil_state(&device, false)?;

        let command_queue = device.newCommandQueue().context("creating command queue")?;
        #[expect(
            clippy::arc_with_non_send_sync,
            reason = "PlatformWindow::sprite_atlas requires Arc, not Send or Sync"
        )]
        let sprite_atlas = Arc::new(MetalAtlas::new(
            device.clone(),
            cfg!(target_os = "ios") || is_apple_gpu,
        ));
        let mut cache = ptr::null_mut();
        // SAFETY: The output pointer is writable and both optional attribute dictionaries are absent.
        let status = unsafe {
            CVMetalTextureCache::create(None, None, &device, None, ptr::NonNull::from(&mut cache))
        };
        anyhow::ensure!(
            status == kCVReturnSuccess,
            "creating CoreVideo Metal texture cache: status {status}"
        );
        let cache = ptr::NonNull::new(cache).context("retrieving CoreVideo Metal texture cache")?;
        // SAFETY: The successful create call transfers ownership of this non-null cache.
        let core_video_texture_cache = unsafe { CFRetained::from_raw(cache) };

        Ok(Self {
            device,
            layer,
            presents_with_transaction: false,
            drawable_size_rejected: false,
            display_link_mode: DisplayLinkMode::Detached,
            is_apple_gpu,
            is_unified_memory,
            opaque,
            command_queue,
            path_tiles_pipeline_state,
            shadows_pipeline_state,
            quads_pipeline_state,
            opaque_quads_pipeline_state,
            underlines_pipeline_state,
            monochrome_sprites_pipeline_state,
            polychrome_sprites_pipeline_state,
            surfaces_pipeline_state,
            depth_write_state,
            depth_test_state,
            depth_texture: None,
            instance_buffer_pool,
            sprite_atlas,
            core_video_texture_cache,
            last_submitted_command_buffer: None,
            #[cfg(any(test, feature = "test-support", feature = "bench-support"))]
            headless_render_target: None,
            presentation_shutdown: None,
        })
    }

    pub fn layer(&self) -> Option<&CAMetalLayer> {
        if self.presentation_shutdown.is_some() {
            None
        } else {
            self.layer.as_deref()
        }
    }

    pub fn layer_ptr(&self) -> *mut CAMetalLayer {
        self.layer()
            .map(|layer| ptr::from_ref(layer).cast_mut())
            .unwrap_or(ptr::null_mut())
    }

    pub fn sprite_atlas(&self) -> &Arc<MetalAtlas> {
        &self.sprite_atlas
    }

    /// Replaces the current link update's drawable, returning any unused drawable.
    pub fn set_frame_drawable(
        &mut self,
        drawable: Option<Retained<ProtocolObject<dyn CAMetalDrawable>>>,
    ) -> Option<Retained<ProtocolObject<dyn CAMetalDrawable>>> {
        match &mut self.display_link_mode {
            DisplayLinkMode::Attached { drawable: current } => mem::replace(current, drawable),
            DisplayLinkMode::Detached => {
                debug_assert!(
                    drawable.is_none(),
                    "received a Metal drawable while detached"
                );
                None
            }
        }
    }

    /// Allows nextDrawable only after the window has invalidated its Metal link.
    pub fn set_display_link_attached(&mut self, attached: bool) {
        self.display_link_mode = if attached && self.presentation_shutdown.is_none() {
            DisplayLinkMode::Attached { drawable: None }
        } else {
            DisplayLinkMode::Detached
        };
    }

    /// Returns whether a Metal display link owns drawable delivery.
    pub fn is_display_link_attached(&self) -> bool {
        matches!(self.display_link_mode, DisplayLinkMode::Attached { .. })
    }

    /// Returns the current transaction presentation mode without changing the layer.
    pub fn presents_with_transaction(&self) -> bool {
        self.presents_with_transaction
    }

    pub fn set_presents_with_transaction(&mut self, presents_with_transaction: bool) {
        if self.presentation_shutdown.is_some() {
            return;
        }
        self.presents_with_transaction = presents_with_transaction;
        if let Some(layer) = &self.layer {
            layer.setPresentsWithTransaction(presents_with_transaction);
        }
    }

    pub fn update_drawable_size(&mut self, size: Size<DevicePixels>) {
        if self.presentation_shutdown.is_some() {
            return;
        }
        if size.width.0 > 0
            && size.height.0 > 0
            && let Err(error) = self.validate_render_target_size(size)
        {
            self.drawable_size_rejected = true;
            log::error!("failed to resize Metal drawable: {error:#}");
            return;
        }
        self.drawable_size_rejected = false;
        if let Some(layer) = &self.layer {
            layer.setDrawableSize(CGSize::new(size.width.0 as f64, size.height.0 as f64));
        }
        if let Err(error) = self.update_depth_texture(size) {
            log::error!("failed to resize depth texture: {error:#}");
        }
    }

    fn validate_render_target_size(&self, size: Size<DevicePixels>) -> Result<()> {
        anyhow::ensure!(
            size.width.0 > 0 && size.height.0 > 0,
            "Invalid Metal render target size: {size:?}"
        );
        let max_dimension = self.max_texture_dimension_2d();
        anyhow::ensure!(
            size.width.0 as NSUInteger <= max_dimension
                && size.height.0 as NSUInteger <= max_dimension,
            "Metal render target size {size:?} exceeds maximum texture dimension {max_dimension}"
        );
        Ok(())
    }

    fn max_texture_dimension_2d(&self) -> NSUInteger {
        if self.device.supportsFamily(MTLGPUFamily::Apple10) {
            32_768
        } else {
            16_384
        }
    }

    fn update_depth_texture(&mut self, size: Size<DevicePixels>) -> Result<()> {
        // We are uncertain when this happens, but sometimes size can be 0 here. Most likely before
        // the layout pass on window creation. Zero-sized texture creation causes SIGABRT.
        // https://github.com/zed-industries/zed/issues/36229
        if size.width.0 <= 0 || size.height.0 <= 0 {
            self.depth_texture = None;
            return Ok(());
        }
        self.validate_render_target_size(size)?;

        if self.depth_texture.as_ref().is_some_and(|texture| {
            texture.width() == size.width.0 as NSUInteger
                && texture.height() == size.height.0 as NSUInteger
        }) {
            return Ok(());
        }

        let texture_descriptor = MTLTextureDescriptor::new();
        // SAFETY: The dimensions have been checked against this device's texture limit.
        unsafe {
            texture_descriptor.setWidth(size.width.0 as NSUInteger);
            texture_descriptor.setHeight(size.height.0 as NSUInteger);
        }
        texture_descriptor.setPixelFormat(MTLPixelFormat::Depth32Float);
        texture_descriptor.setStorageMode(if self.is_apple_gpu {
            MTLStorageMode::Memoryless
        } else {
            MTLStorageMode::Private
        });
        texture_descriptor.setUsage(MTLTextureUsage::RenderTarget);
        let depth_texture = self
            .device
            .newTextureWithDescriptor(&texture_descriptor)
            .context("allocating depth texture")?;
        self.depth_texture = Some(depth_texture);
        Ok(())
    }

    pub fn update_transparency(&mut self, transparent: bool) {
        if self.presentation_shutdown.is_some() {
            return;
        }
        self.opaque = !transparent;
        if let Some(layer) = &self.layer {
            layer.setOpaque(!transparent);
        }
    }

    fn retire_surface(&mut self) {
        self.display_link_mode = DisplayLinkMode::Detached;
        if let Some(command_buffer) = self.last_submitted_command_buffer.take() {
            command_buffer.waitUntilCompleted();
        }
        self.depth_texture = None;
        #[cfg(any(test, feature = "test-support", feature = "bench-support"))]
        {
            self.headless_render_target = None;
        }
        self.layer = None;
    }

    /// Stops all new presentation work for an exact shutdown ticket without issuing AppKit calls.
    /// The caller can then safely detach the native view layer outside its window-state mutex.
    pub fn reserve_surface_quiescence(
        &mut self,
        shutdown: &WindowPresentationShutdownTicket,
    ) -> bool {
        if let Some(current) = &self.presentation_shutdown {
            return current.same_authority(shutdown);
        }

        if shutdown.snapshot().native_terminal() {
            return false;
        }
        self.presentation_shutdown = Some(shutdown.clone());
        true
    }

    /// Releases all Metal work and surface-bound resources after native-layer detachment.
    pub fn finish_surface_quiescence(
        &mut self,
        shutdown: &WindowPresentationShutdownTicket,
    ) -> bool {
        if !self
            .presentation_shutdown
            .as_ref()
            .is_some_and(|current| current.same_authority(shutdown))
        {
            return false;
        }
        if self.is_quiesced_for(shutdown) {
            return true;
        }

        self.retire_surface();
        shutdown.acknowledge_quiesced()
    }

    pub fn is_quiesced_for(&self, shutdown: &WindowPresentationShutdownTicket) -> bool {
        self.presentation_shutdown
            .as_ref()
            .is_some_and(|current| current.same_authority(shutdown))
            && shutdown.snapshot().quiesced()
            && self.last_submitted_command_buffer.is_none()
            && self.depth_texture.is_none()
            && self.layer.is_none()
    }

    pub fn destroy(&mut self) {
        self.retire_surface();
    }

    fn prepare_scene<'a>(&self, scene: &'a Scene) -> Result<PreparedMetalScene<'a>> {
        let mut paths = PreparedMetalPaths::default();
        for path in &scene.paths {
            path.append_to(&mut paths.paints, &mut paths.curves, &mut paths.tiles);
        }
        for paint in &paths.paints {
            anyhow::ensure!(
                clip_envelope_range(paint.clip, scene.clip_shapes().len()).is_some()
                    && valid_clip_bounds(paint.clip.conservative_bounds)
                    && paint.bounds.size.width.0 >= 0.0
                    && paint.bounds.size.height.0 >= 0.0
                    && paint.transform.try_project_bounds(paint.bounds).is_ok(),
                "Metal scene has invalid path paint geometry"
            );
        }
        let batches = scene.batches().collect::<Vec<_>>();
        let instances = batches
            .iter()
            .filter_map(|batch| match batch {
                PrimitiveBatch::MonochromeSprites { texture, .. }
                | PrimitiveBatch::PolychromeSprites { texture, .. }
                | PrimitiveBatch::SubpixelSprites { texture, .. } => Some(*texture),
                _ => None,
            })
            .collect::<Vec<_>>();
        let (atlas_lease, textures) = self.sprite_atlas.retain_metal_textures(&instances)?;
        let batches = batches
            .into_iter()
            .map(|batch| {
                Ok(match batch {
                    PrimitiveBatch::Shadows(range) => PreparedMetalBatch::Shadows(range),
                    PrimitiveBatch::Quads(range) => PreparedMetalBatch::Quads(range),
                    PrimitiveBatch::Paths(range) => PreparedMetalBatch::Paths(range),
                    PrimitiveBatch::Underlines(range) => PreparedMetalBatch::Underlines(range),
                    PrimitiveBatch::MonochromeSprites { texture, range } => {
                        PreparedMetalBatch::MonochromeSprites {
                            texture: textures
                                .get(&texture)
                                .context("missing preflight monochrome texture")?
                                .clone(),
                            range,
                        }
                    }
                    PrimitiveBatch::PolychromeSprites { texture, range } => {
                        PreparedMetalBatch::PolychromeSprites {
                            texture: textures
                                .get(&texture)
                                .context("missing preflight polychrome texture")?
                                .clone(),
                            range,
                        }
                    }
                    PrimitiveBatch::Surfaces(range) => PreparedMetalBatch::Surfaces {
                        first_surface: range.start,
                        surfaces: scene.surfaces[range]
                            .iter()
                            .map(|surface| self.prepare_surface(surface))
                            .collect::<Result<Vec<_>>>()?,
                    },
                    PrimitiveBatch::SubpixelSprites { .. } => {
                        anyhow::bail!("Metal does not support subpixel sprite batches")
                    }
                })
            })
            .collect::<Result<Vec<_>>>()?;
        Ok(PreparedMetalScene {
            scene,
            batches,
            atlas_lease,
            paths,
        })
    }

    fn prepare_surface(&self, surface: &PaintSurface) -> Result<PreparedMetalSurface> {
        let image_buffer = &surface.image_buffer;
        anyhow::ensure!(
            CVPixelBufferGetPixelFormatType(image_buffer)
                == kCVPixelFormatType_420YpCbCr8BiPlanarFullRange,
            "unsupported Metal surface pixel format"
        );
        let texture_size = size(
            DevicePixels(i32::try_from(CVPixelBufferGetWidth(image_buffer))?),
            DevicePixels(i32::try_from(CVPixelBufferGetHeight(image_buffer))?),
        );
        let plane_texture = |plane, format| -> Result<CFRetained<CVMetalTexture>> {
            let mut texture = ptr::null_mut();
            // SAFETY: The validated pixel format has two planes matching these formats and dimensions.
            let status = unsafe {
                CVMetalTextureCache::create_texture_from_image(
                    None,
                    &self.core_video_texture_cache,
                    image_buffer,
                    None,
                    format,
                    CVPixelBufferGetWidthOfPlane(image_buffer, plane),
                    CVPixelBufferGetHeightOfPlane(image_buffer, plane),
                    plane,
                    ptr::NonNull::from(&mut texture),
                )
            };
            anyhow::ensure!(
                status == kCVReturnSuccess,
                "creating CoreVideo Metal texture for plane {plane}: status {status}"
            );
            let texture =
                ptr::NonNull::new(texture).context("retrieving CoreVideo Metal texture")?;
            // SAFETY: The successful create call transfers ownership of this non-null texture.
            Ok(unsafe { CFRetained::from_raw(texture) })
        };
        let y_cv_texture = plane_texture(0, MTLPixelFormat::R8Unorm)?;
        let cb_cr_cv_texture = plane_texture(1, MTLPixelFormat::RG8Unorm)?;
        let y_texture = CVMetalTextureGetTexture(&y_cv_texture)
            .context("missing Metal surface luma texture")?;
        let cb_cr_texture = CVMetalTextureGetTexture(&cb_cr_cv_texture)
            .context("missing Metal surface chroma texture")?;
        Ok(PreparedMetalSurface {
            texture_size,
            y_texture,
            cb_cr_texture,
            _y_cv_texture: y_cv_texture,
            _cb_cr_cv_texture: cb_cr_cv_texture,
        })
    }

    pub fn draw(&mut self, scene: &Scene) -> PlatformWindowPresentOutcome {
        if self.presentation_shutdown.is_some() {
            return PlatformWindowPresentOutcome::Deferred;
        }
        if self.drawable_size_rejected {
            return PlatformWindowPresentOutcome::Rejected;
        }
        if let Err(error) = validate_scene_clip_envelopes(scene) {
            log::error!("refusing to draw a scene with invalid clip geometry: {error}");
            return PlatformWindowPresentOutcome::Rejected;
        }
        let layer = match &self.layer {
            Some(l) => l.clone(),
            None => {
                log::error!(
                    "draw() called on headless renderer - use render_scene_to_image() instead"
                );
                return PlatformWindowPresentOutcome::Rejected;
            }
        };
        let prepared = match self.prepare_scene(scene) {
            Ok(prepared) => prepared,
            Err(error) => {
                log::debug!("Metal scene requires repaint before presentation: {error:#}");
                return PlatformWindowPresentOutcome::RepaintRequired;
            }
        };
        let drawable = match &mut self.display_link_mode {
            DisplayLinkMode::Attached { drawable } => {
                let Some(drawable) = drawable.take() else {
                    log::debug!("deferred a draw outside its Metal display-link update");
                    return PlatformWindowPresentOutcome::Deferred;
                };
                drawable
            }
            DisplayLinkMode::Detached => {
                let Some(drawable) = layer.nextDrawable() else {
                    log::debug!(
                        "deferred draw because no drawable is available, drawable size: {:?}",
                        layer.drawableSize()
                    );
                    return PlatformWindowPresentOutcome::Deferred;
                };
                drawable
            }
        };
        let texture = drawable.texture();
        let viewport_size = size(
            DevicePixels(texture.width() as i32),
            DevicePixels(texture.height() as i32),
        );
        let command_buffer = match self.render_frame(prepared, &texture, viewport_size) {
            Ok(command_buffer) => command_buffer,
            Err(error) => {
                log::error!("failed to render: {error:#}");
                return PlatformWindowPresentOutcome::Rejected;
            }
        };
        if self.presents_with_transaction {
            command_buffer.commit();
            command_buffer.waitUntilScheduled();
            drawable.present();
        } else {
            command_buffer.presentDrawable(ProtocolObject::from_ref(&*drawable));
            command_buffer.commit();
        }
        self.last_submitted_command_buffer = Some(command_buffer);
        PlatformWindowPresentOutcome::Submitted
    }

    /// Renders the scene to a texture and returns the pixel data as an RGBA image.
    /// This does not present the frame to screen - useful for visual testing
    /// where we want to capture what would be rendered without displaying it.
    ///
    /// Note: This requires a layer-backed renderer. For headless rendering,
    /// use `render_scene_to_image()` instead.
    #[cfg(any(test, feature = "test-support", feature = "bench-support"))]
    pub fn render_to_image(&mut self, scene: &Scene) -> Result<RgbaImage> {
        anyhow::ensure!(
            !self.drawable_size_rejected,
            "cannot capture a Metal drawable with a rejected size"
        );
        validate_scene_clip_envelopes(scene)?;
        let layer = self
            .layer
            .clone()
            .ok_or_else(|| anyhow::anyhow!("render_to_image requires a layer-backed renderer"))?;
        let viewport_size = layer.drawableSize();
        let viewport_size: Size<DevicePixels> = size(
            (viewport_size.width.ceil() as i32).into(),
            (viewport_size.height.ceil() as i32).into(),
        );
        self.render_scene_to_image(scene, viewport_size)
    }

    /// Renders a scene to an image without requiring a window or CAMetalLayer.
    ///
    /// This is the primary method for headless rendering. It creates an offscreen
    /// texture, renders the scene to it, and returns the pixel data as an RGBA image.
    #[cfg(any(test, feature = "test-support", feature = "bench-support"))]
    pub fn render_scene_to_image(
        &mut self,
        scene: &Scene,
        size: Size<DevicePixels>,
    ) -> Result<RgbaImage> {
        objc2::rc::autoreleasepool(|_| self.render_offscreen(scene, size))
    }

    #[cfg(any(test, feature = "test-support", feature = "bench-support"))]
    fn render_offscreen(&mut self, scene: &Scene, size: Size<DevicePixels>) -> Result<RgbaImage> {
        self.validate_render_target_size(size)?;
        validate_scene_clip_envelopes(scene)?;
        anyhow::ensure!(
            self.presentation_shutdown.is_none(),
            "cannot render after Metal presentation shutdown"
        );
        let prepared = self.prepare_scene(scene)?;

        // Create an offscreen texture as render target
        let texture_descriptor = MTLTextureDescriptor::new();
        // SAFETY: The dimensions have been checked against this device's texture limit.
        unsafe {
            texture_descriptor.setWidth(size.width.0 as NSUInteger);
            texture_descriptor.setHeight(size.height.0 as NSUInteger);
        }
        texture_descriptor.setPixelFormat(MTLPixelFormat::BGRA8Unorm);
        texture_descriptor.setUsage(MTLTextureUsage::RenderTarget | MTLTextureUsage::ShaderRead);
        let uses_shared_storage = cfg!(target_os = "ios") || self.is_apple_gpu;
        texture_descriptor.setStorageMode(if uses_shared_storage {
            MTLStorageMode::Shared
        } else {
            MTLStorageMode::Managed
        });
        let target_texture = self
            .device
            .newTextureWithDescriptor(&texture_descriptor)
            .context("allocating offscreen render target")?;

        let command_buffer = self.render_frame(prepared, &target_texture, size)?;

        // Managed textures require an explicit blit synchronize before the CPU
        // can read back the rendered data. Without this, get_bytes returns
        // stale zeros.
        #[cfg(target_os = "macos")]
        if !uses_shared_storage {
            let blit = command_buffer
                .blitCommandEncoder()
                .context("creating blit command encoder")?;
            blit.synchronizeResource(ProtocolObject::from_ref(&*target_texture));
            blit.endEncoding();
        }

        // Commit and wait for completion
        command_buffer.commit();
        command_buffer.waitUntilCompleted();
        anyhow::ensure!(
            command_buffer.status() == MTLCommandBufferStatus::Completed,
            "Metal offscreen rendering failed: {:?}",
            command_buffer.error()
        );

        // Read pixels from the texture
        let width = size.width.0 as u32;
        let height = size.height.0 as u32;
        let bytes_per_row = (width as usize)
            .checked_mul(4)
            .context("offscreen row size overflowed")?;
        let buffer_size = (height as usize)
            .checked_mul(bytes_per_row)
            .context("offscreen image size overflowed")?;

        let mut pixels = vec![0u8; buffer_size];

        let region = MTLRegion {
            origin: MTLOrigin { x: 0, y: 0, z: 0 },
            size: MTLSize {
                width: width as usize,
                height: height as usize,
                depth: 1,
            },
        };

        // SAFETY: The destination has one complete BGRA row for every row of the validated region.
        unsafe {
            target_texture.getBytes_bytesPerRow_fromRegion_mipmapLevel(
                ptr::NonNull::from(pixels.as_mut_slice()).cast(),
                bytes_per_row,
                region,
                0,
            );
        }

        // Convert BGRA to RGBA (swap B and R channels)
        for chunk in pixels.chunks_exact_mut(4) {
            chunk.swap(0, 2);
        }

        RgbaImage::from_raw(width, height, pixels)
            .context("Failed to create RgbaImage from pixel data")
    }

    /// Renders a scene without reading back the result.
    ///
    /// This mirrors the CPU cost of presenting a frame to a window (scene
    /// encoding, instance buffer writes, command submission) and is used by
    /// headless benchmark rendering, where the produced pixels are never
    /// inspected.
    #[cfg(any(test, feature = "test-support", feature = "bench-support"))]
    pub fn render_scene(&mut self, scene: &Scene, size: Size<DevicePixels>) -> Result<()> {
        self.validate_render_target_size(size)?;
        validate_scene_clip_envelopes(scene)?;
        anyhow::ensure!(
            self.presentation_shutdown.is_none(),
            "cannot render after Metal presentation shutdown"
        );
        if let Some(command_buffer) = &self.last_submitted_command_buffer {
            anyhow::ensure!(
                command_buffer.status() != MTLCommandBufferStatus::Error,
                "Metal headless rendering failed: {:?}",
                command_buffer.error()
            );
        }
        let prepared = self.prepare_scene(scene)?;

        objc2::rc::autoreleasepool(|_| {
            let needs_new_target = self.headless_render_target.as_ref().is_none_or(|texture| {
                texture.width() != size.width.0 as NSUInteger
                    || texture.height() != size.height.0 as NSUInteger
            });
            if needs_new_target {
                let texture_descriptor = MTLTextureDescriptor::new();
                // SAFETY: The dimensions have been checked against this device's texture limit.
                unsafe {
                    texture_descriptor.setWidth(size.width.0 as NSUInteger);
                    texture_descriptor.setHeight(size.height.0 as NSUInteger);
                }
                texture_descriptor.setPixelFormat(MTLPixelFormat::BGRA8Unorm);
                texture_descriptor
                    .setUsage(MTLTextureUsage::RenderTarget | MTLTextureUsage::ShaderRead);
                texture_descriptor.setStorageMode(MTLStorageMode::Private);
                self.headless_render_target = Some(
                    self.device
                        .newTextureWithDescriptor(&texture_descriptor)
                        .context("allocating headless render target")?,
                );
            }
            let target_texture = self
                .headless_render_target
                .clone()
                .context("retrieving headless render target")?;
            let command_buffer = self.render_frame(prepared, &target_texture, size)?;

            // Commit without waiting, mirroring presentation to a real window where
            // the CPU doesn't block on the GPU.
            command_buffer.commit();
            self.last_submitted_command_buffer = Some(command_buffer);
            Ok(())
        })
    }

    fn render_frame(
        &mut self,
        prepared: PreparedMetalScene<'_>,
        target_texture: &ProtocolObject<dyn MTLTexture>,
        size: Size<DevicePixels>,
    ) -> Result<Retained<ProtocolObject<dyn MTLCommandBuffer>>> {
        let mut writer = InstanceBufferWriter::new(
            &self.device,
            &self.instance_buffer_pool,
            self.is_unified_memory,
        )?;
        let bindings = write_instances(prepared.scene, &mut writer)
            .context("writing Metal scene instance data")?;
        let path_bindings = PathInstanceBindings {
            paints: writer.write(&prepared.paths.paints)?,
            curves: writer.write(&prepared.paths.curves)?,
            tiles: writer.write(&prepared.paths.tiles)?,
        };
        let command_buffer = self.draw_primitives_to_texture(
            &prepared,
            &bindings,
            &path_bindings,
            target_texture,
            size,
        )?;
        let instance_buffer_pool = self.instance_buffer_pool.clone();
        let resources = Cell::new(Some((
            writer.finish(),
            prepared.batches,
            prepared.atlas_lease,
            path_bindings,
        )));
        let block = RcBlock::new(
            move |_: ptr::NonNull<ProtocolObject<dyn MTLCommandBuffer>>| {
                if let Some((instance_buffer, batches, lease, path_bindings)) = resources.take() {
                    instance_buffer_pool.lock().release(instance_buffer);
                    drop((batches, lease, path_bindings));
                }
            },
        );
        // SAFETY: Metal copies the block and invokes it after GPU completion; its captures own all frame resources.
        unsafe {
            command_buffer.addCompletedHandler(RcBlock::as_ptr(&block));
        }
        Ok(command_buffer)
    }

    fn draw_primitives_to_texture(
        &mut self,
        prepared: &PreparedMetalScene<'_>,
        bindings: &InstanceBindings,
        path_bindings: &PathInstanceBindings,
        texture: &ProtocolObject<dyn MTLTexture>,
        viewport_size: Size<DevicePixels>,
    ) -> Result<Retained<ProtocolObject<dyn MTLCommandBuffer>>> {
        let scene = prepared.scene;
        let command_buffer = self
            .command_queue
            .commandBuffer()
            .context("creating command buffer")?;
        let alpha = if self.opaque { 1. } else { 0. };
        self.update_depth_texture(viewport_size)?;
        let depth_texture = self
            .depth_texture
            .as_ref()
            .context("missing depth texture")?;
        let command_encoder = new_command_encoder_for_texture(
            &command_buffer,
            texture,
            depth_texture,
            viewport_size,
            MTLClearColor {
                red: 0.,
                green: 0.,
                blue: 0.,
                alpha,
            },
            bindings,
        )?;

        let depth_scale = quad_depth_scale(scene.quads.len());
        unsafe {
            command_encoder.setVertexBytes_length_atIndex(
                ptr::NonNull::from(&viewport_size).cast(),
                mem::size_of_val(&viewport_size),
                GlobalInputIndex::ViewportSize as usize,
            );
            command_encoder.setVertexBytes_length_atIndex(
                ptr::NonNull::from(&depth_scale).cast(),
                mem::size_of_val(&depth_scale),
                GlobalInputIndex::DepthScale as usize,
            );
        }
        self.draw_opaque_quads(scene.quads.len(), bindings, &command_encoder);
        command_encoder.setDepthStencilState(Some(&self.depth_test_state));

        let mut quad_cursor = 0u32;
        let mut path_tile_cursor = 0;

        for batch in &prepared.batches {
            let (znear, zfar) = if matches!(batch, PreparedMetalBatch::Quads(_)) {
                (0.0, 1.0)
            } else {
                let depth = quad_depth(quad_cursor, depth_scale) as f64;
                (depth, depth)
            };
            command_encoder.setViewport(MTLViewport {
                originX: 0.0,
                originY: 0.0,
                width: i32::from(viewport_size.width) as f64,
                height: i32::from(viewport_size.height) as f64,
                znear,
                zfar,
            });
            let result = match batch {
                PreparedMetalBatch::Shadows(range) => {
                    self.draw_shadows(range.clone(), bindings, &command_encoder)
                }
                PreparedMetalBatch::Quads(range) => {
                    quad_cursor += range.len() as u32;
                    self.draw_quads(range.clone(), bindings, &command_encoder)
                }
                PreparedMetalBatch::Paths(range) => {
                    let start = path_tile_cursor;
                    path_tile_cursor += scene.paths[range.clone()]
                        .iter()
                        .map(PaintedPath::tile_count)
                        .sum::<usize>();
                    self.draw_path_tiles(start..path_tile_cursor, path_bindings, &command_encoder);
                    Ok(())
                }
                PreparedMetalBatch::Underlines(range) => {
                    self.draw_underlines(range.clone(), bindings, &command_encoder)
                }
                PreparedMetalBatch::MonochromeSprites { texture, range } => {
                    self.draw_monochrome_sprites(texture, range.clone(), bindings, &command_encoder)
                }
                PreparedMetalBatch::PolychromeSprites { texture, range } => {
                    self.draw_polychrome_sprites(texture, range.clone(), bindings, &command_encoder)
                }
                PreparedMetalBatch::Surfaces {
                    first_surface,
                    surfaces,
                } => {
                    self.draw_surfaces(*first_surface, surfaces, bindings, &command_encoder);
                    Ok(())
                }
            };
            if let Err(error) = result {
                command_encoder.endEncoding();
                return Err(error);
            }
        }
        command_encoder.endEncoding();
        Ok(command_buffer)
    }

    fn draw_opaque_quads(
        &self,
        quad_count: usize,
        bindings: &InstanceBindings,
        command_encoder: &ProtocolObject<dyn MTLRenderCommandEncoder>,
    ) {
        if quad_count == 0 {
            return;
        }
        let quad_count_value = quad_count as u32;
        command_encoder.setRenderPipelineState(&self.opaque_quads_pipeline_state);
        command_encoder.setDepthStencilState(Some(&self.depth_write_state));
        unsafe {
            command_encoder.setVertexBuffer_offset_atIndex(
                Some(&bindings.quads.buffer),
                bindings.quads.offset,
                QuadInputIndex::Quads as usize,
            );
            command_encoder.setVertexBytes_length_atIndex(
                ptr::NonNull::from(&quad_count_value).cast(),
                mem::size_of_val(&quad_count_value),
                QuadInputIndex::QuadCount as usize,
            );
            command_encoder.setFragmentBuffer_offset_atIndex(
                Some(&bindings.quads.buffer),
                bindings.quads.offset,
                QuadInputIndex::Quads as usize,
            );
            command_encoder.drawPrimitives_vertexStart_vertexCount_instanceCount_baseInstance(
                MTLPrimitiveType::TriangleStrip,
                0,
                4,
                quad_count,
                0,
            );
        }
    }

    fn draw_shadows(
        &self,
        shadows: Range<usize>,
        bindings: &InstanceBindings,
        command_encoder: &ProtocolObject<dyn MTLRenderCommandEncoder>,
    ) -> Result<()> {
        if shadows.is_empty() {
            return Ok(());
        }
        command_encoder.setRenderPipelineState(&self.shadows_pipeline_state);
        // SAFETY: The range indexes the uploaded scene shadows; the viewport is copied into its shader slot.
        unsafe {
            command_encoder.setVertexBuffer_offset_atIndex(
                Some(&bindings.shadows.buffer),
                bindings.shadows.offset,
                ShadowInputIndex::Shadows as usize,
            );
            command_encoder.setFragmentBuffer_offset_atIndex(
                Some(&bindings.shadows.buffer),
                bindings.shadows.offset,
                ShadowInputIndex::Shadows as usize,
            );
            command_encoder.drawPrimitives_vertexStart_vertexCount_instanceCount_baseInstance(
                MTLPrimitiveType::TriangleStrip,
                0,
                4,
                shadows.len(),
                shadows.start,
            );
        }
        Ok(())
    }

    fn draw_quads(
        &self,
        quads: Range<usize>,
        bindings: &InstanceBindings,
        command_encoder: &ProtocolObject<dyn MTLRenderCommandEncoder>,
    ) -> Result<()> {
        if quads.is_empty() {
            return Ok(());
        }
        command_encoder.setRenderPipelineState(&self.quads_pipeline_state);
        // SAFETY: The range indexes the uploaded scene quads; the viewport is copied into its shader slot.
        unsafe {
            command_encoder.setVertexBuffer_offset_atIndex(
                Some(&bindings.quads.buffer),
                bindings.quads.offset,
                QuadInputIndex::Quads as usize,
            );
            command_encoder.setFragmentBuffer_offset_atIndex(
                Some(&bindings.quads.buffer),
                bindings.quads.offset,
                QuadInputIndex::Quads as usize,
            );
            command_encoder.drawPrimitives_vertexStart_vertexCount_instanceCount_baseInstance(
                MTLPrimitiveType::TriangleStrip,
                0,
                4,
                quads.len(),
                quads.start,
            );
        }
        Ok(())
    }

    fn draw_path_tiles(
        &self,
        tiles: Range<usize>,
        bindings: &PathInstanceBindings,
        command_encoder: &ProtocolObject<dyn MTLRenderCommandEncoder>,
    ) {
        if tiles.is_empty() {
            return;
        }
        command_encoder.setRenderPipelineState(&self.path_tiles_pipeline_state);
        unsafe {
            command_encoder.setVertexBuffer_offset_atIndex(
                Some(&bindings.tiles.buffer),
                bindings.tiles.offset,
                PathTileInputIndex::Tiles as usize,
            );
            command_encoder.setVertexBuffer_offset_atIndex(
                Some(&bindings.paints.buffer),
                bindings.paints.offset,
                PathTileInputIndex::Paints as usize,
            );
            command_encoder.setFragmentBuffer_offset_atIndex(
                Some(&bindings.tiles.buffer),
                bindings.tiles.offset,
                PathTileInputIndex::Tiles as usize,
            );
            command_encoder.setFragmentBuffer_offset_atIndex(
                Some(&bindings.curves.buffer),
                bindings.curves.offset,
                PathTileInputIndex::Curves as usize,
            );
            command_encoder.setFragmentBuffer_offset_atIndex(
                Some(&bindings.paints.buffer),
                bindings.paints.offset,
                PathTileInputIndex::Paints as usize,
            );
            command_encoder.drawPrimitives_vertexStart_vertexCount_instanceCount_baseInstance(
                MTLPrimitiveType::TriangleStrip,
                0,
                4,
                tiles.len(),
                tiles.start,
            );
        }
    }

    fn draw_underlines(
        &self,
        underlines: Range<usize>,
        bindings: &InstanceBindings,
        command_encoder: &ProtocolObject<dyn MTLRenderCommandEncoder>,
    ) -> Result<()> {
        if underlines.is_empty() {
            return Ok(());
        }
        command_encoder.setRenderPipelineState(&self.underlines_pipeline_state);
        // SAFETY: The range indexes the uploaded underlines; viewport bytes are copied into their shader slot.
        unsafe {
            command_encoder.setVertexBuffer_offset_atIndex(
                Some(&bindings.underlines.buffer),
                bindings.underlines.offset,
                UnderlineInputIndex::Underlines as usize,
            );
            command_encoder.setFragmentBuffer_offset_atIndex(
                Some(&bindings.underlines.buffer),
                bindings.underlines.offset,
                UnderlineInputIndex::Underlines as usize,
            );
            command_encoder.drawPrimitives_vertexStart_vertexCount_instanceCount_baseInstance(
                MTLPrimitiveType::TriangleStrip,
                0,
                4,
                underlines.len(),
                underlines.start,
            );
        }
        Ok(())
    }

    fn draw_monochrome_sprites(
        &self,
        texture: &ProtocolObject<dyn MTLTexture>,
        sprites: Range<usize>,
        bindings: &InstanceBindings,
        command_encoder: &ProtocolObject<dyn MTLRenderCommandEncoder>,
    ) -> Result<()> {
        if sprites.is_empty() {
            return Ok(());
        }

        let texture_size = size(
            DevicePixels(texture.width() as i32),
            DevicePixels(texture.height() as i32),
        );
        command_encoder.setRenderPipelineState(&self.monochrome_sprites_pipeline_state);
        // SAFETY: The range indexes uploaded sprites and the retained preflight texture matches every tile instance.
        unsafe {
            command_encoder.setVertexBuffer_offset_atIndex(
                Some(&bindings.monochrome_sprites.buffer),
                bindings.monochrome_sprites.offset,
                SpriteInputIndex::Sprites as usize,
            );
            command_encoder.setFragmentBuffer_offset_atIndex(
                Some(&bindings.monochrome_sprites.buffer),
                bindings.monochrome_sprites.offset,
                SpriteInputIndex::Sprites as usize,
            );
            command_encoder.setVertexBytes_length_atIndex(
                ptr::NonNull::from(&texture_size).cast(),
                mem::size_of_val(&texture_size),
                SpriteInputIndex::AtlasTextureSize as usize,
            );
            command_encoder
                .setFragmentTexture_atIndex(Some(texture), SpriteInputIndex::AtlasTexture as usize);
            command_encoder.drawPrimitives_vertexStart_vertexCount_instanceCount_baseInstance(
                MTLPrimitiveType::TriangleStrip,
                0,
                4,
                sprites.len(),
                sprites.start,
            );
        }
        Ok(())
    }

    fn draw_polychrome_sprites(
        &self,
        texture: &ProtocolObject<dyn MTLTexture>,
        sprites: Range<usize>,
        bindings: &InstanceBindings,
        command_encoder: &ProtocolObject<dyn MTLRenderCommandEncoder>,
    ) -> Result<()> {
        if sprites.is_empty() {
            return Ok(());
        }

        let texture_size = size(
            DevicePixels(texture.width() as i32),
            DevicePixels(texture.height() as i32),
        );
        command_encoder.setRenderPipelineState(&self.polychrome_sprites_pipeline_state);
        // SAFETY: The range indexes uploaded sprites and the retained preflight texture matches every tile instance.
        unsafe {
            command_encoder.setVertexBuffer_offset_atIndex(
                Some(&bindings.polychrome_sprites.buffer),
                bindings.polychrome_sprites.offset,
                SpriteInputIndex::Sprites as usize,
            );
            command_encoder.setFragmentBuffer_offset_atIndex(
                Some(&bindings.polychrome_sprites.buffer),
                bindings.polychrome_sprites.offset,
                SpriteInputIndex::Sprites as usize,
            );
            command_encoder.setVertexBytes_length_atIndex(
                ptr::NonNull::from(&texture_size).cast(),
                mem::size_of_val(&texture_size),
                SpriteInputIndex::AtlasTextureSize as usize,
            );
            command_encoder
                .setFragmentTexture_atIndex(Some(texture), SpriteInputIndex::AtlasTexture as usize);
            command_encoder.drawPrimitives_vertexStart_vertexCount_instanceCount_baseInstance(
                MTLPrimitiveType::TriangleStrip,
                0,
                4,
                sprites.len(),
                sprites.start,
            );
        }
        Ok(())
    }

    fn draw_surfaces(
        &self,
        first_surface: usize,
        surfaces: &[PreparedMetalSurface],
        bindings: &InstanceBindings,
        command_encoder: &ProtocolObject<dyn MTLRenderCommandEncoder>,
    ) {
        command_encoder.setRenderPipelineState(&self.surfaces_pipeline_state);
        // SAFETY: The unit vertices, uploaded surface array, and viewport match the shader layouts and slots.
        unsafe {
            command_encoder.setVertexBuffer_offset_atIndex(
                Some(&bindings.surfaces.buffer),
                bindings.surfaces.offset,
                SurfaceInputIndex::Surfaces as usize,
            );
            command_encoder.setFragmentBuffer_offset_atIndex(
                Some(&bindings.surfaces.buffer),
                bindings.surfaces.offset,
                SurfaceInputIndex::Surfaces as usize,
            );
        }
        for (index, surface) in surfaces.iter().enumerate() {
            // SAFETY: The instance indexes the uploaded scene array, and preflight retains both matching video planes.
            unsafe {
                command_encoder.setVertexBytes_length_atIndex(
                    ptr::NonNull::from(&surface.texture_size).cast(),
                    mem::size_of_val(&surface.texture_size),
                    SurfaceInputIndex::TextureSize as usize,
                );
                command_encoder.setFragmentTexture_atIndex(
                    Some(&surface.y_texture),
                    SurfaceInputIndex::YTexture as usize,
                );
                command_encoder.setFragmentTexture_atIndex(
                    Some(&surface.cb_cr_texture),
                    SurfaceInputIndex::CbCrTexture as usize,
                );
                command_encoder.drawPrimitives_vertexStart_vertexCount_instanceCount_baseInstance(
                    MTLPrimitiveType::TriangleStrip,
                    0,
                    4,
                    1,
                    first_surface + index,
                );
            }
        }
    }
}

#[cfg(test)]
#[path = "metal_renderer/tests.rs"]
mod atlas_preflight_tests;

fn new_command_encoder_for_texture(
    command_buffer: &ProtocolObject<dyn MTLCommandBuffer>,
    texture: &ProtocolObject<dyn MTLTexture>,
    depth_texture: &ProtocolObject<dyn MTLTexture>,
    viewport_size: Size<DevicePixels>,
    clear_color: MTLClearColor,
    bindings: &InstanceBindings,
) -> Result<Retained<ProtocolObject<dyn MTLRenderCommandEncoder>>> {
    let render_pass_descriptor = MTLRenderPassDescriptor::new();
    // SAFETY: Index zero is within the render pass's color attachment array.
    let color_attachment = unsafe {
        render_pass_descriptor
            .colorAttachments()
            .objectAtIndexedSubscript(0)
    };
    color_attachment.setTexture(Some(texture));
    color_attachment.setStoreAction(MTLStoreAction::Store);
    color_attachment.setLoadAction(MTLLoadAction::Clear);
    color_attachment.setClearColor(clear_color);
    let depth_attachment = render_pass_descriptor.depthAttachment();
    depth_attachment.setTexture(Some(depth_texture));
    depth_attachment.setLoadAction(MTLLoadAction::Clear);
    depth_attachment.setClearDepth(0.0);
    depth_attachment.setStoreAction(MTLStoreAction::DontCare);
    let command_encoder = command_buffer
        .renderCommandEncoderWithDescriptor(&render_pass_descriptor)
        .context("creating render command encoder")?;
    command_encoder.setViewport(MTLViewport {
        originX: 0.0,
        originY: 0.0,
        width: i32::from(viewport_size.width) as f64,
        height: i32::from(viewport_size.height) as f64,
        znear: 0.0,
        zfar: 1.0,
    });
    bind_clip_shapes(&command_encoder, bindings);
    Ok(command_encoder)
}

fn build_pipeline_state(
    device: &ProtocolObject<dyn MTLDevice>,
    library: &ProtocolObject<dyn MTLLibrary>,
    label: &str,
    vertex_fn_name: &str,
    fragment_fn_name: &str,
    pixel_format: MTLPixelFormat,
    blending: bool,
) -> Result<Retained<ProtocolObject<dyn MTLRenderPipelineState>>> {
    let vertex_fn = library
        .newFunctionWithName(&NSString::from_str(vertex_fn_name))
        .with_context(|| format!("locating vertex function {vertex_fn_name}"))?;
    let fragment_fn = library
        .newFunctionWithName(&NSString::from_str(fragment_fn_name))
        .with_context(|| format!("locating fragment function {fragment_fn_name}"))?;

    let descriptor = MTLRenderPipelineDescriptor::new();
    descriptor.setLabel(Some(&NSString::from_str(label)));
    descriptor.setVertexFunction(Some(&vertex_fn));
    descriptor.setFragmentFunction(Some(&fragment_fn));
    // SAFETY: Index zero is within the pipeline's color attachment array.
    let color_attachment = unsafe { descriptor.colorAttachments().objectAtIndexedSubscript(0) };
    color_attachment.setPixelFormat(pixel_format);
    color_attachment.setBlendingEnabled(blending);
    color_attachment.setRgbBlendOperation(MTLBlendOperation::Add);
    color_attachment.setAlphaBlendOperation(MTLBlendOperation::Add);
    color_attachment.setSourceRGBBlendFactor(MTLBlendFactor::SourceAlpha);
    color_attachment.setSourceAlphaBlendFactor(MTLBlendFactor::One);
    color_attachment.setDestinationRGBBlendFactor(MTLBlendFactor::OneMinusSourceAlpha);
    color_attachment.setDestinationAlphaBlendFactor(MTLBlendFactor::OneMinusSourceAlpha);
    descriptor.setDepthAttachmentPixelFormat(MTLPixelFormat::Depth32Float);

    device
        .newRenderPipelineStateWithDescriptor_error(&descriptor)
        .with_context(|| format!("creating {label} render pipeline"))
}

fn build_depth_stencil_state(
    device: &ProtocolObject<dyn MTLDevice>,
    write_depth: bool,
) -> Result<Retained<ProtocolObject<dyn MTLDepthStencilState>>> {
    let descriptor = MTLDepthStencilDescriptor::new();
    descriptor.setDepthCompareFunction(MTLCompareFunction::Greater);
    descriptor.setDepthWriteEnabled(write_depth);
    device
        .newDepthStencilStateWithDescriptor(&descriptor)
        .context("creating depth stencil state")
}

fn validate_scene_clip_envelopes(scene: &Scene) -> Result<()> {
    let arena_len = scene.clip_shapes().len();
    let primitive_clips = scene
        .shadows
        .iter()
        .map(|primitive| ("shadow", primitive.clip))
        .chain(scene.quads.iter().map(|primitive| ("quad", primitive.clip)))
        .chain(
            scene
                .underlines
                .iter()
                .map(|primitive| ("underline", primitive.clip)),
        )
        .chain(
            scene
                .monochrome_sprites
                .iter()
                .map(|primitive| ("monochrome sprite", primitive.clip)),
        )
        .chain(
            scene
                .subpixel_sprites
                .iter()
                .map(|primitive| ("subpixel sprite", primitive.clip)),
        )
        .chain(
            scene
                .polychrome_sprites
                .iter()
                .map(|primitive| ("polychrome sprite", primitive.clip)),
        )
        .chain(
            scene
                .surfaces
                .iter()
                .map(|primitive| ("surface", primitive.clip)),
        );

    for (kind, clip) in primitive_clips {
        anyhow::ensure!(
            clip_envelope_range(clip, arena_len).is_some(),
            "{kind} references an empty or out-of-bounds clip range"
        );
        anyhow::ensure!(
            valid_clip_bounds(clip.conservative_bounds),
            "{kind} has invalid conservative clip bounds"
        );
    }
    Ok(())
}

fn clip_envelope_range(clip: ClipEnvelope, arena_len: usize) -> Option<std::ops::Range<usize>> {
    let start = usize::try_from(clip.first_clip).ok()?;
    let count = usize::try_from(clip.clip_count).ok()?;
    if count == 0 {
        return None;
    }
    let end = start.checked_add(count)?;
    (end <= arena_len).then_some(start..end)
}

fn valid_clip_bounds(bounds: Bounds<ScaledPixels>) -> bool {
    bounds.origin.x.0.is_finite()
        && bounds.origin.y.0.is_finite()
        && bounds.size.width.0.is_finite()
        && bounds.size.height.0.is_finite()
        && bounds.size.width.0 > 0.0
        && bounds.size.height.0 > 0.0
}

fn bind_clip_shapes(
    command_encoder: &ProtocolObject<dyn MTLRenderCommandEncoder>,
    bindings: &InstanceBindings,
) {
    // SAFETY: The binding owns the scene's complete clip arena in the reserved shader slot for both stages.
    unsafe {
        command_encoder.setVertexBuffer_offset_atIndex(
            Some(&bindings.clip_shapes.buffer),
            bindings.clip_shapes.offset,
            GlobalInputIndex::ClipShapes as usize,
        );
        command_encoder.setFragmentBuffer_offset_atIndex(
            Some(&bindings.clip_shapes.buffer),
            bindings.clip_shapes.offset,
            GlobalInputIndex::ClipShapes as usize,
        );
    }
}

#[repr(C)]
enum GlobalInputIndex {
    ViewportSize = 0,
    DepthScale = 1,
    ClipShapes = 8,
}

#[repr(C)]
enum ShadowInputIndex {
    Shadows = 2,
}

#[repr(C)]
enum QuadInputIndex {
    Quads = 2,
    QuadCount = 3,
}

#[repr(C)]
enum UnderlineInputIndex {
    Underlines = 2,
}

#[repr(C)]
enum SpriteInputIndex {
    Sprites = 2,
    AtlasTextureSize = 3,
    AtlasTexture = 4,
}

#[repr(C)]
enum SurfaceInputIndex {
    Surfaces = 2,
    TextureSize = 3,
    YTexture = 4,
    CbCrTexture = 5,
}

#[repr(C)]
enum PathTileInputIndex {
    Tiles = 2,
    Curves = 3,
    Paints = 4,
}

#[derive(Clone, Debug, PartialEq)]
#[repr(C)]
pub struct SurfaceBounds {
    pub bounds: Bounds<ScaledPixels>,
    pub clip: ClipEnvelope,
    pub transform: PrimitiveTransform,
}

#[cfg(test)]
mod abi_layout_tests {
    use super::*;
    use open_gpui::{
        Background, Corners, GpuClipShape, MonochromeSprite, Point, PolychromeSprite, Quad, Shadow,
        SubpixelSprite, Underline, point,
    };
    use std::mem::{align_of, offset_of, size_of};

    #[test]
    fn headless_texture_size_refusal_preserves_resources_and_allows_retry() -> Result<()> {
        use open_gpui::PlatformHeadlessRenderer as _;

        objc2::rc::autoreleasepool(|_| {
            let mut headless = MetalHeadlessRenderer::new()?;
            let max_dimension = i32::try_from(headless.renderer.max_texture_dimension_2d())?;
            let scene = Scene::default();
            for invalid_size in [
                size(DevicePixels(0), DevicePixels(3)),
                size(DevicePixels(-1), DevicePixels(3)),
                size(DevicePixels(7), DevicePixels(0)),
                size(DevicePixels(7), DevicePixels(-1)),
                size(DevicePixels(max_dimension + 1), DevicePixels(1)),
                size(DevicePixels(1), DevicePixels(max_dimension + 1)),
                size(DevicePixels(i32::MAX), DevicePixels(1)),
            ] {
                assert!(headless.render_scene(&scene, invalid_size).is_err());
                assert!(
                    headless
                        .render_scene_to_image(&scene, invalid_size)
                        .is_err()
                );
                assert!(headless.renderer.depth_texture.is_none());
                assert!(headless.renderer.headless_render_target.is_none());
                assert!(headless.renderer.last_submitted_command_buffer.is_none());
            }

            let valid_size = size(DevicePixels(7), DevicePixels(3));
            headless.render_scene(&scene, valid_size)?;
            let target = headless
                .renderer
                .headless_render_target
                .as_ref()
                .unwrap()
                .clone();
            let depth = headless.renderer.depth_texture.as_ref().unwrap().clone();
            let submission = headless
                .renderer
                .last_submitted_command_buffer
                .as_ref()
                .unwrap()
                .clone();
            let oversized = size(DevicePixels(1), DevicePixels(max_dimension + 1));
            assert!(headless.render_scene(&scene, oversized).is_err());
            assert!(headless.render_scene_to_image(&scene, oversized).is_err());
            assert!(ptr::eq(
                &*target,
                &**headless.renderer.headless_render_target.as_ref().unwrap(),
            ));
            assert!(ptr::eq(
                &*depth,
                &**headless.renderer.depth_texture.as_ref().unwrap(),
            ));
            assert!(ptr::eq(
                &*submission,
                &**headless
                    .renderer
                    .last_submitted_command_buffer
                    .as_ref()
                    .unwrap(),
            ));

            let image = headless.render_scene_to_image(&scene, valid_size)?;
            assert_eq!(image.dimensions(), (7, 3));
            assert!(image.pixels().all(|pixel| pixel.0 == [0, 0, 0, 255]));
            headless.renderer.destroy();
            Ok(())
        })
    }

    #[test]
    fn depth_texture_accepts_the_device_limit_and_rebuilds_after_zero_size() -> Result<()> {
        objc2::rc::autoreleasepool(|_| {
            let mut renderer = MetalRenderer::new_headless(Default::default())?;
            let max_dimension = renderer.max_texture_dimension_2d();
            renderer.update_depth_texture(size(
                DevicePixels(i32::try_from(max_dimension)?),
                DevicePixels(1),
            ))?;
            assert_eq!(
                renderer.depth_texture.as_ref().unwrap().width(),
                max_dimension
            );
            assert_eq!(
                renderer.depth_texture.as_ref().unwrap().pixelFormat(),
                MTLPixelFormat::Depth32Float
            );

            renderer.update_depth_texture(size(DevicePixels(0), DevicePixels(1)))?;
            assert!(renderer.depth_texture.is_none());
            renderer.render_scene(&Scene::default(), size(DevicePixels(7), DevicePixels(3)))?;
            assert!(renderer.last_submitted_command_buffer.is_some());
            renderer.destroy();
            Ok(())
        })
    }

    #[test]
    fn native_shutdown_keeps_depth_resources_until_the_exact_ticket_finishes() -> Result<()> {
        objc2::rc::autoreleasepool(|_| {
            let mut renderer = MetalRenderer::new(Default::default(), false, false)?;
            let scene = Scene::default();
            renderer.render_scene(&scene, size(DevicePixels(7), DevicePixels(3)))?;
            renderer.update_drawable_size(size(DevicePixels(0), DevicePixels(1)));
            assert!(renderer.depth_texture.is_none());
            renderer.render_scene(&scene, size(DevicePixels(9), DevicePixels(5)))?;
            let depth = renderer.depth_texture.as_ref().unwrap().clone();
            assert_eq!((depth.width(), depth.height()), (9, 5));
            let submitted = renderer
                .last_submitted_command_buffer
                .as_ref()
                .unwrap()
                .clone();
            let ticket = WindowPresentationShutdownTicket::new(open_gpui::WindowId::from(42), 7);
            let foreign = WindowPresentationShutdownTicket::new(open_gpui::WindowId::from(42), 7);
            assert!(renderer.reserve_surface_quiescence(&ticket));
            assert!(!renderer.reserve_surface_quiescence(&foreign));
            renderer.update_drawable_size(size(DevicePixels(0), DevicePixels(1)));
            renderer.update_drawable_size(size(DevicePixels(11), DevicePixels(6)));
            assert!(ptr::eq(
                &*depth,
                &**renderer.depth_texture.as_ref().unwrap()
            ));
            assert!(!renderer.finish_surface_quiescence(&foreign));
            assert!(renderer.headless_render_target.is_some());
            assert!(renderer.layer.is_some());
            assert!(!renderer.is_quiesced_for(&ticket));
            assert!(
                renderer
                    .render_scene(&scene, size(DevicePixels(9), DevicePixels(5)))
                    .is_err()
            );
            assert!(renderer.finish_surface_quiescence(&ticket));
            assert_eq!(submitted.status(), MTLCommandBufferStatus::Completed);
            assert!(renderer.depth_texture.is_none());
            assert!(renderer.last_submitted_command_buffer.is_none());
            assert!(renderer.headless_render_target.is_none());
            assert!(renderer.layer.is_none());
            assert!(renderer.is_quiesced_for(&ticket));
            assert!(!renderer.is_quiesced_for(&foreign));
            assert!(renderer.finish_surface_quiescence(&ticket));
            assert!(ticket.acknowledge_native_terminal());
            Ok(())
        })
    }

    #[test]
    fn attached_renderer_requires_and_consumes_its_links_drawable() {
        objc2::rc::autoreleasepool(|_| {
            let mut renderer = MetalRenderer::new(Default::default(), false, false).unwrap();
            renderer.update_drawable_size(size(DevicePixels(64), DevicePixels(64)));
            let max_dimension = i32::try_from(renderer.max_texture_dimension_2d()).unwrap();
            renderer.update_drawable_size(size(DevicePixels(max_dimension + 1), DevicePixels(64)));
            let drawable_size = renderer.layer().unwrap().drawableSize();
            assert_eq!(drawable_size.width, 64.0);
            assert_eq!(drawable_size.height, 64.0);
            assert_eq!(
                renderer.draw(&Scene::default()),
                PlatformWindowPresentOutcome::Rejected
            );
            assert!(renderer.render_to_image(&Scene::default()).is_err());
            assert!(renderer.last_submitted_command_buffer.is_none());
            renderer.update_drawable_size(size(DevicePixels(64), DevicePixels(64)));
            let drawable = renderer.layer().unwrap().nextDrawable().unwrap();
            renderer.set_display_link_attached(true);
            assert_eq!(
                renderer.draw(&Scene::default()),
                PlatformWindowPresentOutcome::Deferred
            );
            assert!(renderer.last_submitted_command_buffer.is_none());
            assert!(
                renderer
                    .set_frame_drawable(Some(drawable.clone()))
                    .is_none()
            );
            assert!(renderer.set_frame_drawable(None).is_some());
            assert!(renderer.set_frame_drawable(Some(drawable)).is_none());
            assert_eq!(
                renderer.draw(&Scene::default()),
                PlatformWindowPresentOutcome::Submitted
            );
            assert!(renderer.set_frame_drawable(None).is_none());
            assert_eq!(
                renderer.draw(&Scene::default()),
                PlatformWindowPresentOutcome::Deferred
            );
            renderer.destroy();
        });
    }

    #[test]
    fn attached_window_screenshots_use_offscreen_storage() {
        objc2::rc::autoreleasepool(|_| {
            let mut renderer = MetalRenderer::new(Default::default(), false, false).unwrap();
            renderer.update_drawable_size(size(DevicePixels(64), DevicePixels(64)));
            renderer.set_display_link_attached(true);
            let image = renderer.render_to_image(&Scene::default()).unwrap();
            assert_eq!(image.dimensions(), (64, 64));
            assert!(renderer.last_submitted_command_buffer.is_none());
            assert!(matches!(
                renderer.display_link_mode,
                DisplayLinkMode::Attached { drawable: None }
            ));
            assert_eq!(
                renderer.draw(&Scene::default()),
                PlatformWindowPresentOutcome::Deferred
            );
            renderer.destroy();
        });
    }

    #[test]
    fn invalidated_link_allows_synchronous_presentation_again() {
        use objc2::AnyThread;
        use objc2_quartz_core::CAMetalDisplayLink;

        if objc2::runtime::AnyClass::get(c"CAMetalDisplayLink").is_none() {
            return;
        }
        objc2::rc::autoreleasepool(|_| {
            let mut renderer = MetalRenderer::new(Default::default(), false, false).unwrap();
            renderer.update_drawable_size(size(DevicePixels(64), DevicePixels(64)));
            let link = CAMetalDisplayLink::initWithMetalLayer(
                CAMetalDisplayLink::alloc(),
                renderer.layer().unwrap(),
            );
            renderer.set_display_link_attached(true);
            assert_eq!(
                renderer.draw(&Scene::default()),
                PlatformWindowPresentOutcome::Deferred
            );
            link.invalidate();
            renderer.set_display_link_attached(false);
            assert_eq!(
                renderer.draw(&Scene::default()),
                PlatformWindowPresentOutcome::Submitted
            );
            renderer.destroy();
        });
    }

    #[test]
    fn primitive_transform_has_shader_abi_layout() {
        assert_eq!(align_of::<PrimitiveTransform>(), align_of::<f32>());
        assert_eq!(size_of::<PrimitiveTransform>(), 4 * size_of::<f32>());
        assert_eq!(
            PrimitiveTransform::IDENTITY.components(),
            [1.0, 1.0, 0.0, 0.0]
        );
    }

    #[test]
    fn transformed_primitive_fields_are_terminal_in_shader_abi() {
        assert_eq!(size_of::<open_gpui::PaddedBool32>(), 4);
        assert_eq!(align_of::<open_gpui::PaddedBool32>(), 4);
        let _: fn(Underline) -> open_gpui::PaddedBool32 = |underline| underline.wavy;
        let _: fn(PolychromeSprite) -> open_gpui::PaddedBool32 = |sprite| sprite.grayscale;
        assert_eq!(size_of::<Quad>(), 184);
        assert_eq!(size_of::<Shadow>(), 136);
        assert_eq!(size_of::<Underline>(), 88);
        assert_eq!(offset_of!(Underline, wavy), 68);
        assert_eq!(size_of::<MonochromeSprite>(), 120);
        assert_eq!(offset_of!(MonochromeSprite, tile), 8);
        assert_eq!(size_of::<SubpixelSprite>(), 120);
        assert_eq!(offset_of!(SubpixelSprite, tile), 8);
        assert_eq!(size_of::<PolychromeSprite>(), 128);
        assert_eq!(offset_of!(PolychromeSprite, tile), 8);
        assert_eq!(offset_of!(PolychromeSprite, grayscale), 48);
        assert_eq!(offset_of!(PolychromeSprite, opacity), 52);
    }

    #[test]
    fn clip_types_have_canonical_shader_abi_layouts() {
        assert_eq!(align_of::<Bounds<ScaledPixels>>(), align_of::<f32>());
        assert_eq!(size_of::<Bounds<ScaledPixels>>(), 16);
        assert_eq!(offset_of!(Bounds<ScaledPixels>, origin), 0);
        assert_eq!(offset_of!(Bounds<ScaledPixels>, size), 8);
        assert_eq!(offset_of!(Point<ScaledPixels>, x), 0);
        assert_eq!(offset_of!(Point<ScaledPixels>, y), 4);
        assert_eq!(offset_of!(Size<ScaledPixels>, width), 0);
        assert_eq!(offset_of!(Size<ScaledPixels>, height), 4);

        assert_eq!(align_of::<Corners<ScaledPixels>>(), align_of::<f32>());
        assert_eq!(size_of::<Corners<ScaledPixels>>(), 16);
        assert_eq!(offset_of!(Corners<ScaledPixels>, top_left), 0);
        assert_eq!(offset_of!(Corners<ScaledPixels>, top_right), 4);
        assert_eq!(offset_of!(Corners<ScaledPixels>, bottom_right), 8);
        assert_eq!(offset_of!(Corners<ScaledPixels>, bottom_left), 12);

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
        assert_eq!(GlobalInputIndex::ClipShapes as u64, 8);
    }

    #[test]
    fn global_clip_buffer_slot_does_not_overlap_pipeline_buffers() {
        let clip_slot = GlobalInputIndex::ClipShapes as u64;
        assert!((GlobalInputIndex::ViewportSize as u64) < clip_slot);
        assert!((GlobalInputIndex::DepthScale as u64) < clip_slot);
        assert!((ShadowInputIndex::Shadows as u64) < clip_slot);
        assert!((QuadInputIndex::Quads as u64) < clip_slot);
        assert!((QuadInputIndex::QuadCount as u64) < clip_slot);
        assert!((UnderlineInputIndex::Underlines as u64) < clip_slot);
        assert!((SpriteInputIndex::AtlasTextureSize as u64) < clip_slot);
        assert!((SurfaceInputIndex::TextureSize as u64) < clip_slot);
        assert!((PathTileInputIndex::Tiles as u64) < clip_slot);
        assert!((PathTileInputIndex::Curves as u64) < clip_slot);
        assert!((PathTileInputIndex::Paints as u64) < clip_slot);
    }

    #[test]
    fn invalid_clip_envelope_ranges_fail_closed_before_encoding() {
        let clip = ClipEnvelope {
            conservative_bounds: Bounds::new(
                point(ScaledPixels(0.0), ScaledPixels(0.0)),
                size(ScaledPixels(10.0), ScaledPixels(10.0)),
            ),
            first_clip: 1,
            clip_count: 1,
        };
        assert_eq!(clip_envelope_range(clip, 2), Some(1..2));
        let assert_invalid = |candidate| assert!(clip_envelope_range(candidate, 2).is_none());
        assert_invalid(ClipEnvelope {
            clip_count: 0,
            ..clip
        });
        assert_invalid(ClipEnvelope {
            first_clip: 2,
            ..clip
        });
        assert_invalid(ClipEnvelope {
            clip_count: 2,
            ..clip
        });
        assert!(valid_clip_bounds(clip.conservative_bounds));
        assert!(!valid_clip_bounds(Bounds::default()));
    }

    #[test]
    fn renderer_owned_shader_structs_have_contiguous_c_layouts() {
        assert_eq!(align_of::<PathCurve>(), align_of::<f32>());
        assert_eq!(size_of::<PathCurve>(), 40);
        assert_eq!(offset_of!(PathCurve, p0), 0);
        assert_eq!(offset_of!(PathCurve, p1), 8);
        assert_eq!(offset_of!(PathCurve, a), 16);
        assert_eq!(offset_of!(PathCurve, b), 24);
        assert_eq!(offset_of!(PathCurve, sign), 32);
        assert_eq!(offset_of!(PathCurve, pad), 36);
        assert_eq!(align_of::<PathTile>(), align_of::<f32>());
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
        assert_eq!(align_of::<PathPaint>(), align_of::<f32>());
        assert_eq!(offset_of!(PathPaint, bounds), 0);
        assert_eq!(offset_of!(PathPaint, clip), 16);
        assert_eq!(offset_of!(PathPaint, color), 40);
        assert_eq!(
            offset_of!(PathPaint, even_odd),
            40 + size_of::<Background>()
        );
        assert_eq!(offset_of!(PathPaint, pad), 44 + size_of::<Background>());
        assert_eq!(
            offset_of!(PathPaint, transform),
            48 + size_of::<Background>()
        );
        assert_eq!(size_of::<PathPaint>(), 64 + size_of::<Background>());

        assert_eq!(align_of::<SurfaceBounds>(), align_of::<f32>());
        assert_eq!(offset_of!(SurfaceBounds, bounds), 0);
        assert_eq!(offset_of!(SurfaceBounds, clip), 16);
        assert_eq!(offset_of!(SurfaceBounds, transform), 40);
        assert_eq!(size_of::<SurfaceBounds>(), 56);
    }
}

#[cfg(any(test, feature = "test-support", feature = "bench-support"))]
pub struct MetalHeadlessRenderer {
    renderer: MetalRenderer,
}

#[cfg(any(test, feature = "test-support", feature = "bench-support"))]
impl MetalHeadlessRenderer {
    pub fn new() -> Result<Self> {
        let instance_buffer_pool = Arc::new(Mutex::new(InstanceBufferPool::default()));
        let renderer = MetalRenderer::new_headless(instance_buffer_pool)?;
        Ok(Self { renderer })
    }
}

#[cfg(any(test, feature = "test-support", feature = "bench-support"))]
impl open_gpui::PlatformHeadlessRenderer for MetalHeadlessRenderer {
    fn render_scene_to_image(
        &mut self,
        scene: &Scene,
        size: Size<DevicePixels>,
    ) -> anyhow::Result<image::RgbaImage> {
        self.renderer.render_scene_to_image(scene, size)
    }

    fn render_scene(&mut self, scene: &Scene, size: Size<DevicePixels>) -> Result<()> {
        self.renderer.render_scene(scene, size)
    }

    fn sprite_atlas(&self) -> Arc<dyn open_gpui::PlatformAtlas> {
        self.renderer.sprite_atlas().clone()
    }
}
