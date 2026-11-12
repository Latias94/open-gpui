use super::*;
use anyhow::{Context as _, Result};
use open_gpui::{
    Corners, ImageTexture, ImageTextureAlpha, ImageTextureSampling, ImageTextureTransfer,
    InteractiveElement as _, MouseButton, PlatformAtlas, PlatformTextSystem, Scene, Size,
    WindowBackgroundAppearance, WindowPresentationShutdownTicket, rgb,
};
use open_gpui_wgpu::{
    CosmicTextSystem, WgpuAtlas, WgpuImportedImageDescriptor, WgpuSceneRecording,
    WgpuSceneRenderer, wgpu,
};
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    mpsc,
};
use windows::Win32::UI::WindowsAndMessaging::{SW_MINIMIZE, SW_RESTORE};

const INITIAL_IMAGE: [u8; 4] = [208, 80, 32, 255];
const UPDATED_IMAGE: [u8; 4] = [48, 112, 224, 255];
const BACKGROUND: [u8; 4] = [22, 35, 52, 255];
const CONTROL: [u8; 4] = [32, 176, 128, 255];

struct NativeWgpuDevice {
    instance: wgpu::Instance,
    adapter: wgpu::Adapter,
    device: Arc<wgpu::Device>,
    queue: Arc<wgpu::Queue>,
    errors: Arc<parking_lot::Mutex<Vec<String>>>,
}

impl NativeWgpuDevice {
    fn new() -> Result<Self> {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::VULKAN,
            ..wgpu::InstanceDescriptor::new_without_display_handle()
        });
        let adapter =
            open_gpui::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::LowPower,
                force_fallback_adapter: false,
                compatible_surface: None,
                apply_limit_buckets: false,
            }))?;
        let (device, queue) = open_gpui::block_on(
            adapter.request_device(&wgpu::DeviceDescriptor {
                label: Some("native application-owned GPUI device"),
                required_limits: wgpu::Limits::downlevel_defaults()
                    .using_resolution(adapter.limits())
                    .using_alignment(adapter.limits()),
                ..Default::default()
            }),
        )?;
        let errors = Arc::new(parking_lot::Mutex::new(Vec::new()));
        device.on_uncaptured_error(Arc::new({
            let errors = errors.clone();
            move |error| errors.lock().push(error.to_string())
        }));
        Ok(Self {
            instance,
            adapter,
            device: Arc::new(device),
            queue: Arc::new(queue),
            errors,
        })
    }
}

#[derive(Default)]
struct NativeRendererObservations {
    fail_creation: Cell<bool>,
    defer_submission: Cell<bool>,
    block_quiescence: Cell<bool>,
    panic_on_next_resize: Cell<bool>,
    panic_on_next_quiesce: Cell<bool>,
    resize_panics: Cell<usize>,
    quiescence_panics: Cell<usize>,
    quiescence_panic_retained_work: Cell<bool>,
    deferred: Cell<usize>,
    submissions: Cell<usize>,
    completed_prefix: Arc<AtomicUsize>,
    resize_sizes: RefCell<Vec<Size<DevicePixels>>>,
    submitted_extents: RefCell<Vec<(u32, u32)>>,
    paint_errors: RefCell<Vec<String>>,
    glyphs_submitted: Cell<usize>,
    quiescence_attempts: Cell<usize>,
    quiescence_tickets: RefCell<Vec<WindowPresentationShutdownTicket>>,
    quiesced_while_native_live: Cell<bool>,
    quiesced_prefix: Cell<usize>,
    pixels: RefCell<Vec<[[u8; 4]; 3]>>,
    image_owner: RefCell<Option<std::sync::Weak<()>>>,
    atlas: RefCell<Option<std::sync::Weak<WgpuAtlas>>>,
}

struct NativeWgpuFactory {
    gpu: Arc<NativeWgpuDevice>,
    observations: Rc<NativeRendererObservations>,
    image: RefCell<Option<ImageTexture>>,
    texture: RefCell<Option<wgpu::Texture>>,
}

