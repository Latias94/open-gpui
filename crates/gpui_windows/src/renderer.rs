use std::{
    num::NonZeroIsize,
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
    sync::Arc,
};

use anyhow::{Context as _, Result};
use open_gpui::{
    DevicePixels, GpuSpecs, PlatformAtlas, PlatformWindowPresentOutcome, Scene, Size,
    WindowBackgroundAppearance, WindowId, WindowPresentationShutdownTicket,
};
use raw_window_handle::{
    RawDisplayHandle, RawWindowHandle, Win32WindowHandle, WindowsDisplayHandle,
};
use windows::Win32::{Foundation::HWND, UI::WindowsAndMessaging::GWLP_HINSTANCE};

use crate::{DirectXDevices, DirectXRenderer, get_window_long};

/// The native target supplied while GPUI creates a window's renderer on its UI thread.
///
/// GPUI owns the native UI binding and retirement. Ordinary windows also use GPUI's HWND and
/// message loop; external hosts retain their original window owner and loop. The HWND remains
/// valid until this renderer successfully quiesces, unless the host or operating system destroys
/// it early. A renderer must release every surface reference before quiescence succeeds.
/// This value is not an independent ownership lease for the HWND.
#[derive(Clone, Copy, Debug)]
pub struct WindowsRendererWindow {
    window_id: WindowId,
    hwnd: NonZeroIsize,
    hinstance: NonZeroIsize,
}

impl WindowsRendererWindow {
    pub(crate) fn new(window_id: WindowId, hwnd: HWND) -> Result<Self> {
        Ok(Self {
            window_id,
            hwnd: NonZeroIsize::new(hwnd.0 as isize)
                .context("renderer requires a non-null HWND")?,
            hinstance: NonZeroIsize::new(unsafe { get_window_long(hwnd, GWLP_HINSTANCE) })
                .context("renderer requires the window's original HINSTANCE")?,
        })
    }

    /// The full GPUI window identity associated with this renderer.
    pub fn window_id(self) -> WindowId {
        self.window_id
    }

    /// The borrowed native target for a renderer's platform surface construction.
    ///
    /// When using an unsafe graphics surface constructor, the renderer must uphold the native
    /// lifetime documented on this type. Copying this handle does not extend that lifetime.
    pub fn raw_window_handle(self) -> RawWindowHandle {
        let mut handle = Win32WindowHandle::new(self.hwnd);
        handle.hinstance = Some(self.hinstance);
        RawWindowHandle::Win32(handle)
    }

    /// The Windows display handle paired with this native target.
    pub fn raw_display_handle(self) -> RawDisplayHandle {
        RawDisplayHandle::Windows(WindowsDisplayHandle::new())
    }
}

/// Creates one renderer per native window using application-owned graphics resources.
///
/// Creation runs on the UI thread during native window construction. It must not enter GPUI,
/// dispatch native messages or install a second native window owner. Errors abort the original
/// window-open transaction. Shared graphics devices and their error callbacks remain owned by
/// the application; GPUI creates no Direct3D device for this factory.
pub trait WindowsRendererFactory: 'static {
    /// Creates a renderer whose atlas remains stable for this window's lifetime.
    fn create(&self, window: WindowsRendererWindow) -> Result<Box<dyn WindowsRenderer>>;
}

