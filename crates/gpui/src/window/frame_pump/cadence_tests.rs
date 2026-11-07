use super::*;
use crate::WindowOptions;

fn limits() -> FrameThrottleFacts {
    let options = WindowOptions::default();
    FrameThrottleFacts {
        require_presentation: false,
        active: true,
        high_rate_input: false,
        thermal_state: ThermalState::Nominal,
        power_mode: PowerMode::Unknown,
        inactive_frame_interval: options.inactive_frame_interval,
        low_power_frame_interval: options.low_power_frame_interval,
        thermal_frame_interval: options.thermal_frame_interval,
    }
}

#[test]
fn frame_throttle_tracks_display_refreshes() {
    let start = Instant::now();
    for (elapsed, interval) in [
        (Duration::from_micros(16_666), Duration::from_micros(16_667)),
        (Duration::from_micros(33_332), Duration::from_micros(33_333)),
    ] {
        let mut pacer = FramePacer::default();
        assert!(!pacer.should_defer(start, Some(interval)));
        assert!(pacer.should_defer(start + elapsed / 2, Some(interval)));
        assert!(!pacer.should_defer(start + elapsed, Some(interval)));
    }
}

#[test]
fn frame_throttle_preserves_cadence_without_catching_up_after_stalls() {
    let start = Instant::now();
    let interval = Duration::from_micros(33_333);
    let mut pacer = FramePacer::default();
    assert!(!pacer.should_defer(start, Some(interval)));
    let delayed = start + interval + Duration::from_millis(5);
    assert!(!pacer.should_defer(delayed, Some(interval)));
    assert!(pacer.should_defer(start + interval + interval / 2, Some(interval)));
    assert!(!pacer.should_defer(start + interval * 2, Some(interval)));
    let after_stall = start + interval * 4 - Duration::from_millis(1);
    assert!(!pacer.should_defer(after_stall, Some(interval)));
    assert!(pacer.should_defer(after_stall + interval / 2, Some(interval)));
    assert!(!pacer.should_defer(after_stall + interval, Some(interval)));
    let mandatory = after_stall + interval + Duration::from_millis(2);
    assert!(!pacer.should_defer(mandatory, None));
    assert!(pacer.should_defer(mandatory + interval / 2, Some(interval)));
}

#[test]
fn cadence_tolerance_and_quarter_interval_have_inclusive_boundaries() {
    let start = Instant::now();
    let interval = Duration::from_millis(40);
    for (delay, resets) in [
        (Duration::from_millis(10), false),
        (Duration::from_millis(10) + Duration::from_nanos(1), true),
    ] {
        let mut pacer = FramePacer::default();
        assert!(!pacer.should_defer(start, Some(interval)));
        let too_early = start + interval - FRAME_INTERVAL_TOLERANCE - Duration::from_nanos(1);
        assert!(pacer.should_defer(too_early, Some(interval)));
        assert_eq!(pacer.last_frame, Some(start));
        let delayed = start + interval + delay;
        assert!(!pacer.should_defer(delayed, Some(interval)));
        assert_eq!(
            pacer.last_frame,
            Some(if resets { delayed } else { start + interval })
        );
    }
    let mut pacer = FramePacer::default();
    assert!(!pacer.should_defer(start, Some(interval)));
    assert!(!pacer.should_defer(start + interval - FRAME_INTERVAL_TOLERANCE, Some(interval)));
    assert_eq!(pacer.last_frame, Some(start + interval));
}

#[test]
fn repeated_small_delivery_delays_do_not_accumulate_cadence_drift() {
    let start = Instant::now();
    let interval = Duration::from_micros(33_333);
    let mut pacer = FramePacer::default();
    assert!(!pacer.should_defer(start, Some(interval)));
    for index in 1..100 {
        let deadline = start + interval * index;
        assert!(!pacer.should_defer(deadline + Duration::from_millis(2), Some(interval)));
        assert_eq!(pacer.last_frame, Some(deadline));
    }
}

#[test]
fn power_modes_preserve_frame_limits() {
    for (power_mode, active_interval) in [
        (
            PowerMode::LowPower,
            Some((Duration::from_micros(33_333), FrameRateLimit::LowPower)),
        ),
        (PowerMode::Unknown, None),
        (PowerMode::Balanced, None),
        (PowerMode::HighPower, None),
    ] {
        assert_eq!(
            FrameThrottleFacts {
                power_mode,
                ..limits()
            }
            .min_frame_interval(),
            active_interval
        );
        let thermal = FrameThrottleFacts {
            power_mode,
            thermal_state: ThermalState::Serious,
            ..limits()
        }
        .min_frame_interval();
        assert_eq!(
            thermal,
            active_interval.or(Some((
                Duration::from_micros(16_667),
                FrameRateLimit::Thermal
            )))
        );
    }
}

