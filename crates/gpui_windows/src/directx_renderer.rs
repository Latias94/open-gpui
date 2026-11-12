use std::{
    ops::Range,
    slice,
    sync::{Arc, OnceLock},
    thread,
    time::{Duration, Instant},
};

use ::open_gpui_util::ResultExt;
use anyhow::{Context, Result, bail};
#[cfg(any(test, feature = "test-support"))]
use image::RgbaImage;
use windows::{
    Win32::{
        Foundation::{HWND, S_FALSE, S_OK},
        Graphics::{
            Direct3D::*,
            Direct3D11::*,
            DirectComposition::*,
            DirectWrite::*,
            Dxgi::{Common::*, *},
        },
    },
    core::{HSTRING, IUnknown, Interface},
};

use crate::directx_renderer::shader_resources::{RawShaderBytes, ShaderModule, ShaderTarget};
use crate::*;
use open_gpui::*;

pub(crate) const DISABLE_DIRECT_COMPOSITION: &str = "GPUI_DISABLE_DIRECT_COMPOSITION";
const RENDER_TARGET_FORMAT: DXGI_FORMAT = DXGI_FORMAT_B8G8R8A8_UNORM;
const CLIP_SHAPES_SHADER_RESOURCE_SLOT: u32 = 2;
const PATH_CURVES_SHADER_RESOURCE_SLOT: u32 = 3;
const MAX_INSTANCE_BUFFER_SIZE: usize = 256 * 1024 * 1024;

#[cfg(all(test, feature = "test-support"))]
#[path = "directx_atlas/render_tests.rs"]
pub(crate) mod atlas_render_tests;

pub(crate) struct FontInfo {
    pub gamma_ratios: [f32; 4],
    pub grayscale_enhanced_contrast: f32,
    pub subpixel_enhanced_contrast: f32,
    pub is_bgr: bool,
}

pub(crate) struct DirectXRenderer {
    hwnd: HWND,
    atlas: Arc<DirectXAtlas>,
    devices: Option<DirectXRendererDevices>,
    resources: Option<DirectXResources>,
    globals: DirectXGlobalElements,
    pipelines: DirectXRenderPipelines,
    direct_composition: Option<DirectComposition>,
    font_info: &'static FontInfo,

    width: u32,
    height: u32,

    /// Whether device recovery invalidated every scene accepted before the reset.
    ///
    /// This remains set until a scene reaches `draw`, so native frame throttling cannot consume the
    /// recovery signal before GPUI establishes sticky fresh-paint authority.
    repaint_required_after_recovery: bool,
    surface_quiesced_for: Option<WindowPresentationShutdownTicket>,
}

/// Direct3D objects
#[derive(Clone)]
pub(crate) struct DirectXRendererDevices {
    pub(crate) adapter: IDXGIAdapter1,
    pub(crate) dxgi_factory: IDXGIFactory6,
    pub(crate) device: ID3D11Device,
    pub(crate) device_context: ID3D11DeviceContext,
    device_context1: Option<ID3D11DeviceContext1>,
    dxgi_device: Option<IDXGIDevice>,
    annotation: Option<ID3DUserDefinedAnnotation>,
}

struct DirectXResources {
    // Direct3D rendering objects
    swap_chain: IDXGISwapChain1,
    render_target: Option<ID3D11Texture2D>,
    render_target_view: Option<ID3D11RenderTargetView>,

    depth_stencil_texture: ID3D11Texture2D,
    depth_stencil_view: Option<ID3D11DepthStencilView>,

    // Cached viewport
    viewport: D3D11_VIEWPORT,
}

struct DirectXRenderPipelines {
    shadow_pipeline: PipelineState<Shadow>,
    quad_pipeline: PipelineState<Quad>,
    opaque_quad_pipeline: OpaqueQuadPipeline,
    path_tile_pipeline: PipelineState<PathTile>,
    path_curves: StructuredBuffer<PathCurve>,
    path_paints: StructuredBuffer<PathPaint>,
    underline_pipeline: PipelineState<Underline>,
    mono_sprites: PipelineState<MonochromeSprite>,
    subpixel_sprites: PipelineState<SubpixelSprite>,
    poly_sprites: PipelineState<PolychromeSprite>,
}

struct DirectXGlobalElements {
    constant_buffers: [Option<ID3D11Buffer>; 2],
    sampler: Option<ID3D11SamplerState>,
    clip_shapes_buffer: ID3D11Buffer,
    clip_shapes_view: Option<ID3D11ShaderResourceView>,
    clip_shapes_capacity: usize,
}

struct Annotation<'a>(&'a ID3DUserDefinedAnnotation);

impl<'a> Annotation<'a> {
    fn new(annotation: &'a ID3DUserDefinedAnnotation, label: HSTRING) -> Self {
        unsafe { annotation.BeginEvent(&label) };
        Self(annotation)
    }
}

impl Drop for Annotation<'_> {
    fn drop(&mut self) {
        unsafe { self.0.EndEvent() };
    }
}

struct DirectComposition {
    comp_device: IDCompositionDevice,
    comp_target: IDCompositionTarget,
    comp_visual: IDCompositionVisual,
}

impl DirectXRendererDevices {
    pub(crate) fn new(
        directx_devices: &DirectXDevices,
        disable_direct_composition: bool,
    ) -> Result<Self> {
        let DirectXDevices {
            adapter,
            dxgi_factory,
            device,
            device_context,
        } = directx_devices;
        let dxgi_device = if disable_direct_composition {
            None
        } else {
            Some(device.cast().context("Creating DXGI device")?)
        };

        Ok(Self {
            adapter: adapter.clone(),
            dxgi_factory: dxgi_factory.clone(),
            device: device.clone(),
            device_context: device_context.clone(),
            device_context1: device_context.cast().ok(),
            dxgi_device,
            annotation: device_context.cast().ok(),
        })
    }
}

impl DirectXRenderer {
    pub(crate) fn new(
        hwnd: HWND,
        directx_devices: &DirectXDevices,
        disable_direct_composition: bool,
    ) -> Result<Self> {
        if disable_direct_composition {
            log::info!("Direct Composition is disabled.");
        }

        let devices = DirectXRendererDevices::new(directx_devices, disable_direct_composition)
            .context("Creating DirectX devices")?;
        let atlas = Arc::new(DirectXAtlas::new(&devices.device, &devices.device_context));

        let resources = DirectXResources::new(&devices, 1, 1, hwnd, disable_direct_composition)
            .context("Creating DirectX resources")?;
        let globals = DirectXGlobalElements::new(&devices.device)
            .context("Creating DirectX global elements")?;
        let pipelines = DirectXRenderPipelines::new(&devices.device)
            .context("Creating DirectX render pipelines")?;

        let direct_composition = if disable_direct_composition {
            None
        } else {
            let composition = DirectComposition::new(devices.dxgi_device.as_ref().unwrap(), hwnd)
                .context("Creating DirectComposition")?;
            composition
                .set_swap_chain(&resources.swap_chain)
                .context("Setting swap chain for DirectComposition")?;
            Some(composition)
        };

        Ok(DirectXRenderer {
            hwnd,
            atlas,
            devices: Some(devices),
            resources: Some(resources),
            globals,
            pipelines,
            direct_composition,
            font_info: Self::get_font_info(),
            width: 1,
            height: 1,
            repaint_required_after_recovery: false,
            surface_quiesced_for: None,
        })
    }

    pub(crate) fn sprite_atlas(&self) -> Arc<dyn PlatformAtlas> {
        self.atlas.clone()
    }

    fn pre_draw(
        &self,
        clear_color: &[f32; 4],
        clip_shape_count: usize,
        depth_scale: f32,
    ) -> Result<()> {
        let resources = self.resources.as_ref().expect("resources missing");
        let device_context = &self
            .devices
            .as_ref()
            .expect("devices missing")
            .device_context;
        let clip_shape_count = u32::try_from(clip_shape_count)
            .context("scene clip arena exceeds the DirectX shader index range")?;
        update_buffer(
            device_context,
            self.globals.constant_buffers[0]
                .as_ref()
                .context("missing DirectX global parameters")?,
            &[GlobalParams {
                gamma_ratios: self.font_info.gamma_ratios,
                viewport_size: [resources.viewport.Width, resources.viewport.Height],
                grayscale_enhanced_contrast: self.font_info.grayscale_enhanced_contrast,
                subpixel_enhanced_contrast: self.font_info.subpixel_enhanced_contrast,
                is_bgr: self.font_info.is_bgr as u32,
                clip_shape_count,
                depth_scale,
                _pad: 0,
            }],
        )?;
        unsafe {
            device_context.ClearRenderTargetView(
                resources
                    .render_target_view
                    .as_ref()
                    .context("missing render target view")?,
                clear_color,
            );
            device_context.ClearDepthStencilView(
                resources
                    .depth_stencil_view
                    .as_ref()
                    .context("missing depth stencil view")?,
                D3D11_CLEAR_DEPTH.0,
                0.0,
                0,
            );
            device_context.OMSetRenderTargets(
                Some(slice::from_ref(&resources.render_target_view)),
                resources.depth_stencil_view.as_ref(),
            );
            device_context.RSSetViewports(Some(slice::from_ref(&resources.viewport)));
            device_context.VSSetConstantBuffers(0, Some(&self.globals.constant_buffers));
            device_context.PSSetConstantBuffers(0, Some(&self.globals.constant_buffers[..1]));
            let clip_shapes = slice::from_ref(&self.globals.clip_shapes_view);
            device_context
                .VSSetShaderResources(CLIP_SHAPES_SHADER_RESOURCE_SLOT, Some(clip_shapes));
            device_context
                .PSSetShaderResources(CLIP_SHAPES_SHADER_RESOURCE_SLOT, Some(clip_shapes));
        }
        Ok(())
    }

    fn discard_depth_buffer(&self) -> Option<()> {
        let context = self.devices.as_ref()?.device_context1.as_ref()?;
        let view = self.resources.as_ref()?.depth_stencil_view.as_ref()?;
        unsafe { context.DiscardView(view) };
        Some(())
    }

    #[inline]
    fn present(&mut self) -> Result<()> {
        let result = unsafe {
            self.resources
                .as_ref()
                .expect("resources missing")
                .swap_chain
                .Present(0, DXGI_PRESENT(0))
        };
        result.ok().context("Presenting swap chain failed")
    }

    pub(crate) fn handle_device_lost(&mut self, directx_devices: &DirectXDevices) -> Result<()> {
        if self.surface_quiesced_for.is_some() {
            return Ok(());
        }
        try_to_recover_from_device_lost(|| {
            self.handle_device_lost_impl(directx_devices)
                .context("DirectXRenderer handling device lost")
        })
    }

