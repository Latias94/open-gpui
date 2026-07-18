use super::*;

fn render_grouped_components_section(
    shell: &mut GalleryShell,
    parent_id: &'static str,
    focused_id: &'static str,
    snapshot: GalleryShellSnapshot,
    window: &mut Window,
    cx: &mut Context<GalleryShell>,
) -> AnyElement {
    let mut snapshot = snapshot;
    snapshot.components_focus = pages::components::ComponentFocusMode::Section(focused_id);
    render_components_section(
        shell,
        component_page_jump_for_id(parent_id),
        snapshot,
        window,
        cx,
    )
}

pub(super) fn render_components_section(
    shell: &mut GalleryShell,
    section: pages::components::ComponentPageJump,
    snapshot: GalleryShellSnapshot,
    window: &mut Window,
    cx: &mut Context<GalleryShell>,
) -> AnyElement {
    let focus_mode = snapshot.components_focus;

    match section.id {
        "tabs" | "table" | "virtualized-list" | "signals" => {
            render_grouped_components_section(shell, "field", section.id, snapshot, window, cx)
        }
        "catalog" => render_component_catalog_section(snapshot, cx),
        "primitives" => component_page_section("primitives")
            .when(!show_component_section(focus_mode, "primitives"), |this| {
                this.hidden()
            })
            .child(component_primitive_samples_section(
                pages::components::separator_samples(snapshot.tokens),
                pages::components::kbd_samples(snapshot.tokens),
                pages::components::progress_samples(snapshot.tokens),
                pages::components::skeleton_samples(snapshot.tokens),
                pages::components::avatar_samples(snapshot.tokens),
                pages::components::avatar_group_samples(snapshot.tokens),
                snapshot.tokens,
            ))
            .into_any_element(),
        "feedback" => component_page_section("feedback")
            .when(!show_component_section(focus_mode, "feedback"), |this| {
                this.hidden()
            })
            .child(component_feedback_samples_section(
                pages::components::status_cue_samples(snapshot.tokens),
                pages::components::empty_state_samples(snapshot.tokens),
                snapshot.tokens,
            ))
            .into_any_element(),
        "foundation-components" => component_page_section("foundation-components")
            .when(
                !show_component_section(focus_mode, "foundation-components"),
                |this| this.hidden(),
            )
            .child(component_foundation_samples_section(
                pages::components::foundation_component_samples(snapshot.tokens),
                snapshot.tokens,
            ))
            .into_any_element(),
        "state-contracts" => component_page_section("state-contracts")
            .when(
                !show_component_section(focus_mode, "state-contracts"),
                |this| this.hidden(),
            )
            .child(component_state_contract_samples_section(
                pages::components::tree_state_contract_samples(),
                pages::components::virtualized_list_state_contract_samples(),
            ))
            .into_any_element(),
        "ecosystem-adapters" => component_page_section("ecosystem-adapters")
            .when(
                !show_component_section(focus_mode, "ecosystem-adapters"),
                |this| this.hidden(),
            )
            .child(render_component_ecosystem_adapters_section(snapshot.tokens))
            .into_any_element(),
        "sidebar" => component_page_section("sidebar")
            .when(!show_component_section(focus_mode, "sidebar"), |this| {
                this.hidden()
            })
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        div()
                            .text_sm()
                            .font_weight(open_gpui::FontWeight::BOLD)
                            .child("Sidebar"),
                    )
                    .child(
                        div().flex().gap_3().flex_wrap().children(
                            pages::components::sidebar_samples(snapshot.tokens)
                                .into_iter()
                                .map(|sample| {
                                    let state = sample.state.clone();
                                    let title = state.label().to_owned();
                                    let last_activation = cx.read_global::<
                                        pages::components::SidebarSampleRuntimeLog,
                                        _,
                                    >(|log, _| log.last_for(sample.id).cloned());
                                    let mut sidebar = Sidebar::new(
                                        format!("component-sidebar:{}", sample.id),
                                        state.label(),
                                    )
                                    .side(state.side())
                                    .variant(state.variant())
                                    .collapse_mode(state.collapse_mode())
                                    .collapsed(state.collapsed())
                                    .with_size(state.size())
                                    .tokens(snapshot.tokens);
                                    if let Some(selected) = state.selected_value() {
                                        sidebar = sidebar.selected(selected);
                                    }
                                    if let Some(focused) = state.focused_value() {
                                        sidebar = sidebar.default_focused(focused);
                                    }
                                    for section in &sample.sections {
                                        let mut sidebar_section =
                                            SidebarSection::new(section.value, section.label);
                                        for item in &section.items {
                                            let mut sidebar_item =
                                                SidebarItem::new(item.value, item.label);
                                            if !item.icon.is_empty() {
                                                let icon = item.icon;
                                                sidebar_item = sidebar_item.icon(icon);
                                            }
                                            if let Some(badge) = item.badge {
                                                sidebar_item = sidebar_item.badge(badge);
                                            }
                                            if let Some(action_label) = item.action_label {
                                                sidebar_item =
                                                    sidebar_item.action_label(action_label);
                                            }
                                            sidebar_section = sidebar_section
                                                .item(sidebar_item.disabled(item.disabled));
                                        }
                                        sidebar = sidebar.section(sidebar_section);
                                    }
                                    let sample_id = sample.id;
                                    sidebar = sidebar.on_activate(
                                        move |activation, input, window, cx| {
                                            pages::components::record_sidebar_activation(
                                                sample_id,
                                                activation,
                                                input.source(),
                                                cx,
                                            );
                                            window.refresh();
                                        },
                                    );

                                    div()
                                        .id(format!("component-sidebar-sample:{}", sample.id))
                                        .debug_selector({
                                            let debug_selector = sample.debug_selector();
                                            move || debug_selector
                                        })
                                        .w(px(360.0))
                                        .flex()
                                        .flex_col()
                                        .gap_2()
                                        .rounded_sm()
                                        .border_1()
                                        .border_color(rgb(0xd6d8ce))
                                        .bg(rgb(0xffffff))
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
                                                .child(label_pill(state.collapse_mode().as_str())),
                                        )
                                        .child(
                                            div()
                                                .text_xs()
                                                .text_color(rgb(0x5a6472))
                                                .child(sample.summary),
                                        )
                                        .child(
                                            div()
                                                .h(px(214.0))
                                                .min_h(px(0.0))
                                                .flex()
                                                .overflow_hidden()
                                                .rounded_sm()
                                                .border_1()
                                                .border_color(rgb(0xe2e4dc))
                                                .bg(rgb(0xfcfcf8))
                                                .when(state.side() == SidebarSide::Right, |this| {
                                                    this.justify_end()
                                                })
                                                .child(sidebar),
                                        )
                                        .child(component_sidebar_state_row(
                                            sample.id,
                                            &state,
                                            last_activation.as_ref(),
                                        ))
                                }),
                        ),
                    ),
            )
            .into_any_element(),
        "tree" => component_page_section("tree")
            .when(!show_component_section(focus_mode, "tree"), |this| {
                this.hidden()
            })
            .child(component_tree_samples_section(
                pages::components::tree_samples(snapshot.tokens),
                cx,
            ))
            .into_any_element(),
        "toolbar" => component_page_section("toolbar")
            .when(!show_component_section(focus_mode, "toolbar"), |this| {
                this.hidden()
            })
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        div()
                            .text_sm()
                            .font_weight(open_gpui::FontWeight::BOLD)
                            .child("Toolbar"),
                    )
                    .child(
                        div().flex().gap_3().flex_wrap().children(
                            pages::components::toolbar_samples(snapshot.tokens)
                                .into_iter()
                                .map(|sample| {
                                    let state = sample.state.clone();
                                    let toolbar = sample.build_toolbar(snapshot.tokens);

                                    div()
                                        .id(format!("component-toolbar-sample:{}", sample.id))
                                        .debug_selector({
                                            let debug_selector = sample.debug_selector();
                                            move || debug_selector
                                        })
                                        .w(px(420.0))
                                        .flex()
                                        .flex_col()
                                        .gap_2()
                                        .rounded_sm()
                                        .border_1()
                                        .border_color(rgb(0xd6d8ce))
                                        .bg(rgb(0xffffff))
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
                                                        .child(state.label().to_owned()),
                                                )
                                                .child(label_pill(match state.orientation() {
                                                    Orientation::Horizontal => "horizontal",
                                                    Orientation::Vertical => "vertical",
                                                })),
                                        )
                                        .child(
                                            div()
                                                .text_xs()
                                                .text_color(rgb(0x5a6472))
                                                .child(sample.summary),
                                        )
                                        .child(toolbar)
                                        .child(component_toolbar_state_row(&state))
                                }),
                        ),
                    ),
            )
            .into_any_element(),
        "listbox" => component_page_section("listbox")
            .when(!show_component_section(focus_mode, "listbox"), |this| {
                this.hidden()
            })
            .child(component_listbox_samples_section(
                pages::components::listbox_samples(snapshot.tokens),
                snapshot.tokens,
            ))
            .into_any_element(),
        "select" => component_page_section("select")
            .when(!show_component_section(focus_mode, "select"), |this| {
                this.hidden()
            })
            .child(component_select_samples_section(
                pages::components::select_samples(snapshot.tokens),
                snapshot.tokens,
            ))
            .into_any_element(),
        "combobox" => component_page_section("combobox")
            .when(!show_component_section(focus_mode, "combobox"), |this| {
                this.hidden()
            })
            .child(component_combobox_samples_section(
                pages::components::combobox_samples(snapshot.tokens),
                snapshot.tokens,
            ))
            .into_any_element(),
        "command" => component_page_section("command")
            .when(!show_component_section(focus_mode, "command"), |this| {
                this.hidden()
            })
            .child(component_command_samples_section(
                pages::components::command_samples(snapshot.tokens),
                snapshot.tokens,
            ))
            .into_any_element(),
        "button" => component_page_section("button")
            .when(!show_component_section(focus_mode, "button"), |this| {
                this.hidden()
            })
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        div()
                            .text_sm()
                            .font_weight(open_gpui::FontWeight::BOLD)
                            .child("Button"),
                    )
                    .child(
                        div().flex().gap_3().flex_wrap().children(
                            pages::components::button_samples(snapshot.tokens)
                                .into_iter()
                                .map(|sample| {
                                    let sample_id = sample.id;
                                    let debug_selector = sample.debug_selector();
                                    let state = sample.state;
                                    div()
                                        .id(format!("component-button-sample:{sample_id}"))
                                        .debug_selector(move || debug_selector)
                                        .min_w(px(180.0))
                                        .flex()
                                        .flex_col()
                                        .gap_2()
                                        .rounded_sm()
                                        .border_1()
                                        .border_color(rgb(0xd6d8ce))
                                        .bg(rgb(0xffffff))
                                        .p_3()
                                        .child(
                                            Button::new(
                                                format!("component-button:{}", sample.id),
                                                sample.label,
                                            )
                                            .variant(state.variant())
                                            .with_size(state.size())
                                            .disabled(state.disabled())
                                            .selected(state.selected())
                                            .tokens(snapshot.tokens),
                                        )
                                        .child(component_button_state_row(state))
                                }),
                        ),
                    ),
            )
            .into_any_element(),
        "splitter" => {
            let motion_demo =
                window.use_keyed_state("component-splitter-motion-demo-runtime", cx, |_, cx| {
                    SplitterMotionDemo::new(cx)
                });

            component_page_section("splitter")
                .when(!show_component_section(focus_mode, "splitter"), |this| {
                    this.hidden()
                })
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_2()
                        .child(
                            div()
                                .text_sm()
                                .font_weight(open_gpui::FontWeight::BOLD)
                                .child("Splitter"),
                        )
                        .child(
                            div()
                                .flex()
                                .gap_3()
                                .flex_wrap()
                                .child(motion_demo)
                                .children(
                                    pages::components::splitter_samples(snapshot.tokens)
                                        .into_iter()
                                        .map(|sample| {
                                            let debug_selector = sample.debug_selector();
                                            let state = sample.state.clone();
                                            let splitter = sample.panels.into_iter().fold(
                                                Splitter::new(format!(
                                                    "component-splitter:{}",
                                                    sample.id
                                                ))
                                                .orientation(state.orientation())
                                                .with_size(state.size()),
                                                |splitter, panel| {
                                                    splitter.panel(SplitterPanel::new(
                                                        panel.descriptor,
                                                        div()
                                                            .size_full()
                                                            .flex()
                                                            .flex_col()
                                                            .gap_1()
                                                            .bg(rgb(0xf8f9f3))
                                                            .px_3()
                                                            .py_2()
                                                            .child(
                                                                div()
                                                                    .text_xs()
                                                                    .font_weight(
                                                                        open_gpui::FontWeight::BOLD,
                                                                    )
                                                                    .text_color(rgb(0x3f4a57))
                                                                    .child(panel.title),
                                                            )
                                                            .child(
                                                                div()
                                                                    .text_xs()
                                                                    .text_color(rgb(0x5a6472))
                                                                    .child(panel.body),
                                                            ),
                                                    ))
                                                },
                                            );

                                            div()
                                                .id(format!(
                                                    "component-splitter-sample:{}",
                                                    sample.id
                                                ))
                                                .debug_selector(move || debug_selector)
                                                .w(px(520.0))
                                                .flex()
                                                .flex_col()
                                                .gap_2()
                                                .rounded_sm()
                                                .border_1()
                                                .border_color(rgb(0xd6d8ce))
                                                .bg(rgb(0xffffff))
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
                                                                .font_weight(
                                                                    open_gpui::FontWeight::BOLD,
                                                                )
                                                                .child(sample.title),
                                                        )
                                                        .child(label_pill(
                                                            match state.orientation() {
                                                                Orientation::Horizontal => {
                                                                    "horizontal"
                                                                }
                                                                Orientation::Vertical => "vertical",
                                                            },
                                                        )),
                                                )
                                                .child(
                                                    div()
                                                        .text_xs()
                                                        .text_color(rgb(0x5a6472))
                                                        .child(sample.summary),
                                                )
                                                .child(
                                                    div()
                                                        .h(px(164.0))
                                                        .rounded_sm()
                                                        .border_1()
                                                        .border_color(rgb(0xe2e4dc))
                                                        .bg(rgb(0xfcfcf8))
                                                        .child(splitter),
                                                )
                                                .child(component_splitter_state_row(&state))
                                        }),
                                ),
                        ),
                )
                .into_any_element()
        }
        "scroll-area" => render_component_scroll_area_section(focus_mode, snapshot.tokens),
        "badge" => component_page_section("badge")
            .when(!show_component_section(focus_mode, "badge"), |this| {
                this.hidden()
            })
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        div()
                            .text_sm()
                            .font_weight(open_gpui::FontWeight::BOLD)
                            .child("Badge"),
                    )
                    .child(
                        div().flex().gap_3().flex_wrap().children(
                            pages::components::badge_samples(snapshot.tokens)
                                .into_iter()
                                .map(|sample| {
                                    let sample_id = sample.id;
                                    let debug_selector = sample.debug_selector();
                                    let state = sample.state;
                                    div()
                                        .id(format!("component-badge-sample:{sample_id}"))
                                        .debug_selector(move || debug_selector)
                                        .min_w(px(160.0))
                                        .flex()
                                        .flex_col()
                                        .items_start()
                                        .gap_2()
                                        .rounded_sm()
                                        .border_1()
                                        .border_color(rgb(0xd6d8ce))
                                        .bg(rgb(0xffffff))
                                        .p_3()
                                        .child(
                                            Badge::new(
                                                format!("component-badge:{}", sample.id),
                                                sample.label,
                                            )
                                            .variant(state.variant())
                                            .with_size(state.size())
                                            .tokens(snapshot.tokens),
                                        )
                                        .child(component_badge_state_row(state))
                                }),
                        ),
                    ),
            )
            .into_any_element(),
        "switch" => render_component_choice_sections(snapshot),
        "icon-button" => component_page_section("icon-button")
            .when(!show_component_section(focus_mode, "icon-button"), |this| {
                this.hidden()
            })
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        div()
                            .text_sm()
                            .font_weight(open_gpui::FontWeight::BOLD)
                            .child("IconButton"),
                    )
                    .child(
                        div().flex().gap_3().flex_wrap().children(
                            pages::components::icon_button_samples(snapshot.tokens)
                                .into_iter()
                                .map(|sample| {
                                    let sample_id = sample.id;
                                    let debug_selector = sample.debug_selector();
                                    let state = sample.state;
                                    let accessible_label = state.accessible_label().to_owned();
                                    div()
                                        .id(format!("component-icon-button-sample:{sample_id}"))
                                        .debug_selector(move || debug_selector)
                                        .min_w(px(170.0))
                                        .flex()
                                        .flex_col()
                                        .items_start()
                                        .gap_2()
                                        .rounded_sm()
                                        .border_1()
                                        .border_color(rgb(0xd6d8ce))
                                        .bg(rgb(0xffffff))
                                        .p_3()
                                        .child(
                                            IconButton::new(
                                                format!("component-icon-button:{}", sample.id),
                                                sample.icon,
                                                accessible_label.clone(),
                                            )
                                            .variant(state.variant())
                                            .disabled(state.disabled())
                                            .with_size(state.size())
                                            .tokens(snapshot.tokens),
                                        )
                                        .child(component_icon_button_state_row(
                                            &accessible_label,
                                            state,
                                        ))
                                }),
                        ),
                    ),
            )
            .into_any_element(),
        "label" => component_page_section("label")
            .when(!show_component_section(focus_mode, "label"), |this| {
                this.hidden()
            })
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        div()
                            .text_sm()
                            .font_weight(open_gpui::FontWeight::BOLD)
                            .child("Label"),
                    )
                    .child(
                        div().flex().gap_3().flex_wrap().children(
                            pages::components::label_samples(snapshot.tokens)
                                .into_iter()
                                .map(|sample| {
                                    let sample_id = sample.id;
                                    let debug_selector = sample.debug_selector();
                                    let state = sample.state.clone();
                                    div()
                                        .id(format!("component-label-sample:{sample_id}"))
                                        .debug_selector(move || debug_selector)
                                        .min_w(px(220.0))
                                        .flex()
                                        .flex_col()
                                        .gap_2()
                                        .rounded_sm()
                                        .border_1()
                                        .border_color(rgb(0xd6d8ce))
                                        .bg(rgb(0xffffff))
                                        .p_3()
                                        .child(component_label(
                                            format!("component-label:{}", sample.id),
                                            &state,
                                            snapshot.tokens,
                                        ))
                                        .child(component_label_state_row(&state))
                                }),
                        ),
                    ),
            )
            .into_any_element(),
        "text-input" => render_component_text_input_section(shell, focus_mode, snapshot.tokens),
        "textarea" => render_component_textarea_section(focus_mode, snapshot.tokens),
        "field" => {
            let focus_mode = match snapshot.components_focus {
                pages::components::ComponentFocusMode::All => {
                    pages::components::ComponentFocusMode::Section("field")
                }
                focus => focus,
            };
            component_page_section("field")
                            .when(show_component_section(focus_mode, "field"), |this| {
                                this.child(render_component_field_section(snapshot.tokens))
                            })
                            .when(show_component_section(focus_mode, "tabs"), |this| {
                                this.child(
                                component_page_section("tabs")
                                    .child(
                                        div()
                                            .flex()
                                            .flex_col()
                                            .gap_2()
                                            .child(
                                                div()
                                                    .text_sm()
                                                    .font_weight(open_gpui::FontWeight::BOLD)
                                                    .child("Tabs"),
                                            )
                                            .child(div().flex().gap_3().flex_wrap().children(
                                                pages::components::tabs_samples(snapshot.tokens)
                                                    .into_iter()
                                                    .map(|sample| {
                                                    let debug_selector = sample.debug_selector();
                                                    let state = sample.state.clone();
                                                    let tabs = sample.build_tabs(snapshot.tokens);

                                                    div()
                                                        .id(format!("component-tabs-sample:{}", sample.id))
                                                        .debug_selector(move || debug_selector)
                                                        .min_w(px(360.0))
                                                        .flex()
                                                        .flex_col()
                                                        .gap_2()
                                                        .rounded_sm()
                                                        .border_1()
                                                        .border_color(rgb(0xd6d8ce))
                                                        .bg(rgb(0xffffff))
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
                                                                        .font_weight(
                                                                            open_gpui::FontWeight::BOLD,
                                                                        )
                                                                        .child(sample.title),
                                                                )
                                                                .child(label_pill(
                                                                    state.activation_mode().as_str(),
                                                                )),
                                                        )
                                                        .child(
                                                            div()
                                                                .text_xs()
                                                                .text_color(rgb(0x5a6472))
                                                                .child(sample.summary),
                                                        )
                                                        .child(
                                                            div()
                                                                .when(
                                                                    state.orientation()
                                                                        == Orientation::Vertical,
                                                                    |this| this.h(px(240.0)).min_h(px(0.0)),
                                                                )
                                                                .overflow_hidden()
                                                                .child(tabs),
                                                        )
                                                        .child(component_tabs_state_row(&state))
                                                }),
                                            )),
                                    ),
                                )
                            })
                            .when(show_component_section(focus_mode, "table"), |this| {
                                this.child(
                                component_page_section("table")
                                    .child(
                                        div()
                                            .flex()
                                            .flex_col()
                                            .gap_2()
                                            .child(
                                                div()
                                                    .text_sm()
                                                    .font_weight(open_gpui::FontWeight::BOLD)
                                                    .child("Table"),
                                            )
                                            .child(div().flex().gap_3().flex_wrap().children(
                                                pages::components::table_samples(snapshot.tokens)
                                                    .into_iter()
                                                    .map(|sample| {
                                                    let sample_id = sample.id;
                                                    let debug_selector = sample.debug_selector();
                                                    let title = sample.title;
                                                    let summary = sample.summary;
                                                    let badge = sample.badge;
                                                    let base_sizing = sample.state.column_sizing().clone();
                                                    let current_sizing =
                                                        pages::components::current_table_sample_sizing(
                                                            sample_id,
                                                            &base_sizing,
                                                            cx,
                                                        );
                                                    let base_expansion = sample.state.expansion().clone();
                                                    let current_expansion =
                                                        pages::components::current_table_sample_expansion(
                                                            sample_id,
                                                            &base_expansion,
                                                            cx,
                                                        );
                                                    let table_state =
                                                        pages::components::table_sample_state_with_runtime(
                                                            sample,
                                                            current_sizing,
                                                            current_expansion,
                                                            cx,
                                                    );
                                                    let state_summary =
                                                        sample.state_summary_for_state(&table_state);
                                                    let mut table = sample.build_table_with_state(table_state);
                                                    let table_behavior = table
                                                        .behavior_snapshot(UiPx::ZERO, sample.viewport_extent);
                                                    let global_filter_control: Option<AnyElement> =
                                                        if sample_id == "filter-board" {
                                                            let query = table
                                                                .state()
                                                                .global_filter()
                                                                .unwrap_or("")
                                                                .to_owned();
                                                            let sample_id_for_global = sample_id.to_owned();
                                                            let base_state = sample.state.clone();

                                                            Some(
                                                                div()
                                                                    .w_full()
                                                                    .child(
                                                                        TableGlobalFilter::new(
                                                                            format!(
                                                                                "component-table-global-filter:{}",
                                                                                sample_id
                                                                            ),
                                                                            "Search",
                                                                        )
                                                                        .query(query)
                                                                        .placeholder("Search board rows")
                                                                        .clear_label("Clear search")
                                                                        .small()
                                                                        .tokens(snapshot.tokens)
                                                                        .on_change(move |change, _, cx| {
                                                                            pages::components::record_table_global_filter_change(
                                                                                sample_id_for_global.clone(),
                                                                                &base_state,
                                                                                &change,
                                                                                cx,
                                                                            );
                                                                        }),
                                                                    )
                                                                    .into_any_element(),
                                                            )
                                                        } else {
                                                            None
                                                        };
                                                    let predicate_filter_control: Option<AnyElement> =
                                                        if sample_id == "filter-board" {
                                                            let name_column = TableColumnId::new("name");
                                                            let (selected_operator, selected_value) = table
                                                                .state()
                                                                .filters()
                                                                .iter()
                                                                .find(|filter| filter.column() == &name_column)
                                                                .and_then(|filter| {
                                                                    filter.text_predicate().map(
                                                                        |(operator, query, _)| {
                                                                            (
                                                                                TablePredicateFilterOperator::text(
                                                                                    operator,
                                                                                ),
                                                                                query.to_owned(),
                                                                            )
                                                                        },
                                                                    )
                                                                })
                                                                .unwrap_or((
                                                                    TablePredicateFilterOperator::text(
                                                                        TableTextFilterOperator::Contains,
                                                                    ),
                                                                    String::new(),
                                                                ));
                                                            let sample_id_for_predicate = sample_id.to_owned();
                                                            let base_state = sample.state.clone();

                                                            Some(
                                                                div()
                                                                    .flex()
                                                                    .flex_col()
                                                                    .gap_1()
                                                                    .child(
                                                                        div()
                                                                            .text_xs()
                                                                            .font_weight(
                                                                                open_gpui::FontWeight::BOLD,
                                                                            )
                                                                            .text_color(rgb(0x3f4a57))
                                                                            .child("Name"),
                                                                    )
                                                                    .child(
                                                                        TablePredicateFilter::new(
                                                                            format!(
                                                                                "component-table-predicate-filter:{}:name",
                                                                                sample_id
                                                                            ),
                                                                            "Name",
                                                                            "name",
                                                                        )
                                                                        .default_operator(
                                                                            TablePredicateFilterOperator::text(
                                                                                TableTextFilterOperator::Contains,
                                                                            ),
                                                                        )
                                                                        .operator(selected_operator)
                                                                        .value(selected_value)
                                                                        .operators([
                                                                            TablePredicateFilterOperator::text(
                                                                                TableTextFilterOperator::Contains,
                                                                            ),
                                                                            TablePredicateFilterOperator::text(
                                                                                TableTextFilterOperator::StartsWith,
                                                                            ),
                                                                            TablePredicateFilterOperator::text(
                                                                                TableTextFilterOperator::EndsWith,
                                                                            ),
                                                                        ])
                                                                        .placeholder("Filter names")
                                                                        .clear_label("Clear name")
                                                                        .small()
                                                                        .tokens(snapshot.tokens)
                                                                        .on_change(move |change, _, cx| {
                                                                            pages::components::record_table_predicate_filter_change(
                                                                                sample_id_for_predicate.clone(),
                                                                                &base_state,
                                                                                &change,
                                                                                cx,
                                                                            );
                                                                        }),
                                                                    )
                                                                    .into_any_element(),
                                                            )
                                                        } else {
                                                            None
                                                        };
                                                    let faceted_filter_control: Option<AnyElement> =
                                                        if sample_id == "filter-board" {
                                                            let status_column = TableColumnId::new("status");
                                                            let selected_values = table
                                                                .state()
                                                                .filters()
                                                                .iter()
                                                                .find(|filter| filter.column() == &status_column)
                                                                .and_then(|filter| filter.selected_values())
                                                                .map(|values| {
                                                                    values.iter().cloned().collect::<Vec<_>>()
                                                                })
                                                                .unwrap_or_default();
                                                            table_behavior
                                                                .column_facet(&status_column)
                                                                .cloned()
                                                                .map(|facets| {
                                                                    let sample_id_for_filter =
                                                                        sample_id.to_owned();
                                                                    let base_state = sample.state.clone();
                                                                    div()
                                                                        .flex()
                                                                        .flex_col()
                                                                        .gap_1()
                                                                        .child(
                                                                            div()
                                                                                .text_xs()
                                                                                .font_weight(
                                                                                    open_gpui::FontWeight::BOLD,
                                                                                )
                                                                                .text_color(rgb(0x3f4a57))
                                                                                .child("Status"),
                                                                        )
                                                                        .child(
                                                                            TableFacetedFilter::new(
                                                                                format!(
                                                                                    "component-table-faceted-filter:{}:status",
                                                                                    sample_id
                                                                                ),
                                                                                "Status",
                                                                                "status",
                                                                            )
                                                                            .facets(facets)
                                                                            .selected_values(selected_values)
                                                                            .default_query("")
                                                                            .placeholder("Filter statuses")
                                                                            .empty_label("No matching statuses")
                                                                            .clear_label("Clear status")
                                                                            .small()
                                                                            .tokens(snapshot.tokens)
                                                                            .on_change(move |change, _, cx| {
                                                                                pages::components::record_table_faceted_filter_change(
                                                                                    sample_id_for_filter.clone(),
                                                                                    &base_state,
                                                                                    &change,
                                                                                    cx,
                                                                                );
                                                                            }),
                                                                        )
                                                                        .into_any_element()
                                                                })
                                                        } else {
                                                            None
                                                        };
                                                    let range_filter_control: Option<AnyElement> =
                                                        if sample_id == "filter-board" {
                                                            let score_column = TableColumnId::new("score");
                                                            let (selected_min, selected_max) = table
                                                                .state()
                                                                .filters()
                                                                .iter()
                                                                .find(|filter| filter.column() == &score_column)
                                                                .and_then(|filter| {
                                                                    filter.number_range_bounds()
                                                                })
                                                                .unwrap_or((None, None));
                                                            table_behavior
                                                                .column_facet(&score_column)
                                                                .cloned()
                                                                .map(|facets| {
                                                                    let sample_id_for_range =
                                                                        sample_id.to_owned();
                                                                    let base_state = sample.state.clone();
                                                                    div()
                                                                        .flex()
                                                                        .flex_col()
                                                                        .gap_1()
                                                                        .child(
                                                                            div()
                                                                                .text_xs()
                                                                                .font_weight(
                                                                                    open_gpui::FontWeight::BOLD,
                                                                                )
                                                                                .text_color(rgb(0x3f4a57))
                                                                                .child("Score"),
                                                                        )
                                                                        .child(
                                                                            TableRangeFilter::new(
                                                                                format!(
                                                                                    "component-table-range-filter:{}:score",
                                                                                    sample_id
                                                                                ),
                                                                                "Score",
                                                                                "score",
                                                                            )
                                                                            .facets(facets)
                                                                            .range(selected_min, selected_max)
                                                                            .clear_label("Clear score")
                                                                            .small()
                                                                            .tokens(snapshot.tokens)
                                                                            .on_change(move |change, _, cx| {
                                                                                pages::components::record_table_range_filter_change(
                                                                                    sample_id_for_range.clone(),
                                                                                    &base_state,
                                                                                    &change,
                                                                                    cx,
                                                                                );
                                                                            }),
                                                                        )
                                                                        .into_any_element()
                                                                })
                                                        } else {
                                                            None
                                                        };
                                                    let column_visibility_control: Option<AnyElement> =
                                                        if sample_id == "release-matrix" {
                                                            let sample_id_for_visibility =
                                                                sample_id.to_owned();
                                                            let base_state = table.state().clone();

                                                            Some(
                                                                TableColumnVisibility::new(
                                                                    format!(
                                                                        "component-table-column-visibility:{}",
                                                                        sample_id
                                                                    ),
                                                                    "Columns",
                                                                )
                                                                .columns(
                                                                    table
                                                                        .state()
                                                                        .columns()
                                                                        .iter()
                                                                        .cloned(),
                                                                )
                                                                .visibility(
                                                                    table
                                                                        .state()
                                                                        .column_visibility()
                                                                        .clone(),
                                                                )
                                                                .default_visibility(
                                                                    sample.state.column_visibility().clone(),
                                                                )
                                                                .show_all_label("Show all metrics")
                                                                .reset_label("Reset columns")
                                                                .viewport_item_count(7)
                                                                .small()
                                                                .tokens(snapshot.tokens)
                                                                .on_change(move |change, _, cx| {
                                                                    pages::components::record_table_column_visibility_change(
                                                                        sample_id_for_visibility.clone(),
                                                                        &base_state,
                                                                        &change,
                                                                        cx,
                                                                    );
                                                                })
                                                                .into_any_element(),
                                                            )
                                                        } else {
                                                            None
                                                        };
                                                    let table_toolbar: Option<AnyElement> =
                                                        if sample_id == "filter-board"
                                                            || sample_id == "release-matrix"
                                                        {
                                                            let mut has_toolbar_controls = false;
                                                            let toolbar_summary = if sample_id == "filter-board"
                                                            {
                                                                format!(
                                                                    "{} filtered / {} final rows",
                                                                    state_summary.filtered_rows,
                                                                    state_summary.final_rows
                                                                )
                                                            } else {
                                                                format!(
                                                                    "{} visible / {} total columns",
                                                                    state_summary.aria_columns,
                                                                    table.state().columns().len()
                                                                )
                                                            };
                                                            let mut toolbar = TableToolbar::new(
                                                                format!("component-table-toolbar:{sample_id}"),
                                                                if sample_id == "filter-board" {
                                                                    "Table filters"
                                                                } else {
                                                                    "Table columns"
                                                                },
                                                            )
                                                            .small()
                                                            .tokens(snapshot.tokens)
                                                            .summary(toolbar_summary);

                                                            if let Some(control) = global_filter_control {
                                                                has_toolbar_controls = true;
                                                                toolbar = toolbar.control(control);
                                                            }
                                                            if let Some(control) = predicate_filter_control {
                                                                has_toolbar_controls = true;
                                                                toolbar = toolbar.secondary_control(control);
                                                            }
                                                            if let Some(control) = faceted_filter_control {
                                                                has_toolbar_controls = true;
                                                                toolbar = toolbar.secondary_control(control);
                                                            }
                                                            if let Some(control) = range_filter_control {
                                                                has_toolbar_controls = true;
                                                                toolbar = toolbar.secondary_control(control);
                                                            }
                                                            if let Some(control) = column_visibility_control {
                                                                has_toolbar_controls = true;
                                                                toolbar = toolbar.control(control);
                                                            }

                                                            has_toolbar_controls
                                                                .then(|| toolbar.into_any_element())
                                                        } else {
                                                            None
                                                        };
                                                    if sample_id == "release-resize" {
                                                        let sample_id_for_resize = sample_id.to_owned();
                                                        table = table.on_column_sizing_change(
                                                            move |change, _, cx| {
                                                                pages::components::record_table_sizing_change(
                                                                    sample_id_for_resize.clone(),
                                                                    &change,
                                                                    cx,
                                                                );
                                                            },
                                                        );
                                                    }
                                                    if sample_id == "release-rollup" {
                                                        let sample_id_for_order = sample_id.to_owned();
                                                        let base_state_for_order = sample.state.clone();
                                                        table = table.on_column_order_change(
                                                            move |change, _, cx| {
                                                                pages::components::record_table_column_order_change(
                                                                    sample_id_for_order.clone(),
                                                                    &base_state_for_order,
                                                                    &change,
                                                                    cx,
                                                                );
                                                            },
                                                        );
                                                    }
                                                    let sample_id_for_activation = sample_id.to_owned();
                                                    table = table.on_row_activate(move |activation, _, cx| {
                                                        pages::components::record_table_row_activation(
                                                            sample_id_for_activation.clone(),
                                                            &activation,
                                                            cx,
                                                        );
                                                    });
                                                    let sample_id_for_expansion = sample_id.to_owned();
                                                    table =
                                                        table.on_row_expansion_request(move |toggle, _, cx| {
                                                            pages::components::record_table_expansion_request(
                                                                sample_id_for_expansion.clone(),
                                                                &base_expansion,
                                                                &toggle,
                                                                cx,
                                                            );
                                                        });
                                                    let sample_id_for_edit = sample_id.to_owned();
                                                    let base_state_for_edit = sample.state.clone();
                                                    table =
                                                        table.on_cell_edit_change(move |change, _, cx| {
                                                            pages::components::record_table_cell_edit_change(
                                                                sample_id_for_edit.clone(),
                                                                &base_state_for_edit,
                                                                &change,
                                                                cx,
                                                            );
                                                        });

                                                    div()
                                                        .id(format!("component-table-sample:{sample_id}"))
                                                        .debug_selector(move || debug_selector)
                                                        .w(px(560.0))
                                                        .flex_none()
                                                        .flex()
                                                        .flex_col()
                                                        .gap_2()
                                                        .rounded_sm()
                                                        .border_1()
                                                        .border_color(rgb(0xd6d8ce))
                                                        .bg(rgb(0xffffff))
                                                        .on_scroll_wheel(|_, _, _| {
                                                            open_gpui::ScrollWheelIntent::handled()
                                                                .stop_propagation()
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
                                                                        .font_weight(
                                                                            open_gpui::FontWeight::BOLD,
                                                                        )
                                                                        .child(title),
                                                                )
                                                                .child(label_pill(badge)),
                                                        )
                                                        .child(
                                                            div()
                                                                .text_xs()
                                                                .text_color(rgb(0x5a6472))
                                                                .child(summary),
                                                        )
                                                        .when_some(table_toolbar, |this, toolbar| {
                                                            this.child(toolbar)
                                                        })
                                                        .child(
                                                            div()
                                                                .h(px(228.0))
                                                                .min_h(px(0.0))
                                                                .overflow_hidden()
                                                                .rounded_sm()
                                                                .border_1()
                                                                .border_color(rgb(0xe2e4dc))
                                                                .bg(rgb(0xfcfcf8))
                                                                .child(table),
                                                        )
                                                        .child(component_table_state_row(&state_summary))
                                                }),
                                            )),
                                    ),
                                )
                            })
                            .when(show_component_section(focus_mode, "virtualized-list"), |this| {
                                this.child(
                                component_page_section("virtualized-list")
                                    .child(
                                        div()
                                            .flex()
                                            .flex_col()
                                            .gap_2()
                                            .child(
                                                div()
                                                    .text_sm()
                                                    .font_weight(open_gpui::FontWeight::BOLD)
                                                    .child("VirtualizedList"),
                                            )
                                            .child(div().flex().gap_3().flex_wrap().children(
                                                pages::components::virtualized_list_samples(snapshot.tokens)
                                                    .into_iter()
                                                    .map(|sample| {
                                                    let sample_id = sample.id;
                                                    let debug_selector = sample.debug_selector();
                                                    let title = sample.title;
                                                    let summary = sample.summary;
                                                    let badge = sample.badge;
                                                    let state = sample.state.clone();
                                                    let state_summary = sample.state_summary();
                                                    let sample_id_for_activation = sample_id.to_owned();
                                                    let mut list = sample.build_list();
                                                    if let Some(reveal_key) = sample.host_reveal_key {
                                                        let host_scroll_handle = window
                                                            .use_keyed_state(
                                                                format!(
                                                                    "component-virtualized-list-host-scroll:{sample_id}"
                                                                ),
                                                                cx,
                                                                |_, _| open_gpui::ScrollHandle::new(),
                                                            )
                                                            .read(cx)
                                                            .clone();
                                                        list = list
                                                            .scroll_handle(&host_scroll_handle)
                                                            .reveal_key(
                                                                reveal_key,
                                                                sample.host_reveal_strategy,
                                                            );
                                                    }
                                                    let list = list.on_activate(
                                                    move |activation, _, cx| {
                                                        pages::components::record_virtualized_list_activation(
                                                            sample_id_for_activation.clone(),
                                                            activation.index(),
                                                            activation.key().to_owned(),
                                                            activation.text_value().to_owned(),
                                                            cx,
                                                        );
                                                    },
                                                );

                                                    div()
                                                        .id(format!(
                                                            "component-virtualized-list-sample:{sample_id}"
                                                        ))
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
                                                            open_gpui::ScrollWheelIntent::handled()
                                                                .stop_propagation()
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
                                                                        .font_weight(
                                                                            open_gpui::FontWeight::BOLD,
                                                                        )
                                                                        .child(title),
                                                                )
                                                                .child(label_pill(badge)),
                                                        )
                                                        .child(
                                                            div()
                                                                .text_xs()
                                                                .text_color(rgb(0x5a6472))
                                                                .child(summary),
                                                        )
                                                        .child(
                                                            div()
                                                                .h(px(224.0))
                                                                .min_h(px(0.0))
                                                                .overflow_hidden()
                                                                .rounded_sm()
                                                                .border_1()
                                                                .border_color(rgb(0xe2e4dc))
                                                                .bg(rgb(0xfcfcf8))
                                                                .child(list),
                                                        )
                                                        .child(component_virtualized_list_state_row(
                                                            &state_summary,
                                                            &state,
                                                        ))
                                                }),
                                            )),
                                    ),
                                )
                            })
                            .when(show_component_section(focus_mode, "signals"), |this| {
                                this.child(
                                component_page_section("signals")
                                    .child(shell.render_signal_list(snapshot.selected_page)),
                                )
                            }).into_any_element()
        }
        _ => div().h_0().into_any_element(),
    }
}
