use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

#[derive(Clone, Default)]
pub(crate) struct WindowInteractionGate(Arc<AtomicBool>);

impl WindowInteractionGate {
    pub(crate) fn is_quiesced(&self) -> bool {
        self.0.load(Ordering::Acquire)
    }

    pub(crate) fn quiesce(&self) {
        self.0.store(true, Ordering::Release);
    }

    pub(crate) fn admits_status(&self, positive: bool) -> bool {
        !positive || !self.is_quiesced()
    }
}

pub(crate) fn accesskit_adapter(
    callbacks: open_gpui::A11yCallbacks,
    interaction_gate: WindowInteractionGate,
) -> accesskit_unix::Adapter {
    let (activation_handler, action_handler, deactivation_handler) =
        accesskit_handlers(callbacks, interaction_gate);
    accesskit_unix::Adapter::new(activation_handler, action_handler, deactivation_handler)
}

fn accesskit_handlers(
    callbacks: open_gpui::A11yCallbacks,
    interaction_gate: WindowInteractionGate,
) -> (
    impl accesskit::ActivationHandler + Send + 'static,
    impl accesskit::ActionHandler + Send + 'static,
    impl accesskit::DeactivationHandler + Send + 'static,
) {
    (
        AccessKitActivationHandler {
            callback: callbacks.activation,
            interaction_gate: interaction_gate.clone(),
        },
        AccessKitActionHandler {
            callback: callbacks.action,
            interaction_gate,
        },
        AccessKitDeactivationHandler {
            callback: callbacks.deactivation,
        },
    )
}

struct AccessKitActivationHandler {
    callback: Box<dyn Fn() -> Option<accesskit::TreeUpdate> + Send + 'static>,
    interaction_gate: WindowInteractionGate,
}

impl accesskit::ActivationHandler for AccessKitActivationHandler {
    fn request_initial_tree(&mut self) -> Option<accesskit::TreeUpdate> {
        if self.interaction_gate.is_quiesced() {
            return None;
        }
        (self.callback)()
    }
}

struct AccessKitActionHandler {
    callback: Box<dyn Fn(accesskit::ActionRequest) + Send + 'static>,
    interaction_gate: WindowInteractionGate,
}

impl accesskit::ActionHandler for AccessKitActionHandler {
    fn do_action(&mut self, request: accesskit::ActionRequest) {
        if self.interaction_gate.is_quiesced() {
            return;
        }
        (self.callback)(request);
    }
}

struct AccessKitDeactivationHandler {
    callback: Box<dyn Fn() + Send + 'static>,
}

impl accesskit::DeactivationHandler for AccessKitDeactivationHandler {
    fn deactivate_accessibility(&mut self) {
        (self.callback)();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };

    fn action_request() -> accesskit::ActionRequest {
        accesskit::ActionRequest {
            action: accesskit::Action::Focus,
            target_tree: accesskit::TreeId::ROOT,
            target_node: accesskit::NodeId(1),
            data: None,
        }
    }

    #[test]
    fn positive_status_is_blocked_after_quiescence_but_loss_is_admitted() {
        let gate = WindowInteractionGate::default();

        assert!(gate.admits_status(true));
        gate.quiesce();
        assert!(!gate.admits_status(true));
        assert!(gate.admits_status(false));
    }

    #[test]
    fn accesskit_handlers_gate_interaction_but_still_deactivate() {
        let gate = WindowInteractionGate::default();
        let activation_calls = Arc::new(AtomicUsize::new(0));
        let action_calls = Arc::new(AtomicUsize::new(0));
        let deactivation_calls = Arc::new(AtomicUsize::new(0));
        let callbacks = open_gpui::A11yCallbacks {
            activation: Box::new({
                let activation_calls = activation_calls.clone();
                move || {
                    activation_calls.fetch_add(1, Ordering::Relaxed);
                    None
                }
            }),
            action: Box::new({
                let action_calls = action_calls.clone();
                move |_| {
                    action_calls.fetch_add(1, Ordering::Relaxed);
                }
            }),
            deactivation: Box::new({
                let deactivation_calls = deactivation_calls.clone();
                move || {
                    deactivation_calls.fetch_add(1, Ordering::Relaxed);
                }
            }),
        };
        let (mut activation_handler, mut action_handler, mut deactivation_handler) =
            accesskit_handlers(callbacks, gate.clone());

        accesskit::ActivationHandler::request_initial_tree(&mut activation_handler);
        accesskit::ActionHandler::do_action(&mut action_handler, action_request());
        gate.quiesce();
        accesskit::ActivationHandler::request_initial_tree(&mut activation_handler);
        accesskit::ActionHandler::do_action(&mut action_handler, action_request());
        accesskit::DeactivationHandler::deactivate_accessibility(&mut deactivation_handler);

        assert_eq!(activation_calls.load(Ordering::Relaxed), 1);
        assert_eq!(action_calls.load(Ordering::Relaxed), 1);
        assert_eq!(deactivation_calls.load(Ordering::Relaxed), 1);
    }
}
