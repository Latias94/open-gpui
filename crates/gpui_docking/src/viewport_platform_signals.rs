use crate::DockViewportTargetContext;
use open_gpui::{AnyWindowHandle, App, PlatformViewportCapabilities, Window, WindowId};

/// Snapshot of platform window signals used to arbitrate overlapping viewport hits.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct DockViewportPlatformSignals {
    /// Window known to be under the pointer for this docking route event.
    hovered_window: Option<WindowId>,
    /// Platform-active window, when known.
    active_window: Option<WindowId>,
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
            hovered_window: None,
            active_window: capabilities
                .active_window
                .then(|| cx.active_window().map(|window| window.window_id()))
                .flatten(),
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

    /// Captures GPUI platform signals for a host known to be under the dragged payload.
    pub(crate) fn from_hovered_window(window: &Window, cx: &App) -> Self {
        Self::from_app(cx).with_hovered_window(window.window_handle())
    }

    /// Adds the hovered window signal.
    pub(crate) fn with_hovered_window(mut self, window: impl Into<AnyWindowHandle>) -> Self {
        self.hovered_window = Some(window.into().window_id());
        self
    }

    pub(crate) fn has_global_window_bounds(&self) -> bool {
        self.capabilities.global_window_bounds
    }

    /// Converts the platform snapshot into the pure resolver context.
    #[cfg(test)]
    pub(crate) fn target_context(&self) -> DockViewportTargetContext {
        DockViewportTargetContext::from_window_signals(
            self.hovered_window,
            self.active_window,
            self.window_stack.clone(),
        )
    }

    #[cfg(test)]
    pub(crate) fn from_target_context(target_context: DockViewportTargetContext) -> Self {
        let (hovered_window, active_window, window_stack) = target_context.into_window_signals();
        Self {
            hovered_window,
            active_window,
            window_stack,
            capabilities: PlatformViewportCapabilities {
                global_window_bounds: true,
                active_window: true,
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
        Self::from_window_signals(
            signals.hovered_window,
            signals.active_window,
            signals.window_stack,
        )
    }
}
