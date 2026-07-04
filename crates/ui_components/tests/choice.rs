mod support;

use open_gpui::{
    Context, InteractiveElement, IntoElement, MouseButton, ParentElement, Render, ScrollDelta,
    ScrollWheelEvent, Styled, Window, actions, div, point, px,
};
use open_gpui_ui_components::{
    Combobox, ComboboxGroup, ComboboxOpenMode, ComboboxOption, ComboboxSelection, Command,
    CommandGroup, CommandGroupDescriptor, CommandIndexSnapshot, CommandIndexSnapshotMode,
    CommandItem, CommandItemDescriptor, CommandLoadingState, CommandMatchSource, CommandOpenMode,
    CommandPaletteController, CommandPaletteKeymapPreflight, CommandPaletteProjection,
    CommandProviderPaletteProjection, CommandQueryMode, CommandSelection, CommandSelectionChange,
    CommandSelectionMode, CommandStatusIntent, CommandStatusItem, Listbox, ListboxGroup,
    ListboxGroupDescriptor, ListboxOption, ListboxOptionDescriptor, ListboxOptionKind,
    ListboxSelection, ListboxState, ScrollArea, ScrollResetPolicy, Select, SelectOpenMode,
    SelectSelection, VirtualizerRange, gpui_adapter::init_text_input, listbox_navigation_target,
};
use open_gpui_ui_core::{
    EscapeKeyPolicy, FocusRestoreIntent, InitialFocusIntent, OutsidePressPolicy, OverlayLayerKind,
    OverlayPlacementAlignment, OverlayPlacementSide, Role, Sizable, Size, ThemeTokens, ui_px,
};
use std::cell::RefCell;
use std::rc::Rc;
use support::tokens::custom_tokens;

actions!(
    command_palette_projection_test,
    [OpenPaletteCommand, RevealPaletteCommand]
);

#[test]
fn listbox_state_resolves_grouped_options_navigation_and_typeahead() {
    let state = ListboxState::resolve(
        Size::Small,
        false,
        "Assignee",
        Some("bravo"),
        Some("missing"),
        Some("ch"),
        "No assignees",
        [ListboxGroupDescriptor::new("team", "Team")
            .option(ListboxOptionDescriptor::option("charlie", "Charlie"))
            .option(ListboxOptionDescriptor::option("delta", "Delta").disabled(true))
            .option(ListboxOptionDescriptor::option("bravo", "Bravo"))],
        [
            ListboxOptionDescriptor::option("alpha", "Alpha"),
            ListboxOptionDescriptor::separator("standalone-separator"),
        ],
        ThemeTokens::default(),
    );

    assert_eq!(state.role(), Role::ListBox);
    assert_eq!(state.label(), "Assignee");
    assert_eq!(state.typeahead_query(), Some("ch"));
    assert_eq!(state.groups().len(), 1);
    assert_eq!(state.groups()[0].role(), Role::Group);
    assert_eq!(state.groups()[0].option_count(), 3);
    assert_eq!(state.options().len(), 5);
    assert_eq!(state.selected_value(), Some("bravo"));
    assert_eq!(state.active_value(), Some("bravo"));
    assert_eq!(state.options()[1].kind(), ListboxOptionKind::Separator);
    assert_eq!(state.options()[1].role(), None);
    assert!(!state.options()[1].focusable());
    assert!(state.options()[3].disabled());
    assert!(!state.options()[3].focusable());
    assert_eq!(state.options()[4].role(), Some(Role::ListBoxOption));
    assert_eq!(state.options()[4].position_in_set(), Some(4));
    assert_eq!(state.options()[4].size_of_set(), 4);
    assert_eq!(
        state.navigation_target("down").map(|option| option.value()),
        Some("alpha")
    );
    assert_eq!(
        state.typeahead_target("ch").map(|option| option.value()),
        Some("charlie")
    );
    assert_eq!(
        state
            .activation_for_key("enter")
            .map(|selection| selection.value().to_owned()),
        Some("bravo".to_string())
    );
    assert_eq!(
        listbox_navigation_target(
            "down",
            state.active_index().unwrap(),
            &[false, true, false, true, false]
        ),
        Some(0)
    );
}

#[test]
fn choice_surfaces_share_stable_value_resolution_and_query_normalization() {
    let listbox = ListboxState::resolve(
        Size::Small,
        false,
        "Shared choices",
        Some("disabled"),
        Some("missing"),
        Some("  AL "),
        "No choices",
        [ListboxGroupDescriptor::new("group", "Group")
            .option(ListboxOptionDescriptor::option("grouped", "Grouped"))],
        [
            ListboxOptionDescriptor::option("alpha", "Alpha"),
            ListboxOptionDescriptor::option("disabled", "Disabled").disabled(true),
        ],
        ThemeTokens::default(),
    );

    let select = Select::new("shared-select", "Shared choices")
        .placeholder("Pick one")
        .selected("disabled")
        .option(ListboxOption::new("alpha", "Alpha"))
        .option(ListboxOption::new("disabled", "Disabled").disabled(true))
        .group(ListboxGroup::new("group", "Group").option(ListboxOption::new("grouped", "Grouped")))
        .state();

    let combobox = Combobox::new("shared-combobox", "Shared choices")
        .default_query("  AL ")
        .selected("disabled")
        .option(ComboboxOption::new("alpha", "Alpha"))
        .option(ComboboxOption::new("disabled", "Disabled").disabled(true))
        .group(
            ComboboxGroup::new("group", "Group").option(ComboboxOption::new("grouped", "Grouped")),
        )
        .state();

    let command = Command::new("shared-command", "Shared choices")
        .default_query("  AL ")
        .selected("disabled")
        .item(CommandItem::new("alpha", "Alpha"))
        .item(CommandItem::new("disabled", "Disabled").disabled(true))
        .group(CommandGroup::new("group", "Group").item(CommandItem::new("grouped", "Grouped")))
        .state();

    assert_eq!(listbox.typeahead_query(), Some("al"));
    assert_eq!(listbox.selected_value(), None);
    assert_eq!(listbox.active_value(), Some("alpha"));
    assert_eq!(
        listbox
            .typeahead_target("  AL ")
            .map(|option| option.value()),
        Some("alpha")
    );

    assert_eq!(select.selected_value(), None);
    assert_eq!(select.active_value(), Some("alpha"));
    assert_eq!(select.trigger_label(), "Pick one");

    assert_eq!(combobox.query(), "  AL ");
    assert_eq!(combobox.filtered_option_count(), 1);
    assert_eq!(combobox.selected_value(), None);
    assert_eq!(combobox.active_value(), Some("alpha"));
    assert_eq!(combobox.listbox().typeahead_query(), Some("al"));

    assert_eq!(command.query(), "  AL ");
    assert_eq!(command.filtered_item_count(), 1);
    assert_eq!(command.selected_value(), None);
    assert_eq!(command.active_value(), Some("alpha"));
    assert_eq!(command.listbox().typeahead_query(), Some("al"));
}

#[test]
fn listbox_select_and_combobox_project_equivalent_choice_semantics() {
    let listbox = ListboxState::resolve(
        Size::Small,
        false,
        "Shared choices",
        Some("bravo"),
        Some("charlie"),
        None,
        "No choices",
        [],
        [
            ListboxOptionDescriptor::option("alpha", "Alpha"),
            ListboxOptionDescriptor::option("bravo", "Bravo"),
            ListboxOptionDescriptor::option("disabled", "Disabled").disabled(true),
            ListboxOptionDescriptor::option("charlie", "Charlie"),
        ],
        ThemeTokens::default(),
    );
    let select = Select::new("shared-select-semantics", "Shared choices")
        .placeholder("Pick one")
        .selected("bravo")
        .active("charlie")
        .option(ListboxOption::new("alpha", "Alpha"))
        .option(ListboxOption::new("bravo", "Bravo"))
        .option(ListboxOption::new("disabled", "Disabled").disabled(true))
        .option(ListboxOption::new("charlie", "Charlie"))
        .state();
    let combobox = Combobox::new("shared-combobox-semantics", "Shared choices")
        .placeholder("Search choices")
        .selected("bravo")
        .active("charlie")
        .option(ComboboxOption::new("alpha", "Alpha"))
        .option(ComboboxOption::new("bravo", "Bravo"))
        .option(ComboboxOption::new("disabled", "Disabled").disabled(true))
        .option(ComboboxOption::new("charlie", "Charlie"))
        .state();

    for state in [
        listbox,
        select.listbox().clone(),
        combobox.listbox().clone(),
    ] {
        assert_eq!(state.selected_value(), Some("bravo"));
        assert_eq!(state.active_value(), Some("charlie"));
        assert_eq!(
            state.selected_option().map(|option| option.value()),
            Some("bravo")
        );
        assert_eq!(
            state.active_option().map(|option| option.value()),
            Some("charlie")
        );
        assert_eq!(
            state.typeahead_target(" al").map(|option| option.value()),
            Some("alpha")
        );
        assert!(state.options()[1].selected());
        assert!(state.options()[2].disabled());
        assert!(!state.options()[2].focusable());
        assert!(state.options()[3].active());
    }

    assert_eq!(select.trigger_label(), "Bravo");
    assert_eq!(combobox.selected_value(), Some("bravo"));
}

#[test]
fn listbox_state_scrollable_content_tracks_flattened_option_count_threshold() {
    let scrollable = ListboxState::resolve(
        Size::Small,
        false,
        "Scrollable",
        None,
        None,
        None,
        "No options",
        [],
        (0..7).map(|index| {
            ListboxOptionDescriptor::option(format!("item-{index}"), format!("Item {index}"))
        }),
        ThemeTokens::default(),
    );
    let not_scrollable = ListboxState::resolve(
        Size::Small,
        false,
        "Compact",
        None,
        None,
        None,
        "No options",
        [],
        (0..6).map(|index| {
            ListboxOptionDescriptor::option(format!("item-{index}"), format!("Item {index}"))
        }),
        ThemeTokens::default(),
    );

    assert!(scrollable.scrollable_content());
    assert!(!not_scrollable.scrollable_content());
}

#[test]
fn listbox_builder_state_models_empty_disabled_and_tokens() {
    let tokens = custom_tokens();
    let empty = Listbox::new("empty-listbox", "Empty")
        .empty_label("Nothing available")
        .tokens(tokens)
        .state();
    let disabled = Listbox::new("disabled-listbox", "Disabled")
        .disabled(true)
        .selected("one")
        .option(ListboxOption::new("one", "One"))
        .state();

    assert!(empty.empty());
    assert_eq!(empty.empty_label(), "Nothing available");
    assert_eq!(empty.colors().surface().token(), tokens.surface);
    assert!(disabled.disabled());
    assert_eq!(disabled.selected_value(), None);
    assert_eq!(disabled.active_value(), None);
    assert_eq!(disabled.activation_for_key("space"), None);
}

