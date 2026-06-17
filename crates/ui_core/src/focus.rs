//! Renderer-neutral focus vocabulary used by the Open GPUI component ecosystem.

use std::fmt;

/// Stable semantic identity for a focus target.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FocusTargetId {
    id: String,
}

impl FocusTargetId {
    /// Creates a focus target id from a stable string.
    pub fn new(id: impl Into<String>) -> Self {
        Self { id: id.into() }
    }

    /// Returns the stable target id.
    pub fn as_str(&self) -> &str {
        &self.id
    }
}

impl From<&str> for FocusTargetId {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

impl From<String> for FocusTargetId {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

impl fmt::Display for FocusTargetId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.id)
    }
}
