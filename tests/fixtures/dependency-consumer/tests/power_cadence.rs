use open_gpui::{App, Platform, PowerMode, Subscription, WindowOptions};
use std::{sync::Arc, time::Duration};

#[test]
fn downstream_power_modes_are_distinct_copyable_public_values() {
    let modes = [
        PowerMode::Unknown,
        PowerMode::LowPower,
        PowerMode::Balanced,
        PowerMode::HighPower,
    ];
    for (index, mode) in modes.into_iter().enumerate() {
        let copied = mode;
        assert_eq!(copied, mode);
        assert!(!format!("{mode:?}").is_empty());
        for other in modes.into_iter().skip(index + 1) {
            assert_ne!(mode, other);
        }
        let name = match mode {
            PowerMode::Unknown => "unknown",
            PowerMode::LowPower => "low-power",
            PowerMode::Balanced => "balanced",
            PowerMode::HighPower => "high-power",
        };
        assert_eq!(
            name,
            ["unknown", "low-power", "balanced", "high-power"][index]
        );
    }
}

#[test]
fn downstream_window_options_preserve_default_and_independently_disabled_intervals() {
    let defaults = WindowOptions::default();
    assert_eq!(
        defaults.inactive_frame_interval,
        Some(Duration::from_micros(33_333))
    );
    assert_eq!(
        defaults.low_power_frame_interval,
        Some(Duration::from_micros(33_333))
    );
    assert_eq!(
        defaults.thermal_frame_interval,
        Some(Duration::from_micros(16_667))
    );

    let interval = Some(Duration::from_secs(1));
    for (inactive_frame_interval, low_power_frame_interval, thermal_frame_interval) in [
        (None, None, None),
        (interval, None, None),
        (None, interval, None),
        (None, None, interval),
        (
            Some(Duration::from_millis(100)),
            Some(Duration::from_millis(200)),
            Some(Duration::from_millis(300)),
        ),
    ] {
        let options = WindowOptions {
            inactive_frame_interval,
            low_power_frame_interval,
            thermal_frame_interval,
            ..Default::default()
        };
        let _: Option<Duration> = options.inactive_frame_interval;
        let _: Option<Duration> = options.low_power_frame_interval;
        let _: Option<Duration> = options.thermal_frame_interval;
        assert_eq!(options.inactive_frame_interval, inactive_frame_interval);
        assert_eq!(options.low_power_frame_interval, low_power_frame_interval);
        assert_eq!(options.thermal_frame_interval, thermal_frame_interval);
    }
}

#[test]
fn downstream_platform_power_signature_accepts_a_shared_send_sync_callback() {
    let _platform_type_only: fn(&dyn Platform) -> PowerMode = |platform| {
        let callback: Arc<dyn Fn() + Send + Sync> = Arc::new(|| {});
        let retained_callback = Arc::clone(&callback);
        platform.on_power_mode_change(retained_callback);
        platform.power_mode()
    };
}

#[test]
fn downstream_app_power_query_and_observer_signatures_typecheck_without_starting_an_app() {
    let _app_type_only: fn(&App) -> (PowerMode, Subscription) = |app| {
        let mode: PowerMode = app.power_mode();
        let subscription: Subscription = app.on_power_mode_change(|app: &mut App| {
            let _: PowerMode = app.power_mode();
        });
        (mode, subscription)
    };
}

#[cfg(feature = "profiler")]
#[test]
fn downstream_low_power_skip_reason_is_distinct_from_existing_skip_reasons() {
    use open_gpui::profiler::journal::FrameSkipReason;

    for other in [
        FrameSkipReason::NoRenderNeeded,
        FrameSkipReason::InactiveFrameRateLimit,
        FrameSkipReason::ThermalFrameRateLimit,
    ] {
        assert_ne!(FrameSkipReason::LowPowerFrameRateLimit, other);
    }
}
