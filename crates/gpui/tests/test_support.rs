//! Public consumers of additive test support, based on Zed
//! 635026fabc38440fce5e1767d125026a17b12012 and introduction
//! 95cd535a5fad96d649513f96c5784ceefd379e47.

#[test]
fn identity_queries_are_public_without_test_support() {
    let _: fn(&open_gpui::App) -> bool = open_gpui::App::is_test;
    let _: fn(&open_gpui::AsyncApp) -> bool = open_gpui::AsyncApp::is_test;
    let _: fn(&open_gpui::BackgroundExecutor) -> bool = open_gpui::BackgroundExecutor::is_test;
    let _: fn(&open_gpui_scheduler::LocalExecutor) -> bool =
        open_gpui_scheduler::LocalExecutor::is_test;
    let _: fn(&open_gpui_scheduler::BackgroundExecutor) -> bool =
        open_gpui_scheduler::BackgroundExecutor::is_test;
    let _: fn() -> bool = open_gpui_scheduler::test_scheduler_created;
}

#[cfg(feature = "test-support")]
mod test_contexts {
    use open_gpui::{
        App, AppContext as _, Context, Entity, IntoElement, LeakDetectorSnapshot, Render,
        Styled as _, Subscription, TestAppContext, VisualTestContext, Window, div, px, rgb, size,
    };
    use std::{
        cell::Cell,
        panic::{AssertUnwindSafe, catch_unwind},
        rc::Rc,
        sync::Arc,
        thread,
        time::Duration,
    };

    fn tracking_expected() -> bool {
        match std::env::var("GPUI_TEST_EXPECT_LEAK_DETECTION").as_deref() {
            Ok("on") => true,
            Ok("off") => {
                assert!(!cfg!(feature = "leak-detection"));
                false
            }
            Err(std::env::VarError::NotPresent) => {
                cfg!(feature = "leak-detection")
                    || option_env!("GPUI_LEAK_DETECTION").is_some_and(|value| !value.is_empty())
            }
            _ => panic!("set GPUI_TEST_EXPECT_LEAK_DETECTION to on or off for this build"),
        }
    }

    struct Model {
        value: usize,
        drops: Rc<Cell<usize>>,
    }

    impl Drop for Model {
        fn drop(&mut self) {
            self.drops.set(self.drops.get() + 1);
        }
    }

    #[open_gpui::test]
    fn typed_erased_and_weak_handles_preserve_real_release_lifetimes(cx: &mut TestAppContext) {
        let drops = Rc::new(Cell::new(0));
        let releases = Rc::new(Cell::new(0));
        let entity = cx.new({
            let drops = drops.clone();
            let releases = releases.clone();
            move |cx| {
                cx.on_release(move |model: &mut Model, app| {
                    assert!(app.is_test());
                    assert_eq!(model.value, 42);
                    releases.set(releases.get() + 1);
                })
                .detach();
                Model { value: 1, drops }
            }
        });
        let weak = entity.downgrade();
        let cloned = entity.clone();
        let erased = cloned.into_any();
        let erased = erased
            .downcast::<usize>()
            .expect_err("a failed downcast must return the original strong handle");
        let erased_weak = erased.downgrade();
        let restored = erased
            .clone()
            .downcast::<Model>()
            .expect("an erased clone must retain its original type");
        assert_eq!(restored.entity_id(), weak.entity_id());
        restored.update(cx, |model, cx| {
            model.value = 42;
            cx.notify();
        });
        assert_eq!(weak.read_with(cx, |model, _| model.value).unwrap(), 42);
        assert!(catch_unwind(AssertUnwindSafe(|| weak.assert_released())).is_err());

        drop((entity, restored));
        assert!(erased_weak.is_upgradable());
        assert!(catch_unwind(AssertUnwindSafe(|| erased_weak.assert_released())).is_err());
        let upgraded = erased_weak
            .upgrade()
            .expect("the erased owner is still live");
        drop(erased);
        assert!(weak.upgrade().is_some());
        assert_eq!(drops.get(), 0);
        assert_eq!(releases.get(), 0);

        drop(upgraded);
        assert!(weak.upgrade().is_none());
        assert!(!erased_weak.is_upgradable());
        assert!(catch_unwind(AssertUnwindSafe(|| weak.assert_released())).is_err());
        assert_eq!(drops.get(), 0);
        assert_eq!(releases.get(), 0);

        cx.update(|_| ());
        weak.assert_released();
        erased_weak.assert_released();
        assert_eq!(drops.get(), 1);
        assert_eq!(releases.get(), 1);
        assert!(weak.read_with(cx, |model, _| model.value).is_err());
        assert!(weak.update(cx, |model, _| model.value += 1).is_err());
    }

