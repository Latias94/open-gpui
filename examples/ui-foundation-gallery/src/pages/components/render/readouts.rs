use super::*;

pub(crate) fn component_status_cue_state_row(state: &StatusCueState) -> impl IntoElement {
    let metrics = state.metrics();

    div()
        .flex()
        .flex_col()
        .gap_1()
        .text_xs()
        .text_color(rgb(0x5a6472))
        .child(format!(
            "{} / {:?} / {}",
            state.intent().as_str(),
            state.role(),
            state.size().as_str()
        ))
        .child(format!(
            "marker {} / gap {} / text {}",
            format_px(metrics.marker_size()),
            format_px(metrics.gap()),
            format_px(metrics.text_size())
        ))
        .child(format!("display-only {}", state.display_only()))
}

pub(crate) fn component_empty_state_state_row(state: &EmptyStateState) -> impl IntoElement {
    let metrics = state.metrics();

    div()
        .flex()
        .flex_col()
        .gap_1()
        .text_xs()
        .text_color(rgb(0x5a6472))
        .child(format!(
            "{} / {:?} / {}",
            state.intent().as_str(),
            state.role(),
            state.size().as_str()
        ))
        .child(format!(
            "description {} / max {}",
            if state.description().is_some() {
                "present"
            } else {
                "none"
            },
            format_px(metrics.max_width())
        ))
        .child(format!(
            "padding {} / gap {}",
            format_px(metrics.padding()),
            format_px(metrics.gap())
        ))
}

pub(crate) fn component_accordion_state_row(state: &AccordionState) -> impl IntoElement {
    let open = if state.open_values().is_empty() {
        "none".to_owned()
    } else {
        state.open_values().join(",")
    };
    let disabled_count = state.items().iter().filter(|item| item.disabled()).count();

    div()
        .flex()
        .flex_col()
        .gap_1()
        .text_xs()
        .text_color(rgb(0x5a6472))
        .child(format!(
            "{} / collapsible {} / {}",
            state.mode().as_str(),
            state.collapsible(),
            state.size().as_str()
        ))
        .child(format!(
            "{} items / {} disabled / open {}",
            state.items().len(),
            disabled_count,
            open
        ))
}

pub(crate) fn component_collapsible_state_row(state: &CollapsibleState) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .gap_1()
        .text_xs()
        .text_color(rgb(0x5a6472))
        .child(format!(
            "{:?} trigger / {:?} panel / {}",
            state.trigger_role(),
            state.content_role(),
            state.size().as_str()
        ))
        .child(format!(
            "open {} / disabled {} / next {}",
            state.open(),
            state.disabled(),
            state.next_open()
        ))
}

pub(crate) fn component_slider_state_row(state: &SliderState) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .gap_1()
        .text_xs()
        .text_color(rgb(0x5a6472))
        .child(format!(
            "{:?} / {} / disabled {}",
            state.role(),
            state.size().as_str(),
            state.disabled()
        ))
        .child(format!(
            "value {:.1} / range {:.1}..{:.1} / step {:.1}",
            state.value(),
            state.min(),
            state.max(),
            state.step()
        ))
        .child(format!("normalized {:.2}", state.normalized_value()))
}

pub(crate) fn component_number_input_state_row(state: &NumberInputState) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .gap_1()
        .text_xs()
        .text_color(rgb(0x5a6472))
        .child(format!(
            "{:?} / {} / display {}",
            state.role(),
            state.size().as_str(),
            state.display_value()
        ))
        .child(format!(
            "range {:.1}..{:.1} / step {:.1} / enabled {}",
            state.min(),
            state.max(),
            state.step(),
            state.input_enabled()
        ))
        .child(format!(
            "read-only {} / invalid {} / required {} / busy {}",
            state.read_only(),
            state.invalid(),
            state.required(),
            state.busy()
        ))
}

