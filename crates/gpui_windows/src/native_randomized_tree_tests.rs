//! The public seeded tree consumed by a Windows Application and DirectX renderer.
//!
//! Cached color notification and ADR 0022 root hide/restore require exact accepted, nonempty
//! Submitted generations and GPU pixels derived from the actual Scene. Synthetic button messages
//! travel through WndProc; they do not qualify physical input, compositor scanout or steady CPU.

use super::*;
use crate::native_test_observation::{
    NativeWindowTestMessageDisposition, NativeWindowTestObservation,
};
use open_gpui::{
    Entity, InteractiveElement, Pixels, Point, PrepaintPublicationId, Primitive, Rgba,
    ScaledPixels, StatefulInteractiveElement, StyleRefinement, SubtreePresentation,
    SubtreePresentationExt, WindowFrameCapture, WindowHandle, WindowInitialPresentationStatus,
    black, fill,
    randomized_element_tree::{
        RandomizedElementTree, RandomizedElementTreeConfig,
        RandomizedElementTreeMutation as Mutation,
        RandomizedElementTreeMutationKind as MutationKind, RandomizedElementTreeTopology,
    },
    rgb,
};

#[derive(Default)]
struct NativeRandomizedTreeCounters {
    root_renders: Cell<usize>,
    cached_renders: Cell<usize>,
    cached_paints: Cell<usize>,
    accepted_cache_mode: Cell<Option<(u64, bool)>>,
}

impl NativeRandomizedTreeCounters {
    fn cached_state(&self) -> (usize, usize) {
        (self.cached_renders.get(), self.cached_paints.get())
    }
}

struct NativeRandomizedTreeCache {
    tree: Entity<RandomizedElementTree>,
    counters: Rc<NativeRandomizedTreeCounters>,
}

impl Render for NativeRandomizedTreeCache {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        self.counters
            .cached_renders
            .set(self.counters.cached_renders.get() + 1);
        let counters = self.counters.clone();
        div().relative().size_full().child(self.tree.clone()).child(
            canvas(
                |_, _, _| (),
                move |_, (), _, _| {
                    counters.cached_paints.set(counters.cached_paints.get() + 1);
                },
            )
            .absolute()
            .size_full(),
        )
    }
}

struct NativeRandomizedTreeRoot {
    tree: Entity<RandomizedElementTree>,
    cached_tree: Entity<NativeRandomizedTreeCache>,
    counters: Rc<NativeRandomizedTreeCounters>,
    cache_mode_publication: PrepaintPublicationId,
    /// Projects the public model's RootVisibility mutations onto its sibling button.
    button_presentation: SubtreePresentation,
    presses: Rc<Cell<usize>>,
    clicks: Rc<Cell<usize>>,
}

impl Render for NativeRandomizedTreeRoot {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.counters
            .root_renders
            .set(self.counters.root_renders.get() + 1);
        let counters = self.counters.clone();
        let cache_mode_publication = self.cache_mode_publication;
        div()
            .relative()
            .size_full()
            .child(
                canvas(
                    move |_, window, cx| {
                        let cache_allowed =
                            !window.is_a11y_active() && !window.is_inspector_picking(cx);
                        let accepted = counters.clone();
                        let discarded = counters.clone();
                        window.record_prepaint_window_transaction(
                            cache_mode_publication,
                            move |fence, _, _| {
                                accepted
                                    .accepted_cache_mode
                                    .set(Some((fence.generation(), cache_allowed)));
                            },
                            move |_, _, _| discarded.accepted_cache_mode.set(None),
                        );
                    },
                    |bounds, (), window, _| window.paint_quad(fill(bounds, black())),
                )
                .absolute()
                .size_full(),
            )
            .child(
                div()
                    .absolute()
                    .left(px(16.))
                    .top(px(16.))
                    .w(px(640.))
                    .h(px(520.))
                    .child(
                        div().w(px(640.)).h(px(440.)).overflow_hidden().child(
                            self.cached_tree
                                .clone()
                                .cached(StyleRefinement::default().w(px(640.)).h(px(440.))),
                        ),
                    )
                    .child(
                        div()
                            .id("native-randomized-tree-close")
                            .absolute()
                            .left_0()
                            .top(px(464.))
                            .w(px(128.))
                            .h(px(40.))
                            .bg(rgb(0x3080d0))
                            .on_mouse_down(
                                open_gpui::MouseButton::Left,
                                cx.listener(|root, _, _, _| {
                                    root.presses.set(root.presses.get() + 1);
                                }),
                            )
                            .on_click(cx.listener(|root, _, window, cx| {
                                root.clicks.set(root.clicks.get() + 1);
                                window.remove_window(cx);
                            }))
                            .with_subtree_presentation(self.button_presentation),
                    ),
            )
    }
}

