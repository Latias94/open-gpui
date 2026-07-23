use super::*;

pub(super) fn component_tree_samples_section(
    tree_samples: &'static [pages::components::TreeSample],
    cx: &mut Context<GalleryShell>,
) -> impl IntoElement {
    let sample_cards = tree_samples
        .iter()
        .map(|sample| {
            let sample_id = sample.id;
            let debug_selector = sample.debug_selector();
            let title = sample.title;
            let summary = sample.summary;
            let badge = sample.badge;
            let sample_id_for_selection = sample_id.to_owned();
            let sample_id_for_toggle = sample_id.to_owned();
            let mut tree = sample
                .build_tree_with_runtime(cx)
                .on_select(move |selection, _, cx| {
                    pages::components::record_tree_selection(
                        sample_id_for_selection.clone(),
                        selection.value().to_owned(),
                        cx,
                    );
                })
                .on_toggle(move |toggle, _, cx| {
                    pages::components::record_tree_toggle(
                        sample_id_for_toggle.clone(),
                        toggle.value().to_owned(),
                        toggle.expanded(),
                        toggle.loaded_child_count(),
                        toggle.children_load_state().as_str().to_owned(),
                        toggle.children_load_state().message().map(str::to_owned),
                        cx,
                    );
                });

            if sample.draggable {
                let sample_id_for_move = sample_id.to_owned();
                let base_items = sample.items.clone();
                tree = tree.on_move(move |tree_move, _, cx| {
                    pages::components::record_tree_move(
                        sample_id_for_move.clone(),
                        &base_items,
                        &tree_move,
                        cx,
                    );
                });
            }

            div()
                .id(format!("component-tree-sample:{sample_id}"))
                .debug_selector(move || debug_selector)
                .w(px(420.0))
                .flex_none()
                .flex()
                .flex_col()
                .gap_2()
                .rounded_sm()
                .border_1()
                .border_color(rgb(0xd6d8ce))
                .bg(rgb(0xffffff))
                .on_scroll_wheel(|_, _, _| {
                    open_gpui::ScrollWheelIntent::handled().stop_propagation()
                })
                .p_3()
                .child(
                    div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .gap_2()
                        .child(
                            div()
                                .text_sm()
                                .font_weight(open_gpui::FontWeight::BOLD)
                                .child(title),
                        )
                        .child(label_pill(badge)),
                )
                .child(div().text_xs().text_color(rgb(0x5a6472)).child(summary))
                .child(
                    div()
                        .h(px(240.0))
                        .min_h(px(0.0))
                        .overflow_hidden()
                        .child(tree),
                )
                .child(component_tree_state_contract_row(&sample.current_state(cx)))
        })
        .collect::<Vec<_>>();

    div()
        .flex()
        .flex_col()
        .gap_2()
        .child(
            div()
                .text_sm()
                .font_weight(open_gpui::FontWeight::BOLD)
                .child("Tree"),
        )
        .child(div().flex().gap_3().flex_wrap().children(sample_cards))
}

