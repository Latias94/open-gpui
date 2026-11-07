#![cfg(feature = "profiler")]

use open_gpui::{
    App, MouseDownEvent, PlatformInput, RunnableMeta,
    profiler::{
        ActionTiming,
        hang::{
            HangDetector, HangIncident, HangTrigger, MEASUREMENT_VERSION,
            SerializedHangContributor, SerializedHangIncident,
        },
        journal::{
            DrainedEntries, ForegroundEvent, ForegroundJournal, ForegroundJournalCollector,
            ForegroundJournalEntry, FrameSnapshot, InputTiming, IntervalBoundary, IntervalSealer,
            PollSummary, SmallPollFlush,
        },
    },
};
use std::time::Duration;

#[test]
fn downstream_journal_and_detector_signatures_typecheck_without_starting_an_app() {
    let _: fn(&App) -> ForegroundJournal = App::foreground_journal;
    let _: fn(&ForegroundJournal) -> ForegroundJournalCollector = ForegroundJournal::collector;
    let _: fn(&mut ForegroundJournalCollector) -> DrainedEntries =
        ForegroundJournalCollector::collect_unseen;
    let _: fn(&mut HangDetector) -> Vec<HangIncident> = HangDetector::poll;

    let _type_only = |app: &App| {
        let journal: ForegroundJournal = app.foreground_journal();
        let mut first: ForegroundJournalCollector = journal.collector();
        let mut second: ForegroundJournalCollector = journal.clone().collector();
        let mut detector = HangDetector::new(
            journal,
            Duration::from_millis(50),
            Duration::from_millis(16),
        );
        let start = RunnableMeta::new_with_callers_location().spawned.0;
        let mut sealer = IntervalSealer::new(start);
        let drained: DrainedEntries = first.collect_unseen();
        let _: u64 = drained.lost;
        let snapshots: Vec<FrameSnapshot> = sealer.push_entries(drained.entries);
        let _: DrainedEntries = second.collect_unseen();
        let incidents: Vec<HangIncident> = detector.poll();
        (snapshots, incidents, detector.first_present_at())
    };
}

