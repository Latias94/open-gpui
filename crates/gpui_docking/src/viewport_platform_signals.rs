use crate::{
    DockViewportFrontToBackWindowStack, DockViewportTargetContext, DockViewportTrustedHoveredSignal,
};
use open_gpui::{
    AnyWindowHandle, App, PlatformFocusedWindow, PlatformHoveredWindow,
    PlatformViewportCapabilities, Window, WindowId,
};

/// Backend focus permit for using focus stamps as a hover fallback source.
///
/// This token is derived from the current backend focus signal. An available token preserves
/// focus-stamp fallback eligibility for a live app snapshot; it does not promote synthetic
/// snapshots into focus-stamp fallback eligibility.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DockViewportFocusStampFallbackPermit {
    Available,
    Unavailable,
}

impl DockViewportFocusStampFallbackPermit {
    pub(crate) fn from_backend_focus(focus: PlatformFocusedWindow) -> Self {
        if focus.is_available() {
            Self::Available
        } else {
            Self::Unavailable
        }
    }

    #[cfg(test)]
    fn available_for_test() -> Self {
        Self::Available
    }

    #[cfg(test)]
    fn unavailable_for_test() -> Self {
        Self::Unavailable
    }

    fn is_unavailable(self) -> bool {
        matches!(self, Self::Unavailable)
    }
}

/// Whether focus-stamp ordering is eligible as an ImGui-style backend fallback.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
enum DockViewportFocusStampFallbackPolicy {
    /// The snapshot was explicit or backend focus was unavailable.
    #[default]
    Disabled,
    /// The snapshot came from live backend signals and may use focus stamps when needed.
    LiveBackendAllowed,
}

impl DockViewportFocusStampFallbackPolicy {
    fn allows_focus_stamp_fallback(self) -> bool {
        matches!(self, Self::LiveBackendAllowed)
    }
}

/// Whether target-arbitration signals may be refreshed from the live app backend.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
enum DockViewportTargetContextResampling {
    /// The snapshot was constructed explicitly and must not consult live backend state.
    #[default]
    FrozenSnapshot,
    /// The snapshot came from the live app backend and may be refreshed before delivery.
    LiveAppBackend,
}

impl DockViewportTargetContextResampling {
    fn allows_live_app_backend(self) -> bool {
        matches!(self, Self::LiveAppBackend)
    }
}

/// Snapshot of platform window signals used to arbitrate overlapping viewport hits.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct DockViewportPlatformSignals {
    /// Trusted backend window reported by the platform as being under the pointer.
    trusted_hovered_signal: DockViewportTrustedHoveredSignal,
    /// ImGui-style drag fallback captured by the runtime when the hovered-window signal is unavailable.
    drag_last_hovered_window: Option<WindowId>,
    /// Window that delivered the GPUI drag/drop event.
    event_receiver_window: Option<WindowId>,
    /// Front-to-back window stack, when the platform provides it.
    window_stack: DockViewportFrontToBackWindowStack,
    /// Window bounds are reported in a shared desktop coordinate space.
    global_window_bounds: bool,
    /// Target arbitration signals came from an explicit snapshot or the live app backend.
    target_context_resampling: DockViewportTargetContextResampling,
    /// ImGui-style focused-window stamp fallback policy for this platform snapshot.
    focus_stamp_fallback_policy: DockViewportFocusStampFallbackPolicy,
}

impl DockViewportPlatformSignals {
    /// Captures GPUI application-level platform signals.
    pub(crate) fn from_app(cx: &App) -> Self {
        let capabilities = cx.viewport_capabilities();
        let trusted_hovered_signal =
            trusted_hovered_signal_from_platform(cx.hovered_window(), capabilities);
        let window_stack = if capabilities.window_stack {
            DockViewportFrontToBackWindowStack::from_platform_windows(
                cx.window_stack().unwrap_or_default(),
            )
        } else {
            DockViewportFrontToBackWindowStack::default()
        };
        Self {
            trusted_hovered_signal,
            drag_last_hovered_window: None,
            event_receiver_window: None,
            window_stack,
            global_window_bounds: capabilities.global_window_bounds,
            target_context_resampling: DockViewportTargetContextResampling::LiveAppBackend,
            focus_stamp_fallback_policy: DockViewportFocusStampFallbackPolicy::LiveBackendAllowed,
        }
    }

    /// Captures GPUI platform signals for a host that delivered this drag/drop event.
    pub(crate) fn from_event_receiver_window(window: &Window, cx: &App) -> Self {
        Self::from_app(cx).with_event_receiver_window(window.window_handle())
    }

    /// Refreshes backend target arbitration signals while preserving release-time coordinate
    /// semantics captured from the event receiver.
    pub(crate) fn with_resampled_target_context_from_app(mut self, cx: &App) -> Self {
        if !self.target_context_resampling.allows_live_app_backend() {
            return self;
        }
        let current = Self::from_app(cx);
        self.trusted_hovered_signal = current.trusted_hovered_signal;
        self.window_stack = current.window_stack;
        self
    }

