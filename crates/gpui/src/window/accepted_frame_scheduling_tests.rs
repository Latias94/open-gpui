use crate::{
    AnyWindowHandle, AppContext as _, Context, Entity, IntoElement, ParentElement as _,
    PlatformFrameDeliveryMode, PlatformWindowPresentOutcome, PrepaintPublicationId, Render,
    StyleRefinement, Styled as _, TestAppContext, TestWindow, Window,
    WindowInitialPresentationStatus, blue, canvas, div, fill, px, red,
};
use std::{
    cell::Cell,
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
};

#[derive(Clone, Copy, PartialEq, Eq)]
enum PanicStage {
    Render,
    Publication,
}

#[derive(Default)]
struct Controls {
    stage: Cell<Option<PanicStage>>,
    renders: Cell<usize>,
    blue: Cell<bool>,
}

impl Controls {
    fn check(&self, stage: PanicStage) {
        if self.stage.get() == Some(stage) {
            self.stage.set(None);
            panic!("injected scheduled frame failure");
        }
    }
}

struct UnwindRoot(Rc<Controls>);

impl Render for UnwindRoot {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        self.0.renders.set(self.0.renders.get() + 1);
        self.0.check(PanicStage::Render);
        let controls = self.0.clone();
        let color = if self.0.blue.get() { blue() } else { red() };
        div().size_full().child(
            canvas(
                move |_, window, _| {
                    if controls.stage.get() == Some(PanicStage::Publication) {
                        let controls = controls.clone();
                        window.record_prepaint_window_transaction(
                            PrepaintPublicationId::new(),
                            move |fence, window, _| {
                                assert!(fence.is_satisfied_by(window));
                                controls.check(PanicStage::Publication);
                            },
                            |_, _, _| {},
                        );
                    }
                },
                move |bounds, (), window, _| window.paint_quad(fill(bounds, color)),
            )
            .size_full(),
        )
    }
}

struct CachedPublication {
    publication: PrepaintPublicationId,
    records: Rc<Cell<usize>>,
    commits: Rc<Cell<usize>>,
}

impl Render for CachedPublication {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let publication = self.publication;
        let records = self.records.clone();
        let commits = self.commits.clone();
        canvas(
            move |_, window, _| {
                records.set(records.get() + 1);
                window.record_prepaint_window_transaction(
                    publication,
                    move |fence, window, _| {
                        assert!(fence.is_satisfied_by(window));
                        commits.set(commits.get() + 1);
                    },
                    |_, _, _| panic!("the retained publication was not removed"),
                );
            },
            |_, (), _, _| {},
        )
        .size_full()
    }
}

struct CachedPublicationRoot {
    retained: Entity<CachedPublication>,
    trial: Option<PrepaintPublicationId>,
    abandoned: Rc<Cell<usize>>,
}

impl Render for CachedPublicationRoot {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let trial = self.trial.take();
        let abandoned = self.abandoned.clone();
        div()
            .size_full()
            .child(
                canvas(
                    move |_, window, _| {
                        if let Some(publication) = trial {
                            let result = window.transact(|window| {
                                let attempt = window
                                    .begin_prepaint_publication_attempt(publication, move |_| {
                                        abandoned.set(abandoned.get() + 1)
                                    });
                                window.record_prepaint_window_transaction_for_attempt(
                                    attempt,
                                    |_, _, _| panic!("the rolled-back record cannot commit"),
                                    |_, _, _| panic!("the rolled-back record cannot retire"),
                                );
                                Err::<(), _>(())
                            });
                            assert_eq!(result, Err(()));
                        }
                    },
                    |_, (), _, _| {},
                )
                .size(px(4.0)),
            )
            .child(
                self.retained
                    .clone()
                    .cached(StyleRefinement::default().size(px(60.0))),
            )
    }
}

