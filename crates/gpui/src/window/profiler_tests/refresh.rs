use super::*;
use crate::{AnyView, AnyWeakView, Entity, RequestFrameOptions};
use profiler::{ViewPhase, ViewTiming, journal::ForegroundJournalEntry};
use std::{any::type_name, time::Duration};

struct TimedLeaf {
    fail: Rc<Cell<Option<ViewPhase>>>,
}

impl Render for TimedLeaf {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        std::thread::sleep(Duration::from_millis(1));
        assert_ne!(
            self.fail.get(),
            Some(ViewPhase::RequestLayout),
            "timed leaf phase failure"
        );
        let prepaint_fail = self.fail.clone();
        let paint_fail = self.fail.clone();
        canvas(
            move |_, _, _| {
                std::thread::sleep(Duration::from_millis(1));
                assert_ne!(
                    prepaint_fail.get(),
                    Some(ViewPhase::Prepaint),
                    "timed leaf phase failure"
                );
            },
            move |bounds, (), window, _| {
                std::thread::sleep(Duration::from_millis(1));
                assert_ne!(
                    paint_fail.get(),
                    Some(ViewPhase::Paint),
                    "timed leaf phase failure"
                );
                window.paint_quad(fill(bounds, red()));
            },
        )
        .size_full()
    }
}

struct TimedParent {
    child: Entity<TimedLeaf>,
    route: Rc<Cell<u8>>,
}

impl Render for TimedParent {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        std::thread::sleep(Duration::from_millis(1));
        let child = match self.route.get() {
            0 => self.child.clone().into_element(),
            1 => AnyView::from(self.child.clone())
                .downgrade()
                .upgrade()
                .unwrap(),
            2 => AnyWeakView::from(self.child.downgrade()).upgrade().unwrap(),
            _ => AnyView::from(self.child.clone())
                .downcast::<crate::EmptyView>()
                .unwrap_err(),
        };
        div()
            .size_full()
            .child(
                canvas(
                    |_, _, _| std::thread::sleep(Duration::from_millis(1)),
                    |_, (), _, _| std::thread::sleep(Duration::from_millis(1)),
                )
                .absolute()
                .size(crate::px(1.)),
            )
            .child(child)
    }
}

fn views(entries: Vec<ForegroundJournalEntry>) -> Vec<ViewTiming> {
    entries
        .into_iter()
        .filter_map(|entry| match entry {
            ForegroundJournalEntry::Event(profiler::journal::ForegroundEvent::View(view)) => {
                Some(view)
            }
            _ => None,
        })
        .collect()
}

fn assert_nested_self_time(timings: &[ViewTiming], phase: ViewPhase) {
    let timing_for = |name| {
        let matches = timings
            .iter()
            .filter(|timing| timing.name == name && timing.phase == phase)
            .collect::<Vec<_>>();
        assert_eq!(matches.len(), 1, "{name}: {timings:?}");
        *matches[0]
    };
    let parent = timing_for(type_name::<TimedParent>());
    let child = timing_for(type_name::<TimedLeaf>());
    assert!(parent.start <= child.start && parent.end >= child.end);
    assert_eq!(child.self_duration, child.end.duration_since(child.start));
    assert_eq!(
        parent.self_duration,
        parent
            .end
            .duration_since(parent.start)
            .saturating_sub(child.end.duration_since(child.start))
    );
}

#[crate::test]
fn actual_view_phases_keep_erased_weak_and_failed_downcast_type_attribution(
    cx: &mut TestAppContext,
) {
    let fail = Rc::new(Cell::new(None));
    let route = Rc::new(Cell::new(0));
    let child = cx.new(|_| TimedLeaf { fail });
    cx.defer_next_window_frame_requests();
    let handle = cx.add_window(|_, _| TimedParent {
        child,
        route: route.clone(),
    });
    cx.test_window(handle.into())
        .defer_frame_requests_for_test();
    cx.run_until_parked();
    let _batch = ManualFrameBatch::new(cx);
    let mut collector = cx.read(|app| app.foreground_journal()).collector();
    for selected in 0..4 {
        route.set(selected);
        collector.collect_unseen();
        cx.update_window(handle.into(), |_, window, app| {
            window.refresh();
            window.draw(app).clear(app);
        })
        .unwrap();
        let drained = collector.collect_unseen();
        assert_eq!(drained.lost, 0);
        let timings = views(drained.entries);
        for phase in [
            ViewPhase::RequestLayout,
            ViewPhase::Prepaint,
            ViewPhase::Paint,
        ] {
            assert_nested_self_time(&timings, phase);
            assert!(
                timings.iter().any(|timing| {
                    timing.name == type_name::<TimedLeaf>()
                        && timing.phase == phase
                        && timing.self_duration >= profiler::VIEW_TIMING_FLOOR
                }),
                "route {selected}, phase {phase:?}: {timings:?}"
            );
        }
    }
}

