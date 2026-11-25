//! System notifications over the XDG notifications D-Bus interface, via
//! `notify-rust`.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use futures::StreamExt as _;
use futures::channel::mpsc;
use open_gpui::{
    ForegroundExecutor, SharedString, SystemNotification, SystemNotificationResponse, Task,
};

/// The XDG action key invoked when the user activates the notification body
/// rather than a specific action button.
const DEFAULT_ACTION: &str = "default";

type ResponseCallback = Rc<RefCell<Option<Box<dyn FnMut(SystemNotificationResponse)>>>>;

pub(crate) struct SystemNotificationState {
    response_sender: mpsc::UnboundedSender<SystemNotificationResponse>,
    response_receiver: Option<mpsc::UnboundedReceiver<SystemNotificationResponse>>,
    callback: ResponseCallback,
    _response_task: Option<Task<()>>,
}

impl SystemNotificationState {
    pub(crate) fn new() -> Self {
        let (response_sender, response_receiver) = mpsc::unbounded();
        Self {
            response_sender,
            response_receiver: Some(response_receiver),
            callback: Rc::new(RefCell::new(None)),
            _response_task: None,
        }
    }

    pub(crate) fn show(&self, app_name: Option<&str>, notification: SystemNotification) {
        let mut builder = notify_rust::Notification::new();
        if let Some(app_name) = app_name {
            builder.appname(app_name);
        }
        builder
            .summary(&notification.title)
            .body(&notification.body)
            .action(DEFAULT_ACTION, DEFAULT_ACTION);
        let mut action_ids = HashMap::new();
        for (index, action) in notification.actions.iter().enumerate() {
            let transport_id = format!("gpui-action-{index}");
            builder.action(&transport_id, &action.label);
            action_ids.insert(transport_id, action.id.clone());
        }
        let built = builder.finalize();

        let sender = self.response_sender.clone();
        let tag = notification.tag.clone();
        // `show` connects to the session bus and `wait_for_action` blocks
        // until the notification closes, so both run off the UI thread.
        let spawn_result = std::thread::Builder::new()
            .name("system-notification".to_string())
            .spawn(move || match built.show() {
                Ok(handle) => handle.wait_for_action(|action| {
                    let action_id = match action {
                        // Sent by `notify-rust` when the notification closes
                        // without being activated.
                        "__closed" => return,
                        transport_id => {
                            let Some(action_id) = response_action_id(transport_id, &action_ids)
                            else {
                                log::warn!(
                                    "system notification returned unknown action {transport_id:?}"
                                );
                                return;
                            };
                            action_id
                        }
                    };
                    sender
                        .unbounded_send(SystemNotificationResponse { tag, action_id })
                        .ok();
                }),
                Err(error) => log::warn!("failed to show system notification: {error}"),
            });
        if let Err(error) = spawn_result {
            log::warn!("failed to spawn system notification thread: {error}");
        }
    }

    pub(crate) fn dismiss(&self, _tag: &str) {
        // The XDG notifications protocol only allows closing via the
        // server-assigned id on a live handle, which `wait_for_action`
        // consumes; stale notifications simply age out of the shade.
    }

    pub(crate) fn on_response(
        &mut self,
        executor: &ForegroundExecutor,
        callback: Box<dyn FnMut(SystemNotificationResponse)>,
    ) {
        *self.callback.borrow_mut() = Some(callback);

        // Responses arrive from per-notification threads; this task hands
        // them to the registered callback on the main thread.
        if let Some(mut receiver) = self.response_receiver.take() {
            let callback = self.callback.clone();
            self._response_task = Some(executor.spawn(async move {
                while let Some(response) = receiver.next().await {
                    // Take the callback out for the call: it may re-enter the
                    // platform or replace itself.
                    let taken = callback.borrow_mut().take();
                    if let Some(mut taken) = taken {
                        taken(response);
                        callback.borrow_mut().get_or_insert(taken);
                    }
                }
            }));
        }
    }
}