pub(super) fn component_feedback_samples_section(
    status_cue_samples: [pages::components::StatusCueSample; 3],
    empty_state_samples: [pages::components::EmptyStateSample; 2],
    tokens: ThemeTokens,
) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .gap_2()
        .child(
            div()
                .text_sm()
                .font_weight(open_gpui::FontWeight::BOLD)
                .child("Feedback"),
        )
        .child(
            div()
                .flex()
                .gap_3()
                .flex_wrap()
                .children(status_cue_samples.into_iter().map(|sample| {
                    let sample_id = sample.id;
                    let debug_selector = sample.debug_selector();
                    let title = sample.title;
                    let state = sample.state.clone();
                    let label = state.label().to_owned();

                    component_gallery_card_shell(
                        format!("component-status-cue-sample:{sample_id}"),
                        Some(debug_selector),
                    )
                    .min_w(px(260.0))
                    .flex()
                    .flex_col()
                    .items_start()
                    .gap_2()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .gap_2()
                            .w_full()
                            .child(
                                div()
                                    .text_sm()
                                    .font_weight(open_gpui::FontWeight::BOLD)
                                    .child(title),
                            )
                            .child(label_pill(state.intent().as_str())),
                    )
                    .child(
                        StatusCue::new(format!("component-status-cue:{sample_id}"), label)
                            .intent(state.intent())
                            .live(state.live())
                            .live_atomic(state.live_atomic())
                            .busy(state.busy())
                            .with_size(state.size())
                            .tokens(tokens),
                    )
                    .child(component_status_cue_state_row(&state))
                })),
        )
        .child(
            div()
                .flex()
                .gap_3()
                .flex_wrap()
                .children(empty_state_samples.into_iter().map(|sample| {
                    let sample_id = sample.id;
                    let debug_selector = sample.debug_selector();
                    let title = sample.title;
                    let state = sample.state.clone();
                    let state_title = state.title().to_owned();
                    let description = state.description().map(str::to_owned);
                    let empty_state =
                        EmptyState::new(format!("component-empty-state:{sample_id}"), state_title)
                            .intent(state.intent())
                            .with_size(state.size())
                            .tokens(tokens);
                    let empty_state = match description {
                        Some(description) => empty_state.description(description),
                        None => empty_state,
                    };

                    component_gallery_card_shell(
                        format!("component-empty-state-sample:{sample_id}"),
                        Some(debug_selector),
                    )
                    .w(px(360.0))
                    .flex()
                    .flex_col()
                    .items_stretch()
                    .gap_2()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .gap_2()
                            .child(
                                div()
                                    .text_sm()
                                    .font_weight(open_gpui::FontWeight::BOLD)
                                    .child(title),
                            )
                            .child(label_pill(state.intent().as_str())),
                    )
                    .child(empty_state)
                    .child(component_empty_state_state_row(&state))
                })),
        )
}

