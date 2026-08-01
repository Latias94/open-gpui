use anyhow::{Result, anyhow};
use block2::RcBlock;
#[cfg(not(feature = "runtime_shaders"))]
use dispatch2::DispatchData;
use objc2::rc::Retained;
use objc2::runtime::ProtocolObject;
use objc2_core_foundation::{CFRetained, CGSize};
use objc2_core_video as cv;
use objc2_foundation::{NSString, NSUInteger};
#[cfg(any(test, feature = "test-support"))]
use objc2_metal::MTLBlitCommandEncoder as _;
use objc2_metal::{self as mtl, MTLBuffer as _};
use objc2_metal::{
    MTLCommandBuffer as _, MTLCommandEncoder as _, MTLCommandQueue as _, MTLDevice as _,
    MTLDrawable as _, MTLLibrary as _, MTLRenderCommandEncoder as _, MTLTexture as _,
};
use objc2_quartz_core::{self as ca, CAMetalDrawable as _};
use std::{ffi::c_void, ops::Deref, ptr::NonNull};

pub(crate) use objc2_foundation::NSRange;
pub(crate) use objc2_metal::{
    MTLBlendFactor, MTLBlendOperation, MTLClearColor, MTLGPUFamily, MTLLoadAction, MTLOrigin,
    MTLPixelFormat, MTLPrimitiveType, MTLRegion, MTLResourceOptions, MTLSize, MTLStorageMode,
    MTLStoreAction, MTLTextureType, MTLTextureUsage, MTLViewport,
};
pub(crate) use objc2_quartz_core::CAMetalLayer;

pub(crate) type CommandBufferHandler = dyn Fn(NonNull<ProtocolObject<dyn mtl::MTLCommandBuffer>>);

#[derive(Clone)]
pub(crate) struct Device(Retained<ProtocolObject<dyn mtl::MTLDevice>>);

pub(crate) type DeviceRef = Device;

impl Device {
    pub(crate) fn system_default() -> Option<Self> {
        mtl::MTLCreateSystemDefaultDevice().map(Self)
    }

    pub(crate) fn all() -> Vec<Self> {
        let devices = mtl::MTLCopyAllDevices();
        (0..devices.len())
            .map(|index| Self(devices.objectAtIndex(index)))
            .collect()
    }

    pub(crate) fn as_protocol(&self) -> &ProtocolObject<dyn mtl::MTLDevice> {
        &self.0
    }

    pub(crate) fn is_low_power(&self) -> bool {
        self.0.isLowPower()
    }

    pub(crate) fn is_removable(&self) -> bool {
        self.0.isRemovable()
    }

    pub(crate) fn has_unified_memory(&self) -> bool {
        self.0.hasUnifiedMemory()
    }

    pub(crate) fn supports_family(&self, family: MTLGPUFamily) -> bool {
        self.0.supportsFamily(family)
    }

    pub(crate) fn new_command_queue(&self) -> CommandQueue {
        CommandQueue(
            self.0
                .newCommandQueue()
                .expect("Metal device failed to create command queue"),
        )
    }

    pub(crate) fn new_buffer(&self, length: u64, options: MTLResourceOptions) -> Buffer {
        Buffer(
            self.0
                .newBufferWithLength_options(length as NSUInteger, options)
                .expect("Metal device failed to create buffer"),
        )
    }

    pub(crate) fn new_buffer_with_data(
        &self,
        data: *const c_void,
        length: u64,
        options: MTLResourceOptions,
    ) -> Buffer {
        let data = NonNull::new(data.cast_mut()).expect("buffer source bytes must be non-null");
        Buffer(
            unsafe {
                self.0
                    .newBufferWithBytes_length_options(data, length as NSUInteger, options)
            }
            .expect("Metal device failed to create buffer from bytes"),
        )
    }

    pub(crate) fn new_texture(&self, descriptor: &TextureDescriptor) -> Texture {
        Texture(
            self.0
                .newTextureWithDescriptor(&descriptor.0)
                .expect("Metal device failed to create texture"),
        )
    }

    #[cfg(not(feature = "runtime_shaders"))]
    pub(crate) fn new_library_with_data(&self, data: &[u8]) -> Result<Library> {
        let data = DispatchData::from_bytes(data);
        self.0
            .newLibraryWithData_error(&data)
            .map(Library)
            .map_err(|error| anyhow!("error building metal library: {error:?}"))
    }