fn mutate_native_tree(
    app: &mut Application,
    handle: WindowHandle<NativeRandomizedTreeRoot>,
    kind: MutationKind,
) -> (Mutation, u64) {
    app.update_for_test(|cx| {
        handle.update(cx, |root, window, cx| {
            let previous = window.rendered_frame_revision();
            let mutation = root
                .tree
                .update(cx, |tree, cx| tree.apply_mutation(kind, cx));
            if mutation == Mutation::RootVisibility {
                root.button_presentation = match root.button_presentation {
                    SubtreePresentation::Visible => SubtreePresentation::Hidden,
                    SubtreePresentation::Hidden => SubtreePresentation::Visible,
                    SubtreePresentation::Inert => unreachable!("this projection only hides"),
                };
                cx.notify();
            }
            (mutation, previous)
        })
    })
    .expect("mutate the ordinary public tree through its live consumer")
}

fn assert_tree_pixel(capture: &WindowFrameCapture, position: Point<Pixels>, color: Rgba) {
    let image = capture
        .image
        .as_ref()
        .expect("native randomized-tree GPU readback");
    let scale = capture.metadata.scale_factor;
    let x = (f32::from(position.x) * scale).floor() as u32;
    let y = (f32::from(position.y) * scale).floor() as u32;
    assert!(x < image.width() && y < image.height());
    let expected = [color.r * color.a, color.g * color.a, color.b * color.a, 1.]
        .map(|channel| (channel * 255.).round() as u8);
    let actual = image.get_pixel(x, y).0;
    assert!(
        actual.iter().zip(expected).all(|(a, b)| a.abs_diff(b) <= 2),
        "tree generation {} at {position:?}: {actual:?} != {expected:?}",
        capture.metadata.framework_frame_generation,
    );
}

