use super::super::{AFTER_GPU_RECOVERY_CREATION, handle_gpu_device_lost};
use super::*;
use crate::{
    SafeHwnd, WM_GPUI_FORCE_UPDATE_WINDOW, WM_GPUI_GPU_DEVICE_LOST,
    native_test_observation::NativeWindowTestMessageDisposition,
};
use futures::FutureExt as _;
use open_gpui::{
    AsyncApp, AtlasTextureLeaseEpoch, Entity, GraphicalEnvironment, Pixels, PlatformAtlas,
    SharedString, Subscription, TextAlign, TextRun, WindowFrameCapture, WindowHandle,
    WindowInitialPresentationStatus, WindowingRequest, black, fill, font, rgb,
};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    mpsc,
};
use windows::core::Interface as _;

fn settle(task: open_gpui::Task<anyhow::Result<()>>) -> anyhow::Result<()> {
    task.now_or_never()
        .expect("Windows windowing requests must settle synchronously")
}

fn windowed() -> WindowingRequest {
    WindowingRequest::Windowed(GraphicalEnvironment::detect())
}

fn native_options(cx: &open_gpui::App) -> WindowOptions {
    WindowOptions {
        window_bounds: Some(WindowBounds::centered(size(px(640.), px(220.)), cx)),
        focus_on_appearing: false,
        ..WindowOptions::default()
    }
}

#[test]
fn native_windowing_startup_applies_modes_before_the_launch_callback() {
    let current = GraphicalEnvironment::detect()
        .session_id
        .expect("native session");
    for (request, expected_windowed) in [
        (None, true),
        (Some(WindowingRequest::Headless), false),
        (Some(windowed()), true),
        (
            Some(WindowingRequest::Windowed(GraphicalEnvironment {
                session_id: Some(current.wrapping_add(1)),
            })),
            false,
        ),
    ] {
        discard_stale_quit_messages();
        let platform = Rc::new(WindowsPlatform::new_returning_for_test(false).unwrap());
        let hwnd = platform.handle;
        let lifecycle = platform.lifecycle_test_probe.clone();
        assert!(!platform.is_windowed());
        assert!(platform.graphical_environment().is_none());
        assert!(platform.direct_write_text_system.is_some());
        let mut app =
            Application::with_platform(platform.clone()).with_quit_mode(QuitMode::Explicit);
        assert!(
            app.update_for_test(|cx| {
                cx.open_window(native_options(cx), |_, cx| cx.new(|_| Empty))
            })
            .is_err()
        );
        assert!(platform.raw_window_handles.read().is_empty());
        if let Some(request) = request {
            app = app.with_windowing(request);
        }
        platform.inner.request_display_topology_refresh();
        assert!(platform.inner.exact_display_topology_snapshot().is_err());
        let launched = Rc::new(Cell::new(0));
        let launch_count = launched.clone();
        let launching_platform = platform.clone();
        app.run_returning_for_test(move |cx| {
            launch_count.set(launch_count.get() + 1);
            assert_eq!(launching_platform.is_windowed(), expected_windowed);
            assert_eq!(cx.graphical_environment().is_some(), expected_windowed);
            assert_eq!(
                launching_platform.vsync_owner_live.borrow().is_some(),
                expected_windowed
            );
            assert!(
                launching_platform
                    .inner
                    .exact_display_topology_snapshot()
                    .is_ok()
            );
            if let Some(environment) = cx.graphical_environment() {
                assert_eq!(environment.session_id, Some(current));
                assert!(cx.primary_display().is_some());
            }
            cx.quit();
        });
        assert_eq!(launched.get(), 1);
        assert!(platform.inner.native_retirement_is_settled());
        assert!(!unsafe { IsWindow(Some(hwnd)).as_bool() });
        assert_eq!(lifecycle.ole_uninitialize_count(), 1);
        drop(app);
        drop(platform);
        assert_eq!(lifecycle.ole_uninitialize_count(), 1);
    }
}

#[test]
fn native_windowing_fixed_headless_refuses_attach_and_explicit_windowed_startup() {
    discard_stale_quit_messages();
    let platform = Rc::new(WindowsPlatform::new_returning_for_test(true).unwrap());
    let hwnd = platform.handle;
    assert!(platform.direct_write_text_system.is_none());
    assert!(settle(platform.request_windowing(windowed())).is_err());
    assert!(settle(platform.request_windowing(WindowingRequest::Headless)).is_err());
    let mut app = Application::with_platform(platform.clone())
        .with_windowing(windowed())
        .with_quit_mode(QuitMode::Explicit);
    let launching_platform = platform.clone();
    app.run_returning_for_test(move |cx| {
        assert!(cx.graphical_environment().is_none());
        assert!(settle(cx.request_windowing(windowed())).is_err());
        assert!(
            cx.open_window(native_options(cx), |_, cx| cx.new(|_| Empty))
                .is_err()
        );
        assert!(launching_platform.raw_window_handles.read().is_empty());
        assert!(launching_platform.vsync_owner_live.borrow().is_none());
        cx.quit();
    });
    assert!(platform.inner.native_retirement_is_settled());
    assert!(!unsafe { IsWindow(Some(hwnd)).as_bool() });
    assert!(settle(platform.request_windowing(windowed())).is_err());
}

