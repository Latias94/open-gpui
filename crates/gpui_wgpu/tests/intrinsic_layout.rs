//! Public consumers of Zed 635026fabc38440fce5e1767d125026a17b12012:
//! b05f40c554 (constrained text caches and shaped-width truncation), b0da438545
//! (auto roots), cdf33ac25e (content-sized grids), and 5ccbbbd88f (row APIs).
//!
//! Cosmic shapes the tracked font and StyledText paints through HeadlessAppContext.
//! Frame acceptance here does not qualify native submission or compositor pixels.

#![cfg(not(target_family = "wasm"))]

use anyhow::Result;
use open_gpui::{
    AppContext as _, AvailableSpace, Bounds, Canvas, Context, Font, FontFeatures,
    HeadlessAppContext, IntoElement, ParentElement as _, Pixels, QuitMode, Render, Size,
    Styled as _, StyledText, TextOverflow, TextRenderingMode, TextRun, TruncateFrom, Window,
    WindowHandle, WindowId, canvas, div, fill, font, point, px, rgb, size,
};
use open_gpui_wgpu::CosmicTextSystem;
use std::{
    borrow::Cow,
    cell::{Cell, RefCell},
    rc::Rc,
    sync::Arc,
};

const IBM_PLEX: &[u8] = include_bytes!("../../../tests/fixtures/fonts/IBMPlexSans-Regular.ttf");
const SOURCE: &str = "AVATAR ToTo Cargo.toml";
const FONT_SIZE: f32 = 24.;

#[derive(Clone, Debug, PartialEq)]
struct PaintedText {
    direction: TruncateFrom,
    text: String,
    bounds: Bounds<Pixels>,
    shaped_width: Pixels,
}

#[derive(Clone, Debug)]
struct PaintedLayout {
    window_id: WindowId,
    version: usize,
    previous_revision: u64,
    root: Bounds<Pixels>,
    width: Pixels,
    oracle_width: Pixels,
    columns: Vec<Bounds<Pixels>>,
    rows: Vec<Bounds<Pixels>>,
    text: Vec<PaintedText>,
}

fn cell(paints: Rc<RefCell<Vec<Bounds<Pixels>>>>) -> Canvas<()> {
    canvas(
        |_, _, _| {},
        move |bounds, (), window, _| {
            window.paint_quad(fill(bounds, rgb(0x285c80)));
            paints.borrow_mut().push(bounds);
        },
    )
}

struct IntrinsicLayoutView {
    paints: Rc<RefCell<Vec<PaintedLayout>>>,
    narrow: bool,
    explicit_size: Option<Size<Pixels>>,
    version: usize,
}

