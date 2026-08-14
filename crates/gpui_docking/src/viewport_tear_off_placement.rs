use crate::{DockViewportTearOffRequest, drag::DockDragTearOffGeometry};
use open_gpui::{
    Bounds, DevicePixels, Pixels, PlatformNativePointerPhysicalFrame, Point, WindowBounds, point,
    px,
};

const DOCK_TEAR_OFF_MAX_WORK_AREA_FRACTION: f32 = 0.90;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DockViewportTearOffPlacementSource {
    Suggested,
    DragGeometry,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct DockViewportTearOffPlacement {
    window_bounds: WindowBounds,
    source: DockViewportTearOffPlacementSource,
}

#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct DockViewportTearOffPlacementPolicy {}

impl DockViewportTearOffPlacement {
    fn new(window_bounds: WindowBounds, source: DockViewportTearOffPlacementSource) -> Self {
        Self {
            window_bounds,
            source,
        }
    }

    pub(crate) fn window_bounds(&self) -> WindowBounds {
        self.window_bounds
    }

    #[cfg(test)]
    pub(crate) fn source(&self) -> DockViewportTearOffPlacementSource {
        self.source
    }
}

impl DockViewportTearOffPlacementPolicy {
    pub(crate) fn resolve(
        &self,
        request: &DockViewportTearOffRequest,
    ) -> Option<DockViewportTearOffPlacement> {
        if let Some(window_bounds) = request.suggested_window_bounds() {
            return Some(DockViewportTearOffPlacement::new(
                window_bounds,
                DockViewportTearOffPlacementSource::Suggested,
            ));
        }

        if let Some(geometry) = request.tear_off_geometry()
            && let Some(release_position) = request.release_position()
        {
            return Some(DockViewportTearOffPlacement::new(
                WindowBounds::Windowed(bounds_from_drag_geometry(release_position, geometry)),
                DockViewportTearOffPlacementSource::DragGeometry,
            ));
        }
        None
    }
}

pub(crate) fn suggested_tear_off_window_bounds(
    source_window_bounds: WindowBounds,
    host_position: Point<Pixels>,
    geometry: DockDragTearOffGeometry,
) -> WindowBounds {
    let source_window_origin = source_window_bounds.get_bounds().origin;
    WindowBounds::Windowed(tear_off_bounds_from_cursor_anchor(
        source_window_origin + host_position,
        geometry,
    ))
}

pub(crate) fn suggested_tear_off_window_bounds_from_native_frame(
    physical_frame: PlatformNativePointerPhysicalFrame,
    geometry: DockDragTearOffGeometry,
) -> Option<WindowBounds> {
    let source_geometry = physical_frame.source_geometry();
    let scale_factor = source_geometry.scale_factor();
    let source_client_origin = source_geometry
        .client_bounds()
        .to_pixels(scale_factor)
        .origin;
    let source_local_position =
        source_geometry.global_to_local(physical_frame.global_position())?;
    Some(WindowBounds::Windowed(tear_off_bounds_from_cursor_anchor(
        source_client_origin + source_local_position,
        geometry,
    )))
}

pub(crate) fn suggested_tear_off_physical_client_bounds_from_native_frame(
    physical_frame: PlatformNativePointerPhysicalFrame,
    geometry: DockDragTearOffGeometry,
) -> Option<Bounds<DevicePixels>> {
    let target_display = physical_frame.target_display()?;
    let scale_factor = target_display.scale_factor();
    let logical_size = tear_off_window_size_for_target_display(geometry, target_display);
    let physical_size = logical_size.to_device_pixels(scale_factor);
    if physical_size.width.0 <= 0 || physical_size.height.0 <= 0 {
        return None;
    }

    let logical_offset = geometry.cursor_offset().clamp(
        &point(px(0.0), px(0.0)),
        &point(logical_size.width, logical_size.height),
    );
    let offset_x = ((logical_offset.x.as_f32() * scale_factor).round() as i32)
        .clamp(0, physical_size.width.0 - 1);
    let offset_y = ((logical_offset.y.as_f32() * scale_factor).round() as i32)
        .clamp(0, physical_size.height.0 - 1);
    let global = physical_frame.global_position();
    let origin = point(
        DevicePixels(global.x.0.checked_sub(offset_x)?),
        DevicePixels(global.y.0.checked_sub(offset_y)?),
    );
    origin.x.0.checked_add(physical_size.width.0)?;
    origin.y.0.checked_add(physical_size.height.0)?;
    Some(Bounds::new(origin, physical_size))
}

fn bounds_from_drag_geometry(
    release_position: Point<Pixels>,
    geometry: DockDragTearOffGeometry,
) -> Bounds<Pixels> {
    tear_off_bounds_from_cursor_anchor(release_position, geometry)
}

fn clamp_bounds_to_work_area(bounds: Bounds<Pixels>, work_area: Bounds<Pixels>) -> Bounds<Pixels> {
    let max_origin = point(
        work_area.right() - bounds.size.width,
        work_area.bottom() - bounds.size.height,
    );
    let origin = bounds.origin.clamp(&work_area.origin, &max_origin);
    Bounds::new(origin, bounds.size)
}

fn tear_off_bounds_from_cursor_anchor(
    cursor_anchor: Point<Pixels>,
    geometry: DockDragTearOffGeometry,
) -> Bounds<Pixels> {
    let size = tear_off_window_size(geometry);
    let cursor_offset = geometry
        .cursor_offset()
        .clamp(&point(px(0.0), px(0.0)), &point(size.width, size.height));
    let bounds = Bounds::new(cursor_anchor - cursor_offset, size);
    geometry
        .display_work_area()
        .map(|work_area| clamp_bounds_to_work_area(bounds, work_area))
        .unwrap_or(bounds)
}

fn tear_off_window_size(geometry: DockDragTearOffGeometry) -> open_gpui::Size<Pixels> {
    let size = geometry
        .preferred_size()
        .unwrap_or_else(|| geometry.source_bounds().size);
    geometry
        .display_work_area()
        .map(|work_area| size.min(&undock_limited_work_area_size(work_area)))
        .unwrap_or(size)
}

fn tear_off_window_size_for_target_display(
    geometry: DockDragTearOffGeometry,
    target_display: open_gpui::PlatformPhysicalDisplayObservation,
) -> open_gpui::Size<Pixels> {
    let size = geometry
        .preferred_size()
        .unwrap_or_else(|| geometry.source_bounds().size);
    let target_work_area_size = target_display
        .visible_bounds()
        .size
        .to_pixels(target_display.scale_factor());
    size.min(&undock_limited_size(target_work_area_size))
}

fn undock_limited_work_area_size(work_area: Bounds<Pixels>) -> open_gpui::Size<Pixels> {
    undock_limited_size(work_area.size)
}

fn undock_limited_size(size: open_gpui::Size<Pixels>) -> open_gpui::Size<Pixels> {
    size.map(|dimension| (dimension * DOCK_TEAR_OFF_MAX_WORK_AREA_FRACTION).floor())
}
