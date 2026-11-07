use super::*;
use crate::native_test_observation::NativeWindowTestMessageDisposition;
use open_gpui::{
    App, FocusHandle, InputHandler, InteractiveElement, Pixels, Point, UTF16Selection,
};
use std::ops::Range;
use windows::Win32::{Graphics::Gdi::GetUpdateRect, UI::WindowsAndMessaging::WM_CHAR};

#[derive(Clone, Default)]
struct NativePaintInput {
    accepts_text: Rc<Cell<bool>>,
    queries: Rc<Cell<usize>>,
    text: Rc<RefCell<String>>,
    outer_draw_active: Rc<Cell<bool>>,
}

impl InputHandler for NativePaintInput {
    fn selected_text_range(
        &mut self,
        _: bool,
        _: &mut Window,
        _: &mut App,
    ) -> Option<UTF16Selection> {
        Some(UTF16Selection {
            range: 0..0,
            reversed: false,
        })
    }

    fn marked_text_range(&mut self, _: &mut Window, _: &mut App) -> Option<Range<usize>> {
        None
    }

    fn text_for_range(
        &mut self,
        _: Range<usize>,
        _: &mut Option<Range<usize>>,
        _: &mut Window,
        _: &mut App,
    ) -> Option<String> {
        Some(self.text.borrow().clone())
    }

    fn replace_text_in_range(
        &mut self,
        _: Option<Range<usize>>,
        text: &str,
        _: &mut Window,
        _: &mut App,
    ) {
        self.text.borrow_mut().push_str(text);
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        _: Option<Range<usize>>,
        text: &str,
        _: Option<Range<usize>>,
        _: &mut Window,
        _: &mut App,
    ) {
        self.text.borrow_mut().push_str(text);
    }

    fn unmark_text(&mut self, _: &mut Window, _: &mut App) {}

    fn bounds_for_range(
        &mut self,
        _: Range<usize>,
        _: &mut Window,
        _: &mut App,
    ) -> Option<Bounds<Pixels>> {
        Some(Bounds::new(
            point(px(0.0), px(0.0)),
            size(px(1.0), px(16.0)),
        ))
    }

    fn character_index_for_point(
        &mut self,
        _: Point<Pixels>,
        _: &mut Window,
        _: &mut App,
    ) -> Option<usize> {
        Some(0)
    }

    fn accepts_text_input(&mut self, _: &mut Window, _: &mut App) -> bool {
        assert!(
            !self.outer_draw_active.get(),
            "IME maintenance must wait until the enclosing window draw has returned"
        );
        self.queries.set(self.queries.get() + 1);
        self.accepts_text.get()
    }
}

struct NativeTextRoot {
    focus: FocusHandle,
    input: NativePaintInput,
}

impl Render for NativeTextRoot {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let focus = self.focus.clone();
        let input = self.input.clone();
        div()
            .id("native-paint-text-handler")
            .size_full()
            .bg(white())
            .track_focus(&self.focus)
            .child(
                canvas(
                    |_, _, _| {},
                    move |_, _, window, cx| window.handle_input(&focus, input, cx),
                )
                .size_full(),
            )
    }
}

struct PaintOtherWindowRoot {
    target: Rc<Cell<Option<HWND>>>,
    fired: Rc<Cell<bool>>,
    input: NativePaintInput,
}

impl Render for PaintOtherWindowRoot {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let target = self.target.clone();
        let fired = self.fired.clone();
        let input = self.input.clone();
        div().size_full().bg(white()).child(
            canvas(
                |_, _, _| {},
                move |_, _, _, _| {
                    let Some(hwnd) = target.take() else {
                        return;
                    };
                    let queries_before = input.queries.get();
                    input.outer_draw_active.set(true);
                    for _ in 0..2 {
                        assert!(unsafe {
                            RedrawWindow(Some(hwnd), None, None, RDW_INVALIDATE).as_bool()
                        });
                        assert!(unsafe { GetUpdateRect(hwnd, None, false).as_bool() });
                        assert_eq!(
                            unsafe {
                                SendMessageW(
                                    hwnd,
                                    WM_PAINT,
                                    Some(WPARAM::default()),
                                    Some(LPARAM::default()),
                                )
                            },
                            LRESULT(0)
                        );
                        assert!(
                            unsafe { !GetUpdateRect(hwnd, None, false).as_bool() },
                            "a nested WM_PAINT must validate its update region before returning"
                        );
                        assert_eq!(
                            input.queries.get(),
                            queries_before,
                            "native paint must retain IME maintenance without querying the busy App"
                        );
                    }
                    input.outer_draw_active.set(false);
                    fired.set(true);
                },
            )
            .size_full(),
        )
    }
}

