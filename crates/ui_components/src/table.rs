//! Renderer-neutral state for dense table surfaces.

use open_gpui_ui_core::{Size, UiPx, ui_px};

use crate::roving_focus::{first_enabled, last_enabled, next_enabled};

/// Horizontal alignment for a table column.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TableColumnAlign {
    /// Align content to the leading edge.
    #[default]
    Start,
    /// Center content in the column.
    Center,
    /// Align content to the trailing edge.
    End,
}

impl TableColumnAlign {
    /// Returns the stable alignment label.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Start => "start",
            Self::Center => "center",
            Self::End => "end",
        }
    }
}

/// Pure descriptor for one table column.
#[derive(Debug, Clone, PartialEq)]
pub struct TableColumnDescriptor {
    value: String,
    label: String,
    width: Option<UiPx>,
    align: TableColumnAlign,
}

impl TableColumnDescriptor {
    /// Creates a column descriptor.
    pub fn new(value: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            value: value.into(),
            label: label.into(),
            width: None,
            align: TableColumnAlign::Start,
        }
    }

    /// Applies a fixed logical width.
    pub fn width(mut self, width: UiPx) -> Self {
        self.width = Some(width);
        self
    }

    /// Applies column alignment.
    pub const fn align(mut self, align: TableColumnAlign) -> Self {
        self.align = align;
        self
    }

    /// Returns the stable column value.
    pub fn value(&self) -> &str {
        &self.value
    }

    /// Returns the visible column label.
    pub fn label(&self) -> &str {
        &self.label
    }

    /// Returns fixed column width, when present.
    pub const fn width_value(&self) -> Option<UiPx> {
        self.width
    }

    /// Returns column alignment.
    pub const fn align_value(&self) -> TableColumnAlign {
        self.align
    }
}

/// Pure descriptor for one table row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableRowDescriptor {
    value: String,
    label: String,
    cells: Vec<String>,
    disabled: bool,
}

impl TableRowDescriptor {
    /// Creates an empty row descriptor.
    pub fn new(value: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            value: value.into(),
            label: label.into(),
            cells: Vec::new(),
            disabled: false,
        }
    }

    /// Adds one cell text value.
    pub fn cell(mut self, cell: impl Into<String>) -> Self {
        self.cells.push(cell.into());
        self
    }

    /// Adds many cell text values.
    pub fn cells(mut self, cells: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.cells.extend(cells.into_iter().map(Into::into));
        self
    }

    /// Marks this row as disabled.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Returns the stable row value.
    pub fn value(&self) -> &str {
        &self.value
    }

    /// Returns the visible row label.
    pub fn label(&self) -> &str {
        &self.label
    }

    /// Returns raw cell text values.
    pub fn cell_values(&self) -> &[String] {
        &self.cells
    }

    /// Returns whether this row is disabled.
    pub const fn disabled_state(&self) -> bool {
        self.disabled
    }
}

/// Resolved table metrics.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TableMetrics {
    row_height: UiPx,
    cell_padding_x: UiPx,
    cell_padding_y: UiPx,
    header_height: UiPx,
    text_size: UiPx,
}

impl TableMetrics {
    /// Resolves metrics from the shared foundation size vocabulary.
    pub const fn from_size(size: Size) -> Self {
        Self {
            row_height: size.list_row_h(),
            cell_padding_x: size.list_px(),
            cell_padding_y: size.list_py(),
            header_height: match size {
                Size::XSmall => ui_px(24.0),
                Size::Small => ui_px(28.0),
                Size::Medium => ui_px(32.0),
                Size::Large => ui_px(36.0),
            },
            text_size: size.control_text_px(),
        }
    }

    /// Returns row height.
    pub const fn row_height(self) -> UiPx {
        self.row_height
    }

    /// Returns cell horizontal padding.
    pub const fn cell_padding_x(self) -> UiPx {
        self.cell_padding_x
    }

    /// Returns cell vertical padding.
    pub const fn cell_padding_y(self) -> UiPx {
        self.cell_padding_y
    }

    /// Returns header row height.
    pub const fn header_height(self) -> UiPx {
        self.header_height
    }

    /// Returns table text size.
    pub const fn text_size(self) -> UiPx {
        self.text_size
    }
}

/// Resolved table column state.
#[derive(Debug, Clone, PartialEq)]
pub struct TableColumnState {
    index: usize,
    value: String,
    label: String,
    width: Option<UiPx>,
    align: TableColumnAlign,
}

impl TableColumnState {
    /// Returns the zero-based column index.
    pub const fn index(&self) -> usize {
        self.index
    }

    /// Returns the stable column value.
    pub fn value(&self) -> &str {
        &self.value
    }

    /// Returns the visible column label.
    pub fn label(&self) -> &str {
        &self.label
    }

    /// Returns fixed width, when present.
    pub const fn width(&self) -> Option<UiPx> {
        self.width
    }

    /// Returns column alignment.
    pub const fn align(&self) -> TableColumnAlign {
        self.align
    }
}

