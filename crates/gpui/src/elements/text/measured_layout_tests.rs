use super::*;
use crate::{AvailableSpace, TestAppContext, TextStyleRefinement, div, px, size};

const SOURCE: &str = "é alpha beta gamma delta\n\nlast café e\u{301} longer line\n";

fn measure(
    label: &mut StyledText,
    width: Pixels,
    window: &mut Window,
    cx: &mut App,
) -> Size<Pixels> {
    let (layout_id, ()) = label.request_layout(None, None, window, cx);
    window.compute_layout(
        layout_id,
        size(AvailableSpace::Definite(width), AvailableSpace::MaxContent),
        cx,
    );
    window.layout_bounds(layout_id).size
}

fn assert_measured_lines(
    layout: &TextLayout,
    source: &str,
) -> SmallVec<[Arc<WrappedLineLayout>; 1]> {
    let lines = layout.line_layouts();
    let state = layout.0.borrow();
    let state = state.as_ref().expect("measurement must publish the layout");
    assert!(state.bounds.is_none());
    assert_eq!(layout.text(), source);
    assert_eq!(layout.len(), source.len());
    assert_eq!(lines.len(), source.split('\n').count());
    let repeated = layout.line_layouts();
    let mut start = 0;
    for (((line, repeated), shaped), text) in lines
        .iter()
        .zip(&repeated)
        .zip(state.lines.iter())
        .zip(source.split('\n'))
    {
        assert!(Arc::ptr_eq(line, repeated));
        assert!(Arc::ptr_eq(line, &shaped.layout));
        assert_eq!(shaped.text.as_ref(), text);
        assert_eq!(line.len(), text.len());
        for index in start..=start + text.len() {
            assert!(Arc::ptr_eq(
                line,
                &layout.line_layout_for_index(index).unwrap(),
            ));
        }
        for boundary in line.wrap_boundaries() {
            let glyph = &line.runs()[boundary.run_ix].glyphs[boundary.glyph_ix];
            assert!(text.is_char_boundary(glyph.index));
            assert!(Arc::ptr_eq(
                line,
                &layout.line_layout_for_index(start + glyph.index).unwrap(),
            ));
        }
        start += text.len() + 1;
    }
    assert_eq!(start - 1, source.len());
    assert!(layout.line_layout_for_index(source.len() + 1).is_none());
    assert!(layout.line_layout_for_index(usize::MAX).is_none());
    lines
}

#[test]
#[should_panic(expected = "measurement has not been performed")]
fn all_line_layouts_require_measurement() {
    TextLayout::default().line_layouts();
}

#[test]
#[should_panic(expected = "measurement has not been performed")]
fn indexed_line_layout_requires_measurement() {
    TextLayout::default().line_layout_for_index(0);
}

#[crate::test]
fn measured_line_queries_preserve_source_order_utf8_newlines_and_wrapped_identity(
    cx: &mut TestAppContext,
) {
    let visual = cx.add_empty_window();
    visual.draw(Point::default(), size(px(800.), px(600.)), |window, cx| {
        window.with_text_style(
            Some(TextStyleRefinement {
                font_size: Some(px(20.).into()),
                line_height: Some(px(24.).into()),
                white_space: Some(WhiteSpace::Normal),
                ..Default::default()
            }),
            |window| {
                for source in ["", "\n", SOURCE] {
                    let mut label = StyledText::new(source);
                    let measured = measure(&mut label, px(80.), window, cx);
                    let lines = assert_measured_lines(label.layout(), source);
                    let rows = lines
                        .iter()
                        .map(|line| line.wrap_boundaries().len() + 1)
                        .sum::<usize>();
                    assert_eq!(measured.height, px(24.) * rows);
                    assert_eq!(label.layout().wrapped_text().split('\n').count(), rows);
                    if source == SOURCE {
                        assert!(rows > lines.len());
                        assert_eq!(lines[1].len(), 0);
                        assert_eq!(lines.last().unwrap().len(), 0);
                    }
                }
            },
        );
        div()
    });
}

#[crate::test]
fn owned_line_queries_preserve_previous_measurement_after_reflow(cx: &mut TestAppContext) {
    let visual = cx.add_empty_window();
    visual.draw(Point::default(), size(px(800.), px(600.)), |window, cx| {
        window.with_text_style(
            Some(TextStyleRefinement {
                font_size: Some(px(20.).into()),
                line_height: Some(px(24.).into()),
                white_space: Some(WhiteSpace::Normal),
                ..Default::default()
            }),
            |window| {
                let mut label = StyledText::new(SOURCE);
                measure(&mut label, px(160.), window, cx);
                let layout = label.layout().clone();
                let previous = assert_measured_lines(&layout, SOURCE);
                let previous_boundaries = previous[0].wrap_boundaries.clone();
                let previous_owner = Rc::downgrade(&layout.0.borrow().as_ref().unwrap().lines);
                measure(&mut label, px(60.), window, cx);
                let current = assert_measured_lines(&layout, SOURCE);
                assert!(previous_owner.upgrade().is_none());
                assert!(!Arc::ptr_eq(&previous[0], &current[0]));
                assert!(current[0].wrap_boundaries.len() > previous_boundaries.len());
                assert!(Arc::ptr_eq(
                    &previous[0].unwrapped_layout,
                    &current[0].unwrapped_layout,
                ));
                assert_eq!(previous[0].wrap_width, Some(px(160.)));
                assert_eq!(previous[0].wrap_boundaries, previous_boundaries);
                drop(label);
                drop(layout);
                assert_eq!(previous[0].len(), SOURCE.split('\n').next().unwrap().len());
                assert_eq!(previous[0].wrap_boundaries, previous_boundaries);
                assert_eq!(current[0].wrap_width, Some(px(60.)));
            },
        );
        div()
    });
}