#[test]
fn native_windowing_validates_again_after_gpu_creation_and_recovers_from_refusal_and_unwind() {
    discard_stale_quit_messages();
    let platform = Rc::new(WindowsPlatform::new_returning_for_test(false).unwrap());
    let current = GraphicalEnvironment::detect().session_id.unwrap();
    assert!(
        settle(
            platform.request_windowing(WindowingRequest::Windowed(GraphicalEnvironment {
                session_id: Some(current.wrapping_add(1))
            }))
        )
        .is_err()
    );
    assert!(!platform.is_windowed());
    let calls = Rc::new(Cell::new(0));
    let validations = calls.clone();
    let reentrant = Rc::downgrade(&platform);
    let refused = platform.request_windowing_with_validation(
        windowed(),
        Box::new(move || {
            validations.set(validations.get() + 1);
            let platform = reentrant.upgrade().unwrap();
            assert!(settle(platform.request_windowing(windowed())).is_err());
            if validations.get() == 2 {
                anyhow::bail!("caller custody changed during GPU creation");
            }
            Ok(())
        }),
    );
    assert!(settle(refused).is_err());
    assert_eq!(calls.get(), 2);
    assert!(!platform.is_windowed());
    assert!(!platform.windowing_transition.get());
    assert!(platform.vsync_owner_live.borrow().is_none());
    assert!(platform.recovered_directx_devices.read().is_none());
    let validations = Rc::new(Cell::new(0));
    let panic = catch_unwind(AssertUnwindSafe(|| {
        platform.request_windowing_with_validation(
            windowed(),
            Box::new(move || {
                validations.set(validations.get() + 1);
                if validations.get() == 2 {
                    panic!("injected native windowing validator unwind");
                }
                Ok(())
            }),
        )
    }));
    assert!(panic.is_err());
    assert!(!platform.is_windowed());
    assert!(!platform.windowing_transition.get());
    assert!(platform.vsync_owner_live.borrow().is_none());
    drop(platform.request_windowing(WindowingRequest::Windowed(GraphicalEnvironment::default())));
    assert!(
        platform.is_windowed(),
        "dropping an admitted ready task cannot cancel attach"
    );
    let device = platform
        .inner
        .state
        .directx_devices
        .borrow()
        .as_ref()
        .unwrap()
        .device
        .clone();
    let rejected = platform.request_windowing_with_validation(
        WindowingRequest::Headless,
        Box::new(|| anyhow::bail!("retained native custody")),
    );
    assert!(settle(rejected).is_err());
    assert_eq!(
        platform
            .inner
            .state
            .directx_devices
            .borrow()
            .as_ref()
            .unwrap()
            .device
            .as_raw(),
        device.as_raw()
    );
    assert!(settle(platform.request_windowing(windowed())).is_err());
    settle(platform.request_windowing(WindowingRequest::Headless)).unwrap();
    assert!(settle(platform.request_windowing(WindowingRequest::Headless)).is_err());
    drop(device);
    let hwnd = platform.handle;
    let exiting_platform = platform.clone();
    platform.run(Box::new(move || {
        assert!(
            !exiting_platform.is_windowed(),
            "run must retain a pre-launch runtime detach"
        );
        exiting_platform.quit_after_terminal_shutdown();
    }));
    assert!(!unsafe { IsWindow(Some(hwnd)).as_bool() });
    assert!(settle(platform.request_windowing(windowed())).is_err());
}

struct LiveWindowingModel {
    version: usize,
}

struct LiveWindowingPaint {
    version: usize,
    previous_revision: u64,
    atlas_epoch: AtlasTextureLeaseEpoch,
    quad: Bounds<Pixels>,
    text: Bounds<Pixels>,
    emoji: Bounds<Pixels>,
    glyph_count: usize,
}

struct LiveWindowingRoot {
    model: Entity<LiveWindowingModel>,
    atlas: Arc<dyn PlatformAtlas>,
    paint: Rc<RefCell<Option<LiveWindowingPaint>>>,
    _subscription: Subscription,
}

const LIVE_COLORS: [u32; 4] = [0x204080, 0x802040, 0x208040, 0x804020];

fn live_window_atlas(platform: &WindowsPlatform, window: &Window) -> Arc<dyn PlatformAtlas> {
    let window_id = window.window_handle().window_id();
    let registration = platform
        .raw_window_handles
        .read()
        .iter()
        .find(|entry| entry.window_id() == window_id)
        .copied()
        .expect("the root must use its exact registered native window");
    let native = platform.window_from_hwnd(registration.as_raw()).unwrap();
    let renderer = native.state.renderer.borrow();
    renderer.sprite_atlas()
}

