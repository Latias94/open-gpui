use super::*;
use open_gpui::size;

/// Encodes GPUI scenes with a host-owned device, queue, target and command encoder.
///
/// This renderer does not install device callbacks, submit work, acquire surfaces or present.
/// The host must use the same device for the adapter, queue, encoder and target. Recording uses
/// storage instance data and requires a single-sample, single-layer 2D color attachment.
pub struct WgpuSceneRenderer {
    core: WgpuRendererCore,
}

/// Retains the atlas, private depth attachment and uploaded data of one encoded scene.
///
/// Keep this value until the host's corresponding GPU submission completes, including when the
/// renderer or original scene has been dropped. This value is not a submission or presentation
/// receipt. Dropping it after discarding an unsubmitted encoder is also valid.
#[must_use]
pub struct WgpuSceneRecording {
    _atlas: WgpuAtlasPresentationReceipt,
    _depth: wgpu::Texture,
    _globals: wgpu::BindGroup,
    _uploads: EncoderUploads,
}

impl WgpuSceneRenderer {
    /// Uses the supplied enabled device without changing its device-wide callback ownership.
    ///
    /// Native devices must enable at least six combined buffers per shader stage, including
    /// when the host opts into strict WebGPU validation that does not enforce that native limit.
    pub fn new(
        adapter: &wgpu::Adapter,
        device: Arc<wgpu::Device>,
        queue: Arc<wgpu::Queue>,
        format: wgpu::TextureFormat,
    ) -> anyhow::Result<Self> {
        Self::new_with_color_space(
            adapter,
            device,
            queue,
            format,
            WgpuSceneColorSpace::SrgbEncoded,
        )
    }

    /// Uses the supplied device and selects the scene's RGB composition working domain.
    ///
    /// `SrgbEncoded` preserves [`Self::new`]. `LinearSrgb` decodes authored sRGB colors before
    /// premultiplication and blending; an sRGB target then encodes once, while unorm and float
    /// targets store linear RGB. Gradient interpolation still follows each background's existing
    /// color-space selection. Imported-image alpha remains defined in its sampled value domain.
    /// This does not select a gamut, HDR policy, native surface or a second GPU resource owner.
    /// The same enabled-device requirements as [`Self::new`] apply.
    pub fn new_with_color_space(
        adapter: &wgpu::Adapter,
        device: Arc<wgpu::Device>,
        queue: Arc<wgpu::Queue>,
        format: wgpu::TextureFormat,
        color_space: WgpuSceneColorSpace,
    ) -> anyhow::Result<Self> {
        anyhow::ensure!(
            matches!(
                format,
                wgpu::TextureFormat::Rgba8Unorm
                    | wgpu::TextureFormat::Rgba8UnormSrgb
                    | wgpu::TextureFormat::Bgra8Unorm
                    | wgpu::TextureFormat::Bgra8UnormSrgb
                    | wgpu::TextureFormat::Rgba16Float
            ),
            "unsupported GPUI recording target format: {format:?}"
        );
        let limits = device.limits();
        for (enabled, required, name) in [
            (limits.max_bind_groups, 3, "bind groups"),
            (
                limits.max_bind_groups_plus_vertex_buffers,
                3,
                "combined bind groups",
            ),
            (
                limits.max_bindings_per_bind_group,
                4,
                "bindings per bind group",
            ),
            (
                limits.max_uniform_buffers_per_shader_stage,
                3,
                "uniform buffers per stage",
            ),
            (
                limits.max_storage_buffers_per_shader_stage,
                4,
                "storage buffers per stage",
            ),
            (
                limits.max_storage_buffers_in_vertex_stage,
                4,
                "vertex storage buffers",
            ),
            (
                limits.max_storage_buffers_in_fragment_stage,
                4,
                "fragment storage buffers",
            ),
            (
                limits.max_sampled_textures_per_shader_stage,
                2,
                "sampled textures per stage",
            ),
            (
                limits.max_samplers_per_shader_stage,
                1,
                "samplers per stage",
            ),
            (limits.max_color_attachments, 1, "color attachments"),
            (
                limits.max_color_attachment_bytes_per_sample,
                format
                    .target_pixel_byte_cost()
                    .context("recording format has no attachment cost")?,
                "color attachment bytes per sample",
            ),
        ] {
            anyhow::ensure!(
                enabled >= required,
                "GPUI recording requires {required} {name}; device enables {enabled}"
            );
        }
        #[cfg(not(target_family = "wasm"))]
        anyhow::ensure!(
            limits.max_buffers_and_acceleration_structures_per_shader_stage >= 6,
            "GPUI recording requires six combined buffers per shader stage"
        );
        let uniform_size = std::mem::size_of::<GlobalParams>()
            .max(std::mem::size_of::<GammaParams>())
            .max(std::mem::size_of::<SurfaceParams>()) as u64;
        anyhow::ensure!(
            limits.max_uniform_buffer_binding_size >= uniform_size,
            "GPUI recording uniforms exceed the device's enabled binding size"
        );
        let color_format = WgpuContext::select_color_texture_format(adapter)?;
        let atlas = Arc::new(WgpuAtlas::new(device.clone(), queue.clone(), color_format));
        let core = WgpuRendererCore::from_device_with_color_space(
            adapter,
            device,
            queue,
            atlas,
            format,
            wgpu::CompositeAlphaMode::PreMultiplied,
            false,
            color_space,
        )?;
        Ok(Self { core })
    }