#[test]
fn frame_limits_combine_enabled_intervals() {
    let short = Some(Duration::from_millis(10));
    let long = Some(Duration::from_millis(100));
    for (inactive, low_power, thermal, expected) in [
        (
            long,
            short,
            short,
            Some((Duration::from_millis(100), FrameRateLimit::Inactive)),
        ),
        (
            short,
            long,
            short,
            Some((Duration::from_millis(100), FrameRateLimit::LowPower)),
        ),
        (
            short,
            short,
            long,
            Some((Duration::from_millis(100), FrameRateLimit::Thermal)),
        ),
        (
            long,
            None,
            None,
            Some((Duration::from_millis(100), FrameRateLimit::Inactive)),
        ),
        (
            None,
            long,
            None,
            Some((Duration::from_millis(100), FrameRateLimit::LowPower)),
        ),
        (
            None,
            None,
            long,
            Some((Duration::from_millis(100), FrameRateLimit::Thermal)),
        ),
        (None, None, None, None),
    ] {
        let facts = FrameThrottleFacts {
            active: false,
            power_mode: PowerMode::LowPower,
            thermal_state: ThermalState::Critical,
            inactive_frame_interval: inactive,
            low_power_frame_interval: low_power,
            thermal_frame_interval: thermal,
            ..limits()
        };
        assert_eq!(facts.min_frame_interval(), expected);
        assert_eq!(
            FrameThrottleFacts {
                require_presentation: true,
                ..facts
            }
            .min_frame_interval(),
            None
        );
    }
}

#[test]
fn equal_limits_report_the_official_inactive_then_low_power_priority() {
    let same = Some(Duration::from_millis(50));
    let facts = FrameThrottleFacts {
        active: false,
        power_mode: PowerMode::LowPower,
        thermal_state: ThermalState::Critical,
        inactive_frame_interval: same,
        low_power_frame_interval: same,
        thermal_frame_interval: same,
        ..limits()
    };
    assert_eq!(
        facts.min_frame_interval().unwrap().1,
        FrameRateLimit::Inactive
    );
    assert_eq!(
        FrameThrottleFacts {
            high_rate_input: true,
            ..facts
        }
        .min_frame_interval()
        .unwrap()
        .1,
        FrameRateLimit::LowPower
    );
}

#[test]
fn unavailable_deadline_defers_without_overflow_and_mandatory_work_resets_it() {
    let start = Instant::now();
    let mut pacer = FramePacer::default();
    assert!(!pacer.should_defer(start, Some(Duration::MAX)));
    assert!(pacer.should_defer(start, Some(Duration::MAX)));
    let mandatory = start + Duration::from_secs(1);
    assert!(!pacer.should_defer(mandatory, None));
    assert_eq!(pacer.last_frame, Some(mandatory));
    let interval = Duration::from_millis(100);
    assert!(pacer.should_defer(start + interval, Some(interval)));
    assert!(!pacer.should_defer(mandatory + interval, Some(interval)));
    assert_eq!(pacer.last_frame, Some(mandatory + interval));
    assert!(!pacer.should_defer(mandatory + interval, Some(Duration::ZERO)));
}

#[crate::test]
fn idle_callbacks_do_not_consume_cadence_and_dirty_work_waits_for_its_slot(
    cx: &mut crate::TestAppContext,
) {
    use crate::{Context, IntoElement, Render, RequestFrameOptions, Styled as _, Window, div};

    struct Root;
    impl Render for Root {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            div().size_full()
        }
    }

    let (_, visual) = cx.add_window_view(|_, _| Root);
    visual.run_until_parked();
    visual.update(|window, app| {
        window.active.set(false);
        window.inactive_frame_interval = Some(Duration::from_secs(1));
        window.draw(app).clear(app);
        window.present(app);
        window.next_frame_callbacks.borrow_mut().clear();
        window.invalidator.set_dirty(false);
        window.needs_present.set(false);
        let baseline = window.presentation_facts();
        let old = Instant::now() - Duration::from_secs(10);
        window.frame_pacer.last_frame = Some(old);
        window.native_frame_requested(RequestFrameOptions::default(), app);
        assert_eq!(window.frame_pacer.last_frame, Some(old));
        assert_eq!(window.presentation_facts(), baseline);

        let previous = Instant::now();
        window.frame_pacer.last_frame = Some(previous);
        window.refresh();
        window.native_frame_requested(RequestFrameOptions::default(), app);
        assert_eq!(window.frame_pacer.last_frame, Some(previous));
        assert!(window.invalidator.is_dirty());
        assert_eq!(window.presentation_facts(), baseline);

        window.native_frame_requested(
            RequestFrameOptions {
                require_presentation: true,
                ..Default::default()
            },
            app,
        );
        let submitted = window.presentation_facts();
        assert_eq!(
            submitted.frame_accepted_generation,
            baseline
                .frame_accepted_generation
                .map(|generation| generation + 1)
        );
        assert_eq!(
            submitted.frame_accepted_generation,
            submitted.present_submitted_generation
        );
        assert!(!window.invalidator.is_dirty());
        assert!(window.frame_pacer.last_frame.unwrap() >= previous);
    });
}
