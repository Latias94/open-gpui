use super::*;

pub(super) fn render_component_choice_sections(snapshot: GalleryShellSnapshot) -> AnyElement {
    let focus_mode = match snapshot.components_focus {
        pages::components::ComponentFocusMode::All => {
            pages::components::ComponentFocusMode::Section("switch")
        }
        focus => focus,
    };

    component_page_section("switch")
        .when(show_component_section(focus_mode, "switch"), |this| {
            this.child(render_switch_section(snapshot.tokens))
        })
        .when(show_component_section(focus_mode, "checkbox"), |this| {
            this.child(render_checkbox_section(snapshot.tokens))
        })
        .when(show_component_section(focus_mode, "radio-group"), |this| {
            this.child(render_radio_group_section(snapshot.tokens))
        })
        .when(show_component_section(focus_mode, "toggle"), |this| {
            this.child(render_toggle_section(snapshot.tokens))
        })
        .into_any_element()
}

fn render_switch_section(tokens: ThemeTokens) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .gap_2()
        .child(
            div()
                .text_sm()
                .font_weight(open_gpui::FontWeight::BOLD)
                .child("Switch"),
        )
        .child(
            div().flex().gap_3().flex_wrap().children(
                pages::components::switch_samples(tokens)
                    .into_iter()
                    .map(|sample| {
                        let sample_id = sample.id;
                        let debug_selector = sample.debug_selector();
                        let state = sample.state;
                        div()
                            .id(format!("component-switch-sample:{sample_id}"))
                            .debug_selector(move || debug_selector)
                            .min_w(px(200.0))
                            .flex()
                            .flex_col()
                            .gap_2()
                            .rounded_sm()
                            .border_1()
                            .border_color(rgb(0xd6d8ce))
                            .bg(rgb(0xffffff))
                            .p_3()
                            .child(
                                Switch::new(format!("component-switch:{}", sample.id))
                                    .label(sample.label)
                                    .checked(state.checked())
                                    .disabled(state.disabled())
                                    .with_size(state.size())
                                    .tokens(tokens),
                            )
                            .child(component_switch_state_row(state))
                    }),
            ),
        )
}

fn render_checkbox_section(tokens: ThemeTokens) -> impl IntoElement {
    component_page_section("checkbox").child(
        div()
            .flex()
            .flex_col()
            .gap_2()
            .child(
                div()
                    .text_sm()
                    .font_weight(open_gpui::FontWeight::BOLD)
                    .child("Checkbox"),
            )
            .child(
                div().flex().gap_3().flex_wrap().children(
                    pages::components::checkbox_samples(tokens)
                        .into_iter()
                        .map(|sample| {
                            let sample_id = sample.id;
                            let debug_selector = sample.debug_selector();
                            let state = sample.state;
                            div()
                                .id(format!("component-checkbox-sample:{sample_id}"))
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
                                .child(component_checkbox(
                                    format!("component-checkbox:{}", sample.id),
                                    sample.label,
                                    state,
                                    tokens,
                                ))
                                .child(component_checkbox_state_row(state))
                        }),
                ),
            ),
    )
}

fn render_radio_group_section(tokens: ThemeTokens) -> impl IntoElement {
    component_page_section("radio-group").child(
        div()
            .flex()
            .flex_col()
            .gap_2()
            .child(
                div()
                    .text_sm()
                    .font_weight(open_gpui::FontWeight::BOLD)
                    .child("RadioGroup"),
            )
            .child(
                div().flex().gap_3().flex_wrap().children(
                    pages::components::radio_group_samples(tokens)
                        .into_iter()
                        .map(|sample| {
                            let sample_id = sample.id;
                            let debug_selector = sample.debug_selector();
                            let state = sample.state.clone();
                            let mut radio =
                                RadioGroup::new(format!("component-radio:{}", sample.id))
                                    .label(sample.title)
                                    .orientation(state.orientation())
                                    .default_selected(state.selected_value().unwrap_or("none"))
                                    .required(state.required())
                                    .disabled(state.disabled())
                                    .with_size(state.size())
                                    .tokens(tokens);
                            for item in state.items().iter() {
                                radio = radio.item(
                                    RadioItem::new(item.value(), item.label())
                                        .disabled(item.disabled()),
                                );
                            }

                            div()
                                .id(format!("component-radio-sample:{sample_id}"))
                                .debug_selector(move || debug_selector)
                                .min_w(px(240.0))
                                .flex()
                                .flex_col()
                                .gap_2()
                                .rounded_sm()
                                .border_1()
                                .border_color(rgb(0xd6d8ce))
                                .bg(rgb(0xffffff))
                                .p_3()
                                .child(radio)
                                .child(component_radio_state_row(&state))
                        }),
                ),
            ),
    )
}

fn render_toggle_section(tokens: ThemeTokens) -> impl IntoElement {
    component_page_section("toggle").child(
        div()
            .flex()
            .flex_col()
            .gap_2()
            .child(
                div()
                    .text_sm()
                    .font_weight(open_gpui::FontWeight::BOLD)
                    .child("Toggle"),
            )
            .child(
                div().flex().gap_3().flex_wrap().children(
                    pages::components::toggle_samples(tokens)
                        .into_iter()
                        .map(|sample| {
                            let sample_id = sample.id;
                            let debug_selector = sample.debug_selector();
                            let state = sample.state;
                            div()
                                .id(format!("component-toggle-sample:{sample_id}"))
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
                                    Toggle::new(
                                        format!("component-toggle:{}", sample.id),
                                        sample.label,
                                    )
                                    .variant(state.variant())
                                    .pressed(state.pressed())
                                    .disabled(state.disabled())
                                    .with_size(state.size())
                                    .tokens(tokens),
                                )
                                .child(component_toggle_state_row(&state))
                        }),
                ),
            ),
    )
}
