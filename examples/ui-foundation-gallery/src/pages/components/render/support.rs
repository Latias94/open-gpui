use super::*;

pub(crate) fn component_gallery_card_shell(
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
