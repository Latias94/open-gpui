use super::*;
use crate::native_test_window::NativeTestOpaqueWindow;
use std::{
    borrow::Cow,
    cell::{Cell, RefCell},
    rc::Rc,
};
use windows::Win32::{
    Foundation::HMODULE,
    UI::WindowsAndMessaging::{IsWindow, IsWindowVisible},
};

struct WarpRenderer {
    renderer: DirectXRenderer,
    window: NativeTestOpaqueWindow,
    devices: DirectXDevices,
}

impl WarpRenderer {
    fn new(viewport: Size<DevicePixels>) -> Result<Self> {
        let devices = warp_devices()?;
        let window = NativeTestOpaqueWindow::create_hidden(Bounds::new(
            point(DevicePixels(0), DevicePixels(0)),
            viewport,
        ))?;
        let mut renderer = DirectXRenderer::new(window.hwnd(), &devices, true)?;
        renderer.resize(viewport)?;
        Ok(Self {
            renderer,
            window,
            devices,
        })
    }

    fn finish(self) -> Result<()> {
        wait_for_gpu_idle(
            self.renderer
                .devices
                .as_ref()
                .context("live WARP devices")?,
        )?;
        drop(self.renderer);
        drop(self.window);
        Ok(())
    }
}

struct WarpSceneSource {
    fixture: WarpRenderer,
    snapshot: Rc<RefCell<Option<Scene>>>,
}

impl PlatformHeadlessRenderer for WarpSceneSource {
    fn render_scene(&mut self, scene: &Scene, viewport: Size<DevicePixels>) -> Result<()> {
        self.fixture.renderer.resize(viewport)?;
        let outcome = self
            .fixture
            .renderer
            .draw(scene, WindowBackgroundAppearance::Opaque)?;
        anyhow::ensure!(
            outcome == PlatformWindowPresentOutcome::Submitted,
            "the WARP Scene source must submit its actual frame: {outcome:?}"
        );
        self.snapshot.replace(Some(scene.clone()));
        Ok(())
    }

    fn render_scene_to_image(
        &mut self,
        scene: &Scene,
        viewport: Size<DevicePixels>,
    ) -> Result<RgbaImage> {
        self.fixture.renderer.resize(viewport)?;
        let image = self
            .fixture
            .renderer
            .render_to_image(scene, WindowBackgroundAppearance::Opaque)?;
        self.snapshot.replace(Some(scene.clone()));
        Ok(image)
    }

    fn sprite_atlas(&self) -> Arc<dyn PlatformAtlas> {
        self.fixture.renderer.sprite_atlas()
    }
}

struct WarpSceneRoot(Arc<RenderImage>);

impl Render for WarpSceneRoot {
    fn render(
        &mut self,
        window: &mut Window,
        _: &mut open_gpui::Context<Self>,
    ) -> impl IntoElement {
        let scale_factor = window.scale_factor();
        div().relative().size_full().children([2., 18.].map(|left| {
            img(self.0.clone())
                .absolute()
                .left(px(left / scale_factor))
                .top(px(2. / scale_factor))
                .size(px(12. / scale_factor))
        }))
    }
}

pub(crate) fn with_warp_scene_template(
    test: impl FnOnce(&Scene, Size<DevicePixels>) -> Result<()>,
) -> Result<()> {
    let viewport = size(DevicePixels(32), DevicePixels(16));
    let root = WarpSceneRoot(render_image([0, 255, 0, 255]));
    with_warp_window(viewport, root, |context, handle, snapshot| {
        let (template, captured) = capture_scene(context, handle.into(), snapshot)?;
        assert_eq!(captured.dimensions(), (32, 16));
        assert_eq!(template.polychrome_sprites.len(), 2);
        assert!(!template.clip_shapes().is_empty());
        for (sprite, left) in template.polychrome_sprites.iter().zip([2., 18.]) {
            assert_eq!(
                sprite.bounds,
                Bounds::new(
                    point(ScaledPixels(left), ScaledPixels(2.)),
                    size(ScaledPixels(12.), ScaledPixels(12.)),
                )
            );
            assert_sprite_pixel(&captured, sprite, [0, 255, 0, 255]);
        }
        test(&template, viewport)
    })
}

fn render_image(bgra: [u8; 4]) -> Arc<RenderImage> {
    Arc::new(RenderImage::new(vec![image::Frame::new(
        RgbaImage::from_pixel(12, 12, image::Rgba(bgra)),
    )]))
}

fn with_warp_window<V: Render + 'static>(
    viewport: Size<DevicePixels>,
    root: V,
    test: impl FnOnce(&mut HeadlessAppContext, WindowHandle<V>, &RefCell<Option<Scene>>) -> Result<()>,
) -> Result<()> {
    let snapshot = Rc::new(RefCell::new(None));
    let native_handle = Rc::new(Cell::new(None));
    let mut context =
        HeadlessAppContext::with_platform(Arc::new(NoopTextSystem::new()), Arc::new(()), {
            let snapshot = snapshot.clone();
            let native_handle = native_handle.clone();
            move || {
                let fixture = WarpRenderer::new(viewport)?;
                native_handle.set(Some(fixture.window.hwnd()));
                Ok(Some(Box::new(WarpSceneSource {
                    fixture,
                    snapshot: snapshot.clone(),
                })
                    as Box<dyn PlatformHeadlessRenderer>))
            }
        });
    let logical_size = viewport.map(|value| px(value.0 as f32 / 2.0));
    let handle = context.open_window(logical_size, |_, cx| cx.new(|_| root))?;
    let native_handle = native_handle
        .get()
        .context("the WARP Scene source must own its hidden native target")?;
    assert!(!unsafe { IsWindowVisible(native_handle).as_bool() });
    let result = test(&mut context, handle, &snapshot);
    let closed = context.update_window(handle.into(), |_, window, cx| {
        window.remove_window(cx);
    });
    context.run_until_parked();
    drop(context);
    assert!(!unsafe { IsWindow(Some(native_handle)).as_bool() });
    closed?;
    result
}

