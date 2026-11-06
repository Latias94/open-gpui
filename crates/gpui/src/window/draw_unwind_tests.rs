use crate::{
    AppContext as _, Context, InteractiveElement as _, IntoElement, ParentElement as _,
    PlatformWindowPresentOutcome, PrepaintPublicationId, Render, Styled as _, TestAppContext,
    TextStyleRefinement, Window, blue, canvas, div, fill, point, px, red,
};
use std::{
    cell::Cell,
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PanicStage {
    Render,
    Prepaint,
    Paint,
    Publication,
}

#[derive(Default)]
struct Controls {
    stage: Cell<Option<PanicStage>>,
    blue: Cell<bool>,
}

impl Controls {
    fn check(&self, stage: PanicStage) {
        if self.stage.get() == Some(stage) {
            self.stage.set(None);
            panic!("injected candidate draw failure");
        }
    }
}

struct UnwindRoot(Rc<Controls>);

impl Render for UnwindRoot {
    fn render(&mut self, window: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        window.with_element_namespace("render-scope", |window| {
            window.with_rem_size(Some(px(23.)), |window| {
                window.with_text_style(Some(TextStyleRefinement::default()), |_| {
                    self.0.check(PanicStage::Render);
                })
            })
        });
        let prepaint = self.0.clone();
        let paint = self.0.clone();
        let color = if self.0.blue.get() { blue() } else { red() };
        div().id("candidate-unwind-root").size_full().child(
            canvas(
                move |_, window, _| {
                    window.with_absolute_element_offset(point(px(7.), px(9.)), |_| {
                        prepaint.check(PanicStage::Prepaint);
                    });
                    if prepaint.stage.get() == Some(PanicStage::Publication) {
                        let controls = prepaint.clone();
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
                move |bounds, (), window, _| {
                    window.with_element_opacity(Some(0.5), |window| {
                        paint.check(PanicStage::Paint);
                        window.paint_quad(fill(bounds, color));
                    });
                },
            )
            .size_full(),
        )
    }
}

#[open_gpui::test]
fn accepted_publication_unwind_finishes_the_frame_and_allows_redraw(cx: &mut TestAppContext) {
    let controls = Rc::new(Controls::default());
    let handle = cx.add_window({
        let controls = controls.clone();
        move |_, _| UnwindRoot(controls)
    });
    cx.test_window(handle.into())
        .defer_frame_requests_for_test();
    cx.update(|app| {
        app.update_window(handle.into(), |_, window, app| {
            window.draw(app).clear(app);
            assert_eq!(window.present(app), PlatformWindowPresentOutcome::Submitted);
            let previous = window.rendered_frame_revision();
            controls.blue.set(true);
            controls.stage.set(Some(PanicStage::Publication));
            window.refresh();
            let failed = catch_unwind(AssertUnwindSafe(|| window.draw(app).clear(app)))
                .expect_err("the actual accepted publication must unwind");
            assert_eq!(
                failed.downcast_ref::<&str>(),
                Some(&"injected candidate draw failure")
            );
            assert_eq!(window.rendered_frame_revision(), previous + 1);
            assert_eq!(
                window.presentation_facts().frame_accepted_generation,
                Some(previous + 1)
            );
            assert!(window.candidate_frame_transaction.is_none());
            assert!(!window.frame_focus_authority_sealed);
            assert!(!window.invalidator.is_building_frame());
            assert_eq!(
                window.rendered_frame.scene.quads[0].background.solid,
                blue().opacity(0.5)
            );
            assert_eq!(window.present(app), PlatformWindowPresentOutcome::Submitted);
            window.refresh();
            window.draw(app).clear(app);
            assert_eq!(window.rendered_frame_revision(), previous + 2);
            assert_eq!(window.present(app), PlatformWindowPresentOutcome::Submitted);
            assert_eq!(
                window.presentation_facts().present_submitted_generation,
                Some(previous + 2)
            );
        })
        .unwrap();
    });
}

#[open_gpui::test]
fn candidate_render_prepaint_and_paint_unwind_preserve_the_last_scene_and_recover(
    cx: &mut TestAppContext,
) {
    let controls = Rc::new(Controls::default());
    let handle = cx.add_window({
        let controls = controls.clone();
        move |_, _| UnwindRoot(controls)
    });
    cx.test_window(handle.into())
        .defer_frame_requests_for_test();
    cx.update(|app| {
        app.update_window(handle.into(), |_, window, app| {
            window.draw(app).clear(app);
            assert_eq!(window.present(app), PlatformWindowPresentOutcome::Submitted);
            for stage in [PanicStage::Render, PanicStage::Prepaint, PanicStage::Paint] {
                let previous = window.rendered_frame_revision();
                let previous_color = window.rendered_frame.scene.quads[0].background.solid;
                controls.blue.set(!controls.blue.get());
                controls.stage.set(Some(stage));
                window.refresh();
                let failed = catch_unwind(AssertUnwindSafe(|| window.draw(app).clear(app)))
                    .expect_err("the actual candidate must unwind");
                assert_eq!(
                    failed.downcast_ref::<&str>(),
                    Some(&"injected candidate draw failure")
                );
                assert_eq!(window.rendered_frame_revision(), previous);
                assert_eq!(window.rendered_frame.scene.quads.len(), 1);
                assert_eq!(
                    window.rendered_frame.scene.quads[0].background.solid,
                    previous_color
                );
                assert!(window.candidate_frame_transaction.is_none());
                assert!(window.rendered_entity_stack.is_empty());
                assert!(window.element_id_stack.is_empty());
                assert!(window.text_style_stack.is_empty());
                assert!(window.element_offset_stack.is_empty());
                assert!(window.rem_size_override_stack.is_empty());
                assert_eq!(window.element_opacity, 1.);
                assert!(window.refresh_pending_for_test());
                window.draw(app).clear(app);
                assert_eq!(window.rendered_frame_revision(), previous + 1);
                assert_ne!(
                    window.rendered_frame.scene.quads[0].background.solid,
                    previous_color
                );
                assert_eq!(window.present(app), PlatformWindowPresentOutcome::Submitted);
                assert_eq!(
                    window.presentation_facts().present_submitted_generation,
                    Some(previous + 1)
                );
            }
        })
        .unwrap();
    });
}
