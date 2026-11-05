use super::*;
use crate::native_test_observation::NativeWindowTestMessageDisposition;
use open_gpui::{
    Entity, FontId, GlyphId, Pixels, ShapedLine, SharedString, TextAlign, TextRun, TextSystem,
    WindowHandle, WindowInitialPresentationStatus, font,
};
use std::borrow::Cow;

const LILEX: &[u8] = include_bytes!("../../../tests/fixtures/fonts/Lilex-Regular.ttf");
const IBM_PLEX: &[u8] = include_bytes!("../../../tests/fixtures/fonts/IBMPlexSans-Regular.ttf");
const SOURCE: &str = "mi\té\t𐐀\u{2028}中\u{2029}z\t";

#[derive(Clone, Debug)]
struct NativeFontInstallationPaint {
    previous_rendered_generation: u64,
    canvas_bounds: Bounds<Pixels>,
    resolved_font: FontId,
    shaped: ShapedLine,
    result: Result<(), String>,
    retained: Option<ShapedLine>,
    retained_result: Option<Result<(), String>>,
}

struct NativeFontInstallationChild {
    source: SharedString,
    retained: Option<ShapedLine>,
    paints: Rc<RefCell<Vec<NativeFontInstallationPaint>>>,
}

impl Render for NativeFontInstallationChild {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let source = self.source.clone();
        let retained = self.retained.clone();
        let paints = self.paints.clone();
        canvas(
            move |_, window, cx| {
                let window_text_system: &Arc<TextSystem> = window.text_system().as_ref();
                assert!(
                    Arc::ptr_eq(window_text_system, cx.text_system()),
                    "the native window must shape through its application's exact TextSystem",
                );
                let runs = [
                    TextRun {
                        len: 1,
                        font: font("Lilex"),
                        color: white(),
                        ..Default::default()
                    },
                    TextRun {
                        len: source.len() - 1,
                        font: font("IBM Plex Sans"),
                        color: white(),
                        ..Default::default()
                    },
                ];
                let resolved_font = window.text_system().resolve_font(&font("Lilex"));
                let shaped = window
                    .text_system()
                    .shape_line(source, px(16.), &runs, None);
                (resolved_font, shaped, retained)
            },
            move |bounds, (resolved_font, shaped, retained), window, cx| {
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
                let retained_result = retained.as_ref().map(|line| {
                    line.paint(
                        bounds.origin + point(px(16.), px(80.)),
                        px(32.),
                        TextAlign::Left,
                        None,
                        window,
                        cx,
                    )
                    .map_err(|error| format!("{error:#}"))
                });
                paints.borrow_mut().push(NativeFontInstallationPaint {
                    previous_rendered_generation,
                    canvas_bounds: bounds,
                    resolved_font,
                    shaped,
                    result,
                    retained,
                    retained_result,
                });
            },
        )
        .size_full()
    }
}

struct NativeFontInstallationRoot {
    child: Entity<NativeFontInstallationChild>,
}

impl Render for NativeFontInstallationRoot {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().size_full().child(self.child.clone())
    }
}

