use std::{cell::RefCell, rc::Rc};

use open_gpui::prelude::FluentBuilder;
use open_gpui::{
    BringIntoViewAlignment, BringIntoViewOptions, Context, IntoElement, ParentElement, Render,
    Styled, Window, accesskit, div, px,
};
use open_gpui_ui_components::{
    Listbox, Splitter, SplitterPanel, SplitterPanelDescriptor, Tree, TreeItemDescriptor,
    VirtualizedList, VirtualizedListItemDescriptor, listbox::ListboxOption,
};
use open_gpui_ui_core::ui_px;

use super::a11y_support::node_with_label;

fn dispatch_action(
    cx: &open_gpui::VisualTestContext,
    action: accesskit::Action,
    target_node: accesskit::NodeId,
) {
    assert!(cx.dispatch_accessibility_action(accesskit::ActionRequest {
        action,
        target_tree: accesskit::TreeId::ROOT,
        target_node,
        data: None,
    }));
}

fn assert_approx(actual: Option<f64>, expected: f64) {
    let actual = actual.expect("expected numeric accessibility metadata");
    assert!(
        (actual - expected).abs() <= 0.001,
        "expected {expected}, got {actual}"
    );
}

fn collapsed_splitter(
    id: &str,
    before_id: &str,
    after_id: &str,
    collapsed_before: bool,
    collapsed_min_fraction: f32,
    collapsed_fraction: f32,
) -> Splitter {
    let before = if collapsed_before {
        SplitterPanelDescriptor::new(before_id, 0.5)
            .min_fraction(collapsed_min_fraction)
            .collapsible(true)
            .collapsed(true)
            .collapsed_fraction(collapsed_fraction)
    } else {
        SplitterPanelDescriptor::new(before_id, 0.5).min_fraction(0.1)
    };
    let after = if collapsed_before {
        SplitterPanelDescriptor::new(after_id, 0.5).min_fraction(0.1)
    } else {
        SplitterPanelDescriptor::new(after_id, 0.5)
            .min_fraction(collapsed_min_fraction)
            .collapsible(true)
            .collapsed(true)
            .collapsed_fraction(collapsed_fraction)
    };

    Splitter::new(id)
        .panel(SplitterPanel::new(
            before,
            div().child(before_id.to_owned()),
        ))
        .panel(SplitterPanel::new(after, div().child(after_id.to_owned())))
}

fn assert_splitter_node(
    node: &accesskit::Node,
    expected_value: f64,
    expected_min: f64,
    expected_max: f64,
) {
    assert_eq!(node.role(), accesskit::Role::Splitter);
    assert_eq!(node.orientation(), Some(accesskit::Orientation::Horizontal));
    assert_approx(node.numeric_value(), expected_value);
    assert_approx(node.min_numeric_value(), expected_min);
    assert_approx(node.max_numeric_value(), expected_max);
    for action in [
        accesskit::Action::Focus,
        accesskit::Action::Increment,
        accesskit::Action::Decrement,
    ] {
        assert!(node.supports_action(action));
    }
    assert!(expected_min <= expected_value && expected_value <= expected_max);
}

