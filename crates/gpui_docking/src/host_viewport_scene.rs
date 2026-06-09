use crate::DockHost;
use open_gpui::{Bounds, Pixels, Point, Window, point};

impl DockHost {
    pub(crate) fn update_viewport_host_scene_from_window(
        &mut self,
        host_bounds: Bounds<Pixels>,
        position: Point<Pixels>,
        window: &Window,
    ) -> bool {
        let Some(runtime) = self.viewport_runtime().cloned() else {
            return false;
        };
        let window_id = window.window_handle().window_id();
        if runtime.window_id_for_space(self.space()) != Some(window_id) {
            return false;
        }

        runtime.begin_viewport_host_scene(
            self.space().clone(),
            window_id,
            window.window_bounds(),
            host_bounds,
            host_local_point(host_bounds, position),
        )
    }
}

fn host_local_point(host_bounds: Bounds<Pixels>, position: Point<Pixels>) -> Point<Pixels> {
    point(
        position.x - host_bounds.origin.x,
        position.y - host_bounds.origin.y,
    )
}