    #[cfg(feature = "runtime_shaders")]
    pub(crate) fn new_library_with_source(
        &self,
        source: &str,
        options: &CompileOptions,
    ) -> Result<Library> {
        self.0
            .newLibraryWithSource_options_error(&NSString::from_str(source), Some(&options.0))
            .map(Library)
            .map_err(|error| anyhow!("error building metal library: {error:?}"))
    }

    pub(crate) fn new_render_pipeline_state(
        &self,
        descriptor: &RenderPipelineDescriptor,
    ) -> Result<RenderPipelineState> {
        self.0
            .newRenderPipelineStateWithDescriptor_error(&descriptor.0)
            .map(RenderPipelineState)
            .map_err(|error| anyhow!("could not create render pipeline state: {error:?}"))
    }
}

impl Deref for Device {
    type Target = ProtocolObject<dyn mtl::MTLDevice>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[derive(Clone)]
pub(crate) struct MetalLayer(Retained<ca::CAMetalLayer>);

pub(crate) type MetalLayerRef = MetalLayer;

impl MetalLayer {
    pub(crate) fn new() -> Self {
        Self(ca::CAMetalLayer::new())
    }

    pub(crate) fn set_device(&self, device: &Device) {
        self.0.setDevice(Some(device.as_protocol()));
    }

    pub(crate) fn set_pixel_format(&self, pixel_format: MTLPixelFormat) {
        self.0.setPixelFormat(pixel_format);
    }

    pub(crate) fn set_opaque(&self, opaque: bool) {
        self.0.setOpaque(opaque);
    }

    pub(crate) fn set_maximum_drawable_count(&self, count: NSUInteger) {
        self.0.setMaximumDrawableCount(count);
    }

    #[cfg(any(test, feature = "test-support"))]
    pub(crate) fn set_framebuffer_only(&self, framebuffer_only: bool) {
        self.0.setFramebufferOnly(framebuffer_only);
    }

    pub(crate) fn set_presents_with_transaction(&self, presents_with_transaction: bool) {
        self.0.setPresentsWithTransaction(presents_with_transaction);
    }

    pub(crate) fn set_allows_next_drawable_timeout(&self, allows_timeout: bool) {
        self.0.setAllowsNextDrawableTimeout(allows_timeout);
    }

    pub(crate) fn set_needs_display_on_bounds_change(&self, needs_display: bool) {
        self.0.setNeedsDisplayOnBoundsChange(needs_display);
    }

    pub(crate) fn set_autoresizes_with_superlayer(&self) {
        self.0.setAutoresizingMask(
            ca::CAAutoresizingMask::LayerWidthSizable | ca::CAAutoresizingMask::LayerHeightSizable,
        );
    }

    pub(crate) fn set_drawable_size(&self, width: f64, height: f64) {
        self.0.setDrawableSize(CGSize { width, height });
    }

    pub(crate) fn set_contents_scale(&self, scale: f64) {
        self.0.setContentsScale(scale);
    }

    pub(crate) fn drawable_size(&self) -> CGSize {
        self.0.drawableSize()
    }

    pub(crate) fn next_drawable(&self) -> Option<MetalDrawable> {
        self.0.nextDrawable().map(MetalDrawable)
    }

    pub(crate) fn as_ptr(&self) -> *mut CAMetalLayer {
        Retained::as_ptr(&self.0).cast_mut()
    }
}

impl Deref for MetalLayer {
    type Target = ca::CAMetalLayer;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[derive(Clone)]
pub(crate) struct MetalDrawable(Retained<ProtocolObject<dyn ca::CAMetalDrawable>>);

pub(crate) type MetalDrawableRef = MetalDrawable;

impl MetalDrawable {
    pub(crate) fn texture(&self) -> Texture {
        Texture(self.0.texture())
    }

