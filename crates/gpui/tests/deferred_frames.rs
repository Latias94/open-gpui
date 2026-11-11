#![cfg(all(feature = "bench-support", feature = "test-support"))]

//! Public CPU consumers of the real Window pending-candidate lifecycle and native ingress.
//! An ordinary Application uses bench_platform with no renderer; TestWindow retains the original
//! submission token and completion explicitly simulates a platform outcome. Submitted assertions
//! qualify framework publication and initial-presentation ordering, not GPU execution, pixel
//! correctness, an OS compositor, or a native event loop. test-support supplies the existing native
//! input-handler slot accessor; these tests never use TestAppContext or private Window hooks.

use open_gpui::{
    AcceptedFrameFence, App, AppContext as _, Application, ApplicationHandle, Bounds, Context,
    ElementInputHandler, Entity, EntityInputHandler, FocusHandle, InteractiveElement as _,
    IntoElement, MouseButton, MouseDownEvent, NoopTextSystem, ParentElement as _, Pixels, Platform,
    PlatformFrameDeliveryMode, PlatformFrameSubmission, PlatformFrameSubmissionStatus,
    PlatformInput, PlatformInputHandlerSlot, PlatformWindow, PlatformWindowPresentOutcome, Point,
    PrepaintPublicationId, QuitMode, Render, RequestFrameOptions, Role, ScaledPixels,
    StatefulInteractiveElement as _, StyleRefinement, Styled as _, UTF16Selection, Window,
    WindowBounds, WindowHandle, WindowInitialPresentationStatus, WindowOptions,
    WindowPresentationFacts, bench_platform, canvas, div, point, px, rgb, size,
};
use std::{
    cell::{Cell, RefCell},
    ops::Range,
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
    sync::Arc,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Revision {
    S0,
    A,
    B,
}

impl Revision {
    fn label(self) -> &'static str {
        match self {
            Self::S0 => "submitted-s0",
            Self::A => "candidate-a",
            Self::B => "candidate-b",
        }
    }

    fn left(self) -> Pixels {
        px(match self {
            Self::S0 => 0.0,
            Self::A => 72.0,
            Self::B => 144.0,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CommitPanicStage {
    Normal,
    Publication,
}

#[derive(Default)]
struct Observations {
    prepared: RefCell<Vec<Revision>>,
    published: RefCell<Vec<(AcceptedFrameFence, Revision)>>,
    discarded: Cell<usize>,
    clicks: RefCell<Vec<Revision>>,
    field_renders: Cell<usize>,
    watched_submission: RefCell<Option<PlatformFrameSubmission>>,
    mutation_statuses: RefCell<Vec<PlatformFrameSubmissionStatus>>,
    panic_commit_once: Cell<Option<CommitPanicStage>>,
    following_normal_outputs: Cell<usize>,
}

impl Observations {
    fn before_mutation(&self) {
        let watched = self.watched_submission.borrow();
        let submission = watched.as_ref().expect("watch the actual parked candidate");
        self.mutation_statuses
            .borrow_mut()
            .push(submission.status());
        assert_eq!(
            submission.status(),
            PlatformFrameSubmissionStatus::Cancelled
        );
    }

    fn latest_publication(&self) -> (AcceptedFrameFence, Revision) {
        *self
            .published
            .borrow()
            .last()
            .expect("accepted publication")
    }
}

struct Field {
    focus: FocusHandle,
    text: String,
    observed: Rc<Observations>,
}

impl EntityInputHandler for Field {
    fn text_for_range(
        &mut self,
        range: Range<usize>,
        adjusted: &mut Option<Range<usize>>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<String> {
        *adjusted = Some(range.clone());
        String::from_utf16(self.text.encode_utf16().collect::<Vec<_>>().get(range)?).ok()
    }

    fn selected_text_range(
        &mut self,
        _: bool,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        let end = self.text.encode_utf16().count();
        Some(UTF16Selection {
            range: end..end,
            reversed: false,
        })
    }

    fn marked_text_range(&self, _: &mut Window, _: &mut Context<Self>) -> Option<Range<usize>> {
        None
    }

    fn unmark_text(&mut self, _: &mut Window, _: &mut Context<Self>) {}

    fn replace_text_in_range(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.observed.before_mutation();
        self.text
            .replace_range(range.unwrap_or(0..self.text.len()), text);
        cx.notify();
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        _: Option<Range<usize>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.replace_text_in_range(range, text, window, cx);
    }

    fn bounds_for_range(
        &mut self,
        _: Range<usize>,
        bounds: Bounds<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        Some(bounds)
    }

    fn character_index_for_point(
        &mut self,
        _: Point<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<usize> {
        Some(0)
    }
}

impl Render for Field {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.observed
            .field_renders
            .set(self.observed.field_renders.get() + 1);
        let focus = self.focus.clone();
        let field = cx.entity();
        div()
            .id("cached-field")
            .size_full()
            .track_focus(&self.focus)
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| {
                    this.observed.before_mutation();
                    this.observed.clicks.borrow_mut().push(Revision::S0);
                    cx.notify();
                }),
            )
            .child(
                canvas(
                    |_, _, _| {},
                    move |bounds, (), window, app| {
                        window.handle_input(&focus, ElementInputHandler::new(bounds, field), app);
                    },
                )
                .size_full(),
            )
    }
}

struct Controls {
    revision: Revision,
    candidate_only: Entity<Revision>,
    publication: PrepaintPublicationId,
    observed: Rc<Observations>,
}

impl Render for Controls {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let revision = if self.revision == Revision::S0 {
            Revision::S0
        } else {
            *self.candidate_only.read(cx)
        };
        self.observed.prepared.borrow_mut().push(revision);
        let publication = self.publication;
        let observed = self.observed.clone();
        div()
            .size_full()
            .bg(rgb(0x203040))
            .child(
                div()
                    .id("pending-frame-button")
                    .role(Role::Button)
                    .aria_label(revision.label())
                    .absolute()
                    .left(revision.left())
                    .top(px(0.0))
                    .w(px(48.0))
                    .h(px(24.0))
                    .bg(rgb(0x507090))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, _, _, cx| {
                            this.observed.before_mutation();
                            this.observed.clicks.borrow_mut().push(revision);
                            cx.notify();
                        }),
                    ),
            )
            .child(
                canvas(
                    move |_, window, _| {
                        let normal = observed.clone();
                        window.record_prepaint_window_commit(move |_, _, _| {
                            if normal.panic_commit_once.get() == Some(CommitPanicStage::Normal) {
                                normal.panic_commit_once.set(None);
                                panic!("injected deferred normal commit failure");
                            }
                        });
                        let following = observed.clone();
                        window.record_prepaint_window_commit(move |_, _, _| {
                            following
                                .following_normal_outputs
                                .set(following.following_normal_outputs.get() + 1);
                        });
                        let committed = observed.clone();
                        window.record_prepaint_window_transaction(
                            publication,
                            move |fence, window, _| {
                                assert!(fence.is_satisfied_by(window));
                                committed.published.borrow_mut().push((fence, revision));
                                if committed.panic_commit_once.get()
                                    == Some(CommitPanicStage::Publication)
                                {
                                    committed.panic_commit_once.set(None);
                                    panic!("injected deferred publication failure");
                                }
                            },
                            move |_, _, _| observed.discarded.set(observed.discarded.get() + 1),
                        );
                    },
                    |_, (), _, _| {},
                )
                .size_full(),
            )
    }
}

