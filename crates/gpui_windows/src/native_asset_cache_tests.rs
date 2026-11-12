#![cfg(feature = "test-support")]

use super::*;
use crate::directx_renderer::atlas_render_tests;
use crate::native_test_observation::NativeWindowTestMessageDisposition;
use futures::channel::oneshot;
use open_gpui::{
    App, ApplicationHandle, Asset, AtlasAccessOutcome, AtlasKey, AtlasRemoveOutcome, ImageId,
    ImagePaintDiagnostic, InteractiveElement, RenderImage, VisualTestPlatform, WindowFrameCapture,
    WindowHandle, WindowInitialPresentationStatus, img, rgb,
};
use std::{
    borrow::Cow,
    collections::VecDeque,
    future::Future,
    hash::{Hash, Hasher},
    sync::{
        Mutex,
        atomic::{AtomicUsize, Ordering},
    },
};

type NativeAssetResult = Result<Arc<RenderImage>, &'static str>;

#[derive(Clone)]
struct NativeAssetSource {
    id: usize,
    loads: Arc<AtomicUsize>,
    completions: Arc<AtomicUsize>,
    receivers: Arc<Mutex<VecDeque<oneshot::Receiver<NativeAssetResult>>>>,
}

impl NativeAssetSource {
    fn new(
        id: usize,
        receivers: impl IntoIterator<Item = oneshot::Receiver<NativeAssetResult>>,
    ) -> Self {
        Self {
            id,
            loads: Arc::new(AtomicUsize::new(0)),
            completions: Arc::new(AtomicUsize::new(0)),
            receivers: Arc::new(Mutex::new(receivers.into_iter().collect())),
        }
    }

    fn load_count(&self) -> usize {
        self.loads.load(Ordering::SeqCst)
    }
}

impl Hash for NativeAssetSource {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.id.hash(state);
    }
}

struct NativeAsset;

impl Asset for NativeAsset {
    type Source = NativeAssetSource;
    type Output = NativeAssetResult;

    fn load(
        source: Self::Source,
        _: &mut App,
    ) -> impl Future<Output = Self::Output> + Send + 'static {
        source.loads.fetch_add(1, Ordering::SeqCst);
        let receiver = source
            .receivers
            .lock()
            .expect("native asset receiver queue must remain usable")
            .pop_front()
            .expect("each actual asset load must consume its own held receiver");
        async move {
            let result = receiver
                .await
                .expect("the native host must supply the asset result");
            source.completions.fetch_add(1, Ordering::SeqCst);
            result
        }
    }
}

fn native_asset_image(rgba: [u8; 4]) -> Arc<RenderImage> {
    Arc::new(RenderImage::new(vec![image::Frame::new(
        image::RgbaImage::from_pixel(1, 1, image::Rgba([rgba[2], rgba[1], rgba[0], rgba[3]])),
    )]))
}

#[test]
fn warp_raw_scene_receipt_retains_original_regions_and_views() {
    atlas_render_tests::with_warp_scene_template(
        atlas_render_tests::warp_raw_scene_receipt_retains_original_regions_and_views,
    )
    .expect("real WARP rendering must consume the pinned original image region and SRV");
}

#[test]
fn warp_mixed_valid_and_stale_resources_refuse_before_target_clear() {
    atlas_render_tests::with_warp_scene_template(
        atlas_render_tests::warp_mixed_valid_and_stale_resources_refuse_before_target_clear,
    )
    .expect("mixed valid and stale WARP scenes must refuse before touching the target");
}

#[test]
fn warp_renderer_recovery_invalidates_receipts_and_requires_fresh_paint() {
    atlas_render_tests::with_warp_scene_template(
        atlas_render_tests::warp_renderer_recovery_invalidates_receipts_and_requires_fresh_paint,
    )
    .expect("actual WARP renderer reconstruction must reject old receipts and permit fresh paint");
}

#[test]
fn warp_nonzero_instance_batches_survive_growth_and_smaller_frames() {
    atlas_render_tests::warp_nonzero_instance_batches_survive_growth_and_smaller_frames()
        .expect("the WARP renderer must read each batch from the current whole-buffer view");
}

#[test]
fn warp_hollow_border_strips_match_full_quad_under_projected_rounded_clip() {
    atlas_render_tests::warp_hollow_border_strips_match_full_quad_under_projected_rounded_clip()
        .expect("WARP strips must preserve the unsplit border's pixels");
}

