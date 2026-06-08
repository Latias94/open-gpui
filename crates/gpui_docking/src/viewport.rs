use crate::{DockLayoutRect, DockSpaceId};
use open_gpui::{AnyWindowHandle, Bounds, DisplayId, Pixels, Point, WindowBounds, WindowId, point};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use thiserror::Error;

/// Current viewport placement serialization version.
pub const DOCK_VIEWPORT_PLACEMENT_VERSION: u32 = 1;

/// Runtime adapter state that maps logical dock spaces to GPUI windows.
///
/// This type owns platform-window facts for docking: window handles, display ids, and the latest
/// bounds snapshots used for coordinate conversion. None of this state belongs in
/// [`DockGraph`](crate::DockGraph) or [`DockLayout`](crate::DockLayout).
///
/// A typical restore flow imports [`DockLayout`](crate::DockLayout) into a controller, opens or
/// reuses GPUI windows for each logical dock space, registers those windows here, then applies a
/// [`DockViewportPlacementLayout`] to rehydrate placement snapshots for coordinate conversion.
#[derive(Debug, Default)]
pub struct DockViewportAdapter {
    viewports: BTreeMap<DockSpaceId, DockViewportSnapshot>,
    windows: HashMap<WindowId, DockSpaceId>,
}

/// Runtime snapshot for one rendered dock viewport.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DockViewportSnapshot {
    /// GPUI window currently rendering the logical dock space.
    pub window: AnyWindowHandle,
    /// Display containing the window, when the application has recorded one.
    pub display_id: Option<DisplayId>,
    /// Last known platform window bounds in screen coordinates.
    pub window_bounds: Option<WindowBounds>,
    /// Last known dock host bounds in window-local coordinates.
    pub host_bounds: Option<Bounds<Pixels>>,
}

impl DockViewportSnapshot {
    /// Creates a snapshot for a newly registered viewport window.
    pub fn new(window: AnyWindowHandle) -> Self {
        Self {
            window,
            display_id: None,
            window_bounds: None,
            host_bounds: None,
        }
    }
}

/// Result of resolving a screen point into a registered dock viewport.
#[derive(Debug, Clone, PartialEq)]
pub struct DockViewportHit {
    /// Logical dock space that contains the point.
    pub space: DockSpaceId,
    /// Point relative to the dock host bounds.
    pub host_position: Point<Pixels>,
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

/// Default behavior for a platform viewport close request.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum DockViewportClosePolicy {
    /// Unregister the runtime window and keep the logical dock layout available for reopen.
    #[default]
    RetainLayout,
    /// Reject the close request and leave the runtime mapping intact.
    Prevent,
}

/// Runtime result of closing a platform viewport.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DockViewportCloseOutcome {
    /// Logical dock space that was associated with the closed window, when known.
    pub space: Option<DockSpaceId>,
    /// GPUI window id received from the close callback.
    pub window_id: WindowId,
    /// How the close request resolved.
    pub status: DockViewportCloseStatus,
}

/// How a close request resolved.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DockViewportCloseStatus {
    /// The window closed and its runtime mapping was removed.
    Closed,
    /// Policy rejected the close request before the window closed.
    Vetoed,
    /// The runtime did not know the closed window id.
    UnknownWindow,
}

/// Runtime result of unregistering a platform viewport mapping.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DockViewportUnregisterOutcome {
    /// Logical dock space removed from the adapter mapping.
    pub space: DockSpaceId,
    /// GPUI window removed from the adapter mapping.
    pub window: AnyWindowHandle,
    /// Why the mapping was removed.
    pub reason: DockViewportUnregisterReason,
}

/// Reason a platform viewport mapping was removed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DockViewportUnregisterReason {
    /// The platform window closed.
    Closed,
    /// A new window replaced the previous mapping.
    Replaced,
    /// The application discarded runtime placement for the space.
    Discarded,
}

/// Summary of applying saved viewport placement to runtime windows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DockViewportRestoreOutcome {
    /// Number of saved placement entries applied to registered windows.
    pub applied: usize,
    /// Number of saved placement entries skipped because no runtime window was registered.
    pub skipped: usize,
}

/// Serializable adapter-level viewport placement data.
///
/// This record is intentionally separate from [`DockLayout`](crate::DockLayout): it stores
/// platform-window placement hints but never stores GPUI window handles.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DockViewportPlacementLayout {
    /// Placement schema version.
    pub placement_version: u32,
    /// Serialized viewport placements.
    pub viewports: Vec<DockViewportPlacement>,
}

