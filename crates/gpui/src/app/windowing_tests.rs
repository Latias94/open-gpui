use super::Application;
use crate::{
    AppContext as _, Context, Empty, GraphicalEnvironment, InteractiveElement as _, IntoElement,
    NativeBoundaryGeneration, NativeBoundaryKind, NativePlatformCommandKind, QuitMode, Render,
    Styled as _, TestAppContext, Window, WindowHandle, WindowingRequest, div, px, red,
    retained_visual, size,
};
use anyhow::Result;
use futures::{FutureExt as _, channel::oneshot};
use std::{
    cell::{Cell, RefCell},
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
};

struct PendingSwitch {
    validate: Box<dyn Fn() -> Result<()>>,
    completed: oneshot::Sender<Result<()>>,
}

fn defer_requests(cx: &TestAppContext) -> Rc<RefCell<Option<PendingSwitch>>> {
    let pending = Rc::new(RefCell::new(None));
    let captured = pending.clone();
    let executor = cx.foreground_executor.clone();
    cx.test_platform()
        .windowing_request_handler
        .replace(Some(Rc::new(move |_, validate| {
            let (completed, result) = oneshot::channel();
            assert!(
                captured
                    .borrow_mut()
                    .replace(PendingSwitch {
                        validate,
                        completed
                    })
                    .is_none()
            );
            executor.spawn(async move {
                result
                    .await
                    .expect("the simulated backend must settle its request")
            })
        })));
    pending
}

fn finish_pending(pending: &RefCell<Option<PendingSwitch>>, cx: &TestAppContext) -> bool {
    let PendingSwitch {
        validate,
        completed,
    } = pending.borrow_mut().take().unwrap();
    let result = validate();
    let accepted = result.is_ok();
    completed.send(result).ok();
    cx.run_until_parked();
    accepted
}

#[crate::test]
fn initial_windowing_is_forwarded_before_launch(cx: &mut TestAppContext) {
    let application = Application(cx.app.clone())
        .with_windowing(WindowingRequest::Headless)
        .with_windowing(WindowingRequest::Windowed(GraphicalEnvironment::detect()));
    let requests = cx.test_platform().initial_windowing_requests.borrow();
    assert!(matches!(
        requests.as_slice(),
        [WindowingRequest::Headless, WindowingRequest::Windowed(_)]
    ));
    drop(requests);
    drop(application);
}

#[crate::test]
fn open_windows_refuse_switching_before_backend_admission(cx: &mut TestAppContext) {
    cx.update(|app| app.set_quit_mode(QuitMode::Explicit));
    let pending = defer_requests(cx);
    let window = cx.open_window(size(px(160.0), px(100.0)), |_, _| Empty);
    let request = cx.read(|app| app.request_windowing(WindowingRequest::Headless));
    assert!(
        request
            .now_or_never()
            .unwrap()
            .unwrap_err()
            .to_string()
            .contains("windows are open")
    );
    assert!(pending.borrow().is_none());
    window
        .update(cx, |_, window, app| window.remove_window(app))
        .unwrap();
    cx.run_until_parked();
}

#[crate::test]
fn queued_switch_revalidates_a_window_opened_after_admission(cx: &mut TestAppContext) {
    cx.update(|app| app.set_quit_mode(QuitMode::Explicit));
    let pending = defer_requests(cx);
    let request = cx.read(|app| app.request_windowing(WindowingRequest::Headless));
    let window = cx.open_window(size(px(160.0), px(100.0)), |_, _| Empty);
    assert!(!finish_pending(&pending, cx));
    assert!(
        request
            .now_or_never()
            .unwrap()
            .unwrap_err()
            .to_string()
            .contains("windows are open")
    );
    window
        .update(cx, |_, window, app| window.remove_window(app))
        .unwrap();
    cx.run_until_parked();
    let retry = cx.read(|app| app.request_windowing(WindowingRequest::Headless));
    assert!(finish_pending(&pending, cx));
    assert!(retry.now_or_never().unwrap().is_ok());
}

