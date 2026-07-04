use open_gpui::{ScrollHandle, ScrollWheelEvent, Window};

use crate::scroll_surface::handle_vertical_wheel_scroll;

pub(in crate::table) fn handle_table_vertical_scroll_wheel(
    scroll_handle: &ScrollHandle,
    event: &ScrollWheelEvent,
    window: &mut Window,
) {
    handle_vertical_wheel_scroll(scroll_handle, event, window);
}