#[test]
fn two_hwnd_reentrant_paint_defers_ime_until_native_frame_command() {
    discard_stale_quit_messages();
    let (_observation_guard, observation) = begin_native_window_test_observation();
    let platform = Rc::new(
        WindowsPlatform::new(false).expect("non-headless Windows platform should initialize"),
    );
    let mut app = Application::with_platform(platform.clone()).with_quit_mode(QuitMode::Explicit);
    let input = NativePaintInput::default();
    input.accepts_text.set(true);
    let target = app
        .update_for_test(|cx| {
            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::centered(size(px(320.0), px(220.0)), cx)),
                    focus_on_appearing: false,
                    inactive_frame_interval: None,
                    low_power_frame_interval: None,
                    thermal_frame_interval: None,
                    ..WindowOptions::default()
                },
                |window, cx| {
                    let focus = cx.focus_handle();
                    focus.focus(window, cx);
                    cx.new(|_| NativeTextRoot {
                        focus,
                        input: input.clone(),
                    })
                },
            )
        })
        .expect("the text-handler target should open");
    let target_hwnd = platform
        .lifecycle_test_probe
        .last_created_hwnd()
        .expect("the text-handler target must have an HWND");
    let target_native = platform
        .window_from_hwnd(target_hwnd)
        .expect("the text-handler target must remain registered");
    let nested_target = Rc::new(Cell::new(None));
    let fired = Rc::new(Cell::new(false));
    let source = app
        .update_for_test(|cx| {
            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::centered(size(px(240.0), px(160.0)), cx)),
                    focus_on_appearing: false,
                    inactive_frame_interval: None,
                    low_power_frame_interval: None,
                    thermal_frame_interval: None,
                    ..WindowOptions::default()
                },
                |_, cx| {
                    cx.new(|_| PaintOtherWindowRoot {
                        target: nested_target.clone(),
                        fired: fired.clone(),
                        input: input.clone(),
                    })
                },
            )
        })
        .expect("the enclosing paint window should open");
    let source_hwnd = platform
        .lifecycle_test_probe
        .last_created_hwnd()
        .expect("the enclosing paint window must have an HWND");
    assert_ne!(source_hwnd, target_hwnd);
    pump_messages_until_idle("initial two-window paint messages");
    assert!(
        input.queries.get() > 0,
        "the committed target must own a text handler"
    );
    assert!(target_native.state.ime_enabled.get());

    let queries_before = input.queries.get();
    let diagnostic_cursor = app.update_for_test(|cx| {
        cx.native_boundary_diagnostics(NativeBoundaryDiagnosticCursor::default())
            .cursor
    });
    let observation_cursor = observation.cursor();
    input.accepts_text.set(false);
    nested_target.set(Some(target_hwnd));
    app.update_for_test(|cx| source.update(cx, |_, window, _| window.refresh()))
        .expect("the source should refresh");
    unsafe {
        SendMessageW(
            source_hwnd,
            WM_PAINT,
            Some(WPARAM::default()),
            Some(LPARAM::default()),
        );
    }
    pump_messages_until("deferred native IME maintenance", || {
        fired.get()
            && input.queries.get() > queries_before
            && !target_native.state.ime_enabled.get()
    });

    let diagnostic_delta =
        app.update_for_test(|cx| cx.native_boundary_diagnostics(diagnostic_cursor));
    let target_id = NativeBoundaryTarget::Window(target.window_id());
    assert!(diagnostic_delta.terminal.iter().any(|diagnostic| {
        diagnostic.target == target_id
            && diagnostic.kind == NativeBoundaryKind::Callback(NativeCallbackKind::RequestFrame)
            && matches!(
                diagnostic.disposition,
                NativeBoundaryDisposition::Coalesced { .. }
            )
    }));
    assert!(diagnostic_delta.terminal.iter().any(|diagnostic| {
        diagnostic.target == target_id
            && diagnostic.kind
                == NativeBoundaryKind::Command(NativePlatformCommandKind::CompleteNativeFrame)
            && diagnostic.disposition == NativeBoundaryDisposition::Delivered { input_result: None }
    }));
    assert!(diagnostic_delta.terminal.iter().all(|diagnostic| {
        !matches!(
            diagnostic.disposition,
            NativeBoundaryDisposition::InvariantFailure(_)
        )
    }));
    let paint_dispositions = observation
        .events_after(observation_cursor)
        .into_iter()
        .filter(|event| {
            event.window().window_id() == target.window_id()
                && event.window().native_generation() == target_native.registration.generation()
        })
        .filter_map(|event| match event.kind() {
            NativeWindowTestEventKind::WindowMessage {
                message: NativeWindowTestMessage::Other(WM_PAINT),
                disposition,
                ..
            } => Some(disposition),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert!(paint_dispositions.len() >= 2);
    assert!(
        paint_dispositions
            .iter()
            .all(|disposition| { *disposition == NativeWindowTestMessageDisposition::Returned(0) }),
        "the native paint boundary must return without catching an invariant panic"
    );

    input.accepts_text.set(true);
    unsafe {
        SendMessageW(
            target_hwnd,
            WM_PAINT,
            Some(WPARAM::default()),
            Some(LPARAM::default()),
        );
    }
    pump_messages_until("restored native text-input admission", || {
        target_native.state.ime_enabled.get()
    });
    unsafe {
        SendMessageW(
            target_hwnd,
            WM_CHAR,
            Some(WPARAM('z' as usize)),
            Some(LPARAM(1)),
        );
    }
    assert_eq!(input.text.borrow().as_str(), "z");

    app.update_for_test(|cx| {
        source
            .update(cx, |_, window, cx| window.remove_window(cx))
            .expect("the enclosing paint window should close");
        target
            .update(cx, |_, window, cx| window.remove_window(cx))
            .expect("the text-handler target should close");
    });
    pump_messages_until("reentrant native paint teardown", || {
        !unsafe { IsWindow(Some(source_hwnd)).as_bool() || IsWindow(Some(target_hwnd)).as_bool() }
            && !is_registered(&platform, source_hwnd)
            && !is_registered(&platform, target_hwnd)
    });
}
