use crate::{
    DockAction, DockNodeId, DockPolicy, DockSpaceId, DropZone,
    drop_target::{self, DockDropIntent, DockDropResolution},
    splitter,
};
use open_gpui::{Bounds, Pixels, Point, point};

#[derive(Debug, Default)]
pub(crate) struct DockInteractionRuntime {
    splitter_drag: Option<SplitterDrag>,
    floating_drag: Option<FloatingDrag>,
    tab_drop_intent: Option<DockDropIntent>,
}

#[derive(Debug, Clone)]
pub(crate) struct SplitterDrag {
    pub(crate) split: DockNodeId,
    pub(crate) handle_index: usize,
    pub(crate) start_position: Pixels,
    pub(crate) split_extent: Pixels,
    pub(crate) initial_fractions: Vec<f32>,
}

#[derive(Debug, Clone)]
pub(crate) struct FloatingDrag {
    pub(crate) space: DockSpaceId,
    pub(crate) floating: DockNodeId,
    pub(crate) start_position: Point<Pixels>,
    pub(crate) initial_bounds: Bounds<Pixels>,
}

impl DockInteractionRuntime {
    pub(crate) fn start_splitter_drag(
        &mut self,
        split: DockNodeId,
        handle_index: usize,
        start_position: Pixels,
        split_extent: Pixels,
        initial_fractions: Vec<f32>,
    ) {
        self.splitter_drag = Some(SplitterDrag {
            split,
            handle_index,
            start_position,
            split_extent,
            initial_fractions,
        });
    }

    pub(crate) fn resize_split_action(
        &self,
        position: Pixels,
        split_min_size: Pixels,
    ) -> Option<DockAction> {
        let drag = self.splitter_drag.as_ref()?;
        let delta = position - drag.start_position;
        let fractions = splitter::resize_adjacent_fractions(
            &drag.initial_fractions,
            drag.initial_fractions.len(),
            drag.handle_index,
            drag.split_extent,
            delta,
            split_min_size,
        )?;

        Some(DockAction::ResizeSplit {
            split: drag.split,
            fractions,
        })
    }

    pub(crate) fn finish_splitter_drag(&mut self) -> bool {
        self.splitter_drag.take().is_some()
    }

    pub(crate) fn start_floating_drag(
        &mut self,
        space: DockSpaceId,
        floating: DockNodeId,
        start_position: Point<Pixels>,
        initial_bounds: Bounds<Pixels>,
    ) {
        self.floating_drag = Some(FloatingDrag {
            space,
            floating,
            start_position,
            initial_bounds,
        });
    }

    pub(crate) fn set_floating_bounds_action(&self, position: Point<Pixels>) -> Option<DockAction> {
        let drag = self.floating_drag.as_ref()?;
        let delta = position - drag.start_position;
        let bounds = Bounds::new(
            point(
                drag.initial_bounds.origin.x + delta.x,
                drag.initial_bounds.origin.y + delta.y,
            ),
            drag.initial_bounds.size,
        );

        Some(DockAction::SetFloatingBounds {
            space: drag.space.clone(),
            floating: drag.floating,
            bounds,
        })
    }

    pub(crate) fn finish_floating_drag(&mut self) -> bool {
        self.floating_drag.take().is_some()
    }

    pub(crate) fn update_tabs_drop_intent(
        &mut self,
        target_tabs: DockNodeId,
        bounds: Bounds<Pixels>,
        position: Point<Pixels>,
        policy: &DockPolicy,
    ) -> bool {
        let mut intent = drop_target::resolve_tabs_drop(target_tabs, bounds, position, policy)
            .and_then(DockDropResolution::intent);
        if let Some(existing) = self.tab_drop_intent
            && existing.target_tabs == target_tabs
            && existing.insert_index.is_some()
            && existing.preview_bounds.contains(&position)
            && intent.as_ref().is_some_and(|intent| {
                intent.target_tabs == target_tabs && intent.zone == DropZone::Center
            })
        {
            intent = Some(existing);
        }
        self.replace_tab_drop_intent(intent)
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
        self.replace_tab_drop_intent(resolution.intent())
    }

    pub(crate) fn take_tab_drop_intent(
        &mut self,
        target_tabs: DockNodeId,
    ) -> Option<DockDropIntent> {
        let intent = self
            .tab_drop_intent
            .filter(|intent| intent.target_tabs == target_tabs);
        self.tab_drop_intent = None;
        intent
    }

    pub(crate) fn tab_drop_preview_bounds(
        &self,
        target_tabs: DockNodeId,
    ) -> Option<Bounds<Pixels>> {
        let intent = self
            .tab_drop_intent
            .filter(|intent| intent.target_tabs == target_tabs)?;
        if intent.insert_index.is_some() {
            return None;
        }
        Some(intent.preview_bounds)
    }

