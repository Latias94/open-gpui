#![cfg(feature = "test-support")]

use super::*;
use crate::native_test_observation::NativeWindowTestMessageDisposition;
use image::{Frame, Rgba as ImageRgba, RgbaImage};
use open_gpui::{
    AnyView, ColorSpace, Corners, Entity, ImageId, Pixels, Point, PrepaintPublicationId,
    RenderImage, Rgba, StrikethroughStyle, StyleRefinement, SubtreeClip, SubtreeClipExt,
    SubtreeTransform, SubtreeTransformExt, UnderlineStyle, WindowFrameCapture, WindowHandle,
    WindowInitialPresentationStatus, black, deferred, fill, linear_color_stop, linear_gradient,
};

const PIXEL_CHANNEL_MARGIN: u8 = 6;

#[derive(Clone, Copy)]
struct NativeGradientCase {
    name: &'static str,
    from: [f64; 4],
    to: [f64; 4],
    color_space: ColorSpace,
}

const GRADIENT_CASES: [NativeGradientCase; 6] = [
    NativeGradientCase {
        name: "Oklab black/white",
        from: [0., 0., 0., 1.],
        to: [1., 1., 1., 1.],
        color_space: ColorSpace::Oklab,
    },
    NativeGradientCase {
        name: "Oklab gray 0.125/0.875",
        from: [0.125, 0.125, 0.125, 1.],
        to: [0.875, 0.875, 0.875, 1.],
        color_space: ColorSpace::Oklab,
    },
    NativeGradientCase {
        name: "Oklab chromatic",
        from: [0.8, 0.2, 0.15, 1.],
        to: [0.12, 0.5, 0.82, 1.],
        color_space: ColorSpace::Oklab,
    },
    NativeGradientCase {
        name: "sRGB black/white control",
        from: [0., 0., 0., 1.],
        to: [1., 1., 1., 1.],
        color_space: ColorSpace::Srgb,
    },
    NativeGradientCase {
        name: "sRGB chromatic control",
        from: [0.8, 0.2, 0.15, 1.],
        to: [0.12, 0.5, 0.82, 1.],
        color_space: ColorSpace::Srgb,
    },
    NativeGradientCase {
        name: "sRGB alpha control over black",
        from: [0.15, 0.55, 0.85, 0.25],
        to: [0.85, 0.25, 0.1, 0.75],
        color_space: ColorSpace::Srgb,
    },
];

const SOLID_CONTROLS: [[f64; 4]; 4] = [
    [0.125, 0.125, 0.125, 1.],
    [0.5, 0.5, 0.5, 1.],
    [0.2, 0.6, 0.9, 1.],
    [0.8, 0.2, 0.4, 0.5],
];

fn scene_rgba([r, g, b, a]: [f64; 4]) -> Rgba {
    Rgba {
        r: r as f32,
        g: g as f32,
        b: b as f32,
        a: a as f32,
    }
}

fn scene_freshness_color(changed: bool) -> [f64; 4] {
    if changed {
        [0.85, 0.25, 0.15, 1.]
    } else {
        [0.15, 0.55, 0.85, 1.]
    }
}

const SCENE_IMAGE_RGB: [[u8; 3]; 2] = [[208, 96, 32], [64, 48, 160]];
const DECORATION_COLOR: [f64; 4] = [0.2, 0.75, 0.9, 1.];

#[derive(Default)]
struct NativeSceneConsumerCounters {
    renders: Cell<usize>,
    uncached_renders: Cell<usize>,
    accepted_draw_cache_mode: Cell<Option<(u64, bool)>>,
    image_paints: Cell<usize>,
    opacity_paints: Cell<usize>,
    clip_paints: Cell<usize>,
    image_errors: RefCell<Vec<String>>,
}

impl NativeSceneConsumerCounters {
    fn cached_state(&self) -> (usize, usize, usize, usize) {
        (
            self.renders.get(),
            self.image_paints.get(),
            self.opacity_paints.get(),
            self.clip_paints.get(),
        )
    }
}

