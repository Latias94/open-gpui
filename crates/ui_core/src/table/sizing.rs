//! Column sizing, resize, and resolved-width contracts for renderer-neutral tables.

use std::collections::BTreeMap;

use crate::geometry::{UiPx, ui_px};

use super::columns::{TableColumn, TableColumnRegion, TableColumnRegions};
use super::identity::TableColumnId;

/// Caller-owned committed column sizing map.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct TableColumnSizing {
    widths: BTreeMap<TableColumnId, UiPx>,
}

impl TableColumnSizing {
    /// Creates an empty sizing map.
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a sizing map from explicit column widths.
    pub fn from_widths(widths: impl IntoIterator<Item = (impl Into<TableColumnId>, UiPx)>) -> Self {
        let mut sizing = Self::default();
        for (column, width) in widths {
            sizing = sizing.with_width(column, width);
        }
        sizing
    }

    /// Returns the committed width for a column, if present.
    pub fn width(&self, column: &TableColumnId) -> Option<UiPx> {
        self.widths.get(column).copied()
    }

    /// Returns the committed sizing map.
    pub fn widths(&self) -> &BTreeMap<TableColumnId, UiPx> {
        &self.widths
    }

    /// Returns whether no committed widths exist.
    pub fn is_empty(&self) -> bool {
        self.widths.is_empty()
    }

    /// Inserts or updates a committed column width.
    pub fn with_width(mut self, column: impl Into<TableColumnId>, width: UiPx) -> Self {
        self.widths
            .insert(column.into(), normalized_column_width(width));
        self
    }

    /// Removes a committed column width.
    pub fn without_width(mut self, column: impl Into<TableColumnId>) -> Self {
        self.widths.remove(&column.into());
        self
    }
}

pub(super) fn normalized_column_width(width: UiPx) -> UiPx {
    let raw = width.as_f32();
    if raw.is_finite() {
        ui_px(raw.max(0.0))
    } else {
        UiPx::ZERO
    }
}

pub(super) fn clamp_column_width(width: UiPx, min_width: UiPx, max_width: UiPx) -> UiPx {
    let min_width = normalized_column_width(min_width);
    let max_width = normalized_column_width(max_width).max(min_width);
    normalized_column_width(width).max(min_width).min(max_width)
}

fn finite_ui_px(value: UiPx) -> UiPx {
    if value.as_f32().is_finite() {
        value
    } else {
        UiPx::ZERO
    }
}

/// Determines when drag interactions commit column sizing changes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TableColumnResizeMode {
    /// Commit widths while the resize drag is moving.
    OnChange,
    /// Commit widths when the resize drag finishes.
    #[default]
    OnEnd,
}

/// Direction used when converting horizontal pointer movement into width deltas.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TableColumnResizeDirection {
    /// Positive pointer movement increases width.
    #[default]
    Ltr,
    /// Positive pointer movement decreases width.
    Rtl,
}

/// Transient state for an active table column resize interaction.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct TableColumnResizeState {
    column_widths_start: Vec<(TableColumnId, UiPx)>,
    delta_offset: Option<UiPx>,
    delta_percentage: Option<f32>,
    active_column: Option<TableColumnId>,
    start_offset: Option<UiPx>,
    start_width: Option<UiPx>,
}

impl TableColumnResizeState {
    /// Starts a column resize interaction.
    pub fn begin(
        active_column: impl Into<TableColumnId>,
        start_offset: UiPx,
        start_width: UiPx,
        column_widths_start: impl IntoIterator<Item = (impl Into<TableColumnId>, UiPx)>,
    ) -> Self {
        Self {
            column_widths_start: column_widths_start
                .into_iter()
                .map(|(column, width)| (column.into(), normalized_column_width(width)))
                .collect(),
            delta_offset: Some(UiPx::ZERO),
            delta_percentage: Some(0.0),
            active_column: Some(active_column.into()),
            start_offset: Some(finite_ui_px(start_offset)),
            start_width: Some(normalized_column_width(start_width)),
        }
    }

    /// Returns true when a resize drag is active.
    pub fn is_resizing(&self) -> bool {
        self.active_column.is_some()
    }

    /// Returns the active resize column, if present.
    pub const fn active_column(&self) -> Option<&TableColumnId> {
        self.active_column.as_ref()
    }

    /// Returns the start pointer offset for the drag.
    pub const fn start_offset(&self) -> Option<UiPx> {
        self.start_offset
    }

    /// Returns the starting width of the active header.
    pub const fn start_width(&self) -> Option<UiPx> {
        self.start_width
    }

    /// Returns the latest pointer delta in logical pixels.
    pub const fn delta_offset(&self) -> Option<UiPx> {
        self.delta_offset
    }

    /// Returns the latest pointer delta as a percentage of the start width.
    pub const fn delta_percentage(&self) -> Option<f32> {
        self.delta_percentage
    }

    /// Returns the starting widths captured for this interaction.
    pub fn column_widths_start(&self) -> &[(TableColumnId, UiPx)] {
        &self.column_widths_start
    }

