#![cfg(feature = "bench-support")]

//! Executes the maintained benchmark harness through crate-external public APIs.
//! Windows use the benchmark platform without a GPU renderer. Presentation assertions
//! describe that platform's accepted/submitted facts, not native GPU output.

use criterion::{Criterion, measurement::WallTime};
use futures::channel::oneshot;
use open_gpui::{
    AppContext as _, AsyncWindowContext, BenchAppContext, BenchMeasurement, BenchReport, Context,
    Entity, Global, IntoElement, NoopTextSystem, ParentElement as _, PlatformWindowPresentOutcome,
    QuitMode, Render, Styled as _, Subscription, VisualContext as _, Window, bench_platform,
    canvas, div, fill, profiler, px, rgb, size,
};
use std::{
    cell::{Cell, RefCell},
    future::Future as _,
    pin::Pin,
    rc::{Rc, Weak},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    task::Poll,
    thread,
    time::{Duration, Instant},
};

const TIMER_DELAY: Duration = Duration::from_millis(2);
static HARNESS_LOCK: Mutex<()> = Mutex::new(());

fn criterion() -> Criterion<BenchMeasurement> {
    Criterion::default()
        .with_measurement(BenchMeasurement::new(WallTime).with_secondary("elapsed", WallTime))
        .warm_up_time(Duration::from_millis(1))
        .measurement_time(Duration::from_millis(5))
        .sample_size(10)
        .nresamples(10)
        .without_plots()
}

struct DropProbe(Rc<Cell<usize>>);

impl Drop for DropProbe {
    fn drop(&mut self) {
        self.0.set(self.0.get() + 1);
    }
}

struct AppOwner(Rc<DropProbe>);

impl Global for AppOwner {}

#[derive(Default)]
struct Observations {
    notifications: Cell<usize>,
    renders: Cell<usize>,
    paints: Cell<usize>,
    painted: Cell<usize>,
    committed: Cell<usize>,
    releases: Cell<usize>,
}

struct SourceView {
    source: Entity<usize>,
    observed: Rc<Observations>,
    _subscription: Subscription,
}

impl SourceView {
    fn new(source: Entity<usize>, observed: Rc<Observations>, cx: &mut Context<Self>) -> Self {
        let subscription = cx.observe(&source, {
            let observed = Rc::clone(&observed);
            move |_, _, cx| {
                observed.notifications.set(observed.notifications.get() + 1);
                cx.notify();
            }
        });
        Self {
            source,
            observed,
            _subscription: subscription,
        }
    }
}

impl Drop for SourceView {
    fn drop(&mut self) {
        self.observed.releases.set(self.observed.releases.get() + 1);
    }
}

impl Render for SourceView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.observed.renders.set(self.observed.renders.get() + 1);
        let value = *self.source.read(cx);
        let prepaint = Rc::clone(&self.observed);
        let paint = Rc::clone(&self.observed);
        div().size_full().child(
            canvas(
                move |_, window, _| {
                    let observed = Rc::clone(&prepaint);
                    window.record_prepaint_window_commit(move |_, _, _| {
                        observed.committed.set(value);
                    });
                },
                move |bounds, (), window, _| {
                    paint.paints.set(paint.paints.get() + 1);
                    paint.painted.set(value);
                    window.paint_quad(fill(bounds, rgb(0x305080)));
                },
            )
            .size_full(),
        )
    }
}

fn assert_submitted(window: &Window) -> u64 {
    let facts = window.presentation_facts();
    let generation = facts
        .frame_accepted_generation
        .expect("the benchmark must accept an actual frame");
    assert_eq!(facts.present_submitted_generation, Some(generation));
    assert_eq!(facts.non_empty_presented_generation, Some(generation));
    let attempt = facts.latest_present_attempt.unwrap();
    assert_eq!(attempt.generation, generation);
    assert_eq!(attempt.outcome, PlatformWindowPresentOutcome::Submitted);
    assert!(attempt.contained_valid_primitives);
    generation
}

