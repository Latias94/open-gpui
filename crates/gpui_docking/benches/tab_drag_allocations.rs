use open_gpui_docking::legacy::benchmark_support::{
    DockComplexDockspaceBenchmark, DockDragBenchmark, DockMultiViewDragBenchmark,
    DockspaceBenchmarkCase, DockspaceScenarioSequenceBenchmark,
};
use serde_json::json;
use std::{
    alloc::{GlobalAlloc, Layout, System},
    hint::black_box,
    sync::atomic::{AtomicBool, AtomicU64, Ordering},
};

struct CountingAllocator;

static TRACK_ALLOCATIONS: AtomicBool = AtomicBool::new(false);
static ALLOCATION_COUNT: AtomicU64 = AtomicU64::new(0);
static ALLOCATED_BYTES: AtomicU64 = AtomicU64::new(0);

unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if TRACK_ALLOCATIONS.load(Ordering::Relaxed) {
            ALLOCATION_COUNT.fetch_add(1, Ordering::Relaxed);
            ALLOCATED_BYTES.fetch_add(layout.size() as u64, Ordering::Relaxed);
        }
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        if TRACK_ALLOCATIONS.load(Ordering::Relaxed) {
            ALLOCATION_COUNT.fetch_add(1, Ordering::Relaxed);
            ALLOCATED_BYTES.fetch_add(new_size as u64, Ordering::Relaxed);
        }
        unsafe { System.realloc(ptr, layout, new_size) }
    }
}

#[global_allocator]
static GLOBAL_ALLOCATOR: CountingAllocator = CountingAllocator;

