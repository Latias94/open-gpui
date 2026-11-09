//! Crate-external notification workflows over the public TestAppContext and its CPU TestPlatform.
//! These tests do not establish operating-system delivery, authorization or native presentation.

#![cfg(feature = "test-support")]

use open_gpui::{
    App, AppContext as _, Context, Entity, IntoElement, QuitMode, Render, Styled as _,
    SystemNotification, SystemNotificationAction, SystemNotificationResponse, TestAppContext,
    Window, div, px, rgb, size,
};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

fn notification(tag: &str, title: &str) -> SystemNotification {
    SystemNotification {
        tag: tag.to_owned().into(),
        title: title.to_owned().into(),
        body: "A downstream build completed.".into(),
        actions: vec![SystemNotificationAction {
            id: "open".into(),
            label: "Open build".into(),
        }],
    }
}

#[derive(Default)]
struct ViewObservations {
    renders: Cell<usize>,
    drops: Cell<usize>,
}

struct NotificationInbox {
    responses: Vec<SystemNotificationResponse>,
    observed: Rc<ViewObservations>,
}

impl Render for NotificationInbox {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        self.observed.renders.set(self.observed.renders.get() + 1);
        div().size_full().bg(if self.responses.is_empty() {
            rgb(0x183048)
        } else {
            rgb(0x306030)
        })
    }
}

impl Drop for NotificationInbox {
    fn drop(&mut self) {
        self.observed.drops.set(self.observed.drops.get() + 1);
    }
}

fn route_responses(app: &App, inbox: &Entity<NotificationInbox>) {
    let inbox = inbox.downgrade();
    app.on_system_notification_response(move |response, app| {
        if response.action_id.as_deref() == Some("open") {
            app.dismiss_system_notification(&response.tag);
        }
        inbox
            .update(app, |inbox, cx| {
                inbox.responses.push(response);
                cx.notify();
            })
            .expect("the current response handler must target the live inbox");
    });
}

#[test]
fn public_test_platform_workflow_notifies_and_reconstructs_the_consuming_view() {
    let mut cx = TestAppContext::single();
    let weak_app = Rc::downgrade(&cx.app);
    let observed = Rc::new(ViewObservations::default());
    let snapshots = Rc::new(RefCell::new(Vec::new()));
    assert_eq!(cx.app_identity(), None);
    cx.read(|app| app.set_app_identity("org.open-gpui.consumer.builds", "Build consumer"));
    cx.update(|app| app.set_quit_mode(QuitMode::Explicit));
    assert_eq!(
        cx.app_identity(),
        Some((
            "org.open-gpui.consumer.builds".into(),
            "Build consumer".into()
        ))
    );

    let window = cx.open_window(size(px(160.), px(100.)), {
        let observed = Rc::clone(&observed);
        move |_, _| NotificationInbox {
            responses: Vec::new(),
            observed,
        }
    });
    let original = window.entity(&cx).expect("the window must own its inbox");
    let weak_original = original.downgrade();
    let original_subscription = cx.update(|app| {
        route_responses(app, &original);
        let snapshots = Rc::clone(&snapshots);
        app.observe(&original, move |inbox, app| {
            snapshots
                .borrow_mut()
                .push(inbox.read(app).responses.clone());
        })
    });
    cx.run_until_parked();

    let started = notification("build:foreground", "Build started");
    let background = notification("build:background", "Another build completed");
    let finished = notification("build:foreground", "Build completed");
    cx.read(|app| {
        app.show_system_notification(started.clone());
        app.show_system_notification(background.clone());
        app.show_system_notification(finished.clone());
    });
    assert_eq!(
        cx.shown_system_notifications(),
        [started, background.clone(), finished.clone()]
    );
    assert_eq!(
        cx.delivered_system_notifications(),
        [finished, background.clone()]
    );

    let body = SystemNotificationResponse {
        tag: "build:foreground".into(),
        action_id: None,
    };
    let renders = observed.renders.get();
    {
        let simulator = cx.clone();
        cx.update(|app| {
            simulator.simulate_system_notification_response(body.clone());
            assert!(original.read(app).responses.is_empty());
            assert!(snapshots.borrow().is_empty());
        });
    }
    cx.run_until_parked();
    assert_eq!(
        original.read_with(&cx, |inbox, _| inbox.responses.clone()),
        [body.clone()]
    );
    assert_eq!(snapshots.borrow().as_slice(), [vec![body.clone()]]);
    assert!(observed.renders.get() > renders);
    assert!(cx.dismissed_system_notifications().is_empty());

    let replacement = cx
        .update_window(window.into(), |_, window, app| {
            let replacement = window.replace_root(app, |_, _| NotificationInbox {
                responses: Vec::new(),
                observed: Rc::clone(&observed),
            });
            route_responses(app, &replacement);
            replacement
        })
        .expect("the response consumer window must allow root reconstruction");
    drop((original_subscription, original));
    cx.update(|_| ());
    assert!(weak_original.upgrade().is_none());
    assert_eq!(observed.drops.get(), 1);

    let replacement_subscription = cx.update(|app| {
        let snapshots = Rc::clone(&snapshots);
        app.observe(&replacement, move |inbox, app| {
            snapshots
                .borrow_mut()
                .push(inbox.read(app).responses.clone());
        })
    });
    let action = SystemNotificationResponse {
        tag: "build:foreground".into(),
        action_id: Some("open".into()),
    };
    let renders = observed.renders.get();
    cx.simulate_system_notification_response(action.clone());
    cx.run_until_parked();
    assert_eq!(
        replacement.read_with(&cx, |inbox, _| inbox.responses.clone()),
        [action.clone()]
    );
    assert_eq!(snapshots.borrow().as_slice(), [vec![body], vec![action]]);
    assert!(observed.renders.get() > renders);
    assert_eq!(cx.delivered_system_notifications(), [background]);
    assert_eq!(cx.dismissed_system_notifications(), ["build:foreground"]);

    cx.read(|app| app.dismiss_system_notification("build:background"));
    assert!(cx.delivered_system_notifications().is_empty());
    assert_eq!(
        cx.dismissed_system_notifications(),
        ["build:foreground", "build:background"]
    );
    let weak_replacement = replacement.downgrade();
    drop((replacement_subscription, replacement));
    cx.update_window(window.into(), |_, window, app| window.remove_window(app))
        .expect("the reconstructed consumer window must close");
    cx.run_until_parked();
    cx.update(|_| ());
    assert!(weak_replacement.upgrade().is_none());
    assert_eq!(observed.drops.get(), 2);
    assert!(cx.windows().is_empty());
    cx.quit();
    drop(cx);
    assert!(weak_app.upgrade().is_none());
}

