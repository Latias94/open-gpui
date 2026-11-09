//! Frame cost of seeded element trees, including cached views and changing shares.
//!
//! The `mixed` groups are the general benchmark: each iteration applies a fixed, seeded
//! sequence of many kinds of change to a freshly built [`RandomizedElementTree`], one per
//! frame. The other tree groups each measure frames in which exactly one kind of thing
//! happens, such as nothing (a notified root that re-renders an unchanged tree), one
//! element's style, a leaf's bounds, the root's layout, one structural change, a sibling
//! reorder or a pointer move, to explain where a difference in `mixed` comes from. The
//! same fixture runs on any GPUI revision, so it doubles as the before/after harness for
//! changes to rendering.
//!
//! Trees come from *families*: bounds on topology, element count and entity density that
//! a seeded RNG is drawn against (see [`RandomizedElementTreeBounds`]). Each family is an
//! input and `#[open_gpui::bench]` runs it once per seed, so an id like `tree/wide/seed-3`
//! names one tree whose whole shape follows from the seed, and a run over a family's
//! seeds is a sample of that family. `SEED` and `ITERATIONS` choose the seeds as for
//! `#[open_gpui::test]`. Filter by family to ask a narrower question, e.g.
//! `--bench randomized_element_tree -- 'tree/tall/'`. Each tree is described once, with
//! the heap its window holds after the first frame, before it is measured.
//!
//! Benchmarks don't check their output; the randomized tree's tests check that what these
//! workloads render is correct, so a faster frame here still needs those tests to pass.
//! Text-panel, layout-churn and scrolling workloads are outside this target's scope.

use std::{
    cell::RefCell,
    fmt,
    time::{Duration, Instant},
};

use open_gpui::{
    BenchAppContext, Context, Entity, Window,
    private::rand::{Rng as _, SeedableRng as _, rngs::StdRng},
    randomized_element_tree::{
        ChangeLocality, RandomizedElementTree, RandomizedElementTreeBounds,
        RandomizedElementTreeConfig, RandomizedElementTreeMutation,
        RandomizedElementTreeMutationKind, RandomizedElementTreeTopology,
    },
};

/// A named family of trees: the bounds one seed's tree is drawn against.
#[derive(Clone)]
struct TreeFamily {
    name: &'static str,
    bounds: RandomizedElementTreeBounds,
    /// Changes per measured iteration of the mixed workload, sized so one iteration of
    /// this family's trees takes at most about half a second on an M-series Mac. Fixed per
    /// family rather than calibrated at run time, so two revisions always measure the
    /// same changes.
    mixed_changes: usize,
}

impl TreeFamily {
    /// Draws this family's tree for the seed `rng` started from. Every family consumes
    /// its RNG the same way, so drawing from `rng` directly would give each family the
    /// same relative draw at a given seed (overlapping families the same tree); mixing in
    /// the family's name gives each family its own stream for the same seed.
    fn sample(&self, rng: &mut StdRng) -> TreeInput {
        // FNV-1a, which unlike `DefaultHasher` is fixed across Rust releases, so a seed
        // names the same tree on every toolchain.
        let name_hash = self
            .name
            .bytes()
            .fold(0xcbf2_9ce4_8422_2325_u64, |hash, byte| {
                (hash ^ u64::from(byte)).wrapping_mul(0x0000_0100_0000_01b3)
            });
        let mut family_rng = StdRng::seed_from_u64(rng.random::<u64>() ^ name_hash);
        TreeInput {
            family: self.name,
            config: self.bounds.sample(&mut family_rng),
        }
    }
}

impl fmt::Display for TreeFamily {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.name)
    }
}

/// One tree drawn from a family. Its `Display` describes the draw, which the benchmark
/// id (family and seed) does not.
struct TreeInput {
    family: &'static str,
    config: RandomizedElementTreeConfig,
}

impl fmt::Display for TreeInput {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let topology = match self.config.topology() {
            RandomizedElementTreeTopology::Wide => "wide",
            RandomizedElementTreeTopology::Deep => "deep",
            RandomizedElementTreeTopology::Mixed => "mixed",
            RandomizedElementTreeTopology::Narrow => "narrow",
        };
        write!(
            formatter,
            "{} tree: {topology}, {} elements, {}% entities, {}% handlers",
            self.family,
            self.config.element_count(),
            (self.config.entity_density() * 100.0).round() as usize,
            (self.config.handler_density() * 100.0).round() as usize
        )
    }
}

/// Tall trees recurse once per element through layout and paint; without gpui's `stacker`
/// feature a chain of 384 draws and one of 512 overflows a 2 MB stack, so tall families
/// stop well short of that. Real UI is not hundreds of levels deep either.
const TALL_MAX_ELEMENTS: usize = 256;

