use crate::{
    DockViewportFrontToBackWindowStack, DockViewportTargetContext, DockViewportTrustedHoveredSignal,
};
use open_gpui::{
    AnyWindowHandle, App, PlatformHoveredWindow, PlatformViewportCapabilities, Window, WindowId,
};

/// Snapshot of platform window signals used to arbitrate overlapping viewport hits.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct DockViewportPlatformSignals {
    /// Trusted backend window reported by the platform as being under the pointer.
    trusted_hovered_signal: DockViewportTrustedHoveredSignal,
    /// Window that delivered the GPUI drag/drop event.
    event_receiver_window: Option<WindowId>,
    /// Front-to-back window stack, when the platform provides it.
    window_stack: DockViewportFrontToBackWindowStack,
    /// Window bounds are reported in a shared desktop coordinate space.
    global_window_bounds: bool,
}

impl DockViewportPlatformSignals {
    /// Captures GPUI application-level platform signals.
    pub(crate) fn from_app(cx: &App) -> Self {
        let capabilities = cx.viewport_capabilities();
        let trusted_hovered_signal =
            trusted_hovered_signal_from_platform(cx.hovered_window(), capabilities);
        let window_stack = if capabilities.window_stack {
            DockViewportFrontToBackWindowStack::from_windows(cx.window_stack().unwrap_or_default())
        } else {
            DockViewportFrontToBackWindowStack::default()
        };
        Self {
            trusted_hovered_signal,
            event_receiver_window: None,
            window_stack,
            global_window_bounds: capabilities.global_window_bounds,
        }
    }

    /// Captures GPUI platform signals for a host that delivered this drag/drop event.
    pub(crate) fn from_event_receiver_window(window: &Window, cx: &App) -> Self {
        Self::from_app(cx).with_event_receiver_window(window.window_handle())
    }

    /// Captures app-level signals for release paths that did not sample the hovered window.
    #[cfg(test)]
    pub(crate) fn from_app_without_hovered_window_authority(cx: &App) -> Self {
        let mut signals = Self::from_app(cx);
        signals.trusted_hovered_signal = DockViewportTrustedHoveredSignal::Unavailable;
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
        self
    }

    /// Adds the GPUI event receiver window signal.
    pub(crate) fn with_event_receiver_window(mut self, window: impl Into<AnyWindowHandle>) -> Self {
        self.event_receiver_window = Some(window.into().window_id());
        self
    }

    pub(crate) fn has_global_window_bounds(&self) -> bool {
        self.global_window_bounds
    }

    pub(crate) fn event_receiver_window(&self) -> Option<WindowId> {
        self.event_receiver_window
    }

    /// Converts the platform snapshot into the pure resolver context.
    pub(crate) fn target_context(&self) -> DockViewportTargetContext {
        DockViewportTargetContext::from_window_signals(
            self.trusted_hovered_signal,
            self.window_stack.clone(),
        )
    }

    #[cfg(test)]
    pub(crate) fn from_target_context(target_context: DockViewportTargetContext) -> Self {
        let (trusted_hovered_signal, window_stack) = target_context.into_window_signals();
        Self {
            trusted_hovered_signal,
            event_receiver_window: None,
            window_stack,
            global_window_bounds: true,
        }
    }

    #[cfg(test)]
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
            "reliable hovered=None remains explicit backend authority"
        );
    }
}