    /// Returns the preview width for a captured column, if the drag has moved.
    pub fn preview_width(&self, column: &TableColumnId) -> Option<UiPx> {
        let delta_percentage = self.delta_percentage?;
        let (_, start_width) = self
            .column_widths_start
            .iter()
            .find(|(id, _)| id == column)?;
        Some(resized_column_width(*start_width, delta_percentage))
    }
}

/// Result of applying a table column resize transition.
#[derive(Debug, Clone, PartialEq)]
pub struct TableColumnResizeUpdate {
    state: TableColumnResizeState,
    committed_sizing: Option<TableColumnSizing>,
}

impl TableColumnResizeUpdate {
    fn new(state: TableColumnResizeState, committed_sizing: Option<TableColumnSizing>) -> Self {
        Self {
            state,
            committed_sizing,
        }
    }

    /// Returns the next transient resize state.
    pub const fn state(&self) -> &TableColumnResizeState {
        &self.state
    }

    /// Returns committed sizing when the transition should publish widths.
    pub const fn committed_sizing(&self) -> Option<&TableColumnSizing> {
        self.committed_sizing.as_ref()
    }

    /// Consumes the update into owned parts.
    pub fn into_parts(self) -> (TableColumnResizeState, Option<TableColumnSizing>) {
        (self.state, self.committed_sizing)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TableColumnResizeEvent {
    Move,
    End,
}

/// Applies a resize drag movement.
pub fn drag_table_column_resize(
    mode: TableColumnResizeMode,
    direction: TableColumnResizeDirection,
    sizing: &TableColumnSizing,
    state: &TableColumnResizeState,
    client_x: UiPx,
) -> TableColumnResizeUpdate {
    update_table_column_resize(
        mode,
        direction,
        sizing,
        state,
        TableColumnResizeEvent::Move,
        Some(client_x),
    )
}

/// Finishes a resize drag.
pub fn end_table_column_resize(
    mode: TableColumnResizeMode,
    direction: TableColumnResizeDirection,
    sizing: &TableColumnSizing,
    state: &TableColumnResizeState,
    client_x: Option<UiPx>,
) -> TableColumnResizeUpdate {
    let update = update_table_column_resize(
        mode,
        direction,
        sizing,
        state,
        TableColumnResizeEvent::End,
        client_x,
    );
    TableColumnResizeUpdate::new(TableColumnResizeState::default(), update.committed_sizing)
}

fn update_table_column_resize(
    mode: TableColumnResizeMode,
    direction: TableColumnResizeDirection,
    sizing: &TableColumnSizing,
    state: &TableColumnResizeState,
    event: TableColumnResizeEvent,
    client_x: Option<UiPx>,
) -> TableColumnResizeUpdate {
    let Some(client_x) = client_x else {
        return TableColumnResizeUpdate::new(state.clone(), None);
    };
    let Some(start_offset) = state.start_offset else {
        return TableColumnResizeUpdate::new(state.clone(), None);
    };
    let Some(start_width) = state.start_width else {
        return TableColumnResizeUpdate::new(state.clone(), None);
    };
    if start_width.as_f32() <= 0.0 {
        return TableColumnResizeUpdate::new(state.clone(), None);
    }

    let direction_multiplier = match direction {
        TableColumnResizeDirection::Ltr => 1.0,
        TableColumnResizeDirection::Rtl => -1.0,
    };
    let delta_offset = (client_x - start_offset) * direction_multiplier;
    let delta_percentage = (delta_offset.as_f32() / start_width.as_f32()).max(-0.999_999);
    let mut next_state = state.clone();
    next_state.delta_offset = Some(delta_offset);
    next_state.delta_percentage = Some(delta_percentage);

    let should_commit =
        mode == TableColumnResizeMode::OnChange || event == TableColumnResizeEvent::End;
    let committed_sizing = should_commit.then(|| {
        let mut next_sizing = sizing.clone();
        for (column, width) in &state.column_widths_start {
            next_sizing = next_sizing.with_width(
                column.clone(),
                resized_column_width(*width, delta_percentage),
            );
        }
        next_sizing
    });

    TableColumnResizeUpdate::new(next_state, committed_sizing)
}

fn resized_column_width(start_width: UiPx, delta_percentage: f32) -> UiPx {
    let raw = start_width.as_f32() + start_width.as_f32() * delta_percentage;
    round_column_width(ui_px(raw.max(0.0)))
}

fn round_column_width(width: UiPx) -> UiPx {
    let raw = normalized_column_width(width).as_f32();
    ui_px((raw * 100.0).round() / 100.0)
}

/// Resolved sizing metadata for one visible table column.
#[derive(Debug, Clone, PartialEq)]
pub struct TableResolvedColumnSizing {
    column_id: TableColumnId,
    region: TableColumnRegion,
    width: UiPx,
    min_width: UiPx,
    max_width: UiPx,
    start: UiPx,
    after: UiPx,
    resizable: bool,
}

impl TableResolvedColumnSizing {
    fn new(
        column: &TableColumn,
        region: TableColumnRegion,
        width: UiPx,
        start: UiPx,
        after: UiPx,
    ) -> Self {
        Self {
            column_id: column.id().clone(),
            region,
            width,
            min_width: column.min_width(),
            max_width: column.max_width(),
            start,
            after,
            resizable: column.resizable(),
        }
    }

    /// Returns the stable column identity.
    pub const fn column_id(&self) -> &TableColumnId {
        &self.column_id
    }

    /// Returns the resolved pinning region for this column.
    pub const fn region(&self) -> TableColumnRegion {
        self.region
    }

    /// Returns the resolved column width.
    pub const fn width(&self) -> UiPx {
        self.width
    }

    /// Returns the lower width bound.
    pub const fn min_width(&self) -> UiPx {
        self.min_width
    }

    /// Returns the upper width bound.
    pub const fn max_width(&self) -> UiPx {
        self.max_width
    }

    /// Returns the offset from the start edge of this column's region.
    pub const fn start(&self) -> UiPx {
        self.start
    }

    /// Returns the offset from the end edge of this column to the region end.
    pub const fn after(&self) -> UiPx {
        self.after
    }

    /// Returns whether this column accepts resize interactions.
    pub const fn resizable(&self) -> bool {
        self.resizable
    }
}

/// Resolved visible column sizing split into render regions.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct TableResolvedColumnSizingRegions {
    left: Vec<TableResolvedColumnSizing>,
    center: Vec<TableResolvedColumnSizing>,
    right: Vec<TableResolvedColumnSizing>,
    left_width: UiPx,
    center_width: UiPx,
    right_width: UiPx,
    total_width: UiPx,
}

impl TableResolvedColumnSizingRegions {
    pub(super) fn from_column_regions(
        regions: &TableColumnRegions,
        sizing: &TableColumnSizing,
    ) -> Self {
        let (left, left_width) =
            resolve_column_sizing_region(TableColumnRegion::Left, regions.left(), sizing);
        let (center, center_width) =
            resolve_column_sizing_region(TableColumnRegion::Center, regions.center(), sizing);
        let (right, right_width) =
            resolve_column_sizing_region(TableColumnRegion::Right, regions.right(), sizing);

        Self {
            left,
            center,
            right,
            left_width,
            center_width,
            right_width,
            total_width: left_width + center_width + right_width,
        }
    }

