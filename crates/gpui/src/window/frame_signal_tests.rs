use crate::{
    Context, FrameRequestSource, IntoElement, Render, RequestFrameOptions, TestAppContext,
    TestWindow, Window, div,
    profiler::journal::{self, ForegroundJournalEntry, IntervalBoundary},
};
use open_gpui_scheduler::Instant;
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

struct ReentrantFrameRoot {
    renders: Rc<Cell<usize>>,
    request: Rc<RefCell<Option<(TestWindow, RequestFrameOptions)>>>,
}

impl Render for ReentrantFrameRoot {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        self.renders.set(self.renders.get() + 1);
        if let Some((platform, options)) = self.request.borrow_mut().take() {
            assert!(platform.simulate_frame(options));
        }
        div()
    }
}

#[open_gpui::test]
fn reentrant_signal_is_recorded_before_delivery_and_keeps_its_deferred_force(
    cx: &mut TestAppContext,
) {
    let (journal, _guard) = journal::install_test_foreground_journal(256, 8);
    cx.defer_next_window_frame_requests();
    let renders = Rc::new(Cell::new(0));
    let request = Rc::new(RefCell::new(None));
    let (_, visual) = cx.add_window_view({
        let renders = renders.clone();
        let request = request.clone();
        move |_, _| ReentrantFrameRoot { renders, request }
    });
    let handle = visual.update(|window, _| window.window_handle());
    let platform = visual.test_window(handle);
    visual.update(|window, cx| {
        window.active.set(true);
        window.draw(cx).clear(cx);
        window.present(cx);
    });
    visual.run_until_parked();
    let mut collector = journal.collector();
    collector.collect_unseen();
    let signal_at = Instant::now();
    *request.borrow_mut() = Some((
        platform,
        RequestFrameOptions {
            force_render: true,
            require_presentation: true,
            signal_at: Some(signal_at),
            signal_source: FrameRequestSource::LocalSchedule,
        },
    ));
    let before = renders.get();
    visual.update(|window, cx| {
        let _turn = journal::foreground_turn();
        window.refresh();
        window.draw(cx).clear(cx);
        assert_eq!(renders.get(), before + 1);
        let during_draw = collector.collect_unseen();
        assert_eq!(during_draw.lost, 0);
        assert_eq!(
            during_draw
                .entries
                .iter()
                .filter(|entry| matches!(
                    entry,
                    ForegroundJournalEntry::PlatformSignal(signal)
                        if signal.window_id == handle.window_id()
                            && signal.signal_at == Some(signal_at)
                            && signal.source == FrameRequestSource::LocalSchedule
                            && signal.handled_at >= signal_at
                ))
                .count(),
            1
        );
        assert!(during_draw.entries.iter().any(|entry| matches!(
            entry,
            ForegroundJournalEntry::ReentrantFrameSkipped { window_id, .. }
                if *window_id == handle.window_id()
        )));
        assert!(during_draw.entries.iter().all(|entry| !matches!(
            entry,
            ForegroundJournalEntry::Boundary(IntervalBoundary::FrameSkipped(_))
        )));
        window.present(cx);
    });
    visual.run_until_parked();
    assert_eq!(renders.get(), before + 2);
    let after_delivery = collector.collect_unseen();
    assert_eq!(after_delivery.lost, 0);
    assert!(after_delivery.entries.iter().all(|entry| !matches!(
        entry,
        ForegroundJournalEntry::PlatformSignal(signal)
            if signal.signal_at == Some(signal_at)
    )));
    assert!(after_delivery.entries.iter().any(|entry| matches!(
        entry,
        ForegroundJournalEntry::Boundary(IntervalBoundary::Presented(frame))
            if frame.frame.window_id == handle.window_id()
    )));
}