/// Resolved table cell state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableCellState {
    row_index: usize,
    column_index: usize,
    row_value: String,
    column_value: String,
    text: String,
}

impl TableCellState {
    /// Returns the zero-based row index.
    pub const fn row_index(&self) -> usize {
        self.row_index
    }

    /// Returns the zero-based column index.
    pub const fn column_index(&self) -> usize {
        self.column_index
    }

    /// Returns the stable row value.
    pub fn row_value(&self) -> &str {
        &self.row_value
    }

    /// Returns the stable column value.
    pub fn column_value(&self) -> &str {
        &self.column_value
    }

    /// Returns resolved cell text.
    pub fn text(&self) -> &str {
        &self.text
    }
}

/// Resolved table row state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableRowState {
    index: usize,
    value: String,
    label: String,
    disabled: bool,
    selected: bool,
    focused: bool,
    cells: Vec<TableCellState>,
}

impl TableRowState {
    /// Returns the zero-based row index.
    pub const fn index(&self) -> usize {
        self.index
    }

    /// Returns the stable row value.
    pub fn value(&self) -> &str {
        &self.value
    }

    /// Returns the visible row label.
    pub fn label(&self) -> &str {
        &self.label
    }

    /// Returns whether this row is disabled.
    pub const fn disabled(&self) -> bool {
        self.disabled
    }

    /// Returns whether the row participates in focus and activation.
    pub const fn focusable(&self) -> bool {
        !self.disabled
    }

    /// Returns whether this row is selected.
    pub const fn selected(&self) -> bool {
        self.selected
    }

    /// Returns whether this row currently has roving focus.
    pub const fn focused(&self) -> bool {
        self.focused
    }

    /// Returns resolved cell states.
    pub fn cells(&self) -> &[TableCellState] {
        &self.cells
    }
}

/// Resolved table row selection payload.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableSelection {
    index: usize,
    value: String,
    label: String,
}

impl TableSelection {
    /// Creates a selection payload from a row state.
    pub fn from_row(row: &TableRowState) -> Option<Self> {
        row.focusable().then(|| Self {
            index: row.index,
            value: row.value.clone(),
            label: row.label.clone(),
        })
    }

    /// Returns the selected row index.
    pub const fn index(&self) -> usize {
        self.index
    }

    /// Returns the selected row value.
    pub fn value(&self) -> &str {
        &self.value
    }

    /// Returns the selected row label.
    pub fn label(&self) -> &str {
        &self.label
    }
}

/// Resolved table state used by tests, adapters, and rendering.
#[derive(Debug, Clone, PartialEq)]
pub struct TableState {
    size: Size,
    label: String,
    columns: Vec<TableColumnState>,
    rows: Vec<TableRowState>,
    selected_index: Option<usize>,
    focused_index: Option<usize>,
    metrics: TableMetrics,
}

impl TableState {
    /// Resolves public state for a table.
    pub fn resolve(
        size: Size,
        label: impl Into<String>,
        selected_value: Option<&str>,
        focused_value: Option<&str>,
        columns: impl IntoIterator<Item = TableColumnDescriptor>,
        rows: impl IntoIterator<Item = TableRowDescriptor>,
    ) -> Self {
        let column_descriptors = columns.into_iter().collect::<Vec<_>>();
        let row_descriptors = rows.into_iter().collect::<Vec<_>>();
        let disabled = row_descriptors
            .iter()
            .map(TableRowDescriptor::disabled_state)
            .collect::<Vec<_>>();
        let selected_index = find_focusable_row(&row_descriptors, selected_value);
        let focused_index = find_focusable_row(&row_descriptors, focused_value)
            .or(selected_index)
            .or_else(|| first_enabled(&disabled));
        let columns = column_descriptors
            .iter()
            .enumerate()
            .map(|(index, column)| TableColumnState {
                index,
                value: column.value.clone(),
                label: column.label.clone(),
                width: column.width,
                align: column.align,
            })
            .collect::<Vec<_>>();
        let rows = row_descriptors
            .into_iter()
            .enumerate()
            .map(|(row_index, row)| {
                let cells = column_descriptors
                    .iter()
                    .enumerate()
                    .map(|(column_index, column)| TableCellState {
                        row_index,
                        column_index,
                        row_value: row.value.clone(),
                        column_value: column.value.clone(),
                        text: row.cells.get(column_index).cloned().unwrap_or_default(),
                    })
                    .collect();

                TableRowState {
                    index: row_index,
                    value: row.value,
                    label: row.label,
                    disabled: row.disabled,
                    selected: selected_index == Some(row_index),
                    focused: focused_index == Some(row_index),
                    cells,
                }
            })
            .collect();

        Self {
            size,
            label: label.into(),
            columns,
            rows,
            selected_index,
            focused_index,
            metrics: TableMetrics::from_size(size),
        }
    }

    /// Returns the foundation size.
    pub const fn size(&self) -> Size {
        self.size
    }

    /// Returns the accessible table label.
    pub fn label(&self) -> &str {
        &self.label
    }

    /// Returns resolved columns.
    pub fn columns(&self) -> &[TableColumnState] {
        &self.columns
    }

