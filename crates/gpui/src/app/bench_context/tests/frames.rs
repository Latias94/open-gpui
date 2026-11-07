use super::*;
use crate::{IntoElement, PlatformWindowPresentOutcome, div, px, size};
use std::cell::Cell;

#[test]
fn benchmark_windows_activate_resize_and_submit_through_platform_frames() {
    run_benchmark("active-resized-benchmark-window", |mut cx| {
        let mut window = cx.add_empty_window();
        assert!(window.update(|window, _| window.is_window_active()));
        let initial = window.update(|window, _| window.presentation_facts());
        assert_eq!(
            initial.frame_accepted_generation,
            initial.present_submitted_generation
        );
        let viewport = size(px(480.0), px(320.0));
        window.simulate_resize(viewport);
        cx.run_until_idle();
        window.update(|window, _| {
            assert_eq!(window.viewport_size(), viewport);
            let resized = window.presentation_facts();
            assert!(resized.frame_accepted_generation > initial.frame_accepted_generation);
            assert_eq!(
                resized.frame_accepted_generation,
                resized.present_submitted_generation
            );
        });
        cx.bench_iter(|_| ());
        drop(window);
        cx.teardown();
    });
}

#[test]
fn frame_callbacks_wait_for_app_release_and_follow_earlier_resize_ingress() {
    run_benchmark("ordered-benchmark-frame-ingress", |mut cx| {
        let mut window = cx.add_empty_window();
        let mut native = platform_window(&cx, window.window_handle());
        let observed_viewport = Rc::new(Cell::new(None));
        window.update(|window, _| {
            let observed_viewport = observed_viewport.clone();
            window.on_next_frame(move |window, _| {
                observed_viewport.set(Some(window.viewport_size()));
            });
        });
        let viewport = size(px(510.0), px(340.0));
        let borrow = cx.app.borrow();
        native.simulate_resize(viewport);
        assert!(native.simulate_scheduled_frame());
        assert_eq!(observed_viewport.get(), None);
        drop(borrow);
        assert_eq!(observed_viewport.get(), Some(viewport));
        cx.bench_iter(|_| ());
        drop(window);
        cx.teardown();
    });
}

#[test]
fn run_until_checks_completion_before_delivering_an_unrelated_frame() {
    run_benchmark("benchmark-completion-before-frame", |mut cx| {
        let mut window = cx.add_empty_window();
        let initial = window.update(|window, _| window.presentation_facts());
        let completed = Rc::new(Cell::new(false));
        let mut task_context = cx.update(|app| app.to_async());
        let handle = window.window_handle();
        let task = cx.foreground_executor().spawn({
            let completed = completed.clone();
            async move {
                task_context
                    .update_window(handle, |_, window, _| window.refresh())
                    .expect("the benchmark window must remain open");
                completed.set(true);
            }
        });
        cx.run_until(|| completed.get().then_some(()));
        assert!(task.is_ready());
        let pending = window.update(|window, _| {
            assert!(window.invalidator.is_dirty());
            window.presentation_facts()
        });
        assert_eq!(
            pending.frame_accepted_generation,
            initial.frame_accepted_generation
        );
        cx.run_until_idle();
        let submitted = window.update(|window, _| window.presentation_facts());
        assert!(submitted.frame_accepted_generation > initial.frame_accepted_generation);
        assert_eq!(
            submitted.frame_accepted_generation,
            submitted.present_submitted_generation
        );
        drop(task);
        cx.bench_iter(|_| ());
        drop(window);
        cx.teardown();
    });
}

struct RendererProbe;

#[test]
fn frame_delivery_finishes_native_ingress_before_returning_to_the_measurement() {
    let iterations = Cell::new(0);
    let report = run_benchmark("benchmark-inline-frame-convergence", |mut cx| {
        let mut window = cx.add_empty_window();
        let native = platform_window(&cx, window.window_handle());
        cx.run_until_idle();
        cx.bench_iter(|cx| {
            let before = native.draw_count();
            window.update(|window, _| window.refresh());
            assert!(cx.dispatch_pending_frames(|| true));
            assert_eq!(
                native.draw_count(),
                before + 1,
                "native submission must finish before another App borrow triggers convergence"
            );
            iterations.set(iterations.get() + 1);
        });
        drop(window);
        cx.teardown();
    });
    assert!(iterations.get() > 0);
    assert_eq!(report.frame_snapshot.borrow().draw.len(), iterations.get());
}

impl Render for RendererProbe {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
    }
}