    pub(crate) fn with_drag_last_hovered_viewport_window(self, window_id: WindowId) -> Self {
        let target_context = self
            .target_context()
            .with_drag_last_hovered_viewport_window(window_id);
        self.apply_target_context(target_context)
    }

    pub(crate) fn with_focus_stamp_window_stack(
        self,
        windows: impl IntoIterator<Item = WindowId>,
    ) -> Self {
        let target_context = self.target_context().with_focus_stamp_window_stack(windows);
        self.apply_target_context(target_context)
    }

    /// Captures app-level signals for release paths that did not sample the hovered window.
    #[cfg(test)]
    pub(crate) fn from_app_without_hovered_window_signal(cx: &App) -> Self {
        let mut signals = Self::from_app(cx);
        signals.trusted_hovered_signal = DockViewportTrustedHoveredSignal::Unavailable;
        signals.target_context_resampling = DockViewportTargetContextResampling::FrozenSnapshot;
        signals
    }

    /// Captures app-level signals for tests where backend hover and window-stack signals are absent.
    #[cfg(test)]
    pub(crate) fn from_app_without_target_window_signals(cx: &App) -> Self {
        let mut signals = Self::from_app_without_hovered_window_signal(cx);
        signals.window_stack = DockViewportFrontToBackWindowStack::default();
        signals
    }

    /// Adds the trusted backend hovered-window signal.
    #[cfg(test)]
    pub(crate) fn with_trusted_hovered_window(
        mut self,
        window: impl Into<AnyWindowHandle>,
    ) -> Self {
        self.trusted_hovered_signal =
            DockViewportTrustedHoveredSignal::Trusted(window.into().window_id());
        self.target_context_resampling = DockViewportTargetContextResampling::FrozenSnapshot;
        self
    }

    #[cfg(test)]
    pub(crate) fn with_target_context_resampling_from_app(mut self) -> Self {
        self.target_context_resampling = DockViewportTargetContextResampling::LiveAppBackend;
        self
    }

    /// Keeps target-window arbitration facts exactly as they were sampled for this event.
    pub(crate) fn with_frozen_target_context(mut self) -> Self {
        self.target_context_resampling = DockViewportTargetContextResampling::FrozenSnapshot;
        self
    }

    /// Adds the GPUI event receiver window signal.
    pub(crate) fn with_event_receiver_window(mut self, window: impl Into<AnyWindowHandle>) -> Self {
        self.event_receiver_window = Some(window.into().window_id());
        self
    }

    pub(crate) fn without_trusted_hovered_window(mut self) -> Self {
        self.trusted_hovered_signal = DockViewportTrustedHoveredSignal::Unavailable;
        self
    }

    pub(crate) fn has_global_window_bounds(&self) -> bool {
        self.global_window_bounds
    }

    pub(crate) fn event_receiver_window(&self) -> Option<WindowId> {
        self.event_receiver_window
    }

    pub(crate) fn allows_focus_stamp_fallback(&self) -> bool {
        self.focus_stamp_fallback_policy
            .allows_focus_stamp_fallback()
    }

    pub(crate) fn with_focus_stamp_fallback_permit(
        mut self,
        permit: DockViewportFocusStampFallbackPermit,
    ) -> Self {
        if permit.is_unavailable() {
            self.focus_stamp_fallback_policy = DockViewportFocusStampFallbackPolicy::Disabled;
            let target_context = self.target_context().without_focus_stamp_window_stack();
            self = self.apply_target_context(target_context);
        }
        self
    }

    /// Converts the platform snapshot into the pure resolver context.
    pub(crate) fn target_context(&self) -> DockViewportTargetContext {
        DockViewportTargetContext::from_window_signals(
            self.trusted_hovered_signal,
            self.window_stack.clone(),
        )
        .with_optional_drag_last_hovered_window(self.drag_last_hovered_window)
    }

    #[cfg(test)]
    pub(crate) fn from_target_context(target_context: DockViewportTargetContext) -> Self {
        let window_signals = target_context.into_window_signals();
        Self {
            trusted_hovered_signal: window_signals.trusted_hovered_signal,
            drag_last_hovered_window: window_signals.drag_last_hovered_window,
            event_receiver_window: None,
            window_stack: window_signals.window_stack,
            global_window_bounds: true,
            target_context_resampling: DockViewportTargetContextResampling::FrozenSnapshot,
            focus_stamp_fallback_policy: DockViewportFocusStampFallbackPolicy::Disabled,
        }
    }

    fn apply_target_context(mut self, target_context: DockViewportTargetContext) -> Self {
        let window_signals = target_context.into_window_signals();
        self.trusted_hovered_signal = window_signals.trusted_hovered_signal;
        self.drag_last_hovered_window = window_signals.drag_last_hovered_window;
        self.window_stack = window_signals.window_stack;
        self
    }

    pub(crate) fn with_global_window_bounds(mut self, supported: bool) -> Self {
        self.global_window_bounds = supported;
        self
    }
}