/// Samples an unobscured root interior using the accepted Scene's solid color over black.
fn assert_tree_scene_pixels(
    window: &Window,
    capture: &WindowFrameCapture,
    visible: bool,
) -> Option<Rgba> {
    let scene = window.rendered_scene();
    let scale = window.scale_factor();
    let tree_origin = point(ScaledPixels(16. * scale), ScaledPixels(16. * scale));
    let root = scene.quads.iter().find(|quad| {
        quad.bounds.origin == tree_origin
            && quad.bounds.size.width.0 >= 520. * scale
            && quad.bounds.size.height.0 >= 360. * scale
    });
    assert_eq!(
        root.is_some(),
        visible,
        "actual Scene root paint membership"
    );
    assert_tree_pixel(capture, point(px(680.), px(40.)), black().into());
    let button_color = if visible {
        rgb(0x3080d0)
    } else {
        black().into()
    };
    assert_tree_pixel(capture, point(px(80.), px(500.)), button_color);
    let Some(root) = root else {
        assert_tree_pixel(capture, point(px(64.), px(64.)), black().into());
        return None;
    };
    let visual_bounds = Primitive::Quad(*root)
        .try_visual_bounds()
        .expect("the native tree root has finite raster geometry");
    for (raster, logical) in [
        (visual_bounds.left(), root.bounds.left()),
        (visual_bounds.top(), root.bounds.top()),
        (visual_bounds.right(), root.bounds.right()),
        (visual_bounds.bottom(), root.bounds.bottom()),
    ] {
        assert!(
            (raster.0 - logical.0).abs() <= 1.,
            "native pixel snapping must fit the sampling margin"
        );
    }
    let color: Rgba = root
        .background
        .as_solid()
        .expect("tree root has a solid background")
        .into();
    let mut inset = [
        root.corner_radii.top_left.0,
        root.corner_radii.top_right.0,
        root.corner_radii.bottom_left.0,
        root.corner_radii.bottom_right.0,
    ]
    .into_iter()
    .fold(0.0, f32::max);
    let mut occupied = Vec::new();
    for quad in scene.quads.iter().filter(|quad| quad.order > root.order) {
        if quad.bounds == root.bounds
            && quad
                .background
                .as_solid()
                .is_some_and(|color| color.is_transparent())
        {
            inset = [
                quad.border_widths.top.0,
                quad.border_widths.right.0,
                quad.border_widths.bottom.0,
                quad.border_widths.left.0,
            ]
            .into_iter()
            .fold(inset, f32::max);
        } else {
            occupied.push(quad.bounds);
        }
    }
    occupied.extend(scene.shadows.iter().map(|shadow| {
        Primitive::Shadow(*shadow)
            .try_visual_bounds()
            .expect("actual shadow raster bounds")
    }));
    occupied.extend(scene.paths.iter().map(|path| {
        Primitive::Path(path.clone())
            .try_visual_bounds()
            .expect("actual path raster bounds")
    }));
    occupied.extend(scene.underlines.iter().map(|primitive| primitive.bounds));
    occupied.extend(
        scene
            .monochrome_sprites
            .iter()
            .map(|primitive| primitive.bounds),
    );
    occupied.extend(
        scene
            .subpixel_sprites
            .iter()
            .map(|primitive| primitive.bounds),
    );
    occupied.extend(
        scene
            .polychrome_sprites
            .iter()
            .map(|primitive| primitive.bounds),
    );
    occupied.extend(scene.surfaces.iter().map(|primitive| primitive.bounds));
    inset += 2. * scale;
    let mut sample = None;
    let mut y = root.bounds.top().0 + inset;
    while y < root.bounds.bottom().0 - inset {
        let mut x = root.bounds.left().0 + inset;
        while x < root.bounds.right().0 - inset {
            let candidate = point(ScaledPixels(x), ScaledPixels(y));
            if !occupied
                .iter()
                .any(|bounds| bounds.dilate(ScaledPixels(scale)).contains(&candidate))
            {
                sample = Some(point(px(x / scale), px(y / scale)));
                break;
            }
            x += 4. * scale;
        }
        if sample.is_some() {
            break;
        }
        y += 2. * scale;
    }
    assert_tree_pixel(
        capture,
        sample.expect("the native tree fixture must leave an unobscured root background"),
        color,
    );
    Some(color)
}

fn submitted_tree_frame(
    platform: &WindowsPlatform,
    app: &mut Application,
    handle: WindowHandle<NativeRandomizedTreeRoot>,
    previous: u64,
    visible: bool,
) -> (u64, Option<Rgba>) {
    let mut submitted = None;
    let mut last_state = None;
    pump_messages_until_with_delay(
        &Rc::downgrade(&platform.inner),
        "native tree exact Submitted pixels",
        Duration::from_millis(5),
        || {
            submitted = app
                .update_for_test(|cx| {
                    handle.update(cx, |root, window, _| {
                        let facts = window.presentation_facts();
                        last_state = Some((facts, window.rendered_frame_revision()));
                        let attempt = facts.latest_present_attempt?;
                        if !(attempt.generation > previous
                            && attempt.outcome == PlatformWindowPresentOutcome::Submitted
                            && attempt.contained_valid_primitives
                            && facts.initial_presentation
                                == WindowInitialPresentationStatus::Completed
                            && facts.native_visible
                            && facts.frame_accepted_generation == Some(attempt.generation)
                            && facts.present_submitted_generation == Some(attempt.generation)
                            && facts.non_empty_presented_generation == Some(attempt.generation)
                            && window.rendered_frame_revision() == attempt.generation
                            && window.submitted_frame_diagnostics_for_test()?.generation()
                                == attempt.generation)
                        {
                            return None;
                        }
                        let (cache_generation, cache_allowed) = root
                            .counters
                            .accepted_cache_mode
                            .get()
                            .expect("root must publish its accepted draw-time cache mode");
                        assert_eq!(cache_generation, attempt.generation);
                        assert!(
                            cache_allowed,
                            "native tree cache qualification requires caching; accessibility or \
                             inspector bypass is not cache proof"
                        );
                        assert_eq!(
                            root.button_presentation == SubtreePresentation::Visible,
                            visible
                        );
                        let capture = window.capture_frame();
                        assert!(capture.unsupported_reason.is_none(), "{capture:?}");
                        assert_eq!(capture.metadata.window_id, handle.window_id());
                        assert_eq!(
                            capture.metadata.framework_frame_generation,
                            attempt.generation
                        );
                        assert_eq!(capture.metadata.scale_factor, window.scale_factor());
                        assert!(
                            capture.metadata.scale_factor.is_finite()
                                && capture.metadata.scale_factor > 0.
                        );
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
                        let image = capture.image.as_ref().expect("Submitted tree RGBA pixels");
                        assert_eq!(
                            image.dimensions(),
                            (
                                capture.metadata.physical_viewport_size.width.0 as u32,
                                capture.metadata.physical_viewport_size.height.0 as u32,
                            )
                        );
                        let color = assert_tree_scene_pixels(window, &capture, visible);
                        assert_eq!(window.rendered_frame_revision(), attempt.generation);
                        assert_eq!(
                            window.presentation_facts().latest_present_attempt,
                            facts.latest_present_attempt
                        );
                        Some((attempt.generation, color))
                    })
                })
                .expect("the exact tree consumer remains live until capture");
            submitted.is_some()
        },
    );
    submitted
        .unwrap_or_else(|| panic!("tree frame did not reach Submitted GPU pixels: {last_state:?}"))
}

