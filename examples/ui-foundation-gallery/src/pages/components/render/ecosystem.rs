use super::*;

pub(super) fn render_component_ecosystem_adapters_section(tokens: ThemeTokens) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .gap_3()
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
                        .child("Ecosystem adapters"),
                )
                .child(label_pill("headless state")),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .gap_2()
                .child(
                    div()
                        .text_xs()
                        .font_weight(open_gpui::FontWeight::BOLD)
                        .text_color(rgb(0x3f4a57))
                        .child("Form adapters"),
                )
                .child(
                    div().flex().gap_3().flex_wrap().children(
                        pages::components::form_adapter_samples(tokens)
                            .into_iter()
                            .map(|sample| render_form_adapter_sample(sample, tokens)),
                    ),
                ),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .gap_2()
                .child(
                    div()
                        .text_xs()
                        .font_weight(open_gpui::FontWeight::BOLD)
                        .text_color(rgb(0x3f4a57))
                        .child("Resource adapters"),
                )
                .child(
                    div().flex().gap_3().flex_wrap().children(
                        pages::components::resource_adapter_samples(tokens)
                            .into_iter()
                            .map(|sample| render_resource_adapter_sample(sample, tokens)),
                    ),
                ),
        )
        .child(render_ecosystem_runtime_readout())
}

fn render_form_adapter_sample(
    sample: pages::components::FormAdapterSample,
    tokens: ThemeTokens,
) -> impl IntoElement {
    let sample_id = sample.id;
    let debug_selector = sample.debug_selector();
    let email_field = sample.email_field.clone();
    let email_input = sample.email_input.clone();
    let notes_field = sample.notes_field.clone();
    let notes_textarea = sample.notes_textarea.clone();
    let seats_input = sample.seats_input.clone();
    let alerts_checkbox = sample.alerts_checkbox;
    let status = form_status_label(sample.form.status());

    component_gallery_card_shell(
        format!("component-form-adapter-sample:{sample_id}"),
        Some(debug_selector),
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
            .child(label_pill(status)),
    )
    .child(
        div()
            .text_xs()
            .text_color(rgb(0x5a6472))
            .child(sample.summary),
    )
    .child(component_field(
        format!("component-form-adapter-email-field:{sample_id}"),
        &email_field,
        component_text_input(
            format!("component-form-adapter-email-input:{sample_id}"),
            email_field.label(),
            &email_input,
            tokens,
            None,
        ),
        tokens,
    ))
    .child(component_field_state_row(&email_field, &email_input))
    .child(component_field(
        format!("component-form-adapter-notes-field:{sample_id}"),
        &notes_field,
        component_textarea(
            format!("component-form-adapter-notes-textarea:{sample_id}"),
            notes_field.label(),
            &notes_textarea,
            tokens,
        ),
        tokens,
    ))
    .child(component_field_textarea_state_row(
        &notes_field,
        &notes_textarea,
    ))
    .child(
        div()
            .grid()
            .grid_cols(2)
            .gap_2()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(
                        NumberInput::new(
                            format!("component-form-adapter-seats:{sample_id}"),
                            seats_input.label(),
                        )
                        .value(seats_input.value())
                        .min(seats_input.min())
                        .max(seats_input.max())
                        .step(seats_input.step())
                        .disabled(seats_input.disabled())
                        .read_only(seats_input.read_only())
                        .invalid(seats_input.invalid())
                        .required(seats_input.required())
                        .busy(seats_input.busy())
                        .with_size(seats_input.size())
                        .tokens(tokens),
                    )
                    .child(component_number_input_state_row(&seats_input)),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(component_checkbox(
                        format!("component-form-adapter-alerts:{sample_id}"),
                        "Alerts",
                        alerts_checkbox,
                        tokens,
                    ))
                    .child(component_checkbox_state_row(alerts_checkbox)),
            ),
    )
    .child(form_adapter_state_row(&sample))
}

