use crate::{
    App, AppContext as _, Bounds, Context, FocusHandle, InteractiveElement as _, IntoElement,
    ParentElement as _, PrepaintPublicationId, Render, Styled as _, SubtreeTransform,
    SubtreeTransformExt as _, SubtreeTransformOrigin, TestAppContext, Window, WindowHandle, canvas,
    div, fill, point, px, red, size,
};
use std::{
    cell::Cell,
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
};

#[derive(Clone, Copy, PartialEq)]
enum Mutation {
    Focus,
    UnwindPrepaint,
    UnwindPaint,
    LateGeometry,
    Rollback,
    QuiesceRollback,
    QuiesceUnwind,
    ProvisionalQuiesce,
    AppQuiesceBeforeBind,
    ProvisionalAppQuiesce,
}

#[derive(Default)]
struct Controls {
    mutation: Cell<Option<Mutation>>,
    published_generation: Cell<Option<u64>>,
}

struct FocusRoot {
    handles: [FocusHandle; 2],
    controls: Rc<Controls>,
}

impl Render for FocusRoot {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let mutation = self.controls.mutation.take();
        let target = self.handles[1].clone();
        let controls = self.controls.clone();
        div()
            .size_full()
            .child(
                canvas(
                    move |_, window, app| {
                        if mutation == Some(Mutation::AppQuiesceBeforeBind) {
                            assert!(app.quiesce_window_interaction_authority(
                                window.window_handle().window_id()
                            ));
                        }
                    },
                    |_, (), _, _| {},
                )
                .size(px(1.)),
            )
            .child(
                div()
                    .id("focus-a")
                    .track_focus(&self.handles[0])
                    .size(px(40.)),
            )
            .child(
                div()
                    .id("focus-b")
                    .track_focus(&self.handles[1])
                    .size(px(40.))
                    .child(
                        canvas(
                            move |_, window, app| {
                                let generation = window.focus_generation;
                                match mutation {
                                    Some(Mutation::Rollback | Mutation::QuiesceRollback) => {
                                        let focus = window.focus;
                                        let result: Result<(), ()> = window.transact(|window| {
                                            if mutation == Some(Mutation::QuiesceRollback) {
                                                assert!(window.quiesce_interaction(app));
                                            } else {
                                                window.focus(&target, app);
                                                assert_eq!(window.focus, Some(target.id));
                                            }
                                            Err(())
                                        });
                                        assert_eq!(result, Err(()));
                                        if mutation == Some(Mutation::QuiesceRollback) {
                                            assert!(!window.interaction_authority.is_admitted());
                                            assert!(window.focus.is_none(), "focus resurrected");
                                            assert!(window.next_frame.focus.is_none());
                                            assert!(window.pending_focus_claim.is_none());
                                            assert!(window.pending_blur_claim_generation.is_none());
                                            assert!(window.pending_focus_completion.is_none());
                                        } else {
                                            assert_eq!(window.focus, focus);
                                        }
                                        assert!(window.provisional_focus_claim.is_none());
                                    }
                                    Some(Mutation::AppQuiesceBeforeBind) => {}
                                    Some(Mutation::QuiesceUnwind) => {
                                        assert!(window.quiesce_interaction(app));
                                    }
                                    Some(_) => {
                                        window.focus(&target, app);
                                        assert_eq!(window.focus, Some(target.id));
                                        assert!(window.provisional_focus_claim.is_some());
                                        if mutation == Some(Mutation::ProvisionalQuiesce) {
                                            assert!(window.quiesce_interaction(app));
                                        } else if mutation == Some(Mutation::ProvisionalAppQuiesce)
                                        {
                                            assert!(app.quiesce_window_interaction_authority(
                                                window.window_handle().window_id()
                                            ));
                                        }
                                    }
                                    None => {}
                                }
                                if !matches!(
                                    mutation,
                                    Some(Mutation::QuiesceRollback | Mutation::QuiesceUnwind)
                                ) {
                                    assert_eq!(window.focus_generation, generation);
                                }
                                if matches!(
                                    mutation,
                                    Some(Mutation::UnwindPrepaint | Mutation::QuiesceUnwind)
                                ) {
                                    panic!("injected focus candidate failure");
                                }
                                if mutation == Some(Mutation::Focus) {
                                    window.record_prepaint_window_transaction(
                                        PrepaintPublicationId::new(),
                                        move |fence, window, _| {
                                            assert!(fence.is_satisfied_by(window));
                                            assert_eq!(window.focus, Some(target.id));
                                            controls
                                                .published_generation
                                                .set(Some(window.focus_generation));
                                        },
                                        |_, _, _| {},
                                    );
                                }
                            },
                            move |_, (), window, _| {
                                if mutation == Some(Mutation::UnwindPaint) {
                                    panic!("injected focus candidate failure");
                                }
                                if mutation == Some(Mutation::LateGeometry) {
                                    let origin = point(px(f32::MAX), px(0.));
                                    let bounds = Bounds::new(origin, size(px(10.), px(10.)));
                                    window.paint_quad(fill(bounds, red()));
                                }
                            },
                        )
                        .size_full(),
                    )
                    .with_subtree_transform(
                        SubtreeTransform::try_new(
                            size(2., 2.),
                            point(px(0.), px(0.)),
                            SubtreeTransformOrigin::TOP_LEFT,
                        )
                        .unwrap(),
                    ),
            )
    }
}

