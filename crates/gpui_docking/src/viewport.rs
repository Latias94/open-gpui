#[cfg(test)]
use crate::viewport_registry::DockViewportRouteUnavailableReason;
#[cfg(test)]
use crate::viewport_target_resolver::choose_diagnostic_viewport_target;
use crate::{
    DockSpaceId,
    viewport_registry::{
        DockViewportPlatformRequests, DockViewportPointerRouting, DockViewportRegistry,
    },
};
use open_gpui::{AnyWindowHandle, WindowId};

/// Runtime adapter state that maps logical dock spaces to GPUI windows.
///
/// This type owns platform-window facts for docking: window handles, display ids, and the latest
/// bounds snapshots used for coordinate conversion. None of this state belongs in
/// [`DockGraph`](crate::DockGraph) or [`DockLayout`](crate::DockLayout).
///
/// A typical restore flow imports [`DockLayout`](crate::DockLayout) into a controller, opens or
/// reuses GPUI windows for each logical dock space, registers those windows here, and lets render
/// frames refresh current platform facts for coordinate conversion.
#[derive(Debug, Default)]
pub(crate) struct DockViewportAdapter {
    pub(crate) registry: DockViewportRegistry,
}

impl DockViewportAdapter {
    /// Creates an empty viewport adapter.
    pub(crate) fn new() -> Self {
        Self::default()
    }

    /// Removes a viewport by logical dock space.
    pub(crate) fn unregister_space(
        &mut self,
        space: &DockSpaceId,
    ) -> Option<crate::DockViewportSnapshot> {
        self.registry.unregister_space(space)
    }

    /// Returns the snapshot for a logical dock space.
    pub(crate) fn snapshot(&self, space: &DockSpaceId) -> Option<&crate::DockViewportSnapshot> {
        self.registry.snapshot(space)
    }

    pub(crate) fn snapshot_mut(
        &mut self,
        space: &DockSpaceId,
    ) -> Option<&mut crate::DockViewportSnapshot> {
        self.registry.snapshot_mut(space)
    }

    #[cfg(test)]
    pub(crate) fn route_ready(&self, space: &DockSpaceId) -> bool {
        self.snapshot(space)
            .is_some_and(|snapshot| snapshot.is_route_ready())
    }

    #[cfg(test)]
    pub(crate) fn route_unavailable_reason(
        &self,
        space: &DockSpaceId,
    ) -> Option<DockViewportRouteUnavailableReason> {
        self.snapshot(space)?.route_unavailable_reason()
    }

    pub(crate) fn window_route_ready(&self, window_id: WindowId) -> Option<bool> {
        let space = self.space_for_window_id(window_id)?;
        self.snapshot(space)
            .map(|snapshot| snapshot.is_route_ready())
    }

    pub(crate) fn space_is_no_input_pass_through(&self, space: &DockSpaceId) -> bool {
        self.snapshot(space).is_some_and(|snapshot| {
            snapshot.pointer_routing == DockViewportPointerRouting::NoInputPassThrough
        })
    }

    pub(crate) fn window_close_requested(&self, window_id: WindowId) -> bool {
        let Some(space) = self.space_for_window_id(window_id) else {
            return false;
        };
        self.snapshot(space)
            .is_some_and(|snapshot| snapshot.is_platform_close_requested())
    }

    pub(crate) fn platform_requests_for_space(
        &self,
        space: &DockSpaceId,
    ) -> DockViewportPlatformRequests {
        self.snapshot(space)
            .map(|snapshot| snapshot.platform_requests())
            .unwrap_or_default()
    }

    pub(crate) fn is_live_window_for_space(
        &self,
        space: &DockSpaceId,
        window_id: WindowId,
    ) -> bool {
        self.window_for_space(space)
            .is_some_and(|window| window.window_id() == window_id)
            && !self.window_close_requested(window_id)
    }

    pub(crate) fn unregister_window_id_snapshot(
        &mut self,
        window_id: WindowId,
    ) -> Option<(DockSpaceId, crate::DockViewportSnapshot)> {
        self.registry.unregister_window_id(window_id)
    }