/// Entities are how GPUI structures a UI, so every family has at least this share of its
/// elements backed by one; a tree with none is a single view, which no renderer can
/// partially reuse and which real UI never is.
const MIN_ENTITY_DENSITY: f64 = 0.05;

/// The families every benchmark samples. Each is a question: what does this class of
/// change cost on a tree like *this*?
fn families() -> Vec<TreeFamily> {
    let handlers = 0.1..=0.4;
    vec![
        TreeFamily {
            name: "any",
            mixed_changes: 192,
            bounds: RandomizedElementTreeBounds::new(32..=TALL_MAX_ELEMENTS)
                .with_entity_density(MIN_ENTITY_DENSITY..=0.5)
                .with_handler_density(handlers.clone()),
        },
        TreeFamily {
            name: "wide",
            mixed_changes: 16,
            bounds: RandomizedElementTreeBounds::new(128..=2048)
                .with_topologies([RandomizedElementTreeTopology::Wide])
                .with_entity_density(MIN_ENTITY_DENSITY..=0.25)
                .with_handler_density(handlers.clone()),
        },
        TreeFamily {
            name: "tall",
            mixed_changes: 512,
            bounds: RandomizedElementTreeBounds::new(32..=TALL_MAX_ELEMENTS)
                .with_topologies(
                    RandomizedElementTreeTopology::ALL
                        .into_iter()
                        .filter(|topology| topology.is_tall()),
                )
                .with_entity_density(MIN_ENTITY_DENSITY..=0.25)
                .with_handler_density(handlers.clone()),
        },
        TreeFamily {
            name: "mixed",
            mixed_changes: 192,
            bounds: RandomizedElementTreeBounds::new(128..=1024)
                .with_topologies([RandomizedElementTreeTopology::Mixed])
                .with_entity_density(MIN_ENTITY_DENSITY..=0.25)
                .with_handler_density(handlers.clone()),
        },
        TreeFamily {
            name: "dense-entities",
            mixed_changes: 64,
            bounds: RandomizedElementTreeBounds::new(128..=1024)
                .with_topologies([
                    RandomizedElementTreeTopology::Wide,
                    RandomizedElementTreeTopology::Mixed,
                ])
                .with_entity_density(0.75..=1.0)
                .with_handler_density(handlers),
        },
    ]
}

/// Builds the tree, draws it twice so layout, the scene and both frame buffers are warm,
/// and measures `mutate` once per frame.
fn measure(
    group: &str,
    input: &TreeInput,
    cx: &mut BenchAppContext,
    mut mutate: impl FnMut(&mut RandomizedElementTree, &mut Context<RandomizedElementTree>),
) {
    measure_in_window(group, input, cx, |tree, _, cx| mutate(tree, cx));
}

fn measure_in_window(
    group: &str,
    input: &TreeInput,
    cx: &mut BenchAppContext,
    mut mutate: impl FnMut(&mut RandomizedElementTree, &mut Window, &mut Context<RandomizedElementTree>),
) {
    measure_prepared(
        group,
        input,
        cx,
        |_, _| (),
        |_, tree, window, cx| mutate(tree, window, cx),
    );
}

/// Like [`measure_in_window`], with `prepare` reading the warm window once before timing
/// to build state that every measured frame uses.
fn measure_prepared<State>(
    group: &str,
    input: &TreeInput,
    cx: &mut BenchAppContext,
    prepare: impl FnOnce(&mut RandomizedElementTree, &Window) -> State,
    mut mutate: impl FnMut(
        &mut State,
        &mut RandomizedElementTree,
        &mut Window,
        &mut Context<RandomizedElementTree>,
    ),
) {
    let config = input.config;
    let before_window = open_gpui_bench_metrics::allocation_stats();
    let mut window = cx.add_empty_window();
    let tree = window.update(|window, cx| {
        window.replace_root(cx, |_, cx| {
            RandomizedElementTree::new_with_config(config, cx)
        })
    });
    cx.run_until_idle();
    // Warm both frame buffers and their scratch allocations. A short sample can
    // otherwise measure only the cheaper first buffer after mounting the tree.
    window.update(|window, _| window.refresh());
    cx.run_until_idle();
    describe_once(
        input,
        before_window,
        open_gpui_bench_metrics::allocation_stats(),
    );
    let mut state = window.update(|window, cx| tree.update(cx, |tree, _| prepare(tree, window)));

    let mut summary = SeedTotals::default();
    let mut previous_iteration: Option<(
        Instant,
        Option<open_gpui_bench_metrics::AllocationStats>,
    )> = None;
    cx.bench_renderer(tree, |tree, window, cx| {
        // From one iteration's start to the next is one whole iteration, the frame
        // included, as Criterion times it; the reads add well under a microsecond.
        let now = Instant::now();
        let stats = open_gpui_bench_metrics::allocation_stats();
        if let Some((started, started_stats)) = previous_iteration {
            summary.elapsed += now - started;
            summary.iterations += 1;
            if let Some((start, end)) = started_stats.zip(stats) {
                summary.allocations += end.allocations - start.allocations;
            }
        }
        previous_iteration = Some((now, stats));

        mutate(&mut state, tree, window, cx);
    });
    window.update(|window, _| report_scene_stats(group, input.family, window));
    record_for_summary(group, input, summary);
}

