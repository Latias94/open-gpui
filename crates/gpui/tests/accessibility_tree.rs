#![cfg(feature = "test-support")]

use open_gpui::{
    A11ySubtreeBuilder, AccessibleAction, AnyElement, App, Bounds, Context, Element, ElementId,
    Empty, FocusHandle, GlobalElementId, InspectorElementId, InteractiveElement as _, IntoElement,
    LayoutId, ParentElement as _, Pixels, Render, Role, StatefulInteractiveElement as _,
    Styled as _, TestAppContext, VisualTestContext, Window, WindowHandle,
    accesskit::{ActionRequest, Node, NodeId, TextPosition, TextSelection, TreeId, TreeUpdate},
    div, px, size,
};
use serde_json::Value;
use std::{cell::Cell, collections::HashSet, panic::Location, rc::Rc};

const RAW_DESCRIPTION: &str = "Explicit inspection: private \"editor\" note\nsecond line";
const RAW_TITLE: &str = "Explicit inspection: private window title";
const SECOND_RUN: &str = "tail β";

struct InvalidClipView {
    hooks: Rc<Cell<usize>>,
    fail_during_paint: bool,
}

impl Render for InvalidClipView {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let hooks = self.hooks.clone();
        let late = self.fail_during_paint;
        div()
            .id("invalid-synthetic-clip")
            .role(Role::Group)
            .size_full()
            .overflow_hidden()
            .rounded(open_gpui::rems(if late { 0.5 } else { f32::MAX }))
            .a11y_synthetic_children(move |builder| {
                hooks.set(hooks.get() + 1);
                let id = builder.synthetic_node_id("rejected");
                let mut child = Node::new(Role::Label);
                child.set_value("invalid-clip-synthetic-canary");
                assert!(builder.push_child(id, child));
            })
            .child(
                open_gpui::canvas(
                    |_, _, _| {},
                    move |_, (), window, _| {
                        if late {
                            window.paint_quad(open_gpui::fill(
                                Bounds::new(
                                    open_gpui::point(px(f32::MAX), px(0.0)),
                                    size(px(10.0), px(10.0)),
                                ),
                                open_gpui::white(),
                            ));
                        }
                    },
                )
                .size_full(),
            )
    }
}

#[open_gpui::test]
fn public_invalid_overflow_clip_suppresses_synthetic_children(cx: &mut TestAppContext) {
    assert_clip_suppresses_synthetic_children(cx, false);
}

#[open_gpui::test]
fn public_late_clip_failure_filters_synthetic_children_and_provenance(cx: &mut TestAppContext) {
    assert_clip_suppresses_synthetic_children(cx, true);
}

fn assert_clip_suppresses_synthetic_children(cx: &mut TestAppContext, late: bool) {
    let hooks = Rc::new(Cell::new(0));
    let handle = cx.open_window(size(px(100.0), px(100.0)), {
        let hooks = hooks.clone();
        move |_, _| InvalidClipView {
            hooks,
            fail_during_paint: late,
        }
    });
    let mut visual = VisualTestContext::from_window(handle.into(), cx);
    assert!(visual.activate_accessibility());
    if late {
        assert!(
            hooks.get() > 0,
            "late invalidation must follow an actual synthetic prepaint"
        );
    } else {
        assert_eq!(
            hooks.get(),
            0,
            "invalid descendant clip must suppress the synthetic hook"
        );
    }
    let tree = visual.latest_accessibility_tree_update().unwrap();
    assert!(
        tree.nodes
            .iter()
            .all(|(_, node)| node.value() != Some("invalid-clip-synthetic-canary"))
    );
    assert!(
        !visual
            .update(|window, _| window.debug_a11y_tree_json().unwrap())
            .contains("invalid-clip-synthetic-canary")
    );
}

struct QuiescingView {
    quiesce_on_commit: bool,
    publication: open_gpui::PrepaintPublicationId,
}