#[open_gpui::test]
fn listbox_final_tree_and_click_action_follow_resolved_state(cx: &mut open_gpui::TestAppContext) {
    struct ListboxProbe {
        disabled: bool,
        empty: bool,
        show: bool,
        selections: Rc<RefCell<Vec<String>>>,
    }

    impl Render for ListboxProbe {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let selections = self.selections.clone();
            let options = (!self.empty).then(|| {
                [
                    ListboxOption::new("alpha", "Alpha"),
                    ListboxOption::separator("divider"),
                    ListboxOption::new("beta", "Beta"),
                    ListboxOption::new("gamma", "Gamma").disabled(true),
                ]
            });
            let listbox = Listbox::new("a11y-listbox", "Choices")
                .options(options.into_iter().flatten())
                .default_active("alpha")
                .disabled(self.disabled)
                .on_select(move |selection, _, _| {
                    selections.borrow_mut().push(selection.value().to_owned());
                });

            div()
                .size_full()
                .when(self.show, |this| this.child(listbox))
        }
    }

    let selections = Rc::new(RefCell::new(Vec::new()));
    let (view, cx) = cx.add_window_view(|_, _| ListboxProbe {
        disabled: false,
        empty: false,
        show: true,
        selections: selections.clone(),
    });

    assert!(cx.activate_accessibility());
    let initial = cx
        .latest_accessibility_tree_update()
        .expect("listbox accessibility tree should publish");
    let (root_id, root) = node_with_label(&initial, "Choices");
    assert_eq!(root.role(), accesskit::Role::ListBox);
    assert!(root.supports_action(accesskit::Action::Focus));

    let (_, alpha) = node_with_label(&initial, "Alpha");
    assert_eq!(alpha.role(), accesskit::Role::ListBoxOption);
    assert_eq!(alpha.is_selected(), Some(false));
    assert_eq!(alpha.position_in_set(), Some(1));
    assert_eq!(alpha.size_of_set(), Some(3));
    assert!(alpha.supports_action(accesskit::Action::Click));

    let separators = initial
        .nodes
        .iter()
        .filter(|(_, node)| node.role() == accesskit::Role::Group)
        .collect::<Vec<_>>();
    assert_eq!(separators.len(), 1);
    let (separator_id, separator) = separators[0];
    assert_eq!(separator.orientation(), None);
    assert_eq!(separator.numeric_value(), None);
    assert_eq!(separator.min_numeric_value(), None);
    assert_eq!(separator.max_numeric_value(), None);
    assert!(!separator.supports_action(accesskit::Action::Focus));
    assert!(!separator.supports_action(accesskit::Action::Click));
    assert!(!separator.supports_action(accesskit::Action::Increment));
    assert!(!separator.supports_action(accesskit::Action::Decrement));
    assert_eq!(separator.position_in_set(), None);
    assert_eq!(separator.size_of_set(), None);
    dispatch_action(cx, accesskit::Action::Click, *separator_id);
    assert!(selections.borrow().is_empty());

    let (beta_id, beta) = node_with_label(&initial, "Beta");
    assert_eq!(beta.position_in_set(), Some(2));
    assert_eq!(beta.size_of_set(), Some(3));
    assert!(beta.supports_action(accesskit::Action::Click));

    let (_, gamma) = node_with_label(&initial, "Gamma");
    assert!(gamma.is_disabled());
    assert!(!gamma.supports_action(accesskit::Action::Click));
    assert!(!gamma.supports_action(accesskit::Action::Focus));

    dispatch_action(cx, accesskit::Action::Click, beta_id);
    cx.run_until_parked();
    assert_eq!(selections.borrow().as_slice(), &["beta"]);

    let selected = cx
        .latest_accessibility_tree_update()
        .expect("listbox selection should publish");
    let (selected_beta_id, selected_beta) = node_with_label(&selected, "Beta");
    assert_eq!(selected_beta_id, beta_id);
    assert_eq!(selected_beta.is_selected(), Some(true));

    view.update(cx, |probe, cx| {
        probe.disabled = true;
        cx.notify();
    });
    cx.run_until_parked();
    let disabled = cx
        .latest_accessibility_tree_update()
        .expect("disabled listbox should publish");
    let (disabled_beta_id, disabled_beta) = node_with_label(&disabled, "Beta");
    assert_eq!(disabled_beta_id, beta_id);
    assert!(disabled_beta.is_disabled());
    assert!(!disabled_beta.supports_action(accesskit::Action::Click));
    dispatch_action(cx, accesskit::Action::Click, disabled_beta_id);
    assert_eq!(selections.borrow().as_slice(), &["beta"]);

    view.update(cx, |probe, cx| {
        probe.disabled = false;
        probe.empty = true;
        cx.notify();
    });
    cx.run_until_parked();
    let empty = cx
        .latest_accessibility_tree_update()
        .expect("empty listbox should publish");
    let (empty_root_id, empty_root) = node_with_label(&empty, "Choices");
    assert_eq!(empty_root_id, root_id);
    assert_eq!(empty_root.role(), accesskit::Role::ListBox);
    assert!(!empty_root.supports_action(accesskit::Action::Focus));
    assert!(!empty_root.supports_action(accesskit::Action::Click));
    assert!(!empty.nodes.iter().any(|(id, _)| *id == beta_id));
    assert!(
        empty
            .nodes
            .iter()
            .all(|(_, node)| node.role() != accesskit::Role::ListBoxOption)
    );
    dispatch_action(cx, accesskit::Action::Click, beta_id);
    assert_eq!(selections.borrow().as_slice(), &["beta"]);

    view.update(cx, |probe, cx| {
        probe.empty = false;
        cx.notify();
    });
    cx.run_until_parked();
    let repopulated = cx
        .latest_accessibility_tree_update()
        .expect("repopulated listbox should publish");
    let (repopulated_root_id, repopulated_root) = node_with_label(&repopulated, "Choices");
    assert_eq!(repopulated_root_id, root_id);
    assert!(repopulated_root.supports_action(accesskit::Action::Focus));
    let (repopulated_beta_id, repopulated_beta) = node_with_label(&repopulated, "Beta");
    assert_eq!(repopulated_beta_id, beta_id);
    assert!(repopulated_beta.supports_action(accesskit::Action::Click));
    dispatch_action(cx, accesskit::Action::Click, repopulated_beta_id);
    cx.run_until_parked();
    assert_eq!(selections.borrow().as_slice(), &["beta", "beta"]);

    view.update(cx, |probe, cx| {
        probe.show = false;
        cx.notify();
    });
    cx.run_until_parked();
    let unmounted = cx
        .latest_accessibility_tree_update()
        .expect("listbox unmount should publish");
    assert!(!unmounted.nodes.iter().any(|(id, _)| *id == beta_id));
}

