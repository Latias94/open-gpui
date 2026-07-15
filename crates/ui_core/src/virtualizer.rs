//! Renderer-neutral one-dimensional virtualizer contracts for Open GPUI components.

use crate::geometry::{UiPx, ui_px};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::ops::Range;
use std::sync::Arc;

/// Convenient imports for renderer-neutral virtualizer work.
pub mod prelude {
    pub use super::{
        VirtualizerItemGeometry, VirtualizerItemKey, VirtualizerItemMeasurement, VirtualizerRange,
        VirtualizerResolvedState, VirtualizerSnapshot, VirtualizerSnapshotItem, VirtualizerState,
    };
}

/// Stable renderer-neutral identity for a virtualized item.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct VirtualizerItemKey(String);

impl VirtualizerItemKey {
    /// Creates an item key from a stable string.
    pub fn new(key: impl Into<String>) -> Self {
        Self(key.into())
    }

    /// Returns the stable string value.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<&str> for VirtualizerItemKey {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

impl From<String> for VirtualizerItemKey {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

/// Resolved virtual range using Rust's exclusive `end` convention.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VirtualizerRange {
    start: usize,
    end: usize,
}

impl VirtualizerRange {
    /// Creates a range, clamping `end` so it is not before `start`.
    pub const fn new(start: usize, end: usize) -> Self {
        let end = if end < start { start } else { end };
        Self { start, end }
    }

    /// Returns an empty range at index zero.
    pub const fn empty() -> Self {
        Self { start: 0, end: 0 }
    }

    /// Returns the first index in the range.
    pub const fn start(&self) -> usize {
        self.start
    }

    /// Returns the exclusive end index.
    pub const fn end(&self) -> usize {
        self.end
    }

    /// Returns whether the range has no items.
    pub const fn is_empty(&self) -> bool {
        self.start == self.end
    }

    /// Returns the number of items in the range.
    pub const fn len(&self) -> usize {
        self.end - self.start
    }

    /// Returns the standard Rust range.
    pub const fn as_range(&self) -> Range<usize> {
        self.start..self.end
    }
}

/// One virtualized item measurement.
#[derive(Debug, Clone, PartialEq)]
pub struct VirtualizerItemMeasurement {
    index: usize,
    key: VirtualizerItemKey,
    start: UiPx,
    size: UiPx,
    end: UiPx,
    measured: bool,
}

impl VirtualizerItemMeasurement {
    /// Creates an item measurement.
    pub const fn new(
        index: usize,
        key: VirtualizerItemKey,
        start: UiPx,
        size: UiPx,
        measured: bool,
    ) -> Self {
        Self {
            index,
            key,
            start,
            size,
            end: UiPx::new(start.as_f32() + size.as_f32()),
            measured,
        }
    }

    /// Returns the item index.
    pub const fn index(&self) -> usize {
        self.index
    }

    /// Returns the stable item key.
    pub const fn key(&self) -> &VirtualizerItemKey {
        &self.key
    }

    /// Returns the item start offset.
    pub const fn start(&self) -> UiPx {
        self.start
    }

    /// Returns the item size.
    pub const fn size(&self) -> UiPx {
        self.size
    }

    /// Returns the item end offset.
    pub const fn end(&self) -> UiPx {
        self.end
    }

    /// Returns whether this size came from the measurement cache.
    pub const fn measured(&self) -> bool {
        self.measured
    }
}

/// Renderer-neutral geometry for one virtualized item.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VirtualizerItemGeometry {
    start: UiPx,
    size: UiPx,
    end: UiPx,
}

impl VirtualizerItemGeometry {
    /// Creates item geometry from a start offset and size.
    pub const fn new(start: UiPx, size: UiPx) -> Self {
        Self {
            start,
            size,
            end: UiPx::new(start.as_f32() + size.as_f32()),
        }
    }

    /// Returns the item start offset.
    pub const fn start(self) -> UiPx {
        self.start
    }

    /// Returns the item size.
    pub const fn size(self) -> UiPx {
        self.size
    }

    /// Returns the item end offset.
    pub const fn end(self) -> UiPx {
        self.end
    }
}

#[derive(Debug, Clone, PartialEq)]
enum VirtualizerGeometryIndex {
    Fixed {
        count: usize,
        size: UiPx,
        gap: UiPx,
        scroll_margin: UiPx,
    },
    Variable {
        ends: Arc<[UiPx]>,
        gap: UiPx,
        scroll_margin: UiPx,
    },
}

impl VirtualizerGeometryIndex {
    fn fixed(count: usize, size: UiPx, gap: UiPx, scroll_margin: UiPx) -> Self {
        Self::Fixed {
            count,
            size: nonnegative_px(size),
            gap: nonnegative_px(gap),
            scroll_margin: nonnegative_px(scroll_margin),
        }
    }

    fn from_ends(ends: Vec<UiPx>, gap: UiPx, scroll_margin: UiPx) -> Self {
        let gap = nonnegative_px(gap);
        let scroll_margin = nonnegative_px(scroll_margin);
        let first_size = ends
            .first()
            .map(|end| nonnegative_px(*end - scroll_margin))
            .unwrap_or(UiPx::ZERO);
        let uniform = ends.iter().enumerate().all(|(index, end)| {
            let start = if index == 0 {
                scroll_margin
            } else {
                ends[index - 1] + gap
            };
            nonnegative_px(*end - start) == first_size
        });

        if uniform {
            Self::fixed(ends.len(), first_size, gap, scroll_margin)
        } else {
            Self::Variable {
                ends: ends.into(),
                gap,
                scroll_margin,
            }
        }
    }

    fn from_measurements(
        measurements: &[VirtualizerItemMeasurement],
        gap: UiPx,
        scroll_margin: UiPx,
    ) -> Self {
        Self::from_ends(
            measurements.iter().map(|item| item.end()).collect(),
            gap,
            scroll_margin,
        )
    }

    fn item(&self, index: usize) -> Option<VirtualizerItemGeometry> {
        match self {
            Self::Fixed {
                count,
                size,
                gap,
                scroll_margin,
            } => {
                if index >= *count {
                    return None;
                }
                let start = *scroll_margin + (*size + *gap) * index as f32;
                Some(VirtualizerItemGeometry::new(start, *size))
            }
            Self::Variable {
                ends,
                gap,
                scroll_margin,
            } => {
                let end = *ends.get(index)?;
                let start = if index == 0 {
                    *scroll_margin
                } else {
                    ends[index - 1] + *gap
                };
                Some(VirtualizerItemGeometry::new(
                    start,
                    nonnegative_px(end - start),
                ))
            }
        }
    }

    fn total_size(&self) -> UiPx {
        match self {
            Self::Fixed {
                count,
                size,
                gap,
                scroll_margin,
            } => fixed_total_size(*count, *size, *gap, *scroll_margin),
            Self::Variable {
                ends,
                scroll_margin,
                ..
            } => ends
                .last()
                .map(|end| *end + *scroll_margin)
                .unwrap_or(UiPx::ZERO),
        }
    }

