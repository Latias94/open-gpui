//! Components page sample rendering helpers for the foundation gallery shell.

use super::support::{format_px, gallery_card_shell, label_pill, toggled_label_text};
use super::*;

pub(crate) fn component_button_state_row(state: ButtonState) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .gap_1()
        .text_xs()
        .text_color(rgb(0x5a6472))
        .child(format!(
            "{} / {} / {}",
            state.variant().as_str(),
            state.size().as_str(),
            if state.activation_enabled() {
                "enabled"
            } else {
                "disabled"
            }
        ))
        .child(format!(
            "h {} px {}",
            format_px(state.metrics().height()),
            format_px(state.metrics().padding_x())
        ))
}

pub(crate) fn component_badge_state_row(state: BadgeState) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .gap_1()
        .text_xs()
        .text_color(rgb(0x5a6472))
        .child(format!(
            "{} / {} / display",
            state.variant().as_str(),
            state.size().as_str()
        ))
        .child(format!(
            "h {} px {}",
            format_px(state.metrics().min_height()),
            format_px(state.metrics().padding_x())
        ))
}

pub(crate) fn component_separator_state_row(state: SeparatorState) -> impl IntoElement {
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
            if state.decorative() {
                "decorative"
            } else {
                "semantic"
            },
            state.size().as_str()
        ))
        .child(format!(
            "role {} / thickness {}",
            state
                .role()
                .map(|role| format!("{role:?}"))
                .unwrap_or_else(|| "none".to_owned()),
            format_px(state.metrics().thickness())
        ))
}

pub(crate) fn component_kbd_state_row(state: KbdState) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .gap_1()
        .text_xs()
        .text_color(rgb(0x5a6472))
        .child(format!("{} / {}", state.label(), state.size().as_str()))
        .child(format!(
            "min {} px {}",
            format_px(state.metrics().min_width()),
            format_px(state.metrics().padding_x())
        ))
}

pub(crate) fn component_progress_state_row(state: ProgressState) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .gap_1()
        .text_xs()
        .text_color(rgb(0x5a6472))
        .child(format!(
            "{:?} / {} / {}",
            state.role(),
            state.size().as_str(),
            if state.indeterminate() {
                "indeterminate".to_owned()
            } else {
                format!("{:.0}%", state.value_percent().unwrap_or(0.0))
            }
        ))
        .child(format!(
            "h {} radius {}",
            format_px(state.metrics().height()),
            format_px(state.metrics().radius())
        ))
        .child(format!(
            "indicator start {:.0}% width {:.0}%",
            state.indicator_start_fraction() * 100.0,
            state.indicator_fraction() * 100.0
        ))
}

pub(crate) fn component_skeleton_state_row(state: SkeletonState) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .gap_1()
        .text_xs()
        .text_color(rgb(0x5a6472))
        .child(format!(
            "{} / {}",
            state.size().as_str(),
            if state.subtle() { "subtle" } else { "default" }
        ))
        .child(format!(
            "{} x {} / radius {}",
            format_px(state.metrics().width()),
            format_px(state.metrics().height()),
            format_px(state.metrics().radius())
        ))
}

pub(crate) fn component_avatar_state_row(state: &AvatarState) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .gap_1()
        .text_xs()
        .text_color(rgb(0x5a6472))
        .child(format!(
            "{} / fallback {} / {}",
            state.size().as_str(),
            state.fallback(),
            if state.has_source() {
                "source"
            } else {
                "fallback"
            }
        ))
        .child(format!(
            "{:?} / aria {} / box {}",
            state.role(),
            state.accessible_label(),
            format_px(state.metrics().diameter())
        ))
}

pub(crate) fn component_icon_button_state_row(
    accessible_label: &str,

    state: IconButtonState,
) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .gap_1()
        .text_xs()
        .text_color(rgb(0x5a6472))
        .child(format!(
            "{} / {} / {}",
            state.variant().as_str(),
            state.size().as_str(),
            if state.activation_enabled() {
                "enabled"
            } else {
                "disabled"
            }
        ))
        .child(format!(
            "box {} icon {}",
            format_px(state.metrics().size()),
            format_px(state.metrics().icon_size())
        ))
        .child(format!("aria {}", accessible_label))
}

pub(crate) fn component_switch_state_row(state: SwitchState) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .gap_1()
        .text_xs()
        .text_color(rgb(0x5a6472))
        .child(format!(
            "{} / {} / {}",
            toggled_label_text(state.toggled()),
            state.size().as_str(),
            if state.activation_enabled() {
                "enabled"
            } else {
                "disabled"
            }
        ))
        .child(format!(
            "{} x {} / thumb {}",
            format_px(state.metrics().track_width()),
            format_px(state.metrics().track_height()),
            format_px(state.metrics().thumb_size())
        ))
}

