use crate::{DockHost, DockItemId, DockSpaceId, DockViewportAdapter, DockViewportRuntimeLineage};
use open_gpui::{AnyWindowHandle, Bounds, Pixels, WindowHandle, WindowId, point, px, size};

pub(crate) fn space(id: &str) -> DockSpaceId {
    DockSpaceId::from(id)
}

pub(crate) fn item(id: &str) -> DockItemId {
    DockItemId::from(id)
}

pub(crate) fn handle(id: u64) -> AnyWindowHandle {
    WindowHandle::<DockHost>::new(WindowId::from(id)).into()
}

pub(crate) fn bounds(x: f32, y: f32, width: f32, height: f32) -> Bounds<Pixels> {
    Bounds::new(point(px(x), px(y)), size(px(width), px(height)))
}

pub(crate) fn register_viewport(
    adapter: &mut DockViewportAdapter,
    space: impl Into<DockSpaceId>,
    window: impl Into<AnyWindowHandle>,
) {
    let _ = adapter
        .register_viewport_with_outcome(space, window, DockViewportRuntimeLineage::Unmanaged)
        .expect("unmanaged test viewport registration cannot conflict by lineage");
}
