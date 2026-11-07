#![cfg(feature = "bench-support")]

//! Independently consumes the maintained benchmark APIs without test-support.
//! The workloads execute native text shaping, source-driven rendering and seeded
//! background tasks. Platform submission facts do not establish native GPU output.

use criterion::Criterion;
use open_gpui::{
    App, AppContext as _, BenchAppContext, BenchMeasurement, Context, Entity, Font, FontRun,
    Global, IntoElement, ParentElement as _, PlatformWindowPresentOutcome, QuitMode, Render,
    Styled as _, Subscription, VisualContext as _, Window, canvas, div, fill, profiler, px, rgb,
    size,
};
use rand::{Rng as _, SeedableRng as _, rngs::StdRng};
use std::{
    cell::{Cell, RefCell},
    hint::black_box,
    rc::{Rc, Weak},
    time::Duration,
};

thread_local! {
    static PREVIOUS_ROUTINE: RefCell<Option<(Weak<RoutineOwner>, Rc<Cell<usize>>)>> =
        const { RefCell::new(None) };
}

struct RoutineOwner {
    operations: Cell<usize>,
    releases: Rc<Cell<usize>>,
}

impl Drop for RoutineOwner {
    fn drop(&mut self) {
        self.releases.set(self.releases.get() + 1);
    }
}

struct RoutineState(Rc<RoutineOwner>);

impl Global for RoutineState {}

fn assert_previous_routine_released() {
    PREVIOUS_ROUTINE.with(|previous| {
        if let Some((owner, releases)) = previous.borrow().as_ref() {
            assert!(owner.upgrade().is_none());
            assert_eq!(releases.get(), 1);
        }
    });
}

fn begin_routine(cx: &mut BenchAppContext) {
    assert_previous_routine_released();
    assert!(!profiler::trace_enabled());
    let releases = Rc::new(Cell::new(0));
    let owner = Rc::new(RoutineOwner {
        operations: Cell::new(0),
        releases: Rc::clone(&releases),
    });
    PREVIOUS_ROUTINE.with(|previous| {
        previous.replace(Some((Rc::downgrade(&owner), releases)));
    });
    cx.update(|app| {
        assert!(app.windows().is_empty());
        assert!(!app.has_global::<RoutineState>());
        app.set_quit_mode(QuitMode::Explicit);
        app.set_global(RoutineState(owner));
    });
}

fn record_operation(app: &App) {
    let owner = &app.global::<RoutineState>().0;
    owner.operations.set(owner.operations.get() + 1);
}

#[open_gpui::bench]
fn simple_native_shaping(cx: &mut BenchAppContext) {
    begin_routine(cx);
    let text_system = open_gpui_platform::bench_text_system();
    let font_id = text_system
        .font_id(&Font::default())
        .expect("the native benchmark text system must resolve its UI font");
    let text = "Open GPUI: office, café, λ, 日本語";
    let runs = [FontRun {
        len: text.len(),
        font_id,
    }];
    let mut last = None;
    cx.bench_iter(|cx| {
        last = Some(text_system.layout_line(black_box(text), px(16.), &runs));
        cx.update(|app| record_operation(app));
    });
    let layout = last.expect("Criterion must execute the native shaping workload");
    assert_eq!(layout.len, text.len());
    assert!(layout.width > px(0.));
    assert!(layout.ascent > px(0.));
    let glyphs = layout.runs.iter().flat_map(|run| &run.glyphs);
    assert!(glyphs.clone().count() > 0);
    assert!(
        glyphs
            .clone()
            .all(|glyph| text.is_char_boundary(glyph.index))
    );
    assert!(glyphs.map(|glyph| glyph.id.0).any(|id| id != 0));
    assert!(cx.read(|app| app.global::<RoutineState>().0.operations.get()) > 0);
}

#[derive(Default)]
struct RenderObservations {
    notifications: Cell<usize>,
    renders: Cell<usize>,
    paints: Cell<usize>,
    painted_value: Cell<usize>,
    releases: Cell<usize>,
}

