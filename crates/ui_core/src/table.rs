//! Renderer-neutral table row-model contracts for Open GPUI components.

use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering as AtomicOrdering};

use crate::geometry::{UiPx, ui_px};

static NEXT_TABLE_ROWS_IDENTITY: AtomicU64 = AtomicU64::new(1);

/// Default preferred width for a table column.
pub const TABLE_DEFAULT_COLUMN_WIDTH: UiPx = ui_px(128.0);

/// Default minimum width for a table column.
pub const TABLE_MIN_COLUMN_WIDTH: UiPx = ui_px(40.0);

/// Default maximum width for a table column.
pub const TABLE_MAX_COLUMN_WIDTH: UiPx = ui_px(1_000_000.0);

/// Stable renderer-neutral identity for a table row.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TableRowId(String);

impl TableRowId {
    /// Creates a row identity from a stable string.
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    /// Returns the stable string value.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<&str> for TableRowId {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

impl From<String> for TableRowId {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

/// Stable renderer-neutral identity for a table column.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TableColumnId(String);

impl TableColumnId {
    /// Creates a column identity from a stable string.
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    /// Returns the stable string value.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<&str> for TableColumnId {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

impl From<String> for TableColumnId {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

/// Renderer-neutral scalar value used by table filtering and sorting.
#[derive(Debug, Clone, PartialEq)]
pub enum TableCellValue {
    /// No meaningful value is present.
    Empty,
    /// Text value.
    Text(String),
    /// Numeric value.
    Number(f64),
    /// Boolean value.
    Bool(bool),
}

impl TableCellValue {
    /// Returns a stable string for filtering and debug output.
    pub fn filter_text(&self) -> String {
        match self {
            Self::Empty => String::new(),
            Self::Text(value) => value.clone(),
            Self::Number(value) => value.to_string(),
            Self::Bool(value) => value.to_string(),
        }
    }

    fn cmp_for_sort(&self, other: &Self) -> Ordering {
        match (self, other) {
            (Self::Number(left), Self::Number(right)) => left.total_cmp(right),
            (Self::Bool(left), Self::Bool(right)) => left.cmp(right),
            (Self::Empty, Self::Empty) => Ordering::Equal,
            (Self::Empty, _) => Ordering::Less,
            (_, Self::Empty) => Ordering::Greater,
            _ => self.filter_text().cmp(&other.filter_text()),
        }
    }
}

impl Default for TableCellValue {
    fn default() -> Self {
        Self::Empty
    }
}

impl From<&str> for TableCellValue {
    fn from(value: &str) -> Self {
        Self::Text(value.to_string())
    }
}

impl From<String> for TableCellValue {
    fn from(value: String) -> Self {
        Self::Text(value)
    }
}

impl From<f64> for TableCellValue {
    fn from(value: f64) -> Self {
        Self::Number(value)
    }
}

impl From<i64> for TableCellValue {
    fn from(value: i64) -> Self {
        Self::Number(value as f64)
    }
}

impl From<usize> for TableCellValue {
    fn from(value: usize) -> Self {
        Self::Number(value as f64)
    }
}

impl From<bool> for TableCellValue {
    fn from(value: bool) -> Self {
        Self::Bool(value)
    }
}

/// Renderer-neutral cell editor kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TableCellEditor {
    /// Single-line text editing with app-owned values.
    Text,
}

impl TableCellEditor {
    /// Returns a stable label.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Text => "text",
        }
    }
}

/// Renderer-neutral column descriptor.
#[derive(Debug, Clone, PartialEq)]
pub struct TableColumn {
    id: TableColumnId,
    label: String,
    visible: bool,
    sortable: bool,
    filterable: bool,
    editor: Option<TableCellEditor>,
    width: UiPx,
    min_width: UiPx,
    max_width: UiPx,
    resizable: bool,
}

impl TableColumn {
    /// Creates a visible, sortable, and filterable column descriptor.
    pub fn new(id: impl Into<TableColumnId>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            visible: true,
            sortable: true,
            filterable: true,
            editor: None,
            width: TABLE_DEFAULT_COLUMN_WIDTH,
            min_width: TABLE_MIN_COLUMN_WIDTH,
            max_width: TABLE_MAX_COLUMN_WIDTH,
            resizable: true,
        }
    }

    /// Returns the stable column identity.
    pub const fn id(&self) -> &TableColumnId {
        &self.id
    }

    /// Returns the human-readable column label.
    pub fn label(&self) -> &str {
        &self.label
    }

    /// Returns whether this column should render by default.
    pub const fn visible(&self) -> bool {
        self.visible
    }

    /// Returns whether this column accepts sorting.
    pub const fn sortable(&self) -> bool {
        self.sortable
    }

    /// Returns whether this column accepts filtering.
    pub const fn filterable(&self) -> bool {
        self.filterable
    }

    /// Returns the cell editor configured for this column, if any.
    pub const fn editor(&self) -> Option<TableCellEditor> {
        self.editor
    }

    /// Returns whether this column renders text-cell editors for editable leaf cells.
    pub const fn text_editable(&self) -> bool {
        matches!(self.editor, Some(TableCellEditor::Text))
    }

    /// Returns the preferred width before committed sizing is applied.
    pub const fn width(&self) -> UiPx {
        self.width
    }

    /// Returns the lower bound used when resolving this column's width.
    pub const fn min_width(&self) -> UiPx {
        self.min_width
    }

    /// Returns the upper bound used when resolving this column's width.
    pub const fn max_width(&self) -> UiPx {
        self.max_width
    }

    /// Returns whether the column can be resized.
    pub const fn resizable(&self) -> bool {
        self.resizable
    }

    /// Applies column visibility.
    pub const fn with_visible(mut self, visible: bool) -> Self {
        self.visible = visible;
        self
    }

    /// Applies sorting capability.
    pub const fn with_sortable(mut self, sortable: bool) -> Self {
        self.sortable = sortable;
        self
    }

    /// Applies filtering capability.
    pub const fn with_filterable(mut self, filterable: bool) -> Self {
        self.filterable = filterable;
        self
    }

    /// Applies cell editor metadata.
    pub const fn with_editor(mut self, editor: Option<TableCellEditor>) -> Self {
        self.editor = editor;
        self
    }

    /// Enables or disables single-line text editing for leaf cells in this column.
    pub const fn with_text_editable(mut self, editable: bool) -> Self {
        self.editor = if editable {
            Some(TableCellEditor::Text)
        } else {
            None
        };
        self
    }

    /// Applies the preferred width.
    pub fn with_width(mut self, width: UiPx) -> Self {
        self.width = normalized_column_width(width);
        self
    }

    /// Applies the minimum width.
    pub fn with_min_width(mut self, min_width: UiPx) -> Self {
        self.min_width = normalized_column_width(min_width);
        if self.max_width < self.min_width {
            self.max_width = self.min_width;
        }
        self
    }

    /// Applies the maximum width.
    pub fn with_max_width(mut self, max_width: UiPx) -> Self {
        self.max_width = normalized_column_width(max_width).max(self.min_width);
        self
    }

    /// Applies resize enablement.
    pub const fn with_resizable(mut self, resizable: bool) -> Self {
        self.resizable = resizable;
        self
    }

    /// Resolves this column's width against committed sizing state.
    pub fn resolved_width(&self, sizing: &TableColumnSizing) -> UiPx {
        let width = sizing.width(&self.id).unwrap_or(self.width);
        clamp_column_width(width, self.min_width, self.max_width)
    }
}

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

fn normalized_column_width(width: UiPx) -> UiPx {
    let raw = width.as_f32();
    if raw.is_finite() {
        ui_px(raw.max(0.0))
    } else {
        UiPx::ZERO
    }
}

fn clamp_column_width(width: UiPx, min_width: UiPx, max_width: UiPx) -> UiPx {
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

/// Resolved table column lane for pinning-aware renderers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum TableColumnRegion {
    /// Columns pinned to the left side.
    Left,
    /// Unpinned center columns.
    Center,
    /// Columns pinned to the right side.
    Right,
}

impl TableColumnRegion {
    /// All column regions in render order.
    pub const ALL: [Self; 3] = [Self::Left, Self::Center, Self::Right];

    /// Returns a stable label.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Left => "left",
            Self::Center => "center",
            Self::Right => "right",
        }
    }
}

/// Caller-owned pinned column state.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TableColumnPinning {
    left: Vec<TableColumnId>,
    right: Vec<TableColumnId>,
}

impl TableColumnPinning {
    /// Creates an empty pinning state.
    pub fn new() -> Self {
        Self::default()
    }

    /// Applies left-pinned column ids.
    pub fn pinned_left(
        mut self,
        columns: impl IntoIterator<Item = impl Into<TableColumnId>>,
    ) -> Self {
        self.left = unique_column_ids(columns);
        let left = self.left.iter().cloned().collect::<BTreeSet<_>>();
        self.right.retain(|column| !left.contains(column));
        self
    }

    /// Applies right-pinned column ids.
    pub fn pinned_right(
        mut self,
        columns: impl IntoIterator<Item = impl Into<TableColumnId>>,
    ) -> Self {
        self.right = unique_column_ids(columns);
        let right = self.right.iter().cloned().collect::<BTreeSet<_>>();
        self.left.retain(|column| !right.contains(column));
        self
    }

    /// Returns left-pinned column ids.
    pub fn left(&self) -> &[TableColumnId] {
        &self.left
    }

    /// Returns right-pinned column ids.
    pub fn right(&self) -> &[TableColumnId] {
        &self.right
    }

    /// Returns true when no columns are pinned.
    pub fn is_empty(&self) -> bool {
        self.left.is_empty() && self.right.is_empty()
    }
}

/// Resolved visible columns split into render regions.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct TableColumnRegions {
    left: Vec<TableColumn>,
    center: Vec<TableColumn>,
    right: Vec<TableColumn>,
}

impl TableColumnRegions {
    fn from_visible_columns(
        visible_columns: impl IntoIterator<Item = TableColumn>,
        pinning: &TableColumnPinning,
    ) -> Self {
        let left = pinning.left().iter().cloned().collect::<BTreeSet<_>>();
        let right = pinning.right().iter().cloned().collect::<BTreeSet<_>>();
        let mut regions = Self::default();

        for column in visible_columns {
            if left.contains(column.id()) {
                regions.left.push(column);
            } else if right.contains(column.id()) {
                regions.right.push(column);
            } else {
                regions.center.push(column);
            }
        }

        regions
    }

    /// Returns visible left-pinned columns.
    pub fn left(&self) -> &[TableColumn] {
        &self.left
    }

    /// Returns visible unpinned center columns.
    pub fn center(&self) -> &[TableColumn] {
        &self.center
    }

    /// Returns visible right-pinned columns.
    pub fn right(&self) -> &[TableColumn] {
        &self.right
    }

    /// Returns visible columns for a region.
    pub fn region(&self, region: TableColumnRegion) -> &[TableColumn] {
        match region {
            TableColumnRegion::Left => self.left(),
            TableColumnRegion::Center => self.center(),
            TableColumnRegion::Right => self.right(),
        }
    }

    /// Returns the total number of visible columns across all regions.
    pub fn len(&self) -> usize {
        self.left.len() + self.center.len() + self.right.len()
    }

    /// Returns true when all regions are empty.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    fn flattened(&self) -> Vec<TableColumn> {
        self.left
            .iter()
            .chain(self.center.iter())
            .chain(self.right.iter())
            .cloned()
            .collect()
    }
}

/// Resolved table row lane for row-pinning-aware renderers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum TableRowRegion {
    /// Rows pinned to the top body band.
    Top,
    /// Unpinned center rows.
    Center,
    /// Rows pinned to the bottom body band.
    Bottom,
}

impl TableRowRegion {
    /// All row regions in render order.
    pub const ALL: [Self; 3] = [Self::Top, Self::Center, Self::Bottom];

    /// Returns a stable label.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Top => "top",
            Self::Center => "center",
            Self::Bottom => "bottom",
        }
    }
}

/// Policy for resolving pinned rows that are outside the current page.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TableRowPinningPolicy {
    /// Pinned rows may resolve from the expanded pre-pagination model.
    KeepPinnedRows,
    /// Pinned rows resolve only when they are present in the current page.
    PageOnly,
}

impl Default for TableRowPinningPolicy {
    fn default() -> Self {
        Self::KeepPinnedRows
    }
}

/// Caller-owned pinned row state.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TableRowPinning {
    top: Vec<TableRowId>,
    bottom: Vec<TableRowId>,
}

impl TableRowPinning {
    /// Creates an empty row pinning state.
    pub fn new() -> Self {
        Self::default()
    }

    /// Applies top-pinned row ids.
    pub fn pinned_top(mut self, rows: impl IntoIterator<Item = impl Into<TableRowId>>) -> Self {
        self.top = unique_row_ids(rows);
        let top = self.top.iter().cloned().collect::<BTreeSet<_>>();
        self.bottom.retain(|row| !top.contains(row));
        self
    }

    /// Applies bottom-pinned row ids.
    pub fn pinned_bottom(mut self, rows: impl IntoIterator<Item = impl Into<TableRowId>>) -> Self {
        self.bottom = unique_row_ids(rows);
        let bottom = self.bottom.iter().cloned().collect::<BTreeSet<_>>();
        self.top.retain(|row| !bottom.contains(row));
        self
    }

    /// Returns top-pinned row ids.
    pub fn top(&self) -> &[TableRowId] {
        &self.top
    }

    /// Returns bottom-pinned row ids.
    pub fn bottom(&self) -> &[TableRowId] {
        &self.bottom
    }

    /// Returns true when no rows are pinned.
    pub fn is_empty(&self) -> bool {
        self.top.is_empty() && self.bottom.is_empty()
    }
}

/// Resolved visible rows split into row-pinning regions.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct TableRowRegions {
    top: Vec<TableResolvedRow>,
    center: Vec<TableResolvedRow>,
    bottom: Vec<TableResolvedRow>,
}

impl TableRowRegions {
    fn from_models(
        expanded_rows: &[TableResolvedRow],
        paginated_rows: &[TableResolvedRow],
        pinning: &TableRowPinning,
        policy: TableRowPinningPolicy,
    ) -> Self {
        if pinning.is_empty() {
            return Self {
                top: Vec::new(),
                center: paginated_rows.to_vec(),
                bottom: Vec::new(),
            };
        }

        let lookup_rows = match policy {
            TableRowPinningPolicy::KeepPinnedRows => expanded_rows,
            TableRowPinningPolicy::PageOnly => paginated_rows,
        };
        let mut top_seen = BTreeSet::new();
        let top_ids = pinning
            .top()
            .iter()
            .filter(|row_id| top_seen.insert((*row_id).clone()))
            .cloned()
            .collect::<BTreeSet<_>>();
        let top = lookup_rows
            .iter()
            .filter(|row| top_ids.contains(row.id()))
            .cloned()
            .collect::<Vec<_>>();
        let top_ids = top
            .iter()
            .map(|row| row.id().clone())
            .collect::<BTreeSet<_>>();

        let mut bottom_seen = BTreeSet::new();
        let bottom_ids = pinning
            .bottom()
            .iter()
            .filter(|row_id| !top_ids.contains(*row_id))
            .filter(|row_id| bottom_seen.insert((*row_id).clone()))
            .cloned()
            .collect::<BTreeSet<_>>();
        let bottom = lookup_rows
            .iter()
            .filter(|row| bottom_ids.contains(row.id()))
            .cloned()
            .collect::<Vec<_>>();
        let pinned_ids = top
            .iter()
            .chain(bottom.iter())
            .map(|row| row.id().clone())
            .collect::<BTreeSet<_>>();
        let center = paginated_rows
            .iter()
            .filter(|row| !pinned_ids.contains(row.id()))
            .cloned()
            .collect();

        Self {
            top,
            center,
            bottom,
        }
    }

    /// Returns top-pinned rows.
    pub fn top(&self) -> &[TableResolvedRow] {
        &self.top
    }

    /// Returns unpinned center rows.
    pub fn center(&self) -> &[TableResolvedRow] {
        &self.center
    }

    /// Returns bottom-pinned rows.
    pub fn bottom(&self) -> &[TableResolvedRow] {
        &self.bottom
    }

    /// Returns rows for a region.
    pub fn region(&self, region: TableRowRegion) -> &[TableResolvedRow] {
        match region {
            TableRowRegion::Top => self.top(),
            TableRowRegion::Center => self.center(),
            TableRowRegion::Bottom => self.bottom(),
        }
    }

    /// Returns the total number of visual body rows across all regions.
    pub fn len(&self) -> usize {
        self.top.len() + self.center.len() + self.bottom.len()
    }

    /// Returns true when all row regions are empty.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    fn flattened(&self) -> Vec<TableResolvedRow> {
        self.top
            .iter()
            .chain(self.center.iter())
            .chain(self.bottom.iter())
            .cloned()
            .collect()
    }
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
    fn from_column_regions(regions: &TableColumnRegions, sizing: &TableColumnSizing) -> Self {
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

/// Loading state for source row children.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TableRowChildrenLoadState {
    /// No child load is currently pending or failed.
    Idle,
    /// Child rows are being loaded by the caller.
    Loading {
        /// Loading status text supplied by the caller.
        message: String,
    },
    /// Child row loading failed.
    Failed {
        /// Failure status text supplied by the caller.
        message: String,
    },
}

impl TableRowChildrenLoadState {
    /// Creates idle child loading metadata.
    pub const fn idle() -> Self {
        Self::Idle
    }

    /// Creates loading child metadata.
    pub fn loading(message: impl Into<String>) -> Self {
        Self::Loading {
            message: message.into(),
        }
    }

    /// Creates failed child loading metadata.
    pub fn failed(message: impl Into<String>) -> Self {
        Self::Failed {
            message: message.into(),
        }
    }

    /// Returns whether child rows are currently loading.
    pub const fn is_loading(&self) -> bool {
        matches!(self, Self::Loading { .. })
    }

    /// Returns whether child row loading failed.
    pub const fn is_failed(&self) -> bool {
        matches!(self, Self::Failed { .. })
    }

    /// Returns a stable loading-state label.
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Loading { .. } => "loading",
            Self::Failed { .. } => "failed",
        }
    }

    /// Returns the loading or failure message, when present.
    pub fn message(&self) -> Option<&str> {
        match self {
            Self::Idle => None,
            Self::Loading { message } | Self::Failed { message } => Some(message.as_str()),
        }
    }
}

impl Default for TableRowChildrenLoadState {
    fn default() -> Self {
        Self::Idle
    }
}

/// Renderer-neutral row descriptor.
#[derive(Debug, Clone, PartialEq)]
pub struct TableRow {
    id: TableRowId,
    cells: BTreeMap<TableColumnId, TableCellValue>,
    children: Vec<TableRow>,
    expandable: bool,
    children_load_state: TableRowChildrenLoadState,
}

impl TableRow {
    /// Creates a row with a stable identity.
    pub fn new(id: impl Into<TableRowId>) -> Self {
        Self {
            id: id.into(),
            cells: BTreeMap::new(),
            children: Vec::new(),
            expandable: false,
            children_load_state: TableRowChildrenLoadState::Idle,
        }
    }

    /// Returns the stable row identity.
    pub const fn id(&self) -> &TableRowId {
        &self.id
    }

    /// Returns all cells keyed by column identity.
    pub const fn cells(&self) -> &BTreeMap<TableColumnId, TableCellValue> {
        &self.cells
    }

    /// Returns nested source rows.
    pub fn children(&self) -> &[TableRow] {
        &self.children
    }

    /// Returns whether this source row has nested children.
    pub fn has_children(&self) -> bool {
        !self.children.is_empty()
    }

    /// Returns whether this source row can be expanded by the caller.
    pub fn can_expand(&self) -> bool {
        self.expandable
            || self.has_children()
            || !matches!(self.children_load_state, TableRowChildrenLoadState::Idle)
    }