fn capture_scene(
    context: &mut HeadlessAppContext,
    handle: AnyWindowHandle,
    snapshot: &RefCell<Option<Scene>>,
) -> Result<(Scene, RgbaImage)> {
    context.update_window(handle, |_, window, cx| window.draw(cx).clear(cx))?;
    let captured = context.capture_screenshot(handle)?;
    let scene = snapshot
        .borrow_mut()
        .take()
        .context("the real Window must provide a complete WARP Scene snapshot")?;
    Ok((scene, captured))
}

struct WarpBatchRoot {
    expanded: bool,
    green: Arc<RenderImage>,
    red: Arc<RenderImage>,
}

impl Render for WarpBatchRoot {
    fn render(&mut self, _: &mut Window, _: &mut open_gpui::Context<Self>) -> impl IntoElement {
        let (expanded, green, red) = (self.expanded, self.green.clone(), self.red.clone());
        canvas(
            |_, _, _| (),
            move |_, (), window, _| {
                let scale = window.scale_factor();
                let bounds = |x, y, width, height| {
                    Bounds::new(
                        point(px(x / scale), px(y / scale)),
                        size(px(width / scale), px(height / scale)),
                    )
                };
                let left = bounds(2., 2., 12., 12.);
                if !expanded {
                    window.paint_quad(fill(left, rgb(0x0000ff)));
                    return;
                }
                for _ in 0..65 {
                    window.paint_quad(fill(left, rgb(0x0000ff)));
                }
                let green_bounds = bounds(2., 2., 28., 12.);
                window
                    .paint_image(
                        green_bounds,
                        green_bounds,
                        Corners::default(),
                        green,
                        0,
                        false,
                    )
                    .unwrap();
                window.paint_quad(fill(bounds(18., 2., 12., 12.), rgb(0x0000ff)));
                let red_bounds = bounds(21., 5., 6., 6.);
                window
                    .paint_image(red_bounds, red_bounds, Corners::default(), red, 0, false)
                    .unwrap();
            },
        )
        .size_full()
    }
}

pub(crate) fn warp_nonzero_instance_batches_survive_growth_and_smaller_frames() -> Result<()> {
    let root = WarpBatchRoot {
        expanded: false,
        green: render_image([0, 255, 0, 255]),
        red: render_image([0, 0, 255, 255]),
    };
    let viewport = size(DevicePixels(32), DevicePixels(16));
    with_warp_window(viewport, root, |context, handle, snapshot| {
        let (_, small) = capture_scene(context, handle.into(), snapshot)?;
        assert_pixel(&small, 5, 8, [0, 0, 255, 255]);
        assert_pixel(&small, 24, 8, [255, 255, 255, 255]);

        handle.update(context, |root, _, cx| {
            root.expanded = true;
            cx.notify();
        })?;
        let (scene, image) = capture_scene(context, handle.into(), snapshot)?;
        let batches = scene.batches().collect::<Vec<_>>();
        assert!(matches!(
            batches.as_slice(),
            [PrimitiveBatch::Quads(first), PrimitiveBatch::PolychromeSprites { range: first_image, texture: first_texture }, PrimitiveBatch::Quads(second), PrimitiveBatch::PolychromeSprites { range: second_image, texture: second_texture }]
                if *first == (0..65) && *first_image == (0..1) && *second == (65..66)
                    && *second_image == (1..2) && first_texture == second_texture
        ));
        assert_pixel(&image, 5, 8, [0, 255, 0, 255]);
        assert_pixel(&image, 19, 8, [0, 0, 255, 255]);
        assert_pixel(&image, 24, 8, [255, 0, 0, 255]);
        assert_pixel(&image, 31, 8, [255, 255, 255, 255]);

        handle.update(context, |root, _, cx| {
            root.expanded = false;
            cx.notify();
        })?;
        let (_, smaller) = capture_scene(context, handle.into(), snapshot)?;
        assert_eq!(smaller, small);
        Ok(())
    })
}

struct WarpBorderRoot {
    style: BorderStyle,
    split: bool,
}

impl Render for WarpBorderRoot {
    fn render(&mut self, _: &mut Window, _: &mut open_gpui::Context<Self>) -> impl IntoElement {
        let (style, split) = (self.style, self.split);
        let transform = SubtreeTransform::try_new(
            size(1.05, 0.9),
            point(px(1.5), px(3.5)),
            SubtreeTransformOrigin::TOP_LEFT,
        )
        .unwrap();
        let clip = SubtreeClip::try_rounded_rect(
            Bounds::new(point(px(12.), px(6.)), size(px(70.), px(49.))),
            Corners::all(px(7.)).map(|radius| size(*radius, *radius)),
        )
        .unwrap();
        canvas(
            |_, _, _| (),
            move |_, (), window, _| {
                let border = quad(
                    Bounds::new(point(px(8.), px(7.)), size(px(75.), px(48.))),
                    Corners {
                        top_left: px(8.),
                        top_right: px(6.),
                        bottom_right: px(3.),
                        bottom_left: px(5.),
                    },
                    hsla(0., 0., 0., if split { 0. } else { f32::EPSILON }),
                    Edges {
                        top: px(2.25),
                        right: px(3.),
                        bottom: px(2.),
                        left: px(1.5),
                    },
                    hsla(0.59, 0.8, 0.45, 0.62),
                    style,
                );
                window.paint_quad(border);
            },
        )
        .size_full()
        .with_subtree_clip(clip)
        .with_subtree_transform(transform)
    }
}

