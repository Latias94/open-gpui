use open_gpui::{
    App, Context, DisplayChanges, DisplayEvent, DisplayId, Subscription, Window,
    refresh_interval_from_hz,
};
use std::time::Duration;

struct Consumer {
    last_refresh_interval: Option<Duration>,
}

#[test]
fn downstream_observes_display_events_and_window_refresh_without_test_support() {
    let _: fn(&Window, &App) -> Option<Duration> = Window::refresh_interval;
    let _consumer = |app: &App, window: &mut Window, cx: &mut Context<Consumer>| {
        let payload = String::from("owned display observer");
        let app_subscription: Subscription = app.observe_displays(move |event, app| {
            let _: &str = &payload;
            match event {
                DisplayEvent::Added(id) => {
                    let _: DisplayId = id;
                    let _ = app.find_display(id);
                }
                DisplayEvent::Removed(id) => {
                    let _: DisplayId = id;
                }
                DisplayEvent::Changed { id, changes } => {
                    let _: DisplayId = id;
                    let _: bool = changes.contains(DisplayChanges::BOUNDS);
                    let _: bool = changes.contains(DisplayChanges::REFRESH_INTERVAL);
                }
                _ => {}
            }
        });
        let window_subscription: Subscription = window.observe_window_display(|window, app| {
            let _: Option<Duration> = window.refresh_interval(app);
        });
        let entity_subscription: Subscription = cx
            .observe_window_display(window, |consumer, window, cx| {
                consumer.last_refresh_interval = window.refresh_interval(cx)
            });
        (app_subscription, window_subscription, entity_subscription)
    };
}

#[test]
fn downstream_refresh_rate_conversion_rejects_invalid_or_unrepresentable_intervals() {
    assert_eq!(
        refresh_interval_from_hz(60.0),
        Some(Duration::from_secs_f64(1.0 / 60.0))
    );
    assert_eq!(
        refresh_interval_from_hz(59.94),
        Some(Duration::from_secs_f64(1.0 / 59.94))
    );
    for hertz in [
        0.0,
        -0.0,
        -60.0,
        f64::NAN,
        f64::INFINITY,
        f64::NEG_INFINITY,
        f64::MIN_POSITIVE,
        f64::from_bits(1),
    ] {
        assert_eq!(
            refresh_interval_from_hz(hertz),
            None,
            "invalid refresh rate: {hertz}"
        );
    }
}