impl crate::WindowsRendererFactory for NativeWgpuFactory {
    fn create(
        &self,
        window: crate::WindowsRendererWindow,
    ) -> Result<Box<dyn crate::WindowsRenderer>> {
        anyhow::ensure!(
            !self.observations.fail_creation.get(),
            "intentional native renderer factory refusal"
        );
        // SAFETY: WindowsWindowInner owns this exact HWND and waits for this renderer's
        // successful quiescence, which drops the surface, before native destruction.
        let surface = unsafe {
            self.gpu
                .instance
                .create_surface_unsafe(wgpu::SurfaceTargetUnsafe::RawHandle {
                    raw_display_handle: Some(window.raw_display_handle()),
                    raw_window_handle: window.raw_window_handle(),
                })?
        };
        let capabilities = surface.get_capabilities(&self.gpu.adapter);
        anyhow::ensure!(
            capabilities.usages.contains(wgpu::TextureUsages::COPY_SRC),
            "the native surface must permit exact submitted-pixel observation"
        );
        let format = [
            wgpu::TextureFormat::Bgra8Unorm,
            wgpu::TextureFormat::Rgba8Unorm,
        ]
        .into_iter()
        .find(|format| capabilities.formats.contains(format))
        .context("native surface has no GPUI unorm presentation format")?;
        let alpha_mode = if capabilities
            .alpha_modes
            .contains(&wgpu::CompositeAlphaMode::Opaque)
        {
            wgpu::CompositeAlphaMode::Opaque
        } else {
            *capabilities
                .alpha_modes
                .first()
                .context("native surface has no alpha mode")?
        };
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            format,
            width: 1,
            height: 1,
            present_mode: wgpu::PresentMode::Fifo,
            desired_maximum_frame_latency: 2,
            alpha_mode,
            view_formats: Vec::new(),
            color_space: wgpu::SurfaceColorSpace::Auto,
        };
        surface.configure(&self.gpu.device, &config);
        let renderer = WgpuSceneRenderer::new(
            &self.gpu.adapter,
            self.gpu.device.clone(),
            self.gpu.queue.clone(),
            format,
        )?;
        *self.observations.atlas.borrow_mut() = Some(Arc::downgrade(renderer.sprite_atlas()));
        let texture = self.gpu.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("application-owned changing viewport texture"),
            size: wgpu::Extent3d {
                width: 4,
                height: 4,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        write_viewport_texture(&self.gpu.queue, &texture, INITIAL_IMAGE);
        let owner = Arc::new(());
        *self.observations.image_owner.borrow_mut() = Some(Arc::downgrade(&owner));
        let image = renderer
            .sprite_atlas()
            .import_image(WgpuImportedImageDescriptor {
                texture: texture.clone(),
                mip_level: 0,
                array_layer: 0,
                region: Bounds::new(
                    point(DevicePixels(0), DevicePixels(0)),
                    size(DevicePixels(4), DevicePixels(4)),
                ),
                transfer: ImageTextureTransfer::Srgb,
                alpha: ImageTextureAlpha::Opaque,
                sampling: ImageTextureSampling::Nearest,
                owner: Some(owner),
            })?;
        *self.image.borrow_mut() = Some(image);
        *self.texture.borrow_mut() = Some(texture);
        let raw_window_handle::RawWindowHandle::Win32(handle) = window.raw_window_handle() else {
            anyhow::bail!("Windows renderer factory received a non-Win32 handle")
        };
        Ok(Box::new(NativeWgpuRenderer {
            gpu: self.gpu.clone(),
            observations: self.observations.clone(),
            hwnd: HWND(handle.hwnd.get() as *mut std::ffi::c_void),
            window_id: window.window_id(),
            surface: Some(surface),
            atlas: renderer.sprite_atlas().clone(),
            renderer: Some(renderer),
            config,
            in_flight: Vec::new(),
        }))
    }
}

fn write_viewport_texture(queue: &wgpu::Queue, texture: &wgpu::Texture, rgba: [u8; 4]) {
    queue.write_texture(
        texture.as_image_copy(),
        &rgba.repeat(16),
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(16),
            rows_per_image: Some(4),
        },
        texture.size(),
    );
}

struct NativeWgpuSubmission {
    index: wgpu::SubmissionIndex,
    _recording: WgpuSceneRecording,
    pixels: wgpu::Buffer,
}