pub(crate) fn component_toggle_group_state_row(state: &ToggleGroupState) -> impl IntoElement {
    let selected = if state.selected_values().is_empty() {
        "none".to_owned()
    } else {
        state.selected_values().join(",")
    };
    let focused = state.focused_value().unwrap_or("none");
    let disabled_count = state.items().iter().filter(|item| item.disabled()).count();

    div()
        .flex()
        .flex_col()
        .gap_1()
        .text_xs()
        .text_color(rgb(0x5a6472))
        .child(format!(
            "{:?} / {} / {} / required {}",
            state.role(),
            match state.orientation() {
                Orientation::Horizontal => "horizontal",
                Orientation::Vertical => "vertical",
            },
            state.mode().as_str(),
            state.selection_required()
        ))
        .child(format!("selected {} / focus {}", selected, focused))
        .child(format!(
            "{} items / {} disabled",
            state.items().len(),
            disabled_count
        ))
}

pub(crate) fn component_link_state_row(state: &LinkState) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .gap_1()
        .text_xs()
        .text_color(rgb(0x5a6472))
        .child(format!(
            "{:?} / {} / external {}",
            state.role(),
            state.size().as_str(),
            state.external()
        ))
        .child(format!(
            "{} -> {} / activation {}",
            state.label(),
            state.href(),
            state.activation_enabled()
        ))
}

pub(crate) fn component_breadcrumb_state_row(state: &BreadcrumbState) -> impl IntoElement {
    let current = state
        .current_index()
        .and_then(|index| state.items().get(index))
        .map(BreadcrumbItemState::value)
        .unwrap_or("none");
    let links = state
        .items()
        .iter()
        .filter(|item| item.activation_enabled())
        .count();

    div()
        .flex()
        .flex_col()
        .gap_1()
        .text_xs()
        .text_color(rgb(0x5a6472))
        .child(format!(
            "{:?} / {} / disabled {}",
            state.role(),
            state.size().as_str(),
            state.disabled()
        ))
        .child(format!(
            "{} items / {} links / current {}",
            state.items().len(),
            links,
            current
        ))
}

pub(crate) fn component_tag_state_row(state: &TagState) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .gap_1()
        .text_xs()
        .text_color(rgb(0x5a6472))
        .child(format!(
            "{:?} / {} / {}",
            state.role(),
            state.variant().as_str(),
            state.size().as_str()
        ))
        .child(format!(
            "value {} / removable {} / remove-enabled {}",
            state.value(),
            state.removable(),
            state.remove_enabled()
        ))
}

pub(crate) fn component_toast_stack_state_row(state: &ToastStackState) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .gap_1()
        .text_xs()
        .text_color(rgb(0x5a6472))
        .child(format!(
            "{:?} / {} / max {}",
            state.role(),
            state.size().as_str(),
            state.max_visible()
        ))
        .child(format!(
            "{} queued / {} visible / {} overflow",
            state.toasts().len(),
            state.visible_toasts().len(),
            state.overflow_count()
        ))
        .child(format!("expired {}", state.expired_dismissals().len()))
}

pub(crate) fn component_tree_state_contract_row(state: &TreeState) -> impl IntoElement {
    let selected = state
        .selected_index()
        .and_then(|index| state.items().get(index))
        .map(TreeItemState::value)
        .unwrap_or("none");
    let focused = state
        .focused_index()
        .and_then(|index| state.items().get(index))
        .map(TreeItemState::value)
        .unwrap_or("none");
    let disabled_count = state.items().iter().filter(|item| item.disabled()).count();

    div()
        .flex()
        .flex_col()
        .gap_1()
        .text_xs()
        .text_color(rgb(0x5a6472))
        .child(format!(
            "{} visible / {} disabled / {}",
            state.items().len(),
            disabled_count,
            state.size().as_str()
        ))
        .child(format!("selected {} / focus {}", selected, focused))
        .child(format!(
            "left {} / right {}",
            tree_keyboard_action_label(state.keyboard_action_for_key("left")),
            tree_keyboard_action_label(state.keyboard_action_for_key("right"))
        ))
        .child(format!(
            "enter {} / space {}",
            tree_keyboard_action_label(state.keyboard_action_for_key("enter")),
            tree_keyboard_action_label(state.keyboard_action_for_key("space"))
        ))
}

