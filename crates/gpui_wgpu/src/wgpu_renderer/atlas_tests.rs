use super::*;
use open_gpui::{
    AppContext as _, AtlasKey, AtlasRemoveOutcome, AtlasTile, BoxShadow, Context, Corners, FontId,
    GlyphId, HeadlessAppContext, IntoElement, NoopTextSystem, Pixels, PlatformAtlas,
    PlatformHeadlessRenderer, PlatformTextSystem, Primitive, Render, RenderImage,
    RenderImageParams, Styled as _, SubtreeClip, SubtreeClipExt as _, SubtreeTransform,
    SubtreeTransformExt as _, UnderlineStyle, Window, canvas, fill, font, point, px, rgb, size,
};
use std::{borrow::Cow, cell::Cell};

struct WindowSceneRenderer {
    atlas: Arc<WgpuAtlas>,
    scale_factor: Rc<Cell<Option<f32>>>,
    captured: Rc<RefCell<Option<(Scene, f32)>>>,
}

impl PlatformHeadlessRenderer for WindowSceneRenderer {
    fn render_scene(&mut self, scene: &Scene, _: Size<DevicePixels>) -> anyhow::Result<()> {
        if self.captured.borrow().is_none() && scene.batches().next().is_some() {
            self.atlas
                .prepare_presentation(scene.batches().filter_map(|batch| match batch {
                    PrimitiveBatch::MonochromeSprites { texture, .. }
                    | PrimitiveBatch::SubpixelSprites { texture, .. }
                    | PrimitiveBatch::PolychromeSprites { texture, .. } => Some(texture),
                    _ => None,
                }))?;
            let scale_factor = self
                .scale_factor
                .get()
                .context("the public Window must supply the scene scale factor")?;
            *self.captured.borrow_mut() = Some((scene.clone(), scale_factor));
        }
        Ok(())
    }

    fn render_scene_to_image(
        &mut self,
        _: &Scene,
        _: Size<DevicePixels>,
    ) -> anyhow::Result<image::RgbaImage> {
        anyhow::bail!("the Window fixture must submit its complete scene directly")
    }

    fn sprite_atlas(&self) -> Arc<dyn PlatformAtlas> {
        self.atlas.clone()
    }
}

#[derive(Clone)]
enum FixturePaint {
    Images(Vec<Arc<RenderImage>>),
    Primitives,
    Mixed {
        image: Arc<RenderImage>,
        font_id: FontId,
        glyph_id: GlyphId,
    },
}

struct WindowFixture {
    paint: FixturePaint,
    text_system: Arc<dyn PlatformTextSystem>,
    content_size: Size<Pixels>,
}

impl WindowFixture {
    fn images(images: Vec<Arc<RenderImage>>) -> Self {
        Self {
            paint: FixturePaint::Images(images),
            text_system: Arc::new(NoopTextSystem),
            content_size: size(px(16.0), px(16.0)),
        }
    }

    fn primitives() -> Self {
        Self {
            paint: FixturePaint::Primitives,
            text_system: Arc::new(NoopTextSystem),
            content_size: size(px(32.0), px(32.0)),
        }
    }

    fn mixed() -> anyhow::Result<Self> {
        let text_system = Arc::new(crate::CosmicTextSystem::new_without_system_fonts(
            "IBM Plex Sans",
        ));
        text_system.add_fonts(vec![Cow::Borrowed(include_bytes!(
            "../../../../tests/fixtures/fonts/IBMPlexSans-Regular.ttf"
        ))])?;
        let font_id = text_system.font_id(&font("IBM Plex Sans"))?;
        let glyph_id = text_system
            .glyph_for_char(font_id, 'M')
            .context("the fixture font must contain M")?;
        Ok(Self {
            paint: FixturePaint::Mixed {
                image: color_image(16, 16, [0, 255, 255, 255]),
                font_id,
                glyph_id,
            },
            text_system,
            content_size: size(px(96.0), px(64.0)),
        })
    }
}

