use super::cached_paint_atlas_tests::RejectOnceAtlas;
use crate::{
    AnyView, App, AppContext as _, Asset, AtlasTextureKind, Context, Entity, Hsla, IntoElement,
    MonochromeSprite, PrimitiveTransform, Render, StyleRefinement, Styled as _, TestAppContext,
    Window, blue, canvas, fill, px, red,
};
use futures::channel::oneshot;
use parking_lot::Mutex;
use std::{
    cell::{Cell, RefCell},
    future::Future,
    hash::{Hash, Hasher},
    rc::Rc,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};

#[derive(Clone)]
struct AssetSource {
    receiver: Arc<Mutex<Option<oneshot::Receiver<Arc<usize>>>>>,
    loads: Arc<AtomicUsize>,
}

impl Hash for AssetSource {
    fn hash<H: Hasher>(&self, state: &mut H) {
        Arc::as_ptr(&self.receiver).hash(state);
    }
}

struct PendingAsset;

impl Asset for PendingAsset {
    type Source = AssetSource;
    type Output = Arc<usize>;

    fn load(
        source: Self::Source,
        _: &mut App,
    ) -> impl Future<Output = Self::Output> + Send + 'static {
        source.loads.fetch_add(1, Ordering::SeqCst);
        let receiver = source.receiver.lock().take().expect("load only once");
        async move { receiver.await.expect("complete the asset") }
    }
}

struct CachedAssetView {
    source: AssetSource,
    dependencies: [Entity<Hsla>; 2],
    output: Rc<RefCell<Option<Arc<usize>>>>,
    renders: Rc<Cell<usize>>,
}

impl Render for CachedAssetView {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        self.renders.set(self.renders.get() + 1);
        let source = self.source.clone();
        let dependencies = self.dependencies.clone();
        let output = self.output.clone();
        canvas(
            move |_, window, cx| {
                let result = window.use_asset::<PendingAsset>(&source, cx);
                assert_eq!(window.use_asset::<PendingAsset>(&source, cx), result);
                let color = *dependencies[usize::from(result.is_some())].read(cx);
                *output.borrow_mut() = result;
                color
            },
            move |bounds, color, window, _| {
                window.paint_quad(fill(bounds, color));
                let _ = window.insert_scene_primitive(MonochromeSprite {
                    order: 1,
                    animation: Default::default(),
                    transformation: Default::default(),
                    bounds: window.device_local_bounds(bounds),
                    clip: Default::default(),
                    color,
                    tile: RejectOnceAtlas::tile(AtlasTextureKind::Monochrome),
                    transform: PrimitiveTransform::IDENTITY,
                });
            },
        )
        .size_full()
    }
}

struct AssetRoot(AnyView);

impl Render for AssetRoot {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        self.0
            .clone()
            .cached(StyleRefinement::default().size(px(100.)))
    }
}

