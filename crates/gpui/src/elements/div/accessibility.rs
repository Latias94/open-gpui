use crate::{
    SharedString,
    window::a11y::{A11yActionListener, ACCESSKIT_ACTIONS, action_mask},
};

#[derive(Clone, Copy, Default)]
struct A11yActionSet(u32);

impl A11yActionSet {
    fn insert(&mut self, action: accesskit::Action) {
        self.0 |= action_mask(action);
    }

    fn iter(self) -> impl Iterator<Item = accesskit::Action> {
        ACCESSKIT_ACTIONS
            .iter()
            .copied()
            .filter(move |action| self.0 & action_mask(*action) != 0)
    }
}

impl FromIterator<accesskit::Action> for A11yActionSet {
    fn from_iter<T: IntoIterator<Item = accesskit::Action>>(iter: T) -> Self {
        let mut actions = Self::default();
        for action in iter {
            actions.insert(action);
        }
        actions
    }
}

#[derive(Default)]
pub(super) struct InteractivityAccessibility {
    pub(super) action_listeners: Vec<(accesskit::Action, A11yActionListener)>,
    explicit_actions: Option<A11yActionSet>,
    pub(super) override_role: Option<accesskit::Role>,
    pub(super) label: Option<SharedString>,
    pub(super) description: Option<SharedString>,
    pub(super) placeholder: Option<SharedString>,
    pub(super) character_lengths: Option<Vec<u8>>,
    pub(super) text_selection: Option<accesskit::TextSelection>,
    pub(super) controls: Option<Vec<accesskit::NodeId>>,
    pub(super) labelled_by: Option<Vec<accesskit::NodeId>>,
    pub(super) described_by: Option<Vec<accesskit::NodeId>>,
    pub(super) error_message: Option<accesskit::NodeId>,
    pub(super) value: Option<SharedString>,
    pub(super) selected: Option<bool>,
    pub(super) required: Option<bool>,
    pub(super) invalid: Option<bool>,
    pub(super) busy: Option<bool>,
    pub(super) live: Option<accesskit::Live>,
    pub(super) live_atomic: Option<bool>,
    pub(super) read_only: Option<bool>,
    pub(super) omit_node: bool,
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
    pub(super) row_span: Option<usize>,
    pub(super) column_span: Option<usize>,
    pub(super) row_count: Option<usize>,
    pub(super) column_count: Option<usize>,
    pub(super) sort_direction: Option<accesskit::SortDirection>,
}

impl InteractivityAccessibility {
    pub(super) fn add_explicit_action(&mut self, action: accesskit::Action) {
        self.explicit_actions.get_or_insert_default().insert(action);
    }

    pub(super) fn set_explicit_actions(
        &mut self,
        actions: impl IntoIterator<Item = accesskit::Action>,
    ) {
        self.explicit_actions = Some(actions.into_iter().collect());
    }

