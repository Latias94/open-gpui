use crate::{
    DockActionOutcome, DockClassId, DockItemId, DockPanelDescriptor, DockPanelPlacement,
    DockPanelPlacementTarget, DockPanelReopenPolicy, DockSpaceId, panel_view::DockPanelViewHandle,
};
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

/// Source of the product placement intent used by a panel open transaction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DockPanelOpenPlacementSource {
    /// The caller supplied an explicit product placement.
    Explicit,
    /// The panel reopened from descriptor-recorded last-known placement.
    LastKnown,
    /// The panel reopened from descriptor default placement.
    DescriptorDefault,
    /// The panel had no product placement metadata, so center placement was used.
    ImplicitCenter,
}

/// Product-level outcome of closing a dock panel.
#[derive(Debug, Clone, PartialEq)]
pub struct DockPanelCloseOutcome {
    action: DockActionOutcome,
    space: DockSpaceId,
    item: DockItemId,
    placement: Option<DockPanelPlacement>,
}

impl DockPanelCloseOutcome {
    pub(crate) fn new(
        action: DockActionOutcome,
        space: DockSpaceId,
        item: DockItemId,
        placement: Option<DockPanelPlacement>,
    ) -> Self {
        Self {
            action,
            space,
            item,
            placement,
        }
    }

    /// Returns the underlying graph action outcome.
    pub fn action(&self) -> DockActionOutcome {
        self.action
    }

    /// Returns true when the close changed docking state.
    pub fn changed(&self) -> bool {
        self.action.changed()
    }

    /// Returns the logical dock space where the panel was closed.
    pub fn space(&self) -> &DockSpaceId {
        &self.space
    }

    /// Returns the panel item id.
    pub fn item(&self) -> &DockItemId {
        &self.item
    }

    /// Returns the product placement inferred before close, when known.
    pub fn placement(&self) -> Option<&DockPanelPlacement> {
        self.placement.as_ref()
    }
}

/// Product-level outcome of opening or reopening a dock panel.
#[derive(Debug, Clone, PartialEq)]
pub struct DockPanelOpenOutcome {
    action: DockActionOutcome,
    space: DockSpaceId,
    item: DockItemId,
    placement: DockPanelPlacement,
    placement_source: DockPanelOpenPlacementSource,
}

impl DockPanelOpenOutcome {
    pub(crate) fn new(
        action: DockActionOutcome,
        space: DockSpaceId,
        item: DockItemId,
        placement: DockPanelPlacement,
        placement_source: DockPanelOpenPlacementSource,
    ) -> Self {
        Self {
            action,
            space,
            item,
            placement,
            placement_source,
        }
    }

    /// Returns the underlying graph action outcome.
    pub fn action(&self) -> DockActionOutcome {
        self.action
    }

    /// Returns true when the open changed docking state.
    pub fn changed(&self) -> bool {
        self.action.changed()
    }

    /// Returns the logical dock space where the panel was opened.
    pub fn space(&self) -> &DockSpaceId {
        &self.space
    }

    /// Returns the panel item id.
    pub fn item(&self) -> &DockItemId {
        &self.item
    }

    /// Returns the product placement intent used by the open transaction.
    pub fn placement(&self) -> &DockPanelPlacement {
        &self.placement
    }

    /// Returns where the product placement intent came from.
    pub fn placement_source(&self) -> DockPanelOpenPlacementSource {
        self.placement_source
    }
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

    /// Sets whether application metadata marks the panel as dirty.
    pub fn dirty(mut self, dirty: bool) -> Self {
        self.descriptor.set_dirty(dirty);
        self
    }

    /// Sets the descriptor-level close veto reason.
    pub fn with_close_veto_reason(mut self, reason: impl Into<String>) -> Self {
        self.descriptor.set_close_veto_reason(Some(reason.into()));
        self
    }

    /// Sets the docking compatibility class for this panel.
    pub fn with_dock_class(mut self, dock_class: impl Into<DockClassId>) -> Self {
        self.descriptor.set_dock_class(Some(dock_class.into()));
        self
    }

    /// Sets the descriptor-level default product placement.
    pub fn with_default_placement(mut self, placement: DockPanelPlacementTarget) -> Self {
        self.descriptor = self.descriptor.with_default_placement(placement);
        self
    }

    /// Sets the placement selection policy used by product-level reopen operations.
    pub fn with_reopen_policy(mut self, policy: DockPanelReopenPolicy) -> Self {
        self.descriptor = self.descriptor.with_reopen_policy(policy);
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