impl Render for QuiescingView {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let quiesce = self.quiesce_on_commit;
        let publication = self.publication;
        div()
            .id("quiescing-synthetic-owner")
            .role(Role::Group)
            .size_full()
            .a11y_synthetic_children(|builder| {
                let id = builder.synthetic_node_id("quiescing-child");
                let mut child = Node::new(Role::Label);
                child.set_value("quiesced-synthetic-canary");
                assert!(builder.push_child(id, child));
            })
            .child(
                open_gpui::canvas(
                    move |_, window, _| {
                        if quiesce {
                            window.record_prepaint_window_transaction(
                                publication,
                                |_, window, app| {
                                    window.quiesce_interaction(app);
                                },
                                |_, _, _| {},
                            );
                        }
                    },
                    |_, (), _, _| {},
                )
                .size_full(),
            )
    }
}

#[open_gpui::test]
fn public_quiescence_keeps_inspection_cleared_across_pending_and_inert_publications(
    cx: &mut TestAppContext,
) {
    let handle = cx.open_window(size(px(100.0), px(100.0)), |_, _| QuiescingView {
        quiesce_on_commit: false,
        publication: open_gpui::PrepaintPublicationId::new(),
    });
    let mut visual = VisualTestContext::from_window(handle.into(), cx);
    assert!(visual.activate_accessibility());
    assert!(
        visual
            .update(|window, _| window.debug_a11y_tree_json())
            .is_some()
    );
    handle
        .update(cx, |view, _, app| {
            view.quiesce_on_commit = true;
            app.notify();
        })
        .unwrap();
    visual.run_until_parked();
    assert!(
        visual
            .update(|window, _| window.debug_a11y_tree_json())
            .is_none(),
        "accepted pre-quiescence candidate must not recapture raw content"
    );
    visual.update(|window, app| {
        window.draw(app).clear(app);
    });
    assert!(
        visual
            .update(|window, _| window.debug_a11y_tree_json())
            .is_none(),
        "later inert publication must not rebuild inspection state"
    );
}

#[derive(Default)]
struct Observations {
    renders: Cell<usize>,
    custom: Cell<usize>,
    div: Cell<usize>,
    stateful: Cell<usize>,
}

impl Observations {
    fn counts(&self) -> [usize; 4] {
        [
            self.renders.get(),
            self.custom.get(),
            self.div.get(),
            self.stateful.get(),
        ]
    }
}

struct PreparedText {
    first: String,
    include_second: bool,
}

struct SyntheticEditor {
    revision: usize,
    include_second: bool,
    observed: Rc<Observations>,
    source: &'static Location<'static>,
}

impl IntoElement for SyntheticEditor {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for SyntheticEditor {
    type RequestLayoutState = AnyElement;
    type PrepaintState = PreparedText;

    fn id(&self) -> Option<ElementId> {
        Some("custom-editor".into())
    }

    fn source_location(&self) -> Option<&'static Location<'static>> {
        Some(self.source)
    }

    fn a11y_role(&self) -> Option<Role> {
        Some(Role::TextInput)
    }

