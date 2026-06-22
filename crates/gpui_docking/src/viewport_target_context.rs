use open_gpui::{AnyWindowHandle, WindowId};

/// Source of the front-to-back viewport ordering used by hover fallback.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) enum DockViewportWindowStackSource {
    /// The platform did not provide a usable ordering signal.
    #[default]
    Unavailable,
    /// The ordering came from the backend platform window stack.
    Platform,
    /// The ordering was derived from ImGui-style focused-viewport stamps.
    FocusStampFallback,
}

/// Front-to-back platform window stack captured for fallback ordering.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct DockViewportFrontToBackWindowStack {
    windows: Vec<WindowId>,
    source: DockViewportWindowStackSource,
}

impl DockViewportFrontToBackWindowStack {
    pub(crate) fn from_window_ids(
        windows: impl IntoIterator<Item = WindowId>,
        source: DockViewportWindowStackSource,
    ) -> Self {
        let mut normalized = Vec::new();
        for window_id in windows {
            if normalized.iter().any(|existing| *existing == window_id) {
                continue;
            }
            normalized.push(window_id);
        }
        let source = if normalized.is_empty() {
            DockViewportWindowStackSource::Unavailable
        } else {
            source
        };
        Self {
            windows: normalized,
            source,
        }
    }

    pub(crate) fn from_platform_windows<I, W>(windows: I) -> Self
    where
        I: IntoIterator<Item = W>,
        W: Into<AnyWindowHandle>,
    {
        Self::from_window_ids(
            windows.into_iter().map(|window| window.into().window_id()),
            DockViewportWindowStackSource::Platform,
        )
    }

    pub(crate) fn as_slice(&self) -> &[WindowId] {
        &self.windows
    }

    #[cfg(test)]
    pub(crate) fn source(&self) -> DockViewportWindowStackSource {
        self.source
    }

    pub(crate) fn is_unavailable(&self) -> bool {
        self.source == DockViewportWindowStackSource::Unavailable
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
            DockViewportFrontToBackWindowStack::from_window_ids(
                window_stack,
                DockViewportWindowStackSource::Platform,
            ),
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

    pub(crate) fn front_to_back_window_stack_for_hover_fallback(&self) -> &[WindowId] {
        self.window_stack.as_slice()
    }

    #[cfg(test)]
    pub(crate) fn window_stack_source(&self) -> DockViewportWindowStackSource {
        self.window_stack.source()
    }

    pub(crate) fn has_hover_fallback_window_stack(&self) -> bool {
        !self.window_stack.is_unavailable()
    }

    pub(crate) fn without_trusted_hovered_window(mut self) -> Self {
        self.trusted_hovered_signal = DockViewportTrustedHoveredSignal::Unavailable;
        self
    }

    pub(crate) fn with_last_hovered_viewport_window(mut self, window_id: WindowId) -> Self {
        if matches!(
            self.trusted_hovered_signal,
            DockViewportTrustedHoveredSignal::Unavailable
        ) {
            self.trusted_hovered_signal = DockViewportTrustedHoveredSignal::Trusted(window_id);
        }
        self
    }

    pub(crate) fn with_focus_stamp_window_stack(
        mut self,
        windows: impl IntoIterator<Item = WindowId>,
    ) -> Self {
        if self.has_hover_fallback_window_stack() {
            return self;
        }
        self.window_stack = DockViewportFrontToBackWindowStack::from_window_ids(
            windows,
            DockViewportWindowStackSource::FocusStampFallback,
        );
        self
    }

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
        self.window_stack = DockViewportFrontToBackWindowStack::from_platform_windows(windows);
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
        assert_eq!(
            context.front_to_back_window_stack_for_hover_fallback(),
            &[first]
        );
        assert_eq!(
            context.window_stack_source(),
            DockViewportWindowStackSource::Platform
        );
    }

    #[test]
    fn clearing_trusted_hovered_window_preserves_stack_fallback() {
        let top = WindowId::from(1);
        let underlay = WindowId::from(2);
        let context = DockViewportTargetContext::from_window_signals_with_hovered_known(
            Some(top),
            true,
            vec![top, underlay],
        )
        .without_trusted_hovered_window();

        assert_eq!(context.trusted_hovered_window(), None);
        assert_eq!(
            context.trusted_hovered_signal(),
            DockViewportTrustedHoveredSignal::Unavailable
        );
        assert_eq!(
            context.front_to_back_window_stack_for_hover_fallback(),
            &[top, underlay]
        );
        assert_eq!(
            context.window_stack_source(),
            DockViewportWindowStackSource::Platform
        );
    }

    #[test]
    fn last_hovered_viewport_only_fills_unavailable_hovered_signal() {
        let last_hovered = WindowId::from(7);
        let current_hovered = WindowId::from(9);

        let unavailable =
            DockViewportTargetContext::default().with_last_hovered_viewport_window(last_hovered);
        assert_eq!(unavailable.trusted_hovered_window(), Some(last_hovered));

        let trusted_current = DockViewportTargetContext::from_window_signals(
            DockViewportTrustedHoveredSignal::Trusted(current_hovered),
            DockViewportFrontToBackWindowStack::default(),
        )
        .with_last_hovered_viewport_window(last_hovered);
        assert_eq!(
            trusted_current.trusted_hovered_window(),
            Some(current_hovered),
            "fresh backend hovered-window authority wins over the drag's last hovered viewport"
        );

        let trusted_none = DockViewportTargetContext::from_window_signals(
            DockViewportTrustedHoveredSignal::TrustedNone,
            DockViewportFrontToBackWindowStack::default(),
        )
        .with_last_hovered_viewport_window(last_hovered);
        assert_eq!(
            trusted_none.trusted_hovered_signal(),
            DockViewportTrustedHoveredSignal::TrustedNone,
            "explicit hovered=None remains authoritative outside the drag fallback path"
        );
    }

    #[test]
    fn front_to_back_window_stack_preserves_first_occurrence() {
        let front = WindowId::from(1);
        let back = WindowId::from(2);
        let context = DockViewportTargetContext::from_window_signals(
            DockViewportTrustedHoveredSignal::Unavailable,
            DockViewportFrontToBackWindowStack::from_window_ids(
                [front, back, front],
                DockViewportWindowStackSource::Platform,
            ),
        );

        assert_eq!(
            context.front_to_back_window_stack_for_hover_fallback(),
            &[front, back]
        );
    }

    #[test]
    fn focus_stamp_stack_only_fills_missing_platform_stack() {
        let focused = WindowId::from(7);
        let platform_front = crate::viewport_test_support::handle(9);

        let fallback = DockViewportTargetContext::new().with_focus_stamp_window_stack([focused]);
        assert_eq!(
            fallback.front_to_back_window_stack_for_hover_fallback(),
            &[focused]
        );
        assert_eq!(
            fallback.window_stack_source(),
            DockViewportWindowStackSource::FocusStampFallback
        );

        let platform = DockViewportTargetContext::new()
            .with_window_stack([platform_front])
            .with_focus_stamp_window_stack([focused]);
        assert_eq!(
            platform.front_to_back_window_stack_for_hover_fallback(),
            &[platform_front.window_id()]
        );
        assert_eq!(
            platform.window_stack_source(),
            DockViewportWindowStackSource::Platform
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
