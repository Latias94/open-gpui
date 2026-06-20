use crate::{DockViewportTargetContext, DockViewportTrustedHoveredSignal};
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
    window_stack: Vec<WindowId>,
    /// Platform capabilities used to decide which signals are reliable.
    capabilities: PlatformViewportCapabilities,
}

impl DockViewportPlatformSignals {
    /// Captures GPUI application-level platform signals.
    pub(crate) fn from_app(cx: &App) -> Self {
        let capabilities = cx.viewport_capabilities();
        let trusted_hovered_signal = trusted_hovered_signal_from_platform(cx.hovered_window());
        Self {
            trusted_hovered_signal,
            event_receiver_window: None,
            window_stack: if capabilities.window_stack {
                cx.window_stack()
                    .unwrap_or_default()
                    .into_iter()
                    .map(|window| window.window_id())
                    .collect()
            } else {
                Vec::new()
            },
            capabilities,
        }
    }

    /// Captures GPUI platform signals for a host that delivered this drag/drop event.
    pub(crate) fn from_event_receiver_window(window: &Window, cx: &App) -> Self {
        Self::from_app(cx).with_event_receiver_window(window.window_handle())
    }

    /// Captures app-level signals for release paths that did not sample the hovered window.
    #[cfg(test)]
    pub(crate) fn from_app_without_hovered_window_authority(cx: &App) -> Self {
        Self::from_app(cx).without_hovered_window_authority()
    }

    /// Removes hovered-window authority while preserving other platform signals.
    #[cfg(test)]
    pub(crate) fn without_hovered_window_authority(mut self) -> Self {
        self.trusted_hovered_signal = DockViewportTrustedHoveredSignal::Unavailable;
        self
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
        self.capabilities.global_window_bounds
    }

    pub(crate) fn hovered_window_ignores_no_input(&self) -> bool {
        self.capabilities.hovered_window_ignores_no_input
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
            capabilities: PlatformViewportCapabilities {
                global_window_bounds: true,
                window_stack: true,
                // Synthetic test contexts stay conservative unless a case opts in explicitly.
                hovered_window_ignores_no_input: false,
                ..Default::default()
            },
        }
    }

    #[cfg(test)]
    pub(crate) fn with_global_window_bounds(mut self, supported: bool) -> Self {
        self.capabilities.global_window_bounds = supported;
        self
    }

    #[cfg(test)]
    pub(crate) fn with_hovered_window_ignores_no_input(mut self, supported: bool) -> Self {
        self.capabilities.hovered_window_ignores_no_input = supported;
        self
    }
}

fn trusted_hovered_signal_from_platform(
    hovered_window: PlatformHoveredWindow,
) -> DockViewportTrustedHoveredSignal {
    match hovered_window {
        PlatformHoveredWindow::Unavailable => DockViewportTrustedHoveredSignal::Unavailable,
        PlatformHoveredWindow::NoWindow => DockViewportTrustedHoveredSignal::TrustedNone,
        PlatformHoveredWindow::Window(window) => {
            DockViewportTrustedHoveredSignal::Trusted(window.window_id())
        }
    }
}
