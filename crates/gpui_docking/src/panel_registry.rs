use crate::{
    DockItemId, DockPanel, DockPanelCatalog, DockPanelDescriptor,
    panel_view::{DockPanelViewError, DockPanelViewHandle, DockPanelViewStore},
};
use open_gpui::{AnyView, App};

/// Render-time registration snapshot for one dock panel.
///
/// This is the narrow shape render code needs for active content: a lifecycle entry point for
/// resolving live view content. Tab chrome and policy read stable metadata through
/// [`DockPanelCatalog`].
#[derive(Debug, Clone)]
pub(crate) struct DockPanelRenderRegistration {
    view: DockPanelViewHandle,
}

#[derive(Debug, Clone)]
struct DockPanelEntrySnapshot {
    descriptor: DockPanelDescriptor,
    view: DockPanelViewHandle,
}

impl DockPanelRenderRegistration {
    fn new(view: DockPanelViewHandle) -> Self {
        Self { view }
    }

    pub(crate) fn resolve_view(&self, cx: &mut App) -> AnyView {
        self.view.resolve_view(cx)
    }
}

impl DockPanelEntrySnapshot {
    fn new(descriptor: &DockPanelDescriptor, view: DockPanelViewHandle) -> Self {
        Self {
            descriptor: descriptor.clone(),
            view,
        }
    }

    fn descriptor(&self) -> &DockPanelDescriptor {
        &self.descriptor
    }

    fn title(&self) -> &str {
        self.descriptor.title()
    }

    fn is_closable(&self) -> bool {
        self.descriptor.is_closable()
    }

    fn view(&self) -> Result<&AnyView, DockPanelViewError> {
        self.view.view()
    }

    fn has_view(&self) -> bool {
        self.view.has_view()
    }

    fn resolve_view(&self, cx: &mut App) -> AnyView {
        self.view.resolve_view(cx)
    }
}

/// Registry entry snapshot for one dock panel.
///
/// This is the public read seam over the split registry: callers can read stable metadata and, when
/// needed, resolve the live GPUI view lifecycle without requiring metadata storage and view storage
/// to be the same map entry.
#[derive(Debug, Clone)]
pub struct DockPanelRegistration {
    entry: DockPanelEntrySnapshot,
}

impl DockPanelRegistration {
    fn new(entry: DockPanelEntrySnapshot) -> Self {
        Self { entry }
    }

    /// Returns panel metadata without touching live view state.
    pub fn descriptor(&self) -> &DockPanelDescriptor {
        self.entry.descriptor()
    }

    /// Returns the panel title shown in tab chrome.
    pub fn title(&self) -> &str {
        self.entry.title()
    }

    /// Returns whether the panel can be closed by panel lifecycle policy.
    pub fn is_closable(&self) -> bool {
        self.entry.is_closable()
    }

    /// Returns the already-instantiated GPUI view used as this panel's rendered root.
    pub fn view(&self) -> Result<&AnyView, DockPanelViewError> {
        self.entry.view()
    }

    /// Returns true when this panel has an instantiated view.
    pub fn has_view(&self) -> bool {
        self.entry.has_view()
    }

    /// Returns the panel view, instantiating lazy panels on first render.
    pub fn resolve_view(&self, cx: &mut App) -> AnyView {
        self.entry.resolve_view(cx)
    }
}

/// Result of resolving a dock item against a panel registry.
#[derive(Debug)]
pub enum DockPanelResolution<'a> {
    /// The dock item has registered panel content.
    Registered(DockPanelRegistration),
    /// The dock item exists in the graph but has no registered live content.
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

/// Registry mapping pure graph item IDs to panel metadata and optional renderable roots.
#[derive(Debug, Default)]
pub struct DockPanelRegistry {
    catalog: DockPanelCatalog,
    views: DockPanelViewStore,
}

impl DockPanelRegistry {
    /// Creates an empty panel registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers metadata and view content for a dock item.
    ///
    /// Returns the previous complete panel registration when both metadata and view content were
    /// present. Descriptor-only metadata registered through [`Self::register_descriptor`] is
    /// replaced but cannot be returned as a [`DockPanel`].
    pub fn register(&mut self, item: impl Into<DockItemId>, panel: DockPanel) -> Option<DockPanel> {
        let item = item.into();
        let (descriptor, view) = panel.into_parts();
        let previous_descriptor = self.catalog.register(item.clone(), descriptor);
        let previous_view = self.views.register(item, view);
        previous_descriptor
            .zip(previous_view)
            .map(|(descriptor, view)| DockPanel::from_parts(descriptor, view))
    }