pub(crate) fn component_checkbox(
    id: String,

    label: impl Into<open_gpui::SharedString>,

    state: CheckboxState,

    tokens: ThemeTokens,
) -> Checkbox {
    Checkbox::new(id)
        .label(label)
        .checked(state.checked())
        .indeterminate(state.indeterminate())
        .disabled(state.disabled())
        .required(state.required())
        .invalid(state.invalid())
        .with_size(state.size())
        .tokens(tokens)
}

pub(crate) fn component_checkbox_state_row(state: CheckboxState) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .gap_1()
        .text_xs()
        .text_color(rgb(0x5a6472))
        .child(format!(
            "{} / {} / {}",
            toggled_label_text(state.toggled()),
            state.size().as_str(),
            if state.activation_enabled() {
                "enabled"
            } else {
                "disabled"
            }
        ))
        .child(format!(
            "{} / {}",
            if state.required() {
                "required"
            } else {
                "optional"
            },
            if state.invalid() { "invalid" } else { "valid" }
        ))
        .child(format!(
            "box {} indicator {}",
            format_px(state.metrics().box_size()),
            format_px(state.metrics().indicator_size())
        ))
}

pub(crate) fn component_label(id: String, state: &LabelState, tokens: ThemeTokens) -> Label {
    let label = Label::new(id, state.text())
        .with_size(state.size())
        .required(state.required())
        .disabled(state.disabled())
        .tokens(tokens);

    if let Some(control_id) = state.control_id() {
        label.for_control(control_id)
    } else {
        label
    }
}

pub(crate) fn component_label_state_row(state: &LabelState) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .gap_1()
        .text_xs()
        .text_color(rgb(0x5a6472))
        .child(format!(
            "{} / {} / {}",
            state.size().as_str(),
            if state.required() {
                "required"
            } else {
                "optional"
            },
            if state.disabled() {
                "disabled"
            } else {
                "enabled"
            }
        ))
        .child(format!(
            "{}",
            state.control_id().unwrap_or("no control association")
        ))
}

pub(crate) fn component_text_input(
    id: String,

    label: impl Into<open_gpui::SharedString>,

    state: &TextInputState,

    tokens: ThemeTokens,

    controller: Option<open_gpui::Entity<TextInputController>>,
) -> TextInput {
    let input = TextInput::new(id, label)
        .value(state.value())
        .with_size(state.size())
        .display_mode(state.display_mode())
        .disabled(state.disabled())
        .read_only(state.read_only())
        .required(state.required())
        .invalid(state.invalid())
        .tokens(tokens);

    let input = if let Some(controller) = controller {
        input.controller(controller)
    } else {
        input
    };

    if let Some(placeholder) = state.placeholder() {
        input.placeholder(placeholder)
    } else {
        input
    }
}

pub(crate) fn component_field(
    id: String,

    state: &FieldState,

    control: impl IntoElement,

    tokens: ThemeTokens,
) -> Field {
    let field = Field::new(id, state.control_id(), state.label())
        .with_size(state.size())
        .required(state.required())
        .disabled(state.disabled())
        .invalid(state.invalid())
        .tokens(tokens)
        .control(control);

    let field = if let Some(help) = state.help() {
        field.help(help)
    } else {
        field
    };

    if let Some(error) = state.error() {
        field.error(error)
    } else {
        field
    }
}

pub(crate) fn component_text_input_state_row(state: &TextInputState) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .gap_1()
        .text_xs()
        .text_color(rgb(0x5a6472))
        .child(format!(
            "{} / {} / {}",
            state.size().as_str(),
            if state.editable() {
                "editable"
            } else {
                "locked"
            },
            if state.invalid() { "invalid" } else { "valid" }
        ))
        .child(format!(
            "{} / {}",
            if state.has_value() { "value" } else { "empty" },
            if state.displaying_placeholder() {
                "placeholder"
            } else {
                "display value"
            }
        ))
        .child(format!("display mode: {}", state.display_mode().as_str()))
        .child(if state.controller_driven() {
            "controller"
        } else {
            "static"
        })
}

pub(crate) fn component_textarea(
    id: String,

    label: impl Into<open_gpui::SharedString>,

    state: &TextareaState,

    tokens: ThemeTokens,
) -> Textarea {
    let textarea = Textarea::new(id, label)
        .value(state.value())
        .rows(state.rows())
        .with_size(state.size())
        .disabled(state.disabled())
        .read_only(state.read_only())
        .required(state.required())
        .invalid(state.invalid())
        .tokens(tokens);

    if let Some(placeholder) = state.placeholder() {
        textarea.placeholder(placeholder)
    } else {
        textarea
    }
}