#[open_gpui::test]
fn tree_final_tree_focus_click_and_expansion_follow_resolved_state(
    cx: &mut open_gpui::TestAppContext,
) {
    struct TreeProbe {
        show: bool,
        selections: Rc<RefCell<Vec<String>>>,
        toggles: Rc<RefCell<Vec<(String, bool)>>>,
    }

    impl Render for TreeProbe {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let selections = self.selections.clone();
            let toggles = self.toggles.clone();
            let tree = Tree::new(
                "a11y-tree",
                "Navigation",
                [
                    TreeItemDescriptor::new("root", "Root")
                        .child(TreeItemDescriptor::new("child", "Child")),
                    TreeItemDescriptor::new("disabled", "Disabled").disabled(true),
                ],
            )
            .default_focused("root")
            .on_select(move |selection, _, _| {
                selections.borrow_mut().push(selection.value().to_owned());
            })
            .on_toggle(move |toggle, _, _| {
                toggles
                    .borrow_mut()
                    .push((toggle.value().to_owned(), toggle.expanded()));
            });

            div().size_full().when(self.show, |this| {
                this.child(div().w(px(320.0)).h(px(220.0)).child(tree))
            })
        }
    }

    let selections = Rc::new(RefCell::new(Vec::new()));
    let toggles = Rc::new(RefCell::new(Vec::new()));
    let (view, cx) = cx.add_window_view(|_, _| TreeProbe {
        show: true,
        selections: selections.clone(),
        toggles: toggles.clone(),
    });

    assert!(cx.activate_accessibility());
    let initial = cx
        .latest_accessibility_tree_update()
        .expect("tree accessibility tree should publish");
    let (_, tree) = node_with_label(&initial, "Navigation");
    assert_eq!(tree.role(), accesskit::Role::Tree);
    let (root_id, root) = node_with_label(&initial, "Root");
    assert_eq!(root.role(), accesskit::Role::TreeItem);
    assert_eq!(root.level(), Some(1));
    assert_eq!(root.is_expanded(), Some(false));
    assert_eq!(root.is_selected(), Some(false));
    assert!(root.supports_action(accesskit::Action::Focus));
    assert!(root.supports_action(accesskit::Action::Click));

    let (_, disabled) = node_with_label(&initial, "Disabled");
    assert!(disabled.is_disabled());
    assert!(!disabled.supports_action(accesskit::Action::Focus));
    assert!(!disabled.supports_action(accesskit::Action::Click));

    dispatch_action(cx, accesskit::Action::Focus, root_id);
    cx.run_until_parked();
    assert_eq!(
        cx.latest_accessibility_tree_update()
            .expect("tree focus should publish")
            .focus,
        root_id
    );

    dispatch_action(cx, accesskit::Action::Click, root_id);
    cx.run_until_parked();
    assert_eq!(selections.borrow().as_slice(), &["root"]);
    let selected = cx
        .latest_accessibility_tree_update()
        .expect("tree selection should publish");
    let (selected_root_id, selected_root) = node_with_label(&selected, "Root");
    assert_eq!(selected_root_id, root_id);
    assert_eq!(selected_root.is_selected(), Some(true));

    let (expand_id, expand) = node_with_label(&selected, "Expand Root");
    assert_eq!(expand.role(), accesskit::Role::Button);
    assert!(expand.supports_action(accesskit::Action::Click));
    dispatch_action(cx, accesskit::Action::Click, expand_id);
    cx.run_until_parked();
    assert_eq!(toggles.borrow().as_slice(), &[("root".to_owned(), true)]);
    let expanded = cx
        .latest_accessibility_tree_update()
        .expect("tree expansion should publish");
    let (expanded_root_id, expanded_root) = node_with_label(&expanded, "Root");
    assert_eq!(expanded_root_id, root_id);
    assert_eq!(expanded_root.is_expanded(), Some(true));
    let (child_id, child) = node_with_label(&expanded, "Child");
    assert_eq!(child.role(), accesskit::Role::TreeItem);
    assert_eq!(child.level(), Some(2));

    let (collapse_id, _) = node_with_label(&expanded, "Collapse Root");
    dispatch_action(cx, accesskit::Action::Click, collapse_id);
    cx.run_until_parked();
    let collapsed = cx
        .latest_accessibility_tree_update()
        .expect("tree collapse should publish");
    assert!(!collapsed.nodes.iter().any(|(id, _)| *id == child_id));
    assert_eq!(
        node_with_label(&collapsed, "Root").0,
        root_id,
        "equivalent tree-item rerenders keep node identity"
    );

    view.update(cx, |probe, cx| {
        probe.show = false;
        cx.notify();
    });
    cx.run_until_parked();
    let unmounted = cx
        .latest_accessibility_tree_update()
        .expect("tree unmount should publish");
    assert!(!unmounted.nodes.iter().any(|(id, _)| *id == root_id));
}

