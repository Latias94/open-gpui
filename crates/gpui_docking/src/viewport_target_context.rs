#[cfg(test)]
use open_gpui::AnyWindowHandle;
use open_gpui::WindowId;

/// Pure resolver context derived from platform window signals.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct DockViewportTargetContext {
    /// Window reported by the platform as being under the pointer.
    platform_hovered_window: Option<WindowId>,
    /// Whether the platform hovered-window signal is reliable for this snapshot, including
    /// the case where no application window is hovered.
    platform_hovered_window_known: bool,
    /// Window that delivered the GPUI drag/drop event.
    event_receiver_window: Option<WindowId>,
    /// Front-to-back window stack, when the platform provides it.
    window_stack: Vec<WindowId>,
}

/// Sort key for choosing between overlapping viewport targets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct DockViewportTargetPriority {
    hovered: usize,
    stacked: usize,
    fallback: usize,
}

impl DockViewportTargetContext {
    #[cfg(test)]
    pub(crate) fn from_window_signals(
        platform_hovered_window: Option<WindowId>,
        window_stack: Vec<WindowId>,
    ) -> Self {
        Self::from_window_and_event_signals(platform_hovered_window, None, window_stack)
    }

    pub(crate) fn from_window_and_event_signals(
        platform_hovered_window: Option<WindowId>,
        event_receiver_window: Option<WindowId>,
        window_stack: Vec<WindowId>,
    ) -> Self {
        Self::from_window_and_event_signals_with_hovered_known(
            platform_hovered_window,
            platform_hovered_window.is_some(),
            event_receiver_window,
            window_stack,
        )
    }

    pub(crate) fn from_window_and_event_signals_with_hovered_known(
        platform_hovered_window: Option<WindowId>,
        platform_hovered_window_known: bool,
        event_receiver_window: Option<WindowId>,
        window_stack: Vec<WindowId>,
    ) -> Self {
        Self {
            platform_hovered_window,
            platform_hovered_window_known,
            event_receiver_window,
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
                .platform_hovered_window
                .map(|hovered| usize::from(hovered != window_id))
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
        self.platform_hovered_window.is_some()
            || self.platform_hovered_window_known
            || self.event_receiver_window.is_some()
            || !self.window_stack.is_empty()
    }

    pub(crate) fn has_signal_for_window(&self, window_id: WindowId) -> bool {
        self.platform_hovered_window == Some(window_id)
            || self.event_receiver_window == Some(window_id)
            || self.window_stack.contains(&window_id)
    }

    pub(crate) fn has_unmatched_arbitration_signal(&self, window_id: WindowId) -> bool {
        self.has_arbitration_signal() && !self.has_signal_for_window(window_id)
    }

    pub(crate) fn hovered_window(&self) -> Option<WindowId> {
        self.platform_hovered_window
    }

    pub(crate) fn hovered_window_known_empty(&self) -> bool {
        self.platform_hovered_window_known && self.platform_hovered_window.is_none()
    }

    pub(crate) fn event_receiver_window(&self) -> Option<WindowId> {
        self.event_receiver_window
    }

    pub(crate) fn window_stack(&self) -> &[WindowId] {
        &self.window_stack
    }

    #[cfg(test)]
    pub(crate) fn into_window_signals(self) -> (Option<WindowId>, Option<WindowId>, Vec<WindowId>) {
        (
            self.platform_hovered_window,
            self.event_receiver_window,
            self.window_stack,
        )
    }

    /// Creates an empty target context that falls back to deterministic adapter ordering.
    #[cfg(test)]
    pub(crate) fn new() -> Self {
        Self::default()
    }

    /// Adds the hovered window signal.
    #[cfg(test)]
    pub(crate) fn with_hovered_window(mut self, window: impl Into<AnyWindowHandle>) -> Self {
        self.platform_hovered_window = Some(window.into().window_id());
        self.platform_hovered_window_known = true;
        self
    }

    /// Marks the platform hovered window signal as reliable and currently empty.
    #[cfg(test)]
    pub(crate) fn with_hovered_window_known_empty(mut self) -> Self {
        self.platform_hovered_window = None;
        self.platform_hovered_window_known = true;
        self
    }

    /// Adds the event receiver window signal.
    #[cfg(test)]
    pub(crate) fn with_event_receiver_window(mut self, window: impl Into<AnyWindowHandle>) -> Self {
        self.event_receiver_window = Some(window.into().window_id());
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
    fn target_priority_prefers_hovered_stack_then_fallback() {
        let first = WindowId::from(1);
        let second = WindowId::from(2);
        let third = WindowId::from(3);
        let context =
            DockViewportTargetContext::from_window_signals(Some(third), vec![second, first]);

        assert!(
            context.priority_for_window(third, 2) < context.priority_for_window(second, 1),
            "hovered window should beat window-stack membership"
        );
        assert!(
            context.priority_for_window(first, 0)
                < context.priority_for_window(WindowId::from(4), 3),
            "window stack membership should beat fallback order"
        );
    }
}