struct LabeledView {
    rows: usize,
    source: Entity<usize>,
    observed: Rc<RenderObservations>,
    _subscription: Subscription,
}

impl LabeledView {
    fn new(
        rows: usize,
        source: Entity<usize>,
        observed: Rc<RenderObservations>,
        cx: &mut Context<Self>,
    ) -> Self {
        let subscription = cx.observe(&source, {
            let observed = Rc::clone(&observed);
            move |_, _, cx| {
                observed.notifications.set(observed.notifications.get() + 1);
                cx.notify();
            }
        });
        Self {
            rows,
            source,
            observed,
            _subscription: subscription,
        }
    }
}

impl Drop for LabeledView {
    fn drop(&mut self) {
        self.observed.releases.set(self.observed.releases.get() + 1);
    }
}

impl Render for LabeledView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.observed.renders.set(self.observed.renders.get() + 1);
        let value = *self.source.read(cx);
        let observed = Rc::clone(&self.observed);
        div()
            .size_full()
            .bg(rgb(0x203040))
            .child(
                canvas(
                    |_, _, _| (),
                    move |bounds, (), window, _| {
                        observed.paints.set(observed.paints.get() + 1);
                        observed.painted_value.set(value);
                        window.paint_quad(fill(bounds, rgb(0x305080)));
                    },
                )
                .w(px(8.))
                .h(px(8.)),
            )
            .children((0..self.rows).map(|row| {
                div()
                    .h(px(20.))
                    .child(format!("Row {row}: revision {value}, café λ"))
            }))
    }
}

#[open_gpui::bench(
    inputs = [4_usize, 16],
    input_name = "rows",
    group = "maintained-renderer",
    sample_size = 10,
    fps = 120
)]
fn parameterized_render(rows: &usize, cx: &mut BenchAppContext) {
    begin_routine(cx);
    let source = cx.new(|_| 0_usize);
    let source_weak = source.downgrade();
    let observed = Rc::new(RenderObservations::default());
    let mut window = cx.add_empty_window();
    window.simulate_resize(size(px(640.), px(480.)));
    let view = window
        .replace_root_view(|_, cx| {
            LabeledView::new(*rows, source.clone(), Rc::clone(&observed), cx)
        })
        .unwrap();
    let weak = view.downgrade();
    cx.settle();
    let initial = window.update(|window, _| {
        window
            .presentation_facts()
            .frame_accepted_generation
            .expect("setup must accept the labeled view")
    });
    let iterations = Cell::new(0);
    cx.bench_renderer(view.clone(), |view, _, cx| {
        iterations.set(iterations.get() + 1);
        record_operation(cx);
        view.source.update(cx, |value, cx| {
            *value += 1;
            cx.notify();
        });
    });
    cx.settle();
    assert!(iterations.get() > 0);
    assert_eq!(observed.notifications.get(), iterations.get());
    assert!(observed.renders.get() >= iterations.get());
    assert!(observed.paints.get() >= iterations.get());
    assert_eq!(observed.painted_value.get(), iterations.get());
    window.update(|window, _| {
        let facts = window.presentation_facts();
        let accepted = facts.frame_accepted_generation.unwrap();
        assert!(accepted > initial);
        assert_eq!(facts.present_submitted_generation, Some(accepted));
        assert_eq!(facts.non_empty_presented_generation, Some(accepted));
        let attempt = facts.latest_present_attempt.unwrap();
        assert_eq!(attempt.generation, accepted);
        assert_eq!(attempt.outcome, PlatformWindowPresentOutcome::Submitted);
        assert!(attempt.contained_valid_primitives);
    });
    window.update(|window, app| window.remove_window(app));
    drop((window, view, source));
    cx.settle();
    assert!(weak.upgrade().is_none());
    assert!(source_weak.upgrade().is_none());
    assert_eq!(observed.releases.get(), 1);
    assert_eq!(
        cx.read(|app| app.global::<RoutineState>().0.operations.get()),
        iterations.get()
    );
}

struct SortedOutput {
    values: Vec<u64>,
    expected: Rc<Vec<u64>>,
    drops: Rc<Cell<usize>>,
}

