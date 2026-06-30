//! Stable table identities shared by core and component layers.

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

/// Stable renderer-neutral identity for a table column group header.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TableColumnGroupId(String);

impl TableColumnGroupId {
    /// Creates a column-group identity from a stable string.
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
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
