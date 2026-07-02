//! Overlay page sample rendering helpers for the foundation gallery shell.

use super::support::{bool_label, format_duration_ms, format_px, format_ui_px, gallery_card_shell};
use super::*;

pub(super) fn overlay_sample_card_shell(
    id: impl Into<open_gpui::ElementId>,

    debug_selector: Option<String>,
) -> open_gpui::Stateful<open_gpui::Div> {
    gallery_card_shell(id, debug_selector)
        .min_w(px(0.0))
        .flex()
        .flex_col()
        .gap_3()
        .text_xs()
        .text_color(rgb(0x3f4a57))
}

pub(super) fn overlay_behavior_card(
    sample: &pages::overlay::OverlayBehaviorSample,
) -> impl IntoElement {
    let policy = &sample.policy;

    let resolved = OverlayResolvedState::resolve(policy.clone());

    let adapter = gpui_overlay_state(&resolved);

    let presence = policy.presence();

    let layer_state = policy.layer_state();

    let outside = policy.outside_press_policy().resolve();

    div()
        .id(format!("overlay-behavior:{}", sample.id))
        .flex()
        .flex_col()
        .gap_2()
        .rounded_sm()
        .border_1()
        .border_color(rgb(0xd6d8ce))
        .bg(rgb(0xffffff))
        .p_3()
        .text_xs()
        .child(
            div()
                .text_sm()
                .font_weight(open_gpui::FontWeight::BOLD)
                .text_color(rgb(0x24313f))
                .child(sample.label),
        )
        .child(format!("kind: {}", policy.kind().as_str()))
        .child(format!(
            "presence: open {} / present {} / interactive {}",
            bool_label(presence.is_open()),
            bool_label(presence.present()),
            bool_label(presence.interactive())
        ))
        .child(format!(
            "outside: {}",
            policy.outside_press_policy().as_str()
        ))
        .child(format!("escape: {}", policy.escape_key_policy().as_str()))
        .child(format!(
            "focus: open {} / close {}",
            policy.initial_focus_intent().as_str(),
            policy.focus_restore_intent().as_str()
        ))
        .child(format!(
            "layer: visible {} / hit {} / underlay {} / outside {}",
            bool_label(layer_state.visible()),
            bool_label(layer_state.hit_testable()),
            bool_label(layer_state.blocks_underlay_input()),
            bool_label(layer_state.wants_outside_press())
        ))
        .child(format!(
            "outside outcome: dismiss {} / consume {} / underlay {}",
            bool_label(outside.dismisses()),
            bool_label(outside.consumes_event()),
            bool_label(outside.allows_underlay_dispatch())
        ))
        .child(format!(
            "gpui: priority {} / margin {} / layer {} / outside handler {}",
            adapter.deferred_priority(),
            format_px(adapter.snap_margin()),
            bool_label(adapter.should_render_deferred_layer()),
            bool_label(adapter.wants_outside_press_handler())
        ))
}

pub(super) fn overlay_catalog_card(
    entry: &pages::overlay::OverlayCatalogEntry,
) -> impl IntoElement {
    let (status_bg, status_border, status_text) = entry.status.badge_colors();
    let catalog_selector = entry.catalog_selector();
    let gates = entry.behavior_gates.join(" / ");

    div()
        .id(catalog_selector)
        .debug_selector(move || catalog_selector.into())
        .w(px(260.0))
        .min_h(px(164.0))
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
                        .text_color(rgb(0x24313f))
                        .child(entry.name),
                )
                .child(
                    div()
                        .px_2()
                        .py_1()
                        .rounded_sm()
                        .border_1()
                        .border_color(rgb(status_border))
                        .bg(rgb(status_bg))
                        .text_color(rgb(status_text))
                        .text_xs()
                        .child(entry.status.as_str()),
                ),
        )
        .child(
            div()
                .text_xs()
                .text_color(rgb(0x5a6472))
                .child(format!("family: {}", entry.family)),
        )
        .child(
            div()
                .text_xs()
                .text_color(rgb(0x5a6472))
                .child(format!("state: {}", entry.state)),
        )
        .child(
            div()
                .text_xs()
                .text_color(rgb(0x5a6472))
                .child(entry.coverage),
        )
        .child(
            div()
                .text_xs()
                .line_height(px(18.0))
                .text_color(rgb(0x5a6472))
                .child(format!("gates: {gates}")),
        )
        .child(
            div()
                .text_xs()
                .line_height(px(18.0))
                .text_color(rgb(0x5a6472))
                .child(format!("selector: {}", entry.sample_selector)),
        )
}

