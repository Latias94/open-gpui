use super::*;
use crate::{AppContext as _, Context, IntoElement, Render, TestAppContext, Window, canvas};
use futures::channel::oneshot;
use std::{
    cell::Cell,
    collections::VecDeque,
    hash::Hasher,
    rc::Rc,
    sync::{
        Mutex,
        atomic::{AtomicUsize, Ordering},
    },
};

struct TestAsset;

#[derive(Clone)]
struct TestAssetSource {
    id: usize,
    loads: Arc<AtomicUsize>,
    completions: Arc<AtomicUsize>,
    receivers: Arc<Mutex<VecDeque<oneshot::Receiver<Result<i32, &'static str>>>>>,
}

impl TestAssetSource {
    fn new(
        id: usize,
        receivers: impl IntoIterator<Item = oneshot::Receiver<Result<i32, &'static str>>>,
    ) -> Self {
        Self {
            id,
            loads: Arc::new(AtomicUsize::new(0)),
            completions: Arc::new(AtomicUsize::new(0)),
            receivers: Arc::new(Mutex::new(receivers.into_iter().collect())),
        }
    }
}

impl Hash for TestAssetSource {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.id.hash(state);
    }
}

impl Asset for TestAsset {
    type Source = TestAssetSource;
    type Output = Result<i32, &'static str>;

    fn load(
        source: Self::Source,
        _: &mut App,
    ) -> impl Future<Output = Self::Output> + Send + 'static {
        source.loads.fetch_add(1, Ordering::SeqCst);
        let receiver = source.receivers.lock().unwrap().pop_front().unwrap();
        async move {
            let result = receiver.await.unwrap_or(Err("cancelled"));
            source.completions.fetch_add(1, Ordering::SeqCst);
            result
        }
    }
}

struct AssetView {
    source: TestAssetSource,
    value: Rc<Cell<Option<Result<i32, &'static str>>>>,
    subscribes: bool,
}

impl Render for AssetView {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let source = self.source.clone();
        let value = self.value.clone();
        let subscribes = self.subscribes;
        canvas(
            move |_, window, cx| {
                value.set(if subscribes {
                    window.use_asset::<TestAsset>(&source, cx)
                } else {
                    window.get_asset::<TestAsset>(&source, cx)
                });
            },
            |_, _, _, _| {},
        )
    }
}

#[open_gpui::test]
fn shared_pending_asset_notifies_every_interested_window(cx: &mut TestAppContext) {
    let (sender, receiver) = oneshot::channel();
    let source = TestAssetSource::new(1, [receiver]);
    let first = cx.add_window(|_, _| AssetView {
        source: source.clone(),
        value: Rc::default(),
        subscribes: true,
    });
    let second = cx.add_window(|_, _| AssetView {
        source: source.clone(),
        value: Rc::default(),
        subscribes: true,
    });
    cx.run_until_parked();
    let notifications = [Rc::new(Cell::new(0)), Rc::new(Cell::new(0))];
    for (window, count) in [first, second].into_iter().zip(&notifications) {
        cx.update(|cx| {
            let entity = window.root(cx).unwrap();
            cx.observe(&entity, {
                let count = count.clone();
                move |_, _| count.set(count.get() + 1)
            })
            .detach();
        });
    }
    assert_eq!(source.loads.load(Ordering::SeqCst), 1);
    sender.send(Ok(42)).unwrap();
    cx.run_until_parked();
    for window in [first, second] {
        cx.update_window(window.into(), |_, window, cx| {
            window.drain_next_frame_callbacks_for_test(cx);
        })
        .unwrap();
    }
    assert_eq!(notifications[0].get(), 1);
    assert_eq!(notifications[1].get(), 1);
}

#[open_gpui::test]
fn removing_pending_asset_cancels_its_window_completion(cx: &mut TestAppContext) {
    let (sender, receiver) = oneshot::channel();
    let source = TestAssetSource::new(2, [receiver]);
    cx.add_window(|_, _| AssetView {
        source: source.clone(),
        value: Rc::default(),
        subscribes: true,
    });
    cx.run_until_parked();
    assert_eq!(source.loads.load(Ordering::SeqCst), 1);
    cx.update(|cx| cx.remove_asset::<TestAsset>(&source));
    cx.run_until_parked();
    assert!(sender.send(Ok(1)).is_err());
    assert_eq!(source.completions.load(Ordering::SeqCst), 0);
}

