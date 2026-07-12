use crate::{SharedString, window::a11y::A11yActionListener};

#[derive(Default)]
pub(super) struct InteractivityAccessibility {
    pub(super) action_listeners: Vec<(accesskit::Action, A11yActionListener)>,
    pub(super) override_role: Option<accesskit::Role>,
    pub(super) label: Option<SharedString>,
    pub(super) description: Option<SharedString>,
    pub(super) controls: Option<Vec<accesskit::NodeId>>,
    pub(super) labelled_by: Option<Vec<accesskit::NodeId>>,
    pub(super) described_by: Option<Vec<accesskit::NodeId>>,
    pub(super) value: Option<SharedString>,
    pub(super) selected: Option<bool>,
    pub(super) required: Option<bool>,
    pub(super) invalid: Option<bool>,
    pub(super) busy: Option<bool>,
    pub(super) read_only: Option<bool>,
    pub(super) hidden: Option<bool>,
    pub(super) modal: Option<bool>,
    pub(super) disabled: Option<bool>,
    pub(super) expanded: Option<bool>,
    pub(super) toggled: Option<accesskit::Toggled>,
    pub(super) numeric_value: Option<f64>,
    pub(super) min_numeric_value: Option<f64>,
    pub(super) max_numeric_value: Option<f64>,
    pub(super) orientation: Option<accesskit::Orientation>,
    pub(super) level: Option<usize>,
    pub(super) position_in_set: Option<usize>,
    pub(super) size_of_set: Option<usize>,
    pub(super) row_index: Option<usize>,
    pub(super) column_index: Option<usize>,
    pub(super) row_count: Option<usize>,
    pub(super) column_count: Option<usize>,
}

impl InteractivityAccessibility {
    pub(super) fn write_node(
        &self,
        node: &mut accesskit::Node,
        supports_click: bool,
        supports_focus: bool,
    ) {
        if let Some(label) = &self.label {
            node.set_label(label.to_string());
        }
        if let Some(description) = &self.description {
            node.set_description(description.to_string());
        }
        if let Some(controls) = &self.controls {
            node.set_controls(controls.clone());
        }
        if let Some(labelled_by) = &self.labelled_by {
            node.set_labelled_by(labelled_by.clone());
        }
        if let Some(described_by) = &self.described_by {
            node.set_described_by(described_by.clone());
        }
        if let Some(value) = &self.value {
            node.set_value(value.to_string());
        }
        if let Some(selected) = self.selected {
            node.set_selected(selected);
        }
        if let Some(required) = self.required {
            if required {
                node.set_required();
            } else {
                node.clear_required();
            }
        }
        if let Some(invalid) = self.invalid {
            if invalid {
                node.set_invalid(accesskit::Invalid::True);
            } else {
                node.clear_invalid();
            }
        }
        if let Some(busy) = self.busy {
            if busy {
                node.set_busy();
            } else {
                node.clear_busy();
            }
        }
        if let Some(read_only) = self.read_only {
            if read_only {
                node.set_read_only();
            } else {
                node.clear_read_only();
            }
        }
        if let Some(hidden) = self.hidden {
            if hidden {
                node.set_hidden();
            } else {
                node.clear_hidden();
            }
        }
        if let Some(modal) = self.modal {
            if modal {
                node.set_modal();
            } else {
                node.clear_modal();
            }
        }
        if let Some(disabled) = self.disabled {
            if disabled {
                node.set_disabled();
            } else {
                node.clear_disabled();
            }
        }
        if let Some(expanded) = self.expanded {
            node.set_expanded(expanded);
        }
        if let Some(toggled) = self.toggled {
            node.set_toggled(toggled);
        }
        if let Some(value) = self.numeric_value {
            node.set_numeric_value(value);
        }
        if let Some(value) = self.min_numeric_value {
            node.set_min_numeric_value(value);
        }
        if let Some(value) = self.max_numeric_value {
            node.set_max_numeric_value(value);
        }
        if let Some(orientation) = self.orientation {
            node.set_orientation(orientation);
        }
        if let Some(level) = self.level {
            node.set_level(level);
        }
        if let Some(position) = self.position_in_set {
            node.set_position_in_set(position);
        }
        if let Some(size) = self.size_of_set {
            node.set_size_of_set(size);
        }
        if let Some(index) = self.row_index {
            node.set_row_index(index);
        }
        if let Some(index) = self.column_index {
            node.set_column_index(index);
        }
        if let Some(count) = self.row_count {
            node.set_row_count(count);
        }
        if let Some(count) = self.column_count {
            node.set_column_count(count);
        }
        if self.disabled != Some(true) {
            if supports_click {
                node.add_action(accesskit::Action::Click);
            }
            if supports_focus {
                node.add_action(accesskit::Action::Focus);
            }
            for (action, _) in &self.action_listeners {
                if self.read_only != Some(true) || !action_mutates_value(*action) {
                    node.add_action(*action);
                }
            }
        }
    }
}

fn action_mutates_value(action: accesskit::Action) -> bool {
    matches!(
        action,
        accesskit::Action::Decrement
            | accesskit::Action::Increment
            | accesskit::Action::ReplaceSelectedText
            | accesskit::Action::SetValue
    )
}
