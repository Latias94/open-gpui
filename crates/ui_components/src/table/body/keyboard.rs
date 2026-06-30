use open_gpui::{App, Entity, KeyDownEvent, ScrollHandle, Window, point, px};
use open_gpui_ui_core::{TableExpansionState, TableResolvedRow, TableRowId, UiPx};

use crate::geometry::{gpui_px_from_ui, ui_px_from_gpui};
use crate::table::interaction::toggle_table_expansion;
use crate::table::{
    TableInputModifiers, TableRowAction, TableRowActivation, TableRowActivationHandler,
    TableRowActivationKind, TableRowExpansionHandler, TableRowExpansionToggle, TableRowRenderPlan,
    TableRuntime, nonnegative_px,
};

#[derive(Debug, Clone, PartialEq, Eq)]
enum TableRowKeyboardAction {
    Focus { index: usize, row_id: TableRowId },
    Toggle { expanded: bool },
    Activate,
}

fn table_row_keyboard_action(
    row: &TableRowRenderPlan,
    final_rows: &[TableResolvedRow],
    key: &str,
) -> Option<TableRowKeyboardAction> {
    let current_index = row.model_index();
    match key {
        "home" if !final_rows.is_empty() => Some(TableRowKeyboardAction::Focus {
            index: 0,
            row_id: final_rows[0].id().clone(),
        }),
        "end" if !final_rows.is_empty() => {
            let index = final_rows.len() - 1;
            Some(TableRowKeyboardAction::Focus {
                index,
                row_id: final_rows[index].id().clone(),
            })
        }
        "up" => current_index.checked_sub(1).and_then(|index| {
            final_rows
                .get(index)
                .map(|target| TableRowKeyboardAction::Focus {
                    index,
                    row_id: target.id().clone(),
                })
        }),
        "down" => {
            let index = current_index + 1;
            final_rows
                .get(index)
                .map(|target| TableRowKeyboardAction::Focus {
                    index,
                    row_id: target.id().clone(),
                })
        }
        "left" if row.row().is_tree_branch() && row.row().tree_expanded() == Some(true) => {
            Some(TableRowKeyboardAction::Toggle { expanded: false })
        }
        "left" => row.row().parent_id().and_then(|parent_id| {
            final_rows
                .iter()
                .position(|candidate| candidate.id() == parent_id)
                .map(|index| TableRowKeyboardAction::Focus {
                    index,
                    row_id: parent_id.clone(),
                })
        }),
        "right" if row.row().is_tree_branch() && row.row().tree_expanded() == Some(false) => {
            Some(TableRowKeyboardAction::Toggle { expanded: true })
        }
        "right" => final_rows
            .get(current_index + 1)
            .filter(|candidate| candidate.parent_id() == Some(row.id()))
            .map(|target| TableRowKeyboardAction::Focus {
                index: current_index + 1,
                row_id: target.id().clone(),
            }),
        "enter" | "space" => Some(TableRowKeyboardAction::Activate),
        _ => None,
    }
}

#[allow(clippy::too_many_arguments)]
pub(in crate::table::body) fn handle_table_row_key_down(
    row: &TableRowRenderPlan,
    final_rows: &[TableResolvedRow],
    vertical_scroll_handle: ScrollHandle,
    top_row_count: usize,
    center_total_row_count: usize,
    runtime: &Entity<TableRuntime>,
    current_expansion: TableExpansionState,
    on_row_activate: Option<TableRowActivationHandler>,
    on_row_expansion_request: Option<TableRowExpansionHandler>,
    event: &KeyDownEvent,
    window: &mut Window,
    cx: &mut App,
) {
    if event.keystroke.modifiers.modified() {
        return;
    }

    let Some(action) = table_row_keyboard_action(row, final_rows, event.keystroke.key.as_str())
    else {
        return;
    };

    cx.stop_propagation();
    window.prevent_default();

    match action {
        TableRowKeyboardAction::Focus { index, row_id } => {
            let focus_handle = runtime.update(cx, |runtime, cx| runtime.set_focused(row_id, cx));
            if let Some(center_index) = index.checked_sub(top_row_count) {
                if center_index < center_total_row_count {
                    scroll_table_row_into_view(
                        &vertical_scroll_handle,
                        row.virtual_size(),
                        center_total_row_count,
                        center_index,
                    );
                }
            }
            if let Some(focus_handle) = focus_handle {
                focus_handle.focus(window, cx);
            }
            window.refresh();
        }
        TableRowKeyboardAction::Toggle { expanded } => {
            let next_expansion =
                toggle_table_expansion(current_expansion, row.id().clone(), expanded);
            runtime.update(cx, |runtime, cx| {
                runtime.set_focused(row.id().clone(), cx);
                runtime.set_expansion_override(next_expansion.clone(), cx);
            });
            if let Some(on_row_expansion_request) = on_row_expansion_request.as_ref() {
                let action = TableRowAction::from_render_plan(
                    row,
                    TableInputModifiers::from_gpui(event.keystroke.modifiers),
                );
                on_row_expansion_request(
                    TableRowExpansionToggle::new(action, expanded),
                    window,
                    cx,
                );
            }
            window.refresh();
        }
        TableRowKeyboardAction::Activate => {
            runtime.update(cx, |runtime, cx| {
                runtime.set_focused(row.id().clone(), cx);
            });
            if let Some(on_row_activate) = on_row_activate.as_ref() {
                let action = TableRowAction::from_render_plan(
                    row,
                    TableInputModifiers::from_gpui(event.keystroke.modifiers),
                );
                on_row_activate(
                    TableRowActivation::new(action, TableRowActivationKind::Keyboard),
                    window,
                    cx,
                );
            }
            window.refresh();
        }
    }
}

fn scroll_table_row_into_view(
    scroll_handle: &ScrollHandle,
    row_height: UiPx,
    row_count: usize,
    index: usize,
) {
    let viewport_extent = ui_px_from_gpui(scroll_handle.bounds().size.height);
    let row_height = nonnegative_px(row_height);
    if viewport_extent.as_f32() <= 0.0 || row_height.as_f32() <= 0.0 {
        return;
    }

    let total_extent = row_height * row_count as f32;
    let current_scroll_offset =
        UiPx::new((-ui_px_from_gpui(scroll_handle.offset().y).as_f32()).max(0.0));
    let row_start = row_height * index as f32;
    let row_end = row_start + row_height;
    let max_scroll = nonnegative_px(total_extent - viewport_extent);
    let target = if row_start < current_scroll_offset {
        row_start
    } else if row_end > current_scroll_offset + viewport_extent {
        row_end - viewport_extent
    } else {
        current_scroll_offset
    };
    let target = target.max(UiPx::ZERO).min(max_scroll);

    scroll_handle.set_offset(point(px(0.0), -gpui_px_from_ui(target)));
}