fn report_scene_stats(group: &str, input: &str, window: &Window) {
    if std::env::var("GPUI_BENCH_SCENE_STATS").as_deref() == Ok("1") {
        let scene = window.rendered_scene();
        eprintln!(
            "scene stats {group}/{input}: batches={}, paths={}, surfaces={}",
            scene.batches().count(),
            scene.paths.len(),
            scene.surfaces.len(),
        );
    }
}

thread_local! {
    static LAST_DESCRIBED: RefCell<Option<RandomizedElementTreeConfig>> =
        const { RefCell::new(None) };
}

/// Prints the tree and the heap its window holds after the first frame, once per run of
/// consecutive routine calls on the same tree: Criterion calls a benchmark's routine for
/// warm-up and every sample, and each call builds the same tree.
fn describe_once(
    input: &TreeInput,
    before_window: Option<open_gpui_bench_metrics::AllocationStats>,
    after_first_frame: Option<open_gpui_bench_metrics::AllocationStats>,
) {
    let is_new =
        LAST_DESCRIBED.with(|last| last.borrow_mut().replace(input.config)) != Some(input.config);
    if !is_new {
        return;
    }
    match before_window.zip(after_first_frame) {
        Some((before, after)) => eprintln!(
            "{input}; window holds {:.1} KB after the first frame",
            (after.live_bytes() - before.live_bytes()) as f64 / 1024.
        ),
        None => eprintln!("{input}"),
    }
}

/// One seed's measured iterations, summed over every routine call Criterion makes for it.
#[derive(Clone, Copy, Default)]
struct SeedTotals {
    elapsed: Duration,
    iterations: u64,
    allocations: u64,
}

/// Totals per group and family, each holding one entry per tree (seed), in run order.
type SummaryTotals = Vec<(
    String,
    &'static str,
    Vec<(RandomizedElementTreeConfig, SeedTotals)>,
)>;

thread_local! {
    static SUMMARY: RefCell<SummaryTotals> = const { RefCell::new(Vec::new()) };
}

fn record_for_summary(group: &str, input: &TreeInput, totals: SeedTotals) {
    SUMMARY.with(|summary| {
        let mut summary = summary.borrow_mut();
        let index = match summary.iter().position(|(existing_group, family, _)| {
            existing_group == group && *family == input.family
        }) {
            Some(index) => index,
            None => {
                summary.push((group.to_owned(), input.family, Vec::new()));
                summary.len() - 1
            }
        };
        let seeds = &mut summary[index].2;
        match seeds.iter_mut().find(|(config, _)| *config == input.config) {
            Some((_, seed)) => {
                seed.elapsed += totals.elapsed;
                seed.iterations += totals.iterations;
                seed.allocations += totals.allocations;
            }
            None => seeds.push((input.config, totals)),
        }
    });
}

fn geometric_mean(values: impl IntoIterator<Item = f64>) -> f64 {
    let (log_sum, count) = values
        .into_iter()
        .fold((0.0, 0_u32), |(sum, count), value| {
            (sum + value.max(f64::MIN_POSITIVE).ln(), count + 1)
        });
    (log_sum / f64::from(count.max(1))).exp()
}