    fn handle_device_lost_impl(&mut self, directx_devices: &DirectXDevices) -> Result<()> {
        if self.surface_quiesced_for.is_some() {
            return Ok(());
        }
        let disable_direct_composition = self.direct_composition.is_none();

        unsafe {
            #[cfg(debug_assertions)]
            if let Some(devices) = &self.devices {
                report_live_objects(&devices.device)
                    .context("Failed to report live objects after device lost")
                    .log_err();
            }

            self.resources.take();
            if let Some(devices) = &self.devices {
                devices.device_context.OMSetRenderTargets(None, None);
                devices.device_context.ClearState();
                devices.device_context.Flush();
                #[cfg(debug_assertions)]
                report_live_objects(&devices.device)
                    .context("Failed to report live objects after device lost")
                    .log_err();
            }

            self.direct_composition.take();
            self.devices.take();
        }

        let devices = DirectXRendererDevices::new(directx_devices, disable_direct_composition)
            .context("Recreating DirectX devices")?;
        let resources = DirectXResources::new(
            &devices,
            self.width,
            self.height,
            self.hwnd,
            disable_direct_composition,
        )
        .context("Creating DirectX resources")?;
        let globals = DirectXGlobalElements::new(&devices.device)
            .context("Creating DirectXGlobalElements")?;
        let pipelines = DirectXRenderPipelines::new(&devices.device)
            .context("Creating DirectXRenderPipelines")?;

        let direct_composition = if disable_direct_composition {
            None
        } else {
            let composition =
                DirectComposition::new(devices.dxgi_device.as_ref().unwrap(), self.hwnd)?;
            composition.set_swap_chain(&resources.swap_chain)?;
            Some(composition)
        };

        self.atlas
            .handle_device_lost(&devices.device, &devices.device_context);

        unsafe {
            devices.device_context.OMSetRenderTargets(
                Some(slice::from_ref(&resources.render_target_view)),
                resources.depth_stencil_view.as_ref(),
            );
        }
        self.devices = Some(devices);
        self.resources = Some(resources);
        self.globals = globals;
        self.pipelines = pipelines;
        self.direct_composition = direct_composition;
        self.repaint_required_after_recovery = true;
        Ok(())
    }

    /// Rejects replay of the first scene observed after device recovery.
    pub(crate) fn draw(
        &mut self,
        scene: &Scene,
        background_appearance: WindowBackgroundAppearance,
    ) -> Result<PlatformWindowPresentOutcome> {
        if self.surface_quiesced_for.is_some() {
            return Ok(PlatformWindowPresentOutcome::Rejected);
        }
        if std::mem::take(&mut self.repaint_required_after_recovery) {
            return Ok(PlatformWindowPresentOutcome::RepaintRequired);
        }
        let preflight = match self.atlas.preflight(scene) {
            Ok(preflight) => preflight,
            Err(error) => {
                log::error!("refusing a DirectX frame with unavailable atlas resources: {error:#}");
                return Ok(PlatformWindowPresentOutcome::RepaintRequired);
            }
        };
        if let Err(error) = self.draw_scene_to_target(scene, background_appearance, &preflight) {
            if let Err(invalidation) = preflight.validate() {
                log::error!("DirectX atlas resources changed during rendering: {invalidation:#}");
                return Ok(PlatformWindowPresentOutcome::RepaintRequired);
            }
            return Err(error);
        }
        if let Err(error) = preflight.validate() {
            log::error!("DirectX atlas resources changed before presentation: {error:#}");
            return Ok(PlatformWindowPresentOutcome::RepaintRequired);
        }
        self.present()?;
        Ok(PlatformWindowPresentOutcome::Submitted)
    }

    fn draw_scene_to_target(
        &mut self,
        scene: &Scene,
        background_appearance: WindowBackgroundAppearance,
        preflight: &DirectXAtlasPreflight,
    ) -> Result<()> {
        preflight.validate()?;
        self.upload_scene_buffers(scene)?;
        let depth_scale = quad_depth_scale(scene.quads.len());
        self.pre_draw(
            &match background_appearance {
                WindowBackgroundAppearance::Opaque => [1.0f32; 4],
                _ => [0.0f32; 4],
            },
            scene.clip_shapes().len(),
            depth_scale,
        )?;

        let annotation = self
            .devices
            .as_ref()
            .and_then(|devices| devices.annotation.as_ref())
            .filter(|annotation| unsafe { annotation.GetStatus().as_bool() })
            .cloned();
        self.draw_opaque_quads(scene.quads.len())?;
        let mut quad_cursor = 0u32;
        let mut path_tile_cursor = 0usize;
        for batch in scene.batches() {
            let _annotation = annotation
                .as_ref()
                .map(|annotation| Annotation::new(annotation, HSTRING::from(batch.label())));
            if matches!(&batch, PrimitiveBatch::Quads(_)) {
                self.set_viewport_depth_range(0.0, 1.0)?;
            } else {
                let depth = quad_depth(quad_cursor, depth_scale);
                self.set_viewport_depth_range(depth, depth)?;
            }
            match batch {
                PrimitiveBatch::Shadows(range) => {
                    self.draw_instance_batch(&self.pipelines.shadow_pipeline, range, None)
                }
                PrimitiveBatch::Quads(range) => {
                    quad_cursor = u32::try_from(range.end).context("DirectX quad count overflow")?;
                    self.draw_instance_batch(&self.pipelines.quad_pipeline, range, None)
                }
                PrimitiveBatch::Paths(range) => {
                    let start = path_tile_cursor;
                    for path in &scene.paths[range] {
                        path_tile_cursor = path_tile_cursor
                            .checked_add(path.tile_count())
                            .context("DirectX path tile count overflow")?;
                    }
                    self.draw_instance_batch(
                        &self.pipelines.path_tile_pipeline,
                        start..path_tile_cursor,
                        None,
                    )
                }
                PrimitiveBatch::Underlines(range) => {
                    self.draw_instance_batch(&self.pipelines.underline_pipeline, range, None)
                }
                PrimitiveBatch::MonochromeSprites { texture, range } => self.draw_instance_batch(
                    &self.pipelines.mono_sprites,
                    range,
                    Some(
                        preflight
                            .texture_view(texture)
                            .context("monochrome texture is absent from the DirectX preflight")?,
                    ),
                ),
                PrimitiveBatch::SubpixelSprites { texture, range } => self.draw_instance_batch(
                    &self.pipelines.subpixel_sprites,
                    range,
                    Some(
                        preflight
                            .texture_view(texture)
                            .context("subpixel texture is absent from the DirectX preflight")?,
                    ),
                ),
                PrimitiveBatch::PolychromeSprites { texture, range } => self.draw_instance_batch(
                    &self.pipelines.poly_sprites,
                    range,
                    Some(
                        preflight
                            .texture_view(texture)
                            .context("polychrome texture is absent from the DirectX preflight")?,
                    ),
                ),
                PrimitiveBatch::Surfaces(range) => self.draw_surfaces(&scene.surfaces[range]),
            }
            .with_context(|| {
                format!(
                    "drawing DirectX scene with {} paths, {} shadows, {} quads, {} underlines, {} mono, {} subpixel, {} poly, {} surfaces",
                    scene.paths.len(),
                    scene.shadows.len(),
                    scene.quads.len(),
                    scene.underlines.len(),
                    scene.monochrome_sprites.len(),
                    scene.subpixel_sprites.len(),
                    scene.polychrome_sprites.len(),
                    scene.surfaces.len(),
                )
            })?;
        }
        let _ = self.discard_depth_buffer();
        Ok(())
    }

    fn set_viewport_depth_range(&self, minimum_depth: f32, maximum_depth: f32) -> Result<()> {
        let devices = self.devices.as_ref().context("devices missing")?;
        let resources = self.resources.as_ref().context("resources missing")?;
        let viewport = D3D11_VIEWPORT {
            MinDepth: minimum_depth,
            MaxDepth: maximum_depth,
            ..resources.viewport
        };
        unsafe {
            devices
                .device_context
                .RSSetViewports(Some(slice::from_ref(&viewport)))
        };
        Ok(())
    }

    fn draw_opaque_quads(&self, quad_count: usize) -> Result<()> {
        let devices = self.devices.as_ref().context("devices missing")?;
        self.pipelines.opaque_quad_pipeline.draw(
            &devices.device_context,
            &self.pipelines.quad_pipeline,
            self.globals.constant_buffers[1]
                .as_ref()
                .context("missing DirectX batch parameters")?,
            quad_count,
        )
    }

    fn draw_instance_batch<T>(
        &self,
        pipeline: &PipelineState<T>,
        range: Range<usize>,
        texture: Option<&[Option<ID3D11ShaderResourceView>; 1]>,
    ) -> Result<()> {
        let devices = self.devices.as_ref().context("devices missing")?;
        let batch_params = self.globals.constant_buffers[1]
            .as_ref()
            .context("missing DirectX batch parameters")?;
        pipeline.draw_range(
            &devices.device_context,
            batch_params,
            range,
            texture.map(|view| (view.as_slice(), slice::from_ref(&self.globals.sampler))),
        )
    }

    #[cfg(any(test, feature = "test-support"))]
    pub(crate) fn render_to_image(
        &mut self,
        scene: &Scene,
        background_appearance: WindowBackgroundAppearance,
    ) -> Result<RgbaImage> {
        anyhow::ensure!(
            !self.repaint_required_after_recovery,
            "cannot capture a scene whose DirectX atlas resources were invalidated by device recovery"
        );
        anyhow::ensure!(
            self.surface_quiesced_for.is_none(),
            "cannot capture a quiesced DirectX surface"
        );
        let preflight = self.atlas.preflight(scene)?;
        self.draw_scene_to_target(scene, background_appearance, &preflight)?;
        preflight.validate()?;
        self.read_render_target_to_image()
    }