pub(super) fn component_foundation_samples_section(
    samples: pages::components::FoundationComponentSamples,
    tokens: ThemeTokens,
) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .gap_3()
        .child(
            div()
                .text_sm()
                .font_weight(open_gpui::FontWeight::BOLD)
                .child("Foundation components"),
        )
        .child(
            div()
                .flex()
                .gap_3()
                .flex_wrap()
                .children(samples.accordions.into_iter().map(|sample| {
                    let state = sample.state.clone();
                    let mut accordion =
                        Accordion::new(format!("component-accordion:{}", sample.id))
                            .mode(state.mode())
                            .collapsible(state.collapsible())
                            .open_values(state.open_values().iter().cloned())
                            .tokens(tokens);
                    for item in sample.items.clone() {
                        accordion = accordion.item(item);
                    }

                    component_gallery_card_shell(
                        format!("component-accordion-sample:{}", sample.id),
                        Some(sample.debug_selector()),
                    )
                    .w(px(420.0))
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .gap_2()
                            .child(
                                div()
                                    .text_sm()
                                    .font_weight(open_gpui::FontWeight::BOLD)
                                    .child(sample.title),
                            )
                            .child(label_pill(state.mode().as_str())),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(0x5a6472))
                            .child(sample.summary),
                    )
                    .child(accordion)
                    .child(component_accordion_state_row(&state))
                })),
        )
        .child(
            div()
                .flex()
                .gap_3()
                .flex_wrap()
                .children(samples.collapsibles.into_iter().map(|sample| {
                    let state = sample.state.clone();
                    let collapsible = Collapsible::new(
                        format!("component-collapsible:{}", sample.id),
                        state.label(),
                    )
                    .open(state.open())
                    .content(
                        div()
                            .rounded_sm()
                            .border_1()
                            .border_color(rgb(0xe2e4dc))
                            .bg(rgb(0xfcfcf8))
                            .p_2()
                            .text_xs()
                            .text_color(rgb(0x5a6472))
                            .child(sample.content),
                    )
                    .with_size(state.size())
                    .tokens(tokens);

                    component_gallery_card_shell(
                        format!("component-collapsible-sample:{}", sample.id),
                        Some(sample.debug_selector()),
                    )
                    .w(px(360.0))
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .gap_2()
                            .child(
                                div()
                                    .text_sm()
                                    .font_weight(open_gpui::FontWeight::BOLD)
                                    .child(state.label().to_owned()),
                            )
                            .child(label_pill(if state.open() { "open" } else { "closed" })),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(0x5a6472))
                            .child(sample.summary),
                    )
                    .child(collapsible)
                    .child(component_collapsible_state_row(&state))
                })),
        )
        .child(
            div()
                .flex()
                .gap_3()
                .flex_wrap()
                .children(samples.sliders.into_iter().map(|sample| {
                    let state = sample.state.clone();
                    component_gallery_card_shell(
                        format!("component-slider-sample:{}", sample.id),
                        Some(sample.debug_selector()),
                    )
                    .w(px(320.0))
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        Slider::new(format!("component-slider:{}", sample.id), state.label())
                            .value(state.value())
                            .min(state.min())
                            .max(state.max())
                            .step(state.step())
                            .disabled(state.disabled())
                            .with_size(state.size())
                            .tokens(tokens),
                    )
                    .child(component_slider_state_row(&state))
                })),
        )
        .child(
            div()
                .flex()
                .gap_3()
                .flex_wrap()
                .children(samples.number_inputs.into_iter().map(|sample| {
                    let state = sample.state.clone();
                    component_gallery_card_shell(
                        format!("component-number-input-sample:{}", sample.id),
                        Some(sample.debug_selector()),
                    )
                    .w(px(260.0))
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        NumberInput::new(
                            format!("component-number-input:{}", sample.id),
                            state.label(),
                        )
                        .value(state.value())
                        .min(state.min())
                        .max(state.max())
                        .step(state.step())
                        .disabled(state.disabled())
                        .read_only(state.read_only())
                        .invalid(state.invalid())
                        .required(state.required())
                        .busy(state.busy())
                        .with_size(state.size())
                        .tokens(tokens),
                    )
                    .child(component_number_input_state_row(&state))
                })),
        )
        .child(
            div()
                .flex()
                .gap_3()
                .flex_wrap()
                .children(samples.toggle_groups.into_iter().map(|sample| {
                    let state = sample.state.clone();
                    let mut group = ToggleGroup::new(
                        format!("component-toggle-group:{}", sample.id),
                        state.label(),
                    )
                    .orientation(state.orientation())
                    .mode(state.mode())
                    .selection_required(state.selection_required())
                    .selected_values(state.selected_values().iter().cloned())
                    .with_size(state.size())
                    .tokens(tokens);
                    if let Some(focused) = state.focused_value() {
                        group = group.default_focused(focused);
                    }
                    for item in state.items() {
                        group = group.item(
                            ToggleGroupItem::new(item.value(), item.label())
                                .disabled(item.disabled()),
                        );
                    }

                    component_gallery_card_shell(
                        format!("component-toggle-group-sample:{}", sample.id),
                        Some(sample.debug_selector()),
                    )
                    .w(px(380.0))
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .gap_2()
                            .child(
                                div()
                                    .text_sm()
                                    .font_weight(open_gpui::FontWeight::BOLD)
                                    .child(state.label().to_owned()),
                            )
                            .child(label_pill(state.mode().as_str())),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(0x5a6472))
                            .child(sample.summary),
                    )
                    .child(group)
                    .child(component_toggle_group_state_row(&state))
                })),
        )
        .child(
            div()
                .flex()
                .gap_3()
                .flex_wrap()
                .children(samples.links.into_iter().map(|sample| {
                    let state = sample.state.clone();
                    component_gallery_card_shell(
                        format!("component-link-sample:{}", sample.id),
                        Some(sample.debug_selector()),
                    )
                    .min_w(px(220.0))
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        Link::new(
                            format!("component-link:{}", sample.id),
                            state.label(),
                            state.href(),
                        )
                        .external(state.external())
                        .disabled(state.disabled())
                        .with_size(state.size())
                        .tokens(tokens),
                    )
                    .child(component_link_state_row(&state))
                })),
        )
        .child(
            div()
                .flex()
                .gap_3()
                .flex_wrap()
                .children(samples.breadcrumbs.into_iter().map(|sample| {
                    let state = sample.state.clone();
                    let mut breadcrumb = Breadcrumb::new(
                        format!("component-breadcrumb:{}", sample.id),
                        state.label(),
                    )
                    .disabled(state.disabled())
                    .with_size(state.size())
                    .tokens(tokens);
                    for item in state.items() {
                        let mut descriptor =
                            BreadcrumbItemDescriptor::new(item.value(), item.label());
                        if let Some(href) = item.href() {
                            descriptor = descriptor.href(href);
                        }
                        descriptor = descriptor.current(item.current()).disabled(item.disabled());
                        breadcrumb = breadcrumb.item(descriptor);
                    }

                    component_gallery_card_shell(
                        format!("component-breadcrumb-sample:{}", sample.id),
                        Some(sample.debug_selector()),
                    )
                    .w(px(420.0))
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(breadcrumb)
                    .child(component_breadcrumb_state_row(&state))
                })),
        )
        .child(
            div()
                .flex()
                .gap_3()
                .flex_wrap()
                .children(samples.tags.into_iter().map(|sample| {
                    let state = sample.state.clone();
                    component_gallery_card_shell(
                        format!("component-tag-sample:{}", sample.id),
                        Some(sample.debug_selector()),
                    )
                    .min_w(px(180.0))
                    .flex()
                    .flex_col()
                    .items_start()
                    .gap_2()
                    .child(
                        Tag::new(
                            format!("component-tag:{}", sample.id),
                            state.value(),
                            state.label(),
                        )
                        .variant(state.variant())
                        .removable(state.removable())
                        .disabled(state.disabled())
                        .with_size(state.size())
                        .tokens(tokens),
                    )
                    .child(component_tag_state_row(&state))
                })),
        )
        .child(
            div()
                .flex()
                .gap_3()
                .flex_wrap()
                .children(samples.toast_stacks.into_iter().map(|sample| {
                    let state = sample.state.clone();
                    component_gallery_card_shell(
                        format!("component-toast-stack-sample:{}", sample.id),
                        Some(sample.debug_selector()),
                    )
                    .w(px(460.0))
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        ToastStack::new(
                            format!("component-toast-stack:{}", sample.id),
                            state.label(),
                        )
                        .toasts(state.toasts().iter().cloned())
                        .max_visible(state.max_visible())
                        .with_size(state.size())
                        .tokens(tokens),
                    )
                    .child(component_toast_stack_state_row(&state))
                })),
        )
}

