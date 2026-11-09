//! System notifications as Windows toast notifications.

use std::cell::{Cell, RefCell};
use std::collections::{HashMap, hash_map::DefaultHasher};
use std::hash::{Hash as _, Hasher as _};
use std::rc::Rc;

use futures::StreamExt as _;
use futures::channel::mpsc;
use open_gpui::{
    ForegroundExecutor, SharedString, SystemNotification, SystemNotificationResponse, Task,
};
use windows::Data::Xml::Dom::XmlDocument;
use windows::Foundation::TypedEventHandler;
use windows::UI::Notifications::{
    ToastActivatedEventArgs, ToastNotification, ToastNotificationManager, ToastNotifier,
};
use windows::core::{HSTRING, IInspectable, Interface as _, h};

type ResponseCallback = Rc<RefCell<Option<Box<dyn FnMut(SystemNotificationResponse)>>>>;

pub(crate) struct SystemNotificationState {
    notifier: Option<ToastNotifier>,
    active_toasts: HashMap<SharedString, ToastNotification>,
    response_sender: mpsc::UnboundedSender<SystemNotificationResponse>,
    response_receiver: Option<mpsc::UnboundedReceiver<SystemNotificationResponse>>,
    callback: ResponseCallback,
    retired: Rc<Cell<bool>>,
    _response_task: Option<Task<()>>,
}

impl SystemNotificationState {
    pub(crate) fn new() -> Self {
        let (response_sender, response_receiver) = mpsc::unbounded();
        Self {
            notifier: None,
            active_toasts: HashMap::new(),
            response_sender,
            response_receiver: Some(response_receiver),
            callback: Rc::new(RefCell::new(None)),
            retired: Rc::new(Cell::new(false)),
            _response_task: None,
        }
    }

    pub(crate) fn show(
        &mut self,
        has_package_identity: bool,
        app_identity: Option<(&str, &str)>,
        notification: SystemNotification,
    ) -> windows::core::Result<()> {
        if self.retired.get() {
            return Ok(());
        }
        let Some(notifier) = self.notifier(has_package_identity, app_identity)? else {
            return Ok(());
        };

        let toast = self.toast(&notification)?;

        if let Some(previous) = self.active_toasts.remove(&notification.tag) {
            notifier.Hide(&previous)?;
        }
        notifier.Show(&toast)?;
        self.active_toasts.insert(notification.tag, toast);
        Ok(())
    }

    fn toast(&self, notification: &SystemNotification) -> windows::core::Result<ToastNotification> {
        let document = toast_document(notification)?;
        let toast = ToastNotification::CreateToastNotification(&document)?;
        toast.SetTag(&toast_tag(&notification.tag).into())?;
        toast.Activated(&self.activation_handler(notification.tag.clone()))?;
        Ok(toast)
    }

    fn activation_handler(
        &self,
        response_tag: SharedString,
    ) -> TypedEventHandler<ToastNotification, IInspectable> {
        let sender = self.response_sender.clone();
        TypedEventHandler::<ToastNotification, IInspectable>::new(move |_sender, arguments| {
            let arguments = arguments
                .as_ref()
                .and_then(|arguments| arguments.cast::<ToastActivatedEventArgs>().ok())
                .and_then(|arguments| arguments.Arguments().ok());
            sender
                .unbounded_send(SystemNotificationResponse {
                    tag: response_tag.clone(),
                    action_id: notification_action_id(arguments),
                })
                .ok();
            Ok(())
        })
    }

    pub(crate) fn dismiss(&mut self, tag: &str) {
        let Some(toast) = self.active_toasts.remove(tag) else {
            return;
        };
        let Some(notifier) = &self.notifier else {
            return;
        };
        if let Err(error) = notifier.Hide(&toast) {
            log::warn!("failed to dismiss system notification: {error}");
        }
    }