    pub(crate) fn present(&self) {
        self.0.present();
    }
}

impl Deref for MetalDrawable {
    type Target = ProtocolObject<dyn ca::CAMetalDrawable>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[derive(Clone)]
pub(crate) struct CommandQueue(Retained<ProtocolObject<dyn mtl::MTLCommandQueue>>);

impl CommandQueue {
    pub(crate) fn new_command_buffer(&self) -> CommandBuffer {
        CommandBuffer(
            self.0
                .commandBuffer()
                .expect("Metal command queue failed to create command buffer"),
        )
    }
}

#[derive(Clone)]
pub(crate) struct CommandBuffer(Retained<ProtocolObject<dyn mtl::MTLCommandBuffer>>);

pub(crate) type CommandBufferRef = CommandBuffer;

impl CommandBuffer {
    pub(crate) fn new_render_command_encoder(
        &self,
        descriptor: RenderPassDescriptor,
    ) -> RenderCommandEncoder {
        RenderCommandEncoder(
            self.0
                .renderCommandEncoderWithDescriptor(&descriptor.0)
                .expect("Metal command buffer failed to create render command encoder"),
        )
    }

    #[cfg(any(test, feature = "test-support"))]
    pub(crate) fn new_blit_command_encoder(&self) -> BlitCommandEncoder {
        BlitCommandEncoder(
            self.0
                .blitCommandEncoder()
                .expect("Metal command buffer failed to create blit command encoder"),
        )
    }

    pub(crate) fn add_completed_handler(&self, block: &RcBlock<CommandBufferHandler>) {
        unsafe { self.0.addCompletedHandler(RcBlock::as_ptr(block)) };
    }

    pub(crate) fn commit(&self) {
        self.0.commit();
    }

    pub(crate) fn wait_until_scheduled(&self) {
        self.0.waitUntilScheduled();
    }

    pub(crate) fn wait_until_completed(&self) {
        self.0.waitUntilCompleted();
    }

    pub(crate) fn present_drawable(&self, drawable: &MetalDrawable) {
        self.0.presentDrawable(drawable.0.as_ref());
    }
}

impl Deref for CommandBuffer {
    type Target = ProtocolObject<dyn mtl::MTLCommandBuffer>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[cfg(any(test, feature = "test-support"))]
#[derive(Clone)]
pub(crate) struct BlitCommandEncoder(Retained<ProtocolObject<dyn mtl::MTLBlitCommandEncoder>>);

#[cfg(any(test, feature = "test-support"))]
impl BlitCommandEncoder {
    pub(crate) fn synchronize_resource(&self, texture: &Texture) {
        self.0.synchronizeResource(texture.0.as_ref());
    }

    pub(crate) fn end_encoding(&self) {
        self.0.endEncoding();
    }
}

#[derive(Clone)]
pub(crate) struct Buffer(Retained<ProtocolObject<dyn mtl::MTLBuffer>>);

impl Buffer {
    pub(crate) fn contents(&self) -> *mut c_void {
        self.0.contents().as_ptr()
    }

    pub(crate) fn did_modify_range(&self, range: NSRange) {
        self.0.didModifyRange(range);
    }
}

impl Deref for Buffer {
    type Target = ProtocolObject<dyn mtl::MTLBuffer>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

pub(crate) struct TextureDescriptor(Retained<mtl::MTLTextureDescriptor>);

impl TextureDescriptor {
    pub(crate) fn new() -> Self {
        Self(mtl::MTLTextureDescriptor::new())
    }

    pub(crate) fn set_width(&self, width: u64) {
        unsafe { self.0.setWidth(width as NSUInteger) };
    }

    pub(crate) fn set_height(&self, height: u64) {
        unsafe { self.0.setHeight(height as NSUInteger) };
    }

    pub(crate) fn set_pixel_format(&self, pixel_format: MTLPixelFormat) {
        self.0.setPixelFormat(pixel_format);
    }

    pub(crate) fn set_usage(&self, usage: MTLTextureUsage) {
        self.0.setUsage(usage);
    }

    pub(crate) fn set_storage_mode(&self, storage_mode: MTLStorageMode) {
        self.0.setStorageMode(storage_mode);
    }

    pub(crate) fn set_texture_type(&self, texture_type: MTLTextureType) {
        self.0.setTextureType(texture_type);
    }

    pub(crate) fn set_sample_count(&self, sample_count: u64) {
        unsafe { self.0.setSampleCount(sample_count as NSUInteger) };
    }
}

impl Deref for TextureDescriptor {
    type Target = mtl::MTLTextureDescriptor;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[derive(Clone)]
pub(crate) struct Texture(Retained<ProtocolObject<dyn mtl::MTLTexture>>);

pub(crate) type TextureRef = Texture;

impl Texture {
    pub(crate) fn from_retained(texture: Retained<ProtocolObject<dyn mtl::MTLTexture>>) -> Self {
        Self(texture)
    }

