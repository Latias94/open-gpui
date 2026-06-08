use crate::{
    DockNodeId, DockPolicy, DropZone,
    drop_target::{self, DockDropIntent, DockDropResolution},
};
use open_gpui::{Bounds, Pixels, Point};

#[derive(Debug, Default)]
pub(crate) struct DockTabDropRuntime {
    intent: Option<DockDropIntent>,
}

impl DockTabDropRuntime {
    pub(crate) fn update_tabs_drop_intent(
        &mut self,
        target_tabs: DockNodeId,
        bounds: Bounds<Pixels>,
        position: Point<Pixels>,
        policy: &DockPolicy,
    ) -> bool {
        let mut intent = drop_target::resolve_tabs_drop(target_tabs, bounds, position, policy)
            .and_then(DockDropResolution::intent);
        if let Some(existing) = self.intent
            && existing.target_tabs == target_tabs
            && existing.insert_index.is_some()
            && existing.preview_bounds.contains(&position)
            && intent.as_ref().is_some_and(|intent| {
                intent.target_tabs == target_tabs && intent.zone == DropZone::Center
            })
        {
            intent = Some(existing);
        }
        self.replace_intent(intent)
    }

    pub(crate) fn update_tab_reorder_drop_intent(
        &mut self,
        target_tabs: DockNodeId,
        target_index: usize,
        bounds: Bounds<Pixels>,
        position: Point<Pixels>,
        policy: &DockPolicy,
    ) -> bool {
        let Some(resolution) = drop_target::resolve_tab_reorder_drop(
            target_tabs,
            target_index,
            bounds,
            position,
            policy,
        ) else {
            return false;
        };
        self.replace_intent(resolution.intent())
    }

    pub(crate) fn take_intent(&mut self, target_tabs: DockNodeId) -> Option<DockDropIntent> {
        let intent = self
            .intent
            .filter(|intent| intent.target_tabs == target_tabs);
        self.intent = None;
        intent
    }

    pub(crate) fn preview_bounds(&self, target_tabs: DockNodeId) -> Option<Bounds<Pixels>> {
        let intent = self
            .intent
            .filter(|intent| intent.target_tabs == target_tabs)?;
        if intent.insert_index.is_some() {
            return None;
        }
        Some(intent.preview_bounds)
    }

    fn replace_intent(&mut self, intent: Option<DockDropIntent>) -> bool {
        if self.intent == intent {
            return false;
        }
        self.intent = intent;
        true
    }

    #[cfg(test)]
    pub(crate) fn intent(&self) -> Option<DockDropIntent> {
        self.intent
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
            &DockPolicy::default(),
        ));
        assert!(!runtime.update_tabs_drop_intent(
            tabs,
            bounds(0.0, 0.0, 400.0, 400.0),
            point(px(95.0), px(108.0)),
            &DockPolicy::default(),
        ));

        let intent = runtime
            .take_intent(tabs)
            .expect("reorder intent should remain available");
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
            &DockPolicy::default(),
        ));

        assert_eq!(runtime.preview_bounds(tabs), None);
    }
}
