use crate::DockItemId;
use std::collections::HashMap;

/// Metadata for one dock panel that can be read without instantiating its view.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DockPanelDescriptor {
    title: String,
    closable: bool,
}

impl DockPanelDescriptor {
    /// Creates panel metadata with the default close policy.
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            closable: true,
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

    pub(crate) fn set_closable(&mut self, closable: bool) {
        self.closable = closable;
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
    pub(crate) fn register(
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
