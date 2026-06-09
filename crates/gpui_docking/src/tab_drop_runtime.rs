use crate::{
    DockNodeId, DockPolicy, DropZone,
    drop_target::{self, DockDropIntent, DockDropResolution, DockResolvedDropTarget},
};
use open_gpui::{Bounds, Pixels, Point};

#[derive(Debug, Default)]
pub(crate) struct DockTabDropRuntime {
    target: Option<DockResolvedDropTarget>,
}

impl DockTabDropRuntime {
    pub(crate) fn update_tabs_drop_intent(
        &mut self,
        target_tabs: DockNodeId,
        bounds: Bounds<Pixels>,
        position: Point<Pixels>,
        is_central: bool,
        policy: &DockPolicy,
    ) -> bool {
        let mut target = drop_target::resolve_tabs_drop_with_central(
            target_tabs,
            bounds,
            position,
            is_central,
            policy,
        )
        .and_then(DockDropResolution::target);
        if let Some(existing) = self.target.as_ref()
            && let Some(existing_intent) = existing.intent()
            && existing_intent.target_tabs == target_tabs
            && existing_intent.insert_index.is_some()
            && existing_intent.preview_bounds.contains(&position)
            && target
                .as_ref()
                .and_then(DockResolvedDropTarget::intent)
                .is_some_and(|intent| {
                    intent.target_tabs == target_tabs && intent.zone == DropZone::Center
                })
        {
            target = Some(existing.clone());
        }
        self.replace_target(target)
    }

    pub(crate) fn update_tab_reorder_drop_intent(
        &mut self,
        target_tabs: DockNodeId,
        target_index: usize,
        bounds: Bounds<Pixels>,
        position: Point<Pixels>,
        is_central: bool,
        policy: &DockPolicy,
    ) -> bool {
        let Some(resolution) = drop_target::resolve_tab_reorder_drop_with_central(
            target_tabs,
            target_index,
            bounds,
            position,
            is_central,
            policy,
        ) else {
            return false;
        };
        self.replace_target(resolution.target())
    }

    pub(crate) fn take_resolved_target(
        &mut self,
        target_tabs: DockNodeId,
    ) -> Option<DockResolvedDropTarget> {
        let target = self.target.take()?;
        match target.intent() {
            Some(intent) if intent.target_tabs == target_tabs => Some(target),
            _ => None,
        }
    }

    pub(crate) fn preview_bounds(&self, target_tabs: DockNodeId) -> Option<Bounds<Pixels>> {
        let intent = self
            .target
            .as_ref()
            .and_then(DockResolvedDropTarget::intent)?;
        if intent.target_tabs != target_tabs {
            return None;
        }
        if intent.insert_index.is_some() {
            return None;
        }
        Some(intent.preview_bounds)
    }

    fn replace_target(&mut self, target: Option<DockResolvedDropTarget>) -> bool {
        if self.target == target {
            return false;
        }
        self.target = target;
        true
    }

    #[cfg(test)]
    pub(crate) fn intent(&self) -> Option<DockDropIntent> {
        self.target
            .as_ref()
            .and_then(DockResolvedDropTarget::intent)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use open_gpui::{point, px, size};
    use slotmap::Key;

    fn bounds(x: f32, y: f32, width: f32, height: f32) -> Bounds<Pixels> {
        Bounds::new(point(px(x), px(y)), size(px(width), px(height)))
    }

    #[test]
    fn tab_reorder_drop_updates_intent_only_inside_tab_bounds() {
        let tabs = DockNodeId::null();
        let mut runtime = DockTabDropRuntime::default();

        assert!(runtime.update_tab_reorder_drop_intent(
            tabs,
            2,
            bounds(10.0, 20.0, 100.0, 24.0),
            point(px(95.0), px(28.0)),
            false,
            &DockPolicy::default(),
        ));
        assert_eq!(
            runtime.intent().map(|intent| intent.insert_index),
            Some(Some(3))
        );

        assert!(!runtime.update_tab_reorder_drop_intent(
            tabs,
            1,
            bounds(10.0, 20.0, 100.0, 24.0),
            point(px(200.0), px(28.0)),
            false,
            &DockPolicy::default(),
        ));
        assert_eq!(
            runtime.intent().map(|intent| intent.insert_index),
            Some(Some(3))
        );
    }

    #[test]
    fn tabs_drop_preserves_reorder_intent_while_pointer_stays_inside_tab() {
        let tabs = DockNodeId::null();
        let mut runtime = DockTabDropRuntime::default();

        assert!(runtime.update_tab_reorder_drop_intent(
            tabs,
            2,
            bounds(10.0, 100.0, 100.0, 24.0),
            point(px(95.0), px(108.0)),
            false,
            &DockPolicy::default(),
        ));
        assert!(!runtime.update_tabs_drop_intent(
            tabs,
            bounds(0.0, 0.0, 400.0, 400.0),
            point(px(95.0), px(108.0)),
            false,
            &DockPolicy::default(),
        ));

        let target = runtime
            .take_resolved_target(tabs)
            .expect("reorder intent should remain available");
        let intent = target.intent().expect("tab drop target should project");
        assert_eq!(intent.zone, DropZone::Center);
        assert_eq!(intent.insert_index, Some(3));
        assert!(runtime.intent().is_none());
    }

    #[test]
    fn reorder_intent_does_not_render_drop_preview() {
        let tabs = DockNodeId::null();
        let mut runtime = DockTabDropRuntime::default();

        assert!(runtime.update_tab_reorder_drop_intent(
            tabs,
            0,
            bounds(10.0, 20.0, 100.0, 24.0),
            point(px(20.0), px(28.0)),
            false,
            &DockPolicy::default(),
        ));

        assert_eq!(runtime.preview_bounds(tabs), None);
    }
}
