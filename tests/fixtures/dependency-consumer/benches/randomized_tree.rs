#![cfg(feature = "bench-support")]

//! Executes public randomized-tree mutations through the maintained platform benchmark harness.
//! Snapshot, scene and release checks run outside measurement. Accepted scene observations do
//! not establish native presentation, GPU pixels or a performance budget.

use criterion::Criterion;
use open_gpui::{
    AtlasTextureId, AtlasTextureInstanceId, AtlasTextureKind, AtlasTile, BenchAppContext,
    BenchWindowContext, Bounds, Context, DevicePixels, Entity, Primitive, PrimitiveBatch,
    Scene, Task, TileId, VisualContext as _, WindowActivationTerminal, point, profiler, px,
    randomized_element_tree::{
        RandomizedElementTree, RandomizedElementTreeBounds, RandomizedElementTreeMutation,
        RandomizedElementTreeMutationKind, RandomizedElementTreeSnapshot,
        RandomizedElementTreeTopology,
    },
    size,
};
use rand::rngs::StdRng;
use std::{
    cell::Cell,
    collections::{BTreeSet, HashMap},
    fmt,
    hint::black_box,
    rc::Rc,
    time::Duration,
};

#[derive(Clone, Copy)]
struct MutationCase {
    kind: RandomizedElementTreeMutationKind,
    cached: bool,
}

impl fmt::Display for MutationCase {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mode = if self.cached { "cached" } else { "ordinary" };
        write!(formatter, "{mode}/{:?}", self.kind)
    }
}

fn mutation_cases() -> Vec<MutationCase> {
    [false, true]
        .into_iter()
        .flat_map(|cached| {
            RandomizedElementTreeMutationKind::ALL
                .into_iter()
                .map(move |kind| MutationCase { kind, cached })
        })
        .collect()
}

fn track_release(cx: &Context<RandomizedElementTree>, releases: &Rc<Cell<usize>>) {
    let releases = Rc::clone(releases);
    cx.on_release(move |_, _| releases.set(releases.get() + 1))
        .detach();
}

#[derive(Default)]
struct ResourceIds {
    textures: HashMap<AtlasTextureInstanceId, u32>,
    tiles: HashMap<(AtlasTextureInstanceId, u32), u32>,
}

impl ResourceIds {
    fn normalize_tile(&mut self, tile: &mut AtlasTile) {
        let texture = tile.texture_instance();
        let texture_id = u32::try_from(self.textures.len()).expect("texture count fits u32");
        let texture_id = *self.textures.entry(texture).or_insert(texture_id);
        let tile_id = u32::try_from(self.tiles.len()).expect("tile count fits u32");
        let tile_id = *self
            .tiles
            .entry((texture, tile.tile_id.0))
            .or_insert(tile_id);
        tile.texture_id.index = texture_id;
        tile.tile_id = TileId(tile_id);
        tile.bounds.origin = Default::default();
        tile.texture_generation = 0;
    }
}

fn assert_resource_identity_controls() {
    let first = AtlasTile {
        texture_id: AtlasTextureId {
            index: 4,
            kind: AtlasTextureKind::Monochrome,
        },
        tile_id: TileId(8),
        padding: 1,
        bounds: Bounds {
            origin: point(DevicePixels(16), DevicePixels(24)),
            size: size(DevicePixels(8), DevicePixels(8)),
        },
        texture_generation: 3,
        texture_generation_padding: 0,
    };
    let second = AtlasTile {
        tile_id: TileId(9),
        ..first
    };
    let renumbered = |mut tile: AtlasTile| {
        tile.texture_id.index += 10;
        tile.texture_generation += 1;
        tile.tile_id = TileId(tile.tile_id.0 + 20);
        tile.bounds.origin = point(DevicePixels(80), DevicePixels(96));
        tile
    };
    let normalize = |mut tiles: [AtlasTile; 3]| {
        let mut resources = ResourceIds::default();
        for tile in &mut tiles {
            resources.normalize_tile(tile);
        }
        tiles
    };
    assert_eq!(
        normalize([first, second, first]),
        normalize([renumbered(first), renumbered(second), renumbered(first)]),
        "legal allocation renumbering must preserve scene identity",
    );
    assert_ne!(
        normalize([first, second, first]),
        normalize([first, first, first]),
        "distinct glyph resources must not normalize into one aliased tile",
    );
    let other_texture = AtlasTile {
        texture_id: AtlasTextureId {
            index: 5,
            ..first.texture_id
        },
        ..first
    };
    assert_ne!(
        normalize([first, other_texture, first]),
        normalize([first, first, first])
    );
}

