//! Ordinary notification consumers without GPUI test-support.
//! Value construction and traits execute; the App and Platform signature closures are unexecuted.
//! Actual App delivery and entity notification belong to the separate TestPlatform consumer.

use open_gpui::{
    App, Platform, SystemNotification, SystemNotificationAction, SystemNotificationResponse,
};
use std::{collections::HashSet, fmt::Debug};

fn assert_value_traits<T: Clone + Debug + PartialEq + Eq>() {}

#[test]
fn downstream_constructs_owned_notification_values_and_executes_their_traits() {
    assert_value_traits::<SystemNotification>();
    assert_value_traits::<SystemNotificationAction>();
    let open = SystemNotificationAction {
        id: String::from("open:<build>&\"").into(),
        label: "Open build".into(),
    };
    let snooze = SystemNotificationAction {
        id: "snooze".into(),
        label: "Remind me later".into(),
    };
    let original = SystemNotification {
        tag: String::from("build/日本語:42").into(),
        title: String::from("Build completed").into(),
        body: "Open the report or request a reminder.".into(),
        actions: vec![open.clone(), snooze.clone()],
    };
    let mut replacement = original.clone();
    assert_eq!(replacement, original);
    replacement.title = "Build report updated".into();
    replacement.body = "The updated report is ready.".into();
    replacement.actions.pop();
    assert_eq!(replacement.tag, original.tag);
    assert_ne!(replacement, original);
    assert_eq!(original.title, "Build completed");
    assert_eq!(original.actions, [open.clone(), snooze.clone()]);
    assert_eq!(replacement.actions, [open.clone()]);
    assert_eq!(open.id, "open:<build>&\"");
    assert_eq!(open.label, "Open build");
    assert_eq!(HashSet::from([open.clone(), open, snooze]).len(), 2);
    assert!(!format!("{original:?}").is_empty());
}

#[test]
fn downstream_constructs_distinct_body_and_action_responses() {
    assert_value_traits::<SystemNotificationResponse>();
    let body = SystemNotificationResponse {
        tag: "build/日本語:42".into(),
        action_id: None,
    };
    let action = SystemNotificationResponse {
        tag: body.tag.clone(),
        action_id: Some(String::from("default").into()),
    };
    let another_action = SystemNotificationResponse {
        tag: action.tag.clone(),
        action_id: Some("__closed".into()),
    };
    assert_eq!(body.clone(), body);
    assert_eq!(action.clone(), action);
    assert_eq!(body.tag, action.tag);
    assert_eq!(body.action_id, None);
    assert_eq!(action.action_id.as_deref(), Some("default"));
    assert_eq!(another_action.action_id.as_deref(), Some("__closed"));
    assert_ne!(body, action);
    assert_ne!(action, another_action);
    assert!(!format!("{another_action:?}").is_empty());
}

#[test]
fn downstream_unexecuted_app_and_platform_signatures_accept_owned_mutable_handlers() {
    let _: fn(&App, &str, &str) = App::set_app_identity;
    let _: fn(&App, SystemNotification) = App::show_system_notification;
    let _: fn(&App, &str) = App::dismiss_system_notification;
    let _: fn(&App, fn(SystemNotificationResponse, &mut App)) =
        App::on_system_notification_response::<fn(SystemNotificationResponse, &mut App)>;

    let _unexecuted_app_calls = |app: &App, notification: SystemNotification| {
        let (): () = app.set_app_identity("org.open-gpui.consumer", "Ordinary consumer");
        let (): () = app.show_system_notification(notification);
        let (): () = app.dismiss_system_notification("build/日本語:42");
        let mut responses = Vec::new();
        let (): () = app.on_system_notification_response(move |response, app: &mut App| {
            app.dismiss_system_notification(&response.tag);
            responses.push(response);
        });
    };
    let _unexecuted_platform_calls = |platform: &dyn Platform, notification: SystemNotification| {
        let (): () = platform.set_app_identity("org.open-gpui.consumer", "Ordinary consumer");
        let (): () = platform.show_system_notification(notification);
        let (): () = platform.dismiss_system_notification("build/日本語:42");
        let mut responses = Vec::new();
        let callback: Box<dyn FnMut(SystemNotificationResponse)> = Box::new(move |response| {
            responses.push(response);
        });
        let (): () = platform.on_system_notification_response(callback);
    };
}
