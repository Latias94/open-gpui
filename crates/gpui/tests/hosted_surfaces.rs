#![cfg(feature = "test-support")]

//! Public CPU lifecycle consumers of the production HostedSurface and ordinary App/Window roots.
//! TestAppContext supplies the existing scheduler and text system. The manual renderer explicitly
//! simulates submission and quiescence; these tests qualify neither GPU output nor native hosting.

use open_gpui::{
    AcceptedFrameFence, AppContext as _, Bounds, Context, ElementInputHandler, Empty, Entity,
    EntityInputHandler, FocusHandle, HeadlessAtlas, HostedSurface, HostedSurfaceGeometry,
    HostedSurfaceOptions, HostedSurfaceRenderer, InteractiveElement as _, IntoElement, MouseButton,
    MouseDownEvent, ParentElement as _, Pixels, PlatformAtlas, PlatformFrameSubmission,
    PlatformFrameSubmissionStatus, PlatformInput, PlatformPresentationShutdownOutcome,
    PlatformWindowActivationSupport, PlatformWindowCreationCapabilities,
    PlatformWindowMutationCapabilities, PlatformWindowPresentOutcome, PrepaintPublicationId,
    PreparedPlatformPresentationShutdown, QuitMode, Render, RequestFrameOptions, Role, Scene,
    StatefulInteractiveElement as _, StyleRefinement, Styled as _, TestAppContext, UTF16Selection,
    Window, WindowActivationTerminal, WindowAppearance, WindowCoordinateSpace,
    WindowCreationSupport, WindowHandle, WindowInitialPresentationStatus, WindowMutationDispatch,
    WindowMutationSupport, WindowOpenFailureStage, WindowOptions, WindowPresentationFacts,
    WindowPresentationShutdownTicket, canvas, div, point, px, rgb, size,
};
use std::{
    cell::{Cell, RefCell},
    ops::Range,
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
    sync::Arc,
};

struct Delivery {
    geometry: HostedSurfaceGeometry,
    submission: PlatformFrameSubmission,
    quads: usize,
}

struct SimulatedRenderer {
    atlas: Arc<dyn PlatformAtlas>,
    outcome: Cell<PlatformWindowPresentOutcome>,
    deliveries: RefCell<Vec<Delivery>>,
    shutdowns: RefCell<Vec<WindowPresentationShutdownTicket>>,
    quiesced: Rc<Cell<usize>>,
}

impl SimulatedRenderer {
    fn new(outcome: PlatformWindowPresentOutcome) -> Rc<Self> {
        Rc::new(Self {
            atlas: Arc::new(HeadlessAtlas::default()),
            outcome: Cell::new(outcome),
            deliveries: RefCell::new(Vec::new()),
            shutdowns: RefCell::new(Vec::new()),
            quiesced: Rc::new(Cell::new(0)),
        })
    }

    fn pending(&self) -> PlatformFrameSubmission {
        let token = self
            .deliveries
            .borrow()
            .last()
            .expect("the real Window must deliver a candidate")
            .submission
            .clone();
        assert!(token.is_pending(), "retain the original pending submission");
        token
    }
}

impl HostedSurfaceRenderer for SimulatedRenderer {
    fn sprite_atlas(&self) -> Arc<dyn PlatformAtlas> {
        self.atlas.clone()
    }

    fn draw(
        &self,
        scene: &Scene,
        geometry: HostedSurfaceGeometry,
        submission: &PlatformFrameSubmission,
    ) -> PlatformWindowPresentOutcome {
        self.deliveries.borrow_mut().push(Delivery {
            geometry,
            submission: submission.clone(),
            quads: scene.quads.len(),
        });
        self.outcome.get()
    }

    fn prepare_shutdown(
        &self,
        shutdown: WindowPresentationShutdownTicket,
    ) -> PreparedPlatformPresentationShutdown {
        self.shutdowns.borrow_mut().push(shutdown.clone());
        let quiesced = self.quiesced.clone();
        PreparedPlatformPresentationShutdown::new(shutdown, move |ticket| {
            assert!(!ticket.snapshot().binding_terminal());
            assert!(ticket.acknowledge_quiesced());
            quiesced.set(quiesced.get() + 1);
            PlatformPresentationShutdownOutcome::Quiesced
        })
    }
}

#[derive(Default)]
struct Observations {
    renders: RefCell<Vec<u8>>,
    published: RefCell<Vec<(AcceptedFrameFence, u8)>>,
    discarded: Cell<usize>,
    clicks: RefCell<Vec<u8>>,
    watched: RefCell<Option<PlatformFrameSubmission>>,
    mutation_statuses: RefCell<Vec<PlatformFrameSubmissionStatus>>,
}

impl Observations {
    fn before_input_mutation(&self) {
        if let Some(token) = self.watched.borrow().as_ref() {
            self.mutation_statuses.borrow_mut().push(token.status());
            assert_eq!(token.status(), PlatformFrameSubmissionStatus::Cancelled);
        }
    }

