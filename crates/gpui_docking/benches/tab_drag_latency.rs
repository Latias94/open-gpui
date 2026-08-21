use open_gpui_docking::benchmark_support::{
    DockComplexDockspaceBenchmark, DockDragBenchmark, DockMultiViewDragBenchmark,
    DockspaceBenchmarkCase, DockspaceScenarioSequenceBenchmark,
};
use serde_json::json;
use std::{hint::black_box, time::Instant};

fn main() {
    let small = DockDragBenchmark::new(8, 8);
    let large = DockDragBenchmark::new(64, 8);
    let complex = DockComplexDockspaceBenchmark::tabs_split_float_merge();
    let multiview = DockMultiViewDragBenchmark::new(8, 8, 8);
    let mut dockspace_cycle = DockspaceScenarioSequenceBenchmark::three_dockspace_rehome_cycle();
    let dockspace_cycle_step_count = dockspace_cycle.step_count();
    let dockspace_cycle_workload_contract = dockspace_cycle.workload_contract().to_owned();

    let open_gpui_ns_per_move =
        median_ns_per_operation(100_000, 7, |iteration| small.resolve_full_move(iteration));
    let open_gpui_large_scene_ns_per_move =
        median_ns_per_operation(20_000, 7, |iteration| large.resolve_full_move(iteration));
    let open_gpui_complex_scene_ns_per_move = median_ns_per_operation(100_000, 7, |iteration| {
        complex.resolve_complex_move(iteration)
    });
    let open_gpui_multiview_known_target_ns_per_move =
        median_ns_per_operation(100_000, 7, |iteration| {
            multiview.resolve_known_target_move(iteration)
        });
    let open_gpui_multiview_unknown_target_ns_per_move =
        median_ns_per_operation(100_000, 7, |iteration| {
            multiview.reject_unknown_target_move(iteration)
        });
    let open_gpui_three_dockspace_cycle_ns_per_sequence =
        median_ns_per_scenario_sequence(2_000, 7, &mut dockspace_cycle);

    let small_case = DockspaceBenchmarkCase::retained_tab_grid_small();
    let large_case = DockspaceBenchmarkCase::retained_tab_grid_large();
    let complex_case = DockspaceBenchmarkCase::complex_split_float();
    let multiview_case = DockspaceBenchmarkCase::multiview_known_target();
    let multiview_unknown_case = DockspaceBenchmarkCase::multiview_unknown_target();
    let dockspace_cycle_case = DockspaceBenchmarkCase::three_dockspace_rehome_cycle();
    let open_gpui_scenario_rows = json!([
        {
            "scenario_id": small_case.scenario_id(),
            "step_id": small_case.step_id(),
            "operation_family": small_case.operation_family(),
            "measurement_layer": small_case.measurement_layer(),
            "scope": small_case.scope(),
            "open_gpui_ns_per_operation": open_gpui_ns_per_move,
        },
        {
            "scenario_id": large_case.scenario_id(),
            "step_id": large_case.step_id(),
            "operation_family": large_case.operation_family(),
            "measurement_layer": large_case.measurement_layer(),
            "scope": large_case.scope(),
            "open_gpui_ns_per_operation": open_gpui_large_scene_ns_per_move,
        },
        {
            "scenario_id": complex_case.scenario_id(),
            "step_id": complex_case.step_id(),
            "operation_family": complex_case.operation_family(),
            "measurement_layer": complex_case.measurement_layer(),
            "scope": complex_case.scope(),
            "open_gpui_ns_per_operation": open_gpui_complex_scene_ns_per_move,
        },
        {
            "scenario_id": multiview_case.scenario_id(),
            "step_id": multiview_case.step_id(),
            "operation_family": multiview_case.operation_family(),
            "measurement_layer": multiview_case.measurement_layer(),
            "scope": multiview_case.scope(),
            "open_gpui_ns_per_operation": open_gpui_multiview_known_target_ns_per_move,
        },
        {
            "scenario_id": multiview_unknown_case.scenario_id(),
            "step_id": multiview_unknown_case.step_id(),
            "operation_family": multiview_unknown_case.operation_family(),
            "measurement_layer": multiview_unknown_case.measurement_layer(),
            "scope": multiview_unknown_case.scope(),
            "open_gpui_ns_per_operation": open_gpui_multiview_unknown_target_ns_per_move,
        },
        {
            "scenario_id": dockspace_cycle_case.scenario_id(),
            "step_id": dockspace_cycle_case.step_id(),
            "operation_family": dockspace_cycle_case.operation_family(),
            "measurement_layer": dockspace_cycle_case.measurement_layer(),
            "scope": dockspace_cycle_case.scope(),
            "open_gpui_ns_per_operation": open_gpui_three_dockspace_cycle_ns_per_sequence,
            "open_gpui_ns_per_step": open_gpui_three_dockspace_cycle_ns_per_sequence / dockspace_cycle_step_count as f64,
            "operation_step_count": dockspace_cycle_step_count,
            "workload_contract": dockspace_cycle_workload_contract,
        }
    ]);

    println!(
        "{}",
        json!({
            "benchmark_passed": i32::from(dockspace_cycle.validate_cycle_state()),
            "open_gpui_ns_per_move": open_gpui_ns_per_move,
            "open_gpui_large_scene_ns_per_move": open_gpui_large_scene_ns_per_move,
            "open_gpui_complex_scene_ns_per_move": open_gpui_complex_scene_ns_per_move,
            "open_gpui_multiview_known_target_ns_per_move": open_gpui_multiview_known_target_ns_per_move,
            "open_gpui_multiview_unknown_target_ns_per_move": open_gpui_multiview_unknown_target_ns_per_move,
            "open_gpui_three_dockspace_cycle_ns_per_sequence": open_gpui_three_dockspace_cycle_ns_per_sequence,
            "open_gpui_three_dockspace_cycle_ns_per_step": open_gpui_three_dockspace_cycle_ns_per_sequence / dockspace_cycle_step_count as f64,
            "open_gpui_scenario_rows": open_gpui_scenario_rows,
        })
    );
}