fn scene_semantics(scene: &Scene) -> Vec<String> {
    let mut resources = ResourceIds::default();
    let mut primitives = Vec::new();
    for batch in scene.batches() {
        match batch {
            PrimitiveBatch::Shadows(range) => {
                primitives.extend(scene.shadows[range].iter().copied().map(Primitive::from));
            }
            PrimitiveBatch::Quads(range) => {
                primitives.extend(scene.quads[range].iter().copied().map(Primitive::from));
            }
            PrimitiveBatch::Paths(range) => {
                primitives.extend(scene.paths[range].iter().cloned().map(Primitive::from));
            }
            PrimitiveBatch::Underlines(range) => {
                primitives.extend(scene.underlines[range].iter().copied().map(Primitive::from));
            }
            PrimitiveBatch::MonochromeSprites { range, .. } => {
                primitives.extend(
                    scene.monochrome_sprites[range]
                        .iter()
                        .copied()
                        .map(Primitive::from),
                );
            }
            PrimitiveBatch::SubpixelSprites { range, .. } => {
                primitives.extend(
                    scene.subpixel_sprites[range]
                        .iter()
                        .copied()
                        .map(Primitive::from),
                );
            }
            PrimitiveBatch::PolychromeSprites { range, .. } => {
                primitives.extend(
                    scene.polychrome_sprites[range]
                        .iter()
                        .copied()
                        .map(Primitive::from),
                );
            }
            PrimitiveBatch::Surfaces(_) => panic!("randomized trees do not paint native surfaces"),
        }
    }
    primitives
        .into_iter()
        .map(|primitive| {
            let clip = primitive.clip_envelope();
            let start = clip.first_clip as usize;
            let end = start + clip.clip_count as usize;
            let clips = scene
                .clip_shapes()
                .get(start..end)
                .expect("accepted primitives must retain their complete clip ancestry");
            let description = match primitive {
                Primitive::Shadow(mut value) => {
                    value.clip.first_clip = 0;
                    format!("shadow {value:?}")
                }
                Primitive::Quad(mut value) => {
                    value.clip.first_clip = 0;
                    format!("quad {value:?}")
                }
                Primitive::Path(value) => {
                    let mut paints = Vec::new();
                    let mut curves = Vec::new();
                    let mut tiles = Vec::new();
                    value.append_to(&mut paints, &mut curves, &mut tiles);
                    for paint in &mut paints {
                        paint.clip.first_clip = 0;
                    }
                    format!("path {paints:?} {curves:?} {tiles:?}")
                }
                Primitive::Underline(mut value) => {
                    value.clip.first_clip = 0;
                    format!("underline {value:?}")
                }
                Primitive::MonochromeSprite(mut value) => {
                    value.clip.first_clip = 0;
                    resources.normalize_tile(&mut value.tile);
                    format!("monochrome {value:?}")
                }
                Primitive::SubpixelSprite(mut value) => {
                    value.clip.first_clip = 0;
                    resources.normalize_tile(&mut value.tile);
                    format!("subpixel {value:?}")
                }
                Primitive::PolychromeSprite(mut value) => {
                    value.clip.first_clip = 0;
                    resources.normalize_tile(&mut value.tile);
                    format!("polychrome {value:?}")
                }
                Primitive::Surface(_) => panic!("randomized trees do not paint native surfaces"),
            };
            format!("{description} clips={clips:?}")
        })
        .collect()
}

struct TreeRun<'a, 'measurement> {
    window: BenchWindowContext<'a, 'measurement>,
    tree: Option<Entity<RandomizedElementTree>>,
    initial: RandomizedElementTreeSnapshot,
    initial_generation: u64,
    case: MutationCase,
    mutation: Option<RandomizedElementTreeMutation>,
    releases: Rc<Cell<usize>>,
    completed: Rc<Cell<usize>>,
}