struct Root {
    controls: Entity<Controls>,
    field: Entity<Field>,
}

impl Render for Root {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .child(
                self.controls
                    .clone()
                    .cached(StyleRefinement::default().size_full()),
            )
            .child(
                div()
                    .absolute()
                    .left(px(0.0))
                    .top(px(40.0))
                    .w(px(48.0))
                    .h(px(24.0))
                    .child(
                        self.field
                            .clone()
                            .cached(StyleRefinement::default().size_full()),
                    ),
            )
    }
}

fn options() -> WindowOptions {
    WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(Bounds::new(
            point(px(0.0), px(0.0)),
            size(px(240.0), px(120.0)),
        ))),
        focus_on_appearing: false,
        inactive_frame_interval: None,
        low_power_frame_interval: None,
        thermal_frame_interval: None,
        ..Default::default()
    }
}

struct Consumer {
    platform: Rc<dyn Platform>,
    window: WindowHandle<Root>,
    native: Box<dyn PlatformWindow>,
    controls: Entity<Controls>,
    field: Entity<Field>,
    candidate_only: Entity<Revision>,
    observed: Rc<Observations>,
    app: ApplicationHandle,
}

impl Consumer {
    fn new(mode: PlatformFrameDeliveryMode) -> Self {
        let platform = bench_platform(None, Arc::new(NoopTextSystem::new()));
        let app = Application::with_platform(platform.clone())
            .with_quit_mode(QuitMode::Explicit)
            .run_embedded(|app| {
                assert!(!app.is_test());
                assert!(!app.background_executor().is_test());
            });
        let observed = Rc::new(Observations::default());
        let mut native = None;
        let mut entities = None;
        let window = app
            .update(|app| {
                app.open_window_with_platform_window(
                    options(),
                    |handle, params, displays| {
                        let mut window =
                            platform.open_window_with_display_snapshot(handle, params, displays)?;
                        let test = window.as_test().expect("the original TestPlatform window");
                        assert_eq!(
                            test.frame_delivery_mode(),
                            PlatformFrameDeliveryMode::Synchronous
                        );
                        assert!(test.pending_frame_submission().is_none());
                        test.set_frame_delivery_mode(mode);
                        native = Some(Box::new(test.clone()) as Box<dyn PlatformWindow>);
                        Ok(window)
                    },
                    |window, app| {
                        let candidate_only = app.new(|_| Revision::A);
                        let field = app.new(|cx| Field {
                            focus: cx.focus_handle(),
                            text: "old".into(),
                            observed: observed.clone(),
                        });
                        window.focus(&field.read(app).focus.clone(), app);
                        let controls = app.new(|_| Controls {
                            revision: Revision::S0,
                            candidate_only: candidate_only.clone(),
                            publication: PrepaintPublicationId::new(),
                            observed: observed.clone(),
                        });
                        entities = Some((controls.clone(), field.clone(), candidate_only));
                        app.new(|_| Root { controls, field })
                    },
                )
            })
            .expect("open through the public factory and initial-presentation transaction");
        let (controls, field, candidate_only) = entities.unwrap();
        let mut consumer = Self {
            app,
            platform,
            window,
            native: native.unwrap(),
            controls,
            field,
            candidate_only,
            observed,
        };
        if mode == PlatformFrameDeliveryMode::Synchronous {
            consumer.frame(false);
        }
        consumer
    }

