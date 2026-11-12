use super::{cached_paint_atlas_tests::RejectOnceAtlas, *};
use crate::{
    Empty, Image, ImageFormat, ImageSource, ImgResourceLoader, Resource, SubtreeClipExt as _,
    SubtreeTransformExt as _, SubtreeTransformOrigin, TestAppContext, blue, canvas, div, green,
    red,
};
use image::{Frame, Rgba, RgbaImage};
use std::{
    cell::Cell,
    io::Cursor,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};

#[crate::test]
fn bounds_change_refreshes_mouse_position_after_native_ingress(cx: &mut TestAppContext) {
    let handle = cx.add_window(|_, _| Empty);
    let mut native = cx.test_window(handle.into());
    let position = point(px(70.), px(80.));
    cx.update_window(handle.into(), |_, window, app| {
        window.dispatch_event(
            PlatformInput::MouseMove(MouseMoveEvent {
                position,
                pressed_button: None,
                modifiers: Modifiers::none(),
            }),
            app,
        );
        assert_eq!(window.mouse_position(), position);
    })
    .unwrap();

    cx.update(|app| {
        native.simulate_resize(size(px(640.), px(480.)));
        app.update_window(handle.into(), |_, window, _| {
            assert_eq!(window.mouse_position(), position);
        })
        .unwrap();
    });
    cx.run_until_parked();
    cx.update_window(handle.into(), |_, window, _| {
        assert_eq!(window.mouse_position(), native.mouse_position());
        assert_ne!(window.mouse_position(), position);
    })
    .unwrap();
}

struct CapturedPointerProbe(PointerCaptureHandle);

impl Render for CapturedPointerProbe {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("bounds-captured-pointer")
            .size_full()
            .track_pointer_capture(&self.0)
    }
}

#[crate::test]
fn bounds_change_preserves_captured_pointer_and_quiesced_mouse_cache(cx: &mut TestAppContext) {
    let handle =
        cx.add_window(|window, _| CapturedPointerProbe(window.new_pointer_capture_handle()));
    let capture_handle = handle.read_with(cx, |view, _| view.0).unwrap();
    cx.update_window(handle.into(), |_, window, _| {
        let _ = window.activate_window();
    })
    .unwrap();
    cx.run_until_parked();
    let mut native = cx.test_window(handle.into());
    let position = point(px(20.), px(30.));
    let capture = cx
        .update_window(handle.into(), |_, window, app| {
            window.draw(app).clear(app);
            window.dispatch_event(
                PlatformInput::MouseDown(crate::MouseDownEvent {
                    position,
                    button: MouseButton::Left,
                    modifiers: Modifiers::none(),
                    click_count: 1,
                    first_mouse: false,
                }),
                app,
            );
            window
                .capture_pointer(&capture_handle, MouseButton::Left)
                .unwrap();
            window.captured_pointer().unwrap()
        })
        .unwrap();
    native.simulate_resize(size(px(640.), px(480.)));
    cx.run_until_parked();
    cx.update_window(handle.into(), |_, window, app| {
        assert_eq!(window.captured_pointer(), Some(capture));
        assert_eq!(window.mouse_position(), position);
        window.quiesce_interaction(app);
    })
    .unwrap();
    native.simulate_resize(size(px(600.), px(400.)));
    cx.run_until_parked();
    cx.update_window(handle.into(), |_, window, _| {
        assert!(!window.user_interaction_is_admitted());
        assert_eq!(window.mouse_position(), position);
    })
    .unwrap();
}

#[crate::test]
fn touch_prediction_getter_tracks_policy_without_requesting_a_frame(cx: &mut TestAppContext) {
    let handle = cx.add_window(|_, _| Empty);
    let native = cx.test_window(handle.into());
    cx.run_until_parked();
    let wakes = native.frame_wake_count();
    handle
        .update(cx, |_, window, _| {
            assert!(window.touch_prediction_enabled());
            window.set_touch_prediction_enabled(false);
            assert!(!window.touch_prediction_enabled());
            window.set_touch_prediction_enabled(true);
            assert!(window.touch_prediction_enabled());
        })
        .unwrap();
    assert_eq!(native.frame_wake_count(), wakes);
}

struct PaintedOutputProbe {
    color: Rc<Cell<Hsla>>,
    invalid: Rc<Cell<bool>>,
}