fn main() {
    let small = DockDragBenchmark::new(8, 8);
    let large = DockDragBenchmark::new(64, 8);
    let complex = DockComplexDockspaceBenchmark::tabs_split_float_merge();
    let multiview = DockMultiViewDragBenchmark::new(8, 8, 8);
    let mut dockspace_cycle = DockspaceScenarioSequenceBenchmark::three_dockspace_rehome_cycle();
    let dockspace_cycle_step_count = dockspace_cycle.step_count();
    let dockspace_cycle_workload_contract = dockspace_cycle.workload_contract().to_owned();

    let allocation_iterations = 20_000;
    let (allocation_count, allocated_bytes, all_resolved) = track_allocations(|| {
        run_iterations(allocation_iterations, |iteration| {
            small.resolve_full_move(iteration)
        })
    });
    let (large_allocation_count, large_allocated_bytes, large_all_resolved) =
        track_allocations(|| {
            run_iterations(allocation_iterations, |iteration| {
                large.resolve_full_move(iteration)
            })
        });
    let (complex_allocation_count, complex_allocated_bytes, complex_all_resolved) =
        track_allocations(|| {
            run_iterations(allocation_iterations, |iteration| {
                complex.resolve_complex_move(iteration)
            })
        });
    let (multiview_allocation_count, multiview_allocated_bytes, multiview_all_resolved) =
        track_allocations(|| {
            run_iterations(allocation_iterations, |iteration| {
                multiview.resolve_known_target_move(iteration)
            })
        });
    let (
        multiview_unknown_allocation_count,
        multiview_unknown_allocated_bytes,
        multiview_unknown_all_rejected,
    ) = track_allocations(|| {
        run_iterations(allocation_iterations, |iteration| {
            multiview.reject_unknown_target_move(iteration)
        })
    });
    let sequence_allocation_iterations = 1_000;
    assert!(dockspace_cycle.validate_cycle_state());
    let (sequence_allocation_count, sequence_allocated_bytes, all_sequences_committed) =
        track_allocations(|| {
            run_iterations(sequence_allocation_iterations, |iteration| {
                dockspace_cycle.run_cycle(iteration)
            })
        });
    let sequence_state_restored = dockspace_cycle.validate_cycle_state();
    let hot_paths_zero_allocation = [
        (allocation_count, allocated_bytes),
        (large_allocation_count, large_allocated_bytes),
        (complex_allocation_count, complex_allocated_bytes),
        (multiview_allocation_count, multiview_allocated_bytes),
        (
            multiview_unknown_allocation_count,
            multiview_unknown_allocated_bytes,
        ),
    ]
    .into_iter()
    .all(|(count, bytes)| count == 0 && bytes == 0);

    let small_case = DockspaceBenchmarkCase::retained_tab_grid_small();
    let large_case = DockspaceBenchmarkCase::retained_tab_grid_large();
    let complex_case = DockspaceBenchmarkCase::complex_split_float();
    let multiview_case = DockspaceBenchmarkCase::multiview_known_target();
    let multiview_unknown_case = DockspaceBenchmarkCase::multiview_unknown_target();
    let dockspace_cycle_case = DockspaceBenchmarkCase::three_dockspace_rehome_cycle();
    let open_gpui_allocation_rows = json!([
        allocation_row(
            small_case,
            allocation_count,
            allocated_bytes,
            allocation_iterations,
        ),
        allocation_row(
            large_case,
            large_allocation_count,
            large_allocated_bytes,
            allocation_iterations,
        ),
        allocation_row(
            complex_case,
            complex_allocation_count,
            complex_allocated_bytes,
            allocation_iterations,
        ),
        allocation_row(
            multiview_case,
            multiview_allocation_count,
            multiview_allocated_bytes,
            allocation_iterations,
        ),
        allocation_row(
            multiview_unknown_case,
            multiview_unknown_allocation_count,
            multiview_unknown_allocated_bytes,
            allocation_iterations,
        ),
        {
            "scenario_id": dockspace_cycle_case.scenario_id(),
            "step_id": dockspace_cycle_case.step_id(),
            "operation_family": dockspace_cycle_case.operation_family(),
            "measurement_layer": dockspace_cycle_case.measurement_layer(),
            "scope": dockspace_cycle_case.scope(),
            "open_gpui_allocations_per_operation": sequence_allocation_count as f64 / sequence_allocation_iterations as f64,
            "open_gpui_bytes_per_operation": sequence_allocated_bytes as f64 / sequence_allocation_iterations as f64,
            "operation_step_count": dockspace_cycle_step_count,
            "workload_contract": dockspace_cycle_workload_contract,
        }
    ]);

    println!(
        "{}",
        json!({
            "allocation_benchmark_passed": i32::from(
                all_resolved
                    && large_all_resolved
                    && complex_all_resolved
                    && multiview_all_resolved
                    && multiview_unknown_all_rejected
                    && all_sequences_committed
                    && sequence_state_restored
                    && hot_paths_zero_allocation
            ),
            "open_gpui_hot_paths_zero_allocation": hot_paths_zero_allocation,
            "open_gpui_three_dockspace_cycle_allocations_per_sequence": sequence_allocation_count as f64 / sequence_allocation_iterations as f64,
            "open_gpui_three_dockspace_cycle_bytes_per_sequence": sequence_allocated_bytes as f64 / sequence_allocation_iterations as f64,
            "open_gpui_allocation_rows": open_gpui_allocation_rows,
        })
    );
}

fn allocation_row(
    case: DockspaceBenchmarkCase,
    allocation_count: u64,
    allocated_bytes: u64,
    iterations: usize,
) -> serde_json::Value {
    json!({
        "scenario_id": case.scenario_id(),
        "step_id": case.step_id(),
        "operation_family": case.operation_family(),
        "measurement_layer": case.measurement_layer(),
        "scope": case.scope(),
        "open_gpui_allocations_per_operation": allocation_count as f64 / iterations as f64,
        "open_gpui_bytes_per_operation": allocated_bytes as f64 / iterations as f64,
    })
}

fn run_iterations(iterations: usize, mut operation: impl FnMut(usize) -> bool) -> bool {
    let mut all_succeeded = true;
    for iteration in 0..iterations {
        all_succeeded &= black_box(operation(iteration));
    }
    all_succeeded
}

fn track_allocations<T>(operation: impl FnOnce() -> T) -> (u64, u64, T) {
    ALLOCATION_COUNT.store(0, Ordering::Relaxed);
    ALLOCATED_BYTES.store(0, Ordering::Relaxed);
    TRACK_ALLOCATIONS.store(true, Ordering::SeqCst);
    let result = operation();
    TRACK_ALLOCATIONS.store(false, Ordering::SeqCst);
    (
        ALLOCATION_COUNT.load(Ordering::Relaxed),
        ALLOCATED_BYTES.load(Ordering::Relaxed),
        result,
    )
}
