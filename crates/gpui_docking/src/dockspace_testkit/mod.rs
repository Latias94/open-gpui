#![cfg_attr(not(test), allow(dead_code))]

pub(crate) mod assertions;
#[cfg(feature = "test-support")]
pub(crate) mod benchmarks;
pub(crate) mod fixtures;
pub(crate) mod headless;
pub(crate) mod ops;
pub(crate) mod topology;

#[cfg(test)]
pub(crate) use assertions::{
    assert_layout_roundtrip_preserves_space_items, assert_root_horizontal_split,
    assert_space_items, assert_unique_items,
};
#[cfg(test)]
pub(crate) use fixtures::load_builtin_scenario_fixtures;
#[cfg(test)]
pub(crate) use headless::DockspaceHeadlessRunner;
#[cfg(test)]
pub(crate) use topology::{
    DockspaceAdapterTag, DockspaceFloatingSpec, DockspaceOracleKind, DockspaceScenario,
    DockspaceScenarioBuildError, DockspaceTopologyNode, DockspaceTopologySpace,
    complex_tabs_split_float_scenario, empty_central_with_floating_scenario,
    three_viewport_rehome_scenario,
};