impl DockViewportPlacementLayout {
    /// Creates a placement layout with the current schema version.
    pub fn new(viewports: Vec<DockViewportPlacement>) -> Self {
        Self {
            placement_version: DOCK_VIEWPORT_PLACEMENT_VERSION,
            viewports,
        }
    }

    /// Validates adapter-level placement invariants before applying snapshots.
    pub fn validate(&self) -> Result<(), DockViewportPlacementValidationError> {
        if self.placement_version != DOCK_VIEWPORT_PLACEMENT_VERSION {
            return Err(DockViewportPlacementValidationError::UnsupportedVersion {
                expected: DOCK_VIEWPORT_PLACEMENT_VERSION,
                found: self.placement_version,
            });
        }

        let mut spaces = BTreeSet::new();
        for viewport in &self.viewports {
            if !spaces.insert(viewport.space.clone()) {
                return Err(DockViewportPlacementValidationError::DuplicateSpace {
                    space: viewport.space.clone(),
                });
            }
        }

        Ok(())
    }
}

/// Serializable placement snapshot for one logical dock space.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DockViewportPlacement {
    /// Logical dock space id.
    pub space: DockSpaceId,
    /// Last known display id, when recorded by the application.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display_id: Option<u64>,
    /// Last known platform window bounds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub window_bounds: Option<DockViewportWindowBounds>,
    /// Last known dock host bounds in window-local coordinates.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub host_bounds: Option<DockLayoutRect>,
}

/// Serializable platform window state plus restore bounds.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct DockViewportWindowBounds {
    /// Platform window state.
    pub state: DockViewportWindowState,
    /// Restore bounds in logical pixels.
    pub bounds: DockLayoutRect,
}

impl DockViewportWindowBounds {
    /// Converts GPUI window bounds into a serializable placement value.
    pub fn from_window_bounds(bounds: WindowBounds) -> Self {
        match bounds {
            WindowBounds::Windowed(bounds) => Self {
                state: DockViewportWindowState::Windowed,
                bounds: DockLayoutRect::from_bounds(bounds),
            },
            WindowBounds::Maximized(bounds) => Self {
                state: DockViewportWindowState::Maximized,
                bounds: DockLayoutRect::from_bounds(bounds),
            },
            WindowBounds::Fullscreen(bounds) => Self {
                state: DockViewportWindowState::Fullscreen,
                bounds: DockLayoutRect::from_bounds(bounds),
            },
        }
    }

    /// Converts this placement value into GPUI window bounds.
    pub fn to_window_bounds(self) -> WindowBounds {
        match self.state {
            DockViewportWindowState::Windowed => WindowBounds::Windowed(self.bounds.to_bounds()),
            DockViewportWindowState::Maximized => WindowBounds::Maximized(self.bounds.to_bounds()),
            DockViewportWindowState::Fullscreen => {
                WindowBounds::Fullscreen(self.bounds.to_bounds())
            }
        }
    }
}

/// Serializable platform window state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DockViewportWindowState {
    /// Windowed restore state.
    Windowed,
    /// Maximized restore state.
    Maximized,
    /// Fullscreen restore state.
    Fullscreen,
}

/// Validation error for serialized viewport placement data.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum DockViewportPlacementValidationError {
    /// The placement version is unsupported.
    #[error("unsupported dock viewport placement version: expected {expected}, found {found}")]
    UnsupportedVersion {
        /// Expected version.
        expected: u32,
        /// Found version.
        found: u32,
    },
    /// A dock space appears more than once in placement data.
    #[error("duplicate dock viewport placement space: {space}")]
    DuplicateSpace {
        /// Duplicate dock space id.
        space: DockSpaceId,
    },
}

impl DockViewportAdapter {
    /// Creates an empty viewport adapter.
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns true when no viewport mappings are registered.
    pub fn is_empty(&self) -> bool {
        self.viewports.is_empty()
    }

    /// Returns the number of registered logical viewports.
    pub fn len(&self) -> usize {
        self.viewports.len()
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
        let window_id = window.window_id();

        if let Some(previous) = self.viewports.get(&space) {
            self.windows.remove(&previous.window.window_id());
        }
        if let Some(previous_space) = self.windows.remove(&window_id)
            && previous_space != space
        {
            self.viewports.remove(&previous_space);
        }

        self.windows.insert(window_id, space.clone());
        self.viewports
            .insert(space, DockViewportSnapshot::new(window))
    }

    /// Removes a viewport by logical dock space.
    pub fn unregister_space(&mut self, space: &DockSpaceId) -> Option<DockViewportSnapshot> {
        let snapshot = self.viewports.remove(space)?;
        self.windows.remove(&snapshot.window.window_id());
        Some(snapshot)
    }

