use super::DockSurface;
use crate::{DockLayout, DockViewportPlacementLayout};
use open_gpui::App;
use serde::{Deserialize, Serialize};

/// Serializable application-level snapshot for one docking surface.
///
/// The snapshot combines durable dock layout with facade-opened platform viewport placement hints.
/// It never stores live GPUI views or platform window handles.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DockSurfaceSnapshot {
    layout: DockLayout,
    viewport_placement: DockViewportPlacementLayout,
}

impl DockSurfaceSnapshot {
    /// Creates a snapshot from durable layout and viewport placement data.
    pub fn new(layout: DockLayout, viewport_placement: DockViewportPlacementLayout) -> Self {
        Self {
            layout,
            viewport_placement,
        }
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
    /// Exports durable layout and facade-opened viewport placement as one app-level snapshot.
    pub fn export_snapshot(&self, cx: &App) -> DockSurfaceSnapshot {
        DockSurfaceSnapshot::new(self.export_layout(cx), self.export_viewport_placement())
    }
}