fn wait_for_native_font_installation_submission(
    app: &mut Application,
    window: WindowHandle<NativeFontInstallationRoot>,
    paints: &RefCell<Vec<NativeFontInstallationPaint>>,
    resolved_font: FontId,
    retained: bool,
    previous_generation: u64,
) -> (u64, NativeFontInstallationPaint) {
    let mut submitted = None;
    pump_messages_until_with_delay(
        "native font installation accepted nonempty submission",
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
                .expect("the native font window must stay open until its frame is submitted");
            let paints = paints.borrow();
            for paint in paints.iter() {
                assert!(
                    paint.result.is_ok(),
                    "native ShapedLine::paint failed for {:?}: {:?}",
                    paint.shaped.text,
                    paint.result,
                );
                if let Some(result) = &paint.retained_result {
                    assert!(
                        result.is_ok(),
                        "retained native glyph paint failed: {result:?}",
                    );
                }
            }
            let Some(attempt) = facts.latest_present_attempt else {
                return false;
            };
            let Some(paint) = paints.last() else {
                return false;
            };
            if paint.shaped.text.as_ref() == SOURCE
                && paint.resolved_font == resolved_font
                && paint.retained.is_some() == retained
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
    submitted.expect("the painted font must belong to an accepted nonempty Submitted frame")
}

fn assert_native_font_layout(shaped: &ShapedLine, bounds: Bounds<Pixels>) {
    assert_eq!(shaped.text.as_ref(), SOURCE);
    assert_eq!(shaped.len(), SOURCE.len());
    assert!(shaped.width.as_f32().is_finite() && shaped.width > px(0.));
    assert!(shaped.width + px(32.) < bounds.size.width);
    assert!(bounds.size.height > px(112.));
    assert_eq!(shaped.x_for_index(SOURCE.len()), shaped.width);
    assert!(SOURCE.contains('\t') && SOURCE.contains('\u{2028}') && SOURCE.contains('\u{2029}'));
    assert!(SOURCE.chars().any(|character| character.len_utf8() == 4));
    let glyphs = shaped
        .runs
        .iter()
        .flat_map(|run| &run.glyphs)
        .collect::<Vec<_>>();
    assert!(!glyphs.is_empty());
    for glyph in &glyphs {
        assert!(glyph.index < SOURCE.len() && SOURCE.is_char_boundary(glyph.index));
        assert!(glyph.position.x.as_f32().is_finite());
        assert!(glyph.position.y.as_f32().is_finite());
    }
    let mut remainder = shaped.clone();
    let mut reconstructed = String::new();
    for (index, character) in SOURCE.char_indices() {
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
    assert_eq!(reconstructed, SOURCE);
    assert!(remainder.text.is_empty());
    assert_eq!(remainder.len(), 0);
}

fn glyph_at_byte(shaped: &ShapedLine, index: usize) -> (FontId, GlyphId) {
    shaped
        .runs
        .iter()
        .find_map(|run| {
            run.glyphs
                .iter()
                .find(|glyph| glyph.index == index)
                .map(|glyph| (run.font_id, glyph.id))
        })
        .expect("the native ASCII control must have its own shaped glyph")
}

#[test]
fn native_font_installation_replaces_fallback_after_child_notify_and_preserves_old_glyphs() {
    discard_stale_quit_messages();
    let (_observation_guard, observation) = begin_native_window_test_observation();
    let observation_cursor = observation.cursor();
    let platform = Rc::new(
        WindowsPlatform::new(false)
            .expect("the native font DirectWrite/DirectX platform must initialize"),
    );
    let platform_hwnd = platform.handle;
    let lifecycle_probe = platform.lifecycle_test_probe.clone();
    let platform_text_system = platform.text_system();
    let weak_platform_text_system = Arc::downgrade(&platform_text_system);
    assert!(
        platform_text_system.font_id(&font("Lilex")).is_err(),
        "native font installation prerequisite: Lilex must not already be installed in the OS font collection",
    );
    let mut app = Application::with_platform(platform.clone()).with_quit_mode(QuitMode::Explicit);
    let text_system = app.text_system();
    let weak_text_system = Arc::downgrade(&text_system);
    app.update_for_test(|cx| assert!(Arc::ptr_eq(cx.text_system(), &text_system)));
    text_system
        .add_fonts(vec![Cow::Borrowed(IBM_PLEX)])
        .expect("the application must install the checked-in IBM Plex fixture");
    assert!(
        platform_text_system.font_id(&font("Lilex")).is_err(),
        "the IBM Plex fixture must leave Lilex unavailable before its own installation",
    );
    let fallback_font = text_system.resolve_font(&font("Lilex"));
    let control_font = platform_text_system
        .font_id(&font("IBM Plex Sans"))
        .expect("the application's IBM Plex installation must reach its DirectWrite backend");
    assert_eq!(
        text_system.resolve_font(&font("IBM Plex Sans")),
        control_font
    );
    platform.begin_vsync_thread();

    let paints = Rc::new(RefCell::new(Vec::new()));
    let (child, window) = app.update_for_test(|cx| {
        let child = cx.new(|_| NativeFontInstallationChild {
            source: SOURCE.into(),
            retained: None,
            paints: paints.clone(),
        });
        let window = cx
            .open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::centered(size(px(640.), px(180.)), cx)),
                    focus_on_appearing: false,
                    activation_policy: WindowActivationPolicy {
                        accepts_activation: false,
                        focus_on_click: false,
                    },
                    accepts_pointer_input: false,
                    show: true,
                    ..WindowOptions::default()
                },
                |_, cx| {
                    cx.new(|_| NativeFontInstallationRoot {
                        child: child.clone(),
                    })
                },
            )
            .expect("the native font installation window must open");
        (child, window)
    });
    let registration = platform
        .raw_window_handles
        .read()
        .iter()
        .copied()
        .find(|registration| registration.window_id() == window.window_id())
        .expect("the native font window must own its exact HWND registration");
    let hwnd = registration.as_raw();
    let native_window = platform
        .window_from_hwnd(hwnd)
        .expect("the native font HWND must remain registered");
    assert_eq!(native_window.registration, registration);
    let native_facts = native_window
        .observed_platform_facts_for_test()
        .expect("native font presentation prerequisites must be observable from the HWND");
    assert!(!native_facts.accepts_pointer_input);
    assert!(!native_facts.accepts_activation);
    assert!(!native_facts.focus_on_click);
    let frame_requests = observe_frame_requests(&native_window);
    let initial_paint_cursor = observation.cursor();
    assert!(unsafe { RedrawWindow(Some(hwnd), None, None, RDW_INVALIDATE).as_bool() });
    pump_messages_until_with_delay(
        "native font installation initialization WM_PAINT",
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
    let (initial_generation, initial_paint) = wait_for_native_font_installation_submission(
        &mut app,
        window,
        &paints,
        fallback_font,
        false,
        0,
    );
    assert_eq!(initial_paint.result, Ok(()));
    assert!(initial_paint.retained_result.is_none());
    assert_native_font_layout(&initial_paint.shaped, initial_paint.canvas_bounds);
    assert_eq!(glyph_at_byte(&initial_paint.shaped, 0).0, fallback_font);
    assert_eq!(glyph_at_byte(&initial_paint.shaped, 1).0, control_font);
    let old_glyphs = [0, 1].map(|index| {
        let (font_id, glyph_id) = glyph_at_byte(&initial_paint.shaped, index);
        (
            font_id,
            glyph_id,
            platform_text_system
                .advance(font_id, glyph_id)
                .expect("the original native glyph must have measurable advance"),
            platform_text_system
                .typographic_bounds(font_id, glyph_id)
                .expect("the original native glyph must have measurable bounds"),
        )
    });

    text_system
        .add_fonts(vec![Cow::Borrowed(LILEX)])
        .expect("the existing application TextSystem must install the checked-in Lilex bytes");
    let installed_font = platform_text_system
        .font_id(&font("Lilex"))
        .expect("Lilex must be directly selectable after the application installs its bytes");
    assert_ne!(installed_font, fallback_font);
    assert_eq!(text_system.resolve_font(&font("Lilex")), installed_font);
    assert_eq!(
        text_system.resolve_font(&font("IBM Plex Sans")),
        control_font
    );
    for (font_id, glyph_id, advance, bounds) in old_glyphs {
        assert_eq!(
            platform_text_system
                .advance(font_id, glyph_id)
                .expect("font collection replacement must preserve the old glyph's font identity"),
            advance,
        );
        assert_eq!(
            platform_text_system
                .typographic_bounds(font_id, glyph_id)
                .expect("font collection replacement must preserve the old glyph's native face"),
            bounds,
        );
    }

    let paints_before = paints.borrow().len();
    let frame_requests_before = frame_requests.get();
    app.update_for_test(|cx| {
        child.update(cx, |child, cx| {
            assert_eq!(child.source.as_ref(), SOURCE);
            child.retained = Some(initial_paint.shaped.clone());
            cx.notify();
        });
    });
    let (changed_generation, changed_paint) = wait_for_native_font_installation_submission(
        &mut app,
        window,
        &paints,
        installed_font,
        true,
        initial_generation,
    );
    assert!(changed_generation > initial_generation);
    assert!(paints.borrow().len() > paints_before);
    assert!(frame_requests.get() > frame_requests_before);
    assert_eq!(changed_paint.result, Ok(()));
    assert_eq!(changed_paint.retained_result, Some(Ok(())));
    assert_native_font_layout(&changed_paint.shaped, changed_paint.canvas_bounds);
    assert_eq!(initial_paint.shaped.text, changed_paint.shaped.text);
    assert_eq!(glyph_at_byte(&changed_paint.shaped, 0).0, installed_font);
    assert_eq!(glyph_at_byte(&changed_paint.shaped, 1).0, control_font);
    let retained = changed_paint
        .retained
        .as_ref()
        .expect("the accepted installation frame must also paint the original shaped line");
    assert_native_font_layout(retained, changed_paint.canvas_bounds);
    assert!(Arc::ptr_eq(&**retained, &*initial_paint.shaped));
    assert_eq!(glyph_at_byte(retained, 0).0, fallback_font);
    assert_eq!(glyph_at_byte(retained, 1).0, control_font);
    drop(child);

    assert!(platform.raw_window_handles.read().contains(&registration));
    app.update_for_test(|cx| window.update(cx, |_, window, cx| window.remove_window(cx)))
        .expect("the exact native font installation window must close");
    pump_messages_until_with_delay(
        "exact native font installation registration retirement",
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
                    "the native font boundary must not swallow a panic",
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
        panic!("unexpected native font installation retirement sequence: {lifecycle:?}");
    };
    assert_eq!(quiesced, destroyed);
    assert_eq!(destroyed, terminal);
    assert!(app.update_for_test(|cx| cx.windows().is_empty()));
    assert!(platform.raw_window_handles.read().is_empty());
    drop(native_window);

    app.update_for_test(|cx| cx.shutdown());
    drop(text_system);
    drop(platform_text_system);
    drop(app);
    drop(platform);
    pump_messages_until_with_delay(
        "native font installation platform message window retirement",
        Duration::from_millis(5),
        || unsafe { !IsWindow(Some(platform_hwnd)).as_bool() },
    );
    assert_eq!(lifecycle_probe.ole_uninitialize_count(), 1);
    assert!(weak_text_system.upgrade().is_none());
    assert!(weak_platform_text_system.upgrade().is_none());
}