#[open_gpui::test]
fn cached_load_publication_waits_for_the_active_app_borrow(cx: &mut TestAppContext) {
    let (sender, receiver) = oneshot::channel();
    let load = cx.update(|cx| CachedLoad::new(async move { receiver.await.unwrap() }, cx));
    let notifications = Rc::new(Cell::new(0));
    let entity = cx.update(|cx| {
        let entity = cx.new(|_| ());
        cx.observe(&entity, {
            let notifications = notifications.clone();
            move |_, cx| {
                let notifications = notifications.clone();
                cx.defer(move |_| notifications.set(notifications.get() + 1));
            }
        })
        .detach();
        entity
    });
    assert_eq!(load.use_by(entity.entity_id()), None);
    cx.run_until_parked();
    cx.update(|_| {
        sender.send(42).unwrap();
        cx.dispatcher.run_until_parked();
        assert_eq!(load.get(), None);
        assert_eq!(notifications.get(), 0);
    });
    cx.update(|_| {
        cx.dispatcher.run_until_parked();
        assert_eq!(load.get(), None);
        assert_eq!(notifications.get(), 0);
    });
    cx.run_until_parked();
    assert_eq!(load.get(), Some(42));
    assert_eq!(notifications.get(), 1);
    cx.run_until_parked();
    assert_eq!(notifications.get(), 1);
}