    /// Returns caller-owned child loading metadata.
    pub const fn children_load_state(&self) -> &TableRowChildrenLoadState {
        &self.children_load_state
    }

    /// Returns a cell value for the given column.
    pub fn cell(&self, column: &TableColumnId) -> Option<&TableCellValue> {
        self.cells.get(column)
    }

    /// Adds or replaces a cell value.
    pub fn with_cell(
        mut self,
        column: impl Into<TableColumnId>,
        value: impl Into<TableCellValue>,
    ) -> Self {
        self.cells.insert(column.into(), value.into());
        self
    }

    /// Adds one nested source row.
    pub fn with_child(mut self, child: TableRow) -> Self {
        self.children.push(child);
        self
    }

    /// Adds nested source rows.
    pub fn with_children(mut self, children: impl IntoIterator<Item = TableRow>) -> Self {
        self.children.extend(children);
        self
    }

    /// Replaces nested source rows.
    pub fn with_replaced_children(mut self, children: impl IntoIterator<Item = TableRow>) -> Self {
        self.children = children.into_iter().collect();
        self
    }

    /// Marks the row as expandable even when no child rows are currently loaded.
    pub const fn with_expandable(mut self, expandable: bool) -> Self {
        self.expandable = expandable;
        self
    }

    /// Applies caller-owned child loading metadata.
    pub fn with_children_load_state(mut self, state: TableRowChildrenLoadState) -> Self {
        if !matches!(state, TableRowChildrenLoadState::Idle) {
            self.expandable = true;
        }
        self.children_load_state = state;
        self
    }

    /// Marks child rows as currently loading.
    pub fn with_children_loading(self, message: impl Into<String>) -> Self {
        self.with_children_load_state(TableRowChildrenLoadState::loading(message))
    }

    /// Marks child row loading as failed.
    pub fn with_children_load_failed(self, message: impl Into<String>) -> Self {
        self.with_children_load_state(TableRowChildrenLoadState::failed(message))
    }
}

/// Sort direction for a table column.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TableSortDirection {
    /// Sort from low to high.
    Ascending,
    /// Sort from high to low.
    Descending,
}

impl TableSortDirection {
    /// Returns a stable label.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Ascending => "ascending",
            Self::Descending => "descending",
        }
    }
}

/// Sort specification for one column.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableSort {
    column: TableColumnId,
    direction: TableSortDirection,
}

impl TableSort {
    /// Creates a sort specification.
    pub fn new(column: impl Into<TableColumnId>, direction: TableSortDirection) -> Self {
        Self {
            column: column.into(),
            direction,
        }
    }

    /// Creates an ascending sort specification.
    pub fn ascending(column: impl Into<TableColumnId>) -> Self {
        Self::new(column, TableSortDirection::Ascending)
    }

    /// Creates a descending sort specification.
    pub fn descending(column: impl Into<TableColumnId>) -> Self {
        Self::new(column, TableSortDirection::Descending)
    }

    /// Returns the sorted column identity.
    pub const fn column(&self) -> &TableColumnId {
        &self.column
    }

    /// Returns the sort direction.
    pub const fn direction(&self) -> TableSortDirection {
        self.direction
    }
}

/// Column filter kind.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TableFilterKind {
    /// Case-insensitive contains filter.
    Contains {
        /// Case-insensitive query text.
        query: String,
    },
    /// Exact categorical filter over stable facet tokens.
    OneOf {
        /// Exact stable facet tokens.
        values: BTreeSet<String>,
    },
}

/// Column filter specification for one column.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableFilter {
    column: TableColumnId,
    kind: TableFilterKind,
}

impl TableFilter {
    /// Creates a case-insensitive contains filter.
    pub fn contains(column: impl Into<TableColumnId>, query: impl Into<String>) -> Self {
        Self {
            column: column.into(),
            kind: TableFilterKind::Contains {
                query: query.into(),
            },
        }
    }

    /// Creates an exact categorical filter over stable facet tokens.
    pub fn exact(column: impl Into<TableColumnId>, value: impl Into<String>) -> Self {
        Self::one_of(column, [value.into()])
    }

    /// Creates an exact categorical filter over multiple stable facet tokens.
    pub fn one_of(
        column: impl Into<TableColumnId>,
        values: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        Self {
            column: column.into(),
            kind: TableFilterKind::OneOf {
                values: values.into_iter().map(Into::into).collect(),
            },
        }
    }

    /// Returns the filtered column identity.
    pub const fn column(&self) -> &TableColumnId {
        &self.column
    }

    /// Returns the filter kind.
    pub const fn kind(&self) -> &TableFilterKind {
        &self.kind
    }

    /// Returns the contains query when this is a contains filter.
    pub fn query(&self) -> &str {
        match &self.kind {
            TableFilterKind::Contains { query } => query,
            TableFilterKind::OneOf { .. } => "",
        }
    }

    /// Returns the selected categorical tokens when this is an exact filter.
    pub fn selected_values(&self) -> Option<&BTreeSet<String>> {
        match &self.kind {
            TableFilterKind::Contains { .. } => None,
            TableFilterKind::OneOf { values } => Some(values),
        }
    }

    fn matches(&self, row: &TableRow) -> bool {
        match &self.kind {
            TableFilterKind::Contains { query } => {
                if query.is_empty() {
                    return true;
                }

                row.cell(&self.column)
                    .map(|value| {
                        value
                            .filter_text()
                            .to_lowercase()
                            .contains(&query.to_lowercase())
                    })
                    .unwrap_or(false)
            }
            TableFilterKind::OneOf { values } => {
                if values.is_empty() {
                    return true;
                }

                row.cell(&self.column)
                    .map(|value| values.contains(&value.filter_text()))
                    .unwrap_or(false)
            }
        }
    }
}

/// Per-stage row-model ownership for client or manual control.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TableStageMode {
    /// The table applies the stage locally.
    Client,
    /// The caller supplies the stage output snapshot.
    Manual,
}

impl TableStageMode {
    /// Returns whether the stage is caller-owned.
    pub const fn is_manual(self) -> bool {
        matches!(self, Self::Manual)
    }
}

impl Default for TableStageMode {
    fn default() -> Self {
        Self::Client
    }
}

/// Row-selection cardinality for a table.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TableSelectionMode {
    /// Multiple rows may be selected at once.
    #[default]
    Multiple,
    /// Exactly one row should be selected at a time.
    Single,
}

impl TableSelectionMode {
    /// Returns whether the table is single-select.
    pub const fn is_single(self) -> bool {
        matches!(self, Self::Single)
    }

    /// Returns whether the table permits multiple selected rows.
    pub const fn is_multiple(self) -> bool {
        matches!(self, Self::Multiple)
    }
}

/// How row selection is triggered from the table surface.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TableSelectionActivationMode {
    /// Selection happens through explicit controls such as checkboxes or radios.
    #[default]
    ExplicitControl,
    /// Clicking the row surface toggles selection.
    RowClick,
}

impl TableSelectionActivationMode {
    /// Returns whether row clicks toggle selection.
    pub const fn is_row_click(self) -> bool {
        matches!(self, Self::RowClick)
    }
}

/// Whether selecting a row propagates to its descendants.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TableSubRowSelectionPolicy {
    /// Child rows stay independent unless they are selected directly.
    #[default]
    Independent,
    /// Selecting a row also selects all of its descendants.
    Descendants,
}

impl TableSubRowSelectionPolicy {
    /// Returns whether descendant rows are selected together with their parent.
    pub const fn propagates_descendants(self) -> bool {
        matches!(self, Self::Descendants)
    }
}

/// Policy for resolving table row selection behavior.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TableSelectionPolicy {
    selection_mode: TableSelectionMode,
    activation_mode: TableSelectionActivationMode,
    sub_row_policy: TableSubRowSelectionPolicy,
}

impl TableSelectionPolicy {
    /// Creates a selection policy from explicit mode choices.
    pub const fn new(
        selection_mode: TableSelectionMode,
        activation_mode: TableSelectionActivationMode,
        sub_row_policy: TableSubRowSelectionPolicy,
    ) -> Self {
        Self {
            selection_mode,
            activation_mode,
            sub_row_policy,
        }
    }

    /// Returns the selection cardinality.
    pub const fn selection_mode(self) -> TableSelectionMode {
        self.selection_mode
    }

    /// Returns how selection is triggered from the row surface.
    pub const fn activation_mode(self) -> TableSelectionActivationMode {
        self.activation_mode
    }

    /// Returns how selection propagates to descendant rows.
    pub const fn sub_row_policy(self) -> TableSubRowSelectionPolicy {
        self.sub_row_policy
    }

    /// Applies a selection cardinality.
    pub const fn with_selection_mode(mut self, selection_mode: TableSelectionMode) -> Self {
        self.selection_mode = selection_mode;
        self
    }

    /// Applies a row-surface activation mode.
    pub const fn with_activation_mode(
        mut self,
        activation_mode: TableSelectionActivationMode,
    ) -> Self {
        self.activation_mode = activation_mode;
        self
    }

    /// Applies a descendant-selection policy.
    pub const fn with_sub_row_policy(mut self, sub_row_policy: TableSubRowSelectionPolicy) -> Self {
        self.sub_row_policy = sub_row_policy;
        self
    }

    fn resolve_selected_rows(
        self,
        rows: &[TableRow],
        selected_rows: &BTreeSet<TableRowId>,
    ) -> BTreeSet<TableRowId> {
        let mut resolved = self.normalize_selected_rows(selected_rows.iter().cloned());
        if self.selection_mode.is_single() {
            return resolved;
        }
        if self.sub_row_policy.propagates_descendants() {
            collect_descendant_selected_rows(rows, selected_rows, &mut resolved);
        }

        resolved
    }

    fn normalize_selected_rows(
        self,
        selected_rows: impl IntoIterator<Item = impl Into<TableRowId>>,
    ) -> BTreeSet<TableRowId> {
        let selected_rows = selected_rows
            .into_iter()
            .map(Into::into)
            .collect::<BTreeSet<_>>();
        if self.selection_mode.is_single() {
            return selected_rows.into_iter().next().into_iter().collect();
        }

        selected_rows
    }
}

impl Default for TableSelectionPolicy {
    fn default() -> Self {
        Self::new(
            TableSelectionMode::Multiple,
            TableSelectionActivationMode::ExplicitControl,
            TableSubRowSelectionPolicy::Independent,
        )
    }
}

/// The resolved state for one table-selection summary scope.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TableSelectionSummaryState {
    /// No rows in the scope are selected.
    None,
    /// Some but not all rows in the scope are selected.
    Some,
    /// Every row in the scope is selected.
    All,
}

impl TableSelectionSummaryState {
    /// Returns a stable label for the summary state.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Some => "some",
            Self::All => "all",
        }
    }

    /// Returns whether the scope has no selected rows.
    pub const fn is_none(self) -> bool {
        matches!(self, Self::None)
    }

    /// Returns whether the scope has some but not all selected rows.
    pub const fn is_some(self) -> bool {
        matches!(self, Self::Some)
    }

    /// Returns whether the scope has every row selected.
    pub const fn is_all(self) -> bool {
        matches!(self, Self::All)
    }
}

/// Summary of selection across one resolved row-model scope.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TableSelectionSummary {
    selected_count: usize,
    total_count: usize,
}

impl TableSelectionSummary {
    /// Creates a selection summary from explicit counts.
    pub const fn new(selected_count: usize, total_count: usize) -> Self {
        Self {
            selected_count,
            total_count,
        }
    }

    /// Returns the number of selected rows in the scope.
    pub const fn selected_count(self) -> usize {
        self.selected_count
    }

    /// Returns the total number of rows in the scope.
    pub const fn total_count(self) -> usize {
        self.total_count
    }

    /// Returns the resolved state for the summary.
    pub const fn state(self) -> TableSelectionSummaryState {
        if self.total_count == 0 || self.selected_count == 0 {
            TableSelectionSummaryState::None
        } else if self.selected_count == self.total_count {
            TableSelectionSummaryState::All
        } else {
            TableSelectionSummaryState::Some
        }
    }

    /// Returns whether the scope has no selected rows.
    pub const fn is_none_selected(self) -> bool {
        self.state().is_none()
    }

    /// Returns whether the scope has some but not all selected rows.
    pub const fn is_some_selected(self) -> bool {
        self.state().is_some()
    }

    /// Returns whether the scope has every row selected.
    pub const fn is_all_selected(self) -> bool {
        self.state().is_all()
    }
}

/// Pagination state for a table row model.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TablePagination {
    page_index: usize,
    page_size: usize,
    mode: TableStageMode,
    row_count: Option<usize>,
    page_count: Option<usize>,
}

impl TablePagination {
    /// Creates pagination state from a page index and page size.
    pub const fn new(page_index: usize, page_size: usize) -> Self {
        Self {
            page_index,
            page_size,
            mode: TableStageMode::Client,
            row_count: None,
            page_count: None,
        }
    }

    /// Creates manual pagination state from a page index, page size, and total row count.
    pub const fn manual(page_index: usize, page_size: usize, row_count: usize) -> Self {
        Self::new(page_index, page_size)
            .with_mode(TableStageMode::Manual)
            .with_row_count(row_count)
    }

    /// Returns pagination that keeps all rows.
    pub const fn disabled() -> Self {
        Self {
            page_index: 0,
            page_size: usize::MAX,
            mode: TableStageMode::Client,
            row_count: None,
            page_count: None,
        }
    }

    /// Returns the zero-based page index.
    pub const fn page_index(self) -> usize {
        self.page_index
    }

    /// Returns the same pagination state with the page index reset.
    pub const fn with_page_index(mut self, page_index: usize) -> Self {
        self.page_index = page_index;
        self
    }

    /// Returns the maximum number of rows per page.
    pub const fn page_size(self) -> usize {
        self.page_size
    }

    /// Returns the pagination ownership mode.
    pub const fn mode(self) -> TableStageMode {
        self.mode
    }

    /// Returns whether pagination is caller-owned.
    pub const fn is_manual(self) -> bool {
        self.mode.is_manual()
    }

    /// Returns the total row count when known.
    pub const fn row_count(self) -> Option<usize> {
        self.row_count
    }

    /// Returns the total page count when known or derivable.
    pub fn page_count(self) -> Option<usize> {
        if let Some(page_count) = self.page_count {
            return Some(page_count);
        }

        let row_count = self.row_count?;
        if self.page_size == 0 {
            return Some(0);
        }

        Some(row_count.div_ceil(self.page_size))
    }

    /// Applies pagination ownership mode.
    pub const fn with_mode(mut self, mode: TableStageMode) -> Self {
        self.mode = mode;
        self
    }

    /// Applies a total row count.
    pub const fn with_row_count(mut self, row_count: usize) -> Self {
        self.row_count = Some(row_count);
        self
    }

    /// Applies a total page count.
    pub const fn with_page_count(mut self, page_count: usize) -> Self {
        self.page_count = Some(page_count);
        self
    }

    fn apply(self, rows: &[TableResolvedRow]) -> Vec<TableResolvedRow> {
        if self.is_manual() || self.page_size == usize::MAX {
            return rows.to_vec();
        }
        if self.page_size == 0 {
            return Vec::new();
        }

        let start = self.page_index.saturating_mul(self.page_size);
        rows.iter()
            .skip(start)
            .take(self.page_size)
            .cloned()
            .collect()
    }
}

impl Default for TablePagination {
    fn default() -> Self {
        Self::disabled()
    }
}

/// Count for one faceted table value.
#[derive(Debug, Clone)]
pub struct TableFacetValueCount {
    value: TableCellValue,
    count: usize,
}

impl PartialEq for TableFacetValueCount {
    fn eq(&self, other: &Self) -> bool {
        self.count == other.count
            && TableFacetValueKey::from_value(&self.value)
                == TableFacetValueKey::from_value(&other.value)
    }
}

impl TableFacetValueCount {
    /// Creates a count entry for one faceted value.
    pub fn new(value: impl Into<TableCellValue>, count: usize) -> Self {
        Self {
            value: value.into(),
            count,
        }
    }

    /// Returns the faceted value.
    pub const fn value(&self) -> &TableCellValue {
        &self.value
    }

    /// Returns the number of rows that produced this value.
    pub const fn count(&self) -> usize {
        self.count
    }
}

/// Numeric min/max metadata for a faceted column.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TableFacetRange {
    min: f64,
    max: f64,
}

impl TableFacetRange {
    /// Creates a numeric range when both bounds are finite.
    pub fn new(left: f64, right: f64) -> Option<Self> {
        if !left.is_finite() || !right.is_finite() {
            return None;
        }

        Some(if left <= right {
            Self {
                min: left,
                max: right,
            }
        } else {
            Self {
                min: right,
                max: left,
            }
        })
    }

    /// Returns the lower numeric bound.
    pub const fn min(self) -> f64 {
        self.min
    }

    /// Returns the upper numeric bound.
    pub const fn max(self) -> f64 {
        self.max
    }
}

/// Faceting metadata for one table column.
#[derive(Debug, Clone, PartialEq)]
pub struct TableColumnFacets {
    column: TableColumnId,
    mode: TableStageMode,
    row_count: usize,
    unique_values: Vec<TableFacetValueCount>,
    numeric_range: Option<TableFacetRange>,
}

impl TableColumnFacets {
    /// Creates empty client-derived facet metadata for one column.
    pub fn new(column: impl Into<TableColumnId>) -> Self {
        Self {
            column: column.into(),
            mode: TableStageMode::Client,
            row_count: 0,
            unique_values: Vec::new(),
            numeric_range: None,
        }
    }

    /// Creates empty manual facet metadata for one column.
    pub fn manual(column: impl Into<TableColumnId>, row_count: usize) -> Self {
        Self::new(column)
            .with_mode(TableStageMode::Manual)
            .with_row_count(row_count)
    }

    fn client(
        column: TableColumnId,
        row_count: usize,
        unique_values: Vec<TableFacetValueCount>,
        numeric_range: Option<TableFacetRange>,
    ) -> Self {
        Self {
            column,
            mode: TableStageMode::Client,
            row_count,
            unique_values,
            numeric_range,
        }
    }

    /// Returns the faceted column identity.
    pub const fn column(&self) -> &TableColumnId {
        &self.column
    }

    /// Returns whether this facet summary was locally derived or caller supplied.
    pub const fn mode(&self) -> TableStageMode {
        self.mode
    }

    /// Returns the number of rows covered by this facet summary.
    pub const fn row_count(&self) -> usize {
        self.row_count
    }

    /// Returns unique values and their row counts.
    pub fn unique_values(&self) -> &[TableFacetValueCount] {
        &self.unique_values
    }

    /// Returns the numeric min/max range, when any finite numeric values exist.
    pub const fn numeric_range(&self) -> Option<TableFacetRange> {
        self.numeric_range
    }

    /// Applies the facet ownership mode.
    pub const fn with_mode(mut self, mode: TableStageMode) -> Self {
        self.mode = mode;
        self
    }

    /// Applies the row count covered by this facet summary.
    pub const fn with_row_count(mut self, row_count: usize) -> Self {
        self.row_count = row_count;
        self
    }

    /// Applies unique values and their row counts.
    pub fn with_unique_values(
        mut self,
        unique_values: impl IntoIterator<Item = TableFacetValueCount>,
    ) -> Self {
        self.unique_values = unique_values.into_iter().collect();
        self
    }

    /// Applies a numeric min/max range when both bounds are finite.
    pub fn with_numeric_range(mut self, min: f64, max: f64) -> Self {
        self.numeric_range = TableFacetRange::new(min, max);
        self
    }
}

/// Built-in aggregate calculation for grouped table rows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TableAggregateKind {
    /// Count descendant leaf rows.
    Count,
    /// Sum numeric descendant cell values.
    Sum,
    /// Minimum numeric descendant cell value.
    Min,
    /// Maximum numeric descendant cell value.
    Max,
    /// Average numeric descendant cell value.
    Average,
}

