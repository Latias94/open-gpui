//! End-to-end allocation counts for retained shared-session tab drags.

mod drag_workload;

use std::{
    alloc::{GlobalAlloc, Layout, System},
    sync::atomic::{AtomicBool, AtomicU64, Ordering},
};

use drag_workload::{DragWorkload, report};
use serde_json::json;

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

fn main() -> anyhow::Result<()> {
    const CYCLES: usize = 30;
    let mut workload = DragWorkload::new()?;
    for _ in 0..5 {
        workload.cycle()?;
    }
    TRACK_ALLOCATIONS.store(true, Ordering::Relaxed);
    let result: anyhow::Result<()> = (0..CYCLES).try_for_each(|_| workload.cycle());
    TRACK_ALLOCATIONS.store(false, Ordering::Relaxed);
    result?;
    println!(
        "{}",
        report(
            CYCLES,
            json!({
                "allocations_per_cycle": ALLOCATION_COUNT.load(Ordering::Relaxed) as f64 / CYCLES as f64,
                "allocated_bytes_per_cycle": ALLOCATED_BYTES.load(Ordering::Relaxed) as f64 / CYCLES as f64,
            })
        )
    );
    Ok(())
}