struct NativeWgpuRenderer {
    gpu: Arc<NativeWgpuDevice>,
    observations: Rc<NativeRendererObservations>,
    hwnd: HWND,
    window_id: WindowId,
    surface: Option<wgpu::Surface<'static>>,
    atlas: Arc<WgpuAtlas>,
    renderer: Option<WgpuSceneRenderer>,
    config: wgpu::SurfaceConfiguration,
    in_flight: Vec<NativeWgpuSubmission>,
}

impl NativeWgpuRenderer {
    fn wait_for_submitted_prefix(&mut self) -> Result<()> {
        let Some(last) = self.in_flight.last() else {
            return Ok(());
        };
        self.gpu.device.poll(wgpu::PollType::Wait {
            submission_index: Some(last.index.clone()),
            timeout: Some(Duration::from_secs(30)),
        })?;
        anyhow::ensure!(
            self.observations.completed_prefix.load(Ordering::SeqCst)
                == self.observations.submissions.get(),
            "the host completion callback must settle the exact submitted GPU prefix"
        );
        for submission in &self.in_flight {
            let (sender, receiver) = mpsc::channel();
            submission
                .pixels
                .slice(..)
                .map_async(wgpu::MapMode::Read, move |result| {
                    let _ = sender.send(result);
                });
            self.gpu.device.poll(wgpu::PollType::Wait {
                submission_index: Some(submission.index.clone()),
                timeout: Some(Duration::from_secs(30)),
            })?;
            receiver.recv_timeout(Duration::from_secs(30))??;
            let view = submission.pixels.slice(..).get_mapped_range()?;
            let mut pixels = [[0u8; 4]; 3];
            for (index, pixel) in pixels.iter_mut().enumerate() {
                pixel.copy_from_slice(&view[index * 256..index * 256 + 4]);
                if self.config.format == wgpu::TextureFormat::Bgra8Unorm {
                    pixel.swap(0, 2);
                }
            }
            drop(view);
            submission.pixels.unmap();
            self.observations.pixels.borrow_mut().push(pixels);
        }
        self.in_flight.clear();
        Ok(())
    }
}

impl crate::WindowsRenderer for NativeWgpuRenderer {
    fn sprite_atlas(&self) -> Arc<dyn PlatformAtlas> {
        self.atlas.clone()
    }

    fn resize(&mut self, size: Size<DevicePixels>) -> Result<()> {
        if self.observations.panic_on_next_resize.replace(false) {
            self.observations
                .resize_panics
                .set(self.observations.resize_panics.get() + 1);
            panic!("intentional native renderer resize panic");
        }
        self.wait_for_submitted_prefix()?;
        self.config.width = u32::try_from(size.width.0)?;
        self.config.height = u32::try_from(size.height.0)?;
        anyhow::ensure!(
            self.config.width > 0 && self.config.height > 0,
            "empty native target"
        );
        self.surface
            .as_ref()
            .context("native surface already quiesced")?
            .configure(&self.gpu.device, &self.config);
        self.observations.resize_sizes.borrow_mut().push(size);
        Ok(())
    }

