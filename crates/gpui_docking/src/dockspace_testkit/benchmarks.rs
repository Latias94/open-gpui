#![cfg_attr(test, allow(dead_code))]

/// Stable scenario metadata shared by docking benchmarks and functional fixtures.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DockspaceBenchmarkCase {
    scenario_id: &'static str,
    step_id: Option<&'static str>,
    operation_family: &'static str,
    measurement_layer: &'static str,
    scope: &'static str,
}

impl DockspaceBenchmarkCase {
    /// Small retained tab-grid resolver benchmark.
    pub const fn retained_tab_grid_small() -> Self {
        Self {
            scenario_id: "synthetic_retained_tab_grid_small",
            step_id: None,
            operation_family: "tab_drag_preview",
            measurement_layer: "resolver",
            scope: "retained_scene_8_stacks_8_tabs",
        }
    }

    /// Large retained tab-grid resolver benchmark.
    pub const fn retained_tab_grid_large() -> Self {
        Self {
            scenario_id: "synthetic_retained_tab_grid_large",
            step_id: None,
            operation_family: "tab_drag_preview",
            measurement_layer: "resolver",
            scope: "retained_scene_64_stacks_8_tabs",
        }
    }

    /// Complex split/floating scenario benchmark.
    pub const fn complex_split_float() -> Self {
        Self {
            scenario_id: "tabs_split_float_merge",
            step_id: Some("tab_to_nested_edge_preview"),
            operation_family: "tab_drag_preview",
            measurement_layer: "resolver",
            scope: "retained_fixture_scene",
        }
    }

    /// Known-target multiview route benchmark.
    pub const fn multiview_known_target() -> Self {
        Self {
            scenario_id: "synthetic_multiview_registry_8x8x8",
            step_id: None,
            operation_family: "known_target_route_preview",
            measurement_layer: "multiview_route",
            scope: "registry_8_hosts_8_stacks_8_tabs",
        }
    }

    /// Unknown-target multiview fail-closed route benchmark.
    pub const fn multiview_unknown_target() -> Self {
        Self {
            scenario_id: "synthetic_multiview_registry_8x8x8",
            step_id: None,
            operation_family: "unknown_target_route_rejection",
            measurement_layer: "multiview_route_fail_closed",
            scope: "registry_8_hosts_8_stacks_8_tabs",
        }
    }

    /// Full eight-step three-dockspace workspace transaction cycle.
    pub const fn three_dockspace_rehome_cycle() -> Self {
        Self {
            scenario_id: "three_dockspace_rehome_cycle",
            step_id: None,
            operation_family: "named_item_docking_sequence",
            measurement_layer: "headless_logical_dockspace_resolve_and_commit",
            scope: "three_dockspaces_8_steps_2_edge_6_center_no_platform_windows",
        }
    }

    /// Stable scenario id shared with functional fixtures.
    pub const fn scenario_id(self) -> &'static str {
        self.scenario_id
    }

    /// Stable fixture step id when this row measures one concrete scenario step.
    pub const fn step_id(self) -> Option<&'static str> {
        self.step_id
    }

    /// User-facing operation family measured by this benchmark row.
    pub const fn operation_family(self) -> &'static str {
        self.operation_family
    }

    /// Measurement layer, such as resolver or multiview route lookup.
    pub const fn measurement_layer(self) -> &'static str {
        self.measurement_layer
    }

    /// Workload scope required for apples-to-apples row pairing.
    pub const fn scope(self) -> &'static str {
        self.scope
    }
}