    fn facts(&self) -> WindowPresentationFacts {
        self.app.update(|app| {
            app.update_window(self.window.into(), |_, window, _| {
                window.presentation_facts()
            })
            .unwrap()
        })
    }

    fn pending(&mut self) -> PlatformFrameSubmission {
        self.native
            .as_test()
            .unwrap()
            .pending_frame_submission()
            .expect("actual pending token")
    }

    fn native_input(&self) -> PlatformInputHandlerSlot {
        self.native
            .input_handler_slot_for_test()
            .expect("original native input-handler slot")
    }

    fn scene_bounds(&self) -> Vec<Bounds<ScaledPixels>> {
        self.app.update(|app| {
            app.update_window(self.window.into(), |_, window, _| {
                window
                    .rendered_scene()
                    .quads
                    .iter()
                    .map(|quad| quad.bounds)
                    .collect()
            })
            .unwrap()
        })
    }

    fn frame(&mut self, force_render: bool) {
        assert!(
            self.native
                .as_test()
                .unwrap()
                .simulate_frame(RequestFrameOptions {
                    force_render,
                    require_presentation: true,
                    ..Default::default()
                })
        );
        self.app.update(|_| ());
    }

    fn prepare_a(&mut self) -> PlatformFrameSubmission {
        self.native
            .as_test()
            .unwrap()
            .set_frame_delivery_mode(PlatformFrameDeliveryMode::Deferred);
        self.app.update(|app| {
            self.controls.update(app, |controls, cx| {
                controls.revision = Revision::A;
                cx.notify();
            });
        });
        self.frame(false);
        self.pending()
    }

    fn complete(
        &self,
        submission: &PlatformFrameSubmission,
        outcome: PlatformWindowPresentOutcome,
    ) {
        self.app.update(|app| {
            let before = app
                .update_window(self.window.into(), |_, window, _| {
                    window.presentation_facts()
                })
                .unwrap();
            let publications = self.observed.published.borrow().len();
            assert!(submission.complete(outcome));
            assert_eq!(self.observed.published.borrow().len(), publications);
            app.update_window(self.window.into(), |_, window, _| {
                assert_eq!(window.presentation_facts(), before);
            })
            .unwrap();
        });
        self.app.update(|_| ());
    }

    fn activate_accessibility(&mut self) {
        assert!(self.native.as_test().unwrap().activate_accessibility());
        self.app.update(|_| ());
        self.frame(true);
    }