pub(crate) fn component_textarea_state_row(state: &TextareaState) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .gap_1()
        .text_xs()
        .text_color(rgb(0x5a6472))
        .child(format!(
            "{} / {} rows / {}",
            state.size().as_str(),
            state.rows(),
            if state.editable() {
                "editable"
            } else {
                "locked"
            }
        ))
        .child(format!(
            "{} / {}",
            if state.has_value() { "value" } else { "empty" },
            if state.displaying_placeholder() {
                "placeholder"
            } else {
                "display value"
            }
        ))
        .child(if state.controller_driven() {
            "controller"
        } else {
            "static"
        })
}

pub(crate) fn component_field_state_row(
    field: &FieldState,

    input: &TextInputState,
) -> impl IntoElement {
    let support = field.support_text().unwrap_or("no support text");

    div()
        .flex()
        .flex_col()
        .gap_1()
        .text_xs()
        .text_color(rgb(0x5a6472))
        .child(format!(
            "{} / {} / {}",
            field.size().as_str(),
            if field.required() {
                "required"
            } else {
                "optional"
            },
            if field.invalid() { "invalid" } else { "valid" }
        ))
        .child(format!(
            "{} / {}",
            if field.support_is_error() {
                "error"
            } else {
                "help"
            },
            support
        ))
        .child(if input.editable() {
            "control editable"
        } else {
            "control locked"
        })
}

pub(crate) fn component_field_textarea_state_row(
    field: &FieldState,

    textarea: &TextareaState,
) -> impl IntoElement {
    let support = field.support_text().unwrap_or("no support text");

    div()
        .flex()
        .flex_col()
        .gap_1()
        .text_xs()
        .text_color(rgb(0x5a6472))
        .child(format!(
            "{} / {} / {}",
            field.size().as_str(),
            if field.required() {
                "required"
            } else {
                "optional"
            },
            if field.invalid() { "invalid" } else { "valid" }
        ))
        .child(format!(
            "{} / {}",
            if field.support_is_error() {
                "error"
            } else {
                "help"
            },
            support
        ))
        .child(format!("textarea rows: {}", textarea.rows()))
        .child(if textarea.editable() {
            "control editable"
        } else {
            "control locked"
        })
}

