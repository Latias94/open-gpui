#![cfg(feature = "test-support")]

//! Exercises encoded and embedded images through a real Windows Application and Img.
//! Qualifies animation, EXIF correction, nonempty Submitted frames, atlas reuse,
//! offscreen framework pixels, notification, reconstruction, and exact owner release.
//! Framework capture does not establish OS compositor presentation.

use super::*;
use crate::native_test_observation::{
    NativeWindowTestMessageDisposition, NativeWindowTestObservation,
};
use open_gpui::{
    AssetSource, AtlasAccessOutcome, Image, ImageFormat, ImagePaintDiagnostic, ImageSource,
    ImgResourceLoader, InteractiveElement, RenderImage, Resource, SharedString, WindowFrameCapture,
    WindowHandle, WindowInitialPresentationStatus, black, img,
};
use std::borrow::Cow;

const ANIMATED_WEBP: &[u8] = include_bytes!("../../../tests/fixtures/images/red-blue-100ms.webp");
const EXIF_JPEG: &[u8] =
    include_bytes!("../../../tests/fixtures/images/exif-orientation-rotate-180.jpg");
const ANIMATED_RESOURCE: &str = "native-image/red-blue-100ms.webp";
const EXIF_RESOURCE: &str = "native-image/exif-orientation-rotate-180.jpg";

struct NativeImageAssets;

impl AssetSource for NativeImageAssets {
    fn load(&self, path: &str) -> anyhow::Result<Option<Cow<'static, [u8]>>> {
        Ok(match path {
            ANIMATED_RESOURCE => Some(Cow::Borrowed(ANIMATED_WEBP)),
            EXIF_RESOURCE => Some(Cow::Borrowed(EXIF_JPEG)),
            _ => None,
        })
    }

    fn list(&self, path: &str) -> anyhow::Result<Vec<SharedString>> {
        Ok([ANIMATED_RESOURCE, EXIF_RESOURCE]
            .into_iter()
            .filter(|resource| resource.starts_with(path))
            .map(SharedString::from)
            .collect())
    }
}

#[derive(Clone, Copy, Debug)]
struct NativeImagePaint {
    window_id: WindowId,
    version: usize,
    previous_revision: u64,
    active: bool,
}

#[derive(Default)]
struct NativeImageCounters {
    renders: Cell<usize>,
    paints: Cell<usize>,
    last_paint: Cell<Option<NativeImagePaint>>,
}

struct NativeImageRoot {
    encoded: Arc<Image>,
    embedded: Resource,
    version: usize,
    counters: Rc<NativeImageCounters>,
}

impl Render for NativeImageRoot {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        self.counters.renders.set(self.counters.renders.get() + 1);
        let counters = self.counters.clone();
        let version = self.version;
        div()
            .relative()
            .size_full()
            .bg(black())
            .child(
                img(self.encoded.clone())
                    .id("encoded-image")
                    .absolute()
                    .left(px(16.))
                    .top(px(16.))
                    .w(px(64.)),
            )
            .child(
                img(ImageSource::Resource(self.embedded.clone()))
                    .id("embedded-image")
                    .absolute()
                    .left(px(112.))
                    .top(px(16.))
                    .w(px(64.)),
            )
            .child(
                canvas(
                    |_, _, _| {},
                    move |_, (), window, _| {
                        counters.paints.set(counters.paints.get() + 1);
                        counters.last_paint.set(Some(NativeImagePaint {
                            window_id: window.window_handle().window_id(),
                            version,
                            previous_revision: window.rendered_frame_revision(),
                            active: window.is_window_active(),
                        }));
                    },
                )
                .absolute()
                .top_0()
                .left_0()
                .size_full(),
            )
    }
}