#[open_gpui::test]
fn listbox_runtime_click_and_keyboard_selection_skip_disabled_items(
    cx: &mut open_gpui::TestAppContext,
) {
    #[derive(Clone, Debug, PartialEq, Eq)]
    struct SelectionEvent {
        source: &'static str,
        selection: ListboxSelection,
    }

    struct TestView {
        events: Rc<RefCell<Vec<SelectionEvent>>>,
    }

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let listbox_events = self.events.clone();
            let alpha_events = self.events.clone();
            let charlie_events = self.events.clone();

            div().size_full().child(
                Listbox::new("runtime-listbox", "Runtime listbox")
                    .selected("alpha")
                    .option(ListboxOption::new("alpha", "Alpha").on_select(
                        move |selection, _, _| {
                            alpha_events.borrow_mut().push(SelectionEvent {
                                source: "option:alpha",
                                selection,
                            });
                        },
                    ))
                    .option(ListboxOption::separator("standalone-separator"))
                    .option(ListboxOption::new("bravo", "Bravo").disabled(true))
                    .group(
                        ListboxGroup::new("team", "Team")
                            .option(ListboxOption::new("charlie", "Charlie").on_select(
                                move |selection, _, _| {
                                    charlie_events.borrow_mut().push(SelectionEvent {
                                        source: "option:charlie",
                                        selection,
                                    });
                                },
                            ))
                            .option(ListboxOption::new("delta", "Delta")),
                    )
                    .on_select(move |selection, _, _| {
                        listbox_events.borrow_mut().push(SelectionEvent {
                            source: "listbox",
                            selection,
                        });
                    }),
            )
        }
    }

    let events = Rc::new(RefCell::new(Vec::new()));
    let (_, cx) = cx.add_window_view(|_, _| TestView {
        events: events.clone(),
    });
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    assert!(
        cx.debug_bounds("listbox:runtime-listbox").is_some(),
        "listbox root should expose a stable debug selector"
    );
    assert!(
        cx.debug_bounds("listbox:runtime-listbox:separator:standalone-separator")
            .is_some(),
        "listbox separator should expose a stable debug selector"
    );
    assert!(
        cx.debug_bounds("listbox:runtime-listbox:group:team")
            .is_some(),
        "listbox group label should expose a stable debug selector"
    );

    let disabled_bravo = cx
        .debug_bounds("listbox:runtime-listbox:option:bravo")
        .expect("disabled Bravo option should be rendered");
    cx.simulate_click(disabled_bravo.center(), Default::default());
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });
    assert!(
        events.borrow().is_empty(),
        "disabled option click should not emit selection callbacks"
    );

    let delta = cx
        .debug_bounds("listbox:runtime-listbox:option:delta")
        .expect("Delta option should be rendered");
    cx.simulate_click(delta.center(), Default::default());
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });
    let after_delta_click = events.borrow().clone();
    assert_eq!(after_delta_click.len(), 1);
    assert_eq!(after_delta_click[0].source, "listbox");
    assert_eq!(after_delta_click[0].selection.index(), 4);
    assert_eq!(after_delta_click[0].selection.value(), "delta");
    assert_eq!(after_delta_click[0].selection.label(), "Delta");

    let alpha = cx
        .debug_bounds("listbox:runtime-listbox:option:alpha")
        .expect("Alpha option should be rendered");
    cx.simulate_click(alpha.center(), Default::default());
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });
    let after_alpha_click = events.borrow().clone();
    assert_eq!(after_alpha_click.len(), 3);
    assert_eq!(after_alpha_click[1].source, "option:alpha");
    assert_eq!(after_alpha_click[1].selection.index(), 0);
    assert_eq!(after_alpha_click[1].selection.value(), "alpha");
    assert_eq!(after_alpha_click[2].source, "listbox");
    assert_eq!(after_alpha_click[2].selection.value(), "alpha");

    cx.simulate_keystrokes("down");
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });
    assert_eq!(
        events.borrow().len(),
        3,
        "arrow navigation should move active option without selecting"
    );

    cx.simulate_keystrokes("enter");
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });
    let after_enter = events.borrow().clone();
    assert_eq!(after_enter.len(), 5);
    assert_eq!(after_enter[3].source, "option:charlie");
    assert_eq!(after_enter[3].selection.index(), 3);
    assert_eq!(after_enter[3].selection.value(), "charlie");
    assert_eq!(after_enter[3].selection.label(), "Charlie");
    assert_eq!(after_enter[4].source, "listbox");
    assert_eq!(after_enter[4].selection.value(), "charlie");

    cx.simulate_keystrokes("up");
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });
    assert_eq!(
        events.borrow().len(),
        5,
        "arrow navigation after selection should still move active option without selecting"
    );

    cx.simulate_keystrokes("space");
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });
    let after_space = events.borrow().clone();
    assert_eq!(after_space.len(), 7);
    assert_eq!(after_space[5].source, "option:alpha");
    assert_eq!(after_space[5].selection.index(), 0);
    assert_eq!(after_space[5].selection.value(), "alpha");
    assert_eq!(after_space[5].selection.label(), "Alpha");
    assert_eq!(after_space[6].source, "listbox");
    assert_eq!(after_space[6].selection.value(), "alpha");
}

#[test]
fn select_state_records_popup_listbox_overlay_and_scroll_contract() {
    let state = Select::new("priority-select", "Priority")
        .placeholder("Choose priority")
        .open(true)
        .selected("high")
        .placement(OverlayPlacementSide::Right, OverlayPlacementAlignment::End)
        .option(ListboxOption::new("low", "Low"))
        .option(ListboxOption::new("medium", "Medium").disabled(true))
        .group(
            ListboxGroup::new("recommended", "Recommended")
                .option(ListboxOption::new("high", "High"))
                .option(ListboxOption::new("urgent", "Urgent"))
                .option(ListboxOption::new("normal", "Normal"))
                .option(ListboxOption::new("later", "Later"))
                .option(ListboxOption::new("someday", "Someday")),
        )
        .state();

    assert!(state.open());
    assert_eq!(state.open_mode(), SelectOpenMode::Controlled);
    assert_eq!(state.trigger_role(), Role::Button);
    assert_eq!(state.content_role(), Role::ListBox);
    assert!(state.trigger_selected());
    assert_eq!(state.trigger_label(), "High");
    assert_eq!(state.selected_value(), Some("high"));
    assert_eq!(state.active_value(), Some("high"));
    assert_eq!(state.placement_side(), OverlayPlacementSide::Right);
    assert_eq!(state.placement_alignment(), OverlayPlacementAlignment::End);
    assert_eq!(
        state.overlay().policy().kind(),
        OverlayLayerKind::NonModalDismissible
    );
    assert_eq!(
        state.outside_press_policy(),
        OutsidePressPolicy::DismissAndConsume
    );
    assert_eq!(
        state.initial_focus_intent(),
        &InitialFocusIntent::FirstFocusable
    );
    assert_eq!(state.focus_restore_intent(), &FocusRestoreIntent::Trigger);
    assert_eq!(state.listbox().role(), Role::ListBox);
    assert_eq!(state.listbox().selected_value(), Some("high"));
    assert!(state.scrollable_content());
    assert!(state.scroll_area().scrolls_y());
}

#[test]
fn select_state_models_disabled_empty_and_policy_overrides() {
    let state = Select::new("empty-select", "Empty")
        .placeholder("Nothing to choose")
        .default_open(true)
        .disabled(true)
        .outside_press_policy(OutsidePressPolicy::DismissAndPassThrough)
        .initial_focus_intent(InitialFocusIntent::None)
        .focus_restore_intent(FocusRestoreIntent::None)
        .small()
        .state();

    assert_eq!(state.open_mode(), SelectOpenMode::Uncontrolled);
    assert!(state.default_open());
    assert!(state.disabled());
    assert!(!state.open());
    assert_eq!(state.trigger_label(), "Nothing to choose");
    assert_eq!(state.selected_value(), None);
    assert_eq!(state.active_value(), None);
    assert!(!state.scrollable_content());
    assert_eq!(
        state.outside_press_policy(),
        OutsidePressPolicy::DismissAndPassThrough
    );
    assert_eq!(state.initial_focus_intent(), &InitialFocusIntent::None);
    assert_eq!(state.focus_restore_intent(), &FocusRestoreIntent::None);
    assert!(!state.overlay().should_render_deferred_layer());
}

#[open_gpui::test]
fn select_runtime_click_and_keyboard_selection_close_popup_and_emit_payloads(
    cx: &mut open_gpui::TestAppContext,
) {
    #[derive(Clone, Debug, PartialEq, Eq)]
    enum SelectRuntimeEvent {
        Open(bool),
        Select(SelectSelection),
    }

    struct TestView {
        events: Rc<RefCell<Vec<SelectRuntimeEvent>>>,
    }

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let open_events = self.events.clone();
            let select_events = self.events.clone();

            div().size_full().child(
                Select::new("runtime-select", "Runtime select")
                    .placeholder("Choose item")
                    .option(ListboxOption::new("alpha", "Alpha"))
                    .option(ListboxOption::new("bravo", "Bravo").disabled(true))
                    .group(
                        ListboxGroup::new("team", "Team")
                            .option(ListboxOption::new("charlie", "Charlie"))
                            .option(ListboxOption::new("delta", "Delta")),
                    )
                    .on_open_change(move |open, _, _| {
                        open_events
                            .borrow_mut()
                            .push(SelectRuntimeEvent::Open(open));
                    })
                    .on_select(move |selection, _, _| {
                        select_events
                            .borrow_mut()
                            .push(SelectRuntimeEvent::Select(selection));
                    }),
            )
        }
    }

    let events = Rc::new(RefCell::new(Vec::new()));
    let (_, cx) = cx.add_window_view(|_, _| TestView {
        events: events.clone(),
    });
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    assert!(
        cx.debug_bounds("select:runtime-select:root").is_some(),
        "select root should expose a stable debug selector"
    );

    let trigger = cx
        .debug_bounds("select:runtime-select:trigger")
        .expect("select trigger should be rendered");
    cx.simulate_click(trigger.center(), Default::default());
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });
    assert_eq!(
        events.borrow().clone(),
        vec![SelectRuntimeEvent::Open(true)]
    );
    assert!(
        cx.debug_bounds("select:Runtime select:select-content-scroll:content")
            .is_some(),
        "select content should open from the real trigger"
    );

    let disabled_bravo = cx
        .debug_bounds("listbox:runtime-select-listbox:option:bravo")
        .expect("disabled Bravo option should be rendered in the popup listbox");
    cx.simulate_click(disabled_bravo.center(), Default::default());
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });
    assert_eq!(
        events.borrow().clone(),
        vec![SelectRuntimeEvent::Open(true)],
        "disabled popup option click should not emit selection callbacks or close the popup"
    );

    let alpha = cx
        .debug_bounds("listbox:runtime-select-listbox:option:alpha")
        .expect("Alpha option should be rendered in the popup listbox");
    cx.simulate_click(alpha.center(), Default::default());
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });
    assert_eq!(
        events.borrow().clone(),
        vec![
            SelectRuntimeEvent::Open(true),
            SelectRuntimeEvent::Select(SelectSelection::new(0, "alpha", "Alpha")),
            SelectRuntimeEvent::Open(false),
        ]
    );
    assert!(
        cx.debug_bounds("select:Runtime select:select-content-scroll:content")
            .is_none(),
        "enabled popup option click should close the content"
    );

    let trigger = cx
        .debug_bounds("select:runtime-select:trigger")
        .expect("select trigger should still be rendered after selection");
    cx.simulate_click(trigger.center(), Default::default());
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });
    assert_eq!(
        events.borrow().clone(),
        vec![
            SelectRuntimeEvent::Open(true),
            SelectRuntimeEvent::Select(SelectSelection::new(0, "alpha", "Alpha")),
            SelectRuntimeEvent::Open(false),
            SelectRuntimeEvent::Open(true),
        ]
    );
    assert!(
        cx.debug_bounds("select:Runtime select:select-content-scroll:content")
            .is_some(),
        "select content should reopen from the trigger after a prior selection"
    );

    let alpha = cx
        .debug_bounds("listbox:runtime-select-listbox:option:alpha")
        .expect("Alpha option should be rendered after reopening");
    cx.simulate_mouse_down(alpha.center(), MouseButton::Left, Default::default());
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });
    assert_eq!(
        events.borrow().clone(),
        vec![
            SelectRuntimeEvent::Open(true),
            SelectRuntimeEvent::Select(SelectSelection::new(0, "alpha", "Alpha")),
            SelectRuntimeEvent::Open(false),
            SelectRuntimeEvent::Open(true),
        ],
        "mouse down should focus the option without selecting until mouse up or keyboard activation"
    );
    assert!(
        cx.debug_bounds("select:Runtime select:select-content-scroll:content")
            .is_some(),
        "mouse down focus should keep the popup open for keyboard activation"
    );

    cx.simulate_keystrokes("down");
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });
    assert_eq!(
        events.borrow().clone(),
        vec![
            SelectRuntimeEvent::Open(true),
            SelectRuntimeEvent::Select(SelectSelection::new(0, "alpha", "Alpha")),
            SelectRuntimeEvent::Open(false),
            SelectRuntimeEvent::Open(true),
        ],
        "arrow navigation in the popup listbox should not select"
    );

    cx.simulate_keystrokes("enter");
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });
    assert_eq!(
        events.borrow().clone(),
        vec![
            SelectRuntimeEvent::Open(true),
            SelectRuntimeEvent::Select(SelectSelection::new(0, "alpha", "Alpha")),
            SelectRuntimeEvent::Open(false),
            SelectRuntimeEvent::Open(true),
            SelectRuntimeEvent::Select(SelectSelection::new(2, "charlie", "Charlie")),
            SelectRuntimeEvent::Open(false),
        ]
    );
    assert!(
        cx.debug_bounds("select:Runtime select:select-content-scroll:content")
            .is_none(),
        "keyboard popup selection should close the content"
    );
}

