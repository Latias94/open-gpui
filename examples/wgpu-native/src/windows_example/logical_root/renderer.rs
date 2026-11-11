use super::*;
use open_gpui::{
    HostedSurfaceGeometry, HostedSurfaceRenderer, PlatformPresentationShutdownOutcome,
    PreparedPlatformPresentationShutdown,
};

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Basis {
    pub size: Size<Pixels>,
    pub scale: f32,
    pub revision: u64,
    pub visible: bool,
}

pub(super) struct Output {
    pub texture: wgpu::Texture,
    pub extent: (u32, u32),
    pub basis: Basis,
}

pub(super) struct Readback {
    pub buffer: wgpu::Buffer,
    pub stride: u32,
}

pub(super) struct Packet {
    pub commands: wgpu::CommandBuffer,
    pub recording: WgpuSceneRecording,
    pub output: Rc<Output>,
    pub token: PlatformFrameSubmission,
    pub count: u32,
    pub readback: Option<Readback>,
}

pub(super) struct Submitted {
    pub prefix: u64,
    pub _recording: WgpuSceneRecording,
    pub output: Rc<Output>,
    pub token: PlatformFrameSubmission,
}

pub(super) struct LastOutput {
    pub output: Rc<Output>,
    pub token: PlatformFrameSubmission,
    pub count: u32,
    pub index: wgpu::SubmissionIndex,
    pub readback: Option<Readback>,
}

pub(super) struct Delivery {
    pub basis: Basis,
    pub pending: Option<Packet>,
    pub in_flight: Vec<Submitted>,
    pub prefix: u64,
    pub completed: Arc<AtomicU64>,
    pub last_index: Option<wgpu::SubmissionIndex>,
    pub last_output: Option<LastOutput>,
    pub offers: u64,
    pub packet_wakes: u64,
    pub discarded: u64,
    pub pin: Option<u64>,
    pub quiesced: bool,
    pub ticket: Option<WindowPresentationShutdownTicket>,
    pub entry: Option<(u64, usize, bool)>,
    pub after_wait: Option<(u64, usize, bool)>,
    pub failure: Option<String>,
}

pub(super) struct Renderer {
    pub gpu: Arc<Gpu>,
    renderer: RefCell<WgpuSceneRenderer>,
    producer: wgpu::Texture,
    pub image: ImageTexture,
    pub count: Rc<Cell<u32>>,
    pub rendered_count: Rc<Cell<u32>>,
    pub delivery: Rc<RefCell<Delivery>>,
    smoke: bool,
    wake: Rc<dyn Fn()>,
}

