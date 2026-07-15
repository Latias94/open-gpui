//! Stable table identities shared by core and component layers.

use std::fmt::Write as _;
use std::sync::Arc;

use super::TableCellValue;

/// Caller-owned business identity for a table row.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TableRowId(Arc<str>);

impl TableRowId {
    /// Creates a row identity from a stable string.
    pub fn new(id: impl Into<String>) -> Self {
        Self(Arc::from(id.into()))
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

/// Caller-owned source-instance identity used when business row ids are not unique.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TableRowInstanceId(Arc<str>);

impl TableRowInstanceId {
    /// Creates a stable source-instance identity.
    pub fn new(id: impl Into<String>) -> Self {
        Self(Arc::from(id.into()))
    }

    /// Returns the caller-owned string value.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<&str> for TableRowInstanceId {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

impl From<String> for TableRowInstanceId {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

/// Structured diagnostic emitted when source rows cannot provide unique identity facts.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TableRowIdentityDiagnostic {
    /// More than one source row uses the same caller-owned business row id.
    DuplicateRowId {
        /// The duplicated business row id.
        row_id: TableRowId,
        /// Number of source rows using the id.
        occurrences: usize,
    },
    /// More than one source row uses the same explicit instance id within one business row id.
    ///
    /// Colliding rows fall back to preorder occurrence identities so they remain distinct in the
    /// current snapshot. Callers that require stability across reorder must provide unique
    /// instance ids.
    DuplicateSourceInstance {
        /// The business row id that scopes the instance id.
        row_id: TableRowId,
        /// The duplicated caller-owned source-instance id.
        instance_id: TableRowInstanceId,
        /// Number of source rows using the scoped instance id.
        occurrences: usize,
    },
}

/// Resolved disambiguator for one source row instance.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TableSourceInstanceIdentity {
    /// The business row id is unique in the source snapshot.
    Unique,
    /// The caller supplied a stable source-instance identity.
    Explicit(TableRowInstanceId),
    /// Duplicate business ids were disambiguated within one source snapshot.
    Occurrence(TableRowOccurrenceIdentity),
}

/// Snapshot-scoped fallback identity for one duplicate source-row occurrence.
///
/// Values are produced by [`super::TableState::source_row_identity_at`]. Callers that retain row
/// identity across source replacement or reorder must provide a [`TableRowInstanceId`] instead.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TableRowOccurrenceIdentity {
    source_snapshot: u64,
    occurrence: usize,
}

impl TableRowOccurrenceIdentity {
    pub(super) const fn new(source_snapshot: u64, occurrence: usize) -> Self {
        Self {
            source_snapshot,
            occurrence,
        }
    }

    /// Returns the opaque source-snapshot token used to reject stale retained identities.
    pub const fn source_snapshot(&self) -> u64 {
        self.source_snapshot
    }

    /// Returns the zero-based preorder occurrence within the business row id.
    pub const fn occurrence(&self) -> usize {
        self.occurrence
    }
}

/// Resolved identity for one source-backed row.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TableSourceRowIdentity {
    row_id: TableRowId,
    instance: TableSourceInstanceIdentity,
}

impl TableSourceRowIdentity {
    /// Creates an identity for a source row whose business id is unique.
    pub fn unique(row_id: impl Into<TableRowId>) -> Self {
        Self {
            row_id: row_id.into(),
            instance: TableSourceInstanceIdentity::Unique,
        }
    }

    /// Creates an identity using a caller-owned source-instance id.
    pub fn explicit(
        row_id: impl Into<TableRowId>,
        instance_id: impl Into<TableRowInstanceId>,
    ) -> Self {
        Self {
            row_id: row_id.into(),
            instance: TableSourceInstanceIdentity::Explicit(instance_id.into()),
        }
    }

    pub(super) fn occurrence(
        row_id: impl Into<TableRowId>,
        source_snapshot: u64,
        occurrence: usize,
    ) -> Self {
        Self {
            row_id: row_id.into(),
            instance: TableSourceInstanceIdentity::Occurrence(TableRowOccurrenceIdentity::new(
                source_snapshot,
                occurrence,
            )),
        }
    }

    /// Returns the caller-owned business row id.
    pub const fn row_id(&self) -> &TableRowId {
        &self.row_id
    }