fn draw(window: &mut Window, app: &mut App) {
    window.refresh();
    window.draw(app).clear(app);
}

fn open(cx: &mut TestAppContext) -> (WindowHandle<FocusRoot>, [FocusHandle; 2], Rc<Controls>) {
    let handles = cx.read(|app| [app.focus_handle(), app.focus_handle()]);
    let controls = Rc::new(Controls::default());
    let handle = cx.add_window({
        let handles = handles.clone();
        let controls = controls.clone();
        move |_, _| FocusRoot { handles, controls }
    });
    cx.test_window(handle.into())
        .defer_frame_requests_for_test();
    cx.update(|app| {
        app.update_window(handle.into(), |_, window, app| {
            draw(window, app);
            window.focus(&handles[0], app);
            draw(window, app);
            assert_eq!(window.committed_focus(app).as_ref(), Some(&handles[0]));
        })
        .unwrap();
    });
    (handle, handles, controls)
}

#[crate::test]
fn input_focus_aba_advances_immediately_but_same_focus_and_empty_blur_do_not(
    cx: &mut TestAppContext,
) {
    let (handle, [a, b], _) = open(cx);
    cx.update(|app| {
        app.update_window(handle.into(), |_, window, app| {
            let generation = window.focus_generation;
            window.focus(&b, app);
            assert_eq!(window.focus_generation, generation + 1);
            window.focus(&a, app);
            assert_eq!(window.focus_generation, generation + 2);
            assert_eq!(window.committed_focus(app).as_ref(), Some(&a));
            draw(window, app);
            assert_eq!(window.focus_generation, generation + 2);
            let revision = window.focus_claim_revision();
            window.focus(&a, app);
            window.focus(&a, app);
            assert_ne!(window.focus_claim_revision(), revision);
            assert_eq!(window.focus_generation, generation + 2);
            window.blur(app);
            assert_eq!(window.focus_generation, generation + 3);
            window.blur(app);
            window.blur(app);
            draw(window, app);
            assert_eq!(window.focus_generation, generation + 3);
            assert!(window.committed_focus(app).is_none());
        })
        .unwrap();
    });
}

#[crate::test]
fn accepted_candidate_advances_focus_generation_before_publication(cx: &mut TestAppContext) {
    let (handle, [_, b], controls) = open(cx);
    cx.update(|app| {
        app.update_window(handle.into(), |_, window, app| {
            let generation = window.focus_generation;
            controls.mutation.set(Some(Mutation::Focus));
            draw(window, app);
            assert_eq!(window.focus_generation, generation + 1);
            assert_eq!(controls.published_generation.get(), Some(generation + 1));
            assert_eq!(window.committed_focus(app).as_ref(), Some(&b));
            draw(window, app);
            assert_eq!(window.focus_generation, generation + 1);
        })
        .unwrap();
    });
}

#[crate::test]
fn rejected_or_rolled_back_candidate_focus_does_not_advance_generation(cx: &mut TestAppContext) {
    let (handle, [a, b], controls) = open(cx);
    cx.update(|app| {
        app.update_window(handle.into(), |_, window, app| {
            let generation = window.focus_generation;
            for mutation in [
                Mutation::UnwindPrepaint,
                Mutation::UnwindPaint,
                Mutation::Rollback,
                Mutation::LateGeometry,
            ] {
                let revision = window.rendered_frame_revision();
                controls.mutation.set(Some(mutation));
                let result = catch_unwind(AssertUnwindSafe(|| draw(window, app)));
                if matches!(mutation, Mutation::UnwindPrepaint | Mutation::UnwindPaint) {
                    let panic = result.expect_err("the real candidate must unwind");
                    let message = panic.downcast_ref::<&str>();
                    assert_eq!(message, Some(&"injected focus candidate failure"));
                    assert_eq!(window.rendered_frame_revision(), revision);
                } else {
                    assert!(result.is_ok());
                    assert_eq!(window.rendered_frame_revision(), revision + 1);
                }
                assert_eq!(window.focus_generation, generation);
                assert_eq!(window.focused(app).as_ref(), Some(&a));
                assert_eq!(window.committed_focus(app).as_ref(), Some(&a));
                assert!(window.provisional_focus_claim.is_none());
                if mutation == Mutation::LateGeometry {
                    assert!(!window.is_focus_handle_rendered(&b));
                    assert_eq!(window.rendered_frame.subtree_transform_diagnostics.len(), 1);
                }
                draw(window, app);
                assert_eq!(window.focus_generation, generation);
            }
        })
        .unwrap();
    });
}

