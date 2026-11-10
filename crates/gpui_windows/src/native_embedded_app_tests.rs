//! Consumes the public embedded App API through a returning VisualTestPlatform over Windows.
//!
//! The test host polls the visual dispatcher and real Windows messages with native vsync.
//! This qualifies a visual test world with real DirectWrite/DX11 windows, not production Windows
//! external-loop support, OS clock/input fidelity, compositor presentation, or performance.
//! Stock WindowsPlatform::run remains blocking and process-owning.

use super::*;
use crate::native_test_observation::NativeWindowTestMessageDisposition;
use open_gpui::{
    ApplicationHandle, AssetSource, AsyncApp, Entity, Modifiers, MouseMoveEvent,
    NativeInputInvariantViolation, NativeInvariantFailure, Pixels, SharedString, Subscription,
    TextAlign, TextRun, VisualTestPlatform, WindowFrameCapture, WindowHandle,
    WindowInitialPresentationStatus, black, fill, font, rgb,
};
use std::borrow::Cow;

const EMBEDDED_FONT: &str = "embedded-app/IBMPlexSans-Regular.ttf";
const EMBEDDED_LABELS: [&str; 4] = [
    "embedded launch",
    "external handle notification",
    "async visual timer completed",
    "external handle resumed",
];
const EMBEDDED_COLORS: [u32; 4] = [0x204080, 0x802040, 0x208040, 0x804020];
const EMBEDDED_TIMER: Duration = Duration::from_millis(100);

struct NativeEmbeddedAssets;

impl AssetSource for NativeEmbeddedAssets {
    fn load(&self, path: &str) -> anyhow::Result<Option<Cow<'static, [u8]>>> {
        Ok((path == EMBEDDED_FONT).then_some(Cow::Borrowed(
            include_bytes!("../../../tests/fixtures/fonts/IBMPlexSans-Regular.ttf").as_slice(),
        )))
    }

    fn list(&self, path: &str) -> anyhow::Result<Vec<SharedString>> {
        Ok(EMBEDDED_FONT
            .starts_with(path)
            .then(|| SharedString::from(EMBEDDED_FONT))
            .into_iter()
            .collect())
    }
}

struct NativeEmbeddedModel {
    version: usize,
}

#[derive(Clone, Debug)]
struct NativeEmbeddedPaint {
    window_id: WindowId,
    previous_revision: u64,
    version: usize,
    quad: Bounds<Pixels>,
    text: Bounds<Pixels>,
    glyph_count: usize,
}

struct NativeEmbeddedRoot {
    model: Entity<NativeEmbeddedModel>,
    paint: Rc<RefCell<Option<NativeEmbeddedPaint>>>,
    _model_subscription: Subscription,
}

impl Render for NativeEmbeddedRoot {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let version = self.model.read(cx).version;
        let paint = self.paint.clone();
        div().size_full().bg(black()).child(
            canvas(
                |_, _, _| {},
                move |bounds, (), window, cx| {
                    let quad = Bounds::new(
                        bounds.origin + point(px(16.), px(16.)),
                        size(px(96.), px(32.)),
                    );
                    window.paint_quad(fill(quad, rgb(EMBEDDED_COLORS[version])));
                    let source = EMBEDDED_LABELS[version];
                    let line = window.text_system().shape_line(
                        source.into(),
                        px(24.),
                        &[TextRun {
                            len: source.len(),
                            font: font("IBM Plex Sans"),
                            color: white(),
                            ..Default::default()
                        }],
                        None,
                    );
                    assert_eq!(line.text.as_ref(), source);
                    let glyph_count = line.runs.iter().flat_map(|run| &run.glyphs).count();
                    assert!(glyph_count > 0);
                    assert!(line.width > px(0.));
                    let text = Bounds::new(
                        bounds.origin + point(px(16.), px(72.)),
                        size(line.width, px(36.)),
                    );
                    line.paint(text.origin, px(36.), TextAlign::Left, None, window, cx)
                        .expect("the embedded consumer must paint real DirectWrite glyphs");
                    paint.borrow_mut().replace(NativeEmbeddedPaint {
                        window_id: window.window_handle().window_id(),
                        previous_revision: window.rendered_frame_revision(),
                        version,
                        quad,
                        text,
                        glyph_count,
                    });
                },
            )
            .size_full(),
        )
    }
}

