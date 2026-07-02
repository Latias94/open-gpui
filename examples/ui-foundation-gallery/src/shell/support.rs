//! Shared rendering support for the foundation gallery shell.

use super::*;

pub(crate) fn label_pill(label: &'static str) -> impl IntoElement {
    div()
        .px_2()
        .py_1()
        .rounded_sm()
        .border_1()
        .border_color(rgb(0xd6d8ce))
        .bg(rgb(0xf6f7f2))
        .text_xs()
        .text_color(rgb(0x3f4a57))
        .child(label)
}

pub(crate) fn component_catalog_status_pill(
    status: pages::components::ComponentCatalogStatus,
) -> impl IntoElement {
    let (background, border, text) = status.badge_colors();

    div()
        .px_2()
        .py_1()
        .rounded_sm()
        .border_1()
        .border_color(rgb(border))
        .bg(rgb(background))
        .text_xs()
        .text_color(rgb(text))
        .child(status.as_str())
}

pub(super) fn toggled_label(toggled: Toggled) -> impl IntoElement {
    div()
        .px_2()
        .py_1()
        .rounded_sm()
        .border_1()
        .border_color(rgb(0xd6d8ce))
        .bg(rgb(0xf6f7f2))
        .text_xs()
        .text_color(rgb(0x3f4a57))
        .child(toggled_label_text(toggled))
}

pub(super) fn gallery_card_shell(
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

pub(super) fn format_duration_ms(duration: std::time::Duration) -> String {
    format!("{}ms", duration.as_millis())
}

pub(super) fn toggled_label_text(toggled: Toggled) -> &'static str {
    match toggled {
        Toggled::True => "on",

        Toggled::False => "off",

        Toggled::Mixed => "mixed",
    }
}

pub(super) fn geometry_row(label: &'static str, rect: Rect) -> impl IntoElement {
    div()
        .px_3()
        .py_2()
        .rounded_sm()
        .border_1()
        .border_color(rgb(0xd6d8ce))
        .bg(rgb(0xffffff))
        .text_xs()
        .text_color(rgb(0x3f4a57))
        .child(format!(
            "{}: {}, {} / {} x {}",
            label,
            format_ui_px(rect.origin.x),
            format_ui_px(rect.origin.y),
            format_ui_px(rect.size.width),
            format_ui_px(rect.size.height)
        ))
}

pub(super) fn ui_px_from_gpui(value: Pixels) -> UiPx {
    UiPx::new(value.as_f32())
}

pub(super) fn format_ui_px(value: UiPx) -> String {
    format!("{:.0}px", value.as_f32())
}

pub(crate) trait DisplayPx {
    fn display_px(self) -> f32;
}

impl DisplayPx for Pixels {
    fn display_px(self) -> f32 {
        self.as_f32()
    }
}

impl DisplayPx for UiPx {
    fn display_px(self) -> f32 {
        self.as_f32()
    }
}

pub(crate) fn format_px(value: impl DisplayPx) -> String {
    format!("{:.0}px", value.display_px())
}

pub(super) fn bool_label(value: bool) -> &'static str {
    if value { "yes" } else { "no" }
}