#[crate::test]
fn actual_nested_view_unwind_balances_the_next_draw_without_accepting_failed_work(
    cx: &mut TestAppContext,
) {
    let fail = Rc::new(Cell::new(None));
    let route = Rc::new(Cell::new(0));
    let child = cx.new(|_| TimedLeaf { fail: fail.clone() });
    cx.defer_next_window_frame_requests();
    let handle = cx.add_window(|_, _| TimedParent { child, route });
    cx.test_window(handle.into())
        .defer_frame_requests_for_test();
    cx.run_until_parked();
    let _batch = ManualFrameBatch::new(cx);
    let mut baseline = cx
        .update_window(handle.into(), |_, window, app| {
            window.draw(app).clear(app);
            window.presentation_facts()
        })
        .unwrap();
    let mut collector = cx.read(|app| app.foreground_journal()).collector();
    for failing_phase in [
        ViewPhase::RequestLayout,
        ViewPhase::Prepaint,
        ViewPhase::Paint,
    ] {
        fail.set(Some(failing_phase));
        let panic = catch_unwind(AssertUnwindSafe(|| {
            cx.update_window(handle.into(), |_, window, app| {
                window.refresh();
                window.draw(app).clear(app);
            })
            .unwrap();
        }))
        .expect_err("the nested view must propagate its phase panic");
        let message = panic
            .downcast_ref::<String>()
            .map(String::as_str)
            .or_else(|| panic.downcast_ref::<&str>().copied())
            .unwrap();
        assert!(message.contains("timed leaf phase failure"), "{message}");
        cx.update_window(handle.into(), |_, window, _| {
            assert_eq!(window.presentation_facts(), baseline);
        })
        .unwrap();
        let failed = collector.collect_unseen();
        assert_eq!(failed.lost, 0);
        assert_nested_self_time(&views(failed.entries), failing_phase);
        fail.set(None);
        cx.update_window(handle.into(), |_, window, app| {
            window.draw(app).clear(app);
            assert_eq!(
                window.presentation_facts().frame_accepted_generation,
                baseline
                    .frame_accepted_generation
                    .map(|generation| generation + 1)
            );
            assert_eq!(
                window.presentation_facts().present_submitted_generation,
                baseline.present_submitted_generation
            );
        })
        .unwrap();
        let recovered = collector.collect_unseen();
        assert_eq!(recovered.lost, 0);
        let timings = views(recovered.entries);
        for phase in [
            ViewPhase::RequestLayout,
            ViewPhase::Prepaint,
            ViewPhase::Paint,
        ] {
            assert_nested_self_time(&timings, phase);
        }
        baseline = cx
            .update_window(handle.into(), |_, window, _| window.presentation_facts())
            .unwrap();
    }
}

#[crate::test]
fn completed_no_render_callback_does_not_lend_its_signal_to_a_later_direct_draw(
    cx: &mut TestAppContext,
) {
    let fixture = WindowFixture::new(cx);
    let journal = cx.read(|app| app.foreground_journal());
    let mut collector = journal.collector();
    fixture.update(cx, |window, app| {
        window.native_frame_requested(
            RequestFrameOptions {
                signal_at: Some(Instant::now()),
                ..Default::default()
            },
            app,
        );
    });
    assert!(
        !collector
            .collect_unseen()
            .entries
            .iter()
            .any(|entry| matches!(
                entry,
                ForegroundJournalEntry::Event(profiler::journal::ForegroundEvent::Draw(_))
            ))
    );
    fixture.draw(cx);
    let draws = collector
        .collect_unseen()
        .entries
        .into_iter()
        .filter_map(|entry| match entry {
            ForegroundJournalEntry::Event(profiler::journal::ForegroundEvent::Draw(draw)) => {
                Some(draw)
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(draws.len(), 1);
    assert_eq!(draws[0].signal_at, None);
    assert_eq!(draws[0].work(Instant::now()), None);
}
