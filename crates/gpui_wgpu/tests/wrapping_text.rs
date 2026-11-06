#![cfg(not(target_family = "wasm"))]

use anyhow::Result;
use open_gpui::{
    AppContext as _, Context, Font, FontFeatures, GlyphId, HeadlessAppContext, IndentAdjustment,
    IntoElement, LineFragment, ParentElement as _, Pixels, QuitMode, Render, Styled as _,
    StyledText, TextAlign, TextOverflow, TextRenderingMode, TextRun, TextSystem, TruncateFrom,
    Window, WindowHandle, canvas, div, font, px, rgb, size,
};
use open_gpui_wgpu::CosmicTextSystem;
use std::{
    borrow::Cow,
    cell::{Cell, RefCell},
    rc::Rc,
    sync::Arc,
};
use unicode_segmentation::UnicodeSegmentation as _;

const LILEX: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/fonts/Lilex-Regular.ttf"
));
const IBM_PLEX: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/fonts/IBMPlexSans-Regular.ttf"
));
const NOTO_EMOJI: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/fonts/NotoEmoji-Regular.ttf"
));
const SOURCE: &str = concat!(
    "  alpha/beta café e\u{301} 😀你好、世界 👩\u{200d}💻 👍🏽 🇯🇵 omega\n",
    "  second (paragraph) keeps its original UTF-8 bytes.",
);
const FILENAME: &str = "préfix_abcdefghijklmnopqrstuvwxyz_0123456789_你好.rs";

fn filename_runs(base: &Font) -> Vec<TextRun> {
    [
        ("préfix_", base.clone(), 0x123456),
        (
            "abcdefghijklmnopqrstuvwxyz_0123456789_",
            Font {
                features: base.features.clone(),
                ..font("IBM Plex Sans")
            },
            0x654321,
        ),
        ("你好.rs", base.clone(), 0x246810),
    ]
    .into_iter()
    .map(|(text, font, color)| TextRun {
        len: text.len(),
        font,
        color: rgb(color).into(),
        ..Default::default()
    })
    .collect()
}

fn assert_middle_runs(original: &[TextRun], text: &str, adjusted: &[TextRun]) {
    assert_eq!(
        adjusted.iter().map(|run| run.len).sum::<usize>(),
        text.len()
    );
    if text == FILENAME {
        assert_eq!(adjusted, original);
        return;
    }
    let (front, back) = text.split_once('…').expect("middle truncation affix");
    assert!(FILENAME.starts_with(front));
    assert!(FILENAME.ends_with(back));
    assert!(front.len() + back.len() < FILENAME.len());
    assert!(FILENAME.is_char_boundary(front.len()));
    assert!(FILENAME.is_char_boundary(FILENAME.len() - back.len()));
    let byte_styles = |runs: &[TextRun]| {
        runs.iter()
            .flat_map(|run| {
                let mut style = run.clone();
                style.len = 1;
                std::iter::repeat_n(style, run.len)
            })
            .collect::<Vec<_>>()
    };
    let original = byte_styles(original);
    let mut expected = original[..front.len()].to_vec();
    expected.extend(std::iter::repeat_n(
        original[front.len()].clone(),
        '…'.len_utf8(),
    ));
    expected.extend_from_slice(&original[FILENAME.len() - back.len()..]);
    assert_eq!(byte_styles(adjusted), expected);
}