pub(crate) fn component_primitive_samples_section(
    separators: [pages::components::SeparatorSample; 3],

    kbds: [pages::components::KbdSample; 3],

    progress: [pages::components::ProgressSample; 3],

    skeletons: [pages::components::SkeletonSample; 3],

    avatars: [pages::components::AvatarSample; 4],

    avatar_groups: [pages::components::AvatarGroupSample; 1],

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
                .child("Low-state primitives"),
        )
        .child(
            div()
                .flex()
                .gap_3()
                .flex_wrap()
                .children(separators.into_iter().map(move |sample| {
                    let state = sample.state;

                    let debug_selector = sample.debug_selector();

                    let separator = Separator::new(format!("component-separator:{}", sample.id))
                        .orientation(state.orientation())
                        .decorative(state.decorative())
                        .with_size(state.size())
                        .tokens(tokens);

                    gallery_card_shell(
                        format!("component-separator-sample:{}", sample.id),
                        Some(debug_selector),
                    )
                    .w(px(220.0))
                    .min_h(px(132.0))
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
                            .child(label_pill(match state.orientation() {
                                Orientation::Horizontal => "horizontal",

                                Orientation::Vertical => "vertical",
                            })),
                    )
                    .child(
                        div()
                            .h(px(46.0))
                            .flex()
                            .items_center()
                            .justify_center()
                            .rounded_sm()
                            .border_1()
                            .border_color(rgb(0xe2e4dc))
                            .bg(rgb(0xfcfcf8))
                            .child(if state.orientation() == Orientation::Vertical {
                                div().h_full().child(separator).into_any_element()
                            } else {
                                div().w_full().child(separator).into_any_element()
                            }),
                    )
                    .child(component_separator_state_row(state))
                })),
        )
        .child(
            div()
                .flex()
                .gap_3()
                .flex_wrap()
                .children(kbds.into_iter().map(move |sample| {
                    let debug_selector = sample.debug_selector();

                    let state = sample.state;

                    gallery_card_shell(
                        format!("component-kbd-sample:{}", sample.id),
                        Some(debug_selector),
                    )
                    .min_w(px(170.0))
                    .flex()
                    .flex_col()
                    .items_start()
                    .gap_2()
                    .child(
                        Kbd::new(format!("component-kbd:{}", sample.id), state.label())
                            .with_size(state.size())
                            .tokens(tokens),
                    )
                    .child(component_kbd_state_row(state))
                })),
        )
        .child(
            div()
                .flex()
                .gap_3()
                .flex_wrap()
                .children(progress.into_iter().map(move |sample| {
                    let state = sample.state;

                    let debug_selector = sample.debug_selector();

                    let progress =
                        Progress::new(format!("component-progress:{}", sample.id), sample.label)
                            .with_size(state.size())
                            .tokens(tokens);

                    let progress = match state.value_percent() {
                        Some(value) => progress.value(value),

                        None => progress.indeterminate(),
                    };

                    gallery_card_shell(
                        format!("component-progress-sample:{}", sample.id),
                        Some(debug_selector),
                    )
                    .w(px(280.0))
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        div()
                            .text_sm()
                            .font_weight(open_gpui::FontWeight::BOLD)
                            .child(sample.label),
                    )
                    .child(progress)
                    .child(component_progress_state_row(state))
                })),
        )
        .child(
            div()
                .flex()
                .gap_3()
                .flex_wrap()
                .children(skeletons.into_iter().map(move |sample| {
                    let state = sample.state;

                    let debug_selector = sample.debug_selector();

                    gallery_card_shell(
                        format!("component-skeleton-sample:{}", sample.id),
                        Some(debug_selector),
                    )
                    .min_w(px(250.0))
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        div()
                            .text_sm()
                            .font_weight(open_gpui::FontWeight::BOLD)
                            .child(sample.title),
                    )
                    .child(
                        Skeleton::new(format!("component-skeleton:{}", sample.id))
                            .subtle(state.subtle())
                            .with_size(state.size())
                            .tokens(tokens),
                    )
                    .child(component_skeleton_state_row(state))
                })),
        )
        .child(
            div()
                .flex()
                .gap_3()
                .flex_wrap()
                .children(avatars.into_iter().map(move |sample| {
                    let debug_selector = sample.debug_selector();

                    let state = sample.state.clone();

                    let avatar_name = state.name().to_owned();

                    let accessible_label = state.accessible_label().to_owned();

                    let fallback = state.fallback().to_owned();

                    let source = state.source().map(|source| source.uri().to_owned());

                    let avatar = Avatar::new(
                        format!("component-avatar:{}", sample.id),
                        avatar_name.clone(),
                    )
                    .accessible_label(accessible_label.clone())
                    .with_size(state.size())
                    .tokens(tokens);

                    let avatar = match source {
                        Some(source) => avatar.source(source),

                        None => avatar,
                    };

                    let avatar = avatar.fallback(fallback);

                    gallery_card_shell(
                        format!("component-avatar-sample:{}", sample.id),
                        Some(debug_selector),
                    )
                    .min_w(px(220.0))
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        div().flex().items_center().gap_3().child(avatar).child(
                            div()
                                .flex()
                                .flex_col()
                                .gap_1()
                                .child(
                                    div()
                                        .text_sm()
                                        .font_weight(open_gpui::FontWeight::BOLD)
                                        .child(if avatar_name.trim().is_empty() {
                                            "Empty name".to_owned()
                                        } else {
                                            avatar_name.clone()
                                        }),
                                )
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(rgb(0x5a6472))
                                        .child(accessible_label),
                                ),
                        ),
                    )
                    .child(component_avatar_state_row(&state))
                })),
        )
        .child(
            div()
                .flex()
                .gap_3()
                .flex_wrap()
                .children(avatar_groups.into_iter().map(move |sample| {
                    let debug_selector = sample.debug_selector();

                    let avatar_names = sample
                        .avatars
                        .iter()
                        .map(|avatar| avatar.state.name().to_owned())
                        .collect::<Vec<_>>();

                    gallery_card_shell(
                        format!("component-avatar-group-sample:{}", sample.id),
                        Some(debug_selector),
                    )
                    .min_w(px(280.0))
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        div()
                            .text_sm()
                            .font_weight(open_gpui::FontWeight::BOLD)
                            .child(sample.summary),
                    )
                    .child(
                        AvatarGroup::new(format!("component-avatar-group:{}", sample.id))
                            .avatars(sample.avatars.iter().map(|avatar| {
                                Avatar::new(
                                    format!("component-avatar-group:{}:{}", sample.id, avatar.id),
                                    avatar.state.name(),
                                )
                                .accessible_label(avatar.state.accessible_label())
                                .fallback(avatar.state.fallback())
                                .with_size(avatar.state.size())
                                .tokens(tokens)
                            }))
                            .max_visible(sample.avatars.len().saturating_sub(1))
                            .with_size(Size::Medium)
                            .tokens(tokens),
                    )
                    .child(div().text_xs().text_color(rgb(0x5a6472)).child(format!(
                        "{}: {}",
                        sample.count_label,
                        avatar_names.join(", ")
                    )))
                })),
        )
}

