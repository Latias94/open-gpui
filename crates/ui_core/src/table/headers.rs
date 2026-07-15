//! Header-group resolution for renderer-neutral table column trees.

use std::collections::{BTreeMap, BTreeSet};

use super::columns::{TableColumn, TableColumnNode, TableColumnRegion, TableColumnRegions};
use super::identity::{
    TableColumnGroupId, TableColumnId, TableHeaderIdentity, TableHeaderRowIdentity,
    TableResolvedHeaderIdentity,
};

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
    identity: TableResolvedHeaderIdentity,
    region: TableColumnRegion,
    depth: usize,
    index: usize,
    label: String,
    row_span: usize,
    sub_header_identities: Vec<TableResolvedHeaderIdentity>,
}

impl TableResolvedHeaderCell {
    fn new(
        identity: TableResolvedHeaderIdentity,
        region: TableColumnRegion,
        depth: usize,
        index: usize,
        label: impl Into<String>,
        row_span: usize,
        sub_header_identities: Vec<TableResolvedHeaderIdentity>,
    ) -> Self {
        Self {
            identity,
            region,
            depth,
            index,
            label: label.into(),
            row_span,
            sub_header_identities,
        }
    }

    /// Returns this resolved header fragment's stable identity.
    pub const fn identity(&self) -> &TableResolvedHeaderIdentity {
        &self.identity
    }

    /// Returns the logical identity independent of pinning and fragmentation.
    pub const fn logical_identity(&self) -> &TableHeaderIdentity {
        self.identity.logical()
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
        match self.logical_identity() {
            TableHeaderIdentity::Group(_) => TableResolvedHeaderKind::Group,
            TableHeaderIdentity::Leaf(_) => TableResolvedHeaderKind::Leaf,
            TableHeaderIdentity::Placeholder { .. } => TableResolvedHeaderKind::Placeholder,
        }
    }

    /// Returns the visible label.
    pub fn label(&self) -> &str {
        &self.label
    }

    /// Returns the source leaf-column identity for leaf headers.
    pub const fn source_column_id(&self) -> Option<&TableColumnId> {
        match self.logical_identity() {
            TableHeaderIdentity::Leaf(column) => Some(column),
            TableHeaderIdentity::Group(_) | TableHeaderIdentity::Placeholder { .. } => None,
        }
    }

    /// Returns the source group path for structural group headers.
    pub fn source_group_path(&self) -> Option<&[TableColumnGroupId]> {
        self.logical_identity().group_path()
    }

    /// Returns whether this header is a placeholder.
    pub const fn is_placeholder(&self) -> bool {
        self.kind().is_placeholder()
    }

    /// Returns whether this header is a visible leaf column.
    pub const fn is_leaf(&self) -> bool {
        self.kind().is_leaf()
    }

    /// Returns whether this header is a structural group.
    pub const fn is_group(&self) -> bool {
        self.kind().is_group()
    }

    /// Returns the number of visible leaf columns covered by this cell.
    pub const fn col_span(&self) -> usize {
        self.identity.covered_leaf_count()
    }

    /// Returns the number of header rows spanned by this cell.
    pub const fn row_span(&self) -> usize {
        self.row_span
    }

    /// Returns the visible leaf column ids covered by this cell.
    pub fn leaf_column_ids(&self) -> &[TableColumnId] {
        self.identity.covered_leaves()
    }

    /// Returns direct child header identities.
    pub fn sub_header_identities(&self) -> &[TableResolvedHeaderIdentity] {
        &self.sub_header_identities
    }
}

/// Resolved header row metadata for one render region.
#[derive(Debug, Clone, PartialEq)]
pub struct TableResolvedHeaderGroup {
    region: TableColumnRegion,
    identity: TableHeaderRowIdentity,
    headers: Vec<TableResolvedHeaderCell>,
}

impl TableResolvedHeaderGroup {
    fn new(region: TableColumnRegion, depth: usize, headers: Vec<TableResolvedHeaderCell>) -> Self {
        Self {
            region,
            identity: TableHeaderRowIdentity::new(depth),
            headers,
        }
    }

    /// Returns the render region that owns this header row.
    pub const fn region(&self) -> TableColumnRegion {
        self.region
    }

    /// Returns the row depth.
    pub const fn depth(&self) -> usize {
        self.identity.depth()
    }

