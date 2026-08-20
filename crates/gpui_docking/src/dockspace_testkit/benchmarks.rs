/// Stable scenario metadata shared by docking benchmarks and functional fixtures.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DockspaceBenchmarkCase {
    scenario_id: &'static str,
    operation: &'static str,
    measurement_layer: &'static str,
}

impl DockspaceBenchmarkCase {
    /// Small retained tab-grid resolver benchmark.
    pub const fn retained_tab_grid_small() -> Self {
        Self {
            scenario_id: "retained_tab_grid_small",
            operation: "tab_drag_preview",
            measurement_layer: "resolver",
        }
    }

    /// Large retained tab-grid resolver benchmark.
    pub const fn retained_tab_grid_large() -> Self {
        Self {
            scenario_id: "retained_tab_grid_large",
            operation: "tab_drag_preview",
            measurement_layer: "resolver",
        }
    }

    /// Complex split/floating scenario benchmark.
    pub const fn complex_split_float() -> Self {
        Self {
            scenario_id: "tabs_split_float_merge",
            operation: "tab_to_nested_edge_preview",
            measurement_layer: "resolver",
        }
    }

    /// Known-target multiview route benchmark.
    pub const fn multiview_known_target() -> Self {
        Self {
            scenario_id: "three_viewport_rehome",
            operation: "cross_viewport_known_target_preview",
            measurement_layer: "multiview_route",
        }
    }

    /// Unknown-target multiview fail-closed route benchmark.
    pub const fn multiview_unknown_target() -> Self {
        Self {
            scenario_id: "three_viewport_rehome",
            operation: "cross_viewport_unknown_target_rejection",
            measurement_layer: "multiview_route_fail_closed",
        }
    }

    /// Stable scenario id shared with functional fixtures.
    pub const fn scenario_id(self) -> &'static str {
        self.scenario_id
    }

    /// User-facing operation family measured by this benchmark row.
    pub const fn operation(self) -> &'static str {
        self.operation
    }

    /// Measurement layer, such as resolver or multiview route lookup.
    pub const fn measurement_layer(self) -> &'static str {
        self.measurement_layer
    }
}
