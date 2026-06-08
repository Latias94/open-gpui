use crate::{
    DockController, DockHost, DockItemId, DockLayoutRect, DockNodeId, DockPolicy, DockPolicyError,
    DockSpaceId,
};
use open_gpui::{
    AnyWindowHandle, App, AppContext as _, Bounds, DisplayId, Entity, Pixels, Point, Result,
    WindowBounds, WindowId, WindowOptions, point,
};
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

/// Request to open a new platform viewport for a tab released outside known dock viewports.
#[derive(Debug, Clone, PartialEq)]
pub struct DockViewportTearOffRequest {
    /// Source dock space containing the dragged item.
    pub source_space: DockSpaceId,
    /// Source tabs node where the drag started.
    pub source_tabs: DockNodeId,
    /// Item being torn off.
    pub item: DockItemId,
    /// Release position in screen coordinates.
    pub release_position: Point<Pixels>,
    /// Suggested platform window bounds for the new viewport, when known.
    pub suggested_window_bounds: Option<WindowBounds>,
}

/// Result of resolving a tab release against registered platform viewports.
#[derive(Debug, Clone, PartialEq)]
pub enum DockViewportTearOffOutcome {
    /// The release landed inside a known viewport; normal drop handling should continue.
    KnownViewport(DockViewportHit),
    /// The release can open a new platform viewport.
    Requested(DockViewportTearOffRequest),
    /// The request was rejected by docking policy.
    Rejected(DockPolicyError),
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

    /// Returns the saved placement for a logical dock space, when present.
    pub fn placement_for_space(&self, space: &DockSpaceId) -> Option<&DockViewportPlacement> {
        self.viewports
            .iter()
            .find(|viewport| viewport.space == *space)
    }

