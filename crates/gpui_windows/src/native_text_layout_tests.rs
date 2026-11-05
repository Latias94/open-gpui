use super::*;
use crate::native_test_observation::NativeWindowTestMessageDisposition;
use open_gpui::{
    Pixels, ShapedLine, SharedString, TextAlign, TextRun, WindowHandle,
    WindowInitialPresentationStatus, font,
};

#[derive(Clone, Debug)]
struct NativeTextLayoutPaint {
    previous_rendered_generation: u64,
    canvas_bounds: Bounds<Pixels>,
    shaped: ShapedLine,
    result: Result<(), String>,
}

struct NativeTextLayoutRoot {
    source: SharedString,
    paints: Rc<RefCell<Vec<NativeTextLayoutPaint>>>,
}

impl Render for NativeTextLayoutRoot {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let source = self.source.clone();
        let paints = self.paints.clone();
        canvas(
            move |_, window, _| {
                let runs = [TextRun {
                    len: source.len(),
                    font: font("Segoe UI"),
                    color: white(),
                    ..Default::default()
                }];
                window
                    .text_system()
                    .shape_line(source, px(16.), &runs, None)
            },
            move |bounds, shaped, window, cx| {
                let previous_rendered_generation = window.rendered_frame_revision();
                let result = shaped
                    .paint(
                        bounds.origin + point(px(16.), px(16.)),
                        px(32.),
                        TextAlign::Left,
                        None,
                        window,
                        cx,
                    )
                    .map_err(|error| format!("{error:#}"));
                paints.borrow_mut().push(NativeTextLayoutPaint {
                    previous_rendered_generation,
                    canvas_bounds: bounds,
                    shaped,
                    result,
                });
            },
        )
        .size_full()
    }
}

fn wait_for_native_text_layout_submission(
    app: &mut Application,
    window: WindowHandle<NativeTextLayoutRoot>,
    paints: &RefCell<Vec<NativeTextLayoutPaint>>,
    source: &str,
    previous_generation: u64,
) -> (u64, NativeTextLayoutPaint) {
    let mut submitted = None;
    pump_messages_until_with_delay(
        "native text layout accepted nonempty submission",
        Duration::from_millis(5),
        || {
            let (facts, rendered_generation, diagnostics_generation) = app
                .update_for_test(|cx| {
                    window.update(cx, |_, window, _| {
                        (
                            window.presentation_facts(),
                            window.rendered_frame_revision(),
                            window
                                .submitted_frame_diagnostics_for_test()
                                .map(|diagnostics| diagnostics.generation()),
                        )
                    })
                })
                .expect("the native text window must stay open until its frame is submitted");
            let paints = paints.borrow();
            for paint in paints.iter() {
                assert!(
                    paint.result.is_ok(),
                    "native ShapedLine::paint failed for {:?}: {:?}",
                    paint.shaped.text,
                    paint.result,
                );
            }
            let Some(attempt) = facts.latest_present_attempt else {
                return false;
            };
            let Some(paint) = paints.last() else {
                return false;
            };
            if paint.shaped.text.as_ref() == source
                && paint.previous_rendered_generation.checked_add(1) == Some(attempt.generation)
                && attempt.generation > previous_generation
                && attempt.outcome == PlatformWindowPresentOutcome::Submitted
                && attempt.contained_valid_primitives
                && facts.initial_presentation == WindowInitialPresentationStatus::Completed
                && facts.native_visible
                && facts.frame_accepted_generation == Some(attempt.generation)
                && facts.present_submitted_generation == Some(attempt.generation)
                && facts.non_empty_presented_generation == Some(attempt.generation)
                && rendered_generation == attempt.generation
                && diagnostics_generation == Some(attempt.generation)
            {
                submitted = Some((attempt.generation, paint.clone()));
                true
            } else {
                false
            }
        },
    );
    submitted.expect("the painted text must belong to an accepted nonempty Submitted frame")
}

fn assert_native_text_layout(paint: &NativeTextLayoutPaint, source: &str) {
    let shaped = &paint.shaped;
    assert_eq!(paint.result, Ok(()));
    assert_eq!(shaped.text.as_ref(), source);
    assert_eq!(shaped.len(), source.len());
    assert!(shaped.width.as_f32().is_finite() && shaped.width > px(0.));
    assert!(shaped.width + px(32.) < paint.canvas_bounds.size.width);
    assert!(paint.canvas_bounds.size.height > px(48.));
    assert_eq!(shaped.x_for_index(source.len()), shaped.width);
    assert!(source.contains('\t') && source.contains('\u{2028}') && source.contains('\u{2029}'));
    assert!(source.chars().any(|character| character.len_utf8() == 4));
    let glyphs = shaped
        .runs
        .iter()
        .flat_map(|run| &run.glyphs)
        .collect::<Vec<_>>();
    assert!(!glyphs.is_empty());
    for glyph in &glyphs {
        assert!(glyph.index < source.len() && source.is_char_boundary(glyph.index));
        assert!(glyph.position.x.as_f32().is_finite());
        assert!(glyph.position.y.as_f32().is_finite());
    }
    let mut remainder = shaped.clone();
    let mut reconstructed = String::new();
    for (index, character) in source.char_indices() {
        let (piece, rest) = remainder.split_at(character.len_utf8());
        remainder = rest;
        assert_eq!(piece.text.as_ref(), character.to_string());
        assert_eq!(piece.len(), character.len_utf8());
        reconstructed.push_str(&piece.text);
        match character {
            '\t' => {
                assert!(piece.width > px(0.), "tab at byte {index} must advance");
                assert!(glyphs.iter().any(|glyph| glyph.index == index));
                assert_eq!(shaped.index_for_x(shaped.x_for_index(index)), Some(index));
            }
            '\u{2028}' | '\u{2029}' => {
                assert!(piece.width.abs() < px(0.001));
            }
            _ if character.len_utf8() == 4 => {
                assert!(glyphs.iter().any(|glyph| glyph.index == index));
                assert!(piece.width > px(0.));
            }
            _ => {}
        }
    }
    assert_eq!(reconstructed, source);
    assert!(remainder.text.is_empty());
    assert_eq!(remainder.len(), 0);
}

