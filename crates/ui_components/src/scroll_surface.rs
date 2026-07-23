use open_gpui::{
    BringIntoViewOptions, RevealTargetHandle, ScrollHandle, ScrollViewportChangeSource,
    ScrollViewportProgrammaticSource, ScrollWheelEvent, TargetedEvent, Window, WindowId, point, px,
};
use open_gpui_ui_core::{UiPx, VirtualizerItemGeometry};

use crate::geometry::{gpui_px_from_ui, ui_px_from_gpui};

#[derive(Debug, Clone)]
struct RevealTargetSlot {
    window_id: WindowId,
    logical_identity: String,
    handle: RevealTargetHandle,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct ScrollSurfaceRuntime {
    reset_key: Option<String>,
    scroll_handle: ScrollHandle,
    reveal_targets: Vec<RevealTargetSlot>,
    pending_bring_into_view: Option<BringIntoViewOptions>,
}

impl ScrollSurfaceRuntime {
    pub(crate) fn new(reset_key: Option<String>) -> Self {
        Self {
            reset_key,
            scroll_handle: ScrollHandle::new(),
            reveal_targets: Vec::new(),
            pending_bring_into_view: None,
        }
    }

    pub(crate) fn reset_key(&self) -> Option<&str> {
        self.reset_key.as_deref()
    }

    pub(crate) fn set_reset_key(&mut self, reset_key: Option<String>) {
        self.reset_key = reset_key;
    }

    pub(crate) fn scroll_handle(&self) -> ScrollHandle {
        self.scroll_handle.clone()
    }

    pub(crate) fn reveal_target_for(
        &mut self,
        logical_identity: &str,
        window: &mut Window,
    ) -> RevealTargetHandle {
        let window_id = window.window_handle().window_id();
        if let Some(slot) = self
            .reveal_targets
            .iter_mut()
            .find(|slot| slot.window_id == window_id)
        {
            if slot.logical_identity != logical_identity {
                slot.logical_identity = logical_identity.to_owned();
                slot.handle = window.new_reveal_target();
            }
            return slot.handle;
        }
        let handle = window.new_reveal_target();
        self.reveal_targets.push(RevealTargetSlot {
            window_id,
            logical_identity: logical_identity.to_owned(),
            handle,
        });
        handle
    }

    pub(crate) fn queue_bring_into_view(&mut self, options: BringIntoViewOptions) {
        self.pending_bring_into_view = Some(options);
    }

    pub(crate) fn queue_vertical_bring_into_view(&mut self) {
        self.queue_bring_into_view(BringIntoViewOptions::vertical(
            open_gpui::BringIntoViewAlignment::Nearest,
        ));
    }

