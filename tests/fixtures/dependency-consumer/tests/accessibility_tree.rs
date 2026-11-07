//! Type-only public synthetic accessibility and explicit raw inspection consumers.
//!
//! This fixture has no `test-support` feature or direct AccessKit dependency. Its closures are
//! deliberately unexecuted: live activation, publication and teardown are covered by the
//! framework's crate-external `accessibility_tree` test target.

use open_gpui::{
    A11ySubtreeBuilder, App, Bounds, Div, Element, ElementId, GlobalElementId, InspectorElementId,
    InteractiveElement as _, IntoElement, LayoutId, Pixels, Role, Stateful,
    StatefulInteractiveElement as _, Style, Window,
    accesskit::{Node, NodeId, TextPosition, TextSelection},
    div,
};

struct ConsumerText(String);

impl IntoElement for ConsumerText {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for ConsumerText {
    type RequestLayoutState = ();
    type PrepaintState = String;

    fn id(&self) -> Option<ElementId> {
        Some("independent-editor".into())
    }

    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        None
    }

    fn a11y_role(&self) -> Option<Role> {
        Some(Role::TextInput)
    }

    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        (window.request_layout(Style::default(), [], cx), ())
    }

    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        _: &mut Self::RequestLayoutState,
        _: &mut Window,
        _: &mut App,
    ) -> Self::PrepaintState {
        self.0.clone()
    }

    fn a11y_synthetic_children(
        &mut self,
        text: &mut Self::PrepaintState,
        builder: &mut A11ySubtreeBuilder<'_>,
    ) {
        let id: NodeId = builder.synthetic_node_id(("text", 0_usize));
        let mut run = Node::new(Role::TextRun);
        run.set_value(text.as_str());
        run.set_character_lengths(
            text.chars()
                .map(|character| character.len_utf8() as u8)
                .collect::<Vec<_>>(),
        );
        let accepted: bool = builder.push_child(id, run);
        if accepted {
            let parent: &mut Node = builder.parent_node();
            parent.set_value(std::mem::take(text));
            parent.set_labelled_by([id]);
            parent.set_text_selection(TextSelection {
                anchor: TextPosition {
                    node: id,
                    character_index: 0,
                },
                focus: TextPosition {
                    node: id,
                    character_index: 0,
                },
            });
        }
    }

    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        _: &mut Self::RequestLayoutState,
        _: &mut Self::PrepaintState,
        _: &mut Window,
        _: &mut App,
    ) {
    }
}

#[test]
fn downstream_custom_element_and_builder_contract_are_type_only() {
    let _: fn(&mut ConsumerText, &mut String, &mut A11ySubtreeBuilder<'_>) =
        <ConsumerText as Element>::a11y_synthetic_children;
    let _type_only = |builder: &mut A11ySubtreeBuilder<'_>, key: String, node: Node| {
        let id: NodeId = builder.synthetic_node_id(key);
        let accepted: bool = builder.push_child(id, node);
        let parent: &mut Node = builder.parent_node();
        parent.set_description("Explicit downstream parent metadata");
        accepted
    };
    let _: ConsumerText = ConsumerText("Independent α🦀 text".into()).into_element();
}

#[test]
fn downstream_stateful_div_callback_and_raw_inspection_are_type_only() {
    let _: fn(&mut Div, &mut <Div as Element>::PrepaintState, &mut A11ySubtreeBuilder<'_>) =
        <Div as Element>::a11y_synthetic_children;
    let _: fn(
        &mut Stateful<Div>,
        &mut <Stateful<Div> as Element>::PrepaintState,
        &mut A11ySubtreeBuilder<'_>,
    ) = <Stateful<Div> as Element>::a11y_synthetic_children;
    let owned_once = String::from("owned FnOnce callback payload");
    let _type_only_element = div()
        .id("independent-synthetic-owner")
        .role(Role::Group)
        .a11y_synthetic_children(move |builder| {
            let id = builder.synthetic_node_id(("owned", 0_usize));
            let mut child = Node::new(Role::Label);
            child.set_label(owned_once);
            let _: bool = builder.push_child(id, child);
            let _: &mut Node = builder.parent_node();
        });
    let _: fn(&Window) -> Option<String> = Window::debug_a11y_tree_json;
    let _type_only = |window: &Window| -> Option<String> { window.debug_a11y_tree_json() };
}
