//! Real deferred Window tokens revoked before renderer access and native retirement.
//!
//! The custom provider uses DirectX's atlas and surface lifetime, but never submits its deferred
//! scenes. These checks qualify token settlement and HWND retirement, not rendered pixels.

use super::*;
use crate::{
    DirectXDevices, DirectXRenderer, WindowsRenderer, WindowsRendererFactory, WindowsRendererWindow,
};
use anyhow::{Context as _, Result};
use open_gpui::{
    NoopTextSystem, PlatformAtlas, PlatformFrameDeliveryMode, PlatformFrameSubmission,
    PlatformFrameSubmissionStatus, PlatformPresentationShutdownOutcome, RequestFrameOptions, Scene,
    Size, WindowBackgroundAppearance, WindowPresentationShutdownTicket, rgb,
};
use raw_window_handle::RawWindowHandle;

#[derive(Default)]
struct DeferredFrameObservations {
    pending: RefCell<Option<PlatformFrameSubmission>>,
    atlas: RefCell<Option<std::sync::Weak<dyn PlatformAtlas>>>,
    shutdowns: RefCell<Vec<WindowPresentationShutdownTicket>>,
}

struct DeferredFrameFactory {
    devices: DirectXDevices,
    observations: Rc<DeferredFrameObservations>,
}

impl WindowsRendererFactory for DeferredFrameFactory {
    fn create(&self, window: WindowsRendererWindow) -> Result<Box<dyn WindowsRenderer>> {
        let RawWindowHandle::Win32(handle) = window.raw_window_handle() else {
            anyhow::bail!("deferred native renderer requires its original Win32 target");
        };
        let hwnd = HWND(handle.hwnd.get() as *mut std::ffi::c_void);
        let renderer = DirectXRenderer::new(hwnd, &self.devices, true)?;
        *self.observations.atlas.borrow_mut() =
            Some(Arc::downgrade(&WindowsRenderer::sprite_atlas(&renderer)));
        Ok(Box::new(DeferredFrameRenderer {
            renderer,
            hwnd,
            window_id: window.window_id(),
            observations: self.observations.clone(),
        }))
    }
}

struct DeferredFrameRenderer {
    renderer: DirectXRenderer,
    hwnd: HWND,
    window_id: WindowId,
    observations: Rc<DeferredFrameObservations>,
}

impl WindowsRenderer for DeferredFrameRenderer {
    fn sprite_atlas(&self) -> Arc<dyn PlatformAtlas> {
        WindowsRenderer::sprite_atlas(&self.renderer)
    }

    fn frame_delivery_mode(&self) -> PlatformFrameDeliveryMode {
        PlatformFrameDeliveryMode::Deferred
    }

    fn resize(&mut self, size: Size<DevicePixels>) -> Result<()> {
        self.renderer.resize(size)
    }

    fn draw(
        &mut self,
        _: &Scene,
        _: WindowBackgroundAppearance,
    ) -> Result<PlatformWindowPresentOutcome> {
        anyhow::bail!("deferred Window must deliver its original submission token");
    }

    fn draw_with_submission(
        &mut self,
        scene: &Scene,
        _: WindowBackgroundAppearance,
        submission: &PlatformFrameSubmission,
    ) -> Result<PlatformWindowPresentOutcome> {
        anyhow::ensure!(submission.window_id() == self.window_id);
        anyhow::ensure!(submission.is_pending());
        anyhow::ensure!(!scene.quads.is_empty(), "real Render must paint a quad");
        anyhow::ensure!(
            !self
                .observations
                .pending
                .borrow()
                .as_ref()
                .is_some_and(|pending| pending.status() == PlatformFrameSubmissionStatus::Pending),
            "another pending attempt cannot replace the original token"
        );
        self.observations.pending.replace(Some(submission.clone()));
        Ok(PlatformWindowPresentOutcome::Deferred)
    }

    fn quiesce(&mut self, shutdown: &WindowPresentationShutdownTicket) -> Result<()> {
        anyhow::ensure!(shutdown.snapshot().window_id() == self.window_id);
        anyhow::ensure!(unsafe { IsWindow(Some(self.hwnd)).as_bool() });
        self.renderer.quiesce_surface(shutdown)?;
        self.observations
            .shutdowns
            .borrow_mut()
            .push(shutdown.clone());
        Ok(())
    }
}

struct DeferredFrameRoot;

impl Render for DeferredFrameRoot {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().size_full().bg(rgb(0x264568))
    }
}