fn trusted_hovered_signal_from_platform(
    hovered_window: PlatformHoveredWindow,
    capabilities: PlatformViewportCapabilities,
) -> DockViewportTrustedHoveredSignal {
    match hovered_window {
        PlatformHoveredWindow::Unavailable => DockViewportTrustedHoveredSignal::Unavailable,
        PlatformHoveredWindow::NoWindow => DockViewportTrustedHoveredSignal::TrustedNone,
        PlatformHoveredWindow::Window(window) if capabilities.hovered_window_ignores_no_input => {
            DockViewportTrustedHoveredSignal::Trusted(window.window_id())
        }
        PlatformHoveredWindow::Window(_) => DockViewportTrustedHoveredSignal::Unavailable,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::viewport_test_support::handle;

    #[test]
    fn hovered_window_signal_requires_no_input_passthrough_capability() {
        let window = handle(7);
        let passthrough_capabilities = PlatformViewportCapabilities {
            hovered_window_ignores_no_input: true,
            ..Default::default()
        };
        let non_passthrough_capabilities = PlatformViewportCapabilities {
            hovered_window_ignores_no_input: false,
            ..Default::default()
        };

        assert_eq!(
            trusted_hovered_signal_from_platform(
                PlatformHoveredWindow::Window(window),
                passthrough_capabilities,
            ),
            DockViewportTrustedHoveredSignal::Trusted(window.window_id())
        );
        assert_eq!(
            trusted_hovered_signal_from_platform(
                PlatformHoveredWindow::Window(window),
                non_passthrough_capabilities,
            ),
            DockViewportTrustedHoveredSignal::Unavailable,
            "a hovered window is not trusted unless the backend can ignore no-input viewports"
        );
        assert_eq!(
            trusted_hovered_signal_from_platform(
                PlatformHoveredWindow::NoWindow,
                non_passthrough_capabilities,
            ),
            DockViewportTrustedHoveredSignal::TrustedNone,
            "reliable hovered=None remains an explicit backend hovered-window signal"
        );
    }

    #[open_gpui::test]
    fn frozen_target_context_is_not_overwritten_by_later_app_hover(
        cx: &mut open_gpui::TestAppContext,
    ) {
        let target = handle(7);
        let later_hover = handle(8);
        let signals = DockViewportPlatformSignals::from_target_context(
            DockViewportTargetContext::new().with_trusted_hovered_window(target),
        )
        .with_target_context_resampling_from_app()
        .with_frozen_target_context();
        cx.set_platform_hovered_window(Some(later_hover));

        let resampled = cx.update(|app| signals.with_resampled_target_context_from_app(app));

        assert_eq!(
            resampled.target_context().trusted_hovered_window(),
            Some(target.window_id())
        );
    }

    #[test]
    fn disabling_focus_stamp_fallback_clears_only_focus_stamp_stack() {
        let focused = handle(7);
        let platform_front = handle(9);

        let focus_stamp_signals = DockViewportPlatformSignals::from_target_context(
            DockViewportTargetContext::new().with_focus_stamp_window_stack([focused.window_id()]),
        )
        .with_focus_stamp_fallback_permit(
            DockViewportFocusStampFallbackPermit::unavailable_for_test(),
        );
        assert!(!focus_stamp_signals.allows_focus_stamp_fallback());
        assert_eq!(
            focus_stamp_signals
                .target_context()
                .front_to_back_window_stack_for_hover_fallback(),
            &[]
        );

        let platform_signals = DockViewportPlatformSignals::from_target_context(
            DockViewportTargetContext::new().with_window_stack([platform_front]),
        )
        .with_focus_stamp_fallback_permit(
            DockViewportFocusStampFallbackPermit::unavailable_for_test(),
        );
        assert!(!platform_signals.allows_focus_stamp_fallback());
        assert_eq!(
            platform_signals
                .target_context()
                .front_to_back_window_stack_for_hover_fallback(),
            &[platform_front.window_id()]
        );
    }

    #[test]
    fn focus_stamp_permit_is_derived_from_backend_focus_availability() {
        let focused = handle(7);

        let window_focus_permit = DockViewportFocusStampFallbackPermit::from_backend_focus(
            PlatformFocusedWindow::Window(focused),
        );
        assert!(!window_focus_permit.is_unavailable());

        let no_window_focus_permit = DockViewportFocusStampFallbackPermit::from_backend_focus(
            PlatformFocusedWindow::NoWindow,
        );
        assert!(!no_window_focus_permit.is_unavailable());

        let unavailable_focus_permit = DockViewportFocusStampFallbackPermit::from_backend_focus(
            PlatformFocusedWindow::Unavailable,
        );
        assert!(unavailable_focus_permit.is_unavailable());
    }

    #[test]
    fn available_focus_stamp_permit_preserves_existing_focus_stamp_stack() {
        let focused = handle(7);
        let signals = DockViewportPlatformSignals::from_target_context(
            DockViewportTargetContext::new().with_focus_stamp_window_stack([focused.window_id()]),
        )
        .with_focus_stamp_fallback_permit(
            DockViewportFocusStampFallbackPermit::available_for_test(),
        );

        assert_eq!(
            signals
                .target_context()
                .front_to_back_window_stack_for_hover_fallback(),
            &[focused.window_id()]
        );
        assert!(!signals.allows_focus_stamp_fallback());
    }
}