pub(crate) fn component_page_section(id: &'static str) -> open_gpui::Stateful<open_gpui::Div> {
    div()
        .id(format!("gallery-components-section:{id}"))
        .debug_selector(move || format!("gallery:components-section:{id}"))
}

pub(crate) fn component_page_jump(
    section: pages::components::ComponentPageJump,
    tokens: ThemeTokens,
    cx: &mut Context<GalleryShell>,
) -> open_gpui::Stateful<open_gpui::Div> {
    let id = section.id;
    let label = section.label;
    let button = Button::new(format!("gallery-components-jump-button:{id}"), label)
        .variant(ButtonVariant::Ghost)
        .with_size(Size::Small)
        .tokens(tokens)
        .on_click(cx.listener(move |this, _, _, cx| {
            this.jump_to_components_section(id, cx);
        }));

    div()
        .id(format!("gallery-components-jump:{id}"))
        .debug_selector(move || format!("gallery:component-page-jump:{id}"))
        .flex_none()
        .child(button)
}

pub(crate) fn component_listbox_samples_section(
    samples: [pages::components::ListboxSample; 2],

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
                .child("Listbox"),
        )
        .child(
            div()
                .flex()
                .gap_3()
                .flex_wrap()
                .children(samples.into_iter().map(move |sample| {
                    let sample_id = sample.id;

                    let debug_selector = sample.debug_selector();

                    let state = sample.state.clone();

                    let label = state.label().to_owned();

                    let listbox_options: Vec<_> = state
                        .standalone_options()
                        .map(resolved_listbox_option)
                        .collect();

                    let listbox_groups: Vec<_> = state
                        .groups()
                        .iter()
                        .map(|group_state| resolved_listbox_group(group_state, &state))
                        .collect();

                    let mut listbox =
                        Listbox::new(format!("component-listbox:{}", sample.id), label.clone())
                            .with_size(state.size())
                            .disabled(state.disabled())
                            .tokens(tokens);

                    if let Some(selected) = state.selected_value() {
                        listbox = listbox.selected(selected);
                    }

                    if let Some(active) = state.active_value() {
                        listbox = listbox.active(active);
                    }

                    for option in listbox_options.iter() {
                        listbox = listbox.option(option.clone());
                    }

                    for group in listbox_groups.iter() {
                        listbox = listbox.group(group.clone());
                    }

                    div()
                        .id(format!("component-listbox-sample:{sample_id}"))
                        .debug_selector(move || debug_selector)
                        .w(px(320.0))
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
                                        .child(label.clone()),
                                )
                                .child(label_pill(state.size().as_str())),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(rgb(0x5a6472))
                                .child(sample.summary),
                        )
                        .child(listbox)
                        .child(component_listbox_state_row(&state))
                })),
        )
}

pub(crate) fn component_select_samples_section(
    samples: [pages::components::SelectSample; 3],

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
                .child("Select"),
        )
        .child(
            div()
                .flex()
                .gap_3()
                .flex_wrap()
                .children(samples.into_iter().map(move |sample| {
                    let sample_id = sample.id;

                    let debug_selector = sample.debug_selector();

                    let state = sample.state.clone();

                    let label = state.label().to_owned();

                    let title = label.clone();

                    let listbox_options: Vec<_> = state
                        .listbox()
                        .standalone_options()
                        .map(resolved_listbox_option)
                        .collect();

                    let listbox_groups: Vec<_> = state
                        .listbox()
                        .groups()
                        .iter()
                        .map(|group_state| resolved_listbox_group(group_state, state.listbox()))
                        .collect();

                    // Keep the gallery sample closed on mount so the page stays scrollable.

                    let mut select =
                        Select::new(format!("component-select:{}", sample.id), label.clone())
                            .placeholder(state.placeholder())
                            .with_size(state.size())
                            .disabled(state.disabled())
                            .tokens(tokens);

                    if let Some(selected) = state.selected_value() {
                        select = select.selected(selected);
                    }

                    if let Some(active) = state.active_value() {
                        select = select.active(active);
                    }

                    select = match state.open_mode() {
                        SelectOpenMode::Controlled => select.open(GALLERY_SAMPLE_MOUNT_OPEN),

                        SelectOpenMode::Uncontrolled => {
                            select.default_open(GALLERY_SAMPLE_MOUNT_OPEN)
                        }
                    };

                    for group in listbox_groups.iter() {
                        select = select.group(group.clone());
                    }

                    for option in listbox_options.iter() {
                        select = select.option(option.clone());
                    }

                    div()
                        .id(format!("component-select-sample:{sample_id}"))
                        .debug_selector(move || debug_selector)
                        .w(px(340.0))
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
                                .child(label_pill(if GALLERY_SAMPLE_MOUNT_OPEN {
                                    "mount open"
                                } else {
                                    "mount closed"
                                })),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(rgb(0x5a6472))
                                .child(sample.summary),
                        )
                        .child(select)
                        .child(component_select_state_row(&state))
                })),
        )
}