impl Render for LiveWindowingRoot {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let version = self.model.read(cx).version;
        let atlas = self.atlas.clone();
        let paint = self.paint.clone();
        div().size_full().bg(black()).child(
            canvas(
                |_, _, _| {},
                move |bounds, (), window, cx| {
                    let quad = Bounds::new(
                        bounds.origin + point(px(16.), px(16.)),
                        size(px(96.), px(32.)),
                    );
                    window.paint_quad(fill(quad, rgb(LIVE_COLORS[version % LIVE_COLORS.len()])));
                    let source: SharedString = format!("Live GPUI 你好 ééé 😀 {version}").into();
                    let emoji_index = source
                        .find('😀')
                        .expect("the native text contains an emoji");
                    let emoji_end = emoji_index + '😀'.len_utf8();
                    let line = window.text_system().shape_line(
                        source.clone(),
                        px(24.),
                        &[
                            TextRun {
                                len: emoji_index,
                                font: font("Segoe UI"),
                                color: white(),
                                ..Default::default()
                            },
                            TextRun {
                                len: emoji_end - emoji_index,
                                font: font("Segoe UI Emoji"),
                                color: white(),
                                ..Default::default()
                            },
                            TextRun {
                                len: source.len() - emoji_end,
                                font: font("Segoe UI"),
                                color: white(),
                                ..Default::default()
                            },
                        ],
                        None,
                    );
                    assert_eq!(line.text, source);
                    assert_eq!(line.len(), source.len());
                    assert!(line.width > px(0.));
                    let glyph_count = line
                        .runs
                        .iter()
                        .flat_map(|run| &run.glyphs)
                        .inspect(|glyph| {
                            assert!(
                                glyph.index < source.len() && source.is_char_boundary(glyph.index)
                            );
                        })
                        .count();
                    assert!(glyph_count > 0);
                    let text = Bounds::new(
                        bounds.origin + point(px(16.), px(72.)),
                        size(line.width, px(40.)),
                    );
                    let mut emoji_glyphs = line
                        .runs
                        .iter()
                        .flat_map(|run| &run.glyphs)
                        .filter(|glyph| glyph.index == emoji_index);
                    let emoji_glyph = emoji_glyphs
                        .next()
                        .expect("the emoji must have its own glyph");
                    assert!(
                        emoji_glyph.is_emoji,
                        "the emoji must use the color glyph path"
                    );
                    assert_ne!(emoji_glyph.id.0, 0, "the emoji must not be a missing glyph");
                    assert!(
                        emoji_glyphs.next().is_none(),
                        "the emoji must shape to one glyph"
                    );
                    let emoji_left = line.x_for_index(emoji_index);
                    let emoji_right = line.x_for_index(emoji_end);
                    assert!(emoji_right > emoji_left);
                    let emoji = Bounds::new(
                        text.origin + point(emoji_left, px(0.)),
                        size(emoji_right - emoji_left, text.size.height),
                    );
                    assert!(
                        emoji.top() > quad.bottom(),
                        "the emoji ROI must exclude the colored quad"
                    );
                    line.paint(text.origin, px(40.), TextAlign::Left, None, window, cx)
                        .expect("real CJK, shaped text and color emoji must paint after attach");
                    *paint.borrow_mut() = Some(LiveWindowingPaint {
                        version,
                        previous_revision: window.rendered_frame_revision(),
                        atlas_epoch: atlas.atlas_texture_lease_epoch(),
                        quad,
                        text,
                        emoji,
                        glyph_count,
                    });
                },
            )
            .size_full(),
        )
    }
}

async fn wait_live(
    cx: &mut AsyncApp,
    description: &str,
    mut ready: impl FnMut(&mut open_gpui::App) -> bool,
) {
    let deadline = Instant::now() + Duration::from_secs(15);
    while !cx.update(&mut ready) {
        assert!(
            Instant::now() < deadline,
            "{description} did not converge in the returning native loop"
        );
        cx.background_executor()
            .timer(Duration::from_millis(5))
            .await;
    }
}

