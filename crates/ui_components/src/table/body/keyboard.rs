use open_gpui::{App, Entity, FocusHandle, KeyDownEvent, ScrollHandle, Window};
use open_gpui_ui_core::{TableResolvedRow, TableRowIdentity, TableRowModel, UiPx};

use crate::scroll_surface::{ScrollSurfaceRevealStrategy, reveal_fixed_row, reveal_row_geometry};
use crate::table::{
    TableInputModifiers, TableRowAction, TableRowActivation, TableRowActivationHandler,
    TableRowActivationKind, TableRowExpansionHandler, TableRowExpansionToggle, TableRowRenderPlan,
    TableRuntime,
};

#[derive(Debug, Clone, PartialEq, Eq)]
enum TableRowKeyboardAction {
    Focus {
        index: usize,
        identity: TableRowIdentity,
    },
    Toggle {
        index: usize,
        expanded: bool,
    },
    Activate {
        index: usize,
    },
}

fn table_row_keyboard_action(
    current_identity: &TableRowIdentity,
    current_index: usize,
    final_model: &TableRowModel,
    key: &str,
) -> Option<TableRowKeyboardAction> {
    let final_rows = final_model.rows();
    let row = final_rows
        .get(current_index)
        .filter(|row| row.identity() == current_identity)?;
    match key {
        "home" if !final_rows.is_empty() => Some(TableRowKeyboardAction::Focus {
            index: 0,
            identity: final_rows[0].identity().clone(),
        }),
        "end" if !final_rows.is_empty() => {
            let index = final_rows.len() - 1;
            Some(TableRowKeyboardAction::Focus {
                index,
                identity: final_rows[index].identity().clone(),
            })
        }
        "up" => current_index.checked_sub(1).and_then(|index| {
            final_rows
                .get(index)
                .map(|target| TableRowKeyboardAction::Focus {
                    index,
                    identity: target.identity().clone(),
                })
        }),
        "down" => {
            let index = current_index + 1;
            final_rows
                .get(index)
                .map(|target| TableRowKeyboardAction::Focus {
                    index,
                    identity: target.identity().clone(),
                })
        }
        "left" if row.is_tree_branch() && row.tree_expanded() == Some(true) => {
            Some(TableRowKeyboardAction::Toggle {
                index: current_index,
                expanded: false,
            })
        }
        "left" => row.parent_identity().and_then(|parent_identity| {
            final_model
                .row_index(parent_identity)
                .map(|index| TableRowKeyboardAction::Focus {
                    index,
                    identity: parent_identity.clone(),
                })
        }),
        "right" if row.is_tree_branch() && row.tree_expanded() == Some(false) => {
            Some(TableRowKeyboardAction::Toggle {
                index: current_index,
                expanded: true,
            })
        }
        "right" => final_rows
            .get(current_index + 1)
            .filter(|candidate| candidate.parent_identity() == Some(row.identity()))
            .map(|target| TableRowKeyboardAction::Focus {
                index: current_index + 1,
                identity: target.identity().clone(),
            }),
        "enter" | "space" => Some(TableRowKeyboardAction::Activate {
            index: current_index,
        }),
        _ => None,
    }
}

pub(in crate::table) struct TableKeyboardDispatchContext<'a> {
    pub(in crate::table) final_model: &'a TableRowModel,
    pub(in crate::table) vertical_scroll_handle: ScrollHandle,
    pub(in crate::table) top_row_count: usize,
    pub(in crate::table) center_total_row_count: usize,
    pub(in crate::table) fallback_row_height: UiPx,
    pub(in crate::table) fallback_viewport_extent: UiPx,
    pub(in crate::table) runtime: &'a Entity<TableRuntime>,
    pub(in crate::table) on_row_activate: Option<TableRowActivationHandler>,
    pub(in crate::table) on_row_expansion_request: Option<TableRowExpansionHandler>,
}

#[derive(Clone, Copy)]
enum TableKeyboardSource<'a> {
    RenderedRow(&'a TableRowRenderPlan),
    FocusProxy,
}

impl TableKeyboardSource<'_> {
    fn identity(self, resolved_row: &TableResolvedRow) -> TableRowIdentity {
        match self {
            Self::RenderedRow(row) => row.identity().clone(),
            Self::FocusProxy => resolved_row.identity().clone(),
        }
    }

    fn row_action(
        self,
        resolved_row: &TableResolvedRow,
        index: usize,
        modifiers: TableInputModifiers,
    ) -> TableRowAction {
        match self {
            Self::RenderedRow(row) => TableRowAction::from_render_plan(row, modifiers),
            Self::FocusProxy => TableRowAction::from_resolved_row(resolved_row, index, modifiers),
        }
    }
}

