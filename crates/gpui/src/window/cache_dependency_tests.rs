use super::cached_paint_atlas_tests::RejectOnceAtlas;
use crate::{
    AnyView, AnyWindowHandle, App, AppContext as _, AtlasTextureKind, AtlasTile, Context, Corners,
    Entity, InteractiveElement as _, IntoElement, MonochromeSprite, ParentElement as _, Pixels,
    PrimitiveTransform, Render, StyleRefinement, Styled as _, SubtreeClip, SubtreeClipExt as _,
    SubtreePresentation, SubtreePresentationExt as _, TestAppContext, Window, WindowHandle,
    WindowOpenFailureStage, WindowOptions, canvas, deferred, div, fill, point, px, red, size,
    window_portal,
};
use std::{
    cell::Cell,
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
    sync::Arc,
};

struct DependencyModel(usize);

struct DependencyRoot {
    child: AnyView,
    renders: Rc<Cell<usize>>,
}

impl Render for DependencyRoot {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        self.renders.set(self.renders.get() + 1);
        self.child
            .clone()
            .cached(StyleRefinement::default().size(px(100.)))
    }
}

#[derive(Clone, Copy)]
enum InnerDraw {
    Commit,
    Panic,
    Close,
}

struct InnerDependencyCanvas {
    model: Entity<DependencyModel>,
    output: Rc<Cell<usize>>,
    renders: Rc<Cell<usize>>,
    draw: InnerDraw,
    tile: Option<AtlasTile>,
}

impl Render for InnerDependencyCanvas {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        self.renders.set(self.renders.get() + 1);
        let model = self.model.clone();
        let output = self.output.clone();
        let draw = self.draw;
        let tile = self.tile;
        canvas(
            |_, _, _| {},
            move |bounds, _, window, cx| {
                output.set(model.read(cx).0);
                match draw {
                    InnerDraw::Commit => window.paint_quad(fill(bounds, red())),
                    InnerDraw::Panic => panic!("injected nested canvas panic"),
                    InnerDraw::Close => window.remove_window(cx),
                }
                if let Some(tile) = tile {
                    let bounds = window.device_local_bounds(bounds);
                    let _ = window.insert_scene_primitive(MonochromeSprite {
                        order: 1,
                        pad: 0,
                        bounds,
                        clip: Default::default(),
                        color: red(),
                        tile,
                        transform: PrimitiveTransform::IDENTITY,
                    });
                }
            },
        )
        .size_full()
    }
}

#[derive(Default)]
struct NestedOpen {
    requested: Cell<bool>,
    attempted: Cell<Option<AnyWindowHandle>>,
    committed: Cell<Option<WindowHandle<DependencyRoot>>>,
    failure: Cell<Option<WindowOpenFailureStage>>,
    unwound: Cell<bool>,
}

struct OuterDependencyCanvas {
    before: Entity<DependencyModel>,
    after: Entity<DependencyModel>,
    inner: Entity<DependencyRoot>,
    open: Rc<NestedOpen>,
    output: Rc<Cell<[usize; 2]>>,
    renders: Rc<Cell<usize>>,
    tile: Option<AtlasTile>,
    inner_atlas: Option<Arc<RejectOnceAtlas>>,
}

impl Render for OuterDependencyCanvas {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        self.renders.set(self.renders.get() + 1);
        let before = self.before.clone();
        let after = self.after.clone();
        let inner = self.inner.clone();
        let open = self.open.clone();
        let output = self.output.clone();
        let tile = self.tile;
        let inner_atlas = self.inner_atlas.clone();
        canvas(
            |_, _, _| {},
            move |bounds, _, window, cx| {
                let before = before.read(cx).0;
                if open.requested.replace(false) {
                    let result = catch_unwind(AssertUnwindSafe(|| {
                        cx.open_window_detailed(
                            WindowOptions {
                                focus_on_appearing: false,
                                ..Default::default()
                            },
                            |window, _| {
                                if let Some(atlas) = inner_atlas {
                                    window.sprite_atlas = atlas;
                                }
                                open.attempted.set(Some(window.window_handle()));
                                inner
                            },
                        )
                    }));
                    match result {
                        Ok(Ok(window)) => open.committed.set(Some(window)),
                        Ok(Err(error)) => open.failure.set(Some(error.stage())),
                        Err(payload) => {
                            assert_eq!(
                                payload.downcast_ref::<&str>().copied(),
                                Some("injected nested canvas panic"),
                                "only the injected inner paint panic may be caught"
                            );
                            open.unwound.set(true);
                        }
                    }
                }
                output.set([before, after.read(cx).0]);
                window.paint_quad(fill(bounds, red()));
                if let Some(tile) = tile {
                    let bounds = window.device_local_bounds(bounds);
                    let _ = window.insert_scene_primitive(MonochromeSprite {
                        order: 1,
                        pad: 0,
                        bounds,
                        clip: Default::default(),
                        color: red(),
                        tile,
                        transform: PrimitiveTransform::IDENTITY,
                    });
                }
            },
        )
        .size_full()
    }
}

struct NestedDependencyWindows {
    models: [Entity<DependencyModel>; 3],
    outer: WindowHandle<DependencyRoot>,
    outer_canvas: Entity<OuterDependencyCanvas>,
    inner_canvas: Entity<InnerDependencyCanvas>,
    open: Rc<NestedOpen>,
    outer_output: Rc<Cell<[usize; 2]>>,
    inner_output: Rc<Cell<usize>>,
    outer_renders: Rc<Cell<usize>>,
    inner_renders: Rc<Cell<usize>>,
    outer_root_renders: Rc<Cell<usize>>,
    inner_root_renders: Rc<Cell<usize>>,
}

impl NestedDependencyWindows {
    fn new(
        cx: &mut TestAppContext,
        draw: InnerDraw,
        open_during_initial_draw: bool,
        atlas: Option<Arc<RejectOnceAtlas>>,
    ) -> Self {
        let models = [1, 2, 3].map(|value| cx.new(|_| DependencyModel(value)));
        let open = Rc::new(NestedOpen::default());
        open.requested.set(open_during_initial_draw);
        let outer_output = Rc::new(Cell::new([0, 0]));
        let inner_output = Rc::new(Cell::new(0));
        let outer_renders = Rc::new(Cell::new(0));
        let inner_renders = Rc::new(Cell::new(0));
        let outer_root_renders = Rc::new(Cell::new(0));
        let inner_root_renders = Rc::new(Cell::new(0));
        let inner_canvas = cx.new(|_| InnerDependencyCanvas {
            model: models[1].clone(),
            output: inner_output.clone(),
            renders: inner_renders.clone(),
            draw,
            tile: None,
        });
        let inner = cx.new(|_| DependencyRoot {
            child: inner_canvas.clone().into(),
            renders: inner_root_renders.clone(),
        });
        let outer_canvas = cx.new(|_| OuterDependencyCanvas {
            before: models[0].clone(),
            after: models[2].clone(),
            inner,
            open: open.clone(),
            output: outer_output.clone(),
            renders: outer_renders.clone(),
            tile: atlas
                .as_ref()
                .map(|_| RejectOnceAtlas::tile(AtlasTextureKind::Monochrome)),
            inner_atlas: None,
        });
        let outer = cx.add_window(|window, _| {
            if let Some(atlas) = atlas {
                window.sprite_atlas = atlas;
            }
            DependencyRoot {
                child: outer_canvas.clone().into(),
                renders: outer_root_renders.clone(),
            }
        });
        Self {
            models,
            outer,
            outer_canvas,
            inner_canvas,
            open,
            outer_output,
            inner_output,
            outer_renders,
            inner_renders,
            outer_root_renders,
            inner_root_renders,
        }
    }

