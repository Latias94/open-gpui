use super::*;
use open_gpui::{IndentAdjustment, LineFragment, font};

struct WrappedNativeText;

impl Render for WrappedNativeText {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().w(px(64.)).child("hello world 你好 ééé")
    }
}

#[test]
fn native_app_reconstruction_releases_wrapped_text_system_and_direct_write() {
    for _ in 0..3 {
        discard_stale_quit_messages();
        let platform = Rc::new(
            WindowsPlatform::new(false).expect("the native text platform should initialize"),
        );
        let platform_text_system = platform.text_system();
        let weak_platform_text_system = Arc::downgrade(&platform_text_system);
        let mut app =
            Application::with_platform(platform.clone()).with_quit_mode(QuitMode::Explicit);
        let text_system = app.text_system();
        let weak_text_system = Arc::downgrade(&text_system);
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
        pump_messages_until_idle("native wrapped text initial presentation");
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
        pump_messages_until("native wrapped text terminal cleanup", || {
            !unsafe { IsWindow(Some(hwnd)).as_bool() } && !is_registered(&platform, hwnd)
        });
        app.update_for_test(|cx| cx.shutdown());
        drop(text_system);
        drop(app);
        drop(platform_text_system);
        drop(platform);
        assert!(weak_text_system.upgrade().is_none());
        assert!(weak_platform_text_system.upgrade().is_none());
    }
}