pub(super) fn component_tree_item_readout(item: &TreeItemState) -> impl IntoElement {
    let position = item
        .position_in_set()
        .map(|position| format!("{position}/{}", item.size_of_set()))
        .unwrap_or_else(|| "disabled".to_owned());
    let parent = item.parent_value().unwrap_or("root");

    div()
        .rounded_sm()
        .border_1()
        .border_color(rgb(0xe2e4dc))
        .bg(if item.focused() {
            rgb(0xe8f3ef)
        } else {
            rgb(0xfcfcf8)
        })
        .px_2()
        .py_1()
        .text_xs()
        .text_color(if item.disabled() {
            rgb(0x7a8492)
        } else {
            rgb(0x3f4a57)
        })
        .child(format!(
            "{}:{} / d{} / parent {} / pos {} / expanded {} / selected {}",
            item.index(),
            item.value(),
            item.depth(),
            parent,
            position,
            item.expanded(),
            item.selected()
        ))
}

pub(crate) fn component_virtualized_list_state_contract_row(
    state: &VirtualizedListState,
    scroll_strategy: VirtualizedListScrollStrategy,
) -> impl IntoElement {
    let activation = state
        .activation_for_key("enter")
        .map(|activation| format!("{} / {}", activation.index(), activation.key()))
        .unwrap_or_else(|| "none".to_owned());

    div()
        .flex()
        .flex_col()
        .gap_1()
        .text_xs()
        .text_color(rgb(0x5a6472))
        .child(format!(
            "{} items / active {} ({}) / selected {} ({})",
            state.item_count(),
            optional_index_label(state.active_index()),
            optional_key_label(state.active_key()),
            optional_index_label(state.selected_index()),
            selected_keys_label(state.selected_keys())
        ))
        .child(format!(
            "viewport {} / row {} / overscan {}",
            state.viewport_item_count(),
            format_px(state.metrics().row_height()),
            state.metrics().overscan_count()
        ))
        .child(format!(
            "home {} / end {} / pageup {} / pagedown {}",
            optional_index_label(state.navigation_target("home")),
            optional_index_label(state.navigation_target("end")),
            optional_index_label(state.navigation_target("pageup")),
            optional_index_label(state.navigation_target("pagedown"))
        ))
        .child(format!(
            "activation {} / scroll {}",
            activation,
            scroll_strategy.as_str()
        ))
}

fn optional_index_label(index: Option<usize>) -> String {
    index
        .map(|index| index.to_string())
        .unwrap_or_else(|| "none".to_owned())
}

fn optional_key_label(key: Option<&str>) -> String {
    key.unwrap_or("none").to_owned()
}

fn selected_keys_label<'a>(keys: impl IntoIterator<Item = &'a str>) -> String {
    let keys = keys.into_iter().collect::<Vec<_>>();
    if keys.is_empty() {
        "none".to_owned()
    } else {
        keys.join(", ")
    }
}

fn tree_keyboard_action_label(action: Option<TreeKeyboardAction>) -> String {
    match action {
        Some(TreeKeyboardAction::Focus(target)) => {
            format!("focus {}@{}", target.value(), target.index())
        }
        Some(TreeKeyboardAction::Toggle(toggle)) => {
            format!("toggle {} -> {}", toggle.value(), toggle.expanded())
        }
        Some(TreeKeyboardAction::Select(selection)) => {
            format!("select {}@{}", selection.value(), selection.index())
        }
        None => "none".to_owned(),
    }
}