fn assert_public_wrapper(text_system: &Arc<TextSystem>, font: &Font, font_size: Pixels) {
    let space = text_system.layout_width(text_system.resolve_font(font), font_size, ' ');
    assert!(space > px(0.));
    let mut wrapper = text_system.line_wrapper(font.clone(), font_size);
    let fragments = [
        LineFragment::text("a e"),
        LineFragment::text("\u{301}"),
        LineFragment::text("😀👩"),
        LineFragment::text("\u{200d}💻👍"),
        LineFragment::text("🏽🇯🇵か"),
        LineFragment::text("\u{3099}終"),
    ];
    let text = "a e\u{301}😀👩\u{200d}💻👍🏽🇯🇵か\u{3099}終";
    let graphemes = text
        .grapheme_indices(true)
        .map(|(index, _)| index)
        .collect::<Vec<_>>();
    for width in [px(0.01), space * 0.5, space * 2.] {
        let whole = wrapper
            .wrap_line(
                &[LineFragment::text(text)],
                width,
                IndentAdjustment::NoIndent,
            )
            .collect::<Vec<_>>();
        let fragmented = wrapper
            .wrap_line(&fragments, width, IndentAdjustment::NoIndent)
            .collect::<Vec<_>>();
        assert!(!whole.is_empty());
        assert_eq!(fragmented, whole);
        assert!(
            whole
                .iter()
                .all(|boundary| boundary.ix > 0 && graphemes.contains(&boundary.ix))
        );
        assert!(whole.windows(2).all(|pair| pair[0].ix < pair[1].ix));
    }
    for (element_columns, expected) in [(2., 6), (1., 7)] {
        let fragments = [
            LineFragment::text("ab"),
            LineFragment::element(space * element_columns, "😀".len()),
            LineFragment::text("cd"),
        ];
        let boundaries = wrapper
            .wrap_line(
                &fragments,
                space * 4. + px(0.25),
                IndentAdjustment::NoIndent,
            )
            .map(|boundary| boundary.ix)
            .collect::<Vec<_>>();
        assert_eq!(boundaries, [expected]);
    }
    assert_eq!(IndentAdjustment::default(), IndentAdjustment::SameIndent);
    for (policy, expected_indent) in [
        (IndentAdjustment::NoIndent, 0),
        (IndentAdjustment::SameIndent, 2),
        (IndentAdjustment::ExtraColumns(2), 4),
        (IndentAdjustment::ExtraColumns(u32::MAX), 0),
    ] {
        let fragments = [LineFragment::text("  ab cd ef gh")];
        let boundaries = wrapper
            .wrap_line(&fragments, space * 10., policy)
            .collect::<Vec<_>>();
        assert!(!boundaries.is_empty());
        assert!(
            boundaries
                .iter()
                .all(|boundary| boundary.next_indent == expected_indent)
        );
    }
}

#[derive(Clone, Debug, PartialEq)]
struct PaintedText {
    version: usize,
    width: Pixels,
    line_widths: Vec<Pixels>,
    wrap_indices: Vec<Vec<usize>>,
    middle: String,
}

impl PaintedText {
    fn rows(&self) -> usize {
        self.wrap_indices
            .iter()
            .map(|indices| indices.len() + 1)
            .sum()
    }
}

struct WrappingView {
    font: Font,
    paints: Rc<RefCell<Vec<PaintedText>>>,
    width: Pixels,
    font_size: Pixels,
    version: usize,
}