#[test]
fn renderer_reports_accepted_draws_without_promoting_rejected_presents() {
    let iterations = Cell::new(0);
    let report = run_benchmark("renderer-rejected-present", |mut cx| {
        let mut window = cx.add_empty_window();
        let view = window
            .replace_root_view(|_, _| RendererProbe)
            .expect("renderer root");
        cx.run_until_idle();
        let native = platform_window(&cx, window.window_handle());
        native.set_present_outcome(PlatformWindowPresentOutcome::Rejected);
        cx.bench_renderer(view, |_, window, _| {
            window.refresh();
            iterations.set(iterations.get() + 1);
        });
        let rejected = window.update(|window, _| window.presentation_facts());
        assert!(
            rejected.frame_accepted_generation > rejected.present_submitted_generation,
            "expected an accepted newer rejected scene: {rejected:?}, iterations={}, active={}, scheduled={}, dirty={}",
            iterations.get(),
            window.update(|window, _| window.is_window_active()),
            native.frame_scheduled(),
            window.update(|window, _| window.invalidator.is_dirty())
        );
        assert_eq!(
            rejected
                .latest_present_attempt
                .expect("the renderer must have been called")
                .outcome,
            PlatformWindowPresentOutcome::Rejected
        );
        native.set_present_outcome(PlatformWindowPresentOutcome::Submitted);
        cx.run_until_idle();
        let submitted = window.update(|window, _| window.presentation_facts());
        assert_eq!(
            submitted.frame_accepted_generation,
            rejected.frame_accepted_generation
        );
        assert_eq!(
            submitted.frame_accepted_generation,
            submitted.present_submitted_generation
        );
        drop(window);
        cx.teardown();
    });
    assert!(iterations.get() > 0);
    let snapshot = report.frame_snapshot.borrow();
    assert_eq!(snapshot.draw.len(), iterations.get());
    assert!(snapshot.present_interval.is_empty());
}

fn keep_animation_pending(window: &mut Window, frames: Rc<Cell<usize>>) {
    window.on_next_frame(move |window, _| {
        frames.set(frames.get() + 1);
        keep_animation_pending(window, frames);
    });
}

struct RendererSession {
    app: std::rc::Weak<AppCell>,
    window: AnyWindowHandle,
    completed: Rc<Cell<u64>>,
    frames: Rc<Cell<usize>>,
    inputs: usize,
}

impl Drop for RendererSession {
    fn drop(&mut self) {
        assert!(!profiler::trace_enabled());
        assert_eq!(self.inputs, 1);
        assert_eq!(self.frames.get(), 2);
        let app = self
            .app
            .upgrade()
            .expect("the session app must remain alive");
        app.borrow_mut()
            .update_window(self.window, |_, window, cx| {
                assert!(!window.invalidator.is_dirty());
                assert!(!window.needs_present.get());
                assert!(!window.next_frame_callbacks.borrow().is_empty());
                let facts = window.presentation_facts();
                assert!(facts.frame_accepted_generation.is_some());
                assert_eq!(
                    facts.frame_accepted_generation,
                    facts.present_submitted_generation
                );
                window.remove_window(cx);
            })
            .expect("the session window must remain open until destruction");
        self.completed.set(self.completed.get() + 1);
    }
}

#[test]
fn renderer_session_drives_final_submission_after_stop_without_waiting_for_animation() {
    let completed = Rc::new(Cell::new(0));
    let report = run_benchmark("renderer-session-final-submission", |mut cx| {
        cx.bench_renderer_session(
            Duration::from_secs(10),
            |cx| {
                assert!(!profiler::trace_enabled());
                let window = cx.add_empty_window();
                let handle = window.window_handle();
                drop(window);
                let stopped = Arc::new(AtomicBool::new(false));
                (
                    (
                        RendererSession {
                            app: Rc::downgrade(&cx.app),
                            window: handle,
                            completed: completed.clone(),
                            frames: Rc::new(Cell::new(0)),
                            inputs: 0,
                        },
                        stopped.clone(),
                    ),
                    handle,
                    stopped,
                )
            },
            |(state, stopped), turn, window, _| {
                assert_eq!(turn, 0);
                state.inputs += 1;
                window.refresh();
                let native = window
                    .platform_window
                    .as_test()
                    .expect("benchmark platform")
                    .clone();
                let frames = state.frames.clone();
                window.on_next_frame(move |window, _| {
                    frames.set(frames.get() + 1);
                    native.set_present_outcome(PlatformWindowPresentOutcome::Deferred);
                    window.on_next_frame(move |window, _| {
                        frames.set(frames.get() + 1);
                        native.set_present_outcome(PlatformWindowPresentOutcome::Submitted);
                        keep_animation_pending(window, frames);
                    });
                });
                stopped.store(true, Ordering::Release);
            },
        );
        cx.teardown();
    });
    assert!(completed.get() > 0);
    assert_eq!(report.frame_snapshot.borrow().draw.len(), completed.get());
}

#[test]
fn renderer_session_checks_the_deadline_between_polls() {
    let message = benchmark_panic("renderer-session-deadline", |cx| {
        cx.bench_renderer_session(
            Duration::ZERO,
            |cx| {
                let window = cx.add_empty_window();
                ((), window.window_handle(), Arc::new(AtomicBool::new(false)))
            },
            |(), _, _, _| panic!("input must not run after the deadline"),
        );
    });
    assert!(message.contains("renderer session did not stop"));
}

#[test]
fn renderer_session_reports_a_removed_window() {
    let message = benchmark_panic("renderer-session-removed-window", |cx| {
        cx.bench_renderer_session(
            Duration::from_secs(10),
            |cx| {
                let mut window = cx.add_empty_window();
                let handle = window.window_handle();
                window.update(|window, cx| window.remove_window(cx));
                ((), handle, Arc::new(AtomicBool::new(true)))
            },
            |(), _, _, _| panic!("input must not run for a removed window"),
        );
    });
    assert!(message.contains("renderer session window must remain open"));
}
