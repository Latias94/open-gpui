use super::*;
use open_gpui_core_util::defer;
use std::cell::Cell;

#[test]
fn criterion_profiles_exclude_setup_and_keep_nested_scope_ownership() {
    profiler::set_trace_enabled(false);
    let _restore_trace = defer(|| {
        profiler::set_trace_enabled(false);
    });
    let iterations = Cell::new(0);
    let report = run_benchmark("nested-profile-successor", |mut app| {
        let mut window = app.add_empty_window();
        window.update(|window, cx| window.draw(cx).clear(cx));
        app.run_until_idle();
        app.bench_iter(|app| {
            assert!(profiler::trace_enabled());
            let scope = TraceScope::start(app.foreground_journal_collector());
            window.update(|window, cx| window.draw(cx).clear(cx));
            let result = 42;
            let events = scope.finish();
            assert_eq!(result, 42);
            assert_eq!(
                events
                    .frame_events
                    .iter()
                    .filter(|event| matches!(event, FrameEvent::Draw(_)))
                    .count(),
                1
            );
            assert!(profiler::trace_enabled());
            window.update(|window, cx| window.draw(cx).clear(cx));
            iterations.set(iterations.get() + 1);
        });
        assert!(!profiler::trace_enabled());
        drop(window);
        app.teardown();
    });
    assert!(iterations.get() > 0);
    assert_eq!(
        report.frame_snapshot.borrow().draw.len(),
        2 * iterations.get()
    );
}

#[test]
fn criterion_profile_unwind_preserves_manual_trace_setting() {
    let _restore_trace = defer(|| {
        profiler::set_trace_enabled(false);
    });
    for enabled in [true, false] {
        profiler::set_trace_enabled(enabled);
        let message = benchmark_panic("unwind-profile-successor", |app| {
            let panic = catch_unwind(AssertUnwindSafe(|| {
                app.bench_iter(|_| panic!("injected benchmark failure"));
            }));
            assert_eq!(profiler::trace_enabled(), enabled);
            resume_unwind(panic.expect_err("the measurement must unwind"));
        });
        assert_eq!(message, "injected benchmark failure");
        assert_eq!(profiler::trace_enabled(), enabled);
    }
}

#[test]
fn cloned_context_cannot_reenter_a_running_criterion_measurement() {
    run_benchmark("nested-bencher-admission", |mut app| {
        let mut clone = app.clone();
        app.bench_iter(|_| {
            let panic = catch_unwind(AssertUnwindSafe(|| clone.bench_iter(|_| ())));
            let panic = panic.expect_err("the active Bencher must have one owner");
            let message = panic
                .downcast_ref::<String>()
                .expect("the reentrancy diagnostic must be a formatted string");
            assert!(message.contains("benchmark measurement is already running"));
            assert!(profiler::trace_enabled());
        });
        drop(clone);
        app.teardown();
    });
}

struct TraceAtDrop(Rc<Cell<Option<bool>>>);

impl Drop for TraceAtDrop {
    fn drop(&mut self) {
        self.0.set(Some(profiler::trace_enabled()));
    }
}

#[test]
fn panicking_batched_work_drops_its_input_after_the_trace_scope() {
    profiler::set_trace_enabled(false);
    let trace_at_drop = Rc::new(Cell::new(None));
    let message = benchmark_panic("batched-input-unwind-scope", |app| {
        app.bench_batched_task(
            |_| TraceAtDrop(trace_at_drop.clone()),
            |_, _| -> Task<()> { panic!("injected batched work failure") },
        );
    });
    assert_eq!(message, "injected batched work failure");
    assert_eq!(trace_at_drop.get(), Some(false));
    assert!(!profiler::trace_enabled());
}