    fn a11y(&self) -> String {
        self.app.update(|app| {
            app.update_window(self.window.into(), |_, window, _| {
                window
                    .debug_a11y_tree_json()
                    .expect("published accessibility tree")
            })
            .unwrap()
        })
    }
}

#[test]
fn hidden_first_frame_waits_for_its_original_completion_and_app_ingress() {
    let mut consumer = Consumer::new(PlatformFrameDeliveryMode::Deferred);
    let first = consumer.pending();
    let same = consumer.pending();
    assert_eq!(first.window_id(), consumer.window.window_id());
    assert_eq!(first.frame_generation(), same.frame_generation());
    assert_eq!(first.attempt_id(), same.attempt_id());
    assert!(first.is_pending());
    let before = consumer.facts();
    assert!(before.native_created);
    assert!(!before.native_visible);
    assert_eq!(before.frame_accepted_generation, None);
    assert_eq!(before.present_submitted_generation, None);
    assert_eq!(before.non_empty_presented_generation, None);
    assert_eq!(
        before.initial_presentation,
        WindowInitialPresentationStatus::Pending
    );
    assert!(consumer.observed.published.borrow().is_empty());
    assert!(consumer.native_input().with_handler(|_| ()).is_none());
    assert!(
        consumer
            .native
            .as_test()
            .unwrap()
            .text_input_configurations()
            .is_empty()
    );
    consumer.app.update(|app| {
        app.update_window(consumer.window.into(), |_, window, _| {
            assert!(window.rendered_scene().quads.is_empty());
            assert!(window.debug_a11y_tree_json().is_none());
        })
        .unwrap();
    });

    consumer.frame(true);
    assert_eq!(consumer.pending().attempt_id(), first.attempt_id());
    assert_eq!(consumer.facts(), before);
    assert!(!same.complete(PlatformWindowPresentOutcome::Deferred));
    consumer.complete(&same, PlatformWindowPresentOutcome::Submitted);

    let submitted = consumer.facts();
    assert_eq!(first.status(), PlatformFrameSubmissionStatus::Submitted);
    assert_eq!(
        submitted.frame_accepted_generation,
        Some(first.frame_generation())
    );
    assert_eq!(
        submitted.present_submitted_generation,
        Some(first.frame_generation())
    );
    assert_eq!(
        submitted.non_empty_presented_generation,
        Some(first.frame_generation())
    );
    assert_eq!(
        submitted.initial_presentation,
        WindowInitialPresentationStatus::Completed
    );
    assert!(submitted.native_visible);
    let (fence, revision) = consumer.observed.latest_publication();
    assert_eq!(fence.window_id(), first.window_id());
    assert_eq!(fence.generation(), first.frame_generation());
    assert_eq!(revision, Revision::S0);
    assert!(consumer.native_input().with_handler(|_| ()).is_some());
    assert!(!first.complete(PlatformWindowPresentOutcome::Submitted));
    consumer.app.update(App::shutdown);
}