fn poll_embedded_tasks(visual: &VisualTestPlatform) {
    for _ in 0..32 {
        if !visual.dispatcher().tick(false) {
            break;
        }
    }
}

fn pump_embedded_until(
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
            poll_embedded_tasks(visual);
            converged()
        },
    )
}

fn wait_for_embedded_submission(
    platform: &WindowsPlatform,
    app: &ApplicationHandle,
    visual: &VisualTestPlatform,
    handle: WindowHandle<NativeEmbeddedRoot>,
    version: usize,
    previous: u64,
) -> WindowFrameCapture {
    let mut submitted = None;
    pump_embedded_until(
        platform,
        visual,
        "embedded host accepted nonempty text/quad submission and exact-generation capture",
        || {
            submitted = app
                .update(|cx| {
                    handle.update(cx, |view, window, cx| {
                        let facts = window.presentation_facts();
                        let attempt = facts.latest_present_attempt?;
                        let paint = view.paint.borrow().as_ref()?.clone();
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
                            || paint.version != version
                            || view.model.read(cx).version != version
                        {
                            return None;
                        }
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
                        let image = capture.image.as_ref().expect("real embedded DX11 RGBA");
                        assert_eq!(
                            image.dimensions(),
                            (
                                capture.metadata.physical_viewport_size.width.0 as u32,
                                capture.metadata.physical_viewport_size.height.0 as u32,
                            )
                        );
                        let scale = capture.metadata.scale_factor;
                        let x = ((f32::from(paint.quad.left())
                            + f32::from(paint.quad.size.width) / 2.)
                            * scale)
                            .floor() as u32;
                        let y = ((f32::from(paint.quad.top())
                            + f32::from(paint.quad.size.height) / 2.)
                            * scale)
                            .floor() as u32;
                        let color = EMBEDDED_COLORS[version];
                        let expected = [(color >> 16) as u8, (color >> 8) as u8, color as u8, 255];
                        let actual = image.get_pixel(x, y).0;
                        assert!(
                            actual
                                .iter()
                                .zip(expected)
                                .all(|(a, b)| a.abs_diff(b) <= 12),
                            "embedded generation {} quad RGBA: {actual:?} != {expected:?}",
                            attempt.generation
                        );
                        let mut text_pixels = 0;
                        for y in (f32::from(paint.text.top()) * scale).floor() as u32
                            ..(f32::from(paint.text.bottom()) * scale).ceil() as u32
                        {
                            for x in (f32::from(paint.text.left()) * scale).floor() as u32
                                ..(f32::from(paint.text.right()) * scale).ceil() as u32
                            {
                                let [r, g, b, a] = image.get_pixel(x, y).0;
                                if r > 24 && g > 24 && b > 24 && a > 240 {
                                    text_pixels += 1;
                                }
                            }
                        }
                        assert!(
                            text_pixels >= paint.glyph_count,
                            "missing embedded text pixels"
                        );
                        Some(capture)
                    })
                })
                .expect("the embedded window must stay open until its actual frame is submitted");
            submitted.is_some()
        },
    );
    submitted.expect("embedded pixels must belong to the exact accepted Submitted generation")
}

