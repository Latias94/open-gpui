use open_gpui::{AnyWindowHandle, WindowId};

/// Front-to-back platform window stack captured for fallback ordering.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct DockViewportFrontToBackWindowStack {
    windows: Vec<WindowId>,
}

impl DockViewportFrontToBackWindowStack {
    pub(crate) fn from_window_ids(windows: impl IntoIterator<Item = WindowId>) -> Self {
        let mut normalized = Vec::new();
        for window_id in windows {
            if normalized.iter().any(|existing| *existing == window_id) {
                continue;
            }
            normalized.push(window_id);
        }
        Self {
            windows: normalized,
        }
    }

    pub(crate) fn from_windows<I, W>(windows: I) -> Self
    where
        I: IntoIterator<Item = W>,
        W: Into<AnyWindowHandle>,
    {
        Self::from_window_ids(windows.into_iter().map(|window| window.into().window_id()))
    }

    pub(crate) fn as_slice(&self) -> &[WindowId] {
        &self.windows
    }
}

/// Trusted backend hovered-window signal captured for this routing snapshot.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) enum DockViewportTrustedHoveredSignal {
    /// The backend did not provide a reliable hovered-window signal for this snapshot.
    #[default]
    Unavailable,
    /// The backend reported that no application window is currently hovered.
    TrustedNone,
    /// The backend reported the application window under the pointer.
    Trusted(WindowId),
}

/// Pure resolver context derived from platform window signals.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct DockViewportTargetContext {
    /// Trusted backend hovered-window signal.
    trusted_hovered_signal: DockViewportTrustedHoveredSignal,
    /// Front-to-back window stack, when the platform provides it.
    window_stack: DockViewportFrontToBackWindowStack,
}

impl DockViewportTrustedHoveredSignal {
    #[cfg(test)]
    pub(crate) fn from_parts(window: Option<WindowId>, known: bool) -> Self {
        match (window, known) {
            (Some(window), _) => Self::Trusted(window),
            (None, true) => Self::TrustedNone,
            (None, false) => Self::Unavailable,
        }
    }

    pub(crate) fn trusted_window(self) -> Option<WindowId> {
        match self {
            Self::Trusted(window) => Some(window),
            Self::TrustedNone | Self::Unavailable => None,
        }
    }
}

impl DockViewportTargetContext {
    pub(crate) fn from_window_signals(
        trusted_hovered_signal: DockViewportTrustedHoveredSignal,
        window_stack: DockViewportFrontToBackWindowStack,
    ) -> Self {
        Self {
            trusted_hovered_signal,
            window_stack,
        }
    }

    #[cfg(test)]
    pub(crate) fn from_window_signals_with_hovered_known(
        platform_hovered_window: Option<WindowId>,
        platform_hovered_window_known: bool,
        window_stack: Vec<WindowId>,
    ) -> Self {
        Self::from_window_signals(
            DockViewportTrustedHoveredSignal::from_parts(
                platform_hovered_window,
                platform_hovered_window_known,
            ),
            DockViewportFrontToBackWindowStack::from_window_ids(window_stack),
        )
    }

    pub(crate) fn trusted_hovered_window(&self) -> Option<WindowId> {
        self.trusted_hovered_signal.trusted_window()
    }

    pub(crate) fn trusted_hovered_window_matches_event_receiver(
        &self,
        event_receiver_window: Option<WindowId>,
    ) -> bool {
        matches!(
            (self.trusted_hovered_window(), event_receiver_window),
            (Some(trusted_hovered_window), Some(event_receiver_window))
                if trusted_hovered_window == event_receiver_window
        )
    }

    pub(crate) fn trusted_hovered_signal(&self) -> DockViewportTrustedHoveredSignal {
        self.trusted_hovered_signal
    }

    pub(crate) fn backend_hover_fallback_window_stack(&self) -> &[WindowId] {
        self.window_stack.as_slice()
    }

    #[cfg(test)]
    pub(crate) fn into_window_signals(
        self,
    ) -> (
        DockViewportTrustedHoveredSignal,
        DockViewportFrontToBackWindowStack,
    ) {
        (self.trusted_hovered_signal, self.window_stack)
    }

    /// Creates an empty target context that only supports diagnostic ordering.
    #[cfg(test)]
    pub(crate) fn new() -> Self {
        Self::default()
    }

    /// Adds the trusted backend hovered-window signal.
    #[cfg(test)]
    pub(crate) fn with_trusted_hovered_window(
        mut self,
        window: impl Into<AnyWindowHandle>,
    ) -> Self {
        self.trusted_hovered_signal =
            DockViewportTrustedHoveredSignal::Trusted(window.into().window_id());
        self
    }

    /// Marks the trusted backend hovered-window signal as reliable and currently empty.
    #[cfg(test)]
    pub(crate) fn with_trusted_hovered_window_known_empty(mut self) -> Self {
        self.trusted_hovered_signal = DockViewportTrustedHoveredSignal::TrustedNone;
        self
    }

    /// Adds the front-to-back window stack signal.
    #[cfg(test)]
    pub(crate) fn with_window_stack(
        mut self,
        windows: impl IntoIterator<Item = impl Into<AnyWindowHandle>>,
    ) -> Self {
        self.window_stack = DockViewportFrontToBackWindowStack::from_windows(windows);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trusted_hovered_window_beats_backend_fallback_order() {
        let first = WindowId::from(1);
        let third = WindowId::from(3);
        let context = DockViewportTargetContext::from_window_signals_with_hovered_known(
            Some(third),
            true,
            vec![first],
        );

        assert_eq!(context.trusted_hovered_window(), Some(third));
        assert_eq!(context.backend_hover_fallback_window_stack(), &[first]);
    }

    #[test]
    fn front_to_back_window_stack_preserves_first_occurrence() {
        let front = WindowId::from(1);
        let back = WindowId::from(2);
        let context = DockViewportTargetContext::from_window_signals(
            DockViewportTrustedHoveredSignal::Unavailable,
            DockViewportFrontToBackWindowStack::from_window_ids([front, back, front]),
        );

        assert_eq!(
            context.backend_hover_fallback_window_stack(),
            &[front, back]
        );
    }

    #[test]
    fn trusted_hovered_window_match_requires_an_explicit_receiver_window() {
        let hovered = WindowId::from(7);
        let context = DockViewportTargetContext::from_window_signals(
            DockViewportTrustedHoveredSignal::Trusted(hovered),
            DockViewportFrontToBackWindowStack::default(),
        );

        assert!(!context.trusted_hovered_window_matches_event_receiver(None));
        assert!(!context.trusted_hovered_window_matches_event_receiver(Some(WindowId::from(8))));
        assert!(context.trusted_hovered_window_matches_event_receiver(Some(hovered)));
    }
}