impl Render for WindowFixture {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let paint = self.paint.clone();
        let clip = SubtreeClip::try_rounded_rect(
            Bounds::new(point(px(0.0), px(0.0)), self.content_size),
            Corners {
                top_left: size(px(4.0), px(4.0)),
                top_right: size(px(4.0), px(4.0)),
                bottom_left: size(px(4.0), px(4.0)),
                bottom_right: size(px(4.0), px(4.0)),
            },
        )
        .expect("the fixture clip must be valid");
        let transform = SubtreeTransform::try_translation(point(px(4.0), px(4.0)))
            .expect("the fixture transform must be valid");
        canvas(
            |_, _, _| {},
            move |bounds, (), window, _| match paint {
                FixturePaint::Images(images) => {
                    for image in images {
                        window
                            .paint_image(bounds, Default::default(), image, 0, false)
                            .expect("the public image consumer must produce a sprite");
                    }
                }
                FixturePaint::Primitives => paint_primitives(window),
                FixturePaint::Mixed {
                    image,
                    font_id,
                    glyph_id,
                } => {
                    paint_primitives(window);
                    window.paint_drop_shadows(
                        Bounds::new(point(px(40.0), px(0.0)), size(px(16.0), px(16.0))),
                        Default::default(),
                        &[BoxShadow {
                            color: rgb(0xff00ff).into(),
                            offset: point(px(0.0), px(0.0)),
                            blur_radius: px(2.0),
                            spread_radius: px(0.0),
                            inset: false,
                        }],
                    );
                    window.paint_underline(
                        point(px(0.0), px(40.0)),
                        px(24.0),
                        &UnderlineStyle {
                            thickness: px(4.0),
                            color: Some(rgb(0x00ffff).into()),
                            wavy: false,
                        },
                    );
                    window
                        .paint_glyph(
                            point(px(40.0), px(56.0)),
                            font_id,
                            glyph_id,
                            px(24.0),
                            rgb(0xffffff).into(),
                        )
                        .expect("the public glyph consumer must produce a sprite");
                    window
                        .paint_image(
                            Bounds::new(point(px(64.0), px(0.0)), size(px(16.0), px(16.0))),
                            Default::default(),
                            image,
                            0,
                            false,
                        )
                        .expect("the mixed fixture must paint its image");
                }
            },
        )
        .w(self.content_size.width)
        .h(self.content_size.height)
        .with_subtree_clip(clip)
        .with_subtree_transform(transform)
    }
}

fn paint_primitives(window: &mut Window) {
    for (left, color) in [(0.0, 0xff0000), (16.0, 0x0000ff)] {
        window.paint_quad(fill(
            Bounds::new(point(px(left), px(0.0)), size(px(16.0), px(32.0))),
            rgb(color),
        ));
    }
    let mut path = Path::new(point(px(16.0), px(0.0)));
    path.line_to(point(px(32.0), px(0.0)));
    path.line_to(point(px(32.0), px(16.0)));
    path.line_to(point(px(16.0), px(0.0)));
    window.paint_path(path, rgb(0x00ff00));
}

fn with_unleased_window_scene(
    renderer: &mut WgpuHeadlessRenderer,
    fixture: WindowFixture,
    action: impl FnOnce(&Scene, f32, &mut WgpuHeadlessRenderer) -> anyhow::Result<()>,
) -> anyhow::Result<()> {
    let captured = Rc::new(RefCell::new(None));
    let output = captured.clone();
    let scale_factor = Rc::new(Cell::new(None));
    let observed_scale = scale_factor.clone();
    let atlas = renderer.core.atlas.clone();
    let mut app =
        HeadlessAppContext::with_platform(fixture.text_system.clone(), Arc::new(()), move || {
            Ok(Some(Box::new(WindowSceneRenderer {
                atlas: atlas.clone(),
                scale_factor: observed_scale.clone(),
                captured: output.clone(),
            })))
        });
    let handle = app.open_window(size(px(112.0), px(80.0)), move |window, app| {
        scale_factor.set(Some(window.scale_factor()));
        app.new(|_| fixture)
    })?;
    app.update_window(handle.into(), |_, window, _| {
        let _ = window.activate_window();
    })?;
    app.run_until_parked();
    app.simulate_window_scheduled_frame(handle.into())?;
    drop(app);
    let (scene, scale_factor) = captured
        .borrow_mut()
        .take()
        .context("the public Window did not submit its complete fixture scene")?;
    action(&scene, scale_factor, renderer)
}

