use std::collections::BTreeMap;

use open_gpui_ui_core::virtualizer::VirtualizerGeometryCache;
use open_gpui_ui_core::{
    TableResolvedRow, TableRowIdentity, UiPx, VirtualizerResolvedState, VirtualizerState,
};

use super::identity::table_row_virtualizer_key_from_key;
use super::{TableRowRenderPlan, nonnegative_px};

/// One typed table-row measurement retained for virtualizer restoration.
#[derive(Debug, Clone, PartialEq)]
pub struct TableVirtualizerSnapshotItem {
    identity: TableRowIdentity,
    size: UiPx,
}

impl TableVirtualizerSnapshotItem {
    /// Creates a retained measurement for one authoritative row identity.
    /// Negative and non-finite sizes normalize to zero for diagnostics. Non-positive sizes do not
    /// become restoration authority, so virtual geometry can fall back and mount the row.
    ///
    /// ```compile_fail
    /// use open_gpui_ui_components::TableVirtualizerSnapshotItem;
    /// use open_gpui_ui_core::ui_px;
    ///
    /// let _ = TableVirtualizerSnapshotItem::new("row-a", ui_px(24.0));
    /// ```
    pub fn new(identity: TableRowIdentity, size: UiPx) -> Self {
        Self {
            identity,
            size: normalize_snapshot_size(size),
        }
    }

    /// Returns the authoritative row identity for this measurement.
    pub const fn identity(&self) -> &TableRowIdentity {
        &self.identity
    }

    /// Returns the retained row size.
    pub const fn size(&self) -> UiPx {
        self.size
    }
}

/// Typed virtualizer restoration data for a table row model.
#[derive(Debug, Clone, PartialEq)]
pub struct TableVirtualizerSnapshot {
    measurements: Vec<TableVirtualizerSnapshotItem>,
}

impl TableVirtualizerSnapshot {
    /// Creates table restoration data from typed row measurements.
    pub fn new(measurements: impl IntoIterator<Item = TableVirtualizerSnapshotItem>) -> Self {
        Self {
            measurements: measurements.into_iter().collect(),
        }
    }

    /// Returns the captured typed row measurements.
    pub fn measurements(&self) -> &[TableVirtualizerSnapshotItem] {
        &self.measurements
    }

    pub(super) fn effective_measurement_map(&self) -> BTreeMap<TableRowIdentity, UiPx> {
        let mut measurements = BTreeMap::new();
        for measurement in &self.measurements {
            if valid_snapshot_size(measurement.size) {
                measurements.insert(measurement.identity.clone(), measurement.size);
            } else {
                measurements.remove(&measurement.identity);
            }
        }
        measurements
    }
}

fn normalize_snapshot_size(size: UiPx) -> UiPx {
    if size.as_f32().is_finite() {
        nonnegative_px(size)
    } else {
        UiPx::ZERO
    }
}

pub(super) fn valid_snapshot_size(size: UiPx) -> bool {
    size.as_f32().is_finite() && size.as_f32() > 0.0
}

pub(super) trait TableRowMeasurementLookup {
    fn row_measurement(&self, identity: &TableRowIdentity) -> Option<UiPx>;
}

impl TableRowMeasurementLookup for BTreeMap<TableRowIdentity, UiPx> {
    fn row_measurement(&self, identity: &TableRowIdentity) -> Option<UiPx> {
        self.get(identity).copied()
    }
}

pub(super) fn measured_virtualizer_state(
    rows: &[TableResolvedRow],
    row_measurements: &impl TableRowMeasurementLookup,
    fallback_row_height: UiPx,
    overscan: usize,
    scroll_offset: UiPx,
    viewport_extent: UiPx,
) -> VirtualizerResolvedState {
    let state = VirtualizerState::new(rows.len(), fallback_row_height)
        .with_viewport_extent(viewport_extent)
        .with_overscan(overscan)
        .with_scroll_offset(scroll_offset);

    state.resolve_known_size_window_by(
        |index| {
            let row = &rows[index];
            row_measurements
                .row_measurement(row.identity())
                .unwrap_or(fallback_row_height)
        },
        |index| table_row_virtualizer_key_from_key(rows[index].identity_key()),
    )
}

pub(super) fn measured_virtualizer_state_cached(
    rows: &[TableResolvedRow],
    row_measurements: &impl TableRowMeasurementLookup,
    fallback_row_height: UiPx,
    overscan: usize,
    scroll_offset: UiPx,
    viewport_extent: UiPx,
    geometry_cache: &mut VirtualizerGeometryCache,
    geometry_revision: u64,
) -> VirtualizerResolvedState {
    let state = VirtualizerState::new(rows.len(), fallback_row_height)
        .with_viewport_extent(viewport_extent)
        .with_overscan(overscan)
        .with_scroll_offset(scroll_offset);

    state.resolve_known_size_window_by_cached(
        geometry_cache,
        geometry_revision,
        |index| {
            let row = &rows[index];
            row_measurements
                .row_measurement(row.identity())
                .unwrap_or(fallback_row_height)
        },
        |index| table_row_virtualizer_key_from_key(rows[index].identity_key()),
    )
}