pub(crate) fn component_combobox_samples_section(
    samples: [pages::components::ComboboxSample; 3],

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
                .child("Combobox"),
        )
        .child(
            div()
                .flex()
                .gap_3()
                .flex_wrap()
                .children(samples.into_iter().map(move |sample| {
                    let sample_id = sample.id;

                    let debug_selector = sample.debug_selector();

                    let state = sample.state.clone();

                    let label = state.label().to_owned();

                    let title = label.clone();

                    let combobox_options: Vec<_> = state
                        .listbox()
                        .standalone_options()
                        .map(resolved_combobox_option)
                        .collect();

                    let combobox_groups: Vec<_> = state
                        .listbox()
                        .groups()
                        .iter()
                        .map(|group_state| resolved_combobox_group(group_state, state.listbox()))
                        .collect();

                    // Keep the gallery sample closed on mount so the page stays scrollable.

                    let mut combobox =
                        Combobox::new(format!("component-combobox:{}", sample.id), label.clone())
                            .placeholder(state.placeholder())
                            .default_query(state.query())
                            .with_size(state.size())
                            .disabled(state.disabled())
                            .tokens(tokens);

                    if let Some(selected) = state.selected_value() {
                        combobox = combobox.selected(selected);
                    }

                    if let Some(active) = state.active_value() {
                        combobox = combobox.active(active);
                    }

                    combobox = match state.open_mode() {
                        ComboboxOpenMode::Controlled => combobox.open(GALLERY_SAMPLE_MOUNT_OPEN),

                        ComboboxOpenMode::Uncontrolled => {
                            combobox.default_open(GALLERY_SAMPLE_MOUNT_OPEN)
                        }
                    };

                    for option in combobox_options.iter() {
                        combobox = combobox.option(option.clone());
                    }

                    for group in combobox_groups.iter() {
                        combobox = combobox.group(group.clone());
                    }

                    div()
                        .id(format!("component-combobox-sample:{sample_id}"))
                        .debug_selector(move || debug_selector)
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
                                .child(label_pill(if GALLERY_SAMPLE_MOUNT_OPEN {
                                    "mount open"
                                } else {
                                    "mount closed"
                                })),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(rgb(0x5a6472))
                                .child(sample.summary),
                        )
                        .child(combobox)
                        .child(component_combobox_state_row(&state))
                })),
        )
}

