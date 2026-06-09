use crate::{
    DockAction, DockNodeId, DockPolicy, DockSpaceId, drop_target::DockResolvedDropTarget, geometry,
    tab_drop_runtime::DockTabDropRuntime,
};
use open_gpui::{Bounds, Pixels, Point, point};

#[derive(Debug, Default)]
pub(crate) struct DockInteractionRuntime {
    splitter_drag: Option<SplitterDrag>,
    floating_drag: Option<FloatingDrag>,
    tab_drop: DockTabDropRuntime,
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
        let fractions = geometry::resize_adjacent_split_fractions(
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
        is_central: bool,
        policy: &DockPolicy,
    ) -> bool {
        self.tab_drop
            .update_tabs_drop_intent(target_tabs, bounds, position, is_central, policy)
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
        self.tab_drop.update_tab_reorder_drop_intent(
            target_tabs,
            target_index,
            bounds,
            position,
            is_central,
            policy,
        )
    }

    pub(crate) fn take_tab_drop_target(
        &mut self,
        target_tabs: DockNodeId,
    ) -> Option<DockResolvedDropTarget> {
        self.tab_drop.take_resolved_target(target_tabs)
    }

    pub(crate) fn tab_drop_preview_bounds(
        &self,
        target_tabs: DockNodeId,
    ) -> Option<Bounds<Pixels>> {
        self.tab_drop.preview_bounds(target_tabs)
    }

    #[cfg(test)]
    pub(crate) fn splitter_drag(&self) -> Option<&SplitterDrag> {
        self.splitter_drag.as_ref()
    }

    #[cfg(test)]
    pub(crate) fn floating_drag(&self) -> Option<&FloatingDrag> {
        self.floating_drag.as_ref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DockNodeId;
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
}