    fn replace_tab_drop_intent(&mut self, intent: Option<DockDropIntent>) -> bool {
        if self.tab_drop_intent == intent {
            return false;
        }
        self.tab_drop_intent = intent;
        true
    }

    #[cfg(test)]
    pub(crate) fn splitter_drag(&self) -> Option<&SplitterDrag> {
        self.splitter_drag.as_ref()
    }

    #[cfg(test)]
    pub(crate) fn floating_drag(&self) -> Option<&FloatingDrag> {
        self.floating_drag.as_ref()
    }

    #[cfg(test)]
    pub(crate) fn tab_drop_intent(&self) -> Option<DockDropIntent> {
        self.tab_drop_intent
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{DockNodeId, DropZone};
    use open_gpui::{point, px, size};
    use slotmap::Key;

    fn bounds(x: f32, y: f32, width: f32, height: f32) -> Bounds<Pixels> {
        Bounds::new(point(px(x), px(y)), size(px(width), px(height)))
    }

    #[test]
    fn splitter_update_without_active_drag_has_no_action() {
        let runtime = DockInteractionRuntime::default();

        assert_eq!(runtime.resize_split_action(px(120.0), px(96.0)), None);
    }

    #[test]
    fn splitter_drag_produces_resize_action() {
        let split = DockNodeId::null();
        let mut runtime = DockInteractionRuntime::default();
        runtime.start_splitter_drag(split, 0, px(100.0), px(400.0), vec![0.5, 0.5]);

        assert_eq!(
            runtime.resize_split_action(px(180.0), px(96.0)),
            Some(DockAction::ResizeSplit {
                split,
                fractions: vec![0.7, 0.3],
            })
        );
    }

    #[test]
    fn finishing_splitter_drag_reports_only_active_state_changes() {
        let split = DockNodeId::null();
        let mut runtime = DockInteractionRuntime::default();

        assert!(!runtime.finish_splitter_drag());
        runtime.start_splitter_drag(split, 0, px(100.0), px(400.0), vec![0.5, 0.5]);
        assert!(runtime.finish_splitter_drag());
        assert!(!runtime.finish_splitter_drag());
    }

    #[test]
    fn floating_drag_produces_bounds_action() {
        let floating = DockNodeId::null();
        let mut runtime = DockInteractionRuntime::default();
        runtime.start_floating_drag(
            DockSpaceId::from("main"),
            floating,
            point(px(10.0), px(20.0)),
            bounds(40.0, 50.0, 200.0, 100.0),
        );

        assert_eq!(
            runtime.set_floating_bounds_action(point(px(25.0), px(35.0))),
            Some(DockAction::SetFloatingBounds {
                space: DockSpaceId::from("main"),
                floating,
                bounds: bounds(55.0, 65.0, 200.0, 100.0),
            })
        );
    }

    #[test]
    fn finishing_floating_drag_reports_only_active_state_changes() {
        let floating = DockNodeId::null();
        let mut runtime = DockInteractionRuntime::default();

        assert!(!runtime.finish_floating_drag());
        runtime.start_floating_drag(
            DockSpaceId::from("main"),
            floating,
            point(px(10.0), px(20.0)),
            bounds(40.0, 50.0, 200.0, 100.0),
        );
        assert!(runtime.finish_floating_drag());
        assert!(!runtime.finish_floating_drag());
    }

    #[test]
    fn tab_reorder_drop_updates_intent_only_inside_tab_bounds() {
        let tabs = DockNodeId::null();
        let mut runtime = DockInteractionRuntime::default();

        assert!(runtime.update_tab_reorder_drop_intent(
            tabs,
            2,
            bounds(10.0, 20.0, 100.0, 24.0),
            point(px(95.0), px(28.0)),
            &DockPolicy::default(),
        ));
        assert_eq!(
            runtime.tab_drop_intent().map(|intent| intent.insert_index),
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
            runtime.tab_drop_intent().map(|intent| intent.insert_index),
            Some(Some(3))
        );
    }

    #[test]
    fn tabs_drop_preserves_reorder_intent_while_pointer_stays_inside_tab() {
        let tabs = DockNodeId::null();
        let mut runtime = DockInteractionRuntime::default();

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
            .take_tab_drop_intent(tabs)
            .expect("reorder intent should remain available");
        assert_eq!(intent.zone, DropZone::Center);
        assert_eq!(intent.insert_index, Some(3));
        assert!(runtime.tab_drop_intent().is_none());
    }

    #[test]
    fn reorder_intent_does_not_render_drop_preview() {
        let tabs = DockNodeId::null();
        let mut runtime = DockInteractionRuntime::default();

        assert!(runtime.update_tab_reorder_drop_intent(
            tabs,
            0,
            bounds(10.0, 20.0, 100.0, 24.0),
            point(px(20.0), px(28.0)),
            &DockPolicy::default(),
        ));

        assert_eq!(runtime.tab_drop_preview_bounds(tabs), None);
    }
}
