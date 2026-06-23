use crate::{
    DockClassId, DockItemId, DockPanel, DockPanelCatalog, DockPanelDescriptor,
    panel_view::{DockPanelViewHandle, DockPanelViewStore},
};
use open_gpui::{AnyView, App, Window};
use thiserror::Error;

/// Render-time registration snapshot for one dock panel.
///
/// This is the narrow shape render code needs for active content: a lifecycle entry point for
/// resolving live view content. Tab chrome and policy read stable metadata through
/// [`DockPanelCatalog`].
#[derive(Debug, Clone)]
pub(crate) struct DockPanelRenderRegistration {
    view: DockPanelViewHandle,
}

impl DockPanelRenderRegistration {
    fn new(view: DockPanelViewHandle) -> Self {
        Self { view }
    }

    pub(crate) fn resolve_view(&self, cx: &mut App) -> AnyView {
        self.view.resolve_view(cx)
    }

    pub(crate) fn request_focus(&self, window: &mut Window, cx: &mut App) -> bool {
        self.view.request_focus(window, cx)
    }
}

/// Registry entry snapshot for one dock panel.
///
/// This is the public read seam over the split registry: callers can read stable metadata for an
/// item that also has live view lifecycle state, without exposing the GPUI view handle itself.
#[derive(Debug, Clone)]
pub struct DockPanelRegistration {
    descriptor: DockPanelDescriptor,
}

impl DockPanelRegistration {
    fn new(descriptor: DockPanelDescriptor) -> Self {
        Self { descriptor }
    }

    /// Returns panel metadata without touching live view state.
    pub fn descriptor(&self) -> &DockPanelDescriptor {
        &self.descriptor
    }

    /// Returns the panel title shown in tab chrome.
    pub fn title(&self) -> &str {
        self.descriptor.title()
    }

    /// Returns whether the panel can be closed by panel lifecycle policy.
    pub fn is_closable(&self) -> bool {
        self.descriptor.is_closable()
    }

    /// Returns the optional docking compatibility class for this panel.
    pub fn dock_class(&self) -> Option<&DockClassId> {
        self.descriptor.dock_class()
    }
}