#[test]
fn native_embedded_visual_host_updates_awaits_submits_and_releases_exact_owners() {
    discard_stale_quit_messages();
    let (_guard, observation) = begin_native_window_test_observation();
    let platform =
        Rc::new(WindowsPlatform::new_windowed_for_test().expect("embedded native Windows backend"));
    let platform_hwnd = platform.handle;
    let platform_lifetime = Rc::downgrade(&platform);
    let lifecycle_probe = platform.lifecycle_test_probe.clone();
    let backend_lifetime = Arc::downgrade(&platform.text_system());
    let visual = Rc::new(VisualTestPlatform::new(platform.clone(), 0));
    let visual_lifetime = Rc::downgrade(&visual);
    let application = Application::with_platform(visual.clone())
        .with_assets(NativeEmbeddedAssets)
        .with_quit_mode(QuitMode::Explicit);
    let launches = Rc::new(Cell::new(0));
    let opened = Rc::new(Cell::new(None));
    let app = application.run_embedded({
        let launches = launches.clone();
        let opened = opened.clone();
        move |cx| {
            launches.set(launches.get() + 1);
            let bytes = cx
                .asset_source()
                .load(EMBEDDED_FONT)
                .expect("load the embedded consumer font")
                .expect("the embedded font must exist");
            cx.text_system()
                .add_fonts(vec![bytes])
                .expect("install the embedded consumer's exact native font");
            let model = cx.new(|_| NativeEmbeddedModel { version: 0 });
            let handle = cx
                .open_window(
                    WindowOptions {
                        window_bounds: Some(WindowBounds::centered(size(px(640.), px(200.)), cx)),
                        focus_on_appearing: false,
                        show: true,
                        ..WindowOptions::default()
                    },
                    |_, cx| {
                        cx.new(|cx| NativeEmbeddedRoot {
                            _model_subscription: cx.observe(&model, |_, _, cx| cx.notify()),
                            model,
                            paint: Rc::new(RefCell::new(None)),
                        })
                    },
                )
                .expect("the embedded launch callback must open an actual Window");
            opened.set(Some(handle));
        }
    });
    assert_eq!(launches.get(), 1);
    let handle = opened
        .take()
        .expect("the visual launch callback must run before returning");
    let registration = platform
        .raw_window_handles
        .read()
        .iter()
        .copied()
        .find(|entry| entry.window_id() == handle.window_id())
        .expect("the embedded window must register its exact HWND");
    let native_window = platform
        .window_from_hwnd(registration.as_raw())
        .expect("the embedded HWND must have a real native owner");
    assert_eq!(native_window.registration, registration);
    let native_lifetime = Rc::downgrade(&native_window);
    let frame_requests = observe_frame_requests(&native_window);
    let (root, model, window_text, global_lifetime, asset_lifetime) = app.update(|cx| {
        let root = handle.root(cx).expect("the embedded root must remain live");
        let model = root.read(cx).model.clone();
        let window_text = handle
            .update(cx, |_, window, _| Rc::downgrade(window.text_system()))
            .expect("the embedded window must own its text cache");
        (
            root,
            model,
            window_text,
            Arc::downgrade(cx.text_system()),
            Arc::downgrade(cx.asset_source()),
        )
    });
    let root_lifetime = root.downgrade();
    let model_lifetime = model.downgrade();
    drop(root);
    let notifications = Rc::new(Cell::new(0));
    let observer = app.update(|cx| {
        let notifications = notifications.clone();
        cx.observe(&model, move |_, _| {
            notifications.set(notifications.get() + 1)
        })
    });
    platform
        .begin_vsync_thread()
        .expect("native vsync provider should start");
    let initial = wait_for_embedded_submission(&platform, &app, &visual, handle, 0, 0);
    let requests_before = frame_requests.get();
    let deferred = Rc::new(Cell::new(0));
    let returned = app.update(|cx| {
        model.update(cx, |model, cx| {
            model.version = 1;
            cx.notify();
        });
        let model = model.clone();
        let deferred = deferred.clone();
        cx.defer(move |cx| {
            assert_eq!(model.read(cx).version, 1);
            deferred.set(deferred.get() + 1);
        });
        String::from("external update returned its owned value")
    });
    assert_eq!(returned, "external update returned its owned value");
    assert_eq!(
        notifications.get(),
        1,
        "handle.update must flush model notification"
    );
    assert_eq!(
        deferred.get(),
        1,
        "handle.update must flush its deferred effect"
    );
    let notified = wait_for_embedded_submission(
        &platform,
        &app,
        &visual,
        handle,
        1,
        initial.metadata.framework_frame_generation,
    );
    assert!(frame_requests.get() > requests_before);
    assert_ne!(initial.image, notified.image);

    let input_calls = Rc::new(Cell::new(0));
    let consume_input = Rc::new(Cell::new(false));
    let interceptor = app
        .update(|cx| {
            handle.update(cx, |_, window, _| {
                let input_calls = input_calls.clone();
                let consume_input = consume_input.clone();
                window.intercept_window_mouse_events(move |event, window, cx| {
                    if matches!(event, WindowMouseEvent::Move(_)) {
                        input_calls.set(input_calls.get() + 1);
                        if consume_input.get() {
                            cx.stop_propagation();
                            window.prevent_default();
                        }
                    }
                })
            })
        })
        .expect("install the embedded window's native input interceptor");
    let diagnostic_cursor = app.update(|cx| {
        cx.native_boundary_diagnostics(NativeBoundaryDiagnosticCursor::default())
            .cursor
    });
    let calls_before = input_calls.get();
    let requests_before_borrow = frame_requests.get();
    app.update(|cx| {
        let panic = catch_unwind(AssertUnwindSafe(|| {
            native_window
                .state
                .callbacks
                .input
                .dispatch(PlatformInput::MouseMove(MouseMoveEvent {
                    position: point(px(12.), px(18.)),
                    pressed_button: None,
                    modifiers: Modifiers::default(),
                }))
        }))
        .expect_err("native input must preserve the active App borrow boundary");
        let violation = panic
            .downcast_ref::<NativeInputInvariantViolation>()
            .expect("busy native input must preserve its typed AppCell invariant");
        assert_eq!(violation.window_id, handle.window_id());
        assert_eq!(violation.failure, NativeInvariantFailure::AppBorrowBusy);
        assert!(violation.slot_generation.is_some());
        assert_eq!(input_calls.get(), calls_before);
        assert!(unsafe {
            RedrawWindow(Some(registration.as_raw()), None, None, RDW_INVALIDATE).as_bool()
        });
        assert_eq!(
            unsafe {
                SendMessageW(
                    registration.as_raw(),
                    WM_PAINT,
                    Some(WPARAM::default()),
                    Some(LPARAM::default()),
                )
            },
            LRESULT(0)
        );
        assert!(frame_requests.get() > requests_before_borrow);
        assert!(
            cx.native_boundary_diagnostics(diagnostic_cursor)
                .pending
                .iter()
                .any(|diagnostic| {
                    diagnostic.target == NativeBoundaryTarget::Window(handle.window_id())
                        && diagnostic.kind
                            == NativeBoundaryKind::Callback(NativeCallbackKind::RequestFrame)
                })
        );
    });
    for (consume, position, result) in [(false, (24, 30), 1), (true, (36, 42), 0)] {
        consume_input.set(consume);
        let before = input_calls.get();
        assert_eq!(
            unsafe {
                SendMessageW(
                    registration.as_raw(),
                    WM_MOUSEMOVE,
                    Some(WPARAM::default()),
                    Some(mouse_position_lparam(position.0, position.1)),
                )
            },
            LRESULT(result)
        );
        assert_eq!(input_calls.get(), before + 1);
    }
    let diagnostics = app.update(|cx| cx.native_boundary_diagnostics(diagnostic_cursor));
    assert_eq!(diagnostics.omitted_before_cursor, 0);
    let exact = diagnostics
        .terminal
        .iter()
        .filter(|diagnostic| diagnostic.target == NativeBoundaryTarget::Window(handle.window_id()));
    assert_eq!(
        exact
            .clone()
            .filter(|diagnostic| matches!(
                diagnostic.disposition,
                NativeBoundaryDisposition::InvariantFailure(NativeInvariantFailure::AppBorrowBusy)
            ))
            .count(),
        1
    );
    assert!(exact.clone().any(|diagnostic| {
        diagnostic.kind == NativeBoundaryKind::Callback(NativeCallbackKind::RequestFrame)
            && matches!(
                diagnostic.disposition,
                NativeBoundaryDisposition::Delivered { .. }
            )
    }));
    for propagate in [true, false] {
        assert!(exact.clone().any(|diagnostic| {
            diagnostic.kind == NativeBoundaryKind::Callback(NativeCallbackKind::PlatformInput)
                && matches!(
                    diagnostic.disposition,
                    NativeBoundaryDisposition::Delivered { input_result: Some(result) }
                        if result.propagate == propagate && result.default_prevented == !propagate
                )
        }));
    }
    assert!(diagnostics.terminal.iter().all(|diagnostic| !matches!(
        diagnostic.disposition,
        NativeBoundaryDisposition::InvariantFailure(failure)
            if failure != NativeInvariantFailure::AppBorrowBusy
    )));
    drop(interceptor);

    let mut async_cx: AsyncApp = app.to_async();
    let stage = Rc::new(Cell::new(0));
    let async_deferred = Rc::new(Cell::new(0));
    let task = async_cx.spawn({
        let stage = stage.clone();
        let model = model.downgrade();
        let async_deferred = async_deferred.clone();
        async move |cx| {
            stage.set(1);
            let payload = cx
                .background_executor()
                .spawn(async { String::from("visual background result") })
                .await;
            assert_eq!(payload, "visual background result");
            stage.set(2);
            cx.background_executor().timer(EMBEDDED_TIMER).await;
            cx.update(|cx| {
                model
                    .upgrade()
                    .expect("the async model must remain live")
                    .update(cx, |model, cx| {
                        model.version = 2;
                        cx.notify();
                    });
                cx.defer(move |_| async_deferred.set(async_deferred.get() + 1));
            });
            stage.set(3);
        }
    });
    assert_eq!(stage.get(), 0, "the external host owns visual task polling");
    pump_embedded_until(
        &platform,
        &visual,
        "embedded async background await and armed timer",
        || stage.get() == 2,
    );
    assert_eq!(app.update(|cx| model.read(cx).version), 1);
    assert_eq!(notifications.get(), 1);
    assert_eq!(async_deferred.get(), 0);
    visual
        .dispatcher()
        .advance_clock(EMBEDDED_TIMER - Duration::from_millis(1));
    poll_embedded_tasks(&visual);
    assert_eq!(
        stage.get(),
        2,
        "the visual timer must not complete before its deadline"
    );
    visual.dispatcher().advance_clock(Duration::from_millis(1));
    pump_embedded_until(
        &platform,
        &visual,
        "embedded async context resumes after visual timer",
        || stage.get() == 3,
    );
    assert_eq!(notifications.get(), 2);
    assert_eq!(async_deferred.get(), 1);
    let timed = wait_for_embedded_submission(
        &platform,
        &app,
        &visual,
        handle,
        2,
        notified.metadata.framework_frame_generation,
    );
    assert_ne!(notified.image, timed.image);
    app.update(|cx| {
        model.update(cx, |model, cx| {
            assert_eq!(model.version, 2);
            model.version = 3;
            cx.notify();
        });
    });
    assert_eq!(notifications.get(), 3);
    let resumed = wait_for_embedded_submission(
        &platform,
        &app,
        &visual,
        handle,
        3,
        timed.metadata.framework_frame_generation,
    );
    assert_ne!(timed.image, resumed.image);
    assert_eq!(
        launches.get(),
        1,
        "host pumping must not launch a second App"
    );
    drop(task);
    drop(observer);
    drop(model);

    let shutdowns = Rc::new(Cell::new(0));
    let shutdown_observer = app.update(|cx| {
        let shutdowns = shutdowns.clone();
        cx.on_app_quit(move |_| {
            shutdowns.set(shutdowns.get() + 1);
            async {}
        })
    });
    app.update(|cx| cx.shutdown());
    pump_embedded_until(
        &platform,
        &visual,
        "embedded shutdown and exact native window retirement",
        || {
            app.update(|cx| cx.windows().is_empty())
                && !unsafe { IsWindow(Some(registration.as_raw())).as_bool() }
                && platform.raw_window_handles.read().is_empty()
                && platform.inner.native_retirement_is_settled()
                && root_lifetime.upgrade().is_none()
                && model_lifetime.upgrade().is_none()
                && window_text.upgrade().is_none()
        },
    );
    assert_eq!(shutdowns.get(), 1);
    assert!(app.update(|cx| handle.update(cx, |_, _, _| ()).is_err()));
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
        panic!("unexpected embedded window retirement sequence: {lifecycle:?}");
    };
    assert_eq!(quiesced, destroyed);
    assert_eq!(destroyed, terminal);
    drop(native_window);
    assert!(native_lifetime.upgrade().is_none());
    drop(shutdown_observer);
    drop(app);
    drop(visual);
    let pump_platform = Rc::downgrade(&platform.inner);
    drop(platform);
    pump_messages_until_with_delay(
        &pump_platform,
        "embedded platform HWND, assets, global text, backend and OLE release",
        Duration::from_millis(5),
        || {
            !unsafe { IsWindow(Some(platform_hwnd)).as_bool() }
                && platform_lifetime.upgrade().is_none()
                && visual_lifetime.upgrade().is_none()
                && asset_lifetime.upgrade().is_none()
                && global_lifetime.upgrade().is_none()
                && backend_lifetime.upgrade().is_none()
        },
    );
    assert_eq!(lifecycle_probe.platform_destroy_attempts(), 1);
    assert_eq!(lifecycle_probe.ole_uninitialize_count(), 1);
    let released = async_cx.update_window(handle.into(), |_, _, _| ());
    assert_eq!(
        released
            .expect_err("AsyncApp must not retain the App")
            .to_string(),
        "app was released"
    );
}