async fn live_submission(
    cx: &mut AsyncApp,
    handle: WindowHandle<LiveWindowingRoot>,
    version: usize,
    previous: u64,
) -> WindowFrameCapture {
    let mut submitted = None;
    wait_live(cx, "exact native Submitted/readback generation", |cx| {
        submitted = handle.update(cx, |view, window, cx| {
            let facts = window.presentation_facts();
            let attempt = facts.latest_present_attempt?;
            let paint = view.paint.borrow();
            let paint = paint.as_ref()?;
            if attempt.generation <= previous
                || attempt.outcome != PlatformWindowPresentOutcome::Submitted
                || !attempt.contained_valid_primitives
                || facts.initial_presentation != WindowInitialPresentationStatus::Completed
                || !facts.native_visible
                || facts.frame_accepted_generation != Some(attempt.generation)
                || facts.present_submitted_generation != Some(attempt.generation)
                || facts.non_empty_presented_generation != Some(attempt.generation)
                || window.rendered_frame_revision() != attempt.generation
                || window.submitted_frame_diagnostics_for_test()?.generation() != attempt.generation
                || paint.previous_revision.checked_add(1) != Some(attempt.generation)
                || paint.atlas_epoch != view.atlas.atlas_texture_lease_epoch()
                || paint.version != version || view.model.read(cx).version != version
            {
                return None;
            }
            let capture = window.capture_frame();
            assert_eq!(capture.metadata.window_id, handle.window_id());
            assert_eq!(capture.metadata.framework_frame_generation, attempt.generation);
            assert_eq!(capture.metadata.logical_viewport_size, window.viewport_size());
            assert_eq!(capture.metadata.scale_factor, window.scale_factor());
            assert!(capture.unsupported_reason.is_none(), "{capture:?}");
            let image = capture.image.as_ref().expect("real DX11 RGBA readback");
            let scale = capture.metadata.scale_factor;
            assert_eq!(image.dimensions(), (
                capture.metadata.physical_viewport_size.width.0 as u32,
                capture.metadata.physical_viewport_size.height.0 as u32,
            ));
            let pixel = image.get_pixel(
                (paint.quad.center().x.as_f32() * scale).floor() as u32,
                (paint.quad.center().y.as_f32() * scale).floor() as u32,
            ).0;
            let color = LIVE_COLORS[version % LIVE_COLORS.len()];
            let expected = [(color >> 16) as u8, (color >> 8) as u8, color as u8, 255];
            assert!(pixel.iter().zip(expected).all(|(a,b)| a.abs_diff(b) <= 12), "quad RGBA {pixel:?} != {expected:?}");
            let mut visible_text_pixels = 0;
            for y in (paint.text.top().as_f32() * scale).floor() as u32..(paint.text.bottom().as_f32() * scale).ceil() as u32 {
                for x in (paint.text.left().as_f32() * scale).floor() as u32..(paint.text.right().as_f32() * scale).ceil() as u32 {
                    let [r,g,b,a] = image.get_pixel(x,y).0;
                    if r > 24 && g > 24 && b > 24 && a > 240 { visible_text_pixels += 1; }
                }
            }
            assert!(visible_text_pixels >= paint.glyph_count, "shaped CJK text must produce real pixels");
            let left = (paint.emoji.left().as_f32() * scale).floor() as u32;
            let right = (paint.emoji.right().as_f32() * scale).ceil() as u32;
            let top = (paint.emoji.top().as_f32() * scale).floor() as u32;
            let bottom = (paint.emoji.bottom().as_f32() * scale).ceil() as u32;
            assert!(left < right && top < bottom);
            assert!(right <= image.width() && bottom <= image.height(), "the emoji ROI must be captured in full");
            let mut colored_emoji_pixels = 0;
            for y in top..bottom {
                for x in left..right {
                    let [r, g, b, a] = image.get_pixel(x, y).0;
                    if a > 240
                        && r.max(g).max(b) > 64
                        && r.max(g).max(b).saturating_sub(r.min(g).min(b)) > 48
                    {
                        colored_emoji_pixels += 1;
                    }
                }
            }
            assert!(colored_emoji_pixels >= 8,
                "the independently shaped emoji must have colored readback pixels, found {colored_emoji_pixels}");
            Some(capture)
        }).expect("the live native window must remain open until exact readback");
        submitted.is_some()
    }).await;
    submitted.unwrap()
}