pub(crate) fn warp_hollow_border_strips_match_full_quad_under_projected_rounded_clip() -> Result<()>
{
    let viewport = size(DevicePixels(192), DevicePixels(128));
    let root = WarpBorderRoot {
        style: BorderStyle::Solid,
        split: false,
    };
    with_warp_window(viewport, root, |context, handle, snapshot| {
        let mut reference = WarpRenderer::new(viewport)?;
        for style in [BorderStyle::Solid, BorderStyle::Dashed] {
            handle.update(context, |root, _, cx| {
                root.style = style;
                root.split = false;
                cx.notify();
            })?;
            let (mut full, _) = capture_scene(context, handle.into(), snapshot)?;
            let mut baseline = full
                .quads
                .iter()
                .find(|quad| quad.border_widths != Edges::default())
                .copied()
                .context("the fixture must preserve its original border geometry")?;
            assert!(baseline.clip.clip_count > 1);
            baseline.background = transparent_black().into();
            let first = baseline.clip.first_clip as usize;
            let end = first + baseline.clip.clip_count as usize;
            let shapes = &full.clip_shapes()[first..end];
            baseline.clip.conservative_bounds = shapes
                .iter()
                .skip(1)
                .fold(shapes[0].bounds, |bounds, shape| {
                    bounds.intersect(&shape.bounds)
                });
            full.quads = vec![baseline];
            let expected = reference
                .renderer
                .render_to_image(&full, WindowBackgroundAppearance::Opaque)?;
            assert!(
                expected
                    .pixels()
                    .filter(|pixel| pixel.0 != [255; 4])
                    .count()
                    > 100
            );

            handle.update(context, |root, _, cx| {
                root.split = true;
                cx.notify();
            })?;
            let (split, actual) = capture_scene(context, handle.into(), snapshot)?;
            assert!(split.quads.len() >= 2);
            assert_eq!(split.clip_shapes(), full.clip_shapes());
            assert!(split.quads.iter().all(|strip| {
                strip.bounds == baseline.bounds
                    && strip.border_widths == baseline.border_widths
                    && strip.clip.first_clip == baseline.clip.first_clip
                    && strip.clip.clip_count == baseline.clip.clip_count
                    && strip.clip.conservative_bounds != baseline.clip.conservative_bounds
            }));
            for (x, y, expected) in expected.enumerate_pixels() {
                assert_pixel(&actual, x, y, expected.0);
            }
        }
        reference.finish()
    })
}

struct WarpFractionalClipRoot {
    edge: f32,
    primitive: &'static str,
    image: Arc<RenderImage>,
}

impl Render for WarpFractionalClipRoot {
    fn render(
        &mut self,
        window: &mut Window,
        _: &mut open_gpui::Context<Self>,
    ) -> impl IntoElement {
        let primitive = self.primitive;
        let image = self.image.clone();
        let clip = SubtreeClip::try_rect(Bounds::new(
            point(px(0.), px(0.)),
            size(
                px(self.edge / window.scale_factor()),
                px(self.edge / window.scale_factor()),
            ),
        ))
        .expect("the fractional clip must be valid");
        canvas(
            |_, _, _| (),
            move |bounds, (), window, _| match primitive {
                "opaque" => window.paint_quad(fill(bounds, rgb(0x00ff00))),
                "path" => {
                    let mut path = PathBuilder::fill();
                    path.add_polygon(
                        &[
                            bounds.origin,
                            point(bounds.right(), bounds.top()),
                            point(bounds.right(), bounds.bottom()),
                            point(bounds.left(), bounds.bottom()),
                        ],
                        true,
                    );
                    window.paint_path(
                        path.build().expect("the fractional path must build"),
                        rgb(0x00ff00),
                    );
                }
                "sprite" => {
                    let image_bounds = bounds.dilate(px(4.));
                    window
                        .paint_image(
                            image_bounds,
                            image_bounds,
                            Corners::default(),
                            image,
                            0,
                            false,
                        )
                        .expect("the fractional image must produce a sprite");
                }
                _ => unreachable!("unknown fractional-clip primitive"),
            },
        )
        .size_full()
        .with_subtree_clip(clip)
    }
}

