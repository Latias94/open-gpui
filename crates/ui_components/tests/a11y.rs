use open_gpui::div;
use open_gpui_ui_components::{
    A11yContractError, A11yLabelSource, A11yStateEvidence, A11yValueKind, A11yValueMetadata,
    Button, COMPONENT_A11Y_EVIDENCE, Checkbox, ComponentA11yContract, Dialog, IconButton, Listbox,
    Menu, MenuItem, NumberInput, Progress, Slider, Splitter, SplitterPanel,
    SplitterPanelDescriptor, Table, Tree, TreeItemDescriptor, VirtualizedList,
    VirtualizedListItemDescriptor, VirtualizedListStatusKind, listbox::ListboxOption,
};
use open_gpui_ui_core::{
    AccessibleAction, Orientation, Role, TableColumn, TableRow, TableState, Toggled, ui_px,
};

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

    let missing_action = ComponentA11yContract::new("Button", Role::Button)
        .with_label_source(A11yLabelSource::VisibleText)
        .validate()
        .unwrap_err();
    assert_eq!(
        missing_action.error(),
        A11yContractError::MissingSupportedAction
    );
}

#[test]
fn component_contract_a11y_evidence_is_valid() {
    for evidence in COMPONENT_A11Y_EVIDENCE {
        let mut contract = ComponentA11yContract::new(evidence.component, evidence.role)
            .with_label_source(evidence.label_source)
            .with_actions(evidence.actions);

        if let Some(value_kind) = evidence.value_kind {
            contract = contract.with_value_metadata(A11yValueMetadata::present(value_kind));
        }
        if let Some(orientation) = evidence.orientation {
            contract = contract.with_orientation(orientation);
        }

        assert!(
            !evidence.state_coverage.is_empty(),
            "component a11y evidence `{}` must declare state or focus coverage",
            evidence.component
        );
        assert_unique_state_coverage(evidence.component, evidence.state_coverage);
        if evidence.state_coverage.contains(&A11yStateEvidence::Value) {
            assert!(
                evidence.value_kind.is_some(),
                "component a11y evidence `{}` declares value coverage without value metadata",
                evidence.component
            );
        }

        contract.validate().unwrap_or_else(|violation| {
            panic!(
                "component a11y evidence `{}` failed validation: {:?}",
                violation.component(),
                violation.error()
            )
        });
    }
}

#[test]
fn component_contract_a11y_evidence_records_state_and_focus_coverage() {
    assert_state_coverage(
        "Button",
        &[A11yStateEvidence::Disabled, A11yStateEvidence::Selected],
    );
    assert_state_coverage(
        "Checkbox",
        &[A11yStateEvidence::Checked, A11yStateEvidence::Disabled],
    );
    assert_state_coverage(
        "Slider",
        &[A11yStateEvidence::Disabled, A11yStateEvidence::Value],
    );
    assert_state_coverage(
        "Listbox option",
        &[
            A11yStateEvidence::Disabled,
            A11yStateEvidence::Selected,
            A11yStateEvidence::Value,
        ],
    );
    assert_state_coverage(
        "Tree item",
        &[
            A11yStateEvidence::Expanded,
            A11yStateEvidence::Focusable,
            A11yStateEvidence::Selected,
        ],
    );
    assert_state_coverage(
        "VirtualizedList row",
        &[
            A11yStateEvidence::Focusable,
            A11yStateEvidence::Selected,
            A11yStateEvidence::Value,
        ],
    );
    assert_state_coverage(
        "VirtualizedList structural row",
        &[A11yStateEvidence::NonInteractiveStructural],
    );
    assert_state_coverage(
        "Splitter handle",
        &[A11yStateEvidence::Disabled, A11yStateEvidence::Focusable],
    );
}

