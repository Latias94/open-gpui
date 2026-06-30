//! Header-group resolution for renderer-neutral table column trees.

use std::collections::{BTreeMap, BTreeSet};

use super::columns::{TableColumn, TableColumnNode, TableColumnRegion, TableColumnRegions};
use super::identity::TableColumnId;

/// Resolved header cell kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TableResolvedHeaderKind {
    /// A structural group header.
    Group,
    /// A visible leaf column header.
    Leaf,
    /// A structural placeholder used to keep rows aligned.
    Placeholder,
}

impl TableResolvedHeaderKind {
    /// Returns whether this kind is a placeholder.
    pub const fn is_placeholder(self) -> bool {
        matches!(self, Self::Placeholder)
    }

    /// Returns whether this kind is a leaf header.
    pub const fn is_leaf(self) -> bool {
        matches!(self, Self::Leaf)
    }

    /// Returns whether this kind is a group header.
    pub const fn is_group(self) -> bool {
        matches!(self, Self::Group)
    }
}

/// Resolved one header cell in a header row.
#[derive(Debug, Clone, PartialEq)]
pub struct TableResolvedHeaderCell {
    id: String,
    region: TableColumnRegion,
    depth: usize,
    index: usize,
    kind: TableResolvedHeaderKind,
    label: String,
    source_id: String,
    placeholder_id: Option<String>,
    col_span: usize,
    row_span: usize,
    leaf_column_ids: Vec<TableColumnId>,
    sub_header_ids: Vec<String>,
}

impl TableResolvedHeaderCell {
    fn new(
        id: impl Into<String>,
        region: TableColumnRegion,
        depth: usize,
        index: usize,
        kind: TableResolvedHeaderKind,
        label: impl Into<String>,
        source_id: impl Into<String>,
        placeholder_id: Option<impl Into<String>>,
        col_span: usize,
        row_span: usize,
        leaf_column_ids: Vec<TableColumnId>,
        sub_header_ids: Vec<String>,
    ) -> Self {
        Self {
            id: id.into(),
            region,
            depth,
            index,
            kind,
            label: label.into(),
            source_id: source_id.into(),
            placeholder_id: placeholder_id.map(Into::into),
            col_span,
            row_span,
            leaf_column_ids,
            sub_header_ids,
        }
    }

    /// Returns the stable header identity.
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Returns the render region that owns this header cell.
    pub const fn region(&self) -> TableColumnRegion {
        self.region
    }

    /// Returns the row depth for this header cell.
    pub const fn depth(&self) -> usize {
        self.depth
    }

    /// Returns the index within the header row.
    pub const fn index(&self) -> usize {
        self.index
    }

    /// Returns the header cell kind.
    pub const fn kind(&self) -> TableResolvedHeaderKind {
        self.kind
    }

    /// Returns the visible label.
    pub fn label(&self) -> &str {
        &self.label
    }

    /// Returns the stable source identity behind this header cell.
    pub fn source_id(&self) -> &str {
        &self.source_id
    }

    /// Returns whether this header is a placeholder.
    pub const fn is_placeholder(&self) -> bool {
        self.kind.is_placeholder()
    }

    /// Returns whether this header is a visible leaf column.
    pub const fn is_leaf(&self) -> bool {
        self.kind.is_leaf()
    }

    /// Returns whether this header is a structural group.
    pub const fn is_group(&self) -> bool {
        self.kind.is_group()
    }

    /// Returns the placeholder id, when this cell is a placeholder.
    pub fn placeholder_id(&self) -> Option<&str> {
        self.placeholder_id.as_deref()
    }

    /// Returns the number of visible leaf columns covered by this cell.
    pub const fn col_span(&self) -> usize {
        self.col_span
    }

    /// Returns the number of header rows spanned by this cell.
    pub const fn row_span(&self) -> usize {
        self.row_span
    }

    /// Returns the visible leaf column ids covered by this cell.
    pub fn leaf_column_ids(&self) -> &[TableColumnId] {
        &self.leaf_column_ids
    }