    /// Returns the logical header-row identity independent of pinned region.
    pub const fn identity(&self) -> TableHeaderRowIdentity {
        self.identity
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
    logical_identity: TableHeaderIdentity,
    label: String,
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
        .map(|(depth, headers)| TableResolvedHeaderGroup::new(region, depth, headers))
        .collect()
}

fn build_table_header_row(
    region: TableColumnRegion,
    depth: usize,
    max_depth: usize,
    leaf_paths: &[TableHeaderPath],
) -> Vec<TableResolvedHeaderCell> {
    let mut cells = Vec::new();
    let mut current_key: Option<TableHeaderIdentity> = None;
    let mut current_label = String::new();
    let mut current_leaf_ids = Vec::new();

    let flush = |cells: &mut Vec<TableResolvedHeaderCell>,
                 key: &mut Option<TableHeaderIdentity>,
                 label: &mut String,
                 leaf_ids: &mut Vec<TableColumnId>| {
        let Some(logical_identity) = key.take() else {
            return;
        };
        if leaf_ids.is_empty() {
            *label = String::new();
            return;
        }
        let leaf_header = matches!(logical_identity, TableHeaderIdentity::Leaf(_));
        let leaf_column_ids = std::mem::take(leaf_ids);
        let identity = TableResolvedHeaderIdentity::new(logical_identity, leaf_column_ids);
        cells.push(TableResolvedHeaderCell::new(
            identity,
            region,
            depth,
            cells.len(),
            label.clone(),
            if leaf_header {
                max_depth.saturating_sub(depth)
            } else {
                1
            },
            Vec::new(),
        ));
        *label = String::new();
    };

    for path in leaf_paths {
        let slot = header_slot_for_path(depth, path);
        let slot_key = slot.logical_identity.clone();

        let should_flush = current_key.as_ref() != Some(&slot_key);
        if should_flush {
            flush(
                &mut cells,
                &mut current_key,
                &mut current_label,
                &mut current_leaf_ids,
            );
            current_key = Some(slot_key);
            current_label = slot.label;
        }

        current_leaf_ids.push(slot.leaf_id);
    }

    flush(
        &mut cells,
        &mut current_key,
        &mut current_label,
        &mut current_leaf_ids,
    );

    cells
}

fn header_slot_for_path(depth: usize, path: &TableHeaderPath) -> TableHeaderSlot {
    let leaf_depth = path.nodes.len().saturating_sub(1);

    if depth == leaf_depth {
        let leaf = path
            .nodes
            .last()
            .and_then(TableColumnNode::as_column)
            .expect("leaf paths should end in a column node");
        return TableHeaderSlot {
            logical_identity: TableHeaderIdentity::Leaf(leaf.id().clone()),
            label: leaf.label().to_owned(),
            leaf_id: leaf.id().clone(),
        };
    }

    if let Some(node) = path.nodes.get(depth) {
        match node {
            TableColumnNode::Column(column) => TableHeaderSlot {
                logical_identity: TableHeaderIdentity::Leaf(column.id().clone()),
                label: column.label().to_owned(),
                leaf_id: path.leaf_id.clone(),
            },
            TableColumnNode::Group(group) => TableHeaderSlot {
                logical_identity: TableHeaderIdentity::Group(
                    path.nodes[..=depth]
                        .iter()
                        .filter_map(TableColumnNode::as_group)
                        .map(|group| group.id().clone())
                        .collect(),
                ),
                label: group.label().to_owned(),
                leaf_id: path.leaf_id.clone(),
            },
        }
    } else {
        TableHeaderSlot {
            logical_identity: TableHeaderIdentity::Placeholder {
                leaf_column: path.leaf_id.clone(),
                depth,
            },
            label: String::new(),
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
            .map(|cell| (cell.identity().clone(), cell.leaf_column_ids().to_vec()))
            .collect::<Vec<_>>();

        for cell in &mut rows[depth] {
            let parent_leaves = cell.leaf_column_ids().to_vec();
            let parent_leaves = parent_leaves
                .into_iter()
                .collect::<BTreeSet<TableColumnId>>();
            let child_identities = next
                .iter()
                .filter(|(_, child_leaves)| {
                    child_leaves
                        .iter()
                        .any(|leaf_id| parent_leaves.contains(leaf_id))
                })
                .map(|(child_identity, _)| child_identity.clone())
                .collect::<Vec<_>>();
            cell.sub_header_identities = child_identities;
        }
    }
}
