#[path = "support/a11y.rs"]
mod a11y_support;
#[path = "navigation/radio.rs"]
mod radio;

use open_gpui::{
    Context, InteractiveElement, IntoElement, KeyDownEvent, KeyUpEvent, Keystroke, Modifiers,
    ParentElement, Render, ScrollDelta, ScrollWheelEvent, Styled, Window, accesskit, div, point,
    px,
};
use open_gpui_ui_components::{
    ActionDescriptor, Button, CommandItemDescriptor, IconButton, Menu, MenuItem,
    MenuItemDescriptor, ResolvedActionIcon, Sidebar, SidebarCollapseMode, SidebarItemDescriptor,
    SidebarSection, SidebarSectionDescriptor, SidebarSide, SidebarState, SidebarVariant, Tabs,
    TabsActivationMode, TabsItem, TabsItemDescriptor, TabsSelection, TabsSelectionAuthority,
    TabsState, ToggleGroup, ToggleGroupItem, Toolbar, ToolbarActivation, ToolbarItemDescriptor,
    ToolbarItemKind, ToolbarState, Tooltip,
    sidebar::SidebarItem,
    sidebar_navigation_target,
    tabs::{active_index_from_str_keys, first_enabled, last_enabled, next_enabled},
    toolbar::ToolbarItem,
    toolbar_navigation_target,
};
use open_gpui_ui_core::{Orientation, Role, Sizable, Size, ThemeTokens, Toggled, ui_px};
use std::cell::RefCell;
use std::rc::Rc;

fn key_down(key: &str) -> KeyDownEvent {
    KeyDownEvent {
        keystroke: Keystroke {
            modifiers: Modifiers::none(),
            key: key.to_owned(),
            key_char: None,
        },
        is_held: false,
        prefer_character_input: false,
    }
}

fn key_up(key: &str) -> KeyUpEvent {
    KeyUpEvent {
        keystroke: Keystroke {
            modifiers: Modifiers::none(),
            key: key.to_owned(),
            key_char: None,
        },
    }
}

use a11y_support::node_with_label as a11y_node_with_label;

#[test]
fn resolved_action_projects_consistent_facts_to_navigation_and_action_surfaces() {
    let command = open_gpui_command::CommandDescriptor::new("workspace.open", "Open Workspace")
        .icon(open_gpui_command::CommandIconDescriptor::new("folder-open").fallback_label("O"))
        .shortcut("Ctrl+O")
        .disabled_reason("No workspace")
        .tooltip("Open a workspace")
        .accessibility_description("Opens the workspace picker");
    let action = ActionDescriptor::from_command_descriptor(&command).resolve_with(
        &|icon: &open_gpui_ui_components::ActionIconDescriptor| {
            ResolvedActionIcon::resolved(icon.clone(), "O")
        },
    );

    let toolbar_state = ToolbarState::resolve(
        Orientation::Horizontal,
        Size::Medium,
        false,
        "Main toolbar",
        None,
        [ToolbarItemDescriptor::from_resolved_action(&action)],
        ThemeTokens::default(),
    );
    let toolbar_item = &toolbar_state.items()[0];
    assert_eq!(toolbar_item.value(), "workspace.open");
    assert_eq!(toolbar_item.label(), "Open Workspace");
    assert_eq!(toolbar_item.icon_label(), Some("O"));
    assert_eq!(toolbar_item.shortcut(), Some("Ctrl+O"));
    assert_eq!(toolbar_item.disabled_reason_ref(), Some("No workspace"));
    assert_eq!(toolbar_item.tooltip(), Some("Open a workspace"));
    assert_eq!(
        toolbar_item.accessibility_description(),
        Some("Opens the workspace picker")
    );
    assert!(!toolbar_item.activation_enabled());

    let sidebar_state = SidebarState::resolve(
        SidebarSide::Left,
        SidebarVariant::Docked,
        SidebarCollapseMode::Icon,
        false,
        false,
        "Primary navigation",
        None,
        None,
        [SidebarSectionDescriptor::new("workspace", "Workspace")
            .item(SidebarItemDescriptor::from_resolved_action(&action))],
        Size::Medium,
        ThemeTokens::default(),
    );
    let sidebar_item = &sidebar_state.items()[0];
    assert_eq!(sidebar_item.icon_label(), Some("O"));
    assert_eq!(sidebar_item.shortcut(), Some("Ctrl+O"));
    assert_eq!(sidebar_item.disabled_reason_ref(), Some("No workspace"));
    assert_eq!(sidebar_item.tooltip(), Some("Open a workspace"));
    assert_eq!(
        sidebar_item.accessibility_description(),
        Some("Opens the workspace picker")
    );
    assert!(!sidebar_item.activation_enabled());

    let menu_state = Menu::new("more", "More")
        .open(true)
        .item(MenuItem::from_resolved_action(&action))
        .state();
    let menu_item = &menu_state.items()[0];
    assert_eq!(menu_item.icon_label(), Some("O"));
    assert_eq!(menu_item.shortcut(), Some("Ctrl+O"));
    assert_eq!(menu_item.disabled_reason_ref(), Some("No workspace"));
    assert_eq!(menu_item.tooltip(), Some("Open a workspace"));
    assert_eq!(
        menu_item.accessibility_description(),
        Some("Opens the workspace picker")
    );
    assert!(!menu_item.activation_enabled());

    let menu_descriptor = MenuItemDescriptor::from_resolved_action(&action);
    let command_descriptor = CommandItemDescriptor::from_resolved_action(&action);
    assert_eq!(menu_descriptor.icon_label(), Some("O"));
    assert_eq!(command_descriptor.icon_label(), Some("O"));
    assert_eq!(
        command_descriptor.disabled_reason_ref(),
        Some("No workspace")
    );
    assert_eq!(
        command_descriptor.accessibility_description_ref(),
        Some("Opens the workspace picker")
    );

    let button = Button::from_resolved_action("open-button", &action);
    let icon_button = IconButton::from_resolved_action("open-icon", &action);
    assert_eq!(
        button.resolved_action().map(|action| action.value()),
        Some("workspace.open")
    );
    assert_eq!(
        icon_button
            .resolved_action()
            .map(|action| action.icon_label()),
        Some(Some("O"))
    );
    assert!(!icon_button.state().activation_enabled());
}

