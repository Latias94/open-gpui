use super::*;
use open_gpui::{
    AppContext as _, AtlasKey, AtlasRemoveOutcome, AtlasTile, BorderStyle, BoxShadow, Context,
    Corners, Edges, FontId, GlyphId, HeadlessAppContext, ImageTexture, ImageTextureAlpha,
    ImageTextureSampling, ImageTextureTransfer, IntoElement, NoopTextSystem, Pixels, PlatformAtlas,
    PlatformHeadlessRenderer, PlatformTextSystem, Primitive, Render, RenderImage,
    RenderImageParams, Styled as _, SubtreeClip, SubtreeClipExt as _, SubtreeTransform,
    SubtreeTransformExt as _, SubtreeTransformOrigin, UnderlineStyle, Window, canvas, fill, font,
    hsla, point, px, quad, rgb, size, transparent_black,
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
    ImportedImage(ImageTexture),
    CroppedImage(Arc<RenderImage>),
    Primitives,
    HollowBorder {
        square: bool,
    },
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

    fn imported_image(image: ImageTexture) -> Self {
        Self {
            paint: FixturePaint::ImportedImage(image),
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

    fn cropped_image(image: Arc<RenderImage>) -> Self {
        Self {
            paint: FixturePaint::CroppedImage(image),
            text_system: Arc::new(NoopTextSystem),
            content_size: size(px(16.0), px(16.0)),
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
        let transform = match &paint {
            FixturePaint::CroppedImage(_) | FixturePaint::HollowBorder { square: false } => {
                SubtreeTransform::try_new(
                    size(1.5, 0.75),
                    point(px(4.25), px(4.5)),
                    SubtreeTransformOrigin::TOP_LEFT,
                )
            }
            FixturePaint::HollowBorder { square: true } => SubtreeTransform::try_new(
                size(2.0, 2.0),
                point(px(4.81), px(4.81)),
                SubtreeTransformOrigin::TOP_LEFT,
            ),
            _ => SubtreeTransform::try_translation(point(px(4.0), px(4.0))),
        }
        .expect("the fixture transform must be valid");
        canvas(
            |_, _, _| {},
            move |bounds, (), window, _| match paint {
                FixturePaint::Images(images) => {
                    for image in images {
                        window
                            .paint_image(bounds, bounds, Default::default(), image, 0, false)
                            .expect("the public image consumer must produce a sprite");
                    }
                }
                FixturePaint::ImportedImage(image) => {
                    window
                        .paint_image_texture(bounds, Default::default(), &image)
                        .expect("the imported image consumer must produce a sprite");
                }
                FixturePaint::CroppedImage(image) => {
                    let image_bounds = Bounds::new(
                        point(bounds.left() - bounds.size.width / 2.0, bounds.top()),
                        size(bounds.size.width * 2.0, bounds.size.height),
                    );
                    window
                        .paint_image(
                            bounds,
                            image_bounds,
                            Corners {
                                top_left: px(4.0),
                                top_right: px(4.0),
                                bottom_left: px(4.0),
                                bottom_right: px(4.0),
                            },
                            image,
                            0,
                            false,
                        )
                        .expect("the cropped image must produce its visible sprite");
                }
                FixturePaint::Primitives => paint_primitives(window),
                FixturePaint::HollowBorder { square } => {
                    let (bounds, corners, edges) = if square {
                        (
                            Bounds::new(point(px(-0.405), px(-0.405)), size(px(8.0), px(8.0))),
                            Corners::default(),
                            Edges {
                                top: px(0.25),
                                right: px(1.0),
                                bottom: px(0.25),
                                left: px(1.0),
                            },
                        )
                    } else {
                        (
                            Bounds::new(point(px(-1.5), px(1.0)), size(px(45.0), px(38.0))),
                            Corners {
                                top_left: px(6.0),
                                top_right: px(8.0),
                                bottom_right: px(4.0),
                                bottom_left: px(3.0),
                            },
                            Edges {
                                top: px(2.0),
                                right: px(3.5),
                                bottom: px(2.5),
                                left: px(1.5),
                            },
                        )
                    };
                    let border = quad(
                        bounds,
                        corners,
                        transparent_black(),
                        edges,
                        hsla(0.09, 0.8, 0.6, 0.6),
                        BorderStyle::Dashed,
                    );
                    window.paint_quad(border.clone().background(hsla(0.0, 0.0, 0.0, f32::EPSILON)));
                    window.paint_quad(border);
                }
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
    renderer.core.resources.instance_buffer = GpuData::Storage(
        renderer
            .context
            .device
            .create_buffer(&wgpu::BufferDescriptor {
                label: Some("small_instance_buffer"),
                size: 64,
                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }),
    );
}

fn narrow_packed_texture(
    device: &wgpu::Device,
    label: &str,
    width: u32,
    height: u32,
) -> (GpuData, u64) {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some(label),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba32Uint,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    let capacity = u64::from(width) * u64::from(height) * INSTANCE_TEXTURE_TEXEL_SIZE;
    (
        GpuData::Texture {
            view: texture.create_view(&wgpu::TextureViewDescriptor::default()),
            texture,
            width,
            height,
        },
        capacity,
    )
}

#[test]
fn cropped_image_preserves_uv_corners_clip_transform_and_removed_tile_lease() -> anyhow::Result<()>
{
    let mut renderer = WgpuHeadlessRenderer::new()?;
    let pixels = image::RgbaImage::from_fn(64, 16, |x, _| {
        image::Rgba(match x {
            0..16 => [0, 0, 255, 255],
            16..48 => [0, 255, 0, 255],
            _ => [255, 0, 0, 255],
        })
    });
    let image = Arc::new(RenderImage::new(smallvec::smallvec![image::Frame::new(
        pixels
    )]));
    let key = image_key(&image);
    let full_tile = insert_image(&renderer.core.atlas, &image)?;
    with_unleased_window_scene(
        &mut renderer,
        WindowFixture::cropped_image(image),
        move |scene, scale_factor, renderer| {
            let sprite = &scene.polychrome_sprites[0];
            assert_eq!(sprite.tile.texture_instance(), full_tile.texture_instance());
            assert_eq!(sprite.tile.tile_id, full_tile.tile_id);
            assert_eq!(sprite.tile.padding, full_tile.padding);
            assert_eq!(
                sprite.tile.bounds,
                Bounds::new(
                    point(
                        full_tile.bounds.origin.x + DevicePixels(16),
                        full_tile.bounds.origin.y
                    ),
                    size(DevicePixels(32), DevicePixels(16)),
                )
            );
            assert_eq!(sprite.corner_radii.top_left, px(4.0).scale(scale_factor));
            assert!(sprite.clip.clip_count >= 2);
            let first = sprite.clip.first_clip as usize;
            let end = first + sprite.clip.clip_count as usize;
            assert!(scene.clip_shapes()[first..end].iter().any(|shape| {
                shape.radii_x.top_left == px(6.0).scale(scale_factor)
                    && shape.radii_y.top_left == px(3.0).scale(scale_factor)
            }));
            let atlas: Arc<dyn PlatformAtlas> = renderer.core.atlas.clone();
            let lease = atlas.retain_texture_instances(&[sprite.tile.texture_instance()])?;
            assert_eq!(
                renderer.core.atlas.remove_with_diagnostics(&key).outcome,
                AtlasRemoveOutcome::TextureRetained,
            );
            insert_image(&renderer.core.atlas, &color_image(64, 16, [255, 0, 0, 255]))?;
            let pixels =
                renderer.render_scene_to_image(scene, render_size(scale_factor, 32.0, 32.0))?;
            assert_eq!(
                pixel_at(&pixels, scale_factor, 16.0, 10.5),
                [0, 255, 0, 255]
            );
            assert_eq!(pixel_at(&pixels, scale_factor, 4.5, 4.5), [0, 0, 0, 255]);
            assert_eq!(pixel_at(&pixels, scale_factor, 2.0, 10.5), [0, 0, 0, 255]);
            lease
                .validate()
                .expect("the removed cropped region must remain leased");
            Ok(())
        },
    )
}

#[test]
fn packed_shader_reads_odd_quads_and_exact_clips_across_texture_rows() -> anyhow::Result<()> {
    let mut renderer = WgpuHeadlessRenderer::new()?;
    with_unleased_window_scene(
        &mut renderer,
        WindowFixture::mixed()?,
        |scene, scale_factor, renderer| {
            assert_eq!(scene.quads.len(), 2);
            assert_window_clip(scene, scene.quads[1].clip, scale_factor);
            let target_size = render_size(scale_factor, 112.0, 80.0);
            let expected = renderer.render_scene_to_image(scene, target_size)?;
            assert_eq!(
                pixel_at(&expected, scale_factor, 12.0, 12.0),
                [255, 0, 0, 255]
            );
            assert_eq!(
                pixel_at(&expected, scale_factor, 28.0, 28.0),
                [0, 0, 255, 255]
            );
            assert_eq!(pixel_at(&expected, scale_factor, 4.0, 4.0), [0, 0, 0, 255]);
            renderer.core = WgpuRendererCore::new_with_transport(
                &renderer.context,
                renderer.core.atlas.clone(),
                renderer.core.target_format,
                wgpu::CompositeAlphaMode::Opaque,
                true,
            )?;

            let instance_width = 5;
            let quad_stride = std::mem::size_of::<open_gpui::Quad>() as u64;
            assert_eq!(quad_stride, 184);
            assert_eq!(quad_stride % INSTANCE_TEXTURE_TEXEL_SIZE, 8);
            let odd_first_texel = quad_stride / INSTANCE_TEXTURE_TEXEL_SIZE;
            let odd_last_texel = (2 * quad_stride - 1) / INSTANCE_TEXTURE_TEXEL_SIZE;
            assert!(
                odd_first_texel / u64::from(instance_width)
                    < odd_last_texel / u64::from(instance_width)
            );
            let (instances, instance_capacity) = narrow_packed_texture(
                &renderer.context.device,
                "cross_row_packed_instances",
                instance_width,
                256,
            );

            let clip_width = 2;
            let clip_stride = std::mem::size_of::<GpuClipShape>() as u64;
            let clip_texels = clip_stride / INSTANCE_TEXTURE_TEXEL_SIZE;
            assert_eq!(clip_texels, 3);
            let clip = scene.quads[1].clip;
            for clip_index in clip.first_clip..clip.first_clip + clip.clip_count {
                let first_texel = u64::from(clip_index) * clip_texels;
                assert!(
                    first_texel / u64::from(clip_width)
                        < (first_texel + clip_texels - 1) / u64::from(clip_width)
                );
            }
            let clip_height = u32::try_from(
                (scene.clip_shapes().len() as u64 * clip_texels).div_ceil(u64::from(clip_width)),
            )?;
            let (clips, clip_capacity) = narrow_packed_texture(
                &renderer.context.device,
                "cross_row_packed_clips",
                clip_width,
                clip_height,
            );
            let core = &mut renderer.core;
            core.resources.instance_buffer = instances;
            core.instance_buffer_capacity = instance_capacity;
            core.resources.clip_buffer = clips;
            core.clip_buffer_capacity = clip_capacity;
            core.resources.globals_bind_group = WgpuRendererCore::create_globals_bind_group(
                &core.resources.device,
                &core.resources.bind_group_layouts.globals,
                &core.resources.globals_buffer,
                &core.resources.clip_buffer,
                0,
                core.gamma_offset,
                clip_capacity,
                "cross_row_globals",
            );
            core.resources.path_globals_bind_group = WgpuRendererCore::create_globals_bind_group(
                &core.resources.device,
                &core.resources.bind_group_layouts.globals,
                &core.resources.globals_buffer,
                &core.resources.clip_buffer,
                core.path_globals_offset,
                core.gamma_offset,
                clip_capacity,
                "cross_row_path_globals",
            );

            let actual = renderer.render_scene_to_image(scene, target_size)?;
            assert_eq!(actual, expected);
            assert_eq!(renderer.core.instance_buffer_capacity, instance_capacity);
            assert_eq!(renderer.core.clip_buffer_capacity, clip_capacity);
            assert_eq!(renderer.core.resources.submitted_scene_count, 1);
            renderer.check_gpu_errors()
        },
    )
}

#[test]
fn hollow_border_matches_full_quad_with_both_wgpu_instance_transports() -> anyhow::Result<()> {
    for square in [true, false] {
        let mut renderer = WgpuHeadlessRenderer::new()?;
        with_unleased_window_scene(
            &mut renderer,
            WindowFixture {
                paint: FixturePaint::HollowBorder { square },
                text_system: Arc::new(NoopTextSystem),
                content_size: size(px(42.0), px(40.0)),
            },
            |scene, scale_factor, renderer| {
                let mut full = scene.clone();
                full.quads.truncate(1);
                full.quads[0].background = transparent_black().into();
                let mut strips = scene.clone();
                strips.quads.remove(0);
                assert!(strips.quads.len() >= 2);
                let full_clip = full.quads[0].clip;
                assert!(strips.quads.iter().all(|strip| {
                    strip.clip.first_clip == full_clip.first_clip
                        && strip.clip.clip_count == full_clip.clip_count
                        && strip.clip.conservative_bounds != full_clip.conservative_bounds
                }));

                let target_size = if square {
                    render_size(scale_factor, 64.0, 64.0)
                } else {
                    render_size(scale_factor, 84.0, 56.0)
                };
                let expected = renderer.render_scene_to_image(&full, target_size)?;
                assert!(
                    expected
                        .pixels()
                        .filter(|pixel| pixel.0 != [0, 0, 0, 255])
                        .count()
                        > if square { 30 } else { 100 }
                );
                for packed in [false, true] {
                    if packed {
                        renderer.core = WgpuRendererCore::new_with_transport(
                            &renderer.context,
                            renderer.core.atlas.clone(),
                            renderer.core.target_format,
                            wgpu::CompositeAlphaMode::Opaque,
                            true,
                        )?;
                    }
                    let actual = renderer.render_scene_to_image(&strips, target_size)?;
                    for ((x, y, expected), actual) in
                        expected.enumerate_pixels().zip(actual.pixels())
                    {
                        assert!(
                            expected
                                .0
                                .iter()
                                .zip(actual.0)
                                .all(|(expected, actual)| expected.abs_diff(actual) <= 2),
                            "packed={packed} square={square}, border pixel ({x}, {y}): {actual:?} != {expected:?}"
                        );
                    }
                    renderer.check_gpu_errors()?;
                }
                Ok(())
            },
        )?;
    }
    Ok(())
}

#[test]
fn empty_path_batch_does_not_copy_stale_intermediate_pixels() -> anyhow::Result<()> {
    let mut renderer = WgpuHeadlessRenderer::new()?;
    with_unleased_window_scene(
        &mut renderer,
        WindowFixture::primitives(),
        |scene, scale_factor, renderer| {
            assert_eq!(scene.paths.len(), 1);
            let mut empty_path_scene = scene.clone();
            empty_path_scene.paths[0].vertices.clear();
            assert!(
                empty_path_scene
                    .batches()
                    .any(|batch| matches!(batch, PrimitiveBatch::Paths(range) if range == (0..1)))
            );
            let target_size = render_size(scale_factor, 48.0, 48.0);

            let rasterized = renderer.render_scene_to_image(scene, target_size)?;
            assert_eq!(
                pixel_at(&rasterized, scale_factor, 32.0, 8.0),
                [0, 255, 0, 255]
            );

            let without_vertices =
                renderer.render_scene_to_image(&empty_path_scene, target_size)?;
            assert_eq!(
                pixel_at(&without_vertices, scale_factor, 32.0, 8.0),
                [0, 0, 255, 255]
            );
            assert_eq!(renderer.core.resources.submitted_scene_count, 2);
            Ok(())
        },
    )
}

#[test]
fn late_path_allocation_refusal_preserves_pixels_and_draw_receipt_until_retry() -> anyhow::Result<()>
{
    let mut renderer = WgpuHeadlessRenderer::new()?;
    with_unleased_window_scene(
        &mut renderer,
        WindowFixture::primitives(),
        |scene, scale_factor, renderer| {
            assert_eq!(scene.quads.len(), 2);
            assert_eq!(scene.paths.len(), 1);
            assert!(projected_path_visible_bounds(&scene.paths[0]).is_some());
            assert!(!scene.paths[0].vertices.is_empty());
            let mut batches = scene.batches();
            assert!(
                matches!(batches.next(), Some(PrimitiveBatch::Quads(range)) if range == (0..2))
            );
            assert!(
                matches!(batches.next(), Some(PrimitiveBatch::Paths(range)) if range == (0..1))
            );
            assert!(batches.next().is_none());
            let mut previous_scene = scene.clone();
            let first_background = previous_scene.quads[0].background;
            previous_scene.quads[0].background = previous_scene.quads[1].background;
            previous_scene.quads[1].background = first_background;
            let target_size = render_size(scale_factor, 48.0, 48.0);

            for texture_transport in [false, true] {
                renderer.core = WgpuRendererCore::new_with_transport(
                    &renderer.context,
                    renderer.core.atlas.clone(),
                    renderer.core.target_format,
                    wgpu::CompositeAlphaMode::Opaque,
                    texture_transport,
                )?;
                let expected = renderer.render_scene_to_image(scene, target_size)?;
                let previous = renderer.render_scene_to_image(&previous_scene, target_size)?;
                assert_ne!(previous, expected);
                assert_eq!(
                    pixel_at(&expected, scale_factor, 12.0, 12.0),
                    [255, 0, 0, 255]
                );
                assert_eq!(
                    pixel_at(&previous, scale_factor, 12.0, 12.0),
                    [0, 0, 255, 255]
                );

                let mut prelude_offset = 0;
                drop(renderer.core.write_instances(scene, &mut prelude_offset)?);
                let path_size = std::mem::size_of::<PathRasterizationVertex>() as u64
                    * scene.paths[0].vertices.len() as u64;
                let (instances, capacity) = GpuData::create(
                    &renderer.context.device,
                    "late_path_refusal_instances",
                    prelude_offset.max(path_size),
                    texture_transport,
                )?;
                let (path_offset, allocation, _) = instance_allocation(
                    prelude_offset,
                    path_size,
                    std::mem::size_of::<PathRasterizationVertex>() as u64,
                    renderer.core.storage_buffer_alignment,
                    texture_transport,
                )?;
                assert!(path_offset + allocation > capacity);
                assert!(allocation <= capacity);
                let maximum = renderer.core.max_buffer_size;
                renderer.core.resources.instance_buffer = instances;
                renderer.core.instance_buffer_capacity = capacity;
                renderer.core.max_buffer_size = capacity;
                let accepted_draws = renderer.core.resources.submitted_scene_count;
                assert_eq!(accepted_draws, 2);
                assert!(renderer.last_submission.take().is_some());

                let error = renderer.render_scene(scene, target_size).unwrap_err();
                assert_eq!(
                    error.to_string(),
                    format!("instance buffer cannot grow beyond {capacity} bytes"),
                );
                assert_eq!(
                    renderer.core.resources.submitted_scene_count,
                    accepted_draws
                );
                assert!(renderer.last_submission.is_none());
                assert_eq!(renderer.read_image()?, previous);
                assert_eq!(
                    renderer.core.resources.submitted_scene_count,
                    accepted_draws
                );
                renderer.check_gpu_errors()?;

                renderer.core.max_buffer_size = maximum;
                assert!(renderer.last_submission.take().is_some());
                renderer.render_scene(scene, target_size)?;
                assert!(renderer.last_submission.is_some());
                assert_eq!(
                    renderer.core.resources.submitted_scene_count,
                    accepted_draws + 1
                );
                assert!(renderer.core.instance_buffer_capacity > capacity);
                assert_eq!(renderer.read_image()?, expected);
                renderer.check_gpu_errors()?;
            }
            Ok(())
        },
    )
}

#[test]
fn packed_texture_transport_matches_native_storage_for_complete_mixed_scene() -> anyhow::Result<()>
{
    let mut renderer = WgpuHeadlessRenderer::new()?;
    with_unleased_window_scene(
        &mut renderer,
        WindowFixture::mixed()?,
        |scene, scale_factor, renderer| {
            let target_size = render_size(scale_factor, 112.0, 80.0);
            let expected = renderer.render_scene_to_image(scene, target_size)?;
            renderer.core = WgpuRendererCore::new_with_transport(
                &renderer.context,
                renderer.core.atlas.clone(),
                renderer.core.target_format,
                wgpu::CompositeAlphaMode::Opaque,
                true,
            )?;
            let (data, capacity) = GpuData::create(
                &renderer.context.device,
                "small_packed_instance_texture",
                64,
                true,
            )?;
            renderer.core.resources.instance_buffer = data;
            renderer.core.instance_buffer_capacity = capacity;
            renderer.core.clip_buffer_capacity = 0;
            let actual = renderer.render_scene_to_image(scene, target_size)?;
            assert_eq!(actual, expected);
            assert!(renderer.core.instance_buffer_capacity > 64);
            assert!(renderer.core.clip_buffer_capacity > 0);
            assert_eq!(renderer.core.resources.submitted_scene_count, 1);
            assert_eq!(renderer.atlas_bind_group_cache_counts(), (2, 0));
            assert!(renderer.core.resources.pipelines.subpixel_sprites.is_none());
            let submissions = renderer.core.resources.submitted_scene_count;
            let maximum = renderer.core.max_clip_buffer_size;
            renderer.core.max_clip_buffer_size = 0;
            assert!(renderer.render_scene(scene, target_size).is_err());
            assert_eq!(renderer.core.resources.submitted_scene_count, submissions);
            assert_eq!(renderer.read_image()?, expected);
            renderer.core.max_clip_buffer_size = maximum;
            assert_eq!(
                renderer.render_scene_to_image(scene, target_size)?,
                expected
            );
            Ok(())
        },
    )
}

#[test]
fn packed_gpu_data_writes_cross_rows_and_rejects_overflow_before_mutation() -> anyhow::Result<()> {
    let mut renderer = WgpuHeadlessRenderer::new()?;
    let texture = renderer
        .context
        .device
        .create_texture(&wgpu::TextureDescriptor {
            label: Some("narrow_packed_data"),
            size: wgpu::Extent3d {
                width: 2,
                height: 3,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba32Uint,
            usage: wgpu::TextureUsages::COPY_SRC
                | wgpu::TextureUsages::COPY_DST
                | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
    let data = GpuData::Texture {
        view: texture.create_view(&wgpu::TextureViewDescriptor::default()),
        texture,
        width: 2,
        height: 3,
    };
    let bytes = (1..=37).collect::<Vec<u8>>();
    data.write(&renderer.context.queue, 16, &bytes)?;
    assert!(data.write(&renderer.context.queue, 1, &bytes).is_err());
    assert!(data.write(&renderer.context.queue, 80, &bytes).is_err());
    let readback = renderer
        .context
        .device
        .create_buffer(&wgpu::BufferDescriptor {
            label: Some("packed_data_readback"),
            size: 3 * 256,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
    let GpuData::Texture { texture, .. } = &data else {
        unreachable!()
    };
    let mut encoder = renderer
        .context
        .device
        .create_command_encoder(&Default::default());
    encoder.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo {
            texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::TexelCopyBufferInfo {
            buffer: &readback,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(256),
                rows_per_image: Some(3),
            },
        },
        wgpu::Extent3d {
            width: 2,
            height: 3,
            depth_or_array_layers: 1,
        },
    );
    let submission = renderer.context.queue.submit([encoder.finish()]);
    let (sender, receiver) = std::sync::mpsc::channel();
    readback
        .slice(..)
        .map_async(wgpu::MapMode::Read, move |result| {
            let _ = sender.send(result);
        });
    renderer.context.device.poll(wgpu::PollType::Wait {
        submission_index: Some(submission),
        timeout: Some(Duration::from_secs(30)),
    })?;
    receiver.recv_timeout(Duration::from_secs(30))??;
    let mapped = readback.slice(..).get_mapped_range()?;
    let actual = mapped
        .chunks_exact(256)
        .flat_map(|row| row[..32].iter().copied())
        .collect::<Vec<_>>();
    assert_eq!(&actual[16..53], bytes.as_slice());
    assert_eq!(&actual[53..64], &[0; 11]);
    drop(mapped);
    readback.unmap();
    renderer.check_gpu_errors()?;
    Ok(())
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
fn builtin_submission_retains_imported_owner_until_completion_callback() -> anyhow::Result<()> {
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct HostOwner(Arc<AtomicUsize>);

    impl Drop for HostOwner {
        fn drop(&mut self) {
            self.0.fetch_add(1, Ordering::SeqCst);
        }
    }

    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
        backends: wgpu::Backends::PRIMARY,
        ..wgpu::InstanceDescriptor::new_without_display_handle()
    });
    let adapter = open_gpui::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        force_fallback_adapter: false,
        apply_limit_buckets: false,
        ..Default::default()
    }))?;
    let (device, queue) = open_gpui::block_on(
        adapter.request_device(&wgpu::DeviceDescriptor {
            required_limits: wgpu::Limits::downlevel_defaults()
                .using_resolution(adapter.limits())
                .using_alignment(adapter.limits()),
            ..Default::default()
        }),
    )?;
    let device = Arc::new(device);
    let queue = Arc::new(queue);
    let format = wgpu::TextureFormat::Rgba8Unorm;
    let atlas = Arc::new(WgpuAtlas::new(device.clone(), queue.clone(), format));
    let mut core = WgpuRendererCore::from_device(
        &adapter,
        device.clone(),
        queue.clone(),
        atlas.clone(),
        format,
        wgpu::CompositeAlphaMode::Opaque,
        false,
    )?;
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("builtin submission imported image"),
        size: wgpu::Extent3d {
            width: 2,
            height: 2,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    queue.write_texture(
        texture.as_image_copy(),
        &[255_u8, 0, 0, 255].repeat(4),
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(8),
            rows_per_image: Some(2),
        },
        texture.size(),
    );
    let drops = Arc::new(AtomicUsize::new(0));
    let owner = Arc::new(HostOwner(drops.clone()));
    let owner_lifetime = Arc::downgrade(&owner);
    let image = atlas.import_image(crate::WgpuImportedImageDescriptor {
        texture: texture.clone(),
        mip_level: 0,
        array_layer: 0,
        region: Bounds::new(
            point(DevicePixels(0), DevicePixels(0)),
            size(DevicePixels(2), DevicePixels(2)),
        ),
        transfer: ImageTextureTransfer::Srgb,
        alpha: ImageTextureAlpha::Straight,
        sampling: ImageTextureSampling::Linear,
        owner: Some(owner),
    })?;
    let fixture = WindowFixture::imported_image(image.clone());
    let captured = Rc::new(RefCell::new(None));
    let output = captured.clone();
    let scale_factor = Rc::new(Cell::new(None));
    let observed_scale = scale_factor.clone();
    let mut app = HeadlessAppContext::with_platform(fixture.text_system.clone(), Arc::new(()), {
        let atlas = atlas.clone();
        move || {
            Ok(Some(Box::new(WindowSceneRenderer {
                atlas: atlas.clone(),
                scale_factor: observed_scale.clone(),
                captured: output.clone(),
            })))
        }
    });
    let handle = app.open_window(size(px(32.0), px(32.0)), move |window, app| {
        scale_factor.set(Some(window.scale_factor()));
        app.new(|_| fixture)
    })?;
    app.update_window(handle.into(), |_, window, _| {
        let _ = window.activate_window();
    })?;
    app.run_until_parked();
    app.simulate_window_scheduled_frame(handle.into())?;
    let (scene, scale_factor) = captured
        .borrow_mut()
        .take()
        .context("the Window must produce an imported-image scene")?;
    assert_eq!(scene.polychrome_sprites.len(), 1);
    assert_eq!(scene.batches().count(), 1);
    let target_size = render_size(scale_factor, 32.0, 32.0);
    let target = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("builtin imported-image render target"),
        size: wgpu::Extent3d {
            width: target_size.width.0 as u32,
            height: target_size.height.0 as u32,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    });
    let view = target.create_view(&wgpu::TextureViewDescriptor::default());
    let submission = core.render_frame(&scene, &view, target_size, false, wgpu::Color::BLACK)?;
    assert_eq!(core.resources.submitted_scene_count, 1);
    assert_eq!(core.resources.atlas_texture_bind_groups.len(), 1);
    drop(image);
    drop(scene);
    drop(app);
    atlas.clear();
    assert!(
        owner_lifetime.upgrade().is_some(),
        "the submission callback must retain the host owner before polling"
    );
    assert_eq!(drops.load(Ordering::SeqCst), 0);
    device.poll(wgpu::PollType::Wait {
        submission_index: Some(submission),
        timeout: Some(Duration::from_secs(30)),
    })?;
    assert!(
        owner_lifetime.upgrade().is_none(),
        "the completion callback must release the original host owner"
    );
    assert_eq!(drops.load(Ordering::SeqCst), 1);
    assert_eq!(core.resources.atlas_texture_bind_groups.len(), 1);
    assert_eq!(texture.width(), 2);
    Ok(())
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