fn paint_scene_image_row(
    window: &mut Window,
    image: &Arc<RenderImage>,
    origin: Point<Pixels>,
    y: f32,
    counters: &NativeSceneConsumerCounters,
) {
    for (index, grayscale) in [false, true, false, true].into_iter().enumerate() {
        if let Err(error) = window.paint_image(
            Bounds::new(
                origin + point(px(336. + 72. * index as f32), px(y)),
                size(px(56.), px(24.)),
            ),
            Corners::default(),
            image.clone(),
            0,
            grayscale,
        ) {
            counters
                .image_errors
                .borrow_mut()
                .push(format!("image row {y}, instance {index}: {error:#}"));
        }
    }
}

fn paint_scene_decorations(window: &mut Window, origin: Point<Pixels>, x: f32) {
    for (y, wavy) in [(112., false), (136., true)] {
        window.paint_underline(
            origin + point(px(x), px(y)),
            px(128.),
            &UnderlineStyle {
                thickness: px(3.),
                color: Some(scene_rgba(DECORATION_COLOR).into()),
                wavy,
            },
        );
    }
    window.paint_strikethrough(
        origin + point(px(x), px(160.)),
        px(128.),
        &StrikethroughStyle {
            thickness: px(3.),
            color: Some(scene_rgba(DECORATION_COLOR).into()),
        },
    );
}

struct NativeSceneConsumers {
    image: Arc<RenderImage>,
    counters: Rc<NativeSceneConsumerCounters>,
}

impl Render for NativeSceneConsumers {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.counters.renders.set(self.counters.renders.get() + 1);
        if window.is_accessibility_active() || window.is_inspector_picking(cx) {
            self.counters
                .uncached_renders
                .set(self.counters.uncached_renders.get() + 1);
        }
        let image = self.image.clone();
        let counters = self.counters.clone();
        let opaque = canvas(
            |_, _, _| {},
            move |bounds, _, window, _| {
                paint_scene_image_row(window, &image, bounds.origin, 16., &counters);
                paint_scene_decorations(window, bounds.origin, 336.);
                counters.image_paints.set(counters.image_paints.get() + 1);
            },
        )
        .absolute()
        .top_0()
        .left_0()
        .size_full();
        let image = self.image.clone();
        let counters = self.counters.clone();
        let translucent = div()
            .absolute()
            .top_0()
            .left_0()
            .size_full()
            .opacity(0.5)
            .child(deferred(
                canvas(
                    |_, _, _| {},
                    move |bounds, _, window, _| {
                        paint_scene_image_row(window, &image, bounds.origin, 56., &counters);
                        paint_scene_decorations(window, bounds.origin, 488.);
                        counters
                            .opacity_paints
                            .set(counters.opacity_paints.get() + 1);
                    },
                )
                .size_full(),
            ));
        let image = self.image.clone();
        let counters = self.counters.clone();
        let clipped = canvas(
            |_, _, _| {},
            move |bounds, _, window, _| {
                if let Err(error) = window.paint_image(bounds, Corners::default(), image, 0, false)
                {
                    counters
                        .image_errors
                        .borrow_mut()
                        .push(format!("translated rounded-clip image: {error:#}"));
                }
                counters.clip_paints.set(counters.clip_paints.get() + 1);
            },
        )
        .absolute()
        .left(px(336.))
        .top(px(204.))
        .w(px(128.))
        .h(px(48.))
        .with_subtree_clip(
            SubtreeClip::try_own_rounded_border_box(Corners {
                top_left: size(px(16.), px(16.)),
                top_right: size(px(16.), px(16.)),
                bottom_left: size(px(16.), px(16.)),
                bottom_right: size(px(16.), px(16.)),
            })
            .expect("the fixture's rounded image clip must be valid"),
        )
        .with_subtree_transform(
            SubtreeTransform::try_translation(point(px(24.), px(8.)))
                .expect("the fixture's image translation must be valid"),
        );
        div()
            .relative()
            .size_full()
            .child(opaque)
            .child(translucent)
            .child(clipped)
    }
}

struct NativeSceneColorRoot {
    changed: bool,
    paints: Rc<Cell<usize>>,
    consumers: Entity<NativeSceneConsumers>,
    counters: Rc<NativeSceneConsumerCounters>,
    cache_mode_publication: PrepaintPublicationId,
}

