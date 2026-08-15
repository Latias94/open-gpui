use super::{DockSurface, DockSurfaceChangeCategory, with_root_transaction};
use crate::{
    DockActionApplyError, DockActionOutcome, DockController, DockGraphDropTarget, DockItemId,
    DockLayout, DockNodeId, DockPanelCloseOutcome, DockPanelDescriptor, DockPanelOpenOutcome,
    DockPanelPlacement, DockPolicyError, DockSpaceId,
};
use open_gpui::{App, AppContext as _, Bounds, Pixels};
use thiserror::Error;

/// Facade-level outcome of a product panel operation.
#[derive(Debug, Clone, PartialEq)]
pub enum DockSurfacePanelOutcome {
    /// A panel was opened or reopened into graph state.
    Opened(DockPanelOpenOutcome),
    /// A panel was closed while its registration stayed available.
    Closed(DockPanelCloseOutcome),
    /// A panel was moved into an in-window floating container.
    Floated(DockSurfaceChange),
    /// A floating panel was moved back into the dock layout.
    Docked(DockSurfaceChange),
    /// A panel became the selected tab in its current tab stack.
    Selected(DockSurfaceChange),
    /// An in-window floating panel container received new bounds.
    FloatingBoundsSet(DockSurfaceChange),
    /// An in-window floating panel container was raised above sibling floating containers.
    FloatingRaised(DockSurfaceChange),
}

/// App-level change flag for facade panel operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DockSurfaceChange {
    /// The operation changed docking state.
    Changed,
    /// The operation was valid but left docking state unchanged.
    Unchanged,
}

/// Common facade location category for a dock panel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DockSurfacePanelLocationKind {
    /// The panel is in the docked root layout for a dock space.
    Docked,
    /// The panel is in an in-window floating container.
    Floating,
}

/// Common facade location facts for one dock panel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DockSurfacePanelLocation {
    space: DockSpaceId,
    kind: DockSurfacePanelLocationKind,
    tab_index: usize,
}

/// Descriptor and live-view facts for one registered dock panel.
#[derive(Debug, Clone, PartialEq)]
pub struct DockSurfacePanelSnapshot {
    item: DockItemId,
    descriptor: DockPanelDescriptor,
    has_view_lifecycle: bool,
    location: Option<DockSurfacePanelLocation>,
}

/// Common facade snapshot for one in-window floating container.
#[derive(Debug, Clone, PartialEq)]
pub struct DockSurfaceFloatingPanelSnapshot {
    space: DockSpaceId,
    items: Vec<DockItemId>,
    bounds: Bounds<Pixels>,
}

impl DockSurfacePanelOutcome {
    /// Returns true when the operation changed docking graph state.
    pub fn changed(&self) -> bool {
        match self {
            Self::Opened(outcome) => outcome.changed(),
            Self::Closed(outcome) => outcome.changed(),
            Self::Floated(change)
            | Self::Docked(change)
            | Self::Selected(change)
            | Self::FloatingBoundsSet(change)
            | Self::FloatingRaised(change) => change.changed(),
        }
    }
}

impl DockSurfacePanelLocation {
    fn new(space: DockSpaceId, kind: DockSurfacePanelLocationKind, tab_index: usize) -> Self {
        Self {
            space,
            kind,
            tab_index,
        }
    }

    /// Returns the dock space that contains the panel.
    pub fn space(&self) -> &DockSpaceId {
        &self.space
    }

    /// Returns whether the panel is docked or in-window floating.
    pub fn kind(&self) -> DockSurfacePanelLocationKind {
        self.kind
    }

    /// Returns the panel's index within its current tab stack.
    pub fn tab_index(&self) -> usize {
        self.tab_index
    }
}

impl DockSurfacePanelSnapshot {
    fn new(
        item: DockItemId,
        descriptor: DockPanelDescriptor,
        has_view_lifecycle: bool,
        location: Option<DockSurfacePanelLocation>,
    ) -> Self {
        Self {
            item,
            descriptor,
            has_view_lifecycle,
            location,
        }
    }

    /// Returns the panel item id.
    pub fn item(&self) -> &DockItemId {
        &self.item
    }

    /// Returns descriptor metadata without touching live view state.
    pub fn descriptor(&self) -> &DockPanelDescriptor {
        &self.descriptor
    }