async fn exercise_live_windowing(platform: Rc<WindowsPlatform>, cx: &mut AsyncApp) {
    let (_observation_guard, observation) = begin_native_window_test_observation();
    let text_system = cx.update(|cx| cx.text_system().clone());
    let model = cx.update(|cx| cx.new(|_| LiveWindowingModel { version: 0 }));
    let model_id = model.entity_id();
    let main_thread = std::thread::current().id();
    let foreground = platform.foreground_executor();
    let background = platform.background_executor();
    let mut retired_attachments: Vec<(RegisteredWindow, usize, Arc<AtomicBool>)> = Vec::new();
    let mut version = 0;
    for (cycle, recoveries) in [1, 2, 1].into_iter().enumerate() {
        if cycle > 0 {
            version += 1;
        }
        cx.update(|cx| {
            assert!(cx.graphical_environment().is_none());
            assert!(Arc::ptr_eq(cx.text_system(), &text_system));
            assert!(
                cx.open_window(native_options(cx), |_, cx| cx.new(|_| Empty))
                    .is_err()
            );
            model.update(cx, |model, cx| {
                model.version = version;
                cx.notify();
            });
        });
        assert_eq!(
            foreground
                .spawn(async { std::thread::current().id() })
                .await,
            main_thread
        );
        assert_ne!(
            background
                .spawn(async { std::thread::current().id() })
                .await,
            main_thread
        );
        cx.update(|cx| cx.request_windowing(windowed()))
            .await
            .unwrap();
        let devices = platform
            .inner
            .state
            .directx_devices
            .borrow()
            .as_ref()
            .unwrap()
            .clone();
        let attachment_generation = platform.inner.state.gpu_generation.get();
        let attachment_owner = platform.vsync_owner_live.borrow().as_ref().unwrap().clone();
        let invalidation = platform.invalidate_devices.borrow().clone();
        assert!(*attachment_owner.read());
        let (handle, interceptor, hits) = cx.update(|cx| {
            assert_eq!(model.entity_id(), model_id);
            assert!(Arc::ptr_eq(cx.text_system(), &text_system));
            assert_eq!(
                cx.graphical_environment().unwrap().session_id,
                GraphicalEnvironment::detect().session_id
            );
            let handle = cx
                .open_window(native_options(cx), |window, cx| {
                    cx.new(|cx| LiveWindowingRoot {
                        _subscription: cx.observe(&model, |_, _, cx| cx.notify()),
                        model: model.clone(),
                        atlas: live_window_atlas(&platform, window),
                        paint: Rc::new(RefCell::new(None)),
                    })
                })
                .unwrap();
            let hits = Rc::new(Cell::new(0));
            let interceptor = handle
                .update(cx, |_, window, _| {
                    let hits = hits.clone();
                    let model = model.clone();
                    window.intercept_window_mouse_events(move |event, _, cx| {
                        if matches!(event, WindowMouseEvent::Down(_)) {
                            hits.set(hits.get() + 1);
                            model.update(cx, |model, cx| {
                                model.version += 1;
                                cx.notify();
                            });
                        }
                    })
                })
                .unwrap();
            (handle, interceptor, hits)
        });
        let registration = platform
            .raw_window_handles
            .read()
            .iter()
            .find(|entry| entry.window_id() == handle.window_id())
            .copied()
            .unwrap();
        for (previous, previous_gpu_generation, previous_invalidation) in &retired_attachments {
            assert_ne!(registration.generation(), previous.generation());
            assert_ne!(registration.window_id(), previous.window_id());
            assert_ne!(attachment_generation, *previous_gpu_generation);
            assert!(!Arc::ptr_eq(&invalidation, previous_invalidation));
            previous_invalidation.store(true, Ordering::Release);
            assert!(
                !invalidation.load(Ordering::Acquire),
                "retired invalidation must not reach this attachment"
            );
        }
        let native = platform.window_from_hwnd(registration.as_raw()).unwrap();
        let native_lifetime = Rc::downgrade(&native);
        let window_text = cx.update(|cx| {
            handle
                .update(cx, |_, window, _| Rc::downgrade(window.text_system()))
                .unwrap()
        });
        let mut submitted = live_submission(cx, handle, version, 0).await;
        assert!(settle(platform.request_windowing(WindowingRequest::Headless)).is_err());
        assert!(
            cx.update(|cx| settle(cx.request_windowing(WindowingRequest::Headless)))
                .is_err()
        );
        assert_eq!(
            platform
                .inner
                .state
                .directx_devices
                .borrow()
                .as_ref()
                .unwrap()
                .device
                .as_raw(),
            devices.device.as_raw()
        );
        for recovery in 0..recoveries {
            let previous_device = platform
                .inner
                .state
                .directx_devices
                .borrow()
                .as_ref()
                .unwrap()
                .device
                .clone();
            let previous_epoch = native
                .state
                .renderer
                .borrow()
                .sprite_atlas()
                .atlas_texture_lease_epoch();
            let recovery_cursor = observation.cursor();
            let before_recovery = cx.update(|cx| {
                handle
                    .update(cx, |_, window, _| window.rendered_frame_revision())
                    .unwrap()
            });
            invalidation.store(true, Ordering::Release);
            wait_live(
                cx,
                "live VSync device and exact HWND renderer recovery",
                |_| {
                    let current = platform.inner.state.directx_devices.borrow();
                    let current = current.as_ref().unwrap();
                    current.device.as_raw() != previous_device.as_raw()
                        && platform
                            .recovered_directx_devices
                            .read()
                            .as_ref()
                            .is_some_and(|recovered| {
                                recovered.device.as_raw() == current.device.as_raw()
                            })
                        && native
                            .state
                            .renderer
                            .borrow()
                            .sprite_atlas()
                            .atlas_texture_lease_epoch()
                            .get()
                            > previous_epoch.get()
                        && observation.any_after(recovery_cursor, |event| {
                            event.window().window_id() == handle.window_id()
                                && event.window().native_generation() == registration.generation()
                                && matches!(event.kind(), NativeWindowTestEventKind::WindowMessage {
                                message: NativeWindowTestMessage::Other(WM_GPUI_GPU_DEVICE_LOST),
                                wparam,
                                disposition: NativeWindowTestMessageDisposition::Returned(0),
                                ..
                            } if wparam == registration.generation())
                        })
                },
            )
            .await;
            assert_eq!(
                platform.inner.state.gpu_generation.get(),
                attachment_generation
            );
            assert!(platform.raw_window_handles.read().contains(&registration));
            assert!(unsafe { IsWindow(Some(registration.as_raw())).as_bool() });
            assert!(!invalidation.load(Ordering::Acquire));
            let recovered = live_submission(
                cx,
                handle,
                version,
                before_recovery.max(submitted.metadata.framework_frame_generation),
            )
            .await;
            let recovered_device = platform
                .inner
                .state
                .directx_devices
                .borrow()
                .as_ref()
                .unwrap()
                .device
                .clone();
            let recovered_epoch = native
                .state
                .renderer
                .borrow()
                .sprite_atlas()
                .atlas_texture_lease_epoch();
            for (previous, previous_gpu_generation, previous_invalidation) in &retired_attachments {
                previous_invalidation.store(true, Ordering::Release);
                assert!(!invalidation.load(Ordering::Acquire));
                unsafe {
                    let _ = SendMessageW(
                        registration.as_raw(),
                        WM_GPUI_GPU_DEVICE_LOST,
                        Some(WPARAM(previous.generation())),
                        Some(LPARAM(0)),
                    );
                    let _ = SendMessageW(
                        registration.as_raw(),
                        WM_GPUI_FORCE_UPDATE_WINDOW,
                        Some(WPARAM(previous.generation())),
                        Some(LPARAM(0)),
                    );
                    let _ = SendMessageW(
                        platform.handle,
                        WM_GPUI_GPU_DEVICE_LOST,
                        Some(WPARAM(platform.inner.validation_number)),
                        Some(LPARAM(*previous_gpu_generation as isize)),
                    );
                }
                assert_eq!(
                    native
                        .state
                        .renderer
                        .borrow()
                        .sprite_atlas()
                        .atlas_texture_lease_epoch(),
                    recovered_epoch,
                );
                assert_eq!(
                    platform
                        .inner
                        .state
                        .directx_devices
                        .borrow()
                        .as_ref()
                        .unwrap()
                        .device
                        .as_raw(),
                    recovered_device.as_raw(),
                );
            }
            unsafe {
                let _ = SetActiveWindow(registration.as_raw());
                let _ = SendMessageW(
                    registration.as_raw(),
                    WM_LBUTTONDOWN,
                    Some(WPARAM(MK_LBUTTON.0 as usize)),
                    Some(mouse_position_lparam(30, 30)),
                );
                let _ = SendMessageW(
                    registration.as_raw(),
                    WM_LBUTTONUP,
                    Some(WPARAM(0)),
                    Some(mouse_position_lparam(30, 30)),
                );
            }
            version += 1;
            wait_live(
                cx,
                "native mouse delivery after live device recovery",
                |_| hits.get() == recovery + 1,
            )
            .await;
            submitted = live_submission(
                cx,
                handle,
                version,
                recovered.metadata.framework_frame_generation,
            )
            .await;
            assert_ne!(submitted.image, recovered.image);
        }
        platform.lifecycle_test_probe.clear_events();
        platform.lifecycle_test_probe.fail_destroy_attempts(3);
        cx.update(|cx| handle.update(cx, |_, window, cx| window.remove_window(cx)))
            .unwrap();
        assert!(cx.update(|cx| cx.windows().is_empty()));
        assert!(unsafe { IsWindow(Some(registration.as_raw())).as_bool() });
        assert!(platform.raw_window_handles.read().contains(&registration));
        let retirement_events = platform.lifecycle_test_probe.events();
        assert!(retirement_events.iter().any(|event| matches!(
            event,
            NativeWindowLifecycleTestEvent::DestroyEntered { window_id, .. }
                if *window_id == handle.window_id()
        )));
        assert!(!retirement_events.iter().any(|event| matches!(
            event,
            NativeWindowLifecycleTestEvent::NativeTerminal { window_id, .. }
                if *window_id == handle.window_id()
        )));
        let refused = settle(platform.request_windowing(WindowingRequest::Headless)).unwrap_err();
        assert!(refused.to_string().contains("native windows remain"));
        assert!(
            cx.update(|cx| settle(cx.request_windowing(WindowingRequest::Headless)))
                .is_err()
        );
        wait_live(
            cx,
            "exact HWND and native retirement completion before detach",
            |cx| {
                cx.windows().is_empty()
                    && !unsafe { IsWindow(Some(registration.as_raw())).as_bool() }
                    && !platform.raw_window_handles.read().contains(&registration)
                    && platform.inner.native_retirement_is_settled()
            },
        )
        .await;
        drop(interceptor);
        drop(native);
        assert!(native_lifetime.upgrade().is_none());
        assert!(window_text.upgrade().is_none());
        cx.update(|cx| cx.request_windowing(WindowingRequest::Headless))
            .await
            .unwrap();
        assert!(!platform.is_windowed());
        assert!(platform.vsync_owner_live.borrow().is_none());
        assert!(!*attachment_owner.read());
        assert!(platform.recovered_directx_devices.read().is_none());
        retired_attachments.push((registration, attachment_generation, invalidation));
    }
    assert_eq!(
        foreground
            .spawn(async { std::thread::current().id() })
            .await,
        main_thread
    );
    assert_eq!(cx.update(|cx| model.read(cx).version), version);
    drop(model);
}