impl Render for WrappingView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let font = self.font.clone();
        let row_font = font.clone();
        let font_size = self.font_size;
        let version = self.version;
        let paints = self.paints.clone();
        let label = StyledText::new(FILENAME).with_runs(filename_runs(&font));
        let label_layout = label.layout().clone();
        let tiny = StyledText::new(FILENAME).with_runs(filename_runs(&font));
        let tiny_layout = tiny.layout().clone();
        let start = StyledText::new("aaaa-bbbb-cccc");
        let start_layout = start.layout().clone();
        let end = StyledText::new("a-abcdefghijklm");
        let end_layout = end.layout().clone();
        let space =
            cx.text_system()
                .layout_width(cx.text_system().resolve_font(&font), font_size, ' ');
        let mut paragraph_layouts = Vec::new();
        let paragraph_controls = [
            ("a\nb", TruncateFrom::Start, 4., 1, Some("…b")),
            ("a\nb", TruncateFrom::End, 4., 1, Some("a…")),
            ("a\nb", TruncateFrom::Middle, 4., 1, Some("a…b")),
            (
                "first\nsecond\nlast",
                TruncateFrom::Middle,
                10.,
                3,
                Some("first…last"),
            ),
            ("     aaaaaaa", TruncateFrom::Start, 7., 2, None),
        ]
        .into_iter()
        .map(|(source, direction, columns, max_lines, expected)| {
            let width = space * columns + px(0.01);
            let runs = source
                .chars()
                .enumerate()
                .map(|(index, character)| TextRun {
                    len: character.len_utf8(),
                    font: font.clone(),
                    color: rgb(if index % 2 == 0 { 0x123456 } else { 0x654321 }).into(),
                    ..Default::default()
                })
                .collect::<Vec<_>>();
            let label = StyledText::new(source).with_runs(runs.clone());
            paragraph_layouts.push((
                source,
                direction,
                width,
                max_lines,
                expected,
                runs,
                label.layout().clone(),
            ));
            let overflow = match direction {
                TruncateFrom::Start => TextOverflow::TruncateStart("…".into()),
                TruncateFrom::End => TextOverflow::Truncate("…".into()),
                TruncateFrom::Middle => TextOverflow::TruncateMiddle("…".into()),
            };
            div()
                .w(width)
                .flex_shrink_0()
                .line_clamp(max_lines)
                .text_overflow(overflow)
                .child(label)
        })
        .collect::<Vec<_>>();
        div()
            .flex()
            .flex_col()
            .size_full()
            .font_family(font.family.clone())
            .font_features(font.features.clone())
            .text_size(font_size)
            .child(
                div()
                    .w(self.width)
                    .flex_shrink_0()
                    .overflow_hidden()
                    .whitespace_nowrap()
                    .text_ellipsis_middle()
                    .child(label),
            )
            .child(
                div()
                    .w(px(1.))
                    .flex_shrink_0()
                    .line_clamp(2)
                    .text_overflow(TextOverflow::TruncateMiddle("~".into()))
                    .child(tiny),
            )
            .child(
                div()
                    .flex()
                    .gap_2()
                    .child(
                        div()
                            .w(space * 7. + px(0.01))
                            .flex_shrink_0()
                            .line_clamp(2)
                            .text_ellipsis_start()
                            .child(start),
                    )
                    .child(
                        div()
                            .w(space * 10. + px(0.01))
                            .flex_shrink_0()
                            .line_clamp(2)
                            .text_ellipsis()
                            .child(end),
                    ),
            )
            .child(div().flex().gap_2().children(paragraph_controls))
            .child(
                canvas(
                    move |bounds, window, cx| {
                        let text_system = cx.text_system().clone();
                        let window_text: &TextSystem = window.text_system();
                        assert!(std::ptr::eq(window_text, text_system.as_ref()));
                        if version == 0 {
                            assert_public_wrapper(&text_system, &font, font_size);
                        }
                        let lines = window
                            .text_system()
                            .shape_text(
                                SOURCE.into(),
                                font_size,
                                &[TextRun {
                                    len: SOURCE.len(),
                                    font: font.clone(),
                                    ..Default::default()
                                }],
                                Some(bounds.size.width),
                                None,
                            )
                            .expect("the original Unicode source must shape");
                        assert_eq!(
                            lines
                                .iter()
                                .map(|line| line.text.as_ref())
                                .collect::<Vec<_>>()
                                .join("\n"),
                            SOURCE,
                        );
                        let runs = filename_runs(&font);
                        let mut wrapper = text_system.line_wrapper(font, font_size);
                        let (text, adjusted) = wrapper.truncate_line(
                            FILENAME.into(),
                            bounds.size.width,
                            "…",
                            &runs,
                            TruncateFrom::Middle,
                        );
                        assert_middle_runs(&runs, &text, &adjusted);
                        let (tiny_text, tiny_runs) = wrapper.truncate_line(
                            FILENAME.into(),
                            px(1.),
                            "…",
                            &runs,
                            TruncateFrom::Middle,
                        );
                        assert_eq!(tiny_text.as_ref(), "…");
                        assert_middle_runs(&runs, &tiny_text, &tiny_runs);
                        let middle = window
                            .text_system()
                            .shape_line(text, font_size, &adjusted, None);
                        (lines, middle)
                    },
                    move |bounds, (lines, middle), window, cx| {
                        assert_eq!(label_layout.text(), middle.text.as_ref());
                        assert_eq!(tiny_layout.text(), "~");
                        assert_eq!(label_layout.len(), middle.len());
                        assert!(start_layout.text().starts_with('…'));
                        assert!(start_layout.text().ends_with("cccc"));
                        assert_eq!(end_layout.text(), "a-abcdefghi…");
                        for (layout, width) in [
                            (&start_layout, space * 7. + px(0.01)),
                            (&end_layout, space * 10. + px(0.01)),
                        ] {
                            let wrapped_text = layout.wrapped_text();
                            assert!(wrapped_text.split('\n').count() <= 2);
                            for row in wrapped_text.split('\n') {
                                let shaped = window.text_system().shape_line(
                                    row.to_owned().into(),
                                    font_size,
                                    &[TextRun {
                                        len: row.len(),
                                        font: row_font.clone(),
                                        ..Default::default()
                                    }],
                                    None,
                                );
                                assert!(shaped.width <= width + px(0.01), "{row:?}");
                            }
                        }
                        for (source, direction, width, max_lines, expected, runs, layout) in
                            &paragraph_layouts
                        {
                            let actual = layout.text();
                            if let Some(expected) = expected {
                                assert_eq!(actual, *expected, "{source:?}: {direction:?}");
                                assert_eq!(layout.wrapped_text(), *expected);
                            } else {
                                let retained = actual.strip_prefix('…').expect("start affix");
                                assert!(source.ends_with(retained));
                                assert!(
                                    retained.graphemes(true).count()
                                        < source.graphemes(true).count()
                                );
                            }
                            let mut wrapper = window
                                .text_system()
                                .line_wrapper(row_font.clone(), font_size);
                            let (text, adjusted) = wrapper.truncate_wrapped_line(
                                (*source).into(),
                                *width,
                                *max_lines,
                                "…",
                                runs,
                                *direction,
                            );
                            assert_eq!(actual, text.as_ref());
                            assert_eq!(layout.len(), text.len());
                            assert_eq!(
                                adjusted.iter().map(|run| run.len).sum::<usize>(),
                                text.len()
                            );
                            let mut offset = 0;
                            for run in adjusted.iter() {
                                assert!(run.len > 0);
                                assert!(text.is_char_boundary(offset));
                                offset += run.len;
                                assert!(text.is_char_boundary(offset));
                            }
                            let (front, back) = actual.split_once('…').expect("paragraph affix");
                            assert!(source.starts_with(front));
                            assert!(source.ends_with(back));
                            let adjusted_bytes = adjusted
                                .iter()
                                .flat_map(|run| {
                                    let mut style = run.clone();
                                    style.len = 1;
                                    std::iter::repeat_n(style, run.len)
                                })
                                .collect::<Vec<_>>();
                            assert_eq!(&adjusted_bytes[..front.len()], &runs[..front.len()]);
                            assert_eq!(
                                &adjusted_bytes[actual.len() - back.len()..],
                                &runs[source.len() - back.len()..]
                            );
                            let shaped = window
                                .text_system()
                                .shape_text(text, font_size, &adjusted, Some(*width), None)
                                .expect("clamped public text must shape without a row limit");
                            assert_eq!(
                                shaped
                                    .iter()
                                    .map(|line| line.text.as_ref())
                                    .collect::<Vec<_>>()
                                    .join("\n"),
                                actual
                            );
                            let rows = shaped
                                .iter()
                                .map(|line| line.wrap_boundaries().len() + 1)
                                .sum::<usize>();
                            assert!(rows <= *max_lines, "{source:?}: {direction:?}");
                            let wrapped = layout.wrapped_text();
                            assert_eq!(wrapped.split('\n').count(), rows);
                            for row in wrapped.split('\n') {
                                let shaped = window.text_system().shape_line(
                                    row.to_owned().into(),
                                    font_size,
                                    &[TextRun {
                                        len: row.len(),
                                        font: row_font.clone(),
                                        ..Default::default()
                                    }],
                                    None,
                                );
                                assert!(shaped.width <= *width + px(0.01), "{row:?}");
                            }
                        }
                        let mut frame = PaintedText {
                            version,
                            width: bounds.size.width,
                            line_widths: Vec::new(),
                            wrap_indices: Vec::new(),
                            middle: label_layout.text(),
                        };
                        let line_height = font_size * 1.5;
                        let mut origin = bounds.origin;
                        for line in lines {
                            let indices = line
                                .wrap_boundaries()
                                .iter()
                                .map(|boundary| {
                                    line.unwrapped_layout.runs[boundary.run_ix].glyphs
                                        [boundary.glyph_ix]
                                        .index
                                })
                                .collect::<Vec<_>>();
                            let graphemes = line
                                .text
                                .grapheme_indices(true)
                                .map(|(index, _)| index)
                                .collect::<Vec<_>>();
                            assert!(
                                indices
                                    .iter()
                                    .all(|index| *index > 0 && graphemes.contains(index))
                            );
                            assert!(indices.windows(2).all(|pair| pair[0] < pair[1]));
                            assert_eq!(line.len(), line.text.len());
                            for glyph in line.runs().iter().flat_map(|run| &run.glyphs) {
                                assert!(glyph.index < line.len());
                                assert!(line.text.is_char_boundary(glyph.index));
                            }
                            if let Some(index) = line.text.find('😀') {
                                let emoji = line
                                    .runs()
                                    .iter()
                                    .flat_map(|run| &run.glyphs)
                                    .find(|glyph| glyph.index == index)
                                    .expect("real emoji fallback glyph");
                                assert_eq!(emoji.id, GlyphId(878));
                            }
                            line.paint(origin, line_height, TextAlign::Left, None, window, cx)
                                .expect("Unicode text must paint through the headless window");
                            origin.y += line.size(line_height).height;
                            frame.line_widths.push(line.unwrapped_layout.width);
                            frame.wrap_indices.push(indices);
                        }
                        middle
                            .paint(origin, line_height, TextAlign::Left, None, window, cx)
                            .expect("redistributed middle runs must paint");
                        paints.borrow_mut().push(frame);
                    },
                )
                .w(self.width)
                .h(px(600.))
                .flex_shrink_0(),
            )
    }
}