    fn draw(
        &mut self,
        scene: &Scene,
        _: WindowBackgroundAppearance,
    ) -> Result<PlatformWindowPresentOutcome> {
        if self.observations.defer_submission.get() {
            self.observations
                .deferred
                .set(self.observations.deferred.get() + 1);
            return Ok(PlatformWindowPresentOutcome::Deferred);
        }
        let surface = self
            .surface
            .as_ref()
            .context("native surface already quiesced")?;
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
            other => anyhow::bail!("native surface acquisition failed: {other:?}"),
        };
        let mut encoder = self
            .gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("application-owned native GPUI encoder"),
            });
        let recording = self
            .renderer
            .as_mut()
            .context("native renderer already quiesced")?
            .record(
                scene,
                &mut encoder,
                &frame.texture,
                wgpu::LoadOp::Clear(wgpu::Color::BLACK),
            )?;
        let pixels = self.gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("exact native submitted pixel probes"),
            size: 3 * 256,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let scale_factor =
            unsafe { windows::Win32::UI::HiDpi::GetDpiForWindow(self.hwnd) } as f32 / 96.;
        for (index, (x, y)) in [(240., 180.), (24., 24.), (128., 68.)]
            .into_iter()
            .enumerate()
        {
            let origin = wgpu::Origin3d {
                x: (x * scale_factor).round() as u32,
                y: (y * scale_factor).round() as u32,
                z: 0,
            };
            anyhow::ensure!(
                origin.x < self.config.width && origin.y < self.config.height,
                "native probe outside target"
            );
            encoder.copy_texture_to_buffer(
                wgpu::TexelCopyTextureInfo {
                    texture: &frame.texture,
                    mip_level: 0,
                    origin,
                    aspect: wgpu::TextureAspect::All,
                },
                wgpu::TexelCopyBufferInfo {
                    buffer: &pixels,
                    layout: wgpu::TexelCopyBufferLayout {
                        offset: (index * 256) as u64,
                        bytes_per_row: Some(256),
                        rows_per_image: Some(1),
                    },
                },
                wgpu::Extent3d {
                    width: 1,
                    height: 1,
                    depth_or_array_layers: 1,
                },
            );
        }
        let index = self.gpu.queue.submit([encoder.finish()]);
        let prefix = self.observations.submissions.get() + 1;
        self.observations.submissions.set(prefix);
        let completed = self.observations.completed_prefix.clone();
        self.gpu.queue.on_submitted_work_done(move || {
            completed.fetch_max(prefix, Ordering::SeqCst);
        });
        self.in_flight.push(NativeWgpuSubmission {
            index,
            _recording: recording,
            pixels,
        });
        self.gpu.queue.present(frame);
        self.observations
            .glyphs_submitted
            .set(self.observations.glyphs_submitted.get() + scene.monochrome_sprites.len());
        self.observations
            .submitted_extents
            .borrow_mut()
            .push((self.config.width, self.config.height));
        Ok(PlatformWindowPresentOutcome::Submitted)
    }

    fn quiesce(&mut self, shutdown: &WindowPresentationShutdownTicket) -> Result<()> {
        self.observations
            .quiescence_attempts
            .set(self.observations.quiescence_attempts.get() + 1);
        self.observations
            .quiescence_tickets
            .borrow_mut()
            .push(shutdown.clone());
        anyhow::ensure!(
            shutdown.snapshot().window_id() == self.window_id,
            "shutdown must identify the original window"
        );
        if self.observations.panic_on_next_quiesce.replace(false) {
            self.observations
                .quiescence_panics
                .set(self.observations.quiescence_panics.get() + 1);
            self.observations
                .quiescence_panic_retained_work
                .set(!self.in_flight.is_empty());
            panic!("intentional native renderer quiescence panic");
        }
        anyhow::ensure!(
            !self.observations.block_quiescence.get(),
            "intentional native renderer quiescence refusal"
        );
        self.wait_for_submitted_prefix()?;
        self.observations
            .quiesced_while_native_live
            .set(unsafe { IsWindow(Some(self.hwnd)).as_bool() });
        self.observations
            .quiesced_prefix
            .set(self.observations.completed_prefix.load(Ordering::SeqCst));
        self.surface.take();
        self.renderer.take();
        Ok(())
    }
}

struct NativeRendererRoot {
    image: ImageTexture,
    observations: Rc<NativeRendererObservations>,
    activations: usize,
}

impl Render for NativeRendererRoot {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let image = self.image.clone();
        let observations = self.observations.clone();
        div()
            .size_full()
            .bg(rgb(0x162334))
            .child(
                canvas(
                    |_, _, _| {},
                    move |bounds, (), window, _| {
                        if let Err(error) = window.paint_image_texture(
                            Bounds::new(
                                bounds.origin + point(px(16.), px(16.)),
                                size(px(160.), px(80.)),
                            ),
                            Corners::all(px(4.)),
                            &image,
                        ) {
                            observations
                                .paint_errors
                                .borrow_mut()
                                .push(error.to_string());
                        }
                    },
                )
                .size_full(),
            )
            .child(
                div()
                    .id("native-custom-renderer-control")
                    .absolute()
                    .left(px(48.))
                    .top(px(40.))
                    .w(px(88.))
                    .h(px(32.))
                    .bg(rgb(0x20b080))
                    .text_color(white())
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _, _, cx| {
                            this.activations += 1;
                            cx.notify();
                        }),
                    )
                    .child(format!("Count {}", self.activations)),
            )
    }
}

