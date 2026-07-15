#[path = "support/a11y.rs"]
mod a11y_support;
#[path = "a11y/collection_semantics.rs"]
mod collection_semantics;

use open_gpui::prelude::FluentBuilder;
use open_gpui::{
    Context, ElementId, InteractiveElement, IntoElement, ParentElement, Render,
    StatefulInteractiveElement, Styled, Window, accesskit, div,
};
use open_gpui_ui_components::gpui_adapter::UiA11yElementExt;
use open_gpui_ui_components::{
    A11yContractError, A11yLabelSource, A11yValueKind, A11yValueMetadata, Button,
    ComponentA11yContract, Dialog, Listbox, Menu, MenuItem, Splitter, SplitterPanel,
    SplitterPanelDescriptor, Tree, TreeItemDescriptor, VirtualizedList,
    VirtualizedListItemDescriptor, VirtualizedListStatusKind, listbox::ListboxOption,
};
use open_gpui_ui_core::{AccessibleAction, Role, SemanticDescriptor, Toggled, ui_px};
use std::{cell::RefCell, rc::Rc};

use a11y_support::node_with_label as a11y_node_with_label;

#[open_gpui::test]
fn button_final_tree_and_actions_follow_resolved_projection(cx: &mut open_gpui::TestAppContext) {
    struct ButtonA11yProbe {
        activations: Rc<RefCell<usize>>,
        disabled: bool,
        show: bool,
    }

    impl Render for ButtonA11yProbe {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let activations = self.activations.clone();
            let disabled = self.disabled;
            let button = Button::new("semantic-button", "Save")
                .selected(true)
                .disabled(disabled)
                .accessibility_description("Writes the document")
                .on_activate(move |_, _, _| *activations.borrow_mut() += 1);

            div()
                .size_full()
                .when(self.show, |this| this.child(button))
                .child(Button::new("focus-only-button", "Focus only"))
        }
    }

    let activations = Rc::new(RefCell::new(0));
    let (view, cx) = cx.add_window_view(|_, _| ButtonA11yProbe {
        activations: activations.clone(),
        disabled: false,
        show: true,
    });

    assert!(cx.activate_accessibility());
    let initial = cx
        .latest_accessibility_tree_update()
        .expect("button accessibility tree should publish");
    let (button_id, button_node) = a11y_node_with_label(&initial, "Save");
    assert_eq!(button_node.role(), accesskit::Role::Button);
    assert_eq!(button_node.description(), Some("Writes the document"));
    assert_eq!(button_node.is_selected(), Some(true));
    assert!(!button_node.is_disabled());
    assert!(button_node.supports_action(accesskit::Action::Click));
    assert!(button_node.supports_action(accesskit::Action::Focus));

    let (focus_only_id, focus_only) = a11y_node_with_label(&initial, "Focus only");
    assert!(!focus_only.supports_action(accesskit::Action::Click));
    assert!(focus_only.supports_action(accesskit::Action::Focus));

    assert!(cx.dispatch_accessibility_action(accesskit::ActionRequest {
        action: accesskit::Action::Focus,
        target_tree: accesskit::TreeId::ROOT,
        target_node: focus_only_id,
        data: None,
    }));
    cx.run_until_parked();
    assert_eq!(
        cx.latest_accessibility_tree_update()
            .expect("focus-only button focus should publish")
            .focus,
        focus_only_id
    );

    assert!(cx.dispatch_accessibility_action(accesskit::ActionRequest {
        action: accesskit::Action::Click,
        target_tree: accesskit::TreeId::ROOT,
        target_node: button_id,
        data: None,
    }));
    assert_eq!(*activations.borrow(), 1);

    view.update(cx, |probe, cx| {
        probe.disabled = true;
        cx.notify();
    });
    cx.run_until_parked();

    let disabled = cx
        .latest_accessibility_tree_update()
        .expect("disabled button accessibility tree should publish");
    let (disabled_id, disabled_node) = a11y_node_with_label(&disabled, "Save");
    assert_eq!(
        disabled_id, button_id,
        "equivalent rerenders keep node identity"
    );
    assert!(disabled_node.is_disabled());
    assert!(!disabled_node.supports_action(accesskit::Action::Click));
    assert!(!disabled_node.supports_action(accesskit::Action::Focus));

    assert!(cx.dispatch_accessibility_action(accesskit::ActionRequest {
        action: accesskit::Action::Click,
        target_tree: accesskit::TreeId::ROOT,
        target_node: button_id,
        data: None,
    }));
    assert_eq!(*activations.borrow(), 1);

    view.update(cx, |probe, cx| {
        probe.show = false;
        cx.notify();
    });
    cx.run_until_parked();
    let unmounted = cx
        .latest_accessibility_tree_update()
        .expect("button unmount accessibility tree should publish");
    assert!(
        !unmounted.nodes.iter().any(|(id, _)| *id == button_id),
        "unmounted semantic nodes must leave the final tree"
    );
}

