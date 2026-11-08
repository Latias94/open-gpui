#![cfg(all(target_os = "windows", feature = "vulkan-surface"))]

//! Public native Vulkan lifetime controls for upstream
//! `3b3506ea31154edc04de64debda0e780cba2639c`.
//!
//! The clear control submits before dropping the surface and waits afterward;
//! it does not assume a minimum GPU execution time or require presentation.

use open_gpui::block_on;
use raw_window_handle::{
    RawDisplayHandle, RawWindowHandle, Win32WindowHandle, WindowsDisplayHandle,
};
use std::{
    num::NonZeroIsize,
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};
use windows::{
    Win32::{
        Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, RECT, WPARAM},
        System::LibraryLoader::GetModuleHandleW,
        UI::WindowsAndMessaging::{
            CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetClientRect, MSG,
            PM_REMOVE, PeekMessageW, RegisterClassExW, SW_SHOWNOACTIVATE, ShowWindow,
            UnregisterClassW, WNDCLASSEXW, WS_EX_NOACTIVATE, WS_OVERLAPPEDWINDOW,
        },
    },
    core::{PCWSTR, w},
};

static NEXT_WINDOW_CLASS: AtomicU64 = AtomicU64::new(0);

struct NativeWindow {
    handle: Option<HWND>,
    instance: HINSTANCE,
    class_name: Vec<u16>,
    class_registered: bool,
}

impl NativeWindow {
    fn new() -> windows::core::Result<Self> {
        // SAFETY: A null module name borrows this executable's loaded module.
        let instance = HINSTANCE(unsafe { GetModuleHandleW(None)? }.0);
        let class_name = format!(
            "OpenGpuiVulkanSurface-{}-{}",
            std::process::id(),
            NEXT_WINDOW_CLASS.fetch_add(1, Ordering::Relaxed),
        )
        .encode_utf16()
        .chain(Some(0))
        .collect::<Vec<_>>();
        let class = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            lpfnWndProc: Some(window_proc),
            hInstance: instance,
            lpszClassName: PCWSTR(class_name.as_ptr()),
            ..Default::default()
        };
        // SAFETY: The descriptor and terminated class name are valid for this call;
        // the callback has process lifetime, and this owner unregisters the class.
        if unsafe { RegisterClassExW(&class) } == 0 {
            return Err(windows::core::Error::from_thread());
        }
        let mut window = Self {
            handle: None,
            instance,
            class_name,
            class_registered: true,
        };
        // SAFETY: This thread owns the registered class and the resulting HWND;
        // no borrowed application data is passed to the window procedure.
        let handle = unsafe {
            CreateWindowExW(
                WS_EX_NOACTIVATE,
                PCWSTR(window.class_name.as_ptr()),
                w!("Vulkan acquired-texture lifetime regression"),
                WS_OVERLAPPEDWINDOW,
                0,
                0,
                128,
                128,
                None,
                None,
                Some(instance),
                None,
            )?
        };
        window.handle = Some(handle);
        // SAFETY: The HWND was just created on this thread and is still owned here.
        let _ = unsafe { ShowWindow(handle, SW_SHOWNOACTIVATE) };
        window.pump_messages();
        Ok(window)
    }

    fn handle(&self) -> HWND {
        self.handle.expect("the native window must still be owned")
    }

    fn pump_messages(&self) {
        let mut message = MSG::default();
        for _ in 0..64 {
            // SAFETY: The output is initialized and the filter is this live HWND
            // on its creating thread; dispatch invokes only the owned procedure.
            unsafe {
                if !PeekMessageW(&mut message, Some(self.handle()), 0, 0, PM_REMOVE).as_bool() {
                    break;
                }
                DispatchMessageW(&message);
            }
        }
    }

    fn dimensions(&self) -> (u32, u32) {
        let mut rectangle = RECT::default();
        // SAFETY: The live HWND belongs to this thread and rectangle is writable.
        unsafe { GetClientRect(self.handle(), &mut rectangle) }
            .expect("the Vulkan fixture must have a client rectangle");
        let width = u32::try_from(rectangle.right - rectangle.left)
            .expect("the Vulkan fixture must have a nonnegative client width");
        let height = u32::try_from(rectangle.bottom - rectangle.top)
            .expect("the Vulkan fixture must have a nonnegative client height");
        assert!(width > 0 && height > 0);
        (width, height)
    }

    fn raw_window_handle(&self) -> RawWindowHandle {
        let mut handle = Win32WindowHandle::new(
            NonZeroIsize::new(self.handle().0 as isize)
                .expect("CreateWindowExW returned a live HWND"),
        );
        handle.hinstance = NonZeroIsize::new(self.instance.0 as isize);
        RawWindowHandle::Win32(handle)
    }

    fn close(&mut self) -> windows::core::Result<()> {
        if let Some(handle) = self.handle {
            // SAFETY: This owner destroys its own HWND on the creating thread,
            // after all surfaces, textures, and GPU owners have been retired.
            unsafe { DestroyWindow(handle)? };
            self.handle = None;
        }
        if self.class_registered {
            // SAFETY: The window is gone and the class name and module still live.
            unsafe {
                UnregisterClassW(PCWSTR(self.class_name.as_ptr()), Some(self.instance))?;
            }
            self.class_registered = false;
        }
        Ok(())
    }
}

