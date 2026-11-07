use super::*;
use crate::{
    Context, InteractiveElement as _, IntoElement, MouseButton, MouseDownEvent, PlatformInput,
    PlatformWindowPresentOutcome, Render, RequestFrameOptions, Styled as _, TestAppContext, Window,
    div, point, px,
};

struct InputRoot;

impl Render for InputRoot {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .bg(crate::red())
            .on_mouse_down(MouseButton::Left, |_, window, _| window.refresh())
    }
}

#[crate::test]
fn high_rate_input_alone_obeys_power_limits_reuses_the_scene_and_expires(cx: &mut TestAppContext) {
    cx.simulate_power_mode(PowerMode::LowPower);
    let (_, visual) = cx.add_window_view(|_, _| InputRoot);
    visual.run_until_parked();
    let handle = visual.update(|window, _| window.window_handle());
    let platform = visual.test_window(handle);
    platform.defer_frame_requests_for_test();
    visual.update(|window, app| {
        window.low_power_frame_interval = Some(Duration::from_secs(1));
        window.active.set(true);
        for _ in 0..8 {
            window.dispatch_event(
                PlatformInput::MouseDown(MouseDownEvent {
                    button: MouseButton::Left,
                    position: point(px(8.), px(8.)),
                    click_count: 1,
                    ..Default::default()
                }),
                app,
            );
        }
        assert!(window.input_rate_tracker.borrow().is_high_rate());
        window.draw(app).clear(app);
        assert_eq!(window.present(app), PlatformWindowPresentOutcome::Submitted);
        window.next_frame_callbacks.borrow_mut().clear();
        window.invalidator.set_dirty(false);
        window.needs_present.set(false);
        window.active.set(false);
        let baseline = window.presentation_facts();
        let draws = platform.draw_count();
        let previous = Instant::now();
        window.frame_pacer.last_frame = Some(previous);
        window.native_frame_requested(RequestFrameOptions::default(), app);
        assert_eq!(platform.draw_count(), draws);
        assert_eq!(window.presentation_facts(), baseline);
        assert_eq!(window.frame_pacer.last_frame, Some(previous));

        window.frame_pacer.last_frame = Some(Instant::now() - Duration::from_secs(2));
        window.native_frame_requested(RequestFrameOptions::default(), app);
        assert_eq!(platform.draw_count(), draws + 1);
        assert_eq!(
            window.presentation_facts().frame_accepted_generation,
            baseline.frame_accepted_generation
        );
        assert_eq!(
            window.presentation_facts().present_submitted_generation,
            baseline.present_submitted_generation
        );
        let cadence = window.frame_pacer.last_frame;
        window.input_rate_tracker.borrow_mut().sustain_until =
            Instant::now() - Duration::from_secs(1);
        window.native_frame_requested(RequestFrameOptions::default(), app);
        assert_eq!(platform.draw_count(), draws + 1);
        assert_eq!(window.frame_pacer.last_frame, cadence);
    });
}

#[cfg(feature = "profiler")]
#[crate::test]
fn low_power_skip_does_not_lend_its_signal_to_a_later_direct_draw(cx: &mut TestAppContext) {
    use crate::profiler::journal::{ForegroundEvent, ForegroundJournalEntry};

    cx.simulate_power_mode(PowerMode::LowPower);
    let (_, visual) = cx.add_window_view(|_, _| InputRoot);
    visual.run_until_parked();
    visual.update(|window, app| {
        let mut collector = app.foreground_journal().collector();
        window.low_power_frame_interval = Some(Duration::from_secs(1));
        window.frame_pacer.last_frame = Some(Instant::now());
        window.refresh();
        let baseline = window.presentation_facts();
        window.native_frame_requested(
            RequestFrameOptions {
                signal_at: Some(Instant::now()),
                ..Default::default()
            },
            app,
        );
        assert_eq!(window.presentation_facts(), baseline);
        assert!(window.invalidator.is_dirty());
        collector.collect_unseen();
        window.draw(app).clear(app);
        let frames = collector
            .collect_unseen()
            .entries
            .into_iter()
            .filter_map(|entry| match entry {
                ForegroundJournalEntry::Event(ForegroundEvent::Draw(frame)) => Some(frame),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0].signal_at, None);
        assert_eq!(frames[0].work(Instant::now()), None);
    });
}