fn color_image(width: u32, height: u32, bgra: [u8; 4]) -> Arc<RenderImage> {
    Arc::new(RenderImage::new(smallvec::smallvec![image::Frame::new(
        image::RgbaImage::from_pixel(width, height, image::Rgba(bgra)),
    )]))
}

fn image_key(image: &RenderImage) -> AtlasKey {
    AtlasKey::Image(RenderImageParams {
        image_id: image.id,
        frame_index: 0,
    })
}

fn insert_image(atlas: &WgpuAtlas, image: &RenderImage) -> anyhow::Result<AtlasTile> {
    atlas
        .get_or_insert_with(image_key(image), &mut || {
            Ok(Some((
                image.size(0),
                Cow::Borrowed(image.as_bytes(0).expect("the fixture image has one frame")),
            )))
        })?
        .context("the test image must allocate a tile")
}

fn render_size(scale_factor: f32, width: f32, height: f32) -> Size<DevicePixels> {
    size(px(width), px(height)).to_device_pixels(scale_factor)
}

fn pixel_at(pixels: &image::RgbaImage, scale_factor: f32, left: f32, top: f32) -> [u8; 4] {
    let position = point(px(left), px(top)).scale(scale_factor);
    pixels
        .get_pixel(position.x.0.round() as u32, position.y.0.round() as u32)
        .0
}

fn assert_window_clip(scene: &Scene, clip: ClipEnvelope, scale_factor: f32) {
    assert!(clip.clip_count >= 2);
    let first = clip.first_clip as usize;
    let end = first + clip.clip_count as usize;
    let shapes = &scene.clip_shapes()[first..end];
    assert!(shapes.iter().any(|shape| {
        shape.radii_x.top_left.0 == 4.0 * scale_factor
            && shape.radii_y.top_left.0 == 4.0 * scale_factor
    }));
}

fn assert_color(
    renderer: &mut WgpuHeadlessRenderer,
    scale_factor: f32,
    rgba: [u8; 4],
) -> anyhow::Result<()> {
    let pixels = renderer.read_image()?;
    assert_eq!(pixel_at(&pixels, scale_factor, 12.0, 12.0), rgba);
    assert_eq!(pixel_at(&pixels, scale_factor, 4.0, 4.0), [0, 0, 0, 255]);
    assert_eq!(pixel_at(&pixels, scale_factor, 2.0, 12.0), [0, 0, 0, 255]);
    Ok(())
}