pub(crate) fn component_tabs_state_row(state: &TabsState) -> impl IntoElement {
    let selected = state.selected_value().unwrap_or("none");
    let focused = state.focused_value().unwrap_or("none");
    let disabled_count = state.items().iter().filter(|item| item.disabled()).count();

    div()
        .flex()
        .flex_col()
        .gap_1()
        .text_xs()
        .text_color(rgb(0x5a6472))
        .child(format!(
            "{} / {} / {}",
            match state.orientation() {
                Orientation::Horizontal => "horizontal",
                Orientation::Vertical => "vertical",
            },
            state.activation_mode().as_str(),
            state.size().as_str()
        ))
        .child(format!("selected {} / focus {}", selected, focused))
        .child(format!(
            "{} items / {} disabled",
            state.items().len(),
            disabled_count
        ))
}

pub(crate) fn component_table_state_row(
    summary: &pages::components::TableSampleStateSummary,
) -> impl IntoElement {
    let mut row = div()
        .flex()
        .flex_col()
        .gap_1()
        .text_xs()
        .text_color(rgb(0x5a6472))
        .child(format!(
            "{} core / {} final / {} rendered",
            summary.core_rows, summary.final_rows, summary.rendered_rows
        ))
        .child(format!(
            "visible {}..{} / overscan {}..{}",
            summary.visible_start,
            summary.visible_end,
            summary.overscan_start,
            summary.overscan_end
        ))
        .child(format!(
            "{} columns / {} aria rows / {} selected",
            summary.aria_columns, summary.aria_rows, summary.selected_rows
        ));

    if summary.grouping_columns > 0 || summary.aggregation_count > 0 || summary.group_rows > 0 {
        row = row.child(format!(
            "grouped {} / expanded {} / groups {} / leaves {} / grouping {} / aggregates {} / custom {} / expanded inputs {}{}",
            summary.grouped_rows,
            summary.expanded_rows,
            summary.group_rows,
            summary.leaf_rows,
            summary.grouping_columns,
            summary.aggregation_count,
            summary.custom_aggregation_count,
            summary.expanded_group_inputs,
            if summary.all_rows_expanded { " all" } else { "" }
        ));
    }

    if summary.header_rows > 1 || summary.header_groups > 0 {
        row = row.child(format!(
            "headers {} / groups {} / leaves {}",
            summary.header_rows, summary.header_groups, summary.visible_leaf_columns
        ));
    }

    if summary.tree_rows > 0 {
        row = row.child(format!(
            "tree {} / branches {} / depth {} / expanded inputs {}{}{}",
            summary.tree_rows,
            summary.tree_branch_rows,
            summary.tree_depth,
            summary.expanded_tree_inputs,
            if summary.manual_expansion {
                " / manual"
            } else {
                ""
            },
            if summary.all_rows_expanded {
                " all"
            } else {
                ""
            }
        ));

        if summary.unloaded_tree_branches > 0
            || summary.loading_tree_rows > 0
            || summary.failed_tree_rows > 0
        {
            row = row.child(format!(
                "async branches unloaded {} / loading {} / failed {}",
                summary.unloaded_tree_branches, summary.loading_tree_rows, summary.failed_tree_rows
            ));
        }
    }

    if summary.manual_filtering || summary.manual_sorting || summary.manual_pagination {
        row = row.child(format!(
            "manual filter {} / sort {} / page {} / page {} size {} / total {} / pages {}",
            summary.manual_filtering,
            summary.manual_sorting,
            summary.manual_pagination,
            summary.pagination_page_index,
            summary.pagination_page_size,
            summary
                .pagination_row_count
                .map(|count| count.to_string())
                .unwrap_or_else(|| "unknown".to_owned()),
            summary
                .pagination_page_count
                .map(|count| count.to_string())
                .unwrap_or_else(|| "unknown".to_owned())
        ));
    }

    if summary.facet_columns > 0 {
        row = row.child(format!(
            "facets {} columns / {} manual / status {} values total {} / score {}..{}",
            summary.facet_columns,
            summary.manual_facet_columns,
            summary.status_facet_values,
            summary.status_facet_total_count,
            summary
                .score_facet_min
                .map(|value| value.to_string())
                .unwrap_or_else(|| "none".to_owned()),
            summary
                .score_facet_max
                .map(|value| value.to_string())
                .unwrap_or_else(|| "none".to_owned())
        ));
    }

    if summary.pinned_top_rows > 0 || summary.pinned_bottom_rows > 0 {
        row = row.child(format!(
            "row pinning {}-{}-{} / {}",
            summary.pinned_top_rows,
            summary.pinned_center_rows,
            summary.pinned_bottom_rows,
            if summary.row_pinning_page_only {
                "page-only"
            } else {
                "keep-pinned"
            }
        ));
    }

    if summary.pinned_left_columns > 0 || summary.pinned_right_columns > 0 {
        row = row.child(format!(
            "pinned {}-{}-{} / widths {}-{}-{}px / {} resizable columns",
            summary.pinned_left_columns,
            summary.pinned_center_columns,
            summary.pinned_right_columns,
            summary.pinned_left_width_px,
            summary.pinned_center_width_px,
            summary.pinned_right_width_px,
            summary.resizable_columns
        ));
    } else {
        row = row.child(format!(
            "width {}px / {} resizable columns",
            summary.total_column_width_px, summary.resizable_columns
        ));
    }

    row
}