impl TreeRun<'_, '_> {
    fn assert_matches_fresh(&mut self) {
        let tree = self
            .tree
            .as_ref()
            .expect("the persistent tree remains live");
        let snapshot = tree.read_with(self.window.app_context(), |tree, _| tree.snapshot());
        assert_eq!(snapshot.element_ids().len(), snapshot.descendant_count());
        assert_eq!(
            snapshot
                .element_ids()
                .into_iter()
                .collect::<BTreeSet<_>>()
                .len(),
            snapshot.descendant_count(),
        );
        let (generation, actual) = self.window.update(|window, _| {
            let generation = window
                .presentation_facts()
                .frame_accepted_generation
                .expect("the notified tree must produce an accepted frame");
            assert!(
                generation > self.initial_generation,
                "{} must accept a fresh frame after mutation; restored={}, facts={:?}",
                self.case,
                snapshot == self.initial,
                window.presentation_facts(),
            );
            (generation, scene_semantics(window.rendered_scene()))
        });
        self.initial_generation = generation;
        let reconstructed = RandomizedElementTree::from_snapshot(snapshot.clone());
        assert_eq!(reconstructed.snapshot(), snapshot);
        let cx = self.window.app_context();
        let mut reference_window = cx.add_empty_window();
        reference_window.simulate_resize(size(px(1200.), px(900.)));
        let plain_releases = Rc::new(Cell::new(0));
        let plain = reference_window
            .replace_root_view(|_, cx| {
                track_release(cx, &plain_releases);
                reconstructed
            })
            .expect("the plain snapshot reference must mount");
        let plain_weak = plain.downgrade();
        cx.settle();
        let (plain_generation, plain_scene) = reference_window.update(|window, _| {
            let generation = window
                .presentation_facts()
                .frame_accepted_generation
                .expect("the plain snapshot reference must draw");
            (generation, scene_semantics(window.rendered_scene()))
        });
        assert!(
            actual == plain_scene,
            "plain snapshot reference: {}, restored={}, actual primitives={}, expected primitives={}",
            self.case,
            snapshot == self.initial,
            actual.len(),
            plain_scene.len(),
        );
        let entity_releases = Rc::new(Cell::new(0));
        let reference = reference_window
            .replace_root_view(|_, cx| {
                track_release(cx, &entity_releases);
                RandomizedElementTree::from_snapshot_with_entities(snapshot.clone(), cx)
            })
            .expect("the entity-preserving snapshot reference must mount");
        let reference_weak = reference.downgrade();
        drop(plain);
        cx.settle();
        assert!(plain_weak.upgrade().is_none());
        assert_eq!(plain_releases.get(), 1);
        assert_eq!(reference.read_with(cx, |tree, _| tree.snapshot()), snapshot);
        let reference_scene = reference_window.update(|window, _| {
            assert!(
                window
                    .presentation_facts()
                    .frame_accepted_generation
                    .unwrap()
                    > plain_generation
            );
            scene_semantics(window.rendered_scene())
        });
        assert!(
            actual == reference_scene,
            "entity snapshot reference: {}, restored={}, actual primitives={}, expected primitives={}",
            self.case,
            snapshot == self.initial,
            actual.len(),
            reference_scene.len(),
        );
        reference_window.update(|window, app| window.remove_window(app));
        drop((reference_window, reference));
        cx.settle();
        assert!(reference_weak.upgrade().is_none());
        assert_eq!(entity_releases.get(), 1);
        let activation = self.window.update(|window, _| window.activate_window());
        self.window.app_context().settle();
        assert_eq!(
            activation.snapshot().target(),
            self.window.window_handle().window_id()
        );
        assert_eq!(
            activation.snapshot().status().terminal(),
            Some(WindowActivationTerminal::Activated)
        );
        assert!(self.window.update(|window, _| window.is_window_active()));
    }
}

impl Drop for TreeRun<'_, '_> {
    fn drop(&mut self) {
        use RandomizedElementTreeMutation as Applied;
        use RandomizedElementTreeMutationKind as Kind;

        assert!(!profiler::trace_enabled());
        let mutation = self.mutation.expect("the measured mutation must execute");
        let tree = self.tree.as_ref().expect("the measured tree remains live");
        let snapshot = tree.read_with(self.window.app_context(), |tree, _| tree.snapshot());
        let before = self.initial.descendant_count();
        match (self.case.kind, mutation) {
            (Kind::Insert, Applied::Inserted { element_id, .. }) => {
                assert_eq!(snapshot.descendant_count(), before + 1);
                assert!(!self.initial.element_ids().contains(&element_id));
                assert!(snapshot.element_ids().contains(&element_id));
            }
            (
                Kind::Remove | Kind::RemoveLeaf,
                Applied::Removed {
                    element_id,
                    removed_element_count,
                    ..
                },
            ) => {
                assert_eq!(removed_element_count, 1);
                assert_eq!(snapshot.descendant_count(), before - 1);
                assert!(!snapshot.element_ids().contains(&element_id));
            }
            (
                Kind::Reorder,
                Applied::Reordered {
                    old_index,
                    new_index,
                    ..
                },
            ) => {
                assert_ne!(old_index, new_index);
                assert_ne!(snapshot.element_ids(), self.initial.element_ids());
                assert_eq!(
                    snapshot.element_ids().into_iter().collect::<BTreeSet<_>>(),
                    self.initial.element_ids().into_iter().collect(),
                );
            }
            (Kind::RootColor, Applied::RootColor)
            | (Kind::RootOpacity, Applied::RootOpacity)
            | (Kind::RootBorder, Applied::RootBorder)
            | (Kind::RootBounds, Applied::RootBounds)
            | (Kind::RootClipping, Applied::RootClipping)
            | (Kind::RootVisibility, Applied::RootVisibility)
            | (Kind::ChildColor, Applied::ChildColor { .. })
            | (Kind::ChildOpacity, Applied::ChildOpacity { .. })
            | (Kind::ChildBorder, Applied::ChildBorder { .. })
            | (Kind::ChildBounds, Applied::ChildBounds { .. })
            | (Kind::ChildClipping, Applied::ChildClipping { .. })
            | (Kind::ChildVisibility, Applied::ChildVisibility { .. }) => {
                assert_eq!(snapshot.element_ids(), self.initial.element_ids());
            }
            _ => panic!("requested {} but applied {mutation:?}", self.case),
        }
        self.assert_matches_fresh();
        if self.case.kind == Kind::RootVisibility {
            self.tree
                .as_ref()
                .unwrap()
                .update(self.window.app_context(), |tree, cx| {
                    tree.apply_mutation(Kind::RootVisibility, cx);
                });
            self.window.app_context().settle();
            self.assert_matches_fresh();
        }
        let tree = self
            .tree
            .take()
            .expect("the persistent tree remains live until close");
        let weak = tree.downgrade();
        self.window.update(|window, app| window.remove_window(app));
        drop(tree);
        self.window.app_context().settle();
        assert!(weak.upgrade().is_none());
        assert_eq!(self.releases.get(), 1);
        assert!(
            self.window
                .app_context()
                .read(|app| app.windows().is_empty())
        );
        self.completed.set(self.completed.get() + 1);
    }
}