fn draw_frame(
    app: &mut HeadlessAppContext,
    handle: WindowHandle<WrappingView>,
    paints: &Rc<RefCell<Vec<PaintedText>>>,
) -> Result<(u64, PaintedText)> {
    let revision = app.update_window(handle.into(), |_, window, cx| {
        window.draw(cx).clear(cx);
        let revision = window.rendered_frame_revision();
        assert!(revision > 0);
        assert_eq!(
            window.presentation_facts().frame_accepted_generation,
            Some(revision),
        );
        revision
    })?;
    Ok((
        revision,
        paints.borrow().last().expect("canvas paint").clone(),
    ))
}

#[test]
fn wrapping_and_middle_truncation_survive_public_headless_reflow_and_reconstruction() -> Result<()>
{
    let platform = Arc::new(CosmicTextSystem::new_without_system_fonts("Lilex"));
    let mut app = HeadlessAppContext::new(platform);
    app.update(|cx| {
        cx.set_quit_mode(QuitMode::Explicit);
        cx.set_text_rendering_mode(TextRenderingMode::Grayscale);
    });
    let text_system = app.text_system().clone();
    app.update(|cx| assert!(Arc::ptr_eq(cx.text_system(), &text_system)));
    text_system.add_fonts(vec![
        Cow::Borrowed(LILEX),
        Cow::Borrowed(IBM_PLEX),
        Cow::Borrowed(NOTO_EMOJI),
    ])?;
    let font = Font {
        features: FontFeatures::disable_ligatures(),
        ..font("Lilex")
    };
    let paints = Rc::new(RefCell::new(Vec::new()));
    let handle = app.open_window(size(px(720.), px(900.)), {
        let font = font.clone();
        let paints = paints.clone();
        move |_, cx| {
            cx.new(|_| WrappingView {
                font,
                paints,
                width: px(620.),
                font_size: px(20.),
                version: 0,
            })
        }
    })?;
    let root = handle.root(&mut app)?;
    let notifications = Rc::new(Cell::new(0));
    let subscription = app.update(|cx| {
        let notifications = notifications.clone();
        cx.observe(&root, move |_, _| {
            notifications.set(notifications.get() + 1)
        })
    });
    let (first_revision, first) = draw_frame(&mut app, handle, &paints)?;
    assert_eq!(first.version, 0);
    handle.update(&mut app, |view, _, cx| {
        view.font_size = px(24.);
        view.version = 1;
        cx.notify();
    })?;
    app.run_until_parked();
    assert_eq!(notifications.get(), 1);
    let (notified_revision, notified) = draw_frame(&mut app, handle, &paints)?;
    assert!(notified_revision > first_revision);
    assert_eq!(notified.version, 1);
    assert!(
        notified
            .line_widths
            .iter()
            .zip(&first.line_widths)
            .all(|(new, old)| new > old)
    );
    handle.update(&mut app, |view, _, cx| {
        view.width = px(90.);
        view.version = 2;
        cx.notify();
    })?;
    app.run_until_parked();
    assert_eq!(notifications.get(), 2);
    let (narrow_revision, narrow) = draw_frame(&mut app, handle, &paints)?;
    assert!(narrow_revision > notified_revision);
    assert_eq!(narrow.version, 2);
    assert_eq!(narrow.width, px(90.));
    assert!(narrow.rows() > notified.rows());
    assert_eq!(narrow.line_widths, notified.line_widths);
    assert!(narrow.middle.len() < notified.middle.len());
    assert!(narrow.middle.starts_with('p') && narrow.middle.ends_with('s'));
    let paint_count = paints.borrow().len();
    app.update_window(handle.into(), |_, window, _| window.refresh())?;
    let (refreshed_revision, refreshed) = draw_frame(&mut app, handle, &paints)?;
    assert!(refreshed_revision > narrow_revision);
    assert!(paints.borrow().len() > paint_count);
    assert_eq!(refreshed, narrow);
    drop(subscription);
    drop(root);
    app.update_window(handle.into(), |_, window, cx| window.remove_window(cx))?;
    app.run_until_parked();
    assert!(app.update(|cx| cx.windows().is_empty()));
    assert!(Arc::ptr_eq(app.text_system(), &text_system));
    let paint_count = paints.borrow().len();
    let rebuilt = app.open_window(size(px(360.), px(900.)), {
        let paints = paints.clone();
        move |_, cx| {
            cx.new(|_| WrappingView {
                font,
                paints,
                width: px(90.),
                font_size: px(24.),
                version: 2,
            })
        }
    })?;
    assert_ne!(rebuilt.window_id(), handle.window_id());
    let (_, reconstructed) = draw_frame(&mut app, rebuilt, &paints)?;
    assert!(paints.borrow().len() > paint_count);
    assert_eq!(reconstructed, narrow);
    app.update_window(rebuilt.into(), |_, window, cx| window.remove_window(cx))?;
    app.run_until_parked();
    assert!(app.update(|cx| cx.windows().is_empty()));
    let lifetime = Arc::downgrade(&text_system);
    drop(text_system);
    drop(app);
    assert!(lifetime.upgrade().is_none());
    Ok(())
}