#[test]
fn renderer_runs_source_notifications_callbacks_resize_and_reconstruction() {
    let _guard = HARNESS_LOCK.lock().unwrap();
    let mut criterion = criterion();
    let routines = Cell::new(0);
    criterion.bench_function("public-benchmark/source-renderer", |bencher| {
        let mut cx = BenchAppContext::new(
            bench_platform(None, Arc::new(NoopTextSystem)),
            Some("source-renderer"),
            bencher,
        );
        assert_eq!(cx.benchmark_name(), Some("source-renderer"));
        cx.update(|app| app.set_quit_mode(QuitMode::Explicit));
        let source_releases = Rc::new(Cell::new(0));
        let source = cx.new({
            let releases = Rc::clone(&source_releases);
            move |cx| {
                cx.on_release(move |_, _| releases.set(releases.get() + 1))
                    .detach();
                0_usize
            }
        });
        let source_weak = source.downgrade();
        let observed = Rc::new(Observations::default());
        let mut window = cx.add_empty_window();
        let view = window
            .replace_root_view(|_, cx| SourceView::new(source.clone(), Rc::clone(&observed), cx))
            .unwrap();
        let weak = view.downgrade();
        cx.settle();
        let initial_generation = window.update(|window, _| assert_submitted(window));
        let iterations = Cell::new(0);
        cx.bench_renderer(view.clone(), |view, _, cx| {
            iterations.set(iterations.get() + 1);
            view.source.update(cx, |value, cx| {
                *value += 1;
                cx.notify();
            });
        });
        cx.settle();
        assert!(iterations.get() > 0);
        assert_eq!(observed.notifications.get(), iterations.get());
        assert_eq!(observed.painted.get(), iterations.get());
        assert_eq!(observed.committed.get(), iterations.get());
        assert!(window.update(|window, _| assert_submitted(window)) > initial_generation);

        let callback = Rc::new(Cell::new(false));
        window.update({
            let callback = Rc::clone(&callback);
            move |window, _| {
                window.on_next_frame(move |_, _| callback.set(true));
            }
        });
        cx.run_until(|| callback.get().then_some(()));
        let renders = observed.renders.get();
        window.simulate_resize(size(px(240.), px(160.)));
        cx.settle();
        window.update(|window, _| {
            assert_eq!(window.viewport_size(), size(px(240.), px(160.)));
            assert_submitted(window);
        });
        assert!(observed.renders.get() > renders);

        let replacement_observed = Rc::new(Observations::default());
        let replacement = window
            .replace_root_view(|_, cx| {
                SourceView::new(source.clone(), Rc::clone(&replacement_observed), cx)
            })
            .unwrap();
        let replacement_weak = replacement.downgrade();
        drop(view);
        cx.settle();
        assert!(weak.upgrade().is_none());
        assert_eq!(observed.releases.get(), 1);
        let retired_notifications = observed.notifications.get();
        source.update(&mut cx, |value, cx| {
            *value += 1;
            cx.notify();
        });
        cx.settle();
        assert_eq!(observed.notifications.get(), retired_notifications);
        assert_eq!(replacement_observed.notifications.get(), 1);
        assert_eq!(replacement_observed.painted.get(), iterations.get() + 1);
        window.update(|window, _| {
            assert_submitted(window);
        });
        window.update(|window, app| window.remove_window(app));
        drop((window, replacement, source));
        cx.settle();
        assert!(replacement_weak.upgrade().is_none());
        assert!(source_weak.upgrade().is_none());
        assert_eq!(replacement_observed.releases.get(), 1);
        assert_eq!(source_releases.get(), 1);
        assert!(cx.read(|app| app.windows().is_empty()));
        cx.teardown();
        routines.set(routines.get() + 1);
    });
    assert!(routines.get() > 0);
}

struct BatchedInput {
    owner: Rc<DropProbe>,
    drops: Rc<Cell<usize>>,
}

impl Drop for BatchedInput {
    fn drop(&mut self) {
        assert!(!profiler::trace_enabled());
        self.drops.set(self.drops.get() + 1);
    }
}

struct BatchedOutput(Rc<DropProbe>, Rc<Cell<usize>>);

impl Drop for BatchedOutput {
    fn drop(&mut self) {
        assert!(!profiler::trace_enabled());
        assert_eq!(Rc::strong_count(&self.0), 2);
        self.1.set(self.1.get() + 1);
    }
}

