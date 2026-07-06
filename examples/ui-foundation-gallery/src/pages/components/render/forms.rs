use super::*;

pub(super) fn render_component_text_input_section(
    shell: &mut GalleryShell,
    focus_mode: pages::components::ComponentFocusMode,
    tokens: ThemeTokens,
) -> AnyElement {
    component_page_section("text-input")
        .when(!show_component_section(focus_mode, "text-input"), |this| {
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
                        .child("TextInput"),
                )
                .child(
                    div().flex().gap_3().flex_wrap().children(
                        pages::components::text_input_samples(tokens)
                            .into_iter()
                            .map(|sample| {
                                let sample_id = sample.id;
                                let debug_selector = sample.debug_selector();
                                let state = sample.state.clone();
                                let controller = state
                                    .controller_driven()
                                    .then(|| shell.editable_text_input().clone());
                                div()
                                    .id(format!("component-text-input-sample:{sample_id}"))
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
                                    .child(
                                        div()
                                            .text_xs()
                                            .font_weight(open_gpui::FontWeight::BOLD)
                                            .text_color(rgb(0x3f4a57))
                                            .child(sample.label),
                                    )
                                    .child(component_text_input(
                                        format!("component-text-input:{}", sample.id),
                                        sample.label,
                                        &state,
                                        tokens,
                                        controller,
                                    ))
                                    .child(component_text_input_state_row(&state))
                            }),
                    ),
                ),
        )
        .into_any_element()
}

pub(super) fn render_component_textarea_section(
    focus_mode: pages::components::ComponentFocusMode,
    tokens: ThemeTokens,
) -> AnyElement {
    component_page_section("textarea")
        .when(!show_component_section(focus_mode, "textarea"), |this| {
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
                        .child("Textarea"),
                )
                .child(
                    div().flex().gap_3().flex_wrap().children(
                        pages::components::textarea_samples(tokens)
                            .into_iter()
                            .map(|sample| {
                                let sample_id = sample.id;
                                let debug_selector = sample.debug_selector();
                                let state = sample.state.clone();
                                div()
                                    .id(format!("component-textarea-sample:{sample_id}"))
                                    .debug_selector(move || debug_selector)
                                    .min_w(px(280.0))
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
                                            .text_xs()
                                            .font_weight(open_gpui::FontWeight::BOLD)
                                            .text_color(rgb(0x3f4a57))
                                            .child(sample.label),
                                    )
                                    .child(component_textarea(
                                        format!("component-textarea:{}", sample.id),
                                        sample.label,
                                        &state,
                                        tokens,
                                    ))
                                    .child(component_textarea_state_row(&state))
                            }),
                    ),
                ),
        )
        .into_any_element()
}

pub(super) fn render_component_field_section(tokens: ThemeTokens) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .gap_2()
        .child(
            div()
                .text_sm()
                .font_weight(open_gpui::FontWeight::BOLD)
                .child("Field"),
        )
        .child(
            div().flex().gap_3().flex_wrap().children(
                pages::components::field_samples(tokens)
                    .into_iter()
                    .map(|sample| {
                        let sample_id = sample.id;
                        let debug_selector = sample.debug_selector();
                        let field_state = sample.state.clone();
                        let input_state = sample.input_state.clone();
                        div()
                            .id(format!("component-field-sample:{sample_id}"))
                            .debug_selector(move || debug_selector)
                            .min_w(px(280.0))
                            .flex()
                            .flex_col()
                            .gap_2()
                            .rounded_sm()
                            .border_1()
                            .border_color(rgb(0xd6d8ce))
                            .bg(rgb(0xffffff))
                            .p_3()
                            .child(component_field(
                                format!("component-field:{}", sample.id),
                                &field_state,
                                component_text_input(
                                    format!("component-field-input:{}", sample.id),
                                    field_state.label(),
                                    &input_state,
                                    tokens,
                                    None,
                                ),
                                tokens,
                            ))
                            .child(component_field_state_row(&field_state, &input_state))
                    }),
            ),
        )
        .child(
            div().flex().gap_3().flex_wrap().children(
                pages::components::field_textarea_samples(tokens)
                    .into_iter()
                    .map(|sample| {
                        let sample_id = sample.id;
                        let debug_selector = sample.debug_selector();
                        let field_state = sample.state.clone();
                        let textarea_state = sample.textarea_state.clone();
                        div()
                            .id(format!("component-field-textarea-sample:{sample_id}"))
                            .debug_selector(move || debug_selector)
                            .min_w(px(320.0))
                            .flex()
                            .flex_col()
                            .gap_2()
                            .rounded_sm()
                            .border_1()
                            .border_color(rgb(0xd6d8ce))
                            .bg(rgb(0xffffff))
                            .p_3()
                            .child(component_field(
                                format!("component-field-textarea:{}", sample.id),
                                &field_state,
                                component_textarea(
                                    format!("component-field-textarea-control:{}", sample.id),
                                    field_state.label(),
                                    &textarea_state,
                                    tokens,
                                ),
                                tokens,
                            ))
                            .child(component_field_textarea_state_row(
                                &field_state,
                                &textarea_state,
                            ))
                    }),
            ),
        )
}
