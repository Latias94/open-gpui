use crate::DockViewportTargetContext;
use open_gpui::{AnyWindowHandle, App, PlatformViewportCapabilities, Window, WindowId};

/// Snapshot of platform window signals used to arbitrate overlapping viewport hits.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct DockViewportPlatformSignals {
    /// Window reported by the platform as being under the pointer.
    platform_hovered_window: Option<WindowId>,
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
        Self {
            platform_hovered_window: capabilities
                .mouse_hovered_window
                .then(|| cx.hovered_window().map(|window| window.window_id()))
                .flatten(),
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
        self.platform_hovered_window = None;
        self.capabilities.mouse_hovered_window = false;
        self
    }

    /// Adds the platform hovered window signal.
    #[cfg(test)]
    pub(crate) fn with_hovered_window(mut self, window: impl Into<AnyWindowHandle>) -> Self {
        self.platform_hovered_window = Some(window.into().window_id());
        self.capabilities.mouse_hovered_window = true;
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

    /// Converts the platform snapshot into the pure resolver context.
    #[cfg(test)]
    pub(crate) fn target_context(&self) -> DockViewportTargetContext {
        DockViewportTargetContext::from_window_and_event_signals_with_hovered_known(
            self.platform_hovered_window,
            self.capabilities.mouse_hovered_window,
            self.event_receiver_window,
            self.window_stack.clone(),
        )
    }

    #[cfg(test)]
    pub(crate) fn from_target_context(target_context: DockViewportTargetContext) -> Self {
        let (
            platform_hovered_window,
            platform_hovered_window_known,
            event_receiver_window,
            window_stack,
        ) = target_context.into_window_signals();
        Self {
            platform_hovered_window,
            event_receiver_window,
            window_stack,
            capabilities: PlatformViewportCapabilities {
                global_window_bounds: true,
                mouse_hovered_window: platform_hovered_window_known,
                window_stack: true,
                ..Default::default()
            },
        }
    }

    #[cfg(test)]
    pub(crate) fn with_global_window_bounds(mut self, supported: bool) -> Self {
        self.capabilities.global_window_bounds = supported;
        self
    }
}

impl From<DockViewportPlatformSignals> for DockViewportTargetContext {
    fn from(signals: DockViewportPlatformSignals) -> Self {
        Self::from_window_and_event_signals_with_hovered_known(
            signals.platform_hovered_window,
            signals.capabilities.mouse_hovered_window,
            signals.event_receiver_window,
            signals.window_stack,
        )
    }
}