    /// Returns resolved rows.
    pub fn rows(&self) -> &[TableRowState] {
        &self.rows
    }

    /// Returns selected row index.
    pub const fn selected_index(&self) -> Option<usize> {
        self.selected_index
    }

    /// Returns focused row index.
    pub const fn focused_index(&self) -> Option<usize> {
        self.focused_index
    }

    /// Returns resolved metrics.
    pub const fn metrics(&self) -> TableMetrics {
        self.metrics
    }

    /// Returns whether the table has no rows or no columns.
    pub fn empty(&self) -> bool {
        self.rows.is_empty() || self.columns.is_empty()
    }

    /// Returns the target row for Up, Down, Home, or End.
    pub fn navigation_target(&self, key: &str) -> Option<&TableRowState> {
        let disabled = self
            .rows
            .iter()
            .map(|row| !row.focusable())
            .collect::<Vec<_>>();
        let target = table_navigation_target(key, self.focused_index?, &disabled)?;

        self.rows.get(target)
    }

    /// Returns activation payload for Enter or Space.
    pub fn activation_for_key(&self, key: &str) -> Option<TableSelection> {
        if !matches!(key, "enter" | "space") {
            return None;
        }

        self.focused_index
            .and_then(|index| self.rows.get(index))
            .and_then(TableSelection::from_row)
    }
}

/// Resolves table row navigation for APG-style key names.
pub fn table_navigation_target(key: &str, current: usize, disabled: &[bool]) -> Option<usize> {
    match key {
        "home" => first_enabled(disabled),
        "end" => last_enabled(disabled),
        "up" => next_enabled(disabled, current, false, true),
        "down" => next_enabled(disabled, current, true, true),
        _ => None,
    }
}

fn find_focusable_row(rows: &[TableRowDescriptor], value: Option<&str>) -> Option<usize> {
    value.and_then(|value| {
        rows.iter()
            .position(|row| row.value() == value && !row.disabled_state())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_columns() -> Vec<TableColumnDescriptor> {
        vec![
            TableColumnDescriptor::new("title", "Title").width(ui_px(240.0)),
            TableColumnDescriptor::new("status", "Status").align(TableColumnAlign::Center),
            TableColumnDescriptor::new("page", "Page").align(TableColumnAlign::End),
        ]
    }

    fn sample_rows() -> Vec<TableRowDescriptor> {
        vec![
            TableRowDescriptor::new("intro", "Introduction").cells([
                "Introduction",
                "Anchored",
                "1",
            ]),
            TableRowDescriptor::new("missing", "Missing").disabled(true),
            TableRowDescriptor::new("figure", "Figure 1").cells(["Figure 1", "Needs review"]),
        ]
    }

    #[test]
    fn table_state_resolves_columns_rows_and_cells() {
        let state = TableState::resolve(
            Size::Small,
            "Search results",
            Some("figure"),
            None,
            sample_columns(),
            sample_rows(),
        );

        assert_eq!(state.label(), "Search results");
        assert_eq!(state.columns().len(), 3);
        assert_eq!(state.rows().len(), 3);
        assert_eq!(state.selected_index(), Some(2));
        assert_eq!(state.focused_index(), Some(2));
        assert_eq!(state.columns()[0].width(), Some(ui_px(240.0)));
        assert_eq!(state.columns()[1].align(), TableColumnAlign::Center);
        assert_eq!(state.rows()[2].cells()[0].text(), "Figure 1");
        assert_eq!(state.rows()[2].cells()[1].text(), "Needs review");
        assert_eq!(state.rows()[2].cells()[2].text(), "");
    }

    #[test]
    fn table_navigation_skips_disabled_rows_and_wraps() {
        let state = TableState::resolve(
            Size::Medium,
            "Search results",
            None,
            Some("figure"),
            sample_columns(),
            sample_rows(),
        );

        assert_eq!(
            state.navigation_target("down").map(TableRowState::value),
            Some("intro")
        );
        assert_eq!(
            state.navigation_target("up").map(TableRowState::value),
            Some("intro")
        );
        assert_eq!(
            state.navigation_target("home").map(TableRowState::value),
            Some("intro")
        );
        assert_eq!(
            state.navigation_target("end").map(TableRowState::value),
            Some("figure")
        );
    }

    #[test]
    fn table_selection_ignores_disabled_rows() {
        let state = TableState::resolve(
            Size::Medium,
            "Search results",
            None,
            Some("missing"),
            sample_columns(),
            sample_rows(),
        );

        assert_eq!(state.focused_index(), Some(0));
        assert_eq!(
            state
                .activation_for_key("enter")
                .map(|selection| selection.value().to_owned()),
            Some("intro".to_owned())
        );
        assert_eq!(TableSelection::from_row(&state.rows()[1]), None);
    }

    #[test]
    fn table_alignment_labels_are_stable() {
        assert_eq!(TableColumnAlign::Start.as_str(), "start");
        assert_eq!(TableColumnAlign::Center.as_str(), "center");
        assert_eq!(TableColumnAlign::End.as_str(), "end");
    }
}