pub(super) fn tooltip_state_row(
    state: &open_gpui_ui_components::TooltipState,

    open: bool,
) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .gap_1()
        .text_xs()
        .text_color(rgb(0x5a6472))
        .child(format!(
            "state: open {} / intent {} / content {}",
            bool_label(open),
            state.open_intent().as_str(),
            state.content_kind().as_str()
        ))
        .child(format!(
            "placement: {} {} / disabled {} / descriptive {}",
            state.overlay().policy().kind().as_str(),
            state.placement_side().as_str(),
            bool_label(state.disabled()),
            bool_label(state.descriptive())
        ))
        .child(format!(
            "delay: open {} / close {} / skip {}",
            format_duration_ms(state.delay().open_delay()),
            format_duration_ms(state.delay().close_delay()),
            format_duration_ms(state.delay().skip_delay())
        ))
}

pub(super) fn hover_card_state_row(
    state: &open_gpui_ui_components::HoverCardState,

    effective_open: bool,
) -> impl IntoElement {
    let outside = state.outside_press_policy().resolve();

    div()
        .flex()
        .flex_col()
        .gap_1()
        .text_xs()
        .text_color(rgb(0x5a6472))
        .child(format!(
            "state: open {} / mode {} / intent {}",
            bool_label(effective_open),
            state.open_mode().as_str(),
            state.open_intent().as_str()
        ))
        .child(format!(
            "placement: {} {} / interactive {} / descriptive {}",
            state.placement_side().as_str(),
            state.placement_alignment().as_str(),
            bool_label(state.interactive_content()),
            bool_label(state.descriptive())
        ))
        .child(format!(
            "delay: open {} / close {} / trigger selected {}",
            format_duration_ms(state.delay().open_delay()),
            format_duration_ms(state.delay().close_delay()),
            bool_label(state.trigger_selected())
        ))
        .child(format!(
            "outside: {} / dismiss {} / consume {} / underlay {}",
            state.outside_press_policy().as_str(),
            bool_label(outside.dismisses()),
            bool_label(outside.consumes_event()),
            bool_label(outside.allows_underlay_dispatch())
        ))
}

pub(super) fn popover_state_row(state: &open_gpui_ui_components::PopoverState) -> impl IntoElement {
    let outside = state.outside_press_policy().resolve();

    div()
        .flex()
        .flex_col()
        .gap_1()
        .text_xs()
        .text_color(rgb(0x5a6472))
        .child(format!(
            "state: open {} / mode {} / disabled {}",
            bool_label(state.open()),
            state.open_mode().as_str(),
            bool_label(state.disabled())
        ))
        .child(format!(
            "placement: {} {} / trigger selected {}",
            state.placement_side().as_str(),
            state.placement_alignment().as_str(),
            bool_label(state.trigger_selected())
        ))
        .child(format!(
            "outside: {} / dismiss {} / consume {} / underlay {}",
            state.outside_press_policy().as_str(),
            bool_label(outside.dismisses()),
            bool_label(outside.consumes_event()),
            bool_label(outside.allows_underlay_dispatch())
        ))
        .child(format!(
            "focus: open {} / close {} / layer {}",
            state.initial_focus_intent().as_str(),
            state.focus_restore_intent().as_str(),
            state.overlay().policy().kind().as_str()
        ))
}

pub(super) fn dialog_state_row(state: &open_gpui_ui_components::DialogState) -> impl IntoElement {
    let layer_state = state.overlay().layer_state();

    let outside = state.outside_press_policy().resolve();

    div()
        .flex()
        .flex_col()
        .gap_1()
        .text_xs()
        .text_color(rgb(0x5a6472))
        .child(format!(
            "state: open {} / mode {} / disabled {}",
            bool_label(state.open()),
            state.open_mode().as_str(),
            bool_label(state.disabled())
        ))
        .child(format!(
            "title: {} / description {} / trigger selected {}",
            state.title(),
            bool_label(state.description().is_some()),
            bool_label(state.trigger_selected())
        ))
        .child(format!(
            "outside: {} / dismiss {} / consume {} / underlay {}",
            state.outside_press_policy().as_str(),
            bool_label(outside.dismisses()),
            bool_label(outside.consumes_event()),
            bool_label(outside.allows_underlay_dispatch())
        ))
        .child(format!(
            "escape: {} / blocks underlay {} / layer {}",
            state.escape_key_policy().as_str(),
            bool_label(layer_state.blocks_underlay_input()),
            state.overlay().policy().kind().as_str()
        ))
}

pub(super) fn alert_dialog_state_row(
    state: &open_gpui_ui_components::AlertDialogState,
) -> impl IntoElement {
    let layer_state = state.overlay().layer_state();

    let outside = state.outside_press_policy().resolve();

    div()
        .flex()
        .flex_col()
        .gap_1()
        .text_xs()
        .text_color(rgb(0x5a6472))
        .child(format!(
            "state: open {} / mode {} / intent {}",
            bool_label(state.open()),
            state.open_mode().as_str(),
            state.intent().as_str()
        ))
        .child(format!(
            "actions: cancel {} / action {} / cancel focus {}",
            state.cancel().label(),
            state.action().label(),
            bool_label(state.cancel().default_focus())
        ))
        .child(format!(
            "outside: {} / dismiss {} / consume {} / underlay {}",
            state.outside_press_policy().as_str(),
            bool_label(outside.dismisses()),
            bool_label(outside.consumes_event()),
            bool_label(outside.allows_underlay_dispatch())
        ))
        .child(format!(
            "escape: {} / blocks underlay {} / role alert {}",
            state.escape_key_policy().as_str(),
            bool_label(layer_state.blocks_underlay_input()),
            bool_label(state.content_role() == Role::AlertDialog)
        ))
}