impl Drop for NativeWindow {
    fn drop(&mut self) {
        if let Err(error) = self.close() {
            eprintln!("native Vulkan fixture cleanup failed: {error}");
        }
    }
}

unsafe extern "system" fn window_proc(
    handle: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    // SAFETY: Windows supplies these parameters to the registered window
    // procedure; this fixture has no custom message state or borrowed callbacks.
    unsafe { DefWindowProcW(handle, message, wparam, lparam) }
}

#[test]
#[ignore = "requires a Windows desktop and an actual Vulkan adapter supporting a Win32 surface"]
fn vulkan_surface_can_drop_while_an_acquired_texture_is_alive() {
    drop_surface_with_acquired_texture(false);
}

#[test]
#[ignore = "requires a Windows desktop and an actual Vulkan adapter supporting a Win32 surface"]
fn vulkan_surface_can_drop_after_submitting_a_clear_before_waiting() {
    drop_surface_with_acquired_texture(true);
}

fn drop_surface_with_acquired_texture(submit_clear: bool) {
    let mut window =
        NativeWindow::new().expect("the Vulkan regression requires a real Win32 window");
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
        backends: wgpu::Backends::VULKAN,
        flags: wgpu::InstanceFlags::DEBUG | wgpu::InstanceFlags::VALIDATION,
        ..wgpu::InstanceDescriptor::new_without_display_handle()
    });
    // SAFETY: This test owns the HWND on its creating thread and explicitly
    // retires the surface, acquired texture, GPU work, and instance before HWND
    // destruction. The Windows display handle borrows no external connection.
    let surface = unsafe {
        instance.create_surface_unsafe(wgpu::SurfaceTargetUnsafe::RawHandle {
            raw_display_handle: Some(RawDisplayHandle::Windows(WindowsDisplayHandle::new())),
            raw_window_handle: window.raw_window_handle(),
        })
    }
    .expect("the Vulkan instance must create a real Win32 surface");
    let adapter = block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        compatible_surface: Some(&surface),
        force_fallback_adapter: false,
        ..Default::default()
    }))
    .expect("an actual Vulkan adapter with Win32 presentation is required; this test cannot skip");
    let adapter_info = adapter.get_info();
    assert_eq!(adapter_info.backend, wgpu::Backend::Vulkan);
    eprintln!("native Vulkan surface regression adapter: {adapter_info:?}");
    let (device, queue) = block_on(
        adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("ordinary consumer Vulkan surface regression"),
            required_limits: wgpu::Limits::downlevel_defaults()
                .using_resolution(adapter.limits())
                .using_alignment(adapter.limits()),
            ..Default::default()
        }),
    )
    .expect("the actual Vulkan adapter must create a device and queue");
    let uncaptured_errors = Arc::new(Mutex::new(Vec::new()));
    device.on_uncaptured_error(Arc::new({
        let errors = Arc::clone(&uncaptured_errors);
        move |error: wgpu::Error| {
            errors
                .lock()
                .expect("the Vulkan error collector must not be poisoned")
                .push(error.to_string());
        }
    }));
    let validation = device.push_error_scope(wgpu::ErrorFilter::Validation);
    let (width, height) = window.dimensions();
    let configuration = surface
        .get_default_config(&adapter, width, height)
        .expect("the actual Vulkan surface must support a renderable configuration");
    surface.configure(&device, &configuration);
    let acquired = match surface.get_current_texture() {
        wgpu::CurrentSurfaceTexture::Success(texture)
        | wgpu::CurrentSurfaceTexture::Suboptimal(texture) => texture,
        status => panic!("the real Vulkan surface must acquire a texture, received {status:?}"),
    };
    let submission = if submit_clear {
        let view = acquired
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Vulkan clear submitted before surface retirement"),
        });
        {
            let _pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("acquired Vulkan texture clear"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.25,
                            g: 0.5,
                            b: 0.75,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                ..Default::default()
            });
        }
        Some(queue.submit([encoder.finish()]))
    } else {
        None
    };

    drop(surface);
    assert_eq!(acquired.texture.width(), width);
    assert_eq!(acquired.texture.height(), height);
    if let Some(submission_index) = submission {
        device
            .poll(wgpu::PollType::Wait {
                submission_index: Some(submission_index),
                timeout: Some(Duration::from_secs(10)),
            })
            .expect("submitted Vulkan clear work must finish after surface retirement");
    }
    drop(acquired);
    device
        .poll(wgpu::PollType::Wait {
            submission_index: None,
            timeout: Some(Duration::from_secs(10)),
        })
        .expect("Vulkan texture retirement must finish within the bounded GPU wait");
    let validation_error = block_on(validation.pop());
    assert!(
        validation_error.is_none(),
        "surface retirement with a live acquired texture failed validation: {validation_error:?}"
    );
    drop(queue);
    drop(device);
    drop(adapter);
    drop(instance);
    window
        .close()
        .expect("the Vulkan fixture must destroy its HWND and unregister its class");
    let errors = uncaptured_errors
        .lock()
        .expect("the Vulkan error collector must not be poisoned");
    assert!(
        errors.is_empty(),
        "Vulkan teardown reported uncaptured errors: {errors:?}"
    );
}

