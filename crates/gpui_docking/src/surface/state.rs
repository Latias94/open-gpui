use super::DockSurface;
use crate::{DockLayout, DockViewportPlacementLayout};
use open_gpui::{App, AppContext as _};
use serde::{Deserialize, Serialize};

/// Serializable application-level snapshot for one docking surface.
///
/// The snapshot combines durable dock layout with facade-opened platform viewport placement hints.
/// It never stores live GPUI views or platform window handles.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DockSurfaceSnapshot {
    #[serde(default)]
    revision: u64,
    layout: DockLayout,
    viewport_placement: DockViewportPlacementLayout,
}

impl DockSurfaceSnapshot {
    /// Creates a snapshot from durable layout and viewport placement data.
    pub fn new(layout: DockLayout, viewport_placement: DockViewportPlacementLayout) -> Self {
        Self {
            revision: 0,
            layout,
            viewport_placement,
        }
    }

    pub(crate) fn from_committed_parts(
        revision: u64,
        layout: DockLayout,
        viewport_placement: DockViewportPlacementLayout,
    ) -> Self {
        Self {
            revision,
            layout,
            viewport_placement,
        }
    }

    /// Returns the committed surface revision paired with this snapshot.
    pub const fn revision(&self) -> u64 {
        self.revision
    }

    /// Returns the durable dock layout portion of this snapshot.
    pub fn layout(&self) -> &DockLayout {
        &self.layout
    }

    /// Returns the platform viewport placement portion of this snapshot.
    pub fn viewport_placement(&self) -> &DockViewportPlacementLayout {
        &self.viewport_placement
    }

    /// Consumes this snapshot into its durable layout and viewport placement parts.
    pub fn into_parts(self) -> (DockLayout, DockViewportPlacementLayout) {
        (self.layout, self.viewport_placement)
    }
}

impl DockSurface {
    /// Exports one revision-consistent durable layout and viewport-placement snapshot.
    pub fn export_snapshot(&self, cx: &App) -> DockSurfaceSnapshot {
        cx.read_entity(self.owner(), |owner, cx| {
            let controller = owner.controller();
            let layout = cx.read_entity(&controller, |controller, _| {
                controller.graph().export_layout()
            });
            DockSurfaceSnapshot::from_committed_parts(
                owner.revision(),
                layout,
                owner.runtime().export_placement(),
            )
        })
    }
}