#[test]
fn tabs_navigation_helpers_skip_disabled_tabs() {
    let keys = vec![
        "overview".to_string(),
        "details".to_string(),
        "history".to_string(),
    ];
    let disabled = [false, true, false];

    assert_eq!(first_enabled(&disabled), Some(0));
    assert_eq!(last_enabled(&disabled), Some(2));
    assert_eq!(next_enabled(&disabled, 0, true, true), Some(2));
    assert_eq!(next_enabled(&disabled, 2, false, true), Some(0));
    assert_eq!(
        active_index_from_str_keys(&keys, Some("details"), &disabled),
        Some(0)
    );
    assert_eq!(
        active_index_from_str_keys(&keys, Some("missing"), &disabled),
        Some(0)
    );
}

#[test]
fn tabs_state_resolution_tracks_selected_focus_and_tab_stop() {
    let state = TabsState::resolve(
        Orientation::Vertical,
        TabsActivationMode::Manual,
        Size::Small,
        TabsSelectionAuthority::Uncontrolled(Some("security")),
        Some("billing"),
        [
            TabsItemDescriptor::new("profile", "Profile"),
            TabsItemDescriptor::new("security", "Security"),
            TabsItemDescriptor::new("billing", "Billing").disabled(true),
            TabsItemDescriptor::new("integrations", "Integrations"),
        ],
        ThemeTokens::default(),
    );

    assert_eq!(state.orientation(), Orientation::Vertical);
    assert_eq!(state.activation_mode(), TabsActivationMode::Manual);
    assert_eq!(state.size(), Size::Small);
    assert_eq!(state.selected_value(), Some("security"));
    assert_eq!(state.focused_value(), Some("security"));
    assert_eq!(state.tab_stop_index(), state.focused_index());
    assert!(state.items()[1].selected());
    assert!(state.items()[1].focused());
    assert!(state.items()[2].disabled());
    assert!(!state.items()[2].focused());
}

#[test]
fn tabs_builder_state_falls_back_to_first_enabled_tab() {
    let state = Tabs::new("settings")
        .orientation(Orientation::Horizontal)
        .activation_mode(TabsActivationMode::Automatic)
        .with_size(Size::Large)
        .default_selected("history")
        .item(TabsItem::new("overview", "Overview", div()))
        .item(TabsItem::new("details", "Details", div()))
        .item(TabsItem::new("history", "History", div()).disabled(true))
        .state();

    assert_eq!(state.orientation(), Orientation::Horizontal);
    assert_eq!(state.activation_mode(), TabsActivationMode::Automatic);
    assert_eq!(state.size(), Size::Large);
    assert_eq!(state.selected_value(), Some("overview"));
    assert_eq!(state.focused_value(), Some("overview"));
    assert_eq!(state.tab_stop_index(), state.focused_index());
    assert_eq!(state.items().len(), 3);
    assert!(state.items()[2].disabled());
    assert!(!state.items()[2].selected());
}