#[test]
fn warp_fractional_clip_preserves_pixel_centers_for_quads_paths_and_sprites() {
    atlas_render_tests::warp_fractional_clip_preserves_pixel_centers_for_quads_paths_and_sprites()
        .expect("WARP quads, analytic paths and sprites must preserve fractional clip coverage");
}

#[test]
fn warp_rounded_clip_depth_preserves_path_and_translucent_corners() {
    atlas_render_tests::warp_rounded_clip_depth_preserves_path_and_translucent_corners().expect(
        "WARP opaque depth and rounded fallback must preserve background, path and translucent pixels",
    );
}

#[test]
fn warp_transformed_path_gradient_and_pattern_match_local_quad() {
    atlas_render_tests::warp_transformed_path_gradient_and_pattern_match_local_quad()
        .expect("WARP path gradients and patterns must retain local paint coordinates");
}

#[test]
fn warp_tiny_image_bounds_preserve_full_and_clipped_uvs() {
    atlas_render_tests::warp_tiny_image_bounds_preserve_full_and_clipped_uvs()
        .expect("WARP tiny enlarged images must preserve original UVs under asymmetric clipping");
}

#[derive(Clone, Copy, Debug)]
struct NativeAssetPaint {
    window_id: WindowId,
    previous_revision: u64,
    image: Option<ImageId>,
}

#[derive(Default)]
struct NativeAssetCounters {
    renders: Cell<usize>,
    paints: Cell<usize>,
    last_paint: Cell<Option<NativeAssetPaint>>,
}

struct NativeAssetRoot {
    interested: NativeAssetSource,
    passive: NativeAssetSource,
    counters: Rc<NativeAssetCounters>,
}

impl Render for NativeAssetRoot {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.counters.renders.set(self.counters.renders.get() + 1);
        let result = window.use_asset::<NativeAsset>(&self.interested, cx);
        assert_eq!(
            result,
            window.use_asset::<NativeAsset>(&self.interested, cx)
        );
        let _passive_window_result: Option<NativeAssetResult> =
            window.get_asset::<NativeAsset>(&self.passive, cx);
        let _passive_app_result: Option<NativeAssetResult> =
            cx.fetch_asset::<NativeAsset>(&self.passive);
        let image =
            result.map(|result| result.expect("the interested asset must load successfully"));
        let image_id = image.as_ref().map(|image| image.id);
        let counters = self.counters.clone();
        div()
            .relative()
            .size_full()
            .bg(rgb(if image.is_some() { 0x000000 } else { 0x202020 }))
            .child(
                canvas(
                    |_, _, _| {},
                    move |_, (), window, _| {
                        counters.paints.set(counters.paints.get() + 1);
                        counters.last_paint.set(Some(NativeAssetPaint {
                            window_id: window.window_handle().window_id(),
                            previous_revision: window.rendered_frame_revision(),
                            image: image_id,
                        }));
                    },
                )
                .absolute()
                .size_full(),
            )
            .children(image.map(|image| {
                img(image)
                    .id("owning-asset-result")
                    .absolute()
                    .left(px(16.))
                    .top(px(16.))
                    .size(px(64.))
            }))
    }
}

fn pump_native_assets_until(
    platform: &WindowsPlatform,
    visual: &VisualTestPlatform,
    description: &str,
    mut converged: impl FnMut() -> bool,
) -> MessageTrace {
    pump_messages_until_with_delay(
        &Rc::downgrade(&platform.inner),
        description,
        Duration::from_millis(5),
        || {
            for _ in 0..32 {
                if !visual.dispatcher().tick(false) {
                    break;
                }
            }
            converged()
        },
    )
}

fn pump_native_asset_frame_opportunities(
    platform: &WindowsPlatform,
    visual: &VisualTestPlatform,
    requests: &[Rc<Cell<usize>>; 2],
) {
    let before = requests.each_ref().map(|request| request.get());
    pump_native_assets_until(
        platform,
        visual,
        "idle native asset frame opportunities",
        || {
            requests
                .iter()
                .zip(before)
                .all(|(request, before)| request.get() >= before + 2)
        },
    );
}

struct NativeAssetSubmission {
    generation: u64,
    image: Option<ImagePaintDiagnostic>,
    capture: WindowFrameCapture,
}