    /// Returns direct child header ids.
    pub fn sub_header_ids(&self) -> &[String] {
        &self.sub_header_ids
    }
}

/// Resolved header row metadata for one render region.
#[derive(Debug, Clone, PartialEq)]
pub struct TableResolvedHeaderGroup {
    region: TableColumnRegion,
    depth: usize,
    id: String,
    headers: Vec<TableResolvedHeaderCell>,
}

impl TableResolvedHeaderGroup {
    fn new(
        region: TableColumnRegion,
        depth: usize,
        id: impl Into<String>,
        headers: Vec<TableResolvedHeaderCell>,
    ) -> Self {
        Self {
            region,
            depth,
            id: id.into(),
            headers,
        }
    }

    /// Returns the render region that owns this header row.
    pub const fn region(&self) -> TableColumnRegion {
        self.region
    }

    /// Returns the row depth.
    pub const fn depth(&self) -> usize {
        self.depth
    }

    /// Returns the stable header-row identity.
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Returns header cells in this row.
    pub fn headers(&self) -> &[TableResolvedHeaderCell] {
        &self.headers
    }
}

/// Resolved header rows split into render regions.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct TableResolvedHeaderGroupRegions {
    left: Vec<TableResolvedHeaderGroup>,
    center: Vec<TableResolvedHeaderGroup>,
    right: Vec<TableResolvedHeaderGroup>,
}

impl TableResolvedHeaderGroupRegions {
    pub(super) fn from_column_tree(
        column_tree: &[TableColumnNode],
        regions: &TableColumnRegions,
    ) -> Self {
        Self {
            left: resolve_table_header_groups_for_region(
                column_tree,
                TableColumnRegion::Left,
                regions.left(),
            ),
            center: resolve_table_header_groups_for_region(
                column_tree,
                TableColumnRegion::Center,
                regions.center(),
            ),
            right: resolve_table_header_groups_for_region(
                column_tree,
                TableColumnRegion::Right,
                regions.right(),
            ),
        }
    }

    /// Returns visible left-pinned header rows.
    pub fn left(&self) -> &[TableResolvedHeaderGroup] {
        &self.left
    }

    /// Returns visible unpinned center header rows.
    pub fn center(&self) -> &[TableResolvedHeaderGroup] {
        &self.center
    }

    /// Returns visible right-pinned header rows.
    pub fn right(&self) -> &[TableResolvedHeaderGroup] {
        &self.right
    }

    /// Returns header rows for a region.
    pub fn region(&self, region: TableColumnRegion) -> &[TableResolvedHeaderGroup] {
        match region {
            TableColumnRegion::Left => self.left(),
            TableColumnRegion::Center => self.center(),
            TableColumnRegion::Right => self.right(),
        }
    }

    /// Returns all header rows in render order.
    pub fn all(&self) -> impl Iterator<Item = &TableResolvedHeaderGroup> {
        self.left
            .iter()
            .chain(self.center.iter())
            .chain(self.right.iter())
    }

    /// Returns the total number of header rows across all regions.
    pub fn len(&self) -> usize {
        self.left.len() + self.center.len() + self.right.len()
    }