    pub(crate) fn width(&self) -> NSUInteger {
        self.0.width()
    }

    pub(crate) fn height(&self) -> NSUInteger {
        self.0.height()
    }

    pub(crate) fn pixel_format(&self) -> MTLPixelFormat {
        self.0.pixelFormat()
    }

    #[cfg(any(test, feature = "test-support"))]
    pub(crate) fn get_bytes(
        &self,
        bytes: *mut c_void,
        bytes_per_row: u64,
        region: MTLRegion,
        level: NSUInteger,
    ) {
        let bytes = NonNull::new(bytes).expect("texture read bytes must be non-null");
        unsafe {
            self.0.getBytes_bytesPerRow_fromRegion_mipmapLevel(
                bytes,
                bytes_per_row as NSUInteger,
                region,
                level,
            );
        }
    }

    pub(crate) fn replace_region(
        &self,
        region: MTLRegion,
        level: NSUInteger,
        bytes: *const c_void,
        bytes_per_row: u64,
    ) {
        let bytes = NonNull::new(bytes.cast_mut()).expect("texture upload bytes must be non-null");
        unsafe {
            self.0.replaceRegion_mipmapLevel_withBytes_bytesPerRow(
                region,
                level,
                bytes,
                bytes_per_row as NSUInteger,
            );
        }
    }
}

impl Deref for Texture {
    type Target = ProtocolObject<dyn mtl::MTLTexture>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[cfg(feature = "runtime_shaders")]
pub(crate) struct CompileOptions(Retained<mtl::MTLCompileOptions>);

#[cfg(feature = "runtime_shaders")]
impl CompileOptions {
    pub(crate) fn new() -> Self {
        Self(mtl::MTLCompileOptions::new())
    }
}

pub(crate) struct Library(Retained<ProtocolObject<dyn mtl::MTLLibrary>>);

pub(crate) type LibraryRef = Library;

impl Library {
    pub(crate) fn get_function(
        &self,
        name: &str,
        _constant_values: Option<()>,
    ) -> Option<Function> {
        self.0
            .newFunctionWithName(&NSString::from_str(name))
            .map(Function)
    }
}

pub(crate) struct Function(Retained<ProtocolObject<dyn mtl::MTLFunction>>);

impl Deref for Function {
    type Target = ProtocolObject<dyn mtl::MTLFunction>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[derive(Clone)]
pub(crate) struct RenderPipelineState(Retained<ProtocolObject<dyn mtl::MTLRenderPipelineState>>);

impl Deref for RenderPipelineState {
    type Target = ProtocolObject<dyn mtl::MTLRenderPipelineState>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

pub(crate) struct RenderPipelineDescriptor(Retained<mtl::MTLRenderPipelineDescriptor>);

impl RenderPipelineDescriptor {
    pub(crate) fn new() -> Self {
        Self(mtl::MTLRenderPipelineDescriptor::new())
    }

    pub(crate) fn set_label(&self, label: &str) {
        self.0.setLabel(Some(&NSString::from_str(label)));
    }

    pub(crate) fn set_vertex_function(&self, function: Option<&Function>) {
        self.0
            .setVertexFunction(function.map(|function| &*function.0));
    }

    pub(crate) fn set_fragment_function(&self, function: Option<&Function>) {
        self.0
            .setFragmentFunction(function.map(|function| &*function.0));
    }

    pub(crate) fn set_raster_sample_count(&self, sample_count: NSUInteger) {
        self.0.setRasterSampleCount(sample_count);
    }

    pub(crate) fn set_alpha_to_coverage_enabled(&self, enabled: bool) {
        self.0.setAlphaToCoverageEnabled(enabled);
    }

    pub(crate) fn color_attachments(&self) -> RenderPipelineColorAttachmentDescriptorArray {
        RenderPipelineColorAttachmentDescriptorArray(self.0.colorAttachments())
    }
}

pub(crate) struct RenderPipelineColorAttachmentDescriptorArray(
    Retained<mtl::MTLRenderPipelineColorAttachmentDescriptorArray>,
);

impl RenderPipelineColorAttachmentDescriptorArray {
    pub(crate) fn object_at(
        &self,
        index: NSUInteger,
    ) -> Option<RenderPipelineColorAttachmentDescriptor> {
        Some(RenderPipelineColorAttachmentDescriptor(unsafe {
            self.0.objectAtIndexedSubscript(index)
        }))
    }
}

pub(crate) struct RenderPipelineColorAttachmentDescriptor(
    Retained<mtl::MTLRenderPipelineColorAttachmentDescriptor>,
);

impl RenderPipelineColorAttachmentDescriptor {
    pub(crate) fn set_pixel_format(&self, pixel_format: MTLPixelFormat) {
        self.0.setPixelFormat(pixel_format);
    }