#[test]
fn combobox_state_filters_query_without_clearing_selection() {
    let state = Combobox::new("framework-combobox", "Framework")
        .placeholder("Search frameworks")
        .open(true)
        .default_query("re")
        .selected("solid")
        .option(ComboboxOption::new("react", "React").keyword("library"))
        .option(ComboboxOption::new("solid", "Solid"))
        .option(ComboboxOption::new("ember", "Ember").disabled(true))
        .group(
            ComboboxGroup::new("meta", "Meta")
                .option(ComboboxOption::new("remix", "Remix").keyword("react"))
                .option(ComboboxOption::new("relay", "Relay").keyword("graphql")),
        )
        .state();

    assert!(state.open());
    assert_eq!(state.open_mode(), ComboboxOpenMode::Controlled);
    assert_eq!(state.input_role(), Role::EditableComboBox);
    assert_eq!(state.content_role(), Role::ListBox);
    assert_eq!(state.query(), "re");
    assert_eq!(state.total_option_count(), 5);
    assert_eq!(state.filtered_option_count(), 3);
    assert!(state.filtered());
    assert_eq!(state.selected_value(), Some("solid"));
    assert_eq!(state.active_value(), Some("react"));
    assert_eq!(state.listbox().role(), Role::ListBox);
    assert_eq!(state.listbox().selected_value(), None);
    assert_eq!(state.listbox().typeahead_query(), Some("re"));
    assert_eq!(
        state.listbox().options()[0].role(),
        Some(Role::ListBoxOption)
    );
    assert_eq!(
        state.overlay().policy().kind(),
        OverlayLayerKind::NonModalDismissible
    );
    assert_eq!(state.input().placeholder(), Some("Search frameworks"));
}

#[test]
fn combobox_state_normalizes_query_with_text_input_policy() {
    let state = Combobox::new("newline-combobox", "Framework")
        .default_query("re\r\nmix")
        .option(ComboboxOption::new("remix", "Remix"))
        .state();

    assert_eq!(state.query(), "re  mix");
    assert_eq!(state.input().value(), "re  mix");
}

#[test]
fn combobox_state_scrollable_content_tracks_filtered_option_count() {
    let scrollable = Combobox::new("scrolling-combobox", "Scrolling combobox")
        .placeholder("Search frameworks")
        .open(true)
        .option(ComboboxOption::new("react", "React").keyword("library"))
        .option(ComboboxOption::new("solid", "Solid"))
        .option(ComboboxOption::new("ember", "Ember"))
        .option(ComboboxOption::new("svelte", "Svelte"))
        .option(ComboboxOption::new("angular", "Angular"))
        .option(ComboboxOption::new("vue", "Vue"))
        .group(
            ComboboxGroup::new("meta", "Meta")
                .option(ComboboxOption::new("remix", "Remix").keyword("react")),
        )
        .state();
    let not_scrollable = Combobox::new("filtered-combobox", "Filtered combobox")
        .placeholder("Search frameworks")
        .open(true)
        .default_query("re")
        .option(ComboboxOption::new("react", "React").keyword("library"))
        .option(ComboboxOption::new("solid", "Solid"))
        .option(ComboboxOption::new("ember", "Ember"))
        .group(
            ComboboxGroup::new("meta", "Meta")
                .option(ComboboxOption::new("remix", "Remix").keyword("react"))
                .option(ComboboxOption::new("relay", "Relay").keyword("graphql")),
        )
        .state();

    assert_eq!(scrollable.total_option_count(), 7);
    assert_eq!(scrollable.filtered_option_count(), 7);
    assert!(scrollable.scrollable_content());

    assert_eq!(not_scrollable.total_option_count(), 5);
    assert_eq!(not_scrollable.filtered_option_count(), 3);
    assert!(!not_scrollable.scrollable_content());
}

#[test]
fn combobox_disabled_empty_state_blocks_popup_and_input() {
    let state = Combobox::new("empty-combobox", "Empty")
        .placeholder("Search")
        .default_open(true)
        .disabled(true)
        .default_query("zzz")
        .option(ComboboxOption::new("react", "React"))
        .empty_label("No frameworks")
        .outside_press_policy(OutsidePressPolicy::DismissAndPassThrough)
        .state();

    assert_eq!(state.open_mode(), ComboboxOpenMode::Uncontrolled);
    assert!(state.default_open());
    assert!(state.disabled());
    assert!(!state.open());
    assert_eq!(state.filtered_option_count(), 0);
    assert!(state.listbox().empty());
    assert_eq!(state.listbox().empty_label(), "No frameworks");
    assert!(!state.input().editable());
    assert_eq!(
        state.outside_press_policy(),
        OutsidePressPolicy::DismissAndPassThrough
    );
    assert!(!state.overlay().should_render_deferred_layer());
}

#[open_gpui::test]
fn combobox_runtime_filters_input_and_selects_filtered_option(cx: &mut open_gpui::TestAppContext) {
    #[derive(Clone, Debug, PartialEq, Eq)]
    enum ComboboxRuntimeEvent {
        Open(bool),
        Select(ComboboxSelection),
    }

    struct TestView {
        events: Rc<RefCell<Vec<ComboboxRuntimeEvent>>>,
    }

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let open_events = self.events.clone();
            let select_events = self.events.clone();

            div().size_full().child(
                Combobox::new("runtime-combobox", "Runtime combobox")
                    .placeholder("Search frameworks")
                    .option(ComboboxOption::new("react", "React").keyword("library"))
                    .option(ComboboxOption::new("solid", "Solid"))
                    .option(ComboboxOption::new("ember", "Ember").disabled(true))
                    .group(
                        ComboboxGroup::new("meta", "Meta")
                            .option(ComboboxOption::new("remix", "Remix").keyword("react"))
                            .option(ComboboxOption::new("relay", "Relay").keyword("graphql")),
                    )
                    .on_open_change(move |open, _, _| {
                        open_events
                            .borrow_mut()
                            .push(ComboboxRuntimeEvent::Open(open));
                    })
                    .on_select(move |selection, _, _| {
                        select_events
                            .borrow_mut()
                            .push(ComboboxRuntimeEvent::Select(selection));
                    }),
            )
        }
    }

    cx.update(init_text_input);
    let events = Rc::new(RefCell::new(Vec::new()));
    let (_, cx) = cx.add_window_view(|_, _| TestView {
        events: events.clone(),
    });
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    let input = cx
        .debug_bounds("text-input:runtime-combobox-input:root")
        .expect("combobox text input should expose a stable debug selector");
    cx.simulate_click(input.center(), Default::default());
    cx.simulate_input("re");
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    assert!(
        cx.debug_bounds("combobox:runtime-combobox:content")
            .is_none(),
        "typing text should filter input without implicitly opening the popup"
    );

    let toggle = cx
        .debug_bounds("combobox:runtime-combobox:toggle")
        .expect("combobox toggle should be rendered");
    cx.simulate_click(toggle.center(), Default::default());
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    assert_eq!(
        events.borrow().clone(),
        vec![ComboboxRuntimeEvent::Open(true)]
    );
    assert!(
        cx.debug_bounds("combobox:runtime-combobox:content")
            .is_some(),
        "toggle click should open filtered popup content"
    );
    assert!(
        cx.debug_bounds("listbox:runtime-combobox-listbox:option:react")
            .is_some(),
        "React should match query text"
    );
    assert!(
        cx.debug_bounds("listbox:runtime-combobox-listbox:option:remix")
            .is_some(),
        "Remix should match query keyword"
    );
    assert!(
        cx.debug_bounds("listbox:runtime-combobox-listbox:option:solid")
            .is_none(),
        "Solid should be filtered out by query text"
    );
    assert!(
        cx.debug_bounds("listbox:runtime-combobox-listbox:option:ember")
            .is_none(),
        "disabled Ember should still be filtered out when it does not match"
    );

    let remix = cx
        .debug_bounds("listbox:runtime-combobox-listbox:option:remix")
        .expect("filtered Remix option should be rendered");
    cx.simulate_click(remix.center(), Default::default());
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    assert_eq!(
        events.borrow().clone(),
        vec![
            ComboboxRuntimeEvent::Open(true),
            ComboboxRuntimeEvent::Select(ComboboxSelection::new("remix", "Remix")),
            ComboboxRuntimeEvent::Open(false),
        ]
    );
    assert!(
        cx.debug_bounds("combobox:runtime-combobox:content")
            .is_none(),
        "combobox selection should close popup content"
    );
}

#[open_gpui::test]
fn combobox_runtime_keyboard_selects_filtered_option(cx: &mut open_gpui::TestAppContext) {
    #[derive(Clone, Debug, PartialEq, Eq)]
    enum ComboboxRuntimeEvent {
        Open(bool),
        Select(ComboboxSelection),
    }

    struct TestView {
        events: Rc<RefCell<Vec<ComboboxRuntimeEvent>>>,
    }

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let open_events = self.events.clone();
            let select_events = self.events.clone();

            div().size_full().child(
                Combobox::new("keyboard-combobox", "Keyboard combobox")
                    .placeholder("Search frameworks")
                    .option(ComboboxOption::new("react", "React").keyword("library"))
                    .option(ComboboxOption::new("solid", "Solid"))
                    .group(
                        ComboboxGroup::new("meta", "Meta")
                            .option(ComboboxOption::new("remix", "Remix").keyword("react"))
                            .option(ComboboxOption::new("relay", "Relay").keyword("graphql")),
                    )
                    .on_open_change(move |open, _, _| {
                        open_events
                            .borrow_mut()
                            .push(ComboboxRuntimeEvent::Open(open));
                    })
                    .on_select(move |selection, _, _| {
                        select_events
                            .borrow_mut()
                            .push(ComboboxRuntimeEvent::Select(selection));
                    }),
            )
        }
    }

    cx.update(init_text_input);
    let events = Rc::new(RefCell::new(Vec::new()));
    let (_, cx) = cx.add_window_view(|_, _| TestView {
        events: events.clone(),
    });
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    let input = cx
        .debug_bounds("text-input:keyboard-combobox-input:root")
        .expect("combobox text input should expose a stable debug selector");
    cx.simulate_click(input.center(), Default::default());
    cx.simulate_input("re");
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    cx.simulate_keystrokes("down");
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    assert_eq!(
        events.borrow().clone(),
        vec![ComboboxRuntimeEvent::Open(true)]
    );
    assert!(
        cx.debug_bounds("combobox:keyboard-combobox:content")
            .is_some(),
        "down arrow should open filtered combobox content from the input row"
    );

    cx.simulate_keystrokes("enter");
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    assert_eq!(
        events.borrow().clone(),
        vec![
            ComboboxRuntimeEvent::Open(true),
            ComboboxRuntimeEvent::Select(ComboboxSelection::new("remix", "Remix")),
            ComboboxRuntimeEvent::Open(false),
        ]
    );
    assert!(
        cx.debug_bounds("combobox:keyboard-combobox:content")
            .is_none(),
        "keyboard selection should close filtered combobox content"
    );
}

#[test]
fn command_state_filters_groups_shortcuts_loading_and_dialog_policy() {
    let state = Command::new("command-palette", "Command palette")
        .placeholder("Type a command")
        .open(true)
        .default_query("file")
        .selected("new-file")
        .loading("Indexing commands", Some(45))
        .dialog("Command palette")
        .dialog_description("Run a workspace command")
        .item(CommandItem::new("open-file", "Open File").shortcut("Ctrl+O"))
        .group(
            CommandGroup::new("file", "File")
                .item(CommandItem::new("new-file", "New File").shortcut("Ctrl+N"))
                .item(CommandItem::new("close-window", "Close Window").shortcut("Alt+F4")),
        )
        .group(
            CommandGroup::new("view", "View")
                .item(CommandItem::new("toggle-sidebar", "Toggle Sidebar").keyword("layout")),
        )
        .state();

    assert!(state.open());
    assert_eq!(state.open_mode(), CommandOpenMode::Controlled);
    assert_eq!(state.input_role(), Role::TextInput);
    assert_eq!(state.list_role(), Role::ListBox);
    assert_eq!(state.query(), "file");
    assert_eq!(state.total_item_count(), 4);
    assert_eq!(state.filtered_item_count(), 2);
    assert!(state.filtered());
    assert_eq!(state.selected_value(), Some("new-file"));
    assert_eq!(state.active_value(), Some("new-file"));
    assert_eq!(state.groups().len(), 2);
    assert_eq!(state.groups()[0].label(), "Commands");
    assert_eq!(state.groups()[1].label(), "File");
    assert!(state.groups()[0].match_score() > 0);
    assert!(state.groups()[1].match_score() > 0);
    assert_eq!(state.items().len(), 2);
    assert_eq!(state.items()[1].shortcut(), Some("Ctrl+N"));
    assert!(state.items()[1].selected());
    let activation = state.activation_for_key("enter").unwrap();
    assert_eq!(activation.value(), "new-file");
    assert_eq!(activation.shortcut(), Some("Ctrl+N"));
    assert!(state.loading().is_some());
    assert_eq!(state.loading().unwrap().role(), Role::ProgressIndicator);
    assert_eq!(state.loading().unwrap().progress_percent(), Some(45));
    assert!(state.scroll_area().scrolls_y());
    assert_eq!(
        state.scroll_area().reset_policy(),
        ScrollResetPolicy::Preserve
    );
    assert_eq!(
        state.overlay().policy().kind(),
        OverlayLayerKind::NonModalDismissible
    );
    let dialog = state.dialog().unwrap();
    assert!(dialog.open());
    assert_eq!(dialog.content_role(), Role::Window);
    assert_eq!(dialog.overlay().policy().kind(), OverlayLayerKind::Modal);
    assert_eq!(dialog.description(), Some("Run a workspace command"));
}

