//! Renderer-neutral one-dimensional virtualizer contracts for Open GPUI components.

use crate::geometry::{UiPx, ui_px};
use std::collections::{BTreeMap, BTreeSet};
use std::ops::Range;

/// Convenient imports for renderer-neutral virtualizer work.
pub mod prelude {
    pub use super::{
        VirtualizerItemKey, VirtualizerItemMeasurement, VirtualizerRange, VirtualizerResolvedState,
        VirtualizerSnapshot, VirtualizerSnapshotItem, VirtualizerState,
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
    measurements: Vec<VirtualizerSnapshotItem>,
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
        for item in snapshot.measurements {
            self.measurements_by_key
                .insert(item.key, nonnegative_px(item.size));
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
        let viewport_extent = nonnegative_px(self.viewport_extent);
        let scroll_offset = nonnegative_px(self.scroll_offset);
        let estimated_size = nonnegative_px(self.estimated_size);
        let gap = nonnegative_px(self.gap);
        let scroll_margin = nonnegative_px(self.scroll_margin);
        let (total_size, visible_range) = known_size_geometry(
            self.count,
            scroll_offset,
            viewport_extent,
            gap,
            scroll_margin,
            &item_for_index,
        );
        let overscan_range = overscan_range(visible_range.clone(), self.count, self.overscan);
        let items = known_size_items(
            overscan_range.as_range(),
            self.count,
            gap,
            scroll_margin,
            &item_for_index,
        );
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
        let viewport_extent = nonnegative_px(self.viewport_extent);
        let scroll_offset = nonnegative_px(self.scroll_offset);
        let estimated_size = nonnegative_px(self.estimated_size);
        let gap = nonnegative_px(self.gap);
        let scroll_margin = nonnegative_px(self.scroll_margin);
        let item_for_index = |index| {
            let key = key_for_index(index);
            let measured_size = self.measurements_by_key.get(&key).copied();
            (
                key,
                measured_size.unwrap_or(estimated_size),
                measured_size.is_some(),
            )
        };
        let (total_size, visible_range) = measured_window_geometry(
            self.count,
            scroll_offset,
            viewport_extent,
            gap,
            scroll_margin,
            &item_for_index,
        );
        let overscan_range = overscan_range(visible_range.clone(), self.count, self.overscan);
        let items = measured_window_items(
            overscan_range.as_range(),
            self.count,
            gap,
            scroll_margin,
            &item_for_index,
        );
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
        let snapshot = self.snapshot_for_current_keys(scroll_offset, key_for_index);

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

fn known_size_geometry(
    count: usize,
    scroll_offset: UiPx,
    viewport_extent: UiPx,
    gap: UiPx,
    scroll_margin: UiPx,
    item_for_index: &impl Fn(usize) -> (VirtualizerItemKey, UiPx),
) -> (UiPx, VirtualizerRange) {
    if count == 0 {
        return (UiPx::ZERO, VirtualizerRange::empty());
    }

    let scroll_margin = scroll_margin.as_f32();
    let gap = gap.as_f32();
    let viewport_start = scroll_offset.as_f32();
    let viewport_end = viewport_start + viewport_extent.as_f32();
    let can_resolve_visible = viewport_extent.as_f32() > 0.0;
    let mut cursor = scroll_margin;
    let mut visible_start = None;
    let mut visible_end = None;

    for index in 0..count {
        let (_, size) = item_for_index(index);
        let size = nonnegative_px(size).as_f32();
        let start = cursor;
        let end = start + size;

        if can_resolve_visible {
            if visible_start.is_none() && end > viewport_start {
                visible_start = Some(index);
            }
            if start < viewport_end {
                visible_end = Some(index + 1);
            }
        }

        cursor = end;
        if index + 1 < count {
            cursor += gap;
        }
    }

    let total_size = ui_px(cursor + scroll_margin);
    let visible_range = if let Some(start) = visible_start {
        VirtualizerRange::new(start, visible_end.unwrap_or(start).max(start))
    } else {
        VirtualizerRange::empty()
    };

    (total_size, visible_range)
}

fn known_size_items(
    range: Range<usize>,
    count: usize,
    gap: UiPx,
    scroll_margin: UiPx,
    item_for_index: &impl Fn(usize) -> (VirtualizerItemKey, UiPx),
) -> Vec<VirtualizerItemMeasurement> {
    if range.is_empty() {
        return Vec::new();
    }

    let scroll_margin = scroll_margin.as_f32();
    let gap = gap.as_f32();
    let mut cursor = scroll_margin;
    let mut items = Vec::with_capacity(range.len());

    for index in 0..range.end.min(count) {
        let (key, size) = item_for_index(index);
        let size = nonnegative_px(size);
        let start = ui_px(cursor);
        let end = cursor + size.as_f32();

        if range.contains(&index) {
            items.push(VirtualizerItemMeasurement::new(
                index, key, start, size, false,
            ));
        }

        cursor = end;
        if index + 1 < count {
            cursor += gap;
        }
    }

    items
}

fn measured_window_geometry(
    count: usize,
    scroll_offset: UiPx,
    viewport_extent: UiPx,
    gap: UiPx,
    scroll_margin: UiPx,
    item_for_index: &impl Fn(usize) -> (VirtualizerItemKey, UiPx, bool),
) -> (UiPx, VirtualizerRange) {
    if count == 0 {
        return (UiPx::ZERO, VirtualizerRange::empty());
    }

    let scroll_margin = scroll_margin.as_f32();
    let gap = gap.as_f32();
    let viewport_start = scroll_offset.as_f32();
    let viewport_end = viewport_start + viewport_extent.as_f32();
    let can_resolve_visible = viewport_extent.as_f32() > 0.0;
    let mut cursor = scroll_margin;
    let mut visible_start = None;
    let mut visible_end = None;

    for index in 0..count {
        let (_, size, _) = item_for_index(index);
        let size = nonnegative_px(size).as_f32();
        let start = cursor;
        let end = start + size;

        if can_resolve_visible {
            if visible_start.is_none() && end > viewport_start {
                visible_start = Some(index);
            }
            if start < viewport_end {
                visible_end = Some(index + 1);
            }
        }

        cursor = end;
        if index + 1 < count {
            cursor += gap;
        }
    }

    let total_size = ui_px(cursor + scroll_margin);
    let visible_range = if let Some(start) = visible_start {
        VirtualizerRange::new(start, visible_end.unwrap_or(start).max(start))
    } else {
        VirtualizerRange::empty()
    };

    (total_size, visible_range)
}

fn measured_window_items(
    range: Range<usize>,
    count: usize,
    gap: UiPx,
    scroll_margin: UiPx,
    item_for_index: &impl Fn(usize) -> (VirtualizerItemKey, UiPx, bool),
) -> Vec<VirtualizerItemMeasurement> {
    if range.is_empty() {
        return Vec::new();
    }

    let scroll_margin = scroll_margin.as_f32();
    let gap = gap.as_f32();
    let mut cursor = scroll_margin;
    let mut items = Vec::with_capacity(range.len());

    for index in 0..range.end.min(count) {
        let (key, size, measured) = item_for_index(index);
        let size = nonnegative_px(size);
        let start = ui_px(cursor);
        let end = cursor + size.as_f32();

        if range.contains(&index) {
            items.push(VirtualizerItemMeasurement::new(
                index, key, start, size, measured,
            ));
        }

        cursor = end;
        if index + 1 < count {
            cursor += gap;
        }
    }

    items
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
        assert!(resolved.snapshot().measurements().is_empty());
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