    #[open_gpui::test]
    fn snapshots_track_new_entities_only_when_explicitly_enabled(cx: &mut TestAppContext) {
        let tracking = tracking_expected();
        let empty = LeakDetectorSnapshot::default();
        cx.read(|app| app.assert_no_new_leaks(&empty));
        let existing = cx.new(|_| 1_usize);
        let baseline = cx.read(App::leak_detector_snapshot);
        let existing_clone = existing.clone().into_any();
        cx.read(|app| app.assert_no_new_leaks(&baseline));
        assert_eq!(
            catch_unwind(AssertUnwindSafe(|| {
                cx.read(|app| app.assert_no_new_leaks(&empty));
            }))
            .is_err(),
            tracking,
            "Default snapshots must detect a live entity only with tracking enabled"
        );

        let created = cx.new(|_| 42_usize);
        let weak = created.downgrade();
        let erased = created.clone().into_any();
        drop(created);
        assert_eq!(
            catch_unwind(AssertUnwindSafe(|| {
                cx.read(|app| app.assert_no_new_leaks(&baseline));
            }))
            .is_err(),
            tracking,
            "an erased handle must keep a newly created entity in a tracked snapshot"
        );
        let upgraded = weak
            .upgrade()
            .expect("the erased handle keeps the entity live");
        drop(erased);
        assert_eq!(
            catch_unwind(AssertUnwindSafe(|| {
                cx.read(|app| app.assert_no_new_leaks(&baseline));
            }))
            .is_err(),
            tracking,
            "a weak upgrade must register its new strong handle"
        );

        drop(upgraded);
        cx.read(|app| app.assert_no_new_leaks(&baseline));
        cx.update(|_| ());
        weak.assert_released();
        assert_eq!(existing.read_with(cx, |value, _| *value), 1);
        drop((existing, existing_clone));
        cx.update(|_| ());
        cx.read(|app| app.assert_no_new_leaks(&empty));
    }

    struct ObservedView {
        source: Entity<Model>,
        renders: Rc<Cell<usize>>,
        rendered_value: Rc<Cell<usize>>,
        _subscription: Subscription,
    }