#[test]
fn command_state_reports_match_sources_for_label_value_keyword_and_shortcut() {
    let label_state = Command::new("label-command", "Commands")
        .default_query("open")
        .item(CommandItem::new("open-file", "Open File"))
        .state();
    assert_eq!(label_state.items()[0].value(), "open-file");
    assert_eq!(
        label_state.items()[0].match_source(),
        Some(CommandMatchSource::Label)
    );

    let value_state = Command::new("value-command", "Commands")
        .default_query("open-file")
        .item(CommandItem::new("open-file", "Open File"))
        .state();
    assert_eq!(
        value_state.items()[0].match_source(),
        Some(CommandMatchSource::Value)
    );

    let keyword_state = Command::new("keyword-command", "Commands")
        .default_query("prefs")
        .item(CommandItem::new("settings", "Settings").keyword("prefs"))
        .state();
    assert_eq!(keyword_state.items()[0].value(), "settings");
    assert_eq!(
        keyword_state.items()[0].match_source(),
        Some(CommandMatchSource::Keyword)
    );

    let shortcut_state = Command::new("shortcut-command", "Commands")
        .default_query("ctrl+p")
        .item(CommandItem::new("palette", "Command Palette").shortcut("Ctrl+P"))
        .state();
    assert_eq!(shortcut_state.items()[0].value(), "palette");
    assert_eq!(
        shortcut_state.items()[0].match_source(),
        Some(CommandMatchSource::Shortcut)
    );
}

#[test]
fn command_state_empty_query_preserves_caller_order() {
    let state = Command::new("ordered-command", "Commands")
        .item(CommandItem::new("root-two", "Root Two"))
        .item(CommandItem::new("root-one", "Root One"))
        .group(
            CommandGroup::new("group", "Group")
                .item(CommandItem::new("group-two", "Group Two"))
                .item(CommandItem::new("group-one", "Group One")),
        )
        .state();
    let values = state
        .items()
        .iter()
        .map(|item| item.value().to_owned())
        .collect::<Vec<_>>();

    assert_eq!(
        values,
        vec![
            "root-two".to_string(),
            "root-one".to_string(),
            "group-two".to_string(),
            "group-one".to_string(),
        ]
    );
    assert!(
        state
            .items()
            .iter()
            .all(|item| item.match_source().is_none() && item.match_score() == 0)
    );
    assert!(state.groups().iter().all(|group| group.match_score() == 0));
}

#[test]
fn command_state_ranks_label_and_value_matches_before_keyword_only_matches() {
    let state = Command::new("ranked-command", "Commands")
        .default_query("file")
        .item(CommandItem::new("archive", "Archive").keyword("file"))
        .item(CommandItem::new("open-file", "Open File"))
        .item(CommandItem::new("file-action", "Launcher"))
        .item(CommandItem::new("bulk-action", "Bulk Action").keyword("file"))
        .state();
    let values = state
        .items()
        .iter()
        .map(|item| item.value().to_owned())
        .collect::<Vec<_>>();

    assert_eq!(
        values,
        vec![
            "open-file".to_string(),
            "file-action".to_string(),
            "archive".to_string(),
            "bulk-action".to_string(),
        ]
    );
    assert_eq!(
        state.items()[0].match_source(),
        Some(CommandMatchSource::Label)
    );
    assert_eq!(
        state.items()[1].match_source(),
        Some(CommandMatchSource::Value)
    );
    assert_eq!(
        state.items()[2].match_source(),
        Some(CommandMatchSource::Keyword)
    );
    assert!(state.items()[1].match_score() > state.items()[2].match_score());
}

#[test]
fn command_state_tracks_active_and_selected_by_value_after_reorder() {
    let first = Command::new("first-command", "Commands")
        .selected("target")
        .active("target")
        .item(CommandItem::new("other", "Other"))
        .item(CommandItem::new("target", "Target"))
        .state();
    let reordered = Command::new("reordered-command", "Commands")
        .selected("target")
        .active("target")
        .item(CommandItem::new("target", "Target"))
        .item(CommandItem::new("other", "Other"))
        .state();

    assert_eq!(first.selected_value(), Some("target"));
    assert_eq!(first.active_value(), Some("target"));
    assert!(first.items()[1].selected());
    assert!(first.items()[1].active());
    assert_eq!(reordered.selected_value(), Some("target"));
    assert_eq!(reordered.active_value(), Some("target"));
    assert!(reordered.items()[0].selected());
    assert!(reordered.items()[0].active());
}

#[test]
fn command_state_keeps_disabled_matches_visible_but_non_activatable() {
    let state = Command::new("disabled-command", "Commands")
        .default_query("delete")
        .selected("delete-project")
        .active("delete-project")
        .item(CommandItem::new("delete-project", "Delete Project").disabled(true))
        .state();

    assert_eq!(state.filtered_item_count(), 1);
    assert_eq!(state.items()[0].value(), "delete-project");
    assert!(state.items()[0].disabled());
    assert_eq!(state.selected_value(), None);
    assert_eq!(state.active_value(), None);
    assert_eq!(state.activation_for_key("enter"), None);
}

#[test]
fn command_state_models_controlled_and_default_query_ownership() {
    let controlled = Command::new("controlled-query-command", "Commands")
        .query("open\r\n")
        .default_query("ignored")
        .item(CommandItem::new("open-file", "Open File"))
        .state();
    let seeded = Command::new("seeded-query-command", "Commands")
        .default_query("new\n")
        .item(CommandItem::new("new-file", "New File"))
        .item(CommandItem::new("open-file", "Open File"))
        .state();

    assert_eq!(controlled.query(), "open  ");
    assert_eq!(controlled.input().value(), "open  ");
    assert_eq!(controlled.query_mode(), CommandQueryMode::Controlled);
    assert_eq!(controlled.filtered_item_count(), 1);
    assert_eq!(seeded.query(), "new ");
    assert_eq!(seeded.input().value(), "new ");
    assert_eq!(seeded.query_mode(), CommandQueryMode::Uncontrolled);
}

#[test]
fn command_state_models_multi_selected_values_and_hidden_chips() {
    let state = Command::new("multi-command", "Commands")
        .default_query("new")
        .multi_select(true)
        .selected_values(["open-file", "new-file", "missing", "delete-file"])
        .item(CommandItem::new("open-file", "Open File"))
        .item(CommandItem::new("delete-file", "Delete File").disabled(true))
        .group(CommandGroup::new("file", "File").item(CommandItem::new("new-file", "New File")))
        .state();

    assert_eq!(state.selection_mode(), CommandSelectionMode::Multiple);
    assert_eq!(
        state.selected_values(),
        &["open-file".to_string(), "new-file".to_string()]
    );
    assert_eq!(state.selected_value(), None);
    assert_eq!(state.filtered_item_count(), 1);
    assert_eq!(
        state
            .selected_chips()
            .iter()
            .map(|chip| chip.value().to_owned())
            .collect::<Vec<_>>(),
        vec!["open-file".to_string(), "new-file".to_string()]
    );
    assert_eq!(state.selected_chips()[0].label(), "Open File");
    assert!(state.items()[0].selected());
}

#[test]
fn command_index_snapshot_matches_equivalent_local_descriptors() {
    let snapshot = CommandIndexSnapshot::new("commands-v1")
        .item(CommandItemDescriptor::new("open-file", "Open File").shortcut("Ctrl+O"))
        .group(
            CommandGroupDescriptor::new("file", "File")
                .item(CommandItemDescriptor::new("new-file", "New File").shortcut("Ctrl+N"))
                .item(
                    CommandItemDescriptor::new("close-window", "Close Window").shortcut("Alt+F4"),
                ),
        );
    let local = Command::new("local-command", "Commands")
        .default_query("file")
        .selected("new-file")
        .active("new-file")
        .item(CommandItem::new("open-file", "Open File").shortcut("Ctrl+O"))
        .group(
            CommandGroup::new("file", "File")
                .item(CommandItem::new("new-file", "New File").shortcut("Ctrl+N"))
                .item(CommandItem::new("close-window", "Close Window").shortcut("Alt+F4")),
        )
        .state();
    let indexed = Command::new("indexed-command", "Commands")
        .default_query("file")
        .selected("new-file")
        .active("new-file")
        .index_snapshot(snapshot)
        .state();

    assert_eq!(indexed.index_revision(), Some("commands-v1"));
    assert_eq!(indexed.index_mode(), CommandIndexSnapshotMode::LocalRanked);
    assert_eq!(indexed.total_item_count(), local.total_item_count());
    assert_eq!(indexed.filtered_item_count(), local.filtered_item_count());
    assert_eq!(
        indexed
            .items()
            .iter()
            .map(|item| (
                item.value().to_owned(),
                item.label().to_owned(),
                item.match_source(),
                item.match_score(),
                item.selected(),
                item.active(),
            ))
            .collect::<Vec<_>>(),
        local
            .items()
            .iter()
            .map(|item| (
                item.value().to_owned(),
                item.label().to_owned(),
                item.match_source(),
                item.match_score(),
                item.selected(),
                item.active(),
            ))
            .collect::<Vec<_>>()
    );
}

#[test]
fn command_items_project_core_command_descriptors() {
    let descriptor = open_gpui_command::CommandDescriptor::new("workspace.open", "Open Workspace")
        .group("Workspace")
        .keyword("project")
        .shortcut("Ctrl+Shift+O")
        .disabled_reason("Workspace is read-only")
        .when("workspace")
        .menu_path(["File", "Open"]);
    let item = CommandItemDescriptor::from_command_descriptor(&descriptor);

    assert_eq!(descriptor.group_ref(), Some("Workspace"));
    assert_eq!(descriptor.menu_path_ref(), ["File", "Open"]);
    assert_eq!(item.value(), "workspace.open");
    assert_eq!(item.label(), "Open Workspace");
    assert_eq!(item.keywords_ref(), ["project"]);
    assert_eq!(item.shortcut_ref(), Some("Ctrl+Shift+O"));
    assert_eq!(item.when_ref(), Some("workspace"));
    assert!(item.disabled_state());
    assert_eq!(item.disabled_reason_ref(), Some("Workspace is read-only"));

    let state = Command::new("descriptor-command", "Commands")
        .item(CommandItem::from_command_descriptor(&descriptor))
        .item(CommandItem::new("workspace.save", "Save Workspace").disabled_reason("No workspace"))
        .state();
    assert_eq!(state.items()[0].value(), "workspace.open");
    assert_eq!(state.items()[0].shortcut(), Some("Ctrl+Shift+O"));
    assert_eq!(state.items()[0].when_ref(), Some("workspace"));
    assert!(state.items()[0].disabled());
    assert_eq!(
        state.items()[0].disabled_reason_ref(),
        Some("Workspace is read-only")
    );
    assert!(state.items()[1].disabled());
    assert_eq!(state.items()[1].disabled_reason_ref(), Some("No workspace"));

    let indexed = Command::new("descriptor-index", "Commands")
        .index_snapshot(CommandIndexSnapshot::new("commands-v1").command_descriptor(&descriptor))
        .state();
    let indexed_group = indexed
        .grouped_groups()
        .next()
        .expect("core command group metadata should project into command index groups");
    assert_eq!(indexed_group.label(), "Workspace");
    assert_eq!(
        indexed
            .group_items(indexed_group.index())
            .next()
            .map(|item| {
                (
                    item.value().to_owned(),
                    item.shortcut().map(str::to_owned),
                    item.when_ref().map(str::to_owned),
                    item.disabled_reason_ref().map(str::to_owned),
                )
            }),
        Some((
            "workspace.open".to_owned(),
            Some("Ctrl+Shift+O".to_owned()),
            Some("workspace".to_owned()),
            Some("Workspace is read-only".to_owned()),
        ))
    );
}

