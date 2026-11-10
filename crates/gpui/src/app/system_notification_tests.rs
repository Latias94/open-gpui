use super::{Application, native_event_ingress::NativeAppEvent};
use crate::{
    NativeBoundaryDiagnosticCursor, NativeBoundaryDisposition, NativeBoundaryKind,
    NativeCallbackKind, NativeInvariantFailure, SystemNotification, SystemNotificationAction,
    SystemNotificationResponse, TestAppContext, TestPlatform,
};
use std::{
    cell::{Cell, RefCell},
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
};

#[crate::test]
async fn test_system_notifications_require_identity_and_replace_matching_tags(
    cx: &mut TestAppContext,
) {
    cx.update(|cx| {
        cx.show_system_notification(SystemNotification {
            tag: "thread-1".into(),
            title: "Task started".into(),
            body: "Running tests".into(),
            actions: Vec::new(),
        });
    });
    assert!(cx.shown_system_notifications().is_empty());
    assert!(cx.delivered_system_notifications().is_empty());

    cx.update(|cx| {
        cx.set_app_identity("com.example.tasks", "Tasks");
        cx.show_system_notification(SystemNotification {
            tag: "thread-1".into(),
            title: "Task started".into(),
            body: "Running tests".into(),
            actions: Vec::new(),
        });
        cx.show_system_notification(SystemNotification {
            tag: "thread-1".into(),
            title: "Task finished".into(),
            body: "All tests passed".into(),
            actions: vec![SystemNotificationAction {
                id: "open".into(),
                label: "Open".into(),
            }],
        });
    });
    assert_eq!(
        cx.app_identity(),
        Some(("com.example.tasks".into(), "Tasks".into()))
    );
    assert_eq!(cx.shown_system_notifications().len(), 2);
    assert_eq!(
        cx.delivered_system_notifications(),
        [SystemNotification {
            tag: "thread-1".into(),
            title: "Task finished".into(),
            body: "All tests passed".into(),
            actions: vec![SystemNotificationAction {
                id: "open".into(),
                label: "Open".into()
            }],
        }]
    );
    cx.update(|cx| cx.dismiss_system_notification("thread-1"));
    assert!(cx.delivered_system_notifications().is_empty());
    assert_eq!(cx.dismissed_system_notifications(), ["thread-1"]);
}

#[crate::test]
async fn test_system_notification_body_and_action_responses(cx: &mut TestAppContext) {
    let responses = Rc::new(RefCell::new(Vec::new()));
    cx.update(|cx| {
        cx.on_system_notification_response({
            let responses = responses.clone();
            move |response, _cx| responses.borrow_mut().push(response)
        });
    });
    cx.simulate_system_notification_response(SystemNotificationResponse {
        tag: "thread-1".into(),
        action_id: None,
    });
    cx.simulate_system_notification_response(SystemNotificationResponse {
        tag: "thread-1".into(),
        action_id: Some("default".into()),
    });
    assert_eq!(
        responses.borrow().as_slice(),
        &[
            SystemNotificationResponse {
                tag: "thread-1".into(),
                action_id: None
            },
            SystemNotificationResponse {
                tag: "thread-1".into(),
                action_id: Some("default".into())
            },
        ]
    );
}

#[crate::test]
async fn test_system_notification_response_handler_can_be_replaced(cx: &mut TestAppContext) {
    let first_responses = Rc::new(RefCell::new(Vec::new()));
    let second_responses = Rc::new(RefCell::new(Vec::new()));
    cx.update(|cx| {
        cx.on_system_notification_response({
            let first_responses = first_responses.clone();
            move |response, _cx| first_responses.borrow_mut().push(response)
        });
        cx.on_system_notification_response({
            let second_responses = second_responses.clone();
            move |response, _cx| second_responses.borrow_mut().push(response)
        });
    });
    let response = SystemNotificationResponse {
        tag: "thread-1".into(),
        action_id: None,
    };
    cx.simulate_system_notification_response(response.clone());
    assert!(first_responses.borrow().is_empty());
    assert_eq!(second_responses.borrow().as_slice(), &[response]);
}