#[test]
fn parked_candidate_keeps_submitted_cached_controls_text_focus_and_semantics() {
    let mut consumer = Consumer::new(PlatformFrameDeliveryMode::Synchronous);
    consumer.activate_accessibility();
    let submitted = consumer.facts();
    let scene = consumer.scene_bounds();
    let tree = consumer.a11y();
    assert!(tree.contains(Revision::S0.label()));
    let publications = consumer.observed.published.borrow().len();
    let configurations = consumer
        .native
        .as_test()
        .unwrap()
        .text_input_configurations();
    let notifications = consumer
        .native
        .as_test()
        .unwrap()
        .text_input_state_changes();
    let old_bounds = consumer
        .native_input()
        .with_handler(|handler| handler.bounds_for_range(0..0))
        .unwrap()
        .unwrap();
    let focus = consumer
        .app
        .update(|app| consumer.field.read(app).focus.clone());
    let candidate = consumer.prepare_a();

    assert_eq!(consumer.facts(), submitted);
    assert_eq!(consumer.scene_bounds(), scene);
    assert_eq!(consumer.a11y(), tree);
    assert_eq!(consumer.observed.published.borrow().len(), publications);
    assert_eq!(consumer.observed.latest_publication().1, Revision::S0);
    assert_eq!(
        consumer
            .native
            .as_test()
            .unwrap()
            .text_input_configurations(),
        configurations
    );
    assert_eq!(
        consumer
            .native
            .as_test()
            .unwrap()
            .text_input_state_changes(),
        notifications
    );
    consumer.app.update(|app| {
        app.update_window(consumer.window.into(), |_, window, app| {
            assert_eq!(window.committed_focus(app).as_ref(), Some(&focus));
        })
        .unwrap();
    });
    consumer
        .native_input()
        .with_handler(|handler| {
            assert_eq!(
                handler.text_for_range(0..3, &mut None).as_deref(),
                Some("old")
            );
            assert_eq!(handler.bounds_for_range(0..0), Some(old_bounds));
        })
        .unwrap();
    assert!(
        candidate.is_pending(),
        "read-only native queries preserve the candidate"
    );
    assert_eq!(consumer.pending().attempt_id(), candidate.attempt_id());

    consumer.complete(&candidate, PlatformWindowPresentOutcome::Submitted);
    assert_ne!(consumer.scene_bounds(), scene);
    assert_eq!(consumer.observed.latest_publication().1, Revision::A);
    assert_eq!(
        consumer.facts().present_submitted_generation,
        Some(candidate.frame_generation())
    );
    let tree = consumer.a11y();
    assert!(tree.contains(Revision::A.label()));
    assert!(!tree.contains(Revision::S0.label()));
    let new_bounds = consumer
        .native_input()
        .with_handler(|handler| handler.bounds_for_range(0..0))
        .unwrap()
        .unwrap();
    assert_eq!(
        new_bounds, old_bounds,
        "the stable field keeps its submitted text geometry"
    );
    consumer.app.update(App::shutdown);
}

#[test]
fn old_button_remains_live_and_cancels_pending_before_its_mutation() {
    for y in [8.0, 48.0] {
        let mut consumer = Consumer::new(PlatformFrameDeliveryMode::Synchronous);
        let submitted = consumer.facts();
        let field_renders = consumer.observed.field_renders.get();
        let candidate = consumer.prepare_a();
        assert_eq!(consumer.observed.field_renders.get(), field_renders);
        consumer
            .observed
            .watched_submission
            .replace(Some(candidate.clone()));
        consumer
            .native
            .as_test()
            .unwrap()
            .simulate_input(PlatformInput::MouseDown(MouseDownEvent {
                button: MouseButton::Left,
                position: point(px(8.0), px(y)),
                ..Default::default()
            }));
        consumer.app.update(|_| ());

        assert_eq!(
            consumer.observed.clicks.borrow().as_slice(),
            &[Revision::S0]
        );
        assert_eq!(
            consumer.observed.mutation_statuses.borrow().as_slice(),
            &[PlatformFrameSubmissionStatus::Cancelled]
        );
        assert_eq!(
            consumer.facts().present_submitted_generation,
            submitted.present_submitted_generation
        );
        assert_eq!(consumer.observed.latest_publication().1, Revision::S0);
        consumer.frame(false);
        let replacement = consumer.pending();
        assert_eq!(replacement.frame_generation(), candidate.frame_generation());
        assert_ne!(replacement.attempt_id(), candidate.attempt_id());
        assert!(!candidate.complete(PlatformWindowPresentOutcome::Submitted));
        assert!(replacement.is_pending());
        assert_eq!(consumer.observed.latest_publication().1, Revision::S0);
        consumer.complete(&replacement, PlatformWindowPresentOutcome::Submitted);
        assert_eq!(consumer.observed.latest_publication().1, Revision::A);
        consumer.app.update(App::shutdown);
    }
}

#[test]
fn old_native_text_handler_cancels_pending_before_editing_the_document() {
    let mut consumer = Consumer::new(PlatformFrameDeliveryMode::Synchronous);
    let submitted = consumer.facts();
    let candidate = consumer.prepare_a();
    consumer
        .observed
        .watched_submission
        .replace(Some(candidate.clone()));
    consumer
        .native_input()
        .with_handler(|handler| {
            assert_eq!(
                handler.text_for_range(0..3, &mut None).as_deref(),
                Some("old")
            );
            assert!(candidate.is_pending());
            handler.replace_text_in_range(Some(0..3), "edit");
        })
        .expect("the submitted field must retain its original native handler");

    assert_eq!(
        consumer
            .app
            .update(|app| consumer.field.read(app).text.clone()),
        "edit"
    );
    assert_eq!(
        consumer.observed.mutation_statuses.borrow().as_slice(),
        &[PlatformFrameSubmissionStatus::Cancelled]
    );
    assert_eq!(
        consumer.facts().present_submitted_generation,
        submitted.present_submitted_generation
    );
    assert_eq!(consumer.observed.latest_publication().1, Revision::S0);
    assert!(!candidate.complete(PlatformWindowPresentOutcome::Submitted));
    consumer.app.update(App::shutdown);
}