    fn visible_range(&self, scroll_offset: UiPx, viewport_extent: UiPx) -> VirtualizerRange {
        if viewport_extent.as_f32() <= 0.0 {
            return VirtualizerRange::empty();
        }

        match self {
            Self::Fixed {
                count,
                size,
                gap,
                scroll_margin,
            } => fixed_visible_range(
                *count,
                scroll_offset,
                viewport_extent,
                *size,
                *gap,
                *scroll_margin,
            ),
            Self::Variable {
                ends,
                gap,
                scroll_margin,
            } => {
                let viewport_start = scroll_offset.as_f32();
                let viewport_end = viewport_start + viewport_extent.as_f32();
                let start = ends.partition_point(|end| end.as_f32() <= viewport_start);
                if start == ends.len() {
                    return VirtualizerRange::empty();
                }

                let end = if scroll_margin.as_f32() < viewport_end {
                    1 + ends[..ends.len().saturating_sub(1)]
                        .partition_point(|end| (*end + *gap).as_f32() < viewport_end)
                } else {
                    0
                };
                VirtualizerRange::new(start, end.max(start))
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum VirtualizerGeometryKind {
    KnownSize,
    Measured,
}

#[derive(Clone)]
struct VirtualizerGeometryCacheKey {
    kind: VirtualizerGeometryKind,
    revision: u64,
    count: usize,
    estimated_size: UiPx,
    gap: UiPx,
    scroll_margin: UiPx,
    measurement_authority: Option<Arc<[VirtualizerSnapshotItem]>>,
}

impl VirtualizerGeometryCacheKey {
    fn matches(&self, other: &Self) -> bool {
        self.matches_geometry_inputs(other)
            && optional_measurement_authorities_match(
                self.measurement_authority.as_ref(),
                other.measurement_authority.as_ref(),
            )
    }

    fn matches_geometry_inputs(&self, other: &Self) -> bool {
        self.kind == other.kind
            && self.revision == other.revision
            && self.count == other.count
            && self.estimated_size == other.estimated_size
            && self.gap == other.gap
            && self.scroll_margin == other.scroll_margin
    }
}

#[derive(Clone)]
struct VirtualizerMeasuredItemMetadata {
    key: VirtualizerItemKey,
    measured: bool,
}

#[derive(Clone)]
struct VirtualizerMeasuredGeometry {
    geometry: VirtualizerGeometryIndex,
    item_metadata: Arc<[VirtualizerMeasuredItemMetadata]>,
    snapshot_measurements: Arc<[VirtualizerSnapshotItem]>,
}

#[derive(Clone)]
enum VirtualizerGeometryCacheValue {
    KnownSize(VirtualizerGeometryIndex),
    Measured(VirtualizerMeasuredGeometry),
}

#[derive(Clone)]
struct VirtualizerGeometryCacheEntry {
    key: VirtualizerGeometryCacheKey,
    value: VirtualizerGeometryCacheValue,
}

/// Caller-owned dense geometry retained across virtualizer resolutions.
///
/// Cached resolvers reuse geometry only when the caller supplies the same revision and all
/// geometry-affecting state remains unchanged. Callers must advance the revision whenever their
/// size callback authority changes; viewport, scroll offset, and overscan changes do not require
/// invalidation.
#[derive(Clone, Default)]
pub struct VirtualizerGeometryCache {
    entry: Option<VirtualizerGeometryCacheEntry>,
}

impl VirtualizerGeometryCache {
    /// Drops retained geometry and measurement metadata.
    pub fn clear(&mut self) {
        self.entry = None;
    }

    fn resolve_known_size(
        &mut self,
        key: VirtualizerGeometryCacheKey,
        build: impl FnOnce() -> VirtualizerGeometryIndex,
    ) -> VirtualizerGeometryIndex {
        if let Some(entry) = &self.entry
            && entry.key.matches(&key)
            && let VirtualizerGeometryCacheValue::KnownSize(geometry) = &entry.value
        {
            return geometry.clone();
        }

        let geometry = build();
        self.entry = Some(VirtualizerGeometryCacheEntry {
            key,
            value: VirtualizerGeometryCacheValue::KnownSize(geometry.clone()),
        });
        geometry
    }

    fn resolve_measured(
        &mut self,
        key: VirtualizerGeometryCacheKey,
        build: impl FnOnce() -> VirtualizerMeasuredGeometry,
    ) -> VirtualizerMeasuredGeometry {
        if let Some(entry) = &self.entry
            && entry.key.matches_geometry_inputs(&key)
            && let VirtualizerGeometryCacheValue::Measured(measured) = &entry.value
            && (entry.key.matches(&key)
                || optional_authority_matches_measurements(
                    key.measurement_authority.as_ref(),
                    &measured.snapshot_measurements,
                ))
        {
            return measured.clone();
        }

        let measured = build();
        self.entry = Some(VirtualizerGeometryCacheEntry {
            key,
            value: VirtualizerGeometryCacheValue::Measured(measured.clone()),
        });
        measured
    }
}

impl fmt::Debug for VirtualizerGeometryCache {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let kind = self.entry.as_ref().map(|entry| match &entry.value {
            VirtualizerGeometryCacheValue::KnownSize(_) => VirtualizerGeometryKind::KnownSize,
            VirtualizerGeometryCacheValue::Measured(_) => VirtualizerGeometryKind::Measured,
        });
        formatter
            .debug_struct("VirtualizerGeometryCache")
            .field("populated", &self.entry.is_some())
            .field("kind", &kind)
            .finish()
    }
}

/// Serializable measured item entry for virtualizer restore.
#[derive(Debug, Clone, PartialEq)]
pub struct VirtualizerSnapshotItem {
    key: VirtualizerItemKey,
    size: UiPx,
}

impl VirtualizerSnapshotItem {
    /// Creates a snapshot entry.
    pub const fn new(key: VirtualizerItemKey, size: UiPx) -> Self {
        Self { key, size }
    }

    /// Returns the stable item key.
    pub const fn key(&self) -> &VirtualizerItemKey {
        &self.key
    }

    /// Returns the measured item size.
    pub const fn size(&self) -> UiPx {
        self.size
    }
}

/// Snapshot data that can seed a future virtualizer state.
#[derive(Debug, Clone, PartialEq)]
pub struct VirtualizerSnapshot {
    scroll_offset: UiPx,
    measurements: Arc<[VirtualizerSnapshotItem]>,
}

impl VirtualizerSnapshot {
    /// Creates snapshot data from offset and measured items.
    pub fn new(
        scroll_offset: UiPx,
        measurements: impl IntoIterator<Item = VirtualizerSnapshotItem>,
    ) -> Self {
        Self {
            scroll_offset: nonnegative_px(scroll_offset),
            measurements: measurements.into_iter().collect(),
        }
    }

    /// Returns the captured scroll offset.
    pub const fn scroll_offset(&self) -> UiPx {
        self.scroll_offset
    }

    /// Returns captured measured items.
    pub fn measurements(&self) -> &[VirtualizerSnapshotItem] {
        &self.measurements
    }

    /// Returns whether two snapshots share the same retained measurement storage.
    ///
    /// Scroll offsets do not participate. Empty measurement authorities are equivalent even when
    /// their empty allocations differ.
    pub fn shares_measurement_authority_with(&self, other: &Self) -> bool {
        shared_measurements_match(&self.measurements, &other.measurements)
    }

    fn from_shared_measurements(
        scroll_offset: UiPx,
        measurements: Arc<[VirtualizerSnapshotItem]>,
    ) -> Self {
        Self {
            scroll_offset: nonnegative_px(scroll_offset),
            measurements,
        }
    }
}

fn shared_measurements_match(
    left: &Arc<[VirtualizerSnapshotItem]>,
    right: &Arc<[VirtualizerSnapshotItem]>,
) -> bool {
    (left.is_empty() && right.is_empty()) || Arc::ptr_eq(left, right)
}

fn optional_measurement_authorities_match(
    left: Option<&Arc<[VirtualizerSnapshotItem]>>,
    right: Option<&Arc<[VirtualizerSnapshotItem]>>,
) -> bool {
    match (left, right) {
        (None, None) => true,
        (Some(left), Some(right)) => shared_measurements_match(left, right),
        (None, Some(measurements)) | (Some(measurements), None) => measurements.is_empty(),
    }
}

fn optional_authority_matches_measurements(
    authority: Option<&Arc<[VirtualizerSnapshotItem]>>,
    measurements: &Arc<[VirtualizerSnapshotItem]>,
) -> bool {
    match authority {
        Some(authority) => shared_measurements_match(authority, measurements),
        None => measurements.is_empty(),
    }
}

/// Renderer-neutral virtualizer input state.
#[derive(Debug, Clone, PartialEq)]
pub struct VirtualizerState {
    count: usize,
    viewport_extent: UiPx,
    scroll_offset: UiPx,
    estimated_size: UiPx,
    overscan: usize,
    gap: UiPx,
    scroll_margin: UiPx,
    item_keys: Vec<VirtualizerItemKey>,
    measurements_by_key: BTreeMap<VirtualizerItemKey, UiPx>,
    measurement_authority: Option<Arc<[VirtualizerSnapshotItem]>>,
}

impl VirtualizerState {
    /// Creates virtualizer state from an item count and estimated item size.
    pub fn new(count: usize, estimated_size: UiPx) -> Self {
        Self {
            count,
            viewport_extent: UiPx::ZERO,
            scroll_offset: UiPx::ZERO,
            estimated_size: nonnegative_px(estimated_size),
            overscan: 0,
            gap: UiPx::ZERO,
            scroll_margin: UiPx::ZERO,
            item_keys: Vec::new(),
            measurements_by_key: BTreeMap::new(),
            measurement_authority: None,
        }
    }

    /// Applies the viewport extent on the virtualized axis.
    pub const fn with_viewport_extent(mut self, viewport_extent: UiPx) -> Self {
        self.viewport_extent = viewport_extent;
        self
    }

    /// Applies the current scroll offset.
    pub const fn with_scroll_offset(mut self, scroll_offset: UiPx) -> Self {
        self.scroll_offset = scroll_offset;
        self
    }

    /// Applies the total overscan item budget outside the visible range.
    pub const fn with_overscan(mut self, overscan: usize) -> Self {
        self.overscan = overscan;
        self
    }

    /// Applies the gap between adjacent items.
    pub const fn with_gap(mut self, gap: UiPx) -> Self {
        self.gap = gap;
        self
    }

    /// Applies the scroll margin before the first and after the last item.
    pub const fn with_scroll_margin(mut self, scroll_margin: UiPx) -> Self {
        self.scroll_margin = scroll_margin;
        self
    }

    /// Applies stable item keys.
    pub fn with_item_keys(
        mut self,
        item_keys: impl IntoIterator<Item = impl Into<VirtualizerItemKey>>,
    ) -> Self {
        self.item_keys = item_keys.into_iter().map(Into::into).collect();
        self
    }

    /// Adds or replaces one measured item size.
    pub fn with_measurement(mut self, key: impl Into<VirtualizerItemKey>, size: UiPx) -> Self {
        self.measurements_by_key
            .insert(key.into(), nonnegative_px(size));
        self
    }

    /// Seeds this state from a snapshot.
    pub fn with_snapshot(mut self, snapshot: VirtualizerSnapshot) -> Self {
        self.scroll_offset = snapshot.scroll_offset();
        self.measurement_authority =
            (!snapshot.measurements.is_empty()).then(|| snapshot.measurements.clone());
        for item in snapshot.measurements.iter() {
            self.measurements_by_key
                .insert(item.key.clone(), nonnegative_px(item.size));
        }
        self
    }

    /// Returns the item count.
    pub const fn count(&self) -> usize {
        self.count
    }

    /// Returns the configured viewport extent.
    pub const fn viewport_extent(&self) -> UiPx {
        self.viewport_extent
    }

    /// Returns the configured scroll offset.
    pub const fn scroll_offset(&self) -> UiPx {
        self.scroll_offset
    }

    /// Returns the estimated item size.
    pub const fn estimated_size(&self) -> UiPx {
        self.estimated_size
    }

    /// Returns the overscan item budget.
    pub const fn overscan(&self) -> usize {
        self.overscan
    }

    /// Returns the item gap.
    pub const fn gap(&self) -> UiPx {
        self.gap
    }

    /// Returns the scroll margin.
    pub const fn scroll_margin(&self) -> UiPx {
        self.scroll_margin
    }

    /// Returns the measurement cache.
    pub const fn measurements_by_key(&self) -> &BTreeMap<VirtualizerItemKey, UiPx> {
        &self.measurements_by_key
    }

    /// Resolves visible range, overscan range, item measurements, and total size.
    pub fn resolve(&self) -> VirtualizerResolvedState {
        let viewport_extent = nonnegative_px(self.viewport_extent);
        let scroll_offset = nonnegative_px(self.scroll_offset);
        let estimated_size = nonnegative_px(self.estimated_size);
        let gap = nonnegative_px(self.gap);
        let scroll_margin = nonnegative_px(self.scroll_margin);
        let measurements = self.resolve_measurements(estimated_size, gap, scroll_margin);
        let total_size = if self.count == 0 {
            UiPx::ZERO
        } else {
            measurements
                .last()
                .map(|item| item.end() + scroll_margin)
                .unwrap_or(UiPx::ZERO)
        };

        let visible_range = visible_range(&measurements, scroll_offset, viewport_extent);
        let overscan_range = overscan_range(visible_range.clone(), self.count, self.overscan);
        let visible_items = measurements[visible_range.as_range()].to_vec();
        let items = measurements[overscan_range.as_range()].to_vec();
        let snapshot =
            self.snapshot_for_current_keys(scroll_offset, |index| self.key_for_index(index));
        let geometry =
            VirtualizerGeometryIndex::from_measurements(&measurements, gap, scroll_margin);

        VirtualizerResolvedState {
            count: self.count,
            viewport_extent,
            scroll_offset,
            estimated_size,
            overscan: self.overscan,
            gap,
            scroll_margin,
            total_size,
            visible_range,
            overscan_range,
            geometry,
            measurements,
            visible_items,
            items,
            snapshot,
        }
    }

    /// Resolves a fixed-size virtual window without materializing measurements for items that
    /// stay outside the render window.
    ///
    /// This path is intended for adapters like tables that know their item size up front and only
    /// need the rendered window plus a compact measurement snapshot. If cached measurements are
    /// already present, this falls back to the full resolver.
    pub fn resolve_fixed_window(
        &self,
        key_for_index: impl Fn(usize) -> VirtualizerItemKey,
    ) -> VirtualizerResolvedState {
        if !self.measurements_by_key.is_empty() {
            let mut full = self.clone();
            full.item_keys = (0..self.count).map(key_for_index).collect();
            return full.resolve();
        }

        let viewport_extent = nonnegative_px(self.viewport_extent);
        let scroll_offset = nonnegative_px(self.scroll_offset);
        let estimated_size = nonnegative_px(self.estimated_size);
        let gap = nonnegative_px(self.gap);
        let scroll_margin = nonnegative_px(self.scroll_margin);
        let visible_range = fixed_visible_range(
            self.count,
            scroll_offset,
            viewport_extent,
            estimated_size,
            gap,
            scroll_margin,
        );
        let overscan_range = overscan_range(visible_range.clone(), self.count, self.overscan);
        let total_size = fixed_total_size(self.count, estimated_size, gap, scroll_margin);
        let visible_items = fixed_items(
            visible_range.as_range(),
            estimated_size,
            gap,
            scroll_margin,
            &key_for_index,
        );
        let items = fixed_items(
            overscan_range.as_range(),
            estimated_size,
            gap,
            scroll_margin,
            &key_for_index,
        );
        let snapshot = VirtualizerSnapshot::new(scroll_offset, []);
        let geometry =
            VirtualizerGeometryIndex::fixed(self.count, estimated_size, gap, scroll_margin);

        VirtualizerResolvedState {
            count: self.count,
            viewport_extent,
            scroll_offset,
            estimated_size,
            overscan: self.overscan,
            gap,
            scroll_margin,
            total_size,
            visible_range,
            overscan_range,
            geometry,
            measurements: items.clone(),
            visible_items,
            items,
            snapshot,
        }
    }

    /// Resolves a virtual window for items whose exact sizes are known by index.
    ///
    /// This path is intended for adapter-owned horizontal virtualization, such as table columns
    /// whose widths are already resolved. The resolver scans exact sizes to compute total
    /// geometry and range boundaries, but only materializes visible and overscan item measurements
    /// in the resolved output.
    pub fn resolve_known_size_window(
        &self,
        item_for_index: impl Fn(usize) -> (VirtualizerItemKey, UiPx),
    ) -> VirtualizerResolvedState {
        self.resolve_known_size_window_by(
            |index| item_for_index(index).1,
            |index| item_for_index(index).0,
        )
    }

    /// Resolves a known-size virtual window with separate size and key lookups.
    ///
    /// The size callback runs for every item because exact geometry is required for range
    /// resolution. The key callback runs only for the materialized overscan window, which avoids
    /// constructing identities for off-window items.
    pub fn resolve_known_size_window_by(
        &self,
        size_for_index: impl Fn(usize) -> UiPx,
        key_for_index: impl Fn(usize) -> VirtualizerItemKey,
    ) -> VirtualizerResolvedState {
        let geometry = size_driven_geometry(
            self.count,
            nonnegative_px(self.gap),
            nonnegative_px(self.scroll_margin),
            size_for_index,
        );
        self.resolve_known_size_window_from_geometry(geometry, key_for_index)
    }

    /// Resolves a known-size window while reusing caller-owned dense geometry.
    ///
    /// `geometry_revision` must change whenever `size_for_index` may return different values for
    /// the same indices. Scrolling, viewport resizing, and overscan changes intentionally reuse
    /// the cached geometry.
    pub fn resolve_known_size_window_by_cached(
        &self,
        geometry_cache: &mut VirtualizerGeometryCache,
        geometry_revision: u64,
        size_for_index: impl Fn(usize) -> UiPx,
        key_for_index: impl Fn(usize) -> VirtualizerItemKey,
    ) -> VirtualizerResolvedState {
        let estimated_size = nonnegative_px(self.estimated_size);
        let gap = nonnegative_px(self.gap);
        let scroll_margin = nonnegative_px(self.scroll_margin);
        let key = VirtualizerGeometryCacheKey {
            kind: VirtualizerGeometryKind::KnownSize,
            revision: geometry_revision,
            count: self.count,
            estimated_size,
            gap,
            scroll_margin,
            measurement_authority: None,
        };
        let geometry = geometry_cache.resolve_known_size(key, || {
            size_driven_geometry(self.count, gap, scroll_margin, size_for_index)
        });
        self.resolve_known_size_window_from_geometry(geometry, key_for_index)
    }

    fn resolve_known_size_window_from_geometry(
        &self,
        geometry: VirtualizerGeometryIndex,
        key_for_index: impl Fn(usize) -> VirtualizerItemKey,
    ) -> VirtualizerResolvedState {
        let viewport_extent = nonnegative_px(self.viewport_extent);
        let scroll_offset = nonnegative_px(self.scroll_offset);
        let estimated_size = nonnegative_px(self.estimated_size);
        let gap = nonnegative_px(self.gap);
        let scroll_margin = nonnegative_px(self.scroll_margin);
        let total_size = geometry.total_size();
        let visible_range = geometry.visible_range(scroll_offset, viewport_extent);
        let overscan_range = overscan_range(visible_range.clone(), self.count, self.overscan);
        let items = window_measurements(overscan_range.as_range(), &geometry, |index| {
            (key_for_index(index), false)
        });
        let visible_items = if visible_range.is_empty() {
            Vec::new()
        } else {
            let visible = visible_range.as_range();
            items
                .iter()
                .filter(|item| visible.contains(&item.index()))
                .cloned()
                .collect()
        };
        let snapshot = VirtualizerSnapshot::new(scroll_offset, []);

        VirtualizerResolvedState {
            count: self.count,
            viewport_extent,
            scroll_offset,
            estimated_size,
            overscan: self.overscan,
            gap,
            scroll_margin,
            total_size,
            visible_range,
            overscan_range,
            geometry,
            measurements: items.clone(),
            visible_items,
            items,
            snapshot,
        }
    }

    /// Resolves a virtual window using keyed measured sizes when present and estimates otherwise.
    ///
    /// Unlike [`VirtualizerState::resolve`], this path scans the collection to compute total
    /// geometry but only materializes visible and overscan item measurements in the returned
    /// output. The snapshot keeps only measurements whose keys are still present in the current
    /// collection.
    pub fn resolve_measured_window(
        &self,
        key_for_index: impl Fn(usize) -> VirtualizerItemKey,
    ) -> VirtualizerResolvedState {
        let measured = self.build_measured_geometry(|index| {
            let key = key_for_index(index);
            let size = self.measurements_by_key.get(&key).copied();
            (key, size)
        });
        self.resolve_measured_window_from_geometry(measured)
    }

    /// Resolves a measured window while reusing caller-owned dense geometry.
    ///
    /// `geometry_revision` must change whenever caller-owned measurements or item-to-key mapping
    /// change. Snapshot measurement storage is tracked independently and invalidates exactly when
    /// its retained authority changes.
    pub fn resolve_measured_window_cached(
        &self,
        geometry_cache: &mut VirtualizerGeometryCache,
        geometry_revision: u64,
        key_for_index: impl Fn(usize) -> VirtualizerItemKey,
    ) -> VirtualizerResolvedState {
        self.resolve_measured_window_by_cached(geometry_cache, geometry_revision, |index| {
            let key = key_for_index(index);
            let size = self.measurements_by_key.get(&key).copied();
            (key, size)
        })
    }

    /// Resolves a measured window from a borrowed keyed-size lookup while reusing dense geometry.
    ///
    /// The callback runs exactly once per current item when the cache is rebuilt and is not called
    /// on cache hits. `geometry_revision` must change whenever the callback's key or measurement
    /// results may change for an index. Snapshot authority, count, estimate, gap, and scroll margin
    /// are tracked independently; scroll, viewport, and overscan changes preserve the cache.
    pub fn resolve_measured_window_by_cached(
        &self,
        geometry_cache: &mut VirtualizerGeometryCache,
        geometry_revision: u64,
        item_for_index: impl Fn(usize) -> (VirtualizerItemKey, Option<UiPx>),
    ) -> VirtualizerResolvedState {
        let estimated_size = nonnegative_px(self.estimated_size);
        let gap = nonnegative_px(self.gap);
        let scroll_margin = nonnegative_px(self.scroll_margin);
        let key = VirtualizerGeometryCacheKey {
            kind: VirtualizerGeometryKind::Measured,
            revision: geometry_revision,
            count: self.count,
            estimated_size,
            gap,
            scroll_margin,
            measurement_authority: self.measurement_authority.clone(),
        };

        let measured =
            geometry_cache.resolve_measured(key, || self.build_measured_geometry(item_for_index));
        self.resolve_measured_window_from_geometry(measured)
    }

    fn build_measured_geometry(
        &self,
        item_for_index: impl Fn(usize) -> (VirtualizerItemKey, Option<UiPx>),
    ) -> VirtualizerMeasuredGeometry {
        build_measured_geometry(
            self.count,
            nonnegative_px(self.estimated_size),
            nonnegative_px(self.gap),
            nonnegative_px(self.scroll_margin),
            self.measurement_authority.as_ref(),
            item_for_index,
        )
    }

    fn resolve_measured_window_from_geometry(
        &self,
        measured: VirtualizerMeasuredGeometry,
    ) -> VirtualizerResolvedState {
        let viewport_extent = nonnegative_px(self.viewport_extent);
        let scroll_offset = nonnegative_px(self.scroll_offset);
        let estimated_size = nonnegative_px(self.estimated_size);
        let gap = nonnegative_px(self.gap);
        let scroll_margin = nonnegative_px(self.scroll_margin);
        let geometry = measured.geometry;
        let total_size = geometry.total_size();
        let visible_range = geometry.visible_range(scroll_offset, viewport_extent);
        let overscan_range = overscan_range(visible_range.clone(), self.count, self.overscan);
        let items = window_measurements(overscan_range.as_range(), &geometry, |index| {
            let metadata = &measured.item_metadata[index];
            (metadata.key.clone(), metadata.measured)
        });
        let visible_items = if visible_range.is_empty() {
            Vec::new()
        } else {
            let visible = visible_range.as_range();
            items
                .iter()
                .filter(|item| visible.contains(&item.index()))
                .cloned()
                .collect()
        };
        let snapshot = VirtualizerSnapshot::from_shared_measurements(
            scroll_offset,
            measured.snapshot_measurements,
        );

        VirtualizerResolvedState {
            count: self.count,
            viewport_extent,
            scroll_offset,
            estimated_size,
            overscan: self.overscan,
            gap,
            scroll_margin,
            total_size,
            visible_range,
            overscan_range,
            geometry,
            measurements: items.clone(),
            visible_items,
            items,
            snapshot,
        }
    }

    fn resolve_measurements(
        &self,
        estimated_size: UiPx,
        gap: UiPx,
        scroll_margin: UiPx,
    ) -> Vec<VirtualizerItemMeasurement> {
        let mut cursor = scroll_margin;
        (0..self.count)
            .map(|index| {
                let key = self.key_for_index(index);
                let measured_size = self.measurements_by_key.get(&key).copied();
                let size = measured_size.unwrap_or(estimated_size);
                let item = VirtualizerItemMeasurement::new(
                    index,
                    key,
                    cursor,
                    size,
                    measured_size.is_some(),
                );
                cursor = item.end();
                if index + 1 < self.count {
                    cursor = cursor + gap;
                }
                item
            })
            .collect()
    }

    fn key_for_index(&self, index: usize) -> VirtualizerItemKey {
        self.item_keys
            .get(index)
            .cloned()
            .unwrap_or_else(|| VirtualizerItemKey::new(index.to_string()))
    }

    fn snapshot_for_current_keys(
        &self,
        scroll_offset: UiPx,
        key_for_index: impl Fn(usize) -> VirtualizerItemKey,
    ) -> VirtualizerSnapshot {
        if self.measurements_by_key.is_empty() {
            return VirtualizerSnapshot::new(scroll_offset, []);
        }

        let current_keys = (0..self.count).map(key_for_index).collect::<BTreeSet<_>>();
        VirtualizerSnapshot::new(
            scroll_offset,
            self.measurements_by_key
                .iter()
                .filter(|(key, _)| current_keys.contains(*key))
                .map(|(key, size)| VirtualizerSnapshotItem::new(key.clone(), *size)),
        )
    }
}

fn fixed_visible_range(
    count: usize,
    scroll_offset: UiPx,
    viewport_extent: UiPx,
    estimated_size: UiPx,
    gap: UiPx,
    scroll_margin: UiPx,
) -> VirtualizerRange {
    if count == 0 || viewport_extent.as_f32() <= 0.0 {
        return VirtualizerRange::empty();
    }

    let estimated_size = estimated_size.as_f32();
    if estimated_size <= 0.0 {
        return VirtualizerRange::empty();
    }

    let gap = gap.as_f32();
    let stride = estimated_size + gap;
    if stride <= 0.0 {
        return VirtualizerRange::empty();
    }

    let viewport_start = scroll_offset.as_f32();
    let viewport_end = viewport_start + viewport_extent.as_f32();
    let first_item_end = scroll_margin.as_f32() + estimated_size;
    let start = if viewport_start < first_item_end {
        0
    } else {
        (((viewport_start - first_item_end) / stride).floor() as usize).saturating_add(1)
    }
    .min(count);
    let end = if viewport_end <= scroll_margin.as_f32() {
        0
    } else {
        (((viewport_end - scroll_margin.as_f32()) / stride).ceil() as usize).min(count)
    };

    VirtualizerRange::new(start, end.max(start))
}

fn fixed_total_size(count: usize, estimated_size: UiPx, gap: UiPx, scroll_margin: UiPx) -> UiPx {
    if count == 0 {
        return UiPx::ZERO;
    }

    let count = count as f32;
    let estimated_size = estimated_size.as_f32();
    let gap = gap.as_f32();
    let scroll_margin = scroll_margin.as_f32();
    ui_px((count * estimated_size) + ((count - 1.0) * gap) + (scroll_margin * 2.0))
}

fn fixed_items(
    range: Range<usize>,
    estimated_size: UiPx,
    gap: UiPx,
    scroll_margin: UiPx,
    key_for_index: &impl Fn(usize) -> VirtualizerItemKey,
) -> Vec<VirtualizerItemMeasurement> {
    let estimated_size = estimated_size.as_f32();
    let gap = gap.as_f32();
    let scroll_margin = scroll_margin.as_f32();
    let stride = estimated_size + gap;

    range
        .map(|index| {
            let start = ui_px(scroll_margin + (index as f32 * stride));
            let size = ui_px(estimated_size);
            VirtualizerItemMeasurement::new(index, key_for_index(index), start, size, false)
        })
        .collect()
}

fn size_driven_geometry(
    count: usize,
    gap: UiPx,
    scroll_margin: UiPx,
    size_for_index: impl Fn(usize) -> UiPx,
) -> VirtualizerGeometryIndex {
    if count == 0 {
        return VirtualizerGeometryIndex::fixed(0, UiPx::ZERO, gap, scroll_margin);
    }

    let scroll_margin = scroll_margin.as_f32();
    let gap = gap.as_f32();
    let mut cursor = scroll_margin;
    let mut ends = Vec::with_capacity(count);

    for index in 0..count {
        let size = nonnegative_px(size_for_index(index)).as_f32();
        let end = cursor + size;
        ends.push(ui_px(end));

        cursor = end;
        if index + 1 < count {
            cursor += gap;
        }
    }

    VirtualizerGeometryIndex::from_ends(ends, ui_px(gap), ui_px(scroll_margin))
}

fn build_measured_geometry(
    count: usize,
    estimated_size: UiPx,
    gap: UiPx,
    scroll_margin: UiPx,
    preferred_snapshot_authority: Option<&Arc<[VirtualizerSnapshotItem]>>,
    item_for_index: impl Fn(usize) -> (VirtualizerItemKey, Option<UiPx>),
) -> VirtualizerMeasuredGeometry {
    let mut cursor = scroll_margin.as_f32();
    let gap_value = gap.as_f32();
    let mut ends = Vec::with_capacity(count);
    let mut item_metadata = Vec::with_capacity(count);
    let mut snapshot_by_key = BTreeMap::new();

    for index in 0..count {
        let (key, measured_size) = item_for_index(index);
        let measured_size = measured_size.map(nonnegative_px);
        let size = measured_size.unwrap_or(estimated_size);
        let end = cursor + size.as_f32();
        ends.push(ui_px(end));
        item_metadata.push(VirtualizerMeasuredItemMetadata {
            key: key.clone(),
            measured: measured_size.is_some(),
        });

        match measured_size {
            Some(size) => {
                snapshot_by_key.insert(key, size);
            }
            None => {
                snapshot_by_key.remove(&key);
            }
        }

        cursor = end;
        if index + 1 < count {
            cursor += gap_value;
        }
    }

    let snapshot_measurements = snapshot_by_key
        .into_iter()
        .map(|(key, size)| VirtualizerSnapshotItem::new(key, size))
        .collect::<Vec<_>>();
    let snapshot_measurements = preferred_snapshot_authority
        .filter(|authority| authority.as_ref() == snapshot_measurements.as_slice())
        .cloned()
        .unwrap_or_else(|| snapshot_measurements.into());
    let geometry = if count == 0 {
        VirtualizerGeometryIndex::fixed(0, UiPx::ZERO, gap, scroll_margin)
    } else {
        VirtualizerGeometryIndex::from_ends(ends, gap, scroll_margin)
    };

    VirtualizerMeasuredGeometry {
        geometry,
        item_metadata: item_metadata.into(),
        snapshot_measurements,
    }
}

fn window_measurements(
    range: Range<usize>,
    geometry: &VirtualizerGeometryIndex,
    metadata_for_index: impl Fn(usize) -> (VirtualizerItemKey, bool),
) -> Vec<VirtualizerItemMeasurement> {
    range
        .filter_map(|index| {
            let item_geometry = geometry.item(index)?;
            let (key, measured) = metadata_for_index(index);
            Some(VirtualizerItemMeasurement::new(
                index,
                key,
                item_geometry.start(),
                item_geometry.size(),
                measured,
            ))
        })
        .collect()
}

/// Resolved virtualizer output.
#[derive(Debug, Clone, PartialEq)]
pub struct VirtualizerResolvedState {
    count: usize,
    viewport_extent: UiPx,
    scroll_offset: UiPx,
    estimated_size: UiPx,
    overscan: usize,
    gap: UiPx,
    scroll_margin: UiPx,
    total_size: UiPx,
    visible_range: VirtualizerRange,
    overscan_range: VirtualizerRange,
    geometry: VirtualizerGeometryIndex,
    measurements: Vec<VirtualizerItemMeasurement>,
    visible_items: Vec<VirtualizerItemMeasurement>,
    items: Vec<VirtualizerItemMeasurement>,
    snapshot: VirtualizerSnapshot,
}

impl VirtualizerResolvedState {
    /// Returns the item count.
    pub const fn count(&self) -> usize {
        self.count
    }

    /// Returns the resolved viewport extent.
    pub const fn viewport_extent(&self) -> UiPx {
        self.viewport_extent
    }

    /// Returns the resolved scroll offset.
    pub const fn scroll_offset(&self) -> UiPx {
        self.scroll_offset
    }

    /// Returns the resolved estimated item size.
    pub const fn estimated_size(&self) -> UiPx {
        self.estimated_size
    }

    /// Returns the overscan item budget.
    pub const fn overscan(&self) -> usize {
        self.overscan
    }

    /// Returns the item gap.
    pub const fn gap(&self) -> UiPx {
        self.gap
    }

    /// Returns the scroll margin.
    pub const fn scroll_margin(&self) -> UiPx {
        self.scroll_margin
    }

    /// Returns the total virtualized content size.
    pub const fn total_size(&self) -> UiPx {
        self.total_size
    }

    /// Returns exact geometry for an item, including items outside the rendered window.
    pub fn item_geometry(&self, index: usize) -> Option<VirtualizerItemGeometry> {
        self.geometry.item(index)
    }

    /// Returns the visible range before overscan.
    pub const fn visible_range(&self) -> &VirtualizerRange {
        &self.visible_range
    }

    /// Returns the range to render after overscan.
    pub const fn overscan_range(&self) -> &VirtualizerRange {
        &self.overscan_range
    }

    /// Returns the materialized item measurements for this resolved state.
    ///
    /// [`VirtualizerState::resolve`] materializes every item. Fixed-window and known-size window
    /// resolution materialize only the rendered window so large lists can avoid per-frame
    /// full-list measurement output.
    pub fn measurements(&self) -> &[VirtualizerItemMeasurement] {
        &self.measurements
    }

    /// Returns visible item measurements before overscan.
    pub fn visible_items(&self) -> &[VirtualizerItemMeasurement] {
        &self.visible_items
    }

    /// Returns rendered item measurements after overscan.
    pub fn items(&self) -> &[VirtualizerItemMeasurement] {
        &self.items
    }

    /// Returns a snapshot that can seed a future virtualizer state.
    pub const fn snapshot(&self) -> &VirtualizerSnapshot {
        &self.snapshot
    }
}

fn visible_range(
    measurements: &[VirtualizerItemMeasurement],
    scroll_offset: UiPx,
    viewport_extent: UiPx,
) -> VirtualizerRange {
    if measurements.is_empty() || viewport_extent.as_f32() <= 0.0 {
        return VirtualizerRange::empty();
    }

    let viewport_start = scroll_offset.as_f32();
    let viewport_end = viewport_start + viewport_extent.as_f32();
    let start = measurements
        .iter()
        .position(|item| item.end().as_f32() > viewport_start)
        .unwrap_or(measurements.len());
    let end = measurements
        .iter()
        .rposition(|item| item.start().as_f32() < viewport_end)
        .map(|index| index + 1)
        .unwrap_or(start);

    VirtualizerRange::new(start, end.max(start))
}

fn overscan_range(
    visible: VirtualizerRange,
    count: usize,
    overscan_budget: usize,
) -> VirtualizerRange {
    if visible.is_empty() {
        return visible;
    }

    let before_budget = overscan_budget / 2;
    let after_budget = overscan_budget - before_budget;
    let before = before_budget.min(visible.start());
    let after = after_budget.min(count.saturating_sub(visible.end()));
    let unused_before = before_budget - before;
    let unused_after = after_budget - after;
    let start = visible
        .start()
        .saturating_sub(before + unused_after.min(visible.start() - before));
    let end = (visible.end() + after + unused_before).min(count);

    VirtualizerRange::new(start, end)
}

const fn nonnegative_px(value: UiPx) -> UiPx {
    if value.as_f32() < 0.0 {
        ui_px(0.0)
    } else {
        value
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn virtualizer_returns_empty_ranges_for_empty_lists() {
        let resolved = VirtualizerState::new(0, ui_px(20.0))
            .with_viewport_extent(ui_px(100.0))
            .resolve();

        assert_eq!(resolved.total_size(), ui_px(0.0));
        assert!(resolved.visible_range().is_empty());
        assert!(resolved.overscan_range().is_empty());
        assert!(resolved.items().is_empty());
    }

    #[test]
    fn virtualizer_returns_deterministic_ranges_and_total_size() {
        let resolved = VirtualizerState::new(5, ui_px(10.0))
            .with_viewport_extent(ui_px(25.0))
            .with_scroll_offset(ui_px(15.0))
            .with_gap(ui_px(2.0))
            .with_scroll_margin(ui_px(5.0))
            .with_overscan(2)
            .resolve();

        assert_eq!(resolved.total_size(), ui_px(68.0));
        assert_eq!(*resolved.visible_range(), VirtualizerRange::new(1, 3));
        assert_eq!(*resolved.overscan_range(), VirtualizerRange::new(0, 4));
        assert_eq!(
            resolved
                .measurements()
                .iter()
                .map(|item| (item.index(), item.start(), item.end()))
                .collect::<Vec<_>>(),
            [
                (0, ui_px(5.0), ui_px(15.0)),
                (1, ui_px(17.0), ui_px(27.0)),
                (2, ui_px(29.0), ui_px(39.0)),
                (3, ui_px(41.0), ui_px(51.0)),
                (4, ui_px(53.0), ui_px(63.0)),
            ]
        );
    }

    #[test]
    fn zero_viewport_returns_stable_empty_range_without_panicking() {
        let resolved = VirtualizerState::new(10, ui_px(20.0))
            .with_viewport_extent(ui_px(0.0))
            .with_scroll_offset(ui_px(20.0))
            .with_overscan(4)
            .resolve();

        assert!(resolved.visible_range().is_empty());
        assert!(resolved.overscan_range().is_empty());
        assert!(resolved.items().is_empty());
    }

    #[test]
    fn overscan_budget_keeps_rendered_items_bounded() {
        let resolved = VirtualizerState::new(100, ui_px(10.0))
            .with_viewport_extent(ui_px(35.0))
            .with_scroll_offset(ui_px(300.0))
            .with_overscan(6)
            .resolve();

        assert!(resolved.visible_items().len() <= 5);
        assert!(resolved.items().len() <= resolved.visible_items().len() + resolved.overscan());
    }

    #[test]
    fn fixed_window_resolver_materializes_only_rendered_items() {
        let resolved = VirtualizerState::new(10_000, ui_px(30.0))
            .with_viewport_extent(ui_px(196.0))
            .with_scroll_offset(ui_px(3_000.0))
            .with_overscan(5)
            .resolve_fixed_window(|index| VirtualizerItemKey::new(format!("row-{index:04}")));

        assert_eq!(resolved.count(), 10_000);
        assert_eq!(resolved.total_size(), ui_px(300_000.0));
        assert_eq!(*resolved.visible_range(), VirtualizerRange::new(100, 107));
        assert_eq!(*resolved.overscan_range(), VirtualizerRange::new(98, 110));
        assert_eq!(resolved.items().len(), 12);
        assert_eq!(resolved.measurements().len(), resolved.items().len());
        assert_eq!(resolved.items()[0].key().as_str(), "row-0098");
        assert_eq!(
            resolved.item_geometry(9_999),
            Some(VirtualizerItemGeometry::new(ui_px(299_970.0), ui_px(30.0),))
        );
        assert!(resolved.snapshot().measurements().is_empty());
    }

    #[test]
    fn fixed_window_resolver_matches_full_fixed_size_boundaries() {
        for offset in [0.0, 4.0, 5.0, 14.0, 15.0, 16.0, 17.0, 27.0, 65.0, 500.0] {
            let full = VirtualizerState::new(5, ui_px(10.0))
                .with_viewport_extent(ui_px(25.0))
                .with_scroll_offset(ui_px(offset))
                .with_gap(ui_px(2.0))
                .with_scroll_margin(ui_px(5.0))
                .with_overscan(2)
                .with_item_keys((0..5).map(|index| format!("row-{index}")))
                .resolve();
            let fixed = VirtualizerState::new(5, ui_px(10.0))
                .with_viewport_extent(ui_px(25.0))
                .with_scroll_offset(ui_px(offset))
                .with_gap(ui_px(2.0))
                .with_scroll_margin(ui_px(5.0))
                .with_overscan(2)
                .resolve_fixed_window(|index| VirtualizerItemKey::new(format!("row-{index}")));

            assert_eq!(fixed.total_size(), full.total_size(), "offset {offset}");
            assert_eq!(
                fixed.visible_range(),
                full.visible_range(),
                "offset {offset}"
            );
            assert_eq!(
                fixed.overscan_range(),
                full.overscan_range(),
                "offset {offset}"
            );
            assert_eq!(
                fixed.visible_items(),
                full.visible_items(),
                "offset {offset}"
            );
            assert_eq!(fixed.items(), full.items(), "offset {offset}");
            assert_eq!(fixed.snapshot().scroll_offset(), ui_px(offset));
            assert!(fixed.snapshot().measurements().is_empty());
        }
    }

    #[test]
    fn fixed_window_resolver_fallback_keeps_stable_measurement_keys() {
        let resolved = VirtualizerState::new(3, ui_px(20.0))
            .with_viewport_extent(ui_px(80.0))
            .with_measurement("row-1", ui_px(34.0))
            .resolve_fixed_window(|index| VirtualizerItemKey::new(format!("row-{index}")));

        assert_eq!(resolved.measurements()[1].key().as_str(), "row-1");
        assert_eq!(resolved.measurements()[1].size(), ui_px(34.0));
        assert!(resolved.measurements()[1].measured());
    }

    #[test]
    fn known_size_window_resolver_materializes_only_rendered_items() {
        let sizes = [
            ui_px(20.0),
            ui_px(30.0),
            ui_px(40.0),
            ui_px(50.0),
            ui_px(60.0),
            ui_px(70.0),
            ui_px(80.0),
            ui_px(90.0),
            ui_px(100.0),
            ui_px(110.0),
        ];
        let resolved = VirtualizerState::new(sizes.len(), ui_px(999.0))
            .with_viewport_extent(ui_px(100.0))
            .with_scroll_offset(ui_px(120.0))
            .with_overscan(4)
            .resolve_known_size_window(|index| {
                (
                    VirtualizerItemKey::new(format!("col-{index}")),
                    sizes[index],
                )
            });

        assert_eq!(resolved.count(), sizes.len());
        assert_eq!(resolved.total_size(), ui_px(650.0));
        assert_eq!(*resolved.visible_range(), VirtualizerRange::new(3, 6));
        assert_eq!(*resolved.overscan_range(), VirtualizerRange::new(1, 8));
        assert_eq!(resolved.items().len(), 7);
        assert_eq!(resolved.measurements().len(), resolved.items().len());
        assert_eq!(resolved.visible_items().len(), 3);
        assert_eq!(resolved.items()[0].key().as_str(), "col-1");
        assert_eq!(resolved.items()[0].start(), ui_px(20.0));
        assert_eq!(resolved.items()[0].end(), ui_px(50.0));
        assert_eq!(resolved.items().last().unwrap().key().as_str(), "col-7");
        assert_eq!(
            resolved.item_geometry(9),
            Some(VirtualizerItemGeometry::new(ui_px(540.0), ui_px(110.0),))
        );
        assert!(resolved.snapshot().measurements().is_empty());
    }

    #[test]
    fn known_size_window_separates_full_geometry_scan_from_key_materialization() {
        let sizes = [
            ui_px(20.0),
            ui_px(30.0),
            ui_px(40.0),
            ui_px(50.0),
            ui_px(60.0),
            ui_px(70.0),
        ];
        let size_calls = std::cell::Cell::new(0);
        let key_calls = std::cell::Cell::new(0);
        let state = VirtualizerState::new(sizes.len(), ui_px(999.0))
            .with_viewport_extent(ui_px(40.0))
            .with_scroll_offset(ui_px(55.0))
            .with_overscan(1);
        let resolved = state.resolve_known_size_window_by(
            |index| {
                size_calls.set(size_calls.get() + 1);
                sizes[index]
            },
            |index| {
                key_calls.set(key_calls.get() + 1);
                VirtualizerItemKey::new(format!("item-{index}"))
            },
        );
        let wrapped = state.resolve_known_size_window(|index| {
            (
                VirtualizerItemKey::new(format!("item-{index}")),
                sizes[index],
            )
        });

        assert_eq!(size_calls.get(), sizes.len());
        assert_eq!(key_calls.get(), resolved.items().len());
        assert!(key_calls.get() < size_calls.get());
        assert_eq!(resolved, wrapped);
    }

    #[test]
    fn geometry_cache_reuses_dense_sizes_until_the_caller_revision_changes() {
        let sizes = [
            ui_px(20.0),
            ui_px(30.0),
            ui_px(40.0),
            ui_px(50.0),
            ui_px(60.0),
            ui_px(70.0),
        ];
        let size_calls = std::cell::Cell::new(0);
        let key_calls = std::cell::Cell::new(0);
        let mut cache = VirtualizerGeometryCache::default();
        let base = VirtualizerState::new(sizes.len(), ui_px(999.0))
            .with_viewport_extent(ui_px(50.0))
            .with_overscan(1);

        let resolve = |state: VirtualizerState, revision, cache: &mut VirtualizerGeometryCache| {
            state.resolve_known_size_window_by_cached(
                cache,
                revision,
                |index| {
                    size_calls.set(size_calls.get() + 1);
                    sizes[index]
                },
                |index| {
                    key_calls.set(key_calls.get() + 1);
                    VirtualizerItemKey::new(format!("item-{index}"))
                },
            )
        };

        let first = resolve(base.clone(), 7, &mut cache);
        assert_eq!(size_calls.get(), sizes.len());
        assert_eq!(key_calls.get(), first.items().len());
        assert_eq!(
            first.item_geometry(5),
            Some(VirtualizerItemGeometry::new(ui_px(200.0), ui_px(70.0)))
        );

        size_calls.set(0);
        key_calls.set(0);
        let scrolled = resolve(base.clone().with_scroll_offset(ui_px(130.0)), 7, &mut cache);
        assert_eq!(size_calls.get(), 0);
        assert_eq!(key_calls.get(), scrolled.items().len());
        assert_ne!(scrolled.visible_range(), first.visible_range());

        size_calls.set(0);
        key_calls.set(0);
        let resized = resolve(
            base.with_viewport_extent(ui_px(100.0))
                .with_scroll_offset(ui_px(130.0)),
            7,
            &mut cache,
        );
        assert_eq!(size_calls.get(), 0);
        assert_eq!(key_calls.get(), resized.items().len());
        assert!(resized.visible_range().len() > scrolled.visible_range().len());

        size_calls.set(0);
        key_calls.set(0);
        let invalidated = resolve(
            VirtualizerState::new(sizes.len(), ui_px(999.0)).with_viewport_extent(ui_px(50.0)),
            8,
            &mut cache,
        );
        assert_eq!(size_calls.get(), sizes.len());
        assert_eq!(key_calls.get(), invalidated.items().len());
    }

    #[test]
    fn measured_cache_reuses_geometry_metadata_and_snapshot_without_revisiting_items() {
        let measured_sizes = [
            Some(ui_px(10.0)),
            None,
            Some(ui_px(30.0)),
            None,
            Some(ui_px(50.0)),
            None,
        ];
        let item_calls = std::cell::Cell::new(0);
        let mut cache = VirtualizerGeometryCache::default();
        let base = VirtualizerState::new(measured_sizes.len(), ui_px(20.0))
            .with_viewport_extent(ui_px(40.0))
            .with_gap(ui_px(2.0))
            .with_scroll_margin(ui_px(3.0))
            .with_overscan(1);

        let resolve = |state: VirtualizerState, cache: &mut VirtualizerGeometryCache| {
            state.resolve_measured_window_by_cached(cache, 7, |index| {
                item_calls.set(item_calls.get() + 1);
                (
                    VirtualizerItemKey::new(format!("item-{index}")),
                    measured_sizes[index],
                )
            })
        };

        let first = resolve(base.clone(), &mut cache);
        assert_eq!(item_calls.get(), measured_sizes.len());
        assert_eq!(first.total_size(), ui_px(166.0));
        assert_eq!(
            first.item_geometry(4),
            Some(VirtualizerItemGeometry::new(ui_px(91.0), ui_px(50.0)))
        );
        assert_eq!(
            first
                .snapshot()
                .measurements()
                .iter()
                .map(|item| (item.key().as_str(), item.size()))
                .collect::<Vec<_>>(),
            [
                ("item-0", ui_px(10.0)),
                ("item-2", ui_px(30.0)),
                ("item-4", ui_px(50.0)),
            ]
        );

        item_calls.set(0);
        let scrolled = resolve(
            base.with_scroll_offset(ui_px(90.0))
                .with_viewport_extent(ui_px(30.0))
                .with_overscan(4),
            &mut cache,
        );
        assert_eq!(item_calls.get(), 0);
        assert_ne!(scrolled.visible_range(), first.visible_range());
        assert!(
            scrolled
                .snapshot()
                .shares_measurement_authority_with(first.snapshot())
        );
        assert_eq!(scrolled.snapshot().scroll_offset(), ui_px(90.0));
        assert_eq!(
            scrolled.item_geometry(4),
            Some(VirtualizerItemGeometry::new(ui_px(91.0), ui_px(50.0)))
        );
        assert!(
            scrolled
                .items()
                .iter()
                .any(|item| item.key().as_str() == "item-4" && item.measured())
        );
    }

    #[test]
    fn measured_cache_rebuilds_for_every_geometry_authority_input() {
        fn configured_state(count: usize, estimated_size: UiPx) -> VirtualizerState {
            VirtualizerState::new(count, estimated_size)
                .with_viewport_extent(ui_px(50.0))
                .with_gap(ui_px(2.0))
                .with_scroll_margin(ui_px(3.0))
                .with_overscan(2)
        }

        fn assert_rebuilds(
            original: VirtualizerState,
            original_revision: u64,
            changed: VirtualizerState,
            changed_revision: u64,
        ) {
            let calls = std::cell::Cell::new(0);
            let original_count = original.count();
            let changed_count = changed.count();
            let mut cache = VirtualizerGeometryCache::default();
            let resolve =
                |state: VirtualizerState, revision, cache: &mut VirtualizerGeometryCache| {
                    state.resolve_measured_window_by_cached(cache, revision, |index| {
                        calls.set(calls.get() + 1);
                        (
                            VirtualizerItemKey::new(format!("item-{index}")),
                            (index % 2 == 0).then(|| ui_px(10.0 + index as f32)),
                        )
                    })
                };

            resolve(original, original_revision, &mut cache);
            assert_eq!(calls.get(), original_count);
            calls.set(0);
            resolve(changed, changed_revision, &mut cache);
            assert_eq!(calls.get(), changed_count);
        }

        let base = configured_state(6, ui_px(20.0));
        assert_rebuilds(base.clone(), 11, base.clone(), 12);
        assert_rebuilds(base.clone(), 11, configured_state(7, ui_px(20.0)), 11);
        assert_rebuilds(base.clone(), 11, configured_state(6, ui_px(21.0)), 11);
        assert_rebuilds(base.clone(), 11, base.clone().with_gap(ui_px(4.0)), 11);
        assert_rebuilds(
            base.clone(),
            11,
            base.clone().with_scroll_margin(ui_px(5.0)),
            11,
        );

        let first_authority = VirtualizerSnapshot::new(
            ui_px(0.0),
            [VirtualizerSnapshotItem::new(
                VirtualizerItemKey::new("item-0"),
                ui_px(10.0),
            )],
        );
        let distinct_authority = VirtualizerSnapshot::new(
            ui_px(0.0),
            [VirtualizerSnapshotItem::new(
                VirtualizerItemKey::new("item-0"),
                ui_px(10.0),
            )],
        );
        assert_eq!(first_authority, distinct_authority);
        assert!(!first_authority.shares_measurement_authority_with(&distinct_authority));
        assert_rebuilds(
            base.clone().with_snapshot(first_authority),
            11,
            base.with_snapshot(distinct_authority),
            11,
        );
    }

    #[test]
    fn filtered_snapshot_authority_is_reused_on_the_next_cached_resolution() {
        let source_snapshot = VirtualizerSnapshot::new(
            ui_px(0.0),
            [
                VirtualizerSnapshotItem::new(VirtualizerItemKey::new("current-b"), ui_px(33.0)),
                VirtualizerSnapshotItem::new(VirtualizerItemKey::new("removed"), ui_px(99.0)),
            ],
        );
        let measurements = BTreeMap::from([
            (VirtualizerItemKey::new("current-b"), ui_px(33.0)),
            (VirtualizerItemKey::new("removed"), ui_px(99.0)),
        ]);
        let keys = ["current-a", "current-b", "current-c", "current-d"];
        let calls = std::cell::Cell::new(0);
        let mut cache = VirtualizerGeometryCache::default();
        let resolve = |state: VirtualizerState, cache: &mut VirtualizerGeometryCache| {
            state.resolve_measured_window_by_cached(cache, 19, |index| {
                calls.set(calls.get() + 1);
                let key = VirtualizerItemKey::new(keys[index]);
                let size = measurements.get(&key).copied();
                (key, size)
            })
        };
        let base = VirtualizerState::new(keys.len(), ui_px(20.0))
            .with_viewport_extent(ui_px(40.0))
            .with_overscan(1);

        let first = resolve(base.clone().with_snapshot(source_snapshot), &mut cache);
        assert_eq!(calls.get(), keys.len());
        assert_eq!(
            first
                .snapshot()
                .measurements()
                .iter()
                .map(|item| item.key().as_str())
                .collect::<Vec<_>>(),
            ["current-b"]
        );

        calls.set(0);
        let next = resolve(
            base.with_snapshot(first.snapshot().clone())
                .with_scroll_offset(ui_px(35.0)),
            &mut cache,
        );
        assert_eq!(calls.get(), 0);
        assert!(
            next.snapshot()
                .shares_measurement_authority_with(first.snapshot())
        );
        assert_eq!(next.snapshot().scroll_offset(), ui_px(35.0));
    }

    #[test]
    fn measured_cache_preserves_ordered_duplicate_metadata_and_last_snapshot_value() {
        let items = [
            ("duplicate", Some(ui_px(10.0))),
            ("kept", Some(ui_px(20.0))),
            ("duplicate", Some(ui_px(30.0))),
            ("plain", None),
        ];
        let mut cache = VirtualizerGeometryCache::default();
        let resolved = VirtualizerState::new(items.len(), ui_px(15.0))
            .with_viewport_extent(ui_px(100.0))
            .with_overscan(items.len())
            .resolve_measured_window_by_cached(&mut cache, 1, |index| {
                let (key, size) = items[index];
                (VirtualizerItemKey::new(key), size)
            });

        assert_eq!(
            resolved
                .items()
                .iter()
                .map(|item| (item.key().as_str(), item.size(), item.measured()))
                .collect::<Vec<_>>(),
            [
                ("duplicate", ui_px(10.0), true),
                ("kept", ui_px(20.0), true),
                ("duplicate", ui_px(30.0), true),
                ("plain", ui_px(15.0), false),
            ]
        );
        assert_eq!(
            resolved
                .snapshot()
                .measurements()
                .iter()
                .map(|item| (item.key().as_str(), item.size()))
                .collect::<Vec<_>>(),
            [("duplicate", ui_px(30.0)), ("kept", ui_px(20.0)),]
        );

        let restored = VirtualizerState::new(2, ui_px(15.0))
            .with_item_keys(["duplicate", "kept"])
            .with_snapshot(resolved.snapshot().clone())
            .resolve();
        assert_eq!(restored.measurements()[0].size(), ui_px(30.0));
        assert_eq!(restored.measurements()[1].size(), ui_px(20.0));
    }

    #[test]
    fn legacy_measured_cached_resolver_delegates_without_hit_callbacks() {
        let calls = std::cell::Cell::new(0);
        let mut cache = VirtualizerGeometryCache::default();
        let base = VirtualizerState::new(5, ui_px(20.0))
            .with_viewport_extent(ui_px(40.0))
            .with_measurement("item-2", ui_px(42.0));
        let resolve = |state: VirtualizerState, cache: &mut VirtualizerGeometryCache| {
            state.resolve_measured_window_cached(cache, 3, |index| {
                calls.set(calls.get() + 1);
                VirtualizerItemKey::new(format!("item-{index}"))
            })
        };

        resolve(base.clone(), &mut cache);
        assert_eq!(calls.get(), base.count());
        calls.set(0);
        let scrolled = resolve(base.with_scroll_offset(ui_px(30.0)), &mut cache);
        assert_eq!(calls.get(), 0);
        assert_eq!(scrolled.item_geometry(2).unwrap().size(), ui_px(42.0));
    }

    #[test]
    fn snapshot_authority_identity_is_o1_while_equality_remains_content_based() {
        let first = VirtualizerSnapshot::new(
            ui_px(4.0),
            [VirtualizerSnapshotItem::new(
                VirtualizerItemKey::new("same-content"),
                ui_px(24.0),
            )],
        );
        let same_content = VirtualizerSnapshot::new(
            ui_px(4.0),
            [VirtualizerSnapshotItem::new(
                VirtualizerItemKey::new("same-content"),
                ui_px(24.0),
            )],
        );
        let shared = first.clone();
        let empty_a = VirtualizerSnapshot::new(ui_px(1.0), []);
        let empty_b = VirtualizerSnapshot::new(ui_px(2.0), []);

        assert_eq!(first, same_content);
        assert!(!first.shares_measurement_authority_with(&same_content));
        assert!(first.shares_measurement_authority_with(&shared));
        assert!(empty_a.shares_measurement_authority_with(&empty_b));
    }

    #[test]
    fn geometry_cache_debug_output_redacts_dense_geometry_and_item_keys() {
        let mut cache = VirtualizerGeometryCache::default();
        VirtualizerState::new(2, ui_px(20.0))
            .with_viewport_extent(ui_px(40.0))
            .resolve_measured_window_by_cached(&mut cache, 77, |index| {
                (
                    VirtualizerItemKey::new(format!("secret-key-{index}")),
                    Some(ui_px(31.0 + index as f32)),
                )
            });

        assert_eq!(
            format!("{cache:?}"),
            "VirtualizerGeometryCache { populated: true, kind: Some(Measured) }"
        );
        cache.clear();
        assert_eq!(
            format!("{cache:?}"),
            "VirtualizerGeometryCache { populated: false, kind: None }"
        );
    }

    #[test]
    fn known_size_window_resolver_matches_full_variable_size_boundaries() {
        let sizes = [
            ui_px(12.0),
            ui_px(18.0),
            ui_px(24.0),
            ui_px(30.0),
            ui_px(36.0),
        ];

        for offset in [0.0, 5.0, 12.0, 18.0, 29.0, 35.0, 60.0, 99.0] {
            let full = sizes
                .iter()
                .enumerate()
                .fold(
                    VirtualizerState::new(sizes.len(), ui_px(1.0))
                        .with_viewport_extent(ui_px(40.0))
                        .with_scroll_offset(ui_px(offset))
                        .with_gap(ui_px(3.0))
                        .with_scroll_margin(ui_px(5.0))
                        .with_overscan(2)
                        .with_item_keys((0..sizes.len()).map(|index| format!("col-{index}"))),
                    |state, (index, size)| state.with_measurement(format!("col-{index}"), *size),
                )
                .resolve();

            let exact = VirtualizerState::new(sizes.len(), ui_px(1.0))
                .with_viewport_extent(ui_px(40.0))
                .with_scroll_offset(ui_px(offset))
                .with_gap(ui_px(3.0))
                .with_scroll_margin(ui_px(5.0))
                .with_overscan(2)
                .resolve_known_size_window(|index| {
                    (
                        VirtualizerItemKey::new(format!("col-{index}")),
                        sizes[index],
                    )
                });

            assert_eq!(exact.total_size(), full.total_size(), "offset {offset}");
            assert_eq!(
                exact.visible_range(),
                full.visible_range(),
                "offset {offset}"
            );
            assert_eq!(
                exact.overscan_range(),
                full.overscan_range(),
                "offset {offset}"
            );
            assert_eq!(
                measurement_geometry(exact.visible_items()),
                measurement_geometry(full.visible_items()),
                "offset {offset}"
            );
            assert_eq!(
                measurement_geometry(exact.items()),
                measurement_geometry(full.items()),
                "offset {offset}"
            );
            assert_eq!(exact.snapshot().scroll_offset(), ui_px(offset));
            assert!(exact.snapshot().measurements().is_empty());
        }
    }

    #[test]
    fn known_size_window_resolver_handles_empty_zero_viewport_and_zero_width_items() {
        let empty = VirtualizerState::new(0, ui_px(20.0))
            .with_viewport_extent(ui_px(100.0))
            .resolve_known_size_window(|index| {
                (VirtualizerItemKey::new(format!("col-{index}")), ui_px(20.0))
            });
        assert_eq!(empty.total_size(), ui_px(0.0));
        assert!(empty.visible_range().is_empty());
        assert!(empty.overscan_range().is_empty());
        assert!(empty.items().is_empty());

        let zero_viewport = VirtualizerState::new(3, ui_px(20.0))
            .with_viewport_extent(ui_px(0.0))
            .with_scroll_offset(ui_px(40.0))
            .resolve_known_size_window(|index| {
                (VirtualizerItemKey::new(format!("col-{index}")), ui_px(20.0))
            });
        assert_eq!(zero_viewport.total_size(), ui_px(60.0));
        assert!(zero_viewport.visible_range().is_empty());
        assert!(zero_viewport.overscan_range().is_empty());
        assert!(zero_viewport.items().is_empty());

        let zero_width = VirtualizerState::new(3, ui_px(1.0))
            .with_viewport_extent(ui_px(100.0))
            .resolve_known_size_window(|index| {
                (VirtualizerItemKey::new(format!("col-{index}")), ui_px(0.0))
            });
        assert_eq!(zero_width.total_size(), ui_px(0.0));
        assert!(zero_width.visible_range().is_empty());
        assert!(zero_width.overscan_range().is_empty());
        assert!(zero_width.items().is_empty());
    }

    fn measurement_geometry(
        measurements: &[VirtualizerItemMeasurement],
    ) -> Vec<(usize, &str, UiPx, UiPx, UiPx)> {
        measurements
            .iter()
            .map(|item| {
                (
                    item.index(),
                    item.key().as_str(),
                    item.start(),
                    item.size(),
                    item.end(),
                )
            })
            .collect()
    }

    #[test]
    fn repeated_measurement_for_same_key_is_idempotent() {
        let first = VirtualizerState::new(3, ui_px(20.0))
            .with_item_keys(["a", "b", "c"])
            .with_measurement("b", ui_px(32.0))
            .resolve();
        let second = VirtualizerState::new(3, ui_px(20.0))
            .with_item_keys(["a", "b", "c"])
            .with_measurement("b", ui_px(32.0))
            .with_measurement("b", ui_px(32.0))
            .resolve();

        assert_eq!(first.measurements(), second.measurements());
        assert_eq!(first.total_size(), second.total_size());
    }

    #[test]
    fn key_stable_snapshot_restores_measured_window() {
        let resolved = VirtualizerState::new(3, ui_px(20.0))
            .with_item_keys(["a", "b", "c"])
            .with_viewport_extent(ui_px(60.0))
            .with_scroll_offset(ui_px(10.0))
            .with_measurement("b", ui_px(34.0))
            .resolve();
        let restored = VirtualizerState::new(3, ui_px(20.0))
            .with_item_keys(["a", "b", "c"])
            .with_viewport_extent(ui_px(60.0))
            .with_snapshot(resolved.snapshot().clone())
            .resolve();

        assert_eq!(restored.scroll_offset(), ui_px(10.0));
        assert_eq!(restored.measurements(), resolved.measurements());
        assert_eq!(restored.visible_range(), resolved.visible_range());
    }

    #[test]
    fn changed_keys_invalidate_only_affected_measurements() {
        let restored = VirtualizerState::new(3, ui_px(20.0))
            .with_item_keys(["x", "b", "c"])
            .with_snapshot(VirtualizerSnapshot::new(
                ui_px(0.0),
                [
                    VirtualizerSnapshotItem::new(VirtualizerItemKey::new("a"), ui_px(40.0)),
                    VirtualizerSnapshotItem::new(VirtualizerItemKey::new("b"), ui_px(30.0)),
                ],
            ))
            .resolve();

        assert_eq!(restored.measurements()[0].size(), ui_px(20.0));
        assert!(!restored.measurements()[0].measured());
        assert_eq!(restored.measurements()[1].size(), ui_px(30.0));
        assert!(restored.measurements()[1].measured());
        assert_eq!(
            restored
                .snapshot()
                .measurements()
                .iter()
                .map(|item| item.key().as_str())
                .collect::<Vec<_>>(),
            ["b"]
        );
    }

    #[test]
    fn measured_window_restores_keyed_measurements_without_full_materialization() {
        let restored = VirtualizerState::new(100, ui_px(20.0))
            .with_viewport_extent(ui_px(48.0))
            .with_overscan(2)
            .with_snapshot(VirtualizerSnapshot::new(
                ui_px(0.0),
                [
                    VirtualizerSnapshotItem::new(VirtualizerItemKey::new("b"), ui_px(34.0)),
                    VirtualizerSnapshotItem::new(VirtualizerItemKey::new("removed"), ui_px(88.0)),
                ],
            ))
            .resolve_measured_window(|index| match index {
                0 => VirtualizerItemKey::new("b"),
                1 => VirtualizerItemKey::new("a"),
                _ => VirtualizerItemKey::new(format!("row-{index}")),
            });

        assert_eq!(restored.count(), 100);
        assert_eq!(restored.items()[0].key().as_str(), "b");
        assert_eq!(restored.items()[0].size(), ui_px(34.0));
        assert!(restored.items()[0].measured());
        assert!(restored.measurements().len() < restored.count());
        assert_eq!(
            restored.item_geometry(99),
            Some(VirtualizerItemGeometry::new(ui_px(1_994.0), ui_px(20.0),))
        );
        assert_eq!(
            restored
                .snapshot()
                .measurements()
                .iter()
                .map(|item| item.key().as_str())
                .collect::<Vec<_>>(),
            ["b"]
        );
    }

    #[test]
    fn measured_window_clamps_invalid_measurements() {
        let resolved = VirtualizerState::new(3, ui_px(20.0))
            .with_viewport_extent(ui_px(80.0))
            .with_overscan(3)
            .with_measurement("a", ui_px(-5.0))
            .with_measurement("b", ui_px(35.0))
            .resolve_measured_window(|index| {
                VirtualizerItemKey::new(match index {
                    0 => "a".to_owned(),
                    1 => "b".to_owned(),
                    _ => "c".to_owned(),
                })
            });

        assert_eq!(resolved.total_size(), ui_px(55.0));
        assert_eq!(resolved.measurements()[0].size(), ui_px(0.0));
        assert!(resolved.measurements()[0].measured());
        assert_eq!(resolved.measurements()[1].size(), ui_px(35.0));
        assert!(resolved.measurements()[1].measured());
    }
}