#[test]
fn command_index_snapshot_projects_core_command_registry_snapshot() {
    let mut registry = open_gpui_command::CommandRegistry::new("registry-v1");
    registry
        .register_contribution(
            open_gpui_command::CommandContribution::new(
                open_gpui_command::CommandDescriptor::new("workspace.open", "Open Workspace")
                    .group("Workspace")
                    .keyword("project")
                    .shortcut("Ctrl+Shift+O")
                    .when("workspace"),
            )
            .source("workspace"),
        )
        .unwrap();
    registry
        .register(
            open_gpui_command::CommandDescriptor::new("file.save", "Save File")
                .group("File")
                .disabled_reason("Read-only file"),
        )
        .unwrap();

    let snapshot = CommandIndexSnapshot::from_registry_snapshot(&registry.snapshot());
    let state = Command::new("registry-index-command", "Commands")
        .index_snapshot(snapshot)
        .state();

    assert_eq!(state.index_revision(), Some("registry-v1"));
    assert_eq!(
        state
            .groups()
            .iter()
            .map(|group| group.label().to_owned())
            .collect::<Vec<_>>(),
        ["Workspace", "File"]
    );
    assert_eq!(
        state
            .items()
            .iter()
            .map(|item| {
                (
                    item.value().to_owned(),
                    item.shortcut().map(str::to_owned),
                    item.when_ref().map(str::to_owned),
                    item.disabled(),
                    item.disabled_reason_ref().map(str::to_owned),
                )
            })
            .collect::<Vec<_>>(),
        [
            (
                "workspace.open".to_owned(),
                Some("Ctrl+Shift+O".to_owned()),
                Some("workspace".to_owned()),
                false,
                None,
            ),
            (
                "file.save".to_owned(),
                None,
                None,
                true,
                Some("Read-only file".to_owned()),
            ),
        ]
    );
}

#[test]
fn command_index_snapshot_revision_preserves_selection_by_value_after_reorder() {
    let first = CommandIndexSnapshot::new("commands-v1")
        .item(CommandItemDescriptor::new("other", "Other"))
        .item(CommandItemDescriptor::new("target", "Target"));
    let second = CommandIndexSnapshot::new("commands-v2")
        .item(CommandItemDescriptor::new("target", "Target"))
        .item(CommandItemDescriptor::new("other", "Other"));
    let first_state = Command::new("snapshot-revision-command", "Commands")
        .selected("target")
        .active("target")
        .index_snapshot(first)
        .state();
    let second_state = Command::new("snapshot-revision-command", "Commands")
        .selected("target")
        .active("target")
        .index_snapshot(second)
        .state();

    assert_eq!(first_state.index_revision(), Some("commands-v1"));
    assert_eq!(second_state.index_revision(), Some("commands-v2"));
    assert_eq!(first_state.items()[1].value(), "target");
    assert!(first_state.items()[1].selected());
    assert!(first_state.items()[1].active());
    assert_eq!(second_state.items()[0].value(), "target");
    assert!(second_state.items()[0].selected());
    assert!(second_state.items()[0].active());
}

#[test]
fn command_index_snapshot_modes_preserve_pre_ranked_and_pre_filtered_order() {
    let pre_ranked = CommandIndexSnapshot::new("pre-ranked")
        .mode(CommandIndexSnapshotMode::PreRankedFilter)
        .item(CommandItemDescriptor::new("archive", "Archive").keyword("file"))
        .item(CommandItemDescriptor::new("open-file", "Open File"))
        .item(CommandItemDescriptor::new("file-action", "Launcher"))
        .item(CommandItemDescriptor::new("bulk-action", "Bulk Action").keyword("file"));
    let pre_filtered = CommandIndexSnapshot::new("pre-filtered")
        .mode(CommandIndexSnapshotMode::PreFiltered)
        .item(CommandItemDescriptor::new("archive", "Archive").keyword("file"))
        .item(CommandItemDescriptor::new("unmatched", "Unmatched"));

    let pre_ranked_state = Command::new("pre-ranked-command", "Commands")
        .query("file")
        .index_snapshot(pre_ranked)
        .state();
    let pre_filtered_state = Command::new("pre-filtered-command", "Commands")
        .query("file")
        .index_snapshot(pre_filtered)
        .state();

    assert_eq!(
        pre_ranked_state
            .items()
            .iter()
            .map(|item| item.value().to_owned())
            .collect::<Vec<_>>(),
        vec![
            "archive".to_string(),
            "open-file".to_string(),
            "file-action".to_string(),
            "bulk-action".to_string(),
        ]
    );
    assert_eq!(
        pre_ranked_state
            .items()
            .iter()
            .map(|item| item.match_source())
            .collect::<Vec<_>>(),
        vec![
            Some(CommandMatchSource::Keyword),
            Some(CommandMatchSource::Label),
            Some(CommandMatchSource::Value),
            Some(CommandMatchSource::Keyword),
        ]
    );
    assert_eq!(pre_filtered_state.filtered_item_count(), 2);
    assert_eq!(
        pre_filtered_state
            .items()
            .iter()
            .map(|item| (
                item.value().to_owned(),
                item.match_source(),
                item.match_score()
            ))
            .collect::<Vec<_>>(),
        vec![
            ("archive".to_string(), None, 0),
            ("unmatched".to_string(), None, 0),
        ]
    );
}

#[test]
fn command_index_snapshot_loading_coexists_with_visible_and_empty_results() {
    let visible = CommandIndexSnapshot::new("loading-visible")
        .mode(CommandIndexSnapshotMode::PreFiltered)
        .loading(CommandLoadingState::new(
            "Refreshing command index",
            Some(30),
        ))
        .item(CommandItemDescriptor::new(
            "stale-open",
            "Open from stale index",
        ));
    let empty = CommandIndexSnapshot::new("loading-empty")
        .loading(CommandLoadingState::new("Indexing commands", None));

    let visible_state = Command::new("snapshot-loading-visible", "Commands")
        .query("anything")
        .loading("Builder loading is overridden", Some(99))
        .index_snapshot(visible)
        .state();
    let empty_state = Command::new("snapshot-loading-empty", "Commands")
        .query("anything")
        .index_snapshot(empty)
        .state();

    assert_eq!(visible_state.filtered_item_count(), 1);
    assert_eq!(
        visible_state.loading().map(CommandLoadingState::message),
        Some("Refreshing command index")
    );
    assert_eq!(
        visible_state
            .loading()
            .and_then(CommandLoadingState::progress_percent),
        Some(30)
    );
    assert!(empty_state.empty());
    assert_eq!(
        empty_state.loading().map(CommandLoadingState::message),
        Some("Indexing commands")
    );
    assert_eq!(empty_state.loading().unwrap().progress_percent(), None);
}

#[test]
fn command_palette_projection_adapts_center_query_shortcuts_providers_and_diagnostics() {
    let mut center = open_gpui_command::CommandCenter::new("palette-center-v1");
    center
        .register_source(
            "workspace",
            "workspace-core",
            [open_gpui_command::CommandContribution::new(
                open_gpui_command::CommandDescriptor::new("workspace.open", "Open Workspace")
                    .group("Workspace")
                    .keyword("project"),
            )],
        )
        .unwrap();
    center.register_provider(
        "recent-provider",
        |request: &open_gpui_command::CommandProviderRequest| {
            open_gpui_command::CommandProviderResponse::ready().source(
                open_gpui_command::CommandProviderSource::new(
                    "workspace",
                    "recent-provider-results",
                    [open_gpui_command::CommandContribution::new(
                        open_gpui_command::CommandDescriptor::new(
                            format!("provider.open.{}", request.query()),
                            format!("Open {}", request.query()),
                        )
                        .group("Provider")
                        .keyword("recent"),
                    )],
                ),
            )
        },
    );
    let mut controller =
        open_gpui_command::CommandProviderRefreshController::new("recent-provider")
            .with_loading_message("Searching provider commands");
    let provider_projection = controller
        .refresh_provider(&mut center, "alpha")
        .expect("provider should be registered")
        .expect("provider response should be valid");
    assert_eq!(provider_projection.query(), "alpha");

    center
        .register_action("workspace.open", OpenPaletteCommand)
        .register_action("provider.open.alpha", RevealPaletteCommand);
    let mut keymap = open_gpui::Keymap::default();
    keymap.add_bindings([
        open_gpui::KeyBinding::new("ctrl-p", OpenPaletteCommand, None),
        open_gpui::KeyBinding::new("ctrl-alt-o", RevealPaletteCommand, None),
    ]);

    let projection = CommandPaletteProjection::from_center_for_keymap(&center, "alpha", &keymap);

    assert_eq!(projection.query(), "alpha");
    assert!(projection.shortcut_diagnostics().is_empty());
    assert_eq!(projection.provider_statuses().len(), 1);
    assert_eq!(
        projection
            .provider_status()
            .map(|status| (status.query(), status.command_count())),
        Some((Some("alpha"), 1))
    );
    assert_eq!(projection.index_snapshot().revision(), "palette-center-v1");
    assert_eq!(
        projection.index_snapshot().snapshot_mode(),
        CommandIndexSnapshotMode::PreFiltered
    );

    let state = Command::new("palette-projected-command", "Projected commands")
        .palette_projection(&projection)
        .selected("provider.open.alpha")
        .active("provider.open.alpha")
        .state();

    assert_eq!(state.query(), "alpha");
    assert_eq!(state.index_revision(), Some("palette-center-v1"));
    assert_eq!(state.index_mode(), CommandIndexSnapshotMode::PreFiltered);
    assert_eq!(state.filtered_item_count(), 1);
    assert_eq!(state.groups()[0].label(), "Provider");
    assert_eq!(
        state
            .group_items(0)
            .map(|item| (item.value().to_owned(), item.shortcut().map(str::to_owned)))
            .collect::<Vec<_>>(),
        vec![(
            "provider.open.alpha".to_string(),
            Some("ctrl-alt-O".to_string())
        )]
    );
}

#[test]
fn command_palette_projection_builds_status_items_from_provider_failures_and_diagnostics() {
    let mut center = open_gpui_command::CommandCenter::new("palette-status-center-v1");
    center
        .register_source(
            "workspace",
            "workspace-core",
            [
                open_gpui_command::CommandContribution::new(
                    open_gpui_command::CommandDescriptor::new("workspace.open", "Open Workspace")
                        .group("Workspace"),
                ),
                open_gpui_command::CommandContribution::new(
                    open_gpui_command::CommandDescriptor::new("workspace.save", "Save Workspace")
                        .group("Workspace"),
                ),
            ],
        )
        .unwrap();
    center.register_action("workspace.open", OpenPaletteCommand);
    let request = center.begin_provider_request("recent-provider", "alpha");
    center
        .apply_provider_response_for_request(
            "recent-provider",
            &request,
            open_gpui_command::CommandProviderResponse::failed("Provider unavailable"),
        )
        .unwrap();

    let projection = CommandPaletteProjection::from_center_for_keymap(
        &center,
        "alpha",
        &open_gpui::Keymap::default(),
    );

    assert_eq!(projection.status_error_count(), 1);
    assert_eq!(projection.status_warning_count(), 2);
    assert!(projection.has_status_items());
    assert_eq!(
        projection.status_items()[0].intent(),
        CommandStatusIntent::Error
    );
    assert!(
        projection.status_items()[0]
            .message()
            .contains("recent-provider")
    );
    assert!(
        projection.status_items()[0]
            .message()
            .contains("Provider unavailable")
    );
    assert!(
        projection
            .status_items()
            .iter()
            .any(|item| item.intent() == CommandStatusIntent::Warning
                && item.message().contains("workspace.open")
                && item.message().contains("shortcut"))
    );
    assert!(
        projection
            .status_items()
            .iter()
            .any(|item| item.intent() == CommandStatusIntent::Warning
                && item.message().contains("workspace.save")
                && item.message().contains("action"))
    );

    let state = Command::new("palette-status-command", "Palette status")
        .palette_projection(&projection)
        .state();

    assert_eq!(state.status_items(), projection.status_items());
    assert_eq!(state.status_error_count(), 1);
    assert_eq!(state.status_warning_count(), 2);
}

#[test]
fn command_state_accepts_explicit_status_items() {
    let state = Command::new("explicit-status-command", "Commands")
        .status_item(CommandStatusItem::warning("Shortcut Ctrl+P is shared"))
        .status_item(CommandStatusItem::info("   "))
        .status_item(CommandStatusItem::info("Two providers returned results"))
        .status_item(CommandStatusItem::error("Provider failed"))
        .item(CommandItem::new("open-file", "Open File"))
        .state();

    assert!(state.has_status_items());
    assert_eq!(state.status_items().len(), 3);
    assert_eq!(state.status_warning_count(), 1);
    assert_eq!(state.status_error_count(), 1);
    assert_eq!(state.status_items()[1].intent(), CommandStatusIntent::Info);
    assert_eq!(state.status_items()[1].role(), Role::Label);
}