    fn assert_dependencies(&self, cx: &App) {
        let model_ids = self.models.each_ref().map(Entity::entity_id);
        let outer_reads = cx
            .tracked_entities
            .get(&self.outer.window_id())
            .expect("the outer frame must publish its dependencies");
        assert_eq!(
            model_ids.map(|id| outer_reads.contains(&id)),
            [true, false, true],
            "the outer window must retain A and C without capturing inner model B"
        );
        if let Some(inner) = self.open.committed.get() {
            let inner_reads = cx
                .tracked_entities
                .get(&inner.window_id())
                .expect("the inner initial draw must publish its dependencies");
            assert_eq!(
                model_ids.map(|id| inner_reads.contains(&id)),
                [false, true, false],
                "the inner window must track B without inheriting outer reads"
            );
        } else if let Some(attempted) = self.open.attempted.get() {
            assert!(
                !cx.tracked_entities.contains_key(&attempted.window_id()),
                "a failed inner open must not publish window dependencies"
            );
        }
    }
}

#[open_gpui::test]
fn cached_canvas_nested_window_reads_remain_window_local_on_replay(cx: &mut TestAppContext) {
    let windows = NestedDependencyWindows::new(cx, InnerDraw::Commit, true, None);
    let inner = windows.open.committed.get().expect("open the inner window");
    assert_eq!(cx.windows().len(), 2);
    assert_eq!(windows.outer_output.get(), [1, 3]);
    assert_eq!(windows.inner_output.get(), 2);
    let outer_renders = windows.outer_renders.get();
    let inner_renders = windows.inner_renders.get();
    cx.read(|cx| windows.assert_dependencies(cx));

    for _ in 0..3 {
        for window in [windows.outer, inner] {
            window
                .update(cx, |_, _, cx| cx.notify())
                .expect("replay the cached canvas");
            cx.read(|cx| windows.assert_dependencies(cx));
        }
        assert_eq!(windows.outer_renders.get(), outer_renders);
        assert_eq!(windows.inner_renders.get(), inner_renders);
    }

    for (index, value, expected) in [(0, 11, [11, 3]), (2, 13, [11, 13])] {
        let outer_before = windows.outer_root_renders.get();
        let inner_before = windows.inner_root_renders.get();
        let cached_renders = windows.outer_renders.get();
        let cached_output = windows.outer_output.get();
        windows.models[index].update(cx, |model, cx| {
            model.0 = value;
            cx.notify();
        });
        assert!(windows.outer_root_renders.get() > outer_before);
        assert_eq!(windows.inner_root_renders.get(), inner_before);
        assert_eq!(windows.outer_renders.get(), cached_renders);
        assert_eq!(windows.outer_output.get(), cached_output);
        windows
            .outer
            .update(cx, |_, window, _| window.refresh())
            .expect("refresh the outer non-view model read");
        assert_eq!(windows.outer_output.get(), expected);
        assert!(windows.outer_renders.get() > cached_renders);
        assert_eq!(windows.inner_root_renders.get(), inner_before);
        cx.read(|cx| windows.assert_dependencies(cx));
    }

    let outer_before = windows.outer_root_renders.get();
    let inner_before = windows.inner_root_renders.get();
    let cached_renders = windows.inner_renders.get();
    windows.models[1].update(cx, |model, cx| {
        model.0 = 12;
        cx.notify();
    });
    assert_eq!(windows.outer_root_renders.get(), outer_before);
    assert!(windows.inner_root_renders.get() > inner_before);
    assert_eq!(windows.inner_renders.get(), cached_renders);
    assert_eq!(windows.inner_output.get(), 2);
    inner
        .update(cx, |_, window, _| window.refresh())
        .expect("refresh the inner non-view model read");
    assert_eq!(windows.inner_output.get(), 12);
    assert!(windows.inner_renders.get() > cached_renders);
    assert_eq!(windows.outer_root_renders.get(), outer_before);
    cx.read(|cx| windows.assert_dependencies(cx));
}

fn assert_outer_cache_survives_failed_inner_open(cx: &mut TestAppContext, draw: InnerDraw) {
    let windows = NestedDependencyWindows::new(cx, draw, true, None);
    assert!(windows.open.committed.get().is_none());
    match draw {
        InnerDraw::Panic => assert!(windows.open.unwound.get()),
        InnerDraw::Close => assert_eq!(
            windows.open.failure.get(),
            Some(WindowOpenFailureStage::ClosedDuringInitialDraw)
        ),
        InnerDraw::Commit => unreachable!("this fixture requires a failed inner draw"),
    }
    assert_eq!(cx.windows().len(), 1);
    assert_eq!(windows.inner_output.get(), 2, "B was read before failure");
    assert_eq!(
        windows.outer_output.get(),
        [1, 3],
        "C was read after failure"
    );
    let outer_renders = windows.outer_renders.get();
    cx.read(|cx| windows.assert_dependencies(cx));
    for _ in 0..3 {
        windows
            .outer
            .update(cx, |_, _, cx| cx.notify())
            .expect("reuse the outer cache after failed inner drawing");
        assert_eq!(windows.outer_renders.get(), outer_renders);
        cx.read(|cx| windows.assert_dependencies(cx));
    }
    let outer_before = windows.outer_root_renders.get();
    windows.models[1].update(cx, |_, cx| cx.notify());
    assert_eq!(
        windows.outer_root_renders.get(),
        outer_before,
        "a failed inner draw must not leak its B subscription into the outer cache"
    );
    for index in [0, 2] {
        let outer_before = windows.outer_root_renders.get();
        windows.models[index].update(cx, |_, cx| cx.notify());
        assert!(windows.outer_root_renders.get() > outer_before);
        assert_eq!(windows.outer_renders.get(), outer_renders);
        cx.read(|cx| windows.assert_dependencies(cx));
    }
    windows
        .outer
        .update(cx, |_, window, _| window.refresh())
        .expect("rebuild the outer canvas after failed inner drawing");
    assert!(windows.outer_renders.get() > outer_renders);
    assert_eq!(windows.outer_output.get(), [1, 3]);
    assert_eq!(cx.windows().len(), 1);
    cx.read(|cx| windows.assert_dependencies(cx));
}

#[open_gpui::test]
fn cached_canvas_restores_outer_reads_after_inner_draw_unwinds(cx: &mut TestAppContext) {
    assert_outer_cache_survives_failed_inner_open(cx, InnerDraw::Panic);
}

#[open_gpui::test]
fn cached_canvas_restores_outer_reads_after_inner_open_fails(cx: &mut TestAppContext) {
    assert_outer_cache_survives_failed_inner_open(cx, InnerDraw::Close);
}