pub(crate) fn component_command_samples_section(
    samples: [pages::components::CommandSample; 6],

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
                .child("Command"),
        )
        .child(
            div()
                .flex()
                .gap_3()
                .flex_wrap()
                .children(samples.into_iter().map(move |sample| {
                    let sample_id = sample.id;

                    let debug_selector = sample.debug_selector();

                    let state = sample.state.clone();

                    let label = state.label().to_owned();

                    let title = label.clone();
                    let provider_status_summary = sample.provider_status.as_ref().map(|status| {
                        format!(
                            "provider {} / {:?} / {} sources / {} commands",
                            status.provider_id().as_str(),
                            status.state(),
                            status.source_count(),
                            status.command_count()
                        )
                    });

                    // Keep the gallery sample closed on mount so the page stays scrollable.

                    let mut command =
                        Command::new(format!("component-command:{}", sample.id), label.clone())
                            .placeholder(state.placeholder())
                            .default_query(state.query())
                            .selection_mode(state.selection_mode())
                            .viewport_item_count(sample.viewport_item_count)
                            .overscan(sample.overscan)
                            .with_size(state.size())
                            .disabled(state.disabled())
                            .tokens(tokens);

                    if let Some(row_height) = sample.row_height {
                        command = command.row_height(row_height);
                    }

                    if let Some(snapshot) = sample.index_snapshot.clone() {
                        command = command.index_snapshot(snapshot);
                    }

                    if let Some(selected) = state.selected_value() {
                        command = command.selected(selected);
                    }

                    if state.selection_mode() == CommandSelectionMode::Multiple {
                        command = command.selected_values(sample.selected_values.iter().cloned());
                    }

                    if let Some(active) = state.active_value() {
                        command = command.active(active);
                    }

                    if let Some(dialog) = state.dialog() {
                        command = command.dialog(dialog.title());

                        if let Some(description) = dialog.description() {
                            command = command.dialog_description(description);
                        }
                    }

                    if let Some(loading) = state.loading() {
                        command = command.loading(loading.message(), loading.progress_percent());
                    }

                    command = match state.open_mode() {
                        CommandOpenMode::Controlled => command.open(GALLERY_SAMPLE_MOUNT_OPEN),

                        CommandOpenMode::Uncontrolled => {
                            command.default_open(GALLERY_SAMPLE_MOUNT_OPEN)
                        }
                    };

                    for item in sample.items.iter() {
                        command = command.item(resolved_command_descriptor_item(item));
                    }

                    for group in sample.groups.iter() {
                        command = command.group(resolved_command_descriptor_group(group));
                    }

                    div()
                        .id(format!("component-command-sample:{sample_id}"))
                        .debug_selector(move || debug_selector)
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
                                        .child(title),
                                )
                                .child(label_pill(if state.dialog().is_some() {
                                    "dialog"
                                } else {
                                    "inline"
                                })),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(rgb(0x5a6472))
                                .child(sample.summary),
                        )
                        .child(command)
                        .child(component_command_state_row(&state))
                        .when_some(provider_status_summary, |this, summary| {
                            this.child(div().text_xs().text_color(rgb(0x5a6472)).child(summary))
                        })
                })),
        )
}

fn resolved_listbox_option(
    option_state: &open_gpui_ui_components::ListboxOptionState,
) -> ListboxOption {
    match option_state.kind() {
        open_gpui_ui_components::ListboxOptionKind::Separator => {
            ListboxOption::separator(option_state.value())
        }

        open_gpui_ui_components::ListboxOptionKind::Option => {
            ListboxOption::new(option_state.value(), option_state.label())
                .disabled(option_state.disabled())
        }
    }
}

fn resolved_listbox_group(
    group_state: &open_gpui_ui_components::ListboxGroupState,

    state: &ListboxState,
) -> ListboxGroup {
    state.group_options(group_state.index()).fold(
        ListboxGroup::new(group_state.value(), group_state.label()),
        |group, option_state| group.option(resolved_listbox_option(option_state)),
    )
}

fn resolved_combobox_option(
    option_state: &open_gpui_ui_components::ListboxOptionState,
) -> ComboboxOption {
    ComboboxOption::new(option_state.value(), option_state.label())
        .disabled(option_state.disabled())
}

fn resolved_combobox_group(
    group_state: &open_gpui_ui_components::ListboxGroupState,

    state: &ListboxState,
) -> ComboboxGroup {
    state.group_options(group_state.index()).fold(
        ComboboxGroup::new(group_state.value(), group_state.label()),
        |group, option_state| group.option(resolved_combobox_option(option_state)),
    )
}

fn resolved_command_descriptor_item(
    descriptor: &open_gpui_ui_components::CommandItemDescriptor,
) -> CommandItem {
    let mut command_item = CommandItem::new(descriptor.value(), descriptor.label())
        .disabled(descriptor.disabled_state());

    if let Some(shortcut) = descriptor.shortcut_ref() {
        command_item = command_item.shortcut(shortcut);
    }

    for keyword in descriptor.keywords_ref() {
        command_item = command_item.keyword(keyword);
    }

    command_item
}

fn resolved_command_descriptor_group(
    descriptor: &open_gpui_ui_components::CommandGroupDescriptor,
) -> CommandGroup {
    descriptor.items_ref().iter().fold(
        CommandGroup::new(descriptor.value(), descriptor.label()),
        |group, item| group.item(resolved_command_descriptor_item(item)),
    )
}

