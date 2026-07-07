use crate::a11y::UiA11yElementExt;
use crate::geometry::{gpui_px_from_ui, ui_px_from_gpui};
use open_gpui::prelude::FluentBuilder;
use open_gpui::{
    AnyElement, App, ClickEvent, Entity, FocusHandle, InteractiveElement, IntoElement,
    ParentElement, Pixels, StatefulInteractiveElement, Styled, Window, div, px,
};
use open_gpui_ui_core::{Size, UiPx};

use super::descriptor::VirtualizedListRowKind;
use super::model::{VirtualizedListActivation, VirtualizedListSelectionMode, VirtualizedListState};
use super::motion::VirtualizedListActiveIndicatorSnapshot;
use super::render_plan::{
    VirtualizedListRowMeasureMode, VirtualizedListRowRenderPlan,
    VirtualizedListStickyOverlaySnapshot,
};
use super::runtime::{
    VirtualizedListActivationHandler, VirtualizedListRowRenderer, VirtualizedListRuntime,
    VirtualizedListSelectionChangeHandler,
};
use super::style::VirtualizedListColors;

pub(super) fn render_virtualized_list_body(
    list_id: &str,
    rows: &[VirtualizedListRowRenderPlan],
    total_size: UiPx,
    active_indicator: Option<VirtualizedListActiveIndicatorSnapshot>,
    sticky_overlay: Option<VirtualizedListStickyOverlaySnapshot>,
    colors: VirtualizedListColors,
    row_measure_mode: VirtualizedListRowMeasureMode,
    estimated_row_height: UiPx,
    row_renderer: Option<VirtualizedListRowRenderer>,
    list_state: VirtualizedListState,
    runtime: Entity<VirtualizedListRuntime>,
    focus_handle: FocusHandle,
    on_activate: Option<VirtualizedListActivationHandler>,
    on_selection_change: Option<VirtualizedListSelectionChangeHandler>,
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    let rows = rows.to_vec();
    let list_id = list_id.to_owned();
    let body_id = format!("virtualized-list:{list_id}:body");
    let mut row_elements = Vec::with_capacity(rows.len());
    for row in rows {
        row_elements.push(
            render_virtualized_list_row(
                list_id.clone(),
                row,
                colors,
                row_measure_mode,
                estimated_row_height,
                row_renderer.clone(),
                list_state.clone(),
                runtime.clone(),
                focus_handle.clone(),
                on_activate.clone(),
                on_selection_change.clone(),
                window,
                cx,
            )
            .into_any_element(),
        );
    }

    div()
        .id(body_id.clone())
        .debug_selector({
            let body_id = body_id.clone();
            move || body_id.clone()
        })
        .relative()
        .w_full()
        .h(gpui_px_from_ui(total_size))
        .children(row_elements)
        .when_some(active_indicator, |this, indicator| {
            this.child(render_virtualized_list_active_indicator(
                indicator, colors, cx,
            ))
        })
        .when_some(sticky_overlay, |this, overlay| {
            this.child(render_virtualized_list_sticky_overlay(
                list_id,
                overlay,
                colors,
                estimated_row_height,
                cx,
            ))
        })
        .into_any_element()
}

fn render_virtualized_list_active_indicator(
    indicator: VirtualizedListActiveIndicatorSnapshot,
    colors: VirtualizedListColors,
    cx: &App,
) -> AnyElement {
    let theme = crate::theme::ThemeResolver::current(cx);
    let indicator_color = if indicator.frame_demand().needs_frame() {
        theme.resolve(colors.active_indicator_moving())
    } else {
        theme.resolve(colors.active_indicator())
    };

    div()
        .absolute()
        .top(gpui_px_from_ui(indicator.top()))
        .left(px(0.0))
        .w(px(3.0))
        .h(gpui_px_from_ui(indicator.height()))
        .rounded(px(2.0))
        .bg(indicator_color)
        .into_any_element()
}

fn render_virtualized_list_sticky_overlay(
    list_id: String,
    overlay: VirtualizedListStickyOverlaySnapshot,
    colors: VirtualizedListColors,
    estimated_row_height: UiPx,
    cx: &App,
) -> AnyElement {
    let theme = crate::theme::ThemeResolver::current(cx);
    let section = overlay.section().clone();
    let key = section.key().to_owned();
    let label = section.label().to_owned();

    div()
        .id(format!("virtualized-list:{list_id}:sticky-overlay:{key}"))
        .debug_selector({
            let list_id = list_id.clone();
            let key = key.clone();
            move || format!("virtualized-list:{list_id}:sticky-overlay:{key}")
        })
        .absolute()
        .top(px(0.0))
        .left(px(0.0))
        .right(px(0.0))
        .h(gpui_px_from_ui(estimated_row_height))
        .flex()
        .items_center()
        .px(px(10.0))
        .border_b_1()
        .border_color(theme.resolve(colors.border()))
        .bg(theme.resolve(colors.sticky_overlay_background()))
        .text_color(theme.resolve(colors.sticky_overlay_foreground()))
        .child(label)
        .into_any_element()
}