    #[cfg(any(test, feature = "test-support"))]
    fn read_render_target_to_image(&self) -> Result<RgbaImage> {
        let devices = self.devices.as_ref().context("devices missing")?;
        let resources = self.resources.as_ref().context("resources missing")?;
        let render_target = resources
            .render_target
            .as_ref()
            .context("missing render target")?;

        let mut desc = D3D11_TEXTURE2D_DESC::default();
        unsafe {
            render_target.GetDesc(&mut desc);
        }
        anyhow::ensure!(
            desc.Width > 0 && desc.Height > 0,
            "cannot capture empty DirectX render target"
        );
        anyhow::ensure!(
            desc.Format == RENDER_TARGET_FORMAT,
            "unsupported DirectX render target format for capture: {:?}",
            desc.Format
        );

        desc.Usage = D3D11_USAGE_STAGING;
        desc.BindFlags = 0;
        desc.CPUAccessFlags = D3D11_CPU_ACCESS_READ.0 as u32;
        desc.MiscFlags = 0;

        let mut staging = None;
        unsafe {
            devices
                .device
                .CreateTexture2D(&desc, None, Some(&mut staging))
                .context("creating DirectX screenshot staging texture")?;
        }
        let staging = staging.context("DirectX screenshot staging texture missing")?;

        unsafe {
            devices.device_context.CopyResource(&staging, render_target);
        }

        let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
        unsafe {
            devices
                .device_context
                .Map(&staging, 0, D3D11_MAP_READ, 0, Some(&mut mapped))
                .context("mapping DirectX screenshot staging texture")?;
        }

        let width = desc.Width as usize;
        let height = desc.Height as usize;
        let row_pitch = mapped.RowPitch as usize;
        let src = mapped.pData as *const u8;
        let mut rgba = Vec::with_capacity(width * height * 4);
        for row in 0..height {
            let row_start = unsafe { src.add(row * row_pitch) };
            let row_bytes = unsafe { std::slice::from_raw_parts(row_start, width * 4) };
            for bgra in row_bytes.chunks_exact(4) {
                rgba.push(bgra[2]);
                rgba.push(bgra[1]);
                rgba.push(bgra[0]);
                rgba.push(bgra[3]);
            }
        }

        unsafe {
            devices.device_context.Unmap(&staging, 0);
        }

        RgbaImage::from_raw(desc.Width, desc.Height, rgba)
            .context("creating RGBA image from DirectX screenshot bytes")
    }

    pub(crate) fn resize(&mut self, new_size: Size<DevicePixels>) -> Result<()> {
        if self.surface_quiesced_for.is_some() {
            bail!("cannot resize a quiesced DirectX window surface");
        }
        let width = new_size.width.0.max(1) as u32;
        let height = new_size.height.0.max(1) as u32;
        if self.width == width && self.height == height {
            return Ok(());
        }
        self.width = width;
        self.height = height;

        // Clear the render target before resizing
        let devices = self.devices.as_ref().context("devices missing")?;
        unsafe { devices.device_context.OMSetRenderTargets(None, None) };
        let resources = self.resources.as_mut().context("resources missing")?;
        resources.render_target.take();
        resources.render_target_view.take();

        // Resizing the swap chain requires a call to the underlying DXGI adapter, which can return the device removed error.
        // The app might have moved to a monitor that's attached to a different graphics device.
        // When a graphics device is removed or reset, the desktop resolution often changes, resulting in a window size change.
        // But here we just return the error, because we are handling device lost scenarios elsewhere.
        unsafe {
            resources
                .swap_chain
                .ResizeBuffers(
                    BUFFER_COUNT as u32,
                    width,
                    height,
                    RENDER_TARGET_FORMAT,
                    DXGI_SWAP_CHAIN_FLAG(0),
                )
                .context("Failed to resize swap chain")?;
        }

        resources.recreate_resources(devices, width, height)?;

        unsafe {
            devices.device_context.OMSetRenderTargets(
                Some(slice::from_ref(&resources.render_target_view)),
                resources.depth_stencil_view.as_ref(),
            );
        }

        Ok(())
    }

    fn upload_scene_buffers(&mut self, scene: &Scene) -> Result<()> {
        let devices = self.devices.as_ref().context("devices missing")?;

        self.globals.update_clip_shapes(
            &devices.device,
            &devices.device_context,
            scene.clip_shapes(),
        )?;

        self.pipelines.shadow_pipeline.update_buffer(
            &devices.device,
            &devices.device_context,
            &scene.shadows,
        )?;
        self.pipelines.quad_pipeline.update_buffer(
            &devices.device,
            &devices.device_context,
            &scene.quads,
        )?;
        let mut paints = Vec::with_capacity(scene.paths.len());
        let mut curves = Vec::new();
        let mut tiles = Vec::new();
        for path in &scene.paths {
            path.append_to(&mut paints, &mut curves, &mut tiles);
        }
        self.pipelines.path_tile_pipeline.update_buffer(
            &devices.device,
            &devices.device_context,
            &tiles,
        )?;
        self.pipelines
            .path_curves
            .write(&devices.device, &devices.device_context, &curves)?;
        self.pipelines
            .path_paints
            .write(&devices.device, &devices.device_context, &paints)?;
        let path_buffers = [
            self.pipelines.path_curves.view.clone(),
            self.pipelines.path_paints.view.clone(),
        ];
        unsafe {
            devices
                .device_context
                .VSSetShaderResources(PATH_CURVES_SHADER_RESOURCE_SLOT, Some(&path_buffers));
            devices
                .device_context
                .PSSetShaderResources(PATH_CURVES_SHADER_RESOURCE_SLOT, Some(&path_buffers));
        }
        self.pipelines.underline_pipeline.update_buffer(
            &devices.device,
            &devices.device_context,
            &scene.underlines,
        )?;
        self.pipelines.mono_sprites.update_buffer(
            &devices.device,
            &devices.device_context,
            &scene.monochrome_sprites,
        )?;
        self.pipelines.subpixel_sprites.update_buffer(
            &devices.device,
            &devices.device_context,
            &scene.subpixel_sprites,
        )?;
        self.pipelines.poly_sprites.update_buffer(
            &devices.device,
            &devices.device_context,
            &scene.polychrome_sprites,
        )
    }

    fn draw_surfaces(&mut self, surfaces: &[PaintSurface]) -> Result<()> {
        if surfaces.is_empty() {
            return Ok(());
        }
        // Windows has no native PaintSurface payload. Keeping unsupported surfaces invisible is
        // fail-closed; drawing their bounds without the exact clip stack would leak pixels.
        Ok(())
    }

    pub(crate) fn gpu_specs(&self) -> Result<GpuSpecs> {
        let devices = self.devices.as_ref().context("devices missing")?;
        let desc = unsafe { devices.adapter.GetDesc1() }?;
        let is_software_emulated = (desc.Flags & DXGI_ADAPTER_FLAG_SOFTWARE.0 as u32) != 0;
        let device_name = String::from_utf16_lossy(&desc.Description)
            .trim_matches(char::from(0))
            .to_string();
        let driver_name = match desc.VendorId {
            0x10DE => "NVIDIA Corporation".to_string(),
            0x1002 => "AMD Corporation".to_string(),
            0x8086 => "Intel Corporation".to_string(),
            id => format!("Unknown Vendor (ID: {:#X})", id),
        };
        let driver_version = match desc.VendorId {
            0x10DE => nvidia::get_driver_version(),
            0x1002 => amd::get_driver_version(),
            // For Intel and other vendors, we use the DXGI API to get the driver version.
            _ => dxgi::get_driver_version(&devices.adapter),
        }
        .context("Failed to get gpu driver info")
        .log_err()
        .unwrap_or("Unknown Driver".to_string());
        Ok(GpuSpecs {
            is_software_emulated,
            device_name,
            driver_name,
            driver_info: driver_version,
        })
    }

    pub(crate) fn get_font_info() -> &'static FontInfo {
        static CACHED_FONT_INFO: OnceLock<FontInfo> = OnceLock::new();
        CACHED_FONT_INFO.get_or_init(|| unsafe {
            let factory: IDWriteFactory5 = DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED).unwrap();
            let render_params: IDWriteRenderingParams1 =
                factory.CreateRenderingParams().unwrap().cast().unwrap();
            FontInfo {
                gamma_ratios: open_gpui::get_gamma_correction_ratios(render_params.GetGamma()),
                grayscale_enhanced_contrast: render_params.GetGrayscaleEnhancedContrast(),
                subpixel_enhanced_contrast: render_params.GetEnhancedContrast(),
                is_bgr: render_params.GetPixelGeometry() == DWRITE_PIXEL_GEOMETRY_BGR,
            }
        })
    }

    pub(crate) fn quiesce_surface(
        &mut self,
        shutdown: &WindowPresentationShutdownTicket,
    ) -> Result<()> {
        if let Some(current) = self.surface_quiesced_for.as_ref() {
            if !current.same_authority(shutdown) {
                bail!("a stale presentation-shutdown generation cannot quiesce this surface");
            }
            shutdown.acknowledge_quiesced();
            return Ok(());
        }

        self.repaint_required_after_recovery = false;
        if let Some(devices) = self.devices.as_ref() {
            unsafe {
                devices.device_context.OMSetRenderTargets(None, None);
                devices.device_context.ClearState();
            }
            wait_for_gpu_idle(devices)?;
        }
        if let Some(composition) = self.direct_composition.as_ref() {
            composition.detach_surface()?;
        }
        self.resources.take();
        self.direct_composition.take();
        self.surface_quiesced_for = Some(shutdown.clone());
        anyhow::ensure!(
            shutdown.acknowledge_quiesced(),
            "native terminal preceded renderer quiescence"
        );
        Ok(())
    }

    pub(crate) fn is_quiesced_for(&self, shutdown: &WindowPresentationShutdownTicket) -> bool {
        self.surface_quiesced_for
            .as_ref()
            .is_some_and(|current| current.same_authority(shutdown))
            && self.resources.is_none()
            && self.direct_composition.is_none()
    }
}

fn wait_for_gpu_idle(devices: &DirectXRendererDevices) -> Result<()> {
    const GPU_IDLE_TIMEOUT: Duration = Duration::from_secs(2);
    let descriptor = D3D11_QUERY_DESC {
        Query: D3D11_QUERY_EVENT,
        MiscFlags: 0,
    };
    let mut query = None;
    unsafe {
        devices
            .device
            .CreateQuery(&descriptor, Some(&mut query))
            .context("creating DirectX presentation-shutdown query")?;
    }
    let query = query.context("DirectX presentation-shutdown query was not created")?;
    unsafe {
        devices.device_context.End(&query);
        devices.device_context.Flush();
    }

    let deadline = Instant::now() + GPU_IDLE_TIMEOUT;
    loop {
        let result = unsafe {
            (Interface::vtable(&devices.device_context).GetData)(
                Interface::as_raw(&devices.device_context),
                Interface::as_raw(&query),
                std::ptr::null_mut(),
                0,
                D3D11_ASYNC_GETDATA_DONOTFLUSH.0 as u32,
            )
        };
        if result == S_OK {
            return Ok(());
        }
        if result != S_FALSE {
            result
                .ok()
                .context("waiting for DirectX presentation work to quiesce")?;
        }
        if Instant::now() >= deadline {
            bail!("timed out waiting for DirectX presentation work to quiesce");
        }
        thread::yield_now();
    }
}

