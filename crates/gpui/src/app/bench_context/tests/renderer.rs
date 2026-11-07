use super::*;
use crate::{
    AtlasKey, AtlasTextureInstanceId, AtlasTextureLeaseEpoch, AtlasTextureLeaseError, AtlasTile,
    Corners, DevicePixels, IntoElement, PlatformAtlas, RenderImage, Scene, Styled as _, TileId,
    canvas, px, size,
};
use std::{borrow::Cow, cell::Cell, sync::atomic::AtomicUsize};

struct RejectingAtlas {
    attempts: AtomicUsize,
}

impl PlatformAtlas for RejectingAtlas {
    fn get_or_insert_with<'a>(
        &self,
        key: &AtlasKey,
        _: &mut dyn FnMut() -> Result<Option<(Size<DevicePixels>, Cow<'a, [u8]>)>>,
    ) -> Result<Option<AtlasTile>> {
        Ok(Some(AtlasTile {
            texture_id: crate::AtlasTextureId {
                index: 1,
                kind: key.texture_kind(),
            },
            tile_id: TileId(1),
            padding: 0,
            bounds: Bounds::new(Default::default(), size(DevicePixels(1), DevicePixels(1))),
            texture_generation: 1,
            texture_generation_padding: 0,
        }))
    }

    fn remove(&self, _: &AtlasKey) {}

    fn atlas_texture_lease_epoch(&self) -> AtlasTextureLeaseEpoch {
        AtlasTextureLeaseEpoch::INITIAL
    }

    unsafe fn acquire_atlas_texture_leases(
        &self,
        textures: &[AtlasTextureInstanceId],
    ) -> std::result::Result<AtlasTextureLeaseEpoch, AtlasTextureLeaseError> {
        self.attempts.fetch_add(1, Ordering::SeqCst);
        Err(AtlasTextureLeaseError::TextureUnavailable {
            texture: *textures
                .first()
                .expect("the image must name an atlas texture"),
            epoch: AtlasTextureLeaseEpoch::INITIAL,
        })
    }

    unsafe fn release_atlas_texture_leases(
        &self,
        _: AtlasTextureLeaseEpoch,
        _: &[AtlasTextureInstanceId],
    ) {
        panic!("a refused texture acquisition must not transfer a release obligation");
    }
}

struct Renderer {
    atlas: Arc<dyn PlatformAtlas>,
    submissions: Rc<Cell<usize>>,
}

impl PlatformHeadlessRenderer for Renderer {
    fn render_scene(&mut self, _: &Scene, _: Size<DevicePixels>) -> Result<()> {
        self.submissions.set(self.submissions.get() + 1);
        Ok(())
    }

    fn render_scene_to_image(
        &mut self,
        _: &Scene,
        _: Size<DevicePixels>,
    ) -> Result<image::RgbaImage> {
        panic!("benchmark presentation must not perform a CPU readback");
    }

    fn sprite_atlas(&self) -> Arc<dyn PlatformAtlas> {
        self.atlas.clone()
    }
}

struct ImageProbe {
    enabled: Rc<Cell<bool>>,
    image: Arc<RenderImage>,
}

impl Render for ImageProbe {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let enabled = self.enabled.get();
        let image = self.image.clone();
        canvas(
            |_, _, _| {},
            move |bounds, _, window, _| {
                if enabled {
                    assert!(
                        !bounds.is_empty(),
                        "the image probe must have paintable bounds"
                    );
                    window
                        .paint_image(bounds, Corners::default(), image, 0, false)
                        .expect("the image probe must reach candidate-frame lease validation");
                }
            },
        )
        .size(px(32.0))
    }
}

#[test]
fn renderer_factory_errors_propagate_without_becoming_a_missing_renderer() {
    criterion().bench_function("fallible-benchmark-renderer-factory", |bencher| {
        let platform = bench_platform(
            Some(Box::new(|| {
                anyhow::bail!("injected renderer creation failure")
            })),
            Arc::new(crate::NoopTextSystem::new()),
        );
        let mut cx = BenchAppContext::new(platform, None, bencher);
        let result =
            cx.update(|app| app.open_window(WindowOptions::default(), |_, cx| cx.new(|_| Empty)));
        let error = result.expect_err("renderer creation must fail the window transaction");
        assert!(format!("{error:#}").contains("injected renderer creation failure"));
        assert!(cx.read(|app| app.windows.is_empty()));
        cx.bench_iter(|_| ());
        cx.teardown();
    });
}

#[test]
fn rejected_candidate_generations_are_not_reported_as_draws_or_submissions() {
    let report = BenchReport::default();
    criterion().bench_function("benchmark-candidate-lease-refusal", |bencher| {
        let atlas = Arc::new(RejectingAtlas {
            attempts: AtomicUsize::new(0),
        });
        let submissions = Rc::new(Cell::new(0));
        let platform = bench_platform(
            Some(Box::new({
                let atlas = atlas.clone();
                let submissions = submissions.clone();
                move || -> Result<Option<Box<dyn PlatformHeadlessRenderer>>> {
                    Ok(Some(Box::new(Renderer {
                        atlas: atlas.clone(),
                        submissions: submissions.clone(),
                    })))
                }
            })),
            Arc::new(crate::NoopTextSystem::new()),
        );
        let mut cx =
            BenchAppContext::new_with_platform_and_report(platform, None, bencher, report.clone());
        let mut window = cx.add_empty_window();
        let enabled = Rc::new(Cell::new(false));
        let image = Arc::new(RenderImage::new(smallvec::smallvec![image::Frame::new(
            image::RgbaImage::from_pixel(1, 1, image::Rgba([0, 0, 0, 255])),
        )]));
        let view = window
            .replace_root_view(|_, _| ImageProbe {
                enabled: enabled.clone(),
                image,
            })
            .expect("the image probe must replace the root");
        cx.settle();
        let initial = window.update(|window, _| window.presentation_facts());
        assert!(initial.frame_accepted_generation.is_some());
        assert_eq!(
            initial.frame_accepted_generation,
            initial.present_submitted_generation
        );
        assert_eq!(atlas.attempts.load(Ordering::SeqCst), 0);
        let initial_submissions = submissions.get();
        let view_id = view.entity_id();
        cx.bench_renderer(view, |_, window, cx| {
            enabled.set(true);
            cx.notify();
            window.refresh();
        });
        assert!(atlas.attempts.load(Ordering::SeqCst) > 0);
        let rejected = window.update(|window, _| window.presentation_facts());
        assert_eq!(
            rejected.frame_accepted_generation,
            initial.frame_accepted_generation
        );
        assert_eq!(
            rejected.present_submitted_generation,
            initial.present_submitted_generation
        );
        assert_eq!(submissions.get(), initial_submissions);
        enabled.set(false);
        window.update(|window, cx| {
            cx.notify(view_id);
            window.refresh();
        });
        cx.run_until_idle();
        let recovered = window.update(|window, _| window.presentation_facts());
        assert!(recovered.frame_accepted_generation > initial.frame_accepted_generation);
        assert_eq!(
            recovered.frame_accepted_generation,
            recovered.present_submitted_generation
        );
        assert!(submissions.get() > initial_submissions);
        drop(window);
        cx.teardown();
    });
    let snapshot = report.frame_snapshot.borrow();
    assert!(snapshot.draw.is_empty());
    assert!(snapshot.present_interval.is_empty());
}
