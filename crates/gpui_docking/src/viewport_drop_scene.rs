#[cfg(test)]
use crate::drop_target::DockResolvedDropTarget;
use crate::{
    DockNodeId, DockPolicy, DockSpaceId, DockViewportIdentity,
    drop_runtime::{DockHostDropScene, DockHostDropSceneFact},
    drop_target::{DockDropResolution, DockDropTargetValidator, DockEdgePlanResolver},
    geometry::DockDropGuideStyle,
    viewport_registry::DockViewportWindowBoundsFrame,
};
use open_gpui::{Bounds, Pixels, Point, Size, WindowId, point};
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DockViewportHostSceneFrame {
    identity: DockViewportIdentity,
    generation: u64,
}

impl DockViewportHostSceneFrame {
    #[cfg(test)]
    pub(crate) fn new_for_test(
        space: impl Into<DockSpaceId>,
        window_id: WindowId,
        generation: u64,
    ) -> Self {
        Self {
            identity: DockViewportIdentity::new(space, window_id),
            generation,
        }
    }

    /// Reports whether this render frame still belongs to the supplied viewport binding.
    pub(crate) fn matches_viewport(&self, space: &DockSpaceId, window_id: WindowId) -> bool {
        self.identity.matches(space, window_id)
    }

    pub(crate) fn generation(&self) -> u64 {
        self.generation
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
    pub(crate) current_bounds: DockViewportWindowBoundsFrame,
    pub(crate) host_bounds: Bounds<Pixels>,
    generation: u64,
    scene: DockHostDropScene,
}

impl DockViewportHostSceneSnapshot {
    pub(crate) fn new(
        space: DockSpaceId,
        window_id: WindowId,
        current_bounds: DockViewportWindowBoundsFrame,
        host_bounds: Bounds<Pixels>,
        host_position: Point<Pixels>,
        drop_guide_style: DockDropGuideStyle,
    ) -> Self {
        let window_position = point(
            host_bounds.origin.x + host_position.x,
            host_bounds.origin.y + host_position.y,
        );
        Self {
            space,
            window_id,
            current_bounds,
            host_bounds,
            generation: 0,
            scene: DockHostDropScene::new(window_position).with_drop_guide_style(drop_guide_style),
        }
    }