#[test]
fn public_response_handler_replaces_itself_during_delivery_and_releases_its_captures() {
    let cx = Rc::new(TestAppContext::single());
    let weak_app = Rc::downgrade(&cx.app);
    let responses = Rc::new(RefCell::new(Vec::new()));
    let first_capture = Rc::new(());
    let replacement_capture = Rc::new(());
    let weak_first = Rc::downgrade(&first_capture);
    let weak_replacement = Rc::downgrade(&replacement_capture);
    let body = SystemNotificationResponse {
        tag: "build:reentrant".into(),
        action_id: None,
    };
    let action = SystemNotificationResponse {
        tag: body.tag.clone(),
        action_id: Some("open".into()),
    };
    let initial = notification("build:reentrant", "Open to inspect the build");
    let follow_up = notification("build:reentrant", "Inspection is ready");
    cx.update(|app| {
        app.set_app_identity("org.open-gpui.consumer.reentrant", "Reentrant consumer");
        app.show_system_notification(initial.clone());
        app.on_system_notification_response({
            let simulator = Rc::downgrade(&cx);
            let responses = Rc::clone(&responses);
            let first_capture = Rc::clone(&first_capture);
            let mut replacement_capture = Some(Rc::clone(&replacement_capture));
            let action = action.clone();
            let follow_up = follow_up.clone();
            move |response, app| {
                assert_eq!(Rc::strong_count(&first_capture), 1);
                responses.borrow_mut().push(response);
                app.show_system_notification(follow_up.clone());
                app.on_system_notification_response({
                    let capture = replacement_capture
                        .take()
                        .expect("only the initial handler may install its replacement");
                    let responses = Rc::clone(&responses);
                    move |response, app| {
                        assert_eq!(Rc::strong_count(&capture), 1);
                        app.dismiss_system_notification(&response.tag);
                        responses.borrow_mut().push(response);
                    }
                });
                simulator
                    .upgrade()
                    .expect("the public test context must outlive response delivery")
                    .simulate_system_notification_response(action.clone());
                assert_eq!(responses.borrow().len(), 1);
            }
        });
    });
    drop((first_capture, replacement_capture));

    cx.update(|_| {
        cx.simulate_system_notification_response(body.clone());
        assert!(responses.borrow().is_empty());
    });
    cx.run_until_parked();

    assert_eq!(responses.borrow().as_slice(), [body, action]);
    assert_eq!(cx.shown_system_notifications(), [initial, follow_up]);
    assert!(cx.delivered_system_notifications().is_empty());
    assert_eq!(cx.dismissed_system_notifications(), ["build:reentrant"]);
    assert!(weak_first.upgrade().is_none());
    assert!(weak_replacement.upgrade().is_some());
    cx.quit();
    drop(cx);
    assert!(weak_replacement.upgrade().is_none());
    assert!(weak_app.upgrade().is_none());
}