#[crate::test]
fn closed_window_native_retirement_blocks_switch_until_its_exact_terminal(cx: &mut TestAppContext) {
    cx.update(|app| app.set_quit_mode(QuitMode::Explicit));
    let pending = defer_requests(cx);
    let window = cx.open_window(size(px(160.0), px(100.0)), |_, _| Empty);
    let terminal = cx.hold_window_native_terminal(window.into());
    window
        .update(cx, |_, window, app| window.remove_window(app))
        .unwrap();
    cx.run_until_parked();
    assert!(cx.windows().is_empty());

    let request = cx.read(|app| app.request_windowing(WindowingRequest::Headless));
    assert!(
        request
            .now_or_never()
            .unwrap()
            .unwrap_err()
            .to_string()
            .contains("resources are retiring")
    );
    assert!(pending.borrow().is_none());

    assert!(terminal.release());
    cx.run_until_parked();
    let retry = cx.read(|app| app.request_windowing(WindowingRequest::Headless));
    assert!(finish_pending(&pending, cx));
    assert!(retry.now_or_never().unwrap().is_ok());
}

#[crate::test]
fn queued_switch_revalidates_retirement_after_the_logical_window_disappears(
    cx: &mut TestAppContext,
) {
    cx.update(|app| app.set_quit_mode(QuitMode::Explicit));
    let pending = defer_requests(cx);
    let request = cx.read(|app| app.request_windowing(WindowingRequest::Headless));
    let window = cx.open_window(size(px(160.0), px(100.0)), |_, _| Empty);
    let terminal = cx.hold_window_native_terminal(window.into());
    window
        .update(cx, |_, window, app| window.remove_window(app))
        .unwrap();
    cx.run_until_parked();
    assert!(cx.windows().is_empty());
    assert!(!finish_pending(&pending, cx));
    assert!(request.now_or_never().unwrap().is_err());
    assert!(terminal.release());
    cx.run_until_parked();
}

#[crate::test]
fn dropping_a_task_does_not_withdraw_an_admitted_windowing_request(cx: &mut TestAppContext) {
    let pending = defer_requests(cx);
    let request = cx.read(|app| app.request_windowing(WindowingRequest::Headless));
    drop(request);
    assert!(pending.borrow().is_some());
    assert!(finish_pending(&pending, cx));
}

#[crate::test]
fn queued_switch_refuses_a_later_shutdown_without_replacing_application_state(
    cx: &mut TestAppContext,
) {
    let pending = defer_requests(cx);
    let request = cx.read(|app| app.request_windowing(WindowingRequest::Headless));
    cx.update(|app| app.shutdown());
    cx.run_until_parked();
    assert!(!finish_pending(&pending, cx));
    assert!(
        request
            .now_or_never()
            .unwrap()
            .unwrap_err()
            .to_string()
            .contains("shutdown")
    );
}

#[crate::test]
fn a_queued_switch_does_not_retain_the_application_owner(cx: &mut TestAppContext) {
    let other = cx.new_app();
    let pending = defer_requests(&other);
    let request = other.read(|app| app.request_windowing(WindowingRequest::Headless));
    let lifetime = Rc::downgrade(&other.app);
    drop(other);
    cx.run_until_parked();
    assert!(lifetime.upgrade().is_none());
    assert!(!finish_pending(&pending, cx));
    assert!(
        request
            .now_or_never()
            .unwrap()
            .unwrap_err()
            .to_string()
            .contains("released")
    );
}

#[crate::test]
fn retained_application_entities_survive_the_mode_request(cx: &mut TestAppContext) {
    let pending = defer_requests(cx);
    let updates = Rc::new(Cell::new(0));
    let entity = cx.new({
        let updates = updates.clone();
        move |_| updates
    });
    let request = cx.read(|app| app.request_windowing(WindowingRequest::Headless));
    cx.update(|app| entity.update(app, |value, _| value.set(1)));
    assert!(finish_pending(&pending, cx));
    assert!(request.now_or_never().unwrap().is_ok());
    assert_eq!(updates.get(), 1);
    assert_eq!(cx.read(|app| entity.read(app).get()), 1);
}

