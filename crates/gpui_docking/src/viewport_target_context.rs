#[cfg(test)]
use open_gpui::AnyWindowHandle;
use open_gpui::WindowId;

/// Platform facts used to arbitrate overlapping viewport hits.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct DockViewportTargetContext {
    /// Window known to be under the pointer for this docking route event.
    hovered_window: Option<WindowId>,
    /// Platform-active window, when known.
    active_window: Option<WindowId>,
    /// Front-to-back window stack, when the platform provides it.
    window_stack: Vec<WindowId>,
}

impl DockViewportTargetContext {
    pub(crate) fn from_window_signals(
        hovered_window: Option<WindowId>,
        active_window: Option<WindowId>,
        window_stack: Vec<WindowId>,
    ) -> Self {
        Self {
            hovered_window,
            active_window,
            window_stack,
        }
    }

    pub(crate) fn hovered_window(&self) -> Option<WindowId> {
        self.hovered_window
    }

    pub(crate) fn active_window(&self) -> Option<WindowId> {
        self.active_window
    }

    pub(crate) fn window_stack(&self) -> &[WindowId] {
        &self.window_stack
    }

    #[cfg(test)]
    pub(crate) fn into_window_signals(self) -> (Option<WindowId>, Option<WindowId>, Vec<WindowId>) {
        (self.hovered_window, self.active_window, self.window_stack)
    }

    /// Creates an empty target context that falls back to deterministic adapter ordering.
    #[cfg(test)]
    pub(crate) fn new() -> Self {
        Self::default()
    }

    /// Adds the hovered window signal.
    #[cfg(test)]
    pub(crate) fn with_hovered_window(mut self, window: impl Into<AnyWindowHandle>) -> Self {
        self.hovered_window = Some(window.into().window_id());
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
