use crate::{
    DockPolicy, DockSpaceId, DockViewportIdentity,
    drop_runtime::{DockHostDropScene, DockHostDropSceneFact},
    drop_target::DockResolvedDropTarget,
};
use open_gpui::{Bounds, Pixels, Point, WindowBounds, WindowId, point};
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DockViewportHostSceneFrame {
    identity: DockViewportIdentity,
    generation: u64,
}

impl DockViewportHostSceneFrame {
    /// Reports whether this render frame still belongs to the supplied viewport binding.
    pub(crate) fn matches_viewport(&self, space: &DockSpaceId, window_id: WindowId) -> bool {
        self.identity.matches(space, window_id)
    }

    fn space(&self) -> &DockSpaceId {
        self.identity.space()
    }

    fn matches_snapshot(&self, snapshot: &DockViewportHostSceneSnapshot) -> bool {
        self.identity == snapshot.identity() && self.generation == snapshot.generation
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DockViewportHostSceneRegistration {
    pub(crate) changed: bool,
    pub(crate) frame: DockViewportHostSceneFrame,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct DockViewportHostSceneSnapshot {
    pub(crate) space: DockSpaceId,
    pub(crate) window_id: WindowId,
    pub(crate) window_bounds: WindowBounds,
    pub(crate) host_bounds: Bounds<Pixels>,
    generation: u64,
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
            generation: 0,
            scene: DockHostDropScene::new(window_position),
        }
    }

    fn same_content_as(&self, other: &Self) -> bool {
        self.space == other.space
            && self.window_id == other.window_id
            && self.window_bounds == other.window_bounds
            && self.host_bounds == other.host_bounds
            && self.scene == other.scene
    }

    fn frame(&self) -> DockViewportHostSceneFrame {
        DockViewportHostSceneFrame {
            identity: self.identity(),
            generation: self.generation,
        }
    }

    fn identity(&self) -> DockViewportIdentity {
        DockViewportIdentity::new(self.space.clone(), self.window_id)
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
    next_generation: u64,
}

impl DockViewportHostSceneRegistry {
    pub(crate) fn register(
        &mut self,
        mut snapshot: DockViewportHostSceneSnapshot,
    ) -> DockViewportHostSceneRegistration {
        let changed = self
            .scenes
            .get(&snapshot.space)
            .is_none_or(|existing| !existing.same_content_as(&snapshot));
        self.next_generation = self.next_generation.wrapping_add(1);
        snapshot.generation = self.next_generation;
        let frame = snapshot.frame();
        self.scenes.insert(snapshot.space.clone(), snapshot);
        DockViewportHostSceneRegistration { changed, frame }
    }

    #[cfg(test)]
    pub(crate) fn push_fact(
        &mut self,
        space: &DockSpaceId,
        window_id: WindowId,
        fact: DockHostDropSceneFact,
    ) -> bool {
        let Some(frame) = self.current_frame(space, window_id) else {
            return false;
        };
        self.push_frame_fact(&frame, fact)
    }

    pub(crate) fn push_frame_fact(
        &mut self,
        frame: &DockViewportHostSceneFrame,
        fact: DockHostDropSceneFact,
    ) -> bool {
        let Some(scene) = self.scenes.get_mut(frame.space()) else {
            return false;
        };
        if !frame.matches_snapshot(scene) {
            return false;
        }
        scene.push_fact(fact);
        true
    }

    #[cfg(test)]
    fn current_frame(
        &self,
        space: &DockSpaceId,
        window_id: WindowId,
    ) -> Option<DockViewportHostSceneFrame> {
        let scene = self.scenes.get(space)?;
        if !scene.identity().matches(space, window_id) {
            return None;
        }
        Some(scene.frame())
    }

    #[cfg(test)]
    pub(crate) fn resolve(
        &self,
        space: &DockSpaceId,
        host_position: Point<Pixels>,
        policy: &DockPolicy,
    ) -> Option<DockResolvedDropTarget> {
        self.resolve_for_window(space, None, host_position, policy)
    }

    pub(crate) fn resolve_for_window(
        &self,
        space: &DockSpaceId,
        window_id: Option<WindowId>,
        host_position: Point<Pixels>,
        policy: &DockPolicy,
    ) -> Option<DockResolvedDropTarget> {
        let snapshot = self.scenes.get(space)?;
        if window_id.is_some_and(|window_id| !snapshot.identity().matches(space, window_id)) {
            return None;
        }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        DockPolicy,
        drop_target::{DockEmptySpaceDropTarget, DockResolvedDropTargetKind},
        viewport_test_support::{bounds, space},
    };
    use open_gpui::{WindowBounds, WindowId, point, px};

    #[test]
    fn host_scene_frame_rejects_facts_from_stale_generation() {
        let space = space("main");
        let window_id = WindowId::from(1);
        let mut registry = DockViewportHostSceneRegistry::default();

        let first = registry.register(snapshot(space.clone(), window_id)).frame;
        assert!(registry.push_frame_fact(&first, empty_space_fact(space.clone())));
        assert_empty_space_target(&registry, &space);

        let second = registry.register(snapshot(space.clone(), window_id)).frame;
        assert!(!registry.push_frame_fact(&first, empty_space_fact(space.clone())));
        assert!(
            registry
                .resolve(&space, point(px(10.0), px(10.0)), &DockPolicy::default())
                .is_none(),
            "new frame should start without stale facts from the previous generation"
        );

        assert!(registry.push_frame_fact(&second, empty_space_fact(space.clone())));
        assert_empty_space_target(&registry, &space);
    }

    #[test]
    fn host_scene_resolve_rejects_stale_window_identity() {
        let space = space("main");
        let old_window = WindowId::from(1);
        let new_window = WindowId::from(2);
        let mut registry = DockViewportHostSceneRegistry::default();

        let old_frame = registry.register(snapshot(space.clone(), old_window)).frame;
        assert!(registry.push_frame_fact(&old_frame, empty_space_fact(space.clone())));
        assert!(
            registry
                .resolve_for_window(
                    &space,
                    Some(old_window),
                    point(px(10.0), px(10.0)),
                    &DockPolicy::default(),
                )
                .is_some(),
            "the current window should resolve its own scene"
        );
        assert!(
            registry
                .resolve_for_window(
                    &space,
                    Some(new_window),
                    point(px(10.0), px(10.0)),
                    &DockPolicy::default(),
                )
                .is_none(),
            "a different window id must not consume another window's scene"
        );

        let new_frame = registry.register(snapshot(space.clone(), new_window)).frame;
        assert!(registry.push_frame_fact(&new_frame, empty_space_fact(space.clone())));
        assert!(
            registry
                .resolve_for_window(
                    &space,
                    Some(old_window),
                    point(px(10.0), px(10.0)),
                    &DockPolicy::default(),
                )
                .is_none(),
            "a stale route from the old window must not resolve the reopened scene"
        );
        assert!(
            registry
                .resolve_for_window(
                    &space,
                    Some(new_window),
                    point(px(10.0), px(10.0)),
                    &DockPolicy::default(),
                )
                .is_some(),
            "the reopened window should resolve its own scene"
        );
    }

    fn snapshot(space: DockSpaceId, window_id: WindowId) -> DockViewportHostSceneSnapshot {
        DockViewportHostSceneSnapshot::new(
            space,
            window_id,
            WindowBounds::Windowed(bounds(0.0, 0.0, 200.0, 120.0)),
            bounds(0.0, 0.0, 200.0, 120.0),
            point(px(10.0), px(10.0)),
        )
    }

    fn empty_space_fact(space: DockSpaceId) -> DockHostDropSceneFact {
        DockHostDropSceneFact::EmptySpace(DockEmptySpaceDropTarget {
            space,
            bounds: bounds(0.0, 0.0, 200.0, 120.0),
        })
    }

    fn assert_empty_space_target(
        registry: &DockViewportHostSceneRegistry,
        expected_space: &DockSpaceId,
    ) {
        let target = registry
            .resolve(
                expected_space,
                point(px(10.0), px(10.0)),
                &DockPolicy::default(),
            )
            .expect("empty space fact should resolve");
        assert!(
            matches!(
                target.kind,
                DockResolvedDropTargetKind::EmptyDockSpace { ref space }
                    if space == expected_space
            ),
            "expected empty-space target, got {:?}",
            target
        );
    }
}
