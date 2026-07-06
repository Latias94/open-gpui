use super::*;

pub(super) fn render_component_catalog_section(
    snapshot: GalleryShellSnapshot,
    cx: &mut Context<GalleryShell>,
) -> AnyElement {
    let component_catalog_cards = pages::components::COMPONENT_CATALOG
        .iter()
        .map(|entry| {
            let catalog_selector = entry.catalog_selector();
            let story = pages::components::component_story_contract_for(entry.name);
            let focus = story.as_ref().and_then(|story| story.section_id());
            let focused = snapshot.components_focus.focused_section() == focus;
            let card =
                component_gallery_card_shell(catalog_selector.clone(), Some(catalog_selector))
                    .min_w(px(180.0))
                    .flex()
                    .flex_col()
                    .gap_1()
                    .border_color(if focused {
                        rgb(0x1f7a66)
                    } else {
                        rgb(0xd6d8ce)
                    })
                    .bg(if focused {
                        rgb(0xe8f3ef)
                    } else {
                        rgb(0xffffff)
                    })
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
                    );

            match focus {
                Some(section_id) => card
                    .cursor_pointer()
                    .hover(|style| style.bg(rgb(0xf1f5ee)))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.set_components_focus(
                            pages::components::ComponentFocusMode::Section(section_id),
                            cx,
                        );
                    })),
                None => card,
            }
        })
        .collect::<Vec<_>>();

    component_page_section("catalog")
        .child(render_component_focus_mode(
            snapshot.components_focus,
            snapshot.tokens,
            cx,
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
        .into_any_element()
}

pub(super) fn render_component_gates_section(
    focus_mode: pages::components::ComponentFocusMode,
) -> AnyElement {
    let conformance_gate_cards =
        pages::components::COMPONENT_CONFORMANCE_GATES
            .iter()
            .map(|gate| {
                let gate_selector = format!("component-gate:{}", gate.id);
                component_gallery_card_shell(gate_selector.clone(), Some(gate_selector))
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

    component_page_section("gates")
        .when(!show_component_section(focus_mode, "gates"), |this| {
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
        .into_any_element()
}
