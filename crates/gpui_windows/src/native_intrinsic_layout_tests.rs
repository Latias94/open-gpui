//! Native consumer of Zed 635026fabc38440fce5e1767d125026a17b12012:
//! b05f40c554 (text), b0da438545 (roots), cdf33ac25e (grid), 5ccbbbd88f (rows).
//! Qualifies real layout/paint, accepted nonempty Submitted frames, notification,
//! reconstruction, and exact native retirement; not compositor pixels or performance.

use super::*;
use crate::native_test_observation::{
    NativeWindowTestMessageDisposition, NativeWindowTestObservation,
};
use open_gpui::{
    Canvas, FontFeatures, Pixels, Size, StyledText, TextOverflow, TextRun, TruncateFrom,
    WindowHandle, WindowInitialPresentationStatus, fill, font, rgb,
};

const SOURCE: &str = "AVATAR ToTo Cargo.toml";
const IBM_PLEX: &[u8] = include_bytes!("../../../tests/fixtures/fonts/IBMPlexSans-Regular.ttf");

#[derive(Clone, Debug, PartialEq)]
struct NativeIntrinsicText {
    direction: TruncateFrom,
    text: String,
    bounds: Bounds<Pixels>,
    shaped_width: Pixels,
}

#[derive(Clone, Debug)]
struct NativeIntrinsicPaint {
    window_id: WindowId,
    version: usize,
    previous_revision: u64,
    root: Bounds<Pixels>,
    viewport: Size<Pixels>,
    width: Pixels,
    oracle_width: Pixels,
    columns: Vec<Bounds<Pixels>>,
    rows: Vec<Bounds<Pixels>>,
    text: Vec<NativeIntrinsicText>,
}

fn intrinsic_cell(paints: Rc<RefCell<Vec<Bounds<Pixels>>>>) -> Canvas<()> {
    canvas(
        |_, _, _| {},
        move |bounds, (), window, _| {
            window.paint_quad(fill(bounds, rgb(0x285c80)));
            paints.borrow_mut().push(bounds);
        },
    )
}

struct NativeIntrinsicRoot {
    paints: Rc<RefCell<Vec<NativeIntrinsicPaint>>>,
    narrow: bool,
    explicit_size: bool,
    version: usize,
}