/// Error returned when attaching live view lifecycle state to restored panel metadata fails.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum DockPanelAttachError {
    /// The dock item has no registered descriptor metadata.
    #[error("dock item {item} has no registered panel descriptor")]
    MissingDescriptor {
        /// The item that was requested.
        item: DockItemId,
    },
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
        let parts = panel.into_parts();
        let previous_descriptor = self.catalog.register(item.clone(), parts.descriptor);
        let previous_view = self.views.register(item, parts.view);
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

    /// Attaches view content to existing panel metadata without rewriting the descriptor.
    ///
    /// This is the lazy-restore seam: callers can restore titles and close policy first, then bind
    /// GPUI view lifecycle state when an application module becomes available.
    /// Returns a registered panel by dock item id.
    pub fn get(&self, item: &DockItemId) -> Option<DockPanelRegistration> {
        if !self.views.contains(item) {
            return None;
        }

        self.catalog
            .descriptor(item)
            .cloned()
            .map(DockPanelRegistration::new)
    }

    /// Returns descriptor-only panel metadata.
    pub fn catalog(&self) -> &DockPanelCatalog {
        &self.catalog
    }

    /// Returns panel metadata without instantiating or exposing a live view.
    pub fn descriptor(&self, item: &DockItemId) -> Option<&DockPanelDescriptor> {
        self.catalog().descriptor(item)
    }

    /// Returns true when a dock item has registered GPUI view lifecycle state.
    ///
    /// This is distinct from [`Self::contains`], which reports descriptor metadata. Restored
    /// descriptor-only panels can be known to docking policy and tab chrome before application code
    /// attaches eager or lazy view content.
    pub fn has_view_lifecycle(&self, item: &DockItemId) -> bool {
        self.views.contains(item)
    }

    pub(crate) fn render_registration(
        &self,
        item: &DockItemId,
    ) -> Option<DockPanelRenderRegistration> {
        self.catalog.descriptor(item)?;
        self.views.view(item).map(DockPanelRenderRegistration::new)
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

    pub(crate) fn attach_view_handle(
        &mut self,
        item: DockItemId,
        view: DockPanelViewHandle,
    ) -> Result<Option<DockPanelRegistration>, DockPanelAttachError> {
        let Some(descriptor) = self.catalog.descriptor(&item).cloned() else {
            return Err(DockPanelAttachError::MissingDescriptor { item });
        };

        let previous = self
            .views
            .register(item, view)
            .map(|_| DockPanelRegistration::new(descriptor));
        Ok(previous)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{DockClassId, DockItemId, DockPanel, DockPanelDescriptor};

    fn item(id: &str) -> DockItemId {
        DockItemId::from(id)
    }

    #[test]
    fn registry_read_entries_snapshot_metadata_and_share_view_lifecycle() {
        let registration = {
            let mut registry = DockPanelRegistry::new();
            registry.register("lazy", DockPanel::lazy("Lazy", |_| unreachable!()));

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
        assert!(!registry.has_view_lifecycle(&item));
        assert_eq!(registry.len(), 1);
        assert!(registry.get(&item).is_none());
        assert!(registry.render_registration(&item).is_none());
        assert!(
            registry.get(&item).is_none(),
            "descriptor-only registration should not pretend live content is available"
        );
    }

    #[test]
    fn registry_attaches_view_lifecycle_without_rewriting_metadata() {
        let mut registry = DockPanelRegistry::new();
        let item = item("restored");
        registry.register_descriptor(
            item.clone(),
            DockPanelDescriptor::new("Restored")
                .closable(false)
                .with_dock_class("editor"),
        );

        assert!(matches!(
            registry.attach_view_handle(
                item.clone(),
                crate::panel_view::DockPanelViewHandle::lazy(|_| unreachable!())
            ),
            Ok(None)
        ));

        let registration = registry
            .get(&item)
            .expect("attached view lifecycle should complete registration");
        assert!(registry.has_view_lifecycle(&item));
        assert_eq!(registration.title(), "Restored");
        assert!(!registration.is_closable());
        assert_eq!(
            registration.dock_class(),
            Some(&DockClassId::from("editor"))
        );
    }

    #[test]
    fn registry_attach_requires_existing_descriptor_metadata() {
        let mut registry = DockPanelRegistry::new();

        assert!(matches!(
            registry.attach_view_handle(
                item("missing"),
                crate::panel_view::DockPanelViewHandle::lazy(|_| unreachable!())
            ),
            Err(DockPanelAttachError::MissingDescriptor { item }) if item == self::item("missing")
        ));
    }

    #[test]
    fn registry_attach_replacement_returns_previous_registration_snapshot() {
        let mut registry = DockPanelRegistry::new();
        let item = item("editor");
        registry.register_descriptor(item.clone(), DockPanelDescriptor::new("Editor"));
        registry
            .attach_view_handle(
                item.clone(),
                crate::panel_view::DockPanelViewHandle::lazy(|_| unreachable!()),
            )
            .expect("first attach should succeed");

        let previous = registry
            .attach_view_handle(
                item.clone(),
                crate::panel_view::DockPanelViewHandle::lazy(|_| unreachable!()),
            )
            .expect("replacement attach should succeed")
            .expect("replacement should return previous registration");

        assert_eq!(previous.title(), "Editor");
    }

    #[test]
    fn descriptor_updates_do_not_drop_existing_view_lifecycle() {
        let mut registry = DockPanelRegistry::new();
        let item = item("editor");
        registry.register(item.clone(), DockPanel::lazy("Editor", |_| unreachable!()));

        let previous = registry
            .register_descriptor(item.clone(), DockPanelDescriptor::new("Renamed"))
            .expect("metadata update should return previous descriptor");
        assert_eq!(previous.title(), "Editor");

        let registration = registry
            .get(&item)
            .expect("updating metadata should preserve the view handle");
        assert!(registry.has_view_lifecycle(&item));
        assert_eq!(registration.title(), "Renamed");
    }
}