#[test]
fn command_palette_controller_refreshes_registered_provider_into_command_projection() {
    let mut center = open_gpui_command::CommandCenter::new("controller-center-v1");
    center.register_provider(
        "recent-provider",
        |request: &open_gpui_command::CommandProviderRequest| {
            open_gpui_command::CommandProviderResponse::ready().source(
                open_gpui_command::CommandProviderSource::new(
                    "workspace",
                    "recent-provider-results",
                    [open_gpui_command::CommandContribution::new(
                        open_gpui_command::CommandDescriptor::new(
                            format!("provider.open.{}", request.query()),
                            format!("Open {}", request.query()),
                        )
                        .group("Provider"),
                    )],
                ),
            )
        },
    );
    center.register_action("provider.open.alpha", RevealPaletteCommand);
    let mut keymap = open_gpui::Keymap::default();
    keymap.add_bindings([open_gpui::KeyBinding::new(
        "ctrl-alt-o",
        RevealPaletteCommand,
        None,
    )]);
    let mut controller = CommandPaletteController::new()
        .provider_with_loading("recent-provider", "Searching provider commands");

    let update = controller
        .set_query_for_keymap(&mut center, "alpha", &keymap)
        .unwrap();

    assert!(update.query_changed());
    assert_eq!(update.query(), "alpha");
    assert!(update.missing_provider_ids().is_empty());
    assert!(update.pending_provider_requests().is_empty());
    assert_eq!(update.provider_projections().len(), 1);
    assert!(
        update
            .provider_projection("recent-provider")
            .is_some_and(|projection| projection
                .outcome()
                .is_some_and(open_gpui_command::CommandProviderApplyOutcome::applied))
    );
    assert_eq!(
        update
            .palette_projection()
            .provider_status()
            .map(|status| (status.query(), status.command_count())),
        Some((Some("alpha"), 1))
    );
    assert!(
        update
            .palette_projection()
            .shortcut_diagnostics()
            .is_empty()
    );

    let state = Command::new("controller-command", "Controller commands")
        .palette_projection(update.palette_projection())
        .selected("provider.open.alpha")
        .active("provider.open.alpha")
        .state();

    assert_eq!(state.query(), "alpha");
    assert_eq!(state.index_revision(), Some("controller-center-v1"));
    assert_eq!(state.index_mode(), CommandIndexSnapshotMode::PreFiltered);
    assert_eq!(state.filtered_item_count(), 1);
    assert_eq!(
        state.group_items(0).next().and_then(|item| item.shortcut()),
        Some("ctrl-alt-O")
    );
}

#[test]
fn command_palette_controller_preflights_keymap_dispatch_with_query() {
    let mut center = open_gpui_command::CommandCenter::new("controller-preflight-v1");
    center
        .register_source(
            "workspace",
            "workspace-core",
            [
                open_gpui_command::CommandContribution::new(
                    open_gpui_command::CommandDescriptor::new("workspace.open", "Open Workspace"),
                ),
                open_gpui_command::CommandContribution::new(
                    open_gpui_command::CommandDescriptor::new("workspace.save", "Save Workspace"),
                ),
            ],
        )
        .unwrap();
    center
        .register_action("workspace.open", OpenPaletteCommand)
        .register_action("workspace.save", RevealPaletteCommand)
        .set_availability(
            open_gpui_command::CommandAvailabilityMap::new()
                .disabled("workspace.save", "Workspace is read-only"),
        )
        .register_key_bindings(
            "workspace-shortcuts",
            [
                open_gpui_command::CommandKeyBinding::new("workspace.open", "ctrl-k ctrl-o"),
                open_gpui_command::CommandKeyBinding::new("workspace.save", "ctrl-s"),
            ],
        );
    let mut keymap = open_gpui::Keymap::default();
    let report = center.install_key_bindings(&mut keymap);
    assert!(report.is_clean());
    let controller = CommandPaletteController::new().with_query("open workspace");

    let pending: CommandPaletteKeymapPreflight = controller
        .preflight_key_sequence_for_keymap(&center, "ctrl-k", &keymap)
        .unwrap();
    assert_eq!(pending.query(), "open workspace");
    assert_eq!(pending.input_label(), "ctrl-k");
    assert!(pending.is_pending());
    assert!(pending.matched_commands().is_empty());
    assert!(
        pending
            .pending_commands()
            .iter()
            .any(|command| command.command_id() == "workspace.open" && command.is_dispatchable())
    );

    let matched = controller
        .preflight_key_sequence_for_keymap(&center, "ctrl-k ctrl-o", &keymap)
        .unwrap();
    assert_eq!(matched.query(), "open workspace");
    assert_eq!(
        matched.primary_dispatchable_command_id(),
        Some("workspace.open")
    );
    assert_eq!(
        matched
            .primary_command()
            .map(|command| (command.command_id(), command.shortcut())),
        Some(("workspace.open", "ctrl-K ctrl-O"))
    );

    let disabled = controller
        .preflight_key_sequence_for_keymap(&center, "ctrl-s", &keymap)
        .unwrap();
    assert_eq!(disabled.query(), "open workspace");
    assert_eq!(disabled.primary_dispatchable_command_id(), None);
    assert_eq!(
        disabled
            .primary_command()
            .and_then(|command| command.state().reason_ref()),
        Some("Workspace is read-only")
    );

    let resolution = disabled.clone().into_resolution();
    assert_eq!(disabled.resolution(), &resolution);
}

#[test]
fn command_palette_controller_navigates_query_history_with_prefix() {
    let mut center = open_gpui_command::CommandCenter::new("history-controller-v1");
    center
        .record_query("open file")
        .record_query("save file")
        .record_query("open settings");

    let keymap = open_gpui::Keymap::default();
    let mut controller = CommandPaletteController::new().with_query("open");

    let latest = controller
        .previous_query_for_keymap(&mut center, &keymap)
        .expect("history entry")
        .unwrap();

    assert!(latest.query_changed());
    assert_eq!(latest.query(), "open settings");
    assert_eq!(controller.query(), "open settings");
    assert_eq!(latest.palette_projection().query(), "open settings");

    let older = controller
        .previous_query_for_keymap(&mut center, &keymap)
        .expect("older history entry")
        .unwrap();
    assert_eq!(older.query(), "open file");
    assert_eq!(controller.query(), "open file");

    let newer = controller
        .next_query_for_keymap(&mut center, &keymap)
        .expect("newer history entry")
        .unwrap();
    assert_eq!(newer.query(), "open settings");
    assert_eq!(controller.query(), "open settings");

    let restored = controller
        .next_query_for_keymap(&mut center, &keymap)
        .expect("restored draft query")
        .unwrap();
    assert_eq!(restored.query(), "open");
    assert_eq!(controller.query(), "open");
    assert_eq!(restored.palette_projection().query(), "open");

    assert!(
        controller
            .next_query_for_keymap(&mut center, &keymap)
            .is_none()
    );
}

#[test]
fn command_palette_controller_tracks_async_provider_requests_and_stale_responses() {
    let mut center = open_gpui_command::CommandCenter::new("async-controller-v1");
    let mut keymap = open_gpui::Keymap::default();
    keymap.add_bindings([open_gpui::KeyBinding::new(
        "ctrl-alt-b",
        RevealPaletteCommand,
        None,
    )]);
    let mut controller =
        CommandPaletteController::new().provider_with_loading("async-provider", "Searching async");

    let alpha = controller
        .set_query_for_keymap(&mut center, "alpha", &keymap)
        .unwrap();
    let alpha_request = alpha
        .provider_projection("async-provider")
        .and_then(open_gpui_command::CommandProviderRefreshProjection::request)
        .expect("alpha request")
        .clone();
    let alpha_pending = alpha
        .pending_provider_request("async-provider")
        .expect("alpha pending provider request");
    assert_eq!(alpha.pending_provider_requests().len(), 1);
    assert_eq!(alpha_pending.provider_id().as_str(), "async-provider");
    assert_eq!(alpha_pending.request(), &alpha_request);
    assert_eq!(alpha_pending.request().query(), "alpha");

    assert_eq!(
        alpha
            .missing_provider_ids()
            .iter()
            .map(|provider_id| provider_id.as_str())
            .collect::<Vec<_>>(),
        ["async-provider"]
    );
    assert_eq!(
        alpha
            .palette_projection()
            .loading_state()
            .map(CommandLoadingState::message),
        Some("Searching async")
    );

    let beta = controller
        .set_query_for_keymap(&mut center, "beta", &keymap)
        .unwrap();
    let beta_request = beta
        .provider_projection("async-provider")
        .and_then(open_gpui_command::CommandProviderRefreshProjection::request)
        .expect("beta request")
        .clone();
    let beta_pending = beta
        .pending_provider_request("async-provider")
        .expect("beta pending provider request");
    assert_eq!(beta.pending_provider_requests().len(), 1);
    assert_eq!(beta_pending.request(), &beta_request);
    assert_eq!(beta_pending.request().query(), "beta");

    let stale = controller
        .apply_provider_response_for_keymap(
            &mut center,
            "async-provider",
            &alpha_request,
            open_gpui_command::CommandProviderResponse::ready().source(
                open_gpui_command::CommandProviderSource::new(
                    "workspace",
                    "async-provider-results",
                    [open_gpui_command::CommandContribution::new(
                        open_gpui_command::CommandDescriptor::new(
                            "provider.open.alpha",
                            "Open Alpha",
                        )
                        .group("Provider"),
                    )],
                ),
            ),
            &keymap,
        )
        .expect("async provider is controlled")
        .unwrap();

    assert!(
        stale
            .provider_projection("async-provider")
            .is_some_and(|projection| projection
                .outcome()
                .is_some_and(open_gpui_command::CommandProviderApplyOutcome::stale))
    );
    assert_eq!(stale.palette_projection().query(), "beta");
    assert_eq!(
        stale
            .palette_projection()
            .loading_state()
            .map(CommandLoadingState::message),
        Some("Searching async")
    );

    center.register_action("provider.open.beta", RevealPaletteCommand);
    let ready = controller
        .apply_provider_response_for_keymap(
            &mut center,
            "async-provider",
            &beta_request,
            open_gpui_command::CommandProviderResponse::ready().source(
                open_gpui_command::CommandProviderSource::new(
                    "workspace",
                    "async-provider-results",
                    [open_gpui_command::CommandContribution::new(
                        open_gpui_command::CommandDescriptor::new(
                            "provider.open.beta",
                            "Open Beta",
                        )
                        .group("Provider"),
                    )],
                ),
            ),
            &keymap,
        )
        .expect("async provider is controlled")
        .unwrap();

    assert!(
        ready
            .provider_projection("async-provider")
            .is_some_and(|projection| projection
                .outcome()
                .is_some_and(open_gpui_command::CommandProviderApplyOutcome::applied))
    );
    assert_eq!(ready.palette_projection().loading_state(), None);
    assert!(ready.palette_projection().shortcut_diagnostics().is_empty());
    assert!(ready.pending_provider_requests().is_empty());

    let state = Command::new("async-controller-command", "Async commands")
        .palette_projection(ready.palette_projection())
        .selected("provider.open.beta")
        .active("provider.open.beta")
        .state();

    assert_eq!(state.query(), "beta");
    assert_eq!(state.filtered_item_count(), 1);
    assert_eq!(
        state.group_items(0).next().and_then(|item| item.shortcut()),
        Some("ctrl-alt-B")
    );
}

#[test]
fn command_provider_palette_projection_maps_refresh_projection_to_prefiltered_index() {
    let mut center = open_gpui_command::CommandCenter::new("provider-center-v1");
    center.register_provider(
        "recent-provider",
        |request: &open_gpui_command::CommandProviderRequest| {
            open_gpui_command::CommandProviderResponse::ready().source(
                open_gpui_command::CommandProviderSource::new(
                    "workspace",
                    "recent-provider-results",
                    [
                        open_gpui_command::CommandContribution::new(
                            open_gpui_command::CommandDescriptor::new(
                                format!("provider.open.{}", request.query()),
                                format!("Open {}", request.query()),
                            )
                            .group("Provider")
                            .keyword("recent"),
                        ),
                        open_gpui_command::CommandContribution::new(
                            open_gpui_command::CommandDescriptor::new(
                                format!("provider.reveal.{}", request.query()),
                                format!("Reveal {}", request.query()),
                            )
                            .group("Provider")
                            .keyword("dynamic"),
                        ),
                    ],
                ),
            )
        },
    );
    let mut controller =
        open_gpui_command::CommandProviderRefreshController::new("recent-provider")
            .with_loading_message("Searching provider commands");

    let projection = controller
        .refresh_provider(&mut center, "alpha")
        .expect("provider should be registered")
        .expect("provider response should be valid");
    let palette_projection = CommandProviderPaletteProjection::from_refresh_projection(&projection);

    assert_eq!(palette_projection.query(), "alpha");
    assert_eq!(palette_projection.loading_state(), None);
    assert_eq!(
        palette_projection.index_snapshot().revision(),
        "provider-center-v1"
    );
    assert_eq!(
        palette_projection.index_snapshot().snapshot_mode(),
        CommandIndexSnapshotMode::PreFiltered
    );
    assert_eq!(
        palette_projection
            .provider_status()
            .map(|status| (status.query(), status.command_count())),
        Some((Some("alpha"), 2))
    );

    let state = Command::new("provider-command", "Provider commands")
        .provider_refresh_projection(&projection)
        .selected("provider.open.alpha")
        .active("provider.open.alpha")
        .state();

    assert_eq!(state.query(), "alpha");
    assert_eq!(state.index_revision(), Some("provider-center-v1"));
    assert_eq!(state.index_mode(), CommandIndexSnapshotMode::PreFiltered);
    assert_eq!(state.filtered_item_count(), 2);
    assert_eq!(state.groups()[0].label(), "Provider");
    assert_eq!(
        state
            .group_items(0)
            .map(|item| (item.value().to_owned(), item.match_source()))
            .collect::<Vec<_>>(),
        vec![
            ("provider.open.alpha".to_string(), None),
            ("provider.reveal.alpha".to_string(), None),
        ]
    );
}