#[test]
fn batched_tasks_execute_real_timers_and_external_callbacks_outside_setup_and_drop() {
    let _guard = HARNESS_LOCK.lock().unwrap();
    let mut criterion = criterion();
    let report = BenchReport::with_frame_budget_nanos(1_000_000);
    let setups = Rc::new(Cell::new(0));
    let input_drops = Rc::new(Cell::new(0));
    let output_drops = Rc::new(Cell::new(0));
    let owner_drops = Rc::new(Cell::new(0));
    let iterations = Rc::new(Cell::new(0));
    criterion.bench_function("public-benchmark/batched-tasks", |bencher| {
        let mut cx = BenchAppContext::new_with_platform_and_report(
            bench_platform(None, Arc::new(NoopTextSystem)),
            Some("batched-tasks"),
            bencher,
            report.clone(),
        );
        let last_owner: RefCell<Option<Weak<DropProbe>>> = RefCell::new(None);
        cx.bench_batched_task(
            |_| {
                assert!(!profiler::trace_enabled());
                if let Some(owner) = last_owner.borrow().as_ref() {
                    assert!(owner.upgrade().is_none());
                }
                setups.set(setups.get() + 1);
                let owner = Rc::new(DropProbe(Rc::clone(&owner_drops)));
                last_owner.replace(Some(Rc::downgrade(&owner)));
                BatchedInput {
                    owner,
                    drops: Rc::clone(&input_drops),
                }
            },
            |input, cx| {
                assert!(profiler::trace_enabled());
                iterations.set(iterations.get() + 1);
                let owner = Rc::clone(&input.owner);
                let drops = Rc::clone(&output_drops);
                let started = Instant::now();
                let timer = cx.background_executor().timer(TIMER_DELAY);
                let main_thread = thread::current().id();
                let (sender, mut receiver) = oneshot::channel();
                let (release_callback, callback_gate) = mpsc::channel();
                let callback = thread::spawn(move || {
                    callback_gate.recv().unwrap();
                    sender.send(thread::current().id()).unwrap();
                });
                cx.update(|app| {
                    app.spawn(async move |_| {
                        thread::sleep(profiler::journal::TASK_POLL_FLOOR * 2);
                        timer.await;
                        assert!(started.elapsed() >= TIMER_DELAY);
                        let mut release_callback = Some(release_callback);
                        let callback_thread =
                            futures::future::poll_fn(|cx| match Pin::new(&mut receiver).poll(cx) {
                                Poll::Pending => {
                                    if let Some(release) = release_callback.take() {
                                        release.send(()).unwrap();
                                    }
                                    Poll::Pending
                                }
                                Poll::Ready(result) => {
                                    assert!(release_callback.is_none());
                                    Poll::Ready(result.unwrap())
                                }
                            })
                            .await;
                        assert_ne!(callback_thread, main_thread);
                        callback.join().unwrap();
                        assert_eq!(thread::current().id(), main_thread);
                        assert!(profiler::trace_enabled());
                        BatchedOutput(owner, drops)
                    })
                })
            },
        );
        cx.settle();
        assert_eq!(setups.get(), iterations.get());
        assert_eq!(input_drops.get(), iterations.get());
        assert_eq!(output_drops.get(), iterations.get());
        assert_eq!(owner_drops.get(), iterations.get());
        assert!(last_owner.borrow().as_ref().unwrap().upgrade().is_none());
        cx.teardown();
    });
    assert!(iterations.get() > 0);
    assert!(report.foreground_work().unwrap().count > 0);
    let elapsed = report.metrics().values("elapsed").unwrap();
    assert!(!elapsed.is_empty());
    assert!(elapsed.iter().all(|value| value.is_finite() && *value > 0.));
}

#[test]
fn teardown_releases_app_and_task_results_before_the_next_shared_dispatcher_routine() {
    let _guard = HARNESS_LOCK.lock().unwrap();
    let mut criterion = criterion();
    let app_drops = Rc::new(Cell::new(0));
    let task_drops = Rc::new(Cell::new(0));
    let resumed = Rc::new(Cell::new(0));
    let dispatcher = RefCell::new(None);
    criterion.bench_function("public-benchmark/teardown", |bencher| {
        let mut cx = BenchAppContext::new(
            bench_platform(None, Arc::new(NoopTextSystem)),
            Some("teardown"),
            bencher,
        );
        let previous_app_drops = app_drops.get();
        let app_owner = Rc::new(DropProbe(Rc::clone(&app_drops)));
        let app_weak = Rc::downgrade(&app_owner);
        cx.update(|app| {
            assert!(!app.has_global::<AppOwner>());
            assert!(app.windows().is_empty());
            app.set_global(AppOwner(app_owner));
            assert_eq!(Rc::strong_count(&app.global::<AppOwner>().0), 1);
        });
        let text_system_weak = cx.read(|app| Arc::downgrade(app.text_system()));
        dispatcher.replace(Some(cx.background_executor().dispatcher().clone()));
        let mut clone = cx.clone();
        clone.update(|app| assert!(app.has_global::<AppOwner>()));
        let previous_task_drops = task_drops.get();
        let owner = Rc::new(DropProbe(Rc::clone(&task_drops)));
        let weak = Rc::downgrade(&owner);
        let timer = cx.background_executor().timer(Duration::from_secs(3_600));
        let task = cx.foreground_executor().spawn({
            let resumed = Rc::clone(&resumed);
            async move {
                timer.await;
                resumed.set(resumed.get() + 1);
                owner
            }
        });
        cx.run_until_idle();
        assert!(!task.is_ready());
        cx.bench_iter(|cx| {
            assert!(cx.read(|app| app.has_global::<AppOwner>()));
        });
        let before_resume = resumed.get();
        drop(clone);
        cx.teardown();
        assert!(app_weak.upgrade().is_none());
        assert!(text_system_weak.upgrade().is_none());
        assert_eq!(app_drops.get(), previous_app_drops + 1);
        assert_eq!(resumed.get(), before_resume + 1);
        assert!(task.is_ready());
        assert!(weak.upgrade().is_some());
        assert_eq!(task_drops.get(), previous_task_drops);
        drop(task);
        assert!(weak.upgrade().is_none());
        assert_eq!(task_drops.get(), previous_task_drops + 1);
    });
    let settled_resumes = resumed.get();
    assert!(settled_resumes > 0);
    criterion.bench_function("public-benchmark/next-routine", |bencher| {
        let mut cx = BenchAppContext::new(
            bench_platform(None, Arc::new(NoopTextSystem)),
            Some("next-routine"),
            bencher,
        );
        assert!(Arc::ptr_eq(
            dispatcher.borrow().as_ref().unwrap(),
            cx.background_executor().dispatcher(),
        ));
        cx.bench_iter(|cx| {
            assert!(cx.read(|app| app.windows().is_empty()));
            assert!(!cx.read(|app| app.has_global::<AppOwner>()));
            assert_eq!(resumed.get(), settled_resumes);
        });
        cx.settle();
        cx.teardown();
        assert_eq!(resumed.get(), settled_resumes);
    });
}

