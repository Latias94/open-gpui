#[cfg(test)]
use crate::viewport_target_resolver::choose_diagnostic_viewport_target;
use crate::{DockLayoutRect, DockSpaceId};
use open_gpui::WindowBounds;
use serde::{Deserialize, Serialize};

/// Current viewport placement serialization version.
pub const DOCK_VIEWPORT_PLACEMENT_VERSION: u32 = 1;

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        DockViewportAdapter, DockViewportHit, DockViewportPlacementValidationError,
        DockViewportRestoreReadiness, DockViewportTargetContext, DockViewportWindowFacts,
        viewport_test_support::{bounds, handle, register_viewport, space},
    };
    use open_gpui::{DisplayId, WindowBounds, WindowOptions, point, px};

    #[test]
    fn viewport_placement_roundtrips_without_runtime_window_handles() {
        let mut adapter = DockViewportAdapter::new();
        let main = space("main");
        let secondary = space("secondary");
        register_viewport(&mut adapter, main.clone(), handle(1));
        register_viewport(&mut adapter, secondary, handle(2));
        assert!(adapter.update_snapshot(
            &main,
            DockViewportWindowFacts::new(
                Some(DisplayId::new(7)),
                WindowBounds::Maximized(bounds(100.0, 200.0, 800.0, 600.0)),
                bounds(0.0, 0.0, 1440.0, 900.0),
            ),
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
        register_viewport(&mut restored, main.clone(), handle(99));
        assert_eq!(
            restored
                .check_placement_restore(&placement)
                .expect("placement restore readiness should be checked"),
            DockViewportRestoreReadiness {
                matched: 1,
                missing: 1,
            }
        );

        let snapshot = restored
            .snapshot(&main)
            .expect("main viewport should be restored");
        assert_eq!(snapshot.window, handle(99));
        assert_eq!(snapshot.display_id, None);
        assert_eq!(snapshot.window_bounds, None);
        assert_eq!(snapshot.current_bounds, None);
        assert_eq!(snapshot.host_geometry, None);
    }

    #[test]
    fn viewport_restore_workflow_waits_for_live_window_facts_after_saved_placement() {
        let mut adapter = DockViewportAdapter::new();
        let main = space("main");
        let secondary = space("secondary");
        register_viewport(&mut adapter, main.clone(), handle(1));
        register_viewport(&mut adapter, secondary.clone(), handle(2));
        adapter.update_snapshot(
            &main,
            DockViewportWindowFacts::from_window_bounds(WindowBounds::Windowed(bounds(
                100.0, 200.0, 800.0, 600.0,
            )))
            .with_display_id(Some(DisplayId::new(7))),
            bounds(10.0, 20.0, 300.0, 200.0),
        );
        adapter.update_snapshot(
            &secondary,
            DockViewportWindowFacts::from_window_bounds(WindowBounds::Windowed(bounds(
                900.0, 200.0, 500.0, 400.0,
            )))
            .with_display_id(Some(DisplayId::new(8))),
            bounds(30.0, 40.0, 240.0, 180.0),
        );
        let placement = adapter.export_placement();

        let mut restored = DockViewportAdapter::new();
        register_viewport(&mut restored, main.clone(), handle(101));
        register_viewport(&mut restored, secondary.clone(), handle(102));

        assert_eq!(
            restored
                .check_placement_restore(&placement)
                .expect("saved placement should validate registered restore windows"),
            DockViewportRestoreReadiness {
                matched: 2,
                missing: 0,
            }
        );
        assert_eq!(restored.window_for_space(&main), Some(handle(101)));
        assert_eq!(
            restored.space_for_window_id(handle(102).window_id()),
            Some(&secondary)
        );
        assert!(
            choose_diagnostic_viewport_target(
                restored.global_screen_viewport_hits(point(px(935.0), px(245.0))),
                &DockViewportTargetContext::new(),
            )
            .is_none(),
            "saved placement must not masquerade as live screen coordinates"
        );
        restored.update_snapshot(
            &secondary,
            DockViewportWindowFacts::from_window_bounds(WindowBounds::Windowed(bounds(
                900.0, 200.0, 500.0, 400.0,
            )))
            .with_display_id(Some(DisplayId::new(8))),
            bounds(30.0, 40.0, 240.0, 180.0),
        );
        let hits = restored.global_screen_viewport_hits(point(px(935.0), px(245.0)));
        assert_eq!(
            choose_diagnostic_viewport_target(hits, &DockViewportTargetContext::new())
                .map(|target| target.into_hit()),
            Some(DockViewportHit::new(secondary, point(px(5.0), px(5.0))))
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
        register_viewport(&mut adapter, main.clone(), handle(1));
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
    fn viewport_placement_validation_rejects_invalid_bounds_before_mutation() {
        let main = space("main");
        let invalid_window = DockViewportPlacementLayout::new(vec![DockViewportPlacement {
            space: main.clone(),
            display_id: None,
            window_bounds: Some(DockViewportWindowBounds {
                state: DockViewportWindowState::Windowed,
                bounds: DockLayoutRect {
                    x: 10.0,
                    y: 20.0,
                    width: -1.0,
                    height: 200.0,
                },
            }),
            host_bounds: None,
        }]);
        assert_eq!(
            invalid_window.validate(),
            Err(DockViewportPlacementValidationError::InvalidWindowBounds {
                space: main.clone()
            })
        );

        let invalid_host = DockViewportPlacementLayout::new(vec![DockViewportPlacement {
            space: main.clone(),
            display_id: None,
            window_bounds: None,
            host_bounds: Some(DockLayoutRect {
                x: f32::INFINITY,
                y: 20.0,
                width: 300.0,
                height: 200.0,
            }),
        }]);
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

        assert_eq!(
            adapter
                .check_placement_restore(&invalid_host)
                .expect_err("invalid host bounds should reject before snapshot mutation"),
            DockViewportPlacementValidationError::InvalidHostBounds {
                space: main.clone()
            }
        );
        let snapshot = adapter
            .snapshot(&main)
            .expect("registered viewport should remain");
        assert_eq!(snapshot.display_id, Some(DisplayId::new(7)));
        assert_eq!(
            snapshot.window_bounds,
            Some(WindowBounds::Windowed(bounds(100.0, 200.0, 800.0, 600.0)))
        );
        assert_eq!(
            snapshot.global_screen_bounds(),
            Some(bounds(100.0, 200.0, 800.0, 600.0))
        );
        assert_eq!(
            snapshot
                .host_geometry
                .map(crate::DockViewportHostGeometry::layout_bounds),
            Some(bounds(10.0, 20.0, 300.0, 200.0))
        );
    }
}