    fn write_a11y_info(&self, node: &mut Node) {
        node.set_label("Custom editor");
    }

    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let mut child = div()
            .id("ordinary-editor-child")
            .role(Role::Label)
            .aria_label("Ordinary editor child")
            .w(px(120.0))
            .h(px(24.0))
            .into_any_element();
        (child.request_layout(window, cx), child)
    }

    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        child: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        child.prepaint(window, cx);
        PreparedText {
            first: format!("revision {}: α🦀", self.revision),
            include_second: self.include_second,
        }
    }

    fn a11y_synthetic_children(
        &mut self,
        prepared: &mut Self::PrepaintState,
        builder: &mut A11ySubtreeBuilder<'_>,
    ) {
        self.observed.custom.set(self.observed.custom.get() + 1);
        let first_id = builder.synthetic_node_id(("line", 0_usize));
        let second_id = builder.synthetic_node_id(("line", 1_usize));
        assert_eq!(first_id, builder.synthetic_node_id(("line", 0_usize)));
        assert_ne!(first_id, second_id);

        let first_value = std::mem::take(&mut prepared.first);
        let mut first = text_run("First run", &first_value);
        let focus = if prepared.include_second {
            first.set_next_on_line(second_id);
            TextPosition {
                node: second_id,
                character_index: 2,
            }
        } else {
            TextPosition {
                node: first_id,
                character_index: 1,
            }
        };
        assert!(builder.push_child(first_id, first));
        if prepared.include_second {
            let mut second = text_run("Second run", SECOND_RUN);
            second.set_previous_on_line(first_id);
            assert!(builder.push_child(second_id, second));
        }
        #[cfg(not(debug_assertions))]
        {
            let mut duplicate = Node::new(Role::Label);
            duplicate.set_label("Discarded duplicate");
            assert!(!builder.push_child(first_id, duplicate));
        }

        let parent = builder.parent_node();
        parent.set_description(RAW_DESCRIPTION);
        parent.set_labelled_by([first_id]);
        parent.set_controls(if prepared.include_second {
            vec![second_id, first_id]
        } else {
            vec![first_id]
        });
        if prepared.include_second {
            parent.set_described_by([second_id]);
        }
        parent.set_value(if prepared.include_second {
            format!("{first_value}{SECOND_RUN}")
        } else {
            first_value
        });
        parent.set_text_selection(TextSelection {
            anchor: TextPosition {
                node: first_id,
                character_index: 1,
            },
            focus,
        });
    }

    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        child: &mut Self::RequestLayoutState,
        _: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        child.paint(window, cx);
    }
}

fn text_run(label: &str, value: &str) -> Node {
    let mut node = Node::new(Role::TextRun);
    node.set_label(label);
    node.set_value(value);
    node.set_character_lengths(
        value
            .chars()
            .map(|character| character.len_utf8() as u8)
            .collect::<Vec<_>>(),
    );
    node
}

struct SyntheticView {
    revision: usize,
    include_second: bool,
    observed: Rc<Observations>,
    lifetime: Rc<()>,
    focus: FocusHandle,
}

impl Render for SyntheticView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.observed.renders.set(self.observed.renders.get() + 1);
        let div_observed = Rc::clone(&self.observed);
        let div_once = Rc::clone(&self.lifetime);
        let stateful_observed = Rc::clone(&self.observed);
        let stateful_once = Rc::clone(&self.lifetime);
        let roleless_once = Rc::clone(&self.lifetime);
        let this = cx.entity().downgrade();

        div()
            .id("synthetic-root")
            .role(Role::Group)
            .aria_label("Synthetic root")
            .flex()
            .flex_col()
            .size_full()
            .child(
                div()
                    .id("before")
                    .role(Role::Label)
                    .aria_label("Before custom")
                    .h(px(16.0)),
            )
            .child(SyntheticEditor {
                revision: self.revision,
                include_second: self.include_second,
                observed: Rc::clone(&self.observed),
                source: Location::caller(),
            })
            .child(
                div()
                    .id("div-owner")
                    .role(Role::Group)
                    .aria_label("Div callback owner")
                    .h(px(24.0))
                    .a11y_synthetic_children(move |builder| {
                        drop(div_once);
                        div_observed.div.set(div_observed.div.get() + 1);
                        let id = builder.synthetic_node_id(("line", 0_usize));
                        let mut child = Node::new(Role::Label);
                        child.set_label("Div synthetic child");
                        assert!(builder.push_child(id, child));
                        builder.parent_node().set_description("Div parent metadata");
                    }),
            )
            .child(
                div()
                    .id("stateful-owner")
                    .role(Role::Button)
                    .aria_label("Stateful callback owner")
                    .aria_actions([AccessibleAction::Click])
                    .track_accessibility_focus(&self.focus)
                    .h(px(24.0))
                    .a11y_synthetic_children(move |builder| {
                        drop(stateful_once);
                        stateful_observed
                            .stateful
                            .set(stateful_observed.stateful.get() + 1);
                        let id = builder.synthetic_node_id(("line", 0_usize));
                        let mut child = Node::new(Role::Label);
                        child.set_label("Stateful synthetic child");
                        assert!(builder.push_child(id, child));
                        builder
                            .parent_node()
                            .set_description("Stateful parent metadata");
                    })
                    .on_a11y_action(AccessibleAction::Click, move |_, _, cx| {
                        this.update(cx, |view, cx| {
                            view.revision += 1;
                            view.include_second = false;
                            cx.notify();
                        })
                        .expect("the mounted callback owner must be alive");
                    }),
            )
            .child(
                div()
                    .id("after")
                    .role(Role::Label)
                    .aria_label("After custom")
                    .h(px(16.0)),
            )
            .child(div().id("without-role").a11y_synthetic_children(move |_| {
                drop(roleless_once);
                panic!("a roleless element must not run the hook");
            }))
    }
}

