use crate::{
    DockLayoutRect, DockViewportAdapter, DockViewportPlacement, DockViewportPlacementLayout,
    DockViewportPlacementValidationError, DockViewportWindowBounds,
};
use open_gpui::DisplayId;

/// Summary of applying saved viewport placement to runtime windows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DockViewportRestoreOutcome {
    /// Number of saved placement entries applied to registered windows.
    pub applied: usize,
    /// Number of saved placement entries skipped because no runtime window was registered.
    pub skipped: usize,
}

impl DockViewportAdapter {
    /// Exports serializable placement snapshots for all registered viewports.
    pub fn export_placement(&self) -> DockViewportPlacementLayout {
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

    /// Applies placement snapshots to already registered viewport windows.
    ///
    /// This does not open windows or create viewport mappings. Applications should first register
    /// the windows they restored, then apply placement data to rehydrate adapter snapshots.
    pub fn apply_placement(
        &mut self,
        placement: &DockViewportPlacementLayout,
    ) -> Result<DockViewportRestoreOutcome, DockViewportPlacementValidationError> {
        placement.validate()?;

        let mut applied = 0;
        let mut skipped = 0;
        for viewport in &placement.viewports {
            let Some(snapshot) = self.snapshot_mut(&viewport.space) else {
                skipped += 1;
                continue;
            };
            snapshot.display_id = viewport.display_id.map(DisplayId::from);
            snapshot.window_bounds = viewport
                .window_bounds
                .map(DockViewportWindowBounds::to_window_bounds);
            snapshot.host_bounds = viewport.host_bounds.map(DockLayoutRect::to_bounds);
            applied += 1;
        }

        Ok(DockViewportRestoreOutcome { applied, skipped })
    }
}
