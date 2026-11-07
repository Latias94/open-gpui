use proptest::prelude::*;

use super::*;
use crate::{
    WindowId,
    profiler::{
        ActionTiming,
        journal::{self, FrameSkipReason, InputActivity, InputTiming},
    },
};

fn retained_input_count(activity: &ActiveUse) -> usize {
    usize::from(activity.latest_input_at.is_some())
}

fn key(at: Instant) {
    journal::record_input(InputTiming {
        kind: "key_down",
        start: at,
        end: at,
        caused_invalidation: false,
    });
}

fn activity_entry(at: Instant, active: bool) -> ForegroundJournalEntry {
    ForegroundJournalEntry::InputActivity(InputActivity { at, active })
}

fn action(at: Instant) {
    journal::record_action(ActionTiming {
        name: "test::ActiveUseAction",
        start: at,
        end: at + Duration::from_millis(20),
    });
}

fn complete(at: Instant) {
    journal::record_frame_skipped(WindowId::from(1), at, FrameSkipReason::NoRenderNeeded);
}

#[test]
fn repeated_poll_without_time_consumption_keeps_bounded_input_attribution() {
    let (journal, _guard) = journal::install_test_foreground_journal(32, 4);
    let mut detector = HangDetector::new(journal, Duration::from_millis(10), Duration::MAX);
    let base = Instant::now();
    for index in 0..512 {
        let at = base + Duration::from_secs(index);
        key(at);
        action(at + Duration::from_millis(1));
        complete(at + Duration::from_millis(30));
        let incidents = detector.poll();
        assert_eq!(incidents.len(), 1);
        assert!(incidents[0].during_active_use);
        assert!(
            retained_input_count(&detector.activity) <= 1,
            "a poll-only consumer must not retain per-input process history"
        );
    }
}

#[test]
fn consuming_active_time_cannot_erase_a_long_open_incidents_input_evidence() {
    for consume in [false, true] {
        let (journal, _guard) = journal::install_test_foreground_journal(32, 4);
        let mut detector = HangDetector::new(journal, Duration::from_millis(10), Duration::MAX);
        let base = Instant::now();
        detector.take_active_time(base, Duration::MAX);
        key(base + Duration::from_secs(1));
        action(base + Duration::from_secs(2));
        assert!(detector.poll().is_empty());
        if consume {
            detector.take_active_time(base + Duration::from_secs(602), Duration::MAX);
        }
        complete(base + Duration::from_secs(603));
        let incidents = detector.poll();
        assert_eq!(incidents.len(), 1);
        assert_eq!(incidents[0].trigger, HangTrigger::Threshold);
        assert!(
            incidents[0].during_active_use,
            "time consumption must not alter the open interval's attribution: consume={consume}"
        );
    }
}

#[test]
fn one_drain_attributes_each_boundary_before_observing_later_input() {
    let (journal, _guard) = journal::install_test_foreground_journal(64, 4);
    let mut detector = HangDetector::new(journal, Duration::from_millis(10), Duration::MAX);
    let base = Instant::now();
    key(base + Duration::from_secs(1));
    action(base + Duration::from_secs(2));
    complete(base + Duration::from_secs(3));
    action(base + Duration::from_secs(100));
    complete(base + Duration::from_secs(101));
    key(base + Duration::from_secs(130));
    action(base + Duration::from_secs(131));
    complete(base + Duration::from_secs(132));
    let incidents = detector.poll();
    assert_eq!(incidents.len(), 3);
    assert_eq!(
        incidents
            .iter()
            .map(|incident| incident.during_active_use)
            .collect::<Vec<_>>(),
        [true, false, true]
    );
}

#[test]
fn synchronous_boundaries_use_live_input_evidence_without_completed_input_work() {
    for drain_inside_input in [false, true] {
        let (journal, _guard) = journal::install_test_foreground_journal(32, 4);
        let mut detector = HangDetector::new(journal, Duration::from_millis(10), Duration::MAX);
        let base = Instant::now();
        let at = |seconds| base + Duration::from_secs(seconds);
        let input = journal::input_activity("key_down", at(1));
        assert!(detector.poll().is_empty());
        assert!(detector.activity.latest_input_at.is_none());
        action(at(100));
        complete(at(101));
        if drain_inside_input {
            let incidents = detector.poll();
            assert_eq!(incidents.len(), 1);
            assert!(incidents[0].during_active_use);
            assert!(detector.activity.latest_input_at.is_none());
        }
        input.finish(at(200));
        action(at(201));
        complete(at(202));
        action(at(261));
        complete(at(262));
        let incidents = detector.poll();
        let expected: &[bool] = if drain_inside_input {
            &[true, false]
        } else {
            &[true, true, false]
        };
        assert_eq!(
            incidents
                .iter()
                .map(|incident| incident.during_active_use)
                .collect::<Vec<_>>(),
            expected,
        );
        assert!(incidents.iter().all(|incident| {
            incident
                .snapshot
                .events
                .iter()
                .all(|event| !matches!(event, ForegroundEvent::Input(_)))
        }));
    }
}