#[test]
fn candidate_only_dependency_keeps_b_dirty_without_substituting_it_for_a() {
    let mut consumer = Consumer::new(PlatformFrameDeliveryMode::Synchronous);
    let candidate = consumer.prepare_a();
    let prepared = consumer.observed.prepared.borrow().len();
    consumer.app.update(|app| {
        consumer.candidate_only.update(app, |revision, cx| {
            *revision = Revision::B;
            cx.notify();
        });
    });
    consumer.frame(true);
    assert_eq!(consumer.observed.prepared.borrow().len(), prepared);
    assert_eq!(consumer.pending().attempt_id(), candidate.attempt_id());
    assert_eq!(consumer.observed.latest_publication().1, Revision::S0);

    consumer.complete(&candidate, PlatformWindowPresentOutcome::Submitted);
    let (fence, revision) = consumer.observed.latest_publication();
    assert_eq!(
        revision,
        Revision::A,
        "completion publishes the frozen original candidate"
    );
    assert_eq!(fence.generation(), candidate.frame_generation());
    consumer.frame(false);
    let next = consumer.pending();
    assert_eq!(
        consumer.observed.prepared.borrow().last(),
        Some(&Revision::B)
    );
    assert!(next.frame_generation() > candidate.frame_generation());
    assert_ne!(next.attempt_id(), candidate.attempt_id());
    assert_eq!(consumer.observed.latest_publication().1, Revision::A);
    assert!(!candidate.complete(PlatformWindowPresentOutcome::Submitted));
    assert!(next.is_pending());
    consumer.complete(&next, PlatformWindowPresentOutcome::Submitted);
    let (fence, revision) = consumer.observed.latest_publication();
    assert_eq!(revision, Revision::B);
    assert_eq!(fence.generation(), next.frame_generation());
    consumer.app.update(App::shutdown);
}

#[test]
fn refusal_and_repaint_keep_s0_and_retry_with_distinct_same_generation_authority() {
    for (outcome, status) in [
        (
            PlatformWindowPresentOutcome::Rejected,
            PlatformFrameSubmissionStatus::Rejected,
        ),
        (
            PlatformWindowPresentOutcome::RepaintRequired,
            PlatformFrameSubmissionStatus::RepaintRequired,
        ),
    ] {
        let mut consumer = Consumer::new(PlatformFrameDeliveryMode::Synchronous);
        let submitted = consumer.facts();
        let candidate = consumer.prepare_a();
        let prepared = consumer.observed.prepared.borrow().len();
        let publications = consumer.observed.published.borrow().len();
        consumer.complete(&candidate, outcome);
        assert_eq!(candidate.status(), status);
        assert_eq!(
            consumer.facts().present_submitted_generation,
            submitted.present_submitted_generation
        );
        assert_eq!(consumer.observed.published.borrow().len(), publications);
        assert_eq!(consumer.observed.discarded.get(), 0);

        consumer.frame(false);
        let replacement = consumer.pending();
        assert!(
            consumer.observed.prepared.borrow().len() > prepared,
            "build a fresh candidate"
        );
        assert_eq!(replacement.frame_generation(), candidate.frame_generation());
        assert_ne!(replacement.attempt_id(), candidate.attempt_id());
        assert_eq!(replacement.window_id(), candidate.window_id());
        assert!(!candidate.complete(PlatformWindowPresentOutcome::Submitted));
        assert_eq!(candidate.status(), status);
        assert!(replacement.is_pending());
        assert_eq!(consumer.observed.latest_publication().1, Revision::S0);
        consumer.complete(&replacement, PlatformWindowPresentOutcome::Submitted);
        assert_eq!(
            consumer.observed.latest_publication().0.generation(),
            replacement.frame_generation()
        );
        assert_eq!(consumer.observed.latest_publication().1, Revision::A);
        consumer.app.update(App::shutdown);
    }
}

