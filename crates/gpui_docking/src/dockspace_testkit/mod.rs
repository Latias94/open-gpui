#![cfg_attr(not(test), allow(dead_code))]

pub(crate) mod assertions;
#[cfg(any(test, feature = "test-support"))]
pub(crate) mod benchmarks;
pub(crate) mod canonical_headless;
pub(crate) mod contract;
pub(crate) mod fixtures;
#[cfg(test)]
pub(crate) mod gpui_host;
pub(crate) mod headless;
pub(crate) mod ops;
pub(crate) mod topology;

#[cfg(test)]
pub(crate) use assertions::{
    DockspaceCheckpointAssertionError, assert_canonical_checkpoint, assert_canonical_edge_locality,
    assert_headless_checkpoint, assert_layout_roundtrip_preserves_space_items,
    assert_root_horizontal_split, assert_space_items, assert_unique_items,
    capture_canonical_edge_target,
};
#[cfg(test)]
pub(crate) use canonical_headless::{
    CanonicalDockspaceHeadlessRunner, CanonicalDockspaceScenarioRunError,
};
pub(crate) use contract::{DockspaceAdapterStepSupport, DockspaceAdapterTag};
#[cfg(test)]
pub(crate) use fixtures::load_builtin_scenario_fixtures;
#[cfg(test)]
pub(crate) use gpui_host::DockspaceGpuiHostRunner;
#[cfg(test)]
pub(crate) use headless::DockspaceHeadlessRunner;
#[cfg(test)]
pub(crate) use topology::{
    DockspaceFloatingSpec, DockspaceOracleKind, DockspaceScenario, DockspaceScenarioBuildError,
    DockspaceTopologyNode, DockspaceTopologySpace,
};
