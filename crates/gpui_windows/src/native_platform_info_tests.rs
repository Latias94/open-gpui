//! Bounded Windows consumer of runtime OS contracts from Zed
//! 389c22e1c747d1a433c3b51c176936166c9cc1cf at fixed target
//! 635026fabc38440fce5e1767d125026a17b12012.
//!
//! Synthetic keys enter the registered native input callback; WM_CHAR enters the
//! installed text-handler slot through the exact HWND's WndProc. DirectWrite labels
//! and text belong to accepted scenes with nonempty DirectX Submitted records. Capture
//! re-renders that scene for GPU readback; it does not read an earlier swapchain buffer.
//! This does not qualify physical keyboard translation, foreground acquisition,
//! live IME composition, browser host detection or compositor scanout.

use super::*;
use crate::native_test_observation::NativeWindowTestMessageDisposition;
use open_gpui::{
    Action, App, FocusHandle, InputHandler, InteractiveElement as _, KeyBinding,
    KeyBindingContextPredicate, KeyContext, KeyDownEvent, Keystroke, Modifiers,
    NativeInputHandlerOperation, OperatingSystem, Pixels, PlatformInfo, PlatformTarget, Point,
    ShapedLine, Subscription, TextAlign, TextRun, UTF16Selection, WeakEntity, WindowFrameCapture,
    WindowHandle, WindowInitialPresentationStatus, actions, black, fill, font, rgb,
};
use std::ops::Range;
use windows::Win32::UI::WindowsAndMessaging::WM_CHAR;

actions!(
    native_platform_consumer,
    [Save, Chord, Control, PlatformKey, Contextual, Prefix]
);

const WINDOWS_INFO: PlatformInfo = PlatformInfo {
    os: OperatingSystem::Windows,
    target: PlatformTarget::Native,
};
const LABELS: [&str; 4] = ["ctrl-S", "ctrl-K ctrl-C", "ctrl-f6", "⊞f7"];

#[derive(Default)]
struct TextState {
    text: RefCell<String>,
    created: Cell<usize>,
    retired: Cell<usize>,
}

struct TextHandler(Rc<TextState>);

impl TextHandler {
    fn new(state: Rc<TextState>) -> Self {
        state.created.set(state.created.get() + 1);
        Self(state)
    }
}

impl Drop for TextHandler {
    fn drop(&mut self) {
        self.0.retired.set(self.0.retired.get() + 1);
    }
}

impl InputHandler for TextHandler {
    fn selected_text_range(
        &mut self,
        _: bool,
        _: &mut Window,
        _: &mut App,
    ) -> Option<UTF16Selection> {
        let end = self.0.text.borrow().encode_utf16().count();
        Some(UTF16Selection {
            range: end..end,
            reversed: false,
        })
    }

    fn marked_text_range(&mut self, _: &mut Window, _: &mut App) -> Option<Range<usize>> {
        None
    }

    fn text_for_range(
        &mut self,
        range: Range<usize>,
        _: &mut Option<Range<usize>>,
        _: &mut Window,
        _: &mut App,
    ) -> Option<String> {
        let text = self.0.text.borrow().encode_utf16().collect::<Vec<_>>();
        String::from_utf16(text.get(range)?).ok()
    }

    fn replace_text_in_range(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        window: &mut Window,
        _: &mut App,
    ) {
        assert!(
            range.is_none(),
            "this native consumer appends committed text"
        );
        self.0.text.borrow_mut().push_str(text);
        window.refresh();
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        _: Option<Range<usize>>,
        _: &str,
        _: Option<Range<usize>>,
        _: &mut Window,
        _: &mut App,
    ) {
        panic!("live IME composition is outside this consumer's scope");
    }

    fn unmark_text(&mut self, _: &mut Window, _: &mut App) {}

    fn bounds_for_range(
        &mut self,
        _: Range<usize>,
        _: &mut Window,
        _: &mut App,
    ) -> Option<Bounds<Pixels>> {
        None
    }

    fn character_index_for_point(
        &mut self,
        _: Point<Pixels>,
        _: &mut Window,
        _: &mut App,
    ) -> Option<usize> {
        None
    }

