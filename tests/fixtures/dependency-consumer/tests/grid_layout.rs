//! Independently locked public API consumer of Zed
//! 635026fabc38440fce5e1767d125026a17b12012, cdf33ac25e and 5ccbbbd88f.
//! This fixture has no test-support: these are style/signature and direct Noop
//! text checks, not evidence of window layout, kerning, paint, or presentation.

use open_gpui::{
    Bounds, Display, GridTemplate, GridTemplateMinSize, NoopTextSystem, Pixels, Styled as _,
    TextLayout, TextRun, TextSystem, TruncateFrom, Window, WindowTextSystem, WrappedLineLayout,
    div, font, px,
};
use std::{rc::Rc, sync::Arc};

#[test]
fn downstream_grid_builders_preserve_column_and_row_sizing_policies() {
    for (mut element, min_size) in [
        (
            div().grid().grid_cols(3).grid_rows(2),
            GridTemplateMinSize::Zero,
        ),
        (
            div()
                .grid()
                .grid_cols_min_content(3)
                .grid_rows_min_content(2),
            GridTemplateMinSize::MinContent,
        ),
        (
            div()
                .grid()
                .grid_cols_max_content(3)
                .grid_rows_max_content(2),
            GridTemplateMinSize::MaxContent,
        ),
    ] {
        let style = element.style();
        assert_eq!(style.display, Some(Display::Grid));
        assert_eq!(
            style.grid_cols,
            Some(GridTemplate {
                repeat: 3,
                min_size
            })
        );
        assert_eq!(
            style.grid_rows,
            Some(GridTemplate {
                repeat: 2,
                min_size
            })
        );
    }
    let mut replaced = div()
        .grid_cols(8)
        .grid_cols_max_content(3)
        .grid_rows(9)
        .grid_rows_min_content(2);
    assert_eq!(
        replaced.style().grid_cols,
        Some(GridTemplate {
            repeat: 3,
            min_size: GridTemplateMinSize::MaxContent
        }),
    );
    assert_eq!(
        replaced.style().grid_rows,
        Some(GridTemplate {
            repeat: 2,
            min_size: GridTemplateMinSize::MinContent
        }),
    );
}

#[test]
fn downstream_can_observe_public_text_layout_and_shape_without_a_test_platform() {
    let _: fn(&TextLayout) -> String = TextLayout::text;
    let _: fn(&TextLayout) -> Bounds<Pixels> = TextLayout::bounds;
    let _: fn(&TextLayout, usize) -> Option<Arc<WrappedLineLayout>> =
        TextLayout::line_layout_for_index;
    let _: for<'a> fn(&'a Window) -> &'a Rc<WindowTextSystem> = Window::text_system;

    let global = Arc::new(TextSystem::new(Arc::new(NoopTextSystem::new())));
    let owner = WindowTextSystem::new(global.clone());
    let source = "downstream_intrinsic_grid.rs";
    let descriptor = font("downstream");
    let runs = [TextRun {
        len: source.len(),
        font: descriptor.clone(),
        ..Default::default()
    }];
    let oracle = owner.shape_line(source.into(), px(16.), &runs, None);
    assert_eq!(oracle.text.as_ref(), source);
    assert!(oracle.width > px(0.));
    for direction in [TruncateFrom::End, TruncateFrom::Start, TruncateFrom::Middle] {
        let mut wrapper = global.line_wrapper(descriptor.clone(), px(16.));
        let (exact, exact_runs) =
            wrapper.truncate_line(source.into(), oracle.width, "…", &runs, direction);
        assert_eq!(exact.as_ref(), source);
        assert_eq!(exact_runs.as_ref(), &runs);
        let (narrow, narrow_runs) =
            wrapper.truncate_line(source.into(), oracle.width * 0.5, "…", &runs, direction);
        let (front, back) = narrow.split_once('…').expect("Noop overflow affix");
        assert!(source.starts_with(front) && source.ends_with(back));
        assert!(front.len() + back.len() < source.len());
        match direction {
            TruncateFrom::End => assert!(!front.is_empty() && back.is_empty()),
            TruncateFrom::Start => assert!(front.is_empty() && !back.is_empty()),
            TruncateFrom::Middle => assert!(!front.is_empty() && !back.is_empty()),
        }
        let shaped = owner.shape_line(narrow.clone(), px(16.), &narrow_runs, None);
        assert_eq!(shaped.text, narrow);
        assert_eq!(shaped.len(), narrow.len());
        assert!(shaped.width <= oracle.width * 0.5 + px(0.01));
    }
}
