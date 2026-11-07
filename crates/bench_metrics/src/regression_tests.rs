use super::*;
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};

#[test]
fn batched_samples_sum_intervals_before_normalizing() {
    let primary = Arc::new(AtomicU64::new(100));
    let secondary = Arc::new(AtomicU64::new(200));
    let measurement = BenchMeasurement::new(FakeCounter::new(primary.clone()))
        .named("primary ticks")
        .with_secondary("ticks", FakeCounter::new(secondary.clone()))
        .with_ratio("ticks per primary", "ticks", "primary ticks");
    let report = MetricReport::new();

    for iterations in [2, 3] {
        measurement.start();
        primary.fetch_add(iterations, Ordering::SeqCst);
        secondary.fetch_add(iterations * 7, Ordering::SeqCst);
        assert_eq!(measurement.end(()), iterations as f64);
    }
    report.record_sample(5);
    primary.fetch_add(1_000_000, Ordering::SeqCst);
    secondary.fetch_add(1_000_000, Ordering::SeqCst);
    measurement.start();
    primary.fetch_add(4, Ordering::SeqCst);
    secondary.fetch_add(28, Ordering::SeqCst);
    assert_eq!(measurement.end(()), 4.0);
    report.record_sample(4);

    assert_eq!(report.values("ticks"), Some(vec![7.0, 7.0]));
    assert_eq!(report.values("ticks per primary"), Some(vec![7.0, 7.0]));
}

#[test]
fn discarding_and_zero_iteration_samples_reset_primary_and_secondary_totals() {
    for zero_iterations in [false, true] {
        let primary = Arc::new(AtomicU64::new(0));
        let secondary = Arc::new(AtomicU64::new(0));
        let measurement = BenchMeasurement::new(FakeCounter::new(primary.clone()))
            .with_secondary("ticks", FakeCounter::new(secondary.clone()))
            .with_ratio("ticks per primary", "ticks", PRIMARY_METRIC_NAME);
        let report = MetricReport::new();

        measurement.start();
        primary.fetch_add(2, Ordering::SeqCst);
        secondary.fetch_add(50, Ordering::SeqCst);
        assert_eq!(measurement.end(()), 2.0);
        if zero_iterations {
            report.record_sample(0);
        } else {
            report.discard_pending();
        }
        assert!(report.is_empty());

        measurement.start();
        primary.fetch_add(3, Ordering::SeqCst);
        secondary.fetch_add(21, Ordering::SeqCst);
        assert_eq!(measurement.end(()), 3.0);
        report.record_sample(3);
        assert_eq!(report.values("ticks"), Some(vec![7.0]));
        assert_eq!(report.values("ticks per primary"), Some(vec![7.0]));
    }
}

#[test]
fn unavailable_ratios_and_zero_denominators_are_omitted() {
    let primary = Arc::new(AtomicU64::new(0));
    let secondary = Arc::new(AtomicU64::new(0));
    let measurement = BenchMeasurement::new(FakeCounter::new(primary.clone()))
        .with_secondary("ticks", FakeCounter::new(secondary.clone()))
        .with_ratio("ticks per primary", "ticks", PRIMARY_METRIC_NAME)
        .with_ratio("missing numerator", "missing", "ticks")
        .with_ratio("missing denominator", "ticks", "missing");
    let report = MetricReport::new();

    measurement.start();
    assert_eq!(measurement.end(()), 0.0);
    report.record_sample(1);
    assert_eq!(report.values("ticks"), Some(vec![0.0]));
    assert!(report.values("ticks per primary").is_none());

    measurement.start();
    primary.fetch_add(2, Ordering::SeqCst);
    secondary.fetch_add(8, Ordering::SeqCst);
    assert_eq!(measurement.end(()), 2.0);
    report.record_sample(2);
    assert_eq!(report.values("ticks"), Some(vec![0.0, 4.0]));
    assert_eq!(report.values("ticks per primary"), Some(vec![4.0]));
    assert!(report.values("missing numerator").is_none());
    assert!(report.values("missing denominator").is_none());
}

#[test]
fn switching_measurements_keeps_discarded_totals_out_of_later_samples() {
    let first_counter = Arc::new(AtomicU64::new(0));
    let second_counter = Arc::new(AtomicU64::new(0));
    let first = BenchMeasurement::new(FakeCounter::new(first_counter.clone()))
        .with_secondary("ticks", FakeCounter::new(first_counter.clone()));
    let second = BenchMeasurement::new(FakeCounter::new(second_counter.clone()))
        .with_secondary("ticks", FakeCounter::new(second_counter.clone()));
    let report = MetricReport::new();

    first.start();
    first_counter.fetch_add(99, Ordering::SeqCst);
    assert_eq!(first.end(()), 99.0);
    report.discard_pending();

    second.start();
    second_counter.fetch_add(7, Ordering::SeqCst);
    assert_eq!(second.end(()), 7.0);
    report.record_sample(1);

    first.start();
    first_counter.fetch_add(5, Ordering::SeqCst);
    assert_eq!(first.end(()), 5.0);
    report.record_sample(1);
    assert_eq!(report.values("ticks"), Some(vec![7.0, 5.0]));
}

#[test]
fn count_formatter_handles_every_criterion_throughput() {
    let formatter = CountFormatter::new("cycles");
    for (throughput, unit) in [
        (criterion::Throughput::Bits(4), "cycles/bit"),
        (criterion::Throughput::Bytes(4), "cycles/byte"),
        (criterion::Throughput::BytesDecimal(4), "cycles/byte"),
        (criterion::Throughput::Elements(4), "cycles/element"),
        (
            criterion::Throughput::ElementsAndBytes {
                elements: 4,
                bytes: 16,
            },
            "cycles/element",
        ),
    ] {
        let mut values = [8.0, 16.0];
        assert_eq!(
            formatter.scale_throughputs(8.0, &throughput, &mut values),
            unit
        );
        assert_eq!(values, [2.0, 4.0]);
    }
}
