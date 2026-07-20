//! Focus/A11y Text/Form scenario rendering helpers for the foundation gallery shell.

use super::*;
const TEXTAREA_FIELD_ERROR_TOGGLE_ID: &str = "focus-a11y-field-error-toggle";

fn scenario_debug_selector(scenario: pages::focus_a11y::FocusA11yScenarioSpec) -> String {
    format!("gallery:focus-a11y-scenario:{}", scenario.id)
}

pub(super) fn render_focus_a11y_text_form_scenarios(
    shell: &GalleryShell,
    tokens: ThemeTokens,
    cx: &mut Context<GalleryShell>,
) -> impl IntoElement {
    let text_input_scenario = pages::focus_a11y::TEXT_INPUT_VALUE_SELECTION_SCENARIO;
    let field_scenario = pages::focus_a11y::TEXTAREA_FIELD_RELATIONS_SCENARIO;
    let password_scenario = pages::focus_a11y::PASSWORD_FREE_TEXT_REDACTION_SCENARIO;
    let story_state = shell.focus_a11y.text_form_story_state(tokens);
    let field_invalid = story_state.field().invalid();
    let entity = cx.entity().downgrade();

    let text_input_selector = scenario_debug_selector(text_input_scenario);
    let field_selector = scenario_debug_selector(field_scenario);
    let password_selector = scenario_debug_selector(password_scenario);

    let textarea = component_textarea(
        pages::focus_a11y::TEXTAREA_COMPONENT_ID.to_owned(),
        pages::focus_a11y::TEXTAREA_FIELD_LABEL,
        story_state.textarea(),
        tokens,
    )
    .on_change({
        let entity = entity.clone();
        move |value, _, cx| {
            entity
                .update(cx, |this, cx| {
                    this.mutate_focus_a11y(|state| state.set_textarea_value(value), cx)
                })
                .ok();
        }
    });
    let field = component_field(
        pages::focus_a11y::TEXTAREA_FIELD_COMPONENT_ID.to_owned(),
        story_state.field(),
        textarea,
        tokens,
    );

    div()
        .flex()
        .gap_3()
        .flex_wrap()
        .child(
            div()
                .id(format!("focus-a11y-scenario:{}", text_input_scenario.id))
                .debug_selector(move || text_input_selector)
                .w(px(300.0))
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
                        .text_sm()
                        .font_weight(open_gpui::FontWeight::BOLD)
                        .child("Account name"),
                )
                .child(
                    component_text_input(
                        pages::focus_a11y::TEXT_INPUT_COMPONENT_ID.to_owned(),
                        pages::focus_a11y::TEXT_INPUT_LABEL,
                        story_state.text_input(),
                        tokens,
                        None,
                    )
                    .on_change({
                        let entity = entity.clone();
                        move |value, _, cx| {
                            entity
                                .update(cx, |this, cx| {
                                    this.mutate_focus_a11y(
                                        |state| state.set_text_input_value(value),
                                        cx,
                                    )
                                })
                                .ok();
                        }
                    }),
                ),
        )
        .child(
            div()
                .id(format!("focus-a11y-scenario:{}", field_scenario.id))
                .debug_selector(move || field_selector)
                .w(px(340.0))
                .flex()
                .flex_col()
                .gap_2()
                .rounded_sm()
                .border_1()
                .border_color(rgb(0xd6d8ce))
                .bg(rgb(0xffffff))
                .p_3()
                .child(field)
                .child(
                    div()
                        .debug_selector(move || {
                            pages::focus_a11y::TEXTAREA_FIELD_ERROR_TOGGLE_SELECTOR.into()
                        })
                        .child(
                            Button::new(
                                TEXTAREA_FIELD_ERROR_TOGGLE_ID,
                                if field_invalid {
                                    "Clear error"
                                } else {
                                    "Show error"
                                },
                            )
                            .variant(ButtonVariant::Secondary)
                            .with_size(Size::Small)
                            .tokens(tokens)
                            .on_activate(cx.processor(
                                |this, _, _, cx| {
                                    this.mutate_focus_a11y(
                                        |state| state.toggle_field_invalid(),
                                        cx,
                                    );
                                },
                            )),
                        ),
                ),
        )
        .child(
            div()
                .id(format!("focus-a11y-scenario:{}", password_scenario.id))
                .debug_selector(move || password_selector)
                .w(px(300.0))
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
                        .text_sm()
                        .font_weight(open_gpui::FontWeight::BOLD)
                        .child("Password"),
                )
                .child(
                    component_text_input(
                        pages::focus_a11y::PASSWORD_COMPONENT_ID.to_owned(),
                        pages::focus_a11y::PASSWORD_LABEL,
                        story_state.password(),
                        tokens,
                        None,
                    )
                    .on_change({
                        let entity = entity.clone();
                        move |value, _, cx| {
                            entity
                                .update(cx, |this, cx| {
                                    this.mutate_focus_a11y(
                                        |state| state.set_password_value(value),
                                        cx,
                                    )
                                })
                                .ok();
                        }
                    }),
                ),
        )
}