fn response_action_id(
    transport_id: &str,
    action_ids: &HashMap<String, SharedString>,
) -> Option<Option<SharedString>> {
    if transport_id == DEFAULT_ACTION {
        Some(None)
    } else {
        action_ids.get(transport_id).cloned().map(Some)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use open_gpui::TestDispatcher;
    use std::cell::Cell;
    use std::sync::Arc;

    #[test]
    fn caller_action_ids_do_not_collide_with_transport_action_ids() {
        let action_ids = HashMap::from([
            ("gpui-action-0".to_string(), SharedString::from("default")),
            ("gpui-action-1".to_string(), SharedString::from("__closed")),
        ]);

        assert_eq!(response_action_id(DEFAULT_ACTION, &action_ids), Some(None));
        assert_eq!(
            response_action_id("gpui-action-0", &action_ids),
            Some(Some("default".into()))
        );
        assert_eq!(
            response_action_id("gpui-action-1", &action_ids),
            Some(Some("__closed".into()))
        );
        assert_eq!(response_action_id("unknown", &action_ids), None);
    }

    #[test]
    fn responses_keep_foreground_order_through_reentry_and_handler_replacement() {
        let dispatcher = Arc::new(TestDispatcher::new(0));
        let executor = ForegroundExecutor::new(dispatcher.clone());
        let state = Rc::new(RefCell::new(SystemNotificationState::new()));
        let responses = [
            SystemNotificationResponse {
                tag: "first".into(),
                action_id: None,
            },
            SystemNotificationResponse {
                tag: "second".into(),
                action_id: Some("default".into()),
            },
            SystemNotificationResponse {
                tag: "third".into(),
                action_id: Some("__closed".into()),
            },
            SystemNotificationResponse {
                tag: "fourth".into(),
                action_id: None,
            },
        ];
        let sender = state.borrow().response_sender.clone();
        let queued = responses.clone();
        std::thread::spawn(move || {
            for response in queued {
                sender.unbounded_send(response).unwrap();
            }
        })
        .join()
        .unwrap();

        let seen = Rc::new(RefCell::new(Vec::new()));
        let main_thread = std::thread::current().id();
        let weak_state = Rc::downgrade(&state);
        let callback_executor = executor.clone();
        state.borrow_mut().on_response(
            &executor,
            Box::new({
                let seen = seen.clone();
                move |response| {
                    assert_eq!(std::thread::current().id(), main_thread);
                    let state = weak_state.upgrade().unwrap();
                    state.borrow().dismiss(&response.tag);
                    let replace = response.tag.as_ref() == "second";
                    seen.borrow_mut().push(("initial", response));
                    if replace {
                        state.borrow_mut().on_response(
                            &callback_executor,
                            Box::new({
                                let seen = seen.clone();
                                move |response| {
                                    assert_eq!(std::thread::current().id(), main_thread);
                                    seen.borrow_mut().push(("replacement", response));
                                }
                            }),
                        );
                    }
                }
            }),
        );

        assert!(seen.borrow().is_empty());
        dispatcher.run_until_parked();
        assert_eq!(
            *seen.borrow(),
            [
                ("initial", responses[0].clone()),
                ("initial", responses[1].clone()),
                ("replacement", responses[2].clone()),
                ("replacement", responses[3].clone()),
            ]
        );
        drop(state);
        dispatcher.run_until_parked();
    }

    #[test]
    fn dropping_state_cancels_queued_responses_and_releases_the_handler() {
        let dispatcher = Arc::new(TestDispatcher::new(0));
        let executor = ForegroundExecutor::new(dispatcher.clone());
        let mut state = SystemNotificationState::new();
        let sender = state.response_sender.clone();
        let seen = Rc::new(Cell::new(0));
        let retained = Rc::new(());
        let weak = Rc::downgrade(&retained);
        state.on_response(
            &executor,
            Box::new({
                let seen = seen.clone();
                move |_| {
                    let _ = &retained;
                    seen.set(seen.get() + 1);
                }
            }),
        );
        let response = SystemNotificationResponse {
            tag: "pending".into(),
            action_id: None,
        };
        sender.unbounded_send(response.clone()).unwrap();

        drop(state);
        dispatcher.run_until_parked();

        assert_eq!(seen.get(), 0);
        assert!(weak.upgrade().is_none());
        assert!(sender.unbounded_send(response).is_err());
    }
}
