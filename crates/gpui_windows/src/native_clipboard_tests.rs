//! Real Windows clipboard snapshots consumed by a registered text handler and native renderer.
//!
//! Official async/paste contract: zed d70c45e5bb1d8b7b3926fba5a6d1c69c23bbc405,
//! crates/gpui/src/{app,input,platform}.rs. Fixed reference:
//! 635026fabc38440fce5e1767d125026a17b12012, at the same paths.
//! Windows text/hash normalization follows decbf641b18f1982b3475c037e7c5c554471574f.
//! Delivery uses the public registered handler, without physical keyboard input or a live IME.
//! Submitted generations qualify renderer acceptance, not compositor scanout.
//! The clipboard guard restores GPUI-readable/writable entries, including text metadata, on
//! normal completion and unwinding. Unsupported contents fail before mutation. Additional native
//! formats, original ownership and clipboard history cannot be restored. Observed external
//! clipboard updates prevent restoration; sequence checks are best effort across native calls.

use super::*;
use crate::clipboard::test_support::{clipboard_state, retry_clipboard_open};
use crate::native_test_observation::NativeWindowTestMessageDisposition;
use futures::FutureExt;
use open_gpui::{
    App, ClipboardEntry, ClipboardItem, ElementInputHandler, EntityInputHandler, FocusHandle,
    ImageFormat, InteractiveElement, NativeInputHandlerOperation, Pixels, PlatformInputHandlerSlot,
    Point, ShapedLine, TextAlign, TextRun, UTF16Selection, WindowHandle,
    WindowInitialPresentationStatus, black, font,
};
use std::ops::Range;
use windows::Win32::System::DataExchange::GetClipboardSequenceNumber;

struct NativeClipboardRestore {
    original: Option<ClipboardItem>,
    sequence: u32,
    modified: bool,
}

impl NativeClipboardRestore {
    fn capture(cx: &App) -> Self {
        let (sequence, formats) = retry_clipboard_open(clipboard_state)
            .expect("the native clipboard must be available before mutation");
        let original = retry_clipboard_open(|| cx.read_from_clipboard());
        assert_eq!(
            unsafe { GetClipboardSequenceNumber() },
            sequence,
            "the clipboard must remain unchanged while capturing its prior contents",
        );
        assert!(
            formats == 0 || original.is_some(),
            "the native clipboard test requires empty or GPUI-restorable prior contents",
        );
        if let Some(item) = &original {
            assert!(
                item.entries.iter().all(|entry| match entry {
                    ClipboardEntry::String(_) => true,
                    ClipboardEntry::Image(image) => matches!(
                        image.format(),
                        ImageFormat::Png | ImageFormat::Jpeg | ImageFormat::Gif | ImageFormat::Svg
                    ),
                    ClipboardEntry::ExternalPaths(_) => false,
                }),
                "the native clipboard test cannot restore these prior entries",
            );
        }
        Self {
            original,
            sequence,
            modified: false,
        }
    }

    fn write(&mut self, cx: &App, item: ClipboardItem) {
        self.write_expected(cx, item.clone(), item);
    }

    fn write_expected(&mut self, cx: &App, input: ClipboardItem, expected: ClipboardItem) {
        retry_clipboard_open(|| {
            assert_eq!(
                unsafe { GetClipboardSequenceNumber() },
                self.sequence,
                "an external clipboard update must not be overwritten by the test",
            );
            cx.write_to_clipboard(input.clone());
        });
        self.sequence = unsafe { GetClipboardSequenceNumber() };
        self.modified = true;
        assert!(
            retry_clipboard_open(|| cx.read_from_clipboard()).as_ref() == Some(&expected),
            "the real Windows clipboard must contain the exact test item",
        );
    }

    fn restore(&mut self) -> bool {
        if !std::mem::take(&mut self.modified) {
            return true;
        }
        if unsafe { GetClipboardSequenceNumber() } != self.sequence {
            return false;
        }
        retry_clipboard_open(|| {
            assert_eq!(
                unsafe { GetClipboardSequenceNumber() },
                self.sequence,
                "an external clipboard update must not be overwritten during restoration",
            );
            crate::clipboard::write_to_clipboard(self.original.clone().unwrap_or(ClipboardItem {
                entries: Vec::new(),
            }));
        });
        if self.original.is_some() {
            return retry_clipboard_open(crate::clipboard::read_from_clipboard) == self.original;
        }
        retry_clipboard_open(clipboard_state).is_some_and(|(_, formats)| formats == 0)
    }
}

impl Drop for NativeClipboardRestore {
    fn drop(&mut self) {
        if !matches!(catch_unwind(AssertUnwindSafe(|| self.restore())), Ok(true)) {
            eprintln!(
                "native clipboard cleanup could not restore prior entries; clipboard changed or became unavailable",
            );
        }
    }
}