fn open_image_window(
    app: &mut Application,
    counters: &Rc<NativeImageCounters>,
    reconstructed: bool,
) -> WindowHandle<NativeImageRoot> {
    app.update_for_test(|cx| {
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::centered(size(px(240.), px(200.)), cx)),
                focus_on_appearing: false,
                show: true,
                ..WindowOptions::default()
            },
            |_, cx| {
                cx.new(|_| NativeImageRoot {
                    encoded: Arc::new(Image::from_bytes(
                        if reconstructed {
                            ImageFormat::Jpeg
                        } else {
                            ImageFormat::Webp
                        },
                        if reconstructed {
                            EXIF_JPEG
                        } else {
                            ANIMATED_WEBP
                        }
                        .to_vec(),
                    )),
                    embedded: Resource::Embedded(
                        if reconstructed {
                            EXIF_RESOURCE
                        } else {
                            ANIMATED_RESOURCE
                        }
                        .into(),
                    ),
                    version: usize::from(reconstructed),
                    counters: counters.clone(),
                })
            },
        )
    })
    .expect("the native image consumer window must open")
}

fn activate_image_window(
    app: &mut Application,
    handle: WindowHandle<NativeImageRoot>,
    registration: RegisteredWindow,
) {
    assert_eq!(registration.window_id(), handle.window_id());
    pump_messages_until_with_delay(
        "native image consumer initial visibility",
        Duration::from_millis(5),
        || {
            app.update_for_test(|cx| {
                handle.update(cx, |_, window, _| {
                    let facts = window.presentation_facts();
                    facts.initial_presentation == WindowInitialPresentationStatus::Completed
                        && facts.native_visible
                })
            })
            .expect("the image consumer must stay open until visible")
        },
    );
    crate::native_test_foreground::acquire_foreground_window(registration.as_raw())
        .expect("the real image consumer HWND must acquire foreground activation");
    pump_messages_until_with_delay(
        "native image consumer actual active status",
        Duration::from_millis(5),
        || {
            let native_active = unsafe {
                GetActiveWindow() == registration.as_raw()
                    && GetForegroundWindow() == registration.as_raw()
            };
            native_active
                && app
                    .update_for_test(|cx| {
                        handle.update(cx, |_, window, _| window.is_window_active())
                    })
                    .expect("the active image consumer must remain open")
        },
    );
}

fn assert_native_decoded_image(image: &RenderImage, version: usize) {
    if version == 0 {
        assert_eq!(image.frame_count(), 2);
        for frame in 0..2 {
            assert_eq!(image.size(frame), size(1.into(), 1.into()));
            assert_eq!(
                Duration::from(image.delay(frame)),
                Duration::from_millis(100)
            );
            let bgra = image.as_bytes(frame).expect("decoded WebP BGRA frame");
            assert_eq!(bgra.len(), 4);
            assert_eq!(bgra[3], 255);
            let (red, blue) = (bgra[2], bgra[0]);
            assert!(
                bgra[1] < 10,
                "lossy WebP must keep green near zero: {bgra:?}"
            );
            if frame == 0 {
                assert!(red > 240 && blue < 10, "expected red BGRA: {bgra:?}");
            } else {
                assert!(blue > 240 && red < 10, "expected blue BGRA: {bgra:?}");
            }
        }
    } else {
        assert_eq!(image.frame_count(), 1);
        assert_eq!(image.size(0), size(16.into(), 32.into()));
        let bgra = image.as_bytes(0).expect("EXIF-corrected JPEG BGRA");
        assert_eq!(bgra.len(), 16 * 32 * 4);
        assert_eq!(&bgra[..4], &[255, 255, 255, 255]);
        assert_eq!(&bgra[bgra.len() - 4..], &[0, 0, 0, 255]);
        assert!(bgra.chunks_exact(4).all(|pixel| pixel[3] == 255));
    }
}

struct NativeImageSubmission {
    generation: u64,
    images: [ImagePaintDiagnostic; 2],
    decoded: [std::sync::Weak<RenderImage>; 2],
    capture: WindowFrameCapture,
}

