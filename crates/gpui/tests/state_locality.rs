#![cfg(feature = "test-support")]

//! Public CPU consumers of Entity, observe, Subscription and cached View locality.
//! Projection values become quad colors; caller-owned counters record actual render and paint
//! callbacks. Submitted test-platform scenes establish CPU publication without GPU or timing claims.

use futures::channel::oneshot;
use open_gpui::{
    App, AppContext as _, Bounds, Context, Entity, EntityId, InteractiveElement as _, IntoElement,
    MouseButton, ParentElement as _, Pixels, PlatformWindowPresentOutcome, QuitMode, Render,
    RequestFrameOptions, StyleRefinement, Styled as _, Subscription, TestAppContext,
    VisualTestContext, WeakEntity, Window, WindowBounds, WindowHandle, WindowOptions, canvas, div,
    fill, point, px, rgb, size,
};
use std::{cell::Cell, rc::Rc};

#[derive(Clone, Copy)]
struct Item {
    label: u32,
    amount: u32,
    detail_height: u32,
}

struct Document {
    revision: u64,
    selected: usize,
    items: [Item; 2],
}

impl Default for Document {
    fn default() -> Self {
        Self {
            revision: 0,
            selected: 0,
            items: [
                Item {
                    label: 3,
                    amount: 11,
                    detail_height: 48,
                },
                Item {
                    label: 7,
                    amount: 42,
                    detail_height: 64,
                },
            ],
        }
    }
}

impl Document {
    fn basis(&self, source: EntityId) -> OperationBasis {
        OperationBasis {
            source,
            revision: self.revision,
            selected: self.selected,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct OperationBasis {
    source: EntityId,
    revision: u64,
    selected: usize,
}

#[derive(Clone, Copy)]
enum PanelKind {
    Outline,
    Inspector,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Projection {
    value: u32,
    height: u32,
}

impl PanelKind {
    fn project(self, document: &Document) -> Projection {
        let item = document.items[document.selected];
        match self {
            Self::Outline => Projection {
                value: item.label,
                height: 48,
            },
            Self::Inspector => Projection {
                value: item.amount / 10,
                height: item.detail_height,
            },
        }
    }
}

#[derive(Default)]
struct Trace {
    observations: Rc<Cell<usize>>,
    renders: Cell<usize>,
    paints: Cell<usize>,
    releases: Cell<usize>,
    painted: Cell<Option<Projection>>,
    bounds: Cell<Option<Bounds<Pixels>>>,
    operated_basis: Cell<Option<OperationBasis>>,
}

fn projection_canvas(projection: Projection, trace: Rc<Trace>) -> impl IntoElement {
    canvas(
        |_, _, _| (),
        move |bounds, (), window, _| {
            assert!(bounds.size.width > px(0.));
            assert!(bounds.size.height > px(0.));
            trace.paints.set(trace.paints.get() + 1);
            trace.painted.set(Some(projection));
            trace.bounds.set(Some(bounds));
            window.paint_quad(fill(bounds, rgb(0x305080 + projection.value)));
        },
    )
    .size_full()
}

struct Panel {
    source: Entity<Document>,
    kind: PanelKind,
    projection: Projection,
    basis: OperationBasis,
    request_generation: u64,
    trace: Rc<Trace>,
    _source_subscription: Subscription,
}

impl Panel {
    fn new(
        source: Entity<Document>,
        kind: PanelKind,
        trace: Rc<Trace>,
        cx: &mut Context<Self>,
    ) -> Self {
        let document = source.read(cx);
        let projection = kind.project(document);
        let basis = document.basis(source.entity_id());
        let subscription = Self::observe_source(&source, &trace, cx);
        cx.on_release({
            let trace = Rc::clone(&trace);
            move |_, _| trace.releases.set(trace.releases.get() + 1)
        })
        .detach();
        Self {
            source,
            kind,
            projection,
            basis,
            request_generation: 0,
            trace,
            _source_subscription: subscription,
        }
    }

    fn observe_source(
        source: &Entity<Document>,
        trace: &Trace,
        cx: &mut Context<Self>,
    ) -> Subscription {
        let observations = Rc::clone(&trace.observations);
        cx.observe(source, move |panel, _, cx| {
            observations.set(observations.get() + 1);
            panel.synchronize(cx);
        })
    }

    fn synchronize(&mut self, cx: &mut Context<Self>) {
        let document = self.source.read(cx);
        let basis = document.basis(self.source.entity_id());
        let projection = self.kind.project(document);
        self.basis = basis;
        if self.projection != projection {
            self.projection = projection;
            cx.notify();
        }
    }

    fn rebind(&mut self, source: Entity<Document>, cx: &mut Context<Self>) {
        self._source_subscription = Self::observe_source(&source, &self.trace, cx);
        self.source = source;
        self.request_generation += 1;
        self.synchronize(cx);
    }

    fn increment(&mut self, cx: &mut Context<Self>) {
        let basis = self.basis;
        if basis != self.source.read(cx).basis(self.source.entity_id()) {
            return;
        }
        self.source.update(cx, |document, cx| {
            document.items[basis.selected].amount += 10;
            document.revision += 1;
            cx.notify();
        });
        self.trace.operated_basis.set(Some(basis));
    }

    fn apply_result(&mut self, ticket: Ticket, amount: u32, cx: &mut Context<Self>) -> Completion {
        if ticket.generation != self.request_generation
            || ticket.basis != self.basis
            || ticket.basis != self.source.read(cx).basis(self.source.entity_id())
        {
            return Completion::Superseded;
        }
        self.source.update(cx, |document, cx| {
            document.items[ticket.basis.selected].amount = amount;
            document.revision += 1;
            cx.notify();
        });
        Completion::Applied
    }
}

impl Render for Panel {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.trace.renders.set(self.trace.renders.get() + 1);
        div()
            .size_full()
            .child(projection_canvas(self.projection, Rc::clone(&self.trace)))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|panel, _, _, cx| panel.increment(cx)),
            )
    }
}

struct Panels {
    panels: [Option<Entity<Panel>>; 2],
    _broad_refresh: Option<Subscription>,
}

impl Render for Panels {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .flex()
            .items_start()
            .bg(rgb(0x101820))
            .children(self.panels.iter().flatten().map(|panel| {
                let height = panel.read(cx).projection.height;
                panel.clone().cached(
                    StyleRefinement::default()
                        .w(px(104.))
                        .h(px(height as f32))
                        .flex_shrink_0(),
                )
            }))
    }
}