impl Render for PaintedOutputProbe {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let color = self.color.get();
        let invalid = self.invalid.get();
        div()
            .size_full()
            .child(
                canvas(
                    |_, _, _| (),
                    move |_, _, window, _| {
                        window.paint_quad(fill(
                            Bounds::new(point(px(4.), px(6.)), size(px(30.), px(20.))),
                            color,
                        ));
                        window.paint_underline(
                            point(px(5.), px(28.)),
                            px(24.),
                            &UnderlineStyle {
                                thickness: px(1.),
                                color: Some(color),
                                wavy: true,
                            },
                        );
                        if invalid {
                            window.paint_quad(fill(
                                Bounds::new(point(px(f32::INFINITY), px(0.)), size(px(1.), px(1.))),
                                color,
                            ));
                        }
                    },
                )
                .size(px(60.))
                .with_subtree_clip(
                    SubtreeClip::try_rounded_rect(
                        Bounds::new(point(px(0.), px(0.)), size(px(60.), px(60.))),
                        Corners {
                            top_left: size(px(8.), px(8.)),
                            top_right: size(px(8.), px(8.)),
                            bottom_left: size(px(8.), px(8.)),
                            bottom_right: size(px(8.), px(8.)),
                        },
                    )
                    .unwrap(),
                )
                .with_subtree_transform(
                    SubtreeTransform::try_new(
                        size(1.25, 0.75),
                        point(px(3.), px(4.)),
                        SubtreeTransformOrigin::TOP_LEFT,
                    )
                    .unwrap(),
                ),
            )
            .child(
                canvas(
                    |_, _, _| (),
                    |_, _, window, _| {
                        window.paint_quad(fill(
                            Bounds::new(point(px(80.), px(6.)), size(px(10.), px(10.))),
                            green(),
                        ));
                        let _ = window.insert_scene_primitive(MonochromeSprite {
                            order: 0,
                            pad: 0,
                            bounds: window.device_local_bounds(Bounds::new(
                                point(px(80.), px(20.)),
                                size(px(1.), px(1.)),
                            )),
                            clip: Default::default(),
                            color: green(),
                            tile: RejectOnceAtlas::tile(crate::AtlasTextureKind::Monochrome),
                            transform: PrimitiveTransform::IDENTITY,
                        });
                    },
                )
                .size(px(60.)),
            )
    }
}

#[crate::test]
fn painted_output_accessors_keep_accepted_scene_after_candidate_refusal(cx: &mut TestAppContext) {
    let color = Rc::new(Cell::new(red()));
    let atlas = Arc::new(RejectOnceAtlas::new());
    let handle = cx.add_window(|window, _| {
        window.set_sprite_atlas_for_test(atlas.clone());
        PaintedOutputProbe {
            color: color.clone(),
            invalid: Rc::new(Cell::new(false)),
        }
    });
    cx.run_until_parked();
    cx.test_window(handle.into())
        .defer_frame_requests_for_test();
    cx.update_window(handle.into(), |_, window, app| {
        let revision = window.rendered_frame_revision();
        let accepted = window.painted_quads();
        let clips = window.rendered_scene().clip_shapes().to_vec();
        assert_eq!(accepted.len(), 2);
        assert_ne!(accepted[0].transform, PrimitiveTransform::IDENTITY);
        let clip = accepted[0].clip;
        let first = clip.first_clip as usize;
        let end = first + clip.clip_count as usize;
        assert!(clips[first..end].iter().any(|shape| {
            shape.radii_x.top_left > Default::default()
                && shape.radii_y.top_left > Default::default()
        }));
        assert_eq!(window.painted_underlines()[0].color, red());
        color.set(blue());
        atlas.fail_next_lease();
        window.refresh();
        window.draw(app).clear(app);
        assert_eq!(window.rendered_frame_revision(), revision);
        let unchanged = window.painted_quads();
        assert_eq!(window.rendered_scene().clip_shapes(), clips);
        assert_eq!(unchanged.len(), accepted.len());
        for (actual, expected) in unchanged.iter().zip(&accepted) {
            assert_eq!(actual.bounds, expected.bounds);
            assert_eq!(actual.background, expected.background);
            assert_eq!(actual.clip, expected.clip);
            assert_eq!(actual.transform, expected.transform);
        }
        assert_eq!(window.painted_underlines()[0].color, red());
        window.draw(app).clear(app);
        assert_eq!(window.rendered_frame_revision(), revision + 1);
        assert!(
            window
                .painted_quads()
                .iter()
                .any(|quad| quad.background.solid == blue())
        );
        assert_eq!(window.painted_underlines()[0].color, blue());
    })
    .unwrap();
}

#[crate::test]
fn painted_output_accessors_exclude_late_invalidated_subtrees(cx: &mut TestAppContext) {
    let atlas = Arc::new(RejectOnceAtlas::new());
    let handle = cx.add_window(|window, _| {
        window.set_sprite_atlas_for_test(atlas.clone());
        PaintedOutputProbe {
            color: Rc::new(Cell::new(red())),
            invalid: Rc::new(Cell::new(true)),
        }
    });
    cx.run_until_parked();
    handle
        .update(cx, |_, window, _| {
            let quads = window.painted_quads();
            assert_eq!(quads.len(), 1);
            assert_eq!(quads[0].background.solid, green());
            assert!(window.painted_underlines().is_empty());
        })
        .unwrap();
}

