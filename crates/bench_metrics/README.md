# Open GPUI Bench Metrics

Criterion measurements and secondary reports, extracted from Zed's Apache-2.0 `bench_metrics`
package. This crate has no dependency on GPUI and can be used by an ordinary Criterion benchmark.

Use `BenchMeasurement` with Criterion 0.8.2, then collect per-iteration secondary values with
`MetricReport`. `BENCH_MEASUREMENT` selects the primary measurement; explicit unavailable hardware
or resource counters return an error. Hardware counters have process and calling-thread scopes.
Allocation metrics require a global `CountingAllocator`; unsupported counters are omitted rather
than represented by zero values. `FakeCounter` is available only with `test-support`.

The [API source](src/bench_metrics.rs) documents environment selection, platform support and
measurement/report composition. The workspace [verification record](../../docs/verification.md#complete-benchmark-context-and-measurements)
distinguishes Windows execution from Linux/macOS source support and hardware counter qualification.

Licensed under Apache-2.0. See [LICENSE-APACHE](LICENSE-APACHE) and [NOTICE](NOTICE).