/// Prints one line per group and family: the geometric mean across its seeds of the time
/// and allocations per iteration, so a family reads as one number while each seed stays
/// its own reproducible benchmark. Registered as the last target so it runs after every
/// group; prints nothing when a filter matched no randomized tree benchmark.
fn family_summary(_: &mut criterion::Criterion<open_gpui::BenchMeasurement>) {
    let summary = SUMMARY.with(|summary| summary.take());
    if summary.is_empty() {
        return;
    }
    eprintln!(
        "\nRandomizedTree family summary (geometric mean across seeds; includes Criterion warm-up)"
    );
    eprintln!(
        "  {:<34} {:<15} {:>5} {:>14} {:>14}",
        "group", "family", "seeds", "time/iter", "allocs/iter"
    );
    for (group, family, seeds) in summary {
        // A routine call measures from one iteration's start to the next, so a run with a
        // single iteration per call, such as `--test`, has nothing to summarize.
        if seeds.iter().all(|(_, totals)| totals.iterations == 0) {
            eprintln!(
                "  {group:<34} {family:<15} {:>5} {:>14}",
                seeds.len(),
                "unavailable"
            );
            continue;
        }
        let per_iteration = |value: fn(&SeedTotals) -> f64| {
            geometric_mean(
                seeds
                    .iter()
                    .filter(|(_, totals)| totals.iterations > 0)
                    .map(|(_, totals)| value(totals) / totals.iterations as f64),
            )
        };
        let time = per_iteration(|totals| totals.elapsed.as_secs_f64());
        let allocations = per_iteration(|totals| totals.allocations as f64);
        eprintln!(
            "  {group:<34} {family:<15} {:>5} {:>11.1} µs {allocations:>14.0}",
            seeds.len(),
            time * 1e6
        );
    }
}

/// A frame that rebuilds everything: `Window::refresh`, the fallback GPUI uses whenever
/// it cannot tell what changed (window activation, theme or settings changes via
/// `refresh_windows`, an inspector toggle). Nothing in the tree changes, so this is the
/// pure cost of the known-good path, and the renderer's worst case: every view and element
/// rendered again, none reused. Its difference between two GPUI revisions is the price of
/// a full refresh on the newer one.
#[open_gpui::bench(inputs = families(), input_name = "tree", iterations = 2, group = "RandomizedTree/full refresh", fps = 120)]
fn full_refresh(family: &TreeFamily, mut rng: StdRng, cx: &mut BenchAppContext) {
    let input = &family.sample(&mut rng);
    measure_in_window("full refresh", input, cx, |_, window, _| window.refresh());
}

/// A frame in which the root is notified but nothing in the tree changed. On a renderer
/// that rebuilds every frame this is the whole tree's cost; on one that retains output
/// it is the floor for a frame that has to do nothing.
#[open_gpui::bench(inputs = families(), input_name = "tree", iterations = 2, group = "RandomizedTree/unchanged", fps = 120)]
fn unchanged(family: &TreeFamily, mut rng: StdRng, cx: &mut BenchAppContext) {
    let input = &family.sample(&mut rng);
    measure("unchanged", input, cx, |_, cx| cx.notify());
}

/// Alternates the background of one element, chosen at random per seed, at any depth.
/// Recoloring the same element every frame keeps short and long samples measuring the
/// same work.
fn measure_element_style(group: &str, input: &TreeInput, cx: &mut BenchAppContext) {
    measure_prepared(
        group,
        input,
        cx,
        |tree, _| tree.random_element_id(),
        |element_id, tree, _, cx| {
            if let Some(element_id) = element_id {
                tree.alternate_element_color(*element_id, cx);
            }
        },
    );
}

/// One element's background color changes each frame: the smallest possible change,
/// confined to that element and, when it has one, its owning entity.
#[open_gpui::bench(inputs = families(), input_name = "tree", iterations = 2, group = "RandomizedTree/element style", fps = 120)]
fn element_style(family: &TreeFamily, mut rng: StdRng, cx: &mut BenchAppContext) {
    let input = &family.sample(&mut rng);
    measure_element_style("element style", input, cx);
}

/// A descendant's width and height change each frame, so its siblings and ancestors
/// re-lay out even where their own content did not change.
#[open_gpui::bench(inputs = families(), input_name = "tree", iterations = 2, group = "RandomizedTree/leaf bounds", fps = 120)]
fn leaf_bounds(family: &TreeFamily, mut rng: StdRng, cx: &mut BenchAppContext) {
    let input = &family.sample(&mut rng);
    measure("leaf bounds", input, cx, |tree, cx| {
        tree.apply_mutation(RandomizedElementTreeMutationKind::ChildBounds, cx);
    });
}

/// The root's width and height change each frame: every element's layout is stale.
#[open_gpui::bench(inputs = families(), input_name = "tree", iterations = 2, group = "RandomizedTree/root layout", fps = 120)]
fn root_layout(family: &TreeFamily, mut rng: StdRng, cx: &mut BenchAppContext) {
    let input = &family.sample(&mut rng);
    measure("root layout", input, cx, |tree, cx| {
        tree.apply_mutation(RandomizedElementTreeMutationKind::RootBounds, cx);
    });
}