#[open_gpui::test]
fn tabs_vertical_tablist_scrolls_when_constrained(cx: &mut open_gpui::TestAppContext) {
    struct TestView;

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let tabs = (0..12).fold(
                Tabs::new("overflow-tabs")
                    .orientation(Orientation::Vertical)
                    .small()
                    .default_selected("tab-0"),
                |tabs, index| {
                    tabs.item(TabsItem::new(
                        format!("tab-{index}"),
                        format!("Tab {index}"),
                        div().child(format!("Panel {index}")),
                    ))
                },
            );

            div()
                .size_full()
                .child(div().w(px(280.0)).h(px(120.0)).child(tabs))
        }
    }

    let (_, cx) = cx.add_window_view(|_, _| TestView);
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    let tab_before = cx
        .debug_bounds("tabs:overflow-tabs:trigger:tab-3")
        .expect("tab trigger should be rendered before scrolling");
    let tablist = cx
        .debug_bounds("tabs:overflow-tabs:tablist")
        .expect("tablist should be rendered");
    let tablist_viewport = cx
        .debug_bounds("scroll-area:tabs:overflow-tabs:tablist-scroll")
        .expect("vertical tablist should use the shared ScrollArea viewport");

    assert!(
        tablist.contains(&tablist_viewport.center()),
        "expected ScrollArea viewport to stay inside the tablist shell; tablist={tablist:?} viewport={tablist_viewport:?}"
    );

    cx.simulate_event(ScrollWheelEvent {
        position: tablist_viewport.center(),
        delta: ScrollDelta::Pixels(point(px(0.0), px(-64.0))),
        ..Default::default()
    });
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    let tab_after = cx
        .debug_bounds("tabs:overflow-tabs:trigger:tab-3")
        .expect("tab trigger should remain rendered after scrolling");

    assert!(
        tab_after.top() < tab_before.top(),
        "expected constrained vertical tablist to scroll; before={tab_before:?} after={tab_after:?}"
    );
}

#[open_gpui::test]
fn tabs_final_tree_relations_actions_and_node_ids_follow_runtime_selection(
    cx: &mut open_gpui::TestAppContext,
) {
    struct TabsA11yProbe;

    impl Render for TabsA11yProbe {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            div().size_full().child(
                Tabs::new("a11y-navigation-tabs")
                    .default_selected("overview")
                    .item(TabsItem::new(
                        "overview",
                        "Overview",
                        div().child("Overview panel"),
                    ))
                    .item(TabsItem::new(
                        "details",
                        "Details",
                        div().child("Details panel"),
                    ))
                    .item(
                        TabsItem::new("billing", "Billing", div().child("Billing panel"))
                            .disabled(true),
                    ),
            )
        }
    }

    fn node_with_role(
        update: &accesskit::TreeUpdate,
        role: accesskit::Role,
    ) -> (accesskit::NodeId, &accesskit::Node) {
        update
            .nodes
            .iter()
            .find(|(_, node)| node.role() == role)
            .map(|(id, node)| (*id, node))
            .unwrap_or_else(|| panic!("missing accessibility node with role {role:?}"))
    }

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

    let (_, cx) = cx.add_window_view(|_, _| TabsA11yProbe);
    assert!(cx.activate_accessibility());

    let initial = cx
        .latest_accessibility_tree_update()
        .expect("tabs accessibility tree should publish");
    let (tablist_id, tablist) = node_with_role(&initial, accesskit::Role::TabList);
    let (panel_id, panel) = node_with_role(&initial, accesskit::Role::TabPanel);
    let (overview_id, overview) = a11y_node_with_label(&initial, "Overview");
    let (details_id, details) = a11y_node_with_label(&initial, "Details");
    let (billing_id, billing) = a11y_node_with_label(&initial, "Billing");

    assert_eq!(
        tablist.orientation(),
        Some(accesskit::Orientation::Horizontal)
    );
    assert!(!tablist.supports_action(accesskit::Action::Click));
    assert!(!tablist.supports_action(accesskit::Action::Focus));
    assert!(!panel.supports_action(accesskit::Action::Click));
    assert!(!panel.supports_action(accesskit::Action::Focus));

    for (index, tab) in [overview, details, billing].into_iter().enumerate() {
        assert_eq!(tab.role(), accesskit::Role::Tab);
        assert_eq!(tab.position_in_set(), Some(index + 1));
        assert_eq!(tab.size_of_set(), Some(3));
        assert_eq!(tab.controls(), &[panel_id]);
    }
    assert_eq!(overview.is_selected(), Some(true));
    assert_eq!(details.is_selected(), Some(false));
    assert_eq!(billing.is_selected(), Some(false));
    assert_eq!(panel.labelled_by(), &[overview_id]);

    for tab in [overview, details] {
        assert!(tab.supports_action(accesskit::Action::Click));
        assert!(tab.supports_action(accesskit::Action::Focus));
        assert!(!tab.supports_action(accesskit::Action::Increment));
    }
    assert!(billing.is_disabled());
    assert!(!billing.supports_action(accesskit::Action::Click));
    assert!(!billing.supports_action(accesskit::Action::Focus));

    dispatch_action(cx, accesskit::Action::Focus, details_id);
    cx.run_until_parked();
    assert_eq!(
        cx.latest_accessibility_tree_update()
            .expect("tab focus should publish")
            .focus,
        details_id
    );

    dispatch_action(cx, accesskit::Action::Click, details_id);
    cx.run_until_parked();
    let selected = cx
        .latest_accessibility_tree_update()
        .expect("tab selection should publish");
    let (selected_tablist_id, _) = node_with_role(&selected, accesskit::Role::TabList);
    let (selected_panel_id, selected_panel) = node_with_role(&selected, accesskit::Role::TabPanel);
    let (selected_overview_id, selected_overview) = a11y_node_with_label(&selected, "Overview");
    let (selected_details_id, selected_details) = a11y_node_with_label(&selected, "Details");
    let (selected_billing_id, selected_billing) = a11y_node_with_label(&selected, "Billing");

    assert_eq!(selected_tablist_id, tablist_id);
    assert_eq!(selected_panel_id, panel_id);
    assert_eq!(selected_overview_id, overview_id);
    assert_eq!(selected_details_id, details_id);
    assert_eq!(selected_billing_id, billing_id);
    assert_eq!(selected_overview.is_selected(), Some(false));
    assert_eq!(selected_details.is_selected(), Some(true));
    assert_eq!(selected_billing.is_selected(), Some(false));
    assert_eq!(selected_details.controls(), &[selected_panel_id]);
    assert_eq!(selected_panel.labelled_by(), &[selected_details_id]);

    dispatch_action(cx, accesskit::Action::Click, billing_id);
    cx.run_until_parked();
    let after_disabled_action = cx
        .latest_accessibility_tree_update()
        .expect("disabled tab action should preserve the final tree");
    let (after_panel_id, after_panel) =
        node_with_role(&after_disabled_action, accesskit::Role::TabPanel);
    let (after_details_id, after_details) = a11y_node_with_label(&after_disabled_action, "Details");
    let (after_billing_id, after_billing) = a11y_node_with_label(&after_disabled_action, "Billing");
    assert_eq!(after_panel_id, panel_id);
    assert_eq!(after_details_id, details_id);
    assert_eq!(after_billing_id, billing_id);
    assert_eq!(after_details.is_selected(), Some(true));
    assert_eq!(after_billing.is_selected(), Some(false));
    assert!(after_billing.is_disabled());
    assert_eq!(after_panel.labelled_by(), &[after_details_id]);
}