    /// Removes a viewport by GPUI window handle.
    pub fn unregister_window(
        &mut self,
        window: impl Into<AnyWindowHandle>,
    ) -> Option<(DockSpaceId, DockViewportSnapshot)> {
        let window = window.into();
        let space = self.windows.remove(&window.window_id())?;
        let snapshot = self.viewports.remove(&space)?;
        Some((space, snapshot))
    }

    /// Returns the snapshot for a logical dock space.
    pub fn snapshot(&self, space: &DockSpaceId) -> Option<&DockViewportSnapshot> {
        self.viewports.get(space)
    }

    /// Returns the window rendering a logical dock space.
    pub fn window_for_space(&self, space: &DockSpaceId) -> Option<AnyWindowHandle> {
        self.snapshot(space).map(|snapshot| snapshot.window)
    }

    /// Returns the logical dock space rendered by a window.
    pub fn space_for_window(&self, window: impl Into<AnyWindowHandle>) -> Option<&DockSpaceId> {
        let window = window.into();
        self.windows
            .get(&window.window_id())
            .and_then(|space| self.viewports.get_key_value(space).map(|(space, _)| space))
    }

    /// Returns known dock spaces in stable lexical order.
    pub fn spaces(&self) -> Vec<DockSpaceId> {
        self.viewports.keys().cloned().collect()
    }

    /// Updates the display id snapshot for a logical dock space.
    pub fn set_display_id(&mut self, space: &DockSpaceId, display_id: Option<DisplayId>) -> bool {
        let Some(snapshot) = self.viewports.get_mut(space) else {
            return false;
        };
        snapshot.display_id = display_id;
        true
    }

    /// Updates the platform window bounds snapshot for a logical dock space.
    pub fn set_window_bounds(&mut self, space: &DockSpaceId, bounds: WindowBounds) -> bool {
        let Some(snapshot) = self.viewports.get_mut(space) else {
            return false;
        };
        snapshot.window_bounds = Some(bounds);
        true
    }

    /// Updates the dock host bounds snapshot for a logical dock space.
    pub fn set_host_bounds(&mut self, space: &DockSpaceId, bounds: Bounds<Pixels>) -> bool {
        let Some(snapshot) = self.viewports.get_mut(space) else {
            return false;
        };
        snapshot.host_bounds = Some(bounds);
        true
    }

    /// Updates display id, window bounds, and host bounds in one snapshot write.
    pub fn update_snapshot(
        &mut self,
        space: &DockSpaceId,
        display_id: Option<DisplayId>,
        window_bounds: WindowBounds,
        host_bounds: Bounds<Pixels>,
    ) -> bool {
        let Some(snapshot) = self.viewports.get_mut(space) else {
            return false;
        };
        snapshot.display_id = display_id;
        snapshot.window_bounds = Some(window_bounds);
        snapshot.host_bounds = Some(host_bounds);
        true
    }

    /// Converts a window-local point into host-local coordinates.
    ///
    /// Returns `None` when the viewport is unknown, host bounds are stale, or the point is outside
    /// the host bounds.
    pub fn window_to_host(
        &self,
        space: &DockSpaceId,
        position: Point<Pixels>,
    ) -> Option<Point<Pixels>> {
        let host_bounds = self.snapshot(space)?.host_bounds?;
        if !host_bounds.contains(&position) {
            return None;
        }

        Some(point(
            position.x - host_bounds.origin.x,
            position.y - host_bounds.origin.y,
        ))
    }

    /// Converts a screen point into host-local coordinates.
    ///
    /// Returns `None` when the viewport is unknown, bounds snapshots are stale, or the point is
    /// outside the host bounds.
    pub fn screen_to_host(
        &self,
        space: &DockSpaceId,
        position: Point<Pixels>,
    ) -> Option<Point<Pixels>> {
        let snapshot = self.snapshot(space)?;
        let window_bounds = snapshot.window_bounds?.get_bounds();
        let window_position = point(
            position.x - window_bounds.origin.x,
            position.y - window_bounds.origin.y,
        );
        self.window_to_host(space, window_position)
    }

    /// Converts a host-local point into screen coordinates.
    pub fn host_to_screen(
        &self,
        space: &DockSpaceId,
        position: Point<Pixels>,
    ) -> Option<Point<Pixels>> {
        let snapshot = self.snapshot(space)?;
        let window_bounds = snapshot.window_bounds?.get_bounds();
        let host_bounds = snapshot.host_bounds?;
        Some(point(
            window_bounds.origin.x + host_bounds.origin.x + position.x,
            window_bounds.origin.y + host_bounds.origin.y + position.y,
        ))
    }