#[test]
fn native_deferred_token_revocation_survives_renderer_borrow_and_retires_original_owner()
-> Result<()> {
    discard_stale_quit_messages();
    let observations = Rc::new(DeferredFrameObservations::default());
    let platform = Rc::new(WindowsPlatform::new_with_renderer(
        Arc::new(NoopTextSystem::new()),
        Rc::new(DeferredFrameFactory {
            devices: DirectXDevices::new()?,
            observations: observations.clone(),
        }),
    )?);
    platform.connect_initially();
    let platform_hwnd = platform.handle;
    let platform_lifetime = Rc::downgrade(&platform.inner);
    let lifecycle = platform.lifecycle_test_probe.clone();
    let mut app = Application::with_platform(platform.clone()).with_quit_mode(QuitMode::Explicit);
    let handle = app.update_for_test(|cx| {
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::centered(size(px(320.), px(220.)), cx)),
                focus_on_appearing: false,
                show: false,
                ..WindowOptions::default()
            },
            |_, cx| cx.new(|_| DeferredFrameRoot),
        )
    })?;
    let registration = {
        let registered = platform.raw_window_handles.read();
        assert_eq!(registered.len(), 1);
        registered[0]
    };
    assert_eq!(registration.window_id(), handle.window_id());
    let hwnd = registration.as_raw();
    let native = platform
        .window_from_hwnd(hwnd)
        .context("original deferred HWND owner")?;
    let native_lifetime = Rc::downgrade(&native);
    let mut pending = None;
    pump_messages_until_with_delay(
        &platform_lifetime,
        "real nonempty deferred Window submission",
        Duration::from_millis(2),
        || {
            let submission = observations.pending.borrow().clone();
            pending = submission.filter(PlatformFrameSubmission::is_pending);
            if pending.is_none() {
                // Initial placement notifications can revoke the first candidate. A hidden
                // HWND has no ordinary paint pump, so request its next real frame explicitly.
                native.request_frame(RequestFrameOptions {
                    force_render: true,
                    require_presentation: true,
                    ..Default::default()
                });
            }
            pending.is_some()
        },
    );
    let pending = pending.expect("provider must retain the production Window's original token");
    assert_eq!(pending.window_id(), registration.window_id());
    assert!(pending.frame_generation() > 0);
    assert!(pending.attempt_id() > 0);
    assert!(pending.is_pending());
    let renderer_borrow = native.state.renderer.borrow_mut();
    let shutdown = native.presentation_shutdown_ticket();
    assert_eq!(pending.status(), PlatformFrameSubmissionStatus::Rejected);
    assert!(!pending.complete(PlatformWindowPresentOutcome::Submitted));
    assert_eq!(
        native.quiesce_presentation(&shutdown),
        PlatformPresentationShutdownOutcome::Rejected
    );
    assert!(observations.shutdowns.borrow().is_empty());
    assert!(!shutdown.snapshot().quiesced());
    assert!(!shutdown.snapshot().native_terminal());
    assert!(native.state.frame_signal.take().is_none());
    assert!(unsafe { IsWindow(Some(hwnd)).as_bool() });
    assert!(platform.raw_window_handles.read().contains(&registration));

    // Drop the real application with the renderer still borrowed. Its original retirement
    // owner must retain the HWND and retry this same shutdown authority after the borrow ends.
    drop(app);
    assert!(shutdown.same_authority(&native.presentation_shutdown_ticket()));
    assert!(unsafe { IsWindow(Some(hwnd)).as_bool() });
    assert!(observations.shutdowns.borrow().is_empty());
    drop(renderer_borrow);
    pump_messages_until_with_delay(
        &platform_lifetime,
        "original deferred HWND retirement after renderer borrow release",
        Duration::from_millis(2),
        || {
            !unsafe { IsWindow(Some(hwnd)).as_bool() }
                && platform.raw_window_handles.read().is_empty()
                && platform.inner.native_retirement_is_settled()
        },
    );
    assert!(shutdown.snapshot().quiesced());
    assert!(shutdown.snapshot().native_terminal());
    assert!(!shutdown.snapshot().protocol_violation());
    let shutdowns = observations.shutdowns.borrow();
    assert_eq!(shutdowns.len(), 1);
    assert!(shutdowns[0].same_authority(&shutdown));
    drop(shutdowns);
    assert_eq!(pending.status(), PlatformFrameSubmissionStatus::Rejected);
    drop(native);
    assert!(native_lifetime.upgrade().is_none());
    assert!(
        observations
            .atlas
            .borrow()
            .as_ref()
            .unwrap()
            .upgrade()
            .is_none()
    );
    drop(platform);
    pump_messages_until_with_delay(
        &platform_lifetime,
        "deferred test platform HWND and OLE cleanup",
        Duration::from_millis(2),
        || {
            !unsafe { IsWindow(Some(platform_hwnd)).as_bool() }
                && platform_lifetime.upgrade().is_none()
        },
    );
    assert_eq!(lifecycle.ole_uninitialize_count(), 1);
    Ok(())
}
