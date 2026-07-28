#[cfg(test)]
use crate::drop_runtime::DockHostDropScene;
use crate::{
    DockNodeId, DockSpaceId,
    drop_runtime::DockHostDropSceneFact,
    viewport_drop_scene::{
        DockViewportHostSceneFrame, DockViewportHostSceneRegistration,
        DockViewportHostSceneRegistry, DockViewportHostSceneSnapshot,
    },
};
#[cfg(test)]
use open_gpui::Point;
use open_gpui::{Bounds, Pixels, WindowId};
#[derive(Debug, Default)]
pub(crate) struct DockViewportFrameCoordinator {
    host_scenes: DockViewportHostSceneRegistry,
}

impl DockViewportFrameCoordinator {
    pub(crate) fn host_scenes(&self) -> &DockViewportHostSceneRegistry {
        &self.host_scenes
    }

    pub(crate) fn register_host_scene_snapshot(
        &mut self,
        snapshot: DockViewportHostSceneSnapshot,
    ) -> DockViewportHostSceneRegistration {
        self.host_scenes.register(snapshot)
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

    pub(crate) fn leaf_displayed_bounds_for_tabs(
        &self,
        space: &DockSpaceId,
        window_id: Option<WindowId>,
        tabs: DockNodeId,
    ) -> Option<Bounds<Pixels>> {
        self.host_scenes
            .leaf_displayed_bounds_for_tabs(space, window_id, tabs)
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
    pub(crate) fn scene_for_window(
        &self,
        space: &DockSpaceId,
        window_id: WindowId,
    ) -> Option<DockHostDropScene> {
        self.host_scenes.scene_for_window(space, window_id)
    }

    #[cfg(test)]
    pub(crate) fn screen_position(&self, space: &DockSpaceId) -> Option<Point<Pixels>> {
        self.host_scenes.screen_position(space)
    }

    pub(crate) fn unregister_space(&mut self, space: &DockSpaceId) {
        self.host_scenes.unregister_space(space);
    }

    pub(crate) fn discard_frame_for_viewport(
        &mut self,
        space: &DockSpaceId,
        window_id: WindowId,
    ) -> bool {
        self.host_scenes
            .discard_frame_for_viewport(space, window_id)
    }

    pub(crate) fn discard_exact_frame(&mut self, frame: &DockViewportHostSceneFrame) -> bool {
        self.host_scenes.discard_exact_frame(frame)
    }

    pub(crate) fn unregister_window_scene(&mut self, window_id: WindowId) {
        self.host_scenes.unregister_window(window_id);
    }
}

#[cfg(test)]
mod tests {}