#[crate::test]
fn cached_publication_cannot_settle_a_rolled_back_replacement(cx: &mut TestAppContext) {
    let publication = PrepaintPublicationId::new();
    let records = Rc::new(Cell::new(0));
    let commits = Rc::new(Cell::new(0));
    let abandoned = Rc::new(Cell::new(0));
    let retained = cx.new(|_| CachedPublication {
        publication,
        records: records.clone(),
        commits: commits.clone(),
    });
    let window = cx.add_window(|_, _| CachedPublicationRoot {
        retained: retained.clone(),
        trial: None,
        abandoned: abandoned.clone(),
    });
    window.update(cx, |_, _, cx| cx.notify()).unwrap();
    let before = (records.get(), commits.get());
    window
        .update(cx, |root, _, cx| {
            root.trial = Some(publication);
            cx.notify();
        })
        .unwrap();
    cx.run_until_parked();

    assert_eq!(records.get(), before.0, "prepaint must reuse its journal");
    assert_eq!(commits.get(), before.1 + 1);
    assert_eq!(abandoned.get(), 1, "the trial must release its own work");
}

fn open_parked_window(cx: &mut TestAppContext) -> (AnyWindowHandle, TestWindow, Rc<Controls>) {
    let controls = Rc::new(Controls::default());
    cx.defer_next_window_frame_requests();
    let handle: AnyWindowHandle = cx
        .add_window({
            let controls = controls.clone();
            move |_, _| UnwindRoot(controls)
        })
        .into();
    let platform = cx.test_window(handle);
    cx.update_window(handle, |_, window, app| {
        window.active.set(true);
        window.draw(app).clear(app);
    })
    .unwrap();
    assert!(platform.simulate_scheduled_frame());
    cx.run_until_parked();
    assert!(platform.simulate_scheduled_frame());
    cx.run_until_parked();
    assert!(
        !platform.frame_scheduled(),
        "startup must settle before injecting a failure"
    );
    assert!(!platform.simulate_scheduled_frame());
    {
        let app = cx.app.borrow();
        let window = app
            .windows
            .get(handle.window_id())
            .unwrap()
            .as_ref()
            .unwrap();
        let facts = window.presentation_facts();
        assert_eq!(
            facts.initial_presentation,
            WindowInitialPresentationStatus::Completed
        );
        assert!(facts.native_visible);
        assert!(facts.frame_accepted_generation.is_some());
        assert_eq!(
            facts.frame_accepted_generation,
            facts.present_submitted_generation
        );
        assert_eq!(
            facts.frame_accepted_generation,
            facts.non_empty_presented_generation
        );
        assert!(
            facts
                .latest_present_attempt
                .unwrap()
                .contained_valid_primitives
        );
        assert!(!window.invalidator.is_dirty());
        assert!(!window.needs_present.get());
    }
    (handle, platform, controls)
}

fn deliver_scheduled_frame(cx: &TestAppContext, platform: &TestWindow) {
    assert!(platform.simulate_scheduled_frame());
    assert!(
        !platform.frame_scheduled(),
        "the wake must be consumed before drawing"
    );
    cx.run_until_parked();
}