/// One element is inserted or removed per frame, alternating so the tree keeps its size
/// to within one element over the whole loop. Removal takes a leaf, never a subtree.
#[open_gpui::bench(inputs = families(), input_name = "tree", iterations = 2, group = "RandomizedTree/insert-remove", fps = 120)]
fn insert_remove(family: &TreeFamily, mut rng: StdRng, cx: &mut BenchAppContext) {
    let input = &family.sample(&mut rng);
    let mut insert = true;
    measure("insert-remove", input, cx, |tree, cx| {
        let kind = if insert {
            RandomizedElementTreeMutationKind::Insert
        } else {
            RandomizedElementTreeMutationKind::RemoveLeaf
        };
        tree.apply_mutation(kind, cx);
        insert = !insert;
    });
}

/// A descendant moves among its siblings each frame; the set of elements is unchanged
/// but their order, and therefore every sibling's position, is not.
#[open_gpui::bench(inputs = families(), input_name = "tree", iterations = 2, group = "RandomizedTree/reorder", fps = 120)]
fn reorder(family: &TreeFamily, mut rng: StdRng, cx: &mut BenchAppContext) {
    let input = &family.sample(&mut rng);
    // A `Deep` tree has no parent with two children until an insert creates one, so the
    // tree inserts instead; every other shape reorders on every frame.
    measure("reorder", input, cx, |tree, cx| {
        tree.apply_mutation(RandomizedElementTreeMutationKind::Reorder, cx);
    });
}

/// Where the pointer goes in hover workloads: the center of each painted quad's visible
/// part, sorted by position. Hairline quads are skipped: their centers sit on an element's
/// edge, where moving the pointer rarely changes what's hovered.
fn hover_targets(window: &Window) -> Vec<open_gpui::Point<open_gpui::Pixels>> {
    let scale_factor = window.scale_factor();
    let mut targets: Vec<open_gpui::Point<open_gpui::Pixels>> = Vec::new();
    for quad in &window.rendered_scene().quads {
        let visible = quad.bounds.intersect(&quad.clip.conservative_bounds);
        let minimum = open_gpui::ScaledPixels(4. * scale_factor);
        if visible.size.width < minimum || visible.size.height < minimum {
            continue;
        }
        let center = visible.center();
        let target = open_gpui::point(
            open_gpui::px(center.x.0 / scale_factor),
            open_gpui::px(center.y.0 / scale_factor),
        );
        targets.push(target);
    }
    // Sorted by position so a change to scene order, which is renderer output, doesn't
    // change where the pointer goes.
    targets.sort_by(|a, b| {
        (a.y.as_f32().total_cmp(&b.y.as_f32())).then(a.x.as_f32().total_cmp(&b.x.as_f32()))
    });
    targets.dedup();
    targets
}

fn measure_hover(group: &str, input: &TreeInput, cx: &mut BenchAppContext) {
    let mut frame = 0;
    measure_prepared(
        group,
        input,
        cx,
        |_, window| hover_targets(window),
        |targets, _, window, cx| {
            if let Some(position) = targets.get(frame % targets.len().max(1)) {
                window.dispatch_event(
                    open_gpui::PlatformInput::MouseMove(open_gpui::MouseMoveEvent {
                        position: *position,
                        modifiers: open_gpui::Modifiers::default(),
                        pressed_button: None,
                    }),
                    cx,
                );
            }
            frame += 1;
        },
    );
}

/// The pointer moves to a different painted element each frame, as when someone moves the
/// mouse over a window: event dispatch, hit-testing, hover styles turning on and off, and
/// the redraw that follows. A move whose set of hovered elements doesn't change draws
/// nothing, as in a real window; how often that happens depends on the tree's shape. The
/// pointer stays inside the root, so its group-hover styles switch on in setup and stay on.
#[open_gpui::bench(inputs = families(), input_name = "tree", iterations = 2, group = "RandomizedTree/hover", fps = 120)]
fn hover(family: &TreeFamily, mut rng: StdRng, cx: &mut BenchAppContext) {
    let mut input = family.sample(&mut rng);
    input.config = input.config.with_hover_styles(true);
    measure_hover("hover", &input, cx);
}

/// A seeded stream of the changes a real window sees, one per frame: restyles, resizes,
/// structural edits, reorders, scattered recolors, pointer moves, scroll events, focus
/// moves, window resizes and bare notifies, on top of entities that animate every frame.
struct MixedWorkload {
    rng: StdRng,
    /// Draws pointer positions and scroll distances, separately from `rng`, so input
    /// details never shift which changes come next.
    input_rng: StdRng,
    initial_size: usize,
    size: usize,
    root_kinds: Vec<RandomizedElementTreeMutationKind>,
}