#[test]
fn active_time_excludes_idle_between_disjoint_input_sessions() {
    let base = Instant::now();
    let at = |seconds| base + Duration::from_secs(seconds);
    let mut activity = ActiveUse::default();
    assert_eq!(activity.take(at(0), Duration::MAX), Duration::ZERO);
    for second in [1, 200] {
        activity.observe(&activity_entry(at(second), true));
        activity.observe(&activity_entry(at(second), false));
        activity.observe(&ForegroundJournalEntry::Event(ForegroundEvent::Input(
            InputTiming {
                kind: "key_down",
                start: at(second),
                end: at(second),
                caused_invalidation: false,
            },
        )));
    }
    assert_eq!(
        activity.take(at(201), Duration::MAX),
        Duration::from_secs(61)
    );
    assert_eq!(activity.take(at(201), Duration::MAX), Duration::ZERO);
}

#[test]
fn disjoint_sessions_have_the_same_duration_across_one_or_multiple_drains() {
    for drain_each in [false, true] {
        let (journal, _guard) = journal::install_test_foreground_journal(32, 4);
        let mut detector = HangDetector::new(journal, Duration::MAX, Duration::MAX);
        let base = Instant::now();
        let at = |seconds| base + Duration::from_secs(seconds);
        assert_eq!(
            detector.take_active_time(at(0), Duration::MAX),
            Duration::ZERO
        );
        for second in [1, 200] {
            journal::input_activity("key_down", at(second)).finish(at(second));
            key(at(second));
            if drain_each {
                assert!(detector.poll().is_empty());
            }
        }
        assert!(detector.poll().is_empty());
        assert_eq!(
            detector.take_active_time(at(201), Duration::MAX),
            Duration::from_secs(61)
        );
        assert_eq!(
            detector.take_active_time(at(201), Duration::MAX),
            Duration::ZERO
        );
        assert_eq!(
            detector.take_active_time(at(300), Duration::MAX),
            Duration::from_secs(59)
        );
        assert_eq!(
            detector.take_active_time(at(301), Duration::MAX),
            Duration::ZERO
        );
    }
}

#[test]
fn overlapping_active_windows_are_counted_once_and_caps_are_consumed_once() {
    let base = Instant::now();
    let at = |seconds| base + Duration::from_secs(seconds);
    let mut activity = ActiveUse::default();
    assert_eq!(activity.take(at(0), Duration::MAX), Duration::ZERO);
    for second in [1, 30] {
        activity.observe(&activity_entry(at(second), true));
        activity.observe(&activity_entry(at(second), false));
    }
    assert_eq!(
        activity.take(at(50), Duration::from_secs(10)),
        Duration::from_secs(10)
    );
    assert_eq!(activity.take(at(50), Duration::MAX), Duration::ZERO);
    assert_eq!(
        activity.take(at(100), Duration::MAX),
        Duration::from_secs(40)
    );
    assert_eq!(activity.take(at(100), Duration::MAX), Duration::ZERO);
}

#[test]
fn nested_child_completion_keeps_the_outer_dispatch_active_until_its_end() {
    for consume_during_outer in [false, true] {
        let (journal, _guard) = journal::install_test_foreground_journal(32, 4);
        let mut detector = HangDetector::new(journal, Duration::MAX, Duration::MAX);
        let base = Instant::now();
        let at = |seconds| base + Duration::from_secs(seconds);
        assert_eq!(
            detector.take_active_time(at(0), Duration::MAX),
            Duration::ZERO
        );
        let outer = journal::input_activity("key_down", at(1));
        let child = journal::input_activity("mouse_down", at(2));
        child.finish(at(3));
        journal::record_input(InputTiming {
            kind: "mouse_down",
            start: at(2),
            end: at(3),
            caused_invalidation: false,
        });
        if consume_during_outer {
            assert!(detector.poll().is_empty());
            assert_eq!(
                detector.take_active_time(at(100), Duration::MAX),
                Duration::from_secs(99)
            );
        }
        outer.finish(at(200));
        journal::record_input(InputTiming {
            kind: "key_down",
            start: at(1),
            end: at(200),
            caused_invalidation: false,
        });
        assert!(detector.poll().is_empty());
        assert_eq!(
            detector.take_active_time(at(201), Duration::MAX),
            Duration::from_secs(if consume_during_outer { 101 } else { 200 }),
        );
        assert_eq!(
            detector.take_active_time(at(300), Duration::MAX),
            Duration::from_secs(59)
        );
        assert_eq!(
            detector.take_active_time(at(301), Duration::MAX),
            Duration::ZERO
        );
    }
}