    /// Returns visible left-pinned column sizing.
    pub fn left(&self) -> &[TableResolvedColumnSizing] {
        &self.left
    }

    /// Returns visible unpinned center column sizing.
    pub fn center(&self) -> &[TableResolvedColumnSizing] {
        &self.center
    }

    /// Returns visible right-pinned column sizing.
    pub fn right(&self) -> &[TableResolvedColumnSizing] {
        &self.right
    }

    /// Returns visible column sizing for a region.
    pub fn region(&self, region: TableColumnRegion) -> &[TableResolvedColumnSizing] {
        match region {
            TableColumnRegion::Left => self.left(),
            TableColumnRegion::Center => self.center(),
            TableColumnRegion::Right => self.right(),
        }
    }

    /// Returns all visible column sizing in render order.
    pub fn all(&self) -> impl Iterator<Item = &TableResolvedColumnSizing> {
        self.left
            .iter()
            .chain(self.center.iter())
            .chain(self.right.iter())
    }

    /// Returns the sizing metadata for a visible column.
    pub fn column(&self, column: &TableColumnId) -> Option<&TableResolvedColumnSizing> {
        self.all().find(|sizing| sizing.column_id() == column)
    }

    /// Returns the total width across all visible columns.
    pub const fn total_width(&self) -> UiPx {
        self.total_width
    }

    /// Returns the total width for a specific region.
    pub const fn region_width(&self, region: TableColumnRegion) -> UiPx {
        match region {
            TableColumnRegion::Left => self.left_width,
            TableColumnRegion::Center => self.center_width,
            TableColumnRegion::Right => self.right_width,
        }
    }

    /// Returns the left-pinned region width.
    pub const fn left_width(&self) -> UiPx {
        self.left_width
    }

    /// Returns the unpinned center region width.
    pub const fn center_width(&self) -> UiPx {
        self.center_width
    }

    /// Returns the right-pinned region width.
    pub const fn right_width(&self) -> UiPx {
        self.right_width
    }

    /// Returns the number of visible columns across all regions.
    pub fn len(&self) -> usize {
        self.left.len() + self.center.len() + self.right.len()
    }

    /// Returns true when no visible column sizing exists.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

fn resolve_column_sizing_region(
    region: TableColumnRegion,
    columns: &[TableColumn],
    sizing: &TableColumnSizing,
) -> (Vec<TableResolvedColumnSizing>, UiPx) {
    let widths = columns
        .iter()
        .map(|column| (column, column.resolved_width(sizing)))
        .collect::<Vec<_>>();
    let total_width = widths
        .iter()
        .fold(UiPx::ZERO, |total, (_, width)| total + *width);
    let mut start = UiPx::ZERO;
    let mut resolved = Vec::with_capacity(widths.len());

    for (column, width) in widths {
        let after = total_width - start - width;
        resolved.push(TableResolvedColumnSizing::new(
            column, region, width, start, after,
        ));
        start = start + width;
    }

    (resolved, total_width)
}
