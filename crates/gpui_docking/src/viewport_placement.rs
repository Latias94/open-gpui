use crate::{DockLayoutRect, DockSpaceId, DockViewportAdapter};
use open_gpui::{DisplayId, WindowBounds, WindowOptions};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use thiserror::Error;

/// Current viewport placement serialization version.
pub const DOCK_VIEWPORT_PLACEMENT_VERSION: u32 = 1;

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
    /// Exports serializable placement snapshots for all registered viewports.
    pub fn export_placement(&self) -> DockViewportPlacementLayout {
        DockViewportPlacementLayout::new(
            self.spaces()
                .into_iter()
                .filter_map(|space| {
                    let snapshot = self.snapshot(&space)?;
                    Some(DockViewportPlacement {
                        space,
                        display_id: snapshot.display_id.map(u64::from),
                        window_bounds: snapshot
                            .window_bounds
                            .map(DockViewportWindowBounds::from_window_bounds),
                        host_bounds: snapshot.host_bounds.map(DockLayoutRect::from_bounds),
                    })
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
            let Some(snapshot) = self.snapshot_mut(&viewport.space) else {
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
    use crate::{DockHost, DockViewportAdapter, DockViewportHit};
    use open_gpui::{
        AnyWindowHandle, Bounds, DisplayId, Pixels, WindowBounds, WindowHandle, WindowId,
        WindowOptions, point, px, size,
    };

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
}