#[open_gpui::test]
fn duplicate_tree_values_keep_distinct_render_and_accessibility_identity(
    cx: &mut open_gpui::TestAppContext,
) {
    struct DuplicateTreeProbe;

    impl Render for DuplicateTreeProbe {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            div().size_full().child(
                Tree::new(
                    "duplicate-tree",
                    "Duplicate tree",
                    [
                        TreeItemDescriptor::new("duplicate", "First duplicate"),
                        TreeItemDescriptor::new("duplicate", "Second duplicate"),
                        TreeItemDescriptor::new("tail", "Tail"),
                    ],
                )
                .default_focused("tail"),
            )
        }
    }

    let (_, cx) = cx.add_window_view(|_, _| DuplicateTreeProbe);
    assert!(cx.activate_accessibility());
    let update = cx
        .latest_accessibility_tree_update()
        .expect("duplicate tree accessibility tree should publish");
    let (first_id, first) = node_with_label(&update, "First duplicate");
    let (second_id, second) = node_with_label(&update, "Second duplicate");

    assert_ne!(first_id, second_id);
    assert_eq!(first.role(), accesskit::Role::TreeItem);
    assert_eq!(second.role(), accesskit::Role::TreeItem);
    assert!(first.is_disabled());
    assert!(second.is_disabled());
    let duplicate_selectors = cx
        .debug_selectors_with_prefix("tree:duplicate-tree:item:")
        .into_iter()
        .filter(|selector| !selector.ends_with(":tail"))
        .collect::<Vec<_>>();
    assert_eq!(duplicate_selectors.len(), 2);
    assert_ne!(duplicate_selectors[0], duplicate_selectors[1]);
}