#[crate::test]
fn image_source_cache_inspection_is_passive_for_pending_loaded_and_removed_assets(
    cx: &mut TestAppContext,
) {
    let mut bytes = Cursor::new(Vec::new());
    RgbaImage::from_pixel(1, 1, Rgba([1, 2, 3, 255]))
        .write_to(&mut bytes, image::ImageFormat::Png)
        .unwrap();
    let image = Arc::new(Image::from_bytes(ImageFormat::Png, bytes.into_inner()));
    let source = ImageSource::Image(image.clone());
    let resource = ImageSource::Resource(Resource::Embedded("missing-cache-inspection.png".into()));
    let custom_calls = Arc::new(AtomicUsize::new(0));
    let custom = ImageSource::Custom(Arc::new({
        let custom_calls = custom_calls.clone();
        move |_, _| {
            custom_calls.fetch_add(1, Ordering::Relaxed);
            None
        }
    }));
    let rendered = ImageSource::Render(Arc::new(RenderImage::new(vec![Frame::new(
        RgbaImage::from_pixel(1, 1, Rgba([1, 2, 3, 255])),
    )])));
    let handle = cx.add_window(|_, _| Empty);
    handle
        .update(cx, |_, window, app| {
            for source in [&source, &resource, &custom, &rendered] {
                assert!(!source.is_asset_cached(app));
            }
            assert!(!image.is_asset_cached(app));
            assert_eq!(custom_calls.load(Ordering::Relaxed), 0);
            assert!(source.get_data(None, window, app).is_none());
            assert!(resource.get_data(None, window, app).is_none());
            assert!(source.is_asset_cached(app));
            assert!(image.is_asset_cached(app));
            assert!(resource.is_asset_cached(app));
            let ImageSource::Resource(resource) = &resource else {
                unreachable!()
            };
            assert!(app.has_asset::<ImgResourceLoader>(resource));
        })
        .unwrap();
    cx.run_until_parked();
    handle
        .update(cx, |_, window, app| {
            assert!(source.get_data(None, window, app).unwrap().is_ok());
            assert!(resource.get_data(None, window, app).unwrap().is_err());
            assert!(source.is_asset_cached(app));
            assert!(image.is_asset_cached(app));
            assert!(resource.is_asset_cached(app));
            source.remove_asset(app);
            resource.remove_asset(app);
            assert!(!source.is_asset_cached(app));
            assert!(!image.is_asset_cached(app));
            assert!(!resource.is_asset_cached(app));
            assert!(!custom.is_asset_cached(app));
            assert_eq!(custom_calls.load(Ordering::Relaxed), 0);
        })
        .unwrap();
}

struct AnimatedAtlasProbe {
    image: Arc<RenderImage>,
    frames_to_paint: Rc<Cell<usize>>,
}

impl Render for AnimatedAtlasProbe {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let image = self.image.clone();
        let frames_to_paint = self.frames_to_paint.get();
        canvas(
            |_, _, _| (),
            move |bounds, _, window, _| {
                for frame_index in 0..frames_to_paint {
                    window
                        .paint_image(
                            bounds,
                            bounds,
                            Default::default(),
                            image.clone(),
                            frame_index,
                            false,
                        )
                        .unwrap();
                }
            },
        )
        .size(px(20.))
    }
}

#[crate::test]
fn image_atlas_membership_requires_all_frames_and_is_local_to_the_window(cx: &mut TestAppContext) {
    let image = Arc::new(RenderImage::new(vec![
        Frame::new(RgbaImage::from_pixel(1, 1, Rgba([1, 2, 3, 255]))),
        Frame::new(RgbaImage::from_pixel(1, 1, Rgba([4, 5, 6, 255]))),
    ]));
    let first_frames = Rc::new(Cell::new(1));
    let second_frames = Rc::new(Cell::new(1));
    let first = cx.add_window(|_, _| AnimatedAtlasProbe {
        image: image.clone(),
        frames_to_paint: first_frames.clone(),
    });
    let second = cx.add_window(|_, _| AnimatedAtlasProbe {
        image: image.clone(),
        frames_to_paint: second_frames.clone(),
    });
    cx.run_until_parked();
    let empty = RenderImage::new(Vec::<Frame>::new());
    for handle in [first, second] {
        cx.update_window(handle.into(), |_, window, _| {
            assert!(!window.has_image_atlas_entry(&image));
            assert!(!window.has_image_atlas_entry(&empty));
        })
        .unwrap();
    }

    cx.update_window(first.into(), |_, window, app| {
        first_frames.set(2);
        window.refresh();
        window.draw(app).clear(app);
        assert!(window.has_image_atlas_entry(&image));
    })
    .unwrap();
    cx.update_window(second.into(), |_, window, app| {
        assert!(!window.has_image_atlas_entry(&image));
        second_frames.set(2);
        window.refresh();
        window.draw(app).clear(app);
        assert!(window.has_image_atlas_entry(&image));
    })
    .unwrap();

    cx.update_window(first.into(), |_, window, _| {
        let revision = window.rendered_frame_revision();
        window.drop_image(image.clone()).unwrap();
        assert!(!window.has_image_atlas_entry(&image));
        assert_eq!(window.rendered_frame_revision(), revision);
        assert_eq!(window.rendered_frame.scene.polychrome_sprites.len(), 2);
    })
    .unwrap();
    cx.update_window(second.into(), |_, window, _| {
        assert!(window.has_image_atlas_entry(&image));
    })
    .unwrap();
}
