use crate::{
    DockSpaceId, DockViewportIdentity, DockViewportWindowFacts,
    drop_runtime::DockHostDropSceneFact,
    geometry::DockDropGuideStyle,
    viewport_drop_scene::{
        DockViewportHostSceneFrame, DockViewportHostSceneRegistration,
        DockViewportHostSceneRegistry, DockViewportHostSceneSnapshot,
    },
};
use open_gpui::{Bounds, Pixels, Point, WindowId};
use std::collections::HashMap;

/// Token proving that a rendered viewport host scene was observed for a specific runtime binding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DockViewportHostSceneLivenessToken {
    identity: DockViewportIdentity,
    generation: u64,
}

impl DockViewportHostSceneLivenessToken {
    pub(crate) fn identity(&self) -> &DockViewportIdentity {
        &self.identity
    }
}

/// Result of checking whether a previously rendered host scene stayed alive.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum DockViewportHostSceneExpiration {
    /// A newer render was leased after this token; keep the scene.
    StillCurrent,
    /// The token belongs to a viewport binding that is no longer registered.
    StaleIdentity(DockViewportIdentity),
    /// The token is still the most recent render for a live viewport and its scene was removed.
    Expired(DockViewportIdentity),
}

#[derive(Debug, Default)]
struct DockViewportHostSceneLiveness {
    generations: HashMap<DockViewportIdentity, u64>,
}

impl DockViewportHostSceneLiveness {
    fn lease(&mut self, identity: DockViewportIdentity) -> DockViewportHostSceneLivenessToken {
        let generation = self
            .generations
            .entry(identity.clone())
            .and_modify(|generation| *generation = generation.wrapping_add(1))
            .or_insert(1);
        DockViewportHostSceneLivenessToken {
            identity,
            generation: *generation,
        }
    }

    fn is_current(&self, token: &DockViewportHostSceneLivenessToken) -> bool {
        self.generations.get(&token.identity).copied() == Some(token.generation)
    }

    fn forget(&mut self, identity: &DockViewportIdentity) {
        self.generations.remove(identity);
    }

    fn forget_window(&mut self, window_id: WindowId) {
        self.generations
            .retain(|identity, _| identity.window_id() != window_id);
    }
}

/// Coordinates per-frame host-scene facts with delayed liveness expiry.
///
/// ImGui keeps viewport frame activity in one per-frame pass. GPUI render callbacks arrive through
/// separate probes, so this module centralizes the equivalent host-scene generation and liveness
/// bookkeeping instead of spreading stale-scene decisions across the runtime.
#[derive(Debug, Default)]
pub(crate) struct DockViewportFrameCoordinator {
    host_scenes: DockViewportHostSceneRegistry,
    liveness: DockViewportHostSceneLiveness,
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

    pub(crate) fn lease_rendered_host_scene(
        &mut self,
        identity: DockViewportIdentity,
    ) -> DockViewportHostSceneLivenessToken {
        self.liveness.lease(identity)
    }

    pub(crate) fn expire_unrendered_host_scene(
        &mut self,
        token: DockViewportHostSceneLivenessToken,
        current_window_id: Option<WindowId>,
    ) -> DockViewportHostSceneExpiration {
        if !self.liveness.is_current(&token) {
            return DockViewportHostSceneExpiration::StillCurrent;
        }
        let identity = token.identity;
        if current_window_id.is_none_or(|window_id| window_id != identity.window_id()) {
            self.liveness.forget(&identity);
            return DockViewportHostSceneExpiration::StaleIdentity(identity);
        }
        self.host_scenes.unregister_window(identity.window_id());
        DockViewportHostSceneExpiration::Expired(identity)
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

    pub(crate) fn forget_window_liveness(&mut self, window_id: WindowId) {
        self.liveness.forget_window(window_id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::viewport_test_support::{bounds, space};
    use open_gpui::{WindowBounds, WindowId, point, px};

    #[test]
    fn newer_render_token_prevents_scene_expiry() {
        let mut coordinator = DockViewportFrameCoordinator::default();
        let space = space("main");
        let window_id = WindowId::from(1);
        let identity = DockViewportIdentity::new(space.clone(), window_id);
        let old_token = coordinator.lease_rendered_host_scene(identity.clone());
        let _new_token = coordinator.lease_rendered_host_scene(identity);

        assert_eq!(
            coordinator.expire_unrendered_host_scene(old_token, Some(window_id)),
            DockViewportHostSceneExpiration::StillCurrent
        );
    }

    #[test]
    fn stale_identity_token_only_forgets_liveness() {
        let mut coordinator = DockViewportFrameCoordinator::default();
        let space = space("main");
        let window_id = WindowId::from(1);
        let token = coordinator
            .lease_rendered_host_scene(DockViewportIdentity::new(space.clone(), window_id));

        assert_eq!(
            coordinator.expire_unrendered_host_scene(token, None),
            DockViewportHostSceneExpiration::StaleIdentity(DockViewportIdentity::new(
                space, window_id
            ))
        );
    }

    #[test]
    fn current_unrendered_token_removes_host_scene() {
        let mut coordinator = DockViewportFrameCoordinator::default();
        let space = space("main");
        let window_id = WindowId::from(1);
        let host_bounds = bounds(0.0, 0.0, 300.0, 200.0);
        coordinator.register_host_scene(
            space.clone(),
            window_id,
            DockViewportWindowFacts::from_window_bounds(WindowBounds::Windowed(bounds(
                100.0, 100.0, 300.0, 200.0,
            ))),
            host_bounds,
            point(px(10.0), px(10.0)),
            DockDropGuideStyle::default(),
        );
        let token = coordinator
            .lease_rendered_host_scene(DockViewportIdentity::new(space.clone(), window_id));

        assert!(coordinator.screen_position(&space).is_some());
        assert_eq!(
            coordinator.expire_unrendered_host_scene(token, Some(window_id)),
            DockViewportHostSceneExpiration::Expired(DockViewportIdentity::new(
                space.clone(),
                window_id
            ))
        );
        assert_eq!(coordinator.screen_position(&space), None);
    }
}