impl Render for NativeIntrinsicRoot {
    fn render(&mut self, window: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let mut descriptor = font("IBM Plex Sans");
        descriptor.features = FontFeatures(Arc::new(vec![
            ("kern".into(), 1),
            ("liga".into(), 0),
            ("clig".into(), 0),
            ("calt".into(), 0),
        ]));
        let runs = [TextRun {
            len: SOURCE.len(),
            font: descriptor.clone(),
            ..Default::default()
        }];
        let oracle = window
            .text_system()
            .shape_line(SOURCE.into(), px(24.), &runs, None);
        let oracle_width = oracle.width;
        assert!(oracle_width > px(80.));
        let scale = window.scale_factor();
        let exact_width = (oracle_width * scale).ceil() / scale;
        let isolated = SOURCE.chars().fold(px(0.), |width, character| {
            width
                + window.text_system().layout_width(
                    window.text_system().resolve_font(&descriptor),
                    px(24.),
                    character,
                )
        });
        assert!(
            isolated > exact_width,
            "the native descriptor must exercise kerning: {isolated:?} <= {exact_width:?}"
        );
        let mut wrapper = window
            .text_system()
            .line_wrapper(descriptor.clone(), px(24.));
        for direction in [TruncateFrom::End, TruncateFrom::Start, TruncateFrom::Middle] {
            let (unguarded, _) =
                wrapper.truncate_line(SOURCE.into(), exact_width, "…", &runs, direction);
            assert_ne!(
                unguarded.as_ref(),
                SOURCE,
                "{direction:?} must require the shaped-width guard"
            );
        }
        let width = if self.narrow {
            (oracle_width * (0.55 * scale)).floor() / scale
        } else {
            exact_width
        };
        let columns = Rc::new(RefCell::new(Vec::new()));
        let rows = Rc::new(RefCell::new(Vec::new()));
        let mut layouts = Vec::new();
        let labels =
            [TruncateFrom::End, TruncateFrom::Start, TruncateFrom::Middle].map(|direction| {
                let label = StyledText::new(SOURCE);
                layouts.push((direction, label.layout().clone()));
                let overflow = match direction {
                    TruncateFrom::End => TextOverflow::Truncate("…".into()),
                    TruncateFrom::Start => TextOverflow::TruncateStart("…".into()),
                    TruncateFrom::Middle => TextOverflow::TruncateMiddle("…".into()),
                };
                div()
                    .w(width)
                    .min_w_0()
                    .flex_shrink_0()
                    .whitespace_nowrap()
                    .text_overflow(overflow)
                    .child(label)
            });
        let paints = self.paints.clone();
        let version = self.version;
        let root = div()
            .relative()
            .flex()
            .flex_col()
            .font_family(descriptor.family)
            .font_features(descriptor.features)
            .text_size(px(24.))
            .text_color(white())
            .line_height(px(36.))
            .child(
                div()
                    .w_full()
                    .flex_shrink_0()
                    .grid()
                    .grid_cols_max_content(3)
                    .children(
                        [100., 200., 50.]
                            .map(|width| intrinsic_cell(columns.clone()).w(px(width)).h(px(16.))),
                    ),
            )
            .child(
                div()
                    .w_full()
                    .h(px(160.))
                    .flex_shrink_0()
                    .grid()
                    .grid_cols(1)
                    .grid_rows_max_content(3)
                    .children(
                        [18., 45., 27.]
                            .map(|height| intrinsic_cell(rows.clone()).w_full().h(px(height))),
                    ),
            )
            .children(labels)
            .child(
                canvas(
                    |_, _, _| {},
                    move |root, (), window, _| {
                        let text = layouts
                            .iter()
                            .map(|(direction, layout)| {
                                let actual = layout.text();
                                let line =
                                    layout.line_layout_for_index(0).expect("native text line");
                                assert_eq!(line.len(), actual.len());
                                assert!(line.wrap_boundaries().is_empty());
                                assert!(!line.runs().is_empty());
                                assert!(line.runs().iter().flat_map(|run| &run.glyphs).all(
                                    |glyph| {
                                        glyph.index < actual.len()
                                            && actual.is_char_boundary(glyph.index)
                                    }
                                ));
                                NativeIntrinsicText {
                                    direction: *direction,
                                    text: actual,
                                    bounds: layout.bounds(),
                                    shaped_width: line.unwrapped_layout.width,
                                }
                            })
                            .collect();
                        paints.borrow_mut().push(NativeIntrinsicPaint {
                            window_id: window.window_handle().window_id(),
                            version,
                            previous_revision: window.rendered_frame_revision(),
                            root,
                            viewport: window.viewport_size(),
                            width,
                            oracle_width,
                            columns: columns.borrow().clone(),
                            rows: rows.borrow().clone(),
                            text,
                        });
                    },
                )
                .absolute()
                .top_0()
                .left_0()
                .size_full(),
            );
        if self.explicit_size {
            root.w(px(360.)).h(px(400.))
        } else {
            root
        }
    }
}

fn open_intrinsic_window(
    app: &mut Application,
    paints: &Rc<RefCell<Vec<NativeIntrinsicPaint>>>,
    reconstructed: bool,
) -> WindowHandle<NativeIntrinsicRoot> {
    app.update_for_test(|cx| {
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::centered(size(px(800.), px(600.)), cx)),
                focus_on_appearing: false,
                show: true,
                ..WindowOptions::default()
            },
            |_, cx| {
                cx.new(|_| NativeIntrinsicRoot {
                    paints: paints.clone(),
                    narrow: reconstructed,
                    explicit_size: reconstructed,
                    version: usize::from(reconstructed),
                })
            },
        )
    })
    .expect("the native intrinsic layout window must open")
}

fn wait_for_intrinsic_submission(
    platform: &WindowsPlatform,
    app: &mut Application,
    handle: WindowHandle<NativeIntrinsicRoot>,
    paints: &RefCell<Vec<NativeIntrinsicPaint>>,
    version: usize,
    previous: u64,
) -> (u64, NativeIntrinsicPaint) {
    let mut submitted = None;
    pump_messages_until_with_delay(
        &Rc::downgrade(&platform.inner),
        "native intrinsic layout accepted nonempty submission",
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
                                .map(|d| d.generation()),
                        )
                    })
                })
                .expect("the intrinsic layout window must stay open until submission");
            let paints = paints.borrow();
            let Some(paint) = paints.last() else {
                return false;
            };
            let Some(attempt) = facts.latest_present_attempt else {
                return false;
            };
            if paint.window_id == handle.window_id()
                && paint.version == version
                && paint.previous_revision.checked_add(1) == Some(attempt.generation)
                && attempt.generation > previous
                && attempt.outcome == PlatformWindowPresentOutcome::Submitted
                && attempt.contained_valid_primitives
                && facts.initial_presentation == WindowInitialPresentationStatus::Completed
                && facts.native_visible
                && facts.frame_accepted_generation == Some(attempt.generation)
                && facts.present_submitted_generation == Some(attempt.generation)
                && facts.non_empty_presented_generation == Some(attempt.generation)
                && revision == attempt.generation
                && diagnostics == Some(attempt.generation)
            {
                submitted = Some((revision, paint.clone()));
                true
            } else {
                false
            }
        },
    );
    submitted.expect("the actual painted geometry must belong to a nonempty Submitted frame")
}