    /// Returns true when the panel has GPUI view lifecycle state attached.
    pub fn has_view_lifecycle(&self) -> bool {
        self.has_view_lifecycle
    }

    /// Returns current graph location facts, when the panel is open.
    pub fn location(&self) -> Option<&DockSurfacePanelLocation> {
        self.location.as_ref()
    }
}

impl DockSurfaceFloatingPanelSnapshot {
    fn new(space: DockSpaceId, items: Vec<DockItemId>, bounds: Bounds<Pixels>) -> Self {
        Self {
            space,
            items,
            bounds,
        }
    }

    /// Returns the dock space that owns this floating container.
    pub fn space(&self) -> &DockSpaceId {
        &self.space
    }

    /// Returns the item ids contained in this floating container.
    pub fn items(&self) -> &[DockItemId] {
        &self.items
    }

    /// Returns container bounds relative to the dock host.
    pub fn bounds(&self) -> Bounds<Pixels> {
        self.bounds
    }
}

impl DockSurfaceChange {
    /// Returns true when the operation changed docking state.
    pub fn changed(self) -> bool {
        matches!(self, Self::Changed)
    }
}

impl From<DockActionOutcome> for DockSurfaceChange {
    fn from(outcome: DockActionOutcome) -> Self {
        if outcome.changed() {
            Self::Changed
        } else {
            Self::Unchanged
        }
    }
}

/// Error returned when a facade panel operation cannot be applied.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum DockSurfacePanelError {
    /// The panel has no registered metadata.
    #[error("dock item {item} has no registered panel")]
    PanelNotRegistered {
        /// The item that was requested.
        item: DockItemId,
    },
    /// The panel is registered but not closable.
    #[error("dock item {item} is not closable")]
    PanelNotClosable {
        /// The item that was requested.
        item: DockItemId,
    },
    /// The requested panel could not be found at the target location.
    #[error("dock item {item} is not available in the requested dock location")]
    PanelUnavailable {
        /// The item that was requested.
        item: DockItemId,
    },
    /// The requested panel is open but is not currently in an in-window floating container.
    #[error("dock item {item} is not currently floating")]
    PanelNotFloating {
        /// The item that was requested.
        item: DockItemId,
    },
    /// The requested dock target is unavailable.
    #[error("dock target is not currently available")]
    DropTargetUnavailable,
    /// The operation was rejected by docking policy.
    #[error(transparent)]
    Policy(#[from] DockPolicyError),
    /// The low-level docking model rejected the operation.
    #[error("dock model rejected the operation: {message}")]
    Model {
        /// Error message from the low-level model.
        message: String,
    },
    /// The runtime could not complete the operation.
    #[error("dock runtime could not complete the operation: {message}")]
    Runtime {
        /// Runtime failure message.
        message: String,
    },
}

impl From<DockActionApplyError> for DockSurfacePanelError {
    fn from(error: DockActionApplyError) -> Self {
        match error {
            DockActionApplyError::ItemNotInTabs { item, .. } => Self::PanelUnavailable { item },
            DockActionApplyError::PanelNotRegistered { item } => Self::PanelNotRegistered { item },
            DockActionApplyError::PanelNotClosable { item } => Self::PanelNotClosable { item },
            DockActionApplyError::Graph(error) => Self::Model {
                message: error.to_string(),
            },
            DockActionApplyError::Policy(error) => Self::Policy(error),
            DockActionApplyError::DropTargetUnavailable => Self::DropTargetUnavailable,
            DockActionApplyError::TearOffViewportOpenFailed { message } => {
                Self::Runtime { message }
            }
            DockActionApplyError::SurfaceSessionUnavailable
            | DockActionApplyError::DropDragSessionStale { .. }
            | DockActionApplyError::DropDragSessionMissing
            | DockActionApplyError::TearOffViewportPlacementUnavailable
            | DockActionApplyError::DropPayloadMismatch { .. } => Self::Runtime {
                message: error.to_string(),
            },
        }
    }
}

impl DockSurface {
    /// Returns logical dock spaces known to the surface graph.
    pub fn dock_spaces(&self, cx: &App) -> Vec<DockSpaceId> {
        let controller = self.controller(cx);
        cx.read_entity(&controller, |controller, _| controller.graph().spaces())
    }