#[open_gpui::test]
fn semantic_relations_resolve_update_and_repair_after_unmount(cx: &mut open_gpui::TestAppContext) {
    struct RelationProbe {
        alternate_label: bool,
        show_controlled: bool,
        show_description: bool,
    }

    impl Render for RelationProbe {
        fn render(&mut self, window: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let controlled_id: ElementId = "semantic-relation-controlled".into();
            let primary_label_id: ElementId = "semantic-relation-label-primary".into();
            let alternate_label_id: ElementId = "semantic-relation-label-alternate".into();
            let description_id: ElementId = "semantic-relation-description".into();
            let controls = [controlled_id.clone()];
            let labelled_by = [if self.alternate_label {
                alternate_label_id.clone()
            } else {
                primary_label_id.clone()
            }];
            let described_by = self
                .show_description
                .then(|| description_id.clone())
                .into_iter()
                .collect::<Vec<_>>();
            let semantics = SemanticDescriptor::<ElementId>::new(Role::TextInput)
                .with_label("Relation source")
                .with_controls(&controls)
                .with_labelled_by(&labelled_by)
                .with_described_by(&described_by);

            let source = div()
                .id("semantic-relation-source")
                .ui_semantics_with_relations(&semantics, |id| {
                    window.with_global_id(id.clone(), |global_id, _| global_id.accesskit_node_id())
                });

            div()
                .child(source)
                .child(
                    div()
                        .id(primary_label_id)
                        .role(accesskit::Role::Label)
                        .aria_label("Primary relation label"),
                )
                .child(
                    div()
                        .id(alternate_label_id)
                        .role(accesskit::Role::Label)
                        .aria_label("Alternate relation label"),
                )
                .when(self.show_controlled, |this| {
                    this.child(
                        div()
                            .id(controlled_id)
                            .role(accesskit::Role::Group)
                            .aria_label("Controlled relation target"),
                    )
                })
                .when(self.show_description, |this| {
                    this.child(
                        div()
                            .id(description_id)
                            .role(accesskit::Role::Label)
                            .aria_label("Relation description"),
                    )
                })
        }
    }

    let (view, cx) = cx.add_window_view(|_, _| RelationProbe {
        alternate_label: false,
        show_controlled: true,
        show_description: true,
    });

    assert!(cx.activate_accessibility());
    let initial = cx
        .latest_accessibility_tree_update()
        .expect("relation projection should publish");
    let (source_id, source) = a11y_node_with_label(&initial, "Relation source");
    let (controlled_id, _) = a11y_node_with_label(&initial, "Controlled relation target");
    let (primary_label_id, _) = a11y_node_with_label(&initial, "Primary relation label");
    let (description_id, _) = a11y_node_with_label(&initial, "Relation description");
    assert_eq!(source.controls(), &[controlled_id]);
    assert_eq!(source.labelled_by(), &[primary_label_id]);
    assert_eq!(source.described_by(), &[description_id]);

    view.update(cx, |probe, cx| {
        probe.alternate_label = true;
        probe.show_controlled = false;
        probe.show_description = false;
        cx.notify();
    });
    cx.run_until_parked();

    let updated = cx
        .latest_accessibility_tree_update()
        .expect("updated relation projection should publish");
    let (updated_source_id, updated_source) = a11y_node_with_label(&updated, "Relation source");
    let (alternate_label_id, _) = a11y_node_with_label(&updated, "Alternate relation label");
    assert_eq!(updated_source_id, source_id);
    assert!(updated_source.controls().is_empty());
    assert_eq!(updated_source.labelled_by(), &[alternate_label_id]);
    assert!(updated_source.described_by().is_empty());
    assert!(
        !updated
            .nodes
            .iter()
            .any(|(id, _)| { *id == controlled_id || *id == description_id })
    );
}