fn open_root<V: Render>(
    cx: &mut TestAppContext,
    build: impl FnOnce(&mut Window, &mut App) -> Entity<V>,
) -> (WindowHandle<V>, VisualTestContext) {
    cx.defer_next_window_frame_requests();
    let handle = cx.update(|app| {
        app.set_quit_mode(QuitMode::Explicit);
        app.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds::new(
                    point(px(0.), px(0.)),
                    size(px(240.), px(160.)),
                ))),
                focus_on_appearing: false,
                inactive_frame_interval: None,
                low_power_frame_interval: None,
                thermal_frame_interval: None,
                ..Default::default()
            },
            build,
        )
        .unwrap()
    });
    let mut visual = VisualTestContext::from_window(handle.into(), cx);
    let _activation = visual.update(|window, _| window.activate_window());
    visual.run_until_parked();
    visual.update(|window, app| {
        assert!(window.is_window_active());
        window.draw(app).clear(app);
    });
    submit_scene(&mut visual);
    (handle, visual)
}

fn submit_scene(visual: &mut VisualTestContext) -> u64 {
    assert!(visual.simulate_frame(RequestFrameOptions {
        require_presentation: true,
        ..Default::default()
    }));
    visual.update(|window, _| {
        let facts = window.presentation_facts();
        let generation = facts.frame_accepted_generation.unwrap();
        assert_eq!(facts.present_submitted_generation, Some(generation));
        assert_eq!(facts.non_empty_presented_generation, Some(generation));
        let attempt = facts.latest_present_attempt.unwrap();
        assert_eq!(attempt.generation, generation);
        assert_eq!(attempt.outcome, PlatformWindowPresentOutcome::Submitted);
        assert!(attempt.contained_valid_primitives);
        generation
    })
}