fn open_window(
    cx: &mut TestAppContext,
) -> (
    WindowHandle<SyntheticView>,
    VisualTestContext,
    Rc<Observations>,
) {
    let observed = Rc::new(Observations::default());
    let handle = cx.open_window(size(px(320.0), px(240.0)), |window, cx| {
        assert_eq!(window.debug_a11y_tree_json(), None);
        window.set_window_title(RAW_TITLE);
        SyntheticView {
            revision: 0,
            include_second: true,
            observed: Rc::clone(&observed),
            lifetime: Rc::new(()),
            focus: cx.focus_handle(),
        }
    });
    let visual = VisualTestContext::from_window(handle.into(), cx);
    visual.run_until_parked();
    (handle, visual, observed)
}

fn labelled<'a>(tree: &'a TreeUpdate, label: &str) -> (NodeId, &'a Node) {
    let mut matches = tree
        .nodes
        .iter()
        .filter(|(_, node)| node.label() == Some(label));
    let (id, node) = matches
        .next()
        .unwrap_or_else(|| panic!("missing node labelled {label:?}"));
    assert!(matches.next().is_none(), "duplicate label {label:?}");
    (*id, node)
}

fn exported_node(snapshot: &Value, id: NodeId) -> (&str, &Value) {
    let raw_id = id.0.to_string();
    snapshot["nodes"]
        .as_object()
        .unwrap()
        .iter()
        .find(|(_, node)| node["accesskit_id"].as_str() == Some(raw_id.as_str()))
        .map(|(key, node)| (key.as_str(), node))
        .unwrap_or_else(|| panic!("missing exported node {id:?}"))
}