impl DirectXResources {
    pub fn new(
        devices: &DirectXRendererDevices,
        width: u32,
        height: u32,
        hwnd: HWND,
        disable_direct_composition: bool,
    ) -> Result<Self> {
        let swap_chain = if disable_direct_composition {
            create_swap_chain(&devices.dxgi_factory, &devices.device, hwnd, width, height)?
        } else {
            create_swap_chain_for_composition(
                &devices.dxgi_factory,
                &devices.device,
                width,
                height,
            )?
        };

        let (
            render_target,
            render_target_view,
            depth_stencil_texture,
            depth_stencil_view,
            viewport,
        ) = create_resources(devices, &swap_chain, width, height)?;
        set_rasterizer_state(&devices.device, &devices.device_context)?;

        Ok(Self {
            swap_chain,
            render_target: Some(render_target),
            render_target_view,
            depth_stencil_texture,
            depth_stencil_view,
            viewport,
        })
    }

    #[inline]
    fn recreate_resources(
        &mut self,
        devices: &DirectXRendererDevices,
        width: u32,
        height: u32,
    ) -> Result<()> {
        let (
            render_target,
            render_target_view,
            depth_stencil_texture,
            depth_stencil_view,
            viewport,
        ) = create_resources(devices, &self.swap_chain, width, height)?;
        self.render_target = Some(render_target);
        self.render_target_view = render_target_view;
        self.depth_stencil_texture = depth_stencil_texture;
        self.depth_stencil_view = depth_stencil_view;
        self.viewport = viewport;
        Ok(())
    }
}

impl DirectXRenderPipelines {
    pub fn new(device: &ID3D11Device) -> Result<Self> {
        let shadow_pipeline = PipelineState::new(
            device,
            "shadow_pipeline",
            ShaderModule::Shadow,
            4,
            create_blend_state(device)?,
        )?;
        let quad_pipeline = PipelineState::new(
            device,
            "quad_pipeline",
            ShaderModule::Quad,
            64,
            create_blend_state(device)?,
        )?;
        let opaque_quad_pipeline = OpaqueQuadPipeline::new(device)?;
        let path_tile_pipeline = PipelineState::new(
            device,
            "path_tile_pipeline",
            ShaderModule::PathTile,
            256,
            create_blend_state(device)?,
        )?;
        let path_curves = StructuredBuffer::new(device, "path_curves", 256)?;
        let path_paints = StructuredBuffer::new(device, "path_paints", 64)?;
        let underline_pipeline = PipelineState::new(
            device,
            "underline_pipeline",
            ShaderModule::Underline,
            4,
            create_blend_state(device)?,
        )?;
        let mono_sprites = PipelineState::new(
            device,
            "monochrome_sprite_pipeline",
            ShaderModule::MonochromeSprite,
            512,
            create_blend_state(device)?,
        )?;
        let subpixel_sprites = PipelineState::new(
            device,
            "subpixel_sprite_pipeline",
            ShaderModule::SubpixelSprite,
            512,
            create_blend_state_for_subpixel_rendering(device)?,
        )?;
        let poly_sprites = PipelineState::new(
            device,
            "polychrome_sprite_pipeline",
            ShaderModule::PolychromeSprite,
            16,
            create_blend_state(device)?,
        )?;

        Ok(Self {
            shadow_pipeline,
            quad_pipeline,
            opaque_quad_pipeline,
            path_tile_pipeline,
            path_curves,
            path_paints,
            underline_pipeline,
            mono_sprites,
            subpixel_sprites,
            poly_sprites,
        })
    }
}

impl DirectComposition {
    pub fn new(dxgi_device: &IDXGIDevice, hwnd: HWND) -> Result<Self> {
        let comp_device = get_comp_device(dxgi_device)?;
        let comp_target = unsafe { comp_device.CreateTargetForHwnd(hwnd, false) }?;
        let comp_visual = unsafe { comp_device.CreateVisual() }?;

        Ok(Self {
            comp_device,
            comp_target,
            comp_visual,
        })
    }

    pub fn set_swap_chain(&self, swap_chain: &IDXGISwapChain1) -> Result<()> {
        unsafe {
            self.comp_visual.SetContent(swap_chain)?;
            self.comp_target.SetRoot(&self.comp_visual)?;
            self.comp_device.Commit()?;
        }
        Ok(())
    }

    fn detach_surface(&self) -> Result<()> {
        unsafe {
            self.comp_visual.SetContent(None::<&IUnknown>)?;
            self.comp_target.SetRoot(None::<&IDCompositionVisual>)?;
            self.comp_device.Commit()?;
            self.comp_device.WaitForCommitCompletion()?;
        }
        Ok(())
    }
}

impl DirectXGlobalElements {
    pub fn new(device: &ID3D11Device) -> Result<Self> {
        let constant_buffers = [
            Some(create_constant_buffer::<GlobalParams>(device)?),
            Some(create_constant_buffer::<BatchParams>(device)?),
        ];

        let sampler = unsafe {
            let desc = D3D11_SAMPLER_DESC {
                Filter: D3D11_FILTER_MIN_MAG_MIP_LINEAR,
                AddressU: D3D11_TEXTURE_ADDRESS_WRAP,
                AddressV: D3D11_TEXTURE_ADDRESS_WRAP,
                AddressW: D3D11_TEXTURE_ADDRESS_WRAP,
                MipLODBias: 0.0,
                MaxAnisotropy: 1,
                ComparisonFunc: D3D11_COMPARISON_ALWAYS,
                BorderColor: [0.0; 4],
                MinLOD: 0.0,
                MaxLOD: D3D11_FLOAT32_MAX,
            };
            let mut output = None;
            device.CreateSamplerState(&desc, Some(&mut output))?;
            output
        };

        let clip_shapes_buffer = create_buffer(device, std::mem::size_of::<GpuClipShape>(), 1)
            .context("creating DirectX clip-shape buffer")?;
        let clip_shapes_view = create_buffer_view(device, &clip_shapes_buffer)
            .context("creating DirectX clip-shape buffer view")?;

        Ok(Self {
            constant_buffers,
            sampler,
            clip_shapes_buffer,
            clip_shapes_view,
            clip_shapes_capacity: 1,
        })
    }

    fn update_clip_shapes(
        &mut self,
        device: &ID3D11Device,
        device_context: &ID3D11DeviceContext,
        clip_shapes: &[GpuClipShape],
    ) -> Result<()> {
        if self.clip_shapes_capacity < clip_shapes.len() {
            let element_size = std::mem::size_of::<GpuClipShape>();
            let new_capacity =
                bounded_instance_capacity(element_size, clip_shapes.len(), "clip shapes")?;
            let buffer = create_buffer(device, element_size, new_capacity)
                .context("growing DirectX clip-shape buffer")?;
            let view = create_buffer_view(device, &buffer)
                .context("recreating DirectX clip-shape buffer view")?;
            self.clip_shapes_buffer = buffer;
            self.clip_shapes_view = view;
            self.clip_shapes_capacity = new_capacity;
        }

        if !clip_shapes.is_empty() {
            update_buffer(device_context, &self.clip_shapes_buffer, clip_shapes)?;
        }
        Ok(())
    }
}

#[derive(Debug, Default)]
#[repr(C)]
struct GlobalParams {
    gamma_ratios: [f32; 4],
    viewport_size: [f32; 2],
    grayscale_enhanced_contrast: f32,
    subpixel_enhanced_contrast: f32,
    is_bgr: u32,
    clip_shape_count: u32,
    depth_scale: f32,
    _pad: u32,
}

#[derive(Clone, Copy, Debug, Default)]
#[repr(C, align(16))]
struct BatchParams {
    start_index: u32,
    _padding: [u32; 3],
}

const _: () = assert!(std::mem::size_of::<BatchParams>() == 16);

struct StructuredBuffer<T> {
    label: &'static str,
    buffer: ID3D11Buffer,
    capacity: usize,
    view: Option<ID3D11ShaderResourceView>,
    _marker: std::marker::PhantomData<T>,
}

impl<T> StructuredBuffer<T> {
    fn new(device: &ID3D11Device, label: &'static str, capacity: usize) -> Result<Self> {
        let buffer = create_buffer(device, std::mem::size_of::<T>(), capacity)?;
        let view = create_buffer_view(device, &buffer)?;
        Ok(Self {
            label,
            buffer,
            capacity,
            view,
            _marker: std::marker::PhantomData,
        })
    }

    fn write(
        &mut self,
        device: &ID3D11Device,
        device_context: &ID3D11DeviceContext,
        data: &[T],
    ) -> Result<()> {
        if data.is_empty() {
            return Ok(());
        }
        if self.capacity < data.len() {
            let capacity =
                bounded_instance_capacity(std::mem::size_of::<T>(), data.len(), self.label)?;
            let buffer = create_buffer(device, std::mem::size_of::<T>(), capacity)?;
            let view = create_buffer_view(device, &buffer)?;
            self.buffer = buffer;
            self.view = view;
            self.capacity = capacity;
        }
        update_buffer(device_context, &self.buffer, data)
    }
}

struct PipelineState<T> {
    label: &'static str,
    vertex: ID3D11VertexShader,
    fragment: ID3D11PixelShader,
    buffer: ID3D11Buffer,
    buffer_size: usize,
    uploaded_count: usize,
    view: Option<ID3D11ShaderResourceView>,
    blend_state: ID3D11BlendState,
    depth_stencil_state: ID3D11DepthStencilState,
    _marker: std::marker::PhantomData<T>,
}

impl<T> PipelineState<T> {
    fn new(
        device: &ID3D11Device,
        label: &'static str,
        shader_module: ShaderModule,
        buffer_size: usize,
        blend_state: ID3D11BlendState,
    ) -> Result<Self> {
        let vertex = {
            let raw_shader = RawShaderBytes::new(shader_module, ShaderTarget::Vertex)?;
            create_vertex_shader(device, raw_shader.as_bytes())?
        };
        let fragment = {
            let raw_shader = RawShaderBytes::new(shader_module, ShaderTarget::Fragment)?;
            create_fragment_shader(device, raw_shader.as_bytes())?
        };
        let buffer = create_buffer(device, std::mem::size_of::<T>(), buffer_size)?;
        let view = create_buffer_view(device, &buffer)?;

        Ok(PipelineState {
            label,
            vertex,
            fragment,
            buffer,
            buffer_size,
            uploaded_count: 0,
            view,
            blend_state,
            depth_stencil_state: create_depth_stencil_state(device, D3D11_DEPTH_WRITE_MASK_ZERO)?,
            _marker: std::marker::PhantomData,
        })
    }

