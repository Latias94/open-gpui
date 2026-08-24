use serde::{Deserialize, Deserializer, Serialize, Serializer};
use slotmap::Key;
use std::{
    borrow::Borrow,
    fmt,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
};

slotmap::new_key_type! {
    /// Runtime identifier for a node in a [`model::DockGraph`](crate::model::DockGraph).
    pub struct DockNodeId;

    /// Runtime structural identity in the canonical binary docking graph.
    pub(crate) struct CanonicalNodeId;
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
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DockSpaceId(Arc<str>);

impl DockSpaceId {
    /// Creates a dock space id from a stable application-provided string.
    pub fn new(id: impl Into<String>) -> Self {
        Self(Arc::from(id.into()))
    }

    /// Returns the underlying string id.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

static NEXT_CANONICAL_AUTHORITY_EPOCH: AtomicU64 = AtomicU64::new(1);

/// Opaque publication identity for one canonical graph authority.
#[derive(Debug)]
pub(crate) struct CanonicalGraphAuthority {
    authority_epoch: u64,
    topology_revision: u64,
    selection_revision: u64,
    interaction_revision: u64,
}

impl CanonicalGraphAuthority {
    pub(crate) fn new() -> Self {
        let authority_epoch = NEXT_CANONICAL_AUTHORITY_EPOCH.fetch_add(1, Ordering::Relaxed);
        assert!(
            authority_epoch != 0,
            "canonical graph authority epoch exhausted"
        );
        Self {
            authority_epoch,
            topology_revision: 0,
            selection_revision: 0,
            interaction_revision: 0,
        }
    }

    pub(crate) fn publish(&mut self) -> CanonicalGraphVersion {
        self.record_topology_change()
    }

    pub(crate) fn next_topology_change(&self) -> Option<CanonicalGraphVersion> {
        Some(CanonicalGraphVersion {
            authority_epoch: self.authority_epoch,
            topology_revision: self.topology_revision.checked_add(1)?,
            selection_revision: self.selection_revision,
            interaction_revision: self.interaction_revision,
        })
    }

    pub(crate) fn record_topology_change(&mut self) -> CanonicalGraphVersion {
        let next = self
            .next_topology_change()
            .expect("canonical graph topology revision exhausted");
        self.topology_revision = next.topology_revision;
        next
    }

    pub(crate) fn current(&self) -> CanonicalGraphVersion {
        CanonicalGraphVersion {
            authority_epoch: self.authority_epoch,
            topology_revision: self.topology_revision,
            selection_revision: self.selection_revision,
            interaction_revision: self.interaction_revision,
        }
    }

    pub(crate) fn record_selection_change(&mut self) -> CanonicalGraphVersion {
        self.next_selection_change()
            .map(|next| {
                self.selection_revision = next.selection_revision;
                next
            })
            .expect("canonical graph selection revision exhausted")
    }

    pub(crate) fn next_selection_change(&self) -> Option<CanonicalGraphVersion> {
        Some(CanonicalGraphVersion {
            authority_epoch: self.authority_epoch,
            topology_revision: self.topology_revision,
            selection_revision: self.selection_revision.checked_add(1)?,
            interaction_revision: self.interaction_revision,
        })
    }

    pub(crate) fn record_interaction_change(&mut self) -> CanonicalGraphVersion {
        self.interaction_revision = self
            .interaction_revision
            .checked_add(1)
            .expect("canonical graph interaction revision exhausted");
        self.current()
    }
}

/// Identifies one canonical graph publication and its independent state revisions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct CanonicalGraphVersion {
    authority_epoch: u64,
    topology_revision: u64,
    selection_revision: u64,
    interaction_revision: u64,
}

impl CanonicalGraphVersion {
    #[cfg(test)]
    pub(crate) const fn new(authority_epoch: u64, topology_revision: u64) -> Self {
        Self {
            authority_epoch,
            topology_revision,
            selection_revision: 0,
            interaction_revision: 0,
        }
    }

    pub(crate) fn authority_epoch(self) -> u64 {
        self.authority_epoch
    }

    pub(crate) fn topology_revision(self) -> u64 {
        self.topology_revision
    }

    pub(crate) fn selection_revision(self) -> u64 {
        self.selection_revision
    }

    pub(crate) fn interaction_revision(self) -> u64 {
        self.interaction_revision
    }
}

/// Durable identity for a tab group.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct DockGroupId(Arc<str>);

impl DockGroupId {
    pub(crate) fn new(id: impl Into<String>) -> Self {
        Self(Arc::from(id.into()))
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<&str> for DockGroupId {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

impl From<String> for DockGroupId {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

impl fmt::Display for DockGroupId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Durable identity for a first-class floating root.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct DockFloatingId(Arc<str>);

impl DockFloatingId {
    pub(crate) fn new(id: impl Into<String>) -> Self {
        Self(Arc::from(id.into()))
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<&str> for DockFloatingId {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

impl From<String> for DockFloatingId {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

impl fmt::Display for DockFloatingId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl Serialize for DockSpaceId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for DockSpaceId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        String::deserialize(deserializer).map(Self::new)
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
pub struct DockItemId(Arc<str>);

impl DockItemId {
    /// Creates a dock item id from a stable application-provided string.
    pub fn new(id: impl Into<String>) -> Self {
        Self(Arc::from(id.into()))
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

/// Stable compatibility class for dockable panels.
///
/// A class is application-defined metadata used by [`DockPolicy`](crate::DockPolicy) to restrict
/// which panels can be dropped into specific dock spaces. Panels without a class remain compatible
/// with every dock space by default.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct DockClassId(String);

impl DockClassId {
    /// Creates a dock class id from a stable application-provided string.
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    /// Returns the underlying string id.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Borrow<str> for DockClassId {
    fn borrow(&self) -> &str {
        self.as_str()
    }
}

impl From<&str> for DockClassId {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

impl From<String> for DockClassId {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

impl fmt::Display for DockClassId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}
