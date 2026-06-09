use crate::{DockHost, debug::DockDebugRegion};

impl DockHost {
    /// Returns a debug selector emitted for a test region during the most recent render.
    #[cfg(test)]
    pub(crate) fn debug_selector(&self, region: &DockDebugRegion) -> Option<&str> {
        self.debug_instrumentation().selector(region)
    }

    pub(crate) fn clear_debug_selectors(&mut self) {
        #[cfg(test)]
        self.debug_instrumentation_mut().clear();
    }

    pub(crate) fn record_debug_selector(
        &mut self,
        region: DockDebugRegion,
        selector: String,
    ) -> String {
        #[cfg(test)]
        {
            self.debug_instrumentation_mut().record(region, selector)
        }
        #[cfg(not(test))]
        {
            let _ = region;
            selector
        }
    }
}