pub(crate) fn component_virtualized_list_state_row(
    summary: &pages::components::VirtualizedListSampleStateSummary,
    state: &VirtualizedListState,
) -> impl IntoElement {
    let activation = state
        .activation_for_key("enter")
        .map(|activation| format!("{} / {}", activation.index(), activation.key()))
        .unwrap_or_else(|| "none".to_owned());

    div()
        .flex()
        .flex_col()
        .gap_1()
        .text_xs()
        .text_color(rgb(0x5a6472))
        .child(format!(
            "{} items / active {} ({}) / selected {} ({})",
            summary.item_count,
            optional_index_label(summary.active_index),
            optional_key_label(summary.active_key.as_deref()),
            optional_index_label(summary.selected_index),
            selected_keys_label(summary.selected_keys.iter().map(String::as_str))
        ))
        .child(format!(
            "viewport {} / row {} / overscan {}",
            state.viewport_item_count(),
            format_px(state.metrics().row_height()),
            state.metrics().overscan_count()
        ))
        .child(format!(
            "visible {}..{} / overscan {}..{}",
            summary.visible_start,
            summary.visible_end,
            summary.overscan_start,
            summary.overscan_end
        ))
        .child(format!(
            "{} visible / {} rendered / activation {}",
            summary.visible_rows, summary.rendered_rows, activation
        ))
}

pub(crate) fn component_scroll_area_state_row(state: &ScrollAreaState) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .gap_1()
        .text_xs()
        .text_color(rgb(0x5a6472))
        .child(format!(
            "{} / {} / {}",
            state.axis().as_str(),
            state.reset_policy().as_str(),
            state.size().as_str()
        ))
        .child(format!(
            "viewport {} / scrollbar {}",
            state.viewport_id(),
            format_px(state.metrics().scrollbar_width())
        ))
        .child(format!(
            "x {} / y {}",
            if state.scrolls_x() { "scroll" } else { "clip" },
            if state.scrolls_y() { "scroll" } else { "clip" }
        ))
}