#[test]
fn downstream_public_interval_values_preserve_nested_work_and_conversion_metadata() {
    let start = RunnableMeta::new_with_callers_location().spawned.0;
    let mut sealer = IntervalSealer::new(start);
    let small_polls = PollSummary {
        count: 2,
        total: Duration::from_micros(500),
    };
    let action = ForegroundEvent::Action(ActionTiming {
        name: "consumer::Save",
        start: start + Duration::from_millis(3),
        end: start + Duration::from_millis(5),
    });
    let input = ForegroundEvent::Input(InputTiming {
        kind: PlatformInput::MouseDown(MouseDownEvent::default()).kind_name(),
        start: start + Duration::from_millis(1),
        end: start + Duration::from_millis(7),
        caused_invalidation: true,
    });
    assert!(
        sealer
            .push_entries([
                ForegroundJournalEntry::Event(ForegroundEvent::SmallPolls(SmallPollFlush {
                    summary: small_polls,
                    since: start,
                    until: start + Duration::from_millis(1),
                })),
                ForegroundJournalEntry::Event(action),
                ForegroundJournalEntry::Event(input),
            ])
            .is_empty()
    );
    let snapshots =
        sealer.push_entries([ForegroundJournalEntry::Boundary(IntervalBoundary::Idle {
            ended_at: start + Duration::from_millis(9),
        })]);
    assert_eq!(snapshots.len(), 1);
    let snapshot = snapshots.into_iter().next().unwrap();
    assert_eq!(snapshot.small_poll_summary(), small_polls);
    assert_eq!(snapshot.occupancy(), Duration::from_micros(6500));
    let incident = HangIncident::detect(snapshot.clone(), Duration::from_millis(5), Duration::MAX)
        .expect("the public input value exceeds the test threshold");
    assert_eq!(incident.trigger, HangTrigger::Threshold);
    assert_eq!(
        incident.active_window(),
        (input.start_time(), snapshot.interval_end())
    );
    let converted = SerializedHangIncident::convert(start, &incident, 1, None);
    assert_eq!(MEASUREMENT_VERSION, 3);
    assert_eq!(converted.measurement_version, MEASUREMENT_VERSION);
    assert!(!converted.during_active_use);
    assert_eq!(converted.phase, "startup");
    assert_eq!(converted.sealed_by, "idle");
    assert_eq!(converted.start_ms, 1.0);
    assert_eq!(converted.active_ms, 8.0);
    assert_eq!(converted.stall_ms, 6.0);
    assert_eq!(converted.dirty_to_present_ms, None);
    assert_eq!(converted.busy_fraction, 0.75);
    assert_eq!(converted.small_poll_count, 2);
    assert_eq!(converted.small_poll_total_ms, 0.5);
    assert_eq!(converted.contributors.len(), 1);
    assert_eq!(converted.event_count, 2);
    assert!(!converted.journal_discontinuous);

    assert!(
        HangIncident::detect(
            snapshot.clone(),
            Duration::from_millis(7),
            Duration::from_micros(6500),
        )
        .is_none(),
        "folded and nested occupancy cannot qualify as one blocking event"
    );
    let blocking = HangIncident::detect(
        snapshot.clone(),
        Duration::from_millis(7),
        Duration::from_millis(6),
    )
    .expect("one input event reaches the blocking-work budget");
    assert_eq!(blocking.trigger, HangTrigger::BlockingWork);
    assert_eq!(blocking.contributors.len(), 1);
    let nested = HangIncident::detect(snapshot.clone(), Duration::from_millis(2), Duration::MAX)
        .expect("both nested events reach the threshold independently");
    let capped = SerializedHangIncident::convert(start, &nested, 1, Some(start));
    assert_eq!(capped.phase, "steady");
    assert_eq!(capped.contributors.len(), 1);
    assert_eq!(capped.contributors_elided, 1);
    let uncapped = SerializedHangIncident::convert(start, &nested, usize::MAX, Some(start));
    assert!(matches!(
        uncapped.contributors.as_slice(),
        [
            SerializedHangContributor::Input {
                input_kind: "mouse_down",
                caused_invalidation: true,
                depth: 0,
                ..
            },
            SerializedHangContributor::Action {
                name: "consumer::Save",
                depth: 1,
                ..
            }
        ]
    ));
    assert!(
        HangIncident::detect(
            snapshot,
            Duration::from_millis(7),
            Duration::from_micros(6001)
        )
        .is_none()
    );
}

#[cfg(not(target_family = "wasm"))]
#[test]
fn downstream_monitor_signature_typechecks_without_spawning_a_thread() {
    use open_gpui::profiler::hang::{HangMonitorConfig, HangMonitorError, HangMonitorPoll};

    let _type_only = |app: &mut App, config: HangMonitorConfig| -> Result<(), HangMonitorError> {
        let mut reasons = Vec::new();
        app.start_hang_monitor(config, move |poll: HangMonitorPoll| {
            reasons.push(poll.reason);
            let _: Vec<HangIncident> = poll.incidents;
            let _: Duration = poll.active_time;
            let _ = poll.first_present_at;
        })
    };
}

#[cfg(not(target_family = "wasm"))]
#[test]
fn downstream_monitor_errors_and_poll_reasons_are_executable_values() {
    use open_gpui::profiler::hang::{HangMonitorError, HangMonitorPollReason};
    use std::{error::Error as _, io};

    let already_started = HangMonitorError::AlreadyStarted;
    assert_eq!(
        already_started.to_string(),
        "the hang monitor was already started"
    );
    assert!(already_started.source().is_none());
    let spawn = HangMonitorError::Spawn(io::Error::other("consumer spawn control"));
    assert_eq!(
        spawn.source().unwrap().to_string(),
        "consumer spawn control"
    );
    assert_ne!(
        HangMonitorPollReason::Interval,
        HangMonitorPollReason::Flush
    );
}