    /// Returns the window rendering a logical dock space.
    pub(crate) fn window_for_space(&self, space: &DockSpaceId) -> Option<AnyWindowHandle> {
        self.registry.window_for_space(space)
    }

    pub(crate) fn record_platform_focus_order_window(&mut self, window_id: WindowId) -> bool {
        self.registry.record_platform_focus_order_window(window_id)
    }

    /// Returns the logical dock space rendered by a window id.
    pub(crate) fn space_for_window_id(&self, window_id: WindowId) -> Option<&DockSpaceId> {
        self.registry.space_for_window_id(window_id)
    }

    /// Returns known dock spaces in stable lexical order.
    pub(crate) fn spaces(&self) -> Vec<DockSpaceId> {
        self.registry.spaces()
    }

    pub(crate) fn viewport_lifecycle_records(&self) -> Vec<crate::DockViewportLifecycleRecord> {
        self.registry
            .snapshots()
            .map(|(space, snapshot)| {
                crate::DockViewportLifecycleRecord::from_snapshot(space.clone(), snapshot)
            })
            .collect()
    }

    #[cfg(test)]
    pub(crate) fn insert_stale_window_index_for_test(
        &mut self,
        window_id: WindowId,
        space: DockSpaceId,
    ) {
        self.registry
            .insert_stale_window_index_for_test(window_id, space);
    }

    #[cfg(test)]
    pub(crate) fn spaces_by_platform_focus_order(&self) -> Vec<DockSpaceId> {
        self.registry.spaces_by_platform_focus_order()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        DockGraph, DockItemId, DockNode, DockViewportTargetContext, DockViewportUnregisterOutcome,
        DockViewportUnregisterReason, DockViewportWindowFacts,
        viewport_test_support::{bounds, handle, register_viewport, space},
    };
    use open_gpui::{DisplayId, WindowBounds, point, px};

    #[test]
    fn registering_viewports_records_and_replaces_window_mappings() {
        let mut adapter = DockViewportAdapter::new();
        let main = space("main");
        let secondary = space("secondary");
        let first = handle(1);
        let second = handle(2);

        assert!(
            adapter
                .register_viewport_with_outcome(main.clone(), first)
                .replaced()
                .is_empty()
        );
        assert_eq!(adapter.window_for_space(&main), Some(first));
        assert_eq!(adapter.space_for_window_id(first.window_id()), Some(&main));

        let previous = adapter.register_viewport_with_outcome(main.clone(), second);
        assert_eq!(
            previous.replaced(),
            &[DockViewportUnregisterOutcome {
                space: main.clone(),
                window: first,
                reason: DockViewportUnregisterReason::Replaced,
            }]
        );
        assert_eq!(adapter.window_for_space(&main), Some(second));
        assert_eq!(adapter.space_for_window_id(first.window_id()), None);

        register_viewport(&mut adapter, secondary.clone(), second);
        assert_eq!(adapter.window_for_space(&main), None);
        assert_eq!(adapter.window_for_space(&secondary), Some(second));
        assert_eq!(adapter.spaces(), vec![secondary]);
    }

    #[test]
    fn unregistering_by_space_or_window_clears_both_indexes() {
        let mut adapter = DockViewportAdapter::new();
        let main = space("main");
        let secondary = space("secondary");
        let first = handle(1);
        let second = handle(2);

        register_viewport(&mut adapter, main.clone(), first);
        register_viewport(&mut adapter, secondary.clone(), second);

        let removed = adapter
            .unregister_space(&main)
            .expect("space should be registered");
        assert_eq!(removed.window, first);
        assert_eq!(adapter.space_for_window_id(first.window_id()), None);
        assert_eq!(adapter.window_for_space(&main), None);

        let removed = adapter
            .unregister_window_id(second.window_id(), DockViewportUnregisterReason::Closed)
            .expect("window should be registered");
        assert_eq!(removed.space, secondary);
        assert_eq!(removed.window, second);
        assert!(adapter.spaces().is_empty());
    }