fn published_snapshot(visual: &mut VisualTestContext) -> (TreeUpdate, Value) {
    let tree = visual
        .latest_accessibility_tree_update()
        .expect("published platform tree");
    let (snapshot, generation, viewport, scale) = visual.update(|window, _| {
        let raw = window
            .debug_a11y_tree_json()
            .expect("published raw inspection tree");
        (
            serde_json::from_str::<Value>(&raw).expect("valid official inspection JSON"),
            window
                .presentation_facts()
                .frame_accepted_generation
                .expect("a published tree must belong to an accepted frame"),
            window.viewport_size(),
            window.scale_factor(),
        )
    });
    assert_eq!(snapshot.as_object().unwrap().len(), 5);
    let nodes = snapshot["nodes"].as_object().unwrap();
    assert_eq!(nodes.len(), tree.nodes.len());
    assert_eq!(
        snapshot["frame"]["node_count"].as_u64(),
        Some(tree.nodes.len() as u64)
    );
    assert_eq!(
        snapshot["frame"]["frame_generation"].as_u64(),
        Some(generation)
    );
    assert!(snapshot["frame"]["activation_generation"].as_u64().unwrap() > 0);
    assert_eq!(
        snapshot["frame"]["frame_number"].as_u64(),
        Some(visual.accessibility_tree_update_history().len() as u64)
    );
    assert!(
        snapshot["frame"]["rendered_at"]
            .as_str()
            .is_some_and(|value| !value.is_empty())
    );
    assert_eq!(snapshot["frame"]["window_title"], RAW_TITLE);
    assert_eq!(snapshot["frame"]["tab_stop_count"], 0);
    assert_eq!(
        snapshot["frame"]["viewport_size"]["width"].as_f64(),
        Some(f64::from(viewport.width))
    );
    assert_eq!(
        snapshot["frame"]["viewport_size"]["height"].as_f64(),
        Some(f64::from(viewport.height))
    );
    assert_eq!(
        snapshot["frame"]["scale_factor"].as_f64(),
        Some(scale as f64)
    );
    let root = tree
        .tree
        .as_ref()
        .expect("a complete tree must include its root")
        .root;
    assert_eq!(snapshot["root"], exported_node(&snapshot, root).0);

    let ids = tree.nodes.iter().map(|(id, _)| *id).collect::<HashSet<_>>();
    assert_eq!(ids.len(), tree.nodes.len());
    assert!(ids.contains(&tree.focus));
    for (id, node) in &tree.nodes {
        let (key, exported) = exported_node(&snapshot, *id);
        assert!(!key.is_empty() && key.bytes().all(|byte| byte.is_ascii_lowercase()));
        assert_eq!(exported["aria"]["role"], format!("{:?}", node.role()));
        for (property, expected) in [
            ("label", node.label()),
            ("description", node.description()),
            ("value", node.value()),
        ] {
            assert_eq!(
                exported["aria"].get(property).and_then(Value::as_str),
                expected
            );
        }
        let children = node
            .children()
            .iter()
            .map(|child| exported_node(&snapshot, *child).0)
            .collect::<Vec<_>>();
        if children.is_empty() {
            assert!(exported.get("children").is_none());
        } else {
            assert_eq!(exported["children"], serde_json::json!(children));
        }
        for related in node
            .children()
            .iter()
            .chain(node.labelled_by())
            .chain(node.described_by())
            .chain(node.controls())
        {
            assert!(
                ids.contains(related),
                "dangling relation from {id:?} to {related:?}"
            );
        }
        #[cfg(not(debug_assertions))]
        for property in ["element_id", "view", "source_location", "synthetic"] {
            assert!(exported.get(property).is_none());
        }
    }
    (tree, snapshot)
}

fn assert_reads_have_no_effects(visual: &mut VisualTestContext, observed: &Observations) {
    let counts = observed.counts();
    let history = visual.accessibility_tree_update_history();
    let scheduled = visual.frame_scheduled();
    let facts = visual.update(|window, _| window.presentation_facts());
    let before = visual.update(|window, _| window.debug_a11y_tree_json());
    for _ in 0..3 {
        assert_eq!(
            visual.update(|window, _| window.debug_a11y_tree_json()),
            before
        );
    }
    visual.run_until_parked();
    assert_eq!(observed.counts(), counts);
    assert_eq!(visual.accessibility_tree_update_history(), history);
    assert_eq!(visual.frame_scheduled(), scheduled);
    assert_eq!(
        visual.update(|window, _| window.presentation_facts()),
        facts
    );
}

