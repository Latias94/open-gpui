use super::DockSurface;
use crate::{
    DockActionApplyError, DockActionOutcome, DockController, DockItemId, DockPanelCloseOutcome,
    DockPanelOpenOutcome, DockPanelPlacement, DockPolicyError, DockSpaceId,
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
}

/// App-level change flag for facade panel operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DockSurfaceChange {
    /// The operation changed docking state.
    Changed,
    /// The operation was valid but left docking state unchanged.
    Unchanged,
}

impl DockSurfacePanelOutcome {
    /// Returns true when the operation changed docking graph state.
    pub fn changed(&self) -> bool {
        match self {
            Self::Opened(outcome) => outcome.changed(),
            Self::Closed(outcome) => outcome.changed(),
            Self::Floated(change) | Self::Docked(change) => change.changed(),
        }
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
            DockActionApplyError::DropDragSessionStale { .. }
            | DockActionApplyError::DropDragSessionMissing
            | DockActionApplyError::TearOffViewportPlacementUnavailable
            | DockActionApplyError::DropPayloadMismatch { .. } => Self::Runtime {
                message: error.to_string(),
            },
        }
    }
}

impl DockSurface {
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
        self.update_controller(cx, |controller| {
            controller
                .reopen_panel(space, item)
                .map(DockSurfacePanelOutcome::Opened)
        })
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
        self.update_controller(cx, |controller| {
            controller
                .open_panel_at_placement(space, placement)
                .map(DockSurfacePanelOutcome::Opened)
        })
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
        self.update_controller(cx, |controller| {
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
        })
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
        self.update_controller(cx, |controller| {
            controller
                .close_panel(space, item)
                .map(DockSurfacePanelOutcome::Closed)
        })
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
        self.update_controller(cx, |controller| {
            controller
                .float_item_in_window(source_space, item, target_space, bounds)
                .map(DockSurfaceChange::from)
                .map(DockSurfacePanelOutcome::Floated)
        })
    }

    fn update_controller(
        &self,
        cx: &mut App,
        update: impl FnOnce(
            &mut DockController,
        ) -> Result<DockSurfacePanelOutcome, DockActionApplyError>,
    ) -> Result<DockSurfacePanelOutcome, DockSurfacePanelError> {
        let controller = self.controller.clone();
        cx.update_entity(&controller, |controller, cx| {
            let outcome = update(controller);
            if outcome
                .as_ref()
                .map(DockSurfacePanelOutcome::changed)
                .unwrap_or(false)
            {
                cx.notify();
            }
            outcome
        })
        .map_err(DockSurfacePanelError::from)
    }
}