pub(crate) fn warp_fractional_clip_preserves_pixel_centers_for_quads_paths_and_sprites()
-> Result<()> {
    let viewport = size(DevicePixels(32), DevicePixels(32));
    let root = WarpFractionalClipRoot {
        edge: 16.4999,
        primitive: "opaque",
        image: render_image([0, 255, 0, 255]),
    };
    with_warp_window(viewport, root, |context, handle, snapshot| {
        for edge in [16.4999, 16.5001] {
            for primitive in ["opaque", "path", "sprite"] {
                handle.update(context, |root, _, cx| {
                    root.edge = edge;
                    root.primitive = primitive;
                    cx.notify();
                })?;
                let (scene, image) = capture_scene(context, handle.into(), snapshot)?;
                assert_eq!(image.dimensions(), (32, 32));
                assert_eq!(scene.quads.len(), usize::from(primitive == "opaque"));
                assert_eq!(scene.paths.len(), usize::from(primitive == "path"));
                assert_eq!(
                    scene.polychrome_sprites.len(),
                    usize::from(primitive == "sprite")
                );
                let painted = match primitive {
                    "opaque" => Primitive::from(scene.quads[0]),
                    "path" => {
                        assert!(scene.paths[0].tile_count() > 0);
                        Primitive::from(scene.paths[0].clone())
                    }
                    "sprite" => Primitive::from(scene.polychrome_sprites[0]),
                    _ => unreachable!(),
                };
                assert!(painted.transform().is_identity());
                let clip = painted.clip_envelope().conservative_bounds;
                assert_eq!(clip.origin, point(ScaledPixels(0.), ScaledPixels(0.)));
                assert_eq!(clip.right().0, edge);
                assert_eq!(clip.bottom().0, edge);
                assert!(scene.clip_shapes().iter().all(|shape| {
                    shape.radii_x == Corners::default() && shape.radii_y == Corners::default()
                }));
                for (x, y, actual) in image.enumerate_pixels() {
                    let inside = x as f32 + 0.5 < edge && y as f32 + 0.5 < edge;
                    let expected = if inside { [0, 255, 0, 255] } else { [255; 4] };
                    assert_eq!(
                        actual.0, expected,
                        "primitive={primitive}, edge={edge}, pixel ({x}, {y})"
                    );
                }
            }
        }
        Ok(())
    })
}

struct WarpRoundedDepthRoot;

impl Render for WarpRoundedDepthRoot {
    fn render(&mut self, _: &mut Window, _: &mut open_gpui::Context<Self>) -> impl IntoElement {
        canvas(
            |bounds, window, _| {
                let clip = SubtreeClip::try_rounded_rect(
                    Bounds::new(point(px(8.), px(8.)), size(px(16.), px(16.))),
                    Corners::all(px(8.)).map(|radius| size(*radius, *radius)),
                )
                .expect("the foreground's rounded clip must be valid");
                window.prepare_subtree_clip(&clip, bounds)
            },
            |canvas_bounds, foreground_clip, window, _| {
                window.paint_quad(fill(canvas_bounds, rgb(0x204060)));
                let bounds = Bounds::new(point(px(8.), px(8.)), size(px(16.), px(16.)));
                let mut path = PathBuilder::fill();
                path.add_polygon(
                    &[
                        bounds.origin,
                        point(bounds.right(), bounds.top()),
                        point(bounds.right(), bounds.bottom()),
                        point(bounds.left(), bounds.bottom()),
                    ],
                    true,
                );
                window.paint_path(
                    path.build().expect("the depth backdrop path must build"),
                    rgb(0x00ff00),
                );
                window.paint_quad(fill(bounds, hsla(2. / 3., 1., 0.5, 0.5)));
                assert!(
                    window
                        .with_prepared_subtree_clip(&foreground_clip, |window| {
                            window.paint_quad(fill(bounds, rgb(0xff0000)));
                        })
                        .is_some()
                );
            },
        )
        .w(px(32.))
        .h(px(32.))
        .with_subtree_transform(
            SubtreeTransform::try_translation(point(px(4.), px(4.)))
                .expect("the depth fixture's translation must be valid"),
        )
    }
}

pub(crate) fn warp_rounded_clip_depth_preserves_path_and_translucent_corners() -> Result<()> {
    let viewport = size(DevicePixels(96), DevicePixels(96));
    with_warp_window(
        viewport,
        WarpRoundedDepthRoot,
        |context, handle, snapshot| {
            let (scene, image) = capture_scene(context, handle.into(), snapshot)?;
            let scale = handle.update(context, |_, window, _| window.scale_factor())?;
            assert_eq!(scene.paths.len(), 1);
            assert!(scene.paths[0].tile_count() > 0);
            assert_eq!(scene.quads.len(), 3);
            let background = scene.quads.first().context("opaque background missing")?;
            let background_color = background
                .background
                .as_solid()
                .context("the depth background must be solid")?;
            assert_eq!(background_color, rgb(0x204060).into());
            assert_eq!(background_color.a, 1.);
            assert_eq!(background.corner_radii, Corners::default());
            assert_eq!(background.border_widths, Edges::default());
            assert_eq!(
                Primitive::from(*background).transform().components(),
                [1., 1., 4. * scale, 4. * scale]
            );
            let start = background.clip.first_clip as usize;
            let end = start + background.clip.clip_count as usize;
            assert!(background.clip.clip_count > 0);
            assert!(scene.clip_shapes()[start..end].iter().all(|shape| {
                shape.radii_x == Corners::default() && shape.radii_y == Corners::default()
            }));
            let foreground = scene.quads.last().context("opaque foreground missing")?;
            assert_eq!(foreground.corner_radii, Corners::default());
            assert_eq!(foreground.border_widths, Edges::default());
            assert_eq!(foreground.background.as_solid(), Some(rgb(0xff0000).into()));
            let start = foreground.clip.first_clip as usize;
            let end = start + foreground.clip.clip_count as usize;
            assert!(scene.clip_shapes()[start..end].iter().any(|shape| {
                shape.radii_x.top_left == px(8.).scale(scale)
                    && shape.radii_y.top_left == px(8.).scale(scale)
            }));
            assert_pixel(
                &image,
                (6. * scale).floor() as u32,
                (6. * scale).floor() as u32,
                [32, 64, 96, 255],
            );
            assert_pixel(
                &image,
                (20. * scale).floor() as u32,
                (20. * scale).floor() as u32,
                [255, 0, 0, 255],
            );
            for (x, y) in [(13., 13.), (27., 13.), (13., 27.), (27., 27.)] {
                assert_pixel(
                    &image,
                    (x * scale).floor() as u32,
                    (y * scale).floor() as u32,
                    [0, 128, 128, 255],
                );
            }
            assert_pixel(&image, 0, 0, [255; 4]);
            assert_pixel(&image, 95, 95, [255; 4]);
            Ok(())
        },
    )
}

