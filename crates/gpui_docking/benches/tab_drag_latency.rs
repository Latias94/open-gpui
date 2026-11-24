//! End-to-end tab drag latency in the retained shared-session adapter.

mod drag_workload;

use std::{hint::black_box, time::Instant};

use drag_workload::{DragWorkload, report};
use serde_json::json;

fn main() -> anyhow::Result<()> {
    const CYCLES: usize = 30;
    const SAMPLES: usize = 7;
    let mut workload = DragWorkload::new()?;
    for _ in 0..5 {
        workload.cycle()?;
    }
    let mut samples = [0.0; SAMPLES];
    for sample in &mut samples {
        let start = Instant::now();
        for _ in 0..CYCLES {
            black_box(workload.cycle())?;
        }
        *sample = start.elapsed().as_nanos() as f64 / CYCLES as f64;
    }
    samples.sort_by(f64::total_cmp);
    println!(
        "{}",
        report(
            CYCLES,
            json!({
                "median_ns_per_cycle": samples[SAMPLES / 2],
                "samples_ns_per_cycle": samples,
            })
        )
    );
    Ok(())
}