    /// Returns true when all regions are empty.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

#[derive(Debug, Clone)]
struct TableHeaderPath {
    leaf_id: TableColumnId,
    nodes: Vec<TableColumnNode>,
}

#[derive(Debug, Clone)]
struct TableHeaderSlot {
    kind: TableResolvedHeaderKind,
    source_id: String,
    label: String,
    placeholder_id: Option<String>,
    leaf_id: TableColumnId,
}

fn resolve_table_header_groups_for_region(
    column_tree: &[TableColumnNode],
    region: TableColumnRegion,
    visible_columns: &[TableColumn],
) -> Vec<TableResolvedHeaderGroup> {
    if visible_columns.is_empty() {
        return Vec::new();
    }

    let leaf_paths = collect_table_leaf_paths(column_tree);
    let visible_leaf_paths = visible_columns
        .iter()
        .filter_map(|column| {
            leaf_paths.get(column.id()).map(|path| TableHeaderPath {
                leaf_id: column.id().clone(),
                nodes: path.clone(),
            })
        })
        .collect::<Vec<_>>();

    if visible_leaf_paths.is_empty() {
        return Vec::new();
    }

    let max_depth = visible_leaf_paths
        .iter()
        .map(|path| path.nodes.len())
        .max()
        .unwrap_or(0);
    if max_depth == 0 {
        return Vec::new();
    }

    let mut rows = (0..max_depth)
        .map(|depth| build_table_header_row(region, depth, max_depth, &visible_leaf_paths))
        .collect::<Vec<_>>();
    attach_table_header_children(&mut rows);

    rows.into_iter()
        .enumerate()
        .filter(|(_, headers)| !headers.is_empty())
        .map(|(depth, headers)| {
            TableResolvedHeaderGroup::new(
                region,
                depth,
                format!("table:{}:header:{}", region.as_str(), depth),
                headers,
            )
        })
        .collect()
}

fn build_table_header_row(
    region: TableColumnRegion,
    depth: usize,
    max_depth: usize,
    leaf_paths: &[TableHeaderPath],
) -> Vec<TableResolvedHeaderCell> {
    let mut cells = Vec::new();
    let mut current_key: Option<String> = None;
    let mut current_kind = TableResolvedHeaderKind::Placeholder;
    let mut current_source_id = String::new();
    let mut current_label = String::new();
    let mut current_placeholder_id = None;
    let mut current_leaf_ids = Vec::new();

    let flush = |cells: &mut Vec<TableResolvedHeaderCell>,
                 key: &mut Option<String>,
                 kind: &mut TableResolvedHeaderKind,
                 source_id: &mut String,
                 label: &mut String,
                 placeholder_id: &mut Option<String>,
                 leaf_ids: &mut Vec<TableColumnId>| {
        let Some(key_value) = key.take() else {
            return;
        };
        let col_span = leaf_ids.len();
        if col_span == 0 {
            *kind = TableResolvedHeaderKind::Placeholder;
            *source_id = String::new();
            *label = String::new();
            *placeholder_id = None;
            return;
        }
        let first_leaf_id = leaf_ids
            .first()
            .expect("header segment should cover at least one leaf")
            .clone();
        let id = if kind.is_placeholder() {
            placeholder_id
                .as_ref()
                .cloned()
                .unwrap_or_else(|| key_value.clone())
        } else if kind.is_leaf() && col_span == 1 {
            source_id.clone()
        } else {
            format!(
                "table:{}:header:{}:{}:{}",
                region.as_str(),
                depth,
                source_id,
                first_leaf_id.as_str()
            )
        };
        let leaf_column_ids = std::mem::take(leaf_ids);
        cells.push(TableResolvedHeaderCell::new(
            id,
            region,
            depth,
            cells.len(),
            *kind,
            label.clone(),
            source_id.clone(),
            placeholder_id.clone(),
            col_span,
            if kind.is_leaf() {
                max_depth.saturating_sub(depth)
            } else {
                1
            },
            leaf_column_ids,
            Vec::new(),
        ));
        *kind = TableResolvedHeaderKind::Placeholder;
        *source_id = String::new();
        *label = String::new();
        *placeholder_id = None;
    };

    for path in leaf_paths {
        let slot = header_slot_for_path(region, depth, path);
        let slot_key = if slot.kind.is_placeholder() {
            slot.placeholder_id
                .clone()
                .expect("placeholder headers always carry a placeholder id")
        } else {
            slot.source_id.clone()
        };

        let should_flush = current_key.as_ref() != Some(&slot_key);
        if should_flush {
            flush(
                &mut cells,
                &mut current_key,
                &mut current_kind,
                &mut current_source_id,
                &mut current_label,
                &mut current_placeholder_id,
                &mut current_leaf_ids,
            );
            current_key = Some(slot_key);
            current_kind = slot.kind;
            current_source_id = slot.source_id;
            current_label = slot.label;
            current_placeholder_id = slot.placeholder_id;
        }

        current_leaf_ids.push(slot.leaf_id);
    }

    flush(
        &mut cells,
        &mut current_key,
        &mut current_kind,
        &mut current_source_id,
        &mut current_label,
        &mut current_placeholder_id,
        &mut current_leaf_ids,
    );

    cells
}

fn header_slot_for_path(
    region: TableColumnRegion,
    depth: usize,
    path: &TableHeaderPath,
) -> TableHeaderSlot {
    let leaf_depth = path.nodes.len().saturating_sub(1);

    if depth == leaf_depth {
        let leaf = path
            .nodes
            .last()
            .and_then(TableColumnNode::as_column)
            .expect("leaf paths should end in a column node");
        return TableHeaderSlot {
            kind: TableResolvedHeaderKind::Leaf,
            source_id: leaf.id().as_str().to_owned(),
            label: leaf.label().to_owned(),
            placeholder_id: None,
            leaf_id: leaf.id().clone(),
        };
    }

    if let Some(node) = path.nodes.get(depth) {
        match node {
            TableColumnNode::Column(column) => TableHeaderSlot {
                kind: TableResolvedHeaderKind::Leaf,
                source_id: column.id().as_str().to_owned(),
                label: column.label().to_owned(),
                placeholder_id: None,
                leaf_id: path.leaf_id.clone(),
            },
            TableColumnNode::Group(group) => TableHeaderSlot {
                kind: TableResolvedHeaderKind::Group,
                source_id: group.id().as_str().to_owned(),
                label: group.label().to_owned(),
                placeholder_id: None,
                leaf_id: path.leaf_id.clone(),
            },
        }
    } else {
        TableHeaderSlot {
            kind: TableResolvedHeaderKind::Placeholder,
            source_id: path.leaf_id.as_str().to_owned(),
            label: String::new(),
            placeholder_id: Some(format!(
                "table:{}:placeholder:{}:{}",
                region.as_str(),
                depth,
                path.leaf_id.as_str()
            )),
            leaf_id: path.leaf_id.clone(),
        }
    }
}

fn collect_table_leaf_paths(
    nodes: &[TableColumnNode],
) -> BTreeMap<TableColumnId, Vec<TableColumnNode>> {
    let mut out = BTreeMap::new();
    collect_table_leaf_paths_inner(nodes, &mut Vec::new(), &mut out);
    out
}

fn collect_table_leaf_paths_inner(
    nodes: &[TableColumnNode],
    path: &mut Vec<TableColumnNode>,
    out: &mut BTreeMap<TableColumnId, Vec<TableColumnNode>>,
) {
    for node in nodes {
        path.push(node.clone());
        match node {
            TableColumnNode::Column(column) => {
                out.insert(column.id().clone(), path.clone());
            }
            TableColumnNode::Group(group) => {
                collect_table_leaf_paths_inner(group.children(), path, out);
            }
        }
        path.pop();
    }
}

fn attach_table_header_children(rows: &mut [Vec<TableResolvedHeaderCell>]) {
    if rows.len() < 2 {
        return;
    }

    for depth in 0..rows.len() - 1 {
        let next = rows[depth + 1]
            .iter()
            .map(|cell| (cell.id().to_owned(), cell.leaf_column_ids().to_vec()))
            .collect::<Vec<_>>();

        for cell in &mut rows[depth] {
            let parent_leaves = cell.leaf_column_ids().to_vec();
            let parent_leaves = parent_leaves
                .into_iter()
                .collect::<BTreeSet<TableColumnId>>();
            let child_ids = next
                .iter()
                .filter(|(_, child_leaves)| {
                    child_leaves
                        .iter()
                        .any(|leaf_id| parent_leaves.contains(leaf_id))
                })
                .map(|(child_id, _)| child_id.clone())
                .collect::<Vec<_>>();
            cell.sub_header_ids = child_ids;
        }
    }
}
