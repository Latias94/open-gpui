use open_gpui::{
    IndentAdjustment, LineFragment, NoopTextSystem, Styled as _, TextRun, TextSystem, TruncateFrom,
    div, font, px,
};
use std::sync::Arc;

#[test]
fn downstream_wraps_unicode_with_an_explicit_indentation_policy() {
    let text_system = Arc::new(TextSystem::new(Arc::new(NoopTextSystem)));
    let descriptor = font("consumer");
    let space = text_system.layout_width(text_system.resolve_font(&descriptor), px(16.), ' ');
    let mut wrapper = text_system.line_wrapper(descriptor, px(16.));
    let fragments = [LineFragment::text("  aa/"), LineFragment::text("bbbb")];
    for (adjustment, indent) in [
        (IndentAdjustment::NoIndent, 0),
        (IndentAdjustment::default(), 2),
        (IndentAdjustment::ExtraColumns(1), 3),
    ] {
        let boundaries = wrapper
            .wrap_line(&fragments, space * 7., adjustment)
            .collect::<Vec<_>>();
        assert_eq!(boundaries.len(), 1);
        assert_eq!(boundaries[0].ix, 5);
        assert_eq!(boundaries[0].next_indent, indent);
    }
}

#[test]
fn downstream_middle_truncation_retains_both_filename_ends() {
    let text_system = Arc::new(TextSystem::new(Arc::new(NoopTextSystem)));
    let descriptor = font("consumer");
    let space = text_system.layout_width(text_system.resolve_font(&descriptor), px(16.), ' ');
    let mut wrapper = text_system.line_wrapper(descriptor.clone(), px(16.));
    let text = "prefix_abcdefghijkl_suffix.rs";
    let runs = [TextRun {
        len: text.len(),
        font: descriptor,
        ..Default::default()
    }];
    let (truncated, adjusted) = wrapper.truncate_line(
        text.into(),
        space * 10. + px(0.01),
        "…",
        &runs,
        TruncateFrom::Middle,
    );
    assert_eq!(truncated.as_ref(), "prefix….rs");
    assert_eq!(
        adjusted.iter().map(|run| run.len).sum::<usize>(),
        truncated.len()
    );
    let _label = div().overflow_hidden().text_ellipsis_middle();
}

#[test]
fn downstream_preserves_fragment_graphemes_and_the_wrapped_start_row_budget() {
    let text_system = Arc::new(TextSystem::new(Arc::new(NoopTextSystem)));
    let descriptor = font("consumer");
    let space = text_system.layout_width(text_system.resolve_font(&descriptor), px(16.), ' ');
    let mut wrapper = text_system.line_wrapper(descriptor.clone(), px(16.));
    let fragments = [LineFragment::text("x e"), LineFragment::text("\u{301}yz")];
    let boundaries = wrapper
        .wrap_line(&fragments, space * 1.5, IndentAdjustment::NoIndent)
        .collect::<Vec<_>>();
    assert!(!boundaries.is_empty());
    assert!(
        boundaries
            .iter()
            .all(|boundary| [1, 2, 5, 6].contains(&boundary.ix))
    );
    let source = "aaaa-bbbb-cccc";
    let runs = [TextRun {
        len: source.len(),
        font: descriptor,
        ..Default::default()
    }];
    let (result, adjusted) = wrapper.truncate_wrapped_line(
        source.into(),
        space * 7. + px(0.01),
        2,
        "…",
        &runs,
        TruncateFrom::Start,
    );
    assert!(result.starts_with('…') && result.ends_with("cccc"));
    assert_eq!(
        adjusted.iter().map(|run| run.len).sum::<usize>(),
        result.len()
    );
    assert!(
        wrapper
            .wrap_line(
                &[LineFragment::text(&result)],
                space * 7. + px(0.01),
                IndentAdjustment::SameIndent
            )
            .count()
            < 2
    );
}
