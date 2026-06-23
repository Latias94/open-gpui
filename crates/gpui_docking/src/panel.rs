use crate::{DockClassId, DockPanelDescriptor, panel_view::DockPanelViewHandle};
use open_gpui::{AnyView, App, Entity, Focusable, Render};
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

pub(crate) struct DockPanelParts {
    pub(crate) descriptor: DockPanelDescriptor,
    pub(crate) view: DockPanelViewHandle,
}

impl fmt::Debug for DockPanel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DockPanel")
            .field("title", &self.title())
            .field("closable", &self.is_closable())
            .field("dock_class", &self.dock_class())
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

    /// Creates a panel registration whose GPUI view can receive focus after docking actions.
    pub fn focusable<V>(title: impl Into<String>, view: Entity<V>) -> Self
    where
        V: Focusable + Render,
    {
        Self::from_parts(
            DockPanelDescriptor::new(title),
            DockPanelViewHandle::focusable_view(view),
        )
    }

    /// Creates a lazily instantiated panel registration with the default close policy.
    pub fn lazy(title: impl Into<String>, factory: impl Fn(&mut App) -> AnyView + 'static) -> Self {
        Self::from_parts(
            DockPanelDescriptor::new(title),
            DockPanelViewHandle::lazy(factory),
        )
    }

    /// Creates a lazily instantiated focusable panel registration with the default close policy.
    pub fn lazy_focusable<V>(
        title: impl Into<String>,
        factory: impl Fn(&mut App) -> Entity<V> + 'static,
    ) -> Self
    where
        V: Focusable + Render,
    {
        Self::from_parts(
            DockPanelDescriptor::new(title),
            DockPanelViewHandle::lazy_focusable(factory),
        )
    }

    pub(crate) fn from_parts(descriptor: DockPanelDescriptor, view: DockPanelViewHandle) -> Self {
        Self { descriptor, view }
    }

    pub(crate) fn into_parts(self) -> DockPanelParts {
        DockPanelParts {
            descriptor: self.descriptor,
            view: self.view,
        }
    }

    /// Sets whether the panel can be closed by future interaction layers.
    pub fn closable(mut self, closable: bool) -> Self {
        self.descriptor.set_closable(closable);
        self
    }

    /// Sets the docking compatibility class for this panel.
    pub fn with_dock_class(mut self, dock_class: impl Into<DockClassId>) -> Self {
        self.descriptor.set_dock_class(Some(dock_class.into()));
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

    /// Returns the optional docking compatibility class for this panel.
    pub fn dock_class(&self) -> Option<&DockClassId> {
        self.descriptor().dock_class()
    }
}