impl Render for IntrinsicLayoutView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let descriptor = Font {
            features: FontFeatures::disable_ligatures(),
            ..font("IBM Plex Sans")
        };
        let oracle = window.text_system().shape_line(
            SOURCE.into(),
            px(FONT_SIZE),
            &[TextRun {
                len: SOURCE.len(),
                font: descriptor.clone(),
                ..Default::default()
            }],
            None,
        );
        let oracle_width = oracle.width;
        let isolated_width = SOURCE.chars().fold(px(0.), |width, character| {
            width
                + cx.text_system().layout_width(
                    cx.text_system().resolve_font(&descriptor),
                    px(FONT_SIZE),
                    character,
                )
        });
        assert!(oracle_width > px(80.));
        assert!(
            isolated_width > oracle_width.ceil(),
            "the fixture must distinguish per-character advances from real kerning"
        );
        let width = if self.narrow {
            (oracle_width * 0.55).floor()
        } else {
            oracle_width.ceil()
        };
        let columns = Rc::new(RefCell::new(Vec::new()));
        let rows = Rc::new(RefCell::new(Vec::new()));
        let text = Rc::new(RefCell::new(Vec::new()));
        let text_paints = text.clone();
        let paints = self.paints.clone();
        let version = self.version;
        let root = div()
            .relative()
            .flex()
            .flex_col()
            .font_family(descriptor.family.clone())
            .font_features(descriptor.features.clone())
            .text_size(px(FONT_SIZE))
            .line_height(px(36.))
            .child(
                div()
                    .w_full()
                    .flex_shrink_0()
                    .grid()
                    .grid_cols_max_content(3)
                    .children(
                        [100., 200., 50.]
                            .map(|width| cell(columns.clone()).w(px(width)).h(px(16.))),
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
                        [18., 45., 27.].map(|height| cell(rows.clone()).w_full().h(px(height))),
                    ),
            )
            .child(
                canvas(
                    move |bounds, window, cx| {
                        [TruncateFrom::End, TruncateFrom::Start, TruncateFrom::Middle]
                            .into_iter()
                            .enumerate()
                            .map(|(row, direction)| {
                                let label = StyledText::new(SOURCE);
                                let layout = label.layout().clone();
                                let overflow = match direction {
                                    TruncateFrom::End => TextOverflow::Truncate("…".into()),
                                    TruncateFrom::Start => TextOverflow::TruncateStart("…".into()),
                                    TruncateFrom::Middle => {
                                        TextOverflow::TruncateMiddle("…".into())
                                    }
                                };
                                let mut element = div()
                                    .min_w_0()
                                    .whitespace_nowrap()
                                    .text_overflow(overflow)
                                    .child(label)
                                    .into_any_element();
                                element.layout_as_root(
                                    size(
                                        AvailableSpace::Definite(px(40.)),
                                        AvailableSpace::MaxContent,
                                    ),
                                    window,
                                    cx,
                                );
                                assert!(layout.text().contains('…'));
                                let intrinsic = element.layout_as_root(
                                    size(AvailableSpace::MaxContent, AvailableSpace::MaxContent),
                                    window,
                                    cx,
                                );
                                assert_eq!(layout.text(), SOURCE, "{direction:?}: poisoned cache");
                                assert_eq!(intrinsic.width, oracle_width.ceil());
                                element.prepaint_as_root(
                                    bounds.origin + point(px(0.), px(row as f32 * 36.)),
                                    size(width, px(36.)).into(),
                                    window,
                                    cx,
                                );
                                (direction, element, layout)
                            })
                            .collect::<Vec<_>>()
                    },
                    move |_, labels, window, cx| {
                        for (direction, mut element, layout) in labels {
                            element.paint(window, cx);
                            let line = layout.line_layout_for_index(0).expect("painted text line");
                            let actual = layout.text();
                            assert_eq!(line.len(), actual.len());
                            assert!(line.wrap_boundaries().is_empty());
                            assert!(!line.runs().is_empty());
                            assert!(line.runs().iter().flat_map(|run| &run.glyphs).all(|glyph| {
                                glyph.index < actual.len() && actual.is_char_boundary(glyph.index)
                            }));
                            text_paints.borrow_mut().push(PaintedText {
                                direction,
                                text: actual,
                                bounds: layout.bounds(),
                                shaped_width: line.unwrapped_layout.width,
                            });
                        }
                    },
                )
                .w(width)
                .h(px(108.))
                .flex_shrink_0(),
            )
            .child(
                canvas(
                    |_, _, _| {},
                    move |root, (), window, _| {
                        paints.borrow_mut().push(PaintedLayout {
                            window_id: window.window_handle().window_id(),
                            version,
                            previous_revision: window.rendered_frame_revision(),
                            root,
                            width,
                            oracle_width,
                            columns: columns.borrow().clone(),
                            rows: rows.borrow().clone(),
                            text: text.borrow().clone(),
                        });
                    },
                )
                .absolute()
                .top_0()
                .left_0()
                .size_full(),
            );
        if let Some(explicit) = self.explicit_size {
            root.w(explicit.width).h(explicit.height)
        } else {
            root
        }
    }
}

fn observe_frame(
    app: &mut HeadlessAppContext,
    handle: WindowHandle<IntrinsicLayoutView>,
    paints: &RefCell<Vec<PaintedLayout>>,
    before: usize,
    expected_root: Size<Pixels>,
    version: usize,
) -> Result<(u64, PaintedLayout)> {
    let revision = app.update_window(handle.into(), |_, window, _| {
        let revision = window.rendered_frame_revision();
        assert_eq!(
            window.presentation_facts().frame_accepted_generation,
            Some(revision),
        );
        revision
    })?;
    let paints = paints.borrow();
    assert!(
        paints.len() > before,
        "the consumer must actually paint a new frame",
    );
    let paint = paints.last().expect("painted layout").clone();
    assert_eq!(paint.window_id, handle.window_id());
    assert_eq!(paint.previous_revision.checked_add(1), Some(revision));
    assert_eq!(paint.version, version);
    assert_eq!(
        paint.root,
        Bounds::new(point(px(0.), px(0.)), expected_root)
    );
    assert_eq!(paint.columns.len(), 3);
    assert_eq!(paint.rows.len(), 3);
    for (column, (x, width)) in paint
        .columns
        .iter()
        .zip([(0., 100.), (100., 200.), (300., 50.)])
    {
        assert_eq!(column.left(), px(x));
        assert_eq!(column.size, size(px(width), px(16.)));
    }
    for (row, (y, height)) in paint.rows.iter().zip([(16., 18.), (34., 45.), (79., 27.)]) {
        assert_eq!(row.top(), px(y));
        assert_eq!(row.size, size(expected_root.width, px(height)));
    }
    assert_eq!(paint.text.len(), 3);
    for (index, text) in paint.text.iter().enumerate() {
        assert_eq!(
            text.bounds.origin,
            point(px(0.), px(176. + index as f32 * 36.))
        );
        assert!(text.bounds.size.width <= paint.width + px(1.));
        assert!(text.shaped_width <= text.bounds.size.width + px(1.));
        assert_eq!(text.bounds.size.height, px(36.));
        if paint.width >= paint.oracle_width {
            assert_eq!(text.text, SOURCE);
            assert_eq!(text.shaped_width, paint.oracle_width);
        } else {
            let (front, back) = text
                .text
                .split_once('…')
                .expect("real overflow must truncate");
            assert!(SOURCE.starts_with(front) && SOURCE.ends_with(back));
            assert!(front.len() + back.len() < SOURCE.len());
            match text.direction {
                TruncateFrom::End => assert!(!front.is_empty() && back.is_empty()),
                TruncateFrom::Start => assert!(front.is_empty() && !back.is_empty()),
                TruncateFrom::Middle => assert!(!front.is_empty() && !back.is_empty()),
            }
        }
    }
    Ok((revision, paint))
}