struct Fixture {
    root: WindowHandle<Panels>,
    visual: VisualTestContext,
    source: Entity<Document>,
    source_releases: Rc<Cell<usize>>,
    broad_refreshes: Rc<Cell<usize>>,
    panels: [WeakEntity<Panel>; 2],
    traces: [Rc<Trace>; 2],
}

impl Fixture {
    fn new(cx: &mut TestAppContext, broad_refresh: bool) -> Self {
        let source_releases = Rc::new(Cell::new(0));
        let source = cx.new(|cx| {
            let releases = Rc::clone(&source_releases);
            cx.on_release(move |_, _| releases.set(releases.get() + 1))
                .detach();
            Document::default()
        });
        let traces = [Rc::new(Trace::default()), Rc::new(Trace::default())];
        let broad_refreshes = Rc::new(Cell::new(0));
        let (root, visual) = open_root(cx, |window, app| {
            let panels = [PanelKind::Outline, PanelKind::Inspector].map(|kind| {
                let index = match kind {
                    PanelKind::Outline => 0,
                    PanelKind::Inspector => 1,
                };
                Some(app.new(|cx| Panel::new(source.clone(), kind, Rc::clone(&traces[index]), cx)))
            });
            let refresh = broad_refresh.then(|| {
                let refreshes = Rc::clone(&broad_refreshes);
                window.observe(&source, app, move |_, window, _| {
                    refreshes.set(refreshes.get() + 1);
                    window.refresh();
                })
            });
            app.new(|_| Panels {
                panels,
                _broad_refresh: refresh,
            })
        });
        let panels = root.entity(&visual).unwrap().read_with(&visual, |root, _| {
            root.panels
                .each_ref()
                .map(|panel| panel.as_ref().unwrap().downgrade())
        });
        Self {
            root,
            visual,
            source,
            source_releases,
            broad_refreshes,
            panels,
            traces,
        }
    }

    fn change(&mut self, change: impl FnOnce(&mut Document)) {
        self.source.update(&mut self.visual, |document, cx| {
            change(document);
            document.revision += 1;
            cx.notify();
        });
    }

    fn renders(&self) -> [usize; 2] {
        self.traces.each_ref().map(|trace| trace.renders.get())
    }

    fn paints(&self) -> [usize; 2] {
        self.traces.each_ref().map(|trace| trace.paints.get())
    }

    fn observations(&self) -> [usize; 2] {
        self.traces.each_ref().map(|trace| trace.observations.get())
    }

    fn values(&self) -> [u32; 2] {
        self.traces
            .each_ref()
            .map(|trace| trace.painted.get().unwrap().value)
    }
}