fn synthetic_tree_button(
    platform: &WindowsPlatform,
    registration: RegisteredWindow,
    observation: &NativeWindowTestObservation,
    scale: f32,
    message: u32,
) {
    let hwnd = registration.as_raw();
    let x = (80. * scale).round() as u16;
    let y = (500. * scale).round() as u16;
    let expected = if message == WM_LBUTTONDOWN {
        NativeWindowTestMessage::PrimaryButtonDown
    } else {
        assert_eq!(message, WM_LBUTTONUP);
        NativeWindowTestMessage::PrimaryButtonUp
    };
    let cursor = observation.cursor();
    let buttons = if message == WM_LBUTTONDOWN {
        MK_LBUTTON.0 as usize
    } else {
        0
    };
    post_message(hwnd, message, WPARAM(buttons), mouse_position_lparam(x, y));
    let delivered = || {
        observation
            .events_after(cursor)
            .into_iter()
            .filter(|event| {
                event.window().window_id() == registration.window_id()
                    && event.window().native_generation() == registration.generation()
                    && matches!(event.kind(), NativeWindowTestEventKind::WindowMessage {
                    message, ..
                } if message == expected)
            })
            .collect::<Vec<_>>()
    };
    let trace = pump_messages_until_with_delay(
        &Rc::downgrade(&platform.inner),
        "synthetic tree button WndProc delivery",
        Duration::from_millis(5),
        || !delivered().is_empty(),
    );
    assert!(trace.contains(hwnd, message));
    let events = delivered();
    assert_eq!(events.len(), 1);
    let NativeWindowTestEventKind::WindowMessage {
        client_point,
        disposition,
        ..
    } = events[0].kind()
    else {
        unreachable!("filtered WndProc message");
    };
    assert_ne!(disposition, NativeWindowTestMessageDisposition::Panicked);
    let point = client_point.expect("synthetic button client coordinates");
    assert_eq!((point.x, point.y), (i32::from(x), i32::from(y)));
}