#[test]
fn public_intrinsic_layout_survives_notify_and_explicit_root_reconstruction() -> Result<()> {
    let backend = Arc::new(CosmicTextSystem::new_without_system_fonts("IBM Plex Sans"));
    let backend_lifetime = Arc::downgrade(&backend);
    let mut app = HeadlessAppContext::new(backend.clone());
    drop(backend);
    app.update(|cx| {
        cx.set_quit_mode(QuitMode::Explicit);
        cx.set_text_rendering_mode(TextRenderingMode::Grayscale);
    });
    let global_lifetime = Arc::downgrade(app.text_system());
    app.text_system().add_fonts(vec![Cow::Borrowed(IBM_PLEX)])?;
    let paints = Rc::new(RefCell::new(Vec::new()));
    let viewport = size(px(800.), px(600.));
    let handle = app.open_window(viewport, {
        let paints = paints.clone();
        move |_, cx| {
            cx.new(|_| IntrinsicLayoutView {
                paints,
                narrow: false,
                explicit_size: None,
                version: 0,
            })
        }
    })?;
    let window_text = app.update_window(handle.into(), |_, window, _| {
        Rc::downgrade(window.text_system())
    })?;
    let root = handle.root(&mut app)?;
    let root_lifetime = root.downgrade();
    let notifications = Rc::new(Cell::new(0));
    let subscription = app.update(|cx| {
        let notifications = notifications.clone();
        cx.observe(&root, move |_, _| {
            notifications.set(notifications.get() + 1)
        })
    });
    let before = paints.borrow().len();
    app.update_window(handle.into(), |_, window, cx| window.draw(cx).clear(cx))?;
    let (initial_revision, initial) =
        observe_frame(&mut app, handle, &paints, before, viewport, 0)?;
    let before = paints.borrow().len();
    handle.update(&mut app, |view, _, cx| {
        view.narrow = true;
        view.version = 1;
        cx.notify();
    })?;
    app.run_until_parked();
    assert_eq!(notifications.get(), 1);
    let (revision, narrow) = observe_frame(&mut app, handle, &paints, before, viewport, 1)?;
    assert!(revision > initial_revision);
    assert!(narrow.width < initial.oracle_width);
    drop(subscription);
    drop(root);
    app.update_window(handle.into(), |_, window, cx| window.remove_window(cx))?;
    app.run_until_parked();
    assert!(app.update(|cx| cx.windows().is_empty()));
    assert!(root_lifetime.upgrade().is_none());
    assert!(window_text.upgrade().is_none());

    let explicit = size(px(360.), px(400.));
    let before = paints.borrow().len();
    let rebuilt = app.open_window(viewport, {
        let paints = paints.clone();
        move |_, cx| {
            cx.new(|_| IntrinsicLayoutView {
                paints,
                narrow: true,
                explicit_size: Some(explicit),
                version: 1,
            })
        }
    })?;
    assert_ne!(rebuilt.window_id(), handle.window_id());
    let rebuilt_text = app.update_window(rebuilt.into(), |_, window, cx| {
        window.draw(cx).clear(cx);
        Rc::downgrade(window.text_system())
    })?;
    let (_, reconstructed) = observe_frame(&mut app, rebuilt, &paints, before, explicit, 1)?;
    assert_eq!(reconstructed.text, narrow.text);
    assert_eq!(reconstructed.columns, narrow.columns);
    app.update_window(rebuilt.into(), |_, window, cx| window.remove_window(cx))?;
    app.run_until_parked();
    assert!(app.update(|cx| cx.windows().is_empty()));
    assert!(rebuilt_text.upgrade().is_none());
    drop(app);
    assert!(global_lifetime.upgrade().is_none());
    assert!(backend_lifetime.upgrade().is_none());
    Ok(())
}
