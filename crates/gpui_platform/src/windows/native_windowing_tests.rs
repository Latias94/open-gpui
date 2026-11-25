use super::*;
use crate::windows::WM_GPUI_FORCE_UPDATE_WINDOW;
use futures::FutureExt as _;
use open_gpui::{
    AsyncApp, AtlasTextureLeaseEpoch, Entity, GraphicalEnvironment, ImageTexture,
    ImageTextureAlpha, ImageTextureSampling, ImageTextureTransfer, Pixels, PlatformAtlas, Scene,
    SharedString, Subscription, TextAlign, TextRun, WindowBackgroundAppearance, WindowFrameCapture,
    WindowHandle, WindowInitialPresentationStatus, WindowingRequest, black, fill, font, rgb,
};
use open_gpui_wgpu::{
    DeviceErrorState, SharedWgpuContext, WgpuAtlas, WgpuImportedImageDescriptor, wgpu,
};

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
        assert!(
            platform
                .text_system()
                .font_id(&font("Segoe UI Emoji"))
                .is_ok()
        );
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
    assert!(platform.text_system().all_font_names().is_empty());
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
fn native_windowing_revalidates_before_attach_and_recovers_from_refusal_and_unwind() {
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
                anyhow::bail!("caller custody changed before windowing attachment");
            }
            Ok(())
        }),
    );
    assert!(settle(refused).is_err());
    assert_eq!(calls.get(), 2);
    assert!(!platform.is_windowed());
    assert!(!platform.windowing_transition.get());
    assert!(platform.vsync_owner_live.borrow().is_none());
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
    let owner = platform.vsync_owner_live.borrow().as_ref().unwrap().clone();
    let rejected = platform.request_windowing_with_validation(
        WindowingRequest::Headless,
        Box::new(|| anyhow::bail!("retained native custody")),
    );
    assert!(settle(rejected).is_err());
    assert!(Arc::ptr_eq(
        platform.vsync_owner_live.borrow().as_ref().unwrap(),
        &owner
    ));
    assert!(settle(platform.request_windowing(windowed())).is_err());
    settle(platform.request_windowing(WindowingRequest::Headless)).unwrap();
    assert!(settle(platform.request_windowing(WindowingRequest::Headless)).is_err());
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
            let image = capture.image.as_ref().expect("real WGPU RGBA readback");
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
    let text_system = cx.update(|cx| cx.text_system().clone());
    let model = cx.update(|cx| cx.new(|_| LiveWindowingModel { version: 0 }));
    let model_id = model.entity_id();
    let main_thread = std::thread::current().id();
    let foreground = platform.foreground_executor();
    let background = platform.background_executor();
    let mut retired_attachments: Vec<(RegisteredWindow, Arc<DeviceErrorState>)> = Vec::new();
    let mut version = 0;
    for cycle in 0..3 {
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
        let attachment_owner = platform.vsync_owner_live.borrow().as_ref().unwrap().clone();
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
        let native = platform.window_from_hwnd(registration.as_raw()).unwrap();
        let device_errors = native
            .state
            .renderer
            .borrow()
            .native_device_errors()
            .unwrap()
            .clone();
        for (previous, previous_errors) in &retired_attachments {
            assert_ne!(registration.generation(), previous.generation());
            assert_ne!(registration.window_id(), previous.window_id());
            assert!(!Arc::ptr_eq(&device_errors, previous_errors));
        }
        let native_lifetime = Rc::downgrade(&native);
        let window_text = cx.update(|cx| {
            handle
                .update(cx, |_, window, _| Rc::downgrade(window.text_system()))
                .unwrap()
        });
        let submitted = live_submission(cx, handle, version, 0).await;
        assert!(settle(platform.request_windowing(WindowingRequest::Headless)).is_err());
        assert!(
            cx.update(|cx| settle(cx.request_windowing(WindowingRequest::Headless)))
                .is_err()
        );
        for (previous, _) in &retired_attachments {
            unsafe {
                let _ = SendMessageW(
                    registration.as_raw(),
                    WM_GPUI_FORCE_UPDATE_WINDOW,
                    Some(WPARAM(platform.inner.validation_number)),
                    Some(LPARAM(previous.generation() as isize)),
                );
            }
        }
        assert!(Arc::ptr_eq(
            native
                .state
                .renderer
                .borrow()
                .native_device_errors()
                .unwrap(),
            &device_errors,
        ));
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
        wait_live(cx, "native mouse delivery after WGPU attachment", |_| {
            hits.get() == 1
        })
        .await;
        let changed = live_submission(
            cx,
            handle,
            version,
            submitted.metadata.framework_frame_generation,
        )
        .await;
        assert_ne!(submitted.image, changed.image);
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
        retired_attachments.push((registration, device_errors));
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

struct SharedViewport(ImageTexture);

impl Render for SharedViewport {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let image = self.0.clone();
        canvas(
            |_, _, _| {},
            move |bounds, (), window, _| {
                window
                    .paint_image_texture(bounds, Default::default(), &image)
                    .expect("the native atlas must accept its shared WGPU image");
            },
        )
        .size_full()
    }
}

