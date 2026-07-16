use std::cell::{Cell, RefCell};
use std::rc::Rc;

use open_gpui::prelude::FluentBuilder;
use open_gpui::{
    AppContext as _, Context, InteractiveElement, IntoElement, KeyDownEvent, KeyUpEvent, Keystroke,
    Modifiers, MouseButton, ParentElement, Render, StatefulInteractiveElement, Styled, Window,
    accesskit, div,
};
use open_gpui_ui_components::{
    ActivationHandle, ActivationKey, ActivationRequestResult, ActivationSource, Button, Checkbox,
    Link, Switch, Toggle,
};
use open_gpui_ui_core::Toggled;

fn key_down(key: &str, modifiers: Modifiers, is_held: bool) -> KeyDownEvent {
    KeyDownEvent {
        keystroke: Keystroke {
            modifiers,
            key: key.to_owned(),
            key_char: None,
        },
        is_held,
        prefer_character_input: false,
    }
}

fn key_up(key: &str, modifiers: Modifiers) -> KeyUpEvent {
    KeyUpEvent {
        keystroke: Keystroke {
            modifiers,
            key: key.to_owned(),
            key_char: None,
        },
    }
}

fn node_with_label(update: &accesskit::TreeUpdate, label: &str) -> accesskit::NodeId {
    update
        .nodes
        .iter()
        .find_map(|(id, node)| (node.label() == Some(label)).then_some(*id))
        .unwrap_or_else(|| panic!("missing accessibility node labelled `{label}`"))
}

fn action_request(
    action: accesskit::Action,
    target: accesskit::NodeId,
) -> accesskit::ActionRequest {
    accesskit::ActionRequest {
        action,
        target_tree: accesskit::TreeId::ROOT,
        target_node: target,
        data: None,
    }
}

#[path = "semantic_activation/button.rs"]
mod button;
#[path = "semantic_activation/disclosure_navigation.rs"]
mod disclosure_navigation;
#[path = "semantic_activation/disclosure_ownership.rs"]
mod disclosure_ownership;
#[path = "semantic_activation/domain_actions.rs"]
mod domain_actions;
#[path = "semantic_activation/handles.rs"]
mod handles;
#[path = "semantic_activation/link.rs"]
mod link;
#[path = "semantic_activation/tabs.rs"]
mod tabs;
#[path = "semantic_activation/toggle_group.rs"]
mod toggle_group;
#[path = "semantic_activation/value_controls.rs"]
mod value_controls;
