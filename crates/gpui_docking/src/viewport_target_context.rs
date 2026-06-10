#[cfg(test)]
use open_gpui::AnyWindowHandle;
use open_gpui::WindowId;

/// Platform facts used to arbitrate overlapping viewport hits.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct DockViewportTargetContext {
    /// Window that produced the route event, when known.
    pub(crate) event_window: Option<WindowId>,
    /// Platform-active window, when known.
    pub(crate) active_window: Option<WindowId>,
    /// Front-to-back window stack, when the platform provides it.
    pub(crate) window_stack: Vec<WindowId>,
}

impl DockViewportTargetContext {
    /// Creates an empty target context that falls back to deterministic adapter ordering.
    #[cfg(test)]
    pub(crate) fn new() -> Self {
        Self::default()
    }

    /// Adds the event window signal.
    #[cfg(test)]
    pub(crate) fn with_event_window(mut self, window: impl Into<AnyWindowHandle>) -> Self {
        self.event_window = Some(window.into().window_id());
        self
    }

    /// Adds the active window signal.
    #[cfg(test)]
    pub(crate) fn with_active_window(mut self, window: impl Into<AnyWindowHandle>) -> Self {
        self.active_window = Some(window.into().window_id());
        self
    }

    /// Adds the front-to-back window stack signal.
    #[cfg(test)]
    pub(crate) fn with_window_stack(
        mut self,
        windows: impl IntoIterator<Item = impl Into<AnyWindowHandle>>,
    ) -> Self {
        self.window_stack = windows
            .into_iter()
            .map(|window| window.into().window_id())
            .collect();
        self
    }
}
