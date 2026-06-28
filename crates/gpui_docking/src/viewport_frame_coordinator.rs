use crate::{
    DockNodeId, DockSpaceId, DockViewportIdentity, DockViewportWindowFacts,
    drop_runtime::DockHostDropSceneFact,
    geometry::DockDropGuideStyle,
    viewport_drop_scene::{
        DockViewportHostSceneFrame, DockViewportHostSceneRegistration,
        DockViewportHostSceneRegistry, DockViewportHostSceneSnapshot,
    },
};
use open_gpui::{Bounds, Pixels, Point, WindowId};
use std::collections::HashMap;

/// Token proving that a viewport host scene was rendered for a specific runtime binding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DockViewportHostSceneRenderToken {
    identity: DockViewportIdentity,
    render_epoch: u64,
}

impl DockViewportHostSceneRenderToken {
    pub(crate) fn identity(&self) -> &DockViewportIdentity {
        &self.identity
    }
}

/// Result of checking whether a previously rendered host scene was rendered again.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum DockViewportHostSceneRenderExpiration {
    /// A newer render was marked after this token; keep the scene.
    StillCurrent,
    /// The token belongs to a viewport binding that is no longer registered.
    StaleIdentity(DockViewportIdentity),
    /// The token is still the most recent render for a live viewport and its scene was removed.
    Expired(DockViewportIdentity),
}

#[derive(Debug, Default)]
struct DockViewportHostSceneRenderEpochs {
    epochs: HashMap<DockViewportIdentity, u64>,
}

impl DockViewportHostSceneRenderEpochs {
    fn mark_rendered(
        &mut self,
        identity: DockViewportIdentity,
    ) -> DockViewportHostSceneRenderToken {
        let render_epoch = self
            .epochs
            .entry(identity.clone())
            .and_modify(|render_epoch| *render_epoch = render_epoch.wrapping_add(1))
            .or_insert(1);
        DockViewportHostSceneRenderToken {
            identity,
            render_epoch: *render_epoch,
        }
    }

    fn is_current(&self, token: &DockViewportHostSceneRenderToken) -> bool {
        self.epochs.get(&token.identity).copied() == Some(token.render_epoch)
    }

    fn forget(&mut self, identity: &DockViewportIdentity) {
        self.epochs.remove(identity);
    }

    fn forget_window(&mut self, window_id: WindowId) {
        self.epochs
            .retain(|identity, _| identity.window_id() != window_id);
    }
}

/// Coordinates per-frame host-scene facts with delayed render-token expiry.
///
/// ImGui keeps viewport frame activity in one per-frame pass. GPUI render callbacks arrive through
/// separate probes, so this module centralizes host-scene content and render-epoch bookkeeping
/// instead of spreading stale-scene decisions across the runtime.
#[derive(Debug, Default)]
pub(crate) struct DockViewportFrameCoordinator {
    host_scenes: DockViewportHostSceneRegistry,
    render_epochs: DockViewportHostSceneRenderEpochs,
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

    pub(crate) fn mark_host_scene_rendered(
        &mut self,
        identity: DockViewportIdentity,
    ) -> DockViewportHostSceneRenderToken {
        self.render_epochs.mark_rendered(identity)
    }

    pub(crate) fn expire_host_scene_if_not_rendered_after(
        &mut self,
        token: DockViewportHostSceneRenderToken,
        current_window_id: Option<WindowId>,
    ) -> DockViewportHostSceneRenderExpiration {
        if !self.render_epochs.is_current(&token) {
            return DockViewportHostSceneRenderExpiration::StillCurrent;
        }
        let identity = token.identity;
        if current_window_id.is_none_or(|window_id| window_id != identity.window_id()) {
            self.render_epochs.forget(&identity);
            return DockViewportHostSceneRenderExpiration::StaleIdentity(identity);
        }
        self.host_scenes.unregister_window(identity.window_id());
        DockViewportHostSceneRenderExpiration::Expired(identity)
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

    pub(crate) fn forget_window_render_epochs(&mut self, window_id: WindowId) {
        self.render_epochs.forget_window(window_id);
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
        let old_token = coordinator.mark_host_scene_rendered(identity.clone());
        let _new_token = coordinator.mark_host_scene_rendered(identity);

        assert_eq!(
            coordinator.expire_host_scene_if_not_rendered_after(old_token, Some(window_id)),
            DockViewportHostSceneRenderExpiration::StillCurrent
        );
    }

    #[test]
    fn stale_identity_token_only_forgets_render_epoch() {
        let mut coordinator = DockViewportFrameCoordinator::default();
        let space = space("main");
        let window_id = WindowId::from(1);
        let token = coordinator
            .mark_host_scene_rendered(DockViewportIdentity::new(space.clone(), window_id));

        assert_eq!(
            coordinator.expire_host_scene_if_not_rendered_after(token, None),
            DockViewportHostSceneRenderExpiration::StaleIdentity(DockViewportIdentity::new(
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
            .mark_host_scene_rendered(DockViewportIdentity::new(space.clone(), window_id));

        assert!(coordinator.screen_position(&space).is_some());
        assert_eq!(
            coordinator.expire_host_scene_if_not_rendered_after(token, Some(window_id)),
            DockViewportHostSceneRenderExpiration::Expired(DockViewportIdentity::new(
                space.clone(),
                window_id
            ))
        );
        assert_eq!(coordinator.screen_position(&space), None);
    }
}