#[test]
fn close_cancels_original_token_before_a_replacement_window_can_submit() {
    let mut consumer = Consumer::new(PlatformFrameDeliveryMode::Synchronous);
    let candidate = consumer.prepare_a();
    consumer.app.update(|app| {
        app.update_window(consumer.window.into(), |_, window, app| {
            window.remove_window(app);
        })
        .unwrap();
    });
    assert_eq!(candidate.status(), PlatformFrameSubmissionStatus::Cancelled);
    assert!(
        consumer
            .native
            .as_test()
            .unwrap()
            .pending_frame_submission()
            .is_none()
    );
    assert!(
        !consumer
            .native
            .as_test()
            .unwrap()
            .simulate_frame(RequestFrameOptions::default())
    );
    consumer.app.update(|app| assert!(app.windows().is_empty()));

    let platform = consumer.platform.clone();
    let mut native = None;
    let replacement = consumer
        .app
        .update(|app| {
            app.open_window_with_platform_window(
                options(),
                |handle, params, displays| {
                    let mut window =
                        platform.open_window_with_display_snapshot(handle, params, displays)?;
                    let test = window.as_test().unwrap();
                    test.set_frame_delivery_mode(PlatformFrameDeliveryMode::Deferred);
                    native = Some(Box::new(test.clone()) as Box<dyn PlatformWindow>);
                    Ok(window)
                },
                |_, app| {
                    app.new(|_| Root {
                        controls: consumer.controls.clone(),
                        field: consumer.field.clone(),
                    })
                },
            )
        })
        .unwrap();
    assert_ne!(replacement.window_id(), candidate.window_id());
    let next = native
        .as_mut()
        .unwrap()
        .as_test()
        .unwrap()
        .pending_frame_submission()
        .unwrap();
    assert_eq!(next.window_id(), replacement.window_id());
    assert!(!candidate.complete(PlatformWindowPresentOutcome::Submitted));
    assert!(!candidate.complete(PlatformWindowPresentOutcome::Rejected));
    assert!(next.is_pending());
    consumer.app.update(|app| {
        app.update_window(replacement.into(), |_, window, _| {
            assert_eq!(window.presentation_facts().frame_accepted_generation, None);
        })
        .unwrap();
        assert!(next.complete(PlatformWindowPresentOutcome::Submitted));
    });
    consumer.app.update(|app| {
        app.update_window(replacement.into(), |_, window, _| {
            assert_eq!(
                window.presentation_facts().present_submitted_generation,
                Some(next.frame_generation())
            );
            assert_eq!(
                window.presentation_facts().initial_presentation,
                WindowInitialPresentationStatus::Completed
            );
        })
        .unwrap();
    });
    consumer.app.update(App::shutdown);
}

#[test]
fn deferred_publication_unwind_records_submission_and_recovers_hidden_initial_presentation() {
    for stage in [CommitPanicStage::Normal, CommitPanicStage::Publication] {
        let mut consumer = Consumer::new(PlatformFrameDeliveryMode::Deferred);
        let first = consumer.pending();
        let before = consumer.facts();
        assert!(!before.native_visible);
        assert_eq!(before.frame_accepted_generation, None);
        assert_eq!(before.present_submitted_generation, None);
        assert_eq!(
            before.initial_presentation,
            WindowInitialPresentationStatus::Pending
        );
        consumer.observed.panic_commit_once.set(Some(stage));
        let following_before = consumer.observed.following_normal_outputs.get();

        let failure = catch_unwind(AssertUnwindSafe(|| {
            consumer.complete(&first, PlatformWindowPresentOutcome::Submitted);
        }))
        .expect_err("the deferred commit must propagate its original panic");
        let expected_message = match stage {
            CommitPanicStage::Normal => "injected deferred normal commit failure",
            CommitPanicStage::Publication => "injected deferred publication failure",
        };
        assert_eq!(failure.downcast_ref::<&str>(), Some(&expected_message));
        assert_eq!(first.status(), PlatformFrameSubmissionStatus::Submitted);
        assert!(!first.complete(PlatformWindowPresentOutcome::Submitted));
        assert_eq!(
            consumer.observed.following_normal_outputs.get(),
            following_before + 1,
            "a later Normal output must execute even when an earlier output panics"
        );

        let submitted = consumer.facts();
        assert_eq!(
            submitted.frame_accepted_generation,
            Some(first.frame_generation())
        );
        assert_eq!(
            submitted.present_submitted_generation,
            Some(first.frame_generation())
        );
        assert_eq!(
            submitted.non_empty_presented_generation,
            Some(first.frame_generation())
        );
        let attempt = submitted
            .latest_present_attempt
            .expect("original submission facts survive unwind");
        assert_eq!(attempt.generation, first.frame_generation());
        assert_eq!(attempt.outcome, PlatformWindowPresentOutcome::Submitted);
        let (fence, revision) = consumer.observed.latest_publication();
        assert_eq!(fence.generation(), first.frame_generation());
        assert_eq!(revision, Revision::S0);

        consumer.app.update(|_| ());
        let revealed = consumer.facts();
        assert_eq!(
            revealed.initial_presentation,
            WindowInitialPresentationStatus::Completed
        );
        assert!(revealed.native_visible);
        assert_eq!(
            revealed.present_submitted_generation,
            Some(first.frame_generation())
        );
        assert!(consumer.native_input().with_handler(|_| ()).is_some());

        let next = consumer.prepare_a();
        assert!(next.frame_generation() > first.frame_generation());
        consumer.complete(&next, PlatformWindowPresentOutcome::Submitted);
        assert_eq!(consumer.observed.latest_publication().1, Revision::A);
        assert_eq!(
            consumer.facts().present_submitted_generation,
            Some(next.frame_generation())
        );
        assert_eq!(
            consumer.observed.following_normal_outputs.get(),
            following_before + 2
        );
        consumer.app.update(App::shutdown);
    }
}

