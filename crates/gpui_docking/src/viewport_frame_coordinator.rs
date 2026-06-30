use crate::{
    DockNodeId, DockSpaceId, DockViewportWindowFacts,
    drop_runtime::DockHostDropSceneFact,
    geometry::DockDropGuideStyle,
    viewport_drop_scene::{
        DockViewportHostSceneFrame, DockViewportHostSceneRegistration,
        DockViewportHostSceneRegistry, DockViewportHostSceneSnapshot,
    },
};
use open_gpui::{Bounds, Pixels, Point, WindowId};
#[derive(Debug, Default)]
pub(crate) struct DockViewportFrameCoordinator {
    host_scenes: DockViewportHostSceneRegistry,
}

impl DockViewportFrameCoordinator {
    pub(crate) fn host_scenes(&self) -> &DockViewportHostSceneRegistry {
        &self.host_scenes
    }

    pub(crate) fn register_host_scene(
        &mut self,
        space: DockSpaceId,
        window_id: WindowId,
        window_facts: DockViewportWindowFacts,
        host_bounds: Bounds<Pixels>,
        host_position: Point<Pixels>,
        drop_guide_style: DockDropGuideStyle,
    ) -> DockViewportHostSceneRegistration {
        self.host_scenes
            .register(DockViewportHostSceneSnapshot::new(
                space,
                window_id,
                window_facts.current_bounds,
                host_bounds,
                host_position,
                drop_guide_style,
            ))
    }

    #[cfg(test)]
    pub(crate) fn push_fact(
        &mut self,
        space: &DockSpaceId,
        window_id: WindowId,
        fact: DockHostDropSceneFact,
    ) -> bool {
        self.host_scenes.push_fact(space, window_id, fact)
    }

    pub(crate) fn push_frame_fact(
        &mut self,
        frame: &DockViewportHostSceneFrame,
        fact: DockHostDropSceneFact,
    ) -> Option<DockViewportHostSceneFrame> {
        self.host_scenes.push_frame_fact(frame, fact)
    }

    pub(crate) fn leaf_bounds_for_tabs(
        &self,
        space: &DockSpaceId,
        window_id: Option<WindowId>,
        tabs: DockNodeId,
    ) -> Option<Bounds<Pixels>> {
        self.host_scenes
            .leaf_bounds_for_tabs(space, window_id, tabs)
    }

    pub(crate) fn tab_bar_bounds_for_tabs(
        &self,
        space: &DockSpaceId,
        window_id: Option<WindowId>,
        tabs: DockNodeId,
    ) -> Option<Bounds<Pixels>> {
        self.host_scenes
            .tab_bar_bounds_for_tabs(space, window_id, tabs)
    }

    pub(crate) fn tab_label_bounds_for_tabs(
        &self,
        space: &DockSpaceId,
        window_id: Option<WindowId>,
        tabs: DockNodeId,
        target_index: usize,
    ) -> Option<Bounds<Pixels>> {
        self.host_scenes
            .tab_label_bounds_for_tabs(space, window_id, tabs, target_index)
    }

    #[cfg(test)]
    pub(crate) fn screen_position(&self, space: &DockSpaceId) -> Option<Point<Pixels>> {
        self.host_scenes.screen_position(space)
    }

    pub(crate) fn unregister_space(&mut self, space: &DockSpaceId) {
        self.host_scenes.unregister_space(space);
    }

    pub(crate) fn unregister_window_scene(&mut self, window_id: WindowId) {
        self.host_scenes.unregister_window(window_id);
    }
}

#[cfg(test)]
mod tests {}