#[open_gpui::test]
fn virtualized_list_final_tree_distinguishes_rows_from_structural_content_and_recycles_by_key(
    cx: &mut open_gpui::TestAppContext,
) {
    struct VirtualizedListProbe {
        empty: bool,
        reveal_key: Option<String>,
        activations: Rc<RefCell<Vec<String>>>,
    }

    impl Render for VirtualizedListProbe {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let mut items = Vec::new();
            if !self.empty {
                items.extend([
                    VirtualizedListItemDescriptor::section("recent", "Recent"),
                    VirtualizedListItemDescriptor::item("alpha", "Alpha"),
                    VirtualizedListItemDescriptor::item("beta", "Beta").disabled(true),
                ]);
                items.extend((3..31).map(|index| {
                    VirtualizedListItemDescriptor::item(
                        format!("item-{index:04}"),
                        format!("Item {index:04}"),
                    )
                }));
            }

            let activations = self.activations.clone();
            let mut list = VirtualizedList::new("a11y-virtualized-list", "Results", items)
                .row_height(ui_px(28.0))
                .viewport_item_count(4)
                .overscan(0)
                .on_activate(move |activation, _, _| {
                    activations.borrow_mut().push(activation.key().to_owned());
                });
            if let Some(reveal_key) = self.reveal_key.as_ref() {
                list = list.bring_key_into_view(
                    reveal_key.clone(),
                    BringIntoViewOptions::vertical(BringIntoViewAlignment::MinEdge),
                );
            }

            div()
                .size_full()
                .child(div().w(px(320.0)).h(px(112.0)).child(list))
        }
    }

    let activations = Rc::new(RefCell::new(Vec::new()));
    let (view, cx) = cx.add_window_view(|_, _| VirtualizedListProbe {
        empty: false,
        reveal_key: None,
        activations: activations.clone(),
    });

    assert!(cx.activate_accessibility());
    let initial = cx
        .latest_accessibility_tree_update()
        .expect("virtualized list accessibility tree should publish");
    let (root_id, root) = node_with_label(&initial, "Results");
    assert_eq!(root.role(), accesskit::Role::ListBox);
    assert!(root.supports_action(accesskit::Action::Focus));

    let (section_id, section) = node_with_label(&initial, "Recent");
    assert_eq!(section.role(), accesskit::Role::Group);
    assert!(!section.supports_action(accesskit::Action::Click));
    assert!(!section.supports_action(accesskit::Action::Focus));

    let (alpha_id, alpha) = node_with_label(&initial, "Alpha");
    assert_eq!(alpha.role(), accesskit::Role::ListBoxOption);
    assert_eq!(alpha.position_in_set(), Some(1));
    assert_eq!(alpha.size_of_set(), Some(29));
    assert!(alpha.supports_action(accesskit::Action::Click));
    assert!(!alpha.supports_action(accesskit::Action::Focus));

    let (_, beta) = node_with_label(&initial, "Beta");
    assert!(beta.is_disabled());
    assert!(!beta.supports_action(accesskit::Action::Click));

    dispatch_action(cx, accesskit::Action::Click, alpha_id);
    cx.run_until_parked();
    assert_eq!(activations.borrow().as_slice(), &["alpha"]);
    let selected = cx
        .latest_accessibility_tree_update()
        .expect("virtualized list selection should publish");
    let (selected_alpha_id, selected_alpha) = node_with_label(&selected, "Alpha");
    assert_eq!(selected_alpha_id, alpha_id);
    assert_eq!(selected_alpha.is_selected(), Some(true));

    view.update(cx, |probe, cx| {
        probe.reveal_key = Some("item-0020".to_owned());
        cx.notify();
    });
    cx.run_until_parked();
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });
    let recycled = cx
        .latest_accessibility_tree_update()
        .expect("virtualized list recycle should publish");
    assert!(!recycled.nodes.iter().any(|(id, _)| *id == alpha_id));
    assert!(!recycled.nodes.iter().any(|(id, _)| *id == section_id));
    let (far_id, far_row) = node_with_label(&recycled, "Item 0020");
    assert_ne!(far_id, alpha_id);
    assert_eq!(far_row.role(), accesskit::Role::ListBoxOption);

    dispatch_action(cx, accesskit::Action::Click, alpha_id);
    assert_eq!(activations.borrow().as_slice(), &["alpha"]);

    view.update(cx, |probe, cx| {
        probe.reveal_key = Some("alpha".to_owned());
        cx.notify();
    });
    cx.run_until_parked();
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });
    let returned = cx
        .latest_accessibility_tree_update()
        .expect("virtualized list return should publish");
    assert_eq!(
        node_with_label(&returned, "Alpha").0,
        alpha_id,
        "the same stable item key must recover the same semantic node"
    );

    view.update(cx, |probe, cx| {
        probe.empty = true;
        probe.reveal_key = None;
        cx.notify();
    });
    cx.run_until_parked();
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });
    let empty = cx
        .latest_accessibility_tree_update()
        .expect("empty virtualized list should publish");
    let (empty_root_id, empty_root) = node_with_label(&empty, "Results");
    assert_eq!(empty_root_id, root_id);
    assert_eq!(empty_root.role(), accesskit::Role::ListBox);
    assert!(!empty_root.supports_action(accesskit::Action::Focus));
    assert!(!empty_root.supports_action(accesskit::Action::Click));
    assert!(!empty.nodes.iter().any(|(id, _)| *id == alpha_id));
    assert!(
        empty
            .nodes
            .iter()
            .all(|(_, node)| node.role() != accesskit::Role::ListBoxOption)
    );
    dispatch_action(cx, accesskit::Action::Click, alpha_id);
    assert_eq!(activations.borrow().as_slice(), &["alpha"]);

    view.update(cx, |probe, cx| {
        probe.empty = false;
        cx.notify();
    });
    cx.run_until_parked();
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });
    let repopulated = cx
        .latest_accessibility_tree_update()
        .expect("repopulated virtualized list should publish");
    let (repopulated_root_id, repopulated_root) = node_with_label(&repopulated, "Results");
    assert_eq!(repopulated_root_id, root_id);
    assert!(repopulated_root.supports_action(accesskit::Action::Focus));
    let (repopulated_alpha_id, repopulated_alpha) = node_with_label(&repopulated, "Alpha");
    assert_eq!(repopulated_alpha_id, alpha_id);
    assert!(repopulated_alpha.supports_action(accesskit::Action::Click));
    dispatch_action(cx, accesskit::Action::Click, repopulated_alpha_id);
    cx.run_until_parked();
    assert_eq!(activations.borrow().as_slice(), &["alpha", "alpha"]);
}

