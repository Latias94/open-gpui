//! Column descriptors, visibility, and pinning contracts for renderer-neutral tables.

use std::collections::{BTreeMap, BTreeSet};

use crate::geometry::{UiPx, ui_px};

use super::sizing::{TableColumnSizing, clamp_column_width, normalized_column_width};
use super::{TableCellEditor, TableColumnGroupId, TableColumnId, TableSelectOption};

/// Default preferred width for a table column.
pub const TABLE_DEFAULT_COLUMN_WIDTH: UiPx = ui_px(128.0);

/// Default minimum width for a table column.
pub const TABLE_MIN_COLUMN_WIDTH: UiPx = ui_px(40.0);

/// Default maximum width for a table column.
pub const TABLE_MAX_COLUMN_WIDTH: UiPx = ui_px(1_000_000.0);

/// Renderer-neutral column width policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TableColumnWidthPolicy {
    /// Keep the configured width unless committed sizing overrides it.
    #[default]
    Fixed,
    /// Let adapter-owned content-fit measurement widen the column.
    ContentFit,
}

impl TableColumnWidthPolicy {
    /// Returns a stable label.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Fixed => "fixed",
            Self::ContentFit => "content-fit",
        }
    }
}

/// Renderer-neutral column descriptor.
#[derive(Debug, Clone, PartialEq)]
pub struct TableColumn {
    id: TableColumnId,
    label: String,
    visible: bool,
    hideable: bool,
    sortable: bool,
    filterable: bool,
    global_filterable: bool,
    editor: Option<TableCellEditor>,
    select_options: Vec<TableSelectOption>,
    width_policy: TableColumnWidthPolicy,
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
            hideable: true,
            sortable: true,
            filterable: true,
            global_filterable: true,
            editor: None,
            select_options: Vec::new(),
            width_policy: TableColumnWidthPolicy::default(),
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

    /// Returns whether user-facing visibility controls may hide this column.
    pub const fn hideable(&self) -> bool {
        self.hideable
    }

    /// Returns whether this column accepts sorting.
    pub const fn sortable(&self) -> bool {
        self.sortable
    }

    /// Returns whether this column accepts filtering.
    pub const fn filterable(&self) -> bool {
        self.filterable
    }

    /// Returns whether this column participates in global filtering.
    pub const fn global_filterable(&self) -> bool {
        self.global_filterable
    }

    /// Returns the cell editor configured for this column, if any.
    pub const fn editor(&self) -> Option<TableCellEditor> {
        self.editor
    }

    /// Returns the fixed select options configured for this column.
    pub fn select_options(&self) -> &[TableSelectOption] {
        &self.select_options
    }

    /// Returns whether this column renders text-cell editors for editable leaf cells.
    pub const fn text_editable(&self) -> bool {
        self.editor.is_some()
    }

    /// Returns the configured width policy for this column.
    pub const fn width_policy(&self) -> TableColumnWidthPolicy {
        self.width_policy
    }