#[open_gpui::test]
fn public_synthetic_hooks_publish_text_relations_source_order_and_updates(cx: &mut TestAppContext) {
    let (handle, mut visual, observed) = open_window(cx);
    assert!(visual.latest_accessibility_tree_update().is_none());
    assert_eq!(observed.counts()[1..], [0, 0, 0]);
    assert_reads_have_no_effects(&mut visual, &observed);
    assert!(visual.activate_accessibility());
    let (initial, json) = published_snapshot(&mut visual);
    assert!(observed.counts()[1..].iter().all(|count| *count > 0));

    let (root_id, root) = labelled(&initial, "Synthetic root");
    let (editor_id, editor) = labelled(&initial, "Custom editor");
    let (first_id, first) = labelled(&initial, "First run");
    let (second_id, second) = labelled(&initial, "Second run");
    let (div_id, div_owner) = labelled(&initial, "Div callback owner");
    let (stateful_id, stateful_owner) = labelled(&initial, "Stateful callback owner");
    let div_child = labelled(&initial, "Div synthetic child").0;
    let stateful_child = labelled(&initial, "Stateful synthetic child").0;
    assert_eq!(
        root.children(),
        [
            labelled(&initial, "Before custom").0,
            editor_id,
            div_id,
            stateful_id,
            labelled(&initial, "After custom").0,
        ]
    );
    assert_eq!(
        editor.children(),
        [
            labelled(&initial, "Ordinary editor child").0,
            first_id,
            second_id,
        ]
    );
    assert_eq!(div_owner.children(), [div_child]);
    assert_eq!(stateful_owner.children(), [stateful_child]);
    assert_eq!(
        HashSet::from([first_id, div_child, stateful_child]).len(),
        3
    );
    assert_eq!(editor.description(), Some(RAW_DESCRIPTION));
    assert_eq!(editor.value(), Some("revision 0: α🦀tail β"));
    assert_eq!(editor.labelled_by(), [first_id]);
    assert_eq!(editor.described_by(), [second_id]);
    assert_eq!(editor.controls(), [second_id, first_id]);
    assert_eq!(first.role(), Role::TextRun);
    assert_eq!(first.value(), Some("revision 0: α🦀"));
    assert_eq!(
        first.character_lengths(),
        [1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 2, 4]
    );
    assert_eq!(second.character_lengths(), [1, 1, 1, 1, 1, 2]);
    assert_eq!(first.next_on_line(), Some(second_id));
    assert_eq!(second.previous_on_line(), Some(first_id));
    assert_eq!(
        editor.text_selection(),
        Some(&TextSelection {
            anchor: TextPosition {
                node: first_id,
                character_index: 1,
            },
            focus: TextPosition {
                node: second_id,
                character_index: 2,
            },
        })
    );
    assert_eq!(div_owner.description(), Some("Div parent metadata"));
    assert_eq!(
        stateful_owner.description(),
        Some("Stateful parent metadata")
    );
    assert_eq!(initial.focus, stateful_id);
    assert_eq!(json["gpui_focus"], exported_node(&json, stateful_id).0);
    assert!(json["active_descendant_focus"].is_null());
    assert_eq!(
        exported_node(&json, editor_id).1["aria"]["description"],
        RAW_DESCRIPTION
    );
    assert_eq!(
        exported_node(&json, stateful_id).1["aria"]["on_action"],
        serde_json::json!(["Click"])
    );
    assert!(
        initial
            .nodes
            .iter()
            .all(|(_, node)| node.label() != Some("Discarded duplicate"))
    );

    handle.update(cx, |_, _, cx| cx.notify()).unwrap();
    visual.run_until_parked();
    let (equivalent, equivalent_json) = published_snapshot(&mut visual);
    assert_eq!(equivalent, initial);
    assert!(
        equivalent_json["frame"]["frame_generation"].as_u64()
            > json["frame"]["frame_generation"].as_u64()
    );

    let history_before = visual.accessibility_tree_update_history().len();
    let counts_before = observed.counts();
    assert!(visual.dispatch_accessibility_action(ActionRequest {
        action: AccessibleAction::Click,
        target_tree: TreeId::ROOT,
        target_node: stateful_id,
        data: None,
    }));
    assert_eq!(handle.read_with(cx, |view, _| view.revision).unwrap(), 1);
    let (changed, changed_json) = published_snapshot(&mut visual);
    assert!(visual.accessibility_tree_update_history().len() > history_before);
    assert!(
        observed
            .counts()
            .iter()
            .zip(counts_before)
            .all(|(after, before)| *after > before)
    );
    assert_eq!(labelled(&changed, "Synthetic root").0, root_id);
    assert_eq!(labelled(&changed, "Custom editor").0, editor_id);
    assert_eq!(labelled(&changed, "First run").0, first_id);
    assert_eq!(labelled(&changed, "Div synthetic child").0, div_child);
    assert_eq!(
        labelled(&changed, "Stateful synthetic child").0,
        stateful_child
    );
    assert!(!changed.nodes.iter().any(|(id, _)| *id == second_id));
    let (_, editor) = labelled(&changed, "Custom editor");
    assert_eq!(editor.value(), Some("revision 1: α🦀"));
    assert_eq!(
        editor.children(),
        [labelled(&changed, "Ordinary editor child").0, first_id]
    );
    assert!(editor.described_by().is_empty());
    assert_eq!(editor.controls(), [first_id]);
    assert_eq!(labelled(&changed, "First run").1.next_on_line(), None);
    assert_eq!(editor.text_selection().unwrap().focus.node, first_id);
    assert_eq!(
        changed_json["frame"]["activation_generation"],
        json["frame"]["activation_generation"]
    );
    assert!(
        changed_json["frame"]["frame_generation"].as_u64()
            > equivalent_json["frame"]["frame_generation"].as_u64()
    );
    assert_reads_have_no_effects(&mut visual, &observed);
}

