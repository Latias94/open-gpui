//! Application-owned WGPU rendering with GPUI or application-owned Win32 hosts.

#[cfg(not(target_os = "windows"))]
fn main() {
    eprintln!("The wgpu example requires Windows.");
}

#[cfg(target_os = "windows")]
fn main() -> anyhow::Result<()> {
    windows_example::run()
}

#[cfg(target_os = "windows")]
mod windows_example {
    mod external_host;

    use anyhow::{Context as _, Result, ensure};
    use open_gpui::{
        AnyView, App, Application, Bounds, Corners, DevicePixels, ImageTexture, ImageTextureAlpha,
        ImageTextureSampling, ImageTextureTransfer, Pixels, PlatformAtlas,
        PlatformFrameDeliveryMode, PlatformFrameSubmission, PlatformWindowPresentOutcome, Scene,
        Size, Window, WindowBackgroundAppearance, WindowBounds, WindowOptions,
        WindowPresentationShutdownTicket, block_on, canvas, div, fill, measured_element, point,
        prelude::*, px, rgb, size,
    };
    use open_gpui_wgpu::{
        CosmicTextSystem, WgpuImportedImageDescriptor, WgpuSceneRecording, WgpuSceneRenderer, wgpu,
    };
    use open_gpui_windows::{
        WindowsPlatform, WindowsRenderer, WindowsRendererFactory, WindowsRendererWindow,
    };
    use std::{
        cell::{Cell, RefCell},
        rc::Rc,
        sync::{
            Arc,
            atomic::{AtomicU64, Ordering},
        },
        time::Duration,
    };

    struct Gpu {
        instance: wgpu::Instance,
        adapter: wgpu::Adapter,
        device: Arc<wgpu::Device>,
        queue: Arc<wgpu::Queue>,
        errors: Arc<AtomicU64>,
    }