impl TableAggregateKind {
    /// Returns a stable label.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Count => "count",
            Self::Sum => "sum",
            Self::Min => "min",
            Self::Max => "max",
            Self::Average => "average",
        }
    }

    /// Resolves a stable label back to a built-in aggregate kind.
    pub fn from_str(value: &str) -> Option<Self> {
        match value {
            "count" => Some(Self::Count),
            "sum" => Some(Self::Sum),
            "min" => Some(Self::Min),
            "max" => Some(Self::Max),
            "average" => Some(Self::Average),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum TableAggregationSpec {
    BuiltIn(TableAggregateKind),
    Named(String),
}

#[derive(Clone)]
struct TableAggregationFn(
    Arc<dyn Fn(&TableColumnId, &[TableResolvedRow]) -> TableCellValue + Send + Sync>,
);

impl TableAggregationFn {
    fn new(
        aggregation_fn: impl Fn(&TableColumnId, &[TableResolvedRow]) -> TableCellValue
        + Send
        + Sync
        + 'static,
    ) -> Self {
        Self(Arc::new(aggregation_fn))
    }

    fn call(&self, column: &TableColumnId, rows: &[TableResolvedRow]) -> TableCellValue {
        (self.0)(column, rows)
    }
}

impl fmt::Debug for TableAggregationFn {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("TableAggregationFn(..)")
    }
}

impl PartialEq for TableAggregationFn {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

impl Eq for TableAggregationFn {}

/// Aggregate specification for one table column.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableAggregation {
    column: TableColumnId,
    spec: TableAggregationSpec,
}

impl TableAggregation {
    /// Creates an aggregate specification for a column.
    pub fn new(column: impl Into<TableColumnId>, kind: TableAggregateKind) -> Self {
        Self {
            column: column.into(),
            spec: TableAggregationSpec::BuiltIn(kind),
        }
    }

    /// Creates a named aggregate specification for a column.
    pub fn named(column: impl Into<TableColumnId>, name: impl Into<String>) -> Self {
        Self {
            column: column.into(),
            spec: TableAggregationSpec::Named(name.into()),
        }
    }

    /// Creates a descendant leaf-count aggregate.
    pub fn count(column: impl Into<TableColumnId>) -> Self {
        Self::new(column, TableAggregateKind::Count)
    }

    /// Creates a numeric sum aggregate.
    pub fn sum(column: impl Into<TableColumnId>) -> Self {
        Self::new(column, TableAggregateKind::Sum)
    }

    /// Creates a numeric minimum aggregate.
    pub fn min(column: impl Into<TableColumnId>) -> Self {
        Self::new(column, TableAggregateKind::Min)
    }

    /// Creates a numeric maximum aggregate.
    pub fn max(column: impl Into<TableColumnId>) -> Self {
        Self::new(column, TableAggregateKind::Max)
    }

    /// Creates a numeric average aggregate.
    pub fn average(column: impl Into<TableColumnId>) -> Self {
        Self::new(column, TableAggregateKind::Average)
    }

    /// Returns the aggregate column identity.
    pub const fn column(&self) -> &TableColumnId {
        &self.column
    }

    /// Returns the aggregate kind when this is a built-in aggregate.
    pub fn kind(&self) -> Option<TableAggregateKind> {
        match self.spec {
            TableAggregationSpec::BuiltIn(kind) => Some(kind),
            TableAggregationSpec::Named(_) => None,
        }
    }

    /// Returns the named aggregate callback key, when present.
    pub fn name(&self) -> Option<&str> {
        match &self.spec {
            TableAggregationSpec::BuiltIn(_) => None,
            TableAggregationSpec::Named(name) => Some(name.as_str()),
        }
    }
}

/// Caller-owned expansion state for grouped table rows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TableExpansionState {
    /// Every group row is expanded.
    All,
    /// Only the listed stable row ids are expanded.
    Rows(BTreeSet<TableRowId>),
}

impl TableExpansionState {
    /// Returns an expansion state where every row is expanded.
    pub const fn all() -> Self {
        Self::All
    }

    /// Returns an expansion state for explicit row ids.
    pub fn rows(rows: impl IntoIterator<Item = impl Into<TableRowId>>) -> Self {
        Self::Rows(rows.into_iter().map(Into::into).collect())
    }

    /// Returns whether the given row id should be expanded.
    pub fn is_expanded(&self, row_id: &TableRowId) -> bool {
        match self {
            Self::All => true,
            Self::Rows(rows) => rows.contains(row_id),
        }
    }
}

impl Default for TableExpansionState {
    fn default() -> Self {
        Self::Rows(BTreeSet::new())
    }
}

/// Row expansion behavior for resolved table row models.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TableExpansionMode {
    /// The core row model hides descendants of collapsed rows.
    Client,
    /// The caller supplies the visible source-tree snapshot.
    Manual,
}

impl TableExpansionMode {
    /// Returns whether local row-model expansion pruning is enabled.
    pub const fn prunes_collapsed_rows(self) -> bool {
        matches!(self, Self::Client)
    }
}

impl Default for TableExpansionMode {
    fn default() -> Self {
        Self::Client
    }
}

/// Row-model stage vocabulary for Open GPUI tables.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TableRowModelStage {
    /// Materialized one-to-one data rows.
    Core,
    /// Filtered rows.
    Filtered,
    /// Grouped rows.
    Grouped,
    /// Sorted rows.
    Sorted,
    /// Expanded rows.
    Expanded,
    /// Paginated rows.
    Paginated,
    /// Final row model consumed by renderers.
    Final,
}

impl TableRowModelStage {
    /// Returns a stable label.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Core => "core",
            Self::Filtered => "filtered",
            Self::Grouped => "grouped",
            Self::Sorted => "sorted",
            Self::Expanded => "expanded",
            Self::Paginated => "paginated",
            Self::Final => "final",
        }
    }

    /// Returns whether this stage belonged to the original v0 resolver subset.
    pub const fn implemented_in_v0(self) -> bool {
        matches!(
            self,
            Self::Core | Self::Filtered | Self::Sorted | Self::Paginated | Self::Final
        )
    }
}

/// Full row-model vocabulary order.
pub const TABLE_ROW_MODEL_PIPELINE: [TableRowModelStage; 7] = [
    TableRowModelStage::Core,
    TableRowModelStage::Filtered,
    TableRowModelStage::Grouped,
    TableRowModelStage::Sorted,
    TableRowModelStage::Expanded,
    TableRowModelStage::Paginated,
    TableRowModelStage::Final,
];

/// Original v0 row-model subset.
pub const TABLE_ROW_MODEL_V0_PIPELINE: [TableRowModelStage; 5] = [
    TableRowModelStage::Core,
    TableRowModelStage::Filtered,
    TableRowModelStage::Sorted,
    TableRowModelStage::Paginated,
    TableRowModelStage::Final,
];

/// Renderer-neutral input state for table row-model resolution.
#[derive(Debug, Clone)]
pub struct TableState {
    columns: Vec<TableColumn>,
    column_order: Vec<TableColumnId>,
    column_pinning: TableColumnPinning,
    column_sizing: TableColumnSizing,
    row_pinning: TableRowPinning,
    row_pinning_policy: TableRowPinningPolicy,
    rows: Arc<[TableRow]>,
    rows_identity: u64,
    sorting: Vec<TableSort>,
    sorting_mode: TableStageMode,
    filters: Vec<TableFilter>,
    filtering_mode: TableStageMode,
    faceting_mode: TableStageMode,
    manual_facets: Vec<TableColumnFacets>,
    grouping: Vec<TableColumnId>,
    aggregations: Vec<TableAggregation>,
    aggregation_fns: BTreeMap<String, TableAggregationFn>,
    expansion: TableExpansionState,
    expansion_mode: TableExpansionMode,
    selection_policy: TableSelectionPolicy,
    selected_rows: BTreeSet<TableRowId>,
    pagination: TablePagination,
}

impl PartialEq for TableState {
    fn eq(&self, other: &Self) -> bool {
        self.columns == other.columns
            && self.column_order == other.column_order
            && self.column_pinning == other.column_pinning
            && self.column_sizing == other.column_sizing
            && self.row_pinning == other.row_pinning
            && self.row_pinning_policy == other.row_pinning_policy
            && self.rows.as_ref() == other.rows.as_ref()
            && self.sorting == other.sorting
            && self.sorting_mode == other.sorting_mode
            && self.filters == other.filters
            && self.filtering_mode == other.filtering_mode
            && self.faceting_mode == other.faceting_mode
            && self.manual_facets == other.manual_facets
            && self.grouping == other.grouping
            && self.aggregations == other.aggregations
            && self.aggregation_fns == other.aggregation_fns
            && self.expansion == other.expansion
            && self.expansion_mode == other.expansion_mode
            && self.selection_policy == other.selection_policy
            && self.selected_rows == other.selected_rows
            && self.pagination == other.pagination
    }
}

impl TableState {
    /// Creates table state from row descriptors.
    pub fn new(rows: impl IntoIterator<Item = TableRow>) -> Self {
        let rows = rows.into_iter().collect::<Vec<_>>();

        Self {
            columns: Vec::new(),
            column_order: Vec::new(),
            column_pinning: TableColumnPinning::default(),
            column_sizing: TableColumnSizing::default(),
            row_pinning: TableRowPinning::default(),
            row_pinning_policy: TableRowPinningPolicy::default(),
            rows: rows.into(),
            rows_identity: next_table_rows_identity(),
            sorting: Vec::new(),
            sorting_mode: TableStageMode::default(),
            filters: Vec::new(),
            filtering_mode: TableStageMode::default(),
            faceting_mode: TableStageMode::default(),
            manual_facets: Vec::new(),
            grouping: Vec::new(),
            aggregations: Vec::new(),
            aggregation_fns: BTreeMap::new(),
            expansion: TableExpansionState::default(),
            expansion_mode: TableExpansionMode::default(),
            selection_policy: TableSelectionPolicy::default(),
            selected_rows: BTreeSet::new(),
            pagination: TablePagination::default(),
        }
    }

    /// Applies column descriptors.
    pub fn with_columns(mut self, columns: impl IntoIterator<Item = TableColumn>) -> Self {
        self.columns = columns.into_iter().collect();
        self
    }

    /// Replaces source rows while preserving the rest of the table configuration.
    pub fn with_rows(mut self, rows: impl IntoIterator<Item = TableRow>) -> Self {
        self.rows = rows.into_iter().collect::<Vec<_>>().into();
        self.rows_identity = next_table_rows_identity();
        self
    }

    /// Applies explicit column order.
    pub fn with_column_order(
        mut self,
        column_order: impl IntoIterator<Item = impl Into<TableColumnId>>,
    ) -> Self {
        self.column_order = column_order.into_iter().map(Into::into).collect();
        self
    }

    /// Applies pinned column state.
    pub fn with_column_pinning(mut self, column_pinning: TableColumnPinning) -> Self {
        self.column_pinning = column_pinning;
        self
    }

    /// Applies pinned row state.
    pub fn with_row_pinning(mut self, row_pinning: TableRowPinning) -> Self {
        self.row_pinning = row_pinning;
        self
    }

    /// Applies the pinned row visibility policy.
    pub const fn with_row_pinning_policy(
        mut self,
        row_pinning_policy: TableRowPinningPolicy,
    ) -> Self {
        self.row_pinning_policy = row_pinning_policy;
        self
    }

    /// Applies committed column sizing state.
    pub fn with_column_sizing(mut self, column_sizing: TableColumnSizing) -> Self {
        self.column_sizing = column_sizing;
        self
    }

    /// Applies sort specifications.
    pub fn with_sorting(mut self, sorting: impl IntoIterator<Item = TableSort>) -> Self {
        self.sorting = sorting.into_iter().collect();
        self
    }

    /// Applies sorting ownership mode.
    pub const fn with_sorting_mode(mut self, sorting_mode: TableStageMode) -> Self {
        self.sorting_mode = sorting_mode;
        self
    }

    /// Marks sorting as caller-owned.
    pub const fn with_manual_sorting(mut self) -> Self {
        self.sorting_mode = TableStageMode::Manual;
        self
    }

    /// Applies filter specifications.
    pub fn with_filters(mut self, filters: impl IntoIterator<Item = TableFilter>) -> Self {
        self.filters = filters.into_iter().collect();
        self
    }

    /// Applies filtering ownership mode.
    pub const fn with_filtering_mode(mut self, filtering_mode: TableStageMode) -> Self {
        self.filtering_mode = filtering_mode;
        self
    }

    /// Marks filtering as caller-owned.
    pub const fn with_manual_filtering(mut self) -> Self {
        self.filtering_mode = TableStageMode::Manual;
        self
    }

    /// Applies faceting ownership mode.
    pub const fn with_faceting_mode(mut self, faceting_mode: TableStageMode) -> Self {
        self.faceting_mode = faceting_mode;
        self
    }

    /// Marks faceting as caller-owned.
    pub const fn with_manual_faceting(mut self) -> Self {
        self.faceting_mode = TableStageMode::Manual;
        self
    }

    /// Applies caller-owned facet payloads keyed by column id.
    pub fn with_manual_facets(
        mut self,
        facets: impl IntoIterator<Item = TableColumnFacets>,
    ) -> Self {
        let mut facets_by_column = BTreeMap::new();
        for facet in facets {
            facets_by_column.insert(
                facet.column().clone(),
                facet.with_mode(TableStageMode::Manual),
            );
        }
        self.manual_facets = facets_by_column.into_values().collect();
        self
    }

    /// Applies grouping column ids in outer-to-inner order.
    pub fn with_grouping(
        mut self,
        grouping: impl IntoIterator<Item = impl Into<TableColumnId>>,
    ) -> Self {
        let mut seen = BTreeSet::new();
        self.grouping = grouping
            .into_iter()
            .map(Into::into)
            .filter(|column| seen.insert(column.clone()))
            .collect();
        self
    }

    /// Applies aggregate specifications keyed by column id.
    pub fn with_aggregations(
        mut self,
        aggregations: impl IntoIterator<Item = TableAggregation>,
    ) -> Self {
        let mut aggregations_by_column = BTreeMap::new();
        for aggregation in aggregations {
            aggregations_by_column.insert(aggregation.column().clone(), aggregation);
        }
        self.aggregations = aggregations_by_column.into_values().collect();
        self
    }

    /// Registers a named aggregation callback.
    pub fn with_aggregation_fn(
        mut self,
        name: impl Into<String>,
        aggregation_fn: impl Fn(&TableColumnId, &[TableResolvedRow]) -> TableCellValue
        + Send
        + Sync
        + 'static,
    ) -> Self {
        self.aggregation_fns
            .insert(name.into(), TableAggregationFn::new(aggregation_fn));
        self
    }

    /// Applies explicit expanded group row ids.
    pub fn with_expanded_rows(
        mut self,
        expanded_rows: impl IntoIterator<Item = impl Into<TableRowId>>,
    ) -> Self {
        self.expansion = TableExpansionState::rows(expanded_rows);
        self
    }

    /// Applies the expansion mode where every group row is expanded.
    pub fn with_all_rows_expanded(mut self) -> Self {
        self.expansion = TableExpansionState::All;
        self
    }

    /// Applies expansion behavior for source-tree row models.
    pub const fn with_expansion_mode(mut self, expansion_mode: TableExpansionMode) -> Self {
        self.expansion_mode = expansion_mode;
        self
    }

    /// Lets callers provide the visible source-tree snapshot directly.
    pub const fn with_manual_expansion(mut self) -> Self {
        self.expansion_mode = TableExpansionMode::Manual;
        self
    }

    /// Applies the row-selection policy.
    pub fn with_selection_policy(mut self, selection_policy: TableSelectionPolicy) -> Self {
        self.selection_policy = selection_policy;
        self.selected_rows = self
            .selection_policy
            .normalize_selected_rows(self.selected_rows.iter().cloned());
        self
    }

    /// Applies the selection cardinality.
    pub fn with_selection_mode(mut self, selection_mode: TableSelectionMode) -> Self {
        self.selection_policy = self.selection_policy.with_selection_mode(selection_mode);
        self.selected_rows = self
            .selection_policy
            .normalize_selected_rows(self.selected_rows.iter().cloned());
        self
    }

    /// Applies the selection activation mode.
    pub const fn with_selection_activation_mode(
        mut self,
        activation_mode: TableSelectionActivationMode,
    ) -> Self {
        self.selection_policy = self.selection_policy.with_activation_mode(activation_mode);
        self
    }

    /// Applies the sub-row selection policy.
    pub fn with_sub_row_selection_policy(
        mut self,
        sub_row_policy: TableSubRowSelectionPolicy,
    ) -> Self {
        self.selection_policy = self.selection_policy.with_sub_row_policy(sub_row_policy);
        self.selected_rows = self
            .selection_policy
            .normalize_selected_rows(self.selected_rows.iter().cloned());
        self
    }

    /// Applies selected row ids.
    pub fn with_selected_rows(
        mut self,
        selected_rows: impl IntoIterator<Item = impl Into<TableRowId>>,
    ) -> Self {
        self.selected_rows = self.selection_policy.normalize_selected_rows(selected_rows);
        self
    }

    /// Applies pagination state.
    pub const fn with_pagination(mut self, pagination: TablePagination) -> Self {
        self.pagination = pagination;
        self
    }

    /// Returns configured columns.
    pub fn columns(&self) -> &[TableColumn] {
        &self.columns
    }

    /// Returns explicit column order ids.
    pub fn column_order(&self) -> &[TableColumnId] {
        &self.column_order
    }

    /// Returns source rows.
    pub fn rows(&self) -> &[TableRow] {
        self.rows.as_ref()
    }

    /// Returns sort specifications.
    pub fn sorting(&self) -> &[TableSort] {
        &self.sorting
    }

    /// Returns the sorting ownership mode.
    pub const fn sorting_mode(&self) -> TableStageMode {
        self.sorting_mode
    }

    /// Returns filter specifications.
    pub fn filters(&self) -> &[TableFilter] {
        &self.filters
    }

    /// Returns the filtering ownership mode.
    pub const fn filtering_mode(&self) -> TableStageMode {
        self.filtering_mode
    }

    /// Returns the faceting ownership mode.
    pub const fn faceting_mode(&self) -> TableStageMode {
        self.faceting_mode
    }

    /// Returns caller-owned facet payloads.
    pub fn manual_facets(&self) -> &[TableColumnFacets] {
        &self.manual_facets
    }

    /// Returns grouping column ids in outer-to-inner order.
    pub fn grouping(&self) -> &[TableColumnId] {
        &self.grouping
    }

    /// Returns aggregate specifications keyed by column id.
    pub fn aggregations(&self) -> &[TableAggregation] {
        &self.aggregations
    }

    /// Returns the number of named aggregation callbacks.
    pub fn aggregation_fn_count(&self) -> usize {
        self.aggregation_fns.len()
    }

    /// Returns whether a named aggregation callback has been registered.
    pub fn has_aggregation_fn(&self, name: &str) -> bool {
        self.aggregation_fns.contains_key(name)
    }

    /// Returns pinned column state.
    pub const fn column_pinning(&self) -> &TableColumnPinning {
        &self.column_pinning
    }

    /// Returns pinned row state.
    pub const fn row_pinning(&self) -> &TableRowPinning {
        &self.row_pinning
    }

    /// Returns the pinned row visibility policy.
    pub const fn row_pinning_policy(&self) -> TableRowPinningPolicy {
        self.row_pinning_policy
    }

    /// Returns committed column sizing state.
    pub const fn column_sizing(&self) -> &TableColumnSizing {
        &self.column_sizing
    }