#[open_gpui::test]
fn cached_canvas_nested_window_reads_survive_candidate_rejection(cx: &mut TestAppContext) {
    let atlas = Arc::new(RejectOnceAtlas::new());
    let windows = NestedDependencyWindows::new(cx, InnerDraw::Commit, false, Some(atlas.clone()));
    assert!(windows.open.attempted.get().is_none());
    assert_eq!(windows.outer_output.get(), [1, 3]);
    windows.open.requested.set(true);
    cx.update_window(windows.outer.into(), |_, window, cx| {
        let committed_revision = window.rendered_frame_revision();
        let committed_scene_len = window.rendered_frame.scene.len();
        assert!(committed_revision > 0);
        assert_eq!(window.rendered_frame.scene.monochrome_sprites.len(), 1);
        window.refresh();
        atlas.fail_next_lease();
        window.draw(cx).clear(cx);
        assert!(windows.open.committed.get().is_some());
        assert_eq!(windows.inner_output.get(), 2);
        assert_eq!(windows.outer_output.get(), [1, 3]);
        assert_eq!(window.rendered_frame_revision(), committed_revision);
        assert_eq!(window.rendered_frame.scene.len(), committed_scene_len);
        assert_eq!(window.rendered_frame.scene.monochrome_sprites.len(), 1);
        assert_eq!(
            window
                .last_atlas_frame_rejection
                .as_ref()
                .expect("the intended candidate must fail a real atlas lease")
                .generation,
            committed_revision + 1
        );
        assert!(window.refresh_pending_for_test());
        windows.assert_dependencies(cx);

        window.draw(cx).clear(cx);
        assert_eq!(window.rendered_frame_revision(), committed_revision + 1);
        windows.assert_dependencies(cx);
    })
    .expect("reject and recover the outer candidate");

    let inner = windows
        .open
        .committed
        .get()
        .expect("retain the inner window");
    let outer_renders = windows.outer_renders.get();
    let inner_renders = windows.inner_renders.get();
    for _ in 0..3 {
        for window in [windows.outer, inner] {
            window
                .update(cx, |_, _, cx| cx.notify())
                .expect("reuse both caches after candidate recovery");
            cx.read(|cx| windows.assert_dependencies(cx));
        }
        assert_eq!(windows.outer_renders.get(), outer_renders);
        assert_eq!(windows.inner_renders.get(), inner_renders);
    }
}

#[open_gpui::test]
fn cached_canvas_restores_outer_reads_after_inner_candidate_rejection(cx: &mut TestAppContext) {
    let atlas = Arc::new(RejectOnceAtlas::new());
    let windows = NestedDependencyWindows::new(cx, InnerDraw::Commit, false, None);
    windows.inner_canvas.update(cx, |canvas, _| {
        canvas.tile = Some(RejectOnceAtlas::tile(AtlasTextureKind::Monochrome));
    });
    windows.outer_canvas.update(cx, |canvas, _| {
        canvas.inner_atlas = Some(atlas.clone());
    });
    windows.open.requested.set(true);

    cx.update_window(windows.outer.into(), |_, window, cx| {
        let committed_revision = window.rendered_frame_revision();
        atlas.fail_next_lease();
        window.refresh();
        window.draw(cx).clear(cx);
        let inner = windows
            .open
            .committed
            .get()
            .expect("retain the retrying inner window");
        assert_eq!(window.rendered_frame_revision(), committed_revision + 1);
        assert_eq!(windows.outer_output.get(), [1, 3]);
        assert_eq!(windows.inner_output.get(), 2);
        assert!(windows.open.failure.get().is_none());
        assert!(!windows.open.unwound.get());

        cx.update_window(inner.into(), |_, window, cx| {
            assert_eq!(window.rendered_frame_revision(), 0);
            assert_eq!(window.rendered_frame.scene.len(), 0);
            assert_eq!(
                window
                    .last_atlas_frame_rejection
                    .as_ref()
                    .expect("the inner candidate must fail a real atlas lease")
                    .generation,
                1
            );
            assert!(window.refresh_pending_for_test());
            assert!(!cx.tracked_entities.contains_key(&inner.window_id()));
            let model_ids = windows.models.each_ref().map(Entity::entity_id);
            let outer_reads = cx
                .tracked_entities
                .get(&windows.outer.window_id())
                .expect("the outer frame must commit after the inner rejection");
            assert_eq!(
                model_ids.map(|id| outer_reads.contains(&id)),
                [true, false, true]
            );
            for (index, model_id) in model_ids.into_iter().enumerate() {
                let invalidators = cx.window_invalidators_by_entity.get(&model_id);
                assert_eq!(
                    invalidators.is_some_and(|windows_by_id| {
                        windows_by_id.contains_key(&windows.outer.window_id())
                    }),
                    index != 1
                );
                assert!(invalidators.is_none_or(|windows_by_id| {
                    !windows_by_id.contains_key(&inner.window_id())
                }));
            }

            window.draw(cx).clear(cx);
            assert_eq!(window.rendered_frame_revision(), 1);
            assert_eq!(window.rendered_frame.scene.monochrome_sprites.len(), 1);
            assert!(!window.refresh_pending_for_test());
            windows.assert_dependencies(cx);
        })
        .expect("inspect and recover the rejected inner candidate");
        assert_eq!(windows.outer_output.get(), [1, 3]);
        windows.assert_dependencies(cx);
    })
    .expect("commit outer reads around a rejected inner draw");

    let inner = windows
        .open
        .committed
        .get()
        .expect("retain the recovered inner window");
    let outer_renders = windows.outer_renders.get();
    let inner_renders = windows.inner_renders.get();
    for _ in 0..3 {
        for window in [windows.outer, inner] {
            window
                .update(cx, |_, _, cx| cx.notify())
                .expect("reuse both caches after inner candidate recovery");
            cx.read(|cx| windows.assert_dependencies(cx));
        }
        assert_eq!(windows.outer_renders.get(), outer_renders);
        assert_eq!(windows.inner_renders.get(), inner_renders);
    }
}

#[open_gpui::test]
fn rejected_candidate_keeps_committed_dependencies_until_retry(cx: &mut TestAppContext) {
    let atlas = Arc::new(RejectOnceAtlas::new());
    let windows = NestedDependencyWindows::new(cx, InnerDraw::Commit, false, Some(atlas.clone()));
    let committed = windows.models[0].clone();
    let candidate = cx.new(|_| DependencyModel(4));
    let window_id = windows.outer.window_id();
    let assert_dependency = |cx: &App, entity: &Entity<DependencyModel>, expected: bool| {
        let entity_id = entity.entity_id();
        assert_eq!(
            cx.tracked_entities
                .get(&window_id)
                .is_some_and(|entities| entities.contains(&entity_id)),
            expected,
            "wrong tracked dependency for {entity_id}"
        );
        assert_eq!(
            cx.window_invalidators_by_entity
                .get(&entity_id)
                .is_some_and(|windows_by_id| windows_by_id.contains_key(&window_id)),
            expected,
            "wrong window invalidator for {entity_id}"
        );
    };
    cx.read(|cx| {
        assert_dependency(cx, &committed, true);
        assert_dependency(cx, &candidate, false);
    });

    cx.update_window(windows.outer.into(), |_, window, cx| {
        let committed_revision = window.rendered_frame_revision();
        let committed_scene_len = window.rendered_frame.scene.len();
        let committed_dependencies = cx
            .tracked_entities
            .get(&window_id)
            .expect("the initial frame must publish D")
            .clone();
        windows.outer_canvas.update(cx, |canvas, _| {
            canvas.before = candidate.clone();
        });
        atlas.fail_next_lease();
        window.refresh();
        window.draw(cx).clear(cx);
        assert_eq!(
            windows.outer_output.get(),
            [4, 3],
            "the candidate must read E"
        );
        assert_eq!(window.rendered_frame_revision(), committed_revision);
        assert_eq!(window.rendered_frame.scene.len(), committed_scene_len);
        assert_eq!(window.rendered_frame.scene.monochrome_sprites.len(), 1);
        assert_eq!(
            window
                .last_atlas_frame_rejection
                .as_ref()
                .expect("the changed dependency candidate must fail a real atlas lease")
                .generation,
            committed_revision + 1
        );
        assert!(window.refresh_pending_for_test());
        assert_eq!(
            cx.tracked_entities.get(&window_id),
            Some(&committed_dependencies)
        );
        assert_dependency(cx, &committed, true);
        assert_dependency(cx, &candidate, false);

        window.draw(cx).clear(cx);
        assert_eq!(window.rendered_frame_revision(), committed_revision + 1);
        assert_eq!(window.rendered_frame.scene.monochrome_sprites.len(), 1);
        assert_eq!(windows.outer_output.get(), [4, 3]);
        assert!(!window.refresh_pending_for_test());
        assert_dependency(cx, &committed, false);
        assert_dependency(cx, &candidate, true);
    })
    .expect("keep D on rejection and publish E on accepted retry");

    let outer_renders = windows.outer_renders.get();
    for _ in 0..3 {
        windows
            .outer
            .update(cx, |_, _, cx| cx.notify())
            .expect("reuse the accepted E dependency");
        assert_eq!(windows.outer_renders.get(), outer_renders);
        cx.read(|cx| {
            assert_dependency(cx, &committed, false);
            assert_dependency(cx, &candidate, true);
        });
    }
    let root_renders = windows.outer_root_renders.get();
    committed.update(cx, |_, cx| cx.notify());
    assert_eq!(windows.outer_root_renders.get(), root_renders);
    candidate.update(cx, |model, cx| {
        model.0 = 5;
        cx.notify();
    });
    assert!(windows.outer_root_renders.get() > root_renders);
    assert_eq!(windows.outer_renders.get(), outer_renders);
    assert_eq!(windows.outer_output.get(), [4, 3]);
    windows
        .outer
        .update(cx, |_, window, _| window.refresh())
        .expect("refresh the accepted non-view dependency E");
    assert_eq!(windows.outer_output.get(), [5, 3]);
    cx.read(|cx| {
        assert_dependency(cx, &committed, false);
        assert_dependency(cx, &candidate, true);
    });
}