struct WarpPathBackgroundRoot(Background);

impl Render for WarpPathBackgroundRoot {
    fn render(&mut self, _: &mut Window, _: &mut open_gpui::Context<Self>) -> impl IntoElement {
        let background = self.0;
        canvas(
            |_, _, _| (),
            move |bounds, (), window, _| {
                window.paint_quad(fill(bounds, background));
                let mut path = PathBuilder::fill();
                path.add_polygon(
                    &[
                        bounds.origin,
                        point(bounds.right(), bounds.top()),
                        point(bounds.right(), bounds.bottom()),
                        point(bounds.left(), bounds.bottom()),
                    ],
                    true,
                );
                window.paint_path(
                    path.build().expect("the local-background path must build"),
                    background,
                );
            },
        )
        .w(px(32.))
        .h(px(32.))
        .with_subtree_clip(
            SubtreeClip::try_rounded_rect(
                Bounds::new(point(px(0.), px(0.)), size(px(32.), px(32.))),
                Corners::all(px(4.)).map(|radius| size(*radius, *radius)),
            )
            .expect("the local-background clip must be valid"),
        )
        .with_subtree_transform(
            SubtreeTransform::try_new(
                size(2., 0.5),
                point(px(4.), px(4.)),
                SubtreeTransformOrigin::TOP_LEFT,
            )
            .expect("the local-background transform must be valid"),
        )
    }
}

pub(crate) fn warp_transformed_path_gradient_and_pattern_match_local_quad() -> Result<()> {
    let viewport = size(DevicePixels(160), DevicePixels(64));
    for background in [
        linear_gradient(
            137.,
            linear_color_stop(rgb(0x204060), 0.1),
            linear_color_stop(rgb(0xe0a060), 0.9),
        )
        .color_space(ColorSpace::Srgb),
        pattern_slash(rgb(0x80c0e0), 3., 2.),
    ] {
        with_warp_window(
            viewport,
            WarpPathBackgroundRoot(background),
            |context, handle, snapshot| {
                let (scene, _) = capture_scene(context, handle.into(), snapshot)?;
                let scale = handle.update(context, |_, window, _| window.scale_factor())?;
                assert_eq!(scene.quads.len(), 1);
                assert_eq!(scene.paths.len(), 1);
                let mut paints = Vec::new();
                let mut curves = Vec::new();
                let mut tiles = Vec::new();
                scene.paths[0].append_to(&mut paints, &mut curves, &mut tiles);
                assert_eq!(paints.len(), 1);
                assert!(!curves.is_empty() && !tiles.is_empty());
                let paint = paints[0];
                let [scale_x, scale_y, translate_x, translate_y] = paint.transform.components();
                assert_eq!([scale_x, scale_y], [2., 0.5]);
                assert_eq!(paint.bounds, scene.quads[0].bounds);
                assert!(paint.clip.clip_count >= 2);
                let mut quad_scene = scene.clone();
                quad_scene.paths.clear();
                let mut path_scene = scene.clone();
                path_scene.quads.clear();
                let mut reference = WarpRenderer::new(viewport)?;
                let expected = reference
                    .renderer
                    .render_to_image(&quad_scene, WindowBackgroundAppearance::Opaque)?;
                let actual = reference
                    .renderer
                    .render_to_image(&path_scene, WindowBackgroundAppearance::Opaque)?;
                let mut compared = 0;
                let mut first_pixel = None;
                let mut varied = false;
                for (x, y, pixel) in actual.enumerate_pixels() {
                    let local_x = ((x as f32 + 0.5 - translate_x) / scale_x
                        - paint.bounds.origin.x.0)
                        / scale;
                    let local_y = ((y as f32 + 0.5 - translate_y) / scale_y
                        - paint.bounds.origin.y.0)
                        / scale;
                    if !(6.0..26.0).contains(&local_x) || !(6.0..26.0).contains(&local_y) {
                        continue;
                    }
                    let expected = expected.get_pixel(x, y).0;
                    assert!(
                        pixel
                            .0
                            .into_iter()
                            .zip(expected)
                            .all(|(actual, expected)| actual.abs_diff(expected) <= 3),
                        "background={background:?}, local ({local_x}, {local_y}), pixel ({x}, {y}): {:?} != {expected:?}",
                        pixel.0,
                    );
                    if let Some(first) = first_pixel {
                        varied |= pixel.0 != first;
                    } else {
                        first_pixel = Some(pixel.0);
                    }
                    compared += 1;
                }
                assert!(
                    compared >= 100 && varied,
                    "the local-paint oracle must sample a visible, varying background"
                );
                assert_pixel(&actual, 0, 0, [255; 4]);
                assert_pixel(&actual, 159, 63, [255; 4]);
                reference.finish()
            },
        )?;
    }
    Ok(())
}

