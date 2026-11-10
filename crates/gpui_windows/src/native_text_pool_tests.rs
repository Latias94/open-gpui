use super::*;
use open_gpui::{
    IndentAdjustment, LineFragment, TextRun, TextSystem, WindowHandle,
    WindowInitialPresentationStatus, WindowTextSystem, font,
};

struct WrappedNativeText;

impl Render for WrappedNativeText {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let owner: &Rc<WindowTextSystem> = window.text_system();
        let global: &Arc<TextSystem> = owner.as_ref();
        assert!(Arc::ptr_eq(global, cx.text_system()));
        div().w(px(64.)).child("hello world 你好 ééé")
    }
}

fn wait_for_native_text_submission(
    platform: &WindowsPlatform,
    app: &mut Application,
    handle: WindowHandle<WrappedNativeText>,
) {
    pump_messages_until_with_delay(
        &Rc::downgrade(&platform.inner),
        "native wrapped text accepted submission",
        Duration::from_millis(5),
        || {
            let (facts, revision, diagnostics) = app
                .update_for_test(|cx| {
                    handle.update(cx, |_, window, _| {
                        (
                            window.presentation_facts(),
                            window.rendered_frame_revision(),
                            window
                                .submitted_frame_diagnostics_for_test()
                                .map(|diagnostics| diagnostics.generation()),
                        )
                    })
                })
                .expect("the native text window must remain open until submission");
            let Some(attempt) = facts.latest_present_attempt else {
                return false;
            };
            attempt.generation > 0
                && attempt.outcome == PlatformWindowPresentOutcome::Submitted
                && attempt.contained_valid_primitives
                && facts.initial_presentation == WindowInitialPresentationStatus::Completed
                && facts.native_visible
                && facts.frame_accepted_generation == Some(attempt.generation)
                && facts.present_submitted_generation == Some(attempt.generation)
                && facts.non_empty_presented_generation == Some(attempt.generation)
                && revision == attempt.generation
                && diagnostics == Some(attempt.generation)
        },
    );
}

