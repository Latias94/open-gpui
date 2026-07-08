use crate::{DockClassId, DockItemId, DockPanelPlacementTarget};
use std::collections::HashMap;

/// Policy for choosing placement when reopening a registered panel.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum DockPanelReopenPolicy {
    /// Prefer the most recent valid product placement, then fall back to the descriptor default.
    #[default]
    RestoreLastKnown,
    /// Ignore remembered placement and use the descriptor default placement when available.
    DescriptorDefaultOnly,
}

/// Metadata for one dock panel that can be read without instantiating its view.
#[derive(Debug, Clone, PartialEq)]
pub struct DockPanelDescriptor {
    title: String,
    closable: bool,
    dirty: bool,
    close_veto_reason: Option<String>,
    dock_class: Option<DockClassId>,
    default_placement: Option<DockPanelPlacementTarget>,
    last_known_placement: Option<DockPanelPlacementTarget>,
    reopen_policy: DockPanelReopenPolicy,
}

impl DockPanelDescriptor {
    /// Creates panel metadata with the default close policy.
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            closable: true,
            dirty: false,
            close_veto_reason: None,
            dock_class: None,
            default_placement: None,
            last_known_placement: None,
            reopen_policy: DockPanelReopenPolicy::default(),
        }
    }

    /// Returns the panel title shown in tab chrome.
    pub fn title(&self) -> &str {
        &self.title
    }

    /// Returns whether the panel can be closed by panel lifecycle policy.
    pub fn is_closable(&self) -> bool {
        self.closable
    }

    /// Returns true when application metadata marks the panel as dirty.
    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    /// Returns the descriptor-level close veto reason, when provided.
    pub fn close_veto_reason(&self) -> Option<&str> {
        self.close_veto_reason.as_deref()
    }

    /// Returns the optional docking compatibility class for this panel.
    pub fn dock_class(&self) -> Option<&DockClassId> {
        self.dock_class.as_ref()
    }

    /// Returns the descriptor-level default product placement.
    pub fn default_placement(&self) -> Option<&DockPanelPlacementTarget> {
        self.default_placement.as_ref()
    }

    /// Returns the most recent product placement recorded while closing or opening this panel.
    pub fn last_known_placement(&self) -> Option<&DockPanelPlacementTarget> {
        self.last_known_placement.as_ref()
    }

    /// Returns the placement selection policy used by product-level reopen operations.
    pub fn reopen_policy(&self) -> DockPanelReopenPolicy {
        self.reopen_policy
    }

    pub(crate) fn set_closable(&mut self, closable: bool) {
        self.closable = closable;
    }

    pub(crate) fn set_dirty(&mut self, dirty: bool) {
        self.dirty = dirty;
    }

    pub(crate) fn set_close_veto_reason(&mut self, reason: Option<String>) {
        self.close_veto_reason = reason;
    }

    pub(crate) fn set_dock_class(&mut self, dock_class: Option<DockClassId>) {
        self.dock_class = dock_class;
    }

    pub(crate) fn set_last_known_placement(&mut self, placement: Option<DockPanelPlacementTarget>) {
        self.last_known_placement = placement;
    }

    /// Sets whether the panel can be closed by panel lifecycle policy.
    pub fn closable(mut self, closable: bool) -> Self {
        self.set_closable(closable);
        self
    }

    /// Sets whether application metadata marks the panel as dirty.
    pub fn dirty(mut self, dirty: bool) -> Self {
        self.set_dirty(dirty);
        self
    }

    /// Sets the descriptor-level close veto reason.
    pub fn with_close_veto_reason(mut self, reason: impl Into<String>) -> Self {
        self.set_close_veto_reason(Some(reason.into()));
        self
    }

    /// Clears the descriptor-level close veto reason.
    pub fn without_close_veto_reason(mut self) -> Self {
        self.set_close_veto_reason(None);
        self
    }

    /// Sets the docking compatibility class for this panel.
    pub fn with_dock_class(mut self, dock_class: impl Into<DockClassId>) -> Self {
        self.set_dock_class(Some(dock_class.into()));
        self
    }

    /// Clears the docking compatibility class for this panel.
    pub fn unclassed(mut self) -> Self {
        self.set_dock_class(None);
        self
    }

    /// Sets the descriptor-level default product placement.
    pub fn with_default_placement(mut self, placement: DockPanelPlacementTarget) -> Self {
        self.default_placement = Some(placement);
        self
    }

    /// Clears the descriptor-level default product placement.
    pub fn without_default_placement(mut self) -> Self {
        self.default_placement = None;
        self
    }

    /// Sets the placement selection policy used by product-level reopen operations.
    pub fn with_reopen_policy(mut self, policy: DockPanelReopenPolicy) -> Self {
        self.reopen_policy = policy;
        self
    }
}

/// Descriptor-only catalog for registered dock panels.
///
/// This catalog is the metadata seam for restore, policy, and tab chrome paths that must not touch
/// GPUI view lifecycle state. Rendering can still resolve live views through
/// [`DockPanelRegistry`](crate::DockPanelRegistry), but callers that only need titles or close
/// policy should read this catalog instead.
#[derive(Debug, Default)]
pub struct DockPanelCatalog {
    descriptors: HashMap<DockItemId, DockPanelDescriptor>,
}

impl DockPanelCatalog {
    /// Registers descriptor-only metadata for a dock item.
    pub fn register(
        &mut self,
        item: DockItemId,
        descriptor: DockPanelDescriptor,
    ) -> Option<DockPanelDescriptor> {
        self.descriptors.insert(item, descriptor)
    }

    /// Returns panel metadata without instantiating or exposing a live view.
    pub fn descriptor(&self, item: &DockItemId) -> Option<&DockPanelDescriptor> {
        self.descriptors.get(item)
    }

    /// Returns registered descriptor metadata in stable item-id order.
    pub fn descriptors(&self) -> Vec<(DockItemId, DockPanelDescriptor)> {
        let mut descriptors = self
            .descriptors
            .iter()
            .map(|(item, descriptor)| (item.clone(), descriptor.clone()))
            .collect::<Vec<_>>();
        descriptors.sort_by(|(left, _), (right, _)| left.cmp(right));
        descriptors
    }

    pub(crate) fn descriptor_mut(&mut self, item: &DockItemId) -> Option<&mut DockPanelDescriptor> {
        self.descriptors.get_mut(item)
    }

    /// Returns true when a dock item has registered metadata.
    pub fn contains(&self, item: &DockItemId) -> bool {
        self.descriptors.contains_key(item)
    }

    /// Returns the number of registered panel descriptors.
    pub fn len(&self) -> usize {
        self.descriptors.len()
    }

    /// Returns true when no panel descriptors are registered.
    pub fn is_empty(&self) -> bool {
        self.descriptors.is_empty()
    }
}