#[open_gpui::test]
fn tabs_vertical_final_tree_panel_relation_uses_scrolled_trigger_node_id(
    cx: &mut open_gpui::TestAppContext,
) {
    struct VerticalTabsA11yProbe;

    impl Render for VerticalTabsA11yProbe {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            div().size_full().child(
                Tabs::new("vertical-a11y-navigation-tabs")
                    .orientation(Orientation::Vertical)
                    .default_selected("overview")
                    .item(TabsItem::new(
                        "overview",
                        "Vertical overview",
                        div().child("Overview panel"),
                    ))
                    .item(TabsItem::new(
                        "details",
                        "Vertical details",
                        div().child("Details panel"),
                    )),
            )
        }
    }

    let (_, cx) = cx.add_window_view(|_, _| VerticalTabsA11yProbe);
    assert!(cx.activate_accessibility());

    let update = cx
        .latest_accessibility_tree_update()
        .expect("vertical tabs accessibility tree should publish");
    let (selected_tab_id, selected_tab) = a11y_node_with_label(&update, "Vertical overview");
    let (panel_id, panel) = update
        .nodes
        .iter()
        .find(|(_, node)| node.role() == accesskit::Role::TabPanel)
        .map(|(id, node)| (*id, node))
        .expect("vertical tabs should publish a tab panel");

    assert_eq!(selected_tab.role(), accesskit::Role::Tab);
    assert_eq!(selected_tab.controls(), &[panel_id]);
    assert_eq!(panel.labelled_by(), &[selected_tab_id]);
}