#[test]
fn command_provider_palette_projection_carries_loading_status_into_index_snapshot() {
    let mut center = open_gpui_command::CommandCenter::new("provider-center-v1");
    let mut controller =
        open_gpui_command::CommandProviderRefreshController::new("recent-provider")
            .with_loading_message("Searching provider commands");

    let projection = controller.set_query(&mut center, "alpha").unwrap();
    let palette_projection = CommandProviderPaletteProjection::from(&projection);
    let state = Command::new("loading-provider-command", "Provider commands")
        .provider_refresh_projection(&projection)
        .state();

    assert_eq!(palette_projection.query(), "alpha");
    assert_eq!(
        palette_projection
            .provider_status()
            .map(open_gpui_command::CommandProviderStatus::state),
        Some(open_gpui_command::CommandProviderState::Loading)
    );
    assert_eq!(
        palette_projection
            .loading_state()
            .map(CommandLoadingState::message),
        Some("Searching provider commands")
    );
    assert_eq!(
        state.loading().map(CommandLoadingState::message),
        Some("Searching provider commands")
    );
    assert_eq!(state.index_mode(), CommandIndexSnapshotMode::PreFiltered);
}

#[test]
fn command_behavior_snapshot_exposes_disabled_reasons() {
    let snapshot = Command::new("reason-command", "Commands")
        .item(CommandItem::new("delete-file", "Delete File").disabled_reason("No file selected"))
        .behavior_snapshot();

    assert_eq!(snapshot.rows()[0].value(), "delete-file");
    assert!(snapshot.rows()[0].disabled());
    assert_eq!(
        snapshot.rows()[0].disabled_reason_ref(),
        Some("No file selected")
    );
}

#[test]
fn command_behavior_snapshot_virtualizes_large_result_sets_with_stable_rows() {
    let command =
        Command::new("large-command", "Commands")
            .with_size(Size::Small)
            .row_height(ui_px(28.0))
            .overscan(4)
            .active("item-0104")
            .selected("item-0101")
            .items((0..10_000).map(|index| {
                CommandItem::new(format!("item-{index:04}"), format!("Item {index:04}"))
            }));
    let snapshot = command.behavior_snapshot_with_viewport(ui_px(2_800.0), ui_px(196.0));

    assert_eq!(snapshot.role(), Role::ListBox);
    assert_eq!(snapshot.row_role(), Role::ListBoxOption);
    assert_eq!(snapshot.state().total_item_count(), 10_000);
    assert_eq!(snapshot.state().filtered_item_count(), 10_000);
    assert_eq!(*snapshot.visible_range(), VirtualizerRange::new(100, 107));
    assert_eq!(*snapshot.overscan_range(), VirtualizerRange::new(98, 109));
    assert_eq!(snapshot.visible_row_count(), 7);
    assert_eq!(snapshot.rendered_row_count(), 11);
    assert_eq!(snapshot.rows()[0].index(), 98);
    assert_eq!(snapshot.rows()[0].render_key(), "item-0098");

    let active = snapshot
        .active_row()
        .expect("active command row should render");
    assert_eq!(active.index(), 104);
    assert_eq!(active.value(), "item-0104");
    assert!(active.active());
    assert_eq!(active.virtual_start(), ui_px(2_912.0));
    assert_eq!(active.virtual_size(), ui_px(28.0));
    assert_eq!(
        snapshot
            .selected_rows()
            .map(|row| row.value().to_owned())
            .collect::<Vec<_>>(),
        vec!["item-0101".to_string()]
    );

    let scrolled = command.behavior_snapshot_with_viewport(ui_px(5_600.0), ui_px(196.0));
    assert_eq!(*scrolled.visible_range(), VirtualizerRange::new(200, 207));
    assert_eq!(scrolled.rows()[0].value(), "item-0198");
}

#[test]
fn command_behavior_snapshot_clamps_filtered_scroll_and_disambiguates_duplicate_values() {
    let duplicate_snapshot = Command::new("duplicate-command", "Commands")
        .row_height(ui_px(28.0))
        .item(CommandItem::new("duplicate", "Open File"))
        .item(CommandItem::new("duplicate", "Open Recent"))
        .item(CommandItem::new("unique", "Close File"))
        .behavior_snapshot_with_viewport(ui_px(0.0), ui_px(112.0));

    assert_eq!(
        duplicate_snapshot
            .rows()
            .iter()
            .map(|row| (row.value().to_owned(), row.render_key().to_owned()))
            .collect::<Vec<_>>(),
        vec![
            ("duplicate".to_string(), "0:duplicate".to_string()),
            ("duplicate".to_string(), "1:duplicate".to_string()),
            ("unique".to_string(), "unique".to_string()),
        ]
    );

    let filtered =
        Command::new("filtered-command", "Commands")
            .default_query("item 0001")
            .row_height(ui_px(28.0))
            .items((0..10_000).map(|index| {
                CommandItem::new(format!("item-{index:04}"), format!("Item {index:04}"))
            }))
            .behavior_snapshot_with_viewport(ui_px(80_000.0), ui_px(112.0));

    assert_eq!(filtered.state().filtered_item_count(), 1);
    assert_eq!(filtered.scroll_offset(), ui_px(0.0));
    assert_eq!(filtered.rows()[0].value(), "item-0001");
}

#[test]
fn command_multi_selection_change_toggles_values_without_duplicates() {
    let add = CommandSelectionChange::new(
        vec!["open-file".to_string(), "new-file".to_string()],
        CommandSelection::new(1, "new-file", "New File", None),
        true,
    );
    let remove = CommandSelectionChange::new(
        vec!["open-file".to_string()],
        CommandSelection::new(1, "new-file", "New File", None),
        false,
    );

    assert_eq!(
        add.values(),
        &["open-file".to_string(), "new-file".to_string()]
    );
    assert!(add.selected());
    assert_eq!(add.toggled().value(), "new-file");
    assert_eq!(remove.values(), &["open-file".to_string()]);
    assert!(!remove.selected());
}

#[test]
fn command_state_models_empty_disabled_and_escape_policy() {
    let state = Command::new("empty-command", "Commands")
        .default_open(true)
        .disabled(true)
        .default_query("missing")
        .item(CommandItem::new("open", "Open"))
        .escape_key_policy(EscapeKeyPolicy::Ignore)
        .focus_restore_intent(FocusRestoreIntent::None)
        .state();

    assert_eq!(state.open_mode(), CommandOpenMode::Uncontrolled);
    assert!(state.default_open());
    assert!(state.disabled());
    assert!(!state.open());
    assert_eq!(state.filtered_item_count(), 0);
    assert!(state.listbox().empty());
    assert!(!state.input().editable());
    assert_eq!(state.escape_key_policy(), EscapeKeyPolicy::Ignore);
    assert_eq!(
        state.overlay().policy().escape_key_policy(),
        EscapeKeyPolicy::Ignore
    );
    assert_eq!(state.focus_restore_intent(), &FocusRestoreIntent::None);
    assert!(!state.overlay().should_render_deferred_layer());
}

#[open_gpui::test]
fn command_runtime_filters_input_and_selects_with_keyboard(cx: &mut open_gpui::TestAppContext) {
    struct TestView {
        selections: Rc<RefCell<Vec<CommandSelection>>>,
    }

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let selections = self.selections.clone();

            div().size_full().child(
                Command::new("runtime-command", "Runtime command")
                    .placeholder("Type a command")
                    .item(CommandItem::new("open-file", "Open File").shortcut("Ctrl+O"))
                    .group(
                        CommandGroup::new("file", "File")
                            .item(CommandItem::new("new-file", "New File").shortcut("Ctrl+N"))
                            .item(
                                CommandItem::new("close-window", "Close Window").shortcut("Alt+F4"),
                            ),
                    )
                    .group(CommandGroup::new("view", "View").item(
                        CommandItem::new("toggle-sidebar", "Toggle Sidebar").keyword("layout"),
                    ))
                    .on_select(move |selection, _, _| {
                        selections.borrow_mut().push(selection);
                    }),
            )
        }
    }

    cx.update(init_text_input);
    let selections = Rc::new(RefCell::new(Vec::new()));
    let (_, cx) = cx.add_window_view(|_, _| TestView {
        selections: selections.clone(),
    });
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    assert!(
        cx.debug_bounds("command:runtime-command:content").is_some(),
        "inline command content should render immediately"
    );
    let input = cx
        .debug_bounds("text-input:runtime-command-input:root")
        .expect("command text input should expose a stable debug selector");
    cx.simulate_click(input.center(), Default::default());
    cx.simulate_input("file");
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    assert!(
        cx.debug_bounds("listbox:runtime-command-listbox:option:open-file")
            .is_some(),
        "Open File should match query text"
    );
    assert!(
        cx.debug_bounds("listbox:runtime-command-listbox:option:new-file")
            .is_some(),
        "New File should match query text"
    );
    assert!(
        cx.debug_bounds("listbox:runtime-command-listbox:option:toggle-sidebar")
            .is_none(),
        "Toggle Sidebar should be filtered out before keyboard activation"
    );

    cx.simulate_keystrokes("down");
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    assert!(
        selections.borrow().is_empty(),
        "arrow navigation should move active command without selecting"
    );

    cx.simulate_keystrokes("enter");
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    assert_eq!(
        selections.borrow().clone(),
        vec![CommandSelection::new(
            1,
            "new-file",
            "New File",
            Some("Ctrl+N".to_string())
        )]
    );
    assert!(
        cx.debug_bounds("command:runtime-command:content").is_some(),
        "inline command selection should not close non-dialog content"
    );
}

#[open_gpui::test]
fn command_runtime_controlled_query_emits_sanitized_query_changes(
    cx: &mut open_gpui::TestAppContext,
) {
    struct TestView {
        query: Rc<RefCell<String>>,
        changes: Rc<RefCell<Vec<String>>>,
    }

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let query = self.query.borrow().clone();
            let next_query = self.query.clone();
            let changes = self.changes.clone();

            div().size_full().child(
                Command::new("controlled-query-runtime-command", "Runtime command")
                    .query(query)
                    .placeholder("Type a command")
                    .item(CommandItem::new("open-file", "Open File"))
                    .item(CommandItem::new("close-window", "Close Window"))
                    .on_query_change(move |query, _, _| {
                        *next_query.borrow_mut() = query.clone();
                        changes.borrow_mut().push(query);
                    }),
            )
        }
    }

    cx.update(init_text_input);
    let query = Rc::new(RefCell::new(String::new()));
    let changes = Rc::new(RefCell::new(Vec::new()));
    let (_, cx) = cx.add_window_view(|_, _| TestView {
        query: query.clone(),
        changes: changes.clone(),
    });
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    let input = cx
        .debug_bounds("text-input:controlled-query-runtime-command-input:root")
        .expect("controlled command input should expose a stable debug selector");
    cx.simulate_click(input.center(), Default::default());
    cx.simulate_input("open\nfile");
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    assert_eq!(query.borrow().as_str(), "open file");
    assert_eq!(
        changes.borrow().last().map(String::as_str),
        Some("open file")
    );
    assert!(
        cx.debug_bounds("listbox:controlled-query-runtime-command-listbox:option:open-file")
            .is_some(),
        "controlled query should feed filtered command rows after caller feedback"
    );
}

