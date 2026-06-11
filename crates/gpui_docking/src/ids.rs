use serde::{Deserialize, Serialize};
use slotmap::Key;
use std::{borrow::Borrow, fmt};

slotmap::new_key_type! {
    /// Runtime identifier for a node in a [`DockGraph`](crate::DockGraph).
    pub struct DockNodeId;
}

impl DockNodeId {
    /// Converts this node id to a stable numeric representation for diagnostics.
    pub fn as_u64(self) -> u64 {
        self.data().as_ffi()
    }
}

/// Logical identifier for a dock host.
///
/// A dock space is not necessarily an OS window. It is the root container managed by one
/// `DockHost` or equivalent application-level owner.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct DockSpaceId(String);

impl DockSpaceId {
    /// Creates a dock space id from a stable application-provided string.
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    /// Returns the underlying string id.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Default for DockSpaceId {
    fn default() -> Self {
        Self::new("main")
    }
}

impl Borrow<str> for DockSpaceId {
    fn borrow(&self) -> &str {
        self.as_str()
    }
}

impl From<&str> for DockSpaceId {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

impl From<String> for DockSpaceId {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

impl fmt::Display for DockSpaceId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Stable identifier for a dockable item.
///
/// Applications map this id to titles, view builders, and close policies outside the pure graph.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct DockItemId(String);

impl DockItemId {
    /// Creates a dock item id from a stable application-provided string.
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    /// Returns the underlying string id.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Borrow<str> for DockItemId {
    fn borrow(&self) -> &str {
        self.as_str()
    }
}

impl From<&str> for DockItemId {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

impl From<String> for DockItemId {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

impl fmt::Display for DockItemId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}