#[open_gpui::test]
fn public_raw_inspection_clears_at_deactivation_and_reactivates_with_fresh_content(
    cx: &mut TestAppContext,
) {
    let (handle, mut visual, observed) = open_window(cx);
    assert!(visual.activate_accessibility());
    let (initial, initial_json) = published_snapshot(&mut visual);
    let first_id = labelled(&initial, "First run").0;
    let history_before = visual.accessibility_tree_update_history().len();
    assert!(visual.deactivate_accessibility());
    assert_eq!(
        visual.update(|window, _| window.debug_a11y_tree_json()),
        None
    );
    let hooks_before = observed.counts();
    handle
        .update(cx, |view, _, cx| {
            view.revision = 7;
            view.include_second = false;
            cx.notify();
        })
        .unwrap();
    visual.run_until_parked();
    assert_eq!(observed.counts()[1..], hooks_before[1..]);
    assert_eq!(
        visual.accessibility_tree_update_history().len(),
        history_before
    );
    assert_eq!(
        visual.update(|window, _| window.debug_a11y_tree_json()),
        None
    );
    assert_reads_have_no_effects(&mut visual, &observed);

    assert!(visual.activate_accessibility());
    let (reactivated, reactivated_json) = published_snapshot(&mut visual);
    assert_eq!(labelled(&reactivated, "First run").0, first_id);
    assert_eq!(
        labelled(&reactivated, "Custom editor").1.value(),
        Some("revision 7: α🦀")
    );
    assert_eq!(
        exported_node(&reactivated_json, first_id).1["aria"]["value"],
        "revision 7: α🦀"
    );
    assert!(
        reactivated
            .nodes
            .iter()
            .all(|(_, node)| node.label() != Some("Second run"))
    );
    assert!(
        reactivated_json["frame"]["activation_generation"].as_u64()
            > initial_json["frame"]["activation_generation"].as_u64()
    );
    assert_reads_have_no_effects(&mut visual, &observed);

    assert!(visual.activate_accessibility());
    let (repeated, repeated_json) = published_snapshot(&mut visual);
    assert_eq!(labelled(&repeated, "First run").0, first_id);
    assert!(
        repeated_json["frame"]["activation_generation"].as_u64()
            > reactivated_json["frame"]["activation_generation"].as_u64()
    );
}