#[derive(Clone, Copy)]
struct Ticket {
    generation: u64,
    basis: OperationBasis,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Completion {
    Applied,
    Superseded,
    Released,
}

fn pending_result(
    panel: &WeakEntity<Panel>,
    visual: &mut VisualTestContext,
) -> (oneshot::Sender<u32>, Rc<Cell<Option<Completion>>>) {
    let (sender, receiver) = oneshot::channel();
    let completion = Rc::new(Cell::new(None));
    panel
        .update(visual, |panel, cx| {
            panel.request_generation += 1;
            let ticket = Ticket {
                generation: panel.request_generation,
                basis: panel.basis,
            };
            let completion = Rc::clone(&completion);
            cx.spawn(async move |panel, cx| {
                let amount = receiver.await.unwrap();
                let outcome = panel
                    .update(cx, |panel, cx| panel.apply_result(ticket, amount, cx))
                    .unwrap_or(Completion::Released);
                completion.set(Some(outcome));
            })
            .detach();
        })
        .unwrap();
    visual.run_until_parked();
    assert_eq!(completion.get(), None);
    (sender, completion)
}

#[open_gpui::test]
fn narrow_observers_rebuild_only_the_changed_panel_against_broad_window_refresh(
    cx: &mut TestAppContext,
) {
    for broad_refresh in [true, false] {
        let mut fixture = Fixture::new(cx, broad_refresh);
        assert_eq!(fixture.values(), [3, 1]);
        assert!(fixture.renders().iter().all(|count| *count > 0));
        assert!(fixture.paints().iter().all(|count| *count > 0));
        let renders = fixture.renders();
        let paints = fixture.paints();
        let observations = fixture.observations();
        let generation = submit_scene(&mut fixture.visual);

        fixture.change(|document| document.items[0].label = 5);
        assert!(submit_scene(&mut fixture.visual) > generation);
        let inspector_rebuilds = usize::from(broad_refresh);
        assert_eq!(fixture.values(), [5, 1]);
        assert_eq!(
            fixture.renders(),
            [renders[0] + 1, renders[1] + inspector_rebuilds]
        );
        assert_eq!(
            fixture.paints(),
            [paints[0] + 1, paints[1] + inspector_rebuilds]
        );
        assert_eq!(fixture.observations(), observations.map(|count| count + 1));
        assert_eq!(fixture.broad_refreshes.get(), usize::from(broad_refresh));

        let renders = fixture.renders();
        let paints = fixture.paints();
        fixture.change(|document| document.items[0].amount = 31);
        submit_scene(&mut fixture.visual);
        let outline_rebuilds = usize::from(broad_refresh);
        assert_eq!(fixture.values(), [5, 3]);
        assert_eq!(
            fixture.renders(),
            [renders[0] + outline_rebuilds, renders[1] + 1]
        );
        assert_eq!(
            fixture.paints(),
            [paints[0] + outline_rebuilds, paints[1] + 1]
        );
        assert_eq!(
            fixture.broad_refreshes.get(),
            2 * usize::from(broad_refresh)
        );
        assert!(fixture.visual.simulate_close());
    }
}

#[open_gpui::test]
fn equal_projection_advances_operation_basis_without_rebuilding_cached_handlers(
    cx: &mut TestAppContext,
) {
    let mut fixture = Fixture::new(cx, false);
    let renders = fixture.renders();
    let paints = fixture.paints();
    let observations = fixture.observations();
    let (sender, completion) = pending_result(&fixture.panels[1], &mut fixture.visual);

    fixture.change(|document| document.items[0].amount = 12);
    submit_scene(&mut fixture.visual);
    assert_eq!(fixture.values(), [3, 1]);
    assert_eq!(fixture.renders(), renders);
    assert_eq!(fixture.paints(), paints);
    assert_eq!(fixture.observations(), observations.map(|count| count + 1));
    let basis = fixture.panels[1]
        .read_with(&fixture.visual, |panel, _| panel.basis)
        .unwrap();
    assert_eq!(basis.revision, 1);
    assert_eq!(basis.selected, 0);
    assert_eq!(basis.source, fixture.source.entity_id());

    sender.send(99).unwrap();
    fixture.visual.run_until_parked();
    assert_eq!(completion.get(), Some(Completion::Superseded));
    assert_eq!(
        fixture
            .source
            .read_with(&fixture.visual, |document, _| document.items[0].amount),
        12
    );
    assert_eq!(fixture.renders(), renders);

    fixture.change(|document| {
        document.items[1] = document.items[0];
        document.selected = 1;
    });
    submit_scene(&mut fixture.visual);
    assert_eq!(fixture.renders(), renders);
    assert_eq!(fixture.paints(), paints);
    let selected_basis = fixture.panels[1]
        .read_with(&fixture.visual, |panel, _| panel.basis)
        .unwrap();
    assert_eq!(selected_basis.revision, 2);
    assert_eq!(selected_basis.selected, 1);

    fixture
        .visual
        .simulate_click(point(px(112.), px(12.)), Default::default());
    submit_scene(&mut fixture.visual);
    assert_eq!(fixture.traces[1].operated_basis.get(), Some(selected_basis));
    assert_eq!(
        fixture.source.read_with(&fixture.visual, |document, _| {
            document.items.map(|item| item.amount)
        }),
        [12, 22]
    );
    assert_eq!(fixture.values(), [3, 2]);
    assert_eq!(fixture.renders(), [renders[0], renders[1] + 1]);
    assert_eq!(fixture.paints(), [paints[0], paints[1] + 1]);
    assert!(fixture.visual.simulate_close());
}

#[open_gpui::test]
fn selection_source_and_content_constraints_update_the_owning_projections(cx: &mut TestAppContext) {
    let mut fixture = Fixture::new(cx, false);
    let renders = fixture.renders();
    fixture.change(|document| document.selected = 1);
    submit_scene(&mut fixture.visual);
    assert_eq!(fixture.values(), [7, 4]);
    assert_eq!(fixture.renders(), renders.map(|count| count + 1));
    assert_eq!(fixture.traces[1].bounds.get().unwrap().size.height, px(64.));

    let replacement = fixture.visual.cx.new(|_| Document {
        revision: 7,
        items: [
            Item {
                label: 9,
                amount: 91,
                detail_height: 80,
            },
            Document::default().items[1],
        ],
        ..Document::default()
    });
    let renders = fixture.renders();
    fixture.panels[1]
        .update(&mut fixture.visual, |panel, cx| {
            panel.rebind(replacement.clone(), cx)
        })
        .unwrap();
    submit_scene(&mut fixture.visual);
    assert_eq!(fixture.values(), [7, 9]);
    assert_eq!(fixture.renders(), [renders[0], renders[1] + 1]);
    assert_eq!(fixture.traces[1].bounds.get().unwrap().size.height, px(80.));
    assert_eq!(
        fixture.panels[1]
            .read_with(&fixture.visual, |panel, _| panel.basis.source)
            .unwrap(),
        replacement.entity_id()
    );

    let observations = fixture.observations();
    let renders = fixture.renders();
    fixture.change(|document| document.items[1].amount = 73);
    submit_scene(&mut fixture.visual);
    assert_eq!(
        fixture.observations(),
        [observations[0] + 1, observations[1]]
    );
    assert_eq!(fixture.renders(), renders);
    assert_eq!(fixture.values(), [7, 9]);

    replacement.update(&mut fixture.visual, |document, cx| {
        document.items[0].detail_height = 96;
        document.revision += 1;
        cx.notify();
    });
    submit_scene(&mut fixture.visual);
    assert_eq!(fixture.renders(), [renders[0], renders[1] + 1]);
    assert_eq!(fixture.values(), [7, 9]);
    assert_eq!(fixture.traces[1].bounds.get().unwrap().size.height, px(96.));
    assert!(fixture.visual.simulate_close());
}

#[open_gpui::test]
fn panel_release_and_request_generation_reject_late_results_after_replacement(
    cx: &mut TestAppContext,
) {
    let mut fixture = Fixture::new(cx, false);
    let old_panel = fixture.panels[1].clone();
    let old_trace = Rc::clone(&fixture.traces[1]);
    assert_eq!(Rc::strong_count(&old_trace.observations), 2);
    let (late_sender, late_completion) = pending_result(&old_panel, &mut fixture.visual);
    fixture
        .root
        .update(&mut fixture.visual, |root, _, cx| {
            root.panels[1] = None;
            cx.notify();
        })
        .unwrap();
    submit_scene(&mut fixture.visual);
    old_panel.assert_released();
    assert_eq!(old_trace.releases.get(), 1);
    assert_eq!(Rc::strong_count(&old_trace.observations), 1);
    let old_observations = old_trace.observations.get();

    let trace = Rc::new(Trace::default());
    let source = fixture.source.clone();
    fixture.panels[1] = fixture
        .root
        .update(&mut fixture.visual, |root, _, cx| {
            let panel =
                cx.new(|cx| Panel::new(source, PanelKind::Inspector, Rc::clone(&trace), cx));
            let weak = panel.downgrade();
            root.panels[1] = Some(panel);
            cx.notify();
            weak
        })
        .unwrap();
    fixture.traces[1] = trace;
    assert_ne!(old_panel.entity_id(), fixture.panels[1].entity_id());
    submit_scene(&mut fixture.visual);
    let renders = fixture.renders();
    late_sender.send(99).unwrap();
    fixture.visual.run_until_parked();
    assert_eq!(late_completion.get(), Some(Completion::Released));
    assert_eq!(fixture.values(), [3, 1]);
    assert_eq!(fixture.renders(), renders);
    assert_eq!(
        fixture
            .source
            .read_with(&fixture.visual, |document, _| document.items[0].amount),
        11
    );

    let (stale_sender, stale_completion) = pending_result(&fixture.panels[1], &mut fixture.visual);
    let (current_sender, current_completion) =
        pending_result(&fixture.panels[1], &mut fixture.visual);
    stale_sender.send(91).unwrap();
    fixture.visual.run_until_parked();
    assert_eq!(stale_completion.get(), Some(Completion::Superseded));
    assert_eq!(fixture.renders(), renders);
    assert_eq!(
        fixture
            .source
            .read_with(&fixture.visual, |document, _| document.items[0].amount),
        11
    );
    current_sender.send(61).unwrap();
    fixture.visual.run_until_parked();
    assert_eq!(current_completion.get(), Some(Completion::Applied));
    submit_scene(&mut fixture.visual);
    assert_eq!(fixture.values(), [3, 6]);
    assert_eq!(fixture.renders(), [renders[0], renders[1] + 1]);
    assert_eq!(old_trace.observations.get(), old_observations);
    assert_eq!(old_trace.releases.get(), 1);

    let source = fixture.source.downgrade();
    assert!(fixture.visual.simulate_close());
    for panel in &fixture.panels {
        panel.assert_released();
    }
    for trace in &fixture.traces {
        assert_eq!(Rc::strong_count(&trace.observations), 1);
    }
    assert_eq!(
        fixture.traces.each_ref().map(|trace| trace.releases.get()),
        [1, 1]
    );
    assert!(source.upgrade().is_some());
    assert_eq!(fixture.source_releases.get(), 0);
    drop(fixture.source);
    fixture.visual.cx.update(|_| ());
    source.assert_released();
    assert_eq!(fixture.source_releases.get(), 1);
}

struct DirectReader {
    source: Entity<Document>,
    trace: Rc<Trace>,
}

impl Render for DirectReader {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.trace.renders.set(self.trace.renders.get() + 1);
        projection_canvas(
            PanelKind::Inspector.project(self.source.read(cx)),
            Rc::clone(&self.trace),
        )
    }
}