#[open_gpui::test]
fn command_runtime_dialog_selects_and_dismisses_without_stale_modal_layer(
    cx: &mut open_gpui::TestAppContext,
) {
    #[derive(Clone, Debug, PartialEq, Eq)]
    enum CommandDialogRuntimeEvent {
        Open(bool),
        Select(CommandSelection),
    }

    struct TestView {
        events: Rc<RefCell<Vec<CommandDialogRuntimeEvent>>>,
    }

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let open_events = self.events.clone();
            let select_events = self.events.clone();

            div().size_full().child(
                Command::new("dialog-runtime-command", "Dialog runtime command")
                    .dialog("Command palette")
                    .trigger_label("Open command")
                    .placeholder("Type a command")
                    .item(CommandItem::new("open-file", "Open File").shortcut("Ctrl+O"))
                    .group(
                        CommandGroup::new("file", "File")
                            .item(CommandItem::new("new-file", "New File").shortcut("Ctrl+N"))
                            .item(
                                CommandItem::new("close-window", "Close Window").shortcut("Alt+F4"),
                            ),
                    )
                    .group(CommandGroup::new("view", "View").item(
                        CommandItem::new("toggle-sidebar", "Toggle Sidebar").keyword("layout"),
                    ))
                    .on_open_change(move |open, _, _| {
                        open_events
                            .borrow_mut()
                            .push(CommandDialogRuntimeEvent::Open(open));
                    })
                    .on_select(move |selection, _, _| {
                        select_events
                            .borrow_mut()
                            .push(CommandDialogRuntimeEvent::Select(selection));
                    }),
            )
        }
    }

    cx.update(init_text_input);
    let events = Rc::new(RefCell::new(Vec::new()));
    let (_, cx) = cx.add_window_view(|_, _| TestView {
        events: events.clone(),
    });
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    assert!(
        cx.debug_bounds("command:dialog-runtime-command:content")
            .is_none(),
        "dialog command content should start closed"
    );

    let trigger = cx
        .debug_bounds("command:dialog-runtime-command:trigger")
        .expect("dialog command trigger should expose a stable debug selector");
    cx.simulate_click(trigger.center(), Default::default());
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    assert_eq!(
        events.borrow().clone(),
        vec![CommandDialogRuntimeEvent::Open(true)]
    );
    assert!(
        cx.debug_bounds("command:dialog-runtime-command:content")
            .is_some(),
        "trigger click should open dialog command content"
    );

    let input = cx
        .debug_bounds("text-input:dialog-runtime-command-input:root")
        .expect("dialog command text input should expose a stable debug selector");
    cx.simulate_click(input.center(), Default::default());
    cx.simulate_input("file");
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    assert!(
        cx.debug_bounds("listbox:dialog-runtime-command-listbox:option:open-file")
            .is_some(),
        "Open File should match query text in dialog mode"
    );
    assert!(
        cx.debug_bounds("listbox:dialog-runtime-command-listbox:option:new-file")
            .is_some(),
        "New File should match query text in dialog mode"
    );
    assert!(
        cx.debug_bounds("listbox:dialog-runtime-command-listbox:option:toggle-sidebar")
            .is_none(),
        "unmatched command rows should be filtered out in dialog mode"
    );

    cx.simulate_keystrokes("down");
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });
    assert_eq!(
        events.borrow().clone(),
        vec![CommandDialogRuntimeEvent::Open(true)],
        "arrow navigation should move the active command without selecting"
    );

    cx.simulate_keystrokes("enter");
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    assert_eq!(
        events.borrow().clone(),
        vec![
            CommandDialogRuntimeEvent::Open(true),
            CommandDialogRuntimeEvent::Select(CommandSelection::new(
                1,
                "new-file",
                "New File",
                Some("Ctrl+N".to_string()),
            )),
            CommandDialogRuntimeEvent::Open(false),
        ]
    );
    assert!(
        cx.debug_bounds("command:dialog-runtime-command:content")
            .is_none(),
        "dialog command selection should close the modal content"
    );

    let trigger = cx
        .debug_bounds("command:dialog-runtime-command:trigger")
        .expect("dialog command trigger should remain rendered after selection");
    cx.simulate_click(trigger.center(), Default::default());
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });
    let input = cx
        .debug_bounds("text-input:dialog-runtime-command-input:root")
        .expect("dialog command input should render after reopening");
    cx.simulate_click(input.center(), Default::default());
    cx.simulate_keystrokes("escape");
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    assert_eq!(
        events.borrow().clone(),
        vec![
            CommandDialogRuntimeEvent::Open(true),
            CommandDialogRuntimeEvent::Select(CommandSelection::new(
                1,
                "new-file",
                "New File",
                Some("Ctrl+N".to_string()),
            )),
            CommandDialogRuntimeEvent::Open(false),
            CommandDialogRuntimeEvent::Open(true),
            CommandDialogRuntimeEvent::Open(false),
        ],
        "escape should close a reopened dialog exactly once"
    );
    assert!(
        cx.debug_bounds("command:dialog-runtime-command:content")
            .is_none(),
        "escape should remove the dialog content"
    );

    let trigger = cx
        .debug_bounds("command:dialog-runtime-command:trigger")
        .expect("dialog command trigger should remain rendered after escape");
    cx.simulate_click(trigger.center(), Default::default());
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });
    cx.simulate_click(point(px(4.0), px(4.0)), Default::default());
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    assert_eq!(
        events.borrow().clone(),
        vec![
            CommandDialogRuntimeEvent::Open(true),
            CommandDialogRuntimeEvent::Select(CommandSelection::new(
                1,
                "new-file",
                "New File",
                Some("Ctrl+N".to_string()),
            )),
            CommandDialogRuntimeEvent::Open(false),
            CommandDialogRuntimeEvent::Open(true),
            CommandDialogRuntimeEvent::Open(false),
            CommandDialogRuntimeEvent::Open(true),
            CommandDialogRuntimeEvent::Open(false),
        ],
        "outside press should close a reopened dialog exactly once"
    );
    assert!(
        cx.debug_bounds("command:dialog-runtime-command:content")
            .is_none(),
        "outside press should remove the dialog content"
    );
}

#[open_gpui::test]
fn command_runtime_multi_select_toggles_chips_without_closing_dialog(
    cx: &mut open_gpui::TestAppContext,
) {
    struct TestView {
        selected_values: Rc<RefCell<Vec<String>>>,
        changes: Rc<RefCell<Vec<CommandSelectionChange>>>,
    }

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let selected_values = self.selected_values.borrow().clone();
            let next_values = self.selected_values.clone();
            let changes = self.changes.clone();

            div().size_full().child(
                Command::new("multi-runtime-command", "Runtime command")
                    .dialog("Command palette")
                    .trigger_label("Open command")
                    .multi_select(true)
                    .selected_values(selected_values)
                    .item(CommandItem::new("open-file", "Open File"))
                    .item(CommandItem::new("new-file", "New File"))
                    .item(CommandItem::new("delete-file", "Delete File").disabled(true))
                    .on_selected_values_change(move |change, _, _| {
                        *next_values.borrow_mut() = change.values().to_vec();
                        changes.borrow_mut().push(change);
                    }),
            )
        }
    }

    cx.update(init_text_input);
    let selected_values = Rc::new(RefCell::new(vec!["open-file".to_string()]));
    let changes = Rc::new(RefCell::new(Vec::new()));
    let (_, cx) = cx.add_window_view(|_, _| TestView {
        selected_values: selected_values.clone(),
        changes: changes.clone(),
    });
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    let trigger = cx
        .debug_bounds("command:multi-runtime-command:trigger")
        .expect("multi command trigger should expose a stable debug selector");
    cx.simulate_click(trigger.center(), Default::default());
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    assert!(
        cx.debug_bounds("command:multi-runtime-command:selected-chip:open-file")
            .is_some(),
        "initial selected value should render as a chip"
    );

    let new_file = cx
        .debug_bounds("listbox:multi-runtime-command-listbox:option:new-file")
        .expect("New File option should render");
    cx.simulate_click(new_file.center(), Default::default());
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    assert!(
        cx.debug_bounds("command:multi-runtime-command:content")
            .is_some(),
        "multi-select activation should not close dialog content"
    );
    assert_eq!(
        selected_values.borrow().as_slice(),
        &["open-file".to_string(), "new-file".to_string()]
    );
    assert_eq!(changes.borrow().len(), 1);
    assert!(changes.borrow()[0].selected());
    assert_eq!(changes.borrow()[0].toggled().value(), "new-file");
    assert!(
        cx.debug_bounds("command:multi-runtime-command:selected-chip:new-file")
            .is_some(),
        "newly selected value should render as a chip after controlled feedback"
    );

    let disabled = cx
        .debug_bounds("listbox:multi-runtime-command-listbox:option:delete-file")
        .expect("disabled matching option should still render");
    cx.simulate_click(disabled.center(), Default::default());
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    assert_eq!(
        selected_values.borrow().as_slice(),
        &["open-file".to_string(), "new-file".to_string()],
        "disabled command should not alter the multi-selection set"
    );
    assert_eq!(changes.borrow().len(), 1);

    let open_file = cx
        .debug_bounds("listbox:multi-runtime-command-listbox:option:open-file")
        .expect("Open File option should render");
    cx.simulate_click(open_file.center(), Default::default());
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    assert_eq!(
        selected_values.borrow().as_slice(),
        &["new-file".to_string()]
    );
    assert_eq!(changes.borrow().len(), 2);
    assert!(!changes.borrow()[1].selected());
    assert_eq!(changes.borrow()[1].toggled().value(), "open-file");
}

#[open_gpui::test]
fn command_runtime_virtualized_results_scroll_inside_viewport_and_reveal_keyboard_targets(
    cx: &mut open_gpui::TestAppContext,
) {
    struct TestView {
        selections: Rc<RefCell<Vec<CommandSelection>>>,
    }

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let selections = self.selections.clone();
            let items = (0..120).map(|index| {
                CommandItem::new(format!("item-{index:04}"), format!("Item {index:04}"))
            });

            div().size_full().child(
                div().w(px(340.0)).h(px(420.0)).child(
                    ScrollArea::new(
                        "command-parent-scroll",
                        div()
                            .flex()
                            .flex_col()
                            .child(
                                div()
                                    .debug_selector(|| "command-parent-top".into())
                                    .h(px(48.0))
                                    .child("Parent top"),
                            )
                            .child(
                                div()
                                    .debug_selector(|| "command-wrapper".into())
                                    .h(px(300.0))
                                    .min_h(px(0.0))
                                    .overflow_hidden()
                                    .child(
                                        Command::new("virtualized-runtime-command", "Commands")
                                            .with_size(Size::Small)
                                            .row_height(ui_px(28.0))
                                            .overscan(2)
                                            .viewport_item_count(4)
                                            .items(items)
                                            .on_select(move |selection, _, _| {
                                                selections.borrow_mut().push(selection);
                                            }),
                                    ),
                            )
                            .child(
                                div()
                                    .debug_selector(|| "command-parent-bottom".into())
                                    .h(px(240.0))
                                    .child("Parent bottom"),
                            ),
                    )
                    .vertical(),
                ),
            )
        }
    }

    cx.update(init_text_input);
    let selections = Rc::new(RefCell::new(Vec::new()));
    let (_, cx) = cx.add_window_view(|_, _| TestView {
        selections: selections.clone(),
    });
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    assert!(
        cx.debug_bounds("command:virtualized-runtime-command:row:item-0000")
            .is_some(),
        "initial command row should render"
    );
    assert!(
        cx.debug_bounds("command:virtualized-runtime-command:row:item-0010")
            .is_none(),
        "row 10 should stay outside the initial virtual window"
    );
    let parent_bottom_before = cx
        .debug_bounds("command-parent-bottom")
        .expect("parent bottom should render before command scrolling");
    let viewport = cx
        .debug_bounds("scroll-area:Commands:command-list-scroll")
        .expect("command result viewport should expose a stable scroll selector");

    cx.simulate_event(ScrollWheelEvent {
        position: viewport.center(),
        delta: ScrollDelta::Pixels(point(px(0.0), px(-240.0))),
        ..Default::default()
    });
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    let parent_bottom_after = cx
        .debug_bounds("command-parent-bottom")
        .expect("parent bottom should remain rendered after command scrolling");
    assert_eq!(
        parent_bottom_after.top(),
        parent_bottom_before.top(),
        "expected wheel input inside Command to stay inside the command viewport"
    );
    assert!(
        cx.debug_bounds("command:virtualized-runtime-command:row:item-0000")
            .is_none(),
        "row 0 should unmount after internal command scroll"
    );
    assert!(
        cx.debug_bounds("command:virtualized-runtime-command:row:item-0010")
            .is_some(),
        "row 10 should render after internal command scroll"
    );

    let input = cx
        .debug_bounds("text-input:virtualized-runtime-command-input:root")
        .expect("virtualized command input should render");
    cx.simulate_click(input.center(), Default::default());
    cx.simulate_keystrokes("pagedown");
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });
    assert!(
        cx.debug_bounds("command:virtualized-runtime-command:row:item-0007")
            .is_some(),
        "PageDown should reveal the newly active command row"
    );

    cx.simulate_keystrokes("enter");
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    assert_eq!(
        selections.borrow().as_slice(),
        &[CommandSelection::new(7, "item-0007", "Item 0007", None)]
    );
}