#[crate::test]
async fn test_system_notification_response_handler_can_reenter_app(cx: &mut TestAppContext) {
    cx.update(|cx| {
        cx.set_app_identity("com.example.tasks", "Tasks");
        cx.show_system_notification(SystemNotification {
            tag: "thread-1".into(),
            title: "Task finished".into(),
            body: "All tests passed".into(),
            actions: Vec::new(),
        });
        cx.on_system_notification_response(|response, cx| {
            cx.dismiss_system_notification(&response.tag);
        });
    });
    cx.simulate_system_notification_response(SystemNotificationResponse {
        tag: "thread-1".into(),
        action_id: None,
    });
    assert!(cx.delivered_system_notifications().is_empty());
    assert_eq!(cx.dismissed_system_notifications(), ["thread-1"]);
}

fn response(tag: &'static str) -> SystemNotificationResponse {
    SystemNotificationResponse {
        tag: tag.into(),
        action_id: None,
    }
}

#[crate::test]
fn notification_simulation_while_borrowed_does_not_run_other_app_tasks(cx: &mut TestAppContext) {
    let delivered = Rc::new(Cell::new(0));
    cx.update(|app| {
        app.on_system_notification_response({
            let delivered = delivered.clone();
            move |_, _| delivered.set(delivered.get() + 1)
        })
    });
    let task_ran = Rc::new(Cell::new(false));
    let queued = cx.clone();
    let completed = task_ran.clone();
    cx.foreground_executor()
        .spawn(async move {
            queued.update(|_| completed.set(true));
        })
        .detach();
    cx.update(|_| {
        cx.simulate_system_notification_response(response("borrowed"));
        assert_eq!(delivered.get(), 0);
        assert!(!task_ran.get());
    });
    cx.run_until_parked();
    assert_eq!(delivered.get(), 1);
    assert!(task_ran.get());
}

#[crate::test]
fn busy_app_notification_responses_keep_native_order_and_flush_effects(cx: &mut TestAppContext) {
    let observed = Rc::new(RefCell::new(Vec::new()));
    cx.update(|app| {
        app.on_system_notification_response({
            let observed = observed.clone();
            move |response, app| {
                observed.borrow_mut().push(response.tag.to_string());
                let observed = observed.clone();
                app.defer(move |_| observed.borrow_mut().push("effect".into()));
            }
        })
    });
    Application(cx.app.clone()).on_reopen({
        let observed = observed.clone();
        move |_| observed.borrow_mut().push("reopen".into())
    });
    cx.update(|_| {
        cx.test_platform()
            .simulate_system_notification_response(response("first"));
        cx.test_platform().simulate_reopen();
        cx.test_platform()
            .simulate_system_notification_response(response("last"));
        assert!(observed.borrow().is_empty());
    });
    cx.run_until_parked();
    assert_eq!(
        observed.take(),
        ["first", "effect", "reopen", "last", "effect"]
    );
    let diagnostics =
        cx.read(|app| app.native_boundary_diagnostics(NativeBoundaryDiagnosticCursor::default()));
    assert!(diagnostics.pending.is_empty());
    let mut responses = diagnostics
        .terminal
        .iter()
        .filter(|entry| {
            entry.kind
                == NativeBoundaryKind::Callback(NativeCallbackKind::SystemNotificationResponse)
        })
        .collect::<Vec<_>>();
    responses.sort_by_key(|entry| entry.sequence);
    assert_eq!(responses.len(), 2);
    assert!(
        responses
            .iter()
            .all(|entry| entry.disposition == NativeBoundaryDisposition::DELIVERED)
    );
    assert!(responses[0].sequence < responses[1].sequence);
}

#[crate::test]
fn reentrant_notification_replacement_receives_queued_responses_without_restoring_old_handler(
    cx: &mut TestAppContext,
) {
    let observed = Rc::new(RefCell::new(Vec::new()));
    cx.update(|app| {
        app.on_system_notification_response({
            let observed = observed.clone();
            move |activation, app| {
                observed
                    .borrow_mut()
                    .push(format!("old:{}", activation.tag));
                app.on_system_notification_response({
                    let observed = observed.clone();
                    move |response, _| observed.borrow_mut().push(format!("new:{}", response.tag))
                });
                app.this.upgrade().unwrap().enqueue_native_app_event(
                    NativeAppEvent::SystemNotificationResponse(response("nested")),
                );
            }
        })
    });
    cx.simulate_system_notification_response(response("first"));
    cx.simulate_system_notification_response(response("last"));
    assert_eq!(observed.take(), ["old:first", "new:nested", "new:last"]);
}

