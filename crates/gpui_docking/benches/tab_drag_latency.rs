use open_gpui_docking::benchmark_support::DockDragBenchmark;
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

    let open_gpui_ns_per_move =
        median_ns_per_operation(100_000, 7, |iteration| small.resolve_full_move(iteration));
    let open_gpui_large_scene_ns_per_move =
        median_ns_per_operation(20_000, 7, |iteration| large.resolve_full_move(iteration));
    let allocation_iterations = 20_000;
    let (allocation_count, allocated_bytes, all_resolved) = track_allocations(|| {
        let mut all_resolved = true;
        for iteration in 0..allocation_iterations {
            all_resolved &= black_box(small.resolve_full_move(iteration));
        }
        all_resolved
    });

    println!(
        "{}",
        json!({
            "benchmark_passed": i32::from(all_resolved),
            "open_gpui_ns_per_move": open_gpui_ns_per_move,
            "open_gpui_large_scene_ns_per_move": open_gpui_large_scene_ns_per_move,
            "open_gpui_allocations_per_move": allocation_count as f64 / allocation_iterations as f64,
            "open_gpui_bytes_per_move": allocated_bytes as f64 / allocation_iterations as f64,
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
            "the benchmark fixture must resolve every pointer move"
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