fn render_resource_adapter_sample(
    sample: pages::components::ResourceAdapterSample,
    tokens: ThemeTokens,
) -> impl IntoElement {
    let sample_id = sample.id;
    let debug_selector = sample.debug_selector();
    let mutation_cue = sample
        .mutation
        .as_ref()
        .and_then(|mutation| mutation.status_cue_state(tokens));
    let list = VirtualizedList::from_shared_items(
        format!("component-resource-adapter-list:{sample_id}"),
        sample.title,
        sample.virtualized_items.clone(),
    )
    .with_size(Size::Small)
    .row_height(open_gpui_ui_core::ui_px(30.0))
    .viewport_item_count(4)
    .tokens(tokens);

    component_gallery_card_shell(
        format!("component-resource-adapter-sample:{sample_id}"),
        Some(debug_selector),
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
            .child(label_pill(sample.badge)),
    )
    .child(
        div()
            .text_xs()
            .text_color(rgb(0x5a6472))
            .child(sample.summary),
    )
    .when_some(sample.status_cue.clone(), |this, state| {
        this.child(
            StatusCue::new(
                format!("component-resource-adapter-cue:{sample_id}"),
                state.label(),
            )
            .intent(state.intent())
            .with_size(state.size())
            .tokens(tokens),
        )
    })
    .when_some(sample.empty_state.clone(), |this, state| {
        let empty = EmptyState::new(
            format!("component-resource-adapter-empty:{sample_id}"),
            state.title(),
        )
        .intent(state.intent())
        .with_size(state.size())
        .tokens(tokens);
        let empty = if let Some(description) = state.description() {
            empty.description(description)
        } else {
            empty
        };
        this.child(empty)
    })
    .when_some(mutation_cue, |this, state| {
        this.child(
            StatusCue::new(
                format!("component-resource-adapter-mutation:{sample_id}"),
                state.label(),
            )
            .intent(state.intent())
            .with_size(state.size())
            .tokens(tokens),
        )
    })
    .child(
        div()
            .h(px(154.0))
            .min_h(px(0.0))
            .overflow_hidden()
            .rounded_sm()
            .border_1()
            .border_color(rgb(0xe2e4dc))
            .bg(rgb(0xfcfcf8))
            .child(list),
    )
    .child(resource_adapter_state_row(&sample))
}

fn form_adapter_state_row(sample: &pages::components::FormAdapterSample) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .gap_1()
        .text_xs()
        .text_color(rgb(0x5a6472))
        .child(format!(
            "{} fields / {} validating / {} invalid / {} dirty / {} redacted",
            sample.field_count,
            sample.validating_field_count,
            sample.invalid_field_count,
            sample.dirty_field_count,
            sample.redacted_field_count
        ))
        .child(format!(
            "form busy {} / validating {} / submitting {} / submit enabled {}",
            sample.form.busy(),
            sample.form.validating(),
            sample.form.submitting(),
            sample.form.submit_enabled()
        ))
        .child(format!("select workspace.region = {}", sample.region_value))
}

fn resource_adapter_state_row(
    sample: &pages::components::ResourceAdapterSample,
) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .gap_1()
        .text_xs()
        .text_color(rgb(0x5a6472))
        .child(format!(
            "{} / loading {} / stale {} / refreshing {} / retry {}",
            resource_status_label(sample.collection.status()),
            sample.collection.loading(),
            sample.collection.stale(),
            sample.collection.refreshing(),
            sample.collection.retryable()
        ))
        .child(format!(
            "command loading {} / status {}",
            sample.command_loading_message.as_deref().unwrap_or("none"),
            sample.command_status_message.as_deref().unwrap_or("none")
        ))
        .child(format!(
            "table children {} / tree children {} / rows {}",
            table_children_state_label(&sample.table_children_state),
            sample.tree_children_state.as_str(),
            sample.virtualized_items.len()
        ))
}

fn render_ecosystem_runtime_readout() -> impl IntoElement {
    let form_log = pages::components::form_sample_runtime_log();
    let resource_log = pages::components::resource_sample_runtime_log();
    let form_status = form_log
        .entries()
        .last()
        .map(|event| form_status_label(&event.status))
        .unwrap_or("empty");

    div()
        .rounded_sm()
        .border_1()
        .border_color(rgb(0xe2e4dc))
        .bg(rgb(0xfcfcf8))
        .p_3()
        .flex()
        .flex_col()
        .gap_1()
        .text_xs()
        .text_color(rgb(0x5a6472))
        .child(format!(
            "form runtime {} events / final {form_status}",
            form_log.len()
        ))
        .child(format!("resource runtime {} events", resource_log.len()))
}

fn form_status_label(status: &open_gpui_form::FormStatus) -> &'static str {
    match status {
        open_gpui_form::FormStatus::Idle => "idle",
        open_gpui_form::FormStatus::Validating => "validating",
        open_gpui_form::FormStatus::Submitting => "submitting",
        open_gpui_form::FormStatus::Submitted => "submitted",
        open_gpui_form::FormStatus::SubmitFailed => "submit-failed",
    }
}

fn resource_status_label(status: &open_gpui_resource::ResourceStatus) -> &'static str {
    match status {
        open_gpui_resource::ResourceStatus::Idle => "idle",
        open_gpui_resource::ResourceStatus::Loading => "loading",
        open_gpui_resource::ResourceStatus::Success => "success",
        open_gpui_resource::ResourceStatus::Stale => "stale",
        open_gpui_resource::ResourceStatus::Refetching => "refetching",
        open_gpui_resource::ResourceStatus::Error => "error",
    }
}

fn table_children_state_label(
    state: &open_gpui_ui_core::TableRowChildrenLoadState,
) -> &'static str {
    match state {
        open_gpui_ui_core::TableRowChildrenLoadState::Idle => "idle",
        open_gpui_ui_core::TableRowChildrenLoadState::Loading { .. } => "loading",
        open_gpui_ui_core::TableRowChildrenLoadState::Failed { .. } => "failed",
    }
}