    /// Returns the resolved source-instance disambiguator.
    pub const fn instance(&self) -> &TableSourceInstanceIdentity {
        &self.instance
    }
}

/// Canonical identity for one numeric grouping value.
///
/// Raw IEEE-754 bits cannot bypass NaN and signed-zero normalization:
///
/// ```compile_fail
/// use open_gpui_ui_core::TableGroupValueIdentity;
///
/// let _ = TableGroupValueIdentity::Number((-0.0_f64).to_bits());
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TableGroupNumberIdentity(u64);

impl TableGroupNumberIdentity {
    /// Creates a canonical numeric group identity.
    pub fn new(value: f64) -> Self {
        let value = if value.is_nan() {
            f64::NAN
        } else if value == 0.0 {
            0.0
        } else {
            value
        };
        Self(value.to_bits())
    }

    /// Returns the normalized numeric value.
    pub fn value(self) -> f64 {
        f64::from_bits(self.0)
    }

    const fn bits(self) -> u64 {
        self.0
    }
}

impl From<f64> for TableGroupNumberIdentity {
    fn from(value: f64) -> Self {
        Self::new(value)
    }
}

/// Typed grouping value used in synthetic group identities.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TableGroupValueIdentity {
    /// No meaningful value is present.
    Empty,
    /// Text grouping value.
    Text(String),
    /// Numeric grouping value represented by normalized IEEE-754 bits.
    Number(TableGroupNumberIdentity),
    /// Boolean grouping value.
    Bool(bool),
}

impl TableGroupValueIdentity {
    pub(super) fn from_cell_value(value: &TableCellValue) -> Self {
        match value {
            TableCellValue::Empty => Self::Empty,
            TableCellValue::Text(value) => Self::Text(value.clone()),
            TableCellValue::Number(value) => Self::Number((*value).into()),
            TableCellValue::Bool(value) => Self::Bool(*value),
        }
    }

    /// Returns a human-readable value for diagnostics, not identity serialization.
    pub fn debug_text(&self) -> String {
        match self {
            Self::Empty => String::new(),
            Self::Text(value) => value.clone(),
            Self::Number(number) => number.value().to_string(),
            Self::Bool(value) => value.to_string(),
        }
    }
}

impl From<&str> for TableGroupValueIdentity {
    fn from(value: &str) -> Self {
        Self::Text(value.to_owned())
    }
}

impl From<String> for TableGroupValueIdentity {
    fn from(value: String) -> Self {
        Self::Text(value)
    }
}

impl From<f64> for TableGroupValueIdentity {
    fn from(value: f64) -> Self {
        Self::Number(value.into())
    }
}

impl From<usize> for TableGroupValueIdentity {
    fn from(value: usize) -> Self {
        Self::from(value as f64)
    }
}

impl From<bool> for TableGroupValueIdentity {
    fn from(value: bool) -> Self {
        Self::Bool(value)
    }
}

/// One typed segment in a synthetic group-row path.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TableGroupRowSegment {
    column_id: TableColumnId,
    value: TableGroupValueIdentity,
}

impl TableGroupRowSegment {
    /// Creates a grouping path segment.
    pub fn new(
        column_id: impl Into<TableColumnId>,
        value: impl Into<TableGroupValueIdentity>,
    ) -> Self {
        Self {
            column_id: column_id.into(),
            value: value.into(),
        }
    }

    /// Returns the grouped column identity.
    pub const fn column_id(&self) -> &TableColumnId {
        &self.column_id
    }

    /// Returns the typed grouping value identity.
    pub const fn value(&self) -> &TableGroupValueIdentity {
        &self.value
    }
}

/// Stable typed identity for a synthetic group row.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TableGroupRowIdentity(Arc<[TableGroupRowSegment]>);

impl TableGroupRowIdentity {
    /// Creates a root group-row identity.
    pub fn new(
        column_id: impl Into<TableColumnId>,
        value: impl Into<TableGroupValueIdentity>,
    ) -> Self {
        Self(vec![TableGroupRowSegment::new(column_id, value)].into())
    }

    /// Appends one nested grouping segment.
    pub fn child(
        mut self,
        column_id: impl Into<TableColumnId>,
        value: impl Into<TableGroupValueIdentity>,
    ) -> Self {
        let mut segments = self.0.to_vec();
        segments.push(TableGroupRowSegment::new(column_id, value));
        self.0 = segments.into();
        self
    }