impl Drop for SortedOutput {
    fn drop(&mut self) {
        assert!(!profiler::trace_enabled());
        assert_eq!(self.values, *self.expected);
        self.drops.set(self.drops.get() + 1);
    }
}

#[open_gpui::bench(
    inputs = [64_usize, 256],
    input_name = "items",
    group = "maintained-seeded-tasks",
    sample_size = 10,
    seeds(7, 23)
)]
fn seeded_tasks(count: &usize, mut rng: StdRng, cx: &mut BenchAppContext) {
    begin_routine(cx);
    let values = (0..*count).map(|_| rng.random::<u64>()).collect::<Vec<_>>();
    assert!(open_gpui::calculate_seeds(1, &[7, 23]).0.any(|seed| {
        let mut replay = StdRng::seed_from_u64(seed);
        values.iter().all(|value| *value == replay.random::<u64>())
    }));
    let mut expected = values.clone();
    expected.sort_unstable();
    let expected = Rc::new(expected);
    let setups = Cell::new(0);
    let outputs = Rc::new(Cell::new(0));
    cx.bench_batched_task(
        |_| {
            assert!(!profiler::trace_enabled());
            assert_eq!(outputs.get(), setups.get());
            setups.set(setups.get() + 1);
            values.clone()
        },
        |input, cx| {
            let mut values = std::mem::take(input);
            let work = cx.background_executor().spawn(async move {
                values.sort_unstable();
                values
            });
            let expected = Rc::clone(&expected);
            let drops = Rc::clone(&outputs);
            cx.update(|app| {
                app.spawn(async move |app| {
                    let values = work.await;
                    app.update(|app| record_operation(app));
                    SortedOutput {
                        values,
                        expected,
                        drops,
                    }
                })
            })
        },
    );
    cx.settle();
    assert!(setups.get() > 0);
    assert_eq!(outputs.get(), setups.get());
    assert_eq!(
        cx.read(|app| app.global::<RoutineState>().0.operations.get()),
        setups.get()
    );
    assert_eq!(Rc::strong_count(&expected), 1);
}

#[open_gpui::bench(seed = 11)]
fn seeded_task(mut rng: StdRng, cx: &mut BenchAppContext) {
    begin_routine(cx);
    let value = rng.random::<u64>();
    let completed = Rc::new(Cell::new(0));
    cx.bench_task(|cx| {
        let completed = Rc::clone(&completed);
        let work = cx
            .background_executor()
            .spawn(async move { value.wrapping_mul(17) });
        cx.update(|app| {
            app.spawn(async move |app| {
                let result = work.await;
                assert_eq!(result, value.wrapping_mul(17));
                completed.set(completed.get() + 1);
                app.update(|app| record_operation(app));
            })
        })
    });
    assert!(completed.get() > 0);
    assert_eq!(
        cx.read(|app| app.global::<RoutineState>().0.operations.get()),
        completed.get()
    );
    assert_eq!(Rc::strong_count(&completed), 1);
}

mod module_cases {
    use super::*;

    #[open_gpui::bench]
    pub(super) fn public_routine(cx: &mut BenchAppContext) {
        begin_routine(cx);
        cx.bench_iter(|cx| cx.update(|app| record_operation(app)));
        assert!(cx.read(|app| app.global::<RoutineState>().0.operations.get()) > 0);
    }
}

fn maintained_workflows(criterion: &mut Criterion<BenchMeasurement>) {
    simple_native_shaping(criterion);
    assert_previous_routine_released();
    parameterized_render(criterion);
    assert_previous_routine_released();
    seeded_tasks(criterion);
    assert_previous_routine_released();
    seeded_task(criterion);
    assert_previous_routine_released();
}

open_gpui::bench_group! {
    name = benches;
    config = Criterion::default()
        .warm_up_time(Duration::from_millis(50))
        .measurement_time(Duration::from_millis(100))
        .sample_size(10)
        .nresamples(1_000)
        .without_plots();
    targets = maintained_workflows, module_cases::public_routine
}
open_gpui::bench_main!(benches);