#[test]
fn native_shared_wgpu_windows_present_same_gpu_import_and_retire() -> anyhow::Result<()> {
    discard_stale_quit_messages();
    let context = open_gpui::block_on(async {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::DX12 | wgpu::Backends::VULKAN,
            ..wgpu::InstanceDescriptor::new_without_display_handle()
        });
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions::default())
            .await?;
        SharedWgpuContext::request_device(instance, adapter, wgpu::DeviceDescriptor::default())
            .await
    })?;
    let texture = context.device().create_texture(&wgpu::TextureDescriptor {
        label: Some("shared viewport"),
        size: wgpu::Extent3d {
            width: 1,
            height: 1,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8UnormSrgb,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    });
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    let mut encoder = context.device().create_command_encoder(&Default::default());
    drop(encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view: &view,
            depth_slice: None,
            resolve_target: None,
            ops: wgpu::Operations {
                load: wgpu::LoadOp::Clear(wgpu::Color {
                    r: 0.,
                    g: 0.5,
                    b: 1.,
                    a: 1.,
                }),
                store: wgpu::StoreOp::Store,
            },
        })],
        ..Default::default()
    }));
    context.queue().submit([encoder.finish()]);

    let platform = Rc::new(WindowsPlatform::new_with_shared_wgpu(context.clone())?);
    platform.connect_initially();
    let mut app = Application::with_platform(platform.clone()).with_quit_mode(QuitMode::Explicit);
    let mut windows = Vec::new();
    for window_background in [
        WindowBackgroundAppearance::Opaque,
        WindowBackgroundAppearance::Transparent,
    ] {
        let handle = app.update_for_test(|cx| {
            cx.open_window(
                WindowOptions {
                    window_background,
                    ..native_options(cx)
                },
                |window, cx| {
                    let image = window
                        .image_atlas::<WgpuAtlas>()
                        .unwrap()
                        .import_image(WgpuImportedImageDescriptor {
                            texture: texture.clone(),
                            mip_level: 0,
                            array_layer: 0,
                            region: Bounds::new(
                                point(DevicePixels(0), DevicePixels(0)),
                                size(DevicePixels(1), DevicePixels(1)),
                            ),
                            transfer: ImageTextureTransfer::Srgb,
                            alpha: ImageTextureAlpha::Premultiplied,
                            sampling: ImageTextureSampling::Nearest,
                            owner: None,
                        })
                        .expect("both native WGPU windows must import the same device's texture");
                    cx.new(|_| SharedViewport(image))
                },
            )
        })?;
        let registration = *platform.raw_window_handles.read().last().unwrap();
        assert_eq!(registration.window_id(), handle.window_id());
        let native = platform.window_from_hwnd(registration.as_raw()).unwrap();
        assert!(Arc::ptr_eq(
            native
                .state
                .renderer
                .borrow()
                .native_device_errors()
                .unwrap(),
            context.errors()
        ));
        assert_eq!(
            native.state.background_appearance.get(),
            WindowBackgroundAppearance::Opaque
        );
        pump_messages_until_with_delay(
            &Rc::downgrade(&platform.inner),
            "native shared WGPU submission",
            Duration::from_millis(2),
            || {
                app.update_for_test(|cx| {
                    handle.update(cx, |_, window, _| {
                        window
                            .presentation_facts()
                            .non_empty_presented_generation
                            .is_some()
                    })
                })
                .unwrap()
            },
        );
        let image = app
            .update_for_test(|cx| handle.update(cx, |_, window, _| window.capture_frame()))?
            .into_image()?;
        let actual = image.get_pixel(image.width() / 2, image.height() / 2).0;
        assert!(
            actual
                .iter()
                .zip([0, 188, 255, 255])
                .all(|(actual, expected)| actual.abs_diff(expected) <= 2),
            "unexpected shared viewport RGBA {actual:?}"
        );
        windows.push((handle, registration));
    }

    context.device().destroy();
    context.device().poll(wgpu::PollType::Poll)?;
    assert!(context.device_lost());
    for (handle, registration) in &windows {
        let native = platform.window_from_hwnd(registration.as_raw()).unwrap();
        let mut renderer = native.state.renderer.borrow_mut();
        assert_eq!(
            renderer.draw(&Scene::default(), WindowBackgroundAppearance::Opaque)?,
            PlatformWindowPresentOutcome::Rejected
        );
        assert!(Arc::ptr_eq(
            renderer.native_device_errors().unwrap(),
            context.errors()
        ));
        drop(renderer);
        app.update_for_test(|cx| handle.update(cx, |_, window, cx| window.remove_window(cx)))?;
    }
    pump_messages_until_with_delay(
        &Rc::downgrade(&platform.inner),
        "native shared WGPU retirement",
        Duration::from_millis(2),
        || {
            platform.inner.native_retirement_is_settled()
                && platform.raw_window_handles.read().is_empty()
        },
    );
    assert!(
        windows
            .iter()
            .all(|(_, registration)| !unsafe { IsWindow(Some(registration.as_raw())).as_bool() })
    );
    Ok(())
}

#[test]
fn native_windowing_terminal_exit_releases_callback_owners_once() {
    discard_stale_quit_messages();
    let platform = Rc::new(WindowsPlatform::new_returning_for_test(false).unwrap());
    settle(platform.request_windowing(windowed())).unwrap();
    let hwnd = platform.handle;
    let lifecycle = platform.lifecycle_test_probe.clone();
    let lifetime = Rc::downgrade(&platform.inner);
    let text_lifetime = Arc::downgrade(&platform.text_system());
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
    assert!(settle(platform.request_windowing(windowed())).is_err());
    assert!(settle(platform.request_windowing(WindowingRequest::Headless)).is_err());
    drop(app);
    drop(platform);
    assert!(lifetime.upgrade().is_none());
    assert!(text_lifetime.upgrade().is_none());
    assert!(callback_lifetime.upgrade().is_none());
    assert_eq!(lifecycle.ole_uninitialize_count(), 1);
}