    /// Registers metadata for a dock item without binding GPUI view lifecycle state.
    ///
    /// This is the restore/policy/tab-chrome seam for items whose live view content will be
    /// attached later. Updating metadata leaves any existing view handle in place.
    pub fn register_descriptor(
        &mut self,
        item: impl Into<DockItemId>,
        descriptor: DockPanelDescriptor,
    ) -> Option<DockPanelDescriptor> {
        self.catalog.register(item.into(), descriptor)
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

    /// Registers a lazily created view factory for a dock item.
    pub fn register_factory(
        &mut self,
        item: impl Into<DockItemId>,
        title: impl Into<String>,
        factory: impl Fn(&mut App) -> AnyView + 'static,
    ) -> Option<DockPanel> {
        self.register(item, DockPanel::lazy(title, factory))
    }

    /// Returns a registered panel by dock item id.
    pub fn get(&self, item: &DockItemId) -> Option<DockPanelRegistration> {
        self.entry_snapshot(item).map(DockPanelRegistration::new)
    }

    /// Returns descriptor-only panel metadata.
    pub fn catalog(&self) -> &DockPanelCatalog {
        &self.catalog
    }

    /// Returns panel metadata without instantiating or exposing a live view.
    pub fn descriptor(&self, item: &DockItemId) -> Option<&DockPanelDescriptor> {
        self.catalog().descriptor(item)
    }

    pub(crate) fn render_registration(
        &self,
        item: &DockItemId,
    ) -> Option<DockPanelRenderRegistration> {
        self.catalog.descriptor(item)?;
        self.views.view(item).map(DockPanelRenderRegistration::new)
    }

    /// Resolves a dock item to either registered live content or a missing-content state.
    pub fn resolve<'a>(&'a self, item: &'a DockItemId) -> DockPanelResolution<'a> {
        self.get(item)
            .map(DockPanelResolution::Registered)
            .unwrap_or(DockPanelResolution::Missing { item })
    }

    /// Returns true when a dock item has registered metadata.
    pub fn contains(&self, item: &DockItemId) -> bool {
        self.catalog().contains(item)
    }

    /// Returns the number of registered panel descriptors.
    pub fn len(&self) -> usize {
        self.catalog().len()
    }

    /// Returns true when no panels are registered.
    pub fn is_empty(&self) -> bool {
        self.catalog().is_empty()
    }

    fn entry_snapshot(&self, item: &DockItemId) -> Option<DockPanelEntrySnapshot> {
        Some(DockPanelEntrySnapshot::new(
            self.catalog.descriptor(item)?,
            self.views.view(item)?,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{DockItemId, DockPanelDescriptor, DockPanelViewError};

    fn item(id: &str) -> DockItemId {
        DockItemId::from(id)
    }

    #[test]
    fn registry_read_entries_snapshot_metadata_and_share_view_lifecycle() {
        let registration = {
            let mut registry = DockPanelRegistry::new();
            registry.register_factory("lazy", "Lazy", |_| unreachable!());

            assert_eq!(
                registry
                    .descriptor(&item("lazy"))
                    .expect("descriptor should be registered")
                    .title(),
                "Lazy"
            );
            assert!(registry.contains(&item("lazy")));
            assert_eq!(registry.len(), 1);

            registry
                .get(&item("lazy"))
                .expect("registration should be readable")
        };

        assert_eq!(registration.title(), "Lazy");
        assert!(registration.is_closable());
        assert!(!registration.has_view());
        assert!(matches!(
            registration.view(),
            Err(DockPanelViewError::LazyViewNotInstantiated)
        ));
    }

    #[test]
    fn registry_can_register_metadata_before_view_lifecycle() {
        let mut registry = DockPanelRegistry::new();
        let item = item("restored");

        assert_eq!(
            registry.register_descriptor(
                item.clone(),
                DockPanelDescriptor::new("Restored").closable(false),
            ),
            None
        );

        let descriptor = registry
            .descriptor(&item)
            .expect("descriptor-only metadata should be registered");
        assert_eq!(descriptor.title(), "Restored");
        assert!(!descriptor.is_closable());
        assert!(registry.contains(&item));
        assert_eq!(registry.len(), 1);
        assert!(registry.get(&item).is_none());
        assert!(registry.render_registration(&item).is_none());
        assert!(
            registry.resolve(&item).is_missing(),
            "descriptor-only registration should not pretend live content is available"
        );
    }

    #[test]
    fn descriptor_updates_do_not_drop_existing_view_lifecycle() {
        let mut registry = DockPanelRegistry::new();
        let item = item("editor");
        registry.register_factory(item.clone(), "Editor", |_| unreachable!());

        let previous = registry
            .register_descriptor(item.clone(), DockPanelDescriptor::new("Renamed"))
            .expect("metadata update should return previous descriptor");
        assert_eq!(previous.title(), "Editor");

        let registration = registry
            .get(&item)
            .expect("updating metadata should preserve the view handle");
        assert_eq!(registration.title(), "Renamed");
        assert!(matches!(
            registration.view(),
            Err(DockPanelViewError::LazyViewNotInstantiated)
        ));
    }
}