    fn latest_revision(&self) -> u8 {
        self.published
            .borrow()
            .last()
            .expect("the submitted frame must publish its original transaction")
            .1
    }
}

struct Field {
    focus: FocusHandle,
    text: String,
    marked: Option<Range<usize>>,
    unmarked: usize,
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
        let encoded: Vec<_> = self.text.encode_utf16().collect();
        *adjusted = Some(range.clone());
        String::from_utf16(encoded.get(range)?).ok()
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
        self.marked.clone()
    }

    fn unmark_text(&mut self, _: &mut Window, _: &mut Context<Self>) {
        if self.marked.take().is_some() {
            self.unmarked += 1;
        }
    }

    fn replace_text_in_range(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.observed.before_input_mutation();
        let mut encoded: Vec<_> = self.text.encode_utf16().collect();
        let range = range.unwrap_or(0..encoded.len());
        encoded.splice(range, text.encode_utf16());
        self.text =
            String::from_utf16(&encoded).expect("the test replaces complete UTF-16 scalars");
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
        let start = range.as_ref().map_or(0, |range| range.start);
        self.replace_text_in_range(range, text, window, cx);
        self.marked = Some(start..start + text.encode_utf16().count());
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
        _: open_gpui::Point<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<usize> {
        Some(0)
    }
}