    /// Returns open item ids reachable from one dock space.
    pub fn items_in_space(&self, space: impl Into<DockSpaceId>, cx: &App) -> Vec<DockItemId> {
        let space = space.into();
        let controller = self.controller(cx);
        cx.read_entity(&controller, |controller, _| {
            controller.graph().collect_items_in_space(&space)
        })
    }

    /// Returns selected panels in stable tree order for one dock space.
    pub fn selected_panels_in_space(
        &self,
        space: impl Into<DockSpaceId>,
        cx: &App,
    ) -> Vec<DockItemId> {
        let space = space.into();
        let controller = self.controller(cx);
        cx.read_entity(&controller, |controller, _| {
            let graph = controller.graph();
            graph
                .tabs_in_space(&space)
                .into_iter()
                .filter_map(|tabs| graph.selected_item_in_tabs(tabs))
                .collect()
        })
    }

    /// Returns the first selected panel in stable tree order for one dock space.
    pub fn selected_panel_in_space(
        &self,
        space: impl Into<DockSpaceId>,
        cx: &App,
    ) -> Option<DockItemId> {
        self.selected_panels_in_space(space, cx).into_iter().next()
    }

    /// Returns in-window floating containers for one dock space.
    pub fn floating_panels_in_space(
        &self,
        space: impl Into<DockSpaceId>,
        cx: &App,
    ) -> Vec<DockSurfaceFloatingPanelSnapshot> {
        let space = space.into();
        let controller = self.controller(cx);
        cx.read_entity(&controller, |controller, _| {
            let graph = controller.graph();
            graph
                .floating_containers(&space)
                .iter()
                .map(|container| {
                    DockSurfaceFloatingPanelSnapshot::new(
                        space.clone(),
                        graph.collect_items_in_subtree(container.node),
                        container.bounds,
                    )
                })
                .collect()
        })
    }

    /// Returns semantic location facts for one open panel.
    pub fn panel_location(
        &self,
        item: impl Into<DockItemId>,
        cx: &App,
    ) -> Option<DockSurfacePanelLocation> {
        let item = item.into();
        let controller = self.controller(cx);
        cx.read_entity(&controller, |controller, _| {
            panel_location(controller, &item)
        })
    }

    /// Returns descriptor and lifecycle snapshots for registered panels in stable item-id order.
    pub fn registered_panels(&self, cx: &App) -> Vec<DockSurfacePanelSnapshot> {
        let controller = self.controller(cx);
        cx.read_entity(&controller, |controller, _| {
            controller
                .panels()
                .descriptors()
                .into_iter()
                .map(|(item, descriptor)| {
                    let location = panel_location(controller, &item);
                    DockSurfacePanelSnapshot::new(
                        item.clone(),
                        descriptor,
                        controller.panels().has_view_lifecycle(&item),
                        location,
                    )
                })
                .collect()
        })
    }

    /// Exports durable dock layout state without exposing the graph controller.
    pub fn export_layout(&self, cx: &App) -> DockLayout {
        let controller = self.controller(cx);
        cx.read_entity(&controller, |controller, _| {
            controller.graph().export_layout()
        })
    }

    /// Opens a registered panel using descriptor last-known or default placement.
    pub fn open_panel(
        &self,
        item: impl Into<DockItemId>,
        cx: &mut App,
    ) -> Result<DockSurfacePanelOutcome, DockSurfacePanelError> {
        let space = self.primary_space.clone();
        self.open_panel_in_space(space, item, cx)
    }

    /// Opens a registered panel in one dock space using descriptor placement metadata.
    pub fn open_panel_in_space(
        &self,
        space: impl Into<DockSpaceId>,
        item: impl Into<DockItemId>,
        cx: &mut App,
    ) -> Result<DockSurfacePanelOutcome, DockSurfacePanelError> {
        self.update_controller(
            cx,
            &[
                DockSurfaceChangeCategory::Layout,
                DockSurfaceChangeCategory::Selection,
                DockSurfaceChangeCategory::PanelLifecycle,
            ],
            |controller| {
                controller
                    .reopen_panel(space, item)
                    .map(DockSurfacePanelOutcome::Opened)
            },
        )
    }

    /// Opens a registered panel at an explicit product placement in the primary dock space.
    pub fn open_panel_at(
        &self,
        placement: DockPanelPlacement,
        cx: &mut App,
    ) -> Result<DockSurfacePanelOutcome, DockSurfacePanelError> {
        let space = self.primary_space.clone();
        self.open_panel_at_in_space(space, placement, cx)
    }

