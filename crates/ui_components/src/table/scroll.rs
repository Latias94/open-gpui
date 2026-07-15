use open_gpui::{
    AnyElement, Axis, InteractiveElement, IntoElement, ParentElement, Pixels, ScrollHandle, Styled,
    div, px,
};

pub(super) fn render_table_scroll_viewport(
    debug_selector: String,
    axis: Axis,
    scrollbar_width: Pixels,
    scroll_handle: &ScrollHandle,
    content: impl IntoElement,
) -> AnyElement {
    let viewport = div()
        .debug_selector(move || debug_selector.clone())
        .flex_1()
        .size_full()
        .min_w(px(0.0))
        .min_h(px(0.0))
        .overflow_hidden()
        .scrollbar_width(scrollbar_width);
    let viewport = match axis {
        Axis::Horizontal => viewport.overflow_x_scroll(),
        Axis::Vertical => viewport.overflow_y_scroll(),
    };

    viewport
        .track_scroll(scroll_handle)
        .child(content)
        .into_any_element()
}