struct DeferredOpacityParent {
    window_space: bool,
    renders: Rc<Cell<usize>>,
    paints: Rc<Cell<usize>>,
}

impl Render for DeferredOpacityParent {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        self.renders.set(self.renders.get() + 1);
        let paints = self.paints.clone();
        let child = div().size(px(50.)).opacity(0.5).child(
            canvas(
                |_, _, _| {},
                move |bounds, _, window, _| {
                    paints.set(paints.get() + 1);
                    window.paint_quad(fill(bounds, red()));
                },
            )
            .size_full(),
        );
        if self.window_space {
            window_portal(child)
        } else {
            deferred(child)
        }
    }
}

struct DeferredOpacityRoot {
    opacity: f32,
    cached_parent: bool,
    parent: Entity<DeferredOpacityParent>,
}

impl Render for DeferredOpacityRoot {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let parent = AnyView::from(self.parent.clone());
        let parent = if self.cached_parent {
            parent
                .cached(StyleRefinement::default().size(px(100.)))
                .into_any_element()
        } else {
            parent.into_any_element()
        };
        div().size(px(100.)).opacity(self.opacity).child(parent)
    }
}

fn assert_deferred_scene_alpha(
    cx: &mut TestAppContext,
    window: WindowHandle<DeferredOpacityRoot>,
    alpha: f32,
) {
    cx.update_window(window.into(), |_, window, _| {
        let frame = &window.rendered_frame;
        assert_eq!(frame.scene.quads.len(), 1, "only the deferred child paints");
        assert_eq!(frame.scene.quads[0].background.solid.a, alpha);
        assert_eq!(frame.deferred_draws.len(), 1);
        assert!(frame.generation > 0, "inspect a committed draw");
        assert!(!window.refresh_pending_for_test());
    })
    .expect("inspect the actual deferred scene primitive");
}

fn assert_deferred_opacity_inheritance(
    cx: &mut TestAppContext,
    window_space: bool,
    cached_parent: bool,
) {
    let renders = Rc::new(Cell::new(0));
    let paints = Rc::new(Cell::new(0));
    let parent = cx.new(|_| DeferredOpacityParent {
        window_space,
        renders: renders.clone(),
        paints: paints.clone(),
    });
    let window = cx.add_window(|_, _| DeferredOpacityRoot {
        opacity: 0.5,
        cached_parent,
        parent,
    });
    assert_deferred_scene_alpha(cx, window, 0.25);
    let initial_renders = renders.get();
    let initial_paints = paints.get();
    assert!(initial_renders > 0);
    assert!(initial_paints > 0);
    for _ in 0..3 {
        let before = (renders.get(), paints.get());
        window
            .update(cx, |_, _, cx| cx.notify())
            .expect("redraw with unchanged inherited opacity");
        assert_deferred_scene_alpha(cx, window, 0.25);
        if cached_parent {
            assert_eq!(
                (renders.get(), paints.get()),
                before,
                "reuse the deferred draw"
            );
        } else {
            assert!(renders.get() > before.0);
            assert!(paints.get() > before.1);
        }
    }

    let before_change = (renders.get(), paints.get());
    window
        .update(cx, |root, _, cx| {
            root.opacity = 1.;
            cx.notify();
        })
        .expect("change the inherited parent opacity");
    assert_deferred_scene_alpha(cx, window, 0.5);
    assert!(renders.get() > before_change.0);
    assert!(paints.get() > before_change.1);
    let changed = (renders.get(), paints.get());
    for _ in 0..3 {
        window
            .update(cx, |_, _, cx| cx.notify())
            .expect("reuse the updated parent opacity");
        assert_deferred_scene_alpha(cx, window, 0.5);
        if cached_parent {
            assert_eq!((renders.get(), paints.get()), changed);
        }
    }
}

#[open_gpui::test]
fn ordinary_deferred_inherits_opacity_from_direct_parent(cx: &mut TestAppContext) {
    assert_deferred_opacity_inheritance(cx, false, false);
}

#[open_gpui::test]
fn ordinary_deferred_inherits_opacity_from_cached_parent(cx: &mut TestAppContext) {
    assert_deferred_opacity_inheritance(cx, false, true);
}

#[open_gpui::test]
fn window_portal_inherits_opacity_from_direct_parent(cx: &mut TestAppContext) {
    assert_deferred_opacity_inheritance(cx, true, false);
}

#[open_gpui::test]
fn window_portal_inherits_opacity_from_cached_parent(cx: &mut TestAppContext) {
    assert_deferred_opacity_inheritance(cx, true, true);
}

struct DeferredHoverOpacityChild {
    renders: Rc<Cell<usize>>,
}

impl Render for DeferredHoverOpacityChild {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        self.renders.set(self.renders.get() + 1);
        div().size_full().bg(red())
    }
}

struct DeferredHoverOpacityParent {
    window_space: bool,
    child: Entity<DeferredHoverOpacityChild>,
    renders: Rc<Cell<usize>>,
}

impl Render for DeferredHoverOpacityParent {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        self.renders.set(self.renders.get() + 1);
        let child = div().size(px(100.)).opacity(0.5).child(
            AnyView::from(self.child.clone()).cached(StyleRefinement::default().size(px(100.))),
        );
        if self.window_space {
            window_portal(child)
        } else {
            deferred(child)
        }
    }
}

struct DeferredHoverOpacityRoot {
    width: Pixels,
    cached_parent: bool,
    parent: Entity<DeferredHoverOpacityParent>,
}

impl Render for DeferredHoverOpacityRoot {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let parent = AnyView::from(self.parent.clone());
        let parent = if self.cached_parent {
            parent
                .cached(StyleRefinement::default().size(px(100.)))
                .into_any_element()
        } else {
            parent.into_any_element()
        };
        div()
            .id("deferred-hover-opacity-parent")
            .w(self.width)
            .h(px(200.))
            .bg(crate::blue())
            .opacity(0.5)
            .hover(|style| style.opacity(1.))
            .child(parent)
    }
}