    impl Render for ObservedView {
        fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            self.renders.set(self.renders.get() + 1);
            self.rendered_value.set(self.source.read(cx).value);
            div().size_full().bg(rgb(0x304050))
        }
    }

    #[open_gpui::test]
    fn real_test_window_flushes_notifications_and_cascading_entity_releases(
        cx: &mut TestAppContext,
    ) {
        let drops = Rc::new(Cell::new(0));
        let source = cx.new({
            let drops = drops.clone();
            move |_| Model { value: 1, drops }
        });
        let source_weak = source.downgrade();
        let renders = Rc::new(Cell::new(0));
        let rendered_value = Rc::new(Cell::new(0));
        let handle = cx.open_window(size(px(96.), px(64.)), {
            let source = source.clone();
            let renders = renders.clone();
            let rendered_value = rendered_value.clone();
            move |_, cx| ObservedView {
                _subscription: cx.observe(&source, |_, _, cx| cx.notify()),
                source,
                renders,
                rendered_value,
            }
        });
        let view = handle.root(cx).expect("the window must own its root view");
        let view_weak = view.downgrade();
        let mut visual = VisualTestContext::from_window(handle.into(), cx);
        visual.update(|window, _| window.refresh());
        assert!(renders.get() > 0);
        assert_eq!(rendered_value.get(), 1);
        let initial_renders = renders.get();

        source.update(cx, |model, cx| {
            model.value = 42;
            cx.notify();
        });
        assert!(renders.get() > initial_renders);
        assert_eq!(rendered_value.get(), 42);
        drop((view, source));
        cx.update(|_| ());
        assert!(view_weak.upgrade().is_some());
        assert!(source_weak.upgrade().is_some());

        visual.update(|window, app| window.remove_window(app));
        drop(visual);
        cx.run_until_parked();
        cx.update(|_| ());
        view_weak.assert_released();
        source_weak.assert_released();
        assert_eq!(drops.get(), 1);
    }

    #[test]
    fn test_and_threaded_executors_keep_their_own_identity_in_one_process() {
        let dispatcher = Arc::new(open_gpui::ThreadedDispatcher::new());
        let real_background = open_gpui::BackgroundExecutor::new(dispatcher.clone());
        let real_foreground = open_gpui::ForegroundExecutor::new(dispatcher.clone());
        assert!(!real_background.is_test());
        assert!(!real_foreground.scheduler_executor().is_test());

        let cx = TestAppContext::single();
        let other_app = cx.new_app();
        assert!(open_gpui_scheduler::test_scheduler_created());
        assert!(cx.read(App::is_test));
        assert!(other_app.read(App::is_test));
        assert!(cx.to_async().is_test());
        assert!(cx.executor().is_test());
        assert!(cx.executor().scheduler_executor().is_test());
        assert!(cx.foreground_executor().scheduler_executor().is_test());
        assert!(!real_background.clone().is_test());
        assert!(!real_background.scheduler_executor().is_test());
        assert!(!real_foreground.scheduler_executor().clone().is_test());

        let test_task = cx.spawn(async |app| {
            assert!(app.is_test());
            let value = app.background_executor().spawn(async { 42_usize }).await;
            app.update(|app| app.new(|_| value))
        });
        let entity = cx.foreground_executor().block_test(test_task);
        assert_eq!(entity.read_with(&cx, |value, _| *value), 42);
        let weak = entity.downgrade();
        drop(entity);
        cx.update(|_| ());
        weak.assert_released();

        let main_thread = thread::current().id();
        let task = real_background.spawn(async move {
            assert_ne!(thread::current().id(), main_thread);
            42_usize
        });
        let Ok(value) = real_foreground.block_with_timeout(Duration::from_secs(10), task) else {
            panic!("the real background task did not complete");
        };
        assert_eq!(value, 42);
        dispatcher.run_until_idle();
        other_app.quit();
        cx.quit();
        assert!(open_gpui_scheduler::test_scheduler_created());
        assert!(!real_background.is_test());
    }

    #[cfg(feature = "bench-support")]
    #[test]
    fn benchmark_context_uses_real_workers_and_timers_after_a_test_scheduler_exists() {
        use open_gpui::{BenchAppContext, BenchMeasurement, NoopTextSystem, bench_platform};
        use std::time::Instant;

        let test_app = TestAppContext::single();
        let test_clock = test_app.executor().now();
        let iterations = Cell::new(0_usize);
        let mut criterion = criterion::Criterion::default()
            .with_measurement(BenchMeasurement::new(criterion::measurement::WallTime))
            .without_plots()
            .sample_size(10)
            .nresamples(100)
            .warm_up_time(Duration::from_millis(1))
            .measurement_time(Duration::from_millis(1));
        criterion.bench_function("public-test-support-real-clock", |bencher| {
            let platform = bench_platform(None, Arc::new(NoopTextSystem::new()));
            let mut cx = BenchAppContext::new(platform, None, bencher);
            assert!(open_gpui_scheduler::test_scheduler_created());
            assert!(test_app.read(App::is_test));
            assert!(!cx.read(App::is_test));
            assert!(!cx.read(|app| app.to_async().is_test()));
            assert!(!cx.background_executor().is_test());
            assert!(!cx.background_executor().scheduler_executor().is_test());
            assert!(!cx.foreground_executor().scheduler_executor().is_test());
            let drops = Rc::new(Cell::new(0));
            let entity = cx.new({
                let drops = drops.clone();
                move |_| Model { value: 0, drops }
            });
            let weak = entity.downgrade();
            let completed = Rc::new(Cell::new(0_usize));
            cx.bench_task(|cx| {
                let main_thread = thread::current().id();
                let background = cx.background_executor().clone();
                let task = cx.background_executor().spawn(async move {
                    assert!(!background.is_test());
                    assert_ne!(thread::current().id(), main_thread);
                    let start = Instant::now();
                    let delay = Duration::from_millis(2);
                    background.timer(delay).await;
                    assert!(start.elapsed() >= delay);
                    42_usize
                });
                let entity = entity.clone();
                let completed = completed.clone();
                cx.update(|app| {
                    app.spawn(async move |app| {
                        assert!(!app.is_test());
                        let value = task.await;
                        app.update(|app| entity.update(app, |model, _| model.value += value));
                        completed.set(completed.get() + 1);
                        entity
                    })
                })
            });
            assert!(completed.get() > 0);
            assert_eq!(
                entity.read_with(&cx, |model, _| model.value),
                completed.get() * 42
            );
            iterations.set(iterations.get() + completed.get());
            drop(entity);
            cx.settle();
            weak.assert_released();
            assert_eq!(drops.get(), 1);
            assert_eq!(test_app.executor().now(), test_clock);
            cx.teardown();
        });
        assert!(iterations.get() > 0);
        assert_eq!(test_app.executor().now(), test_clock);
        test_app.quit();
    }
}