#[test]
#[ignore = "requires an interactive Windows desktop and a DirectX GPU"]
fn native_randomized_tree_cached_color_hide_restore_click_and_retire() {
    discard_stale_quit_messages();
    let (_observation_guard, observation) = begin_native_window_test_observation();
    let platform = Rc::new(WindowsPlatform::new_windowed_for_test().expect("native tree backend"));
    let platform_hwnd = platform.handle;
    let platform_lifetime = Rc::downgrade(&platform);
    let lifecycle_probe = platform.lifecycle_test_probe.clone();
    let mut app = Application::with_platform(platform.clone()).with_quit_mode(QuitMode::Explicit);
    let mut weak_app = app.update_for_test(|cx| cx.to_async());
    platform
        .begin_vsync_thread()
        .expect("native vsync provider should start");
    let presses = Rc::new(Cell::new(0));
    let clicks = Rc::new(Cell::new(0));
    let counters = Rc::new(NativeRandomizedTreeCounters::default());
    let handle = app
        .update_for_test(|cx| {
            assert!(
                !cx.is_test() && !cx.background_executor().is_test() && !cx.to_async().is_test()
            );
            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::centered(size(px(720.), px(560.)), cx)),
                    focus_on_appearing: false,
                    show: true,
                    ..WindowOptions::default()
                },
                |_, cx| {
                    cx.new(|cx| {
                        let tree = cx.new(|cx| {
                            RandomizedElementTree::new_with_config(
                                RandomizedElementTreeConfig::new(0, 8)
                                    .with_topology(RandomizedElementTreeTopology::Wide)
                                    .with_entity_density(1.)
                                    .with_cached_views(true),
                                cx,
                            )
                        });
                        let cached_tree = cx.new(|_| NativeRandomizedTreeCache {
                            tree: tree.clone(),
                            counters: counters.clone(),
                        });
                        NativeRandomizedTreeRoot {
                            tree,
                            cached_tree,
                            counters: counters.clone(),
                            cache_mode_publication: PrepaintPublicationId::new(),
                            button_presentation: SubtreePresentation::Visible,
                            presses: presses.clone(),
                            clicks: clicks.clone(),
                        }
                    })
                },
            )
        })
        .expect("actual randomized-tree consumer opens");
    let registration = platform
        .raw_window_handles
        .read()
        .iter()
        .copied()
        .find(|entry| entry.window_id() == handle.window_id())
        .expect("exact tree HWND registration");
    let native = platform
        .window_from_hwnd(registration.as_raw())
        .expect("tree WindowsWindow owner");
    assert_eq!(native.registration, registration);
    let native_lifetime = Rc::downgrade(&native);
    let (root_lifetime, tree_lifetime, cache_lifetime, scale) = app
        .update_for_test(|cx| {
            let root = handle.root(cx).expect("public root entity owner");
            handle.update(cx, |view, window, cx| {
                let snapshot = view.tree.read(cx).snapshot();
                assert_eq!(snapshot.descendant_count(), 8);
                assert_eq!(snapshot.entity_count(), 8);
                (
                    root.downgrade(),
                    view.tree.downgrade(),
                    view.cached_tree.downgrade(),
                    window.scale_factor(),
                )
            })
        })
        .expect("native tree root and model lifetimes");
    let initial_paint_cursor = observation.cursor();
    assert!(unsafe {
        RedrawWindow(Some(registration.as_raw()), None, None, RDW_INVALIDATE).as_bool()
    });
    pump_messages_until_with_delay(
        &Rc::downgrade(&platform.inner),
        "native tree initialization WM_PAINT",
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
    pump_messages_until_idle(
        &Rc::downgrade(&platform.inner),
        "native tree initialization follow-up",
    );
    let previous = app
        .update_for_test(|cx| {
            handle.update(cx, |_, window, _| {
                let previous = window.rendered_frame_revision();
                window.refresh();
                previous
            })
        })
        .expect("initialize the native tree cache before measuring reuse");
    let (initial, initial_color) =
        submitted_tree_frame(&platform, &mut app, handle, previous, true);
    let initial_cached_state = counters.cached_state();
    assert!(initial_cached_state.0 > 0 && initial_cached_state.1 > 0);
    let root_renders_before_replay = counters.root_renders.get();
    app.update_for_test(|cx| handle.update(cx, |_, _, cx| cx.notify()))
        .expect("clean cached replay");
    let (_, replay_color) = submitted_tree_frame(&platform, &mut app, handle, initial, true);
    assert_eq!(initial_color, replay_color);
    assert!(counters.root_renders.get() > root_renders_before_replay);
    assert_eq!(
        counters.cached_state(),
        initial_cached_state,
        "parent notification must reuse the actual tree's cached render and paint"
    );
    let (mutation, previous) = mutate_native_tree(&mut app, handle, MutationKind::RootColor);
    assert_eq!(mutation, Mutation::RootColor);
    let (_, changed_color) = submitted_tree_frame(&platform, &mut app, handle, previous, true);
    assert_ne!(changed_color, initial_color);
    let changed_cached_state = counters.cached_state();
    assert!(
        changed_cached_state.0 > initial_cached_state.0
            && changed_cached_state.1 > initial_cached_state.1,
        "public RootColor notification must invalidate the cached tree consumer"
    );
    let colored_snapshot = app
        .update_for_test(|cx| handle.read_with(cx, |root, cx| root.tree.read(cx).snapshot()))
        .expect("ordinary mutated model snapshot");
    synthetic_tree_button(&platform, registration, &observation, scale, WM_LBUTTONDOWN);
    assert_eq!(presses.get(), 1);
    let (mutation, previous) = mutate_native_tree(&mut app, handle, MutationKind::RootVisibility);
    assert_eq!(mutation, Mutation::RootVisibility);
    let (_, hidden_color) = submitted_tree_frame(&platform, &mut app, handle, previous, false);
    assert!(hidden_color.is_none());
    for message in [WM_LBUTTONUP, WM_LBUTTONDOWN, WM_LBUTTONUP] {
        synthetic_tree_button(&platform, registration, &observation, scale, message);
    }
    assert_eq!((presses.get(), clicks.get()), (1, 0));
    let (mutation, previous) = mutate_native_tree(&mut app, handle, MutationKind::RootVisibility);
    assert_eq!(mutation, Mutation::RootVisibility);
    let (_, restored_color) = submitted_tree_frame(&platform, &mut app, handle, previous, true);
    assert_eq!(restored_color, changed_color);
    app.update_for_test(|cx| {
        handle.read_with(cx, |root, cx| {
            assert_eq!(root.tree.read(cx).snapshot(), colored_snapshot);
        })
    })
    .expect("restore preserves the same public tree model");
    synthetic_tree_button(&platform, registration, &observation, scale, WM_LBUTTONUP);
    assert_eq!((presses.get(), clicks.get()), (1, 0));
    synthetic_tree_button(&platform, registration, &observation, scale, WM_LBUTTONDOWN);
    assert_eq!(presses.get(), 2);
    synthetic_tree_button(&platform, registration, &observation, scale, WM_LBUTTONUP);
    assert_eq!(clicks.get(), 1);
    pump_messages_until_with_delay(
        &Rc::downgrade(&platform.inner),
        "tree click exact native and entity retirement",
        Duration::from_millis(5),
        || {
            !unsafe { IsWindow(Some(registration.as_raw())).as_bool() }
                && !platform.raw_window_handles.read().contains(&registration)
                && platform.inner.native_retirement_is_settled()
                && root_lifetime.upgrade().is_none()
                && tree_lifetime.upgrade().is_none()
                && cache_lifetime.upgrade().is_none()
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
        panic!("unexpected tree retirement sequence: {lifecycle:?}");
    };
    assert_eq!(quiesced, destroyed);
    assert_eq!(destroyed, terminal);
    app.update_for_test(|cx| {
        assert!(handle.update(cx, |_, _, _| ()).is_err());
        assert!(cx.windows().is_empty());
    });
    assert!(platform.raw_window_handles.read().is_empty());
    drop(native);
    assert!(native_lifetime.upgrade().is_none());
    assert_eq!(Rc::strong_count(&presses), 1);
    assert_eq!(Rc::strong_count(&clicks), 1);
    assert_eq!(Rc::strong_count(&counters), 1);
    app.update_for_test(|cx| cx.shutdown());
    drop(app);
    let released = weak_app
        .update_window(AnyWindowHandle::from(handle), |_, _, _| ())
        .expect_err("tree input and frame callbacks must not retain the App");
    assert_eq!(released.to_string(), "app was released");
    drop(weak_app);
    let pump_platform = Rc::downgrade(&platform.inner);
    drop(platform);
    pump_messages_until_with_delay(
        &pump_platform,
        "tree platform HWND and OLE release",
        Duration::from_millis(5),
        || {
            !unsafe { IsWindow(Some(platform_hwnd)).as_bool() }
                && platform_lifetime.upgrade().is_none()
        },
    );
    assert_eq!(lifecycle_probe.platform_destroy_attempts(), 1);
    assert_eq!(lifecycle_probe.ole_uninitialize_count(), 1);
}