    /// Opens a registered panel at an explicit product placement in one dock space.
    pub fn open_panel_at_in_space(
        &self,
        space: impl Into<DockSpaceId>,
        placement: DockPanelPlacement,
        cx: &mut App,
    ) -> Result<DockSurfacePanelOutcome, DockSurfacePanelError> {
        self.update_controller(
            cx,
            &[
                DockSurfaceChangeCategory::Layout,
                DockSurfaceChangeCategory::Selection,
                DockSurfaceChangeCategory::PanelLifecycle,
            ],
            |controller| {
                controller
                    .open_panel_at_placement(space, placement)
                    .map(DockSurfacePanelOutcome::Opened)
            },
        )
    }

    /// Docks a floating panel back into the primary dock space at an explicit product placement.
    pub fn dock_panel_at(
        &self,
        placement: DockPanelPlacement,
        cx: &mut App,
    ) -> Result<DockSurfacePanelOutcome, DockSurfacePanelError> {
        let space = self.primary_space.clone();
        self.dock_panel_at_in_space(space, placement, cx)
    }

    /// Docks a floating panel back into one dock space at an explicit product placement.
    pub fn dock_panel_at_in_space(
        &self,
        space: impl Into<DockSpaceId>,
        placement: DockPanelPlacement,
        cx: &mut App,
    ) -> Result<DockSurfacePanelOutcome, DockSurfacePanelError> {
        let space = space.into();
        self.update_controller(
            cx,
            &[
                DockSurfaceChangeCategory::Layout,
                DockSurfaceChangeCategory::Selection,
            ],
            |controller| {
                let graph = controller.graph();
                let target_tabs = graph
                    .target_tabs_for_panel_placement(&space, &placement)
                    .ok_or(DockActionApplyError::DropTargetUnavailable)?;
                let floating = graph
                    .floating_containers(&space)
                    .iter()
                    .find(|container| graph.subtree_contains_item(container.node, placement.item()))
                    .map(|container| container.node)
                    .ok_or_else(|| DockActionApplyError::PanelNotRegistered {
                        item: placement.item().clone(),
                    })?;

                controller
                    .merge_floating_into(space, floating, target_tabs)
                    .map(DockSurfaceChange::from)
                    .map(DockSurfacePanelOutcome::Docked)
            },
        )
    }

    /// Closes a registered panel in the primary dock space.
    pub fn close_panel(
        &self,
        item: impl Into<DockItemId>,
        cx: &mut App,
    ) -> Result<DockSurfacePanelOutcome, DockSurfacePanelError> {
        let space = self.primary_space.clone();
        self.close_panel_in_space(space, item, cx)
    }

    /// Closes a registered panel in one dock space.
    pub fn close_panel_in_space(
        &self,
        space: impl Into<DockSpaceId>,
        item: impl Into<DockItemId>,
        cx: &mut App,
    ) -> Result<DockSurfacePanelOutcome, DockSurfacePanelError> {
        self.update_controller(
            cx,
            &[
                DockSurfaceChangeCategory::Layout,
                DockSurfaceChangeCategory::Selection,
                DockSurfaceChangeCategory::PanelLifecycle,
            ],
            |controller| {
                controller
                    .close_panel(space, item)
                    .map(DockSurfacePanelOutcome::Closed)
            },
        )
    }

    /// Selects a registered panel in the primary dock space without exposing tabs node ids.
    pub fn select_panel(
        &self,
        item: impl Into<DockItemId>,
        cx: &mut App,
    ) -> Result<DockSurfacePanelOutcome, DockSurfacePanelError> {
        let space = self.primary_space.clone();
        self.select_panel_in_space(space, item, cx)
    }

    /// Selects a registered panel in one dock space without exposing tabs node ids.
    pub fn select_panel_in_space(
        &self,
        space: impl Into<DockSpaceId>,
        item: impl Into<DockItemId>,
        cx: &mut App,
    ) -> Result<DockSurfacePanelOutcome, DockSurfacePanelError> {
        let space = space.into();
        let item = item.into();
        self.update_controller_with_panel_error(
            cx,
            &[DockSurfaceChangeCategory::Selection],
            |controller| {
                if controller
                    .graph()
                    .find_item_in_space(&space, &item)
                    .is_none()
                {
                    return Err(DockSurfacePanelError::PanelUnavailable { item });
                }

                controller
                    .workspace_mut()
                    .select_item_in_space(space, item)
                    .map(DockSurfaceChange::from)
                    .map(DockSurfacePanelOutcome::Selected)
                    .map_err(DockSurfacePanelError::from)
            },
        )
    }