    /// Returns caller-owned row expansion state.
    pub const fn expansion(&self) -> &TableExpansionState {
        &self.expansion
    }

    /// Returns source-tree row expansion behavior.
    pub const fn expansion_mode(&self) -> TableExpansionMode {
        self.expansion_mode
    }

    /// Returns the selection policy.
    pub const fn selection_policy(&self) -> TableSelectionPolicy {
        self.selection_policy
    }

    /// Returns selected row ids.
    pub const fn selected_rows(&self) -> &BTreeSet<TableRowId> {
        &self.selected_rows
    }

    /// Returns pagination state.
    pub const fn pagination(&self) -> TablePagination {
        self.pagination
    }

    /// Returns a cheap identity key for runtime row-model caches.
    ///
    /// The key is conservative: cloned states share the row identity, while newly
    /// constructed states get a new identity even when their row contents match.
    pub fn cache_key(&self) -> TableStateCacheKey {
        TableStateCacheKey {
            rows_identity: self.rows_identity,
            row_count: count_table_rows(&self.rows),
            columns: self.columns.clone(),
            column_order: self.column_order.clone(),
            column_pinning: self.column_pinning.clone(),
            column_sizing: self.column_sizing.clone(),
            row_pinning: self.row_pinning.clone(),
            row_pinning_policy: self.row_pinning_policy,
            sorting: self.sorting.clone(),
            sorting_mode: self.sorting_mode,
            filters: self.filters.clone(),
            filtering_mode: self.filtering_mode,
            faceting_mode: self.faceting_mode,
            manual_facets: self.manual_facets.clone(),
            grouping: self.grouping.clone(),
            aggregations: self.aggregations.clone(),
            aggregation_fns: self.aggregation_fns.clone(),
            expansion: self.expansion.clone(),
            expansion_mode: self.expansion_mode,
            selection_policy: self.selection_policy,
            selected_rows: self.selected_rows.clone(),
            pagination: self.pagination,
        }
    }

    /// Returns visible columns in resolved order.
    pub fn visible_columns(&self) -> Vec<TableColumn> {
        self.visible_column_regions().flattened()
    }

    /// Returns visible columns split into pinned regions.
    pub fn visible_column_regions(&self) -> TableColumnRegions {
        TableColumnRegions::from_visible_columns(
            self.ordered_visible_columns(),
            &self.column_pinning,
        )
    }

    fn ordered_visible_columns(&self) -> Vec<TableColumn> {
        if self.column_order.is_empty() {
            return self
                .columns
                .iter()
                .filter(|column| column.visible())
                .cloned()
                .collect();
        }

        let columns_by_id: BTreeMap<_, _> = self
            .columns
            .iter()
            .map(|column| (column.id().clone(), column.clone()))
            .collect();

        self.column_order
            .iter()
            .filter_map(|id| columns_by_id.get(id))
            .filter(|column| column.visible())
            .cloned()
            .collect()
    }

    /// Resolves row models from the input state.
    pub fn resolve(&self) -> TableResolvedState {
        let mut duplicate_row_ids = BTreeSet::new();
        let mut seen_row_ids = BTreeSet::new();
        record_source_row_ids(&self.rows, &mut seen_row_ids, &mut duplicate_row_ids);
        let include_source_children = self.grouping.is_empty();
        let selected_rows = self
            .selection_policy
            .resolve_selected_rows(&self.rows, &self.selected_rows);
        let mut source_index = 0;
        let source_nodes = build_source_row_nodes(
            &self.rows,
            &selected_rows,
            &self.expansion,
            include_source_children,
            None,
            0,
            &mut source_index,
        );
        let core_rows = flatten_nodes(&source_nodes);
        let column_facets = self.resolve_column_facets(&source_nodes);

        let core_model = TableRowModel::new(TableRowModelStage::Core, core_rows);

        let filtered_nodes = if self.filtering_mode.is_manual() {
            source_nodes.clone()
        } else {
            filter_source_row_nodes(&source_nodes, &self.filters, None)
        };
        let filtered_rows = flatten_nodes(&filtered_nodes);
        let filtered_model = TableRowModel::new(TableRowModelStage::Filtered, filtered_rows);

        let grouped_nodes = if self.grouping.is_empty() {
            filtered_nodes
        } else {
            self.group_nodes(filtered_model.rows())
        };
        let grouped_rows = flatten_nodes(&grouped_nodes);
        let grouped_model = TableRowModel::new(TableRowModelStage::Grouped, grouped_rows);

        let sorted_nodes = if self.sorting_mode.is_manual() {
            grouped_nodes.clone()
        } else {
            self.sort_nodes(grouped_nodes)
        };
        let sorted_rows = flatten_nodes(&sorted_nodes);
        let sorted_model = TableRowModel::new(TableRowModelStage::Sorted, sorted_rows);

        let expanded_rows = self.expand_nodes(&sorted_nodes);
        let expanded_model = TableRowModel::new_with_lookup(
            TableRowModelStage::Expanded,
            expanded_rows,
            sorted_model.rows().to_vec(),
        );

        let paginated_model = TableRowModel::new(
            TableRowModelStage::Paginated,
            self.pagination.apply(expanded_model.rows()),
        );
        let row_regions = TableRowRegions::from_models(
            expanded_model.rows(),
            paginated_model.rows(),
            &self.row_pinning,
            self.row_pinning_policy,
        );
        let final_model = TableRowModel::new_with_lookup(
            TableRowModelStage::Final,
            row_regions.flattened(),
            expanded_model.rows_by_id().values().cloned(),
        );

        let visible_column_regions = self.visible_column_regions();
        let visible_column_sizing = TableResolvedColumnSizingRegions::from_column_regions(
            &visible_column_regions,
            &self.column_sizing,
        );

        TableResolvedState {
            visible_columns: visible_column_regions.flattened(),
            visible_column_regions,
            visible_column_sizing,
            duplicate_row_ids: duplicate_row_ids.into_iter().collect(),
            faceting_mode: self.faceting_mode,
            column_facets,
            row_pinning_policy: self.row_pinning_policy,
            row_regions,
            core_model,
            filtered_model,
            grouped_model,
            sorted_model,
            expanded_model,
            paginated_model,
            final_model,
            selection_policy: self.selection_policy,
        }
    }

    fn compare_rows(&self, left: &TableResolvedRow, right: &TableResolvedRow) -> Ordering {
        if self.sorting.is_empty() {
            return Ordering::Equal;
        }

        for sort in &self.sorting {
            let left_value = left.cell(sort.column()).cloned().unwrap_or_default();
            let right_value = right.cell(sort.column()).cloned().unwrap_or_default();
            let ordering = left_value.cmp_for_sort(&right_value);
            let ordering = match sort.direction() {
                TableSortDirection::Ascending => ordering,
                TableSortDirection::Descending => ordering.reverse(),
            };

            if ordering != Ordering::Equal {
                return ordering;
            }
        }

        left.id().cmp(right.id())
    }

    fn group_nodes(&self, rows: &[TableResolvedRow]) -> Vec<TableRowNode> {
        if self.grouping.is_empty() {
            return rows
                .iter()
                .cloned()
                .map(TableRowNode::leaf)
                .collect::<Vec<_>>();
        }

        build_group_nodes(
            rows,
            &self.grouping,
            &self.aggregations,
            &self.aggregation_fns,
            0,
            None,
            None,
        )
    }

    fn sort_nodes(&self, mut nodes: Vec<TableRowNode>) -> Vec<TableRowNode> {
        for node in &mut nodes {
            node.children = self.sort_nodes(std::mem::take(&mut node.children));
        }

        if !self.sorting.is_empty() {
            nodes.sort_by(|left, right| self.compare_rows(&left.row, &right.row));
        }

        nodes
    }

    fn expand_nodes(&self, nodes: &[TableRowNode]) -> Vec<TableResolvedRow> {
        if self.grouping.is_empty() && !self.expansion_mode.prunes_collapsed_rows() {
            return flatten_nodes(nodes);
        }

        let mut rows = Vec::new();
        for node in nodes {
            push_expanded_rows(node, &self.expansion, &mut rows);
        }
        rows
    }

    fn resolve_column_facets(&self, source_nodes: &[TableRowNode]) -> Vec<TableColumnFacets> {
        self.columns
            .iter()
            .filter_map(|column| {
                if let Some(manual) = self
                    .manual_facets
                    .iter()
                    .find(|facet| facet.column() == column.id())
                {
                    return Some(manual.clone());
                }

                if self.faceting_mode.is_manual() {
                    return None;
                }

                Some(resolve_client_column_facets(
                    column.id().clone(),
                    source_nodes,
                    &self.filters,
                    self.filtering_mode,
                ))
            })
            .collect()
    }
}

/// Cheap invalidation key for runtime caches of resolved table row models.
#[derive(Debug, Clone, PartialEq)]
pub struct TableStateCacheKey {
    rows_identity: u64,
    row_count: usize,
    columns: Vec<TableColumn>,
    column_order: Vec<TableColumnId>,
    column_pinning: TableColumnPinning,
    column_sizing: TableColumnSizing,
    row_pinning: TableRowPinning,
    row_pinning_policy: TableRowPinningPolicy,
    sorting: Vec<TableSort>,
    sorting_mode: TableStageMode,
    filters: Vec<TableFilter>,
    filtering_mode: TableStageMode,
    faceting_mode: TableStageMode,
    manual_facets: Vec<TableColumnFacets>,
    grouping: Vec<TableColumnId>,
    aggregations: Vec<TableAggregation>,
    aggregation_fns: BTreeMap<String, TableAggregationFn>,
    expansion: TableExpansionState,
    expansion_mode: TableExpansionMode,
    selection_policy: TableSelectionPolicy,
    selected_rows: BTreeSet<TableRowId>,
    pagination: TablePagination,
}

impl TableStateCacheKey {
    /// Returns the opaque identity assigned to this state's row storage.
    pub const fn rows_identity(&self) -> u64 {
        self.rows_identity
    }

    /// Returns the number of source rows covered by this cache key.
    pub const fn row_count(&self) -> usize {
        self.row_count
    }
}

fn next_table_rows_identity() -> u64 {
    NEXT_TABLE_ROWS_IDENTITY.fetch_add(1, AtomicOrdering::Relaxed)
}

fn count_table_rows(rows: &[TableRow]) -> usize {
    rows.iter()
        .map(|row| 1 + count_table_rows(row.children()))
        .sum()
}

fn collect_descendant_selected_rows(
    rows: &[TableRow],
    selected_rows: &BTreeSet<TableRowId>,
    resolved: &mut BTreeSet<TableRowId>,
) {
    for row in rows {
        if selected_rows.contains(row.id()) {
            collect_all_descendant_rows(row.children(), resolved);
            continue;
        }

        collect_descendant_selected_rows(row.children(), selected_rows, resolved);
    }
}

fn collect_all_descendant_rows(rows: &[TableRow], resolved: &mut BTreeSet<TableRowId>) {
    for row in rows {
        resolved.insert(row.id().clone());
        collect_all_descendant_rows(row.children(), resolved);
    }
}

fn record_source_row_ids(
    rows: &[TableRow],
    seen: &mut BTreeSet<TableRowId>,
    duplicates: &mut BTreeSet<TableRowId>,
) {
    for row in rows {
        if !seen.insert(row.id().clone()) {
            duplicates.insert(row.id().clone());
        }
        record_source_row_ids(row.children(), seen, duplicates);
    }
}

fn unique_column_ids(
    columns: impl IntoIterator<Item = impl Into<TableColumnId>>,
) -> Vec<TableColumnId> {
    let mut seen = BTreeSet::new();
    columns
        .into_iter()
        .map(Into::into)
        .filter(|column| seen.insert(column.clone()))
        .collect()
}

fn unique_row_ids(rows: impl IntoIterator<Item = impl Into<TableRowId>>) -> Vec<TableRowId> {
    let mut seen = BTreeSet::new();
    rows.into_iter()
        .map(Into::into)
        .filter(|row| seen.insert(row.clone()))
        .collect()
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
enum TableFacetValueKey {
    Empty,
    Bool(bool),
    Number(u64),
    Text(String),
}

impl TableFacetValueKey {
    fn from_value(value: &TableCellValue) -> Self {
        match value {
            TableCellValue::Empty => Self::Empty,
            TableCellValue::Bool(value) => Self::Bool(*value),
            TableCellValue::Number(value) => Self::Number(table_facet_number_key(*value)),
            TableCellValue::Text(value) => Self::Text(value.clone()),
        }
    }
}

fn table_facet_number_key(value: f64) -> u64 {
    let normalized = if value == 0.0 { 0.0 } else { value };
    let bits = normalized.to_bits();
    if bits & (1 << 63) != 0 {
        !bits
    } else {
        bits | (1 << 63)
    }
}

fn resolve_client_column_facets(
    column_id: TableColumnId,
    source_nodes: &[TableRowNode],
    filters: &[TableFilter],
    filtering_mode: TableStageMode,
) -> TableColumnFacets {
    let mut unique_values = BTreeMap::<TableFacetValueKey, TableFacetValueCount>::new();
    let mut min = f64::INFINITY;
    let mut max = f64::NEG_INFINITY;
    let mut found_numeric = false;
    let mut row_count = 0;

    {
        let mut visit = |row: &TableResolvedRow| {
            row_count += 1;
            let value = row.cell(&column_id).cloned().unwrap_or_default();
            let key = TableFacetValueKey::from_value(&value);
            unique_values
                .entry(key)
                .and_modify(|entry| entry.count += 1)
                .or_insert_with(|| TableFacetValueCount::new(value.clone(), 1));

            if let TableCellValue::Number(number) = value {
                if number.is_finite() {
                    found_numeric = true;
                    if number < min {
                        min = number;
                    }
                    if number > max {
                        max = number;
                    }
                }
            }
        };

        visit_facet_rows(
            source_nodes,
            filters,
            &column_id,
            filtering_mode,
            &mut visit,
        );
    }

    let numeric_range = if found_numeric {
        TableFacetRange::new(min, max)
    } else {
        None
    };

    TableColumnFacets::client(
        column_id,
        row_count,
        unique_values.into_values().collect(),
        numeric_range,
    )
}

fn visit_facet_rows(
    nodes: &[TableRowNode],
    filters: &[TableFilter],
    excluded_column: &TableColumnId,
    filtering_mode: TableStageMode,
    visit: &mut impl FnMut(&TableResolvedRow),
) {
    for node in nodes {
        if !filtering_mode.is_manual()
            && !row_matches_facet_filters(&node.row, filters, excluded_column)
        {
            continue;
        }

        visit(&node.row);
        visit_facet_rows(
            &node.children,
            filters,
            excluded_column,
            filtering_mode,
            visit,
        );
    }
}

fn row_matches_facet_filters(
    row: &TableResolvedRow,
    filters: &[TableFilter],
    excluded_column: &TableColumnId,
) -> bool {
    filters.iter().all(|filter| {
        filter.column() == excluded_column
            || row.source().is_some_and(|source| filter.matches(source))
    })
}

/// Metadata for a grouped table row.
#[derive(Debug, Clone, PartialEq)]
pub struct TableGroupRow {
    grouping_column: TableColumnId,
    grouping_value: TableCellValue,
    depth: usize,
    parent_id: Option<TableRowId>,
    first_leaf_row_id: TableRowId,
    leaf_row_count: usize,
}

impl TableGroupRow {
    fn new(
        grouping_column: TableColumnId,
        grouping_value: TableCellValue,
        depth: usize,
        parent_id: Option<TableRowId>,
        first_leaf_row_id: TableRowId,
        leaf_row_count: usize,
    ) -> Self {
        Self {
            grouping_column,
            grouping_value,
            depth,
            parent_id,
            first_leaf_row_id,
            leaf_row_count,
        }
    }

    /// Returns the grouped column identity.
    pub const fn grouping_column(&self) -> &TableColumnId {
        &self.grouping_column
    }

    /// Returns the grouped value.
    pub const fn grouping_value(&self) -> &TableCellValue {
        &self.grouping_value
    }

    /// Returns this group row's depth in the grouped tree.
    pub const fn depth(&self) -> usize {
        self.depth
    }

    /// Returns the parent group row id, if present.
    pub const fn parent_id(&self) -> Option<&TableRowId> {
        self.parent_id.as_ref()
    }

    /// Returns the first descendant leaf row id.
    pub const fn first_leaf_row_id(&self) -> &TableRowId {
        &self.first_leaf_row_id
    }

    /// Returns the descendant leaf row count.
    pub const fn leaf_row_count(&self) -> usize {
        self.leaf_row_count
    }
}

/// Source hierarchy metadata for a resolved table row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableTreeRow {
    depth: usize,
    parent_id: Option<TableRowId>,
    has_children: bool,
    can_expand: bool,
    expanded: bool,
    descendant_count: usize,
    loaded_child_count: usize,
    children_load_state: TableRowChildrenLoadState,
}

impl TableTreeRow {
    fn new(
        depth: usize,
        parent_id: Option<TableRowId>,
        has_children: bool,
        can_expand: bool,
        expanded: bool,
        descendant_count: usize,
        loaded_child_count: usize,
        children_load_state: TableRowChildrenLoadState,
    ) -> Self {
        Self {
            depth,
            parent_id,
            has_children,
            can_expand,
            expanded,
            descendant_count,
            loaded_child_count,
            children_load_state,
        }
    }

    /// Returns this source row's zero-based depth.
    pub const fn depth(&self) -> usize {
        self.depth
    }

    /// Returns the parent source row id, if present.
    pub const fn parent_id(&self) -> Option<&TableRowId> {
        self.parent_id.as_ref()
    }

    /// Returns whether this source row has nested children.
    pub const fn has_children(&self) -> bool {
        self.has_children
    }

    /// Returns whether this source row can be expanded.
    pub const fn can_expand(&self) -> bool {
        self.can_expand
    }

    /// Returns whether this source branch is expanded in caller-owned state.
    pub const fn expanded(&self) -> bool {
        self.expanded
    }

    /// Returns the number of nested descendant source rows.
    pub const fn descendant_count(&self) -> usize {
        self.descendant_count
    }

    /// Returns the number of directly loaded child rows.
    pub const fn loaded_child_count(&self) -> usize {
        self.loaded_child_count
    }

    /// Returns caller-owned child loading metadata.
    pub const fn children_load_state(&self) -> &TableRowChildrenLoadState {
        &self.children_load_state
    }
}

/// Resolved row kind for Open GPUI table row models.
#[derive(Debug, Clone, PartialEq)]
pub enum TableResolvedRowKind {
    /// A row backed by one source data row.
    Leaf,
    /// A synthetic grouped row.
    Group(TableGroupRow),
}

/// A resolved row that carries source identity and derived metadata.
#[derive(Debug, Clone, PartialEq)]
pub struct TableResolvedRow {
    id: TableRowId,
    cells: BTreeMap<TableColumnId, TableCellValue>,
    source: Option<TableRow>,
    source_index: Option<usize>,
    selected: bool,
    kind: TableResolvedRowKind,
    tree: Option<TableTreeRow>,
    depth: usize,
    parent_id: Option<TableRowId>,
}

impl TableResolvedRow {
    fn from_row(
        row: &TableRow,
        source_index: usize,
        selected: bool,
        tree: Option<TableTreeRow>,
    ) -> Self {
        let depth = tree.as_ref().map(TableTreeRow::depth).unwrap_or(0);
        let parent_id = tree.as_ref().and_then(|tree| tree.parent_id().cloned());
        Self {
            id: row.id().clone(),
            cells: row.cells().clone(),
            source: Some(row.clone()),
            source_index: Some(source_index),
            selected,
            kind: TableResolvedRowKind::Leaf,
            tree,
            depth,
            parent_id,
        }
    }