pub(super) fn component_state_contract_samples_section(
    tree_samples: [pages::components::TreeStateContractSample; 1],
    virtualized_list_samples: [pages::components::VirtualizedListStateContractSample; 1],
) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .gap_2()
        .child(
            div()
                .text_sm()
                .font_weight(open_gpui::FontWeight::BOLD)
                .child("State contracts"),
        )
        .child(
            div()
                .flex()
                .gap_3()
                .flex_wrap()
                .children(tree_samples.into_iter().map(|sample| {
                    let debug_selector = sample.debug_selector();
                    let state = sample.state.clone();

                    component_gallery_card_shell(
                        format!("component-tree-state-contract:{}", sample.id),
                        Some(debug_selector),
                    )
                    .w(px(520.0))
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .gap_2()
                            .child(
                                div()
                                    .text_sm()
                                    .font_weight(open_gpui::FontWeight::BOLD)
                                    .child(sample.title),
                            )
                            .child(label_pill("TreeState")),
                    )
                    .child(
                        div()
                            .text_xs()
                            .line_height(px(18.0))
                            .text_color(rgb(0x5a6472))
                            .child(sample.summary),
                    )
                    .child(component_tree_state_contract_row(&state))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .children(state.items().iter().map(component_tree_item_readout)),
                    )
                })),
        )
        .child(
            div()
                .flex()
                .gap_3()
                .flex_wrap()
                .children(virtualized_list_samples.into_iter().map(|sample| {
                    let debug_selector = sample.debug_selector();
                    let state = sample.state.clone();
                    let bring_into_view_options = sample.bring_into_view_options;

                    component_gallery_card_shell(
                        format!("component-virtualized-list-state-contract:{}", sample.id),
                        Some(debug_selector),
                    )
                    .w(px(520.0))
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .gap_2()
                            .child(
                                div()
                                    .text_sm()
                                    .font_weight(open_gpui::FontWeight::BOLD)
                                    .child(sample.title),
                            )
                            .child(label_pill("VirtualizedListState")),
                    )
                    .child(
                        div()
                            .text_xs()
                            .line_height(px(18.0))
                            .text_color(rgb(0x5a6472))
                            .child(sample.summary),
                    )
                    .child(component_virtualized_list_state_contract_row(
                        &state,
                        bring_into_view_options,
                    ))
                })),
        )
}
