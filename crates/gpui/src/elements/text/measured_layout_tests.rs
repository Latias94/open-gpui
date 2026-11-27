use super::*;
use crate::{
    AppContext as _, AvailableSpace, Context, HeadlessAppContext, Hsla, ParentElement as _, Render,
    Styled as _, TestAppContext, TextStyleRefinement, canvas, div, font, px, size,
    text_system::font_installation_tests::DynamicPlatform,
};
use std::panic::{AssertUnwindSafe, catch_unwind};

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

#[derive(Clone, Copy)]
struct RetainedTextInput {
    text: &'static str,
    color: Hsla,
    width: Pixels,
    offset: Pixels,
}

type TextObservation = Rc<RefCell<Option<(LayoutId, TextLayout)>>>;

struct RetainedTextView {
    input: Rc<Cell<RetainedTextInput>>,
    observed: TextObservation,
}

impl Render for RetainedTextView {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let input = self.input.get();
        let label = StyledText::new(input.text).with_runs(vec![TextRun {
            len: input.text.len(),
            font: font("primary"),
            color: input.color,
            ..Default::default()
        }]);
        div()
            .flex()
            .flex_col()
            .w(input.width)
            .ml(input.offset)
            .child(RetainedTextProbe {
                label,
                observed: self.observed.clone(),
            })
    }
}

struct RetainedTextProbe {
    label: StyledText,
    observed: TextObservation,
}

impl Element for RetainedTextProbe {
    type RequestLayoutState = LayoutId;
    type PrepaintState = ();

    fn id(&self) -> Option<ElementId> {
        Some("retained-text".into())
    }

    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        id: Option<&GlobalElementId>,
        inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, LayoutId) {
        let (layout, ()) = self.label.request_layout(id, inspector_id, window, cx);
        (layout, layout)
    }

    fn prepaint(
        &mut self,
        id: Option<&GlobalElementId>,
        inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        layout_id: &mut LayoutId,
        window: &mut Window,
        cx: &mut App,
    ) {
        let layout = self.label.layout().clone();
        assert!(layout.0.borrow().as_ref().unwrap().bounds.is_none());
        self.label
            .prepaint(id, inspector_id, bounds, &mut (), window, cx);
        self.observed.borrow_mut().replace((*layout_id, layout));
    }

    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        _: &mut LayoutId,
        _: &mut (),
        _: &mut Window,
        _: &mut App,
    ) {
    }
}