impl MixedWorkload {
    fn new(seed: u64, initial_size: usize, cached_views: bool) -> Self {
        use RandomizedElementTreeMutationKind::*;
        let mut root_kinds = vec![RootColor, RootOpacity, RootBorder, RootBounds, RootClipping];
        // Hiding an ancestor of `.cached()` views reuses stale output, and can panic, on
        // `main` today; the cached mix leaves it out until that's fixed.
        if !cached_views {
            root_kinds.push(RootVisibility);
        }
        Self {
            rng: StdRng::seed_from_u64(seed ^ 0x6d69_7865_6421),
            input_rng: StdRng::seed_from_u64(seed ^ 0x696e_7075_7473),
            initial_size,
            size: initial_size,
            root_kinds,
        }
    }

    /// A position over the tree's root, whose largest size is 980 × 760, or just past it.
    /// Drawn from the model's coordinates rather than the rendered scene, so a renderer
    /// change can't move the pointer.
    fn pointer_position(&mut self) -> open_gpui::Point<open_gpui::Pixels> {
        open_gpui::point(
            open_gpui::px(self.input_rng.random_range(0.0..1000.0)),
            open_gpui::px(self.input_rng.random_range(0.0..800.0)),
        )
    }

    fn apply(
        &mut self,
        tree: &mut RandomizedElementTree,
        kind: RandomizedElementTreeMutationKind,
        cx: &mut Context<RandomizedElementTree>,
    ) {
        match tree.apply_mutation(kind, cx) {
            RandomizedElementTreeMutation::Inserted { .. } => self.size += 1,
            RandomizedElementTreeMutation::Removed {
                removed_element_count,
                ..
            } => self.size = self.size.saturating_sub(removed_element_count),
            _ => {}
        }
    }

    /// Applies one change. Weights are percentages of changes. Returns a new window size
    /// when the change is a resize, which the caller applies outside the window update.
    fn change(
        &mut self,
        tree: &mut RandomizedElementTree,
        window: &mut Window,
        cx: &mut Context<RandomizedElementTree>,
    ) -> Option<open_gpui::Size<open_gpui::Pixels>> {
        use RandomizedElementTreeMutationKind::*;
        let roll = self.rng.random_range(0..100);
        match roll {
            0..30 => {
                let kind = [ChildColor, ChildOpacity, ChildBorder][self.rng.random_range(0..3)];
                self.apply(tree, kind, cx);
            }
            30..40 => self.apply(tree, ChildBounds, cx),
            40..45 => self.apply(tree, ChildClipping, cx),
            45..48 => self.apply(tree, ChildVisibility, cx),
            48..52 => {
                let kind = self.root_kinds[self.rng.random_range(0..self.root_kinds.len())];
                self.apply(tree, kind, cx);
            }
            // Inserts add one element and removals take a leaf or a whole subtree, so
            // grow back toward the initial size after a large removal.
            52..64 => {
                let kind = if self.size < self.initial_size {
                    Insert
                } else if self.rng.random_bool(0.5) {
                    Remove
                } else {
                    RemoveLeaf
                };
                self.apply(tree, kind, cx);
            }
            64..70 => self.apply(tree, Reorder, cx),
            70..74 => {
                let count = (self.size / 20).max(1);
                tree.recolor_children(count, ChangeLocality::Spread, cx);
            }
            74..84 => {
                let position = self.pointer_position();
                window.dispatch_event(
                    open_gpui::PlatformInput::MouseMove(open_gpui::MouseMoveEvent {
                        position,
                        modifiers: open_gpui::Modifiers::default(),
                        pressed_button: None,
                    }),
                    cx,
                );
            }
            84..92 => {
                let position = self.pointer_position();
                let distance = open_gpui::px(self.input_rng.random_range(-160.0..160.0));
                let delta = if self.input_rng.random_bool(0.8) {
                    open_gpui::point(open_gpui::px(0.), distance)
                } else {
                    open_gpui::point(distance, open_gpui::px(0.))
                };
                window.dispatch_event(
                    open_gpui::PlatformInput::ScrollWheel(open_gpui::ScrollWheelEvent {
                        position,
                        delta: open_gpui::ScrollDelta::Pixels(delta),
                        ..Default::default()
                    }),
                    cx,
                );
            }
            92..95 => window.focus_next(cx),
            // Sizes on both sides of the tree's root, which shrinks to fit a smaller window.
            95..98 => {
                return Some(open_gpui::size(
                    open_gpui::px(self.input_rng.random_range(480.0..1600.0)),
                    open_gpui::px(self.input_rng.random_range(360.0..1000.0)),
                ));
            }
            _ => cx.notify(),
        }
        None
    }
}