/// A synchronous renderer for GPUI's existing Win32 window and retirement owner.
///
/// Calls run on the UI thread and must not reenter GPUI or pump native messages. The renderer
/// owns its surface and retains exact scene resources through the corresponding GPU work.
/// Device loss and recovery belong to the implementation: return `RepaintRequired` when old
/// scenes become unusable, and retain the original atlas authority across recovery.
pub trait WindowsRenderer: 'static {
    /// Returns the original atlas used by the Window to prepare every scene.
    fn sprite_atlas(&self) -> Arc<dyn PlatformAtlas>;

    /// Resizes the native presentation target to the current physical client extent.
    ///
    /// Errors reject presentation until a later resize succeeds. Minimized windows retain
    /// their existing target and are resized when native visibility permits presentation.
    /// Unwinding panics from custom providers are reported as errors and retain the resize
    /// for retry, so the implementation must remain retryable after a failed call.
    fn resize(&mut self, size: Size<DevicePixels>) -> Result<()>;

    /// Submits this exact scene synchronously to the renderer's presentation path.
    ///
    /// `Submitted` requires an actual queue submission and presentation handoff before this
    /// method returns. Recording commands or retaining a scene for later execution is not
    /// submission. `Deferred` retains demand; `RepaintRequired` requests a fresh scene. Errors
    /// are reported as rejected attempts and never establish submitted-frame evidence.
    fn draw(
        &mut self,
        scene: &Scene,
        background: WindowBackgroundAppearance,
    ) -> Result<PlatformWindowPresentOutcome>;

    /// Settles all in-flight work and releases native surface references before HWND retirement.
    ///
    /// GPUI retains the renderer and retries on error, using this exact shutdown authority.
    /// Success must make future surface use impossible and retain resources until their exact
    /// submitted GPU prefix has completed. Do not destroy the HWND or the application's shared
    /// device. GPUI acknowledges the shutdown ticket only after successful quiescence.
    /// Unwinding panics from custom providers are reported as errors and retry with the same
    /// shutdown authority.
    fn quiesce(&mut self, shutdown: &WindowPresentationShutdownTicket) -> Result<()>;

    /// Whether this renderer and target support subpixel text rendering.
    fn is_subpixel_rendering_supported(&self) -> bool {
        false
    }

    /// Optional graphics diagnostics without taking device callback ownership.
    fn gpu_specs(&self) -> Result<Option<GpuSpecs>> {
        Ok(None)
    }

    /// Captures a scene using the implementation's own graphics resources.
    #[cfg(any(test, feature = "test-support"))]
    fn render_to_image(
        &mut self,
        _scene: &Scene,
        _background: WindowBackgroundAppearance,
    ) -> Result<image::RgbaImage> {
        anyhow::bail!("this Windows renderer does not support image capture")
    }
}

#[derive(Clone)]
pub(crate) enum WindowsRendererSource {
    DirectX(DirectXDevices),
    Custom(Rc<dyn WindowsRendererFactory>),
}

impl WindowsRendererSource {
    pub(crate) fn create(
        &self,
        window: WindowsRendererWindow,
        hwnd: HWND,
        disable_direct_composition: bool,
    ) -> Result<WindowsWindowRenderer> {
        let backend = match self {
            Self::DirectX(devices) => WindowsRendererBackend::DirectX(DirectXRenderer::new(
                hwnd,
                devices,
                disable_direct_composition,
            )?),
            Self::Custom(factory) => WindowsRendererBackend::Custom(factory.create(window)?),
        };
        Ok(WindowsWindowRenderer {
            backend,
            pending_resize: None,
            quiesced_for: None,
        })
    }
}

enum WindowsRendererBackend {
    DirectX(DirectXRenderer),
    Custom(Box<dyn WindowsRenderer>),
}

pub(crate) struct WindowsWindowRenderer {
    backend: WindowsRendererBackend,
    pending_resize: Option<Size<DevicePixels>>,
    quiesced_for: Option<WindowPresentationShutdownTicket>,
}

impl WindowsWindowRenderer {
    fn renderer(&self) -> &dyn WindowsRenderer {
        match &self.backend {
            WindowsRendererBackend::DirectX(renderer) => renderer,
            WindowsRendererBackend::Custom(renderer) => renderer.as_ref(),
        }
    }

    fn renderer_mut(&mut self) -> &mut dyn WindowsRenderer {
        match &mut self.backend {
            WindowsRendererBackend::DirectX(renderer) => renderer,
            WindowsRendererBackend::Custom(renderer) => renderer.as_mut(),
        }
    }

    pub(crate) fn uses_directx(&self) -> bool {
        matches!(self.backend, WindowsRendererBackend::DirectX(_))
    }

    pub(crate) fn sprite_atlas(&self) -> Arc<dyn PlatformAtlas> {
        self.renderer().sprite_atlas()
    }

    pub(crate) fn resize(&mut self, size: Size<DevicePixels>) -> Result<()> {
        if self.quiesced_for.is_some() {
            return Ok(());
        }
        match &mut self.backend {
            WindowsRendererBackend::DirectX(renderer) => renderer.resize(size)?,
            WindowsRendererBackend::Custom(renderer) => {
                self.pending_resize = Some(size);
                call_custom_renderer("resize", || renderer.resize(size))?;
            }
        }
        self.pending_resize = None;
        Ok(())
    }

