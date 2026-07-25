use crate::{
    DockLayoutRect, DockViewportAdapter, DockViewportPlacement, DockViewportPlacementLayout,
    DockViewportPlacementValidationError, DockViewportWindowBounds,
};

/// Summary of checking saved placement against currently registered runtime windows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DockViewportRestoreReadiness {
    /// Number of saved placement entries with a currently registered runtime window.
    pub matched: usize,
    /// Number of saved placement entries without a currently registered runtime window.
    pub missing: usize,
}

impl DockViewportAdapter {
    /// Exports serializable placement snapshots for all registered viewports.
    pub(crate) fn export_placement(&self) -> DockViewportPlacementLayout {
        DockViewportPlacementLayout::new(
            self.spaces()
                .into_iter()
                .filter_map(|space| {
                    let snapshot = self.snapshot(&space)?;
                    Some(DockViewportPlacement {
                        space,
                        display_id: snapshot.display_id.map(u64::from),
                        window_bounds: snapshot
                            .window_bounds
                            .map(DockViewportWindowBounds::from_window_bounds),
                        host_bounds: snapshot
                            .host_geometry
                            .as_ref()
                            .map(|geometry| DockLayoutRect::from_bounds(geometry.layout_bounds())),
                    })
                })
                .collect(),
        )
    }

    /// Checks placement snapshots against already registered viewport windows.
    ///
    /// This does not open, move, or resize windows. Saved placement should be converted into
    /// `WindowOptions` before opening a viewport; live snapshots are then refreshed by render
    /// frames from real platform window facts.
    pub(crate) fn check_placement_restore(
        &mut self,
        placement: &DockViewportPlacementLayout,
    ) -> Result<DockViewportRestoreReadiness, DockViewportPlacementValidationError> {
        placement.validate()?;

        let mut matched = 0;
        let mut missing = 0;
        for viewport in &placement.viewports {
            if self.snapshot(&viewport.space).is_none() {
                missing += 1;
                continue;
            }
            matched += 1;
        }

        Ok(DockViewportRestoreReadiness { matched, missing })
    }
}
