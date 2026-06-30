//! Renderer-neutral active-descendant identity for composite widget surfaces.

use crate::focus::FocusTargetId;

/// Stable active-descendant target metadata.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ActiveDescendant {
    target: Option<FocusTargetId>,
}

impl ActiveDescendant {
    /// Creates an empty active-descendant reference.
    pub const fn none() -> Self {
        Self { target: None }
    }

    /// Creates an active-descendant reference from a stable focus target id.
    pub fn new(target: impl Into<FocusTargetId>) -> Self {
        Self {
            target: Some(target.into()),
        }
    }

    /// Returns the active focus target id, if any.
    pub fn target(&self) -> Option<&FocusTargetId> {
        self.target.as_ref()
    }

    /// Returns the active target string, if any.
    pub fn as_str(&self) -> Option<&str> {
        self.target.as_ref().map(FocusTargetId::as_str)
    }

    /// Consumes the reference and returns the underlying target id, if any.
    pub fn into_target(self) -> Option<FocusTargetId> {
        self.target
    }
}

impl From<FocusTargetId> for ActiveDescendant {
    fn from(target: FocusTargetId) -> Self {
        Self::new(target)
    }
}

#[cfg(test)]
mod tests {
    use super::ActiveDescendant;
    use crate::focus::FocusTargetId;

    #[test]
    fn active_descendant_wraps_focus_target_identity() {
        let active = ActiveDescendant::new("choice.item.1");

        assert_eq!(active.as_str(), Some("choice.item.1"));
        assert_eq!(
            active.target().map(FocusTargetId::as_str),
            Some("choice.item.1")
        );
    }

    #[test]
    fn active_descendant_none_is_empty() {
        assert!(ActiveDescendant::none().target().is_none());
    }
}