    pub(super) fn child_cell_value(
        mut self,
        column_id: TableColumnId,
        value: &TableCellValue,
    ) -> Self {
        let mut segments = self.0.to_vec();
        segments.push(TableGroupRowSegment::new(
            column_id,
            TableGroupValueIdentity::from_cell_value(value),
        ));
        self.0 = segments.into();
        self
    }

    pub(super) fn from_cell_value(column_id: TableColumnId, value: &TableCellValue) -> Self {
        Self(
            vec![TableGroupRowSegment::new(
                column_id,
                TableGroupValueIdentity::from_cell_value(value),
            )]
            .into(),
        )
    }

    /// Returns the outer-to-inner grouping path.
    pub fn segments(&self) -> &[TableGroupRowSegment] {
        &self.0
    }
}

/// Authoritative identity for a resolved source or synthetic table row.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TableRowIdentity {
    /// A source-backed row instance.
    Source(TableSourceRowIdentity),
    /// A synthetic grouping row.
    Group(TableGroupRowIdentity),
}

/// Canonical encoded key for one exact logical row identity.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TableRowIdentityKey(Arc<str>);

impl TableRowIdentityKey {
    /// Returns the collision-free, versioned identity encoding.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TableRowIdentity {
    /// Creates an identity for a source row whose business id is unique.
    pub fn source(row_id: impl Into<TableRowId>) -> Self {
        Self::Source(TableSourceRowIdentity::unique(row_id))
    }

    /// Creates a source identity with a caller-owned instance id.
    pub fn source_instance(
        row_id: impl Into<TableRowId>,
        instance_id: impl Into<TableRowInstanceId>,
    ) -> Self {
        Self::Source(TableSourceRowIdentity::explicit(row_id, instance_id))
    }

    /// Creates a synthetic group-row identity.
    pub fn group(group: TableGroupRowIdentity) -> Self {
        Self::Group(group)
    }

    /// Returns source identity metadata when this row is source-backed.
    pub const fn source_identity(&self) -> Option<&TableSourceRowIdentity> {
        match self {
            Self::Source(source) => Some(source),
            Self::Group(_) => None,
        }
    }

    /// Returns the caller-owned business row id for source-backed rows.
    pub const fn source_row_id(&self) -> Option<&TableRowId> {
        match self {
            Self::Source(source) => Some(source.row_id()),
            Self::Group(_) => None,
        }
    }

    /// Returns group identity metadata for synthetic rows.
    pub const fn group_identity(&self) -> Option<&TableGroupRowIdentity> {
        match self {
            Self::Source(_) => None,
            Self::Group(group) => Some(group),
        }
    }

    /// Returns a human-readable label for diagnostics, never for identity comparison.
    pub fn debug_label(&self) -> String {
        match self {
            Self::Source(source) => source.row_id().as_str().to_owned(),
            Self::Group(group) => {
                let path = group
                    .segments()
                    .iter()
                    .map(|segment| {
                        format!(
                            "{}={}",
                            segment.column_id().as_str(),
                            segment.value().debug_text()
                        )
                    })
                    .collect::<Vec<_>>()
                    .join(">");
                format!("group:{path}")
            }
        }
    }

    /// Encodes this identity for renderer keys, snapshots, and diagnostic selectors.
    pub fn key(&self) -> TableRowIdentityKey {
        TableRowIdentityKey(Arc::from(encode_table_row_identity(self)))
    }
}

fn encode_table_row_identity(identity: &TableRowIdentity) -> String {
    let mut encoded = String::from("tr2");
    match identity {
        TableRowIdentity::Source(source) => {
            encoded.push('s');
            push_identity_text(&mut encoded, source.row_id().as_str());
            match source.instance() {
                TableSourceInstanceIdentity::Unique => encoded.push('u'),
                TableSourceInstanceIdentity::Explicit(instance) => {
                    encoded.push('e');
                    push_identity_text(&mut encoded, instance.as_str());
                }
                TableSourceInstanceIdentity::Occurrence(occurrence) => {
                    encoded.push('o');
                    let _ = write!(
                        encoded,
                        "{};{};",
                        occurrence.source_snapshot(),
                        occurrence.occurrence()
                    );
                }
            }
        }
        TableRowIdentity::Group(group) => {
            encoded.push('g');
            let _ = write!(encoded, "{};", group.segments().len());
            for segment in group.segments() {
                push_identity_text(&mut encoded, segment.column_id().as_str());
                push_group_value_identity(&mut encoded, segment.value());
            }
        }
    }
    encoded
}