#[test]
fn native_windowing_same_app_round_trip_renders_receives_input_retires_and_exits_once() {
    discard_stale_quit_messages();
    let platform = Rc::new(WindowsPlatform::new_returning_for_test(false).unwrap());
    let hwnd = platform.handle;
    let lifetime = Rc::downgrade(&platform.inner);
    let backend_text = Arc::downgrade(&platform.text_system());
    let lifecycle = platform.lifecycle_test_probe.clone();
    let mut app = Application::with_platform(platform.clone())
        .with_windowing(WindowingRequest::Headless)
        .with_quit_mode(QuitMode::Explicit);
    let text_lifetime = Arc::downgrade(&app.text_system());
    let shutdowns = Rc::new(Cell::new(0));
    let callback_capture = Arc::new(());
    let callback_lifetime = Arc::downgrade(&callback_capture);
    let outcome = Rc::new(RefCell::new(None));
    let completed = outcome.clone();
    let launching_platform = platform.clone();
    let quit_count = shutdowns.clone();
    app.run_returning_for_test(move |cx| {
        assert!(cx.graphical_environment().is_none());
        cx.on_app_quit(move |_| {
            let _capture = &callback_capture;
            quit_count.set(quit_count.get() + 1);
            async {}
        })
        .detach();
        cx.spawn(async move |cx| {
            let result = AssertUnwindSafe(exercise_live_windowing(launching_platform.clone(), cx))
                .catch_unwind()
                .await;
            *completed.borrow_mut() = Some(result);
            launching_platform.quit();
        })
        .detach();
    });
    if let Err(panic) = outcome
        .borrow_mut()
        .take()
        .expect("native workflow must settle before exit")
    {
        std::panic::resume_unwind(panic);
    }
    assert_eq!(shutdowns.get(), 1);
    assert!(platform.inner.native_retirement_is_settled());
    assert!(!unsafe { IsWindow(Some(hwnd)).as_bool() });
    assert_eq!(lifecycle.ole_uninitialize_count(), 1);
    assert!(settle(platform.request_windowing(windowed())).is_err());
    app.update_for_test(|cx| {
        assert!(cx.windows().is_empty());
        assert!(cx.native_exit_authority_is_settled_for_test());
    });
    drop(app);
    drop(platform);
    assert!(lifetime.upgrade().is_none());
    assert!(backend_text.upgrade().is_none());
    assert!(text_lifetime.upgrade().is_none());
    assert!(callback_lifetime.upgrade().is_none());
}