impl TableKeyboardDispatchContext<'_> {
    pub(in crate::table) fn dispatch_rendered_row(
        self,
        row: &TableRowRenderPlan,
        focus_handle: &FocusHandle,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut App,
    ) {
        if !focus_handle.is_focused(window) {
            return;
        }
        self.dispatch(TableKeyboardSource::RenderedRow(row), event, window, cx);
    }

    pub(in crate::table) fn dispatch_focus_proxy(
        self,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut App,
    ) {
        self.dispatch(TableKeyboardSource::FocusProxy, event, window, cx);
    }

    fn dispatch(
        self,
        source: TableKeyboardSource<'_>,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut App,
    ) {
        if event.keystroke.modifiers.modified() {
            return;
        }

        let proxy_identity;
        let (current_identity, current_index) = match source {
            TableKeyboardSource::RenderedRow(row) => (row.identity(), row.model_index()),
            TableKeyboardSource::FocusProxy => {
                let Some(identity) = self.runtime.read(cx).focused_row.clone() else {
                    return;
                };
                let Some(index) = self.final_model.row_index(&identity) else {
                    return;
                };
                proxy_identity = identity;
                (&proxy_identity, index)
            }
        };
        let Some(action) = table_row_keyboard_action(
            current_identity,
            current_index,
            self.final_model,
            event.keystroke.key.as_str(),
        ) else {
            return;
        };

        cx.stop_propagation();
        window.prevent_default();

        match action {
            TableRowKeyboardAction::Focus { index, identity } => {
                let focus_handle = self
                    .runtime
                    .update(cx, |runtime, cx| runtime.set_focused(identity, cx));
                if let Some(center_index) = index.checked_sub(self.top_row_count)
                    && center_index < self.center_total_row_count
                {
                    scroll_table_row_into_view(
                        &self.vertical_scroll_handle,
                        self.runtime,
                        cx,
                        self.fallback_row_height,
                        self.fallback_viewport_extent,
                        self.center_total_row_count,
                        center_index,
                    );
                }
                if let Some(focus_handle) = focus_handle {
                    focus_handle.focus(window, cx);
                }
                window.refresh();
            }
            TableRowKeyboardAction::Toggle { index, expanded } => {
                let row = &self.final_model.rows()[index];
                let identity = source.identity(row);
                self.runtime.update(cx, |runtime, cx| {
                    runtime.set_focused(identity, cx);
                });
                if let Some(on_row_expansion_request) = self.on_row_expansion_request.as_ref() {
                    let action = source.row_action(
                        row,
                        index,
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
            TableRowKeyboardAction::Activate { index } => {
                let row = &self.final_model.rows()[index];
                let identity = source.identity(row);
                self.runtime.update(cx, |runtime, cx| {
                    runtime.set_focused(identity, cx);
                });
                if let Some(on_row_activate) = self.on_row_activate.as_ref() {
                    let action = source.row_action(
                        row,
                        index,
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
}

fn scroll_table_row_into_view(
    scroll_handle: &ScrollHandle,
    runtime: &Entity<TableRuntime>,
    cx: &App,
    fallback_row_height: UiPx,
    fallback_viewport_extent: UiPx,
    row_count: usize,
    index: usize,
) {
    if let Some(virtualizer) = runtime.read(cx).center_virtualizer()
        && let Some(geometry) = virtualizer.item_geometry(index)
    {
        reveal_row_geometry(
            scroll_handle,
            ScrollSurfaceRevealStrategy::Nearest,
            geometry,
            virtualizer.total_size(),
            Some(fallback_viewport_extent),
        );
        return;
    }

    reveal_fixed_row(
        scroll_handle,
        ScrollSurfaceRevealStrategy::Nearest,
        index,
        row_count,
        fallback_row_height,
        Some(fallback_viewport_extent),
    );
}

#[cfg(test)]
mod tests {
    use open_gpui_ui_core::{TableRow, TableState};

    use super::*;

    #[test]
    fn keyboard_navigation_uses_exact_materialized_model_indices() {
        let resolved = TableState::new([
            TableRow::new("duplicate").with_instance_id("first"),
            TableRow::new("duplicate").with_instance_id("second"),
            TableRow::new("parent").with_child(TableRow::new("child")),
        ])
        .with_all_rows_expanded()
        .resolve();
        let model = resolved.final_model();
        let second = TableRowIdentity::source_instance("duplicate", "second");
        let second_index = model
            .row_index(&second)
            .expect("explicit source instance should be materialized");

        assert_eq!(
            table_row_keyboard_action(&second, second_index, model, "down"),
            Some(TableRowKeyboardAction::Focus {
                index: 2,
                identity: TableRowIdentity::source("parent"),
            })
        );

        let child = TableRowIdentity::source("child");
        let child_index = model
            .row_index(&child)
            .expect("expanded child should be materialized");
        assert_eq!(
            table_row_keyboard_action(&child, child_index, model, "left"),
            Some(TableRowKeyboardAction::Focus {
                index: 2,
                identity: TableRowIdentity::source("parent"),
            })
        );
        assert_eq!(
            table_row_keyboard_action(&second, 0, model, "down"),
            None,
            "a stale rendered index must not retarget a different exact identity"
        );
    }
}