#[open_gpui::test]
fn splitter_final_tree_actions_resize_and_disabled_state_remove_capability(
    cx: &mut open_gpui::TestAppContext,
) {
    struct SplitterProbe {
        disabled: bool,
        show: bool,
    }

    impl Render for SplitterProbe {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let splitter = Splitter::new("a11y-splitter")
                .vertical()
                .disabled(self.disabled)
                .panel(SplitterPanel::new(
                    SplitterPanelDescriptor::new("primary", 0.5)
                        .min_fraction(0.2)
                        .max_fraction(0.8),
                    div().child("Primary"),
                ))
                .panel(SplitterPanel::new(
                    SplitterPanelDescriptor::new("detail", 0.5)
                        .min_fraction(0.2)
                        .max_fraction(0.8),
                    div().child("Detail"),
                ));

            div().size_full().when(self.show, |this| {
                this.child(div().w(px(320.0)).h(px(240.0)).child(splitter))
            })
        }
    }

    let (view, cx) = cx.add_window_view(|_, _| SplitterProbe {
        disabled: false,
        show: true,
    });

    assert!(cx.activate_accessibility());
    let initial = cx
        .latest_accessibility_tree_update()
        .expect("splitter accessibility tree should publish");
    let (handle_id, handle) = node_with_label(&initial, "Resize primary and detail");
    assert_eq!(handle.role(), accesskit::Role::Splitter);
    assert_eq!(handle.orientation(), Some(accesskit::Orientation::Vertical));
    assert_approx(handle.numeric_value(), 50.0);
    assert_approx(handle.min_numeric_value(), 20.0);
    assert_approx(handle.max_numeric_value(), 80.0);
    assert!(handle.supports_action(accesskit::Action::Focus));
    assert!(handle.supports_action(accesskit::Action::Increment));
    assert!(handle.supports_action(accesskit::Action::Decrement));

    dispatch_action(cx, accesskit::Action::Increment, handle_id);
    cx.run_until_parked();
    let incremented = cx
        .latest_accessibility_tree_update()
        .expect("splitter increment should publish");
    let (incremented_id, incremented_handle) =
        node_with_label(&incremented, "Resize primary and detail");
    assert_eq!(incremented_id, handle_id);
    assert_approx(incremented_handle.numeric_value(), 55.0);

    dispatch_action(cx, accesskit::Action::Decrement, handle_id);
    cx.run_until_parked();
    let decremented = cx
        .latest_accessibility_tree_update()
        .expect("splitter decrement should publish");
    assert_approx(
        node_with_label(&decremented, "Resize primary and detail")
            .1
            .numeric_value(),
        50.0,
    );

    view.update(cx, |probe, cx| {
        probe.disabled = true;
        cx.notify();
    });
    cx.run_until_parked();
    let disabled = cx
        .latest_accessibility_tree_update()
        .expect("disabled splitter should publish");
    let (disabled_id, disabled_handle) = node_with_label(&disabled, "Resize primary and detail");
    assert_eq!(disabled_id, handle_id);
    assert!(disabled_handle.is_disabled());
    assert!(!disabled_handle.supports_action(accesskit::Action::Focus));
    assert!(!disabled_handle.supports_action(accesskit::Action::Increment));
    assert!(!disabled_handle.supports_action(accesskit::Action::Decrement));

    dispatch_action(cx, accesskit::Action::Increment, disabled_id);
    cx.run_until_parked();
    let after_disabled_action = cx
        .latest_accessibility_tree_update()
        .expect("disabled splitter action should preserve the final tree");
    let (after_disabled_id, after_disabled_handle) =
        node_with_label(&after_disabled_action, "Resize primary and detail");
    assert_eq!(after_disabled_id, disabled_id);
    assert_approx(after_disabled_handle.numeric_value(), 50.0);

    view.update(cx, |probe, cx| {
        probe.show = false;
        cx.notify();
    });
    cx.run_until_parked();
    let unmounted = cx
        .latest_accessibility_tree_update()
        .expect("splitter unmount should publish");
    assert!(!unmounted.nodes.iter().any(|(id, _)| *id == handle_id));
}