struct RendererSession {
    view: Entity<SourceView>,
    window: AsyncWindowContext,
    stopped: Arc<AtomicBool>,
    inputs: usize,
    completed: Rc<Cell<usize>>,
}

impl Drop for RendererSession {
    fn drop(&mut self) {
        assert!(!profiler::trace_enabled());
        assert!(self.stopped.load(Ordering::Acquire));
        assert_eq!(self.inputs, 3);
        self.window
            .update(|window, app| {
                assert_submitted(window);
                let observed = &self.view.read(app).observed;
                assert_eq!(observed.painted.get(), 3);
                assert_eq!(observed.committed.get(), 3);
                window.remove_window(app);
            })
            .unwrap();
        self.completed.set(self.completed.get() + 1);
    }
}

#[test]
fn finite_renderer_sessions_submit_the_final_source_value_and_release_each_iteration() {
    let _guard = HARNESS_LOCK.lock().unwrap();
    let mut criterion = criterion();
    let completed = Rc::new(Cell::new(0));
    criterion.bench_function("public-benchmark/renderer-session", |bencher| {
        let mut cx = BenchAppContext::new(
            bench_platform(None, Arc::new(NoopTextSystem)),
            Some("renderer-session"),
            bencher,
        );
        cx.update(|app| app.set_quit_mode(QuitMode::Explicit));
        let previous: RefCell<Option<(open_gpui::WeakEntity<SourceView>, Rc<Observations>)>> =
            RefCell::new(None);
        cx.bench_renderer_session(
            Duration::from_secs(5),
            |cx| {
                assert!(!profiler::trace_enabled());
                if let Some((weak, observed)) = previous.borrow().as_ref() {
                    assert!(weak.upgrade().is_none());
                    assert_eq!(observed.releases.get(), 1);
                }
                assert!(cx.read(|app| app.windows().is_empty()));
                let source = cx.new(|_| 0_usize);
                let observed = Rc::new(Observations::default());
                let mut window = cx.add_empty_window();
                let view = window
                    .replace_root_view(|_, cx| SourceView::new(source, Rc::clone(&observed), cx))
                    .unwrap();
                previous.replace(Some((view.downgrade(), observed)));
                let handle = window.window_handle();
                let weak_window = window.update(|window, app| window.to_async(app));
                drop(window);
                let stopped = Arc::new(AtomicBool::new(false));
                (
                    RendererSession {
                        view,
                        window: weak_window,
                        stopped: Arc::clone(&stopped),
                        inputs: 0,
                        completed: Rc::clone(&completed),
                    },
                    handle,
                    stopped,
                )
            },
            |session, turn, _, app| {
                assert!(profiler::trace_enabled());
                assert_eq!(turn as usize, session.inputs);
                session.inputs += 1;
                session.view.update(app, |view, cx| {
                    view.source.update(cx, |value, cx| {
                        *value += 1;
                        cx.notify();
                    });
                });
                if session.inputs == 3 {
                    session.stopped.store(true, Ordering::Release);
                }
            },
        );
        cx.settle();
        let previous = previous.borrow();
        let (weak, observed) = previous.as_ref().unwrap();
        assert!(weak.upgrade().is_none());
        assert_eq!(observed.releases.get(), 1);
        assert!(cx.read(|app| app.windows().is_empty()));
        cx.teardown();
    });
    assert!(completed.get() > 0);
}