    pub(crate) fn on_response(
        &mut self,
        executor: &ForegroundExecutor,
        callback: Box<dyn FnMut(SystemNotificationResponse)>,
    ) {
        if self.retired.get() {
            return;
        }
        *self.callback.borrow_mut() = Some(callback);

        if let Some(mut receiver) = self.response_receiver.take() {
            let callback = self.callback.clone();
            let retired = self.retired.clone();
            self._response_task = Some(executor.spawn(async move {
                while let Some(response) = receiver.next().await {
                    if retired.get() {
                        break;
                    }
                    // Take the callback out for the call: it may re-enter the
                    // platform (e.g. to dismiss the notification it was told
                    // about) or replace itself.
                    let taken = callback.borrow_mut().take();
                    if let Some(mut taken) = taken {
                        taken(response);
                        if !retired.get() {
                            callback.borrow_mut().get_or_insert(taken);
                        }
                    }
                }
            }));
        }
    }

    /// Stops response delivery and releases WinRT objects before native platform teardown.
    pub(crate) fn retire(&mut self) {
        self.retired.set(true);
        self.response_sender.close_channel();
        self.response_receiver.take();
        self._response_task.take();
        self.callback.borrow_mut().take();
        self.active_toasts.clear();
        self.notifier.take();
    }

    #[cfg(test)]
    pub(crate) fn response_sender_for_test(
        &self,
    ) -> mpsc::UnboundedSender<SystemNotificationResponse> {
        self.response_sender.clone()
    }

    fn notifier(
        &mut self,
        has_package_identity: bool,
        app_identity: Option<(&str, &str)>,
    ) -> windows::core::Result<Option<ToastNotifier>> {
        if let Some(notifier) = &self.notifier {
            return Ok(Some(notifier.clone()));
        }

        let notifier = if has_package_identity {
            ToastNotificationManager::CreateToastNotifier()?
        } else {
            let Some((app_identifier, app_name)) = app_identity else {
                log::warn!(
                    "cannot show a system notification without an app identity; \
                     call `App::set_app_identity` during startup"
                );
                return Ok(None);
            };
            register_app_user_model_id(app_identifier, app_name);
            ToastNotificationManager::CreateToastNotifierWithId(&app_identifier.into())?
        };

        self.notifier = Some(notifier.clone());
        Ok(Some(notifier))
    }
}

impl Drop for SystemNotificationState {
    fn drop(&mut self) {
        self.retire();
    }
}