    pub(crate) fn set_blending_enabled(&self, enabled: bool) {
        self.0.setBlendingEnabled(enabled);
    }

    pub(crate) fn set_rgb_blend_operation(&self, operation: MTLBlendOperation) {
        self.0.setRgbBlendOperation(operation);
    }

    pub(crate) fn set_alpha_blend_operation(&self, operation: MTLBlendOperation) {
        self.0.setAlphaBlendOperation(operation);
    }

    pub(crate) fn set_source_rgb_blend_factor(&self, factor: MTLBlendFactor) {
        self.0.setSourceRGBBlendFactor(factor);
    }

    pub(crate) fn set_source_alpha_blend_factor(&self, factor: MTLBlendFactor) {
        self.0.setSourceAlphaBlendFactor(factor);
    }

    pub(crate) fn set_destination_rgb_blend_factor(&self, factor: MTLBlendFactor) {
        self.0.setDestinationRGBBlendFactor(factor);
    }

    pub(crate) fn set_destination_alpha_blend_factor(&self, factor: MTLBlendFactor) {
        self.0.setDestinationAlphaBlendFactor(factor);
    }
}

pub(crate) struct RenderPassDescriptor(Retained<mtl::MTLRenderPassDescriptor>);

impl RenderPassDescriptor {
    pub(crate) fn new() -> Self {
        Self(mtl::MTLRenderPassDescriptor::renderPassDescriptor())
    }

    pub(crate) fn color_attachments(&self) -> RenderPassColorAttachmentDescriptorArray {
        RenderPassColorAttachmentDescriptorArray(self.0.colorAttachments())
    }
}

pub(crate) struct RenderPassColorAttachmentDescriptorArray(
    Retained<mtl::MTLRenderPassColorAttachmentDescriptorArray>,
);

impl RenderPassColorAttachmentDescriptorArray {
    pub(crate) fn object_at(
        &self,
        index: NSUInteger,
    ) -> Option<RenderPassColorAttachmentDescriptor> {
        Some(RenderPassColorAttachmentDescriptor(unsafe {
            self.0.objectAtIndexedSubscript(index)
        }))
    }
}

pub(crate) struct RenderPassColorAttachmentDescriptor(
    Retained<mtl::MTLRenderPassColorAttachmentDescriptor>,
);

impl RenderPassColorAttachmentDescriptor {
    pub(crate) fn set_texture(&self, texture: Option<&Texture>) {
        self.0.setTexture(texture.map(|texture| &*texture.0));
    }

    pub(crate) fn set_resolve_texture(&self, texture: Option<&Texture>) {
        self.0.setResolveTexture(texture.map(|texture| &*texture.0));
    }

    pub(crate) fn set_load_action(&self, action: MTLLoadAction) {
        self.0.setLoadAction(action);
    }

    pub(crate) fn set_store_action(&self, action: MTLStoreAction) {
        self.0.setStoreAction(action);
    }

    pub(crate) fn set_clear_color(&self, clear_color: MTLClearColor) {
        self.0.setClearColor(clear_color);
    }
}

#[derive(Clone)]
pub(crate) struct RenderCommandEncoder(Retained<ProtocolObject<dyn mtl::MTLRenderCommandEncoder>>);

pub(crate) type RenderCommandEncoderRef = RenderCommandEncoder;

impl RenderCommandEncoder {
    pub(crate) fn end_encoding(&self) {
        self.0.endEncoding();
    }

    pub(crate) fn set_render_pipeline_state(&self, state: &RenderPipelineState) {
        self.0.setRenderPipelineState(&state.0);
    }

    pub(crate) fn set_viewport(&self, viewport: MTLViewport) {
        self.0.setViewport(viewport);
    }

    pub(crate) fn set_vertex_buffer(&self, index: u64, buffer: Option<&Buffer>, offset: u64) {
        unsafe {
            self.0.setVertexBuffer_offset_atIndex(
                buffer.map(|buffer| &*buffer.0),
                offset as NSUInteger,
                index as NSUInteger,
            );
        }
    }