#[test]
fn native_text_layout_submits_complete_unicode_source_and_notified_redraw() {
    const INITIAL_TEXT: &str = "\té\t𐐀\u{2028}中\u{2029}z\t";
    const CHANGED_TEXT: &str = "updated\t𐐀\u{2029}é\t中\u{2028}zz\t";

    discard_stale_quit_messages();
    let (_observation_guard, observation) = begin_native_window_test_observation();
    let observation_cursor = observation.cursor();
    let platform = Rc::new(
        WindowsPlatform::new(false)
            .expect("the native DirectWrite/DirectX platform must initialize"),
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

    let paints = Rc::new(RefCell::new(Vec::new()));
    let window = app
        .update_for_test(|cx| {
            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::centered(size(px(640.), px(180.)), cx)),
                    focus_on_appearing: false,
                    show: true,
                    ..WindowOptions::default()
                },
                |_, cx| {
                    cx.new(|_| NativeTextLayoutRoot {
                        source: INITIAL_TEXT.into(),
                        paints: paints.clone(),
                    })
                },
            )
        })
        .expect("the native text layout window must open");
    let registration = platform
        .raw_window_handles
        .read()
        .iter()
        .copied()
        .find(|registration| registration.window_id() == window.window_id())
        .expect("the native text layout window must own its exact HWND registration");
    let hwnd = registration.as_raw();
    let native_window = platform
        .window_from_hwnd(hwnd)
        .expect("the native text layout HWND must remain registered");
    assert_eq!(native_window.registration, registration);
    let frame_requests = observe_frame_requests(&native_window);
    let initial_paint_cursor = observation.cursor();
    assert!(unsafe { RedrawWindow(Some(hwnd), None, None, RDW_INVALIDATE).as_bool() });
    pump_messages_until_with_delay(
        "native text layout initialization WM_PAINT",
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
    let (initial_generation, initial_paint) =
        wait_for_native_text_layout_submission(&mut app, window, &paints, INITIAL_TEXT, 0);
    assert_native_text_layout(&initial_paint, INITIAL_TEXT);

    let paints_before = paints.borrow().len();
    let frame_requests_before = frame_requests.get();
    app.update_for_test(|cx| {
        window.update(cx, |root, _, cx| {
            root.source = CHANGED_TEXT.into();
            cx.notify();
        })
    })
    .expect("changing the native text source must notify its existing view");
    let (changed_generation, changed_paint) = wait_for_native_text_layout_submission(
        &mut app,
        window,
        &paints,
        CHANGED_TEXT,
        initial_generation,
    );
    assert!(changed_generation > initial_generation);
    assert!(paints.borrow().len() > paints_before);
    assert!(frame_requests.get() > frame_requests_before);
    assert_native_text_layout(&changed_paint, CHANGED_TEXT);
    assert_ne!(initial_paint.shaped.text, changed_paint.shaped.text);

    assert!(platform.raw_window_handles.read().contains(&registration));
    app.update_for_test(|cx| window.update(cx, |_, window, cx| window.remove_window(cx)))
        .expect("the exact native text layout window must close");
    pump_messages_until_with_delay(
        "exact native text layout registration retirement",
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
                    "the native text boundary must not swallow a panic",
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
        panic!("unexpected native text layout retirement sequence: {lifecycle:?}");
    };
    assert_eq!(quiesced, destroyed);
    assert_eq!(destroyed, terminal);
    assert!(app.update_for_test(|cx| cx.windows().is_empty()));
    assert!(platform.raw_window_handles.read().is_empty());
    drop(native_window);

    app.update_for_test(|cx| cx.shutdown());
    drop(app);
    drop(platform);
    pump_messages_until_with_delay(
        "native text layout platform message window retirement",
        Duration::from_millis(5),
        || unsafe { !IsWindow(Some(platform_hwnd)).as_bool() },
    );
    assert_eq!(lifecycle_probe.ole_uninitialize_count(), 1);
    assert!(weak_text_system.upgrade().is_none());
    assert!(weak_platform_text_system.upgrade().is_none());
}