    #[test]
    fn registering_with_outcome_reports_replaced_window_mapping() {
        let mut adapter = DockViewportAdapter::new();
        let main = space("main");
        let secondary = space("secondary");
        let first = handle(1);

        register_viewport(&mut adapter, main.clone(), first);
        let outcome = adapter.register_viewport_with_outcome(secondary.clone(), first);

        assert_eq!(outcome.space(), &secondary);
        assert_eq!(outcome.window(), first);
        assert_eq!(
            outcome.replaced(),
            &[DockViewportUnregisterOutcome {
                space: main.clone(),
                window: first,
                reason: DockViewportUnregisterReason::Replaced,
            }]
        );
        assert_eq!(adapter.window_for_space(&main), None);
        assert_eq!(adapter.window_for_space(&secondary), Some(first));
        assert_eq!(
            adapter.space_for_window_id(first.window_id()),
            Some(&secondary)
        );
    }

    #[test]
    fn registering_with_outcome_reports_all_replaced_mappings() {
        let mut adapter = DockViewportAdapter::new();
        let main = space("main");
        let secondary = space("secondary");
        let first = handle(1);
        let second = handle(2);

        register_viewport(&mut adapter, main.clone(), first);
        register_viewport(&mut adapter, secondary.clone(), second);
        let outcome = adapter.register_viewport_with_outcome(main.clone(), second);

        assert_eq!(
            outcome.replaced(),
            &[
                DockViewportUnregisterOutcome {
                    space: main.clone(),
                    window: first,
                    reason: DockViewportUnregisterReason::Replaced,
                },
                DockViewportUnregisterOutcome {
                    space: secondary.clone(),
                    window: second,
                    reason: DockViewportUnregisterReason::Replaced,
                },
            ]
        );
        assert_eq!(adapter.window_for_space(&main), Some(second));
        assert_eq!(adapter.window_for_space(&secondary), None);
        assert_eq!(adapter.space_for_window_id(first.window_id()), None);
        assert_eq!(adapter.space_for_window_id(second.window_id()), Some(&main));
    }

    #[test]
    fn registering_same_viewport_preserves_runtime_snapshot() {
        let mut adapter = DockViewportAdapter::new();
        let main = space("main");
        let window = handle(1);
        let display_id = Some(DisplayId::new(7));
        let window_bounds = WindowBounds::Windowed(bounds(100.0, 200.0, 800.0, 600.0));
        let host_bounds = bounds(10.0, 20.0, 300.0, 200.0);

        register_viewport(&mut adapter, main.clone(), window);
        adapter.update_snapshot(
            &main,
            DockViewportWindowFacts::from_window_bounds(window_bounds).with_display_id(display_id),
            host_bounds,
        );

        let outcome = adapter.register_viewport_with_outcome(main.clone(), window);
        assert!(outcome.replaced().is_empty());
        assert_eq!(outcome.space(), &main);
        assert_eq!(outcome.window(), window);

        let snapshot = adapter
            .snapshot(&main)
            .expect("idempotent registration should preserve the snapshot");
        assert_eq!(snapshot.window, window);
        assert_eq!(snapshot.display_id, display_id);
        assert_eq!(snapshot.window_bounds, Some(window_bounds));
        assert_eq!(snapshot.host_bounds, Some(host_bounds));
        assert!(snapshot.is_route_ready());
    }