#[open_gpui::test]
fn cached_load_caches_success_and_error(cx: &mut TestAppContext) {
    let successful = cx.update(|cx| CachedLoad::new(async { Ok::<_, &'static str>(42) }, cx));
    let failed = cx.update(|cx| CachedLoad::new(async { Err::<i32, _>("load failed") }, cx));
    assert_eq!(successful.get(), None);
    assert_eq!(failed.get(), None);
    cx.run_until_parked();
    for _ in 0..2 {
        assert_eq!(successful.get(), Some(Ok(42)));
        assert_eq!(failed.get(), Some(Err("load failed")));
    }
}

#[open_gpui::test]
fn completed_load_without_subscribers_is_cached_and_deduplicated(cx: &mut TestAppContext) {
    let (sender, receiver) = oneshot::channel();
    let source = TestAssetSource::new(3, [receiver]);
    for _ in 0..2 {
        assert_eq!(cx.update(|cx| cx.fetch_asset::<TestAsset>(&source)), None);
    }
    assert_eq!(source.loads.load(Ordering::SeqCst), 1);
    sender.send(Ok(42)).unwrap();
    cx.run_until_parked();
    for _ in 0..2 {
        assert_eq!(
            cx.update(|cx| cx.fetch_asset::<TestAsset>(&source)),
            Some(Ok(42))
        );
    }
    assert_eq!(source.loads.load(Ordering::SeqCst), 1);
}

#[open_gpui::test]
fn load_completion_notifies_each_observing_entity_once(cx: &mut TestAppContext) {
    let (sender, receiver) = oneshot::channel();
    let load = cx.update(|cx| CachedLoad::new(async move { receiver.await.unwrap() }, cx));
    let notifications = [Rc::new(Cell::new(0)), Rc::new(Cell::new(0))];
    let entities = notifications.each_ref().map(|count| {
        cx.update(|cx| {
            let entity = cx.new(|_| ());
            cx.observe(&entity, {
                let count = count.clone();
                move |_, _| count.set(count.get() + 1)
            })
            .detach();
            entity
        })
    });
    for entity in &entities {
        for _ in 0..2 {
            assert_eq!(load.use_by(entity.entity_id()), None);
        }
    }
    sender.send(42).unwrap();
    cx.run_until_parked();
    for (entity, count) in entities.iter().zip(&notifications) {
        assert_eq!(count.get(), 1);
        assert_eq!(load.use_by(entity.entity_id()), Some(42));
    }
    cx.run_until_parked();
    assert!(notifications.iter().all(|count| count.get() == 1));
}

#[open_gpui::test]
fn removing_pending_asset_cancels_it_and_replacement_ignores_stale_completion(
    cx: &mut TestAppContext,
) {
    let (stale_sender, stale_receiver) = oneshot::channel();
    let (replacement_sender, replacement_receiver) = oneshot::channel();
    let source = TestAssetSource::new(4, [stale_receiver, replacement_receiver]);
    assert_eq!(cx.update(|cx| cx.fetch_asset::<TestAsset>(&source)), None);
    cx.run_until_parked();
    cx.update(|cx| {
        cx.remove_asset::<TestAsset>(&source);
        assert!(!cx.has_asset::<TestAsset>(&source));
    });
    cx.run_until_parked();
    assert!(stale_sender.send(Ok(1)).is_err());
    assert_eq!(cx.update(|cx| cx.fetch_asset::<TestAsset>(&source)), None);
    replacement_sender.send(Err("replacement failed")).unwrap();
    cx.run_until_parked();
    assert_eq!(
        cx.update(|cx| cx.fetch_asset::<TestAsset>(&source)),
        Some(Err("replacement failed")),
    );
    assert_eq!(source.loads.load(Ordering::SeqCst), 2);
}

#[open_gpui::test]
fn eviction_after_worker_completion_does_not_publish_into_replacement(cx: &mut TestAppContext) {
    let (sender, receiver) = oneshot::channel();
    let (replacement_sender, replacement_receiver) = oneshot::channel();
    let source = TestAssetSource::new(5, [receiver, replacement_receiver]);
    assert_eq!(cx.update(|cx| cx.fetch_asset::<TestAsset>(&source)), None);
    sender.send(Ok(1)).unwrap();
    while source.completions.load(Ordering::SeqCst) == 0 {
        assert!(cx.background_executor.tick());
    }
    assert_eq!(cx.update(|cx| cx.fetch_asset::<TestAsset>(&source)), None);
    cx.update(|cx| {
        cx.remove_asset::<TestAsset>(&source);
        assert_eq!(cx.fetch_asset::<TestAsset>(&source), None);
    });
    cx.run_until_parked();
    assert_eq!(cx.update(|cx| cx.fetch_asset::<TestAsset>(&source)), None);
    replacement_sender.send(Ok(2)).unwrap();
    cx.run_until_parked();
    assert_eq!(
        cx.update(|cx| cx.fetch_asset::<TestAsset>(&source)),
        Some(Ok(2))
    );
    assert_eq!(source.loads.load(Ordering::SeqCst), 2);
}

#[open_gpui::test]
fn get_only_window_does_not_subscribe_and_later_use_still_does(cx: &mut TestAppContext) {
    let (sender, receiver) = oneshot::channel();
    let source = TestAssetSource::new(6, [receiver]);
    assert_eq!(cx.update(|cx| cx.fetch_asset::<TestAsset>(&source)), None);
    let windows = [false, true].map(|subscribes| {
        cx.add_window(|_, _| AssetView {
            source: source.clone(),
            value: Rc::default(),
            subscribes,
        })
    });
    cx.run_until_parked();
    let notifications = [Rc::new(Cell::new(0)), Rc::new(Cell::new(0))];
    for (window, count) in windows.into_iter().zip(&notifications) {
        cx.update(|cx| {
            let entity = window.root(cx).unwrap();
            cx.observe(&entity, {
                let count = count.clone();
                move |_, _| count.set(count.get() + 1)
            })
            .detach();
        });
    }
    sender.send(Ok(42)).unwrap();
    cx.run_until_parked();
    assert_eq!(notifications[0].get(), 0);
    assert_eq!(notifications[1].get(), 1);
    assert_eq!(
        cx.update(|cx| cx.fetch_asset::<TestAsset>(&source)),
        Some(Ok(42))
    );
    assert_eq!(source.loads.load(Ordering::SeqCst), 1);
}

#[open_gpui::test]
fn eviction_while_publication_waits_cancels_old_entry_and_notifies_only_replacement(
    cx: &mut TestAppContext,
) {
    let (sender, receiver) = oneshot::channel();
    let (replacement_sender, replacement_receiver) = oneshot::channel();
    let source = TestAssetSource::new(7, [receiver, replacement_receiver]);
    let notifications = Rc::new(Cell::new(0));
    let entity = cx.update(|cx| {
        let entity = cx.new(|_| ());
        cx.observe(&entity, {
            let notifications = notifications.clone();
            move |_, _| notifications.set(notifications.get() + 1)
        })
        .detach();
        assert_eq!(
            cx.asset_entry::<TestAsset>(&source)
                .use_by(entity.entity_id()),
            None
        );
        entity
    });
    cx.run_until_parked();
    let dispatcher = cx.dispatcher.clone();
    cx.update(|cx| {
        sender.send(Ok(1)).unwrap();
        dispatcher.run_until_parked();
        assert_eq!(cx.fetch_asset::<TestAsset>(&source), None);
        assert_eq!(notifications.get(), 0);
        cx.remove_asset::<TestAsset>(&source);
        assert_eq!(
            cx.asset_entry::<TestAsset>(&source)
                .use_by(entity.entity_id()),
            None
        );
        replacement_sender.send(Ok(2)).unwrap();
        dispatcher.run_until_parked();
        assert_eq!(cx.fetch_asset::<TestAsset>(&source), None);
        assert_eq!(notifications.get(), 0);
    });
    cx.run_until_parked();
    assert_eq!(
        cx.update(|cx| cx.fetch_asset::<TestAsset>(&source)),
        Some(Ok(2))
    );
    assert_eq!(source.loads.load(Ordering::SeqCst), 2);
    assert_eq!(notifications.get(), 1);
}

#[test]
fn pending_foreground_publication_does_not_retain_a_released_app_or_result() {
    let cx = TestAppContext::single();
    let lifetime = Rc::downgrade(&cx.app);
    let dispatcher = cx.dispatcher.clone();
    let output = Arc::new(());
    let output_lifetime = Arc::downgrade(&output);
    let load = cx.update(|cx| CachedLoad::new(async move { output }, cx));
    while dispatcher.tick(true) {}
    assert!(output_lifetime.upgrade().is_some());
    assert!(load.get().is_none());
    drop(cx);
    assert!(lifetime.upgrade().is_none());
    dispatcher.run_until_parked();
    assert!(output_lifetime.upgrade().is_none());
    assert!(load.get().is_none());
}

#[open_gpui::test]
fn completion_observer_can_remove_and_replace_its_own_asset(cx: &mut TestAppContext) {
    let (sender, receiver) = oneshot::channel();
    let (replacement_sender, replacement_receiver) = oneshot::channel();
    let source = TestAssetSource::new(8, [receiver, replacement_receiver]);
    let notifications = Rc::new(Cell::new(0));
    let entity = cx.update(|cx| {
        let entity = cx.new(|_| ());
        cx.observe(&entity, {
            let source = source.clone();
            let notifications = notifications.clone();
            move |_, cx| {
                notifications.set(notifications.get() + 1);
                assert_eq!(cx.fetch_asset::<TestAsset>(&source), Some(Ok(1)));
                cx.remove_asset::<TestAsset>(&source);
                assert_eq!(cx.fetch_asset::<TestAsset>(&source), None);
            }
        })
        .detach();
        assert_eq!(
            cx.asset_entry::<TestAsset>(&source)
                .use_by(entity.entity_id()),
            None
        );
        entity
    });
    sender.send(Ok(1)).unwrap();
    cx.run_until_parked();
    assert_eq!(notifications.get(), 1);
    assert_eq!(source.loads.load(Ordering::SeqCst), 2);
    assert_eq!(cx.update(|cx| cx.fetch_asset::<TestAsset>(&source)), None);
    replacement_sender.send(Ok(2)).unwrap();
    cx.run_until_parked();
    assert_eq!(
        cx.update(|cx| cx.fetch_asset::<TestAsset>(&source)),
        Some(Ok(2))
    );
    assert_eq!(notifications.get(), 1);
    drop(entity);
}

#[open_gpui::test]
fn publication_recovers_after_the_waited_app_update_unwinds(cx: &mut TestAppContext) {
    let (sender, receiver) = oneshot::channel();
    let load = cx.update(|cx| CachedLoad::new(async move { receiver.await.unwrap() }, cx));
    let notifications = Rc::new(Cell::new(0));
    let entity = cx.update(|cx| {
        let entity = cx.new(|_| ());
        cx.observe(&entity, {
            let notifications = notifications.clone();
            move |_, cx| {
                let notifications = notifications.clone();
                cx.defer(move |_| notifications.set(notifications.get() + 1));
            }
        })
        .detach();
        entity
    });
    assert_eq!(load.use_by(entity.entity_id()), None);
    let unwound = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        cx.update(|_| {
            sender.send(42).unwrap();
            cx.dispatcher.run_until_parked();
            assert_eq!(load.get(), None);
            panic!("injected waited-update unwind");
        });
    }));
    assert!(unwound.is_err());
    cx.run_until_parked();
    assert_eq!(load.get(), Some(42));
    assert_eq!(notifications.get(), 1);
    cx.run_until_parked();
    assert_eq!(notifications.get(), 1);
}