#[open_gpui::test]
fn tabs_runtime_manual_keyboard_activation_preserves_selected_seed_and_payloads(
    cx: &mut open_gpui::TestAppContext,
) {
    struct TestView {
        selections: Rc<RefCell<Vec<TabsSelection>>>,
    }

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let selections = self.selections.clone();

            div().size_full().child(
                Tabs::new("runtime-tabs")
                    .activation_mode(TabsActivationMode::Manual)
                    .default_selected("details")
                    .item(TabsItem::new(
                        "overview",
                        "Overview",
                        div()
                            .debug_selector(|| "tabs-panel:overview".to_string())
                            .child("Overview panel"),
                    ))
                    .item(
                        TabsItem::new(
                            "billing",
                            "Billing",
                            div()
                                .debug_selector(|| "tabs-panel:billing".to_string())
                                .child("Billing panel"),
                        )
                        .disabled(true),
                    )
                    .item(TabsItem::new(
                        "details",
                        "Details",
                        div()
                            .debug_selector(|| "tabs-panel:details".to_string())
                            .child("Details panel"),
                    ))
                    .item(TabsItem::new(
                        "history",
                        "History",
                        div()
                            .debug_selector(|| "tabs-panel:history".to_string())
                            .child("History panel"),
                    ))
                    .on_selection_change(move |selection, _, _| {
                        selections.borrow_mut().push(selection);
                    }),
            )
        }
    }

    let selections = Rc::new(RefCell::new(Vec::new()));
    let (_, cx) = cx.add_window_view(|_, _| TestView {
        selections: selections.clone(),
    });
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    assert!(
        cx.debug_bounds("tabs-panel:details").is_some(),
        "expected seeded selected tab to render the Details panel"
    );

    let disabled_billing = cx
        .debug_bounds("tabs:runtime-tabs:trigger:billing")
        .expect("disabled Billing tab trigger should be rendered");
    cx.simulate_click(disabled_billing.center(), Default::default());
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });
    assert!(
        selections.borrow().is_empty(),
        "disabled tab click should not emit a selection change"
    );
    assert!(
        cx.debug_bounds("tabs-panel:details").is_some(),
        "disabled tab click should keep the current selected panel"
    );

    let overview = cx
        .debug_bounds("tabs:runtime-tabs:trigger:overview")
        .expect("Overview tab trigger should be rendered");
    cx.simulate_click(overview.center(), Default::default());
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });
    let after_click = selections.borrow().clone();
    assert_eq!(after_click.len(), 1);
    assert_eq!(after_click[0].index(), 0);
    assert_eq!(after_click[0].value(), "overview");
    assert_eq!(after_click[0].label(), "Overview");
    assert!(
        cx.debug_bounds("tabs-panel:overview").is_some(),
        "enabled tab click should render the selected panel"
    );

    cx.simulate_keystrokes("right");
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });
    assert_eq!(
        selections.borrow().len(),
        1,
        "manual activation should move roving focus without selecting on arrow key"
    );
    assert!(
        cx.debug_bounds("tabs-panel:overview").is_some(),
        "manual activation should keep the selected panel until Enter or Space"
    );

    cx.simulate_event(key_down("enter"));
    cx.simulate_event(key_up("enter"));
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });
    let after_enter = selections.borrow().clone();
    assert_eq!(after_enter.len(), 2);
    assert_eq!(after_enter[1].index(), 2);
    assert_eq!(after_enter[1].value(), "details");
    assert_eq!(after_enter[1].label(), "Details");
    assert!(
        cx.debug_bounds("tabs-panel:details").is_some(),
        "Enter should activate the focused tab after keyboard navigation skips disabled tabs"
    );

    cx.simulate_keystrokes("home");
    cx.simulate_event(key_down("enter"));
    cx.simulate_event(key_up("enter"));
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });
    let after_home = selections.borrow().clone();
    assert_eq!(after_home.len(), 3);
    assert_eq!(after_home[2].index(), 0);
    assert_eq!(after_home[2].value(), "overview");
    assert_eq!(after_home[2].label(), "Overview");
    assert!(
        cx.debug_bounds("tabs-panel:overview").is_some(),
        "Home plus Enter should activate the first enabled tab in manual mode"
    );

    cx.simulate_keystrokes("end");
    cx.simulate_event(key_down("space"));
    cx.simulate_event(key_up("space"));
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });
    let after_space = selections.borrow().clone();
    assert_eq!(after_space.len(), 4);
    assert_eq!(after_space[3].index(), 3);
    assert_eq!(after_space[3].value(), "history");
    assert_eq!(after_space[3].label(), "History");
    assert!(
        cx.debug_bounds("tabs-panel:history").is_some(),
        "End plus Space should activate the last enabled tab in manual mode"
    );
}

#[open_gpui::test]
fn toolbar_runtime_keyboard_navigation_skips_disabled_and_separator_items(
    cx: &mut open_gpui::TestAppContext,
) {
    struct TestView {
        selections: Rc<RefCell<Vec<ToolbarActivation>>>,
    }

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let selections = self.selections.clone();

            div().size_full().child(
                Toolbar::new("keyboard-toolbar", "Keyboard toolbar")
                    .small()
                    .default_focused("bold")
                    .item(ToolbarItem::icon("undo", "U", "Undo"))
                    .item(ToolbarItem::icon("redo", "R", "Redo").disabled(true))
                    .item(ToolbarItem::separator("history-separator"))
                    .item(ToolbarItem::toggle_icon("bold", "B", "Bold").pressed(true))
                    .item(ToolbarItem::toggle_icon("italic", "I", "Italic"))
                    .on_activate(move |selection, _, _, _| {
                        selections.borrow_mut().push(selection);
                    }),
            )
        }
    }

    let selections = Rc::new(RefCell::new(Vec::new()));
    let (_, cx) = cx.add_window_view(|_, _| TestView {
        selections: selections.clone(),
    });
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    let undo = cx
        .debug_bounds("toolbar:keyboard-toolbar:item:undo")
        .expect("undo toolbar item should be rendered");
    cx.simulate_click(undo.center(), Default::default());
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    cx.simulate_keystrokes("right");
    cx.simulate_event(key_down("space"));
    cx.simulate_event(key_up("space"));
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });
    let after_right = selections.borrow().clone();
    assert_eq!(after_right.len(), 2);
    assert_eq!(after_right[0].value(), "undo");
    assert_eq!(after_right[0].kind(), ToolbarItemKind::Action);
    assert_eq!(after_right[1].value(), "bold");
    assert_eq!(after_right[1].kind(), ToolbarItemKind::Toggle);

    cx.simulate_keystrokes("right");
    cx.simulate_event(key_down("space"));
    cx.simulate_event(key_up("space"));
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });
    let after_second_right = selections.borrow().clone();
    assert_eq!(after_second_right.len(), 3);
    assert_eq!(after_second_right[2].value(), "italic");
    assert_eq!(after_second_right[2].kind(), ToolbarItemKind::Toggle);
    assert!(!after_second_right[2].pressed());

    cx.simulate_keystrokes("home");
    cx.simulate_event(key_down("enter"));
    cx.simulate_event(key_up("enter"));
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });
    let after_home = selections.borrow().clone();
    assert_eq!(after_home.len(), 4);
    assert_eq!(after_home[3].value(), "undo");
    assert_eq!(after_home[3].kind(), ToolbarItemKind::Action);
}