fn shrink_instance_buffer(renderer: &mut WgpuHeadlessRenderer) {
    renderer.core.instance_buffer_capacity = 64;
    renderer.core.resources.instance_buffer =
        renderer
            .context
            .device
            .create_buffer(&wgpu::BufferDescriptor {
                label: Some("small_instance_buffer"),
                size: 64,
                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
}

#[test]
fn atlas_cache_creates_unique_groups_and_reuses_them_across_batches_and_resize()
-> anyhow::Result<()> {
    let mut renderer = WgpuHeadlessRenderer::new()?;
    let red = color_image(700, 700, [0, 0, 255, 255]);
    let blue = color_image(700, 700, [255, 0, 0, 255]);
    with_unleased_window_scene(
        &mut renderer,
        WindowFixture::images(vec![red.clone(), blue.clone(), red.clone(), blue, red]),
        |scene, scale_factor, renderer| {
            assert_eq!(scene.polychrome_sprites.len(), 5);
            let red = scene.polychrome_sprites[0];
            let blue = scene.polychrome_sprites[1];
            assert_ne!(red.tile.texture_instance(), blue.tile.texture_instance());
            assert_ne!(red.renderer_transform(), PrimitiveTransform::IDENTITY);
            assert_window_clip(scene, red.clip, scale_factor);
            assert_eq!(scene.batches().count(), 5);
            let target_size = render_size(scale_factor, 32.0, 32.0);

            renderer.render_scene(scene, target_size)?;
            assert_eq!(renderer.atlas_bind_group_cache_counts(), (2, 0));
            assert_eq!(renderer.core.resources.submitted_scene_count, 1);
            assert_color(renderer, scale_factor, [255, 0, 0, 255])?;
            let intermediate = renderer
                .core
                .resources
                .path_intermediate_texture
                .clone()
                .unwrap();
            for _ in 0..3 {
                renderer.render_scene(scene, target_size)?;
            }
            assert_eq!(renderer.atlas_bind_group_cache_counts(), (2, 6));
            assert_eq!(renderer.core.resources.submitted_scene_count, 4);

            renderer.render_scene(scene, render_size(scale_factor, 48.0, 32.0))?;
            assert_ne!(
                intermediate,
                renderer
                    .core
                    .resources
                    .path_intermediate_texture
                    .clone()
                    .unwrap(),
            );
            assert_eq!(renderer.atlas_bind_group_cache_counts(), (2, 8));
            assert_color(renderer, scale_factor, [255, 0, 0, 255])?;

            renderer.render_scene(&Scene::default(), target_size)?;
            assert!(renderer.core.resources.atlas_texture_bind_groups.is_empty());
            renderer.render_scene(scene, target_size)?;
            assert_eq!(renderer.atlas_bind_group_cache_counts(), (4, 8));
            assert_color(renderer, scale_factor, [255, 0, 0, 255])
        },
    )
}

#[test]
fn warm_atlas_cache_refuses_mixed_stale_generation_without_partial_submission() -> anyhow::Result<()>
{
    let mut renderer = WgpuHeadlessRenderer::new()?;
    let red = color_image(700, 700, [0, 0, 255, 255]);
    let blue = color_image(700, 700, [255, 0, 0, 255]);
    let red_key = image_key(&red);
    let recovery = blue.clone();
    let valid_texture = Rc::new(RefCell::new(None));
    let expected = valid_texture.clone();
    with_unleased_window_scene(
        &mut renderer,
        WindowFixture::images(vec![blue, red]),
        move |scene, scale_factor, renderer| {
            assert_eq!(scene.polychrome_sprites.len(), 2);
            let blue = scene.polychrome_sprites[0].tile;
            let red = scene.polychrome_sprites[1].tile;
            assert_ne!(blue.texture_instance(), red.texture_instance());
            *expected.borrow_mut() = Some(blue.texture_instance());
            let target_size = render_size(scale_factor, 32.0, 32.0);
            renderer.render_scene(scene, target_size)?;
            assert_color(renderer, scale_factor, [255, 0, 0, 255])?;
            assert_eq!(renderer.atlas_bind_group_cache_counts(), (2, 0));

            assert_eq!(
                renderer
                    .core
                    .atlas
                    .remove_with_diagnostics(&red_key)
                    .outcome,
                AtlasRemoveOutcome::TextureFreed,
            );
            let replacement = insert_image(
                &renderer.core.atlas,
                &color_image(700, 700, [0, 255, 0, 255]),
            )?;
            assert_eq!(red.texture_id, replacement.texture_id);
            assert_ne!(red.texture_instance(), replacement.texture_instance());
            let submissions = renderer.core.resources.submitted_scene_count;
            let error = renderer.render_scene(scene, target_size).unwrap_err();
            assert!(matches!(
                error.downcast_ref::<AtlasTextureLeaseError>(),
                Some(AtlasTextureLeaseError::TextureUnavailable { texture, .. })
                    if *texture == red.texture_instance()
            ));
            assert_eq!(renderer.core.resources.submitted_scene_count, submissions);
            assert_eq!(renderer.atlas_bind_group_cache_counts(), (2, 0));
            assert_color(renderer, scale_factor, [255, 0, 0, 255])
        },
    )?;
    with_unleased_window_scene(
        &mut renderer,
        WindowFixture::images(vec![recovery]),
        move |scene, scale_factor, renderer| {
            assert_eq!(
                Some(scene.polychrome_sprites[0].tile.texture_instance()),
                *valid_texture.borrow(),
            );
            renderer.render_scene(scene, render_size(scale_factor, 32.0, 32.0))?;
            assert_eq!(renderer.atlas_bind_group_cache_counts(), (2, 1));
            assert_eq!(renderer.core.resources.submitted_scene_count, 2);
            assert_color(renderer, scale_factor, [0, 0, 255, 255])
        },
    )
}

#[test]
fn prepared_receipt_preserves_removed_region_until_submission_and_release() -> anyhow::Result<()> {
    let mut renderer = WgpuHeadlessRenderer::new()?;
    let red = color_image(700, 700, [0, 0, 255, 255]);
    let red_key = image_key(&red);
    with_unleased_window_scene(
        &mut renderer,
        WindowFixture::images(vec![red]),
        move |scene, scale_factor, renderer| {
            let old = scene.polychrome_sprites[0].tile;
            let keeper = insert_image(&renderer.core.atlas, &color_image(1, 1, [0, 255, 0, 255]))?;
            assert_eq!(keeper.texture_instance(), old.texture_instance());
            let target_size = render_size(scale_factor, 32.0, 32.0);
            renderer.ensure_render_target(target_size)?;
            let (clip_count, receipt) = renderer.core.prepare_scene(scene)?;
            assert_eq!(
                renderer
                    .core
                    .atlas
                    .remove_with_diagnostics(&red_key)
                    .outcome,
                AtlasRemoveOutcome::TextureRetained,
            );
            let replacement = insert_image(
                &renderer.core.atlas,
                &color_image(700, 700, [255, 0, 0, 255]),
            )?;
            renderer.core.atlas.before_frame();
            let view = renderer.render_target.as_ref().unwrap().view.clone();
            renderer.last_submission = Some(renderer.core.render_prepared_frame(
                scene,
                &view,
                target_size,
                false,
                clip_count,
                &receipt,
                wgpu::Color::BLACK,
            )?);
            assert_eq!(renderer.core.resources.submitted_scene_count, 1);
            assert_ne!(replacement.texture_instance(), old.texture_instance());
            drop(receipt);
            let reused = insert_image(
                &renderer.core.atlas,
                &color_image(700, 700, [0, 255, 0, 255]),
            )?;
            assert_eq!(reused.texture_instance(), old.texture_instance());
            assert_eq!(reused.bounds, old.bounds);
            renderer.core.atlas.before_frame();
            assert_color(renderer, scale_factor, [255, 0, 0, 255])
        },
    )
}

#[test]
fn prepared_atlas_receipt_renders_original_pixels_after_reset() -> anyhow::Result<()> {
    let mut renderer = WgpuHeadlessRenderer::new()?;
    let red = color_image(16, 16, [0, 0, 255, 255]);
    let blue = color_image(16, 16, [255, 0, 0, 255]);
    let replacement = blue.clone();
    with_unleased_window_scene(
        &mut renderer,
        WindowFixture::images(vec![red]),
        move |scene, scale_factor, renderer| {
            let old = scene.polychrome_sprites[0].tile;
            let target_size = render_size(scale_factor, 32.0, 32.0);
            renderer.ensure_render_target(target_size)?;
            let (clip_count, receipt) = renderer.core.prepare_scene(scene)?;
            renderer.core.atlas.clear();
            let new = insert_image(&renderer.core.atlas, &replacement)?;
            assert_eq!(old.texture_id, new.texture_id);
            assert_ne!(old.texture_instance(), new.texture_instance());
            let view = renderer.render_target.as_ref().unwrap().view.clone();
            renderer.last_submission = Some(renderer.core.render_prepared_frame(
                scene,
                &view,
                target_size,
                false,
                clip_count,
                &receipt,
                wgpu::Color::BLACK,
            )?);
            assert_color(renderer, scale_factor, [255, 0, 0, 255])?;
            assert!(renderer.render_scene(scene, target_size).is_err());
            assert_eq!(renderer.core.resources.submitted_scene_count, 1);
            assert_color(renderer, scale_factor, [255, 0, 0, 255])
        },
    )?;
    with_unleased_window_scene(
        &mut renderer,
        WindowFixture::images(vec![blue]),
        |scene, scale_factor, renderer| {
            renderer.render_scene(scene, render_size(scale_factor, 32.0, 32.0))?;
            assert_color(renderer, scale_factor, [0, 0, 255, 255])?;
            assert_eq!(renderer.atlas_bind_group_cache_counts(), (2, 0));
            assert_eq!(renderer.core.resources.submitted_scene_count, 2);
            Ok(())
        },
    )
}

#[test]
fn atlas_lease_preserves_removed_tile_pixels_until_final_release() -> anyhow::Result<()> {
    let mut renderer = WgpuHeadlessRenderer::new()?;
    let red = color_image(700, 700, [0, 0, 255, 255]);
    let red_key = image_key(&red);
    with_unleased_window_scene(
        &mut renderer,
        WindowFixture::images(vec![red]),
        move |scene, scale_factor, renderer| {
            let red = scene.polychrome_sprites[0].tile;
            let keeper = insert_image(&renderer.core.atlas, &color_image(1, 1, [0, 255, 0, 255]))?;
            assert_eq!(keeper.texture_instance(), red.texture_instance());
            let atlas: Arc<dyn PlatformAtlas> = renderer.core.atlas.clone();
            let lease = atlas.retain_texture_instances(&[red.texture_instance()])?;
            assert_eq!(
                renderer
                    .core
                    .atlas
                    .remove_with_diagnostics(&red_key)
                    .outcome,
                AtlasRemoveOutcome::TextureRetained,
            );
            let blue = insert_image(
                &renderer.core.atlas,
                &color_image(700, 700, [255, 0, 0, 255]),
            )?;
            assert_ne!(red.texture_instance(), blue.texture_instance());
            renderer.render_scene(scene, render_size(scale_factor, 32.0, 32.0))?;
            assert_color(renderer, scale_factor, [255, 0, 0, 255])?;
            lease
                .validate()
                .expect("the removed region must remain leased");
            drop(lease);
            let reused = insert_image(
                &renderer.core.atlas,
                &color_image(700, 700, [0, 255, 0, 255]),
            )?;
            assert_eq!(reused.texture_instance(), red.texture_instance());
            assert_eq!(reused.bounds, red.bounds);
            Ok(())
        },
    )
}

#[test]
fn atlas_same_device_reset_reconstructs_core_with_an_empty_cache() -> anyhow::Result<()> {
    let mut renderer = WgpuHeadlessRenderer::new()?;
    let red = color_image(16, 16, [0, 0, 255, 255]);
    let blue = color_image(16, 16, [255, 0, 0, 255]);
    let replacement = blue.clone();
    with_unleased_window_scene(
        &mut renderer,
        WindowFixture::images(vec![red]),
        move |scene, scale_factor, renderer| {
            let old = scene.polychrome_sprites[0].tile;
            let target_size = render_size(scale_factor, 32.0, 32.0);
            renderer.render_scene(scene, target_size)?;
            assert_color(renderer, scale_factor, [255, 0, 0, 255])?;
            let device = renderer.context.device.clone();
            let atlas = renderer.core.atlas.clone();
            let format = renderer.core.target_format;
            atlas.handle_device_lost(&renderer.context);
            renderer.core = WgpuRendererCore::new(
                &renderer.context,
                atlas,
                format,
                wgpu::CompositeAlphaMode::Opaque,
            )?;
            assert!(Arc::ptr_eq(&device, &renderer.core.resources.device));
            assert_eq!(renderer.atlas_bind_group_cache_counts(), (0, 0));
            assert!(renderer.render_scene(scene, target_size).is_err());
            assert_eq!(renderer.core.resources.submitted_scene_count, 0);
            let new = insert_image(&renderer.core.atlas, &replacement)?;
            assert_eq!(old.texture_id, new.texture_id);
            assert_ne!(old.texture_instance(), new.texture_instance());
            Ok(())
        },
    )?;
    with_unleased_window_scene(
        &mut renderer,
        WindowFixture::images(vec![blue]),
        |scene, scale_factor, renderer| {
            renderer.render_scene(scene, render_size(scale_factor, 32.0, 32.0))?;
            assert_eq!(renderer.atlas_bind_group_cache_counts(), (1, 0));
            assert_eq!(renderer.core.resources.submitted_scene_count, 1);
            assert_color(renderer, scale_factor, [0, 0, 255, 255])
        },
    )
}

#[test]
fn preuploaded_primitives_and_paths_survive_instance_buffer_growth() -> anyhow::Result<()> {
    let mut renderer = WgpuHeadlessRenderer::new()?;
    shrink_instance_buffer(&mut renderer);
    with_unleased_window_scene(
        &mut renderer,
        WindowFixture::primitives(),
        |scene, scale_factor, renderer| {
            assert_eq!(scene.quads.len(), 2);
            assert_eq!(scene.paths.len(), 1);
            assert_window_clip(scene, scene.quads[0].clip, scale_factor);
            assert_window_clip(scene, scene.paths[0].clip, scale_factor);
            assert_ne!(
                Primitive::from(scene.quads[0]).transform(),
                PrimitiveTransform::IDENTITY
            );
            let pixels =
                renderer.render_scene_to_image(scene, render_size(scale_factor, 48.0, 48.0))?;
            assert_eq!(
                pixel_at(&pixels, scale_factor, 12.0, 12.0),
                [255, 0, 0, 255]
            );
            assert_eq!(
                pixel_at(&pixels, scale_factor, 28.0, 28.0),
                [0, 0, 255, 255]
            );
            assert_eq!(pixel_at(&pixels, scale_factor, 32.0, 8.0), [0, 255, 0, 255]);
            assert_eq!(pixel_at(&pixels, scale_factor, 4.0, 4.0), [0, 0, 0, 255]);
            assert_eq!(pixel_at(&pixels, scale_factor, 2.0, 12.0), [0, 0, 0, 255]);
            assert!(renderer.core.instance_buffer_capacity > 64);
            assert_eq!(renderer.core.resources.submitted_scene_count, 1);
            assert_eq!(renderer.atlas_bind_group_cache_counts(), (0, 0));
            Ok(())
        },
    )
}

#[test]
fn preuploaded_mixed_primitives_use_real_clips_transforms_and_glyph_pixels() -> anyhow::Result<()> {
    let mut renderer = WgpuHeadlessRenderer::new()?;
    shrink_instance_buffer(&mut renderer);
    with_unleased_window_scene(
        &mut renderer,
        WindowFixture::mixed()?,
        |scene, scale_factor, renderer| {
            assert_eq!(scene.shadows.len(), 1);
            assert_eq!(scene.quads.len(), 2);
            assert_eq!(scene.paths.len(), 1);
            assert_eq!(scene.underlines.len(), 1);
            assert_eq!(scene.monochrome_sprites.len(), 1);
            assert_eq!(scene.polychrome_sprites.len(), 1);
            for clip in [
                scene.shadows[0].clip,
                scene.quads[0].clip,
                scene.paths[0].clip,
                scene.underlines[0].clip,
                scene.monochrome_sprites[0].clip,
                scene.polychrome_sprites[0].clip,
            ] {
                assert_window_clip(scene, clip, scale_factor);
            }
            assert_ne!(
                Primitive::from(scene.monochrome_sprites[0]).transform(),
                PrimitiveTransform::IDENTITY,
            );
            let target_size = render_size(scale_factor, 112.0, 80.0);
            let pixels = renderer.render_scene_to_image(scene, target_size)?;
            assert_eq!(
                pixel_at(&pixels, scale_factor, 12.0, 12.0),
                [255, 0, 0, 255]
            );
            assert_eq!(
                pixel_at(&pixels, scale_factor, 28.0, 28.0),
                [0, 0, 255, 255]
            );
            assert_eq!(pixel_at(&pixels, scale_factor, 32.0, 8.0), [0, 255, 0, 255]);
            assert_eq!(
                pixel_at(&pixels, scale_factor, 12.0, 46.0),
                [0, 255, 255, 255]
            );
            assert_eq!(
                pixel_at(&pixels, scale_factor, 76.0, 12.0),
                [255, 255, 0, 255]
            );
            let shadow = pixel_at(&pixels, scale_factor, 52.0, 12.0);
            assert!(shadow[0] > 200 && shadow[1] < 10 && shadow[2] > 200);
            let glyph_pixels = (36..64)
                .flat_map(|top| (44..68).map(move |left| (left, top)))
                .filter(|&(left, top)| {
                    let pixel = pixel_at(&pixels, scale_factor, left as f32, top as f32);
                    pixel[0] > 220 && pixel[1] > 220 && pixel[2] > 220
                })
                .count();
            assert!(
                glyph_pixels >= 10,
                "the real font glyph must paint opaque white pixels"
            );
            assert_eq!(pixel_at(&pixels, scale_factor, 4.0, 4.0), [0, 0, 0, 255]);
            assert_eq!(pixel_at(&pixels, scale_factor, 2.0, 12.0), [0, 0, 0, 255]);
            assert!(renderer.core.instance_buffer_capacity > 64);
            assert_eq!(renderer.atlas_bind_group_cache_counts(), (2, 0));
            renderer.render_scene(scene, target_size)?;
            assert_eq!(renderer.atlas_bind_group_cache_counts(), (2, 2));
            assert_eq!(renderer.core.resources.submitted_scene_count, 2);
            Ok(())
        },
    )
}