#[crate::test]
fn close_observer_cannot_switch_during_native_retirement_handoff(cx: &mut TestAppContext) {
    cx.update(|app| app.set_quit_mode(QuitMode::Explicit));
    let pending = defer_requests(cx);
    let request = cx.read(|app| app.request_windowing(WindowingRequest::Headless));
    let window = cx.open_window(size(px(160.0), px(100.0)), |_, _| Empty);
    let terminal = cx.hold_window_native_terminal(window.into());
    let callbacks = Rc::new(Cell::new(0));
    let _subscription = cx.update(|app| {
        app.on_window_closed({
            let pending = pending.clone();
            let callbacks = callbacks.clone();
            move |app, closed| {
                assert_eq!(closed, window.window_id());
                assert!(app.windows().is_empty());
                assert!(
                    (pending.borrow().as_ref().unwrap().validate)().is_err(),
                    "native retirement custody must already exist before close observers run",
                );
                let reentrant = app.request_windowing(WindowingRequest::Headless);
                assert!(reentrant.now_or_never().unwrap().is_err());
                callbacks.set(callbacks.get() + 1);
            }
        })
    });

    window
        .update(cx, |_, window, app| window.remove_window(app))
        .unwrap();
    cx.run_until_parked();
    assert_eq!(callbacks.get(), 1);
    assert!(terminal.release());
    cx.run_until_parked();
    assert!(finish_pending(&pending, cx));
    assert!(request.now_or_never().unwrap().is_ok());
}

struct PreparationPanicResource(Rc<Cell<usize>>);

impl Drop for PreparationPanicResource {
    fn drop(&mut self) {
        self.0.set(self.0.get() + 1);
        if !std::thread::panicking() {
            panic!("injected retained resource preparation panic");
        }
    }
}

struct PreparationPanicView {
    source: retained_visual::SourceId,
    resource: Option<Rc<PreparationPanicResource>>,
}

impl Render for PreparationPanicView {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        match &self.resource {
            Some(resource) => retained_visual::source(
                self.source.clone(),
                div()
                    .size_full()
                    .bg(red())
                    .retain_for_frame(resource.clone()),
            )
            .into_any_element(),
            None => div().size_full().into_any_element(),
        }
    }
}

/// Retires both frame copies so only a real visual lease owns the panic resource.
fn window_with_panicking_retained_visual(
    cx: &mut TestAppContext,
    resource_drops: Rc<Cell<usize>>,
) -> WindowHandle<PreparationPanicView> {
    let source = retained_visual::SourceId::new("retirement-preparation-panic");
    let window = cx.open_window(size(px(160.0), px(100.0)), |_, _| PreparationPanicView {
        source: source.clone(),
        resource: Some(Rc::new(PreparationPanicResource(resource_drops.clone()))),
    });
    window
        .update(cx, |view, window, app| {
            retained_visual::lease_committed(window, &source)
                .expect("the actual rendered source must retain its resource");
            view.resource = None;
            app.notify();
        })
        .unwrap();
    cx.update_window(window.into(), |_, window, app| {
        window.draw(app).clear(app);
        window.draw(app).clear(app);
    })
    .unwrap();
    assert_eq!(resource_drops.get(), 0);
    window
}

fn panic_message(payload: &(dyn std::any::Any + Send)) -> Option<&str> {
    payload
        .downcast_ref::<&str>()
        .copied()
        .or_else(|| payload.downcast_ref::<String>().map(String::as_str))
}