    pub(crate) fn set_fragment_buffer(&self, index: u64, buffer: Option<&Buffer>, offset: u64) {
        unsafe {
            self.0.setFragmentBuffer_offset_atIndex(
                buffer.map(|buffer| &*buffer.0),
                offset as NSUInteger,
                index as NSUInteger,
            );
        }
    }

    pub(crate) fn set_vertex_bytes(&self, index: u64, length: u64, bytes: *const c_void) {
        let bytes = NonNull::new(bytes.cast_mut()).expect("vertex bytes must be non-null");
        unsafe {
            self.0
                .setVertexBytes_length_atIndex(bytes, length as NSUInteger, index as NSUInteger)
        };
    }

    pub(crate) fn set_fragment_texture(&self, index: u64, texture: Option<&Texture>) {
        unsafe {
            self.0.setFragmentTexture_atIndex(
                texture.map(|texture| &*texture.0),
                index as NSUInteger,
            );
        }
    }

    pub(crate) fn draw_primitives(
        &self,
        primitive_type: MTLPrimitiveType,
        vertex_start: u64,
        vertex_count: u64,
    ) {
        unsafe {
            self.0.drawPrimitives_vertexStart_vertexCount(
                primitive_type,
                vertex_start as NSUInteger,
                vertex_count as NSUInteger,
            );
        }
    }

    pub(crate) fn draw_primitives_instanced(
        &self,
        primitive_type: MTLPrimitiveType,
        vertex_start: u64,
        vertex_count: u64,
        instance_count: u64,
    ) {
        unsafe {
            self.0.drawPrimitives_vertexStart_vertexCount_instanceCount(
                primitive_type,
                vertex_start as NSUInteger,
                vertex_count as NSUInteger,
                instance_count as NSUInteger,
            );
        }
    }
}

pub(crate) fn clear_color(red: f64, green: f64, blue: f64, alpha: f64) -> MTLClearColor {
    MTLClearColor {
        red,
        green,
        blue,
        alpha,
    }
}

pub(crate) fn region_2d(
    x: NSUInteger,
    y: NSUInteger,
    width: NSUInteger,
    height: NSUInteger,
) -> MTLRegion {
    MTLRegion {
        origin: MTLOrigin { x, y, z: 0 },
        size: MTLSize {
            width,
            height,
            depth: 1,
        },
    }
}

pub(crate) struct CoreVideoTextureCache(CFRetained<cv::CVMetalTextureCache>);

impl CoreVideoTextureCache {
    pub(crate) fn new(device: &Device) -> Result<Self> {
        let mut cache = std::ptr::null_mut();
        let result = unsafe {
            cv::CVMetalTextureCache::create(
                None,
                None,
                device.as_protocol(),
                None,
                NonNull::from(&mut cache),
            )
        };
        anyhow::ensure!(
            result == cv::kCVReturnSuccess,
            "could not create texture cache, code: {result}"
        );
        let cache = NonNull::new(cache).ok_or_else(|| anyhow!("CVMetalTextureCache is null"))?;
        Ok(Self(unsafe { CFRetained::from_raw(cache) }))
    }

    pub(crate) fn create_texture_from_image(
        &self,
        source: &cv::CVImageBuffer,
        pixel_format: MTLPixelFormat,
        width: usize,
        height: usize,
        plane_index: usize,
    ) -> Result<CoreVideoTexture> {
        let mut texture = std::ptr::null_mut();
        let result = unsafe {
            cv::CVMetalTextureCache::create_texture_from_image(
                None,
                &self.0,
                source,
                None,
                pixel_format,
                width,
                height,
                plane_index,
                NonNull::from(&mut texture),
            )
        };
        anyhow::ensure!(
            result == cv::kCVReturnSuccess,
            "could not create texture, code: {result}"
        );
        let texture = NonNull::new(texture).ok_or_else(|| anyhow!("CVMetalTexture is null"))?;
        Ok(CoreVideoTexture(unsafe { CFRetained::from_raw(texture) }))
    }
}

pub(crate) struct CoreVideoTexture(CFRetained<cv::CVMetalTexture>);

impl CoreVideoTexture {
    pub(crate) fn texture(&self) -> Option<Texture> {
        cv::CVMetalTextureGetTexture(&self.0).map(Texture::from_retained)
    }
}