#[test]
fn terminal_shutdown_before_borrow_release_discards_the_pending_publication() {
    let cx = TestAppContext::single();
    let dispatcher = cx.dispatcher.clone();
    let (sender, receiver) = oneshot::channel();
    let load = cx.update(|cx| CachedLoad::new(async move { receiver.await.unwrap() }, cx));
    let notifications = Rc::new(Cell::new(0));
    let entity = cx.update(|cx| {
        let entity = cx.new(|_| ());
        cx.observe(&entity, {
            let notifications = notifications.clone();
            move |_, _| notifications.set(notifications.get() + 1)
        })
        .detach();
        entity
    });
    assert!(load.use_by(entity.entity_id()).is_none());
    let output = Arc::new(());
    let output_lifetime = Arc::downgrade(&output);
    cx.update(|cx| {
        sender.send(output).unwrap();
        dispatcher.run_until_parked();
        assert!(load.get().is_none());
        assert!(output_lifetime.upgrade().is_some());
        cx.shutdown_for_native_exit_test();
    });
    dispatcher.run_until_parked();
    assert!(cx.update(|cx| cx.native_exit_authority_is_settled_for_test()));
    assert!(load.get().is_none());
    assert!(output_lifetime.upgrade().is_none());
    assert_eq!(notifications.get(), 0);
}
