#![cfg(all(not(target_family = "wasm"), feature = "test-support"))]

use anyhow::Result;
use open_gpui::{
    AppContext as _, Context, DevicePixels, HeadlessAppContext, IntoElement, ParentElement as _,
    PlatformAtlas, PlatformHeadlessRenderer, PlatformWindowPresentOutcome, Render, RenderImage,
    Scene, Size, Styled as _, Window, WindowHandle, WindowPresentationFacts, blue, canvas, div, px,
    size,
};
use open_gpui_wgpu::{CosmicTextSystem, WgpuHeadlessRenderer};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    sync::{Arc, Weak},
};

#[derive(Default)]
struct Observed {
    factories: Cell<usize>,
    draws: Cell<usize>,
    submitted: Cell<usize>,
    readbacks: Cell<usize>,
    live_renderers: Cell<usize>,
    renders: Cell<usize>,
    reject_factory: Cell<bool>,
    reject_draw: Cell<bool>,
    atlases: RefCell<Vec<Weak<dyn PlatformAtlas>>>,
}

struct ObservedRenderer {
    renderer: WgpuHeadlessRenderer,
    observed: Rc<Observed>,
}

impl PlatformHeadlessRenderer for ObservedRenderer {
    fn render_scene(&mut self, scene: &Scene, size: Size<DevicePixels>) -> Result<()> {
        self.observed.draws.set(self.observed.draws.get() + 1);
        anyhow::ensure!(
            !self.observed.reject_draw.get(),
            "injected headless draw rejection"
        );
        self.renderer.render_scene(scene, size)?;
        self.observed
            .submitted
            .set(self.observed.submitted.get() + 1);
        Ok(())
    }

    fn render_scene_to_image(
        &mut self,
        scene: &Scene,
        size: Size<DevicePixels>,
    ) -> Result<image::RgbaImage> {
        self.observed
            .readbacks
            .set(self.observed.readbacks.get() + 1);
        anyhow::ensure!(
            !self.observed.reject_draw.get(),
            "injected headless readback rejection"
        );
        self.renderer.render_scene_to_image(scene, size)
    }

    fn sprite_atlas(&self) -> Arc<dyn PlatformAtlas> {
        self.renderer.sprite_atlas()
    }
}

impl Drop for ObservedRenderer {
    fn drop(&mut self) {
        self.observed
            .live_renderers
            .set(self.observed.live_renderers.get() - 1);
    }
}

fn context(observed: Rc<Observed>) -> HeadlessAppContext {
    HeadlessAppContext::with_platform(
        Arc::new(CosmicTextSystem::new_without_system_fonts("sans-serif")),
        Arc::new(()),
        move || {
            observed.factories.set(observed.factories.get() + 1);
            anyhow::ensure!(
                !observed.reject_factory.get(),
                "injected headless factory failure"
            );
            let renderer = WgpuHeadlessRenderer::new()?;
            observed
                .atlases
                .borrow_mut()
                .push(Arc::downgrade(&renderer.sprite_atlas()));
            observed
                .live_renderers
                .set(observed.live_renderers.get() + 1);
            Ok(Some(Box::new(ObservedRenderer {
                renderer,
                observed: observed.clone(),
            })))
        },
    )
}

struct PaintedRoot {
    image: Option<Arc<RenderImage>>,
    observed: Rc<Observed>,
}

impl Render for PaintedRoot {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        self.observed.renders.set(self.observed.renders.get() + 1);
        let image = self.image.clone();
        div().relative().size_full().bg(blue()).child(
            canvas(
                |_, _, _| {},
                move |bounds, (), window, _| {
                    if let Some(image) = &image {
                        assert!(
                            !bounds.is_empty(),
                            "the decoded image must have paintable bounds"
                        );
                        window
                            .paint_image(bounds, Default::default(), image.clone(), 0, false)
                            .expect("headless image paint must succeed");
                    }
                },
            )
            .absolute()
            .size_full(),
        )
    }
}

fn open(cx: &mut HeadlessAppContext, observed: Rc<Observed>) -> Result<WindowHandle<PaintedRoot>> {
    let red_bgra = image::Rgba([0, 0, 255, 255]);
    let image = Arc::new(RenderImage::new(smallvec::smallvec![image::Frame::new(
        image::RgbaImage::from_pixel(3, 2, red_bgra),
    )]));
    let handle = cx.open_window(size(px(37.0), px(19.0)), move |_, app| {
        app.new(|_| PaintedRoot {
            image: Some(image),
            observed,
        })
    })?;
    cx.update_window(handle.into(), |_, window, _| {
        let _ = window.activate_window();
    })?;
    cx.run_until_parked();
    assert!(cx.update_window(handle.into(), |_, window, _| window.is_window_active())?);
    cx.simulate_window_scheduled_frame(handle.into())?;
    Ok(handle)
}