fn wait_for_submission(
    platform: &WindowsPlatform,
    app: &mut Application,
    handle: open_gpui::WindowHandle<NativeRendererRoot>,
    after: u64,
) -> u64 {
    let mut submitted = None;
    pump_messages_until_with_delay(
        &Rc::downgrade(&platform.inner),
        "custom renderer exact nonempty native submission",
        Duration::from_millis(2),
        || {
            submitted = app
                .update_for_test(|cx| {
                    handle.update(cx, |_, window, _| {
                        let facts = window.presentation_facts();
                        let attempt = facts.latest_present_attempt?;
                        (facts.initial_presentation
                            == open_gpui::WindowInitialPresentationStatus::Completed
                            && facts.native_visible
                            && attempt.generation > after
                            && attempt.outcome == PlatformWindowPresentOutcome::Submitted
                            && attempt.contained_valid_primitives
                            && facts.frame_accepted_generation == Some(attempt.generation)
                            && facts.present_submitted_generation == Some(attempt.generation)
                            && facts.non_empty_presented_generation == Some(attempt.generation))
                        .then_some(attempt.generation)
                    })
                })
                .expect("native custom-renderer window must remain live");
            submitted.is_some()
        },
    );
    submitted.expect("exact native submission")
}

#[test]
#[ignore = "requires a native Windows WGPU presentation surface"]
fn native_custom_renderer_submits_imported_texture_resizes_and_retires_exact_gpu_prefix()
-> Result<()> {
    discard_stale_quit_messages();
    let gpu = Arc::new(NativeWgpuDevice::new()?);
    let observations = Rc::new(NativeRendererObservations::default());
    let factory = Rc::new(NativeWgpuFactory {
        gpu: gpu.clone(),
        observations: observations.clone(),
        image: RefCell::new(None),
        texture: RefCell::new(None),
    });
    let text: Arc<dyn PlatformTextSystem> = Arc::new(CosmicTextSystem::new("Segoe UI"));
    let platform = Rc::new(WindowsPlatform::new_with_renderer(
        text.clone(),
        factory.clone(),
    )?);
    platform.connect_initially();
    assert!(platform.is_windowed());
    assert!(platform.graphical_environment().is_some());
    assert!(platform.inner.state.directx_devices.borrow().is_none());
    assert!(Arc::ptr_eq(&platform.text_system(), &text));
    assert!(platform.vsync_owner_live.borrow().is_some());
    let mut app = Application::with_platform(platform.clone()).with_quit_mode(QuitMode::Explicit);
    observations.fail_creation.set(true);
    let failed = app
        .update_for_test(|cx| cx.open_window(WindowOptions::default(), |_, cx| cx.new(|_| Empty)));
    let failure =
        failed.expect_err("factory errors must fail the original window-open transaction");
    assert!(format!("{failure:#}").contains("intentional native renderer factory refusal"));
    assert!(platform.raw_window_handles.read().is_empty());
    assert!(app.update_for_test(|cx| cx.windows().is_empty()));
    observations.fail_creation.set(false);
    let handle = app.update_for_test(|cx| {
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::centered(size(px(480.), px(260.)), cx)),
                focus_on_appearing: false,
                ..WindowOptions::default()
            },
            |_, cx| {
                cx.new(|_| NativeRendererRoot {
                    image: factory
                        .image
                        .borrow()
                        .as_ref()
                        .expect("factory imported texture")
                        .clone(),
                    observations: observations.clone(),
                    activations: 0,
                })
            },
        )
    })?;
    let registration = *platform
        .raw_window_handles
        .read()
        .last()
        .context("custom native registration")?;
    assert_eq!(registration.window_id(), handle.window_id());
    let hwnd = registration.as_raw();
    let native = platform
        .window_from_hwnd(hwnd)
        .context("custom native window")?;
    assert!(!native.state.renderer.borrow().uses_directx());
    let first = wait_for_submission(&platform, &mut app, handle, 0);
    assert!(observations.glyphs_submitted.get() > 0);
    let scale = native.state.scale_factor.get();
    post_message(
        hwnd,
        WM_LBUTTONDOWN,
        WPARAM(MK_LBUTTON.0 as usize),
        mouse_position_lparam((64. * scale) as u16, (56. * scale) as u16),
    );
    post_message(
        hwnd,
        WM_LBUTTONUP,
        WPARAM(0),
        mouse_position_lparam((64. * scale) as u16, (56. * scale) as u16),
    );
    pump_messages_until(
        &Rc::downgrade(&platform.inner),
        "ordinary native control activation",
        || {
            app.update_for_test(|cx| handle.update(cx, |root, _, _| root.activations))
                .unwrap()
                == 1
        },
    );
    let second = wait_for_submission(&platform, &mut app, handle, first);
    observations.defer_submission.set(true);
    let submissions = observations.submissions.get();
    let texture = factory
        .texture
        .borrow()
        .as_ref()
        .context("host viewport texture")?
        .clone();
    write_viewport_texture(&gpu.queue, &texture, UPDATED_IMAGE);
    app.update_for_test(|cx| handle.update(cx, |_, window, _| window.refresh()))?;
    pump_messages_until_with_delay(
        &Rc::downgrade(&platform.inner),
        "explicit deferred renderer attempt",
        Duration::from_millis(2),
        || observations.deferred.get() > 0,
    );
    assert_eq!(observations.submissions.get(), submissions);
    app.update_for_test(|cx| {
        handle.update(cx, |_, window, _| {
            let facts = window.presentation_facts();
            assert_eq!(facts.present_submitted_generation, Some(second));
            assert_eq!(
                facts.latest_present_attempt.unwrap().outcome,
                PlatformWindowPresentOutcome::Deferred
            );
        })
    })?;
    observations.defer_submission.set(false);
    let third = wait_for_submission(&platform, &mut app, handle, second);
    let old_size = native.state.logical_size.get();
    let resize_successes = observations.resize_sizes.borrow().len();
    observations.panic_on_next_resize.set(true);
    unsafe {
        SetWindowPos(
            hwnd,
            None,
            0,
            0,
            (640. * scale) as i32,
            (360. * scale) as i32,
            SWP_NOMOVE | SWP_NOZORDER | SWP_NOACTIVATE,
        )
    }?;
    assert_eq!(observations.resize_panics.get(), 1);
    assert!(!observations.panic_on_next_resize.get());
    let fourth = wait_for_submission(&platform, &mut app, handle, third);
    assert_ne!(native.state.logical_size.get(), old_size);
    let mut rect = RECT::default();
    unsafe { GetClientRect(hwnd, &mut rect) }?;
    let physical_size = size(
        DevicePixels(rect.right - rect.left),
        DevicePixels(rect.bottom - rect.top),
    );
    let logical_size = physical_size.to_pixels(native.state.scale_factor.get());
    assert_eq!(native.state.logical_size.get(), logical_size);
    app.update_for_test(|cx| {
        handle.update(cx, |_, window, _| {
            assert_eq!(window.viewport_size(), logical_size);
            assert_eq!(window.scale_factor(), native.state.scale_factor.get());
        })
    })?;
    assert_eq!(
        observations.submitted_extents.borrow().last().copied(),
        Some((
            (rect.right - rect.left) as u32,
            (rect.bottom - rect.top) as u32
        ))
    );
    assert!(observations.resize_sizes.borrow().len() > resize_successes);
    assert_eq!(
        observations.resize_sizes.borrow().last().copied(),
        Some(physical_size)
    );
    assert_eq!(observations.resize_panics.get(), 1);
    assert!(observations.resize_sizes.borrow().len() >= 2);
    let _ = unsafe { ShowWindow(hwnd, SW_MINIMIZE) };
    assert!(unsafe { IsIconic(hwnd).as_bool() });
    let minimized_prefix = observations.submissions.get();
    app.update_for_test(|cx| handle.update(cx, |_, window, _| window.refresh()))?;
    pump_messages_until_idle(
        &Rc::downgrade(&platform.inner),
        "minimized custom-renderer callback parking",
    );
    assert_eq!(observations.submissions.get(), minimized_prefix);
    let _ = unsafe { ShowWindow(hwnd, SW_RESTORE) };
    let restored = wait_for_submission(&platform, &mut app, handle, fourth);
    assert!(native.registration.matches(registration));
    app.update_for_test(|cx| handle.update(cx, |_, window, _| window.refresh()))?;
    wait_for_submission(&platform, &mut app, handle, restored);
    let blocked_prefix = observations.submissions.get();
    observations.block_quiescence.set(true);
    observations.panic_on_next_quiesce.set(true);
    factory.image.borrow_mut().take();
    app.update_for_test(|cx| handle.update(cx, |_, window, cx| window.remove_window(cx)))?;
    pump_messages_until_with_delay(
        &Rc::downgrade(&platform.inner),
        "custom quiescence panic and refusal retain native owner",
        Duration::from_millis(2),
        || observations.quiescence_attempts.get() >= 2,
    );
    assert_eq!(observations.quiescence_panics.get(), 1);
    assert!(!observations.panic_on_next_quiesce.get());
    assert!(observations.quiescence_panic_retained_work.get());
    assert!(unsafe { IsWindow(Some(hwnd)).as_bool() });
    assert!(platform.raw_window_handles.read().contains(&registration));
    let shutdown = native.presentation_shutdown_ticket();
    assert!(!shutdown.snapshot().quiesced());
    assert!(!shutdown.snapshot().native_terminal());
    assert!(
        observations
            .image_owner
            .borrow()
            .as_ref()
            .unwrap()
            .upgrade()
            .is_some()
    );
    observations.block_quiescence.set(false);
    pump_messages_until_with_delay(
        &Rc::downgrade(&platform.inner),
        "GPU-prefix quiescence before exact HWND retirement",
        Duration::from_millis(2),
        || {
            !unsafe { IsWindow(Some(hwnd)).as_bool() }
                && platform.raw_window_handles.read().is_empty()
                && platform.inner.native_retirement_is_settled()
        },
    );
    assert!(observations.quiesced_while_native_live.get());
    assert_eq!(observations.quiesced_prefix.get(), blocked_prefix);
    assert!(shutdown.same_authority(&native.presentation_shutdown_ticket()));
    assert!(
        observations
            .quiescence_tickets
            .borrow()
            .iter()
            .all(|ticket| ticket.same_authority(&shutdown))
    );
    assert!(shutdown.snapshot().quiesced());
    assert!(shutdown.snapshot().native_terminal());
    assert!(!shutdown.snapshot().protocol_violation());
    assert_eq!(observations.quiescence_panics.get(), 1);
    let lifecycle = platform.lifecycle_test_probe.events();
    let quiesced = lifecycle.iter().position(|event| matches!(event, NativeWindowLifecycleTestEvent::PresentationQuiesced { window_id, .. } if *window_id == handle.window_id())).context("quiescence event")?;
    let terminal = lifecycle.iter().position(|event| matches!(event, NativeWindowLifecycleTestEvent::NativeTerminal { window_id, .. } if *window_id == handle.window_id())).context("native terminal event")?;
    assert!(quiesced < terminal);
    let pixels = observations.pixels.borrow();
    assert!(pixels.iter().any(|sample| sample[1] == INITIAL_IMAGE));
    assert!(pixels.iter().any(|sample| sample[1] == UPDATED_IMAGE));
    for sample in pixels.iter() {
        assert_eq!(sample[0], BACKGROUND);
        assert_eq!(sample[2], CONTROL);
    }
    assert!(observations.paint_errors.borrow().is_empty());
    assert!(
        observations
            .image_owner
            .borrow()
            .as_ref()
            .unwrap()
            .upgrade()
            .is_none()
    );
    assert!(
        observations
            .atlas
            .borrow()
            .as_ref()
            .unwrap()
            .upgrade()
            .is_some()
    );
    gpu.device.poll(wgpu::PollType::Poll)?;
    assert!(
        gpu.errors.lock().is_empty(),
        "host device errors: {:?}",
        gpu.errors.lock()
    );
    assert!(platform.inner.state.directx_devices.borrow().is_none());
    open_gpui::block_on(platform.request_windowing(open_gpui::WindowingRequest::Headless))?;
    assert!(!platform.is_windowed());
    assert!(platform.vsync_owner_live.borrow().is_none());
    open_gpui::block_on(
        platform.request_windowing(open_gpui::WindowingRequest::Windowed(
            open_gpui::GraphicalEnvironment::detect(),
        )),
    )?;
    assert!(platform.is_windowed());
    assert!(platform.vsync_owner_live.borrow().is_some());
    assert!(platform.inner.state.directx_devices.borrow().is_none());
    assert!(Arc::ptr_eq(&platform.text_system(), &text));
    drop(native);
    assert!(
        observations
            .atlas
            .borrow()
            .as_ref()
            .unwrap()
            .upgrade()
            .is_none()
    );
    drop(app);
    drop(platform);
    Ok(())
}
