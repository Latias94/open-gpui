use crate::{
    DockSpaceId, DockViewportPlacementLayout, DockViewportPlacementValidationError,
    DockViewportWindowBounds,
};
use open_gpui::{DisplayId, WindowOptions};

impl DockViewportPlacementLayout {
    /// Applies saved platform-window placement to fallback GPUI window options.
    ///
    /// This validates the placement layout before returning options so restore flows can reject
    /// corrupt placement data before opening runtime windows.
    pub fn window_options_for_space(
        &self,
        space: &DockSpaceId,
        mut fallback: WindowOptions,
    ) -> Result<WindowOptions, DockViewportPlacementValidationError> {
        self.validate()?;

        if let Some(placement) = self.placement_for_space(space) {
            if let Some(display_id) = placement.display_id {
                fallback.display_id = Some(DisplayId::from(display_id));
            }
            fallback.window_bounds = placement
                .window_bounds
                .map(DockViewportWindowBounds::to_window_bounds)
                .or(fallback.window_bounds);
        }

        Ok(fallback)
    }
}