impl Render for NativeSceneColorRoot {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let changed = self.changed;
        let paints = self.paints.clone();
        let counters = self.counters.clone();
        let cache_mode_publication = self.cache_mode_publication;
        let consumers = AnyView::from(self.consumers.clone())
            .cached(StyleRefinement::default().w(px(640.)).h(px(288.)));
        div()
            .relative()
            .size_full()
            .bg(black())
            .child(
                canvas(
                    move |_, window, cx| {
                        let cache_allowed =
                            !window.is_accessibility_active() && !window.is_inspector_picking(cx);
                        let accepted = counters.clone();
                        let discarded = counters.clone();
                        window.record_prepaint_window_transaction(
                            cache_mode_publication,
                            move |fence, _, _| {
                                accepted
                                    .accepted_draw_cache_mode
                                    .set(Some((fence.generation(), cache_allowed)));
                            },
                            move |_, _, _| discarded.accepted_draw_cache_mode.set(None),
                        );
                    },
                    move |bounds, _, window, _| {
                        for (index, case) in GRADIENT_CASES.iter().enumerate() {
                            let rect = Bounds::new(
                                bounds.origin + point(px(16.), px(16. + 28. * index as f32)),
                                size(px(288.), px(20.)),
                            );
                            let gradient = linear_gradient(
                                90.,
                                linear_color_stop(scene_rgba(case.from), 0.),
                                linear_color_stop(scene_rgba(case.to), 1.),
                            )
                            .color_space(case.color_space);
                            window.paint_quad(fill(rect, gradient));
                        }
                        for (index, color) in SOLID_CONTROLS.iter().enumerate() {
                            window.paint_quad(fill(
                                Bounds::new(
                                    bounds.origin + point(px(16. + 72. * index as f32), px(192.)),
                                    size(px(64.), px(24.)),
                                ),
                                scene_rgba(*color),
                            ));
                        }
                        window.paint_quad(fill(
                            Bounds::new(
                                bounds.origin + point(px(16.), px(236.)),
                                size(px(288.), px(20.)),
                            ),
                            scene_rgba(scene_freshness_color(changed)),
                        ));
                        paints.set(paints.get() + 1);
                    },
                )
                .size_full(),
            )
            .child(
                div()
                    .absolute()
                    .top_0()
                    .left_0()
                    .w(px(640.))
                    .h(px(288.))
                    .child(deferred(consumers)),
            )
    }
}