#[test]
fn empty_initial_submissions_request_retries_and_terminate_within_a_bound() {
    struct EmptyRoot;

    impl Render for EmptyRoot {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            div().size_full()
        }
    }

    let platform = bench_platform(None, Arc::new(NoopTextSystem::new()));
    let app = Application::with_platform(platform.clone())
        .with_quit_mode(QuitMode::Explicit)
        .run_embedded(|_| ());
    let mut native = None;
    let window = app
        .update(|app| {
            app.open_window_with_platform_window(
                options(),
                |handle, params, displays| {
                    let mut window =
                        platform.open_window_with_display_snapshot(handle, params, displays)?;
                    let test = window.as_test().unwrap();
                    test.set_frame_delivery_mode(PlatformFrameDeliveryMode::Deferred);
                    native = Some(test.clone());
                    Ok(window)
                },
                |_, app| app.new(|_| EmptyRoot),
            )
        })
        .unwrap();
    let native = native.unwrap();
    let mut completions = 0;

    for _ in 0..8 {
        if app.update(|app| app.windows().is_empty()) {
            break;
        }
        let submission = native
            .pending_frame_submission()
            .expect("initial candidate");
        native.simulate_scheduled_frame();
        app.update(|_| ());
        assert_eq!(
            native.pending_frame_submission().unwrap().attempt_id(),
            submission.attempt_id()
        );
        assert!(
            !native.frame_scheduled(),
            "the delivered candidate stays parked"
        );
        let wakes_before = native.frame_wake_count();
        app.update(|app| {
            app.update_window(window.into(), |_, window, _| {
                let facts = window.presentation_facts();
                assert!(!facts.native_visible);
                assert_eq!(facts.non_empty_presented_generation, None);
                assert_eq!(
                    facts.initial_presentation,
                    WindowInitialPresentationStatus::Pending
                );
                assert!(window.rendered_scene().batches().next().is_none());
            })
            .unwrap();
            assert!(submission.complete(PlatformWindowPresentOutcome::Submitted));
        });
        app.update(|_| ());
        completions += 1;
        assert_eq!(
            submission.status(),
            PlatformFrameSubmissionStatus::Submitted
        );

        if app.update(|app| app.windows().is_empty()) {
            break;
        }
        assert!(
            native.frame_wake_count() > wakes_before,
            "a still-hidden empty submission must request another initial frame"
        );
        if native.pending_frame_submission().is_none() {
            assert!(
                native.simulate_scheduled_frame(),
                "the retry must have a real wakeup"
            );
            app.update(|_| ());
        }
        let next = native
            .pending_frame_submission()
            .expect("fresh initial retry");
        assert!(next.frame_generation() > submission.frame_generation());
        assert_ne!(next.attempt_id(), submission.attempt_id());
    }

    assert!(completions > 1, "initial presentation must actually retry");
    app.update(|app| {
        assert!(
            app.windows().is_empty(),
            "empty initial frames must terminate within the bound"
        );
    });
    assert!(!native.is_visible());
    assert!(native.pending_frame_submission().is_none());
    assert!(!native.frame_scheduled());
    assert!(!native.simulate_scheduled_frame());
    app.update(App::shutdown);
}