#[crate::test]
fn retained_presentation_coalesces_and_survives_a_pending_deferred_delivery(
    cx: &mut TestAppContext,
) {
    let (handle, platform, controls) = open_parked_window(cx);
    let draws = platform.draw_count();
    let renders = controls.renders.get();
    let (generation, submissions) = cx
        .update_window(handle, |_, window, _| {
            window.request_retained_presentation();
            window.request_retained_presentation();
            (
                window.rendered_frame_revision(),
                window.present_submission_count(),
            )
        })
        .unwrap();
    deliver_scheduled_frame(cx, &platform);
    assert_eq!(platform.draw_count(), draws + 1);
    // TestWindow schedules a compositor callback after each draw. With no retained demand
    // remaining, that callback must neither draw again nor leave the frame pump armed.
    deliver_scheduled_frame(cx, &platform);
    assert_eq!(platform.draw_count(), draws + 1);
    assert!(!platform.frame_scheduled());

    platform.set_frame_delivery_mode(PlatformFrameDeliveryMode::Deferred);
    cx.update_window(handle, |_, window, _| {
        window.request_retained_presentation()
    })
    .unwrap();
    deliver_scheduled_frame(cx, &platform);
    let first = platform.pending_frame_submission().unwrap();
    assert_eq!(first.frame_generation(), generation);
    assert_eq!(platform.draw_count(), draws + 2);
    deliver_scheduled_frame(cx, &platform);
    assert_eq!(platform.draw_count(), draws + 2);

    cx.update_window(handle, |_, window, _| {
        window.request_retained_presentation()
    })
    .unwrap();
    assert!(!platform.frame_scheduled());
    assert!(first.complete(PlatformWindowPresentOutcome::Submitted));
    cx.run_until_parked();
    deliver_scheduled_frame(cx, &platform);
    let second = platform.pending_frame_submission().unwrap();
    assert_eq!(second.frame_generation(), generation);
    assert!(second.attempt_id() > first.attempt_id());
    deliver_scheduled_frame(cx, &platform);
    assert!(second.complete(PlatformWindowPresentOutcome::Submitted));
    cx.run_until_parked();

    assert_eq!(platform.draw_count(), draws + 3);
    assert_eq!(controls.renders.get(), renders);
    assert!(!platform.frame_scheduled());
    cx.update_window(handle, |_, window, _| {
        assert_eq!(window.rendered_frame_revision(), generation);
        assert_eq!(window.present_submission_count(), submissions + 3);
        assert!(!window.invalidator.is_dirty());
    })
    .unwrap();
}

#[crate::test]
fn accepted_publication_unwind_retries_the_same_scheduled_frame_then_parks(
    cx: &mut TestAppContext,
) {
    let (handle, platform, controls) = open_parked_window(cx);
    let (baseline, invalidator) = cx
        .update_window(handle, |_, window, _| {
            (window.presentation_facts(), window.invalidator.clone())
        })
        .unwrap();
    controls.blue.set(true);
    invalidator.set_dirty(true);
    controls.stage.set(Some(PanicStage::Publication));
    assert!(invalidator.is_dirty());
    assert!(platform.frame_scheduled());
    let panic = catch_unwind(AssertUnwindSafe(|| deliver_scheduled_frame(cx, &platform)))
        .expect_err("the accepted publication must unwind through the native callback");
    assert_eq!(
        panic.downcast_ref::<&str>(),
        Some(&"injected scheduled frame failure")
    );
    let generation = baseline.frame_accepted_generation.unwrap() + 1;
    let renders = controls.renders.get();
    {
        let app = cx.app.borrow();
        let window = app
            .windows
            .get(handle.window_id())
            .unwrap()
            .as_ref()
            .unwrap();
        let facts = window.presentation_facts();
        assert_eq!(window.rendered_frame_revision(), generation);
        assert_eq!(facts.frame_accepted_generation, Some(generation));
        assert_eq!(
            facts.present_submitted_generation,
            baseline.present_submitted_generation
        );
        assert_eq!(
            facts.non_empty_presented_generation,
            baseline.non_empty_presented_generation
        );
        assert_eq!(
            facts.latest_present_attempt,
            baseline.latest_present_attempt
        );
        assert_eq!(
            window.rendered_frame.scene.quads[0].background.solid,
            blue()
        );
        assert!(window.candidate_frame_transaction.is_none());
        assert!(!window.frame_focus_authority_sealed);
        assert!(!window.invalidator.is_building_frame());
        assert!(!window.invalidator.is_dirty());
        assert!(window.needs_present.get());
    }
    assert!(
        platform.frame_scheduled(),
        "accepted work must retain a presentation retry after unwind"
    );
    deliver_scheduled_frame(cx, &platform);
    {
        let app = cx.app.borrow();
        let window = app
            .windows
            .get(handle.window_id())
            .unwrap()
            .as_ref()
            .unwrap();
        let facts = window.presentation_facts();
        assert_eq!(window.rendered_frame_revision(), generation);
        assert_eq!(facts.frame_accepted_generation, Some(generation));
        assert_eq!(facts.present_submitted_generation, Some(generation));
        assert_eq!(facts.non_empty_presented_generation, Some(generation));
        assert_eq!(
            facts.latest_present_attempt.unwrap().outcome,
            PlatformWindowPresentOutcome::Submitted
        );
        assert!(!window.invalidator.is_dirty());
        assert!(!window.needs_present.get());
    }
    assert_eq!(
        controls.renders.get(),
        renders,
        "presentation retry must reuse the accepted scene"
    );
    assert!(
        platform.frame_scheduled(),
        "Submitted must await its compositor callback"
    );
    deliver_scheduled_frame(cx, &platform);
    cx.run_until_parked();
    assert!(!platform.frame_scheduled());
    assert!(!platform.simulate_scheduled_frame());
    assert_eq!(controls.renders.get(), renders);
}