    fn update_buffer(
        &mut self,
        device: &ID3D11Device,
        device_context: &ID3D11DeviceContext,
        data: &[T],
    ) -> Result<()> {
        self.uploaded_count = 0;
        if data.is_empty() {
            return Ok(());
        }
        if self.buffer_size < data.len() {
            let new_buffer_size =
                bounded_instance_capacity(std::mem::size_of::<T>(), data.len(), self.label)?;
            log::debug!(
                "Updating {} buffer size from {} to {}",
                self.label,
                self.buffer_size,
                new_buffer_size
            );
            let buffer = create_buffer(device, std::mem::size_of::<T>(), new_buffer_size)?;
            let view = create_buffer_view(device, &buffer)?;
            self.buffer = buffer;
            self.view = view;
            self.buffer_size = new_buffer_size;
        }
        update_buffer(device_context, &self.buffer, data)?;
        self.uploaded_count = data.len();
        Ok(())
    }

    fn draw(
        &self,
        device_context: &ID3D11DeviceContext,
        topology: D3D_PRIMITIVE_TOPOLOGY,
        vertex_count: u32,
        instance_count: u32,
    ) {
        set_pipeline_state(
            device_context,
            slice::from_ref(&self.view),
            topology,
            &self.vertex,
            &self.fragment,
            &self.blend_state,
            &self.depth_stencil_state,
        );
        unsafe {
            device_context.DrawInstanced(vertex_count, instance_count, 0, 0);
        }
    }

    fn draw_with_texture(
        &self,
        device_context: &ID3D11DeviceContext,
        texture: &[Option<ID3D11ShaderResourceView>],
        sampler: &[Option<ID3D11SamplerState>],
        instance_count: u32,
    ) {
        unsafe {
            device_context.PSSetSamplers(0, Some(sampler));
            device_context.VSSetShaderResources(0, Some(texture));
            device_context.PSSetShaderResources(0, Some(texture));
        }
        self.draw(
            device_context,
            D3D_PRIMITIVE_TOPOLOGY_TRIANGLESTRIP,
            4,
            instance_count,
        );
    }

    fn draw_range(
        &self,
        device_context: &ID3D11DeviceContext,
        batch_params: &ID3D11Buffer,
        range: Range<usize>,
        texture: Option<(
            &[Option<ID3D11ShaderResourceView>],
            &[Option<ID3D11SamplerState>],
        )>,
    ) -> Result<()> {
        anyhow::ensure!(
            range.start <= range.end && range.end <= self.uploaded_count,
            "DirectX instance range exceeds uploaded {} data",
            self.label,
        );
        if range.is_empty() {
            return Ok(());
        }
        let start_index = u32::try_from(range.start).context("DirectX batch offset overflow")?;
        let instance_count = u32::try_from(range.len()).context("DirectX batch count overflow")?;
        update_buffer(
            device_context,
            batch_params,
            &[BatchParams {
                start_index,
                _padding: [0; 3],
            }],
        )?;
        if let Some((view, sampler)) = texture {
            self.draw_with_texture(device_context, view, sampler, instance_count);
        } else {
            self.draw(
                device_context,
                D3D_PRIMITIVE_TOPOLOGY_TRIANGLESTRIP,
                4,
                instance_count,
            );
        }
        Ok(())
    }
}

struct OpaqueQuadPipeline {
    vertex: ID3D11VertexShader,
    fragment: ID3D11PixelShader,
    blend_state: ID3D11BlendState,
    depth_stencil_state: ID3D11DepthStencilState,
}

impl OpaqueQuadPipeline {
    fn new(device: &ID3D11Device) -> Result<Self> {
        let vertex_bytes = RawShaderBytes::new(ShaderModule::OpaqueQuad, ShaderTarget::Vertex)?;
        let fragment_bytes = RawShaderBytes::new(ShaderModule::OpaqueQuad, ShaderTarget::Fragment)?;
        Ok(Self {
            vertex: create_vertex_shader(device, vertex_bytes.as_bytes())?,
            fragment: create_fragment_shader(device, fragment_bytes.as_bytes())?,
            blend_state: create_opaque_blend_state(device)?,
            depth_stencil_state: create_depth_stencil_state(device, D3D11_DEPTH_WRITE_MASK_ALL)?,
        })
    }

    fn draw(
        &self,
        device_context: &ID3D11DeviceContext,
        quad_pipeline: &PipelineState<Quad>,
        batch_params: &ID3D11Buffer,
        quad_count: usize,
    ) -> Result<()> {
        anyhow::ensure!(
            quad_count <= quad_pipeline.uploaded_count,
            "DirectX opaque quad range exceeds uploaded data",
        );
        if quad_count == 0 {
            return Ok(());
        }
        let quad_count = u32::try_from(quad_count).context("DirectX opaque quad count overflow")?;
        update_buffer(
            device_context,
            batch_params,
            &[BatchParams {
                start_index: quad_count - 1,
                _padding: [0; 3],
            }],
        )?;
        set_pipeline_state(
            device_context,
            slice::from_ref(&quad_pipeline.view),
            D3D_PRIMITIVE_TOPOLOGY_TRIANGLESTRIP,
            &self.vertex,
            &self.fragment,
            &self.blend_state,
            &self.depth_stencil_state,
        );
        unsafe { device_context.DrawInstanced(4, quad_count, 0, 0) };
        Ok(())
    }
}

const _: () = {
    assert!(std::mem::size_of::<PrimitiveTransform>() == 16);
    const IDENTITY_COMPONENTS: [f32; 4] = PrimitiveTransform::IDENTITY.components();
    assert!(IDENTITY_COMPONENTS[0].to_bits() == 0x3f80_0000);
    assert!(IDENTITY_COMPONENTS[1].to_bits() == 0x3f80_0000);
    assert!(IDENTITY_COMPONENTS[2].to_bits() == 0);
    assert!(IDENTITY_COMPONENTS[3].to_bits() == 0);

    assert!(std::mem::size_of::<Quad>() == 184);
    assert!(std::mem::size_of::<Shadow>() == 136);
    assert!(std::mem::size_of::<Underline>() == 88);
    assert!(std::mem::size_of::<MonochromeSprite>() == 120);
    assert!(std::mem::size_of::<SubpixelSprite>() == 120);
    assert!(std::mem::size_of::<PolychromeSprite>() == 128);
    assert!(std::mem::size_of::<PathCurve>() == 40);
    assert!(std::mem::offset_of!(PathCurve, sign) == 32);
    assert!(std::mem::size_of::<PathTile>() == 56);
    assert!(std::mem::offset_of!(PathTile, top_left) == 16);
    assert!(std::mem::offset_of!(PathTile, curves_bottom_right) == 48);
    assert!(std::mem::size_of::<PathPaint>() == 136);
    assert!(std::mem::offset_of!(PathPaint, bounds) == 0);
    assert!(std::mem::offset_of!(PathPaint, clip) == 16);
    assert!(std::mem::offset_of!(PathPaint, color) == 40);
    assert!(std::mem::offset_of!(PathPaint, even_odd) == 112);
    assert!(std::mem::offset_of!(PathPaint, pad) == 116);
    assert!(std::mem::offset_of!(PathPaint, transform) == 120);

    assert!(std::mem::align_of::<GpuClipShape>() == std::mem::align_of::<f32>());
    assert!(std::mem::size_of::<GpuClipShape>() == 48);
    assert!(std::mem::offset_of!(GpuClipShape, bounds) == 0);
    assert!(std::mem::offset_of!(GpuClipShape, radii_x) == 16);
    assert!(std::mem::offset_of!(GpuClipShape, radii_y) == 32);
    assert!(std::mem::align_of::<ClipEnvelope>() == std::mem::align_of::<f32>());
    assert!(std::mem::size_of::<ClipEnvelope>() == 24);
    assert!(std::mem::offset_of!(ClipEnvelope, conservative_bounds) == 0);
    assert!(std::mem::offset_of!(ClipEnvelope, first_clip) == 16);
    assert!(std::mem::offset_of!(ClipEnvelope, clip_count) == 20);
    assert!(CLIP_SHAPES_SHADER_RESOURCE_SLOT == 2);
    assert!(PATH_CURVES_SHADER_RESOURCE_SLOT == 3);

    assert!(std::mem::align_of::<GlobalParams>() == std::mem::align_of::<f32>());
    assert!(std::mem::size_of::<GlobalParams>() == 48);
    assert!(std::mem::offset_of!(GlobalParams, is_bgr) == 32);
    assert!(std::mem::offset_of!(GlobalParams, clip_shape_count) == 36);
    assert!(std::mem::offset_of!(GlobalParams, depth_scale) == 40);

    assert!(std::mem::offset_of!(MonochromeSprite, tile) == 8);
    assert!(std::mem::offset_of!(MonochromeSprite, bounds) == 48);
    assert!(std::mem::offset_of!(MonochromeSprite, clip) == 64);
    assert!(std::mem::offset_of!(MonochromeSprite, color) == 88);
    assert!(std::mem::offset_of!(SubpixelSprite, tile) == 8);
    assert!(std::mem::offset_of!(SubpixelSprite, bounds) == 48);
    assert!(std::mem::offset_of!(SubpixelSprite, clip) == 64);
    assert!(std::mem::offset_of!(SubpixelSprite, color) == 88);
    assert!(std::mem::offset_of!(PolychromeSprite, tile) == 8);
    assert!(std::mem::offset_of!(PolychromeSprite, grayscale) == 48);
    assert!(std::mem::offset_of!(PolychromeSprite, opacity) == 52);
    assert!(std::mem::offset_of!(PolychromeSprite, bounds) == 56);
    assert!(std::mem::offset_of!(PolychromeSprite, clip) == 72);
    assert!(std::mem::offset_of!(PolychromeSprite, corner_radii) == 96);
};

impl Drop for DirectXRenderer {
    fn drop(&mut self) {
        #[cfg(debug_assertions)]
        if let Some(devices) = &self.devices {
            report_live_objects(&devices.device).ok();
        }
    }
}

#[inline]
fn get_comp_device(dxgi_device: &IDXGIDevice) -> Result<IDCompositionDevice> {
    Ok(unsafe { DCompositionCreateDevice(dxgi_device)? })
}