    /// Finds the registered viewport containing a screen point.
    pub fn hit_test_screen(&self, position: Point<Pixels>) -> Option<DockViewportHit> {
        self.viewports.iter().find_map(|(space, _)| {
            self.screen_to_host(space, position)
                .map(|host_position| DockViewportHit {
                    space: space.clone(),
                    host_position,
                })
        })
    }

    /// Exports serializable placement snapshots for all registered viewports.
    pub fn export_placement(&self) -> DockViewportPlacementLayout {
        DockViewportPlacementLayout::new(
            self.viewports
                .iter()
                .map(|(space, snapshot)| DockViewportPlacement {
                    space: space.clone(),
                    display_id: snapshot.display_id.map(u64::from),
                    window_bounds: snapshot
                        .window_bounds
                        .map(DockViewportWindowBounds::from_window_bounds),
                    host_bounds: snapshot.host_bounds.map(DockLayoutRect::from_bounds),
                })
                .collect(),
        )
    }

    /// Applies placement snapshots to already registered viewport windows.
    ///
    /// This does not open windows or create viewport mappings. Applications should first register
    /// the windows they restored, then apply placement data to rehydrate adapter snapshots.
    pub fn apply_placement(
        &mut self,
        placement: &DockViewportPlacementLayout,
    ) -> Result<DockViewportRestoreOutcome, DockViewportPlacementValidationError> {
        placement.validate()?;

        let mut applied = 0;
        let mut skipped = 0;
        for viewport in &placement.viewports {
            let Some(snapshot) = self.viewports.get_mut(&viewport.space) else {
                skipped += 1;
                continue;
            };
            snapshot.display_id = viewport.display_id.map(DisplayId::from);
            snapshot.window_bounds = viewport
                .window_bounds
                .map(DockViewportWindowBounds::to_window_bounds);
            snapshot.host_bounds = viewport.host_bounds.map(DockLayoutRect::to_bounds);
            applied += 1;
        }

        Ok(DockViewportRestoreOutcome { applied, skipped })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{DockGraph, DockHost, DockItemId, DockNode};
    use open_gpui::{WindowHandle, px, size};

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
    fn viewport_lifecycle_types_preserve_runtime_boundaries() {
        let main = space("main");
        let window = handle(7);
        let open = DockViewportOpenOutcome {
            space: main.clone(),
            window,
            status: DockViewportOpenStatus::Opened,
        };
        assert_eq!(open.space, main.clone());
        assert_eq!(open.window, window);
        assert_eq!(open.status, DockViewportOpenStatus::Opened);
        assert_eq!(
            DockViewportClosePolicy::default(),
            DockViewportClosePolicy::RetainLayout
        );

        let close = DockViewportCloseOutcome {
            space: Some(main.clone()),
            window_id: window.window_id(),
            status: DockViewportCloseStatus::Closed,
        };
        assert_eq!(close.space, Some(main.clone()));
        assert_eq!(close.window_id, window.window_id());
        assert_eq!(close.status, DockViewportCloseStatus::Closed);

        let unregister = DockViewportUnregisterOutcome {
            space: main,
            window,
            reason: DockViewportUnregisterReason::Closed,
        };
        assert_eq!(unregister.reason, DockViewportUnregisterReason::Closed);
    }

    #[test]
    fn coordinate_conversion_requires_current_bounds_snapshots() {
        let mut adapter = DockViewportAdapter::new();
        let main = space("main");
        adapter.register_viewport(main.clone(), handle(1));

        assert!(
            adapter
                .screen_to_host(&main, point(px(115.0), px(225.0)))
                .is_none()
        );

        assert!(adapter.update_snapshot(
            &main,
            Some(DisplayId::new(7)),
            WindowBounds::Windowed(bounds(100.0, 200.0, 800.0, 600.0)),
            bounds(10.0, 20.0, 300.0, 200.0),
        ));

        assert_eq!(
            adapter.window_to_host(&main, point(px(15.0), px(25.0))),
            Some(point(px(5.0), px(5.0)))
        );
        assert_eq!(
            adapter.screen_to_host(&main, point(px(115.0), px(225.0))),
            Some(point(px(5.0), px(5.0)))
        );
        assert_eq!(
            adapter.host_to_screen(&main, point(px(5.0), px(5.0))),
            Some(point(px(115.0), px(225.0)))
        );
        assert_eq!(
            adapter.hit_test_screen(point(px(115.0), px(225.0))),
            Some(DockViewportHit {
                space: main.clone(),
                host_position: point(px(5.0), px(5.0)),
            })
        );
        assert!(
            adapter
                .screen_to_host(&main, point(px(500.0), px(500.0)))
                .is_none()
        );
    }

    #[test]
    fn viewport_placement_roundtrips_without_runtime_window_handles() {
        let mut adapter = DockViewportAdapter::new();
        let main = space("main");
        let secondary = space("secondary");
        adapter.register_viewport(main.clone(), handle(1));
        adapter.register_viewport(secondary, handle(2));
        assert!(adapter.update_snapshot(
            &main,
            Some(DisplayId::new(7)),
            WindowBounds::Maximized(bounds(100.0, 200.0, 800.0, 600.0)),
            bounds(10.0, 20.0, 300.0, 200.0),
        ));

        let placement = adapter.export_placement();
        let json = serde_json::to_string(&placement).expect("placement should serialize");
        assert!(json.contains("placement_version"));
        assert!(json.contains("maximized"));
        assert!(!json.contains("WindowHandle"));
        assert!(!json.contains("WindowId"));
        assert!(!json.contains("AnyWindowHandle"));

        let placement: DockViewportPlacementLayout =
            serde_json::from_str(&json).expect("placement should deserialize");
        let mut restored = DockViewportAdapter::new();
        restored.register_viewport(main.clone(), handle(99));
        assert_eq!(
            restored
                .apply_placement(&placement)
                .expect("placement should apply"),
            DockViewportRestoreOutcome {
                applied: 1,
                skipped: 1,
            }
        );

        let snapshot = restored
            .snapshot(&main)
            .expect("main viewport should be restored");
        assert_eq!(snapshot.window, handle(99));
        assert_eq!(snapshot.display_id, Some(DisplayId::new(7)));
        assert_eq!(
            snapshot.window_bounds,
            Some(WindowBounds::Maximized(bounds(100.0, 200.0, 800.0, 600.0)))
        );
        assert_eq!(snapshot.host_bounds, Some(bounds(10.0, 20.0, 300.0, 200.0)));
    }

    #[test]
    fn viewport_restore_workflow_uses_new_runtime_windows_with_saved_placement() {
        let mut adapter = DockViewportAdapter::new();
        let main = space("main");
        let secondary = space("secondary");
        adapter.register_viewport(main.clone(), handle(1));
        adapter.register_viewport(secondary.clone(), handle(2));
        adapter.update_snapshot(
            &main,
            Some(DisplayId::new(7)),
            WindowBounds::Windowed(bounds(100.0, 200.0, 800.0, 600.0)),
            bounds(10.0, 20.0, 300.0, 200.0),
        );
        adapter.update_snapshot(
            &secondary,
            Some(DisplayId::new(8)),
            WindowBounds::Windowed(bounds(900.0, 200.0, 500.0, 400.0)),
            bounds(30.0, 40.0, 240.0, 180.0),
        );
        let placement = adapter.export_placement();

        let mut restored = DockViewportAdapter::new();
        restored.register_viewport(main.clone(), handle(101));
        restored.register_viewport(secondary.clone(), handle(102));

        assert_eq!(
            restored
                .apply_placement(&placement)
                .expect("saved placement should apply to registered restore windows"),
            DockViewportRestoreOutcome {
                applied: 2,
                skipped: 0,
            }
        );
        assert_eq!(restored.window_for_space(&main), Some(handle(101)));
        assert_eq!(restored.space_for_window(handle(102)), Some(&secondary));
        assert_eq!(
            restored.hit_test_screen(point(px(935.0), px(245.0))),
            Some(DockViewportHit {
                space: secondary,
                host_position: point(px(5.0), px(5.0)),
            })
        );
    }

    #[test]
    fn viewport_placement_validation_rejects_bad_version_and_duplicate_spaces() {
        let main = space("main");
        let mut placement = DockViewportPlacementLayout::new(vec![DockViewportPlacement {
            space: main.clone(),
            display_id: None,
            window_bounds: None,
            host_bounds: None,
        }]);
        placement.placement_version = 99;
        assert_eq!(
            placement.validate(),
            Err(DockViewportPlacementValidationError::UnsupportedVersion {
                expected: DOCK_VIEWPORT_PLACEMENT_VERSION,
                found: 99,
            })
        );

        let placement = DockViewportPlacementLayout::new(vec![
            DockViewportPlacement {
                space: main.clone(),
                display_id: None,
                window_bounds: None,
                host_bounds: None,
            },
            DockViewportPlacement {
                space: main.clone(),
                display_id: None,
                window_bounds: None,
                host_bounds: None,
            },
        ]);
        assert_eq!(
            placement.validate(),
            Err(DockViewportPlacementValidationError::DuplicateSpace { space: main })
        );
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