#[open_gpui::test]
fn asset_completion_survives_candidate_refusal_and_notified_cached_retry(cx: &mut TestAppContext) {
    let (sender, receiver) = oneshot::channel();
    let source = AssetSource {
        receiver: Arc::new(Mutex::new(Some(receiver))),
        loads: Arc::new(AtomicUsize::new(0)),
    };
    let dependencies = [red(), blue()].map(|color| cx.new(|_| color));
    let output = Rc::new(RefCell::new(None));
    let renders = Rc::new(Cell::new(0));
    let child = cx.new(|_| CachedAssetView {
        source: source.clone(),
        dependencies: dependencies.clone(),
        output: output.clone(),
        renders: renders.clone(),
    });
    let atlas = Arc::new(RejectOnceAtlas::new());
    let handle = cx.add_window(|window, _| {
        window.sprite_atlas = atlas.clone();
        AssetRoot(child.clone().into())
    });
    cx.run_until_parked();
    let notifications = Rc::new(Cell::new(0));
    let _subscription = cx.update(|cx| {
        cx.observe(&child, {
            let notifications = notifications.clone();
            move |_, _| notifications.set(notifications.get() + 1)
        })
    });
    handle.update(cx, |_, _, cx| cx.notify()).unwrap();
    assert_eq!(renders.get(), 1, "reuse the pending cached view");
    assert!(output.borrow().is_none());
    assert_eq!(source.loads.load(Ordering::SeqCst), 1);
    let assert_dependencies = |cx: &App, loaded: bool| {
        for (index, dependency) in dependencies.iter().enumerate() {
            let id = dependency.entity_id();
            let expected = (index == 1) == loaded;
            assert_eq!(
                cx.tracked_entities[&handle.window_id()].contains(&id),
                expected
            );
            assert_eq!(
                cx.window_invalidators_by_entity
                    .get(&id)
                    .is_some_and(|windows| windows.contains_key(&handle.window_id())),
                expected
            );
        }
    };
    let (generation, scene_len, accepted_dependencies) = cx
        .update_window(handle.into(), |_, window, cx| {
            assert_dependencies(cx, false);
            assert_eq!(window.rendered_frame.scene.quads.len(), 1);
            assert_eq!(window.rendered_frame.scene.quads[0].background.solid, red());
            assert_eq!(window.rendered_frame.scene.monochrome_sprites.len(), 1);
            (
                window.rendered_frame_revision(),
                window.rendered_frame.scene.len(),
                cx.tracked_entities[&handle.window_id()].clone(),
            )
        })
        .unwrap();
    let result = Arc::new(42);
    atlas.fail_next_lease();
    sender.send(result.clone()).unwrap();
    cx.run_until_parked();
    assert_eq!(
        notifications.get(),
        1,
        "deduplicate pending uses of this view"
    );
    assert_eq!(
        renders.get(),
        2,
        "completion must invalidate the cached view"
    );
    assert!(Arc::ptr_eq(output.borrow().as_ref().unwrap(), &result));
    cx.update_window(handle.into(), |_, window, cx| {
        assert_eq!(window.rendered_frame_revision(), generation);
        assert_eq!(
            window.presentation_facts().frame_accepted_generation,
            Some(generation)
        );
        assert_eq!(window.rendered_frame.scene.len(), scene_len);
        assert_eq!(window.rendered_frame.scene.quads[0].background.solid, red());
        assert_eq!(window.rendered_frame.scene.monochrome_sprites.len(), 1);
        assert_eq!(
            window
                .last_atlas_frame_rejection
                .as_ref()
                .unwrap()
                .generation,
            generation + 1
        );
        assert!(window.refresh_pending_for_test());
        assert_eq!(
            cx.tracked_entities[&handle.window_id()],
            accepted_dependencies
        );
        assert_dependencies(cx, false);
        assert!(Arc::ptr_eq(
            &cx.fetch_asset::<PendingAsset>(&source).unwrap(),
            &result
        ));
        cx.notify(child.entity_id());
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(handle.into(), |_, window, cx| {
        assert_eq!(window.rendered_frame_revision(), generation + 1);
        assert_eq!(
            window.presentation_facts().frame_accepted_generation,
            Some(generation + 1)
        );
        assert_eq!(
            window.rendered_frame.scene.quads[0].background.solid,
            blue()
        );
        assert_eq!(window.rendered_frame.scene.monochrome_sprites.len(), 1);
        assert!(!window.refresh_pending_for_test());
        assert_dependencies(cx, true);
        assert!(Arc::ptr_eq(
            &cx.fetch_asset::<PendingAsset>(&source).unwrap(),
            &result
        ));
    })
    .unwrap();
    assert!(Arc::ptr_eq(output.borrow().as_ref().unwrap(), &result));
    let accepted_renders = renders.get();
    handle.update(cx, |_, _, cx| cx.notify()).unwrap();
    cx.run_until_parked();
    assert_eq!(
        renders.get(),
        accepted_renders,
        "reuse the recovered cached view"
    );
    assert_eq!(
        notifications.get(),
        2,
        "completion and the explicit retry only"
    );
    assert_eq!(source.loads.load(Ordering::SeqCst), 1);
}
