//! Independently locked value and signature coverage without test-support or new dependencies.
//! The measured-query closure is typechecked but never invoked. Actual element lifecycle
//! execution belongs to the WGPU measured_text HeadlessAppContext integration test.

use open_gpui::{Element, IntoElement as _, StyledText, TextLayout, WrappedLineLayout};
use std::sync::Arc;

#[test]
fn downstream_measured_text_values_and_query_signatures_need_no_test_support() {
    let _: fn(&TextLayout, usize) -> Option<Arc<WrappedLineLayout>> =
        TextLayout::line_layout_for_index;
    let _: fn(&StyledText) -> &TextLayout = StyledText::layout;
    fn assert_text_element<T: Element<RequestLayoutState = (), PrepaintState = ()>>() {}
    assert_text_element::<StyledText>();

    let source = "downstream café e\u{301}\n\nlast source line\n";
    let label = StyledText::new(source).into_element();
    let _: TextLayout = label.layout().clone();
    let _measured_queries_type_only = |layout: &TextLayout| {
        let lines = layout.line_layouts();
        let _: &[Arc<WrappedLineLayout>] = lines.as_slice();
        let mut start = 0;
        for (line, text) in lines.iter().zip(source.split('\n')) {
            assert_eq!(line.len(), text.len());
            assert!(Arc::ptr_eq(
                line,
                &layout.line_layout_for_index(start).unwrap(),
            ));
            start += text.len() + 1;
        }
        let owned: Vec<Arc<WrappedLineLayout>> = lines.into_vec();
        owned
    };
}
