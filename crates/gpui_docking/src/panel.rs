use crate::DockItemId;
use open_gpui::AnyView;
use std::collections::HashMap;

/// Renderable content and metadata for one dock item.
#[derive(Clone, Debug)]
pub struct DockPanel {
    title: String,
    closable: bool,
    view: AnyView,
}

impl DockPanel {
    /// Creates a panel registration with the default close policy.
    pub fn new(title: impl Into<String>, view: impl Into<AnyView>) -> Self {
        Self {
            title: title.into(),
            closable: true,
            view: view.into(),
        }
    }

    /// Sets whether the panel can be closed by future interaction layers.
    pub fn closable(mut self, closable: bool) -> Self {
        self.closable = closable;
        self
    }

    /// Returns the panel title shown in tab chrome.
    pub fn title(&self) -> &str {
        &self.title
    }

    /// Returns whether the panel can be closed by future interaction layers.
    pub fn is_closable(&self) -> bool {
        self.closable
    }

    /// Returns the GPUI view used as this panel's rendered root.
    pub fn view(&self) -> &AnyView {
        &self.view
    }
}

/// Result of resolving a dock item against a panel registry.
#[derive(Debug)]
pub enum DockPanelResolution<'a> {
    /// The dock item has registered panel content.
    Registered(&'a DockPanel),
    /// The dock item exists in the graph but has no registered content.
    Missing {
        /// The missing dock item id.
        item: &'a DockItemId,
    },
}

impl DockPanelResolution<'_> {
    /// Returns true when the item has no registered panel content.
    pub fn is_missing(&self) -> bool {
        matches!(self, Self::Missing { .. })
    }
}

/// Registry mapping pure graph item IDs to renderable panel roots.
#[derive(Debug, Default)]
pub struct DockPanelRegistry {
    panels: HashMap<DockItemId, DockPanel>,
}

impl DockPanelRegistry {
    /// Creates an empty panel registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers a panel for a dock item, returning any previous registration.
    pub fn register(&mut self, item: impl Into<DockItemId>, panel: DockPanel) -> Option<DockPanel> {
        self.panels.insert(item.into(), panel)
    }

    /// Registers a view with a title for a dock item.
    pub fn register_view(
        &mut self,
        item: impl Into<DockItemId>,
        title: impl Into<String>,
        view: impl Into<AnyView>,
    ) -> Option<DockPanel> {
        self.register(item, DockPanel::new(title, view))
    }

    /// Returns a registered panel by dock item id.
    pub fn get(&self, item: &DockItemId) -> Option<&DockPanel> {
        self.panels.get(item)
    }

    /// Resolves a dock item to either registered content or a missing-panel state.
    pub fn resolve<'a>(&'a self, item: &'a DockItemId) -> DockPanelResolution<'a> {
        self.get(item)
            .map(DockPanelResolution::Registered)
            .unwrap_or(DockPanelResolution::Missing { item })
    }

    /// Returns true when a dock item has registered content.
    pub fn contains(&self, item: &DockItemId) -> bool {
        self.panels.contains_key(item)
    }

    /// Returns the number of registered panels.
    pub fn len(&self) -> usize {
        self.panels.len()
    }

    /// Returns true when no panels are registered.
    pub fn is_empty(&self) -> bool {
        self.panels.is_empty()
    }
}