#[open_gpui::test]
fn splitter_keyboard_reopens_collapsed_panels_on_both_sides(cx: &mut open_gpui::TestAppContext) {
    struct KeyboardProbe;

    impl Render for KeyboardProbe {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let collapsed_before = collapsed_splitter(
                "keyboard-before",
                "keyboard-before-collapsed",
                "keyboard-before-peer",
                true,
                0.1,
                0.0,
            );
            let collapsed_after = collapsed_splitter(
                "keyboard-after",
                "keyboard-after-peer",
                "keyboard-after-collapsed",
                false,
                0.2,
                0.05,
            );

            div()
                .size_full()
                .flex()
                .flex_col()
                .child(div().w(px(320.0)).h(px(120.0)).child(collapsed_before))
                .child(div().w(px(320.0)).h(px(120.0)).child(collapsed_after))
        }
    }

    let (_, cx) = cx.add_window_view(|_, _| KeyboardProbe);
    assert!(cx.activate_accessibility());
    let initial = cx
        .latest_accessibility_tree_update()
        .expect("collapsed keyboard splitters should publish");
    let (before_id, before) = node_with_label(
        &initial,
        "Resize keyboard-before-collapsed and keyboard-before-peer",
    );
    assert_splitter_node(before, 0.0, 0.0, 90.0);
    let (after_id, after) = node_with_label(
        &initial,
        "Resize keyboard-after-peer and keyboard-after-collapsed",
    );
    assert_splitter_node(after, 95.0, 10.0, 95.0);

    dispatch_action(cx, accesskit::Action::Focus, before_id);
    cx.run_until_parked();
    assert!(cx.debug_selector_is_focused("splitter:keyboard-before:handle:0"));
    cx.simulate_keystrokes("right");
    cx.run_until_parked();
    let reopened_before = cx
        .latest_accessibility_tree_update()
        .expect("right arrow should reopen the collapsed before panel");
    let (reopened_before_id, reopened_before) = node_with_label(
        &reopened_before,
        "Resize keyboard-before-collapsed and keyboard-before-peer",
    );
    assert_eq!(reopened_before_id, before_id);
    assert_splitter_node(reopened_before, 10.0, 10.0, 90.0);

    dispatch_action(cx, accesskit::Action::Focus, after_id);
    cx.run_until_parked();
    assert!(cx.debug_selector_is_focused("splitter:keyboard-after:handle:0"));
    cx.simulate_keystrokes("left");
    cx.run_until_parked();
    let reopened_after = cx
        .latest_accessibility_tree_update()
        .expect("left arrow should reopen the collapsed after panel");
    let (reopened_after_id, reopened_after) = node_with_label(
        &reopened_after,
        "Resize keyboard-after-peer and keyboard-after-collapsed",
    );
    assert_eq!(reopened_after_id, after_id);
    assert_splitter_node(reopened_after, 80.0, 10.0, 80.0);
}