pub(crate) fn component_splitter_state_row(state: &SplitterState) -> impl IntoElement {
    let fractions = state
        .panels()
        .iter()
        .map(|panel| {
            if panel.collapsed() {
                format!("{}:{:.0}% collapsed", panel.id(), panel.fraction() * 100.0)
            } else {
                format!("{}:{:.0}%", panel.id(), panel.fraction() * 100.0)
            }
        })
        .collect::<Vec<_>>()
        .join(" / ");

    div()
        .flex()
        .flex_col()
        .gap_1()
        .text_xs()
        .text_color(rgb(0x5a6472))
        .child(format!(
            "{} / {} panels / {} handles",
            match state.orientation() {
                Orientation::Horizontal => "horizontal",
                Orientation::Vertical => "vertical",
            },
            state.panels().len(),
            state.handles().len()
        ))
        .child(fractions)
        .child(format!(
            "handle {} hit {}",
            format_px(state.metrics().handle_thickness()),
            format_px(state.metrics().handle_hit_size())
        ))
}

pub(crate) fn component_sidebar_state_row(
    sample_id: &'static str,
    state: &SidebarState,
    last_activation: Option<&pages::components::SidebarSampleActivation>,
) -> impl IntoElement {
    let selected = state.selected_value().unwrap_or("none");
    let focused = state.focused_value().unwrap_or("none");
    let disabled_count = state.items().iter().filter(|item| item.disabled()).count();
    let (activation_value, activation_selected, activation_source) =
        last_activation.map_or(("none", false, "none"), |entry| {
            let source = match entry.source() {
                ActivationSource::Pointer => "pointer",
                ActivationSource::Keyboard(ActivationKey::Enter) => "keyboard-enter",
                ActivationSource::Keyboard(ActivationKey::Space) => "keyboard-space",
                ActivationSource::Accessibility => "accessibility",
                ActivationSource::Programmatic => "programmatic",
            };
            (
                entry.activation().value(),
                entry.activation().selected(),
                source,
            )
        });
    let debug_selector = format!("gallery:component-sidebar-sample:{sample_id}:runtime");
    let value_selector = format!("{debug_selector}:last-value:{activation_value}");
    let selected_selector = format!("{debug_selector}:last-selected:{activation_selected}");
    let source_selector = format!("{debug_selector}:last-source:{activation_source}");

    div()
        .debug_selector(move || debug_selector.clone())
        .flex()
        .flex_col()
        .gap_1()
        .text_xs()
        .text_color(rgb(0x5a6472))
        .child(format!(
            "{:?} / {} / {} / {}",
            state.role(),
            state.side().as_str(),
            state.variant().as_str(),
            state.collapse_mode().as_str()
        ))
        .child(format!("selected {} / focus {}", selected, focused))
        .child(format!(
            "{} sections / {} items / {} disabled / width {}",
            state.sections().len(),
            state.items().len(),
            disabled_count,
            format_px(state.metrics().resolved_width())
        ))
        .child(
            div()
                .flex()
                .gap_1()
                .child(
                    div()
                        .debug_selector(move || value_selector.clone())
                        .child(format!("last activation {activation_value}")),
                )
                .child(
                    div()
                        .debug_selector(move || selected_selector.clone())
                        .child(format!("/ selected {activation_selected}")),
                )
                .child(
                    div()
                        .debug_selector(move || source_selector.clone())
                        .child(format!("/ source {activation_source}")),
                ),
        )
}

pub(crate) fn component_toolbar_state_row(state: &ToolbarState) -> impl IntoElement {
    let focused = state.focused_value().unwrap_or("none");
    let disabled_count = state.items().iter().filter(|item| item.disabled()).count();
    let kinds = state
        .items()
        .iter()
        .map(|item| item.kind().as_str())
        .collect::<Vec<_>>()
        .join("/");

    div()
        .flex()
        .flex_col()
        .gap_1()
        .text_xs()
        .text_color(rgb(0x5a6472))
        .child(format!(
            "{:?} / {} / {}",
            state.role(),
            match state.orientation() {
                Orientation::Horizontal => "horizontal",
                Orientation::Vertical => "vertical",
            },
            state.size().as_str()
        ))
        .child(format!("focus {}", focused))
        .child(format!(
            "{} items / {} disabled / {}",
            state.items().len(),
            disabled_count,
            kinds
        ))
}