impl Renderer {
    pub fn new(
        gpu: Arc<Gpu>,
        count: Rc<Cell<u32>>,
        smoke: bool,
        wake: Rc<dyn Fn()>,
    ) -> Result<Rc<Self>> {
        let renderer = WgpuSceneRenderer::new(
            &gpu.adapter,
            gpu.device.clone(),
            gpu.queue.clone(),
            wgpu::TextureFormat::Rgba8Unorm,
        )?;
        let producer = gpu.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("logical-root imported producer"),
            size: wgpu::Extent3d {
                width: 128,
                height: 128,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let image = renderer
            .sprite_atlas()
            .import_image(WgpuImportedImageDescriptor {
                texture: producer.clone(),
                mip_level: 0,
                array_layer: 0,
                region: Bounds::new(
                    point(DevicePixels(0), DevicePixels(0)),
                    size(DevicePixels(128), DevicePixels(128)),
                ),
                transfer: ImageTextureTransfer::Srgb,
                alpha: ImageTextureAlpha::Opaque,
                sampling: ImageTextureSampling::Linear,
                owner: None,
            })?;
        Ok(Rc::new(Self {
            gpu,
            renderer: RefCell::new(renderer),
            producer,
            image,
            count,
            delivery: Rc::new(RefCell::new(Delivery {
                basis: Basis {
                    size: size(px(640.), px(420.)),
                    scale: 1.,
                    revision: 0,
                    visible: true,
                },
                pending: None,
                in_flight: Vec::new(),
                prefix: 0,
                completed: Arc::new(AtomicU64::new(0)),
                last_index: None,
                last_output: None,
                offers: 0,
                packet_wakes: 0,
                discarded: 0,
                pin: None,
                quiesced: false,
                ticket: None,
                entry: None,
                after_wait: None,
                failure: None,
            })),
            smoke,
            wake,
            rendered_count: Rc::new(Cell::new(0)),
        }))
    }

    fn record(
        &self,
        scene: &Scene,
        geometry: HostedSurfaceGeometry,
        token: &PlatformFrameSubmission,
    ) -> Result<Packet> {
        let extent = geometry.physical_size();
        ensure!(
            extent.width.0 > 0 && extent.height.0 > 0,
            "logical zero extent reached recording"
        );
        let basis = self.delivery.borrow().basis;
        ensure!(
            basis.size == geometry.size()
                && basis.scale == geometry.scale_factor()
                && basis.visible,
            "logical recording geometry differs from the host's basis"
        );
        let output = Rc::new(Output {
            texture: self.gpu.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("host-owned logical-root GPUI output"),
                size: wgpu::Extent3d {
                    width: extent.width.0 as u32,
                    height: extent.height.0 as u32,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8Unorm,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::COPY_SRC
                    | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            }),
            extent: (extent.width.0 as u32, extent.height.0 as u32),
            basis,
        });
        let mut encoder = self
            .gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("original logical-root producer, GPUI and output-readback encoder"),
            });
        let count = self.rendered_count.get();
        let view = self.producer.create_view(&Default::default());
        {
            let _pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("logical-root image producer"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    depth_slice: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(if count % 2 == 0 {
                            wgpu::Color {
                                r: 0.15,
                                g: 0.55,
                                b: 0.85,
                                a: 1.,
                            }
                        } else {
                            wgpu::Color {
                                r: 0.85,
                                g: 0.35,
                                b: 0.15,
                                a: 1.,
                            }
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                ..Default::default()
            });
        }
        let recording = self.renderer.borrow_mut().record(
            scene,
            &mut encoder,
            &output.texture,
            wgpu::LoadOp::Clear(wgpu::Color::BLACK),
        )?;
        let readback = self.smoke.then(|| {
            let stride = (output.extent.0 * 4).div_ceil(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT)
                * wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
            let buffer = self.gpu.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("original GPUI output readback including glyphs and field"),
                size: stride as u64 * output.extent.1 as u64,
                usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                mapped_at_creation: false,
            });
            encoder.copy_texture_to_buffer(
                output.texture.as_image_copy(),
                wgpu::TexelCopyBufferInfo {
                    buffer: &buffer,
                    layout: wgpu::TexelCopyBufferLayout {
                        offset: 0,
                        bytes_per_row: Some(stride),
                        rows_per_image: None,
                    },
                },
                output.texture.size(),
            );
            Readback { buffer, stride }
        });
        Ok(Packet {
            commands: encoder.finish(),
            recording,
            output,
            token: token.clone(),
            count,
            readback,
        })
    }

    pub fn process_pending(&self, allow_submit: bool) -> Result<bool> {
        self.gpu.device.poll(wgpu::PollType::Poll)?;
        let mut delivery = self.delivery.borrow_mut();
        ensure!(
            delivery.failure.is_none(),
            "logical renderer failed: {:?}",
            delivery.failure
        );
        let completed = delivery.completed.load(Ordering::Acquire);
        let pin = delivery.pin;
        delivery
            .in_flight
            .retain(|frame| frame.prefix > completed || pin == Some(frame.prefix));
        let Some(packet) = delivery.pending.as_ref() else {
            return Ok(false);
        };
        if !packet.token.is_pending() || packet.output.basis != delivery.basis || delivery.quiesced
        {
            let packet = delivery.pending.take().unwrap();
            packet
                .token
                .complete(PlatformWindowPresentOutcome::RepaintRequired);
            delivery.discarded += 1;
            return Ok(false);
        }
        if !allow_submit {
            return Ok(false);
        }
        let packet = delivery.pending.take().unwrap();
        let Packet {
            commands,
            recording,
            output,
            token,
            count,
            readback,
        } = packet;
        let prefix = delivery
            .prefix
            .checked_add(1)
            .context("logical GPU prefix exhausted")?;
        ensure!(
            token.is_pending(),
            "logical packet was invalidated before queue submission"
        );
        let index = self.gpu.queue.submit([commands]);
        delivery.prefix = prefix;
        delivery.last_index = Some(index.clone());
        delivery.in_flight.push(Submitted {
            prefix,
            _recording: recording,
            output: output.clone(),
            token: token.clone(),
        });
        let completed = delivery.completed.clone();
        self.gpu.queue.on_submitted_work_done(move || {
            completed.fetch_max(prefix, Ordering::Release);
        });
        delivery.last_output = Some(LastOutput {
            output,
            token: token.clone(),
            count,
            index,
            readback,
        });
        drop(delivery);
        ensure!(
            token.complete(PlatformWindowPresentOutcome::Submitted),
            "original logical token refused actual submission"
        );
        Ok(true)
    }
}

