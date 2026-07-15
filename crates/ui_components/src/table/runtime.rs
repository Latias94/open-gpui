use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;

use open_gpui::{Context, FocusHandle, Window};
use open_gpui_ui_core::virtualizer::VirtualizerGeometryCache;
use open_gpui_ui_core::{
    TableColumnResizeState, TableExpansionState, TableResolvedRow, TableResolvedState,
    TableRowIdentity, TableRowModel, TableStateCacheKey, UiPx, VirtualizerResolvedState,
};

use crate::scroll_surface::ScrollSurfaceRuntime;

use super::content_fit::TableContentFitMeasureCache;
use super::virtualization::{
    TableRowMeasurementLookup, measured_virtualizer_state_cached, valid_snapshot_size,
};
use super::{TableColumnRenderPlan, TableRenderPlan, TableVirtualizerSnapshot, nonnegative_px};

#[derive(Debug, Clone)]
pub(super) struct TableResolvedCache {
    pub(super) key: TableStateCacheKey,
    pub(super) table: Rc<TableResolvedState>,
    pub(super) columns: Vec<TableColumnRenderPlan>,
}

pub(super) struct TableRuntime {
    pub(super) scroll_surface: ScrollSurfaceRuntime,
    pub(super) horizontal_scroll_surface: ScrollSurfaceRuntime,
    pub(super) resolved: Option<TableResolvedCache>,
    pub(super) content_fit: TableContentFitMeasureCache,
    row_measurements: BTreeMap<TableRowIdentity, TableRowMeasurementEntry>,
    pub(super) column_resize: TableColumnResizeState,
    pub(super) focused_row: Option<TableRowIdentity>,
    pending_focus_intent: bool,
    pub(super) focus_handles: BTreeMap<TableRowIdentity, FocusHandle>,
    focus_proxy: FocusHandle,
    applied_snapshot: Option<AppliedTableVirtualizerSnapshot>,
    resolved_model_revision: TableRuntimeRevision,
    row_geometry_cache: VirtualizerGeometryCache,
    row_geometry_revision: TableRuntimeRevision,
    center_virtualizer: Option<VirtualizerResolvedState>,
    pub(super) expansion_override: Option<TableExpansionState>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TableRowMeasurementProvenance {
    Snapshot,
    Live,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct TableRowMeasurementEntry {
    size: UiPx,
    provenance: TableRowMeasurementProvenance,
}

impl TableRowMeasurementEntry {
    const fn snapshot(size: UiPx) -> Self {
        Self {
            size,
            provenance: TableRowMeasurementProvenance::Snapshot,
        }
    }

    const fn live(size: UiPx) -> Self {
        Self {
            size,
            provenance: TableRowMeasurementProvenance::Live,
        }
    }

    const fn is_live(self) -> bool {
        matches!(self.provenance, TableRowMeasurementProvenance::Live)
    }
}

#[derive(Debug, Clone, PartialEq)]
struct AppliedTableVirtualizerSnapshot {
    resolved_model_revision: TableRuntimeRevision,
    snapshot: Option<TableVirtualizerSnapshot>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct TableRuntimeRevision(u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TableRuntimeRevisionAdvance {
    Advanced,
    Wrapped,
}

impl TableRuntimeRevision {
    const fn value(self) -> u64 {
        self.0
    }

    fn advance(&mut self) -> TableRuntimeRevisionAdvance {
        let Some(next) = self.0.checked_add(1) else {
            self.0 = 0;
            return TableRuntimeRevisionAdvance::Wrapped;
        };

        self.0 = next;
        TableRuntimeRevisionAdvance::Advanced
    }
}

#[derive(Debug)]
pub(super) struct TableRuntimeRenderSnapshot {
    focused_row: Option<TableRowIdentity>,
    focus_handles: BTreeMap<TableRowIdentity, FocusHandle>,
    focus_proxy: Option<FocusHandle>,
}

impl TableRuntimeRenderSnapshot {
    pub(super) fn is_focused(&self, identity: &TableRowIdentity) -> bool {
        self.focused_row.as_ref() == Some(identity)
    }

    pub(super) fn focus_handle(&self, identity: &TableRowIdentity) -> Option<FocusHandle> {
        self.focus_handles.get(identity).cloned()
    }

    pub(super) fn focus_proxy(&self) -> Option<FocusHandle> {
        self.focus_proxy.clone()
    }
}

impl TableRuntime {
    pub(super) fn new(
        default_focused_row: Option<TableRowIdentity>,
        focus_proxy: FocusHandle,
    ) -> Self {
        Self {
            scroll_surface: ScrollSurfaceRuntime::new(None),
            horizontal_scroll_surface: ScrollSurfaceRuntime::new(None),
            resolved: None,
            content_fit: TableContentFitMeasureCache::default(),
            row_measurements: BTreeMap::new(),
            column_resize: TableColumnResizeState::default(),
            focused_row: default_focused_row,
            pending_focus_intent: false,
            focus_handles: BTreeMap::new(),
            focus_proxy,
            applied_snapshot: None,
            resolved_model_revision: TableRuntimeRevision::default(),
            row_geometry_cache: VirtualizerGeometryCache::default(),
            row_geometry_revision: TableRuntimeRevision::default(),
            center_virtualizer: None,
            expansion_override: None,
        }
    }

    pub(super) fn sync_rows(
        &mut self,
        plan: &TableRenderPlan,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.center_virtualizer = plan
            .row_measure_mode()
            .measured()
            .then(|| plan.virtualizer().clone());
        let final_model = plan.table().final_model();
        let rendered_rows = plan
            .rendered_rows()
            .map(|row| row.identity().clone())
            .collect::<Vec<_>>();
        let rendered_row_ids = rendered_rows.iter().cloned().collect::<BTreeSet<_>>();
        let focused_rendered_row = self.focus_handles.iter().find_map(|(identity, handle)| {
            handle
                .contains_focused(window, cx)
                .then(|| identity.clone())
        });
        let table_owned_focus =
            self.focus_proxy.is_focused(window) || focused_rendered_row.is_some();

        let focused_descendant_remains_rendered = focused_rendered_row
            .as_ref()
            .is_some_and(|identity| rendered_row_ids.contains(identity));
        let pending_focus_intent = std::mem::take(&mut self.pending_focus_intent);
        let focused_row_authority = if pending_focus_intent {
            self.focused_row.as_ref()
        } else if focused_descendant_remains_rendered {
            focused_rendered_row.as_ref()
        } else {
            self.focused_row.as_ref()
        };
        let next_focused = reconcile_focused_row(focused_row_authority, final_model);
        if self.focused_row != next_focused {
            self.focused_row = next_focused;
            cx.notify();
        }

        self.focus_handles
            .retain(|identity, _| rendered_row_ids.contains(identity));

        for identity in &rendered_rows {
            if !self.focus_handles.contains_key(identity) {
                self.focus_handles
                    .insert(identity.clone(), cx.focus_handle());
            }
        }

        // The focus tree describes the previous frame. Preserve it only when it agrees with the
        // resolved authority; otherwise hand off to the explicit logical intent or proxy.
        let focused_descendant_is_authoritative = focused_descendant_remains_rendered
            && focused_rendered_row.as_ref() == self.focused_row.as_ref();
        if table_owned_focus && !focused_descendant_is_authoritative {
            match self.focused_row.as_ref() {
                Some(identity) => {
                    let target = self
                        .focus_handles
                        .get(identity)
                        .unwrap_or(&self.focus_proxy);
                    if !target.is_focused(window) {
                        target.focus(window, cx);
                    }
                }
                None => window.blur(),
            }
        }
    }

    pub(super) fn render_snapshot(&self) -> TableRuntimeRenderSnapshot {
        TableRuntimeRenderSnapshot {
            focused_row: self.focused_row.clone(),
            focus_handles: self.focus_handles.clone(),
            focus_proxy: self
                .focused_row
                .as_ref()
                .filter(|identity| !self.focus_handles.contains_key(*identity))
                .map(|_| self.focus_proxy.clone()),
        }
    }

    pub(super) const fn center_virtualizer(&self) -> Option<&VirtualizerResolvedState> {
        self.center_virtualizer.as_ref()
    }

    pub(super) fn set_focused(
        &mut self,
        identity: TableRowIdentity,
        cx: &mut Context<Self>,
    ) -> Option<FocusHandle> {
        let changed = self.focused_row.as_ref() != Some(&identity) || !self.pending_focus_intent;
        self.focused_row = Some(identity.clone());
        self.pending_focus_intent = true;
        if changed {
            cx.notify();
        }
        self.focus_handles.get(&identity).cloned()
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
        identity: TableRowIdentity,
        height: UiPx,
        cx: &mut Context<Self>,
    ) {
        let height = nonnegative_px(height);
        let size_changed = self
            .row_measurements
            .get(&identity)
            .map(|measurement| measurement.size)
            != Some(height);
        self.row_measurements
            .insert(identity, TableRowMeasurementEntry::live(height));
        if size_changed {
            self.invalidate_row_geometry();
            cx.notify();
        }
    }

    pub(super) fn apply_virtualizer_snapshot(
        &mut self,
        snapshot: Option<&TableVirtualizerSnapshot>,
    ) -> bool {
        let changed = apply_virtualizer_snapshot_measurements(
            &mut self.applied_snapshot,
            &mut self.row_measurements,
            self.resolved_model_revision,
            snapshot,
        );
        if changed {
            self.invalidate_row_geometry();
        }
        changed
    }

    pub(super) fn advance_resolved_model_revision(&mut self) {
        advance_resolved_model_revision(
            &mut self.resolved_model_revision,
            &mut self.applied_snapshot,
        );
        self.invalidate_row_geometry();
    }

    pub(super) fn reconcile_row_measurements(&mut self, table: &TableResolvedState) {
        let previous_len = self.row_measurements.len();
        retain_resolved_row_measurements(&mut self.row_measurements, table);
        if self.row_measurements.len() != previous_len {
            self.invalidate_row_geometry();
        }
    }

    pub(super) fn invalidate_row_geometry(&mut self) {
        advance_row_geometry_revision(
            &mut self.row_geometry_revision,
            &mut self.row_geometry_cache,
        );
    }

    pub(super) fn resolve_measured_virtualizer(
        &mut self,
        rows: &[TableResolvedRow],
        fallback_row_height: UiPx,
        overscan: usize,
        scroll_offset: UiPx,
        viewport_extent: UiPx,
    ) -> VirtualizerResolvedState {
        measured_virtualizer_state_cached(
            rows,
            &self.row_measurements,
            fallback_row_height,
            overscan,
            scroll_offset,
            viewport_extent,
            &mut self.row_geometry_cache,
            self.row_geometry_revision.value(),
        )
    }
}

impl TableRowMeasurementLookup for BTreeMap<TableRowIdentity, TableRowMeasurementEntry> {
    fn row_measurement(&self, identity: &TableRowIdentity) -> Option<UiPx> {
        self.get(identity).map(|measurement| measurement.size)
    }
}

impl TableRowMeasurementLookup for TableRuntime {
    fn row_measurement(&self, identity: &TableRowIdentity) -> Option<UiPx> {
        self.row_measurements.row_measurement(identity)
    }
}

fn apply_virtualizer_snapshot_measurements(
    applied: &mut Option<AppliedTableVirtualizerSnapshot>,
    measurements: &mut BTreeMap<TableRowIdentity, TableRowMeasurementEntry>,
    resolved_model_revision: TableRuntimeRevision,
    snapshot: Option<&TableVirtualizerSnapshot>,
) -> bool {
    if applied.as_ref().is_some_and(|applied| {
        applied.resolved_model_revision == resolved_model_revision
            && applied.snapshot.as_ref() == snapshot
    }) {
        return false;
    }

    replace_snapshot_measurements(measurements, snapshot);
    *applied = Some(AppliedTableVirtualizerSnapshot {
        resolved_model_revision,
        snapshot: snapshot.cloned(),
    });
    true
}

fn advance_resolved_model_revision(
    revision: &mut TableRuntimeRevision,
    applied_snapshot: &mut Option<AppliedTableVirtualizerSnapshot>,
) {
    if revision.advance() == TableRuntimeRevisionAdvance::Wrapped {
        *applied_snapshot = None;
    }
}

fn advance_row_geometry_revision(
    revision: &mut TableRuntimeRevision,
    geometry_cache: &mut VirtualizerGeometryCache,
) {
    if revision.advance() == TableRuntimeRevisionAdvance::Wrapped {
        *geometry_cache = VirtualizerGeometryCache::default();
    }
}

fn replace_snapshot_measurements(
    measurements: &mut BTreeMap<TableRowIdentity, TableRowMeasurementEntry>,
    snapshot: Option<&TableVirtualizerSnapshot>,
) {
    measurements.retain(|_, measurement| measurement.is_live());
    let Some(snapshot) = snapshot else {
        return;
    };

    for snapshot_measurement in snapshot.measurements() {
        let preserve_live = measurements
            .get(snapshot_measurement.identity())
            .is_some_and(|measurement| measurement.is_live());
        if preserve_live {
            continue;
        }

        if valid_snapshot_size(snapshot_measurement.size()) {
            measurements.insert(
                snapshot_measurement.identity().clone(),
                TableRowMeasurementEntry::snapshot(snapshot_measurement.size()),
            );
        } else {
            measurements.remove(snapshot_measurement.identity());
        }
    }
}

fn retain_resolved_row_measurements(
    measurements: &mut BTreeMap<TableRowIdentity, TableRowMeasurementEntry>,
    table: &TableResolvedState,
) {
    measurements.retain(|identity, _| resolved_row_identity_exists(table, identity));
}

fn resolved_row_identity_exists(table: &TableResolvedState, identity: &TableRowIdentity) -> bool {
    table.core_model().row(identity).is_some() || table.final_model().row(identity).is_some()
}

fn reconcile_focused_row(
    focused_row: Option<&TableRowIdentity>,
    final_model: &TableRowModel,
) -> Option<TableRowIdentity> {
    if let Some(focused_row) = focused_row
        && final_model.row_index(focused_row).is_some()
    {
        return Some(focused_row.clone());
    }

    final_model.rows().first().map(|row| row.identity().clone())
}

#[cfg(test)]
mod tests {
    use super::*;
    use open_gpui_ui_core::{
        TableColumn, TableFilter, TablePagination, TableRow, TableState, ui_px,
    };

    fn snapshot_measurement(size: UiPx) -> TableRowMeasurementEntry {
        TableRowMeasurementEntry::snapshot(size)
    }

    fn live_measurement(size: UiPx) -> TableRowMeasurementEntry {
        TableRowMeasurementEntry::live(size)
    }

    #[test]
    fn stale_focus_does_not_guess_a_replacement_by_business_id() {
        let stale = TableRowIdentity::source("duplicate");
        let first = TableRowIdentity::source("first");
        let resolved = TableState::new([
            TableRow::new("first"),
            TableRow::new("duplicate").with_instance_id("replacement"),
        ])
        .resolve();

        assert_eq!(
            reconcile_focused_row(Some(&stale), resolved.final_model()),
            Some(first.clone())
        );
        assert_eq!(
            reconcile_focused_row(Some(&first), resolved.final_model()),
            Some(first)
        );
        assert_eq!(
            reconcile_focused_row(
                Some(&stale),
                TableState::new(std::iter::empty::<TableRow>())
                    .resolve()
                    .final_model(),
            ),
            None
        );
    }

    #[test]
    fn row_measurements_survive_filter_pagination_and_reorder_by_exact_identity() {
        let rows = || {
            [
                TableRow::new("duplicate")
                    .with_instance_id("first")
                    .with_cell("name", "First"),
                TableRow::new("duplicate")
                    .with_instance_id("second")
                    .with_cell("name", "Second"),
            ]
        };
        let initial_state =
            TableState::new(rows()).with_columns([TableColumn::new("name", "Name")]);
        let stale_occurrence_state =
            TableState::new([TableRow::new("occurrence"), TableRow::new("occurrence")]);
        let stale_occurrence = TableRowIdentity::Source(
            stale_occurrence_state
                .source_row_identity_at("occurrence", 1)
                .expect("second occurrence should resolve"),
        );
        let mut measurements = BTreeMap::from([
            (
                TableRowIdentity::source_instance("duplicate", "first"),
                snapshot_measurement(ui_px(18.0)),
            ),
            (
                TableRowIdentity::source_instance("duplicate", "second"),
                live_measurement(ui_px(42.0)),
            ),
            (stale_occurrence, snapshot_measurement(ui_px(64.0))),
        ]);

        let filtered = initial_state
            .clone()
            .with_filters([TableFilter::contains("name", "Second")])
            .resolve();
        retain_resolved_row_measurements(&mut measurements, &filtered);
        assert_eq!(measurements.len(), 2);

        let paginated = initial_state
            .clone()
            .with_pagination(TablePagination::new(0, 1))
            .resolve();
        retain_resolved_row_measurements(&mut measurements, &paginated);
        assert_eq!(measurements.len(), 2);

        let [first, second] = rows();
        let reordered = initial_state.with_rows([second, first]).resolve();
        retain_resolved_row_measurements(&mut measurements, &reordered);

        assert_eq!(measurements.len(), 2);
        assert_eq!(
            measurements.get(&TableRowIdentity::source_instance("duplicate", "first")),
            Some(&snapshot_measurement(ui_px(18.0)))
        );
        assert_eq!(
            measurements.get(&TableRowIdentity::source_instance("duplicate", "second")),
            Some(&live_measurement(ui_px(42.0)))
        );
    }

    #[test]
    fn replacing_snapshot_seeds_preserves_live_row_measurements() {
        let first = TableRowIdentity::source_instance("duplicate", "first");
        let second = TableRowIdentity::source_instance("duplicate", "second");
        let mut measurements = BTreeMap::from([
            (first.clone(), snapshot_measurement(ui_px(12.0))),
            (second.clone(), live_measurement(ui_px(42.0))),
        ]);
        let replacement_snapshot = TableVirtualizerSnapshot::new([
            super::super::TableVirtualizerSnapshotItem::new(first.clone(), ui_px(18.0)),
            super::super::TableVirtualizerSnapshotItem::new(second.clone(), ui_px(24.0)),
        ]);

        replace_snapshot_measurements(&mut measurements, Some(&replacement_snapshot));

        assert_eq!(
            measurements,
            BTreeMap::from([
                (first, snapshot_measurement(ui_px(18.0))),
                (second, live_measurement(ui_px(42.0))),
            ])
        );
    }

    #[test]
    fn grouped_snapshot_reseeds_when_synthetic_row_leaves_and_returns() {
        let base_state = TableState::new([
            TableRow::new("row-a").with_cell("team", "ops"),
            TableRow::new("row-b").with_cell("team", "design"),
        ])
        .with_columns([TableColumn::new("team", "Team")]);
        let rows_identity = base_state.cache_key().rows_identity();
        let grouped_state = base_state.clone().with_grouping(["team"]);
        assert_eq!(grouped_state.cache_key().rows_identity(), rows_identity);
        let grouped = grouped_state.resolve();
        let group_identity = grouped
            .final_model()
            .rows()
            .iter()
            .find(|row| row.is_group())
            .expect("grouped model should expose a synthetic row")
            .identity()
            .clone();
        let live_identity = TableRowIdentity::source("row-a");
        let snapshot = TableVirtualizerSnapshot::new([
            super::super::TableVirtualizerSnapshotItem::new(group_identity.clone(), ui_px(48.0)),
            super::super::TableVirtualizerSnapshotItem::new(live_identity.clone(), ui_px(24.0)),
        ]);
        let mut applied = None;
        let mut measurements = BTreeMap::new();
        let mut model_revision = TableRuntimeRevision::default();

        advance_resolved_model_revision(&mut model_revision, &mut applied);
        assert!(apply_virtualizer_snapshot_measurements(
            &mut applied,
            &mut measurements,
            model_revision,
            Some(&snapshot),
        ));
        measurements.insert(live_identity.clone(), live_measurement(ui_px(72.0)));
        retain_resolved_row_measurements(&mut measurements, &grouped);
        assert_eq!(
            measurements.get(&group_identity),
            Some(&snapshot_measurement(ui_px(48.0)))
        );

        assert_eq!(base_state.cache_key().rows_identity(), rows_identity);
        advance_resolved_model_revision(&mut model_revision, &mut applied);
        let ungrouped = base_state.clone().resolve();
        assert_eq!(ungrouped.core_model().rows().len(), 2);
        retain_resolved_row_measurements(&mut measurements, &ungrouped);
        assert_eq!(measurements.get(&group_identity), None);
        let _ = apply_virtualizer_snapshot_measurements(
            &mut applied,
            &mut measurements,
            model_revision,
            Some(&snapshot),
        );
        retain_resolved_row_measurements(&mut measurements, &ungrouped);
        assert_eq!(measurements.get(&group_identity), None);

        let returned_state = base_state.with_grouping(["team"]);
        assert_eq!(returned_state.cache_key().rows_identity(), rows_identity);
        advance_resolved_model_revision(&mut model_revision, &mut applied);
        let returned = returned_state.resolve();
        retain_resolved_row_measurements(&mut measurements, &returned);
        let _ = apply_virtualizer_snapshot_measurements(
            &mut applied,
            &mut measurements,
            model_revision,
            Some(&snapshot),
        );
        retain_resolved_row_measurements(&mut measurements, &returned);
        assert_eq!(
            measurements.get(&group_identity),
            Some(&snapshot_measurement(ui_px(48.0)))
        );
        assert_eq!(
            measurements.get(&live_identity),
            Some(&live_measurement(ui_px(72.0))),
            "snapshot replay must preserve live measurement precedence"
        );
        assert!(
            !apply_virtualizer_snapshot_measurements(
                &mut applied,
                &mut measurements,
                model_revision,
                Some(&snapshot),
            ),
            "an unchanged resolved model and snapshot should be idempotent"
        );
    }

    #[test]
    fn removing_snapshot_authority_drops_snapshot_only_measurements_once() {
        let snapshot_only = TableRowIdentity::source("snapshot-only");
        let live_override = TableRowIdentity::source("live-override");
        let snapshot = TableVirtualizerSnapshot::new([
            super::super::TableVirtualizerSnapshotItem::new(snapshot_only.clone(), ui_px(18.0)),
            super::super::TableVirtualizerSnapshotItem::new(live_override.clone(), ui_px(24.0)),
        ]);
        let mut applied = None;
        let mut measurements = BTreeMap::new();
        let revision = TableRuntimeRevision::default();

        assert!(apply_virtualizer_snapshot_measurements(
            &mut applied,
            &mut measurements,
            revision,
            Some(&snapshot),
        ));
        measurements.insert(live_override.clone(), live_measurement(ui_px(42.0)));

        assert!(apply_virtualizer_snapshot_measurements(
            &mut applied,
            &mut measurements,
            revision,
            None,
        ));
        assert_eq!(measurements.get(&snapshot_only), None);
        assert_eq!(
            measurements.get(&live_override),
            Some(&live_measurement(ui_px(42.0)))
        );

        assert!(
            !apply_virtualizer_snapshot_measurements(
                &mut applied,
                &mut measurements,
                revision,
                None,
            ),
            "repeating absent snapshot authority should be idempotent"
        );
        assert_eq!(
            measurements,
            BTreeMap::from([(live_override, live_measurement(ui_px(42.0)))])
        );
    }

    #[test]
    fn resolved_model_revision_wrap_invalidates_applied_snapshot_authority() {
        let snapshot = TableVirtualizerSnapshot::new([]);
        let mut revision = TableRuntimeRevision(u64::MAX);
        let mut applied = Some(AppliedTableVirtualizerSnapshot {
            resolved_model_revision: TableRuntimeRevision::default(),
            snapshot: Some(snapshot),
        });

        advance_resolved_model_revision(&mut revision, &mut applied);

        assert_eq!(revision, TableRuntimeRevision::default());
        assert_eq!(applied, None);
    }
}