fn capture_native_scene_submission<V: Render + 'static>(
    app: &mut Application,
    handle: WindowHandle<V>,
    previous_generation: u64,
    expected_image_id: ImageId,
    counters: &NativeSceneConsumerCounters,
) -> (WindowFrameCapture, bool) {
    let mut submitted = None;
    pump_messages_until_with_delay(
        "native scene accepted nonempty Submitted frame and RGBA capture",
        Duration::from_millis(5),
        || {
            submitted = app
                .update_for_test(|cx| {
                    handle.update(cx, |_, window, _| {
                        let facts = window.presentation_facts();
                        let attempt = facts.latest_present_attempt?;
                        if attempt.generation <= previous_generation
                            || attempt.outcome != PlatformWindowPresentOutcome::Submitted
                            || !attempt.contained_valid_primitives
                            || facts.initial_presentation
                                != WindowInitialPresentationStatus::Completed
                            || !facts.native_visible
                            || facts.frame_accepted_generation != Some(attempt.generation)
                            || facts.present_submitted_generation != Some(attempt.generation)
                            || facts.non_empty_presented_generation != Some(attempt.generation)
                            || window.rendered_frame_revision() != attempt.generation
                            || window
                                .submitted_frame_diagnostics_for_test()
                                .map(|diagnostics| diagnostics.generation())
                                != Some(attempt.generation)
                        {
                            return None;
                        }
                        let diagnostics = window
                            .submitted_frame_diagnostics_for_test()
                            .expect("the native scene must retain its submitted diagnostics");
                        assert_eq!(diagnostics.image_paint_diagnostics().len(), 9);
                        for diagnostic in diagnostics.image_paint_diagnostics() {
                            assert_eq!(diagnostic.frame_generation, attempt.generation);
                            assert_eq!(diagnostic.image.image_id, expected_image_id);
                            assert_eq!(diagnostic.image.frame_index, 0);
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
                        let image = capture
                            .image
                            .as_ref()
                            .expect("the real DirectX scene must support RGBA readback");
                        assert_eq!(
                            image.dimensions(),
                            (
                                capture.metadata.physical_viewport_size.width.0 as u32,
                                capture.metadata.physical_viewport_size.height.0 as u32,
                            )
                        );
                        let (generation, cache_allowed) = counters
                            .accepted_draw_cache_mode
                            .get()
                            .expect("the root must publish its admitted draw-time cache mode");
                        assert_eq!(generation, attempt.generation);
                        Some((capture, cache_allowed))
                    })
                })
                .expect("the native scene window must remain live until capture");
            submitted.is_some()
        },
    );
    submitted.expect("capture must belong to the exact accepted and submitted scene generation")
}

/// Independent f64 oracle for Windows' approximate gamma-2.2 Oklab contract.
fn scene_oklab([r, g, b]: [f64; 3]) -> [f64; 3] {
    let [r, g, b] = [r, g, b].map(|channel| channel.powf(2.2));
    let l = (0.4122214708 * r + 0.5363325363 * g + 0.0514459929 * b).cbrt();
    let m = (0.2119034982 * r + 0.6806995451 * g + 0.1073969566 * b).cbrt();
    let s = (0.0883024619 * r + 0.2817188376 * g + 0.6299787005 * b).cbrt();
    [
        0.2104542553 * l + 0.7936177850 * m - 0.0040720468 * s,
        1.9779984951 * l - 2.4285922050 * m + 0.4505937099 * s,
        0.0259040371 * l + 0.7827717662 * m - 0.8086757660 * s,
    ]
}

fn scene_srgb([lightness, a, b]: [f64; 3]) -> [f64; 3] {
    let l = (lightness + 0.3963377774 * a + 0.2158037573 * b).powi(3);
    let m = (lightness - 0.1055613458 * a - 0.0638541728 * b).powi(3);
    let s = (lightness - 0.0894841775 * a - 1.2914855480 * b).powi(3);
    [
        4.0767416621 * l - 3.3077115913 * m + 0.2309699292 * s,
        -1.2684380046 * l + 2.6097574011 * m - 0.3413193965 * s,
        -0.0041960863 * l - 0.7034186147 * m + 1.7076147010 * s,
    ]
    .map(|channel| channel.max(0.).powf(1. / 2.2))
}

fn scene_gradient_rgb(case: NativeGradientCase, t: f64) -> [f64; 3] {
    let mut from = [case.from[0], case.from[1], case.from[2]];
    let mut to = [case.to[0], case.to[1], case.to[2]];
    if case.color_space == ColorSpace::Oklab {
        from = scene_oklab(from);
        to = scene_oklab(to);
    }
    let mut color =
        std::array::from_fn(|channel| from[channel] + (to[channel] - from[channel]) * t);
    if case.color_space == ColorSpace::Oklab {
        color = scene_srgb(color);
    }
    let alpha = case.from[3] + (case.to[3] - case.from[3]) * t;
    color.map(|channel| channel * alpha)
}

fn assert_scene_pixel(image: &RgbaImage, x: u32, y: u32, expected: [f64; 3], label: &str) {
    let expected = expected.map(|channel| (channel.clamp(0., 1.) * 255.).round() as u8);
    let actual = image.get_pixel(x, y).0;
    for channel in 0..3 {
        assert!(
            actual[channel].abs_diff(expected[channel]) <= PIXEL_CHANNEL_MARGIN,
            "{label} at physical pixel ({x}, {y}), channel {channel}: actual RGBA {actual:?}, expected RGB {expected:?}, margin {PIXEL_CHANNEL_MARGIN}"
        );
    }
    assert!(
        actual[3].abs_diff(255) <= PIXEL_CHANNEL_MARGIN,
        "{label} at physical pixel ({x}, {y}) must remain opaque over black: {actual:?}"
    );
}

fn assert_native_gradient_capture(capture: &WindowFrameCapture, changed: bool) {
    let image = capture.image.as_ref().expect("captured native RGBA pixels");
    let scale = f64::from(capture.metadata.scale_factor);
    assert!(capture.metadata.logical_viewport_size.width >= px(304.));
    assert!(capture.metadata.logical_viewport_size.height >= px(256.));
    let left = (16. * scale - 0.5).ceil();
    let right = (304. * scale - 0.5).ceil();
    for (index, case) in GRADIENT_CASES.iter().copied().enumerate() {
        let y = ((26. + 28. * index as f64) * scale).floor() as u32;
        for fraction in [0.5, 0.25, 0.75, 0.125, 0.875] {
            let x = ((16. + 288. * fraction) * scale).floor() as u32;
            let t = (f64::from(x) + 0.5 - left) / (right - left);
            assert!(t > 0. && t < 1.);
            assert_scene_pixel(image, x, y, scene_gradient_rgb(case, t), case.name);
        }
    }
    for (index, color) in SOLID_CONTROLS.iter().enumerate() {
        let x = ((48. + 72. * index as f64) * scale).floor() as u32;
        let y = (204. * scale).floor() as u32;
        assert_scene_pixel(
            image,
            x,
            y,
            [
                color[0] * color[3],
                color[1] * color[3],
                color[2] * color[3],
            ],
            "solid/alpha control over black",
        );
    }
    let color = scene_freshness_color(changed);
    assert_scene_pixel(
        image,
        (160. * scale).floor() as u32,
        (246. * scale).floor() as u32,
        [color[0], color[1], color[2]],
        "notified current-frame color witness",
    );
}

fn assert_native_scene_consumers(capture: &WindowFrameCapture) {
    let image = capture
        .image
        .as_ref()
        .expect("captured native consumer pixels");
    let scale = f64::from(capture.metadata.scale_factor);
    assert!(capture.metadata.logical_viewport_size.width >= px(624.));
    for (row, opacity) in [(16., 1.), (56., 0.5)] {
        for (instance, grayscale) in [false, true, false, true].into_iter().enumerate() {
            for (side, fraction) in [0.25, 0.75].into_iter().enumerate() {
                let mut expected = SCENE_IMAGE_RGB[side].map(|channel| f64::from(channel) / 255.);
                if grayscale {
                    let gray = expected[0] * 0.2126 + expected[1] * 0.7152 + expected[2] * 0.0722;
                    expected = [gray; 3];
                }
                assert_scene_pixel(
                    image,
                    ((336. + 72. * instance as f64 + 56. * fraction) * scale).floor() as u32,
                    ((row + 12.) * scale).floor() as u32,
                    expected.map(|channel| channel * opacity),
                    &format!(
                        "sprite instance {instance}, grayscale {grayscale}, opacity {opacity}, side {side}"
                    ),
                );
            }
        }
    }
    for (origin_x, opacity) in [(336., 1.), (488., 0.5)] {
        let expected = [
            DECORATION_COLOR[0],
            DECORATION_COLOR[1],
            DECORATION_COLOR[2],
        ]
        .map(|channel| channel * opacity);
        for (origin_y, name) in [(112., "straight underline"), (160., "strikethrough")] {
            for offset in [8., 32., 64., 96., 120.] {
                let x = ((origin_x + offset) * scale).floor() as u32;
                assert_scene_pixel(
                    image,
                    x,
                    ((origin_y + 1.5) * scale).floor() as u32,
                    expected,
                    &format!("{name}, opacity {opacity}"),
                );
                assert_scene_pixel(
                    image,
                    x,
                    ((origin_y + 6.) * scale).floor() as u32,
                    [0.; 3],
                    &format!("outside {name}, opacity {opacity}"),
                );
            }
        }
        let top = (136. * scale - 0.5).ceil() as u32;
        let height = (3. * capture.metadata.scale_factor - 0.5).ceil().max(1.) as u32 * 3;
        for phase in 0..8 {
            let x = ((origin_x + 6.75 + 13.5 * f64::from(phase)) * scale).floor() as u32;
            let mut weight = 0.;
            let mut moment = 0.;
            let mut peak = (top, 0);
            for offset in 0..height {
                let y = top + offset;
                let green = image.get_pixel(x, y).0[1];
                weight += f64::from(green);
                moment += (f64::from(offset) + 0.5) * f64::from(green);
                if green > peak.1 {
                    peak = (y, green);
                }
            }
            assert!(weight > 0., "wavy underline phase {phase} must contain ink");
            let centroid = moment / (weight * f64::from(height));
            assert!(
                if phase % 2 == 0 {
                    centroid > 0.6
                } else {
                    centroid < 0.4
                },
                "wavy underline phase {phase}, opacity {opacity}: vertical centroid {centroid} must alternate between the lower and upper bands"
            );
            let ink_fraction = weight / (expected[1] * 255. * f64::from(height));
            assert!(
                (0.2..0.6).contains(&ink_fraction),
                "wavy underline phase {phase}, opacity {opacity}: ink fraction {ink_fraction} must be a wave stroke"
            );
            assert_scene_pixel(
                image,
                x,
                peak.0,
                expected,
                &format!("wavy underline peak {phase}, opacity {opacity}"),
            );
        }
    }
    for (side, x) in [392., 456.].into_iter().enumerate() {
        assert_scene_pixel(
            image,
            (x * scale).floor() as u32,
            (236. * scale).floor() as u32,
            SCENE_IMAGE_RGB[side].map(|channel| f64::from(channel) / 255.),
            "translated rounded-clip image interior",
        );
    }
    for (x, y) in [
        (361., 213.),
        (487., 213.),
        (361., 259.),
        (487., 259.),
        (340., 208.),
        (358., 236.),
        (490., 236.),
    ] {
        assert_scene_pixel(
            image,
            (x * scale).floor() as u32,
            (y * scale).floor() as u32,
            [0.; 3],
            "rounded corners, translated-away source and outside clip",
        );
    }
}

#[test]
fn native_scene_oklab_pixels_match_independent_gamma_oracle_and_notified_frame() {
    discard_stale_quit_messages();
    let (_observation_guard, observation) = begin_native_window_test_observation();
    let observation_cursor = observation.cursor();
    let platform = Rc::new(
        WindowsPlatform::new(false)
            .expect("the real DirectWrite/DirectX Windows platform must initialize"),
    );
    let platform_hwnd = platform.handle;
    let lifecycle_probe = platform.lifecycle_test_probe.clone();
    let platform_text_system = platform.text_system();
    let weak_platform_text_system = Arc::downgrade(&platform_text_system);
    drop(platform_text_system);
    let mut app = Application::with_platform(platform.clone()).with_quit_mode(QuitMode::Explicit);
    let text_system = app.text_system();
    let weak_text_system = Arc::downgrade(&text_system);
    drop(text_system);
    platform.begin_vsync_thread();
    let diagnostic_cursor = app.update_for_test(|cx| {
        cx.native_boundary_diagnostics(NativeBoundaryDiagnosticCursor::default())
            .cursor
    });
    let paints = Rc::new(Cell::new(0));
    let consumer_counters = Rc::new(NativeSceneConsumerCounters::default());
    let image = Arc::new(RenderImage::new(smallvec::smallvec![Frame::new(
        RgbaImage::from_fn(8, 8, |x, _| {
            let [r, g, b] = SCENE_IMAGE_RGB[usize::from(x >= 4)];
            ImageRgba([b, g, r, 255])
        }),
    )]));
    let handle = app
        .update_for_test(|cx| {
            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::centered(size(px(680.), px(320.)), cx)),
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
                    let consumers = cx.new(|_| NativeSceneConsumers {
                        image: image.clone(),
                        counters: consumer_counters.clone(),
                    });
                    cx.new(|_| NativeSceneColorRoot {
                        changed: false,
                        paints: paints.clone(),
                        consumers,
                        counters: consumer_counters.clone(),
                        cache_mode_publication: PrepaintPublicationId::new(),
                    })
                },
            )
        })
        .expect("the native scene color window must open");
    let registration = platform
        .raw_window_handles
        .read()
        .iter()
        .copied()
        .find(|registration| registration.window_id() == handle.window_id())
        .expect("the scene must own its exact native HWND registration");
    let hwnd = registration.as_raw();
    let native_window = platform
        .window_from_hwnd(hwnd)
        .expect("the native scene HWND must remain registered");
    assert_eq!(native_window.registration, registration);
    let native_facts = native_window
        .observed_platform_facts_for_test()
        .expect("the scene's stable native creation facts must be observable");
    assert!(!native_facts.accepts_pointer_input);
    assert!(!native_facts.accepts_activation);
    assert!(!native_facts.focus_on_click);
    let frame_requests = observe_frame_requests(&native_window);
    let initial_paint_cursor = observation.cursor();
    assert!(unsafe { RedrawWindow(Some(hwnd), None, None, RDW_INVALIDATE).as_bool() });
    pump_messages_until_with_delay(
        "native scene initialization WM_PAINT",
        Duration::from_millis(5),
        || {
            observation.any_after(initial_paint_cursor, |event| {
                event.window().window_id() == registration.window_id()
                    && event.window().native_generation() == registration.generation()
                    && matches!(
                        event.kind(),
                        NativeWindowTestEventKind::WindowMessage {
                            message: NativeWindowTestMessage::Other(WM_PAINT),
                            disposition: NativeWindowTestMessageDisposition::Returned(0),
                            ..
                        }
                    )
            })
        },
    );
    pump_messages_until_idle("native scene initialization follow-up");
    let previous_generation = app
        .update_for_test(|cx| {
            handle.update(cx, |_, window, _| {
                let generation = window.rendered_frame_revision();
                window.refresh();
                generation
            })
        })
        .expect("the fully initialized native scene must refresh");
    let (initial, initial_cache_allowed) = capture_native_scene_submission(
        &mut app,
        handle,
        previous_generation,
        image.id,
        &consumer_counters,
    );
    assert!(paints.get() > 0);
    let paints_before = paints.get();
    let requests_before = frame_requests.get();
    let initial_consumer_state = consumer_counters.cached_state();
    let initial_uncached_renders = consumer_counters.uncached_renders.get();
    assert!(initial_consumer_state.0 > 0);
    assert!(initial_consumer_state.1 > 0);
    assert!(initial_consumer_state.2 > 0);
    assert!(initial_consumer_state.3 > 0);
    app.update_for_test(|cx| handle.update(cx, |_, _, cx| cx.notify()))
        .expect("notifying the root must replay its cached deferred consumers");
    let (replay, replay_cache_allowed) = capture_native_scene_submission(
        &mut app,
        handle,
        initial.metadata.framework_frame_generation,
        image.id,
        &consumer_counters,
    );
    let replay_consumer_state = consumer_counters.cached_state();
    let replay_uncached_renders = consumer_counters.uncached_renders.get();
    assert!(paints.get() > paints_before);
    assert!(frame_requests.get() > requests_before);
    assert!(replay.metadata.capture_generation > initial.metadata.capture_generation);
    let replay_paints = paints.get();
    let replay_requests = frame_requests.get();
    app.update_for_test(|cx| {
        handle.update(cx, |root, _, cx| {
            root.changed = true;
            cx.notify();
        })
    })
    .expect("the existing native scene must notify its changed color");
    let (changed, changed_cache_allowed) = capture_native_scene_submission(
        &mut app,
        handle,
        replay.metadata.framework_frame_generation,
        image.id,
        &consumer_counters,
    );
    let changed_consumer_state = consumer_counters.cached_state();
    let changed_uncached_renders = consumer_counters.uncached_renders.get();
    assert!(paints.get() > paints_before);
    assert!(frame_requests.get() > requests_before);
    assert!(paints.get() > replay_paints);
    assert!(frame_requests.get() > replay_requests);
    assert!(changed.metadata.capture_generation > initial.metadata.capture_generation);
    assert!(changed.metadata.capture_generation > replay.metadata.capture_generation);
    assert_eq!(changed.metadata.scale_factor, initial.metadata.scale_factor);
    assert_eq!(
        changed.metadata.physical_viewport_size,
        initial.metadata.physical_viewport_size
    );

    assert!(platform.raw_window_handles.read().contains(&registration));
    app.update_for_test(|cx| handle.update(cx, |_, window, cx| window.remove_window(cx)))
        .expect("the exact native scene window must close");
    pump_messages_until_with_delay(
        "exact native scene HWND retirement",
        Duration::from_millis(5),
        || {
            !unsafe { IsWindow(Some(hwnd)).as_bool() }
                && !is_registered(&platform, hwnd)
                && platform.inner.native_retirement_is_settled()
        },
    );
    let lifecycle = observation
        .events_after(observation_cursor)
        .into_iter()
        .filter(|event| {
            event.window().window_id() == registration.window_id()
                && event.window().native_generation() == registration.generation()
        })
        .filter_map(|event| match event.kind() {
            NativeWindowTestEventKind::WindowMessage { disposition, .. } => {
                assert_ne!(
                    disposition,
                    NativeWindowTestMessageDisposition::Panicked,
                    "the native scene boundary must not swallow a panic"
                );
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
        panic!("unexpected native scene retirement sequence: {lifecycle:?}");
    };
    assert_eq!(quiesced, destroyed);
    assert_eq!(destroyed, terminal);
    let diagnostics = app.update_for_test(|cx| cx.native_boundary_diagnostics(diagnostic_cursor));
    assert_eq!(diagnostics.omitted_before_cursor, 0);
    assert!(diagnostics.terminal.iter().all(|diagnostic| {
        !matches!(
            diagnostic.disposition,
            NativeBoundaryDisposition::InvariantFailure(_)
        )
    }));
    assert!(app.update_for_test(|cx| {
        assert!(handle.update(cx, |_, _, _| ()).is_err());
        cx.windows().is_empty()
    }));
    assert!(platform.raw_window_handles.read().is_empty());
    drop(native_window);
    app.update_for_test(|cx| cx.shutdown());
    drop(app);
    drop(platform);
    pump_messages_until_with_delay(
        "native scene platform message HWND retirement",
        Duration::from_millis(5),
        || unsafe { !IsWindow(Some(platform_hwnd)).as_bool() },
    );
    assert_eq!(lifecycle_probe.platform_destroy_attempts(), 1);
    assert_eq!(lifecycle_probe.ole_uninitialize_count(), 1);
    assert!(weak_text_system.upgrade().is_none());
    assert!(weak_platform_text_system.upgrade().is_none());

    assert_native_gradient_capture(&initial, false);
    assert_native_gradient_capture(&changed, true);
    assert!(
        initial.image != changed.image,
        "notified pixels must be current"
    );
    assert!(
        consumer_counters.image_errors.borrow().is_empty(),
        "native public image paint errors: {:?}",
        consumer_counters.image_errors.borrow()
    );
    for (label, before, after, reuse_allowed) in [
        (
            "unchanged root notification",
            initial_consumer_state,
            replay_consumer_state,
            initial_cache_allowed
                && replay_cache_allowed
                && initial_uncached_renders == replay_uncached_renders,
        ),
        (
            "changed root color",
            replay_consumer_state,
            changed_consumer_state,
            replay_cache_allowed
                && changed_cache_allowed
                && replay_uncached_renders == changed_uncached_renders,
        ),
    ] {
        eprintln!(
            "native scene {label}: scale {}, embedded FXC {}, cached replay {reuse_allowed}, consumer counts {before:?} -> {after:?}",
            initial.metadata.scale_factor,
            !cfg!(debug_assertions)
        );
        if reuse_allowed {
            assert_eq!(
                after, before,
                "{label}: cached deferred replay must reuse child render and all three consumer canvas paints"
            );
        } else {
            assert!(
                after.0 > before.0
                    && after.1 > before.1
                    && after.2 > before.2
                    && after.3 > before.3,
                "{label}: accessibility or inspector cache bypass must rebuild the child and all three consumer canvases: {before:?} -> {after:?}"
            );
        }
    }
    assert!(
        initial.image == replay.image,
        "cached deferred replay must preserve every captured RGBA pixel"
    );
    assert_native_gradient_capture(&replay, false);
    for capture in [&initial, &replay, &changed] {
        assert_native_scene_consumers(capture);
    }
}
