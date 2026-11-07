use super::*;
use std::panic::{AssertUnwindSafe, catch_unwind, resume_unwind};

mod frames;
mod lifecycle;
#[cfg(feature = "test-support")]
mod measurement;
mod official;
mod profiling;
mod renderer;
mod report;

fn criterion() -> criterion::Criterion<BenchMeasurement> {
    criterion::Criterion::default()
        .with_measurement(BenchMeasurement::new(criterion::measurement::WallTime))
        .without_plots()
        .sample_size(10)
        .warm_up_time(Duration::from_millis(1))
        .measurement_time(Duration::from_millis(1))
}

fn run_benchmark(
    name: &'static str,
    mut benchmark: impl FnMut(BenchAppContext<'_, '_>),
) -> BenchReport {
    let report = BenchReport::default();
    criterion().bench_function(name, |bencher| {
        let platform = bench_platform(None, Arc::new(crate::NoopTextSystem::new()));
        benchmark(BenchAppContext::new_with_platform_and_report(
            platform,
            Some(name),
            bencher,
            report.clone(),
        ));
    });
    report
}

fn benchmark_panic(
    name: &'static str,
    mut benchmark: impl FnMut(&mut BenchAppContext<'_, '_>),
) -> String {
    let panic = catch_unwind(AssertUnwindSafe(|| {
        criterion().bench_function(name, |bencher| {
            let platform = bench_platform(None, Arc::new(crate::NoopTextSystem::new()));
            let mut cx = BenchAppContext::new(platform, Some(name), bencher);
            let panic = catch_unwind(AssertUnwindSafe(|| benchmark(&mut cx)));
            cx.teardown();
            resume_unwind(panic.expect_err("the benchmark must panic"));
        });
    }))
    .expect_err("the benchmark panic must reach the caller");
    panic
        .downcast_ref::<&str>()
        .copied()
        .or_else(|| panic.downcast_ref::<String>().map(String::as_str))
        .expect("benchmark panic must contain a message")
        .to_owned()
}

fn platform_window(cx: &BenchAppContext<'_, '_>, window: AnyWindowHandle) -> crate::TestWindow {
    let mut app = cx.app.borrow_mut();
    app.windows
        .get_mut(window.window_id())
        .and_then(Option::as_deref_mut)
        .expect("benchmark window must remain registered")
        .platform_window
        .as_test()
        .expect("benchmark window must use TestPlatform")
        .clone()
}

fn run_task_to_completion<Output>(
    foreground_executor: &ForegroundExecutor,
    task: Task<Output>,
) -> Output
where
    Output: 'static,
{
    let output = Rc::new(RefCell::new(None));
    foreground_executor
        .spawn({
            let output = output.clone();
            async move {
                *output.borrow_mut() = Some(task.await);
            }
        })
        .detach();

    foreground_executor
        .dispatcher()
        .as_threaded()
        .expect("BenchAppContext requires a ThreadedDispatcher")
        .run_until(|| output.borrow_mut().take())
}
