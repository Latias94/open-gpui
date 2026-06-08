use crate::{
    DockController, DockHost, DockSpaceId,
    viewport_registry::{DockViewportRegistry, DockViewportSnapshot},
};
use open_gpui::{AnyWindowHandle, App, AppContext as _, Entity, Result, WindowId, WindowOptions};

/// Runtime adapter state that maps logical dock spaces to GPUI windows.
///
/// This type owns platform-window facts for docking: window handles, display ids, and the latest
/// bounds snapshots used for coordinate conversion. None of this state belongs in
/// [`DockGraph`](crate::DockGraph) or [`DockLayout`](crate::DockLayout).
///
/// A typical restore flow imports [`DockLayout`](crate::DockLayout) into a controller, opens or
/// reuses GPUI windows for each logical dock space, registers those windows here, then applies a
/// [`DockViewportPlacementLayout`](crate::DockViewportPlacementLayout) to rehydrate placement
/// snapshots for coordinate conversion.
#[derive(Debug, Default)]
pub struct DockViewportAdapter {
    registry: DockViewportRegistry,
}

/// Runtime result of opening or reopening a platform viewport.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DockViewportOpenOutcome {
    /// Logical dock space rendered by the window.
    pub space: DockSpaceId,
    /// GPUI window that renders the logical dock space.
    pub window: AnyWindowHandle,
    /// Whether the runtime opened, reused, or replaced a window.
    pub status: DockViewportOpenStatus,
}

/// How an open or reopen request resolved.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DockViewportOpenStatus {
    /// A new GPUI window was opened and registered.
    Opened,
    /// An existing live GPUI window was reused.
    Reused,
    /// A stale or superseded mapping was replaced by a new window.
    Replaced,
}

impl DockViewportAdapter {
    /// Creates an empty viewport adapter.
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns true when no viewport mappings are registered.
    pub fn is_empty(&self) -> bool {
        self.registry.is_empty()
    }

    /// Returns the number of registered logical viewports.
    pub fn len(&self) -> usize {
        self.registry.len()
    }

    /// Registers or replaces the window for a logical dock space.
    ///
    /// A window can belong to only one dock space at a time. Registering the same window for a
    /// different space removes its previous space mapping.
    pub fn register_viewport(
        &mut self,
        space: impl Into<DockSpaceId>,
        window: impl Into<AnyWindowHandle>,
    ) -> Option<DockViewportSnapshot> {
        let space = space.into();
        let window = window.into();
        self.registry.register(space, window)
    }

    /// Opens or reuses a GPUI window that renders a logical dock space.
    ///
    /// The returned window root is a controller-backed [`DockHost`]. If the dock space already has
    /// a live registered window, that window is activated and reused. If the existing mapping is
    /// stale, it is removed before opening a replacement window.
    pub fn open_viewport(
        &mut self,
        controller: Entity<DockController>,
        space: impl Into<DockSpaceId>,
        options: WindowOptions,
        cx: &mut App,
    ) -> Result<DockViewportOpenOutcome> {
        let space = space.into();
        let mut status = DockViewportOpenStatus::Opened;

        if let Some(window) = self.window_for_space(&space) {
            if window
                .update(cx, |_, window, _| window.activate_window())
                .is_ok()
            {
                return Ok(DockViewportOpenOutcome {
                    space,
                    window,
                    status: DockViewportOpenStatus::Reused,
                });
            }

            self.unregister_space(&space);
            status = DockViewportOpenStatus::Replaced;
        }

        let host_space = space.clone();
        let window = cx
            .open_window(options, move |_, cx| {
                cx.new(move |cx| DockHost::from_controller(controller, host_space, cx))
            })?
            .into();
        self.register_viewport(space.clone(), window);

        Ok(DockViewportOpenOutcome {
            space,
            window,
            status,
        })
    }

    /// Removes a viewport by logical dock space.
    pub fn unregister_space(&mut self, space: &DockSpaceId) -> Option<DockViewportSnapshot> {
        self.registry.unregister_space(space)
    }

    /// Removes a viewport by GPUI window handle.
    pub fn unregister_window(
        &mut self,
        window: impl Into<AnyWindowHandle>,
    ) -> Option<(DockSpaceId, DockViewportSnapshot)> {
        let window = window.into();
        self.registry.unregister_window(window)
    }

    /// Returns the snapshot for a logical dock space.
    pub fn snapshot(&self, space: &DockSpaceId) -> Option<&DockViewportSnapshot> {
        self.registry.snapshot(space)
    }