#[test]
fn first_take_and_suspend_consume_pending_time_without_rewriting_input_state() {
    let base = Instant::now();
    let at = |seconds| base + Duration::from_secs(seconds);
    let mut activity = ActiveUse::default();
    activity.observe(&activity_entry(at(1), true));
    assert_eq!(activity.take(at(20), Duration::MAX), Duration::ZERO);
    assert_eq!(activity.take(at(21), Duration::MAX), Duration::from_secs(1));
    activity.observe(&ForegroundJournalEntry::Boundary(
        IntervalBoundary::PowerTransition { ended_at: at(100) },
    ));
    assert!(activity.input_active);
    activity.observe(&activity_entry(at(200), false));
    assert_eq!(activity.take(at(201), Duration::MAX), Duration::ZERO);
    assert_eq!(activity.take(at(201), Duration::MAX), Duration::ZERO);
    assert_eq!(
        activity.take(at(300), Duration::MAX),
        Duration::from_secs(59)
    );
}

#[test]
fn loss_forgets_unproven_activity_until_a_new_absolute_control_arrives() {
    let base = Instant::now();
    let at = |seconds| base + Duration::from_secs(seconds);
    let mut activity = ActiveUse::default();
    assert_eq!(activity.take(at(0), Duration::MAX), Duration::ZERO);
    activity.observe(&activity_entry(at(1), true));
    assert_eq!(activity.take(at(2), Duration::MAX), Duration::from_secs(1));
    activity.observe(&ForegroundJournalEntry::Discontinuity { lost: 1 });
    assert_eq!(activity.take(at(200), Duration::MAX), Duration::ZERO);
    activity.observe(&activity_entry(at(201), false));
    assert_eq!(
        activity.take(at(202), Duration::MAX),
        Duration::from_secs(1)
    );
    assert_eq!(
        activity.take(at(300), Duration::MAX),
        Duration::from_secs(59)
    );
    assert_eq!(activity.take(at(301), Duration::MAX), Duration::ZERO);
}

#[test]
fn loss_preserves_only_time_already_proven_by_controls() {
    let base = Instant::now();
    let at = |seconds| base + Duration::from_secs(seconds);
    let mut activity = ActiveUse::default();
    assert_eq!(activity.take(at(0), Duration::MAX), Duration::ZERO);
    activity.observe(&activity_entry(at(1), true));
    activity.observe(&activity_entry(at(20), false));
    activity.observe(&ForegroundJournalEntry::Discontinuity { lost: 2 });
    activity.observe(&activity_entry(at(200), false));
    assert_eq!(
        activity.take(at(201), Duration::MAX),
        Duration::from_secs(20)
    );
    assert_eq!(activity.take(at(201), Duration::MAX), Duration::ZERO);
}

proptest! {
    #![proptest_config(ProptestConfig {
        failure_persistence: None,
        ..ProptestConfig::default()
    })]

    #[test]
    fn lifecycle_accounting_matches_independent_interval_union(
        sessions in prop::collection::vec((1u16..=1000, 0u16..=200), 0..=32),
        mut take_times in prop::collection::vec(0u16..=1500, 0..=32),
        cap in 0u16..=250,
    ) {
        let base = Instant::now();
        let at = |seconds: u64| base + Duration::from_secs(seconds);
        let mut transitions = Vec::new();
        let mut windows = Vec::new();
        for (start, duration) in sessions {
            let start = u64::from(start);
            let end = start + u64::from(duration);
            transitions.push((start, true));
            transitions.push((end, false));
            windows.push((start, end + 60));
        }
        transitions.sort_unstable_by_key(|&(at, active)| (at, !active));
        windows.sort_unstable();
        let mut coverage: Vec<(u64, u64)> = Vec::new();
        for (start, end) in windows {
            match coverage.last_mut() {
                Some((_, previous_end)) if start <= *previous_end => {
                    *previous_end = (*previous_end).max(end);
                }
                _ => coverage.push((start, end)),
            }
        }
        take_times.push(1500);
        take_times.sort_unstable();
        let mut activity = ActiveUse::default();
        prop_assert_eq!(activity.take(at(0), Duration::MAX), Duration::ZERO);
        let mut depth = 0;
        let mut next_transition = 0;
        let mut previous_take = 0;
        let cap = Duration::from_secs(u64::from(cap));
        for now in take_times {
            let now = u64::from(now);
            while let Some(&(second, active)) = transitions.get(next_transition) {
                if second > now {
                    break;
                }
                let was_active = depth > 0;
                if active {
                    depth += 1;
                } else {
                    depth -= 1;
                }
                if was_active != (depth > 0) {
                    activity.observe(&activity_entry(at(second), depth > 0));
                }
                next_transition += 1;
            }
            let expected: u64 = coverage
                .iter()
                .map(|&(start, end)| end.min(now).saturating_sub(start.max(previous_take)))
                .sum();
            prop_assert_eq!(
                activity.take(at(now), cap),
                Duration::from_secs(expected).min(cap),
            );
            prop_assert_eq!(activity.take(at(now), Duration::MAX), Duration::ZERO);
            previous_take = now;
        }
    }
}