fn suspended_native_recovery(
    platform: &WindowsPlatform,
) -> (
    mpsc::SyncSender<()>,
    std::thread::JoinHandle<anyhow::Result<()>>,
) {
    let generation = platform.inner.state.gpu_generation.get();
    let owner = platform.vsync_owner_live.borrow().as_ref().unwrap().clone();
    let recovered = platform.recovered_directx_devices.clone();
    let text = Arc::downgrade(platform.direct_write_text_system.as_ref().unwrap());
    let windows = Arc::downgrade(&platform.raw_window_handles);
    let platform_hwnd: SafeHwnd = platform.handle.into();
    let validation = platform.inner.validation_number;
    let mut devices = platform
        .inner
        .state
        .directx_devices
        .borrow()
        .as_ref()
        .unwrap()
        .clone();
    let (prepared_sender, prepared_receiver) = mpsc::sync_channel(1);
    let (resume_sender, resume_receiver) = mpsc::sync_channel(1);
    let worker = std::thread::spawn(move || {
        AFTER_GPU_RECOVERY_CREATION.with_borrow_mut(|hook| {
            *hook = Some(Box::new(move || {
                prepared_sender.send(()).unwrap();
                resume_receiver
                    .recv_timeout(Duration::from_secs(15))
                    .unwrap();
            }))
        });
        handle_gpu_device_lost(
            &mut devices,
            platform_hwnd.as_raw(),
            validation,
            &windows,
            &text,
            &recovered,
            &owner,
            generation,
        )
    });
    prepared_receiver
        .recv_timeout(Duration::from_secs(15))
        .expect("real DirectX recovery must reach publication");
    (resume_sender, worker)
}