fn create_swap_chain_for_composition(
    dxgi_factory: &IDXGIFactory6,
    device: &ID3D11Device,
    width: u32,
    height: u32,
) -> Result<IDXGISwapChain1> {
    let desc = DXGI_SWAP_CHAIN_DESC1 {
        Width: width,
        Height: height,
        Format: RENDER_TARGET_FORMAT,
        Stereo: false.into(),
        SampleDesc: DXGI_SAMPLE_DESC {
            Count: 1,
            Quality: 0,
        },
        BufferUsage: DXGI_USAGE_RENDER_TARGET_OUTPUT,
        BufferCount: BUFFER_COUNT as u32,
        // Composition SwapChains only support the DXGI_SCALING_STRETCH Scaling.
        Scaling: DXGI_SCALING_STRETCH,
        SwapEffect: DXGI_SWAP_EFFECT_FLIP_SEQUENTIAL,
        AlphaMode: DXGI_ALPHA_MODE_PREMULTIPLIED,
        Flags: 0,
    };
    Ok(unsafe { dxgi_factory.CreateSwapChainForComposition(device, &desc, None)? })
}

fn create_swap_chain(
    dxgi_factory: &IDXGIFactory6,
    device: &ID3D11Device,
    hwnd: HWND,
    width: u32,
    height: u32,
) -> Result<IDXGISwapChain1> {
    use windows::Win32::Graphics::Dxgi::DXGI_MWA_NO_ALT_ENTER;

    let desc = DXGI_SWAP_CHAIN_DESC1 {
        Width: width,
        Height: height,
        Format: RENDER_TARGET_FORMAT,
        Stereo: false.into(),
        SampleDesc: DXGI_SAMPLE_DESC {
            Count: 1,
            Quality: 0,
        },
        BufferUsage: DXGI_USAGE_RENDER_TARGET_OUTPUT,
        BufferCount: BUFFER_COUNT as u32,
        Scaling: DXGI_SCALING_NONE,
        SwapEffect: DXGI_SWAP_EFFECT_FLIP_SEQUENTIAL,
        AlphaMode: DXGI_ALPHA_MODE_IGNORE,
        Flags: 0,
    };
    let swap_chain =
        unsafe { dxgi_factory.CreateSwapChainForHwnd(device, hwnd, &desc, None, None) }?;
    unsafe { dxgi_factory.MakeWindowAssociation(hwnd, DXGI_MWA_NO_ALT_ENTER) }?;
    Ok(swap_chain)
}

#[inline]
fn create_resources(
    devices: &DirectXRendererDevices,
    swap_chain: &IDXGISwapChain1,
    width: u32,
    height: u32,
) -> Result<(
    ID3D11Texture2D,
    Option<ID3D11RenderTargetView>,
    ID3D11Texture2D,
    Option<ID3D11DepthStencilView>,
    D3D11_VIEWPORT,
)> {
    let (render_target, render_target_view) =
        create_render_target_and_its_view(swap_chain, &devices.device)?;
    let (depth_stencil_texture, depth_stencil_view) =
        create_depth_stencil_texture_and_view(&devices.device, width, height)?;
    let viewport = set_viewport(&devices.device_context, width as f32, height as f32);
    Ok((
        render_target,
        render_target_view,
        depth_stencil_texture,
        depth_stencil_view,
        viewport,
    ))
}

#[inline]
fn create_render_target_and_its_view(
    swap_chain: &IDXGISwapChain1,
    device: &ID3D11Device,
) -> Result<(ID3D11Texture2D, Option<ID3D11RenderTargetView>)> {
    let render_target: ID3D11Texture2D = unsafe { swap_chain.GetBuffer(0) }?;
    let mut render_target_view = None;
    unsafe { device.CreateRenderTargetView(&render_target, None, Some(&mut render_target_view))? };
    Ok((render_target, render_target_view))
}

#[inline]
fn create_depth_stencil_texture_and_view(
    device: &ID3D11Device,
    width: u32,
    height: u32,
) -> Result<(ID3D11Texture2D, Option<ID3D11DepthStencilView>)> {
    let texture = unsafe {
        let mut output = None;
        let desc = D3D11_TEXTURE2D_DESC {
            Width: width,
            Height: height,
            MipLevels: 1,
            ArraySize: 1,
            Format: DXGI_FORMAT_D32_FLOAT,
            SampleDesc: DXGI_SAMPLE_DESC {
                Count: 1,
                Quality: 0,
            },
            Usage: D3D11_USAGE_DEFAULT,
            BindFlags: D3D11_BIND_DEPTH_STENCIL.0 as u32,
            CPUAccessFlags: 0,
            MiscFlags: 0,
        };
        device.CreateTexture2D(&desc, None, Some(&mut output))?;
        output.context("DirectX did not return a depth texture")?
    };

    let mut view = None;
    unsafe { device.CreateDepthStencilView(&texture, None, Some(&mut view))? };
    anyhow::ensure!(
        view.is_some(),
        "DirectX did not return a depth stencil view"
    );
    Ok((texture, view))
}

#[inline]
fn set_viewport(device_context: &ID3D11DeviceContext, width: f32, height: f32) -> D3D11_VIEWPORT {
    let viewport = [D3D11_VIEWPORT {
        TopLeftX: 0.0,
        TopLeftY: 0.0,
        Width: width,
        Height: height,
        MinDepth: 0.0,
        MaxDepth: 1.0,
    }];
    unsafe { device_context.RSSetViewports(Some(&viewport)) };
    viewport[0]
}

#[inline]
fn set_rasterizer_state(device: &ID3D11Device, device_context: &ID3D11DeviceContext) -> Result<()> {
    let desc = D3D11_RASTERIZER_DESC {
        FillMode: D3D11_FILL_SOLID,
        CullMode: D3D11_CULL_NONE,
        FrontCounterClockwise: false.into(),
        DepthBias: 0,
        DepthBiasClamp: 0.0,
        SlopeScaledDepthBias: 0.0,
        DepthClipEnable: true.into(),
        ScissorEnable: false.into(),
        MultisampleEnable: true.into(),
        AntialiasedLineEnable: false.into(),
    };
    let rasterizer_state = unsafe {
        let mut state = None;
        device.CreateRasterizerState(&desc, Some(&mut state))?;
        state.unwrap()
    };
    unsafe { device_context.RSSetState(&rasterizer_state) };
    Ok(())
}

// https://learn.microsoft.com/en-us/windows/win32/api/d3d11/ns-d3d11-d3d11_blend_desc
#[inline]
fn create_blend_state(device: &ID3D11Device) -> Result<ID3D11BlendState> {
    let mut desc = D3D11_BLEND_DESC::default();
    desc.RenderTarget[0].BlendEnable = true.into();
    desc.RenderTarget[0].BlendOp = D3D11_BLEND_OP_ADD;
    desc.RenderTarget[0].BlendOpAlpha = D3D11_BLEND_OP_ADD;
    desc.RenderTarget[0].SrcBlend = D3D11_BLEND_SRC_ALPHA;
    desc.RenderTarget[0].SrcBlendAlpha = D3D11_BLEND_ONE;
    desc.RenderTarget[0].DestBlend = D3D11_BLEND_INV_SRC_ALPHA;
    desc.RenderTarget[0].DestBlendAlpha = D3D11_BLEND_INV_SRC_ALPHA;
    desc.RenderTarget[0].RenderTargetWriteMask = D3D11_COLOR_WRITE_ENABLE_ALL.0 as u8;
    unsafe {
        let mut state = None;
        device.CreateBlendState(&desc, Some(&mut state))?;
        Ok(state.unwrap())
    }
}

#[inline]
fn create_blend_state_for_subpixel_rendering(device: &ID3D11Device) -> Result<ID3D11BlendState> {
    let mut desc = D3D11_BLEND_DESC::default();
    desc.RenderTarget[0].BlendEnable = true.into();
    desc.RenderTarget[0].BlendOp = D3D11_BLEND_OP_ADD;
    desc.RenderTarget[0].BlendOpAlpha = D3D11_BLEND_OP_ADD;
    desc.RenderTarget[0].SrcBlend = D3D11_BLEND_SRC1_COLOR;
    desc.RenderTarget[0].DestBlend = D3D11_BLEND_INV_SRC1_COLOR;
    // It does not make sense to draw transparent subpixel-rendered text, since it cannot be meaningfully alpha-blended onto anything else.
    desc.RenderTarget[0].SrcBlendAlpha = D3D11_BLEND_ONE;
    desc.RenderTarget[0].DestBlendAlpha = D3D11_BLEND_ZERO;
    desc.RenderTarget[0].RenderTargetWriteMask =
        D3D11_COLOR_WRITE_ENABLE_ALL.0 as u8 & !D3D11_COLOR_WRITE_ENABLE_ALPHA.0 as u8;

    unsafe {
        let mut state = None;
        device.CreateBlendState(&desc, Some(&mut state))?;
        Ok(state.unwrap())
    }
}

#[inline]
fn create_opaque_blend_state(device: &ID3D11Device) -> Result<ID3D11BlendState> {
    let mut desc = D3D11_BLEND_DESC::default();
    desc.RenderTarget[0].BlendEnable = false.into();
    desc.RenderTarget[0].BlendOp = D3D11_BLEND_OP_ADD;
    desc.RenderTarget[0].BlendOpAlpha = D3D11_BLEND_OP_ADD;
    desc.RenderTarget[0].SrcBlend = D3D11_BLEND_ONE;
    desc.RenderTarget[0].SrcBlendAlpha = D3D11_BLEND_ONE;
    desc.RenderTarget[0].DestBlend = D3D11_BLEND_ZERO;
    desc.RenderTarget[0].DestBlendAlpha = D3D11_BLEND_ZERO;
    desc.RenderTarget[0].RenderTargetWriteMask = D3D11_COLOR_WRITE_ENABLE_ALL.0 as u8;
    unsafe {
        let mut state = None;
        device.CreateBlendState(&desc, Some(&mut state))?;
        Ok(state.unwrap())
    }
}