    fn action_allowed(&self, action: accesskit::Action) -> bool {
        self.read_only != Some(true) || !action_mutates_value(action)
    }

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
        if let Some(placeholder) = &self.placeholder {
            node.set_placeholder(placeholder.to_string());
        }
        if let Some(character_lengths) = &self.character_lengths {
            node.set_character_lengths(character_lengths.clone());
        }
        if let Some(text_selection) = self.text_selection {
            node.set_text_selection(text_selection);
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
        if let Some(error_message) = self.error_message {
            node.set_error_message(error_message);
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
        if let Some(live) = self.live {
            node.set_live(live);
        }
        if let Some(atomic) = self.live_atomic {
            if atomic {
                node.set_live_atomic();
            } else {
                node.clear_live_atomic();
            }
        }
        if let Some(read_only) = self.read_only {
            if read_only {
                node.set_read_only();
            } else {
                node.clear_read_only();
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
        if let Some(span) = self.row_span {
            node.set_row_span(span);
        }
        if let Some(span) = self.column_span {
            node.set_column_span(span);
        }
        if let Some(count) = self.row_count {
            node.set_row_count(count);
        }
        if let Some(count) = self.column_count {
            node.set_column_count(count);
        }
        if let Some(direction) = self.sort_direction {
            node.set_sort_direction(direction);
        }
        if self.disabled == Some(true) {
            return;
        }
        if let Some(actions) = self.explicit_actions {
            for action in actions.iter().filter(|action| self.action_allowed(*action)) {
                node.add_action(action);
            }
            return;
        }
        if supports_click {
            node.add_action(accesskit::Action::Click);
        }
        if supports_focus {
            node.add_action(accesskit::Action::Focus);
        }
        for (action, _) in &self.action_listeners {
            if self.action_allowed(*action) {
                node.add_action(*action);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn declared_actions_are_written_without_listeners() {
        let accessibility = InteractivityAccessibility {
            explicit_actions: Some(
                [accesskit::Action::Click, accesskit::Action::Increment]
                    .into_iter()
                    .collect(),
            ),
            ..Default::default()
        };
        let mut node = accesskit::Node::new(accesskit::Role::Button);

        accessibility.write_node(&mut node, false, false);

        assert!(node.supports_action(accesskit::Action::Click));
        assert!(node.supports_action(accesskit::Action::Increment));
    }

    #[test]
    fn exact_empty_actions_override_legacy_inference() {
        let mut accessibility = InteractivityAccessibility {
            explicit_actions: Some(A11yActionSet::default()),
            ..Default::default()
        };
        accessibility.action_listeners.push((
            accesskit::Action::Increment,
            Box::new(|_, _, _| unreachable!("undeclared listeners must not run")),
        ));
        let mut node = accesskit::Node::new(accesskit::Role::Button);

        accessibility.write_node(&mut node, true, true);

        assert!(!node.supports_action(accesskit::Action::Click));
        assert!(!node.supports_action(accesskit::Action::Focus));
        assert!(!node.supports_action(accesskit::Action::Increment));
    }

    #[test]
    fn declared_and_listener_actions_share_disabled_and_read_only_policy() {
        let mut disabled = InteractivityAccessibility {
            explicit_actions: Some([accesskit::Action::Click].into_iter().collect()),
            disabled: Some(true),
            ..Default::default()
        };
        disabled.action_listeners.push((
            accesskit::Action::Increment,
            Box::new(|_, _, _| unreachable!("disabled listeners must not run")),
        ));
        let mut disabled_node = accesskit::Node::new(accesskit::Role::Button);

        disabled.write_node(&mut disabled_node, false, false);

        assert!(!disabled_node.supports_action(accesskit::Action::Click));
        assert!(!disabled_node.supports_action(accesskit::Action::Increment));

        let mut read_only = InteractivityAccessibility {
            explicit_actions: Some(
                [accesskit::Action::Click, accesskit::Action::SetValue]
                    .into_iter()
                    .collect(),
            ),
            read_only: Some(true),
            ..Default::default()
        };
        read_only.action_listeners.push((
            accesskit::Action::Increment,
            Box::new(|_, _, _| unreachable!("read-only mutation listeners must not run")),
        ));
        let mut read_only_node = accesskit::Node::new(accesskit::Role::Slider);

        read_only.write_node(&mut read_only_node, false, false);

        assert!(read_only_node.supports_action(accesskit::Action::Click));
        assert!(!read_only_node.supports_action(accesskit::Action::SetValue));
        assert!(!read_only_node.supports_action(accesskit::Action::Increment));
    }

    #[test]
    fn text_run_and_selection_properties_are_written_exactly() {
        let text_run_id = accesskit::NodeId(7);
        let selection = accesskit::TextSelection {
            anchor: accesskit::TextPosition {
                node: text_run_id,
                character_index: 2,
            },
            focus: accesskit::TextPosition {
                node: text_run_id,
                character_index: 1,
            },
        };
        let text_run_accessibility = InteractivityAccessibility {
            character_lengths: Some(vec![1, 4, 3]),
            ..Default::default()
        };
        let control_accessibility = InteractivityAccessibility {
            text_selection: Some(selection),
            ..Default::default()
        };
        let mut text_run = accesskit::Node::new(accesskit::Role::TextRun);
        let mut control = accesskit::Node::new(accesskit::Role::TextInput);

        text_run_accessibility.write_node(&mut text_run, false, false);
        control_accessibility.write_node(&mut control, false, false);

        assert_eq!(text_run.character_lengths(), &[1, 4, 3]);
        assert_eq!(control.text_selection(), Some(&selection));
    }

    #[test]
    fn live_region_properties_are_written_exactly() {
        let accessibility = InteractivityAccessibility {
            busy: Some(true),
            live: Some(accesskit::Live::Assertive),
            live_atomic: Some(true),
            ..Default::default()
        };
        let mut node = accesskit::Node::new(accesskit::Role::Alert);

        accessibility.write_node(&mut node, false, false);

        assert!(node.is_busy());
        assert_eq!(node.live(), Some(accesskit::Live::Assertive));
        assert!(node.is_live_atomic());
    }
}