fn wait_for_image_submission(
    app: &mut Application,
    handle: WindowHandle<NativeImageRoot>,
    registration: RegisteredWindow,
    version: usize,
    previous: u64,
    mut wanted: impl FnMut(&[ImagePaintDiagnostic; 2]) -> bool,
) -> NativeImageSubmission {
    let mut submitted = None;
    pump_messages_until_with_delay(
        "native image consumer accepted nonempty Submitted frame and RGBA capture",
        Duration::from_millis(5),
        || {
            submitted = app
                .update_for_test(|cx| {
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
                            || view.version != version
                            || paint.window_id != handle.window_id()
                            || paint.version != version
                            || paint.previous_revision.checked_add(1) != Some(attempt.generation)
                            || !paint.active
                            || !window.is_window_active()
                            || unsafe { GetActiveWindow() } != registration.as_raw()
                            || unsafe { GetForegroundWindow() } != registration.as_raw()
                            || diagnostics.image_paint_diagnostics().len() != 2
                        {
                            return None;
                        }
                        let paints = diagnostics.image_paint_diagnostics().to_vec();
                        let encoded = view.encoded.clone().get_render_image(window, cx)?;
                        let embedded = window
                            .get_asset::<ImgResourceLoader>(&view.embedded, cx)?
                            .expect("the embedded Img resource must decode successfully");
                        assert_native_decoded_image(&encoded, version);
                        assert_native_decoded_image(&embedded, version);
                        assert_ne!(encoded.id, embedded.id);
                        let images = [encoded.as_ref(), embedded.as_ref()].map(|image| {
                            let matching = paints
                                .iter()
                                .filter(|paint| paint.image.image_id == image.id)
                                .copied()
                                .collect::<Vec<_>>();
                            assert_eq!(matching.len(), 1, "each actual Img must paint once");
                            let diagnostic = matching[0];
                            assert_eq!(diagnostic.frame_generation, attempt.generation);
                            assert!(diagnostic.image.frame_index < image.frame_count());
                            assert_eq!(diagnostic.atlas_access.image, Some(diagnostic.image));
                            assert_eq!(diagnostic.atlas_access.tile, Some(diagnostic.tile));
                            assert_eq!(
                                diagnostic.atlas_access.texture_id,
                                Some(diagnostic.tile.texture_id)
                            );
                            assert_eq!(
                                diagnostic.atlas_access.size,
                                Some(image.size(diagnostic.image.frame_index))
                            );
                            assert!(matches!(
                                diagnostic.atlas_access.outcome,
                                AtlasAccessOutcome::Inserted | AtlasAccessOutcome::Hit
                            ));
                            assert!(diagnostic.bounds.size.width.0 > 0.);
                            assert!(diagnostic.bounds.size.height.0 > 0.);
                            let expected_height = if version == 0 { 64. } else { 128. };
                            assert!(
                                (diagnostic.bounds.size.width.0 - 64. * window.scale_factor())
                                    .abs()
                                    <= 1.
                            );
                            assert!(
                                (diagnostic.bounds.size.height.0
                                    - expected_height * window.scale_factor())
                                .abs()
                                    <= 1.
                            );
                            diagnostic
                        });
                        if !wanted(&images) {
                            return None;
                        }
                        let capture = window.capture_frame();
                        assert_eq!(capture.metadata.window_id, handle.window_id());
                        assert_eq!(
                            capture.metadata.framework_frame_generation,
                            attempt.generation
                        );
                        assert_eq!(capture.metadata.scale_factor, window.scale_factor());
                        assert!(capture.metadata.scale_factor.is_finite());
                        assert!(capture.metadata.scale_factor > 0.);
                        assert_eq!(
                            capture.metadata.logical_viewport_size,
                            window.viewport_size()
                        );
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
                            .expect("real DirectX image scene pixels");
                        assert_eq!(
                            rgba.dimensions(),
                            (
                                capture.metadata.physical_viewport_size.width.0 as u32,
                                capture.metadata.physical_viewport_size.height.0 as u32,
                            )
                        );
                        Some(NativeImageSubmission {
                            generation: attempt.generation,
                            images,
                            decoded: [Arc::downgrade(&encoded), Arc::downgrade(&embedded)],
                            capture,
                        })
                    })
                })
                .expect("the native image window must remain live until submission");
            submitted.is_some()
        },
    );
    submitted.expect("the actual Img paints and capture must belong to the exact Submitted frame")
}