#[test]
fn retained_text_reuses_measurement_with_fresh_bounds_and_invalidates_changed_inputs() {
    let mut app = HeadlessAppContext::new(Arc::new(DynamicPlatform::default()));
    app.update(|cx| cx.set_quit_mode(crate::QuitMode::Explicit));
    let initial = RetainedTextInput {
        text: "alpha beta gamma delta epsilon",
        color: crate::black(),
        width: px(180.),
        offset: px(0.),
    };
    let input = Rc::new(Cell::new(initial));
    let observed = Rc::new(RefCell::new(None));
    let handle = app
        .open_window(size(px(320.), px(240.)), |window, cx| {
            window.set_view_retention_for_test(true);
            cx.new(|_| RetainedTextView {
                input: input.clone(),
                observed: observed.clone(),
            })
        })
        .unwrap();
    let draw = |app: &mut HeadlessAppContext, next| {
        input.set(next);
        app.update_window(handle.into(), |_, window, cx| {
            window.refresh();
            window.draw(cx).clear(cx);
            assert!(window.layout_keys.current().is_none());
        })
        .unwrap();
        observed.borrow_mut().take().unwrap()
    };
    let lines = |text: &TextLayout| text.0.borrow().as_ref().unwrap().lines.clone();

    let (id, first) = draw(&mut app, initial);
    let moved_input = RetainedTextInput {
        offset: px(18.),
        ..initial
    };
    let (moved_id, moved) = draw(&mut app, moved_input);
    assert_eq!(moved_id, id);
    assert!(!Rc::ptr_eq(&first.0, &moved.0));
    assert!(Rc::ptr_eq(&lines(&first), &lines(&moved)));
    assert_eq!(moved.bounds().origin.x - first.bounds().origin.x, px(18.));

    let narrow_input = RetainedTextInput {
        width: px(68.),
        ..moved_input
    };
    let (narrow_id, narrow) = draw(&mut app, narrow_input);
    assert_eq!(narrow_id, id);
    assert!(!Rc::ptr_eq(&lines(&moved), &lines(&narrow)));
    assert!(
        narrow.line_layouts()[0].wrap_boundaries().len()
            > moved.line_layouts()[0].wrap_boundaries().len()
    );

    let colored_input = RetainedTextInput {
        color: crate::red(),
        ..narrow_input
    };
    let (_, colored) = draw(&mut app, colored_input);
    assert!(!Rc::ptr_eq(&lines(&narrow), &lines(&colored)));
    assert_eq!(
        lines(&colored)[0].decoration_runs[0].color,
        colored_input.color
    );

    let changed_input = RetainedTextInput {
        text: "alpha beta gamma",
        ..colored_input
    };
    let (_, changed) = draw(&mut app, changed_input);
    assert_eq!(changed.text(), changed_input.text);
    app.update(|cx| {
        cx.text_system()
            .add_fonts(vec![Cow::Borrowed(b"font")])
            .unwrap()
    });
    let (_, installed) = draw(&mut app, changed_input);
    assert!(!Rc::ptr_eq(&lines(&changed), &lines(&installed)));
    assert_eq!(
        installed.line_layouts()[0].unwrapped_layout.width,
        changed.line_layouts()[0].unwrapped_layout.width * 2.
    );

    app.update_window(handle.into(), |_, window, _| {
        window.set_view_retention_for_test(false)
    })
    .unwrap();
    let (_, fresh) = draw(&mut app, changed_input);
    assert_eq!(installed.bounds(), fresh.bounds());
    assert_eq!(installed.wrapped_text(), fresh.wrapped_text());
}

struct LateMeasurementView {
    fail: Rc<Cell<bool>>,
    layout: Rc<Cell<Option<LayoutId>>>,
}

impl Render for LateMeasurementView {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let fail = self.fail.clone();
        let observed = self.layout.clone();
        canvas(
            move |_, window, cx| {
                assert!(window.layout_keys.current().is_some());
                let layout =
                    window.request_measured_layout(Default::default(), move |_, _, _, _| {
                        assert!(!fail.replace(false), "measured layout unwind");
                        size(px(30.), px(15.))
                    });
                window.compute_layout(layout, AvailableSpace::min_size(), cx);
                observed.set(Some(layout));
            },
            |_, _, _, _| {},
        )
    }
}

#[test]
fn measured_prepaint_unwind_resets_the_layout_keys_and_can_retry() {
    let mut app = HeadlessAppContext::new(Arc::new(DynamicPlatform::default()));
    app.update(|cx| cx.set_quit_mode(crate::QuitMode::Explicit));
    let fail = Rc::new(Cell::new(false));
    let layout = Rc::new(Cell::new(None));
    let handle = app
        .open_window(size(px(160.), px(120.)), |window, cx| {
            window.set_view_retention_for_test(true);
            cx.new(|_| LateMeasurementView {
                fail: fail.clone(),
                layout: layout.clone(),
            })
        })
        .unwrap();
    app.update_window(handle.into(), |_, window, cx| {
        window.refresh();
        window.draw(cx).clear(cx);
        let accepted = window.rendered_frame_revision();
        let original = layout.get().unwrap();

        fail.set(true);
        let panic = catch_unwind(AssertUnwindSafe(|| window.draw(cx).clear(cx))).unwrap_err();
        assert_eq!(
            panic.downcast_ref::<&str>(),
            Some(&"measured layout unwind")
        );
        assert_eq!(window.rendered_frame_revision(), accepted);
        assert!(window.layout_keys.current().is_none());

        window.draw(cx).clear(cx);
        assert_eq!(window.rendered_frame_revision(), accepted + 1);
        let rebuilt = layout.get().unwrap();
        assert_ne!(rebuilt, original);
        window.draw(cx).clear(cx);
        assert_eq!(layout.get(), Some(rebuilt));
        assert!(window.layout_keys.current().is_none());
    })
    .unwrap();
}