    #[test]
    fn viewport_target_empty_context_uses_stable_space_order_despite_platform_focus_order() {
        let mut adapter = DockViewportAdapter::new();
        let alpha = space("alpha");
        let zeta = space("zeta");
        let alpha_window = handle(1);
        let zeta_window = handle(2);

        register_viewport(&mut adapter, alpha.clone(), alpha_window);
        register_viewport(&mut adapter, zeta.clone(), zeta_window);
        for space in [&alpha, &zeta] {
            adapter.update_snapshot(
                space,
                DockViewportWindowFacts::from_window_bounds(WindowBounds::Windowed(bounds(
                    100.0, 100.0, 320.0, 240.0,
                ))),
                bounds(0.0, 0.0, 320.0, 240.0),
            );
        }
        adapter.record_platform_focus_order_window(alpha_window.window_id());
        adapter.record_platform_focus_order_window(zeta_window.window_id());
        let hits = adapter.global_screen_viewport_hits(point(px(120.0), px(140.0)));

        assert_eq!(
            choose_diagnostic_viewport_target(hits, &DockViewportTargetContext::new())
                .map(|target| target.space().clone()),
            Some(alpha.clone()),
            "default viewport hit testing must not infer target priority from platform focus order"
        );
        assert_eq!(
            adapter.spaces_by_platform_focus_order(),
            vec![zeta, alpha],
            "platform focus order remains available only through the explicit diagnostic ordering interface"
        );
    }

    #[test]
    fn live_window_binding_rejects_replaced_and_closing_windows() {
        let mut adapter = DockViewportAdapter::new();
        let main = space("main");
        let first = handle(1);
        let second = handle(2);

        register_viewport(&mut adapter, main.clone(), first);
        assert!(adapter.is_live_window_for_space(&main, first.window_id()));
        assert!(!adapter.is_live_window_for_space(&main, second.window_id()));

        register_viewport(&mut adapter, main.clone(), second);
        assert!(!adapter.is_live_window_for_space(&main, first.window_id()));
        assert!(adapter.is_live_window_for_space(&main, second.window_id()));

        adapter.mark_window_close_requested(second.window_id());
        assert!(!adapter.is_live_window_for_space(&main, second.window_id()));
    }

    #[test]
    fn dock_layout_import_does_not_require_viewport_placement() {
        let mut graph = DockGraph::new();
        let main = space("main");
        let tabs = graph.insert_node(DockNode::Tabs {
            items: vec![DockItemId::from("a")],
            selected: Some(DockItemId::from("a")),
        });
        graph.set_root(main.clone(), tabs);

        let mut adapter = DockViewportAdapter::new();
        register_viewport(&mut adapter, main.clone(), handle(1));
        adapter.update_snapshot(
            &main,
            DockViewportWindowFacts::from_window_bounds(WindowBounds::Windowed(bounds(
                100.0, 200.0, 800.0, 600.0,
            )))
            .with_display_id(Some(DisplayId::new(7))),
            bounds(10.0, 20.0, 300.0, 200.0),
        );

        let placement_json = serde_json::to_string(&adapter.export_placement())
            .expect("placement should serialize independently");
        let dock_layout = graph.export_layout();
        let layout_json =
            serde_json::to_string(&dock_layout).expect("dock layout should serialize");

        assert!(placement_json.contains("placement_version"));
        assert!(!layout_json.contains("placement_version"));
        assert!(!layout_json.contains("window_bounds"));
        let imported = DockGraph::import_layout(&dock_layout).expect("dock layout should import");
        assert!(imported.root(&main).is_some());
    }

    #[test]
    fn adapter_state_stays_out_of_layout_export() {
        let mut graph = DockGraph::new();
        let tabs = graph.insert_node(DockNode::Tabs {
            items: vec![DockItemId::from("a")],
            selected: Some(DockItemId::from("a")),
        });
        graph.set_root(space("main"), tabs);

        let mut adapter = DockViewportAdapter::new();
        let main = space("main");
        register_viewport(&mut adapter, main.clone(), handle(42));
        adapter.update_snapshot(
            &main,
            DockViewportWindowFacts::from_window_bounds(WindowBounds::Windowed(bounds(
                100.0, 200.0, 800.0, 600.0,
            )))
            .with_display_id(Some(DisplayId::new(7))),
            bounds(10.0, 20.0, 300.0, 200.0),
        );

        let json = serde_json::to_string(&graph.export_layout()).expect("layout should serialize");
        assert!(!json.contains("WindowHandle"));
        assert!(!json.contains("WindowId"));
        assert!(!json.contains("DisplayId"));
        assert!(!json.contains("AnyWindowHandle"));
    }
}