    fn from_group(
        id: TableRowId,
        group: TableGroupRow,
        aggregate_cells: BTreeMap<TableColumnId, TableCellValue>,
    ) -> Self {
        let mut cells = aggregate_cells;
        cells.insert(
            group.grouping_column().clone(),
            group.grouping_value().clone(),
        );

        Self {
            id,
            cells,
            source: None,
            source_index: None,
            selected: false,
            depth: group.depth(),
            parent_id: group.parent_id().cloned(),
            kind: TableResolvedRowKind::Group(group),
            tree: None,
        }
    }

    fn with_parent(mut self, parent_id: TableRowId, depth: usize) -> Self {
        self.parent_id = Some(parent_id);
        self.depth = depth;
        self
    }

    /// Returns the stable row identity.
    pub const fn id(&self) -> &TableRowId {
        &self.id
    }

    /// Returns the resolved row kind.
    pub const fn kind(&self) -> &TableResolvedRowKind {
        &self.kind
    }

    /// Returns true when this is a grouped row.
    pub const fn is_group(&self) -> bool {
        matches!(self.kind, TableResolvedRowKind::Group(_))
    }

    /// Returns true when this is a leaf source row.
    pub const fn is_leaf(&self) -> bool {
        matches!(self.kind, TableResolvedRowKind::Leaf)
    }

    /// Returns grouped row metadata when this row is a group row.
    pub const fn group(&self) -> Option<&TableGroupRow> {
        match &self.kind {
            TableResolvedRowKind::Group(group) => Some(group),
            TableResolvedRowKind::Leaf => None,
        }
    }

    /// Returns source hierarchy metadata when this row came from source tree data.
    pub const fn tree(&self) -> Option<&TableTreeRow> {
        self.tree.as_ref()
    }

    /// Returns whether this row is a source row that can expand.
    pub fn is_tree_branch(&self) -> bool {
        self.tree().map(TableTreeRow::can_expand).unwrap_or(false)
    }

    /// Returns whether this source branch is expanded in caller-owned state.
    pub fn tree_expanded(&self) -> Option<bool> {
        self.tree()
            .filter(|tree| tree.can_expand())
            .map(TableTreeRow::expanded)
    }

    /// Returns the number of nested source descendants.
    pub fn descendant_count(&self) -> usize {
        self.tree().map(TableTreeRow::descendant_count).unwrap_or(0)
    }

    /// Returns the number of directly loaded child rows.
    pub fn loaded_child_count(&self) -> usize {
        self.tree()
            .map(TableTreeRow::loaded_child_count)
            .unwrap_or(0)
    }

    /// Returns caller-owned child loading metadata when this is a source-tree row.
    pub fn children_load_state(&self) -> Option<&TableRowChildrenLoadState> {
        self.tree().map(TableTreeRow::children_load_state)
    }

    /// Returns the original row descriptor for leaf rows.
    pub const fn source(&self) -> Option<&TableRow> {
        self.source.as_ref()
    }

    /// Returns all resolved cells keyed by column identity.
    pub const fn cells(&self) -> &BTreeMap<TableColumnId, TableCellValue> {
        &self.cells
    }

    /// Returns a resolved cell value for the given column.
    pub fn cell(&self, column: &TableColumnId) -> Option<&TableCellValue> {
        self.cells.get(column)
    }

    /// Returns the original source index before row-model transforms for leaf rows.
    pub const fn source_index(&self) -> Option<usize> {
        self.source_index
    }

    /// Returns this row's depth in a grouped row model.
    pub const fn depth(&self) -> usize {
        self.depth
    }

    /// Returns the parent group row id, if present.
    pub const fn parent_id(&self) -> Option<&TableRowId> {
        self.parent_id.as_ref()
    }

    /// Returns whether this row id is selected.
    pub const fn selected(&self) -> bool {
        self.selected
    }
}

/// Resolved rows for one row-model stage.
#[derive(Debug, Clone, PartialEq)]
pub struct TableRowModel {
    stage: TableRowModelStage,
    rows: Vec<TableResolvedRow>,
    rows_by_id: BTreeMap<TableRowId, TableResolvedRow>,
}

impl TableRowModel {
    /// Creates a row model from rows at one stage.
    pub fn new(stage: TableRowModelStage, rows: impl Into<Vec<TableResolvedRow>>) -> Self {
        let rows = rows.into();
        Self::new_with_lookup(stage, rows.clone(), rows)
    }

    fn new_with_lookup(
        stage: TableRowModelStage,
        rows: impl Into<Vec<TableResolvedRow>>,
        lookup_rows: impl IntoIterator<Item = TableResolvedRow>,
    ) -> Self {
        let rows = rows.into();
        let rows_by_id = lookup_rows
            .into_iter()
            .map(|row| (row.id().clone(), row))
            .collect();

        Self {
            stage,
            rows,
            rows_by_id,
        }
    }

    /// Returns this model's stage.
    pub const fn stage(&self) -> TableRowModelStage {
        self.stage
    }

    /// Returns rows in model order.
    pub fn rows(&self) -> &[TableResolvedRow] {
        &self.rows
    }

    /// Returns the row lookup for this model.
    pub const fn rows_by_id(&self) -> &BTreeMap<TableRowId, TableResolvedRow> {
        &self.rows_by_id
    }

    /// Returns a row by stable id.
    pub fn row(&self, id: &TableRowId) -> Option<&TableResolvedRow> {
        self.rows_by_id.get(id)
    }

    /// Returns the number of selected rows in this model.
    pub fn selected_count(&self) -> usize {
        self.rows.iter().filter(|row| row.selected()).count()
    }
}

/// Resolved table row models and metadata.
#[derive(Debug, Clone, PartialEq)]
pub struct TableResolvedState {
    visible_columns: Vec<TableColumn>,
    visible_column_regions: TableColumnRegions,
    visible_column_sizing: TableResolvedColumnSizingRegions,
    duplicate_row_ids: Vec<TableRowId>,
    faceting_mode: TableStageMode,
    column_facets: Vec<TableColumnFacets>,
    row_pinning_policy: TableRowPinningPolicy,
    selection_policy: TableSelectionPolicy,
    row_regions: TableRowRegions,
    core_model: TableRowModel,
    filtered_model: TableRowModel,
    grouped_model: TableRowModel,
    sorted_model: TableRowModel,
    expanded_model: TableRowModel,
    paginated_model: TableRowModel,
    final_model: TableRowModel,
}

impl TableResolvedState {
    /// Returns visible columns in resolved order.
    pub fn visible_columns(&self) -> &[TableColumn] {
        &self.visible_columns
    }

    /// Returns visible columns split into pinned regions.
    pub const fn visible_column_regions(&self) -> &TableColumnRegions {
        &self.visible_column_regions
    }

    /// Returns resolved visible column sizing split into pinned regions.
    pub const fn visible_column_sizing(&self) -> &TableResolvedColumnSizingRegions {
        &self.visible_column_sizing
    }

    /// Returns the faceting ownership mode.
    pub const fn faceting_mode(&self) -> TableStageMode {
        self.faceting_mode
    }

    /// Returns resolved facet metadata for configured columns.
    pub fn column_facets(&self) -> &[TableColumnFacets] {
        &self.column_facets
    }

    /// Returns resolved facet metadata for one configured column.
    pub fn column_facet(&self, column: &TableColumnId) -> Option<&TableColumnFacets> {
        self.column_facets
            .iter()
            .find(|facet| facet.column() == column)
    }

    /// Returns the pinned row visibility policy.
    pub const fn row_pinning_policy(&self) -> TableRowPinningPolicy {
        self.row_pinning_policy
    }

    /// Returns the row-selection policy.
    pub const fn selection_policy(&self) -> TableSelectionPolicy {
        self.selection_policy
    }

    /// Returns resolved row metadata for pinned and center regions.
    pub const fn row_regions(&self) -> &TableRowRegions {
        &self.row_regions
    }

    /// Returns top-pinned rows.
    pub fn top_rows(&self) -> &[TableResolvedRow] {
        self.row_regions.top()
    }

    /// Returns center rows.
    pub fn center_rows(&self) -> &[TableResolvedRow] {
        self.row_regions.center()
    }

    /// Returns bottom-pinned rows.
    pub fn bottom_rows(&self) -> &[TableResolvedRow] {
        self.row_regions.bottom()
    }

    /// Returns duplicate source row ids detected during resolution.
    pub fn duplicate_row_ids(&self) -> &[TableRowId] {
        &self.duplicate_row_ids
    }

    /// Returns the core row model.
    pub const fn core_model(&self) -> &TableRowModel {
        &self.core_model
    }

    /// Returns the filtered row model.
    pub const fn filtered_model(&self) -> &TableRowModel {
        &self.filtered_model
    }

    /// Returns the grouped row model.
    pub const fn grouped_model(&self) -> &TableRowModel {
        &self.grouped_model
    }

    /// Returns the sorted row model.
    pub const fn sorted_model(&self) -> &TableRowModel {
        &self.sorted_model
    }

    /// Returns the expanded row model.
    pub const fn expanded_model(&self) -> &TableRowModel {
        &self.expanded_model
    }

    /// Returns the paginated row model.
    pub const fn paginated_model(&self) -> &TableRowModel {
        &self.paginated_model
    }

    /// Returns the final row model consumed by renderers.
    pub const fn final_model(&self) -> &TableRowModel {
        &self.final_model
    }

    /// Returns the selection summary for the core row model.
    pub fn core_selection_summary(&self) -> TableSelectionSummary {
        TableSelectionSummary::new(
            self.core_model.selected_count(),
            self.core_model.rows().len(),
        )
    }

    /// Returns the selection summary for the filtered row model.
    pub fn filtered_selection_summary(&self) -> TableSelectionSummary {
        TableSelectionSummary::new(
            self.filtered_model.selected_count(),
            self.filtered_model.rows().len(),
        )
    }

    /// Returns the selection summary for the grouped row model.
    pub fn grouped_selection_summary(&self) -> TableSelectionSummary {
        TableSelectionSummary::new(
            self.grouped_model.selected_count(),
            self.grouped_model.rows().len(),
        )
    }

    /// Returns the selection summary for the sorted row model.
    pub fn sorted_selection_summary(&self) -> TableSelectionSummary {
        TableSelectionSummary::new(
            self.sorted_model.selected_count(),
            self.sorted_model.rows().len(),
        )
    }

    /// Returns the selection summary for the expanded row model.
    pub fn expanded_selection_summary(&self) -> TableSelectionSummary {
        TableSelectionSummary::new(
            self.expanded_model.selected_count(),
            self.expanded_model.rows().len(),
        )
    }

    /// Returns the selection summary for the full resolved model before pagination.
    pub fn full_selection_summary(&self) -> TableSelectionSummary {
        self.core_selection_summary()
    }

    /// Returns the selection summary for the paginated row model.
    pub fn paginated_selection_summary(&self) -> TableSelectionSummary {
        TableSelectionSummary::new(
            self.paginated_model.selected_count(),
            self.paginated_model.rows().len(),
        )
    }

    /// Returns the selection summary for the current page scope.
    pub fn current_page_selection_summary(&self) -> TableSelectionSummary {
        self.final_selection_summary()
    }

    /// Returns the selection summary for the final row model.
    pub fn final_selection_summary(&self) -> TableSelectionSummary {
        TableSelectionSummary::new(
            self.final_model.selected_count(),
            self.final_model.rows().len(),
        )
    }
}

#[derive(Debug, Clone, PartialEq)]
struct TableRowNode {
    row: TableResolvedRow,
    children: Vec<TableRowNode>,
}

impl TableRowNode {
    fn leaf(row: TableResolvedRow) -> Self {
        Self {
            row,
            children: Vec::new(),
        }
    }
}

fn build_source_row_nodes(
    rows: &[TableRow],
    selected_rows: &BTreeSet<TableRowId>,
    expansion: &TableExpansionState,
    include_children: bool,
    parent_id: Option<TableRowId>,
    depth: usize,
    source_index: &mut usize,
) -> Vec<TableRowNode> {
    rows.iter()
        .map(|row| {
            let current_source_index = *source_index;
            *source_index += 1;
            let loaded_child_count = row.children().len();
            let can_expand = row.can_expand();

            let children = if include_children {
                build_source_row_nodes(
                    row.children(),
                    selected_rows,
                    expansion,
                    include_children,
                    Some(row.id().clone()),
                    depth + 1,
                    source_index,
                )
            } else {
                Vec::new()
            };
            let tree = (include_children && (parent_id.is_some() || can_expand)).then(|| {
                TableTreeRow::new(
                    depth,
                    parent_id.clone(),
                    loaded_child_count > 0,
                    can_expand,
                    expansion.is_expanded(row.id()),
                    count_table_rows(row.children()),
                    loaded_child_count,
                    row.children_load_state().clone(),
                )
            });
            let resolved = TableResolvedRow::from_row(
                row,
                current_source_index,
                selected_rows.contains(row.id()),
                tree,
            );

            TableRowNode {
                row: resolved,
                children,
            }
        })
        .collect()
}

fn filter_source_row_nodes(
    nodes: &[TableRowNode],
    filters: &[TableFilter],
    excluded_column: Option<&TableColumnId>,
) -> Vec<TableRowNode> {
    if filters.is_empty()
        || filters
            .iter()
            .all(|filter| excluded_column.is_some_and(|column| filter.column() == column))
    {
        return nodes.to_vec();
    }

    nodes
        .iter()
        .filter_map(|node| {
            let source = node.row.source()?;
            if !filters.iter().all(|filter| {
                excluded_column.is_some_and(|column| filter.column() == column)
                    || filter.matches(source)
            }) {
                return None;
            }

            Some(TableRowNode {
                row: node.row.clone(),
                children: filter_source_row_nodes(&node.children, filters, excluded_column),
            })
        })
        .collect()
}

fn flatten_nodes(nodes: &[TableRowNode]) -> Vec<TableResolvedRow> {
    let mut rows = Vec::new();
    for node in nodes {
        rows.push(node.row.clone());
        rows.extend(flatten_nodes(&node.children));
    }
    rows
}

fn build_group_nodes(
    rows: &[TableResolvedRow],
    grouping: &[TableColumnId],
    aggregations: &[TableAggregation],
    aggregation_fns: &BTreeMap<String, TableAggregationFn>,
    depth: usize,
    parent_group_id: Option<TableRowId>,
    inherited_parent_id: Option<TableRowId>,
) -> Vec<TableRowNode> {
    if grouping.is_empty() {
        return rows
            .iter()
            .cloned()
            .map(|row| {
                let row = match inherited_parent_id.as_ref() {
                    Some(parent_id) => row.with_parent(parent_id.clone(), depth),
                    None => row,
                };
                TableRowNode::leaf(row)
            })
            .collect();
    }

    let grouping_column = grouping[0].clone();
    let mut buckets: Vec<(String, TableCellValue, Vec<TableResolvedRow>)> = Vec::new();
    let mut bucket_index_by_key = BTreeMap::new();

    for row in rows {
        let value = row.cell(&grouping_column).cloned().unwrap_or_default();
        let key = value.filter_text();
        let index = match bucket_index_by_key.get(&key).copied() {
            Some(index) => index,
            None => {
                let index = buckets.len();
                bucket_index_by_key.insert(key.clone(), index);
                buckets.push((key.clone(), value.clone(), Vec::new()));
                index
            }
        };
        buckets[index].2.push(row.clone());
    }

    let mut nodes = Vec::new();
    for (value_text, value, bucket_rows) in buckets {
        let group_id = build_group_row_id(parent_group_id.as_ref(), &grouping_column, &value_text);
        let first_leaf_row_id = bucket_rows
            .first()
            .map(|row| row.id().clone())
            .unwrap_or_else(|| group_id.clone());
        let leaf_row_count = bucket_rows.len();
        let parent_id = inherited_parent_id.clone();
        let group = TableGroupRow::new(
            grouping_column.clone(),
            value,
            depth,
            parent_id.clone(),
            first_leaf_row_id,
            leaf_row_count,
        );
        let children = build_group_nodes(
            &bucket_rows,
            &grouping[1..],
            aggregations,
            aggregation_fns,
            depth + 1,
            Some(group_id.clone()),
            Some(group_id.clone()),
        );
        let aggregate_cells = resolve_aggregate_cells(&bucket_rows, aggregations, aggregation_fns);
        let row = TableResolvedRow::from_group(group_id, group, aggregate_cells);
        nodes.push(TableRowNode { row, children });
    }

    nodes
}

fn resolve_aggregate_cells(
    rows: &[TableResolvedRow],
    aggregations: &[TableAggregation],
    aggregation_fns: &BTreeMap<String, TableAggregationFn>,
) -> BTreeMap<TableColumnId, TableCellValue> {
    aggregations
        .iter()
        .map(|aggregation| {
            (
                aggregation.column().clone(),
                resolve_aggregate_cell(rows, aggregation, aggregation_fns),
            )
        })
        .collect()
}

fn resolve_aggregate_cell(
    rows: &[TableResolvedRow],
    aggregation: &TableAggregation,
    aggregation_fns: &BTreeMap<String, TableAggregationFn>,
) -> TableCellValue {
    match aggregation.kind() {
        Some(kind) => resolve_aggregate_cell_builtin(rows, aggregation.column(), kind),
        None => match aggregation.name() {
            Some(name) => aggregation_fns
                .get(name)
                .map(|aggregation_fn| aggregation_fn.call(aggregation.column(), rows))
                .or_else(|| {
                    TableAggregateKind::from_str(name).map(|kind| {
                        resolve_aggregate_cell_builtin(rows, aggregation.column(), kind)
                    })
                })
                .unwrap_or_default(),
            None => TableCellValue::Empty,
        },
    }
}

fn resolve_aggregate_cell_builtin(
    rows: &[TableResolvedRow],
    column: &TableColumnId,
    kind: TableAggregateKind,
) -> TableCellValue {
    match kind {
        TableAggregateKind::Count => TableCellValue::Number(rows.len() as f64),
        TableAggregateKind::Sum => {
            let mut seen_numeric = false;
            let sum = numeric_values(rows, column).fold(0.0, |sum, value| {
                seen_numeric = true;
                sum + value
            });

            if seen_numeric {
                TableCellValue::Number(sum)
            } else {
                TableCellValue::Empty
            }
        }
        TableAggregateKind::Min => numeric_values(rows, column)
            .min_by(f64::total_cmp)
            .map(TableCellValue::Number)
            .unwrap_or_default(),
        TableAggregateKind::Max => numeric_values(rows, column)
            .max_by(f64::total_cmp)
            .map(TableCellValue::Number)
            .unwrap_or_default(),
        TableAggregateKind::Average => {
            let mut count = 0_usize;
            let sum = numeric_values(rows, column).fold(0.0, |sum, value| {
                count += 1;
                sum + value
            });

            if count > 0 {
                TableCellValue::Number(sum / count as f64)
            } else {
                TableCellValue::Empty
            }
        }
    }
}

fn numeric_values<'a>(
    rows: &'a [TableResolvedRow],
    column: &'a TableColumnId,
) -> impl Iterator<Item = f64> + 'a {
    rows.iter().filter_map(|row| match row.cell(column) {
        Some(TableCellValue::Number(value)) => Some(*value),
        _ => None,
    })
}

fn build_group_row_id(
    parent_id: Option<&TableRowId>,
    column: &TableColumnId,
    value_text: &str,
) -> TableRowId {
    let segment = format!("{}={}", column.as_str(), value_text);
    match parent_id {
        Some(parent) => TableRowId::new(format!("{}>{segment}", parent.as_str())),
        None => TableRowId::new(format!("group:{segment}")),
    }
}

