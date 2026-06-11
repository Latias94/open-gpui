use crate::{
    DockLayoutRect, DockViewportAdapter, DockViewportPlacement, DockViewportPlacementLayout,
    DockViewportPlacementValidationError, DockViewportWindowBounds,
};

/// Summary of applying saved viewport placement to runtime windows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DockViewportRestoreOutcome {
    /// Number of saved placement entries matched to registered windows.
    pub applied: usize,
    /// Number of saved placement entries skipped because no runtime window was registered.
    pub skipped: usize,
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
                        host_bounds: snapshot.host_bounds.map(DockLayoutRect::from_bounds),
                    })
                })
                .collect(),
        )
    }

    /// Validates placement snapshots against already registered viewport windows.
    ///
    /// This does not open, move, or resize windows. Saved placement should be converted into
    /// `WindowOptions` before opening a viewport; live snapshots are then refreshed by render
    /// frames from real platform window facts.
    pub(crate) fn apply_placement(
        &mut self,
        placement: &DockViewportPlacementLayout,
    ) -> Result<DockViewportRestoreOutcome, DockViewportPlacementValidationError> {
        placement.validate()?;

        let mut applied = 0;
        let mut skipped = 0;
        for viewport in &placement.viewports {
            if self.snapshot(&viewport.space).is_none() {
                skipped += 1;
                continue;
            }
            applied += 1;
        }

        Ok(DockViewportRestoreOutcome { applied, skipped })
    }
}