struct NativeClipboardPaint {
    previous_rendered_generation: u64,
    shaped: ShapedLine,
}

struct NativeClipboardTextRoot {
    focus: FocusHandle,
    text: String,
    replacements: usize,
    paints: Rc<RefCell<Vec<NativeClipboardPaint>>>,
}

impl EntityInputHandler for NativeClipboardTextRoot {
    fn text_for_range(
        &mut self,
        range: Range<usize>,
        adjusted_range: &mut Option<Range<usize>>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<String> {
        let utf16 = self.text.encode_utf16().collect::<Vec<_>>();
        let text = String::from_utf16(utf16.get(range.clone())?).ok()?;
        *adjusted_range = Some(range);
        Some(text)
    }

    fn selected_text_range(
        &mut self,
        _: bool,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        let end = self.text.encode_utf16().count();
        Some(UTF16Selection {
            range: end..end,
            reversed: false,
        })
    }

    fn marked_text_range(&self, _: &mut Window, _: &mut Context<Self>) -> Option<Range<usize>> {
        None
    }

    fn unmark_text(&mut self, _: &mut Window, _: &mut Context<Self>) {}

    fn replace_text_in_range(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        assert!(
            range.is_none(),
            "the default paste must use the current selection",
        );
        self.text.push_str(text);
        self.replacements += 1;
        cx.notify();
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        _: Option<Range<usize>>,
        _: &str,
        _: Option<Range<usize>>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) {
        panic!("live IME composition is outside this native clipboard consumer");
    }

    fn bounds_for_range(
        &mut self,
        _: Range<usize>,
        element_bounds: Bounds<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        Some(element_bounds)
    }

    fn character_index_for_point(
        &mut self,
        _: Point<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<usize> {
        None
    }
}

impl Render for NativeClipboardTextRoot {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let source = self.text.clone();
        let focus = self.focus.clone();
        let view = cx.entity();
        let paints = self.paints.clone();
        div()
            .id("native-clipboard-text")
            .track_focus(&self.focus)
            .size_full()
            .bg(white())
            .child(
                canvas(
                    move |_, window, _| {
                        let runs = [TextRun {
                            len: source.len(),
                            font: font("Segoe UI"),
                            color: black(),
                            ..Default::default()
                        }];
                        window
                            .text_system()
                            .shape_line(source.into(), px(16.), &runs, None)
                    },
                    move |bounds, shaped, window, cx| {
                        let previous_rendered_generation = window.rendered_frame_revision();
                        shaped
                            .paint(
                                bounds.origin + point(px(16.), px(16.)),
                                px(24.),
                                TextAlign::Left,
                                None,
                                window,
                                cx,
                            )
                            .expect("the actual clipboard text must paint through DirectWrite");
                        window.handle_input(&focus, ElementInputHandler::new(bounds, view), cx);
                        paints.borrow_mut().push(NativeClipboardPaint {
                            previous_rendered_generation,
                            shaped,
                        });
                    },
                )
                .size_full(),
            )
    }
}

fn wait_for_native_clipboard_submission(
    app: &mut Application,
    handle: WindowHandle<NativeClipboardTextRoot>,
    expected_text: &str,
    previous_generation: u64,
) -> u64 {
    let mut submitted = None;
    pump_messages_until_with_delay(
        "native clipboard text accepted nonempty submission",
        Duration::from_millis(5),
        || {
            submitted = app
                .update_for_test(|cx| {
                    handle.update(cx, |root, window, cx| {
                        let facts = window.presentation_facts();
                        let attempt = facts.latest_present_attempt?;
                        let diagnostics = window.submitted_frame_diagnostics_for_test()?;
                        let paints = root.paints.borrow();
                        let paint = paints.last()?;
                        (root.text == expected_text
                            && paint.shaped.text.as_ref() == expected_text
                            && paint.shaped.width > px(0.)
                            && paint.shaped.runs.iter().any(|run| !run.glyphs.is_empty())
                            && paint.previous_rendered_generation.checked_add(1)
                                == Some(attempt.generation)
                            && window.focused(cx) == Some(root.focus.clone())
                            && window.committed_focus(cx) == Some(root.focus.clone())
                            && attempt.generation > previous_generation
                            && attempt.outcome == PlatformWindowPresentOutcome::Submitted
                            && attempt.contained_valid_primitives
                            && facts.initial_presentation
                                == WindowInitialPresentationStatus::Completed
                            && facts.native_visible
                            && facts.frame_accepted_generation == Some(attempt.generation)
                            && facts.present_submitted_generation == Some(attempt.generation)
                            && facts.non_empty_presented_generation == Some(attempt.generation)
                            && window.rendered_frame_revision() == attempt.generation
                            && diagnostics.generation() == attempt.generation)
                            .then_some(attempt.generation)
                    })
                })
                .expect("the native clipboard consumer must remain open until submission");
            submitted.is_some()
        },
    );
    submitted.expect("the clipboard text must belong to an actual accepted Submitted frame")
}

#[test]
fn native_async_clipboard_snapshot_pastes_into_submitted_text_and_reconstructs_cleanly() {
    const INPUT_TEXT: &str = "clipboard\0你好 e\u{301}\0😀 𐐀";
    const COPIED_TEXT: &str = "clipboard 你好 e\u{301} 😀 𐐀";
    const METADATA: &str = "{\"native_clipboard_snapshot\":true}";

    discard_stale_quit_messages();
    let (_observation_guard, observation) = begin_native_window_test_observation();
    let platform = Rc::new(
        WindowsPlatform::new_windowed_for_test()
            .expect("the native DirectWrite/DirectX clipboard platform must initialize"),
    );
    let platform_hwnd = platform.handle;
    let lifecycle_probe = platform.lifecycle_test_probe.clone();
    let weak_platform_text_system = Arc::downgrade(&platform.text_system());
    let mut app = Application::with_platform(platform.clone()).with_quit_mode(QuitMode::Explicit);
    let weak_text_system = Arc::downgrade(&app.text_system());
    platform
        .begin_vsync_thread()
        .expect("native vsync provider should start");

    let mut snapshot: Option<ClipboardItem> = None;
    let mut retired_slot: Option<PlatformInputHandlerSlot> = None;
    let mut previous_registration: Option<RegisteredWindow> = None;
    for initial_text in ["initial: ", "reconstructed: "] {
        let observation_cursor = observation.cursor();
        let paints = Rc::new(RefCell::new(Vec::new()));
        let handle = app
            .update_for_test(|cx| {
                cx.open_window(
                    WindowOptions {
                        window_bounds: Some(WindowBounds::centered(size(px(640.), px(160.)), cx)),
                        focus_on_appearing: false,
                        activation_policy: WindowActivationPolicy {
                            accepts_activation: false,
                            focus_on_click: false,
                        },
                        accepts_pointer_input: false,
                        show: true,
                        ..WindowOptions::default()
                    },
                    |window, cx| {
                        cx.new(|cx| {
                            let focus = cx.focus_handle();
                            window.focus(&focus, cx);
                            NativeClipboardTextRoot {
                                focus,
                                text: initial_text.into(),
                                replacements: 0,
                                paints: paints.clone(),
                            }
                        })
                    },
                )
            })
            .expect("the native clipboard text consumer must open");
        let registration = platform
            .raw_window_handles
            .read()
            .iter()
            .copied()
            .find(|registration| registration.window_id() == handle.window_id())
            .expect("the clipboard consumer must own its exact HWND registration");
        if let Some(previous) = previous_registration {
            assert_ne!(registration.window_id(), previous.window_id());
            assert_ne!(registration.generation(), previous.generation());
        }
        let hwnd = registration.as_raw();
        let native_window = platform
            .window_from_hwnd(hwnd)
            .expect("the native clipboard HWND must remain registered");
        assert_eq!(native_window.registration, registration);
        let native_lifetime = Rc::downgrade(&native_window);
        let native_facts = native_window
            .observed_platform_facts_for_test()
            .expect("native clipboard window facts must be observable");
        assert!(!native_facts.accepts_pointer_input);
        assert!(!native_facts.accepts_activation);
        assert!(!native_facts.focus_on_click);
        let input_slot = native_window.state.input_handler.clone();
        let frame_requests = observe_frame_requests(&native_window);
        let (root_lifetime, window_text_lifetime) = app.update_for_test(|cx| {
            let root = handle
                .root(cx)
                .expect("the clipboard text entity must exist")
                .downgrade();
            let text = handle
                .update(cx, |_, window, _| Rc::downgrade(window.text_system()))
                .expect("the clipboard window must expose its own text system");
            (root, text)
        });
        let initial_paint_cursor = observation.cursor();
        assert!(unsafe { RedrawWindow(Some(hwnd), None, None, RDW_INVALIDATE).as_bool() });
        pump_messages_until_with_delay(
            "native clipboard initialization WM_PAINT",
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
        let initial_generation =
            wait_for_native_clipboard_submission(&mut app, handle, initial_text, 0);
        let diagnostic_cursor = app.update_for_test(|cx| {
            cx.native_boundary_diagnostics(NativeBoundaryDiagnosticCursor::default())
                .cursor
        });
        let item = snapshot
            .get_or_insert_with(|| {
                app.update_for_test(|cx| {
                    let mut clipboard = NativeClipboardRestore::capture(cx);
                    let expected = ClipboardItem::new_string_with_metadata(
                        COPIED_TEXT.into(),
                        METADATA.into(),
                    );
                    clipboard.write_expected(
                        cx,
                        ClipboardItem::new_string_with_metadata(INPUT_TEXT.into(), METADATA.into()),
                        expected.clone(),
                    );
                    let task = retry_clipboard_open(|| cx.read_from_clipboard_async());
                    clipboard.write(
                        cx,
                        ClipboardItem::new_string("later clipboard value".into()),
                    );
                    let item = task
                        .now_or_never()
                        .expect(
                            "Windows async clipboard reads must return an already-ready snapshot",
                        )
                        .expect("the native clipboard snapshot must read successfully")
                        .expect("the known native clipboard snapshot must be nonempty");
                    assert_eq!(item, expected);
                    assert!(
                        clipboard.restore(),
                        "the representable prior clipboard contents must be restored",
                    );
                    item
                })
            })
            .clone();
        if let Some(retired) = &retired_slot {
            assert!(retired.take().is_none());
        }
        let paints_before = paints.borrow().len();
        let frame_requests_before = frame_requests.get();
        assert_eq!(
            input_slot.with_handler(|handler| handler.paste(item)),
            Some(()),
        );
        let expected_text = format!("{initial_text}{COPIED_TEXT}");
        let pasted_generation = wait_for_native_clipboard_submission(
            &mut app,
            handle,
            &expected_text,
            initial_generation,
        );
        assert!(pasted_generation > initial_generation);
        assert!(paints.borrow().len() > paints_before);
        assert!(frame_requests.get() > frame_requests_before);
        app.update_for_test(|cx| {
            handle.update(cx, |root, _, _| {
                assert_eq!(root.text, expected_text);
                assert_eq!(root.replacements, 1);
            })
        })
        .expect("the registered paste must update the real text entity exactly once");
        let end = expected_text.encode_utf16().count();
        assert_eq!(
            input_slot.with_handler(|handler| {
                handler
                    .selected_text_range(false)
                    .map(|selection| (selection.range, selection.reversed))
            }),
            Some(Some((end..end, false))),
        );
        let mut adjusted_range = None;
        assert_eq!(
            input_slot.with_handler(|handler| handler.text_for_range(0..end, &mut adjusted_range)),
            Some(Some(expected_text)),
        );
        assert_eq!(adjusted_range, Some(0..end));

        app.update_for_test(|cx| handle.update(cx, |_, window, cx| window.remove_window(cx)))
            .expect("the exact native clipboard text window must close");
        pump_messages_until_with_delay(
            "exact native clipboard registration retirement",
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
            panic!("unexpected native clipboard retirement sequence: {lifecycle:?}");
        };
        assert_eq!(quiesced, destroyed);
        assert_eq!(destroyed, terminal);
        let diagnostics = app.update_for_test(|cx| {
            assert!(handle.update(cx, |_, _, _| ()).is_err());
            assert!(cx.windows().is_empty());
            cx.native_boundary_diagnostics(diagnostic_cursor)
        });
        assert_eq!(diagnostics.omitted_before_cursor, 0);
        assert!(diagnostics.terminal.iter().all(|diagnostic| {
            !matches!(
                diagnostic.disposition,
                NativeBoundaryDisposition::InvariantFailure(_)
            )
        }));
        assert_eq!(
            diagnostics
                .terminal
                .iter()
                .filter(|diagnostic| {
                    diagnostic.target == NativeBoundaryTarget::Window(handle.window_id())
                        && diagnostic.kind
                            == NativeBoundaryKind::Callback(
                                NativeCallbackKind::PlatformInputHandler(
                                    NativeInputHandlerOperation::Paste,
                                ),
                            )
                        && matches!(
                            diagnostic.disposition,
                            NativeBoundaryDisposition::Delivered { .. }
                        )
                })
                .count(),
            1,
        );
        assert!(platform.raw_window_handles.read().is_empty());
        assert!(input_slot.take().is_none());
        assert!(root_lifetime.upgrade().is_none());
        assert!(window_text_lifetime.upgrade().is_none());
        drop(native_window);
        assert!(native_lifetime.upgrade().is_none());
        previous_registration = Some(registration);
        retired_slot = Some(input_slot);
    }

    drop(retired_slot);
    app.update_for_test(|cx| cx.shutdown());
    drop(app);
    drop(platform);
    pump_messages_until_with_delay(
        "native clipboard platform message window retirement",
        Duration::from_millis(5),
        || unsafe { !IsWindow(Some(platform_hwnd)).as_bool() },
    );
    assert_eq!(lifecycle_probe.platform_destroy_attempts(), 1);
    assert_eq!(lifecycle_probe.ole_uninitialize_count(), 1);
    assert!(weak_text_system.upgrade().is_none());
    assert!(weak_platform_text_system.upgrade().is_none());
}