    /// Returns whether this column should widen from visible content.
    pub const fn is_content_fit(&self) -> bool {
        matches!(self.width_policy, TableColumnWidthPolicy::ContentFit)
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

    /// Applies user-facing hideability.
    pub const fn with_hideable(mut self, hideable: bool) -> Self {
        self.hideable = hideable;
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

    /// Applies global filtering capability.
    pub const fn with_global_filterable(mut self, global_filterable: bool) -> Self {
        self.global_filterable = global_filterable;
        self
    }

    /// Applies cell editor metadata.
    pub fn with_editor(mut self, editor: Option<TableCellEditor>) -> Self {
        self.editor = editor;
        if !matches!(editor, Some(TableCellEditor::Select)) {
            self.select_options = Vec::new();
        }
        self
    }

    /// Marks this column as content-fit sized.
    pub const fn with_content_fit(mut self) -> Self {
        self.width_policy = TableColumnWidthPolicy::ContentFit;
        self
    }

    /// Marks this column as fixed-width sized.
    pub const fn with_fixed_width(mut self) -> Self {
        self.width_policy = TableColumnWidthPolicy::Fixed;
        self
    }

    /// Enables or disables single-line text editing for leaf cells in this column.
    pub fn with_text_editable(mut self, editable: bool) -> Self {
        self.editor = if editable {
            Some(TableCellEditor::Text)
        } else {
            None
        };
        self.select_options = Vec::new();
        self
    }

    /// Enables fixed-row multiline text editing for leaf cells in this column.
    pub fn with_multiline_text_editor(mut self, rows: usize) -> Self {
        self.editor = Some(TableCellEditor::multiline(rows));
        self.select_options = Vec::new();
        self
    }

    /// Enables checkbox editing for leaf cells in this column.
    pub fn with_checkbox_editor(mut self) -> Self {
        self.editor = Some(TableCellEditor::checkbox());
        self.select_options = Vec::new();
        self
    }

    /// Enables fixed-option select editing for leaf cells in this column.
    pub fn with_select_editor(
        mut self,
        options: impl IntoIterator<Item = TableSelectOption>,
    ) -> Self {
        self.editor = Some(TableCellEditor::select());
        self.select_options = options.into_iter().collect();
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

/// One node in a renderer-neutral table column tree.
#[derive(Debug, Clone, PartialEq)]
pub enum TableColumnNode {
    /// Behavioral leaf column.
    Column(TableColumn),
    /// Visual group header with nested column nodes.
    Group(TableColumnGroup),
}

impl TableColumnNode {
    /// Creates a leaf column node.
    pub fn column(column: TableColumn) -> Self {
        Self::Column(column)
    }

    /// Creates a group node.
    pub fn group(group: TableColumnGroup) -> Self {
        Self::Group(group)
    }

    /// Returns the leaf column, when this node is a leaf.
    pub const fn as_column(&self) -> Option<&TableColumn> {
        match self {
            Self::Column(column) => Some(column),
            Self::Group(_) => None,
        }
    }

    /// Returns the column group, when this node is a group.
    pub const fn as_group(&self) -> Option<&TableColumnGroup> {
        match self {
            Self::Column(_) => None,
            Self::Group(group) => Some(group),
        }
    }

    /// Returns true when this node is a leaf column.
    pub const fn is_column(&self) -> bool {
        matches!(self, Self::Column(_))
    }

    /// Returns true when this node is a column group.
    pub const fn is_group(&self) -> bool {
        matches!(self, Self::Group(_))
    }
}

impl From<TableColumn> for TableColumnNode {
    fn from(value: TableColumn) -> Self {
        Self::Column(value)
    }
}

impl From<TableColumnGroup> for TableColumnNode {
    fn from(value: TableColumnGroup) -> Self {
        Self::Group(value)
    }
}

/// Renderer-neutral column group descriptor used by nested table headers.
#[derive(Debug, Clone, PartialEq)]
pub struct TableColumnGroup {
    id: TableColumnGroupId,
    label: String,
    children: Vec<TableColumnNode>,
}

impl TableColumnGroup {
    /// Creates a column group from stable id, label, and child column nodes.
    pub fn new<N>(
        id: impl Into<TableColumnGroupId>,
        label: impl Into<String>,
        children: impl IntoIterator<Item = N>,
    ) -> Self
    where
        N: Into<TableColumnNode>,
    {
        Self {
            id: id.into(),
            label: label.into(),
            children: children.into_iter().map(Into::into).collect(),
        }
    }

    /// Returns the stable group identity.
    pub const fn id(&self) -> &TableColumnGroupId {
        &self.id
    }

    /// Returns the visible group label.
    pub fn label(&self) -> &str {
        &self.label
    }

    /// Returns child column-tree nodes.
    pub fn children(&self) -> &[TableColumnNode] {
        &self.children
    }

    /// Adds one child column-tree node.
    pub fn with_child(mut self, child: impl Into<TableColumnNode>) -> Self {
        self.children.push(child.into());
        self
    }

    /// Adds child column-tree nodes.
    pub fn with_children<N>(mut self, children: impl IntoIterator<Item = N>) -> Self
    where
        N: Into<TableColumnNode>,
    {
        self.children.extend(children.into_iter().map(Into::into));
        self
    }

    fn with_normalized_children(mut self, children: Vec<TableColumnNode>) -> Self {
        self.children = children;
        self
    }
}

/// Caller-owned runtime column visibility overrides.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TableColumnVisibilityOverrides {
    overrides: BTreeMap<TableColumnId, bool>,
}

impl TableColumnVisibilityOverrides {
    /// Creates an empty visibility override map.
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates visibility state from explicit column overrides.
    pub fn from_overrides<I, C>(overrides: I) -> Self
    where
        I: IntoIterator<Item = (C, bool)>,
        C: Into<TableColumnId>,
    {
        let mut visibility = Self::default();
        for (column, visible) in overrides {
            visibility = visibility.with_visibility(column, visible);
        }
        visibility
    }

    /// Returns the runtime override for a column, if present.
    pub fn override_for(&self, column: &TableColumnId) -> Option<bool> {
        self.overrides.get(column).copied()
    }

    /// Returns the full override map.
    pub fn overrides(&self) -> &BTreeMap<TableColumnId, bool> {
        &self.overrides
    }

    /// Returns whether no runtime overrides exist.
    pub fn is_empty(&self) -> bool {
        self.overrides.is_empty()
    }

    /// Returns the number of runtime overrides.
    pub fn len(&self) -> usize {
        self.overrides.len()
    }

    /// Resolves effective visibility for a column descriptor.
    pub fn is_visible(&self, column: &TableColumn) -> bool {
        match self.override_for(column.id()) {
            Some(false) if !column.hideable() => column.visible(),
            Some(visible) => visible,
            None => column.visible(),
        }
    }

    /// Inserts or updates a runtime column visibility override.
    pub fn with_visibility(mut self, column: impl Into<TableColumnId>, visible: bool) -> Self {
        self.overrides.insert(column.into(), visible);
        self
    }

    /// Shows a column at runtime.
    pub fn show(self, column: impl Into<TableColumnId>) -> Self {
        self.with_visibility(column, true)
    }

    /// Hides a column at runtime.
    pub fn hide(self, column: impl Into<TableColumnId>) -> Self {
        self.with_visibility(column, false)
    }

    /// Removes the runtime override for a column.
    pub fn without(mut self, column: impl Into<TableColumnId>) -> Self {
        self.overrides.remove(&column.into());
        self
    }

    /// Removes all runtime overrides.
    pub fn clear(mut self) -> Self {
        self.overrides.clear();
        self
    }
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
    pub(super) fn from_visible_columns(
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

    pub(super) fn flattened(&self) -> Vec<TableColumn> {
        self.left
            .iter()
            .chain(self.center.iter())
            .chain(self.right.iter())
            .cloned()
            .collect()
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

pub(super) fn normalize_table_column_tree<N>(
    column_tree: impl IntoIterator<Item = N>,
) -> (Vec<TableColumnNode>, Vec<TableColumn>)
where
    N: Into<TableColumnNode>,
{
    let mut seen = BTreeSet::new();
    normalize_table_column_nodes(column_tree.into_iter().map(Into::into), &mut seen)
}

fn normalize_table_column_nodes(
    column_tree: impl IntoIterator<Item = TableColumnNode>,
    seen: &mut BTreeSet<TableColumnId>,
) -> (Vec<TableColumnNode>, Vec<TableColumn>) {
    let mut normalized_tree = Vec::new();
    let mut leaf_columns = Vec::new();

    for node in column_tree {
        match node {
            TableColumnNode::Column(column) => {
                if seen.insert(column.id().clone()) {
                    leaf_columns.push(column.clone());
                    normalized_tree.push(TableColumnNode::Column(column));
                }
            }
            TableColumnNode::Group(group) => {
                let (children, leaves) =
                    normalize_table_column_nodes(group.children().iter().cloned(), seen);
                if !children.is_empty() {
                    leaf_columns.extend(leaves);
                    normalized_tree.push(TableColumnNode::Group(
                        group.with_normalized_children(children),
                    ));
                }
            }
        }
    }

    (normalized_tree, leaf_columns)
}