#[open_gpui::test]
fn splitter_accesskit_actions_reopen_collapsed_panels_on_both_sides(
    cx: &mut open_gpui::TestAppContext,
) {
    struct AccessKitProbe;

    impl Render for AccessKitProbe {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let collapsed_before = collapsed_splitter(
                "accesskit-before",
                "accesskit-before-collapsed",
                "accesskit-before-peer",
                true,
                0.2,
                0.05,
            );
            let collapsed_after = collapsed_splitter(
                "accesskit-after",
                "accesskit-after-peer",
                "accesskit-after-collapsed",
                false,
                0.1,
                0.0,
            );

            div()
                .size_full()
                .flex()
                .flex_col()
                .child(div().w(px(320.0)).h(px(120.0)).child(collapsed_before))
                .child(div().w(px(320.0)).h(px(120.0)).child(collapsed_after))
        }
    }

    let (_, cx) = cx.add_window_view(|_, _| AccessKitProbe);
    assert!(cx.activate_accessibility());
    let initial = cx
        .latest_accessibility_tree_update()
        .expect("collapsed AccessKit splitters should publish");
    let (before_id, before) = node_with_label(
        &initial,
        "Resize accesskit-before-collapsed and accesskit-before-peer",
    );
    assert_splitter_node(before, 5.0, 5.0, 90.0);
    let (after_id, after) = node_with_label(
        &initial,
        "Resize accesskit-after-peer and accesskit-after-collapsed",
    );
    assert_splitter_node(after, 100.0, 10.0, 100.0);

    dispatch_action(cx, accesskit::Action::Increment, before_id);
    cx.run_until_parked();
    let reopened_before_update = cx
        .latest_accessibility_tree_update()
        .expect("AccessKit increment should reopen the collapsed before panel");
    let (reopened_before_id, reopened_before) = node_with_label(
        &reopened_before_update,
        "Resize accesskit-before-collapsed and accesskit-before-peer",
    );
    assert_eq!(reopened_before_id, before_id);
    assert_splitter_node(reopened_before, 20.0, 20.0, 90.0);
    let sibling_handle_bounds = cx
        .debug_bounds("splitter:accesskit-after:handle:0")
        .expect("resizing the first splitter must retain its sibling rendered handle");
    let sibling_accessibility_node = reopened_before_update
        .nodes
        .iter()
        .find(|(_, node)| {
            node.label() == Some("Resize accesskit-after-peer and accesskit-after-collapsed")
        })
        .map(|(id, _)| *id)
        .unwrap_or_else(|| {
            panic!(
                "resizing the first splitter dropped the sibling accessibility node at {sibling_handle_bounds:?}"
            )
        });
    assert_eq!(
        sibling_accessibility_node, after_id,
        "resizing the first splitter must retain its sibling accessibility node"
    );

    dispatch_action(cx, accesskit::Action::Decrement, after_id);
    cx.run_until_parked();
    let reopened_after_update = cx
        .latest_accessibility_tree_update()
        .expect("AccessKit decrement should reopen the collapsed after panel");
    let reopened_handle_bounds = cx
        .debug_bounds("splitter:accesskit-after:handle:0")
        .expect("AccessKit decrement must retain the reopened splitter handle");
    let (reopened_after_id, reopened_after) = reopened_after_update
        .nodes
        .iter()
        .find(|(_, node)| {
            node.label() == Some("Resize accesskit-after-peer and accesskit-after-collapsed")
        })
        .map(|(id, node)| (*id, node))
        .unwrap_or_else(|| {
            let labels = reopened_after_update
                .nodes
                .iter()
                .filter_map(|(_, node)| node.label())
                .collect::<Vec<_>>();
            panic!(
                "AccessKit decrement dropped the reopened splitter node at {reopened_handle_bounds:?}; published labels: {labels:?}"
            )
        });
    assert_eq!(reopened_after_id, after_id);
    assert_splitter_node(reopened_after, 90.0, 10.0, 90.0);
}
