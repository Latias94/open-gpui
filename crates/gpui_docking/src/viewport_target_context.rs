#[cfg(test)]
use open_gpui::AnyWindowHandle;
use open_gpui::WindowId;

/// Platform facts used to arbitrate overlapping viewport hits.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct DockViewportTargetContext {
    /// Window that produced the route event, when known.
    event_window: Option<WindowId>,
    /// Platform-active window, when known.
    active_window: Option<WindowId>,
    /// Front-to-back window stack, when the platform provides it.
    window_stack: Vec<WindowId>,
}

impl DockViewportTargetContext {
    pub(crate) fn from_window_signals(
        event_window: Option<WindowId>,
        active_window: Option<WindowId>,
        window_stack: Vec<WindowId>,
    ) -> Self {
        Self {
            event_window,
            active_window,
            window_stack,
        }
    }

    pub(crate) fn event_window(&self) -> Option<WindowId> {
        self.event_window
    }

    pub(crate) fn active_window(&self) -> Option<WindowId> {
        self.active_window
    }

    pub(crate) fn window_stack(&self) -> &[WindowId] {
        &self.window_stack
    }

    #[cfg(test)]
    pub(crate) fn into_window_signals(self) -> (Option<WindowId>, Option<WindowId>, Vec<WindowId>) {
        (self.event_window, self.active_window, self.window_stack)
    }

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