fn render_virtualized_list_row(
    list_id: String,
    row: VirtualizedListRowRenderPlan,
    colors: VirtualizedListColors,
    row_measure_mode: VirtualizedListRowMeasureMode,
    estimated_row_height: UiPx,
    row_renderer: Option<VirtualizedListRowRenderer>,
    list_state: VirtualizedListState,
    runtime: Entity<VirtualizedListRuntime>,
    focus_handle: FocusHandle,
    on_activate: Option<VirtualizedListActivationHandler>,
    on_selection_change: Option<VirtualizedListSelectionChangeHandler>,
    window: &mut Window,
    cx: &mut App,
) -> impl IntoElement {
    let render_key = row.render_key().to_owned();
    let target = row.target();
    let activation = VirtualizedListActivation::from_target(target.clone(), row.selected());
    let row_kind = row.item().kind();
    let primary_text = row.label().to_owned();
    let secondary_text = row.item().secondary_text_ref().map(str::to_owned);
    let leading_metadata = row.item().leading_metadata_ref().map(str::to_owned);
    let trailing_metadata = row.item().trailing_metadata_ref().map(str::to_owned);
    let badge = row.item().badge_ref().map(str::to_owned);
    let status = row.item().status_ref().map(str::to_owned);
    let retry_action_label = row.item().retry_action_label_ref().map(str::to_owned);
    let theme = crate::theme::ThemeResolver::current(cx);
    let row_background = if row.selected() {
        theme.resolve(colors.row_selected_background())
    } else if row.active() {
        theme.resolve(colors.row_active_background())
    } else if row.index().is_multiple_of(2) {
        theme.resolve(colors.row_background())
    } else {
        theme.resolve(colors.row_alternate_background())
    };
    let text_color = if row.disabled() {
        theme.resolve(colors.row_disabled_foreground())
    } else {
        theme.resolve(colors.foreground())
    };
    let row_content = if let Some(row_renderer) = row_renderer.as_ref() {
        row_renderer(row.render_context(row_measure_mode), window, cx)
    } else {
        render_default_virtualized_list_row_content(
            row_kind,
            primary_text,
            secondary_text,
            leading_metadata,
            trailing_metadata,
            badge,
            status,
            retry_action_label,
            colors,
            cx,
        )
    };

    div()
        .on_children_prepainted({
            let runtime = runtime.clone();
            let render_key = render_key.clone();
            move |row_bounds, _window, cx| {
                if row_measure_mode.measured() {
                    let measured_height = row_bounds
                        .iter()
                        .map(|bounds| bounds.size.height)
                        .fold(Pixels::ZERO, Pixels::max);
                    let measured_height = measured_height.ceil();
                    runtime.update(cx, |runtime, cx| {
                        runtime.set_row_measurement(
                            render_key.clone(),
                            ui_px_from_gpui(measured_height),
                            cx,
                        );
                    });
                }
            }
        })
        .id(format!("virtualized-list:{list_id}:row:{render_key}"))
        .debug_selector({
            let list_id = list_id.clone();
            let render_key = render_key.clone();
            move || format!("virtualized-list:{list_id}:row:{render_key}")
        })
        .absolute()
        .top(gpui_px_from_ui(row.virtual_start()))
        .left(px(0.0))
        .right(px(0.0))
        .when(row_measure_mode.measured(), |this| {
            this.min_h(gpui_px_from_ui(estimated_row_height))
        })
        .when(!row_measure_mode.measured(), |this| {
            this.h(gpui_px_from_ui(row.virtual_size()))
        })
        .min_w(px(0.0))
        .flex()
        .items_center()
        .overflow_hidden()
        .border_b_1()
        .border_color(theme.resolve(colors.border()))
        .bg(row_background)
        .text_color(text_color)
        .ui_role(row.role())
        .aria_selected(row.selected())
        .aria_disabled(row.disabled())
        .when_some(row.position_in_set(), |this, position| {
            this.aria_position_in_set(position)
        })
        .when(!row.disabled(), |this| {
            this.cursor_pointer()
                .hover(move |style| style.bg(theme.resolve(colors.row_hover_background())))
        })
        .when(!row.disabled(), |this| {
            let runtime = runtime.clone();
            let focus_handle = focus_handle.clone();
            let on_activate = on_activate.clone();
            let on_selection_change = on_selection_change.clone();
            let list_state = list_state.clone();
            let target = target.clone();
            let activation = activation.clone();
            let activate_on_click =
                list_state.selection_mode() == VirtualizedListSelectionMode::Single;
            this.on_click(move |event: &ClickEvent, window, cx| {
                cx.stop_propagation();
                window.prevent_default();
                let shift_range = event.modifiers().shift
                    && list_state.selection_mode() == VirtualizedListSelectionMode::Multiple;
                let runtime_anchor = runtime.read(cx).selection_anchor_key.clone();
                let anchor_key = shift_range
                    .then(|| {
                        list_state
                            .range_anchor_key(runtime_anchor.as_deref(), target.key())
                            .map(str::to_owned)
                    })
                    .flatten();
                let selection_change = if shift_range {
                    list_state.range_selection_change(anchor_key.as_deref(), target.key())
                } else {
                    list_state.selection_change_for_target(&target)
                };
                runtime.update(cx, |runtime, _| {
                    runtime.active_key = Some(target.key().to_owned());
                    runtime.selection_anchor_key = if shift_range {
                        anchor_key.clone().or_else(|| Some(target.key().to_owned()))
                    } else {
                        Some(target.key().to_owned())
                    };
                    if let Some(selection_change) = selection_change.as_ref() {
                        runtime.selected_keys = selection_change.selected_key_set();
                    }
                    runtime.pending_scroll_to_active = None;
                });
                focus_handle.focus(window, cx);
                if let (Some(on_selection_change), Some(selection_change)) =
                    (on_selection_change.as_ref(), selection_change)
                {
                    on_selection_change(selection_change, window, cx);
                }
                if activate_on_click && let Some(on_activate) = on_activate.as_ref() {
                    on_activate(activation.clone(), window, cx);
                }
            })
        })
        .child(row_content)
}