fn push_expanded_rows(
    node: &TableRowNode,
    expansion: &TableExpansionState,
    rows: &mut Vec<TableResolvedRow>,
) {
    rows.push(node.row.clone());
    if node.children.is_empty() {
        return;
    }

    if !expansion.is_expanded(node.row.id()) {
        return;
    }

    for child in &node.children {
        push_expanded_rows(child, expansion, rows);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_rows() -> Vec<TableRow> {
        vec![
            TableRow::new("row-b")
                .with_cell("name", "Beta")
                .with_cell("team", "ops")
                .with_cell("score", 20_usize),
            TableRow::new("row-a")
                .with_cell("name", "Alpha")
                .with_cell("team", "design")
                .with_cell("score", 10_usize),
            TableRow::new("row-c")
                .with_cell("name", "Gamma")
                .with_cell("team", "ops")
                .with_cell("score", 30_usize),
        ]
    }

    fn aggregate_rows() -> Vec<TableRow> {
        vec![
            TableRow::new("row-1")
                .with_cell("team", "ops")
                .with_cell("name", "Alpha")
                .with_cell("score", 20_usize)
                .with_cell("low", 4_usize)
                .with_cell("high", 11_usize)
                .with_cell("duration", 2_usize)
                .with_cell("noise", "n/a"),
            TableRow::new("row-2")
                .with_cell("team", "ops")
                .with_cell("name", "Beta")
                .with_cell("score", 30_usize)
                .with_cell("low", 2_usize)
                .with_cell("high", 9_usize)
                .with_cell("duration", 4_usize)
                .with_cell("noise", "unknown"),
            TableRow::new("row-3")
                .with_cell("team", "design")
                .with_cell("name", "Gamma")
                .with_cell("score", 7_usize)
                .with_cell("low", 7_usize)
                .with_cell("high", 7_usize)
                .with_cell("duration", 10_usize)
                .with_cell("noise", "unknown"),
        ]
    }

    fn tree_rows() -> Vec<TableRow> {
        vec![
            TableRow::new("pkg")
                .with_cell("name", "Workspace")
                .with_cell("team", "core")
                .with_cell("score", 100_usize)
                .with_child(
                    TableRow::new("pkg-ui")
                        .with_cell("name", "UI")
                        .with_cell("team", "ui")
                        .with_cell("score", 30_usize),
                )
                .with_child(
                    TableRow::new("pkg-core")
                        .with_cell("name", "Core")
                        .with_cell("team", "core")
                        .with_cell("score", 70_usize)
                        .with_child(
                            TableRow::new("pkg-core-test")
                                .with_cell("name", "Core Test")
                                .with_cell("team", "core")
                                .with_cell("score", 10_usize),
                        ),
                ),
            TableRow::new("docs")
                .with_cell("name", "Docs")
                .with_cell("team", "docs")
                .with_cell("score", 20_usize),
        ]
    }

    fn text_facet_counts(facet: &TableColumnFacets) -> Vec<(String, usize)> {
        facet
            .unique_values()
            .iter()
            .map(|entry| match entry.value() {
                TableCellValue::Text(value) => (value.clone(), entry.count()),
                value => panic!("expected text facet value, got {value:?}"),
            })
            .collect()
    }

    fn row_ids(rows: &[TableResolvedRow]) -> Vec<&str> {
        rows.iter().map(|row| row.id().as_str()).collect()
    }

    #[test]
    fn row_model_pipeline_names_full_and_v0_stages() {
        assert_eq!(
            TABLE_ROW_MODEL_PIPELINE.map(TableRowModelStage::as_str),
            [
                "core",
                "filtered",
                "grouped",
                "sorted",
                "expanded",
                "paginated",
                "final"
            ]
        );
        assert_eq!(
            TABLE_ROW_MODEL_V0_PIPELINE.map(TableRowModelStage::as_str),
            ["core", "filtered", "sorted", "paginated", "final"]
        );
        assert!(!TableRowModelStage::Grouped.implemented_in_v0());
        assert!(!TableRowModelStage::Expanded.implemented_in_v0());
        assert!(TableRowModelStage::Sorted.implemented_in_v0());
    }

    #[test]
    fn column_widths_resolve_from_defaults_and_committed_sizing() {
        let column = TableColumn::new("name", "Name")
            .with_width(ui_px(120.0))
            .with_min_width(ui_px(80.0))
            .with_max_width(ui_px(160.0));

        assert_eq!(column.width(), ui_px(120.0));
        assert_eq!(column.min_width(), ui_px(80.0));
        assert_eq!(column.max_width(), ui_px(160.0));
        assert!(column.resizable());
        assert_eq!(
            column.resolved_width(&TableColumnSizing::new()),
            ui_px(120.0),
            "without committed sizing, the preferred width is used"
        );

        let undersized = TableColumnSizing::new().with_width("name", ui_px(40.0));
        assert_eq!(
            column.resolved_width(&undersized),
            ui_px(80.0),
            "committed widths are clamped to the column minimum"
        );

        let oversized = TableColumnSizing::new().with_width("name", ui_px(220.0));
        assert_eq!(
            column.resolved_width(&oversized),
            ui_px(160.0),
            "committed widths are clamped to the column maximum"
        );

        let unrelated = TableColumnSizing::new().with_width("team", ui_px(140.0));
        assert_eq!(
            column.resolved_width(&unrelated),
            ui_px(120.0),
            "unknown committed sizing ids do not affect this column"
        );
    }

    #[test]
    fn sizing_state_keeps_unknown_ids_without_changing_visible_columns() {
        let state = TableState::new(sample_rows())
            .with_columns([
                TableColumn::new("name", "Name"),
                TableColumn::new("team", "Team").with_visible(false),
            ])
            .with_column_sizing(TableColumnSizing::from_widths([
                ("team", ui_px(320.0)),
                ("missing", ui_px(480.0)),
            ]));

        let visible_columns = state.visible_columns();
        assert_eq!(
            visible_columns
                .iter()
                .map(|column| column.id().as_str())
                .collect::<Vec<_>>(),
            ["name"]
        );
        assert_eq!(
            visible_columns[0].resolved_width(state.column_sizing()),
            TABLE_DEFAULT_COLUMN_WIDTH,
            "hidden and unknown sizing entries do not contribute visible widths"
        );
        assert_eq!(
            state.column_sizing().width(&TableColumnId::new("missing")),
            Some(ui_px(480.0)),
            "unknown ids remain caller-owned state instead of being silently pruned"
        );
    }

    #[test]
    fn resolved_column_sizing_tracks_region_offsets_and_totals() {
        let resolved = TableState::new(sample_rows())
            .with_columns([
                TableColumn::new("name", "Name").with_width(ui_px(100.0)),
                TableColumn::new("team", "Team").with_width(ui_px(120.0)),
                TableColumn::new("score", "Score")
                    .with_width(ui_px(80.0))
                    .with_min_width(ui_px(70.0))
                    .with_max_width(ui_px(90.0)),
                TableColumn::new("status", "Status")
                    .with_width(ui_px(60.0))
                    .with_resizable(false),
            ])
            .with_column_order(["status", "score", "team", "name"])
            .with_column_pinning(
                TableColumnPinning::new()
                    .pinned_left(["name", "score"])
                    .pinned_right(["status"]),
            )
            .with_column_sizing(TableColumnSizing::new().with_width("score", ui_px(95.0)))
            .resolve();
        let sizing = resolved.visible_column_sizing();

        assert_eq!(sizing.left_width(), ui_px(190.0));
        assert_eq!(sizing.center_width(), ui_px(120.0));
        assert_eq!(sizing.right_width(), ui_px(60.0));
        assert_eq!(sizing.total_width(), ui_px(370.0));
        assert_eq!(sizing.region_width(TableColumnRegion::Left), ui_px(190.0));
        assert_eq!(
            sizing
                .left()
                .iter()
                .map(|column| {
                    (
                        column.column_id().as_str(),
                        column.width(),
                        column.start(),
                        column.after(),
                    )
                })
                .collect::<Vec<_>>(),
            [
                ("score", ui_px(90.0), ui_px(0.0), ui_px(100.0)),
                ("name", ui_px(100.0), ui_px(90.0), ui_px(0.0)),
            ],
            "left region offsets follow resolved visible order and clamp committed widths"
        );

        let status = sizing
            .column(&TableColumnId::new("status"))
            .expect("status sizing should resolve");
        assert_eq!(status.region(), TableColumnRegion::Right);
        assert_eq!(status.width(), ui_px(60.0));
        assert_eq!(status.start(), ui_px(0.0));
        assert_eq!(status.after(), ui_px(0.0));
        assert!(!status.resizable());
    }

    #[test]
    fn resolved_column_sizing_is_stable_across_row_model_changes() {
        let base = TableState::new(sample_rows())
            .with_columns([
                TableColumn::new("name", "Name").with_width(ui_px(100.0)),
                TableColumn::new("team", "Team").with_width(ui_px(120.0)),
                TableColumn::new("score", "Score").with_width(ui_px(80.0)),
            ])
            .with_column_sizing(TableColumnSizing::new().with_width("score", ui_px(96.0)));

        let base_sizing = base.resolve().visible_column_sizing().clone();
        let changed_rows = base
            .with_sorting([TableSort::descending("score")])
            .with_selected_rows(["row-a"])
            .with_pagination(TablePagination::new(0, 1))
            .resolve()
            .visible_column_sizing()
            .clone();

        assert_eq!(base_sizing, changed_rows);
    }

    #[test]
    fn column_resize_on_end_commits_only_when_finished() {
        let sizing = TableColumnSizing::new().with_width("name", ui_px(100.0));
        let resize = TableColumnResizeState::begin(
            "name",
            ui_px(10.0),
            ui_px(100.0),
            [("name", ui_px(100.0))],
        );

        let moved = drag_table_column_resize(
            TableColumnResizeMode::OnEnd,
            TableColumnResizeDirection::Ltr,
            &sizing,
            &resize,
            ui_px(60.0),
        );
        assert!(moved.committed_sizing().is_none());
        assert_eq!(moved.state().delta_offset(), Some(ui_px(50.0)));
        assert_eq!(moved.state().delta_percentage(), Some(0.5));
        assert_eq!(
            moved.state().preview_width(&TableColumnId::new("name")),
            Some(ui_px(150.0))
        );

        let ended = end_table_column_resize(
            TableColumnResizeMode::OnEnd,
            TableColumnResizeDirection::Ltr,
            &sizing,
            moved.state(),
            Some(ui_px(60.0)),
        );
        assert!(!ended.state().is_resizing());
        assert_eq!(
            ended
                .committed_sizing()
                .and_then(|sizing| sizing.width(&TableColumnId::new("name"))),
            Some(ui_px(150.0))
        );
    }

    #[test]
    fn column_resize_on_change_commits_during_drag_and_resets_on_end() {
        let sizing = TableColumnSizing::new().with_width("name", ui_px(100.0));
        let resize = TableColumnResizeState::begin(
            "name",
            ui_px(10.0),
            ui_px(100.0),
            [("name", ui_px(100.0))],
        );

        let moved = drag_table_column_resize(
            TableColumnResizeMode::OnChange,
            TableColumnResizeDirection::Ltr,
            &sizing,
            &resize,
            ui_px(60.0),
        );
        assert_eq!(
            moved
                .committed_sizing()
                .and_then(|sizing| sizing.width(&TableColumnId::new("name"))),
            Some(ui_px(150.0))
        );

        let ended = end_table_column_resize(
            TableColumnResizeMode::OnChange,
            TableColumnResizeDirection::Ltr,
            &sizing,
            moved.state(),
            Some(ui_px(60.0)),
        );
        assert!(!ended.state().is_resizing());
        assert_eq!(
            ended
                .committed_sizing()
                .and_then(|sizing| sizing.width(&TableColumnId::new("name"))),
            Some(ui_px(150.0))
        );
    }

    #[test]
    fn column_resize_rtl_flips_pointer_delta() {
        let sizing = TableColumnSizing::new().with_width("name", ui_px(100.0));
        let resize = TableColumnResizeState::begin(
            "name",
            ui_px(10.0),
            ui_px(100.0),
            [("name", ui_px(100.0))],
        );

        let moved = drag_table_column_resize(
            TableColumnResizeMode::OnChange,
            TableColumnResizeDirection::Rtl,
            &sizing,
            &resize,
            ui_px(60.0),
        );

        assert_eq!(moved.state().delta_offset(), Some(ui_px(-50.0)));
        assert_eq!(
            moved
                .committed_sizing()
                .and_then(|sizing| sizing.width(&TableColumnId::new("name"))),
            Some(ui_px(50.0))
        );
    }

    #[test]
    fn stable_row_ids_survive_filtering_sorting_and_pagination() {
        let resolved = TableState::new(sample_rows())
            .with_filters([TableFilter::contains("team", "ops")])
            .with_sorting([TableSort::descending("score")])
            .with_pagination(TablePagination::new(0, 1))
            .resolve();

        assert_eq!(
            resolved
                .filtered_model()
                .rows()
                .iter()
                .map(|row| row.id().as_str())
                .collect::<Vec<_>>(),
            ["row-b", "row-c"]
        );
        assert_eq!(
            resolved
                .sorted_model()
                .rows()
                .iter()
                .map(|row| row.id().as_str())
                .collect::<Vec<_>>(),
            ["row-c", "row-b"]
        );
        assert_eq!(
            resolved
                .final_model()
                .rows()
                .iter()
                .map(|row| row.id().as_str())
                .collect::<Vec<_>>(),
            ["row-c"]
        );
        assert!(
            resolved
                .core_model()
                .row(&TableRowId::new("row-b"))
                .is_some()
        );
    }

    #[test]
    fn categorical_filters_match_exact_tokens_and_multiple_values() {
        let resolved = TableState::new([
            TableRow::new("row-ready")
                .with_cell("status", "Ready")
                .with_cell("score", 20_usize)
                .with_cell("enabled", true),
            TableRow::new("row-review")
                .with_cell("status", "Review")
                .with_cell("score", 30_usize)
                .with_cell("enabled", false),
            TableRow::new("row-blocked")
                .with_cell("status", "Blocked")
                .with_cell("score", 40_usize)
                .with_cell("enabled", true),
        ])
        .with_columns([
            TableColumn::new("status", "Status"),
            TableColumn::new("score", "Score"),
            TableColumn::new("enabled", "Enabled"),
        ])
        .with_filters([
            TableFilter::one_of("status", ["Ready", "Blocked"]),
            TableFilter::exact("enabled", "true"),
        ])
        .resolve();

        assert_eq!(
            resolved
                .filtered_model()
                .rows()
                .iter()
                .map(|row| row.id().as_str())
                .collect::<Vec<_>>(),
            ["row-ready", "row-blocked"],
            "categorical filters use exact facet tokens and compose with other filters"
        );
    }

    #[test]
    fn categorical_filter_values_are_order_independent_cache_keys() {
        let left = TableFilter::one_of("status", ["Ready", "Blocked", "Ready"]);
        let right = TableFilter::one_of("status", ["Blocked", "Ready"]);

        assert_eq!(
            left, right,
            "selected categorical tokens are a deterministic set, not click order"
        );
        assert_eq!(
            left.selected_values()
                .expect("categorical filter should expose selected values")
                .iter()
                .cloned()
                .collect::<Vec<_>>(),
            ["Blocked".to_string(), "Ready".to_string()]
        );

        let base = TableState::new(sample_rows()).with_columns([TableColumn::new("team", "Team")]);
        assert_eq!(
            base.clone().with_filters([left]).cache_key(),
            base.clone().with_filters([right]).cache_key(),
            "cache keys should not depend on selection order"
        );
        assert_ne!(
            base.clone()
                .with_filters([TableFilter::one_of("team", ["ops"])])
                .cache_key(),
            base.with_filters([TableFilter::one_of("team", ["design"])])
                .cache_key(),
            "changing the selected categorical token should invalidate caches"
        );
    }

    #[test]
    fn empty_categorical_filters_are_noops() {
        let resolved = TableState::new(sample_rows())
            .with_filters([TableFilter::one_of("team", std::iter::empty::<&str>())])
            .resolve();

        assert_eq!(
            resolved
                .filtered_model()
                .rows()
                .iter()
                .map(|row| row.id().as_str())
                .collect::<Vec<_>>(),
            ["row-b", "row-a", "row-c"],
            "an empty categorical filter should behave like no filter"
        );
    }

    #[test]
    fn pagination_total_page_count_uses_row_count_or_explicit_page_count() {
        let pagination = TablePagination::manual(2, 10, 42);

        assert_eq!(pagination.mode(), TableStageMode::Manual);
        assert!(pagination.is_manual());
        assert_eq!(pagination.page_index(), 2);
        assert_eq!(pagination.page_size(), 10);
        assert_eq!(pagination.row_count(), Some(42));
        assert_eq!(pagination.page_count(), Some(5));
        assert_eq!(pagination.with_page_count(9).page_count(), Some(9));
        assert_eq!(TablePagination::new(0, 10).page_count(), None);
        assert_eq!(TablePagination::manual(0, 0, 42).page_count(), Some(0));
    }

    #[test]
    fn manual_row_model_modes_preserve_supplied_snapshot() {
        let resolved = TableState::new(sample_rows())
            .with_filters([TableFilter::contains("team", "missing")])
            .with_manual_filtering()
            .with_sorting([TableSort::ascending("score")])
            .with_manual_sorting()
            .with_pagination(TablePagination::manual(2, 1, 30))
            .resolve();

        let expected = ["row-b", "row-a", "row-c"];
        assert_eq!(
            resolved
                .filtered_model()
                .rows()
                .iter()
                .map(|row| row.id().as_str())
                .collect::<Vec<_>>(),
            expected
        );
        assert_eq!(
            resolved
                .sorted_model()
                .rows()
                .iter()
                .map(|row| row.id().as_str())
                .collect::<Vec<_>>(),
            expected
        );
        assert_eq!(
            resolved
                .final_model()
                .rows()
                .iter()
                .map(|row| row.id().as_str())
                .collect::<Vec<_>>(),
            expected
        );
    }

    #[test]
    fn manual_stage_modes_participate_in_cache_keys() {
        let state = TableState::new(sample_rows())
            .with_filters([TableFilter::contains("team", "ops")])
            .with_sorting([TableSort::descending("score")])
            .with_pagination(TablePagination::new(0, 1));

        assert_ne!(
            state.cache_key(),
            state.clone().with_manual_filtering().cache_key()
        );
        assert_ne!(
            state.cache_key(),
            state.clone().with_manual_sorting().cache_key()
        );
        assert_ne!(
            state.cache_key(),
            state
                .with_pagination(TablePagination::manual(0, 1, 30))
                .cache_key()
        );
    }

    #[test]
    fn row_pinning_state_dedupes_and_moves_rows_between_regions() {
        let pinning = TableRowPinning::new()
            .pinned_top(["row-a", "row-b", "row-a"])
            .pinned_bottom(["row-b", "row-c", "row-c"]);

        assert_eq!(
            pinning
                .top()
                .iter()
                .map(|row| row.as_str())
                .collect::<Vec<_>>(),
            ["row-a"],
            "bottom pins remove overlapping top pins"
        );
        assert_eq!(
            pinning
                .bottom()
                .iter()
                .map(|row| row.as_str())
                .collect::<Vec<_>>(),
            ["row-b", "row-c"]
        );

        let moved = pinning.pinned_top(["row-c"]);
        assert_eq!(
            moved
                .top()
                .iter()
                .map(|row| row.as_str())
                .collect::<Vec<_>>(),
            ["row-c"]
        );
        assert_eq!(
            moved
                .bottom()
                .iter()
                .map(|row| row.as_str())
                .collect::<Vec<_>>(),
            ["row-b"]
        );
    }

    #[test]
    fn row_pinning_keep_pinned_rows_partitions_final_model_around_page() {
        let resolved = TableState::new(sample_rows())
            .with_pagination(TablePagination::new(1, 1))
            .with_row_pinning(
                TableRowPinning::new()
                    .pinned_top(["row-b"])
                    .pinned_bottom(["row-c"]),
            )
            .resolve();

        assert_eq!(
            resolved.row_pinning_policy(),
            TableRowPinningPolicy::KeepPinnedRows
        );
        assert_eq!(row_ids(resolved.paginated_model().rows()), ["row-a"]);
        assert_eq!(row_ids(resolved.row_regions().top()), ["row-b"]);
        assert_eq!(row_ids(resolved.row_regions().center()), ["row-a"]);
        assert_eq!(row_ids(resolved.row_regions().bottom()), ["row-c"]);
        assert_eq!(
            row_ids(resolved.final_model().rows()),
            ["row-b", "row-a", "row-c"]
        );
    }

    #[test]
    fn row_pinning_page_only_policy_ignores_rows_outside_page() {
        let resolved = TableState::new(sample_rows())
            .with_pagination(TablePagination::new(1, 1))
            .with_row_pinning(
                TableRowPinning::new()
                    .pinned_top(["row-b"])
                    .pinned_bottom(["row-c"]),
            )
            .with_row_pinning_policy(TableRowPinningPolicy::PageOnly)
            .resolve();

        assert_eq!(
            resolved.row_pinning_policy(),
            TableRowPinningPolicy::PageOnly
        );
        assert!(resolved.row_regions().top().is_empty());
        assert_eq!(row_ids(resolved.row_regions().center()), ["row-a"]);
        assert!(resolved.row_regions().bottom().is_empty());
        assert_eq!(row_ids(resolved.final_model().rows()), ["row-a"]);
    }

    #[test]
    fn row_pinning_ignores_unknown_filtered_and_collapsed_rows() {
        let filtered = TableState::new(sample_rows())
            .with_filters([TableFilter::contains("team", "ops")])
            .with_row_pinning(
                TableRowPinning::new()
                    .pinned_top(["missing", "row-a"])
                    .pinned_bottom(["row-c"]),
            )
            .with_pagination(TablePagination::disabled())
            .resolve();

        assert!(filtered.row_regions().top().is_empty());
        assert_eq!(row_ids(filtered.row_regions().center()), ["row-b"]);
        assert_eq!(row_ids(filtered.row_regions().bottom()), ["row-c"]);
        assert_eq!(row_ids(filtered.final_model().rows()), ["row-b", "row-c"]);

        let collapsed = TableState::new(tree_rows())
            .with_columns([TableColumn::new("name", "Name")])
            .with_row_pinning(TableRowPinning::new().pinned_top(["pkg-core-test"]))
            .resolve();

        assert!(
            collapsed.row_regions().top().is_empty(),
            "collapsed descendants are not promoted into pinned bands"
        );
        assert_eq!(row_ids(collapsed.final_model().rows()), ["pkg", "docs"]);
        assert!(
            collapsed
                .final_model()
                .row(&TableRowId::new("pkg-core-test"))
                .is_some(),
            "collapsed descendants remain addressable through row lookup"
        );
    }

    #[test]
    fn row_pinning_preserves_duplicate_source_row_instances_in_visual_order() {
        let resolved = TableState::new([
            TableRow::new("duplicate").with_cell("name", "First"),
            TableRow::new("unique").with_cell("name", "Middle"),
            TableRow::new("duplicate").with_cell("name", "Second"),
        ])
        .with_row_pinning(TableRowPinning::new().pinned_top(["duplicate"]))
        .resolve();

        assert_eq!(
            row_ids(resolved.row_regions().top()),
            ["duplicate", "duplicate"]
        );
        assert_eq!(row_ids(resolved.row_regions().center()), ["unique"]);
        assert!(resolved.row_regions().bottom().is_empty());
        assert_eq!(
            row_ids(resolved.final_model().rows()),
            ["duplicate", "duplicate", "unique"]
        );
    }

    #[test]
    fn overlapping_raw_row_pinning_state_resolves_without_duplicates() {
        let resolved = TableState::new(sample_rows())
            .with_row_pinning(TableRowPinning {
                top: vec![TableRowId::new("row-a"), TableRowId::new("row-a")],
                bottom: vec![TableRowId::new("row-a"), TableRowId::new("row-c")],
            })
            .resolve();

        assert_eq!(row_ids(resolved.row_regions().top()), ["row-a"]);
        assert_eq!(row_ids(resolved.row_regions().bottom()), ["row-c"]);
        assert_eq!(
            row_ids(resolved.final_model().rows()),
            ["row-a", "row-b", "row-c"]
        );
    }

    #[test]
    fn selection_policy_single_keeps_only_one_selected_row() {
        let state = TableState::new(sample_rows())
            .with_selection_mode(TableSelectionMode::Single)
            .with_selected_rows(["row-a", "row-c"]);

        assert_eq!(
            state
                .selected_rows()
                .iter()
                .map(TableRowId::as_str)
                .collect::<Vec<_>>(),
            ["row-a"]
        );
        assert_eq!(
            state.selection_policy().selection_mode(),
            TableSelectionMode::Single
        );
    }

    #[test]
    fn selection_policy_descendants_propagates_to_tree_children() {
        let resolved = TableState::new(tree_rows())
            .with_selection_policy(
                TableSelectionPolicy::default()
                    .with_sub_row_policy(TableSubRowSelectionPolicy::Descendants),
            )
            .with_all_rows_expanded()
            .with_selected_rows(["pkg"])
            .resolve();

        let selected_ids = resolved
            .core_model()
            .rows()
            .iter()
            .filter(|row| row.selected())
            .map(|row| row.id().as_str())
            .collect::<Vec<_>>();

        assert_eq!(selected_ids, ["pkg", "pkg-ui", "pkg-core", "pkg-core-test"]);
        assert_eq!(resolved.core_selection_summary().selected_count(), 4);
        assert!(resolved.core_selection_summary().is_some_selected());
        assert_eq!(resolved.final_selection_summary().selected_count(), 4);
    }

    #[test]
    fn selection_summaries_report_all_some_and_none() {
        let all = TableState::new(sample_rows())
            .with_selected_rows(["row-a", "row-b", "row-c"])
            .resolve();
        let some = TableState::new(sample_rows())
            .with_selected_rows(["row-a"])
            .resolve();
        let none = TableState::new(sample_rows()).resolve();

        assert!(all.final_selection_summary().is_all_selected());
        assert_eq!(all.final_selection_summary().state().as_str(), "all");
        assert!(some.final_selection_summary().is_some_selected());
        assert_eq!(some.final_selection_summary().state().as_str(), "some");
        assert!(none.final_selection_summary().is_none_selected());
        assert_eq!(none.final_selection_summary().state().as_str(), "none");
    }

    #[test]
    fn full_and_current_page_selection_summaries_use_different_scopes() {
        let resolved = TableState::new(sample_rows())
            .with_selected_rows(["row-c"])
            .with_pagination(TablePagination::new(0, 1))
            .resolve();

        assert_eq!(resolved.full_selection_summary().selected_count(), 1);
        assert_eq!(
            resolved.current_page_selection_summary().selected_count(),
            0
        );
        assert!(resolved.full_selection_summary().is_some_selected());
        assert!(resolved.current_page_selection_summary().is_none_selected());
    }

    #[test]
    fn row_pinning_inputs_participate_in_cache_keys() {
        let state = TableState::new(sample_rows());

        assert_ne!(
            state.cache_key(),
            state
                .clone()
                .with_row_pinning(TableRowPinning::new().pinned_top(["row-a"]))
                .cache_key()
        );
        assert_ne!(
            state.cache_key(),
            state
                .with_row_pinning_policy(TableRowPinningPolicy::PageOnly)
                .cache_key()
        );
    }

    #[test]
    fn facet_values_are_deterministic_and_ranges_ignore_non_numeric_values() {
        let resolved = TableState::new([
            TableRow::new("row-empty").with_cell("score", 4_usize),
            TableRow::new("row-bool")
                .with_cell("mixed", true)
                .with_cell("score", "n/a"),
            TableRow::new("row-number")
                .with_cell("mixed", 1_usize)
                .with_cell("score", 10_usize),
            TableRow::new("row-number-2")
                .with_cell("mixed", 1_usize)
                .with_cell("score", f64::INFINITY),
            TableRow::new("row-text")
                .with_cell("mixed", "1")
                .with_cell("score", f64::NAN),
        ])
        .with_columns([
            TableColumn::new("mixed", "Mixed"),
            TableColumn::new("score", "Score"),
        ])
        .resolve();

        let mixed = resolved
            .column_facet(&TableColumnId::new("mixed"))
            .expect("mixed facet should resolve");

        assert_eq!(mixed.mode(), TableStageMode::Client);
        assert_eq!(mixed.row_count(), 5);
        assert_eq!(mixed.unique_values().len(), 4);
        assert!(matches!(
            mixed.unique_values()[0].value(),
            TableCellValue::Empty
        ));
        assert_eq!(mixed.unique_values()[0].count(), 1);
        assert!(matches!(
            mixed.unique_values()[1].value(),
            TableCellValue::Bool(true)
        ));
        assert_eq!(mixed.unique_values()[1].count(), 1);
        assert!(matches!(
            mixed.unique_values()[2].value(),
            TableCellValue::Number(value) if *value == 1.0
        ));
        assert_eq!(mixed.unique_values()[2].count(), 2);
        assert!(matches!(
            mixed.unique_values()[3].value(),
            TableCellValue::Text(value) if value == "1"
        ));
        assert_eq!(mixed.unique_values()[3].count(), 1);

        let score = resolved
            .column_facet(&TableColumnId::new("score"))
            .expect("score facet should resolve");
        let range = score
            .numeric_range()
            .expect("finite score values should produce a range");
        assert_eq!(range.min(), 4.0);
        assert_eq!(range.max(), 10.0);
    }

    #[test]
    fn client_facets_exclude_own_filter_and_ignore_pagination() {
        let resolved = TableState::new([
            TableRow::new("row-1")
                .with_cell("team", "UI")
                .with_cell("status", "Ready")
                .with_cell("score", 10_usize),
            TableRow::new("row-2")
                .with_cell("team", "UI")
                .with_cell("status", "Blocked")
                .with_cell("score", 20_usize),
            TableRow::new("row-3")
                .with_cell("team", "API")
                .with_cell("status", "Ready")
                .with_cell("score", 30_usize),
            TableRow::new("row-4")
                .with_cell("team", "UI")
                .with_cell("status", "Ready")
                .with_cell("score", 40_usize),
        ])
        .with_columns([
            TableColumn::new("team", "Team"),
            TableColumn::new("status", "Status"),
            TableColumn::new("score", "Score"),
        ])
        .with_filters([
            TableFilter::contains("status", "Ready"),
            TableFilter::contains("team", "UI"),
        ])
        .with_pagination(TablePagination::new(0, 1))
        .resolve();

        assert_eq!(
            resolved
                .final_model()
                .rows()
                .iter()
                .map(|row| row.id().as_str())
                .collect::<Vec<_>>(),
            ["row-1"],
            "pagination still limits the final row model"
        );

        let status = resolved
            .column_facet(&TableColumnId::new("status"))
            .expect("status facet should resolve");
        assert_eq!(status.row_count(), 3);
        assert_eq!(
            text_facet_counts(status),
            [("Blocked".to_string(), 1), ("Ready".to_string(), 2)],
            "status facet ignores its own filter and honors the team filter"
        );

        let team = resolved
            .column_facet(&TableColumnId::new("team"))
            .expect("team facet should resolve");
        assert_eq!(team.row_count(), 3);
        assert_eq!(
            text_facet_counts(team),
            [("API".to_string(), 1), ("UI".to_string(), 2)],
            "team facet ignores its own filter and honors the status filter"
        );
    }

    #[test]
    fn manual_filtering_client_facets_describe_supplied_snapshot() {
        let resolved = TableState::new(sample_rows())
            .with_columns([TableColumn::new("team", "Team")])
            .with_filters([TableFilter::contains("team", "missing")])
            .with_manual_filtering()
            .resolve();

        let team = resolved
            .column_facet(&TableColumnId::new("team"))
            .expect("team facet should resolve");

        assert_eq!(team.mode(), TableStageMode::Client);
        assert_eq!(team.row_count(), 3);
        assert_eq!(
            text_facet_counts(team),
            [("design".to_string(), 1), ("ops".to_string(), 2)],
            "manual filtering leaves client facets scoped to the supplied snapshot"
        );
    }

    #[test]
    fn manual_facet_payloads_override_client_facets_and_cache_keys() {
        let base = TableState::new([
            TableRow::new("row-1").with_cell("status", "Ready"),
            TableRow::new("row-2").with_cell("status", "Ready"),
        ])
        .with_columns([TableColumn::new("status", "Status")]);
        let server_facets = TableColumnFacets::manual("status", 64).with_unique_values([
            TableFacetValueCount::new("Blocked", 24),
            TableFacetValueCount::new("Ready", 40),
        ]);

        let resolved = base
            .clone()
            .with_manual_facets([server_facets.clone()])
            .resolve();
        let status = resolved
            .column_facet(&TableColumnId::new("status"))
            .expect("status facet should resolve");

        assert_eq!(status.mode(), TableStageMode::Manual);
        assert_eq!(status.row_count(), 64);
        assert_eq!(
            text_facet_counts(status),
            [("Blocked".to_string(), 24), ("Ready".to_string(), 40)],
            "manual payloads should not be derived from the current row snapshot"
        );

        assert_ne!(
            base.cache_key(),
            base.clone().with_manual_faceting().cache_key(),
            "faceting ownership participates in cache keys"
        );
        assert_ne!(
            base.clone().with_manual_facets([server_facets]).cache_key(),
            base.clone()
                .with_manual_facets([TableColumnFacets::manual("status", 64)
                    .with_unique_values([TableFacetValueCount::new("Ready", 64)])])
                .cache_key(),
            "manual facet payload content participates in cache keys"
        );

        let nan_facets = TableColumnFacets::manual("status", 2)
            .with_unique_values([TableFacetValueCount::new(f64::NAN, 2)]);
        let same_nan_facets = TableColumnFacets::manual("status", 2)
            .with_unique_values([TableFacetValueCount::new(f64::NAN, 2)]);
        assert_eq!(
            nan_facets, same_nan_facets,
            "facet equality should use stable numeric keys instead of raw f64 equality"
        );
        assert_eq!(
            base.clone().with_manual_facets([nan_facets]).cache_key(),
            base.clone()
                .with_manual_facets([same_nan_facets])
                .cache_key(),
            "manual facet NaN payloads should not make cache keys non-reflexive"
        );

        let unknown = base
            .with_manual_facets([TableColumnFacets::manual("missing", 10)])
            .resolve();
        assert!(
            unknown
                .column_facet(&TableColumnId::new("missing"))
                .is_none()
        );
        assert!(
            unknown
                .column_facet(&TableColumnId::new("status"))
                .is_some(),
            "unknown manual payloads do not corrupt configured-column facets"
        );
    }

    #[test]
    fn row_lookup_does_not_depend_on_numeric_index_positions() {
        let resolved = TableState::new(sample_rows())
            .with_sorting([TableSort::ascending("score")])
            .resolve();

        let row_c = resolved
            .core_model()
            .row(&TableRowId::new("row-c"))
            .expect("row-c should remain addressable by id");

        assert_eq!(row_c.source_index(), Some(2));
        assert_eq!(
            resolved
                .final_model()
                .rows()
                .iter()
                .map(|row| row.id().as_str())
                .collect::<Vec<_>>(),
            ["row-a", "row-b", "row-c"]
        );
    }

    #[test]
    fn selection_follows_row_ids_after_filtering_and_sorting() {
        let resolved = TableState::new(sample_rows())
            .with_selected_rows(["row-c"])
            .with_filters([TableFilter::contains("team", "ops")])
            .with_sorting([TableSort::ascending("score")])
            .resolve();

        let selected = resolved
            .final_model()
            .row(&TableRowId::new("row-c"))
            .expect("selected row should still be present");

        assert!(selected.selected());
        assert_eq!(resolved.final_model().selected_count(), 1);
    }

    #[test]
    fn nested_source_rows_resolve_parent_depth_and_lookup_metadata() {
        let resolved = TableState::new(tree_rows()).resolve();

        assert_eq!(
            resolved
                .core_model()
                .rows()
                .iter()
                .map(|row| row.id().as_str())
                .collect::<Vec<_>>(),
            ["pkg", "pkg-ui", "pkg-core", "pkg-core-test", "docs"]
        );

        let pkg = resolved
            .core_model()
            .row(&TableRowId::new("pkg"))
            .expect("root source row should be addressable");
        let pkg_tree = pkg.tree().expect("source row should expose tree metadata");
        assert_eq!(pkg.source_index(), Some(0));
        assert_eq!(pkg.depth(), 0);
        assert_eq!(pkg.parent_id(), None);
        assert!(pkg.is_tree_branch());
        assert_eq!(pkg.tree_expanded(), Some(false));
        assert_eq!(pkg_tree.descendant_count(), 3);

        let nested = resolved
            .core_model()
            .row(&TableRowId::new("pkg-core-test"))
            .expect("nested descendant should be addressable");
        assert_eq!(nested.source_index(), Some(3));
        assert_eq!(nested.depth(), 2);
        assert_eq!(nested.parent_id().map(TableRowId::as_str), Some("pkg-core"));
        assert!(!nested.is_tree_branch());
        assert_eq!(nested.descendant_count(), 0);
    }

    #[test]
    fn collapsed_tree_rows_hide_descendants_but_preserve_lookup() {
        let resolved = TableState::new(tree_rows()).resolve();

        assert_eq!(
            resolved
                .final_model()
                .rows()
                .iter()
                .map(|row| row.id().as_str())
                .collect::<Vec<_>>(),
            ["pkg", "docs"]
        );
        assert!(
            resolved
                .final_model()
                .row(&TableRowId::new("pkg-core-test"))
                .is_some(),
            "collapsed tree descendants should remain addressable by stable row id"
        );
    }

    #[test]
    fn expanded_tree_rows_show_descendants_with_parent_depth_and_selection() {
        let resolved = TableState::new(tree_rows())
            .with_expanded_rows(["pkg", "pkg-core"])
            .with_selected_rows(["pkg-core-test"])
            .resolve();

        assert_eq!(
            resolved
                .final_model()
                .rows()
                .iter()
                .map(|row| row.id().as_str())
                .collect::<Vec<_>>(),
            ["pkg", "pkg-ui", "pkg-core", "pkg-core-test", "docs"]
        );

        let pkg_core = resolved
            .final_model()
            .row(&TableRowId::new("pkg-core"))
            .expect("expanded branch should be addressable");
        assert_eq!(pkg_core.tree_expanded(), Some(true));
        assert_eq!(pkg_core.depth(), 1);
        assert_eq!(pkg_core.parent_id().map(TableRowId::as_str), Some("pkg"));

        let nested = resolved
            .final_model()
            .rows()
            .iter()
            .find(|row| row.id().as_str() == "pkg-core-test")
            .expect("expanded nested descendant should be visible");
        assert!(nested.selected());
        assert_eq!(resolved.final_model().selected_count(), 1);
    }

    #[test]
    fn child_expansion_does_not_bypass_collapsed_parent() {
        let resolved = TableState::new(tree_rows())
            .with_expanded_rows(["pkg-core"])
            .resolve();

        assert_eq!(
            resolved
                .final_model()
                .rows()
                .iter()
                .map(|row| row.id().as_str())
                .collect::<Vec<_>>(),
            ["pkg", "docs"]
        );
    }

    #[test]
    fn all_rows_expanded_expands_source_tree_branches() {
        let resolved = TableState::new(tree_rows())
            .with_all_rows_expanded()
            .resolve();

        assert_eq!(
            resolved
                .final_model()
                .rows()
                .iter()
                .map(|row| row.id().as_str())
                .collect::<Vec<_>>(),
            ["pkg", "pkg-ui", "pkg-core", "pkg-core-test", "docs"]
        );
        assert_eq!(
            resolved
                .final_model()
                .row(&TableRowId::new("pkg"))
                .and_then(TableResolvedRow::tree_expanded),
            Some(true)
        );
    }

    #[test]
    fn expandable_unloaded_source_rows_resolve_as_tree_branches() {
        let resolved = TableState::new([TableRow::new("remote-root")
            .with_cell("team", "remote")
            .with_expandable(true)])
        .resolve();

        let remote = resolved
            .final_model()
            .row(&TableRowId::new("remote-root"))
            .expect("expandable source row should resolve");
        let tree = remote
            .tree()
            .expect("expandable source row should expose tree metadata");

        assert!(remote.is_tree_branch());
        assert_eq!(remote.tree_expanded(), Some(false));
        assert!(!tree.has_children());
        assert!(tree.can_expand());
        assert_eq!(tree.loaded_child_count(), 0);
        assert_eq!(tree.children_load_state(), &TableRowChildrenLoadState::Idle);
        assert_eq!(remote.loaded_child_count(), 0);
        assert_eq!(
            remote.children_load_state(),
            Some(&TableRowChildrenLoadState::Idle)
        );
    }

    #[test]
    fn child_loading_metadata_survives_row_lookup() {
        let resolved = TableState::new([
            TableRow::new("loading").with_children_loading("Loading packages"),
            TableRow::new("failed").with_children_load_failed("Network unavailable"),
        ])
        .resolve();

        let loading = resolved
            .final_model()
            .row(&TableRowId::new("loading"))
            .expect("loading branch should resolve");
        let failed = resolved
            .final_model()
            .row(&TableRowId::new("failed"))
            .expect("failed branch should resolve");

        assert!(loading.is_tree_branch());
        assert_eq!(
            loading
                .children_load_state()
                .and_then(|state| state.message()),
            Some("Loading packages")
        );
        assert!(
            loading
                .children_load_state()
                .is_some_and(TableRowChildrenLoadState::is_loading)
        );
        assert!(failed.is_tree_branch());
        assert_eq!(
            failed
                .children_load_state()
                .and_then(|state| state.message()),
            Some("Network unavailable")
        );
        assert!(
            failed
                .children_load_state()
                .is_some_and(TableRowChildrenLoadState::is_failed)
        );
    }

    #[test]
    fn manual_expansion_keeps_supplied_tree_descendants_visible() {
        let resolved = TableState::new(tree_rows())
            .with_manual_expansion()
            .resolve();

        assert_eq!(
            resolved
                .final_model()
                .rows()
                .iter()
                .map(|row| row.id().as_str())
                .collect::<Vec<_>>(),
            ["pkg", "pkg-ui", "pkg-core", "pkg-core-test", "docs"]
        );
        assert_eq!(
            resolved
                .final_model()
                .row(&TableRowId::new("pkg"))
                .and_then(TableResolvedRow::tree_expanded),
            Some(false)
        );
    }

    #[test]
    fn manual_expansion_preserves_expanded_metadata_without_pruning() {
        let resolved = TableState::new(tree_rows())
            .with_manual_expansion()
            .with_expanded_rows(["pkg"])
            .resolve();

        assert_eq!(
            resolved
                .final_model()
                .rows()
                .iter()
                .map(|row| row.id().as_str())
                .collect::<Vec<_>>(),
            ["pkg", "pkg-ui", "pkg-core", "pkg-core-test", "docs"]
        );
        assert_eq!(
            resolved
                .final_model()
                .row(&TableRowId::new("pkg"))
                .and_then(TableResolvedRow::tree_expanded),
            Some(true)
        );
        assert_eq!(
            resolved
                .final_model()
                .row(&TableRowId::new("pkg-core"))
                .and_then(TableResolvedRow::tree_expanded),
            Some(false)
        );
    }

    #[test]
    fn manual_expansion_does_not_bypass_grouped_row_expansion() {
        let resolved = TableState::new(aggregate_rows())
            .with_grouping(["team"])
            .with_manual_expansion()
            .resolve();

        assert_eq!(
            resolved
                .final_model()
                .rows()
                .iter()
                .map(|row| row.id().as_str())
                .collect::<Vec<_>>(),
            ["group:team=ops", "group:team=design"]
        );
    }

    #[test]
    fn tree_filtering_uses_parent_to_child_policy() {
        let resolved = TableState::new(tree_rows())
            .with_filters([TableFilter::contains("team", "core")])
            .resolve();

        assert_eq!(
            resolved
                .filtered_model()
                .rows()
                .iter()
                .map(|row| row.id().as_str())
                .collect::<Vec<_>>(),
            ["pkg", "pkg-core", "pkg-core-test"]
        );

        let leaf_match_without_parent = TableState::new(tree_rows())
            .with_filters([TableFilter::contains("team", "ui")])
            .resolve();
        assert!(
            leaf_match_without_parent.filtered_model().rows().is_empty(),
            "first slice keeps TanStack's default parent-to-child filtering policy"
        );
    }

    #[test]
    fn pagination_applies_after_tree_expansion() {
        let resolved = TableState::new(tree_rows())
            .with_all_rows_expanded()
            .with_pagination(TablePagination::new(0, 3))
            .resolve();

        assert_eq!(
            resolved
                .final_model()
                .rows()
                .iter()
                .map(|row| row.id().as_str())
                .collect::<Vec<_>>(),
            ["pkg", "pkg-ui", "pkg-core"]
        );
        assert!(
            resolved
                .final_model()
                .row(&TableRowId::new("pkg-core-test"))
                .is_some(),
            "expanded-but-not-paginated tree descendants should remain addressable"
        );
    }

    #[test]
    fn duplicate_row_ids_are_reported_across_nested_source_rows() {
        let resolved = TableState::new([
            TableRow::new("root").with_child(TableRow::new("duplicate")),
            TableRow::new("duplicate"),
        ])
        .resolve();

        assert_eq!(
            resolved
                .duplicate_row_ids()
                .iter()
                .map(TableRowId::as_str)
                .collect::<Vec<_>>(),
            ["duplicate"]
        );
    }

    #[test]
    fn cache_key_row_count_includes_child_topology() {
        let flat = TableState::new([TableRow::new("root")]);
        let nested = TableState::new([TableRow::new("root").with_child(TableRow::new("child"))]);

        assert_eq!(flat.cache_key().row_count(), 1);
        assert_eq!(nested.cache_key().row_count(), 2);
        assert_ne!(flat.cache_key(), nested.cache_key());
    }

    #[test]
    fn grouping_keeps_source_tree_rows_out_of_the_grouped_path() {
        let resolved = TableState::new(tree_rows())
            .with_grouping(["team"])
            .with_all_rows_expanded()
            .resolve();

        assert_eq!(
            resolved
                .final_model()
                .rows()
                .iter()
                .map(|row| row.id().as_str())
                .collect::<Vec<_>>(),
            ["group:team=core", "pkg", "group:team=docs", "docs"]
        );
        assert!(
            resolved
                .final_model()
                .row(&TableRowId::new("pkg-ui"))
                .is_none(),
            "tree plus grouping composition is deferred for a later policy slice"
        );
    }

    #[test]
    fn grouped_row_model_creates_stable_group_rows() {
        let resolved = TableState::new(sample_rows())
            .with_grouping(["team"])
            .resolve();

        assert_eq!(
            resolved
                .grouped_model()
                .rows()
                .iter()
                .map(|row| row.id().as_str())
                .collect::<Vec<_>>(),
            [
                "group:team=ops",
                "row-b",
                "row-c",
                "group:team=design",
                "row-a"
            ]
        );

        let ops = resolved
            .grouped_model()
            .row(&TableRowId::new("group:team=ops"))
            .expect("ops group row should be addressable by id");
        let ops_group = ops.group().expect("ops row should be a group row");

        assert_eq!(ops_group.grouping_column().as_str(), "team");
        assert_eq!(ops_group.grouping_value().filter_text(), "ops");
        assert_eq!(ops_group.depth(), 0);
        assert_eq!(ops_group.parent_id(), None);
        assert_eq!(ops_group.first_leaf_row_id().as_str(), "row-b");
        assert_eq!(ops_group.leaf_row_count(), 2);
        assert!(ops.is_group());
    }

    #[test]
    fn collapsed_groups_hide_descendants_but_preserve_lookup() {
        let resolved = TableState::new(sample_rows())
            .with_grouping(["team"])
            .resolve();

        assert_eq!(
            resolved
                .expanded_model()
                .rows()
                .iter()
                .map(|row| row.id().as_str())
                .collect::<Vec<_>>(),
            ["group:team=ops", "group:team=design"]
        );
        assert!(
            resolved
                .expanded_model()
                .row(&TableRowId::new("row-c"))
                .is_some(),
            "collapsed descendants should remain addressable in lookup metadata"
        );
    }

    #[test]
    fn expanded_groups_show_descendants_with_parent_depth_and_selection() {
        let resolved = TableState::new(sample_rows())
            .with_grouping(["team"])
            .with_expanded_rows(["group:team=ops"])
            .with_selected_rows(["row-c"])
            .resolve();

        assert_eq!(
            resolved
                .final_model()
                .rows()
                .iter()
                .map(|row| row.id().as_str())
                .collect::<Vec<_>>(),
            ["group:team=ops", "row-b", "row-c", "group:team=design"]
        );

        let row_c = resolved
            .final_model()
            .rows()
            .iter()
            .find(|row| row.id().as_str() == "row-c")
            .expect("expanded descendant should be visible");

        assert_eq!(row_c.depth(), 1);
        assert_eq!(
            row_c.parent_id().map(TableRowId::as_str),
            Some("group:team=ops")
        );
        assert!(row_c.selected());
        assert_eq!(resolved.final_model().selected_count(), 1);
    }

    #[test]
    fn multi_column_grouping_creates_nested_group_paths() {
        let resolved = TableState::new(sample_rows())
            .with_grouping(["team", "score"])
            .resolve();

        let nested = resolved
            .grouped_model()
            .row(&TableRowId::new("group:team=ops>score=20"))
            .expect("nested score group should use the parent path");
        let group = nested.group().expect("nested row should be grouped");

        assert_eq!(group.depth(), 1);
        assert_eq!(
            group.parent_id().map(TableRowId::as_str),
            Some("group:team=ops")
        );
        assert_eq!(group.leaf_row_count(), 1);
    }

    #[test]
    fn pagination_applies_after_expansion() {
        let resolved = TableState::new(sample_rows())
            .with_grouping(["team"])
            .with_all_rows_expanded()
            .with_pagination(TablePagination::new(0, 2))
            .resolve();

        assert_eq!(
            resolved
                .final_model()
                .rows()
                .iter()
                .map(|row| row.id().as_str())
                .collect::<Vec<_>>(),
            ["group:team=ops", "row-b"]
        );
        assert!(
            resolved
                .final_model()
                .row(&TableRowId::new("row-c"))
                .is_some(),
            "final lookup keeps expanded-but-not-paginated rows addressable"
        );
    }

    #[test]
    fn aggregate_kind_labels_are_stable() {
        assert_eq!(TableAggregateKind::Count.as_str(), "count");
        assert_eq!(TableAggregateKind::Sum.as_str(), "sum");
        assert_eq!(TableAggregateKind::Min.as_str(), "min");
        assert_eq!(TableAggregateKind::Max.as_str(), "max");
        assert_eq!(TableAggregateKind::Average.as_str(), "average");
    }

    #[test]
    fn grouped_rows_expose_builtin_aggregate_cells() {
        let resolved = TableState::new(aggregate_rows())
            .with_grouping(["team"])
            .with_aggregations([
                TableAggregation::count("name"),
                TableAggregation::sum("score"),
                TableAggregation::min("low"),
                TableAggregation::max("high"),
                TableAggregation::average("duration"),
                TableAggregation::sum("noise"),
            ])
            .resolve();

        let ops = resolved
            .grouped_model()
            .row(&TableRowId::new("group:team=ops"))
            .expect("ops group should resolve");

        assert_eq!(
            ops.cell(&TableColumnId::new("name")),
            Some(&TableCellValue::Number(2.0))
        );
        assert_eq!(
            ops.cell(&TableColumnId::new("score")),
            Some(&TableCellValue::Number(50.0))
        );
        assert_eq!(
            ops.cell(&TableColumnId::new("low")),
            Some(&TableCellValue::Number(2.0))
        );
        assert_eq!(
            ops.cell(&TableColumnId::new("high")),
            Some(&TableCellValue::Number(11.0))
        );
        assert_eq!(
            ops.cell(&TableColumnId::new("duration")),
            Some(&TableCellValue::Number(3.0))
        );
        assert_eq!(
            ops.cell(&TableColumnId::new("noise")),
            Some(&TableCellValue::Empty)
        );
    }

    #[test]
    fn grouped_rows_resolve_named_custom_aggregation_callbacks() {
        let state = TableState::new(aggregate_rows())
            .with_grouping(["team"])
            .with_aggregations([
                TableAggregation::count("name"),
                TableAggregation::named("score", "score_plus_one"),
                TableAggregation::named("duration", "sum"),
                TableAggregation::named("noise", "missing_custom"),
            ])
            .with_aggregation_fn("score_plus_one", |column, rows| {
                TableCellValue::Number(
                    numeric_values(rows, column).fold(0.0, |sum, value| sum + value) + 1.0,
                )
            });

        let resolved = state.resolve();
        let ops = resolved
            .grouped_model()
            .row(&TableRowId::new("group:team=ops"))
            .expect("ops group should resolve");

        assert_eq!(
            ops.cell(&TableColumnId::new("score")),
            Some(&TableCellValue::Number(51.0))
        );
        assert_eq!(
            ops.cell(&TableColumnId::new("duration")),
            Some(&TableCellValue::Number(6.0))
        );
        assert_eq!(
            ops.cell(&TableColumnId::new("noise")),
            Some(&TableCellValue::Empty)
        );
        assert_eq!(state.aggregation_fn_count(), 1);
        assert!(state.has_aggregation_fn("score_plus_one"));
        assert!(!state.has_aggregation_fn("missing_custom"));
        assert_ne!(
            state.cache_key(),
            state
                .clone()
                .with_aggregation_fn("score_plus_one", |column, rows| {
                    TableCellValue::Number(
                        numeric_values(rows, column).fold(0.0, |sum, value| sum + value) + 2.0,
                    )
                })
                .cache_key()
        );
    }

    #[test]
    fn grouping_value_overrides_aggregate_on_grouping_column() {
        let resolved = TableState::new(aggregate_rows())
            .with_grouping(["team"])
            .with_aggregations([TableAggregation::count("team")])
            .resolve();

        let ops = resolved
            .grouped_model()
            .row(&TableRowId::new("group:team=ops"))
            .expect("ops group should resolve");

        assert_eq!(
            ops.cell(&TableColumnId::new("team")),
            Some(&TableCellValue::Text("ops".to_string()))
        );
    }

    #[test]
    fn visible_columns_respect_explicit_order_and_visibility() {
        let resolved = TableState::new(sample_rows())
            .with_columns([
                TableColumn::new("name", "Name"),
                TableColumn::new("team", "Team").with_visible(false),
                TableColumn::new("score", "Score"),
            ])
            .with_column_order(["score", "team", "name"])
            .resolve();

        assert_eq!(
            resolved
                .visible_columns()
                .iter()
                .map(|column| column.id().as_str())
                .collect::<Vec<_>>(),
            ["score", "name"]
        );
    }

    #[test]
    fn pinned_columns_split_visible_regions_after_order_and_visibility() {
        let resolved = TableState::new(sample_rows())
            .with_columns([
                TableColumn::new("name", "Name"),
                TableColumn::new("team", "Team").with_visible(false),
                TableColumn::new("score", "Score"),
                TableColumn::new("owner", "Owner"),
                TableColumn::new("status", "Status"),
            ])
            .with_column_order(["status", "score", "owner", "team", "name"])
            .with_column_pinning(
                TableColumnPinning::new()
                    .pinned_left(["name", "score", "missing"])
                    .pinned_right(["status"]),
            )
            .resolve();
        let regions = resolved.visible_column_regions();

        assert_eq!(TableColumnRegion::Left.as_str(), "left");
        assert_eq!(TableColumnRegion::Center.as_str(), "center");
        assert_eq!(TableColumnRegion::Right.as_str(), "right");
        assert_eq!(
            regions
                .left()
                .iter()
                .map(|column| column.id().as_str())
                .collect::<Vec<_>>(),
            ["score", "name"],
            "pinned left columns preserve resolved visible order"
        );
        assert_eq!(
            regions
                .center()
                .iter()
                .map(|column| column.id().as_str())
                .collect::<Vec<_>>(),
            ["owner"],
            "unknown and invisible pinned ids are ignored"
        );
        assert_eq!(
            regions
                .right()
                .iter()
                .map(|column| column.id().as_str())
                .collect::<Vec<_>>(),
            ["status"]
        );
        assert_eq!(
            resolved
                .visible_columns()
                .iter()
                .map(|column| column.id().as_str())
                .collect::<Vec<_>>(),
            ["score", "name", "owner", "status"]
        );
    }

    #[test]
    fn column_pinning_moves_columns_between_regions_without_duplicates() {
        let pinning = TableColumnPinning::new()
            .pinned_left(["name", "score", "name"])
            .pinned_right(["score", "status", "score"]);

        assert_eq!(
            pinning
                .left()
                .iter()
                .map(TableColumnId::as_str)
                .collect::<Vec<_>>(),
            ["name"]
        );
        assert_eq!(
            pinning
                .right()
                .iter()
                .map(TableColumnId::as_str)
                .collect::<Vec<_>>(),
            ["score", "status"]
        );
        assert!(!pinning.is_empty());
    }

    #[test]
    fn duplicate_row_ids_are_reported_without_panicking() {
        let resolved = TableState::new([
            TableRow::new("row-a").with_cell("name", "A"),
            TableRow::new("row-a").with_cell("name", "A duplicate"),
        ])
        .resolve();

        assert_eq!(
            resolved
                .duplicate_row_ids()
                .iter()
                .map(TableRowId::as_str)
                .collect::<Vec<_>>(),
            ["row-a"]
        );
    }

    #[test]
    fn cache_key_reuses_row_identity_for_clones_and_invalidates_state_changes() {
        let base = TableState::new(sample_rows()).with_columns([
            TableColumn::new("name", "Name"),
            TableColumn::new("team", "Team"),
            TableColumn::new("score", "Score"),
        ]);
        let cloned = base.clone();
        let sorted = base.clone().with_sorting([TableSort::descending("score")]);
        let aggregated = base
            .clone()
            .with_aggregations([TableAggregation::sum("score")]);
        let pinned = base.clone().with_column_pinning(
            TableColumnPinning::new()
                .pinned_left(["name"])
                .pinned_right(["score"]),
        );
        let sized = base
            .clone()
            .with_column_sizing(TableColumnSizing::new().with_width("name", ui_px(180.0)));
        let rebuilt = TableState::new(sample_rows()).with_columns([
            TableColumn::new("name", "Name"),
            TableColumn::new("team", "Team"),
            TableColumn::new("score", "Score"),
        ]);

        assert_eq!(base, cloned);
        assert_eq!(base.cache_key(), cloned.cache_key());
        assert_eq!(
            base.cache_key().rows_identity(),
            cloned.cache_key().rows_identity()
        );

        assert_ne!(base.cache_key(), sorted.cache_key());
        assert_ne!(base.cache_key(), aggregated.cache_key());
        assert_ne!(base.cache_key(), pinned.cache_key());
        assert_ne!(base.cache_key(), sized.cache_key());
        assert_eq!(base, rebuilt);
        assert_ne!(
            base.cache_key().rows_identity(),
            rebuilt.cache_key().rows_identity()
        );
        assert_ne!(base.cache_key(), rebuilt.cache_key());
    }
}