fn update_deferred_hover_opacity(
    cx: &mut TestAppContext,
    window: WindowHandle<DeferredHoverOpacityRoot>,
    expected_ancestor_alpha: f32,
    phase: &str,
    update: impl FnOnce(
        &mut DeferredHoverOpacityRoot,
        &mut Window,
        &mut Context<DeferredHoverOpacityRoot>,
    ),
) {
    let start_revision = cx
        .update_window(window.into(), |_, window, _| {
            window.rendered_frame_revision()
        })
        .expect("read the accepted deferred hover frame");
    window
        .update(cx, |root, window, cx| {
            update(root, window, cx);
            cx.notify();
        })
        .expect("update the deferred hover ancestor");

    for _ in 0..3 {
        let (alphas, captured_opacity, requested_frames) = cx
            .update_window(window.into(), |_, window, cx| {
                let frame = &window.rendered_frame;
                let frames = frame.generation - start_revision;
                assert!(
                    (1..=3).contains(&frames),
                    "{phase}: took {frames} accepted frames"
                );
                assert_eq!(
                    window.presentation_facts().frame_accepted_generation,
                    Some(frame.generation),
                    "{phase}: inspect the accepted candidate"
                );
                assert_eq!(
                    frame.scene.quads.len(),
                    2,
                    "{phase}: ancestor and child quads"
                );
                assert_eq!(frame.deferred_draws.len(), 1, "{phase}");
                let alphas = [
                    frame.scene.quads[0].background.solid.a,
                    frame.scene.quads[1].background.solid.a,
                ];
                let captured_opacity = frame.deferred_draws[0].element_opacity;
                let requested_frames = window.drain_next_frame_callbacks_for_test(cx);
                if requested_frames == 0 {
                    assert!(
                        !window.refresh_pending_for_test(),
                        "{phase}: convergence must leave no dirty follow-up"
                    );
                }
                (alphas, captured_opacity, requested_frames)
            })
            .expect("inspect deferred hover alpha and advance self-requested callbacks");
        if requested_frames == 0 {
            assert_eq!(
                alphas[0], expected_ancestor_alpha,
                "{phase}: the ancestor must observe the current hit test"
            );
            assert_eq!(
                alphas[1],
                expected_ancestor_alpha * 0.5,
                "{phase}: deferred alpha must converge without an external redraw; captured opacity {captured_opacity}"
            );
            return;
        }
    }
    panic!("{phase}: deferred opacity kept requesting frames");
}

fn assert_deferred_hover_opacity_converges(
    cx: &mut TestAppContext,
    window_space: bool,
    cached_parent: bool,
) {
    let child_renders = Rc::new(Cell::new(0));
    let parent_renders = Rc::new(Cell::new(0));
    let child = cx.new(|_| DeferredHoverOpacityChild {
        renders: child_renders.clone(),
    });
    let parent = cx.new(|_| DeferredHoverOpacityParent {
        window_space,
        child,
        renders: parent_renders.clone(),
    });
    let window = cx.add_window(|_, _| DeferredHoverOpacityRoot {
        width: px(200.),
        cached_parent,
        parent,
    });
    let case = format!("window portal {window_space}, cached parent {cached_parent}");
    for (position, alpha, phase) in [
        (point(px(500.), px(500.)), 0.5, "outside"),
        (point(px(50.), px(50.)), 1., "mouse enter"),
        (point(px(250.), px(50.)), 0.5, "mouse leave"),
    ] {
        update_deferred_hover_opacity(
            cx,
            window,
            alpha,
            &format!("{case}: {phase}"),
            |_, window, cx| window.simulate_mouse_move(position, cx),
        );
        let settled = (parent_renders.get(), child_renders.get());
        for replay in 1..=2 {
            update_deferred_hover_opacity(
                cx,
                window,
                alpha,
                &format!("{case}: {phase} unchanged replay {replay}"),
                |_, _, _| {},
            );
            assert_eq!(
                child_renders.get(),
                settled.1,
                "{case}: reuse the cached child"
            );
            if cached_parent {
                assert_eq!(
                    parent_renders.get(),
                    settled.0,
                    "{case}: reuse the cached parent"
                );
            } else {
                assert!(
                    parent_renders.get() > settled.0,
                    "{case}: rebuild the direct parent"
                );
            }
        }
    }

    update_deferred_hover_opacity(
        cx,
        window,
        1.,
        &format!("{case}: expand ancestor under the stationary pointer"),
        |root, _, _| root.width = px(300.),
    );
    let settled = (parent_renders.get(), child_renders.get());
    for replay in 1..=2 {
        update_deferred_hover_opacity(
            cx,
            window,
            1.,
            &format!("{case}: stationary hover unchanged replay {replay}"),
            |_, _, _| {},
        );
        assert_eq!(
            child_renders.get(),
            settled.1,
            "{case}: reuse the converged child"
        );
        if cached_parent {
            assert_eq!(
                parent_renders.get(),
                settled.0,
                "{case}: reuse the converged parent"
            );
        }
    }

    update_deferred_hover_opacity(
        cx,
        window,
        0.5,
        &format!("{case}: shrink ancestor away from the stationary pointer"),
        |root, _, _| root.width = px(200.),
    );
    let before_fresh_parent = parent_renders.get();
    update_deferred_hover_opacity(
        cx,
        window,
        1.,
        &format!("{case}: expand ancestor and rebuild the parent in the same candidate"),
        |root, _, cx| {
            root.width = px(300.);
            root.parent.update(cx, |_, cx| cx.notify());
        },
    );
    assert!(
        parent_renders.get() > before_fresh_parent,
        "{case}: rebuild the notified parent"
    );
    let settled = (parent_renders.get(), child_renders.get());
    for replay in 1..=2 {
        update_deferred_hover_opacity(
            cx,
            window,
            1.,
            &format!("{case}: fresh-parent hover unchanged replay {replay}"),
            |_, _, _| {},
        );
        assert_eq!(
            child_renders.get(),
            settled.1,
            "{case}: reuse the rebuilt child"
        );
        if cached_parent {
            assert_eq!(
                parent_renders.get(),
                settled.0,
                "{case}: reuse the rebuilt parent"
            );
        } else {
            assert!(
                parent_renders.get() > settled.0,
                "{case}: render the direct parent"
            );
        }
    }
}

#[open_gpui::test]
fn ordinary_deferred_hover_opacity_converges_from_direct_parent(cx: &mut TestAppContext) {
    assert_deferred_hover_opacity_converges(cx, false, false);
}

#[open_gpui::test]
fn ordinary_deferred_hover_opacity_converges_from_cached_parent(cx: &mut TestAppContext) {
    assert_deferred_hover_opacity_converges(cx, false, true);
}

#[open_gpui::test]
fn window_portal_hover_opacity_converges_from_direct_parent(cx: &mut TestAppContext) {
    assert_deferred_hover_opacity_converges(cx, true, false);
}

#[open_gpui::test]
fn window_portal_hover_opacity_converges_from_cached_parent(cx: &mut TestAppContext) {
    assert_deferred_hover_opacity_converges(cx, true, true);
}

struct GroupOpacityParent {
    window_space: bool,
    child: Entity<DeferredHoverOpacityChild>,
    renders: Rc<Cell<usize>>,
}

impl Render for GroupOpacityParent {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        self.renders.set(self.renders.get() + 1);
        let child = div().size(px(100.)).opacity(0.5).child(
            AnyView::from(self.child.clone()).cached(StyleRefinement::default().size(px(100.))),
        );
        let child = if self.window_space {
            window_portal(child)
        } else {
            deferred(child)
        };
        div()
            .id("group-opacity-parent")
            .size(px(100.))
            .opacity(0.5)
            .group_hover("cache-opacity-group", |style| style.opacity(1.))
            .child(child)
    }
}

struct GroupOpacityRoot {
    left: Pixels,
    width: Pixels,
    cached_parent: bool,
    parent: Entity<GroupOpacityParent>,
}