#[test]
fn a11y_contract_validation_reports_required_metadata_failures() {
    let missing_name = ComponentA11yContract::new("IconButton", Role::Button)
        .with_actions(&[AccessibleAction::Click])
        .validate()
        .unwrap_err();
    assert_eq!(missing_name.component(), "IconButton");
    assert_eq!(missing_name.role(), Role::Button);
    assert_eq!(
        missing_name.error(),
        A11yContractError::MissingAccessibleName
    );

    let missing_value = ComponentA11yContract::new("Slider", Role::Slider)
        .with_label_source(A11yLabelSource::VisibleText)
        .with_actions(&[
            AccessibleAction::Increment,
            AccessibleAction::Decrement,
            AccessibleAction::SetValue,
        ])
        .validate()
        .unwrap_err();
    assert_eq!(
        missing_value.error(),
        A11yContractError::MissingValueMetadata
    );

    let missing_action = ComponentA11yContract::new("Action control", Role::Button)
        .with_label_source(A11yLabelSource::VisibleText)
        .validate()
        .unwrap_err();
    assert_eq!(
        missing_action.error(),
        A11yContractError::MissingSupportedAction
    );
}

#[test]
fn representative_component_a11y_contracts_are_valid() {
    let dialog = Dialog::new("release-dialog", "Open", "Release notes", "Details").state();
    contract("Dialog trigger", dialog.trigger_role())
        .with_label_source(A11yLabelSource::VisibleText)
        .selected_state(dialog.trigger_selected())
        .disabled_state(dialog.disabled())
        .with_actions(&[AccessibleAction::Click])
        .validate()
        .unwrap();
    contract("Dialog content", dialog.content_role())
        .with_label_source(A11yLabelSource::VisibleText)
        .validate()
        .unwrap();

    let menu = Menu::new("file-menu", "File")
        .item(MenuItem::action("open", "Open"))
        .default_open(true)
        .default_focused_value("open")
        .state();
    let menu_item = menu
        .visible_items()
        .first()
        .expect("menu item should exist");
    contract("Menu trigger", menu.trigger_role())
        .with_label_source(A11yLabelSource::VisibleText)
        .selected_state(menu.trigger_selected())
        .disabled_state(menu.disabled())
        .with_actions(&[AccessibleAction::Click])
        .validate()
        .unwrap();
    contract("Menu content", menu.content_role())
        .with_label_source(A11yLabelSource::VisibleText)
        .validate()
        .unwrap();
    contract(
        "Menu item",
        menu_item.role().expect("menu item role should exist"),
    )
    .with_label_source(A11yLabelSource::VisibleText)
    .disabled_state(menu_item.disabled())
    .checked_state(menu_item.toggled().unwrap_or(Toggled::False))
    .with_actions(&[AccessibleAction::Click])
    .validate()
    .unwrap();

    let listbox = Listbox::new("choices", "Choices")
        .option(ListboxOption::new("alpha", "Alpha"))
        .selected("alpha")
        .state();
    let option = listbox
        .selected_option()
        .expect("selected listbox option should exist");
    contract("Listbox", listbox.role())
        .with_label_source(A11yLabelSource::VisibleText)
        .disabled_state(listbox.disabled())
        .with_value_metadata(A11yValueMetadata::present(A11yValueKind::Selection))
        .validate()
        .unwrap();
    contract(
        "Listbox option",
        option.role().expect("listbox option role should exist"),
    )
    .with_label_source(A11yLabelSource::VisibleText)
    .selected_state(option.selected())
    .disabled_state(option.disabled())
    .with_actions(&[AccessibleAction::Click])
    .validate()
    .unwrap();

    let tree = Tree::new(
        "nav-tree",
        "Navigation",
        [TreeItemDescriptor::new("root", "Root")
            .expanded(true)
            .child(TreeItemDescriptor::new("child", "Child"))],
    )
    .default_selected("root")
    .default_focused("root")
    .behavior_snapshot(ui_px(0.0), ui_px(160.0));
    let tree_row = tree.rows().first().expect("tree row should exist");
    contract("Tree", tree.role())
        .with_label_source(A11yLabelSource::VisibleText)
        .validate()
        .unwrap();
    contract("Tree item", tree.row_role())
        .with_label_source(A11yLabelSource::VisibleText)
        .selected_state(tree_row.selected())
        .expanded_state(true)
        .with_actions(&[AccessibleAction::Click, AccessibleAction::Focus])
        .validate()
        .unwrap();

    let virtualized_list = VirtualizedList::new(
        "virtual-list",
        "Virtual list",
        [
            VirtualizedListItemDescriptor::new("alpha", "Alpha"),
            VirtualizedListItemDescriptor::new("beta", "Beta"),
        ],
    )
    .default_active_key("alpha")
    .default_selected_key("alpha")
    .behavior_snapshot_with_viewport(ui_px(0.0), ui_px(80.0));
    let virtualized_row = virtualized_list
        .rows()
        .first()
        .expect("virtualized row should exist");
    contract("VirtualizedList", virtualized_list.role())
        .with_label_source(A11yLabelSource::VisibleText)
        .with_value_metadata(A11yValueMetadata::present(A11yValueKind::Count))
        .validate()
        .unwrap();
    contract("VirtualizedList row", virtualized_list.row_role())
        .with_label_source(A11yLabelSource::VisibleText)
        .selected_state(virtualized_row.selected())
        .with_actions(&[AccessibleAction::Click, AccessibleAction::Focus])
        .validate()
        .unwrap();

    let status_list = VirtualizedList::new(
        "virtual-status-list",
        "Virtual status list",
        [
            VirtualizedListItemDescriptor::prepend_loading("prepend", "Loading previous rows"),
            VirtualizedListItemDescriptor::item("alpha", "Alpha"),
            VirtualizedListItemDescriptor::retry("retry", "Refresh failed", "Retry"),
            VirtualizedListItemDescriptor::exhausted("done", "End of list"),
        ],
    )
    .default_active_key("prepend")
    .default_selected_keys(["prepend", "retry", "done"])
    .behavior_snapshot_with_viewport(ui_px(0.0), ui_px(112.0));
    assert_eq!(status_list.state().active_key(), Some("alpha"));
    assert_eq!(status_list.state().selected_keys(), Vec::<&str>::new());
    assert_eq!(
        status_list.rows()[0].status_kind(),
        Some(VirtualizedListStatusKind::PrependLoading)
    );
    assert_eq!(status_list.rows()[0].position_in_set(), None);
    assert_eq!(status_list.rows()[1].position_in_set(), Some(1));
    assert_eq!(status_list.rows()[1].size_of_set(), 1);
    assert_eq!(
        status_list.rows()[2].status_kind(),
        Some(VirtualizedListStatusKind::Retry)
    );
    assert_eq!(status_list.rows()[2].retry_action_label(), Some("Retry"));
    assert_eq!(status_list.rows()[2].position_in_set(), None);

    let sticky_list = VirtualizedList::new(
        "virtual-sticky-list",
        "Virtual sticky list",
        [
            VirtualizedListItemDescriptor::section("recent", "Recent"),
            VirtualizedListItemDescriptor::item("alpha", "Alpha"),
            VirtualizedListItemDescriptor::section("archived", "Archived"),
            VirtualizedListItemDescriptor::item("gamma", "Gamma"),
        ],
    )
    .row_height(ui_px(20.0))
    .overscan(0)
    .behavior_snapshot_with_viewport(ui_px(60.0), ui_px(40.0));
    let sticky_overlay = sticky_list
        .sticky_overlay()
        .expect("grouped visible rows should expose overlay metadata");
    assert_eq!(sticky_overlay.section().key(), "archived");
    assert_eq!(sticky_overlay.role(), None);
    assert!(!sticky_overlay.focusable());
    assert!(!sticky_overlay.pointer_interactive());

    let splitter = Splitter::new("main-split")
        .vertical()
        .panel(SplitterPanel::new(
            SplitterPanelDescriptor::new("left", 0.5),
            div(),
        ))
        .panel(SplitterPanel::new(
            SplitterPanelDescriptor::new("right", 0.5),
            div(),
        ))
        .state();
    let handle = splitter
        .handles()
        .first()
        .expect("splitter handle should exist");
    contract("Splitter handle", Role::Splitter)
        .with_label_source(A11yLabelSource::Generated)
        .disabled_state(handle.disabled())
        .with_orientation(splitter.orientation())
        .with_actions(&[AccessibleAction::Increment, AccessibleAction::Decrement])
        .validate()
        .unwrap();
}

fn contract(component: &'static str, role: Role) -> ComponentA11yContract {
    ComponentA11yContract::new(component, role)
}