#[inline]
fn create_depth_stencil_state(
    device: &ID3D11Device,
    write_mask: D3D11_DEPTH_WRITE_MASK,
) -> Result<ID3D11DepthStencilState> {
    let stencil_op = D3D11_DEPTH_STENCILOP_DESC {
        StencilFailOp: D3D11_STENCIL_OP_KEEP,
        StencilDepthFailOp: D3D11_STENCIL_OP_KEEP,
        StencilPassOp: D3D11_STENCIL_OP_KEEP,
        StencilFunc: D3D11_COMPARISON_ALWAYS,
    };
    let desc = D3D11_DEPTH_STENCIL_DESC {
        DepthEnable: true.into(),
        DepthWriteMask: write_mask,
        DepthFunc: D3D11_COMPARISON_GREATER,
        StencilEnable: false.into(),
        StencilReadMask: D3D11_DEFAULT_STENCIL_READ_MASK as u8,
        StencilWriteMask: D3D11_DEFAULT_STENCIL_WRITE_MASK as u8,
        FrontFace: stencil_op,
        BackFace: stencil_op,
    };
    unsafe {
        let mut state = None;
        device.CreateDepthStencilState(&desc, Some(&mut state))?;
        state.context("DirectX did not return a depth stencil state")
    }
}

#[inline]
fn create_vertex_shader(device: &ID3D11Device, bytes: &[u8]) -> Result<ID3D11VertexShader> {
    unsafe {
        let mut shader = None;
        device.CreateVertexShader(bytes, None, Some(&mut shader))?;
        Ok(shader.unwrap())
    }
}

#[inline]
fn create_fragment_shader(device: &ID3D11Device, bytes: &[u8]) -> Result<ID3D11PixelShader> {
    unsafe {
        let mut shader = None;
        device.CreatePixelShader(bytes, None, Some(&mut shader))?;
        Ok(shader.unwrap())
    }
}

#[inline]
fn create_constant_buffer<T>(device: &ID3D11Device) -> Result<ID3D11Buffer> {
    let desc = D3D11_BUFFER_DESC {
        ByteWidth: std::mem::size_of::<T>() as u32,
        Usage: D3D11_USAGE_DYNAMIC,
        BindFlags: D3D11_BIND_CONSTANT_BUFFER.0 as u32,
        CPUAccessFlags: D3D11_CPU_ACCESS_WRITE.0 as u32,
        ..Default::default()
    };
    let mut buffer = None;
    unsafe { device.CreateBuffer(&desc, None, Some(&mut buffer)) }?;
    buffer.context("DirectX did not return a constant buffer")
}

fn bounded_instance_capacity(element_size: usize, required: usize, label: &str) -> Result<usize> {
    let maximum = MAX_INSTANCE_BUFFER_SIZE
        .checked_div(element_size)
        .context("DirectX instance buffer element size must be nonzero")?;
    anyhow::ensure!(
        required <= maximum,
        "{label} buffer needs {required} elements of {element_size} bytes, above the {MAX_INSTANCE_BUFFER_SIZE}-byte maximum",
    );
    Ok(required.next_power_of_two().min(maximum))
}

#[inline]
fn create_buffer(
    device: &ID3D11Device,
    element_size: usize,
    buffer_size: usize,
) -> Result<ID3D11Buffer> {
    let byte_width = element_size
        .checked_mul(buffer_size)
        .filter(|size| (1..=MAX_INSTANCE_BUFFER_SIZE).contains(size))
        .context("DirectX structured buffer byte width is outside its bounds")?;
    let desc = D3D11_BUFFER_DESC {
        ByteWidth: u32::try_from(byte_width).context("DirectX buffer byte width overflow")?,
        Usage: D3D11_USAGE_DYNAMIC,
        BindFlags: D3D11_BIND_SHADER_RESOURCE.0 as u32,
        CPUAccessFlags: D3D11_CPU_ACCESS_WRITE.0 as u32,
        MiscFlags: D3D11_RESOURCE_MISC_BUFFER_STRUCTURED.0 as u32,
        StructureByteStride: u32::try_from(element_size)
            .context("DirectX buffer stride overflow")?,
    };
    let mut buffer = None;
    unsafe { device.CreateBuffer(&desc, None, Some(&mut buffer)) }?;
    buffer.context("DirectX did not return a structured buffer")
}

#[inline]
fn create_buffer_view(
    device: &ID3D11Device,
    buffer: &ID3D11Buffer,
) -> Result<Option<ID3D11ShaderResourceView>> {
    let mut view = None;
    unsafe { device.CreateShaderResourceView(buffer, None, Some(&mut view)) }?;
    Ok(view)
}

#[inline]
fn update_buffer<T>(
    device_context: &ID3D11DeviceContext,
    buffer: &ID3D11Buffer,
    data: &[T],
) -> Result<()> {
    unsafe {
        let mut dest = std::mem::zeroed();
        device_context.Map(buffer, 0, D3D11_MAP_WRITE_DISCARD, 0, Some(&mut dest))?;
        std::ptr::copy_nonoverlapping(data.as_ptr(), dest.pData as _, data.len());
        device_context.Unmap(buffer, 0);
    }
    Ok(())
}

#[inline]
fn set_pipeline_state(
    device_context: &ID3D11DeviceContext,
    buffer_view: &[Option<ID3D11ShaderResourceView>],
    topology: D3D_PRIMITIVE_TOPOLOGY,
    vertex_shader: &ID3D11VertexShader,
    fragment_shader: &ID3D11PixelShader,
    blend_state: &ID3D11BlendState,
    depth_stencil_state: &ID3D11DepthStencilState,
) {
    unsafe {
        device_context.VSSetShaderResources(1, Some(buffer_view));
        device_context.PSSetShaderResources(1, Some(buffer_view));
        device_context.IASetPrimitiveTopology(topology);
        device_context.VSSetShader(vertex_shader, None);
        device_context.PSSetShader(fragment_shader, None);
        device_context.OMSetBlendState(blend_state, None, 0xFFFFFFFF);
        device_context.OMSetDepthStencilState(depth_stencil_state, 0);
    }
}

#[cfg(debug_assertions)]
fn report_live_objects(device: &ID3D11Device) -> Result<()> {
    let debug_device: ID3D11Debug = device.cast()?;
    unsafe {
        debug_device.ReportLiveDeviceObjects(D3D11_RLDO_DETAIL)?;
    }
    Ok(())
}

const BUFFER_COUNT: usize = 3;

pub(crate) mod shader_resources {
    use anyhow::Result;

    #[cfg(debug_assertions)]
    use windows::{
        Win32::Graphics::Direct3D::{
            Fxc::{D3DCOMPILE_DEBUG, D3DCOMPILE_SKIP_OPTIMIZATION, D3DCompileFromFile},
            ID3DBlob,
        },
        core::{HSTRING, PCSTR},
    };

    #[derive(Copy, Clone, Debug, Eq, PartialEq)]
    pub(crate) enum ShaderModule {
        Quad,
        OpaqueQuad,
        Shadow,
        Underline,
        PathTile,
        MonochromeSprite,
        SubpixelSprite,
        PolychromeSprite,
        EmojiRasterization,
    }

    #[derive(Copy, Clone, Debug, Eq, PartialEq)]
    pub(crate) enum ShaderTarget {
        Vertex,
        Fragment,
    }

    pub(crate) struct RawShaderBytes<'t> {
        inner: &'t [u8],

        #[cfg(debug_assertions)]
        _blob: ID3DBlob,
    }

    impl<'t> RawShaderBytes<'t> {
        pub(crate) fn new(module: ShaderModule, target: ShaderTarget) -> Result<Self> {
            #[cfg(not(debug_assertions))]
            {
                Ok(Self::from_bytes(module, target))
            }
            #[cfg(debug_assertions)]
            {
                let blob = build_shader_blob(module, target)?;
                let inner = unsafe {
                    std::slice::from_raw_parts(
                        blob.GetBufferPointer() as *const u8,
                        blob.GetBufferSize(),
                    )
                };
                Ok(Self { inner, _blob: blob })
            }
        }

        pub(crate) fn as_bytes(&'t self) -> &'t [u8] {
            self.inner
        }

        #[cfg(not(debug_assertions))]
        fn from_bytes(module: ShaderModule, target: ShaderTarget) -> Self {
            let bytes = match module {
                ShaderModule::Quad => match target {
                    ShaderTarget::Vertex => QUAD_VERTEX_BYTES,
                    ShaderTarget::Fragment => QUAD_FRAGMENT_BYTES,
                },
                ShaderModule::OpaqueQuad => match target {
                    ShaderTarget::Vertex => OPAQUE_QUAD_VERTEX_BYTES,
                    ShaderTarget::Fragment => OPAQUE_QUAD_FRAGMENT_BYTES,
                },
                ShaderModule::Shadow => match target {
                    ShaderTarget::Vertex => SHADOW_VERTEX_BYTES,
                    ShaderTarget::Fragment => SHADOW_FRAGMENT_BYTES,
                },
                ShaderModule::Underline => match target {
                    ShaderTarget::Vertex => UNDERLINE_VERTEX_BYTES,
                    ShaderTarget::Fragment => UNDERLINE_FRAGMENT_BYTES,
                },
                ShaderModule::PathTile => match target {
                    ShaderTarget::Vertex => PATH_TILE_VERTEX_BYTES,
                    ShaderTarget::Fragment => PATH_TILE_FRAGMENT_BYTES,
                },
                ShaderModule::MonochromeSprite => match target {
                    ShaderTarget::Vertex => MONOCHROME_SPRITE_VERTEX_BYTES,
                    ShaderTarget::Fragment => MONOCHROME_SPRITE_FRAGMENT_BYTES,
                },
                ShaderModule::SubpixelSprite => match target {
                    ShaderTarget::Vertex => SUBPIXEL_SPRITE_VERTEX_BYTES,
                    ShaderTarget::Fragment => SUBPIXEL_SPRITE_FRAGMENT_BYTES,
                },
                ShaderModule::PolychromeSprite => match target {
                    ShaderTarget::Vertex => POLYCHROME_SPRITE_VERTEX_BYTES,
                    ShaderTarget::Fragment => POLYCHROME_SPRITE_FRAGMENT_BYTES,
                },
                ShaderModule::EmojiRasterization => match target {
                    ShaderTarget::Vertex => EMOJI_RASTERIZATION_VERTEX_BYTES,
                    ShaderTarget::Fragment => EMOJI_RASTERIZATION_FRAGMENT_BYTES,
                },
            };
            Self { inner: bytes }
        }
    }

    #[cfg(debug_assertions)]
    pub(super) fn build_shader_blob(entry: ShaderModule, target: ShaderTarget) -> Result<ID3DBlob> {
        unsafe {
            use windows::Win32::Graphics::{
                Direct3D::ID3DInclude, Hlsl::D3D_COMPILE_STANDARD_FILE_INCLUDE,
            };

            let shader_name = if matches!(entry, ShaderModule::EmojiRasterization) {
                "color_text_raster.hlsl"
            } else {
                "shaders.hlsl"
            };

            let entry = format!(
                "{}_{}\0",
                entry.as_str(),
                match target {
                    ShaderTarget::Vertex => "vertex",
                    ShaderTarget::Fragment => "fragment",
                }
            );
            let target = match target {
                ShaderTarget::Vertex => "vs_4_1\0",
                ShaderTarget::Fragment => "ps_4_1\0",
            };

            let mut compile_blob = None;
            let mut error_blob = None;
            let shader_path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join(&format!("src/{}", shader_name))
                .canonicalize()?;

            let entry_point = PCSTR::from_raw(entry.as_ptr());
            let target_cstr = PCSTR::from_raw(target.as_ptr());

            // really dirty trick because winapi bindings are unhappy otherwise
            let include_handler = &std::mem::transmute::<usize, ID3DInclude>(
                D3D_COMPILE_STANDARD_FILE_INCLUDE as usize,
            );

            let ret = D3DCompileFromFile(
                &HSTRING::from(shader_path.to_str().unwrap()),
                None,
                include_handler,
                entry_point,
                target_cstr,
                D3DCOMPILE_DEBUG | D3DCOMPILE_SKIP_OPTIMIZATION,
                0,
                &mut compile_blob,
                Some(&mut error_blob),
            );
            if ret.is_err() {
                let Some(error_blob) = error_blob else {
                    return Err(anyhow::anyhow!("{ret:?}"));
                };

                let error_string =
                    std::ffi::CStr::from_ptr(error_blob.GetBufferPointer() as *const i8)
                        .to_string_lossy();
                log::error!("Shader compile error: {}", error_string);
                return Err(anyhow::anyhow!("Compile error: {}", error_string));
            }
            Ok(compile_blob.unwrap())
        }
    }

    #[cfg(not(debug_assertions))]
    include!(concat!(env!("OUT_DIR"), "/shaders_bytes.rs"));

    #[cfg(debug_assertions)]
    impl ShaderModule {
        pub fn as_str(self) -> &'static str {
            match self {
                ShaderModule::Quad => "quad",
                ShaderModule::OpaqueQuad => "opaque_quad",
                ShaderModule::Shadow => "shadow",
                ShaderModule::Underline => "underline",
                ShaderModule::PathTile => "path_tile",
                ShaderModule::MonochromeSprite => "monochrome_sprite",
                ShaderModule::SubpixelSprite => "subpixel_sprite",
                ShaderModule::PolychromeSprite => "polychrome_sprite",
                ShaderModule::EmojiRasterization => "emoji_rasterization",
            }
        }
    }
}