    pub(crate) fn snapshot_mut(
        &mut self,
        space: &DockSpaceId,
    ) -> Option<&mut DockViewportSnapshot> {
        self.registry.snapshot_mut(space)
    }

    pub(crate) fn unregister_window_id_snapshot(
        &mut self,
        window_id: WindowId,
    ) -> Option<(DockSpaceId, DockViewportSnapshot)> {
        self.registry.unregister_window_id(window_id)
    }

    /// Returns the window rendering a logical dock space.
    pub fn window_for_space(&self, space: &DockSpaceId) -> Option<AnyWindowHandle> {
        self.registry.window_for_space(space)
    }

    /// Returns the logical dock space rendered by a window.
    pub fn space_for_window(&self, window: impl Into<AnyWindowHandle>) -> Option<&DockSpaceId> {
        let window = window.into();
        self.space_for_window_id(window.window_id())
    }

    /// Returns the logical dock space rendered by a window id.
    pub fn space_for_window_id(&self, window_id: WindowId) -> Option<&DockSpaceId> {
        self.registry.space_for_window_id(window_id)
    }

    /// Returns known dock spaces in stable lexical order.
    pub fn spaces(&self) -> Vec<DockSpaceId> {
        self.registry.spaces()
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{DockGraph, DockHost, DockItemId, DockNode};
    use open_gpui::{Bounds, DisplayId, Pixels, WindowBounds, WindowHandle, point, px, size};

    fn space(id: &str) -> DockSpaceId {
        DockSpaceId::from(id)
    }

    fn handle(id: u64) -> AnyWindowHandle {
        WindowHandle::<DockHost>::new(WindowId::from(id)).into()
    }

    fn bounds(x: f32, y: f32, width: f32, height: f32) -> Bounds<Pixels> {
        Bounds::new(point(px(x), px(y)), size(px(width), px(height)))
    }

    #[test]
    fn registering_viewports_records_and_replaces_window_mappings() {
        let mut adapter = DockViewportAdapter::new();
        let main = space("main");
        let secondary = space("secondary");
        let first = handle(1);
        let second = handle(2);

        assert!(adapter.register_viewport(main.clone(), first).is_none());
        assert_eq!(adapter.window_for_space(&main), Some(first));
        assert_eq!(adapter.space_for_window(first), Some(&main));

        let previous = adapter
            .register_viewport(main.clone(), second)
            .expect("replacing a space should return the previous snapshot");
        assert_eq!(previous.window, first);
        assert_eq!(adapter.window_for_space(&main), Some(second));
        assert_eq!(adapter.space_for_window(first), None);

        adapter.register_viewport(secondary.clone(), second);
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

        adapter.register_viewport(main.clone(), first);
        adapter.register_viewport(secondary.clone(), second);

        let removed = adapter
            .unregister_space(&main)
            .expect("space should be registered");
        assert_eq!(removed.window, first);
        assert_eq!(adapter.space_for_window(first), None);
        assert_eq!(adapter.window_for_space(&main), None);

        let (removed_space, removed) = adapter
            .unregister_window(second)
            .expect("window should be registered");
        assert_eq!(removed_space, secondary);
        assert_eq!(removed.window, second);
        assert!(adapter.is_empty());
    }

    #[test]
    fn dock_layout_import_does_not_require_viewport_placement() {
        let mut graph = DockGraph::new();
        let main = space("main");
        let tabs = graph.insert_node(DockNode::Tabs {
            items: vec![DockItemId::from("a")],
            active: 0,
        });
        graph.set_root(main.clone(), tabs);

        let mut adapter = DockViewportAdapter::new();
        adapter.register_viewport(main.clone(), handle(1));
        adapter.update_snapshot(
            &main,
            Some(DisplayId::new(7)),
            WindowBounds::Windowed(bounds(100.0, 200.0, 800.0, 600.0)),
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
            active: 0,
        });
        graph.set_root(space("main"), tabs);

        let mut adapter = DockViewportAdapter::new();
        let main = space("main");
        adapter.register_viewport(main.clone(), handle(42));
        adapter.update_snapshot(
            &main,
            Some(DisplayId::new(7)),
            WindowBounds::Windowed(bounds(100.0, 200.0, 800.0, 600.0)),
            bounds(10.0, 20.0, 300.0, 200.0),
        );

        let json = serde_json::to_string(&graph.export_layout()).expect("layout should serialize");
        assert!(!json.contains("WindowHandle"));
        assert!(!json.contains("WindowId"));
        assert!(!json.contains("DisplayId"));
        assert!(!json.contains("AnyWindowHandle"));
    }
}