impl HostedSurfaceRenderer for Renderer {
    fn sprite_atlas(&self) -> Arc<dyn PlatformAtlas> {
        self.renderer.borrow().sprite_atlas().clone()
    }

    fn draw(
        &self,
        scene: &Scene,
        geometry: HostedSurfaceGeometry,
        submission: &PlatformFrameSubmission,
    ) -> PlatformWindowPresentOutcome {
        {
            let mut delivery = self.delivery.borrow_mut();
            if delivery
                .pending
                .as_ref()
                .is_some_and(|packet| !packet.token.is_pending())
            {
                delivery.pending.take();
                delivery.discarded += 1;
            }
            if delivery.pending.is_some() || delivery.quiesced || !submission.is_pending() {
                return PlatformWindowPresentOutcome::Rejected;
            }
        }
        match self.record(scene, geometry, submission) {
            Ok(packet) => {
                let mut delivery = self.delivery.borrow_mut();
                delivery.pending = Some(packet);
                delivery.offers += 1;
                delivery.packet_wakes += 1;
                drop(delivery);
                (self.wake)();
                PlatformWindowPresentOutcome::Deferred
            }
            Err(error) => {
                self.delivery.borrow_mut().failure = Some(format!("{error:#}"));
                PlatformWindowPresentOutcome::Rejected
            }
        }
    }

    fn prepare_shutdown(
        &self,
        ticket: WindowPresentationShutdownTicket,
    ) -> PreparedPlatformPresentationShutdown {
        self.delivery.borrow_mut().ticket = Some(ticket.clone());
        let delivery = self.delivery.clone();
        let gpu = self.gpu.clone();
        PreparedPlatformPresentationShutdown::new(ticket, move |ticket| {
            let result = (|| -> Result<()> {
                let mut delivery = delivery.borrow_mut();
                if delivery.quiesced {
                    ensure!(
                        ticket.acknowledge_quiesced(),
                        "logical shutdown rejected repeated exact quiescence"
                    );
                    return Ok(());
                }
                let custody = |delivery: &Delivery| {
                    (
                        delivery.prefix,
                        delivery.in_flight.len(),
                        delivery
                            .in_flight
                            .iter()
                            .any(|frame| frame.prefix == delivery.prefix),
                    )
                };
                let entry = custody(&delivery);
                delivery.entry = Some(entry);
                if let Some(packet) = delivery.pending.take() {
                    packet
                        .token
                        .complete(PlatformWindowPresentOutcome::Rejected);
                    delivery.discarded += 1;
                }
                if let Some(index) = &delivery.last_index {
                    gpu.device.poll(wgpu::PollType::Wait {
                        submission_index: Some(index.clone()),
                        timeout: Some(Duration::from_secs(10)),
                    })?;
                }
                let after = custody(&delivery);
                delivery.after_wait = Some(after);
                ensure!(
                    entry == after,
                    "logical recording/output custody changed before prefix quiescence"
                );
                ensure!(
                    delivery.completed.load(Ordering::Acquire) >= delivery.prefix,
                    "logical original GPU prefix did not complete"
                );
                delivery.in_flight.clear();
                delivery.quiesced = true;
                ensure!(
                    ticket.acknowledge_quiesced(),
                    "logical shutdown rejected exact quiescence"
                );
                Ok(())
            })();
            match result {
                Ok(()) => PlatformPresentationShutdownOutcome::Quiesced,
                Err(error) => {
                    delivery.borrow_mut().failure = Some(format!("{error:#}"));
                    PlatformPresentationShutdownOutcome::Rejected
                }
            }
        })
    }
}