fn push_group_value_identity(encoded: &mut String, value: &TableGroupValueIdentity) {
    match value {
        TableGroupValueIdentity::Empty => encoded.push('e'),
        TableGroupValueIdentity::Text(value) => {
            encoded.push('t');
            push_identity_text(encoded, value);
        }
        TableGroupValueIdentity::Number(number) => {
            encoded.push('n');
            let _ = write!(encoded, "{:016x};", number.bits());
        }
        TableGroupValueIdentity::Bool(value) => {
            encoded.push('b');
            encoded.push(if *value { '1' } else { '0' });
        }
    }
}

fn push_identity_text(encoded: &mut String, value: &str) {
    let _ = write!(encoded, "{}:", value.len());
    encoded.push_str(value);
}

/// Stable renderer-neutral identity for a table column.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TableColumnId(Arc<str>);

impl TableColumnId {
    /// Creates a column identity from a stable string.
    pub fn new(id: impl Into<String>) -> Self {
        Self(Arc::from(id.into()))
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

/// Stable renderer-neutral identity for a table column group.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TableColumnGroupId(Arc<str>);

impl TableColumnGroupId {
    /// Creates a column-group identity from a stable string.
    pub fn new(id: impl Into<String>) -> Self {
        Self(Arc::from(id.into()))
    }

    /// Returns the stable string value.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<&str> for TableColumnGroupId {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

impl From<String> for TableColumnGroupId {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

/// Logical identity for a table header independent of its pinned render region.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TableHeaderIdentity {
    /// A visible leaf-column header.
    Leaf(TableColumnId),
    /// A structural column-group header identified by its full group path.
    Group(Vec<TableColumnGroupId>),
    /// A structural placeholder for a shallow leaf at one header depth.
    Placeholder {
        /// The leaf column covered by the placeholder.
        leaf_column: TableColumnId,
        /// The zero-based header depth.
        depth: usize,
    },
}

impl TableHeaderIdentity {
    /// Returns the source leaf column for leaf and placeholder identities.
    pub const fn leaf_column(&self) -> Option<&TableColumnId> {
        match self {
            Self::Leaf(column)
            | Self::Placeholder {
                leaf_column: column,
                ..
            } => Some(column),
            Self::Group(_) => None,
        }
    }

    /// Returns the source group path for structural group identities.
    pub fn group_path(&self) -> Option<&[TableColumnGroupId]> {
        match self {
            Self::Group(path) => Some(path),
            Self::Leaf(_) | Self::Placeholder { .. } => None,
        }
    }
}

/// Identity for one resolved header fragment.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TableResolvedHeaderIdentity {
    logical: TableHeaderIdentity,
    covered_leaves: Vec<TableColumnId>,
}

impl TableResolvedHeaderIdentity {
    /// Creates a leaf-header identity covering exactly its logical column.
    pub fn leaf(column_id: impl Into<TableColumnId>) -> Self {
        let column_id = column_id.into();
        Self {
            logical: TableHeaderIdentity::Leaf(column_id.clone()),
            covered_leaves: vec![column_id],
        }
    }

    pub(super) fn new(
        logical: TableHeaderIdentity,
        covered_leaves: impl IntoIterator<Item = TableColumnId>,
    ) -> Self {
        Self {
            logical,
            covered_leaves: covered_leaves.into_iter().collect(),
        }
    }

    /// Returns the logical header identity.
    pub const fn logical(&self) -> &TableHeaderIdentity {
        &self.logical
    }

    /// Returns the ordered visible leaves covered by this fragment.
    pub fn covered_leaves(&self) -> &[TableColumnId] {
        &self.covered_leaves
    }

    pub(super) const fn covered_leaf_count(&self) -> usize {
        self.covered_leaves.len()
    }
}

/// Logical identity for one table header row, independent of pinned region.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TableHeaderRowIdentity {
    depth: usize,
}

impl TableHeaderRowIdentity {
    /// Creates a logical header-row identity.
    pub const fn new(depth: usize) -> Self {
        Self { depth }
    }

    /// Returns the zero-based header depth.
    pub const fn depth(self) -> usize {
        self.depth
    }
}