    fn prefers_ime_for_printable_keys(&mut self, _: &mut Window, _: &mut App) -> bool {
        true
    }
}

#[derive(Default)]
struct Trace {
    calls: RefCell<Vec<(&'static str, usize)>>,
    pending: RefCell<Vec<bool>>,
    drops: Cell<usize>,
    releases: Cell<usize>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Source {
    labels: Vec<String>,
    text: [String; 2],
    calls: Vec<(&'static str, usize)>,
    focus: Option<usize>,
}

struct Paint {
    source: Source,
    previous_generation: u64,
    lines: Vec<ShapedLine>,
    bounds: Bounds<Pixels>,
}

struct NativePlatformRoot {
    focus: [FocusHandle; 2],
    inputs: [Rc<TextState>; 2],
    trace: Rc<Trace>,
    paint: Rc<RefCell<Option<Paint>>>,
    _pending: Subscription,
}

impl NativePlatformRoot {
    fn source(&self, window: &Window, app: &App) -> Source {
        assert_eq!(app.platform_info(), WINDOWS_INFO);
        Source {
            labels: [&Save as &dyn Action, &Chord, &Control, &PlatformKey]
                .map(|action| window.keystroke_text_for(action, app))
                .to_vec(),
            text: self
                .inputs
                .each_ref()
                .map(|input| input.text.borrow().clone()),
            calls: self.trace.calls.borrow().clone(),
            focus: self.focus.iter().position(|focus| focus.is_focused(window)),
        }
    }

    fn record(&self, name: &'static str, window: &Window) {
        let focus = self
            .focus
            .iter()
            .position(|focus| focus.is_focused(window))
            .unwrap();
        self.trace.calls.borrow_mut().push((name, focus));
    }
}

impl Drop for NativePlatformRoot {
    fn drop(&mut self) {
        self.trace.drops.set(self.trace.drops.get() + 1);
    }
}

fn marker(source: &Source) -> u32 {
    let red = 32 + source.calls.len() as u32 * 16;
    let green = 32
        + source
            .text
            .iter()
            .map(|text| text.len() as u32)
            .sum::<u32>()
            * 16;
    (red << 16) | (green << 8) | (64 + source.focus.unwrap_or(2) as u32 * 32)
}

impl Render for NativePlatformRoot {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let mut context = KeyContext::new_with_defaults(cx.platform_info().os);
        context.add("NativePlatformConsumer");
        let mut source = self.source(window, cx);
        let labels = source.labels.clone();
        let text = source.text.clone();
        let paint = self.paint.clone();
        let paint_focus = self.focus.clone();
        div()
            .relative()
            .size_full()
            .bg(black())
            .key_context(context)
            .on_action(cx.listener(|root, _: &Save, window, cx| {
                root.record("save", window);
                cx.notify();
            }))
            .on_action(cx.listener(|root, _: &Chord, window, cx| {
                root.record("chord", window);
                cx.notify();
            }))
            .on_action(cx.listener(|root, _: &Control, window, cx| {
                root.record("control", window);
                cx.notify();
            }))
            .on_action(cx.listener(|root, _: &PlatformKey, window, cx| {
                root.record("platform", window);
                cx.notify();
            }))
            .on_action(cx.listener(|root, _: &Contextual, window, cx| {
                root.record("context", window);
                cx.notify();
            }))
            .on_action(cx.listener(|root, _: &Prefix, window, cx| {
                root.record("prefix", window);
                cx.notify();
            }))
            .child(
                canvas(
                    move |_, window, _| {
                        labels
                            .into_iter()
                            .chain([
                                format!("Primary: {}", text[0]),
                                format!("Secondary: {}", text[1]),
                            ])
                            .map(|text| {
                                let runs = [TextRun {
                                    len: text.len(),
                                    font: font("Segoe UI"),
                                    color: white(),
                                    ..Default::default()
                                }];
                                window
                                    .text_system()
                                    .shape_line(text.into(), px(16.), &runs, None)
                            })
                            .collect::<Vec<_>>()
                    },
                    move |bounds, lines, window, app| {
                        source.focus = paint_focus
                            .iter()
                            .position(|focus| focus.is_focused(window));
                        for (index, line) in lines.iter().enumerate() {
                            line.paint(
                                bounds.origin + point(px(16.), px(16. + index as f32 * 28.)),
                                px(24.),
                                TextAlign::Left,
                                None,
                                window,
                                app,
                            )
                            .expect("DirectWrite shortcut labels and input text must paint");
                        }
                        window.paint_quad(fill(
                            Bounds::new(
                                bounds.origin + point(px(16.), px(208.)),
                                size(px(256.), px(20.)),
                            ),
                            rgb(marker(&source)),
                        ));
                        *paint.borrow_mut() = Some(Paint {
                            source,
                            previous_generation: window.rendered_frame_revision(),
                            lines,
                            bounds,
                        });
                    },
                )
                .size_full(),
            )
            .children(self.focus.iter().enumerate().map(|(index, focus)| {
                let focus = focus.clone();
                let input = self.inputs[index].clone();
                div()
                    .id(("native-platform-field", index))
                    .absolute()
                    .left(px(16.))
                    .top(px(128. + index as f32 * 28.))
                    .w(px(360.))
                    .h(px(28.))
                    .track_focus(&focus)
                    .child(
                        canvas(
                            |_, _, _| {},
                            move |_, (), window, app| {
                                if focus.is_focused(window) {
                                    window.handle_input(&focus, TextHandler::new(input), app);
                                }
                            },
                        )
                        .size(px(1.)),
                    )
            }))
    }
}

struct NativePlatformWindow {
    handle: WindowHandle<NativePlatformRoot>,
    registration: RegisteredWindow,
    native: Rc<WindowsWindowInner>,
    root: WeakEntity<NativePlatformRoot>,
    inputs: [Rc<TextState>; 2],
    trace: Rc<Trace>,
    paint: Rc<RefCell<Option<Paint>>>,
}

impl NativePlatformWindow {
    fn open(app: &mut Application, platform: &WindowsPlatform) -> Self {
        let inputs = [Rc::new(TextState::default()), Rc::new(TextState::default())];
        let trace = Rc::new(Trace::default());
        let paint = Rc::new(RefCell::new(None));
        let handle = app
            .update_for_test(|app| {
                assert!(!app.is_test());
                assert!(!app.background_executor().is_test());
                assert_eq!(app.platform_info(), platform.platform_info());
                assert_eq!(app.platform_info(), WINDOWS_INFO);
                assert!(!Modifiers::command().secondary(app));
                assert!(Modifiers::control().secondary(app));
                assert_eq!(Modifiers::secondary_key(app), Modifiers::control());
                app.open_window(
                    WindowOptions {
                        window_bounds: Some(WindowBounds::centered(size(px(440.), px(260.)), app)),
                        focus_on_appearing: false,
                        show: true,
                        ..Default::default()
                    },
                    |window, app| {
                        app.new(|cx| {
                            let focus = [cx.focus_handle(), cx.focus_handle()];
                            window.focus(&focus[0], cx);
                            let pending = trace.clone();
                            let released = trace.clone();
                            cx.on_release(move |_: &mut NativePlatformRoot, _| {
                                released.releases.set(released.releases.get() + 1);
                            })
                            .detach();
                            NativePlatformRoot {
                                focus,
                                inputs: inputs.clone(),
                                trace: trace.clone(),
                                paint: paint.clone(),
                                _pending: cx.observe_pending_input(window, move |_, window, _| {
                                    pending
                                        .pending
                                        .borrow_mut()
                                        .push(window.has_pending_keystrokes());
                                }),
                            }
                        })
                    },
                )
            })
            .expect("the native runtime-OS consumer must open");
        let registration = platform
            .raw_window_handles
            .read()
            .iter()
            .copied()
            .find(|entry| entry.window_id() == handle.window_id())
            .unwrap();
        let native = platform.window_from_hwnd(registration.as_raw()).unwrap();
        assert_eq!(native.registration, registration);
        let root = app.update_for_test(|app| handle.entity(app).unwrap().downgrade());
        Self {
            handle,
            registration,
            native,
            root,
            inputs,
            trace,
            paint,
        }
    }

    fn key(&self, source: &'static str) -> bool {
        self.native
            .dispatch_input(PlatformInput::KeyDown(KeyDownEvent {
                keystroke: Keystroke::parse(source).unwrap().with_simulated_ime(),
                is_held: false,
                prefer_character_input: false,
            }))
            .propagate
    }

    fn submitted(
        &self,
        platform: &WindowsPlatform,
        app: &mut Application,
        previous: u64,
        expected_focus: usize,
    ) -> u64 {
        let expected = Source {
            labels: LABELS.map(str::to_owned).to_vec(),
            text: self
                .inputs
                .each_ref()
                .map(|input| input.text.borrow().clone()),
            calls: self.trace.calls.borrow().clone(),
            focus: Some(expected_focus),
        };
        let mut generation = None;
        pump_messages_until_with_delay(
            &Rc::downgrade(&platform.inner),
            "runtime-OS labels and text Submitted/readback",
            Duration::from_millis(5),
            || {
                generation = app
                    .update_for_test(|app| {
                        self.handle.update(app, |root, window, app| {
                            let facts = window.presentation_facts();
                            let paint = self.paint.borrow();
                            let paint = paint.as_ref()?;
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
                                    == attempt.generation
                                && paint.previous_generation.checked_add(1)
                                    == Some(attempt.generation)
                                && paint.source == expected)
                            {
                                return None;
                            }
                            assert_eq!(root.source(window, app), expected);
                            assert_eq!(self.native.registration, self.registration);
                            let capture = window.capture_frame();
                            assert!(capture.unsupported_reason.is_none(), "{capture:?}");
                            assert_eq!(capture.metadata.window_id, self.handle.window_id());
                            assert_eq!(
                                capture.metadata.framework_frame_generation,
                                attempt.generation
                            );
                            assert_eq!(
                                capture.metadata.logical_viewport_size,
                                window.viewport_size()
                            );
                            assert_eq!(
                                capture.metadata.physical_viewport_size,
                                window
                                    .viewport_size()
                                    .to_device_pixels(window.scale_factor()),
                            );
                            assert_eq!(capture.metadata.scale_factor, window.scale_factor());
                            assert_native_platform_pixels(&capture, paint);
                            assert_eq!(window.rendered_frame_revision(), attempt.generation);
                            assert_eq!(
                                window.presentation_facts().latest_present_attempt,
                                facts.latest_present_attempt,
                            );
                            Some(attempt.generation)
                        })
                    })
                    .expect("the exact native consumer must remain live until readback");
                generation.is_some()
            },
        );
        generation.unwrap()
    }
}

fn assert_native_platform_pixels(capture: &WindowFrameCapture, paint: &Paint) {
    let image = capture
        .image
        .as_ref()
        .expect("the DirectX consumer must supply RGBA readback");
    let scale = capture.metadata.scale_factor;
    assert!(scale.is_finite() && scale > 0.);
    assert_eq!(
        image.dimensions(),
        (
            capture.metadata.physical_viewport_size.width.0 as u32,
            capture.metadata.physical_viewport_size.height.0 as u32,
        ),
    );
    assert_eq!(paint.lines.len(), 6);
    for (index, line) in paint.lines.iter().enumerate() {
        let expected = match index {
            0..=3 => paint.source.labels[index].clone(),
            4 => format!("Primary: {}", paint.source.text[0]),
            _ => format!("Secondary: {}", paint.source.text[1]),
        };
        assert_eq!(line.text.as_ref(), expected);
        assert!(line.width > px(0.));
        assert!(line.width + px(32.) < paint.bounds.size.width);
        let x = f32::from(paint.bounds.origin.x) + 16.;
        let y = f32::from(paint.bounds.origin.y) + 16. + index as f32 * 28.;
        let mut ink = 0;
        for y in (y * scale).floor() as u32..((y + 24.) * scale).ceil() as u32 {
            let columns =
                (x * scale).floor() as u32..((x + f32::from(line.width)) * scale).ceil() as u32;
            for x in columns {
                assert!(x < image.width() && y < image.height());
                let pixel = image.get_pixel(x, y).0;
                if pixel[0] > 32 && pixel[1] > 32 && pixel[2] > 32 {
                    ink += 1;
                }
            }
        }
        assert!(
            ink > 5,
            "accepted-scene row {index} must contain visible shortcut/text glyphs",
        );
    }
    let position = paint.bounds.origin + point(px(32.), px(216.));
    let pixel = image
        .get_pixel(
            (f32::from(position.x) * scale).floor() as u32,
            (f32::from(position.y) * scale).floor() as u32,
        )
        .0;
    let color = marker(&paint.source);
    let expected = [(color >> 16) as u8, (color >> 8) as u8, color as u8, 255];
    assert!(
        pixel
            .iter()
            .zip(expected)
            .all(|(actual, expected)| actual.abs_diff(expected) <= 2),
        "accepted-scene source marker {pixel:?} != {expected:?}",
    );
}

#[test]
#[ignore = "requires an interactive Windows desktop and a DirectX GPU"]
fn native_runtime_os_loads_routes_labels_text_and_exact_window_retirement() {
    discard_stale_quit_messages();
    let (_guard, observation) = begin_native_window_test_observation();
    let platform =
        Rc::new(WindowsPlatform::new_windowed_for_test().expect("native runtime-OS platform"));
    assert_eq!(platform.platform_info(), WINDOWS_INFO);
    let platform_hwnd = platform.handle;
    let platform_lifetime = Rc::downgrade(&platform);
    let lifecycle_probe = platform.lifecycle_test_probe.clone();
    let text_lifetime = Arc::downgrade(&platform.text_system());
    let mut app = Application::with_platform(platform.clone()).with_quit_mode(QuitMode::Explicit);
    let mut async_app = app.update_for_test(|app| app.to_async());
    app.update_for_test(|app| {
        let predicate = Rc::new(
            KeyBindingContextPredicate::parse("NativePlatformConsumer && os == windows").unwrap(),
        );
        for use_key_equivalents in [false, true] {
            let dynamic = String::from("secondary-@");
            let expected = app
                .keyboard_mapper()
                .map_key_equivalent(Keystroke::parse("ctrl-@").unwrap(), use_key_equivalents);
            let loaded = KeyBinding::load(
                &dynamic,
                Box::new(Control),
                None,
                use_key_equivalents,
                None,
                app,
            )
            .unwrap();
            assert_eq!(loaded.keystrokes(), [expected]);
        }
        for (source, action) in [
            ("secondary-s", Box::new(Save) as Box<dyn Action>),
            ("secondary-k secondary-c", Box::new(Chord)),
            ("ctrl-f6", Box::new(Control)),
            ("cmd-f7", Box::new(PlatformKey)),
        ] {
            let dynamic = source.to_owned();
            let binding = KeyBinding::load(&dynamic, action, None, true, None, app).unwrap();
            app.bind_keys([binding]);
        }
        let contextual =
            KeyBinding::load("f8", Box::new(Contextual), Some(predicate), true, None, app).unwrap();
        app.bind_keys([contextual]);
    });
    platform
        .begin_vsync_thread()
        .expect("native vsync provider should start");
    let consumer = NativePlatformWindow::open(&mut app, &platform);
    let registration = consumer.registration;
    let hwnd = registration.as_raw();
    let input_slot = consumer.native.state.input_handler.clone();
    let diagnostic_cursor = app.update_for_test(|app| {
        app.native_boundary_diagnostics(NativeBoundaryDiagnosticCursor::default())
            .cursor
    });
    assert!(unsafe { RedrawWindow(Some(hwnd), None, None, RDW_INVALIDATE).as_bool() });
    let mut generation = consumer.submitted(&platform, &mut app, 0, 0);
    assert_eq!(
        input_slot.with_handler(|handler| handler.query_prefers_ime_for_printable_keys()),
        Some(true),
    );
    assert!(consumer.key("cmd-s"));
    assert!(consumer.trace.calls.borrow().is_empty());
    for key in ["ctrl-s", "ctrl-f6", "cmd-f7", "f8"] {
        assert!(!consumer.key(key));
    }
    assert_eq!(
        *consumer.trace.calls.borrow(),
        [("save", 0), ("control", 0), ("platform", 0), ("context", 0)],
    );
    generation = consumer.submitted(&platform, &mut app, generation, 0);
    consumer.trace.pending.borrow_mut().clear();
    assert!(!consumer.key("ctrl-k"));
    app.update_for_test(|app| {
        consumer.handle.update(app, |_, window, _| {
            assert_eq!(
                window.pending_input_keystrokes(),
                Some([Keystroke::parse("ctrl-k").unwrap()].as_slice()),
            );
        })
    })
    .unwrap();
    assert_eq!(
        input_slot.with_handler(|handler| handler.query_prefers_ime_for_printable_keys()),
        Some(false),
    );
    assert!(!consumer.key("ctrl-c"));
    assert_eq!(consumer.trace.calls.borrow().last(), Some(&("chord", 0)));
    assert_eq!(*consumer.trace.pending.borrow(), [true, false]);
    generation = consumer.submitted(&platform, &mut app, generation, 0);
    assert!(!consumer.key("ctrl-k"));
    app.update_for_test(|app| {
        consumer.handle.update(app, |root, window, cx| {
            window.focus(&root.focus[1], cx);
            assert!(!window.has_pending_keystrokes());
        })
    })
    .unwrap();
    assert_eq!(*consumer.trace.pending.borrow(), [true, false, true, false]);
    generation = consumer.submitted(&platform, &mut app, generation, 1);
    assert_eq!(
        consumer.inputs[0].created.get(),
        consumer.inputs[0].retired.get()
    );
    assert_eq!(
        consumer.inputs[1].created.get() - consumer.inputs[1].retired.get(),
        1,
    );
    assert!(consumer.key("ctrl-c"));
    assert_eq!(consumer.trace.calls.borrow().last(), Some(&("chord", 0)));
    assert_eq!(
        input_slot.with_handler(|handler| handler.query_prefers_ime_for_printable_keys()),
        Some(true),
    );
    assert!(consumer.key("z"));
    assert_eq!(
        unsafe { SendMessageW(hwnd, WM_CHAR, Some(WPARAM('z' as usize)), Some(LPARAM(1))) }.0,
        0,
    );
    assert_eq!(consumer.inputs[1].text.borrow().as_str(), "z");
    assert!(consumer.inputs[0].text.borrow().is_empty());
    generation = consumer.submitted(&platform, &mut app, generation, 1);
    assert!(!consumer.key("ctrl-s"));
    assert_eq!(consumer.trace.calls.borrow().last(), Some(&("save", 1)));
    generation = consumer.submitted(&platform, &mut app, generation, 1);

    app.update_for_test(|app| {
        let fallback =
            KeyBinding::load("secondary-k", Box::new(Prefix), None, true, None, app).unwrap();
        let chord = KeyBinding::load(
            "secondary-k secondary-c",
            Box::new(Chord),
            None,
            true,
            None,
            app,
        )
        .unwrap();
        app.bind_keys([fallback, chord]);
    });
    let before_timeout = consumer.trace.calls.borrow().clone();
    assert!(!consumer.key("ctrl-k"));
    app.update_for_test(|app| {
        consumer.handle.update(app, |_, window, _| {
            assert_eq!(
                window.pending_input_keystrokes(),
                Some([Keystroke::parse("ctrl-k").unwrap()].as_slice()),
            );
        })
    })
    .unwrap();
    assert_eq!(*consumer.trace.calls.borrow(), before_timeout);
    pump_messages_until_with_delay(
        &Rc::downgrade(&platform.inner),
        "runtime-OS pending prefix dispatches its actual timeout fallback",
        Duration::from_millis(5),
        || consumer.trace.calls.borrow().last() == Some(&("prefix", 1)),
    );
    app.update_for_test(|app| {
        consumer.handle.update(app, |_, window, _| {
            assert!(!window.has_pending_keystrokes());
        })
    })
    .unwrap();
    consumer.submitted(&platform, &mut app, generation, 1);

    let before_retirement = consumer.trace.calls.borrow().clone();
    assert!(!consumer.key("ctrl-k"));
    let foreground_barrier = Rc::new(Cell::new(false));
    app.update_for_test(|app| {
        let barrier = foreground_barrier.clone();
        app.foreground_executor()
            .spawn(async move { barrier.set(true) })
            .detach();
    });
    pump_messages_until_with_delay(
        &Rc::downgrade(&platform.inner),
        "runtime-OS pending timeout reaches its foreground poll before retirement",
        Duration::from_millis(1),
        || foreground_barrier.get(),
    );
    app.update_for_test(|app| {
        consumer.handle.update(app, |_, window, _| {
            assert_eq!(
                window.pending_input_keystrokes(),
                Some([Keystroke::parse("ctrl-k").unwrap()].as_slice()),
            );
        })
    })
    .unwrap();
    assert_eq!(*consumer.trace.calls.borrow(), before_retirement);
    let calls = consumer.trace.calls.borrow().clone();
    let pending = consumer.trace.pending.borrow().clone();
    let root_lifetime = consumer.root.clone();
    let native_lifetime = Rc::downgrade(&consumer.native);
    let paint_lifetime = Rc::downgrade(&consumer.paint);
    let trace_lifetime = Rc::downgrade(&consumer.trace);
    let input_lifetimes = consumer.inputs.each_ref().map(Rc::downgrade);
    app.update_for_test(|app| {
        consumer.handle.update(app, |_, window, app| {
            window.remove_window(app);
        })
    })
    .unwrap();
    pump_messages_until_with_delay(
        &Rc::downgrade(&platform.inner),
        "runtime-OS exact registration and root retirement",
        Duration::from_millis(5),
        || {
            !unsafe { IsWindow(Some(hwnd)).as_bool() }
                && !platform.raw_window_handles.read().contains(&registration)
                && platform.inner.native_retirement_is_settled()
                && root_lifetime.upgrade().is_none()
        },
    );
    assert_eq!(consumer.trace.drops.get(), 1);
    assert_eq!(consumer.trace.releases.get(), 1);
    assert!(
        input_slot.take().is_none(),
        "the retired slot must own no input handler"
    );
    for input in &consumer.inputs {
        assert!(input.created.get() > 0);
        assert_eq!(input.created.get(), input.retired.get());
    }
    assert_eq!(*consumer.trace.pending.borrow(), pending);
    let replacement = NativePlatformWindow::open(&mut app, &platform);
    assert_ne!(replacement.handle.window_id(), registration.window_id());
    assert_ne!(
        replacement.registration.generation(),
        registration.generation()
    );
    assert!(unsafe {
        RedrawWindow(
            Some(replacement.registration.as_raw()),
            None,
            None,
            RDW_INVALIDATE,
        )
        .as_bool()
    });
    let replacement_generation = replacement.submitted(&platform, &mut app, 0, 0);
    assert!(replacement.key("ctrl-c"));
    assert!(replacement.trace.calls.borrow().is_empty());
    assert!(!replacement.key("ctrl-s"));
    assert_eq!(*replacement.trace.calls.borrow(), [("save", 0)]);
    replacement.submitted(&platform, &mut app, replacement_generation, 0);
    let retired_deadline = std::time::Instant::now();
    pump_messages_until_with_delay(
        &Rc::downgrade(&platform.inner),
        "runtime-OS retired prefix cannot reach its deadline or successor",
        Duration::from_millis(5),
        || retired_deadline.elapsed() >= Duration::from_millis(1100),
    );
    assert_eq!(*replacement.trace.calls.borrow(), [("save", 0)]);
    assert_eq!(*consumer.trace.calls.borrow(), calls);
    assert_eq!(*consumer.trace.pending.borrow(), pending);
    let replacement_registration = replacement.registration;
    let replacement_native_lifetime = Rc::downgrade(&replacement.native);
    app.update_for_test(|app| {
        replacement
            .handle
            .update(app, |_, window, app| window.remove_window(app))
    })
    .unwrap();
    pump_messages_until_with_delay(
        &Rc::downgrade(&platform.inner),
        "runtime-OS successor exact retirement",
        Duration::from_millis(5),
        || {
            !unsafe { IsWindow(Some(replacement_registration.as_raw())).as_bool() }
                && !platform
                    .raw_window_handles
                    .read()
                    .contains(&replacement_registration)
                && platform.inner.native_retirement_is_settled()
                && replacement.root.upgrade().is_none()
        },
    );
    for exact in [registration, replacement_registration] {
        let lifecycle = observation
            .events()
            .into_iter()
            .filter(|event| {
                event.window().window_id() == exact.window_id()
                    && event.window().native_generation() == exact.generation()
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
            panic!("unexpected runtime-OS consumer retirement: {lifecycle:?}");
        };
        assert_eq!(quiesced, destroyed);
        assert_eq!(destroyed, terminal);
    }
    let diagnostics = app.update_for_test(|app| app.native_boundary_diagnostics(diagnostic_cursor));
    assert_eq!(diagnostics.omitted_before_cursor, 0);
    assert!(diagnostics.terminal.iter().all(|event| {
        !matches!(
            event.disposition,
            NativeBoundaryDisposition::InvariantFailure(_)
        )
    }));
    let target = NativeBoundaryTarget::Window(registration.window_id());
    for propagate in [true, false] {
        assert!(diagnostics.terminal.iter().any(|event| {
            event.target == target
                && event.kind == NativeBoundaryKind::Callback(NativeCallbackKind::PlatformInput)
                && matches!(
                    event.disposition,
                    NativeBoundaryDisposition::Delivered { input_result: Some(result) }
                        if result.propagate == propagate
                )
        }));
    }
    assert!(diagnostics.terminal.iter().any(|event| {
        event.target == target
            && event.kind
                == NativeBoundaryKind::Callback(NativeCallbackKind::PlatformInputHandler(
                    NativeInputHandlerOperation::ReplaceTextInRange,
                ))
            && matches!(
                event.disposition,
                NativeBoundaryDisposition::Delivered { .. }
            )
    }));
    assert!(observation.events().iter().any(|event| {
        event.window().window_id() == registration.window_id()
            && event.window().native_generation() == registration.generation()
            && matches!(
                event.kind(),
                NativeWindowTestEventKind::WindowMessage {
                    message: NativeWindowTestMessage::Other(WM_CHAR),
                    disposition: NativeWindowTestMessageDisposition::Returned(0),
                    ..
                }
            )
    }));
    let handle = consumer.handle;
    let replacement_handle = replacement.handle;
    assert_eq!(replacement.trace.drops.get(), 1);
    assert_eq!(replacement.trace.releases.get(), 1);
    for input in &replacement.inputs {
        assert_eq!(input.created.get(), input.retired.get());
    }
    drop(input_slot);
    drop(replacement);
    drop(consumer);
    assert!(native_lifetime.upgrade().is_none());
    assert!(replacement_native_lifetime.upgrade().is_none());
    assert!(paint_lifetime.upgrade().is_none());
    assert!(trace_lifetime.upgrade().is_none());
    assert!(
        input_lifetimes
            .iter()
            .all(|input| input.upgrade().is_none())
    );
    assert!(app.update_for_test(|app| {
        assert!(handle.update(app, |_, _, _| ()).is_err());
        assert!(replacement_handle.update(app, |_, _, _| ()).is_err());
        app.windows().is_empty()
    }));
    assert!(platform.raw_window_handles.read().is_empty());
    app.update_for_test(|app| app.shutdown());
    drop(app);
    assert_eq!(
        async_app
            .update_window(handle.into(), |_, _, _| ())
            .unwrap_err()
            .to_string(),
        "app was released",
    );
    drop(async_app);
    let pump_platform = Rc::downgrade(&platform.inner);
    drop(platform);
    pump_messages_until_with_delay(
        &pump_platform,
        "runtime-OS platform HWND and OLE release",
        Duration::from_millis(5),
        || {
            !unsafe { IsWindow(Some(platform_hwnd)).as_bool() }
                && platform_lifetime.upgrade().is_none()
        },
    );
    assert_eq!(lifecycle_probe.platform_destroy_attempts(), 1);
    assert_eq!(lifecycle_probe.ole_uninitialize_count(), 1);
    assert!(text_lifetime.upgrade().is_none());
}