    impl Gpu {
        fn new() -> Result<Self> {
            let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
                backends: wgpu::Backends::DX12 | wgpu::Backends::VULKAN,
                ..wgpu::InstanceDescriptor::new_without_display_handle()
            });
            let adapter = block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::LowPower,
                ..Default::default()
            }))?;
            let (device, queue) = block_on(
                adapter.request_device(&wgpu::DeviceDescriptor {
                    label: Some("application-owned GPUI device"),
                    required_limits: wgpu::Limits::downlevel_defaults()
                        .using_resolution(adapter.limits())
                        .using_alignment(adapter.limits()),
                    ..Default::default()
                }),
            )?;
            let errors = Arc::new(AtomicU64::new(0));
            device.on_uncaptured_error(Arc::new({
                let errors = errors.clone();
                move |error| {
                    errors.fetch_add(1, Ordering::Relaxed);
                    eprintln!("WGPU: {error}");
                }
            }));
            Ok(Self {
                instance,
                adapter,
                device: Arc::new(device),
                queue: Arc::new(queue),
                errors,
            })
        }

        fn surface(&self, window: WindowsRendererWindow) -> Result<wgpu::Surface<'static>> {
            // SAFETY: GPUI retains this HWND until renderer quiescence, which releases the
            // surface after its original GPU submissions complete. The handle is only borrowed.
            unsafe {
                self.instance
                    .create_surface_unsafe(wgpu::SurfaceTargetUnsafe::RawHandle {
                        raw_display_handle: Some(window.raw_display_handle()),
                        raw_window_handle: window.raw_window_handle(),
                    })
            }
            .context("creating the application's Win32 WGPU surface")
        }
    }

    struct Factory {
        gpu: Arc<Gpu>,
        image: Rc<RefCell<Option<ImageTexture>>>,
        count: Rc<Cell<u32>>,
        submitted: Rc<Cell<u64>>,
        observation: Rc<RenderObservation>,
        delivery_mode: PlatformFrameDeliveryMode,
        delivery: Rc<RefCell<FrameDelivery>>,
    }

    #[derive(Default)]
    struct RenderObservation {
        extent: Cell<(i32, i32)>,
        drawn_extent: Cell<(i32, i32)>,
        drawn_count: Cell<u32>,
        retained_recordings: Cell<usize>,
        completed: RefCell<Option<Arc<AtomicU64>>>,
        quiesced_prefix: Cell<Option<u64>>,
        button_bounds: Cell<Option<Bounds<Pixels>>>,
        submitted_button_bounds: Cell<Option<Bounds<Pixels>>>,
        published_button: Cell<Option<(u64, Bounds<Pixels>)>>,
        submitted_token: RefCell<Option<PlatformFrameSubmission>>,
        offered_frames: Cell<u64>,
        discarded_packets: Cell<u64>,
        failures: Cell<u64>,
        shutdown_ticket: RefCell<Option<WindowPresentationShutdownTicket>>,
        producer_readback: RefCell<Option<ProducerReadback>>,
        /// Smoke close keeps this original recording through quiescence, even if already complete.
        closing_prefix: Cell<Option<u64>>,
        quiesce_entry: Cell<Option<GpuRetention>>,
        quiesce_after_wait: Cell<Option<GpuRetention>>,
    }

    impl RenderObservation {
        fn completed_prefix(&self) -> u64 {
            self.completed
                .borrow()
                .as_ref()
                .map_or(0, |completed| completed.load(Ordering::Acquire))
        }
    }

    /// The native facts sampled while recording, rather than a later same-sized target.
    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    struct NativeTargetBasis {
        hwnd: windows::Win32::Foundation::HWND,
        origin: (i32, i32),
        extent: (i32, i32),
        dpi: u32,
        visible: bool,
        iconic: bool,
    }

    impl NativeTargetBasis {
        fn capture(window: WindowsRendererWindow) -> Result<Self> {
            use windows::Win32::{
                Foundation::{HWND, POINT, RECT},
                Graphics::Gdi::ClientToScreen,
                UI::{
                    HiDpi::GetDpiForWindow,
                    WindowsAndMessaging::{GetClientRect, IsIconic, IsWindowVisible},
                },
            };
            let raw_window_handle::RawWindowHandle::Win32(handle) = window.raw_window_handle()
            else {
                anyhow::bail!("the renderer requires its original Win32 target");
            };
            let hwnd = HWND(handle.hwnd.get() as *mut std::ffi::c_void);
            let mut rect = RECT::default();
            let mut origin = POINT::default();
            // SAFETY: The original renderer's HWND is retained through quiescence. These
            // queries run on its UI thread, outside native dispatch and App updates at submit.
            unsafe {
                GetClientRect(hwnd, &mut rect)?;
                ensure!(
                    ClientToScreen(hwnd, &mut origin).as_bool(),
                    "reading native client origin"
                );
            }
            let dpi = unsafe { GetDpiForWindow(hwnd) };
            ensure!(dpi > 0, "reading the native target DPI");
            Ok(Self {
                hwnd,
                origin: (origin.x, origin.y),
                extent: (rect.right - rect.left, rect.bottom - rect.top),
                dpi,
                visible: unsafe { IsWindowVisible(hwnd) }.as_bool(),
                iconic: unsafe { IsIconic(hwnd) }.as_bool(),
            })
        }
    }

    struct ProducerReadback {
        buffer: wgpu::Buffer,
        submission: PlatformFrameSubmission,
        index: wgpu::SubmissionIndex,
        prefix: u64,
    }

    /// Observed callback completion and custody, not whether hardware is still executing.
    #[derive(Clone, Copy, Debug)]
    struct GpuRetention {
        prefix: u64,
        completed: u64,
        recordings: usize,
        original_retained: bool,
    }

    /// Owns the finished encoder and acquired texture before any queue submission.
    /// The producer's inputs and the exact scene recording are frozen together.
    struct FramePacket {
        window: WindowsRendererWindow,
        command_buffer: wgpu::CommandBuffer,
        frame: wgpu::SurfaceTexture,
        recording: WgpuSceneRecording,
        submission: Option<PlatformFrameSubmission>,
        recorded_count: u32,
        extent: (i32, i32),
        target_generation: u64,
        native_basis: NativeTargetBasis,
        button_bounds: Option<Bounds<Pixels>>,
        producer_readback: Option<wgpu::Buffer>,
    }

    #[derive(Clone)]
    struct PendingFrame {
        submission: PlatformFrameSubmission,
        recorded_count: u32,
        extent: (i32, i32),
        target_generation: u64,
        button_bounds: Option<Bounds<Pixels>>,
    }

    /// Shared only on the UI thread. Both renderer paths use this original GPU ledger;
    /// the external host can drain its single packet after the draw call has returned.
    #[derive(Default)]
    struct FrameDelivery {
        pending: Option<FramePacket>,
        last_submission: Option<wgpu::SubmissionIndex>,
        in_flight: Vec<(u64, WgpuSceneRecording)>,
        submitted_prefix: u64,
        completed_prefix: Arc<AtomicU64>,
        target_generation: u64,
        extent: (i32, i32),
        quiesced: bool,
    }

    impl FrameDelivery {
        fn retention(&self) -> GpuRetention {
            GpuRetention {
                prefix: self.submitted_prefix,
                completed: self.completed_prefix.load(Ordering::Acquire),
                recordings: self.in_flight.len(),
                original_retained: self
                    .in_flight
                    .iter()
                    .any(|(prefix, _)| *prefix == self.submitted_prefix),
            }
        }

        fn wait(&self, gpu: &Gpu) -> Result<()> {
            if let Some(index) = &self.last_submission {
                gpu.device.poll(wgpu::PollType::Wait {
                    submission_index: Some(index.clone()),
                    timeout: Some(Duration::from_secs(30)),
                })?;
            }
            Ok(())
        }

        fn discard_pending(
            &mut self,
            observation: &RenderObservation,
            outcome: PlatformWindowPresentOutcome,
        ) {
            if let Some(packet) = self.pending.take() {
                if let Some(token) = &packet.submission {
                    token.complete(outcome);
                }
                drop(packet);
                observation
                    .discarded_packets
                    .set(observation.discarded_packets.get() + 1);
            }
        }

        fn submit(
            &mut self,
            gpu: &Gpu,
            submitted: &Cell<u64>,
            observation: &RenderObservation,
            packet: FramePacket,
        ) -> Result<bool> {
            ensure!(!self.quiesced, "submission after renderer quiescence");
            let prefix = self
                .submitted_prefix
                .checked_add(1)
                .context("GPU submission prefix exhausted")?;
            if packet
                .submission
                .as_ref()
                .is_some_and(|token| !token.is_pending())
            {
                drop(packet);
                observation
                    .discarded_packets
                    .set(observation.discarded_packets.get() + 1);
                return Ok(false);
            }
            let FramePacket {
                command_buffer,
                frame,
                recording,
                submission,
                recorded_count,
                extent,
                button_bounds,
                producer_readback,
                ..
            } = packet;
            let index = gpu.queue.submit([command_buffer]);
            self.last_submission = Some(index.clone());
            self.submitted_prefix = prefix;
            self.in_flight.push((prefix, recording));
            let completed = self.completed_prefix.clone();
            gpu.queue.on_submitted_work_done(move || {
                completed.fetch_max(prefix, Ordering::Release);
            });
            gpu.queue.present(frame);
            submitted.set(prefix);
            observation.drawn_count.set(recorded_count);
            observation.drawn_extent.set(extent);
            observation.submitted_button_bounds.set(button_bounds);
            observation.retained_recordings.set(self.in_flight.len());
            *observation.submitted_token.borrow_mut() = submission.clone();
            if let Some(buffer) = producer_readback {
                *observation.producer_readback.borrow_mut() = Some(ProducerReadback {
                    buffer,
                    submission: submission
                        .as_ref()
                        .context("producer readback requires its original token")?
                        .clone(),
                    index,
                    prefix,
                });
            }
            if let Some(token) = submission {
                ensure!(
                    token.complete(PlatformWindowPresentOutcome::Submitted),
                    "the original submitted token refused completion admission"
                );
            }
            Ok(true)
        }
    }

    impl Factory {
        fn read_producer_pixel(&self, original: &PlatformFrameSubmission) -> Result<[u8; 4]> {
            let readback = self
                .observation
                .producer_readback
                .borrow_mut()
                .take()
                .context("the original submission has no producer readback")?;
            ensure!(
                readback.prefix == self.submitted.get()
                    && readback.submission.window_id() == original.window_id()
                    && readback.submission.frame_generation() == original.frame_generation()
                    && readback.submission.attempt_id() == original.attempt_id(),
                "producer readback belongs to a different original submission"
            );
            let (sender, receiver) = std::sync::mpsc::sync_channel(1);
            readback
                .buffer
                .slice(..4)
                .map_async(wgpu::MapMode::Read, move |result| {
                    let _ = sender.send(result);
                });
            self.gpu.device.poll(wgpu::PollType::Wait {
                submission_index: Some(readback.index),
                timeout: Some(Duration::from_secs(5)),
            })?;
            receiver.recv_timeout(Duration::from_secs(1))??;
            let bytes = readback.buffer.slice(..4).get_mapped_range()?;
            let pixel: [u8; 4] = bytes[..4].try_into()?;
            drop(bytes);
            readback.buffer.unmap();
            println!(
                "Original producer GPU readback prefix {}: {pixel:?}",
                readback.prefix
            );
            Ok(pixel)
        }

        fn pending_frame(&self) -> Option<PendingFrame> {
            let delivery = self.delivery.borrow();
            let packet = delivery.pending.as_ref()?;
            Some(PendingFrame {
                submission: packet.submission.as_ref()?.clone(),
                recorded_count: packet.recorded_count,
                extent: packet.extent,
                target_generation: packet.target_generation,
                button_bounds: packet.button_bounds,
            })
        }

        /// Called only by the external host loop, never from ApplicationHandle::update.
        fn drain_pending(&self, allow_submit: bool) -> Result<bool> {
            let mut delivery = self.delivery.borrow_mut();
            let Some(packet) = delivery.pending.as_ref() else {
                return Ok(false);
            };
            let token = packet
                .submission
                .as_ref()
                .context("a deferred packet has no original token")?;
            if !token.is_pending() || delivery.quiesced {
                delivery.discard_pending(&self.observation, PlatformWindowPresentOutcome::Rejected);
                return Ok(false);
            }
            if packet.target_generation != delivery.target_generation
                || packet.extent != delivery.extent
                || packet.native_basis != NativeTargetBasis::capture(packet.window)?
            {
                delivery.discard_pending(
                    &self.observation,
                    PlatformWindowPresentOutcome::RepaintRequired,
                );
                return Ok(false);
            }
            if !allow_submit {
                return Ok(false);
            }
            let packet = delivery
                .pending
                .take()
                .context("the retained packet disappeared")?;
            delivery.submit(&self.gpu, &self.submitted, &self.observation, packet)
        }
    }

    impl WindowsRendererFactory for Factory {
        fn create(&self, window: WindowsRendererWindow) -> Result<Box<dyn WindowsRenderer>> {
            let surface = self.gpu.surface(window)?;
            let capabilities = surface.get_capabilities(&self.gpu.adapter);
            let format = [
                wgpu::TextureFormat::Bgra8Unorm,
                wgpu::TextureFormat::Rgba8Unorm,
            ]
            .into_iter()
            .find(|format| capabilities.formats.contains(format))
            .context("the surface has no supported GPUI presentation format")?;
            ensure!(
                capabilities
                    .alpha_modes
                    .contains(&wgpu::CompositeAlphaMode::Opaque),
                "the example requires an opaque window surface"
            );
            let renderer = WgpuSceneRenderer::new(
                &self.gpu.adapter,
                self.gpu.device.clone(),
                self.gpu.queue.clone(),
                format,
            )?;
            let texture = self.gpu.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("application GPU-produced image"),
                size: wgpu::Extent3d {
                    width: 128,
                    height: 128,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8Unorm,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::TEXTURE_BINDING
                    | wgpu::TextureUsages::COPY_SRC,
                view_formats: &[],
            });
            *self.image.borrow_mut() = Some(renderer.sprite_atlas().import_image(
                WgpuImportedImageDescriptor {
                    texture: texture.clone(),
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
                },
            )?);
            let completed_prefix = self.delivery.borrow().completed_prefix.clone();
            *self.observation.completed.borrow_mut() = Some(completed_prefix);
            Ok(Box::new(Renderer {
                gpu: self.gpu.clone(),
                window,
                surface: Some(surface),
                renderer,
                texture,
                count: self.count.clone(),
                submitted: self.submitted.clone(),
                observation: self.observation.clone(),
                delivery_mode: self.delivery_mode,
                delivery: self.delivery.clone(),
                config: wgpu::SurfaceConfiguration {
                    usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                    format,
                    width: 0,
                    height: 0,
                    present_mode: wgpu::PresentMode::Fifo,
                    desired_maximum_frame_latency: 2,
                    alpha_mode: wgpu::CompositeAlphaMode::Opaque,
                    view_formats: Vec::new(),
                    color_space: wgpu::SurfaceColorSpace::Auto,
                },
            }))
        }
    }

    struct Renderer {
        gpu: Arc<Gpu>,
        window: WindowsRendererWindow,
        surface: Option<wgpu::Surface<'static>>,
        renderer: WgpuSceneRenderer,
        texture: wgpu::Texture,
        count: Rc<Cell<u32>>,
        submitted: Rc<Cell<u64>>,
        observation: Rc<RenderObservation>,
        config: wgpu::SurfaceConfiguration,
        delivery_mode: PlatformFrameDeliveryMode,
        delivery: Rc<RefCell<FrameDelivery>>,
    }

    impl Renderer {
        fn record_scene(
            &mut self,
            scene: &Scene,
            submission: Option<&PlatformFrameSubmission>,
        ) -> Result<Option<FramePacket>> {
            self.gpu.device.poll(wgpu::PollType::Poll)?;
            {
                let mut delivery = self.delivery.borrow_mut();
                let completed = delivery.completed_prefix.load(Ordering::Acquire);
                delivery.in_flight.retain(|(prefix, _)| {
                    *prefix > completed || self.observation.closing_prefix.get() == Some(*prefix)
                });
                self.observation
                    .retained_recordings
                    .set(delivery.in_flight.len());
                ensure!(
                    delivery.pending.is_none(),
                    "recording over an unsubmitted packet"
                );
            }
            let surface = self.surface.as_ref().context("the renderer has quiesced")?;
            if self.config.width == 0 || self.config.height == 0 {
                return Ok(None);
            }
            let frame = match surface.get_current_texture() {
                wgpu::CurrentSurfaceTexture::Success(frame)
                | wgpu::CurrentSurfaceTexture::Suboptimal(frame) => frame,
                wgpu::CurrentSurfaceTexture::Timeout | wgpu::CurrentSurfaceTexture::Occluded => {
                    return Ok(None);
                }
                wgpu::CurrentSurfaceTexture::Outdated => {
                    surface.configure(&self.gpu.device, &self.config);
                    return Ok(None);
                }
                wgpu::CurrentSurfaceTexture::Lost => {
                    self.delivery.borrow().wait(&self.gpu)?;
                    self.surface.take();
                    let surface = self.gpu.surface(self.window)?;
                    surface.configure(&self.gpu.device, &self.config);
                    self.surface = Some(surface);
                    return Ok(None);
                }
                other => anyhow::bail!("surface acquisition failed: {other:?}"),
            };
            let mut encoder =
                self.gpu
                    .device
                    .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                        label: Some("application UI encoder"),
                    });
            let view = self.texture.create_view(&Default::default());
            let recorded_count = self.count.get();
            let color = if recorded_count % 2 == 0 {
                wgpu::Color {
                    r: 0.15,
                    g: 0.55,
                    b: 0.85,
                    a: 1.0,
                }
            } else {
                wgpu::Color {
                    r: 0.85,
                    g: 0.35,
                    b: 0.15,
                    a: 1.0,
                }
            };
            {
                let _pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("GPU image producer before GPUI sampling"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &view,
                        resolve_target: None,
                        depth_slice: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(color),
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    ..Default::default()
                });
            }
            let producer_readback = (submission.is_some()
                && std::env::args().any(|argument| argument == "--smoke"))
            .then(|| {
                let buffer = self.gpu.device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("original producer pixel readback"),
                    size: wgpu::COPY_BYTES_PER_ROW_ALIGNMENT as u64,
                    usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                    mapped_at_creation: false,
                });
                encoder.copy_texture_to_buffer(
                    self.texture.as_image_copy(),
                    wgpu::TexelCopyBufferInfo {
                        buffer: &buffer,
                        layout: wgpu::TexelCopyBufferLayout {
                            offset: 0,
                            bytes_per_row: Some(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT),
                            rows_per_image: None,
                        },
                    },
                    wgpu::Extent3d {
                        width: 1,
                        height: 1,
                        depth_or_array_layers: 1,
                    },
                );
                buffer
            });
            let recording: WgpuSceneRecording = self.renderer.record(
                scene,
                &mut encoder,
                &frame.texture,
                wgpu::LoadOp::Clear(wgpu::Color::BLACK),
            )?;
            Ok(Some(FramePacket {
                window: self.window,
                command_buffer: encoder.finish(),
                frame,
                recording,
                submission: submission.cloned(),
                recorded_count,
                extent: (self.config.width as i32, self.config.height as i32),
                target_generation: self.delivery.borrow().target_generation,
                native_basis: NativeTargetBasis::capture(self.window)?,
                button_bounds: self.observation.button_bounds.get(),
                producer_readback,
            }))
        }

        fn draw_scene(
            &mut self,
            scene: &Scene,
            submission: Option<&PlatformFrameSubmission>,
        ) -> Result<PlatformWindowPresentOutcome> {
            if self
                .delivery
                .borrow()
                .pending
                .as_ref()
                .is_some_and(|packet| {
                    packet
                        .submission
                        .as_ref()
                        .is_some_and(|token| !token.is_pending())
                })
            {
                self.delivery
                    .borrow_mut()
                    .discard_pending(&self.observation, PlatformWindowPresentOutcome::Rejected);
            }
            if let Some(packet) = self.delivery.borrow().pending.as_ref() {
                let token = submission.context("synchronous drawing with a retained packet")?;
                let original = packet
                    .submission
                    .as_ref()
                    .context("retained packet lost its token")?;
                ensure!(
                    token.window_id() == original.window_id()
                        && token.frame_generation() == original.frame_generation()
                        && token.attempt_id() == original.attempt_id(),
                    "a retained packet cannot be retargeted"
                );
                return Ok(PlatformWindowPresentOutcome::Deferred);
            }
            let Some(packet) = self.record_scene(scene, submission)? else {
                return Ok(
                    if self.delivery_mode == PlatformFrameDeliveryMode::Deferred {
                        PlatformWindowPresentOutcome::RepaintRequired
                    } else {
                        PlatformWindowPresentOutcome::Deferred
                    },
                );
            };
            if self.delivery_mode == PlatformFrameDeliveryMode::Deferred {
                ensure!(
                    submission.is_some(),
                    "deferred drawing requires the original submission token"
                );
                self.delivery.borrow_mut().pending = Some(packet);
                Ok(PlatformWindowPresentOutcome::Deferred)
            } else {
                ensure!(
                    self.delivery.borrow_mut().submit(
                        &self.gpu,
                        &self.submitted,
                        &self.observation,
                        packet,
                    )?,
                    "synchronous submission unexpectedly discarded its packet"
                );
                Ok(PlatformWindowPresentOutcome::Submitted)
            }
        }
    }

    impl WindowsRenderer for Renderer {
        fn sprite_atlas(&self) -> Arc<dyn PlatformAtlas> {
            self.renderer.sprite_atlas().clone()
        }

        fn frame_delivery_mode(&self) -> PlatformFrameDeliveryMode {
            self.delivery_mode
        }

        fn resize(&mut self, extent: Size<DevicePixels>) -> Result<()> {
            let surface = self.surface.as_ref().context("the renderer has quiesced")?;
            ensure!(
                extent.width.0 >= 0 && extent.height.0 >= 0,
                "negative surface extent"
            );
            let mut delivery = self.delivery.borrow_mut();
            delivery.discard_pending(
                &self.observation,
                PlatformWindowPresentOutcome::RepaintRequired,
            );
            delivery.target_generation = delivery
                .target_generation
                .checked_add(1)
                .context("native target generation exhausted")?;
            delivery.extent = (extent.width.0, extent.height.0);
            delivery.wait(&self.gpu)?;
            self.config.width = extent.width.0 as u32;
            self.config.height = extent.height.0 as u32;
            if self.config.width > 0 && self.config.height > 0 {
                surface.configure(&self.gpu.device, &self.config);
            }
            self.observation
                .extent
                .set((extent.width.0, extent.height.0));
            Ok(())
        }

        fn draw(
            &mut self,
            scene: &Scene,
            _: WindowBackgroundAppearance,
        ) -> Result<PlatformWindowPresentOutcome> {
            self.draw_scene(scene, None).inspect_err(|error| {
                self.observation
                    .failures
                    .set(self.observation.failures.get() + 1);
                eprintln!("Drawing failed: {error:#}");
            })
        }

        fn draw_with_submission(
            &mut self,
            scene: &Scene,
            background: WindowBackgroundAppearance,
            submission: &PlatformFrameSubmission,
        ) -> Result<PlatformWindowPresentOutcome> {
            if self.delivery_mode == PlatformFrameDeliveryMode::Synchronous {
                return self.draw(scene, background);
            }
            self.observation
                .offered_frames
                .set(self.observation.offered_frames.get() + 1);
            ensure!(
                submission.window_id() == self.window.window_id() && submission.is_pending(),
                "deferred drawing requires this window's pending token"
            );
            self.draw_scene(scene, Some(submission))
                .inspect_err(|error| {
                    self.observation
                        .failures
                        .set(self.observation.failures.get() + 1);
                    eprintln!("Recording failed: {error:#}");
                })
        }

        fn quiesce(&mut self, shutdown: &WindowPresentationShutdownTicket) -> Result<()> {
            ensure!(
                shutdown.snapshot().window_id() == self.window.window_id(),
                "shutdown must identify the renderer's original window"
            );
            let mut delivery = self.delivery.borrow_mut();
            let entry = delivery.retention();
            self.observation.quiesce_entry.set(Some(entry));
            delivery.discard_pending(&self.observation, PlatformWindowPresentOutcome::Rejected);
            delivery.wait(&self.gpu)?;
            let waited = delivery.retention();
            self.observation.quiesce_after_wait.set(Some(waited));
            ensure!(
                entry.prefix == waited.prefix
                    && entry.recordings == waited.recordings
                    && entry.original_retained == waited.original_retained,
                "original recording custody changed before the GPU wait returned"
            );
            ensure!(
                delivery.completed_prefix.load(Ordering::Acquire) >= delivery.submitted_prefix,
                "the original GPU completion callbacks must finish before quiescence"
            );
            delivery.in_flight.clear();
            self.observation.producer_readback.borrow_mut().take();
            self.surface.take();
            delivery.quiesced = true;
            self.observation.retained_recordings.set(0);
            self.observation
                .quiesced_prefix
                .set(Some(delivery.submitted_prefix));
            *self.observation.shutdown_ticket.borrow_mut() = Some(shutdown.clone());
            if std::env::args().any(|argument| argument == "--smoke") {
                println!("Quiesced original GPU prefix {}", delivery.submitted_prefix);
            }
            Ok(())
        }
    }

    struct Root {
        image: Rc<RefCell<Option<ImageTexture>>>,
        count: Rc<Cell<u32>>,
        host_input: Option<AnyView>,
        observation: Rc<RenderObservation>,
        button_shift: Pixels,
    }

    impl Render for Root {
        fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            let image = self.image.borrow().clone();
            div()
                .size_full()
                .flex()
                .flex_col()
                .gap_4()
                .p_6()
                .bg(rgb(0x162334))
                .text_color(rgb(0xf2f4f8))
                .font_family("Segoe UI")
                .child("Application-owned WGPU + GPUI")
                .child(if self.host_input.is_some() {
                    "The application owns the HWND, message loop and GPU."
                } else {
                    "GPUI owns the window and event loop. The application owns the GPU."
                })
                .child(measured_element(
                    "submitted-change-image",
                    div()
                        .id("change-image")
                        .relative()
                        .ml(self.button_shift)
                        .w(px(200.))
                        .px_4()
                        .py_2()
                        .bg(rgb(0x276650))
                        .rounded_md()
                        .cursor_pointer()
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.count.set(this.count.get().wrapping_add(1));
                            cx.notify();
                        }))
                        .child(
                            canvas(
                                {
                                    let observation = self.observation.clone();
                                    move |bounds, _, _| observation.button_bounds.set(Some(bounds))
                                },
                                |_, (), _, _| {},
                            )
                            .absolute()
                            .top_0()
                            .left_0()
                            .size_full(),
                        )
                        .child("Change GPU image"),
                    {
                        let observation = self.observation.clone();
                        move |snapshot, _| {
                            observation.published_button.set(Some((
                                snapshot.frame_generation(),
                                snapshot.geometry().displayed_bounds(),
                            )));
                        }
                    },
                ))
                .child(
                    div()
                        .relative()
                        .w(px(300.))
                        .h(px(160.))
                        .overflow_hidden()
                        .child(
                            canvas(|_, _, _| (), {
                                let observation = self.observation.clone();
                                move |bounds, (), window, _| {
                                    window.paint_quad(fill(bounds, rgb(0x344358)));
                                    if let Some(image) = image {
                                        let bounds = Bounds::new(
                                            bounds.origin + point(px(-24.), px(12.)),
                                            size(px(360.), px(180.)),
                                        );
                                        if let Err(error) = window.paint_image_texture(
                                            bounds,
                                            Corners::all(px(16.)),
                                            &image,
                                        ) {
                                            observation
                                                .failures
                                                .set(observation.failures.get() + 1);
                                            eprintln!("Image painting failed: {error:#}");
                                        }
                                    }
                                }
                            })
                            .size_full(),
                        )
                        .child(
                            div()
                                .absolute()
                                .left(px(16.))
                                .top(px(24.))
                                .p_3()
                                .bg(rgb(0x263448))
                                .rounded_md()
                                .child("Ordinary UI above the clipped GPU image"),
                        ),
                )
                .children(self.host_input.clone())
        }
    }

    pub fn run() -> Result<()> {
        let smoke = std::env::args().any(|argument| argument == "--smoke");
        let deferred = std::env::args().any(|argument| argument == "--deferred");
        let external_host = std::env::args().any(|argument| argument == "--external-host");
        ensure!(
            !deferred || external_host,
            "--deferred requires --external-host, which owns the submission drain"
        );
        let factory = Rc::new(Factory {
            gpu: Arc::new(Gpu::new()?),
            image: Rc::new(RefCell::new(None)),
            count: Rc::new(Cell::new(0)),
            submitted: Rc::new(Cell::new(0)),
            observation: Rc::new(RenderObservation::default()),
            delivery_mode: if deferred {
                PlatformFrameDeliveryMode::Deferred
            } else {
                PlatformFrameDeliveryMode::Synchronous
            },
            delivery: Rc::new(RefCell::new(FrameDelivery::default())),
        });
        if external_host {
            return external_host::run(factory, smoke);
        }
        let platform = Rc::new(WindowsPlatform::new_with_renderer(
            Arc::new(CosmicTextSystem::new("Segoe UI")),
            factory.clone(),
        )?);
        Application::with_platform(platform).run(move |cx: &mut App| {
            let bounds = Bounds::centered(None, size(px(660.), px(400.)), cx);
            let handle = match cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    ..Default::default()
                },
                |_, cx| {
                    cx.new(|_| Root {
                        image: factory.image.clone(),
                        count: factory.count.clone(),
                        host_input: None,
                        observation: factory.observation.clone(),
                        button_shift: px(0.),
                    })
                },
            ) {
                Ok(handle) => handle,
                Err(error) => {
                    eprintln!("Opening the WGPU window failed: {error:#}");
                    std::process::exit(1);
                }
            };
            cx.activate(true);
            if smoke {
                let submitted = factory.submitted.clone();
                let errors = factory.gpu.errors.clone();
                let observation = factory.observation.clone();
                cx.spawn(async move |cx| {
                    let mut changed_after = None;
                    for _ in 0..600 {
                        cx.background_executor()
                            .timer(Duration::from_millis(16))
                            .await;
                        if submitted.get() == 0 {
                            continue;
                        }
                        if let Some(previous) = changed_after {
                            if submitted.get() > previous {
                                if errors.load(Ordering::Relaxed) != 0
                                    || observation.failures.get() != 0
                                {
                                    eprintln!("The WGPU example reported a GPU error");
                                    std::process::exit(1);
                                }
                                println!(
                                    "Submitted changed GPU image at prefix {}",
                                    submitted.get()
                                );
                                cx.update(|cx| cx.quit());
                                return;
                            }
                        } else {
                            changed_after = Some(submitted.get());
                            if let Err(error) = handle.update(cx, |root, _, cx| {
                                root.count.set(1);
                                cx.notify();
                            }) {
                                eprintln!("Updating the WGPU example failed: {error:#}");
                                std::process::exit(1);
                            }
                        }
                    }
                    eprintln!("The WGPU example did not submit its initial and changed frames");
                    std::process::exit(1);
                })
                .detach();
            }
        });
        Ok(())
    }
}