#[test]
fn native_app_reconstruction_releases_wrapped_text_system_and_direct_write() {
    for _ in 0..3 {
        discard_stale_quit_messages();
        let platform = Rc::new(
            WindowsPlatform::new_windowed_for_test()
                .expect("the native text platform should initialize"),
        );
        let platform_hwnd = platform.handle;
        let lifecycle_probe = platform.lifecycle_test_probe.clone();
        let platform_text_system = platform.text_system();
        let weak_platform_text_system = Arc::downgrade(&platform_text_system);
        let mut app =
            Application::with_platform(platform.clone()).with_quit_mode(QuitMode::Explicit);
        let text_system = app.text_system();
        let weak_text_system = Arc::downgrade(&text_system);
        app.update_for_test(|cx| assert!(Arc::ptr_eq(cx.text_system(), &text_system)));
        let fragments = [LineFragment::text("hello world 你好 ééé")];
        let expected = {
            let mut wrapper = text_system.line_wrapper(font("Segoe UI"), px(16.));
            wrapper
                .wrap_line(&fragments, px(64.), IndentAdjustment::default())
                .collect::<Vec<_>>()
        };
        assert!(!expected.is_empty());
        let window = app
            .update_for_test(|cx| {
                cx.open_window(
                    WindowOptions {
                        window_bounds: Some(WindowBounds::centered(size(px(240.), px(160.)), cx)),
                        focus_on_appearing: false,
                        ..WindowOptions::default()
                    },
                    |_, cx| cx.new(|_| WrappedNativeText),
                )
            })
            .expect("the native wrapped text window should open");
        let hwnd = platform
            .lifecycle_test_probe
            .last_created_hwnd()
            .expect("the native wrapped text window must own an HWND");
        pump_messages_until_idle(
            &Rc::downgrade(&platform.inner),
            "native wrapped text initial presentation",
        );
        platform
            .begin_vsync_thread()
            .expect("native vsync provider should start");
        wait_for_native_text_submission(&platform, &mut app, window);
        let (window_text, retained_line) = app.update_for_test(|cx| {
            window
                .update(cx, |_, window, cx| {
                    let owner: &Rc<WindowTextSystem> = window.text_system();
                    let global: &Arc<TextSystem> = owner.as_ref();
                    assert!(Arc::ptr_eq(global, &text_system));
                    assert!(Arc::ptr_eq(global, cx.text_system()));
                    let source = "hello world 你好 ééé";
                    let line = owner.shape_line(
                        source.into(),
                        px(16.),
                        &[TextRun {
                            len: source.len(),
                            font: font("Segoe UI"),
                            ..Default::default()
                        }],
                        None,
                    );
                    (owner.clone(), line)
                })
                .expect("the native window must expose its own Rc text cache")
        });
        let weak_window_text = Rc::downgrade(&window_text);
        let retained_indices = retained_line
            .runs
            .iter()
            .flat_map(|run| run.glyphs.iter().map(|glyph| glyph.index))
            .collect::<Vec<_>>();
        assert!(!retained_indices.is_empty());
        {
            let mut wrapper = text_system.line_wrapper(font("Segoe UI"), px(16.));
            assert_eq!(
                wrapper
                    .wrap_line(&fragments, px(64.), IndentAdjustment::default())
                    .collect::<Vec<_>>(),
                expected
            );
        }
        app.update_for_test(|cx| {
            window
                .update(cx, |_, window, cx| window.remove_window(cx))
                .expect("the native wrapped text window should close");
        });
        pump_messages_until(
            &Rc::downgrade(&platform.inner),
            "native wrapped text terminal cleanup",
            || !unsafe { IsWindow(Some(hwnd)).as_bool() } && !is_registered(&platform, hwnd),
        );
        pump_messages_until_with_delay(
            &Rc::downgrade(&platform.inner),
            "native wrapped text retirement settlement",
            Duration::from_millis(5),
            || platform.inner.native_retirement_is_settled(),
        );
        assert!(weak_window_text.upgrade().is_some());
        assert!(app.update_for_test(|cx| cx.windows().is_empty()));
        let reconstructed = app
            .update_for_test(|cx| {
                assert!(Arc::ptr_eq(cx.text_system(), &text_system));
                cx.open_window(
                    WindowOptions {
                        window_bounds: Some(WindowBounds::centered(size(px(240.), px(160.)), cx)),
                        focus_on_appearing: false,
                        ..WindowOptions::default()
                    },
                    |_, cx| cx.new(|_| WrappedNativeText),
                )
            })
            .expect("a reconstructed native wrapped text window should open");
        assert_ne!(window.window_id(), reconstructed.window_id());
        let reconstructed_hwnd = platform
            .lifecycle_test_probe
            .last_created_hwnd()
            .expect("the reconstructed native text window must own an HWND");
        wait_for_native_text_submission(&platform, &mut app, reconstructed);
        let reconstructed_text = app.update_for_test(|cx| {
            reconstructed
                .update(cx, |_, window, cx| {
                    let owner: &Rc<WindowTextSystem> = window.text_system();
                    let global: &Arc<TextSystem> = owner.as_ref();
                    let previous_global: &Arc<TextSystem> = window_text.as_ref();
                    assert!(Arc::ptr_eq(global, previous_global));
                    assert!(Arc::ptr_eq(global, &text_system));
                    assert!(Arc::ptr_eq(global, cx.text_system()));
                    owner.clone()
                })
                .expect("the reconstructed native window must expose its own cache")
        });
        assert!(!Rc::ptr_eq(&window_text, &reconstructed_text));
        let weak_reconstructed_text = Rc::downgrade(&reconstructed_text);
        {
            let mut wrapper = text_system.line_wrapper(font("Segoe UI"), px(16.));
            assert_eq!(
                wrapper
                    .wrap_line(&fragments, px(64.), IndentAdjustment::default())
                    .collect::<Vec<_>>(),
                expected
            );
        }
        app.update_for_test(|cx| {
            reconstructed
                .update(cx, |_, window, cx| window.remove_window(cx))
                .expect("the reconstructed native wrapped text window should close");
        });
        pump_messages_until_with_delay(
            &Rc::downgrade(&platform.inner),
            "reconstructed native wrapped text terminal cleanup",
            Duration::from_millis(5),
            || {
                !unsafe { IsWindow(Some(reconstructed_hwnd)).as_bool() }
                    && !is_registered(&platform, reconstructed_hwnd)
                    && platform.inner.native_retirement_is_settled()
            },
        );
        assert!(app.update_for_test(|cx| cx.windows().is_empty()));
        assert!(platform.raw_window_handles.read().is_empty());
        drop(reconstructed_text);
        assert!(weak_reconstructed_text.upgrade().is_none());
        assert!(weak_window_text.upgrade().is_some());
        app.update_for_test(|cx| cx.shutdown());
        drop(text_system);
        drop(app);
        drop(platform_text_system);
        let pump_platform = Rc::downgrade(&platform.inner);
        drop(platform);
        pump_messages_until_with_delay(
            &pump_platform,
            "native wrapped text platform message window retirement",
            Duration::from_millis(5),
            || unsafe { !IsWindow(Some(platform_hwnd)).as_bool() },
        );
        assert_eq!(lifecycle_probe.ole_uninitialize_count(), 1);
        assert!(weak_text_system.upgrade().is_some());
        assert!(weak_platform_text_system.upgrade().is_some());
        drop(window_text);
        assert!(weak_window_text.upgrade().is_none());
        assert!(weak_text_system.upgrade().is_none());
        assert!(weak_platform_text_system.upgrade().is_none());
        assert_eq!(retained_line.text.as_ref(), "hello world 你好 ééé");
        assert!(retained_line.width > px(0.));
        assert_eq!(
            retained_line
                .runs
                .iter()
                .flat_map(|run| run.glyphs.iter().map(|glyph| glyph.index))
                .collect::<Vec<_>>(),
            retained_indices
        );
    }
}
