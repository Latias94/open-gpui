use criterion::measurement::Measurement;
use open_gpui_bench_metrics::{
    AllocationCounter, AllocationMetric, BENCH_MEASUREMENT_ENV_VAR, BenchMeasurement, MetricReport,
    MetricScope, allocation_stats,
};
#[cfg(not(any(target_os = "linux", target_os = "macos")))]
use open_gpui_bench_metrics::{HardwareCounter, HardwareEvent};
#[cfg(not(target_os = "linux"))]
use open_gpui_bench_metrics::{ResourceCounter, ResourceMetric};

#[test]
fn allocation_counters_require_a_counting_allocator() {
    assert_eq!(allocation_stats(), None);
    for scope in [MetricScope::Process, MetricScope::CallingThread] {
        for metric in [
            AllocationMetric::Allocations,
            AllocationMetric::AllocatedBytes,
        ] {
            let error = AllocationCounter::new(metric, scope)
                .err()
                .expect("allocation counters require CountingAllocator");
            assert!(error.to_string().contains("CountingAllocator"));
        }
    }
    assert_eq!(allocation_stats(), None);
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
#[test]
fn unsupported_hardware_events_and_scopes_fail_explicitly() {
    for scope in [MetricScope::Process, MetricScope::CallingThread] {
        for event in [
            HardwareEvent::Instructions,
            HardwareEvent::Cycles,
            HardwareEvent::BranchMisses,
            HardwareEvent::CacheMisses,
            HardwareEvent::CacheReferences,
        ] {
            let error = HardwareCounter::new(event, scope)
                .err()
                .expect("this platform has no hardware counter backend");
            assert!(error.to_string().contains("unavailable on this platform"));
        }
    }
}

#[cfg(not(unix))]
#[test]
fn unsupported_resource_counters_fail_explicitly() {
    for scope in [MetricScope::Process, MetricScope::CallingThread] {
        for metric in [
            ResourceMetric::VoluntaryContextSwitches,
            ResourceMetric::MinorPageFaults,
        ] {
            let error = ResourceCounter::new(metric, scope)
                .err()
                .expect("getrusage is unavailable on this platform");
            assert!(error.to_string().contains("unavailable on this platform"));
        }
    }
}

#[cfg(all(unix, not(target_os = "linux")))]
#[test]
fn calling_thread_resource_counters_require_rusage_thread() {
    for metric in [
        ResourceMetric::VoluntaryContextSwitches,
        ResourceMetric::MinorPageFaults,
    ] {
        let error = ResourceCounter::new(metric, MetricScope::CallingThread)
            .err()
            .expect("the official thread resource backend requires Linux");
        assert!(error.to_string().contains("RUSAGE_THREAD"));
    }
}

#[test]
fn environment_selection_preserves_requested_primary_and_unavailability() {
    const CHILD_ENV: &str = "OPEN_GPUI_BENCH_METRICS_ENV_TEST_CHILD";
    if std::env::var_os(CHILD_ENV).is_none() {
        let executable = std::env::current_exe().expect("the test executable has a path");
        for mode in [
            None,
            Some("wall-time"),
            Some("allocations"),
            Some("foreground-allocations"),
            Some("instructions"),
            Some("foreground-instructions"),
            Some("invalid-measurement"),
        ] {
            if cfg!(any(target_os = "linux", target_os = "macos"))
                && matches!(mode, Some("instructions" | "foreground-instructions"))
            {
                continue;
            }
            let mut command = std::process::Command::new(&executable);
            command
                .args([
                    "--exact",
                    "environment_selection_preserves_requested_primary_and_unavailability",
                    "--nocapture",
                ])
                .env(CHILD_ENV, "1");
            match mode {
                Some(mode) => command.env(BENCH_MEASUREMENT_ENV_VAR, mode),
                None => command.env_remove(BENCH_MEASUREMENT_ENV_VAR),
            };
            let output = command.output().expect("the isolated test process starts");
            assert!(
                output.status.success(),
                "measurement {mode:?}:\nstdout:\n{}\nstderr:\n{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr),
            );
        }
        return;
    }

    let mode = std::env::var(BENCH_MEASUREMENT_ENV_VAR);
    let selected = BenchMeasurement::from_env();
    match mode.as_deref() {
        Err(std::env::VarError::NotPresent) => {
            let measurement = selected.expect("the default primary is wall time");
            assert_eq!(measurement.formatter().scale_for_machines(&mut []), "ns");
        }
        Ok("wall-time") => {
            let measurement = selected.expect("explicit wall time needs no counters");
            assert_eq!(measurement.formatter().scale_for_machines(&mut []), "ns");
            measurement.start();
            std::hint::black_box(1);
            assert!(measurement.end(()) >= 0.0);
            let report = MetricReport::new();
            report.record_sample(1);
            assert!(report.is_empty());
        }
        Ok("allocations" | "foreground-allocations") => {
            let error = selected
                .err()
                .expect("an unavailable primary must not fall back to wall time");
            assert!(error.to_string().contains("CountingAllocator"));
        }
        Ok("instructions" | "foreground-instructions") => {
            let error = selected
                .err()
                .expect("an unsupported hardware primary must not fall back to wall time");
            assert!(error.to_string().contains("unavailable on this platform"));
        }
        Ok("invalid-measurement") => {
            let error = selected.err().expect("unknown primary names are rejected");
            assert!(error.to_string().contains("unsupported BENCH_MEASUREMENT"));
        }
        mode => panic!("unexpected isolated environment mode: {mode:?}"),
    }
}