struct WarpTinyImageRoot {
    image: Arc<RenderImage>,
    clipped: bool,
}

impl Render for WarpTinyImageRoot {
    fn render(
        &mut self,
        window: &mut Window,
        _: &mut open_gpui::Context<Self>,
    ) -> impl IntoElement {
        let image = self.image.clone();
        let scale = window.scale_factor();
        let (origin, extent) = if self.clipped {
            (2e-8, 4e-8)
        } else {
            (0., 1e-7)
        };
        let clip = SubtreeClip::try_rect(Bounds::new(
            point(px(origin / scale), px(origin / scale)),
            size(px(extent / scale), px(extent / scale)),
        ))
        .expect("the tiny-image clip must be valid");
        canvas(
            |_, _, _| (),
            move |_, (), window, _| {
                let bounds = Bounds::new(
                    point(px(0.), px(0.)),
                    size(px(1e-7 / scale), px(1e-7 / scale)),
                );
                window
                    .paint_image(bounds, bounds, Corners::default(), image, 0, false)
                    .expect("the enlarged tiny image must produce a sprite");
            },
        )
        .size_full()
        .with_subtree_clip(clip)
        .with_subtree_transform(
            SubtreeTransform::try_new(
                size(1e8, 1e8),
                point(px(0.), px(0.)),
                SubtreeTransformOrigin::TOP_LEFT,
            )
            .expect("the tiny-image transform must be valid"),
        )
    }
}

pub(crate) fn warp_tiny_image_bounds_preserve_full_and_clipped_uvs() -> Result<()> {
    let viewport = size(DevicePixels(32), DevicePixels(32));
    let root = WarpTinyImageRoot {
        image: Arc::new(RenderImage::new(vec![image::Frame::new(
            RgbaImage::from_fn(16, 16, |x, y| {
                image::Rgba(match (x < 8, y < 8) {
                    (true, true) => [0, 0, 255, 255],
                    (false, true) => [0, 255, 0, 255],
                    (true, false) => [255, 0, 0, 255],
                    (false, false) => [0, 255, 255, 255],
                })
            }),
        )])),
        clipped: false,
    };
    with_warp_window(viewport, root, |context, handle, snapshot| {
        for clipped in [false, true] {
            handle.update(context, |root, _, cx| {
                root.clipped = clipped;
                cx.notify();
            })?;
            let (scene, image) = capture_scene(context, handle.into(), snapshot)?;
            assert_eq!(scene.polychrome_sprites.len(), 1);
            let sprite = scene.polychrome_sprites[0];
            let [scale_x, scale_y, ..] = sprite.renderer_transform().components();
            assert!(sprite.bounds.size.width.0 > 0. && sprite.bounds.size.width.0 < 1e-6);
            assert!(sprite.bounds.size.height.0 > 0. && sprite.bounds.size.height.0 < 1e-6);
            assert!(scale_x > 9e7 && scale_y > 9e7);
            assert!((sprite.bounds.size.width.0 * scale_x - 10.).abs() < 1e-4);
            assert!((sprite.bounds.size.height.0 * scale_y - 10.).abs() < 1e-4);
            let samples = if clipped {
                [
                    (2, 2, [255, 0, 0, 255]),
                    (4, 2, [255, 0, 0, 255]),
                    (2, 4, [255, 0, 0, 255]),
                    (5, 5, [255, 255, 0, 255]),
                ]
            } else {
                [
                    (2, 2, [255, 0, 0, 255]),
                    (7, 2, [0, 255, 0, 255]),
                    (2, 7, [0, 0, 255, 255]),
                    (7, 7, [255, 255, 0, 255]),
                ]
            };
            for (x, y, expected) in samples {
                assert_eq!(
                    image.get_pixel(x, y).0,
                    expected,
                    "clipped={clipped}, pixel ({x}, {y})"
                );
            }
            assert_pixel(&image, 12, 12, [255; 4]);
            if clipped {
                assert_pixel(&image, 1, 1, [255; 4]);
                assert_pixel(&image, 7, 2, [255; 4]);
                assert_pixel(&image, 2, 7, [255; 4]);
                assert_pixel(&image, 7, 7, [255; 4]);
            } else {
                assert_pixel(&image, 1, 1, [255, 0, 0, 255]);
            }
        }
        Ok(())
    })
}

fn warp_devices() -> Result<DirectXDevices> {
    let mut device = None;
    let mut device_context = None;
    unsafe {
        D3D11CreateDevice(
            None,
            D3D_DRIVER_TYPE_WARP,
            HMODULE::default(),
            D3D11_CREATE_DEVICE_BGRA_SUPPORT,
            Some(&[D3D_FEATURE_LEVEL_11_0]),
            D3D11_SDK_VERSION,
            Some(&mut device),
            None,
            Some(&mut device_context),
        )
    }
    .context("creating the WARP renderer device")?;
    let device = device.context("WARP must return a renderer device")?;
    let device_context = device_context.context("WARP must return an immediate context")?;
    let dxgi_device: IDXGIDevice = device.cast()?;
    let adapter: IDXGIAdapter1 = unsafe { dxgi_device.GetAdapter() }?.cast()?;
    let dxgi_factory = unsafe { adapter.GetParent::<IDXGIFactory6>() }?;
    Ok(DirectXDevices {
        adapter,
        dxgi_factory,
        device,
        device_context,
    })
}