#[test]
fn native_windowing_late_device_recovery_and_old_invalidation_cannot_cross_attachments() {
    discard_stale_quit_messages();
    let platform = Rc::new(WindowsPlatform::new_returning_for_test(false).unwrap());
    settle(platform.request_windowing(windowed())).unwrap();
    let old_generation = platform.inner.state.gpu_generation.get();
    let old_invalidation = platform.invalidate_devices.borrow().clone();
    let validation = platform.inner.validation_number;
    let (resume_sender, worker) = suspended_native_recovery(&platform);
    settle(platform.request_windowing(WindowingRequest::Headless)).unwrap();
    settle(platform.request_windowing(windowed())).unwrap();
    let new_generation = platform.inner.state.gpu_generation.get();
    assert_ne!(old_generation, new_generation);
    assert!(!Arc::ptr_eq(
        &old_invalidation,
        &platform.invalidate_devices.borrow()
    ));
    old_invalidation.store(true, std::sync::atomic::Ordering::Release);
    assert!(
        !platform
            .invalidate_devices
            .borrow()
            .load(std::sync::atomic::Ordering::Acquire)
    );
    let device = platform
        .inner
        .state
        .directx_devices
        .borrow()
        .as_ref()
        .unwrap()
        .device
        .clone();
    resume_sender.send(()).unwrap();
    worker
        .join()
        .expect("late recovery worker must exit")
        .unwrap();
    assert!(platform.recovered_directx_devices.read().is_none());
    let owner = platform.vsync_owner_live.borrow().as_ref().unwrap().clone();
    let mut recovered_devices = platform
        .inner
        .state
        .directx_devices
        .borrow()
        .as_ref()
        .unwrap()
        .clone();
    handle_gpu_device_lost(
        &mut recovered_devices,
        platform.handle,
        validation,
        &Arc::downgrade(&platform.raw_window_handles),
        &Arc::downgrade(platform.direct_write_text_system.as_ref().unwrap()),
        &platform.recovered_directx_devices,
        &owner,
        new_generation,
    )
    .unwrap();
    assert_ne!(recovered_devices.device.as_raw(), device.as_raw());
    unsafe {
        let _ = SendMessageW(
            platform.handle,
            WM_GPUI_GPU_DEVICE_LOST,
            Some(WPARAM(platform.inner.validation_number)),
            Some(LPARAM(old_generation as isize)),
        );
    }
    assert_eq!(
        platform
            .inner
            .state
            .directx_devices
            .borrow()
            .as_ref()
            .unwrap()
            .device
            .as_raw(),
        device.as_raw()
    );
    unsafe {
        let _ = SendMessageW(
            platform.handle,
            WM_GPUI_GPU_DEVICE_LOST,
            Some(WPARAM(platform.inner.validation_number)),
            Some(LPARAM(new_generation as isize)),
        );
    }
    assert_eq!(
        platform
            .inner
            .state
            .directx_devices
            .borrow()
            .as_ref()
            .unwrap()
            .device
            .as_raw(),
        recovered_devices.device.as_raw()
    );
    let mut app = Application::with_platform(platform.clone()).with_quit_mode(QuitMode::Explicit);
    let outcome = Rc::new(RefCell::new(None));
    let completed = outcome.clone();
    let launching_platform = platform.clone();
    app.run_returning_for_test(move |cx| {
        let model = cx.new(|_| LiveWindowingModel { version: 0 });
        let handle = cx
            .open_window(native_options(cx), |window, cx| {
                cx.new(|cx| LiveWindowingRoot {
                    _subscription: cx.observe(&model, |_, _, cx| cx.notify()),
                    model: model.clone(),
                    atlas: live_window_atlas(&launching_platform, window),
                    paint: Rc::new(RefCell::new(None)),
                })
            })
            .unwrap();
        let hwnd = launching_platform
            .raw_window_handles
            .read()
            .last()
            .unwrap()
            .as_raw();
        cx.spawn(async move |cx| {
            let workflow = async {
                live_submission(cx, handle, 0, 0).await;
                cx.update(|cx| handle.update(cx, |_, window, cx| window.remove_window(cx)))
                    .unwrap();
                wait_live(cx, "late recovery HWND retirement", |_| {
                    !unsafe { IsWindow(Some(hwnd)).as_bool() }
                        && launching_platform.inner.native_retirement_is_settled()
                })
                .await;
                cx.update(|cx| cx.request_windowing(WindowingRequest::Headless))
                    .await
                    .unwrap();
                assert!(
                    launching_platform
                        .recovered_directx_devices
                        .read()
                        .is_none()
                );
                unsafe {
                    let _ = SendMessageW(
                        launching_platform.handle,
                        WM_GPUI_GPU_DEVICE_LOST,
                        Some(WPARAM(launching_platform.inner.validation_number)),
                        Some(LPARAM(new_generation as isize)),
                    );
                }
                assert!(!launching_platform.is_windowed());
            };
            *completed.borrow_mut() = Some(AssertUnwindSafe(workflow).catch_unwind().await);
            launching_platform.quit();
        })
        .detach();
    });
    if let Err(panic) = outcome
        .borrow_mut()
        .take()
        .expect("late recovery workflow must settle")
    {
        std::panic::resume_unwind(panic);
    }
    assert!(platform.inner.native_retirement_is_settled());
    assert!(!unsafe { IsWindow(Some(platform.handle)).as_bool() });
}

#[test]
fn native_windowing_terminal_exit_fences_prepared_recovery_and_releases_callback_owners_once() {
    discard_stale_quit_messages();
    let platform = Rc::new(WindowsPlatform::new_returning_for_test(false).unwrap());
    settle(platform.request_windowing(windowed())).unwrap();
    let hwnd = platform.handle;
    let lifecycle = platform.lifecycle_test_probe.clone();
    let lifetime = Rc::downgrade(&platform.inner);
    let text_lifetime = Arc::downgrade(&platform.text_system());
    let (resume_sender, worker) = suspended_native_recovery(&platform);
    let mut app = Application::with_platform(platform.clone()).with_quit_mode(QuitMode::Explicit);
    let callback_capture = Arc::new(());
    let callback_lifetime = Arc::downgrade(&callback_capture);
    let shutdowns = Rc::new(Cell::new(0));
    let calls = shutdowns.clone();
    app.run_returning_for_test(move |cx| {
        cx.on_app_quit(move |_| {
            let _capture = &callback_capture;
            calls.set(calls.get() + 1);
            async {}
        })
        .detach();
        cx.quit();
        cx.quit();
    });
    assert_eq!(shutdowns.get(), 1);
    assert!(!unsafe { IsWindow(Some(hwnd)).as_bool() });
    assert!(platform.inner.native_retirement_is_settled());
    assert_eq!(lifecycle.ole_uninitialize_count(), 1);
    resume_sender.send(()).unwrap();
    worker
        .join()
        .expect("terminal recovery worker must exit")
        .unwrap();
    assert!(platform.recovered_directx_devices.read().is_none());
    assert!(settle(platform.request_windowing(windowed())).is_err());
    assert!(settle(platform.request_windowing(WindowingRequest::Headless)).is_err());
    drop(app);
    drop(platform);
    assert!(lifetime.upgrade().is_none());
    assert!(text_lifetime.upgrade().is_none());
    assert!(callback_lifetime.upgrade().is_none());
    assert_eq!(lifecycle.ole_uninitialize_count(), 1);
}
