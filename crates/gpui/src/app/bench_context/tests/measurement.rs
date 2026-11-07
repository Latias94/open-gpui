use super::*;
use open_gpui_bench_metrics::FakeCounter;
use std::{cell::Cell, sync::atomic::AtomicU64};

struct DropCounter {
    counter: Arc<AtomicU64>,
    drops: Rc<Cell<u64>>,
}

impl Drop for DropCounter {
    fn drop(&mut self) {
        assert!(!profiler::trace_enabled());
        self.counter.fetch_add(1_000, Ordering::SeqCst);
        self.drops.set(self.drops.get() + 1);
    }
}

#[test]
fn batched_measurement_excludes_setup_and_input_output_destruction() {
    let counter = Arc::new(AtomicU64::new(0));
    let drops = Rc::new(Cell::new(0));
    let iterations = Cell::new(0);
    let report = BenchReport::default();
    let mut criterion = criterion::Criterion::default()
        .with_measurement(
            BenchMeasurement::new(FakeCounter::new(counter.clone()))
                .with_secondary("measured ticks", FakeCounter::new(counter.clone())),
        )
        .without_plots()
        .sample_size(10)
        .warm_up_time(Duration::from_millis(1))
        .measurement_time(Duration::from_millis(1));
    criterion.bench_function("batched-task-measurement-boundaries", |bencher| {
        let platform = bench_platform(None, Arc::new(crate::NoopTextSystem::new()));
        let mut cx =
            BenchAppContext::new_with_platform_and_report(platform, None, bencher, report.clone());
        cx.bench_batched_task(
            |_| {
                assert!(!profiler::trace_enabled());
                counter.fetch_add(100_000, Ordering::SeqCst);
                DropCounter {
                    counter: counter.clone(),
                    drops: drops.clone(),
                }
            },
            |_, cx| {
                assert!(profiler::trace_enabled());
                counter.fetch_add(11, Ordering::SeqCst);
                iterations.set(iterations.get() + 1);
                let output = DropCounter {
                    counter: counter.clone(),
                    drops: drops.clone(),
                };
                cx.foreground_executor().spawn(async move { output })
            },
        );
        cx.teardown();
    });
    assert!(iterations.get() > 0);
    assert_eq!(drops.get(), 2 * iterations.get());
    let values = report
        .metrics()
        .values("measured ticks")
        .expect("recorded ticks");
    assert!(!values.is_empty());
    assert!(values.iter().all(|value| *value == 11.0), "{values:?}");
}