fn assert_intrinsic_geometry(paint: &NativeIntrinsicPaint, explicit: bool) {
    let close = |actual: Pixels, expected: Pixels| {
        assert!(
            (actual - expected).abs() <= px(1.),
            "{actual:?} != {expected:?}"
        );
    };
    let root_size = if explicit {
        size(px(360.), px(400.))
    } else {
        paint.viewport
    };
    close(paint.root.left(), px(0.));
    close(paint.root.top(), px(0.));
    close(paint.root.size.width, root_size.width);
    close(paint.root.size.height, root_size.height);
    assert_eq!(paint.columns.len(), 3);
    assert_eq!(paint.rows.len(), 3);
    for (column, (x, width)) in paint
        .columns
        .iter()
        .zip([(0., 100.), (100., 200.), (300., 50.)])
    {
        close(column.left(), px(x));
        close(column.size.width, px(width));
        close(column.size.height, px(16.));
    }
    for (row, (y, height)) in paint.rows.iter().zip([(16., 18.), (34., 45.), (79., 27.)]) {
        close(row.top(), px(y));
        close(row.size.width, root_size.width);
        close(row.size.height, px(height));
    }
    assert_eq!(paint.text.len(), 3);
    for (index, text) in paint.text.iter().enumerate() {
        close(text.bounds.top(), px(176. + index as f32 * 36.));
        close(text.bounds.size.height, px(36.));
        assert!(text.bounds.size.width <= paint.width + px(1.));
        assert!(text.shaped_width <= text.bounds.size.width + px(1.));
        if paint.width >= paint.oracle_width {
            assert_eq!(text.text, SOURCE);
            close(text.shaped_width, paint.oracle_width);
        } else {
            let (front, back) = text.text.split_once('…').expect("native overflow affix");
            assert!(SOURCE.starts_with(front) && SOURCE.ends_with(back));
            assert!(front.len() + back.len() < SOURCE.len());
            match text.direction {
                TruncateFrom::End => assert!(!front.is_empty() && back.is_empty()),
                TruncateFrom::Start => assert!(front.is_empty() && !back.is_empty()),
                TruncateFrom::Middle => assert!(!front.is_empty() && !back.is_empty()),
            }
        }
    }
}

fn close_intrinsic_window(
    app: &mut Application,
    platform: &WindowsPlatform,
    handle: WindowHandle<NativeIntrinsicRoot>,
    registration: RegisteredWindow,
    observation: &NativeWindowTestObservation,
) {
    assert!(platform.raw_window_handles.read().contains(&registration));
    assert_eq!(registration.window_id(), handle.window_id());
    app.update_for_test(|cx| handle.update(cx, |_, window, cx| window.remove_window(cx)))
        .expect("the exact intrinsic layout window must close");
    pump_messages_until_with_delay(
        &Rc::downgrade(&platform.inner),
        "exact intrinsic layout native retirement",
        Duration::from_millis(5),
        || {
            !unsafe { IsWindow(Some(registration.as_raw())).as_bool() }
                && !platform.raw_window_handles.read().contains(&registration)
                && platform.inner.native_retirement_is_settled()
        },
    );
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
        panic!("unexpected intrinsic layout retirement sequence: {lifecycle:?}");
    };
    assert_eq!(quiesced, destroyed);
    assert_eq!(destroyed, terminal);
    assert!(app.update_for_test(|cx| cx.windows().is_empty()));
    assert!(platform.raw_window_handles.read().is_empty());
}