fn wait_for_native_asset_submission(
    platform: &WindowsPlatform,
    app: &ApplicationHandle,
    visual: &VisualTestPlatform,
    handle: WindowHandle<NativeAssetRoot>,
    expected: Option<ImageId>,
    previous: u64,
) -> NativeAssetSubmission {
    let mut submitted = None;
    pump_native_assets_until(
        platform,
        visual,
        "owning asset consumer exact accepted nonempty Submitted frame and result pixels",
        || {
            submitted = app
                .update(|cx| {
                    handle.update(cx, |view, window, cx| {
                        let facts = window.presentation_facts();
                        let attempt = facts.latest_present_attempt?;
                        let paint = view.counters.last_paint.get()?;
                        let diagnostics = window.submitted_frame_diagnostics_for_test()?;
                        if attempt.generation <= previous
                            || attempt.outcome != PlatformWindowPresentOutcome::Submitted
                            || !attempt.contained_valid_primitives
                            || facts.initial_presentation
                                != WindowInitialPresentationStatus::Completed
                            || !facts.native_visible
                            || facts.frame_accepted_generation != Some(attempt.generation)
                            || facts.present_submitted_generation != Some(attempt.generation)
                            || facts.non_empty_presented_generation != Some(attempt.generation)
                            || window.rendered_frame_revision() != attempt.generation
                            || diagnostics.generation() != attempt.generation
                            || paint.window_id != handle.window_id()
                            || paint.previous_revision.checked_add(1) != Some(attempt.generation)
                            || paint.image != expected
                        {
                            return None;
                        }
                        let paints = diagnostics.image_paint_diagnostics();
                        let image = if let Some(expected) = expected {
                            assert_eq!(paints.len(), 1);
                            let diagnostic = paints[0];
                            assert_eq!(diagnostic.frame_generation, attempt.generation);
                            assert_eq!(diagnostic.image.image_id, expected);
                            assert_eq!(diagnostic.image.frame_index, 0);
                            assert_eq!(diagnostic.atlas_access.image, Some(diagnostic.image));
                            assert_eq!(diagnostic.atlas_access.tile, Some(diagnostic.tile));
                            assert_eq!(
                                diagnostic.atlas_access.texture_id,
                                Some(diagnostic.tile.texture_id)
                            );
                            assert_eq!(
                                diagnostic.atlas_access.size,
                                Some(size(1.into(), 1.into()))
                            );
                            assert!(matches!(
                                diagnostic.atlas_access.outcome,
                                AtlasAccessOutcome::Inserted | AtlasAccessOutcome::Hit
                            ));
                            for extent in [
                                diagnostic.bounds.size.width.0,
                                diagnostic.bounds.size.height.0,
                            ] {
                                assert!((extent - 64. * window.scale_factor()).abs() <= 1.);
                            }
                            let fetched = cx
                                .fetch_asset::<NativeAsset>(&view.interested)?
                                .expect("the submitted image must have a published asset result");
                            assert_eq!(fetched.id, expected);
                            Some(diagnostic)
                        } else {
                            assert!(paints.is_empty());
                            assert!(cx.fetch_asset::<NativeAsset>(&view.interested).is_none());
                            None
                        };
                        let capture = window.capture_frame();
                        assert_eq!(capture.metadata.window_id, handle.window_id());
                        assert_eq!(
                            capture.metadata.framework_frame_generation,
                            attempt.generation
                        );
                        assert_eq!(
                            capture.metadata.logical_viewport_size,
                            window.viewport_size()
                        );
                        assert_eq!(capture.metadata.scale_factor, window.scale_factor());
                        assert!(capture.metadata.scale_factor.is_finite());
                        assert!(capture.metadata.scale_factor > 0.);
                        assert_eq!(
                            capture.metadata.physical_viewport_size,
                            window
                                .viewport_size()
                                .to_device_pixels(window.scale_factor())
                        );
                        assert!(capture.unsupported_reason.is_none(), "{capture:?}");
                        let rgba = capture
                            .image
                            .as_ref()
                            .expect("native DX11 asset result RGBA");
                        assert_eq!(
                            rgba.dimensions(),
                            (
                                capture.metadata.physical_viewport_size.width.0 as u32,
                                capture.metadata.physical_viewport_size.height.0 as u32,
                            )
                        );
                        let (x, y) = image.map_or_else(
                            || {
                                let coordinate =
                                    (32. * capture.metadata.scale_factor).floor() as u32;
                                (coordinate, coordinate)
                            },
                            |image| {
                                (
                                    (image.bounds.origin.x.0 + image.bounds.size.width.0 / 2.)
                                        .floor() as u32,
                                    (image.bounds.origin.y.0 + image.bounds.size.height.0 / 2.)
                                        .floor() as u32,
                                )
                            },
                        );
                        let wanted = if expected.is_some() {
                            [32, 192, 96, 255]
                        } else {
                            [32, 32, 32, 255]
                        };
                        let actual = rgba.get_pixel(x, y).0;
                        assert!(
                            actual.iter().zip(wanted).all(|(a, b)| a.abs_diff(b) <= 12),
                            "owning asset generation {} RGBA: {actual:?} != {wanted:?}",
                            attempt.generation
                        );
                        Some(NativeAssetSubmission {
                            generation: attempt.generation,
                            image,
                            capture,
                        })
                    })
                })
                .expect("the owning asset consumer must remain open until actual submission");
            submitted.is_some()
        },
    );
    submitted.expect("the asset result must belong to its exact accepted Submitted frame")
}