struct DirectRoot {
    child: Entity<DirectReader>,
    renders: Rc<Cell<usize>>,
}

impl Render for DirectRoot {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        self.renders.set(self.renders.get() + 1);
        div().size_full().bg(rgb(0x101820)).child(
            self.child
                .clone()
                .cached(StyleRefinement::default().w(px(104.)).h(px(48.))),
        )
    }
}

#[open_gpui::test]
fn direct_non_view_notify_updates_the_window_while_cached_view_requires_owner_invalidation(
    cx: &mut TestAppContext,
) {
    let source = cx.new(|_| Document::default());
    let trace = Rc::new(Trace::default());
    let root_renders = Rc::new(Cell::new(0));
    let (_, mut visual) = open_root(cx, |_, app| {
        let child = app.new(|_| DirectReader {
            source: source.clone(),
            trace: Rc::clone(&trace),
        });
        app.new(|_| DirectRoot {
            child,
            renders: Rc::clone(&root_renders),
        })
    });
    let renders = trace.renders.get();
    let paints = trace.paints.get();
    let roots = root_renders.get();
    let generation = submit_scene(&mut visual);
    assert_eq!(trace.painted.get().unwrap().value, 1);
    source.update(&mut visual, |document, cx| {
        document.items[0].amount = 31;
        document.revision += 1;
        cx.notify();
    });
    assert!(submit_scene(&mut visual) > generation);
    assert!(root_renders.get() > roots);
    assert_eq!(trace.renders.get(), renders);
    assert_eq!(trace.paints.get(), paints);
    assert_eq!(trace.painted.get().unwrap().value, 1);

    visual.update(|window, _| window.refresh());
    submit_scene(&mut visual);
    assert_eq!(trace.renders.get(), renders + 1);
    assert_eq!(trace.paints.get(), paints + 1);
    assert_eq!(trace.painted.get().unwrap().value, 3);
    assert!(visual.simulate_close());
}