fn render_default_virtualized_list_row_content(
    row_kind: VirtualizedListRowKind,
    primary_text: String,
    secondary_text: Option<String>,
    leading_metadata: Option<String>,
    trailing_metadata: Option<String>,
    badge: Option<String>,
    status: Option<String>,
    retry_action_label: Option<String>,
    colors: VirtualizedListColors,
    cx: &App,
) -> AnyElement {
    let theme = crate::theme::ThemeResolver::current(cx);
    if row_kind == VirtualizedListRowKind::Separator {
        return div()
            .mx(px(8.0))
            .h(px(1.0))
            .w_full()
            .bg(theme.resolve(colors.separator()))
            .into_any_element();
    }

    div()
        .w_full()
        .min_w(px(0.0))
        .px(px(10.0))
        .flex()
        .items_center()
        .gap_2()
        .when_some(leading_metadata, |this, metadata| {
            this.child(
                div()
                    .text_color(theme.resolve(colors.muted_foreground()))
                    .text_size(gpui_px_from_ui(Size::XSmall.control_text_px()))
                    .child(metadata),
            )
        })
        .child(
            div()
                .flex_1()
                .min_w(px(0.0))
                .flex()
                .flex_col()
                .child(primary_text)
                .when_some(secondary_text, |this, secondary_text| {
                    this.child(
                        div()
                            .text_color(theme.resolve(colors.muted_foreground()))
                            .text_size(gpui_px_from_ui(Size::XSmall.control_text_px()))
                            .child(secondary_text),
                    )
                }),
        )
        .when_some(badge, |this, badge| {
            this.child(
                div()
                    .rounded(px(4.0))
                    .bg(theme.resolve(colors.badge_background()))
                    .px_1()
                    .text_color(theme.resolve(colors.badge_foreground()))
                    .text_size(gpui_px_from_ui(Size::XSmall.control_text_px()))
                    .child(badge),
            )
        })
        .when_some(status, |this, status| {
            this.child(
                div()
                    .text_color(theme.resolve(colors.status_foreground()))
                    .text_size(gpui_px_from_ui(Size::XSmall.control_text_px()))
                    .child(status),
            )
        })
        .when_some(retry_action_label, |this, action_label| {
            this.child(
                div()
                    .rounded(px(4.0))
                    .border_1()
                    .border_color(theme.resolve(colors.border()))
                    .px_1()
                    .text_color(theme.resolve(colors.status_foreground()))
                    .text_size(gpui_px_from_ui(Size::XSmall.control_text_px()))
                    .child(action_label),
            )
        })
        .when_some(trailing_metadata, |this, metadata| {
            this.child(
                div()
                    .text_color(theme.resolve(colors.muted_foreground()))
                    .text_size(gpui_px_from_ui(Size::XSmall.control_text_px()))
                    .child(metadata),
            )
        })
        .into_any_element()
}
