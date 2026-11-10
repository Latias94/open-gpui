mod window;

use std::rc::Rc;

use open_gpui::{AnyWindowHandle, PlatformDisplay, PlatformWindow, WindowParams};

pub(crate) use window::HeadlessWindow;

/// The state of a `LinuxPlatform` with no display server.
///
/// It reports no displays and opens [`HeadlessWindow`]s, which lay out and handle input but draw
/// nothing.
pub(crate) struct HeadlessConnection {
    /// Cloned into every open window, so its count tells whether windows are open.
    window_lease: Rc<()>,
}

impl HeadlessConnection {
    pub(crate) fn new() -> Self {
        Self {
            window_lease: Rc::new(()),
        }
    }

    pub(crate) fn has_windows(&self) -> bool {
        Rc::strong_count(&self.window_lease) > 1
    }

    pub(crate) fn displays(&self) -> Vec<Rc<dyn PlatformDisplay>> {
        Vec::new()
    }

    pub(crate) fn primary_display(&self) -> Option<Rc<dyn PlatformDisplay>> {
        None
    }

    pub(crate) fn open_window(
        &self,
        _handle: AnyWindowHandle,
        params: WindowParams,
    ) -> anyhow::Result<Box<dyn PlatformWindow>> {
        Ok(Box::new(HeadlessWindow::new(
            params,
            self.window_lease.clone(),
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn headless_client_has_no_physical_displays() {
        let connection = HeadlessConnection::new();
        assert!(connection.displays().is_empty());
        assert!(connection.primary_display().is_none());
        assert!(
            connection
                .displays()
                .into_iter()
                .find(|display| display.id() == open_gpui::DisplayId::new(0))
                .is_none()
        );
    }

    #[test]
    fn retained_window_lease_blocks_connection_replacement() {
        let connection = HeadlessConnection::new();
        assert!(!connection.has_windows());
        let lease = connection.window_lease.clone();
        assert!(connection.has_windows());
        drop(lease);
        assert!(!connection.has_windows());
    }
}
