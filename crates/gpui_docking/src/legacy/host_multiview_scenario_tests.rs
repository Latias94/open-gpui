use crate::legacy::{
    DockActionOutcome,
    dockspace_testkit::{DockspaceGpuiHostRunner, load_builtin_scenario_fixtures},
};
use open_gpui::TestAppContext;

fn assert_gpui_host_scenario_matches_headless(cx: &mut TestAppContext, scenario_id: &str) {
    let scenarios =
        load_builtin_scenario_fixtures().expect("builtin dockspace fixtures should parse");
    let scenario = scenarios
        .iter()
        .find(|scenario| scenario.id == scenario_id)
        .unwrap_or_else(|| panic!("dockspace scenario {scenario_id} should exist"));

    let runner = DockspaceGpuiHostRunner::from_scenario(cx, scenario)
        .expect("GPUI host scenario adapter should build");
    let outcomes = runner
        .run_all(cx, scenario)
        .expect("GPUI host scenario should commit through real pointer drags");

    assert!(
        outcomes
            .iter()
            .all(|outcome| *outcome == DockActionOutcome::Changed),
        "every GPUI host scenario step should mutate the production graph"
    );
    assert_eq!(outcomes.len(), scenario.steps.len());
}

#[open_gpui::test]
fn gpui_host_adapter_executes_the_three_viewport_fixture_with_real_drag_input(
    cx: &mut TestAppContext,
) {
    assert_gpui_host_scenario_matches_headless(cx, "three_viewport_rehome");
}

#[open_gpui::test]
fn gpui_host_adapter_executes_the_eight_step_rehome_cycle_with_real_drag_input(
    cx: &mut TestAppContext,
) {
    assert_gpui_host_scenario_matches_headless(cx, "three_dockspace_rehome_cycle");
}