fn image_key(image_id: usize) -> AtlasKey {
    AtlasKey::Image(RenderImageParams {
        image_id: ImageId(image_id),
        frame_index: 0,
    })
}

fn upload(atlas: &DirectXAtlas, key: AtlasKey, bgra: &[u8; 4]) -> Result<AtlasTile> {
    let bytes = bgra.repeat(12 * 12);
    atlas
        .get_or_insert_with(key, &mut || {
            Ok(Some((
                size(DevicePixels(12), DevicePixels(12)),
                Cow::Borrowed(bytes.as_slice()),
            )))
        })?
        .context("a complete WARP image must upload")
}

fn sprite_scene(template: &Scene, tiles: &[AtlasTile]) -> Scene {
    assert_eq!(template.polychrome_sprites.len(), 2);
    assert!(!tiles.is_empty() && tiles.len() <= template.polychrome_sprites.len());
    let mut scene = template.clone();
    scene.polychrome_sprites.truncate(tiles.len());
    for (sprite, tile) in scene.polychrome_sprites.iter_mut().zip(tiles) {
        assert!(sprite.renderer_transform().is_identity());
        assert!(sprite.clip.clip_count > 0);
        let clip_start = sprite.clip.first_clip as usize;
        let clip_end = clip_start + sprite.clip.clip_count as usize;
        assert!(clip_end <= template.clip_shapes().len());
        sprite.tile = *tile;
    }
    assert_eq!(scene.clip_shapes(), template.clip_shapes());
    scene
}

fn assert_pixel(image: &RgbaImage, x: u32, y: u32, expected: [u8; 4]) {
    let actual = image.get_pixel(x, y).0;
    assert!(
        actual
            .iter()
            .zip(expected)
            .all(|(actual, expected)| actual.abs_diff(expected) <= 2),
        "WARP pixel ({x}, {y}): {actual:?} != {expected:?}"
    );
}

fn assert_sprite_pixel(image: &RgbaImage, sprite: &PolychromeSprite, expected: [u8; 4]) {
    let center = sprite.bounds.center();
    assert_pixel(image, center.x.0 as u32, center.y.0 as u32, expected);
}

pub(crate) fn warp_raw_scene_receipt_retains_original_regions_and_views(
    template: &Scene,
    viewport: Size<DevicePixels>,
) -> Result<()> {
    let mut fixture = WarpRenderer::new(viewport)?;
    let atlas = fixture.renderer.atlas.clone();
    let key = image_key(1);
    let original = upload(&atlas, key.clone(), &[0, 255, 0, 255])?;
    let scene = sprite_scene(template, &[original, original]);
    let preflight = atlas.preflight(&scene)?;
    let view = preflight
        .texture_view(original.texture_instance())
        .and_then(|views| views[0].as_ref())
        .context("the receipt must freeze the actual SRV")?
        .clone();
    assert_eq!(
        atlas.remove_with_diagnostics(&key).outcome,
        AtlasRemoveOutcome::TextureRetained
    );
    let replacement = upload(&atlas, key.clone(), &[0, 0, 255, 255])?;
    assert_eq!(replacement.texture_instance(), original.texture_instance());
    assert!(!replacement.bounds.intersects(&original.bounds));
    let current_view = atlas
        .get_texture_view(original.texture_instance())
        .and_then(|views| views[0].clone())
        .context("the receipt must keep the original resource resident")?;
    assert_eq!(view.as_raw(), current_view.as_raw());
    fixture.renderer.draw_scene_to_target(
        &scene,
        WindowBackgroundAppearance::Opaque,
        &preflight,
    )?;
    let original_pixels = fixture.renderer.read_render_target_to_image()?;
    assert_sprite_pixel(
        &original_pixels,
        &scene.polychrome_sprites[0],
        [0, 255, 0, 255],
    );
    assert_sprite_pixel(
        &original_pixels,
        &scene.polychrome_sprites[1],
        [0, 255, 0, 255],
    );
    drop(preflight);
    assert_eq!(
        atlas.remove_with_diagnostics(&key).outcome,
        AtlasRemoveOutcome::TextureFreed
    );
    assert!(
        atlas
            .get_texture_view(original.texture_instance())
            .is_none()
    );
    assert_eq!(
        fixture
            .renderer
            .draw(&scene, WindowBackgroundAppearance::Opaque)?,
        PlatformWindowPresentOutcome::RepaintRequired
    );
    assert!(
        fixture
            .renderer
            .render_to_image(&scene, WindowBackgroundAppearance::Opaque)
            .is_err()
    );
    assert_eq!(
        fixture.renderer.read_render_target_to_image()?,
        original_pixels
    );
    let reconstructed = upload(&atlas, key, &[255, 0, 0, 255])?;
    assert_eq!(reconstructed.texture_id, original.texture_id);
    assert_ne!(
        reconstructed.texture_instance(),
        original.texture_instance()
    );
    assert_eq!(
        fixture
            .renderer
            .draw(&scene, WindowBackgroundAppearance::Opaque)?,
        PlatformWindowPresentOutcome::RepaintRequired
    );
    assert!(
        fixture
            .renderer
            .render_to_image(&scene, WindowBackgroundAppearance::Opaque)
            .is_err()
    );
    assert_eq!(
        fixture.renderer.read_render_target_to_image()?,
        original_pixels
    );
    let fresh = sprite_scene(template, &[reconstructed]);
    let fresh_pixels = fixture
        .renderer
        .render_to_image(&fresh, WindowBackgroundAppearance::Opaque)?;
    assert_sprite_pixel(
        &fresh_pixels,
        &fresh.polychrome_sprites[0],
        [0, 0, 255, 255],
    );
    assert_eq!(
        fixture
            .renderer
            .draw(&fresh, WindowBackgroundAppearance::Opaque)?,
        PlatformWindowPresentOutcome::Submitted
    );
    fixture.finish()
}