fn median_ns_per_operation(
    iterations: usize,
    samples: usize,
    mut operation: impl FnMut(usize) -> bool,
) -> f64 {
    for iteration in 0..iterations.min(10_000) {
        black_box(operation(iteration));
    }

    let mut measurements = Vec::with_capacity(samples);
    for sample in 0..samples {
        let started = Instant::now();
        let mut all_resolved = true;
        for iteration in 0..iterations {
            all_resolved &= black_box(operation(iteration + sample * iterations));
        }
        let elapsed = started.elapsed();
        assert!(
            all_resolved,
            "the benchmark fixture expectation must hold for every pointer move"
        );
        measurements.push(elapsed.as_nanos() as f64 / iterations as f64);
    }
    measurements.sort_by(f64::total_cmp);
    measurements[measurements.len() / 2]
}

fn median_ns_per_scenario_sequence(
    iterations: usize,
    samples: usize,
    benchmark: &mut DockspaceScenarioSequenceBenchmark,
) -> f64 {
    for iteration in 0..iterations.min(2_000) {
        assert!(benchmark.run_cycle(iteration));
    }
    assert!(
        benchmark.validate_cycle_state(),
        "the warmup must restore the exact initial dockspace topology"
    );

    let mut measurements = Vec::with_capacity(samples);
    for sample in 0..samples {
        assert!(
            benchmark.validate_cycle_state(),
            "each sample must start from the exact initial dockspace topology"
        );
        let started = Instant::now();
        let mut all_committed = true;
        for iteration in 0..iterations {
            all_committed &= black_box(benchmark.run_cycle(iteration + sample * iterations));
        }
        let elapsed = started.elapsed();
        assert!(
            all_committed,
            "every named-item docking transaction must commit"
        );
        assert!(
            benchmark.validate_cycle_state(),
            "each sample must restore the exact initial dockspace topology"
        );
        measurements.push(elapsed.as_nanos() as f64 / iterations as f64);
    }
    measurements.sort_by(f64::total_cmp);
    measurements[measurements.len() / 2]
}