fn component_listbox_state_row(state: &ListboxState) -> impl IntoElement {
    let selected = state.selected_value().unwrap_or("none");

    let active = state.active_value().unwrap_or("none");
    let typeahead = state.typeahead_query().unwrap_or("");
    let typeahead_label = if typeahead.is_empty() {
        "none"
    } else {
        typeahead
    };
    let first_typeahead_target = if typeahead.is_empty() {
        "none"
    } else {
        state
            .typeahead_target(typeahead)
            .map(|option| option.value())
            .unwrap_or("none")
    };

    let disabled_count = state
        .options()
        .iter()
        .filter(|option| option.disabled())
        .count();

    div()
        .flex()
        .flex_col()
        .gap_1()
        .text_xs()
        .text_color(rgb(0x5a6472))
        .child(format!("{:?} / {}", state.role(), state.size().as_str()))
        .child(format!("selected {} / active {}", selected, active))
        .child(format!(
            "typeahead '{}' / target {}",
            typeahead_label, first_typeahead_target
        ))
        .child(format!(
            "{} groups / {} options / {} disabled",
            state.groups().len(),
            state.options().len(),
            disabled_count
        ))
}

fn component_select_state_row(state: &SelectState) -> impl IntoElement {
    let selected = state.selected_value().unwrap_or("none");

    let active = state.active_value().unwrap_or("none");
    let listbox_selected = state.listbox().selected_value().unwrap_or("none");

    div()
        .flex()
        .flex_col()
        .gap_1()
        .text_xs()
        .text_color(rgb(0x5a6472))
        .child(format!(
            "{:?} / {:?} / {}",
            state.trigger_role(),
            state.content_role(),
            state.size().as_str()
        ))
        .child(format!(
            "{} / selected {} / active {}",
            if state.open() { "open" } else { "closed" },
            selected,
            active
        ))
        .child(format!(
            "listbox selected {} / scroll {}",
            listbox_selected,
            if state.scrollable_content() {
                "enabled"
            } else {
                "not needed"
            }
        ))
        .child(format!(
            "{} options / {:?}",
            state.listbox().options().len(),
            state.outside_press_policy()
        ))
}

fn component_combobox_state_row(state: &ComboboxState) -> impl IntoElement {
    let selected = state.selected_value().unwrap_or("none");

    let active = state.active_value().unwrap_or("none");
    let query = state.query();
    let typeahead = state.listbox().typeahead_query().unwrap_or("none");

    div()
        .flex()
        .flex_col()
        .gap_1()
        .text_xs()
        .text_color(rgb(0x5a6472))
        .child(format!(
            "{:?} / {:?} / {}",
            state.input_role(),
            state.content_role(),
            state.size().as_str()
        ))
        .child(format!(
            "query '{}' / selected {} / active {}",
            query, selected, active
        ))
        .child(format!(
            "visible {} of {} / typeahead '{}' / {:?}",
            state.filtered_option_count(),
            state.total_option_count(),
            typeahead,
            state.outside_press_policy()
        ))
}

fn component_command_state_row(state: &CommandState) -> impl IntoElement {
    let selected = state.selected_value().unwrap_or("none");

    let active = state.active_value().unwrap_or("none");
    let revision = state.index_revision().unwrap_or("local");
    let selected_count = state.selected_values().len();

    div()
        .flex()
        .flex_col()
        .gap_1()
        .text_xs()
        .text_color(rgb(0x5a6472))
        .child(format!(
            "{:?} / {:?} / {}",
            state.input_role(),
            state.list_role(),
            state.size().as_str()
        ))
        .child(format!(
            "query '{}' / selected {} / active {}",
            state.query(),
            selected,
            active
        ))
        .child(format!(
            "{} groups / {} of {} commands / {:?}",
            state.groups().len(),
            state.filtered_item_count(),
            state.total_item_count(),
            state.selection_mode()
        ))
        .child(format!(
            "{} / index {} / {:?} / {} chips / selected_values {:?}",
            if state.dialog().is_some() {
                "dialog"
            } else {
                "inline"
            },
            revision,
            state.index_mode(),
            selected_count,
            state.selected_values()
        ))
}

pub(crate) fn component_radio_state_row(
    state: &open_gpui_ui_components::RadioGroupState,
) -> impl IntoElement {
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
            if state.required() {
                "required"
            } else {
                "optional"
            },
            if state.activation_enabled() {
                "enabled"
            } else {
                "disabled"
            }
        ))
        .child(format!("selected {} / focus {}", selected, focused))
        .child(format!(
            "{} items / {} disabled",
            state.items().len(),
            disabled_count
        ))
}

pub(crate) fn component_toggle_state_row(state: &ToggleState) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .gap_1()
        .text_xs()
        .text_color(rgb(0x5a6472))
        .child(format!(
            "{} / {} / {}",
            if state.pressed() {
                "pressed"
            } else {
                "released"
            },
            state.variant().as_str(),
            state.size().as_str()
        ))
        .child(format!(
            "h {} px {}",
            format_px(state.metrics().height()),
            format_px(state.metrics().padding_x())
        ))
}
