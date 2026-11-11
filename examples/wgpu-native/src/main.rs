//! Application-owned WGPU rendering with GPUI-owned Win32 windows and event loop.

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
    use anyhow::{Context as _, Result, ensure};
    use open_gpui::{
        App, Application, Bounds, Corners, DevicePixels, ImageTexture, ImageTextureAlpha,
        ImageTextureSampling, ImageTextureTransfer, PlatformAtlas, PlatformWindowPresentOutcome,
        Scene, Size, Window, WindowBackgroundAppearance, WindowBounds, WindowOptions,
        WindowPresentationShutdownTicket, block_on, canvas, div, fill, point, prelude::*, px, rgb,
        size,
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
                    | wgpu::TextureUsages::TEXTURE_BINDING,
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
            Ok(Box::new(Renderer {
                gpu: self.gpu.clone(),
                window,
                surface: Some(surface),
                renderer,
                texture,
                count: self.count.clone(),
                submitted: self.submitted.clone(),
                last_submission: None,
                in_flight: Vec::new(),
                submitted_prefix: 0,
                completed_prefix: Arc::new(AtomicU64::new(0)),
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
        config: wgpu::SurfaceConfiguration,
        last_submission: Option<wgpu::SubmissionIndex>,
        in_flight: Vec<(u64, WgpuSceneRecording)>,
        submitted_prefix: u64,
        completed_prefix: Arc<AtomicU64>,
    }

    impl Renderer {
        fn wait(&self) -> Result<()> {
            if let Some(index) = &self.last_submission {
                self.gpu.device.poll(wgpu::PollType::Wait {
                    submission_index: Some(index.clone()),
                    timeout: Some(Duration::from_secs(30)),
                })?;
            }
            Ok(())
        }

        fn draw_scene(&mut self, scene: &Scene) -> Result<PlatformWindowPresentOutcome> {
            self.gpu.device.poll(wgpu::PollType::Poll)?;
            let completed = self.completed_prefix.load(Ordering::Acquire);
            self.in_flight.retain(|(prefix, _)| *prefix > completed);
            let surface = self.surface.as_ref().context("the renderer has quiesced")?;
            if self.config.width == 0 || self.config.height == 0 {
                return Ok(PlatformWindowPresentOutcome::Deferred);
            }
            let frame = match surface.get_current_texture() {
                wgpu::CurrentSurfaceTexture::Success(frame)
                | wgpu::CurrentSurfaceTexture::Suboptimal(frame) => frame,
                wgpu::CurrentSurfaceTexture::Timeout | wgpu::CurrentSurfaceTexture::Occluded => {
                    return Ok(PlatformWindowPresentOutcome::Deferred);
                }
                wgpu::CurrentSurfaceTexture::Outdated => {
                    surface.configure(&self.gpu.device, &self.config);
                    return Ok(PlatformWindowPresentOutcome::Deferred);
                }
                wgpu::CurrentSurfaceTexture::Lost => {
                    self.wait()?;
                    self.surface.take();
                    let surface = self.gpu.surface(self.window)?;
                    surface.configure(&self.gpu.device, &self.config);
                    self.surface = Some(surface);
                    return Ok(PlatformWindowPresentOutcome::Deferred);
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
            let color = if self.count.get() % 2 == 0 {
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
            let prefix = self
                .submitted_prefix
                .checked_add(1)
                .context("GPU submission prefix exhausted")?;
            let recording: WgpuSceneRecording = self.renderer.record(
                scene,
                &mut encoder,
                &frame.texture,
                wgpu::LoadOp::Clear(wgpu::Color::BLACK),
            )?;
            self.last_submission = Some(self.gpu.queue.submit([encoder.finish()]));
            self.submitted_prefix = prefix;
            self.in_flight.push((prefix, recording));
            let completed = self.completed_prefix.clone();
            self.gpu.queue.on_submitted_work_done(move || {
                completed.fetch_max(prefix, Ordering::Release);
            });
            self.gpu.queue.present(frame);
            self.submitted.set(prefix);
            Ok(PlatformWindowPresentOutcome::Submitted)
        }
    }

    impl WindowsRenderer for Renderer {
        fn sprite_atlas(&self) -> Arc<dyn PlatformAtlas> {
            self.renderer.sprite_atlas().clone()
        }

        fn resize(&mut self, extent: Size<DevicePixels>) -> Result<()> {
            let surface = self.surface.as_ref().context("the renderer has quiesced")?;
            ensure!(
                extent.width.0 >= 0 && extent.height.0 >= 0,
                "negative surface extent"
            );
            self.wait()?;
            self.config.width = extent.width.0 as u32;
            self.config.height = extent.height.0 as u32;
            if self.config.width > 0 && self.config.height > 0 {
                surface.configure(&self.gpu.device, &self.config);
            }
            Ok(())
        }

        fn draw(
            &mut self,
            scene: &Scene,
            _: WindowBackgroundAppearance,
        ) -> Result<PlatformWindowPresentOutcome> {
            self.draw_scene(scene)
                .inspect_err(|error| eprintln!("Drawing failed: {error:#}"))
        }

        fn quiesce(&mut self, shutdown: &WindowPresentationShutdownTicket) -> Result<()> {
            ensure!(
                shutdown.snapshot().window_id() == self.window.window_id(),
                "shutdown must identify the renderer's original window"
            );
            self.wait()?;
            ensure!(
                self.completed_prefix.load(Ordering::Acquire) >= self.submitted_prefix,
                "the original GPU completion callbacks must finish before quiescence"
            );
            self.in_flight.clear();
            self.surface.take();
            if std::env::args().any(|argument| argument == "--smoke") {
                println!("Quiesced original GPU prefix {}", self.submitted_prefix);
            }
            Ok(())
        }
    }

    struct Root {
        image: Rc<RefCell<Option<ImageTexture>>>,
        count: Rc<Cell<u32>>,
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
                .child("GPUI owns the window and event loop. The application owns the GPU.")
                .child(
                    div()
                        .id("change-image")
                        .px_4()
                        .py_2()
                        .bg(rgb(0x276650))
                        .rounded_md()
                        .cursor_pointer()
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.count.set(this.count.get().wrapping_add(1));
                            cx.notify();
                        }))
                        .child(format!("Change GPU image · count {}", self.count.get())),
                )
                .child(
                    div()
                        .relative()
                        .w(px(300.))
                        .h(px(160.))
                        .overflow_hidden()
                        .child(
                            canvas(
                                |_, _, _| (),
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
                                            eprintln!("Image painting failed: {error:#}");
                                        }
                                    }
                                },
                            )
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
        }
    }

    pub fn run() -> Result<()> {
        let smoke = std::env::args().any(|argument| argument == "--smoke");
        let factory = Rc::new(Factory {
            gpu: Arc::new(Gpu::new()?),
            image: Rc::new(RefCell::new(None)),
            count: Rc::new(Cell::new(0)),
            submitted: Rc::new(Cell::new(0)),
        });
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
                                if errors.load(Ordering::Relaxed) != 0 {
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