#[test]
fn representative_component_a11y_contracts_are_valid() {
    let button = Button::new("save", "Save").state();
    contract("Button", button.role())
        .with_label_source(A11yLabelSource::VisibleText)
        .selected_state(button.selected())
        .disabled_state(button.disabled())
        .with_actions(&[AccessibleAction::Click])
        .validate()
        .unwrap();

    let icon_button = IconButton::new("search", "?", "Search").state();
    assert_eq!(icon_button.accessible_label(), "Search");
    contract("IconButton", icon_button.role())
        .with_label_source(A11yLabelSource::ExplicitLabel)
        .disabled_state(icon_button.disabled())
        .with_actions(&[AccessibleAction::Click])
        .validate()
        .unwrap();

    let checkbox = Checkbox::new("terms")
        .label("Accept terms")
        .checked_state(Toggled::Mixed)
        .state();
    contract("Checkbox", checkbox.role())
        .with_label_source(A11yLabelSource::VisibleText)
        .checked_state(checkbox.toggled())
        .disabled_state(checkbox.disabled())
        .with_actions(&[AccessibleAction::Click])
        .validate()
        .unwrap();

    let slider = Slider::new("volume", "Volume").value(40.0).state();
    contract("Slider", slider.role())
        .with_label_source(A11yLabelSource::VisibleText)
        .with_value_metadata(A11yValueMetadata::present(A11yValueKind::Percent))
        .with_orientation(Orientation::Horizontal)
        .disabled_state(slider.disabled())
        .with_actions(&[
            AccessibleAction::Increment,
            AccessibleAction::Decrement,
            AccessibleAction::SetValue,
        ])
        .validate()
        .unwrap();

    let number_input = NumberInput::new("quantity", "Quantity").value(3.0).state();
    contract("NumberInput", number_input.role())
        .with_label_source(A11yLabelSource::VisibleText)
        .with_value_metadata(A11yValueMetadata::present(A11yValueKind::Number))
        .disabled_state(number_input.disabled())
        .with_actions(&[
            AccessibleAction::Increment,
            AccessibleAction::Decrement,
            AccessibleAction::SetValue,
        ])
        .validate()
        .unwrap();

    let progress = Progress::new("build-progress", "Build progress")
        .value(70.0)
        .state();
    assert_eq!(progress.value_percent(), Some(70.0));
    contract("Progress", progress.role())
        .with_label_source(A11yLabelSource::VisibleText)
        .with_value_metadata(A11yValueMetadata::present(A11yValueKind::Percent))
        .validate()
        .unwrap();

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

    let table_state = TableState::new([TableRow::new("row-a").with_cell("name", "Alpha")])
        .with_columns([TableColumn::new("name", "Name")]);
    let table = Table::new("release-table", "Release table", table_state)
        .behavior_snapshot(ui_px(0.0), ui_px(160.0));
    contract("Table", table.role())
        .with_label_source(A11yLabelSource::VisibleText)
        .with_value_metadata(A11yValueMetadata::present(A11yValueKind::Count))
        .validate()
        .unwrap();
    contract("Table row", table.row_role()).validate().unwrap();
    contract("Table header", table.column_header_role())
        .with_label_source(A11yLabelSource::VisibleText)
        .validate()
        .unwrap();
    contract("Table cell", table.cell_role())
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

fn assert_state_coverage(component: &str, expected: &[A11yStateEvidence]) {
    let evidence = COMPONENT_A11Y_EVIDENCE
        .iter()
        .find(|evidence| evidence.component == component)
        .unwrap_or_else(|| panic!("missing a11y evidence for `{component}`"));

    assert_eq!(
        evidence.state_coverage, expected,
        "unexpected a11y state coverage for `{component}`"
    );
}

fn assert_unique_state_coverage(component: &str, coverage: &[A11yStateEvidence]) {
    for (index, state) in coverage.iter().enumerate() {
        assert!(
            !coverage[..index].contains(state),
            "component a11y evidence `{component}` repeats state coverage `{state:?}`"
        );
    }
}
