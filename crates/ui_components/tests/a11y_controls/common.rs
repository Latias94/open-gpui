#[path = "../support/a11y.rs"]
mod a11y_support;

pub(super) use open_gpui::prelude::FluentBuilder;
pub(super) use open_gpui::{
    App, AppContext, Context, InteractiveElement, IntoElement, ParentElement, Render, RenderOnce,
    Window, accesskit, div,
};
pub(super) use open_gpui_ui_components::gpui_adapter::{
    FieldControl, FieldControlSemantics, TextInputController, UiA11yElementExt, init_text_input,
};
pub(super) use open_gpui_ui_components::{
    Checkbox, Field, FormControlState, IconButton, Label, NumberInput, NumberInputChange,
    NumberInputStepAction, Progress, Slider, SliderChange, Switch, TextInput, TextInputDisplayMode,
    Textarea, Toggle,
};
pub(super) use open_gpui_ui_core::{Role, SemanticDescriptor, Size, Toggled};
pub(super) use std::{cell::RefCell, rc::Rc};

pub(super) use a11y_support::{assert_exact_actions, node_with_label as a11y_node_with_label};

#[derive(IntoElement)]
pub(super) struct ExternalFieldControl {
    id: &'static str,
    state: FormControlState,
    field_semantics: Option<FieldControlSemantics>,
}

impl ExternalFieldControl {
    pub(super) fn new(id: &'static str, state: FormControlState) -> Self {
        Self {
            id,
            state,
            field_semantics: None,
        }
    }
}

impl FieldControl for ExternalFieldControl {
    fn field_control_state(&self) -> FormControlState {
        self.state
    }

    fn with_field_semantics(mut self, semantics: FieldControlSemantics) -> Self {
        self.field_semantics = Some(semantics);
        self
    }
}

impl RenderOnce for ExternalFieldControl {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        let field_semantics = self
            .field_semantics
            .expect("FieldControl should receive renderer-scoped semantics");
        let state = field_semantics.apply_control_state(self.state);
        let labelled_by = [field_semantics.labelled_by()];
        let described_by = field_semantics
            .described_by()
            .into_iter()
            .collect::<Vec<_>>();
        let error_message = field_semantics.error_message();
        let mut semantics = SemanticDescriptor::new(Role::TextInput)
            .with_labelled_by(&labelled_by)
            .with_described_by(&described_by)
            .with_required(state.required())
            .with_invalid(state.invalid())
            .with_busy(state.busy())
            .with_disabled(state.disabled());
        if let Some(error_message) = error_message.as_ref() {
            semantics = semantics.with_error_message(error_message);
        }

        div()
            .id(self.id)
            .ui_semantics_with_relations(&semantics, |node_id| *node_id)
    }
}

pub(super) fn a11y_node_by_id(
    update: &accesskit::TreeUpdate,
    id: accesskit::NodeId,
) -> &accesskit::Node {
    update
        .nodes
        .iter()
        .find(|(node_id, _)| *node_id == id)
        .map(|(_, node)| node)
        .unwrap_or_else(|| panic!("missing accessibility node {id:?}"))
}

pub(super) fn a11y_text_run_child<'a>(
    update: &'a accesskit::TreeUpdate,
    control: &accesskit::Node,
) -> (accesskit::NodeId, &'a accesskit::Node) {
    a11y_text_run_children(update, control)
        .into_iter()
        .next()
        .expect("text control should publish a TextRun child")
}

pub(super) fn a11y_text_run_children<'a>(
    update: &'a accesskit::TreeUpdate,
    control: &accesskit::Node,
) -> Vec<(accesskit::NodeId, &'a accesskit::Node)> {
    control
        .children()
        .iter()
        .filter_map(|id| {
            let node = a11y_node_by_id(update, *id);
            (node.role() == accesskit::Role::TextRun).then_some((*id, node))
        })
        .collect()
}

pub(super) fn assert_tree_excludes_text(update: &accesskit::TreeUpdate, canary: &str) {
    let snapshot = format!("{update:#?}");
    assert!(
        !snapshot.contains(canary),
        "accessibility tree leaked canary {canary:?}"
    );
}

pub(super) fn action_request(
    action: accesskit::Action,
    target_node: accesskit::NodeId,
    data: Option<accesskit::ActionData>,
) -> accesskit::ActionRequest {
    accesskit::ActionRequest {
        action,
        target_tree: accesskit::TreeId::ROOT,
        target_node,
        data,
    }
}