#[open_gpui::test]
fn toggle_group_controlled_values_override_runtime_selection(cx: &mut open_gpui::TestAppContext) {
    struct TestView {
        changes: Rc<RefCell<Vec<Vec<String>>>>,
    }

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let changes = self.changes.clone();

            div().size_full().child(
                ToggleGroup::new("controlled-toggle-group", "Alignment")
                    .default_selected_values(["right"])
                    .selected_values(Vec::<String>::new())
                    .item(ToggleGroupItem::new("left", "Left"))
                    .item(ToggleGroupItem::new("right", "Right"))
                    .on_change(move |change, _, _| {
                        changes.borrow_mut().push(change.selected_values().to_vec());
                    }),
            )
        }
    }

    let changes = Rc::new(RefCell::new(Vec::new()));
    let (_, cx) = cx.add_window_view(|_, _| TestView {
        changes: changes.clone(),
    });
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    let left = cx
        .debug_bounds("toggle-group:controlled-toggle-group:item:left")
        .expect("left toggle item should expose a stable debug selector");
    cx.simulate_click(left.center(), Default::default());
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    assert_eq!(changes.borrow().as_slice(), &[vec!["left".to_string()]]);

    let left = cx
        .debug_bounds("toggle-group:controlled-toggle-group:item:left")
        .expect("left toggle item should remain rendered after controlled redraw");
    cx.simulate_click(left.center(), Default::default());
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    assert_eq!(
        changes.borrow().as_slice(),
        &[vec!["left".to_string()], vec!["left".to_string()]],
        "controlled empty selection should reset adapter runtime before each activation"
    );
}

#[test]
fn sidebar_state_exposes_shell_navigation_contract() {
    let state = SidebarState::resolve(
        SidebarSide::Left,
        SidebarVariant::Docked,
        SidebarCollapseMode::Icon,
        false,
        false,
        "Primary navigation",
        Some("projects"),
        None,
        [
            SidebarSectionDescriptor::new("workspace", "Workspace").items([
                SidebarItemDescriptor::new("home", "Home").icon("H"),
                SidebarItemDescriptor::new("projects", "Projects")
                    .icon("P")
                    .badge("12"),
                SidebarItemDescriptor::new("archive", "Archive")
                    .icon("A")
                    .disabled(true),
            ]),
            SidebarSectionDescriptor::new("account", "Account").items([
                SidebarItemDescriptor::new("settings", "Settings").icon("S"),
                SidebarItemDescriptor::new("billing", "Billing")
                    .icon("B")
                    .action_label("new"),
            ]),
        ],
        Size::Medium,
        ThemeTokens::default(),
    );

    assert_eq!(state.role(), Role::Navigation);
    assert_eq!(state.side(), SidebarSide::Left);
    assert_eq!(state.variant(), SidebarVariant::Docked);
    assert_eq!(state.collapse_mode(), SidebarCollapseMode::Icon);
    assert!(!state.collapsed());
    assert_eq!(state.sections().len(), 2);
    assert_eq!(state.sections()[0].role(), Role::Section);
    assert_eq!(state.items().len(), 5);
    assert_eq!(state.selected_value(), Some("projects"));
    assert_eq!(state.focused_value(), Some("projects"));
    assert_eq!(state.focused_index(), Some(1));
    assert!(state.scrollable());
    assert!(state.items()[1].selected());
    assert_eq!(state.items()[1].badge_label(), Some("12"));
    assert!(!state.items()[2].activation_enabled());
    assert_eq!(state.items()[1].role(), Role::Button);
    assert_eq!(state.items()[1].position_in_set(), Some(2));
    assert_eq!(state.items()[1].size_of_set(), 4);
    assert_eq!(
        state.navigation_target("down").map(|item| item.value()),
        Some("settings")
    );
}