    /// The atlas supplied to the GPUI Window that produces scenes for this renderer.
    pub fn sprite_atlas(&self) -> &Arc<WgpuAtlas> {
        &self.core.atlas
    }

    /// Records a complete scene in the caller's encoder, preserving explicit attachment load.
    ///
    /// Per-scene data uploads are ordered in this encoder, so several scenes can be recorded
    /// before one host submission. The full target texture, not a subresource view, defines the
    /// viewport. Invalid target facts fail before commands are emitted. On any subsequent
    /// recording error the caller must discard the encoder rather than submit partial commands.
    pub fn record(
        &mut self,
        scene: &Scene,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::Texture,
        load: wgpu::LoadOp<wgpu::Color>,
    ) -> anyhow::Result<WgpuSceneRecording> {
        anyhow::ensure!(
            target.dimension() == wgpu::TextureDimension::D2
                && target.depth_or_array_layers() == 1
                && target.sample_count() == 1
                && target.format() == self.core.target_format
                && target
                    .usage()
                    .contains(wgpu::TextureUsages::RENDER_ATTACHMENT),
            "scene target must be a matching single-layer single-sample 2D color attachment"
        );
        let size = size(
            DevicePixels(i32::try_from(target.width())?),
            DevicePixels(i32::try_from(target.height())?),
        );
        anyhow::ensure!(
            size.width.0 > 0
                && size.height.0 > 0
                && target.width() <= self.core.max_texture_size
                && target.height() <= self.core.max_texture_size,
            "scene target extent exceeds renderer limits"
        );
        anyhow::ensure!(
            scene.surfaces.is_empty(),
            "external scene recording does not yet support platform video surfaces"
        );
        self.core.encoder_uploads = Some(EncoderUploads::default());
        let result = (|| {
            let (clip_count, atlas) = self.core.prepare_scene(scene)?;
            self.core.prepare_frame_uniforms(size, true, clip_count)?;
            let view = target.create_view(&wgpu::TextureViewDescriptor {
                mip_level_count: Some(1),
                ..Default::default()
            });
            self.core
                .encode_frame(scene, &view, &atlas, encoder, load)?;
            Ok(WgpuSceneRecording {
                _atlas: atlas,
                _depth: self
                    .core
                    .resources
                    .depth_texture
                    .as_ref()
                    .context("private depth missing")?
                    .texture
                    .clone(),
                _globals: self.core.resources.globals_bind_group.clone(),
                _uploads: self
                    .core
                    .encoder_uploads
                    .take()
                    .context("recording uploads missing")?,
            })
        })();
        self.core.encoder_uploads = None;
        result
    }
}
