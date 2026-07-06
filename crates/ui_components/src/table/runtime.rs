use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;

use open_gpui::{Context, FocusHandle};
use open_gpui_ui_core::{
    TableColumnResizeState, TableExpansionState, TableResolvedState, TableRowId,
    TableStateCacheKey, UiPx,
};

use crate::scroll_surface::ScrollSurfaceRuntime;

use super::content_fit::TableContentFitMeasureCache;
use super::{TableColumnRenderPlan, TableRenderPlan, nonnegative_px};

#[derive(Debug, Clone)]
pub(super) struct TableResolvedCache {
    pub(super) key: TableStateCacheKey,
    pub(super) table: Rc<TableResolvedState>,
    pub(super) columns: Vec<TableColumnRenderPlan>,
}

#[derive(Debug, Clone, Default)]
pub(super) struct TableRuntime {
    pub(super) scroll_surface: ScrollSurfaceRuntime,
    pub(super) horizontal_scroll_surface: ScrollSurfaceRuntime,
    pub(super) resolved: Option<TableResolvedCache>,
    pub(super) content_fit: TableContentFitMeasureCache,
    pub(super) row_measurements: BTreeMap<String, UiPx>,
    pub(super) column_resize: TableColumnResizeState,
    pub(super) focused_row: Option<TableRowId>,
    pub(super) focus_handles: BTreeMap<TableRowId, FocusHandle>,
    pub(super) expansion_override: Option<TableExpansionState>,
    pub(super) selection_anchor: Option<TableRowId>,
}

impl TableRuntime {
    pub(super) fn new(default_focused_row: Option<TableRowId>) -> Self {
        Self {
            scroll_surface: ScrollSurfaceRuntime::new(None),
            horizontal_scroll_surface: ScrollSurfaceRuntime::new(None),
            resolved: None,
            content_fit: TableContentFitMeasureCache::default(),
            row_measurements: BTreeMap::new(),
            column_resize: TableColumnResizeState::default(),
            focused_row: default_focused_row,
            focus_handles: BTreeMap::new(),
            expansion_override: None,
            selection_anchor: None,
        }
    }

    pub(super) fn sync_rows(&mut self, plan: &TableRenderPlan, cx: &mut Context<Self>) {
        let rendered_row_ids = plan
            .rendered_rows()
            .map(|row| row.id().clone())
            .collect::<BTreeSet<_>>();
        self.focus_handles
            .retain(|row_id, _| rendered_row_ids.contains(row_id));

        for row in plan.rendered_rows() {
            self.focus_handles
                .entry(row.id().clone())
                .or_insert_with(|| cx.focus_handle());
        }

        if self.focused_row.is_none() {
            self.focused_row = plan.rendered_rows().next().map(|row| row.id().clone());
        }
    }

    pub(super) fn set_focused(
        &mut self,
        row_id: TableRowId,
        cx: &mut Context<Self>,
    ) -> Option<FocusHandle> {
        let changed = self.focused_row.as_ref() != Some(&row_id);
        self.focused_row = Some(row_id.clone());
        if changed {
            cx.notify();
        }
        self.focus_handles.get(&row_id).cloned()
    }

    pub(super) fn set_expansion_override(
        &mut self,
        expansion: TableExpansionState,
        cx: &mut Context<Self>,
    ) {
        if self.expansion_override.as_ref() != Some(&expansion) {
            self.expansion_override = Some(expansion);
            self.resolved = None;
            cx.notify();
        }
    }

    pub(super) fn set_row_measurement(
        &mut self,
        render_key: String,
        height: UiPx,
        cx: &mut Context<Self>,
    ) {
        let height = nonnegative_px(height);
        if self.row_measurements.get(&render_key).copied() != Some(height) {
            self.row_measurements.insert(render_key, height);
            cx.notify();
        }
    }

    pub(super) fn clear_row_measurements(&mut self) {
        self.row_measurements.clear();
    }

    pub(super) fn set_selection_anchor(
        &mut self,
        row_id: Option<TableRowId>,
        cx: &mut Context<Self>,
    ) {
        if self.selection_anchor != row_id {
            self.selection_anchor = row_id;
            cx.notify();
        }
    }
}