#[test]
fn native_intrinsic_layout_submits_notified_geometry_and_releases_exact_owners() {
    discard_stale_quit_messages();
    let (_guard, observation) = begin_native_window_test_observation();
    let platform =
        Rc::new(WindowsPlatform::new_windowed_for_test().expect("native intrinsic platform"));
    let platform_hwnd = platform.handle;
    let platform_lifetime = Rc::downgrade(&platform);
    let lifecycle_probe = platform.lifecycle_test_probe.clone();
    let backend_lifetime = Arc::downgrade(&platform.text_system());
    let mut app = Application::with_platform(platform.clone()).with_quit_mode(QuitMode::Explicit);
    app.text_system()
        .add_fonts(vec![std::borrow::Cow::Borrowed(IBM_PLEX)])
        .expect("the native intrinsic fixture must install its exact font");
    let global_lifetime = Arc::downgrade(&app.text_system());
    platform
        .begin_vsync_thread()
        .expect("native vsync provider should start");
    let paints = Rc::new(RefCell::new(Vec::new()));
    let handle = open_intrinsic_window(&mut app, &paints, false);
    let registration = platform
        .raw_window_handles
        .read()
        .iter()
        .copied()
        .find(|entry| entry.window_id() == handle.window_id())
        .expect("exact initial registration");
    let native_window = platform
        .window_from_hwnd(registration.as_raw())
        .expect("initial HWND");
    assert_eq!(native_window.registration, registration);
    let frame_requests = observe_frame_requests(&native_window);
    let window_text = app
        .update_for_test(|cx| handle.update(cx, |_, window, _| Rc::downgrade(window.text_system())))
        .expect("initial text cache");
    let root = app
        .update_for_test(|cx| handle.root(cx))
        .expect("initial root");
    let root_lifetime = root.downgrade();
    let notifications = Rc::new(Cell::new(0));
    let subscription = app.update_for_test(|cx| {
        let notifications = notifications.clone();
        cx.observe(&root, move |_, _| {
            notifications.set(notifications.get() + 1)
        })
    });
    let (initial_generation, initial) =
        wait_for_intrinsic_submission(&platform, &mut app, handle, &paints, 0, 0);
    assert_intrinsic_geometry(&initial, false);
    let before = paints.borrow().len();
    let requests_before = frame_requests.get();
    app.update_for_test(|cx| {
        handle.update(cx, |view, _, cx| {
            view.narrow = true;
            view.version = 1;
            cx.notify();
        })
    })
    .expect("notify the existing intrinsic layout root");
    let (_, narrow) =
        wait_for_intrinsic_submission(&platform, &mut app, handle, &paints, 1, initial_generation);
    assert_eq!(notifications.get(), 1);
    assert!(paints.borrow().len() > before);
    assert!(frame_requests.get() > requests_before);
    assert!(narrow.width < initial.oracle_width);
    assert_intrinsic_geometry(&narrow, false);
    drop(subscription);
    drop(root);
    close_intrinsic_window(&mut app, &platform, handle, registration, &observation);
    drop(native_window);
    assert!(root_lifetime.upgrade().is_none());
    assert!(window_text.upgrade().is_none());

    let before = paints.borrow().len();
    let rebuilt = open_intrinsic_window(&mut app, &paints, true);
    assert_ne!(rebuilt.window_id(), handle.window_id());
    let rebuilt_registration = platform
        .raw_window_handles
        .read()
        .iter()
        .copied()
        .find(|entry| entry.window_id() == rebuilt.window_id())
        .expect("exact rebuilt registration");
    let rebuilt_text = app
        .update_for_test(|cx| {
            rebuilt.update(cx, |_, window, _| Rc::downgrade(window.text_system()))
        })
        .expect("rebuilt text cache");
    let (_, reconstructed) =
        wait_for_intrinsic_submission(&platform, &mut app, rebuilt, &paints, 1, 0);
    assert!(paints.borrow().len() > before);
    assert_intrinsic_geometry(&reconstructed, true);
    assert_eq!(reconstructed.text, narrow.text);
    assert_eq!(reconstructed.columns, narrow.columns);
    close_intrinsic_window(
        &mut app,
        &platform,
        rebuilt,
        rebuilt_registration,
        &observation,
    );
    assert!(rebuilt_text.upgrade().is_none());

    app.update_for_test(|cx| cx.shutdown());
    drop(app);
    let pump_platform = Rc::downgrade(&platform.inner);
    drop(platform);
    pump_messages_until_with_delay(
        &pump_platform,
        "intrinsic layout platform message HWND retirement",
        Duration::from_millis(5),
        || unsafe { !IsWindow(Some(platform_hwnd)).as_bool() },
    );
    assert!(platform_lifetime.upgrade().is_none());
    assert_eq!(lifecycle_probe.ole_uninitialize_count(), 1);
    assert!(global_lifetime.upgrade().is_none());
    assert!(backend_lifetime.upgrade().is_none());
}