impl Render for GroupOpacityRoot {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let parent = AnyView::from(self.parent.clone());
        let parent = if self.cached_parent {
            parent
                .cached(StyleRefinement::default().size(px(100.)))
                .into_any_element()
        } else {
            parent.into_any_element()
        };
        div()
            .id("cache-opacity-group-owner")
            .group("cache-opacity-group")
            .absolute()
            .left(self.left)
            .w(self.width)
            .h(px(200.))
            .bg(crate::blue())
            .child(parent)
    }
}

fn update_group_opacity(
    cx: &mut TestAppContext,
    window: WindowHandle<GroupOpacityRoot>,
    expected_alpha: f32,
    update: impl FnOnce(&mut GroupOpacityRoot, &mut Window, &mut Context<GroupOpacityRoot>),
) -> crate::HitboxId {
    let start_revision = cx
        .update_window(window.into(), |_, window, _| {
            window.rendered_frame_revision()
        })
        .expect("read the accepted group-opacity frame");
    window
        .update(cx, |root, window, cx| {
            update(root, window, cx);
            cx.notify();
        })
        .expect("update the external group owner");
    for _ in 0..3 {
        let (alpha, group_id, callbacks) = cx
            .update_window(window.into(), |_, window, cx| {
                let frame = &window.rendered_frame;
                assert!((1..=3).contains(&(frame.generation - start_revision)));
                assert_eq!(
                    window.presentation_facts().frame_accepted_generation,
                    Some(frame.generation)
                );
                assert_eq!(frame.scene.quads.len(), 2);
                assert_eq!(frame.scene.quads[0].background.solid.a, 1.);
                assert_eq!(frame.deferred_draws.len(), 1);
                let alpha = frame.scene.quads[1].background.solid.a;
                let group_id = frame.hitboxes[0].id;
                let callbacks = window.drain_next_frame_callbacks_for_test(cx);
                if callbacks == 0 {
                    assert!(!window.refresh_pending_for_test());
                }
                (alpha, group_id, callbacks)
            })
            .expect("advance only requested group-opacity follow-up frames");
        if callbacks == 0 {
            assert_eq!(alpha, expected_alpha, "accepted group-opacity scene");
            return group_id;
        }
    }
    panic!("group opacity did not converge within three accepted frames");
}

#[open_gpui::test]
fn group_opacity_outside_cached_child_converges_after_stationary_expansion(
    cx: &mut TestAppContext,
) {
    for window_space in [false, true] {
        let child_renders = Rc::new(Cell::new(0));
        let parent_renders = Rc::new(Cell::new(0));
        let child = cx.new(|_| DeferredHoverOpacityChild {
            renders: child_renders.clone(),
        });
        let parent = cx.new(|_| GroupOpacityParent {
            window_space,
            child,
            renders: parent_renders.clone(),
        });
        let window = cx.add_window(|_, _| GroupOpacityRoot {
            left: px(0.),
            width: px(200.),
            cached_parent: false,
            parent,
        });
        update_group_opacity(cx, window, 0.25, |_, window, cx| {
            window.simulate_mouse_move(point(px(250.), px(50.)), cx)
        });
        update_group_opacity(cx, window, 0.5, |root, _, _| root.width = px(300.));
        let settled_child_renders = child_renders.get();
        let mut group_id = update_group_opacity(cx, window, 0.5, |_, _, _| {});
        for _ in 0..2 {
            let previous_group_id = group_id;
            group_id = update_group_opacity(cx, window, 0.5, |_, _, _| {});
            assert_ne!(group_id, previous_group_id);
            assert_eq!(child_renders.get(), settled_child_renders);
        }
    }
}

#[open_gpui::test]
fn cached_group_follower_keeps_prediction_until_owning_view_is_notified(cx: &mut TestAppContext) {
    for window_space in [false, true] {
        let child_renders = Rc::new(Cell::new(0));
        let parent_renders = Rc::new(Cell::new(0));
        let child = cx.new(|_| DeferredHoverOpacityChild {
            renders: child_renders.clone(),
        });
        let parent = cx.new(|_| GroupOpacityParent {
            window_space,
            child,
            renders: parent_renders.clone(),
        });
        let window = cx.add_window(|_, _| GroupOpacityRoot {
            left: px(0.),
            width: px(300.),
            cached_parent: true,
            parent,
        });
        let mut group_id = update_group_opacity(cx, window, 0.5, |root, window, cx| {
            window.simulate_mouse_move(point(px(250.), px(50.)), cx);
            root.parent.update(cx, |_, cx| cx.notify());
        });
        let settled = (parent_renders.get(), child_renders.get());
        for _ in 0..3 {
            let previous_group_id = group_id;
            group_id = update_group_opacity(cx, window, 0.5, |_, _, _| {});
            assert_ne!(group_id, previous_group_id);
            assert_eq!((parent_renders.get(), child_renders.get()), settled);
        }

        update_group_opacity(cx, window, 0.5, |root, _, _| root.width = px(200.));
        assert_eq!((parent_renders.get(), child_renders.get()), settled);
        update_group_opacity(cx, window, 0.25, |root, _, cx| {
            root.parent.update(cx, |_, cx| cx.notify());
        });
        assert!(parent_renders.get() > settled.0);
        assert!(child_renders.get() > settled.1);
        let settled = (parent_renders.get(), child_renders.get());
        for _ in 0..2 {
            update_group_opacity(cx, window, 0.25, |_, _, _| {});
            assert_eq!((parent_renders.get(), child_renders.get()), settled);
        }
    }
}

struct RefusedHoverOpacityRoot {
    width: Pixels,
    presentation: SubtreePresentation,
    parent: Entity<DeferredHoverOpacityParent>,
    tile: AtlasTile,
}

impl Render for RefusedHoverOpacityRoot {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let tile = self.tile;
        div()
            .id("refused-hover-opacity-parent")
            .w(self.width)
            .h(px(200.))
            .opacity(0.5)
            .hover(|style| style.opacity(1.))
            .child(self.parent.clone())
            .child(
                canvas(
                    |_, _, _| {},
                    move |bounds, _, window, _| {
                        let bounds = window.device_local_bounds(bounds);
                        let _ = window.insert_scene_primitive(MonochromeSprite {
                            order: 1,
                            pad: 0,
                            bounds,
                            clip: Default::default(),
                            color: red(),
                            tile,
                            transform: PrimitiveTransform::IDENTITY,
                        });
                    },
                )
                .size(px(10.)),
            )
            .with_subtree_presentation(self.presentation)
    }
}