#[cfg(feature = "wgpu-renderer")]
#[test]
#[ignore = "requires a Windows desktop and an actual Vulkan adapter supporting two Win32 surfaces"]
fn native_wgpu_renderer_initializes_shares_resumes_and_retires_real_surfaces() {
    use open_gpui::{
        Bounds, DevicePixels, PlatformWindowPresentOutcome, Quad, ScaledPixels, Scene, WindowId,
        WindowPresentationShutdownTicket, point, rgb, size,
    };
    use open_gpui_wgpu::{
        GpuContext, WgpuRenderer, WgpuSurfaceConfig, WgpuSurfaceShutdownProgress,
    };
    use raw_window_handle::{
        DisplayHandle, HandleError, HasDisplayHandle, HasWindowHandle, WindowHandle,
    };
    use std::{cell::RefCell, rc::Rc};

    /// Borrows handle values; the test retains both native owners on this thread until all
    /// renderers, shared contexts, devices, queues, and surface factories have been dropped.
    #[derive(Clone, Copy, Debug)]
    struct BorrowedNativeWindow {
        window: NonZeroIsize,
        instance: NonZeroIsize,
    }

    impl HasWindowHandle for BorrowedNativeWindow {
        fn window_handle(&self) -> Result<WindowHandle<'_>, HandleError> {
            let mut handle = Win32WindowHandle::new(self.window);
            handle.hinstance = Some(self.instance);
            // SAFETY: The original NativeWindow retains this HWND through renderer and context
            // teardown, and only the creating thread destroys it after these borrows are gone.
            Ok(unsafe { WindowHandle::borrow_raw(RawWindowHandle::Win32(handle)) })
        }
    }

    impl HasDisplayHandle for BorrowedNativeWindow {
        fn display_handle(&self) -> Result<DisplayHandle<'_>, HandleError> {
            Ok(DisplayHandle::windows())
        }
    }

    let mut first_window =
        NativeWindow::new().expect("the first renderer requires a real owned Win32 window");
    let mut second_window =
        NativeWindow::new().expect("the shared renderer requires another owned Win32 window");
    let borrowed_handle = |window: &NativeWindow| BorrowedNativeWindow {
        window: NonZeroIsize::new(window.handle().0 as isize).expect("the owned HWND must be live"),
        instance: NonZeroIsize::new(window.instance.0 as isize)
            .expect("the executable must have a live module handle"),
    };
    let drawable_size = |window: &NativeWindow| {
        let (width, height) = window.dimensions();
        size(
            DevicePixels(i32::try_from(width).expect("the fixture width must fit device pixels")),
            DevicePixels(i32::try_from(height).expect("the fixture height must fit device pixels")),
        )
    };
    let first_handle = borrowed_handle(&first_window);
    let second_handle = borrowed_handle(&second_window);
    let zero_size = size(DevicePixels(0), DevicePixels(0));
    let gpu_context: GpuContext = Rc::new(RefCell::new(None));
    let context_lifetime = Rc::downgrade(&gpu_context);
    assert!(gpu_context.borrow().is_none());
    let mut first_renderer = WgpuRenderer::new(
        Rc::clone(&gpu_context),
        &first_handle,
        WgpuSurfaceConfig {
            size: zero_size,
            transparent: false,
            preferred_present_mode: Some(wgpu::PresentMode::Fifo),
        },
        None,
    )
    .expect("the first zero-size renderer must initialize a context using the real surface");
    let (device, queue, device_errors) = {
        let context = gpu_context.borrow();
        let context = context
            .as_ref()
            .expect("the first renderer must publish its initialized GPU context");
        let adapter_info = context.adapter.get_info();
        assert_eq!(adapter_info.backend, wgpu::Backend::Vulkan);
        eprintln!("ordinary native WgpuRenderer regression adapter: {adapter_info:?}");
        (
            Arc::clone(&context.device),
            Arc::clone(&context.queue),
            Arc::clone(context.errors()),
        )
    };
    let validation = device.push_error_scope(wgpu::ErrorFilter::Validation);
    let mut scene = Scene::default();
    let mut quad = Quad::default();
    quad.bounds = Bounds::new(
        point(ScaledPixels(4.0), ScaledPixels(4.0)),
        size(ScaledPixels(24.0), ScaledPixels(24.0)),
    );
    quad.clip.conservative_bounds = quad.bounds;
    quad.background = rgb(0xe63946).into();
    scene.quads.push(quad);
    scene.finish();
    assert!(scene.batches().next().is_some());
    assert_eq!(first_renderer.viewport_size(), zero_size);
    assert_eq!(
        first_renderer.draw(&scene),
        PlatformWindowPresentOutcome::Deferred,
        "a zero-size first context must defer instead of acquiring an invalid drawable"
    );
    let first_size = drawable_size(&first_window);
    first_renderer.update_drawable_size(first_size);
    assert_eq!(first_renderer.viewport_size(), first_size);
    assert_eq!(
        first_renderer.draw(&scene),
        PlatformWindowPresentOutcome::Submitted,
        "the first context must configure and submit a nonempty scene after resuming"
    );
    let first_atlas_lifetime = Arc::downgrade(first_renderer.sprite_atlas());

    let second_size = drawable_size(&second_window);
    let mut second_renderer = WgpuRenderer::new(
        Rc::clone(&gpu_context),
        &second_handle,
        WgpuSurfaceConfig {
            size: second_size,
            transparent: false,
            preferred_present_mode: Some(wgpu::PresentMode::Fifo),
        },
        None,
    )
    .expect("the second real surface must initialize its renderer using the shared context");
    {
        let context = gpu_context.borrow();
        let context = context
            .as_ref()
            .expect("the shared renderer must preserve the initialized context");
        assert!(Arc::ptr_eq(&context.device, &device));
        assert!(Arc::ptr_eq(&context.queue, &queue));
        assert!(Arc::ptr_eq(context.errors(), &device_errors));
    }
    let second_atlas_lifetime = Arc::downgrade(second_renderer.sprite_atlas());
    assert_eq!(second_renderer.viewport_size(), second_size);
    assert_eq!(
        second_renderer.draw(&scene),
        PlatformWindowPresentOutcome::Submitted,
        "a renderer joining the shared context must configure and submit its own real surface"
    );
    second_renderer.update_drawable_size(zero_size);
    assert_eq!(
        second_renderer.draw(&scene),
        PlatformWindowPresentOutcome::Deferred
    );
    second_renderer.update_drawable_size(second_size);
    assert_eq!(
        second_renderer.draw(&scene),
        PlatformWindowPresentOutcome::Submitted,
        "the shared-context surface must submit again after zero-size suspension"
    );
    assert!(!first_renderer.surface_owner_release_is_safe());
    assert!(!second_renderer.surface_owner_release_is_safe());
    device
        .poll(wgpu::PollType::Wait {
            submission_index: None,
            timeout: Some(Duration::from_secs(10)),
        })
        .expect("both real renderer submissions must complete within the bounded GPU wait");

    let first_shutdown = WindowPresentationShutdownTicket::new(WindowId::from(1), 1);
    let second_shutdown = WindowPresentationShutdownTicket::new(WindowId::from(2), 1);
    for (renderer, shutdown) in [
        (&mut first_renderer, &first_shutdown),
        (&mut second_renderer, &second_shutdown),
    ] {
        assert_eq!(
            renderer.quiesce_surface(shutdown),
            WgpuSurfaceShutdownProgress::Quiesced
        );
        assert!(renderer.is_quiesced_for(shutdown));
        assert!(renderer.surface_owner_release_is_safe());
        assert!(shutdown.snapshot().quiesced());
        assert!(!shutdown.snapshot().native_terminal());
        assert_eq!(
            renderer.draw(&scene),
            PlatformWindowPresentOutcome::Deferred,
            "a quiesced real surface must refuse further rendering before HWND destruction"
        );
    }
    drop(second_renderer);
    drop(first_renderer);
    assert!(second_atlas_lifetime.upgrade().is_none());
    assert!(first_atlas_lifetime.upgrade().is_none());
    drop(gpu_context);
    assert!(context_lifetime.upgrade().is_none());
    device
        .poll(wgpu::PollType::Wait {
            submission_index: None,
            timeout: Some(Duration::from_secs(10)),
        })
        .expect("renderer resource retirement must finish before native window destruction");
    let validation_error = block_on(validation.pop());
    assert!(
        validation_error.is_none(),
        "initial and shared-context native surface rendering failed validation: {validation_error:?}"
    );
    drop(queue);
    drop(device);
    let mut error_generation = 0;
    let uncaptured_error = device_errors.observe_error(&mut error_generation);
    assert!(
        uncaptured_error.is_none(),
        "native renderer initialization or retirement reported an error: {uncaptured_error:?}"
    );
    assert!(!device_errors.device_lost());
    first_window
        .close()
        .expect("the first HWND must retire only after its renderer and shared GPU owners");
    assert!(first_shutdown.acknowledge_native_terminal());
    second_window
        .close()
        .expect("the second HWND must retire only after its renderer and shared GPU owners");
    assert!(second_shutdown.acknowledge_native_terminal());
    assert!(!first_shutdown.snapshot().protocol_violation());
    assert!(!second_shutdown.snapshot().protocol_violation());
}