    /// Schedules the pending final reveal after the current render can bind its target.
    pub(crate) fn schedule_pending_bring_into_view(
        &mut self,
        target: RevealTargetHandle,
        window: &mut Window,
    ) {
        let Some(options) = self.pending_bring_into_view.take() else {
            return;
        };
        request_bring_into_view(target, options, window);
    }
}

pub(crate) fn request_bring_into_view(
    target: RevealTargetHandle,
    options: BringIntoViewOptions,
    window: &mut Window,
) {
    window.on_next_frame(move |window, cx| {
        let _ = window.bring_into_view(&target, options, cx);
    });
}

pub(crate) fn scroll_surface_handle(
    runtime: &ScrollSurfaceRuntime,
    external_scroll_handle: Option<&ScrollHandle>,
) -> ScrollHandle {
    external_scroll_handle
        .cloned()
        .unwrap_or_else(|| runtime.scroll_handle())
}

pub(crate) fn should_reset_scroll_surface(
    reset_on_key_change: bool,
    previous_reset_key: Option<&str>,
    current_reset_key: Option<&str>,
) -> bool {
    if !reset_on_key_change {
        return false;
    }

    match (previous_reset_key, current_reset_key) {
        (Some(previous), Some(current)) => previous != current,
        _ => false,
    }
}

pub(crate) fn vertical_viewport_extent(scroll_handle: &ScrollHandle) -> UiPx {
    ui_px_from_gpui(scroll_handle.bounds().size.height)
}

pub(crate) fn vertical_scroll_offset(scroll_handle: &ScrollHandle) -> UiPx {
    UiPx::new((-ui_px_from_gpui(scroll_handle.offset().y).as_f32()).max(0.0))
}

pub(crate) fn set_vertical_scroll_offset(scroll_handle: &ScrollHandle, scroll_offset: UiPx) {
    set_vertical_scroll_offset_with_source(
        scroll_handle,
        scroll_offset,
        ScrollViewportChangeSource::Programmatic(ScrollViewportProgrammaticSource::Offset),
    );
}

pub(crate) fn set_vertical_scroll_offset_with_source(
    scroll_handle: &ScrollHandle,
    scroll_offset: UiPx,
    source: ScrollViewportChangeSource,
) {
    let current = scroll_handle.offset();
    scroll_handle.set_offset_with_source(
        point(current.x, -gpui_px_from_ui(nonnegative_px(scroll_offset))),
        source,
    );
}

pub(crate) fn materialization_fixed_row_offset(
    target_index: usize,
    item_count: usize,
    row_height: UiPx,
    viewport_extent: UiPx,
    current_scroll_offset: UiPx,
) -> UiPx {
    let row_height = nonnegative_px(row_height);
    let viewport_extent = nonnegative_px(viewport_extent);
    if item_count == 0 || row_height.as_f32() <= 0.0 {
        return UiPx::ZERO;
    }

    let target_index = target_index.min(item_count - 1);
    let total_size = row_height * item_count as f32;
    let row_start = row_height * target_index as f32;
    materialization_row_geometry_offset(
        VirtualizerItemGeometry::new(row_start, row_height),
        total_size,
        viewport_extent,
        current_scroll_offset,
    )
}

pub(crate) fn materialization_row_geometry_offset(
    geometry: VirtualizerItemGeometry,
    total_size: UiPx,
    viewport_extent: UiPx,
    current_scroll_offset: UiPx,
) -> UiPx {
    let viewport_extent = nonnegative_px(viewport_extent);
    let max_scroll_offset = nonnegative_px(nonnegative_px(total_size) - viewport_extent);
    let current_scroll_offset = nonnegative_px(current_scroll_offset).min(max_scroll_offset);
    let row_start = geometry.start();
    let row_end = geometry.end();
    let viewport_start = current_scroll_offset;
    let viewport_end = viewport_start + viewport_extent;
    let target = if row_start < viewport_start {
        row_start
    } else if row_end > viewport_end {
        row_end - viewport_extent
    } else {
        viewport_start
    };

    nonnegative_px(target).min(max_scroll_offset)
}

pub(crate) fn materialize_row_geometry(
    scroll_handle: &ScrollHandle,
    geometry: VirtualizerItemGeometry,
    total_size: UiPx,
    fallback_viewport_extent: Option<UiPx>,
) -> bool {
    let viewport_extent =
        resolved_vertical_viewport_extent(scroll_handle, fallback_viewport_extent);
    if viewport_extent.as_f32() <= 0.0 {
        return false;
    }

    let current = vertical_scroll_offset(scroll_handle);
    let target =
        materialization_row_geometry_offset(geometry, total_size, viewport_extent, current);
    if target == current {
        return false;
    }

    set_vertical_scroll_offset_with_source(
        scroll_handle,
        target,
        ScrollViewportChangeSource::Programmatic(ScrollViewportProgrammaticSource::Offset),
    );
    true
}

pub(crate) fn materialize_fixed_row(
    scroll_handle: &ScrollHandle,
    target_index: usize,
    item_count: usize,
    row_height: UiPx,
    fallback_viewport_extent: Option<UiPx>,
) -> bool {
    let row_height = nonnegative_px(row_height);
    let viewport_extent =
        resolved_vertical_viewport_extent(scroll_handle, fallback_viewport_extent);
    if item_count == 0 || row_height.as_f32() <= 0.0 || viewport_extent.as_f32() <= 0.0 {
        return false;
    }

    let current = vertical_scroll_offset(scroll_handle);
    let target = materialization_fixed_row_offset(
        target_index,
        item_count,
        row_height,
        viewport_extent,
        current,
    );
    if target == current {
        return false;
    }

    set_vertical_scroll_offset_with_source(
        scroll_handle,
        target,
        ScrollViewportChangeSource::Programmatic(ScrollViewportProgrammaticSource::Offset),
    );
    true
}

pub(crate) fn handle_vertical_wheel_scroll(
    scroll_handle: &ScrollHandle,
    event: &TargetedEvent<ScrollWheelEvent>,
    window: &mut Window,
) -> bool {
    let Ok(delta) = event.target_local_delta() else {
        return false;
    };
    let delta = delta.pixel_delta(px(16.0));
    if delta.y.abs() <= delta.x.abs() {
        return false;
    }

    let current = scroll_handle.offset();
    let max_offset_y = scroll_handle.max_offset().y;
    let next_y = (current.y + delta.y).clamp(-max_offset_y, px(0.0));

    if next_y == current.y {
        return false;
    }

    scroll_handle
        .set_offset_with_source(point(current.x, next_y), ScrollViewportChangeSource::Wheel);
    window.refresh();
    true
}

fn resolved_vertical_viewport_extent(
    scroll_handle: &ScrollHandle,
    fallback_viewport_extent: Option<UiPx>,
) -> UiPx {
    let viewport_extent = vertical_viewport_extent(scroll_handle);
    if viewport_extent.as_f32() > 0.0 {
        viewport_extent
    } else {
        fallback_viewport_extent.unwrap_or(UiPx::ZERO)
    }
}

const fn nonnegative_px(value: UiPx) -> UiPx {
    if value.as_f32() < 0.0 {
        UiPx::ZERO
    } else {
        value
    }
}

#[cfg(test)]
mod tests {
    use open_gpui::{point, px};
    use open_gpui_ui_core::ui_px;