    fn same_content_as(&self, other: &Self) -> bool {
        self.space == other.space
            && self.window_id == other.window_id
            && self.current_bounds == other.current_bounds
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

    fn leaf_bounds_for_tabs(&self, tabs: DockNodeId) -> Option<Bounds<Pixels>> {
        self.scene
            .leaves
            .iter()
            .find(|leaf| leaf.target_tabs == tabs)
            .map(|leaf| leaf.bounds)
    }

    #[cfg(test)]
    pub(crate) fn global_screen_position(&self) -> Option<Point<Pixels>> {
        let screen_bounds = self.current_bounds.global_screen_bounds()?;
        Some(point(
            screen_bounds.origin.x + self.scene.position.x,
            screen_bounds.origin.y + self.scene.position.y,
        ))
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
        if changed {
            self.next_generation = self.next_generation.wrapping_add(1);
            snapshot.generation = self.next_generation;
        } else if let Some(existing) = self.scenes.get(&snapshot.space) {
            snapshot.generation = existing.generation;
        }
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
        self.push_frame_fact(&frame, fact).is_some()
    }

    pub(crate) fn push_frame_fact(
        &mut self,
        frame: &DockViewportHostSceneFrame,
        fact: DockHostDropSceneFact,
    ) -> Option<DockViewportHostSceneFrame> {
        let Some(scene) = self.scenes.get_mut(frame.space()) else {
            return None;
        };
        if !frame.matches_snapshot(scene) {
            return None;
        }
        scene.push_fact(fact);
        self.next_generation = self.next_generation.wrapping_add(1);
        scene.generation = self.next_generation;
        Some(scene.frame())
    }

    pub(crate) fn leaf_bounds_for_tabs(
        &self,
        space: &DockSpaceId,
        window_id: Option<WindowId>,
        tabs: DockNodeId,
    ) -> Option<Bounds<Pixels>> {
        let snapshot = self.scenes.get(space)?;
        if window_id.is_some_and(|window_id| !snapshot.identity().matches(space, window_id)) {
            return None;
        }
        snapshot.leaf_bounds_for_tabs(tabs)
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
        self.resolve_for_window(space, None, host_position, policy, None)
    }

    #[cfg(test)]
    pub(crate) fn resolve_for_window(
        &self,
        space: &DockSpaceId,
        window_id: Option<WindowId>,
        host_position: Point<Pixels>,
        policy: &DockPolicy,
        target_validator: Option<&DockDropTargetValidator<'_>>,
    ) -> Option<DockResolvedDropTarget> {
        self.resolve_frame_for_window(
            space,
            window_id,
            host_position,
            None,
            Vec::new(),
            policy,
            target_validator,
            None,
        )
        .and_then(|(_, resolution)| resolution.target())
    }

    pub(crate) fn resolve_frame_for_window(
        &self,
        space: &DockSpaceId,
        window_id: Option<WindowId>,
        host_position: Point<Pixels>,
        payload_size: Option<Size<Pixels>>,
        excluded_nodes: Vec<DockNodeId>,
        policy: &DockPolicy,
        target_validator: Option<&DockDropTargetValidator<'_>>,
        edge_plan_resolver: Option<&DockEdgePlanResolver<'_>>,
    ) -> Option<(DockViewportHostSceneFrame, DockDropResolution)> {
        let snapshot = self.scenes.get(space)?;
        if window_id.is_some_and(|window_id| !snapshot.identity().matches(space, window_id)) {
            return None;
        }
        let frame = snapshot.frame();
        let mut scene = snapshot.scene.clone().excluding_nodes(excluded_nodes);
        scene.position = point(
            snapshot.host_bounds.origin.x + host_position.x,
            snapshot.host_bounds.origin.y + host_position.y,
        );
        scene = scene.with_payload_size(payload_size);
        let resolution =
            scene.resolve_drop_with_validator(policy, target_validator, edge_plan_resolver)?;
        Some((frame, resolution))
    }

    #[cfg(test)]
    pub(crate) fn screen_position(&self, space: &DockSpaceId) -> Option<Point<Pixels>> {
        self.scenes
            .get(space)
            .and_then(DockViewportHostSceneSnapshot::global_screen_position)
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
        DockGraph, DockNode, DockPolicy,
        drop_target::{DockEmptySpaceDropTarget, DockLeafDropTarget, DockResolvedDropTargetKind},
        viewport_test_support::{bounds, space},
    };
    use open_gpui::{WindowId, point, px};

    #[test]
    fn host_scene_frame_rejects_facts_from_stale_generation() {
        let space = space("main");
        let window_id = WindowId::from(1);
        let mut registry = DockViewportHostSceneRegistry::default();

        let first = registry.register(snapshot(space.clone(), window_id)).frame;
        let first_after_fact = registry
            .push_frame_fact(&first, empty_space_fact(space.clone()))
            .expect("first frame should accept a fact");
        assert_ne!(
            first, first_after_fact,
            "scene content changes should advance frame generation"
        );
        assert_empty_space_target(&registry, &space);
        assert!(
            registry
                .push_frame_fact(&first, empty_space_fact(space.clone()))
                .is_none(),
            "the frame captured before a scene mutation should become stale"
        );

        let second = registry.register(snapshot(space.clone(), window_id)).frame;
        assert!(
            registry
                .push_frame_fact(&first, empty_space_fact(space.clone()))
                .is_none()
        );
        assert!(
            registry
                .resolve(&space, point(px(10.0), px(10.0)), &DockPolicy::default())
                .is_none(),
            "new frame should start without stale facts from the previous generation"
        );

        assert!(
            registry
                .push_frame_fact(&second, empty_space_fact(space.clone()))
                .is_some()
        );
        assert_empty_space_target(&registry, &space);
    }

    #[test]
    fn host_scene_resolve_rejects_stale_window_identity() {
        let space = space("main");
        let old_window = WindowId::from(1);
        let new_window = WindowId::from(2);
        let mut registry = DockViewportHostSceneRegistry::default();

        let old_frame = registry.register(snapshot(space.clone(), old_window)).frame;
        assert!(
            registry
                .push_frame_fact(&old_frame, empty_space_fact(space.clone()))
                .is_some()
        );
        assert!(
            registry
                .resolve_for_window(
                    &space,
                    Some(old_window),
                    point(px(10.0), px(10.0)),
                    &DockPolicy::default(),
                    None,
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
                    None,
                )
                .is_none(),
            "a different window id must not consume another window's scene"
        );

        let new_frame = registry.register(snapshot(space.clone(), new_window)).frame;
        assert!(
            registry
                .push_frame_fact(&new_frame, empty_space_fact(space.clone()))
                .is_some()
        );
        assert!(
            registry
                .resolve_for_window(
                    &space,
                    Some(old_window),
                    point(px(10.0), px(10.0)),
                    &DockPolicy::default(),
                    None,
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
                    None,
                )
                .is_some(),
            "the reopened window should resolve its own scene"
        );
    }

    #[test]
    fn host_scene_register_preserves_generation_for_identical_content() {
        let space = space("main");
        let window_id = WindowId::from(1);
        let mut registry = DockViewportHostSceneRegistry::default();

        let first = registry.register(snapshot(space.clone(), window_id)).frame;
        let second = registry.register(snapshot(space.clone(), window_id)).frame;

        assert_eq!(
            first, second,
            "identical scene content should keep the same frame"
        );
        assert!(
            registry
                .push_frame_fact(&first, empty_space_fact(space.clone()))
                .is_some(),
            "the preserved frame should remain current after identical re-render registration"
        );
    }

    #[test]
    fn host_scene_resolve_applies_host_bounds_origin_once() {
        let space = space("main");
        let window_id = WindowId::from(1);
        let mut registry = DockViewportHostSceneRegistry::default();
        let screen_bounds = bounds(100.0, 200.0, 320.0, 240.0);
        let host_bounds = bounds(40.0, 30.0, 10.0, 10.0);
        let host_position = point(px(5.0), px(6.0));

        let frame = registry
            .register(DockViewportHostSceneSnapshot::new(
                space.clone(),
                window_id,
                DockViewportWindowBoundsFrame::GlobalScreen(screen_bounds),
                host_bounds,
                host_position,
                crate::DockDropGuideStyle::default(),
            ))
            .frame;
        assert!(
            registry
                .push_frame_fact(
                    &frame,
                    DockHostDropSceneFact::EmptySpace(DockEmptySpaceDropTarget {
                        space: space.clone(),
                        bounds: host_bounds,
                        is_central: false,
                    })
                )
                .is_some()
        );

        let target = registry
            .resolve_for_window(
                &space,
                Some(window_id),
                host_position,
                &DockPolicy::default(),
                None,
            )
            .expect("offset host scene should still resolve");

        assert!(
            matches!(
                target.kind,
                DockResolvedDropTargetKind::EmptyDockSpace { space: ref resolved_space, .. }
                    if resolved_space == &space
            ),
            "expected empty-space target, got {:?}",
            target
        );
        assert_eq!(target.preview_bounds, Some(host_bounds));
    }

    #[test]
    fn host_scene_resolves_window_local_bounds_without_global_screen_position() {
        let space = space("main");
        let window_id = WindowId::from(1);
        let mut registry = DockViewportHostSceneRegistry::default();
        let host_bounds = bounds(40.0, 30.0, 200.0, 120.0);
        let host_position = point(px(5.0), px(6.0));

        let frame = registry
            .register(DockViewportHostSceneSnapshot::new(
                space.clone(),
                window_id,
                DockViewportWindowBoundsFrame::WindowLocal(bounds(0.0, 0.0, 320.0, 240.0)),
                host_bounds,
                host_position,
                crate::DockDropGuideStyle::default(),
            ))
            .frame;
        assert!(
            registry
                .push_frame_fact(
                    &frame,
                    DockHostDropSceneFact::EmptySpace(DockEmptySpaceDropTarget {
                        space: space.clone(),
                        bounds: host_bounds,
                        is_central: false,
                    })
                )
                .is_some()
        );

        assert_eq!(registry.screen_position(&space), None);
        assert!(
            registry
                .resolve_for_window(
                    &space,
                    Some(window_id),
                    host_position,
                    &DockPolicy::default(),
                    None,
                )
                .is_some(),
            "window-local scene facts should still resolve for the receiving viewport"
        );
    }

    #[test]
    fn host_scene_leaf_bounds_for_tabs_is_bound_to_space_and_window() {
        let source_space = space("main");
        let other_space = space("other");
        let window_id = WindowId::from(1);
        let other_window_id = WindowId::from(2);
        let mut graph = DockGraph::new();
        let tabs = graph.insert_node(DockNode::Tabs {
            items: Vec::new(),
            selected: None,
        });
        let leaf_bounds = bounds(20.0, 30.0, 400.0, 240.0);
        let mut registry = DockViewportHostSceneRegistry::default();

        let frame = registry
            .register(snapshot(source_space.clone(), window_id))
            .frame;
        assert!(
            registry
                .push_frame_fact(
                    &frame,
                    DockHostDropSceneFact::Leaf(DockLeafDropTarget {
                        root: tabs,
                        target_tabs: tabs,
                        bounds: leaf_bounds,
                        is_central: false,
                    })
                )
                .is_some()
        );

        assert_eq!(
            registry.leaf_bounds_for_tabs(&source_space, Some(window_id), tabs),
            Some(leaf_bounds)
        );
        assert_eq!(
            registry.leaf_bounds_for_tabs(&source_space, Some(other_window_id), tabs),
            None,
            "a stale window id must not reuse another viewport's leaf bounds"
        );
        assert_eq!(
            registry.leaf_bounds_for_tabs(&other_space, Some(window_id), tabs),
            None,
            "leaf bounds are scoped to the rendered dock space"
        );
    }

    fn snapshot(space: DockSpaceId, window_id: WindowId) -> DockViewportHostSceneSnapshot {
        DockViewportHostSceneSnapshot::new(
            space,
            window_id,
            DockViewportWindowBoundsFrame::GlobalScreen(bounds(0.0, 0.0, 200.0, 120.0)),
            bounds(0.0, 0.0, 200.0, 120.0),
            point(px(10.0), px(10.0)),
            crate::DockDropGuideStyle::default(),
        )
    }

    fn empty_space_fact(space: DockSpaceId) -> DockHostDropSceneFact {
        DockHostDropSceneFact::EmptySpace(DockEmptySpaceDropTarget {
            space,
            bounds: bounds(0.0, 0.0, 200.0, 120.0),
            is_central: false,
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
                DockResolvedDropTargetKind::EmptyDockSpace { ref space, .. }
                    if space == expected_space
            ),
            "expected empty-space target, got {:?}",
            target
        );
    }
}