    /// Applies saved platform-window placement to fallback GPUI window options.
    ///
    /// This validates the placement layout before returning options so restore flows can reject
    /// corrupt placement data before opening runtime windows.
    pub fn window_options_for_space(
        &self,
        space: &DockSpaceId,
        mut fallback: WindowOptions,
    ) -> Result<WindowOptions, DockViewportPlacementValidationError> {
        self.validate()?;

        if let Some(placement) = self.placement_for_space(space) {
            if let Some(display_id) = placement.display_id {
                fallback.display_id = Some(DisplayId::from(display_id));
            }
            fallback.window_bounds = placement
                .window_bounds
                .map(DockViewportWindowBounds::to_window_bounds)
                .or(fallback.window_bounds);
        }

        Ok(fallback)
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

    /// Removes a viewport by GPUI window id and returns a lifecycle outcome.
    ///
    /// This is the cleanup path for close callbacks that report only [`WindowId`].
    pub fn unregister_window_id(
        &mut self,
        window_id: WindowId,
        reason: DockViewportUnregisterReason,
    ) -> Option<DockViewportUnregisterOutcome> {
        let space = self.windows.remove(&window_id)?;
        let snapshot = self.viewports.remove(&space)?;
        Some(DockViewportUnregisterOutcome {
            space,
            window: snapshot.window,
            reason,
        })
    }

    /// Applies viewport close policy to the adapter mapping for a window id.
    ///
    /// `RetainLayout` removes only the runtime mapping. It does not mutate the docking graph.
    /// `Prevent` returns a veto outcome and leaves the mapping intact.
    pub fn close_viewport_mapping(
        &mut self,
        window_id: WindowId,
        policy: DockViewportClosePolicy,
    ) -> DockViewportCloseOutcome {
        let Some(space) = self.windows.get(&window_id).cloned() else {
            return DockViewportCloseOutcome {
                space: None,
                window_id,
                status: DockViewportCloseStatus::UnknownWindow,
            };
        };

        if !self.viewports.contains_key(&space) {
            self.windows.remove(&window_id);
            return DockViewportCloseOutcome {
                space: None,
                window_id,
                status: DockViewportCloseStatus::UnknownWindow,
            };
        }

        match policy {
            DockViewportClosePolicy::Prevent => DockViewportCloseOutcome {
                space: Some(space),
                window_id,
                status: DockViewportCloseStatus::Vetoed,
            },
            DockViewportClosePolicy::RetainLayout => {
                if let Some(outcome) =
                    self.unregister_window_id(window_id, DockViewportUnregisterReason::Closed)
                {
                    DockViewportCloseOutcome {
                        space: Some(outcome.space),
                        window_id,
                        status: DockViewportCloseStatus::Closed,
                    }
                } else {
                    DockViewportCloseOutcome {
                        space: None,
                        window_id,
                        status: DockViewportCloseStatus::UnknownWindow,
                    }
                }
            }
        }
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
        self.space_for_window_id(window.window_id())
    }

    /// Returns the logical dock space rendered by a window id.
    pub fn space_for_window_id(&self, window_id: WindowId) -> Option<&DockSpaceId> {
        self.windows
            .get(&window_id)
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

    /// Resolves a tab release into either an existing viewport hit or a platform tear-off request.
    ///
    /// This method never mutates the docking graph. Callers should open/register a destination
    /// viewport first, then commit a move action after runtime setup succeeds.
    pub fn resolve_tear_off_request(
        &self,
        source_space: impl Into<DockSpaceId>,
        source_tabs: DockNodeId,
        item: impl Into<DockItemId>,
        release_position: Point<Pixels>,
        suggested_window_bounds: Option<WindowBounds>,
        policy: &DockPolicy,
    ) -> DockViewportTearOffOutcome {
        if let Some(hit) = self.hit_test_screen(release_position) {
            return DockViewportTearOffOutcome::KnownViewport(hit);
        }

        if let Err(reason) = policy.validate_platform_viewports() {
            return DockViewportTearOffOutcome::Rejected(reason);
        }

        DockViewportTearOffOutcome::Requested(DockViewportTearOffRequest {
            source_space: source_space.into(),
            source_tabs,
            item: item.into(),
            release_position,
            suggested_window_bounds,
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
    use slotmap::Key;

    fn space(id: &str) -> DockSpaceId {
        DockSpaceId::from(id)
    }

    fn item(id: &str) -> DockItemId {
        DockItemId::from(id)
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
    fn unregistering_by_window_id_clears_close_callback_mapping() {
        let mut adapter = DockViewportAdapter::new();
        let main = space("main");
        let secondary = space("secondary");
        let first = handle(1);
        let second = handle(2);

        adapter.register_viewport(main.clone(), first);
        adapter.register_viewport(secondary.clone(), second);

        let removed = adapter
            .unregister_window_id(first.window_id(), DockViewportUnregisterReason::Closed)
            .expect("window id should be registered");
        assert_eq!(removed.space, main);
        assert_eq!(removed.window, first);
        assert_eq!(removed.reason, DockViewportUnregisterReason::Closed);
        assert_eq!(adapter.space_for_window_id(first.window_id()), None);
        assert_eq!(adapter.window_for_space(&removed.space), None);
        assert_eq!(adapter.window_for_space(&secondary), Some(second));

        assert_eq!(
            adapter.unregister_window_id(first.window_id(), DockViewportUnregisterReason::Closed),
            None
        );
    }

    #[test]
    fn close_policy_retain_layout_removes_only_runtime_mapping() {
        let mut graph = DockGraph::new();
        let main = space("main");
        let tabs = graph.insert_node(DockNode::Tabs {
            items: vec![DockItemId::from("a")],
            active: 0,
        });
        graph.set_root(main.clone(), tabs);

        let mut adapter = DockViewportAdapter::new();
        let window = handle(1);
        adapter.register_viewport(main.clone(), window);

        let outcome = adapter
            .close_viewport_mapping(window.window_id(), DockViewportClosePolicy::RetainLayout);
        assert_eq!(
            outcome,
            DockViewportCloseOutcome {
                space: Some(main.clone()),
                window_id: window.window_id(),
                status: DockViewportCloseStatus::Closed,
            }
        );
        assert!(adapter.is_empty());
        assert!(
            graph.root(&main).is_some(),
            "runtime cleanup must not mutate the logical docking graph"
        );

        let reopened = handle(2);
        adapter.register_viewport(main.clone(), reopened);
        assert_eq!(adapter.window_for_space(&main), Some(reopened));
        assert_eq!(
            adapter.space_for_window_id(reopened.window_id()),
            Some(&main)
        );
    }

    #[test]
    fn close_policy_prevent_vetoes_and_preserves_mapping() {
        let mut adapter = DockViewportAdapter::new();
        let main = space("main");
        let window = handle(1);
        adapter.register_viewport(main.clone(), window);

        let outcome =
            adapter.close_viewport_mapping(window.window_id(), DockViewportClosePolicy::Prevent);
        assert_eq!(
            outcome,
            DockViewportCloseOutcome {
                space: Some(main.clone()),
                window_id: window.window_id(),
                status: DockViewportCloseStatus::Vetoed,
            }
        );
        assert_eq!(adapter.window_for_space(&main), Some(window));
        assert_eq!(adapter.space_for_window_id(window.window_id()), Some(&main));
    }

    #[test]
    fn close_mapping_unknown_window_is_not_reported_as_vetoed() {
        let mut adapter = DockViewportAdapter::new();
        let unknown = WindowId::from(99);

        assert_eq!(
            adapter.close_viewport_mapping(unknown, DockViewportClosePolicy::Prevent),
            DockViewportCloseOutcome {
                space: None,
                window_id: unknown,
                status: DockViewportCloseStatus::UnknownWindow,
            }
        );
        assert_eq!(
            adapter.close_viewport_mapping(unknown, DockViewportClosePolicy::RetainLayout),
            DockViewportCloseOutcome {
                space: None,
                window_id: unknown,
                status: DockViewportCloseStatus::UnknownWindow,
            }
        );
    }

    #[test]
    fn close_mapping_discards_stale_window_index() {
        let mut adapter = DockViewportAdapter::new();
        let main = space("main");
        let window_id = WindowId::from(1);
        adapter.windows.insert(window_id, main);

        assert_eq!(
            adapter.close_viewport_mapping(window_id, DockViewportClosePolicy::Prevent),
            DockViewportCloseOutcome {
                space: None,
                window_id,
                status: DockViewportCloseStatus::UnknownWindow,
            }
        );
        assert_eq!(adapter.windows.get(&window_id), None);
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
    fn tear_off_release_inside_known_viewport_returns_hit() {
        let mut adapter = DockViewportAdapter::new();
        let main = space("main");
        adapter.register_viewport(main.clone(), handle(1));
        adapter.update_snapshot(
            &main,
            Some(DisplayId::new(7)),
            WindowBounds::Windowed(bounds(100.0, 200.0, 800.0, 600.0)),
            bounds(10.0, 20.0, 300.0, 200.0),
        );

        assert_eq!(
            adapter.resolve_tear_off_request(
                main.clone(),
                DockNodeId::null(),
                item("a"),
                point(px(115.0), px(225.0)),
                None,
                &DockPolicy::default(),
            ),
            DockViewportTearOffOutcome::KnownViewport(DockViewportHit {
                space: main,
                host_position: point(px(5.0), px(5.0)),
            })
        );
    }

    #[test]
    fn tear_off_release_outside_viewports_respects_platform_policy() {
        let adapter = DockViewportAdapter::new();
        let main = space("main");

        assert_eq!(
            adapter.resolve_tear_off_request(
                main,
                DockNodeId::null(),
                item("a"),
                point(px(900.0), px(900.0)),
                None,
                &DockPolicy::default(),
            ),
            DockViewportTearOffOutcome::Rejected(DockPolicyError::PlatformViewportsDisabled)
        );
    }

    #[test]
    fn tear_off_release_outside_viewports_emits_request_when_enabled() {
        let adapter = DockViewportAdapter::new();
        let main = space("main");
        let item = item("a");
        let release_position = point(px(900.0), px(900.0));
        let suggested_window_bounds = WindowBounds::Windowed(bounds(880.0, 880.0, 360.0, 240.0));
        let mut policy = DockPolicy::default();
        policy.set_allow_platform_viewports(true);

        assert_eq!(
            adapter.resolve_tear_off_request(
                main.clone(),
                DockNodeId::null(),
                item.clone(),
                release_position,
                Some(suggested_window_bounds),
                &policy,
            ),
            DockViewportTearOffOutcome::Requested(DockViewportTearOffRequest {
                source_space: main,
                source_tabs: DockNodeId::null(),
                item,
                release_position,
                suggested_window_bounds: Some(suggested_window_bounds),
            })
        );
    }

    #[test]
    fn stale_viewport_bounds_do_not_block_tear_off_request() {
        let mut adapter = DockViewportAdapter::new();
        let main = space("main");
        adapter.register_viewport(main.clone(), handle(1));
        let mut policy = DockPolicy::default();
        policy.set_allow_platform_viewports(true);

        assert!(matches!(
            adapter.resolve_tear_off_request(
                main,
                DockNodeId::null(),
                item("a"),
                point(px(115.0), px(225.0)),
                None,
                &policy,
            ),
            DockViewportTearOffOutcome::Requested(_)
        ));
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
    fn placement_window_options_use_saved_bounds_and_display_hint() {
        let main = space("main");
        let saved_bounds = DockViewportWindowBounds {
            state: DockViewportWindowState::Maximized,
            bounds: DockLayoutRect::from_bounds(bounds(100.0, 200.0, 800.0, 600.0)),
        };
        let fallback_bounds = WindowBounds::Windowed(bounds(0.0, 0.0, 320.0, 240.0));
        let placement = DockViewportPlacementLayout::new(vec![DockViewportPlacement {
            space: main.clone(),
            display_id: Some(7),
            window_bounds: Some(saved_bounds),
            host_bounds: None,
        }]);

        let options = placement
            .window_options_for_space(
                &main,
                WindowOptions {
                    window_bounds: Some(fallback_bounds),
                    focus: false,
                    ..Default::default()
                },
            )
            .expect("valid placement should produce window options");

        assert_eq!(
            placement
                .placement_for_space(&main)
                .map(|p| p.space.clone()),
            Some(main)
        );
        assert_eq!(options.window_bounds, Some(saved_bounds.to_window_bounds()));
        assert_eq!(options.display_id, Some(DisplayId::from(7)));
        assert!(
            !options.focus,
            "fallback options should preserve non-placement fields"
        );
    }

    #[test]
    fn placement_window_options_keep_fallback_for_missing_space() {
        let main = space("main");
        let secondary = space("secondary");
        let fallback_bounds = WindowBounds::Windowed(bounds(0.0, 0.0, 320.0, 240.0));
        let placement = DockViewportPlacementLayout::new(vec![DockViewportPlacement {
            space: main.clone(),
            display_id: None,
            window_bounds: None,
            host_bounds: None,
        }]);

        let matching_options = placement
            .window_options_for_space(
                &main,
                WindowOptions {
                    window_bounds: Some(fallback_bounds),
                    display_id: Some(DisplayId::from(9)),
                    ..Default::default()
                },
            )
            .expect("missing saved fields should keep fallback options");
        assert_eq!(matching_options.window_bounds, Some(fallback_bounds));
        assert_eq!(matching_options.display_id, Some(DisplayId::from(9)));

        let options = placement
            .window_options_for_space(
                &secondary,
                WindowOptions {
                    window_bounds: Some(fallback_bounds),
                    display_id: Some(DisplayId::from(9)),
                    ..Default::default()
                },
            )
            .expect("valid placement should preserve fallback for missing spaces");

        assert!(placement.placement_for_space(&secondary).is_none());
        assert_eq!(options.window_bounds, Some(fallback_bounds));
        assert_eq!(options.display_id, Some(DisplayId::from(9)));
    }

    #[test]
    fn invalid_placement_rejects_window_options_before_runtime_mutation() {
        let main = space("main");
        let mut adapter = DockViewportAdapter::new();
        adapter.register_viewport(main.clone(), handle(1));
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

        let error = placement
            .window_options_for_space(&main, WindowOptions::default())
            .expect_err("invalid placement should fail before options are returned");
        assert_eq!(
            error,
            DockViewportPlacementValidationError::DuplicateSpace {
                space: main.clone()
            }
        );
        assert_eq!(adapter.window_for_space(&main), Some(handle(1)));
        assert_eq!(adapter.spaces(), vec![main]);
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