#[open_gpui::test]
fn atlas_refusal_preserves_hover_prediction_and_queues_no_followup(cx: &mut TestAppContext) {
    let atlas = Arc::new(RejectOnceAtlas::new());
    let child = cx.new(|_| DeferredHoverOpacityChild {
        renders: Rc::new(Cell::new(0)),
    });
    let parent = cx.new(|_| DeferredHoverOpacityParent {
        window_space: false,
        child,
        renders: Rc::new(Cell::new(0)),
    });
    let window = cx.add_window(|window, _| {
        window.sprite_atlas = atlas.clone();
        RefusedHoverOpacityRoot {
            width: px(200.),
            presentation: SubtreePresentation::Visible,
            parent,
            tile: RejectOnceAtlas::tile(AtlasTextureKind::Monochrome),
        }
    });
    window
        .update(cx, |_, window, cx| {
            window.simulate_mouse_move(point(px(250.), px(50.)), cx);
            cx.notify();
        })
        .expect("establish an unhovered committed prediction");
    let mut baseline_settled = false;
    for _ in 0..3 {
        let callbacks = cx
            .update_window(window.into(), |_, window, cx| {
                window.drain_next_frame_callbacks_for_test(cx)
            })
            .expect("finish only self-requested baseline frames");
        if callbacks == 0 {
            baseline_settled = true;
            break;
        }
    }
    assert!(baseline_settled, "the unhovered baseline must converge");
    cx.update_window(window.into(), |_, window, cx| {
        assert_eq!(window.drain_next_frame_callbacks_for_test(cx), 0);
        assert_eq!(window.rendered_frame.scene.quads.len(), 1);
        assert_eq!(
            window.rendered_frame.scene.quads[0].background.solid.a,
            0.25
        );
        assert_eq!(window.rendered_frame.deferred_draws.len(), 1);
        assert_eq!(window.rendered_frame.deferred_draws[0].element_opacity, 0.5);
        assert!(!window.refresh_pending_for_test());
    })
    .expect("the baseline must be settled before refusing a candidate");

    window
        .update(cx, |root, _, _| root.width = px(300.))
        .expect("prepare the candidate without notifying the root");
    cx.update_window(window.into(), |_, window, cx| {
        let prediction = {
            let mut predictions =
                window
                    .rendered_frame
                    .element_states
                    .values()
                    .filter_map(|state| {
                        state
                            .inner
                            .downcast_ref::<Option<crate::InteractiveElementState>>()
                            .and_then(Option::as_ref)
                            .and_then(|state| state.hover_state.clone())
                    });
            let prediction = predictions.next().expect("the ancestor hover prediction");
            assert!(predictions.next().is_none());
            prediction
        };
        assert!(!prediction.borrow().element);
        let committed_prediction = *prediction.borrow();
        let committed_revision = window.rendered_frame_revision();
        atlas.fail_next_lease();
        window.refresh();
        window.draw(cx).clear(cx);
        assert_eq!(window.rendered_frame_revision(), committed_revision);
        assert_eq!(
            window
                .last_atlas_frame_rejection
                .as_ref()
                .expect("reject the actual atlas lease")
                .generation,
            committed_revision + 1
        );
        assert!(!prediction.borrow().element);
        assert!(*prediction.borrow() == committed_prediction);
        assert_eq!(window.drain_next_frame_callbacks_for_test(cx), 0);
        assert_eq!(
            window.rendered_frame.scene.quads[0].background.solid.a,
            0.25
        );
        assert_eq!(window.rendered_frame.deferred_draws[0].element_opacity, 0.5);

        window.draw(cx).clear(cx);
        assert_eq!(window.rendered_frame_revision(), committed_revision + 1);
        assert!(!prediction.borrow().element);
        assert!(*prediction.borrow() == committed_prediction);
        assert_eq!(window.rendered_frame.deferred_draws[0].element_opacity, 0.5);
        assert_eq!(
            window.rendered_frame.scene.quads[0].background.solid.a,
            0.25
        );
        assert_eq!(window.drain_next_frame_callbacks_for_test(cx), 1);
    })
    .expect("refuse without publishing prediction, then admit the retry");
    cx.update_window(window.into(), |_, window, cx| {
        assert_eq!(window.rendered_frame.scene.quads[0].background.solid.a, 0.5);
        assert_eq!(window.rendered_frame.deferred_draws[0].element_opacity, 1.);
        assert_eq!(window.drain_next_frame_callbacks_for_test(cx), 0);
        assert!(!window.refresh_pending_for_test());
    })
    .expect("the accepted retry must converge through its own follow-up");

    cx.update_window(window.into(), |root, window, cx| {
        let root = root
            .downcast::<RefusedHoverOpacityRoot>()
            .ok()
            .expect("the atlas fixture's source view");
        let committed_revision = window.rendered_frame_revision();
        assert_eq!(window.rendered_frame.scene.quads.len(), 1);
        assert_eq!(window.rendered_frame.scene.quads[0].background.solid.a, 0.5);
        assert_eq!(window.rendered_frame.deferred_draws[0].element_opacity, 1.);
        root.update(cx, |root, _| root.presentation = SubtreePresentation::Inert);
        atlas.fail_next_lease();
        window.refresh();
        window.draw(cx).clear(cx);
        assert_eq!(window.rendered_frame_revision(), committed_revision);
        assert_eq!(
            window
                .last_atlas_frame_rejection
                .as_ref()
                .expect("the inert candidate must fail a real atlas lease")
                .generation,
            committed_revision + 1
        );
        assert_eq!(window.rendered_frame.scene.quads[0].background.solid.a, 0.5);
        assert_eq!(window.rendered_frame.deferred_draws[0].element_opacity, 1.);
        assert_eq!(window.drain_next_frame_callbacks_for_test(cx), 0);

        root.update(cx, |root, _| {
            root.presentation = SubtreePresentation::Visible
        });
        window.draw(cx).clear(cx);
        assert_eq!(window.rendered_frame_revision(), committed_revision + 1);
        assert_eq!(
            window.presentation_facts().frame_accepted_generation,
            Some(committed_revision + 1)
        );
        assert_eq!(window.rendered_frame.deferred_draws[0].element_opacity, 1.);
        assert_eq!(window.rendered_frame.scene.quads[0].background.solid.a, 0.5);
        assert_eq!(window.drain_next_frame_callbacks_for_test(cx), 0);
        assert!(!window.refresh_pending_for_test());
    })
    .expect("an inert refusal must preserve the accepted forecast on the first visible retry");
}

struct WidthOnlyHoverRoot {
    left: Pixels,
}

impl Render for WidthOnlyHoverRoot {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().size_full().child(
            div()
                .id("width-only-hover")
                .debug_selector(|| "width-only-hover".into())
                .absolute()
                .left(self.left)
                .top(px(0.))
                .w(px(200.))
                .h(px(100.))
                .bg(red())
                .hover(|style| style.w(px(100.))),
        )
    }
}

#[open_gpui::test]
fn width_only_hover_does_not_start_a_stationary_layout_loop(cx: &mut TestAppContext) {
    let window = cx.add_window(|_, _| WidthOnlyHoverRoot { left: px(300.) });
    window
        .update(cx, |_, window, cx| {
            window.simulate_mouse_move(point(px(150.), px(50.)), cx);
            cx.notify();
        })
        .expect("leave the width-only hover target outside the pointer");
    cx.update_window(window.into(), |_, window, cx| {
        let frame = &window.rendered_frame;
        let bounds = frame.debug_bounds["width-only-hover"];
        assert_eq!(bounds.origin, point(px(300.), px(0.)));
        assert_eq!(bounds.size.width, px(200.));
        assert!(!frame.hitboxes[0].id.is_hovered(window));
        let input = frame
            .element_states
            .values()
            .find_map(|state| {
                state
                    .inner
                    .downcast_ref::<Option<crate::InteractiveElementState>>()
                    .and_then(Option::as_ref)
                    .and_then(|state| state.hover_state.as_ref())
            })
            .expect("the width-only target must keep its input hover state");
        assert!(!input.borrow().element);
        assert_eq!(window.drain_next_frame_callbacks_for_test(cx), 0);
    })
    .expect("the non-opacity baseline must not request a follow-up");

    window
        .update(cx, |root, _, cx| {
            root.left = px(0.);
            cx.notify();
        })
        .expect("move the width-only target under the stationary pointer");
    for _ in 0..3 {
        let revision = cx
            .update_window(window.into(), |_, window, cx| {
                assert!(window.rendered_frame.hitboxes[0].id.is_hovered(window));
                assert_eq!(
                    window.rendered_frame.debug_bounds["width-only-hover"]
                        .size
                        .width,
                    px(200.)
                );
                assert_eq!(window.drain_next_frame_callbacks_for_test(cx), 0);
                assert!(!window.refresh_pending_for_test());
                window.rendered_frame_revision()
            })
            .expect("width-only hover must not self-request alternating layouts");
        window
            .update(cx, |_, _, cx| cx.notify())
            .expect("an external redraw must remain a single accepted frame");
        cx.update_window(window.into(), |_, window, _| {
            assert_eq!(window.rendered_frame_revision(), revision + 1);
        })
        .expect("no hidden non-opacity redraw loop");
    }
}

