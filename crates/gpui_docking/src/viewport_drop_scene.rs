use crate::drop_target::DockResolvedDropTarget;
use crate::{
    DockNodeId, DockPolicy, DockSpaceId, DockViewportHostGeometry, DockViewportIdentity,
    drop_runtime::{DockHostDropScene, DockHostDropSceneFact},
    drop_target::{DockDropResolution, DockDropTargetValidator, DockEdgePlanResolver},
    geometry::DockDropGuideMetrics,
    viewport_registry::{DockViewportRegistrationKey, DockViewportWindowBoundsFrame},
};
#[cfg(test)]
use open_gpui::point;
use open_gpui::{Bounds, Pixels, Point, Size, WindowId};
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DockViewportHostSceneFrame {
    identity: DockViewportIdentity,
    registration_key: DockViewportRegistrationKey,
    generation: u64,
}

impl DockViewportHostSceneFrame {
    #[cfg(test)]
    pub(crate) fn new_for_test(
        registration_key: DockViewportRegistrationKey,
        generation: u64,
    ) -> Self {
        let identity = DockViewportIdentity::new(
            registration_key.space().clone(),
            registration_key.window_id(),
        );
        Self {
            identity,
            registration_key,
            generation,
        }
    }

    /// Reports whether this render frame still belongs to the supplied viewport binding.
    pub(crate) fn matches_viewport(&self, space: &DockSpaceId, window_id: WindowId) -> bool {
        self.identity.matches(space, window_id)
    }

    fn space(&self) -> &DockSpaceId {
        self.identity.space()
    }

    fn matches_snapshot(&self, snapshot: &DockViewportHostSceneSnapshot) -> bool {
        self.identity == snapshot.identity()
            && self.registration_key == snapshot.registration_key
            && self.generation == snapshot.generation
    }

    pub(crate) fn registration_key(&self) -> &DockViewportRegistrationKey {
        &self.registration_key
    }