    /// Moves a panel from the primary dock space into an in-window floating container.
    pub fn float_panel_in_window(
        &self,
        item: impl Into<DockItemId>,
        bounds: Bounds<Pixels>,
        cx: &mut App,
    ) -> Result<DockSurfacePanelOutcome, DockSurfacePanelError> {
        let space = self.primary_space.clone();
        self.float_panel_between_spaces(space.clone(), item, space, bounds, cx)
    }

    /// Moves a panel into an in-window floating container, optionally across dock spaces.
    pub fn float_panel_between_spaces(
        &self,
        source_space: impl Into<DockSpaceId>,
        item: impl Into<DockItemId>,
        target_space: impl Into<DockSpaceId>,
        bounds: Bounds<Pixels>,
        cx: &mut App,
    ) -> Result<DockSurfacePanelOutcome, DockSurfacePanelError> {
        self.update_controller(
            cx,
            &[
                DockSurfaceChangeCategory::Layout,
                DockSurfaceChangeCategory::Selection,
            ],
            |controller| {
                controller
                    .float_item_in_window(source_space, item, target_space, bounds)
                    .map(DockSurfaceChange::from)
                    .map(DockSurfacePanelOutcome::Floated)
            },
        )
    }

    /// Moves an open panel into an empty logical dock space without exposing graph node ids.
    ///
    /// This is the facade helper for product flows that detach a panel into a child dock space
    /// before opening that space with [`DockSurface::viewports`].
    pub fn detach_panel_to_space(
        &self,
        source_space: impl Into<DockSpaceId>,
        item: impl Into<DockItemId>,
        target_space: impl Into<DockSpaceId>,
        cx: &mut App,
    ) -> Result<DockSurfaceChange, DockSurfacePanelError> {
        let source_space = source_space.into();
        let item = item.into();
        let target_space = target_space.into();
        let controller = self.controller(cx);
        with_root_transaction(self.owner(), cx, |owner, transaction, cx| {
            let outcome = cx.update_entity(&controller, |controller, cx| {
                if !controller.panels().contains(&item) {
                    return Err(DockSurfacePanelError::PanelNotRegistered { item });
                }

                let (source_tabs, _) = controller
                    .graph()
                    .find_item_in_space(&source_space, &item)
                    .ok_or_else(|| DockSurfacePanelError::PanelUnavailable {
                        item: item.clone(),
                    })?;

                controller
                    .workspace_mut()
                    .commit_tab_move(
                        &source_space,
                        source_tabs,
                        &item,
                        &target_space,
                        DockGraphDropTarget::empty_space(),
                    )
                    .map(DockSurfaceChange::from)
                    .map_err(DockSurfacePanelError::from)
                    .inspect(|change| {
                        if change.changed() {
                            cx.notify();
                        }
                    })
            });
            if outcome.as_ref().is_ok_and(|change| change.changed()) {
                owner.record_changes(
                    transaction,
                    [
                        DockSurfaceChangeCategory::Layout,
                        DockSurfaceChangeCategory::Selection,
                    ],
                );
            }
            outcome
        })
    }

    /// Updates the bounds of an in-window floating panel container in the primary dock space.
    pub fn set_floating_panel_bounds(
        &self,
        item: impl Into<DockItemId>,
        bounds: Bounds<Pixels>,
        cx: &mut App,
    ) -> Result<DockSurfacePanelOutcome, DockSurfacePanelError> {
        let space = self.primary_space.clone();
        self.set_floating_panel_bounds_in_space(space, item, bounds, cx)
    }

    /// Updates the bounds of an in-window floating panel container without exposing node ids.
    pub fn set_floating_panel_bounds_in_space(
        &self,
        space: impl Into<DockSpaceId>,
        item: impl Into<DockItemId>,
        bounds: Bounds<Pixels>,
        cx: &mut App,
    ) -> Result<DockSurfacePanelOutcome, DockSurfacePanelError> {
        let space = space.into();
        let item = item.into();
        self.update_controller_with_panel_error(
            cx,
            &[DockSurfaceChangeCategory::Layout],
            |controller| {
                let floating = floating_container_for_item(controller, &space, &item)?;
                controller
                    .set_floating_bounds(space, floating, bounds)
                    .map(DockSurfaceChange::from)
                    .map(DockSurfacePanelOutcome::FloatingBoundsSet)
                    .map_err(DockSurfacePanelError::from)
            },
        )
    }