#[open_gpui::bench(
    inputs = mutation_cases(),
    input_name = "mutation",
    group = "randomized-public",
    sample_size = 10,
    seeds(7, 23)
)]
fn build_mutate_draw_close(case: &MutationCase, mut rng: StdRng, cx: &mut BenchAppContext) {
    assert_resource_identity_controls();
    let config = RandomizedElementTreeBounds::new(16..=24)
        .with_topologies([RandomizedElementTreeTopology::Wide])
        .with_entity_density(1.0..=1.0)
        .with_handler_density(1.0..=1.0)
        .sample(&mut rng)
        .with_cached_views(case.cached);
    let completed = Rc::new(Cell::new(0));
    let started = Cell::new(0);
    cx.bench_batched_task(
        |cx| {
            assert!(!profiler::trace_enabled());
            assert_eq!(started.get(), completed.get());
            assert!(cx.read(|app| app.windows().is_empty()));
            started.set(started.get() + 1);
            let mut window = cx.add_empty_window();
            window.simulate_resize(size(px(1200.), px(900.)));
            let releases = Rc::new(Cell::new(0));
            let tree = window
                .replace_root_view(|_, cx| {
                    track_release(cx, &releases);
                    RandomizedElementTree::new_with_config(config, cx)
                })
                .expect("the configured randomized tree must mount");
            cx.settle();
            window.update(|window, _| window.refresh());
            cx.settle();
            assert!(window.update(|window, _| window.is_window_active()));
            let initial = tree.read_with(cx, |tree, _| tree.snapshot());
            assert_eq!(initial.descendant_count(), config.element_count());
            assert_eq!(initial.max_depth(), 1);
            assert_eq!(initial.entity_count(), config.element_count());
            assert_eq!(initial.handler_count(), config.element_count() * 2);
            let initial_generation = window.update(|window, _| {
                assert!(!scene_semantics(window.rendered_scene()).is_empty());
                window
                    .presentation_facts()
                    .frame_accepted_generation
                    .unwrap()
            });
            TreeRun {
                window,
                tree: Some(tree),
                initial,
                initial_generation,
                case: *case,
                mutation: None,
                releases,
                completed: Rc::clone(&completed),
            }
        },
        |run, cx| {
            run.mutation = Some(run.tree.as_ref().unwrap().update(cx, |tree, cx| {
                black_box(tree.apply_mutation(black_box(run.case.kind), cx))
            }));
            Task::ready(())
        },
    );
    cx.settle();
    assert!(completed.get() > 0);
    assert_eq!(started.get(), completed.get());
    assert!(cx.read(|app| app.windows().is_empty()));
}

open_gpui::bench_group! {
    name = benches;
    config = Criterion::default()
        .warm_up_time(Duration::from_millis(50))
        .measurement_time(Duration::from_millis(100))
        .sample_size(10)
        .nresamples(1_000)
        .without_plots();
    targets = build_mutate_draw_close
}
open_gpui::bench_main!(benches);
