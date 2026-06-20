//! Components page rendering for the foundation gallery.

use crate::pages;
use crate::shell::*;
use open_gpui::prelude::*;
use open_gpui::{IntoElement, div, px, rgb};
use open_gpui_ui_components::*;
use open_gpui_ui_core::{Orientation, Sizable};

pub(crate) fn render_components_page(
    shell: &GalleryShell,
    snapshot: GalleryShellSnapshot,
) -> impl IntoElement {
    let component_catalog = pages::components::COMPONENT_CATALOG;
    let component_catalog_cards = component_catalog.iter().map(|entry| {
        let catalog_selector = entry.catalog_selector();
        gallery_card_shell(catalog_selector.clone(), Some(catalog_selector))
            .min_w(px(180.0))
            .flex()
            .flex_col()
            .gap_1()
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
                            .child(entry.name),
                    )
                    .child(component_catalog_status_pill(entry.status)),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(rgb(0x5a6472))
                    .child(entry.family),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(rgb(0x5a6472))
                    .child(entry.display_state_label()),
            )
            .child(
                div()
                    .text_xs()
                    .line_height(px(18.0))
                    .text_color(rgb(0x5a6472))
                    .child(entry.coverage),
            )
    });
    let conformance_gates = pages::components::CONFORMANCE_GATES;
    let conformance_gate_cards = conformance_gates.iter().map(|gate| {
        let gate_selector = format!("component-gate:{}", gate.id);
        gallery_card_shell(gate_selector.clone(), Some(gate_selector))
            .min_w(px(220.0))
            .flex()
            .flex_col()
            .gap_2()
            .child(
                div()
                    .text_sm()
                    .font_weight(open_gpui::FontWeight::BOLD)
                    .child(gate.title),
            )
            .child(
                div()
                    .text_xs()
                    .line_height(px(18.0))
                    .text_color(rgb(0x5a6472))
                    .child(gate.summary),
            )
            .child(
                div()
                    .text_xs()
                    .line_height(px(18.0))
                    .text_color(rgb(0x5a6472))
                    .child(gate.evidence.join(" / ")),
            )
    });
    let tabs_samples = pages::components::tabs_samples(snapshot.tokens);
    let radio_samples = pages::components::radio_group_samples(snapshot.tokens);
    let toggle_samples = pages::components::toggle_samples(snapshot.tokens);
    let toolbar_samples = pages::components::toolbar_samples(snapshot.tokens);
    let sidebar_samples = pages::components::sidebar_samples(snapshot.tokens);
    let listbox_samples = pages::components::listbox_samples(snapshot.tokens);
    let select_samples = pages::components::select_samples(snapshot.tokens);
    let combobox_samples = pages::components::combobox_samples(snapshot.tokens);
    let command_samples = pages::components::command_samples(snapshot.tokens);
    let badge_samples = pages::components::badge_samples(snapshot.tokens);
    let icon_button_samples = pages::components::icon_button_samples(snapshot.tokens);
    let separator_samples = pages::components::separator_samples(snapshot.tokens);
    let kbd_samples = pages::components::kbd_samples(snapshot.tokens);
    let progress_samples = pages::components::progress_samples(snapshot.tokens);
    let skeleton_samples = pages::components::skeleton_samples(snapshot.tokens);
    let avatar_samples = pages::components::avatar_samples(snapshot.tokens);
    let scroll_area_samples = pages::components::scroll_area_samples(snapshot.tokens);
    let splitter_samples = pages::components::splitter_samples(snapshot.tokens);

    div()
        .id("gallery-components-page")
        .debug_selector(|| "gallery:components-page".into())
        .flex()
        .flex_col()
        .gap_5()
        .child(
            div()
                .flex()
                .flex_col()
                .gap_2()
                .child(
                    div()
                        .text_sm()
                        .font_weight(open_gpui::FontWeight::BOLD)
                        .child("Component catalog"),
                )
                .child(
                    div()
                        .flex()
                        .gap_3()
                        .flex_wrap()
                        .children(component_catalog_cards),
                ),
        )
        .child(component_primitive_samples_section(
            separator_samples,
            kbd_samples,
            progress_samples,
            skeleton_samples,
            avatar_samples,
            snapshot.tokens,
        ))
        .child(
            div()
                .flex()
                .flex_col()
                .gap_2()
                .child(
                    div()
                        .text_sm()
                        .font_weight(open_gpui::FontWeight::BOLD)
                        .child("Conformance gates"),
                )
                .child(
                    div()
                        .flex()
                        .gap_3()
                        .flex_wrap()
                        .children(conformance_gate_cards),
                ),
        )
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
                    div()
                        .flex()
                        .gap_3()
                        .flex_wrap()
                        .children(sidebar_samples.into_iter().map(|sample| {
                            let state = sample.state.clone();
                            let title = state.label().to_owned();
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
                                sidebar = sidebar.focused(focused);
                            }
                            for section in state.sections() {
                                let mut sidebar_section =
                                    SidebarSection::new(section.value(), section.label());
                                for item in state
                                    .items()
                                    .iter()
                                    .filter(|item| item.section_index() == section.index())
                                {
                                    let mut sidebar_item =
                                        SidebarItem::new(item.value(), item.label());
                                    if let Some(icon) = item.icon_label() {
                                        sidebar_item = sidebar_item.icon(icon);
                                    }
                                    if let Some(badge) = item.badge_label() {
                                        sidebar_item = sidebar_item.badge(badge);
                                    }
                                    if let Some(action_label) = item.action_label_text() {
                                        sidebar_item = sidebar_item.action_label(action_label);
                                    }
                                    sidebar_section = sidebar_section
                                        .item(sidebar_item.disabled(item.disabled()));
                                }
                                sidebar = sidebar.section(sidebar_section);
                            }

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
                                .child(component_sidebar_state_row(&state))
                        })),
                ),
        )
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
                    div()
                        .flex()
                        .gap_3()
                        .flex_wrap()
                        .children(toolbar_samples.into_iter().map(|sample| {
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
                        })),
                ),
        )
        .child(component_listbox_samples_section(
            listbox_samples,
            snapshot.tokens,
        ))
        .child(component_select_samples_section(
            select_samples,
            snapshot.tokens,
        ))
        .child(component_combobox_samples_section(
            combobox_samples,
            snapshot.tokens,
        ))
        .child(component_command_samples_section(
            command_samples,
            snapshot.tokens,
        ))
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
                .child(div().flex().gap_3().flex_wrap().children(
                    splitter_samples.into_iter().map(|sample| {
                        let debug_selector = sample.debug_selector();
                        let state = sample.state.clone();
                        let splitter = sample.panels.into_iter().fold(
                            Splitter::new(format!("component-splitter:{}", sample.id))
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
                                                .font_weight(open_gpui::FontWeight::BOLD)
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
                            .id(format!("component-splitter-sample:{}", sample.id))
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
                )),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .gap_2()
                .child(
                    div()
                        .text_sm()
                        .font_weight(open_gpui::FontWeight::BOLD)
                        .child("ScrollArea"),
                )
                .child(div().flex().gap_3().flex_wrap().children(
                    scroll_area_samples.into_iter().map(|sample| {
                        let sample_id = sample.id;
                        let debug_selector = sample.debug_selector();
                        let title = sample.title;
                        let summary = sample.summary;
                        let items = sample.items;
                        let state = sample.state.clone();
                        let horizontal = state.axis() == ScrollAreaAxis::Horizontal;
                        let two_axis = state.axis() == ScrollAreaAxis::Both;
                        let content = div()
                            .when(horizontal, |this| this.flex().gap_2().min_w(px(860.0)))
                            .when(two_axis, |this| {
                                this.flex().flex_col().gap_1().min_w(px(620.0))
                            })
                            .when(!horizontal && !two_axis, |this| {
                                this.flex().flex_col().gap_1()
                            })
                            .children(items.into_iter().enumerate().map(move |(index, item)| {
                                let vertical_only = !horizontal && !two_axis;
                                div()
                                    .id(format!(
                                        "component-scroll-area-item:{}:{}",
                                        sample_id, index
                                    ))
                                    .debug_selector(move || {
                                        format!(
                                            "gallery:component-scroll-area-item:{sample_id}:{index}"
                                        )
                                    })
                                    .when(horizontal, |this| this.w(px(132.0)).min_h(px(88.0)))
                                    .when(two_axis, |this| this.w(px(620.0)).min_h(px(34.0)))
                                    .when(vertical_only, |this| this.min_h(px(28.0)))
                                    .rounded_sm()
                                    .border_1()
                                    .border_color(rgb(0xd6d8ce))
                                    .bg(rgb(0xf8f9f3))
                                    .px_3()
                                    .py_2()
                                    .text_xs()
                                    .text_color(rgb(0x3f4a57))
                                    .child(item)
                            }));
                        let scroll_area = ScrollArea::new(
                            format!("component-scroll-area:{}", sample_id),
                            content,
                        )
                        .axis(state.axis())
                        .with_size(state.size());
                        let scroll_area = if let Some(reset_key) = state.reset_key() {
                            scroll_area.reset_on_key(reset_key)
                        } else {
                            scroll_area
                        };

                        div()
                            .id(format!("component-scroll-area-sample:{}", sample_id))
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
                                    .child(label_pill(state.axis().as_str())),
                            )
                            .child(div().text_xs().text_color(rgb(0x5a6472)).child(summary))
                            .child(
                                div()
                                    .h(px(154.0))
                                    .rounded_sm()
                                    .border_1()
                                    .border_color(rgb(0xe2e4dc))
                                    .bg(rgb(0xfcfcf8))
                                    .child(scroll_area),
                            )
                            .child(component_scroll_area_state_row(&state))
                    }),
                )),
        )
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
                    div()
                        .flex()
                        .gap_3()
                        .flex_wrap()
                        .children(badge_samples.into_iter().map(|sample| {
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
                        })),
                ),
        )
        .child(
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
                        pages::components::switch_samples(snapshot.tokens)
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
                                            .tokens(snapshot.tokens),
                                    )
                                    .child(component_switch_state_row(state))
                            }),
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
                        .text_sm()
                        .font_weight(open_gpui::FontWeight::BOLD)
                        .child("Checkbox"),
                )
                .child(
                    div().flex().gap_3().flex_wrap().children(
                        pages::components::checkbox_samples(snapshot.tokens)
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
                                        snapshot.tokens,
                                    ))
                                    .child(component_checkbox_state_row(state))
                            }),
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
                        .text_sm()
                        .font_weight(open_gpui::FontWeight::BOLD)
                        .child("RadioGroup"),
                )
                .child(
                    div()
                        .flex()
                        .gap_3()
                        .flex_wrap()
                        .children(radio_samples.into_iter().map(|sample| {
                            let sample_id = sample.id;
                            let debug_selector = sample.debug_selector();
                            let state = sample.state.clone();
                            let mut radio =
                                RadioGroup::new(format!("component-radio:{}", sample.id))
                                    .label(sample.title)
                                    .orientation(state.orientation())
                                    .selected(state.selected_value().unwrap_or("none"))
                                    .required(state.required())
                                    .disabled(state.disabled())
                                    .with_size(state.size())
                                    .tokens(snapshot.tokens);
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
                        })),
                ),
        )
        .child(
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
                    div()
                        .flex()
                        .gap_3()
                        .flex_wrap()
                        .children(toggle_samples.into_iter().map(|sample| {
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
                                    .tokens(snapshot.tokens),
                                )
                                .child(component_toggle_state_row(&state))
                        })),
                ),
        )
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
                .child(div().flex().gap_3().flex_wrap().children(
                    icon_button_samples.into_iter().map(|sample| {
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
                            .child(component_icon_button_state_row(&accessible_label, state))
                    }),
                )),
        )
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
                        pages::components::text_input_samples(snapshot.tokens)
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
                                        snapshot.tokens,
                                        controller,
                                    ))
                                    .child(component_text_input_state_row(&state))
                            }),
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
                        .text_sm()
                        .font_weight(open_gpui::FontWeight::BOLD)
                        .child("Field"),
                )
                .child(
                    div().flex().gap_3().flex_wrap().children(
                        pages::components::field_samples(snapshot.tokens)
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
                                            snapshot.tokens,
                                            None,
                                        ),
                                        snapshot.tokens,
                                    ))
                                    .child(component_field_state_row(&field_state, &input_state))
                            }),
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
                        .text_sm()
                        .font_weight(open_gpui::FontWeight::BOLD)
                        .child("Tabs"),
                )
                .child(
                    div()
                        .flex()
                        .gap_3()
                        .flex_wrap()
                        .children(tabs_samples.into_iter().map(|sample| {
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
                                                .font_weight(open_gpui::FontWeight::BOLD)
                                                .child(sample.title),
                                        )
                                        .child(label_pill(state.activation_mode().as_str())),
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
                                            state.orientation() == Orientation::Vertical,
                                            |this| this.h(px(240.0)),
                                        )
                                        .child(tabs),
                                )
                                .child(component_tabs_state_row(&state))
                        })),
                ),
        )
        .child(shell.render_signal_list(snapshot.selected_page))
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

pub(crate) fn component_sidebar_state_row(state: &SidebarState) -> impl IntoElement {
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

pub(crate) fn gallery_card_shell(
    id: impl Into<open_gpui::ElementId>,
    debug_selector: Option<String>,
) -> open_gpui::Stateful<open_gpui::Div> {
    let card = div().id(id);
    let card = match debug_selector {
        Some(debug_selector) => card.debug_selector(move || debug_selector),
        None => card,
    };

    card.rounded_sm()
        .border_1()
        .border_color(rgb(0xd6d8ce))
        .bg(rgb(0xffffff))
        .p_3()
}