    pub(crate) fn draw(
        &mut self,
        scene: &Scene,
        background: WindowBackgroundAppearance,
    ) -> Result<PlatformWindowPresentOutcome> {
        if self.quiesced_for.is_some() {
            return Ok(PlatformWindowPresentOutcome::Rejected);
        }
        if let Some(size) = self.pending_resize {
            self.resize(size)?;
        }
        self.renderer_mut().draw(scene, background)
    }

    pub(crate) fn handle_device_lost(&mut self, devices: &DirectXDevices) -> Result<()> {
        if let WindowsRendererBackend::DirectX(renderer) = &mut self.backend {
            renderer.handle_device_lost(devices)?;
        }
        Ok(())
    }

    pub(crate) fn quiesce_surface(
        &mut self,
        shutdown: &WindowPresentationShutdownTicket,
    ) -> Result<()> {
        if let Some(current) = &self.quiesced_for {
            anyhow::ensure!(
                current.same_authority(shutdown),
                "a stale shutdown ticket cannot quiesce this Windows renderer"
            );
            return Ok(());
        }
        match &mut self.backend {
            WindowsRendererBackend::DirectX(renderer) => renderer.quiesce(shutdown)?,
            WindowsRendererBackend::Custom(renderer) => {
                call_custom_renderer("quiesce", || renderer.quiesce(shutdown))?;
            }
        }
        anyhow::ensure!(
            shutdown.acknowledge_quiesced(),
            "native terminal preceded Windows renderer quiescence"
        );
        self.quiesced_for = Some(shutdown.clone());
        self.pending_resize = None;
        Ok(())
    }

    pub(crate) fn is_quiesced_for(&self, shutdown: &WindowPresentationShutdownTicket) -> bool {
        self.quiesced_for
            .as_ref()
            .is_some_and(|current| current.same_authority(shutdown))
            && match &self.backend {
                WindowsRendererBackend::DirectX(renderer) => renderer.is_quiesced_for(shutdown),
                WindowsRendererBackend::Custom(_) => true,
            }
    }

    pub(crate) fn is_subpixel_rendering_supported(&self) -> bool {
        self.renderer().is_subpixel_rendering_supported()
    }

    pub(crate) fn gpu_specs(&self) -> Result<Option<GpuSpecs>> {
        self.renderer().gpu_specs()
    }

    #[cfg(any(test, feature = "test-support"))]
    pub(crate) fn render_to_image(
        &mut self,
        scene: &Scene,
        background: WindowBackgroundAppearance,
    ) -> Result<image::RgbaImage> {
        self.renderer_mut().render_to_image(scene, background)
    }
}

fn call_custom_renderer(
    operation: &'static str,
    callback: impl FnOnce() -> Result<()>,
) -> Result<()> {
    match catch_unwind(AssertUnwindSafe(callback)) {
        Ok(result) => result,
        Err(payload) => {
            let message = payload
                .downcast_ref::<String>()
                .map(String::as_str)
                .or_else(|| payload.downcast_ref::<&str>().copied())
                .unwrap_or("non-string panic payload");
            let error = anyhow::anyhow!("custom Windows renderer {operation} panicked: {message}");
            std::mem::forget(payload);
            Err(error)
        }
    }
}

impl WindowsRenderer for DirectXRenderer {
    fn sprite_atlas(&self) -> Arc<dyn PlatformAtlas> {
        DirectXRenderer::sprite_atlas(self)
    }

    fn resize(&mut self, size: Size<DevicePixels>) -> Result<()> {
        DirectXRenderer::resize(self, size)
    }

    fn draw(
        &mut self,
        scene: &Scene,
        background: WindowBackgroundAppearance,
    ) -> Result<PlatformWindowPresentOutcome> {
        DirectXRenderer::draw(self, scene, background)
    }

    fn quiesce(&mut self, shutdown: &WindowPresentationShutdownTicket) -> Result<()> {
        self.quiesce_surface(shutdown)
    }

    fn is_subpixel_rendering_supported(&self) -> bool {
        true
    }

    fn gpu_specs(&self) -> Result<Option<GpuSpecs>> {
        DirectXRenderer::gpu_specs(self).map(Some)
    }

    #[cfg(any(test, feature = "test-support"))]
    fn render_to_image(
        &mut self,
        scene: &Scene,
        background: WindowBackgroundAppearance,
    ) -> Result<image::RgbaImage> {
        DirectXRenderer::render_to_image(self, scene, background)
    }
}