    use super::*;

    #[test]
    fn materialization_fixed_row_offset_keeps_target_in_mount_window() {
        assert_eq!(
            materialization_fixed_row_offset(10, 100, ui_px(32.0), ui_px(96.0), ui_px(0.0),),
            ui_px(256.0)
        );
        assert_eq!(
            materialization_fixed_row_offset(10, 100, ui_px(32.0), ui_px(96.0), ui_px(320.0),),
            ui_px(320.0)
        );
    }

    #[test]
    fn reset_only_fires_after_existing_key_changes() {
        assert!(should_reset_scroll_surface(
            true,
            Some("before"),
            Some("after")
        ));
        assert!(!should_reset_scroll_surface(true, None, Some("after")));
        assert!(!should_reset_scroll_surface(
            false,
            Some("before"),
            Some("after")
        ));
    }

    #[test]
    fn materialization_row_geometry_offset_uses_variable_item_bounds() {
        let geometry = VirtualizerItemGeometry::new(ui_px(240.0), ui_px(20.0));

        assert_eq!(
            materialization_row_geometry_offset(geometry, ui_px(320.0), ui_px(80.0), ui_px(0.0),),
            ui_px(180.0)
        );
    }

    #[test]
    fn materialization_fixed_row_offset_clamps_to_scrollable_range() {
        assert_eq!(
            materialization_fixed_row_offset(usize::MAX, 10, ui_px(20.0), ui_px(100.0), ui_px(0.0),),
            ui_px(100.0)
        );
        assert_eq!(
            materialization_fixed_row_offset(0, 10, ui_px(20.0), ui_px(100.0), ui_px(0.0),),
            ui_px(0.0)
        );
    }

    #[test]
    fn set_vertical_scroll_offset_preserves_horizontal_offset() {
        let scroll_handle = ScrollHandle::new();
        scroll_handle.set_offset(point(px(42.0), px(-12.0)));

        set_vertical_scroll_offset(&scroll_handle, ui_px(64.0));

        assert_eq!(scroll_handle.offset().x, px(42.0));
        assert_eq!(vertical_scroll_offset(&scroll_handle), ui_px(64.0));
    }

    #[test]
    fn materialize_fixed_row_noops_when_target_is_visible() {
        let scroll_handle = ScrollHandle::new();
        assert!(!materialize_fixed_row(
            &scroll_handle,
            0,
            10,
            ui_px(32.0),
            Some(ui_px(96.0)),
        ));
        assert_eq!(vertical_scroll_offset(&scroll_handle), ui_px(0.0));
    }
}
