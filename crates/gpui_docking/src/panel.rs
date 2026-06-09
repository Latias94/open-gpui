use crate::{
    DockPanelDescriptor,
    panel_view::{DockPanelViewError, DockPanelViewHandle},
};
use open_gpui::{AnyView, App};
use std::fmt;

/// Panel registration for one dock item.
///
/// Metadata is available through [`DockPanelDescriptor`] without instantiating lazy GPUI views.
/// Live view creation and caching stay behind this registration and outside graph/layout
/// persistence.
#[derive(Clone)]
pub struct DockPanel {
    descriptor: DockPanelDescriptor,
    view: DockPanelViewHandle,
}

impl fmt::Debug for DockPanel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DockPanel")
            .field("title", &self.title())
            .field("closable", &self.is_closable())
            .field("view_lifecycle", &self.view)
            .finish()
    }
}

impl DockPanel {
    /// Creates a panel registration with the default close policy.
    pub fn new(title: impl Into<String>, view: impl Into<AnyView>) -> Self {
        Self::from_parts(
            DockPanelDescriptor::new(title),
            DockPanelViewHandle::from_view(view),
        )
    }

    /// Creates a lazily instantiated panel registration with the default close policy.
    pub fn lazy(title: impl Into<String>, factory: impl Fn(&mut App) -> AnyView + 'static) -> Self {
        Self::from_parts(
            DockPanelDescriptor::new(title),
            DockPanelViewHandle::lazy(factory),
        )
    }

    pub(crate) fn from_parts(descriptor: DockPanelDescriptor, view: DockPanelViewHandle) -> Self {
        Self { descriptor, view }
    }

    pub(crate) fn into_parts(self) -> (DockPanelDescriptor, DockPanelViewHandle) {
        (self.descriptor, self.view)
    }

    /// Sets whether the panel can be closed by future interaction layers.
    pub fn closable(mut self, closable: bool) -> Self {
        self.descriptor.set_closable(closable);
        self
    }

    /// Returns panel metadata without touching live view state.
    pub fn descriptor(&self) -> &DockPanelDescriptor {
        &self.descriptor
    }

    /// Returns the panel title shown in tab chrome.
    pub fn title(&self) -> &str {
        self.descriptor().title()
    }

    /// Returns whether the panel can be closed by future interaction layers.
    pub fn is_closable(&self) -> bool {
        self.descriptor().is_closable()
    }

    /// Returns the already-instantiated GPUI view used as this panel's rendered root.
    ///
    /// Lazy panels instantiate when rendered through [`Self::resolve_view`]. Before that happens,
    /// this method returns [`DockPanelViewError::LazyViewNotInstantiated`].
    pub fn view(&self) -> Result<&AnyView, DockPanelViewError> {
        self.view.view()
    }

    /// Returns true when this panel has an instantiated view.
    pub fn has_view(&self) -> bool {
        self.view.has_view()
    }

    /// Returns the panel view, instantiating lazy panels on first render.
    pub fn resolve_view(&self, cx: &mut App) -> AnyView {
        self.view.resolve_view(cx)
    }
}
