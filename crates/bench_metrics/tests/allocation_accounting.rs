//! Exercises allocator forwarding directly so harness allocations do not affect totals.

use open_gpui_bench_metrics::{
    AllocationCounter, AllocationMetric, AllocationStats, CountingAllocator, MetricScope,
    allocation_stats,
};
use std::{
    alloc::{GlobalAlloc, Layout, System},
    cell::Cell,
};

struct FallibleAllocator<'a> {
    fail: &'a Cell<bool>,
}

// SAFETY: successful requests are forwarded to System unchanged; failures leave allocations intact.
unsafe impl GlobalAlloc for FallibleAllocator<'_> {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if self.fail.get() {
            std::ptr::null_mut()
        } else {
            // SAFETY: the caller supplies a valid allocation layout.
            unsafe { System.alloc(layout) }
        }
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        if self.fail.get() {
            std::ptr::null_mut()
        } else {
            // SAFETY: the caller supplies a valid allocation layout.
            unsafe { System.alloc_zeroed(layout) }
        }
    }

    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        // SAFETY: the caller supplies a live System allocation and its layout.
        unsafe { System.dealloc(pointer, layout) };
    }

    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        if self.fail.get() {
            std::ptr::null_mut()
        } else {
            // SAFETY: the caller supplies a live System allocation and a valid replacement size.
            unsafe { System.realloc(pointer, layout, new_size) }
        }
    }
}

#[test]
fn allocation_failures_preserve_totals_and_reallocations_account_for_both_sides() {
    use criterion::measurement::Measurement;

    let fail = Cell::new(true);
    let allocator = CountingAllocator::new(FallibleAllocator { fail: &fail });
    let original_layout = Layout::from_size_align(16, 8).expect("valid allocation layout");
    let grown_layout = Layout::from_size_align(64, 8).expect("valid allocation layout");
    let shrunk_layout = Layout::from_size_align(8, 8).expect("valid allocation layout");
    let zeroed_layout = Layout::from_size_align(24, 8).expect("valid allocation layout");

    assert_eq!(allocation_stats(), None);
    // SAFETY: both layouts are nonzero and valid; null means no allocation was created.
    unsafe {
        assert!(allocator.alloc(original_layout).is_null());
        assert!(allocator.alloc_zeroed(zeroed_layout).is_null());
    }
    assert_eq!(allocation_stats(), None);

    fail.set(false);
    // SAFETY: original_layout is nonzero and valid.
    let original = unsafe { allocator.alloc(original_layout) };
    assert!(!original.is_null());
    // SAFETY: original holds 16 writable bytes from the successful allocation.
    unsafe { original.write_bytes(0xa5, original_layout.size()) };
    let before_failure = allocation_stats().expect("the counting allocator served a request");
    assert_eq!(
        before_failure,
        AllocationStats {
            allocations: 1,
            allocated_bytes: 16,
            freed_bytes: 0,
        }
    );
    assert_eq!(before_failure.live_bytes(), 16);
    let allocations =
        AllocationCounter::new(AllocationMetric::Allocations, MetricScope::CallingThread)
            .expect("the counting allocator served a request");
    let bytes =
        AllocationCounter::new(AllocationMetric::AllocatedBytes, MetricScope::CallingThread)
            .expect("the counting allocator served a request");
    let allocations_start = allocations.start();
    let bytes_start = bytes.start();

    fail.set(true);
    // SAFETY: original is live with original_layout; a null result leaves it live.
    let failed = unsafe { allocator.realloc(original, original_layout, grown_layout.size()) };
    assert!(failed.is_null());
    assert_eq!(allocation_stats(), Some(before_failure));
    assert_eq!(allocations.end(allocations_start), 0.0);
    assert_eq!(bytes.end(bytes_start), 0.0);
    // SAFETY: the failed reallocation preserved all 16 initialized bytes of original.
    assert_eq!(
        unsafe { std::slice::from_raw_parts(original, 16) },
        &[0xa5; 16]
    );

    fail.set(false);
    // SAFETY: original is live with original_layout and the requested size is valid.
    let grown = unsafe { allocator.realloc(original, original_layout, grown_layout.size()) };
    assert!(!grown.is_null());
    assert_eq!(
        allocation_stats(),
        Some(AllocationStats {
            allocations: 2,
            allocated_bytes: 80,
            freed_bytes: 16,
        })
    );
    // SAFETY: successful growth preserved the first 16 initialized bytes.
    assert_eq!(
        unsafe { std::slice::from_raw_parts(grown, 16) },
        &[0xa5; 16]
    );

    // SAFETY: grown is live with grown_layout and the requested size is valid.
    let shrunk = unsafe { allocator.realloc(grown, grown_layout, shrunk_layout.size()) };
    assert!(!shrunk.is_null());
    // SAFETY: successful shrinking preserved the first eight initialized bytes.
    assert_eq!(unsafe { std::slice::from_raw_parts(shrunk, 8) }, &[0xa5; 8]);
    assert_eq!(
        allocation_stats(),
        Some(AllocationStats {
            allocations: 3,
            allocated_bytes: 88,
            freed_bytes: 80,
        })
    );

    // SAFETY: zeroed_layout is nonzero and valid.
    let zeroed = unsafe { allocator.alloc_zeroed(zeroed_layout) };
    assert!(!zeroed.is_null());
    // SAFETY: successful alloc_zeroed initialized all 24 bytes to zero.
    assert_eq!(unsafe { std::slice::from_raw_parts(zeroed, 24) }, &[0; 24]);
    let before_failure = allocation_stats().expect("successful requests are counted");
    assert_eq!(
        before_failure,
        AllocationStats {
            allocations: 4,
            allocated_bytes: 112,
            freed_bytes: 80,
        }
    );
    assert_eq!(before_failure.live_bytes(), 32);
    assert_eq!(allocations.end(allocations_start), 3.0);
    assert_eq!(bytes.end(bytes_start), 96.0);

    fail.set(true);
    // SAFETY: both layouts are valid; failed allocations do not create blocks to free.
    unsafe {
        assert!(allocator.alloc(original_layout).is_null());
        assert!(allocator.alloc_zeroed(zeroed_layout).is_null());
    }
    assert_eq!(allocation_stats(), Some(before_failure));

    // SAFETY: each pointer is live with its matching layout and is freed exactly once.
    unsafe {
        allocator.dealloc(shrunk, shrunk_layout);
        allocator.dealloc(zeroed, zeroed_layout);
    }
    let final_stats = allocation_stats().expect("successful requests are counted");
    assert_eq!(
        final_stats,
        AllocationStats {
            allocations: 4,
            allocated_bytes: 112,
            freed_bytes: 112,
        }
    );
    assert_eq!(final_stats.live_bytes(), 0);
}
