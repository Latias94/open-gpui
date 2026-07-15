use super::*;

pub(super) fn render_component_focus_mode(
    mode: pages::components::ComponentFocusMode,
    tokens: ThemeTokens,
    cx: &mut Context<GalleryShell>,
) -> impl IntoElement {
    let label = match mode {
        pages::components::ComponentFocusMode::All => "All components".to_owned(),
        pages::components::ComponentFocusMode::Section(section) => {
            let label = pages::components::COMPONENT_PAGE_JUMPS
                .iter()
                .find(|jump| jump.id == section)
                .map(|jump| jump.label)
                .unwrap_or(section);
            format!("Focused: {label}")
        }
    };

    div()
        .id("gallery-components-focus-mode")
        .debug_selector(|| "gallery:component-focus:mode".into())
        .flex()
        .items_center()
        .justify_between()
        .gap_2()
        .rounded_sm()
        .border_1()
        .border_color(rgb(0xd6d8ce))
        .bg(rgb(0xfcfcf8))
        .px_3()
        .py_2()
        .child(
            div()
                .flex()
                .flex_col()
                .gap_1()
                .child(
                    div()
                        .text_sm()
                        .font_weight(open_gpui::FontWeight::BOLD)
                        .child(label),
                )
                .child(
                    div().text_xs().text_color(rgb(0x5a6472)).child(
                        "Use catalog cards or section chips to inspect one component family.",
                    ),
                ),
        )
        .child(component_focus_button(
            "all-inline",
            "All components",
            mode == pages::components::ComponentFocusMode::All,
            pages::components::ComponentFocusMode::All,
            tokens,
            cx,
        ))
}

pub(super) fn component_focus_button(
    id: &'static str,
    label: &'static str,
    selected: bool,
    focus: pages::components::ComponentFocusMode,
    tokens: ThemeTokens,
    cx: &mut Context<GalleryShell>,
) -> open_gpui::Stateful<open_gpui::Div> {
    div()
        .id(format!("gallery-components-focus:{id}"))
        .debug_selector(move || format!("gallery:component-focus:{id}"))
        .flex_none()
        .child(
            Button::new(format!("gallery-components-focus-button:{id}"), label)
                .variant(if selected {
                    ButtonVariant::Secondary
                } else {
                    ButtonVariant::Ghost
                })
                .selected(selected)
                .with_size(Size::Small)
                .tokens(tokens)
                .on_activate(cx.processor(move |this, _, _, cx| {
                    this.set_components_focus(focus, cx);
                })),
        )
}

pub(super) fn show_component_section(
    mode: pages::components::ComponentFocusMode,
    id: &'static str,
) -> bool {
    mode.shows_section(id)
}