fn draw(
    cx: &mut HeadlessAppContext,
    handle: WindowHandle<PaintedRoot>,
) -> Result<WindowPresentationFacts> {
    cx.simulate_window_scheduled_frame(handle.into())?;
    cx.update_window(handle.into(), |_, window, _| window.presentation_facts())
}

fn assert_pixel(image: &image::RgbaImage, expected: [u8; 4]) {
    assert_eq!(image.dimensions(), (74, 38));
    assert_eq!(image.get_pixel(37, 19).0, expected);
}

#[test]
fn injected_gpu_renders_without_readback_and_survives_notification_and_reconstruction() -> Result<()>
{
    let observed = Rc::new(Observed::default());
    for _ in 0..2 {
        let mut cx = context(observed.clone());
        let previous_readbacks = observed.readbacks.get();
        let previous_submissions = observed.submitted.get();
        let handle = open(&mut cx, observed.clone())?;
        assert!(observed.submitted.get() > previous_submissions);
        assert_eq!(observed.readbacks.get(), previous_readbacks);
        let first = cx.update_window(handle.into(), |_, window, _| window.presentation_facts())?;
        assert_eq!(
            first.present_submitted_generation,
            first.frame_accepted_generation
        );
        assert_eq!(
            first.non_empty_presented_generation,
            first.frame_accepted_generation
        );
        assert_eq!(
            first.latest_present_attempt.unwrap().outcome,
            PlatformWindowPresentOutcome::Submitted
        );
        assert_pixel(&cx.capture_screenshot(handle.into())?, [255, 0, 0, 255]);

        let renders = observed.renders.get();
        handle.update(&mut cx, |root, _, app| {
            root.image = None;
            app.notify();
        })?;
        cx.run_until_parked();
        let notified = draw(&mut cx, handle)?;
        assert!(observed.renders.get() > renders);
        assert!(notified.frame_accepted_generation > first.frame_accepted_generation);
        assert_eq!(
            notified.present_submitted_generation,
            notified.frame_accepted_generation
        );
        assert_pixel(&cx.capture_screenshot(handle.into())?, [0, 0, 255, 255]);

        cx.update_window(handle.into(), |_, window, app| window.remove_window(app))?;
        cx.run_until_parked();
        assert!(cx.update(|app| app.windows().is_empty()));
        drop(cx);
        assert_eq!(observed.live_renderers.get(), 0);
        assert!(
            observed
                .atlases
                .borrow()
                .iter()
                .all(|atlas| atlas.upgrade().is_none())
        );
    }
    assert_eq!(observed.factories.get(), 2);
    Ok(())
}

#[test]
fn injected_factory_and_draw_failures_preserve_reservations_and_submission_authority() -> Result<()>
{
    let observed = Rc::new(Observed::default());
    observed.reject_factory.set(true);
    let mut cx = context(observed.clone());
    let error = open(&mut cx, observed.clone()).unwrap_err();
    assert!(format!("{error:#}").contains("injected headless factory failure"));
    assert_eq!(observed.live_renderers.get(), 0);
    assert_eq!(observed.renders.get(), 0);
    assert!(cx.update(|app| app.windows().is_empty()));

    observed.reject_factory.set(false);
    let handle = open(&mut cx, observed.clone())?;
    let baseline = draw(&mut cx, handle)?;
    let submissions = observed.submitted.get();
    let readbacks = observed.readbacks.get();
    observed.reject_draw.set(true);
    handle.update(&mut cx, |root, _, app| {
        root.image = None;
        app.notify();
    })?;
    let rejected = draw(&mut cx, handle)?;
    assert!(rejected.frame_accepted_generation > baseline.frame_accepted_generation);
    assert_eq!(
        rejected.present_submitted_generation,
        baseline.present_submitted_generation
    );
    assert_eq!(
        rejected.latest_present_attempt.unwrap().outcome,
        PlatformWindowPresentOutcome::Rejected
    );
    assert_eq!(observed.submitted.get(), submissions);
    assert_eq!(observed.readbacks.get(), readbacks);

    observed.reject_draw.set(false);
    let recovered = draw(&mut cx, handle)?;
    assert_eq!(
        recovered.present_submitted_generation,
        recovered.frame_accepted_generation
    );
    assert!(observed.submitted.get() > submissions);
    assert_pixel(&cx.capture_screenshot(handle.into())?, [0, 0, 255, 255]);
    drop(cx);
    assert_eq!(observed.live_renderers.get(), 0);
    assert!(
        observed
            .atlases
            .borrow()
            .iter()
            .all(|atlas| atlas.upgrade().is_none())
    );
    Ok(())
}