struct MixedWidthOpacityHoverRoot {
    left: Pixels,
    parent: Entity<DeferredHoverOpacityParent>,
}

impl Render for MixedWidthOpacityHoverRoot {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().size_full().child(
            div()
                .id("mixed-width-opacity-hover")
                .debug_selector(|| "mixed-width-opacity-hover".into())
                .absolute()
                .left(self.left)
                .top(px(0.))
                .w(px(200.))
                .h(px(200.))
                .bg(crate::blue())
                .opacity(0.5)
                .hover(|style| style.w(px(100.)).opacity(1.))
                .child(self.parent.clone()),
        )
    }
}

#[open_gpui::test]
fn mixed_width_opacity_hover_converges_without_changing_latched_layout(cx: &mut TestAppContext) {
    for window_space in [false, true] {
        let child_renders = Rc::new(Cell::new(0));
        let child = cx.new(|_| DeferredHoverOpacityChild {
            renders: child_renders.clone(),
        });
        let parent = cx.new(|_| DeferredHoverOpacityParent {
            window_space,
            child,
            renders: Rc::new(Cell::new(0)),
        });
        let window = cx.add_window(|_, _| MixedWidthOpacityHoverRoot {
            left: px(300.),
            parent,
        });
        window
            .update(cx, |_, window, cx| {
                window.simulate_mouse_move(point(px(150.), px(50.)), cx);
                cx.notify();
            })
            .expect("establish the mixed-hover input state outside the target");
        cx.update_window(window.into(), |_, window, cx| {
            let frame = &window.rendered_frame;
            let bounds = frame.debug_bounds["mixed-width-opacity-hover"];
            assert_eq!(bounds.origin, point(px(300.), px(0.)));
            assert_eq!(bounds.size.width, px(200.));
            assert!(!frame.hitboxes[0].id.is_hovered(window));
            let input = frame
                .element_states
                .values()
                .find_map(|state| {
                    state
                        .inner
                        .downcast_ref::<Option<crate::InteractiveElementState>>()
                        .and_then(Option::as_ref)
                        .and_then(|state| state.hover_state.as_ref())
                })
                .expect("the mixed-hover target must keep its input hover state");
            assert!(!input.borrow().element);
            assert_eq!(window.drain_next_frame_callbacks_for_test(cx), 0);
        })
        .expect("prove the mixed-hover target's initial placement and input latch");

        let mut settled_child_renders = None;
        for (left, ancestor_alpha, unchanged) in [
            (px(300.), 0.5, false),
            (px(0.), 1., false),
            (px(0.), 1., true),
            (px(0.), 1., true),
        ] {
            let start_revision = cx
                .update_window(window.into(), |_, window, _| {
                    window.rendered_frame_revision()
                })
                .expect("read the accepted mixed-hover frame");
            window
                .update(cx, |root, _, cx| {
                    root.left = left;
                    cx.notify();
                })
                .expect("update the mixed-hover target without moving the pointer");

            let mut settled = false;
            for _ in 0..3 {
                let (alpha, callbacks) = cx
                    .update_window(window.into(), |_, window, cx| {
                        let frame = &window.rendered_frame;
                        assert!((1..=3).contains(&(frame.generation - start_revision)));
                        assert_eq!(
                            window.presentation_facts().frame_accepted_generation,
                            Some(frame.generation)
                        );
                        assert_eq!(
                            frame.debug_bounds["mixed-width-opacity-hover"].size.width,
                            px(200.)
                        );
                        assert_eq!(frame.hitboxes[0].id.is_hovered(window), left == px(0.));
                        assert_eq!(frame.scene.quads.len(), 2);
                        assert_eq!(frame.scene.quads[0].background.solid.a, ancestor_alpha);
                        assert_eq!(frame.deferred_draws.len(), 1);
                        let alpha = frame.scene.quads[1].background.solid.a;
                        let callbacks = window.drain_next_frame_callbacks_for_test(cx);
                        assert!(callbacks <= 1);
                        if callbacks == 0 {
                            assert!(!window.refresh_pending_for_test());
                        }
                        (alpha, callbacks)
                    })
                    .expect("advance only requested mixed-hover follow-up frames");
                if callbacks == 0 {
                    assert_eq!(alpha, ancestor_alpha * 0.5);
                    settled = true;
                    break;
                }
            }
            assert!(
                settled,
                "mixed hover must converge within three accepted frames"
            );
            if unchanged {
                assert_eq!(Some(child_renders.get()), settled_child_renders);
            }
            settled_child_renders = Some(child_renders.get());
        }
    }
}

struct RoundedClipChild {
    renders: Rc<Cell<usize>>,
}

impl Render for RoundedClipChild {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        self.renders.set(self.renders.get() + 1);
        div().size_full().bg(red())
    }
}

struct RoundedClipRoot {
    radius: Pixels,
    child: Entity<RoundedClipChild>,
}

impl Render for RoundedClipRoot {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let radius = size(self.radius, self.radius);
        let clip = SubtreeClip::try_own_rounded_border_box(Corners {
            top_left: radius,
            top_right: radius,
            bottom_right: radius,
            bottom_left: radius,
        })
        .expect("valid rounded clip");
        AnyView::from(self.child.clone())
            .cached(StyleRefinement::default().size(px(100.)))
            .with_subtree_clip(clip)
    }
}

#[open_gpui::test]
fn cached_view_rebuilds_when_rounded_clip_changes_with_same_aabb(cx: &mut TestAppContext) {
    let renders = Rc::new(Cell::new(0));
    let child = cx.new(|_| RoundedClipChild {
        renders: renders.clone(),
    });
    let window = cx.add_window(|_, _| RoundedClipRoot {
        radius: px(4.),
        child,
    });
    let initial_renders = renders.get();
    let (initial_bounds, initial_shapes) = cx
        .update_window(window.into(), |_, window, _| {
            let scene = &window.rendered_frame.scene;
            assert_eq!(scene.quads.len(), 1);
            (
                scene.quads[0].clip.conservative_bounds,
                scene.clip_shapes().to_vec(),
            )
        })
        .expect("read the initial rounded clip scene");
    for _ in 0..2 {
        window
            .update(cx, |_, _, cx| cx.notify())
            .expect("reuse the unchanged rounded clip");
        assert_eq!(renders.get(), initial_renders);
    }

    window
        .update(cx, |root, _, cx| {
            root.radius = px(24.);
            cx.notify();
        })
        .expect("change clip radii without changing its AABB");
    assert_eq!(renders.get(), initial_renders + 1);
    let changed_shapes = cx
        .update_window(window.into(), |_, window, _| {
            let scene = &window.rendered_frame.scene;
            assert_eq!(scene.quads.len(), 1);
            assert_eq!(scene.quads[0].clip.conservative_bounds, initial_bounds);
            assert_ne!(scene.clip_shapes(), initial_shapes.as_slice());
            scene.clip_shapes().to_vec()
        })
        .expect("verify the actual primitive uses the new rounded clip");
    for _ in 0..3 {
        window
            .update(cx, |_, _, cx| cx.notify())
            .expect("reuse the changed rounded clip");
        assert_eq!(renders.get(), initial_renders + 1);
        cx.update_window(window.into(), |_, window, _| {
            let scene = &window.rendered_frame.scene;
            assert_eq!(scene.quads.len(), 1);
            assert_eq!(scene.quads[0].clip.conservative_bounds, initial_bounds);
            assert_eq!(scene.clip_shapes(), changed_shapes.as_slice());
        })
        .expect("verify cached scene clip replay");
    }
}