#[crate::test]
fn failed_prepaint_transaction_cannot_resurrect_quiesced_focus_or_claims(cx: &mut TestAppContext) {
    let (handle, [a, _], controls) = open(cx);
    cx.update(|app| {
        app.update_window(handle.into(), |_, window, app| {
            let generation = window.focus_generation;
            let unbound = app.focus_handle();
            let _completion = window.focus_with_completion(&unbound, app, |_, _, _| {});
            assert_eq!(window.retained_focus_claim_count_for_test(), 1);
            assert!(window.pending_focus_completion.is_some());
            controls.mutation.set(Some(Mutation::QuiesceRollback));
            draw(window, app);
            assert_eq!(window.focus_generation, generation + 1);
            for _ in 0..2 {
                assert!(!window.quiesce_interaction(app));
                window.focus(&a, app);
                window.blur(app);
                draw(window, app);
                assert!(window.focused(app).is_none());
                assert!(window.committed_focus(app).is_none());
                assert_eq!(window.retained_focus_claim_count_for_test(), 0);
                assert!(window.pending_focus_completion.is_none());
                assert_eq!(window.focus_generation, generation + 1);
            }
        })
        .unwrap();
    });
}

#[crate::test]
fn irreversible_quiescence_advances_focus_generation_even_when_the_candidate_unwinds(
    cx: &mut TestAppContext,
) {
    let (handle, [a, _], controls) = open(cx);
    cx.update(|app| {
        app.update_window(handle.into(), |_, window, app| {
            let generation = window.focus_generation;
            let revision = window.rendered_frame_revision();
            controls.mutation.set(Some(Mutation::QuiesceUnwind));
            let failure = catch_unwind(AssertUnwindSafe(|| draw(window, app)))
                .expect_err("the real candidate must unwind");
            assert_eq!(
                failure.downcast_ref::<&str>(),
                Some(&"injected focus candidate failure")
            );
            assert_eq!(window.rendered_frame_revision(), revision);
            assert_eq!(window.focus_generation, generation + 1);
            assert!(window.focused(app).is_none());
            assert!(!window.interaction_authority.is_admitted());
            assert!(!window.quiesce_interaction(app));
            window.focus(&a, app);
            draw(window, app);
            assert_eq!(window.focus_generation, generation + 1);
            assert!(window.committed_focus(app).is_none());
        })
        .unwrap();
    });
}

#[crate::test]
fn quiescing_provisional_only_focus_does_not_invent_a_committed_focus_transition(
    cx: &mut TestAppContext,
) {
    let (handle, _, controls) = open(cx);
    cx.update(|app| {
        app.update_window(handle.into(), |_, window, app| {
            window.blur(app);
            draw(window, app);
            let generation = window.focus_generation;
            assert!(window.committed_focus(app).is_none());
            controls.mutation.set(Some(Mutation::ProvisionalQuiesce));
            draw(window, app);
            assert_eq!(window.focus_generation, generation);
            assert!(!window.interaction_authority.is_admitted());
            assert!(window.focused(app).is_none());
            assert!(window.committed_focus(app).is_none());
        })
        .unwrap();
    });
}

#[crate::test]
fn app_quiescence_counts_the_actual_leaf_after_candidate_commit_and_deferred_cleanup(
    cx: &mut TestAppContext,
) {
    for initially_focused in [false, true] {
        let (handle, _, controls) = open(cx);
        let generation = cx.update(|app| {
            app.update_window(handle.into(), |_, window, app| {
                if !initially_focused {
                    window.blur(app);
                    draw(window, app);
                }
                let generation = window.focus_generation;
                controls.mutation.set(Some(if initially_focused {
                    Mutation::AppQuiesceBeforeBind
                } else {
                    Mutation::ProvisionalAppQuiesce
                }));
                draw(window, app);
                assert!(!window.interaction_authority.is_admitted());
                generation
            })
            .unwrap()
        });
        cx.run_until_parked();
        cx.update(|app| {
            app.update_window(handle.into(), |_, window, app| {
                assert!(window.focused(app).is_none());
                assert_eq!(
                    window.focus_generation,
                    generation + u64::from(initially_focused)
                );
                assert!(!window.quiesce_interaction(app));
                draw(window, app);
                assert_eq!(
                    window.focus_generation,
                    generation + u64::from(initially_focused)
                );
                assert!(window.committed_focus(app).is_none());
            })
            .unwrap();
        });
    }
}

#[crate::test]
fn other_window_focus_and_activation_preserve_local_focus_generation(cx: &mut TestAppContext) {
    let (first, [a, _], _) = open(cx);
    let (second, [_, b], _) = open(cx);
    let generation = cx.update(|app| {
        let generation = app
            .update_window(first.into(), |_, window, _| window.focus_generation)
            .unwrap();
        app.update_window(second.into(), |_, window, app| {
            window.focus(&b, app);
            window.blur(app);
            draw(window, app);
        })
        .unwrap();
        generation
    });
    let platform = cx.test_window(first.into());
    for active in [true, false, true] {
        platform.simulate_active_status_change(active);
        cx.run_until_parked();
        cx.update(|app| {
            app.update_window(first.into(), |_, window, app| {
                assert_eq!(window.is_window_active(), active);
                draw(window, app);
                assert_eq!(window.focus_generation, generation);
                assert_eq!(window.committed_focus(app).as_ref(), Some(&a));
            })
            .unwrap();
        });
    }
}
