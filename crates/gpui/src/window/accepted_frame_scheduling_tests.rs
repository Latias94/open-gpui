use crate::{
    AnyWindowHandle, AppContext as _, Context, IntoElement, ParentElement as _,
    PlatformWindowPresentOutcome, PrepaintPublicationId, Render, Styled as _, TestAppContext,
    TestWindow, Window, WindowInitialPresentationStatus, blue, canvas, div, fill, red,
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
