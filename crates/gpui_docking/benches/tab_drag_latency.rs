use open_gpui_docking::benchmark_support::{
    DockComplexDockspaceBenchmark, DockDragBenchmark, DockMultiViewDragBenchmark,
    DockspaceBenchmarkCase,
};
use serde_json::json;
use std::{
    alloc::{GlobalAlloc, Layout, System},
    hint::black_box,
    sync::atomic::{AtomicBool, AtomicU64, Ordering},
    time::Instant,
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
    let allocation_iterations = 20_000;
    let (allocation_count, allocated_bytes, all_resolved) = track_allocations(|| {
        let mut all_resolved = true;
        for iteration in 0..allocation_iterations {
            all_resolved &= black_box(small.resolve_full_move(iteration));
        }
        all_resolved
    });
    let (complex_allocation_count, complex_allocated_bytes, complex_all_resolved) =
        track_allocations(|| {
            let mut all_resolved = true;
            for iteration in 0..allocation_iterations {
                all_resolved &= black_box(complex.resolve_complex_move(iteration));
            }
            all_resolved
        });
    let (multiview_allocation_count, multiview_allocated_bytes, multiview_all_resolved) =
        track_allocations(|| {
            let mut all_resolved = true;
            for iteration in 0..allocation_iterations {
                all_resolved &= black_box(multiview.resolve_known_target_move(iteration));
            }
            all_resolved
        });
    let (
        multiview_unknown_allocation_count,
        multiview_unknown_allocated_bytes,
        multiview_unknown_all_rejected,
    ) = track_allocations(|| {
        let mut all_rejected = true;
        for iteration in 0..allocation_iterations {
            all_rejected &= black_box(multiview.reject_unknown_target_move(iteration));
        }
        all_rejected
    });

    let small_case = DockspaceBenchmarkCase::retained_tab_grid_small();
    let large_case = DockspaceBenchmarkCase::retained_tab_grid_large();
    let complex_case = DockspaceBenchmarkCase::complex_split_float();
    let multiview_case = DockspaceBenchmarkCase::multiview_known_target();
    let multiview_unknown_case = DockspaceBenchmarkCase::multiview_unknown_target();
    let open_gpui_scenario_rows = json!([
        {
            "scenario_id": small_case.scenario_id(),
            "measurement_layer": small_case.measurement_layer(),
            "operation": small_case.operation(),
            "open_gpui_ns_per_operation": open_gpui_ns_per_move,
            "open_gpui_allocations_per_operation": allocation_count as f64 / allocation_iterations as f64,
            "open_gpui_bytes_per_operation": allocated_bytes as f64 / allocation_iterations as f64,
        },
        {
            "scenario_id": large_case.scenario_id(),
            "measurement_layer": large_case.measurement_layer(),
            "operation": large_case.operation(),
            "open_gpui_ns_per_operation": open_gpui_large_scene_ns_per_move,
        },
        {
            "scenario_id": complex_case.scenario_id(),
            "measurement_layer": complex_case.measurement_layer(),
            "operation": complex_case.operation(),
            "open_gpui_ns_per_operation": open_gpui_complex_scene_ns_per_move,
            "open_gpui_allocations_per_operation": complex_allocation_count as f64 / allocation_iterations as f64,
            "open_gpui_bytes_per_operation": complex_allocated_bytes as f64 / allocation_iterations as f64,
        },
        {
            "scenario_id": multiview_case.scenario_id(),
            "measurement_layer": multiview_case.measurement_layer(),
            "operation": multiview_case.operation(),
            "open_gpui_ns_per_operation": open_gpui_multiview_known_target_ns_per_move,
            "open_gpui_allocations_per_operation": multiview_allocation_count as f64 / allocation_iterations as f64,
            "open_gpui_bytes_per_operation": multiview_allocated_bytes as f64 / allocation_iterations as f64,
        },
        {
            "scenario_id": multiview_unknown_case.scenario_id(),
            "measurement_layer": multiview_unknown_case.measurement_layer(),
            "operation": multiview_unknown_case.operation(),
            "open_gpui_ns_per_operation": open_gpui_multiview_unknown_target_ns_per_move,
            "open_gpui_allocations_per_operation": multiview_unknown_allocation_count as f64 / allocation_iterations as f64,
            "open_gpui_bytes_per_operation": multiview_unknown_allocated_bytes as f64 / allocation_iterations as f64,
        }
    ]);

    println!(
        "{}",
        json!({
            "benchmark_passed": i32::from(all_resolved && complex_all_resolved && multiview_all_resolved && multiview_unknown_all_rejected),
            "open_gpui_ns_per_move": open_gpui_ns_per_move,
            "open_gpui_large_scene_ns_per_move": open_gpui_large_scene_ns_per_move,
            "open_gpui_complex_scene_ns_per_move": open_gpui_complex_scene_ns_per_move,
            "open_gpui_allocations_per_move": allocation_count as f64 / allocation_iterations as f64,
            "open_gpui_bytes_per_move": allocated_bytes as f64 / allocation_iterations as f64,
            "open_gpui_complex_scene_allocations_per_move": complex_allocation_count as f64 / allocation_iterations as f64,
            "open_gpui_complex_scene_bytes_per_move": complex_allocated_bytes as f64 / allocation_iterations as f64,
            "open_gpui_multiview_known_target_ns_per_move": open_gpui_multiview_known_target_ns_per_move,
            "open_gpui_multiview_known_target_allocations_per_move": multiview_allocation_count as f64 / allocation_iterations as f64,
            "open_gpui_multiview_known_target_bytes_per_move": multiview_allocated_bytes as f64 / allocation_iterations as f64,
            "open_gpui_multiview_unknown_target_ns_per_move": open_gpui_multiview_unknown_target_ns_per_move,
            "open_gpui_multiview_unknown_target_allocations_per_move": multiview_unknown_allocation_count as f64 / allocation_iterations as f64,
            "open_gpui_multiview_unknown_target_bytes_per_move": multiview_unknown_allocated_bytes as f64 / allocation_iterations as f64,
            "open_gpui_scenario_rows": open_gpui_scenario_rows,
        })
    );
}

fn median_ns_per_operation(
    iterations: usize,
    samples: usize,
    mut operation: impl FnMut(usize) -> bool,
) -> f64 {
    for iteration in 0..10_000 {
        black_box(operation(iteration));
    }

    let mut measurements = Vec::with_capacity(samples);
    for sample in 0..samples {
        let started = Instant::now();
        let mut all_resolved = true;
        for iteration in 0..iterations {
            all_resolved &= black_box(operation(iteration + sample * iterations));
        }
        assert!(
            all_resolved,
            "the benchmark fixture expectation must hold for every pointer move"
        );
        measurements.push(started.elapsed().as_nanos() as f64 / iterations as f64);
    }
    measurements.sort_by(f64::total_cmp);
    measurements[measurements.len() / 2]
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