#[test]
fn sidebar_state_fails_closed_for_duplicate_values() {
    let state = SidebarState::resolve(
        SidebarSide::Left,
        SidebarVariant::Docked,
        SidebarCollapseMode::Icon,
        false,
        false,
        "Duplicate navigation",
        Some("duplicate"),
        Some("duplicate"),
        [SidebarSectionDescriptor::new("main", "Main").items([
            SidebarItemDescriptor::new("duplicate", "Duplicate first"),
            SidebarItemDescriptor::new("duplicate", "Duplicate second"),
            SidebarItemDescriptor::new("unique", "Unique"),
        ])],
        Size::Medium,
        ThemeTokens::default(),
    );

    assert_eq!(state.selected_value(), None);
    assert_eq!(state.focused_value(), Some("unique"));
    assert_eq!(state.focused_index(), Some(2));
    assert_eq!(state.items()[2].position_in_set(), Some(1));
    assert_eq!(state.items()[2].size_of_set(), 1);

    for duplicate in &state.items()[..2] {
        assert!(duplicate.duplicate_value());
        assert!(!duplicate.focusable());
        assert!(!duplicate.activation_enabled());
        assert_eq!(duplicate.position_in_set(), None);
        assert_eq!(duplicate.size_of_set(), 0);
    }
}

#[test]
fn sidebar_icon_collapse_keeps_accessible_items_but_hides_text() {
    let state = Sidebar::new("app-sidebar", "Application")
        .collapse_mode(SidebarCollapseMode::Icon)
        .collapsed(true)
        .selected("dashboard")
        .section(
            SidebarSection::new("main", "Main")
                .item(SidebarItem::new("dashboard", "Dashboard").icon("D"))
                .item(SidebarItem::new("inbox", "Inbox").icon("I").badge("4")),
        )
        .state();

    assert!(state.collapsed());
    assert!(state.icon_collapsed());
    assert!(!state.offcanvas_collapsed());
    assert_eq!(
        state.metrics().resolved_width(),
        state.metrics().collapsed_width()
    );
    assert_eq!(state.selected_value(), Some("dashboard"));
    assert_eq!(state.focused_value(), Some("dashboard"));
    assert!(state.scrollable());
    assert!(state.items()[0].focusable());
    assert_eq!(state.items()[0].label(), "Dashboard");
    assert_eq!(state.items()[1].badge_label(), Some("4"));
}

#[test]
fn sidebar_offcanvas_collapse_removes_items_from_roving_focus() {
    let state = SidebarState::resolve(
        SidebarSide::Right,
        SidebarVariant::Floating,
        SidebarCollapseMode::Offcanvas,
        true,
        false,
        "Secondary navigation",
        Some("reports"),
        None,
        [SidebarSectionDescriptor::new("main", "Main").items([
            SidebarItemDescriptor::new("overview", "Overview"),
            SidebarItemDescriptor::new("reports", "Reports"),
        ])],
        Size::Small,
        ThemeTokens::default(),
    );

    assert!(state.collapsed());
    assert!(state.offcanvas_collapsed());
    assert_eq!(state.metrics().resolved_width(), ui_px(0.0));
    assert_eq!(state.selected_value(), None);
    assert_eq!(state.focused_value(), None);
    assert_eq!(state.focused_index(), None);
    assert!(!state.scrollable());
    assert!(!state.items()[0].focusable());
}

#[open_gpui::test]
fn sidebar_long_navigation_scrolls_inside_shared_scroll_area(cx: &mut open_gpui::TestAppContext) {
    struct TestView;

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let section = (0..14).fold(SidebarSection::new("main", "Main"), |section, index| {
                section.item(
                    SidebarItem::new(format!("item-{index}"), format!("Item {index}"))
                        .icon(index.to_string()),
                )
            });

            div().size_full().child(
                div()
                    .h(px(120.0))
                    .child(Sidebar::new("long-sidebar", "Long navigation").section(section)),
            )
        }
    }

    let (_, cx) = cx.add_window_view(|_, _| TestView);
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    let item_before = cx
        .debug_bounds("sidebar:long-sidebar:item:item-2")
        .expect("sidebar item should be rendered before scrolling");
    let sidebar = cx
        .debug_bounds("sidebar:long-sidebar")
        .expect("sidebar shell should be rendered");
    let sidebar_viewport = cx
        .debug_bounds("scroll-area:sidebar:long-sidebar:scroll")
        .expect("long Sidebar should use the shared ScrollArea viewport");

    assert!(
        sidebar.contains(&sidebar_viewport.center()),
        "expected Sidebar ScrollArea viewport to stay inside the sidebar shell; sidebar={sidebar:?} viewport={sidebar_viewport:?}"
    );

    cx.simulate_event(ScrollWheelEvent {
        position: sidebar_viewport.center(),
        delta: ScrollDelta::Pixels(point(px(0.0), px(-72.0))),
        ..Default::default()
    });
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    let item_after = cx
        .debug_bounds("sidebar:long-sidebar:item:item-2")
        .expect("sidebar item should remain rendered after scrolling");

    assert!(
        item_after.top() < item_before.top(),
        "expected long Sidebar navigation to scroll inside its ScrollArea; before={item_before:?} after={item_after:?}"
    );
}