/// One mixed-workload iteration's window, tree and workload, built before timing and
/// removed after it.
struct MixedRun<'a, 'measurement> {
    window: open_gpui::BenchWindowContext<'a, 'measurement>,
    tree: Entity<RandomizedElementTree>,
    workload: MixedWorkload,
}

impl Drop for MixedRun<'_, '_> {
    fn drop(&mut self) {
        self.window.update(|window, cx| window.remove_window(cx));
    }
}

fn measure_mixed(
    family: &TreeFamily,
    input: &TreeInput,
    cached_views: bool,
    cx: &mut BenchAppContext,
) {
    let config = input
        .config
        .with_cached_views(cached_views)
        .with_hover_styles(true)
        .with_animation_density(0.15);
    // Every iteration starts from the same freshly built tree and RNG state, so every
    // iteration applies exactly the same changes, however many Criterion runs.
    cx.bench_batched_task(
        |cx| {
            let mut window = cx.add_empty_window();
            let tree = window.update(|window, cx| {
                window.replace_root(cx, |_, cx| {
                    RandomizedElementTree::new_with_config(config, cx)
                })
            });
            cx.run_until_idle();
            window.update(|window, _| window.refresh());
            cx.run_until_idle();
            let initial_size = tree.read_with(cx, |tree, _| tree.snapshot().descendant_count());
            MixedRun {
                window,
                tree,
                workload: MixedWorkload::new(config.seed(), initial_size, cached_views),
            }
        },
        |run, cx| {
            // A change that leaves the window clean draws nothing, as in a real window, so
            // this counts changes, not frames. Animated entities make most changes draw.
            for _ in 0..family.mixed_changes {
                let resize = run.window.update(|window, cx| {
                    run.tree
                        .update(cx, |tree, cx| run.workload.change(tree, window, cx))
                });
                if let Some(size) = resize {
                    run.window.simulate_resize(size);
                }
                cx.run_until_idle();
            }
            open_gpui::Task::ready(())
        },
    );
}

/// The general benchmark: many kinds of change, one per frame, in a seeded random order,
/// with some entities animating every frame. One iteration applies the family's
/// `mixed_changes` changes to a freshly built tree; the single-change groups explain where
/// a difference here comes from.
#[open_gpui::bench(inputs = families(), input_name = "tree", iterations = 2, group = "RandomizedTree/mixed", fps = 120)]
fn mixed(family: &TreeFamily, mut rng: StdRng, cx: &mut BenchAppContext) {
    let input = family.sample(&mut rng);
    measure_mixed(family, &input, false, cx);
}

/// The same tree with every entity boundary wrapped in a definite-size `.cached()` view,
/// as Zed does for panes and docks, so a frame can reuse clean entities' output.
fn cached_input(family: &TreeFamily, rng: &mut StdRng) -> TreeInput {
    let mut input = family.sample(rng);
    input.config = input.config.with_cached_views(true);
    input
}

#[open_gpui::bench(inputs = families(), input_name = "tree", iterations = 2, group = "CachedTree/full refresh", fps = 120)]
fn cached_full_refresh(family: &TreeFamily, mut rng: StdRng, cx: &mut BenchAppContext) {
    let input = cached_input(family, &mut rng);
    measure_in_window("cached full refresh", &input, cx, |_, window, _| {
        window.refresh()
    });
}

#[open_gpui::bench(inputs = families(), input_name = "tree", iterations = 2, group = "CachedTree/element style", fps = 120)]
fn cached_element_style(family: &TreeFamily, mut rng: StdRng, cx: &mut BenchAppContext) {
    let input = cached_input(family, &mut rng);
    measure_element_style("cached element style", &input, cx);
}

#[open_gpui::bench(inputs = families(), input_name = "tree", iterations = 2, group = "CachedTree/leaf bounds", fps = 120)]
fn cached_leaf_bounds(family: &TreeFamily, mut rng: StdRng, cx: &mut BenchAppContext) {
    let input = cached_input(family, &mut rng);
    measure("cached leaf bounds", &input, cx, |tree, cx| {
        tree.apply_mutation(RandomizedElementTreeMutationKind::ChildBounds, cx);
    });
}

#[open_gpui::bench(inputs = families(), input_name = "tree", iterations = 2, group = "CachedTree/insert-remove", fps = 120)]
fn cached_insert_remove(family: &TreeFamily, mut rng: StdRng, cx: &mut BenchAppContext) {
    let input = cached_input(family, &mut rng);
    let mut insert = true;
    measure("cached insert-remove", &input, cx, |tree, cx| {
        let kind = if insert {
            RandomizedElementTreeMutationKind::Insert
        } else {
            RandomizedElementTreeMutationKind::RemoveLeaf
        };
        tree.apply_mutation(kind, cx);
        insert = !insert;
    });
}