#[open_gpui::test]
fn public_inspection_releases_removed_roots_and_rebuilds_without_stale_nodes(
    cx: &mut TestAppContext,
) {
    let (handle, mut visual, observed) = open_window(cx);
    let old_view = handle.root(cx).unwrap();
    let old_weak = old_view.downgrade();
    let old_lifetime = cx.read(|app| Rc::downgrade(&old_view.read(app).lifetime));
    drop(old_view);
    assert!(visual.activate_accessibility());
    let (initial, _) = published_snapshot(&mut visual);
    let platform_root = initial.tree.as_ref().unwrap().root;

    visual.update(|window, app| {
        window.replace_root(app, |_, _| Empty);
    });
    visual.run_until_parked();
    let (empty, _) = published_snapshot(&mut visual);
    assert_eq!(empty.nodes.len(), 1);
    assert_eq!(empty.tree.as_ref().unwrap().root, platform_root);
    assert!(empty.nodes[0].1.children().is_empty());
    assert!(old_weak.upgrade().is_none());
    assert!(old_lifetime.upgrade().is_none());

    let rebuilt = visual.update(|window, app| {
        window.replace_root(app, |_, cx| SyntheticView {
            revision: 9,
            include_second: false,
            observed: Rc::clone(&observed),
            lifetime: Rc::new(()),
            focus: cx.focus_handle(),
        })
    });
    let rebuilt_weak = rebuilt.downgrade();
    let rebuilt_lifetime = cx.read(|app| Rc::downgrade(&rebuilt.read(app).lifetime));
    drop(rebuilt);
    visual.run_until_parked();
    let (rebuilt_tree, _) = published_snapshot(&mut visual);
    assert_eq!(
        labelled(&rebuilt_tree, "Custom editor").1.value(),
        Some("revision 9: α🦀")
    );
    assert!(
        rebuilt_tree
            .nodes
            .iter()
            .all(|(_, node)| node.label() != Some("Second run"))
    );
    assert_reads_have_no_effects(&mut visual, &observed);
    let after_close = visual.update(|window, app| {
        window.remove_window(app);
        window.debug_a11y_tree_json()
    });
    assert_eq!(after_close, None);
    cx.run_until_parked();
    assert!(!cx.windows().contains(&handle.into()));
    assert!(rebuilt_weak.upgrade().is_none());
    assert!(rebuilt_lifetime.upgrade().is_none());
}

#[cfg(debug_assertions)]
#[open_gpui::test]
fn public_debug_json_attributes_synthetic_nodes_to_their_real_creator(cx: &mut TestAppContext) {
    let (_, mut visual, _) = open_window(cx);
    assert!(visual.activate_accessibility());
    let (tree, snapshot) = published_snapshot(&mut visual);
    for (parent_label, child_label) in [
        ("Custom editor", "First run"),
        ("Custom editor", "Second run"),
        ("Div callback owner", "Div synthetic child"),
        ("Stateful callback owner", "Stateful synthetic child"),
    ] {
        let parent = exported_node(&snapshot, labelled(&tree, parent_label).0).1;
        let child = exported_node(&snapshot, labelled(&tree, child_label).0).1;
        assert!(parent.get("synthetic").is_none());
        assert_eq!(child["synthetic"], true);
        assert!(
            parent["element_id"]
                .as_str()
                .is_some_and(|value| !value.is_empty())
        );
        assert_eq!(child["element_id"], parent["element_id"]);
        assert!(parent["view"].as_str().unwrap().contains("SyntheticView"));
        assert_eq!(child["view"], parent["view"]);
        assert!(
            parent["source_location"]
                .as_str()
                .unwrap()
                .contains("accessibility_tree.rs:")
        );
        assert_eq!(child["source_location"], parent["source_location"]);
    }
    let ordinary = exported_node(&snapshot, labelled(&tree, "Ordinary editor child").0).1;
    assert!(ordinary.get("synthetic").is_none());
    assert_ne!(
        ordinary["element_id"],
        exported_node(&snapshot, labelled(&tree, "Custom editor").0).1["element_id"]
    );
}