#[crate::test]
fn notification_handler_checkout_survives_unwind(cx: &mut TestAppContext) {
    let calls = Rc::new(Cell::new(0));
    cx.update(|app| {
        app.on_system_notification_response({
            let calls = calls.clone();
            move |_, _| {
                calls.set(calls.get() + 1);
                assert_ne!(calls.get(), 1, "injected notification handler panic");
            }
        })
    });
    assert!(
        catch_unwind(AssertUnwindSafe(
            || cx.simulate_system_notification_response(response("first"))
        ))
        .is_err()
    );
    cx.simulate_system_notification_response(response("second"));
    assert_eq!(calls.get(), 2);
    let diagnostics =
        cx.read(|app| app.native_boundary_diagnostics(NativeBoundaryDiagnosticCursor::default()));
    assert!(diagnostics.pending.is_empty());
    let mut responses = diagnostics
        .terminal
        .iter()
        .filter(|entry| {
            entry.kind
                == NativeBoundaryKind::Callback(NativeCallbackKind::SystemNotificationResponse)
        })
        .collect::<Vec<_>>();
    responses.sort_by_key(|entry| entry.sequence);
    assert_eq!(responses.len(), 2);
    assert_eq!(
        responses[0].disposition,
        NativeBoundaryDisposition::InvariantFailure(NativeInvariantFailure::CallbackPanicked)
    );
    assert_eq!(
        responses[1].disposition,
        NativeBoundaryDisposition::DELIVERED
    );
}

#[crate::test]
fn notification_response_shutdown_retires_later_and_new_responses(cx: &mut TestAppContext) {
    let seen = Rc::new(RefCell::new(Vec::new()));
    cx.update(|app| {
        app.on_system_notification_response({
            let seen = seen.clone();
            move |response, app| {
                seen.borrow_mut().push(response.tag);
                app.shutdown_for_native_exit_test();
            }
        })
    });
    cx.update(|_| {
        cx.test_platform()
            .simulate_system_notification_response(response("first"));
        cx.test_platform()
            .simulate_system_notification_response(response("queued"));
    });
    cx.run_until_parked();
    cx.simulate_system_notification_response(response("late"));
    assert_eq!(seen.take(), ["first"]);
    let diagnostics =
        cx.read(|app| app.native_boundary_diagnostics(NativeBoundaryDiagnosticCursor::default()));
    assert!(diagnostics.pending.is_empty());
    let mut responses = diagnostics
        .terminal
        .iter()
        .filter(|entry| {
            entry.kind
                == NativeBoundaryKind::Callback(NativeCallbackKind::SystemNotificationResponse)
        })
        .collect::<Vec<_>>();
    responses.sort_by_key(|entry| entry.sequence);
    assert_eq!(responses.len(), 3);
    assert_eq!(
        responses[0].disposition,
        NativeBoundaryDisposition::DELIVERED
    );
    assert!(
        responses[1..]
            .iter()
            .all(|entry| entry.disposition == NativeBoundaryDisposition::Closed)
    );
}

#[crate::test]
fn retained_platform_notification_callback_does_not_retain_app_or_handler(cx: &mut TestAppContext) {
    let platform = TestPlatform::new(
        cx.background_executor.clone(),
        cx.foreground_executor.clone(),
    );
    let application = Application::with_platform(platform.clone());
    let weak_app = Rc::downgrade(&application.0);
    let capture = Rc::new(Cell::new(0));
    let weak_capture = Rc::downgrade(&capture);
    application
        .0
        .borrow()
        .on_system_notification_response(move |_, _| capture.set(capture.get() + 1));
    application.0.borrow_mut().shutdown();
    drop(application);
    assert!(weak_app.upgrade().is_none());
    assert!(weak_capture.upgrade().is_none());
    platform.simulate_system_notification_response(response("after-release"));
    cx.run_until_parked();
    assert!(weak_app.upgrade().is_none());
}