#[open_gpui::bench(inputs = families(), input_name = "tree", iterations = 2, group = "CachedTree/hover", fps = 120)]
fn cached_hover(family: &TreeFamily, mut rng: StdRng, cx: &mut BenchAppContext) {
    let mut input = cached_input(family, &mut rng);
    input.config = input.config.with_hover_styles(true);
    measure_hover("cached hover", &input, cx);
}

#[open_gpui::bench(inputs = families(), input_name = "tree", iterations = 2, group = "CachedTree/mixed", fps = 120)]
fn cached_mixed(family: &TreeFamily, mut rng: StdRng, cx: &mut BenchAppContext) {
    let input = family.sample(&mut rng);
    measure_mixed(family, &input, true, cx);
}

#[open_gpui::bench(inputs = families(), input_name = "tree", iterations = 2, group = "CachedTree/reorder", fps = 120)]
fn cached_reorder(family: &TreeFamily, mut rng: StdRng, cx: &mut BenchAppContext) {
    let input = cached_input(family, &mut rng);
    measure("cached reorder", &input, cx, |tree, cx| {
        tree.apply_mutation(RandomizedElementTreeMutationKind::Reorder, cx);
    });
}

/// A tree family, the share of its elements that change each frame, and where those
/// changes sit.
#[derive(Clone)]
struct ChangingShareInput {
    family: TreeFamily,
    share_percent: usize,
    locality: ChangeLocality,
}

impl ChangingShareInput {
    fn locality_name(&self) -> &'static str {
        match self.locality {
            ChangeLocality::Localized => "local",
            ChangeLocality::Spread => "spread",
        }
    }
}

impl fmt::Display for ChangingShareInput {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let locality = self.locality_name();
        write!(
            formatter,
            "{}-f{}-{locality}",
            self.family, self.share_percent
        )
    }
}

/// The share of elements recolored per frame, from one element to all of them. A renderer
/// that retains clean entity subtrees should fall toward the left of this axis; one that
/// redraws everything is flat across it, so the two curves' distance is what retention
/// buys at each share.
///
/// The main line keeps each frame's change inside one entity, as a user action does. Two
/// `spread` points scatter it uniformly instead: the pessimistic bound, where every
/// changed element may dirty a different subtree.
fn changing_share_inputs() -> Vec<ChangingShareInput> {
    families()
        .into_iter()
        .flat_map(|family| {
            [
                (0, ChangeLocality::Localized),
                (1, ChangeLocality::Localized),
                (5, ChangeLocality::Localized),
                (25, ChangeLocality::Localized),
                (100, ChangeLocality::Localized),
                (5, ChangeLocality::Spread),
                (25, ChangeLocality::Spread),
            ]
            .into_iter()
            .map(move |(share_percent, locality)| ChangingShareInput {
                family: family.clone(),
                share_percent,
                locality,
            })
        })
        .collect()
}

// One seed rather than two: every family is multiplied by seven shares.
#[open_gpui::bench(inputs = changing_share_inputs(), input_name = "tree", iterations = 1, group = "RandomizedTree/changing share", fps = 120)]
fn changing_share(input: &ChangingShareInput, mut rng: StdRng, cx: &mut BenchAppContext) {
    let tree = &input.family.sample(&mut rng);
    let element_count = tree.config.element_count();
    let per_frame = if input.share_percent == 0 {
        0
    } else {
        (input.share_percent * element_count).div_ceil(100).max(1)
    };
    let group = format!(
        "changing share f{}-{}",
        input.share_percent,
        input.locality_name()
    );
    measure(&group, tree, cx, |tree, cx| {
        if per_frame == 0 {
            cx.notify();
        } else {
            tree.recolor_children(per_frame, input.locality, cx);
        }
    });
}

open_gpui::bench_group!(
    name = benches;
    config = criterion::Criterion::default()
        .warm_up_time(Duration::from_millis(500))
        .measurement_time(Duration::from_secs(1))
        .sample_size(20);
    targets = full_refresh,
    cached_full_refresh,
    cached_element_style,
    cached_leaf_bounds,
    cached_insert_remove,
    cached_reorder,
    cached_hover,
    cached_mixed,
    unchanged,
    element_style,
    leaf_bounds,
    root_layout,
    insert_remove,
    reorder,
    hover,
    mixed,
    changing_share,
    family_summary
);
open_gpui::bench_main!(benches);