#[test]
fn native_owning_asset_consumers_share_notify_submit_evict_and_retire() {
    discard_stale_quit_messages();
    let (_guard, observation) = begin_native_window_test_observation();
    let platform =
        Rc::new(WindowsPlatform::new_windowed_for_test().expect("native owning asset platform"));
    let platform_hwnd = platform.handle;
    let platform_lifetime = Rc::downgrade(&platform);
    let lifecycle_probe = platform.lifecycle_test_probe.clone();
    let backend_lifetime = Arc::downgrade(&platform.text_system());
    let visual = Rc::new(VisualTestPlatform::new(platform.clone(), 0));
    let visual_lifetime = Rc::downgrade(&visual);
    let (interested_sender, interested_receiver) = oneshot::channel();
    let interested = NativeAssetSource::new(1, [interested_receiver]);
    let (passive_sender, passive_receiver) = oneshot::channel();
    let (stale_sender, stale_receiver) = oneshot::channel();
    let (replacement_sender, replacement_receiver) = oneshot::channel();
    let passive =
        NativeAssetSource::new(2, [passive_receiver, stale_receiver, replacement_receiver]);
    let counters: [Rc<NativeAssetCounters>; 2] = std::array::from_fn(|_| Rc::default());
    let opened = Rc::new(Cell::new(None));
    let app = Application::with_platform(visual.clone())
        .with_quit_mode(QuitMode::Explicit)
        .run_embedded({
            let interested = interested.clone();
            let passive = passive.clone();
            let counters = counters.clone();
            let opened = opened.clone();
            move |cx| {
                assert!(cx.fetch_asset::<NativeAsset>(&interested).is_none());
                assert!(cx.fetch_asset::<NativeAsset>(&passive).is_none());
                let handles: [WindowHandle<NativeAssetRoot>; 2] = std::array::from_fn(|index| {
                    cx.open_window(
                        WindowOptions {
                            window_bounds: Some(WindowBounds::Windowed(Bounds::new(
                                point(px(80. + index as f32 * 280.), px(80.)),
                                size(px(260.), px(180.)),
                            ))),
                            focus_on_appearing: false,
                            activation_policy: WindowActivationPolicy {
                                accepts_activation: false,
                                focus_on_click: false,
                            },
                            accepts_pointer_input: false,
                            show: true,
                            ..WindowOptions::default()
                        },
                        |_, cx| {
                            cx.new(|_| NativeAssetRoot {
                                interested: interested.clone(),
                                passive: passive.clone(),
                                counters: counters[index].clone(),
                            })
                        },
                    )
                    .expect("both interested asset consumers must open real Windows windows")
                });
                opened.set(Some(handles));
            }
        });
    let handles = opened
        .take()
        .expect("the visual launch must return both consumers");
    assert_ne!(handles[0].window_id(), handles[1].window_id());
    let registrations = handles.map(|handle| {
        platform
            .raw_window_handles
            .read()
            .iter()
            .copied()
            .find(|entry| entry.window_id() == handle.window_id())
            .expect("each interested consumer must own its exact native registration")
    });
    let native_windows = registrations.map(|registration| {
        let window = platform
            .window_from_hwnd(registration.as_raw())
            .expect("each asset consumer must have a real native window owner");
        assert_eq!(window.registration, registration);
        let facts = window
            .observed_platform_facts_for_test()
            .expect("the passive measurement must observe its native input policy");
        assert!(!facts.accepts_pointer_input);
        assert!(!facts.accepts_activation);
        assert!(!facts.focus_on_click);
        window
    });
    let native_lifetimes = native_windows.each_ref().map(Rc::downgrade);
    let frame_requests = native_windows.each_ref().map(observe_frame_requests);
    let roots = handles.map(|handle| app.update(|cx| handle.root(cx).expect("live asset root")));
    let root_lifetimes = roots.each_ref().map(|root| root.downgrade());
    let window_text_lifetimes = handles.map(|handle| {
        app.update(|cx| handle.update(cx, |_, window, _| Rc::downgrade(window.text_system())))
            .expect("each asset consumer must own its window text cache")
    });
    let global_lifetime = app.update(|cx| Arc::downgrade(cx.text_system()));
    let assets_lifetime = app.update(|cx| Arc::downgrade(cx.asset_source()));
    let diagnostic_cursor = app.update(|cx| {
        cx.native_boundary_diagnostics(NativeBoundaryDiagnosticCursor::default())
            .cursor
    });
    let notifications: [Rc<Cell<usize>>; 2] = std::array::from_fn(|_| Rc::default());
    let subscriptions: [_; 2] = std::array::from_fn(|index| {
        app.update(|cx| {
            let notifications = notifications[index].clone();
            cx.observe(&roots[index], move |_, _| {
                notifications.set(notifications.get() + 1);
            })
        })
    });
    drop(roots);
    platform
        .begin_vsync_thread()
        .expect("native vsync provider should start");
    let mut pending = handles
        .map(|handle| wait_for_native_asset_submission(&platform, &app, &visual, handle, None, 0));
    for index in 0..handles.len() {
        app.update(|cx| handles[index].update(cx, |_, window, _| window.refresh()))
            .expect("settle the native initialization before passive completion measurements");
        pending[index] = wait_for_native_asset_submission(
            &platform,
            &app,
            &visual,
            handles[index],
            None,
            pending[index].generation,
        );
    }
    assert_eq!(interested.load_count(), 1);
    assert_eq!(passive.load_count(), 1);
    assert_eq!(interested.completions.load(Ordering::SeqCst), 0);
    assert_eq!(passive.completions.load(Ordering::SeqCst), 0);
    pump_native_asset_frame_opportunities(&platform, &visual, &frame_requests);
    let notifications_before = notifications.each_ref().map(|count| count.get());
    let passive_renders = counters.each_ref().map(|count| count.renders.get());
    let passive_paints = counters.each_ref().map(|count| count.paints.get());

    let passive_image = native_asset_image([192, 32, 96, 255]);
    let passive_image_lifetime = Arc::downgrade(&passive_image);
    let passive_image_id = passive_image.id;
    assert!(passive_sender.send(Ok(passive_image)).is_ok());
    pump_native_assets_until(
        &platform,
        &visual,
        "non-subscribing asset result publication",
        || {
            app.update(|cx| {
                cx.fetch_asset::<NativeAsset>(&passive)
                    .is_some_and(|result| {
                        result.expect("passive load must succeed").id == passive_image_id
                    })
            })
        },
    );
    pump_native_asset_frame_opportunities(&platform, &visual, &frame_requests);
    for (index, handle) in handles.iter().enumerate() {
        app.update(|cx| {
            handle.update(cx, |_, window, cx| {
                let result = window
                    .get_asset::<NativeAsset>(&passive, cx)
                    .expect("get_asset must see the foreground-published result")
                    .expect("the passive result must succeed");
                assert_eq!(result.id, passive_image_id);
                assert!(window.get_asset::<NativeAsset>(&interested, cx).is_none());
            })
        })
        .expect("passive queries must keep both native consumers live");
        assert_eq!(notifications[index].get(), notifications_before[index]);
        assert_eq!(counters[index].renders.get(), passive_renders[index]);
        assert_eq!(counters[index].paints.get(), passive_paints[index]);
    }
    assert_eq!(passive.load_count(), 1);
    app.update(|cx| {
        cx.remove_asset::<NativeAsset>(&passive);
        assert!(!cx.has_asset::<NativeAsset>(&passive));
        assert!(cx.fetch_asset::<NativeAsset>(&passive).is_none());
    });
    assert!(passive_image_lifetime.upgrade().is_none());
    assert_eq!(passive.load_count(), 2);
    pump_native_assets_until(
        &platform,
        &visual,
        "second passive load is held before eviction",
        || {
            !stale_sender.is_canceled()
                && app.update(|cx| cx.fetch_asset::<NativeAsset>(&passive).is_none())
        },
    );
    app.update(|cx| {
        cx.remove_asset::<NativeAsset>(&passive);
        assert!(!cx.has_asset::<NativeAsset>(&passive));
        assert!(cx.fetch_asset::<NativeAsset>(&passive).is_none());
    });
    pump_native_assets_until(
        &platform,
        &visual,
        "evicted native asset work cancels its actual receiver",
        || stale_sender.is_canceled(),
    );
    assert!(stale_sender.send(Err("evicted native result")).is_err());
    assert_eq!(passive.load_count(), 3);
    assert!(app.update(|cx| cx.fetch_asset::<NativeAsset>(&passive).is_none()));
    assert!(
        replacement_sender
            .send(Err("native replacement failure"))
            .is_ok()
    );
    pump_native_assets_until(
        &platform,
        &visual,
        "replacement caches its own result identity",
        || {
            app.update(|cx| {
                cx.fetch_asset::<NativeAsset>(&passive) == Some(Err("native replacement failure"))
            })
        },
    );
    pump_native_asset_frame_opportunities(&platform, &visual, &frame_requests);
    for (index, handle) in handles.iter().enumerate() {
        app.update(|cx| {
            handle.update(cx, |_, window, cx| {
                assert_eq!(
                    window.get_asset::<NativeAsset>(&passive, cx),
                    Some(Err("native replacement failure"))
                );
            })
        })
        .expect("the passive replacement result must remain public and readable");
        assert_eq!(notifications[index].get(), notifications_before[index]);
        assert_eq!(counters[index].renders.get(), passive_renders[index]);
        assert_eq!(counters[index].paints.get(), passive_paints[index]);
    }
    assert_eq!(passive.load_count(), 3);
    assert_eq!(passive.completions.load(Ordering::SeqCst), 2);
    let renders_before = counters.each_ref().map(|count| count.renders.get());
    let paints_before = counters.each_ref().map(|count| count.paints.get());
    let requests_before = frame_requests.each_ref().map(|count| count.get());
    let interested_image = native_asset_image([32, 192, 96, 255]);
    let interested_image_lifetime = Arc::downgrade(&interested_image);
    let interested_image_id = interested_image.id;
    assert!(interested_sender.send(Ok(interested_image)).is_ok());
    let ready: [_; 2] = std::array::from_fn(|index| {
        wait_for_native_asset_submission(
            &platform,
            &app,
            &visual,
            handles[index],
            Some(interested_image_id),
            pending[index].generation,
        )
    });
    for index in 0..2 {
        assert_eq!(notifications[index].get(), notifications_before[index] + 1);
        assert!(counters[index].renders.get() > renders_before[index]);
        assert!(counters[index].paints.get() > paints_before[index]);
        assert!(frame_requests[index].get() > requests_before[index]);
        assert_ne!(pending[index].capture.image, ready[index].capture.image);
    }
    assert_eq!(interested.load_count(), 1);
    assert_eq!(interested.completions.load(Ordering::SeqCst), 1);

    app.update(|cx| {
        let image = cx
            .fetch_asset::<NativeAsset>(&interested)
            .expect("the shared result must remain cached")
            .expect("the shared result must succeed");
        let repeated = cx
            .fetch_asset::<NativeAsset>(&interested)
            .expect("repeated fetch must return an output rather than a task")
            .expect("repeated fetch must preserve the result");
        assert!(Arc::ptr_eq(&image, &repeated));
        cx.remove_asset::<NativeAsset>(&interested);
        assert!(!cx.has_asset::<NativeAsset>(&interested));
        for (index, handle) in handles.iter().enumerate() {
            handle
                .update(cx, |_, window, _| {
                    let facts = window.presentation_facts();
                    let attempt = facts
                        .latest_present_attempt
                        .expect("a submitted result frame");
                    assert_eq!(attempt.outcome, PlatformWindowPresentOutcome::Submitted);
                    assert!(attempt.contained_valid_primitives);
                    assert!(attempt.generation >= ready[index].generation);
                    assert_eq!(facts.frame_accepted_generation, Some(attempt.generation));
                    assert_eq!(facts.present_submitted_generation, Some(attempt.generation));
                    assert_eq!(
                        facts.non_empty_presented_generation,
                        Some(attempt.generation)
                    );
                    assert_eq!(window.rendered_frame_revision(), attempt.generation);
                    let submitted = window
                        .submitted_frame_diagnostics_for_test()
                        .expect("the retained scene must have exact submitted diagnostics");
                    assert_eq!(submitted.generation(), attempt.generation);
                    assert_eq!(submitted.image_paint_diagnostics().len(), 1);
                    let paint = submitted.image_paint_diagnostics()[0];
                    assert_eq!(paint.frame_generation, attempt.generation);
                    assert_eq!(paint.image.image_id, interested_image_id);
                    assert_eq!(
                        paint.image,
                        ready[index].image.expect("a submitted image").image
                    );
                    let before_eviction = window.capture_frame();
                    assert!(
                        before_eviction.unsupported_reason.is_none(),
                        "{before_eviction:?}"
                    );
                    assert_eq!(
                        before_eviction.metadata.framework_frame_generation,
                        attempt.generation
                    );
                    assert_eq!(before_eviction.image, ready[index].capture.image);
                    window
                        .drop_image(image.clone())
                        .expect("remove the actual image atlas key");
                    let diagnostic = window
                        .atlas_remove_diagnostics()
                        .last()
                        .expect("each real atlas must record removal of the shared image key");
                    assert_eq!(diagnostic.image, Some(paint.image));
                    assert_eq!(diagnostic.texture_id, Some(paint.tile.texture_id));
                    assert_eq!(diagnostic.outcome, AtlasRemoveOutcome::TextureRetained);
                    assert_eq!(window.rendered_frame_revision(), attempt.generation);
                    let retained = window.capture_frame();
                    assert!(retained.unsupported_reason.is_none(), "{retained:?}");
                    assert_eq!(retained.metadata.window_id, handle.window_id());
                    assert_eq!(
                        retained.metadata.framework_frame_generation,
                        attempt.generation
                    );
                    assert!(
                        retained.metadata.capture_generation
                            > before_eviction.metadata.capture_generation
                    );
                    assert_eq!(retained.image, before_eviction.image);
                    let atlas = native_windows[index].state.renderer.borrow().sprite_atlas();
                    let key = AtlasKey::Image(paint.image);
                    assert!(!atlas.contains(&key));
                    let replacement_bytes = [255, 0, 255, 255];
                    let replacement = atlas
                        .get_or_insert_with_diagnostics(key.clone(), &mut || {
                            Ok(Some((
                                size(1.into(), 1.into()),
                                Cow::Borrowed(&replacement_bytes),
                            )))
                        })
                        .expect("the removed native image key must allow reconstruction");
                    assert_eq!(replacement.diagnostic.outcome, AtlasAccessOutcome::Inserted);
                    let replacement_tile = replacement.tile.expect("the rebuilt native image tile");
                    assert_eq!(
                        replacement_tile.texture_instance(),
                        paint.tile.texture_instance()
                    );
                    assert!(!replacement_tile.bounds.intersects(&paint.tile.bounds));
                    assert!(atlas.contains(&key));
                    let after_reinsertion = window.capture_frame();
                    assert!(
                        after_reinsertion.unsupported_reason.is_none(),
                        "{after_reinsertion:?}"
                    );
                    assert_eq!(
                        after_reinsertion.metadata.framework_frame_generation,
                        attempt.generation
                    );
                    assert_eq!(after_reinsertion.image, before_eviction.image);
                    let submitted_after_reinsertion = window
                        .submitted_frame_diagnostics_for_test()
                        .expect("reconstruction must preserve the exact submitted diagnostics");
                    assert_eq!(submitted_after_reinsertion.generation(), attempt.generation);
                    assert_eq!(
                        submitted_after_reinsertion.image_paint_diagnostics(),
                        &[paint]
                    );
                    window
                        .drop_image(image.clone())
                        .expect("the reconstructed native key must retire independently");
                    assert!(!atlas.contains(&key));
                    assert_eq!(
                        window
                            .atlas_remove_diagnostics()
                            .last()
                            .expect("the reconstructed key must record its exact removal")
                            .outcome,
                        AtlasRemoveOutcome::TextureRetained
                    );
                    let after_second_eviction = window.capture_frame();
                    assert!(
                        after_second_eviction.unsupported_reason.is_none(),
                        "{after_second_eviction:?}"
                    );
                    assert_eq!(after_second_eviction.image, before_eviction.image);
                    let facts_after_reinsertion = window.presentation_facts();
                    assert_eq!(
                        facts_after_reinsertion.frame_accepted_generation,
                        facts.frame_accepted_generation
                    );
                    assert_eq!(
                        facts_after_reinsertion.present_submitted_generation,
                        facts.present_submitted_generation
                    );
                    assert_eq!(
                        facts_after_reinsertion.non_empty_presented_generation,
                        facts.non_empty_presented_generation
                    );
                })
                .expect("accepted native frame leases must survive asset and atlas key eviction");
        }
        cx.remove_asset::<NativeAsset>(&passive);
        for handle in handles {
            handle
                .update(cx, |_, window, cx| window.remove_window(cx))
                .expect("both exact asset consumer windows must close");
        }
    });
    drop(subscriptions);
    pump_native_assets_until(
        &platform,
        &visual,
        "both exact owning asset consumer windows retire",
        || {
            app.update(|cx| {
                cx.windows().is_empty()
                    && handles
                        .iter()
                        .all(|handle| handle.update(cx, |_, _, _| ()).is_err())
            }) && registrations
                .iter()
                .all(|registration| !unsafe { IsWindow(Some(registration.as_raw())).as_bool() })
                && platform.raw_window_handles.read().is_empty()
                && platform.inner.native_retirement_is_settled()
                && root_lifetimes.iter().all(|root| root.upgrade().is_none())
                && window_text_lifetimes
                    .iter()
                    .all(|text| text.upgrade().is_none())
        },
    );
    for registration in registrations {
        let lifecycle = observation
            .events()
            .into_iter()
            .filter(|event| {
                event.window().window_id() == registration.window_id()
                    && event.window().native_generation() == registration.generation()
            })
            .filter_map(|event| match event.kind() {
                NativeWindowTestEventKind::WindowMessage { disposition, .. } => {
                    assert_ne!(disposition, NativeWindowTestMessageDisposition::Panicked);
                    None
                }
                lifecycle => Some(lifecycle),
            })
            .collect::<Vec<_>>();
        let [
            NativeWindowTestEventKind::PresentationQuiesced {
                ticket_generation: quiesced,
            },
            NativeWindowTestEventKind::DestroyEntered {
                ticket_generation: destroyed,
            },
            NativeWindowTestEventKind::NativeTerminal {
                ticket_generation: terminal,
            },
        ] = lifecycle.as_slice()
        else {
            panic!("unexpected owning asset consumer retirement sequence: {lifecycle:?}");
        };
        assert_eq!(quiesced, destroyed);
        assert_eq!(destroyed, terminal);
    }
    let diagnostics = app.update(|cx| cx.native_boundary_diagnostics(diagnostic_cursor));
    assert_eq!(diagnostics.omitted_before_cursor, 0);
    assert!(diagnostics.terminal.iter().all(|diagnostic| {
        !matches!(
            diagnostic.disposition,
            NativeBoundaryDisposition::InvariantFailure(_)
        )
    }));
    drop(native_windows);
    assert!(
        native_lifetimes
            .iter()
            .all(|window| window.upgrade().is_none())
    );
    assert!(interested_image_lifetime.upgrade().is_none());
    app.update(|cx| cx.shutdown());
    drop(app);
    drop(visual);
    let pump_platform = Rc::downgrade(&platform.inner);
    drop(platform);
    pump_messages_until_with_delay(
        &pump_platform,
        "owning asset platform HWND, renderer, assets and backend ownership release",
        Duration::from_millis(5),
        || {
            !unsafe { IsWindow(Some(platform_hwnd)).as_bool() }
                && platform_lifetime.upgrade().is_none()
                && visual_lifetime.upgrade().is_none()
                && global_lifetime.upgrade().is_none()
                && backend_lifetime.upgrade().is_none()
                && assets_lifetime.upgrade().is_none()
                && interested_image_lifetime.upgrade().is_none()
                && passive_image_lifetime.upgrade().is_none()
        },
    );
    assert_eq!(lifecycle_probe.platform_destroy_attempts(), 1);
    assert_eq!(lifecycle_probe.ole_uninitialize_count(), 1);
}