pub(super) fn sheet_state_row(state: &open_gpui_ui_components::SheetState) -> impl IntoElement {
    let layer_state = state.overlay().layer_state();

    let outside = state.outside_press_policy().resolve();

    div()
        .flex()
        .flex_col()
        .gap_1()
        .text_xs()
        .text_color(rgb(0x5a6472))
        .child(format!(
            "state: open {} / mode {} / side {}",
            bool_label(state.open()),
            state.open_mode().as_str(),
            state.side().as_str()
        ))
        .child(format!(
            "surface: {} / close {} / title {}",
            state.modal_mode().as_str(),
            bool_label(state.close_affordance().visible()),
            state.title()
        ))
        .child(format!(
            "outside: {} / dismiss {} / consume {} / underlay {}",
            state.outside_press_policy().as_str(),
            bool_label(outside.dismisses()),
            bool_label(outside.consumes_event()),
            bool_label(outside.allows_underlay_dispatch())
        ))
        .child(format!(
            "escape: {} / blocks underlay {} / layer {}",
            state.escape_key_policy().as_str(),
            bool_label(layer_state.blocks_underlay_input()),
            state.overlay().policy().kind().as_str()
        ))
}

pub(super) fn menu_state_row(state: &open_gpui_ui_components::MenuState) -> impl IntoElement {
    let outside = state.outside_press_policy().resolve();

    let focused = state.focused_value().unwrap_or("none");

    let active_items = state.items().iter().filter(|item| item.focusable()).count();

    div()
        .flex()
        .flex_col()
        .gap_1()
        .text_xs()
        .text_color(rgb(0x5a6472))
        .child(format!(
            "state: open {} / mode {} / disabled {}",
            bool_label(state.open()),
            state.open_mode().as_str(),
            bool_label(state.disabled())
        ))
        .child(format!(
            "items: {} / active {} / focused {}",
            state.items().len(),
            active_items,
            focused
        ))
        .child(format!(
            "outside: {} / dismiss {} / consume {}",
            state.outside_press_policy().as_str(),
            bool_label(outside.dismisses()),
            bool_label(outside.consumes_event())
        ))
        .child(format!(
            "escape: {} / layer {}",
            state.escape_key_policy().as_str(),
            state.overlay().policy().kind().as_str()
        ))
}

pub(super) fn context_menu_state_row(
    state: &open_gpui_ui_components::ContextMenuState,
) -> impl IntoElement {
    let menu = state.menu();

    let focused = menu.focused_value().unwrap_or("none");

    div()
        .flex()
        .flex_col()
        .gap_1()
        .text_xs()
        .text_color(rgb(0x5a6472))
        .child(format!(
            "state: open {} / mode {} / focused {}",
            bool_label(state.open()),
            state.open_mode().as_str(),
            focused
        ))
        .child(format!(
            "anchor: {} x {} / snap {}",
            format_ui_px(state.anchor_point().x),
            format_ui_px(state.anchor_point().y),
            format_px(DEFAULT_OVERLAY_SAFE_MARGIN)
        ))
        .child(format!(
            "items: {} / layer {} / outside {}",
            menu.items().len(),
            state.overlay().policy().kind().as_str(),
            menu.outside_press_policy().as_str()
        ))
}

pub(super) fn resolved_menu_items(
    items: &[open_gpui_ui_components::MenuItemState],
) -> Vec<MenuItem> {
    items
        .iter()
        .map(|item_state| match item_state.kind() {
            open_gpui_ui_components::MenuItemKind::Separator => {
                MenuItem::separator(item_state.value())
            }

            open_gpui_ui_components::MenuItemKind::Action => {
                MenuItem::action(item_state.value(), item_state.label().to_owned())
                    .disabled(item_state.disabled())
            }
            open_gpui_ui_components::MenuItemKind::Checkbox => MenuItem::checkbox(
                item_state.value(),
                item_state.label().to_owned(),
                item_state.checked(),
            )
            .disabled(item_state.disabled()),
            open_gpui_ui_components::MenuItemKind::Radio => MenuItem::radio(
                item_state.value(),
                item_state.label().to_owned(),
                item_state.checked(),
            )
            .disabled(item_state.disabled()),
            open_gpui_ui_components::MenuItemKind::Submenu => MenuItem::submenu(
                item_state.value(),
                item_state.label().to_owned(),
                resolved_menu_items(item_state.children()),
            )
            .disabled(item_state.disabled()),
        })
        .collect()
}
