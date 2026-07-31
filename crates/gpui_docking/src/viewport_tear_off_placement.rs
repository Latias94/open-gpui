use crate::{DockViewportTearOffRequest, drag::DockDragTearOffGeometry};
use open_gpui::{
    Bounds, Pixels, PlatformNativePointerPhysicalFrame, Point, WindowBounds, point, px,
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

fn undock_limited_work_area_size(work_area: Bounds<Pixels>) -> open_gpui::Size<Pixels> {
    work_area
        .size
        .map(|dimension| (dimension * DOCK_TEAR_OFF_MAX_WORK_AREA_FRACTION).floor())
}
