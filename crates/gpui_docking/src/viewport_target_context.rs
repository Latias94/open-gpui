#[cfg(test)]
use open_gpui::AnyWindowHandle;
use open_gpui::WindowId;

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
    /// The backend reported a hovered window, but docking intentionally discarded that authority.
    Discarded,
}

/// Pure resolver context derived from platform window signals.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct DockViewportTargetContext {
    /// Trusted backend hovered-window signal.
    trusted_hovered_signal: DockViewportTrustedHoveredSignal,
    /// Front-to-back window stack, when the platform provides it.
    window_stack: Vec<WindowId>,
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
            Self::TrustedNone | Self::Unavailable | Self::Discarded => None,
        }
    }
}

impl DockViewportTargetContext {
    pub(crate) fn from_window_signals(
        trusted_hovered_signal: DockViewportTrustedHoveredSignal,
        window_stack: Vec<WindowId>,
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
            window_stack,
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

    pub(crate) fn trusted_hovered_window_unavailable(&self) -> bool {
        matches!(
            self.trusted_hovered_signal,
            DockViewportTrustedHoveredSignal::Unavailable
        )
    }

    pub(crate) fn trusted_hovered_window_known_empty(&self) -> bool {
        matches!(
            self.trusted_hovered_signal,
            DockViewportTrustedHoveredSignal::TrustedNone
        )
    }

    pub(crate) fn trusted_hovered_window_authority_discarded(&self) -> bool {
        matches!(
            self.trusted_hovered_signal,
            DockViewportTrustedHoveredSignal::Discarded
        )
    }

    pub(crate) fn without_trusted_hovered_window_authority(&self) -> Self {
        Self {
            trusted_hovered_signal: DockViewportTrustedHoveredSignal::Discarded,
            window_stack: self.window_stack.clone(),
        }
    }

    pub(crate) fn backend_hover_fallback_window_stack(&self) -> &[WindowId] {
        &self.window_stack
    }

    #[cfg(test)]
    pub(crate) fn into_window_signals(self) -> (DockViewportTrustedHoveredSignal, Vec<WindowId>) {
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
        assert!(!context.trusted_hovered_window_unavailable());
    }

    #[test]
    fn trusted_hovered_window_match_requires_an_explicit_receiver_window() {
        let hovered = WindowId::from(7);
        let context = DockViewportTargetContext::from_window_signals(
            DockViewportTrustedHoveredSignal::Trusted(hovered),
            vec![],
        );

        assert!(!context.trusted_hovered_window_matches_event_receiver(None));
        assert!(!context.trusted_hovered_window_matches_event_receiver(Some(WindowId::from(8))));
        assert!(context.trusted_hovered_window_matches_event_receiver(Some(hovered)));
    }
}