/// Exercises retirement handoff with an actual Window whose ticket has not been prepared.
#[crate::test]
fn retirement_preparation_panic_retains_owner_through_retry_and_native_terminal(
    cx: &mut TestAppContext,
) {
    cx.update(|app| app.set_quit_mode(QuitMode::Explicit));
    let pending = defer_requests(cx);
    let request = cx.read(|app| app.request_windowing(WindowingRequest::Headless));
    let resource_drops = Rc::new(Cell::new(0));
    let window = window_with_panicking_retained_visual(cx, resource_drops.clone());
    let platform_window = cx.test_window(window.into());
    let terminal = cx.hold_window_native_terminal(window.into());
    let native_drops = Rc::new(Cell::new(0));
    platform_window.set_native_drop_callback({
        let native_drops = native_drops.clone();
        move || native_drops.set(native_drops.get() + 1)
    });
    let observations = Rc::new(Cell::new(0));
    let _subscription = cx.update(|app| {
        app.on_window_closed({
            let pending = pending.clone();
            let resource_drops = resource_drops.clone();
            let native_drops = native_drops.clone();
            let observations = observations.clone();
            move |app, closed| {
                assert_eq!(closed, window.window_id());
                assert!(app.windows().is_empty());
                assert_eq!(resource_drops.get(), 1);
                assert_eq!(
                    native_drops.get(),
                    0,
                    "preparation unwind must retain the native owner"
                );
                assert!((pending.borrow().as_ref().unwrap().validate)().is_err());
                assert!(
                    app.request_windowing(WindowingRequest::Headless)
                        .now_or_never()
                        .unwrap()
                        .is_err()
                );
                observations.set(observations.get() + 1);
            }
        })
    });

    let mut app = cx.app.borrow_mut();
    let panic = catch_unwind(AssertUnwindSafe(|| {
        window
            .update(&mut *app, |_, window, _| window.removed = true)
            .unwrap();
    }))
    .expect_err("the retained resource must panic during retirement preparation");
    assert_eq!(
        panic_message(panic.as_ref()),
        Some("injected retained resource preparation panic")
    );
    assert_eq!(observations.get(), 1);
    assert_eq!(native_drops.get(), 0);
    assert_eq!(platform_window.presentation_shutdown_counts(), (0, 0, 0));
    let before_retry = app.native_boundary_diagnostics(Default::default());
    let unprepared = before_retry
        .pending
        .iter()
        .find(|diagnostic| {
            diagnostic.kind
                == NativeBoundaryKind::Command(NativePlatformCommandKind::RetireNativeWindow)
        })
        .expect("the existing native queue must own the unprepared retirement");
    assert_eq!(
        unprepared.domain_generation, None,
        "an unprepared retirement must not invent a ticket"
    );
    drop(app);
    cx.run_until_parked();

    assert_eq!(resource_drops.get(), 1);
    assert_eq!(native_drops.get(), 1);
    assert_eq!(platform_window.presentation_shutdown_counts(), (1, 1, 1));
    assert!((pending.borrow().as_ref().unwrap().validate)().is_err());
    let after_retry = cx.read(|app| app.native_boundary_diagnostics(Default::default()));
    assert!(
        after_retry.terminal.iter().any(|diagnostic| {
            diagnostic.sequence == unprepared.sequence
                && matches!(
                    diagnostic.domain_generation,
                    Some(NativeBoundaryGeneration::PresentationShutdown(_))
                )
        }),
        "successful preparation must attach the real ticket to the retirement terminal"
    );
    assert!(terminal.release());
    cx.run_until_parked();
    assert!(finish_pending(&pending, cx));
    assert!(request.now_or_never().unwrap().is_ok());
}

#[crate::test]
fn close_observer_panic_wins_over_retirement_preparation_panic(cx: &mut TestAppContext) {
    cx.update(|app| app.set_quit_mode(QuitMode::Explicit));
    let resource_drops = Rc::new(Cell::new(0));
    let window = window_with_panicking_retained_visual(cx, resource_drops.clone());
    let platform_window = cx.test_window(window.into());
    let terminal = cx.hold_window_native_terminal(window.into());
    let observations = Rc::new(Cell::new(0));
    let _first =
        cx.update(|app| app.on_window_closed(|_, _| panic!("primary close observer panic")));
    let _second = cx.update(|app| {
        app.on_window_closed({
            let observations = observations.clone();
            move |_, _| observations.set(observations.get() + 1)
        })
    });

    let mut app = cx.app.borrow_mut();
    let panic = catch_unwind(AssertUnwindSafe(|| {
        window
            .update(&mut *app, |_, window, _| window.removed = true)
            .unwrap();
    }))
    .expect_err("the first observer panic must remain observable");
    assert_eq!(
        panic_message(panic.as_ref()),
        Some("primary close observer panic")
    );
    assert_eq!(observations.get(), 1);
    assert_eq!(resource_drops.get(), 1);
    assert!(!platform_window.is_native_terminal());
    drop(app);
    cx.run_until_parked();
    assert_eq!(platform_window.presentation_shutdown_counts(), (1, 1, 1));
    assert!(terminal.release());
    cx.run_until_parked();
}