pub(super) fn table_rows_virtual_size(rows: &[TableRowRenderPlan]) -> UiPx {
    rows.iter()
        .fold(UiPx::ZERO, |total, row| total + row.virtual_size())
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;

    use open_gpui_ui_core::{TableRow, TableState, ui_px};

    use super::*;

    struct CountingMeasurements {
        sizes: BTreeMap<TableRowIdentity, UiPx>,
        calls: Cell<usize>,
    }

    impl TableRowMeasurementLookup for CountingMeasurements {
        fn row_measurement(&self, identity: &TableRowIdentity) -> Option<UiPx> {
            self.calls.set(self.calls.get() + 1);
            self.sizes.get(identity).copied()
        }
    }

    #[test]
    fn table_adapter_reuses_geometry_until_measurement_revision_changes() {
        let table =
            TableState::new((0..8).map(|index| TableRow::new(format!("row-{index}")))).resolve();
        let mut measurements = CountingMeasurements {
            sizes: BTreeMap::new(),
            calls: Cell::new(0),
        };
        let mut cache = VirtualizerGeometryCache::default();

        let first = measured_virtualizer_state_cached(
            table.center_rows(),
            &measurements,
            ui_px(20.0),
            2,
            UiPx::ZERO,
            ui_px(40.0),
            &mut cache,
            1,
        );
        assert_eq!(measurements.calls.get(), table.center_rows().len());

        measurements.calls.set(0);
        let scrolled = measured_virtualizer_state_cached(
            table.center_rows(),
            &measurements,
            ui_px(20.0),
            2,
            ui_px(80.0),
            ui_px(40.0),
            &mut cache,
            1,
        );
        assert_eq!(measurements.calls.get(), 0);
        assert_ne!(first.visible_range(), scrolled.visible_range());

        measurements
            .sizes
            .insert(table.center_rows()[0].identity().clone(), ui_px(60.0));
        measurements.calls.set(0);
        let invalidated = measured_virtualizer_state_cached(
            table.center_rows(),
            &measurements,
            ui_px(20.0),
            2,
            UiPx::ZERO,
            ui_px(40.0),
            &mut cache,
            2,
        );
        assert_eq!(measurements.calls.get(), table.center_rows().len());
        assert_eq!(invalidated.item_geometry(0).unwrap().size(), ui_px(60.0));
    }

    #[test]
    fn table_virtualizer_snapshot_measurements_are_nonnegative() {
        let measurement =
            TableVirtualizerSnapshotItem::new(TableRowIdentity::source("row"), ui_px(-12.0));

        assert_eq!(measurement.size(), UiPx::ZERO);
    }

    #[test]
    fn table_virtualizer_snapshot_measurements_normalize_non_finite_sizes() {
        for size in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            let measurement =
                TableVirtualizerSnapshotItem::new(TableRowIdentity::source("row"), ui_px(size));

            assert_eq!(
                measurement.size(),
                UiPx::ZERO,
                "non-finite snapshot size {size:?} should normalize to zero"
            );
        }
    }

    #[test]
    fn non_finite_snapshot_measurements_cannot_poison_virtual_geometry() {
        let table =
            TableState::new([TableRow::new("invalid"), TableRow::new("fallback")]).resolve();
        let invalid_identity = TableRowIdentity::source("invalid");

        for size in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            let measurements = TableVirtualizerSnapshot::new([TableVirtualizerSnapshotItem::new(
                invalid_identity.clone(),
                ui_px(size),
            )])
            .effective_measurement_map();
            let virtualizer = measured_virtualizer_state(
                table.center_rows(),
                &measurements,
                ui_px(24.0),
                0,
                UiPx::ZERO,
                ui_px(48.0),
            );

            assert_eq!(virtualizer.total_size(), ui_px(48.0));
            for index in 0..2 {
                let geometry = virtualizer
                    .item_geometry(index)
                    .expect("resolved table rows should retain virtual geometry");
                assert!(geometry.start().as_f32().is_finite());
                assert!(geometry.size().as_f32().is_finite());
                assert_eq!(geometry.size(), ui_px(24.0));
            }
        }
    }

    #[test]
    fn invalid_single_row_snapshot_measurement_falls_back_and_mounts() {
        let table = TableState::new([TableRow::new("invalid")]).resolve();
        let identity = TableRowIdentity::source("invalid");

        for size in [0.0, -12.0, f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            let snapshot = TableVirtualizerSnapshot::new([TableVirtualizerSnapshotItem::new(
                identity.clone(),
                ui_px(size),
            )]);
            assert_eq!(
                snapshot.measurements()[0].size(),
                UiPx::ZERO,
                "the public snapshot should retain normalized zero for diagnostics"
            );

            let measurements = snapshot.effective_measurement_map();
            let virtualizer = measured_virtualizer_state(
                table.center_rows(),
                &measurements,
                ui_px(24.0),
                0,
                UiPx::ZERO,
                ui_px(24.0),
            );

            assert_eq!(virtualizer.total_size(), ui_px(24.0));
            assert_eq!(virtualizer.items().len(), 1);
            assert_eq!(virtualizer.measurements()[0].size(), ui_px(24.0));
            assert!(!virtualizer.measurements()[0].measured());
        }
    }
}