fn assert_native_image_pixels(submission: &NativeImageSubmission, animated: bool) {
    let rgba = submission
        .capture
        .image
        .as_ref()
        .expect("captured image scene RGBA");
    for diagnostic in &submission.images {
        let sample = |x: f32, y: f32| {
            let x = (diagnostic.bounds.origin.x.0 + diagnostic.bounds.size.width.0 * x).floor();
            let y = (diagnostic.bounds.origin.y.0 + diagnostic.bounds.size.height.0 * y).floor();
            assert!(x >= 0. && x < rgba.width() as f32);
            assert!(y >= 0. && y < rgba.height() as f32);
            rgba.get_pixel(x as u32, y as u32).0
        };
        let assert_pixel = |actual: [u8; 4], expected: [u8; 4]| {
            assert!(
                actual
                    .iter()
                    .zip(expected)
                    .all(|(actual, expected)| actual.abs_diff(expected) <= 12),
                "submitted generation {}, image {:?}: actual RGBA {actual:?}, expected {expected:?}",
                submission.generation,
                diagnostic.image
            );
        };
        if animated {
            let expected = if diagnostic.image.frame_index == 0 {
                [255, 0, 0, 255]
            } else {
                [0, 0, 255, 255]
            };
            assert_pixel(sample(0.5, 0.5), expected);
        } else {
            assert_eq!(diagnostic.image.frame_index, 0);
            assert_pixel(sample(0.5 / 16., 0.5 / 32.), [255, 255, 255, 255]);
            assert_pixel(sample(15.5 / 16., 31.5 / 32.), [0, 0, 0, 255]);
        }
    }
}

fn close_image_window(
    app: &mut Application,
    platform: &WindowsPlatform,
    handle: WindowHandle<NativeImageRoot>,
    registration: RegisteredWindow,
    observation: &NativeWindowTestObservation,
) {
    assert_eq!(registration.window_id(), handle.window_id());
    assert!(platform.raw_window_handles.read().contains(&registration));
    app.update_for_test(|cx| handle.update(cx, |_, window, cx| window.remove_window(cx)))
        .expect("the exact image consumer window must close");
    pump_messages_until_with_delay(
        "exact image consumer native HWND retirement",
        Duration::from_millis(5),
        || {
            !unsafe { IsWindow(Some(registration.as_raw())).as_bool() }
                && !platform.raw_window_handles.read().contains(&registration)
                && platform.inner.native_retirement_is_settled()
        },
    );
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
        panic!("unexpected image consumer retirement sequence: {lifecycle:?}");
    };
    assert_eq!(quiesced, destroyed);
    assert_eq!(destroyed, terminal);
    assert!(app.update_for_test(|cx| {
        assert!(handle.update(cx, |_, _, _| ()).is_err());
        cx.windows().is_empty()
    }));
    assert!(platform.raw_window_handles.read().is_empty());
}

