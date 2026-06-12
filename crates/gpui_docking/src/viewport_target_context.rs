#[cfg(test)]
use open_gpui::AnyWindowHandle;
use open_gpui::WindowId;

/// Pure resolver context derived from platform window signals.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct DockViewportTargetContext {
    /// Window known to be under the pointer for this docking route event.
    hovered_window: Option<WindowId>,
    /// Platform-active window, when known.
    active_window: Option<WindowId>,
    /// Front-to-back window stack, when the platform provides it.
    window_stack: Vec<WindowId>,
}

/// Sort key for choosing between overlapping viewport targets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct DockViewportTargetPriority {
    hovered: usize,
    active: usize,
    stacked: usize,
    fallback: usize,
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

    pub(crate) fn priority_for_window(
        &self,
        window_id: WindowId,
        fallback: usize,
    ) -> DockViewportTargetPriority {
        DockViewportTargetPriority {
            hovered: self
                .hovered_window
                .map(|hovered| usize::from(hovered != window_id))
                .unwrap_or(1),
            active: self
                .active_window
                .map(|active| usize::from(active != window_id))
                .unwrap_or(1),
            stacked: self
                .window_stack
                .iter()
                .position(|stacked| *stacked == window_id)
                .unwrap_or(usize::MAX),
            fallback,
        }
    }

    pub(crate) fn has_arbitration_signal(&self) -> bool {
        self.hovered_window.is_some()
            || self.active_window.is_some()
            || !self.window_stack.is_empty()
    }

    pub(crate) fn is_trusted_window(&self, window_id: WindowId) -> bool {
        self.hovered_window == Some(window_id)
            || self.active_window == Some(window_id)
            || self.window_stack.contains(&window_id)
    }

    pub(crate) fn hovered_window(&self) -> Option<WindowId> {
        self.hovered_window
    }

    #[cfg(test)]
    pub(crate) fn active_window(&self) -> Option<WindowId> {
        self.active_window
    }

    #[cfg(test)]
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

#[cfg(test)]
mod tests {
    use super::*;
    use open_gpui::WindowId;

    #[test]
    fn target_priority_prefers_hovered_active_stack_then_fallback() {
        let first = WindowId::from(1);
        let second = WindowId::from(2);
        let third = WindowId::from(3);
        let context = DockViewportTargetContext::from_window_signals(
            Some(third),
            Some(second),
            vec![second, first],
        );

        assert!(
            context.priority_for_window(third, 2) < context.priority_for_window(second, 1),
            "hovered window should beat active window"
        );
        assert!(
            context.priority_for_window(second, 1) < context.priority_for_window(first, 0),
            "active window should beat window stack order"
        );
        assert!(
            context.priority_for_window(first, 0)
                < context.priority_for_window(WindowId::from(4), 3),
            "window stack membership should beat fallback order"
        );
    }
}
