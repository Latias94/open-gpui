use crate::{
    DockPolicy, DockSpaceId,
    drop_runtime::{DockHostDropScene, DockHostDropSceneFact},
    drop_target::DockResolvedDropTarget,
};
use open_gpui::{Bounds, Pixels, Point, WindowBounds, WindowId, point};
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct DockViewportHostSceneSnapshot {
    pub(crate) space: DockSpaceId,
    pub(crate) window_id: WindowId,
    pub(crate) window_bounds: WindowBounds,
    pub(crate) host_bounds: Bounds<Pixels>,
    scene: DockHostDropScene,
}

impl DockViewportHostSceneSnapshot {
    pub(crate) fn new(
        space: DockSpaceId,
        window_id: WindowId,
        window_bounds: WindowBounds,
        host_bounds: Bounds<Pixels>,
        host_position: Point<Pixels>,
    ) -> Self {
        let window_position = point(
            host_bounds.origin.x + host_position.x,
            host_bounds.origin.y + host_position.y,
        );
        Self {
            space,
            window_id,
            window_bounds,
            host_bounds,
            scene: DockHostDropScene::new(window_position),
        }
    }

    pub(crate) fn push_fact(&mut self, fact: DockHostDropSceneFact) {
        self.scene.push_fact(fact);
    }

    #[cfg(test)]
    pub(crate) fn screen_position(&self) -> Point<Pixels> {
        let window_bounds = self.window_bounds.get_bounds();
        point(
            window_bounds.origin.x + self.scene.position.x,
            window_bounds.origin.y + self.scene.position.y,
        )
    }
}

#[derive(Debug, Default)]
pub(crate) struct DockViewportHostSceneRegistry {
    scenes: BTreeMap<DockSpaceId, DockViewportHostSceneSnapshot>,
}

impl DockViewportHostSceneRegistry {
    pub(crate) fn register(&mut self, snapshot: DockViewportHostSceneSnapshot) -> bool {
        let changed = self
            .scenes
            .get(&snapshot.space)
            .is_none_or(|existing| existing != &snapshot);
        self.scenes.insert(snapshot.space.clone(), snapshot);
        changed
    }

    pub(crate) fn push_fact(
        &mut self,
        space: &DockSpaceId,
        window_id: WindowId,
        fact: DockHostDropSceneFact,
    ) -> bool {
        let Some(scene) = self.scenes.get_mut(space) else {
            return false;
        };
        if scene.window_id != window_id {
            return false;
        }

        scene.push_fact(fact);
        true
    }

    pub(crate) fn resolve(
        &self,
        space: &DockSpaceId,
        host_position: Point<Pixels>,
        policy: &DockPolicy,
    ) -> Option<DockResolvedDropTarget> {
        let snapshot = self.scenes.get(space)?;
        let mut scene = snapshot.scene.clone();
        scene.position = point(
            snapshot.host_bounds.origin.x + host_position.x,
            snapshot.host_bounds.origin.y + host_position.y,
        );
        scene.resolved_target(policy)
    }

    #[cfg(test)]
    pub(crate) fn screen_position(&self, space: &DockSpaceId) -> Option<Point<Pixels>> {
        self.scenes
            .get(space)
            .map(DockViewportHostSceneSnapshot::screen_position)
    }

    pub(crate) fn unregister_space(&mut self, space: &DockSpaceId) {
        self.scenes.remove(space);
    }

    pub(crate) fn unregister_window(&mut self, window_id: WindowId) {
        self.scenes
            .retain(|_, snapshot| snapshot.window_id != window_id);
    }
}