#[crate::test]
fn render_unwind_keeps_the_old_scene_until_a_scheduled_retry_then_parks(cx: &mut TestAppContext) {
    let (handle, platform, controls) = open_parked_window(cx);
    let (baseline, invalidator) = cx
        .update_window(handle, |_, window, _| {
            (window.presentation_facts(), window.invalidator.clone())
        })
        .unwrap();
    controls.blue.set(true);
    invalidator.set_dirty(true);
    controls.stage.set(Some(PanicStage::Render));
    assert!(invalidator.is_dirty());
    assert!(platform.frame_scheduled());
    let panic = catch_unwind(AssertUnwindSafe(|| deliver_scheduled_frame(cx, &platform)))
        .expect_err("render must unwind through the native callback");
    assert_eq!(
        panic.downcast_ref::<&str>(),
        Some(&"injected scheduled frame failure")
    );
    let renders = controls.renders.get();
    {
        let app = cx.app.borrow();
        let window = app
            .windows
            .get(handle.window_id())
            .unwrap()
            .as_ref()
            .unwrap();
        assert_eq!(window.presentation_facts(), baseline);
        assert_eq!(window.rendered_frame.scene.quads[0].background.solid, red());
        assert!(window.candidate_frame_transaction.is_none());
        assert!(!window.frame_focus_authority_sealed);
        assert!(!window.invalidator.is_building_frame());
        assert!(window.invalidator.is_dirty());
        assert!(!window.needs_present.get());
    }
    assert!(
        platform.frame_scheduled(),
        "a rejected candidate must retain its dirty-frame wake"
    );
    deliver_scheduled_frame(cx, &platform);
    {
        let app = cx.app.borrow();
        let window = app
            .windows
            .get(handle.window_id())
            .unwrap()
            .as_ref()
            .unwrap();
        let facts = window.presentation_facts();
        let generation = baseline.frame_accepted_generation.unwrap() + 1;
        assert_eq!(window.rendered_frame_revision(), generation);
        assert_eq!(facts.frame_accepted_generation, Some(generation));
        assert_eq!(facts.present_submitted_generation, Some(generation));
        assert_eq!(facts.non_empty_presented_generation, Some(generation));
        assert_eq!(
            facts.latest_present_attempt.unwrap().outcome,
            PlatformWindowPresentOutcome::Submitted
        );
        assert_eq!(
            window.rendered_frame.scene.quads[0].background.solid,
            blue()
        );
        assert!(!window.invalidator.is_dirty());
        assert!(!window.needs_present.get());
    }
    assert_eq!(controls.renders.get(), renders + 1);
    assert!(platform.frame_scheduled());
    deliver_scheduled_frame(cx, &platform);
    cx.run_until_parked();
    assert!(!platform.frame_scheduled());
    assert!(!platform.simulate_scheduled_frame());
    assert_eq!(controls.renders.get(), renders + 1);
}