#[crate::test]
fn window_removal_panic_wins_over_observer_after_retirement_preparation_retry(
    cx: &mut TestAppContext,
) {
    cx.update(|app| app.set_quit_mode(QuitMode::Explicit));
    let resource_drops = Rc::new(Cell::new(0));
    let window = window_with_panicking_retained_visual(cx, resource_drops.clone());
    let platform_window = cx.test_window(window.into());
    let terminal = cx.hold_window_native_terminal(window.into());
    let observations = Rc::new(Cell::new(0));
    let _first =
        cx.update(|app| app.on_window_closed(|_, _| panic!("secondary close observer panic")));
    let _second = cx.update(|app| {
        app.on_window_closed({
            let observations = observations.clone();
            move |_, _| observations.set(observations.get() + 1)
        })
    });

    let mut app = cx.app.borrow_mut();
    let panic = catch_unwind(AssertUnwindSafe(|| {
        window
            .update(&mut *app, |_, window, app| window.remove_window(app))
            .unwrap();
    }))
    .expect_err("the actual removal callback's panic must remain primary");
    assert_eq!(
        panic_message(panic.as_ref()),
        Some("injected retained resource preparation panic")
    );
    assert_eq!(observations.get(), 1);
    assert_eq!(resource_drops.get(), 1);
    assert!(!platform_window.is_native_terminal());
    drop(app);
    cx.run_until_parked();
    assert_eq!(platform_window.presentation_shutdown_counts(), (1, 1, 1));
    assert!(terminal.release());
    cx.run_until_parked();
}

#[crate::test]
fn unprepared_retirement_retries_before_dependency_and_shutdown_completion(
    cx: &mut TestAppContext,
) {
    assert_unprepared_retirement_waits_for_shutdown(cx, false);
}

#[crate::test]
fn unprepared_retirement_retries_before_dependency_and_terminal_shutdown(cx: &mut TestAppContext) {
    assert_unprepared_retirement_waits_for_shutdown(cx, true);
}

fn assert_unprepared_retirement_waits_for_shutdown(
    cx: &mut TestAppContext,
    terminate_ingress: bool,
) {
    cx.update(|app| app.set_quit_mode(QuitMode::Explicit));
    let resource_drops = Rc::new(Cell::new(0));
    let anchor = window_with_panicking_retained_visual(cx, resource_drops.clone());
    let dependent = cx.open_window(size(px(160.0), px(100.0)), |_, _| Empty);
    let anchor_platform = cx.test_window(anchor.into());
    let dependent_platform = cx.test_window(dependent.into());
    let anchor_terminal = cx.hold_window_native_terminal(anchor.into());
    let dependent_terminal = cx.hold_window_native_terminal(dependent.into());
    cx.update(|app| {
        app.register_native_window_retirement_dependencies(
            anchor.window_id(),
            [dependent.window_id()],
        )
        .unwrap()
    });
    let _subscription = cx.update(|app| {
        app.on_window_closed(move |app, closed| {
            if closed == anchor.window_id() {
                if terminate_ingress {
                    app.shutdown_for_native_exit_test();
                } else {
                    app.shutdown();
                }
            }
        })
    });

    let mut app = cx.app.borrow_mut();
    let panic = catch_unwind(AssertUnwindSafe(|| {
        anchor
            .update(&mut *app, |_, window, _| window.removed = true)
            .unwrap();
    }))
    .expect_err("the anchor's actual retained resource must fail preparation once");
    assert_eq!(
        panic_message(panic.as_ref()),
        Some("injected retained resource preparation panic")
    );
    assert!(!anchor_platform.is_native_terminal());
    drop(app);
    cx.run_until_parked();

    assert_eq!(resource_drops.get(), 1);
    let (prepared, quiesced, retired) = anchor_platform.presentation_shutdown_counts();
    assert_eq!(prepared, 1);
    assert!(
        quiesced > 0,
        "shutdown must claim the late-prepared presentation authority"
    );
    assert_eq!(
        retired, 0,
        "preparation retry must not skip the native dependency"
    );
    assert_eq!(dependent_platform.presentation_shutdown_counts().2, 1);
    assert!(
        cx.update(|app| app.open_window_detailed(Default::default(), |_, app| app.new(|_| Empty)))
            .is_err(),
        "shutdown must retain its window-open barrier until both terminals"
    );

    assert!(dependent_terminal.release());
    cx.run_until_parked();
    assert_eq!(anchor_platform.presentation_shutdown_counts().2, 1);
    assert!(
        cx.update(|app| app.open_window_detailed(Default::default(), |_, app| app.new(|_| Empty)))
            .is_err()
    );
    assert!(anchor_terminal.release());
    cx.run_until_parked();
    if terminate_ingress {
        assert!(cx.update(|app| app.native_exit_authority_is_settled_for_test()));
    } else {
        let replacement = cx.open_window(size(px(160.0), px(100.0)), |_, _| Empty);
        replacement
            .update(cx, |_, window, app| window.remove_window(app))
            .unwrap();
        cx.run_until_parked();
    }
}