#[test]
fn native_encoded_and_embedded_images_animate_replace_and_release_exact_owners() {
    discard_stale_quit_messages();
    let (_guard, observation) = begin_native_window_test_observation();
    let platform = Rc::new(WindowsPlatform::new(false).expect("native image platform"));
    let platform_hwnd = platform.handle;
    let platform_lifetime = Rc::downgrade(&platform);
    let lifecycle_probe = platform.lifecycle_test_probe.clone();
    let backend_lifetime = Arc::downgrade(&platform.text_system());
    let mut app = Application::with_platform(platform.clone())
        .with_assets(NativeImageAssets)
        .with_quit_mode(QuitMode::Explicit);
    let global_lifetime = Arc::downgrade(&app.text_system());
    let asset_lifetime = app.update_for_test(|cx| Arc::downgrade(cx.asset_source()));
    let diagnostic_cursor = app.update_for_test(|cx| {
        cx.native_boundary_diagnostics(NativeBoundaryDiagnosticCursor::default())
            .cursor
    });
    platform.begin_vsync_thread();
    let counters = Rc::new(NativeImageCounters::default());
    let handle = open_image_window(&mut app, &counters, false);
    let registration = platform
        .raw_window_handles
        .read()
        .iter()
        .copied()
        .find(|entry| entry.window_id() == handle.window_id())
        .expect("exact initial image consumer HWND registration");
    let native_window = platform
        .window_from_hwnd(registration.as_raw())
        .expect("the initial image consumer HWND must be registered");
    assert_eq!(native_window.registration, registration);
    let frame_requests = observe_frame_requests(&native_window);
    let window_text = app
        .update_for_test(|cx| handle.update(cx, |_, window, _| Rc::downgrade(window.text_system())))
        .expect("initial image consumer window text owner");
    let root = app
        .update_for_test(|cx| handle.root(cx))
        .expect("initial image consumer root");
    let root_lifetime = root.downgrade();
    activate_image_window(&mut app, handle, registration);
    let requests_before_animation = frame_requests.get();
    let mut cycles: [Vec<ImagePaintDiagnostic>; 2] = std::array::from_fn(|_| Vec::new());
    let mut generation = 0;
    let mut animated_lifetimes = None;
    while cycles.iter().any(|cycle| cycle.len() < 3) {
        let submitted =
            wait_for_image_submission(&mut app, handle, registration, 0, generation, |images| {
                images.iter().zip(&cycles).any(|(image, cycle)| {
                    cycle.len() < 3 && image.image.frame_index == [0, 1, 0][cycle.len()]
                })
            });
        assert_native_image_pixels(&submitted, true);
        for (diagnostic, cycle) in submitted.images.iter().zip(&mut cycles) {
            if cycle.len() < 3 && diagnostic.image.frame_index == [0, 1, 0][cycle.len()] {
                cycle.push(*diagnostic);
            }
        }
        generation = submitted.generation;
        animated_lifetimes = Some(submitted.decoded);
    }
    let animated_lifetimes =
        animated_lifetimes.expect("both actual animated loaders must have been observed");
    for cycle in &cycles {
        assert_eq!(
            cycle
                .iter()
                .map(|paint| paint.image.frame_index)
                .collect::<Vec<_>>(),
            [0, 1, 0]
        );
        assert!(
            cycle
                .windows(2)
                .all(|pair| pair[0].frame_generation < pair[1].frame_generation)
        );
        assert!(
            cycle
                .iter()
                .all(|paint| paint.image.image_id == cycle[0].image.image_id)
        );
        assert_eq!(cycle[2].atlas_access.outcome, AtlasAccessOutcome::Hit);
        assert_eq!(cycle[0].tile, cycle[2].tile);
        assert_ne!(cycle[0].tile, cycle[1].tile);
    }
    assert!(frame_requests.get() > requests_before_animation);

    let notifications = Rc::new(Cell::new(0));
    let subscription = app.update_for_test(|cx| {
        let notifications = notifications.clone();
        cx.observe(&root, move |_, _| {
            notifications.set(notifications.get() + 1)
        })
    });
    let requests_before_replacement = frame_requests.get();
    let paints_before_replacement = counters.paints.get();
    app.update_for_test(|cx| {
        handle.update(cx, |view, _, cx| {
            view.encoded = Arc::new(Image::from_bytes(ImageFormat::Jpeg, EXIF_JPEG.to_vec()));
            view.embedded = Resource::Embedded(EXIF_RESOURCE.into());
            view.version = 1;
            cx.notify();
        })
    })
    .expect("replace the existing stable Img sources and notify their root");
    assert_eq!(
        notifications.get(),
        1,
        "replacement notification must be observed before pumping"
    );
    let replaced =
        wait_for_image_submission(&mut app, handle, registration, 1, generation, |_| true);
    assert_native_image_pixels(&replaced, false);
    assert!(counters.paints.get() > paints_before_replacement);
    assert!(frame_requests.get() > requests_before_replacement);
    for (index, diagnostic) in replaced.images.iter().enumerate() {
        assert_ne!(diagnostic.image.image_id, cycles[index][0].image.image_id);
        assert_eq!(diagnostic.image.frame_index, 0);
    }

    let notifications_before_repaint = notifications.get();
    let renders_before_repaint = counters.renders.get();
    let paints_before_repaint = counters.paints.get();
    let requests_before_repaint = frame_requests.get();
    app.update_for_test(|cx| handle.update(cx, |_, _, cx| cx.notify()))
        .expect("an unchanged image consumer notification must repaint through native vsync");
    assert_eq!(notifications.get(), notifications_before_repaint + 1);
    let repainted = wait_for_image_submission(
        &mut app,
        handle,
        registration,
        1,
        replaced.generation,
        |_| true,
    );
    assert!(counters.renders.get() > renders_before_repaint);
    assert!(counters.paints.get() > paints_before_repaint);
    assert!(frame_requests.get() > requests_before_repaint);
    assert_native_image_pixels(&repainted, false);
    for (initial, repeated) in replaced.images.iter().zip(&repainted.images) {
        assert_eq!(initial.image, repeated.image);
        assert_eq!(initial.tile, repeated.tile);
        assert_eq!(repeated.atlas_access.outcome, AtlasAccessOutcome::Hit);
    }
    assert_eq!(replaced.capture.image, repainted.capture.image);
    assert!(
        repainted.capture.metadata.capture_generation
            > replaced.capture.metadata.capture_generation
    );
    drop(subscription);
    drop(root);
    close_image_window(&mut app, &platform, handle, registration, &observation);
    drop(native_window);
    assert!(root_lifetime.upgrade().is_none());
    assert!(window_text.upgrade().is_none());

    let rebuilt_counters = Rc::new(NativeImageCounters::default());
    let rebuilt = open_image_window(&mut app, &rebuilt_counters, true);
    assert_ne!(rebuilt.window_id(), handle.window_id());
    let rebuilt_registration = platform
        .raw_window_handles
        .read()
        .iter()
        .copied()
        .find(|entry| entry.window_id() == rebuilt.window_id())
        .expect("exact reconstructed image consumer HWND registration");
    assert_ne!(rebuilt_registration.generation(), registration.generation());
    let rebuilt_native = platform
        .window_from_hwnd(rebuilt_registration.as_raw())
        .expect("the reconstructed image consumer HWND must be registered");
    assert_eq!(rebuilt_native.registration, rebuilt_registration);
    let rebuilt_text = app
        .update_for_test(|cx| {
            rebuilt.update(cx, |_, window, _| Rc::downgrade(window.text_system()))
        })
        .expect("reconstructed image consumer window text owner");
    let rebuilt_root = app
        .update_for_test(|cx| rebuilt.root(cx))
        .expect("reconstructed image root");
    let rebuilt_root_lifetime = rebuilt_root.downgrade();
    activate_image_window(&mut app, rebuilt, rebuilt_registration);
    let reconstructed =
        wait_for_image_submission(&mut app, rebuilt, rebuilt_registration, 1, 0, |_| true);
    assert_native_image_pixels(&reconstructed, false);
    assert!(rebuilt_counters.renders.get() > 0);
    assert!(rebuilt_counters.paints.get() > 0);
    for (previous, reconstructed) in repainted.images.iter().zip(&reconstructed.images) {
        assert_eq!(previous.image, reconstructed.image);
    }
    assert_eq!(repainted.capture.image, reconstructed.capture.image);
    drop(rebuilt_root);
    close_image_window(
        &mut app,
        &platform,
        rebuilt,
        rebuilt_registration,
        &observation,
    );
    drop(rebuilt_native);
    assert!(rebuilt_root_lifetime.upgrade().is_none());
    assert!(rebuilt_text.upgrade().is_none());
    let diagnostics = app.update_for_test(|cx| cx.native_boundary_diagnostics(diagnostic_cursor));
    assert_eq!(diagnostics.omitted_before_cursor, 0);
    assert!(diagnostics.terminal.iter().all(|diagnostic| {
        !matches!(
            diagnostic.disposition,
            NativeBoundaryDisposition::InvariantFailure(_)
        )
    }));

    app.update_for_test(|cx| cx.shutdown());
    drop(app);
    drop(platform);
    pump_messages_until_with_delay(
        "image consumer platform message HWND retirement and owner release",
        Duration::from_millis(5),
        || {
            !unsafe { IsWindow(Some(platform_hwnd)).as_bool() }
                && global_lifetime.upgrade().is_none()
                && backend_lifetime.upgrade().is_none()
                && asset_lifetime.upgrade().is_none()
                && animated_lifetimes
                    .iter()
                    .chain(&replaced.decoded)
                    .chain(&repainted.decoded)
                    .chain(&reconstructed.decoded)
                    .all(|decoded| decoded.upgrade().is_none())
        },
    );
    assert!(platform_lifetime.upgrade().is_none());
    assert_eq!(lifecycle_probe.platform_destroy_attempts(), 1);
    assert_eq!(lifecycle_probe.ole_uninitialize_count(), 1);
    for decoded in animated_lifetimes
        .iter()
        .chain(&replaced.decoded)
        .chain(&repainted.decoded)
        .chain(&reconstructed.decoded)
    {
        assert!(decoded.upgrade().is_none());
    }
}
