use open_gpui::{ScrollHandle, ScrollWheelEvent, Window, point, px};

pub(in crate::table) fn handle_table_vertical_scroll_wheel(
    scroll_handle: &ScrollHandle,
    event: &ScrollWheelEvent,
    window: &mut Window,
) {
    let delta = event.delta.pixel_delta(px(16.0));
    if delta.y.abs() <= delta.x.abs() {
        return;
    }

    let current = scroll_handle.offset();
    let max_offset_y = scroll_handle.max_offset().y;
    let next_y = (current.y + delta.y).clamp(-max_offset_y, px(0.0));

    if next_y != current.y {
        scroll_handle.set_offset(point(current.x, next_y));
        window.refresh();
    }
}
