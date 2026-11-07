use open_gpui::{
    FrameRequestSource, PlatformFrameSignal, PlatformWindow, RequestFrameOptions, RunnableMeta,
};
use std::{cell::Cell, time::Duration};

#[test]
fn downstream_frame_request_options_keep_the_same_public_shape_without_a_platform() {
    let default = RequestFrameOptions::default();
    assert!(!default.require_presentation);
    assert!(!default.force_render);
    assert_eq!(default.signal_at, None);
    assert_eq!(default.signal_source, FrameRequestSource::NativeCallback);
    assert_eq!(
        FrameRequestSource::default(),
        FrameRequestSource::NativeCallback
    );

    let at = RunnableMeta::new_with_callers_location().spawned.0;
    let options = RequestFrameOptions {
        require_presentation: true,
        force_render: false,
        signal_at: Some(at),
        signal_source: FrameRequestSource::LocalSchedule,
    };
    let copied = options;
    assert_eq!(copied, options);
    assert_eq!(copied.signal_at, Some(at));
    assert_eq!(copied.signal_source, FrameRequestSource::LocalSchedule);
    assert!(copied.require_presentation);
    assert!(!copied.force_render);
    assert_ne!(options, default);
    assert_eq!(PlatformFrameSignal::new().take(), None);
    assert_eq!(PlatformFrameSignal::default().take(), None);
}

#[test]
fn downstream_frame_callback_signature_typechecks_without_constructing_a_platform() {
    let _backend_type_only = |window: &dyn PlatformWindow| {
        let callback: Box<dyn Fn(RequestFrameOptions)> = Box::new(|options| {
            let _: FrameRequestSource = options.signal_source;
            let _: bool = options.require_presentation;
            let _: bool = options.force_render;
            let _captured_time = options.signal_at;
        });
        window.on_request_frame(callback);
    };
}

#[cfg(not(feature = "profiler"))]
#[test]
fn downstream_disabled_frame_signal_never_evaluates_capture_or_retains_timestamps() {
    let captures = Cell::new(0);
    let captured = PlatformFrameSignal::capture(|| {
        captures.set(captures.get() + 1);
        RunnableMeta::new_with_callers_location().spawned.0
    });
    assert_eq!(captured, None);
    assert_eq!(captures.get(), 0);
    assert_eq!(std::mem::size_of::<PlatformFrameSignal>(), 0);

    let signals = PlatformFrameSignal::new();
    let at = RunnableMeta::new_with_callers_location().spawned.0;
    signals.record(at, FrameRequestSource::NativeCallback);
    signals.record(
        at + Duration::from_millis(1),
        FrameRequestSource::LocalSchedule,
    );
    assert_eq!(signals.take(), None);
    assert_eq!(signals.take(), None);
}

#[cfg(feature = "profiler")]
#[test]
fn downstream_enabled_frame_signal_keeps_the_earliest_timestamp_and_its_source() {
    for source in [
        FrameRequestSource::NativeCallback,
        FrameRequestSource::LocalSchedule,
    ] {
        let signals = PlatformFrameSignal::new();
        let captures = Cell::new(0);
        let at = RunnableMeta::new_with_callers_location().spawned.0;
        let captured = PlatformFrameSignal::capture(|| {
            captures.set(captures.get() + 1);
            at
        });
        assert_eq!(captured, Some(at));
        assert_eq!(captures.get(), 1);
        let other_source = match source {
            FrameRequestSource::NativeCallback => FrameRequestSource::LocalSchedule,
            FrameRequestSource::LocalSchedule => FrameRequestSource::NativeCallback,
        };
        signals.record(at + Duration::from_millis(2), other_source);
        signals.record(at, source);
        signals.record(at + Duration::from_millis(1), other_source);
        assert_eq!(signals.take(), Some((at, source)));
        assert_eq!(signals.take(), None);

        let next = at + Duration::from_millis(3);
        signals.record(next, other_source);
        assert_eq!(signals.take(), Some((next, other_source)));
        assert_eq!(signals.take(), None);
    }
}

#[cfg(all(feature = "profiler", not(target_family = "wasm")))]
#[test]
fn downstream_concurrent_producers_preserve_one_coherent_timestamp_source_pair() {
    use std::{sync::Barrier, thread};

    for source in [
        FrameRequestSource::NativeCallback,
        FrameRequestSource::LocalSchedule,
    ] {
        let signals = PlatformFrameSignal::new();
        let at = RunnableMeta::new_with_callers_location().spawned.0;
        let other_source = match source {
            FrameRequestSource::NativeCallback => FrameRequestSource::LocalSchedule,
            FrameRequestSource::LocalSchedule => FrameRequestSource::NativeCallback,
        };
        let start = Barrier::new(3);
        thread::scope(|scope| {
            scope.spawn(|| {
                start.wait();
                signals.record(at, source);
            });
            scope.spawn(|| {
                start.wait();
                signals.record(at + Duration::from_millis(1), other_source);
            });
            start.wait();
        });
        assert_eq!(signals.take(), Some((at, source)));
        assert_eq!(signals.take(), None);
    }
}

#[cfg(feature = "profiler")]
#[test]
fn downstream_journal_marker_and_skip_signatures_typecheck_without_starting_an_app() {
    use open_gpui::{
        Window,
        profiler::{
            hang::{HangIncident, SerializedHangIncident},
            journal::{
                ForegroundEvent, ForegroundJournalEntry, FrameSkipReason, FrameSkipped,
                InputTiming, IntervalBoundary, IntervalSealer, PlatformSignal,
            },
        },
    };

    let _journal_type_only = |window: &Window, options: RequestFrameOptions| {
        let at = RunnableMeta::new_with_callers_location().spawned.0;
        let window_id = window.window_handle().window_id();
        let signal = PlatformSignal {
            window_id,
            signal_at: options.signal_at,
            handled_at: at,
            source: options.signal_source,
        };
        let skipped = FrameSkipped {
            window_id,
            at,
            reason: FrameSkipReason::NoRenderNeeded,
        };
        let mut sealer = IntervalSealer::new(at);
        let raw_snapshots = sealer.push_entries([
            ForegroundJournalEntry::PlatformSignal(signal),
            ForegroundJournalEntry::ReentrantFrameSkipped { window_id, at },
        ]);
        let snapshots = sealer.push_entries([
            ForegroundJournalEntry::Event(ForegroundEvent::Input(InputTiming {
                kind: "consumer-control",
                start: at,
                end: at,
                caused_invalidation: false,
            })),
            ForegroundJournalEntry::Boundary(IntervalBoundary::FrameSkipped(skipped)),
        ]);
        let converted: Vec<SerializedHangIncident> = snapshots
            .into_iter()
            .filter_map(|snapshot| HangIncident::detect(snapshot, Duration::ZERO, Duration::MAX))
            .map(|incident| SerializedHangIncident::convert(at, &incident, 1, None))
            .collect();
        (signal, skipped, raw_snapshots, converted)
    };
}

#[cfg(feature = "profiler")]
#[test]
fn downstream_completed_skip_reasons_remain_three_distinct_values() {
    use open_gpui::profiler::journal::FrameSkipReason;

    assert_ne!(
        FrameSkipReason::NoRenderNeeded,
        FrameSkipReason::InactiveFrameRateLimit
    );
    assert_ne!(
        FrameSkipReason::NoRenderNeeded,
        FrameSkipReason::ThermalFrameRateLimit
    );
    assert_ne!(
        FrameSkipReason::InactiveFrameRateLimit,
        FrameSkipReason::ThermalFrameRateLimit
    );
}