pub(super) fn render_focus_a11y_live_region_scenario(
    shell: &GalleryShell,
    tokens: ThemeTokens,
    cx: &mut Context<GalleryShell>,
) -> impl IntoElement {
    let scenario = pages::focus_a11y::LIVE_REGIONS_AND_ANNOUNCEMENTS_SCENARIO;
    let scenario_selector = scenario_debug_selector(scenario);
    let status_text = shell.focus_a11y.live_status_text();
    let status_busy = shell.focus_a11y.live_busy();
    let alert_visible = shell.focus_a11y.live_alert_visible();

    div()
        .id(format!("focus-a11y-scenario:{}", scenario.id))
        .debug_selector(move || scenario_selector)
        .flex()
        .flex_col()
        .gap_3()
        .rounded_sm()
        .border_1()
        .border_color(rgb(0xd6d8ce))
        .bg(rgb(0xffffff))
        .p_3()
        .child(
            div()
                .text_sm()
                .font_weight(open_gpui::FontWeight::BOLD)
                .child("Live regions"),
        )
        .child(
            StatusCue::new(pages::focus_a11y::LIVE_STATUS_COMPONENT_ID, status_text)
                .intent(FeedbackIntent::Info)
                .live(shell.focus_a11y.live_status_priority())
                .busy(status_busy)
                .tokens(tokens),
        )
        .when(alert_visible, |this| {
            this.child(
                StatusCue::new(
                    pages::focus_a11y::LIVE_ALERT_COMPONENT_ID,
                    pages::focus_a11y::LIVE_ALERT_TEXT,
                )
                .intent(FeedbackIntent::Danger)
                .tokens(tokens),
            )
        })
        .child(
            div()
                .flex()
                .flex_wrap()
                .gap_2()
                .child(
                    Button::new(
                        pages::focus_a11y::LIVE_STATUS_UPDATE_CONTROL_ID,
                        "Update status",
                    )
                    .variant(ButtonVariant::Secondary)
                    .with_size(Size::Small)
                    .tokens(tokens)
                    .on_activate(cx.processor(|this, _, _, cx| {
                        this.mutate_focus_a11y(|state| state.update_live_status(), cx);
                    })),
                )
                .child(
                    Button::new(
                        pages::focus_a11y::LIVE_BUSY_TOGGLE_CONTROL_ID,
                        if status_busy {
                            "Finish batch"
                        } else {
                            "Start batch"
                        },
                    )
                    .variant(ButtonVariant::Secondary)
                    .with_size(Size::Small)
                    .tokens(tokens)
                    .on_activate(cx.processor(|this, _, _, cx| {
                        this.mutate_focus_a11y(|state| state.toggle_live_busy(), cx);
                    })),
                )
                .child(
                    Button::new(
                        pages::focus_a11y::LIVE_ALERT_TOGGLE_CONTROL_ID,
                        if alert_visible {
                            "Clear alert"
                        } else {
                            "Show alert"
                        },
                    )
                    .variant(ButtonVariant::Secondary)
                    .with_size(Size::Small)
                    .tokens(tokens)
                    .on_activate(cx.processor(|this, _, _, cx| {
                        this.mutate_focus_a11y(|state| state.toggle_live_alert(), cx);
                    })),
                )
                .child(
                    Button::new(
                        pages::focus_a11y::WINDOW_ANNOUNCEMENT_CONTROL_ID,
                        "Announce completion",
                    )
                    .variant(ButtonVariant::Secondary)
                    .with_size(Size::Small)
                    .tokens(tokens)
                    .on_activate(cx.processor(|_, _, window, cx| {
                        let _ = window.announce(
                            AccessibilityAnnouncement::polite(
                                pages::focus_a11y::WINDOW_ANNOUNCEMENT_TEXT,
                            ),
                            cx,
                        );
                    })),
                ),
        )
}