pub(crate) fn warp_mixed_valid_and_stale_resources_refuse_before_target_clear(
    template: &Scene,
    viewport: Size<DevicePixels>,
) -> Result<()> {
    let mut fixture = WarpRenderer::new(viewport)?;
    let atlas = fixture.renderer.atlas.clone();
    let key = image_key(2);
    let old_tile = upload(&atlas, key.clone(), &[0, 255, 0, 255])?;
    atlas.handle_device_lost(&fixture.devices.device, &fixture.devices.device_context);
    let current = upload(&atlas, key, &[0, 0, 255, 255])?;
    assert_eq!(old_tile.texture_id, current.texture_id);
    assert_ne!(old_tile.texture_instance(), current.texture_instance());
    let before = fixture
        .renderer
        .render_to_image(&Scene::default(), WindowBackgroundAppearance::Opaque)?;
    let mixed = sprite_scene(template, &[current, old_tile]);
    assert_eq!(
        fixture
            .renderer
            .draw(&mixed, WindowBackgroundAppearance::Opaque)?,
        PlatformWindowPresentOutcome::RepaintRequired
    );
    assert_eq!(fixture.renderer.read_render_target_to_image()?, before);
    assert!(
        fixture
            .renderer
            .render_to_image(&mixed, WindowBackgroundAppearance::Opaque)
            .is_err()
    );
    assert_eq!(fixture.renderer.read_render_target_to_image()?, before);
    let fresh = sprite_scene(template, &[current]);
    let fresh_pixels = fixture
        .renderer
        .render_to_image(&fresh, WindowBackgroundAppearance::Opaque)?;
    assert_sprite_pixel(
        &fresh_pixels,
        &fresh.polychrome_sprites[0],
        [255, 0, 0, 255],
    );
    assert_sprite_pixel(
        &fresh_pixels,
        &template.polychrome_sprites[1],
        [255, 255, 255, 255],
    );
    assert_eq!(
        fixture
            .renderer
            .draw(&fresh, WindowBackgroundAppearance::Opaque)?,
        PlatformWindowPresentOutcome::Submitted
    );
    fixture.finish()
}

pub(crate) fn warp_renderer_recovery_invalidates_receipts_and_requires_fresh_paint(
    template: &Scene,
    viewport: Size<DevicePixels>,
) -> Result<()> {
    let mut fixture = WarpRenderer::new(viewport)?;
    let atlas = fixture.renderer.atlas.clone();
    let key = image_key(3);
    let old_tile = upload(&atlas, key.clone(), &[0, 255, 0, 255])?;
    let old_scene = sprite_scene(template, &[old_tile]);
    let old_preflight = atlas.preflight(&old_scene)?;
    let replacement_devices = warp_devices()?;
    assert_ne!(
        replacement_devices.device.as_raw(),
        fixture.devices.device.as_raw()
    );
    fixture.renderer.handle_device_lost(&replacement_devices)?;
    fixture.devices = replacement_devices;
    assert!(old_preflight.validate().is_err());
    assert!(
        fixture
            .renderer
            .render_to_image(&old_scene, WindowBackgroundAppearance::Opaque)
            .is_err()
    );
    let current = upload(&atlas, key.clone(), &[255, 0, 0, 255])?;
    let fresh = sprite_scene(template, &[current]);
    let fresh_preflight = atlas.preflight(&fresh)?;
    assert_eq!(current.texture_id, old_tile.texture_id);
    assert!(current.texture_generation > old_tile.texture_generation);
    assert_eq!(
        atlas.remove_with_diagnostics(&key).outcome,
        AtlasRemoveOutcome::TextureRetained
    );
    assert_eq!(
        fixture
            .renderer
            .draw(&old_scene, WindowBackgroundAppearance::Opaque)?,
        PlatformWindowPresentOutcome::RepaintRequired
    );
    let before_refused_receipt = fixture
        .renderer
        .render_to_image(&Scene::default(), WindowBackgroundAppearance::Opaque)?;
    assert!(
        fixture
            .renderer
            .draw_scene_to_target(
                &old_scene,
                WindowBackgroundAppearance::Opaque,
                &old_preflight,
            )
            .is_err()
    );
    assert_eq!(
        fixture.renderer.read_render_target_to_image()?,
        before_refused_receipt
    );
    drop(old_preflight);
    assert!(atlas.get_texture_view(current.texture_instance()).is_some());
    assert_eq!(
        fixture
            .renderer
            .draw(&old_scene, WindowBackgroundAppearance::Opaque)?,
        PlatformWindowPresentOutcome::RepaintRequired
    );
    let image = fixture
        .renderer
        .render_to_image(&fresh, WindowBackgroundAppearance::Opaque)?;
    assert_sprite_pixel(&image, &fresh.polychrome_sprites[0], [0, 0, 255, 255]);
    assert_eq!(
        fixture
            .renderer
            .draw(&fresh, WindowBackgroundAppearance::Opaque)?,
        PlatformWindowPresentOutcome::Submitted
    );
    drop(fresh_preflight);
    assert!(atlas.get_texture_view(current.texture_instance()).is_none());
    fixture.finish()
}