impl Render for Field {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let focus = self.focus.clone();
        let field = cx.entity();
        div()
            .id("hosted-field")
            .size_full()
            .track_focus(&self.focus)
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

struct Root {
    revision: u8,
    field: Entity<Field>,
    publication: PrepaintPublicationId,
    observed: Rc<Observations>,
}

impl Render for Root {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let revision = self.revision;
        self.observed.renders.borrow_mut().push(revision);
        let publication = self.publication;
        let observed = self.observed.clone();
        div()
            .size_full()
            .bg(rgb(0x203040))
            .child(
                div()
                    .id("hosted-button")
                    .role(Role::Button)
                    .aria_label(if revision == 0 {
                        "submitted"
                    } else {
                        "successor"
                    })
                    .absolute()
                    .left(px(f32::from(revision) * 64.0))
                    .top(px(0.0))
                    .w(px(48.0))
                    .h(px(24.0))
                    .bg(rgb(0x507090))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, _, _, cx| {
                            this.observed.before_input_mutation();
                            this.observed.clicks.borrow_mut().push(revision);
                            cx.notify();
                        }),
                    ),
            )
            .child(
                div()
                    .absolute()
                    .left(px(0.0))
                    .top(px(40.0))
                    .w(px(120.0))
                    .h(px(24.0))
                    .child(
                        self.field
                            .clone()
                            .cached(StyleRefinement::default().size_full()),
                    ),
            )
            .child(
                canvas(
                    move |_, window, _| {
                        let capture = Box::new(revision);
                        window.record_prepaint_window_commit(move |_, _, _| {
                            assert_eq!(*capture, revision);
                        });
                        let committed = observed.clone();
                        window.record_prepaint_window_transaction(
                            publication,
                            move |fence, window, _| {
                                assert!(fence.is_satisfied_by(window));
                                committed.published.borrow_mut().push((fence, revision));
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

struct Consumer {
    surface: HostedSurface,
    renderer: Rc<SimulatedRenderer>,
    window: WindowHandle<Root>,
    observed: Rc<Observations>,
    wakes: Rc<Cell<usize>>,
}

impl Consumer {
    fn open(cx: &TestAppContext, outcome: PlatformWindowPresentOutcome, text: &str) -> Self {
        Self::open_with_options(
            cx,
            outcome,
            text,
            HostedSurfaceOptions {
                size: size(px(240.0), px(120.0)),
                scale_factor: 1.0,
                appearance: WindowAppearance::Dark,
                active: true,
                visible: true,
            },
        )
    }

    fn open_with_options(
        cx: &TestAppContext,
        outcome: PlatformWindowPresentOutcome,
        text: &str,
        options: HostedSurfaceOptions,
    ) -> Self {
        cx.update(|app| app.set_quit_mode(QuitMode::Explicit));
        let renderer = SimulatedRenderer::new(outcome);
        let wakes = Rc::new(Cell::new(0));
        let wake_count = wakes.clone();
        let surface = HostedSurface::new(options, renderer.clone(), move || {
            wake_count.set(wake_count.get() + 1)
        })
        .expect("construct the production hosted adapter");
        let observed = Rc::new(Observations::default());
        let text = text.to_owned();
        let capture = String::from("single-use hosted root capture");
        let window = cx
            .update(|app| {
                app.open_surface(&surface, |window, app| {
                    drop(capture);
                    let field = app.new(|cx| Field {
                        focus: cx.focus_handle(),
                        text,
                        marked: None,
                        unmarked: 0,
                        observed: observed.clone(),
                    });
                    window.focus(&field.read(app).focus.clone(), app);
                    app.new(|_| Root {
                        revision: 0,
                        field,
                        publication: PrepaintPublicationId::new(),
                        observed: observed.clone(),
                    })
                })
            })
            .expect("bind an ordinary typed root through the normal creation transaction");
        Self {
            surface,
            renderer,
            window,
            observed,
            wakes,
        }
    }

    fn facts(&self, cx: &TestAppContext) -> WindowPresentationFacts {
        cx.update(|app| {
            app.update_window(self.window.into(), |_, window, _| {
                window.presentation_facts()
            })
            .unwrap()
        })
    }

    fn frame(&self, cx: &TestAppContext) {
        self.surface
            .request_frame(RequestFrameOptions {
                force_render: true,
                require_presentation: true,
                ..Default::default()
            })
            .unwrap();
        assert!(self.surface.needs_frame());
        assert!(self.surface.drive_frame().unwrap());
        cx.update(|_| ());
    }

    fn prepare_successor(&self, cx: &TestAppContext) -> PlatformFrameSubmission {
        self.renderer
            .outcome
            .set(PlatformWindowPresentOutcome::Deferred);
        cx.update(|app| {
            self.window
                .update(app, |root, _, cx| {
                    root.revision += 1;
                    cx.notify();
                })
                .unwrap();
        });
        self.frame(cx);
        self.renderer.pending()
    }

    fn complete(&self, cx: &TestAppContext, token: &PlatformFrameSubmission) {
        assert!(token.complete(PlatformWindowPresentOutcome::Submitted));
        cx.update(|_| ());
        cx.run_until_parked();
    }

    fn field(&self, cx: &TestAppContext) -> Entity<Field> {
        cx.read(|app| self.window.read(app).unwrap().field.clone())
    }
}

fn assert_logical_facts(facts: WindowPresentationFacts) {
    assert!(!facts.native_created);
    assert!(!facts.native_visible);
}

#[test]
fn initial_frame_publishes_only_the_exact_original_submission_without_native_facts() {
    let cx = TestAppContext::single();
    let consumer = Consumer::open(&cx, PlatformWindowPresentOutcome::Deferred, "old");
    let first = consumer.renderer.pending();
    assert_eq!(first.window_id(), consumer.window.window_id());
    let pending = consumer.facts(&cx);
    assert_logical_facts(pending);
    assert_eq!(pending.frame_accepted_generation, None);
    assert_eq!(pending.present_submitted_generation, None);
    assert_eq!(
        pending.initial_presentation,
        WindowInitialPresentationStatus::Pending
    );
    assert!(consumer.observed.published.borrow().is_empty());
    assert!(
        consumer
            .surface
            .with_input_handler(|_| ())
            .unwrap()
            .is_none()
    );
    let deliveries = consumer.renderer.deliveries.borrow().len();
    consumer.frame(&cx);
    assert_eq!(consumer.renderer.deliveries.borrow().len(), deliveries);
    assert_eq!(consumer.renderer.pending().attempt_id(), first.attempt_id());
    consumer.complete(&cx, &first);
    let submitted = consumer.facts(&cx);
    assert_logical_facts(submitted);
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
    assert_eq!(
        submitted.latest_present_attempt.unwrap().outcome,
        PlatformWindowPresentOutcome::Submitted
    );
    assert_eq!(consumer.observed.latest_revision(), 0);
    assert!(
        consumer
            .renderer
            .deliveries
            .borrow()
            .iter()
            .all(|draw| draw.quads > 0)
    );

    assert!(!first.complete(PlatformWindowPresentOutcome::Submitted));
    assert!(consumer.wakes.get() > 0);
    consumer.surface.close().unwrap();
    cx.quit();
}

#[test]
fn successor_keeps_submitted_generation_focus_and_the_original_text_lease() {
    let cx = TestAppContext::single();
    let consumer = Consumer::open(&cx, PlatformWindowPresentOutcome::Submitted, "old");
    consumer.frame(&cx);
    let submitted = consumer.facts(&cx);
    let focus = consumer
        .field(&cx)
        .read_with(&cx, |field, _| field.focus.clone());
    let bounds = consumer
        .surface
        .with_input_handler(|handler| handler.bounds_for_range(0..0))
        .unwrap()
        .unwrap();
    let publications = consumer.observed.published.borrow().len();
    let next = consumer.prepare_successor(&cx);
    assert_eq!(
        consumer.facts(&cx).present_submitted_generation,
        submitted.present_submitted_generation
    );
    assert_eq!(consumer.observed.latest_revision(), 0);
    assert_eq!(consumer.observed.published.borrow().len(), publications);
    cx.update(|app| {
        app.update_window(consumer.window.into(), |_, window, app| {
            assert_eq!(window.committed_focus(app).as_ref(), Some(&focus));
        })
        .unwrap()
    });
    consumer
        .surface
        .with_input_handler(|handler| {
            assert_eq!(
                handler.text_for_range(0..3, &mut None).as_deref(),
                Some("old")
            );
            assert_eq!(handler.bounds_for_range(0..0), bounds);
        })
        .unwrap()
        .expect("the original submitted text handler remains leased");
    assert!(next.is_pending());
    consumer.frame(&cx);
    assert_eq!(consumer.renderer.pending().attempt_id(), next.attempt_id());
    assert_eq!(consumer.observed.latest_revision(), 0);
    consumer.complete(&cx, &next);
    assert_eq!(consumer.observed.latest_revision(), 1);
    let (fence, _) = *consumer.observed.published.borrow().last().unwrap();
    assert_eq!(fence.window_id(), next.window_id());
    assert_eq!(fence.generation(), next.frame_generation());
    assert_eq!(
        consumer.facts(&cx).present_submitted_generation,
        Some(next.frame_generation())
    );
    assert!(!next.complete(PlatformWindowPresentOutcome::Submitted));
    consumer.surface.close().unwrap();
    cx.quit();
}

#[test]
fn submitted_control_and_text_input_cancel_the_successor_before_mutating() {
    for edit_text in [false, true] {
        let cx = TestAppContext::single();
        let consumer = Consumer::open(&cx, PlatformWindowPresentOutcome::Submitted, "a🦀b");
        consumer.frame(&cx);
        let submitted = consumer.facts(&cx);
        let candidate = consumer.prepare_successor(&cx);
        consumer.observed.watched.replace(Some(candidate.clone()));
        if edit_text {
            consumer
                .surface
                .with_input_handler(|handler| {
                    assert_eq!(
                        handler.text_for_range(1..3, &mut None).as_deref(),
                        Some("🦀")
                    );
                    assert!(candidate.is_pending());
                    handler.replace_text_in_range(Some(1..3), "中");
                })
                .unwrap()
                .expect("edit through the original ElementInputHandler lease");
            assert_eq!(
                consumer
                    .field(&cx)
                    .read_with(&cx, |field, _| field.text.clone()),
                "a中b"
            );
        } else {
            consumer
                .surface
                .dispatch_input(PlatformInput::MouseDown(MouseDownEvent {
                    button: MouseButton::Left,
                    position: point(px(8.0), px(8.0)),
                    ..Default::default()
                }))
                .unwrap();
            assert_eq!(consumer.observed.clicks.borrow().as_slice(), &[0]);
        }
        assert_eq!(
            consumer.observed.mutation_statuses.borrow().as_slice(),
            &[PlatformFrameSubmissionStatus::Cancelled]
        );
        assert!(!candidate.complete(PlatformWindowPresentOutcome::Submitted));
        assert_eq!(
            consumer.facts(&cx).present_submitted_generation,
            submitted.present_submitted_generation
        );
        assert_eq!(consumer.observed.latest_revision(), 0);
        assert_eq!(consumer.observed.discarded.get(), 0);
        consumer.surface.close().unwrap();
        cx.quit();
    }
}

#[test]
fn logical_roots_keep_independent_focus_text_and_submitted_controls() {
    let cx = TestAppContext::single();
    let first = Consumer::open(&cx, PlatformWindowPresentOutcome::Submitted, "one");
    let second = Consumer::open(&cx, PlatformWindowPresentOutcome::Submitted, "two");
    first.frame(&cx);
    second.frame(&cx);
    let second_facts = second.facts(&cx);
    let focuses = [first.field(&cx), second.field(&cx)]
        .map(|field| field.read_with(&cx, |field, _| field.focus.clone()));
    assert_ne!(focuses[0], focuses[1]);
    for (consumer, focus) in [(&first, &focuses[0]), (&second, &focuses[1])] {
        cx.update(|app| {
            app.update_window(consumer.window.into(), |_, window, app| {
                assert!(window.is_window_active());
                assert_eq!(window.committed_focus(app).as_ref(), Some(focus));
            })
            .unwrap()
        });
    }
    first
        .surface
        .with_input_handler(|handler| {
            handler.replace_text_in_range(Some(0..3), "edited");
        })
        .unwrap()
        .unwrap();
    first
        .surface
        .dispatch_input(PlatformInput::MouseDown(MouseDownEvent {
            button: MouseButton::Left,
            position: point(px(8.0), px(8.0)),
            ..Default::default()
        }))
        .unwrap();
    assert_eq!(
        first
            .field(&cx)
            .read_with(&cx, |field, _| field.text.clone()),
        "edited"
    );
    second
        .surface
        .with_input_handler(|handler| {
            assert_eq!(
                handler.text_for_range(0..3, &mut None).as_deref(),
                Some("two")
            );
        })
        .unwrap()
        .unwrap();
    assert_eq!(second.facts(&cx), second_facts);
    assert_eq!(first.observed.clicks.borrow().as_slice(), &[0]);
    assert!(second.observed.clicks.borrow().is_empty());
    first.surface.close().unwrap();
    assert!(!second.surface.is_closed());
    second.frame(&cx);
    second.surface.close().unwrap();
    cx.quit();
}

#[test]
fn creation_captures_hosted_capabilities_instead_of_the_app_desktop_profile() {
    let cx = TestAppContext::single();
    cx.set_platform_window_creation_capabilities(PlatformWindowCreationCapabilities {
        focus_on_appearing: WindowCreationSupport::Unsupported,
        ..Default::default()
    });
    cx.set_platform_window_mutation_capabilities(PlatformWindowMutationCapabilities {
        size: WindowMutationSupport::Live,
        topmost: WindowMutationSupport::Live,
        coordinate_space: WindowCoordinateSpace::GlobalScreen,
        ..Default::default()
    });
    let desktop = cx.read(|app| app.window_capabilities());
    let consumer = Consumer::open(&cx, PlatformWindowPresentOutcome::Submitted, "old");
    let captured = cx.read(|app| {
        app.window_profile(consumer.window.into())
            .unwrap()
            .capabilities
    });
    assert_ne!(captured, desktop);
    assert_eq!(
        captured.activation,
        PlatformWindowActivationSupport::Unsupported
    );
    assert_eq!(
        captured.creation.focus_on_appearing,
        WindowCreationSupport::Supported
    );
    assert_eq!(
        captured.mutations,
        PlatformWindowMutationCapabilities {
            activation_policy: WindowMutationSupport::CreationOnly,
            ..Default::default()
        }
    );
    assert_eq!(
        captured.mutations.coordinate_space,
        WindowCoordinateSpace::WindowLocal
    );
    cx.set_platform_window_mutation_capabilities(PlatformWindowMutationCapabilities::default());
    cx.update(|app| {
        assert_eq!(
            app.window_profile(consumer.window.into())
                .unwrap()
                .capabilities,
            captured
        );
        app.update_window(consumer.window.into(), |_, window, _| {
            assert_eq!(window.window_capabilities(), captured);
            assert!(matches!(
                window.resize(size(px(600.0), px(400.0))),
                WindowMutationDispatch::Unsupported
            ));
            assert!(matches!(
                window.request_topmost(true),
                WindowMutationDispatch::Unsupported
            ));
            assert_eq!(
                window.activate_window().snapshot().status().terminal(),
                Some(WindowActivationTerminal::Unsupported)
            );
            assert_eq!(window.viewport_size(), size(px(240.0), px(120.0)));
            assert_eq!(window.platform_facts().display_id, None);
            assert_eq!(window.platform_facts().physical_geometry, None);
            assert!(matches!(
                raw_window_handle::HasWindowHandle::window_handle(&*window),
                Err(raw_window_handle::HandleError::NotSupported)
            ));
            assert!(matches!(
                raw_window_handle::HasDisplayHandle::display_handle(&*window),
                Err(raw_window_handle::HandleError::NotSupported)
            ));
            assert_logical_facts(window.presentation_facts());
        })
        .unwrap();
    });
    consumer.surface.close().unwrap();
    cx.quit();
}

#[test]
fn geometry_changes_invalidate_the_original_candidate_including_equal_and_aba_extents() {
    for (next_size, next_scale, restore) in [
        (size(px(300.0), px(160.0)), 1.0, false),
        (size(px(240.0), px(120.0)), 1.0, false),
        (size(px(300.0), px(160.0)), 1.0, true),
        (size(px(120.0), px(60.0)), 2.0, false),
    ] {
        let cx = TestAppContext::single();
        let consumer = Consumer::open(&cx, PlatformWindowPresentOutcome::Submitted, "old");
        consumer.frame(&cx);
        let submitted = consumer.facts(&cx).present_submitted_generation;
        let original_geometry = consumer.surface.geometry();
        let candidate = consumer.prepare_successor(&cx);
        consumer
            .surface
            .set_geometry(next_size, next_scale)
            .unwrap();
        assert_eq!(candidate.status(), PlatformFrameSubmissionStatus::Cancelled);
        if restore {
            consumer
                .surface
                .set_geometry(original_geometry.size(), original_geometry.scale_factor())
                .unwrap();
        }
        let expected = consumer.surface.geometry();
        if next_scale == 2.0 {
            assert_eq!(expected.physical_size(), original_geometry.physical_size());
            assert_ne!(expected.size(), original_geometry.size());
            assert_ne!(expected.scale_factor(), original_geometry.scale_factor());
        }
        assert!(!candidate.complete(PlatformWindowPresentOutcome::Submitted));
        assert_eq!(consumer.facts(&cx).present_submitted_generation, submitted);
        consumer.frame(&cx);
        let replacement = consumer.renderer.pending();
        assert_ne!(replacement.attempt_id(), candidate.attempt_id());
        assert_eq!(replacement.window_id(), candidate.window_id());
        let delivered = consumer
            .renderer
            .deliveries
            .borrow()
            .last()
            .unwrap()
            .geometry;
        assert_eq!(delivered, expected);
        cx.update(|app| {
            app.update_window(consumer.window.into(), |_, window, _| {
                assert_eq!(window.viewport_size(), expected.size());
                assert_eq!(window.scale_factor(), expected.scale_factor());
            })
            .unwrap()
        });
        consumer.complete(&cx, &replacement);
        assert_eq!(
            consumer.facts(&cx).present_submitted_generation,
            Some(replacement.frame_generation())
        );
        assert_logical_facts(consumer.facts(&cx));
        consumer.surface.close().unwrap();
        cx.quit();
    }
}

#[test]
fn zero_extent_suspends_renderer_delivery_and_restore_builds_fresh_output() {
    let cx = TestAppContext::single();
    let consumer = Consumer::open(&cx, PlatformWindowPresentOutcome::Submitted, "old");
    consumer.frame(&cx);
    let submitted = consumer.facts(&cx).present_submitted_generation;
    let candidate = consumer.prepare_successor(&cx);
    let deliveries = consumer.renderer.deliveries.borrow().len();
    consumer
        .surface
        .set_geometry(size(px(0.0), px(120.0)), 1.0)
        .unwrap();
    assert_eq!(candidate.status(), PlatformFrameSubmissionStatus::Cancelled);
    consumer.frame(&cx);
    assert_eq!(consumer.renderer.deliveries.borrow().len(), deliveries);
    assert_eq!(consumer.facts(&cx).present_submitted_generation, submitted);
    assert!(!candidate.complete(PlatformWindowPresentOutcome::Submitted));
    consumer
        .surface
        .set_geometry(size(px(240.0), px(120.0)), 1.5)
        .unwrap();
    consumer.frame(&cx);
    let restored = consumer.renderer.pending();
    assert_ne!(restored.attempt_id(), candidate.attempt_id());
    let draws = consumer.renderer.deliveries.borrow();
    let draw = draws.last().unwrap();
    assert_eq!(draw.geometry, consumer.surface.geometry());
    assert!(draw.quads > 0);
    drop(draws);
    consumer.complete(&cx, &restored);
    assert_eq!(
        consumer.facts(&cx).present_submitted_generation,
        Some(restored.frame_generation())
    );
    consumer.surface.close().unwrap();
    cx.quit();

    let cx = TestAppContext::single();
    let consumer = Consumer::open_with_options(
        &cx,
        PlatformWindowPresentOutcome::Deferred,
        "old",
        HostedSurfaceOptions {
            size: size(px(0.0), px(120.0)),
            ..Default::default()
        },
    );
    let original = consumer.window;
    let pending = consumer.facts(&cx);
    assert_logical_facts(pending);
    assert_eq!(pending.frame_accepted_generation, None);
    assert_eq!(pending.present_submitted_generation, None);
    assert_eq!(pending.non_empty_presented_generation, None);
    assert_eq!(
        pending.initial_presentation,
        WindowInitialPresentationStatus::Pending
    );
    assert!(consumer.renderer.deliveries.borrow().is_empty());
    assert!(consumer.observed.published.borrow().is_empty());
    cx.update(|app| {
        assert_eq!(app.windows(), [original.into()]);
        assert!(app.window_profile(original.into()).is_some());
        app.update_window(original.into(), |_, window, _| {
            assert!(window.creation_facts().show);
            assert!(!window.creation_facts().focus_on_appearing);
            assert_eq!(window.viewport_size(), size(px(0.0), px(120.0)));
        })
        .unwrap();
    });
    consumer.frame(&cx);
    assert_eq!(consumer.facts(&cx), pending);
    assert!(consumer.renderer.deliveries.borrow().is_empty());
    assert!(consumer.observed.published.borrow().is_empty());
    assert!(
        consumer
            .surface
            .with_input_handler(|_| ())
            .unwrap()
            .is_none()
    );
    consumer
        .surface
        .set_geometry(size(px(240.0), px(120.0)), 1.5)
        .unwrap();
    consumer.frame(&cx);
    let restored = consumer.renderer.pending();
    assert_eq!(restored.window_id(), original.window_id());
    let delivered = consumer
        .renderer
        .deliveries
        .borrow()
        .last()
        .unwrap()
        .geometry;
    assert_eq!(delivered, consumer.surface.geometry());
    assert!(consumer.renderer.deliveries.borrow().last().unwrap().quads > 0);
    assert_eq!(consumer.facts(&cx), pending);
    assert!(consumer.observed.published.borrow().is_empty());
    consumer.complete(&cx, &restored);
    let submitted = consumer.facts(&cx);
    assert_logical_facts(submitted);
    assert_eq!(
        submitted.frame_accepted_generation,
        Some(restored.frame_generation())
    );
    assert_eq!(
        submitted.present_submitted_generation,
        Some(restored.frame_generation())
    );
    assert_eq!(
        submitted.non_empty_presented_generation,
        Some(restored.frame_generation())
    );
    assert_eq!(
        submitted.initial_presentation,
        WindowInitialPresentationStatus::Completed
    );
    assert_eq!(consumer.observed.latest_revision(), 0);
    consumer
        .surface
        .with_input_handler(|handler| {
            assert_eq!(
                handler.text_for_range(0..3, &mut None).as_deref(),
                Some("old")
            );
        })
        .unwrap()
        .expect("restoration must install the original root's submitted text handler");
    consumer.surface.close().unwrap();
    cx.quit();
}

#[test]
fn invalid_geometry_refuses_before_disturbing_a_bound_root_or_its_original_token() {
    let cx = TestAppContext::single();
    let consumer = Consumer::open(&cx, PlatformWindowPresentOutcome::Submitted, "old");
    consumer.frame(&cx);
    let candidate = consumer.prepare_successor(&cx);
    let geometry = consumer.surface.geometry();
    let facts = consumer.facts(&cx);
    for (size, scale) in [
        (size(px(-1.0), px(120.0)), 1.0),
        (size(px(f32::NAN), px(120.0)), 1.0),
        (size(px(240.0), px(f32::INFINITY)), 1.0),
        (size(px(240.0), px(120.0)), 0.0),
        (size(px(240.0), px(120.0)), f32::NAN),
    ] {
        assert!(consumer.surface.set_geometry(size, scale).is_err());
        assert_eq!(consumer.surface.geometry(), geometry);
        assert!(candidate.is_pending());
        assert_eq!(consumer.facts(&cx), facts);
    }
    consumer.complete(&cx, &candidate);
    consumer.surface.close().unwrap();
    cx.quit();
}

#[test]
fn exact_close_detaches_after_original_quiescence_and_preserves_both_sibling_kinds() {
    let cx = TestAppContext::single();
    let consumer = Consumer::open(&cx, PlatformWindowPresentOutcome::Submitted, "old");
    let sibling = Consumer::open(&cx, PlatformWindowPresentOutcome::Submitted, "two");
    let native = cx
        .update(|app| {
            app.open_window_detailed(
                WindowOptions {
                    focus_on_appearing: false,
                    ..Default::default()
                },
                |_, app| app.new(|_| Empty),
            )
        })
        .unwrap();
    consumer.frame(&cx);
    sibling.frame(&cx);
    let weak_root = consumer.window.entity(&cx).unwrap().downgrade();
    let retained_field = consumer.field(&cx);
    let weak_field = retained_field.downgrade();
    consumer
        .surface
        .with_input_handler(|handler| {
            handler.replace_and_mark_text_in_range(None, "组合", None);
        })
        .unwrap()
        .expect("the submitted field must receive a real marked composition");
    assert_eq!(
        cx.read(|app| retained_field.read(app).marked.clone()),
        Some(0..2)
    );
    let candidate = consumer.prepare_successor(&cx);
    let original = consumer.window;
    let terminals = Rc::new(RefCell::new(Vec::new()));
    let _terminal = cx.update(|app| {
        let terminals = terminals.clone();
        let renderer = consumer.renderer.clone();
        app.on_window_native_terminal(move |app, id| {
            if id == original.window_id() {
                assert!(!app.windows().contains(&original.into()));
                assert!(app.window_profile(original.into()).is_none());
                let shutdowns = renderer.shutdowns.borrow();
                assert_eq!(shutdowns.len(), 1);
                let snapshot = shutdowns[0].snapshot();
                assert_eq!(snapshot.window_id(), id);
                assert!(snapshot.generation() > 0);
                assert!(snapshot.quiesced());
                assert!(snapshot.detached());
                assert!(!snapshot.native_terminal());
                assert!(!snapshot.protocol_violation());
                terminals.borrow_mut().push(snapshot);
            }
        })
    });
    let clone = consumer.surface.clone();
    clone.close().unwrap();
    cx.update(|_| ());
    cx.run_until_parked();
    assert!(consumer.surface.is_closed());
    assert_eq!(candidate.status(), PlatformFrameSubmissionStatus::Cancelled);
    assert!(!candidate.complete(PlatformWindowPresentOutcome::Submitted));
    assert!(weak_root.upgrade().is_none());
    cx.read(|app| {
        let field = retained_field.read(app);
        assert_eq!(
            field.marked, None,
            "original Window cleanup must finish retained composition"
        );
        assert_eq!(field.unmarked, 1);
        assert_eq!(field.text, "组合");
    });
    drop(retained_field);
    assert!(weak_field.upgrade().is_none());
    assert_eq!(consumer.renderer.quiesced.get(), 1);
    assert_eq!(terminals.borrow().len(), 1);
    assert_eq!(
        consumer.renderer.shutdowns.borrow()[0].snapshot(),
        terminals.borrow()[0]
    );
    clone.close().unwrap();
    assert_eq!(consumer.renderer.quiesced.get(), 1);
    assert_eq!(terminals.borrow().len(), 1);
    assert!(!clone.needs_frame());
    assert!(clone.drive_frame().is_err());
    assert!(clone.request_frame(RequestFrameOptions::default()).is_err());
    assert!(clone.with_input_handler(|_| ()).is_err());
    assert!(clone.set_geometry(size(px(80.0), px(60.0)), 1.0).is_err());
    cx.update(|app| {
        assert!(original.update(app, |_, _, _| ()).is_err());
        assert!(app.window_profile(original.into()).is_none());
        assert!(app.windows().contains(&sibling.window.into()));
        assert!(app.windows().contains(&native.into()));
        native
            .update(app, |_, window, _| {
                assert!(window.presentation_facts().native_created)
            })
            .unwrap();
    });
    sibling
        .surface
        .with_input_handler(|handler| {
            assert_eq!(
                handler.text_for_range(0..3, &mut None).as_deref(),
                Some("two")
            );
        })
        .unwrap()
        .unwrap();
    sibling.frame(&cx);
    let replacement = Consumer::open(&cx, PlatformWindowPresentOutcome::Submitted, "new");
    assert_ne!(replacement.window.window_id(), original.window_id());
    assert!(!candidate.complete(PlatformWindowPresentOutcome::Submitted));
    replacement.window.entity(&cx).unwrap();
    replacement.surface.close().unwrap();
    sibling.surface.close().unwrap();
    cx.update(|app| app.update_window(native.into(), |_, window, app| window.remove_window(app)))
        .unwrap();
    cx.quit();
}

#[test]
fn second_binding_refusal_preserves_the_first_typed_root_and_original_pending_submission() {
    let cx = TestAppContext::single();
    let consumer = Consumer::open(&cx, PlatformWindowPresentOutcome::Submitted, "old");
    consumer.frame(&cx);
    let candidate = consumer.prepare_successor(&cx);
    let root_called = Cell::new(false);
    let clone = consumer.surface.clone();
    let error = cx
        .update(|app| {
            app.open_surface(&clone, |_, app| {
                root_called.set(true);
                app.new(|_| Empty)
            })
        })
        .expect_err("one production surface must not bind a second root");
    assert_eq!(error.stage(), WindowOpenFailureStage::PlatformCreateOrMap);
    assert!(!root_called.get());
    assert!(!consumer.surface.is_closed());
    assert!(candidate.is_pending());
    cx.read(|app| assert_eq!(app.windows(), [consumer.window.into()]));
    consumer.complete(&cx, &candidate);
    assert_eq!(consumer.observed.latest_revision(), 1);
    consumer.surface.close().unwrap();
    cx.quit();
}

#[test]
fn refused_initial_submission_and_root_unwind_roll_back_the_normal_typed_reservation() {
    for unwind in [false, true] {
        let cx = TestAppContext::single();
        cx.update(|app| app.set_quit_mode(QuitMode::Explicit));
        let renderer = SimulatedRenderer::new(PlatformWindowPresentOutcome::Rejected);
        let surface =
            HostedSurface::new(HostedSurfaceOptions::default(), renderer.clone(), || {}).unwrap();
        let reserved = Cell::new(None);
        let weak_root = RefCell::new(None);
        let result = catch_unwind(AssertUnwindSafe(|| {
            cx.update(|app| {
                app.open_surface(&surface, |window, app| {
                    reserved.set(Some(window.window_handle()));
                    if unwind {
                        panic!("hosted root construction unwound");
                    }
                    let field = app.new(|cx| Field {
                        focus: cx.focus_handle(),
                        text: "old".into(),
                        marked: None,
                        unmarked: 0,
                        observed: Rc::new(Observations::default()),
                    });
                    let root = app.new(|_| Root {
                        revision: 0,
                        field,
                        publication: PrepaintPublicationId::new(),
                        observed: Rc::new(Observations::default()),
                    });
                    weak_root.replace(Some(root.downgrade()));
                    root
                })
            })
        }));
        if unwind {
            let panic = result.expect_err("the original constructor panic must propagate");
            assert_eq!(
                panic.downcast_ref::<&str>(),
                Some(&"hosted root construction unwound")
            );
        } else {
            let error = result
                .unwrap()
                .expect_err("a rejected initial submission must roll back");
            assert_eq!(
                error.stage(),
                WindowOpenFailureStage::BeforeVisibilityPresentation
            );
            assert!(!renderer.deliveries.borrow().is_empty());
            assert!(weak_root.borrow().as_ref().unwrap().upgrade().is_none());
        }
        let original = reserved.get().unwrap();
        cx.update(|app| {
            assert!(app.windows().is_empty());
            assert!(app.window_profile(original).is_none());
            assert!(app.update_window(original, |_, _, _| ()).is_err());
        });
        assert!(surface.is_closed());
        assert!(surface.drive_frame().is_err());
        let retry = Consumer::open(&cx, PlatformWindowPresentOutcome::Submitted, "new");
        assert_ne!(retry.window.window_id(), original.window_id());
        retry.surface.close().unwrap();
        cx.quit();
    }
}