#[test]
fn sidebar_navigation_helper_skips_disabled_items() {
    assert_eq!(
        sidebar_navigation_target("down", 0, &[false, true, false]),
        Some(2)
    );
    assert_eq!(
        sidebar_navigation_target("up", 0, &[false, true, false]),
        Some(2)
    );
    assert_eq!(
        sidebar_navigation_target("home", 2, &[false, true, false]),
        Some(0)
    );
    assert_eq!(sidebar_navigation_target("right", 0, &[false, false]), None);
}

#[test]
fn toolbar_state_exposes_roving_focus_and_toggle_metadata() {
    let state = ToolbarState::resolve(
        Orientation::Horizontal,
        Size::Small,
        false,
        "Editor toolbar",
        Some("bold"),
        [
            ToolbarItemDescriptor::action("undo", "Undo"),
            ToolbarItemDescriptor::separator("history-separator"),
            ToolbarItemDescriptor::toggle("bold", "Bold").pressed(true),
            ToolbarItemDescriptor::toggle("italic", "Italic").disabled(true),
            ToolbarItemDescriptor::action("save", "Save"),
        ],
        ThemeTokens::default(),
    );

    assert_eq!(state.role(), Role::Toolbar);
    assert_eq!(state.orientation(), Orientation::Horizontal);
    assert_eq!(state.size(), Size::Small);
    assert_eq!(state.label(), "Editor toolbar");
    assert_eq!(state.items().len(), 5);
    assert_eq!(state.focused_value(), Some("bold"));
    assert_eq!(state.tab_stop_index(), state.focused_index());
    assert_eq!(state.items()[0].role(), Some(Role::Button));
    assert_eq!(state.items()[1].kind(), ToolbarItemKind::Separator);
    assert_eq!(state.items()[1].role(), None);
    assert!(!state.items()[1].focusable());
    assert!(state.items()[2].pressed());
    assert_eq!(state.items()[2].toggled(), Some(Toggled::True));
    assert!(!state.items()[3].activation_enabled());
    assert_eq!(
        state.navigation_target("right").map(|item| item.value()),
        Some("save")
    );
    assert_eq!(
        state
            .activation_for_key("space")
            .map(|selection| (selection.value().to_owned(), selection.kind())),
        Some(("bold".to_string(), ToolbarItemKind::Toggle))
    );
}

#[test]
fn toolbar_builder_state_skips_disabled_and_separator_items() {
    let state = Toolbar::new("editor-tools", "Editor")
        .orientation(Orientation::Vertical)
        .large()
        .default_focused("missing")
        .item(ToolbarItem::action("cut", "Cut").disabled(true))
        .item(ToolbarItem::separator("clipboard-separator"))
        .item(ToolbarItem::icon("copy", "C", "Copy"))
        .item(ToolbarItem::toggle("wrap", "Wrap").pressed(true))
        .state();

    assert_eq!(state.orientation(), Orientation::Vertical);
    assert_eq!(state.size(), Size::Large);
    assert_eq!(state.focused_value(), Some("copy"));
    assert_eq!(state.tab_stop_index(), state.focused_index());
    assert!(state.items()[0].disabled());
    assert_eq!(state.items()[1].kind(), ToolbarItemKind::Separator);
    assert!(state.items()[3].pressed());
    assert_eq!(
        toolbar_navigation_target(
            Orientation::Vertical,
            "down",
            state.focused_index().unwrap(),
            &[true, true, false, false],
        ),
        Some(3)
    );
}

#[test]
fn toolbar_duplicate_values_fail_closed() {
    let state = Toolbar::new("duplicate-tools", "Duplicate tools")
        .item(ToolbarItem::action("duplicate", "Duplicate action"))
        .item(ToolbarItem::toggle("duplicate", "Duplicate toggle"))
        .state();

    assert_eq!(state.focused_value(), None);
    assert!(state.items().iter().all(|item| item.duplicate_value()));
    assert!(state.items().iter().all(|item| item.disabled()));
    assert!(state.activation_for_key("enter").is_none());
    assert!(state.activation_for_key("space").is_none());
}

#[test]
fn toolbar_items_accept_tooltip_builders() {
    let state = Toolbar::new("editor-tools", "Editor")
        .item(ToolbarItem::icon("undo", "U", "Undo").tooltip(Tooltip::text("Undo")))
        .item(ToolbarItem::separator("separator").tooltip(Tooltip::text("Ignored")))
        .state();

    assert_eq!(state.items().len(), 2);
    assert_eq!(state.items()[0].value(), "undo");
    assert_eq!(state.items()[1].kind(), ToolbarItemKind::Separator);
}