mod nvidia {
    use std::{
        ffi::CStr,
        os::raw::{c_char, c_int, c_uint},
    };

    use anyhow::Result;
    use windows::{Win32::System::LibraryLoader::GetProcAddress, core::s};

    use crate::with_dll_library;

    // https://github.com/NVIDIA/nvapi/blob/7cb76fce2f52de818b3da497af646af1ec16ce27/nvapi_lite_common.h#L180
    const NVAPI_SHORT_STRING_MAX: usize = 64;

    // https://github.com/NVIDIA/nvapi/blob/7cb76fce2f52de818b3da497af646af1ec16ce27/nvapi_lite_common.h#L235
    #[allow(non_camel_case_types)]
    type NvAPI_ShortString = [c_char; NVAPI_SHORT_STRING_MAX];

    // https://github.com/NVIDIA/nvapi/blob/7cb76fce2f52de818b3da497af646af1ec16ce27/nvapi_lite_common.h#L447
    #[allow(non_camel_case_types)]
    type NvAPI_SYS_GetDriverAndBranchVersion_t = unsafe extern "C" fn(
        driver_version: *mut c_uint,
        build_branch_string: *mut NvAPI_ShortString,
    ) -> c_int;

    pub(super) fn get_driver_version() -> Result<String> {
        #[cfg(target_pointer_width = "64")]
        let nvidia_dll_name = s!("nvapi64.dll");
        #[cfg(target_pointer_width = "32")]
        let nvidia_dll_name = s!("nvapi.dll");

        with_dll_library(nvidia_dll_name, |nvidia_dll| unsafe {
            let nvapi_query_addr = GetProcAddress(nvidia_dll, s!("nvapi_QueryInterface"))
                .ok_or_else(|| anyhow::anyhow!("Failed to get nvapi_QueryInterface address"))?;
            let nvapi_query: extern "C" fn(u32) -> *mut () = std::mem::transmute(nvapi_query_addr);

            // https://github.com/NVIDIA/nvapi/blob/7cb76fce2f52de818b3da497af646af1ec16ce27/nvapi_interface.h#L41
            let nvapi_get_driver_version_ptr = nvapi_query(0x2926aaad);
            if nvapi_get_driver_version_ptr.is_null() {
                anyhow::bail!("Failed to get NVIDIA driver version function pointer");
            }
            let nvapi_get_driver_version: NvAPI_SYS_GetDriverAndBranchVersion_t =
                std::mem::transmute(nvapi_get_driver_version_ptr);

            let mut driver_version: c_uint = 0;
            let mut build_branch_string: NvAPI_ShortString = [0; NVAPI_SHORT_STRING_MAX];
            let result = nvapi_get_driver_version(
                &mut driver_version as *mut c_uint,
                &mut build_branch_string as *mut NvAPI_ShortString,
            );

            if result != 0 {
                anyhow::bail!(
                    "Failed to get NVIDIA driver version, error code: {}",
                    result
                );
            }
            let major = driver_version / 100;
            let minor = driver_version % 100;
            let branch_string = CStr::from_ptr(build_branch_string.as_ptr());
            Ok(format!(
                "{}.{} {}",
                major,
                minor,
                branch_string.to_string_lossy()
            ))
        })
    }
}

mod amd {
    use std::os::raw::{c_char, c_int, c_void};

    use anyhow::Result;
    use windows::{Win32::System::LibraryLoader::GetProcAddress, core::s};

    use crate::with_dll_library;

    // https://github.com/GPUOpen-LibrariesAndSDKs/AGS_SDK/blob/5d8812d703d0335741b6f7ffc37838eeb8b967f7/ags_lib/inc/amd_ags.h#L145
    const AGS_CURRENT_VERSION: i32 = (6 << 22) | (3 << 12);

    // https://github.com/GPUOpen-LibrariesAndSDKs/AGS_SDK/blob/5d8812d703d0335741b6f7ffc37838eeb8b967f7/ags_lib/inc/amd_ags.h#L204
    // This is an opaque type, using struct to represent it properly for FFI
    #[repr(C)]
    struct AGSContext {
        _private: [u8; 0],
    }

    #[repr(C)]
    pub struct AGSGPUInfo {
        pub driver_version: *const c_char,
        pub radeon_software_version: *const c_char,
        pub num_devices: c_int,
        pub devices: *mut c_void,
    }

    // https://github.com/GPUOpen-LibrariesAndSDKs/AGS_SDK/blob/5d8812d703d0335741b6f7ffc37838eeb8b967f7/ags_lib/inc/amd_ags.h#L429
    #[allow(non_camel_case_types)]
    type agsInitialize_t = unsafe extern "C" fn(
        version: c_int,
        config: *const c_void,
        context: *mut *mut AGSContext,
        gpu_info: *mut AGSGPUInfo,
    ) -> c_int;

    // https://github.com/GPUOpen-LibrariesAndSDKs/AGS_SDK/blob/5d8812d703d0335741b6f7ffc37838eeb8b967f7/ags_lib/inc/amd_ags.h#L436
    #[allow(non_camel_case_types)]
    type agsDeInitialize_t = unsafe extern "C" fn(context: *mut AGSContext) -> c_int;

    pub(super) fn get_driver_version() -> Result<String> {
        #[cfg(target_pointer_width = "64")]
        let amd_dll_name = s!("amd_ags_x64.dll");
        #[cfg(target_pointer_width = "32")]
        let amd_dll_name = s!("amd_ags_x86.dll");

        with_dll_library(amd_dll_name, |amd_dll| unsafe {
            let ags_initialize_addr = GetProcAddress(amd_dll, s!("agsInitialize"))
                .ok_or_else(|| anyhow::anyhow!("Failed to get agsInitialize address"))?;
            let ags_deinitialize_addr = GetProcAddress(amd_dll, s!("agsDeInitialize"))
                .ok_or_else(|| anyhow::anyhow!("Failed to get agsDeInitialize address"))?;

            let ags_initialize: agsInitialize_t = std::mem::transmute(ags_initialize_addr);
            let ags_deinitialize: agsDeInitialize_t = std::mem::transmute(ags_deinitialize_addr);

            let mut context: *mut AGSContext = std::ptr::null_mut();
            let mut gpu_info: AGSGPUInfo = AGSGPUInfo {
                driver_version: std::ptr::null(),
                radeon_software_version: std::ptr::null(),
                num_devices: 0,
                devices: std::ptr::null_mut(),
            };

            let result = ags_initialize(
                AGS_CURRENT_VERSION,
                std::ptr::null(),
                &mut context,
                &mut gpu_info,
            );
            if result != 0 {
                anyhow::bail!("Failed to initialize AMD AGS, error code: {}", result);
            }

            // Vulkan actually returns this as the driver version
            let software_version = if !gpu_info.radeon_software_version.is_null() {
                std::ffi::CStr::from_ptr(gpu_info.radeon_software_version)
                    .to_string_lossy()
                    .into_owned()
            } else {
                "Unknown Radeon Software Version".to_string()
            };

            let driver_version = if !gpu_info.driver_version.is_null() {
                std::ffi::CStr::from_ptr(gpu_info.driver_version)
                    .to_string_lossy()
                    .into_owned()
            } else {
                "Unknown Radeon Driver Version".to_string()
            };

            ags_deinitialize(context);
            Ok(format!("{} ({})", software_version, driver_version))
        })
    }
}

mod dxgi {
    use windows::{
        Win32::Graphics::Dxgi::{IDXGIAdapter1, IDXGIDevice},
        core::Interface,
    };

    pub(super) fn get_driver_version(adapter: &IDXGIAdapter1) -> anyhow::Result<String> {
        let number = unsafe { adapter.CheckInterfaceSupport(&IDXGIDevice::IID as _) }?;
        Ok(format!(
            "{}.{}.{}.{}",
            number >> 48,
            (number >> 32) & 0xFFFF,
            (number >> 16) & 0xFFFF,
            number & 0xFFFF
        ))
    }
}