fn toast_tag(tag: &SharedString) -> String {
    // Windows caps toast tags at 64 characters (post-Creators Update),
    // so hash the arbitrary GPUI tag down to a fixed-width value.
    let mut hasher = DefaultHasher::new();
    tag.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

fn notification_action_id(arguments: Option<HSTRING>) -> Option<SharedString> {
    arguments
        .filter(|arguments| !arguments.is_empty())
        .map(|arguments| SharedString::from(arguments.to_string()))
}

fn toast_document(notification: &SystemNotification) -> windows::core::Result<XmlDocument> {
    let document = XmlDocument::new()?;
    let toast = document.CreateElement(h!("toast"))?;
    document.AppendChild(&toast)?;

    let visual = document.CreateElement(h!("visual"))?;
    toast.AppendChild(&visual)?;
    let binding = document.CreateElement(h!("binding"))?;
    binding.SetAttribute(h!("template"), h!("ToastGeneric"))?;
    visual.AppendChild(&binding)?;
    for text in [&notification.title, &notification.body] {
        let element = document.CreateElement(h!("text"))?;
        element.SetInnerText(&text.as_ref().into())?;
        binding.AppendChild(&element)?;
    }

    if !notification.actions.is_empty() {
        let actions = document.CreateElement(h!("actions"))?;
        toast.AppendChild(&actions)?;
        for action in &notification.actions {
            let action_element = document.CreateElement(h!("action"))?;
            action_element.SetAttribute(h!("content"), &action.label.as_ref().into())?;
            action_element.SetAttribute(h!("arguments"), &action.id.as_ref().into())?;
            actions.AppendChild(&action_element)?;
        }
    }

    let audio = document.CreateElement(h!("audio"))?;
    audio.SetAttribute(h!("silent"), h!("true"))?;
    toast.AppendChild(&audio)?;
    Ok(document)
}

/// Registers the app's AUMID so toasts display correctly for an unpackaged app without a Start Menu shortcut.
fn register_app_user_model_id(app_identifier: &str, app_name: &str) {
    let result = windows_registry::CURRENT_USER
        .create(format!(r"Software\Classes\AppUserModelId\{app_identifier}"))
        .and_then(|key| key.set_string("DisplayName", app_name));
    if let Err(error) = result {
        log::warn!("failed to register AppUserModelID; notifications may not display: {error}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::WindowsPlatform;
    use open_gpui::SystemNotificationAction;

    fn notification(tag: impl Into<SharedString>) -> SystemNotification {
        SystemNotification {
            tag: tag.into(),
            title: "Build <ready> & \"checked\" '日本語' 🦀".into(),
            body: "Inspect <changes> & continue".into(),
            actions: vec![
                SystemNotificationAction {
                    id: "open?path=<file>&mode=\"read\"".into(),
                    label: "Open <file> & inspect".into(),
                },
                SystemNotificationAction {
                    id: "dismiss/日本語".into(),
                    label: "Later 'please'".into(),
                },
            ],
        }
    }

    fn xml_text(document: &XmlDocument, xpath: &str) -> String {
        document
            .SelectSingleNode(&xpath.into())
            .expect("toast XML node")
            .InnerText()
            .expect("toast XML text")
            .to_string()
    }

    #[test]
    fn system_notification_xml_round_trips_text_and_ordered_action_arguments() {
        let _platform = WindowsPlatform::new(true).expect("Windows XML apartment");
        let notification = notification("xml-actions");
        let document = toast_document(&notification).expect("toast document");
        let round_trip = XmlDocument::new().expect("round-trip document");
        round_trip
            .LoadXml(&document.GetXml().expect("serialized toast XML"))
            .expect("well-formed escaped toast XML");

        assert_eq!(
            xml_text(&round_trip, "/toast/visual/binding/@template"),
            "ToastGeneric",
        );
        assert_eq!(
            round_trip
                .SelectNodes(h!("/toast/visual/binding/text"))
                .unwrap()
                .Size()
                .unwrap(),
            2,
        );
        assert_eq!(
            xml_text(&round_trip, "/toast/visual/binding/text[1]"),
            notification.title.as_ref(),
        );
        assert_eq!(
            xml_text(&round_trip, "/toast/visual/binding/text[2]"),
            notification.body.as_ref(),
        );
        assert_eq!(
            round_trip
                .SelectNodes(h!("/toast/actions/action"))
                .unwrap()
                .Size()
                .unwrap(),
            2,
        );
        for (index, action) in notification.actions.iter().enumerate() {
            let xpath = format!("/toast/actions/action[{}]", index + 1);
            assert_eq!(
                xml_text(&round_trip, &format!("{xpath}/@content")),
                action.label.as_ref(),
            );
            assert_eq!(
                xml_text(&round_trip, &format!("{xpath}/@arguments")),
                action.id.as_ref(),
            );
        }
        assert_eq!(xml_text(&round_trip, "/toast/audio/@silent"), "true");
    }

    #[test]
    fn system_notification_xml_omits_absent_actions_and_keeps_empty_body() {
        let _platform = WindowsPlatform::new(true).expect("Windows XML apartment");
        let mut notification = notification("xml-body");
        notification.body = "".into();
        notification.actions.clear();
        let document = toast_document(&notification).expect("body-only toast document");

        assert_eq!(
            document
                .SelectNodes(h!("/toast/actions"))
                .unwrap()
                .Size()
                .unwrap(),
            0,
        );
        assert_eq!(
            document
                .SelectNodes(h!("/toast/visual/binding/text"))
                .unwrap()
                .Size()
                .unwrap(),
            2,
        );
        assert_eq!(xml_text(&document, "/toast/visual/binding/text[2]"), "");
        assert_eq!(xml_text(&document, "/toast/audio/@silent"), "true");
    }

    #[test]
    fn system_notification_tags_are_fixed_width_and_stable_for_replacement() {
        let tags = [
            "".into(),
            "short".into(),
            SharedString::from("🦀".repeat(256)),
        ];
        for tag in &tags {
            let hash = toast_tag(tag);
            assert_eq!(hash.len(), 16);
            assert!(hash.bytes().all(|byte| byte.is_ascii_hexdigit()));
            assert_eq!(toast_tag(&SharedString::from(tag.to_string())), hash);
        }
        assert_ne!(toast_tag(&tags[1]), toast_tag(&tags[2]));
    }

    #[test]
    fn system_notification_action_arguments_distinguish_body_from_nonempty_ids() {
        assert_eq!(notification_action_id(None), None);
        assert_eq!(notification_action_id(Some(HSTRING::new())), None);
        for id in ["open?path=<file>&mode=\"read\" 日本語🦀", " "] {
            assert_eq!(
                notification_action_id(Some(id.into())),
                Some(SharedString::from(id.to_string())),
            );
        }
    }

    #[test]
    fn system_notification_constructed_toast_hashes_long_tag_and_preserves_response_tag() {
        let _platform = WindowsPlatform::new(true).expect("Windows toast apartment");
        let mut state = SystemNotificationState::new();
        let notification = notification("notification/日本語/🦀".repeat(256));
        let toast = state.toast(&notification).expect("constructed toast");
        let replacement = state
            .toast(&notification)
            .expect("replacement toast construction");
        assert_eq!(
            toast.Tag().unwrap().to_string(),
            toast_tag(&notification.tag)
        );
        assert_eq!(toast.Tag().unwrap(), replacement.Tag().unwrap());
        assert_eq!(
            xml_text(&toast.Content().unwrap(), "/toast/visual/binding/text[1]"),
            notification.title.as_ref(),
        );

        let handler = state.activation_handler(notification.tag.clone());
        handler
            .Invoke(None::<&ToastNotification>, None::<&IInspectable>)
            .expect("synthetic body activation delegate");
        let response = state
            .response_receiver
            .as_mut()
            .unwrap()
            .try_recv()
            .unwrap();
        assert_eq!(
            response,
            SystemNotificationResponse {
                tag: notification.tag,
                action_id: None,
            },
        );
    }

    #[test]
    fn system_notification_activation_tolerates_unexpected_arguments_and_retirement() {
        let _platform = WindowsPlatform::new(true).expect("Windows toast apartment");
        let mut state = SystemNotificationState::new();
        let handler = state.activation_handler("unexpected-arguments".into());
        let unexpected: IInspectable = XmlDocument::new().unwrap().cast().unwrap();
        handler
            .Invoke(None::<&ToastNotification>, &unexpected)
            .expect("unrecognized activation arguments are a body activation");
        assert_eq!(
            state
                .response_receiver
                .as_mut()
                .unwrap()
                .try_recv()
                .unwrap(),
            SystemNotificationResponse {
                tag: "unexpected-arguments".into(),
                action_id: None,
            },
        );

        let sender = state.response_sender.clone();
        state.retire();
        handler
            .Invoke(None::<&ToastNotification>, None::<&IInspectable>)
            .expect("late native activation is harmless");
        assert!(sender.is_closed());
        assert!(state.response_receiver.is_none());
        assert!(state.callback.borrow().is_none());
        assert!(state._response_task.is_none());
        assert!(state.active_toasts.is_empty());
        assert!(state.notifier.is_none());
    }

    #[test]
    fn system_notification_without_identity_does_not_create_a_notifier_or_retain_toasts() {
        let mut state = SystemNotificationState::new();
        for title in ["first", "replacement"] {
            let mut notification = notification("same-tag");
            notification.title = title.into();
            state
                .show(false, None, notification)
                .expect("missing identity is a best-effort no-op before WinRT construction");
            assert!(state.notifier.is_none());
            assert!(state.active_toasts.is_empty());
        }
        state.dismiss("same-tag");
        state.dismiss("unknown-tag");
        assert!(state.notifier.is_none());
        assert!(state.active_toasts.is_empty());
    }
}