    pub(crate) const fn generation(&self) -> u64 {
        self.generation
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DockViewportHostSceneRegistration {
    pub(crate) changed: bool,
    pub(crate) placement_changed: bool,
    pub(crate) frame: DockViewportHostSceneFrame,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct DockViewportHostSceneSnapshot {
    pub(crate) space: DockSpaceId,
    pub(crate) window_id: WindowId,
    pub(crate) current_bounds: DockViewportWindowBoundsFrame,
    pub(crate) host_geometry: DockViewportHostGeometry,
    registration_key: DockViewportRegistrationKey,
    generation: u64,
    scene: DockHostDropScene,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct DockViewportHostSceneDraft {
    pub(crate) space: DockSpaceId,
    pub(crate) window_id: WindowId,
    pub(crate) current_bounds: DockViewportWindowBoundsFrame,
    pub(crate) host_geometry: DockViewportHostGeometry,
    scene: DockHostDropScene,
}

impl DockViewportHostSceneDraft {
    pub(crate) fn has_same_native_routing_content(&self, other: &Self) -> bool {
        self.space == other.space
            && self.window_id == other.window_id
            && self.current_bounds == other.current_bounds
            && self
                .host_geometry
                .has_same_native_routing_geometry(&other.host_geometry)
            && self.scene == other.scene
    }

    pub(crate) fn new(
        space: DockSpaceId,
        window_id: WindowId,
        current_bounds: DockViewportWindowBoundsFrame,
        host_geometry: impl Into<DockViewportHostGeometry>,
        host_position: Point<Pixels>,
        drop_guide_metrics: DockDropGuideMetrics,
    ) -> Self {
        let host_geometry = host_geometry.into();
        let layout_position = host_geometry
            .host_to_layout(host_position)
            .expect("finite host-local positions must remain representable in layout space");
        Self {
            space,
            window_id,
            current_bounds,
            host_geometry,
            scene: DockHostDropScene::new(layout_position)
                .with_drop_guide_metrics(drop_guide_metrics),
        }
    }

    pub(crate) fn new_with_facts(
        space: DockSpaceId,
        window_id: WindowId,
        current_bounds: DockViewportWindowBoundsFrame,
        host_geometry: impl Into<DockViewportHostGeometry>,
        host_position: Point<Pixels>,
        drop_guide_metrics: DockDropGuideMetrics,
        initial_facts: impl IntoIterator<Item = DockHostDropSceneFact>,
    ) -> Self {
        let mut snapshot = Self::new(
            space,
            window_id,
            current_bounds,
            host_geometry,
            host_position,
            drop_guide_metrics,
        );
        for fact in initial_facts {
            snapshot.push_fact(fact);
        }
        snapshot
    }

    pub(crate) fn bind(
        self,
        registration_key: DockViewportRegistrationKey,
    ) -> Option<DockViewportHostSceneSnapshot> {
        if registration_key.space() != &self.space || registration_key.window_id() != self.window_id
        {
            return None;
        }
        Some(DockViewportHostSceneSnapshot {
            space: self.space,
            window_id: self.window_id,
            current_bounds: self.current_bounds,
            host_geometry: self.host_geometry,
            registration_key,
            generation: 0,
            scene: self.scene,
        })
    }

    pub(crate) fn push_fact(&mut self, fact: DockHostDropSceneFact) -> bool {
        self.scene.push_fact(fact)
    }
}

impl DockViewportHostSceneSnapshot {
    #[cfg(test)]
    pub(crate) fn new(
        space: DockSpaceId,
        window_id: WindowId,
        current_bounds: DockViewportWindowBoundsFrame,
        host_geometry: impl Into<DockViewportHostGeometry>,
        host_position: Point<Pixels>,
        drop_guide_metrics: DockDropGuideMetrics,
    ) -> Self {
        let registration_key = DockViewportRegistrationKey::for_test(space.clone(), window_id);
        DockViewportHostSceneDraft::new(
            space,
            window_id,
            current_bounds,
            host_geometry,
            host_position,
            drop_guide_metrics,
        )
        .bind(registration_key)
        .expect("matching test registration must bind")
    }

    #[cfg(test)]
    pub(crate) fn new_with_facts(
        space: DockSpaceId,
        window_id: WindowId,
        current_bounds: DockViewportWindowBoundsFrame,
        host_geometry: impl Into<DockViewportHostGeometry>,
        host_position: Point<Pixels>,
        drop_guide_metrics: DockDropGuideMetrics,
        initial_facts: impl IntoIterator<Item = DockHostDropSceneFact>,
    ) -> Self {
        let registration_key = DockViewportRegistrationKey::for_test(space.clone(), window_id);
        DockViewportHostSceneDraft::new_with_facts(
            space,
            window_id,
            current_bounds,
            host_geometry,
            host_position,
            drop_guide_metrics,
            initial_facts,
        )
        .bind(registration_key)
        .expect("matching test registration must bind")
    }

    fn same_content_as(&self, other: &Self) -> bool {
        self.space == other.space
            && self.window_id == other.window_id
            && self.registration_key == other.registration_key
            && self.current_bounds == other.current_bounds
            && self.host_geometry == other.host_geometry
            && self.scene == other.scene
    }

    fn frame(&self) -> DockViewportHostSceneFrame {
        DockViewportHostSceneFrame {
            identity: self.identity(),
            registration_key: self.registration_key.clone(),
            generation: self.generation,
        }
    }

    fn identity(&self) -> DockViewportIdentity {
        DockViewportIdentity::new(self.space.clone(), self.window_id)
    }

    pub(crate) fn registration_key(&self) -> &DockViewportRegistrationKey {
        &self.registration_key
    }

    fn push_fact(&mut self, fact: DockHostDropSceneFact) -> bool {
        self.scene.push_fact(fact)
    }

    fn leaf_bounds_for_tabs(&self, tabs: DockNodeId) -> Option<Bounds<Pixels>> {
        self.scene
            .leaves
            .iter()
            .find(|leaf| leaf.target_tabs == tabs)
            .map(|leaf| leaf.bounds)
    }

    fn leaf_displayed_bounds_for_tabs(&self, tabs: DockNodeId) -> Option<Bounds<Pixels>> {
        let bounds = self.leaf_bounds_for_tabs(tabs)?;
        self.host_geometry.layout_to_window_bounds(bounds)
    }

    fn tab_bar_bounds_for_tabs(&self, tabs: DockNodeId) -> Option<Bounds<Pixels>> {
        self.scene
            .tab_bars
            .iter()
            .find(|target| target.target_tabs == tabs)
            .map(|target| target.bounds)
    }

    fn tab_label_bounds_for_tabs(
        &self,
        tabs: DockNodeId,
        target_index: usize,
    ) -> Option<Bounds<Pixels>> {
        self.scene
            .tab_labels
            .iter()
            .find(|target| target.target_tabs == tabs && target.target_index == target_index)
            .map(|target| target.bounds)
    }

    #[cfg(test)]
    pub(crate) fn global_screen_position(&self) -> Option<Point<Pixels>> {
        let screen_bounds = self.current_bounds.global_screen_bounds()?;
        let host_position = self.host_geometry.layout_to_host(self.scene.position)?;
        let window_position = self.host_geometry.host_to_window(host_position)?;
        Some(point(
            screen_bounds.origin.x + window_position.x,
            screen_bounds.origin.y + window_position.y,
        ))
    }
}

#[derive(Debug, Default)]
pub(crate) struct DockViewportHostSceneRegistry {
    scenes: BTreeMap<DockSpaceId, DockViewportHostSceneSnapshot>,
    next_generation: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct DockViewportResolvedFrame {
    pub(crate) frame: DockViewportHostSceneFrame,
    pub(crate) drop_guide_metrics: DockDropGuideMetrics,
    pub(crate) resolution: DockViewportFrameResolution,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum DockViewportFrameResolution {
    Drop(DockDropResolution),
    GuideOnly(DockResolvedDropTarget),
}

impl DockViewportFrameResolution {
    #[cfg(test)]
    pub(crate) fn target(self) -> Option<DockResolvedDropTarget> {
        match self {
            Self::Drop(resolution) => resolution.target(),
            Self::GuideOnly(target) => Some(target),
        }
    }
}

impl DockViewportHostSceneRegistry {
    pub(crate) fn register(
        &mut self,
        mut snapshot: DockViewportHostSceneSnapshot,
    ) -> DockViewportHostSceneRegistration {
        if let Some(existing) = self.scenes.get(&snapshot.space) {
            if existing.identity() == snapshot.identity()
                && existing.registration_key == snapshot.registration_key
            {
                snapshot
                    .scene
                    .preserve_measured_tab_labels_from(&existing.scene);
            }
        }
        let changed = self
            .scenes
            .get(&snapshot.space)
            .is_none_or(|existing| !existing.same_content_as(&snapshot));
        if changed {
            self.next_generation = self
                .next_generation
                .checked_add(1)
                .expect("dock viewport host-scene generation exhausted");
            snapshot.generation = self.next_generation;
        } else if let Some(existing) = self.scenes.get(&snapshot.space) {
            snapshot.generation = existing.generation;
        }
        let frame = snapshot.frame();
        self.scenes.insert(snapshot.space.clone(), snapshot);
        DockViewportHostSceneRegistration {
            changed,
            placement_changed: false,
            frame,
        }
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
        if !scene.push_fact(fact) {
            return None;
        }
        self.next_generation = self.next_generation.wrapping_add(1);
        scene.generation = self.next_generation;
        Some(scene.frame())
    }

    pub(crate) fn is_current_frame(&self, frame: &DockViewportHostSceneFrame) -> bool {
        self.scenes
            .get(frame.space())
            .is_some_and(|snapshot| frame.matches_snapshot(snapshot))
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

    pub(crate) fn leaf_displayed_bounds_for_tabs(
        &self,
        space: &DockSpaceId,
        window_id: Option<WindowId>,
        tabs: DockNodeId,
    ) -> Option<Bounds<Pixels>> {
        let snapshot = self.scenes.get(space)?;
        if window_id.is_some_and(|window_id| !snapshot.identity().matches(space, window_id)) {
            return None;
        }
        snapshot.leaf_displayed_bounds_for_tabs(tabs)
    }

    pub(crate) fn tab_bar_bounds_for_tabs(
        &self,
        space: &DockSpaceId,
        window_id: Option<WindowId>,
        tabs: DockNodeId,
    ) -> Option<Bounds<Pixels>> {
        let snapshot = self.scenes.get(space)?;
        if window_id.is_some_and(|window_id| !snapshot.identity().matches(space, window_id)) {
            return None;
        }
        snapshot.tab_bar_bounds_for_tabs(tabs)
    }

    pub(crate) fn tab_label_bounds_for_tabs(
        &self,
        space: &DockSpaceId,
        window_id: Option<WindowId>,
        tabs: DockNodeId,
        target_index: usize,
    ) -> Option<Bounds<Pixels>> {
        let snapshot = self.scenes.get(space)?;
        if window_id.is_some_and(|window_id| !snapshot.identity().matches(space, window_id)) {
            return None;
        }
        snapshot.tab_label_bounds_for_tabs(tabs, target_index)
    }

    #[cfg(test)]
    pub(crate) fn scene_for_window(
        &self,
        space: &DockSpaceId,
        window_id: WindowId,
    ) -> Option<DockHostDropScene> {
        let snapshot = self.scenes.get(space)?;
        if !snapshot.identity().matches(space, window_id) {
            return None;
        }
        Some(snapshot.scene.clone())
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
        .and_then(|resolved| resolved.resolution.target())
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
    ) -> Option<DockViewportResolvedFrame> {
        let Some(snapshot) = self.scenes.get(space) else {
            return None;
        };
        if window_id.is_some_and(|window_id| !snapshot.identity().matches(space, window_id)) {
            return None;
        }
        let frame = snapshot.frame();
        let drop_guide_metrics = snapshot.scene.drop_guide_metrics();
        let mut scene = snapshot.scene.clone().excluding_nodes(excluded_nodes);
        scene.position = snapshot.host_geometry.host_to_layout(host_position)?;
        scene = scene.with_payload_size(payload_size);
        let resolution = scene
            .resolve_drop_with_validator(policy, target_validator, edge_plan_resolver)
            .map(DockViewportFrameResolution::Drop)
            .or_else(|| {
                scene
                    .resolve_guide_target_with_validator(
                        policy,
                        target_validator,
                        edge_plan_resolver,
                    )
                    .map(DockViewportFrameResolution::GuideOnly)
            });
        let Some(resolution) = resolution else {
            return None;
        };
        Some(DockViewportResolvedFrame {
            frame,
            drop_guide_metrics,
            resolution,
        })
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

    pub(crate) fn discard_frame_for_viewport(
        &mut self,
        space: &DockSpaceId,
        window_id: WindowId,
    ) -> bool {
        if self
            .scenes
            .get(space)
            .is_none_or(|snapshot| snapshot.window_id != window_id)
        {
            return false;
        }
        self.scenes.remove(space);
        true
    }

    pub(crate) fn discard_exact_frame(&mut self, frame: &DockViewportHostSceneFrame) -> bool {
        if self
            .scenes
            .get(frame.space())
            .is_none_or(|snapshot| !frame.matches_snapshot(snapshot))
        {
            return false;
        }
        self.scenes.remove(frame.space());
        true
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
        drop_target::{
            DockEmptySpaceDropTarget, DockLeafDropTarget, DockResolvedDropTargetKind,
            DockTabBarDropTarget, DockTabLabelDropTarget,
        },
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
    fn host_scene_frame_rejects_same_window_from_replaced_registration() {
        let space = space("main");
        let window_id = WindowId::from(1);
        let mut registry = DockViewportHostSceneRegistry::default();
        let first_registration =
            DockViewportRegistrationKey::for_test_generation(space.clone(), window_id, 1);
        let replacement_registration =
            DockViewportRegistrationKey::for_test_generation(space.clone(), window_id, 2);

        let old_frame = registry
            .register(
                DockViewportHostSceneDraft::new(
                    space.clone(),
                    window_id,
                    DockViewportWindowBoundsFrame::GlobalScreen(bounds(0.0, 0.0, 800.0, 600.0)),
                    bounds(0.0, 0.0, 800.0, 600.0),
                    point(px(10.0), px(10.0)),
                    DockDropGuideMetrics::default(),
                )
                .bind(first_registration)
                .expect("matching registration must bind"),
            )
            .frame;
        let replacement_frame = registry
            .register(
                DockViewportHostSceneDraft::new(
                    space.clone(),
                    window_id,
                    DockViewportWindowBoundsFrame::GlobalScreen(bounds(0.0, 0.0, 800.0, 600.0)),
                    bounds(0.0, 0.0, 800.0, 600.0),
                    point(px(10.0), px(10.0)),
                    DockDropGuideMetrics::default(),
                )
                .bind(replacement_registration)
                .expect("matching replacement registration must bind"),
            )
            .frame;

        assert_ne!(old_frame, replacement_frame);
        assert!(
            registry
                .push_frame_fact(&old_frame, empty_space_fact(space.clone()))
                .is_none(),
            "a frame from the replaced registration must not mutate the replacement scene"
        );
        assert!(
            registry
                .push_frame_fact(&replacement_frame, empty_space_fact(space.clone()))
                .is_some()
        );
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
    fn host_scene_preserves_measured_tab_labels_without_generation_churn() {
        let space = space("main");
        let window_id = WindowId::from(1);
        let mut graph = DockGraph::new();
        let tabs = graph.insert_node(DockNode::Tabs {
            items: Vec::new(),
            selected: None,
        });
        let tab_bar_fact = tab_bar_fact(tabs, 2, false);
        let label_bounds = bounds(8.0, 4.0, 72.0, 28.0);
        let label_fact = tab_label_fact(tabs, 0, label_bounds, false);
        let mut registry = DockViewportHostSceneRegistry::default();

        let first = registry
            .register(snapshot_with_facts(
                space.clone(),
                window_id,
                vec![tab_bar_fact.clone()],
            ))
            .frame;
        let measured = registry
            .push_frame_fact(&first, label_fact.clone())
            .expect("measured tab-label fact should advance the frame once");

        assert_eq!(
            registry.tab_label_bounds_for_tabs(&space, Some(window_id), tabs, 0),
            Some(label_bounds)
        );
        assert!(
            registry
                .push_frame_fact(&measured, label_fact.clone())
                .is_none(),
            "pushing the same measured label should be a no-op"
        );

        let rerender = registry
            .register(snapshot_with_facts(
                space.clone(),
                window_id,
                vec![tab_bar_fact],
            ))
            .frame;
        assert_eq!(
            rerender, measured,
            "base-scene re-registration should preserve current measured labels"
        );
        assert_eq!(
            registry.tab_label_bounds_for_tabs(&space, Some(window_id), tabs, 0),
            Some(label_bounds)
        );

        let next_bounds = bounds(10.0, 4.0, 74.0, 28.0);
        let next = registry
            .push_frame_fact(&rerender, tab_label_fact(tabs, 0, next_bounds, false))
            .expect("changed measured tab-label bounds should advance the frame");
        assert_ne!(next, rerender);
        assert_eq!(
            registry.tab_label_bounds_for_tabs(&space, Some(window_id), tabs, 0),
            Some(next_bounds)
        );
    }

    #[test]
    fn host_scene_drops_measured_tab_labels_without_matching_tab_slot() {
        let space = space("main");
        let window_id = WindowId::from(1);
        let mut graph = DockGraph::new();
        let tabs = graph.insert_node(DockNode::Tabs {
            items: Vec::new(),
            selected: None,
        });
        let stale_label_bounds = bounds(90.0, 4.0, 72.0, 28.0);
        let mut registry = DockViewportHostSceneRegistry::default();

        let first = registry
            .register(snapshot_with_facts(
                space.clone(),
                window_id,
                vec![tab_bar_fact(tabs, 2, false)],
            ))
            .frame;
        registry
            .push_frame_fact(&first, tab_label_fact(tabs, 1, stale_label_bounds, false))
            .expect("second tab label should be measured in the first frame");

        registry.register(snapshot_with_facts(
            space.clone(),
            window_id,
            vec![tab_bar_fact(tabs, 1, false)],
        ));

        assert_eq!(
            registry.tab_label_bounds_for_tabs(&space, Some(window_id), tabs, 1),
            None,
            "measured labels beyond the current tab-bar insert index must not persist"
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
                crate::DockDropGuideMetrics::default(),
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
                crate::DockDropGuideMetrics::default(),
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
            crate::DockDropGuideMetrics::default(),
        )
    }

    fn snapshot_with_facts(
        space: DockSpaceId,
        window_id: WindowId,
        facts: Vec<DockHostDropSceneFact>,
    ) -> DockViewportHostSceneSnapshot {
        DockViewportHostSceneSnapshot::new_with_facts(
            space,
            window_id,
            DockViewportWindowBoundsFrame::GlobalScreen(bounds(0.0, 0.0, 200.0, 120.0)),
            bounds(0.0, 0.0, 200.0, 120.0),
            point(px(10.0), px(10.0)),
            crate::DockDropGuideMetrics::default(),
            facts,
        )
    }

    fn empty_space_fact(space: DockSpaceId) -> DockHostDropSceneFact {
        DockHostDropSceneFact::EmptySpace(DockEmptySpaceDropTarget {
            space,
            bounds: bounds(0.0, 0.0, 200.0, 120.0),
            is_central: false,
        })
    }

    fn tab_bar_fact(
        target_tabs: DockNodeId,
        insert_index: usize,
        is_central: bool,
    ) -> DockHostDropSceneFact {
        DockHostDropSceneFact::TabBar(DockTabBarDropTarget {
            target_tabs,
            insert_index,
            bounds: bounds(0.0, 0.0, 200.0, 36.0),
            is_central,
        })
    }

    fn tab_label_fact(
        target_tabs: DockNodeId,
        target_index: usize,
        label_bounds: Bounds<Pixels>,
        is_central: bool,
    ) -> DockHostDropSceneFact {
        DockHostDropSceneFact::TabLabel(DockTabLabelDropTarget {
            target_tabs,
            target_index,
            bounds: label_bounds,
            is_central,
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