    /// Raises an in-window floating panel container in the primary dock space.
    pub fn raise_floating_panel(
        &self,
        item: impl Into<DockItemId>,
        cx: &mut App,
    ) -> Result<DockSurfacePanelOutcome, DockSurfacePanelError> {
        let space = self.primary_space.clone();
        self.raise_floating_panel_in_space(space, item, cx)
    }

    /// Raises an in-window floating panel container without exposing node ids.
    pub fn raise_floating_panel_in_space(
        &self,
        space: impl Into<DockSpaceId>,
        item: impl Into<DockItemId>,
        cx: &mut App,
    ) -> Result<DockSurfacePanelOutcome, DockSurfacePanelError> {
        let space = space.into();
        let item = item.into();
        self.update_controller_with_panel_error(
            cx,
            &[DockSurfaceChangeCategory::Layout],
            |controller| {
                let floating = floating_container_for_item(controller, &space, &item)?;
                controller
                    .raise_floating(space, floating)
                    .map(DockSurfaceChange::from)
                    .map(DockSurfacePanelOutcome::FloatingRaised)
                    .map_err(DockSurfacePanelError::from)
            },
        )
    }

    fn update_controller(
        &self,
        cx: &mut App,
        categories: &[DockSurfaceChangeCategory],
        update: impl FnOnce(
            &mut DockController,
        ) -> Result<DockSurfacePanelOutcome, DockActionApplyError>,
    ) -> Result<DockSurfacePanelOutcome, DockSurfacePanelError> {
        self.update_controller_with_panel_error(cx, categories, |controller| {
            update(controller).map_err(DockSurfacePanelError::from)
        })
    }

    fn update_controller_with_panel_error(
        &self,
        cx: &mut App,
        categories: &[DockSurfaceChangeCategory],
        update: impl FnOnce(
            &mut DockController,
        ) -> Result<DockSurfacePanelOutcome, DockSurfacePanelError>,
    ) -> Result<DockSurfacePanelOutcome, DockSurfacePanelError> {
        let controller = self.controller(cx);
        with_root_transaction(self.owner(), cx, |owner, transaction, cx| {
            let outcome = cx.update_entity(&controller, |controller, cx| {
                let outcome = update(controller);
                if outcome
                    .as_ref()
                    .map(DockSurfacePanelOutcome::changed)
                    .unwrap_or(false)
                {
                    cx.notify();
                }
                outcome
            });
            if outcome
                .as_ref()
                .map(DockSurfacePanelOutcome::changed)
                .unwrap_or(false)
            {
                owner.record_changes(transaction, categories.iter().copied());
            }
            outcome
        })
    }
}

fn panel_location(
    controller: &DockController,
    item: &DockItemId,
) -> Option<DockSurfacePanelLocation> {
    let graph = controller.graph();
    graph
        .spaces()
        .into_iter()
        .find_map(|space| panel_location_in_space(graph, space, item))
}

fn panel_location_in_space(
    graph: &crate::DockGraph,
    space: DockSpaceId,
    item: &DockItemId,
) -> Option<DockSurfacePanelLocation> {
    let (_, tab_index) = graph.find_item_in_space(&space, item)?;
    let kind = if graph
        .root(&space)
        .is_some_and(|root| graph.subtree_contains_item(root, item))
    {
        DockSurfacePanelLocationKind::Docked
    } else {
        DockSurfacePanelLocationKind::Floating
    };
    Some(DockSurfacePanelLocation::new(space, kind, tab_index))
}

fn floating_container_for_item(
    controller: &DockController,
    space: &DockSpaceId,
    item: &DockItemId,
) -> Result<DockNodeId, DockSurfacePanelError> {
    let graph = controller.graph();
    if graph.find_item_in_space(space, item).is_none() {
        return Err(DockSurfacePanelError::PanelUnavailable { item: item.clone() });
    }

    graph
        .floating_containers(space)
        .iter()
        .find(|container| graph.subtree_contains_item(container.node, item))
        .map(|container| container.node)
        .ok_or_else(|| DockSurfacePanelError::PanelNotFloating { item: item.clone() })
}
