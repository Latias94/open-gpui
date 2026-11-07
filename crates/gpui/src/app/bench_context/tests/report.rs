use super::*;
use crate::profiler::{
    DrawPhases, FrameTiming, PresentTiming,
    journal::{PollSummary, SmallPollFlush},
};

#[test]
fn report_budget_counts_whole_deadlines_after_the_first_budget() {
    let report = BenchReport::with_frame_budget_nanos(10);
    for (nanos, expected) in [(0, 0), (10, 0), (11, 1), (20, 1), (21, 2)] {
        assert_eq!(
            report.budget_overruns(Duration::from_nanos(nanos)),
            expected
        );
    }
    let mut histogram = Histogram::new(3).expect("histogram precision must be valid");
    for nanos in [0, 10, 11, 20, 21] {
        histogram.record(nanos).expect("histogram must auto-resize");
    }
    assert_eq!(report.total_budget_overruns(&histogram), 4);
    assert_eq!(
        BenchReport::default().frame_budget_nanos,
        1_000_000_000 / 120
    );
    assert_eq!(
        BenchReport::with_fps(60).frame_budget_nanos,
        1_000_000_000 / 60
    );
    assert!(BenchReport::default().foreground_work().is_none());
}

#[test]
fn report_rejects_zero_length_frame_budgets() {
    for build in [
        || BenchReport::with_fps(0),
        || BenchReport::with_fps(1_000_000_001),
        || BenchReport::with_frame_budget_nanos(0),
    ] {
        assert!(catch_unwind(build).is_err());
    }
}

#[test]
fn report_preserves_frame_histogram_and_optional_sample_semantics() {
    let start = open_gpui_scheduler::Instant::now();
    let draw = FrameTiming {
        window_id: crate::WindowId::default(),
        dirty_at: Some(start),
        invalidations: 7,
        draw_start: start + Duration::from_nanos(3),
        draw_end: start + Duration::from_nanos(8),
        refresh_interval: None,
        signal_at: None,
        phases: DrawPhases::default(),
    };
    let present = PresentTiming {
        window_id: draw.window_id,
        present_start: draw.draw_end,
        present_end: draw.draw_end + Duration::from_nanos(2),
        animation_interval: Some(Duration::from_nanos(17)),
    };
    let events = [
        FrameEvent::Draw(draw),
        FrameEvent::Draw(FrameTiming {
            dirty_at: None,
            invalidations: 0,
            ..draw
        }),
        FrameEvent::Present(present),
        FrameEvent::Present(PresentTiming {
            animation_interval: None,
            ..present
        }),
    ];
    let report = BenchReport::default();
    report.record_frame_timings(&events);
    let snapshot = report.frame_snapshot.borrow();
    assert_eq!(snapshot.draw.len(), 2);
    assert_eq!(snapshot.draw.count_at(5), 2);
    assert_eq!(snapshot.dirty_to_draw.len(), 1);
    assert_eq!(snapshot.dirty_to_draw.count_at(8), 1);
    assert_eq!(snapshot.present_interval.len(), 1);
    assert_eq!(snapshot.present_interval.count_at(17), 1);
    assert_eq!(snapshot.invalidations_per_frame.len(), 1);
    assert_eq!(snapshot.invalidations_per_frame.count_at(7), 1);
    assert!(snapshot.foreground_work.histogram.is_empty());
    assert!(!snapshot.is_empty());
}

#[test]
fn foreground_report_uses_folded_poll_total_and_excludes_draw_and_present() {
    let start = open_gpui_scheduler::Instant::now();
    let draw = FrameTiming {
        window_id: crate::WindowId::default(),
        dirty_at: None,
        invalidations: 0,
        draw_start: start,
        draw_end: start + Duration::from_secs(1),
        refresh_interval: None,
        signal_at: None,
        phases: DrawPhases::default(),
    };
    let events = [
        ForegroundEvent::Draw(draw),
        ForegroundEvent::Present(PresentTiming {
            window_id: draw.window_id,
            present_start: draw.draw_end,
            present_end: draw.draw_end + Duration::from_secs(1),
            animation_interval: None,
        }),
        ForegroundEvent::SmallPolls(SmallPollFlush {
            summary: PollSummary {
                count: 40,
                total: Duration::from_nanos(21),
            },
            since: start,
            until: start + Duration::from_secs(3),
        }),
    ];
    let report = BenchReport::with_frame_budget_nanos(10);
    let clone = report.clone();
    report.record_foreground_events(&events);
    let summary = clone
        .foreground_work()
        .expect("the folded poll must be reported");
    assert_eq!(summary.count, 1);
    assert_eq!(summary.total, Duration::from_nanos(21));
    assert_eq!(summary.max, Duration::from_nanos(21));
    assert_eq!(summary.p50, summary.max);
    assert_eq!(summary.p90, summary.max);
    assert_eq!(summary.p95, summary.max);
    assert_eq!(summary.p99, summary.max);
    assert_eq!(summary.frame_budget_overruns_total, 2);
    assert_eq!(summary.frame_budget_overruns_max, 2);
    assert!(report.frame_snapshot.borrow().draw.is_empty());
}
